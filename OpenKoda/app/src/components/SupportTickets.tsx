import {useRef,useState} from 'react';
import {request} from '../api';
import {errorMessage} from '../types';
type Ticket={id:string;subject:string;message:string;status:string;replies:{message:string;createdAt:string}[]};
export function SupportTickets(){
  const [subject,setSubject]=useState(''),[body,setBody]=useState(''),[tickets,setTickets]=useState<Ticket[]>();
  const [pending,setPending]=useState(false),[error,setError]=useState(''),[feedback,setFeedback]=useState('');
  const busy=useRef(false),supportRequest=useRef('');
  async function run(work:()=>Promise<void>){
    if(busy.current)return;busy.current=true;setPending(true);setError('');setFeedback('');
    try{await work();}catch(e){setError(errorMessage(e));}finally{busy.current=false;setPending(false);}
  }
  async function loadTickets(){setTickets((await request<{tickets:Ticket[]}>('account.supportList')).tickets);}
  return <div className="account-services">
      <form onSubmit={e=>{e.preventDefault();void run(async()=>{
        supportRequest.current||=crypto.randomUUID();
        await request('account.support',{subject:subject.trim(),message:body.trim(),requestId:supportRequest.current});
        setSubject('');setBody('');supportRequest.current='';setFeedback('Chamado registrado.');await loadTickets();
      });}}>
        <label>Assunto<input value={subject} required maxLength={160} disabled={pending} onChange={e=>{supportRequest.current='';setSubject(e.target.value);}}/></label>
        <label>Descrição do chamado<textarea value={body} required maxLength={4000} disabled={pending} onChange={e=>{supportRequest.current='';setBody(e.target.value);}}/></label>
        <button className="button" type="submit" disabled={pending||!subject.trim()||!body.trim()}>Enviar chamado</button>
      </form>
      <button className="button" type="button" disabled={pending} onClick={()=>void run(loadTickets)}>Consultar meus chamados</button>
      {tickets?.length===0&&<p>Nenhum chamado registrado.</p>}
      {tickets?.map(t=><details key={t.id}><summary>{t.subject}</summary><p>{t.message}</p>
        {t.replies.map((r,i)=><p key={i}><strong>Suporte</strong> · {r.message}</p>)}</details>)}
    {feedback&&<p role="status">{feedback}</p>}{error&&<p role="alert">{error}</p>}
  </div>;
}
