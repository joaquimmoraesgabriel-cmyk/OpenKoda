import { useEffect, useState } from 'react';
import { RefreshCw, Search } from 'lucide-react';
import { request } from '../api';
import { useDesktopSession } from '../desktop-session';
import type { ModelCatalogEntry } from '../model-catalog';
import { errorMessage } from '../types';
import { KodaAccessCard, kodaAccess } from './KodaProvider';

export function AccountModelCatalog(){
  const session=useDesktopSession();
  const access=kodaAccess(session);
  const [entries,setEntries]=useState<ModelCatalogEntry[]>([]),[note,setNote]=useState(''),[query,setQuery]=useState('');
  const [error,setError]=useState(''),[pending,setPending]=useState(false),[revision,setRevision]=useState(0);
  useEffect(()=>{
    let current=true;setEntries([]);setNote('');setError('');setPending(false);
    if(!access.catalog)return;
    setPending(true);
    void request<{entries?:ModelCatalogEntry[];note?:string}>('account.models').then(result=>{
      if(current){setEntries(result.entries??[]);setNote(result.note??'');}
    }).catch(cause=>{if(current)setError(errorMessage(cause));}).finally(()=>{if(current)setPending(false);});
    return()=>{current=false;};
  },[session.user?.id,access.catalog,access.allowed,revision]);
  const search=query.trim().toLocaleLowerCase('pt-BR');
  const visible=entries.filter(entry=>(entry.id+' '+entry.name).toLocaleLowerCase('pt-BR').includes(search));
  const allowed=access.allowed;
  return <section className="account-model-catalog content-section"><KodaAccessCard/>
    {access.catalog&&<div className="catalog-section-heading"><div><h3>Catálogo da assinatura</h3><p>Modelos consultados pelo serviço Koda, com suas capacidades e restrições.</p></div><button className="button" disabled={pending} onClick={()=>setRevision(value=>value+1)}><RefreshCw size={15}/>Atualizar</button></div>}
    {!!entries.length&&<><label className="settings-search"><Search size={15}/><input type="search" aria-label="Pesquisar catálogo da assinatura" placeholder="Pesquisar nome ou desenvolvedor" value={query} onChange={e=>setQuery(e.target.value)}/></label><p className="catalog-count">{visible.length} resultado(s) no catálogo. Compatibilidade não garante disponibilidade para sua conta.</p>
      <div className="account-catalog-list" role="region" aria-label="Modelos da assinatura" tabIndex={0}><table><thead><tr><th>Modelo</th><th>Recursos</th><th>Disponibilidade</th></tr></thead><tbody>{visible.map(entry=><tr key={entry.id} data-available={entry.selectable}><th scope="row">{entry.name||entry.id}<small>{entry.id}</small></th><td>{entry.chat?'Chat':'Outra modalidade'}{entry.analysis?' · Análise':''}</td><td>{entry.selectable?(allowed?'Compatível; sujeito à cota':'Requer assinatura'):entry.reason||'Fora do suporte atual'}</td></tr>)}</tbody></table></div></>}
    {pending&&<p role="status">Consultando o catálogo…</p>}{error&&<p role="alert">{error}</p>}{note&&<p className="catalog-capabilities">{note}</p>}
  </section>;
}
