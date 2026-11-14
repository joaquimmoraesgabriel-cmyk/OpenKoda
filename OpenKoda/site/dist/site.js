const box=document.querySelector('#cookie-box');
document.documentElement.classList.add('nav-ready');
const menu=document.querySelector('#primary-nav'),toggle=document.querySelector('#menu-toggle');
const navGroups=[...menu.querySelectorAll('.nav-group')];
function closeGroups(except){for(const group of navGroups)if(group!==except)group.open=false;}
function menuOpen(open){
 menu.classList.toggle('is-open',open);
 toggle.setAttribute('aria-expanded',String(open));
 if(!open)closeGroups();
}
toggle.addEventListener('click',()=>{
 const open=toggle.getAttribute('aria-expanded')!=='true';
 menuOpen(open);
 if(open)menu.querySelector('summary').focus();
});
const pointerMenu=matchMedia('(min-width:901px) and (hover:hover) and (pointer:fine)');
for(const group of navGroups){
 const summary=group.querySelector('summary');
 let openedOnHover=false;
 group.addEventListener('pointerenter',event=>{
  if(!group.open&&pointerMenu.matches&&event.pointerType==='mouse'&&!navGroups.some(peer=>peer!==group&&peer.contains(document.activeElement))){
   closeGroups(group);group.open=true;openedOnHover=true;
  }
 });
 group.addEventListener('pointerleave',()=>{
  if(pointerMenu.matches&&!group.contains(document.activeElement))group.open=false;
 });
 summary.addEventListener('click',event=>{
  if(event.detail>0&&pointerMenu.matches&&group.open&&openedOnHover){event.preventDefault();summary.focus();}
  openedOnHover=false;
  closeGroups(group);
 });
 group.addEventListener('focusout',event=>{if(!group.contains(event.relatedTarget))group.open=false;});
}
document.addEventListener('keydown',event=>{
 if(event.key!=='Escape')return;
 const group=navGroups.find(item=>item.open&&item.contains(document.activeElement))??navGroups.find(item=>item.open);
 if(group){group.open=false;group.querySelector('summary').focus();}
 else if(toggle.getAttribute('aria-expanded')==='true'){menuOpen(false);toggle.focus();}
});
document.addEventListener('click',event=>{if(!event.target.closest('.site-header'))menuOpen(false);});
matchMedia('(min-width:901px)').addEventListener('change',()=>menuOpen(false));
const cookieSettings=document.querySelector('#cookies');
let openedFromFooter=false;
try{box.hidden=localStorage.getItem('koda.cookies')==='essential';}catch{box.hidden=false;}
cookieSettings.addEventListener('click',()=>{openedFromFooter=true;box.hidden=false;box.querySelector('button').focus();});
document.querySelector('#accept-essential').addEventListener('click',()=>{
 try{localStorage.setItem('koda.cookies','essential');}catch{}
 box.hidden=true;
 if(openedFromFooter)cookieSettings.focus();
});
for(const widget of document.querySelectorAll('[data-switcher]')){
 const nav=widget.querySelector('[data-switch-nav]');
 const tabs=[...nav.querySelectorAll('[data-panel]')];
 nav.setAttribute('role','tablist');
 function select(current){
  for(const tab of tabs){
   const active=tab===current;
   const panel=document.getElementById(tab.dataset.panel);
   tab.setAttribute('role','tab');
   tab.setAttribute('aria-selected',String(active));
   tab.tabIndex=active?0:-1;
   panel.hidden=!active;
   panel.setAttribute('role','tabpanel');
   panel.setAttribute('aria-labelledby',tab.id);
   panel.tabIndex=0;
  }
 }
 select(tabs.find(tab=>tab.getAttribute('aria-selected')==='true')??tabs[0]);
 tabs.forEach((tab,index)=>{
  tab.addEventListener('click',()=>select(tab));
  tab.addEventListener('keydown',event=>{
   const target={Home:0,End:tabs.length-1,ArrowRight:(index+1)%tabs.length,ArrowLeft:(index-1+tabs.length)%tabs.length}[event.key];
   if(target===undefined)return;
   event.preventDefault();select(tabs[target]);tabs[target].focus();
  });
 });
}

const reducedMotion=matchMedia('(prefers-reduced-motion:reduce)');
// Only visual illustrations animate. All descriptions remain readable at rest.
if('IntersectionObserver' in window&&!reducedMotion.matches){
 const visuals=new IntersectionObserver(entries=>{
  for(const entry of entries)if(entry.isIntersecting){entry.target.classList.add('visual-entered');visuals.unobserve(entry.target);}
 },{threshold:.16});
 document.querySelectorAll('[data-visual]').forEach(visual=>visuals.observe(visual));
}
for(const evidence of document.querySelectorAll('[data-evidence-example]')){
 const button=evidence.querySelector('[data-evidence-toggle]');
 const explanation=document.getElementById(button.getAttribute('aria-controls'));
 button.addEventListener('click',()=>{
  const open=button.getAttribute('aria-expanded')!=='true';
  button.setAttribute('aria-expanded',String(open));explanation.hidden=!open;
  evidence.dataset.open=String(open);
  button.textContent=open?'Fechar explicação':'Examinar esta linha';
 });
}
// Reading content is never hidden. The index follows the section being read.
const indexLinks=[...document.querySelectorAll('.lesson-index a')];
if('IntersectionObserver' in window && indexLinks.length){
 const visible=new Set();
 const targets=indexLinks.map(link=>document.getElementById(link.hash.slice(1))).filter(Boolean);
 const observer=new IntersectionObserver(entries=>{
  for(const entry of entries){if(entry.isIntersecting)visible.add(entry.target);else visible.delete(entry.target);}
  const current=targets.find(section=>visible.has(section));
  if(current)for(const link of indexLinks){
   if(link.hash==='#'+current.id)link.setAttribute('aria-current','location');
   else link.removeAttribute('aria-current');
  }
 },{rootMargin:'-12% 0px -45% 0px',threshold:0});
 targets.forEach(target=>observer.observe(target));
}

// A real capture remains alongside the explanation; scrolling changes only the
// selected stage. Touch and reduced-motion users choose with the same buttons.
for(const story of document.querySelectorAll('[data-review-story]')){
 const controls=[...story.querySelectorAll('[data-review-select]')];
 const captures=[...story.querySelectorAll('[data-review-capture]')];
 const steps=[...story.querySelectorAll('[data-review-step]')];
 const wide=matchMedia('(min-width:960px)');
 let current=-1,frame=0;
 function select(index){
  if(index===current)return;current=index;
  story.dataset.reviewCurrent=String(index);
  controls.forEach((control,i)=>control.setAttribute('aria-pressed',String(i===index)));
  captures.forEach((capture,i)=>{capture.hidden=i!==index;});
  steps.forEach((step,i)=>step.toggleAttribute('data-current',i===index));
 }
 select(0);story.classList.add('review-ready');
 controls.forEach((control,index)=>control.addEventListener('click',()=>select(index)));
 function update(){
  frame=0;
  if(!wide.matches||reducedMotion.matches)return;
  const bounds=story.getBoundingClientRect();
  if(bounds.top>innerHeight*.8||bounds.bottom<innerHeight*.2)return;
  let nearest=0,distance=Infinity;
  steps.forEach((step,index)=>{
   const box=step.getBoundingClientRect();
   const delta=Math.abs(box.top+Math.min(box.height,innerHeight)*.35-innerHeight*.48);
   if(delta<distance){nearest=index;distance=delta;}
  });
  select(nearest);
 }
 window.addEventListener('scroll',()=>{if(!frame)frame=requestAnimationFrame(update);},{passive:true});
 wide.addEventListener('change',update);
 reducedMotion.addEventListener('change',()=>{if(!reducedMotion.matches)update();});
}

for(const example of document.querySelectorAll('[data-route-example]')){
 const button=example.querySelector('[data-route-run]'),status=example.querySelector('[data-route-status]');
 let busy=false;
 button.addEventListener('click',()=>{
  if(busy)return;busy=true;button.disabled=true;example.dataset.running='true';
  const own=example.dataset.routeExample==='propria';
  const stages=own?['O pedido usa a chave do seu provedor.','A resposta informa os tokens processados.','O uso aparece no histórico local. O provedor fatura sua conta.']:
   ['O serviço confere a assinatura e reserva a cota.','O pedido aguarda uma vaga e recebe a resposta.','A cota é conciliada com o uso informado. Confira o saldo em Uso e limites.'];
  const stops=[...example.querySelectorAll('[data-route-stop]')];
  const stage=index=>{status.textContent=stages[index];stops.forEach((stop,i)=>{stop.dataset.reached=String(i<=index);stop.dataset.active=String(i===index);});};
  if(reducedMotion.matches){stage(2);busy=false;button.disabled=false;example.dataset.running='false';return;}
  stage(0);
  setTimeout(()=>stage(1),650);
  setTimeout(()=>{stage(2);busy=false;button.disabled=false;example.dataset.running='false';},1350);
 });
}
for(const example of document.querySelectorAll('[data-scope-example]')){
 const buttons=[...example.querySelectorAll('[data-scope]')];
 for(const button of buttons)button.addEventListener('click',()=>{
  const folder=button.dataset.scope==='folder';
  for(const peer of buttons)peer.setAttribute('aria-pressed',String(peer===button));
  const row=example.querySelector('[data-scope-file="worker"]');
  row.dataset.included=String(folder);row.querySelector('span').textContent=folder?'Incluído':'Fora do escopo';
  example.querySelector('[data-scope-message]').textContent=folder?'A revisão recebe api.py e math_worker.py. Agora é possível acompanhar a chamada entre os dois arquivos.':'A revisão recebe api.py. Uma chamada para outro arquivo ainda precisa de contexto.';
 });
}
for(const example of document.querySelectorAll('[data-quota-example]')){
 const slider=example.querySelector('[data-quota-range]');
 const update=()=>{
  const used=Math.max(0,Math.min(100,Number(slider.value)));
  example.querySelector('[data-quota-used]').textContent=used+'%';
  example.querySelector('[data-quota-left]').textContent=(100-used)+'%';
  example.querySelector('[data-quota-fill]').style.transform='scaleX('+(1-used/100)+')';
  example.dataset.low=String(used>=90);
  example.querySelector('[data-quota-note]').textContent=used===100?'A cota desta janela terminou. Consulte a data da próxima liberação antes de tentar outro pedido.':used>=90?'A cota está perto do fim. Confira o saldo e a data da próxima liberação.':'Ainda há cota para novos pedidos. O modelo e o tamanho do contexto determinam o consumo.';
 };
 slider.addEventListener('input',update);update();
}

// Soft edges describe horizontal overflow while keeping endpoints and focus clear.
for(const scroller of document.querySelectorAll('.switch-nav,.comparison-scroll')){
 const update=()=>{
  scroller.classList.toggle('fade-start',scroller.scrollLeft>3);
  scroller.classList.toggle('fade-end',scroller.scrollLeft+scroller.clientWidth<scroller.scrollWidth-3);
 };
 scroller.addEventListener('scroll',update,{passive:true});
 if('ResizeObserver' in window)new ResizeObserver(update).observe(scroller);
 scroller.addEventListener('focusin',()=>requestAnimationFrame(update));update();
}
