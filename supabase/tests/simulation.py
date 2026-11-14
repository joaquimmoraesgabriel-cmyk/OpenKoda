"""Internal-plan acceptance checks against the real isolated PostgreSQL fixture."""
import json
import uuid


def exercise_simulation(run, rpc, denied, race, user, other):
    request = str(uuid.uuid4())
    denied("simulation_not_allowed", rpc, "simulation", user, "plus", request, "activate")
    run(f"insert into openkoda_private.simulation_testers values('{user}',now()+interval '1 day');")
    denied("simulation_not_allowed", rpc, "simulation", user, "plus", request, "activate")
    run("update openkoda_private.controls set simulation_enabled=true;")
    denied("simulation_not_allowed", rpc, "simulation", other, "plus", request, "activate")
    before = run("select count(*) from openkoda_private.subscriptions;")
    results = race([lambda: rpc("simulation", user, "plus", request, "activate") for _ in range(12)])
    assert sum(not result["replay"] for result in results) == 1, results
    assert all(result["status"] == "active" and result["grantsProductionCredit"] is False for result in results)
    assert run(f"select count(*) from openkoda_private.simulation_events where user_id='{user}';") == "1"
    denied("idempotency_conflict", rpc, "simulation", user, "pro", request, "activate")
    snapshot = rpc("account", user)
    assert snapshot["plan"] is None and snapshot["usage"] is None
    assert snapshot["simulation"]["plan"] == "plus"
    assert snapshot["entitlements"] == {"reasoning": True, "skills": True, "hostedInference": False}
    assert len(rpc("skills", user)["skills"]) == 39
    assert rpc("account", other)["simulation"] is None
    assert rpc("account", other)["controls"]["simulationEnabled"] is False
    assert rpc("simulation", user, "pro", str(uuid.uuid4()), "fail")["status"] == "failed"
    assert rpc("account", user)["simulation"]["plan"] == "plus"
    assert rpc("simulation", user, "plus", str(uuid.uuid4()), "cancel")["status"] == "canceled"
    assert rpc("account", user)["entitlements"]["reasoning"] is False
    denied("subscription_required", rpc, "skills", user)
    rpc("simulation", user, "ultra", str(uuid.uuid4()), "activate")
    assert rpc("simulation", user, "ultra", str(uuid.uuid4()), "expire")["status"] == "expired"
    assert rpc("account", user)["entitlements"]["skills"] is False
    rpc("simulation", user, "plus", str(uuid.uuid4()), "activate")
    run(f"update openkoda_private.simulations set period_start=now()-interval '8 days',period_end=now()-interval '1 day' where user_id='{user}';")
    assert rpc("account", user)["simulation"]["status"] == "expired"
    assert rpc("account", user)["entitlements"]["reasoning"] is False
    rpc("simulation", user, "plus", str(uuid.uuid4()), "activate")
    run(f"update openkoda_private.simulation_testers set expires_at=now()-interval '1 second' where user_id='{user}';")
    assert rpc("account", user)["simulation"] is None
    assert rpc("account", user)["entitlements"]["reasoning"] is False
    assert run("select count(*) from openkoda_private.subscriptions;") == before
    assert len(rpc("export", user)["simulationEvents"]) == 7
    run("update openkoda_private.controls set simulation_enabled=false;")
    print(json.dumps({"internalSimulation": "passed", "parallelIdempotency": 12, "productionCreditGranted": False}))
