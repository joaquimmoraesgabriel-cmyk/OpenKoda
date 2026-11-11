import { useState } from 'react';
import { ArrowRight, ArrowUpRight, Clock3, Folder, FolderOpen, Layers, Plus } from 'lucide-react';
import { kindLabels, shortDate, statusLabels, type Assessment, type Project } from '../types';

type Period = 'week' | 'month';

/** Local calendar days keep today's activity in today's column across UTC and DST boundaries. */
export function assessmentActivity(assessments: Assessment[], period: Period, today = new Date()) {
  const day = new Date(today);
  day.setHours(0, 0, 0, 0);
  const weekly = period === 'week';
  const steps = weekly ? 7 : 4;
  const daysPerStep = weekly ? 1 : 7;
  const shortDay = new Intl.DateTimeFormat('pt-BR', { weekday: 'short' });
  const numericDate = new Intl.DateTimeFormat('pt-BR', { day: 'numeric', month: 'numeric' });
  const buckets = Array.from({ length: steps }, (_, index) => {
    const first = new Date(day);
    first.setDate(first.getDate() + 1 - (steps - index) * daysPerStep);
    const end = new Date(first);
    end.setDate(end.getDate() + daysPerStep);
    return { first, end, label: weekly ? shortDay.format(first).replace('.', '') : numericDate.format(first), count: 0 };
  });
  for (const assessment of assessments) {
    const created = new Date(assessment.createdAt);
    const bucket = buckets.find(bucket => created >= bucket.first && created < bucket.end);
    if (bucket && created <= today) bucket.count++;
  }
  return buckets;
}

function ActivityChart({ assessments }: { assessments: Assessment[] }) {
  const [period, setPeriod] = useState<Period>('week');
  const buckets = assessmentActivity(assessments, period);
  const peak = Math.max(1, ...buckets.map(bucket => bucket.count));
  const hasActivity = buckets.some(bucket => bucket.count > 0);
  return <section className="dashboard-panel dashboard-activity" aria-labelledby="dashboard-activity-title">
    <header className="dashboard-section-head"><h2 id="dashboard-activity-title">Atividade das avaliações</h2>
      <div className="dashboard-period" role="group" aria-label="Período da atividade">
        <button type="button" aria-pressed={period === 'week'} onClick={() => setPeriod('week')}>Semana</button>
        <button type="button" aria-pressed={period === 'month'} onClick={() => setPeriod('month')}>Mês</button>
      </div>
    </header>
    <p className="dashboard-caption">{period === 'week' ? 'Avaliações criadas nos últimos 7 dias' : 'Avaliações criadas nas últimas 4 semanas'}</p>
    <div className="dashboard-plot">
      {!hasActivity && <p className="dashboard-no-activity">Nenhuma avaliação criada no período</p>}
      <ol className="dashboard-chart" aria-label="Avaliações criadas por período">{buckets.map(bucket => (
        <li key={bucket.first.getTime()} aria-label={bucket.label + ', avaliações criadas: ' + bucket.count}>
          <div className="dashboard-bar-track" aria-hidden="true">
            {bucket.count > 0 && <div className="dashboard-bar" style={{ height: (bucket.count / peak * 100) + '%' }}>
              <span>{bucket.count}</span></div>}
          </div><span className="dashboard-axis" aria-hidden="true">{bucket.label}</span>
        </li>
      ))}</ol>
    </div>
  </section>;
}

interface Props {
  assessments: Assessment[];
  projects: Project[];
  busy: boolean;
  onOpenAssessment: (id: string) => void;
  onNewAssessment: () => void;
  onOpenProject: (id: string) => void;
  onNewProject: () => void;
}

export default function Overview({ assessments, projects, busy, onOpenAssessment, onNewAssessment, onOpenProject, onNewProject }: Props) {
  const activeProjects = projects.filter(project => !project.archived);
  const awaiting = assessments.filter(assessment => assessment.status === 'awaiting_approval');
  const ongoing = assessments.filter(assessment =>
    ['preparing', 'planning', 'queued', 'running', 'pausing', 'validating', 'canceling'].includes(assessment.status));
  const pending = awaiting.sort((a, b) => Date.parse(a.updatedAt) - Date.parse(b.updatedAt))[0];
  const recentAssessment = [...assessments].sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))[0];
  const next = pending ?? recentAssessment;
  const lastActivity = new Map(activeProjects.map(project => [project.id, project.createdAt]));
  for (const assessment of assessments) {
    const previous = lastActivity.get(assessment.projectId);
    if (previous && Date.parse(assessment.updatedAt) > Date.parse(previous)) lastActivity.set(assessment.projectId, assessment.updatedAt);
  }
  const recentProjects = activeProjects.sort((a, b) => Date.parse(lastActivity.get(b.id)!) - Date.parse(lastActivity.get(a.id)!)).slice(0, 4);
  return <div className="workspace-page dashboard">
    <header className="workspace-heading"><h1>Visão geral</h1><p>Seu espaço de avaliações</p></header>
    <div className="dashboard-numbers">
      <section className="dashboard-panel dashboard-stat dashboard-stat-featured">
        <h2>Aguardando revisão</h2><Clock3 aria-hidden="true" size={28}/><strong>{String(awaiting.length).padStart(2, '0')}</strong>
        {pending && <button className="dashboard-link" onClick={() => onOpenAssessment(pending.id)}>Revisar pendência<ArrowUpRight size={15}/></button>}
      </section>
      <section className="dashboard-panel dashboard-stat"><h2>Avaliações em andamento</h2><Layers aria-hidden="true" size={28}/><strong>{String(ongoing.length).padStart(2, '0')}</strong></section>
      <section className="dashboard-panel dashboard-stat"><h2>Projetos ativos</h2><Folder aria-hidden="true" size={28}/><strong>{String(activeProjects.length).padStart(2, '0')}</strong></section>
    </div>
    <div className="dashboard-middle"><ActivityChart assessments={assessments}/>
      <section className="dashboard-panel dashboard-next">
        <h2>{next && !pending ? 'Última avaliação' : 'Próxima ação'}</h2>
        <div className="dashboard-next-body">
          <span className="dashboard-symbol"><FolderOpen size={22} aria-hidden="true"/></span>
          <h3>{next ? next.title : activeProjects.length ? 'Crie uma avaliação' : 'Adicione seu primeiro projeto'}</h3>
          <p>{next ? statusLabels[next.status] : activeProjects.length ? 'Escolha um projeto e defina o objetivo.' : 'Escolha um alvo para iniciar seu trabalho.'}</p>
          {next && <small>{projects.find(project => project.id === next.projectId)?.name}</small>}
        </div>
        <button type="button" className="dashboard-pill" disabled={busy && !next}
          onClick={() => next ? onOpenAssessment(next.id) : activeProjects.length ? onNewAssessment() : onNewProject()}>
          {next ? pending ? 'Revisar avaliação' : 'Abrir avaliação' : activeProjects.length ? 'Preparar avaliação' : 'Criar projeto'}<ArrowRight size={16}/>
        </button>
      </section>
    </div>
    <section className="dashboard-panel dashboard-projects">
      <header className="dashboard-section-head"><h2>Projetos recentes</h2><button className="dashboard-pill" disabled={busy} onClick={onNewProject}><Plus size={16}/>Novo projeto</button></header>
      {recentProjects.length === 0 ? <p className="dashboard-empty-projects">Seus projetos aparecerão aqui.</p> :
        <ul>{recentProjects.map(project => <li key={project.id}><button type="button" onClick={() => onOpenProject(project.id)}>
          <span className="dashboard-symbol"><Folder size={20} aria-hidden="true"/></span><strong>{project.name}</strong>
          <span className="dashboard-project-kind">{kindLabels[project.kind]}</span><time dateTime={lastActivity.get(project.id)}>{shortDate(lastActivity.get(project.id)!)}</time><ArrowRight size={17}/>
        </button></li>)}</ul>}
    </section>
  </div>;
}
