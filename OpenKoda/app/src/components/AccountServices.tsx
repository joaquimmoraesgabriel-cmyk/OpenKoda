import {useRef,useState} from 'react';
import {request} from '../api';
import type {AccountView} from '../commercial';
import {errorMessage} from '../types';

import {SupportTickets} from './SupportTickets';
export function AccountServices({account,reload}:{account:AccountView;reload:()=>Promise<void>}){
  const [pending,setPending]=useState(false),[error,setError]=useState(''),[feedback,setFeedback]=useState('');
  const busy=useRef(false),checkout=useRef<{id:string;plan:string}|undefined>(undefined);
  async function run(work:()=>Promise<void>){
    if(busy.current)return;busy.current=true;setPending(true);setError('');setFeedback('');
    try{await work();}catch(e){setError(errorMessage(e));}finally{busy.current=false;setPending(false);}
  }
  return <div className="account-services">
    {account.controls?.mockCheckout&&account.plans?.length&&<details><summary>Simular assinatura</summary>
      <p>O checkout de teste não cobra e não libera créditos de produção.</p>
      {account.plans.map(plan=><button className="button" type="button" key={plan.id} disabled={pending} onClick={()=>void run(async()=>{
        if(checkout.current?.plan!==plan.id)checkout.current={id:crypto.randomUUID(),plan:plan.id};
        const result=await request<{status:string;grantsProductionCredit:boolean}>('account.checkout',{plan:plan.id,requestId:checkout.current.id});
        if(result.status!=='simulated'||result.grantsProductionCredit!==false)throw new Error('Retorno de simulação inválido.');
        setFeedback('Simulação registrada.');checkout.current=undefined;
      })}>{plan.id.toLocaleUpperCase()} · {new Intl.NumberFormat('pt-BR',{style:'currency',currency:'BRL'}).format(plan.priceBrlCents/100)} por mês</button>)}
    </details>}
    {account?.subscription?.status==='active'&&!account.subscription.cancelAtPeriodEnd&&<details><summary>Cancelar renovação</summary>
      <button className="button" type="button" disabled={pending} onClick={()=>void run(async()=>{
        await request('account.cancel');await reload();setFeedback('Renovação cancelada.');
      })}>Confirmar cancelamento da renovação</button></details>}
    <details><summary>Chamados de suporte e erros</summary><SupportTickets/></details>
    {account.consent?.termsVersion&&<label><input type="checkbox" disabled={pending} checked={!!account.consent.marketing} onChange={e=>{
      const enabled=e.target.checked;void run(async()=>{await request('account.marketing',{enabled});await reload();});
    }}/> Receber novidades por e-mail</label>}
    {feedback&&<p role="status">{feedback}</p>}{error&&<p role="alert">{error}</p>}
  </div>;
}
