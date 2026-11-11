import RemediationContentReview from './RemediationContentReview';
import { ProjectionNotice } from './DataAccess';
import type { ProjectionInfo } from '../data-policy';
import { OperationTarget, useOperationTarget } from './TargetSelection';
import { ScopeRestriction, useScopeAccess } from './ScopeAccess';
import { useCallback, useEffect, useRef, useState } from 'react';
import { native, request } from '../api';
import { Action, Assessment, Finding, Project, errorMessage, shortDate } from '../types';
import { Field, KeyValue, Notice } from './shared';
import RemediationReconciliation, { RemediationReceipt } from './RemediationReconciliation';

type RetestKind='contains'|'absent'|'sha256'|'json.debug-disabled.v1';
interface RemediationPlan {
  id:string;projectId:string;assessmentId:string;findingId:string;target:string;relativePath:string;filePath:string;
  projection?:ProjectionInfo;before?:string|null;after?:string|null;baseSha256:string;afterSha256:string;retest:{kind:RetestKind;value:string};
  digest:string;createdAt:string;expiresAt:string;status:string;approvedAt:string|null;appliedAt:string|null;
  retestedAt:string|null;confirmedAt:string|null;evidenceId:string|null;passed:boolean|null;message:string;limits:string[];
  targetRef?:import('../combined-scope').ScopeTargetRef;scopeRef?:import('../scope').ScopeRef;retestScopeRef?:import('../scope').ScopeRef;interruptedOperation?:string|null;reconciliation?:RemediationReceipt|null;
}
const states:Record<string,string>={proposed:'Aguardando revisão',approved:'Aprovada',applying:'Aplicando',applied:'Alteração aplicada',retesting:'Verificando',retested:'Reteste registrado',confirming:'Registrando confirmação',confirmed:'Confirmada',revoked:'Revogada',invalidated:'Invalidada',rolled_back:'Alteração revertida',failed:'Falhou',unknown:'Efeito desconhecido',reconciled:'Reconciliada por observação'};
const retestLabels:Record<RetestKind,string>={contains:'Conter o texto esperado',absent:'Não conter o texto indicado',sha256:'Corresponder ao SHA-256 esperado','json.debug-disabled.v1':'Desativar o campo debug de uma configuração JSON'};
const maxFileBytes=256*1024;

export default function Remediation({finding,assessment,project,action,busy}:{finding:Finding;assessment?:Assessment;project?:Project;action:Action;busy:boolean}) {
  const scope=useScopeAccess();const target=useOperationTarget('local.text-remediation.v1');
  const [plans,setPlans]=useState<RemediationPlan[]>([]);const [selected,setSelected]=useState('');
  const [creating,setCreating]=useState(false);const [relativePath,setRelativePath]=useState('');
  const [after,setAfter]=useState('');const [kind,setKind]=useState<RetestKind>('absent');const [value,setValue]=useState('');
  const [contentReviewed,setContentReviewed]=useState('');const [approval,setApproval]=useState('');const [confirmChecked,setConfirmChecked]=useState(false);const [rationale,setRationale]=useState('');
  const [pending,setPending]=useState(false);const pendingRef=useRef(false);const [error,setError]=useState('');
  const plan=plans.find(item=>item.id===selected)??plans[0];
  const afterBytes=new TextEncoder().encode(after).byteLength;
  const local=target.combined?target.options.some(t=>['code','files'].includes(t.entry.kind)):assessment&&['code','files'].includes(assessment.kind);
  const unresolved=plans.some(item=>['applying','retesting','confirming','unknown'].includes(item.status));
  const canPrepare=scope.allows('local.text-remediation.v1')&&native&&local&&project&&!project.archived&&finding.status==='validated'&&!unresolved;
  const expired=plan?Date.parse(plan.expiresAt)<=Date.now():false;
  const blocked=busy||pending;const methodAllowed=plan?scope.allowsTarget('local.text-remediation.v1',plan.targetRef):scope.allows('local.text-remediation.v1');const writeBlocked=blocked||!methodAllowed||!!plan&&!scope.matches(plan.scopeRef);const retestBlocked=blocked||!methodAllowed||!!plan?.scopeRef&&!scope.matches(plan.scopeRef);const confirmBlocked=retestBlocked||!scope.matches(plan?.retestScopeRef);

  const reload=useCallback(async()=>{
    const response=await request<{plans:RemediationPlan[]}>('remediation.list',{findingId:finding.id});
    setPlans(response.plans);return response.plans;
  },[finding.id]);
  useEffect(()=>{
    setSelected('');setCreating(false);setApproval('');setConfirmChecked(false);setRationale('');setError('');setPlans([]);
    let mounted=true;
    if(native)void request<{plans:RemediationPlan[]}>('remediation.list',{findingId:finding.id}).then(response=>{if(mounted)setPlans(response.plans);}).catch(e=>{if(mounted)setError(errorMessage(e));});
    return()=>{mounted=false;};
  },[finding.id]);

  async function perform(name:string,payload:Record<string,unknown>){
    if(pendingRef.current)return;if(name==='remediation.prepare'&&!target.ready)return;if(name==='remediation.approve'&&contentReviewed!==plan?.digest)return;if(name!=='remediation.reconcile'&&(!(name==='remediation.prepare'?scope.allowsTarget('local.text-remediation.v1',target.option?.targetRef):methodAllowed)||(name==='remediation.confirm'?confirmBlocked:name==='remediation.retest'?retestBlocked:name!=='remediation.prepare'&&!scope.matches(plan?.scopeRef))))return;
    pendingRef.current=true;setPending(true);setError('');
    try {
      const result=await action<RemediationPlan>(name,payload);
      setPlans(items=>[result,...items.filter(item=>item.id!==result.id)]);setSelected(result.id);
      setCreating(false);setApproval('');setConfirmChecked(false);setRationale('');
    }catch(e){setError(errorMessage(e));await reload().catch(()=>{});}
    finally{pendingRef.current=false;setPending(false);}
  }
  async function refresh(){setPending(true);setError('');try{await reload();}catch(e){setError(errorMessage(e));}finally{setPending(false);}}
  function prepare(e:React.FormEvent){e.preventDefault();void perform('remediation.prepare',{findingId:finding.id,...target.payload,relativePath,after,retest:{kind,value:kind==='json.debug-disabled.v1'?'false':value}});}

  return <div className="remediation">
    <section className="content-section"><h2>Orientação de correção</h2><ScopeRestriction method="local.text-remediation.v1"/><p className="preserve-lines">{finding.recommendation||'Uma recomendação ainda não foi registrada.'}</p>
      <Notice>O fluxo altera um arquivo de texto local após revisão e aprovação. O reteste verifica a condição escolhida no arquivo revisado. A confirmação do achado exige uma decisão humana.</Notice>
      {!native?<p className="muted">Aplicar correções e retestar está disponível no aplicativo desktop.</p>:!local?<p className="muted">Este fluxo está disponível para projetos locais de código ou arquivos. Alvos Web e rede mantêm recomendações de correção.</p>:finding.status==='candidate'?<p className="muted">Valide o candidato com evidências antes de propor uma alteração.</p>:project?.archived?<p className="muted">O projeto está arquivado e não permite alterações.</p>:null}
      {unresolved&&<Notice tone="warning">Há uma alteração com resultado ainda não reconciliado. Novas alterações estão bloqueadas; consulte os registros e preserve o arquivo atual.</Notice>}
      <div className="inline-actions">{canPrepare&&<button className="button primary" type="button" disabled={blocked} onClick={()=>setCreating(true)}>Preparar correção local</button>}{native&&<button className="button" type="button" disabled={blocked} onClick={()=>void refresh()}>Atualizar histórico de correções</button>}</div>
    </section>
    {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}
    {creating&&canPrepare&&<form className="content-section" onSubmit={prepare}><h2>Nova proposta de alteração</h2><KeyValue label="Projeto">{project?.name}</KeyValue><OperationTarget selection={target} label="Entrada da correção local" disabled={blocked}/><KeyValue label="Alvo autorizado"><span className="break-word">{target.combined?target.option?.entry.target??'Selecione uma entrada':assessment?.target}</span></KeyValue><div className="form-group"><Field label="Caminho relativo do arquivo" hint="Um arquivo de texto existente dentro do alvo autorizado. O conteúdo original será lido para a revisão."><input value={relativePath} onChange={e=>setRelativePath(e.target.value)} required maxLength={2000} placeholder="src/arquivo.ts" disabled={blocked}/></Field><Field label="Conteúdo proposto" hint="Informe o conteúdo completo que o arquivo deverá ter após a alteração."><textarea className="mono" value={after} onChange={e=>setAfter(e.target.value)} rows={10} maxLength={maxFileBytes} disabled={blocked}/></Field><p className="muted">{afterBytes.toLocaleString('pt-BR')} de {maxFileBytes.toLocaleString('pt-BR')} bytes permitidos</p><Field label="Condição do reteste"><select value={kind} onChange={e=>setKind(e.target.value as RetestKind)} disabled={blocked}>{Object.entries(retestLabels).map(([key,label])=><option key={key} value={key}>{label}</option>)}</select></Field>{kind==='json.debug-disabled.v1'?<p>A base deve ter <code>debug: true</code> na raiz do objeto JSON. O reteste verifica o valor booleano <code>false</code> no arquivo gravado.</p>:<Field label={kind==='sha256'?'SHA-256 esperado':'Texto da condição'} hint="A condição deve falhar no conteúdo original e passar no conteúdo proposto.">{kind==='sha256'?<input value={value} onChange={e=>setValue(e.target.value)} required pattern="[a-fA-F0-9]{64}" maxLength={64} disabled={blocked}/>:<textarea value={value} onChange={e=>setValue(e.target.value)} required maxLength={4000} rows={3} disabled={blocked}/>}</Field>}</div><div className="form-actions"><button className="button" type="button" disabled={blocked} onClick={()=>setCreating(false)}>Cancelar proposta</button><button className="button primary" type="submit" disabled={blocked||!target.ready||!relativePath.trim()||(kind!=='json.debug-disabled.v1'&&!value)||afterBytes>maxFileBytes}>Preparar revisão da alteração</button></div></form>}
    {plans.length>0&&!creating&&<section className="content-section"><div className="section-heading"><h2>Alterações propostas</h2>{plans.length>1&&<select aria-label="Selecionar proposta de correção" value={plan?.id??''} onChange={e=>{setSelected(e.target.value);setApproval('');setConfirmChecked(false);setRationale('');}}>{plans.map(item=><option key={item.id} value={item.id}>{item.relativePath} · {shortDate(item.createdAt)} · {states[item.status]??item.status}</option>)}</select>}</div>
      {plan&&<><div className="review-sheet"><KeyValue label="Entrada da alteração">{plan.target}</KeyValue><KeyValue label="Arquivo"><span className="break-word">{plan.filePath}</span></KeyValue><KeyValue label="Estado">{states[plan.status]??plan.status}</KeyValue><KeyValue label="Validade para aprovação">{shortDate(plan.expiresAt)}{expired?' · Expirada':''}</KeyValue><KeyValue label="Reteste">{retestLabels[plan.retest.kind]}</KeyValue><pre aria-label="Condição revisada do reteste">{plan.retest.value}</pre></div>
        <ProjectionNotice projection={plan.projection}/><div className="diff-comparison"><section><h3>Antes</h3><pre className="diff-before" aria-label="Conteúdo original do arquivo">{plan.before===undefined||plan.before===null?'Conteúdo omitido nesta projeção. Revise o registro local abaixo.':plan.before||'(arquivo vazio)'}</pre></section><section><h3>Depois</h3><pre className="diff-after" aria-label="Conteúdo proposto do arquivo">{plan.after===undefined||plan.after===null?'Conteúdo omitido nesta projeção. Revise o registro local abaixo.':plan.after||'(arquivo vazio)'}</pre></section></div>
        <details className="plan-details"><summary>Integridade e limites da alteração</summary><KeyValue label="Plano"><span className="mono break-word">{plan.digest}</span></KeyValue>{plan.scopeRef&&<KeyValue label="Escopo da alteração"><span className="mono break-word">{plan.scopeRef.revisionId} · {plan.scopeRef.digest}</span></KeyValue>}{plan.retestScopeRef&&<KeyValue label="Escopo do reteste"><span className="mono break-word">{plan.retestScopeRef.revisionId} · {plan.retestScopeRef.digest}</span></KeyValue>}<KeyValue label="Arquivo original"><span className="mono break-word">{plan.baseSha256}</span></KeyValue><KeyValue label="Arquivo proposto"><span className="mono break-word">{plan.afterSha256}</span></KeyValue><ul className="plain-list">{plan.limits.map((limit,index)=><li key={index}>{limit}</li>)}</ul></details>
        {plan.message&&<Notice tone={['failed','unknown','invalidated'].includes(plan.status)?'warning':'info'}>{plan.message}</Notice>}
        {native&&<RemediationReconciliation key={plan.id} status={plan.status} interruptedOperation={plan.interruptedOperation} receipt={plan.reconciliation} blocked={blocked} onReconcile={note=>perform('remediation.reconcile',{id:plan.id,note,acknowledgeUnknownEffects:true})}/>}
        {plan.status==='proposed'&&!expired&&<><RemediationContentReview key={plan.digest} assessmentId={plan.assessmentId} planId={plan.id} digest={plan.digest} onReviewed={complete=>{setContentReviewed(complete?plan.digest:'');if(!complete)setApproval('');}}/><label className="check-field"><input type="checkbox" checked={approval===plan.digest} onChange={e=>setApproval(e.target.checked?plan.digest:'')} disabled={blocked||contentReviewed!==plan.digest}/><span>Revisei o arquivo, o conteúdo antes e depois e a condição de reteste. Autorizo esta alteração específica.</span></label><button className="button primary" type="button" disabled={writeBlocked||approval!==plan.digest||contentReviewed!==plan.digest} onClick={()=>void perform('remediation.approve',{id:plan.id,digest:plan.digest})}>Aprovar alteração revisada</button></>}
        {plan.status==='approved'&&!expired&&<div className="inline-actions"><button className="button primary" type="button" disabled={writeBlocked} onClick={()=>void perform('remediation.apply',{id:plan.id,digest:plan.digest})}>Aplicar alteração aprovada</button></div>}
        {expired&&['proposed','approved'].includes(plan.status)&&<Notice tone="warning">A autorização para iniciar esta proposta expirou. Prepare e revise uma nova proposta antes de alterar o arquivo.</Notice>}
        {(plan.status==='applied'||plan.status==='retested'&&!plan.scopeRef)&&<><Notice>O arquivo foi alterado. O achado continua validado até o reteste e a confirmação humana.</Notice><button className="button" type="button" disabled={retestBlocked} onClick={()=>void perform('remediation.retest',{id:plan.id,digest:plan.digest})}>{plan.retest.kind==='json.debug-disabled.v1'?'Verificar o campo debug no JSON':'Executar reteste de texto ou hash'}</button></>}
        {plan.status==='retested'&&!scope.matches(plan.retestScopeRef)&&<Notice tone="warning">O reteste anterior não está vinculado à revisão atual de escopo. Uma confirmação exige novo reteste autorizado; a alteração antiga não será aplicada outra vez.</Notice>}{plan.status==='retested'&&<section className="retest-result"><h3>Resultado do reteste</h3><KeyValue label="Condição verificada">{plan.passed===true?'Atendida':plan.passed===false?'Não atendida':'Não confirmada'}</KeyValue>{plan.evidenceId&&<KeyValue label="Evidência"><span className="mono break-word">{plan.evidenceId}</span></KeyValue>}<Notice>A condição verificada descreve somente o teste acima. Confirme o alcance do resultado antes de registrar a correção.</Notice>{plan.passed&&plan.evidenceId?<form onSubmit={e=>{e.preventDefault();void perform('remediation.confirm',{id:plan.id,digest:plan.digest,evidenceId:plan.evidenceId,rationale});}}><Field label="Justificativa da confirmação"><textarea rows={4} value={rationale} onChange={e=>setRationale(e.target.value)} required maxLength={4000} disabled={blocked}/></Field><label className="check-field"><input type="checkbox" checked={confirmChecked} onChange={e=>setConfirmChecked(e.target.checked)} disabled={blocked}/><span>Revisei a evidência e os limites do reteste e confirmo a correção deste achado neste alcance.</span></label><button className="button primary" type="submit" disabled={confirmBlocked||!confirmChecked||!rationale.trim()}>Confirmar correção revisada</button></form>:<Notice tone="warning">A confirmação está bloqueada porque a condição não foi comprovada.</Notice>}</section>}
        {plan.status==='confirmed'&&<Notice>Correção confirmada com justificativa humana e evidência do reteste. Consulte o histórico para os limites registrados.</Notice>}
      </>}
    </section>}
  </div>;
}
