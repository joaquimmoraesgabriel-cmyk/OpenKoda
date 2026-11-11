import { useEffect, useState } from 'react';
import { request } from '../api';
import { Action, errorMessage, shortDate } from '../types';
import { KeyValue, Notice } from './shared';

type ProfileRef={kind:'base'}|{kind:'profile';id:string};
interface ActivationPlan {
  id:string;kind:'restore'|'rollback';sourceRecoveryId:string|null;
  fromProfile:ProfileRef;sourceProfile:ProfileRef;targetProfile:ProfileRef;
  files:{name:string;bytes:number;sha256:string}[];digest:string;createdAt:string;expiresAt:string;
  status:'prepared'|'scheduled'|'blocked'|'canceled'|'committed';message:string;
  effects:string[];limits:string[];restartRequired:true;
}
interface ActivationStatus {currentProfile:ProfileRef;activeProfile:ProfileRef;previousProfile:ProfileRef|null;pendingPlan:ActivationPlan|null;restartRequired:boolean;notice:string}
const labels:Record<ActivationPlan['status'],string>={prepared:'Preparado para revisão',scheduled:'Agendado',blocked:'Bloqueado',canceled:'Cancelado',committed:'Ativado'};
function profileName(profile:ProfileRef){return profile.kind==='base'?'Perfil original':`Perfil ${profile.id}`;}

export default function RecoveryActivation({recoveryId,action,busy}:{recoveryId?:string;action:Action;busy:boolean}) {
  const [status,setStatus]=useState<ActivationStatus>();const [plans,setPlans]=useState<ActivationPlan[]>([]);
  const [review,setReview]=useState<ActivationPlan>();const [approved,setApproved]=useState(false);
  const [pending,setPending]=useState(false);const [error,setError]=useState('');const [message,setMessage]=useState('');
  async function refresh(){
    const nextStatus=await request<ActivationStatus>('activation.status');
    const nextPlans=await request<{plans:ActivationPlan[]}>('activation.list');
    setStatus(nextStatus);setPlans(nextPlans.plans);
    setReview(current=>current?nextPlans.plans.find(plan=>plan.id===current.id)??current:current);
  }
  useEffect(()=>{void refresh().catch(e=>setError(errorMessage(e)));},[]);
  async function perform(name:string,payload:Record<string,unknown>={}){
    setPending(true);setError('');setMessage('');setApproved(false);
    const preparing=name==='activation.plan'||name==='activation.rollbackPlan';
    if(preparing)setReview(undefined);
    try{
      const result=await action<ActivationPlan>(name,payload);
      setReview(result);await refresh();
      if(name==='activation.schedule')setMessage('Troca agendada. Este processo continua usando o perfil atual. Feche e abra o aplicativo para verificar e ativar a cópia.');
      if(name==='activation.cancel')setMessage('Agendamento cancelado. O perfil atual foi preservado.');
    }catch(e){setError(errorMessage(e));await refresh().catch(()=>{});}finally{setPending(false);}
  }
  async function consult(){setPending(true);try{await refresh();}catch(e){setError(errorMessage(e));}finally{setPending(false);}}
  const disabled=busy||pending;const scheduled=status?.pendingPlan;
  const expired=review?Date.parse(review.expiresAt)<=Date.now():false;
  return <div className="recovery-activation spaced"><h3>Ativação de uma cópia</h3>
    <p className="muted">A troca acontece na próxima abertura do aplicativo. O perfil anterior é preservado; as credenciais e a propriedade do ambiente Kali não são transferidas.</p>
    {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}{message&&<p role="status">{message}</p>}
    {status&&<><KeyValue label="Perfil deste processo"><span className="mono break-word">{profileName(status.currentProfile)}</span></KeyValue><KeyValue label="Perfil ativo registrado"><span className="mono break-word">{profileName(status.activeProfile)}</span></KeyValue>{status.notice&&<Notice>{status.notice}</Notice>}</>}
    <div className="inline-actions">
      {recoveryId&&<button type="button" className="button primary" disabled={disabled||!status||!!scheduled} onClick={()=>void perform('activation.plan',{recoveryId})}>Revisar ativação desta cópia</button>}
      {status?.previousProfile&&<button type="button" className="button" disabled={disabled||!!scheduled} onClick={()=>void perform('activation.rollbackPlan')}>Revisar retorno ao perfil anterior</button>}
      <button type="button" className="button" disabled={disabled} onClick={()=>void consult()}>Atualizar estado da recuperação</button>
    </div>
    {scheduled&&<div className="maintenance-review"><h4>{scheduled.status==='blocked'?'Agendamento bloqueado':'Troca pendente'}</h4><p>{scheduled.message}</p><KeyValue label="Destino pendente"><span className="mono break-word">{profileName(scheduled.targetProfile)}</span></KeyValue>
      {scheduled.status==='scheduled'&&<Notice>Feche e abra o aplicativo para verificar a cópia e concluir a ativação. O agendamento não muda os dados usados nesta janela.</Notice>}
      {scheduled.status==='blocked'&&<Notice tone="warning">A troca não foi concluída. Revise o motivo e cancele este agendamento antes de preparar outro plano.</Notice>}
      <button type="button" className="button" disabled={disabled} onClick={()=>void perform('activation.cancel',{id:scheduled.id,digest:scheduled.digest})}>Cancelar troca agendada</button>
    </div>}
    {review&&<div className="maintenance-review" aria-label="Revisão da ativação"><h4>{review.kind==='rollback'?'Revisar retorno ao perfil anterior':'Revisar ativação da restauração'}</h4>
      <KeyValue label="Estado do plano">{labels[review.status]}</KeyValue><KeyValue label="Perfil atual previsto"><span className="mono break-word">{profileName(review.fromProfile)}</span></KeyValue><KeyValue label="Origem da cópia"><span className="mono break-word">{profileName(review.sourceProfile)}</span></KeyValue><KeyValue label="Nova cópia"><span className="mono break-word">{profileName(review.targetProfile)}</span></KeyValue><KeyValue label="Validade">{shortDate(review.expiresAt)}{expired?' · Expirado':''}</KeyValue>
      <p>{review.message}</p><ul>{review.effects.map((effect,index)=><li key={index}>{effect}</li>)}</ul>
      <Notice>{review.limits.map((limit,index)=><p key={index}>{limit}</p>)}</Notice>
      <details className="plan-details"><summary>Arquivos e integridade da cópia</summary><pre>{JSON.stringify(review.files,null,2)}</pre><p className="mono break-word">SHA-256 do plano: {review.digest}</p></details>
      {review.status==='prepared'&&<><label className="check-field"><input type="checkbox" checked={approved} onChange={e=>setApproved(e.target.checked)} disabled={disabled||expired||!!scheduled}/><span>Revisei os perfis, os arquivos e os efeitos. Autorizo esta troca na próxima abertura do aplicativo.</span></label><button type="button" className="button primary" disabled={disabled||!approved||expired||!!scheduled} onClick={()=>void perform('activation.schedule',{id:review.id,digest:review.digest})}>Agendar ativação revisada</button></>}
    </div>}
    {!!plans.length&&<details className="plan-details spaced"><summary>Histórico de ativação</summary><div className="backup-list">{plans.map(plan=><article className="backup-row" key={plan.id}><div><strong>{plan.kind==='rollback'?'Retorno ao perfil anterior':'Restauração'}</strong><small>{shortDate(plan.createdAt)} · {labels[plan.status]}</small></div><button type="button" className="button" disabled={disabled} onClick={()=>{setReview(plan);setApproved(false);}}>Rever plano</button></article>)}</div></details>}
  </div>;
}
