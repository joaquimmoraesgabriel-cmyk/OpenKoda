import assert from 'node:assert/strict';
import test from 'node:test';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { Readable } from 'node:stream';
import { createHandler } from '../openkoda/handler.mjs';
import { buildInference, verifiedModels } from '../openkoda/models.mjs';

const userId = '5548b5c4-5e39-41c3-a51f-004a5c1770e1';
const requestId = 'e79cc82d-fd86-4201-92d4-5028a5e31f41';
const site = 'https://openkoda-desktop.steelyrowan1.chatgpt.site';
const model = 'google/gemini-2.5-flash-lite';
const catalogRow = {id:model,architecture:{input_modalities:['text','image'],output_modalities:['text']},
  supported_parameters:['max_tokens','reasoning','structured_outputs'],pricing:{prompt:'0.0000001',completion:'0.0000004',image:'0.0000001'},
  context_length:1048576,top_provider:{max_completion_tokens:8192}};
const base = {action:'scan',requestId,model,messages:[{role:'user',content:'Examine os trechos enviados.'}],maxTokens:100,format:'decision'};
const tokens = {prompt_tokens:17,completion_tokens:10,cost:0.0000057};
const json = (data,status=200) => new Response(JSON.stringify(data),{status,headers:{'content-type':'application/json'}});
const sse = frames => new Response(frames.map(entry => 'data: '+(entry === '[DONE]' ? entry : JSON.stringify(entry))+'\n\n').join(''),{headers:{'content-type':'text/event-stream'}});
const goodAnswer = () => json({id:'gen-test',model,usage:tokens,choices:[{finish_reason:'stop',message:{content:'Análise concluída.'}}]});
function fixture(options={}) {
  const calls = [];
  const variables = {SUPABASE_URL:'https://fakeproject.supabase.co',SUPABASE_PUBLISHABLE_KEYS:'{"default":"public-test"}',
    SUPABASE_SECRET_KEYS:'{"default":"sb_secret_fixture"}',OPENKODA_AI_ENABLED:'true',OPENKODA_OPENROUTER_KEY:'fake-test-key',
    OPENKODA_IP_HMAC_KEY:'only-for-offline-tests-not-a-real-secret',...options.env};
  let reserved = false;
  const fetcher = async (url,init) => {
    const path = new URL(url).pathname;
    const body = init.body ? JSON.parse(init.body) : null;
    calls.push({path,body,headers:init.headers});
    if (path === '/auth/v1/user') return options.auth?.() ?? json({id:userId,role:'authenticated',email_confirmed_at:'2026-01-01'});
    if (path === '/api/v1/models') return json({data:[catalogRow]});
    if (path === '/api/v1/chat/completions') return (options.answer ?? goodAnswer)();
    if (path === '/api/v1/generation') return options.receipt?.() ?? json({},404);
    if (path === '/auth/v1/admin/users/'+userId) return json({});
    if (url === 'https://api.resend.com/emails') return options.email?.() ?? json({id:'email-fixture'});
    assert.ok(path.startsWith('/rest/v1/rpc/openkoda_'));
    const name = path.slice('/rest/v1/rpc/openkoda_'.length);
    if (options.rpc) {
      const handled = options.rpc(name,body);
      if (handled) return handled;
    }
    if (name === 'ai_admit') {
      const replay = reserved;
      reserved = true;
      return json({requestId,status:replay?'completed':'reserved',replay});
    }
    if (name === 'ai_dispatch') return json({dispatch:true,status:'dispatched',requestId});
    if (name === 'ai_settle') return json({requestId,status:body.p_status});
    if (name === 'delete') return json({scheduled:true});
    if (name === 'checkout_mock') return json({id:body.p_request,status:'simulated',grantsProductionCredit:false});
    return json({ok:true});
  };
  const app = createHandler({env:name=>variables[name],fetcher});
  const ask = (body,headers={}) => app(new Request('https://fakeproject.supabase.co/functions/v1/openkoda',{
    method:'POST',headers:{'content-type':'application/json',authorization:'Bearer aa.bb.cc',
      'cf-connecting-ip':'192.0.2.7',...headers},body:JSON.stringify(body)}));
  return {app,ask,calls,variables,fetcher};
}

test('auth fails closed and identity, limits and secrets cannot be supplied by clients',async () => {
  const denied = fixture({auth:()=>json({role:'authenticated',id:userId,is_anonymous:true,email_confirmed_at:'fake'})});
  assert.equal((await denied.ask({action:'account'})).status,403);
  assert.equal(denied.calls.length,1);
  const f = fixture();
  assert.equal((await f.ask({action:'account'},{authorization:'sb_secret_fixture'})).status,401);
  assert.equal((await f.ask({action:'account',p_user:'another-user'})).status,400);
  assert.equal((await f.ask({...base,provider:{zdr:false}})).status,400);
  assert.equal((await f.ask({...base,messages:[{role:'system',content:'untrusted'}]})).status,400);
  assert.equal((await f.ask(base,{'cf-connecting-ip':'','x-forwarded-for':'127.0.0.1'})).status,503);
  assert.ok(!f.calls.some(call=>call.path.endsWith('ai_admit')));
});

test('only approved origin is allowed; account results and errors are not cacheable',async () => {
  const f = fixture();
  assert.equal((await f.ask({action:'account'},{origin:'https://unknown.test'})).status,403);
  const allowed = await f.ask({action:'account'},{origin:site});
  assert.equal(allowed.status,200);
  assert.equal(allowed.headers.get('access-control-allow-origin'),site);
  assert.ok(allowed.headers.get('cache-control').includes('no-store'));
  assert.deepEqual(f.calls.at(-1).body,{p_user:userId});
});

test('only verified affordable models appear and client capabilities are constrained',() => {
  const models = verifiedModels({data:[
    catalogRow,{...catalogRow,id:'openrouter/auto'},
    {...catalogRow,pricing:{...catalogRow.pricing,input_cache_write:'25'}},
    {...catalogRow,pricing:{prompt:'0.5',completion:'2'}},
  ]});
  assert.equal(models.length,1);
  const m = models[0];
  assert.throws(()=>buildInference({...base,maxTokens:60000},m));
  assert.throws(()=>buildInference({...base,reasoning:'ultra'},m));
  assert.throws(()=>buildInference({...base,attachments:[{name:'../passwd',content:'foo'}]},m));
  assert.throws(()=>buildInference({...base,action:'chat',format:undefined,attachments:[{name:'a.jpg',mediaType:'image/jpeg',data:'aGVsbG8='}]},m));
  const converted = buildInference({...base,attachments:[{name:'example.txt',content:'only this text'}]},m);
  assert.ok(converted.body.messages.at(-1).content.some(p=>p.text?.includes('only this text')));
  assert.equal(converted.body.provider.data_collection,'deny');
  assert.equal(converted.body.provider.zdr,true);
  assert.equal(converted.body.provider.max_price.request,0);
  assert.equal(converted.body.provider.max_price.image,0.0000001);
});

test('requests above the database one-cent ceiling fail before reservation or inference',async () => {
  const f=fixture();
  const response=await f.ask({...base,maxTokens:2048,messages:[
    {role:'assistant',content:'a'.repeat(47000)},{role:'user',content:'b'.repeat(47000)},
  ]});
  assert.equal(response.status,422);
  assert.equal((await response.json()).code,'request_cost_limit');
  assert.deepEqual(f.calls.map(call=>call.path),['/auth/v1/user','/api/v1/models']);
});

test('scan uses reserve and dispatch before inference, private routing, a bounded schema and settlement',async () => {
  const f = fixture();
  const response = await f.ask(base);
  assert.equal(response.status,200);
  assert.equal((await response.json()).text,'Análise concluída.');
  assert.deepEqual(f.calls.map(c=>c.path.split('/').at(-1)),['user','models','openkoda_ai_admit','openkoda_ai_dispatch','completions','openkoda_ai_settle']);
  const reserve = f.calls[2].body;
  assert.equal(reserve.p_user,userId);
  assert.match(reserve.p_ip_hash,/^[0-9a-f]{64}$/);
  assert.ok(reserve.p_max_cost_micros >= 6);
  const upstream = f.calls[4];
  assert.equal(upstream.headers['X-OpenRouter-Cache'],'false');
  assert.deepEqual(upstream.body.response_format.json_schema.schema.required,['decision']);
  assert.ok(!JSON.stringify(upstream).includes(userId));
  assert.equal(f.calls[5].body.p_actual_cost_micros,6);
  const replay = await f.ask(base);
  assert.equal(replay.status,409);
  assert.equal(f.calls.filter(c=>c.path.endsWith('/completions')).length,1);
});

test('small JPEGs are accepted while metadata before or after image data is rejected',() => {
  const m=verifiedModels({data:[catalogRow]})[0];
  // Synthetic 2 × 2 baseline JPEG.
  const valid=Buffer.from('/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAMCAgMCAgMDAwMEAwMEBQgFBQQEBQoHBwYIDAoMDAsKCwsNDhIQDQ4RDgsLEBYQERMUFRUVDA8XGBYUGBIUFRT/2wBDAQMEBAUEBQkFBQkUDQsNFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBT/wAARCAACAAIDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwD8qqKKKAP/2Q==','base64');
  const make = data => buildInference({...base,action:'chat',format:undefined,attachments:[
    {name:'mini.jpg',mediaType:'image/jpeg',data:data.toString('base64')}]},m);
  const converted=make(valid);
  assert.match(converted.body.messages.at(-1).content.at(-1).image_url.url,/^data:image\/jpeg;base64,/);
  const exif=Buffer.from([255,225,0,8,69,120,105,102,0,0]);
  for (const at of [2,valid.length-2]) {
    assert.throws(()=>make(Buffer.concat([valid.subarray(0,at),exif,valid.subarray(at)])),/jpeg_metadata/);
  }
  assert.throws(()=>make(Buffer.concat([valid,valid])),/invalid_jpeg/);
  const huge=Buffer.from(valid);
  huge.writeUInt16BE(4096,huge.indexOf(Buffer.from([255,192]))+5);
  assert.throws(()=>make(huge),/jpeg_dimensions/);
});

test('disabled service, rejected reservation and non-winning dispatch never call the model',async () => {
  const cases = [
    fixture({env:{OPENKODA_AI_ENABLED:'false'}}),
    fixture({rpc:n=>n==='ai_admit'&&json({message:'weekly_quota_exceeded'},400)}),
    fixture({rpc:n=>n==='ai_dispatch'&&json({dispatch:false,status:'dispatched',requestId})}),
    fixture({rpc:n=>n==='ai_admit'&&json({replay:false})}),
    fixture({rpc:n=>n==='ai_dispatch'&&json({dispatch:true,requestId,status:'reserved'})}),
  ];
  for (const f of cases) {
    assert.ok((await f.ask(base)).status>=400);
    assert.ok(!f.calls.some(call=>call.path.endsWith('/completions')));
  }
});

test('lost or malformed post-dispatch results retain the reservation',async () => {
  for (const answer of [()=>{throw new Error('network');},()=>json({id:'gen-test',model,usage:{},choices:[]})]) {
    const f = fixture({answer});
    const result = await f.ask(base);
    assert.ok(result.status>=500);
    const settlement = f.calls.findLast(c => c.path.endsWith('ai_settle')).body;
    assert.equal(settlement.p_status,'unknown');
    assert.equal(settlement.p_actual_cost_micros,null);
  }
});

test('capacity tickets retry the identical request, authenticate cancellation, and never call the provider while waiting',async()=>{
  const token='50fa8d07-aa43-44c2-b0f1-b90f58aacdf2';
  const waiting={status:'waiting',requestId,queueToken:token,position:3,expiresAt:new Date(Date.now()+120000).toISOString(),retryAfterSeconds:2,replay:false};
  let firstDigest;
  const f=fixture({rpc:(name,body)=>{
    if(name==='ai_admit'){
      assert.equal(body.p_user,userId);
      if(body.p_queue_token===null){firstDigest=body.p_digest;return json(waiting);}
      assert.equal(body.p_digest,firstDigest);assert.equal(body.p_queue_token,token);
      return json({status:'reserved',requestId,replay:false});
    }
    if(name==='cancel_wait'){
      assert.deepEqual(body,{p_user:userId,p_request:requestId,p_queue_token:token});
      return json({requestId,cancelled:true});
    }
  }});
  const wait=await f.ask(base);
  assert.equal(wait.status,429);assert.equal(wait.headers.get('retry-after'),'2');
  assert.deepEqual(await wait.json(),{code:'capacity_wait',requestId,queueToken:token,position:3,retryAfterSeconds:2,expiresAt:waiting.expiresAt});
  assert.ok(!f.calls.some(c=>c.path.endsWith('completions')||c.path.endsWith('ai_dispatch')));
  assert.equal((await f.ask(base,{'x-koda-queue-token':'bad'})).status,400);
  assert.equal((await f.ask(base,{'x-koda-queue-token':token})).status,200);
  assert.equal(f.calls.filter(c=>c.path.endsWith('completions')).length,1);
  assert.equal((await f.ask({action:'cancel_wait',requestId,queueToken:token,p_user:'other'})).status,400);
  const cancelled=await f.ask({action:'cancel_wait',requestId,queueToken:token});
  assert.deepEqual(await cancelled.json(),{requestId,cancelled:true});
  for(const code of ['queue_full','queue_expired','queue_user_limit','quota_exceeded']){
    const denied=fixture({rpc:name=>name==='ai_admit'&&json({message:code},400)});
    const r=await denied.ask(base);assert.equal(r.status,code==='queue_expired'?409:429);
    assert.equal(r.headers.get('retry-after'),null);assert.deepEqual(await r.json(),{code});
    assert.ok(!denied.calls.some(c=>c.path.endsWith('completions')));
  }
});

test('chat streams public deltas and commits measured usage before reporting the final frame',async () => {
  const f = fixture({answer:()=>sse([
    {id:'gen-test',model,choices:[{delta:{reasoning:'private content',content:'Olá'}}]},
    {id:'gen-test',model,choices:[{delta:{content:' mundo'},finish_reason:'stop'}]},
    {id:'gen-test',model,choices:[],usage:tokens},'[DONE]',
  ])});
  const result = await f.ask({...base,action:'chat',format:undefined});
  assert.equal(result.headers.get('content-type'),'text/event-stream; charset=utf-8');
  const stream = await result.text();
  assert.ok(!stream.includes('private content'));
  assert.ok(stream.includes('Olá') && stream.endsWith('data: [DONE]\n\n'));
  assert.equal(f.calls.at(-1).body.p_status,'completed');
});

test('truncated chat has unknown cost and no successful DONE frame',async () => {
  const f = fixture({answer:()=>sse([{id:'gen-test',model,choices:[{delta:{content:'partial'}}]}])});
  const result = await f.ask({...base,action:'chat',format:undefined});
  const text = await result.text();
  assert.ok(text.includes('outcome_unknown') && !text.includes('[DONE]'));
  assert.equal(f.calls.findLast(c=>c.path.endsWith('ai_settle')).body.p_status,'unknown');
});

test('post-dispatch receipt settles the matching generation without another inference',async () => {
  const f = fixture({answer:()=>json({id:'gen-test',model,usage:null}),receipt:()=>json({data:{
    model,id:'gen-test',finish_reason:'stop',native_tokens_prompt:17,native_tokens_completion:10,total_cost:0.0000057}})});
  assert.ok((await f.ask(base)).status>=500);
  const settlements = f.calls.filter(c=>c.path.endsWith('ai_settle')).map(c=>c.body);
  assert.deepEqual(settlements.map(r=>r.p_status),['unknown','completed']);
  assert.equal(settlements[0].p_generation_id,'gen-test');
  assert.equal(settlements[1].p_actual_cost_micros,6);
  assert.equal(f.calls.filter(c=>c.path.endsWith('completions')).length,1);
});

test('loopback HTTP sends the first delta before the provider releases its final receipt', {timeout:7000},async () => {
  const encoder = new TextEncoder();
  let release;
  const gate = new Promise(resolve=>{release=resolve;});
  const provider = new ReadableStream({async start(stream) {
    const frame = value => stream.enqueue(encoder.encode('data: '+JSON.stringify(value)+'\n\n'));
    frame({id:'gen-loop',model,choices:[{delta:{content:'primeiro'}}]});
    await gate;
    frame({id:'gen-loop',model,choices:[{delta:{},finish_reason:'stop'}],usage:tokens});
    stream.enqueue(encoder.encode('data: [DONE]\n\n'));
    stream.close();
  }});
  const f = fixture({answer:()=>new Response(provider,{headers:{'content-type':'text/event-stream'}})});
  let origin;
  const app = createHandler({env:name=>f.variables[name],fetcher:(url,init)=>{
    assert.ok(['openrouter.ai','fakeproject.supabase.co'].includes(new URL(url).hostname));
    return fetch(origin+new URL(url).pathname+new URL(url).search,init);
  }});
  const server = createServer(async (req,res) => {
    try {
      const chunks=[];
      for await (const chunk of req) chunks.push(chunk);
      const body=Buffer.concat(chunks);
      const init={method:req.method,headers:req.headers,body:body.length?body:undefined};
      const response = req.url === '/functions/v1/openkoda'
        ? await app(new Request(origin+req.url,init))
        : await f.fetcher(origin+req.url,init);
      res.writeHead(response.status,Object.fromEntries(response.headers));
      Readable.fromWeb(response.body).pipe(res);
    } catch { res.writeHead(500).end(); }
  });
  try {
    server.listen(0,'127.0.0.1');
    await once(server,'listening');
    origin='http://127.0.0.1:'+server.address().port;
    const result = await fetch(origin+'/functions/v1/openkoda',{method:'POST',headers:{
      'content-type':'application/json',authorization:'Bearer aa.bb.cc','cf-connecting-ip':'192.0.2.7'},
      body:JSON.stringify({...base,action:'chat',format:undefined}),signal:AbortSignal.timeout(5000)});
    assert.equal(result.status,200);
    const reader=result.body.getReader();
    assert.match(new TextDecoder().decode((await reader.read()).value),/primeiro/);
    assert.ok(!f.calls.some(c=>c.path.endsWith('ai_settle')));
    release();
    let rest='';
    for (;;) {
      const {done,value}=await reader.read();
      if (done) break;
      rest+=new TextDecoder().decode(value);
    }
    assert.ok(rest.includes('[DONE]'));
    assert.equal(f.calls.at(-1).body.p_status,'completed');
    assert.equal(f.calls.find(c=>c.path.endsWith('completions')).headers.authorization,'Bearer fake-test-key');
  } finally {
    release();
    server.closeAllConnections();
    await new Promise(resolve=>server.close(resolve));
  }
});

test('checkout is explicitly simulated; consent, support and purge use the authenticated owner',async () => {
  const f = fixture();
  const checkout = await (await f.ask({action:'checkout',plan:'plus',requestId})).json();
  assert.deepEqual(checkout,{id:requestId,status:'simulated',grantsProductionCredit:false});
  assert.equal((await f.ask({action:'consent',termsVersion:'2026-11-13',privacyVersion:'2026-11-13',marketing:false})).status,200);
  assert.equal((await f.ask({action:'support',subject:'Bug',message:'Reproduzível ao abrir',requestId})).status,200);
  assert.equal((await f.ask({action:'delete',confirmation:'no'})).status,400);
  assert.equal((await f.ask({action:'delete',confirmation:'EXCLUIR'})).status,200);
  assert.ok(f.calls.every(c=>c.path.startsWith('/rest/v1/') || c.path.startsWith('/auth/v1/')));
  for (const c of f.calls.filter(c=>c.path.startsWith('/rest/v1/'))) assert.equal(c.body.p_user,userId);
});

test('initial consent delegates to the authenticated insert-once RPC and returns its replay status', async () => {
  let existing = false;
  const f = fixture({rpc(name, body) {
    assert.equal(name, 'consent_initial');
    assert.deepEqual(body, {p_user:userId, p_terms_version:'2026-11-13', p_privacy_version:'2026-11-13', p_marketing:!existing});
    const result = json({accepted:true, existing});
    existing = true;
    return result;
  }});
  const first = {action:'consent_initial', termsVersion:'2026-11-13', privacyVersion:'2026-11-13', marketing:true};
  assert.equal((await f.ask({...first, p_user:requestId})).status, 400);
  assert.equal((await f.ask({...first, marketing:'true'})).status, 400);
  for (const replay of [false, true]) {
    const result = await f.ask({...first, marketing:!replay});
    assert.equal(result.status, 200);
    assert.deepEqual(await result.json(), {accepted:true, existing:replay});
  }
});

test('support retries transient failures with a stable key and never exports ticket content',async () => {
  let created=true,attempt=0,queued=true;
  const f=fixture({env:{RESEND_API_KEY:'fake-resend',KODA_SUPPORT_FROM:'koda@example.test'},
    email:()=>{if(attempt===1)throw new Error('offline');return json({id:'sent'});},
    rpc:(name,body)=>{
      if(name==='support_create')return json({id:requestId,created});
      if(name==='support_notice_claim')return json({tickets:queued?[{id:requestId,attempt:++attempt}]:[]});
      if(name==='support_notice_finish'){
        assert.equal(body.p_id,requestId);
        assert.equal(body.p_outcome,attempt===1?'retry':'sent');
        queued=body.p_outcome==='retry';return json({updated:true});
      }
    }});
  const ticket={action:'support',subject:'PRIVATE-SUBJECT',message:'PRIVATE-MESSAGE',requestId};
  assert.equal((await f.ask(ticket)).status,200);
  created=false;
  assert.equal((await f.ask(ticket)).status,200);
  assert.equal((await f.ask({action:'support_list'})).status,200);
  const notices=f.calls.filter(c=>c.path==='/emails');
  assert.equal(notices.length,2);
  assert.deepEqual(notices[0],notices[1]);
  assert.equal(notices[0].headers['Idempotency-Key'],'koda-support/'+requestId);
  assert.deepEqual(notices[0].body.to,['joaquimmoraesgabriel@gmail.com']);
  assert.ok(!JSON.stringify(notices[0].body).includes('PRIVATE-') && !JSON.stringify(notices[0].body).includes(userId));
});

test('support records a permanent sender rejection and limits who controls delivery',async () => {
  const f=fixture({env:{RESEND_API_KEY:'fixture',KODA_SUPPORT_FROM:'koda@example.test'},email:()=>json({},403),
    rpc:name=>name==='support_notice_claim' && json({tickets:[{id:requestId,attempt:1}]})});
  assert.equal((await f.ask({action:'support_list'})).status,200);
  assert.equal(f.calls.find(c=>c.path.endsWith('support_notice_finish')).body.p_outcome,'failed');
  assert.equal((await f.ask({action:'support_notice_claim'})).status,404);
  const unconfigured=fixture();
  assert.equal((await unconfigured.ask({action:'support_list'})).status,200);
  assert.ok(!unconfigured.calls.some(c=>c.path.endsWith('support_notice_claim')));
});

test('budget notices retry with stable content and key without retrying model generations',async () => {
  const event={windowStart:'2026-11-13',kind:'day',threshold:80};
  let permit=true, attempt=0, delivered=false;
  const f=fixture({env:{KODA_BUDGET_EMAIL_ENABLED:'true',RESEND_API_KEY:'fake-resend',KODA_SUPPORT_FROM:'koda@example.test'},
    email:()=>delivered ? json({id:'mail-fixture'}) : json({},503),
    rpc(name,body) {
      if (name==='budget_notice_claim') {
        if (!permit) return json({alerts:[]});
        permit=false;
        return json({alerts:[{...event,attempt:++attempt}]});
      }
      if (name==='budget_notice_finish') return json({updated:true});
      if (name==='ai_admit' && delivered) return json({code:'P0001',message:'budget_exceeded'},400);
    }});
  assert.equal((await f.ask(base)).status,200);
  let acks=f.calls.filter(c=>c.path.endsWith('budget_notice_finish'));
  assert.deepEqual(acks.map(c=>c.body.p_sent),[false]);
  delivered=permit=true;
  assert.equal((await f.ask({...base,requestId:'6695de83-094e-4391-bb00-d22e47b6ad9c'})).status,429);
  acks=f.calls.filter(c=>c.path.endsWith('budget_notice_finish'));
  assert.deepEqual(acks.map(c=>c.body.p_sent),[false,true]);
  const messages=f.calls.filter(c=>c.path==='/emails');
  assert.equal(messages.length,2);
  assert.equal(messages[0].headers['Idempotency-Key'],'koda-budget/2026-11-13/day/80');
  assert.deepEqual(messages[0],messages[1]);
  assert.deepEqual(messages[0].body.to,['joaquimmoraesgabriel@gmail.com']);
  for (const privateValue of [userId,requestId,base.messages[0].content,model,'fake-resend'])
    assert.ok(!JSON.stringify(messages[0].body).includes(privateValue));
  assert.equal(f.calls.filter(c=>c.path.endsWith('completions')).length,1);
});

test('budget notices need explicit enablement and never share client-side controls',async () => {
  const f=fixture({env:{RESEND_API_KEY:'fake-resend',KODA_SUPPORT_FROM:'koda@example.test'}});
  assert.equal((await f.ask(base)).status,200);
  assert.equal((await f.ask({action:'budget_notice_claim'})).status,404);
  assert.ok(!f.calls.some(c=>c.path.includes('budget_notice') || c.path==='/emails'));
});
