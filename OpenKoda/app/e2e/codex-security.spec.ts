import type { Page } from '@playwright/test';
import type { SecurityScan } from '../src/security-scan';
import { emptySnapshot } from '../src/preview';
import { test,expect } from './fixtures';
import { installScopeFixture } from './scope-fixture';

type Scenario = 'available'|'unavailable'|'lost-receipt'|'foreign-record'|'existing-policy'|'revoked-policy'|'sensitive'|'github-download-fails'|'github-cancel'|'github-cancel-ready'|'github-retry-cancelled'|'github-retry-failed'|'github-retry-interrupted'|'github-import-reply-lost';
async function newProject(page:Page,scenario:Scenario='available',configuredProvider:'openai'|'openrouter'|'anthropic'='openai'){
  const state=emptySnapshot();
  const existingPolicy=scenario==='existing-policy'||scenario==='revoked-policy';
  state.settings={...state.settings,hasApiKey:true,provider:configuredProvider,model:'fixture/modelo-seguro'};
  await installScopeFixture(page);
  await page.addInitScript(({state,scenario})=>{
    type Call={action:string;payload:Record<string,unknown>};
    const calls:Call[]=[];
    let scan:SecurityScan|null=null;
    let denied=false;
    let importCancelled=false;
    let importKey='';
    let importAttempts=0;
    let importReplyLost=false;
    let finishConcurrentDownload:(()=>void)|undefined;
    let options={advancedEnabled:false,additionalContextEnabled:false,context:['','',''],maxCostUsd:null as number|null,minutes:null as number|null,title:''};
    const provider=state.settings.provider as 'openai'|'openrouter'|'anthropic';
    let scope:ReturnType<typeof window.__scopeFixture>;
    const at='2026-11-12T12:00:00Z';
    function publish(){
      if(!scan)return;
      state.assessments[0].events=[{id:'recibo',kind:'security',at,message:scan.message,securityScan:{
        id:scan.id,createdAt:scan.createdAt,updatedAt:scan.updatedAt,status:scan.status,findingsTotal:scan.result?.coverage.findingsTotal,
      }} as typeof state.assessments[number]['events'][number]];
    }
    window.addEventListener('fixture:stage',event=>{
      if(!scan)return;
      const status=(event as CustomEvent).detail as SecurityScan['status'];
      scan.status=status;scan.stage=status;scan.updatedAt='2026-11-12T12:01:00Z';
      scan.message=status==='running'?'Examinando os caminhos selecionados.':status==='completed'?'Relatório disponível.':'Cancelamento confirmado pelo motor.';
      scan.events.push({at:scan.updatedAt,stage:status,message:scan.message});
      if(status==='completed')scan.result={scanId:'sdk-1',report:'## Relatório sintético\n\nFoi observado um ponto de entrada.',
        findings:[{title:'Entrada sem validação',severity:'high',description:'Revisar o fluxo de entrada.',location:'src/input.ts:3'}],
        coverage:{scope:['src'],excludedPaths:['src/generated'],additionalExclusions:1,findingsTotal:1,reportTruncated:true},cost:{estimatedUsd:0.32}};
      if(status==='completed')state.findings=[{id:'candidato-1',assessmentId:'a',title:'Entrada sem validação',severity:'high',status:'candidate',confidence:'SDK',
        description:'Revisar o fluxo de entrada.',recommendation:'Revisar evidência.',evidenceIds:['resultado-sdk'],createdAt:scan.updatedAt}];
      publish();
    });
    window.addEventListener('fixture:public-message',()=>{
      if(scan){scan.stage='message';scan.events.push({at,stage:'message',message:'Conferi os pontos de entrada. Agora vou verificar os controles de acesso.'});publish();}
    });
    Object.assign(window,{isTauri:true,__dataMode:scenario.endsWith('-policy')?'reviewed':'unreviewed',__securityCalls:calls,__TAURI_INTERNALS__:{invoke:async(_command:string,args:{request?:Call})=>{
      if(!args?.request)return {};
      const {action,payload}=args.request;calls.push({action,payload});
      if(action==='snapshot')return state;
      if(action==='ai.status'){
        const requested=String(payload.provider??provider);
        return {provider:requested,hasApiKey:['openai','openrouter',provider].includes(requested),enabledModels:['fixture/modelo-seguro','fixture/image-only']};
      }
      if(action==='ai.models')return {models:payload.forAnalysis?['fixture/modelo-seguro','fixture/inativo']:['fixture/modelo-seguro','fixture/image-only']};
      if(action==='ai.selection.save'){
        state.assessments[0].aiSelection=payload.aiSelection as typeof state.assessments[number]['aiSelection'];
        return state.assessments[0];
      }
      if(action==='assessment.scopeStatus')return (scope??=window.__scopeFixture(state,['source.rules.v1','ai.remote-review.v1'])).status(payload.assessmentId);
      if(action==='ai.history')return {messages:[]};
      if(action==='ai.chat.status')return {requestId:null,stopping:false};
      if(action==='ai.budget.history')return {rows:[]};
      if(action==='project.options.get')return {options:structuredClone(options)};
      if(action==='project.options.save'){options=payload.options as typeof options;return {options:structuredClone(options)};}
      if(action==='github.repository.import'){
        const replayed=importKey===payload.idempotencyKey;
        if(!replayed){importKey=String(payload.idempotencyKey);importAttempts++;importCancelled=false;}
        if(scenario==='github-import-reply-lost'&&!importReplyLost){importReplyLost=true;throw new Error('Resposta do download perdida.');}
        return {id:'import-'+importAttempts,status:'downloading',name:'remote-fixture',url:String(payload.url),message:'Baixando o repositório.',replayed};
      }
      if(action==='github.repository.get'){
        if(scenario==='github-cancel-ready'&&!importCancelled)await new Promise<void>(resolve=>{finishConcurrentDownload=resolve;});
        const retryStatus=importAttempts===1&&scenario.startsWith('github-retry-')?scenario.slice('github-retry-'.length):null;
        return {id:String(payload.id),status:scenario==='github-cancel-ready'?'ready':importCancelled?'cancelled':retryStatus==='failed'?'failed':retryStatus==='interrupted'?'interrupted':scenario==='github-download-fails'?'failed':scenario==='github-cancel'||retryStatus==='cancelled'?'downloading':'ready',path:'D:/Managed/import-'+importAttempts,name:'remote-fixture',url:'https://github.com/fixture/remote-fixture',revision:'a'.repeat(40),message:scenario==='github-download-fails'?'O repositório é privado ou não está disponível.':'Baixando o repositório.',replayed:true};
      }
      if(action==='github.repository.cancel'){importCancelled=true;finishConcurrentDownload?.();return {id:String(payload.id),status:scenario==='github-cancel-ready'?'ready':'cancelled',path:'D:/Managed/import-'+importAttempts,revision:'a'.repeat(40),name:'remote-fixture',url:'https://github.com/fixture/remote-fixture',message:'Download cancelado.',replayed:false};}
      if(action==='project.launch'){
        const replayed=!!state.projects.length;
        if(!replayed){
          const root=String(payload.root);
          options=payload.analysisOptions as typeof options;
          state.projects=[{id:'p',kind:'code',name:String(payload.name),target:root,description:'',createdAt:at}];
          state.assessments=[{id:'a',projectId:'p',kind:'code',target:root+(payload.relativeTarget?'/'+payload.relativeTarget:''),title:'Análise inicial',objective:String(payload.objective),
            aiSelection:{provider,model:'fixture/modelo-seguro'},createdAt:at,updatedAt:at,status:'draft',events:[],coverage:[],evidence:[],summary:''}];
        }
        return {projectId:'p',assessmentId:'a',replayed};
      }
      if(action==='security.status')return {available:scenario!=='unavailable',requiresSensitiveConfirmation:scenario==='sensitive',reason:scenario==='unavailable'?'Motor oficial indisponível na fixture.':'',provider,model:'fixture/modelo-seguro',hasApiKey:true,runtime:{}};
      if(action==='security.scan.start'){
        scan??={id:'scan-1',assessmentId:'a',status:'starting',stage:'queued',message:'Scan recebido.',provider,model:'fixture/modelo-seguro',
          maxCostUsd:Number(payload.maxCostUsd),createdAt:at,updatedAt:at,events:[],result:null,error:null};
        publish();
        if(scenario==='lost-receipt'&&!denied){denied=true;throw new Error('Recibo perdido após admissão.');}
        return {scan:structuredClone(scan)};
      }
      if(action==='security.scan.get')return {scan:scenario==='foreign-record'&&scan?{...scan,assessmentId:'outra-avaliacao',message:'SENTINELA ESTRANGEIRA'}:structuredClone(scan)};
      if(action==='security.scan.cancel'&&scan){scan.status='cancelling';scan.message='Cancelamento solicitado.';publish();return {scan:structuredClone(scan)};}
      return {};
    }}});
  },{state,scenario});
  await page.goto('/',{waitUntil:'domcontentloaded'});
  if(existingPolicy)await page.evaluate(mode=>{
    type Args={request?:{action:string;payload:Record<string,unknown>}};
    const host=(window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:Args)=>Promise<Record<string,unknown>>}}).__TAURI_INTERNALS__;
    const invoke=host.invoke.bind(host);
    let bound=false;
    host.invoke=async(command,args)=>{
      const result=await invoke(command,args);
      if(args.request?.action==='data.binding.commit')bound=true;
      if(args.request?.action==='data.policy.status'){
        if(mode==='revoked-policy')return {...result,active:false};
        if(args.request.payload.assessmentId&&!bound)return {...result,binding:null};
      }
      return result;
    };
  },scenario);
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByLabel('Pasta do projeto',{exact:true}).fill('D:/Fixture');
  return page.getByRole('button',{name:'Criar projeto',exact:true});
}
async function calls(page:Page,action:string){
  return page.evaluate(name=>(window as unknown as {__securityCalls:{action:string;payload:Record<string,unknown>}[]}).__securityCalls.filter(c=>c.action===name),action);
}
const stage=(page:Page,value:string)=>page.evaluate(detail=>window.dispatchEvent(new CustomEvent('fixture:stage',{detail})),value);

const analysisFileNotice='O provedor recebe trechos do código selecionado. As buscas e leituras consultam uma cópia do escopo autorizado, sem executar comandos.';
test('o seletor de análise oferece somente modelos compatíveis e ativos para a chave',async({page})=>{
  await newProject(page);
  await page.getByRole('button',{name:/Escolher modelo/}).click();
  const chooser=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  const router=chooser.getByRole('heading',{name:'OpenRouter',exact:true}).locator('..');
  await expect(router.getByRole('button',{name:'fixture/modelo-seguro'})).toBeVisible();
  await expect(router.getByRole('button')).toHaveCount(1);
  expect((await calls(page,'ai.models')).every(call=>call.payload.forAnalysis===true)).toBe(true);
});
for(const provider of ['openrouter','anthropic'] as const){
  test(`${provider} inicia a análise com o modelo configurado`,async({page})=>{
    const create=await newProject(page,'available',provider);
    await expect(create).toBeEnabled();
    await create.click();
    const panel=page.getByRole('region',{name:'Análise de segurança'});
    await expect(panel.getByRole('status')).toHaveText('Iniciando');
    expect((await calls(page,'project.launch'))[0].payload.aiSelection).toEqual({provider,model:'fixture/modelo-seguro'});
    expect(await calls(page,'security.scan.start')).toHaveLength(1);
  });
}
test('o aviso de leitura aparece antes de criar e antes de iniciar uma análise existente',async({page})=>{
  const create=await newProject(page,'unavailable');
  await expect(page.getByText(analysisFileNotice,{exact:true})).toBeVisible();
  await expect(create).toBeEnabled();
  await create.click();
  await expect(page.getByRole('alert').filter({hasText:'Motor oficial indisponível'})).toBeVisible();
  await page.evaluate(()=>{
    const host=(window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:{request?:{action:string}})=>Promise<unknown>}}).__TAURI_INTERNALS__;
    const invoke=host.invoke.bind(host);
    host.invoke=async(command,args)=>{
      const response=await invoke(command,args);
      return args.request?.action==='security.status'?{...response as object,available:true,reason:''}:response;
    };
  });
  await page.getByRole('button',{name:'Abrir conversa criada'}).click();
  const panel=page.getByRole('region',{name:'Análise de segurança'});
  await expect(panel.getByText(analysisFileNotice,{exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Iniciar análise de segurança',exact:true}).click();
  await expect(panel.getByRole('status')).toHaveText('Iniciando');
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
});

test('OpenAI continua analisando e OpenRouter permanece selecionável nas conversas',async({page})=>{
  await (await newProject(page)).click();
  await expect(page.getByRole('region',{name:'Análise de segurança'}).getByRole('status')).toHaveText('Iniciando');
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
  await stage(page,'completed');
  await page.getByRole('button',{name:'Escolher modelo · fixture/modelo-seguro'}).click();
  const chooser=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await chooser.getByRole('heading',{name:'OpenRouter',exact:true}).locator('..').getByRole('button',{name:'fixture/modelo-seguro',exact:true}).click();
  await expect(page.getByRole('button',{name:'Escolher modelo · fixture/modelo-seguro'})).toHaveAttribute('title','OpenRouter · fixture/modelo-seguro');
  expect((await calls(page,'ai.selection.save'))[0].payload).toMatchObject({aiSelection:{provider:'openrouter',model:'fixture/modelo-seguro'}});
  await expect(page.getByRole('region',{name:'Análise de segurança'}).getByRole('status')).toHaveText('Concluída');
});

test('a criação executa somente o motor selecionado e apresenta progresso, relatório e cobertura recebidos',async({page})=>{
  const create=await newProject(page);
  await expect(page.getByLabel('Limite da análise em USD')).toHaveCount(0);
  await page.getByRole('switch',{name:'Opções avançadas',exact:true}).check();
  await page.getByRole('checkbox',{name:'Personalizar limite de custo'}).check();
  await expect(page.getByLabel('Limite da análise em USD')).toHaveValue('5');
  await page.getByLabel('Limite da análise em USD').fill('2.50');
  await page.getByLabel('Alvo',{exact:true}).selectOption('subset');
  await page.getByLabel('Caminho dentro do projeto').fill('src');
  await page.getByLabel('Descrição',{exact:true}).fill('Revisar entradas.');
  await create.click();
  const panel=page.getByRole('region',{name:'Análise de segurança'});
  await expect(panel.getByRole('status')).toHaveText('Iniciando');
  const [start]=await calls(page,'security.scan.start');
  expect(start.payload).toEqual({assessmentId:'a',idempotencyKey:expect.stringMatching(/^[0-9a-f-]{36}$/),prompt:'Revisar entradas.',target:'src',maxCostUsd:2.5});
  await stage(page,'running');
  await expect(panel.getByRole('status')).toHaveText('Em análise');
  await expect(panel.getByRole('paragraph').filter({hasText:'Examinando os caminhos selecionados.'})).toBeVisible();
  await stage(page,'completed');
  await expect(panel.getByRole('status')).toHaveText('Concluída');
  await panel.getByText('Entrada sem validação',{exact:true}).click();
  await expect(panel.getByText('src/input.ts:3',{exact:true})).toBeVisible();
  await panel.getByText('Cobertura da análise',{exact:true}).click();
  await expect(panel.getByText('src/generated',{exact:true})).toBeVisible();
  await panel.getByText('Relatório da análise',{exact:true}).click();
  await expect(panel.getByRole('heading',{name:'Relatório sintético'})).toBeVisible();
  await expect(panel.getByText('O relatório armazenado é parcial.')).toBeVisible();
  await page.getByRole('textbox',{name:'Mensagem da conversa'}).fill('Explique a localização.');
  await expect(page.getByRole('button',{name:'Enviar mensagem'})).toBeEnabled();
  for(const action of ['assessment.launchPlan','plan.run','ai.ask'])expect(await calls(page,action)).toEqual([]);
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
  await page.getByRole('button',{name:'Painel do projeto'}).click();
  const overview=page.getByRole('complementary',{name:'Projeto',exact:true});
  await overview.getByRole('button',{name:'Execuções 1'}).click();
  await expect(overview.getByRole('table',{name:'Execuções'}).getByRole('row',{name:/Análise do OpenKoda Análise inicial Concluída/})).toBeVisible();
  await overview.getByRole('button',{name:'Achados 1'}).click();
  await expect(overview.getByRole('table',{name:'Achados registrados'})).toContainText('Candidato');
  await expect(overview.getByRole('table',{name:'Resultados da análise'})).toContainText('Análise inicial');
});

test('o cancelamento aguarda confirmação do host e preserva a mensagem em edição',async({page})=>{
  await (await newProject(page)).click();
  const panel=page.getByRole('region',{name:'Análise de segurança'});
  const draft=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await draft.fill('Guardar a pergunta.');
  await panel.getByRole('button',{name:'Cancelar análise de segurança'}).click();
  await expect(panel.getByRole('status')).toHaveText('Cancelamento solicitado');
  await expect(panel.getByRole('button',{name:'Cancelar análise de segurança'})).toBeDisabled();
  await stage(page,'cancelled');
  await expect(panel.getByRole('status')).toHaveText('Cancelada');
  await expect(draft).toHaveValue('Guardar a pergunta.');
  expect(await calls(page,'security.scan.cancel')).toHaveLength(1);
  expect(await calls(page,'assessment.cancel')).toEqual([]);
});

test('motor indisponível mantém a conversa criada, sem execução alternativa ou cobrança',async({page})=>{
  await (await newProject(page,'unavailable')).click();
  await expect(page.getByRole('alert').filter({hasText:'Motor oficial indisponível'})).toBeVisible();
  await page.getByRole('button',{name:'Abrir conversa criada'}).click();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
  for(const action of ['security.scan.start','assessment.launchPlan','plan.run','ai.ask'])expect(await calls(page,action)).toEqual([]);
});

test('a repetição após perda de recibo conserva a chave idempotente do scan',async({page})=>{
  const create=await newProject(page,'lost-receipt');
  await create.click();
  await expect(page.getByRole('alert').filter({hasText:'Recibo perdido após admissão.'})).toBeVisible();
  await create.click();
  await expect(page.getByRole('region',{name:'Análise de segurança'})).toBeVisible();
  const sent=await calls(page,'security.scan.start');
  expect(sent).toHaveLength(2);
  expect(sent[1].payload).toEqual(sent[0].payload);
});

test('um registro cuja avaliação diverge fecha a leitura e impede outra análise',async({page})=>{
  await (await newProject(page,'foreign-record')).click();
  await expect(page.getByRole('alert').filter({hasText:'registro válido desta análise'})).toBeVisible();
  await expect(page.getByRole('button',{name:'Iniciar análise de segurança'})).toBeDisabled();
  await expect(page.getByText('SENTINELA ESTRANGEIRA')).toHaveCount(0);
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
});

test('uma política de conversa existente é vinculada à nova avaliação antes do scan',async({page})=>{
  await (await newProject(page,'existing-policy')).click();
  await expect(page.getByRole('region',{name:'Análise de segurança'})).toBeVisible();
  const names=await page.evaluate(()=>(window as unknown as {__dataCalls:{action:string}[]}).__dataCalls.map(x=>x.action));
  expect(names.indexOf('data.binding.commit')).toBeGreaterThan(names.indexOf('data.binding.prepare'));
  expect(names.indexOf('security.scan.start')).toBeGreaterThan(names.indexOf('data.binding.commit'));
  expect(names).not.toContain('data.policy.prepare');
  expect(names).not.toContain('ai.context');
  expect(names).not.toContain('ai.ask');
});

test('uma política revogada permanece inativa e impede o início do scan',async({page})=>{
  await (await newProject(page,'revoked-policy')).click();
  await expect(page.getByRole('alert').filter({hasText:'política de dados deste projeto está inativa'})).toBeVisible();
  expect(await calls(page,'security.scan.start')).toEqual([]);
});

test('fonte sensível exige consentimento específico para o modelo e teto revisados',async({page})=>{
  const create=await newProject(page,'sensitive');
  await page.getByRole('switch',{name:'Opções avançadas',exact:true}).check();
  await page.getByRole('checkbox',{name:'Personalizar limite de custo'}).check();
  await create.click();
  const consent=page.getByRole('checkbox',{name:/Autorizo enviar a fonte sensível/});
  await expect(consent).not.toBeChecked();
  await expect(create).toBeDisabled();
  expect(await calls(page,'security.scan.start')).toEqual([]);
  await consent.check();
  await page.getByLabel('Limite da análise em USD').fill('3');
  await create.click();
  await expect(consent).not.toBeChecked();
  await consent.check();
  await create.click();
  await expect(page.getByRole('region',{name:'Análise de segurança'})).toBeVisible();
  const sent=await calls(page,'security.scan.start');
  expect(sent).toHaveLength(1);
  expect(sent[0].payload).toMatchObject({maxCostUsd:3,acknowledgeSensitive:true});
});

test('o limite padrão não ocupa o formulário nem o chat e as atualizações públicas vêm do motor',async({page})=>{
  const create=await newProject(page);
  await expect(page.getByLabel('Limite da análise em USD')).toHaveCount(0);
  await create.click();
  expect((await calls(page,'security.scan.start'))[0].payload.maxCostUsd).toBe(5);
  const panel=page.getByRole('region',{name:'Análise de segurança'});
  await expect(panel.getByRole('spinbutton')).toHaveCount(0);
  await page.evaluate(()=>window.dispatchEvent(new Event('fixture:public-message')));
  await expect(panel.getByRole('log',{name:'Atualizações do OpenKoda'})).toContainText('Conferi os pontos de entrada. Agora vou verificar os controles de acesso.');
  await expect(panel).not.toContainText('Codex Security');
});

test('a importação do GitHub termina antes de criar e analisar a cópia local',async({page})=>{
  const create=await newProject(page);
  await page.getByRole('button',{name:'GitHub',exact:true}).click();
  await expect(page.getByLabel('Pasta do projeto',{exact:true})).toHaveCount(0);
  await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/fixture/remote-fixture');
  await create.click();
  await expect(page.getByRole('region',{name:'Análise de segurança'})).toContainText('Iniciando');
  const [download]=await calls(page,'github.repository.import'),[launch]=await calls(page,'project.launch');
  expect(download.payload).toMatchObject({url:'https://github.com/fixture/remote-fixture',idempotencyKey:expect.any(String)});
  expect(launch.payload).toMatchObject({root:'D:/Managed/import-1',name:'remote-fixture',kind:'code',githubImportId:'import-1',idempotencyKey:download.payload.idempotencyKey});
  expect(await calls(page,'github.repository.import')).toHaveLength(1);
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
});

test('download recusado não cria projeto nem inicia análise',async({page})=>{
  const create=await newProject(page,'github-download-fails');
  await page.getByRole('button',{name:'GitHub',exact:true}).click();
  await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/fixture/private');
  await create.click();
  await expect(page.getByRole('alert')).toContainText('privado ou não está disponível');
  expect(await calls(page,'project.launch')).toEqual([]);expect(await calls(page,'security.scan.start')).toEqual([]);
});

test('cancelar download impede a criação do projeto',async({page})=>{
  const create=await newProject(page,'github-cancel');
  await page.getByRole('button',{name:'GitHub',exact:true}).click();
  await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/fixture/remote-fixture');
  await create.click();await page.getByRole('button',{name:'Cancelar download'}).click();
  await expect(page.getByRole('button',{name:'Tentar novo download',exact:true})).toBeEnabled();
  await expect(create).toBeDisabled();
  expect(await calls(page,'github.repository.cancel')).toHaveLength(1);
  expect(await calls(page,'project.launch')).toEqual([]);expect(await calls(page,'security.scan.start')).toEqual([]);
});

test('cancelar quando o download termina em paralelo impede criar projeto e iniciar análise paga',async({page})=>{
  const create=await newProject(page,'github-cancel-ready');
  await page.getByRole('button',{name:'GitHub',exact:true}).click();
  await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/fixture/remote-fixture');
  await create.click();
  await expect.poll(async()=>(await calls(page,'github.repository.get')).length).toBe(1);
  await page.getByRole('button',{name:'Cancelar download'}).click();
  await expect(page.getByRole('alert')).toContainText('Criação cancelada');
  await expect(create).toBeEnabled();
  expect(await calls(page,'project.launch')).toEqual([]);expect(await calls(page,'security.scan.start')).toEqual([]);
});

for(const terminal of ['cancelled','failed','interrupted'] as const){
  test(`um download ${terminal} só ganha outra intenção por repetição explícita`,async({page})=>{
    const create=await newProject(page,`github-retry-${terminal}`);
    await page.getByRole('button',{name:'GitHub',exact:true}).click();
    await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/fixture/remote-fixture');
    await create.click();
    if(terminal==='cancelled')await page.getByRole('button',{name:'Cancelar download'}).click();
    const retry=page.getByRole('button',{name:'Tentar novo download',exact:true});
    await expect(retry).toBeEnabled();
    await expect(create).toBeDisabled();
    expect(await calls(page,'project.launch')).toEqual([]);expect(await calls(page,'security.scan.start')).toEqual([]);
    await retry.click();
    await expect(page.getByRole('region',{name:'Análise de segurança'})).toContainText('Iniciando');
    const imports=await calls(page,'github.repository.import');
    expect(imports).toHaveLength(2);
    expect(imports[1].payload.url).toBe(imports[0].payload.url);
    expect(imports[1].payload.idempotencyKey).not.toBe(imports[0].payload.idempotencyKey);
    expect((await calls(page,'project.launch'))[0].payload).toMatchObject({githubImportId:'import-2',root:'D:/Managed/import-2'});
    expect(await calls(page,'project.launch')).toHaveLength(1);expect(await calls(page,'security.scan.start')).toHaveLength(1);
  });
}

test('resposta perdida do download preserva a intenção e não oferece reinício desconhecido',async({page})=>{
  const create=await newProject(page,'github-import-reply-lost');
  await page.getByRole('button',{name:'GitHub',exact:true}).click();
  await page.getByLabel('Link do repositório no GitHub').fill('https://github.com/fixture/remote-fixture');
  await create.click();
  await expect(page.getByRole('alert')).toContainText('Resposta do download perdida.');
  await expect(page.getByRole('button',{name:'Tentar novo download',exact:true})).toHaveCount(0);
  await create.click();
  await expect(page.getByRole('region',{name:'Análise de segurança'})).toContainText('Iniciando');
  const imports=await calls(page,'github.repository.import');
  expect(imports).toHaveLength(2);expect(imports[1].payload).toEqual(imports[0].payload);
  expect((await calls(page,'project.launch'))[0].payload.githubImportId).toBe('import-1');
  expect(await calls(page,'project.launch')).toHaveLength(1);expect(await calls(page,'security.scan.start')).toHaveLength(1);
});

test('o projeto preserva contexto e opções avançadas editadas depois da criação',async({page})=>{
  await (await newProject(page)).click();await stage(page,'completed');
  await page.getByRole('button',{name:'Painel do projeto'}).click();
  const panel=page.getByRole('complementary',{name:'Projeto',exact:true});
  await panel.getByRole('tab',{name:'Configurações',exact:true}).click();
  const options=panel.getByRole('region',{name:'Opções de análise do projeto'});
  await options.getByRole('switch',{name:'Contexto adicional',exact:true}).check();
  await options.getByLabel('Vetores de ataque',{exact:true}).fill('Contorno da autenticação');
  await options.getByRole('switch',{name:'Opções avançadas',exact:true}).check();
  await expect(options.getByLabel('Limite da análise em USD')).toHaveCount(0);
  await options.getByRole('checkbox',{name:'Personalizar limite de custo'}).check();
  await options.getByLabel('Limite da análise em USD').fill('1.75');
  await options.getByLabel('Título das próximas conversas').fill('Revisão de autorização');
  await options.getByLabel('Limite para iniciar operações (minutos)').fill('90');
  await options.getByRole('button',{name:'Salvar opções de análise'}).click();
  await expect(options.getByRole('status')).toHaveText('Opções salvas para as próximas análises e conversas.');
  const [save]=await calls(page,'project.options.save');
  expect(save.payload).toEqual({projectId:'p',options:{advancedEnabled:true,additionalContextEnabled:true,context:['Contorno da autenticação','',''],title:'Revisão de autorização',minutes:90,maxCostUsd:1.75}});
  await panel.getByRole('tab',{name:'Visão geral',exact:true}).click();
  await panel.getByRole('tab',{name:'Configurações',exact:true}).click();
  await expect(options.getByLabel('Limite da análise em USD')).toHaveValue('1.75');
  await expect(options.getByLabel('Vetores de ataque',{exact:true})).toHaveValue('Contorno da autenticação');
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
});
