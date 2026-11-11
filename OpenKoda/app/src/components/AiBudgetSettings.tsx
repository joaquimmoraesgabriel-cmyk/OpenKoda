import { useEffect, useRef, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import { native, request } from '../api';
import { BudgetPrice, BudgetStatus, usd, usdMicros } from '../ai-budget';
import { Action, Payload, errorMessage, shortDate } from '../types';
import { Field, KeyValue, Notice } from './shared';
import { BudgetAnalytics, BudgetOverview } from './BudgetInsights';
import { ModelCatalogField, ProviderField } from './ProviderModels';
import './profile-usage.css';

const sections = ['Visão geral', 'Análises', 'Política de orçamento'] as const;

export default function AiBudgetSettings({action,busy}:{action:Action;busy:boolean}){
  const [section,setSection] = useState<typeof sections[number]>('Visão geral');
  const [status,setStatus]=useState<BudgetStatus>();const [enabled,setEnabled]=useState(false);const [installation,setInstallation]=useState('');const [assessment,setAssessment]=useState('');const [prices,setPrices]=useState<BudgetPrice[]>([]);const [review,setReview]=useState<Payload>();const [ackEstimates,setAckEstimates]=useState(false);const [ackLegacy,setAckLegacy]=useState(false);const [ackUnbounded,setAckUnbounded]=useState(false);
  const [chargeId,setChargeId]=useState('');const [assumed,setAssumed]=useState('');const [note,setNote]=useState('');const [ackUnknown,setAckUnknown]=useState(false);const [pending,setPending]=useState(false);const [error,setError]=useState('');const [message,setMessage]=useState('');const pendingRef=useRef(false);
  async function reload(reset=false){try{const next=await request<BudgetStatus>('ai.budget.status');if(typeof next.enabled!=='boolean')throw new Error('Estado de orçamento indisponível.');setStatus(next);if(reset){setEnabled(next.enabled);setInstallation(next.policy?.installationLimitUsd??'');setAssessment(next.policy?.assessmentLimitUsd??'');setPrices(next.policy?.prices.map(({revision: _revision,...price})=>price)??[]);}return next;}catch(e){setStatus(undefined);throw e;}}
  useEffect(()=>{if(native)void reload(true).catch(e=>setError(errorMessage(e)));},[]);
  const draftKey=JSON.stringify([enabled,installation,assessment,prices,status?.revision,status?.legacyReviewDigest]);
  useEffect(()=>{setReview(undefined);setAckEstimates(false);setAckLegacy(false);setAckUnbounded(false);},[draftKey]);
  useEffect(()=>{setAssumed('');setNote('');setAckUnknown(false);},[chargeId]);
  const disabled=busy||pending;const amountValid=(value:string)=>usdMicros(value)!==null;const positive=(value:string)=>{const amount=usdMicros(value);return amount!==null&&amount>0n;};
  const validPrices=prices.length>0&&prices.length<=100&&new Set(prices.map(p=>JSON.stringify([p.provider??'openai',p.model.trim()]))).size===prices.length&&prices.every(p=>/^[A-Za-z0-9._:/-]+$/.test(p.model.trim())&&new TextEncoder().encode(p.model.trim()).length<=160&&positive(p.inputUsdPerMillion)&&positive(p.outputUsdPerMillion)&&p.sourceNote.trim()&&new TextEncoder().encode(p.sourceNote.trim()).length<=500);
  const validLimits=positive(installation)&&positive(assessment)&&usdMicros(assessment)!<=usdMicros(installation)!;
  const charge=status?.unknownCharges.find(c=>c.chargeId===chargeId);const minCharge=charge?.heldUsd==null?null:usdMicros(charge.heldUsd);const assumedMicros=usdMicros(assumed);const noteBytes=new TextEncoder().encode(note.trim()).length;
  function priceChange(index:number,key:keyof BudgetPrice,value:string){setPrices(items=>items.map((p,i)=>i===index?{...p,[key]:value}:p));}
  function prepare(){if(!status)return;setMessage('');setError('');setAckEstimates(false);setAckLegacy(false);setAckUnbounded(false);setReview({expectedRevision:status.revision,enabled,currency:'USD',installationLimitUsd:installation||null,assessmentLimitUsd:assessment||null,prices:prices.map(p=>({model:p.model.trim(),...(p.provider ? {provider:p.provider} : {}),inputUsdPerMillion:p.inputUsdPerMillion,outputUsdPerMillion:p.outputUsdPerMillion,sourceNote:p.sourceNote.trim()})),legacyReviewDigest:status.legacyReviewDigest});}
  async function save(){if(!review||!ackEstimates||!ackLegacy||pendingRef.current)return;pendingRef.current=true;setPending(true);setError('');setMessage('');try{await action('ai.budget.configure',{...review,acknowledgeEstimates:true,acknowledgeLegacyExcluded:true,acknowledgeUnbounded:ackUnbounded});await reload(true);setMessage('Política de orçamento salva. Custos e reservas anteriores foram preservados.');}catch(e){setError(errorMessage(e));await reload().catch(()=>{});}finally{setReview(undefined);setAckEstimates(false);setAckLegacy(false);setAckUnbounded(false);pendingRef.current=false;setPending(false);}}
  async function resolve(){if(!charge||!ackUnknown||pendingRef.current)return;pendingRef.current=true;setPending(true);setError('');setMessage('');setAckUnknown(false);try{await action('ai.budget.resolveUnknown',{chargeId:charge.chargeId,assumedChargeUsd:assumed,note:note.trim(),acknowledgeUnknownCharge:true});await reload();setChargeId('');setMessage('Estimativa conservadora debitada no controle local. Uso e cobrança reais continuam desconhecidos; nenhuma chamada foi repetida.');}catch(e){setError(errorMessage(e));await reload().catch(()=>{});}finally{pendingRef.current=false;setPending(false);}}
  async function consult(){if(pendingRef.current)return;pendingRef.current=true;setPending(true);setError('');setReview(undefined);setAckUnknown(false);try{await reload(true);}catch(e){setError(errorMessage(e));}finally{pendingRef.current=false;setPending(false);}}
  return <section className="ai-budget-settings" onKeyDown={e=>{if(e.key==='Enter'&&(e.target as HTMLElement).tagName==='INPUT' && (e.target as HTMLInputElement).type!=='radio')e.preventDefault();}}>
    {!native ? <Notice>O orçamento de IA está disponível no aplicativo desktop. A prévia não configura limites nem realiza chamadas.</Notice> : <>
      <div className="usage-view-nav" role="group" aria-label="Seções de uso e orçamento">
        {sections.map(name=><button key={name} type="button" aria-pressed={section===name} onClick={()=>setSection(name)}>{name}</button>)}
        <button type="button" className="usage-refresh" aria-label="Atualizar orçamento e preços" disabled={disabled} onClick={()=>void consult()}><RefreshCw size={15} aria-hidden="true"/>Atualizar</button>
      </div>
      {error && <div role="alert"><Notice tone="warning">{error}</Notice></div>}
      {message && <p role="status">{message}</p>}
      {!status && !error && <p role="status">Carregando o registro de uso…</p>}
      {status && <>
        {status.degraded && <Notice tone="warning">{status.notice}</Notice>}
        {section === 'Visão geral' && <>
          <BudgetOverview status={status} onAnalysis={()=>setSection('Análises')} onPolicy={()=>setSection('Política de orçamento')}/>
          {!status.degraded && <p className="analysis-scope">{status.notice}</p>}
        </>}
        {section === 'Análises' && <BudgetAnalytics status={status}/>}
        {section === 'Política de orçamento' && <div className="usage-policy">
          <h3>Limites da instalação</h3>
          <p className="analysis-scope">Defina os limites e as tarifas em USD. Os custos registrados acompanham a instalação, inclusive após restaurar um perfil ou excluir um projeto.</p>
          <Notice>Os preços são manuais. A estimativa local pode divergir da fatura do provedor.</Notice>
        <label className="check-field"><input type="checkbox" checked={enabled} disabled={disabled||status.degraded} onChange={e=>setEnabled(e.target.checked)}/><span>Ativar controle monetário local para próximas chamadas.</span></label>
        <Field label="Limite da instalação em USD" hint="Valor positivo até USD 1.000.000, com ponto e até seis casas. Alterar o limite não reinicia débitos."><input inputMode="decimal" value={installation} disabled={disabled||status.degraded} onChange={e=>setInstallation(e.target.value)} placeholder="Informe o limite"/></Field>
        <Field label="Limite por avaliação em USD" hint="Deve ser positivo e não superar o limite da instalação. O consumo já registrado é preservado."><input inputMode="decimal" value={assessment} disabled={disabled||status.degraded} onChange={e=>setAssessment(e.target.value)} placeholder="Informe o limite"/></Field>
        <h3>Preços manuais por modelo</h3>
        {prices.map((price,index)=><fieldset className="context-selection" key={index} disabled={disabled||status.degraded}>
          <legend>Preço {index+1}</legend>
          <ProviderField label={`Provedor do preço ${index+1}`} value={price.provider??'openai'} disabled={disabled||status.degraded}
            onChange={provider=>setPrices(items=>items.map((item,i)=>i===index ? {...item,provider,model:'',inputUsdPerMillion:'',outputUsdPerMillion:'',sourceNote:''} : item))}/>
          <ModelCatalogField label={`Modelo do preço ${index+1}`} provider={price.provider??'openai'} value={price.model} disabled={disabled||status.degraded} allowManual onChange={value=>priceChange(index,'model',value)}/>
          <Field label={`Entrada por milhão em USD ${index+1}`}><input inputMode="decimal" value={price.inputUsdPerMillion} onChange={e=>priceChange(index,'inputUsdPerMillion',e.target.value)}/></Field>
          <Field label={`Saída por milhão em USD ${index+1}`}><input inputMode="decimal" value={price.outputUsdPerMillion} onChange={e=>priceChange(index,'outputUsdPerMillion',e.target.value)}/></Field>
          <Field label={`Origem e data do preço ${index+1}`} hint={`${new TextEncoder().encode(price.sourceNote).length} de 500 bytes UTF-8`}><textarea value={price.sourceNote} rows={2} maxLength={500} onChange={e=>priceChange(index,'sourceNote',e.target.value)}/></Field>
          <button type="button" className="button" onClick={()=>setPrices(items=>items.filter((_,i)=>i!==index))}>Remover preço {index+1}</button>
        </fieldset>)}
        <button type="button" className="button" disabled={disabled||status.degraded||prices.length>=100} onClick={()=>setPrices(items=>[...items,{model:'',inputUsdPerMillion:'',outputUsdPerMillion:'',sourceNote:''}])}>Adicionar preço de modelo</button>
        <div className="inline-actions"><button type="button" className="button" disabled={disabled||status.degraded||enabled&&(!validLimits||!validPrices)||!enabled&&((installation!==''&&!amountValid(installation))||(assessment!==''&&!amountValid(assessment))||(prices.length>0&&!validPrices))} onClick={prepare}>Revisar política de orçamento</button></div>
        {review&&<section className="maintenance-review" aria-label="Revisão da política de orçamento"><h3>Política a salvar</h3><KeyValue label="Controle">{enabled?'Ativado':'Desativado'}</KeyValue><KeyValue label="Limites">Instalação: {usd(installation||null)} · avaliação: {usd(assessment||null)}</KeyValue>{prices.map((p,i)=><p className="preserve-lines" key={i}>{p.provider??'openai'} · {p.model}: entrada {usd(p.inputUsdPerMillion)} e saída {usd(p.outputUsdPerMillion)} por milhão. Origem: {p.sourceNote}</p>)}<KeyValue label="Revisão anterior"><span className="mono break-word">{status.revision??'Nenhuma'}</span></KeyValue><KeyValue label="Revisão do legado"><span className="mono break-word">{status.legacyReviewDigest}</span></KeyValue><label className="check-field"><input type="checkbox" checked={ackEstimates} disabled={disabled} onChange={e=>setAckEstimates(e.target.checked)}/><span>Entendo que estes preços são manuais e as estimativas locais não são fatura nem garantia de cobrança máxima.</span></label><label className="check-field"><input type="checkbox" checked={ackLegacy} disabled={disabled} onChange={e=>setAckLegacy(e.target.checked)}/><span>Revisei o legado e reconheço que custos anteriores sem preço não entram no saldo deste período.</span></label>{!enabled&&status.epochId&&<label className="check-field"><input type="checkbox" checked={ackUnbounded} disabled={disabled} onChange={e=>setAckUnbounded(e.target.checked)}/><span>Autorizo desativar o controle monetário para novas chamadas, mantendo débitos e reservas já registrados.</span></label>}<button type="button" className="button" disabled={disabled||!ackEstimates||!ackLegacy||!enabled&&!!status.epochId&&!ackUnbounded} onClick={()=>void save()}>Salvar política revisada</button></section>}
        <details className="plan-details"><summary>Custos desconhecidos · {status.unknownChargesTotal??'não informado'}</summary><p className="muted">Reconhecer uma solicitação interrompida na conversa não resolve este custo global. Registros ainda em andamento não podem ser resolvidos. O débito assumido deve ser pelo menos a reserva retida; a cobrança real continuará desconhecida.</p><Field label="Custo desconhecido para revisar"><select value={chargeId} disabled={disabled} onChange={e=>setChargeId(e.target.value)}><option value="">Selecione um registro</option>{status.unknownCharges.map(c=><option key={c.chargeId} value={c.chargeId}>{shortDate(c.createdAt)} · {c.model} · {c.chargeId}</option>)}</select></Field>{charge&&<><KeyValue label="Reserva retida deste registro">{usd(charge.heldUsd)}</KeyValue><Field label="Débito conservador assumido em USD"><input inputMode="decimal" value={assumed} disabled={disabled} onChange={e=>{setAssumed(e.target.value);setAckUnknown(false);}}/></Field><Field label="Justificativa do custo desconhecido" hint={`${noteBytes} bytes UTF-8`}><textarea value={note} rows={3} maxLength={2000} disabled={disabled} onChange={e=>{setNote(e.target.value);setAckUnknown(false);}}/></Field><label className="check-field"><input type="checkbox" checked={ackUnknown} disabled={disabled} onChange={e=>setAckUnknown(e.target.checked)}/><span>Reconheço a cobrança desconhecida e autorizo debitar esta estimativa conservadora no controle local, sem afirmar o valor da fatura.</span></label><button type="button" className="button" disabled={disabled||!ackUnknown||!note.trim()||noteBytes>2000||assumedMicros===null||assumedMicros<=0n||minCharge!==null&&assumedMicros<minCharge} onClick={()=>void resolve()}>Registrar débito conservador</button></>}</details>
        </div>}
        {status.legacyCostsUnknown && !status.degraded && <Notice tone="warning">Existem solicitações anteriores sem custo precificado. Elas ficam fora do saldo deste período e o custo permanece desconhecido.</Notice>}
      </>}
    </>}
  </section>;
}
