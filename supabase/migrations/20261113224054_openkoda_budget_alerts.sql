create table openkoda_private.budget_alerts (
  window_start date not null, kind text not null check(kind in ('day','month','daily_spike')),
  threshold int not null check(threshold in (80,95,200)), liability_micros bigint not null,
  reference_micros bigint not null, created_at timestamptz not null default now(),
  primary key(window_start,kind,threshold)
);
alter table openkoda_private.budget_alerts enable row level security;
revoke all on openkoda_private.budget_alerts from public,anon,authenticated,service_role;
grant select,insert on openkoda_private.budget_alerts to service_role;

-- Include held reservations in exposure. Alerts remain unique across retries.
create function openkoda_private.capture_budget_alert() returns trigger language plpgsql security invoker set search_path='' as $$
declare t timestamptz:=clock_timestamp(); day_start timestamptz:=date_trunc('day',t,'UTC');
  month_start timestamptz:=date_trunc('month',t,'UTC'); c openkoda_private.controls;
  day_total bigint; month_total bigint; baseline numeric; observed_days int;
begin
  if tg_op='UPDATE' and new.liability<=old.liability then return null; end if;
  select * into strict c from openkoda_private.controls where singleton;
  select coalesce(sum(liability) filter(where created_at>=day_start),0),coalesce(sum(liability),0)
    into day_total,month_total from openkoda_private.requests where created_at>=month_start;
  insert into openkoda_private.budget_alerts(window_start,kind,threshold,liability_micros,reference_micros)
    select (period_start at time zone 'UTC')::date,kind,threshold,used,cap
      from (values (day_start,'day',day_total,c.daily_limit),(month_start,'month',month_total,c.monthly_limit)) p(period_start,kind,used,cap)
      cross join (values (80),(95)) levels(threshold)
      where cap>0 and used::numeric*100>=cap::numeric*threshold on conflict do nothing;
  select coalesce(sum(liability),0)/7.0,count(distinct (created_at at time zone 'UTC')::date)
    into baseline,observed_days from openkoda_private.requests
    where created_at>=day_start-interval '168 hours' and created_at<day_start and liability>0;
  if observed_days>=3 and baseline>0 and day_total>=2*baseline then
    insert into openkoda_private.budget_alerts values((day_start at time zone 'UTC')::date,'daily_spike',200,
      day_total,ceil(baseline)::bigint,t) on conflict do nothing;
  end if;
  return null;
end $$;
revoke all on function openkoda_private.capture_budget_alert() from public,anon,authenticated,service_role;
create trigger openkoda_budget_alert after insert or update of status,actual,reserved on openkoda_private.requests
  for each row execute function openkoda_private.capture_budget_alert();
create function public.openkoda_budget_health() returns jsonb language sql stable security invoker set search_path='' as $$
  select jsonb_build_object('inferenceEnabled',c.inference_enabled,'dailyLimitMicros',c.daily_limit,'monthlyLimitMicros',c.monthly_limit,
    'alerts',coalesce((select jsonb_agg(to_jsonb(a) order by a.created_at desc) from (select * from openkoda_private.budget_alerts
      order by created_at desc limit 100) a),'[]'::jsonb)) from openkoda_private.controls c where singleton
$$;
revoke all on function public.openkoda_budget_health() from public,anon,authenticated,service_role;
grant execute on function public.openkoda_budget_health() to service_role;
