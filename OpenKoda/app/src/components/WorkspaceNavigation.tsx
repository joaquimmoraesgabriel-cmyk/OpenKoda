import { ClipboardCheck, Folder, House, PanelsTopLeft, Plus, ScanSearch, Settings, UserRound } from 'lucide-react';
import type { View } from '../types';

const destinations = [
  { view: 'overview', label: 'Visão geral', icon: House },
  { view: 'projects', label: 'Projetos', icon: Folder },
  { view: 'assessments', label: 'Avaliações', icon: ClipboardCheck },
  { view: 'findings', label: 'Achados', icon: ScanSearch },
  { view: 'kali', label: 'Kali e ferramentas', icon: PanelsTopLeft },
  { view: 'settings', label: 'Configurações', icon: Settings },
  { view: 'profile', label: 'Perfil', icon: UserRound },
] as const;

interface Props {
  view: View;
  onNavigate: (next: View) => void;
  onCreateAssessment: () => void;
  creationDisabled: boolean;
}

export default function WorkspaceNavigation({ view, onNavigate, onCreateAssessment, creationDisabled }: Props) {
  const buttons = destinations.map(({ view: dest, label, icon: Icon }) => (
    <button key={dest} type="button" className="workspace-nav-item" aria-label={label}
      title={label} aria-current={view === dest ? 'page' : undefined} onClick={() => onNavigate(dest)}>
      <Icon size={20} strokeWidth={1.65} aria-hidden="true"/><span>{label}</span>
    </button>
  ));
  return <aside className="workspace-sidebar">
    <div className="workspace-wordmark" aria-label="OpenKoda">
      <svg viewBox="0 0 26 24" width="26" height="24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="7" strokeLinecap="round" strokeLinejoin="round">
        <path d="M3.5 3.5v17M21.5 3.5l-9 8.5 9 8.5"/>
      </svg><span>OpenKoda</span>
    </div>
    <nav aria-label="Navegação principal">
      <div className="workspace-nav-primary">
        {buttons.slice(0, 5)}
        <button type="button" className="workspace-nav-item workspace-create" aria-label="Nova avaliação"
          title="Nova avaliação" disabled={creationDisabled} onClick={onCreateAssessment}>
          <Plus size={18} aria-hidden="true"/><span>Nova avaliação</span>
        </button>
      </div>
      <div className="workspace-nav-footer">{buttons.slice(5)}</div>
    </nav>
  </aside>;
}
