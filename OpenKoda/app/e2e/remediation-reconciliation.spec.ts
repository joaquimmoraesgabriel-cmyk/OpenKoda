import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Only the UI/IPC contract is simulated. These fixtures never touch target files.
async function fixture(page:Page,mode='replacement'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const observedAt='2026-11-06T12:34:56.000Z';
    const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__reconcileCalls:{action:string;payload:Record<string,unknown>}[]};w.isTauri=true;w.__reconcileCalls=[];
    const assessment={id:'assessment-reconcile',projectId:'project-reconcile',title:'Avaliação com correção interrompida',kind:'code',target:'D:/Fixture/source',objective:'Fixture',status:'partial',createdAt:now,updatedAt:now,events:[],evidence:[],coverage:[],summary:''};
    const state={projects:[{id:assessment.projectId,name:'Projeto de contrato',kind:'code',target:assessment.target,description:'Fixture',createdAt:now}],assessments:[assessment],findings:[{id:'finding-reconcile',assessmentId:assessment.id,title:'Achado com alteração interrompida',severity:'low',confidence:'Revisado',status:'validated',description:'Fixture',recommendation:'Inspecionar os limites.',evidenceIds:[],createdAt:now}],settings:{provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false},plans:[],executions:[],validations:[]};
    const scope=window.__scopeFixture(state,['local.text-remediation.v1']);
    const plan:any={id:'remediation-interrupted',projectId:assessment.projectId,assessmentId:assessment.id,findingId:'finding-reconcile',target:assessment.target,relativePath:'src/access.ts',filePath:'D:/Fixture/source/src/access.ts',before:'permit=true',after:'permit=false',baseSha256:'a'.repeat(64),afterSha256:'b'.repeat(64),retest:{kind:'absent',value:'permit=true'},digest:'interrupted-digest',createdAt:now,expiresAt:now,status:mode==='unavailable'?'retesting':'unknown',approvedAt:now,appliedAt:null,retestedAt:null,confirmedAt:null,evidenceId:null,passed:null,message:'Efeito anterior desconhecido.',limits:['Nenhuma conclusão automática.'],interruptedOperation:'applying',reconciliation:null as Record<string,unknown>|null};
    if(mode==='legacy-retest'){plan.status='retested';plan.passed=true;plan.evidenceId='legacy-retest-proof';plan.message='Reteste anterior ao escopo versionado.';}
    if(mode==='revoked-reconcile'){const current=scope.statuses[assessment.id];current.phase='revoked';current.actions.canPrepareWork=false;current.workGates.forEach(g=>{g.allowedByScope=false;g.reasonCodes=['scope_revoked'];});}
    function receipt(note:string){const unavailable=mode==='unavailable';return {id:'receipt-reconcile',evidenceId:'evidence-reconcile',previousStatus:plan.status,interruptedOperation:plan.interruptedOperation,recordedAt:observedAt,author:'local-user',note,acknowledgeUnknownEffects:true,findingStatus:'validated',observation:{state:unavailable?'unavailable':'after',observedAt,observedSha256:unavailable?null:'b'.repeat(64),observedBytes:unavailable?null:12,observedIdentity:unavailable?null:{volume:'123',index:'9007199254740993'},identityMatches:unavailable?null:false,message:unavailable?'Acesso ao arquivo indisponível na inspeção.':'Os bytes coincidem com o proposto, mas a identidade mudou.'}};}
    if(mode==='pending')plan.reconciliation=receipt('Justificativa preservada antes da interrupção.');
    let attempts=0;
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__reconcileCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture sem ambiente.',tools:[],executorReady:false};
      if(action==='remediation.list')return {plans:[structuredClone(plan)]};
      if(action==='remediation.retest'&&mode==='legacy-retest'){plan.retestScopeRef=scope.ref;plan.evidenceId='new-retest-proof';return structuredClone(plan);}
      if(action==='remediation.confirm'&&mode==='legacy-retest'){if(!plan.retestScopeRef||payload.evidenceId!=='new-retest-proof')throw new Error('Novo reteste exigido.');plan.status='confirmed';state.findings[0].status='fixed';return structuredClone(plan);}
      if(action==='remediation.reconcile'){
        attempts++;if(payload.id!==plan.id||payload.acknowledgeUnknownEffects!==true||!String(payload.note).trim())throw new Error('Reconhecimento e justificativa exigidos.');
        if(mode==='refusal')throw new Error('Operação ainda ativa; reconciliação recusada.');
        if(plan.reconciliation&&payload.note!==plan.reconciliation.note)throw new Error('Justificativa diferente da preservada.');
        plan.reconciliation??=receipt(String(payload.note));
        if(mode==='pending'&&attempts===1)throw new Error('Registro ainda indisponível. A observação original permanece preservada.');
        plan.status='reconciled';plan.message='Reconciliação registrada; estado do achado preservado.';return structuredClone(plan);
      }
      throw new Error('Ação não prevista no contrato de reconciliação: '+action);
    }};
  },{mode});
  await page.goto('/');await page.getByRole('button',{name:'Achados',exact:true}).click();await page.getByText('Achado com alteração interrompida',{exact:true}).click();await page.getByRole('button',{name:'Correção',exact:true}).click();if(mode!=='legacy-retest')await expect(page.getByRole('heading',{name:'Revisar correção interrompida',exact:true})).toBeVisible();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__reconcileCalls:{action:string;payload:Record<string,unknown>}[]}).__reconcileCalls.filter(call=>call.action===name),action);}
const ack='Reconheço que os efeitos anteriores podem continuar desconhecidos e autorizo somente o registro desta reconciliação.';
async function noMutatingReplay(page:Page){for(const action of ['remediation.apply','remediation.retest','remediation.confirm','remediation.prepare'])expect(await calls(page,action)).toEqual([]);}

test('reconciliação permanece disponível com novas operações revogadas',async({page})=>{await fixture(page,'revoked-reconcile');await expect(page.getByRole('button',{name:'Preparar correção local',exact:true})).toHaveCount(0);await page.getByLabel('Justificativa da reconciliação',{exact:true}).fill('Inspecionar o registro sem admitir outra alteração.');await page.getByRole('checkbox',{name:ack,exact:true}).check();await page.getByRole('button',{name:'Inspecionar e registrar reconciliação',exact:true}).click();await expect(page.getByText('Reconciliada por observação',{exact:true})).toBeVisible();expect(await calls(page,'remediation.reconcile')).toHaveLength(1);await noMutatingReplay(page);});
test('correção antiga exige reteste vinculado ao escopo atual antes de confirmar sem reaplicar',async({page})=>{await fixture(page,'legacy-retest');await page.getByLabel('Justificativa da confirmação',{exact:true}).fill('Confirmação restrita ao reteste autorizado.');await page.getByRole('checkbox',{name:'Revisei a evidência e os limites do reteste e confirmo a correção deste achado neste alcance.',exact:true}).check();await expect(page.getByRole('button',{name:'Confirmar correção revisada',exact:true})).toBeDisabled();expect(await calls(page,'remediation.retest')).toEqual([]);await page.getByRole('button',{name:'Executar reteste de texto ou hash',exact:true}).click();await expect(page.getByRole('checkbox',{name:'Revisei a evidência e os limites do reteste e confirmo a correção deste achado neste alcance.',exact:true})).not.toBeChecked();await page.getByLabel('Justificativa da confirmação',{exact:true}).fill('O novo reteste foi revisado com os limites registrados.');await page.getByRole('checkbox',{name:'Revisei a evidência e os limites do reteste e confirmo a correção deste achado neste alcance.',exact:true}).check();await page.getByRole('button',{name:'Confirmar correção revisada',exact:true}).click();await expect(page.locator('.detail-heading').getByText('Corrigido',{exact:true})).toBeVisible();expect((await calls(page,'remediation.confirm'))[0].payload.evidenceId).toBe('new-retest-proof');expect(await calls(page,'remediation.apply')).toEqual([]);expect(await calls(page,'remediation.prepare')).toEqual([]);});

test('reconciliação exige justificativa e reconhecimento e distingue arquivo substituído',async({page})=>{
  await fixture(page);const button=page.getByRole('button',{name:'Inspecionar e registrar reconciliação',exact:true});
  await expect(button).toBeDisabled();await expect(page.getByRole('button',{name:'Preparar correção local',exact:true})).toHaveCount(0);expect(await calls(page,'remediation.reconcile')).toEqual([]);
  await page.getByLabel('Justificativa da reconciliação',{exact:true}).fill('Revisei o histórico; os efeitos anteriores continuam desconhecidos.');await expect(button).toBeDisabled();await page.getByRole('checkbox',{name:ack}).check();await button.click();
  const record=page.getByLabel('Reconciliação da correção',{exact:true});await expect(record).toContainText('Bytes correspondem ao conteúdo proposto');await expect(record).toContainText('Identidade diferente: o arquivo foi substituído');await expect(record).toContainText('este registro não confirma a correção do achado');await expect(record.locator('time').first()).toHaveAttribute('datetime','2026-11-06T12:34:56.000Z');
  await expect(page.locator('.detail-heading').getByText('Validado',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Preparar correção local',exact:true})).toBeVisible();
  expect((await calls(page,'remediation.reconcile'))[0].payload).toEqual({id:'remediation-interrupted',note:'Revisei o histórico; os efeitos anteriores continuam desconhecidos.',acknowledgeUnknownEffects:true});await noMutatingReplay(page);
  await record.screenshot({path:test.info().outputPath('reconciliacao-correcao.png')});
});

test('recibo incompleto conserva justificativa e data sem inspeção ou repetição automática',async({page})=>{
  await fixture(page,'pending');const note=page.getByLabel('Justificativa da reconciliação',{exact:true});const button=page.getByRole('button',{name:'Concluir registro da reconciliação',exact:true});
  await expect(note).toHaveValue('Justificativa preservada antes da interrupção.');await expect(note).toHaveAttribute('readonly','');await expect(page.getByLabel('Reconciliação da correção',{exact:true})).toContainText('não faz uma nova inspeção');expect(await calls(page,'remediation.reconcile')).toEqual([]);
  await page.getByRole('checkbox',{name:ack}).check();await button.click();await expect(page.locator('.remediation').getByRole('alert')).toContainText('observação original permanece preservada');await expect(button).toBeDisabled();await expect(page.getByRole('checkbox',{name:ack})).not.toBeChecked();
  await page.getByRole('button',{name:'Atualizar histórico de correções',exact:true}).click();await expect(page.getByRole('button',{name:'Atualizar histórico de correções',exact:true})).toBeEnabled();expect(await calls(page,'remediation.reconcile')).toHaveLength(1);
  await page.getByRole('checkbox',{name:ack}).check();await button.click();await expect(page.getByText('Reconciliada por observação',{exact:true})).toBeVisible();
  const sent=await calls(page,'remediation.reconcile');expect(sent).toHaveLength(2);expect(sent[1].payload).toEqual(sent[0].payload);await expect(page.getByLabel('Reconciliação da correção',{exact:true}).locator('time').first()).toHaveAttribute('datetime','2026-11-06T12:34:56.000Z');await noMutatingReplay(page);
});

test('inspeção indisponível não é apresentada como arquivo ausente nem correção confirmada',async({page})=>{
  await fixture(page,'unavailable');await page.getByLabel('Justificativa da reconciliação',{exact:true}).fill('Aceito encerrar este registro com os limites de observação indicados.');await page.getByRole('checkbox',{name:ack}).check();await page.getByRole('button',{name:'Inspecionar e registrar reconciliação',exact:true}).click();
  const record=page.getByLabel('Reconciliação da correção',{exact:true});await expect(record).toContainText('Arquivo não pôde ser inspecionado');await expect(record).toContainText('Isso não significa que ele esteja ausente');await expect(record).toContainText('Não confirmada nesta observação');await expect(record).not.toContainText('Arquivo ausente no momento da inspeção');await expect(page.locator('.detail-heading').getByText('Validado',{exact:true})).toBeVisible();await noMutatingReplay(page);
});

test('limite em bytes e recusa nativa mantêm reconciliação pendente sem reenvio',async({page})=>{
  await fixture(page,'refusal');const note=page.getByLabel('Justificativa da reconciliação',{exact:true});const button=page.getByRole('button',{name:'Inspecionar e registrar reconciliação',exact:true});
  await note.fill('é'.repeat(2001));await page.getByRole('checkbox',{name:ack}).check();await expect(button).toBeDisabled();expect(await calls(page,'remediation.reconcile')).toEqual([]);
  await note.fill('Revisão com efeitos não confirmados.');await expect(page.getByRole('checkbox',{name:ack})).not.toBeChecked();await page.getByRole('checkbox',{name:ack}).check();await button.click();
  await expect(page.locator('.remediation').getByRole('alert')).toContainText('Operação ainda ativa');await expect(button).toBeDisabled();await expect(page.getByText('Efeito desconhecido',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Preparar correção local',exact:true})).toHaveCount(0);
  expect(await calls(page,'remediation.reconcile')).toHaveLength(1);await noMutatingReplay(page);
});
