import { ContextTargets, useContextTargets } from './TargetSelection';
import { ScopeRestriction, useScopeAccess } from './ScopeAccess';
import { useEffect, useRef, useState } from 'react';
import { native, request } from '../api';
import { Action, Assessment, Settings, errorMessage, shortDate } from '../types';
import { ProjectionInfo } from '../data-policy';
import { composerShortcut, composerSubmitKey } from '../composer-preference';
import { Notice } from './shared';
import { ProjectionNotice } from './DataAccess';
import AiRequestLedger, { AiRequestRecord } from './AiRequestLedger';
import AiBudgetReview, { useBudgetReview } from './AiBudgetReview';
import { PacketContext, PacketOptions, PacketReview, packetReady, usePacketOptions, validPacketContext } from './AiDataPacket';

interface Message {id:string;role:string;text:string;createdAt:string;projection?:ProjectionInfo}
interface History {messages:Message[];requests?:AiRequestRecord[];projection?:ProjectionInfo}
export default function AiConversation({assessment,settings,action,busy}:{assessment:Assessment;settings:Settings;action:Action;busy:boolean}) {
 const submitButton=useRef<HTMLButtonElement>(null);
 const scope=useScopeAccess();const targets=useContextTargets('ai.remote-review.v1');const packet=usePacketOptions(assessment);
 const [messages,setMessages]=useState<Message[]>([]);const [selected,setSelected]=useState<string[]>([]);const [context,setContext]=useState<PacketContext>();const [consent,setConsent]=useState(false);
 const [message,setMessage]=useState('');const [pending,setPending]=useState(false);const [error,setError]=useState('');const [requests,setRequests]=useState<AiRequestRecord[]>();const [historyProjection,setHistoryProjection]=useState<ProjectionInfo>();
 const revision=JSON.stringify([assessment.id,assessment.updatedAt,scope.revision,targets.key,packet.key,message,selected,settings.model,settings.endpoint,settings.hasApiKey]);const current=useRef(revision);current.current=revision;
 const payload={assessmentId:assessment.id,evidenceIds:selected,...targets.payload,contextDigest:context?.digest,message,dataReview:context?.dataReview,acknowledgeSensitive:packet.sensitive};
 const budget=useBudgetReview('ask',context?.canSend?payload:null,assessment.id,revision);
 async function refreshHistory(){const history=await request<History>('ai.history',{assessmentId:assessment.id});setMessages(history.messages);setRequests(history.requests);setHistoryProjection(history.projection);}
 useEffect(()=>{setSelected([]);setMessage('');setMessages([]);setRequests(undefined);setError('');let active=true;if(native)void request<History>('ai.history',{assessmentId:assessment.id}).then(history=>{if(active){setMessages(history.messages);setRequests(history.requests);setHistoryProjection(history.projection);}}).catch(e=>{if(active)setError(errorMessage(e));});return()=>{active=false;};},[assessment.id]);
 useEffect(()=>{setContext(undefined);setConsent(false);},[revision]);
 async function review(){if(!packet.ready||!message.trim()||pending)return;const started=revision;setPending(true);setError('');setContext(undefined);setConsent(false);try{const value=await request<unknown>('ai.context',{assessmentId:assessment.id,evidenceIds:selected,...targets.payload,message,...packet.payload,operation:'ask'});if(started!==current.current)return;if(!validPacketContext(value))throw new Error('O núcleo não retornou o pacote final de dados esperado.');setContext(value);}catch(e){setError(errorMessage(e));}finally{setPending(false);}}
 async function send(e:React.FormEvent){e.preventDefault();if(!context||!consent||!packetReady(context,packet)||!targets.ready||!scope.allows('ai.remote-review.v1'))return;setPending(true);setError('');setConsent(false);try{const authorization=budget.authorization();budget.invalidate();await action('ai.ask',{...payload,allowRemote:true,...authorization});await refreshHistory();setMessage('');setContext(undefined);}catch(e){setError(errorMessage(e));await refreshHistory().catch(()=>{});}finally{setPending(false);void budget.refresh();}}
 if(!native)return <Notice>A conversa com a IA está disponível no aplicativo desktop após configurar o provedor. A prévia não envia mensagens ou evidências.</Notice>;
 return <div className="ai-conversation"><ProjectionNotice projection={historyProjection}/><div className="chat-history" aria-label="Histórico da conversa">{messages.length?messages.map(item=><article className={'chat-message chat-'+item.role} key={item.id}><div className="chat-message-meta"><strong>{item.role==='assistant'?'OpenKoda':'Você'}</strong><small>{shortDate(item.createdAt)}</small></div><ProjectionNotice projection={item.projection}/><p className="preserve-lines">{item.text}</p></article>):<p className="muted">Converse sobre esta avaliação. As respostas da IA são análises e propostas; não executam ferramentas nem confirmam achados.</p>}</div>
 <AiRequestLedger assessmentId={assessment.id} requests={requests} action={action} blocked={busy||pending} onRefresh={refreshHistory}/><ContextTargets selection={targets} label="Entradas para a mensagem" disabled={busy||pending}/><fieldset className="context-selection" disabled={busy||pending}><legend>Contexto para esta mensagem</legend><p className="muted">Selecione as evidências adicionais. O pacote final mostra também os metadados e a pergunta.</p>{assessment.evidence.map(e=><label className="check-field" key={e.id}><input type="checkbox" checked={selected.includes(e.id)} disabled={!selected.includes(e.id)&&(!e.eligibility.ai.allowedByShape||selected.length>=20)} onChange={event=>setSelected(ids=>event.target.checked?[...ids,e.id]:ids.filter(id=>id!==e.id))}/><span>{e.name}</span></label>)}</fieldset><PacketOptions packet={packet} disabled={busy||pending}/>
 <form onSubmit={e=>void send(e)}><label className="chat-input-label" htmlFor={'message-'+assessment.id}>Sua mensagem</label><textarea id={'message-'+assessment.id} value={message} onChange={e=>setMessage(e.target.value)} onKeyDown={event=>{
   if(!composerSubmitKey(event.nativeEvent, composerShortcut()))return;
   event.preventDefault();
   if(busy||pending||!targets.ready||!packet.ready||!message.trim())return;
   if(!context)void review();
   else submitButton.current?.click();
 }} rows={3} required maxLength={16000} placeholder="Peça uma análise ou esclareça o objetivo…" disabled={busy||pending}/><button className="button" type="button" disabled={busy||pending||!targets.ready||!packet.ready||!message.trim()} onClick={()=>void review()}>Revisar contexto de envio</button>
 {context&&<><PacketReview context={context} packet={packet} disabled={busy||pending}/><label className="check-field"><input type="checkbox" checked={consent} onChange={e=>setConsent(e.target.checked)} disabled={busy||pending||!context.canSend}/><span>Autorizo o envio deste contexto e da mensagem ao provedor configurado.</span></label></>}<ScopeRestriction method="ai.remote-review.v1"/><AiBudgetReview budget={budget} disabled={busy||pending}/><div className="form-actions"><button ref={submitButton} className="button primary" disabled={busy||pending||!targets.ready||!scope.allows('ai.remote-review.v1')||budget.pending||!budget.ready||!packetReady(context,packet)||!consent||!message.trim()} type="submit">{pending?'Processando…':'Enviar mensagem'}</button></div></form><p className="muted">O histórico não é enviado automaticamente. Limites atuais: até 2.048 tokens de resposta por chamada e 100 chamadas por avaliação.</p>{error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}
 </div>;
}
