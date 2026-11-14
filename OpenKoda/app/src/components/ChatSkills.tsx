import { forwardRef, useEffect, useImperativeHandle, useRef, useState, type KeyboardEvent } from 'react';
import { ArrowLeft, Box, Info, Search, X } from 'lucide-react';
import { request } from '../api';
import type { Skill } from '../commercial';
import { errorMessage } from '../types';
import { springAnimate, springs } from './motion/spring';
import './chat-skills.css';

type Document = Skill & { body: string; source: string; license: string; resources: Record<string, string> };
export type ChatSkillsHandle = { keyDown: (event: KeyboardEvent) => boolean };
type Props = { query: string; fromComposer?: boolean; onSelect: (s: Skill) => void; onClose: () => void };

export const ChatSkills = forwardRef<ChatSkillsHandle, Props>(function ChatSkills({ query, fromComposer = false, onSelect, onClose }, ref) {
  const [items, setItems] = useState<Skill[]>(), [error, setError] = useState(''), [doc, setDoc] = useState<Document>();
  const [search, setSearch] = useState(''), [active, setActive] = useState(0), [retry, setRetry] = useState(0);
  const [loadingDoc, setLoadingDoc] = useState(false);
  const panel = useRef<HTMLElement>(null), buttons = useRef<(HTMLButtonElement | null)[]>([]), documentRequest = useRef(0);
  const filter = (fromComposer ? query : search).trim().toLocaleLowerCase();
  const found = items?.filter(s => `${s.id} ${s.name} ${s.description}`.toLocaleLowerCase().includes(filter)) ?? [];
  const selected = found[Math.min(active, Math.max(0, found.length - 1))];
  useEffect(() => {
    let current = true;
    setError(''); setItems(undefined);
    void request<{ skills: Skill[] }>('account.skills').then(result => {
      if (current) setItems(Array.isArray(result.skills) ? result.skills : []);
    }).catch(e => { if (current) setError(errorMessage(e)); });
    return () => { current = false; documentRequest.current++; };
  }, [retry]);
  useEffect(() => { setActive(0); setDoc(undefined); documentRequest.current++; setLoadingDoc(false); }, [filter]);
  useEffect(() => {
    if (!panel.current) return;
    const animation = springAnimate(panel.current, value => ({ opacity: Math.max(0, Math.min(1, value)), transform: `translateY(${(1 - value) * 10}px)` }), springs.soft);
    return () => animation?.cancel();
  }, []);
  function keyDown(event: KeyboardEvent) {
    if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return false;
    if (event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return false;
    if (event.key === 'Escape') { event.preventDefault(); onClose(); return true; }
    const textInput = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
    if (doc || !found.length) {
      if (fromComposer && textInput && event.key === 'Enter') { event.preventDefault(); return true; }
      return false;
    }
    const focused = buttons.current.indexOf(document.activeElement as HTMLButtonElement);
    const origin = focused >= 0 ? focused : active;
    let next: number | undefined;
    if (event.key === 'ArrowDown') next = focused < 0 ? 0 : (origin + 1) % found.length;
    if (event.key === 'ArrowUp') next = focused < 0 ? found.length - 1 : (origin - 1 + found.length) % found.length;
    if (focused >= 0 && event.key === 'Home') next = 0;
    if (focused >= 0 && event.key === 'End') next = found.length - 1;
    if (next !== undefined) {
      event.preventDefault(); setActive(next); buttons.current[next]?.focus(); buttons.current[next]?.scrollIntoView({ block: 'nearest' }); return true;
    }
    if (event.key === 'Enter' && textInput && focused < 0 && selected) { event.preventDefault(); onSelect(selected); return true; }
    return false;
  }
  useImperativeHandle(ref, () => ({ keyDown }));
  async function details() {
    if (!selected) return;
    const version = ++documentRequest.current;
    setLoadingDoc(true); setError('');
    try { const result = await request<Document>('account.skill', { id: selected.id }); if (version === documentRequest.current) setDoc(result); }
    catch (cause) { if (version === documentRequest.current) setError(errorMessage(cause)); }
    finally { if (version === documentRequest.current) setLoadingDoc(false); }
  }
  return <section ref={panel} className="chat-skills" aria-label="Skills da assinatura" onKeyDown={keyDown}>
    <header><strong>{doc ? 'Sobre a habilidade' : 'Habilidades'}</strong><span>Assinatura Koda</span><button type="button" aria-label="Fechar skills" onClick={onClose}><X size={16}/></button></header>
    {doc ? <div className="skill-document"><button type="button" className="text-button" onClick={() => setDoc(undefined)}><ArrowLeft size={14}/>Voltar às habilidades</button><h3>{doc.name}</h3><p>{doc.description}</p><small>/{doc.id} · {doc.revision} · {doc.invocation}</small><p className="skill-origin">{doc.source}</p><pre>{doc.body}</pre>{Object.entries(doc.resources ?? {}).map(([name, body]) => <details key={name}><summary>{name}</summary><pre>{body}</pre></details>)}<details><summary>Licença</summary><pre>{doc.license}</pre></details></div> : <>
      {!fromComposer && <label className="skill-search"><Search size={16}/><input autoFocus type="search" aria-label="Buscar habilidades" placeholder="Buscar por nome ou função" value={search} onChange={e => setSearch(e.target.value)}/></label>}
      {error && <p className="skill-feedback" role="alert">{error}<button type="button" className="text-button" onClick={() => setRetry(n => n + 1)}>Tentar novamente</button></p>}
      {!items && !error && <p className="skill-feedback" role="status">Carregando habilidades…</p>}
      {items && !found.length && <p className="skill-feedback" role="status">Nenhuma habilidade encontrada. Tente outro nome ou função.</p>}
      <ul className="skill-list" aria-label="Habilidades disponíveis">{found.map((s, index) => <li key={s.id}>
        <button ref={node => { buttons.current[index] = node; }} type="button" className="skill-option" aria-label={`/${s.id} · ${s.name}`} data-active={selected?.id === s.id} tabIndex={selected?.id === s.id ? 0 : -1} onFocus={() => setActive(index)} onPointerMove={() => setActive(index)} onClick={() => onSelect(s)}>
          <Box size={17} aria-hidden="true"/><span className="skill-name">{s.name}</span><span className="skill-description">{s.description}</span><span className="skill-source">Nuvem · Koda</span>
        </button>
      </li>)}</ul>
      <footer><span><kbd>↑</kbd><kbd>↓</kbd> navegar <kbd>Enter</kbd> escolher</span>{selected && <button type="button" disabled={loadingDoc} aria-label={`Detalhes de ${selected.name}`} onClick={() => void details()}><Info size={14}/>{loadingDoc ? 'Abrindo…' : 'Detalhes'}</button>}</footer>
    </>}
  </section>;
});
