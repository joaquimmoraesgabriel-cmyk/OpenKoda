"""Exercise the commercial schema in a new local database, with real concurrent connections."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone
from pathlib import Path
import json
import subprocess
import sys
import uuid

if len(sys.argv) != 2 or sys.argv[1] != "--local":
    raise SystemExit("Usage: python supabase/tests/commercial.py --local")
CONTAINER = "openkoda-commercial-pg17-verify"
DATABASE = "openkoda_test_" + uuid.uuid4().hex
ROOT = Path(__file__).resolve().parents[1]
USER = str(uuid.uuid4())
OTHER = str(uuid.uuid4())
MODEL = "google/gemini-2.5-flash-lite"
DIGEST = "a"*64
IP = "b"*64


def run(sql, db=DATABASE):
    result = subprocess.run(["docker", "exec", "-i", CONTAINER, "psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1",
                             "-U", "postgres", "-d", db], input=sql, encoding="utf-8",
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise RuntimeError(result.stderr[-1500:].strip())
    return result.stdout.strip()


def rpc(name, *args):
    def literal(value):
        if value is None:
            return "NULL"
        if isinstance(value, bool):
            return str(value).lower()
        if isinstance(value, int):
            return str(value)
        return "'" + value.replace("'", "''") + "'"
    result=run("set role service_role; select public.openkoda_" + name + "(" + ",".join(map(literal, args)) + ");")
    return json.loads(result) if result else None


def denied(error, fn, *args):
    try:
        fn(*args)
    except RuntimeError as exc:
        assert error in str(exc), (error, str(exc))
        return
    raise AssertionError("Expected denial: " + error)


def reserve(user, request, size=6, digest=DIGEST):
    return rpc("ai_reserve",user,request,digest,MODEL,"chat",IP,size)


def race(calls):
    def execute(call):
        try:
            return call()
        except RuntimeError as exc:
            return str(exc)
    with ThreadPoolExecutor(max_workers=len(calls)) as pool:
        return list(pool.map(execute,calls))


run("create database " + DATABASE + ";", "postgres")
try:
    # PG roles are cluster-wide. This explicitly named offline fixture can be reused.
    run("""do $$begin
      if not exists(select from pg_roles where rolname='anon') then create role anon; end if;
      if not exists(select from pg_roles where rolname='authenticated') then create role authenticated; end if;
      if not exists(select from pg_roles where rolname='service_role') then create role service_role bypassrls; end if;
      if not exists(select from pg_roles where rolname='authenticator') then create role authenticator; end if;
    end$$; create schema auth; create table auth.users(id uuid primary key,email text,email_confirmed_at timestamptz);
    create function auth.uid() returns uuid language sql stable as
      $$ select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid $$;
    alter default privileges in schema public grant execute on functions to authenticated,anon,service_role;""")
    for file in sorted((ROOT/"migrations").glob("*_openkoda*.sql")):
        run("begin;\n"+file.read_text(encoding="utf-8")+"\ncommit;")
    run(f"insert into auth.users values('{USER}','one@example.invalid',now()),('{OTHER}','two@example.invalid',now());")
    for role in ["anon","authenticated"]:
        denied("permission denied",run,f"set role {role}; select public.openkoda_export('{USER}');")
        denied("permission denied",run,f"set role {role}; select * from openkoda_private.requests;")
    assert run("""select count(*) from pg_proc p join pg_namespace n on n.oid=p.pronamespace
      where n.nspname='public' and p.proname like 'openkoda_%' and
        (has_function_privilege('authenticated',p.oid,'EXECUTE') or has_function_privilege('anon',p.oid,'EXECUTE'));""") == "0"
    assert run("select count(*) from pg_class where relnamespace='openkoda_private'::regnamespace and relkind='r' and not relrowsecurity;") == "0"
    assert rpc("account",USER)["plan"] is None
    assert rpc("account",USER)["controls"]["paymentsEnabled"] is False
    newcomer=str(uuid.uuid4())
    run(f"insert into auth.users values('{newcomer}','consent@example.invalid',now());")
    def initial(choice):
        return rpc("consent_initial",newcomer,"2026-11-13","2026-11-13",choice)
    first=race([lambda:initial(True),lambda:initial(False)])
    assert sorted(r["existing"] for r in first)==[False,True]
    expected=not first[0]["existing"]
    assert rpc("account",newcomer)["consent"]["marketing"] is expected
    rpc("marketing",newcomer,False)
    assert initial(True)=={"accepted":True,"existing":True}
    assert rpc("account",newcomer)["consent"]["marketing"] is False
    for user in [USER, OTHER]:
        rpc("consent",user,"2026-11-13","2026-11-13",False)
    denied("consent_version_invalid",rpc,"consent",USER,"old","old",True)
    rpc("marketing",USER,True)
    assert rpc("marketing",USER,False)["enabled"] is False
    mock_id=str(uuid.uuid4())
    assert rpc("checkout_mock",USER,"plus",mock_id)["grantsProductionCredit"] is False
    assert rpc("checkout_mock",USER,"plus",mock_id)["id"] == mock_id
    denied("idempotency_conflict",rpc,"checkout_mock",USER,"pro",mock_id)
    run(f"insert into openkoda_private.checkouts(user_id,request_id,plan) select '{USER}',gen_random_uuid(),'plus' from generate_series(1,29);")
    assert rpc("checkout_mock",USER,"plus",mock_id)["id"] == mock_id
    denied("rate_limited",rpc,"checkout_mock",USER,"plus",str(uuid.uuid4()))
    run(f"insert into openkoda_private.consent_events(user_id,marketing) select '{USER}',false from generate_series(1,30);")
    denied("rate_limited",rpc,"marketing",USER,True)
    assert rpc("marketing",USER,False)["enabled"] is False
    denied("subscription_required",rpc,"skills",USER)
    denied("inference_disabled",reserve,USER,str(uuid.uuid4()))
    support_request=str(uuid.uuid4())
    created=rpc("support_create",USER,"Teste de acesso","Mensagem sintética",support_request)
    assert created["created"] is True
    ticket=created["id"]
    assert rpc("support_create",USER,"Teste de acesso","Mensagem sintética",support_request)=={"id":ticket,"created":False}
    run(f"insert into openkoda_private.replies(ticket_id,message) values('{ticket}','Resposta do atendimento');")
    assert rpc("support_list",OTHER)["tickets"] == []
    assert rpc("support_list",USER)["tickets"][0]["replies"][0]["message"]=="Resposta do atendimento"
    assert rpc("export",OTHER)["email"]=="two@example.invalid"
    run("update openkoda_private.controls set inference_enabled=true,daily_limit=10,monthly_limit=10,max_parallel=9,user_parallel=8,user_per_minute=200;")
    denied("subscription_required",reserve,USER,str(uuid.uuid4()))
    run(f"""insert into openkoda_private.subscriptions(user_id,plan,period_start,period_end)
      values('{USER}','plus',now()-interval '1 day',now()+interval '29 days'),
            ('{OTHER}','plus',now()-interval '1 day',now()+interval '29 days');""")
    assert rpc("account",USER)["usage"]["windowLimitMicros"] == 4666666
    for invalid in ["openrouter/auto","vendor/model:free","vendor","@verified"]:
        denied("model_not_allowed",rpc,"ai_reserve",USER,str(uuid.uuid4()),DIGEST,invalid,"chat",IP,3)
    assert run("select openkoda_private.model_allowed('plus','future/model','chat')")=="t"
    run("delete from openkoda_private.model_access where model='@verified' and plan='plus'")
    assert run("select openkoda_private.model_allowed('plus','future/model','chat')")=="f"
    assert run("select openkoda_private.model_allowed('plus','openai/gpt-5-nano','chat')")=="t"
    skills=rpc("skills",USER)["skills"]
    assert len(skills)==39 and len({s["id"] for s in skills})==39
    tdd=rpc("skill",USER,"tdd")
    assert "MIT License" in tdd["license"] and tdd["revision"]=="5863806dd4e15c63174bee4e7515a4256792ecfa"
    assert rpc("skill",USER,"implement")["invocation"]=="explicit"
    assert "agents/openai.yaml" in tdd["resources"] and tdd["body"].startswith("---")
    requests=[str(uuid.uuid4()) for _ in range(2)]
    results=race([lambda u=u,r=r:reserve(u,r) for u,r in zip([USER,OTHER],requests)])
    assert sum(isinstance(result,dict) for result in results)==1 and sum("platform_quota_exceeded" in str(result) for result in results)==1
    winner=next(i for i,item in enumerate(results) if isinstance(item,dict))
    owner=[USER,OTHER][winner]; request=requests[winner]
    assert reserve(owner,request)["replay"]
    denied("idempotency_conflict",reserve,owner,request,6,"d"*64)
    dispatched=race([lambda:rpc("ai_dispatch",owner,request)]*2)
    assert sorted(d["dispatch"] for d in dispatched)==[False,True]
    denied("settlement_conflict",rpc,"ai_settle",owner,request,"not_sent",None,None,None,None)
    rpc("ai_settle",owner,request,"unknown",None,None,None,"gen-test-01")
    assert int(run("select sum(liability) from openkoda_private.requests"))==6
    rpc("ai_settle",owner,request,"completed",4,10,20,"gen-test-01")
    assert rpc("ai_settle",owner,request,"completed",4,10,20,"gen-test-01")["status"]=="completed"
    denied("settlement_conflict",rpc,"ai_settle",owner,request,"completed",3,10,20,"gen-test-01")
    run("update openkoda_private.controls set monthly_limit=10000,daily_limit=10000,user_parallel=1;")
    ids=[str(uuid.uuid4()),str(uuid.uuid4())]
    same=race([lambda r=r:reserve(USER,r) for r in ids])
    assert sum(isinstance(x,dict) for x in same)==1 and sum("concurrency_limited" in str(x) for x in same)==1
    active=next(x["requestId"] for x in same if isinstance(x,dict))
    run("update openkoda_private.controls set inference_enabled=false;")
    assert rpc("ai_dispatch",USER,active)=={"requestId":active,"status":"not_sent","dispatch":False}
    run("update openkoda_private.controls set inference_enabled=true,user_parallel=8,ip_per_minute=1;")
    denied("rate_limited",reserve,OTHER,str(uuid.uuid4()))
    run("update openkoda_private.controls set ip_per_minute=100; update openkoda_private.plans set monthly_limit=24 where id='plus';")
    # A 30-day cycle receives five complete weekly micros. Two concurrent reservations of 3 cannot both win.
    fresh = OTHER if owner==USER else USER
    quota=race([lambda:reserve(fresh,str(uuid.uuid4()),3)]*2)
    assert sum(isinstance(x,dict) for x in quota)==1 and sum("quota_exceeded" in str(x) for x in quota)==1
    # A charge earlier in this cycle must count independently of this week's allowance.
    run(f"""update openkoda_private.subscriptions set period_start=now()-interval '9 days',period_end=now()+interval '21 days' where user_id='{fresh}';
      insert into openkoda_private.requests(id,user_id,model,operation,reserved,actual,status,created_at,updated_at,expires_at)
      values(gen_random_uuid(),'{fresh}','{MODEL}','chat',21,21,'completed',now()-interval '8 days',now(),now());""")
    denied("quota_exceeded",reserve,fresh,str(uuid.uuid4()),1)
    run("update openkoda_private.plans set monthly_limit=20000000 where id='plus';")
    expired=str(uuid.uuid4())
    reserve(USER,expired)
    run(f"update openkoda_private.requests set expires_at=now()-interval '1 second' where id='{expired}';")
    assert rpc("ai_dispatch",USER,expired)["status"]=="not_sent"
    interrupted=str(uuid.uuid4())
    reserve(USER,interrupted); assert rpc("ai_dispatch",USER,interrupted)["dispatch"]
    run(f"update openkoda_private.requests set expires_at=now()-interval '1 second' where id='{interrupted}'; select openkoda_private.retention();")
    assert reserve(USER,interrupted)["status"]=="unknown"
    rpc("ai_settle",USER,interrupted,"unknown",None,None,None,"gen-test-recovered")
    rpc("ai_settle",USER,interrupted,"completed",7,1,4,"gen-test-recovered")
    assert not rpc("account",USER)["controls"]["inferenceEnabled"]
    run(f"""update openkoda_private.plans set monthly_limit=20000000 where id='plus';
      update openkoda_private.subscriptions set period_start=now()-interval '29 days',period_end=now()+interval '1 day'
      where user_id='{USER}';""")
    assert rpc("account",USER)["usage"]["windowLimitMicros"]==1333334
    assert rpc("cancel",USER)["cancelAtPeriodEnd"]
    run(f"update openkoda_private.tickets set closed_at=now()-interval '31 days' where id='{ticket}';select openkoda_private.retention();")
    assert rpc("support_list",USER)["tickets"]==[]
    assert rpc("delete",owner)["scheduled"]
    denied("account_unavailable",rpc,"account",owner)
    assert run(f"select count(*) from openkoda_private.requests where user_id='{owner}'")=="0"
    assert int(run("select sum(liability) from openkoda_private.requests"))>=4
    payer=str(uuid.uuid4())
    run(f"insert into auth.users values('{payer}','payer@example.invalid',now());")
    rpc("consent",payer,"2026-11-13","2026-11-13",False)
    checkout_request=str(uuid.uuid4())
    denied("payments_disabled",rpc,"billing_start",payer,checkout_request,"pro")
    run("update openkoda_private.controls set payments_enabled=true;")
    simulated_request=str(uuid.uuid4())
    expires_request=str(uuid.uuid4())
    rpc("billing_start",payer,expires_request,"plus")
    rpc("billing_link",payer,expires_request,"checkout_expired","https://sandbox.asaas.com/checkoutSession/show?id=expired")
    assert not rpc("billing_checkout_closed","checkout_expired","production","CHECKOUT_EXPIRED")["closed"]
    assert rpc("billing_checkout_closed","checkout_expired","sandbox","CHECKOUT_EXPIRED")["closed"]
    assert rpc("billing_checkout_closed","checkout_expired","sandbox","CHECKOUT_CANCELED")["closed"]
    simulated=rpc("billing_start",payer,simulated_request,"plus")
    assert simulated["environment"]=="sandbox" and simulated["billingAnchorDate"]
    rpc("billing_link",payer,simulated_request,"checkout_sandbox","https://sandbox.asaas.com/checkoutSession/show?id=demo")
    demo_time=datetime.now(timezone.utc)
    rpc("billing_event","evt_demo",simulated["orderId"],"pay_demo","paid",21900,"BRL",
      (demo_time-timedelta(days=1)).isoformat(),(demo_time+timedelta(days=29)).isoformat(),"sub_demo")
    assert rpc("account",payer)["plan"] is None
    denied("subscription_required",rpc,"skills",payer)
    rpc("billing_cancel_commit",payer,simulated["orderId"],"checkout_sandbox","sub_demo")
    run("update openkoda_private.controls set payment_environment='production';")
    assert rpc("account",payer)["controls"]["paymentsEnabled"]
    order=rpc("billing_start",payer,checkout_request,"pro")
    assert order["existing"] is False and order["priceBrlCents"]==99900
    order_id=order["orderId"]
    assert rpc("billing_start",payer,checkout_request,"pro")["existing"]
    denied("checkout_in_progress",rpc,"billing_start",payer,str(uuid.uuid4()),"plus")
    denied("checkout_invalid",rpc,"billing_link",payer,checkout_request,"abc123","https://asaas.com.attacker.test/x")
    rpc("billing_link",payer,checkout_request,"checkout_123","https://asaas.com/checkoutSession/show?id=abc123")
    assert rpc("billing_checkout","checkout_123")["orderId"]==order_id
    target=rpc("billing_cancel_target",payer)
    assert target["checkoutId"]=="checkout_123" and target["subscriptionId"] is None
    now=datetime.now(timezone.utc)
    def bill(event,payment,state,day,*,amount=99900):
        return rpc("billing_event",event,order_id,payment,state,amount,"BRL",(now+timedelta(days=day)).isoformat(),
                   (now+timedelta(days=day+30)).isoformat(),"sub_recurring")
    denied("payment_mismatch",lambda:bill("evt_wrong","pay_first","paid",-32,amount=1))
    assert bill("evt_a","pay_first","paid",-32)["replay"] is False
    assert rpc("billing_subscription","sub_recurring")==rpc("billing_checkout","checkout_123")
    assert rpc("billing_subscription","sub_unknown") is None
    twice=race([lambda:bill("evt_b","pay_second","paid",-2)]*2)
    assert sorted(evt["replay"] for evt in twice)==[False,True]
    assert rpc("account",payer)["plan"]["id"]=="pro"
    assert bill("evt_c","pay_first","refunded",-32)["status"]=="refunded"
    assert rpc("account",payer)["plan"]["id"]=="pro"
    assert bill("evt_d","pay_first","paid",-32)["status"]=="refunded"
    denied("cycle_overlap",bill,"evt_overlap","pay_overlap","paid",-4)
    bill("evt_future","pay_future","paid",28)
    assert run(f"select plan_id from openkoda_private.period('{payer}',now()+interval '29 days');")=="pro"
    denied("cancellation_target_changed",rpc,"billing_cancel_commit",payer,order_id,"checkout_123",None)
    denied("provider_cancellation_required",rpc,"delete",payer)
    denied("provider_cancellation_required",rpc,"cancel",payer)
    assert rpc("billing_cancel_target",payer)["subscriptionId"]=="sub_recurring"
    assert rpc("billing_cancel_commit",payer,order_id,"checkout_123","sub_recurring")["cancelAtPeriodEnd"]
    assert rpc("account",payer)["subscription"]["cancelAtPeriodEnd"]
    assert rpc("billing_cancel_target",payer) is None
    bill("evt_after","pay_after","paid",58)
    assert run(f"select count(*) from openkoda_private.period('{payer}',now()+interval '59 days');")=="0"
    bill("evt_pre_refund","pay_pre_refund","refunded",88)
    assert bill("evt_late","pay_pre_refund","paid",88)["status"]=="refunded"
    denied("idempotency_conflict",bill,"evt_late","pay_other","paid",118)
    assert order_id in [item["orderId"] for item in rpc("export",payer)["billingOrders"]]
    assert rpc("delete",payer)["scheduled"]
    assert run(f"select user_id is null and checkout_url is null from openkoda_private.billing_orders where id='{order_id}';")=="t"
    run("""update openkoda_private.controls set daily_limit=1000000,monthly_limit=1500000;
      insert into openkoda_private.requests(id,operation,model,status,reserved,actual,created_at,updated_at,expires_at)
      select gen_random_uuid(),'chat','fixture','completed',1000,1000,now()-n*interval '1 day',now(),now() from generate_series(1,7)n;
      insert into openkoda_private.requests(id,operation,model,status,reserved,actual,created_at,updated_at,expires_at)
      select gen_random_uuid(),'chat','fixture','completed',480000,480000,now(),now(),now() from generate_series(1,3);""")
    alerts=rpc("budget_health")["alerts"]
    assert {("day",80),("day",95),("month",80),("month",95),("daily_spike",200)}=={(a["kind"],a["threshold"]) for a in alerts}
    assert len(alerts)==len({(a["kind"],a["window_start"],a["threshold"]) for a in alerts})
    claims = race([lambda:rpc("budget_notice_claim")] * 2)
    notices = [a for result in claims for a in result["alerts"]]
    assert len(notices) == 5 and all(a["attempt"] == 1 for a in notices)
    assert len({(a["windowStart"], a["kind"], a["threshold"]) for a in notices}) == 5
    retry = notices.pop()
    def finish(a, sent):
        return rpc("budget_notice_finish", a["windowStart"], a["kind"], a["threshold"], a["attempt"], sent)["updated"]
    assert all(finish(a, True) for a in notices)
    assert finish(retry, False) and not rpc("budget_notice_claim")["alerts"]
    run("update openkoda_private.budget_alerts set notice_retry_at=now()-interval '1 second' where notice_sent_at is null;")
    second, = rpc("budget_notice_claim")["alerts"]
    assert second == {**retry, "attempt":2}
    assert not finish(retry, True) and finish(second, True) and not rpc("budget_notice_claim")["alerts"]
    run("update openkoda_private.budget_alerts set notice_sent_at=null,notice_retry_at=null,created_at=now()-interval '23 hours';")
    assert not rpc("budget_notice_claim")["alerts"]
    print("PASS: RBAC/RLS, consent, support/replies, export/deletion, catalogue, mock checkout, period/rate caps, concurrent quotas and dispatch, retention, Asaas disabled by default, settlement, recurring cycles, refunds, replay and cancellation CAS.")
finally:
    run("drop database " + DATABASE + ";","postgres")
