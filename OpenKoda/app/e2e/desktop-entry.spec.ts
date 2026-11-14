import { test, expect, type Page } from '@playwright/test';
import { emptySnapshot } from '../src/preview';

// Renderer contract tests only. Native OAuth, quota enforcement and installation
// preservation are tested separately against their real implementations.
async function fixture(page:Page,initial='signedOut',allowSimulation=false,realSubscription=false){
  await page.addInitScript(({snapshot,initial,allowSimulation,realSubscription})=>{
    const w=window as any;
    let state=localStorage.getItem('entry-fixture-state')||initial;
    let simulation=JSON.parse(localStorage.getItem('entry-fixture-simulation')||'null');
    const calls:any[]=[],receipts=new Map();
    const user={id:'user-a',email:'conta-a@example.invalid',displayName:'Pessoa A'};
    const account=()=>({plan:realSubscription?{id:'plus'}:null,subscription:realSubscription?{status:'active'}:null,simulation,controls:{simulationEnabled:allowSimulation},entitlements:{reasoning:simulation?.status==='active',skills:simulation?.status==='active',hostedInference:false},consent:{termsVersion:'v1',privacyVersion:'v1',requiredTermsVersion:'v1',requiredPrivacyVersion:'v1',marketing:false},plans:[{id:'plus',priceBrlCents:21900,monthlyLimitMicros:20_000_000},{id:'pro',priceBrlCents:99900,monthlyLimitMicros:100_000_000},{id:'ultra',priceBrlCents:199920,monthlyLimitMicros:200_000_000}]});
    let finish:(value:unknown)=>void,rejectLogin:(cause:Error)=>void;
    Object.assign(w,{isTauri:true,entryCalls:calls,entrySetState:(next:string)=>{state=next;},entryFinishLogin:()=>{state='authenticated';localStorage.setItem('entry-fixture-state',state);finish({user});},__TAURI_INTERNALS__:{invoke:async(command:string,args:any)=>{
      if(command==='desktop_update')return {phase:'current',currentVersion:'1.1.0',version:null,notes:null,downloadedBytes:0,totalBytes:null,message:'Aplicativo atualizado.',deferred:false,backupPath:null};
      if(command!=='request')return null;
      const {action,payload}=args.request;calls.push({action,payload});
      switch(action){
        case 'snapshot':return snapshot;
        case 'account.status':
          if(state==='error')throw new Error('Falha sintética ao ler a sessão.');
          if(state==='hold')await new Promise<void>(resolve=>{w.entryRelease=()=>{state='signedOut';resolve();};});
          return {state,user:['authenticated','offline'].includes(state)?user:null,account:state==='authenticated'?account():null,message:state==='offline'?'Sem conexão. Continue com seus projetos locais.':state==='expired'?'Sua sessão expirou. Entre novamente.':undefined};
        case 'account.session':return {user:['authenticated','offline'].includes(state)?user:null};
        case 'account.overview':return account();
        case 'account.google':return new Promise((resolve,reject)=>{finish=resolve;rejectLogin=reject;});
        case 'account.loginCancel':rejectLogin?.(new Error('Entrada cancelada'));return {canceled:true};
        case 'account.signOut':state='signedOut';localStorage.removeItem('entry-fixture-state');return {revoked:true};
        case 'account.simulation':{
          if(!allowSimulation)throw new Error('Conta não autorizada para teste.');
          if(receipts.has(payload.requestId))return {...receipts.get(payload.requestId),replay:true};
          await new Promise(resolve=>setTimeout(resolve,80));
          const status=({activate:'active',cancel:'canceled',expire:'expired',fail:'failed'} as any)[payload.outcome];
          if(status!=='failed')simulation={plan:payload.plan,status,mode:'internal',periodStart:new Date().toISOString(),periodEnd:new Date(Date.now()+86400000).toISOString()};
          localStorage.setItem('entry-fixture-simulation',JSON.stringify(simulation));
          const result={status,simulation,grantsProductionCredit:false,replay:false};receipts.set(payload.requestId,result);return result;
        }
        case 'ai.budget.status':return {enabled:false,degraded:false};
        case 'ai.budget.history':return {rows:[]};
        case 'ai.status':return {provider:payload.provider,hasApiKey:false,enabledModels:[]};
        case 'account.models':return {models:[{id:'fixture/chat',reasoningLevels:[],chat:true,analysis:true}],entries:[
          {id:'fixture/chat',name:'Modelo de conversa',chat:true,analysis:true,selectable:true,availability:'eligible'},
          {id:'fixture/audio',name:'Modelo de áudio',chat:false,analysis:false,selectable:false,availability:'unsupported',reason:'A modalidade de áudio não é suportada no chat.'},
          {id:'fixture/policy',name:'Modelo fora da política',chat:true,analysis:true,selectable:false,availability:'policy',reason:'O custo excede o limite do serviço.'},
        ]};
        default:return {};
      }
    }}});
  },{snapshot:emptySnapshot(),initial,allowSimulation,realSubscription});
  await page.goto('/',{waitUntil:'commit'});
}

test('primeiro acesso aguarda a sessão e oferece entrada ou uso local explícito',async({page})=>{
  await fixture(page,'hold');
  await expect(page.getByRole('status')).toContainText('verificando a sessão');
  await expect(page.getByRole('navigation',{name:'Navegação principal'})).toHaveCount(0);
  await page.evaluate(()=>Reflect.get(window,'entryRelease')());
  await expect(page.getByRole('heading',{name:'Entre para continuar.'})).toBeVisible();
  await page.screenshot({path:'.local/desktop-welcome-reviewed.png',animations:'disabled'});
  await page.getByRole('button',{name:'Continuar com minha chave de API'}).click();
  await expect(page.getByRole('navigation',{name:'Navegação principal'})).toBeVisible();
  await page.reload();
  await page.evaluate(()=>Reflect.get(window,'entryRelease')?.());
  await expect(page.getByRole('navigation',{name:'Navegação principal'})).toBeVisible();
});

test('erro de inicialização permite repetir sem limpar dados',async({page})=>{
  await fixture(page,'error');
  await expect(page.getByRole('alert')).toHaveText('Falha sintética ao ler a sessão.');
  await expect(page.getByText('Seus projetos e conversas permanecem neste dispositivo.')).toBeVisible();
  await page.evaluate(()=>Reflect.get(window,'entrySetState')('signedOut'));
  await page.getByRole('button',{name:'Tentar novamente'}).click();
  await expect(page.getByRole('heading',{name:'Entre para continuar.'})).toBeVisible();
});

test('retorno do Google atualiza identidade e restaura sessão após recarregar',async({page})=>{
  await fixture(page);
  await page.getByRole('button',{name:'Continuar com Google'}).click();
  await expect(page.getByText('Conclua a entrada no navegador e clique em “Abrir aplicativo”.',{exact:false})).toBeVisible();
  await page.evaluate(()=>Reflect.get(window,'entryFinishLogin')());
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Pessoa A'})).toBeVisible();
  await expect(page.getByText('conta-a@example.invalid')).toBeVisible();
  await page.screenshot({path:'.local/desktop-profile-reviewed.png',animations:'disabled'});
  await page.reload();
  await expect(page.getByRole('navigation',{name:'Navegação principal'})).toBeVisible();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Conta e segurança',exact:true}).click();
  await page.getByRole('button',{name:'Sair da conta'}).click();
  await expect(page.getByRole('heading',{name:'Entre para continuar.'})).toBeVisible();
  await expect(page.getByText('conta-a@example.invalid')).toHaveCount(0);
});

test('cancelar o login permite iniciar outra tentativa',async({page})=>{
  await fixture(page);
  await page.getByRole('button',{name:'Continuar com Google'}).click();
  await page.getByRole('button',{name:'Cancelar entrada'}).click();
  await expect(page.getByText('Entrada cancelada. Você pode tentar novamente.')).toBeVisible();
  await expect(page.getByRole('button',{name:'Continuar com Google'})).toBeEnabled();
});

test('falha de rede preserva identidade sem presumir assinatura',async({page})=>{
  await fixture(page,'offline');
  await expect(page.getByText('Sem conexão. Continue com seus projetos locais.')).toBeVisible();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await expect(page.getByText('As permissões pagas não serão presumidas sem conexão.',{exact:false})).toBeVisible();
  await expect(page.getByRole('button',{name:'Simular Plus'})).toHaveCount(0);
});

test('simulação usa serviço uma vez, restaura plano e revoga acesso ao expirar',async({page})=>{
  await fixture(page,'authenticated',true);
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await page.getByRole('button',{name:'Simular Plus',exact:true}).dblclick();
  await expect(page.getByRole('heading',{name:'Plano Plus · simulação interna'})).toBeVisible();
  await page.screenshot({path:'.local/desktop-plans-reviewed.png',animations:'disabled'});
  const calls=await page.evaluate(()=>Reflect.get(window,'entryCalls').filter((call:any)=>call.action==='account.simulation'));
  expect(calls).toHaveLength(1);expect(calls[0].payload.requestId).toMatch(/^[a-f0-9-]{36}$/);
  await page.reload();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Plano Plus · simulação interna'})).toBeVisible();
  await page.getByRole('button',{name:'Simular expiração'}).click();
  await expect(page.getByRole('heading',{name:'Uso com chave própria'})).toBeVisible();
  await expect(page.getByRole('button',{name:'Simular Plus'})).toBeEnabled();
});

test('conta comum não recebe controles de simulação',async({page})=>{
  await fixture(page,'authenticated');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await expect(page.getByText('Contratação ainda não disponível')).toHaveCount(3);
  await expect(page.getByRole('button',{name:/Simular/})).toHaveCount(0);
});

test('assinatura real permanece principal quando há uma simulação de outro plano',async({page})=>{
  await fixture(page,'authenticated',true,true);
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await page.getByRole('button',{name:'Simular Ultra',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Plano Plus',exact:true})).toBeVisible();
  await expect(page.locator('.account-plan-current .account-state')).toHaveText('Ativo');
  await expect(page.locator('.simulation-tools')).toContainText('Plano Ultra · simulação interna ativa');
  await expect(page.locator('.desktop-plan[data-selected="true"] h4')).toHaveText('Plus');
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await expect(page.getByText('Plano Plus',{exact:true})).toBeVisible();
});

test('catálogo distingue compatibilidade, modalidade e política sem conceder acesso',async({page})=>{
  await fixture(page,'authenticated');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'IA e modelos',exact:true}).click();
  const catalog=page.getByRole('region',{name:'Modelos da assinatura'});
  await expect(catalog.getByText('Requer assinatura',{exact:true})).toBeVisible();
  await expect(catalog.getByText('A modalidade de áudio não é suportada no chat.')).toBeVisible();
  await expect(catalog.getByText('O custo excede o limite do serviço.')).toBeVisible();
  await page.getByRole('searchbox',{name:'Pesquisar catálogo da assinatura'}).fill('áudio');
  await expect(catalog.getByRole('row')).toHaveCount(2);
  await expect(catalog.getByRole('button')).toHaveCount(0);
  await page.getByRole('button',{name:'Início',exact:true}).click();
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const picker=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await expect(picker.getByText('Os modelos da assinatura exigem acesso ativo e cota disponível.',{exact:false})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'entryCalls').filter((call:{action:string})=>call.action==='account.models').length)).toBeGreaterThan(1);
  await expect(picker.getByRole('button',{name:'fixture/chat',exact:true})).toHaveCount(0);
});

test('configurações se adaptam à janela menor e respeitam movimento reduzido',async({page})=>{
  await page.setViewportSize({width:900,height:640});await page.emulateMedia({reducedMotion:'reduce'});
  await fixture(page,'authenticated',true);
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await expect(page.getByRole('button',{name:'Simular Plus',exact:true})).toBeVisible();
  await expect(page.locator('.account-plans')).toHaveCSS('animation-name','none');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  const target=page.getByRole('button',{name:'Simular Plus',exact:true});await target.focus();await target.press('Enter');
  await expect(page.getByRole('heading',{name:'Plano Plus · simulação interna'})).toBeVisible();
});
