const box=document.querySelector('#cookie-box');
document.documentElement.classList.add('nav-ready');
const menu=document.querySelector('#primary-nav'),toggle=document.querySelector('#menu-toggle');
function menuOpen(open){menu.classList.toggle('is-open',open);toggle.setAttribute('aria-expanded',String(open));}
toggle.addEventListener('click',()=>menuOpen(toggle.getAttribute('aria-expanded')!=='true'));
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&toggle.getAttribute('aria-expanded')==='true'){menuOpen(false);toggle.focus();}});
document.addEventListener('click',event=>{if(!event.target.closest('.site-header'))menuOpen(false);});
matchMedia('(min-width:761px)').addEventListener('change',()=>menuOpen(false));
// The site stores only essentials. The footer opens the disclosure on demand.
document.querySelector('#cookies').addEventListener('click',()=>{box.hidden=false;box.querySelector('button').focus();});
document.querySelector('#accept-essential').addEventListener('click',()=>{try{localStorage.setItem('koda.cookies','essential');}catch{} box.hidden=true;document.querySelector('#cookies').focus();});
const panel=document.querySelector('#demo-body');
if(panel){
 const views={relatorio:panel.innerHTML,
  chat:'<div class="code-sample"><p class="file"><span>api.py</span><span>Python</span></p><pre><code><mark><span class="ln">6</span>     expression = request.args["formula"]</mark>\n<span class="ln">7</span>     <b>return</b> str(compute_formula(expression))</code></pre></div><div class="demo-copy"><small>Conversa sobre o relatório</small><h3>De onde vem a fórmula?</h3><p>A rota HTTP lê o parâmetro <code>formula</code> e o passa para <code>compute_formula</code>, em <code>math_worker.py</code>.</p><p class="caption">Pergunta de exemplo sobre o caso analisado.</p></div>',
  projeto:'<div class="demo-note"><dl><dt>Pasta selecionada</dt><dd>calculadora/</dd><dt>Arquivos autorizados</dt><dd>api.py<br>math_worker.py</dd></dl></div><div class="demo-copy"><small>Escopo da revisão</small><h3>Investigar dois arquivos</h3><p>O objetivo deste projeto é seguir a fórmula desde a entrada HTTP até a operação que a executa.</p></div>'};
 for(const btn of document.querySelectorAll('[data-view]')) btn.addEventListener('click',()=>{for(const peer of document.querySelectorAll('[data-view]'))peer.setAttribute('aria-pressed',String(peer===btn));panel.innerHTML=views[btn.dataset.view];});
}
