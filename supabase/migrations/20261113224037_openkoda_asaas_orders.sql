alter table openkoda_private.controls add column payments_enabled boolean not null default false,
  add column payment_environment text not null default 'sandbox' check(payment_environment in ('sandbox','production'));
create table openkoda_private.billing_orders (
  id uuid primary key default gen_random_uuid(), user_id uuid references auth.users(id) on delete set null,
  request_id uuid not null, plan text not null references openkoda_private.plans,
  price_brl_cents integer not null check(price_brl_cents>0),
  status text not null default 'pending' check(status in ('pending','linked','active','cancelled','refunded')),
  environment text not null check(environment in ('sandbox','production')),
  checkout_id text unique, checkout_url text, subscription_id text unique,
  created_at timestamptz not null default now(), unique(user_id,request_id)
);
create unique index openkoda_one_live_order on openkoda_private.billing_orders(user_id)
  where status in ('pending','linked','active') and user_id is not null;
create index on openkoda_private.billing_orders(plan);
create table openkoda_private.payment_cycles (
  payment text primary key, order_id uuid not null references openkoda_private.billing_orders,
  cycle_start timestamptz not null,cycle_end timestamptz not null, status text not null check(status in ('paid','refunded')),
  entitlement boolean not null,
  unique(order_id,cycle_start), check(cycle_end-cycle_start between interval '27 days' and interval '32 days')
);
create table openkoda_private.billing_events (
  event text primary key, payment text not null references openkoda_private.payment_cycles,
  status text not null check(status in ('paid','refunded')), received_at timestamptz not null default now()
);
create index on openkoda_private.billing_events(payment);
alter table openkoda_private.subscriptions add column billing_payment text references openkoda_private.payment_cycles(payment);
create index on openkoda_private.subscriptions(billing_payment) where billing_payment is not null;
alter table openkoda_private.billing_orders enable row level security;
alter table openkoda_private.payment_cycles enable row level security;
alter table openkoda_private.billing_events enable row level security;
revoke all on openkoda_private.billing_orders,openkoda_private.payment_cycles,openkoda_private.billing_events
  from public,anon,authenticated,service_role;
grant select,insert,update on openkoda_private.billing_orders,openkoda_private.payment_cycles to service_role;
grant select,insert on openkoda_private.billing_events to service_role;
grant insert,update on openkoda_private.subscriptions to service_role;

create function public.openkoda_billing_start(p_user uuid,p_request uuid,p_plan text) returns jsonb
language plpgsql security invoker set search_path='' as $$
declare c openkoda_private.controls; o openkoda_private.billing_orders; price integer; existing boolean:=true;
begin
  select * into strict c from openkoda_private.controls where singleton for update;
  if not c.payments_enabled then raise exception 'payments_disabled'; end if;
  perform openkoda_private.member(p_user);
  select price_brl_cents into price from openkoda_private.plans where id=p_plan;
  if not found or p_request is null then raise exception 'checkout_invalid'; end if;
  select * into o from openkoda_private.billing_orders where user_id=p_user and request_id=p_request;
  if found then
    if o.plan<>p_plan then raise exception 'idempotency_conflict'; end if;
  else
    if not exists(select from openkoda_private.accounts where user_id=p_user and terms_version=c.terms_version and privacy_version=c.privacy_version)
      then raise exception 'consent_required'; end if;
    if (select count(*) from openkoda_private.billing_orders where user_id=p_user and created_at>now()-interval '1 hour')>=5
      then raise exception 'rate_limited'; end if;
    if exists(select from openkoda_private.billing_orders where user_id=p_user and status in ('pending','linked','active'))
      or exists(select from openkoda_private.subscriptions where user_id=p_user and period_end>now())
      or exists(select from openkoda_private.payment_cycles pc join openkoda_private.billing_orders bo on bo.id=pc.order_id
        where bo.user_id=p_user and pc.entitlement and pc.status='paid' and pc.cycle_end>now())
      then raise exception 'checkout_in_progress'; end if;
    insert into openkoda_private.billing_orders(user_id,request_id,plan,price_brl_cents,environment) values(p_user,p_request,p_plan,price,c.payment_environment)
      returning * into o;
    existing:=false;
  end if;
  return jsonb_build_object('orderId',o.id,'priceBrlCents',o.price_brl_cents,'plan',o.plan,
    'existing',existing,'checkoutUrl',o.checkout_url,'status',o.status,'environment',o.environment,
    'billingAnchorDate',(o.created_at at time zone 'America/Sao_Paulo')::date);
end $$;
create function public.openkoda_billing_link(p_user uuid,p_request uuid,p_checkout_id text,p_url text) returns jsonb
language plpgsql security invoker set search_path='' as $$
declare o openkoda_private.billing_orders;
begin
  if not (select payments_enabled from openkoda_private.controls where singleton for share) then raise exception 'payments_disabled'; end if;
  perform openkoda_private.member(p_user);
  if p_checkout_id is null or p_checkout_id !~ '^[A-Za-z0-9_-]{6,128}$' or p_url is null or length(p_url)>1024
    or p_url !~ '^https://(www\.|sandbox\.)?asaas\.com/[A-Za-z0-9_./?=&%+-]+$' then raise exception 'checkout_invalid'; end if;
  select * into strict o from openkoda_private.billing_orders where user_id=p_user and request_id=p_request for update;
  if (o.environment='sandbox')<>(p_url like 'https://sandbox.asaas.com/%') then raise exception 'checkout_environment_mismatch'; end if;
  if o.checkout_id is not null then
    if o.checkout_id<>p_checkout_id or o.checkout_url<>p_url then raise exception 'idempotency_conflict'; end if;
  else
    if o.status<>'pending' then raise exception 'order_not_pending'; end if;
    update openkoda_private.billing_orders set checkout_id=p_checkout_id,checkout_url=p_url,status='linked' where id=o.id returning * into o;
  end if;
  return jsonb_build_object('orderId',o.id,'checkoutUrl',o.checkout_url,'status',o.status);
end $$;

create function public.openkoda_billing_event(p_event text,p_order uuid,p_payment text,p_status text,p_amount_cents integer,
  p_currency text,p_cycle_start timestamptz,p_cycle_end timestamptz,p_subscription text) returns jsonb
language plpgsql security invoker set search_path='' as $$
declare o openkoda_private.billing_orders; cycle openkoda_private.payment_cycles; e openkoda_private.billing_events;
  enabled boolean; old_subscription openkoda_private.subscriptions; replay boolean:=false;
begin
  select payments_enabled into strict enabled from openkoda_private.controls where singleton for update;
  if not enabled then raise exception 'payments_disabled'; end if;
  if p_event is null or p_event !~ '^[A-Za-z0-9_-]{5,160}$' or p_payment is null or p_payment !~ '^pay_[A-Za-z0-9_-]{1,120}$'
    or p_subscription is null or p_subscription !~ '^sub_[A-Za-z0-9_-]{1,120}$' or p_status is null or p_status not in ('paid','refunded')
    or p_currency is distinct from 'BRL' or p_cycle_start is null or p_cycle_end is null
    or (p_cycle_end-p_cycle_start) not between interval '27 days' and interval '32 days'
    then raise exception 'payment_invalid'; end if;
  select * into strict o from openkoda_private.billing_orders where id=p_order for update;
  if o.price_brl_cents is distinct from p_amount_cents or o.checkout_id is null
    or o.subscription_id is not null and o.subscription_id<>p_subscription then raise exception 'payment_mismatch'; end if;
  select * into e from openkoda_private.billing_events where event=p_event;
  if found and (e.payment<>p_payment or e.status<>p_status) then raise exception 'idempotency_conflict'; end if;
  replay:=found;
  select * into cycle from openkoda_private.payment_cycles where payment=p_payment for update;
  if found then
    if cycle.order_id<>p_order or cycle.cycle_start<>p_cycle_start or cycle.cycle_end<>p_cycle_end then raise exception 'payment_mismatch'; end if;
    if cycle.status='refunded' or cycle.status=p_status then replay:=true; end if;
  elsif exists(select from openkoda_private.payment_cycles where order_id=p_order
    and cycle_start<p_cycle_end and cycle_end>p_cycle_start) then raise exception 'cycle_overlap'; end if;
  if not replay then
    insert into openkoda_private.payment_cycles(payment,order_id,cycle_start,cycle_end,status,entitlement)
      values(p_payment,p_order,p_cycle_start,p_cycle_end,p_status,p_status='paid' and o.environment='production'
        and o.user_id is not null and o.status<>'cancelled')
      on conflict(payment) do update set status=excluded.status, entitlement=excluded.entitlement;
    update openkoda_private.billing_orders set subscription_id=p_subscription where id=p_order and subscription_id is null;
    if o.user_id is not null and p_status='refunded' then
      delete from openkoda_private.subscriptions where user_id=o.user_id and billing_payment=p_payment;
    elsif o.user_id is not null and p_status='paid' and o.environment='production' and o.status<>'cancelled' and p_cycle_end>now()
      and not exists(select from openkoda_private.accounts where user_id=o.user_id and deletion_pending) then
      if exists(select from openkoda_private.payment_cycles pc join openkoda_private.billing_orders bo on bo.id=pc.order_id
        where bo.user_id=o.user_id and pc.payment<>p_payment and pc.entitlement and pc.status='paid'
          and pc.cycle_start<p_cycle_end and pc.cycle_end>p_cycle_start) then raise exception 'cycle_overlap'; end if;
      select * into old_subscription from openkoda_private.subscriptions where user_id=o.user_id for update;
      if old_subscription.user_id is not null and old_subscription.period_start<p_cycle_end and old_subscription.period_end>p_cycle_start then
        raise exception 'cycle_overlap';
      end if;
      -- Future cycles are stored now; period() resolves their entitlement at their
      -- exact start, even when the next payment arrives before the current term ends.
      if p_cycle_start<=now() and (old_subscription.user_id is null or old_subscription.period_start<=p_cycle_start) then
        insert into openkoda_private.subscriptions(user_id,plan,period_start,period_end,billing_payment)
          values(o.user_id,o.plan,p_cycle_start,p_cycle_end,p_payment)
          on conflict(user_id) do update set plan=excluded.plan,period_start=excluded.period_start,
            period_end=excluded.period_end,billing_payment=excluded.billing_payment,cancel_at_period_end=false;
      end if;
      update openkoda_private.billing_orders set status='active' where id=o.id;
    end if;
  end if;
  insert into openkoda_private.billing_events(event,payment,status) values(p_event,p_payment,p_status) on conflict(event) do nothing;
  select * into strict cycle from openkoda_private.payment_cycles where payment=p_payment;
  return jsonb_build_object('orderId',p_order,'status',cycle.status,'replay',replay);
end $$;

create function public.openkoda_billing_cancel_target(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare o openkoda_private.billing_orders;
begin
  perform openkoda_private.member(p_user);
  select * into o from openkoda_private.billing_orders where user_id=p_user and status in ('pending','linked','active')
    order by created_at desc limit 1;
  if not found then return null; end if;
  return jsonb_build_object('orderId',o.id,'subscriptionId',o.subscription_id,'checkoutId',o.checkout_id,'environment',o.environment);
end $$;
create function public.openkoda_billing_checkout(p_checkout_id text) returns jsonb language sql stable security invoker set search_path='' as $$
  select jsonb_build_object('orderId',id,'priceBrlCents',price_brl_cents,'checkoutId',checkout_id,
    'billingAnchorDate',(created_at at time zone 'America/Sao_Paulo')::date,'environment',environment)
  from openkoda_private.billing_orders where checkout_id=p_checkout_id
$$;
create function public.openkoda_billing_checkout_closed(p_checkout_id text,p_environment text,p_status text) returns jsonb
language plpgsql security invoker set search_path='' as $$
declare o openkoda_private.billing_orders;
begin
  perform 1 from openkoda_private.controls where singleton for update;
  if p_environment is null or p_environment not in ('sandbox','production') or p_status is null
    or p_status not in ('CHECKOUT_CANCELED','CHECKOUT_EXPIRED') then raise exception 'checkout_invalid'; end if;
  select * into o from openkoda_private.billing_orders where checkout_id=p_checkout_id and environment=p_environment for update;
  if not found or o.subscription_id is not null or exists(select from openkoda_private.payment_cycles where order_id=o.id)
    then return jsonb_build_object('closed',false); end if;
  if o.status='linked' then update openkoda_private.billing_orders set status='cancelled' where id=o.id; end if;
  return jsonb_build_object('closed',o.status in ('linked','cancelled'));
end $$;
create function public.openkoda_billing_cancel_commit(p_user uuid,p_order uuid,p_checkout_id text,p_subscription_id text)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare o openkoda_private.billing_orders;
begin
  perform 1 from openkoda_private.controls where singleton for update;
  perform openkoda_private.member(p_user);
  select * into strict o from openkoda_private.billing_orders where id=p_order and user_id=p_user for update;
  if p_checkout_id is null or o.checkout_id is distinct from p_checkout_id or o.subscription_id is distinct from p_subscription_id
    then raise exception 'cancellation_target_changed'; end if;
  if o.status not in ('active','linked','cancelled') then raise exception 'order_not_cancellable'; end if;
  update openkoda_private.billing_orders set status='cancelled' where id=o.id;
  update openkoda_private.subscriptions sub set cancel_at_period_end=true where sub.user_id=p_user
    and exists(select from openkoda_private.payment_cycles c where c.payment=sub.billing_payment and c.order_id=o.id);
  return jsonb_build_object('cancelAtPeriodEnd',true);
end $$;

create or replace function openkoda_private.period(p_user uuid,p_at timestamptz)
returns table(plan_id text, month_start timestamptz, month_end timestamptz, week_start timestamptz,
              week_end timestamptz, month_limit bigint, week_limit bigint, cancel_at_end boolean)
language sql stable security invoker set search_path='' as $$
  with entitlement(plan,period_start,period_end,cancel_at_period_end) as (
    select sub.plan,sub.period_start,sub.period_end,sub.cancel_at_period_end from openkoda_private.subscriptions sub
      where sub.user_id=p_user and sub.billing_payment is null and p_at>=sub.period_start and p_at<sub.period_end
    union all select o.plan,c.cycle_start,c.cycle_end,(o.status='cancelled') from openkoda_private.payment_cycles c
      join openkoda_private.billing_orders o on o.id=c.order_id where o.user_id=p_user and c.status='paid' and c.entitlement
        and p_at>=c.cycle_start and p_at<c.cycle_end
  ), s as (select e.*,p.monthly_limit,extract(epoch from period_end-period_start) as secs,
    floor(extract(epoch from p_at-period_start)/604800)::int as block from entitlement e
    join openkoda_private.plans p on p.id=e.plan order by e.period_start desc limit 1)
  select plan,period_start,period_end,period_start+block*interval '168 hours',
    least(period_end,period_start+(block+1)*interval '168 hours'),monthly_limit,
    (floor(monthly_limit*least(secs,604800*(block+1))/secs)-floor(monthly_limit*(604800*block)/secs))::bigint,
    cancel_at_period_end from s
$$;
create or replace function public.openkoda_cancel(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare active boolean;
begin
  perform 1 from openkoda_private.controls where singleton for update;
  perform openkoda_private.member(p_user);
  if exists(select from openkoda_private.billing_orders where user_id=p_user and status in ('pending','linked','active'))
    then raise exception 'provider_cancellation_required'; end if;
  update openkoda_private.subscriptions set cancel_at_period_end=true where user_id=p_user and period_end>now();
  active:=found;
  return jsonb_build_object('cancelAtPeriodEnd',active);
end $$;
do $rights$
declare obj record;
begin
  for obj in select p.oid::regprocedure as signature from pg_proc p join pg_namespace n on n.oid=p.pronamespace
    where n.nspname='public' and p.proname in ('openkoda_billing_start','openkoda_billing_link','openkoda_billing_event',
      'openkoda_billing_cancel_target','openkoda_billing_checkout','openkoda_billing_cancel_commit','openkoda_billing_checkout_closed') loop
    execute format('revoke all on function %s from public,anon,authenticated,service_role',obj.signature);
    execute format('grant execute on function %s to service_role',obj.signature);
  end loop;
end $rights$;

alter function public.openkoda_account(uuid) rename to openkoda_account_core;
alter function public.openkoda_account_core(uuid) set schema openkoda_private;
create function public.openkoda_account(p_user uuid) returns jsonb language sql security invoker set search_path='' as $$
  select jsonb_set(openkoda_private.openkoda_account_core(p_user),'{controls,paymentsEnabled}',
    to_jsonb((select payments_enabled from openkoda_private.controls where singleton)))
$$;
alter function public.openkoda_export(uuid) rename to openkoda_export_core;
alter function public.openkoda_export_core(uuid) set schema openkoda_private;
create function public.openkoda_export(p_user uuid) returns jsonb language sql security invoker set search_path='' as $$
  select openkoda_private.openkoda_export_core(p_user) || jsonb_build_object('billingOrders',coalesce((select
    jsonb_agg(jsonb_build_object('orderId',o.id,'plan',o.plan,'priceBrlCents',o.price_brl_cents,'status',o.status,
      'createdAt',o.created_at,'cycles',coalesce((select jsonb_agg(jsonb_build_object('payment',c.payment,'status',c.status,
        'start',c.cycle_start,'end',c.cycle_end) order by c.cycle_start) from openkoda_private.payment_cycles c
        where c.order_id=o.id),'[]'::jsonb)) order by o.created_at) from openkoda_private.billing_orders o
        where o.user_id=p_user),'[]'::jsonb))
$$;
alter function public.openkoda_delete(uuid) rename to openkoda_delete_core;
alter function public.openkoda_delete_core(uuid) set schema openkoda_private;
create function public.openkoda_delete(p_user uuid) returns jsonb language plpgsql security invoker set search_path='' as $$
declare result jsonb;
begin
  result:=openkoda_private.openkoda_delete_core(p_user);
  -- The provider cancellation must succeed first. On error the entire local deletion rolls back.
  if exists(select from openkoda_private.billing_orders where user_id=p_user and status in ('linked','active','pending'))
    then raise exception 'provider_cancellation_required'; end if;
  update openkoda_private.billing_orders set user_id=null,checkout_url=null where user_id=p_user;
  return result;
end $$;
revoke all on function public.openkoda_account(uuid),public.openkoda_export(uuid),public.openkoda_delete(uuid)
  from public,anon,authenticated,service_role;
grant execute on function public.openkoda_account(uuid),public.openkoda_export(uuid),public.openkoda_delete(uuid) to service_role;
