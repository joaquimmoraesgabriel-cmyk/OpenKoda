import { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Search, X } from 'lucide-react';
import { native, request } from '../api';
import { providers, type AiSelection, type ProviderId, type ProviderStatus } from '../ai-provider';
import { errorMessage } from '../types';

type Group = {provider:ProviderId; models:string[]};
export function ChatModelPicker({value,onChange,disabled,onModels}:{value:AiSelection|null;onChange:(value:AiSelection)=>void|Promise<void>;disabled?:boolean;onModels?:()=>void}) {
  const dialog=useRef<HTMLDialogElement>(null);
  const [groups,setGroups]=useState<Group[]>([]),[query,setQuery]=useState(''),[filter,setFilter]=useState<ProviderId>();
  const [open,setOpen]=useState(false),[loading,setLoading]=useState(false),[saving,setSaving]=useState(false),[error,setError]=useState('');
  useEffect(()=>{
    if(!open)return;
    const el=dialog.current; el?.showModal(); let active=true;
    setQuery('');setFilter(undefined);setError('');setGroups([]);setLoading(true);
    async function load(){
      const result=await Promise.allSettled((Object.keys(providers) as ProviderId[]).map(async provider=>{
        const status=await request<ProviderStatus>('ai.status',{provider});
        if(!status.hasApiKey||status.provider!==provider)return {provider,models:[]};
        const models=status.enabledModels??(await request<{models:string[]}>('ai.models',{provider})).models;
        return {provider,models};
      }));
      if(!active)return;
      setGroups(result.flatMap(r=>r.status==='fulfilled'&&r.value.models.length?[r.value]:[]));
      if(result.some(r=>r.status==='rejected'))setError('Alguns catálogos não puderam ser carregados.');
      setLoading(false);
    }
    void load();
    return ()=>{active=false;el?.close();};
  },[open]);
  const visible=groups.filter(g=>!filter||g.provider===filter).map(g=>({...g,models:g.models.filter(m=>(m+' '+providers[g.provider].name).toLocaleLowerCase().includes(query.toLocaleLowerCase().trim()))})).filter(g=>g.models.length);
  async function choose(selection:AiSelection){setSaving(true);setError('');try{await onChange(selection);setOpen(false);}catch(e){setError(errorMessage(e));}finally{setSaving(false);}}
  return <><button type="button" className="chat-model-trigger" disabled={disabled||!native} aria-haspopup="dialog" aria-label={'Escolher modelo · '+(value?.model||'Nenhum selecionado')} onClick={()=>setOpen(true)}>{value&&<img src={providers[value.provider].logo} alt="" width="18" height="18"/>}<span>{value?.model||'Escolher modelo'}</span><ChevronDown size={14}/></button>
    {open&&<dialog ref={dialog} className="chat-model-dialog" aria-label="Escolher provedor e modelo" onCancel={e=>{e.preventDefault();if(!saving)setOpen(false);}}>
      <header><Search size={16}/><input type="search" autoFocus placeholder="Buscar modelos…" aria-label="Buscar modelos" value={query} onChange={e=>setQuery(e.target.value)}/><button type="button" aria-label="Fechar modelos" disabled={saving} onClick={()=>setOpen(false)}><X size={16}/></button></header>
      <div className="chat-model-columns"><nav aria-label="Filtrar por provedor"><button type="button" aria-pressed={!filter} onClick={()=>setFilter(undefined)}>Todos</button>{groups.map(g=><button key={g.provider} type="button" title={providers[g.provider].name} aria-label={providers[g.provider].name} aria-pressed={filter===g.provider} onClick={()=>setFilter(g.provider)}><img src={providers[g.provider].logo} alt="" width="20" height="20"/></button>)}</nav><div className="chat-model-list">
        {loading&&<p role="status">Consultando provedores…</p>}{!loading&&!visible.length&&<p>{groups.length?'Nenhum modelo corresponde à busca.':'Ative um modelo em IA e modelos.'}</p>}
        {visible.map(g=><section key={g.provider}><h4>{providers[g.provider].name}</h4>{g.models.map(m=><button key={m} type="button" disabled={saving} aria-pressed={value?.provider===g.provider&&value.model===m} onClick={()=>void choose({provider:g.provider,model:m})}><img alt="" width="17" height="17" src={providers[g.provider].logo}/><span>{m}</span>{value?.provider===g.provider&&value.model===m&&<Check size={16}/>}</button>)}</section>)}</div></div>{error&&<p role="alert">{error}</p>}
      {onModels&&<footer><button type="button" disabled={saving} onClick={()=>{setOpen(false);onModels();}}>Configurar modelos</button></footer>}</dialog>}</>;
}

