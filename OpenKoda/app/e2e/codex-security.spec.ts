import type { Page } from '@playwright/test';
import type { SecurityScan } from '../src/security-scan';
import { emptySnapshot } from '../src/preview';
import { test,expect } from './fixtures';
import { installScopeFixture } from './scope-fixture';

type Scenario = 'available'|'unavailable'|'lost-receipt'|'foreign-record'|'existing-policy'|'revoked-policy'|'sensitive';
async function newProject(page:Page,scenario:Scenario='available'){
  const state=emptySnapshot();
  const existingPolicy=scenario==='existing-policy'||scenario==='revoked-policy';
  state.settings={...state.settings,hasApiKey:true,provider:existingPolicy?'openai':'openrouter',model:'fixture/modelo-seguro'};
  await installScopeFixture(page);
  await page.addInitScript(({state,scenario})=>{
    type Call={action:string;payload:Record<string,unknown>};
    const calls:Call[]=[];
    let scan:SecurityScan|null=null;
    let denied=false;
    const provider=state.settings.provider==='openai'?'openai':'openrouter';
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
    Object.assign(window,{isTauri:true,__dataMode:scenario.endsWith('-policy')?'reviewed':'unreviewed',__securityCalls:calls,__TAURI_INTERNALS__:{invoke:async(_command:string,args:{request?:Call})=>{
      if(!args?.request)return {};
      const {action,payload}=args.request;calls.push({action,payload});
      if(action==='snapshot')return state;
      if(action==='ai.status')return {provider,hasApiKey:true,enabledModels:['fixture/modelo-seguro']};
      if(action==='assessment.scopeStatus')return (scope??=window.__scopeFixture(state,['source.rules.v1','ai.remote-review.v1'])).status(payload.assessmentId);
      if(action==='ai.history')return {messages:[]};
      if(action==='ai.chat.status')return {requestId:null,stopping:false};
      if(action==='ai.budget.history')return {rows:[]};
      if(action==='project.launch'){
        const replayed=!!state.projects.length;
        if(!replayed){
          const root=String(payload.root);
          state.projects=[{id:'p',kind:'code',name:'Fixture',target:root,description:'',createdAt:at}];
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

test('a criação executa somente o motor selecionado e apresenta progresso, relatório e cobertura recebidos',async({page})=>{
  const create=await newProject(page);
  await expect(page.getByLabel('Limite da análise em USD')).toHaveValue('5');
  await page.getByLabel('Limite da análise em USD').fill('2.50');
  await page.getByLabel('Alvo',{exact:true}).selectOption('subset');
  await page.getByLabel('Caminho dentro do projeto').fill('src');
  await page.getByLabel('Descrição',{exact:true}).fill('Revisar entradas.');
  await create.click();
  const panel=page.getByRole('region',{name:'Análise do Codex Security'});
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
  await panel.getByText('Relatório do Codex Security',{exact:true}).click();
  await expect(panel.getByRole('heading',{name:'Relatório sintético'})).toBeVisible();
  await expect(panel.getByText('O relatório armazenado é parcial.')).toBeVisible();
  await page.getByRole('textbox',{name:'Mensagem da conversa'}).fill('Explique a localização.');
  await expect(page.getByRole('button',{name:'Enviar mensagem'})).toBeEnabled();
  for(const action of ['assessment.launchPlan','plan.run','ai.ask'])expect(await calls(page,action)).toEqual([]);
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
  await page.getByRole('button',{name:'Painel do projeto'}).click();
  const overview=page.getByRole('complementary',{name:'Projeto',exact:true});
  await overview.getByRole('button',{name:'Execuções 1'}).click();
  await expect(overview.getByRole('table',{name:'Execuções'}).getByRole('row',{name:/Codex Security Análise inicial Concluída/})).toBeVisible();
  await overview.getByRole('button',{name:'Achados 1'}).click();
  await expect(overview.getByRole('table',{name:'Achados registrados'})).toContainText('Candidato');
  await expect(overview.getByRole('table',{name:'Resultados do Codex Security'})).toContainText('Análise inicial');
});

test('o cancelamento aguarda confirmação do host e preserva a mensagem em edição',async({page})=>{
  await (await newProject(page)).click();
  const panel=page.getByRole('region',{name:'Análise do Codex Security'});
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
  await expect(page.getByRole('region',{name:'Análise do Codex Security'})).toBeVisible();
  const sent=await calls(page,'security.scan.start');
  expect(sent).toHaveLength(2);
  expect(sent[1].payload).toEqual(sent[0].payload);
});

test('um registro cuja avaliação diverge fecha a leitura e impede outra análise',async({page})=>{
  await (await newProject(page,'foreign-record')).click();
  await page.getByText('Analisar com Codex Security',{exact:true}).click();
  await expect(page.getByRole('alert').filter({hasText:'registro válido desta análise'})).toBeVisible();
  await expect(page.getByRole('button',{name:'Iniciar análise de segurança'})).toBeDisabled();
  await expect(page.getByText('SENTINELA ESTRANGEIRA')).toHaveCount(0);
  expect(await calls(page,'security.scan.start')).toHaveLength(1);
});

test('uma política de conversa existente é vinculada à nova avaliação antes do scan',async({page})=>{
  await (await newProject(page,'existing-policy')).click();
  await expect(page.getByRole('region',{name:'Análise do Codex Security'})).toBeVisible();
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
  await expect(page.getByRole('region',{name:'Análise do Codex Security'})).toBeVisible();
  const sent=await calls(page,'security.scan.start');
  expect(sent).toHaveLength(1);
  expect(sent[0].payload).toMatchObject({maxCostUsd:3,acknowledgeSensitive:true});
});
