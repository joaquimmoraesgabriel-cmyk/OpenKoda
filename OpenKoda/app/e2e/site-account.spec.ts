import { expect, test } from './fixtures';
import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {resolve, sep} from 'node:path';
import type {Page} from '@playwright/test';
import {sessionBridge} from '../../site/worker.mjs';

const site='https://koda.example.invalid';
const auth='https://ynjcyvnrfxsrijrwpaye.supabase.co';
const dist=fileURLToPath(new URL('../../site/dist/',import.meta.url));
const version='2026-11-13';

test.describe.configure({ mode: 'default' });
test.beforeAll(() => {
  const build = fileURLToPath(new URL('../../site/build.mjs', import.meta.url));
  execFileSync(process.execPath, [build]);
});

async function siteFixture(page:Page,accepted=false){
  const posted:{action:string;marketing?:boolean}[]=[];
  const registrations:{data:{adult_requested:boolean;privacy_requested:string}}[]=[];
  const user={id:'synthetic-user',email:'teste@example.invalid',email_confirmed_at:version};
  let access='synthetic.credential.onlyfortesting',refresh='synthetic-refresh-001',validSession=false,refreshes=0;
  const provider=async(raw:string,init:RequestInit)=>{
    const url=new URL(raw);expect(url.origin).toBe(auth);
    const body=init.body?JSON.parse(String(init.body)):{};
    if(url.pathname==='/auth/v1/signup'){registrations.push(body);return Response.json({});}
    if(url.pathname==='/auth/v1/token'){
      if(url.searchParams.get('grant_type')==='refresh_token'){
        if(body.refresh_token!==refresh||!validSession)return Response.json({},{status:400});
        refreshes++;refresh='synthetic-refresh-'+String(100+refreshes);access='synthetic.credential.rotated'+refreshes;
      }
      validSession=true;return Response.json({access_token:access,refresh_token:refresh,expires_in:3600,user});
    }
    if(url.pathname==='/auth/v1/user')return Response.json(validSession?user:{},{status:validSession?200:401});
    if(url.pathname==='/auth/v1/logout'){validSession=false;return new Response(null,{status:204});}
    expect(url.pathname).toBe('/functions/v1/openkoda');posted.push(body);
    if(body.action==='account')return Response.json({plan:null,consent:{acceptedAt:accepted?version:null,requiredTermsVersion:version,requiredPrivacyVersion:version,marketing:accepted}});
    if(body.action==='consent_initial')accepted=true;
    return Response.json(body.action==='support_list'?{tickets:[]}:{});
  };
  await page.context().route('**/*',async route=>{
    const request=route.request(),url=new URL(request.url());
    if(url.origin===site){
      if(url.pathname.startsWith('/conta/api/')){
        const headers=await request.allHeaders();
        const r=await sessionBridge(new Request(url,{method:request.method(),headers,body:request.postData()??undefined}),provider);
        const cookies=r.headers.getSetCookie();
        // Playwright route.fulfill exposes one string per header. Apply each
        // Worker Set-Cookie separately to this isolated browser context.
        for(const raw of cookies){
          const [pair]=raw.split(';'),index=pair.indexOf('=');
          const age=Number(raw.match(/Max-Age=(\d+)/)?.[1]);
          expect(raw).toContain('Path=/; Secure; HttpOnly; SameSite=Lax');
          await page.context().addCookies([{name:pair.slice(0,index),value:pair.slice(index+1),url:site+'/',secure:true,httpOnly:true,sameSite:'Lax',expires:age?Math.floor(Date.now()/1000)+age:1}]);
        }
        return route.fulfill({status:r.status,contentType:'application/json',body:await r.text()});
      }
      const filename=resolve(dist,'.'+url.pathname+(url.pathname.endsWith('/')?'index.html':''));
      if(!filename.startsWith(resolve(dist)+sep))throw Error('Caminho de fixture inválido.');
      const mime=filename.endsWith('.js')?'text/javascript':filename.endsWith('.css')?'text/css':filename.endsWith('.png')?'image/png':'text/html';
      return route.fulfill({contentType:mime,body:await readFile(filename)});
    }
    if(url.origin==='https://challenges.cloudflare.com')
      return route.fulfill({contentType:'text/javascript',body:'window.turnstile={render(el){const token=document.createElement("input");token.type="hidden";token.name="cf-turnstile-response";token.value="fixture";el.append(token);}}'});
    expect(url.origin).toBe(auth);expect(url.pathname).toBe('/auth/v1/authorize');
    return route.fulfill({contentType:'text/html',body:'<title>Google de teste</title>'});
  });
  return {posted,registrations,refreshes:()=>refreshes};
}

test('site exige os aceites no Google e preserva o marketing depois do primeiro login',async({page})=>{
  const {posted}=await siteFixture(page);
  for(const first of [true,false]){
    if(!first)await page.getByRole('button',{name:'Sair',exact:true}).click();
    await page.goto(site+(first?'/conta/retorno/?error=access_denied&error_description=valor_privado':'/conta/'));
    if(first){
      await expect(page.getByRole('heading',{name:'O login não foi concluído'})).toBeVisible();
      expect(new URL(page.url()).search).toBe('');
      await expect(page.locator('body')).not.toContainText('valor_privado');
    }
    await page.getByRole('button',{name:first?'Tentar com Google novamente':'Entrar com Google'}).click();
    const next=page.getByRole('button',{name:'Continuar com Google'});
    await next.click();
    await expect(page.getByRole('heading',{name:'Continue com Google'})).toBeVisible();
    await page.getByRole('checkbox',{name:/18 anos/}).check();
    await page.getByRole('checkbox',{name:/Li e aceito/}).check();
    await Promise.all([page.waitForURL('**/auth/v1/authorize?**'),next.click()]);
    const destination=new URL(page.url());
    expect(destination.searchParams.get('code_challenge_method')).toBe('s256');
    if(first)expect(destination.searchParams.get('prompt')).toBe('select_account');
    await page.goto(site+'/conta/retorno/?code=fixture-one-use');
    await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
    expect(posted.filter(p=>p.action==='consent_initial')).toEqual([{action:'consent_initial',termsVersion:version,privacyVersion:version,marketing:false}]);
  }
});

test('site associa o marketing opcional do cadastro por e-mail à primeira sessão confirmada',async({page})=>{
  const {posted,registrations}=await siteFixture(page);
  await page.goto(site+'/conta/?intent=cadastro');
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha de pelo menos 12 caracteres').fill('fixture-local-somente');
  await page.getByRole('checkbox',{name:/18 anos/}).check();
  await page.getByRole('checkbox',{name:/Li e aceito/}).check();
  await page.getByRole('checkbox',{name:/receber novidades/}).check();
  await page.getByRole('button',{name:'Cadastrar'}).click();
  await expect(page.getByRole('heading',{name:'Entre no OpenKoda'})).toBeVisible();
  expect(registrations[0].data).toMatchObject({adult_requested:true,privacy_requested:version,terms_requested:version,marketing_requested:true});
  expect(posted.filter(p=>p.action==='consent_initial')).toHaveLength(0);
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('fixture-local-somente');
  await page.getByRole('button',{name:'Entrar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  expect(posted.filter(p=>p.action==='consent_initial')).toEqual([{action:'consent_initial',termsVersion:version,privacyVersion:version,marketing:true}]);
});
test('site mantém a sessão no reload, nos preços e numa nova aba; sair revoga ambas',async({page,context})=>{
  const f=await siteFixture(page,true);
  await page.goto(site+'/conta/');
  await page.getByLabel('E-mail',{exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('fixture-local-somente');
  await page.getByRole('button',{name:'Entrar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Minha conta'})).toBeVisible();
  expect(await page.evaluate(()=>document.cookie)).not.toContain('koda-');
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
