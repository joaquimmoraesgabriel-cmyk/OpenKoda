import { useRef, useState, type FormEvent } from 'react';
import { CalendarDays, Database, Monitor, Pencil, UserRound } from 'lucide-react';
import { native } from '../api';
import { avatarColors, saveIdentity, useIdentity, type Identity } from '../profile-identity';
import type { Snapshot } from '../types';
import './profile-usage.css';
import { CommercialAccount } from './CommercialAccount';

const dateFormat = new Intl.DateTimeFormat('pt-BR', {day: 'numeric', month: 'short', year: 'numeric'});

function activeDays(snapshot: Snapshot, today: Date) {
  const start = new Date(today);
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - (start.getDay() + 6) % 7 - 25 * 7);
  const counts = new Map<string, number>();
  for (const assessment of snapshot.assessments) {
    const date = new Date(assessment.createdAt);
    if (date >= start && date <= today) counts.set(date.toDateString(), (counts.get(date.toDateString()) ?? 0) + 1);
  }
  return Array.from({length: 26 * 7}, (_, i) => {
    const date = new Date(start);
    date.setDate(start.getDate() + i);
    return {date, count: counts.get(date.toDateString()) ?? 0, future: date > today};
  });
}

export default function WorkspaceProfile({ snapshot, onSettings, onUsageHistory }: { snapshot: Snapshot; onSettings: () => void; onUsageHistory: () => void }) {
  const identity = useIdentity();
  const [draft, setDraft] = useState(identity);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState('');
  const editButton = useRef<HTMLButtonElement>(null);
  const nameField = useRef<HTMLInputElement>(null);
  const archived = snapshot.projects.filter(project => project.archived).length;
  const days = activeDays(snapshot, new Date());
  const active = days.filter(day => day.count > 0);
  const lastDay = days.filter(day => !day.future).at(-1)!;
  const initials = (editing ? draft : identity).name.trim().split(/\s+/u).filter(Boolean);
  const letters = (Array.from(initials[0] ?? '')[0] ?? '') + (initials.length > 1 ? Array.from(initials[initials.length - 1])[0] : '');
  function endEdit() {
    setEditing(false);
    setError('');
    requestAnimationFrame(() => editButton.current?.focus());
  }
  function save(event: FormEvent) {
    event.preventDefault();
    const next = {...draft, name: draft.name.trim()};
    try {
      saveIdentity(next);
      endEdit();
    } catch {
      setError('Não foi possível salvar o perfil neste dispositivo.');
    }
  }
  return <div className="workspace-page profile-page">
    <header className="workspace-heading profile-toolbar">
      <h1>Perfil</h1>
      {!editing && <button ref={editButton} type="button" className="button" onClick={() => {
        setDraft(identity); setEditing(true); requestAnimationFrame(() => nameField.current?.focus());
      }}><Pencil size={15} aria-hidden="true"/>Editar perfil</button>}
    </header>
    <div className="profile-content">
      <section className="person-hero" aria-label="Identidade do perfil">
        <div className="person-avatar" data-color={editing ? draft.color : identity.color} aria-hidden="true">
          {letters ? letters.toLocaleUpperCase('pt-BR') : <UserRound size={40} strokeWidth={1.5}/>}
        </div>
        {editing ? <form className="person-editor" onSubmit={save} onKeyDown={e => { if (e.key === 'Escape') endEdit(); }}>
          <label htmlFor="profile-display-name">Nome de exibição</label>
          <input id="profile-display-name" ref={nameField} autoComplete="off" maxLength={60} placeholder="Meu perfil" value={draft.name} onChange={e => setDraft({...draft, name:e.target.value})}/>
          <fieldset><legend>Cor do avatar</legend><div className="avatar-options">
            {(Object.keys(avatarColors) as Identity['color'][]).map(color => <label key={color} title={avatarColors[color]}>
              <input type="radio" name="avatar-color" aria-label={avatarColors[color]} checked={draft.color === color} onChange={() => setDraft({...draft,color})}/>
              <span data-color={color} aria-hidden="true"/>
            </label>)}
          </div></fieldset>
          <p>O nome e a cor são salvos neste dispositivo.</p>
          {error && <p role="alert">{error}</p>}
          <div className="person-actions"><button type="button" className="button" onClick={endEdit}>Cancelar</button><button type="submit" className="button primary">Salvar perfil</button></div>
        </form> : <><h2>{identity.name || 'Meu perfil'}</h2><p>Sua área de trabalho neste dispositivo</p></>}
      </section>
      <CommercialAccount/>
      <dl className="profile-metrics">
        <div><dt>Projetos ativos</dt><dd>{snapshot.projects.length - archived}</dd></div>
        <div><dt>Projetos arquivados</dt><dd>{archived}</dd></div>
        <div><dt>Avaliações registradas</dt><dd>{snapshot.assessments.length}</dd></div>
        <div><dt>Execuções registradas</dt><dd>{snapshot.executions?.length ?? '—'}</dd></div>
      </dl>
      <section className="profile-block" aria-labelledby="profile-activity-heading">
        <div className="insight-title"><div><h2 id="profile-activity-heading">Atividade das avaliações</h2><p>Avaliações criadas nas últimas 26 semanas</p></div><CalendarDays size={20} aria-hidden="true"/></div>
        <div className="activity-heatmap" aria-hidden="true">
          {days.map(day => <span key={day.date.getTime()} data-level={day.future ? 'future' : Math.min(day.count, 3)} title={dateFormat.format(day.date) + ' · ' + day.count + ' avaliações'}/>)}
        </div>
        <div className="heatmap-range"><span>{dateFormat.format(days[0].date)}</span><span>{dateFormat.format(lastDay.date)}</span></div>
        {active.length ? <details className="activity-dates"><summary>Ver atividade por dia ({active.length})</summary>
          <dl>{[...active].reverse().map(day => <div key={day.date.getTime()}><dt>{dateFormat.format(day.date)}</dt><dd>{day.count} {day.count === 1 ? 'avaliação' : 'avaliações'}</dd></div>)}</dl>
        </details> : <p className="insight-empty">Nenhuma avaliação criada neste período.</p>}
      </section>
      <section className="profile-block" aria-labelledby="profile-device-heading">
        <div className="insight-title"><div><h2 id="profile-device-heading">Este dispositivo</h2></div></div>
        <dl className="device-facts">
          <div><dt><Monitor size={17} aria-hidden="true"/>Ambiente</dt><dd>{native ? 'Aplicativo desktop' : 'Prévia no navegador'}</dd></div>
          <div><dt><Database size={17} aria-hidden="true"/>Armazenamento</dt><dd>{native ? 'Banco de dados local' : 'Dados de demonstração do navegador'}</dd></div>
        </dl>
        <div className="inline-actions">
          <button type="button" className="button" onClick={onSettings}>Preferências do aplicativo</button>
          <button type="button" className="button" onClick={onUsageHistory}>Histórico de uso</button>
        </div>
      </section>
    </div>
  </div>;
}
