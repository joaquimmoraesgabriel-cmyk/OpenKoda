import { useEffect, useRef, useState } from 'react';
import { request } from '../api';
import { providers, type AiSelection } from '../ai-provider';
import { conversationTitle, finishDraft, readDraft, updateDraft, useChatDraft } from '../chat-draft';
import { effectiveScanLimit, loadProjectOptions } from '../project-analysis-options';
import { analysisFileAccessNotice, readScan, SensitiveScanConfirmation, startSecurityScan } from '../security-scan';
import { errorMessage, type Action, type Assessment } from '../types';

type Props = {
  draftId:string; source?:Assessment; projectId:string; selection:AiSelection;
  newConversation:boolean; action:Action; onClose:()=>void; onAdmitted:(id:string)=>void;
};

export default function ChatCodeAnalysis({draftId,source,projectId,selection,newConversation,action,onClose,onAdmitted}:Props){
  const draft=useChatDraft(draftId);
  const [limit,setLimit]=useState<number>();
  const [error,setError]=useState('');
  const [sending,setSending]=useState(false);
  const [sensitive,setSensitive]=useState<string>();
  const [consent,setConsent]=useState(false);
  const attempt=useRef<{intent:string;id:string}|undefined>(undefined);
  const job=useRef<AbortController|undefined>(undefined);
  const formKey=JSON.stringify([source?.id,projectId,selection,draft.text.trim(),limit]);
  useEffect(()=>{
    let current=true;
    loadProjectOptions(projectId).then(options=>{if(current)setLimit(effectiveScanLimit(options));})
      .catch(e=>{if(current)setError(errorMessage(e));});
    return ()=>{current=false;job.current?.abort();};
  },[projectId]);

  async function submit(){
    if(job.current||!limit||!draft.text.trim())return;
    const controller=new AbortController();
    job.current=controller;
    const submitted=readDraft(draftId);
    setSending(true);setError('');
    try{
      const previous=source?readScan(await request('security.scan.get',{assessmentId:source.id}),source.id):null;
      if(previous&&!Array.isArray(previous.selectedPaths))throw new Error('A seleção de arquivos desta análise está indisponível. Selecione o alvo ao criar um projeto.');
      const paths=previous?.selectedPaths;
      const intent=JSON.stringify([formKey,paths]);
      if(attempt.current?.intent!==intent)attempt.current={intent,id:crypto.randomUUID()};
      const key=attempt.current.id;
      controller.signal.throwIfAborted();
      const created=await action<Assessment>('project.newConversation',{
        projectId,sourceAssessmentId:source?.id,firstTurn:true,analysisObjective:submitted.text.trim(),
        title:conversationTitle(submitted.text),aiSelection:selection,idempotencyKey:key,
      });
      const target=paths?.length?paths:undefined;
      // Omit the projected objective: the host sends the exact classified value stored on this assessment.
      await startSecurityScan({projectId,assessmentId:created.id,selection,idempotencyKey:key,
        maxCostUsd:limit,target,acknowledgeSensitive:sensitive===formKey&&consent,signal:controller.signal});
      await action('snapshot');
      finishDraft(draftId,submitted);
      if(!controller.signal.aborted)onAdmitted(created.id);
    }catch(e){
      if(!controller.signal.aborted){
        if(e instanceof SensitiveScanConfirmation){setSensitive(formKey);setConsent(false);}
        setError(errorMessage(e));
      }
    }finally{job.current=undefined;if(!controller.signal.aborted)setSending(false);}
  }
  return <form className="project-composer" aria-label="Analisar código do projeto" onSubmit={e=>{e.preventDefault();void submit();}}>
    <label>Objetivo da análise<textarea autoFocus required maxLength={4000} rows={2} disabled={sending} value={draft.text}
      onChange={e=>{updateDraft(draftId,{text:e.target.value});setConsent(false);}}/></label>
    <small>{analysisFileAccessNotice}</small>
    <small>{providers[selection.provider].name} · {selection.model}{limit!==undefined?' · Limite de US$ '+limit.toLocaleString('pt-BR'):''}
      {newConversation?' · A análise abre uma nova conversa neste projeto.':''}</small>
    {sensitive===formKey&&<label><input type="checkbox" checked={consent} disabled={sending} onChange={e=>setConsent(e.target.checked)}/>Autorizo enviar esta fonte sensível ao provedor selecionado.</label>}
    {error&&<p role="alert">{error}</p>}
    <div className="inline-actions"><button type="button" className="button" disabled={sending} onClick={onClose}>Voltar à conversa</button>
      <button className="button primary" disabled={!limit||sending||!draft.text.trim()||sensitive===formKey&&!consent}>{sending?'Preparando análise…':'Iniciar análise de código'}</button></div>
  </form>;
}
