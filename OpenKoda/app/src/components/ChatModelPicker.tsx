import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Grid2X2, Plus, Search, X } from 'lucide-react';
import { native, request } from '../api';
import { personalProviders, providers, type AiSelection, type ProviderId, type ProviderStatus } from '../ai-provider';
import { type CommercialModel, type Reasoning, accountChanged } from '../commercial';
import { errorMessage } from '../types';

type Group = { provider: ProviderId; models: string[] };
type Props = {
  value: AiSelection | null;
  onChange: (value: AiSelection) => void | Promise<void>;
  disabled?: boolean;
  forAnalysis?: boolean;
  onModels?: () => void;
};

export function ChatModelPicker({ value, onChange, disabled, onModels, forAnalysis = false }: Props) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const dialog = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [groups, setGroups] = useState<Group[]>([]);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<ProviderId>();
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [commercial,setCommercial]=useState<CommercialModel[]>([]);
  const [sessionRevision,setSessionRevision]=useState(0);
  useEffect(()=>{
    const reset=()=>{setCommercial([]);setSessionRevision(n=>n+1);};
    window.addEventListener(accountChanged,reset);return ()=>window.removeEventListener(accountChanged,reset);
  },[]);
  useEffect(()=>{
    if(!native||!open&&value?.provider!=='openkoda')return;
    let active=true;
    async function catalog(){
      try{
        const session=await request<{user:{id:string}|null}>('account.session');
        if(!session?.user){if(active)setCommercial([]);return;}
        const result=await request<{models:CommercialModel[]}>('account.models');
        if(active)setCommercial(Array.isArray(result.models)?result.models:[]);
      }catch{if(active)setCommercial([]);}
    }
    void catalog();return ()=>{active=false;};
  },[open,value?.provider,sessionRevision]);
  const [error, setError] = useState('');

  useLayoutEffect(() => {
    if (!open || !trigger.current || !dialog.current) return;
    const anchor = trigger.current;
    const panel = dialog.current;
    function position() {
      const rect = anchor.getBoundingClientRect();
      const margin = 8;
      const gap = 6;
      const above = rect.top - margin - gap;
      const below = innerHeight - rect.bottom - margin - gap;
      const upward = above > below;
      const room = Math.max(above, below);
      const detached = room < 176;
      const height = detached ? innerHeight - 2 * margin : room;
      panel.style.maxHeight = Math.min(480, height) + 'px';
      panel.style.left = Math.max(margin, Math.min(rect.left, innerWidth - panel.offsetWidth - margin)) + 'px';
      panel.style.top = (detached ? margin : upward ? rect.top - gap - panel.offsetHeight : rect.bottom + gap) + 'px';
    }
    position();
    search.current?.focus({ preventScroll: true });
    const observer = new ResizeObserver(position);
    observer.observe(panel);
    observer.observe(anchor);
    window.addEventListener('resize', position);
    // An ancestor may scroll in the project wizard while the control is open.
    window.addEventListener('scroll', position, true);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', position);
      window.removeEventListener('scroll', position, true);
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    let active = true;
    async function load() {
      const result: Group[] = [];
      let incomplete = false;
      // Native catalog requests share an inference lock across providers.
      for (const provider of personalProviders) {
        if (!active) return;
        try {
          const status = await request<ProviderStatus>('ai.status', { provider });
          if (!status.hasApiKey || status.provider !== provider || status.enabledModels?.length === 0) continue;
          let models = status.enabledModels;
          if (forAnalysis || !models) {
            const catalog = await request<{ models: string[] }>('ai.models', { provider, forAnalysis });
            const enabled = status.enabledModels && new Set(status.enabledModels);
            models = catalog.models.filter(model => !enabled || enabled.has(model));
          }
          if (models.length) result.push({ provider, models });
        } catch { incomplete = true; }
        if (active) setGroups([...result]);
      }
      if (!active) return;
      if (incomplete) setError('Alguns catálogos não puderam ser carregados.');
      setLoading(false);
    }
    void load();
    return () => { active = false; };
  }, [open, forAnalysis]);

  function close(restoreFocus: boolean) {
    const panel = dialog.current;
    if (!panel?.matches(':popover-open')) return;
    panel.hidePopover();
    if (restoreFocus) trigger.current?.focus({ preventScroll: true });
  }
  const term = query.toLocaleLowerCase().trim();
  const allGroups:Group[]=commercial.length?[{provider:'openkoda',models:commercial.map(m=>m.id)},...groups]:groups;
  const levels=value?.provider==='openkoda'?commercial.find(m=>m.id===value.model)?.reasoningLevels??[]:[];
  const levelLabel:Record<Reasoning,string>={low:'Leve',medium:'Médio',high:'Alto'};
  const visible = allGroups.filter(g => !filter || g.provider === filter).map(g => ({
    ...g, models: g.models.filter(m => (m + ' ' + providers[g.provider].name).toLocaleLowerCase().includes(term)),
  })).filter(g => g.models.length);

  async function choose(selection: AiSelection) {
    setSaving(true); setError('');
    try { await onChange(selection); close(true); }
    catch (e) { setError(errorMessage(e)); }
    finally { setSaving(false); }
  }
  return <>
    <button ref={trigger} type="button" popoverTarget={id} className="chat-model-trigger" disabled={disabled || !native}
      aria-haspopup="dialog" aria-controls={id} aria-expanded={open}
      title={value ? providers[value.provider].name + ' · ' + value.model : 'Escolher modelo'}
      aria-label={'Escolher modelo · ' + (value?.model || 'Nenhum selecionado')}>
      {value && <img src={providers[value.provider].logo} alt="" width="17" height="17"/>}
      <span>{value?.model || 'Escolher modelo'}</span><ChevronDown size={13}/>
    </button>
    <div id={id} popover="auto" role="dialog" ref={dialog} className="chat-model-dialog" aria-label="Escolher provedor e modelo"
      onToggle={e => {
        const opening=e.currentTarget.matches(':popover-open');
        if(opening){setGroups([]);setQuery('');setFilter(undefined);setError('');setLoading(true);}
        setOpen(opening);
      }}
      onBlur={e => {if(e.relatedTarget instanceof Element && !e.currentTarget.contains(e.relatedTarget) && e.relatedTarget!==trigger.current)close(false);}}
      onKeyDown={e => {
        if(e.key==='Escape'){e.preventDefault();close(true);return;}
        if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
        const rows = Array.from(list.current?.querySelectorAll<HTMLButtonElement>('button:enabled') ?? []);
        const current = rows.indexOf(e.target as HTMLButtonElement);
        if ((current < 0 && e.target !== search.current) || !rows.length) return;
        e.preventDefault();
        if (e.key === 'ArrowUp' && current <= 0) search.current?.focus();
        else rows[Math.min(rows.length - 1, current + (e.key === 'ArrowDown' ? 1 : -1))].focus();
      }}>
      <header><Search size={17}/><input ref={search} type="search" placeholder="Buscar modelos…" aria-label="Buscar modelos" value={query} onChange={e => setQuery(e.target.value)}/>
        <button type="button" aria-label="Fechar modelos" disabled={saving} onClick={() => close(true)}><X size={16}/></button>
      </header>
      <div className="chat-model-columns">
        <nav aria-label="Filtrar por provedor">
          <button type="button" title="Todos os provedores" aria-label="Todos" aria-pressed={!filter} onClick={() => setFilter(undefined)}><Grid2X2 size={17}/></button>
          {allGroups.map(g => <button key={g.provider} type="button" title={providers[g.provider].name} aria-label={providers[g.provider].name} aria-pressed={filter === g.provider} onClick={() => setFilter(g.provider)}>
            <img src={providers[g.provider].logo} alt="" width="19" height="19"/>
          </button>)}
        </nav>
        <div className="chat-model-list" ref={list}>
          {loading && <p role="status">Consultando provedores…</p>}
          {!loading && !visible.length && <p>{groups.length ? 'Nenhum modelo corresponde à busca.' : forAnalysis ? 'Ative um modelo de texto com tarifa disponível para análise.' : 'Ative um modelo em IA e modelos.'}</p>}
          {visible.map(g => <section key={g.provider}><h4>{providers[g.provider].name}</h4>
            {g.models.map(m => {
              const selected = value?.provider === g.provider && value.model === m;
              return <button key={m} type="button" disabled={saving} aria-pressed={selected} onClick={() => {
                const effort=g.provider==='openkoda'?commercial.find(row=>row.id===m)?.reasoningLevels[0]:undefined;
                void choose({provider:g.provider,model:m,...(effort?{reasoning:effort}:{})});
              }}>
                <span>{m}</span>{selected && <Check size={15}/>}
              </button>;
            })}
          </section>)}
        </div>
      </div>
      {error && <p role="alert">{error}</p>}
      {onModels && <footer><button type="button" disabled={saving} onClick={() => { close(false); onModels(); }}><Plus size={15}/>Configurar modelos</button></footer>}
    </div>
    {value?.provider==='openkoda'&&levels.length>0&&<label className="reasoning-control">Raciocínio · {levelLabel[value.reasoning??levels[0]]}
      <input aria-label="Nível de raciocínio" type="range" min={0} max={levels.length-1} step={1} disabled={disabled||saving||levels.length===1}
        value={Math.max(0,levels.indexOf(value.reasoning??levels[0]))} aria-valuetext={levelLabel[value.reasoning??levels[0]]}
        onChange={event=>void choose({...value,reasoning:levels[event.target.valueAsNumber]})}/></label>}
  </>;
}
