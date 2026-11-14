const securityHeaders = {
 'Content-Security-Policy': "default-src 'self'; script-src 'self' https://challenges.cloudflare.com; frame-src https://challenges.cloudflare.com; connect-src 'self' https://ynjcyvnrfxsrijrwpaye.supabase.co https://challenges.cloudflare.com; style-src 'self'; img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'",
 'X-Frame-Options': 'DENY',
 'X-Content-Type-Options': 'nosniff',
 'Strict-Transport-Security': 'max-age=31536000',
 'Referrer-Policy': 'no-referrer',
 'Permissions-Policy': 'camera=(), microphone=(), geolocation=(), payment=()'
};
const projectOrigin='https://ynjcyvnrfxsrijrwpaye.supabase.co';
// Supabase publishable key. The bridge operates only as the authenticated user.
const publishableKey='sb_publishable_7HcLt5Ibu4dnsyj6DvvW9A_8OOCuI3N';
const publicActions=new Set(['account','consent','consent_initial','marketing','checkout','cancel','support','support_list','export','delete']);
const accessCookie='__Host-koda-access',refreshCookie='__Host-koda-refresh';
const attemptKeys={google:'__Host-koda-google',signup:'__Host-koda-signup',recover:'__Host-koda-recover'};
const cookieLifetime=60*60*24*30;
const accessFormat=/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;
const refreshFormat=/^[A-Za-z0-9._~-]{8,1024}$/;
class SiteError extends Error{constructor(status,code){super(code);this.status=status;}}
function check(condition,status=400,code='invalid_request'){if(!condition)throw new SiteError(status,code);}
function cookie(headers,name,value,age){headers.append('Set-Cookie',name+'='+value+'; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age='+age);}
function clearCookies(headers){cookie(headers,accessCookie,'',0);cookie(headers,refreshCookie,'',0);}
function requestCookies(request){
 const cookies=new Map();
 for(const pair of (request.headers.get('cookie')??'').split(';')){
  const at=pair.indexOf('=');if(at<1)continue;
  const name=pair.slice(0,at).trim();if(![accessCookie,refreshCookie,...Object.values(attemptKeys)].includes(name))continue;
  check(!cookies.has(name),400,'duplicate_cookie');cookies.set(name,pair.slice(at+1).trim());
 }
 return {access:cookies.get(accessCookie),refresh:cookies.get(refreshCookie),attempts:cookies};
}
function publicUser(value){
 check(typeof value?.id==='string'&&typeof value?.email==='string'&&!!value.email_confirmed_at,401,'unauthenticated');
 return {id:value.id,email:value.email};
}
async function jsonBody(request){
 const reader=request.body?.getReader();check(reader);
 const decoder=new TextDecoder('utf-8',{fatal:true});let size=0,text='';
 try{
  while(true){
   const {done,value}=await reader.read();if(done)break;
   size+=value.length;if(size>16384){await reader.cancel();throw new SiteError(413,'body_too_large');}
   text+=decoder.decode(value,{stream:true});
  }
  let body;try{body=JSON.parse(text+decoder.decode());}catch{throw new SiteError(400,'invalid_request');}
  check(body&&Object.getPrototypeOf(body)===Object.prototype);return body;
 }finally{reader.releaseLock();}
}
const b64=bytes=>btoa(String.fromCharCode(...bytes)).replaceAll('+','-').replaceAll('/','_').replaceAll('=','');
async function createAttempt(headers,kind){
 const verifier=b64(crypto.getRandomValues(new Uint8Array(32)));
 const challenge=b64(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(verifier))));
 cookie(headers,attemptKeys[kind],verifier,kind==='google'?600:3600);
 return challenge;
}
const fields=(body,allowed)=>check(Object.keys(body).every(key=>allowed.includes(key)));
const smallText=(v,min,max)=>typeof v==='string'&&v.length>=min&&v.length<=max;

async function verifyGoogleCaptcha(token,secret,hostname,send){
 check(smallText(token,1,2048)&&token.trim()===token,400,'captcha_required');
 check(typeof secret==='string'&&secret.length>0,503,'captcha_unavailable');
 const controller=new AbortController();
 const deadline=setTimeout(()=>controller.abort(),10000);
 let result;
 try{
  const response=await send('https://challenges.cloudflare.com/turnstile/v0/siteverify',{
   method:'POST',redirect:'manual',headers:{'Content-Type':'application/json','Cache-Control':'no-store'},
   body:JSON.stringify({secret,response:token}),signal:controller.signal
  });
  check(response.ok,503,'captcha_unavailable');
  result=await response.json();
 }catch{throw new SiteError(503,'captcha_unavailable');}
 finally{clearTimeout(deadline);}
 const errors=Array.isArray(result?.['error-codes'])?result['error-codes']:[];
 check(!errors.some(code=>['internal-error','invalid-input-secret','missing-input-secret'].includes(code)),503,'captcha_unavailable');
 check(result?.success===true&&result.hostname===hostname&&result.action==='login',403,'captcha_failed');
}

export async function sessionBridge(request,send=fetch,env={}){
 const url=new URL(request.url),headers=new Headers(securityHeaders);
 headers.set('Content-Type','application/json');headers.set('Cache-Control','private, no-store, max-age=0');headers.set('Vary','Origin, Cookie');
 const reply=(data,status=200)=>new Response(JSON.stringify(data),{status,headers});
 async function upstream(path,method,data,bearer){
  // workerd supports manual/follow redirects. Reject each redirect before its
  // Location can receive a bearer, password, verifier or refresh token.
  const controller=new AbortController();
  const opts={method,redirect:'manual',headers:{apikey:publishableKey,'Cache-Control':'no-store','Pragma':'no-cache', ...(data?{'content-type':'application/json'}:{}), ...(bearer?{Authorization:'Bearer '+bearer}:{})},signal:controller.signal};
  if(data)opts.body=JSON.stringify(data);
  const deadline=setTimeout(()=>controller.abort(),15000);
  try{
   const res=await send(projectOrigin+path,opts);
   if(res.status>=300&&res.status<400)throw new SiteError(502,'upstream_redirect');
   if(!res.ok){
    if(path==='/auth/v1/token?grant_type=pkce'&&res.status===400){
     const failure=await res.json().catch(()=>({}));
     if(failure.error_code==='bad_code_verifier'||failure.code==='bad_code_verifier')throw new SiteError(400,'bad_code_verifier');
    }
    throw new SiteError([400,401,403,404,409,422,429].includes(res.status)?res.status:503,'service_refused');
   }
   if(res.status===204)return {};
   const text=await res.text();return text?JSON.parse(text):{};
  }finally{clearTimeout(deadline);}
 }
 function save(result){
  const {access_token,refresh_token,expires_in}=result;
  check(smallText(access_token,20,3900)&&accessFormat.test(access_token)&&typeof refresh_token==='string'&&refreshFormat.test(refresh_token)
   &&Number.isSafeInteger(expires_in)&&expires_in>0,502,'invalid_session');
  const user=publicUser(result.user);
  cookie(headers,accessCookie,access_token,Math.min(expires_in,3600));cookie(headers,refreshCookie,refresh_token,cookieLifetime);
  return {access:access_token,user};
 }
 async function session(){
  const {access,refresh}=requestCookies(request);
  if(access&&access.length<=3900&&accessFormat.test(access)){
   try{return {access,user:publicUser(await upstream('/auth/v1/user','GET',null,access))};}
   catch(e){if(!(e instanceof SiteError)||![401,403].includes(e.status))throw e;}
  }
  if(refresh&&refreshFormat.test(refresh)){
   try{return save(await upstream('/auth/v1/token?grant_type=refresh_token','POST',{refresh_token:refresh}));}
   catch(e){if(!(e instanceof SiteError)||![400,401,403,422].includes(e.status))throw e;}
  }
  // A stale request must not clear a newer session set in a different tab.
  throw new SiteError(401,'unauthenticated');
 }
 try{
  check(url.protocol==='https:',403,'https_required');
  check(request.method==='POST',405,'method_not_allowed');
  check(request.headers.get('origin')===url.origin&&(!request.headers.get('sec-fetch-site')||request.headers.get('sec-fetch-site')==='same-origin'),403,'origin_denied');
  check(request.headers.get('content-type')?.split(';')[0].trim()==='application/json',415,'json_required');
  const data=await jsonBody(request),action=url.pathname.slice('/conta/api/'.length);
  if(action==='status'){
   fields(data,[]);try{return reply({user:(await session()).user});}
   catch(e){if(e instanceof SiteError&&e.status===401)return reply({user:null});throw e;}
  }
  if(action==='password'){
   fields(data,['email','password','gotrue_meta_security']);
   check(smallText(data.email,3,254)&&smallText(data.password,1,128));
   const result=await upstream('/auth/v1/token?grant_type=password','POST',data);return reply({user:save(result).user});
  }
  if(action==='google'){
   fields(data,['captchaToken','desktop']);
   check(data.desktop===undefined||data.desktop===true);
   await verifyGoogleCaptcha(data.captchaToken,env.TURNSTILE_SECRET_KEY,url.hostname,send);
   if(data.desktop)return reply({verified:true});
   return reply({challenge:await createAttempt(headers,'google')});
  }
  if(action==='exchange'){
   fields(data,['auth_code']);check(smallText(data.auth_code,1,1024));
   const {attempts}=requestCookies(request);
   for(const [kind,key] of Object.entries(attemptKeys)){
    const verifier=attempts.get(key);if(!verifier||!/^[A-Za-z0-9_-]{43}$/.test(verifier))continue;
    try{
     const result=await upstream('/auth/v1/token?grant_type=pkce','POST',{auth_code:data.auth_code,code_verifier:verifier});
     const user=save(result).user;cookie(headers,key,'',0);return reply({user,kind});
    }catch(e){if(e instanceof SiteError&&e.message==='bad_code_verifier')continue;throw e;}
   }
   return reply({code:'unmatched_attempt'},409);
  }
  if(action==='signup'||action==='recover'){
   fields(data,action==='signup'?['email','password','data','gotrue_meta_security']:['email','gotrue_meta_security']);
   check(smallText(data.email,3,254));
   if(action==='signup'){
    check(smallText(data.password,12,128)&&data.data&&typeof data.data==='object');
    fields(data.data,['adult_requested','terms_requested','privacy_requested','marketing_requested']);
   }
   const callback=url.origin+'/conta/retorno/';
   const pending=new Headers();const hash=await createAttempt(pending,action);
   await upstream('/auth/v1/'+action+'?redirect_to='+encodeURIComponent(callback),'POST',{...data,code_challenge:hash,code_challenge_method:'s256'});
   headers.append('Set-Cookie',pending.get('Set-Cookie'));return reply({ok:true});
  }
  if(action==='logout'){
   fields(data,[]);
   const revoked=()=>{clearCookies(headers);return reply({ok:true});};
   let current;try{current=await session();}catch(e){if(!(e instanceof SiteError)||e.status!==401)throw e;}
   if(current)try{await upstream('/auth/v1/logout?scope=local','POST',{},current.access);}
   catch(e){if(!(e instanceof SiteError)||![401,403].includes(e.status))throw e;}
   return revoked();
  }
  if(action==='reset'){
   fields(data,['password']);check(smallText(data.password,12,128));const current=await session();
   await upstream('/auth/v1/user','PUT',data,current.access);return reply({user:current.user});
  }
  check(action==='account'&&publicActions.has(data.action),404,'unknown_action');
  const current=await session();
  const result=await upstream('/functions/v1/openkoda','POST',data,current.access);
  if(data.action==='delete')clearCookies(headers);
  return reply(result);
 }catch(e){return reply({code:e instanceof SiteError?e.message:'service_unavailable'},e instanceof SiteError?e.status:503);}
}

// Fixed release only. Never forwards browser credentials, cookies or a user URL.
export async function downloadInstaller(request,installer,send=fetch){
 const headers=new Headers(securityHeaders);
 headers.set('Cache-Control','no-store');
 const fail=(message,status)=>new Response(message,{status,headers});
 if(!['GET','HEAD'].includes(request.method)){headers.set('Allow','GET, HEAD');return fail('Método não permitido.',405);}
 let range=request.headers.get('Range');
 const unsatisfied=()=>{headers.set('Content-Range','bytes */'+installer.bytes);return fail('Intervalo de download inválido.',416);};
 if(range&&!/^bytes=(?:\d+-\d*|-\d+)$/.test(range))return unsatisfied();
 if(range){
  const [start,end]=range.slice(6).split('-').map(v=>v===''?null:Number(v));
  if(start===null){if(!Number.isSafeInteger(end)||end<=0)return unsatisfied();range='bytes='+Math.max(0,installer.bytes-end)+'-'+(installer.bytes-1);}
  else {
   if(!Number.isSafeInteger(start)||start>=installer.bytes||end!==null&&(!Number.isSafeInteger(end)||end<start))return unsatisfied();
   range='bytes='+start+'-'+Math.min(end??installer.bytes-1,installer.bytes-1);
  }
 }
 const controller=new AbortController();const timer=setTimeout(()=>controller.abort(),25000);
 try{
  const upstreamHeaders=new Headers({'Accept':'application/octet-stream','User-Agent':'OpenKoda-Download/1.0'});
  if(range){upstreamHeaders.set('Range',range);const condition=request.headers.get('If-Range');if(condition&&condition.length<=512)upstreamHeaders.set('If-Range',condition);}
  const upstream=await send(installer.url,{method:request.method,headers:upstreamHeaders,redirect:'follow',signal:controller.signal});
  clearTimeout(timer);
  if(![200,206].includes(upstream.status)){await upstream.body?.cancel();return fail('Download temporariamente indisponível. Use o link alternativo no GitHub em /download/.',503);}
  headers.set('Content-Type','application/octet-stream');
  headers.set('Content-Disposition','attachment; filename="'+installer.filename+'"');
  headers.set('Accept-Ranges','bytes');
  for(const key of ['Content-Length','Content-Range','ETag','Last-Modified']){
   const value=upstream.headers.get(key);if(value)headers.set(key,value);
  }
  return new Response(request.method==='HEAD'?null:upstream.body,{status:upstream.status,headers});
 }catch{return fail('Não foi possível iniciar a transferência. Abra /download/ e use o link alternativo.',503);}
 finally{clearTimeout(timer);}
}

export default {
 fetch(request,env) {
  const url = new URL(request.url);
  const headers = new Headers(securityHeaders);
  headers.set('Cache-Control',url.pathname.startsWith('/conta')?'private, no-store':'public, max-age=0, must-revalidate');
  if(url.pathname.startsWith('/conta/api/'))return sessionBridge(request,fetch,env);
  if (url.protocol !== 'https:') {
   url.protocol='https:';
   headers.set('Location',url.href);
   return new Response(null,{status:308,headers});
  }
  if (!['GET','HEAD'].includes(request.method)) {
   headers.set('Allow','GET, HEAD');
   return new Response(null,{status:405,headers});
  }
  if(url.pathname==='/download/windows')return downloadInstaller(request,desktopInstaller);
  const path=url.pathname;
  const file=websiteAssets[path] ?? websiteAssets[path.replace(/\/?$/,'/index.html')];
  if (!file) return new Response('Página não encontrada.',{status:404,headers});
  headers.set('Content-Type',file.type);
  if(request.method==='HEAD')return new Response(null,{headers});
  const body=file.type==='image/png'?Uint8Array.from(atob(file.body),value=>value.charCodeAt(0)):file.body;
  return new Response(body,{headers});
 }
};
