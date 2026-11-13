import {useEffect,useState} from 'react';
import {request} from '../api';
import type {Skill} from '../commercial';
import {errorMessage} from '../types';

type Document=Skill&{body:string;source:string;license:string;resources:Record<string,string>};
export function ChatSkills({query,onSelect,onClose}:{query:string;onSelect:(s:Skill)=>void;onClose:()=>void}){
  const [items,setItems]=useState<Skill[]>(),[error,setError]=useState(''),[doc,setDoc]=useState<Document>();
  useEffect(()=>{let current=true;void request<{skills:Skill[]}>('account.skills').then(result=>{
    if(current)setItems(Array.isArray(result.skills)?result.skills:[]);
  }).catch(e=>{if(current)setError(errorMessage(e));});return ()=>{current=false;};},[]);
  const found=items?.filter(s=>(s.id+' '+s.name).toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  return <section className="chat-skills" aria-label="Skills da assinatura">
    <div className="inline-actions"><strong>Skills da assinatura</strong><button type="button" onClick={onClose}>Fechar skills</button></div>
    {error&&<p role="alert">{error}</p>}
    {!items&&!error&&<p role="status">Carregando skills…</p>}
    {found?.length===0&&<p>Nenhuma skill encontrada.</p>}
    {found?.map(s=><div key={s.id}>
      <button type="button" onClick={()=>onSelect(s)}><strong>/{s.id}</strong> · {s.name}</button>
      <p>{s.description}</p>
      <button type="button" onClick={()=>void request<Document>('account.skill',{id:s.id}).then(setDoc).catch(e=>setError(errorMessage(e)))}>Detalhes de {s.name}</button>
    </div>)}
    {doc&&<details open><summary>{doc.name} · {doc.revision}</summary><p>{doc.invocation} · {doc.source}</p>
      <pre>{doc.body}</pre>{Object.entries(doc.resources??{}).map(([name,body])=><details key={name}><summary>{name}</summary><pre>{body}</pre></details>)}
      <details><summary>Licença</summary><pre>{doc.license}</pre></details></details>}
  </section>;
}
