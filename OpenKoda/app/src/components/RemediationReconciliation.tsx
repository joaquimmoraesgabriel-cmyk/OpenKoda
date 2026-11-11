import { useEffect, useState } from 'react';
import { FileIdentity, findingLabels, shortDate } from '../types';
import { Field, KeyValue, Notice } from './shared';

export interface RemediationReceipt {
  id:string;evidenceId:string;previousStatus:string;interruptedOperation:string|null;recordedAt:string;author:string;
  note:string;acknowledgeUnknownEffects:boolean;findingStatus:string;
  observation:{state:'before'|'after'|'other'|'missing'|'unavailable';observedAt:string;observedSha256:string|null;
    observedBytes:number|null;observedIdentity:FileIdentity|null;identityMatches:boolean|null;message:string};
}
const observationLabels={before:'Bytes correspondem ao conteúdo original',after:'Bytes correspondem ao conteúdo proposto',other:'Bytes diferentes do original e do proposto',missing:'Arquivo ausente no momento da inspeção',unavailable:'Arquivo não pôde ser inspecionado'};
const operationLabels:Record<string,string>={applying:'Aplicação da alteração',retesting:'Reteste de texto ou hash',confirming:'Registro da confirmação'};

export default function RemediationReconciliation({status,interruptedOperation,receipt,blocked,onReconcile}:{status:string;interruptedOperation?:string|null;receipt?:RemediationReceipt|null;blocked:boolean;onReconcile:(note:string)=>Promise<void>}){
  const [note,setNote]=useState(receipt?.note??'');const [acknowledged,setAcknowledged]=useState(false);
  useEffect(()=>{setNote(receipt?.note??'');setAcknowledged(false);},[receipt?.id,receipt?.note]);
  const unresolved=['unknown','applying','retesting','confirming'].includes(status);
  const noteBytes=new TextEncoder().encode(note).byteLength;
  const operation=receipt?.interruptedOperation??interruptedOperation??(status==='unknown'?null:status);
  async function submit(e:React.FormEvent){e.preventDefault();try{await onReconcile(receipt?.note??note);}finally{setAcknowledged(false);}}
  if(!unresolved&&!receipt)return null;
  return <section className="remediation-reconciliation" aria-label="Reconciliação da correção"><h3>{unresolved?'Revisar correção interrompida':'Registro da reconciliação'}</h3>
    {operation&&<KeyValue label="Operação interrompida">{operationLabels[operation]??operation}</KeyValue>}
    <Notice>A reconciliação inspeciona o estado local e registra o reconhecimento humano dos limites. Ela não escreve nem restaura o arquivo, não repete a alteração ou o reteste e não muda o estado do achado. Efeitos anteriores podem continuar desconhecidos.</Notice>
    {receipt&&<div className="review-sheet"><KeyValue label="Observação registrada">{observationLabels[receipt.observation.state]??receipt.observation.state}</KeyValue><KeyValue label="Data da observação"><time dateTime={receipt.observation.observedAt}>{shortDate(receipt.observation.observedAt)}</time></KeyValue><p className="preserve-lines">{receipt.observation.message}</p>
      {receipt.observation.state==='unavailable'&&<Notice tone="warning">Não foi possível verificar o arquivo. Isso não significa que ele esteja ausente ou tenha sido restaurado.</Notice>}
      <KeyValue label="Identidade do arquivo">{receipt.observation.identityMatches===true?'Corresponde à identidade revisada':receipt.observation.identityMatches===false?'Identidade diferente: o arquivo foi substituído':'Não confirmada nesta observação'}</KeyValue>
      {receipt.observation.identityMatches===false&&<Notice tone="warning">Mesmo que os bytes coincidam, a identidade é diferente da aprovada. A observação não comprova a conclusão da alteração anterior.</Notice>}
      {receipt.observation.observedBytes!==null&&<KeyValue label="Tamanho observado">{receipt.observation.observedBytes.toLocaleString('pt-BR')} bytes</KeyValue>}
      {receipt.observation.observedSha256&&<KeyValue label="SHA-256 observado"><span className="mono break-word">{receipt.observation.observedSha256}</span></KeyValue>}
      <KeyValue label="Estado do achado na observação">{(findingLabels as Record<string,string>)[receipt.findingStatus]??receipt.findingStatus}</KeyValue>
      <KeyValue label={status==='reconciled'?'Evidência do registro':'Identificador reservado da evidência'}><span className="mono break-word">{receipt.evidenceId}</span></KeyValue>
      <details className="plan-details"><summary>Recibo e identidade observada</summary><p className="mono break-word">{receipt.id}</p><p>Registro: <time dateTime={receipt.recordedAt}>{shortDate(receipt.recordedAt)}</time> · {receipt.author}</p><pre>{JSON.stringify(receipt.observation.observedIdentity,null,2)}</pre></details>
    </div>}
    {unresolved?<form onSubmit={e=>void submit(e).catch(()=>{})}>
      {receipt&&<Notice tone="warning">Há uma observação já preservada cujo registro ficou incompleto. Concluir usa a mesma justificativa e a data original acima; não faz uma nova inspeção. Não houve repetição automática.</Notice>}
      <Field label="Justificativa da reconciliação" hint={receipt?'A justificativa preservada deve ser mantida exatamente para concluir este recibo.':'Descreva o que revisou e os limites que permanecem desconhecidos.'}><textarea rows={4} required value={receipt?.note??note} onChange={e=>{setNote(e.target.value);setAcknowledged(false);}} readOnly={!!receipt} disabled={blocked} maxLength={4000}/></Field>
      <p className="muted">{noteBytes.toLocaleString('pt-BR')} de 4.000 bytes permitidos</p>
      <label className="check-field"><input type="checkbox" checked={acknowledged} onChange={e=>setAcknowledged(e.target.checked)} disabled={blocked}/><span>Reconheço que os efeitos anteriores podem continuar desconhecidos e autorizo somente o registro desta reconciliação.</span></label>
      <button type="submit" className="button" disabled={blocked||!acknowledged||!note.trim()||noteBytes>4000}>{receipt?'Concluir registro da reconciliação':'Inspecionar e registrar reconciliação'}</button>
    </form>:<><KeyValue label="Justificativa registrada"><span className="preserve-lines">{receipt?.note}</span></KeyValue><Notice>O registro da interrupção foi concluído. Uma alteração futura exige um novo plano revisado; este registro não confirma a correção do achado.</Notice></>}
  </section>;
}
