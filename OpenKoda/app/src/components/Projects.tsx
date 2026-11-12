import { useLayoutEffect, useRef, useState } from 'react';
import { Plus, Folder, Globe, Code2, Network, FileText, Archive, Play, ArrowLeft, ChevronRight } from 'lucide-react';
import { Action, Assessment, Kind, Project, kindLabels, shortDate } from '../types';
import { Empty, Field, Notice, SearchBox, Status, Tabs } from './shared';
import AssessmentComparison from './AssessmentComparison';
import CredentialReferences from './CredentialReferences';
import DataPolicy from './DataPolicy';
import ProjectTargetField from './ProjectTargetField';
import CopyTargetButton from './CopyTargetButton';
import ProjectTargets from './ProjectTargets';
import './Projects.css';

export function KindIcon({kind, size = 20}: {kind: Kind; size?: number}) {
  const Icon = { code: Code2, web: Globe, network: Network, files: FileText }[kind];
  return <Icon size={size} aria-hidden="true"/>;
}

interface ProjectsProps {
  projects: Project[];
  assessments: Assessment[];
  action: Action;
  onNewAssessment: (id?: string) => void;
  onOpenAssessment: (id: string) => void;
  busy: boolean;
  initialProjectId?: string;
  initialCreate?: boolean;
  panel?: 'overview'|'settings';
  insights?: React.ReactNode;
  onArchived?: () => void;
}

export default function Projects({ projects, assessments, action, onNewAssessment, onOpenAssessment, busy, initialProjectId, initialCreate, panel, insights, onArchived }: ProjectsProps) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(initialProjectId ?? '');
  const [creating, setCreating] = useState(!!initialCreate);
  const [tab, setTab] = useState('Visão geral');
  const [catalogOpen, setCatalogOpen] = useState(false);
  const [name, setName] = useState('');
  const [kind, setKind] = useState<Kind>('code');
  const [target, setTarget] = useState('');
  const [description, setDescription] = useState('');
  const [confirmArchive, setConfirmArchive] = useState(false);
  const live = projects.filter(project => !project.archived);
  const project = live.find(project => project.id === selected);
  const filtered = live.filter(project => (project.name + ' ' + project.target).toLowerCase().includes(query.toLowerCase()));
  const history = assessments.filter(item => item.projectId === project?.id).sort((a,b)=>b.updatedAt.localeCompare(a.updatedAt));
  const page = useRef<HTMLElement>(null);
  const listTrigger = useRef('');
  const restoreFocus = useRef(false);

  useLayoutEffect(() => {
    if (!restoreFocus.current || !page.current) return;
    restoreFocus.current = false;
    const trigger = !project && listTrigger.current
      ? page.current.querySelector<HTMLElement>(`[data-project-trigger="${CSS.escape(listTrigger.current)}"]`)
      : null;
    (trigger ?? page.current.querySelector('h1'))?.focus();
  }, [project?.id, creating]);

  function open(project: Project) {
    listTrigger.current = 'row-' + project.id;
    restoreFocus.current = true;
    setSelected(project.id);
    setCreating(false);
    setConfirmArchive(false);
    setTab('Visão geral');
  }

  function startProject(trigger: string) {
    listTrigger.current = trigger;
    setCreating(true);
  }

  function backToProjects() {
    if (selected) listTrigger.current = 'row-' + selected;
    restoreFocus.current = true;
    setSelected('');
    setCreating(false);
  }

  async function create(event: React.FormEvent) {
    event.preventDefault();
    const created = await action<Project>('project.create', { name, kind, target, description });
    open(created);
    setName('');
    setTarget('');
    setDescription('');
  }

  if (panel && !project) return null;
  if (!creating && !project) return <section ref={page} className="projects-page" aria-label="Lista de projetos">
    <header className="projects-heading">
      <div className="projects-title"><h1 tabIndex={-1}>Projetos</h1><span className="count" aria-label={`${live.length} projetos ativos`}>{live.length}</span></div>
      <button className="button primary" data-project-trigger="new" onClick={() => startProject('new')}><Plus size={17}/>Novo projeto</button>
    </header>
    <div className="projects-sheet">
      {live.length > 0 && <div className="projects-toolbar"><SearchBox label="Pesquisar projetos" value={query} onChange={setQuery}/></div>}
      {filtered.length ? <ul className="project-rows">{filtered.map(item => <li key={item.id}>
        <button type="button" className="project-row" data-project-trigger={'row-' + item.id} onClick={() => open(item)}>
          <span className="project-kind-icon"><KindIcon kind={item.kind}/></span>
          <span className="project-row-copy"><strong>{item.name}</strong><small>{kindLabels[item.kind]} · {item.target}</small></span>
          <time dateTime={item.createdAt}>{shortDate(item.createdAt)}</time>
          <ChevronRight size={17} aria-hidden="true"/>
        </button>
      </li>)}</ul> : <Empty title={query ? 'Nenhum projeto encontrado' : 'Nenhum projeto cadastrado'} description={query ? 'Experimente outro nome ou endereço.' : 'Crie um projeto para reunir seus alvos e avaliações.'} icon={<Folder size={30}/>} action={!query && <button className="button" data-project-trigger="add" onClick={() => startProject('add')}><Plus size={17}/>Adicionar projeto</button>}/>}
    </div>
  </section>;

  return <section ref={page} className="projects-page project-detail" aria-label="Detalhes do projeto">
    {!panel && <button className="project-back" type="button" onClick={backToProjects}><ArrowLeft size={16}/>Voltar aos projetos</button>}
    {creating ? <form className="form-page" onSubmit={event => void create(event).catch(() => {})}>
      <div className="detail-heading"><div><h1>Novo projeto</h1><p>Defina o alvo e o contexto do seu trabalho.</p></div></div>
      <div className="project-form-sheet">
        <div className="form-group">
          <Field label="Nome do projeto"><input value={name} onChange={e => setName(e.target.value)} required maxLength={120} placeholder="Ex.: API de pagamentos" autoFocus/></Field>
          <Field label="Modalidade"><select value={kind} onChange={e => setKind(e.target.value as Kind)}>{Object.entries(kindLabels).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</select></Field>
          <ProjectTargetField key={kind} kind={kind} label="Alvo" value={target} onChange={setTarget}/>
          <Field label="Descrição"><textarea value={description} onChange={e => setDescription(e.target.value)} maxLength={4000} rows={3} placeholder="Objetivo e informações relevantes sobre este projeto"/></Field>
        </div>
        <Notice>Adicionar um projeto apenas cadastra o alvo. Nenhuma ferramenta é executada.</Notice>
        <div className="form-actions"><button className="button" type="button" onClick={backToProjects}>Cancelar</button><button className="button primary" disabled={busy} type="submit">{busy ? 'Salvando…' : 'Criar projeto'}</button></div>
      </div>
    </form> : project && <>
      <div className="detail-heading"><div><h1 tabIndex={-1}>{project.name}</h1><p>{kindLabels[project.kind]}</p></div>{!panel && <button className="button primary" onClick={() => onNewAssessment(project.id)}><Play size={16}/>Nova avaliação neste projeto</button>}{panel==='overview'&&<button className="button" disabled={busy} onClick={()=>onNewAssessment(project.id)}><Plus size={15}/>Nova conversa</button>}</div>
      {!panel && <Tabs items={['Visão geral','Alvos e escopo','Avaliações','Configurações']} selected={tab} onChange={setTab}/>}
      {panel==='overview' && insights}
      {!panel && tab === 'Visão geral' && <div className="project-overview">
        <section className="content-section">
          <h2>Alvo cadastrado</h2>
          <div className="target-row">
            <KindIcon kind={project.kind}/>
            <div><strong>{project.name}</strong><p className="break-word">{project.target}</p></div>
            <span className="muted">{kindLabels[project.kind]}</span>
          </div>
          <CopyTargetButton key={project.id} target={project.target}/>
        </section>
        <section className="content-section">
          <h2>Fontes e alvos</h2>
          {project.description && <p className="preserve-lines">{project.description}</p>}
          <p className="muted">Cada avaliação escolhe as entradas do catálogo e os métodos permitidos.</p>
          <button className="text-button" onClick={() => setTab('Alvos e escopo')}>Revisar escopo</button>
        </section>
        <section className="content-section">
          <h2>Últimas avaliações</h2>
          {history.length ? history.slice(0, 4).map(a => <button className="history-row" key={a.id} onClick={() => onOpenAssessment(a.id)}>
            <Play size={17}/><strong>{a.title}</strong><Status value={a.status}/>
          </button>) : <p className="muted">As avaliações deste projeto aparecerão aqui.</p>}
        </section>
      </div>}
        {!panel&&tab==='Alvos e escopo'&&<ProjectTargets key={project.id} project={project} action={action} busy={busy}/>}
        {panel==='settings'&&<details className="disclosure spaced" open={catalogOpen} onToggle={event=>setCatalogOpen(event.currentTarget.open)}><summary>Alvos e escopo</summary>{catalogOpen&&<div className="disclosure-body"><ProjectTargets project={project} action={action} busy={busy}/></div>}</details>}
        {!panel&&tab==='Avaliações'&&<section className="content-section">{history.length?history.map(a=><button key={a.id} className="history-row" onClick={()=>onOpenAssessment(a.id)}><Play size={17}/><strong>{a.title}</strong><Status value={a.status}/></button>):<Empty title="Nenhuma avaliação neste projeto" description="Crie uma avaliação para definir o objetivo e revisar o trabalho." action={<button className="button primary" onClick={()=>onNewAssessment(project.id)}>Nova avaliação neste projeto</button>}/>}</section>}
        {(panel==='overview'||(!panel&&tab==='Avaliações'))&&<AssessmentComparison key={project.id} projectId={project.id} assessments={history}/>}
        {(panel==='settings'||(!panel&&tab==='Configurações'))&&<><DataPolicy key={project.id} projectId={project.id} action={action} busy={busy}/><CredentialReferences key={'credentials-'+project.id} projectId={project.id} action={action} busy={busy}/><section className="content-section"><h2>Arquivar projeto</h2><p className="muted">Retire o projeto da lista ativa, preservando suas avaliações e evidências.</p>{confirmArchive?<Notice tone="warning">Arquivar “{project.name}”?<div className="inline-actions"><button className="button" onClick={()=>setConfirmArchive(false)}>Manter projeto</button><button className="button danger" disabled={busy} onClick={()=>void action('project.archive',{id:project.id}).then(()=>{setSelected('');setConfirmArchive(false);onArchived?.();}).catch(()=>{})}>Confirmar arquivamento</button></div></Notice>:<button className="button" onClick={()=>setConfirmArchive(true)}><Archive size={16}/>Arquivar projeto</button>}</section></>}
    </>}
  </section>;
}
