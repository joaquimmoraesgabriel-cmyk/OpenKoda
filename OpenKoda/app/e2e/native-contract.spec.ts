import { openAssessmentList } from './navigation';
import { chooseAiData, reviewKaliData } from './data-fixture';
import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Browser contract fixtures exercise UI decisions only. They do not prove native
// authority, credential encryption, WSL, or provider connectivity.
async function nativeFixture(page:Page, environmentReady=false, bridgeMode='success', remediationMode='none', imageMode='available', recoveryMode='none', assessmentMode='normal') {
  await installScopeFixture(page);
  await page.addInitScript(({environmentReady,bridgeMode,remediationMode,imageMode,recoveryMode,assessmentMode})=>{
    const appWindow=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__contractCalls:{action:string;payload:Record<string,unknown>}[]};
    appWindow.isTauri=true;appWindow.__contractCalls=[];
    const now=new Date().toISOString();
    const disk={id:crypto.randomUUID(),sourceId:crypto.randomUUID(),source:{name:'OpenKoda-Kali'},maxBytes:64*1024**3,spool:{terminalCaptures:0,sessionRecords:0},
      ...(imageMode==='archive-legacy-vhdx'?{}:{format:'tar'})};
    const restore={id:crypto.randomUUID(),backup:disk,bytes:4096,sha256:'a'.repeat(64),firstWslDistribution:false};
    const activationFixture = imageMode.startsWith('archive-bind');
    const formerId = crypto.randomUUID();
    const targetName = 'OpenKoda-Recovery-' + restore.id;
    const previousName = 'OpenKoda-Recovery-' + formerId;
    let activationReviewed: { id: string; digest: string } | undefined;
    let activated = false;
    let supportReview: { reviewId: string; fileName: string; relativePath: string; sha256: string; bytes: number } | undefined;
    let releaseOldInventory: (() => void) | undefined;
    const oldInventory = imageMode === 'archive-bind-probe'
      ? new Promise<void>(resolve => { releaseOldInventory = resolve; }) : undefined;
    if (oldInventory) Object.assign(window, { __releaseOldInventory: () => releaseOldInventory?.() });
    const interrupted={operation:{kind:'export',backup:{...disk,id:crypto.randomUUID()}},status:imageMode==='archive-metadata'?'exporting_source':'stop_unknown',startedAt:now};
    const archives:{operation:object;status:string;startedAt:string}[]=[interrupted];
    if(imageMode==='archive-legacy-vhdx')archives.push({operation:{kind:'export',backup:disk},status:'exported',startedAt:now});
    const diskReview={digest:imageMode==='archive-metadata'?null:'b'.repeat(64),expiresAt:new Date(Date.now()+600000).toISOString()};
    if(imageMode==='archive-metadata')archives.push({operation:{kind:'[Texto indisponível]'},status:'[Texto indisponível]',startedAt:now});
    const assessment={id:'assessment-fixture',projectId:'project-fixture',title:'Avaliação de contrato',kind:remediationMode==='none'?'web':'code',target:remediationMode==='none'?'https://fixture.example.test/':'D:\\Fixture',objective:'Verificar controles de interface.',status:'awaiting_approval',createdAt:now,updatedAt:now,events:[],evidence:[{id:'evidence-fixture',name:'Cabeçalhos de contrato',content:'X-Fixture: true',sha256:'fixture-hash',createdAt:now}],coverage:[],summary:''};
    const plan={id:'plan-fixture',assessmentId:assessment.id,adapter:remediationMode==='none'?'http.headers.v1':'local.inventory.v1',target:assessment.target,timeoutSeconds:10,outputLimitKiB:16,inputFiles:[],resolvedAddresses:['192.0.2.20'],digest:'test-plan-digest',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),approvedAt:null as string|null,status:'proposed'};
    const state={projects:[{id:'project-fixture',name:'Projeto de contrato',kind:assessment.kind,target:assessment.target,description:'Fixture de interface',createdAt:now}],assessments:[assessment],findings:[{id:'finding-fixture',assessmentId:assessment.id,title:'Candidato de contrato',severity:'low',confidence:'Requer revisão',status:remediationMode==='none'?'candidate':'validated',description:'Observação simulada para o contrato da interface.',recommendation:'Revisar a evidência.',evidenceIds:['evidence-fixture'],createdAt:now}],validations:[] as {id:string;findingId:string;assessmentId:string;method:string;expected:string;observed:string;evidence:{id:string;sha256:string}[];author:string;createdAt:string}[],settings:{provider:'openai',endpoint:'https://api.example.test/v1',model:'fixture-model',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false},plans:[plan],executions:[] as {id:string;planId:string;assessmentId:string;status:string;startedAt:string;finishedAt:string|null;evidenceId:null;message:string}[]};
    const scope=window.__scopeFixture(state,remediationMode==='none'?['http.headers.v1','ai.remote-review.v1','kali.runtime-diagnostics.v1','kali.hash-evidence.v1']:['local.inventory.v1','local.text-remediation.v1','ai.remote-review.v1']);
    if(assessmentMode.startsWith('unknown')){assessment.status='unknown';plan.status='unknown';}
    if(assessmentMode.startsWith('import'))assessment.status='draft';
    if(bridgeMode==='oversized')state.assessments[0].evidence[0].content='é'.repeat(17000);
    const providerKeys=new Set<string>();
    const revisions=new Map<string,string>();
    const enabledModels=new Map<string,string[]>();
    const providerEndpoints:Record<string,string>={openai:'https://api.openai.com/v1',openrouter:'https://openrouter.ai/api/v1',gemini:'https://generativelanguage.googleapis.com/v1beta/openai'};
    let catalogAttempts=0, verified=false;
    if (assessmentMode==='legacy-provider'){
      state.settings.model='';state.settings.hasApiKey=true;providerKeys.add('openai');revisions.set('openai','legacy-revision');
    }
    const catalogs:Record<string,string[]>={openai:['fixture-model'],openrouter:['example/model-a','example/model-b',...Array.from({length:24},(_,i)=>'fixture/model-'+i)],groq:['example-chat'],gemini:['gemini-fixture']};
    let finishRun:((result:unknown)=>void)|undefined;
    const messages:{id:string;role:string;text:string;createdAt:string}[]=[];
    let remediationPlan:Record<string,unknown>|undefined;
    const currentProfile={kind:'base'};const previousProfile=recoveryMode==='rollback'?{kind:'profile',id:'previous-profile-fixture'}:null;
    let activationPlan:Record<string,unknown>|null=null;let pendingActivation:Record<string,unknown>|null=null;
    function makeActivationPlan(kind:'restore'|'rollback'){
      return {id:'activation-fixture',kind,sourceRecoveryId:kind==='restore'?'recovery-receipt-fixture':null,fromProfile:currentProfile,sourceProfile:kind==='rollback'?previousProfile:currentProfile,targetProfile:{kind:'profile',id:'new-sanitized-fixture'},files:[{name:'openkoda.sqlite',bytes:4096,sha256:'activation-file-fixture-hash'}],digest:'activation-fixture-digest',createdAt:now,expiresAt:new Date(Date.now()+86400000).toISOString(),status:'prepared',message:'Uma cópia sanitizada está pronta para revisão.',effects:['Ativar a cópia na próxima abertura.','Preservar o perfil anterior.'],limits:['Credenciais não são transferidas.','O Kali exige revalidação.','Arquivos externos de projetos não são restaurados.'],restartRequired:true};
    }
    if(recoveryMode==='blocked'){activationPlan=makeActivationPlan('restore');activationPlan.status='blocked';activationPlan.message='Integridade da cópia não confirmada; perfil original preservado.';pendingActivation=activationPlan;}
    type BridgeRun={id:string;assessmentId:string;environmentId:string;tool:string;status:string;startedAt:string;finishedAt:string|null;sourceSha256:string|null;evidenceId?:string;error?:string};
    let bridgeActive:BridgeRun|null=null;const bridgeRecent:BridgeRun[]=[];
    let bridgeResolve:((value:unknown)=>void)|undefined;
    if(bridgeMode==='unknown-error')bridgeRecent.push({id:'bridge-interrupted',assessmentId:assessment.id,environmentId:'environment-fixture',tool:'runtime_diagnostics',status:'unknown',startedAt:now,finishedAt:null,sourceSha256:null,error:'Identidade da sessão ainda não confirmada.'});
    (window as unknown as {__finishBridge:()=>void}).__finishBridge=()=>{if(bridgeActive){bridgeActive.status='canceled';bridgeActive.finishedAt=now;const run=structuredClone(bridgeActive);bridgeRecent.push(run);bridgeActive=null;bridgeResolve?.({run,result:{canceled:true},cleanupConfirmed:true});}};
    appWindow.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');
      const {action,payload}=args.request;appWindow.__contractCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='support.review') {
        const reviewId=crypto.randomUUID(), fileName='suporte-'+reviewId+'.json';
        const contents={formatVersion:1,appVersion:'0.1.9',platform:'windows',architecture:'x86_64',coreSchema:8,
          localDatabaseFiles:{present:7,absent:1,unavailable:0},telemetry:'disabled',remoteTransmission:false};
        const preview=JSON.stringify(contents,null,2)+'\n', encoded=new TextEncoder().encode(preview);
        const sha256=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',encoded)),b=>b.toString(16).padStart(2,'0')).join('');
        supportReview={reviewId,fileName,relativePath:'exports/'+fileName,sha256,bytes:encoded.length};
        return {...supportReview,contents,preview:imageMode==='support-changed'?preview+' ':preview,
          expiresAt:new Date(Date.now()+(imageMode==='support-expired'?-1000:600000)).toISOString()};
      }
      if(action==='support.export') {
        if(!supportReview || payload.reviewId!==supportReview.reviewId || payload.confirmation!==true)throw Error('Confirmação do arquivo divergente.');
        const {reviewId,...receipt}=supportReview;
        if(imageMode==='support-error')throw Error('A pasta de exportações está indisponível.');
        return {...receipt,bytes:imageMode==='support-receipt'?receipt.bytes+1:receipt.bytes};
      }
      if(oldInventory && (action==='environment.status' || action==='packages.catalog')){
        await oldInventory;
        if(action==='environment.status')document.documentElement.dataset.oldInventoryReturned='true';
        if (action==='packages.catalog') return {version:'1',inventoryVerified:true,distro:previousName,tools:[{
          package:'previous-fixture',name:'Ferramenta do executor anterior',group:'Teste',description:'Inventário antigo',adapter:'',
          executionReady:true,availability:'Disponível',installed:true,version:'1',
        }]};
        return {status:'ready',distro:previousName,environmentId:formerId,message:'Diagnóstico anterior',tools:[],executorReady:true};
      }
      if(action==='environment.host.check'||action==='environment.host.resume'){
        if(action==='environment.host.resume' && !sessionStorage.getItem('host-checkpoint')) return null;
        if(action==='environment.host.check') sessionStorage.setItem('host-checkpoint','test');
        const resumed = action==='environment.host.resume';
        return {step:resumed?'review_kali':'prepare_wsl',startedAt:now,checkedAt:now,rebootObserved:resumed,
          windowsBuild:22631,architecture:'x64',virtualMachinePlatform:resumed,hypervisor:true,
          virtualizationFirmware:null,freeBytes:16*2**30,distributionCount:0,wslClient:resumed,wslVersion:resumed?'2.6.1':null};
      }
      if(action==='environment.status'||action==='environment.diagnose')return {status:environmentReady?'ready':imageMode==='registered'||imageMode.startsWith('repair')?'degraded':'unavailable',distro:'OpenKoda-Kali',message:'Ambiente de contrato sem executor.',tools:[],executorReady:false,managedRegistration:environmentReady||imageMode==='registered'||imageMode.startsWith('repair'),imageProvisionAvailable:!environmentReady&&imageMode!=='registered'&&!imageMode.startsWith('repair'),repairAvailable:imageMode.startsWith('repair')};
      if(action==='packages.catalog')return {version:'1',tools:[],inventoryVerified:false,distro:'OpenKoda-Kali'};
      if(action==='environment.archive.list' && activationFixture){
        if(payload.beforeId !== undefined && payload.beforeId !== formerId) throw new Error('Cursor inesperado.');
        const older=payload.beforeId===formerId;
        const record={status:'inactive',startedAt:now,operation:{kind:'import',restore:older?restore:{...restore,id:formerId}},
          activationState:older?(activated?'active':'available'):(activated?'previous':'active')};
        return {records:[record],nextBeforeId:older?null:formerId};
      }
      if(action==='environment.archive.list')return {records:structuredClone(archives)};
      if(action==='environment.archive.activatePlan' && activationFixture){
        if(payload.id!==restore.id || activated)throw new Error('Cópia indisponível para ativação.');
        activationReviewed={id:crypto.randomUUID(),digest:'d'.repeat(64)};
        return {...activationReviewed,digest:imageMode==='archive-bind-hidden'?null:activationReviewed.digest,
          expiresAt:new Date(Date.now()+(imageMode==='archive-bind-expired'?-60000:60000)).toISOString(),
          payload:{previousEnvironmentId:formerId,previousDistribution:previousName,targetEnvironmentId:restore.id,targetDistribution:targetName,
            registration:{name:targetName,guid:'{'+crypto.randomUUID()+'}',basePath:'D:/Fixture/recovery/'+restore.id,version:2},
            effects:['Verificar a identidade da cópia antes de vincular o executor.','Preservar a distribuição anterior e suas autorizações.']}};
      }
      if(action==='environment.archive.activate' && activationFixture){
        if (!activationReviewed || payload.planId!==activationReviewed.id || payload.digest!==activationReviewed.digest || activated) throw new Error('Revisão divergente.');
        activated=true;
        if(imageMode==='archive-bind-lost') throw new Error('Resposta da troca indisponível.');
        return {status:'activated',environmentId:restore.id,distro:targetName,previousEnvironmentId:formerId,previousDistribution:previousName};
      }
      if(action==='environment.archive.inspectStop'){interrupted.status='incomplete_copy';return structuredClone(interrupted);}
      if(action==='environment.archive.exportPlan')return {...diskReview,id:'export-review',payload:{backup:disk,target:'fixture.tar',effects:'Exportar arquivos da distribuição de teste'}};
      if(action==='environment.archive.importPlan')return {...diskReview,id:'import-review',payload:{restore,target:'fixture-clone',distribution:'OpenKoda-Recovery-archive-clone',effects:'Recuperar cópia de teste'}};
      if(action==='environment.archive.export'||action==='environment.archive.import'){
        const receipt=action.endsWith('.export')?{operation:{kind:'export',backup:disk},status:'exported',startedAt:now}:{operation:{kind:'import',restore},status:'inactive',startedAt:now};
        archives.push(receipt);return receipt;
      }
      if(action==='plan.approve'){if(payload.digest!==plan.digest)throw new Error('Digest inválido');plan.status='approved';plan.approvedAt=now;return structuredClone(plan);}
      if(action==='plan.run'){plan.status='running';assessment.status='running';state.executions.push({id:'execution-fixture',planId:plan.id,assessmentId:assessment.id,status:'running',startedAt:now,finishedAt:null,evidenceId:null,message:'Operação em andamento'});return new Promise(resolve=>{finishRun=resolve;});}
      if(action==='assessment.cancel'){assessment.status='canceled';plan.status='canceled';state.executions[0].status='canceled';state.executions[0].finishedAt=new Date().toISOString();state.executions[0].message='Cancelamento confirmado';finishRun?.(structuredClone(state.executions[0]));return structuredClone(assessment);}
      if(action==='assessment.reconcile'){
        if(assessmentMode==='unknown-refusal')throw new Error('Supervisor ainda ativo. A reconciliação permanece bloqueada.');
        if(payload.id!==assessment.id||payload.acknowledgeUnknownEffects!==true||!String(payload.note).trim())throw new Error('Nota e reconhecimento exigidos.');
        assessment.status='partial';assessment.summary='Interrupção registrada sem repetir execução; efeitos remotos permanecem não confirmados.';return structuredClone(assessment);
      }
      if(action==='evidence.import'){
        if(assessmentMode==='import-refusal')throw new Error('A avaliação tem atividade pendente; importação recusada.');
        if(payload.acknowledgeUnverifiedSource!==true)throw new Error('Origem não reconhecida.');
        const evidence={id:'imported-evidence-fixture',name:String(payload.name),content:JSON.stringify({origin:'user_supplied',sourceVerified:false,sourceDescription:payload.sourceDescription,text:payload.content}),sha256:'imported-fixture-sha',createdAt:now};assessment.evidence.push(evidence);return structuredClone(evidence);
      }
      if(action==='candidate.record'){const finding={id:'recorded-fixture',assessmentId:assessment.id,title:String(payload.title),severity:String(payload.severity),confidence:'A validar',status:'candidate',description:String(payload.description),recommendation:String(payload.recommendation),evidenceIds:payload.evidenceIds as string[],createdAt:now};state.findings.push(finding);return structuredClone(finding);}
      if(action==='finding.validate'){state.findings[0].status='validated';state.validations.push({id:'validation-fixture',findingId:'finding-fixture',assessmentId:assessment.id,method:String(payload.method),expected:String(payload.expected),observed:String(payload.observed),evidence:[{id:'evidence-fixture',sha256:'fixture-hash'}],author:'local-user',createdAt:now});return structuredClone(state.findings[0]);}
      if(action==='remediation.list')return {plans:remediationPlan?[structuredClone(remediationPlan)]:[]};
      if(action==='remediation.get')return structuredClone(remediationPlan);
      if(action==='remediation.prepare'){
        remediationPlan={scopeRef:scope.ref,id:'remediation-fixture',projectId:'project-fixture',assessmentId:assessment.id,findingId:'finding-fixture',target:assessment.target,relativePath:payload.relativePath,filePath:'D:/Fixture/'+payload.relativePath,before:'permit = true\n',after:payload.after,baseSha256:'before-fixture-hash',afterSha256:'after-fixture-hash',retest:payload.retest,digest:'remediation-fixture-digest',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),status:'proposed',approvedAt:null,appliedAt:null,retestedAt:null,confirmedAt:null,evidenceId:null,passed:null,message:'Um arquivo de contrato será alterado.',limits:['Verificação de texto não comprova segurança completa.']};return structuredClone(remediationPlan);
      }
      if(action.startsWith('remediation.')){
        if(!remediationPlan||payload.id!==remediationPlan.id||payload.digest!==remediationPlan.digest)throw new Error('Plano divergente.');
        if(action==='remediation.approve'){remediationPlan.status='approved';remediationPlan.approvedAt=now;}
        if(action==='remediation.apply'){
          if(remediationMode==='stale'){remediationPlan.status='invalidated';remediationPlan.message='Arquivo mudou desde a revisão. Nenhuma alteração aplicada.';throw new Error(remediationPlan.message as string);}
          remediationPlan.status='applied';remediationPlan.appliedAt=now;remediationPlan.message='Alteração aplicada; confirmação ainda pendente.';
        }
        if(action==='remediation.retest'){remediationPlan.retestScopeRef=scope.ref;remediationPlan.status='retested';remediationPlan.retestedAt=now;remediationPlan.evidenceId='retest-fixture';remediationPlan.passed=remediationMode!=='negative';remediationPlan.message='Condição de reteste registrada.';}
        if(action==='remediation.confirm'){if(payload.evidenceId!=='retest-fixture')throw new Error('Evidência divergente.');remediationPlan.status='confirmed';remediationPlan.confirmedAt=now;state.findings[0].status='fixed';remediationPlan.message='Correção confirmada com limites.';}
        return structuredClone(remediationPlan);
      }
      if(action==='kali.mcp.status')return {active:structuredClone(bridgeActive),recent:structuredClone(bridgeRecent),hasUnresolved:bridgeRecent.some(run=>run.status==='unknown'),capabilities:['runtime_diagnostics','hash_bytes'],executorReady:false};
      if(action==='kali.mcp.diagnose'||action==='kali.mcp.hashEvidence'){
        const run:BridgeRun={id:'bridge-'+action,assessmentId:assessment.id,environmentId:'environment-fixture',tool:action==='kali.mcp.diagnose'?'runtime_diagnostics':'hash_bytes',status:'completed',startedAt:now,finishedAt:now,sourceSha256:action==='kali.mcp.hashEvidence'?'fixture-hash':null,evidenceId:'bridge-evidence-fixture'};
        if(bridgeMode==='failure'){run.status='failed';run.error='Falha de comunicação com o bridge de teste.';bridgeRecent.push(run);throw new Error(run.error);}
        if(bridgeMode==='cancel'){run.status='running';run.finishedAt=null;bridgeActive=run;return new Promise(resolve=>{bridgeResolve=resolve;});}
        bridgeRecent.push(run);return {run,result:action==='kali.mcp.diagnose'?{distribution:'Kali de contrato',python:'fixture',capabilities:['runtime_diagnostics','hash_bytes']}:{algorithm:'sha256',sha256:'fixture-hash',bytes:15},cleanupConfirmed:true};
      }
      if(action==='kali.mcp.cancel'){if(!bridgeActive)throw new Error('Nenhuma sessão ativa.');bridgeActive.status='canceling';return {requested:true,runId:bridgeActive.id,status:'canceling'};}
      if(action==='kali.mcp.reconcile')throw new Error('Identidade da sessão não confirmada; efeito continua desconhecido.');
      if(action==='settings.save'){Object.assign(state.settings,payload);state.settings.hasApiKey=providerKeys.has(state.settings.provider);verified=false;return state.settings;}
      if(action==='ai.selection.save'){Object.assign(assessment,{aiSelection:payload.aiSelection});return {ok:true};}
      const provider=String(payload.provider??state.settings.provider);
      const providerStatus=()=>({provider,hasApiKey:providerKeys.has(provider),credentialRevision:revisions.get(provider)??null,enabledModels:enabledModels.get(provider)??null,verified,checkedAt:verified?now:null,model:state.settings.model,endpoint:providerEndpoints[provider]??state.settings.endpoint});
      if(action==='ai.status')return providerStatus();
      if(action==='ai.key.save'){
        providerKeys.add(provider);revisions.set(provider,crypto.randomUUID());
        if(payload.prepareActivation)enabledModels.set(provider,[]);
        state.settings.hasApiKey=providerKeys.has(state.settings.provider);return {saved:true};
      }
      if(action==='ai.key.remove'){providerKeys.delete(provider);revisions.delete(provider);enabledModels.delete(provider);state.settings.hasApiKey=providerKeys.has(state.settings.provider);return {removed:true};}
      if(action==='ai.provider.configure'){
        if (!providerKeys.has(provider) || !revisions.get(provider) || revisions.get(provider)!==payload.credentialRevision)throw new Error('Credencial alterada.');
        const models=payload.models as string[];
        if(!models?.length||models.some(m=>!catalogs[provider]?.includes(m)))throw new Error('Seleção de modelos inválida.');
        enabledModels.set(provider,[...models]);
        if(typeof payload.defaultModel==='string')Object.assign(state.settings,{provider,model:payload.defaultModel,endpoint:providerEndpoints[provider],hasApiKey:true});
        return providerStatus();
      }
      if(action==='ai.test'){verified=true;return {verified:true,capability:'text'};}
      if(action==='ai.models'){
        if(!providerKeys.has(provider))throw new Error('Credencial indisponível para este provedor.');
        if(assessmentMode==='catalog-transient' && catalogAttempts++===0)throw new Error('Não foi possível consultar o provedor.');
        return {models:catalogs[provider]??[],provider,credentialRevision:revisions.get(provider)};
      }
      if(action==='ai.context')return {context:{objective:assessment.objective,target:assessment.target,evidence:[]},digest:'context-fixture-digest',bytes:156,notice:'Fixture: conteúdo destinado ao provedor configurado.'};
      if(action==='ai.history')return {messages:structuredClone(messages)};
      if(action==='ai.ask'){if(payload.contextDigest!=='context-fixture-digest'||payload.allowRemote!==true)throw new Error('Contexto sem aprovação');messages.push({id:'user-fixture',role:'user',text:String(payload.message),createdAt:now},{id:'assistant-fixture',role:'assistant',text:'Resposta de contrato sem chamada externa.',createdAt:now});return {text:messages[1].text,usage:{},toolsExecuted:false};}
      if(action==='backup.list')return {backups:[{id:'backup-fixture',createdAt:now,bytes:8192,verification:'requires-check'}]};
      if(action==='retention.inventory')return {items:[],journals:[],hasUnresolved:false,blocked:false,limits:[],projectPurgeSupported:false,automaticPurge:false,retentionDays:90};
      if(action==='backup.verify')return {verified:true,path:'D:/Fixture/backup'};
      if(action==='backup.restore')return {id:'recovery-receipt-fixture',path:'D:/Fixture/recovery/recovery-receipt-fixture',verified:true,activated:false,message:'Restauração preparada em uma pasta separada; os dados atuais continuam ativos.'};
      if(action==='activation.status')return {currentProfile,activeProfile:currentProfile,previousProfile,pendingPlan:structuredClone(pendingActivation),restartRequired:pendingActivation?.status==='scheduled',notice:'O processo atual conserva o perfil original.'};
      if(action==='activation.list')return {plans:activationPlan?[structuredClone(activationPlan)]:[]};
      if(action==='activation.plan'||action==='activation.rollbackPlan'){
        if(action==='activation.plan'&&payload.recoveryId!=='recovery-receipt-fixture')throw new Error('Recibo da restauração divergente.');
        activationPlan=makeActivationPlan(action==='activation.plan'?'restore':'rollback');return structuredClone(activationPlan);
      }
      if(action==='activation.schedule'||action==='activation.cancel'){
        if(payload.id!==activationPlan?.id||payload.digest!==activationPlan?.digest)throw new Error('Plano de ativação divergente.');
        if(action==='activation.schedule'&&recoveryMode==='schedule-error')throw new Error('SHA-256 da cópia divergiu. Agendamento recusado.');
        if(action==='activation.schedule'){activationPlan!.status='scheduled';activationPlan!.message='Troca agendada; perfil atual preservado até nova abertura.';pendingActivation=activationPlan;}
        if(action==='activation.cancel'){activationPlan!.status='canceled';pendingActivation=null;}
        return structuredClone(activationPlan);
      }
      if(action==='environment.plan')return {id:'maintenance-fixture',kind:'provision',environmentId:'environment-fixture',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),payload:{effects:['Criar distribuição dedicada de teste'],source:'fixture',location:'test-only'},digest:'maintenance-fixture-digest',consumed:false};
      if(action==='environment.provision'){if(payload.planId!=='maintenance-fixture'||payload.digest!=='maintenance-fixture-digest')throw new Error('Plano inválido');if(imageMode==='standard-error')throw new Error('Preparação padrão: tempo de download excedido. Nenhuma distribuição registrada.');return {status:'ready'};}
      if(action==='environment.image.plan'){
        if(imageMode==='mismatch')throw new Error('SHA-256 da imagem diverge do manifesto oficial. Nenhuma distribuição registrada.');
        return {id:'image-plan-fixture',kind:'provision-image',environmentId:'environment-fixture',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),payload:{distro:'OpenKoda-Kali',location:'D:/Fixture/managed-kali',image:{manifestUrl:'https://fixture.example.test/SHA256SUMS',url:'https://fixture.example.test/kali.wsl',sha256:'a'.repeat(64),bytes:247857686,localFile:'D:/Fixture/cache/kali.wsl'},effects:['Importar a imagem verificada no ambiente dedicado'],provenance:'Manifesto oficial verificado pelo backend de contrato',limits:['Fixture; nenhum download real']},digest:'image-plan-fixture-digest',consumed:false};
      }
      if(action==='environment.image.provision'){if(payload.planId!=='image-plan-fixture'||payload.digest!=='image-plan-fixture-digest')throw new Error('Plano de imagem inválido');return {status:'ready'};}
      if(action==='environment.repair.plan')return {id:'repair-plan-fixture',kind:'environment-repair',environmentId:'environment-fixture',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),payload:{distro:'OpenKoda-Kali',environmentId:'environment-fixture',registration:{guid:'managed-fixture',name:'OpenKoda-Kali',basePath:'D:/Fixture/managed-kali',version:2},stateSha256:'state-fixture-hash',maintenanceSha256:'maintenance-fixture-hash',maintenance:{action:'fixture',status:'unknown'},effects:['Encerrar somente OpenKoda-Kali e verificar o estado gerido'],limits:['Sem shutdown global e sem instalação automática de pacotes']},digest:'repair-fixture-digest',consumed:false};
      if(action==='environment.repair.apply'){
        if(payload.planId!=='repair-plan-fixture'||payload.digest!=='repair-fixture-digest')throw new Error('Plano de recuperação inválido.');
        if(imageMode==='repair-error')throw new Error('Tempo de recuperação excedido. O ambiente continua degradado; revise um novo plano.');
        return {status:'degraded',repair:{status:'completed'},repairAudit:[]};
      }
      throw new Error('Ação não implementada no contrato de teste: '+action);
    }};
  },{environmentReady,bridgeMode,remediationMode,imageMode,recoveryMode,assessmentMode});
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__contractCalls:{action:string;payload:Record<string,unknown>}[]}).__contractCalls.filter(call=>call.action===name),action);}

test('plano exige revisão explícita e permite cancelar durante a operação',async({page})=>{
  await nativeFixture(page);await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toBeDisabled();
  expect(await calls(page,'plan.approve')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
  await page.getByRole('checkbox',{name:'Revisei o alvo, os efeitos e os limites deste plano e autorizo esta operação.'}).check();
  await page.getByRole('button',{name:'Aprovar plano',exact:true}).click();
  await expect(page.getByRole('button',{name:'Executar plano aprovado',exact:true})).toBeEnabled();
  expect((await calls(page,'plan.approve'))[0].payload).toEqual({id:'plan-fixture',digest:'test-plan-digest'});
  expect(await calls(page,'plan.run')).toEqual([]);
  await page.getByRole('button',{name:'Executar plano aprovado',exact:true}).click();
  await expect(page.locator('.detail-heading .status')).toHaveText('Em execução');
  await expect(page.getByRole('button',{name:'Pausar',exact:true})).toHaveCount(0);
  await expect(page.getByText('A operação atual não oferece pausa. Use Cancelar para solicitar o encerramento e aguarde a confirmação do executor.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Cancelar',exact:true}).click();
  await page.getByRole('button',{name:'Confirmar cancelamento',exact:true}).click();
  await expect(page.getByText('Cancelada',{exact:true}).first()).toBeVisible();
  expect(await calls(page,'assessment.cancel')).toHaveLength(1);
});

test('ambiente preparado não é apresentado como executor disponível',async({page})=>{
  await nativeFixture(page,true);await page.goto('/?area=tecnica');
  await expect(page.getByRole('button',{name:'Kali preparado',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Kali pronto para executar',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Kali preparado',exact:true}).click();
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  await expect(page.getByText('Ainda não liberado',{exact:true})).toBeVisible();
});

test('revisão de avaliação desconhecida exige nota e reconhecimento sem repetir execução',async({page})=>{
  await nativeFixture(page,false,'success','none','available','none','unknown');await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  const panel=page.locator('.assessment-reconciliation');
  await expect(panel).toContainText('Efeitos enviados antes da interrupção podem ter ocorrido e não são desfeitos.');
  await expect(panel.getByRole('button',{name:'Registrar revisão da interrupção',exact:true})).toBeDisabled();
  await panel.getByLabel('Nota sobre a interrupção',{exact:true}).fill('O processo local encerrou. Não foi possível confirmar os efeitos remotos.');
  await expect(panel.getByRole('button',{name:'Registrar revisão da interrupção',exact:true})).toBeDisabled();
  expect(await calls(page,'assessment.reconcile')).toEqual([]);
  await panel.getByRole('checkbox',{name:'Entendo que efeitos já enviados não são desfeitos e que esta revisão não repete a operação.'}).check();
  await panel.getByRole('button',{name:'Registrar revisão da interrupção',exact:true}).click();
  await expect(page.locator('.detail-heading .status')).toHaveText('Parcial');await expect(panel).toHaveCount(0);
  expect((await calls(page,'assessment.reconcile'))[0].payload).toEqual({id:'assessment-fixture',note:'O processo local encerrou. Não foi possível confirmar os efeitos remotos.',acknowledgeUnknownEffects:true});
  expect(await calls(page,'plan.run')).toEqual([]);expect(await calls(page,'assessment.start')).toEqual([]);
});

test('reconciliação recusada mantém estado desconhecido e pede nova confirmação',async({page})=>{
  await nativeFixture(page,false,'success','none','available','none','unknown-refusal');await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  const panel=page.locator('.assessment-reconciliation');
  await panel.getByLabel('Nota sobre a interrupção',{exact:true}).fill('Revisão da interrupção local.');
  await panel.getByRole('checkbox',{name:'Entendo que efeitos já enviados não são desfeitos e que esta revisão não repete a operação.'}).check();
  await panel.getByRole('button',{name:'Registrar revisão da interrupção',exact:true}).click();
  await expect(panel.getByRole('alert')).toContainText('Supervisor ainda ativo');
  await expect(page.locator('.detail-heading .status')).toHaveText('Efeito desconhecido');
  await expect(panel.getByRole('button',{name:'Registrar revisão da interrupção',exact:true})).toBeDisabled();
  expect(await calls(page,'assessment.reconcile')).toHaveLength(1);expect(await calls(page,'plan.run')).toEqual([]);
});

async function openEvidenceImport(page:Page,mode='import'){
  await nativeFixture(page,false,'success','none','available','none',mode);await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:/^Evidências$/}).click();await page.locator('summary').filter({hasText:/^Importar texto$/}).click();
  await page.getByLabel('Nome do texto importado',{exact:true}).fill('Trecho de log declarado');await page.getByLabel('Origem declarada',{exact:true}).fill('Texto copiado manualmente do laboratório.');
}

test('importação preserva texto literal com origem não verificada e sem envio automático',async({page})=>{
  await openEvidenceImport(page);const literal='<script>window.untrusted=true</script>\n$(Get-Process)\n  espaços preservados  ';
  await page.getByLabel('Conteúdo textual',{exact:true}).fill(literal);
  await expect(page.getByRole('button',{name:'Registrar texto com origem declarada',exact:true})).toBeDisabled();expect(await calls(page,'evidence.import')).toEqual([]);
  await page.getByRole('checkbox',{name:'Declaro a origem deste texto e entendo que ela não foi verificada pelo OpenKoda.'}).check();
  await page.getByRole('button',{name:'Registrar texto com origem declarada',exact:true}).click();
  await expect(page.getByText('Texto registrado com origem declarada não verificada. Nenhum conteúdo foi enviado à IA.',{exact:true})).toBeVisible();
  expect((await calls(page,'evidence.import'))[0].payload).toMatchObject({assessmentId:'assessment-fixture',content:literal,acknowledgeUnverifiedSource:true});
  await page.locator('summary').filter({hasText:'Trecho de log declarado'}).click();
  await expect(page.locator('.evidence-item').filter({hasText:'Trecho de log declarado'})).toContainText('"sourceVerified":false');
  expect(await page.evaluate(()=>(window as unknown as {untrusted?:boolean}).untrusted)).toBeUndefined();
  expect(await calls(page,'ai.ask')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
});

test('importação mede bytes UTF-8 e conserva recusa do backend sem repetir',async({page})=>{
  await openEvidenceImport(page,'import-refusal');await page.getByLabel('Conteúdo textual',{exact:true}).fill('é'.repeat(17000));
  await page.getByRole('checkbox',{name:'Declaro a origem deste texto e entendo que ela não foi verificada pelo OpenKoda.'}).check();
  await expect(page.locator('.evidence-import').getByRole('alert')).toContainText('excede 32 KiB');
  await expect(page.getByRole('button',{name:'Registrar texto com origem declarada',exact:true})).toBeDisabled();expect(await calls(page,'evidence.import')).toEqual([]);
  await page.getByLabel('Conteúdo textual',{exact:true}).fill('Trecho dentro do limite.');await page.getByRole('button',{name:'Registrar texto com origem declarada',exact:true}).click();
  await expect(page.locator('.evidence-import').getByRole('alert')).toContainText('importação recusada');
  expect(await calls(page,'evidence.import')).toHaveLength(1);expect(await calls(page,'ai.ask')).toEqual([]);
});

test('mensagem exige contexto visível e consentimento específico',async({page})=>{
  await nativeFixture(page);await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Conversa com a IA'}).click();
  await page.getByLabel('Sua mensagem',{exact:true}).fill('Analise os limites da observação.');
  await expect(page.getByRole('button',{name:'Enviar mensagem',exact:true})).toBeDisabled();
  await chooseAiData(page.locator('.ai-conversation'));await page.getByRole('button',{name:'Revisar contexto de envio',exact:true}).click();
  await expect(page.getByLabel('Pacote exato 1',{exact:true})).toContainText('https://fixture.example.test/');
  await expect(page.getByRole('button',{name:'Enviar mensagem',exact:true})).toBeDisabled();
  expect(await calls(page,'ai.ask')).toEqual([]);
  await page.getByRole('checkbox',{name:'Autorizo o envio deste contexto e da mensagem ao provedor configurado.'}).check();
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect(page.getByText('Resposta de contrato sem chamada externa.',{exact:true})).toBeVisible();
  expect((await calls(page,'ai.ask'))[0].payload).toMatchObject({contextDigest:'context-fixture-digest',allowRemote:true,evidenceIds:[]});
  await expect(page.getByRole('button',{name:'Enviar mensagem',exact:true})).toBeDisabled();
});

async function openProviderSettings(page:Page) {
  await page.getByRole('navigation',{name:'Navegação principal'}).getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'IA e modelos',exact:true}).click();
}
async function beginProvider(page:Page, name:string) {
  await page.getByRole('button',{name:'Adicionar provedor',exact:true}).click();
  const wizard=page.getByRole('dialog',{name:'Adicionar provedor'});
  await wizard.getByRole('searchbox',{name:'Pesquisar provedores'}).fill(name);
  await wizard.getByRole('button',{name:new RegExp(name)}).click();
}
async function saveKey(page:Page, value:string) {
  const wizard=page.getByRole('dialog');
  const input=wizard.getByLabel('Chave de API',{exact:true});
  await expect(input).toHaveAttribute('type','password');
  await input.fill(value);
  await wizard.getByRole('button',{name:'Salvar chave e buscar modelos'}).click();
}
test('credencial chega ao cofre e ativação e teste de conexão são ações distintas', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page);await page.goto('/');await openProviderSettings(page);
  await page.getByRole('button',{name:'Conectar com chave de API'}).click();
  await saveKey(page,'fixture-not-a-real-secret');
  const wizard=page.getByRole('dialog');
  await expect(wizard.getByRole('checkbox',{name:'fixture-model',exact:true})).toBeChecked();
  expect(await calls(page,'ai.test')).toEqual([]);
  expect(await calls(page,'ai.provider.configure')).toEqual([]);
  expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('fixture-not-a-real-secret');
  await wizard.getByRole('button',{name:'Ativar provedor'}).click();
  await expect(wizard).toHaveCount(0);
  await page.getByRole('button',{name:'Testar conexão',exact:true}).click();
  await expect(page.getByText(/Conexão verificada/)).toBeVisible();
  expect(await calls(page,'ai.test')).toHaveLength(1);
});

test('catálogo e modelos ativados alimentam configurações e uma nova análise oficial', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page);await page.goto('/');await openProviderSettings(page);
  await beginProvider(page,'OpenRouter');
  await expect(page.getByRole('dialog')).toContainText('https://openrouter.ai/api/v1');
  await saveKey(page,'synthetic-not-a-key');
  const wizard=page.getByRole('dialog');
  const filter=wizard.getByRole('searchbox',{name:'Buscar modelos'});
  await filter.fill('nenhum-modelo-existe');
  await expect(wizard.getByRole('status')).toHaveText('Nenhum modelo corresponde ao filtro.');
  await filter.fill('example/');
  const list=wizard.getByRole('group',{name:'Modelos do provedor'});
  await expect(list.getByRole('checkbox')).toHaveCount(2);
  await list.getByRole('checkbox',{name:'example/model-a',exact:true}).check();
  await list.getByRole('checkbox',{name:'example/model-b',exact:true}).check();
  await expect(wizard.getByLabel('Usar como padrão da instalação')).toBeChecked();
  await wizard.getByLabel('Modelo padrão').selectOption('example/model-a');
  await page.screenshot({path:test.info().outputPath('selecao-de-modelos.png')});
  await wizard.getByRole('button',{name:'Ativar provedor'}).click();
  await expect(wizard).toHaveCount(0);
  await page.screenshot({path:test.info().outputPath('provedor-ativado.png')});
  expect((await calls(page,'ai.key.save'))[0].payload).toMatchObject({provider:'openrouter',prepareActivation:true});
  expect((await calls(page,'ai.models')).every(call=>call.payload.provider==='openrouter')).toBe(true);
  expect((await calls(page,'ai.provider.configure'))[0].payload).toMatchObject({provider:'openrouter',models:['example/model-a','example/model-b'],defaultModel:'example/model-a'});
  expect(await calls(page,'ai.test')).toEqual([]);
  await page.getByRole('button',{name:'Início',exact:true}).click();
  const create=page.getByRole('button',{name:'Criar projeto',exact:true});
  const treeNew=page.getByRole('button',{name:'Novo projeto',exact:true});
  await create.or(treeNew).first().click();
  await page.getByRole('button',{name:'Escolher modelo'}).click();
  const picker=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
  await expect(picker.getByRole('button',{name:'example/model-a'})).toBeVisible();
  await picker.getByRole('button',{name:'example/model-b'}).click();
  await expect(page.getByRole('button',{name:'Escolher modelo'})).toContainText('example/model-b');
  expect(await calls(page,'ai.ask')).toEqual([]);
});

test('provedor salvo sem modelo na versão anterior pode ser ativado sem redigitar a chave', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page,false,'success','none','available','none','legacy-provider');
  await page.goto('/');await openProviderSettings(page);
  await page.getByRole('button',{name:'Escolher modelos e ativar'}).click();
  const wizard=page.getByRole('dialog');
  await expect(wizard.getByRole('checkbox',{name:'fixture-model'})).toBeChecked();
  await wizard.getByRole('button',{name:'Ativar provedor'}).click();
  await expect(page.getByRole('list',{name:'Modelos selecionados'})).toContainText('fixture-model');
  expect(await calls(page,'ai.key.save')).toHaveLength(0);
  expect((await calls(page,'ai.provider.configure'))[0].payload).toMatchObject({defaultModel:'fixture-model',provider:'openai'});
});

test('novos provedores admitem consulta repetida e retomada após salvar a chave', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page,false,'success','none','available','none','catalog-transient');
  await page.goto('/');await openProviderSettings(page);
  await page.getByRole('button',{name:'Adicionar provedor'}).click();
  const wizard=page.getByRole('dialog');
  for(const domain of ['api.anthropic.com','generativelanguage.googleapis.com','api.deepseek.com','api.x.ai'])await expect(wizard.getByText(domain,{exact:true})).toBeVisible();
  await wizard.getByRole('button',{name:/Google Gemini/}).click();
  await saveKey(page,'synthetic-gemini-fixture');
  await expect(wizard.getByRole('alert')).toContainText('Não foi possível consultar o provedor.');
  await wizard.getByRole('button',{name:'Tentar novamente'}).click();
  await expect(wizard.getByLabel('gemini-fixture',{exact:true})).toBeChecked();
  await wizard.getByRole('button',{name:'Cancelar'}).click();
  await expect(page.getByRole('button',{name:'Escolher modelos e ativar'})).toBeVisible();
  await expect(page.getByText('Aguardando ativação').first()).toBeVisible();
  await page.getByRole('button',{name:'Escolher modelos e ativar'}).click();
  await wizard.getByRole('button',{name:'Ativar provedor'}).click();
  await expect(wizard).toHaveCount(0);
  expect(await calls(page,'ai.key.save')).toHaveLength(1);
  expect((await calls(page,'ai.provider.configure'))[0].payload).toMatchObject({provider:'gemini',defaultModel:'gemini-fixture'});
  expect(await calls(page,'ai.test')).toEqual([]);
});

test('voltar ou cancelar descarta a chave não salva e conserva o padrão', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page);await page.goto('/');await openProviderSettings(page);
  await beginProvider(page,'OpenAI');
  const wizard=page.getByRole('dialog');
  const credential=wizard.getByLabel('Chave de API',{exact:true});
  await credential.fill('synthetic-openai-entry');
  await wizard.getByRole('button',{name:'Voltar'}).click();
  await wizard.getByRole('button',{name:/Groq/}).click();
  await expect(credential).toHaveValue('');
  await expect(wizard.getByRole('button',{name:'Salvar chave e buscar modelos'})).toBeDisabled();
  await credential.fill('synthetic-groq-entry');
  await wizard.getByRole('button',{name:'Cancelar'}).click();
  await expect(page.getByRole('complementary',{name:'Provedores configurados'}).getByRole('button',{name:/OpenAI/})).toHaveAttribute('aria-pressed','true');
  expect(await calls(page,'ai.key.save')).toEqual([]);
  expect(await calls(page,'ai.models')).toEqual([]);
});

test('ativar um segundo provedor conserva o padrão até a troca explícita', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page);await page.goto('/');await openProviderSettings(page);
  await beginProvider(page,'OpenAI');await saveKey(page,'first-provider-fixture');
  const wizard=page.getByRole('dialog');
  await wizard.getByRole('button',{name:'Ativar provedor'}).click();
  await beginProvider(page,'Groq');await saveKey(page,'second-provider-fixture');
  await expect(wizard.getByLabel('Usar como padrão da instalação')).not.toBeChecked();
  await wizard.getByRole('button',{name:'Ativar provedor'}).click();
  await expect(wizard).toHaveCount(0);
  expect((await calls(page,'ai.provider.configure')).at(-1)?.payload.defaultModel).toBe(null);
  await page.getByRole('button',{name:'Definir como padrão'}).click();
  await expect(wizard.getByLabel('Usar como padrão da instalação')).toBeChecked();
  await wizard.getByRole('button',{name:'Salvar modelos'}).click();
  await expect(wizard).toHaveCount(0);
  expect((await calls(page,'ai.provider.configure')).at(-1)?.payload).toMatchObject({provider:'groq',defaultModel:'example-chat'});
  await expect(page.getByRole('complementary',{name:'Provedores configurados'}).getByRole('button',{name:/Groq/})).toContainText('Padrão da instalação');
});

test('nenhum modelo no catálogo mantém a credencial pendente e permite voltar sem redigitá-la', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page);await page.goto('/');await openProviderSettings(page);
  await beginProvider(page,'Anthropic');await saveKey(page,'empty-catalog-fixture');
  const wizard=page.getByRole('dialog');
  await expect(wizard.getByRole('status')).toContainText('O provedor não retornou modelos');
  await expect(wizard.getByRole('button',{name:'Ativar provedor'})).toBeDisabled();
  await wizard.getByRole('button',{name:'Voltar'}).click();
  await wizard.getByRole('button',{name:'Usar a chave já salva'}).click();
  await expect(wizard.getByRole('status')).toContainText('O provedor não retornou modelos');
  expect(await calls(page,'ai.provider.configure')).toEqual([]);
  expect(await calls(page,'ai.key.save')).toHaveLength(1);
  expect(await calls(page,'ai.test')).toEqual([]);
});
test('cópias do Kali mantêm a origem parada após inspeção, exportação e restauração',async({page})=>{
  await nativeFixture(page,true);await page.goto('/?area=tecnica');
  await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Manutenção',exact:true}).click();
  const panel=page.getByRole('region',{name:'Cópias do Kali'});
  await panel.getByRole('button',{name:'Consultar cópias do Kali'}).click();
  await expect(panel.getByText('Parada da origem a conferir')).toBeVisible();
  const initialProbes=(await calls(page,'environment.status')).length;
  expect(initialProbes).toBeGreaterThan(0);
  await panel.getByRole('button',{name:'Conferir se a origem parou'}).click();
  await expect(panel.getByText('Exportação incompleta')).toBeVisible();
  await panel.getByRole('button',{name:'Revisar exportação do Kali'}).click();
  await expect(panel.getByText('TAR · arquivos da distribuição WSL',{exact:true})).toBeVisible();
  await expect(panel).toContainText('registros e capturas presentes');
  await expect(panel).not.toContainText('espaço livre');
  const approval=panel.getByRole('checkbox',{name:/Revisei a origem e o destino/});
  await expect(approval).toHaveAccessibleName(/todos os arquivos da cópia TAR/);
  await expect(panel.getByRole('button',{name:'Exportar cópia revisada'})).toBeDisabled();
  await approval.check();
  await panel.getByRole('button',{name:'Exportar cópia revisada'}).click();
  await panel.getByRole('button',{name:'Revisar restauração desta cópia'}).click();
  await expect(approval).not.toBeChecked();
  await expect(panel.getByText('TAR · arquivos da distribuição WSL',{exact:true})).toBeVisible();
  await approval.check();
  await panel.getByRole('button',{name:'Importar cópia revisada'}).click();
  await expect(panel.getByText('Acessar os arquivos recuperados')).toBeVisible();
  expect(await calls(page,'environment.status')).toHaveLength(initialProbes);
  expect(await calls(page,'environment.archive.inspectStop')).toHaveLength(1);
  expect(await calls(page,'environment.archive.export')).toHaveLength(1);
  expect(await calls(page,'environment.archive.import')).toHaveLength(1);
});

test('cópias do Kali com detalhes ocultos permitem consultar o histórico e exigem nova revisão',async({page})=>{
  await nativeFixture(page,true,'success','none','archive-metadata');await page.goto('/?area=tecnica');
  await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Manutenção',exact:true}).click();
  const panel=page.getByRole('region',{name:'Cópias do Kali'});
  await panel.getByRole('button',{name:'Consultar cópias do Kali'}).click();
  await expect(panel.getByText('Exportando arquivos do Kali')).toBeVisible();
  await expect(panel.getByRole('button',{name:'Revisar restauração desta cópia'})).toHaveCount(0);
  await expect(panel.getByText('Identidade da tentativa indisponível.')).toBeVisible();
  await panel.getByRole('button',{name:'Revisar exportação do Kali'}).click();
  await expect(panel.getByRole('alert')).toContainText('detalhes da operação estão ocultos');
  await expect(panel.getByRole('button',{name:'Exportar cópia revisada'})).toHaveCount(0);
  expect(await calls(page,'environment.archive.export')).toHaveLength(0);
});

test('cópias do Kali anteriores sem formato exibem VHDX e exigem revisão antes de restaurar',async({page})=>{
  await nativeFixture(page,true,'success','none','archive-legacy-vhdx');
  await page.goto('/?area=tecnica');
  await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Manutenção',exact:true}).click();
  const panel=page.getByRole('region',{name:'Cópias do Kali'});
  await panel.getByRole('button',{name:'Consultar cópias do Kali'}).click();
  await panel.getByRole('button',{name:'Revisar restauração desta cópia'}).click();
  await expect(panel.getByText('VHDX · disco virtual',{exact:true})).toBeVisible();
  await expect(panel).toContainText('O espaço livre pode conter dados já excluídos');
  await expect(panel.getByRole('checkbox',{name:/conteúdo integral do disco VHDX/})).not.toBeChecked();
  await expect(panel.getByRole('button',{name:'Importar cópia revisada'})).toBeDisabled();
  await panel.getByRole('button',{name:'Fechar revisão da cópia'}).click();
  expect(await calls(page,'environment.archive.import')).toHaveLength(0);
  await expect(panel.getByRole('navigation',{name:'Páginas de cópias do Kali'})).toHaveCount(0);
});

async function openRecoveryCandidate(page: Page, mode: string) {
  await nativeFixture(page, true, 'success', 'none', mode);
  await page.goto('/?area=tecnica');
  await page.getByRole('button', { name: 'Kali e ferramentas', exact: true }).click();
  await page.getByRole('button', { name: 'Manutenção', exact: true }).click();
  const panel = page.getByRole('region', { name: 'Cópias do Kali' });
  await panel.getByRole('button', { name: 'Consultar cópias do Kali' }).click();
  await expect(panel.getByText('Cópia vinculada ao executor')).toBeVisible();
  const navigation = panel.getByRole('navigation', { name: 'Páginas de cópias do Kali' });
  await expect(navigation.getByRole('button', { name: 'Tentativas mais recentes' })).toBeDisabled();
  await navigation.getByRole('button', { name: 'Tentativas mais antigas' }).click();
  await expect(navigation).toContainText('Página 2');
  await expect(navigation.getByRole('button', { name: 'Tentativas mais antigas' })).toBeDisabled();
  return { panel, navigation, prepare: panel.getByRole('button', { name: 'Revisar uso desta cópia como executor' }) };
}

async function openSupport(page: Page, mode?: string) {
  if(mode) await nativeFixture(page,true,'success','none',mode);
  await page.goto('/');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Diagnóstico e suporte',exact:true}).click();
  return page.getByRole('region',{name:'Pacote de suporte local'});
}
test('pacote de suporte mostra os bytes exatos, exige confirmação e confere o recibo', { tag: '@source-edition' }, async ({page})=>{
  const panel=await openSupport(page,'support-ok');
  await expect(panel).toContainText('Desativada');
  expect(await calls(page,'support.review')).toEqual([]);
  const prepare=panel.getByRole('button',{name:'Preparar prévia do pacote de suporte'});
  const review=panel.getByRole('region',{name:'Revisão do pacote de suporte'});
  await prepare.click(); await expect(review).toBeFocused();
  await review.getByRole('checkbox').check(); await review.getByRole('button',{name:'Descartar prévia'}).click();
  await expect(panel.getByRole('status')).toHaveText('Revisão descartada.');
  expect(await calls(page,'support.export')).toEqual([]);
  await prepare.click(); await expect(review).toBeFocused();
  const text=await review.locator('pre').textContent(); expect(text).toMatch(/\n$/);
  const content=JSON.parse(text!);expect(content.telemetry).toBe('disabled');expect(content.remoteTransmission).toBe(false);
  const target=await review.getByText(/^exports\/suporte-/).innerText();
  const save=review.getByRole('button',{name:'Salvar pacote de suporte'});
  await expect(save).toBeDisabled();await page.keyboard.press('Tab');await expect(review.getByRole('checkbox')).toBeFocused();
  await page.keyboard.press('Space');await page.keyboard.press('Tab');await expect(save).toBeFocused();await page.keyboard.press('Enter');
  await expect(panel.getByRole('status')).toContainText('Pacote de suporte salvo');
  await expect(panel.getByText(target,{exact:true})).toBeVisible();
  const reviewId=target.slice('exports/suporte-'.length,-5);
  expect((await calls(page,'support.export')).map(c=>c.payload)).toEqual([{reviewId,confirmation:true}]);
  expect(await calls(page,'ai.test')).toEqual([]);
});
for(const mode of ['changed','expired','error','receipt']) test('pacote de suporte trata falha '+mode+' sem declarar arquivo salvo', { tag: '@source-edition' }, async ({page})=>{
  const panel=await openSupport(page,'support-'+mode);
  await panel.getByRole('button',{name:'Preparar prévia do pacote de suporte'}).click();
  const invalidReview=['changed','expired'].includes(mode);
  if (!invalidReview) {
    const review=panel.getByRole('region',{name:'Revisão do pacote de suporte'});
    await review.getByRole('checkbox').check();await review.getByRole('button',{name:'Salvar pacote de suporte'}).click();
  }
  await expect(panel.getByRole('alert')).toContainText(invalidReview?'Não foi possível conferir':mode==='error'?'indisponível':'O recibo não confirma');
  await expect(panel.getByRole('region',{name:'Revisão do pacote de suporte'})).toHaveCount(0);
  await expect(panel.getByRole('status')).not.toContainText('Pacote de suporte salvo');
  expect(await calls(page,'support.export')).toHaveLength(invalidReview?0:1);
});
test('pacote de suporte na prévia do navegador explica a disponibilidade no desktop', { tag: '@source-edition' }, async ({page})=>{
  const panel=await openSupport(page);await expect(panel).toContainText('Telemetria');await expect(panel).toContainText('Desativada');
  await expect(panel).toContainText('aplicativo desktop');await expect(panel.getByRole('button')).toHaveCount(0);
});

test('histórico paginado do Kali usa o cursor da consulta e descarta o consentimento ao mudar de página', async ({ page }) => {
  const { panel, navigation, prepare } = await openRecoveryCandidate(page, 'archive-bind');
  const pages = await calls(page, 'environment.archive.list');
  expect(pages.map(call=>call.payload)).toEqual([{}, { beforeId: expect.stringMatching(/^[0-9a-f-]{36}$/) }]);
  expect(await calls(page, 'environment.archive.activatePlan')).toHaveLength(0);
  await prepare.click();
  const review = panel.getByRole('region', { name: 'Revisão do vínculo do executor' });
  await review.getByRole('checkbox').check();
  await navigation.getByRole('button', { name: 'Tentativas mais recentes' }).click();
  await expect(review).toHaveCount(0);
  await expect(navigation).toContainText('Página 1');
  await expect(prepare).toHaveCount(0);
  await navigation.getByRole('button', { name: 'Tentativas mais antigas' }).click();
  await prepare.click();
  await expect(review.getByRole('checkbox')).not.toBeChecked();
  await expect(review.getByRole('button', { name: 'Confirmar troca de executor' })).toBeDisabled();
  await review.getByRole('button', { name: 'Fechar revisão do vínculo' }).click();
  expect(await calls(page, 'environment.archive.activate')).toHaveLength(0);
  expect((await calls(page, 'environment.archive.list')).map(call=>call.payload)).toEqual([{}, pages[1].payload, {}, pages[1].payload]);
});

test('ativar cópia do Kali exige revisão exata e atualiza o executor sem iniciar outro diagnóstico', async ({ page }) => {
  const { panel, navigation, prepare } = await openRecoveryCandidate(page, 'archive-bind');
  const initialProbes = (await calls(page, 'environment.status')).length;
  await prepare.click();
  const review = panel.getByRole('region', { name: 'Revisão do vínculo do executor' });
  const targetId = (await calls(page, 'environment.archive.activatePlan'))[0].payload.id as string;
  await expect(review).toContainText('OpenKoda-Recovery-'+targetId);
  await expect(review).toContainText('D:/Fixture/recovery/'+targetId);
  await expect(review.getByRole('listitem')).toHaveText(['Verificar a identidade da cópia antes de vincular o executor.','Preservar a distribuição anterior e suas autorizações.']);
  const submit = review.getByRole('button', { name: 'Confirmar troca de executor' });
  await expect(submit).toBeDisabled();
  await review.getByRole('checkbox').check();
  await submit.click();
  await expect(panel.getByRole('status')).toContainText('Executor vinculado a OpenKoda-Recovery-'+targetId);
  await expect(panel.getByText('Vínculo anterior do executor')).toBeVisible();
  await navigation.getByRole('button', { name: 'Tentativas mais antigas' }).click();
  await expect(panel.getByText('Cópia vinculada ao executor')).toBeVisible();
  await expect(prepare).toHaveCount(0);
  expect((await calls(page,'environment.archive.activate')).map(call=>call.payload)).toEqual([
    { planId:expect.stringMatching(/^[0-9a-f-]{36}$/), digest:'d'.repeat(64) },
  ]);
  expect(await calls(page, 'environment.status')).toHaveLength(initialProbes);
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  await expect(page.getByText(targetId,{exact:true})).toBeVisible();
  await expect(page.getByText('Consultar diagnóstico',{exact:true})).toBeVisible();
});

test('troca de executor preserva a identidade nova quando o inventário anterior termina depois', async ({page}) => {
  const {panel,prepare} = await openRecoveryCandidate(page, 'archive-bind-probe');
  await prepare.click();
  const targetId = (await calls(page,'environment.archive.activatePlan'))[0].payload.id as string;
  const review = panel.getByRole('region',{name:'Revisão do vínculo do executor'});
  await review.getByRole('checkbox').check();
  await review.getByRole('button',{name:'Confirmar troca de executor'}).click();
  await expect(panel.getByRole('status')).toContainText('Executor vinculado');
  await expect(page.locator('html')).not.toHaveAttribute('data-old-inventory-returned','true');
  await page.evaluate(() => (window as unknown as {__releaseOldInventory: () => void}).__releaseOldInventory());
  await expect(page.locator('html')).toHaveAttribute('data-old-inventory-returned','true');
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  await expect(page.getByText(targetId,{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Ferramentas',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Nenhuma ferramenta verificada'})).toBeVisible();
  await expect(page.getByText('Ferramenta do executor anterior')).toHaveCount(0);
});

for (const mode of ['hidden', 'expired', 'lost']) {
  test('troca de executor com revisão ou resposta ' + mode + ' exige nova decisão', async ({ page }) => {
    const {panel, prepare, navigation} = await openRecoveryCandidate(page, 'archive-bind-'+mode);
    await prepare.click();
    const review=panel.getByRole('region',{name:'Revisão do vínculo do executor'});
    const submit=review.getByRole('button',{name:'Confirmar troca de executor'});
    if(mode==='hidden') {
      await expect(panel.getByRole('alert')).toContainText('identidade da cópia não estão disponíveis');
      await expect(review).toHaveCount(0);
    } else if(mode==='expired') {
      await review.getByRole('checkbox').check();
      await expect(submit).toBeDisabled();
    } else {
      await review.getByRole('checkbox').check();
      await submit.click();
      await expect(panel.getByRole('alert')).toContainText('Resposta da troca indisponível');
      await expect(review).toHaveCount(0);
      await navigation.getByRole('button',{name:'Tentativas mais antigas'}).click();
      await expect(panel.getByText('Cópia vinculada ao executor')).toBeVisible();
      await expect(prepare).toHaveCount(0);
    }
    expect(await calls(page,'environment.archive.activate')).toHaveLength(mode==='lost'?1:0);
  });
}

test('preparação do Kali só aplica o plano depois da revisão',async({page})=>{
  await nativeFixture(page);await page.goto('/?area=tecnica');await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();await page.getByRole('button',{name:'Revisar preparação do Kali',exact:true}).click();
  await expect(page.getByLabel('Detalhes do plano de manutenção')).toContainText('Criar distribuição dedicada de teste');
  await expect(page.getByRole('button',{name:'Executar operação revisada',exact:true})).toBeDisabled();expect(await calls(page,'environment.provision')).toEqual([]);
  await page.getByRole('checkbox',{name:'Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.'}).check();
  await page.getByRole('button',{name:'Executar operação revisada',exact:true}).click();
  await expect(page.getByText('Preparação encerrada. Consulte o estado verificado do ambiente.',{exact:true})).toBeVisible();
  expect((await calls(page,'environment.provision'))[0].payload).toEqual({planId:'maintenance-fixture',digest:'maintenance-fixture-digest'});
});

test('preparação guiada do Windows reconsulta os requisitos após reabrir', async ({page}) => {
  await nativeFixture(page);
  async function openEnvironment() {
    await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
    await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  }
  await page.goto('/?area=tecnica');
  await openEnvironment();
  const guidance = page.getByRole('region',{name:'Preparar o Windows para o WSL'});
  await guidance.getByRole('button',{name:'Verificar requisitos do Windows'}).click();
  await expect(guidance.getByLabel('Comando manual para o administrador')).toHaveText('wsl --install --no-distribution');
  expect(await calls(page,'environment.provision')).toHaveLength(0);
  await page.reload();
  await openEnvironment();
  await expect(guidance.getByRole('status')).toContainText('O WSL respondeu');
  await expect(guidance.getByLabel('Comando manual para o administrador')).toHaveCount(0);
  expect(await calls(page,'environment.host.check')).toHaveLength(0);
  expect(await calls(page,'environment.provision')).toHaveLength(0);
});

test('imagem oficial exige escolha explícita e revisão após falha da preparação padrão',async({page})=>{
  await nativeFixture(page,false,'success','none','standard-error');await page.goto('/?area=tecnica');await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  expect(await calls(page,'environment.image.plan')).toEqual([]);
  await page.getByRole('button',{name:'Revisar preparação do Kali',exact:true}).click();
  await page.getByRole('checkbox',{name:'Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.'}).check();
  await page.getByRole('button',{name:'Executar operação revisada',exact:true}).click();
  await expect(page.locator('.single-page').getByRole('alert')).toContainText('tempo de download excedido');
  expect(await calls(page,'environment.image.plan')).toEqual([]);
  await page.getByRole('button',{name:'Preparar com imagem oficial verificada',exact:true}).click();
  const review=page.getByLabel('Detalhes do plano de manutenção');
  await expect(review).toContainText('https://fixture.example.test/kali.wsl');
  await expect(review).toContainText('a'.repeat(64));await expect(review).toContainText('247857686');
  expect((await calls(page,'environment.image.plan'))[0].payload).toEqual({});
  expect(await calls(page,'environment.image.provision')).toEqual([]);
  await expect(page.getByRole('button',{name:'Executar operação revisada',exact:true})).toBeDisabled();
  await page.screenshot({path:test.info().outputPath('kali-imagem-revisao.png'),fullPage:true});
  await page.getByRole('checkbox',{name:'Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.'}).check();
  await page.getByRole('button',{name:'Executar operação revisada',exact:true}).click();
  await expect(page.getByText('Preparação encerrada. Consulte o estado verificado do ambiente.',{exact:true})).toBeVisible();
  expect((await calls(page,'environment.image.provision'))[0].payload).toEqual({planId:'image-plan-fixture',digest:'image-plan-fixture-digest'});
  expect(await calls(page,'environment.provision')).toHaveLength(1);
});

test('hash divergente da imagem mantém erro e descarta aprovação de um plano anterior',async({page})=>{
  await nativeFixture(page,false,'success','none','mismatch');await page.goto('/?area=tecnica');await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();await page.getByRole('button',{name:'Revisar preparação do Kali',exact:true}).click();
  await page.getByRole('checkbox',{name:'Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.'}).check();
  await page.getByRole('button',{name:'Preparar com imagem oficial verificada',exact:true}).click();
  await expect(page.locator('.single-page').getByRole('alert')).toContainText('SHA-256 da imagem diverge');
  await expect(page.getByRole('button',{name:'Executar operação revisada',exact:true})).toHaveCount(0);
  expect(await calls(page,'environment.image.plan')).toHaveLength(1);
  expect(await calls(page,'environment.image.provision')).toEqual([]);expect(await calls(page,'environment.provision')).toEqual([]);
});

test('imagem alternativa não é oferecida quando o backend informa registro existente',async({page})=>{
  await nativeFixture(page,false,'success','none','registered');await page.goto('/?area=tecnica');await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Ambiente requer atenção',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Preparar com imagem oficial verificada',exact:true})).toHaveCount(0);
  expect(await calls(page,'environment.image.plan')).toEqual([]);
});

async function reviewEnvironmentRepair(page:Page,mode='repair'){
  await nativeFixture(page,false,'success','none',mode);await page.goto('/?area=tecnica');await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  expect(await calls(page,'environment.repair.plan')).toEqual([]);expect(await calls(page,'environment.repair.apply')).toEqual([]);
  await page.getByRole('button',{name:'Revisar recuperação do Kali',exact:true}).click();
  await expect(page.getByLabel('Detalhes do plano de manutenção')).toContainText('Encerrar somente OpenKoda-Kali');
}

test('recuperação Kali exige plano explícito e não presume executor pronto',async({page})=>{
  await reviewEnvironmentRepair(page);
  await expect(page.getByRole('button',{name:'Executar operação revisada',exact:true})).toBeDisabled();
  expect(await calls(page,'environment.repair.apply')).toEqual([]);
  await page.getByRole('checkbox',{name:'Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.'}).check();
  await page.getByRole('button',{name:'Executar operação revisada',exact:true}).click();
  await expect(page.getByText('Tentativa de recuperação registrada. Consulte o estado verificado do ambiente.',{exact:true})).toBeVisible();
  expect((await calls(page,'environment.repair.apply'))[0].payload).toEqual({planId:'repair-plan-fixture',digest:'repair-fixture-digest'});
  await expect(page.getByRole('button',{name:'Kali requer atenção',exact:true})).toBeVisible();
  expect(await calls(page,'packages.apply')).toEqual([]);expect(await calls(page,'environment.provision')).toEqual([]);
});

test('recuperação Kali com erro preserva diagnóstico e impede reenviar o mesmo plano',async({page})=>{
  await reviewEnvironmentRepair(page,'repair-error');
  await page.getByRole('checkbox',{name:'Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.'}).check();
  await page.getByRole('button',{name:'Executar operação revisada',exact:true}).click();
  await expect(page.locator('.single-page').getByRole('alert')).toContainText('ambiente continua degradado');
  await expect(page.getByText('Este plano já foi enviado. Consulte o diagnóstico e revise um novo plano antes de outra tentativa.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Executar operação revisada',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Atualizar diagnóstico',exact:true}).click();
  await expect(page.getByRole('button',{name:'Atualizar diagnóstico',exact:true})).toBeEnabled();
  expect(await calls(page,'environment.repair.apply')).toHaveLength(1);
  await expect(page.getByRole('button',{name:'Kali requer atenção',exact:true})).toBeVisible();
});

async function openRecovery(page:Page,mode='success'){
  await nativeFixture(page,false,'success','none','available',mode);await page.goto('/');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();await page.getByRole('button',{name:'Backup e recuperação',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Ativação de uma cópia',exact:true})).toBeVisible();
}
async function stageAndReviewRecovery(page:Page){
  await page.getByRole('button',{name:'Verificar backup',exact:true}).click();
  await page.getByRole('button',{name:'Preparar restauração',exact:true}).click();
  expect(await calls(page,'activation.plan')).toEqual([]);expect(await calls(page,'activation.schedule')).toEqual([]);
  await page.getByRole('button',{name:'Revisar ativação desta cópia',exact:true}).click();
  await expect(page.getByLabel('Revisão da ativação')).toContainText('Arquivos externos de projetos não são restaurados.');
}

test('ativação agenda a cópia revisada para próxima abertura e permite cancelar', { tag: '@source-edition' }, async ({page})=>{
  await openRecovery(page);await stageAndReviewRecovery(page);
  expect((await calls(page,'activation.plan'))[0].payload).toEqual({recoveryId:'recovery-receipt-fixture'});
  await expect(page.getByRole('button',{name:'Agendar ativação revisada',exact:true})).toBeDisabled();
  await page.getByRole('checkbox',{name:'Revisei os perfis, os arquivos e os efeitos. Autorizo esta troca na próxima abertura do aplicativo.'}).check();
  await page.screenshot({path:test.info().outputPath('ativacao-revisao.png'),fullPage:true});
  await page.getByRole('button',{name:'Agendar ativação revisada',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Troca pendente',exact:true})).toBeVisible();
  await expect(page.getByText('O processo atual conserva o perfil original.',{exact:true})).toBeVisible();
  expect((await calls(page,'activation.schedule'))[0].payload).toEqual({id:'activation-fixture',digest:'activation-fixture-digest'});
  expect(await calls(page,'settings.save')).toEqual([]);
  await page.getByRole('button',{name:'Cancelar troca agendada',exact:true}).click();
  await expect(page.getByText('Agendamento cancelado. O perfil atual foi preservado.',{exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Troca pendente',exact:true})).toHaveCount(0);
  expect((await calls(page,'activation.cancel'))[0].payload).toEqual({id:'activation-fixture',digest:'activation-fixture-digest'});
});

test('ativação recusada por integridade exige nova revisão sem repetir o agendamento', { tag: '@source-edition' }, async ({page})=>{
  await openRecovery(page,'schedule-error');await stageAndReviewRecovery(page);
  await page.getByRole('checkbox',{name:'Revisei os perfis, os arquivos e os efeitos. Autorizo esta troca na próxima abertura do aplicativo.'}).check();
  await page.getByRole('button',{name:'Agendar ativação revisada',exact:true}).click();
  await expect(page.locator('.recovery-activation').getByRole('alert')).toContainText('SHA-256 da cópia divergiu');
  await expect(page.getByRole('button',{name:'Agendar ativação revisada',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Atualizar estado da recuperação',exact:true}).click();
  await expect(page.getByRole('button',{name:'Atualizar estado da recuperação',exact:true})).toBeEnabled();
  expect(await calls(page,'activation.schedule')).toHaveLength(1);
  await expect(page.getByRole('heading',{name:'Troca pendente',exact:true})).toHaveCount(0);
});

test('retorno ao perfil anterior prepara nova cópia e conserva revisão explícita', { tag: '@source-edition' }, async ({page})=>{
  await openRecovery(page,'rollback');
  expect(await calls(page,'activation.rollbackPlan')).toEqual([]);
  await page.getByRole('button',{name:'Revisar retorno ao perfil anterior',exact:true}).click();
  const review=page.getByLabel('Revisão da ativação');
  await expect(review).toContainText('Perfil previous-profile-fixture');await expect(review).toContainText('Perfil new-sanitized-fixture');
  await expect(review).toContainText('Credenciais não são transferidas.');
  await expect(page.getByRole('button',{name:'Agendar ativação revisada',exact:true})).toBeDisabled();
  expect(await calls(page,'activation.schedule')).toEqual([]);
  expect((await calls(page,'activation.rollbackPlan'))[0].payload).toEqual({});
});

test('ativação bloqueada preserva aviso e só pode ser cancelada explicitamente', { tag: '@source-edition' }, async ({page})=>{
  await openRecovery(page,'blocked');
  await expect(page.getByRole('heading',{name:'Agendamento bloqueado',exact:true})).toBeVisible();
  await expect(page.getByText('Integridade da cópia não confirmada; perfil original preservado.',{exact:true})).toBeVisible();
  expect(await calls(page,'activation.cancel')).toEqual([]);expect(await calls(page,'activation.schedule')).toEqual([]);
  await page.getByRole('button',{name:'Cancelar troca agendada',exact:true}).click();
  await expect(page.getByText('Agendamento cancelado. O perfil atual foi preservado.',{exact:true})).toBeVisible();
  await expect(page.getByRole('heading',{name:'Agendamento bloqueado',exact:true})).toHaveCount(0);
});

test('candidato só é validado com revisão humana e evidência selecionada', { tag: '@source-edition' }, async ({page})=>{
  await nativeFixture(page);await page.goto('/');await page.getByRole('button',{name:'Achados',exact:true}).click();
  await page.getByText('Candidato de contrato',{exact:true}).click();
  expect(await calls(page,'finding.validate')).toEqual([]);
  await page.getByRole('button',{name:'Validar candidato',exact:true}).click();
  await expect(page.getByRole('button',{name:'Confirmar validação',exact:true})).toBeDisabled();
  await page.getByLabel('Comportamento esperado',{exact:true}).fill('O cabeçalho deve declarar o contrato esperado.');
  await page.getByLabel('Comportamento observado',{exact:true}).fill('A evidência registra o cabeçalho fixture.');
  await page.getByRole('checkbox',{name:'Cabeçalhos de contrato',exact:true}).uncheck();
  await expect(page.getByRole('button',{name:'Confirmar validação',exact:true})).toBeDisabled();
  await page.getByRole('checkbox',{name:'Cabeçalhos de contrato',exact:true}).check();
  await page.getByRole('button',{name:'Confirmar validação',exact:true}).click();
  await expect(page.getByText('Validado',{exact:true}).first()).toBeVisible();
  await page.getByRole('button',{name:'Histórico',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Validação registrada',exact:true})).toBeVisible();
  expect((await calls(page,'finding.validate'))[0].payload).toMatchObject({id:'finding-fixture',method:'human_review',evidenceIds:['evidence-fixture']});
});

test('registro manual conserva candidato separado de validação',async({page})=>{
  await nativeFixture(page);await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.getByRole('navigation',{name:'Seções',exact:true}).getByRole('button',{name:'Achados',exact:true}).click();
  await page.getByRole('button',{name:'Registrar candidato',exact:true}).click();
  await page.getByLabel('Título do candidato',{exact:true}).fill('Observação para revisão');
  await page.getByLabel('Descrição do candidato',{exact:true}).fill('A observação requer confirmação humana.');
  await expect(page.getByRole('button',{name:'Salvar candidato',exact:true})).toBeDisabled();
  await page.getByRole('checkbox',{name:'Cabeçalhos de contrato',exact:true}).check();
  await page.getByRole('button',{name:'Salvar candidato',exact:true}).click();
  await expect(page.getByText('Observação para revisão',{exact:true})).toBeVisible();
  expect((await calls(page,'candidate.record'))[0].payload).toMatchObject({assessmentId:'assessment-fixture',evidenceIds:['evidence-fixture'],severity:'medium'});
  expect(await calls(page,'finding.validate')).toEqual([]);
});

test('bridge Kali limita a UI a diagnóstico e hash da evidência selecionada',async({page})=>{
  await nativeFixture(page,true);await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();
  const panel=page.locator('.kali-bridge');
  expect(await calls(page,'kali.mcp.diagnose')).toEqual([]);expect(await calls(page,'kali.mcp.hashEvidence')).toEqual([]);
  await panel.getByRole('button',{name:'Diagnosticar Kali nesta avaliação',exact:true}).click();
  await expect(panel.getByLabel('Resultado do diagnóstico ou hash')).toContainText('Kali de contrato');
  await expect(panel.getByText('Confirmada',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Kali preparado',exact:true})).toBeVisible();
  await panel.getByLabel('Evidência para verificar no Kali',{exact:true}).selectOption('evidence-fixture');
  await reviewKaliData(panel,'hash');await panel.getByRole('button',{name:'Verificar hash no Kali',exact:true}).click();
  await expect(panel.getByLabel('Resultado do diagnóstico ou hash')).toContainText('fixture-hash');
  expect((await calls(page,'kali.mcp.hashEvidence'))[0].payload).toMatchObject({assessmentId:'assessment-fixture',evidenceId:'evidence-fixture',acknowledgeLocalTransfer:true});
  expect(await calls(page,'plan.run')).toEqual([]);expect(await calls(page,'ai.ask')).toEqual([]);
  await page.screenshot({path:test.info().outputPath('kali-bridge.png'),fullPage:true});
});

test('falha do bridge fica visível e consultar estado não repete a operação',async({page})=>{
  await nativeFixture(page,true,'failure');await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();
  const panel=page.locator('.kali-bridge');
  await panel.getByRole('button',{name:'Diagnosticar Kali nesta avaliação',exact:true}).click();
  await expect(panel.getByRole('alert')).toContainText('Falha de comunicação com o bridge de teste.');
  await expect(panel.getByText('Falhou',{exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Consultar estado da conexão',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Consultar estado da conexão',exact:true})).toBeEnabled();
  expect(await calls(page,'kali.mcp.diagnose')).toHaveLength(1);
  await expect(panel.getByText('Concluído',{exact:true})).toHaveCount(0);
});

test('limite do hash no Kali é medido em bytes UTF-8 antes do envio',async({page})=>{
  await nativeFixture(page,true,'oversized');await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();
  const panel=page.locator('.kali-bridge');
  await panel.getByLabel('Evidência para verificar no Kali',{exact:true}).selectOption('evidence-fixture');
  await expect(panel.getByText(/Acima do limite de 32 KiB/)).toBeVisible();
  await expect(panel.getByRole('button',{name:'Verificar hash no Kali',exact:true})).toBeDisabled();
  expect(await calls(page,'kali.mcp.hashEvidence')).toEqual([]);
});

test('cancelamento do bridge permanece solicitado até o processo confirmar término',async({page})=>{
  await nativeFixture(page,true,'cancel');await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();
  const panel=page.locator('.kali-bridge');
  await panel.getByRole('button',{name:'Diagnosticar Kali nesta avaliação',exact:true}).click();
  await panel.getByRole('button',{name:'Solicitar cancelamento da sessão',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Cancelamento solicitado',exact:true})).toBeDisabled();
  await expect(panel.getByText('Cancelado',{exact:true})).toHaveCount(0);
  expect(await calls(page,'kali.mcp.cancel')).toHaveLength(1);
  await page.evaluate(()=>(window as unknown as {__finishBridge:()=>void}).__finishBridge());
  await expect(panel.getByText('Cancelado',{exact:true}).first()).toBeVisible();
  await expect(panel.getByText('Confirmada',{exact:true})).toBeVisible();
  expect(await calls(page,'kali.mcp.diagnose')).toHaveLength(1);
});

test('sessão desconhecida só tenta reconciliação após ação explícita e preserva recusa',async({page})=>{
  await nativeFixture(page,true,'unknown-error');await page.goto('/?area=tecnica');await openAssessmentList(page);await page.getByText('Avaliação de contrato',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();
  const panel=page.locator('.kali-bridge');
  await expect(panel.getByText('Efeito desconhecido',{exact:true})).toBeVisible();
  await expect(panel.getByRole('button',{name:'Diagnosticar Kali nesta avaliação',exact:true})).toBeDisabled();
  expect(await calls(page,'kali.mcp.reconcile')).toEqual([]);
  await panel.getByRole('button',{name:'Reconciliar sessão interrompida',exact:true}).click();
  await expect(panel.getByRole('alert')).toContainText('Identidade da sessão não confirmada');
  await expect(panel.getByText('Efeito desconhecido',{exact:true})).toBeVisible();
  expect((await calls(page,'kali.mcp.reconcile'))[0].payload).toEqual({runId:'bridge-interrupted',assessmentId:'assessment-fixture'});
  expect(await calls(page,'kali.mcp.diagnose')).toEqual([]);
});

async function prepareRemediation(page:Page,mode='success'){
  await nativeFixture(page,false,'success',mode);await page.goto('/?area=tecnica');
  await page.getByRole('button',{name:'Achados',exact:true}).click();await page.getByText('Candidato de contrato',{exact:true}).click();
  await page.getByRole('button',{name:'Correção',exact:true}).click();
  await page.getByRole('button',{name:'Preparar correção local',exact:true}).click();
  await page.getByLabel('Caminho relativo do arquivo',{exact:true}).fill('src/access.ts');
  await page.getByLabel('Conteúdo proposto',{exact:true}).fill('permit = false\n');
  await page.getByLabel('Condição do reteste',{exact:true}).selectOption('absent');
  await page.getByLabel('Texto da condição',{exact:true}).fill('permit = true');
  await page.getByRole('button',{name:'Preparar revisão da alteração',exact:true}).click();
  await expect(page.getByLabel('Conteúdo original do arquivo',{exact:true})).toContainText('Conteúdo omitido');
  await expect(page.getByLabel('Conteúdo proposto do arquivo',{exact:true})).toContainText('Conteúdo omitido');
}
async function approveRemediation(page:Page){
  await page.getByRole('button',{name:'Revisar trecho armazenado',exact:true}).click();await page.getByRole('button',{name:'Confirmar leitura desta janela',exact:true}).click();await page.getByRole('checkbox',{name:'Revisei o arquivo, o conteúdo antes e depois e a condição de reteste. Autorizo esta alteração específica.'}).check();
  await page.getByRole('button',{name:'Aprovar alteração revisada',exact:true}).click();
}

test('correção exige revisão, aplicação, reteste e confirmação humana separados',async({page})=>{
  await prepareRemediation(page);
  await page.screenshot({path:test.info().outputPath('correcao-revisao.png'),fullPage:true});
  await expect(page.getByRole('button',{name:'Aprovar alteração revisada',exact:true})).toBeDisabled();
  expect(await calls(page,'remediation.approve')).toEqual([]);expect(await calls(page,'remediation.apply')).toEqual([]);
  await approveRemediation(page);
  expect((await calls(page,'remediation.approve'))[0].payload).toEqual({id:'remediation-fixture',digest:'remediation-fixture-digest'});
  expect(await calls(page,'remediation.apply')).toEqual([]);
  await page.getByRole('button',{name:'Aplicar alteração aprovada',exact:true}).click();
  await expect(page.locator('.detail-heading').getByText('Validado',{exact:true})).toBeVisible();
  expect(await calls(page,'remediation.retest')).toEqual([]);
  await page.getByRole('button',{name:'Executar reteste de texto ou hash',exact:true}).click();
  await expect(page.getByText('Atendida',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Confirmar correção revisada',exact:true})).toBeDisabled();
  expect(await calls(page,'remediation.confirm')).toEqual([]);
  await page.getByLabel('Justificativa da confirmação',{exact:true}).fill('A condição textual revisada passou. A confirmação se limita a esse comportamento observado.');
  await page.getByRole('checkbox',{name:'Revisei a evidência e os limites do reteste e confirmo a correção deste achado neste alcance.'}).check();
  await page.getByRole('button',{name:'Confirmar correção revisada',exact:true}).click();
  await expect(page.locator('.detail-heading').getByText('Corrigido',{exact:true})).toBeVisible();
  expect((await calls(page,'remediation.confirm'))[0].payload).toMatchObject({id:'remediation-fixture',digest:'remediation-fixture-digest',evidenceId:'retest-fixture'});
});

test('arquivo alterado após revisão invalida aplicação sem repetição automática',async({page})=>{
  await prepareRemediation(page,'stale');await approveRemediation(page);
  await page.getByRole('button',{name:'Aplicar alteração aprovada',exact:true}).click();
  await expect(page.locator('.remediation').getByRole('alert')).toContainText('Arquivo mudou desde a revisão');
  await expect(page.getByText('Invalidada',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Atualizar histórico de correções',exact:true}).click();
  await expect(page.getByRole('button',{name:'Atualizar histórico de correções',exact:true})).toBeEnabled();
  expect(await calls(page,'remediation.apply')).toHaveLength(1);expect(await calls(page,'remediation.retest')).toEqual([]);
  await expect(page.locator('.detail-heading').getByText('Validado',{exact:true})).toBeVisible();
});

test('reteste negativo mantém achado validado e bloqueia confirmação de correção',async({page})=>{
  await prepareRemediation(page,'negative');await approveRemediation(page);
  await page.getByRole('button',{name:'Aplicar alteração aprovada',exact:true}).click();
  await page.getByRole('button',{name:'Executar reteste de texto ou hash',exact:true}).click();
  await expect(page.getByText('Não atendida',{exact:true})).toBeVisible();
  await expect(page.getByText('A confirmação está bloqueada porque a condição não foi comprovada.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Confirmar correção revisada',exact:true})).toHaveCount(0);
  expect(await calls(page,'remediation.confirm')).toEqual([]);
  await expect(page.locator('.detail-heading').getByText('Validado',{exact:true})).toBeVisible();
});
