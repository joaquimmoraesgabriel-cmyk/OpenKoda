import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { Check, Folder, Globe2, LayoutDashboard, Maximize2, Network, PanelRight, PanelLeft, Pencil, Plus, X } from 'lucide-react';
import { type Action, type Assessment, type Project, type Snapshot } from '../types';
import Projects from './Projects';
import ProjectInsights from './ProjectInsights';
import { ProjectTree } from './ProjectRail';
import ProjectLaunch from './ProjectLaunch';
import ProjectConversation from './ProjectConversation';
import { knownProvider, type AiSelection } from '../ai-provider';
import { bindChatDraft, pruneChatDrafts } from '../chat-draft';
import ProjectWelcome, { type StartKind } from './ProjectWelcome';

interface Props {
  sourceOnly?:boolean;
  snapshot:Snapshot; busy:boolean; action:Action; selected:string; onSelect:(id:string)=>void;
  onAdvanced:(id:string)=>void; onModels:()=>void; onFinding:(id:string)=>void;
}
export default function ProjectDesk({sourceOnly=false,snapshot,busy,action,selected,onSelect,onAdvanced,onModels,onFinding}:Props) {
  const live=snapshot.projects.filter(p=>!p.archived && (!sourceOnly || p.kind==='code' || p.kind==='files'));
  const activeIds=new Set(live.map(p=>p.id));
  const [projectId,setProjectId]=useState('');
  const [launch,setLaunch]=useState(false);
  const [startKind,setStartKind]=useState<StartKind>('code');
  const [panel,setPanel]=useState<'overview'|'settings'>();
  const [split,setSplit]=useState(false);
  const [tree,setTree]=useState(false);
  const [treeCollapsed,setTreeCollapsed]=useState(false);
  const desk=useRef<HTMLElement>(null);
  const drag=useRef<{start:number;width:number;scale:number}|undefined>(undefined);
  const [treeWidth,setTreeWidth]=useState(()=>{try{const n=Number(localStorage.getItem('openkoda.project-tree.width'));return Number.isFinite(n)&&n>=185&&n<=420?n:240;}catch{return 240;}});
  const [maximumWidth,setMaximumWidth]=useState(420);
  const [resizing,setResizing]=useState(false);
  useEffect(()=>{
    const el=desk.current;if(!el)return;
    const update=()=>{if(window.matchMedia('(max-width:760px)').matches)return;const max=Math.max(185,Math.min(420,el.clientWidth-360));setMaximumWidth(max);setTreeWidth(w=>Math.min(w,max));};
    update();const observer=new ResizeObserver(update);observer.observe(el);return()=>observer.disconnect();
  },[]);
  function resizeTree(width:number){const next=Math.round(Math.max(185,Math.min(maximumWidth,width)));setTreeWidth(next);return next;}
  function persistWidth(width:number){try{localStorage.setItem('openkoda.project-tree.width',String(width));}catch{/* Layout stays usable without storage. */}}

  const [error,setError]=useState('');
  const [editingTitle,setEditingTitle]=useState<{id:string;text:string}>();
  const renameButton=useRef<HTMLButtonElement>(null);
  type NewChat = {key:string; project:Project; source?:Assessment; selection:AiSelection|null; prepared?:Assessment};
  const [session,setSession]=useState<NewChat>();
  const creation=useRef<{key:string;promise:Promise<Assessment>}|undefined>(undefined);
  const main=useRef<HTMLDivElement>(null);
  const mobileMenu=useRef<HTMLButtonElement>(null);
  const panelLauncher=useRef<HTMLButtonElement|null>(null);
  const panelClose=useRef<HTMLButtonElement>(null);
  const panelOpen=panel!==undefined;
  const pendingFocus=useRef(false);
  const draft=session&&activeIds.has(session.project.id)?session:undefined;
  const project=draft?.project ?? live.find(p=>p.id===(snapshot.assessments.find(a=>a.id===selected)?.projectId || projectId)) ?? live[0];
  const history=snapshot.assessments.filter(a=>a.projectId===project?.id && activeIds.has(a.projectId)).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt));
  const saved=history.find(a=>a.id===selected) ?? history[0];
  const chat:Assessment|undefined=draft ? history.find(a=>a.id===draft.prepared?.id) ?? draft.prepared ?? {
    id:draft.key,projectId:draft.project.id,kind:draft.project.kind,target:draft.source?.target??draft.project.target,
    title:'',objective:'',status:'draft',events:[],evidence:[],coverage:[],summary:'',
    aiSelection:draft.selection,createdAt:'',updatedAt:'',
  } : saved;
  useEffect(()=>{window.dispatchEvent(new CustomEvent('openkoda:active-provider',{detail:chat?.aiSelection?.provider??snapshot.settings.provider}));},[chat?.id,chat?.aiSelection?.provider,snapshot.settings.provider]);
  const ProjectIcon=project?.kind==='network'?Network:project?.kind==='web'?Globe2:Folder;
  useEffect(()=>{
    const projects=new Set(snapshot.projects.filter(p=>!p.archived).map(p=>p.id));
    const keys=new Set(snapshot.assessments.filter(a=>projects.has(a.projectId)).map(a=>a.id));
    if(draft)keys.add(draft.key);
    pruneChatDrafts(keys);
  },[snapshot.projects,snapshot.assessments,draft?.key]);
  useEffect(()=>{
    if(!tree)return;
    document.querySelector<HTMLButtonElement>('#project-tree .tree-new')?.focus();
    const compact=window.matchMedia('(max-width:760px)');
    const onResize=()=>{if(!compact.matches)setTree(false);};
    compact.addEventListener('change',onResize);
    return ()=>compact.removeEventListener('change',onResize);
  },[tree]);
  useEffect(()=>{if(panelOpen)panelClose.current?.focus({preventScroll:true});},[panelOpen]);
  function closePanel(){
    setPanel(undefined);
    panelLauncher.current?.focus({preventScroll:true});
  }
  useEffect(()=>{if(!draft && !launch && chat && chat.id!==selected)onSelect(chat.id);},[draft,launch,chat?.id,selected,onSelect]);
  useEffect(()=>{if(pendingFocus.current){main.current?.focus();pendingFocus.current=false;}},[launch,project?.id,chat?.id,panelOpen]);
  function openChat(id:string){setSession(undefined);pendingFocus.current=true;onSelect(id);setLaunch(false);setPanel(undefined);setTree(false);setError('');}
  function openProject(id:string){setProjectId(id);openChat(snapshot.assessments.filter(a=>a.projectId===id).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt))[0]?.id??'');}
  function togglePanel(){
    if(panelOpen){closePanel();return;}
    setPanel('overview');setSplit(false);setTree(false);
  }
  function newChat(p:Project){
    const previous=p.id===project?.id && chat?.createdAt ? chat : snapshot.assessments
      .filter(item=>item.projectId===p.id).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt))[0];
    const settings=snapshot.settings;
    const selection=previous?.aiSelection ?? (settings.hasApiKey&&knownProvider(settings.provider)
      ? {provider:settings.provider,model:settings.model} : null);
    const key=crypto.randomUUID();
    setSession({key,project:p,source:previous,selection});setProjectId(p.id);
    onSelect('');setLaunch(false);setPanel(undefined);setTree(false);setError('');
  }
  async function prepareConversation():Promise<Assessment>{
    if(!draft)throw new Error('O projeto desta conversa mudou.');
    if(draft.prepared)return draft.prepared;
    if(creation.current?.key===draft.key)return creation.current.promise;
    const promise=requestFirstConversation(draft);
    creation.current={key:draft.key,promise};
    try{return await promise;}finally{if(creation.current?.key===draft.key)creation.current=undefined;}
  }
  async function requestFirstConversation(current:NewChat){
    const result=await action<Assessment>('project.newConversation',{
      projectId:current.project.id,sourceAssessmentId:current.source?.id,firstTurn:true,
      title:'Nova conversa',aiSelection:current.selection,idempotencyKey:current.key,
    });
    bindChatDraft(current.key,result.id);
    setSession(value=>value?.key===current.key?{...value,prepared:result}:value);
    return result;
  }
  useEffect(()=>{
    if(draft?.prepared&&selected!==draft.prepared.id)onSelect(draft.prepared.id);
  },[draft?.prepared?.id,onSelect,selected]);
  async function chooseDraftModel(choice:AiSelection) {
    if(!draft)return;
    if(draft.prepared)await action('ai.selection.save',{assessmentId:draft.prepared.id,aiSelection:choice});
    setSession(current=>current?.key===draft.key?{...current,selection:choice,
      prepared:current.prepared?{...current.prepared,aiSelection:choice}:undefined}:current);
  }
  function newProject(kind:StartKind='code'){setSession(undefined);pendingFocus.current=true;setStartKind(kind);setLaunch(true);setPanel(undefined);setTree(false);setError('');}
  function dismissTree(){setTree(false);mobileMenu.current?.focus({preventScroll:true});}
  function finishRename(){
    setEditingTitle(undefined);
    requestAnimationFrame(()=>renameButton.current?.focus());
  }
  async function saveTitle(){
    if(!editingTitle || busy || !editingTitle.text.trim())return;
    try{
      await action('project.renameConversation',{assessmentId:editingTitle.id,title:editingTitle.text.trim()});
      finishRename();
    }catch(e){setError(e instanceof Error?e.message:String(e));}
  }
  const showTree=live.length>0;
  return <section ref={desk} style={{'--project-tree-width':treeWidth+'px'} as CSSProperties} data-resizing={resizing} className={'project-desk'+(showTree?' with-tree':'')+(tree?' tree-open':'')+(treeCollapsed?' tree-collapsed':'')} aria-label="Área de projetos"
    onKeyDown={event=>{if(event.key==='Escape'&&tree&&!event.defaultPrevented){event.preventDefault();dismissTree();}}}>
    {showTree && <ProjectTree projects={live} assessments={snapshot.assessments} projectId={launch?undefined:project?.id} assessmentId={launch?undefined:chat?.id} busy={busy} onNew={()=>newProject()} onChat={openChat} onProject={openProject} onNewChat={p=>void newChat(p)}/>}
    {showTree&&!treeCollapsed&&<div className="project-tree-resizer" role="separator" aria-label="Largura da lista de projetos" aria-orientation="vertical" aria-controls="project-tree" aria-valuemin={185} aria-valuemax={maximumWidth} aria-valuenow={treeWidth} tabIndex={0}
      onPointerDown={event=>{if(event.button!==0)return;event.preventDefault();event.currentTarget.focus();event.currentTarget.setPointerCapture(event.pointerId);drag.current={start:event.clientX,width:treeWidth,scale:(desk.current?.getBoundingClientRect().width??1)/(desk.current?.clientWidth??1)};setResizing(true);}}
      onPointerMove={event=>{if(drag.current)resizeTree(drag.current.width+(event.clientX-drag.current.start)/drag.current.scale);}}
      onPointerUp={event=>{if(!drag.current)return;persistWidth(treeWidth);drag.current=undefined;setResizing(false);event.currentTarget.releasePointerCapture(event.pointerId);}}
      onLostPointerCapture={()=>{drag.current=undefined;setResizing(false);}}
      onPointerCancel={()=>{drag.current=undefined;setResizing(false);}}
      onDoubleClick={()=>{persistWidth(resizeTree(240));}}
      onKeyDown={event=>{const next=event.key==='ArrowLeft'?treeWidth-16:event.key==='ArrowRight'?treeWidth+16:event.key==='Home'?185:event.key==='End'?maximumWidth:undefined;
        if(next!==undefined){event.preventDefault();persistWidth(resizeTree(next));}}}/>}
    {tree&&<button className="tree-dismiss" aria-label="Fechar lista de projetos" tabIndex={-1} onClick={dismissTree}/>}
    <div className="desk-main" ref={main} tabIndex={-1}>
      {showTree&&<button ref={mobileMenu} type="button" className="tree-mobile-toggle" aria-controls="project-tree" aria-expanded={tree} onClick={()=>setTree(!tree)}><PanelLeft size={16}/>Projetos</button>}
      {error&&<p className="app-alert" role="alert">{error}</p>}
      {launch?<ProjectLaunch sourceOnly={sourceOnly} snapshot={snapshot} action={action} busy={busy} initialKind={startKind==='github'?'code':startKind} initialSource={startKind==='github'?'github':'local'} onModels={onModels} onCancel={()=>setLaunch(false)} onCreated={(id,assessment)=>{setProjectId(id);openChat(assessment);}}/>
      :!project?<ProjectWelcome sourceOnly={sourceOnly} onStart={newProject}/>
      :<>
        <header className="desk-bar"><div className="desk-location">
          <button className="desk-tree-toggle" aria-controls="project-tree" aria-expanded={!treeCollapsed} aria-label={treeCollapsed?'Mostrar lista de projetos':'Recolher lista de projetos'} title={treeCollapsed?'Mostrar lista de projetos':'Recolher lista de projetos'} onClick={()=>setTreeCollapsed(!treeCollapsed)}><PanelLeft size={17}/></button>
          <ProjectIcon className="desk-location-icon" size={17}/><div><h1 title={project.name}>{project.name}</h1>
            {chat&&(!draft||history.some(a=>a.id===chat.id))&&(editingTitle?.id===chat.id?
              <form className="desk-title-editor" onSubmit={e=>{e.preventDefault();void saveTitle();}} onKeyDown={e=>{if(e.key==='Escape'){e.preventDefault();finishRename();}}}>
                <input autoFocus aria-label="Título da conversa" maxLength={160} required value={editingTitle.text} onChange={e=>setEditingTitle({id:chat.id,text:e.target.value})}/>
                <button type="submit" aria-label="Salvar título" disabled={busy||!editingTitle.text.trim()}><Check size={14}/></button>
                <button type="button" aria-label="Cancelar renomeação" onClick={finishRename}><X size={14}/></button>
              </form>:
              <button className="desk-chat-title" type="button" ref={renameButton} aria-label="Renomear conversa" title={chat.title} onClick={()=>setEditingTitle({id:chat.id,text:chat.title})}>
                <small>{chat.title}</small><Pencil size={12} aria-hidden="true"/>
              </button>)}</div></div>
          <div className="desk-actions"><button ref={panelLauncher} type="button" title="Painel do projeto" aria-label="Painel do projeto" aria-expanded={panelOpen} aria-controls="desk-project-panel" onClick={togglePanel}><LayoutDashboard size={16} aria-hidden="true"/><span>Projeto</span></button></div>
        </header>
        <div className={'desk-content'+(panel&&split?' desk-split':'')}>
          <div className="desk-conversation" hidden={!!panel&&!split}>
            {chat?<ProjectConversation key={draft?.key??chat.id} draftKey={draft?.key} unsaved={!!draft&&!draft.prepared} prepareConversation={draft?prepareConversation:undefined}
              analysisSource={draft && !history.some(item=>item.id===chat.id)?draft.source:chat} onAnalysisAdmitted={openChat}
              onDraftSelection={draft?chooseDraftModel:undefined}
              sourceOnly={sourceOnly} assessment={chat} plans={snapshot.plans?.filter(plan=>!snapshot.workPlanBindings?.some(binding=>binding.executionPlanId===plan.id))} settings={snapshot.settings} action={action} busy={busy} onAdvanced={sourceOnly?undefined:()=>onAdvanced(chat.id)} onModels={onModels}/>
            :<div className="desk-welcome"><h2>Comece uma conversa</h2><button className="button primary" disabled={busy} onClick={()=>void newChat(project)}><Plus size={16}/>Nova conversa</button></div>}
          </div>
          {panel&&<aside id="desk-project-panel" className="desk-panel" aria-label="Projeto" onKeyDown={event=>{
            if(event.key==='Escape'&&!event.defaultPrevented&&!event.nativeEvent.isComposing&&event.target instanceof Element&&!event.target.closest('[role="dialog"],dialog,select,[aria-expanded="true"]')){event.stopPropagation();closePanel();}
          }}>
            <div className="desk-panel-head"><div className="desk-panel-tabs" role="tablist" aria-label="Painel do projeto">
              {(['overview','settings'] as const).map((value,index)=><button key={value} id={'desk-tab-'+value} type="button" role="tab" aria-selected={panel===value} tabIndex={panel===value?0:-1} aria-controls="desk-panel-content" onClick={()=>setPanel(value)} onKeyDown={event=>{
                const next=event.key==='Home'?0:event.key==='End'?1:['ArrowLeft','ArrowRight'].includes(event.key)?1-index:undefined;
                if(next===undefined)return;
                event.preventDefault();const target=next===0?'overview':'settings';setPanel(target);
                document.getElementById('desk-tab-'+target)?.focus();
              }}>{value==='overview'?'Visão geral':'Configurações'}</button>)}
            </div><div><button className="icon-button" title={split?'Ampliar painel':'Mostrar ao lado da conversa'} aria-label={split?'Ampliar painel':'Mostrar ao lado da conversa'} aria-pressed={split} onClick={()=>setSplit(!split)}>{split?<Maximize2 size={16}/>:<PanelRight size={16}/>}</button><button ref={panelClose} className="icon-button" aria-label="Fechar painel" onClick={closePanel}><X size={17}/></button></div></div>
            <div role="tabpanel" id="desk-panel-content" aria-labelledby={'desk-tab-'+panel}>
              <Projects sourceOnly={sourceOnly} key={project.id} panel={panel} initialProjectId={project.id} projects={snapshot.projects} assessments={snapshot.assessments} action={action} busy={busy} onNewAssessment={()=>void newChat(project)} onOpenAssessment={openChat} onArchived={()=>{onSelect('');setProjectId('');setPanel(undefined);}} insights={<ProjectInsights snapshot={snapshot} project={project} selected={chat?.id} onConversation={openChat} onFinding={onFinding}/>}/>
            </div>
          </aside>}
        </div>
      </>}
    </div>
  </section>;
}
