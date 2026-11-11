import { useEffect, useRef, useState } from 'react';
import { FolderOpen, LayoutDashboard, Maximize2, PanelRight, PanelLeft, Plus, Settings2, X } from 'lucide-react';
import { type Action, type Assessment, type Payload, type Project, type Snapshot, errorMessage } from '../types';
import Projects from './Projects';
import ProjectInsights from './ProjectInsights';
import { ProjectTree } from './ProjectRail';
import ProjectLaunch from './ProjectLaunch';
import ProjectConversation from './ProjectConversation';

interface Props {
  snapshot:Snapshot; busy:boolean; action:Action; selected:string; onSelect:(id:string)=>void;
  onAdvanced:(id:string)=>void; onModels:()=>void; onFinding:(id:string)=>void;
}
export default function ProjectDesk({snapshot,busy,action,selected,onSelect,onAdvanced,onModels,onFinding}:Props) {
  const live=snapshot.projects.filter(p=>!p.archived);
  const activeIds=new Set(live.map(p=>p.id));
  const [projectId,setProjectId]=useState('');
  const [launch,setLaunch]=useState(false);
  const [panel,setPanel]=useState<'overview'|'settings'>();
  const [split,setSplit]=useState(false);
  const [tree,setTree]=useState(false);
  const [error,setError]=useState('');
  const [starting,setStarting]=useState(false);
  const working=useRef(false);
  const retry=useRef<{projectId:string;payload:Payload}|undefined>(undefined);
  const main=useRef<HTMLDivElement>(null);
  const pendingFocus=useRef(false);
  const project=live.find(p=>p.id===(snapshot.assessments.find(a=>a.id===selected)?.projectId || projectId)) ?? live[0];
  const history=snapshot.assessments.filter(a=>a.projectId===project?.id && activeIds.has(a.projectId)).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt));
  const chat=history.find(a=>a.id===selected) ?? history[0];
  useEffect(()=>{if(!launch && chat && chat.id!==selected)onSelect(chat.id);},[launch,chat?.id,selected,onSelect]);
  useEffect(()=>{if(pendingFocus.current){main.current?.focus();pendingFocus.current=false;}},[launch,project?.id,chat?.id]);
  function openChat(id:string){pendingFocus.current=true;onSelect(id);setLaunch(false);setPanel(undefined);setTree(false);setError('');}
  function openProject(id:string){setProjectId(id);openChat(snapshot.assessments.filter(a=>a.projectId===id).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt))[0]?.id??'');}
  function showPanel(value:'overview'|'settings'){setPanel(value);setSplit(false);setTree(false);}
  async function newChat(p:Project){
    if(working.current)return;
    working.current=true;setStarting(true);setError('');
    try{
      let payload=retry.current?.projectId===p.id?retry.current.payload:undefined;
      if(!payload){
        const n=snapshot.assessments.filter(a=>a.projectId===p.id).length+1;
        const previous = p.id===project?.id && chat ? chat : snapshot.assessments
          .filter(item=>item.projectId===p.id)
          .sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt))[0];
        payload={projectId:p.id,sourceAssessmentId:previous?.id,title:'Conversa '+n,idempotencyKey:crypto.randomUUID()};
        retry.current={projectId:p.id,payload};
      }
      const a=await action<Assessment>('project.newConversation',payload);retry.current=undefined;setProjectId(p.id);openChat(a.id);
    }catch(e){setError(errorMessage(e));}finally{working.current=false;setStarting(false);}
  }
  function newProject(){pendingFocus.current=true;setLaunch(true);setPanel(undefined);setTree(false);setError('');}
  const showTree=live.length>0;
  return <section className={'project-desk'+(showTree?' with-tree':'')+(tree?' tree-open':'')} aria-label="Área de projetos">
    {showTree && <ProjectTree projects={live} assessments={snapshot.assessments} projectId={launch?undefined:project?.id} assessmentId={launch?undefined:chat?.id} busy={busy||starting} onNew={newProject} onChat={openChat} onProject={openProject} onNewChat={p=>void newChat(p)}/>}
    <div className="desk-main" ref={main} tabIndex={-1}>
      {showTree&&<button type="button" className="tree-mobile-toggle" aria-expanded={tree} onClick={()=>setTree(!tree)}><PanelLeft size={16}/>Projetos</button>}
      {error&&<p className="app-alert" role="alert">{error}</p>}
      {launch?<ProjectLaunch snapshot={snapshot} action={action} busy={busy} onModels={onModels} onCancel={()=>setLaunch(false)} onCreated={(id,assessment)=>{setProjectId(id);openChat(assessment);}}/>
      :!project?<div className="desk-welcome"><div className="desk-welcome-mark"><FolderOpen size={34} strokeWidth={1.3}/></div><h1>Seu primeiro projeto</h1><button className="button primary" onClick={newProject}><Plus size={17}/>Criar projeto</button></div>
      :<>
        <header className="desk-bar"><div><h1>{project.name}</h1></div>
          <div className="desk-actions"><button title="Visão geral do projeto" aria-label="Visão geral do projeto" onClick={()=>showPanel('overview')}><LayoutDashboard size={18}/><span>Visão geral</span></button><button title="Configurações do projeto" aria-label="Configurações do projeto" onClick={()=>showPanel('settings')}><Settings2 size={18}/></button></div>
        </header>
        <div className={'desk-content'+(panel&&split?' desk-split':'')}>
          <div className="desk-conversation" hidden={!!panel&&!split}>
            {chat?<ProjectConversation key={chat.id} assessment={chat} plans={snapshot.plans?.filter(plan=>!snapshot.workPlanBindings?.some(binding=>binding.executionPlanId===plan.id))} settings={snapshot.settings} action={action} busy={busy} onAdvanced={()=>onAdvanced(chat.id)} onModels={onModels}/>
            :<div className="desk-welcome"><h2>Comece uma conversa</h2><button className="button primary" disabled={starting||busy} onClick={()=>void newChat(project)}><Plus size={16}/>Nova conversa</button></div>}
          </div>
          {panel&&<aside className="desk-panel" aria-label={panel==='settings'?'Configurações do projeto':'Visão geral do projeto'}>
            <div className="desk-panel-head"><strong>{panel==='settings'?'Configurações do projeto':'Visão geral do projeto'}</strong><div><button className="icon-button" title={split?'Ampliar painel':'Mostrar ao lado da conversa'} aria-label={split?'Ampliar painel':'Mostrar ao lado da conversa'} onClick={()=>setSplit(!split)}>{split?<Maximize2 size={16}/>:<PanelRight size={16}/>}</button><button className="icon-button" aria-label="Fechar painel" onClick={()=>setPanel(undefined)}><X size={17}/></button></div></div>
            <Projects key={project.id+panel} panel={panel} initialProjectId={project.id} projects={snapshot.projects} assessments={snapshot.assessments} action={action} busy={busy} onNewAssessment={()=>void newChat(project)} onOpenAssessment={openChat} onArchived={()=>{onSelect('');setProjectId('');setPanel(undefined);}} insights={<ProjectInsights snapshot={snapshot} projectId={project.id} onFinding={onFinding}/>}/>
          </aside>}
        </div>
      </>}
    </div>
  </section>;
}
