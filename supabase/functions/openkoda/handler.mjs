import { ApiError, fields, fingerprint, ipFingerprint, readJson, requireValue, textField, uuid } from './limits.mjs';
import { buildInference, verifiedModels } from './models.mjs';
import { chatStream, generationId, metering } from './stream.mjs';

const PLATFORM_SITE = 'https://openkoda-desktop.steelyrowan1.chatgpt.site';
const ROUTER = 'https://openrouter.ai/api/v1';
const accountActions = {
  account:['account',[]], export:['export',[]], cancel:['cancel',[]], support_list:['support_list',[]],
  skills:['skills',[]], skill:['skill',['id']], marketing:['marketing',['enabled']],
  consent:['consent',['termsVersion','privacyVersion','marketing']],
  consent_initial:['consent_initial',['termsVersion','privacyVersion','marketing']],
  checkout:['checkout_mock',['plan','requestId']],
  support:['support_create',['subject','message','requestId']], delete:['delete',['confirmation']],
};

function accountArgs(action, input) {
  const [,allowed] = accountActions[action];
  fields(input, ['action',...allowed]);
  if (allowed.includes('requestId')) requireValue(uuid(input.requestId));
  switch (action) {
    case 'consent':
    case 'consent_initial':
      requireValue(typeof input.marketing === 'boolean');
      return {p_terms_version:textField(input.termsVersion,64),p_privacy_version:textField(input.privacyVersion,64),p_marketing:input.marketing};
    case 'marketing':
      requireValue(typeof input.enabled === 'boolean');
      return {p_enabled:input.enabled};
    case 'checkout':
      requireValue(['plus','pro','ultra'].includes(input.plan));
      return {p_plan:input.plan,p_request:input.requestId};
    case 'support': return {p_subject:textField(input.subject,160),p_message:textField(input.message,4000),p_request:input.requestId};
    case 'skill': return {p_skill:textField(input.id,96)};
    case 'delete': requireValue(input.confirmation === 'EXCLUIR','confirmation_required'); return {};
    default: return {};
  }
}

export function createHandler({env, fetcher = fetch, waitUntil}) {
  let catalogCache, catalogLoading;
  const key = (dictionary, fallback) => {
    const json = env(dictionary);
    return json ? JSON.parse(json).default : env(fallback);
  };
  const supabase = () => {
    const url = env('SUPABASE_URL');
    requireValue(/^https:\/\/[a-z0-9-]+\.supabase\.co$/.test(url ?? ''),'configuration_unavailable',503);
    return url;
  };
  const service = () => {
    const token = key('SUPABASE_SECRET_KEYS','SUPABASE_SERVICE_ROLE_KEY');
    requireValue(!!token,'configuration_unavailable',503);
    const headers = {apikey:token,'content-type':'application/json'};
    if (!token.startsWith('sb_secret_')) headers.authorization = 'Bearer '+token;
    return headers;
  };
  async function rpc(name, args) {
    const res = await fetcher(supabase()+'/rest/v1/rpc/openkoda_'+name,{
      method:'POST',headers:service(),body:JSON.stringify(args),redirect:'error',signal:AbortSignal.timeout(9000)});
    if (!res.ok) {
      const fault = await readJson(res,8192,502).catch(() => ({}));
      // Never return DB detail or hint; validation failures may echo supplied text.
      const message = typeof fault.message === 'string' ? fault.message : '';
      const safe = /^(?:[a-z][a-z_]{1,64})$/.test(message) ? message : 'operation_refused';
      const status = /quota|rate_limit|concurrency|budget/.test(safe) ? 429 :
        /disabled|kill_switch/.test(safe) ? 503 : /conflict|idempotency/.test(safe) ? 409 :
        res.status >= 500 || /PGRST202/.test(fault.code) ? 503 : 403;
      throw new ApiError(status,safe);
    }
    return readJson(res,name === 'export' ? 8*1024*1024 : 2*1024*1024,502);
  }

  async function sendNotice(id, subject, message) {
    const project = new URL(supabase()).hostname.split('.')[0];
    const notice = await fetcher('https://api.resend.com/emails',{method:'POST',redirect:'error',
      signal:AbortSignal.timeout(3500),headers:{authorization:'Bearer '+env('RESEND_API_KEY'),
        'content-type':'application/json','Idempotency-Key':id},
      body:JSON.stringify({from:env('KODA_SUPPORT_FROM'),to:['joaquimmoraesgabriel@gmail.com'],subject,
        text:message+' Consulte o painel administrativo: https://supabase.com/dashboard/project/'+project+'/editor?schema=openkoda_private'})});
    await notice.body?.cancel();
    return notice.ok;
  }

  async function deliverBudgetNotices() {
    const {alerts} = await rpc('budget_notice_claim',{});
    for (const a of alerts) {
      const labels = {day:'orçamento diário',month:'orçamento mensal',daily_spike:'exposição diária em relação à média recente'};
      if (!Object.hasOwn(labels,a.kind) || !/^\d{4}-\d{2}-\d{2}$/.test(a.windowStart) ||
          !(a.kind === 'daily_spike' ? a.threshold === 200 : [80,95].includes(a.threshold))) continue;
      const id = ['koda-budget',a.windowStart,a.kind,a.threshold].join('/');
      const message = 'A exposição do serviço atingiu '+a.threshold+'% do valor de referência ('+labels[a.kind]+'). Período UTC iniciado em '+a.windowStart+'.';
      const sent = await sendNotice(id,'Koda · Alerta de orçamento',message).catch(() => false);
      await rpc('budget_notice_finish',{p_window:a.windowStart,p_kind:a.kind,p_threshold:a.threshold,
        p_attempt:a.attempt,p_sent:sent}).catch(() => {});
    }
  }

  async function notifyBudget() {
    if (env('KODA_BUDGET_EMAIL_ENABLED') !== 'true' || !env('RESEND_API_KEY') || !env('KODA_SUPPORT_FROM')) return;
    const task = deliverBudgetNotices().catch(() => {});
    if (waitUntil) waitUntil(task);
    else await task;
  }

  async function authenticate(request) {
    const auth = request.headers.get('authorization') ?? '';
    requireValue(/^Bearer [A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/.test(auth) && auth.length < 16384,'unauthenticated',401);
    const apiKey = key('SUPABASE_PUBLISHABLE_KEYS','SUPABASE_ANON_KEY');
    requireValue(!!apiKey,'configuration_unavailable',503);
    const res = await fetcher(supabase()+'/auth/v1/user',{headers:{authorization:auth,apikey:apiKey},redirect:'error',signal:AbortSignal.timeout(7000)});
    requireValue(res.ok,res.status >= 500 ? 'auth_unavailable' : 'unauthenticated',res.status >= 500 ? 503 : 401);
    const user = await readJson(res,65536,502);
    requireValue(uuid(user.id) && user.role === 'authenticated' && user.email_confirmed_at && !user.is_anonymous,'verified_account_required',403);
    return user.id;
  }

  async function loadCatalog() {
    if (catalogCache && Date.now() < catalogCache.until) return catalogCache.rows;
    if (!catalogLoading) catalogLoading = (async () => {
      const res = await fetcher(ROUTER+'/models?zdr=true',{redirect:'error',signal:AbortSignal.timeout(7000)});
      requireValue(res.ok,'catalog_unavailable',503);
      const limits = {input:Number(env('OPENKODA_MAX_INPUT_USD_PER_MILLION') ?? '0.20'),
        output:Number(env('OPENKODA_MAX_OUTPUT_USD_PER_MILLION') ?? '0.50')};
      requireValue(Object.values(limits).every(v=>Number.isFinite(v) && v > 0 && v <= 100),'invalid_price_policy',503);
      const rows = verifiedModels(await readJson(res,20*1024*1024,502),limits);
      requireValue(rows.length,'catalog_unavailable',503);
      catalogCache = {rows,until:Date.now()+60000};
      return rows;
    })().finally(() => { catalogLoading = undefined; });
    return catalogLoading;
  }

  return async function handle(request) {
    const headers = new Headers({'cache-control':'no-store, private','vary':'Origin, Authorization',
      'x-content-type-options':'nosniff','content-type':'application/json; charset=utf-8'});
    const json = (value,status = 200) => new Response(JSON.stringify(value),{status,headers});
    try {
      const origin = request.headers.get('origin');
      if (origin) {
        requireValue(origin === (env('OPENKODA_SITE_ORIGIN') || PLATFORM_SITE), 'origin_refused',403);
        headers.set('access-control-allow-origin',origin);
      }
      if (request.method === 'OPTIONS') {
        headers.set('access-control-allow-methods','POST, OPTIONS');
        headers.set('access-control-allow-headers','authorization, apikey, content-type, x-client-info');
        return new Response(null,{status:204,headers});
      }
      requireValue(request.method === 'POST','method_not_allowed',405);
      requireValue(request.headers.get('content-type')?.split(';')[0].trim() === 'application/json','json_required',415);
      const user = await authenticate(request);
      const input = await readJson(request,480*1024);
      const action = input?.action;
      if (Object.hasOwn(accountActions,action)) {
        const args = accountArgs(action,input);
        const value = await rpc(accountActions[action][0],{p_user:user,...args});
        if (action === 'support' && value?.created === true && uuid(value.id) &&
            env('RESEND_API_KEY') && env('KODA_SUPPORT_FROM')) {
          try {
            await sendNotice('koda-support/'+value.id,'Koda · Novo chamado de suporte','Um chamado foi registrado.');
          } catch { /* The committed ticket remains available in the dashboard. */ }
        }
        if (action === 'delete') {
          requireValue(value?.scheduled === true,'deletion_pending',503);
          const res = await fetcher(supabase()+'/auth/v1/admin/users/'+user,{
            method:'DELETE',headers:service(),redirect:'error',signal:AbortSignal.timeout(9000)});
          requireValue(res.ok || res.status === 404,'deletion_pending',503);
          return json({deleted:true});
        }
        return json(value);
      }
      requireValue(['models','chat','scan'].includes(action),'unknown_action',404);
      if (action === 'models') {
        fields(input,['action']);
        return json({models:await loadCatalog()});
      }
      const routerKey = env('OPENKODA_OPENROUTER_KEY');
      requireValue(env('OPENKODA_AI_ENABLED') === 'true' && routerKey,'inference_disabled',503);
      const ipHash = await ipFingerprint(request,env('OPENKODA_IP_HMAC_KEY'));
      const models = await loadCatalog();
      const model = models.find(row => row.id === input.model);
      const {body,reservation} = buildInference(input,model);
      const identity = {p_user:user,p_request:input.requestId};
      const reserveArgs = {...identity,p_digest:await fingerprint(JSON.stringify(body)),p_model:model.id,
        p_operation:action === 'scan' ? 'analysis' : 'chat',p_ip_hash:ipHash,p_max_cost_micros:reservation};
      const hold = await rpc('ai_reserve',reserveArgs).catch(async error => {
        if (error.status === 429) await notifyBudget();
        throw error;
      });
      requireValue(hold?.requestId === input.requestId && typeof hold.replay === 'boolean','reservation_unconfirmed',503);
      if (hold.replay) return json({code:'request_replayed',requestId:input.requestId,status:hold.status},409);
      requireValue(hold.status === 'reserved','reservation_unconfirmed',503);
      await notifyBudget();
      const settle = async (status, usage, id) => {
        const ack = await rpc('ai_settle',{...identity,p_status:status,
          p_actual_cost_micros:usage ? Math.ceil(usage.costUsd*1e6) : null,
          p_input_tokens:usage?.inputTokens ?? null,p_output_tokens:usage?.outputTokens ?? null,
          p_generation_id:usage?.id ?? (generationId(id) ? id : null)});
        requireValue(ack?.requestId === input.requestId && ack.status === status,'settlement_unconfirmed',503);
        if (status === 'completed') await notifyBudget();
      };
      const recordFailure = async id => {
        // Hold the upper bound durably before looking up a possibly delayed receipt.
        await settle('unknown',null,id).catch(() => {});
        if (!generationId(id)) return;
        try {
          const receipt = await fetcher(ROUTER+'/generation?id='+id,{headers:{authorization:'Bearer '+routerKey},
            redirect:'error',signal:AbortSignal.timeout(4000)});
          if (!receipt.ok) { await receipt.body?.cancel(); return; }
          const {data} = await readJson(receipt,65536,502);
          if (data?.id !== id || data.model !== model.id || !(data.finish_reason || data.cancelled === true)) return;
          const usage = metering({cost:data.total_cost,prompt_tokens:data.native_tokens_prompt,
            completion_tokens:data.native_tokens_completion},id);
          await settle('completed',usage);
        } catch { /* The unknown reservation survives a delayed or unverifiable receipt. */ }
      };
      if (request.signal.aborted) { await settle('not_sent'); throw new ApiError(409,'cancelled_before_dispatch'); }
      const admitted = await rpc('ai_dispatch',identity);
      requireValue(admitted?.requestId === input.requestId && typeof admitted.dispatch === 'boolean','dispatch_unconfirmed',503);
      if (admitted.dispatch !== true) return json({code:'dispatch_refused',requestId:input.requestId,status:admitted.status},409);
      requireValue(admitted.status === 'dispatched','dispatch_unconfirmed',503);
      const controller = new AbortController();
      const abort = () => controller.abort();
      request.signal.addEventListener('abort',abort,{once:true});
      if (request.signal.aborted) abort();
      const timeout = setTimeout(abort,65000);
      const cleanup = () => { clearTimeout(timeout); request.signal.removeEventListener('abort',abort); };
      let upstreamId;
      try {
        const response = await fetcher(ROUTER+'/chat/completions',{method:'POST',
          headers:{authorization:'Bearer '+routerKey,'content-type':'application/json','X-OpenRouter-Cache':'false'},
          redirect:'error',signal:controller.signal,body:JSON.stringify(body)});
        upstreamId = response.headers.get('x-generation-id');
        if (!response.ok) {
          await response.body?.cancel();
          throw new ApiError(502,'provider_refused');
        }
        if (action === 'chat') {
          requireValue(response.body && response.headers.get('content-type')?.includes('text/event-stream'),'provider_stream_invalid',502);
          headers.set('content-type','text/event-stream; charset=utf-8');
          return new Response(chatStream(response,input.requestId,model.id,controller,settle,recordFailure,cleanup),{headers});
        }
        const answer = await readJson(response,512*1024,502);
        requireValue(answer.model === model.id,'provider_model_mismatch',502);
        upstreamId = answer.id ?? upstreamId;
        const usage = metering(answer.usage,answer.id);
        const choice = answer.choices?.[0];
        // Keep cost even when a complete report cannot be produced.
        await settle('completed',usage);
        cleanup();
        if (choice?.finish_reason !== 'stop' || typeof choice.message?.content !== 'string' || !choice.message.content.trim()) {
          return json({code:'incomplete_report',requestId:input.requestId,usage},422);
        }
        return json({requestId:input.requestId,model:model.id,text:choice.message.content,usage,status:'completed'});
      } catch (error) {
        cleanup();
        await recordFailure(upstreamId);
        throw error;
      }
    } catch (error) {
      return json({code:error instanceof ApiError ? error.code : 'service_unavailable'}, error instanceof ApiError ? error.status : 503);
    }
  };
}
