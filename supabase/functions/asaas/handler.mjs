import { ApiError, fields, fingerprint, readJson, requireValue, uuid } from '../openkoda/limits.mjs';

const SITE = 'https://openkoda-desktop.steelyrowan1.chatgpt.site';
const SANDBOX = 'https://api-sandbox.asaas.com/v3';
const SUPABASE = 'https://ynjcyvnrfxsrijrwpaye.supabase.co';
const orderPlans = ['plus','pro','ultra'];
const paymentId = s => typeof s === 'string' && /^pay_[A-Za-z\d_-]{1,120}$/.test(s);
const subscriptionId = s => typeof s === 'string' && /^sub_[A-Za-z\d_-]{1,120}$/.test(s);
const valid = (condition, code='provider_mismatch') => requireValue(condition,code,409);
const logo = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aTqkAAAAASUVORK5CYII=';

function hostedLink(raw, id) {
  valid(typeof raw === 'string' && raw.length <= 1024 && uuid(id),'invalid_checkout_link');
  const url = URL.parse(raw);
  valid(url?.origin === 'https://sandbox.asaas.com' && !url.username && !url.password && !url.hash &&
    (url.pathname === '/checkoutSession/show' && url.search === '?id='+id ||
    url.pathname === '/checkoutSession/show/'+id && !url.search),'invalid_checkout_link');
  return url.href;
}
function existingLink(raw) {
  const url = typeof raw === 'string' ? URL.parse(raw) : null;
  const id = url?.searchParams.get('id') ?? url?.pathname.split('/').at(-1);
  return hostedLink(raw,id);
}
function cents(amount) {
  valid(typeof amount === 'number' && amount>0 && /^\d+(\.\d{1,2})?$/.test(String(amount)));
  const [integral, decimal=''] = String(amount).split('.');
  const converted=Number(integral)*100+Number(decimal.padEnd(2,'0'));
  valid(Number.isSafeInteger(converted));return converted;
}
function dateValue(day) {
  valid(typeof day==='string' && /^\d{4}-\d{2}-\d{2}$/.test(day));
  const start = new Date(day+'T03:00:00Z');
  valid(Number.isFinite(start.valueOf())&&start.toISOString().startsWith(day));
  return start;
}
function cycle(dueDate, anchorDate) {
  // Replays use the original order's anchor, including when a 31st rolls
  // back to February's last day and then returns to the 31st in March.
  const due=dateValue(dueDate), anchor=dateValue(anchorDate);
  const year=due.getUTCFullYear(), month=due.getUTCMonth();
  function boundary(monthOffset) {
    const lastDay=new Date(Date.UTC(year,month+monthOffset+1,0)).getUTCDate();
    return new Date(Date.UTC(year,month+monthOffset,Math.min(anchor.getUTCDate(),lastDay),3));
  }
  valid(due>=anchor&&due.valueOf()===boundary(0).valueOf(),'payment_schedule_mismatch');
  return [due.toISOString(),boundary(1).toISOString()];
}
async function tokenMatches(actual,expected) {
  if(typeof expected!=='string'||expected.length<32||typeof actual!=='string'||actual.length>1024)return false;
  const bytes=s=>new TextEncoder().encode(s);
  const [a,b]=await Promise.all([actual,expected].map(v=>crypto.subtle.digest('SHA-256',bytes(v))));
  const left=new Uint8Array(a),right=new Uint8Array(b);let diff=0;
  for(let i=0;i<left.length;i++)diff|=left[i]^right[i];
  return diff===0;
}
export function createBillingHandler({env,fetcher=fetch}) {
  // Nothing is deployed or enabled by importing this module. The only supported
  // processor origin in this adapter is the sandbox. SQL adds its own gate.
  function key(group,fallback) {
    const value=env(group);
    return value?JSON.parse(value).default:env(fallback);
  }
  async function call(url,init,{database=false}={}) {
    const res=await fetcher(url,{...init,redirect:'error',signal:AbortSignal.timeout(10000)});
    if(!res.ok){
      if(database&&res.status===400){
        const {message}=await readJson(res,4096,502);
        const known={payments_disabled:503,rate_limited:429,consent_required:403,checkout_in_progress:409,
          idempotency_conflict:409,payment_mismatch:409,cancellation_target_changed:409,cycle_overlap:409};
        if(Object.hasOwn(known,message))throw new ApiError(known[message],message);
      }
      throw new ApiError(res.status===401||res.status===403?403:res.status===429?429:502,'upstream_refused');
    }
    return readJson(res,128*1024,502);
  }
  async function rpc(name,args){
    const service=key('SUPABASE_SECRET_KEYS','SUPABASE_SERVICE_ROLE_KEY');
    requireValue(!!service,'configuration_unavailable',503);
    const headers={apikey:service,'content-type':'application/json'};
    if(!service.startsWith('sb_secret_'))headers.authorization='Bearer '+service;
    return call(SUPABASE+'/rest/v1/rpc/openkoda_'+name,{method:'POST',headers,body:JSON.stringify(args)},{database:true});
  }
  async function authenticate(request){
    const publishable=key('SUPABASE_PUBLISHABLE_KEYS','SUPABASE_ANON_KEY');
    requireValue(!!publishable,'configuration_unavailable',503);
    const bearer=request.headers.get('authorization')??'';
    requireValue(/^Bearer [\w.-]{16,16384}$/.test(bearer),'authentication_required',401);
    const user=await call(SUPABASE+'/auth/v1/user',{headers:{apikey:publishable,authorization:bearer}});
    requireValue(uuid(user.id)&&!!user.email_confirmed_at&&!user.is_anonymous,'confirmed_account_required',403);
    return user.id;
  }
  async function provider(path,method='GET',data){
    const apiKey=env('ASAAS_SANDBOX_API_KEY');
    requireValue(!!apiKey,'configuration_unavailable',503);
    return call(SANDBOX+path,{method,headers:{access_token:apiKey,'user-agent':'OpenKoda/0.2','content-type':'application/json'},...(data?{body:JSON.stringify(data)}:{})});
  }
  async function checkout(user,input){
    fields(input,['action','plan','requestId']);requireValue(orderPlans.includes(input.plan)&&uuid(input.requestId));
    const ids={p_user:user,p_request:input.requestId};
    const order=await rpc('billing_start',{...ids,p_plan:input.plan});
    valid(order?.environment==='sandbox','checkout_environment_mismatch');
    valid(uuid(order.orderId)&&order.plan===input.plan&&Number.isSafeInteger(order.priceBrlCents)&&order.priceBrlCents>0);
    if(order.existing){
      // POST /checkouts has no documented idempotency contract. If creation
      // may have reached Asaas before a local failure, never send it again.
      valid(order.status==='linked'&&order.checkoutUrl,'checkout_reconciliation_required');
      return {orderId:order.orderId,checkoutUrl:existingLink(order.checkoutUrl),replay:true};
    }
    valid(order.status==='pending'&&order.existing===false);
    dateValue(order.billingAnchorDate);
    const created=await provider('/checkouts','POST',{
      billingTypes:['CREDIT_CARD'],chargeTypes:['RECURRENT'],externalReference:order.orderId,minutesToExpire:30,
      callback:{successUrl:SITE+'/conta/',cancelUrl:SITE+'/precos/',expiredUrl:SITE+'/precos/'},
      items:[{name:'OpenKoda '+order.plan,quantity:1,value:order.priceBrlCents/100,imageBase64:logo}],
      subscription:{cycle:'MONTHLY',nextDueDate:order.billingAnchorDate},
    });
    valid(uuid(created.id)&&(created.externalReference==null||created.externalReference===order.orderId)
      &&(created.status==null||created.status==='ACTIVE'));
    // The Asaas checkout guide also documents a minimal response with only
    // the checkout UUID. The public path and sandbox host are fixed here.
    const checkoutUrl=hostedLink(created.link??'https://sandbox.asaas.com/checkoutSession/show?id='+created.id,created.id);
    await rpc('billing_link',{...ids,p_checkout_id:created.id,p_url:checkoutUrl});
    return {orderId:order.orderId,checkoutUrl,replay:false};
  }
  async function cancel(user,input){
    fields(input,['action']);
    const target=await rpc('billing_cancel_target',{p_user:user});
    if(!target)return rpc('cancel',{p_user:user});
    valid(target.environment==='sandbox','checkout_environment_mismatch');
    valid(uuid(target.orderId),'invalid_order');
    if(target.subscriptionId){
      valid(subscriptionId(target.subscriptionId));
      const path='/subscriptions/'+target.subscriptionId;
      const current=await provider(path);
      valid(current.id===target.subscriptionId&&(current.checkoutSession==null||current.checkoutSession===target.checkoutId),'cancellation_target_changed');
      if(current.deleted!==true){
        const removed=await provider(path,'DELETE');
        valid(removed.id===target.subscriptionId&&removed.deleted===true,'cancellation_unconfirmed');
      }
    }else{
      valid(uuid(target.checkoutId),'checkout_reconciliation_required');
      const removed=await provider('/checkouts/'+target.checkoutId+'/cancel','POST');
      valid(removed.id===target.checkoutId&&['CANCELED','EXPIRED'].includes(removed.status),'cancellation_unconfirmed');
    }
    return rpc('billing_cancel_commit',{p_user:user,p_order:target.orderId,p_checkout_id:target.checkoutId,p_subscription_id:target.subscriptionId??null});
  }
  async function webhook(request){
    requireValue(await tokenMatches(request.headers.get('asaas-access-token'),env('ASAAS_WEBHOOK_TOKEN')),'webhook_unauthorized',401);
    const event=await readJson(request,128*1024);
    requireValue(typeof event.id==='string'&&/^[\x21-\x7e]{5,256}$/.test(event.id));
    if(['CHECKOUT_CANCELED','CHECKOUT_EXPIRED'].includes(event.event)){
      const checkout=event.checkout;
      valid(uuid(checkout?.id)&&checkout.status===event.event.slice('CHECKOUT_'.length));
      return rpc('billing_checkout_closed',{p_checkout_id:checkout.id,p_environment:'sandbox',p_status:event.event});
    }
    const paid=['PAYMENT_RECEIVED','PAYMENT_CONFIRMED'].includes(event.event);
    if(!paid&&event.event!=='PAYMENT_REFUNDED')return {ignored:true};
    const pid=event.payment?.id;
    requireValue(paymentId(pid));
    const pay=await provider('/payments/'+pid);
    valid(pay.id===pid&&subscriptionId(pay.subscription)&&pay.billingType==='CREDIT_CARD'&&!pay.deleted);
    const sub=await provider('/subscriptions/'+pay.subscription);
    valid(sub.id===pay.subscription&&sub.billingType==='CREDIT_CARD'&&sub.cycle==='MONTHLY');
    valid(typeof sub.customer==='string'&&!!sub.customer&&pay.customer===sub.customer);
    const checkoutId=sub.checkoutSession??pay.checkoutSession;
    if(checkoutId!=null)valid(uuid(checkoutId)&&(!pay.checkoutSession||pay.checkoutSession===checkoutId));
    // On renewals Asaas can omit checkoutSession. Only the original payment
    // verified via checkout may have bound the subscription ID to an order.
    const order=checkoutId==null?await rpc('billing_subscription',{p_subscription:sub.id})
      :await rpc('billing_checkout',{p_checkout_id:checkoutId});
    valid(order?.environment==='sandbox','checkout_environment_mismatch');
    valid(uuid(order.orderId)&&uuid(order.checkoutId)&&(checkoutId==null?order.subscriptionId===sub.id:order.checkoutId===checkoutId)
      &&(order.subscriptionId==null||order.subscriptionId===sub.id)&&order.priceBrlCents===cents(pay.value)&&order.priceBrlCents===cents(sub.value));
    if(paid&&pay.status==='REFUNDED')return {ignored:true};
    const status=!paid&&pay.status==='REFUNDED'?'refunded':paid&&['CONFIRMED','RECEIVED'].includes(pay.status)?'paid':null;
    valid(status,'payment_unconfirmed');
    const [start,end]=cycle(pay.dueDate,order.billingAnchorDate);
    // Official event IDs can contain punctuation such as "&".
    const eventKey=await fingerprint(event.id);
    return rpc('billing_event',{p_event:eventKey,p_order:order.orderId,p_payment:pid,p_subscription:sub.id,
      p_status:status,p_amount_cents:order.priceBrlCents,p_currency:'BRL',p_cycle_start:start,p_cycle_end:end});
  }
  return async request=>{
    const origin=request.headers.get('origin');
    const headers={'content-type':'application/json','cache-control':'no-store','vary':'Origin'};
    const json=(body,status=200)=>new Response(JSON.stringify(body),{status,headers});
    try{
      if(origin){requireValue(origin===SITE,'origin_denied',403);headers['access-control-allow-origin']=SITE;}
      if(request.method==='OPTIONS'){
        headers['access-control-allow-methods']='POST, OPTIONS';
        headers['access-control-allow-headers']='authorization, apikey, content-type';
        return new Response(null,{status:204,headers});
      }
      requireValue(env('OPENKODA_ASAAS_MODE')==='sandbox'&&env('SUPABASE_URL')===SUPABASE,'payments_disabled',503);
      requireValue(request.method==='POST','method_not_allowed',405);
      requireValue(request.headers.get('content-type')?.split(';')[0]==='application/json','json_required',415);
      const webhookPath=new URL(request.url).pathname.endsWith('/asaas/webhook');
      if(webhookPath)return json(await webhook(request));
      const user=await authenticate(request),input=await readJson(request,2048);
      if(input.action==='checkout')return json(await checkout(user,input));
      if(input.action==='cancel')return json(await cancel(user,input));
      throw new ApiError(400,'unknown_action');
    }catch(e){return json({code:e instanceof ApiError?e.code:'service_unavailable'},e instanceof ApiError?e.status:503);}
  };
}
