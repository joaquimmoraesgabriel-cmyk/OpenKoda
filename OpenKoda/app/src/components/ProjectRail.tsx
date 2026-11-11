import { useState } from 'react';
import { ChevronDown, ChevronRight, Folder, Globe, House, Network, PanelsTopLeft, Plus, ScanSearch, Settings, UserRound } from 'lucide-react';
import { useIdentity } from '../profile-identity';
import type { Assessment, Project, View } from '../types';

export function Rail({view,navigate}:{view:View;navigate:(v:View)=>void}) {
  const identity=useIdentity();
  const destinations=[
    {id:'overview',label:'Início',icon:House},{id:'findings',label:'Achados',icon:ScanSearch},
    {id:'kali',label:'Kali e ferramentas',icon:PanelsTopLeft},{id:'settings',label:'Configurações',icon:Settings},
    {id:'profile',label:'Perfil',icon:UserRound},
  ] as const;
  return <nav className="project-rail" aria-label="Navegação principal">
    <div className="rail-brand" aria-label="OpenKoda"><svg viewBox="0 0 26 24" width="23" height="22" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="7" strokeLinecap="round" strokeLinejoin="round"><path d="M3.5 3.5v17M21.5 3.5l-9 8.5 9 8.5"/></svg></div>
    {destinations.map(({id,label,icon:Icon})=><button key={id} type="button" className={'rail-button rail-'+id} title={label} aria-label={label} aria-current={view===id?'page':undefined} onClick={()=>navigate(id)}>
      {id==='profile' && identity.name ? <span className="rail-avatar" data-color={identity.color}>{identity.name.trim().split(/\s+/).slice(0,2).map(s=>s[0]).join('').toUpperCase()}</span> : <Icon size={20} strokeWidth={1.65}/>}
    </button>)}
  </nav>;
}

interface TreeProps {
  projects:Project[]; assessments:Assessment[]; projectId?:string; assessmentId?:string;
  busy:boolean; onNew:()=>void; onProject:(id:string)=>void; onChat:(id:string)=>void; onNewChat:(project:Project)=>void;
}
export function ProjectTree({projects,assessments,projectId,assessmentId,busy,onNew,onProject,onChat,onNewChat}:TreeProps) {
  const [collapsed,setCollapsed]=useState<string[]>([]);
  return <aside className="project-tree" aria-label="Projetos e conversas">
    <button className="tree-new" type="button" onClick={onNew} disabled={busy}><Plus size={18}/>Novo projeto</button>
    <h2>Projetos</h2>
    <div className="tree-list">{projects.filter(p=>!p.archived).map(project=>{
      const Icon=project.kind==='web'?Globe:project.kind==='network'?Network:Folder;
      const open=!collapsed.includes(project.id);
      const chats=assessments.filter(a=>a.projectId===project.id).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt));
      return <section key={project.id} className="tree-group">
        <div className="tree-row" data-active={project.id===projectId}>
          <button type="button" className="tree-toggle" aria-label={(open?'Recolher ':'Expandir ')+project.name} aria-expanded={open} onClick={()=>setCollapsed(prev=>open?[...prev,project.id]:prev.filter(id=>id!==project.id))}>{open?<ChevronDown size={12}/>:<ChevronRight size={12}/>}</button>
          <button className="tree-project" type="button" onClick={()=>{setCollapsed(prev=>prev.filter(id=>id!==project.id));onProject(project.id);}} title={project.name}><Icon size={17}/><span>{project.name}</span></button>
          <button className="tree-add" type="button" disabled={busy} title="Criar nova conversa" aria-label={'Criar nova conversa em '+project.name} onClick={()=>{setCollapsed(prev=>prev.filter(id=>id!==project.id));onNewChat(project);}}><Plus size={16}/></button>
        </div>
        {open&&chats.map(chat=><button type="button" key={chat.id} className="tree-chat" title={chat.title} aria-current={chat.id===assessmentId?'page':undefined} onClick={()=>onChat(chat.id)}>{chat.title}</button>)}
      </section>;
    })}</div>
  </aside>;
}
