create or replace function public.openkoda_billing_checkout(p_checkout_id text) returns jsonb language sql stable security invoker set search_path='' as $$
  select jsonb_build_object('orderId',id,'priceBrlCents',price_brl_cents,'checkoutId',checkout_id,'subscriptionId',subscription_id,
    'billingAnchorDate',(created_at at time zone 'America/Sao_Paulo')::date,'environment',environment)
  from openkoda_private.billing_orders where checkout_id=p_checkout_id
$$;
create function public.openkoda_billing_subscription(p_subscription text) returns jsonb language sql stable security invoker set search_path='' as $$
  select public.openkoda_billing_checkout(checkout_id) from openkoda_private.billing_orders where subscription_id=p_subscription
$$;
revoke all on function public.openkoda_billing_checkout(text),public.openkoda_billing_subscription(text)
  from public,anon,authenticated,service_role;
grant execute on function public.openkoda_billing_checkout(text),public.openkoda_billing_subscription(text) to service_role;
