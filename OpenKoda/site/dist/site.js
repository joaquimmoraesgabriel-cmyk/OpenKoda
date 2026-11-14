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
