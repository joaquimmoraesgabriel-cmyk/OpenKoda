import {project,turnstileSiteKey} from './cloud.js';
import {enhanceMotion} from './motion-ui.js';
const root=document.querySelector('#account');
const params=new URLSearchParams(location.search);
const callback=location.origin+'/conta/retorno/';
const namespace='koda.auth.attempt';
const legalVersion='2026-11-13';
let email='', submitting=false, simulationRequest=null;
let captcha, googleGrant, grantTimer, captchaLoading;
const notice=document.createElement('p');
notice.setAttribute('role','status');
let feedbackAction='';
const say=(text,error=false)=>{
 const target=(feedbackAction&&root.querySelector('[data-feedback="'+feedbackAction+'"]'))||notice;
 target.setAttribute('role',error?'alert':'status');target.textContent=text;
};
function content(html,title='Minha conta',subtitle=''){
 clearTimeout(grantTimer);googleGrant=null;
 if(captcha){
  clearTimeout(captcha.timer);
  if(captcha.id!=null)window.turnstile?.remove?.(captcha.id);
  captcha=null;
 }
 root.setAttribute('aria-busy','false');
 const mark=document.querySelector('.site-header .brand svg').cloneNode(true);
 root.replaceChildren();
 if(title){
  const icon=document.createElement('div');icon.className='account-mark';icon.append(mark);root.append(icon);
  const heading=document.createElement('h1');heading.textContent=title;root.append(heading);
  const description=document.createElement('p');description.className='account-intro';description.textContent=subtitle;root.append(description);
 }
 root.insertAdjacentHTML('beforeend',html);root.append(notice);
 queueMicrotask(()=>enhanceMotion(root));
}
function payload(form){return Object.fromEntries(new FormData(form));}
async function call(operation,data={}){
 const run=async()=>{
  const response=await fetch('/conta/api/'+operation,{method:'POST',credentials:'same-origin',cache:'no-store',redirect:'error',headers:{'Content-Type':'application/json'},body:JSON.stringify(data),signal:AbortSignal.timeout(25000)});
  const parsed=await response.json().catch(()=>({}));
  if(!response.ok){
   const message=parsed.code==='captcha_failed'?'A verificação foi recusada ou expirou. Conclua a nova verificação.':parsed.code==='captcha_unavailable'?'A verificação de segurança está indisponível. Tente novamente em instantes.':response.status===429?'Muitas tentativas. Aguarde antes de tentar novamente.':response.status===401||response.status===403?'Acesso recusado. Confira a conta e a confirmação do e-mail.':'Não foi possível concluir. Confira os dados e tente novamente.';
   throw Object.assign(Error(message),{code:parsed.code});
  }
  return parsed;
 };
 // Cookie rotation and logout share one queue across same-origin tabs. The
 // worker still validates every request independently on older browsers.
 return navigator.locks?navigator.locks.request('koda.site.session',run):run();
}
const edge=(action,body={})=>call('account',{action,...body});
async function authorize(desktop,chooseAccount=false,intent){
 const state=desktop?params.get('state'):crypto.randomUUID();
 const port=desktop?params.get('port'):null;
 const nativeHash=desktop?params.get('code_challenge'):null;
 if(desktop && (!/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(state??'')||!/^\d{2,5}$/.test(port??'')||Number(port)<1024||Number(port)>65535||!/^[\w-]{43}$/.test(nativeHash??'')))throw Error('A solicitação do aplicativo expirou ou é inválida.');
 if(googleGrant&&googleGrant.expires<=Date.now()){
  googleGrant=null;clearTimeout(grantTimer);attachCaptcha('login');
  throw Error('A verificação expirou. Conclua a nova verificação.');
 }
 const grant=googleGrant??await verifyGoogle(desktop);
 googleGrant=null;clearTimeout(grantTimer);
 const hash=desktop?nativeHash:grant.challenge;
 sessionStorage.removeItem(namespace+(desktop?'.google':'.desktop'));
 sessionStorage.setItem(namespace+(desktop?'.desktop':'.google'),JSON.stringify({desktop,state,port,consent:desktop?null:intent,expires:Date.now()+180000}));
 const url=new URL(project+'/auth/v1/authorize');
 url.search=new URLSearchParams({provider:'google',redirect_to:callback,code_challenge:hash,code_challenge_method:'s256'});
 if(chooseAccount)url.searchParams.set('prompt','select_account');
 location.assign(url.href);
}
function login(){
 content('<form id="login"><label>E-mail<input type="email" name="email" autocomplete="email" required maxlength="254"></label><label>Senha<input type="password" name="password" autocomplete="current-password" required maxlength="128"></label><button>Entrar</button></form><div class="auth-divider">ou</div><button class="secondary auth-provider" data-do="google">Entrar com Google</button><div class="auth-links"><button class="secondary" data-do="signup">Criar conta</button><button class="secondary" data-do="recover">Esqueci a senha</button></div>','Entre no OpenKoda','Acesse seu plano, consulte seu uso e acompanhe seus chamados.');attachCaptcha();
}
const requiredAgreements='<label><input type="checkbox" name="adult" required> Tenho 18 anos ou mais.</label><label><input type="checkbox" name="agreement" required> Li e aceito os <a href="/termos/" target="_blank" rel="noopener">termos de uso</a> e a <a href="/privacidade/" target="_blank" rel="noopener">política de privacidade</a>.</label>';
const agreements=requiredAgreements+'<label><input type="checkbox" name="marketing"> Quero receber novidades da Koda por e-mail.</label>';
function consentIntent(data){
 if(data?.adult!=='on'||data?.agreement!=='on')throw Error('Confirme a idade e o aceite dos termos e da privacidade.');
 return {adult:true,termsVersion:legalVersion,privacyVersion:legalVersion,marketing:data.marketing==='on'};
}
async function verifyGoogle(desktop){
 const result=await call('google',{captchaToken:takeCaptcha(),...(desktop?{desktop:true}:{})});
 if(desktop?result.verified!==true:!/^[-\w]{43}$/.test(result.challenge??''))throw Error('Não foi possível iniciar o login com Google.');
 return {challenge:result.challenge,expires:Date.now()+240000};
}
function googleChoice(desktop,chooseAccount=false,grant=null){
 const operation=desktop?'googleDesktopConsent':chooseAccount?'googleRetryConsent':'googleConsent';
 content('<form id="'+operation+'">'+requiredAgreements+'<button>Continuar com Google</button></form><button class="secondary account-back" data-do="login">Voltar ao login</button>','Continue com Google',
  desktop?'Confirme os requisitos da conta. Depois do login, o aplicativo permite registrar os aceites e escolher suas preferências.':'Confira os requisitos antes de abrir o Google. Você poderá escolher se deseja receber novidades em Minha conta.');
 googleGrant=grant;
 const refresh=()=>{googleGrant=null;attachCaptcha('login');};
 if(grant){grantTimer=setTimeout(refresh,Math.max(0,grant.expires-Date.now()));}else{refresh();}
}
function signup(){
 if(!turnstileSiteKey){content('<p>O cadastro público está em preparação. A edição gratuita com chave própria está disponível na página de download.</p><button class="secondary" data-do="login">Voltar ao login</button>');return;}
 content('<form id="signup"><label>E-mail<input type="email" name="email" required autocomplete="email"></label><label>Senha de pelo menos 12 caracteres<input type="password" minlength="12" maxlength="128" name="password" required autocomplete="new-password"></label>'+agreements+'<button>Cadastrar</button></form><button class="secondary account-back" data-do="login">Voltar ao login</button>','Crie sua conta','Use um e-mail ao qual você tenha acesso para confirmar o cadastro.');
 attachCaptcha();
}
function oauthFailure(desktop){
 if(desktop){
  content('<p>Abra o OpenKoda e inicie o login novamente. Você poderá escolher outra conta no Google.</p><button class="secondary account-back" data-do="login">Entrar no site</button>','O login não foi concluído');
 }else{
  content('<button data-do="retryGoogle">Tentar com Google novamente</button><div class="auth-links"><button class="secondary" data-do="login">Entrar com e-mail</button></div>','O login não foi concluído','Tente outra vez ou escolha outra conta do Google.');attachCaptcha('login');
 }
}
function emailLinkFailure(){
 content('<button data-do="recover">Solicitar nova recuperação</button><div class="auth-links"><button class="secondary" data-do="login">Voltar ao login</button><button class="secondary" data-do="signup">Confirmar um cadastro</button></div>',
  'Este link não pôde ser usado','Abra o e-mail no navegador em que fez o pedido. Se o link já foi usado ou expirou, solicite um novo.');
}
function takeCaptcha(){
 if(!captcha?.token||captcha.expires<=Date.now())throw Error('Conclua a verificação de segurança.');
 const token=captcha.token;
 captcha.token='';captcha.enable(false);clearTimeout(captcha.timer);
 return token;
}
function retryCaptcha(){
 if(!captcha||captcha.token||captcha.id==null)return;
 window.turnstile?.reset?.(captcha.id);
}
async function attachCaptcha(action){
 const form=root.querySelector('form');
 const first=form?.querySelector('button')??root.querySelector('[data-do="retryGoogle"]');
 if(!first)return;
 const buttons=[first,...root.querySelectorAll('[data-do="google"]')];
 const slot=document.createElement('div');first.before(slot);
 const current={id:null,token:'',expires:0,timer:null,enable:ready=>{for(const b of buttons)b.disabled=!ready;}};
 captcha=current;current.enable(false);
 if(!turnstileSiteKey){say('A verificação de segurança está indisponível.');return;}
 function clear(){
  if(captcha!==current)return;
  current.token='';clearTimeout(current.timer);current.enable(false);
 }
 captchaLoading??=new Promise((ok,fail)=>{const script=document.createElement('script');script.src='https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';script.onload=ok;script.onerror=()=>{script.remove();fail();};document.head.append(script);}).catch(()=>{captchaLoading=null;throw Error();});
 try{
  await captchaLoading;if(!slot.isConnected)return;
  current.id=window.turnstile.render(slot,{sitekey:turnstileSiteKey,action:action??form.id,
   callback(token){
    if(captcha!==current||typeof token!=='string'||!token||token.length>2048)return;
    clearTimeout(current.timer);current.token=token;current.expires=Date.now()+285000;
    current.enable(true);current.timer=setTimeout(()=>{clear();retryCaptcha();},285000);
   },
   'expired-callback':clear,'timeout-callback':clear,
   'error-callback':()=>{if(captcha===current){clear();say('A verificação de segurança falhou. Aguarde a nova tentativa ou recarregue a página.');}return true;}
  });
 }catch{if(captcha===current)say('A verificação de segurança não carregou. Recarregue a página para tentar novamente.');}
}
const accountSections=[['visao-geral','Visão geral'],['plano','Plano'],['uso','Uso e limites'],['chamados','Chamados'],['preferencias','Preferências'],['dados','Dados e acesso']];
let accountSnapshot=null,accountRevision=0,ticketRevision=0,ticketsLoaded=false;
const feedback=action=>'<p class="account-feedback" data-feedback="'+action+'" role="status"></p>';
function account(){
 accountSnapshot=null;ticketsLoaded=false;simulationRequest=null;
 content(`<div class="dashboard account-workspace">
 <header class="dashboard-head"><div><p class="account-eyebrow">Seu espaço no OpenKoda</p><h1>Minha conta</h1><p id="identity"></p></div><div><button class="secondary" data-do="logout">Sair</button>${feedback('logout')}</div></header>
 <div class="account-layout"><nav class="account-navigation" aria-label="Seções da conta">${accountSections.map(([id,label])=>`<a href="#${id}" data-section-link="${id}">${label}</a>`).join('')}</nav>
 <div class="account-panels">
 <section class="account-panel" data-account-section="visao-geral" aria-labelledby="heading-visao-geral"><div class="account-section-heading"><p class="account-eyebrow">Visão geral</p><h2 id="heading-visao-geral" tabindex="-1">Tudo no seu lugar.</h2><p>Consulte sua conta, acompanhe os limites e encontre o próximo passo.</p></div>
 <div class="account-summary" data-account-summary aria-live="polite"><p>Consultando seu plano…</p></div>
 <div class="account-task-list"><a href="#plano"><span><strong>Seu plano</strong><small>Veja a situação da assinatura e as opções disponíveis.</small></span><span aria-hidden="true">01</span></a><a href="#chamados"><span><strong>Precisa de ajuda?</strong><small>Abra um chamado e acompanhe as respostas no mesmo lugar.</small></span><span aria-hidden="true">02</span></a><a href="#dados"><span><strong>Você controla seus dados</strong><small>Exporte as informações da conta ou consulte a exclusão.</small></span><span aria-hidden="true">03</span></a></div>
 </section>
 <section class="account-panel" data-account-section="plano" aria-labelledby="heading-plano" hidden><div class="account-section-heading"><p class="account-eyebrow">Plano</p><h2 id="heading-plano" tabindex="-1">Uma assinatura, limites claros.</h2><p>Veja o que está ativo antes de escolher qualquer alteração.</p></div><div class="account-plan-state" data-plan-state aria-live="polite"><p>Consultando seu plano…</p></div>
 <section class="account-surface"><h3>Explore os planos</h3><p class="account-callout">A simulação interna está disponível somente para contas de teste autorizadas. Ela libera recursos de teste, sem cobrança, renovação automática ou crédito de IA gerenciada.</p><form id="checkout" hidden><fieldset class="plan-choice" data-plan-switch><legend>Plano para simulação</legend>
 <label><input type="radio" name="plan" value="plus" checked><span>Plus</span><span>R$ 219 por mês</span></label><label><input type="radio" name="plan" value="pro"><span>Pro</span><span>R$ 999 por mês</span></label><label><input type="radio" name="plan" value="ultra"><span>Ultra</span><span>R$ 1.999 por mês</span></label></fieldset><button>Simular assinatura</button>${feedback('checkout')}</form><a class="account-inline-link" href="/precos/">Compare os recursos dos planos</a></section>
 <section class="account-surface"><h3>Cancelamento</h3><p>Consulte a situação acima. Um pedido de cancelamento é registrado na sua conta, sem contratar um novo plano.</p><button class="secondary" data-do="cancel">Cancelar renovação ou simulação</button>${feedback('cancel')}<a class="account-inline-link" href="/reembolso/">Leia a política de cancelamento e reembolso</a></section></section>
 <section class="account-panel" data-account-section="uso" aria-labelledby="heading-uso" hidden><div class="account-section-heading"><p class="account-eyebrow">Uso e limites</p><h2 id="heading-uso" tabindex="-1">Acompanhe sua cota.</h2><p>Os valores abaixo pertencem à IA da assinatura. O uso com chave própria é acompanhado no aplicativo e no seu provedor.</p></div><div data-usage-detail aria-live="polite"><p>Consultando seu uso…</p></div><button class="secondary" data-do="refresh">Atualizar uso</button>${feedback('refresh')}<div class="account-explainer"><h3>Como a cota funciona</h3><p>A cota do ciclo mensal é liberada em janelas de até sete dias. O saldo da janela e o limite mensal valem ao mesmo tempo. A próxima liberação aparece quando existe uma assinatura ativa.</p><a href="/guias/uso/">Entenda os limites de uso</a></div></section>
 <section class="account-panel" data-account-section="chamados" aria-labelledby="heading-chamados" hidden><div class="account-section-heading"><p class="account-eyebrow">Chamados</p><h2 id="heading-chamados" tabindex="-1">Vamos resolver juntos.</h2><p>Descreva o que aconteceu, o que esperava e os passos para reproduzir. Não envie senhas, chaves de API ou arquivos confidenciais.</p></div><section class="account-surface"><h3>Novo chamado</h3><form id="support"><label>Assunto<input name="subject" required maxlength="120"></label><label>Mensagem sem dados confidenciais<textarea name="message" required rows="5" minlength="10" maxlength="2000"></textarea></label><button>Enviar chamado</button>${feedback('support')}</form></section><section class="account-ticket-history"><div class="account-row-heading"><h3>Meus chamados e respostas</h3><button class="secondary" data-do="tickets">Atualizar chamados</button></div><p id="tickets-state" role="status"></p><ul id="tickets"></ul>${feedback('tickets')}</section></section>
 <section class="account-panel" data-account-section="preferencias" aria-labelledby="heading-preferencias" hidden><div class="account-section-heading"><p class="account-eyebrow">Preferências</p><h2 id="heading-preferencias" tabindex="-1">Escolhas que são suas.</h2><p>O consentimento para novidades é opcional e fica separado dos documentos necessários à conta.</p></div><section class="account-surface"><h3>Novidades por e-mail</h3><p id="marketing-state">Consultando sua preferência…</p><form id="marketing"><label><input type="checkbox" name="marketing"> Quero receber novidades da Koda por e-mail.</label><button>Salvar preferência</button>${feedback('marketing')}</form><button class="secondary" data-do="unsubscribe">Retirar consentimento de marketing</button>${feedback('unsubscribe')}</section><section class="account-surface"><h3>Termos e privacidade</h3><p id="consent-state">Consultando seus aceites…</p><form id="consent" hidden>${requiredAgreements}<label><input type="checkbox" name="marketing"> Quero autorizar novidades por e-mail junto a estes aceites.</label><p class="account-small">Esta escolha opcional será registrada junto com os aceites. Se você deixar a opção desmarcada, o envio de novidades ficará desativado. Depois, poderá alterá-la na seção de e-mails acima.</p><button>Salvar aceites</button></form>${feedback('consent')}</section></section>
 <section class="account-panel" data-account-section="dados" aria-labelledby="heading-dados" hidden><div class="account-section-heading"><p class="account-eyebrow">Dados e acesso</p><h2 id="heading-dados" tabindex="-1">Sua conta, seu controle.</h2><p>Estas ações se referem à conta hospedada. Projetos e conversas guardados no computador são gerenciados no aplicativo.</p></div><section class="account-surface"><h3>Uma cópia dos seus dados</h3><p>Baixe as informações disponíveis da conta em um arquivo JSON. Guarde-o em um lugar seguro.</p><button class="secondary" data-do="export">Exportar dados da conta</button>${feedback('export')}</section><section class="account-surface account-danger"><h3>Excluir a conta</h3><p>A exclusão da conta hospedada é permanente. Leia a política de privacidade e exporte os dados que deseja guardar antes de continuar.</p><button class="secondary" data-do="showDelete" aria-expanded="false" aria-controls="delete">Ver opções de exclusão</button><form id="delete" hidden><p>Para confirmar o pedido, digite EXCLUIR.</p><label>Frase de confirmação<input name="confirmation" required pattern="EXCLUIR" autocomplete="off"></label><button>Excluir minha conta</button>${feedback('delete')}</form></section><a class="account-inline-link" href="/privacidade/">Consulte a política de privacidade</a></section>
 </div></div></div>`,null);
 root.querySelector('#identity').textContent=email;
 root.querySelector('.dashboard-head>div').append(notice);
 const selected=params.get('plano');if(['plus','pro','ultra'].includes(selected))root.querySelector('#checkout input[value="'+selected+'"]').checked=true;
 const requested=location.hash.slice(1),initial=accountSections.some(([id])=>id===requested)?requested:params.get('acao')==='suporte'?'chamados':selected?'plano':'visao-geral';
 if(!requested&&initial!=='visao-geral')history.replaceState(null,'',location.pathname+location.search+'#'+initial);
 selectAccountSection(initial,false);
 void refreshAccount();
}
function selectAccountSection(id,focus=true){
 if(!root.querySelector('.dashboard'))return;
 if(!accountSections.some(([key])=>key===id))id='visao-geral';
 for(const panel of root.querySelectorAll('[data-account-section]'))panel.hidden=panel.dataset.accountSection!==id;
 for(const link of root.querySelectorAll('[data-section-link]')){if(link.dataset.sectionLink===id)link.setAttribute('aria-current','page');else link.removeAttribute('aria-current');}
 if(focus)root.querySelector('#heading-'+id)?.focus({preventScroll:true});
 if(id==='chamados'&&!ticketsLoaded)void refreshTickets();
}
window.addEventListener('hashchange',()=>selectAccountSection(location.hash.slice(1)));
const amount=value=>typeof value==='number'&&Number.isFinite(value)&&value>=0?value:null;
const usd=value=>amount(value)===null?'Não disponível':(value/1000000).toLocaleString('pt-BR',{style:'currency',currency:'USD',maximumFractionDigits:6});
const dateLabel=value=>{if(!value)return 'Não disponível';const date=new Date(value);return Number.isNaN(date.valueOf())?'Não disponível':date.toLocaleString('pt-BR',{dateStyle:'short',timeStyle:'short'});};
const planLabel=plan=>({plus:'Plus',pro:'Pro',ultra:'Ultra'})[plan?.id]??(plan?'Plano da conta':'Gratuito');
function textAt(selector,text){for(const el of root.querySelectorAll(selector))el.textContent=text;}
function renderAccountSummary(r){
 const usage=r.usage,active=Boolean(r.plan),sim=r.simulation;
 const simulated=sim?.mode==='internal'&&sim.status==='active'&&Date.parse(sim.periodEnd)>Date.now();
 const label=active?planLabel(r.plan):simulated?planLabel({id:sim.plan})+' · simulação interna':planLabel(null);
 const summary=root.querySelector('[data-account-summary]'),plan=root.querySelector('[data-plan-state]'),detail=root.querySelector('[data-usage-detail]');
 summary.innerHTML='<div><span class="account-stat-label">Plano atual</span><strong data-plan-label></strong><p data-plan-note></p></div><div><span class="account-stat-label">Próxima liberação</span><strong class="account-date" data-next-release></strong><a href="#uso">Ver uso e limites</a></div>';
 plan.innerHTML='<span class="account-stat-label">Plano atual</span><strong data-plan-label></strong><p data-plan-note></p>';
 textAt('[data-plan-label]',label);
 textAt('[data-plan-note]',!active?(simulated?'Recursos de teste até '+dateLabel(sim.periodEnd)+'. Use sua própria chave para IA; não há crédito gerenciado.':sim?'Simulação '+(sim.status==='canceled'?'cancelada':'expirada')+'. Use sua própria chave de API no aplicativo.':'Use sua própria chave de API no aplicativo.'):r.subscription?.cancelAtPeriodEnd?'Renovação cancelada. Consulte o encerramento do ciclo em Uso e limites.':'Consulte o consumo e as datas do seu ciclo em Uso e limites.');
 root.querySelector('#checkout').hidden=r.controls?.simulationEnabled!==true;
 textAt('[data-next-release]',usage?dateLabel(usage.nextRelease):'Sem cota ativa');
 if(!usage){detail.innerHTML='<div class="account-empty"><h3>Não há uma cota de assinatura ativa.</h3><p>Você pode usar sua própria chave de API no aplicativo. Os gastos dessa conexão aparecem no histórico de uso do desktop e são cobrados pelo provedor escolhido.</p><a href="#plano">Consultar plano</a></div>';}
 else{
  detail.innerHTML='<div class="account-usage-grid"><section class="account-surface"><h3>Janela de sete dias</h3><p class="account-usage-value" data-week-value></p><progress aria-label="Consumo da janela de sete dias"></progress><p data-week-dates></p></section><section class="account-surface"><h3>Ciclo mensal</h3><p class="account-usage-value" data-month-value></p><progress aria-label="Consumo do ciclo mensal"></progress><p data-month-dates></p></section></div><dl class="account-usage-ledger"><div><dt>Próxima liberação</dt><dd data-usage-release></dd></div><div><dt>Reservado para solicitações em andamento</dt><dd data-reserved></dd></div><div><dt>Consumo concluído no ciclo</dt><dd data-settled></dd></div></dl><p class="account-small">O uso pode incluir valores reservados enquanto uma solicitação aguarda conclusão. Eles são reconciliados pelo servidor.</p>';
  for(const [period,used,limit] of [['week',usage.windowUsedMicros,usage.windowLimitMicros],['month',usage.monthlyUsedMicros,usage.monthlyLimitMicros]]){
   textAt('[data-'+period+'-value]',usd(used)+' de '+usd(limit));
   const progress=detail.querySelector(period==='week'?'progress[aria-label="Consumo da janela de sete dias"]':'progress[aria-label="Consumo do ciclo mensal"]');
   if(amount(used)!==null&&amount(limit)!==null&&limit>0){progress.max=limit;progress.value=Math.min(used,limit);progress.setAttribute('aria-valuetext',usd(used)+' de '+usd(limit));}else{progress.hidden=true;}
  }
  textAt('[data-week-dates]',dateLabel(usage.windowStart)+' a '+dateLabel(usage.windowEnd));
  textAt('[data-month-dates]',dateLabel(r.subscription?.periodStart)+' a '+dateLabel(r.subscription?.periodEnd));
  textAt('[data-usage-release]',dateLabel(usage.nextRelease));textAt('[data-reserved]',usd(usage.reservedMicros));textAt('[data-settled]',usd(usage.settledMicros));
 }
 const saved=Boolean(r.consent?.acceptedAt&&r.consent.termsVersion===r.consent.requiredTermsVersion&&r.consent.privacyVersion===r.consent.requiredPrivacyVersion);
 textAt('#consent-state',saved?'Aceites atuais registrados em '+dateLabel(r.consent.acceptedAt)+'.':'Confira e registre os aceites dos termos e da política de privacidade.');
 textAt('#marketing-state',r.consent?.marketing?'Você autorizou o envio de novidades. Pode mudar essa escolha a qualquer momento.':'O envio de novidades não está autorizado. E-mails necessários à conta e ao suporte são tratados separadamente.');
 const form=root.querySelector('#marketing');if(!form.dataset.dirty)form.elements.marketing.checked=Boolean(r.consent?.marketing);
 const terms=root.querySelector('#consent');terms.hidden=saved;
 if(!terms.dataset.dirty){terms.elements.agreement.checked=false;terms.elements.marketing.checked=false;}
}
async function refreshAccount(){
 const revision=++accountRevision,dashboard=root.querySelector('.dashboard');if(!dashboard)return;
 for(const node of root.querySelectorAll('[data-account-summary],[data-plan-state],[data-usage-detail]'))node.setAttribute('aria-busy','true');
 try{
  const r=await edge('account');if(revision!==accountRevision||!dashboard.isConnected)return;
  accountSnapshot=r;renderAccountSummary(r);
 }catch(e){
  if(revision!==accountRevision||!dashboard.isConnected)return;
  accountSnapshot=null;
  if(e.code==='unauthenticated'){email='';login();say('Sua sessão expirou. Entre novamente.');return;}
  for(const node of root.querySelectorAll('[data-account-summary],[data-plan-state],[data-usage-detail]')){node.innerHTML='<p role="alert">Não foi possível consultar os dados agora. Nenhum valor estimado foi exibido.</p><button class="secondary" data-do="refresh">Tentar novamente</button>';}
  textAt('#marketing-state','Não foi possível consultar sua preferência. Atualize os dados antes de alterar.');textAt('#consent-state','Não foi possível consultar os aceites.');
 }finally{if(dashboard.isConnected)for(const node of root.querySelectorAll('[data-account-summary],[data-plan-state],[data-usage-detail]'))node.setAttribute('aria-busy','false');}
}
async function refreshTickets(){
 const revision=++ticketRevision,dashboard=root.querySelector('.dashboard'),list=root.querySelector('#tickets'),state=root.querySelector('#tickets-state');if(!dashboard||!list)return;
 state.textContent='Consultando seus chamados…';list.setAttribute('aria-busy','true');
 try{
  const r=await edge('support_list');if(revision!==ticketRevision||!dashboard.isConnected)return;
  const tickets=Array.isArray(r.tickets)?r.tickets:[];list.replaceChildren();ticketsLoaded=true;
  state.setAttribute('role','status');state.textContent=tickets.length?'':'Você ainda não abriu chamados.';
  for(const ticket of tickets){const row=document.createElement('li'),head=document.createElement('div'),title=document.createElement('h4'),status=document.createElement('span'),time=document.createElement('p'),message=document.createElement('p');
   head.className='account-ticket-heading';title.textContent=ticket.subject;status.className='account-ticket-status';status.textContent=ticket.status==='closed'?'Encerrado':'Aberto';head.append(title,status);time.className='account-small';time.textContent=dateLabel(ticket.createdAt);message.textContent=ticket.message??'';row.append(head,time,message);
   for(const reply of ticket.replies??[]){const response=document.createElement('div'),label=document.createElement('strong'),body=document.createElement('p');response.className='account-ticket-reply';label.textContent='Resposta do suporte';body.textContent=reply.message;response.append(label,body);row.append(response);}list.append(row);
  }
 }catch(e){if(revision!==ticketRevision||!dashboard.isConnected)return;if(e.code==='unauthenticated'){email='';login();say('Sua sessão expirou. Entre novamente.');return;}state.setAttribute('role','alert');state.textContent='Não foi possível atualizar seus chamados. Tente novamente; o rascunho foi mantido.';}
 finally{if(list.isConnected)list.setAttribute('aria-busy','false');}
}
async function signedIn(result,oauthIntent){
 if(!result.user?.email)throw Error('Confirme seu e-mail antes de entrar.');
 email=result.user.email;
 let intent=oauthIntent,fromSignup=false;
 if(!intent&&email){
  try{const saved=JSON.parse(localStorage.getItem(namespace+'.signup')||'null');
   if(saved?.expires>Date.now()&&saved.email===email.trim().toLowerCase()){intent=saved.consent;fromSignup=true;}
  }catch{}
 }
 let message='Sessão aberta.';
 if(intent?.adult&&intent.termsVersion===legalVersion&&intent.privacyVersion===legalVersion){
  try{
   const {consent}=await edge('account');
   if(consent&&!consent.acceptedAt&&consent.requiredTermsVersion===legalVersion&&consent.requiredPrivacyVersion===legalVersion){
    await edge('consent_initial',{termsVersion:legalVersion,privacyVersion:legalVersion,marketing:fromSignup?intent.marketing===true:consent.marketing===true});
   }
   if(consent&&(consent.requiredTermsVersion!==legalVersion||consent.requiredPrivacyVersion!==legalVersion))message='Sessão aberta. Consulte as versões atuais dos documentos em Preferências.';
   if(fromSignup)localStorage.removeItem(namespace+'.signup');
  }catch{message='Sessão aberta. Confira seus aceites em Preferências.';}
 }
 say(message);account();
}
async function handle(action,data){
 let security;
 if(data&&['login','signup','recover'].includes(action)&&turnstileSiteKey){
  delete data['cf-turnstile-response'];
  security={captcha_token:takeCaptcha()};
 }
 switch(action){
 case 'retryGoogle':return googleChoice(false,true,await verifyGoogle(false));
 case 'googleConsent':case 'googleRetryConsent':case 'googleDesktopConsent':
  return authorize(action==='googleDesktopConsent',action==='googleRetryConsent',consentIntent(data));
 case 'desktopCaptcha':{
  const state=params.get('state'), port=params.get('port');
  if(!/^[0-9a-f-]{36}$/i.test(state??'') || !/^\d{4,5}$/.test(port??'') || Number(port)<1024 || Number(port)>65535)throw Error('Retorno local inválido.');
  const token=takeCaptcha();
  const url=new URL('http://127.0.0.1:'+Number(port)+'/openkoda/callback');
  url.search=new URLSearchParams({state,captcha:token});location.replace(url.href);return;
 }
 case 'google': return googleChoice(false,false,await verifyGoogle(false));
 case 'desktop':return googleChoice(true);
 case 'login':if(!data){login();return;}return signedIn(await call('password',{...data,gotrue_meta_security:security}));
 case 'signup':if(!data)return signup();
  if(!turnstileSiteKey)throw Error('O cadastro público está em preparação.');
  {const consent=consentIntent(data);
   await call('signup',{email:data.email,password:data.password,gotrue_meta_security:security,data:{adult_requested:true,terms_requested:legalVersion,privacy_requested:legalVersion,marketing_requested:consent.marketing}});
   try{localStorage.setItem(namespace+'.signup',JSON.stringify({email:data.email.trim().toLowerCase(),consent,expires:Date.now()+86400000}));}catch{}
  }
  login();return say('Confira seu e-mail e abra o link no mesmo navegador em que iniciou o cadastro.');
 case 'recover':if(!turnstileSiteKey)throw Error('Para recuperar o acesso no pré-lançamento, entre em contato com o suporte.');if(!data){content('<form id="recover"><label>E-mail da conta<input type="email" required name="email"></label><button>Enviar recuperação</button></form>');attachCaptcha();return;}
  await call('recover',{email:data.email,gotrue_meta_security:security});return say('Se a conta existir, você receberá instruções. Abra o link no mesmo navegador.');
 case 'reset':{const result=await call('reset',{password:data.password});email=result.user.email;history.replaceState(null,'','/conta/');account();return say('Senha atualizada.');}
 case 'consent':{
  consentIntent(data);
  // The current endpoint updates terms and marketing together. Never reuse an
  // old opt-in. Only the optional checkbox in this form grants a new opt-in.
  let latest;
  try{latest=await edge('account');accountSnapshot=latest;}catch(error){accountSnapshot=null;throw error;}
  const consent=latest.consent;
  if(consent?.requiredTermsVersion!==legalVersion||consent?.requiredPrivacyVersion!==legalVersion)throw Error('Os documentos foram atualizados. Recarregue a página antes de registrar os aceites.');
  if(consent.acceptedAt&&consent.termsVersion===legalVersion&&consent.privacyVersion===legalVersion){renderAccountSummary(latest);return say('Seus aceites atuais já estão registrados. A preferência de e-mail foi mantida.');}
  await edge('consent',{termsVersion:legalVersion,privacyVersion:legalVersion,marketing:data.marketing==='on'});
  delete root.querySelector('#consent').dataset.dirty;await refreshAccount();return say('Aceites registrados.');
 }
 case 'marketing':if(!accountSnapshot)throw Error('Atualize os dados da conta antes de salvar sua preferência.');await edge('marketing',{enabled:data.marketing==='on'});if(accountSnapshot)accountSnapshot.consent={...accountSnapshot.consent,marketing:data.marketing==='on'};delete root.querySelector('#marketing').dataset.dirty;await refreshAccount();return say('Preferência de e-mail salva.');
 case 'unsubscribe':await edge('marketing',{enabled:false});if(accountSnapshot)accountSnapshot.consent={...accountSnapshot.consent,marketing:false};root.querySelector('#marketing').elements.marketing.checked=false;delete root.querySelector('#marketing').dataset.dirty;await refreshAccount();return say('Consentimento de marketing retirado.');
 case 'support':{const r=await edge('support',{...data,requestId:crypto.randomUUID()});root.querySelector('#support').reset();await refreshTickets();return say('Chamado registrado. '+(r.id??''));}
 case 'checkout':{
  if(accountSnapshot?.controls?.simulationEnabled!==true)throw Error('A simulação está restrita às contas de teste autorizadas.');
  if(simulationRequest?.plan!==data.plan||simulationRequest?.outcome!=='activate')simulationRequest={plan:data.plan,outcome:'activate',requestId:crypto.randomUUID()};
  const result=await edge('simulation',simulationRequest);
  if(result.status!=='active'||result.grantsProductionCredit!==false)throw Error('Não foi possível confirmar a simulação. Atualize sua conta.');
  simulationRequest=null;await refreshAccount();return say('Simulação ativa. Recursos de teste liberados; nenhuma cobrança foi realizada.');
 }
 case 'cancel':{
  if(accountSnapshot?.simulation?.status==='active'&&!accountSnapshot?.plan){
   const plan=accountSnapshot.simulation.plan;
   if(simulationRequest?.plan!==plan||simulationRequest?.outcome!=='cancel')simulationRequest={plan,outcome:'cancel',requestId:crypto.randomUUID()};
   await edge('simulation',simulationRequest);simulationRequest=null;await refreshAccount();return say('Simulação cancelada. Nenhuma cobrança foi realizada.');
  }
  const result=await edge('cancel');await refreshAccount();return say(result.cancelAtPeriodEnd?'Solicitação de cancelamento registrada.':'Não há renovação ativa para cancelar. Nenhuma cobrança foi realizada.');
 }
 case 'refresh':await refreshAccount();return;
 case 'tickets':await refreshTickets();return;
 case 'showDelete':{const form=root.querySelector('#delete'),button=root.querySelector('[data-do=showDelete]');form.hidden=!form.hidden;button.setAttribute('aria-expanded',String(!form.hidden));if(!form.hidden)form.elements.confirmation.focus();return;}
 case 'export':{const result=await edge('export');const a=document.createElement('a');const url=URL.createObjectURL(new Blob([JSON.stringify(result,null,2)],{type:'application/json'}));a.href=url;a.download='koda-meus-dados.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),3000);return say('Arquivo preparado. Confira os downloads do navegador.');}
 case 'delete':await edge('delete',{confirmation:data.confirmation});email='';login();return say('Pedido de exclusão aceito.');
 case 'logout':await call('logout');email='';login();return say('Sessão encerrada.');
 }
}
async function dispatch(action,data){
 if(submitting)return;submitting=true;feedbackAction=action;say('');
 const dashboard=root.querySelector('.dashboard'),form=data&&root.querySelector('form#'+action),scope=form||root.querySelector('[data-do="'+action+'"]');
 const controls=dashboard?[...root.querySelectorAll('form button,[data-do]')].map(el=>[el,el.disabled]):[];
 for(const [el]of controls)el.disabled=true;
 scope?.setAttribute('aria-busy','true');
 const submit=form?.querySelector('button'),label=submit?.textContent;if(submit)submit.textContent='Aguarde…';
 try{await handle(action,data);}catch(e){retryCaptcha();say(e.message||'Serviço indisponível.',true);}
 finally{submitting=false;feedbackAction='';scope?.removeAttribute('aria-busy');for(const [el,disabled]of controls)if(el.isConnected)el.disabled=disabled;if(submit?.isConnected)submit.textContent=label;}
}
root.addEventListener('submit',e=>{e.preventDefault();void dispatch(e.target.id,payload(e.target));});
root.addEventListener('click',e=>{const b=e.target.closest('[data-do]');if(b)void dispatch(b.dataset.do);});
root.addEventListener('input',e=>{const form=e.target.closest('#marketing,#consent');if(form)form.dataset.dirty='true';});
async function restoreDashboard(){
 if(!root.querySelector('.dashboard'))return;
 try{
  const state=await call('status');
  if(!root.querySelector('.dashboard'))return;
  if(!state.user){email='';login();return;}
  if(email!==state.user.email){email=state.user.email;account();}else{await refreshAccount();if(root.querySelector('[data-account-section=chamados]:not([hidden])'))await refreshTickets();}
 }catch(e){say(e.message);}
}
window.addEventListener('pageshow',event=>{if(event.persisted)void restoreDashboard();});
document.addEventListener('visibilitychange',()=>{if(document.visibilityState==='visible')void restoreDashboard();});
function resetForm(){
 content('<form id="reset"><label>Nova senha<input type="password" name="password" minlength="12" maxlength="128" autocomplete="new-password" required></label><button>Salvar nova senha</button></form>','Redefina sua senha');
}
async function loadAccount(reset=false){
 const {user}=await call('status');
 if(!user){login();return;}
 email=user.email;
 if(reset)resetForm();else account();
}
async function init(){
 // An old implicit-flow email may still contain credentials in its fragment.
 // Discard the fragment locally rather than importing tokens into the session.
 const fragment=new URLSearchParams(location.hash.slice(1));
 if(fragment.has('access_token')||fragment.has('refresh_token')){history.replaceState(null,'',location.pathname);login();return say('Abra sua conta com e-mail e senha ou solicite um novo link de recuperação.');}
 if(location.pathname.includes('/retorno')){
  const code=params.get('code'),failed=params.has('error')||new URLSearchParams(location.hash.slice(1)).has('error');history.replaceState(null,'',location.pathname);
  const desktop=JSON.parse(sessionStorage.getItem(namespace+'.desktop')||'null'),google=JSON.parse(sessionStorage.getItem(namespace+'.google')||'null');
  const native=desktop?.desktop&&desktop.expires>=Date.now();
  if(failed){
   if(!native&&(params.get('error_code')==='otp_expired'||fragment.get('error_code')==='otp_expired'))emailLinkFailure();
   else oauthFailure(native);
   return;
  }
  if(!code){history.replaceState(null,'','/conta/');await loadAccount();return;}
  if(code.length>1024)throw Error('O retorno é inválido. Inicie novamente pelo dispositivo que solicitou o acesso.');
  // A native code belongs exclusively to its native PKCE verifier. A stale web
  // login cookie must never try to redeem it or prevent its loopback delivery.
  if(desktop?.desktop){
   sessionStorage.removeItem(namespace+'.desktop');
   if(!native||!/^\d{2,5}$/.test(desktop.port??'')||Number(desktop.port)<1024||Number(desktop.port)>65535||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(desktop.state??'')||!/^[-\w.~]{8,512}$/.test(code)){
    oauthFailure(true);return;
   }
   const target=new URL('http://127.0.0.1:'+Number(desktop.port)+'/openkoda/callback');
   target.search=new URLSearchParams({state:desktop.state,code});
   content('<p>A autenticação com Google foi concluída. Entregue o retorno ao OpenKoda para conectar sua conta neste computador.</p><p>Mantenha o aplicativo aberto. Se ele foi fechado, o prazo terminou ou você cancelou, inicie uma nova tentativa pelo aplicativo.</p>','Voltar ao OpenKoda');
   const open=document.createElement('a');open.className='primary';open.href=target.href;open.textContent='Abrir aplicativo';open.referrerPolicy='no-referrer';root.append(open);
   return;
  }
  let result;
  try{result=await call('exchange',{auth_code:code});}
  catch(error){
   if(!native&&!google&&error.code==='unmatched_attempt')emailLinkFailure();else oauthFailure(native);
   return;
  }
  if(result.kind==='recover'){email=result.user.email;history.replaceState(null,'','/conta/?intent=nova-senha');resetForm();return;}
  if(result.kind==='google')sessionStorage.removeItem(namespace+'.google');
  history.replaceState(null,'','/conta/');
  await signedIn(result,result.kind==='google'&&google?.expires>=Date.now()?google.consent:undefined);return;
 }
 if(params.get('desktop')==='1'){
  if(params.get('intent')==='captcha'){
   if(!turnstileSiteKey)throw Error('A verificação de segurança ainda não está configurada. Consulte o suporte.');
   content('<form id="desktopCaptcha"><p>Conclua a verificação para entrar no OpenKoda neste computador.</p><button>Voltar ao aplicativo</button></form>');attachCaptcha();return;
  }
  content('<p>O aplicativo OpenKoda solicitou entrar com Google neste computador.</p><button data-do="desktop">Continuar com Google para o aplicativo</button>');return;
 }
 if(params.get('intent')==='cadastro'){signup();return;}
 if(params.get('intent')==='recuperar'){login();await handle('recover');return;}
 await loadAccount(params.get('intent')==='nova-senha');
}
init().catch(e=>{content('<button id="reload-session" type="button">Tentar novamente</button>','Não foi possível abrir a conta');root.querySelector('#reload-session').onclick=()=>location.reload();say(e.message);});
