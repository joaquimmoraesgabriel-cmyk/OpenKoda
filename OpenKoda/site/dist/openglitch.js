// The first paint is always the complete static artwork. Canvas is progressive.
// Recompose a static fallback when Canvas is unavailable at unusual aspect ratios.
import {renderMatterSvg} from './matter-field.js';
const cleanups=[];
function mount(){
 for(const section of document.querySelectorAll('.og-opening[data-matter-section], .og-closing[data-matter-section]')){
  const canvas=section.querySelector('canvas');
  const fallback=document.createElement('div');
  fallback.className='og-runtime-fallback'; fallback.setAttribute('aria-hidden','true');
  canvas.before(fallback);
  let timer;
  function update(){
   clearTimeout(timer);
   timer=setTimeout(()=>{
    if(section.dataset.matterReady==='true'){fallback.replaceChildren();delete section.dataset.fallbackReady;return;}
    const box=canvas.getBoundingClientRect();if(box.width<2||box.height<2)return;
    const style=getComputedStyle(section);
    const svg=new DOMParser().parseFromString(renderMatterSvg({width:box.width,height:box.height,kind:canvas.dataset.matterField,
     ink:style.getPropertyValue('--matter-ink').trim(),accent:style.getPropertyValue('--matter-accent').trim()}),'image/svg+xml').documentElement;
    // Merge marks into equal-opacity paths, keeping the fallback DOM bounded.
    for(const group of svg.children){
     const paths=new Map();
     for(const mark of group.children){
      const opacity=mark.getAttribute('opacity'); let path;
      if(mark.localName==='circle'){
       const x=Number(mark.getAttribute('cx')),y=Number(mark.getAttribute('cy')),r=Number(mark.getAttribute('r'));
       path=`M${x-r} ${y}a${r} ${r} 0 1 0 ${r*2} 0a${r} ${r} 0 1 0 ${-r*2} 0Z`;
      }else{
       const x=mark.getAttribute('x'),y=mark.getAttribute('y'),w=mark.getAttribute('width'),h=mark.getAttribute('height');
       path=`M${x} ${y}h${w}v${h}h-${w}Z`;
      }
      paths.set(opacity,(paths.get(opacity)||'')+path);
     }
     group.replaceChildren(...[...paths].map(([opacity,d])=>{const path=svg.ownerDocument.createElementNS(svg.namespaceURI,'path');path.setAttribute('d',d);path.setAttribute('opacity',opacity);return path;}));
    }
    svg.setAttribute('preserveAspectRatio','none');fallback.replaceChildren(svg);section.dataset.fallbackReady='true';
   },100);
  }
  const resize=new ResizeObserver(update);resize.observe(canvas);
  const theme=new MutationObserver(update);theme.observe(document.documentElement,{attributes:true,attributeFilter:['data-appearance']});theme.observe(section,{attributes:true,attributeFilter:['data-matter-ready']});
  const media=matchMedia('(prefers-color-scheme: dark)');media.addEventListener('change',update);update();
  cleanups.push(()=>{clearTimeout(timer);resize.disconnect();theme.disconnect();media.removeEventListener('change',update);fallback.remove();delete section.dataset.fallbackReady;});
 }
}
mount();
window.addEventListener('pagehide',()=>{cleanups.splice(0).forEach(fn=>fn());});
window.addEventListener('pageshow',event=>{if(event.persisted)mount();});
