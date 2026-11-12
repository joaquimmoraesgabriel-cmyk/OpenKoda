import { useEffect, useRef, useState } from 'react';
import { LoaderCircle, ScanLine } from 'lucide-react';
import { native, request } from '../api';
import { providers } from '../ai-provider';
import { analysisDisplayText, projectScanReceipts, readScan, scanActive, scanStateLabel, SensitiveScanConfirmation, startSecurityScan, supportsSecurityModel, type ScanResult, type SecurityScan, type SecurityStatus } from '../security-scan';
import { effectiveScanLimit, loadProjectOptions } from '../project-analysis-options';
import { errorMessage, shortDate, type Assessment } from '../types';
import { ConversationText } from './ConversationText';
import './SecurityScan.css';

export function useSecurityScan(assessment:Assessment) {
  const [scan,setScan]=useState<SecurityScan|null>(null);
  const [recordKnown,setRecordKnown]=useState(false);
  const [status,setStatus]=useState<SecurityStatus>();
  const [error,setError]=useState('');
  const [busy,setBusy]=useState(false);
  const [revision,reload]=useState(0);
  const working=useRef(false);
  const key=useRef<string|undefined>(undefined);
  const mounted=useRef(true);
  const generation=useRef(0);
  const lastNotification=useRef('');
  const assessmentRef=useRef(assessment);
  assessmentRef.current=assessment;
  const id=assessment.id;
  const local=assessment.kind==='code'||assessment.kind==='files';
  const selection=assessment.aiSelection;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{
    if(!native||!local)return;
    let disposed=false, timer:number|undefined;
    const version=++generation.current;
    const current=()=>!disposed && version===generation.current;
    async function read(){
      try{
        const response=await request('security.scan.get',{assessmentId:id});
        if(!current())return;
        const record=readScan(response,id);
        setScan(record);setRecordKnown(true);setError('');
        if(record){
          const receipt=projectScanReceipts([assessmentRef.current]).find(item=>item.id===record.id);
          const stamp=JSON.stringify([record.id,record.status,record.updatedAt]);
          if(stamp!==lastNotification.current && (receipt?.status!==record.status || receipt.updatedAt!==record.updatedAt)){
            lastNotification.current=stamp;
            window.dispatchEvent(new Event('openkoda:security-scan-updated'));
          }
        }
        if(record&&scanActive(record.status))timer=window.setTimeout(()=>void read(),1000);
      }catch(cause){if(current()){setRecordKnown(false);setError(errorMessage(cause));}}
    }
    // Query the record independently: changing model or losing a key must preserve a past report.
    void read();
    void request<SecurityStatus>('security.status',{assessmentId:id}).then(value=>{
      if(current())setStatus(typeof value?.available==='boolean'?value:undefined);
    }).catch(()=>{if(current())setStatus(undefined);});
    return()=>{disposed=true;window.clearTimeout(timer);};
  },[id,local,selection?.model,selection?.provider,revision]);
  async function mutate(operation:()=>Promise<SecurityScan|null>){
    if(working.current)return;
    working.current=true;generation.current++;setBusy(true);setError('');
    try{
      const updated=await operation();
      if(!updated)throw new Error('Não foi possível confirmar o registro desta análise.');
      if(mounted.current){setScan(updated);reload(n=>n+1);}
    }catch(cause){if(mounted.current){
      if(cause instanceof SensitiveScanConfirmation)setStatus(value=>value&&({...value,requiresSensitiveConfirmation:true}));
      else setRecordKnown(false);
      setError(errorMessage(cause));
    }}
    finally{working.current=false;if(mounted.current)setBusy(false);}
  }
  const start=(maxCostUsd:number|undefined,acknowledgeSensitive:boolean)=>mutate(async()=>{
    if(!recordKnown||scan)throw new Error('Consulte o registro da análise antes de iniciar.');
    if(!selection||!supportsSecurityModel(selection))throw new Error('Escolha um modelo OpenAI ou OpenRouter para analisar o projeto.');
    key.current??=crypto.randomUUID();
    const limit=maxCostUsd??effectiveScanLimit(await loadProjectOptions(assessment.projectId));
    return startSecurityScan({projectId:assessment.projectId,assessmentId:id,selection,idempotencyKey:key.current,prompt:assessment.objective,maxCostUsd:limit,acknowledgeSensitive});
  });
  const cancel=()=>mutate(async()=>{
    const updated=readScan(await request('security.scan.cancel',{assessmentId:id}),id);
    if(!updated||updated.id!==scan?.id)throw new Error('O motor não confirmou o cancelamento desta análise.');
    return updated;
  });
  return {scan,status,busy,error,recordKnown,start,cancel,refresh:()=>reload(n=>n+1)};
}

const currency=new Intl.NumberFormat('pt-BR',{style:'currency',currency:'USD',maximumFractionDigits:4});
const severityLabels:Record<string,string>={critical:'Crítica',high:'Alta',medium:'Média',moderate:'Moderada',low:'Baixa',info:'Informativa',informational:'Informativa'};

function Report({result}:{result:ScanResult}){
  const {coverage,findings}=result;
  return <div className="security-scan-result">
    <p className="security-coverage-note">A conclusão vale para os arquivos efetivamente revisados. Consulte a cobertura e os limites do relatório.</p>
    {coverage.summary&&<ConversationText text={analysisDisplayText(coverage.summary)}/>}
    <section aria-label="Achados do motor"><h4>Achados <span>{coverage.findingsTotal}</span></h4>
      {findings.length?<ol className="security-findings">{findings.map((f,i)=><li key={i}><details><summary><strong>{f.title}</strong><small>{severityLabels[f.severity.toLowerCase()]??f.severity}</small></summary>
        {f.location&&<code>{f.location}</code>}{f.description&&<ConversationText text={f.description}/>}</details></li>)}</ol>
        :<p>{coverage.findingsTotal?'Os detalhes dos achados não vieram neste resultado.':'O motor não registrou achados no escopo analisado.'}</p>}
      {findings.length>0&&findings.length<coverage.findingsTotal&&<p>Exibindo {findings.length} de {coverage.findingsTotal} achados.</p>}
    </section>
    <details><summary>Cobertura da análise</summary>
      {!!coverage.limitations?.length&&<><h4>Limites informados pelo motor</h4><ul>{coverage.limitations.map((text,i)=><li key={i}>{analysisDisplayText(text)}</li>)}</ul></>}
      <p>Escopo informado pelo motor</p>{coverage.scope.length?<ul>{coverage.scope.map((p,i)=><li key={i}><code>{p}</code></li>)}</ul>:<p>Sem caminhos detalhados.</p>}
      <p>Caminhos excluídos</p>{coverage.excludedPaths.length?<ul>{coverage.excludedPaths.map((p,i)=><li key={i}><code>{p}</code></li>)}</ul>:<p>Sem caminhos detalhados.</p>}
      {coverage.additionalExclusions>0&&<p>Outras exclusões registradas: {coverage.additionalExclusions}.</p>}
    </details>
    {result.report&&<details><summary>Relatório da análise</summary>
      {coverage.reportTruncated&&<p>O relatório armazenado é parcial.</p>}<ConversationText text={analysisDisplayText(result.report)}/></details>}
    {result.cost?.estimatedUsd!=null&&<small>Custo estimado pelo motor · {currency.format(result.cost.estimatedUsd)}</small>}
  </div>;
}

export default function SecurityScanPanel({assessment,controller}:{assessment:Assessment;controller:ReturnType<typeof useSecurityScan>}){
  const [sensitiveConsent,setSensitiveConsent]=useState('');
  useEffect(()=>{
    const changed=(event:Event)=>{if((event as CustomEvent<string>).detail===assessment.projectId)setSensitiveConsent('');};
    window.addEventListener('openkoda:project-options-updated',changed);
    return()=>window.removeEventListener('openkoda:project-options-updated',changed);
  },[assessment.projectId]);
  const {scan,status,busy,error}=controller;
  const selection=assessment.aiSelection;
  const review=JSON.stringify([assessment.id,assessment.target,assessment.objective,selection]);
  const acknowledged=sensitiveConsent===review;
  if(!native||!['code','files'].includes(assessment.kind))return null;
  if(!scan && (!status||!selection||!supportsSecurityModel(selection)))return null;
  if(!scan)return <section className="security-start" aria-label="Análise de segurança">
    {status?.available&&selection?<form onSubmit={event=>{event.preventDefault();void controller.start(undefined,acknowledged);}}>
      <small>A análise usa {providers[selection.provider].name} com {selection.model}. Arquivos e contexto poderão ser enviados ao provedor.</small>
      {status.requiresSensitiveConfirmation&&<label><input type="checkbox" checked={acknowledged} onChange={e=>setSensitiveConsent(e.target.checked?review:'')}/>Autorizo enviar a fonte sensível desta avaliação a {providers[selection.provider].name} para esta análise.</label>}
      <button className="button" disabled={busy||!controller.recordKnown||!!status.requiresSensitiveConfirmation&&!acknowledged}>Iniciar análise de segurança</button>
    </form>:<p>{analysisDisplayText(status?.reason||'A análise de segurança está indisponível.')}</p>}
    {error&&<p role="alert">{analysisDisplayText(error)} <button className="text-button" onClick={controller.refresh}>Consultar estado novamente</button></p>}
  </section>;
  const updates=scan.events.filter(event=>event.stage==='message');
  const activity=scan.events.filter(event=>event.stage!=='message');
  return <section className="security-scan" aria-label="Análise de segurança">
    <header><ScanLine size={18} aria-hidden="true"/><h3>Análise do projeto</h3><span role="status">{scanActive(scan.status)&&<LoaderCircle size={13} aria-hidden="true"/>}{scanStateLabel[scan.status]}</span></header>
    <small>{providers[scan.provider as keyof typeof providers]?.name??scan.provider} · {scan.model}</small>
    {scan.message&&scan.stage!=='message'&&<p>{analysisDisplayText(scan.message)}</p>}
    {updates.length>0&&<div className="security-public-updates" role="log" aria-label="Atualizações do OpenKoda" aria-live="polite" aria-relevant="additions text">
      {updates.map((event,index)=><article key={event.at+':'+index}><header><strong>OpenKoda</strong><time dateTime={event.at}>{shortDate(event.at)}</time></header><ConversationText text={analysisDisplayText(event.message)}/></article>)}
    </div>}
    {activity.length>0&&<details><summary>Atividade da análise ({activity.length})</summary><ol className="security-events">
      {activity.map((e,i)=><li key={i}><span>{analysisDisplayText(e.message||e.stage)}</span><time dateTime={e.at}>{shortDate(e.at)}</time></li>)}</ol></details>}
    {scan.result&&<Report result={scan.result}/>}
    {scan.error&&<p role="alert">{analysisDisplayText(scan.error)}</p>}
    {scanActive(scan.status)&&<button className="button" disabled={busy||scan.status==='cancelling'} onClick={()=>void controller.cancel()}>Cancelar análise de segurança</button>}
    {error&&<div role="alert">{analysisDisplayText(error)} <button className="text-button" onClick={controller.refresh}>Consultar estado novamente</button></div>}
  </section>;
}
