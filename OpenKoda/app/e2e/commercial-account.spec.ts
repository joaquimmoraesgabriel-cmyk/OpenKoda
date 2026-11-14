import {expect,test} from './fixtures';
import {emptySnapshot} from '../src/preview';
import type {Page} from '@playwright/test';
import {installScopeFixture} from './scope-fixture';

async function desktop(page:Page,subscribed=false,personal=false,onlyLow=false,extended=false){
  await installScopeFixture(page);
  const snapshot=emptySnapshot(),at='2026-11-13T08:00:00Z';
  if(subscribed)Object.assign(snapshot.settings,{provider:personal?'openrouter':'openkoda',model:'fixture/vision',endpoint:personal?'https://openrouter.ai/api/v1':'https://ynjcyvnrfxsrijrwpaye.supabase.co/functions/v1/openkoda'});
  snapshot.projects=[{id:'fixture',kind:'code',name:'Fonte sintética',target:'D:/Fixture',description:'',createdAt:at}];
  snapshot.assessments=[{id:'assessment',projectId:'fixture',kind:'code',title:'Chat sintético',target:'D:/Fixture',
    objective:'Revisar a fixture',createdAt:at,updatedAt:at,status:'draft',coverage:[],evidence:[],events:[],summary:'',
    aiSelection:subscribed?{provider:personal?'openrouter':'openkoda',model:'fixture/vision',reasoning:'medium'}:{provider:'openai',model:'fixture/byok'}}];
  await page.addInitScript(({snapshot,subscribed,personal,onlyLow,extended})=>{
    let signed=subscribed,simulation:any=null;
    const scope=window.__scopeFixture(snapshot,['ai.remote-review.v1']);
    const tickets:{id:string;subject:unknown;message:unknown;replies:unknown[]}[]=[];
    const calls:{action:string;payload:Record<string,unknown>}[]=[];
    Object.assign(window,{isTauri:true,commercialCalls:calls,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      const {action,payload}=args.request;calls.push({action,payload});
      switch(action){
        case 'snapshot':return snapshot;
        case 'assessment.scopeStatus':return scope.status(payload.assessmentId);
        case 'ai.context':return {bytes:100,digest:payload.disableAutoSkill?'plain-packet':'skill-packet',
          context:{message:payload.message},...(!payload.disableAutoSkill&&!personal?{autoSkill:'code-review'}:{})};
        case 'ai.budget.status':return {enabled:false,degraded:false};
        case 'ai.history':return {messages:[],requests:[]};
        case 'ai.chat.status':return {requestId:null,stopping:false};
        case 'security.scan.get':return {scan:null};
        case 'security.status':return {available:false};
        case 'ai.budget.history':return {rows:[]};
        case 'account.session':return {user:signed?{id:'fixture-user',email:'teste@example.invalid'}:null};
        case 'account.signIn':signed=true;return {user:{id:'fixture-user',email:'teste@example.invalid'}};
        case 'account.signOut':signed=false;return {revoked:true};
        case 'account.overview':return {plan:subscribed?{id:'plus'}:null,subscription:subscribed?{status:'active'}:null,simulation,entitlements:{reasoning:subscribed||simulation?.status==='active',skills:subscribed||simulation?.status==='active',hostedInference:subscribed},consent:{},controls:{simulationEnabled:true},
          plans:[{id:'plus',priceBrlCents:21900,monthlyLimitMicros:20000000}]};
        case 'account.simulation':simulation={plan:payload.plan,status:'active',mode:'internal',periodStart:new Date().toISOString(),periodEnd:new Date(Date.now()+86400000).toISOString()};return {status:'active',simulation,grantsProductionCredit:false,replay:false};
        case 'account.support':tickets.push({id:'synthetic',subject:payload.subject,message:payload.message,replies:[]});return {id:'synthetic'};
        case 'account.supportList':return {tickets};
        case 'account.skills':return {skills:[{id:'code-review',name:'Revisar código',description:'Avaliar os trechos revisados.',
          revision:'fixture',invocation:'explicit'}]};
        case 'account.models':return {models:[{id:'fixture/vision',reasoningLevels:['low','medium','high'],image:true}]};
        case 'ai.models':return payload.reasoningModel?{subscribed:signed&&subscribed,reasoningLevels:signed&&subscribed?(onlyLow?['low']:extended?['minimal','low','medium','high','xhigh','max']:['low','medium','high']):[]}:{models:['fixture/vision']};
        case 'ai.status':return {provider:payload.provider,hasApiKey:personal&&payload.provider==='openrouter',enabledModels:['fixture/vision']};
        case 'ai.selection.save':snapshot.assessments[0].aiSelection=payload.aiSelection as typeof snapshot.assessments[0]['aiSelection'];return {};
        default:return {};
      }
    }}});
  },{snapshot,subscribed,personal,onlyLow,extended});
  await page.goto('/',{waitUntil:'commit'});
}

test('assinante ajusta raciocínio com chave própria e volta ao automático sem mudar provedor',async({page})=>{
  await desktop(page,true,true);
  await expect(page.getByRole('group',{name:'Nível de raciocínio'})).toHaveCount(0);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await expect(page.getByRole('button',{name:'Médio',exact:true})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByText('O consumo é cobrado na sua chave.',{exact:false})).toBeVisible();
  await page.getByRole('button',{name:'Alto',exact:true}).click();
  await expect(page.getByRole('button',{name:'Alto',exact:true})).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button',{name:'Automático',exact:true}).click();
  await expect(page.getByRole('button',{name:'Automático',exact:true})).toHaveAttribute('aria-pressed','true');
  const selection=await page.evaluate(()=>Reflect.get(window,'commercialCalls').filter((c:{action:string})=>c.action==='ai.selection.save').at(-1)?.payload.aiSelection);
  expect(selection).toEqual({provider:'openrouter',model:'fixture/vision'});
});

test('conta gratuita não exibe barra de raciocínio de chave própria',async({page})=>{
  await desktop(page);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await expect(page.getByRole('group',{name:'Nível de raciocínio'})).toHaveCount(0);
  await expect(page.getByText('O ajuste depende de um plano ativo e de suporte nesta conexão.')).toBeVisible();
});

test('um modelo com esforço único permite sair de Automático e selecionar Leve',async({page})=>{
  await desktop(page,true,true,true);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await page.getByRole('button',{name:'Automático',exact:true}).click();
  await expect(page.getByRole('button',{name:'Automático',exact:true})).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button',{name:'Leve',exact:true}).click();
  await expect(page.getByRole('button',{name:'Leve',exact:true})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByRole('button',{name:'Alto',exact:true})).toHaveCount(0);
});

test('seletor preserva os níveis adicionais que a conexão realmente oferece',async({page})=>{
  await desktop(page,true,true,false,true);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  for(const [label,value] of [['Mínimo','minimal'],['Muito alto','xhigh'],['Máximo','max']]){
    await page.getByRole('button',{name:label,exact:true}).click();
    await expect(page.getByRole('button',{name:label,exact:true})).toHaveAttribute('aria-pressed','true');
    await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'commercialCalls').filter((c:{action:string})=>c.action==='ai.selection.save').at(-1)?.payload.aiSelection.reasoning)).toBe(value);
  }
});

test('expiração atualizada pelo foco remove níveis antigos no mesmo chat',async({page})=>{
  await desktop(page,true,true);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await expect(page.getByRole('button',{name:'Médio',exact:true})).toHaveAttribute('aria-pressed','true');
  await page.clock.install();
  await page.evaluate(()=>{
    const bridge=Reflect.get(window,'__TAURI_INTERNALS__'),invoke=bridge.invoke.bind(bridge);
    bridge.invoke=async(command:string,args:{request:{action:string}})=>{
      const result=await invoke(command,args);
      if(args.request?.action==='account.status')return {...result,account:{...result.account,plan:null,subscription:null,entitlements:{reasoning:false,skills:false,hostedInference:false}}};
      if(args.request?.action==='credential.reference.list')return {...result,references:[{...result.references[0],providerSlotRef:{provider:'openrouter',slotId:'personal-fixture'}}]};
      return result;
    };
  });
  await page.clock.fastForward(31_000);
  await page.evaluate(()=>window.dispatchEvent(new Event('focus')));
  await expect(page.getByRole('group',{name:'Nível de raciocínio'})).toHaveCount(0);
  await page.getByRole('button',{name:'Usar Automático',exact:true}).click();
  const selection=await page.evaluate(()=>Reflect.get(window,'commercialCalls').filter((c:{action:string})=>c.action==='ai.selection.save').at(-1)?.payload.aiSelection);
  expect(selection).toEqual({provider:'openrouter',model:'fixture/vision'});
  await page.getByRole('button',{name:'Fechar modelos'}).click();
  await page.getByRole('textbox',{name:'Mensagem da conversa'}).fill('Continuar no modo automático');
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'commercialCalls').filter((c:{action:string})=>c.action==='ai.ask').length)).toBe(1);
});
test('cadastro protegido abre o site e a conta pode entrar e sair sem alterar o provedor pessoal',async({page})=>{
  await desktop(page);
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await page.getByRole('button',{name:'Entrar na minha conta',exact:true}).click();
  await page.getByRole('button',{name:'Criar conta no site'}).click();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'commercialCalls').some((c:{action:string;payload:{page?:string}})=>c.action==='account.site'&&c.payload.page==='signup'))).toBe(true);
  await page.getByRole('button',{name:'Entrar com e-mail',exact:true}).click();
  await page.getByRole('textbox',{name:'E-mail',exact:true}).fill('teste@example.invalid');
  await page.getByLabel('Senha',{exact:true}).fill('synthetic-password-only');
  await page.getByRole('button',{name:'Entrar com e-mail'}).click();
  await expect(page.getByText('teste@example.invalid',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Plano e assinatura',exact:true}).click();
  await page.getByRole('button',{name:'Simular Plus',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Plano Plus · simulação interna'})).toBeVisible();
  await page.getByRole('button',{name:'Relatar um erro',exact:true}).click();
  await page.getByLabel('Assunto',{exact:true}).fill('Falha sintética');
  await page.getByLabel('Descrição do chamado').fill('Teste do fluxo de suporte.');
  await page.getByRole('button',{name:'Enviar chamado'}).click();
  await expect(page.getByText('Falha sintética',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Conta e segurança',exact:true}).click();
  await page.getByRole('button',{name:'Sair da conta'}).click();
  await expect(page.getByRole('button',{name:'Continuar com Google'})).toBeVisible();
});
test('o mesmo controle de raciocínio funciona na criação do projeto',async({page})=>{
  await desktop(page,true);
  await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await page.getByRole('button',{name:'fixture/vision',exact:true}).click();
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await expect(page.getByRole('button',{name:'Automático',exact:true})).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button',{name:'Médio',exact:true}).click();
  await expect(page.getByRole('button',{name:'Médio',exact:true})).toHaveAttribute('aria-pressed','true');
});
test('assinatura usa níveis reais e seleciona skills por barra ou menu',async({page})=>{
  await desktop(page,true);
  await page.getByRole('button',{name:'Chat sintético',exact:true}).click();
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  await expect(page.getByRole('button',{name:'Médio',exact:true})).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button',{name:'Alto',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>Reflect.get(window,'commercialCalls').some((c:{action:string;payload:{aiSelection?:{reasoning?:string}}})=>c.action==='ai.selection.save'&&c.payload.aiSelection?.reasoning==='high'))).toBe(true);
  await page.getByRole('button',{name:'Fechar modelos'}).click();
  await input.fill('/code');
  await page.getByRole('button',{name:'/code-review · Revisar código'}).click();
  await expect(page.getByRole('button',{name:'Remover skill'})).toBeVisible();
  await page.getByRole('button',{name:'Remover skill'}).click();
  await page.getByRole('button',{name:'Adicionar à mensagem'}).click();
  await page.getByRole('menuitem',{name:'Skills da assinatura'}).click();
  await expect(page.getByText('Avaliar os trechos revisados.')).toBeVisible();
});

test('habilidades aceitam teclado e uma busca vazia não vira chamada paga',async({page})=>{
  await desktop(page,true);
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('/code');
  await expect(page.getByRole('button',{name:'/code-review · Revisar código'})).toBeVisible();
  await input.dispatchEvent('keydown',{key:'Enter',keyCode:229,isComposing:false});
  await expect(input).toHaveValue('/code');
  await expect(page.getByRole('button',{name:'Remover skill'})).toHaveCount(0);
  await input.dispatchEvent('keydown',{key:'Enter',altKey:true});
  await expect(input).toHaveValue('/code');
  await expect(page.getByRole('button',{name:'Remover skill'})).toHaveCount(0);
  await input.press('Shift+Enter');
  await expect(input).toHaveValue('/code\n');
  await expect(page.getByRole('button',{name:'Remover skill'})).toHaveCount(0);
  await input.fill('/inexistente');
  await expect(page.getByText('Nenhuma habilidade encontrada. Tente outro nome ou função.')).toBeVisible();
  await input.press('Enter');
  await expect(input).toHaveValue('/inexistente');
  await input.fill('/code');
  const option=page.getByRole('button',{name:'/code-review · Revisar código'});
  await expect(option).toBeVisible();
  await input.press('ArrowDown');
  await expect(option).toBeFocused();
  await option.press('Enter');
  await expect(page.getByRole('button',{name:'Remover skill'})).toBeVisible();
  await expect(input).toBeFocused();
  await expect(input).toHaveValue('');
  expect(await page.evaluate(()=>Reflect.get(window,'commercialCalls').filter((call:{action:string})=>call.action==='ai.ask').length)).toBe(0);
});

test('skill explícita funciona com chave própria e a troca de conta descarta a autorização',async({page})=>{
  await desktop(page,true,true);
  await page.evaluate(()=>{
    const bridge=Reflect.get(window,'__TAURI_INTERNALS__'),invoke=bridge.invoke.bind(bridge);
    bridge.invoke=async(command:string,args:{request:{action:string}})=>{
      const result=await invoke(command,args);
      if(args.request?.action==='credential.reference.list')return {...result,references:[{...result.references[0],providerSlotRef:{provider:'openrouter',slotId:'personal-fixture'}}]};
      return result;
    };
  });
  await page.getByRole('button',{name:'Chat sintético',exact:true}).click();
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('/code');
  await page.getByRole('button',{name:'/code-review · Revisar código'}).click();
  await input.fill('Revisar sem perder este rascunho');
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Autorizar envio para OpenRouter?'})).toBeVisible();
  await expect(page.getByText('Skill sugerida para esta mensagem:',{exact:false})).toHaveCount(0);
  await page.evaluate(async()=>{
    await Reflect.get(window,'__TAURI_INTERNALS__').invoke('request',{request:{action:'account.signOut',payload:{}}});
    window.dispatchEvent(new Event('openkoda:account-change'));
  });
  await expect(page.getByRole('button',{name:'Autorizar envio'})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Remover skill'})).toHaveCount(0);
  await expect(input).toHaveValue('Revisar sem perder este rascunho');
  await page.getByRole('button',{name:'Adicionar à mensagem'}).click();
  await expect(page.getByRole('menuitem',{name:'Skills da assinatura'})).toHaveCount(0);
  const calls=await page.evaluate(()=>Reflect.get(window,'commercialCalls'));
  expect(calls.filter((call:{action:string})=>call.action==='ai.ask')).toHaveLength(0);
  expect(calls.some((call:{action:string;payload:{skillId?:string}})=>call.action==='ai.context'&&call.payload.skillId==='code-review')).toBe(true);
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
