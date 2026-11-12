import { useEffect, useRef, useState } from 'react';
import { Folder, Globe2, LayoutDashboard, Maximize2, Network, PanelRight, PanelLeft, Plus, Settings2, X } from 'lucide-react';
import { type Action, type Assessment, type Payload, type Project, type Snapshot, errorMessage } from '../types';
import Projects from './Projects';
import ProjectInsights from './ProjectInsights';
import { ProjectTree } from './ProjectRail';
import ProjectLaunch from './ProjectLaunch';
import ProjectConversation from './ProjectConversation';
import { pruneChatDrafts } from '../chat-draft';
import ProjectWelcome, { type StartKind } from './ProjectWelcome';

interface Props {
  snapshot:Snapshot; busy:boolean; action:Action; selected:string; onSelect:(id:string)=>void;
  onAdvanced:(id:string)=>void; onModels:()=>void; onFinding:(id:string)=>void;
}
export default function ProjectDesk({snapshot,busy,action,selected,onSelect,onAdvanced,onModels,onFinding}:Props) {
  const live=snapshot.projects.filter(p=>!p.archived);
  const activeIds=new Set(live.map(p=>p.id));
  const [projectId,setProjectId]=useState('');
  const [launch,setLaunch]=useState(false);
  const [startKind,setStartKind]=useState<StartKind>('code');
  const [panel,setPanel]=useState<'overview'|'settings'>();
  const [split,setSplit]=useState(false);
  const [tree,setTree]=useState(false);
  const [treeCollapsed,setTreeCollapsed]=useState(false);
  const [error,setError]=useState('');
  const [starting,setStarting]=useState(false);
  const working=useRef(false);
  const retry=useRef<{projectId:string;payload:Payload}|undefined>(undefined);
  const main=useRef<HTMLDivElement>(null);
  const mobileMenu=useRef<HTMLButtonElement>(null);
  const panelLauncher=useRef<HTMLButtonElement|null>(null);
  const panelClose=useRef<HTMLButtonElement>(null);
  const pendingFocus=useRef(false);
  const project=live.find(p=>p.id===(snapshot.assessments.find(a=>a.id===selected)?.projectId || projectId)) ?? live[0];
  const history=snapshot.assessments.filter(a=>a.projectId===project?.id && activeIds.has(a.projectId)).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt));
  const chat=history.find(a=>a.id===selected) ?? history[0];
  const ProjectIcon=project?.kind==='network'?Network:project?.kind==='web'?Globe2:Folder;
  useEffect(()=>{
    const projects=new Set(snapshot.projects.filter(p=>!p.archived).map(p=>p.id));
    pruneChatDrafts(new Set(snapshot.assessments.filter(a=>projects.has(a.projectId)).map(a=>a.id)));
  },[snapshot.projects,snapshot.assessments]);
  useEffect(()=>{
    if(!tree)return;
    document.querySelector<HTMLButtonElement>('#project-tree .tree-new')?.focus();
    const compact=window.matchMedia('(max-width:760px)');
    const onResize=()=>{if(!compact.matches)setTree(false);};
    compact.addEventListener('change',onResize);
    return ()=>compact.removeEventListener('change',onResize);
  },[tree]);
  useEffect(()=>{if(panel)panelClose.current?.focus({preventScroll:true});},[panel]);
  function closePanel(){
    setPanel(undefined);
    panelLauncher.current?.focus({preventScroll:true});
  }
  useEffect(()=>{if(!launch && chat && chat.id!==selected)onSelect(chat.id);},[launch,chat?.id,selected,onSelect]);
  useEffect(()=>{if(pendingFocus.current){main.current?.focus();pendingFocus.current=false;}},[launch,project?.id,chat?.id]);
  function openChat(id:string){pendingFocus.current=true;onSelect(id);setLaunch(false);setPanel(undefined);setTree(false);setError('');}
  function openProject(id:string){setProjectId(id);openChat(snapshot.assessments.filter(a=>a.projectId===id).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt))[0]?.id??'');}
  function showPanel(value:'overview'|'settings',trigger:HTMLButtonElement){panelLauncher.current=trigger;setPanel(value);setSplit(false);setTree(false);}
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
  function newProject(kind:StartKind='code'){pendingFocus.current=true;setStartKind(kind);setLaunch(true);setPanel(undefined);setTree(false);setError('');}
  function dismissTree(){setTree(false);mobileMenu.current?.focus({preventScroll:true});}
  const showTree=live.length>0;
  return <section className={'project-desk'+(showTree?' with-tree':'')+(tree?' tree-open':'')+(treeCollapsed?' tree-collapsed':'')} aria-label="Área de projetos"
    onKeyDown={event=>{if(event.key==='Escape'&&tree&&!event.defaultPrevented){event.preventDefault();dismissTree();}}}>
    {showTree && <ProjectTree projects={live} assessments={snapshot.assessments} projectId={launch?undefined:project?.id} assessmentId={launch?undefined:chat?.id} busy={busy||starting} onNew={()=>newProject()} onChat={openChat} onProject={openProject} onNewChat={p=>void newChat(p)}/>}
    {tree&&<button className="tree-dismiss" aria-label="Fechar lista de projetos" tabIndex={-1} onClick={dismissTree}/>}
    <div className="desk-main" ref={main} tabIndex={-1}>
      {showTree&&<button ref={mobileMenu} type="button" className="tree-mobile-toggle" aria-controls="project-tree" aria-expanded={tree} onClick={()=>setTree(!tree)}><PanelLeft size={16}/>Projetos</button>}
      {error&&<p className="app-alert" role="alert">{error}</p>}
      {launch?<ProjectLaunch snapshot={snapshot} action={action} busy={busy} initialKind={startKind} onModels={onModels} onCancel={()=>setLaunch(false)} onCreated={(id,assessment)=>{setProjectId(id);openChat(assessment);}}/>
      :!project?<ProjectWelcome onStart={newProject}/>
      :<>
        <header className="desk-bar"><div className="desk-location">
          <button className="desk-tree-toggle" aria-controls="project-tree" aria-expanded={!treeCollapsed} aria-label={treeCollapsed?'Mostrar lista de projetos':'Recolher lista de projetos'} title={treeCollapsed?'Mostrar lista de projetos':'Recolher lista de projetos'} onClick={()=>setTreeCollapsed(!treeCollapsed)}><PanelLeft size={17}/></button>
          <ProjectIcon className="desk-location-icon" size={17}/><div><h1 title={project.name}>{project.name}</h1>{chat&&<small title={chat.title}>{chat.title}</small>}</div></div>
          <div className="desk-actions"><button title="Visão geral do projeto" aria-label="Visão geral do projeto" aria-expanded={panel==='overview'} onClick={e=>showPanel('overview',e.currentTarget)}><LayoutDashboard size={17}/><span>Visão geral</span></button><button title="Configurações do projeto" aria-label="Configurações do projeto" aria-expanded={panel==='settings'} onClick={e=>showPanel('settings',e.currentTarget)}><Settings2 size={17}/></button></div>
        </header>
        <div className={'desk-content'+(panel&&split?' desk-split':'')}>
          <div className="desk-conversation" hidden={!!panel&&!split}>
            {chat?<ProjectConversation key={chat.id} assessment={chat} plans={snapshot.plans?.filter(plan=>!snapshot.workPlanBindings?.some(binding=>binding.executionPlanId===plan.id))} settings={snapshot.settings} action={action} busy={busy} onAdvanced={()=>onAdvanced(chat.id)} onModels={onModels}/>
            :<div className="desk-welcome"><h2>Comece uma conversa</h2><button className="button primary" disabled={starting||busy} onClick={()=>void newChat(project)}><Plus size={16}/>Nova conversa</button></div>}
          </div>
          {panel&&<aside className="desk-panel" aria-label={panel==='settings'?'Configurações do projeto':'Visão geral do projeto'} onKeyDown={event=>{
            if(event.key==='Escape'&&!event.defaultPrevented&&!event.nativeEvent.isComposing&&event.target instanceof Element&&!event.target.closest('[role="dialog"],dialog,select,[aria-expanded="true"]')){event.stopPropagation();closePanel();}
          }}>
            <div className="desk-panel-head"><strong>{panel==='settings'?'Configurações do projeto':'Visão geral do projeto'}</strong><div><button className="icon-button" title={split?'Ampliar painel':'Mostrar ao lado da conversa'} aria-label={split?'Ampliar painel':'Mostrar ao lado da conversa'} onClick={()=>setSplit(!split)}>{split?<Maximize2 size={16}/>:<PanelRight size={16}/>}</button><button ref={panelClose} className="icon-button" aria-label="Fechar painel" onClick={closePanel}><X size={17}/></button></div></div>
            <Projects key={project.id+panel} panel={panel} initialProjectId={project.id} projects={snapshot.projects} assessments={snapshot.assessments} action={action} busy={busy} onNewAssessment={()=>void newChat(project)} onOpenAssessment={openChat} onArchived={()=>{onSelect('');setProjectId('');setPanel(undefined);}} insights={<ProjectInsights snapshot={snapshot} projectId={project.id} onFinding={onFinding}/>}/>
          </aside>}
        </div>
      </>}
    </div>
  </section>;
}
