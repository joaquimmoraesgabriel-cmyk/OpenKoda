import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// The source scanner is mocked here. Native hash/identity and rule guarantees
// are established by Rust/native evidence, never by this browser fixture.
async function fixture(page:Page,mode='success'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__sourceCalls:{action:string;payload:Record<string,unknown>}[]};w.isTauri=true;w.__sourceCalls=[];
    const assessment={id:'source-assessment',projectId:'source-project',title:'Análise local de contrato',kind:'code',target:'D:/Fixture/source',objective:'Revisar sinais textuais sem executar código.',status:'draft',createdAt:now,updatedAt:now,events:[],evidence:[] as {id:string;name:string;content:string;sha256:string;createdAt:string}[],coverage:[],summary:''};
    const settings={provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false};
    const state={projects:[{id:assessment.projectId,name:'Projeto local de contrato',kind:assessment.kind,target:assessment.target,description:'Fixture sem leitura de disco.',createdAt:now,archived:mode==='archived'}],assessments:[assessment],findings:[] as Record<string,unknown>[],settings,plans:[] as Record<string,unknown>[],executions:[] as Record<string,unknown>[],validations:[]};
    const scope=window.__scopeFixture(state,['local.inventory.v1','source.rules.v1']);
    function result(){return {adapter:'source.rules.v1',rulesVersion:'2026-11-07.1',manifestSha256:'c'.repeat(64),termination:mode==='partial'?'timed_out':'completed',canceled:false,timedOut:mode==='partial',partial:mode==='partial',counts:{plannedFiles:2,plannedBytes:72,verifiedFiles:2,scannedFiles:mode==='partial'?1:2,binaryFiles:0,tooLargeFiles:0,partialFiles:0,readErrors:0,excludedDirectories:1,unvisitedFiles:mode==='partial'?1:0,candidates:mode==='partial'?3:1,omittedCandidates:mode==='partial'?3:0,omittedFileResults:mode==='partial'?1:0},candidates:mode==='partial'?[]:[{id:'source-signal',ruleId:'dynamic-evaluation',ruleVersion:'1',status:'candidate',title:'Uso textual de avaliação dinâmica',observation:'O padrão lexical requer revisão de contexto e pode ser falso positivo.',path:'src/access.ts',line:5,fileSha256:'a'.repeat(64)}],fileResults:[{path:'src/access.ts',sha256:'a'.repeat(64),bytes:56,status:'scanned',candidateCount:1}],limits:['Regras lexicais não comprovam vulnerabilidade.','Nenhum código executado ou enviado.','Comentários e testes podem gerar sinais.']};}
    function evidence(){return {id:'source-evidence',name:'Sinais locais de contrato',content:JSON.stringify(result()),sha256:'scan-evidence-sha',createdAt:now};}
    if(mode==='untrusted'){assessment.status='partial';assessment.evidence.push(evidence());}
    if(mode==='followup'||mode==='archived'){assessment.status='partial';assessment.evidence.push({id:'prior-evidence',name:'Evidência da operação anterior',content:'Observação anterior preservada.',sha256:'prior-evidence-hash',createdAt:now});}
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__sourceCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture sem ambiente.',tools:[],executorReady:false};
      if(action==='ai.history')return {messages:[]};
      if(action==='assessment.start'||action==='assessment.sourcePlan'){
        const source=action==='assessment.sourcePlan';const plan={scopeRef:scope.ref,id:'source-plan',assessmentId:assessment.id,adapter:source?'source.rules.v1':'local.inventory.v1',target:assessment.target,timeoutSeconds:10,outputLimitKiB:16,inputFiles:[{path:'src/access.ts',size:56,sha256:'a'.repeat(64)},{path:'README.md',size:16,sha256:'b'.repeat(64)}],resolvedAddresses:[],sourceScope:source?{version:1,rulesVersion:'2026-11-07.1',rootIdentity:{volume:'123',index:'9007199254740993'},targetIsFile:false,bindings:[{path:'src/access.ts',identity:{volume:'123',index:'9007199254740994'}},{path:'README.md',identity:{volume:'123',index:'9007199254740995'}}],excludedDirectories:[{path:'node_modules',reason:'policy-directory'}],manifestSha256:'c'.repeat(64)}:null,digest:'source-plan-digest',createdAt:now,expiresAt:new Date(Date.now()+(mode==='expired'?-1000:600000)).toISOString(),approvedAt:null,status:'proposed'};state.plans=[plan];assessment.status='awaiting_approval';return structuredClone(plan);
      }
      if(action==='plan.revoke'){const plan=state.plans[0];if(payload.id!==plan.id||payload.digest!==plan.digest)throw new Error('Plano divergente.');plan.status='revoked';assessment.status='blocked';return structuredClone(plan);}
      if(action==='plan.approve'||action==='plan.run'){
        const plan=state.plans[0];if(payload.id!==plan?.id||payload.digest!==plan?.digest)throw new Error('Plano divergente.');
        if(action==='plan.approve'){plan.status='approved';plan.approvedAt=now;assessment.status='queued';return structuredClone(plan);}
        if(plan.status!=='approved')throw new Error('Aprovação ausente.');
        if(mode==='stale'){plan.status='failed';assessment.status='failed';throw new Error('O conteúdo mudou após a aprovação; os novos bytes não foram analisados.');}
        plan.status='completed';assessment.status='partial';assessment.evidence.push(evidence());const execution={id:'source-execution',planId:plan.id,assessmentId:assessment.id,status:'completed',startedAt:now,finishedAt:now,evidenceId:'source-evidence',message:'Observação lexical limitada registrada.'};state.executions.push(execution);return structuredClone(execution);
      }
      if(action==='candidate.record'){const finding={id:'manual-source-candidate',assessmentId:assessment.id,title:payload.title,description:payload.description,severity:payload.severity,evidenceIds:payload.evidenceIds,recommendation:payload.recommendation,status:'candidate',confidence:'Requer revisão',createdAt:now};state.findings.push(finding);return structuredClone(finding);}
      throw new Error('Ação não prevista no contrato de análise: '+action);
    }};
  },{mode});
  await page.goto('/');await page.getByText('Análise local de contrato',{exact:true}).click();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__sourceCalls:{action:string;payload:Record<string,unknown>}[]}).__sourceCalls.filter(call=>call.action===name),action);}
async function prepareAndRun(page:Page){
  await page.getByRole('button',{name:'Preparar análise local',exact:true}).click();
  await page.getByRole('checkbox',{name:'Revisei o alvo, os efeitos e os limites deste plano e autorizo esta operação.'}).check();
  await page.getByRole('button',{name:'Aprovar plano',exact:true}).click();await page.getByRole('button',{name:'Executar plano aprovado',exact:true}).click();
}
async function openResult(page:Page){await page.getByRole('navigation',{name:'Seções',exact:true}).getByRole('button',{name:'Arquivos',exact:true}).click();await page.locator('.detail-pane>.evidence-list summary').filter({hasText:'Sinais locais de contrato'}).click();return page.locator('.detail-pane>.evidence-list .source-results');}

test('inventário continua sendo o método padrão e análise local exige escolha separada',async({page})=>{
  await fixture(page);await expect(page.getByRole('button',{name:'Preparar análise local',exact:true})).toBeVisible();
  expect(await calls(page,'assessment.sourcePlan')).toEqual([]);
  await page.getByRole('button',{name:'Iniciar avaliação',exact:true}).click();
  await expect(page.getByText('Inventário local de arquivos',{exact:true})).toBeVisible();expect(await calls(page,'assessment.start')).toHaveLength(1);expect(await calls(page,'assessment.sourcePlan')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
});

test('análise revisa identidades e exclusões e só registra candidato após revisão manual',async({page})=>{
  await fixture(page);await page.getByRole('button',{name:'Preparar análise local',exact:true}).click();
  await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toBeDisabled();
  await expect(page.getByLabel('Escopo da análise local',{exact:true})).toContainText('2026-11-07.1');
  await page.getByText('Identidades dos arquivos aprovados',{exact:true}).click();await expect(page.getByLabel('Escopo da análise local',{exact:true})).toContainText('9007199254740994');
  await page.getByText('Diretórios excluídos · 1',{exact:true}).click();await expect(page.getByLabel('Escopo da análise local',{exact:true})).toContainText('node_modules');
  expect(await calls(page,'plan.run')).toEqual([]);expect(await calls(page,'candidate.record')).toEqual([]);
  await page.getByRole('checkbox',{name:'Revisei o alvo, os efeitos e os limites deste plano e autorizo esta operação.'}).check();await page.getByRole('button',{name:'Aprovar plano',exact:true}).click();await page.getByRole('button',{name:'Executar plano aprovado',exact:true}).click();
  const result=await openResult(page);await expect(result).toContainText('src/access.ts · linha 5');await expect(result).toContainText('não comprova uma vulnerabilidade');
  await result.screenshot({path:test.info().outputPath('analise-local-candidatos.png')});
  expect(await calls(page,'candidate.record')).toEqual([]);await result.getByRole('button',{name:'Preparar registro deste candidato',exact:true}).click();
  await expect(page.getByLabel('Título do candidato',{exact:true})).toHaveValue('Uso textual de avaliação dinâmica');await expect(page.getByLabel('Descrição do candidato',{exact:true})).toHaveValue(/src\/access\.ts, linha 5/);
  await expect(page.getByRole('checkbox',{name:'Sinais locais de contrato',exact:true})).toBeChecked();expect(await calls(page,'candidate.record')).toEqual([]);
  await page.getByRole('button',{name:'Salvar candidato',exact:true}).click();await expect.poll(async()=>(await calls(page,'candidate.record')).length).toBe(1);expect((await calls(page,'candidate.record'))[0].payload).toMatchObject({assessmentId:'source-assessment',evidenceIds:['source-evidence']});expect(await calls(page,'finding.validate')).toEqual([]);expect(await calls(page,'ai.ask')).toEqual([]);
});

test('análise recusa bytes alterados após aprovação e não produz sinais fictícios',async({page})=>{
  await fixture(page,'stale');await prepareAndRun(page);await expect(page.getByRole('alert')).toContainText('novos bytes não foram analisados');
  await expect(page.locator('.detail-heading .status')).toHaveText('Falhou');await expect(page.locator('.source-results')).toHaveCount(0);expect(await calls(page,'plan.run')).toHaveLength(1);expect(await calls(page,'candidate.record')).toEqual([]);
});

test('resultado parcial exibe tempo limite, arquivos não visitados e sinais omitidos',async({page})=>{
  await fixture(page,'partial');await prepareAndRun(page);const result=await openResult(page);
  await expect(result).toContainText('Tempo limite atingido');await expect(result).toContainText('O resultado é parcial');await expect(result).toContainText('1 não visitado(s)');await expect(result).toContainText('3 detectado(s) · 0 exibido(s) · 3 omitido(s)');await expect(result).toContainText('Isso não comprova ausência de vulnerabilidades');
  await expect(result.getByRole('button',{name:'Preparar registro deste candidato',exact:true})).toHaveCount(0);expect(await calls(page,'candidate.record')).toEqual([]);
});

test('JSON de fonte sem execução nativa vinculada permanece texto bruto',async({page})=>{
  await fixture(page,'untrusted');await page.getByRole('navigation',{name:'Seções',exact:true}).getByRole('button',{name:'Arquivos',exact:true}).click();await page.locator('.detail-pane>.evidence-list summary').filter({hasText:'Sinais locais de contrato'}).click();
  await expect(page.locator('.detail-pane>.evidence-list pre')).toContainText('source.rules.v1');await expect(page.locator('.source-results')).toHaveCount(0);expect(await calls(page,'candidate.record')).toEqual([]);
});

test('nova operação após resultado parcial exige novo plano aprovado e preserva evidência anterior',async({page})=>{
  await fixture(page,'followup');await page.getByRole('button',{name:'Preparar outra operação',exact:true}).click();await expect(page.getByText('Inventário local de arquivos',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toBeDisabled();expect(await calls(page,'plan.run')).toEqual([]);await page.getByRole('checkbox',{name:'Revisei o alvo, os efeitos e os limites deste plano e autorizo esta operação.'}).check();await page.getByRole('button',{name:'Aprovar plano',exact:true}).click();expect(await calls(page,'plan.run')).toEqual([]);await page.getByRole('button',{name:'Executar plano aprovado',exact:true}).click();await expect(page.getByRole('button',{name:'Preparar outra operação',exact:true})).toBeVisible();await page.getByRole('navigation',{name:'Seções',exact:true}).getByRole('button',{name:'Arquivos',exact:true}).click();await page.locator('.detail-pane>.evidence-list summary').filter({hasText:'Evidência da operação anterior'}).click();await expect(page.locator('.detail-pane>.evidence-list')).toContainText('Observação anterior preservada.');await expect(page.locator('.detail-pane>.evidence-list')).toContainText('prior-evidence-hash');expect(await calls(page,'assessment.start')).toHaveLength(1);expect(await calls(page,'plan.approve')).toHaveLength(1);expect(await calls(page,'plan.run')).toHaveLength(1);
});

test('inventário expirado é revogado antes de outra preparação sem chamada implícita a assessment.plan',async({page})=>{
  await fixture(page,'expired');await page.getByRole('button',{name:'Iniciar avaliação',exact:true}).click();await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toHaveCount(0);await page.getByRole('button',{name:'Descartar plano expirado',exact:true}).click();await expect(page.getByRole('button',{name:'Iniciar avaliação',exact:true})).toBeEnabled();expect((await calls(page,'plan.revoke'))[0].payload).toEqual({id:'source-plan',digest:'source-plan-digest'});expect(await calls(page,'assessment.plan')).toEqual([]);expect(await calls(page,'assessment.start')).toHaveLength(1);expect(await calls(page,'plan.run')).toEqual([]);
});

test('projeto arquivado não oferece nova operação executável nem análise local',async({page})=>{
  await fixture(page,'archived');await expect(page.getByRole('button',{name:'Preparar outra operação',exact:true})).toBeDisabled();await expect(page.getByRole('button',{name:'Preparar análise local',exact:true})).toBeDisabled();expect(await calls(page,'assessment.start')).toEqual([]);expect(await calls(page,'assessment.sourcePlan')).toEqual([]);
});
