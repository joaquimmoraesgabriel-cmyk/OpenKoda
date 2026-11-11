import { expect, test } from './fixtures';
import { emptySnapshot } from '../src/preview';
import { installScopeFixture } from './scope-fixture';
import type { Page } from '@playwright/test';

test('a conversa troca de provedor e aprova o plano TCP exato antes de executar', async ({page}) => {
  const snapshot = emptySnapshot();
  const now = new Date().toISOString();
  snapshot.projects.push({id:'project-chat',name:'Projeto TCP',kind:'network',target:'127.0.0.1',description:'',createdAt:now});
  snapshot.assessments.push({
    id:'chat-tcp',projectId:'project-chat',kind:'network',target:'127.0.0.1',title:'Consulta TCP',objective:'Examinar TCP',
    aiSelection:{provider:'openai',model:'modelo-a'},createdAt:now,updatedAt:now,status:'awaiting_approval',events:[],evidence:[],coverage:[],summary:'',
  });
  snapshot.settings={...snapshot.settings,provider:'openai',model:'modelo-a',endpoint:'https://api.openai.com/v1',hasApiKey:true};
  snapshot.plans=[{
    id:'plan-tcp',assessmentId:'chat-tcp',adapter:'network.tcp-connect.v1',target:'127.0.0.1',timeoutSeconds:10,outputLimitKiB:16,
    digest:'digest-tcp',status:'proposed',createdAt:now,approvedAt:null,expiresAt:new Date(Date.now()+3_600_000).toISOString(),
    inputFiles:[],resolvedAddresses:['127.0.0.1:443'],requestedPorts:[443],
  }];
  await page.addInitScript(s=>{
    const sent: {action:string;payload:Record<string,unknown>}[]=[];
    Object.assign(window,{isTauri:true,__chatCalls:sent,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request?:typeof sent[number]})=>{
      const req=args?.request;
      if(!req)return {};
      sent.push(req);
      if(req.action==='snapshot')return s;
      if(req.action==='environment.status')return {tools:[],status:'unavailable',message:'Ambiente de teste'};
      if(req.action==='ai.status'){
        const provider=req.payload.provider;
        return {provider,hasApiKey:['openai','openrouter'].includes(String(provider)),enabledModels:provider==='openai'?['modelo-a']:['fornecedor/modelo-b']};
      }
      if(req.action==='ai.selection.save')s.assessments[0].aiSelection=req.payload.aiSelection as typeof s.assessments[0]['aiSelection'];
      if(req.action==='ai.history')return {messages:[]};
      if(req.action==='ai.chat.status')return {requestId:null,stopping:false};
      return {};
    }}});
  },snapshot);
  await page.goto('/');
  const chat=page.getByRole('region',{name:'Conversa do projeto'});
  await expect(chat).toBeVisible();
  await chat.getByRole('button',{name:/Escolher modelo/}).click();
  const dialog=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await dialog.getByRole('button',{name:'OpenRouter',exact:true}).click();
  await dialog.getByRole('searchbox',{name:'Buscar modelos'}).fill('modelo-b');
  await dialog.getByRole('button',{name:'fornecedor/modelo-b'}).click();
  await expect(chat.getByRole('button',{name:/Escolher modelo/})).toContainText('fornecedor/modelo-b');
  const calls=()=>page.evaluate(()=>(window as unknown as {__chatCalls:{action:string;payload:Record<string,unknown>}[]}).__chatCalls.filter(x=>/^plan\./.test(x.action)));
  await chat.getByRole('button',{name:'Executar operação'}).click();
  await expect(chat.getByRole('button',{name:'Autorizar operação'})).toBeVisible();
  expect(await calls()).toEqual([]);
  await chat.getByRole('button',{name:'Autorizar operação'}).click();
  await expect.poll(calls).toEqual([
    {action:'plan.approve',payload:{id:'plan-tcp',digest:'digest-tcp'}},
    {action:'plan.run',payload:{id:'plan-tcp',digest:'digest-tcp'}},
  ]);
});

async function installLocalChat(page:Page){
  await installScopeFixture(page);
  const snapshot=emptySnapshot();
  const now=new Date().toISOString();
  snapshot.settings={...snapshot.settings,provider:'openai',model:'fixture-model',endpoint:'https://api.openai.com/v1',hasApiKey:true};
  snapshot.projects=[{id:'workspace',name:'Exemplo local',kind:'code',target:'D:/fixture',description:'',createdAt:now}];
  snapshot.assessments=[{id:'thread',projectId:'workspace',title:'Consulta de código',kind:'code',target:'D:/fixture',objective:'Examinar código',status:'partial',events:[],evidence:[],coverage:[],summary:'',createdAt:now,updatedAt:now,aiSelection:{provider:'openai',model:'fixture-model'}}];
  await page.addInitScript(s=>{
    const trace:string[]=[];
    const scope=window.__scopeFixture(s,['source.rules.v1','ai.remote-review.v1']);
    Object.assign(window,{isTauri:true,__planTrace:trace,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request?:{action:string;payload:Record<string,unknown>}})=>{
      if(!args.request)return {};
      const {action,payload}=args.request;
      trace.push(action);
      if(action==='snapshot')return s;
      if(action==='environment.status')return {tools:[],status:'unavailable',message:'Ambiente de teste'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='ai.chat.status')return {requestId:null,stopping:false};
      if(action==='ai.history')return {messages:[]};
      if(action==='ai.context')return {digest:'packet-1',bytes:100,context:{assessmentId:'thread'}};
      if(action==='ai.budget.status')return {enabled:false,degraded:false};
      if(action==='ai.ask')return new Promise(resolve=>window.addEventListener('test:responder',()=>resolve({}),{once:true}));
      if(action==='ai.operation.propose')return {id:'p1',requestId:'q1',responseId:'r1',assessmentId:'thread',adapter:'source.rules.v1',contextDigest:payload.contextDigest,rationale:'Verificar padrões nos arquivos permitidos.',question:null,model:'fixture-model',endpoint:s.settings.endpoint,toolsExecuted:false,requiresHumanReview:true,availabilityVerified:false,notice:'Sugestão'};
      if(action==='assessment.sourcePlan'){
        s.plans=[{id:'prepared',digest:'typed-plan',assessmentId:'thread',adapter:'source.rules.v1',target:'D:/fixture',timeoutSeconds:20,outputLimitKiB:16,inputFiles:[],resolvedAddresses:[],scopeRef:scope.ref,createdAt:new Date().toISOString(),expiresAt:new Date(Date.now()+60000).toISOString(),approvedAt:null,status:'proposed'}];
        return s.plans[0];
      }
      return {};
    }}});
  },snapshot);
  await page.goto('/');
}

test('pedir verificação cria uma sugestão delimitada e só executa após autorizar', async ({page})=>{
  await installLocalChat(page);
  const chat=page.getByRole('region',{name:'Conversa do projeto'});
  await chat.getByLabel('Modo de aprovação').selectOption('always');
  await chat.getByRole('button',{name:'Pedir verificação'}).click();
  await chat.getByLabel('Mensagem da conversa').fill('Examine sinais estáticos no código.');
  await chat.getByLabel('Enviar mensagem').click();
  await chat.getByRole('button',{name:'Autorizar envio'}).click();
  await expect(chat.getByText('Verificar padrões nos arquivos permitidos.')).toBeVisible();
  const trace=()=>page.evaluate(()=>(window as unknown as {__planTrace:string[]}).__planTrace.filter(a=>['ai.ask','ai.operation.propose','assessment.sourcePlan','plan.approve','plan.run'].includes(a)));
  expect(await trace()).toEqual(['ai.operation.propose']);
  await chat.getByRole('button',{name:'Preparar esta verificação'}).click();
  await chat.getByRole('button',{name:'Executar operação'}).click();
  expect(await trace()).toEqual(['ai.operation.propose','assessment.sourcePlan']);
  await chat.getByRole('button',{name:'Autorizar operação'}).click();
  await expect.poll(trace).toEqual(['ai.operation.propose','assessment.sourcePlan','plan.approve','plan.run']);
});

test('a análise inicial e a resposta de um turno preservam a próxima mensagem em edição',async({page})=>{
  await installLocalChat(page);
  const chat=page.getByRole('region',{name:'Conversa do projeto'});
  const draft=chat.getByLabel('Mensagem da conversa');
  await draft.fill('Rascunho enquanto a análise começa');
  await page.evaluate(async()=>{
    const source='/src/chat-opening.ts';
    const {stageOpeningAnalysis}=await import(source);
    stageOpeningAnalysis('thread','Faça a análise inicial.');
  });
  const requests=()=>page.evaluate(()=>(window as unknown as {__planTrace:string[]}).__planTrace.filter(a=>a==='ai.ask').length);
  const reply=()=>page.evaluate(()=>{window.dispatchEvent(new Event('test:responder'));});
  await expect.poll(requests).toBe(1);
  await expect(draft).toHaveValue('Rascunho enquanto a análise começa');
  await reply();
  await expect(chat.getByLabel('Enviar mensagem')).toBeEnabled();
  await expect(draft).toHaveValue('Rascunho enquanto a análise começa');
  await chat.getByLabel('Enviar mensagem').click();
  await expect.poll(requests).toBe(2);
  await draft.fill('Próxima pergunta antes da resposta');
  await reply();
  await expect(chat.getByLabel('Enviar mensagem')).toBeEnabled();
  await expect(draft).toHaveValue('Próxima pergunta antes da resposta');
});

