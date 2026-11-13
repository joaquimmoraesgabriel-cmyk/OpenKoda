-- This schema is private. Edge functions verify the JWT and supply p_user.
-- Production admission and payment processing are disabled at installation.
create schema openkoda_private;
revoke all on schema openkoda_private from public, anon, authenticated;
grant usage on schema openkoda_private to service_role;
set local search_path = openkoda_private, pg_catalog;

create table openkoda_private.controls (
  singleton boolean primary key default true check(singleton),
  inference_enabled boolean not null default false,
  daily_limit bigint not null default 0 check(daily_limit >= 0),
  monthly_limit bigint not null default 0 check(monthly_limit >= 0),
  max_parallel integer not null default 10 check(max_parallel between 1 and 100),
  user_parallel integer not null default 1 check(user_parallel between 1 and 10),
  user_per_minute integer not null default 8 check(user_per_minute between 1 and 1000),
  ip_per_minute integer not null default 24 check(ip_per_minute between 1 and 5000),
  terms_version text not null default '2026-11-13',
  privacy_version text not null default '2026-11-13'
);
insert into openkoda_private.controls(singleton) values(true);
create table openkoda_private.plans (
  id text primary key, price_brl_cents integer not null check(price_brl_cents > 0),
  monthly_limit bigint not null check(monthly_limit > 0)
);
insert into openkoda_private.plans values ('plus',21900,20000000),('pro',99900,100000000),('ultra',199900,200000000);
create table openkoda_private.model_access (
  plan text references openkoda_private.plans, model text, operation text check(operation in ('chat','analysis')),
  primary key(plan,model,operation)
);
insert into openkoda_private.model_access
  select id, model, op from openkoda_private.plans cross join
    (values('google/gemini-2.5-flash-lite'),('mistralai/ministral-8b-2512'),('openai/gpt-5-nano'),('@verified')) m(model)
    cross join (values('chat'),('analysis')) o(op);
create table openkoda_private.accounts (
  user_id uuid primary key references auth.users(id) on delete cascade,
  terms_version text, privacy_version text, accepted_at timestamptz,
  marketing boolean not null default false, marketing_updated_at timestamptz, deletion_pending boolean not null default false
);
create table openkoda_private.consent_events (
  id bigint generated always as identity primary key, user_id uuid not null references auth.users(id) on delete cascade,
  terms_version text, privacy_version text, marketing boolean not null, recorded_at timestamptz not null default now()
);
create index on openkoda_private.consent_events(user_id,id);
-- Only an independently verified entitlement can be provisioned here; mock checkout never writes it.
create table openkoda_private.subscriptions (
  user_id uuid primary key references auth.users(id) on delete cascade,
  plan text not null references openkoda_private.plans(id), period_start timestamptz not null, period_end timestamptz not null,
  cancel_at_period_end boolean not null default false,
  check(period_end > period_start and period_end - period_start between interval '27 days' and interval '32 days')
);
create table openkoda_private.checkouts (
  user_id uuid not null references auth.users(id) on delete cascade, request_id uuid not null,
  plan text not null references openkoda_private.plans(id), created_at timestamptz not null default now(),
  primary key(user_id,request_id)
);
create index on openkoda_private.subscriptions(plan);
create index on openkoda_private.checkouts(plan);
create table openkoda_private.requests (
  id uuid primary key, user_id uuid references auth.users(id) on delete set null,
  digest text, ip_hash text, operation text not null check(operation in ('chat','analysis')), model text not null,
  reserved bigint not null check(reserved > 0), actual bigint check(actual >= 0),
  status text not null check(status in ('reserved','dispatched','not_sent','unknown','completed')),
  created_at timestamptz not null, updated_at timestamptz not null, expires_at timestamptz not null,
  input_tokens bigint check(input_tokens >= 0), output_tokens bigint check(output_tokens >= 0),
  generation_id text, liability bigint generated always as
    (case when status='not_sent' then 0 when status='completed' then actual else reserved end) stored,
  check((status='completed')=(actual is not null))
);
create unique index on openkoda_private.requests(generation_id) where generation_id is not null;
create index on openkoda_private.requests(user_id,created_at);
create index on openkoda_private.requests(ip_hash,created_at) where ip_hash is not null;
create index on openkoda_private.requests(created_at);
create index on openkoda_private.requests(expires_at) where status in ('reserved','dispatched');
create table openkoda_private.tickets (
  id uuid primary key default gen_random_uuid(), user_id uuid not null references auth.users(id) on delete cascade,
  request_id uuid not null, subject text not null check(length(subject) between 1 and 160),
  message text not null check(length(message) between 1 and 4000), created_at timestamptz not null default now(),
  closed_at timestamptz, unique(user_id,request_id)
);
create index on openkoda_private.tickets(user_id,created_at);
create table openkoda_private.replies (
  id uuid primary key default gen_random_uuid(), ticket_id uuid not null references openkoda_private.tickets on delete cascade,
  message text not null check(length(message) between 1 and 4000), created_at timestamptz not null default now()
);
create index on openkoda_private.replies(ticket_id,created_at);
create table openkoda_private.skills (
  id text primary key, name text not null, description text not null, revision text not null check(revision ~ '^[0-9a-f]{40}$'),
  invocation text not null check(invocation in ('explicit','automatic_or_explicit')), body text not null,
  resources jsonb not null check(jsonb_typeof(resources)='object'), license text not null, source text not null
);

-- Read only the identity the Edge already verified. Never grant a general auth.users listing.
create function openkoda_private.member(p_user uuid) returns text language plpgsql security definer
set search_path = '' as $$
declare verified_email text;
begin
  select u.email into verified_email from auth.users u where u.id=p_user and u.email_confirmed_at is not null for share;
  if verified_email is null or exists(select from openkoda_private.accounts a where a.user_id=p_user and a.deletion_pending)
    then raise exception 'account_unavailable'; end if;
  return verified_email;
end $$;

-- The Edge verifies each catalogue entry's live price, capabilities and ZDR
-- endpoint before calling these service-only RPCs. Remove @verified for a closed list.
create function openkoda_private.model_allowed(p_plan text,p_model text,p_operation text) returns boolean
language sql stable security invoker set search_path='' as $$
  select p_model ~ '^[a-z0-9][a-z0-9._-]{0,80}/[a-z0-9][a-z0-9._-]{0,120}$' and p_model not like 'openrouter/%'
    and exists(select from openkoda_private.model_access where plan=p_plan and operation=p_operation
      and model in (p_model,'@verified'))
$$;

create function openkoda_private.period(p_user uuid,p_at timestamptz)
returns table(plan_id text, month_start timestamptz, month_end timestamptz, week_start timestamptz,
              week_end timestamptz, month_limit bigint, week_limit bigint, cancel_at_end boolean)
language sql stable security invoker set search_path='' as $$
  with s as (select sub.*,p.monthly_limit,extract(epoch from sub.period_end-sub.period_start) as secs,
    floor(extract(epoch from p_at-sub.period_start)/604800)::int as block
    from openkoda_private.subscriptions sub join openkoda_private.plans p on p.id=sub.plan
    where sub.user_id=p_user and p_at >= sub.period_start and p_at < sub.period_end)
  select plan,period_start,period_end,period_start+block*interval '168 hours',
    least(period_end,period_start+(block+1)*interval '168 hours'),monthly_limit,
    (floor(monthly_limit*least(secs,604800*(block+1))/secs)-floor(monthly_limit*(604800*block)/secs))::bigint,
    cancel_at_period_end from s
$$;

create function public.openkoda_ai_reserve(p_user uuid,p_request uuid,p_digest text,p_model text,p_operation text,p_ip_hash text,p_max_cost_micros bigint)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare c openkoda_private.controls; prior openkoda_private.requests; w record; t timestamptz;
  day_start timestamptz; month_start timestamptz; used_month numeric; used_week numeric;
begin
  if p_request is null or p_digest is null or p_digest !~ '^[0-9a-f]{64}$' or p_ip_hash is null
     or p_ip_hash !~ '^[0-9a-f]{64}$' or p_max_cost_micros is null or p_max_cost_micros not between 1 and 10000
     or p_model is null or p_operation is null then raise exception 'request_invalid'; end if;
  select * into strict c from openkoda_private.controls where singleton for update;
  perform openkoda_private.member(p_user);
  t:=clock_timestamp();
  update openkoda_private.requests set status=case when status='reserved' then 'not_sent' else 'unknown' end,updated_at=t
    where status in ('reserved','dispatched') and expires_at<=t;
  select * into prior from openkoda_private.requests where id=p_request;
  if found then
    if prior.user_id is distinct from p_user or prior.digest is distinct from p_digest or prior.model<>p_model
      or prior.operation<>p_operation or prior.reserved<>p_max_cost_micros then raise exception 'idempotency_conflict'; end if;
    return jsonb_build_object('requestId',p_request,'status',prior.status,'replay',true);
  end if;
  if not c.inference_enabled then raise exception 'inference_disabled'; end if;
  if not exists(select from openkoda_private.accounts where user_id=p_user and terms_version=c.terms_version and privacy_version=c.privacy_version)
    then raise exception 'consent_required'; end if;
  select * into w from openkoda_private.period(p_user,t);
  if not found then raise exception 'subscription_required'; end if;
  if not openkoda_private.model_allowed(w.plan_id,p_model,p_operation)
    then raise exception 'model_not_allowed'; end if;
  if (select count(*) from openkoda_private.requests where status in ('reserved','dispatched'))>=c.max_parallel
    or (select count(*) from openkoda_private.requests where user_id=p_user and status in ('reserved','dispatched'))>=c.user_parallel
    then raise exception 'concurrency_limited'; end if;
  if (select count(*) from openkoda_private.requests where user_id=p_user and created_at>=t-interval '1 minute')>=c.user_per_minute
    or (select count(*) from openkoda_private.requests where ip_hash=p_ip_hash and created_at>=t-interval '1 minute')>=c.ip_per_minute
    then raise exception 'rate_limited'; end if;
  select coalesce(sum(liability),0),coalesce(sum(liability) filter(where created_at>=w.week_start),0)
    into used_month,used_week from openkoda_private.requests where user_id=p_user and created_at>=w.month_start and created_at<w.month_end;
  if used_month+p_max_cost_micros>w.month_limit or used_week+p_max_cost_micros>w.week_limit then raise exception 'quota_exceeded'; end if;
  day_start:=date_trunc('day',t,'UTC'); month_start:=date_trunc('month',t,'UTC');
  if (select coalesce(sum(liability),0) from openkoda_private.requests where created_at>=day_start)+p_max_cost_micros>c.daily_limit
    or (select coalesce(sum(liability),0) from openkoda_private.requests where created_at>=month_start)+p_max_cost_micros>c.monthly_limit
    then raise exception 'platform_quota_exceeded'; end if;
  insert into openkoda_private.requests(id,user_id,digest,ip_hash,operation,model,reserved,status,created_at,updated_at,expires_at)
    values(p_request,p_user,p_digest,p_ip_hash,p_operation,p_model,p_max_cost_micros,'reserved',t,t,t+interval '60 seconds');
  return jsonb_build_object('requestId',p_request,'status','reserved','replay',false);
end $$;

create function public.openkoda_ai_dispatch(p_user uuid,p_request uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare r openkoda_private.requests; c openkoda_private.controls; t timestamptz; can_send boolean;
begin
  select * into strict c from openkoda_private.controls where singleton for update;
  perform openkoda_private.member(p_user);
  select * into strict r from openkoda_private.requests where id=p_request and user_id=p_user for update;
  t:=clock_timestamp();
  can_send:=r.status='reserved' and r.expires_at>t and c.inference_enabled
    and exists(select from openkoda_private.period(p_user,t) w where r.created_at>=w.week_start
      and openkoda_private.model_allowed(w.plan_id,r.model,r.operation))
    and date_trunc('day',r.created_at,'UTC')=date_trunc('day',t,'UTC')
    and (select coalesce(sum(liability),0) from openkoda_private.requests where created_at>=date_trunc('day',t,'UTC'))<=c.daily_limit
    and (select coalesce(sum(liability),0) from openkoda_private.requests where created_at>=date_trunc('month',t,'UTC'))<=c.monthly_limit
    and exists(select from openkoda_private.accounts where user_id=p_user and terms_version=c.terms_version and privacy_version=c.privacy_version);
  if r.status='reserved' then
    update openkoda_private.requests set status=case when can_send then 'dispatched' else 'not_sent' end,
      updated_at=t,expires_at=t+interval '5 minutes' where id=p_request returning * into r;
  end if;
  return jsonb_build_object('requestId',r.id,'status',r.status,'dispatch',can_send);
end $$;

create function public.openkoda_ai_settle(p_user uuid,p_request uuid,p_status text,p_actual_cost_micros bigint,
  p_input_tokens bigint,p_output_tokens bigint,p_generation_id text) returns jsonb language plpgsql security invoker set search_path='' as $$
declare r openkoda_private.requests;
begin
  perform 1 from openkoda_private.controls where singleton for update;
  -- A close/delete can detach ownership after dispatch. The service still owns
  -- the original immutable request ID and must be able to record its remote bill.
  select * into strict r from openkoda_private.requests where id=p_request
    and (user_id=p_user or user_id is null and status in ('unknown','completed')) for update;
  if p_status is null or p_status not in ('completed','unknown','not_sent')
    or (p_status='completed')<>(p_actual_cost_micros is not null)
    or p_actual_cost_micros<0 or p_input_tokens<0 or p_output_tokens<0
    or (p_generation_id is not null and p_generation_id !~ '^gen-[a-zA-Z0-9-]{1,124}$')
    or (p_status='completed' and p_generation_id is null)
    or (p_status='not_sent' and (p_input_tokens is not null or p_output_tokens is not null or p_generation_id is not null))
    then raise exception 'settlement_invalid'; end if;
  if r.status='unknown' and p_status='unknown' and r.generation_id is null and p_generation_id is not null then
    update openkoda_private.requests set generation_id=p_generation_id,input_tokens=p_input_tokens,
      output_tokens=p_output_tokens,updated_at=clock_timestamp() where id=p_request;
  elsif r.status=p_status then
    if r.actual is distinct from p_actual_cost_micros or r.generation_id is distinct from p_generation_id
      or r.input_tokens is distinct from p_input_tokens or r.output_tokens is distinct from p_output_tokens
      then raise exception 'settlement_conflict'; end if;
  else
    if not ((r.status='reserved' and p_status='not_sent') or (r.status='dispatched' and p_status in ('unknown','completed'))
        or (r.status='unknown' and p_status='completed')) or r.generation_id is not null and r.generation_id is distinct from p_generation_id
      then raise exception 'settlement_conflict'; end if;
    update openkoda_private.requests set status=p_status,actual=p_actual_cost_micros,generation_id=p_generation_id,
      input_tokens=p_input_tokens,output_tokens=p_output_tokens,updated_at=clock_timestamp() where id=p_request;
    if p_actual_cost_micros > r.reserved then update openkoda_private.controls set inference_enabled=false where singleton; end if;
  end if;
  return jsonb_build_object('requestId',r.id,'status',p_status);
end $$;

create function public.openkoda_account(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare c openkoda_private.controls; a openkoda_private.accounts; w record; t timestamptz:=now();
  plan jsonb; sub jsonb; usage jsonb; catalogue jsonb;
begin
  perform openkoda_private.member(p_user);
  select * into strict c from openkoda_private.controls where singleton;
  select * into a from openkoda_private.accounts where user_id=p_user;
  select jsonb_agg(jsonb_build_object('id',id,'priceBrlCents',price_brl_cents,'monthlyLimitMicros',monthly_limit) order by monthly_limit)
    into catalogue from openkoda_private.plans;
  select * into w from openkoda_private.period(p_user,t);
  if found then
    select v into plan from jsonb_array_elements(catalogue) v where v->>'id'=w.plan_id;
    sub:=jsonb_build_object('status','active','periodStart',w.month_start,'periodEnd',w.month_end,'cancelAtPeriodEnd',w.cancel_at_end);
    select jsonb_build_object('monthlyLimitMicros',w.month_limit,'monthlyUsedMicros',coalesce(sum(liability),0),
      'windowLimitMicros',w.week_limit,'windowUsedMicros',coalesce(sum(liability) filter(where created_at>=w.week_start),0),
      'windowStart',w.week_start,'windowEnd',w.week_end,'nextRelease',least(w.month_end,w.week_end),
      'reservedMicros',coalesce(sum(reserved) filter(where status in ('reserved','dispatched','unknown')),0),
      'settledMicros',coalesce(sum(actual),0)) into usage from openkoda_private.requests
      where user_id=p_user and created_at>=w.month_start and created_at<w.month_end;
  end if;
  return jsonb_build_object('plan',plan,'plans',catalogue,'subscription',sub,'usage',usage,
    'consent',jsonb_build_object('termsVersion',a.terms_version,'privacyVersion',a.privacy_version,'acceptedAt',a.accepted_at,
      'marketing',coalesce(a.marketing,false),'requiredTermsVersion',c.terms_version,'requiredPrivacyVersion',c.privacy_version),
    'controls',jsonb_build_object('inferenceEnabled',c.inference_enabled,'paymentsEnabled',false,'mockCheckout',true));
end $$;

create function public.openkoda_consent(p_user uuid,p_terms_version text,p_privacy_version text,p_marketing boolean)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare c openkoda_private.controls;
begin
  perform openkoda_private.member(p_user);
  select * into strict c from openkoda_private.controls where singleton for share;
  if p_terms_version is distinct from c.terms_version or p_privacy_version is distinct from c.privacy_version
    or p_marketing is null then raise exception 'consent_version_invalid'; end if;
  insert into openkoda_private.accounts(user_id,terms_version,privacy_version,accepted_at,marketing,marketing_updated_at)
    values(p_user,p_terms_version,p_privacy_version,now(),p_marketing,now())
    on conflict(user_id) do update set terms_version=excluded.terms_version,privacy_version=excluded.privacy_version,
      accepted_at=now(),marketing=excluded.marketing,marketing_updated_at=now()
    where not openkoda_private.accounts.deletion_pending and
      (openkoda_private.accounts.terms_version,openkoda_private.accounts.privacy_version,openkoda_private.accounts.marketing)
      is distinct from (excluded.terms_version,excluded.privacy_version,excluded.marketing);
  if found then insert into openkoda_private.consent_events(user_id,terms_version,privacy_version,marketing)
    values(p_user,p_terms_version,p_privacy_version,p_marketing); end if;
  return jsonb_build_object('accepted',true);
end $$;
create function public.openkoda_marketing(p_user uuid,p_enabled boolean) returns jsonb language plpgsql security invoker set search_path='' as $$
declare a openkoda_private.accounts;
begin
  perform openkoda_private.member(p_user);
  if p_enabled is null then raise exception 'preference_invalid'; end if;
  select * into a from openkoda_private.accounts where user_id=p_user for update;
  if not found then raise exception 'consent_required'; end if;
  if a.marketing<>p_enabled then
    update openkoda_private.accounts set marketing=p_enabled,marketing_updated_at=now() where user_id=p_user;
    insert into openkoda_private.consent_events(user_id,terms_version,privacy_version,marketing)
      values(p_user,a.terms_version,a.privacy_version,p_enabled);
  end if;
  return jsonb_build_object('enabled',p_enabled);
end $$;

create function public.openkoda_checkout_mock(p_user uuid,p_plan text,p_request uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare chosen text;
begin
  perform openkoda_private.member(p_user);
  if not exists(select from openkoda_private.plans where id=p_plan) or p_request is null then raise exception 'checkout_invalid'; end if;
  insert into openkoda_private.checkouts(user_id,request_id,plan) values(p_user,p_request,p_plan) on conflict do nothing;
  select plan into strict chosen from openkoda_private.checkouts where user_id=p_user and request_id=p_request;
  if chosen<>p_plan then raise exception 'idempotency_conflict'; end if;
  return jsonb_build_object('id',p_request,'plan',chosen,'status','simulated','grantsProductionCredit',false);
end $$;
create function public.openkoda_cancel(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform openkoda_private.member(p_user);
  update openkoda_private.subscriptions set cancel_at_period_end=true where user_id=p_user and period_end>now();
  return jsonb_build_object('cancelAtPeriodEnd',found);
end $$;

create function public.openkoda_support_create(p_user uuid,p_subject text,p_message text,p_request uuid)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare ticket openkoda_private.tickets;
begin
  perform openkoda_private.member(p_user);
  if p_request is null or p_subject is null or p_message is null or length(trim(p_subject)) not between 1 and 160
    or length(trim(p_message)) not between 1 and 4000 then raise exception 'ticket_invalid'; end if;
  perform pg_advisory_xact_lock(hashtextextended(p_user::text, 13));
  select * into ticket from openkoda_private.tickets where user_id=p_user and request_id=p_request;
  if found then
    if ticket.subject<>p_subject or ticket.message<>p_message then raise exception 'idempotency_conflict'; end if;
  else
    if (select count(*) from openkoda_private.tickets where user_id=p_user and created_at>now()-interval '1 day')>=10
      then raise exception 'rate_limited'; end if;
    insert into openkoda_private.tickets(user_id,request_id,subject,message)
      values(p_user,p_request,p_subject,p_message) returning * into ticket;
  end if;
  return jsonb_build_object('id',ticket.id);
end $$;
create function public.openkoda_support_list(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform openkoda_private.member(p_user);
  return jsonb_build_object('tickets',coalesce((select jsonb_agg(jsonb_build_object('id',t.id,'subject',t.subject,'message',t.message,
    'status',case when t.closed_at is null then 'open' else 'closed' end,'createdAt',t.created_at,
    'replies',coalesce((select jsonb_agg(jsonb_build_object('message',r.message,'createdAt',r.created_at) order by r.created_at,r.id)
      from openkoda_private.replies r where r.ticket_id=t.id),'[]'::jsonb)) order by t.created_at desc,t.id)
    from openkoda_private.tickets t where t.user_id=p_user),'[]'::jsonb));
end $$;

create function public.openkoda_skills(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform openkoda_private.member(p_user);
  if not exists(select from openkoda_private.period(p_user,now())) then raise exception 'subscription_required'; end if;
  return jsonb_build_object('skills',coalesce((select jsonb_agg(jsonb_build_object('id',id,'name',name,'description',description,
    'revision',revision,'invocation',invocation) order by name) from openkoda_private.skills),'[]'::jsonb));
end $$;
create function public.openkoda_skill(p_user uuid,p_skill text) returns jsonb language plpgsql security invoker set search_path='' as $$
declare payload jsonb;
begin
  perform openkoda_private.member(p_user);
  if not exists(select from openkoda_private.period(p_user,now())) then raise exception 'subscription_required'; end if;
  select to_jsonb(s) into payload from openkoda_private.skills s where id=p_skill;
  if payload is null then raise exception 'skill_not_found'; end if;
  return payload;
end $$;

create function public.openkoda_export(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform openkoda_private.member(p_user);
  return jsonb_build_object('account',public.openkoda_account(p_user),'userId',p_user,
    'email',openkoda_private.member(p_user),'support',public.openkoda_support_list(p_user),
    'consentEvents',coalesce((select jsonb_agg(to_jsonb(e)-'user_id' order by id) from openkoda_private.consent_events e where user_id=p_user),'[]'::jsonb),
    'mockCheckouts',coalesce((select jsonb_agg(to_jsonb(o)-'user_id' order by created_at) from openkoda_private.checkouts o where user_id=p_user),'[]'::jsonb),
    'requests',coalesce((select jsonb_agg(jsonb_build_object('requestId',id,'operation',operation,'model',model,'status',status,
      'createdAt',created_at,'reservedMicros',reserved,'actualMicros',actual,'inputTokens',input_tokens,'outputTokens',output_tokens)
      order by created_at) from openkoda_private.requests where user_id=p_user),'[]'::jsonb));
end $$;
create function openkoda_private.lock_identity_for_delete(p_user uuid) returns boolean language plpgsql security definer set search_path='' as $$
begin
  perform 1 from auth.users where id=p_user for update;
  return found;
end $$;
create function public.openkoda_delete(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform 1 from openkoda_private.controls where singleton for update;
  if not openkoda_private.lock_identity_for_delete(p_user) then return jsonb_build_object('scheduled',true); end if;
  insert into openkoda_private.accounts(user_id,deletion_pending) values(p_user,true)
    on conflict(user_id) do update set deletion_pending=true,terms_version=null,privacy_version=null,marketing=false,accepted_at=null,marketing_updated_at=null;
  delete from openkoda_private.subscriptions where user_id=p_user;
  delete from openkoda_private.tickets where user_id=p_user;
  delete from openkoda_private.checkouts where user_id=p_user;
  delete from openkoda_private.consent_events where user_id=p_user;
  update openkoda_private.requests set user_id=null,digest=null,ip_hash=null,
    status=case status when 'reserved' then 'not_sent' when 'dispatched' then 'unknown' else status end where user_id=p_user;
  return jsonb_build_object('scheduled',true);
end $$;

-- Prune on a schedule, independent of a customer making a new request. Detached
-- financial totals remain for the current platform cap, never containing prompts.
create function openkoda_private.retention() returns void language plpgsql security invoker set search_path='' as $$
declare t timestamptz:=clock_timestamp();
begin
  perform 1 from openkoda_private.controls where singleton for update;
  update openkoda_private.requests set status=case when status='reserved' then 'not_sent' else 'unknown' end,updated_at=t
    where status in ('reserved','dispatched') and expires_at<=t;
  update openkoda_private.requests set ip_hash=null,generation_id=case when status='unknown' then generation_id else null end
    where created_at<t-interval '30 days' and (ip_hash is not null or generation_id is not null and status<>'unknown');
  update openkoda_private.requests set user_id=null,ip_hash=null,digest=null,generation_id=null,input_tokens=null,output_tokens=null
    where created_at<t-interval '90 days' and status in ('completed','not_sent') and user_id is not null;
  delete from openkoda_private.tickets where closed_at<t-interval '30 days';
  delete from openkoda_private.checkouts where created_at<t-interval '90 days';
end $$;

do $rights$
declare obj record;
begin
  for obj in select tablename from pg_tables where schemaname='openkoda_private' loop
    execute format('alter table openkoda_private.%I enable row level security',obj.tablename);
  end loop;
  for obj in select p.oid::regprocedure as signature from pg_proc p join pg_namespace n on n.oid=p.pronamespace
    where n.nspname='openkoda_private' or n.nspname='public' and p.proname in
      ('openkoda_ai_reserve','openkoda_ai_dispatch','openkoda_ai_settle','openkoda_account','openkoda_consent',
        'openkoda_marketing','openkoda_checkout_mock','openkoda_cancel','openkoda_support_create',
        'openkoda_support_list','openkoda_skills','openkoda_skill','openkoda_export','openkoda_delete') loop
    execute format('revoke all on function %s from public,anon,authenticated,service_role',obj.signature);
    execute format('grant execute on function %s to service_role',obj.signature);
  end loop;
end $rights$;
revoke all on all tables in schema openkoda_private from public,anon,authenticated,service_role;
revoke all on all sequences in schema openkoda_private from public,anon,authenticated,service_role;
grant select on all tables in schema openkoda_private to service_role;
grant insert,update on openkoda_private.accounts,openkoda_private.requests to service_role;
grant insert,delete on openkoda_private.consent_events,openkoda_private.checkouts,openkoda_private.tickets to service_role;
grant update (cancel_at_period_end),delete on openkoda_private.subscriptions to service_role;
grant update (inference_enabled) on openkoda_private.controls to service_role;
grant usage on all sequences in schema openkoda_private to service_role;
-- On hosted Supabase, install the scheduler. Plain PostgreSQL can exercise the
-- same retention routine independently without an optional extension.
do $schedule$
begin
  if exists(select from pg_available_extensions where name='pg_cron') then
    create extension if not exists pg_cron;
    perform cron.schedule('openkoda-commercial-retention','17 3 * * *','select openkoda_private.retention()');
  end if;
end $schedule$;
