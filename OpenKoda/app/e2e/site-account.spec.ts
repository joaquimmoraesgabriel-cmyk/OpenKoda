import { expect, test } from './fixtures';
import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import {resolve, sep} from 'node:path';
import type {Page} from '@playwright/test';
import {sessionBridge} from '../../site/worker.mjs';

const site='https://koda.example.invalid';
const auth='https://ynjcyvnrfxsrijrwpaye.supabase.co';
const dist=fileURLToPath(new URL('../../site/dist/',import.meta.url));
const version='2026-11-13';
// A test-only widget with controls for all three callbacks used by the browser.
const captchaWidget = `window.turnstile = {
  render(slot, options) {
    const events = [
      ['Concluir CAPTCHA de teste', () => options.callback('fixture-' + crypto.randomUUID())],
      ['Expirar CAPTCHA de teste', () => options['expired-callback']()],
      ['Falhar CAPTCHA de teste', () => options['error-callback']()]
    ];
    for (const [name, callback] of events) {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = name;
      button.addEventListener('click', callback);
      slot.append(button);
    }
    return 'widget-fixture';
  },
  remove() {},
  reset() {}
};`;
const completeCaptcha=(page:Page)=>page.getByRole('button',{name:'Concluir CAPTCHA de teste'}).click();

test.describe.configure({ mode: 'default' });
test.beforeAll(() => {
  const build = fileURLToPath(new URL('../../site/build.mjs', import.meta.url));
  execFileSync(process.execPath, [build]);
});

async function siteFixture(page:Page,accepted=false,options:{beforeAction?:(body:Record<string,unknown>)=>Promise<void>;account?:()=>unknown;failAction?:()=>string|undefined}={}){
  const posted:{action:string;marketing?:boolean}[]=[];
  let marketing=accepted;
  const tickets:{id:string;subject:string;message:string;status:string;createdAt:string;replies:unknown[]}[]=[];
  const registrations:{data:{adult_requested:boolean;privacy_requested:string}}[]=[];
  const user={id:'synthetic-user',email:'teste@example.invalid',email_confirmed_at:version};
  const challenges=new Map<string,string>(), spentCaptcha=new Set<string>();
  let recoveryCode='';
  let access='synthetic.credential.onlyfortesting',refresh='synthetic-refresh-001',validSession=false,refreshes=0;
  const provider=async(raw:string,init:RequestInit)=>{
    const url=new URL(raw);
    const body=init.body?JSON.parse(String(init.body)):{};
    if(url.href==='https://challenges.cloudflare.com/turnstile/v0/siteverify'){
      const success=body.secret==='fixture-secret'&&/^fixture-[\da-f-]{36}$/.test(body.response)&&!spentCaptcha.has(body.response);
      if(success)spentCaptcha.add(body.response);
      return Response.json({success,hostname:new URL(site).hostname,action:'login'});
    }
    expect(url.origin).toBe(auth);
    if(url.pathname==='/auth/v1/signup'){registrations.push(body);return Response.json({});}
    if(url.pathname==='/auth/v1/recover'){
      recoveryCode=randomUUID();
      challenges.set(recoveryCode,body.code_challenge);
      return Response.json({});
    }
    if(url.pathname==='/auth/v1/token'){
      if(url.searchParams.get('grant_type')==='pkce'){
        const hash=createHash('sha256').update(body.code_verifier).digest('base64url');
        if(challenges.get(body.auth_code)!==hash)return Response.json({error_code:'bad_code_verifier'},{status:400});
        challenges.delete(body.auth_code);
      }
      if(url.searchParams.get('grant_type')==='refresh_token'){
        if(body.refresh_token!==refresh||!validSession)return Response.json({},{status:400});
        refreshes++;refresh='synthetic-refresh-'+String(100+refreshes);access='synthetic.credential.rotated'+refreshes;
      }
      validSession=true;return Response.json({access_token:access,refresh_token:refresh,expires_in:3600,user});
    }
    if(url.pathname==='/auth/v1/user')return Response.json(validSession?user:{},{status:validSession?200:401});
    if(url.pathname==='/auth/v1/logout'){validSession=false;return new Response(null,{status:204});}
    expect(url.pathname).toBe('/functions/v1/openkoda');posted.push(body);
    await options.beforeAction?.(body);
    if(options.failAction?.()===body.action)return Response.json({error:'fixture_unavailable'},{status:503});
    if(body.action==='account')return Response.json(options.account?.()??{plan:null,consent:{acceptedAt:accepted?version:null,termsVersion:version,privacyVersion:version,requiredTermsVersion:version,requiredPrivacyVersion:version,marketing}});
    if(['consent_initial','consent'].includes(body.action)){accepted=true;marketing=body.marketing===true;}
    if(body.action==='marketing')marketing=body.enabled===true;
    if(body.action==='support'){tickets.push({id:body.requestId,subject:body.subject,message:body.message,status:'open',createdAt:new Date().toISOString(),replies:[]});return Response.json({id:body.requestId});}
    return Response.json(body.action==='support_list'?{tickets}:{});
  };
  await page.context().route('**/*',async route=>{
    const request=route.request(),url=new URL(request.url());
    if(url.origin===site){
      if(url.pathname.startsWith('/conta/api/')){
        const headers=await request.allHeaders();
        const r=await sessionBridge(new Request(url,{method:request.method(),headers,body:request.postData()??undefined}),provider,{TURNSTILE_SECRET_KEY:'fixture-secret'});
        const cookies=r.headers.getSetCookie();
        // Playwright splits newline-separated Set-Cookie fields on fulfillment.
        // Let the browser apply expiry, scope and HttpOnly to the response.
        return route.fulfill({status:r.status,contentType:'application/json',
          headers:cookies.length?{'set-cookie':cookies.join('\n')}:{},body:await r.text()});
      }
      const filename=resolve(dist,'.'+url.pathname+(url.pathname.endsWith('/')?'index.html':''));
      if(!filename.startsWith(resolve(dist)+sep))throw Error('Caminho de fixture inválido.');
      const mime=filename.endsWith('.js')?'text/javascript':filename.endsWith('.css')?'text/css':filename.endsWith('.png')?'image/png':'text/html';
      return route.fulfill({contentType:mime,body:await readFile(filename)});
    }
    if(url.origin==='https://challenges.cloudflare.com')
      return route.fulfill({contentType:'text/javascript',body:captchaWidget});
    expect(url.origin).toBe(auth);expect(url.pathname).toBe('/auth/v1/authorize');
    expect(url.searchParams.get('redirect_to')).toBe(site+'/conta/retorno/');
    expect(url.searchParams.get('code_challenge_method')).toBe('s256');
    const code=randomUUID(), challenge=url.searchParams.get('code_challenge')!;
    expect(challenge).toMatch(/^[-\w]{43}$/);
    challenges.set(code,challenge);
    return route.fulfill({contentType:'text/html',body:'<a href="'+site+'/conta/retorno/?code='+code+'">Entrar na conta de teste</a>'});
  });
  return {posted,registrations,refreshes:()=>refreshes,recovery:()=>recoveryCode};
}

for(const first of [true,false]){
  const scenario=first?'registra os aceites de uma conta nova após tentar novamente':'preserva as escolhas da conta existente ao entrar';
  test('Google '+scenario+' e mantém a sessão no F5',async({page})=>{
    const {posted}=await siteFixture(page,!first);
    await page.goto(site+(first?'/conta/retorno/?error=access_denied&error_description=valor_privado':'/conta/'),{waitUntil:'commit'});
    if(first){
      await expect(page.getByRole('heading',{name:'O login não foi concluído'})).toBeVisible();
      expect(new URL(page.url()).search).toBe('');
      await expect(page.locator('body')).not.toContainText('valor_privado');
    }
    const google=page.getByRole('button',{name:first?'Tentar com Google novamente':'Entrar com Google'});
    await expect(google).toBeDisabled();
    await completeCaptcha(page);
    if(!first){
      await expect(google).toBeEnabled();
      await page.getByRole('button',{name:'Expirar CAPTCHA de teste'}).click();
      await expect(google).toBeDisabled();
      await completeCaptcha(page);
      await page.getByRole('button',{name:'Falhar CAPTCHA de teste'}).click();
      await expect(google).toBeDisabled();
      await completeCaptcha(page);
    }
    await google.click();
    const next=page.getByRole('button',{name:'Continuar com Google'});
    await next.click();
    await expect(page.getByRole('heading',{name:'Continue com Google'})).toBeVisible();
    await page.getByRole('checkbox',{name:/18 anos/}).check();
    await page.getByRole('checkbox',{name:/Li e aceito/}).check();
    await Promise.all([page.waitForURL('**/auth/v1/authorize?**'),next.click()]);
    const destination=new URL(page.url());
    expect(destination.searchParams.get('code_challenge_method')).toBe('s256');
    if(first)expect(destination.searchParams.get('prompt')).toBe('select_account');
    await page.getByRole('link',{name:'Entrar na conta de teste'}).click();
    await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
    await page.reload({waitUntil:'commit'});
    await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
    await expect(page).toHaveURL(site+'/conta/');
    // A bookmark or tab restored at the cleaned callback can still hold a valid session.
    await page.goto(site+'/conta/retorno/',{waitUntil:'commit'});
    await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
    expect(posted.filter(p=>p.action==='consent_initial')).toEqual(first?[{action:'consent_initial',termsVersion:version,privacyVersion:version,marketing:false}]:[]);
  });
}

test('site associa o marketing opcional do cadastro por e-mail à primeira sessão confirmada',async({page})=>{
  const {posted,registrations}=await siteFixture(page);
  await page.goto(site+'/conta/?intent=cadastro');
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha de pelo menos 12 caracteres').fill('fixture-local-somente');
  await page.getByRole('checkbox',{name:/18 anos/}).check();
  await page.getByRole('checkbox',{name:/Li e aceito/}).check();
  await page.getByRole('checkbox',{name:/receber novidades/}).check();
  await completeCaptcha(page);
  await page.getByRole('button',{name:'Cadastrar'}).click();
  await expect(page.getByRole('heading',{name:'Entre no OpenKoda'})).toBeVisible();
  expect(registrations[0].data).toMatchObject({adult_requested:true,privacy_requested:version,terms_requested:version,marketing_requested:true});
  expect(posted.filter(p=>p.action==='consent_initial')).toHaveLength(0);
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('fixture-local-somente');
  await completeCaptcha(page);
  await page.getByRole('button',{name:'Entrar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  expect(posted.filter(p=>p.action==='consent_initial')).toEqual([{action:'consent_initial',termsVersion:version,privacyVersion:version,marketing:true}]);
});
test('site mantém a sessão no reload, nos preços e numa nova aba; sair revoga ambas',async({page,context})=>{
  const f=await siteFixture(page,true);
  await page.goto(site+'/conta/');
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('fixture-local-somente');
  await completeCaptcha(page);
  await page.getByRole('button',{name:'Entrar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  expect(await page.evaluate(()=>document.cookie)).not.toContain('koda-');
  const tickets=page.getByRole('link',{name:'Chamados',exact:true});
  await tickets.focus();await page.keyboard.press('Enter');
  await expect(page.getByRole('heading',{name:'Meus chamados e respostas'})).toBeVisible();
  await expect(page.getByText('Você ainda não abriu chamados.')).toBeVisible();
  await page.getByRole('link',{name:'Visão geral',exact:true}).click();
  await expect(page.getByText('Você ainda não abriu chamados.')).toBeHidden();
  await page.goto(site+'/conta/?plano=ultra');
  await expect(page.getByRole('radio',{name:/Ultra/})).toBeChecked();
  await page.getByRole('radio',{name:/Pro/}).check();
  await page.getByRole('button',{name:'Simular assinatura'}).click();
  await expect(page.getByText('Simulação registrada.',{exact:false})).toBeVisible();
  expect(f.posted.filter(p=>p.action==='checkout')).toEqual([expect.objectContaining({plan:'pro'})]);
  await page.reload();await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  await page.getByRole('link',{name:'Preços',exact:true}).click();
  await page.getByRole('link',{name:'Minha conta',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  // Simulate expiry of the shorter-lived access cookie.
  await context.clearCookies({name:'__Host-koda-access'});
  const next=await context.newPage();await next.goto(site+'/conta/');
  await expect(next.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  expect(f.refreshes()).toBe(1);
  await next.getByRole('button',{name:'Sair',exact:true}).click();
  await expect(next.getByRole('heading',{name:'Entre no OpenKoda'})).toBeVisible();
  await page.evaluate(()=>window.dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true})));
  await expect(page.getByRole('heading',{name:'Entre no OpenKoda'})).toBeVisible();
  await page.reload();await expect(page.getByRole('heading',{name:'Entre no OpenKoda'})).toBeVisible();
});

test('Google do aplicativo aguarda o CAPTCHA e devolve o código ao aplicativo com o PKCE nativo',async({page})=>{
  await siteFixture(page);
  const state=randomUUID(),challenge='p'.repeat(43);
  const loopback='http://127.0.0.1:19834/openkoda/callback';
  await page.route(loopback+'?*',route=>route.fulfill({contentType:'text/html',body:'<p>Retorno local de teste</p>'}));
  await page.goto(site+'/conta/?'+new URLSearchParams({desktop:'1',port:'19834',state,code_challenge:challenge}));
  await page.getByRole('button',{name:'Continuar com Google para o aplicativo'}).click();
  const next=page.getByRole('button',{name:'Continuar com Google',exact:true});
  await expect(next).toBeDisabled();
  await page.getByRole('checkbox',{name:/18 anos/}).check();
  await page.getByRole('checkbox',{name:/Li e aceito/}).check();
  await completeCaptcha(page);
  await Promise.all([page.waitForURL('**/auth/v1/authorize?**'),next.click()]);
  expect(new URL(page.url()).searchParams.get('code_challenge')).toBe(challenge);
  await Promise.all([page.waitForURL(loopback+'?*'),page.getByRole('link',{name:'Entrar na conta de teste'}).click()]);
  expect(new URL(page.url()).searchParams.get('state')).toBe(state);
  expect(new URL(page.url()).searchParams.get('code')).toMatch(/^[\da-f-]{36}$/);
});

test('atualizar depois do link de recuperação preserva o formulário até salvar a senha',async({page})=>{
  const fixture=await siteFixture(page);
  await page.goto(site+'/conta/?intent=recuperar');
  await page.getByLabel('E-mail da conta').fill('teste@example.invalid');
  await completeCaptcha(page);
  await page.getByRole('button',{name:'Enviar recuperação'}).click();
  await expect(page.getByRole('status')).toContainText('receberá instruções');
  await page.goto(site+'/conta/retorno/?code='+fixture.recovery());
  await expect(page.getByRole('heading',{name:'Redefina sua senha'})).toBeVisible();
  await expect(page).toHaveURL(site+'/conta/?intent=nova-senha');
  await page.reload();
  await page.getByLabel('Nova senha').fill('nova-senha-apenas-no-teste');
  await page.getByRole('button',{name:'Salvar nova senha'}).click();
  await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  await expect(page).toHaveURL(site+'/conta/');
});

test('link de e-mail expirado permite pedir outro e remove o erro da URL',async({page})=>{
  await siteFixture(page);
  await page.goto(site+'/conta/retorno/?error=access_denied&error_code=otp_expired');
  await expect(page.getByRole('heading',{name:'Este link não pôde ser usado'})).toBeVisible();
  expect(new URL(page.url()).search).toBe('');
  await page.getByRole('button',{name:'Solicitar nova recuperação'}).click();
  await expect(page.getByLabel('E-mail da conta')).toBeVisible();
});

async function passwordLogin(page:Page,path='/conta/'){
  await page.goto(site+path);
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('fixture-local-somente');
  await completeCaptcha(page);
  await page.getByRole('button',{name:'Entrar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Minha conta',exact:true})).toBeVisible();
}

test('conta organiza tarefas, preserva rascunhos e restaura a seção sem consultar chamados fora dela',async({page})=>{
  const fixture=await siteFixture(page,true);
  await passwordLogin(page);
  await expect(page.getByRole('link',{name:'Visão geral',exact:true})).toHaveAttribute('aria-current','page');
  await expect(page.getByText('Gratuito',{exact:true}).first()).toBeVisible();
  expect(fixture.posted.filter(p=>p.action==='support_list')).toHaveLength(0);
  await page.getByRole('link',{name:'Chamados',exact:true}).click();
  await page.getByLabel('Assunto',{exact:true}).fill('Rascunho preservado');
  await page.getByLabel('Mensagem sem dados confidenciais').fill('Ainda estou reunindo os passos do problema.');
  await page.getByRole('link',{name:'Plano',exact:true}).click();
  await page.goBack();
  await expect(page.getByLabel('Assunto',{exact:true})).toHaveValue('Rascunho preservado');
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  await page.reload();
  await expect(page.getByRole('heading',{name:'Escolhas que são suas.'})).toBeVisible();
  await expect(page.getByRole('link',{name:'Preferências',exact:true})).toHaveAttribute('aria-current','page');
  expect(fixture.posted.filter(p=>p.action==='support_list')).toHaveLength(1);
});

test('chamado mostra envio em andamento, evita duplicação e mantém texto após falha',async({page})=>{
  let release:(()=>void)|undefined,fail=true;
  const fixture=await siteFixture(page,true,{beforeAction:async body=>{if(body.action==='support')await new Promise<void>(resolve=>{release=resolve;});},failAction:()=>fail?'support':undefined});
  await passwordLogin(page,'/conta/?acao=suporte');
  await page.getByLabel('Assunto',{exact:true}).fill('Erro ao salvar um projeto');
  await page.getByLabel('Mensagem sem dados confidenciais').fill('Criei um projeto e cliquei em salvar. O formulário mostrou um erro.');
  const send=page.locator('#support button');
  await send.click();
  await expect(send).toBeDisabled();
  await expect(send).toHaveText('Aguarde…');
  await page.locator('#support').evaluate(form=>(form as HTMLFormElement).requestSubmit());
  await expect.poll(()=>fixture.posted.filter(p=>p.action==='support').length).toBe(1);
  release?.();
  await expect(page.locator('[data-feedback=support]')).toHaveAttribute('role','alert');
  await expect(page.getByLabel('Assunto',{exact:true})).toHaveValue('Erro ao salvar um projeto');
  fail=false;
  await send.click();
  await expect.poll(()=>fixture.posted.filter(p=>p.action==='support').length).toBe(2);release?.();
  await expect(page.locator('[data-feedback=support]')).toContainText('Chamado registrado.');
  await expect(page.getByLabel('Assunto',{exact:true})).toHaveValue('');
  await expect(page.locator('#tickets')).toContainText('Erro ao salvar um projeto');
});

test('uso mostra dados indisponíveis sem NaN e permite recuperar a consulta',async({page})=>{
  let failing=true;
  await siteFixture(page,true,{failAction:()=>failing?'account':undefined,account:()=>({plan:{id:'plus'},subscription:{periodStart:'invalid',periodEnd:'invalid'},usage:{windowEnd:'invalid',nextRelease:'invalid',windowUsedMicros:'not-a-number',monthlyUsedMicros:null},consent:{marketing:false}})});
  await passwordLogin(page);
  await expect(page.locator('[data-account-summary]')).toContainText('Não foi possível consultar os dados agora.');
  failing=false;
  await page.locator('[data-account-summary]').getByRole('button',{name:'Tentar novamente'}).click();
  await page.getByRole('link',{name:'Uso e limites',exact:true}).click();
  await expect(page.locator('[data-usage-detail]')).toContainText('Não disponível');
  await expect(page.locator('[data-usage-detail]')).not.toContainText(/NaN|Invalid Date/);
  await expect(page.locator('[data-usage-detail] progress:visible')).toHaveCount(0);
});

test('preferência de marketing salva e retirada permanece correta no reload',async({page})=>{
  const fixture=await siteFixture(page,true);
  await passwordLogin(page);
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  const marketing=page.getByRole('checkbox',{name:/receber novidades/});
  await expect(marketing).toBeChecked();
  await page.getByRole('button',{name:'Retirar consentimento de marketing'}).click();
  await expect(marketing).not.toBeChecked();
  await expect(page.locator('[data-feedback=unsubscribe]')).toContainText('retirado');
  await page.reload();
  await expect(marketing).not.toBeChecked();
  await marketing.check();
  await page.getByRole('button',{name:'Salvar preferência'}).click();
  await expect(page.locator('[data-feedback=marketing]')).toContainText('salva');
  await page.reload();
  await expect(marketing).toBeChecked();
  expect(fixture.posted.filter(p=>p.action==='marketing')).toEqual([{action:'marketing',enabled:false},{action:'marketing',enabled:true}]);
});

for(const width of [390,768,1440])test('conta navegável sem corte horizontal em '+width+'px',async({page},testInfo)=>{
  await page.setViewportSize({width,height:900});
  await siteFixture(page,true);
  await passwordLogin(page);
  await expect(page.getByText('Gratuito',{exact:true}).first()).toBeVisible();
  const nav=page.getByRole('navigation',{name:'Seções da conta'});
  await expect(nav).toBeVisible();
  for(const name of ['Plano','Uso e limites','Chamados','Preferências','Dados e acesso']){
    await nav.getByRole('link',{name,exact:true}).click();
    await expect(nav.getByRole('link',{name,exact:true})).toHaveAttribute('aria-current','page');
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  }
  await nav.getByRole('link',{name:'Visão geral',exact:true}).click();
  await page.getByRole('button',{name:'Entendi',exact:true}).click();
  for(const appearance of ['light','dark']){
    await page.getByLabel('Aparência',{exact:true}).selectOption(appearance);
    await nav.getByRole('link',{name:'Visão geral',exact:true}).click();
    await page.screenshot({path:testInfo.outputPath('account-overview-'+appearance+'-'+width+'.png'),fullPage:true});
    await nav.getByRole('link',{name:'Chamados',exact:true}).click();
    await page.screenshot({path:testInfo.outputPath('account-support-'+appearance+'-'+width+'.png'),fullPage:true});
  }
});

test('conta mantém contraste escuro e atualiza a cota ao restaurar a mesma sessão',async({page},testInfo)=>{
  let used=2000000;
  await siteFixture(page,true,{account:()=>({plan:{id:'plus'},subscription:{periodStart:'2026-11-01T00:00:00Z',periodEnd:'2026-12-01T00:00:00Z'},usage:{windowStart:'2026-11-08T00:00:00Z',windowEnd:'2026-11-15T00:00:00Z',nextRelease:'2026-11-15T00:00:00Z',windowUsedMicros:used,windowLimitMicros:5000000,monthlyUsedMicros:used,monthlyLimitMicros:20000000,reservedMicros:0,settledMicros:used},consent:{marketing:false}})});
  await passwordLogin(page);
  await page.getByLabel('Aparência',{exact:true}).selectOption('dark');
  await page.getByRole('button',{name:'Entendi',exact:true}).click();
  await expect(page.locator('.account-summary')).toHaveCSS('color','rgb(242, 245, 246)');
  await expect(page.locator('body')).toHaveCSS('background-color','rgb(17, 21, 25)');
  await page.screenshot({path:testInfo.outputPath('account-dark-1440.png'),fullPage:true});
  await page.getByRole('link',{name:'Uso e limites',exact:true}).click();
  const week=page.getByRole('progressbar',{name:'Consumo da janela de sete dias'});
  await expect(week).toHaveAttribute('value','2000000');
  used=3000000;
  await page.evaluate(()=>window.dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true})));
  await expect(week).toHaveAttribute('value','3000000');
  await page.screenshot({path:testInfo.outputPath('account-usage-dark-1440.png'),fullPage:true});
  await page.setViewportSize({width:390,height:900});
  await page.screenshot({path:testInfo.outputPath('account-usage-dark-390.png'),fullPage:true});
  await page.getByRole('link',{name:'Dados e acesso',exact:true}).click();
  await page.getByRole('button',{name:'Ver opções de exclusão'}).click();
  await expect(page.locator('#delete button')).toHaveCSS('color','rgb(255, 255, 255)');
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  await expect(page.getByLabel('Aparência',{exact:true})).toHaveValue('dark');
});

test('falha ao consultar aceites não reutiliza marketing armazenado',async({page})=>{
  let failRefresh=false;
  const fixture=await siteFixture(page,true,{failAction:()=>failRefresh?'account':undefined,account:()=>({consent:{acceptedAt:version,termsVersion:'versao-anterior',privacyVersion:'versao-anterior',requiredTermsVersion:version,requiredPrivacyVersion:version,marketing:true}})});
  await passwordLogin(page);
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  await page.getByRole('checkbox',{name:/18 anos/}).check();
  await page.getByRole('checkbox',{name:/Li e aceito/}).check();
  failRefresh=true;
  await page.getByRole('button',{name:'Salvar aceites'}).click();
  await expect(page.locator('[data-feedback=consent]')).toHaveAttribute('role','alert');
  expect(fixture.posted.filter(p=>p.action==='consent')).toHaveLength(0);
});

test('aceites consultam retirada de marketing em outro dispositivo e nunca reenviam opt-in',async({page})=>{
  let remoteMarketing=true;
  const fixture=await siteFixture(page,true,{account:()=>({consent:{acceptedAt:version,termsVersion:'versao-anterior',privacyVersion:'versao-anterior',requiredTermsVersion:version,requiredPrivacyVersion:version,marketing:remoteMarketing}})});
  await passwordLogin(page);
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  await expect(page.locator('#marketing [name=marketing]')).toBeChecked();
  await expect(page.locator('#consent [name=marketing]')).not.toBeChecked();
  remoteMarketing=false; // A withdrawal on another device after this page loaded.
  await page.getByRole('checkbox',{name:/18 anos/}).check();
  await page.getByRole('checkbox',{name:/Li e aceito/}).check();
  await page.getByRole('button',{name:'Salvar aceites'}).click();
  await expect(page.locator('[data-feedback=consent]')).toContainText('Aceites registrados.');
  expect(fixture.posted.filter(p=>p.action==='consent')).toEqual([{action:'consent',termsVersion:version,privacyVersion:version,marketing:false}]);
});

test('novos aceites só autorizam marketing quando a caixa opcional é marcada explicitamente',async({page})=>{
  const fixture=await siteFixture(page,false);
  await passwordLogin(page);
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  const optional=page.locator('#consent [name=marketing]');
  await expect(optional).not.toBeChecked();
  await page.getByRole('checkbox',{name:/18 anos/}).check();
  await page.getByRole('checkbox',{name:/Li e aceito/}).check();
  await optional.check();
  await page.getByRole('button',{name:'Salvar aceites'}).click();
  await expect(page.locator('[data-feedback=consent]')).toContainText('Aceites registrados.');
  expect(fixture.posted.filter(p=>p.action==='consent')).toEqual([{action:'consent',termsVersion:version,privacyVersion:version,marketing:true}]);
  await expect(page.locator('#consent')).toBeHidden();
  await expect(page.locator('[data-feedback=consent]')).toBeVisible();
});

test('aceites já atuais mostram o registro e ocultam o formulário de nova autorização',async({page})=>{
  const fixture=await siteFixture(page,true);
  await passwordLogin(page);
  await page.getByRole('link',{name:'Preferências',exact:true}).click();
  await expect(page.locator('#consent-state')).toContainText('Aceites atuais registrados');
  await expect(page.locator('#consent')).toBeHidden();
  await expect(page.locator('#marketing [name=marketing]')).toBeChecked();
  expect(fixture.posted.filter(p=>['consent','marketing'].includes(p.action))).toHaveLength(0);
});
