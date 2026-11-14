import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
const source=await readFile(new URL('../dist/motion-core.js',import.meta.url),'utf8');
const {spring,springs,step,retargetEdges,createCopyAction,createMotionRuntime,pauseKey,pauseEvent}=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));

test('rapid plan selection retargets both edges without resetting velocity or losing its final selection',()=>{
 const left=spring(0),right=spring(100);
 retargetEdges(left,right,200,300);
 assert.deepEqual([right.k,right.c],springs.lead);assert.deepEqual([left.k,left.c],springs.trail);
 for(let i=0;i<5;i++){step(left,1/60);step(right,1/60);}
 const previous={x:left.x,v:left.v};retargetEdges(left,right,0,100);
 assert.equal(left.x,previous.x);assert.equal(left.v,previous.v);
 assert.deepEqual([left.k,left.c],springs.lead);
 for(let i=0;i<240;i++){step(left,1/60);step(right,1/60);assert.ok(Number.isFinite(left.x+right.x));assert.ok(right.x>left.x);}
 assert.equal(left.x,0);assert.equal(right.x,100);
});

test('reduced motion snaps to the actual selected state and long frame gaps are bounded',()=>{
 const a=spring(0,springs.word),b=spring(0,springs.word);a.t=b.t=1;
 step(a,30);step(b,1/30);assert.deepEqual(a,b);
 assert.equal(step(a,1/60,true),false);assert.equal(a.x,1);assert.equal(a.v,0);
});

test('copy success is reported only after the write resolves and overlapping requests do not duplicate writes',async()=>{
 let complete,writes=0;const states=[];
 const action=createCopyAction(()=>{writes++;return new Promise(resolve=>{complete=resolve;});},state=>states.push(state));
 const first=action.run('code');assert.deepEqual(states,['pending']);
 assert.equal(await action.run('duplicate'),false);assert.equal(writes,1);
 complete();assert.equal(await first,true);assert.deepEqual(states,['pending','success']);
});

test('clipboard rejection or missing permissions never produces a copied confirmation',async()=>{
 for(const writer of [undefined,async()=>{throw Error('denied');}]){
  const states=[],action=createCopyAction(writer,state=>states.push(state));
  assert.equal(await action.run('code'),false);assert.deepEqual(states,['pending','error']);
 }
});

test('unmount cancels stale clipboard feedback without hiding a later request',async()=>{
 let complete;const states=[],action=createCopyAction(()=>new Promise(resolve=>{complete=resolve;}),state=>states.push(state));
 const pending=action.run('first');action.cancel();complete();await pending;assert.deepEqual(states,['pending']);
 const second=action.run('second');complete();await second;assert.deepEqual(states,['pending','pending','success']);
});

class Events {
 listeners=new Map();
 addEventListener(name,fn){if(!this.listeners.has(name))this.listeners.set(name,new Set());this.listeners.get(name).add(fn);}
 removeEventListener(name,fn){this.listeners.get(name)?.delete(fn);}
 dispatchEvent(event){for(const fn of this.listeners.get(event.type)||[])fn(event);}
 count(){return [...this.listeners.values()].reduce((total,set)=>total+set.size,0);}
}
function fixture(){
 const win=new Events(),doc=new Events(),media=new Events(),frames=new Map(),store=new Map();let id=0,time=0;
 win.document=doc;doc.hidden=false;media.matches=false;win.matchMedia=()=>media;
 win.localStorage={getItem:key=>store.get(key),setItem:(key,value)=>store.set(key,value)};
 win.CustomEvent=class{constructor(type,options={}){this.type=type;this.detail=options.detail;}};
 win.requestAnimationFrame=fn=>{frames.set(++id,fn);return id;};win.cancelAnimationFrame=id=>frames.delete(id);
 const flush=()=>{time+=1000/60;const list=[...frames.values()];frames.clear();list.forEach(fn=>fn(time));};
 return {win,doc,media,frames,store,flush};
}
test('UI animations pause together, remain stopped when hidden and release all resources',()=>{
 const env=fixture(),runtime=createMotionRuntime(env.win);let renders=0;
 const job=runtime.watch({isConnected:true},()=>{renders++;return false;},true);
 env.flush();env.flush();assert.equal(renders,2);assert.equal(env.frames.size,1);
 job.setContinuous(false);assert.equal(env.frames.size,0);job.request();env.flush();assert.equal(renders,3);assert.equal(env.frames.size,0);
 job.setContinuous(true);env.flush();assert.equal(env.frames.size,1);
 runtime.togglePause();env.flush();assert.equal(env.store.get(pauseKey),'true');assert.equal(env.frames.size,0);
 env.win.dispatchEvent(new env.win.CustomEvent(pauseEvent,{detail:{paused:false}}));env.flush();assert.equal(env.frames.size,1);
 env.doc.hidden=true;env.doc.dispatchEvent({type:'visibilitychange'});assert.equal(env.frames.size,0);
 env.doc.hidden=false;env.doc.dispatchEvent({type:'visibilitychange'});assert.equal(env.frames.size,1);
 env.media.matches=true;env.media.dispatchEvent({type:'change'});env.flush();assert.equal(env.frames.size,0);
 job.destroy();runtime.destroy();assert.equal(env.frames.size,0);assert.equal(env.win.count()+env.doc.count()+env.media.count(),0);
});
