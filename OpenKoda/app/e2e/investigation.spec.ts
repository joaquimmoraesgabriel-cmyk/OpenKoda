import { openAssessmentList } from './navigation';
import { chooseAiData, reviewKaliData } from './data-fixture';
import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Synthetic IPC only: no provider, credential or paid inference is used.
async function fixture(page:Page,mode='success'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__investigationCalls:{action:string;payload:Record<string,unknown>}[];__finishInvestigation:()=>void};
    w.isTauri=true;w.__investigationCalls=[];
    const assessment={id:'assessment-investigation',projectId:'project-investigation',title:'Investigação de contrato',kind:'files',target:'D:/Fixture',objective:'Comparar interpretações sobre o mesmo trecho.',status:'partial',createdAt:now,updatedAt:now,events:[],evidence:[{id:'evidence-imported',name:'Texto declarado de contrato',content:'user_supplied: trecho literal',sha256:'fixture-evidence-hash',createdAt:now}],coverage:[],summary:'Observação limitada.'};
    const settings={provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture-model',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false};
    const snapshot={projects:[{id:assessment.projectId,name:'Projeto de contrato',kind:assessment.kind,target:assessment.target,description:'Somente fixture',createdAt:now}],assessments:[assessment],findings:[],plans:[],executions:[],validations:[],settings};
    const scope=window.__scopeFixture(snapshot,['ai.remote-review.v1']);
    let plan:Record<string,unknown>|undefined;let finish:((value:unknown)=>void)|undefined;let sequence=0;
    const makeResult=(perspective:string,index:number)=>({perspective,requestId:'fixture-request-'+index,status:'completed',text:perspective==='counterevidence'?'Há uma explicação alternativa; a hipótese não está confirmada.':'O trecho sustenta uma hipótese limitada que exige validação.',usage:{input_tokens:100,output_tokens:40},error:null,startedAt:now,finishedAt:now});
    w.__finishInvestigation=()=>{if(plan){plan.status='canceled';plan.results=[makeResult('evidence',0)];plan.knownTokens=140;plan.usageUnknown=false;plan.outcomeNote='Cancelamento observado entre chamadas. A chamada enviada terminou; não há confirmação de estorno.';finish?.(structuredClone(plan));}};
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__investigationCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(snapshot);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture sem ambiente.',tools:[],executorReady:false};
      if(action==='ai.history')return {messages:[]};
      if(action==='ai.investigation.list')return {items:plan?[structuredClone(plan)]:[],limit:500};
      if(action==='ai.context')return {context:{assessment:{id:assessment.id,target:assessment.target,objective:assessment.objective},evidence:(payload.evidenceIds as string[]).includes('evidence-imported')?assessment.evidence:[]},digest:'context-investigation-digest',bytes:256,notice:'Este contexto será enviado somente após revisão do plano.'};
      if(action==='ai.investigation.plan'){
        if(mode==='budget-error')throw new Error('Orçamento abaixo da reserva conservadora; reduza o contexto ou revise o limite.');
        if(payload.contextDigest!=='context-investigation-digest')throw new Error('Contexto divergente.');
        const roles=payload.perspectives as string[];
        plan={scopeRef:scope.ref,id:'investigation-'+(++sequence),digest:'investigation-digest-'+sequence,status:'prepared',assessmentId:assessment.id,evidenceIds:payload.evidenceIds,contextDigest:payload.contextDigest,context:{selectedEvidence:payload.evidenceIds,target:assessment.target,literal:'Contexto exato do plano de teste.'},message:payload.message,perspectives:roles,endpoint:'https://fixture.example.test/v1',model:'fixture-model',credentialRevision:'non-secret-fixture-revision',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),budget:{maxRequests:roles.length,maxOutputTokensPerRequest:payload.maxOutputTokensPerRequest,maxTotalTokens:payload.maxTotalTokens,conservativeReservationPerRequest:4096,maxContextBytes:65536},results:[],outcomeNote:'',reconciledFrom:null,knownTokens:0,usageUnknown:false,toolsExecuted:false,maxConcurrentRequests:1,notice:'Perspectivas sequenciais sem respostas cruzadas. Reserva estimada não é preço nem teto financeiro.'};return structuredClone(plan);
      }
      if(action==='ai.investigation.run'){
        if(payload.id!==plan?.id||payload.digest!==plan?.digest||payload.allowRemote!==true)throw new Error('Plano ou consentimento divergente.');
        plan!.status='running';
        if(mode==='cancel'){plan!.results=[{perspective:'evidence',requestId:'fixture-pending',status:'pending',text:'',usage:null,error:null,startedAt:now,finishedAt:null}];plan!.usageUnknown=true;return new Promise(resolve=>{finish=resolve;});}
        if(mode==='unknown'){plan!.status='unknown';plan!.results=[{perspective:'evidence',requestId:'fixture-unknown',status:'unknown',text:'',usage:null,error:'Resposta não confirmada.',startedAt:now,finishedAt:null}];plan!.usageUnknown=true;plan!.outcomeNote='Nenhuma chamada será repetida automaticamente.';throw new Error('Falha após envio; consumo não confirmado.');}
        plan!.status='completed';plan!.results=(plan!.perspectives as string[]).map(makeResult);plan!.knownTokens=(plan!.perspectives as string[]).length*140;plan!.outcomeNote='Divergências preservadas. Nenhum candidato criado automaticamente.';return structuredClone(plan);
      }
      if(action==='ai.investigation.cancel'){
        if(payload.id!==plan?.id)throw new Error('Investigação divergente.');
        if(plan!.status==='prepared'){plan!.status='canceled';return structuredClone(plan);}
        plan!.status='canceling';return {id:plan!.id,requested:true,status:'canceling',providerCancellationConfirmed:false};
      }
      if(action==='ai.investigation.reconcile'){
        if(payload.id!==plan?.id||payload.acknowledgeUnknownUsage!==true||!String(payload.note).trim())throw new Error('Revisão da incerteza exigida.');
        plan!.status='reconciled';plan!.reconciledFrom='unknown';plan!.outcomeNote='Registro encerrado localmente. Resultado remoto e cobrança continuam desconhecidos.';return structuredClone(plan);
      }
      throw new Error('Ação não prevista na investigação de teste: '+action);
    }};
  },{mode});
  await page.goto('/');await openAssessmentList(page);await page.getByText('Investigação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:/^Perspectivas independentes de IA$/}).click();
  await expect(page.locator('.ai-investigation')).toBeVisible();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__investigationCalls:{action:string;payload:Record<string,unknown>}[]}).__investigationCalls.filter(call=>call.action===name),action);}
async function prepare(page:Page,two=true){
  const panel=page.locator('.ai-investigation');await panel.getByRole('checkbox',{name:'Texto declarado de contrato',exact:true}).check();
  await panel.getByLabel('Objetivo da investigação',{exact:true}).fill('Confronte o que o trecho sustenta com explicações alternativas.');
  if(two)await panel.getByRole('checkbox',{name:'Contraevidências e alternativas',exact:true}).check();await chooseAiData(panel);await panel.getByRole('button',{name:'Revisar contexto da investigação',exact:true}).click();
  await panel.getByRole('button',{name:'Preparar investigação para revisão',exact:true}).click();
}
async function run(page:Page){
  const panel=page.locator('.ai-investigation');await panel.getByRole('checkbox',{name:'Autorizo o envio deste contexto e objetivo ao modelo e provedor mostrados, nas perspectivas e limites deste plano.'}).check();
  await panel.getByRole('button',{name:'Enviar investigação revisada',exact:true}).click();
}

test('investigação exige plano visível e consentimento e conserva respostas independentes',async({page})=>{
  await fixture(page);const panel=page.locator('.ai-investigation');
  expect(await calls(page,'ai.investigation.plan')).toEqual([]);expect(await calls(page,'ai.investigation.run')).toEqual([]);
  await prepare(page);
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('fixture-model');await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('https://fixture.example.test/v1');
  await expect(panel.getByLabel('Representação do contexto do plano da investigação')).toContainText('evidence-imported');
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('não é preço nem teto de faturamento');
  await expect(panel.getByRole('button',{name:'Enviar investigação revisada',exact:true})).toBeDisabled();
  expect((await calls(page,'ai.investigation.plan'))[0].payload).toMatchObject({evidenceIds:['evidence-imported'],contextDigest:'context-investigation-digest',perspectives:['evidence','counterevidence'],maxOutputTokensPerRequest:512,maxTotalTokens:10000});
  await panel.screenshot({path:test.info().outputPath('investigacao-plano.png')});
  await run(page);
  const results=panel.getByRole('region',{name:'Resultados separados por perspectiva'});
  await expect(results.getByRole('heading',{name:'Evidências e hipóteses',exact:true})).toBeVisible();await expect(results.getByRole('heading',{name:'Contraevidências e alternativas',exact:true})).toBeVisible();
  await expect(results).toContainText('hipótese limitada');await expect(results).toContainText('explicação alternativa');
  expect((await calls(page,'ai.investigation.run'))[0].payload).toMatchObject({id:'investigation-1',digest:'investigation-digest-1',allowRemote:true});
  expect(await calls(page,'candidate.record')).toEqual([]);expect(await calls(page,'finding.validate')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
});

test('alterar objetivo invalida o consentimento do plano anterior da investigação',async({page})=>{
  await fixture(page);await prepare(page,false);const panel=page.locator('.ai-investigation');
  await panel.getByRole('checkbox',{name:'Autorizo o envio deste contexto e objetivo ao modelo e provedor mostrados, nas perspectivas e limites deste plano.'}).check();
  await panel.getByLabel('Objetivo da investigação',{exact:true}).fill('Objetivo alterado exige outra revisão.');
  await expect(panel.getByRole('button',{name:'Enviar investigação revisada',exact:true})).toHaveCount(0);
  expect(await calls(page,'ai.investigation.run')).toEqual([]);
});

test('investigação não eleva orçamento automaticamente após recusa da reserva',async({page})=>{
  await fixture(page,'budget-error');const panel=page.locator('.ai-investigation');
  await panel.getByLabel('Objetivo da investigação',{exact:true}).fill('Analisar o trecho.');
  await panel.getByLabel('Limiar total de tokens',{exact:true}).fill('100');await expect(panel.getByRole('button',{name:'Preparar investigação para revisão',exact:true})).toBeDisabled();
  expect(await calls(page,'ai.investigation.plan')).toEqual([]);
  await panel.getByLabel('Limiar total de tokens',{exact:true}).fill('1024');await chooseAiData(panel);await panel.getByRole('button',{name:'Revisar contexto da investigação',exact:true}).click();await panel.getByRole('button',{name:'Preparar investigação para revisão',exact:true}).click();
  await expect(panel.getByRole('alert')).toContainText('Orçamento abaixo da reserva');await expect(panel.getByLabel('Limiar total de tokens',{exact:true})).toHaveValue('1024');
  expect(await calls(page,'ai.investigation.plan')).toHaveLength(1);expect(await calls(page,'ai.investigation.run')).toEqual([]);
});

test('cancelamento da investigação espera término local e não confirma cancelamento no provedor',async({page})=>{
  await fixture(page,'cancel');await prepare(page);await run(page);const panel=page.locator('.ai-investigation');
  await panel.getByRole('button',{name:'Cancelar próximos envios',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Cancelamento solicitado',exact:true})).toBeDisabled();
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('cancelamento no provedor não está confirmado');
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).not.toContainText('Cancelada');
  await page.evaluate(()=>(window as unknown as {__finishInvestigation:()=>void}).__finishInvestigation());
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('Cancelada');await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('Sem resultado registrado: Contraevidências e alternativas');
  expect(await calls(page,'ai.investigation.run')).toHaveLength(1);expect(await calls(page,'ai.investigation.cancel')).toHaveLength(1);
});

test('investigação com consumo desconhecido não repete chamada e exige reconciliação explícita',async({page})=>{
  await fixture(page,'unknown');await prepare(page,false);await run(page);const panel=page.locator('.ai-investigation');
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('Resultado ou consumo desconhecido');
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('A soma conhecida não representa o consumo total');
  await expect(panel.getByRole('button',{name:'Preparar investigação para revisão',exact:true})).toBeDisabled();
  await panel.getByRole('button',{name:'Atualizar investigações',exact:true}).click();await expect(panel.getByRole('button',{name:'Atualizar investigações',exact:true})).toBeEnabled();
  expect(await calls(page,'ai.investigation.run')).toHaveLength(1);expect(await calls(page,'ai.investigation.reconcile')).toEqual([]);
  await panel.getByLabel('Nota da reconciliação da investigação',{exact:true}).fill('Verificado registro local; consumo remoto continua sem confirmação.');
  await expect(panel.getByRole('button',{name:'Encerrar registro após revisão',exact:true})).toBeDisabled();
  await panel.getByRole('checkbox',{name:'Reconheço que o resultado remoto e a cobrança podem continuar desconhecidos. Encerrar o registro não repete nem estorna chamadas.'}).check();
  await panel.getByRole('button',{name:'Encerrar registro após revisão',exact:true}).click();
  await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('Registro reconciliado');await expect(panel.getByLabel('Plano da investigação',{exact:true})).toContainText('cobrança continuam desconhecidos');
  expect((await calls(page,'ai.investigation.reconcile'))[0].payload).toMatchObject({id:'investigation-1',acknowledgeUnknownUsage:true});expect(await calls(page,'ai.investigation.run')).toHaveLength(1);
});

test('prévia da investigação não oferece envio de contexto ao provedor',async({page})=>{
  await page.goto('/');await page.evaluate(async()=>{const path='/src/api.ts';const {request}=await import(path);const project=await request('project.create',{name:'Projeto de prévia',kind:'files',target:'D:/Preview',description:''});await request('assessment.create',{projectId:project.id,title:'Perspectivas na prévia',kind:project.kind,target:project.target,objective:'Verificar limites.'});});
  await page.reload();await openAssessmentList(page);await page.getByText('Perspectivas na prévia',{exact:true}).click();await page.locator('summary').filter({hasText:/^Perspectivas independentes de IA$/}).click();
  await expect(page.getByText('A investigação por perspectivas está disponível no aplicativo desktop. A prévia não envia contexto à IA nem executa ferramentas.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Revisar contexto da investigação',exact:true})).toHaveCount(0);
});
