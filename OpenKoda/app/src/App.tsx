import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertCircle, LoaderCircle, RefreshCw, X } from 'lucide-react';
import { native, request } from './api';
import { budgetChanged } from './ai-budget';
import { dataChanged } from './data-policy';
import { ProjectionNotice } from './components/DataAccess';
import { scopeChanged } from './components/ScopeAccess';
import { Action, Payload, Snapshot, View, errorMessage } from './types';
import CloseWorkPrompt, { type CloseActivity } from './components/CloseWorkPrompt';
import Findings from './components/Findings';
import Settings from './components/Settings';
import { ProjectPurgeStatus } from './components/ProjectPurge';
import WorkspaceProfile from './components/WorkspaceProfile';
import ProjectDesk from './components/ProjectDesk';
import { Rail } from './components/ProjectRail';
import { useApplyAppearance } from './appearance';
import './workspace.css';
import './project-desk.css';
import './codex-workspace.css';

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [view, setView] = useState<View>('overview');
  const [modelsRequest,setModelsRequest]=useState(false);
  useEffect(()=>{const url=new URL(window.location.href);if(url.searchParams.has('area')){url.searchParams.delete('area');window.history.replaceState(null,'',url);}},[]);
  const main = useRef<HTMLElement>(null);
  const [selected, setSelected] = useState('');
  const [selectedFinding,setSelectedFinding]=useState('');
  const pageKey = view;
  const previousPage = useRef(pageKey);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const runningRequests = useRef(new Set<string>());
  const [runningCount,setRunningCount] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [purgeStatus,setPurgeStatus]=useState<ProjectPurgeStatus>();const [purgeError,setPurgeError]=useState('');const [settingsDataRequest,setSettingsDataRequest]=useState(0);
  const refreshPurge=useCallback(async()=>{if(!native)return;try{setPurgeStatus(await request<ProjectPurgeStatus>('projectPurge.status'));setPurgeError('');}catch(e){setPurgeError(errorMessage(e));}},[]);
  const dataAssessmentId=view==='overview'?selected:view==='findings'?snapshot?.findings.find(f=>f.id===selectedFinding)?.assessmentId:undefined;
  useEffect(()=>{if(native)void request('data.session.select',{assessmentId:dataAssessmentId||null}).catch(e=>setError(errorMessage(e)));},[dataAssessmentId]);
  const [settingsUsageRequest, setSettingsUsageRequest] = useState(0);
  function openDataSettings(){setSettingsUsageRequest(0);setSettingsDataRequest(value=>value+1);setView('settings');}
  function openUsageHistory(){setSettingsDataRequest(0);setSettingsUsageRequest(value=>value+1);setView('settings');}

  const refresh = useCallback(async () => {
    const next = await request<Snapshot>('snapshot');
    setSnapshot(next);
  }, []);
  useEffect(() => {
    const update = () => { void refresh().catch(cause => setError(errorMessage(cause))); };
    window.addEventListener('openkoda:security-scan-updated', update);
    return () => window.removeEventListener('openkoda:security-scan-updated', update);
  }, [refresh]);
  const load = useCallback(async () => {
    setLoading(true); setError('');
    try { await refresh(); await refreshPurge(); }
    catch (e) { setError(errorMessage(e)); }
    finally { setLoading(false); }
  }, [refresh,refreshPurge]);
  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    const active = runningCount>0 || snapshot?.assessments.some(a=>['preparing','running','canceling','pausing','queued'].includes(a.status));
    if(!active)return;
    let disposed=false;
    let timer:ReturnType<typeof setTimeout>;
    async function poll(){try{await refresh();}catch(e){if(!disposed)setError(errorMessage(e));}finally{if(!disposed)timer=setTimeout(()=>void poll(),1000);}}
    timer=setTimeout(()=>void poll(),1000);
    return()=>{disposed=true;clearTimeout(timer);};
  },[runningCount,snapshot?.assessments.some(a=>['preparing','running','canceling','pausing','queued'].includes(a.status)),refresh]);
  useApplyAppearance(snapshot?.settings.theme);
  useEffect(() => {
    if (previousPage.current === pageKey) return;
    previousPage.current = pageKey;
    const content = main.current;
    if (!content) return;
    content.scrollTo(0, 0);
    content.querySelector('.detail-pane')?.scrollTo(0, 0);
    // Preserve autofocus in the newly opened form and the initial skip-link order.
    if (!content.contains(document.activeElement)) content.focus({ preventScroll: true });
  }, [pageKey]);

  const action: Action = async <T,>(name: string, payload: Payload = {}): Promise<T> => {
    if(name==='ai.ask') {
      const id=name+':'+String(payload.id??payload.assessmentId);
      if(runningRequests.current.has(id))throw new Error('Esta execução já está em andamento.');
      runningRequests.current.add(id);setRunningCount(runningRequests.current.size);setError('');setNotice('');
      try{const result=await request<T>(name,payload);await refresh();return result;}
      catch(e){await refresh().catch(()=>{});setError(errorMessage(e));throw e;}
      finally{runningRequests.current.delete(id);setRunningCount(runningRequests.current.size);scopeChanged();}
    }
    if (busyRef.current) throw new Error('Aguarde a ação em andamento.');
    busyRef.current = true; setBusy(true); setError(''); setNotice('');
    try {
      const result = await request<T>(name, payload);
      const connectionChanged = ['settings.save','ai.provider.configure','ai.key.save','ai.key.remove'].includes(name);
      if(connectionChanged || ['ai.budget.configure','ai.budget.resolveUnknown'].includes(name))budgetChanged();
      if(connectionChanged || name.startsWith('data.') || name.startsWith('credential.reference.'))dataChanged();
      await refresh();
      return result;
    } catch (e) { await refresh().catch(()=>{});setError(errorMessage(e)); throw e; }
    finally { if(name.startsWith('projectPurge.'))await refreshPurge();busyRef.current = false; setBusy(false); if(name!=='snapshot')scopeChanged(); }
  };
  function openFinding(id=''){setSelectedFinding(id);setView('findings');}
  function openAssessment(id: string) {
    const assessment=snapshot?.assessments.find(item=>item.id===id);
    if(assessment&&!['code','files'].includes(assessment.kind)){
      setError('Esta avaliação pertence ao motor anterior. Seus registros permanecem no histórico; a edição atual abre conversas de código local.');
      return;
    }
    setSelected(id); setView('overview');
  }
  function navigate(next: View) {
    if (next === 'settings') { setSettingsDataRequest(0); setSettingsUsageRequest(0); setModelsRequest(false); }
    setView(['projects','assessments','kali'].includes(next)?'overview':next);
  }
  function reviewCloseActivity(work:CloseActivity) {
    if(work.area==='settings'){ setView('settings'); return; }
    if(work.area==='current')return;
    const id=work.assessmentId || snapshot?.assessments.find(a=>['running','preparing','pausing','canceling'].includes(a.status))?.id || selected;
    setView('overview');
    if(id)setSelected(id);
    void refresh().catch(e=>setError(errorMessage(e)));
  }

  return <div className="app-shell workspace-shell project-shell">
    <CloseWorkPrompt onReview={reviewCloseActivity}/>
    <a className="skip-link" href="#main-content">Ir para o conteúdo</a>
    <Rail sourceOnly view={view} navigate={navigate}/>
    <main ref={main} id="main-content" tabIndex={-1} className="main-content workspace-main" aria-busy={loading||busy}>
      {snapshot&&<div className="workspace-projection"><ProjectionNotice projection={snapshot.projection}/></div>}
      {!native&&<div className="preview-banner">Prévia no navegador · Dados locais de demonstração</div>}
      {error&&<div className="app-alert" role="alert"><AlertCircle size={20}/><span>{error}</span><button className="icon-button" title="Fechar aviso" aria-label="Fechar aviso" onClick={()=>setError('')}><X size={18}/></button></div>}
      {notice&&<div className="app-notice" role="status"><span>{notice}</span><button className="icon-button" title="Fechar confirmação" aria-label="Fechar confirmação" onClick={()=>setNotice('')}><X size={18}/></button></div>}
      {native&&(purgeError||purgeStatus?.hasUnresolved||purgeStatus?.hasPartialHistory)&&<div className="app-alert project-purge-banner" role="alert"><AlertCircle size={20}/><span>{purgeError?`Não foi possível consultar exclusões de projetos: ${purgeError}`:purgeStatus?.hasUnresolved?'Exclusão de projeto pendente: novas alterações estão bloqueadas. O histórico pode estar parcialmente removido.':'Há exclusão encerrada parcialmente. A trilha do projeto pode estar incompleta; os dados restantes foram preservados.'}</span><button className="button" type="button" onClick={openDataSettings}>Revisar exclusão em Dados</button></div>}
      {loading&&!snapshot?<div className="loading-state" role="status"><LoaderCircle className="spin" size={26}/>Carregando seu trabalho…</div>:!snapshot?<div className="loading-state"><h1>Não foi possível abrir seus dados</h1><p>Os dados existentes foram preservados.</p><button className="button" onClick={()=>void load()}><RefreshCw size={17}/>Tentar novamente</button></div>:<>
        <div className="desk-host" hidden={view!=='overview'}><ProjectDesk sourceOnly snapshot={snapshot} busy={busy} action={action} selected={selected} onSelect={setSelected} onAdvanced={openAssessment} onModels={()=>{navigate('settings');setModelsRequest(true);}} onFinding={openFinding}/></div>
        {view==='profile'&&<WorkspaceProfile snapshot={snapshot} onSettings={()=>navigate('settings')} onUsageHistory={openUsageHistory}/>}
        {view==='findings'&&<Findings sourceOnly selected={selectedFinding} onSelect={setSelectedFinding} onOpenAssessment={openAssessment} findings={snapshot.findings} assessments={snapshot.assessments} projects={snapshot.projects} validations={snapshot.validations} action={action} busy={busy}/>}
        {view==='settings'&&<Settings settings={snapshot.settings} projects={snapshot.projects} assessments={snapshot.assessments} action={action} busy={busy} dataRequest={settingsDataRequest} usageRequest={settingsUsageRequest} initialModels={modelsRequest} onOpenAssessment={openAssessment} onPurgeStatus={value=>{setPurgeStatus(value);setPurgeError('');}}/>}
      </>}
    </main>
  </div>;
}
