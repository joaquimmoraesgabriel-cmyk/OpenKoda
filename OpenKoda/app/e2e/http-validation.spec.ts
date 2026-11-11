import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';
import type { Snapshot, Finding, ExecutionPlan, Execution } from '../src/types';
import type { HttpFindingStatus, HttpObservation, HttpRetest, HttpRetestPreview } from '../src/http-validation';

const ids={assessment:'10000000-0000-4000-8000-000000000001',project:'10000000-0000-4000-8000-000000000002',finding:'10000000-0000-4000-8000-000000000003',base:'10000000-0000-4000-8000-000000000004',next:'10000000-0000-4000-8000-000000000005',plan:'10000000-0000-4000-8000-000000000006',nextPlan:'10000000-0000-4000-8000-000000000007',retest:'10000000-0000-4000-8000-000000000008'};

// Synthetic IPC contracts only: no HTTP server, target, native database or provider.
async function fixture(page:Page,mode='flow'){
  await installScopeFixture(page);
  await page.addInitScript(({ids,mode})=>{
    const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__contractCalls:{action:string;payload:Record<string,unknown>}[];__httpRestoreStatus:()=>void};
    w.isTauri=true;w.__contractCalls=[];
    const rule='http.x-content-type-options-nosniff.v1';const limits='Conclusão limitada à resposta HEAD, URL, destinos, peer e instante registrados. Não demonstra exploração nem cobertura do site inteiro. Nenhuma mudança remota é aplicada por esta revisão.';
    const time=(n:number)=>new Date(Date.now()-600000+n*1000).toISOString();const hash='a'.repeat(64);
    const target='https://http.fixture.test/recurso';let rev=1;let statusFailure=mode==='status-error';let hasValidation=['mismatch','missing','record-blocked','newer','confirm-stale','confirm-missing-id'].includes(mode);let validationRationale='Revisão humana da captura inicial.';let confirmation:HttpFindingStatus['confirmation']=null;const retests:HttpRetest[]=[];
    const assessment={id:ids.assessment,projectId:ids.project,title:'HTTP factual de contrato',kind:'web' as const,target,objective:'Revisar uma resposta HEAD autorizada.',status:'partial' as const,createdAt:time(0),updatedAt:time(5),events:[],evidence:[{id:'baseline-evidence',name:'Captura HEAD inicial',content:'{"schema":"openkoda.http-observation.v2"}',sha256:hash,createdAt:time(5)}],coverage:[],summary:'Uma resposta limitada registrada.'};
    const makePlan=(next=false):ExecutionPlan=>({id:next?ids.nextPlan:ids.plan,assessmentId:ids.assessment,adapter:'http.headers.v1',target,timeoutSeconds:10,outputLimitKiB:16,inputFiles:[],resolvedAddresses:['192.0.2.20:443'],digest:next?'new-plan-digest':'baseline-plan-digest',createdAt:time(next?30:1),expiresAt:new Date(Date.now()+600000).toISOString(),approvedAt:time(next?31:2),status:'completed'});
    const makeExecution=(next=false):Execution=>({id:next?ids.next:ids.base,planId:next?ids.nextPlan:ids.plan,assessmentId:ids.assessment,status:'completed',startedAt:time(next?32:3),finishedAt:time(next?35:5),evidenceId:next?'next-evidence':'baseline-evidence',message:next?'Nova observação limitada de contrato.':'Observação inicial de contrato.'});
    const finding:Finding={id:ids.finding,assessmentId:ids.assessment,title:'Resposta HEAD sem X-Content-Type-Options',severity:'low',confidence:'Fato da resposta observada; impacto não determinado',status:hasValidation?'validated':'candidate',description:'Ausência observada em uma resposta HEAD 200. Não demonstra exploração.',recommendation:'Revisar a configuração e observar uma nova resposta após aprovação.',evidenceIds:['baseline-evidence','claim-evidence'],createdAt:time(10)};
    const state:Snapshot={projects:[{id:ids.project,name:'HTTP contrato',kind:'web',target,description:'Fixture isolada',createdAt:time(0)}],assessments:[assessment],findings:mode==='flow'||['present','legacy','duplicate','nontext','cross-id'].includes(mode)?[]:[finding],validations:[],plans:[makePlan()],executions:[makeExecution()],settings:{provider:'openai',endpoint:'https://api.fixture.test/v1',model:'fixture',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false}};
    const scope=window.__scopeFixture(state,['http.headers.v1']);
    const a=state.assessments[0];
    if(hasValidation){state.plans.push(makePlan(true));state.executions.push(makeExecution(true));}
    if(mode==='forged'){finding.description='{"schema":"openkoda.http-claim.v1","ruleId":"http.x-content-type-options-nosniff.v1"}';a.evidence[0].content='{"schema":"openkoda.manual-evidence.v1","text":"openkoda.http-claim.v1"}';}
    function observation(next=false):HttpObservation{
      const outcome=next?(mode==='missing'?'control_missing':'control_present'):mode==='present'?'control_present':['legacy','duplicate','nontext'].includes(mode)?'inconclusive':'control_missing';
      const reasons=next?[]:mode==='legacy'?['legacy_capture']:mode==='duplicate'?['duplicate_header']:mode==='nontext'?['nontext_header']:[];
      return {executionId:next?ids.next:ids.base,planId:next?ids.nextPlan:ids.plan,planDigest:next?'new-plan-digest':'baseline-plan-digest',assessmentId:ids.assessment,projectId:ids.project,target,executionStatus:'completed',startedAt:time(next?32:3),finishedAt:time(next?35:5),approvedAt:time(next?31:2),planCreatedAt:time(next?30:1),evidenceId:next?'next-evidence':'baseline-evidence',evidenceSha256:hash,schema:mode==='legacy'&&!next?null:'openkoda.http-observation.v2',statusCode:200,resolvedAddresses:['192.0.2.20:443'],remoteAddress:mode==='mismatch'&&next?'192.0.2.21:443':'192.0.2.20:443',headerValues:mode==='nontext'&&!next?[{text:null,bytes:1,sha256:hash}]:mode==='duplicate'&&!next?[{text:'nosniff',bytes:7,sha256:hash},{text:'nosniff',bytes:7,sha256:hash}]:outcome==='control_present'?[{text:'nosniff',bytes:7,sha256:hash}]:[],outcome,reasons};
    }
    const comparison=()=>({differentExecution:true,differentPlan:true,afterValidation:true,sameTarget:true,sameResolvedAddresses:true,samePeer:mode!=='mismatch'});
    function retestPreview():HttpRetestPreview{return {findingId:ids.finding,observation:observation(true),comparison:comparison(),outcome:mode==='mismatch'?'inconclusive':observation(true).outcome,reviewDigest:'retest-review-'+rev,canRecord:mode!=='record-blocked'&&!retests.length,reasons:mode==='record-blocked'?['predates_validation']:[],factReasons:mode==='mismatch'?['changed_peer']:[],existingRetestEvidenceId:retests[0]?.evidenceId??null,limits};}
    const makeRetest=():HttpRetest=>({evidenceId:ids.retest,sha256:hash,createdAt:time(40),executionId:ids.next,observation:observation(true),comparison:comparison(),outcome:retestPreview().outcome,reasons:retestPreview().factReasons});
    if(['newer','confirm-stale','confirm-missing-id'].includes(mode))retests.push(makeRetest());
    function httpStatus():HttpFindingStatus{
      const canConfirm=finding.status==='validated'&&retests.length>0&&retests[retests.length-1].outcome==='control_present'&&mode!=='newer';
      return {supported:true,findingId:ids.finding,ruleId:rule,findingStatus:finding.status,stateDigest:'state-'+rev,claim:{evidenceId:'claim-evidence',sha256:hash,createdAt:time(10)},baseline:observation(),validation:hasValidation?{evidenceId:'validation-evidence',sha256:hash,recordId:'validation-record',createdAt:time(20),rationale:validationRationale}:null,retests:structuredClone(retests),confirmation,actions:{canValidate:finding.status==='candidate'&&!hasValidation,canRetest:finding.status==='validated',canConfirm,confirmRetestEvidenceId:canConfirm&&mode!=='confirm-missing-id'?ids.retest:null,reasons:mode==='newer'?['newer_unreviewed_execution']:[]},availableExecutions:hasValidation?state.executions.map(e=>({executionId:e.id,status:e.status,startedAt:e.startedAt,finishedAt:e.finishedAt,canReview:e.id!==ids.base,reasons:e.id===ids.base?['same_execution','same_plan','predates_validation']:[]})):[],limits};
    }
    function change(){rev++;a.updatedAt=time(40+rev);}
    w.__httpRestoreStatus=()=>{statusFailure=false;};
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected IPC');const {action,payload}=args.request;w.__contractCalls.push({action,payload});
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture offline',tools:[],executorReady:false};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='kali.mcp.status')return {active:null,recent:[],hasUnresolved:false,capabilities:[],executorReady:false};
      if(action==='ai.status')return {hasApiKey:false,verified:false,checkedAt:null,model:'fixture',endpoint:state.settings.endpoint};
      if(action==='ai.history')return {messages:[],requests:[]};
      if(action==='ai.investigation.list')return {items:[],limit:500};
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Fixture sem provedor.'};
      if(action==='finding.httpStatus'){if(statusFailure)throw new Error('Integridade da prova não confirmada.');return mode==='forged'?{supported:false,findingId:ids.finding}:structuredClone(httpStatus());}
      if(action==='finding.httpPreview'){const o=observation();if(mode==='cross-id')o.executionId=ids.next;return {ruleId:rule,observation:o,reviewDigest:'candidate-review',canCreateCandidate:o.outcome==='control_missing'&&!state.findings.length,existingFindingId:state.findings.length?ids.finding:null,reasons:o.reasons,limits};}
      if(action==='finding.httpCandidate'){if(payload.reviewDigest!=='candidate-review'||payload.acknowledgeLimits!==true)throw new Error('Revisão exata exigida');const created=!state.findings.length;if(created){state.findings.push(finding);change();}return {created,finding:structuredClone(finding),status:structuredClone(httpStatus())};}
      if(action==='finding.httpValidate'){if(payload.stateDigest!=='state-'+rev||payload.acknowledgeLimits!==true)throw new Error('Revisão mudou');hasValidation=true;finding.status='validated';validationRationale=String(payload.rationale);finding.evidenceIds.push('validation-evidence');change();return structuredClone(httpStatus());}
      if(action==='assessment.start'){a.status='awaiting_approval';const p={...makePlan(true),scopeRef:scope.ref};p.status='proposed';p.approvedAt=null;state.plans.push(p);change();return structuredClone(p);}
      if(action==='plan.approve'){const p=state.plans[state.plans.length-1];if(payload.digest!==p.digest)throw new Error('Digest divergente');p.status='approved';p.approvedAt=time(31);return structuredClone(p);}
      if(action==='plan.run'){const p=state.plans[state.plans.length-1];if(p.status!=='approved'||payload.digest!==p.digest)throw new Error('Aprovação exigida');p.status='completed';state.executions.push(makeExecution(true));a.status='partial';a.evidence.push({id:'next-evidence',name:'Nova HEAD de contrato',content:'nosniff',sha256:hash,createdAt:time(35)});change();return structuredClone(state.executions[state.executions.length-1]);}
      if(action==='finding.httpRetestPreview')return structuredClone(retestPreview());
      if(action==='finding.httpRetest'){if(payload.reviewDigest!=='retest-review-'+rev)throw new Error('Comparação mudou');const recorded=!retests.length;if(recorded){retests.push(makeRetest());finding.evidenceIds.push(ids.retest);change();}return {recorded,retest:structuredClone(retests[0]),status:structuredClone(httpStatus())};}
      if(action==='finding.httpConfirm'){if(mode==='confirm-stale'){rev++;throw new Error('A revisão HTTP mudou. Atualize os fatos e revise novamente.');}if(payload.retestEvidenceId!==ids.retest||payload.stateDigest!=='state-'+rev||payload.acknowledgeLimits!==true)throw new Error('Prova ou revisão divergente');finding.status='fixed';confirmation={evidenceId:'confirmation-evidence',sha256:hash,recordId:'confirmation-record',createdAt:time(50),retestEvidenceId:ids.retest,rationale:String(payload.rationale)};finding.evidenceIds.push('confirmation-evidence');change();return structuredClone(httpStatus());}
      throw new Error('Ação não prevista na fixture HTTP: '+action);
    }};
  },{ids,mode});
  await page.goto('/?area=tecnica');
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__contractCalls:{action:string;payload:Record<string,unknown>}[]}).__contractCalls.filter(c=>c.action===name),action);}
async function openAssessment(page:Page){await page.getByRole('button',{name:'Avaliações',exact:true}).click();await page.getByText('HTTP factual de contrato',{exact:true}).first().click();}
async function openFinding(page:Page){await page.getByRole('navigation',{name:'Navegação principal',exact:true}).getByRole('button',{name:'Achados',exact:true}).click();await page.getByRole('button',{name:/Resposta HEAD sem X-Content-Type-Options/}).click();}
async function openReview(page:Page){await openAssessment(page);await page.locator('summary').filter({hasText:'Revisar controle HTTP'}).click();await page.getByLabel('Execução HTTP para revisar',{exact:true}).selectOption(ids.base);await page.getByRole('button',{name:'Revisar observação HTTP',exact:true}).click();}
async function openRetest(page:Page){await openFinding(page);await page.getByRole('button',{name:'Correção',exact:true}).click();}
async function previewRetest(page:Page){await page.getByLabel('Execução para comparar no reteste',{exact:true}).selectOption(ids.next);await page.getByRole('button',{name:'Comparar observação do reteste',exact:true}).click();}

test('HTTP factual exige revisões independentes do candidato, da nova HEAD e da confirmação',async({page})=>{
  await fixture(page);await openReview(page);
  const create=page.getByRole('button',{name:'Registrar candidato desta observação',exact:true});await expect(create).toBeDisabled();expect(await calls(page,'finding.httpCandidate')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
  await page.getByRole('checkbox',{name:/Revisei a observação e seus limites/}).check();await create.click();await page.getByRole('button',{name:'Abrir candidato registrado',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Alegação factual HTTP',exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Validar candidato',exact:true})).toHaveCount(0);
  await page.getByLabel('Justificativa da validação HTTP',{exact:true}).fill('A captura 200 completa não lista este cabeçalho; impacto não determinado.');await expect(page.getByRole('button',{name:'Confirmar validação factual',exact:true})).toBeDisabled();await page.getByRole('checkbox',{name:/Confirmo a alegação limitada/}).check();await page.getByRole('button',{name:'Confirmar validação factual',exact:true}).click();await expect(page.getByRole('heading',{name:'Validação humana registrada',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Correção',exact:true}).click();await page.getByRole('button',{name:'Abrir avaliação para nova observação',exact:true}).click();expect(await calls(page,'assessment.start')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
  await page.getByRole('button',{name:'Preparar outra operação',exact:true}).click();await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toBeDisabled();await page.getByRole('checkbox',{name:/Revisei o alvo, os efeitos/}).check();await page.getByRole('button',{name:'Aprovar plano',exact:true}).click();expect(await calls(page,'plan.run')).toEqual([]);await page.getByRole('button',{name:'Executar plano aprovado',exact:true}).click();
  await openRetest(page);await previewRetest(page);await expect(page.getByRole('button',{name:'Registrar conclusão deste reteste',exact:true})).toBeDisabled();await page.getByRole('checkbox',{name:/Revisei a comparação e registro/}).check();await page.getByRole('button',{name:'Registrar conclusão deste reteste',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Confirmar controle no reteste',exact:true})).toBeVisible();await expect(page.locator('.detail-heading')).toContainText('Validado');expect(await calls(page,'finding.httpConfirm')).toEqual([]);
  await page.getByLabel('Justificativa da confirmação HTTP',{exact:true}).fill('A nova resposta comparável contém exatamente um nosniff. Limite: somente esta HEAD.');await page.getByRole('checkbox',{name:/Revisei o último reteste e confirmo/}).check();await page.getByRole('button',{name:'Confirmar controle observado no reteste',exact:true}).click();
  await expect(page.locator('.detail-heading')).toContainText('Corrigido');await expect(page.getByRole('region',{name:'Confirmação HTTP registrada',exact:true})).toContainText('não afirma que o site está seguro');
  expect((await calls(page,'finding.httpCandidate'))[0].payload).toEqual({executionId:ids.base,ruleId:'http.x-content-type-options-nosniff.v1',reviewDigest:'candidate-review',acknowledgeLimits:true});expect((await calls(page,'finding.httpConfirm'))[0].payload).toMatchObject({findingId:ids.finding,retestEvidenceId:ids.retest,acknowledgeLimits:true});expect(await calls(page,'finding.validate')).toEqual([]);expect(await calls(page,'ai.ask')).toEqual([]);expect(await calls(page,'remediation.apply')).toEqual([]);
  await page.getByRole('button',{name:'Histórico',exact:true}).click();await expect(page.getByRole('heading',{name:'Histórico factual HTTP',exact:true})).toBeVisible();await expect(page.getByText('claim-evidence',{exact:false}).first()).toBeVisible();await page.screenshot({path:test.info().outputPath('http-confirmacao-limitada.png'),fullPage:true});
});

for(const [mode,label,reason] of [['present','Valor nosniff observado',''],['legacy','Observação inconclusiva','A captura antiga'],['duplicate','Observação inconclusiva','ocorrências repetidas'],['nontext','Observação inconclusiva','não textual']])test(`HTTP ${mode} não vira candidato por ausência`,async({page})=>{
  await fixture(page,mode);await openReview(page);const review=page.getByRole('region',{name:'Revisão do candidato HTTP',exact:true});await expect(review).toContainText(label);if(reason)await expect(review).toContainText(reason);await expect(page.getByRole('button',{name:'Registrar candidato desta observação',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpCandidate')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
});

test('revisão recusa retorno de outra execução e não registra candidato',async({page})=>{
  await fixture(page,'cross-id');await openReview(page);await expect(page.locator('.http-candidate-review').getByRole('alert')).toContainText('origem ou o estado');await expect(page.getByRole('button',{name:'Registrar candidato desta observação',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpCandidate')).toEqual([]);
});

test('erro de integridade bloqueia validação sem converter o achado em genérico',async({page})=>{
  await fixture(page,'status-error');await openFinding(page);await expect(page.getByRole('region',{name:'Consulta da origem do achado',exact:true})).toContainText('Integridade da prova não confirmada');await expect(page.getByRole('button',{name:'Validar candidato',exact:true})).toHaveCount(0);await expect(page.getByRole('button',{name:'Confirmar validação factual',exact:true})).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {__httpRestoreStatus:()=>void}).__httpRestoreStatus());await page.getByRole('button',{name:'Atualizar revisão HTTP',exact:true}).click();await expect(page.getByRole('heading',{name:'Alegação factual HTTP',exact:true})).toBeVisible();expect(await calls(page,'finding.httpValidate')).toEqual([]);
});

test('JSON importado e título semelhante não concedem alegação HTTP tipada',async({page})=>{
  await fixture(page,'forged');await openFinding(page);await expect(page.getByRole('button',{name:'Validar candidato',exact:true})).toBeVisible();await expect(page.getByRole('heading',{name:'Alegação factual HTTP',exact:true})).toHaveCount(0);await expect(page.getByRole('button',{name:'Confirmar validação factual',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpValidate')).toEqual([]);
});

for(const mode of ['mismatch','missing'])test(`reteste ${mode} preserva conclusão sem autorizar confirmação`,async({page})=>{
  await fixture(page,mode);await openRetest(page);await previewRetest(page);const review=page.getByRole('region',{name:'Revisão do reteste HTTP',exact:true});await expect(review).toContainText(mode==='mismatch'?'O peer real do reteste difere':'Cabeçalho ausente');await page.getByRole('checkbox',{name:/Revisei a comparação e registro/}).check();await page.getByRole('button',{name:'Registrar conclusão deste reteste',exact:true}).click();await expect(page.getByRole('heading',{name:'Conclusões registradas',exact:true})).toBeVisible();await expect(page.locator('.detail-heading')).toContainText('Validado');await expect(page.getByRole('button',{name:'Confirmar controle observado no reteste',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpConfirm')).toEqual([]);
});

test('impedimento cronológico e nova HEAD não revisada bloqueiam ações distintas',async({page})=>{
  await fixture(page,'record-blocked');await openRetest(page);await previewRetest(page);await expect(page.getByRole('region',{name:'Impedimentos para registrar reteste',exact:true})).toContainText('não é posterior');await expect(page.getByRole('button',{name:'Registrar conclusão deste reteste',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpRetest')).toEqual([]);
});

test('HEAD posterior não revisada e reteste positivo não autorizam fixed',async({page})=>{
  await fixture(page,'newer');await openRetest(page);await expect(page.locator('.http-finding-panel')).toContainText('observação HEAD posterior ainda não revisada');await expect(page.getByRole('button',{name:'Confirmar controle observado no reteste',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpConfirm')).toEqual([]);
});

test('reteste a confirmar exige ID explícito do núcleo',async({page})=>{
  await fixture(page,'confirm-missing-id');await openRetest(page);await expect(page.locator('.http-finding-panel')).toContainText('identificação do reteste para confirmação está incompleta');await expect(page.getByRole('button',{name:'Confirmar controle observado no reteste',exact:true})).toHaveCount(0);expect(await calls(page,'finding.httpConfirm')).toEqual([]);
});

test('revisão vencida na confirmação exige nova leitura e novo consentimento sem repetir',async({page})=>{
  await fixture(page,'confirm-stale');await openRetest(page);await page.getByLabel('Justificativa da confirmação HTTP',{exact:true}).fill('Controle observado no reteste indicado.');await page.getByRole('checkbox',{name:/Revisei o último reteste e confirmo/}).check();await page.getByRole('button',{name:'Confirmar controle observado no reteste',exact:true}).click();await expect(page.locator('.http-finding-panel').getByRole('alert')).toContainText('revisão HTTP mudou');await expect(page.getByRole('checkbox',{name:/Revisei o último reteste e confirmo/})).not.toBeChecked();await expect(page.getByRole('button',{name:'Confirmar controle observado no reteste',exact:true})).toBeDisabled();await page.getByRole('button',{name:'Atualizar revisão HTTP',exact:true}).click();expect(await calls(page,'finding.httpConfirm')).toHaveLength(1);await expect(page.locator('.detail-heading')).toContainText('Validado');
});

test('candidato existente abre registro sem permitir duplicação pela interface',async({page})=>{
  await fixture(page,'candidate');await openReview(page);await expect(page.getByRole('button',{name:'Registrar candidato desta observação',exact:true})).toHaveCount(0);await page.getByRole('button',{name:'Abrir registro existente',exact:true}).click();await expect(page.getByRole('heading',{name:'Alegação factual HTTP',exact:true})).toBeVisible();expect(await calls(page,'finding.httpCandidate')).toEqual([]);
});
