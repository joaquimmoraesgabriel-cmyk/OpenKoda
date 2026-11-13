-- A post-login handoff can initialize a new account exactly once. Only the
-- explicit preferences endpoint may change a previously recorded choice.
create function public.openkoda_consent_initial(p_user uuid,p_terms_version text,p_privacy_version text,p_marketing boolean)
returns jsonb language plpgsql security invoker set search_path='' as $$
declare c openkoda_private.controls; a openkoda_private.accounts;
begin
  perform openkoda_private.member(p_user);
  select * into strict c from openkoda_private.controls where singleton for share;
  if p_terms_version is distinct from c.terms_version or p_privacy_version is distinct from c.privacy_version
    or p_marketing is null then raise exception 'consent_version_invalid'; end if;
  insert into openkoda_private.accounts(user_id) values(p_user) on conflict(user_id) do nothing;
  select * into strict a from openkoda_private.accounts where user_id=p_user for update;
  if a.deletion_pending then raise exception 'account_unavailable'; end if;
  if a.accepted_at is not null or a.marketing_updated_at is not null then
    return jsonb_build_object('accepted',a.terms_version is not distinct from c.terms_version
      and a.privacy_version is not distinct from c.privacy_version and a.accepted_at is not null,'existing',true);
  end if;
  perform public.openkoda_consent(p_user,p_terms_version,p_privacy_version,p_marketing);
  return jsonb_build_object('accepted',true,'existing',false);
end $$;
revoke all on function public.openkoda_consent_initial(uuid,text,text,boolean) from public,anon,authenticated,service_role;
grant execute on function public.openkoda_consent_initial(uuid,text,text,boolean) to service_role;
