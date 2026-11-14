import {createMotionRuntime} from './motion-core.js';
import {mountPlanSwitch,mountCopy} from './motion-controls.js';
import {mountWordRoll,mountWeightWave,mountFan} from './motion-editorial.js';

let runtime;const mounted=new Map();
function pauseControl(button){
 const refresh=()=>{button.disabled=runtime.reduced();button.setAttribute('aria-pressed',String(runtime.paused()));button.textContent=runtime.reduced()?'Movimento reduzido':runtime.paused()?'Retomar animações':'Pausar animações';};
 const click=()=>runtime.togglePause();button.addEventListener('click',click);const stop=runtime.onMode(refresh);refresh();
 return()=>{button.removeEventListener('click',click);stop();};
}
export function enhanceMotion(root=document){
 runtime??=createMotionRuntime(window);
 for(const [element,cleanup] of mounted)if(!element.isConnected){cleanup();mounted.delete(element);}
 const patterns=[['[data-plan-switch]',mountPlanSwitch],['pre:has(code)',mountCopy],['[data-word-roll]',mountWordRoll],['[data-weight-wave]',mountWeightWave],['[data-fan-guides]',mountFan],['[data-ui-pause]',pauseControl]];
 for(const [selector,mount] of patterns)for(const element of root.querySelectorAll(selector))if(!mounted.has(element))mounted.set(element,mount(element,runtime));
}
function cleanup(){for(const stop of mounted.values())stop();mounted.clear();runtime?.destroy();runtime=null;}
if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',()=>enhanceMotion(),{once:true});else enhanceMotion();
window.addEventListener('pagehide',cleanup);
window.addEventListener('pageshow',event=>{if(event.persisted)enhanceMotion();});
