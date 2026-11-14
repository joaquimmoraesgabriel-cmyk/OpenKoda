import { useId, useLayoutEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { springAnimate } from './spring';
import './motion-controls.css';

function Tag({value,onRemove}:{value:string;onRemove:()=>void}) {
  const tag=useRef<HTMLSpanElement>(null);
  useLayoutEffect(()=>{if(!tag.current)return;const a=springAnimate(tag.current,v=>({transform:`scale(${.8+.2*v})`,opacity:Math.min(1,Math.max(0,v))}),[420,16]);return()=>a?.cancel();},[]);
  return <span className="motion-tag" ref={tag}>{value}<button type="button" onClick={onRemove} aria-label={`Remover ${value}`}><X size={13}/></button></span>;
}

// Controlled by the owning form; this component never submits or saves rules.
export function TagInput({label,values,onChange,disabled=false,maxTags=12}:{label:string;values:string[];onChange:(values:string[])=>void;disabled?:boolean;maxTags?:number}) {
  const id=useId(),[draft,setDraft]=useState(''),[notice,setNotice]=useState('');
  function add() {
    const value=draft.trim();if(!value)return;
    if(values.includes(value)){setNotice('Esse item já está na lista.');return;}
    if(values.length>=maxTags){setNotice(`Limite de ${maxTags} itens.`);return;}
    onChange([...values,value]);setDraft('');setNotice('');
  }
  return <fieldset className="motion-tags" disabled={disabled}><legend>{label}</legend><div className="motion-tags-wrap">{values.map(value=><Tag key={value} value={value} onRemove={()=>onChange(values.filter(item=>item!==value))}/>)}<input id={id} aria-label={`Adicionar ${label.toLocaleLowerCase()}`} aria-describedby={id+'-hint'} value={draft} maxLength={80} placeholder="Adicionar…" onChange={event=>setDraft(event.target.value)} onKeyDown={event=>{if(event.nativeEvent.isComposing)return;if(event.key==='Enter'){event.preventDefault();add();}if(event.key==='Backspace'&&!draft){onChange(values.slice(0,-1));}}}/><button type="button" className="text-button" onClick={add} disabled={!draft.trim()}>Adicionar</button></div><small id={id+'-hint'}>Enter adiciona. Backspace em um campo vazio remove o último item.</small><span role="status">{notice}</span></fieldset>;
}
