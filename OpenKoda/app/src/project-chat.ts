import { request } from './api';
import { dataReason, type ClassificationSubjects, type DataCategory, type DataPolicyStatus, type MetadataReview } from './data-policy';
import { scopeAllows, validScopeStatus, type ScopeStatus } from './scope';
import { operationAdapters, type OperationAdapter } from './ai-operation';
import { currentScopeRef, targetAllows, type ScopeTargetRef } from './combined-scope';
import { validPacketContext } from './components/AiDataPacket';
import type { CredentialList } from './components/CredentialReferences';
import type { Assessment } from './types';
import { providers,type AiSelection } from './ai-provider';

export type ApprovalMode='always'|'risk'|'scope';
export const approvalModes:Record<ApprovalMode,string>={always:'Sempre solicitar',risk:'Solicitar para ações de risco',scope:'Não solicitar dentro do escopo'};
export const chatCategories:DataCategory[]=['human_text','target_metadata','execution_summary','source_excerpt'];
function checkReview(value:MetadataReview){
  if(!value.canCommit||!value.review) throw new Error(value.reasonCodes?.map(dataReason).join(' ')||'A configuração do projeto precisa de revisão.');
  return value.review;
}
function assertCurrent(signal?:AbortSignal){if(signal?.aborted)throw new Error('Envio cancelado.');}

async function conversationScope(assessmentId:string){
  const scope=await request<ScopeStatus>('assessment.scopeStatus',{assessmentId});
  if(!validScopeStatus(scope,assessmentId)||!scopeAllows(scope,'ai.remote-review.v1'))throw new Error('Este escopo não permite enviar mensagens à IA. Consulte o escopo do projeto.');
  const combined=!!scope.revision?.body.targetSet;
  const refs=scope.targetOptions?.filter(o=>targetAllows(o,'ai.remote-review.v1'))??[];
  if(combined && refs.length!==1)throw new Error('Use os controles avançados para escolher as entradas de uma avaliação combinada.');
  return {scope,combined,refs};
}

async function bindMetadata(projectId:string,assessmentId:string,selection:AiSelection,policy:DataPolicyStatus,entryId?:string,signal?:AbortSignal){
  if(!policy.active||!policy.currentRevision)throw new Error('A política de dados deste projeto está inativa.');
  const state=await request<DataPolicyStatus>('data.policy.status',{assessmentId,projectId});
  if(!state.binding){
    const p=policy.currentRevision;
    const rev=await request<MetadataReview>('data.binding.prepare',{assessmentId,expectedStateDigest:state.stateDigest,policyRef:{projectId,revisionId:p.id,digest:p.digest}});
    assertCurrent(signal);
    await request('data.binding.commit',{review:checkReview(rev),idempotencyKey:crypto.randomUUID(),acknowledgeProspectiveAdoption:true});
  }
  const subjects=await request<ClassificationSubjects>('data.classification.subjects',{assessmentId});
  for(const row of subjects.subjects){
    const subject=row.subject;
    if(subject.kind==='scope_source'&&entryId!==undefined&&subject.targetRef?.entryId!==entryId)continue;
    if(subject.kind!=='scope_source'&&!(subject.kind==='assessment_metadata'&&['human_text','execution_summary'].includes(subject.category)))continue;
    if(row.classification.classificationRef)continue;
    if(row.classification.secretMaterial)throw new Error('O projeto identificou material secreto na fonte escolhida.');
    const rev=await request<MetadataReview&{downgrade:boolean;proposal:{secretMaterial:boolean}}>('data.classification.prepare',{subject,expectedSourceFingerprint:row.sourceFingerprint,sensitivity:'internal',reason:'Contexto interno da conversa, selecionado ao solicitar a análise deste projeto.'});
    if(rev.downgrade||rev.proposal.secretMaterial)throw new Error('A classificação da fonte requer revisão.');
    assertCurrent(signal);
    await request('data.classification.commit',{review:checkReview(rev),idempotencyKey:crypto.randomUUID(),acknowledgeClassification:true});
  }
  let credentials=await request<CredentialList>('credential.reference.list',{projectId});
  const related=()=>credentials.references.filter(r=>r.providerSlotRef?.provider===selection.provider);
  let ref=related().find(r=>!r.revoked);
  if(!ref){
    if(related().some(r=>r.revoked))throw new Error('A referência a este provedor foi revogada nas configurações do projeto.');
    const slot=credentials.providerSlots.find(s=>s.provider===selection.provider&&s.available);
    if(!slot)throw new Error('Conecte sua chave em IA e modelos.');
    assertCurrent(signal);
    await request('credential.reference.create',{projectId,expectedStateDigest:credentials.stateDigest,idempotencyKey:crypto.randomUUID(),kind:'provider_reference',label:providers[selection.provider].name,purpose:'Conversa sobre a análise deste projeto',providerSlotId:slot.id});
    credentials=await request<CredentialList>('credential.reference.list',{projectId});
    ref=related().find(r=>!r.revoked);
  }
  if(!ref)throw new Error('O provedor não está disponível neste projeto.');
  assertCurrent(signal);
  return ref;
}

// A new project has no policy to adopt. Existing project policies retain their
// rules when a subsequent conversation adopts the source and provider references.
export async function prepareScanMetadata(projectId:string,assessmentId:string,selection:AiSelection){
  const policy=await request<DataPolicyStatus>('data.policy.status',{projectId});
  if(policy?.schema!=='openkoda.data-policy-status.v1'||policy.projectId!==projectId)throw new Error('Não foi possível conferir a política do projeto.');
  if(policy.currentRevision===null)return false;
  if(!policy.active)throw new Error('A política de dados deste projeto está inativa.');
  const {combined,refs}=await conversationScope(assessmentId);
  await bindMetadata(projectId,assessmentId,selection,policy,combined?refs[0].entry.entryId:undefined);
  return true;
}

// Set up only missing settings. Existing policies, classifications and revocations
// belong to the project and are rechecked by the native core before each HTTP call.
export async function prepareChat(assessment:Assessment,selection:AiSelection,message:string,options:{image?:string;file?:boolean;signal?:AbortSignal;propose?:boolean;compact?:boolean;skillId?:string;disableAutoSkill?:boolean}={}){
  const {id:assessmentId,projectId}=assessment;
  const {scope,combined,refs}=await conversationScope(assessmentId);
  const targets=combined?{contextTargetRefs:[refs[0].targetRef]}:{};
  const operation=options.propose?'operation_proposal':'ask';
  const allowedAdapters=operationAdapters(combined?refs[0].entry.kind:assessment.kind)
    .filter(method=>method!=='kali.rg.signals.v1'&&scopeAllows(scope,method)&&(!combined||targetAllows(refs[0],method)));
  if(options.propose&&(!allowedAdapters.length||options.file||options.image))throw new Error('Escolha um método permitido no escopo e envie o objetivo da verificação em texto.');
  const proposalOptions:{allowedAdapters?:OperationAdapter[];operationTargetRef?:ScopeTargetRef}=options.propose?{allowedAdapters,...(combined?{operationTargetRef:refs[0].targetRef}:{})}:{};
  const source=combined?refs[0].entry.projectTargetId:'primary';
  let policy=await request<DataPolicyStatus>('data.policy.status',{projectId});
  if(!policy.currentRevision){
    const body=structuredClone(policy.defaultBody);
    const remote=body.destinations.find(r=>r.destination==='remote_ai');
    if(!remote)throw new Error('O núcleo não reconheceu o destino de IA.');
    Object.assign(remote,{enabled:true,categories:chatCategories,sensitivities:['internal','public'],targetCatalogIds:[source],allowOriginalEvidence:false});
    const rev=await request<MetadataReview>('data.policy.prepare',{projectId,body,expectedStateDigest:policy.stateDigest});
    assertCurrent(options.signal);
    await request('data.policy.commit',{review:checkReview(rev),acknowledgePolicy:true,idempotencyKey:crypto.randomUUID()});
    policy=await request<DataPolicyStatus>('data.policy.status',{projectId});
  }
  const ref=await bindMetadata(projectId,assessmentId,selection,policy,combined?refs[0].entry.entryId:undefined,options.signal);
  const base={assessmentId,evidenceIds:[],...targets,...proposalOptions,message};
  const context=await request<unknown>('ai.context',{...base,operation,credentialReferenceId:ref.id,messageSensitivity:'internal',packageSensitivity:'internal',
    categories:(options.file||options.image)?chatCategories:chatCategories.filter(c=>c!=='source_excerpt'),
    includeChatHistory:!options.propose,...(options.compact?{compactHistory:true}:{}),...(options.image?{chatImage:options.image}:{}),...(options.skillId?{skillId:options.skillId}:{}),
    ...(options.disableAutoSkill?{disableAutoSkill:true}:{})});
  if(!validPacketContext(context)||!context.canSend||!context.dataReview)throw new Error(validPacketContext(context)?context.reasonCodes.map(dataReason).join(' '):'O núcleo não autorizou este pacote.');
  if(context.packetPreview.model!==selection.model||context.packetPreview.endpoint!==providers[selection.provider].endpoint)throw new Error('O modelo mudou durante a preparação.');
  return {context,operation,allowedAdapters,scopeRef:currentScopeRef(scope),payload:{...base,contextDigest:context.digest,dataReview:context.dataReview}};
}

export type Attachment={name:string; image?:string; text?:string};
export async function readChatAttachment(file:File):Promise<Attachment>{
  if(!file.size||file.size>12*1024*1024)throw new Error('Selecione um arquivo de até 12 MB com conteúdo.');
  if(['image/jpeg','image/png','image/webp'].includes(file.type)){
    const bitmap=await createImageBitmap(file);
    try{
      const canvas=document.createElement('canvas');
      for(const size of [1024,768,512,384]){
        const scale=Math.min(1,size/Math.max(bitmap.width,bitmap.height));
        canvas.width=Math.max(1,Math.round(bitmap.width*scale));canvas.height=Math.max(1,Math.round(bitmap.height*scale));
        const ctx=canvas.getContext('2d');
        if(!ctx)throw new Error('Não foi possível preparar a imagem.');
        ctx.fillStyle='#fff';ctx.fillRect(0,0,canvas.width,canvas.height);ctx.drawImage(bitmap,0,0,canvas.width,canvas.height);
        const image=canvas.toDataURL('image/jpeg',0.77).split(',')[1];
        if(image.length<=81920)return {name:file.name,image};
      }
      throw new Error('A imagem é muito detalhada. Selecione um recorte menor.');
    }finally{bitmap.close();}
  }
  if(file.size>6500)throw new Error('Para arquivos de texto, selecione um trecho UTF-8 de até 6.500 bytes.');
  let text:string;
  try{text=new TextDecoder('utf-8',{fatal:true}).decode(await file.arrayBuffer());}catch{throw new Error('Use um texto UTF-8 ou uma imagem PNG, JPEG ou WebP.');}
  if(/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(text))throw new Error('Este formato binário não é suportado.');
  return {name:file.name,text};
}

