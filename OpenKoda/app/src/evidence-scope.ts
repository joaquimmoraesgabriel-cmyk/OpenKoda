import { sameTargetRef, ScopeTargetRef, targetAllows } from './combined-scope';
import { ScopeStatus, scopeAllows } from './scope';

type EvidenceMethod='kali.hash-evidence.v1'|'kali.rg.signals.v1';
interface EvidenceOrigin {verified:boolean;targetRefs:ScopeTargetRef[]}
export function evidenceMethodGate(status:ScopeStatus|undefined,origin:EvidenceOrigin|undefined,method:EvidenceMethod):{allowed:boolean;reason:string}{
 if(!status)return {allowed:false,reason:'O escopo ainda não foi confirmado pelo núcleo.'};
 if(!status.revision?.body.targetSet)return {allowed:scopeAllows(status,method),reason:scopeAllows(status,method)?'':'Este método não está permitido pelo escopo atual.'};
 if(!origin?.verified)return {allowed:false,reason:'A origem da evidência ainda não foi confirmada pelo núcleo. Atualize a origem antes de transferir seus bytes.'};
 if(!origin.targetRefs.length)return {allowed:false,reason:'Esta evidência não possui entradas contribuintes verificadas. Um registro comum não autoriza hash ou análise de um alvo.'};
 if(new Set(origin.targetRefs.map(ref=>ref.entryId)).size!==origin.targetRefs.length)return {allowed:false,reason:'A origem retornada contém referências repetidas. Atualize a origem antes de continuar.'};
 for(const ref of origin.targetRefs){
  const option=status.targetOptions?.find(item=>sameTargetRef(item.targetRef,ref));
  if(!option)return {allowed:false,reason:'Uma referência de origem não pertence à revisão atual. Atualize a origem e o escopo.'};
  if(!targetAllows(option,method))return {allowed:false,reason:`A entrada “${option.entry.label}” não permite este método no escopo atual. Todas as entradas contribuintes precisam permitir a operação.`};
 }
 return {allowed:true,reason:''};
}
