import { useLayoutEffect, useRef } from 'react';
import { reducedMotion, respectMotionPreference, springSamples, springs } from './spring';
import './motion-controls.css';

export function PlanSwitch({items,value,onChange}:{items:{id:string;label:string}[];value:string;onChange:(id:string)=>void}) {
  const group=useRef<HTMLDivElement>(null),highlight=useRef<HTMLSpanElement>(null);
  const previous=useRef<{left:number;right:number}|undefined>(undefined);
  useLayoutEffect(()=>{
    const root=group.current,marker=highlight.current;if(!root||!marker)return;
    let animation:Animation|undefined;
    const update=(animate:boolean)=>{
      const selected=root.querySelector<HTMLButtonElement>('[aria-checked=true]');if(!selected)return;
      const target={left:selected.offsetLeft,right:selected.offsetLeft+selected.offsetWidth},before=previous.current??target;
      marker.style.left=target.left+'px';marker.style.width=(target.right-target.left)+'px';
      animation?.cancel();
      if(animate&&!reducedMotion()&&marker.animate){
        const movingRight=target.left>before.left;
        const leading=springSamples(...springs.lead),trailing=springSamples(...springs.trail);
        const samples=Math.max(leading.length,trailing.length);
        const frames=Array.from({length:samples},(_,i)=>{
          const lead=leading[Math.min(i,leading.length-1)],trail=trailing[Math.min(i,trailing.length-1)];
          const left=before.left+(target.left-before.left)*(movingRight?trail:lead);
          const right=before.right+(target.right-before.right)*(movingRight?lead:trail);
          return {left:left+'px',width:Math.max(1,right-left)+'px'};
        });
        animation=respectMotionPreference(marker.animate(frames,{duration:(samples-1)*1000/60,easing:'linear'}));
      }
      previous.current=target;
    };
    update(true);let width=root.clientWidth;
    const observer=new ResizeObserver(()=>{if(root.clientWidth!==width){width=root.clientWidth;update(false);}});observer.observe(root);
    return()=>{animation?.cancel();observer.disconnect();};
  },[value,items.length]);
  return <div className="motion-plan-switch" ref={group} role="radiogroup" aria-label="Comparar planos" onKeyDown={event=>{
    const current=items.findIndex(item=>item.id===value);
    const next=event.key==='ArrowRight'?(current+1)%items.length:event.key==='ArrowLeft'?(current-1+items.length)%items.length:event.key==='Home'?0:event.key==='End'?items.length-1:undefined;
    if(next===undefined||!items[next])return;event.preventDefault();onChange(items[next].id);group.current?.querySelectorAll<HTMLButtonElement>('button')[next]?.focus();
  }}><span ref={highlight} className="motion-plan-highlight" aria-hidden="true"/>{items.map(item=><button key={item.id} type="button" role="radio" aria-checked={item.id===value} tabIndex={item.id===value?0:-1} onClick={()=>onChange(item.id)}>{item.label}</button>)}</div>;
}
