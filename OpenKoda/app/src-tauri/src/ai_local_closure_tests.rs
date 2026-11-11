//! No real provider: ownership decisions retain the original unknown response.
use super::*;
fn queued_unknown(f:&Fixture)->Value{let(plan,_,_)=f.queued();let(token,grant)=f.next();assert_eq!(f.ai.run_queued_investigation(&f.store,token,grant).unwrap().termination(),EffectTermination::Unknown);plan}
fn prepare(f:&Fixture,target:Value)->Value{f.call("work.aiLocalClosure.prepare",json!({"target":target})).unwrap()}
fn confirmation(preview:&Value)->Value{json!({"review":preview["review"],"idempotencyKey":model::id(),"note":"Close local ownership while remote result and cost remain unknown","acknowledgeRemoteUnknown":true,"acknowledgeNoReplay":true,"acknowledgeQueueInvalidation":true})}
fn target(f:&Fixture)->Value{json!({"kind":"attempt","attemptId":f.view()["attempts"][0]["id"]})}
fn journal(f:&Fixture,id:&Value)->String{f.ai.db.lock().unwrap().query_row("SELECT data FROM investigations WHERE id=?1",[id.as_str().unwrap()],|row|row.get(0)).unwrap()}
fn record_count(f:&Fixture)->u64{f.store.work_read(|db,_|db.query_row("SELECT count(*) FROM work_ai_dispositions",[],|row|row.get(0)).map_err(failure)).unwrap()}
fn fill_ordinary_metadata(f:&Fixture){
    f.store.work_transaction(|conn,_|{
        use crate::{workflow::*,work_store};
        let key:String=conn.query_row("SELECT idempotency_key FROM work_commands WHERE action='work.plan.commit' AND assessment_id=?1 LIMIT 1",[&f.aid],|row|row.get(0)).map_err(storage_error)?;let source=work_store::command_by_key(conn,&f.aid,&key)?.unwrap();let work_store::CommandResult::Completed{receipt,output}=source.result else{unreachable!()};let(mut actual,reserved)=work_store::metadata_usage(conn)?;
        loop{let mut receipt=receipt.clone();receipt.id=model::id();let command=work_store::StoredCommand{assessment_id:f.aid.clone(),idempotency_key:model::id(),review_id:None,result:work_store::CommandResult::Completed{receipt,output:output.clone()}};let size=encode(&command.result)?.len() as u64;if actual+reserved+size>MAX_METADATA_BYTES as u64{break;}work_store::insert_command(conn,&command)?;actual+=size;}
        assert!(MAX_METADATA_BYTES as u64-actual-reserved<1024);Ok(())
    }).unwrap();
}

#[test]
fn queued_local_closure_preserves_remote_unknown_and_money_and_never_releases_descendants(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);let plan=queued_unknown(&f);let before=journal(&f,&plan["id"]);let charges=f.charges();let requests=f.requests();
    let list=f.call("work.aiLocalClosure.list",json!({})).unwrap();assert_eq!(list["candidates"][0]["eligible"],true);
    let preview=prepare(&f,target(&f));assert_eq!(preview["unknownRequests"],1);assert_eq!(record_count(&f),0);assert_eq!(f.leases(),1);
    let input=confirmation(&preview);let result=f.call("work.aiLocalClosure.commit",input.clone()).unwrap();assert_eq!(result["value"]["remoteOutcomeUnknown"],true);assert_eq!(result["value"]["appliesToCurrentProfile"],true);
    assert_eq!(f.leases(),0);assert_eq!(f.requests(),requests);assert_eq!(f.charges(),charges);assert_eq!(journal(&f,&plan["id"]),before);assert!(!work::has_unresolved_work(&f.store.root).unwrap());
    let view=f.view();assert_eq!(view["control"],"paused");assert_eq!(view["jobs"][0]["state"],"closed_locally");assert_eq!(view["attempts"][0]["cleanup"],"unknown");assert_eq!(view["jobs"][0]["canRetry"],false);assert_ne!(view["jobs"][1]["state"],"awaiting_preparation");assert_eq!(view["outcome"],"unknown");
    assert!(f.supervisor.next_dispatch(&f.store,&f.service).unwrap().is_none());assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
    let replay=f.call("work.aiLocalClosure.commit",input.clone()).unwrap();assert_eq!(replay["replayed"],true);assert_eq!(result["receipt"],replay["receipt"]);assert_eq!(record_count(&f),1);
    let mut changed=input.clone();changed["note"]=json!("A different decision cannot reuse this command key");assert!(f.call("work.aiLocalClosure.commit",changed).is_err());
    let f=f.restart();assert!(!work::has_unresolved_work(&f.store.root).unwrap());assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.call("work.aiLocalClosure.commit",input).unwrap()["receipt"],result["receipt"]);
    assert_eq!(f.ai.handle("ai.investigation.list",json!({"assessmentId":f.aid}),&f.store).unwrap()["items"][0]["localOwnershipClosed"],true);
}

#[test]
fn local_closure_rolls_back_receipt_attempt_and_lease_together_after_commit_fault(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);let plan=queued_unknown(&f);let preview=prepare(&f,target(&f));let input=confirmation(&preview);let before=journal(&f,&plan["id"]);let charges=f.charges();
    f.core_sql("CREATE TEMP TRIGGER closure_delete_fault BEFORE DELETE ON work_leases BEGIN SELECT RAISE(ABORT,'synthetic closure commit interruption'); END;");
    assert!(f.call("work.aiLocalClosure.commit",input.clone()).is_err());assert_eq!(record_count(&f),0);assert_eq!(f.leases(),1);assert_eq!(f.view()["attempts"][0]["state"],"unknown");assert_eq!(f.view()["control"],"active");assert_eq!(journal(&f,&plan["id"]),before);assert_eq!(f.charges(),charges);
    f.core_sql("DROP TRIGGER closure_delete_fault;");let result=f.call("work.aiLocalClosure.commit",input).unwrap();assert_eq!(result["replayed"],false);assert_eq!(record_count(&f),1);assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
}

#[test]
fn local_closure_requires_current_review_and_each_specific_confirmation_not_old_acknowledgment(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);let plan=queued_unknown(&f);let stored=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();
    f.ai.handle("ai.request.reconcile",json!({"assessmentId":f.aid,"requestId":stored.results[0].request_id,"note":"The remote result remains unknown","acknowledgeUnknownUsage":true}),&f.store).unwrap();
    let preview=prepare(&f,target(&f));let input=confirmation(&preview);
    for flag in ["acknowledgeRemoteUnknown","acknowledgeNoReplay","acknowledgeQueueInvalidation"]{let mut rejected=input.clone();rejected[flag]=json!(false);assert!(f.call("work.aiLocalClosure.commit",rejected).is_err());}
    let mut forged=input.clone();forged["review"]["digest"]=json!(digest("forged proof"));assert!(f.call("work.aiLocalClosure.commit",forged).is_err());
    let guard=f.ai.inference.lock().unwrap();assert!(f.call("work.aiLocalClosure.commit",input.clone()).is_err());drop(guard);
    let view=f.view();f.call("work.pause",json!({"planId":view["id"],"expectedGeneration":view["generation"],"idempotencyKey":model::id(),"acknowledgePause":true})).unwrap();assert!(f.call("work.aiLocalClosure.commit",input).is_err());
    assert_eq!(record_count(&f),0);assert_eq!(f.leases(),1);assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
}

#[test]
fn local_closure_invalidates_old_grants_and_prepared_investigations_but_new_review_can_be_prepared(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);let old_plan=f.plan();queued_unknown(&f);
    let inspection=f.ai.local_closure_reviews.lock().unwrap();assert!(inspection.is_empty());drop(inspection);
    let preview=prepare(&f,target(&f));f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();
    assert!(f.ai.handle("ai.investigation.run",json!({"id":old_plan["id"],"digest":old_plan["digest"],"allowRemote":true}),&f.store).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
    // Same authorized scope may be prepared again, but no prior DataReview or
    // workflow grant survives this decision and nothing dispatches by itself.
    assert!(f.plan()["id"].is_string());assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert!(f.supervisor.next_dispatch(&f.store,&f.service).unwrap().is_none());
}

#[test]
fn local_closure_fits_reserved_metadata_when_unrelated_commands_fill_ordinary_quota(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);queued_unknown(&f);
    fill_ordinary_metadata(&f);
    let preview=prepare(&f,target(&f));f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();assert_eq!(record_count(&f),1);assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
}

struct DirectFake{calls:AtomicUsize}
impl Provider for DirectFake{fn response(&self,_:ProviderId,_:&str,key:&str,_:&str,_:&str,_:u32)->Result<Value,String>{assert_eq!(key,KEY);self.calls.fetch_add(1,Ordering::SeqCst);Err("Falha de conexão: synthetic direct timeout".into())}}

#[test]
fn direct_connection_test_closure_uses_nullable_assessment_and_keeps_financial_rows(){
    let mut f=Fixture::new(vec![]);let fake=Arc::new(DirectFake{calls:AtomicUsize::new(0)});f.ai=AiService::with_provider(f.store.root.clone(),fake.clone()).unwrap().with_arbiter(f.supervisor.shared_arbiter());
    assert!(f.ai.handle("ai.test",json!({}),&f.store).is_err());assert_eq!(fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.leases(),1);
    let id:String=f.ai.db.lock().unwrap().query_row("SELECT id FROM requests WHERE assessment='connection-test'",[],|row|row.get(0)).unwrap();let preview=prepare(&f,json!({"kind":"ai_request","requestId":id}));assert_eq!(preview["assessmentId"],Value::Null);let charges=f.charges();
    let result=f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();assert_eq!(result["value"]["assessmentId"],Value::Null);assert_eq!(f.charges(),charges);assert_eq!(f.leases(),0);assert_eq!(fake.calls.load(Ordering::SeqCst),1);assert!(!work::has_unresolved_work(&f.store.root).unwrap());
    let status:String=f.ai.db.lock().unwrap().query_row("SELECT status FROM requests WHERE id=?1",[id],|row|row.get(0)).unwrap();assert_eq!(status,"unknown");
}

#[test]
fn local_closure_rejects_terminal_provider_status_even_when_usage_is_unknown(){
    for queued in [false,true]{
        let mut f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);
        let target=if queued{queued_unknown(&f);target(&f)}else{f.ai=AiService::with_provider(f.store.root.clone(),Arc::new(DirectFake{calls:AtomicUsize::new(0)})).unwrap().with_arbiter(f.supervisor.shared_arbiter());assert!(f.ai.handle("ai.test",json!({}),&f.store).is_err());let id:String=f.ai.db.lock().unwrap().query_row("SELECT id FROM requests",[],|r|r.get(0)).unwrap();json!({"kind":"ai_request","requestId":id})};
        let charges=f.charges();
        // Synthetic persisted terminal responses with missing accounting are
        // eligible only for known-termination recovery, never local closure.
        for terminal in ["completed","incomplete","failed","cancelled"]{
            f.ai.db.lock().unwrap().execute("UPDATE requests SET details=json_set(details,'$.providerStatus',?1)",[terminal]).unwrap();
            assert!(f.call("work.aiLocalClosure.prepare",json!({"target":target})).is_err());
            assert_eq!(record_count(&f),0);assert_eq!(f.leases(),1);assert_eq!(f.charges(),charges);
        }
    }
}

#[test]
fn local_closure_cannot_race_a_provider_worker_or_invent_its_termination(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);f.queued();let(token,grant)=f.next();
    let(started,ready)=std::sync::mpsc::channel();let(release,wait)=std::sync::mpsc::channel();*f.fake.block.lock().unwrap()=Some((started,wait));
    std::thread::scope(|threads|{
        let task=threads.spawn(||f.ai.run_queued_investigation(&f.store,token,grant));ready.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(f.call("work.aiLocalClosure.prepare",json!({"target":target(&f)})).is_err());assert_eq!(record_count(&f),0);assert_eq!(f.leases(),1);
        release.send(()).unwrap();assert_eq!(task.join().unwrap().unwrap().termination(),EffectTermination::Unknown);
    });
    let preview=prepare(&f,target(&f));f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
}

#[test]
fn local_closure_read_only_gate_rejects_rehashed_invalid_markers_and_ownership(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);queued_unknown(&f);let preview=prepare(&f,target(&f));f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();
    let original=f.store.work_read(|db,_|Ok(crate::work_disposition::records(db)?.remove(0))).unwrap();
    let db=Connection::open(f.store.root.join("openkoda.sqlite3")).unwrap();
    for mutation in ["marker","schema","owner","target","column"]{
        let mut damaged=original.clone();
        match mutation{
            "marker"=>damaged.remote_outcome_unknown=false,"schema"=>damaged.schema="unsupported-disposition".into(),
            "owner"=>damaged.candidate.lease.as_mut().unwrap().owner.id=model::id(),
            "target"=>damaged.candidate.target=crate::work_disposition::AiLocalClosureTarget::AiRequest{request_id:model::id()},_=>{},
        }
        damaged.digest=crate::work_disposition::record_digest(&damaged).unwrap();
        let review=if mutation=="column"{model::id()}else{original.review.id.clone()};
        db.execute("UPDATE work_ai_dispositions SET data=?1,review_id=?2 WHERE id=?3",params![serde_json::to_string(&damaged).unwrap(),review,original.id]).unwrap();
        assert!(work::has_unresolved_work(&f.store.root).is_err(),"{mutation}");
        db.execute("UPDATE work_ai_dispositions SET data=?1,review_id=?2 WHERE id=?3",params![serde_json::to_string(&original).unwrap(),original.review.id,original.id]).unwrap();
        assert!(!work::has_unresolved_work(&f.store.root).unwrap());
    }
    assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
}

#[test]
fn closing_one_legacy_root_does_not_hide_another_unknown_root(){
    let f=Fixture::new(vec![]);let first=model::id();let second=model::id();
    for id in [&first,&second]{f.ai.db.lock().unwrap().execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created,details) VALUES(?1,'connection-test',?2,'fixture-model','synthetic-revision',?3,'unknown',?4,'{}')",params![id,ENDPOINT,digest("synthetic legacy context"),model::now()]).unwrap();}
    assert!(work::has_unresolved_work(&f.store.root).unwrap());
    let preview=prepare(&f,json!({"kind":"ai_request","requestId":first}));f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();assert!(work::has_unresolved_work(&f.store.root).unwrap());
    let preview=prepare(&f,json!({"kind":"ai_request","requestId":second}));f.call("work.aiLocalClosure.commit",confirmation(&preview)).unwrap();assert!(!work::has_unresolved_work(&f.store.root).unwrap());
    assert_eq!(record_count(&f),2);assert_eq!(f.requests(),2);assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
}

#[test]
fn local_closure_restored_into_another_profile_is_historical_and_cannot_grant_new_work(){
    let f=Fixture::new(vec![Err("Falha de conexão: synthetic timeout".into())]);let plan=queued_unknown(&f);let preview=prepare(&f,target(&f));let input=confirmation(&preview);let receipt=f.call("work.aiLocalClosure.commit",input.clone()).unwrap();
    let charges=f.charges();let before=journal(&f,&plan["id"]);let target=tempfile::tempdir().unwrap();
    // Exercise the actual SQLite backup and restore-deactivation helpers.
    // The archive manifest/UI is outside this backend fixture's scope.
    crate::recovery::backup_database(&f.store.root.join("openkoda.sqlite3"),&target.path().join("openkoda.sqlite3")).unwrap();
    crate::recovery::backup_assistant(&f.store.root.join("assistant.sqlite3"),&target.path().join("assistant.sqlite3")).unwrap();
    let store=Arc::new(Store::open(target.path().into()).unwrap());crate::work_supervisor::deactivate_after_restore(&store).unwrap();
    let supervisor=WorkSupervisor::default();let fake=Arc::new(Fake{store:store.clone(),calls:AtomicUsize::new(0),answers:Mutex::new(VecDeque::new()),block:Mutex::new(None)});
    // A restored profile uses the installation's existing ledger; the backup
    // must not copy/reset financial rows or create a new spending epoch.
    let ai=AiService::open_profile(store.root.clone(),fake.clone(),budget::Ledger::open(&f.store.root)).unwrap().with_arbiter(supervisor.shared_arbiter());
    let list=ai.handle_local_closure("work.aiLocalClosure.list",json!({}),&store,||Ok(())).unwrap();assert_eq!(list["items"][0]["appliesToCurrentProfile"],false);assert!(work::has_unresolved_work(&store.root).unwrap());
    let listed=ai.handle("ai.investigation.list",json!({"assessmentId":f.aid}),&store).unwrap();assert_eq!(listed["items"][0]["localOwnershipClosed"],false);
    let service=crate::workflow::WorkflowService::default();assert!(supervisor.next_dispatch(&store,&service).unwrap().is_none());
    let closed=store.work_read(|db,s|Ok(crate::work_store::plan_view(db,s,&f.aid,None)?.unwrap())).unwrap();assert_eq!(closed.control,crate::workflow::PlanControl::Paused);assert_eq!(closed.jobs[0].state,crate::workflow::JobState::ClosedLocally);assert!(!closed.jobs[0].can_retry);
    ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();
    let replay=ai.handle_local_closure("work.aiLocalClosure.commit",input,&store,||panic!("replay cannot invalidate current reviews")).unwrap();assert_eq!(replay["receipt"],receipt["receipt"]);assert_eq!(replay["value"]["appliesToCurrentProfile"],false);
    assert!(ai.handle("ai.test",json!({}),&store).is_err());assert_eq!(fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
    assert_eq!(ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&store).unwrap()["recentCharges"],charges);assert_eq!(f.charges(),charges);assert_eq!(journal(&f,&plan["id"]),before);assert!(!work::has_unresolved_work(&f.store.root).unwrap());
}

#[test]
fn direct_closure_fits_its_reserved_envelope_with_one_hundred_prepared_jobs_at_full_quota(){
    let mut f=Fixture::new(vec![]);let project=f.store.snapshot().unwrap().projects[0].clone();
    for group in 0..4{
        let aid=if group==0{f.aid.clone()}else{
            let created=f.store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":project.id,"title":format!("Prepared queue {group}"),"kind":"web","target":project.target,"objective":"Metadata preparation under quota","scope":scope::test_creation(&f.store,&project.id,&[AI_METHOD])})}).unwrap();
            let aid=created["id"].as_str().unwrap().to_owned();let evidence=f.store.snapshot().unwrap().assessments.iter().find(|a|a.id==f.aid).unwrap().evidence[0].clone();f.store.mutate(|s|{s.assessments.iter_mut().find(|a|a.id==aid).unwrap().evidence.push(model::Evidence{id:model::id(),..evidence});Ok(())}).unwrap();aid
        };
        let evidence=f.store.snapshot().unwrap().assessments.iter().find(|a|a.id==aid).unwrap().evidence[0].clone();
        let nodes=(0..25).map(|index|json!({"key":format!("ai-{index}"),"existingNodeId":null,"purpose":"Prepare an explicit future independent review","expectedObservation":"No provider call during preparation","dependsOnKeys":[],"intent":{"kind":"ai_investigation","inputs":[{"kind":"evidence","evidenceId":evidence.id,"expectedRawSha256":evidence.sha256}],"perspectives":["evidence","counterevidence"]}})).collect::<Vec<_>>();
        let preview=f.call("work.plan.prepare",json!({"assessmentId":aid,"expectedHead":null,"expectedGeneration":0,"goal":"Prepare bounded independent work","nodes":nodes,"limits":{"maxAttemptsTotal":100,"maxCaptureBytesTotal":1024}})).unwrap();
        let committed=f.call("work.plan.commit",json!({"review":preview["review"],"idempotencyKey":model::id(),"acknowledgePlan":true})).unwrap();
        for job in committed["value"]["jobs"].as_array().unwrap(){let prepared=f.call("work.operation.prepare",json!({"job":{"id":job["id"],"generation":job["generation"]},"revision":committed["value"]["head"],"idempotencyKey":model::id()})).unwrap();assert_eq!(prepared["value"]["job"]["state"],"awaiting_approval");}
    }
    // Exercise the extra digit in every invalidated job's generation without
    // adding a compiled payload whose removal would artificially free space.
    f.store.work_transaction(|db,_|{let mut q=db.prepare("SELECT plan_id FROM work_plan_heads").map_err(failure)?;let plans=q.query_map([],|r|r.get::<_,String>(0)).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?;drop(q);for plan in plans{for mut job in crate::work_store::jobs_for_plan(db,&plan)?{assert!(job.compiled.is_none());job.generation=9;crate::work_store::save_job(db,&job)?;}}Ok(())}).unwrap();
    let fake=Arc::new(DirectFake{calls:AtomicUsize::new(0)});f.ai=AiService::with_provider(f.store.root.clone(),fake.clone()).unwrap().with_arbiter(f.supervisor.shared_arbiter());assert!(f.ai.handle("ai.test",json!({}),&f.store).is_err());assert_eq!(fake.calls.load(Ordering::SeqCst),1);
    let request:String=f.ai.db.lock().unwrap().query_row("SELECT id FROM requests",[],|r|r.get(0)).unwrap();fill_ordinary_metadata(&f);
    let preview=prepare(&f,json!({"kind":"ai_request","requestId":request}));assert_eq!(preview["queue"]["preparedJobs"],100);assert_eq!(preview["queue"]["activePlans"],4);assert_eq!(preview["queue"]["reservedAttempts"],0);
    let mut input=confirmation(&preview);input["note"]=json!("🧭".repeat(256));f.call("work.aiLocalClosure.commit",input).unwrap();
    f.store.work_read(|db,_|{let count:u64=db.query_row("SELECT count(*) FROM work_jobs WHERE state='awaiting_preparation' AND generation=10",[],|r|r.get(0)).map_err(failure)?;assert_eq!(count,100);let active:u64=db.query_row("SELECT count(*) FROM work_plan_heads WHERE control='active'",[],|r|r.get(0)).map_err(failure)?;assert_eq!(active,0);crate::work_store::enforce_caps(db)}).unwrap();
    assert_eq!(record_count(&f),1);assert_eq!(f.leases(),0);assert_eq!(fake.calls.load(Ordering::SeqCst),1);assert!(f.supervisor.next_dispatch(&f.store,&f.service).unwrap().is_none());
}
