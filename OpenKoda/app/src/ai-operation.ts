import { ScopeTargetRef, sameTargetRef } from './combined-scope';
import { Kind } from './types';
import type { SuggestionSource } from './workflow-types';
export const operationLabels={
  'local.inventory.v1':'Inventário local de arquivos',
  'source.rules.v1':'Análise local de padrões textuais',
  'http.headers.v1':'Observação de cabeçalhos HTTP',
  'network.tcp-connect.v1':'Observação TCP com portas escolhidas pela pessoa',
  'kali.rg.signals.v1':'Análise de uma evidência no Kali local',
};
export type OperationAdapter=keyof typeof operationLabels;
export interface OperationProposal {planChange?:{kind:'review_then_observe';source:SuggestionSource};operationTargetRef?:ScopeTargetRef;contextTargetRefs?:ScopeTargetRef[];id:string;requestId:string;responseId:string;assessmentId:string;contextDigest:string;adapter:OperationAdapter;rationale:string;question:string|null;model:string;endpoint:string;usage:{input_tokens?:number|null;output_tokens?:number|null}|null;toolsExecuted:false;requiresHumanReview:true;availabilityVerified:false;notice:string}
export interface OperationAudit {id?:string;requestId?:string;assessmentId?:string;contextDigest?:string;allowedAdapters?:OperationAdapter[];status:'pending'|'accepted'|'rejected'|'stale'|'provider_error'|'interrupted';model?:string;endpoint?:string;createdAt?:string;proposal?:OperationProposal;reason?:string}
export function operationAdapters(kind:Kind):OperationAdapter[]{return kind==='code'||kind==='files'?['local.inventory.v1','source.rules.v1','kali.rg.signals.v1']:kind==='web'?['http.headers.v1','kali.rg.signals.v1']:['network.tcp-connect.v1','kali.rg.signals.v1'];}
export function validOperationProposal(value:unknown,assessmentId:string):value is OperationProposal {
  if(!value||typeof value!=='object')return false;const p=value as Record<string,unknown>;
  return p.assessmentId===assessmentId&&typeof p.adapter==='string'&&Object.hasOwn(operationLabels,p.adapter)&&p.toolsExecuted===false&&p.requiresHumanReview===true&&p.availabilityVerified===false&&['id','requestId','responseId','contextDigest','rationale','model','endpoint','notice'].every(key=>typeof p[key]==='string')&&(p.question===null||typeof p.question==='string');
}

export function operationMatchesTargets(proposal:OperationProposal,refs?:ScopeTargetRef[],operation?:ScopeTargetRef){
 if(!refs&&!operation)return proposal.operationTargetRef===undefined&&proposal.contextTargetRefs===undefined;
 if(!operation||!refs?.length||!sameTargetRef(proposal.operationTargetRef,operation)||!Array.isArray(proposal.contextTargetRefs)||proposal.contextTargetRefs.length!==refs.length||new Set(proposal.contextTargetRefs.map(r=>r.entryId)).size!==refs.length)return false;
 return refs.every(ref=>proposal.contextTargetRefs!.some(actual=>sameTargetRef(actual,ref)))&&refs.some(ref=>sameTargetRef(ref,operation));
}
