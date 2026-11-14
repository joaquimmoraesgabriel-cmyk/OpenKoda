-- Existing tickets used the previous direct sender. Only new tickets enter this queue.
alter table openkoda_private.tickets
  add column notice_pending boolean not null default false,
  add column notice_attempts smallint not null default 0,
  add column notice_retry_at timestamptz,
  add column notice_first_at timestamptz,
  add column notice_sent_at timestamptz,
  add column notice_failed_at timestamptz;
alter table openkoda_private.tickets alter column notice_pending set default true;
create index openkoda_ticket_notices_due on openkoda_private.tickets (notice_retry_at,created_at)
  where notice_pending;
grant update (notice_pending,notice_attempts,notice_first_at,notice_retry_at,notice_sent_at,notice_failed_at)
  on openkoda_private.tickets to service_role;

create function public.openkoda_support_notice_claim() returns jsonb
language sql volatile security invoker set search_path='' as $$
  with due as (
    select id from openkoda_private.tickets where notice_pending
      and (notice_retry_at is null or notice_retry_at <= now())
    order by created_at,id limit 4 for update skip locked
  ), claimed as (
    update openkoda_private.tickets t set
      notice_attempts=t.notice_attempts+1,
      notice_first_at=coalesce(t.notice_first_at,now()),
      notice_retry_at=now()+interval '2 minutes',
      notice_pending=t.notice_attempts<4 and (t.notice_first_at is null or t.notice_first_at>now()-interval '23 hours'),
      notice_failed_at=case when t.notice_attempts>=4 or t.notice_first_at<=now()-interval '23 hours' then now() end
    from due where t.id=due.id returning t.id,t.notice_attempts,t.notice_pending
  )
  select jsonb_build_object('tickets',coalesce(jsonb_agg(jsonb_build_object('id',id,'attempt',notice_attempts))
    filter(where notice_pending),'[]'::jsonb)) from claimed;
$$;
create function public.openkoda_support_notice_finish(p_id uuid,p_attempt integer,p_outcome text) returns jsonb
language sql volatile security invoker set search_path='' as $$
  with updated as (
    update openkoda_private.tickets t set
      notice_pending=p_outcome='retry' and p_attempt<4,
      notice_sent_at=case when p_outcome='sent' then now() end,
      notice_retry_at=case when p_outcome='retry' and p_attempt=1 then now()+interval '8 seconds'
        when p_outcome='retry' and p_attempt<4 then now()+interval '2 minutes'*p_attempt end,
      notice_failed_at=case when p_outcome='failed' or (p_outcome='retry' and p_attempt>=4) then now() end
    where t.id=p_id and t.notice_pending and t.notice_attempts=p_attempt
      and p_attempt between 1 and 4 and p_outcome in ('sent','retry','failed') returning 1
  ) select jsonb_build_object('updated',exists(select from updated));
$$;
revoke all on function public.openkoda_support_notice_claim(),public.openkoda_support_notice_finish(uuid,integer,text)
  from public,anon,authenticated,service_role;
grant execute on function public.openkoda_support_notice_claim(),public.openkoda_support_notice_finish(uuid,integer,text)
  to service_role;
