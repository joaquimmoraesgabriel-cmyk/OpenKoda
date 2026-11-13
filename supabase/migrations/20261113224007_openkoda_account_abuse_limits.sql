-- These checks run inside the same request transaction. A rejected audit event
-- rolls back the preference change. Revoking marketing remains available.
create function openkoda_private.account_write_quota() returns trigger
language plpgsql security invoker set search_path='' as $$
begin
  perform pg_advisory_xact_lock(hashtextextended(new.user_id::text,14));
  if tg_table_name='checkouts' then
    if exists(select from openkoda_private.checkouts where user_id=new.user_id and request_id=new.request_id) then return new; end if;
    if (select count(*) from openkoda_private.checkouts where user_id=new.user_id and created_at>now()-interval '1 day')>=30
      then raise exception 'rate_limited'; end if;
  elsif new.marketing and
    (select count(*) from openkoda_private.consent_events where user_id=new.user_id and recorded_at>now()-interval '1 day')>=30 then
    raise exception 'rate_limited';
  end if;
  return new;
end $$;
revoke all on function openkoda_private.account_write_quota() from public,anon,authenticated,service_role;
create trigger openkoda_checkout_quota before insert on openkoda_private.checkouts
  for each row execute function openkoda_private.account_write_quota();
create trigger openkoda_consent_quota before insert on openkoda_private.consent_events
  for each row execute function openkoda_private.account_write_quota();

create or replace function public.openkoda_support_create(p_user uuid,p_subject text,p_message text,p_request uuid)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare ticket openkoda_private.tickets; created boolean:=false;
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
      values(p_user,p_request,p_subject,p_message) returning * into ticket; created:=true;
  end if;
  return jsonb_build_object('id',ticket.id,'created',created);
end $$;
