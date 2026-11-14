import {test,expect} from './fixtures';
import {emptySnapshot} from '../src/preview';
import type {Page} from '@playwright/test';

type Mode='signedOut'|'free'|'simulation'|'paid'|'disabled'|'offline';
async function fixture(page:Page,mode:Mode,empty=false){
  const snapshot=emptySnapshot(),at='2026-11-14T08:00:00Z';
  Object.assign(snapshot.settings,{provider:'openrouter',model:'fixture/byok',endpoint:'https://openrouter.ai/api/v1'});
  if(!empty){
    snapshot.projects=[{id:'source',kind:'code',name:'Projeto preservado',target:'D:/Fixture',description:'',createdAt:at}];
    snapshot.assessments=[{id:'chat',projectId:'source',kind:'code',title:'Conversa preservada',target:'D:/Fixture',objective:'Revisar',createdAt:at,updatedAt:at,status:'draft',coverage:[],evidence:[],events:[],summary:'',aiSelection:{provider:'openrouter',model:'fixture/byok'}}];
  }
  await page.addInitScript(({snapshot,mode})=>{
    const calls:any[]=[];
    const user=mode==='signedOut'?null:{id:'account-fixture',email:'fixture@example.invalid'};
    const paid=mode==='paid'||mode==='disabled';
    const account={plan:paid?{id:'plus'}:null,subscription:paid?{status:'active'}:null,
      entitlements:{reasoning:paid||mode==='simulation',skills:paid||mode==='simulation',hostedInference:paid},controls:{inferenceEnabled:mode!=='disabled',simulationEnabled:mode==='simulation'},
      simulation:mode==='simulation'?{plan:'plus',status:'active',mode:'internal',periodStart:'2026-11-14T00:00:00Z',periodEnd:'2099-01-01T00:00:00Z'}:null,
      plans:[{id:'plus',priceBrlCents:21900,monthlyLimitMicros:20_000_000}]};
    Object.assign(window,{isTauri:true,kodaCalls:calls,__TAURI_INTERNALS__:{invoke:async(command:string,args:any)=>{
      if(command==='desktop_update')return {phase:'current',currentVersion:'1.1.1'};
      if(command!=='request')return null;
      const {action,payload}=args.request;calls.push({action,payload});
      if(payload?.provider==='openkoda'&&/^(ai\.status|ai\.models|ai\.key\.|ai\.provider\.configure|ai\.test)/.test(action))throw new Error('Koda cannot use personal API-key operations');
      switch(action){
        case 'snapshot':return snapshot;
        case 'account.status':return {state:mode==='signedOut'?'signedOut':mode==='offline'?'offline':'authenticated',user,account:mode==='signedOut'||mode==='offline'?null:account};
        case 'account.session':return {user};
        case 'account.models':return {models:[{id:'fixture/koda',name:'Modelo Koda disponível',analysis:true,chat:true,reasoningLevels:['low','high']},{id:'fixture/chat-only',analysis:false,chat:true,reasoningLevels:[]}],entries:[{id:'fixture/koda',name:'Modelo Koda disponível',analysis:true,chat:true,selectable:true,availability:'eligible'}]};
        case 'ai.status':return {provider:payload.provider,endpoint:'https://openrouter.ai/api/v1',hasApiKey:payload.provider==='openrouter',enabledModels:payload.provider==='openrouter'?['fixture/byok']:[],verified:true};
        case 'ai.models':return payload.reasoningModel?{reasoningLevels:[],subscribed:paid}:{provider:payload.provider,models:['fixture/byok'],credentialRevision:'fixture'};
        case 'ai.selection.save':snapshot.assessments[0].aiSelection=payload.aiSelection;return {};
        case 'ai.history':return {messages:[],requests:[]};
        case 'ai.chat.status':return {requestId:null,stopping:false};
        case 'ai.budget.history':return {rows:[]};
        case 'ai.budget.status':return {enabled:false,degraded:false,unknownCharges:[],recentCharges:[],policy:null};
        case 'security.scan.get':return {scan:null};
        case 'security.status':return {available:false};
        default:return {};
      }
    }}});
  },{snapshot,mode});
  await page.goto('/');
}
async function calls(page:Page){return page.evaluate(()=>Reflect.get(window,'kodaCalls') as {action:string;payload:Record<string,unknown>}[]);}
async function noPersonalKoda(page:Page){expect((await calls(page)).filter(call=>call.payload?.provider==='openkoda'&&/^(ai\.status|ai\.models|ai\.key\.|ai\.provider\.configure|ai\.test)/.test(call.action))).toEqual([]);}

for(const [mode,label] of [['signedOut','Entre na sua conta'],['free','Disponível com assinatura'],['simulation','Simulação interna ativa'],['paid','Disponível pelo plano'],['disabled','Serviço indisponível'],['offline','Conta não verificada']] as const){
  test(`Koda permanece visível e separa o estado ${mode} de uma chave pessoal`,async({page})=>{
    await fixture(page,mode);
    await page.getByRole('button',{name:'Configurações',exact:true}).click();
    await page.getByRole('button',{name:'IA e modelos',exact:true}).click();
    const rail=page.getByRole('complementary',{name:'Provedores configurados'});
    await expect(rail.getByRole('button',{name:new RegExp('^Koda '+label)})).toBeVisible();
    await rail.getByRole('button',{name:'Adicionar provedor'}).click();
    const dialog=page.getByRole('dialog',{name:'Adicionar provedor'});
    await dialog.getByRole('searchbox',{name:'Pesquisar provedores'}).fill('Koda');
    await dialog.getByRole('button',{name:/^Koda/}).click();
    const access=page.getByRole('region',{name:'Acesso à Koda'});
    await expect(access.getByText(label,{exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'Conectar com chave de API'})).toHaveCount(0);
    await expect(page.getByLabel('Chave de API',{exact:true})).toHaveCount(0);
    if(mode==='simulation')await expect(access).toContainText('não concede créditos de IA da Koda');
    if(mode==='signedOut'||mode==='offline')expect((await calls(page)).filter(call=>call.action==='account.models')).toEqual([]);
    else await expect(page.getByRole('region',{name:'Modelos da assinatura'})).toBeVisible();
    if(mode==='free'||mode==='simulation')await page.screenshot({path:`.local/koda-${mode}-settings.png`,animations:'disabled'});
    await noPersonalKoda(page);
  });
}

test('Conhecer planos no seletor preserva o rascunho e os modelos gratuitos permanecem bloqueados',async({page})=>{
  await fixture(page,'free');
  const composer=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await composer.fill('Continuar exatamente deste ponto');
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const picker=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await picker.getByRole('button',{name:'Koda',exact:true}).click();
  await expect(picker.getByRole('button',{name:'fixture/koda',exact:true})).toBeDisabled();
  await page.screenshot({path:'.local/koda-free-picker.png',animations:'disabled'});
  await picker.getByRole('button',{name:'Conhecer planos',exact:true}).click();
  await expect(page.getByRole('region',{name:'Conteúdo de Plano e assinatura'})).toBeVisible();
  await page.getByRole('button',{name:'Início',exact:true}).click();
  await expect(composer).toHaveValue('Continuar exatamente deste ponto');
  expect((await calls(page)).filter(call=>call.action==='ai.selection.save')).toEqual([]);await noPersonalKoda(page);
});

test('Koda na criação fica visível sem sessão e a entrada abre a conta existente',async({page})=>{
  await fixture(page,'signedOut',true);
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByLabel('Pasta do projeto',{exact:true}).fill('D:/MeuRascunho');
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const picker=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await picker.getByRole('button',{name:'Koda',exact:true}).click();
  await picker.getByRole('button',{name:'Entrar na Koda'}).click();
  await expect(page.getByRole('region',{name:'Conteúdo de Conta e segurança'})).toBeVisible();
  await expect(page.getByRole('button',{name:'Continuar com Google'})).toBeVisible();
  await page.getByRole('button',{name:'Início',exact:true}).click();
  await expect(page.getByLabel('Pasta do projeto',{exact:true})).toHaveValue('D:/MeuRascunho');
  expect((await calls(page)).filter(call=>call.action==='account.models')).toEqual([]);await noPersonalKoda(page);
});

for(const mode of ['paid','simulation','disabled','offline'] as const)test(`seleção de modelo da Koda respeita acesso ${mode}`,async({page})=>{
  await fixture(page,mode);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const picker=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await picker.getByRole('button',{name:'Koda',exact:true}).click();
  const model=picker.getByRole('button',{name:'fixture/koda',exact:true});
  if(mode==='paid'){
    await expect(model).toBeEnabled();await model.click();
    await expect.poll(async()=>(await calls(page)).filter(call=>call.action==='ai.selection.save').at(-1)?.payload.aiSelection).toEqual({provider:'openkoda',model:'fixture/koda'});
  }else if(mode==='offline'){await expect(model).toHaveCount(0);await expect(picker.getByRole('button',{name:'Verificar conta'})).toBeVisible();}
  else await expect(model).toBeDisabled();
  if(mode==='simulation')await page.screenshot({path:'.local/koda-simulation-picker.png',animations:'disabled'});
  await noPersonalKoda(page);
});
