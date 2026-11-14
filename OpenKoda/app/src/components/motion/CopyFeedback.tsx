import { Check, Copy } from 'lucide-react';
import { useLayoutEffect, useRef } from 'react';
import { springAnimate, springs } from './spring';
import './motion-controls.css';

export function CopyFeedback({copied,label='Copiar',done='Copiado'}:{copied:boolean;label?:string;done?:string}) {
  const icon=useRef<HTMLSpanElement>(null),word=useRef<HTMLSpanElement>(null),labelBox=useRef<HTMLSpanElement>(null),width=useRef(0),first=useRef(true);
  useLayoutEffect(()=>{
    const next=word.current?.getBoundingClientRect().width??0,before=width.current||next;width.current=next;
    if(labelBox.current)labelBox.current.style.width=next+'px';
    if(first.current){first.current=false;return;}
    const a=icon.current&&springAnimate(icon.current,v=>({opacity:Math.min(1,Math.max(0,v*1.6)),transform:`scale(${.4+.6*v}) rotate(${(1-v)*(copied?-40:40)}deg)`}),springs.copyIcon);
    const b=word.current&&springAnimate(word.current,v=>({transform:`translateY(${(1-v)*(copied?18:-18)}px)`,opacity:Math.min(1,Math.max(0,v))}),springs.copyLabel);
    const c=labelBox.current&&springAnimate(labelBox.current,v=>({width:before+(next-before)*v+'px'}),springs.copyWidth);
    return()=>{a?.cancel();b?.cancel();c?.cancel();};
  },[copied]);
  return <><span className="motion-copy-icon" ref={icon} aria-hidden="true">{copied?<Check size={15}/>:<Copy size={15}/>}</span><span className="motion-copy-label" ref={labelBox}><span ref={word}>{copied?done:label}</span></span></>;
}
