import { useEffect, useRef, useState } from 'react';
import { Action, errorMessage, shortDate } from '../types';
import { Field, KeyValue, Notice } from './shared';
import type { ScopeRef } from '../scope';
import { OperationAudit } from '../ai-operation';
import AiLocalClosure from './AiLocalClosure';

export interface AiRequestRecord {
  id:string;model:string;status:string;createdAt:string;inputTokens:number|null;outputTokens:number|null;unknownAcknowledged:boolean;
  resolution:null|{id:string;recordedAt:string;author:string;note:string;acknowledgeUnknownUsage:true;usageRemainsUnknown:true};
  operationProposal?:OperationAudit|null;scopeRef?:ScopeRef|null;
  dataReceiptStatus?:'recorded'|'legacy_no_data_policy_receipt';dataUseReceiptRef?:{id:string;digest:string}|null;dataReviewRef?:{id:string;digest:string}|null;dataPolicyRef?:unknown;dataBinding?:unknown;providerCredentialRef?:unknown;
}
const states:Record<string,string>={pending:'Envio pendente',completed:'Resposta recebida',incomplete:'Resposta incompleta',unknown:'Resultado ou consumo desconhecido',failed:'Falha registrada'};
export default function AiRequestLedger({assessmentId,requests,action,blocked,onRefresh}:{assessmentId:string;requests?:AiRequestRecord[];action:Action;blocked:boolean;onRefresh:()=>Promise<void>}){
  const [selected,setSelected]=useState('');const [note,setNote]=useState('');const [ack,setAck]=useState(false);const [pending,setPending]=useState(false);const [error,setError]=useState('');const pendingRef=useRef(false);
  const record=requests?.find(item=>item.id===selected)??requests?.find(item=>item.status==='unknown'&&!item.unknownAcknowledged)??requests?.[0];
  const noteBytes=new TextEncoder().encode(note).byteLength;
  const completeUsage=requests?.filter(item=>item.inputTokens!==null&&item.outputTokens!==null)??[];
  const usageMissing=requests?.some(item=>item.status==='unknown'||item.inputTokens===null||item.outputTokens===null)??false;
  useEffect(()=>{setNote('');setAck(false);setError('');},[record?.id]);
  async function refresh(){if(pendingRef.current)return;pendingRef.current=true;setPending(true);setError('');try{await onRefresh();}catch(e){setError(errorMessage(e));}finally{pendingRef.current=false;setPending(false);}}
  async function reconcile(e:React.FormEvent){
    e.preventDefault();if(!record||!ack||pendingRef.current)return;pendingRef.current=true;setPending(true);setError('');setAck(false);setSelected(record.id);
    try{await action('ai.request.reconcile',{assessmentId,requestId:record.id,note:note.trim(),acknowledgeUnknownUsage:true});await onRefresh();setNote('');}
    catch(e){setError(errorMessage(e));await onRefresh().catch(()=>{});}finally{pendingRef.current=false;setPending(false);}
  }
  return <details className="plan-details ai-request-ledger"><summary>Solicitações e consumo da IA{requests?` · ${requests.length}`:''}</summary>
    <p className="muted">Registro local das solicitações desta avaliação. Consultar ou reconhecer uma interrupção não repete o envio ao provedor.</p>
    {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}
    <div className="inline-actions"><button type="button" className="button" disabled={blocked||pending} onClick={()=>void refresh()}>Atualizar solicitações da IA</button></div>
    {requests===undefined?<p className="muted">O serviço não retornou o registro de solicitações nesta consulta.</p>:requests.length===0?<p className="muted">Nenhuma solicitação registrada nesta avaliação.</p>:<>
      <KeyValue label="Tokens com entrada e saída informadas">{completeUsage.length?completeUsage.reduce((total,item)=>total+(item.inputTokens??0)+(item.outputTokens??0),0).toLocaleString('pt-BR'):'Nenhum consumo completo informado'}</KeyValue>
      {usageMissing&&<Notice tone="warning">Há consumo não confirmado. O total conhecido não representa necessariamente o consumo total; valores não informados não são zero e não confirmam cancelamento, ausência de cobrança ou estorno.</Notice>}
      <Field label="Solicitação para consultar"><select value={record?.id??''} disabled={blocked||pending} onChange={e=>setSelected(e.target.value)}>{requests.map(item=><option key={item.id} value={item.id}>{shortDate(item.createdAt)} · {item.model} · {states[item.status]??item.status}{item.unknownAcknowledged?' · reconhecimento registrado':''}</option>)}</select></Field>
      {record&&<section className="review-sheet" aria-label="Detalhes da solicitação de IA"><KeyValue label="Solicitação"><span className="mono break-word">{record.id}</span></KeyValue><KeyValue label="Modelo">{record.model}</KeyValue><KeyValue label="Estado">{states[record.status]??record.status}</KeyValue><KeyValue label="Data"><time dateTime={record.createdAt}>{shortDate(record.createdAt)}</time></KeyValue><KeyValue label="Tokens de entrada">{record.inputTokens===null?'Não informados':record.inputTokens.toLocaleString('pt-BR')}</KeyValue><KeyValue label="Tokens de saída">{record.outputTokens===null?'Não informados':record.outputTokens.toLocaleString('pt-BR')}</KeyValue>
        {record.dataReceiptStatus==='legacy_no_data_policy_receipt'&&<Notice>Registro legado sem recibo da política de dados. Ele não comprova autorização segundo a política atual.</Notice>}{record.dataReceiptStatus==='recorded'&&record.dataUseReceiptRef&&<details className="plan-details"><summary>Recibo do uso de dados</summary><p>O recibo registra a admissão deste uso. Não concede permissão para novos envios.</p><pre>{JSON.stringify({receiptRef:record.dataUseReceiptRef,reviewRef:record.dataReviewRef,policyRef:record.dataPolicyRef,binding:record.dataBinding,credentialRef:record.providerCredentialRef},null,2)}</pre></details>}
        {record.unknownAcknowledged&&<Notice>Reconhecimento humano registrado. O resultado ou consumo continua desconhecido; nenhuma resposta foi criada e nenhuma chamada foi repetida.</Notice>}
        {record.operationProposal&&<KeyValue label="Sugestão de operação">{({pending:'Pendente',accepted:'Aceita pelo esquema; sem execução',rejected:'Resposta recusada como sugestão',stale:'Contexto desatualizado',provider_error:'Erro do provedor',interrupted:'Interrompida'} as Record<string,string>)[record.operationProposal.status]??record.operationProposal.status}</KeyValue>}
        {record.resolution&&<><KeyValue label="Data do reconhecimento"><time dateTime={record.resolution.recordedAt}>{shortDate(record.resolution.recordedAt)}</time></KeyValue><KeyValue label="Justificativa preservada"><span className="preserve-lines">{record.resolution.note}</span></KeyValue><details className="plan-details"><summary>Recibo do reconhecimento</summary><p className="mono break-word">{record.resolution.id}</p><p>Autor: {record.resolution.author}</p></details></>}
        {record.status==='unknown'&&!record.unknownAcknowledged&&<form onSubmit={e=>void reconcile(e)}><h3>Revisar solicitação interrompida</h3><Notice>Este reconhecimento encerra a pendência local de revisão. Ele não recupera uma resposta, não consulta o provedor e mantém o consumo desconhecido.</Notice><Field label="Justificativa da solicitação interrompida" hint={`${noteBytes.toLocaleString('pt-BR')} de 2.000 bytes UTF-8`}><textarea rows={3} required value={note} onChange={e=>{setNote(e.target.value);setAck(false);}} maxLength={2000} disabled={blocked||pending}/></Field><label className="check-field"><input type="checkbox" checked={ack} onChange={e=>setAck(e.target.checked)} disabled={blocked||pending}/><span>Reconheço que o resultado e o consumo desta solicitação continuam desconhecidos, inclusive a possível cobrança.</span></label><button type="submit" className="button" disabled={blocked||pending||!ack||!note.trim()||noteBytes>2000}>Registrar reconhecimento da solicitação</button></form>}
      </section>}
    </>}
    <AiLocalClosure assessmentId={assessmentId} action={action} busy={blocked||pending} onCommitted={onRefresh}/>
  </details>;
}
