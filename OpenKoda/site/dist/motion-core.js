// Original integration using the stiffness/damping pairs in the supplied references.
export const springs = Object.freeze({lead:[620,36],trail:[190,19],pop:[520,16],fade:[420,32],soft:[260,24],copy:[260,20],copyIcon:[420,18],copyWidth:[300,24],word:[420,26],wordWidth:[320,30],fan:[220,18],lift:[420,20],scale:[420,22],weight:[260,22],focus:[150,22]});
export const pauseKey = 'openglitch.motion-paused';
export const pauseEvent = 'openglitch:motion-change';
export const clamp = (n,a,b) => Math.max(a,Math.min(b,n));
export const spring = (x,pair=springs.soft) => ({x,t:x,v:0,k:pair[0],c:pair[1]});
export function step(s,elapsed,staticMode=false){
 if(staticMode){s.x=s.t;s.v=0;return false;}
 const dt=clamp(Number.isFinite(elapsed)?elapsed:0,0,1/30)/4;
 for(let i=0;i<4;i++){s.v+=(-s.k*(s.x-s.t)-s.c*s.v)*dt;s.x+=s.v*dt;}
 if(Math.abs(s.x-s.t)<.001&&Math.abs(s.v)<.001){s.x=s.t;s.v=0;return false;}
 return true;
}
export function retargetEdges(left,right,start,end,immediate=false){
 const forward=start>=left.t;
 [left.k,left.c]=forward?springs.trail:springs.lead;
 [right.k,right.c]=forward?springs.lead:springs.trail;
 left.t=start;right.t=end;
 if(immediate){left.x=start;right.x=end;left.v=right.v=0;}
}
export function createCopyAction(write,onState){
 let pending=false,sequence=0;
 return {async run(text){
  if(pending)return false;
  pending=true;const id=++sequence;onState('pending');
  try{if(typeof write!=='function')throw Error('Clipboard unavailable');await write(text);if(id===sequence)onState('success');return true;}
  catch{if(id===sequence)onState('error');return false;}
  finally{if(id===sequence)pending=false;}
 },cancel(){sequence++;pending=false;}};
}
export function createMotionRuntime(win){
 const doc=win.document,reduce=win.matchMedia('(prefers-reduced-motion: reduce)');
 const entries=new Set(),listeners=new Set();let id=0,last=0,disposed=false,paused=false;
 function readPause(){try{return win.localStorage.getItem(pauseKey)==='true';}catch{return paused;}}
 paused=readPause();
 const isStatic=()=>reduce.matches||paused;
 function needed(e){return e.visible&&e.element.isConnected&&(e.pending||e.continuous&&!isStatic());}
 function schedule(){
  const active=!disposed&&!doc.hidden&&[...entries].some(needed);
  if(active&&!id)id=win.requestAnimationFrame(frame);
  else if(!active&&id){win.cancelAnimationFrame(id);id=0;last=0;}
 }
 function frame(now){
  id=0;const dt=last?Math.min((now-last)/1000,1/30):1/60;last=now;
  for(const e of entries)if(needed(e))e.pending=!!e.tick(dt,isStatic());
  schedule();
 }
 const observer=win.IntersectionObserver?new win.IntersectionObserver(changes=>{
  for(const change of changes)for(const e of entries)if(e.element===change.target){e.visible=change.isIntersecting;e.pending=true;}
  last=0;schedule();
 }):null;
 function mode(event){paused=event?.type===pauseEvent&&typeof event.detail?.paused==='boolean'?event.detail.paused:readPause();for(const e of entries)e.pending=true;for(const fn of listeners)fn();last=0;schedule();}
 function visibility(){last=0;schedule();}
 const storage=event=>{if(event.key===pauseKey||event.key===null)mode();};
 reduce.addEventListener('change',mode);win.addEventListener(pauseEvent,mode);win.addEventListener('storage',storage);doc.addEventListener('visibilitychange',visibility);
 return {isStatic,paused:()=>paused,reduced:()=>reduce.matches,
  togglePause(){paused=!paused;try{win.localStorage.setItem(pauseKey,String(paused));}catch{}win.dispatchEvent(new win.CustomEvent(pauseEvent,{detail:{paused}}));},
  onMode(fn){listeners.add(fn);return()=>listeners.delete(fn);},
  watch(element,tick,continuous=false){
   const e={element,tick,continuous,visible:!observer,pending:true};entries.add(e);observer?.observe(element);schedule();
   return {request(){e.pending=true;schedule();},setContinuous(value){e.continuous=!!value;schedule();},destroy(){entries.delete(e);observer?.unobserve(element);schedule();}};
  },
  destroy(){disposed=true;if(id)win.cancelAnimationFrame(id);observer?.disconnect();entries.clear();listeners.clear();reduce.removeEventListener('change',mode);win.removeEventListener(pauseEvent,mode);win.removeEventListener('storage',storage);doc.removeEventListener('visibilitychange',visibility);}
 };
}
