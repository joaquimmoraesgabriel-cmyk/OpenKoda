import assert from 'node:assert/strict';
import test from 'node:test';
import {createHash} from 'node:crypto';
import {createBillingHandler} from '../functions/asaas/handler.mjs';

const user='11111111-1111-4111-8111-111111111111',orderId='22222222-2222-4222-8222-222222222222';
const checkoutId='33333333-3333-4333-8333-333333333333',requestId='44444444-4444-4444-8444-444444444444';
const checkoutUrl='https://sandbox.asaas.com/checkoutSession/show?id='+checkoutId;
const base='https://ynjcyvnrfxsrijrwpaye.supabase.co',processor='https://api-sandbox.asaas.com';
const subscriptionId='sub_Example',pid='pay_Example',webhookToken='offline-verification-credential-32-characters';
function fixture({mode='sandbox',api={},rpc={},emailConfirmed=true}={}){
  const calls=[];
  const sandbox={environment:'sandbox',billingAnchorDate:'2026-01-31'};
  const order={...sandbox,orderId,plan:'plus',priceBrlCents:21900,status:'pending',existing:false};
  const routes={
    'POST /v3/checkouts':{id:checkoutId,externalReference:orderId,status:'ACTIVE',link:checkoutUrl},
    ['GET /v3/payments/'+pid]:{id:pid,billingType:'CREDIT_CARD',customer:'cus_fixture',status:'CONFIRMED',value:219,
      subscription:subscriptionId,checkoutSession:checkoutId,dueDate:'2026-01-31'},
    ['GET /v3/subscriptions/'+subscriptionId]:{id:subscriptionId,customer:'cus_fixture',billingType:'CREDIT_CARD',
      cycle:'MONTHLY',value:219,checkoutSession:checkoutId,nextDueDate:'2026-02-28'},
    ['DELETE /v3/subscriptions/'+subscriptionId]:{id:subscriptionId,deleted:true},
    ['POST /v3/checkouts/'+checkoutId+'/cancel']:{id:checkoutId,status:'CANCELED'},
  };
  const records={start:order,link:{},checkout:{...sandbox,orderId,checkoutId,priceBrlCents:21900},
    checkout_closed:{closed:true},event:{status:'paid'},cancel_target:{...sandbox,orderId,checkoutId,subscriptionId},
    local_cancel:{cancelAtPeriodEnd:true},cancel_commit:{cancelAtPeriodEnd:true},...rpc};
  const fetcher=async(raw,init)=>{
    const url=new URL(raw),method=init.method??'GET',body=init.body?JSON.parse(init.body):null;
    calls.push({url:url.href,method,body,headers:init.headers});
    assert.equal(init.redirect,'error');
    if(url.origin===base){
      if(url.pathname==='/auth/v1/user')return Response.json({id:user,email_confirmed_at:emailConfirmed?'2026-01-01':null});
      assert.equal(init.headers.apikey,'sb_secret_synthetic');
      const rpcName=url.pathname==='/rest/v1/rpc/openkoda_cancel'?'local_cancel':url.pathname.replace('/rest/v1/rpc/openkoda_billing_','');
      assert.ok(Object.hasOwn(records,rpcName),'unexpected RPC '+rpcName);
      const data=records[rpcName];
      return data instanceof Response?data.clone():Response.json(data);
    }
    assert.equal(url.origin,processor);
    assert.equal(init.headers.access_token,'synthetic-sandbox-api-key');
    const identity=method+' '+url.pathname;
    const data=api[identity]??routes[identity];
    assert.notEqual(data,undefined,'unexpected processor operation');
    return data instanceof Response?data.clone():Response.json(data);
  };
  const config={OPENKODA_ASAAS_MODE:mode,SUPABASE_URL:base,ASAAS_SANDBOX_API_KEY:'synthetic-sandbox-api-key',ASAAS_WEBHOOK_TOKEN:webhookToken,
    SUPABASE_SECRET_KEYS:JSON.stringify({default:'sb_secret_synthetic'}),SUPABASE_PUBLISHABLE_KEYS:JSON.stringify({default:'sb_publishable_synthetic'})};
  const handle=createBillingHandler({env:name=>config[name],fetcher});
  const post=(body,webhook=false,headers={})=>handle(new Request(base+'/functions/v1/asaas'+(webhook?'/webhook':''),{
    method:'POST',headers:{'content-type':'application/json',...(webhook?{'asaas-access-token':webhookToken}:{authorization:'Bearer synthetic-confirmed-jwt'}),...headers},body:JSON.stringify(body)}));
  return {calls,routes,post,records};
}
const start={action:'checkout',plan:'plus',requestId};
const notice={id:'evt_fixture&15370',event:'PAYMENT_CONFIRMED',payment:{id:pid,value:0,status:'PENDING'}};
test('default gate, live mode and untrusted origins cannot contact any service',async()=>{
  for(const mode of ['','production','live']){
    const f=fixture({mode});assert.equal((await f.post(start)).status,503);assert.equal(f.calls.length,0);
  }
  const f=fixture();assert.equal((await f.post(start,false,{origin:'https://untrusted.invalid'})).status,403);assert.equal(f.calls.length,0);
});
test('only confirmed client JWT can create a recurring credit-card checkout at the stored price',async()=>{
  const f=fixture(),ok=await f.post(start);
  assert.equal(ok.status,200);assert.equal((await ok.json()).checkoutUrl,checkoutUrl);
  const created=f.calls.find(c=>c.method==='POST'&&c.url.startsWith(processor));
  assert.deepEqual(created.body.chargeTypes,['RECURRENT']);assert.deepEqual(created.body.billingTypes,['CREDIT_CARD']);
  assert.deepEqual(created.body.subscription,{cycle:'MONTHLY',nextDueDate:'2026-01-31'});assert.equal(created.body.items[0].value,219);
  assert.equal(created.body.externalReference,orderId);assert.equal(f.calls.at(-1).body.p_checkout_id,checkoutId);
  const denied=fixture({emailConfirmed:false});assert.equal((await denied.post(start)).status,403);
  assert.ok(!denied.calls.some(c=>c.url.startsWith(processor)));
  const tamper=fixture();assert.equal((await tamper.post({...start,value:0.01})).status,400);
  assert.ok(!tamper.calls.some(c=>c.url.startsWith(processor)));
});
test('unknown dispatch stays reserved; replayed linked order reuses its checked sandbox URL',async()=>{
  const f=fixture();f.records.start.existing=true;
  assert.equal((await f.post(start)).status,409);assert.ok(!f.calls.some(c=>c.url.startsWith(processor)));
  Object.assign(f.records.start,{status:'linked',checkoutUrl});
  const repeat=await f.post(start);assert.equal((await repeat.json()).replay,true);
  f.records.start.checkoutUrl='https://sandbox.asaas.com@attacker.invalid/checkoutSession/show?id='+checkoutId;
  assert.equal((await f.post(start)).status,409);assert.ok(!f.calls.some(c=>c.url.startsWith(processor)));
});
test('documented ID-only response resolves a fixed sandbox checkout URL; inconsistent optional fields fail',async()=>{
  for(const [created,status] of [[{id:checkoutId},200],[{id:checkoutId,externalReference:requestId},409],
    [{id:checkoutId,status:'PAID'},409],[{id:checkoutId,link:'https://asaas.com/checkoutSession/show?id='+checkoutId},409]]){
    const f=fixture({api:{'POST /v3/checkouts':created}}),response=await f.post(start);
    assert.equal(response.status,status);
    if(status===200)assert.equal((await response.json()).checkoutUrl,checkoutUrl);
    else assert.ok(!f.calls.some(c=>c.url.endsWith('billing_link')));
  }
});
test('database gate, consent and attempt limits all act before processor dispatch',async()=>{
  for(const [message,status] of [['payments_disabled',503],['consent_required',403],['rate_limited',429],['checkout_in_progress',409]]){
    const f=fixture({rpc:{start:Response.json({message},{status:400})}});
    const rejected=await f.post(start);assert.equal(rejected.status,status);assert.equal((await rejected.json()).code,message);
    assert.ok(!f.calls.some(c=>c.url.startsWith(processor)));
  }
});
test('untrusted webhooks cannot query resources; signed notices are cross-checked against payment/checkout/subscription',async()=>{
  const f=fixture();
  assert.equal((await f.post(notice,true,{'asaas-access-token':'bad'})).status,401);assert.equal(f.calls.length,0);
  assert.equal((await f.post(notice,true)).status,200);
  assert.deepEqual(f.calls.at(-1).body,{p_event:createHash('sha256').update(notice.id).digest('hex'),p_order:orderId,p_payment:pid,p_subscription:subscriptionId,p_status:'paid',
    p_amount_cents:21900,p_currency:'BRL',p_cycle_start:'2026-01-31T03:00:00.000Z',p_cycle_end:'2026-02-28T03:00:00.000Z'});
  for(const [endpoint,changes] of [
    ['GET /v3/payments/'+pid,{status:'PENDING'}],['GET /v3/payments/'+pid,{value:1}],['GET /v3/subscriptions/'+subscriptionId,{customer:'cus_other'}],
    ['GET /v3/subscriptions/'+subscriptionId,{checkoutSession:requestId}],['GET /v3/subscriptions/'+subscriptionId,{cycle:'YEARLY'}]]){
    const failed=fixture({api:{[endpoint]:{...f.routes[endpoint],...changes}}});
    assert.equal((await failed.post(notice,true)).status,409);assert.ok(!failed.calls.some(c=>c.url.endsWith('billing_event')));
  }
  const refund=fixture({api:{['GET /v3/payments/'+pid]:{...f.routes['GET /v3/payments/'+pid],status:'REFUNDED'}}});
  assert.deepEqual(await (await refund.post(notice,true)).json(),{ignored:true});assert.ok(!refund.calls.some(c=>c.url.endsWith('billing_event')));
  assert.equal((await refund.post({...notice,event:'PAYMENT_REFUNDED'},true)).status,200);assert.equal(refund.calls.at(-1).body.p_status,'refunded');
});
test('cancel commits exact ownership only after confirmed remote subscription or checkout cancellation',async()=>{
  const f=fixture();assert.equal((await f.post({action:'cancel'})).status,200);assert.equal(f.calls.at(-1).body.p_subscription_id,subscriptionId);
  const pending=fixture({rpc:{cancel_target:{environment:'sandbox',orderId,checkoutId,subscriptionId:null}}});
  assert.equal((await pending.post({action:'cancel'})).status,200);assert.equal(pending.calls.at(-1).body.p_subscription_id,null);
  const bad=fixture({api:{['DELETE /v3/subscriptions/'+subscriptionId]:{id:subscriptionId,deleted:false}}});
  assert.equal((await bad.post({action:'cancel'})).status,409);assert.ok(!bad.calls.some(c=>c.url.endsWith('cancel_commit')));
  const uncertain=fixture({rpc:{cancel_target:{environment:'sandbox',orderId,subscriptionId:null,checkoutId:null}}});
  assert.equal((await uncertain.post({action:'cancel'})).status,409);assert.ok(!uncertain.calls.some(c=>c.url.startsWith(processor)));
  const done=fixture({rpc:{cancel_target:null}});assert.equal((await done.post({action:'cancel'})).status,200);
  assert.ok(done.calls.at(-1).url.endsWith('openkoda_cancel'));assert.ok(!done.calls.some(c=>c.url.startsWith(processor)));
});
test('sandbox actions are bound to the stored environment and renewals to the original anchor',async()=>{
  for(const [record,body,isWebhook] of [['start',start,false],['checkout',notice,true],['cancel_target',{action:'cancel'},false]]){
    const f=fixture();f.records[record].environment='production';
    assert.equal((await f.post(body,isWebhook)).status,409);
    assert.ok(!f.calls.some(c=>c.url.endsWith('billing_event')||c.url.endsWith('billing_link')||c.url.endsWith('cancel_commit')));
    if(!isWebhook)assert.ok(!f.calls.some(c=>c.url.startsWith(processor)));
  }
  const f=fixture(),pay=f.routes['GET /v3/payments/'+pid],sub=f.routes['GET /v3/subscriptions/'+subscriptionId];
  pay.dueDate='2026-02-28';sub.nextDueDate='2027-01-31';
  assert.equal((await f.post(notice,true)).status,200);assert.equal(f.calls.at(-1).body.p_cycle_end,'2026-03-31T03:00:00.000Z');
  pay.dueDate='2026-03-18';assert.equal((await f.post(notice,true)).status,409);
});
test('expired or canceled signed checkouts close only their bound, unpaid order',async()=>{
  for(const status of ['EXPIRED','CANCELED']){
    const f=fixture();
    assert.equal((await f.post({id:'evt_signed&12',event:'CHECKOUT_'+status,checkout:{id:checkoutId,status}},true)).status,200);
    assert.deepEqual(f.calls.at(-1).body,{p_checkout_id:checkoutId,p_environment:'sandbox',p_status:'CHECKOUT_'+status});
    assert.ok(!f.calls.some(c=>c.url.startsWith(processor)||c.url.endsWith('billing_event')));
  }
});
test('renewals without checkout metadata require the previously authenticated subscription binding',async()=>{
  for(const stored of [null,subscriptionId,'sub_wrong']){
    const f=fixture({rpc:{subscription:stored?{environment:'sandbox',billingAnchorDate:'2026-01-31',orderId,checkoutId,
      subscriptionId:stored,priceBrlCents:21900}:null}});
    delete f.routes['GET /v3/payments/'+pid].checkoutSession;
    delete f.routes['GET /v3/subscriptions/'+subscriptionId].checkoutSession;
    const result=await f.post(notice,true);
    assert.equal(result.status,stored===subscriptionId?200:409);
    assert.equal(f.calls.some(c=>c.url.endsWith('billing_event')),stored===subscriptionId);
  }
});
