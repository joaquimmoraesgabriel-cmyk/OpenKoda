import { useEffect, useMemo, useRef, useState } from 'react';
import { ArrowUpRight, CalendarDays, RefreshCw } from 'lucide-react';
import { native, request } from '../api';
import { errorMessage, type Assessment } from '../types';
import { emptyTotals, groupUsage, localDay, modelKey, offsetDay, periodRows, providerName, streaks, type UsageRow, type UsageTotals } from '../usage-history';
import './usage-history.css';

const number = new Intl.NumberFormat('pt-BR');
const dateLabel = (day: string) => new Date(day + 'T12:00:00').toLocaleDateString('pt-BR', {day:'numeric',month:'short',year:'numeric'});
const metric = (group?: UsageTotals) => group?.measured ? number.format(group.volume) : '—';
const modelLabel = (row: UsageRow) => providerName(row.provider) + ' · ' + row.model;
const byVolume = ([,a]: [string, UsageTotals], [,b]: [string, UsageTotals]) => b.volume-a.volume || b.requests-a.requests;
const presets = [['all','Todo o histórico'],['30d','Últimos 30 dias'],['7d','Últimos 7 dias'],['today','Hoje']];

function ActivityCalendar({days,today,selected,onSelect}: {
  days: Map<string,UsageTotals>; today: string; selected: string; onSelect:(day:string)=>void;
}) {
  const weekDay = (new Date(today+'T12:00:00').getDay()+6)%7;
  const grid = useRef<HTMLDivElement>(null);
  const [fitWeeks,setFitWeeks] = useState(20);
  useEffect(()=>{
    if(!grid.current)return;
    const observer=new ResizeObserver(entries=>setFitWeeks(Math.max(20,Math.floor((entries[0].contentRect.width+3)/15))));
    observer.observe(grid.current);
    return ()=>observer.disconnect();
  },[]);
  const first=Array.from(days.keys()).sort()[0]??today;
  const historyWeeks=1+Math.ceil((Date.parse(today)-Date.parse(first))/604800000);
  const weeks=Math.min(fitWeeks,Math.max(20,historyWeeks));
  const start = offsetDay(today,-((weeks-1)*7 + weekDay));
  const sequence = Array.from({length:weeks*7}, (_,i) => offsetDay(start,i));
  const peak = Math.max(1,...sequence.map(day=>days.get(day)?.volume ?? 0));
  const [focusDay,setFocusDay] = useState(today);
  return <section className="uh-card" aria-label="Calendário de atividade">
    <div className="uh-card-heading"><h3>Calendário de atividade</h3><span>{weeks} semanas</span></div>
    <div className="uh-months" aria-hidden="true" style={{gridTemplateColumns:'repeat('+weeks+',minmax(0,12px))'}}>
      {sequence.filter((_,i)=>i%7===0).map((day,i)=>{
        const show=i===0||day.slice(5,7)!==sequence[(i-1)*7].slice(5,7);
        return <span key={day}>{show?new Date(day+'T12:00:00').toLocaleDateString('pt-BR',{month:'short'}):''}</span>;
      })}
    </div>
    <div className="uh-calendar" ref={grid}>
      {sequence.map((day,index)=>{
        if(day>today)return <span className="uh-future" key={day}/>;
        const entry=days.get(day);
        const level=entry?.volume ? Math.max(1,Math.ceil(Math.sqrt(entry.volume/peak)*4)) : 0;
        const label=dateLabel(day)+' · '+(entry ? metric(entry)+' tokens informados' : 'Sem registro');
        return <button key={day} type="button" data-day={day} data-level={level} tabIndex={(focusDay>=start?focusDay:today)===day?0:-1} title={label}
          aria-label={label} aria-pressed={selected===day} onFocus={()=>setFocusDay(day)} onClick={()=>onSelect(day)}
          onKeyDown={event=>{
            const delta:Record<string,number>={ArrowUp:-1,ArrowDown:1,ArrowLeft:-7,ArrowRight:7};
            if(!(event.key in delta))return;
            event.preventDefault();
            const next=sequence[index+delta[event.key]];
            if(next && next<=today)grid.current?.querySelector<HTMLButtonElement>('[data-day="'+next+'"]')?.focus();
          }}/>;
      })}
    </div>
    <div className="uh-axis"><span>{dateLabel(start)}</span><span>{dateLabel(today)}</span></div>
  </section>;
}

function Ranking({title,label,groups,names,colors,onOpen}:{
  title:string;label:string;groups:Map<string,UsageTotals>;names:Map<string,string>;colors?:Map<string,number>;onOpen?:(id:string)=>void;
}) {
  const [showAll,setShowAll]=useState(false);
  const entries=[...groups].sort(byVolume);
  return <section className="uh-card"><div className="uh-card-heading"><h3>{title}</h3><span>{entries.length}</span></div>
    {!entries.length ? <p className="uh-muted">Sem registros neste período.</p> : <>
      <div className="uh-table-wrap"><table><thead><tr><th scope="col">{label}</th><th scope="col">Chamadas</th><th scope="col">Tokens</th></tr></thead>
        <tbody>{(showAll?entries:entries.slice(0,6)).map(([id,totals])=><tr key={id}><td>
          {colors && <i aria-hidden="true" className="uh-dot" data-color={colors.get(id)}/>}
          {onOpen ? <button type="button" className="text-button" onClick={()=>onOpen(id)}>{names.get(id)} <ArrowUpRight size={12}/></button> : names.get(id) ?? id}
        </td><td>{number.format(totals.requests)}</td><td>{metric(totals)}</td></tr>)}</tbody>
      </table></div>{entries.length>6 && <button type="button" className="text-button uh-expand" aria-expanded={showAll} onClick={()=>setShowAll(!showAll)}>{showAll?'Mostrar menos':'Mostrar todos ('+entries.length+')'}</button>}
    </>}
  </section>;
}

export default function UsageHistory({assessments=[],onOpenAssessment}: {assessments?:Assessment[];onOpenAssessment?:(id:string)=>void}) {
  const [rows,setRows]=useState<UsageRow[]>();
  const [error,setError]=useState('');
  const [refresh,setRefresh]=useState(0);
  const [pending,setPending]=useState(false);
  const [range,setRange]=useState('all');
  const [model,setModel]=useState('');
  const today=localDay();
  useEffect(()=>{
    if(!native)return;
    let current=true;
    setPending(true);setError('');
    void request<{rows:UsageRow[]}>('ai.budget.history').then(result=>{
      if(!Array.isArray(result?.rows))throw new Error('Histórico de tokens indisponível.');
      if(current)setRows(result.rows);
    }).catch(cause=>{if(current)setError(errorMessage(cause));}).finally(()=>{if(current)setPending(false);});
    return ()=>{current=false;};
  },[refresh]);
  const models=useMemo(()=>new Map((rows??[]).map(row=>[modelKey(row),modelLabel(row)])),[rows]);
  const allGroups=useMemo(()=>groupUsage(rows??[],modelKey),[rows]);
  const colors=useMemo(()=>new Map([...allGroups].sort(byVolume).map(([id],i)=>[id,i%6])),[allGroups]);
  const selectedModel=models.has(model)?model:'';
  const history=useMemo(()=>(rows??[]).filter(row=>(!selectedModel || modelKey(row)===selectedModel) && row.day<=today),[rows,selectedModel,today]);
  const daily=useMemo(()=>groupUsage(history,row=>row.day),[history]);
  const filtered=useMemo(()=>periodRows(history,range,today),[history,range,today]);
  const summary=groupUsage(filtered,()=>'total').get('total') ?? emptyTotals();
  const modelTotals=groupUsage(filtered,modelKey);
  const providerTotals=groupUsage(filtered,row=>providerName(row.provider));
  const names=new Map(assessments.map(a=>[a.id,a.title]));
  const tasks=groupUsage(filtered.filter(row=>row.assessmentId && names.has(row.assessmentId)),row=>row.assessmentId!);
  const streak=streaks([...daily].filter(([,v])=>v.volume>0).map(([day])=>day),today);
  const chartDays=Array.from({length:30},(_,i)=>offsetDay(today,i-29));
  const chartGroups=useMemo(()=>groupUsage(periodRows(history,'30d',today),row=>row.day+'/'+modelKey(row)),[history,today]);
  const chartModels=[...new Set(periodRows(history,'30d',today).map(modelKey))].sort((a,b)=>(colors.get(a)??0)-(colors.get(b)??0));
  const peak=Math.max(1,...chartDays.map(day=>daily.get(day)?.volume??0));
  const chosenDate=presets.some(([id])=>id===range)?'':range;
  return <div className="usage-history-page">
    <p className="uh-intro">Tokens informados nas chamadas de IA do perfil ativo neste dispositivo.</p>
    {!native ? <p className="uh-card">Abra o aplicativo desktop para consultar o histórico local.</p> : <>
      <div className="uh-filters">
        <label>Período<select value={range} onChange={event=>setRange(event.target.value)}>
          {presets.map(([id,name])=><option key={id} value={id}>{name}</option>)}{chosenDate && <option value={range}>{dateLabel(range)}</option>}
        </select></label>
        <label>Escolher dia<input type="date" value={chosenDate} max={today} onChange={event=>{const value=event.target.value;if(value<=today)setRange(value||'all');}}/></label>
        <label>Modelo<select value={selectedModel} onChange={event=>setModel(event.target.value)}><option value="">Todos os modelos</option>{[...models].sort((a,b)=>a[1].localeCompare(b[1])).map(([id,name])=><option key={id} value={id}>{name}</option>)}</select></label>
        <button type="button" className="button" disabled={pending} onClick={()=>setRefresh(n=>n+1)}><RefreshCw size={14}/>Atualizar histórico</button>
      </div>
      {error && <p role="alert" className="notice warning">{error}</p>}
      {pending && <p role="status" className="uh-muted">Consultando o histórico…</p>}
      {rows && <><dl className="uh-stats">
        <div><dt>Tokens no período</dt><dd>{metric(summary)}</dd></div>
        {!chosenDate && range!=='today' && <div><dt>Tokens hoje</dt><dd>{metric(daily.get(today))}</dd></div>}
        <div><dt>Chamadas no período</dt><dd>{number.format(summary.requests)}</dd></div>
        <div><dt>Modelos no período</dt><dd>{modelTotals.size}</dd></div>
        <div><dt>Sequência atual</dt><dd>{streak.current} <small>{streak.current===1?'dia':'dias'}</small></dd><span>Recorde de {streak.longest}</span></div>
      </dl>
      {summary.partial>0 && <p className="uh-muted">{summary.partial===1?'1 chamada com contagem de tokens incompleta':number.format(summary.partial)+' chamadas com contagens de tokens incompletas'} no período.</p>}
      <details className="uh-counting"><summary>Como os tokens são contados</summary><p>Os totais somam os valores de entrada e saída informados. O traço indica ausência de medição. O histórico conserva respostas com contagem parcial. A taxa de acerto de cache está indisponível neste registro.</p></details>
      {!rows.length && <p className="uh-card uh-empty"><CalendarDays size={20}/>O histórico começa quando uma chamada de IA é registrada.</p>}
      {!!rows.length && <><ActivityCalendar today={today} days={daily} selected={chosenDate || (range==='today'?today:'')} onSelect={setRange}/>
        <section className="uh-card"><div className="uh-card-heading"><h3>Tokens por dia</h3><span>Últimos 30 dias</span></div>
          <div className="uh-bars" role="group" aria-label="Selecionar dia pelo gráfico de tokens">
            {chartDays.map(day=><button type="button" key={day} aria-pressed={range===day} onClick={()=>setRange(day)} title={dateLabel(day)+' · '+metric(daily.get(day))+' tokens'} aria-label={dateLabel(day)+' · '+metric(daily.get(day))+' tokens'}>
              {chartModels.map(id=>{const volume=chartGroups.get(day+'/'+id)?.volume;return volume ? <i key={id} data-color={colors.get(id)} style={{height:(100*volume/peak)+'%'}}/> : null;})}
            </button>)}
          </div><div className="uh-axis"><span>{dateLabel(chartDays[0])}</span><span>{dateLabel(today)}</span></div>
          {chartModels.length>0 && <ul className="uh-legend" aria-label="Modelos no gráfico">{chartModels.map(id=><li key={id}><i className="uh-dot" aria-hidden="true" data-color={colors.get(id)}/>{models.get(id)}</li>)}</ul>}
        </section>
        <div className="uh-rankings"><Ranking title="Uso por modelo" label="Modelo" groups={modelTotals} names={models} colors={colors}/>
        <Ranking title="Uso por provedor" label="Provedor" groups={providerTotals} names={new Map()}/></div>
        {tasks.size>0 && <Ranking title="Avaliações com mais tokens" label="Avaliação" groups={tasks} names={names} onOpen={onOpenAssessment}/>}
      </>}</>}
    </>}
  </div>;
}
