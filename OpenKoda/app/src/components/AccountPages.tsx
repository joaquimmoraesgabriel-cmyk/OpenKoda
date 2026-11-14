import { useEffect, useRef, useState } from 'react';
import { Check, LoaderCircle, ShieldCheck } from 'lucide-react';
import { native, request } from '../api';
import { accountChanged, site, usdMicros, type AccountView, type SimulationOutcome, type SimulationResult } from '../commercial';
import { useDesktopSession } from '../desktop-session';
import { errorMessage } from '../types';
import { AccountSignIn } from './DesktopEntry';
import { PlanSwitch } from './motion/PlanSwitch';

export function accountPlanLabel(account:AccountView|null){
  if(account?.plan&&account.subscription?.status==='active')return `Plano ${account.plan.id[0].toUpperCase()+account.plan.id.slice(1)}`;
  if(account?.simulation?.status==='active')return `Plano ${account.simulation.plan[0].toUpperCase()+account.simulation.plan.slice(1)} · simulação interna`;
  if(account?.plan)return `Plano ${account.plan.id[0].toUpperCase()+account.plan.id.slice(1)}`;
  return 'Uso com chave própria';
}
function useAccountAction(){
  const [pending,setPending]=useState(false),[error,setError]=useState(''),[message,setMessage]=useState('');
  const lock=useRef(false);
  async function run(work:()=>Promise<void>){
    if(lock.current)return;lock.current=true;setPending(true);setError('');setMessage('');
    try{await work();}catch(cause){setError(errorMessage(cause));}finally{lock.current=false;setPending(false);}
  }
  return {pending,error,message,setMessage,run};
}
function Feedback({pending,error,message}:{pending:boolean;error:string;message:string}){
  return <>{pending&&<p className="account-progress" role="status"><LoaderCircle className="spin" size={16}/>Atualizando sua conta…</p>}{error&&<p role="alert" className="account-error">{error}</p>}{message&&<p role="status" className="account-confirmation">{message}</p>}</>;
}

export function AccountSecurity(){
  const session=useDesktopSession(),work=useAccountAction();
  const [terms,setTerms]=useState(false),[marketing,setMarketing]=useState(false);
  useEffect(()=>{setTerms(false);setMarketing(false);},[session.user?.id]);
  if(!native)return <p>Na prévia, os serviços de conta não estão conectados.</p>;
  if(session.loading&&!session.user)return <p role="status">Verificando sua conta…</p>;
  if(!session.user)return <><p>Entre para acessar os serviços da sua conta. Seus projetos locais continuam disponíveis.</p><AccountSignIn/></>;
  const account=session.account,consent=account?.consent;
  const termsVersion=consent?.requiredTermsVersion,privacyVersion=consent?.requiredPrivacyVersion;
  const accepted=!!termsVersion&&!!privacyVersion&&consent?.termsVersion===termsVersion&&consent?.privacyVersion===privacyVersion;
  const offline=session.state==='offline'||!account;
  return <div className="account-settings">
    <div className="account-setting-row"><div><h3>Conta conectada</h3><p>{session.user.email}</p></div><span className="account-state">{session.state==='offline'?'Sem conexão':'Conectada'}</span></div>
    <section className="account-settings-section"><h3>Sessão e acesso</h3><p>A sessão é guardada pelo aplicativo neste dispositivo. Sair remove o acesso à conta; os projetos e conversas locais são preservados.</p>
      <div className="inline-actions"><button type="button" className="button" disabled={work.pending} onClick={()=>void work.run(()=>request('account.recover',{}).then(()=>{work.setMessage('Continue a recuperação de senha no navegador.');}))}>Recuperar senha</button>
        <button type="button" className="button" disabled={work.pending} onClick={()=>void work.run(async()=>{
          await request('account.signOut');window.dispatchEvent(new Event(accountChanged));
        })}>Sair da conta</button></div>
    </section>
    <section className="account-settings-section"><h3>Privacidade e comunicações</h3>
      {accepted?<p className="account-accepted"><ShieldCheck size={17}/>Termos e política de privacidade aceitos nesta conta.</p>:
        termsVersion&&privacyVersion?<div className="account-consent"><p>Revise os documentos antes de ativar os serviços da conta.</p><label className="check-field"><input type="checkbox" checked={terms} disabled={work.pending} onChange={e=>setTerms(e.target.checked)}/><span>Li e aceito os termos e a política de privacidade.</span></label>
          <label className="check-field"><input type="checkbox" checked={marketing} disabled={work.pending} onChange={e=>setMarketing(e.target.checked)}/><span>Quero receber novidades por e-mail. Opcional.</span></label>
          <button className="button primary" disabled={!terms||work.pending||offline} onClick={()=>void work.run(async()=>{await request('account.consent',{termsVersion,privacyVersion,marketing});await session.refresh();work.setMessage('Aceite registrado.');})}>Registrar aceite</button></div>:
          <p>{offline?'Conecte-se para consultar o aceite e as preferências da conta.':'Os documentos da conta ainda não foram carregados.'}</p>}
      {accepted&&<label className="check-field"><input type="checkbox" disabled={work.pending||offline} checked={!!consent?.marketing} onChange={e=>{
        const enabled=e.target.checked;void work.run(async()=>{await request('account.marketing',{enabled});await session.refresh();work.setMessage(enabled?'Recebimento de novidades ativado.':'Recebimento de novidades desativado.');});
      }}/><span>Receber novidades por e-mail. Você pode desativar a qualquer momento.</span></label>}
      <div className="entry-links"><button disabled={work.pending} onClick={()=>void work.run(()=>site('terms'))}>Termos de uso</button><button disabled={work.pending} onClick={()=>void work.run(()=>site('privacy'))}>Política de privacidade</button><button disabled={work.pending} onClick={()=>void work.run(()=>site('account'))}>Exportação e exclusão da conta</button></div>
    </section><Feedback {...work}/>
  </div>;
}

export function AccountPlans(){
  const session=useDesktopSession(),work=useAccountAction();
  const [previewPlan,setPreviewPlan]=useState('plus');
  const operation=useRef<{id:string;key:string}|undefined>(undefined);
  const account=session.account,simulation=account?.simulation;
  const activeSimulation=simulation?.status==='active';
  const productionActive=!!account?.plan&&account.subscription?.status==='active';
  const simulationOnly=activeSimulation&&!productionActive;
  const allowed=account?.controls?.simulationEnabled===true;
  async function simulate(plan:string,outcome:SimulationOutcome){
    await work.run(async()=>{
      const key=`${session.user?.id}:${plan}:${outcome}`;
      if(operation.current?.key!==key)operation.current={key,id:crypto.randomUUID()};
      const result=await request<SimulationResult>('account.simulation',{plan,requestId:operation.current.id,outcome});
      if(result.grantsProductionCredit!==false)throw new Error('O serviço retornou uma simulação inválida.');
      await session.refresh();window.dispatchEvent(new Event(accountChanged));
      operation.current=undefined;
      work.setMessage(result.status==='active'?'Plano de teste ativado. As permissões foram atualizadas.':result.status==='canceled'?'Simulação cancelada.':result.status==='expired'?'A simulação terminou. O acesso foi atualizado conforme sua conta.':'Falha simulada. Nenhum novo acesso foi concedido.');
    });
  }
  if(!native)return <p>Os planos são consultados no aplicativo conectado à sua conta.</p>;
  if(session.loading&&!session.user)return <p role="status">Verificando sua conta…</p>;
  if(!session.user)return <><p>Entre para consultar seu plano e as opções disponíveis.</p><AccountSignIn/></>;
  return <div className="account-plans">
    <div className="account-plan-current"><div><span>Seu plano</span><h3>{accountPlanLabel(account)}</h3><p>{simulationOnly?'Teste interno persistido na sua conta. Não há cobrança nem créditos de produção.':account?.plan?'A cota e as permissões são verificadas pelo serviço a cada operação.':'Você pode trabalhar com os provedores configurados no dispositivo.'}</p></div><span className="account-state">{productionActive?'Ativo':activeSimulation?'Em teste':'Local'}</span></div>
    {account?.usage&&<div className="plan-quota"><h3>Cota disponível</h3><dl>
      {typeof account.usage.windowLimitMicros==='number'&&typeof account.usage.windowUsedMicros==='number'&&<div><dt>Janela de 7 dias</dt><dd>{usdMicros(Math.max(0,account.usage.windowLimitMicros-account.usage.windowUsedMicros))} de {usdMicros(account.usage.windowLimitMicros)}</dd></div>}
      {typeof account.usage.monthlyLimitMicros==='number'&&typeof account.usage.monthlyUsedMicros==='number'&&<div><dt>Ciclo mensal</dt><dd>{usdMicros(Math.max(0,account.usage.monthlyLimitMicros-account.usage.monthlyUsedMicros))} de {usdMicros(account.usage.monthlyLimitMicros)}</dd></div>}
    </dl>{account.usage.windowEnd&&<p>Próxima liberação: {new Date(account.usage.windowEnd).toLocaleString('pt-BR')}.</p>}</div>}
    {session.loading?<p role="status">Consultando seu plano…</p>:!account?<div className="account-empty"><p>Não foi possível consultar os planos. As permissões pagas não serão presumidas sem conexão.</p><button type="button" className="button" onClick={()=>void session.refresh()}>Tentar novamente</button></div>:<>
      <h3 className="plans-heading">Planos OpenKoda</h3><p>Escolha de modelos conforme o plano e a compatibilidade da tarefa. Recursos e consumo são controlados pela conta.</p>
      {!!account.plans?.length&&<PlanSwitch value={account.plans.some(plan=>plan.id===previewPlan)?previewPlan:account.plans[0].id} onChange={setPreviewPlan} items={account.plans.map(plan=>({id:plan.id,label:plan.id[0].toUpperCase()+plan.id.slice(1)}))}/>}
      <div className="desktop-plan-grid">{account.plans?.map(plan=>{
        const selected=simulationOnly?simulation.plan===plan.id:account.plan?.id===plan.id;
        const simulated=activeSimulation&&simulation.plan===plan.id;
        const title=plan.id[0].toUpperCase()+plan.id.slice(1);
        return <section key={plan.id} className="desktop-plan" data-selected={selected} data-previewed={previewPlan===plan.id}><div className="desktop-plan-heading"><h4>{title}</h4>{selected&&<span><Check size={13}/>Atual</span>}</div>
          <p className="desktop-plan-price">{new Intl.NumberFormat('pt-BR',{style:'currency',currency:'BRL'}).format(plan.priceBrlCents/100)}<small>/mês</small></p>
          <p>{usdMicros(plan.monthlyLimitMicros)} de uso de IA por ciclo mensal, distribuído em janelas de 7 dias.</p>
          <ul><li>Modelos de IA pela Koda, conforme o catálogo disponível</li><li>Ajuste de raciocínio nos modelos compatíveis</li><li>Skills e histórico de uso</li></ul>
          {allowed?<button type="button" className="button" disabled={work.pending||session.loading||simulated} onClick={()=>void simulate(plan.id,'activate')}>{simulated?'Simulação ativa':`Simular ${title}`}</button>:<span className="plan-unavailable">Contratação ainda não disponível</span>}
        </section>;
      })}</div>
      {(allowed||simulation)&&<section className="simulation-tools"><h3>Controles do teste interno</h3><p>Disponível somente para contas autorizadas. A simulação libera os recursos de teste, incluindo raciocínio com chave própria; não faz chamadas de IA com saldo do serviço.</p>
        {simulation&&<p className="simulation-status">Plano {simulation.plan[0].toUpperCase()+simulation.plan.slice(1)} · simulação interna {simulation.status==='active'?'ativa até':simulation.status==='expired'?'expirada em':'cancelada'} {simulation.status!=='canceled'&&new Date(simulation.periodEnd).toLocaleString('pt-BR')}.</p>}
        {allowed&&<div className="inline-actions"><button className="button" type="button" disabled={work.pending||!activeSimulation} onClick={()=>void simulate(simulation!.plan,'cancel')}>Cancelar simulação</button><button className="button" type="button" disabled={work.pending||!activeSimulation} onClick={()=>void simulate(simulation!.plan,'expire')}>Simular expiração</button><button className="button" type="button" disabled={work.pending} onClick={()=>void simulate(simulation?.plan??'plus','fail')}>Simular falha</button></div>}</section>}
      {account.subscription?.status==='active'&&!account.subscription.cancelAtPeriodEnd&&<section className="account-settings-section"><h3>Renovação</h3><p>Cancele a renovação sem sair da sua conta.</p><button className="button" disabled={work.pending} onClick={()=>void work.run(async()=>{await request('account.cancel');await session.refresh();work.setMessage('Renovação cancelada.');})}>Cancelar renovação</button></section>}
    </>}
    <Feedback {...work}/>
  </div>;
}
