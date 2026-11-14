"""Queue and billing invariants, exercised against the current PostgreSQL fixtures."""
import json
import uuid


def exercise_capacity(run, rpc, race, denied, model, digest):
    def uid():
        return str(uuid.uuid4())

    people = [uid() for _ in range(68)]
    ids = [uid() for _ in people]
    by_request = {request_id: i for i, request_id in enumerate(ids)}
    actors = ",".join(f"('{p}','queue-{i}@example.invalid',now())" for i,p in enumerate(people))
    selected = ",".join(f"'{p}'" for p in people)
    run(f"insert into auth.users values {actors}")
    run(f"""insert into openkoda_private.accounts(user_id,terms_version,privacy_version,accepted_at)
        select id,'2026-11-13','2026-11-13',now() from auth.users where id in ({selected});
        insert into openkoda_private.subscriptions(user_id,plan,period_start,period_end)
        select id, 'plus', now()-interval '1 day',now()+interval '29 days' from auth.users where id in ({selected});
        update openkoda_private.controls set inference_enabled=true,max_parallel=4,user_parallel=1,
        user_per_minute=8,ip_per_minute=24,daily_limit=10000000,monthly_limit=20000000;""")

    def admit(i, token=None, request_id=None, payload=digest):
        return rpc("ai_admit",people[i],request_id or ids[i],payload,model,"chat",f"{i:064x}",30,token)

    def hold(i, token=None):
        assert admit(i,token)["status"] == "reserved"
        return rpc("ai_dispatch",people[i],ids[i])["dispatch"]

    def release(i):
        return rpc("ai_settle",people[i],ids[i],"completed",6,15,5,"gen-queue-"+ids[i])

    def waiting_count():
        return int(run("select count(*) from openkoda_private.capacity_waiters"))

    def ledger_counts():
        return json.loads(run(f"""select json_build_array(count(*),count(*) filter(where status='dispatched'),
            coalesce(sum(liability),0)) from openkoda_private.requests where user_id in ({selected})"""))

    initial = race([lambda i=i: admit(i) for i in range(30)])
    assert all(isinstance(r,dict) for r in initial), initial
    active = [i for i,r in enumerate(initial) if r["status"]=="reserved"]
    waiting = [i for i,r in enumerate(initial) if r["status"]=="waiting"]
    assert len(active)==4 and len(waiting)==26, (len(active),len(waiting),ledger_counts())
    assert all(rpc("ai_dispatch",people[i],ids[i])["dispatch"] for i in active)
    assert ledger_counts() == [4,4,120]
    ordered = [by_request[r] for r in run("select request_id from openkoda_private.capacity_waiters order by sequence").splitlines()]
    tokens = {i: initial[i]["queueToken"] for i in waiting}
    assert sorted(initial[i]["position"] for i in waiting)==list(range(1,27))
    first,second,third,fourth = ordered[:4]
    # This section visits many independent branches using one Windows docker
    # process per SQL call. Keep its fixtures alive while exercising ordering;
    # expiry is tested explicitly below, without depending on host load.
    run("update openkoda_private.capacity_waiters set lease_until=expires_at")
    denied("idempotency_conflict",admit,first,tokens[first],None,"c"*64)
    denied("queue_expired",admit,first,uid())
    denied("idempotency_conflict",admit,second,tokens[first],ids[first])
    denied("queue_user_limit",admit,first,None,uid())
    assert not rpc("cancel_wait",people[second],ids[first],tokens[first])["cancelled"]
    assert not rpc("cancel_wait",people[first],ids[first],uid())["cancelled"]
    assert rpc("cancel_wait",people[first],ids[first],tokens[first])["cancelled"]
    denied("queue_expired",admit,first,tokens[first])
    # The oldest disconnected client loses its place. The next user can claim it.
    run(f"update openkoda_private.capacity_waiters set lease_until=now()-interval '1 second' where request_id='{ids[second]}'")
    denied("queue_expired",admit,second,tokens[second])
    assert release(active[0])["status"]=="completed"
    assert admit(fourth,tokens[fourth])["status"]=="waiting"
    competing = race([lambda:admit(third,tokens[third])]*2)
    assert sorted(r["replay"] for r in competing)==[False,True]
    assert sorted(r["dispatch"] for r in race([lambda:rpc("ai_dispatch",people[third],ids[third])]*2))==[False,True]
    # A user already using their slot may wait, without holding back other people.
    next_id=uid()
    assert admit(third,request_id=next_id)["status"]=="waiting"
    # A switch transition invalidates all remaining tickets, including a fast re-enable.
    run("update openkoda_private.controls set inference_enabled=false; update openkoda_private.controls set inference_enabled=true;")
    assert not waiting_count()
    denied("queue_expired",admit,fourth,tokens[fourth])
    for i in [*active[1:],third]:
        release(i)
    assert ledger_counts()==[5,0,30]

    # Polling account 0 cannot claim a second seat while it is busy.
    ids[:]=[uid() for _ in people]
    run("update openkoda_private.controls set max_parallel=2;")
    assert hold(0) and hold(1)
    busy_wait=admit(0,request_id=uid())
    next_wait=admit(2)
    assert busy_wait["position"]==1 and next_wait["position"]==1
    release(1)
    assert hold(2,next_wait["queueToken"])
    assert rpc("cancel_wait",people[0],busy_wait["requestId"],busy_wait["queueToken"])["cancelled"]
    release(0);release(2)
    assert ledger_counts()==[8,0,48]

    # Fill the entire 64-entry queue. Use one SQL session for fixture setup so
    # that Windows Docker startup latency doesn't expire legitimate 15s leases.
    run("update openkoda_private.controls set max_parallel=1;")
    ids[:]=[uid() for _ in people]
    assert hold(0)
    def sql_admit(i):
        return f"public.openkoda_ai_admit('{people[i]}','{ids[i]}','{digest}','{model}','chat','{i:064x}',30,null)"
    for_row=",".join(sql_admit(i) for i in range(1,65))
    run(f"set role service_role; select array[{for_row}];")
    assert waiting_count()==64
    denied("queue_full",admit,65)
    first_token=run(f"select token from openkoda_private.capacity_waiters where request_id='{ids[1]}'")
    before=run(f"select expires_at from openkoda_private.capacity_waiters where request_id='{ids[1]}'")
    assert admit(1,first_token)["queueToken"]==first_token
    assert run(f"select expires_at from openkoda_private.capacity_waiters where request_id='{ids[1]}'")==before
    run(f"update openkoda_private.capacity_waiters set expires_at=now()-interval '1 second',lease_until=now()-interval '1 second' where request_id='{ids[1]}'")
    denied("queue_expired",admit,1,first_token)
    assert admit(65)["status"]=="waiting" and waiting_count()==64
    # On loss of downstream receipts, release the slot and retain the money.
    run(f"update openkoda_private.requests set expires_at=now()-interval '1 second' where id='{ids[0]}';"
        "select openkoda_private.retention();")
    assert run(f"select status||'/'||liability from openkoda_private.requests where id='{ids[0]}'")=="unknown/30"
    assert ledger_counts()==[9,0,78]
    run("update openkoda_private.capacity_waiters set lease_until=now()-interval '1 second'; select openkoda_private.retention();")
    assert not waiting_count()
    # Restore the initial state of this throwaway database for the existing tests.
    run(f"""delete from openkoda_private.requests where user_id in ({selected});
        delete from auth.users where id in ({selected});
        update openkoda_private.controls set inference_enabled=false,max_parallel=10,user_parallel=1,
          user_per_minute=8,ip_per_minute=24,daily_limit=0,monthly_limit=0;""")
    print("PASS: 30 accounts raced for 4 slots; 26 waited; 64 waiting accounts enforced; "
          "FIFO, disconnect, cancellation, expiry, replay, atomic claim, kill switch, and crash recovery.")
