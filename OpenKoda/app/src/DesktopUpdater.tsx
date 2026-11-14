import { useEffect, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Download, RefreshCw, X } from 'lucide-react';
import { version } from '../package.json';
import './desktop-updater.css';

type UpdatePhase = 'idle'|'checking'|'available'|'current'|'downloading'|'ready'|'installing'|'error';
type UpdateStatus = { phase:UpdatePhase; currentVersion:string; version:string|null; notes:string|null; downloadedBytes:number; totalBytes:number|null; message:string; deferred:boolean; backupPath:string|null };
type Action = 'status'|'check'|'download'|'install'|'defer';
const native=typeof window!=='undefined'&&'__TAURI_INTERNALS__' in window;
let current:UpdateStatus = { phase:'idle',currentVersion:version,version:null,notes:null,downloadedBytes:0,totalBytes:null,message:'Verifique se há uma nova versão do aplicativo.',deferred:false,backupPath:null };
const subscribers = new Set<() => void>();
let poll:ReturnType<typeof setInterval>|undefined;
let started=false;
let actionPending=false;
function notify(next:UpdateStatus) { if(!next||typeof next.phase!=='string'||typeof next.currentVersion!=='string')return; current=next; for(const fn of subscribers) fn(); }
function subscribe(fn:() => void) {
  subscribers.add(fn);
  if(native&&!poll) poll=setInterval(()=>{ if(['checking','downloading','installing'].includes(current.phase)) void act('status'); },700);
  return ()=>{ subscribers.delete(fn); if(!subscribers.size&&poll){ clearInterval(poll); poll=undefined; } };
}
async function act(action:Action) {
  if(!native||(actionPending&&action!=='status'&&action!=='defer'))return;
  const operation=action!=='status'&&action!=='defer';
  if(operation)actionPending=true;
  if(action==='check')notify({...current,phase:'checking',message:'Consultando atualizações…'});
  if(action==='download')notify({...current,phase:'downloading',downloadedBytes:0,totalBytes:null,deferred:false,message:'Baixando a atualização. Você pode continuar trabalhando.'});
  if(action==='install')notify({...current,phase:'installing',message:'Verificando operações e criando a cópia de segurança…'});
  try { notify(await invoke<UpdateStatus>('desktop_update',{ input:{action} })); }
  catch(error) { notify({...current,phase:current.phase==='installing'?'ready':current.phase==='downloading'?'available':current.phase==='checking'?'error':current.phase,message:String(error)}); }
  finally { if(operation)actionPending=false; }
}
function useUpdate() {
  const value=useSyncExternalStore(subscribe,()=>current);
  useEffect(()=>{ if(native&&!started){ started=true; void act('check'); } },[]);
  return value;
}
function Progress({state}:{state:UpdateStatus}) {
  const percent=state.totalBytes?Math.min(100,Math.round(state.downloadedBytes/state.totalBytes*100)):undefined;
  return <div className="desktop-update-progress"><progress aria-label="Download da atualização" max={100} value={percent}/><span>{percent===undefined?`${(state.downloadedBytes/1048576).toFixed(1)} MB recebidos`:`${percent}% · ${(state.downloadedBytes/1048576).toFixed(1)} MB de ${((state.totalBytes||0)/1048576).toFixed(1)} MB`}</span></div>;
}
function Controls({state}:{state:UpdateStatus}) {
  const busy=['checking','downloading','installing'].includes(state.phase);
  return <div className="desktop-update-actions">
    {state.phase==='available'&&<button type="button" onClick={()=>void act('download')}><Download size={15}/> Baixar atualização</button>}
    {state.phase==='ready'&&<button type="button" onClick={()=>void act('install')}><RefreshCw size={15}/> Instalar e reiniciar</button>}
    {!['available','ready'].includes(state.phase)&&<button type="button" disabled={busy} onClick={()=>void act('check')}><RefreshCw size={15} className={state.phase==='checking'?'desktop-update-spin':undefined}/>{state.phase==='checking'?'Verificando…':state.phase==='installing'?'Preparando instalação…':'Verificar atualizações'}</button>}
  </div>;
}
export function UpdateSettings() {
  const state=useUpdate();
  return <section className="desktop-update-settings" aria-label="Atualizações do aplicativo">
    <div className="desktop-update-heading"><div><h3>Atualizações</h3><p>OpenKoda {state.currentVersion}</p></div><span className="desktop-update-channel">Desktop · Windows</span></div>
    {native?<>
      <p role="status">{state.message}</p>
      {state.version&&<p className="desktop-update-version">Versão disponível: {state.version}</p>}
      {state.phase==='downloading'&&<Progress state={state}/>}
      {state.notes&&<details className="desktop-update-notes"><summary>O que mudou nesta versão</summary><p>{state.notes}</p></details>}
      <Controls state={state}/>
      <p className="desktop-update-caption">A atualização é verificada antes de instalar. O OpenKoda cria uma cópia local dos bancos de dados e preserva projetos, conversas e preferências. Termine as respostas e análises em andamento antes de reiniciar.</p>
      {state.backupPath&&<p className="desktop-update-caption">Cópia de segurança: <code>{state.backupPath}</code></p>}
    </>:<p>As atualizações são instaladas pelo aplicativo desktop. Esta demonstração não instala programas no computador.</p>}
  </section>;
}
export function UpdateNotice({onOpenSettings}:{onOpenSettings?:()=>void}) {
  const state=useUpdate();
  if(!native||state.deferred||!['available','downloading','ready','installing'].includes(state.phase))return null;
  return <aside className="desktop-update-notice" aria-label="Atualização disponível">
    <div><strong>{state.phase==='ready'?'Atualização pronta para instalar':`OpenKoda ${state.version??''}`}</strong><p role="status">{state.message}</p>{state.phase==='downloading'&&<Progress state={state}/>}</div>
    <div className="desktop-update-notice-actions"><Controls state={state}/>{onOpenSettings&&<button type="button" className="desktop-update-link" onClick={onOpenSettings}>Detalhes</button>}
      {state.phase!=='installing'&&<button type="button" className="desktop-update-dismiss" aria-label="Adiar atualização" title="Mais tarde" onClick={()=>void act('defer')}><X size={16}/></button>}
    </div>
  </aside>;
}
