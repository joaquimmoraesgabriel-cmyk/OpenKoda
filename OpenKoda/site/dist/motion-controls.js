import {spring,springs,step,clamp,retargetEdges,createCopyAction} from './motion-core.js';

export function mountPlanSwitch(group,runtime){
 const native=group.matches('fieldset'),buttons=[...group.querySelectorAll(native?'input[type="radio"]':'[role="tab"]')];
 if(!buttons.length)return()=>{};
 const highlight=document.createElement('span');highlight.className='motion-plan-highlight';highlight.setAttribute('aria-hidden','true');group.prepend(highlight);
 const left=spring(0),right=spring(0);let first=true;
 const output=group.parentElement.querySelector('[data-plan-preview]');
 const drums=[];let separator;
 if(output){
  output.replaceChildren();const currency=document.createElement('span');currency.className='motion-price-currency';currency.textContent='R$';output.append(currency);
  for(let i=0;i<4;i++){
   const column=document.createElement('span'),strip=document.createElement('span');column.className='motion-digit-column';strip.className='motion-digit-strip';
   for(let digit=0;digit<10;digit++){const cell=document.createElement('span');cell.textContent=String(digit);strip.append(cell);}
   column.append(strip);output.append(column);drums.push({column,strip,y:spring(0,[260,22+i*2]),width:spring(1,springs.soft),delay:0,digit:0});
   if(i===0){separator=document.createElement('span');separator.className='motion-price-separator';separator.textContent='.';output.append(separator);}
  }
 }
 let current=buttons.find(button=>native?button.checked:button.getAttribute('aria-selected')==='true')||buttons[0];
 const registrations=[];
 function select(button,immediate=false){
  current=button;
  if(!native){
   for(const entry of buttons){const selected=entry===button;entry.setAttribute('aria-selected',String(selected));entry.tabIndex=selected?0:-1;const panel=document.getElementById(entry.getAttribute('aria-controls'));if(panel)panel.hidden=!selected;}
   const next=Number(button.dataset.price);
   if(Number.isFinite(next)){
    const value=String(Math.round(next)).padStart(4,'0'),missing=4-String(Math.round(next)).length;
    drums.forEach((drum,i)=>{drum.digit=Number(value[i]);drum.delay=first?0:i*.045;drum.width.t=i<missing?0:1;if(first){drum.y.x=drum.y.t=drum.digit;drum.width.x=drum.width.t;}});
   }
  }
  const item=native?button.closest('label'):button;
  highlight.style.top=item.offsetTop+'px';highlight.style.height=item.offsetHeight+'px';
  retargetEdges(left,right,item.offsetLeft,item.offsetLeft+item.offsetWidth,immediate||first);first=false;animation.request();
 }
 const animation=runtime.watch(group,(dt,staticMode)=>{
  let moving=[left,right].map(value=>step(value,dt,staticMode)).some(Boolean);
  highlight.style.transform=`translateX(${left.x.toFixed(2)}px)`;highlight.style.width=Math.max(1,right.x-left.x).toFixed(2)+'px';
  for(const drum of drums){
   drum.delay-=dt;if(drum.delay<=0||staticMode)drum.y.t=drum.digit;else moving=true;
   moving=step(drum.y,dt,staticMode)||moving;moving=step(drum.width,dt,staticMode)||moving;
   drum.column.style.width=(Math.max(0,drum.width.x)*.62)+'em';drum.column.style.opacity=String(clamp(drum.width.x,0,1));drum.strip.style.transform=`translateY(${-drum.y.x*1.1}em)`;
  }
  if(separator){separator.style.width=(clamp(drums[0].width.x,0,1)*.28)+'em';separator.style.opacity=String(clamp(drums[0].width.x,0,1));}
  return moving;
 });
 for(const button of buttons){const fn=()=>select(button);button.addEventListener(native?'change':'click',fn);registrations.push(()=>button.removeEventListener(native?'change':'click',fn));}
 const keydown=event=>{
  if(native||!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
  event.preventDefault();const index=buttons.indexOf(current),next=event.key==='Home'?0:event.key==='End'?buttons.length-1:(index+(event.key==='ArrowRight'?1:-1)+buttons.length)%buttons.length;
  buttons[next].focus();select(buttons[next]);
 };
 group.addEventListener('keydown',keydown);
 const layout=()=>select(current,true),resize=typeof ResizeObserver==='function'?new ResizeObserver(layout):null;
 resize?.observe(group);select(current,true);group.classList.add('motion-plan-ready');
 return()=>{animation.destroy();resize?.disconnect();registrations.forEach(fn=>fn());group.removeEventListener('keydown',keydown);highlight.remove();group.classList.remove('motion-plan-ready');};
}

export function mountCopy(pre,runtime){
 const code=pre.querySelector('code');if(!code)return()=>{};
 const row=document.createElement('div');row.className='motion-copy-row';
 const button=document.createElement('button');button.type='button';button.className='motion-copy';button.setAttribute('aria-label','Copiar trecho de código');
 button.innerHTML='<span class="motion-copy-icons" aria-hidden="true"><svg data-copy-icon viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="8" y="8" width="12" height="12" rx="3"/><path d="M15 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2"/></svg><svg data-copy-check viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m5 12 4 4L19 6"/></svg></span><span class="motion-copy-slot" aria-hidden="true"><span class="motion-copy-words"><span>Copiar</span><span>Copiado</span></span></span>';
 const message=document.createElement('span');message.className='motion-copy-message';message.setAttribute('role','status');row.append(button,message);pre.after(row);
 const label=button.querySelector('.motion-copy-slot'),words=button.querySelector('.motion-copy-words'),icon=button.querySelector('[data-copy-icon]'),check=button.querySelector('[data-copy-check]');
 const roll=spring(0,springs.copy),pop=spring(0,springs.copyIcon),width=spring(56,springs.copyWidth);let timer;
 const animation=runtime.watch(button,(dt,staticMode)=>{
  const moving=[roll,pop,width].map(value=>step(value,dt,staticMode)).some(Boolean),v=clamp(pop.x,0,1);
  label.style.width=width.x.toFixed(2)+'px';words.style.transform=`translateY(${-roll.x*1.25}em)`;
  icon.style.opacity=String(1-v);icon.style.transform=`scale(${1-.5*v}) rotate(${-40*v}deg)`;
  check.style.opacity=String(v);check.style.transform=`scale(${.4+.6*pop.x})`;return moving;
 });
 function state(value){
  clearTimeout(timer);button.disabled=value==='pending';
  const ok=value==='success';roll.t=pop.t=ok?1:0;width.t=ok?67:56;
  button.setAttribute('aria-label',ok?'Trecho copiado':'Copiar trecho de código');
  message.textContent=value==='error'?'Não foi possível copiar. Selecione o texto e copie pelo teclado.':ok?'Trecho copiado.':'';animation.request();
  if(ok)timer=setTimeout(()=>state('idle'),1800);
 }
 const action=createCopyAction(navigator.clipboard?.writeText?.bind(navigator.clipboard),state);
 const click=()=>action.run(code.textContent);button.addEventListener('click',click);
 return()=>{clearTimeout(timer);action.cancel();animation.destroy();button.removeEventListener('click',click);row.remove();};
}
