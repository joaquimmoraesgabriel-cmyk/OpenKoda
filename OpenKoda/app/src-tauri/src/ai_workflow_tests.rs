//! Real policy/journal fixtures with a provider that can only return queued fakes.
use super::*;
use std::{collections::VecDeque,sync::atomic::{AtomicUsize,Ordering}};
use crate::work_supervisor::{WorkSupervisor,QueuedDispatch};
#[path="ai_local_closure_tests.rs"]
mod local_closure;

const KEY:&str="synthetic-workflow-provider-key-not-real";
struct Fake {store:Arc<Store>,calls:AtomicUsize,answers:Mutex<VecDeque<Result<Value,String>>>,block:Mutex<Option<(std::sync::mpsc::Sender<()>,std::sync::mpsc::Receiver<()>)>>}
impl Provider for Fake {
    fn response(&self,_:ProviderId,_:&str,key:&str,_:&str,_:&str,_:u32)->Result<Value,String>{
        assert_eq!(key,KEY);self.calls.fetch_add(1,Ordering::SeqCst);
        let bound:u64=self.store.work_read(|db,_|db.query_row("SELECT count(*) FROM work_leases WHERE state='bound' AND effect_kind='ai_investigation'",[],|r|r.get(0)).map_err(failure)).unwrap();assert_eq!(bound,1);
        if let Some((started,wait))=self.block.lock().unwrap().take(){started.send(()).unwrap();wait.recv_timeout(Duration::from_secs(10)).unwrap();}
        self.answers.lock().unwrap().pop_front().expect("unplanned fake provider call")
    }
}
fn answer()->Result<Value,String>{Ok(json!({"id":model::id(),"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Synthetic independent observation"}]}],"usage":{"input_tokens":7,"output_tokens":9}}))}
struct Fixture {
    _dir:tempfile::TempDir,store:Arc<Store>,ai:AiService,fake:Arc<Fake>,
    supervisor:WorkSupervisor,service:crate::workflow::WorkflowService,executor:crate::executor::Executor,kali:crate::kali_bridge::KaliBridge,aid:String,eid:String,
}
impl Fixture {
    fn new(answers:Vec<Result<Value,String>>)->Self{
        let dir=tempfile::tempdir().unwrap();let store=Arc::new(Store::open(dir.path().into()).unwrap());
        let fake=Arc::new(Fake{store:store.clone(),calls:AtomicUsize::new(0),answers:Mutex::new(answers.into()),block:Mutex::new(None)});
        let supervisor=WorkSupervisor::default();
        let ai=AiService::with_provider(store.root.clone(),fake.clone()).unwrap().with_arbiter(supervisor.shared_arbiter());
        ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
        let project=store.handle(crate::core::Request{action:"project.create".into(),payload:json!({"name":"Queued AI fixture","kind":"web","target":"https://never-dispatch.invalid/"})}).unwrap();
        let aid=store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Queued independent review","kind":"web","target":project["target"],"objective":"Review synthetic selected bytes","scope":scope::test_creation(&store,project["id"].as_str().unwrap(),&[AI_METHOD])})}).unwrap()["id"].as_str().unwrap().to_owned();
        let eid=model::id();let content="Synthetic workflow observation";
        store.mutate(|s|{s.assessments[0].evidence.push(model::Evidence{id:eid.clone(),name:"fixture.txt".into(),content:content.into(),sha256:digest(content),created_at:model::now()});Ok(())}).unwrap();
        ai.test_data_authority(&store,&aid);
        let executor=crate::executor::Executor::default().with_arbiter(supervisor.shared_arbiter());
        let kali=crate::kali_bridge::KaliBridge::new(store.root.clone()).unwrap();
        Self{_dir:dir,store,ai,fake,supervisor,service:crate::workflow::WorkflowService::default(),executor,kali,aid,eid}
    }
    fn plan(&self)->Value{
        let payload=self.ai.test_data_input(&self.store,"investigation",json!({"assessmentId":self.aid,"evidenceIds":[self.eid],"message":"Independent views of synthetic selected evidence","perspectives":["evidence","counterevidence"],"maxOutputTokensPerRequest":512,"maxTotalTokens":100000}));
        self.ai.handle("ai.investigation.plan",payload,&self.store).unwrap()
    }
    fn expected(&self)->WorkAiExpectation{
        let snapshot=self.store.snapshot().unwrap();let evidence=&snapshot.assessments[0].evidence[0];
        WorkAiExpectation{assessment_id:self.aid.clone(),scope_ref:scope::current_ref(&snapshot,&self.aid).unwrap().unwrap(),inputs:vec![ResultRef::Evidence{id:evidence.id.clone(),raw_sha256:evidence.sha256.clone()}],perspectives:vec![Perspective::Evidence,Perspective::Counterevidence]}
    }
    fn inspect(&self,plan:&Value)->WorkAiInspection{self.ai.inspect_work_investigation(&self.store,plan["id"].as_str().unwrap(),plan["digest"].as_str().unwrap(),&self.expected()).unwrap()}
    fn requests(&self)->u64{self.ai.db.lock().unwrap().query_row("SELECT count(*) FROM requests",[],|r|r.get(0)).unwrap()}
    fn leases(&self)->u64{self.store.work_read(|db,_|db.query_row("SELECT count(*) FROM work_leases",[],|r|r.get(0)).map_err(failure)).unwrap()}
    fn call(&self,action:&str,payload:Value)->Result<Value,String>{
        let redactor=self.ai.data_redactor();
        if matches!(action,"work.plan.prepare"|"work.plan.commit"|"work.plan.get"|"work.queue"|"work.events"){return self.service.handle(action,payload,&self.store,&redactor);}
        self.service.handle_backend(action,payload,&self.store,&self.executor,&self.supervisor,&self.ai,&self.kali,None,&||Err("unexpected identity lookup in AI fixture".into()),&redactor)
    }
    fn queued(&self)->(Value,Value,Value){
        let expected=self.expected();let input=&expected.inputs[0];let ResultRef::Evidence{id,raw_sha256}=input else{unreachable!()};
        let preview=self.call("work.plan.prepare",json!({"assessmentId":self.aid,"expectedHead":null,"expectedGeneration":0,"goal":"Review the selected observation independently","nodes":[
            {"key":"ai","existingNodeId":null,"purpose":"Inspect selected evidence","expectedObservation":"Independent bounded reviews","dependsOnKeys":[],"intent":{"kind":"ai_investigation","inputs":[{"kind":"evidence","evidenceId":id,"expectedRawSha256":raw_sha256}],"perspectives":["evidence","counterevidence"]}},
            {"key":"human","existingNodeId":null,"purpose":"Inspect the independent analyses","expectedObservation":"Explicit human decision","dependsOnKeys":["ai"],"intent":{"kind":"human_review","inputs":[{"kind":"dependency_output","nodeKey":"ai","outputRole":"investigation"}]}}
        ],"limits":{"maxAttemptsTotal":10,"maxCaptureBytesTotal":1024}})).unwrap();
        let committed=self.call("work.plan.commit",json!({"review":preview["review"],"idempotencyKey":model::id(),"acknowledgePlan":true})).unwrap();let revision=committed["value"]["head"].clone();let job=&committed["value"]["jobs"][0];
        let prepared=self.call("work.operation.prepare",json!({"job":{"id":job["id"],"generation":job["generation"]},"revision":revision,"idempotencyKey":model::id()})).unwrap();
        assert_eq!(prepared["value"]["kind"],"ai_investigation");let plan=self.plan();let job=&prepared["value"]["job"];
        let attached=self.call("work.operation.attachInvestigation",json!({"job":{"id":job["id"],"generation":job["generation"]},"revision":revision,"investigationId":plan["id"],"digest":plan["digest"],"idempotencyKey":model::id()})).unwrap();let job=&attached["value"]["job"];
        let request=json!({"kind":"ai_investigation","job":{"id":job["id"],"generation":job["generation"]},"revision":revision,"compiled":attached["value"]["compiled"],"dataReview":plan["dataReview"],"budgetReview":null,"allowRemote":true,"idempotencyKey":model::id()});
        let queued=self.call("work.operation.enqueue",request.clone()).unwrap();(plan,request,queued)
    }
    fn next(&self)->(QueuedAttemptToken,QueuedAiGrant){let Some(QueuedDispatch::Ai{token,grant})=self.supervisor.next_dispatch(&self.store,&self.service).unwrap()else{panic!("expected queued AI")};(token,grant)}
    fn view(&self)->Value{self.call("work.plan.get",json!({"assessmentId":self.aid,"revisionId":null})).unwrap()["plan"].clone()}
    fn recovery_request(&self)->Value{json!({"attemptId":self.view()["attempts"][0]["id"],"expectedState":"unknown","note":"Inspect persisted synthetic journals without repeating requests","idempotencyKey":model::id(),"acknowledgeUnknownEffects":true})}
    fn charges(&self)->Value{self.ai.handle("ai.budget.status",json!({"assessmentId":self.aid}),&self.store).unwrap()["recentCharges"].clone()}
    fn core_sql(&self,sql:&str){self.store.work_transaction(|db,_|db.execute_batch(sql).map_err(failure)).unwrap();}
    fn fault_completed_attempt(&self){self.core_sql("CREATE TEMP TRIGGER queued_ai_finish_fault BEFORE UPDATE ON work_attempts WHEN NEW.state='completed' BEGIN SELECT RAISE(ABORT,'synthetic final commit fault'); END;");}
    fn restart(self)->Self{
        let Self{_dir,store,ai,fake,supervisor,service,executor,kali,aid,eid}=self;
        let root=store.root.clone();
        // Fake and DataPolicy can retain Store Arcs. Close every owner before
        // reopening the installation lock; keep only the temporary directory.
        drop(ai);drop(fake);drop(kali);drop(executor);drop(service);drop(supervisor);drop(store);
        let store=Arc::new(Store::open(root.clone()).unwrap());let supervisor=WorkSupervisor::default();
        let fake=Arc::new(Fake{store:store.clone(),calls:AtomicUsize::new(0),answers:Mutex::new(VecDeque::new()),block:Mutex::new(None)});
        let ai=AiService::with_provider(root.clone(),fake.clone()).unwrap().with_arbiter(supervisor.shared_arbiter());
        supervisor.recover_after_boot(&store).unwrap();
        let executor=crate::executor::Executor::default().with_arbiter(supervisor.shared_arbiter());let kali=crate::kali_bridge::KaliBridge::new(root).unwrap();
        Self{_dir,store,ai,fake,supervisor,service:crate::workflow::WorkflowService::default(),executor,kali,aid,eid}
    }
}

#[test]
fn inspect_and_enqueue_preflight_are_pure_and_attach_holds_inference_without_db_lock(){
    let f=Fixture::new(vec![]);let plan=f.plan();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
    let inspection=f.ai.with_work_investigation(&f.store,plan["id"].as_str().unwrap(),plan["digest"].as_str().unwrap(),&f.expected(),|inspection|{
        assert!(f.ai.inference.try_lock().is_err());assert!(f.ai.db.try_lock().is_ok());
        f.store.work_read(|_,_|Ok(()))?;Ok(inspection.clone())
    }).unwrap();
    let grant=f.ai.validate_queued_investigation(&f.store,&inspection,inspection.data_review(),None,false).unwrap();
    assert_eq!(grant.inspection().id(),plan["id"].as_str().unwrap());
    assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert_eq!(f.requests(),0);assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    let budget=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert!(budget["recentCharges"].as_array().unwrap().is_empty());
    // A failed attach callback must leave the data review available for retry.
    assert!(f.ai.with_work_investigation(&f.store,inspection.id(),inspection.digest(),&f.expected(),|_|Err::<(),String>("synthetic core commit failure".into())).is_err());
    assert!(f.ai.validate_queued_investigation(&f.store,&inspection,inspection.data_review(),None,false).is_ok());
}

#[test]
fn inspection_rejects_cross_assessment_scope_input_hash_and_perspective_changes(){
    let f=Fixture::new(vec![]);let plan=f.plan();let original=f.expected();
    for change in ["assessment","scope","hash","evidence","perspective","duplicate"]{
        let mut expected=original.clone();match change{
            "assessment"=>expected.assessment_id=model::id(),"scope"=>expected.scope_ref.digest=digest("different scope"),
            "hash"=>if let ResultRef::Evidence{raw_sha256,..}=&mut expected.inputs[0]{*raw_sha256=digest("different evidence")},
            "evidence"=>expected.inputs.clear(),"duplicate"=>expected.inputs.push(expected.inputs[0].clone()),
            _=>expected.perspectives.reverse(),
        }
        assert!(f.ai.inspect_work_investigation(&f.store,plan["id"].as_str().unwrap(),plan["digest"].as_str().unwrap(),&expected).is_err(),"{change}");
    }
    assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
}

#[test]
fn stale_credential_and_wrong_data_review_fail_preflight_without_reservation(){
    let f=Fixture::new(vec![]);let plan=f.plan();let inspection=f.inspect(&plan);
    let mut wrong=inspection.data_review().clone();wrong.digest=digest("not this review");
    assert!(f.ai.validate_queued_investigation(&f.store,&inspection,&wrong,None,false).is_err());
    f.ai.handle("ai.key.save",json!({"key":"another-synthetic-workflow-key-not-real"}),&f.store).unwrap();
    assert!(f.ai.validate_queued_investigation(&f.store,&inspection,inspection.data_review(),None,false).is_err());
    assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
}

#[test]
fn known_terminal_without_usage_is_partial_but_remote_nonterminal_and_timeout_hold_lease(){
    for (response,terminal) in [
        (Ok(json!({"id":model::id(),"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Result with unknown accounting"}]}]})),true),
        (Ok(json!({"id":model::id(),"status":"in_progress"})),false),
        (Err("Falha de conexão com a OpenAI.".into()),false),
    ]{
        let f=Fixture::new(vec![response]);let plan=f.plan();
        let result=f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).unwrap();
        assert_eq!(result["status"],if terminal{"partial"}else{"unknown"});assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.requests(),1);assert_eq!(f.leases(),u64::from(!terminal));
        let budget=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(budget["unknownChargesTotal"],1);
    }
}

#[test]
fn queued_investigation_binds_parent_and_publishes_exact_derived_result_without_replay(){
    let f=Fixture::new(vec![answer(),answer()]);let(plan,request,queued)=f.queued();
    assert_eq!(f.requests(),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
    let replay=f.call("work.operation.enqueue",request.clone()).unwrap();assert_eq!(replay["receipt"],queued["receipt"]);assert_eq!(replay["replayed"],true);
    assert!(f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    let listed=f.ai.handle("ai.investigation.list",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(listed["items"][0]["workOwner"]["jobId"],queued["value"]["job"]["id"]);assert!(f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap().digest==plan["digest"].as_str().unwrap());
    let(token,grant)=f.next();let completion=f.ai.run_queued_investigation(&f.store,token,grant).unwrap();assert_eq!(completion.termination(),EffectTermination::Stopped);assert_eq!(completion.outcome(),AttemptState::Completed);assert_eq!(f.fake.calls.load(Ordering::SeqCst),2);assert_eq!(f.leases(),0);
    let view=f.view();assert_eq!(view["jobs"][0]["state"],"completed");assert_eq!(view["jobs"][1]["state"],"awaiting_preparation");assert_eq!(view["attempts"][0]["dataReceiptRefs"].as_array().unwrap().len(),3);
    let result=&view["jobs"][0]["results"][0];assert_eq!(result["kind"],"derived");assert_eq!(result["producer"],"investigation");assert_eq!(result["id"],plan["id"]);
    let stored=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();assert_eq!(result["fingerprint"],journal_fingerprint(&stored).unwrap());
    let replay=f.call("work.operation.enqueue",request).unwrap();assert_eq!(replay["receipt"],queued["receipt"]);assert!(f.supervisor.next_dispatch(&f.store,&f.service).unwrap().is_none());assert_eq!(f.fake.calls.load(Ordering::SeqCst),2);
}

#[test]
fn queued_rotation_cancels_before_admission_and_historical_ownership_still_blocks_direct(){
    let f=Fixture::new(vec![]);let(plan,_,_)=f.queued();
    f.ai.handle("ai.key.save",json!({"key":"rotated-synthetic-queued-provider-key"}),&f.store).unwrap();
    let(token,grant)=f.next();let completion=f.ai.run_queued_investigation(&f.store,token,grant).unwrap();assert_eq!(completion.termination(),EffectTermination::NotStarted);assert_eq!(f.leases(),0);assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
    assert!(f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).is_err());
    let budget=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert!(budget["recentCharges"].as_array().unwrap().is_empty());
}

#[test]
fn pause_or_cancel_during_first_queued_perspective_preserves_charge_and_prevents_second(){
    for pause in [true,false]{
        let f=Fixture::new(vec![answer()]);let(_,_,_)=f.queued();let(token,grant)=f.next();
        let(started,ready)=std::sync::mpsc::channel();let(release,wait)=std::sync::mpsc::channel();*f.fake.block.lock().unwrap()=Some((started,wait));
        std::thread::scope(|threads|{
            let task=threads.spawn(||f.ai.run_queued_investigation(&f.store,token,grant));ready.recv_timeout(Duration::from_secs(10)).unwrap();let view=f.view();
            if pause{f.call("work.pause",json!({"planId":view["id"],"expectedGeneration":view["generation"],"idempotencyKey":model::id(),"acknowledgePause":true})).unwrap();}
            else{let job=&view["jobs"][0];f.call("work.cancel",json!({"job":{"id":job["id"],"generation":job["generation"]},"idempotencyKey":model::id(),"acknowledgeCancel":true})).unwrap();}
            release.send(()).unwrap();let completion=task.join().unwrap().unwrap();assert_eq!(completion.termination(),EffectTermination::Stopped);assert_eq!(completion.outcome(),if pause{AttemptState::Partial}else{AttemptState::Canceled});
        });
        assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.requests(),1);assert_eq!(f.leases(),0);let view=f.view();assert_ne!(view["jobs"][1]["state"],"awaiting_preparation");
        let budget=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(budget["recentChargesTotal"],1);assert_eq!(budget["recentCharges"][0]["inputTokens"],7);assert_eq!(budget["recentCharges"][0]["outputTokens"],9);
    }
}

#[test]
fn queued_response_journal_fault_keeps_unknown_lease_and_never_repeats_request(){
    let f=Fixture::new(vec![answer()]);let(_,request,_)=f.queued();
    f.ai.db.lock().unwrap().execute_batch("CREATE TRIGGER queued_ai_journal_fault BEFORE UPDATE ON investigations WHEN json_extract(NEW.data,'$.results[0].status')='completed' BEGIN SELECT RAISE(ABORT,'synthetic queued journal fault'); END;").unwrap();
    let(token,grant)=f.next();let result=f.ai.run_queued_investigation(&f.store,token,grant);
    if let Ok(completion)=result{assert_eq!(completion.termination(),EffectTermination::Unknown);}
    assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.requests(),1);assert_eq!(f.leases(),1);
    assert!(f.call("work.operation.enqueue",request).is_ok());assert!(f.supervisor.next_dispatch(&f.store,&f.service).unwrap().is_none());assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
    let budget=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(budget["recentCharges"][0]["inputTokens"],7);assert_eq!(budget["recentCharges"][0]["outputTokens"],9);
}

#[test]
fn queued_parent_binding_failure_finishes_not_sent_without_child_or_charge(){
    let f=Fixture::new(vec![]);let(plan,_,_)=f.queued();
    f.store.work_transaction(|db,_|db.execute_batch("CREATE TEMP TRIGGER queued_ai_bind_fault BEFORE UPDATE ON work_leases WHEN NEW.effect_kind='ai_investigation' BEGIN SELECT RAISE(ABORT,'synthetic parent binding fault'); END;").map_err(failure)).unwrap();
    let(token,grant)=f.next();let completion=f.ai.run_queued_investigation(&f.store,token,grant).unwrap();
    assert_eq!(completion.termination(),EffectTermination::NotStarted);assert_eq!(f.leases(),0);assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    let stored=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();assert_eq!(stored.status,"failed");assert!(stored.results.is_empty());
    let snapshot=f.store.snapshot().unwrap();assert_eq!(snapshot.data_use_receipts.len(),1);assert_eq!(snapshot.data_use_receipts[0].events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::NotSent);
    assert!(f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap()["recentCharges"].as_array().unwrap().is_empty());
}

#[test]
fn lost_queue_cache_never_recreates_data_or_budget_grants(){
    let f=Fixture::new(vec![]);let(plan,_,_)=f.queued();
    let reopened=crate::workflow::WorkflowService::default();assert!(f.supervisor.next_dispatch(&f.store,&reopened).unwrap().is_none());
    assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.leases(),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
    assert!(f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).is_err());
    let view=f.view();assert_eq!(view["attempts"][0]["state"],"not_started");
}

#[test]
fn recovery_imports_completed_journal_after_final_commit_fault_and_replays_receipt(){
    for restart in [false,true]{
        let f=Fixture::new(vec![answer(),answer()]);let(plan,_,_)=f.queued();f.fault_completed_attempt();
        let(token,grant)=f.next();assert!(f.ai.run_queued_investigation(&f.store,token,grant).is_err());
        assert_eq!(f.fake.calls.load(Ordering::SeqCst),2);assert_eq!(f.leases(),1);let charges=f.charges();
        f.core_sql("DROP TRIGGER queued_ai_finish_fault;");let f=if restart{f.restart()}else{f};let calls=f.fake.calls.load(Ordering::SeqCst);
        let input=f.recovery_request();let recovered=f.call("work.reconcile",input.clone()).unwrap();
        assert_eq!(recovered["value"]["attempt"]["state"],"completed");assert_eq!(recovered["value"]["reconciliationRequired"],false);assert_eq!(f.leases(),0);assert_eq!(f.charges(),charges);
        let journal=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();assert_eq!(recovered["value"]["attempt"]["results"][0]["fingerprint"],journal_fingerprint(&journal).unwrap());
        let replay=f.call("work.reconcile",input.clone()).unwrap();assert_eq!(replay["replayed"],true);assert_eq!(replay["receipt"],recovered["receipt"]);
        let mut changed=input;changed["note"]=json!("Different acknowledgment does not match the same idempotency key");assert!(f.call("work.reconcile",changed).is_err());
        assert_eq!(f.fake.calls.load(Ordering::SeqCst),calls);assert_eq!(f.requests(),2);
    }
}

#[test]
fn recovery_repairs_lost_parent_result_once_and_rolls_back_lease_if_command_commit_fails(){
    let f=Fixture::new(vec![answer()]);let(plan,_,_)=f.queued();
    f.ai.db.lock().unwrap().execute_batch("CREATE TRIGGER queued_ai_parent_fault BEFORE UPDATE ON investigations WHEN json_extract(NEW.data,'$.results[0].status')='completed' BEGIN SELECT RAISE(ABORT,'synthetic lost result'); END;").unwrap();
    let(token,grant)=f.next();let _=f.ai.run_queued_investigation(&f.store,token,grant);
    assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.leases(),1);let charges=f.charges();
    f.ai.db.lock().unwrap().execute_batch("DROP TRIGGER queued_ai_parent_fault;").unwrap();let f=f.restart();
    f.core_sql("CREATE TEMP TRIGGER queued_ai_receipt_fault BEFORE INSERT ON work_commands WHEN NEW.action='work.reconcile' BEGIN SELECT RAISE(ABORT,'synthetic command commit failure'); END;");
    let input=f.recovery_request();assert!(f.call("work.reconcile",input.clone()).is_err());
    let repaired=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();assert_eq!(repaired.status,"failed");assert_eq!(repaired.results[0].status,"failed");assert!(repaired.results[0].text.is_empty());
    let fingerprint=journal_fingerprint(&repaired).unwrap();assert_eq!(f.view()["attempts"][0]["state"],"unknown");assert_eq!(f.leases(),1);assert_eq!(f.charges(),charges);
    let commands:u64=f.store.work_read(|db,_|db.query_row("SELECT count(*) FROM work_commands WHERE action='work.reconcile'",[],|row|row.get(0)).map_err(failure)).unwrap();assert_eq!(commands,0);
    f.core_sql("DROP TRIGGER queued_ai_receipt_fault;");let receipts=serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap();
    let result=f.call("work.reconcile",input.clone()).unwrap();assert_eq!(result["value"]["attempt"]["state"],"failed");assert!(result["value"]["attempt"]["results"].as_array().unwrap().is_empty());assert_eq!(f.leases(),0);
    assert_eq!(journal_fingerprint(&f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap()).unwrap(),fingerprint);assert_eq!(serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap(),receipts);assert_eq!(f.charges(),charges);
    assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),1);assert_eq!(f.call("work.reconcile",input).unwrap()["replayed"],true);
}

#[test]
fn recovery_releases_old_queued_claim_before_admission_without_reconstructing_grant(){
    let f=Fixture::new(vec![]);let(plan,_,_)=f.queued();let(token,grant)=f.next();
    let permit=f.ai.arbiter.claim_queued(&f.store,token).unwrap();drop(permit);drop(grant);assert_eq!(f.leases(),1);
    let f=f.restart();let result=f.call("work.reconcile",f.recovery_request()).unwrap();
    assert_eq!(result["value"]["attempt"]["state"],"not_started");assert_eq!(f.leases(),0);assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert!(f.charges().as_array().unwrap().is_empty());
    assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());assert_eq!(f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap().status,"failed");
    assert!(f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).is_err());
}

#[test]
fn recovery_marks_admitted_parent_not_sent_after_crash_before_binding(){
    let f=Fixture::new(vec![]);let(plan,_,_)=f.queued();
    f.core_sql("CREATE TEMP TRIGGER queued_ai_bind_fault BEFORE UPDATE ON work_leases WHEN NEW.effect_kind='ai_investigation' BEGIN SELECT RAISE(ABORT,'synthetic binding fault'); END;");
    f.ai.db.lock().unwrap().execute_batch("CREATE TRIGGER queued_ai_cleanup_fault BEFORE UPDATE ON investigations WHEN json_extract(NEW.data,'$.status')='failed' BEGIN SELECT RAISE(ABORT,'synthetic cleanup interruption'); END;").unwrap();
    let(token,grant)=f.next();assert!(f.ai.run_queued_investigation(&f.store,token,grant).is_err());assert_eq!(f.leases(),1);assert_eq!(f.requests(),0);
    f.ai.db.lock().unwrap().execute_batch("DROP TRIGGER queued_ai_cleanup_fault;").unwrap();let f=f.restart();let input=f.recovery_request();let result=f.call("work.reconcile",input.clone()).unwrap();
    assert_eq!(result["value"]["attempt"]["state"],"not_started");assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),0);assert!(f.charges().as_array().unwrap().is_empty());
    let snapshot=f.store.snapshot().unwrap();assert_eq!(snapshot.data_use_receipts.len(),1);let parent=&snapshot.data_use_receipts[0];assert_eq!(parent.effect.id,plan["id"].as_str().unwrap());assert_eq!(parent.events.last().unwrap().outcome,DataUseOutcome::NotSent);
    let receipts=serde_json::to_value(snapshot.data_use_receipts).unwrap();assert_eq!(f.call("work.reconcile",input).unwrap()["replayed"],true);assert_eq!(serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap(),receipts);
}

#[test]
fn recovery_terminal_without_usage_releases_execution_and_preserves_unknown_accounting(){
    let f=Fixture::new(vec![Ok(json!({"id":model::id(),"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Synthetic terminal result without usage"}]}]}))]);let(_,_,_)=f.queued();
    f.core_sql("CREATE TEMP TRIGGER queued_ai_partial_fault BEFORE UPDATE ON work_attempts WHEN NEW.state='partial' BEGIN SELECT RAISE(ABORT,'synthetic partial finalization fault'); END;");
    let(token,grant)=f.next();assert!(f.ai.run_queued_investigation(&f.store,token,grant).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);let charges=f.charges();let f=f.restart();
    let recovered=f.call("work.reconcile",f.recovery_request()).unwrap();assert_eq!(recovered["value"]["attempt"]["state"],"partial");assert_eq!(f.leases(),0);assert_eq!(f.charges(),charges);assert_eq!(f.charges()[0]["state"],"unknown");assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),1);assert_ne!(f.view()["jobs"][1]["state"],"awaiting_preparation");
}

#[test]
fn recovery_keeps_unknown_transport_and_nonterminal_calls_even_after_human_acknowledgment(){
    for response in [Err("Falha de conexão: synthetic timeout".into()),Ok(json!({"id":model::id(),"status":"in_progress"}))]{
        let f=Fixture::new(vec![response]);let(plan,_,_)=f.queued();let(token,grant)=f.next();let completion=f.ai.run_queued_investigation(&f.store,token,grant).unwrap();assert_eq!(completion.termination(),EffectTermination::Unknown);
        let stored=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();let request_id=&stored.results[0].request_id;
        f.ai.handle("ai.request.reconcile",json!({"assessmentId":f.aid,"requestId":request_id,"note":"Consumption and remote result remain unknown","acknowledgeUnknownUsage":true}),&f.store).unwrap();
        let f=f.restart();let charges=f.charges();let before=serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap();
        assert!(f.call("work.reconcile",f.recovery_request()).is_err());assert_eq!(f.leases(),1);assert_eq!(f.view()["attempts"][0]["state"],"unknown");assert_eq!(f.charges(),charges);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),1);
        assert_eq!(serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap(),before);
    }
}

#[test]
fn recovery_rejects_divergent_terminal_child_identity_and_busy_worker_without_mutating_journals(){
    let f=Fixture::new(vec![answer(),answer()]);let(plan,_,_)=f.queued();f.fault_completed_attempt();let(token,grant)=f.next();assert!(f.ai.run_queued_investigation(&f.store,token,grant).is_err());f.core_sql("DROP TRIGGER queued_ai_finish_fault;");
    let input=f.recovery_request();let guard=f.ai.inference.lock().unwrap();assert!(f.call("work.reconcile",input.clone()).is_err());drop(guard);
    let journal=f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap();let id=&journal.results[0].request_id;
    f.ai.db.lock().unwrap().execute("UPDATE requests SET details=json_set(details,'$.perspective','remediation') WHERE id=?1",[id]).unwrap();
    let before=serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap();let charges=f.charges();assert!(f.call("work.reconcile",input).is_err());
    assert_eq!(journal_fingerprint(&f.ai.load_investigation(plan["id"].as_str().unwrap()).unwrap()).unwrap(),journal_fingerprint(&journal).unwrap());assert_eq!(f.leases(),1);assert_eq!(f.fake.calls.load(Ordering::SeqCst),2);assert_eq!(f.charges(),charges);assert_eq!(serde_json::to_value(f.store.snapshot().unwrap().data_use_receipts).unwrap(),before);
}
