import assert from 'node:assert/strict';
import test from 'node:test';
import {createHash} from 'node:crypto';
import {sessionBridge} from '../worker.mjs';

const origin='https://site.example.invalid',auth='https://ynjcyvnrfxsrijrwpaye.supabase.co';
const user={id:'fixture',email:'user@example.invalid',email_confirmed_at:'2026-11-13'};
const access='synthetic.jwt.onlytestcredentials',refresh='synthetic-refresh-token';
const session={access_token:access,refresh_token:refresh,user,expires_in:3600};
const captchaToken='only-a-fixture', captchaSecret='fixture-server-config';
const captchaOk={success:true,hostname:new URL(origin).hostname,action:'login'};
function request(action,body={},headers={},method='POST'){
 return new Request(origin+'/conta/api/'+action,{method,headers:{origin,'content-type':'application/json',...headers},
   ...(method==='GET'?{}:{body:JSON.stringify(body)})});
}
const pass={email:user.email,password:'fixture-password',gotrue_meta_security:{captcha_token:'test'}};
function fixture(upstream,siteverify=()=>Response.json(captchaOk),env={TURNSTILE_SECRET_KEY:captchaSecret}){
 const calls=[];
 const run=(action,body,headers,method)=>sessionBridge(request(action,body,headers,method),async(raw,init)=>{
   if(raw==='https://challenges.cloudflare.com/turnstile/v0/siteverify'){
    assert.equal(init.redirect,'manual');assert.equal(init.method,'POST');
    assert.equal(init.headers.Authorization,undefined);assert.equal(init.headers.apikey,undefined);
    assert.deepEqual(JSON.parse(init.body),{secret:captchaSecret,response:captchaToken});
    return siteverify();
   }
   const url=new URL(raw);assert.equal(url.origin,auth);assert.equal(init.redirect,'manual');assert.ok(!('cache' in init));
   assert.equal(init.headers['Cache-Control'],'no-store');assert.equal(init.headers.Pragma,'no-cache');assert.ok(init.signal instanceof AbortSignal);
   assert.ok(init.headers.apikey.startsWith('sb_publishable_'));
   const call={url,method:init.method,body:init.body?JSON.parse(init.body):{},bearer:init.headers.Authorization};
   calls.push(call);return upstream(call);
 },env);
 return {calls,run};
}
const pairs=response=>response.headers.getSetCookie().map(cookie=>cookie.split(';')[0]).join('; ');

test('JSON POST, origin and fetch metadata are checked before any outgoing call',async()=>{
 const f=fixture(()=>assert.fail('unexpected network'));
 for(const [headers,method,status] of [[{origin:'https://evil.example'},'POST',403],[{origin:''},'POST',403],
   [{'sec-fetch-site':'cross-site'},'POST',403],[{'content-type':'text/plain'},'POST',415],[{},'GET',405]]){
   const r=await f.run('logout',{},headers,method);assert.equal(r.status,status);
   assert.match(r.headers.get('cache-control'),/no-store/);assert.equal(r.headers.getSetCookie().length,0);
 }
 assert.equal((await f.run('status',{}, {cookie:'__Host-koda-access=a; __Host-koda-access=b'})).status,400);
 assert.equal(f.calls.length,0);
});
test('login exposes only identity; refresh rotates both HttpOnly cookies and does not echo tokens',async()=>{
 const f=fixture(({url,body})=>{
  if(url.searchParams.get('grant_type')==='password'){assert.equal(body.password,pass.password);return Response.json(session);}
  assert.equal(url.searchParams.get('grant_type'),'refresh_token');assert.equal(body.refresh_token,refresh);
  return Response.json({...session,refresh_token:'rotated-synthetic-refresh',access_token:access+'1'});
 });
 const login=await f.run('password',pass);
 assert.deepEqual(await login.json(),{user:{id:user.id,email:user.email}});
 assert.equal(login.headers.getSetCookie().length,2);
 for(const value of login.headers.getSetCookie()){
  assert.match(value,/^__Host-koda-(access|refresh)=/);assert.match(value,/; Path=\/; Secure; HttpOnly; SameSite=Lax; Max-Age=/);
  assert.ok(!value.includes('Domain='));
 }
 const renewed=await f.run('status',{}, {cookie:'__Host-koda-refresh='+refresh});
 assert.equal(renewed.status,200);assert.ok(!JSON.stringify(await renewed.json()).includes(refresh));
 assert.match(pairs(renewed),/rotated-synthetic-refresh/);
});
test('no arbitrary upstream actions, redirects or credentials can be supplied',async()=>{
 const f=fixture(()=>assert.fail('unexpected network'));
 for(const action of ['chat','scan','admin','https://evil.example','skill'])assert.equal((await f.run('account',{action})).status,404);
 assert.equal((await f.run('signup',{...pass,redirect_to:'https://evil.example'})).status,400);
 assert.equal((await f.run('exchange',{auth_code:'fake',code_verifier:'override'})).status,400);
 assert.equal((await f.run('password',{...pass,body:'x'.repeat(20000)})).status,413);
 assert.equal(f.calls.length,0);
});
test('workerd manual redirects are rejected without a second request, and a missing PKCE transaction preserves the upstream status',async()=>{
 for(const redirect of [301,302,303,307,308]){
  const f=fixture(()=>new Response(null,{status:redirect,headers:{Location:'https://other.example.invalid/steal'}}));
  const res=await f.run('password',pass);
  assert.equal(res.status,502);assert.deepEqual(await res.json(),{code:'upstream_redirect'});
  assert.equal(f.calls.length,1);assert.deepEqual(res.headers.getSetCookie(),[]);
 }
 const f=fixture(()=>Response.json({error_code:'flow_state_not_found'},{status:404}));
 const attempt=await f.run('google',{captchaToken});
 const res=await f.run('exchange',{auth_code:'unknown-test-code'},{cookie:pairs(attempt)});
 assert.equal(res.status,404);assert.deepEqual(await res.json(),{code:'service_refused'});
});
test('PKCE binds email signup and recovery to the original same callback and selects the matching flow',async()=>{
 const challenges={};
 const f=fixture(({url,body})=>{
  const intent=url.pathname.split('/').pop();
  if(intent==='recover'||intent==='signup'){
   assert.equal(url.searchParams.get('redirect_to'),origin+'/conta/retorno/');
   assert.equal(body.code_challenge_method,'s256');assert.match(body.code_challenge,/^[\w-]{43}$/);
   challenges[intent]=body.code_challenge;return Response.json({id:'fixture'});
  }
  assert.ok(intent==='token'&&url.searchParams.get('grant_type')==='pkce');
  const computed=createHash('sha256').update(body.code_verifier).digest('base64url');
  return computed===challenges[body.auth_code]?Response.json(session):Response.json({error_code:'bad_code_verifier'},{status:400});
 });
 const google=await f.run('google',{captchaToken}),signup=await f.run('signup',{...pass,data:{adult_requested:true,terms_requested:'test',privacy_requested:'test',marketing_requested:false}}),
   recover=await f.run('recover',{email:user.email});
 const cookie=[google,signup,recover].map(pairs).join('; ');
 const exchanged=await f.run('exchange',{auth_code:'recover'},{cookie});
 assert.deepEqual(await exchanged.json(),{user:{id:user.id,email:user.email},kind:'recover'});
 assert.match(pairs(exchanged),/__Host-koda-recover=($|;)/);
 assert.ok(!pairs(exchanged).includes('__Host-koda-google')&&!pairs(exchanged).includes('__Host-koda-signup'));
 const desktop=await f.run('exchange',{auth_code:'desktop'},{cookie});
 assert.equal(desktop.status,409);assert.equal((await desktop.json()).code,'unmatched_attempt');assert.equal(desktop.headers.getSetCookie().length,0);
});
test('Google starts only after server verification for this hostname and action',async()=>{
 const noNetwork=()=>assert.fail('unexpected outbound request');
 const invalid=fixture(noNetwork,noNetwork);
 for(const body of [{},{captchaToken:''},{captchaToken:'x'.repeat(2049)},{captchaToken,desktop:'true'},{captchaToken,secret:'client-override'}]){
  const r=await invalid.run('google',body);assert.equal(r.status,400);assert.deepEqual(r.headers.getSetCookie(),[]);
 }
 const unconfigured=await fixture(noNetwork,noNetwork,{}).run('google',{captchaToken});
 assert.equal(unconfigured.status,503);assert.deepEqual(await unconfigured.json(),{code:'captcha_unavailable'});
 for(const [verified,status] of [[{...captchaOk,hostname:'elsewhere.example'},403],[{...captchaOk,action:'signup'},403],
  [{'success':false,'error-codes':['timeout-or-duplicate']},403],[{'success':false,'error-codes':['invalid-input-secret']},503]]){
  const r=await fixture(noNetwork,()=>Response.json(verified)).run('google',{captchaToken});
  assert.equal(r.status,status);assert.equal(r.headers.getSetCookie().length,0);assert.ok(!(await r.text()).includes(captchaSecret));
 }
 const redirect=await fixture(noNetwork,()=>new Response(null,{status:302,headers:{Location:'https://evil.example'}})).run('google',{captchaToken});
 assert.equal(redirect.status,503);assert.deepEqual(redirect.headers.getSetCookie(),[]);
 let verifications=0;
 const f=fixture(noNetwork,()=>{verifications++;return Response.json(captchaOk);});
 const web=await f.run('google',{captchaToken});assert.equal(web.status,200);
 assert.match((await web.json()).challenge,/^[A-Za-z0-9_-]{43}$/);assert.equal(web.headers.getSetCookie().length,1);
 const native=await f.run('google',{captchaToken,desktop:true});
 assert.equal(native.status,200);assert.deepEqual(await native.json(),{verified:true});
 assert.deepEqual(native.headers.getSetCookie(),[]);assert.equal(verifications,2);
});
test('expired or revoked sessions cannot act; a transient outage cannot clear an active session',async()=>{
 for(const status of [400,503]){
  const f=fixture(()=>Response.json({},{status}));
  const response=await f.run('status',{}, {cookie:'__Host-koda-refresh='+refresh});
  assert.equal(response.status,status===400?200:503);
  if(status===400)assert.deepEqual(await response.json(),{user:null});
  assert.equal(response.headers.getSetCookie().length,0);
  assert.ok(!f.calls.some(c=>c.url.pathname==='/functions/v1/openkoda'));
 }
 const f=fixture(({url,bearer})=>{
  assert.equal(bearer,'Bearer '+access);
  if(url.pathname==='/auth/v1/user')return Response.json(user);
  assert.equal(url.pathname,'/auth/v1/logout');assert.equal(url.searchParams.get('scope'),'local');
  return new Response(null,{status:204});
 });
 const response=await f.run('logout',{}, {cookie:'__Host-koda-access='+access});
 assert.equal(response.status,200);for(const value of response.headers.getSetCookie())assert.match(value,/Max-Age=0/);
});
