import {spring,springs,step,clamp} from './motion-core.js';

export function mountWordRoll(host,runtime){
 const words=host.dataset.wordRoll.split('|'),slot=host.querySelector('[data-word-slot]'),button=host.querySelector('[data-word-next]');
 slot.replaceChildren();slot.setAttribute('aria-hidden','true');
 let index=0,clock=0,live=[];const width=spring(0,springs.wordWidth);
 function show(immediate=false){
  for(const word of live){word.out=true;word.age=0;}
  const node=document.createElement('span');node.className='motion-word';node.setAttribute('aria-hidden','true');
  const letters=[...words[index]].map((text,i)=>{const element=document.createElement('span');element.textContent=text;node.append(element);return {element,i,y:spring(immediate?0:1,springs.word)};});
  slot.append(node);live.push({node,letters,out:false,age:0});
  // Rapid clicks retain just the previous and current word.
  while(live.length>2)live.shift().node.remove();
  width.t=node.offsetWidth;if(immediate)width.x=width.t;
  button.setAttribute('aria-label','Mostrar outro exemplo: '+words[(index+1)%words.length]);animation.request();
 }
 const animation=runtime.watch(host,(dt,staticMode)=>{
  if(!staticMode){clock+=dt;if(clock>=2.4){clock=0;index=(index+1)%words.length;show();}}
  let moving=step(width,dt,staticMode);slot.style.width=width.x+'px';
  for(const word of live){
   word.age+=dt;
   for(const letter of word.letters){
    if(staticMode||word.age>=(word.out?letter.i*.022:.06+letter.i*.028))letter.y.t=word.out?-1:0;
    moving=step(letter.y,dt,staticMode)||moving;letter.element.style.transform=`translateY(${letter.y.x*105}%)`;letter.element.style.opacity=String(clamp(1-Math.abs(letter.y.x)*.9,0,1));
   }
  }
  live=live.filter(word=>{if(word.out&&(staticMode||word.age>1.2)){word.node.remove();return false;}return true;});
  return moving;
 },true);
 const next=()=>{clock=0;index=(index+1)%words.length;show(runtime.isStatic());};button.addEventListener('click',next);show(true);
 const resize=()=>{const current=live.find(word=>!word.out);if(current){width.t=current.node.offsetWidth;animation.request();}};window.addEventListener('resize',resize,{passive:true});
 return()=>{button.removeEventListener('click',next);window.removeEventListener('resize',resize);animation.destroy();live.forEach(word=>word.node.remove());};
}

export function mountWeightWave(host,runtime){
 const text=host.textContent;host.replaceChildren();
 const readable=document.createElement('span');readable.className='sr-only';readable.textContent=text;
 const visual=document.createElement('span');visual.setAttribute('aria-hidden','true');host.append(readable,visual);
 const letters=[];
 text.split(' ').forEach((word,wordIndex)=>{
  if(wordIndex)visual.append(document.createTextNode(' '));
  const group=document.createElement('span');group.className='motion-wave-word';visual.append(group);
  for(const char of word){const element=document.createElement('span');element.textContent=char;group.append(element);letters.push({element,index:letters.length,w:spring(400,springs.weight)});}
 });
 const focus=spring(-.2,springs.focus);let hover=false,clock=0;
 const animation=runtime.watch(host,(dt,staticMode)=>{
  if(!staticMode){clock+=dt;if(!hover)focus.t=-.06+1.12*(.5-.5*Math.cos(clock*Math.PI*2/4.6));}
  step(focus,dt,staticMode);
  for(const letter of letters){const distance=((letter.index+.5)/letters.length-focus.x)/.13;letter.w.t=staticMode?600:400+380*Math.exp(-distance*distance);step(letter.w,dt,staticMode);letter.element.style.fontWeight=String(clamp(letter.w.x,400,800));}
  return false;
 },true);
 const move=event=>{if(event.pointerType==='touch'||runtime.isStatic())return;const rect=host.getBoundingClientRect();hover=true;focus.t=(event.clientX-rect.left)/rect.width;animation.request();};
 const leave=()=>{hover=false;};host.addEventListener('pointermove',move,{passive:true});host.addEventListener('pointerleave',leave,{passive:true});
 return()=>{host.removeEventListener('pointermove',move);host.removeEventListener('pointerleave',leave);animation.destroy();host.textContent=text;};
}

export function mountFan(host,runtime){
 const cards=[...host.querySelectorAll('a')].map((element,index)=>({element,index,lift:spring(0,springs.lift),scale:spring(1,springs.scale)}));
 const fan=spring(0,springs.fan);let hot=-1,inside=false,clock=0,sequence=0,wait=1.4,compact=host.clientWidth<400;
 function select(index){hot=index;for(const card of cards){card.lift.t=card.index===hot?-22:0;card.scale.t=card.index===hot?1.06:1;card.element.style.zIndex=String(card.index===hot?5:card.index===1?3:2);}animation.request();}
 const animation=runtime.watch(host,(dt,staticMode)=>{
  let moving=step(fan,dt,staticMode);
  if(!staticMode&&!compact&&!inside&&!host.contains(document.activeElement)){
   clock+=dt;
   if(clock>=wait){clock=0;if(sequence===0)fan.t=1;else if(sequence<4)select(sequence-1);else if(sequence===4)select(-1);else fan.t=0;sequence=(sequence+1)%6;wait=sequence===0?1.4:.75;}
  }
  for(const card of cards){moving=step(card.lift,dt,staticMode)||moving;moving=step(card.scale,dt,staticMode)||moving;const d=card.index-1;
   card.element.style.transform=staticMode||compact?'none':`translate(${d*(6+fan.x*72)}px,${Math.abs(d)*fan.x*12+card.lift.x}px) rotate(${d*(3+fan.x*9)}deg) scale(${card.scale.x})`;
  }
  host.classList.toggle('motion-fan-static',staticMode||compact);return moving;
 },!compact);
 const enter=()=>{inside=true;fan.t=1;animation.request();},leave=()=>{inside=false;if(!host.contains(document.activeElement)){fan.t=0;select(-1);}};
 host.addEventListener('pointerenter',enter);host.addEventListener('pointerleave',leave);
 const handlers=[];
 for(const card of cards){const point=()=>{if(inside)select(card.index);},focus=()=>{fan.t=1;select(card.index);},blur=()=>{queueMicrotask(leave);};card.element.addEventListener('pointerenter',point);card.element.addEventListener('focus',focus);card.element.addEventListener('blur',blur);handlers.push(()=>{card.element.removeEventListener('pointerenter',point);card.element.removeEventListener('focus',focus);card.element.removeEventListener('blur',blur);});}
 const resize=()=>{compact=host.clientWidth<400;animation.setContinuous(!compact);animation.request();};window.addEventListener('resize',resize,{passive:true});host.classList.add('motion-fan-ready');select(-1);
 return()=>{animation.destroy();host.removeEventListener('pointerenter',enter);host.removeEventListener('pointerleave',leave);window.removeEventListener('resize',resize);handlers.forEach(fn=>fn());host.classList.remove('motion-fan-ready');cards.forEach(card=>card.element.style.removeProperty('transform'));};
}
