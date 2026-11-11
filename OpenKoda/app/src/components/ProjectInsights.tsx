import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { groupUsage, modelKey, providerName, type UsageRow } from '../usage-history';
import { errorMessage, findingLabels, severityLabels, type Snapshot } from '../types';

export default function ProjectInsights({snapshot,projectId,onFinding}:{snapshot:Snapshot;projectId:string;onFinding:(id:string)=>void}) {
  const ids=new Set(snapshot.assessments.filter(a=>a.projectId===projectId).map(a=>a.id));
  const findings=snapshot.findings.filter(f=>ids.has(f.assessmentId));
  const [usage,setUsage]=useState<UsageRow[]>();
  const [error,setError]=useState('');
  useEffect(()=>{
    if(!native)return;
    let active=true;
    void request<{rows:UsageRow[]}>('ai.budget.history').then(r=>{if(active){setUsage(r.rows);setError('');}}).catch(e=>{if(active)setError(errorMessage(e));});
    return ()=>{active=false;};
  },[snapshot]);
  const rows=(usage??[]).filter(r=>r.assessmentId && ids.has(r.assessmentId));
  const totals=groupUsage(rows,modelKey);
  return <>
    <section className="content-section"><h2>Uso de IA</h2>
      {error?<p role="alert">{error}</p>:totals.size?<table className="project-stats"><thead><tr><th>Provedor e modelo</th><th>Chamadas</th><th>Tokens medidos</th></tr></thead><tbody>{[...totals].map(([key,total])=>{
        const row=rows.find(r=>modelKey(r)===key)!;
        return <tr key={key}><th scope="row">{providerName(row.provider)} · {row.model}</th><td>{total.requests}</td><td>{total.measured?total.volume.toLocaleString('pt-BR'):'—'}{total.partial?' (parcial)':''}</td></tr>;
      })}</tbody></table>:<p>{native && !usage?'Consultando o histórico…':'Nenhuma chamada registrada neste projeto.'}</p>}
    </section>
    <section className="content-section"><h2>Achados do projeto</h2>{findings.length?<table className="project-stats"><thead><tr><th>Achado</th><th>Severidade</th><th>Estado</th></tr></thead><tbody>{findings.map(f=><tr key={f.id}><th scope="row"><button className="text-button" onClick={()=>onFinding(f.id)}>{f.title}</button></th><td>{severityLabels[f.severity]}</td><td>{findingLabels[f.status]}</td></tr>)}</tbody></table>:<p>Nenhum achado registrado neste projeto.</p>}</section>
  </>;
}
