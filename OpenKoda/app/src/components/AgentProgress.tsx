import { useState } from 'react';
import { Circle, LoaderCircle, Minus, Plus } from 'lucide-react';
import type { ActivityEvent } from '../types';
import './motion/motion-controls.css';

// Only backend events and the real request status are rendered. There is no
// demo timeline and no timer that manufactures progress.
export function AgentProgress({label,events}:{label:string;events:ActivityEvent[]}) {
  const [open,setOpen]=useState(false);
  return <section className="agent-progress" aria-label="Andamento do trabalho">
    <div className="agent-progress-heading"><p className="chat-progress" role="status"><LoaderCircle size={16}/><span>{label}</span></p>{events.length>0&&<button type="button" aria-expanded={open} onClick={()=>setOpen(value=>!value)} aria-label={open?'Recolher histórico recente da conversa':'Ver histórico recente da conversa'}>{open?<Minus size={14}/>:<Plus size={14}/>}</button>}</div>
    {open&&<section className="agent-recent-history" aria-label="Histórico recente desta conversa"><p>Histórico recente desta conversa</p><ol className="agent-step-list">{events.slice(-5).map(event=><li key={event.id}><Circle size={10} aria-hidden="true"/><span>{event.message}</span></li>)}</ol></section>}
  </section>;
}
