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
const pointerMenu=matchMedia('(min-width:761px) and (hover:hover) and (pointer:fine)');
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
matchMedia('(min-width:761px)').addEventListener('change',()=>menuOpen(false));
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
// All content is visible before enhancement. Only offscreen material waits for
// intersection, and keyboard focus/hash navigation always reveals it immediately.
const sections=[...document.querySelectorAll('[data-motion], .story-card, .guide-directory li, .offer')];
const indexLinks=[...document.querySelectorAll('.lesson-index a')];
const visible=new Set(),played=new WeakSet(),letterHeadings=new WeakSet();
const activeAnimations=new Set();
function reveal(section,immediate=false){
 section.classList.remove('motion-pending');
 if(played.has(section))return;
 played.add(section);
 if(immediate||reducedMotion.matches)return;
 section.classList.add('entered');
 const title=section.matches('h2,h3')?section:section.querySelector('.feature-heading h2, :scope > h2, :scope > div > h2');
 if(!title||title.children.length||letterHeadings.has(title)||title.textContent.length>72||!Element.prototype.animate)return;
 letterHeadings.add(title);
 const original=title.textContent;
 const visual=document.createElement('span');visual.setAttribute('aria-hidden','true');
 const letters=[];
 for(const word of original.split(/(\s+)/)){
  if(/^\s+$/.test(word)){visual.append(document.createTextNode(word));continue;}
  const wordNode=document.createElement('span');wordNode.className='letter-word';
  for(const char of Array.from(word)){
   const letter=document.createElement('span');letter.className='letter-piece';letter.textContent=char;
   letters.push(letter);wordNode.append(letter);
  }
  visual.append(wordNode);
 }
 title.setAttribute('aria-label',original);title.replaceChildren(visual);
 for(const [index,letter] of letters.entries()){
  const animation=letter.animate([{opacity:0,filter:'blur(5px)',transform:'translateY(-.16em)'},{opacity:1,filter:'blur(0px)',transform:'translateY(0)'}],{
   duration:520,delay:index*13,easing:'cubic-bezier(.2,.7,.2,1)',fill:'backwards'});
  activeAnimations.add(animation);
  animation.finished.catch(()=>{}).finally(()=>activeAnimations.delete(animation));
 }
}
function revealTarget(target){
 if(!(target instanceof Element))return;
 for(const section of sections)if(section.contains(target)||target.contains(section))reveal(section,true);
}
function revealHash(){
 try{if(location.hash)revealTarget(document.getElementById(decodeURIComponent(location.hash.slice(1))));}catch{}
}
if('IntersectionObserver' in window && sections.length){
 const observer=new IntersectionObserver(entries=>{
  for(const entry of entries){
   if(entry.isIntersecting){visible.add(entry.target);reveal(entry.target);}
   else visible.delete(entry.target);
  }
  const current=sections.find(section=>visible.has(section)&&section.id);
  if(current)for(const link of indexLinks){
   if(link.hash==='#'+current.id)link.setAttribute('aria-current','location');
   else link.removeAttribute('aria-current');
  }
 },{rootMargin:'0px 0px -6% 0px',threshold:.02});
 for(const section of sections){
  if(!reducedMotion.matches&&section.getBoundingClientRect().top>innerHeight*.96)section.classList.add('motion-pending');
  else reveal(section,true);
  observer.observe(section);
 }
 document.addEventListener('focusin',event=>revealTarget(event.target));
 window.addEventListener('hashchange',revealHash);revealHash();
}
reducedMotion.addEventListener('change',event=>{
 if(event.matches){for(const animation of activeAnimations)animation.cancel();for(const section of sections)reveal(section,true);}
});

for(const example of document.querySelectorAll('[data-route-example]')){
 const button=example.querySelector('[data-route-run]'),status=example.querySelector('[data-route-status]');
 let busy=false;
 button.addEventListener('click',()=>{
  if(busy)return;busy=true;button.disabled=true;example.dataset.running='true';
  const own=example.dataset.routeExample==='propria';
  const stages=own?['O pedido usa a chave do seu provedor.','A resposta informa os tokens processados.','O uso aparece no histórico local. O provedor fatura sua conta.']:
   ['O serviço confere a assinatura e reserva a cota.','O pedido aguarda uma vaga e recebe a resposta.','A cota é conciliada com o uso informado. Confira o saldo em Plano e uso.'];
  if(reducedMotion.matches){status.textContent=stages.at(-1);busy=false;button.disabled=false;example.dataset.running='false';return;}
  status.textContent=stages[0];
  setTimeout(()=>{status.textContent=stages[1];},650);
  setTimeout(()=>{status.textContent=stages[2];busy=false;button.disabled=false;example.dataset.running='false';},1350);
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
