import { useRef, useState, type FormEvent } from 'react';
import { ArrowRight, LoaderCircle, RefreshCw } from 'lucide-react';
import { request } from '../api';
import { accountChanged, site } from '../commercial';
import { useDesktopSession } from '../desktop-session';
import { errorMessage } from '../types';
import { KMark } from './ProjectRail';

export function Startup({error,onRetry}:{error?:string;onRetry:()=>void}){
  return <main className="desktop-entry startup-entry" aria-busy={!error}>
    <div className="entry-mark"><KMark size={34}/></div><h1>OpenKoda</h1>
    {error?<><p role="alert">{error}</p><p>Seus projetos e conversas permanecem neste dispositivo.</p><button className="button primary" onClick={onRetry}><RefreshCw size={16}/>Tentar novamente</button></>:
      <p role="status"><LoaderCircle size={17} className="spin"/>Preparando seus projetos e verificando a sessão…</p>}
  </main>;
}

export function AccountSignIn(){
  const [email,setEmail]=useState(''),[password,setPassword]=useState(''),[emailMode,setEmailMode]=useState(false);
  const [pending,setPending]=useState<''|'google'|'email'|'external'>(''),[error,setError]=useState(''),[message,setMessage]=useState('');
  const lock=useRef(false),attempt=useRef(0);
  async function authenticate(method:'google'|'signIn'){
    if(lock.current)return;lock.current=true;const stamp=++attempt.current;setPending(method==='google'?'google':'email');setError('');setMessage('');
    try {
      await request('account.'+method,method==='google'?{}:{email,password});
      if(stamp===attempt.current){setPassword('');window.dispatchEvent(new Event(accountChanged));}
    } catch(cause){if(stamp===attempt.current)setError(errorMessage(cause));}
    finally{if(stamp===attempt.current){lock.current=false;setPending('');}}
  }
  async function open(page:Parameters<typeof site>[0]){
    if(lock.current)return;lock.current=true;setPending('external');setError('');
    try{await site(page);setMessage('Continue no navegador. Volte aqui para entrar após concluir.');}
    catch(cause){setError(errorMessage(cause));}finally{lock.current=false;setPending('');}
  }
  async function cancel(){
    try{await request('account.loginCancel');attempt.current++;lock.current=false;setPending('');setMessage('Entrada cancelada. Você pode tentar novamente.');}
    catch(cause){setError(errorMessage(cause));}
  }
  function submit(event:FormEvent){event.preventDefault();void authenticate('signIn');}
  return <div className="account-signin">
    <button type="button" className="button primary google-signin" disabled={!!pending} onClick={()=>void authenticate('google')}>
      {pending==='google'?<LoaderCircle size={18} className="spin"/>:<span className="google-letter" aria-hidden="true">G</span>}Continuar com Google
    </button>
    {pending==='google'&&<div className="auth-wait" role="status"><p>Conclua a entrada no navegador e clique em “Abrir aplicativo”. Mantenha o OpenKoda aberto para receber o retorno.</p><button type="button" className="button quiet" onClick={()=>void cancel()}>Cancelar entrada</button></div>}
    {!emailMode?<button type="button" className="button email-choice" disabled={!!pending} onClick={()=>setEmailMode(true)}>Entrar com e-mail</button>:
      <form onSubmit={submit}><label>E-mail<input type="email" required autoComplete="email" value={email} disabled={!!pending} onChange={e=>setEmail(e.target.value)}/></label>
        <label>Senha<input type="password" required autoComplete="current-password" value={password} disabled={!!pending} onChange={e=>setPassword(e.target.value)}/></label>
        <button className="button primary" disabled={!!pending} type="submit">{pending==='email'?'Entrando…':'Entrar com e-mail'}</button>
      </form>}
    <div className="entry-links"><button type="button" disabled={!!pending} onClick={()=>void open('signup')}>Criar conta no site</button><button type="button" disabled={!!pending} onClick={()=>{
      if(lock.current)return;lock.current=true;setPending('external');setError('');
      void request('account.recover',{}).then(()=>setMessage('Continue a recuperação de senha no navegador.')).catch(cause=>setError(errorMessage(cause))).finally(()=>{lock.current=false;setPending('');});
    }}>Recuperar senha</button></div>
    {error&&<p className="account-error" role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
  </div>;
}

export function WelcomeEntry(){
  const session=useDesktopSession();
  return <main className="desktop-entry welcome-entry">
    <header><div className="entry-mark"><KMark size={29}/></div><span>OpenKoda</span></header>
    <section className="welcome-account" aria-labelledby="welcome-account-title"><p className="entry-eyebrow">Sua área de trabalho</p><h1 id="welcome-account-title">Entre para continuar.</h1>
      <p>Conecte sua conta para acompanhar o plano, o uso e o suporte. Seus projetos e conversas ficam neste computador.</p>
      {(session.state==='expired'||session.message)&&<p className="entry-session-message" role="status">{session.message||'Sua sessão expirou. Entre novamente para acessar os serviços da conta.'}</p>}
      <AccountSignIn/>
      <div className="entry-local"><button type="button" onClick={session.continueLocally}>Continuar com minha chave de API<ArrowRight size={16}/></button><p>Uso local, sem conta. Você configura seu provedor e acompanha os custos da sua chave.</p></div>
    </section><footer>Projetos locais. Controle sobre cada envio.</footer>
  </main>;
}
