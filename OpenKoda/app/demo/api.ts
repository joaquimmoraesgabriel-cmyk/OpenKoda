import { emptySnapshot } from '../src/preview';
import { providers, type AiSelection, type ProviderId } from '../src/ai-provider';
import { scopeMethods } from '../src/scope';
import type { Assessment, Payload } from '../src/types';
import type { SecurityScan, ScanResult } from '../src/security-scan';
import { announce } from './main';

// Build-only substitute for the Tauri transport. All records live in this frame.
export const native = true;
const now = () => new Date().toISOString();
const later = () => new Date(Date.now() + 3_600_000).toISOString();
let sequence = 0;
const id = (prefix: string) => `${prefix}-${++sequence}`;
const clone = <T,>(value: T): T => structuredClone(value);
const sleep = (milliseconds: number) => new Promise(resolve => setTimeout(resolve, milliseconds));
const s = emptySnapshot();
const defaultSelection: AiSelection = { provider: 'openkoda', model: 'Aurora · exemplo', reasoning: 'medium' };
s.settings = { ...s.settings, ...defaultSelection, endpoint: providers.openkoda.endpoint, hasApiKey: true };
s.projection.mode = 'ready';
s.projects = [{ id: 'aurora', name: 'Loja Aurora', kind: 'code', target: 'D:/Exemplos/loja-aurora',
  description: 'Uma loja de exemplo. O objetivo é revisar autenticação, pedidos e dados enviados à IA.', createdAt: now() }];
function conversation(projectId: string, title: string, selection = defaultSelection): Assessment {
  const project = s.projects.find(p => p.id === projectId)!;
  return { id: id('conversa'), projectId, title, kind: 'code', target: project.target,
    objective: 'Revisar o acesso aos pedidos da loja de exemplo.', aiSelection: clone(selection),
    createdAt: now(), updatedAt: now(), status: 'draft', events: [], evidence: [], coverage: [], summary: '' };
}
s.assessments = [conversation('aurora', 'Entender o acesso aos pedidos'), conversation('aurora', 'Relatório de segurança')];
const first = s.assessments[0];
const reportConversation = s.assessments[1];
reportConversation.createdAt = reportConversation.updatedAt = new Date(Date.now()-3_600_000).toISOString();
type Message = {id: string; role: 'user' | 'assistant'; text: string; createdAt: string};
const histories = new Map<string, Message[]>([[first.id, [
  { id: id('mensagem'), role: 'user', text: 'Onde devo começar a revisão de segurança desta loja?', createdAt: now() },
  { id: id('mensagem'), role: 'assistant', text: 'Comece pelo caminho de um pedido: **quem está conectado, qual pedido foi solicitado e quem pode acessá-lo**.\n\nNo exemplo, a rota `src/routes/orders.ts` precisa comparar o usuário da sessão com o proprietário do pedido antes de retornar os dados.\n\nVocê pode explorar o relatório, criar uma conversa ou escrever uma pergunta abaixo. Esta resposta é pré-definida; nenhum código real foi analisado.', createdAt: now() },
]]]);
const reply = 'Neste exemplo, o ponto de atenção está em `src/routes/orders.ts`, na consulta de um pedido pelo identificador.\n\n**O que revisar**\n\n1. Obter a identidade da sessão no servidor.\n2. Restringir a consulta ao proprietário autorizado.\n3. Testar uma conta tentando acessar o pedido de outra conta.\n\nA proposta é conferir a autorização antes de retornar o pedido. A conclusão deve ser validada no contexto do projeto.\n\nEsta é uma resposta pré-definida da demonstração. Nenhuma mensagem foi enviada a um provedor e nenhum crédito foi consumido.';
const report = '# Revisão de exemplo: acesso aos pedidos\n\n## Escopo observado\n\n`src/routes/orders.ts` e `src/auth/session.ts` — dados fictícios preparados para esta demonstração.\n\n## Candidato: pedido sem verificação de proprietário\n\nA rota recebe um identificador e consulta o pedido. Neste exemplo, falta restringir o resultado ao usuário da sessão.\n\n```ts\nconst order = await orders.findById(params.id);\n// Antes de retornar, confira a permissão do usuário.\n```\n\n## Como validar\n\nCrie dois usuários de teste, cada um com um pedido, e confirme que um não consegue ler o pedido do outro.\n\n## Limitações\n\nEste relatório é pré-definido. A demonstração não examina arquivos, não executa testes e não comprova uma vulnerabilidade em um projeto real.';
function scanResult(scanId: string): ScanResult { return { scanId, report, findings: [{ title: 'Conferir autorização ao consultar um pedido', severity: 'medium', location: 'src/routes/orders.ts:18', description: 'Candidato fictício: a consulta precisa verificar o proprietário do pedido. Valide o fluxo antes de tratar como vulnerabilidade confirmada.' }], coverage: { scope: ['src/routes/orders.ts', 'src/auth/session.ts'], reviewedRanges: ['src/routes/orders.ts:12–26', 'src/auth/session.ts:1–38'], excludedPaths: ['.env', 'node_modules/'], additionalExclusions: 0, findingsTotal: 1, reportTruncated: false, summary: 'Dois arquivos de exemplo, um candidato para revisão.', limitations: ['Relatório fictício. Nenhum arquivo real foi lido.'] }, cost: { estimatedUsd: 0 } }; }
function createScan(assessmentId: string, status: SecurityScan['status']): SecurityScan {
  const selection = assessment(assessmentId).aiSelection ?? defaultSelection;
  const scanId = id('analise');
  return { id: scanId, assessmentId, status, stage: status === 'completed' ? 'report' : 'preparing', message: status === 'completed' ? 'Relatório de exemplo pronto para revisão.' : 'Preparando os dois arquivos de exemplo…', events: [], createdAt: now(), updatedAt: now(), provider: selection.provider, model: selection.model, maxCostUsd: 0, selectedPaths: ['src/routes/orders.ts', 'src/auth/session.ts'], result: status === 'completed' ? scanResult(scanId) : null, error: null };
}
const scans = new Map<string, SecurityScan>([[reportConversation.id, createScan(reportConversation.id, 'completed')]]);
first.events.push({ id: id('evento'), at: now(), kind: 'chat_first_turn', message: 'Conversa criada. Exemplo local.' });
reportConversation.events.push({ id: id('evento'), at: now(), kind: 'info', message: 'Análise de exemplo concluída.', securityScan: { id: scans.get(reportConversation.id)!.id, status: 'completed', createdAt: now(), findingsTotal: 1 } } as Assessment['events'][number]);
const live = new Map<string, {requestId:string;stopping:boolean;phase:string;partialText:string;processingMs:number;messageId:string;dataReview:unknown;start:number;stopped:boolean}>();
const contexts = new Map<string, Payload>();
const receipts = new Map<string, unknown>();
const activeProviders = new Set<ProviderId>(['openrouter','openai']);
let signed = true, minimize = false, marketing = false, renewalCancelled = false;
const tickets = [{ id: 'chamado-exemplo', subject: 'Como acompanhar uma revisão?', message: 'Gostaria de entender a diferença entre candidato e resultado confirmado.', createdAt: now(), status: 'answered', replies: [{ id: 'resposta-exemplo', message: 'Abra o relatório da análise e consulte a cobertura. Um candidato precisa de validação no contexto do projeto.', createdAt: now() }] }];
function assessment(value: unknown) { const a = s.assessments.find(a => a.id === value); if (!a) throw new Error('Escolha uma conversa da demonstração.'); return a; }
function selectionFor(p: Payload) { return s.assessments.find(a => a.id === (p.assessmentId ?? p.id))?.aiSelection ?? defaultSelection; }
function scope(a: Assessment) {
  const catalog = scopeMethods.map(method => ({ id: method, label: method, allowed: ['source.rules.v1','source.capture.v1','ai.remote-review.v1'].includes(method), reasonCodes: [], effects: 'Simulação local, sem operações reais.', windowEnforcement: 'admission_only' }));
  return { schema:'openkoda.scope-status.v1', assessmentId:a.id, projectId:a.projectId, stateDigest:'escopo-'+a.id, phase:'frozen', revision:{id:'revisao-'+a.id,assessmentId:a.id,number:1,previousRevisionId:null,body:{objective:a.objective,kind:'code',target:a.target,methods:['source.rules.v1','source.capture.v1','ai.remote-review.v1'],window:{startsAt:null,endsAt:null}},createdAt:now(),origin:'human_review',review:null,digest:'digest-'+a.id}, legacySummary:null, frozen:null,revocation:null,origin:null,windowStatus:'unbounded',evaluatedAt:now(),methodCatalog:catalog,actions:{canSave:false,canAdopt:false,canDerive:true,canRevoke:true,canPrepareWork:true,canReviewStoredEvidence:true,canImportEvidence:true,reasonCodes:[]},freezeReasons:[],workGates:catalog.map(c=>({method:c.id,allowedByScope:c.allowed,reasonCodes:[]})),limits:['Demonstração sem acesso a arquivos.'] };
}
function policy(p: Payload) {
  const a = s.assessments.find(a => a.id === p.assessmentId), projectId = String(p.projectId ?? a?.projectId ?? 'aurora');
  const body = { schema:'openkoda.data-policy.v1', destinations:['remote_ai','report_export','local_kali'].map(destination=>({destination,enabled:true,categories:['human_text','target_metadata','execution_summary','source_excerpt'],sensitivities:['internal','public'],targetCatalogIds:['primary'],allowOriginalEvidence:false})),localRawView:{enabled:true,sensitivities:['internal','public']},recoveryBackup:{enabled:false,sensitivities:[]},redactorVersion:'openkoda.redactor.v1' };
  const revision = {id:'politica-'+projectId,projectId,number:1,body,digest:'politica-demo',createdAt:now()};
  return {schema:'openkoda.data-policy-status.v1',projectId,assessmentId:a?.id??null,stateDigest:'politica-demo',generation:1,active:true,currentRevision:revision,pinnedRevision:a?revision:null,binding:a?{assessmentId:a.id,projectId,policyRef:{projectId,revisionId:revision.id,digest:revision.digest},adoptedAt:now(),generation:1,origin:'demo',frozen:null}:null,defaultBody:body,categoryCatalog:[],destinationCatalog:[],sourceOptions:[{targetCatalogId:'primary',label:'Arquivos fictícios da loja',kind:'code',inactive:false}],actions:{canPrepare:true,canAdopt:false,canRebind:false,canRevoke:false,canReactivate:false},reasonCodes:[],limits:['Todas as alterações ficam somente nesta demonstração.']};
}
function account() { return {plan:{id:'plus',priceBrlCents:21900,monthlyLimitMicros:20000000},subscription:{status:'active',mode:'production',periodStart:now(),periodEnd:new Date(Date.now()+25*86400000).toISOString(),cancelAtPeriodEnd:renewalCancelled},usage:{monthlyLimitMicros:20000000,monthlyUsedMicros:3200000,windowLimitMicros:4600000,windowUsedMicros:940000,windowEnd:new Date(Date.now()+4*86400000).toISOString()},entitlements:{reasoning:true,skills:true,hostedInference:true},controls:{inferenceEnabled:false,paymentsEnabled:false,mockCheckout:false},consent:{termsVersion:'demo',privacyVersion:'demo',requiredTermsVersion:'demo',requiredPrivacyVersion:'demo',marketing}}; }
function usageRows() { return Array.from({length:18},(_,i)=>({day:new Date(Date.now()-(17-i)*86400000).toISOString().slice(0,10),provider:i%3?'openkoda':'openrouter',model:i%3?'Aurora · exemplo':'Atlas · exemplo',assessmentId:first.id,requests:i%4+1,input:1250+i*170,output:310+i*37,partial:0})); }
function budget() { const balance={limitUsd:'10',pricedUsageUsd:'1.24',heldUsd:'0',assumedChargeUsd:'0',remainingUsd:'8.76',unresolvedCount:0};return {enabled:false,degraded:false,revision:'demo',epochId:'demo',currency:'USD',policy:{installationLimitUsd:'10',assessmentLimitUsd:'2',prices:[]},installation:balance,assessment:balance,legacyCostsUnknown:false,legacyReviewDigest:'demo',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:usageRows().map((r,i)=>({chargeId:'uso-'+i,model:r.model,endpoint:providers[r.provider as ProviderId].endpoint,createdAt:r.day+'T12:00:00Z',state:'priced',reservedUsd:'0',pricedUsageUsd:'0.03',assumedChargeUsd:'0',inputTokens:r.input,outputTokens:r.output})),recentChargesTotal:18,notice:'Números fictícios para explorar o histórico. A demonstração não consome créditos.'}; }
const models = ['Aurora · exemplo','Atlas · exemplo','Orion · exemplo'];
const providerModels = new Map<ProviderId,string[]>(Object.keys(providers).map(provider=>[provider as ProviderId,[...models]]));
const revisions = new Map<ProviderId,string>();
const verifiedProviders = new Set<ProviderId>();
function providerStatus(provider:ProviderId) {return {provider,endpoint:providers[provider].endpoint,hasApiKey:activeProviders.has(provider)||provider==='openkoda',enabledModels:providerModels.get(provider)??[],credentialRevision:revisions.get(provider)??'credential-demo',model:s.settings.provider===provider?s.settings.model:providerModels.get(provider)?.[0]??'',verified:verifiedProviders.has(provider),checkedAt:verifiedProviders.has(provider)?now():undefined};}
function credentials(p:Payload) { return {projectId:p.projectId,stateDigest:'demo',references:Object.keys(providers).map(provider=>({id:'credencial-'+provider,projectId:p.projectId,kind:'provider_reference',label:providers[provider as ProviderId].name,purpose:'Referência fictícia para demonstração',createdAt:now(),revisionId:'demo',digest:'demo',capability:'provider_slot_reference',revoked:false,providerSlotRef:{provider,slotId:'slot-'+provider},providerState:{available:true,credentialRevision:'demo',connectionVerified:false}})),providerSlots:Object.keys(providers).map(provider=>({id:'slot-'+provider,provider,available:true,credentialRevision:'demo',connectionVerified:false})),limits:[]}; }

export async function request<T>(action:string,p:Payload={}):Promise<T> {
  let value: unknown;
  switch(action) {
    case 'snapshot': value=s; break;
    case 'data.session.select': value={selected:true};break;
    case 'projectPurge.status':value={hasUnresolved:false,hasPartialHistory:false,journals:[],limits:['A demonstração não exclui dados reais.'],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};break;
    case 'window.behavior.get':value={minimize};break;
    case 'window.behavior.set':minimize=p.minimize===true;value={minimize};break;
    case 'project.pickTarget':announce('Pasta de exemplo selecionada. Nenhuma pasta do seu dispositivo foi aberta.');value='D:/Exemplos/projeto-demonstracao';break;
    case 'github.repository.import':value={id:id('repositorio'),status:'ready',path:'D:/Exemplos/repositorio-demonstracao',name:'repositorio-demonstracao',url:'https://github.com/exemplo/repositorio-demonstracao',revision:'exemplo-local',replayed:false,message:'Cópia fictícia pronta. A URL digitada não foi acessada.'};announce('Repositório fictício usado. Nenhuma conexão com o GitHub foi feita.');break;
    case 'project.launch': {
      const key=String(p.idempotencyKey);if(receipts.has(key)){value=receipts.get(key);break;}
      const projectId=id('projeto');s.projects.unshift({id:projectId,name:String(p.name||'Projeto de demonstração'),kind:'code',target:'D:/Exemplos/'+projectId,description:String(p.objective??''),createdAt:now()});
      const a=conversation(projectId,String(p.title||'Primeira revisão'),p.aiSelection as AiSelection ?? defaultSelection);s.assessments.unshift(a);histories.set(a.id,[]);
      value={projectId,assessmentId:a.id,replayed:false,targetRef:null,scopeRef:{assessmentId:a.id,revisionId:'revisao-'+a.id,digest:'digest-'+a.id}};receipts.set(key,value);break;
    }
    case 'project.newConversation': { const key=String(p.idempotencyKey);if(receipts.has(key)){value=receipts.get(key);break;}const original=s.assessments.find(a=>a.id===p.sourceAssessmentId);const a=conversation(String(p.projectId),String(p.title||'Nova conversa'),p.aiSelection as AiSelection ?? original?.aiSelection??defaultSelection);if(p.firstTurn)a.events.push({id:id('evento'),at:now(),kind:'chat_first_turn',message:'Conversa criada. Exemplo local.'});s.assessments.unshift(a);histories.set(a.id,[]);value=a;receipts.set(key,a);break; }
    case 'project.renameConversation':assessment(p.assessmentId).title=String(p.title);value={id:p.assessmentId};break;
    case 'project.archive': { const project=s.projects.find(project=>project.id===p.id);if(project)project.archived=true;value={ok:true};break; }
    case 'project.options.save': {const project=s.projects.find(project=>project.id===p.projectId);if(project)Object.assign(project,{analysisOptions:p.options});value={ok:true};break;}
    case 'settings.save': Object.assign(s.settings,p);value=s.settings;break;
    case 'ai.status':value=providerStatus(String(p.provider??s.settings.provider) as ProviderId);break;
    case 'ai.models':value=p.reasoningModel?{subscribed:signed,reasoningLevels:['low','medium','high']}:{provider:p.provider??s.settings.provider,models,credentialRevision:revisions.get(p.provider as ProviderId)??'credential-demo'};break;
    case 'account.models':value={models:models.map(model=>({id:model,reasoningLevels:['low','medium','high'],image:true,contextLength:32000,maxOutputTokens:4096}))};break;
    case 'ai.selection.save':assessment(p.assessmentId).aiSelection=clone(p.aiSelection as AiSelection);value={saved:true};break;
    case 'ai.provider.configure': {
      const provider=p.provider as ProviderId,chosen=Array.isArray(p.models)?p.models.filter((model):model is string=>typeof model==='string'&&models.includes(model)):[];
      if(!chosen.length)throw new Error('Selecione um modelo de exemplo.');
      if(p.credentialRevision!==(revisions.get(provider)??'credential-demo'))throw new Error('A conexão fictícia mudou. Abra o catálogo novamente.');
      providerModels.set(provider,chosen);if(typeof p.defaultModel==='string'&&chosen.includes(p.defaultModel))Object.assign(s.settings,{provider,model:p.defaultModel,endpoint:providers[provider].endpoint,hasApiKey:true});
      value=providerStatus(provider);break;
    }
    case 'ai.key.save': {const provider=p.provider as ProviderId;activeProviders.add(provider);revisions.set(provider,id('credential'));providerModels.set(provider,[]);verifiedProviders.delete(provider);announce('Conexão simulada. Nenhuma chave foi salva ou enviada.');value={saved:true};break;}
    case 'ai.key.remove':activeProviders.delete(p.provider as ProviderId);verifiedProviders.delete(p.provider as ProviderId);value={removed:true};break;
    case 'ai.test':verifiedProviders.add(s.settings.provider as ProviderId);announce('Teste fictício concluído. Nenhum provedor foi consultado.');value={verified:true};break;
    case 'assessment.scopeStatus':value=scope(assessment(p.assessmentId));break;
    case 'data.policy.status':value=policy(p);break;
    case 'data.classification.subjects':value={assessmentId:p.assessmentId,stateDigest:'demo',subjects:[]};break;
    case 'credential.reference.list':value=credentials(p);break;
    case 'data.review.cancel':value={cancelled:true};break;
    case 'ai.context': {
      const selection=selectionFor(p),endpoint=providers[selection.provider].endpoint;
      const review={id:id('pacote'),digest:id('digest'),expiresAt:later()};contexts.set(review.id,clone(p));
      value={context:{message:p.message},digest:review.digest,bytes:new TextEncoder().encode(String(p.message)).length,endpoint,sourceFilesIncluded:false,dataReview:review,canSend:true,reasonCodes:[],...(p.compactHistory?{compactedTurns:histories.get(String(p.assessmentId))?.length??0}:{}),packetPreview:{schema:'openkoda.ai-packet-preview.v1',operation:'ask',model:selection.model,endpoint,requestEndpoint:endpoint+'/chat/completions',prompts:[{index:0,prompt:String(p.message),instructions:'Resposta local pré-definida para explorar a interface.',maxOutputTokens:4096}],selection,policyVersions:{demo:true},credentialRef:{demo:true},requiresSensitiveConfirmation:false}};break;
    }
    case 'ai.history':value={messages:histories.get(String(p.assessmentId))??[],requests:[]};break;
    case 'ai.chat.status': {const state=live.get(String(p.assessmentId));value=state?{...state,processingMs:Date.now()-state.start}:{requestId:null,stopping:false};break;}
    case 'ai.ask': {
      const a=assessment(p.assessmentId);if(live.has(a.id))throw new Error('A resposta de exemplo já está em andamento.');
      const history=histories.get(a.id)??[];histories.set(a.id,history);
      if(!history.length&&a.title==='Nova conversa')a.title=String(p.message??'Nova conversa').trim().replace(/\s+/g,' ').slice(0,70);
      const message:Message={id:id('mensagem'),role:'user',text:String(p.message??''),createdAt:now()};history.push(message);
      const state={requestId:id('resposta'),stopping:false,phase:'processing',partialText:'',processingMs:0,messageId:message.id,dataReview:p.dataReview,start:Date.now(),stopped:false};live.set(a.id,state);
      await sleep(650);
      for(let end=24;end<reply.length+24;end+=24){if(state.stopped)break;state.partialText=reply.slice(0,end);state.phase='streaming';await sleep(70);}
      history.push({id:id('mensagem'),role:'assistant',text:state.stopped?(state.partialText||'Resposta interrompida antes do primeiro trecho.')+'\n\n*Demonstração interrompida. Nenhuma chamada paga foi realizada.*':reply,createdAt:now()});
      live.delete(a.id);a.updatedAt=now();value={completed:!state.stopped};break;
    }
    case 'ai.chat.stop': {const state=live.get(String(p.assessmentId));if(state){state.stopped=true;state.stopping=true;}value={stopped:true};break;}
    case 'ai.budget.status':value=budget();break;
    case 'ai.budget.history':value={rows:usageRows()};break;
    case 'security.status': {const selection=selectionFor(p);value={available:true,hasApiKey:true,provider:selection.provider,model:selection.model,runtime:{available:false,version:'Demonstração · sem execução'},reason:'Motor fictício da demonstração. A análise real é executada pelo aplicativo instalado.',requiresSensitiveConfirmation:false};break;}
    case 'security.scan.get':value={scan:scans.get(String(p.assessmentId))??null};break;
    case 'security.scan.start': {
      const scan=createScan(String(p.assessmentId),'running');scans.set(scan.assessmentId,scan);value={scan};
      setTimeout(()=>{if(scan.status!=='running')return;scan.stage='reading';scan.message='Conferindo o trecho preparado de src/routes/orders.ts…';scan.events.push({at:now(),stage:'reading',message:scan.message});},650);
      setTimeout(()=>{if(scan.status!=='running')return;scan.status='completed';scan.stage='report';scan.message='Relatório fictício pronto para revisão.';scan.updatedAt=now();scan.result=scanResult(scan.id);const a=assessment(scan.assessmentId);a.updatedAt=now();a.events.push({id:id('evento'),at:now(),kind:'info',message:scan.message,securityScan:{id:scan.id,status:'completed',createdAt:scan.createdAt,findingsTotal:1}} as Assessment['events'][number]);window.dispatchEvent(new Event('openkoda:security-scan-updated'));},2200);break;
    }
    case 'security.scan.cancel': {const scan=scans.get(String(p.assessmentId));if(scan){scan.status='cancelled';scan.message='Análise de exemplo cancelada.';}value={scan:scan??null};break;}
    case 'account.session':value={user:signed?{id:'visitante-demo',email:'visitante@example.invalid'}:null};break;
    case 'account.status':value={state:signed?'authenticated':'signedOut',user:signed?{id:'visitante-demo',email:'visitante@example.invalid',displayName:'Visitante de exemplo'}:null,account:signed?account():null};break;
    case 'account.overview':value=account();break;
    case 'account.signOut':signed=false;value={revoked:true};break;
    case 'account.signIn':case 'account.google':signed=true;announce('Conta fictícia ativada. Nenhuma autenticação real foi feita.');value={user:{id:'visitante-demo',email:'visitante@example.invalid'}};break;
    case 'account.recover':case 'account.site':announce('Na demonstração, esta ação não abre serviços nem envia e-mails. Explore a conta fictícia nesta janela.');value={opened:false};break;
    case 'account.marketing':marketing=p.enabled===true;value={updated:true};break;
    case 'account.cancel':renewalCancelled=true;value={cancelled:true};break;
    case 'account.supportList':value={tickets};break;
    case 'account.support':tickets.unshift({id:id('chamado'),subject:String(p.subject),message:String(p.message),createdAt:now(),status:'open',replies:[]});announce('Chamado fictício salvo somente nesta demonstração. Nenhuma mensagem foi enviada.');value={created:true};break;
    case 'account.skills':value={skills:[{id:'code-review',name:'Revisar código',description:'Organizar a leitura dos trechos e os pontos que merecem verificação.',revision:'demo',invocation:'explicit'},{id:'explain-code',name:'Explicar código',description:'Entender responsabilidades, entradas e saídas de um trecho.',revision:'demo',invocation:'explicit'}]};break;
    case 'packages.catalog':value={tools:[],inventoryVerified:true};break;
    case 'environment.status':value={status:'unavailable',tools:[],message:'Esta demonstração não instala nem executa ferramentas.'};break;
    case 'retention.inventory':value={items:[],journals:[],hasUnresolved:false,blocked:false,limits:['Nenhum arquivo real é armazenado nesta demonstração.'],projectPurgeSupported:false,automaticPurge:false,retentionDays:s.settings.retentionDays};break;
    case 'backup.list':value={backups:[]};break;
    case 'activation.status':value={currentProfile:{kind:'base'},activeProfile:{kind:'base'},previousProfile:null,pendingPlan:null,restartRequired:false,notice:'Não há cópia para ativar. A demonstração mantém somente dados fictícios em memória.'};break;
    case 'activation.list':value={plans:[]};break;
    default:throw new Error('Esta operação usa recursos do aplicativo instalado. Na demonstração, você pode explorar projetos, conversas, relatório, modelos, aparência, uso e perfil; nenhuma ação real foi executada.');
  }
  return clone(value) as T;
}
