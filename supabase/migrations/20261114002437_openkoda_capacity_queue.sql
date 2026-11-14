-- Only an authenticated client with the original request may advance a place.
-- No prompts or attachments are stored or sent by a background job.
create table openkoda_private.capacity_waiters (
  request_id uuid primary key, user_id uuid not null unique references auth.users(id) on delete cascade,
  token uuid not null default gen_random_uuid(), sequence bigint generated always as identity unique,
  digest text not null, model text not null, operation text not null, cost_bound bigint not null,
  ip_hash text not null, expires_at timestamptz not null, lease_until timestamptz not null
);
alter table openkoda_private.capacity_waiters enable row level security;
revoke all on openkoda_private.capacity_waiters from public,anon,authenticated,service_role;
grant select,insert,delete,update (lease_until) on openkoda_private.capacity_waiters to service_role;
revoke all on sequence openkoda_private.capacity_waiters_sequence_seq from public,anon,authenticated,service_role;
grant usage on sequence openkoda_private.capacity_waiters_sequence_seq to service_role;

create function public.openkoda_ai_admit(p_user uuid,p_request uuid,p_digest text,p_model text,p_operation text,p_ip_hash text,p_max_cost_micros bigint,p_queue_token uuid)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare c openkoda_private.controls; prior openkoda_private.requests; w record; t timestamptz;
  day_start timestamptz; month_start timestamptz; used_month numeric; used_week numeric;
  waiter openkoda_private.capacity_waiters; available int; mine int; ahead int;
begin
  if p_request is null or p_digest is null or p_digest !~ '^[0-9a-f]{64}$' or p_ip_hash is null
     or p_ip_hash !~ '^[0-9a-f]{64}$' or p_max_cost_micros is null or p_max_cost_micros not between 1 and 10000
     or p_model is null or p_operation is null then raise exception 'request_invalid'; end if;
  select * into strict c from openkoda_private.controls where singleton for update;
  perform openkoda_private.member(p_user);
  t:=clock_timestamp();
  delete from openkoda_private.capacity_waiters where lease_until<=t;
  update openkoda_private.requests set status=case when status='reserved' then 'not_sent' else 'unknown' end,updated_at=t
    where status in ('reserved','dispatched') and expires_at<=t;
  select * into prior from openkoda_private.requests where id=p_request;
  if found then
    if prior.user_id is distinct from p_user or prior.digest is distinct from p_digest or prior.model<>p_model
      or prior.operation<>p_operation or prior.reserved<>p_max_cost_micros then raise exception 'idempotency_conflict'; end if;
    return jsonb_build_object('requestId',p_request,'status',prior.status,'replay',true);
  end if;
  if not c.inference_enabled then raise exception 'inference_disabled'; end if;
  select * into waiter from openkoda_private.capacity_waiters where request_id=p_request;
  if waiter.request_id is not null then
    if waiter.user_id<>p_user or waiter.digest<>p_digest or waiter.model<>p_model
      or waiter.operation<>p_operation or waiter.cost_bound<>p_max_cost_micros then raise exception 'idempotency_conflict'; end if;
    if p_queue_token is not null and p_queue_token<>waiter.token then raise exception 'queue_expired'; end if;
  elsif p_queue_token is not null then raise exception 'queue_expired';
  end if;
  if not exists(select from openkoda_private.accounts where user_id=p_user and terms_version=c.terms_version and privacy_version=c.privacy_version)
    then raise exception 'consent_required'; end if;
  select * into w from openkoda_private.period(p_user,t);
  if not found then raise exception 'subscription_required'; end if;
  if not openkoda_private.model_allowed(w.plan_id,p_model,p_operation)
    then raise exception 'model_not_allowed'; end if;
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
  select c.max_parallel-count(*),count(*) filter(where user_id=p_user) into available,mine
    from openkoda_private.requests where status in ('reserved','dispatched');
  -- Hold each free slot for one eligible older waiter. A busy user never
  -- blocks unrelated accounts. A disconnected head holds its place for 15s.
  select count(*) into ahead from openkoda_private.capacity_waiters q
    where (waiter.sequence is null or q.sequence<waiter.sequence)
      and (select count(*) from openkoda_private.requests r where r.user_id=q.user_id
        and r.status in ('reserved','dispatched'))<c.user_parallel;
  if available<=ahead or mine>=c.user_parallel or (waiter.request_id is not null and p_queue_token is null) then
    if waiter.request_id is null then
      if exists(select from openkoda_private.capacity_waiters where user_id=p_user)
        then raise exception 'queue_user_limit'; end if;
      if (select count(*) from openkoda_private.capacity_waiters)>=64 or
         (select count(*) from openkoda_private.capacity_waiters where ip_hash=p_ip_hash)>=c.ip_per_minute
        then raise exception 'queue_full'; end if;
      insert into openkoda_private.capacity_waiters(request_id,user_id,digest,model,operation,cost_bound,ip_hash,expires_at,lease_until)
        values(p_request,p_user,p_digest,p_model,p_operation,p_max_cost_micros,p_ip_hash,t+interval '120 seconds',t+interval '15 seconds')
        returning * into waiter;
    elsif p_queue_token=waiter.token then
      update openkoda_private.capacity_waiters set lease_until=least(expires_at,t+interval '15 seconds') where request_id=p_request;
    end if;
    return jsonb_build_object('requestId',p_request,'status','waiting','queueToken',waiter.token,
      'position',ahead+1,'retryAfterSeconds',2,'expiresAt',waiter.expires_at,'replay',false);
  end if;
  if waiter.request_id is not null then delete from openkoda_private.capacity_waiters where request_id=p_request; end if;
  insert into openkoda_private.requests(id,user_id,digest,ip_hash,operation,model,reserved,status,created_at,updated_at,expires_at)
    values(p_request,p_user,p_digest,p_ip_hash,p_operation,p_model,p_max_cost_micros,'reserved',t,t,t+interval '60 seconds');
  return jsonb_build_object('requestId',p_request,'status','reserved','replay',false);
end $$;

-- Extend the existing daily retention job without creating a second scheduler.
alter function openkoda_private.retention() rename to retain_commercial_data;
create function openkoda_private.retention() returns void language plpgsql security invoker set search_path='' as $$
begin
  perform openkoda_private.retain_commercial_data();
  delete from openkoda_private.capacity_waiters where lease_until<=clock_timestamp();
end $$;
revoke all on function openkoda_private.retention() from public,anon,authenticated,service_role;
grant execute on function openkoda_private.retention() to service_role;

create function public.openkoda_cancel_wait(p_user uuid,p_request uuid,p_queue_token uuid)
returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform 1 from openkoda_private.controls where singleton for update;
  perform openkoda_private.member(p_user);
  delete from openkoda_private.capacity_waiters where request_id=p_request and user_id=p_user and token=p_queue_token;
  return jsonb_build_object('requestId',p_request,'cancelled',found);
end $$;
revoke all on function public.openkoda_ai_admit(uuid,uuid,text,text,text,text,bigint,uuid),
  public.openkoda_cancel_wait(uuid,uuid,uuid) from public,anon,authenticated,service_role;
grant execute on function public.openkoda_ai_admit(uuid,uuid,text,text,text,text,bigint,uuid),
  public.openkoda_cancel_wait(uuid,uuid,uuid) to service_role;

-- Invalidate outstanding places even if the operator rapidly re-enables AI.
create function openkoda_private.clear_capacity_queue() returns trigger language plpgsql security invoker set search_path='' as $$
begin
  delete from openkoda_private.capacity_waiters;
  return null;
end $$;
revoke all on function openkoda_private.clear_capacity_queue() from public,anon,authenticated,service_role;
create trigger openkoda_clear_capacity_queue after update of inference_enabled on openkoda_private.controls
  for each row when (old.inference_enabled and not new.inference_enabled) execute function openkoda_private.clear_capacity_queue();

-- Existing service integrations retain their fail-fast RPC, honoring the same queue.
create or replace function public.openkoda_ai_reserve(p_user uuid,p_request uuid,p_digest text,p_model text,p_operation text,p_ip_hash text,p_max_cost_micros bigint)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare r jsonb;
begin
  r:=public.openkoda_ai_admit(p_user,p_request,p_digest,p_model,p_operation,p_ip_hash,p_max_cost_micros,null);
  if r->>'status'='waiting' then raise exception 'concurrency_limited'; end if;
  return r;
end $$;
