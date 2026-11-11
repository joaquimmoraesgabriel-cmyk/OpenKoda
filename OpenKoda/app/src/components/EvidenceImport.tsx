import { ContextTargets, useContextTargets } from './TargetSelection';
import { ScopeRestriction, useScopeAccess } from './ScopeAccess';
import { useEffect, useState } from 'react';
import { native } from '../api';
import { Action, Assessment, errorMessage } from '../types';
import { Disclosure, Field, Notice } from './shared';

export default function EvidenceImport({assessment,projectActive,action,busy}:{assessment:Assessment;projectActive:boolean;action:Action;busy:boolean}){
  const scope=useScopeAccess();const targets=useContextTargets();
  const [name,setName]=useState('');const [source,setSource]=useState('');const [content,setContent]=useState('');const [ack,setAck]=useState(false);
  const [pending,setPending]=useState(false);const [error,setError]=useState('');const [message,setMessage]=useState('');
  useEffect(()=>setAck(false),[targets.key]);
  const bytes=new TextEncoder().encode(content).length;
  const allowed=scope.canImport&&projectActive&&['draft','blocked','partial','completed','failed','canceled'].includes(assessment.status);
  async function save(){
    setPending(true);setError('');setMessage('');
    try{
      await action('evidence.import',{assessmentId:assessment.id,...targets.payload,name:name.trim(),sourceDescription:source.trim(),content,acknowledgeUnverifiedSource:true});
      setName('');setSource('');setContent('');setAck(false);setMessage('Texto registrado com origem declarada não verificada. Nenhum conteúdo foi enviado à IA.');
    }catch(e){setError(errorMessage(e));}finally{setPending(false);}
  }
  return <Disclosure label="Importar texto"><div className="evidence-import">
    {!native?<Notice>A importação de texto está disponível no aplicativo desktop. A prévia não registra evidências nativas.</Notice>:<>
      <Notice>O texto será preservado literalmente como conteúdo fornecido por você, com origem não verificada. O registro não confirma a alegação contida no texto e não envia dados à IA automaticamente.</Notice>
      {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}{message&&<p role="status">{message}</p>}
      {!allowed&&<Notice tone="warning">Mantenha o projeto ativo e encerre ou reconcilie a atividade pendente da avaliação antes de importar texto.</Notice>}
      <ContextTargets selection={targets} label="Entradas associadas ao texto importado" disabled={busy||pending||!allowed}/><ScopeRestriction kind="import"/><Field label="Nome do texto importado"><input value={name} onChange={e=>setName(e.target.value)} maxLength={160} disabled={busy||pending||!allowed}/></Field>
      <Field label="Origem declarada" hint="Descreva de onde o texto veio. Essa origem não será verificada automaticamente."><textarea value={source} onChange={e=>setSource(e.target.value)} rows={2} maxLength={1000} disabled={busy||pending||!allowed}/></Field>
      <Field label="Conteúdo textual" hint={`${bytes.toLocaleString('pt-BR')} de 32.768 bytes UTF-8`}><textarea value={content} onChange={e=>setContent(e.target.value)} rows={6} maxLength={32768} disabled={busy||pending||!allowed} spellCheck={false}/></Field>
      {bytes>32768&&<p role="alert">O texto excede 32 KiB em UTF-8. Reduza o conteúdo antes de importar.</p>}
      <label className="check-field"><input type="checkbox" checked={ack} onChange={e=>setAck(e.target.checked)} disabled={busy||pending||!allowed}/><span>Declaro a origem deste texto e entendo que ela não foi verificada pelo OpenKoda.</span></label>
      <button type="button" className="button" disabled={busy||pending||!allowed||!targets.ready||!name.trim()||!source.trim()||!content.trim()||bytes>32768||!ack} onClick={()=>void save()}>Registrar texto com origem declarada</button>
    </>}
  </div></Disclosure>;
}
