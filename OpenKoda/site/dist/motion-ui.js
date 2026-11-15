import {createMotionRuntime} from './motion-core.js';
import {mountPlanSwitch,mountCopy} from './motion-controls.js';
import {mountWordRoll,mountWeightWave,mountFan} from './motion-editorial.js';
import {mountScramble,mountGooMorph} from './motion-type.js';

let runtime;const mounted=new Map();
export function enhanceMotion(root=document){
 runtime??=createMotionRuntime(window);
 for(const [element,cleanup] of mounted)if(!element.isConnected){cleanup();mounted.delete(element);}
 const patterns=[['[data-plan-switch]',mountPlanSwitch],['pre:has(code)',mountCopy],['[data-word-roll]',mountWordRoll],['[data-weight-wave]',mountWeightWave],['[data-fan-guides]',mountFan],['[data-scramble-text]',mountScramble],['[data-goo-morph]',mountGooMorph]];
 for(const [selector,mount] of patterns)for(const element of root.querySelectorAll(selector))if(!mounted.has(element))mounted.set(element,mount(element,runtime));
}
function cleanup(){for(const stop of mounted.values())stop();mounted.clear();runtime?.destroy();runtime=null;}
if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',()=>enhanceMotion(),{once:true});else enhanceMotion();
window.addEventListener('pagehide',cleanup);
window.addEventListener('pageshow',event=>{if(event.persisted)enhanceMotion();});
