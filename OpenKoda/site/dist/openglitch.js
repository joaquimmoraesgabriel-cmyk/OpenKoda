// The first paint is always the complete static artwork. Canvas is progressive.
// Recompose a static fallback when Canvas is unavailable at unusual aspect ratios.
import {renderMatterSvg} from './matter-field.js';
const cleanups=[];
function mount(){
 for(const section of document.querySelectorAll('.og-opening[data-matter-section]')){
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
    const svg=new DOMParser().parseFromString(renderMatterSvg({width:box.width,height:box.height,kind:'aperture',
     ink:style.getPropertyValue('--matter-ink').trim(),accent:style.getPropertyValue('--matter-accent').trim()}),'image/svg+xml').documentElement;
    // Merge squares into equal-opacity paths, keeping the fallback DOM bounded.
    for(const group of svg.children){
     const paths=new Map();
     for(const rect of group.children){const opacity=rect.getAttribute('opacity');const x=rect.getAttribute('x'),y=rect.getAttribute('y'),w=rect.getAttribute('width'),h=rect.getAttribute('height');paths.set(opacity,(paths.get(opacity)||'')+`M${x} ${y}h${w}v${h}h-${w}Z`);}
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
