import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertCircle, Laptop, LoaderCircle, Plus, RefreshCw, Settings as SettingsIcon, UserRound, Wrench, X } from 'lucide-react';
import { native, request } from './api';
import { budgetChanged } from './ai-budget';
import { dataChanged } from './data-policy';
import { ProjectionNotice } from './components/DataAccess';
import { scopeChanged } from './components/ScopeAccess';
import { Action, Assessment, Environment, Payload, Snapshot, View, errorMessage } from './types';
import Assessments from './components/Assessments';
import Findings from './components/Findings';
import Projects from './components/Projects';
import Kali from './components/Kali';
import Settings from './components/Settings';
import { ProjectPurgeStatus } from './components/ProjectPurge';
import { Brand } from './components/shared';

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [environment, setEnvironment] = useState<Environment>({ status:'unavailable', distro:'OpenKoda-Kali', message:'Ambiente ainda não consultado.', tools:[] });
  const [view, setView] = useState<View>('assessments');
  const [selected, setSelected] = useState('');
  const [selectedFinding,setSelectedFinding]=useState('');
  const [creating, setCreating] = useState(false);
  const [initialProject, setInitialProject] = useState<string>();
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const runningRequests = useRef(new Set<string>());
  const [runningCount,setRunningCount] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [purgeStatus,setPurgeStatus]=useState<ProjectPurgeStatus>();const [purgeError,setPurgeError]=useState('');const [settingsDataRequest,setSettingsDataRequest]=useState(0);
  const refreshPurge=useCallback(async()=>{if(!native)return;try{setPurgeStatus(await request<ProjectPurgeStatus>('projectPurge.status'));setPurgeError('');}catch(e){setPurgeError(errorMessage(e));}},[]);
  const dataAssessmentId=view==='assessments'&&!creating?selected:view==='findings'?snapshot?.findings.find(f=>f.id===selectedFinding)?.assessmentId:undefined;
  useEffect(()=>{if(native)void request('data.session.select',{assessmentId:dataAssessmentId||null}).catch(e=>setError(errorMessage(e)));},[dataAssessmentId]);
  function openDataSettings(){setSettingsDataRequest(value=>value+1);setView('settings');}

  const refresh = useCallback(async () => {
    const next = await request<Snapshot>('snapshot');
    setSnapshot(next);
  }, []);
  const refreshEnvironment = useCallback(async () => {
    const next = await request<Environment>('environment.status');
    setEnvironment(next);
  }, []);
  const load = useCallback(async () => {
    setLoading(true); setError('');
    try { await refresh(); await refreshEnvironment(); await refreshPurge(); }
    catch (e) { setError(errorMessage(e)); }
    finally { setLoading(false); }
  }, [refresh, refreshEnvironment,refreshPurge]);
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
  useEffect(() => {
    const theme = snapshot?.settings.theme ?? 'light';
    document.documentElement.dataset.theme = theme;
  }, [snapshot?.settings.theme]);

  const action: Action = async <T,>(name: string, payload: Payload = {}): Promise<T> => {
    if(['plan.run','assessment.cancel','kali.mcp.diagnose','kali.mcp.hashEvidence','kali.mcp.scanEvidence','kali.mcp.cancel','ai.investigation.run','ai.investigation.cancel'].includes(name)) {
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
      if(['settings.save','ai.key.save','ai.key.remove','ai.budget.configure','ai.budget.resolveUnknown'].includes(name))budgetChanged();
      if(name.startsWith('data.')||name.startsWith('credential.reference.')||['settings.save','ai.key.save','ai.key.remove'].includes(name))dataChanged();
      await refresh();
      // environment.status probes Linux and would restart a just-stopped backup source.
      if ((name.startsWith('environment.') && !name.startsWith('environment.archive.')) || name.startsWith('packages.')) {
        await refreshEnvironment();
      }
      return result;
    } catch (e) { await refresh().catch(()=>{});setError(errorMessage(e)); throw e; }
    finally { if(name.startsWith('projectPurge.'))await refreshPurge();busyRef.current = false; setBusy(false); if(name!=='snapshot')scopeChanged(); }
  };
  function newAssessment(projectId?: string) { setInitialProject(projectId); setCreating(true); setView('assessments'); }
  function openFinding(id=''){setSelectedFinding(id);setView('findings');}
  function openAssessment(id: string) { setSelected(id); setCreating(false); setView('assessments'); }
  async function createAssessment(payload: Payload, _start: boolean) {
    const assessment = await action<Assessment>('assessment.create', payload);
    openAssessment(assessment.id);
  }
  const envLabel = environment.status === 'ready' ? environment.executorReady ? 'Kali pronto para executar' : 'Kali preparado' : environment.status === 'degraded' ? 'Kali requer atenção' : 'Kali não conectado';

  return <div className="app-shell">
    <a className="skip-link" href="#main-content">Ir para o conteúdo</a>
    <aside className="rail" aria-label="Navegação secundária">
      <Brand/>
      <button className={`rail-button ${['assessments','findings','projects'].includes(view)?'active':''}`} title="Área de trabalho" aria-label="Área de trabalho" onClick={()=>setView('assessments')}><Laptop size={23}/></button>
      <button className={`rail-button ${view==='kali'?'active':''}`} title="Kali e ferramentas" aria-label="Kali e ferramentas" onClick={()=>setView('kali')}><Wrench size={23}/></button>
      <div className="rail-spacer"/>
      <button className={`rail-button ${view==='settings'?'active':''}`} title="Configurações" aria-label="Configurações" onClick={()=>setView('settings')}><SettingsIcon size={23}/></button>
      <span className="local-profile" title="Perfil local" aria-label="Perfil local"><UserRound size={22}/></span>
    </aside>
    <header className="topbar"><span className="wordmark">OpenKoda</span><nav aria-label="Navegação principal">{([['assessments','Avaliações'],['findings','Achados'],['projects','Projetos']] as const).map(([id,label])=><button key={id} className={view===id?'active':''} aria-current={view===id?'page':undefined} onClick={()=>setView(id)}>{label}</button>)}</nav><div className="topbar-actions"><button className={`environment-indicator env-${environment.status}`} onClick={()=>setView('kali')} title={environment.message}><span className="status-dot"/>{envLabel}</button><button className="button primary" onClick={()=>newAssessment()} disabled={!snapshot||busy}><Plus size={19}/>Nova avaliação</button></div></header>
    <main id="main-content" tabIndex={-1} className="main-content" aria-busy={loading||busy}>
      {snapshot&&<ProjectionNotice projection={snapshot.projection}/>}
      {!native&&<div className="preview-banner">Prévia no navegador · Dados locais de demonstração</div>}
      {error&&<div className="app-alert" role="alert"><AlertCircle size={20}/><span>{error}</span><button className="icon-button" title="Fechar aviso" aria-label="Fechar aviso" onClick={()=>setError('')}><X size={18}/></button></div>}
      {notice&&<div className="app-notice" role="status"><span>{notice}</span><button className="icon-button" title="Fechar confirmação" aria-label="Fechar confirmação" onClick={()=>setNotice('')}><X size={18}/></button></div>}
      {native&&(purgeError||purgeStatus?.hasUnresolved||purgeStatus?.hasPartialHistory)&&<div className="app-alert project-purge-banner" role="alert"><AlertCircle size={20}/><span>{purgeError?`Não foi possível consultar exclusões de projetos: ${purgeError}`:purgeStatus?.hasUnresolved?'Exclusão de projeto pendente: novas alterações estão bloqueadas. O histórico pode estar parcialmente removido.':'Há exclusão encerrada parcialmente. A trilha do projeto pode estar incompleta; os dados restantes foram preservados.'}</span><button className="button" type="button" onClick={openDataSettings}>Revisar exclusão em Dados</button></div>}
      {loading&&!snapshot?<div className="loading-state" role="status"><LoaderCircle className="spin" size={26}/>Carregando seu trabalho…</div>:!snapshot?<div className="loading-state"><h1>Não foi possível abrir seus dados</h1><p>Os dados existentes foram preservados.</p><button className="button" onClick={()=>void load()}><RefreshCw size={17}/>Tentar novamente</button></div>:<>
        {view==='assessments'&&<Assessments {...snapshot} selected={selected} onSelect={id=>{setSelected(id);setCreating(false);}} creating={creating} initialProject={initialProject} onCancelNew={()=>setCreating(false)} onCreate={createAssessment} onProjects={()=>setView('projects')} onFindings={openFinding} action={action} busy={busy} onExport={path=>setNotice(`Relatório exportado para ${path}`)}/>}
        {view==='projects'&&<Projects projects={snapshot.projects} assessments={snapshot.assessments} action={action} busy={busy} onNewAssessment={newAssessment} onOpenAssessment={openAssessment}/>}
        {view==='findings'&&<Findings selected={selectedFinding} onSelect={setSelectedFinding} onOpenAssessment={openAssessment} findings={snapshot.findings} assessments={snapshot.assessments} projects={snapshot.projects} validations={snapshot.validations} action={action} busy={busy}/>}
        {view==='kali'&&<Kali environment={environment} action={action} busy={busy}/>}
        {view==='settings'&&<Settings settings={snapshot.settings} projects={snapshot.projects} action={action} busy={busy} dataRequest={settingsDataRequest} onPurgeStatus={value=>{setPurgeStatus(value);setPurgeError('');}}/>}
      </>}
    </main>
    <footer className="statusbar"><span><span className={`status-dot ${busy||runningCount?'working':''}`}/>{busy?'Atualizando…':runningCount?'Execução em andamento':snapshot?'Dados locais disponíveis':'Armazenamento não carregado'}</span><span>{native?'Aplicativo desktop · Dados neste dispositivo':'Prévia isolada · Sem execução de ferramentas'}</span></footer>
  </div>;
}
