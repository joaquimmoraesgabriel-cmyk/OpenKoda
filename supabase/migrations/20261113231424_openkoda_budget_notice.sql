alter table openkoda_private.budget_alerts
  add column notice_attempts integer not null default 0,
  add column notice_retry_at timestamptz,
  add column notice_sent_at timestamptz;
grant update (notice_attempts,notice_retry_at,notice_sent_at)
  on openkoda_private.budget_alerts to service_role;

-- A worker claims only aggregate alerts. The retry window stays inside the
-- provider's 24-hour idempotency window, including a crash after delivery.
create function public.openkoda_budget_notice_claim() returns jsonb
language sql volatile security invoker set search_path='' as $$
  with available as (
    select window_start,kind,threshold from openkoda_private.budget_alerts
    where notice_sent_at is null and notice_attempts < 4
      and created_at > now()-interval '22 hours'
      and (notice_retry_at is null or notice_retry_at <= now())
    order by created_at,threshold desc,kind
    limit 5 for update skip locked
  ), claimed as (
    update openkoda_private.budget_alerts a set
      notice_attempts=a.notice_attempts+1, notice_retry_at=now()+interval '2 minutes'
    from available x where (a.window_start,a.kind,a.threshold)=(x.window_start,x.kind,x.threshold)
    returning a.window_start,a.kind,a.threshold,a.notice_attempts
  )
  select jsonb_build_object('alerts',coalesce(jsonb_agg(jsonb_build_object('windowStart',window_start,
    'kind',kind,'threshold',threshold,'attempt',notice_attempts) order by threshold desc,kind),'[]'::jsonb)) from claimed
$$;

create function public.openkoda_budget_notice_finish(
  p_window date,p_kind text,p_threshold integer,p_attempt integer,p_sent boolean
) returns jsonb language sql volatile security invoker set search_path='' as $$
  with updated as (
    update openkoda_private.budget_alerts set
      notice_sent_at=case when p_sent then now() else null end,
      notice_retry_at=case when p_sent then null
        else now()+interval '2 minutes'*power(3,p_attempt-1)::integer end
    where (window_start,kind,threshold)=(p_window,p_kind,p_threshold)
      and notice_attempts=p_attempt and p_attempt between 1 and 4
      and notice_sent_at is null and p_sent is not null
    returning 1
  ) select jsonb_build_object('updated',exists(select from updated))
$$;
revoke all on function public.openkoda_budget_notice_claim(),
  public.openkoda_budget_notice_finish(date,text,integer,integer,boolean) from public,anon,authenticated,service_role;
grant execute on function public.openkoda_budget_notice_claim(),
  public.openkoda_budget_notice_finish(date,text,integer,integer,boolean) to service_role;
