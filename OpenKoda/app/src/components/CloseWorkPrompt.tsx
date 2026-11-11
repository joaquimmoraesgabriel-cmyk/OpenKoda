import { useEffect, useRef, useState } from 'react';
import { native } from '../api';
import './CloseWorkPrompt.css';

export type CloseActivity = {area: 'queue'|'assessment'|'kali'|'settings'|'current'; assessmentId: string|null};

const labels = {
  queue:'Revisar plano e fila', assessment:'Revisar avaliação', kali:'Revisar operação no Kali',
  settings:'Revisar configurações', current:'Voltar à operação',
};

export default function CloseWorkPrompt({onReview}:{onReview:(work:CloseActivity)=>void}) {
  const [activity,setActivity]=useState<CloseActivity>();
  const dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{
    if(!native)return;
    function notify(event:Event){
      const detail=(event as CustomEvent<CloseActivity>).detail;
      if(!detail || !Object.hasOwn(labels,detail.area) || (detail.assessmentId!==null && typeof detail.assessmentId!=='string'))return;
      setActivity(detail);
    }
    window.addEventListener('openkoda-close-review',notify);
    return ()=>window.removeEventListener('openkoda-close-review',notify);
  },[]);
  useEffect(()=>{
    if(activity && dialog.current && !dialog.current.open)dialog.current.showModal();
  },[activity]);
  function dismiss(){
    dialog.current?.close();
    setActivity(undefined);
  }
  if(!activity)return null;
  return <dialog ref={dialog} className="close-work-dialog" aria-labelledby="close-work-title" aria-describedby="close-work-help"
    onCancel={event=>{event.preventDefault();dismiss();}}>
    <h2 id="close-work-title">Há uma operação em andamento</h2>
    <p id="close-work-help">O OpenKoda manteve a janela aberta. Confira o andamento ou use os controles de pausa e cancelamento disponíveis para essa operação. Quando ela terminar, feche a janela novamente.</p>
    <div className="inline-actions"><button className="button primary" type="button" autoFocus onClick={dismiss}>Manter aberto</button>
      <button className="button" type="button" onClick={()=>{dismiss();onReview(activity);}}>{labels[activity.area]}</button></div>
  </dialog>;
}

