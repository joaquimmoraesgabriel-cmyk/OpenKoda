import { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowUp, FileText, ListChecks, LoaderCircle, Paperclip, ShieldCheck, Square, X } from 'lucide-react';
import { request, native } from '../api';
import { providers,type AiSelection } from '../ai-provider';
import { operationMatchesTargets,validOperationProposal,type OperationProposal } from '../ai-operation';
import type { ScopeRef } from '../scope';
import { ChatVerification } from './ChatVerification';
import { validBudgetReview,usd,type BudgetReview,type BudgetStatus } from '../ai-budget';
import { approvalModes,prepareChat,readChatAttachment,type ApprovalMode,type Attachment } from '../project-chat';
import { openingAnalysisEvent,takeOpeningAnalysis } from '../chat-opening';
import { composerShortcut,composerSubmitKey } from '../composer-preference';
import { errorMessage,shortDate,statusLabels,type Action,type Assessment,type ExecutionPlan,type Settings } from '../types';
import { ChatPlanApproval } from './ChatPlanApproval';
import ProjectKaliTools from './ProjectKaliTools';
import { ProjectionNotice } from './DataAccess';
import type { ProjectionInfo } from '../data-policy';
import { ChatModelPicker } from './ChatModelPicker';
import './project-conversation.css';

type Message={id:string;role:string;text:string;createdAt:string;projection?:ProjectionInfo};
type Turn=Awaited<ReturnType<typeof prepareChat>>&{budget?:BudgetReview;selection:AiSelection;line:string;opening:boolean;draftRevision:number;image?:string;signal:AbortSignal};
type Props={assessment:Assessment;settings:Settings;plans?:ExecutionPlan[];action:Action;busy:boolean;onAdvanced?:()=>void;onModels?:()=>void};
export default function ProjectConversation(props:Props){return <Conversation key={props.assessment.id} {...props}/>;}

function Conversation({assessment,plans,action,busy,onAdvanced,onModels}:Props){
  const [messages,setMessages]=useState<Message[]>([]),[draft,setDraft]=useState(''),[attachment,setAttachment]=useState<Attachment>();
  const draftRevision=useRef(0);
  const [mode,setMode]=useState<ApprovalMode>(()=>{
    try{const saved=localStorage.getItem('openkoda.chat.approval.'+assessment.id);return saved==='always'||saved==='scope'?saved:'risk';}catch{return 'risk';}
  });
  const [approval,setApproval]=useState<Turn>(),[phase,setPhase]=useState(''),[pendingLine,setPendingLine]=useState(''),[error,setError]=useState(''),[loadingFile,setLoadingFile]=useState(false);
  const [proposing,setProposing]=useState(false),[offer,setOffer]=useState<{proposal:OperationProposal;scopeRef:ScopeRef}>();
  const [remote,setRemote]=useState<{requestId:string|null;stopping:boolean}>({requestId:null,stopping:false}),[stopping,setStopping]=useState(false);
  const mount=useRef(true),job=useRef<AbortController|undefined>(undefined),file=useRef<HTMLInputElement>(null),stamp=useRef(0),logEnd=useRef<HTMLDivElement>(null),scroll=useRef<HTMLDivElement>(null),stick=useRef(true);
  const sel:AiSelection|null=assessment.aiSelection??null;
  const localRunning=['queued','preparing','planning','running','pausing','canceling'].includes(assessment.status);
  const running=!!phase||!!remote.requestId||localRunning;
  const refresh=useCallback(async()=>{
    if(!native)return;
    const version=++stamp.current;
    const [history,state]=await Promise.all([request<{messages:Message[]}>('ai.history',{assessmentId:assessment.id}),request<typeof remote>('ai.chat.status',{assessmentId:assessment.id})]);
    if(!mount.current||version!==stamp.current)return;
    if(Array.isArray(history?.messages)){
      setMessages(history.messages.filter(m=>m&&typeof m.id==='string'&&typeof m.text==='string'&&typeof m.createdAt==='string'&&['user','assistant'].includes(m.role)));
    }
    if(state&&(state.requestId===null||typeof state.requestId==='string')&&typeof state.stopping==='boolean')setRemote(state);
  },[assessment.id]);
  useEffect(()=>{
    mount.current=true;
    const read=()=>void refresh().catch(e=>{if(mount.current)setError(errorMessage(e));});read();
    const timer=window.setInterval(read,phase||remote.requestId||localRunning?850:5500);
    return ()=>window.clearInterval(timer);
  },[refresh,phase,!!remote.requestId,localRunning]);
  useEffect(()=>()=>{mount.current=false;job.current?.abort();},[]);
  useEffect(()=>{if(stick.current)logEnd.current?.scrollIntoView({block:'nearest'});},[messages.length,pendingLine,phase,assessment.events.length,approval]);
  async function dispatch(turn:Turn){
    if(turn.signal.aborted||!mount.current)return;
    setApproval(undefined);setPhase('Aguardando resposta do provedor…');
    const payload={...turn.payload,allowRemote:true,acknowledgeSensitive:turn.context.packetPreview.requiresSensitiveConfirmation,...(turn.budget?{budgetReview:{id:turn.budget.id,digest:turn.budget.digest}}:{})};
    if(turn.operation==='operation_proposal'){
      const result=await action('ai.operation.propose',payload);
      if(!validOperationProposal(result,assessment.id)||!turn.allowedAdapters.includes(result.adapter)||result.contextDigest!==turn.context.digest||!turn.scopeRef||!operationMatchesTargets(result,turn.payload.contextTargetRefs,turn.payload.operationTargetRef)){
        throw new Error('O modelo não retornou uma verificação válida para o escopo desta conversa.');
      }
      if(mount.current){setOffer({proposal:result,scopeRef:turn.scopeRef});setProposing(false);}
    }else await action('ai.ask',payload);
    if(mount.current){
      if(!turn.opening){
        if(draftRevision.current===turn.draftRevision)setDraft('');
        setAttachment(undefined);
      }
      await refresh();
    }
  }
  async function prepare(line:string,opening=false){
    if(job.current||!native||!line.trim()||!sel)return;
    const controller=new AbortController();job.current=controller;
    const active=()=>!controller.signal.aborted&&mount.current;
    let waiting=false;
    const picked=opening?undefined:attachment;
    const revision=draftRevision.current;
    const message=picked?.text!==undefined?line+'\n\nArquivo selecionado ('+picked.name+'):\n'+picked.text:line;
    setError('');setPendingLine(line);setPhase('Preparando o contexto…');
    try{
      if(new TextEncoder().encode(message).byteLength>8000)throw new Error('A mensagem e o texto anexado podem ter até 8.000 bytes. Envie um trecho menor.');
      const prepared=await prepareChat(assessment,sel,message,{image:picked?.image,file:!!picked,signal:controller.signal,propose:proposing&&!opening});
      if(!active())return;
      const budget=await request<BudgetStatus>('ai.budget.status',{assessmentId:assessment.id});
      if(!budget||typeof budget.enabled!=='boolean'||budget.degraded)throw new Error('Verifique o orçamento nas configurações antes de enviar.');
      let estimate:BudgetReview|undefined;
      if(budget.enabled){
        const b=await request<{review:BudgetReview|null}>('ai.budget.preview',{operation:prepared.operation,payload:prepared.payload});
        if(!validBudgetReview(b.review))throw new Error('Não foi possível confirmar o custo desta solicitação.');
        estimate=b.review;
      }
      if(!active())return;
      const turn={...prepared,selection:sel,budget:estimate,line,opening,draftRevision:revision,image:picked?.image,signal:controller.signal};
      if(estimate||prepared.context.packetPreview.requiresSensitiveConfirmation||(!opening&&(mode==='always'||(mode==='risk'&&!!picked)))){
        waiting=true;setPhase('');setApproval(turn);
      }else await dispatch(turn);
    }catch(e){if(active()){
      setError(errorMessage(e));
      if(opening&&draftRevision.current===revision)setDraft(current=>current||line);
    }}
    finally{if(!waiting&&job.current===controller){job.current=undefined;if(mount.current){setPhase('');setPendingLine('');void refresh().catch(()=>{});}}}
  }
  const auto=useRef(prepare);auto.current=prepare;
  useEffect(()=>{
    // Wait one microtask so StrictMode can finish remounting before consuming the one-shot handoff.
    let active=true;
    const run=()=>{if(!active||job.current)return;const prompt=takeOpeningAnalysis(assessment.id);if(prompt)void auto.current(prompt,true);};
    void Promise.resolve().then(run);
    window.addEventListener(openingAnalysisEvent,run);
    return()=>{active=false;window.removeEventListener(openingAnalysisEvent,run);};
  },[assessment.id]);
  async function confirm(){
    if(!approval||phase)return;
    const turn=approval;setApproval(undefined);
    try{await dispatch(turn);}catch(e){if(mount.current)setError(errorMessage(e));}
    finally{job.current=undefined;if(mount.current){setPhase('');setPendingLine('');void refresh().catch(()=>{});}}
  }
  function discard(){
    job.current?.abort();job.current=undefined;setApproval(undefined);setPendingLine('');setPhase('');
  }
  async function stop(){
    if(stopping)return;
    setStopping(true);
    try{
      if(remote.requestId||phase==='Aguardando resposta do provedor…'){
        const live=await request<typeof remote>('ai.chat.status',{assessmentId:assessment.id});
        if(live.requestId)await request('ai.chat.stop',{assessmentId:assessment.id,requestId:live.requestId});
        else throw new Error('A solicitação está mudando de etapa. Confira o estado antes de interromper.');
        await refresh();
      }else if(localRunning)await action('assessment.cancel',{id:assessment.id});
      else discard();
    }catch(e){if(mount.current)setError(errorMessage(e));}
    finally{if(mount.current)setStopping(false);}
  }
  async function attach(f?:File){
    if(!f)return;setLoadingFile(true);setError('');
    try{const prepared=await readChatAttachment(f);if(mount.current)setAttachment(prepared);}catch(e){if(mount.current)setError(errorMessage(e));}finally{if(mount.current)setLoadingFile(false);}
  }
  const disabled=busy||running||!!approval||loadingFile;
  const timeline=[...assessment.events.map(e=>({id:e.id,at:e.at,text:e.message,role:'event',projection:undefined as ProjectionInfo|undefined})),...messages.map(m=>({id:m.id,at:m.createdAt,text:m.text,role:m.role,projection:m.projection}))].sort((a,b)=>a.at.localeCompare(b.at));
  return <section className="project-chat" aria-label="Conversa do projeto">
    <div className="project-chat-log" ref={scroll} onScroll={()=>{const el=scroll.current;if(el)stick.current=el.scrollTop+el.clientHeight>=el.scrollHeight-120;}}>
      <div className="project-chat-transcript"><header><h2>{assessment.title}</h2><span>{statusLabels[assessment.status]}</span></header>
      {timeline.map(t=>t.role==='event'?<div className="chat-step" key={t.id}><span aria-hidden="true"/><div>{t.text}<time>{shortDate(t.at)}</time></div></div>:<article className={'project-chat-turn '+t.role} key={t.id}><strong>{t.role==='user'?'Você':'OpenKoda'}</strong><ProjectionNotice projection={t.projection}/><p>{t.text}</p></article>)}
      {assessment.summary&&<div className="chat-summary"><strong>Resultado da análise</strong><p>{assessment.summary}</p></div>}
      {offer&&<ChatVerification proposal={offer.proposal} scopeRef={offer.scopeRef} action={action} disabled={busy||running||!!approval} onPrepared={()=>setOffer(undefined)}/>}
      <ProjectKaliTools assessment={assessment} action={action} busy={busy||running||!!approval}/>
      {(plans??[]).filter(plan=>plan.assessmentId===assessment.id&&['proposed','approved'].includes(plan.status)&&Date.parse(plan.expiresAt)>Date.now()).map(plan=>
        <ChatPlanApproval key={plan.id} plan={plan} mode={mode} disabled={busy||!!phase||!!remote.requestId||!!approval} action={action}/>)}
      {pendingLine&&<article className="project-chat-turn user"><strong>Você</strong><p>{pendingLine}</p></article>}
      {(phase||remote.requestId)&&<p className="chat-progress" role="status"><LoaderCircle size={16}/>{remote.stopping?'Interrompendo a resposta…':phase||'Aguardando resposta do provedor…'}</p>}
      {approval&&<section className="chat-approval"><ShieldCheck size={20}/><h3>Autorizar envio para {providers[approval.selection.provider].name}?</h3><p>{approval.selection.model} · {approval.context.bytes.toLocaleString('pt-BR')} bytes preparados{approval.budget?' · reserva de '+usd(approval.budget.reservedUsd):''}.</p>{approval.image&&<img alt="Imagem selecionada para envio" src={'data:image/jpeg;base64,'+approval.image}/>}
        <details><summary>Ver conteúdo do envio</summary>{approval.context.packetPreview.prompts.map(p=><pre key={p.index}>{p.instructions}{'\n'}{p.prompt.replace(/"reviewedJpeg":"[A-Za-z0-9+/=]+"/g,'"imagem":"Prévia acima"')}</pre>)}</details>
        {approval.context.packetPreview.requiresSensitiveConfirmation&&<p>Esta autorização inclui os dados sensíveis identificados no pacote.</p>}
        <div><button type="button" className="button" onClick={discard}>Recusar</button><button type="button" className="button primary" onClick={()=>void confirm()}>Autorizar envio</button></div></section>}
      <div ref={logEnd}/></div></div>
    <div className="project-chat-bottom">
      {error&&<div className="chat-error" role="alert">{error}{onAdvanced&&<button type="button" onClick={onAdvanced}>Revisar avaliação</button>}</div>}
      <form className="project-composer" onSubmit={e=>{e.preventDefault();if(!disabled)void prepare(draft);}}>
        {attachment&&<div className="chat-attachment">{attachment.image?<img alt="Anexo selecionado" src={'data:image/jpeg;base64,'+attachment.image}/>:<FileText size={16}/>}<span>{attachment.name}</span><button type="button" disabled={disabled} onClick={()=>setAttachment(undefined)} aria-label="Remover anexo"><X size={14}/></button></div>}
        <textarea aria-label="Mensagem da conversa" rows={2} maxLength={8000} value={draft} disabled={!!approval} onChange={e=>{draftRevision.current++;setDraft(e.target.value);}} placeholder={proposing?'O que você quer verificar neste alvo?':'Pergunte sobre o resultado da análise…'} onKeyDown={e=>{if(composerSubmitKey(e.nativeEvent,composerShortcut())){e.preventDefault();if(!disabled)void prepare(draft);}}}/>
        <div className="project-composer-tools"><input type="file" hidden ref={file} onChange={e=>{void attach(e.target.files?.[0]);e.currentTarget.value='';}}/><button type="button" aria-label="Anexar foto ou arquivo" title="Anexar foto ou arquivo" disabled={disabled||proposing} onClick={()=>file.current?.click()}><Paperclip size={17}/></button>
          <button type="button" aria-pressed={proposing} disabled={disabled||!!attachment||!native} className="chat-intent-button" onClick={()=>setProposing(!proposing)}><ListChecks size={16}/>Pedir verificação</button>
          <ChatModelPicker value={sel} disabled={disabled} onModels={onModels} onChange={async choice=>{await action('ai.selection.save',{assessmentId:assessment.id,aiSelection:choice});}}/>
          <label className="chat-permission"><ShieldCheck size={16}/><select aria-label="Modo de aprovação" value={mode} disabled={disabled} onChange={e=>{const next=e.target.value as ApprovalMode;setMode(next);try{localStorage.setItem('openkoda.chat.approval.'+assessment.id,next);}catch{/* session only */}}}>{(Object.entries(approvalModes) as [ApprovalMode,string][]).map(([key,label])=><option key={key} value={key}>{label}</option>)}</select></label>
          {running?<button className="chat-send" type="button" disabled={stopping||remote.stopping} aria-label="Cancelar trabalho" title="Cancelar trabalho" onClick={()=>void stop()}><Square size={16}/></button>:<button className="chat-send" type="submit" disabled={!native||disabled||!sel||!draft.trim()} aria-label="Enviar mensagem" title="Enviar mensagem"><ArrowUp size={18}/></button>}
        </div></form>
        <small>{sel?'A conversa usa '+providers[sel.provider].name+'. O envio inclui a mensagem, os turnos recentes, o contexto do projeto e o anexo escolhido.':'Escolha um modelo ativo para conversar.'}</small>
    </div>
  </section>;
}

