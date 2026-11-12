import { Activity, ChevronRight } from 'lucide-react';
import { shortDate, type ActivityEvent } from '../types';
import type { ProjectionInfo } from '../data-policy';
import { ProjectionNotice } from './DataAccess';
import { ConversationText } from './ConversationText';

export type ChatMessage = { id: string; role: string; text: string; createdAt: string; projection?: ProjectionInfo };
type Entry = { at: string; event: ActivityEvent } | { at: string; message: ChatMessage };

export function ConversationTimeline({ events, messages }: { events: ActivityEvent[]; messages: ChatMessage[] }) {
  const ordered: Entry[] = [
    ...events.map(event => ({ at: event.at, event })),
    ...messages.map(message => ({ at: message.createdAt, message })),
  ].sort((a, b) => a.at.localeCompare(b.at));
  const groups: (ChatMessage | ActivityEvent[])[] = [];
  for (const entry of ordered) {
    if ('message' in entry) { groups.push(entry.message); continue; }
    const last = groups[groups.length - 1];
    if (Array.isArray(last)) last.push(entry.event);
    else groups.push([entry.event]);
  }
  return <>{groups.map(group => {
    if (!Array.isArray(group)) return <article className={'project-chat-turn ' + group.role} key={'turn-' + group.id}>
      <header><strong>{group.role === 'user' ? 'Você' : 'OpenKoda'}</strong><time dateTime={group.createdAt}>{shortDate(group.createdAt)}</time></header>
      <ProjectionNotice projection={group.projection}/>
      {group.role === 'user' ? <p>{group.text}</p> : <ConversationText text={group.text}/>}
    </article>;
    const last = group[group.length - 1];
    return <details className="chat-activity" key={'activity-' + group[0].id}>
      <summary><span className="chat-activity-label"><Activity size={16}/><strong>Atividade</strong><span>{group.length} {group.length === 1 ? 'etapa' : 'etapas'}</span><ChevronRight size={14}/></span><span className="chat-last-step">{last.message}</span></summary>
      <ol>{group.map(event => <li key={event.id}><span>{event.message}</span><time dateTime={event.at}>{shortDate(event.at)}</time></li>)}</ol>
    </details>;
  })}</>;
}
