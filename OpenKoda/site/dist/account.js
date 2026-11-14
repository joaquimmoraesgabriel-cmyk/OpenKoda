import {project,turnstileSiteKey} from './cloud.js';
const root=document.querySelector('#account');
const params=new URLSearchParams(location.search);
const callback=location.origin+'/conta/retorno/';
const namespace='koda.auth.attempt';
const legalVersion='2026-11-13';
let email='', submitting=false;
let captcha, googleGrant, grantTimer, captchaLoading;
const notice=document.createElement('p');
notice.setAttribute('role','status');
const say=text=>{notice.textContent=text;};
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
function account(){
 content(`<div class="dashboard"><div class="dashboard-head"><div><h1>Minha conta</h1><p id="identity"></p></div><button class="secondary" data-do="logout">Sair</button></div>
 <section class="dash-block is-summary"><h2>Plano e uso</h2><p id="usage" aria-live="polite">Consultando seu plano…</p></section>
 <div class="dash-grid"><section class="dash-block"><h2>Assinatura</h2><form id="checkout"><fieldset class="plan-choice"><legend>Plano para simulação</legend>
 <label><input type="radio" name="plan" value="plus" checked><span>Plus</span><span>R$ 219 por mês</span></label>
 <label><input type="radio" name="plan" value="pro"><span>Pro</span><span>R$ 999 por mês</span></label>
 <label><input type="radio" name="plan" value="ultra"><span>Ultra</span><span>R$ 1.999 por mês</span></label>
 </fieldset><p>Simulação sem cobrança, renovação ou crédito de produção.</p><button>Simular assinatura</button></form><button class="secondary" data-do="cancel">Cancelar renovação ou simulação</button></section>
 <section class="dash-block"><h2>Suporte</h2><form id="support"><label>Assunto<input name="subject" required maxlength="120"></label><label>Mensagem sem dados confidenciais<textarea name="message" required rows="3" minlength="10" maxlength="2000"></textarea></label><button>Enviar chamado</button></form><details><summary>Meus chamados e respostas</summary><p id="tickets-empty" hidden>Você ainda não abriu chamados.</p><ul id="tickets"></ul></details></section>
 <section class="dash-block"><h2>Preferências</h2><details><summary>Aceites e comunicações</summary><form id="consent">${agreements}<button>Salvar aceites</button></form></details><button class="secondary" data-do="unsubscribe">Retirar consentimento de marketing</button></section>
 <section class="dash-block"><h2>Meus dados</h2><button class="secondary" data-do="export">Exportar dados da conta</button><details class="danger"><summary>Excluir minha conta</summary><form id="delete"><p>A exclusão da conta hospedada é permanente. Para continuar, digite EXCLUIR.</p><label>Frase de confirmação<input name="confirmation" required pattern="EXCLUIR"></label><button>Excluir minha conta</button></form></details></section></div></div>`,null);
 root.querySelector('#identity').textContent=email;
 const selected=params.get('plano');if(['plus','pro','ultra'].includes(selected))root.querySelector('#checkout input[value="'+selected+'"]').checked=true;
 void refreshAccount();
 if(params.get('acao')==='suporte')root.querySelector('#support input').focus();
}
const usd = micros => (micros / 1000000).toLocaleString('pt-BR',{style:'currency',currency:'USD',maximumFractionDigits:6});
async function refreshAccount(){
 try {
  const r=await edge('account');
  const target=root.querySelector('#usage');
  if(!target)return;
  const usage=r.usage;
  const parts=['Plano atual: '+(r.plan?.id??'Gratuito')+'.'];
  if(usage?.windowEnd){
   parts.push('Nesta janela de sete dias: '+usd(usage.windowUsedMicros)+' de '+usd(usage.windowLimitMicros)+'.');
   parts.push('No ciclo mensal: '+usd(usage.monthlyUsedMicros)+' de '+usd(usage.monthlyLimitMicros)+'.');
   parts.push('Próxima liberação: '+new Date(usage.nextRelease).toLocaleString('pt-BR')+'.');
  }
  if(r.subscription?.cancelAtPeriodEnd)parts.push('Renovação cancelada.');
  target.textContent=parts.join(' ');
  root.querySelector('#consent [name=marketing]').checked=Boolean(r.consent?.marketing);
  const saved=Boolean(r.consent?.acceptedAt && r.consent.termsVersion===r.consent.requiredTermsVersion && r.consent.privacyVersion===r.consent.requiredPrivacyVersion);
  root.querySelector('#consent [name=agreement]').checked=saved;
  const list=root.querySelector('#tickets');
  if(list){const {tickets}=await edge('support_list');list.replaceChildren();
   root.querySelector('#tickets-empty').hidden=Boolean(tickets?.length);
   for(const ticket of tickets??[]){
   const row=document.createElement('li');row.textContent=ticket.subject+' · '+ticket.status;
   for(const reply of ticket.replies??[]){const p=document.createElement('p');p.textContent=reply.message;row.append(p);}
   list.append(row);
  }}
 } catch(e){if(e.code==='unauthenticated'){email='';login();}say(e.message);}

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
  login();return say('Confira seu e-mail para confirmar o cadastro. Ao entrar, registre os aceites no perfil.');
 case 'recover':if(!turnstileSiteKey)throw Error('Para recuperar o acesso no pré-lançamento, entre em contato com o suporte.');if(!data){content('<form id="recover"><label>E-mail da conta<input type="email" required name="email"></label><button>Enviar recuperação</button></form>');attachCaptcha();return;}
  await call('recover',{email:data.email,gotrue_meta_security:security});return say('Se a conta existir, você receberá instruções. Abra o link no mesmo navegador.');
 case 'reset':{const result=await call('reset',{password:data.password});email=result.user.email;history.replaceState(null,'','/conta/');account();return say('Senha atualizada.');}
 case 'consent':await edge('consent',{termsVersion:'2026-11-13',privacyVersion:'2026-11-13',marketing:data.marketing==='on'});return say('Aceites registrados.');
 case 'unsubscribe':await edge('marketing',{enabled:false});return say('Consentimento de marketing retirado.');
 case 'support':{const r=await edge('support',{...data,requestId:crypto.randomUUID()});root.querySelector('#support').reset();void refreshAccount();return say('Chamado registrado. '+(r.id??''));}
 case 'checkout':await edge('checkout',{plan:data.plan,requestId:crypto.randomUUID()});return say('Simulação registrada. Nenhuma cobrança foi realizada.');
 case 'cancel':await edge('cancel');void refreshAccount();return say('Solicitação de cancelamento registrada.');
 case 'export':{const result=await edge('export');const a=document.createElement('a');const url=URL.createObjectURL(new Blob([JSON.stringify(result,null,2)],{type:'application/json'}));a.href=url;a.download='koda-meus-dados.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),3000);return;}
 case 'delete':await edge('delete',{confirmation:data.confirmation});email='';login();return say('Pedido de exclusão aceito.');
 case 'logout':await call('logout');email='';login();return say('Sessão encerrada.');
 }
}
async function dispatch(action,data){if(submitting)return;submitting=true;say('');try{await handle(action,data);}catch(e){retryCaptcha();say(e.message||'Serviço indisponível.');}finally{submitting=false;}}
root.addEventListener('submit',e=>{e.preventDefault();void dispatch(e.target.id,payload(e.target));});
root.addEventListener('click',e=>{const b=e.target.closest('[data-do]');if(b)void dispatch(b.dataset.do);});
async function restoreDashboard(){
 if(!root.querySelector('.dashboard'))return;
 try{
  const state=await call('status');
  if(!root.querySelector('.dashboard'))return;
  if(!state.user){email='';login();return;}
  if(email!==state.user.email){email=state.user.email;account();}
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
  if(failed){oauthFailure(native);return;}
  if(!code){history.replaceState(null,'','/conta/');await loadAccount();return;}
  if(code.length>1024)throw Error('O retorno é inválido. Inicie novamente pelo dispositivo que solicitou o acesso.');
  let result;
  try{result=await call('exchange',{auth_code:code});}
  catch(error){
   if(error.code==='unmatched_attempt'&&native){
    sessionStorage.removeItem(namespace+'.desktop');const url=new URL('http://127.0.0.1:'+Number(desktop.port)+'/openkoda/callback');url.search=new URLSearchParams({state:desktop.state,code});location.replace(url.href);return;
   }
   oauthFailure(native);return;
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
