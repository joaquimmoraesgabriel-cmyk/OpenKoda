import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { native, request } from './api';
import { accountChanged, type AccountStatus } from './commercial';
import { errorMessage } from './types';
import { clearDraftSkills } from './chat-draft';

const localChoiceKey = 'openkoda.local-workspace.v1';
const signedOut: AccountStatus = {state:'signedOut', user:null, account:null};
type Session = AccountStatus & {loading:boolean;error:string;local:boolean;refresh:()=>Promise<void>;continueLocally:()=>void};
const Context = createContext<Session | null>(null);

export function DesktopSessionProvider({children}:{children:ReactNode}) {
  const [status,setStatus]=useState<AccountStatus>(signedOut),[loading,setLoading]=useState(native),[error,setError]=useState('');
  const [local,setLocal]=useState(()=>{try{return !native || localStorage.getItem(localChoiceKey)==='true';}catch{return !native;}});
  const revision=useRef(0),mounted=useRef(true);
  const lastChecked=useRef(0);
  const refresh=useCallback(async()=>{
    if(!native)return;
    const stamp=++revision.current;lastChecked.current=Date.now();setLoading(true);setError('');
    // An account change never leaves another account's plan or consent on screen.
    setStatus(previous=>({...previous,account:null}));
    try {
      const next=await request<AccountStatus>('account.status');
      if(!next || !['signedOut','authenticated','offline','expired'].includes(next.state))throw new Error('Não foi possível verificar a sessão. Tente novamente.');
      if(mounted.current&&stamp===revision.current)setStatus(next);
    } catch(cause) {
      if(mounted.current&&stamp===revision.current)setError(errorMessage(cause));
    } finally {if(mounted.current&&stamp===revision.current)setLoading(false);}
  },[]);
  useEffect(()=>{
    mounted.current=true;void refresh();
    const changed=()=>{clearDraftSkills();setStatus(signedOut);void refresh();};
    const resumed=()=>{if(document.visibilityState==='visible'&&Date.now()-lastChecked.current>30_000)void refresh();};
    window.addEventListener(accountChanged,changed);
    window.addEventListener('focus',resumed);
    return()=>{mounted.current=false;revision.current++;window.removeEventListener(accountChanged,changed);window.removeEventListener('focus',resumed);};
  },[refresh]);
  useEffect(()=>{
    if(!loading&&status.account?.entitlements?.skills!==true)clearDraftSkills();
  },[loading,status.account?.entitlements?.skills]);
  useEffect(()=>{
    const end=status.account?.simulation?.status==='active'?Date.parse(status.account.simulation.periodEnd):NaN;
    if(!Number.isFinite(end))return;
    const remaining=end-Date.now();if(remaining<=0)return;
    const timer=setTimeout(()=>void refresh(),Math.min(remaining+50,2147483647));
    return()=>clearTimeout(timer);
  },[status.account?.simulation?.periodEnd,status.account?.simulation?.status,refresh]);
  const continueLocally=()=>{
    try{localStorage.setItem(localChoiceKey,'true');}catch{/* The current local session still works when WebView storage is unavailable. */}
    setLocal(true);
  };
  return <Context.Provider value={{...status,loading,error,local,refresh,continueLocally}}>{children}</Context.Provider>;
}

export function useDesktopSession(){
  const value=useContext(Context);
  if(!value)throw new Error('A sessão precisa ser carregada antes da área de trabalho.');
  return value;
}
