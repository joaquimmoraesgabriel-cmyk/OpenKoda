import { openAssessmentList } from './navigation';
import { chooseAiData, reviewKaliData } from './data-fixture';
import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// The provider and native API are mocked. No request in these tests uses API credits.
async function fixture(page:Page,mode='source'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__proposalCalls:{action:string;payload:Record<string,unknown>}[]};w.isTauri=true;w.__proposalCalls=[];
    const assessment={id:'assessment-proposal',projectId:'project-proposal',title:'Avaliação para sugestão',kind:mode==='network'?'network':'code',target:mode==='network'?'192.0.2.1':'D:/Fixture/source',objective:'Objetivo sintético da avaliação',status:'draft',createdAt:now,updatedAt:now,events:[],evidence:[{id:'eligible-evidence',name:'Evidência pequena',content:'Conteúdo sintético',sha256:'eligible-hash',createdAt:now},{id:'oversized-evidence',name:'Evidência acima de 32 KiB',content:'x'.repeat(33000),sha256:'oversized-hash',createdAt:now}],coverage:[],summary:''};
    const state={projects:[{id:assessment.projectId,name:'Projeto para sugestão',kind:assessment.kind,target:assessment.target,description:'Fixture',createdAt:now,archived:false}],assessments:[assessment],findings:[],settings:{provider:'openai',endpoint:'https://api.openai.com/v1',model:'fixture-model',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false},plans:[] as Record<string,unknown>[],executions:[],validations:[]};
    const scope=window.__scopeFixture(state,mode==='network'?['ai.remote-review.v1','network.tcp-connect.v1']:mode==='kali'?['ai.remote-review.v1','kali.rg.signals.v1']:['ai.remote-review.v1','source.rules.v1','local.inventory.v1']);
    const requests:Record<string,unknown>[]=[];
    const graphSource={requestId:'proposal-request',proposalId:'operation-proposal',contextDigest:'proposal-context',
      evidenceId:'eligible-evidence',evidenceSha256:'eligible-hash',scopeRef:scope.ref,adapter:'source.rules.v1',
      base:{head:null,generation:0,lastEvent:0}};
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__proposalCalls.push({action,payload});
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture',tools:[],executorReady:false};
      if(action==='ai.history')return {messages:[],requests:structuredClone(requests)};
      if(action==='ai.context')return {context:{assessmentId:assessment.id,objective:assessment.objective,target:assessment.target,evidenceIds:payload.evidenceIds},digest:'proposal-context',bytes:160,notice:'Contexto sintético destinado à chamada revisada.'};
      if(action==='kali.mcp.status')return {active:null,recent:[],hasUnresolved:false,capabilities:['runtime_diagnostics','hash_bytes','scan_bytes'],executorReady:false};
      if(action==='ai.operation.propose'){
        const adapter=mode==='network'?'network.tcp-connect.v1':mode==='kali'?'kali.rg.signals.v1':'source.rules.v1';
        if(payload.assessmentId!==assessment.id||payload.contextDigest!=='proposal-context'||payload.allowRemote!==true||!(payload.allowedAdapters as string[]).includes(adapter))throw new Error('Consentimento, contexto ou opções divergentes.');
        const proposal={id:'operation-proposal',requestId:'proposal-request',responseId:'provider-response',assessmentId:assessment.id,contextDigest:'proposal-context',adapter,rationale:'Observar os padrões selecionados sem confirmar vulnerabilidade.',question:mode==='network'?'Quais portas a pessoa deseja revisar?':null,model:'fixture-model',endpoint:'https://api.openai.com/v1',usage:{input_tokens:17,output_tokens:23},toolsExecuted:false,requiresHumanReview:true,availabilityVerified:false,notice:'Sugestão dentro das opções humanas; nenhuma operação preparada.'};
        const audit={id:proposal.id,requestId:proposal.requestId,assessmentId:assessment.id,contextDigest:proposal.contextDigest,allowedAdapters:payload.allowedAdapters,status:mode==='rejected'?'rejected':mode==='stale'?'stale':'accepted',model:proposal.model,endpoint:proposal.endpoint,createdAt:now,proposal:mode==='rejected'||mode==='stale'?undefined:proposal};requests.push({id:proposal.requestId,model:proposal.model,status:'completed',createdAt:now,inputTokens:17,outputTokens:23,unknownAcknowledged:false,resolution:null,operationProposal:audit});
        if(mode==='graph')Object.assign(proposal,{planChange:{kind:'review_then_observe',source:graphSource}});
        if(mode==='stale'){assessment.updatedAt=new Date(Date.now()+5000).toISOString();throw new Error('O contexto mudou durante a solicitação. Resposta e consumo foram preservados.');}
        if(mode==='rejected')throw new Error('A resposta não respeitou o esquema ou as opções permitidas. Histórico e consumo preservados.');
        return structuredClone(proposal);
      }
      if(mode==='graph'&&action==='work.plan.get')return {plan:null};
      if(mode==='graph'&&action==='work.plan.prepareSuggestion') {
        const nodes=[
          {id:'review',purpose:'Revisar a evidência',dependsOn:[],intent:{kind:'human_review',inputs:[{kind:'evidence',evidenceId:'eligible-evidence',expectedRawSha256:'eligible-hash'}]}},
          {id:'observe',purpose:'Observar padrões no código',dependsOn:['review'],intent:{kind:'native',adapter:'source.rules.v1',targetRef:null,ports:null}}];
        return {review:{id:'grant',digest:'reviewed-digest',expiresAt:new Date(Date.now()+60000).toISOString()},supersededJobIds:[],
          proposal:{id:'revision',number:1,previousRevisionId:null,assessmentId:assessment.id,suggestionSource:graphSource,
            body:{nodes,scopeRef:scope.ref,assessmentId:assessment.id,limits:{maxAttemptsTotal:2,maxCaptureBytesTotal:1048576}}}};
      }
      if(mode==='graph'&&action==='work.plan.commit')throw new Error('work_revision_changed: outra revisão foi salva');
      if(action==='assessment.sourcePlan'||action==='assessment.plan'){
        const plan={scopeRef:scope.ref,id:'prepared-host-plan',assessmentId:assessment.id,adapter:action==='assessment.sourcePlan'?'source.rules.v1':'local.inventory.v1',target:assessment.target,timeoutSeconds:10,outputLimitKiB:16,inputFiles:[{path:'reviewed.txt',size:100,sha256:'a'.repeat(64)}],sourceScope:action==='assessment.sourcePlan'?{version:1,rulesVersion:'2026-11-07.1',rootIdentity:{volume:'123',index:'456'},targetIsFile:false,bindings:[{path:'reviewed.txt',identity:{volume:'123',index:'789'}}],excludedDirectories:[],manifestSha256:'b'.repeat(64)}:null,resolvedAddresses:[],digest:'host-plan-digest',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),approvedAt:null,status:'proposed'};state.plans.push(plan);assessment.status='awaiting_approval';assessment.updatedAt=new Date(Date.now()+5000).toISOString();return structuredClone(plan);
      }
      throw new Error('Ação inesperada no contrato de sugestão: '+action);
    }};
  },{mode});
  await page.goto('/');await openAssessmentList(page);await page.getByText('Avaliação para sugestão',{exact:true}).click();await page.locator('.ai-operation-proposal>summary').click();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__proposalCalls:{action:string;payload:Record<string,unknown>}[]}).__proposalCalls.filter(call=>call.action===name),action);}
const consent='Autorizo enviar este contexto, objetivo e opções ao modelo e provedor exibidos para receber uma sugestão sem execução.';
async function prepare(page:Page,adapterLabel='Análise local de padrões textuais'){
  const panel=page.locator('.operation-proposal-panel');await panel.getByRole('checkbox',{name:adapterLabel,exact:true}).check();await panel.getByLabel('Objetivo da sugestão',{exact:true}).fill('Sugerir a próxima observação dentro do escopo.');await chooseAiData(panel);await panel.getByRole('button',{name:'Revisar contexto da sugestão',exact:true}).click();await panel.getByRole('checkbox',{name:consent}).check();return panel;
}
async function noExecution(page:Page){for(const action of ['plan.approve','plan.run','assessment.networkPlan','kali.mcp.scanEvidence','candidate.record'])expect(await calls(page,action)).toEqual([]);}

test('revisão sugerida exibe o diff, exige decisão e preserva um conflito de edição',async({page})=>{
  await fixture(page,'graph');
  await page.locator('.operation-proposal-panel').getByRole('checkbox',{name:/Evidência pequena/}).check();
  const panel=await prepare(page);
  await panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}).click();
  const compare=panel.getByRole('button',{name:'Comparar alteração sugerida no plano'});
  await compare.click();
  const save=panel.getByRole('button',{name:'Salvar alteração no plano'});
  await expect(save).toBeDisabled();await expect(panel).toContainText('Esta avaliação receberá seu primeiro plano.');
  await panel.getByRole('button',{name:'Fechar comparação'}).click();
  expect(await calls(page,'work.plan.commit')).toHaveLength(0);
  await compare.click();await panel.getByRole('checkbox',{name:/Revisei a evidência, a dependência/}).check();
  await save.click();await expect(panel.getByRole('alert')).toContainText('outra revisão foi salva');
  expect((await calls(page,'work.plan.prepareSuggestion')).map(r=>r.payload)).toEqual(Array(2).fill({assessmentId:'assessment-proposal',requestId:'proposal-request',proposalId:'operation-proposal'}));
  expect(await calls(page,'work.plan.commit')).toHaveLength(1);expect(await calls(page,'ai.operation.propose')).toHaveLength(1);
  for(const action of ['work.operation.prepare','work.operation.enqueue','work.review.commit'])expect(await calls(page,action)).toHaveLength(0);
});

test('sugestão exige contexto, opções e consentimento; plano real depende de novo clique e aprovação',async({page})=>{
  await fixture(page);const panel=page.locator('.operation-proposal-panel');await expect(panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true})).toBeDisabled();await expect(panel.getByRole('checkbox',{name:'Análise local de padrões textuais',exact:true})).not.toBeChecked();await prepare(page);await expect(panel).toContainText('https://api.openai.com/v1');expect(await calls(page,'ai.operation.propose')).toEqual([]);await panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}).click();const result=panel.getByLabel('Sugestão de operação',{exact:true});await expect(result).toContainText('A disponibilidade do adaptador ainda não foi verificada');await expect(result).toContainText('Entrada: 17 · saída: 23');expect((await calls(page,'ai.operation.propose'))[0].payload).toMatchObject({assessmentId:'assessment-proposal',evidenceIds:[],contextDigest:'proposal-context',message:'Sugerir a próxima observação dentro do escopo.',allowRemote:true,allowedAdapters:['source.rules.v1']});expect(await calls(page,'assessment.sourcePlan')).toEqual([]);await noExecution(page);await result.screenshot({path:test.info().outputPath('sugestao-operacao-revisada.png')});await result.getByRole('button',{name:'Revisar operação sugerida',exact:true}).click();await expect(page.getByRole('heading',{name:'Plano de execução',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toBeDisabled();expect((await calls(page,'assessment.sourcePlan'))[0].payload).toEqual({id:'assessment-proposal'});await noExecution(page);
});

test('mudanças de objetivo invalidam revisão e consultar histórico não repete chamada ou prepara plano',async({page})=>{
  await fixture(page);const panel=await prepare(page);await panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}).click();await panel.getByLabel('Objetivo da sugestão',{exact:true}).fill('Objetivo humano alterado.');await expect(panel.getByRole('button',{name:'Revisar operação sugerida',exact:true})).toBeDisabled();await expect(panel.getByRole('checkbox',{name:consent})).toHaveCount(0);await panel.getByRole('button',{name:'Atualizar histórico de sugestões',exact:true}).click();await panel.getByText('Histórico de sugestões',{exact:true}).click();await panel.getByRole('button',{name:'Consultar sugestão operation-proposal',exact:true}).click();await expect(panel.getByLabel('Sugestão de operação',{exact:true})).toContainText('Esta sugestão é histórica ou seu contexto/opções mudaram');await expect(panel.getByRole('button',{name:'Revisar operação sugerida',exact:true})).toBeDisabled();expect(await calls(page,'ai.operation.propose')).toHaveLength(1);expect(await calls(page,'assessment.sourcePlan')).toEqual([]);await noExecution(page);
});

for(const mode of ['rejected','stale'])test(`resposta ${mode} mantém consumo e histórico sem autoridade operacional`,async({page})=>{
  await fixture(page,mode);const panel=await prepare(page);await panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}).click();await expect(panel.getByRole('alert')).toContainText(mode==='rejected'?'não respeitou o esquema':'contexto mudou');await expect(panel.getByLabel('Registro da sugestão',{exact:true})).toContainText(mode==='rejected'?'Resposta recusada como sugestão':'Contexto desatualizado durante o envio');await expect(panel.getByLabel('Registro da sugestão',{exact:true})).toContainText('Entrada: 17 · saída: 23');await expect(panel.getByLabel('Sugestão de operação',{exact:true})).toHaveCount(0);await panel.getByRole('button',{name:'Atualizar histórico de sugestões',exact:true}).click();expect(await calls(page,'ai.operation.propose')).toHaveLength(1);expect(await calls(page,'assessment.plan')).toEqual([]);expect(await calls(page,'assessment.sourcePlan')).toEqual([]);await noExecution(page);
});

test('sugestão TCP abre formulário vazio e mantém portas e plano sob escolha humana',async({page})=>{
  await fixture(page,'network');const panel=await prepare(page,'Observação TCP com portas escolhidas pela pessoa');await expect(panel.getByRole('checkbox',{name:'Inventário local de arquivos',exact:true})).toHaveCount(0);await panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}).click();await panel.getByRole('button',{name:'Revisar operação sugerida',exact:true}).click();await expect(page.getByLabel('Portas TCP autorizadas',{exact:true})).toHaveValue('');await expect(page.getByLabel('Portas TCP autorizadas',{exact:true})).toBeFocused();await expect(page.getByRole('button',{name:'Preparar plano TCP',exact:true})).toBeDisabled();expect((await calls(page,'ai.operation.propose'))[0].payload.allowedAdapters).toEqual(['network.tcp-connect.v1']);await noExecution(page);
});

test('sugestão Kali exige evidência elegível e encaminha sem seleção ou transferência automáticas',async({page})=>{
  await fixture(page,'kali');const panel=page.locator('.operation-proposal-panel');const option=panel.getByRole('checkbox',{name:'Análise de uma evidência no Kali local',exact:true});await expect(option).toBeDisabled();await panel.getByRole('checkbox',{name:'Evidência acima de 32 KiB',exact:true}).check();await expect(option).toBeDisabled();await panel.getByRole('checkbox',{name:'Evidência acima de 32 KiB',exact:true}).uncheck();await panel.getByRole('checkbox',{name:'Evidência pequena',exact:true}).check();await prepare(page,'Análise de uma evidência no Kali local');await panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}).click();await panel.getByRole('button',{name:'Revisar operação sugerida',exact:true}).click();const kali=page.locator('.kali-bridge');await expect(kali).toHaveAttribute('open','');await expect(kali.getByLabel('Evidência para verificar no Kali',{exact:true})).toHaveValue('');await expect(kali.getByRole('checkbox',{name:'Autorizo transferir somente esta evidência revisada ao Kali local para a operação exibida.'})).toHaveCount(0);await expect(kali.getByRole('button',{name:'Analisar evidência no Kali',exact:true})).toBeDisabled();expect(await calls(page,'kali.mcp.hashEvidence')).toEqual([]);await noExecution(page);
});

test('prévia da sugestão não oferece envio ao provedor ou preparação de operações',async({page})=>{
  await page.goto('/');await page.evaluate(async()=>{const moduleUrl='/src/api.ts';const {request}=await import(moduleUrl);const p=await request('project.create',{name:'Prévia de sugestão',kind:'code',target:'D:/Fixture/preview',description:'Fixture'});await request('assessment.create',{projectId:p.id,title:'Sugestão indisponível na prévia',kind:p.kind,target:p.target,objective:'Verificar bloqueio.'});});await page.reload();await openAssessmentList(page);await page.getByText('Sugestão indisponível na prévia',{exact:true}).click();await page.locator('.ai-operation-proposal>summary').click();await expect(page.locator('.ai-operation-proposal')).toContainText('A prévia não envia contexto à IA nem prepara operações');await expect(page.getByRole('button',{name:'Solicitar sugestão revisada',exact:true})).toHaveCount(0);
});
