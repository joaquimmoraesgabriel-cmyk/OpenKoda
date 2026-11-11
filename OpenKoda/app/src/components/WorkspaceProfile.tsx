import { ArrowUpRight, Monitor, UserRound } from 'lucide-react';
import type { Snapshot } from '../types';
import { native } from '../api';

export default function WorkspaceProfile({ snapshot, onSettings }: { snapshot: Snapshot; onSettings: () => void }) {
  const archived = snapshot.projects.filter(project => project.archived).length;
  return <div className="workspace-page profile-page">
    <header className="workspace-heading"><h1>Perfil</h1><p>Sua área de trabalho neste dispositivo</p></header>
    <section className="dashboard-panel profile-detail">
      <span className="profile-symbol"><UserRound size={30} strokeWidth={1.5} aria-hidden="true"/></span>
      <h2>Este dispositivo</h2>
      <dl>
        <div><dt>Ambiente</dt><dd>{native ? 'Aplicativo desktop' : 'Prévia no navegador'}</dd></div>
        <div><dt>Armazenamento</dt><dd>{native ? 'Banco de dados local' : 'Dados de demonstração do navegador'}</dd></div>
        <div><dt>Projetos ativos</dt><dd>{snapshot.projects.length - archived}</dd></div>
        <div><dt>Projetos arquivados</dt><dd>{archived}</dd></div>
        <div><dt>Avaliações registradas</dt><dd>{snapshot.assessments.length}</dd></div>
      </dl>
      <button type="button" className="dashboard-link" onClick={onSettings}>
        <Monitor size={16} aria-hidden="true"/>Preferências do aplicativo<ArrowUpRight size={15} aria-hidden="true"/>
      </button>
    </section>
  </div>;
}
