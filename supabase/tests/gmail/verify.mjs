// Disposable GoTrue, real Resend SMTP and a single authorized inbox.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash, randomBytes, timingSafeEqual} from 'node:crypto';
import {mkdtemp,readFile,unlink,rmdir,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createServer} from 'node:http';
import {setTimeout as pause} from 'node:timers/promises';
import {fileURLToPath} from 'node:url';

const email = process.argv[2];
assert.match(email ?? '', /^[a-z0-9.]+@gmail\.com$/i, 'Use the exact Gmail address of the Resend operator.');
const offline = process.argv[3] === '--offline';
const suffix = randomBytes(10).toString('hex');
const subjects = {
  signup: 'Confirme seu cadastro no OpenKoda [teste ' + suffix + ']',
  recovery: 'Redefina sua senha do OpenKoda [teste ' + suffix + ']'
};
const local = path => fileURLToPath(new URL(path, import.meta.url));
const env = {
  ...process.env, KODA_MAIL_TEST_PASSWORD:randomBytes(30).toString('hex'),
  KODA_MAIL_TEST_JWT:randomBytes(40).toString('hex'),
  KODA_CONFIRM_SUBJECT:subjects.signup, KODA_RECOVER_SUBJECT:subjects.recovery
};
let authOrigin, activeLinkType, currentConfirmation, key;
const receiptDir = await mkdtemp(join(tmpdir(),'koda-inbox-'+suffix+'-'));
let userId;
const web = createServer(async(req, res) => {
  res.setHeader('Cache-Control','no-store');
  res.setHeader('Referrer-Policy','no-referrer');
  res.setHeader('X-Content-Type-Options','nosniff');
  res.setHeader('Content-Type','text/plain; charset=utf-8');
  const refuse = (status, text) => {res.writeHead(status);res.end(text);};
  if (!authOrigin || req.method !== 'GET' || req.headers.host !== new URL(env.KODA_TEST_LOOPBACK).host)
    return refuse(404,'Endereço de verificação desconhecido.');
  try {
    const url = new URL(req.url,env.KODA_TEST_LOOPBACK);
    if (url.pathname === '/auth/v1/verify') {
      const token = url.searchParams.get('token');
      if (!activeLinkType || url.searchParams.get('type') !== activeLinkType ||
          !token || token.length > 256 ||
          url.searchParams.get('redirect_to') !== env.KODA_TEST_LOOPBACK+'/conta/retorno/')
        return refuse(400,'Este link não pertence à etapa atual.');
      const result = await fetch(authOrigin+'/verify'+url.search,{redirect:'manual'});
      const target = new URL(result.headers.get('Location'));
      assert.equal(target.origin,env.KODA_TEST_LOOPBACK);
      assert.equal(target.pathname,'/conta/retorno/');
      if (!target.searchParams.get('code')) return refuse(403,'Este link está inválido ou expirou.');
      res.writeHead(303,{Location:target.href});return res.end();
    }
    if (url.pathname === '/conta/retorno/' && activeLinkType && currentConfirmation) {
      const code = url.searchParams.get('code');
      if (!code || code.length > 1024) return refuse(400,'Código incompleto.');
      const confirm = currentConfirmation;
      await confirm(code);
      return refuse(200,'Link confirmado na conta descartável do OpenKoda.');
    }
    return refuse(404,'Não encontrado.');
  } catch {return refuse(400,'Não foi possível verificar o link.');}
});
await new Promise((resolve,reject)=>web.once('error',reject).listen(0,'127.0.0.1',resolve));
env.KODA_TEST_LOOPBACK = 'http://127.0.0.1:'+web.address().port;
const files = ['-f',local('../mail/compose.yaml'),'-f',local('compose.yaml')];
function compose(args,input) {
  const result = spawnSync('docker',['compose','-p','koda-gmail-'+suffix,...files,...args],{
    env,input,stdio:['pipe','pipe','pipe'],encoding:'utf8',windowsHide:true,timeout:95000
  });
  // Child output may contain a mail URL on an SMTP failure.
  if (result.status !== 0 || result.error) throw Error('Docker falhou na operação '+args[0]+'.');
  return result.stdout.trim();
}
const projectUrl = service => {
  const addr = compose(['port',service,service === 'auth'?'9999':'8025']);
  assert.match(addr,/^127\.0\.0\.1:\d+$/);return 'http://'+addr;
};
async function eventually(fn) {
  for(let i=0;i<60;i++){
    const value=await fn().catch(()=>null);
    if(value)return value;
    await pause(500);
  }
  throw Error('O serviço de teste não ficou pronto a tempo.');
}
function secret() {
  if (offline) return Buffer.from('offline-placeholder\n');
  const child = spawnSync('powershell.exe',['-NoProfile','-STA','-File',local('open-field.ps1')],{
    windowsHide:false,stdio:['ignore','pipe','inherit'],timeout:480000
  });
  if(child.status!==0||!child.stdout?.length) throw Error('Campo seguro cancelado ou indisponível.');
  return child.stdout;
}
async function openFromInbox(kind) {
  const mail=projectUrl('mail');
  const message=await eventually(async()=>{
    const {messages}=await (await fetch(mail+'/api/v1/messages')).json();
    return messages.find(m=>m.Subject===subjects[kind]&&m.To?.[0]?.Address===email);
  });
  const detail=await(await fetch(mail+'/api/v1/message/'+message.ID)).json();
  const href=detail.HTML.match(/href="([^"]+)"/)?.[1]?.replaceAll('&amp;','&');
  const expectedHash=createHash('sha256').update(href).digest('hex');
  const receipt=join(receiptDir,kind+'.sha256');
  const raw=await fetch(mail+'/api/v1/message/'+message.ID+'/raw');
  if(!raw.ok)throw Error('O SMTP local não devolveu a mensagem.');
  const mime=Buffer.from(await raw.arrayBuffer());
  const envelope={key:key.toString('ascii'),mime:mime.toString('base64'),recipient:email,subject:subjects[kind],offline};
  const result=spawnSync('python',[local('relay.py')],{
    input:JSON.stringify(envelope),windowsHide:true,stdio:['pipe','ignore','ignore'],timeout:35000
  });
  if(result.status!==0)throw Error('Falha ao conferir ou transmitir o MIME da mensagem SMTP.');
  if(offline){
    // Exercise the same one-way handoff using a hash of the actually received
    // MIME anchor in the local inbox. The SMTP credential dialog stays closed.
    await writeFile(receipt,expectedHash,{flag:'wx'});
  } else {
    console.log(JSON.stringify({awaiting:kind,subject:subjects[kind],origin:env.KODA_TEST_LOOPBACK,receipt}));
  }
  let received;
  for(let i=0;i<600;i++){
    try{received=(await readFile(receipt,{encoding:'utf8'})).trim();break;}catch(error){if(error.code!=='ENOENT')throw error;}
    await pause(1000);
  }
  if(!/^[a-f0-9]{64}$/.test(received) || !timingSafeEqual(Buffer.from(expectedHash),Buffer.from(received)))throw Error('O link do Gmail não corresponde à mensagem desta tentativa.');
  await unlink(receipt);
  const url=new URL(href);
  if(url.origin!==env.KODA_TEST_LOOPBACK||url.pathname!=='/auth/v1/verify'||
      url.searchParams.get('type')!==kind||url.username||url.password)throw Error('Endereço recebido inesperado.');
  const first=await fetch(url,{redirect:'manual'});
  assert.equal(first.status,303,'O resgate do link falhou.');
  const target=new URL(first.headers.get('location'));
  assert.equal(target.origin,env.KODA_TEST_LOOPBACK);
  assert.equal(target.pathname,'/conta/retorno/');
  const callback=await fetch(target,{redirect:'manual'});assert.equal(callback.status,200,'A troca PKCE falhou.');
  const repeat=await fetch(url,{redirect:'manual'});
  assert.ok([400,403].includes(repeat.status),'O link pôde ser reutilizado.');
}
async function request(path,body,token) {
  const options={method:path==='/user'&&token?'PUT':'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body),redirect:'error'};
  if(token)options.headers.authorization='Bearer '+token;
  const result=await fetch(authOrigin+path,options);
  let data={};try{data=await result.json()}catch{}
  return {status:result.status,data};
}
async function signIn(password, expected) {
  const response=await request('/token?grant_type=password',{email,password});
  assert.equal(response.status,expected,'Resultado de login inesperado.');
  if(expected===200)assert.equal(response.data.user.id,userId);
}
async function verify(kind,password) {
  let session;
  const verifier=randomBytes(48).toString('base64url');
  const hash=Buffer.from(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(verifier))).toString('base64url');
  const callback=env.KODA_TEST_LOOPBACK+'/conta/retorno/';
  activeLinkType=kind;
  currentConfirmation=async code=>{
    const exchange=await request('/token?grant_type=pkce',{auth_code:code,code_verifier:verifier});
    assert.equal(exchange.status,200);
    assert.equal(exchange.data.user.email,email);
    assert.ok(exchange.data.user.email_confirmed_at && exchange.data.access_token);
    if(userId)assert.equal(exchange.data.user.id,userId);
    userId=exchange.data.user.id;
    session=exchange.data.access_token;
    currentConfirmation=undefined;
  };
  const body={email,code_challenge:hash,code_challenge_method:'s256'};
  if(kind==='signup')body.password=password;
  const result=await request((kind==='signup'?'/signup':'/recover')+'?redirect_to='+encodeURIComponent(callback),body);
  if(result.status!==200)throw Error('O serviço recusou a emissão de '+kind+' (HTTP '+result.status+').');
  await openFromInbox(kind);
  assert.ok(session);
  activeLinkType=undefined;
  console.log(kind+' confirmado com uso único.');
  return session;
}
try {
  compose(['up','-d','--pull','never']);
  key=secret();
  authOrigin=projectUrl('auth');
  await eventually(async()=>(await fetch(authOrigin+'/health')).ok);
  const oldPassword=randomBytes(28).toString('hex'),newPassword=randomBytes(28).toString('hex');
  await verify('signup',oldPassword);await signIn(oldPassword,200);
  const access=await verify('recovery');
  assert.equal((await request('/user',{password:newPassword},access)).status,200);
  await signIn(oldPassword,400);await signIn(newPassword,200);
  console.log(offline?'PASS offline. Nenhuma mensagem externa.':'PASS Gmail. Cadastro confirmado, recuperação resgatada e nova senha verificada.');
} catch (error) {
  // Avoid including assertion input or token-bearing URLs in logs.
  console.error('Teste não concluído.',error.code==='ERR_ASSERTION'?'Falha ao conferir a autenticação.':String(error.message).replace(/https?:\/\/\S+/g,'[endereço]'));
  process.exitCode=1;
} finally {
  key?.fill(0);
  await new Promise(resolve=>web.close(resolve));
  compose(['down','--volumes','--remove-orphans']);
  for(const kind of Object.keys(subjects)){await unlink(join(receiptDir,kind+'.sha256')).catch(()=>{});}
  await rmdir(receiptDir);
}
