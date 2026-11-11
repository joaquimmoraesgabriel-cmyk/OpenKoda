import { openAssessmentList } from './navigation';
import { reviewKaliData } from './data-fixture';
import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// UI contracts only; this fixture does not start WSL, ripgrep or any external API.
async function fixture(page:Page,mode='completed'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__scanCalls:{action:string;payload:Record<string,unknown>}[];__finishKaliScan:()=>void};w.isTauri=true;w.__scanCalls=[];
    const makeEvidence=(id:string,content:string)=>({id,name:id,content,sha256:id+'-hash',createdAt:now});
    const assessment={id:'assessment-kali-scan',projectId:'project-kali-scan',title:'Avaliação do analisador Kali',kind:'code',target:'D:/Fixture/source',objective:'Fixture de interface',status:'partial',createdAt:now,updatedAt:now,events:[],evidence:[makeEvidence('selected-evidence','Texto sintético\nlinha 2\nlinha 3\nlinha 4'),makeEvidence('other-evidence','Outro conteúdo'),makeEvidence('nul-evidence','Texto\0 inválido'),makeEvidence('oversize-evidence','é'.repeat(17000))],coverage:[],summary:''};
    const state={projects:[{id:assessment.projectId,name:'Projeto de contrato',kind:'code',target:assessment.target,description:'Fixture',createdAt:now}],assessments:[assessment],findings:[] as Record<string,unknown>[],settings:{provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture-model',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false},plans:[],executions:[],validations:[]};
    const scope=window.__scopeFixture(state,['kali.rg.signals.v1']);
    type Run={id:string;assessmentId:string;environmentId:string;tool:string;status:string;startedAt:string;finishedAt:string|null;sourceEvidenceId:string;sourceSha256:string;evidenceId:string|null;error?:string};
    let active:Run|null=null;const recent:Run[]=[];let finish:((result:unknown)=>void)|undefined;
    function output(termination='completed'){return {operation:'scan_bytes',adapter:'kali.rg.signals.v1',rulesVersion:'2026-11-07.1',inputSha256:'selected-evidence-hash',bytes:44,environmentId:'environment-fixture',sessionId:'session-fixture',assessmentId:assessment.id,tool:{name:'ripgrep',version:'14.1.0 (contrato)',binarySha256:'b'.repeat(64)},candidates:[{ruleId:'dynamic-evaluation',line:4,label:'Sinal de avaliação dinâmica',status:'candidate'}],counts:{'private-key-marker':0,'dynamic-evaluation':termination==='completed'?1:3,'credential-identifier':0},observedSignals:termination==='completed'?1:3,omittedCandidates:termination==='completed'?0:2,countsComplete:termination==='completed',partial:termination!=='completed',termination,effects:'Fixed offline parsing of selected bytes; no target execution, paths or network',limitations:'Lexical candidates can be false positives; absence is not proof of security'};}
    function complete(run:Run,termination:string){run.status=termination==='canceled'?'canceled':'completed';run.finishedAt=now;run.evidenceId='scan-output-evidence';const result=output(termination);assessment.evidence.push(makeEvidence('scan-output-evidence',JSON.stringify({result})));recent.push(structuredClone(run));active=null;return {run:structuredClone(run),result,cleanupConfirmed:true};}
    w.__finishKaliScan=()=>{if(active)finish?.(complete(active,'canceled'));};
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__scanCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='environment.status')return {status:'ready',distro:'OpenKoda-Kali',message:'Fixture sem execução real.',tools:[],executorReady:false};
      if(action==='ai.history')return {messages:[],requests:[]};
      if(action==='kali.mcp.status')return {active:structuredClone(active),recent:structuredClone(recent),hasUnresolved:!!active,capabilities:['runtime_diagnostics','hash_bytes','scan_bytes'],executorReady:false};
      if(action==='packages.catalog')return {version:'1',distro:'OpenKoda-Kali',inventoryVerified:true,tools:[{package:'ripgrep',name:'ripgrep',group:'Análise textual',description:'Busca por padrões.',adapter:'kali.rg.signals.v1',adapterImplemented:true,executionReady:false,availability:'Adaptador de sinais disponível na avaliação. Identidade, instalação e limites são verificados antes de analisar a evidência selecionada.',installed:false,version:null},{package:'jq',name:'jq',group:'Utilitários',description:'Processamento JSON.',adapter:'none',adapterImplemented:false,executionReady:false,availability:'Nenhum adaptador liberado para execução.',installed:true,version:'fixture'}]};
      if(action==='kali.mcp.scanEvidence'){
        if(payload.assessmentId!==assessment.id||payload.evidenceId!=='selected-evidence'||payload.acknowledgeLocalTransfer!==true)throw new Error('Entrada e consentimento específicos exigidos.');
        const run:Run={id:'scan-run-fixture',assessmentId:assessment.id,environmentId:'environment-fixture',tool:'scan_bytes',status:'running',startedAt:now,finishedAt:null,sourceEvidenceId:'selected-evidence',sourceSha256:'selected-evidence-hash',evidenceId:null};
        if(mode==='failure'){run.status='failed';run.error='ripgrep não está instalado ou sua identidade não foi confirmada.';recent.push(run);throw new Error(run.error);}
        if(mode==='cancel'){active=run;return new Promise(resolve=>{finish=resolve;});}
        return complete(run,mode==='partial'?'output_limited':'completed');
      }
      if(action==='kali.mcp.cancel'){if(!active)throw new Error('Sem sessão ativa.');active.status='canceling';return {requested:true,runId:active.id,status:'canceling'};}
      throw new Error('Ação inesperada no contrato de análise Kali: '+action);
    }};
  },{mode});
  await page.goto('/');await openAssessmentList(page);await page.getByText('Avaliação do analisador Kali',{exact:true}).click();await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();await expect(page.getByRole('heading',{name:'Análise de padrões no Kali',exact:true})).toBeVisible();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__scanCalls:{action:string;payload:Record<string,unknown>}[]}).__scanCalls.filter(call=>call.action===name),action);}
const consent='Autorizo transferir somente esta evidência revisada ao Kali local para a operação exibida.';
async function prepare(page:Page){const panel=page.locator('.kali-bridge');await panel.getByLabel('Evidência para verificar no Kali',{exact:true}).selectOption('selected-evidence');await reviewKaliData(panel,'scan');return panel;}

test('análise Kali exige seleção e transferência local e bloqueia NUL ou excesso de bytes',async({page})=>{
  await fixture(page);const panel=page.locator('.kali-bridge');const select=panel.getByLabel('Evidência para verificar no Kali',{exact:true});const button=panel.getByRole('button',{name:'Analisar evidência no Kali',exact:true});
  await expect(button).toBeDisabled();await select.selectOption('selected-evidence');await expect(button).toBeDisabled();await reviewKaliData(panel,'scan');await select.selectOption('other-evidence');await expect(panel.getByRole('checkbox',{name:consent})).toHaveCount(0);await expect(button).toBeDisabled();
  await select.selectOption('nul-evidence');await expect(panel).toContainText('não considera esta evidência elegível');await expect(panel.getByRole('button',{name:'Revisar dados para análise',exact:true})).toBeDisabled();await select.selectOption('oversize-evidence');await expect(panel).toContainText('Acima do limite de 32 KiB');await expect(button).toBeDisabled();expect(await calls(page,'kali.mcp.scanEvidence')).toEqual([]);
  await prepare(page);await button.click();await expect(panel.getByLabel('Análise de padrões na evidência',{exact:true})).toContainText('Leitura delimitada encerrada');await expect(panel.getByRole('checkbox',{name:consent})).toHaveCount(0);expect((await calls(page,'kali.mcp.scanEvidence'))[0].payload).toMatchObject({assessmentId:'assessment-kali-scan',evidenceId:'selected-evidence',acknowledgeLocalTransfer:true});expect(await calls(page,'ai.ask')).toEqual([]);expect(await calls(page,'candidate.record')).toEqual([]);
});

test('sessão concluída com saída limitada mantém cobertura parcial e linha da evidência',async({page})=>{
  await fixture(page,'partial');const panel=await prepare(page);await panel.getByRole('button',{name:'Analisar evidência no Kali',exact:true}).click();const result=panel.getByLabel('Análise de padrões na evidência',{exact:true});
  await expect(panel).toContainText('Resposta recebida');await expect(result).toContainText('Limite de saída atingido');await expect(result).toContainText('Resultado parcial');await expect(result).toContainText('Contagem incompleta');await expect(result).toContainText('3 observado(s) · 1 exibido(s) · 2 omitido(s)');await expect(result).toContainText('Linha 4 da evidência');await expect(result).toContainText('não em um arquivo do alvo');await expect(result).toContainText('ripgrep · 14.1.0 (contrato)');
  await expect(panel.locator('.key-value').filter({hasText:'Evidência de entrada'})).toContainText('selected-evidence');await expect(panel.locator('.key-value').filter({hasText:'Evidência registrada'})).toContainText('scan-output-evidence');
  await result.screenshot({path:test.info().outputPath('kali-analise-parcial.png')});await result.getByRole('button',{name:'Preparar candidato desta evidência',exact:true}).click();await expect(page.getByLabel('Descrição do candidato',{exact:true})).toHaveValue(/Linha 4 da evidência de entrada selected-evidence, não de um arquivo do alvo/);await expect(page.getByRole('checkbox',{name:'scan-output-evidence',exact:true})).toBeChecked();expect(await calls(page,'candidate.record')).toEqual([]);expect(await calls(page,'finding.validate')).toEqual([]);
});

test('falha de ferramenta Kali fica visível sem instalar pacotes nem repetir a análise',async({page})=>{
  await fixture(page,'failure');const panel=await prepare(page);await panel.getByRole('button',{name:'Analisar evidência no Kali',exact:true}).click();await expect(panel.getByRole('alert')).toContainText('ripgrep não está instalado');await expect(panel.getByRole('button',{name:'Analisar evidência no Kali',exact:true})).toBeDisabled();await panel.getByRole('button',{name:'Consultar estado da conexão',exact:true}).click();await expect(panel.getByRole('button',{name:'Consultar estado da conexão',exact:true})).toBeEnabled();expect(await calls(page,'kali.mcp.scanEvidence')).toHaveLength(1);expect(await calls(page,'packages.plan')).toEqual([]);expect(await calls(page,'packages.apply')).toEqual([]);await expect(panel.locator('.kali-scan-results')).toHaveCount(0);
});

test('cancelamento da análise Kali aguarda confirmação sem bloquear o controle',async({page})=>{
  await fixture(page,'cancel');const panel=await prepare(page);await panel.getByRole('button',{name:'Analisar evidência no Kali',exact:true}).click();await panel.getByRole('button',{name:'Solicitar cancelamento da sessão',exact:true}).click();await expect(panel.getByRole('button',{name:'Cancelamento solicitado',exact:true})).toBeDisabled();await expect(panel.getByText('Confirmada',{exact:true})).toHaveCount(0);await page.evaluate(()=>(window as unknown as {__finishKaliScan:()=>void}).__finishKaliScan());await expect(panel.getByLabel('Análise de padrões na evidência',{exact:true})).toContainText('Análise cancelada');await expect(panel.getByText('Confirmada',{exact:true})).toBeVisible();expect(await calls(page,'kali.mcp.scanEvidence')).toHaveLength(1);expect(await calls(page,'kali.mcp.cancel')).toHaveLength(1);
});

test('catálogo separa adaptador de sinais disponível de pacote instalado e execução pronta',async({page})=>{
  await fixture(page);await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();const tool=page.locator('.tool-row').filter({hasText:'ripgrep'});await expect(tool).toContainText('Adaptador disponível');await expect(tool).toContainText('Não instalado');await expect(tool).toContainText('Identidade, instalação e limites são verificados');await expect(tool).not.toContainText('Execução disponível');const other=page.locator('.tool-row').filter({hasText:'jq'});await expect(other).toContainText('Instalado');await expect(other).toContainText('Adaptador não liberado');expect(await calls(page,'packages.plan')).toEqual([]);expect(await calls(page,'kali.mcp.scanEvidence')).toEqual([]);
});
