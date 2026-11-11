import { useState } from 'react';
import { native } from '../api';
import { Action, Assessment, errorMessage } from '../types';
import { Field, Notice } from './shared';

export default function AssessmentReconciliation({assessment,action,busy}:{assessment:Assessment;action:Action;busy:boolean}){
  const [note,setNote]=useState('');const [acknowledged,setAcknowledged]=useState(false);const [pending,setPending]=useState(false);const [error,setError]=useState('');
  async function reconcile(){
    setPending(true);setError('');
    try{await action('assessment.reconcile',{id:assessment.id,note:note.trim(),acknowledgeUnknownEffects:true});}
    catch(e){setError(errorMessage(e));setAcknowledged(false);}finally{setPending(false);}
  }
  if(assessment.status!=='unknown')return null;
  return <section className="content-section assessment-reconciliation"><h2>Revisão da avaliação interrompida</h2>
    <Notice tone="warning">O término da operação não foi confirmado. Esta revisão registra a interrupção sem repetir a execução. Efeitos enviados antes da interrupção podem ter ocorrido e não são desfeitos.</Notice>
    {!native?<p className="muted">A reconciliação está disponível no aplicativo desktop.</p>:<>
      {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}
      <Field label="Nota sobre a interrupção" hint="Registre o que foi verificado e o que continua desconhecido. Máximo de 2.000 caracteres."><textarea value={note} maxLength={2000} rows={4} onChange={e=>setNote(e.target.value)} disabled={busy||pending}/></Field>
      <label className="check-field"><input type="checkbox" checked={acknowledged} onChange={e=>setAcknowledged(e.target.checked)} disabled={busy||pending}/><span>Entendo que efeitos já enviados não são desfeitos e que esta revisão não repete a operação.</span></label>
      <button type="button" className="button" disabled={busy||pending||!note.trim()||!acknowledged} onClick={()=>void reconcile()}>Registrar revisão da interrupção</button>
    </>}
  </section>;
}
