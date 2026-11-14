// Real local GoTrue, PostgreSQL and SMTP. Only the transport is redirected to loopback.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { sessionBridge } from '../../../OpenKoda/site/worker.mjs';

const site = 'https://openkoda.test';
const addr = 'fixture@openkoda.test';
const project = 'koda-mail-' + randomBytes(6).toString('hex');
const config = fileURLToPath(new URL('compose.yaml', import.meta.url));
const env = {...process.env, KODA_MAIL_TEST_PASSWORD: randomBytes(24).toString('hex'), KODA_MAIL_TEST_JWT: randomBytes(40).toString('hex')};
const compose = (...args) => execFileSync('docker', ['compose', '-p', project, '-f', config, ...args], {env, encoding:'utf8', stdio:['ignore','pipe','inherit']}).trim();
const port = (service, n) => {
  const address = compose('port',service,String(n));
  assert.match(address,/^127\.0\.0\.1:\d+$/);
  return 'http://' + address;
};
async function until(check, step){
  for(let i=0;i<50;i++){
    const result = await check().catch(()=>false);
    if(result) return result;
    await delay(400);
  }
  throw Error('Tempo esgotado em '+step);
}

try {
  compose('up','-d','--pull','missing');
  const auth = port('auth',9999), mail = port('mail',8025);
  await until(async()=> (await fetch(auth+'/health')).ok, 'inicialização de GoTrue');
  const upstream = (raw, opts) => {
    const url = new URL(raw);
    assert.equal(url.origin,'https://ynjcyvnrfxsrijrwpaye.supabase.co');
    assert.ok(url.pathname.startsWith('/auth/v1/'));
    return fetch(auth + url.pathname.slice('/auth/v1'.length) + url.search,opts);
  };
  const jar = new Map();
  async function call(action,body,expected=200, useCookies=true){
    const cookies=useCookies?[...jar].map(([k,v])=>k+'='+v).join('; '):'';
    const res=await sessionBridge(new Request(site+'/conta/api/'+action,{method:'POST',headers:{
      origin:site, cookie:cookies, 'content-type':'application/json'},body:JSON.stringify(body)}),upstream);
    const result=await res.json();
    assert.equal(res.status,expected, action+' returned '+res.status+' ('+(result.code??'')+')');
    for(const line of res.headers.getSetCookie()){
      assert.match(line,/; Secure; HttpOnly; SameSite=Lax;/);
      const [pair]=line.split(';'), at=pair.indexOf('='), name=pair.slice(0,at), value=pair.slice(at+1);
      if(value) jar.set(name,value); else jar.delete(name);
    }
    return result;
  }
  async function message(subject,anchor){
    const record=await until(async()=>{
      const {messages}=await (await fetch(mail+'/api/v1/messages')).json();
      return messages.find(m=>m.Subject===subject && m.To.length===1 && m.To[0].Address===addr);
    },subject);
    const detail=await (await fetch(mail+'/api/v1/message/'+record.ID)).json();
    assert.equal(detail.From.Address,'conta@openkoda.test');
    assert.ok(detail.HTML.includes(anchor),'Esperado o modelo em português');
    const matches=[...detail.HTML.matchAll(/href="([^"]+)"/gi)];
    assert.equal(matches.length,1);
    const link=new URL(matches[0][1].replaceAll('&amp;','&'));
    assert.equal(link.origin,'https://auth.openkoda.test');assert.equal(link.pathname,'/auth/v1/verify');
    assert.equal(link.searchParams.get('redirect_to'),site+'/conta/retorno/');
    // The public fake hostname deliberately has no DNS entry. Use the exact emitted path/query on local GoTrue.
    return auth+'/verify'+link.search;
  }
  async function redeem(link,kind){
    const verify=async()=>{const res=await fetch(link,{redirect:'manual'});assert.ok([302,303].includes(res.status));return new URL(res.headers.get('location'));};
    const callback=await verify();
    assert.equal(callback.origin+callback.pathname,site+'/conta/retorno/');
    const code=callback.searchParams.get('code');assert.ok(code);
    await call('exchange',{auth_code:code},409,false);
    const result=await call('exchange',{auth_code:code});
    assert.equal(result.kind,kind);assert.equal(result.user.email,addr);
    assert.ok(!jar.has('__Host-koda-'+kind));
    await call('exchange',{auth_code:code},409);
    const replay=await verify();assert.ok(!replay.searchParams.has('code'));
    assert.ok(replay.searchParams.has('error') || new URLSearchParams(replay.hash.slice(1)).has('error'));
  }
  const old='Koda-'+randomBytes(20).toString('hex'), fresh='Nova-'+randomBytes(20).toString('hex');
  await call('signup',{email:addr,password:old,data:{adult_requested:true,terms_requested:'2026-11-13',privacy_requested:'2026-11-13',marketing_requested:false}});
  await call('password',{email:addr,password:old},400);
  await redeem(await message('Confirme seu cadastro no OpenKoda','Confirmar meu e-mail'),'signup');
  await call('logout',{});
  await call('password',{email:addr,password:old});
  await call('logout',{});
  await call('recover',{email:addr});
  await redeem(await message('Redefina sua senha do OpenKoda','Escolher uma nova senha'),'recover');
  await call('reset',{password:fresh});
  await call('logout',{});
  await call('password',{email:addr,password:old},400);
  assert.equal((await call('password',{email:addr,password:fresh})).user.email,addr);
  console.log('PASS: dois e-mails por SMTP local, modelos em português, PKCE, confirmação, recuperação, uso único e troca efetiva de senha.');
} finally {
  compose('down','--volumes','--remove-orphans');
}
