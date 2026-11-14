const box=document.querySelector('#cookie-box');
document.documentElement.classList.add('nav-ready');
const menu=document.querySelector('#primary-nav'),toggle=document.querySelector('#menu-toggle');
function menuOpen(open){menu.classList.toggle('is-open',open);toggle.setAttribute('aria-expanded',String(open));}
toggle.addEventListener('click',()=>menuOpen(toggle.getAttribute('aria-expanded')!=='true'));
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&toggle.getAttribute('aria-expanded')==='true'){menuOpen(false);toggle.focus();}});
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
const viewButtons=[...document.querySelectorAll('[data-view]')];
for(const button of viewButtons){
 const panelId='demo-'+button.dataset.view;
 button.setAttribute('aria-controls',panelId);
 document.getElementById(panelId).hidden=button.getAttribute('aria-pressed')!=='true';
 button.addEventListener('click',()=>{
  if(button.getAttribute('aria-pressed')==='true')return;
  for(const peer of viewButtons){
   const active=peer===button;
   const view=document.getElementById(peer.getAttribute('aria-controls'));
   peer.setAttribute('aria-pressed',String(active));
   view.hidden=!active;
   view.classList.toggle('switched',active);
  }
 });
}

const sections=document.querySelectorAll('[data-motion]');
if('IntersectionObserver' in window && sections.length){
 const indexLinks=[...document.querySelectorAll('.lesson-index a')];
 const visible=new Set(),played=new WeakSet();
 const observer=new IntersectionObserver(entries=>{
  for(const entry of entries){
   if(entry.isIntersecting){
    visible.add(entry.target);
    if(!played.has(entry.target)){entry.target.classList.add('entered');played.add(entry.target);}
   }else visible.delete(entry.target);
  }
  const current=[...sections].find(section=>visible.has(section)&&section.id);
  if(current)for(const link of indexLinks){
   if(link.hash==='#'+current.id)link.setAttribute('aria-current','location');
   else link.removeAttribute('aria-current');
  }
 },{rootMargin:'0px 0px -10% 0px'});
 for(const section of sections)observer.observe(section);
}
