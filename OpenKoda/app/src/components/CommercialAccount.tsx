import { useEffect, useRef, useState, type FormEvent } from 'react';
import { native, request } from '../api';
import { accountChanged, type AccountUser, type AccountView, site, usdMicros } from '../commercial';
import { errorMessage } from '../types';
import {AccountServices} from './AccountServices';

export function CommercialAccount({summary=false}:{summary?:boolean}){
  const [user,setUser]=useState<AccountUser|null>(null),[account,setAccount]=useState<AccountView|null>(null);
  const [mode,setMode]=useState<'signIn'|'recover'>('signIn');
  const [email,setEmail]=useState(''),[password,setPassword]=useState('');
  const [pending,setPending]=useState(false),[error,setError]=useState(''),[message,setMessage]=useState('');
  const [terms,setTerms]=useState(false),[marketing,setMarketing]=useState(false);
  const currentIdentity=useRef<string|null>(null),loadRevision=useRef(0);
  async function reload(){
    const revision=++loadRevision.current;
    const session=await request<{user:AccountUser|null}>('account.session');
    if(revision!==loadRevision.current)return;
    if(currentIdentity.current!==(session.user?.id??null)){setAccount(null);setTerms(false);setMarketing(false);}
    currentIdentity.current=session.user?.id??null;
    setUser(session.user);
    if(session.user){
      const updated=await request<AccountView>('account.overview');
      if(revision===loadRevision.current)setAccount(updated);
    }else setAccount(null);
  }
  useEffect(()=>{
    if(!native)return;
    function changed(){void reload().catch(e=>setError(errorMessage(e)));}
    changed();window.addEventListener(accountChanged,changed);return()=>window.removeEventListener(accountChanged,changed);
  },[]);
  async function run(work:()=>Promise<void>){
    if(pending)return;setPending(true);setError('');setMessage('');
    try{await work();}catch(e){setError(errorMessage(e));}finally{setPending(false);}
  }
  async function authenticate(method:typeof mode|'google'){
    await run(async()=>{
      await request('account.'+method,method==='google'?{}:
        method==='recover'?{email}:{email,password});
      setPassword('');
      if(method==='recover'){setMessage('Continue a recuperação de senha no navegador.');return;}
      window.dispatchEvent(new Event(accountChanged));
    });
  }
  function submit(e:FormEvent){e.preventDefault();void authenticate(mode);}
  const open=(page:Parameters<typeof site>[0])=>void run(()=>site(page));
  if(!native||summary&&!user)return null;
  const termsVersion=account?.consent?.requiredTermsVersion,privacyVersion=account?.consent?.requiredPrivacyVersion;
  const accepted=!!termsVersion&&!!privacyVersion&&account?.consent?.termsVersion===termsVersion&&account?.consent?.privacyVersion===privacyVersion;
  return <section className="profile-block commercial-account" aria-label={summary?'Uso da assinatura':'Conta OpenKoda'}>
    <h2>{summary?'Uso da assinatura':'Conta OpenKoda'}</h2>
    {user?<><p>{user.email} · {account?.plan?.id?.toUpperCase()??'Sem assinatura'}</p>
      {account?.usage&&<dl className="profile-metrics">
        {[[account.usage.monthlyUsedMicros,'Consumo no ciclo'],[account.usage.monthlyLimitMicros,'Cota do ciclo'],
          [account.usage.windowUsedMicros,'Consumo na janela'],[account.usage.windowLimitMicros,'Cota da janela'],
          [typeof account.usage.monthlyLimitMicros==='number'&&typeof account.usage.monthlyUsedMicros==='number'?Math.max(0,account.usage.monthlyLimitMicros-account.usage.monthlyUsedMicros):null,'Saldo do ciclo'],
          [typeof account.usage.windowLimitMicros==='number'&&typeof account.usage.windowUsedMicros==='number'?Math.max(0,account.usage.windowLimitMicros-account.usage.windowUsedMicros):null,'Saldo da janela']].map(([v,k])=>
          typeof v==='number'&&<div key={String(k)}><dt>{k}</dt><dd>{usdMicros(v)}</dd></div>)}
      </dl>}
      {account?.usage?.windowEnd&&<p>Próxima liberação em {new Date(account.usage.windowEnd).toLocaleDateString('pt-BR')}.</p>}
      {account?.subscription?.periodEnd&&<p>Fim do ciclo em {new Date(account.subscription.periodEnd).toLocaleDateString('pt-BR')}.</p>}
      {account&&!summary&&<AccountServices key={user.id} account={account} reload={reload}/>}
      {!summary&&<>{account&&!accepted&&termsVersion&&privacyVersion&&<div>
        <p><button className="button" type="button" onClick={()=>open('terms')}>Ler os termos</button> <button className="button" type="button" onClick={()=>open('privacy')}>Ler a política de privacidade</button></p>
        <label><input type="checkbox" checked={terms} onChange={e=>setTerms(e.target.checked)}/> Li e aceito os termos e a política de privacidade.</label>
        <label><input type="checkbox" checked={marketing} onChange={e=>setMarketing(e.target.checked)}/> Quero receber novidades por e-mail.</label>
        <button className="button" type="button" disabled={!terms||pending} onClick={()=>void run(async()=>{
          await request('account.consent',{termsVersion,privacyVersion,marketing});await reload();
        })}>Registrar aceite</button></div>}
        <div className="inline-actions"><button type="button" className="button" disabled={pending} onClick={()=>open('account')}>Gerenciar conta e assinatura</button>
          <button type="button" className="button" disabled={pending} onClick={()=>open('plans')}>Ver planos</button>
          <button type="button" className="button" disabled={pending} onClick={()=>void run(async()=>{
            const result=await request<{revoked:boolean}>('account.signOut');window.dispatchEvent(new Event(accountChanged));
            if(!result.revoked)setMessage('Sessão removida deste dispositivo. A revogação remota não foi confirmada.');
          })}>Sair da conta</button></div></>}
    </>:<form onSubmit={submit}>
      <div className="inline-actions">{(['signIn','recover'] as const).map((v,i)=><button type="button" className="button" key={v} disabled={pending} aria-pressed={mode===v}
        onClick={()=>{setMode(v);setPassword('');setError('');setMessage('');}}>{['Entrar','Recuperar senha'][i]}</button>)}
        <button className="button" type="button" disabled={pending} onClick={()=>open('signup')}>Criar conta no site</button></div>
      {mode!=='recover'&&<label>E-mail<input type="email" required autoComplete="email" value={email} disabled={pending} onChange={e=>setEmail(e.target.value)}/></label>}
      {mode!=='recover'&&<label>Senha<input type="password" required autoComplete={mode==='signIn'?'current-password':'new-password'} value={password}
        disabled={pending} onChange={e=>setPassword(e.target.value)}/></label>}
      <div className="inline-actions"><button className="button primary" type="submit" disabled={pending}>{mode==='recover'?'Recuperar no site':'Entrar com e-mail'}</button>
        <button className="button" type="button" disabled={pending} onClick={()=>void authenticate('google')}>Continuar com Google</button>
        <button className="button" type="button" disabled={pending} onClick={()=>open('signin')}>Acessar conta no site</button></div>
    </form>}
    {!summary&&<button className="button" type="button" disabled={pending} onClick={()=>open('support')}>Suporte e relato de erro</button>}
    {pending&&<p role="status">Aguardando o serviço de conta…</p>}
    {error&&<p role="alert">{error}</p>}{message&&<p role="status">{message}</p>}
  </section>;
}
