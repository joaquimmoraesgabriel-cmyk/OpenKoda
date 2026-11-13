import {expect,test} from './fixtures';
import {emptySnapshot} from '../src/preview';
import type {Page} from '@playwright/test';
import {installScopeFixture} from './scope-fixture';

async function desktop(page:Page,subscribed=false){
  await installScopeFixture(page);
  const snapshot=emptySnapshot(),at='2026-11-13T08:00:00Z';
  if(subscribed)Object.assign(snapshot.settings,{provider:'openkoda',model:'fixture/vision',endpoint:'https://ynjcyvnrfxsrijrwpaye.supabase.co/functions/v1/openkoda'});
  snapshot.projects=[{id:'fixture',kind:'code',name:'Fonte sintética',target:'D:/Fixture',description:'',createdAt:at}];
  snapshot.assessments=[{id:'assessment',projectId:'fixture',kind:'code',title:'Chat sintético',target:'D:/Fixture',
    objective:'Revisar a fixture',createdAt:at,updatedAt:at,status:'draft',coverage:[],evidence:[],events:[],summary:'',
    aiSelection:subscribed?{provider:'openkoda',model:'fixture/vision',reasoning:'medium'}:{provider:'openai',model:'fixture/byok'}}];
  await page.addInitScript(({snapshot,subscribed})=>{
    let signed=subscribed;
    const scope=window.__scopeFixture(snapshot,['ai.remote-review.v1']);
    const tickets:{id:string;subject:unknown;message:unknown;replies:unknown[]}[]=[];
    const calls:{action:string;payload:Record<string,unknown>}[]=[];
    Object.assign(window,{isTauri:true,commercialCalls:calls,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      const {action,payload}=args.request;calls.push({action,payload});
      switch(action){
        case 'snapshot':return snapshot;
        case 'assessment.scopeStatus':return scope.status(payload.assessmentId);
        case 'ai.context':return {bytes:100,digest:payload.disableAutoSkill?'plain-packet':'skill-packet',
          context:{message:payload.message},...(!payload.disableAutoSkill?{autoSkill:'code-review'}:{})};
        case 'ai.budget.status':return {enabled:false,degraded:false};
        case 'ai.history':return {messages:[],requests:[]};
        case 'ai.chat.status':return {requestId:null,stopping:false};
        case 'security.scan.get':return {scan:null};
        case 'security.status':return {available:false};
        case 'ai.budget.history':return {rows:[]};
        case 'account.session':return {user:signed?{id:'fixture-user',email:'teste@example.invalid'}:null};
        case 'account.signIn':signed=true;return {user:{id:'fixture-user',email:'teste@example.invalid'}};
        case 'account.signOut':signed=false;return {revoked:true};
        case 'account.overview':return {plan:subscribed?{id:'plus'}:null,consent:{},controls:{mockCheckout:true},
          plans:[{id:'plus',priceBrlCents:21900,monthlyLimitMicros:20000000}]};
        case 'account.checkout':return {status:'simulated',grantsProductionCredit:false};
        case 'account.support':tickets.push({id:'synthetic',subject:payload.subject,message:payload.message,replies:[]});return {id:'synthetic'};
        case 'account.supportList':return {tickets};
        case 'account.skills':return {skills:[{id:'code-review',name:'Revisar código',description:'Avaliar os trechos revisados.',
          revision:'fixture',invocation:'explicit'}]};
        case 'account.models':return {models:[{id:'fixture/vision',reasoningLevels:['low','medium','high'],image:true}]};
        case 'ai.selection.save':snapshot.assessments[0].aiSelection=payload.aiSelection as typeof snapshot.assessments[0]['aiSelection'];return {};
        default:return {};
      }
    }}});
  },{snapshot,subscribed});
  await page.goto('/',{waitUntil:'commit'});
}
test('cadastro protegido abre o site e a conta pode entrar e sair sem alterar o provedor pessoal',async({page})=>{
  await desktop(page);
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await page.getByRole('button',{name:'Criar conta no site'}).click();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'commercialCalls').some((c:{action:string;payload:{page?:string}})=>c.action==='account.site'&&c.payload.page==='signup'))).toBe(true);
  await page.getByRole('textbox',{name:'E-mail',exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('synthetic-password-only');
  await page.getByRole('button',{name:'Entrar com e-mail'}).click();
  await expect(page.getByText('teste@example.invalid · Sem assinatura')).toBeVisible();
  await page.getByText('Simular assinatura',{exact:true}).click();
  await page.getByRole('button',{name:/PLUS.*219/}).click();
  await expect(page.getByText('Simulação registrada.')).toBeVisible();
  await expect(page.getByText('teste@example.invalid · Sem assinatura')).toBeVisible();
  await page.getByText('Chamados de suporte e erros',{exact:true}).click();
  await page.getByLabel('Assunto',{exact:true}).fill('Falha sintética');
  await page.getByLabel('Descrição do chamado').fill('Teste do fluxo de suporte.');
  await page.getByRole('button',{name:'Enviar chamado'}).click();
  await expect(page.getByText('Falha sintética',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Sair da conta'}).click();
  await expect(page.getByRole('button',{name:'Continuar com Google'})).toBeVisible();
});
test('o mesmo controle de raciocínio funciona na criação do projeto',async({page})=>{
  await desktop(page,true);
  await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await page.getByRole('button',{name:'fixture/vision',exact:true}).click();
  const slider=page.getByRole('slider',{name:'Nível de raciocínio'});
  await expect(slider).toHaveAttribute('aria-valuetext','Leve');
  await slider.focus();await slider.press('ArrowRight');
  await expect(slider).toHaveAttribute('aria-valuetext','Médio');
});
test('assinatura usa níveis reais e seleciona skills por barra ou menu',async({page})=>{
  await desktop(page,true);
  await page.getByRole('button',{name:'Chat sintético',exact:true}).click();
  const slider=page.getByRole('slider',{name:'Nível de raciocínio'}),input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await expect(slider).toHaveAttribute('aria-valuetext','Médio');
  await slider.focus();await slider.press('End');
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'commercialCalls').some((c:{action:string;payload:{aiSelection?:{reasoning?:string}}})=>c.action==='ai.selection.save'&&c.payload.aiSelection?.reasoning==='high'))).toBe(true);
  await input.fill('/code');
  await page.getByRole('button',{name:'/code-review · Revisar código'}).click();
  await expect(page.getByRole('button',{name:'Remover skill'})).toBeVisible();
  await page.getByRole('button',{name:'Remover skill'}).click();
  await page.getByRole('button',{name:'Adicionar à mensagem'}).click();
  await page.getByRole('menuitem',{name:'Skills da assinatura'}).click();
  await expect(page.getByText('Avaliar os trechos revisados.')).toBeVisible();
});
test('skill automática exige revisão e pode ser removida antes de enviar a mesma mensagem',async({page})=>{
  await desktop(page,true);
  await page.evaluate(()=>{
    // The shared legacy policy fixture initializes a BYOK slot. This account
    // has an active subscription and an authorized commercial credential.
    const bridge=Reflect.get(window,'__TAURI_INTERNALS__');
    const invoke=bridge.invoke.bind(bridge);
    bridge.invoke=async(command:string,args:{request:{action:string}})=>{
      const result=await invoke(command,args);
      if(args.request?.action==='credential.reference.list')return {...result,references:[{
        ...result.references[0],providerSlotRef:{provider:'openkoda',slotId:'paid-fixture'},
      }]};
      return result;
    };
    localStorage.setItem('openkoda.chat.approval.assessment','scope');
  });
  await page.getByRole('button',{name:'Chat sintético',exact:true}).click();
  const message='Revise o código desta função';
  await page.getByRole('textbox',{name:'Mensagem da conversa'}).fill(message);
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect(page.getByText('Skill sugerida para esta mensagem:',{exact:false})).toBeVisible();
  await page.getByRole('button',{name:'Revisar sem a skill'}).click();
  await expect(page.getByRole('button',{name:'Revisar sem a skill'})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Autorizar envio'})).toBeVisible();
  const calls=()=>page.evaluate(()=>Reflect.get(window,'commercialCalls') as {action:string;payload:{message?:string;contextDigest?:string}}[]);
  expect((await calls()).filter(c=>c.action==='ai.ask')).toHaveLength(0);
  await page.getByRole('button',{name:'Autorizar envio'}).click();
  await expect.poll(async()=>(await calls()).filter(c=>c.action==='ai.ask').map(c=>[c.payload.message,c.payload.contextDigest])).toEqual([[message,'plain-packet']]);
});
