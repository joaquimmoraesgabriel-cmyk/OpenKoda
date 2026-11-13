import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, FileText, Folder, Globe, ListChecks, LoaderCircle, Network, ShieldCheck, Square, X } from 'lucide-react';
import { request, native } from '../api';
import { providers,type AiSelection } from '../ai-provider';
import { operationMatchesTargets,validOperationProposal,type OperationProposal } from '../ai-operation';
import type { ScopeRef } from '../scope';
import { ChatVerification } from './ChatVerification';
import { validBudgetReview,usd,type BudgetReview,type BudgetStatus } from '../ai-budget';
import { prepareChat,readChatAttachment,type ApprovalMode } from '../project-chat';
import { openingAnalysisEvent,takeOpeningAnalysis } from '../chat-opening';
import { composerShortcut,matchesComposerShortcut } from '../composer-preference';
import { errorMessage,statusLabels,type Action,type Assessment,type ExecutionPlan,type Settings } from '../types';
import { ChatPlanApproval } from './ChatPlanApproval';
import ProjectKaliTools from './ProjectKaliTools';
import { finishDraft, readDraft, updateDraft, useChatDraft, type ChatDraft } from '../chat-draft';
import { ConversationTimeline, type ChatMessage } from './ConversationTimeline';
import { ConversationText } from './ConversationText';
import { ChatModelPicker } from './ChatModelPicker';
import { ChatComposerMenus } from './ChatComposerMenus';
import { ChatSkills } from './ChatSkills';
import ChatCodeAnalysis from './ChatCodeAnalysis';
import SecurityScanPanel, { useSecurityScan } from './SecurityScan';
import { scanActive, scanStateLabel } from '../security-scan';
import './project-conversation.css';

type LiveStatus={requestId:string|null;stopping:boolean;queuePosition?:number|null;processingMs?:number|null;partialText?:string|null;messageId?:string|null;dataReview?:{id:string;digest:string}|null};
type Turn=Awaited<ReturnType<typeof prepareChat>>&{budget?:BudgetReview;selection:AiSelection;line:string;opening:boolean;draft:ChatDraft;image?:string;signal:AbortSignal};
type Props={draftKey?:string;unsaved?:boolean;analysisSource?:Assessment;onAnalysisAdmitted?:(id:string)=>void;prepareConversation?:()=>Promise<Assessment>;onDraftSelection?:(selection:AiSelection)=>Promise<void>;sourceOnly?:boolean;assessment:Assessment;settings:Settings;plans?:ExecutionPlan[];action:Action;busy:boolean;onAdvanced?:()=>void;onModels?:()=>void};
export default function ProjectConversation(props:Props){return <Conversation key={props.draftKey??props.assessment.id} {...props}/>;}

function compactionMessage(covered:number,omitted=0){
  const summary=covered===1?'O resumo inclui um turno anterior.':`O resumo inclui ${covered} turnos anteriores.`;
  if(!omitted)return summary;
  return summary+' '+(omitted===1?'Um turno autorizado ficou fora do contexto por falta de espaço.':`${omitted} turnos autorizados ficaram fora do contexto por falta de espaço.`);
}

function Conversation({draftKey,unsaved=false,analysisSource,onAnalysisAdmitted,prepareConversation,onDraftSelection,sourceOnly=false,assessment,plans,action,busy,onAdvanced,onModels}:Props){
  const inputId=draftKey??assessment.id;
  const [messages,setMessages]=useState<ChatMessage[]>([]);
  const [acceptedMessage,setAcceptedMessage]=useState<string>();
  const {text:draft,attachment,skill,proposing:requestedProposal}=useChatDraft(inputId);
  const [skillsOpen,setSkillsOpen]=useState(false);
  const proposing=!sourceOnly&&requestedProposal;
  const [mode,setMode]=useState<ApprovalMode>(()=>{
    try{const saved=localStorage.getItem('openkoda.chat.approval.'+assessment.id);return saved==='always'||saved==='scope'?saved:'risk';}catch{return 'risk';}
  });
  const [approval,setApproval]=useState<Turn>(),[phase,setPhase]=useState(''),[pendingLine,setPendingLine]=useState(''),[error,setError]=useState(''),[loadingFile,setLoadingFile]=useState(false);
  const [offer,setOffer]=useState<{proposal:OperationProposal;scopeRef:ScopeRef}>();
  const [remote,setRemote]=useState<LiveStatus>({requestId:null,stopping:false}),[stopping,setStopping]=useState(false);
  const mount=useRef(true),job=useRef<AbortController|undefined>(undefined),file=useRef<HTMLInputElement>(null),stamp=useRef(0);
  const scroll=useRef<HTMLDivElement>(null),transcript=useRef<HTMLDivElement>(null),stick=useRef(true),input=useRef<HTMLTextAreaElement>(null);
  const [awayFromEnd,setAwayFromEnd]=useState(false);
  const [analyzing,setAnalyzing]=useState(false);
  const [compactionNotice,setCompactionNotice]=useState('');
  const [compactNext,setCompactNext]=useState(false);
  const sel:AiSelection|null=assessment.aiSelection??null;
  const scanner=useSecurityScan(assessment,!unsaved);
  const scanRunning=!!scanner.scan && scanActive(scanner.scan.status);
  const localRunning=!sourceOnly&&['queued','preparing','planning','running','pausing','canceling'].includes(assessment.status);
  const running=!!phase||!!remote.requestId||localRunning||scanRunning;
  const refresh=useCallback(async()=>{
    if(!native||unsaved)return;
    const version=++stamp.current;
    const history=await request<{messages:ChatMessage[]}>('ai.history',{assessmentId:assessment.id});
    if(!mount.current||version!==stamp.current)return;
    if(Array.isArray(history?.messages)){
      setMessages(history.messages.filter(m=>m&&typeof m.id==='string'&&typeof m.text==='string'&&typeof m.createdAt==='string'&&['user','assistant'].includes(m.role)));
    }
  },[assessment.id,unsaved]);
  const liveKey=useRef<string|null|undefined>(undefined);
  useEffect(()=>{
    mount.current=true;
    if(unsaved||!native)return;
    let disposed=false,timer:number;
    const poll=async()=>{
      let active=false;
      try{
        const state=await request<LiveStatus>('ai.chat.status',{assessmentId:assessment.id});
        if(disposed)return;
        if(state&&(state.requestId===null||typeof state.requestId==='string')&&typeof state.stopping==='boolean'){
          active=!!state.requestId;
          setRemote(state);
          if(liveKey.current!==state.requestId||!active){
            liveKey.current=state.requestId;
            await refresh();
          }
        }else {await refresh();}
      }catch(e){if(!disposed){setRemote({requestId:null,stopping:false});setError(errorMessage(e));}}
      if(!disposed)timer=window.setTimeout(poll,active||phase?350:localRunning?850:5500);
    };
    void poll();
    const changed=()=>{setRemote(state=>({...state,partialText:null}));void refresh().catch(()=>{});};
    window.addEventListener('openkoda:data-policy-change',changed);
    return ()=>{disposed=true;window.clearTimeout(timer);window.removeEventListener('openkoda:data-policy-change',changed);};
  },[assessment.id,refresh,phase,localRunning,unsaved]);
  useEffect(()=>()=>{mount.current=false;job.current?.abort();},[]);
  useLayoutEffect(()=>{
    const pane=scroll.current, content=transcript.current;
    if(!pane||!content)return;
    function syncScroll(){
      if(!pane||!pane.clientHeight)return;
      if(stick.current)pane.scrollTop=pane.scrollHeight;
      setAwayFromEnd(pane.scrollHeight-pane.clientHeight-pane.scrollTop>64);
    }
    const observer=new ResizeObserver(syncScroll);
    observer.observe(pane);observer.observe(content);syncScroll();
    return ()=>observer.disconnect();
  },[]);
  function followEnd(){
    const pane=scroll.current;
    if(!pane)return;
    stick.current=true;pane.scrollTop=pane.scrollHeight;setAwayFromEnd(false);
    input.current?.focus({preventScroll:true});
  }
  async function dispatch(turn:Turn){
    const assessmentId=turn.payload.assessmentId;
    if(turn.signal.aborted||!mount.current)return;
    setApproval(undefined);setPhase('Conectando ao provedor…');
    const payload={...turn.payload,allowRemote:true,acknowledgeSensitive:turn.context.packetPreview.requiresSensitiveConfirmation,...(turn.budget?{budgetReview:{id:turn.budget.id,digest:turn.budget.digest}}:{})};
    if(turn.operation==='operation_proposal'){
      const result=await action('ai.operation.propose',payload);
      if(!validOperationProposal(result,assessmentId)||!turn.allowedAdapters.includes(result.adapter)||result.contextDigest!==turn.context.digest||!turn.scopeRef||!operationMatchesTargets(result,turn.payload.contextTargetRefs,turn.payload.operationTargetRef)){
        throw new Error('O modelo não retornou uma verificação válida para o escopo desta conversa.');
      }
      if(mount.current)setOffer({proposal:result,scopeRef:turn.scopeRef});
    }else {
      // The request promise resolves only after the provider answers. Clear the
      // submitted snapshot when the native host confirms this exact admitted
      // review and its durable user message, including after navigating away.
      let settled=false,accepted=false;
      const acceptMessage=(messageId:string)=>{
        if(settled||accepted)return;
        accepted=true;
        window.dispatchEvent(new Event('openkoda:chat-admitted'));
        if(!turn.opening)finishDraft(inputId,turn.draft);
        if(mount.current){setAcceptedMessage(messageId);void refresh().catch(()=>{});}
      };
      const acknowledge=async()=>{
        if(settled||accepted)return;
        try{
          const state=await request<LiveStatus>('ai.chat.status',{assessmentId});
          const review=turn.payload.dataReview;
          if(settled||accepted||!state.requestId||typeof state.messageId!=='string'||
            !review||state.dataReview?.id!==review.id||state.dataReview?.digest!==review.digest)return;
          acceptMessage(state.messageId);
        }catch{/* A missing acknowledgement never clears unsent input. */}
      };
      const answer=action('ai.ask',payload);
      void acknowledge();
      const timer=window.setInterval(()=>void acknowledge(),400);
      try{await answer;}catch(error){
        // A fast provider refusal can finish between status polls. The exact
        // review and durable user-message link also confirm that admission.
        if(!accepted)try{
          const history=await request<{messages:ChatMessage[];requests?:{dataReviewRef?:{id:string;digest:string};chatMessageIds?:unknown}[]}>('ai.history',{assessmentId});
          const review=turn.payload.dataReview;
          const receipt=history.requests?.find(receipt=>receipt.dataReviewRef?.id===review.id&&receipt.dataReviewRef?.digest===review.digest);
          const ids=Array.isArray(receipt?.chatMessageIds)?receipt.chatMessageIds:[];
          const message=history.messages?.find(message=>message.role==='user'&&ids.includes(message.id));
          if(message)acceptMessage(message.id);
        }catch{/* Keep unsent input when the host cannot confirm acceptance. */}
        throw error;
      }finally{settled=true;window.clearInterval(timer);}
    }
    if(!turn.opening)finishDraft(inputId,turn.draft);
    if(mount.current)await refresh();
  }
  async function prepare(line:string,opening=false,disableAutoSkill=false){
    if(job.current||!native||!line.trim()||!sel)return;
    const controller=new AbortController();job.current=controller;
    const active=()=>!controller.signal.aborted&&mount.current;
    let waiting=false;
    const picked=opening?undefined:attachment;
    const submittedDraft=readDraft(inputId);
    const message=picked?.text!==undefined?line+'\n\nArquivo selecionado ('+picked.name+'):\n'+picked.text:line;
    setError('');setAcceptedMessage(undefined);setPendingLine(line);setPhase('Preparando o contexto…');
    try{
      if(new TextEncoder().encode(message).byteLength>8000)throw new Error('A mensagem e o texto anexado podem ter até 8.000 bytes. Envie um trecho menor.');
      const target=prepareConversation?await prepareConversation():assessment;
      if(!active())return;
      const prepared=await prepareChat(target,sel,message,{image:picked?.image,file:!!picked,signal:controller.signal,propose:proposing&&!opening,compact:compactNext,skillId:opening?undefined:skill?.id,disableAutoSkill});
      setCompactionNotice(prepared.context.compactedTurns?compactionMessage(prepared.context.compactedTurns,prepared.context.omittedTurns):'');
      setCompactNext(false);
      if(!active())return;
      const budget=await request<BudgetStatus>('ai.budget.status',{assessmentId:target.id});
      if(!budget||typeof budget.enabled!=='boolean'||budget.degraded)throw new Error('Verifique o orçamento nas configurações antes de enviar.');
      let estimate:BudgetReview|undefined;
      if(budget.enabled){
        const b=await request<{review:BudgetReview|null}>('ai.budget.preview',{operation:prepared.operation,payload:prepared.payload});
        if(!validBudgetReview(b.review))throw new Error('Não foi possível confirmar o custo desta solicitação.');
        estimate=b.review;
      }
      if(!active())return;
      const turn={...prepared,selection:sel,budget:estimate,line,opening,draft:submittedDraft,image:picked?.image,signal:controller.signal};
      if(estimate||prepared.context.autoSkill||disableAutoSkill||prepared.context.packetPreview.requiresSensitiveConfirmation||(!opening&&(mode==='always'||(mode==='risk'&&(!!picked||!!skill))))){
        waiting=true;setPhase('');setApproval(turn);
      }else await dispatch(turn);
    }catch(e){if(active()){
      setError(errorMessage(e));
      if(opening&&readDraft(inputId)===submittedDraft&&!submittedDraft.text)updateDraft(inputId,{text:line});
    }}
    finally{if(!waiting&&job.current===controller){job.current=undefined;if(mount.current){setPhase('');setPendingLine('');void refresh().catch(()=>{});}}}
  }
  const auto=useRef(prepare);auto.current=prepare;
  useEffect(()=>{
    if(sourceOnly)return;
    // Wait one microtask so StrictMode can finish remounting before consuming the one-shot handoff.
    let active=true;
    const run=()=>{if(!active||job.current)return;const prompt=takeOpeningAnalysis(assessment.id);if(prompt)void auto.current(prompt,true);};
    void Promise.resolve().then(run);
    window.addEventListener(openingAnalysisEvent,run);
    return()=>{active=false;window.removeEventListener(openingAnalysisEvent,run);};
  },[assessment.id,sourceOnly]);
  async function confirm(){
    if(!approval||phase)return;
    const turn=approval;setApproval(undefined);
    try{await dispatch(turn);}catch(e){if(mount.current)setError(errorMessage(e));}
    finally{job.current=undefined;if(mount.current){setPhase('');setPendingLine('');void refresh().catch(()=>{});}}
  }
  function discard(){
    job.current?.abort();job.current=undefined;setApproval(undefined);setPendingLine('');setPhase('');
  }
  async function reviewWithoutSkill(){
    if(!approval?.context.autoSkill||phase)return;
    const current=approval;
    setPhase('Removendo a skill sugerida…');
    try{
      await request('data.review.cancel',{review:current.context.dataReview});
      if(current.signal.aborted||!mount.current)return;
      discard();
      await prepare(current.line,current.opening,true);
    }catch(e){if(!current.signal.aborted&&mount.current){setPhase('');setError(errorMessage(e));}}
  }
  async function stop(){
    if(stopping)return;
    setStopping(true);
    try{
      if(remote.requestId||phase==='Conectando ao provedor…'){
        const live=await request<typeof remote>('ai.chat.status',{assessmentId:assessment.id});
        if(live.requestId)await request('ai.chat.stop',{assessmentId:assessment.id,requestId:live.requestId});
        else throw new Error('A solicitação está mudando de etapa. Confira o estado antes de interromper.');
        await refresh();
      }else if(scanRunning)await scanner.cancel();
      else if(localRunning)await action('assessment.cancel',{id:assessment.id});
      else discard();
    }catch(e){if(mount.current)setError(errorMessage(e));}
    finally{if(mount.current)setStopping(false);}
  }
  async function attach(f?:File){
    if(!f)return;setLoadingFile(true);setError('');
    try{const prepared=await readChatAttachment(f);if(mount.current)updateDraft(inputId,{attachment:prepared});}catch(e){if(mount.current)setError(errorMessage(e));}finally{if(mount.current)setLoadingFile(false);}
  }
  const disabled=busy||running||!!approval||loadingFile;
  const shortcut=composerShortcut();
  const canSend=native && !disabled && !!sel && !!draft.trim();
  const slash=sel?.provider==='openkoda'&&!skill&&/^\/[\w-]*$/.test(draft);
  async function compact(){
    if(!sel||job.current||disabled||unsaved)return;
    const controller=new AbortController();job.current=controller;setPhase('Compactando o contexto…');setError('');
    try{
      const result=await prepareChat(assessment,sel,'Compactação local do contexto da conversa.',{compact:true,disableAutoSkill:true,signal:controller.signal});
      await request('data.review.cancel',{review:result.context.dataReview});
      if(mount.current){
        const count=result.context.compactedTurns??0;
        setCompactionNotice(count?compactionMessage(count,result.context.omittedTurns):'Nenhum turno autorizado para compactar.');
        setCompactNext(count>0);
      }
    }catch(e){if(mount.current)setError(errorMessage(e));}
    finally{if(job.current===controller)job.current=undefined;if(mount.current)setPhase('');}
  }
  function changeApproval(next:ApprovalMode){
    setMode(next);
    try{localStorage.setItem('openkoda.chat.approval.'+assessment.id,next);}catch{/* The choice still applies to this conversation. */}
  }
  const TargetIcon=assessment.kind==='web'?Globe:assessment.kind==='network'?Network:Folder;
  const processing=typeof remote.processingMs==='number' && remote.requestId;
  const seconds=Math.floor((remote.processingMs??0)/1000);
  const elapsed=seconds<60?seconds+' s':Math.floor(seconds/60)+' min '+(seconds%60)+' s';
  const partial=remote.requestId&&typeof remote.partialText==='string'?remote.partialText:'';
  const workLabel=remote.stopping||stopping?'Interrompendo o trabalho…':remote.requestId && remote.queuePosition
    ? 'Aguardando vaga · posição '+remote.queuePosition : processing
    ? (partial?'Respondendo':'Processando')+' · '+elapsed
    : phase|| (remote.requestId?'Conectando ao provedor…':localRunning?statusLabels[assessment.status]:'');
  return <section className="project-chat" aria-label="Conversa do projeto">
    <div className="project-chat-log" tabIndex={0} role="region" aria-label="Histórico da conversa" ref={scroll}
      onScroll={e=>{const el=e.currentTarget;if(!el.clientHeight)return;stick.current=el.scrollHeight-el.clientHeight-el.scrollTop<=64;setAwayFromEnd(!stick.current);}}
      onClickCapture={e=>{if(e.target instanceof Element && e.target.closest('summary'))stick.current=false;}}>
      <div className="project-chat-transcript" ref={transcript}>{!unsaved&&!draftKey&&(!sourceOnly||!!scanner.scan||messages.length>0)&&<header className="chat-heading">
        <div><h2>{assessment.title}</h2><span className="chat-state" title={scanner.scan?'Estado da análise do OpenKoda':'Estado da avaliação'} data-state={scanner.scan?.status??assessment.status}>{scanner.scan?scanStateLabel[scanner.scan.status]:statusLabels[assessment.status]}</span></div>
        <p title={assessment.target}><TargetIcon size={14}/><span>{assessment.target}</span></p>
      </header>}
      {scanner.scan&&assessment.events.some(e=>e.kind==='chat_scan_first_turn')&&<article className="project-chat-turn user"><header><strong>Você</strong></header><p>{assessment.objective}</p></article>}
      <ConversationTimeline events={assessment.events.filter(e=>e.kind!=='chat_scan_first_turn'&&!e.kind.startsWith('chat_first_turn')&&(!sourceOnly||!e.message.startsWith('Conversa criada.')))} messages={messages}
        onContinue={disabled||draft||attachment||skill||proposing||analyzing?undefined:()=>{
          updateDraft(inputId,{text:'Continue a resposta anterior do ponto em que ela parou.'});
          input.current?.focus();
        }}/>
      {(!draftKey && (!sourceOnly || !assessment.events.some(e=>e.kind.startsWith('chat_first_turn')||e.message.startsWith('Conversa criada.'))) || !!scanner.scan || !!scanner.error)&&<SecurityScanPanel assessment={assessment} controller={scanner}/>}
      {assessment.summary&&<section className="chat-summary"><h3>Resultado da análise</h3><ConversationText text={assessment.summary}/></section>}
      {!sourceOnly&&offer&&<ChatVerification proposal={offer.proposal} scopeRef={offer.scopeRef} action={action} disabled={busy||running||!!approval} onPrepared={()=>setOffer(undefined)}/>}
      {!sourceOnly&&<ProjectKaliTools assessment={assessment} action={action} busy={busy||running||!!approval}/>}
      {!sourceOnly&&(plans??[]).filter(plan=>plan.assessmentId===assessment.id&&['proposed','approved'].includes(plan.status)&&Date.parse(plan.expiresAt)>Date.now()).map(plan=>
        <ChatPlanApproval key={plan.id} plan={plan} mode={mode} disabled={busy||!!phase||!!remote.requestId||!!approval} action={action}/>)}
      {pendingLine&&!messages.some(message=>message.id===acceptedMessage)&&<article className="project-chat-turn user"><header><strong>Você</strong></header><p>{pendingLine}</p></article>}
      {partial&&<article className="project-chat-turn assistant" data-live-answer={remote.requestId} aria-label="Resposta em andamento"><header><strong>OpenKoda</strong></header><ConversationText text={partial}/></article>}
      {workLabel&&<p className="chat-progress" role="status"><LoaderCircle size={16}/>{workLabel}</p>}
      {approval&&<section className="chat-approval"><ShieldCheck size={20}/><h3>Autorizar envio para {providers[approval.selection.provider].name}?</h3><p>{approval.selection.model} · {approval.context.bytes.toLocaleString('pt-BR')} bytes preparados{approval.budget?' · reserva de '+usd(approval.budget.reservedUsd):''}.</p>{approval.image&&<img alt="Imagem selecionada para envio" src={'data:image/jpeg;base64,'+approval.image}/>}
        {approval.context.autoSkill&&<p>Skill sugerida para esta mensagem: <strong>/{approval.context.autoSkill}</strong>. As instruções estão no conteúdo do envio.</p>}
        <details><summary>Ver conteúdo do envio</summary>{approval.context.packetPreview.prompts.map(p=><pre key={p.index}>{p.instructions}{'\n'}{p.prompt.replace(/"reviewedJpeg":"[A-Za-z0-9+/=]+"/g,'"imagem":"Prévia acima"')}</pre>)}</details>
        {approval.context.packetPreview.requiresSensitiveConfirmation&&<p>Esta autorização inclui os dados sensíveis identificados no pacote.</p>}
        <div><button type="button" className="button" onClick={discard}>Recusar</button>{approval.context.autoSkill&&<button type="button" className="button" disabled={!!phase} onClick={()=>void reviewWithoutSkill()}>Revisar sem a skill</button>}<button type="button" className="button primary" disabled={!!phase} onClick={()=>void confirm()}>Autorizar envio</button></div></section>}
      </div></div>
    <div className="project-chat-bottom">
      {awayFromEnd&&<button type="button" className="chat-jump" aria-label="Voltar à mensagem mais recente" title="Voltar à mensagem mais recente" onClick={followEnd}><ArrowDown size={17}/></button>}
      {error&&<div className="chat-error" role="alert">{error}{onAdvanced&&<button type="button" onClick={onAdvanced}>Revisar avaliação</button>}</div>}
      {compactionNotice&&<p role="status">{compactionNotice}</p>}
      {sel?.provider==='openkoda'&&(skillsOpen||slash)&&<ChatSkills query={slash?draft.slice(1):''}
        onSelect={item=>{updateDraft(inputId,{skill:item,...(slash?{text:''}:{})});setSkillsOpen(false);input.current?.focus();}}
        onClose={()=>{setSkillsOpen(false);if(slash)updateDraft(inputId,{text:''});}}/>}
      {analyzing&&sel&&onAnalysisAdmitted?<ChatCodeAnalysis draftId={inputId} source={analysisSource} selection={sel} projectId={assessment.projectId}
        newConversation={!unsaved} action={action} onClose={()=>setAnalyzing(false)} onAdmitted={onAnalysisAdmitted}/>:
      <><form className="project-composer" onSubmit={e=>{e.preventDefault();if(canSend)void prepare(draft);}}>
        {skill&&<div className="chat-attachment"><span>/{skill.id} · {skill.name}</span><button type="button" aria-label="Remover skill" disabled={disabled} onClick={()=>updateDraft(inputId,{skill:undefined})}><X size={14}/></button></div>}
        {attachment&&<div className="chat-attachment">{attachment.image?<img alt="Anexo selecionado" src={'data:image/jpeg;base64,'+attachment.image}/>:<FileText size={16}/>}<span>{attachment.name}</span><button type="button" disabled={disabled} onClick={()=>updateDraft(inputId,{attachment:undefined})} aria-label="Remover anexo"><X size={14}/></button></div>}
        {proposing&&<div className="chat-attachment"><ListChecks size={16}/><span>Verificação</span><button type="button" disabled={disabled} aria-label="Voltar à conversa" onClick={()=>updateDraft(inputId,{proposing:false})}><X size={14}/></button></div>}
        <textarea ref={input} aria-label="Mensagem da conversa" rows={2} maxLength={8000} value={draft} disabled={!!approval} onChange={e=>updateDraft(inputId,{text:e.target.value})} placeholder={proposing?'O que você quer verificar neste alvo?':'Envie uma mensagem sobre o projeto…'} onKeyDown={e=>{
          if(!matchesComposerShortcut(e.nativeEvent,composerShortcut()))return;
          e.preventDefault();
          if(!e.repeat && canSend)e.currentTarget.form?.requestSubmit();
        }}/>
        <div className="project-composer-tools"><input type="file" hidden ref={file} onChange={e=>{void attach(e.target.files?.[0]);e.currentTarget.value='';}}/>
          <ChatComposerMenus mode={mode} onMode={changeApproval} proposing={proposing} hasAttachment={!!attachment} disabled={disabled} canPropose={native&&!sourceOnly}
            onCompact={native&&!unsaved&&messages.length>0?()=>void compact():undefined}
            onSkills={native&&sel?.provider==='openkoda'?()=>setSkillsOpen(true):undefined}
            onAnalyze={sourceOnly&&native&&sel&&onAnalysisAdmitted?()=>setAnalyzing(true):undefined}
            onAttach={()=>file.current?.click()} onPropose={()=>updateDraft(inputId,{proposing:!proposing})}/>
          <ChatModelPicker value={sel} disabled={disabled} onModels={onModels} onChange={async choice=>{if(onDraftSelection){await onDraftSelection(choice);return;}await action('ai.selection.save',{assessmentId:assessment.id,aiSelection:choice});}}/>
          {running?<button className="chat-send" type="button" disabled={stopping||remote.stopping||scanner.busy||scanner.scan?.status==='cancelling'} aria-label="Cancelar trabalho" title="Cancelar trabalho" onClick={()=>void stop()}><Square size={14} fill="currentColor"/></button>:<button className="chat-send" type="submit" disabled={!canSend} aria-label="Enviar mensagem" aria-keyshortcuts={shortcut==='enter'?'Enter':'Control+Enter Meta+Enter'} title={shortcut==='enter'?'Enviar mensagem (Enter)':'Enviar mensagem (Ctrl + Enter)'}><ArrowUp size={18}/></button>}
        </div></form>
        <small>{sel?providers[sel.provider].name+' recebe a mensagem e o contexto do projeto'+(proposing?'.':', incluindo turnos recentes e o anexo escolhido.'):'Escolha um modelo ativo para conversar.'}</small></>}
    </div>
  </section>;
}

