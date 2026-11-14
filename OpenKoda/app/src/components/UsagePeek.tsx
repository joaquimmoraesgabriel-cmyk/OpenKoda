import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Gauge, X } from 'lucide-react';
import { native, request } from '../api';
import { knownProvider, personalProviders, type ProviderStatus } from '../ai-provider';
import type { BudgetStatus } from '../ai-budget';
import { accountChanged, getAccount, type AccountView } from '../commercial';
import { groupUsage, localDay, offsetDay, providerName, type UsageRow } from '../usage-history';
import { activityCells, recentModelCosts, usageLimit } from '../usage-peek';
import './usage-peek.css';

const dollars=(value:number|null|undefined)=>value==null?'Não informado':new Intl.NumberFormat('pt-BR',{style:'currency',currency:'USD',maximumFractionDigits:6}).format(value);
const count=(value:number)=>new Intl.NumberFormat('pt-BR').format(value);
export default function UsagePeek({onOpen,activeProvider='openai'}:{onOpen:()=>void;activeProvider?:string}){
  const [open,setOpen]=useState(false),[provider,setProvider]=useState(activeProvider),[effectiveProvider,setEffectiveProvider]=useState(activeProvider);
  const [budget,setBudget]=useState<BudgetStatus>(),[account,setAccount]=useState<AccountView|null>();
  const [rows,setRows]=useState<UsageRow[]>([]),[configured,setConfigured]=useState<string[]>([]);
  const [historyAvailable,setHistoryAvailable]=useState(false),[pending,setPending]=useState(false);
  const root=useRef<HTMLDivElement>(null),button=useRef<HTMLButtonElement>(null),panel=useRef<HTMLDivElement>(null);
  const revision=useRef(0),mounted=useRef(true),closing=useRef<ReturnType<typeof setTimeout>|undefined>(undefined);
  const dragging=useRef<{x:number;left:number;pointerId:number;active:boolean}|undefined>(undefined);
  const providersRef=useRef<HTMLDivElement>(null),[edges,setEdges]=useState({start:false,end:false});
  async function reload(){
    if(!native)return;
    const stamp=++revision.current;setPending(true);
    const result=await Promise.allSettled([
      request<BudgetStatus>('ai.budget.status'),request<{rows:UsageRow[]}>('ai.budget.history'),getAccount(),
    ]);
    if(!mounted.current||stamp!==revision.current)return;
    const [b,h,a]=result;
    setBudget(b.status==='fulfilled'&&typeof b.value?.enabled==='boolean'?b.value:undefined);
    const valid=h.status==='fulfilled'&&Array.isArray(h.value?.rows);
    setRows(valid?h.value.rows:[]);setHistoryAvailable(valid);
    setAccount(a.status==='fulfilled'?a.value.account:undefined);setPending(false);
  }
  useEffect(()=>{
    mounted.current=true;void reload();
    const changed=()=>void reload();
    const identityChanged=()=>{setAccount(undefined);void reload();};
    window.addEventListener('openkoda-budget-changed',changed);
    window.addEventListener(accountChanged,identityChanged);
    return()=>{mounted.current=false;revision.current++;clearTimeout(closing.current);
      window.removeEventListener('openkoda-budget-changed',changed);window.removeEventListener(accountChanged,identityChanged);};
  },[]);
  useEffect(()=>{setProvider(activeProvider);setEffectiveProvider(activeProvider);},[activeProvider]);
  useEffect(()=>{const changed=(e:Event)=>{const next=(e as CustomEvent<string>).detail;if(typeof next==='string'){setEffectiveProvider(next);setProvider(next);}};
    window.addEventListener('openkoda:active-provider',changed);return()=>window.removeEventListener('openkoda:active-provider',changed);},[]);
  useEffect(()=>{
    if(!open)return;
    void reload();let current=true;
    if(native)void Promise.allSettled(personalProviders.map(id=>request<ProviderStatus>('ai.status',{provider:id}).then(s=>s?.hasApiKey?id:null)))
      .then(results=>{if(current)setConfigured(results.flatMap(r=>r.status==='fulfilled'&&r.value?[r.value]:[]));});
    const outside=(event:PointerEvent)=>{if(!root.current?.contains(event.target as Node))setOpen(false);};
    document.addEventListener('pointerdown',outside);
    return()=>{current=false;document.removeEventListener('pointerdown',outside);};
  },[open]);
  useEffect(()=>{
    const end=account?.subscription?.periodEnd;if(!end)return;
    const remaining=Date.parse(end)-Date.now();if(!Number.isFinite(remaining)||remaining<=0)return;
    const timer=setTimeout(()=>{setAccount(undefined);void reload();},Math.min(remaining+1,2147483647));
    return()=>clearTimeout(timer);
  },[account?.subscription?.periodEnd]);
  const activeLimit=usageLimit(effectiveProvider,budget,account),limit=usageLimit(provider,budget,account);
  useEffect(()=>{
    document.documentElement.dataset.usageLow=String(activeLimit.low);
    return()=>{delete document.documentElement.dataset.usageLow;};
  },[activeLimit.low]);
  const choices=[...new Set([effectiveProvider,...configured,...rows.map(r=>r.provider).filter((id):id is string=>!!id),...(account?.plan?['openkoda']:[])])];
  const selected=choices.includes(provider)?provider:effectiveProvider;
  const updateEdges=()=>{const el=providersRef.current;if(el)setEdges({start:el.scrollLeft>2,end:el.scrollLeft+el.clientWidth<el.scrollWidth-2});};
  useLayoutEffect(()=>{
    if(!open||!panel.current||!button.current)return;
    const target=panel.current,anchor=button.current;
    const position=()=>{const rect=anchor.getBoundingClientRect();
      const scale=Number.parseFloat(getComputedStyle(document.documentElement).zoom)||1;
      target.style.maxWidth=Math.max(180,(window.innerWidth-rect.right-18)/scale)+'px';
      const width=target.getBoundingClientRect().width,height=target.getBoundingClientRect().height;
      target.style.left=Math.max(8,Math.min(rect.right+10,window.innerWidth-width-8))/scale+'px';
      target.style.top=Math.max(8,Math.min(rect.bottom-height,window.innerHeight-height-8))/scale+'px';
      target.style.maxHeight=(window.innerHeight-16)/scale+'px';updateEdges();};
    position();const observer=new ResizeObserver(position);observer.observe(target);
    window.addEventListener('resize',position);return()=>{observer.disconnect();window.removeEventListener('resize',position);};
  },[open]);
  const relevant=rows.filter(r=>r.provider===selected&&r.day>=offsetDay(localDay(),-20)&&r.day<=localDay());
  const groups=groupUsage(relevant,r=>r.model),costs=recentModelCosts(budget,selected);
  const models=[...new Set([...groups.keys(),...costs.keys()])];
  function reveal(){clearTimeout(closing.current);setOpen(true);}
  function closeSoon(){clearTimeout(closing.current);closing.current=setTimeout(()=>{if(!root.current?.contains(document.activeElement))setOpen(false);},180);}
  function history(){setOpen(false);onOpen();}
  return <div className="usage-peek" ref={root} onMouseEnter={reveal} onMouseLeave={closeSoon}
    onFocus={e=>{if(!e.currentTarget.contains(e.relatedTarget))reveal();}} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))setOpen(false);}}
    onKeyDown={e=>{if(e.key==='Escape'&&open){e.stopPropagation();setOpen(false);button.current?.focus();}}}>
    <button type="button" ref={button} className="rail-button usage-ring" data-low={activeLimit.low} data-known={activeLimit.ratio!==null}
      aria-label="Abrir histórico de uso" aria-describedby={open?'usage-peek-title':undefined} aria-expanded={open} aria-controls="usage-peek-panel" onClick={history}>
      <svg className="usage-ring-track" viewBox="0 0 40 40" aria-hidden="true"><circle className="ring-track" cx="20" cy="20" r="17"/>
        {activeLimit.ratio!==null&&<circle className="ring-value" cx="20" cy="20" r="17" pathLength="100" strokeDasharray={`${100*activeLimit.ratio} 100`}/>}</svg>
      <Gauge size={17} aria-hidden="true"/>
    </button>
    {open&&<div id="usage-peek-panel" className="usage-peek-panel" role="region" aria-labelledby="usage-peek-title" ref={panel}>
      <div className="usage-providers-wrap" data-start={edges.start} data-end={edges.end}>
        <div className="usage-providers" role="group" aria-label="Provedores do resumo de uso" ref={providersRef} onScroll={updateEdges}
          onPointerDown={e=>{if(e.pointerType==='mouse'&&e.button===0)dragging.current={x:e.clientX,left:e.currentTarget.scrollLeft,pointerId:e.pointerId,active:false};}}
          onPointerMove={e=>{const d=dragging.current;if(!d||e.pointerId!==d.pointerId)return;const dx=e.clientX-d.x;if(Math.abs(dx)>5)d.active=true;
            if(d.active){e.currentTarget.setPointerCapture(e.pointerId);e.currentTarget.scrollLeft=d.left-dx;e.preventDefault();}}}
          onPointerUp={e=>{const d=dragging.current;if(d?.active){e.currentTarget.releasePointerCapture(e.pointerId);e.preventDefault();}setTimeout(()=>{dragging.current=undefined;},0);}}
          onPointerCancel={()=>{dragging.current=undefined;}} onClickCapture={e=>{if(dragging.current?.active){e.preventDefault();e.stopPropagation();}}}>
          {choices.map(id=><button type="button" key={id} aria-pressed={selected===id} onClick={()=>setProvider(id)}>{knownProvider(id)?providerName(id):'Outro provedor'}</button>)}
        </div>
      </div>
      <header><div><h2 id="usage-peek-title">Seu uso de IA</h2><p>{limit.caption}</p></div><button type="button" className="usage-close" aria-label="Fechar resumo de uso" onClick={()=>{setOpen(false);button.current?.focus();}}><X size={16}/></button></header>
      {pending&&<span className="usage-refreshing" role="status">Atualizando…</span>}
      {limit.ratio!==null&&<strong className="usage-remaining" data-low={limit.low}>{Math.round(limit.ratio*100)}% disponível</strong>}
      <dl className="usage-peek-metrics"><div><dt>{limit.kind==='subscription'?'Comprometido na janela':'Uso com preço registrado'}</dt><dd>{dollars(limit.used)}</dd></div>
        <div><dt>{limit.kind==='subscription'?'Cota da janela':'Limite local'}</dt><dd>{dollars(limit.limit)}</dd></div>
        <div><dt>Disponível</dt><dd>{dollars(limit.remaining)}</dd></div>
        <div><dt>{limit.kind==='subscription'?'Disponível no ciclo':'Reservado'}</dt><dd>{dollars(limit.kind==='subscription'?limit.cycleRemaining:limit.held)}</dd></div></dl>
      {limit.resetAt&&<p className="usage-caption">Próxima janela em {new Date(limit.resetAt).toLocaleString('pt-BR')}.</p>}
      {limit.kind==='personal'&&<p className="usage-caption">O orçamento é compartilhado pelas chaves deste dispositivo. O saldo da conta no provedor não é consultado.</p>}
      {limit.kind==='personal'&&(budget?.unknownChargesTotal??0)>0&&<p className="usage-caption">Há {budget?.unknownChargesTotal} cobranças sem valor confirmado.</p>}
      <div className="usage-activity-heading"><strong>Atividade neste dispositivo</strong><small>21 dias</small></div>
      {historyAvailable?<div className="usage-mini-calendar" aria-label="Chamadas nos últimos 21 dias">{activityCells(rows,selected).map(cell=>{
        const label=new Date(cell.day+'T12:00:00').toLocaleDateString('pt-BR')+' · '+(cell.totals?count(cell.totals.requests)+' chamadas':'Sem registro');
        return <span key={cell.day} data-level={cell.level} title={label} role="img" aria-label={label}/>;
      })}</div>:<p className="usage-caption">Histórico local indisponível.</p>}
      {!!models.length&&<div className="usage-model-scroll"><table><caption>Modelos · atividade de 21 dias e custos recentes</caption><thead><tr><th>Modelo</th><th>Chamadas</th><th>Custo recente</th></tr></thead><tbody>{models.map(model=><tr key={model}><th scope="row">{model}</th><td>{count(groups.get(model)?.requests??0)}</td><td>{costs.has(model)?dollars(costs.get(model)?.unknown&&!costs.get(model)?.cost?null:costs.get(model)?.cost):'Não informado'}{(costs.get(model)?.unknown??0)>0&&<small>Há registros sem preço</small>}</td></tr>)}</tbody></table></div>}
      {!models.length&&historyAvailable&&<p className="usage-caption">Ainda não há chamadas registradas para este provedor.</p>}
      <button type="button" className="usage-history-link" onClick={history}>Ver histórico completo</button>
    </div>}
  </div>;
}
