import {clamp} from './motion-core.js';

const GLYPHS='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789#%&*+=/<>';
const hash=i=>{const n=Math.sin((i+1)*129.13)*43758.5453;return n-Math.floor(n);};

// Decode once on first visibility. The original sentence stays available to AT;
// fixed per-letter geometry and word wrapping avoid layout shift during decoding.
export function mountScramble(host,runtime){
 const text=host.textContent;host.replaceChildren();
 const accessible=document.createElement('span');accessible.className='sr-only';accessible.textContent=text;
 const visual=document.createElement('span');visual.setAttribute('aria-hidden','true');host.append(accessible,visual);
 const cells=[];let index=0,clock=0;
 for(const word of text.split(/(\s+)/)){
  if(/^\s+$/.test(word)){visual.append(document.createTextNode(word));index+=word.length;continue;}
  const group=document.createElement('span');group.className='motion-scramble-word';visual.append(group);
  for(const char of word){
   const cell=document.createElement('span'),measure=document.createElement('span'),glyph=document.createElement('span');
   cell.className='motion-scramble-cell';measure.className='motion-scramble-measure';glyph.className='motion-scramble-glyph';
   measure.textContent=glyph.textContent=char;cell.append(measure,glyph);group.append(cell);
   cells.push({glyph,char,index,at:.18+index*.035+hash(index)*.28,last:-1,done:false});index++;
  }
 }
 const animation=runtime.watch(host,(dt,staticMode)=>{
  clock+=dt;let active=false;
  for(const cell of cells){
   if(cell.done)continue;
   if(staticMode||clock>=cell.at){cell.done=true;cell.glyph.textContent=cell.char;cell.glyph.classList.add('settled');}
   else{
    active=true;const tick=Math.floor(clock/.055);
    if(tick!==cell.last){cell.last=tick;cell.glyph.textContent=GLYPHS[Math.floor(hash(cell.index+tick*131)*GLYPHS.length)];}
   }
  }
  return active;
 });
 return()=>{animation.destroy();host.textContent=text;};
}

let gooId=0;
export function mountGooMorph(host,runtime){
 const words=host.dataset.gooMorph.split('|'),original=host.textContent;
 const svg=document.createElementNS('http://www.w3.org/2000/svg','svg'),filter=document.createElementNS(svg.namespaceURI,'filter'),matrix=document.createElementNS(svg.namespaceURI,'feColorMatrix');
 const id='og-text-melt-'+(++gooId);svg.setAttribute('width','0');svg.setAttribute('height','0');svg.setAttribute('aria-hidden','true');svg.classList.add('motion-goo-filter');filter.id=id;
 matrix.setAttribute('in','SourceGraphic');matrix.setAttribute('type','matrix');matrix.setAttribute('values','1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 255 -140');filter.append(matrix);svg.append(filter);
 const a=document.createElement('span'),b=document.createElement('span');a.className=b.className='motion-goo-word';a.textContent=words[0];
 host.replaceChildren(svg,a,b);host.classList.add('motion-goo-ready');let current=0,next=0,time=0,hold=1.6,melting=false;
 const look=(element,f)=>{element.style.filter=f>=1?'none':`blur(${Math.min(8/Math.max(.001,f)-8,100)}px)`;element.style.opacity=String(Math.pow(f,.4));};
 const animation=runtime.watch(host,(dt,staticMode)=>{
  if(staticMode){melting=false;time=0;hold=1.6;a.textContent=words[current];b.textContent='';}
  else if(melting){time+=dt/1.05;if(time>=1){current=next;melting=false;hold=1.6;a.textContent=words[current];b.textContent='';}}
  else if((hold-=dt)<=0){next=(current+1)%words.length;time=0;melting=true;b.textContent=words[next];}
  host.style.filter=melting?`url(#${id})`:'none';
  if(melting){const v=clamp(time,0,1),f=v*v*(3-2*v);look(a,1-f);look(b,f);}
  else{look(a,1);b.style.opacity='0';}
  return false;
 },true);
 return()=>{animation.destroy();host.style.removeProperty('filter');host.classList.remove('motion-goo-ready');host.textContent=original;};
}
