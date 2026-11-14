-- Internal entitlements are deliberately separate from financial subscriptions.
-- Only database administrators can enroll testers or enable this feature.
alter table openkoda_private.controls add column simulation_enabled boolean not null default false;
create table openkoda_private.simulation_testers (
  user_id uuid primary key references auth.users(id) on delete cascade,
  expires_at timestamptz not null
);
create table openkoda_private.simulations (
  user_id uuid primary key references auth.users(id) on delete cascade,
  plan text not null references openkoda_private.plans(id),
  status text not null check(status in ('active','canceled','expired')),
  period_start timestamptz not null, period_end timestamptz not null,
  check(period_end>period_start and period_end-period_start<=interval '7 days')
);
create table openkoda_private.simulation_events (
  user_id uuid not null references auth.users(id) on delete cascade, request_id uuid not null,
  plan text not null references openkoda_private.plans(id),
  outcome text not null check(outcome in ('activate','cancel','expire','fail')),
  result jsonb not null, created_at timestamptz not null default now(),
  primary key(user_id,request_id)
);
alter table openkoda_private.simulation_testers enable row level security;
alter table openkoda_private.simulations enable row level security;
alter table openkoda_private.simulation_events enable row level security;
revoke all on openkoda_private.simulation_testers,openkoda_private.simulations,openkoda_private.simulation_events from public,anon,authenticated;
grant select on openkoda_private.simulation_testers to service_role;
grant select,insert,update,delete on openkoda_private.simulations,openkoda_private.simulation_events to service_role;

create function openkoda_private.simulation_allowed(p_user uuid) returns boolean
language sql stable security invoker set search_path='' as $$
  select exists(select from openkoda_private.simulation_testers t cross join openkoda_private.controls c
    where t.user_id=p_user and t.expires_at>now() and c.singleton and c.simulation_enabled)
$$;
create function openkoda_private.simulation_view(p_user uuid) returns jsonb
language sql stable security invoker set search_path='' as $$
  select jsonb_build_object('plan',s.plan,'status',case when s.status='active' and s.period_end<=now() then 'expired' else s.status end,
    'periodStart',s.period_start,'periodEnd',s.period_end,'mode','internal')
  from openkoda_private.simulations s where s.user_id=p_user and openkoda_private.simulation_allowed(p_user)
$$;
create function openkoda_private.simulation_active(p_user uuid) returns boolean
language sql stable security invoker set search_path='' as $$
  select openkoda_private.simulation_allowed(p_user) and exists(select from openkoda_private.simulations s
    where s.user_id=p_user and s.status='active' and now()>=s.period_start and now()<s.period_end)
$$;

create function public.openkoda_simulation(p_user uuid,p_plan text,p_request uuid,p_outcome text) returns jsonb
language plpgsql security invoker set search_path='' as $$
declare previous openkoda_private.simulation_events; result jsonb; t timestamptz:=now();
begin
  perform openkoda_private.member(p_user);
  if not openkoda_private.simulation_allowed(p_user) then raise exception 'simulation_not_allowed'; end if;
  if p_request is null or p_outcome is null or p_outcome not in ('activate','cancel','expire','fail') or
    not exists(select from openkoda_private.plans where id=p_plan) then raise exception 'simulation_invalid'; end if;
  perform pg_advisory_xact_lock(hashtextextended(p_user::text,42));
  select * into previous from openkoda_private.simulation_events where user_id=p_user and request_id=p_request;
  if found then
    if previous.plan<>p_plan or previous.outcome<>p_outcome then raise exception 'idempotency_conflict'; end if;
    return previous.result||jsonb_build_object('replay',true);
  end if;
  if (select count(*) from openkoda_private.simulation_events where user_id=p_user and created_at>t-interval '1 day')>=30
    then raise exception 'rate_limited'; end if;
  if p_outcome='activate' then
    insert into openkoda_private.simulations(user_id,plan,status,period_start,period_end)
      values(p_user,p_plan,'active',t,t+interval '7 days')
      on conflict(user_id) do update set plan=excluded.plan,status=excluded.status,
        period_start=excluded.period_start,period_end=excluded.period_end;
  elsif p_outcome in ('cancel','expire') then
    update openkoda_private.simulations set status=case p_outcome when 'cancel' then 'canceled' else 'expired' end
      where user_id=p_user and plan=p_plan;
    if not found then raise exception 'simulation_missing'; end if;
  end if;
  result:=jsonb_build_object('status',case p_outcome when 'fail' then 'failed' when 'cancel' then 'canceled' when 'expire' then 'expired' else 'active' end,
    'simulation',openkoda_private.simulation_view(p_user),'grantsProductionCredit',false,'replay',false);
  insert into openkoda_private.simulation_events(user_id,request_id,plan,outcome,result) values(p_user,p_request,p_plan,p_outcome,result);
  delete from openkoda_private.simulation_events where user_id=p_user and created_at<t-interval '90 days';
  return result;
end $$;

alter function public.openkoda_account(uuid) rename to openkoda_account_financial;
create function public.openkoda_account(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare result jsonb; paid boolean; simulation boolean;
begin
  result:=public.openkoda_account_financial(p_user);
  paid:=exists(select from openkoda_private.period(p_user,now()));
  simulation:=openkoda_private.simulation_active(p_user);
  return result||jsonb_build_object('simulation',openkoda_private.simulation_view(p_user),
    'entitlements',jsonb_build_object('reasoning',paid or simulation,'skills',paid or simulation,
      'hostedInference',paid and coalesce((result->'controls'->>'inferenceEnabled')::boolean,false)),
    'controls',(result->'controls')||jsonb_build_object('mockCheckout',false,'simulationEnabled',openkoda_private.simulation_allowed(p_user)));
end $$;

create or replace function public.openkoda_skills(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
begin
  perform openkoda_private.member(p_user);
  if not exists(select from openkoda_private.period(p_user,now())) and not openkoda_private.simulation_active(p_user) then raise exception 'subscription_required'; end if;
  return jsonb_build_object('skills',coalesce((select jsonb_agg(jsonb_build_object('id',id,'name',name,'description',description,
    'revision',revision,'invocation',invocation) order by name) from openkoda_private.skills),'[]'::jsonb));
end $$;
create or replace function public.openkoda_skill(p_user uuid,p_skill text) returns jsonb language plpgsql security invoker set search_path='' as $$
declare payload jsonb;
begin
  perform openkoda_private.member(p_user);
  if not exists(select from openkoda_private.period(p_user,now())) and not openkoda_private.simulation_active(p_user) then raise exception 'subscription_required'; end if;
  select to_jsonb(s) into payload from openkoda_private.skills s where id=p_skill;
  if payload is null then raise exception 'skill_not_found'; end if;
  return payload;
end $$;

alter function public.openkoda_export(uuid) rename to openkoda_export_without_simulation;
create function public.openkoda_export(p_user uuid) returns jsonb language sql security invoker set search_path='' as $$
  select public.openkoda_export_without_simulation(p_user)||jsonb_build_object('simulation',openkoda_private.simulation_view(p_user),
    'simulationEvents',coalesce((select jsonb_agg(to_jsonb(e)-'user_id' order by created_at) from openkoda_private.simulation_events e where user_id=p_user),'[]'::jsonb))
$$;
revoke all on function openkoda_private.simulation_allowed(uuid),openkoda_private.simulation_view(uuid),openkoda_private.simulation_active(uuid),
  public.openkoda_simulation(uuid,text,uuid,text),public.openkoda_account(uuid),public.openkoda_export(uuid) from public,anon,authenticated;
grant execute on function openkoda_private.simulation_allowed(uuid),openkoda_private.simulation_view(uuid),openkoda_private.simulation_active(uuid),
  public.openkoda_simulation(uuid,text,uuid,text),public.openkoda_account(uuid),public.openkoda_export(uuid) to service_role;
