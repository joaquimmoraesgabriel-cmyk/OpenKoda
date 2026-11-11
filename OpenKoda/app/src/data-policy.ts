import type { ScopeRef } from './scope';
import type { ScopeTargetRef } from './combined-scope';
import type { DataSensitivity } from './data-labels';

export type Sensitivity=DataSensitivity;
export type DataCategory='human_text'|'target_metadata'|'execution_summary'|'evidence_excerpt'|'source_excerpt'|'finding_text'|'activity_metadata';
export type DataDestination='local_raw_view'|'remote_ai'|'local_kali'|'report_export'|'recovery_backup';
export interface ProjectionInfo {schema:'openkoda.projection.v1';redactorVersion:'openkoda.redactor.v1';mode:'ready'|'no_credential_configured'|'metadata_only';redacted:boolean;omittedFields:string[];reasonCodes:string[]}
export interface ClassificationRef {id:string;digest:string}
export interface DataPolicyRef {projectId:string;revisionId:string;digest:string}
export interface DataReviewRef {id:string;digest:string;expiresAt:string}
export interface DataOriginRef {scopeRef:ScopeRef;targetRef:ScopeTargetRef|null}
export interface ClassificationState {sensitivity:Sensitivity;classificationRef:ClassificationRef|null;sourceFingerprint:string;secretMaterial:boolean;contributorRefs:ClassificationRef[];reasonCodes:string[]}
export interface ShapeEligibility {allowedByShape:boolean;reasonCodes:string[];requiresDataReview:true}
export interface EvidenceView {
 id:string;name:string;createdAt:string;rawSha256:string;rawBytes:number;hasRaw:boolean;
 classification:ClassificationState;preview:string|null;previewBytes:number;previewTruncated:boolean;projectionSha256:string|null;projection:ProjectionInfo;
 originRefs:DataOriginRef[];originKind:'host_record'|'user_association'|'unknown';
 structuredView:null|{kind:'source_rules'|'source_capture'|'http'|'tcp'|'inventory'|'kali';data:unknown};
 eligibility:{ai:ShapeEligibility;kaliHash:ShapeEligibility;kaliScan:ShapeEligibility;rawRead:ShapeEligibility};
}
export type DataSubject={kind:'assessment_metadata';assessmentId:string;category:DataCategory}|{kind:'scope_source';assessmentId:string;scopeRef:ScopeRef;targetRef:ScopeTargetRef|null}|{kind:'evidence';assessmentId:string;evidenceId:string}|{kind:'derived';assessmentId:string;producer:'finding'|'validation'|'ai_request'|'investigation'|'remediation'|'kali_run'|'work_plan'|'work_attempt'|'capture';objectId:string};
export interface DestinationRule {destination:DataDestination;enabled:boolean;categories:DataCategory[];sensitivities:Sensitivity[];targetCatalogIds:string[];allowOriginalEvidence:boolean}
export interface DataPolicyBody {schema:'openkoda.data-policy.v1';destinations:DestinationRule[];localRawView:{enabled:boolean;sensitivities:Sensitivity[]};recoveryBackup:{enabled:boolean;sensitivities:Sensitivity[]};redactorVersion:'openkoda.redactor.v1'}
export interface DataPolicyRevision {id:string;projectId:string;number:number;body:DataPolicyBody;digest:string;createdAt:string}
export interface DataBinding {assessmentId:string;projectId:string;policyRef:DataPolicyRef;adoptedAt:string;generation:number;origin:string;frozen:null|{receiptRef:ClassificationRef;frozenAt:string}}
export interface DataPolicyStatus {schema:'openkoda.data-policy-status.v1';projectId:string;assessmentId?:string|null;stateDigest:string;generation:number;active:boolean;currentRevision:DataPolicyRevision|null;pinnedRevision:DataPolicyRevision|null;binding:DataBinding|null;defaultBody:DataPolicyBody;categoryCatalog:unknown[];destinationCatalog:unknown[];sourceOptions:{targetCatalogId:string;label:string;kind:string;inactive:boolean}[];actions:{canPrepare:boolean;canAdopt:boolean;canRebind:boolean;canRevoke:boolean;canReactivate:boolean};reasonCodes:string[];limits:string[]}
export interface MetadataReview {review:DataReviewRef|null;canCommit:boolean;reasonCodes:string[]}
export interface ClassificationSubject {subject:DataSubject;label:string;category:DataCategory;sourceFingerprint:string;classification:ClassificationState;originRefs:DataOriginRef[];originVerified:boolean;canClassify:boolean;reasonCodes:string[]}
export interface ClassificationSubjects {assessmentId:string;stateDigest:string;subjects:ClassificationSubject[]}
export interface AccessPreview {review:DataReviewRef|null;subject:DataSubject;rawSha256:string;rawBytes:number;projectionSha256:string;totalBytes:number;rangeStart:number;rangeEnd:number;hasMore:boolean;representation:'raw_view'|'redacted_raw_view';classification:ClassificationState;redacted:boolean;canRead:boolean;reasonCodes:string[]}
export interface AccessWindow {receiptRef:ClassificationRef;subject:DataSubject;rawSha256:string;rawBytes:number;projectionSha256:string;totalBytes:number;rangeStart:number;rangeEnd:number;hasMore:boolean;representation:'raw_view'|'redacted_raw_view';text:string;returnedBytes:number;redacted:boolean}

const reasons:Record<string,string>={data_policy_required:'Revise a política de dados deste projeto.',data_binding_required:'Adote explicitamente uma política de dados para esta avaliação.',data_policy_inactive:'A política está inativa. Revise sua reativação para permitir novos usos.',data_policy_changed:'A política mudou. Prepare uma nova revisão.',data_classification_required:'Classifique as fontes e os dados selecionados antes de preparar a saída.',data_classification_changed:'A classificação mudou. Revise novamente a seleção.',data_secret_material:'O núcleo identificou material de segredo. Esta saída permanece bloqueada.',data_source_unknown:'A origem dos dados precisa de revisão.',data_review_required:'Prepare e confirme a revisão dos dados desta ação.',data_review_expired:'A revisão expirou. Prepare uma nova revisão.',data_review_used:'Esta revisão já foi utilizada. Consulte o recibo ou prepare outra janela.',data_review_changed:'O material selecionado mudou. Prepare uma nova revisão.',data_binding_frozen:'Esta avaliação já usou dados com sua política vinculada. Para ampliar a permissão, derive outra avaliação.',data_redactor_unavailable:'O redator de dados está indisponível. Metadados de controle e manutenção continuam acessíveis.',data_limit:'O limite desta operação foi atingido.',credential_reference_changed:'A referência de credencial mudou. Revise novamente o pacote.'};
export function dataReason(code:string){return reasons[code]??`Motivo informado pelo núcleo: ${code}`;}
export function reviewCurrent(review:DataReviewRef|null|undefined){return !!review&&Date.parse(review.expiresAt)>Date.now();}
export const dataChangeEvent='openkoda:data-policy-change';
export function dataChanged(){window.dispatchEvent(new Event(dataChangeEvent));}
export function validEvidenceView(value:unknown):value is EvidenceView {
 if(!value||typeof value!=='object')return false;const e=value as EvidenceView;
 if(e.preview!==null&&typeof e.preview!=='string'||new TextEncoder().encode(e.preview??'').byteLength!==e.previewBytes)return false;
 return !('content'in e)&&!('sha256'in e)&&typeof e.id==='string'&&typeof e.name==='string'&&typeof e.rawSha256==='string'&&Number.isSafeInteger(e.rawBytes)&&e.rawBytes>=0&&typeof e.hasRaw==='boolean'&&!!e.classification&&['unclassified','public','internal','sensitive'].includes(e.classification.sensitivity)&&e.projection?.schema==='openkoda.projection.v1'&&(e.preview===null||typeof e.preview==='string')&&Number.isSafeInteger(e.previewBytes)&&e.previewBytes>=0&&e.previewBytes<=2048&&typeof e.previewTruncated==='boolean'&&!!e.eligibility&&['ai','kaliHash','kaliScan','rawRead'].every(key=>{const gate=e.eligibility[key as keyof typeof e.eligibility];return !!gate&&typeof gate.allowedByShape==='boolean'&&gate.requiresDataReview===true&&Array.isArray(gate.reasonCodes);});
}
