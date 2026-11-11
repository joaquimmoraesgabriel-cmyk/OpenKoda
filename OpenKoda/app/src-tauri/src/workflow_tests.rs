//! Real SQLite transaction/replay tests. No adapter, DNS, WSL, or provider runs.
use crate::{core::{Request,Store}, data_projection::{DataRedactor,RedactedText,RedactionMode}, model, workflow::*, work_store, work_supervisor::WorkSupervisor};
use serde_json::{json,Value};
use std::sync::{Arc,Barrier};

struct Redactor;
impl DataRedactor for Redactor {
    fn mode(&self)->RedactionMode{RedactionMode::NoCredentialConfigured}
    fn redact(&self,text:&str)->RedactedText{let secret=text.contains("SYNTHETIC-WORK-SECRET");RedactedText{text:if secret{"[redacted]".into()}else{text.into()},changed:secret,secret_material:secret}}
}
struct Fixture { _dir:tempfile::TempDir, store:Arc<Store>, service:Arc<WorkflowService>, aid:String }
impl Fixture {
    fn new()->Self{
        let dir=tempfile::tempdir().unwrap();let store=Arc::new(Store::open(dir.path().into()).unwrap());let aid=create_assessment(&store);
        Self{_dir:dir,store,service:Arc::new(WorkflowService::default()),aid}
    }
    fn draft(&self)->PlanPrepareRequest{draft(&self.aid)}
    fn prepare(&self)->PlanPreparation{self.service.prepare(&self.store,self.draft(),&Redactor).unwrap()}
    fn commit(&self,p:&PlanPreparation)->MutationResult<PlanView>{self.service.commit(&self.store,commit_input(p),&Redactor).unwrap()}
    fn count(&self,table:&str)->u64{self.store.work_read(|conn,_|conn.query_row(&format!("SELECT COUNT(*) FROM {table}"),[],|r|r.get(0)).map_err(storage_error)).unwrap()}
}
fn create_assessment(store:&Store)->String{
    let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Workflow fixture","kind":"web","target":"https://never-resolve.invalid/","description":"Synthetic only"})}).unwrap();
    store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Workflow fixture","kind":"web","target":project["target"],"objective":"No network in foundation","scope":crate::scope::test_creation(store,project["id"].as_str().unwrap(),&["http.headers.v1"])})}).unwrap()["id"].as_str().unwrap().into()
}
fn native(key:&str,dependencies:&[&str])->NodeDraft{NodeDraft{key:key.into(),existing_node_id:None,purpose:"Observe the authorized fixture".into(),expected_observation:"A bounded observation".into(),depends_on_keys:dependencies.iter().map(|d|(*d).into()).collect(),intent:DraftIntent::Native{adapter:NativeAdapter::HttpHeaders,target_ref:None,ports:None}}}
fn draft(aid:&str)->PlanPrepareRequest{
    PlanPrepareRequest{assessment_id:aid.into(),expected_head:None,expected_generation:0,goal:"Review before every concrete observation".into(),nodes:vec![native("a",&[]),NodeDraft{key:"b".into(),existing_node_id:None,purpose:"Review observed evidence".into(),expected_observation:"An explicit human decision".into(),depends_on_keys:vec!["a".into()],intent:DraftIntent::HumanReview{inputs:vec![DraftInput::DependencyOutput{node_key:"a".into(),output_role:OutputRole::Evidence}]}},native("c",&["b"])],limits:PlanLimits{max_attempts_total:10,max_capture_bytes_total:4*1024*1024}}
}
fn commit_input(p:&PlanPreparation)->PlanCommitRequest{PlanCommitRequest{review:p.review.clone(),idempotency_key:model::id(),acknowledge_plan:true}}

#[test]
fn nullable_fields_are_required_and_unknown_authority_fields_are_rejected(){
    let f=Fixture::new();let original=serde_json::to_value(f.draft()).unwrap();
    for pointer in ["/expectedHead","/nodes/0/existingNodeId","/nodes/0/intent/targetRef","/nodes/0/intent/ports"]{
        let mut request=original.clone();let (parent,key)=pointer.rsplit_once('/').unwrap();request.pointer_mut(parent).unwrap().as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<PlanPrepareRequest>(request).is_err(),"{pointer}");
    }
    for key in ["argv","shell","url","approved","dataReview"]{let mut request=original.clone();request["nodes"][0]["intent"][key]=json!("untrusted");assert!(serde_json::from_value::<PlanPrepareRequest>(request).is_err());}
    assert!(serde_json::from_value::<QueueRequest>(json!({})).is_err());assert!(serde_json::from_value::<PlanGetRequest>(json!({"assessmentId":f.aid})).is_err());
    assert!(known_reason_code("work_revision_changed"));assert!(!known_reason_code("work_SYNTHETIC-WORK-SECRET"));assert!(!known_reason_code("scope_revoked"));
}

#[test]
fn dag_preparation_is_pure_and_commit_is_one_atomic_revision(){
    let f=Fixture::new();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();let p=f.prepare();
    assert_eq!(validate_graph(&p.proposal.body).unwrap(),vec![0,1,2]);assert_eq!(f.count("work_plan_revisions"),0);
    assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);
    let result=f.commit(&p);assert!(!result.replayed);assert_eq!(result.value.jobs.len(),3);assert_eq!(result.value.jobs[0].state,JobState::AwaitingPreparation);assert_eq!(result.value.jobs[1].state,JobState::WaitingDependencies);assert!(!result.value.jobs[1].can_prepare);assert_eq!(result.value.event_cursor.after_sequence,1);
    assert_eq!(f.count("work_plan_revisions"),1);assert_eq!(f.count("work_commands"),1);assert_eq!(f.count("work_attempts"),0);assert_eq!(f.count("work_plan_bindings"),0);
    let after=f.store.snapshot().unwrap();assert!(after.plans.is_empty());assert!(after.executions.is_empty());assert_eq!(serde_json::to_value(after).unwrap(),before);
}

#[test]
fn graph_rejects_cycles_dangling_edges_duplicates_and_nonancestor_outputs(){
    let f=Fixture::new();let baseline=f.prepare().proposal.body;
    let mut bad=baseline.clone();bad.nodes[0].depends_on=vec![bad.nodes[2].id.clone()];assert!(validate_graph(&bad).is_err());
    let mut bad=baseline.clone();bad.nodes[1].depends_on=vec![model::id()];assert!(validate_graph(&bad).is_err());
    let mut bad=baseline.clone();let repeated=bad.nodes[0].id.clone();bad.nodes[1].depends_on.push(repeated);assert!(validate_graph(&bad).is_err());
    let mut bad=baseline.clone();bad.nodes[1].depends_on.clear();assert!(validate_graph(&bad).is_err());
    let mut bad=baseline.clone();bad.nodes[2].intent=OperationIntent::HumanReview{inputs:vec![WorkInput::DependencyOutput{node_id:bad.nodes[1].id.clone(),output_role:OutputRole::Evidence}]};assert!(validate_graph(&bad).is_err());
    let mut bad=baseline.clone();bad.nodes[1].id=bad.nodes[0].id.clone();assert!(validate_graph(&bad).is_err());
    let mut bad=baseline.clone();bad.scope_ref.assessment_id=model::id();assert!(validate_graph(&bad).is_err());
    let mut bad=baseline;bad.nodes[0].intent=OperationIntent::Native{adapter:NativeAdapter::HttpHeaders,target_ref:None,ports:Some(vec![80])};assert!(validate_graph(&bad).is_err());
}

#[test]
fn graph_caps_and_evidence_ownership_fail_before_commit(){
    let f=Fixture::new();let mut input=f.draft();input.nodes=(0..33).map(|i|native(&format!("n{i}"),&[])).collect();assert!(f.service.prepare(&f.store,input,&Redactor).unwrap_err().starts_with("work_limit"));
    let mut input=f.draft();input.nodes=(0..13).map(|i|{let mut n=native(&format!("n{i}"),&[]);n.depends_on_keys=(0..i).map(|d|format!("n{d}")).collect();n}).collect();assert!(f.service.prepare(&f.store,input,&Redactor).is_err());
    for limits in [PlanLimits{max_attempts_total:0,max_capture_bytes_total:1},PlanLimits{max_attempts_total:101,max_capture_bytes_total:1},PlanLimits{max_attempts_total:1,max_capture_bytes_total:MAX_PROFILE_CAPTURE_BYTES+1}]{let mut input=f.draft();input.limits=limits;assert!(f.service.prepare(&f.store,input,&Redactor).is_err());}
    let mut input=f.draft();input.nodes[1].intent=DraftIntent::HumanReview{inputs:vec![DraftInput::Evidence{evidence_id:model::id(),expected_raw_sha256:crate::core::hash("not owned")}]};assert!(f.service.prepare(&f.store,input,&Redactor).unwrap_err().starts_with("work_input_changed"));assert_eq!(f.count("work_jobs"),0);
}

#[test]
fn secret_and_metadata_only_plan_text_is_not_journaled(){
    struct Unavailable;impl DataRedactor for Unavailable{fn mode(&self)->RedactionMode{RedactionMode::MetadataOnly}fn redact(&self,_:&str)->RedactedText{RedactedText{text:String::new(),changed:true,secret_material:false}}}
    let f=Fixture::new();let mut input=f.draft();input.goal="SYNTHETIC-WORK-SECRET".into();assert!(f.service.prepare(&f.store,input,&Redactor).is_err());assert!(f.service.prepare(&f.store,f.draft(),&Unavailable).is_err());assert_eq!(f.count("work_plan_revisions"),0);
}

#[test]
fn two_threads_same_key_get_one_revision_job_set_and_receipt(){
    let f=Fixture::new();let p=f.prepare();let input=commit_input(&p);let barrier=Arc::new(Barrier::new(3));let mut handles=vec![];
    for _ in 0..2{let store=f.store.clone();let service=f.service.clone();let input=input.clone();let barrier=barrier.clone();handles.push(std::thread::spawn(move||{barrier.wait();service.commit(&store,input,&Redactor)}));}
    barrier.wait();let results=handles.into_iter().map(|h|h.join().unwrap().unwrap()).collect::<Vec<_>>();assert_eq!(results[0].receipt,results[1].receipt);assert_eq!(results.iter().filter(|r|r.replayed).count(),1);assert_eq!(results[0].value.jobs[0].id,results[1].value.jobs[0].id);assert_eq!(f.count("work_plan_revisions"),1);assert_eq!(f.count("work_jobs"),3);assert_eq!(f.count("work_events"),1);
}

#[test]
fn repeated_review_and_key_conflicts_never_create_more_work(){
    let f=Fixture::new();let p=f.prepare();let input=commit_input(&p);let first=f.service.commit(&f.store,input.clone(),&Redactor).unwrap();
    assert!(f.service.commit(&f.store,input.clone(),&Redactor).unwrap().replayed);
    let mut altered=input.clone();altered.review.digest=crate::core::hash("altered");assert!(f.service.commit(&f.store,altered,&Redactor).unwrap_err().starts_with("work_idempotency_conflict"));
    let mut new_key=input.clone();new_key.idempotency_key=model::id();assert!(f.service.commit(&f.store,new_key,&Redactor).unwrap_err().starts_with("work_review_used"));
    let mut next=f.draft();next.expected_head=Some(first.value.head.clone());next.expected_generation=first.value.generation;let proposal=f.service.prepare(&f.store,next,&Redactor).unwrap();let reused_key=PlanCommitRequest{review:proposal.review,idempotency_key:input.idempotency_key,acknowledge_plan:true};assert!(f.service.commit(&f.store,reused_key,&Redactor).unwrap_err().starts_with("work_idempotency_conflict"));assert_eq!(f.count("work_jobs"),3);
}

#[test]
fn competing_revisions_compare_head_generation_and_preserve_history(){
    let f=Fixture::new();let first=f.commit(&f.prepare());let mut input=f.draft();input.expected_head=Some(first.value.head.clone());input.expected_generation=first.value.generation;
    input.nodes[0].existing_node_id=Some(first.value.revision.body.nodes[0].id.clone());let a=f.service.prepare(&f.store,input.clone(),&Redactor).unwrap();let b=f.service.prepare(&f.store,input,&Redactor).unwrap();let committed=f.commit(&a);
    assert_eq!(committed.value.head.number,2);assert_eq!(committed.value.revision.body.nodes[0].id,first.value.revision.body.nodes[0].id);assert!(f.service.commit(&f.store,commit_input(&b),&Redactor).unwrap_err().starts_with("work_revision_changed"));
    let old=f.store.work_read(|conn,s|work_store::plan_view(conn,s,&f.aid,Some(&first.value.head.revision_id))).unwrap().unwrap();assert_eq!(old.revision.digest,first.value.revision.digest);assert!(old.jobs.iter().all(|j|j.state==JobState::Superseded));assert_eq!(f.count("work_jobs"),6);assert_eq!(f.count("work_attempts"),0);
}

#[test]
fn failed_transaction_after_all_writes_rolls_back_and_review_can_retry(){
    let f=Fixture::new();let response=f.prepare();let prepared=PreparedPlan{response:response.clone(),expected_head:None,expected_generation:0};let input=commit_input(&response);let request_digest=digest(&json!({"action":"work.plan.commit","payload":input})).unwrap();
    let failed:Result<(),String>=f.store.work_transaction(|tx,s|{work_store::commit_plan(tx,s,&prepared,&input,&request_digest)?;Err("injected before commit".into())});assert!(failed.is_err());
    for table in ["work_plan_heads","work_plan_revisions","work_jobs","work_commands","work_events"]{assert_eq!(f.count(table),0,"{table}");}
    assert!(!f.service.commit(&f.store,input,&Redactor).unwrap().replayed);assert_eq!(f.count("work_jobs"),3);
}

#[test]
fn insert_failures_at_journal_boundaries_leave_no_half_revision(){
    for table in ["work_plan_heads","work_plan_revisions","work_jobs","work_events","work_commands"]{
        let f=Fixture::new();let p=f.prepare();let input=commit_input(&p);
        f.store.work_transaction(|tx,_|tx.execute_batch(&format!("CREATE TEMP TRIGGER workflow_test_fault BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'synthetic fault'); END;")).map_err(storage_error)).unwrap();
        assert!(f.service.commit(&f.store,input.clone(),&Redactor).is_err(),"{table}");
        f.store.work_transaction(|tx,_|tx.execute_batch("DROP TRIGGER temp.workflow_test_fault;").map_err(storage_error)).unwrap();
        assert_eq!(f.count("work_plan_heads"),0);assert_eq!(f.count("work_plan_revisions"),0);assert_eq!(f.count("work_jobs"),0);assert_eq!(f.count("work_events"),0);assert_eq!(f.count("work_commands"),0);
        assert!(!f.service.commit(&f.store,input,&Redactor).unwrap().replayed);
    }
}

#[test]
fn restart_replay_reads_durable_receipt_without_resurrecting_review(){
    let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();let aid=create_assessment(&store);let service=WorkflowService::default();let p=service.prepare(&store,draft(&aid),&Redactor).unwrap();let input=commit_input(&p);let first=service.commit(&store,input.clone(),&Redactor).unwrap();drop(service);drop(store);
    let store=Store::open(dir.path().into()).unwrap();let supervisor=WorkSupervisor::default();supervisor.recover_after_boot(&store).unwrap();let service=WorkflowService::default();
    let replay=service.commit(&store,input,&Redactor).unwrap();assert!(replay.replayed);assert_eq!(replay.receipt,first.receipt);assert_eq!(replay.value.jobs[0].id,first.value.jobs[0].id);assert_eq!(replay.value.control,PlanControl::Paused);assert!(!replay.value.jobs[0].can_prepare);assert!(store.snapshot().unwrap().executions.is_empty());
    let before=replay.value.event_cursor;supervisor.recover_after_boot(&store).unwrap();let after=store.work_read(|conn,s|work_store::plan_view(conn,s,&aid,None)).unwrap().unwrap().event_cursor;assert_eq!(before,after);
}

#[test]
fn event_cursor_pages_are_monotonic_owned_and_have_no_raw_payload(){
    let f=Fixture::new();let first=f.commit(&f.prepare());let mut next=f.draft();next.expected_head=Some(first.value.head);next.expected_generation=first.value.generation;let second=f.commit(&f.service.prepare(&f.store,next,&Redactor).unwrap());
    let mut cursor=0;let mut sequences=vec![];loop{let response=f.service.handle("work.events",json!({"assessmentId":f.aid,"streamId":second.value.id,"afterSequence":cursor,"limit":2}),&f.store,&Redactor).unwrap();assert_eq!(response["gap"],false);for e in response["events"].as_array().unwrap(){assert!(e.get("content").is_none());assert!(e.get("payload").is_none());sequences.push(e["sequence"].as_u64().unwrap());}cursor=response["next"]["afterSequence"].as_u64().unwrap();if response["hasMore"]==false{break;}}
    assert_eq!(sequences,(1..=second.value.event_cursor.after_sequence).collect::<Vec<_>>());
    let other=create_assessment(&f.store);assert!(f.service.handle("work.events",json!({"assessmentId":other,"streamId":second.value.id,"afterSequence":0,"limit":10}),&f.store,&Redactor).is_err());
    let gap=f.service.handle("work.events",json!({"assessmentId":f.aid,"streamId":second.value.id,"afterSequence":cursor+100,"limit":10}),&f.store,&Redactor).unwrap();assert_eq!(gap["gap"],true);assert_eq!(gap["next"]["afterSequence"],cursor);
}

#[test]
fn archived_project_does_not_destroy_historical_work_or_grant_preparation(){
    let f=Fixture::new();let p=f.commit(&f.prepare());let project=f.store.snapshot().unwrap().assessments[0].project_id.clone();f.store.handle(Request{action:"project.archive".into(),payload:json!({"id":project})}).unwrap();
    let current=f.store.work_read(|conn,s|work_store::plan_view(conn,s,&f.aid,None)).unwrap().unwrap();assert_eq!(current.revision.digest,p.value.revision.digest);assert!(!current.jobs[0].can_prepare);
}

#[test]
fn profile_queue_cap_rolls_back_overflow_revision(){
    let f=Fixture::new();for i in 0..4{let aid=if i==0{f.aid.clone()}else{create_assessment(&f.store)};let mut input=draft(&aid);input.nodes=(0..32).map(|i|native(&format!("n{i}"),&[])).collect();let p=f.service.prepare(&f.store,input,&Redactor).unwrap();let result=f.service.commit(&f.store,commit_input(&p),&Redactor);if i<3{result.unwrap();}else{assert!(result.unwrap_err().starts_with("work_limit"));}}
    assert_eq!(f.count("work_jobs"),96);assert_eq!(f.count("work_plan_heads"),3);
}

#[test]
fn altered_columns_and_extra_schema_are_rejected_transactionally(){
    let f=Fixture::new();f.commit(&f.prepare());
    assert!(f.store.work_transaction(|tx,_|{tx.execute("UPDATE work_jobs SET state='completed'",[]).map_err(storage_error)?;Ok(())}).is_err());
    assert!(f.store.work_transaction(|tx,_|tx.execute_batch("ALTER TABLE work_jobs ADD COLUMN arbitrary TEXT;").map_err(storage_error)).is_err());
    assert!(f.store.work_transaction(|tx,_|tx.execute_batch("CREATE TABLE unexpected_data(value TEXT);").map_err(storage_error)).is_err());
    assert_eq!(f.count("work_jobs"),3);assert!(f.store.work_read(|conn,s|work_store::validate_store(conn,s)).is_ok());
}

fn local_fixture()->(tempfile::TempDir,Store,WorkflowService,WorkSupervisor,crate::executor::Executor,String,std::path::PathBuf){
    let dir=tempfile::tempdir().unwrap();let file=dir.path().join("fixture.js");std::fs::write(&file,"const message = 'synthetic fixture';\n").unwrap();
    let profile=dir.path().join("profile");let store=Store::open(profile).unwrap();let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Local workflow fixture","kind":"code","target":file.to_str().unwrap(),"description":"Temporary fixture only"})}).unwrap();
    let assessment=store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Native and human workflow","kind":"code","target":project["target"],"objective":"Read only fixture observations","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&["local.inventory.v1","source.rules.v1"])})}).unwrap();
    let supervisor=WorkSupervisor::default();let executor=crate::executor::Executor::default().with_arbiter(supervisor.shared_arbiter());
    (dir,store,WorkflowService::default(),supervisor,executor,assessment["id"].as_str().unwrap().into(),file)
}
fn local_draft(aid:&str)->PlanPrepareRequest{let mut input=draft(aid);input.nodes[0].intent=DraftIntent::Native{adapter:NativeAdapter::LocalInventory,target_ref:None,ports:None};input.nodes[2].intent=DraftIntent::Native{adapter:NativeAdapter::SourceRules,target_ref:None,ports:None};input}
fn job_reference(job:&Value)->Value{json!({"id":job["id"],"generation":job["generation"]})}
fn runtime(service:&WorkflowService,store:&Store,executor:&crate::executor::Executor,supervisor:&WorkSupervisor,action:&str,payload:Value)->Value{service.handle_runtime(action,payload,store,executor,supervisor,&Redactor).unwrap()}
fn plan_json(service:&WorkflowService,store:&Store,aid:&str)->Value{service.handle("work.plan.get",json!({"assessmentId":aid,"revisionId":null}),store,&Redactor).unwrap()["plan"].clone()}
fn prepare_node(service:&WorkflowService,store:&Store,executor:&crate::executor::Executor,supervisor:&WorkSupervisor,plan:&Value,index:usize)->Value{runtime(service,store,executor,supervisor,"work.operation.prepare",json!({"job":job_reference(&plan["jobs"][index]),"revision":plan["head"],"idempotencyKey":model::id()}))}
fn enqueue(service:&WorkflowService,store:&Store,executor:&crate::executor::Executor,supervisor:&WorkSupervisor,preparation:&Value,revision:&Value,key:&str)->Value{runtime(service,store,executor,supervisor,"work.operation.enqueue",json!({"kind":"native","job":job_reference(&preparation["value"]["job"]),"revision":revision,"compiled":preparation["value"]["compiled"],"idempotencyKey":key,"acknowledgeExecution":true}))}

#[test]
fn exhausted_completed_plan_cannot_offer_an_ai_observation() {
    for max_attempts in [1, 5] {
        let (_dir,store,service,supervisor,executor,aid,_) = local_fixture();
        let mut draft = local_draft(&aid);
        draft.nodes.truncate(1);
        draft.limits = PlanLimits { max_attempts_total: max_attempts, max_capture_bytes_total: 1048576 };
        let prepared = service.prepare(&store, draft, &Redactor).unwrap();
        service.commit(&store, commit_input(&prepared), &Redactor).unwrap();
        let plan = plan_json(&service, &store, &aid);
        let prepared = prepare_node(&service, &store, &executor, &supervisor, &plan, 0);
        enqueue(&service, &store, &executor, &supervisor, &prepared, &plan["head"], &model::id());
        let dispatch = supervisor.next_native(&store).unwrap().unwrap();
        let outcome = executor.run_queued(&store, &dispatch.plan_id, &dispatch.digest, dispatch.token).unwrap();
        assert_eq!(plan_json(&service, &store, &aid)["outcome"], "completed");
        if max_attempts > 1 {
            // A later human-only revision can retain exactly the bytes already
            // captured by the plan. Completing its review consumes no bytes.
            let completed = plan_json(&service, &store, &aid);
            let mut follow_up = local_draft(&aid);
            follow_up.expected_head = Some(serde_json::from_value(completed["head"].clone()).unwrap());
            follow_up.expected_generation = completed["generation"].as_u64().unwrap();
            let evidence = outcome.evidence.unwrap();
            let mut review = follow_up.nodes.remove(1);
            review.depends_on_keys.clear();
            review.intent = DraftIntent::HumanReview { inputs: vec![DraftInput::Evidence {
                evidence_id: evidence.id, expected_raw_sha256: evidence.sha256,
            }] };
            follow_up.nodes = vec![review];
            let used = store.work_read(|db, _| work_store::capture_usage(db, completed["id"].as_str())).unwrap();
            assert!(used > 0);
            follow_up.limits = PlanLimits { max_attempts_total: max_attempts, max_capture_bytes_total: used };
            let change = service.prepare(&store, follow_up, &Redactor).unwrap();
            service.commit(&store, commit_input(&change), &Redactor).unwrap();
            let latest = plan_json(&service, &store, &aid);
            let review = prepare_node(&service, &store, &executor, &supervisor, &latest, 0);
            let value = &review["value"];
            runtime(&service, &store, &executor, &supervisor, "work.review.commit", json!({
                "job": job_reference(&value["job"]), "revision": latest["head"], "review": value["review"],
                "inputDigest": value["inputDigest"], "decision": "continue", "note": "Synthetic evidence reviewed",
                "idempotencyKey": model::id(), "acknowledgeReview": true
            }));
            assert_eq!(plan_json(&service, &store, &aid)["outcome"], "completed");
        }
        assert!(store.work_read(|db, state| suggestion_base(db, state, &aid)).unwrap().is_none());
    }
}

#[test]
fn native_human_dag_requires_each_review_and_preserves_native_outputs(){
    let (_dir,store,service,supervisor,executor,aid,_file)=local_fixture();let prepared=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&prepared),&Redactor).unwrap();
    let plan=plan_json(&service,&store,&aid);assert!(!plan["jobs"][1]["canPrepare"].as_bool().unwrap());
    let a=prepare_node(&service,&store,&executor,&supervisor,&plan,0);assert!(store.snapshot().unwrap().executions.is_empty());
    let same=prepare_node(&service,&store,&executor,&supervisor,&plan_json(&service,&store,&aid),0);assert_eq!(a["value"]["compiled"],same["value"]["compiled"]);assert_eq!(store.snapshot().unwrap().plans.len(),1);
    let key=model::id();let queued=enqueue(&service,&store,&executor,&supervisor,&a,&plan["head"],&key);assert!(store.snapshot().unwrap().executions.is_empty());
    assert!(enqueue(&service,&store,&executor,&supervisor,&a,&plan["head"],&key)["replayed"].as_bool().unwrap());
    let dispatch=supervisor.next_native(&store).unwrap().unwrap();let outcome=executor.run_queued(&store,&dispatch.plan_id,&dispatch.digest,dispatch.token).unwrap();assert_eq!(outcome.execution.status,"completed");assert!(supervisor.next_native(&store).unwrap().is_none());
    let after_a=plan_json(&service,&store,&aid);assert_eq!(after_a["jobs"][0]["state"],"completed");assert_eq!(after_a["jobs"][1]["state"],"awaiting_preparation");assert_eq!(after_a["jobs"][2]["state"],"waiting_dependencies");assert_eq!(after_a["attempts"][0]["id"],queued["value"]["attempt"]["id"]);
    let b=prepare_node(&service,&store,&executor,&supervisor,&after_a,1);assert_eq!(b["value"]["resolvedInputs"][0]["id"],outcome.evidence.as_ref().unwrap().id);
    runtime(&service,&store,&executor,&supervisor,"work.review.commit",json!({"job":job_reference(&b["value"]["job"]),"revision":after_a["head"],"review":b["value"]["review"],"inputDigest":b["value"]["inputDigest"],"decision":"continue","note":"Reviewed the identified synthetic observation.","idempotencyKey":model::id(),"acknowledgeReview":true}));
    let after_b=plan_json(&service,&store,&aid);assert_eq!(after_b["jobs"][2]["state"],"awaiting_preparation");assert_eq!(store.snapshot().unwrap().plans.len(),1);
    let c=prepare_node(&service,&store,&executor,&supervisor,&after_b,2);enqueue(&service,&store,&executor,&supervisor,&c,&after_b["head"],&model::id());
    runtime(&service,&store,&executor,&supervisor,"work.pause",json!({"planId":after_b["id"],"expectedGeneration":after_b["generation"],"idempotencyKey":model::id(),"acknowledgePause":true}));assert!(supervisor.next_native(&store).unwrap().is_none());
    let paused=plan_json(&service,&store,&aid);let resume=runtime(&service,&store,&executor,&supervisor,"work.resume.prepare",json!({"planId":paused["id"],"expectedGeneration":paused["generation"]}));runtime(&service,&store,&executor,&supervisor,"work.resume.commit",json!({"review":resume["review"],"idempotencyKey":model::id(),"acknowledgeResume":true}));
    let dispatch=supervisor.next_native(&store).unwrap().unwrap();executor.run_queued(&store,&dispatch.plan_id,&dispatch.digest,dispatch.token).unwrap();
    let completed=plan_json(&service,&store,&aid);assert_eq!(completed["outcome"],"completed");assert_eq!(completed["attempts"].as_array().unwrap().len(),2);assert_eq!(store.snapshot().unwrap().executions.len(),2);
    let captures=store.work_read(|conn,_|crate::capture::list_locked(conn,&aid,None)).unwrap();assert_eq!(captures.len(),2);assert!(captures.iter().all(|m|m.state==crate::capture::CaptureState::Sealed));
    assert!(enqueue(&service,&store,&executor,&supervisor,&a,&plan["head"],&key)["replayed"].as_bool().unwrap());assert!(supervisor.next_native(&store).unwrap().is_none());assert_eq!(store.snapshot().unwrap().executions.len(),2);
}

#[test]
fn queued_cancel_and_old_enqueue_replay_never_dispatch(){
    let (_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let plan=plan_json(&service,&store,&aid);let p=prepare_node(&service,&store,&executor,&supervisor,&plan,0);let key=model::id();let q=enqueue(&service,&store,&executor,&supervisor,&p,&plan["head"],&key);
    let canceled=runtime(&service,&store,&executor,&supervisor,"work.cancel",json!({"job":job_reference(&q["value"]["job"]),"idempotencyKey":model::id(),"acknowledgeCancel":true}));assert_eq!(canceled["value"]["job"]["state"],"canceled");assert_eq!(canceled["value"]["attempt"]["state"],"not_started");assert_eq!(canceled["value"]["terminationConfirmed"],true);
    assert_eq!(enqueue(&service,&store,&executor,&supervisor,&p,&plan["head"],&key)["replayed"],true);assert!(supervisor.next_native(&store).unwrap().is_none());assert!(store.snapshot().unwrap().executions.is_empty());
}

#[test]
fn failed_preparation_key_is_never_reused_for_a_new_source_read(){
    let (_dir,store,service,supervisor,executor,aid,file)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let plan=plan_json(&service,&store,&aid);let key=model::id();let request=json!({"job":job_reference(&plan["jobs"][0]),"revision":plan["head"],"idempotencyKey":key});
    std::fs::remove_file(&file).unwrap();assert!(service.handle_runtime("work.operation.prepare",request.clone(),&store,&executor,&supervisor,&Redactor).is_err());assert!(store.snapshot().unwrap().plans.is_empty());
    std::fs::write(&file,"const restored = 'synthetic fixture';\n").unwrap();assert!(service.handle_runtime("work.operation.prepare",request,&store,&executor,&supervisor,&Redactor).is_err());assert!(store.snapshot().unwrap().plans.is_empty());
    let fresh=prepare_node(&service,&store,&executor,&supervisor,&plan_json(&service,&store,&aid),0);assert_eq!(fresh["value"]["kind"],"native");assert_eq!(store.snapshot().unwrap().plans.len(),1);assert!(store.snapshot().unwrap().executions.is_empty());
}

#[test]
fn active_attempt_cannot_lose_its_matching_lease(){
    let (_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let plan=plan_json(&service,&store,&aid);let p=prepare_node(&service,&store,&executor,&supervisor,&plan,0);enqueue(&service,&store,&executor,&supervisor,&p,&plan["head"],&model::id());
    let dispatch=supervisor.next_native(&store).unwrap().unwrap();let permit=supervisor.arbiter().claim_queued(&store,dispatch.token).unwrap();
    assert!(store.work_transaction(|conn,_|{conn.execute("DELETE FROM work_leases",[]).map_err(storage_error)?;Ok(())}).unwrap_err().starts_with("work_storage_unavailable"));
    assert!(store.work_read(|conn,s|crate::work_supervisor::validate_leases(conn,s)).is_ok());
    supervisor.arbiter().finish(permit,crate::work_supervisor::EffectTermination::NotStarted).unwrap();
    assert!(supervisor.arbiter().ensure_quiescent(&store).is_ok());assert!(store.snapshot().unwrap().executions.is_empty());
}

#[test]
fn abandoned_dispatch_is_unknown_immediately_and_keeps_its_reservations(){
    let (_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let plan=plan_json(&service,&store,&aid);let p=prepare_node(&service,&store,&executor,&supervisor,&plan,0);let q=enqueue(&service,&store,&executor,&supervisor,&p,&plan["head"],&model::id());let attempt_id=q["value"]["attempt"]["id"].as_str().unwrap();
    let reserved=store.work_read(|conn,_|work_store::attempt(conn,attempt_id)).unwrap().reserved_capture_bytes;
    let dispatch=supervisor.next_native(&store).unwrap().unwrap();let permit=supervisor.arbiter().claim_queued(&store,dispatch.token).unwrap();drop(permit);
    let current=plan_json(&service,&store,&aid);assert_eq!(current["jobs"][0]["state"],"unknown");assert_eq!(current["attempts"][0]["state"],"unknown");assert_eq!(current["attempts"][0]["cleanup"],"unknown");
    let remaining=store.work_read(|conn,_|work_store::attempt(conn,attempt_id)).unwrap();assert_eq!(remaining.reserved_capture_bytes,reserved);assert!(remaining.reserved_critical_events>0);assert!(supervisor.arbiter().ensure_quiescent(&store).unwrap_err().starts_with("work_unknown_effect"));assert!(supervisor.next_native(&store).unwrap().is_none());assert!(store.snapshot().unwrap().executions.is_empty());
}

#[test]
fn unbound_native_unknown_reconciles_without_effect_and_requires_fresh_prepare(){
    for reopen in [false,true]{
        let(_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let p=prepare_node(&service,&store,&executor,&supervisor,&plan_json(&service,&store,&aid),0);let q=enqueue(&service,&store,&executor,&supervisor,&p,&p["value"]["job"]["revision"],&model::id());
        let original_plan_id=p["value"]["compiled"]["planId"].as_str().unwrap().to_owned();let attempt_id=q["value"]["attempt"]["id"].as_str().unwrap().to_owned();let dispatch=supervisor.next_native(&store).unwrap().unwrap();drop(supervisor.arbiter().claim_queued(&store,dispatch.token).unwrap());
        let path=store.root.clone();let store=if reopen{drop(store);Store::open(path).unwrap()}else{store};let supervisor=WorkSupervisor::default();let executor=crate::executor::Executor::default().with_arbiter(supervisor.shared_arbiter());let service=WorkflowService::default();if reopen{supervisor.recover_after_boot(&store).unwrap();}
        let input=json!({"attemptId":attempt_id,"expectedState":"unknown","note":"Inspect the durable native admission boundary without re-running its adapter.","idempotencyKey":model::id(),"acknowledgeUnknownEffects":true});
        let reconciled=runtime(&service,&store,&executor,&supervisor,"work.reconcile",input.clone());assert_eq!(reconciled["replayed"],false);assert_eq!(reconciled["value"]["attempt"]["state"],"not_started");assert_eq!(reconciled["value"]["attempt"]["cleanup"],"confirmed");assert_eq!(reconciled["value"]["job"]["state"],"awaiting_preparation");assert_eq!(reconciled["value"]["reconciliationRequired"],false);
        assert_eq!(runtime(&service,&store,&executor,&supervisor,"work.reconcile",input)["replayed"],true);assert!(store.snapshot().unwrap().executions.is_empty());assert_eq!(store.snapshot().unwrap().plans[0].status,"revoked");assert!(supervisor.arbiter().ensure_quiescent(&store).is_ok());
        if reopen{let paused=plan_json(&service,&store,&aid);let review=runtime(&service,&store,&executor,&supervisor,"work.resume.prepare",json!({"planId":paused["id"],"expectedGeneration":paused["generation"]}));runtime(&service,&store,&executor,&supervisor,"work.resume.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeResume":true}));}
        let fresh=prepare_node(&service,&store,&executor,&supervisor,&plan_json(&service,&store,&aid),0);assert_ne!(fresh["value"]["compiled"]["planId"],original_plan_id);assert_eq!(store.snapshot().unwrap().plans.len(),2);assert!(store.snapshot().unwrap().executions.is_empty());assert!(supervisor.next_native(&store).unwrap().is_none());
    }
}

#[test]
fn unknown_capture_keeps_full_reservation_after_boot_seals_its_prefix(){
    let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"owned partial prefix",crate::capture::CaptureState::Open);
    assert_eq!(store.snapshot().unwrap().executions[0].status,"unknown");let reserved=store.work_read(|conn,_|Ok(work_store::attempt(conn,&manifest.attempt_id)?.reserved_capture_bytes)).unwrap();assert!(reserved>manifest.preserved_bytes);
    let supervisor=WorkSupervisor::default();supervisor.recover_after_boot(&store).unwrap();
    store.work_read(|conn,s|{let sealed=crate::capture::load_locked(conn,&manifest.id)?;assert_eq!(sealed.state,crate::capture::CaptureState::Interrupted);assert_eq!(sealed.preserved_bytes,manifest.preserved_bytes);assert_eq!(work_store::capture_usage(conn,None)?,reserved);let h=work_store::head_for_assessment(conn,&aid)?.unwrap();let revision=work_store::revision(conn,&h.head_revision_id)?;assert_eq!(work_store::remaining_quotas(conn,s,&revision.reference())?.2,MAX_PROFILE_CAPTURE_BYTES-reserved);Ok(())}).unwrap();
    assert!(supervisor.arbiter().ensure_quiescent(&store).is_err());assert!(supervisor.next_native(&store).unwrap().is_none());
}

// Populate a valid bounded metadata journal in one transaction. This exercises
// the actual byte quota; no effect, approval, binding or receipt is injected.
fn fill_event_capacity(store:&Store,aid:&str,leave:u64){
    store.work_transaction(|conn,_|{
        let h=work_store::head_for_assessment(conn,aid)?.unwrap();let (mut used,reserve)=work_store::event_usage(conn)?;let budget=(MAX_EVENT_BYTES as u64).saturating_sub(reserve).saturating_sub(leave);
        let mut sequence:u64=conn.query_row("SELECT COALESCE(MAX(sequence),0) FROM work_events WHERE stream_id=?1",[&h.plan_id],|row|row.get(0)).map_err(storage_error)?;
        let mut insert=conn.prepare("INSERT INTO work_events(stream_id,sequence,id,data) VALUES(?1,?2,?3,?4)").map_err(storage_error)?;
        loop{let now=model::now();let e=WorkEvent{id:model::id(),stream_id:h.plan_id.clone(),sequence:sequence+1,schema:"openkoda.work-event.v1".into(),assessment_id:aid.into(),job_id:None,attempt_id:None,event_type:WorkEventType::ReconciliationRecorded,observed_at:now.clone(),recorded_at:now,state:None,reason_code:Some(WorkReasonCode::WorkPreparationRequired),effect:None,capture_id:None,preserved_bytes:None};let raw=encode(&e)?;if used.saturating_add(raw.len() as u64)>budget{break;}insert.execute(rusqlite::params![e.stream_id,e.sequence,e.id,raw]).map_err(storage_error)?;used+=raw.len() as u64;sequence+=1;}
        Ok(())
    }).unwrap();
}

#[test]
fn progress_cannot_spend_the_space_reserved_for_critical_events(){
    let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"owned prefix",crate::capture::CaptureState::Open);fill_event_capacity(&store,&aid,0);
    let (before,reserved)=store.work_read(|conn,_|Ok((conn.query_row("SELECT count(*) FROM work_events",[],|row|row.get::<_,u64>(0)).map_err(storage_error)?,work_store::attempt(conn,&manifest.attempt_id)?.reserved_critical_events))).unwrap();
    store.work_transaction(|conn,_|work_store::append_capture_event(conn,&manifest,false)).unwrap();
    store.work_read(|conn,_|{assert_eq!(conn.query_row("SELECT count(*) FROM work_events",[],|row|row.get::<_,u64>(0)).map_err(storage_error)?,before);assert_eq!(work_store::attempt(conn,&manifest.attempt_id)?.reserved_critical_events,reserved);Ok(())}).unwrap();
    store.work_transaction(|conn,_|{let a=work_store::attempt(conn,&manifest.attempt_id)?;let j=work_store::job(conn,&a.view.job_id)?;work_store::append_event(conn,&j.revision.plan_id,&aid,WorkEventType::JobStateChanged,Some(j.id),Some(a.view.id),Some(JobState::Unknown),Some(WorkReasonCode::WorkUnknownEffect),a.view.effect)?;Ok(())}).unwrap();
    store.work_read(|conn,_|{assert_eq!(conn.query_row("SELECT count(*) FROM work_events",[],|row|row.get::<_,u64>(0)).map_err(storage_error)?,before+1);assert_eq!(work_store::attempt(conn,&manifest.attempt_id)?.reserved_critical_events,reserved-1);Ok(())}).unwrap();
}

#[test]
fn insufficient_event_reservation_rejects_enqueue_without_approving_or_effect(){
    let (_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let plan=plan_json(&service,&store,&aid);let p=prepare_node(&service,&store,&executor,&supervisor,&plan,0);fill_event_capacity(&store,&aid,64*1024);
    let request=json!({"kind":"native","job":job_reference(&p["value"]["job"]),"revision":plan["head"],"compiled":p["value"]["compiled"],"idempotencyKey":model::id(),"acknowledgeExecution":true});
    assert!(service.handle_runtime("work.operation.enqueue",request,&store,&executor,&supervisor,&Redactor).unwrap_err().starts_with("work_limit"));
    let snapshot=store.snapshot().unwrap();assert!(snapshot.executions.is_empty());assert_eq!(snapshot.plans[0].status,"proposed");assert!(snapshot.plans[0].approved_at.is_none());assert_eq!(plan_json(&service,&store,&aid)["attempts"].as_array().unwrap().len(),0);assert!(supervisor.next_native(&store).unwrap().is_none());
}

fn fill_command_capacity(store:&Store,aid:&str){
    store.work_transaction(|conn,_|{
        let key:String=conn.query_row("SELECT idempotency_key FROM work_commands WHERE assessment_id=?1 AND json_extract(result,'$.phase')='completed' LIMIT 1",[aid],|row|row.get(0)).map_err(storage_error)?;let source=work_store::command_by_key(conn,aid,&key)?.unwrap();
        let work_store::CommandResult::Completed{receipt,output}=source.result else{unreachable!()};let (mut actual,reserved)=work_store::metadata_usage(conn)?;
        loop{let mut receipt=receipt.clone();receipt.id=model::id();let command=work_store::StoredCommand{assessment_id:aid.into(),idempotency_key:model::id(),review_id:None,result:work_store::CommandResult::Completed{receipt,output:output.clone()}};let size=encode(&command.result)?.len() as u64;if actual+reserved+size>MAX_METADATA_BYTES as u64{break;}work_store::insert_command(conn,&command)?;actual+=size;}
        assert!(MAX_METADATA_BYTES as u64-actual-reserved<1024);Ok(())
    }).unwrap();
}

#[test]
fn prepared_job_can_recover_when_all_unreserved_event_space_is_full(){
    let(_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();prepare_node(&service,&store,&executor,&supervisor,&plan_json(&service,&store,&aid),0);fill_event_capacity(&store,&aid,0);
    let path=store.root.clone();drop(store);let store=Store::open(path).unwrap();WorkSupervisor::default().recover_after_boot(&store).unwrap();
    let current=plan_json(&WorkflowService::default(),&store,&aid);assert_eq!(current["control"],"paused");assert_eq!(current["jobs"][0]["state"],"awaiting_preparation");assert_eq!(store.snapshot().unwrap().plans[0].status,"revoked");assert!(store.snapshot().unwrap().executions.is_empty());
}

#[test]
fn unknown_metadata_expands_and_reopens_inside_reserved_envelope(){
    let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"owned metadata prefix",crate::capture::CaptureState::Open);fill_command_capacity(&store,&aid);
    let path=store.root.clone();drop(store);let store=Store::open(path).unwrap();let supervisor=WorkSupervisor::default();supervisor.recover_after_boot(&store).unwrap();
    store.work_read(|conn,_|{let(used,reserved)=work_store::metadata_usage(conn)?;assert!(used+reserved<=MAX_METADATA_BYTES as u64);assert!(reserved>0);assert_eq!(crate::capture::load_locked(conn,&manifest.id)?.state,crate::capture::CaptureState::Interrupted);assert_eq!(work_store::attempt(conn,&manifest.attempt_id)?.view.state,AttemptState::Unknown);Ok(())}).unwrap();assert!(supervisor.arbiter().ensure_quiescent(&store).is_err());
}

#[test]
fn running_native_finalizes_when_commands_fill_unreserved_metadata(){
    use std::{io::{Read,Write},net::TcpListener,sync::mpsc,time::Duration};
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();let target=format!("http://{}/fixture",listener.local_addr().unwrap());let (seen_tx,seen_rx)=mpsc::channel();let(release_tx,release_rx)=mpsc::channel();
    let peer=std::thread::spawn(move||{let(mut socket,_)=listener.accept().unwrap();socket.set_read_timeout(Some(Duration::from_secs(30))).unwrap();let mut raw=Vec::new();let mut block=[0;1024];while !raw.windows(4).any(|w|w==b"\r\n\r\n"){let n=socket.read(&mut block).unwrap();assert!(n>0);raw.extend_from_slice(&block[..n]);assert!(raw.len()<4096);}assert!(raw.starts_with(b"HEAD /fixture "));seen_tx.send(()).unwrap();release_rx.recv_timeout(Duration::from_secs(90)).unwrap();socket.write_all(b"HTTP/1.1 200 OK\r\nX-Content-Type-Options: nosniff\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();});
    let dir=tempfile::tempdir().unwrap();let store=Arc::new(Store::open(dir.path().join("state")).unwrap());store.handle(Request{action:"settings.save".into(),payload:json!({"timeoutSeconds":120})}).unwrap();let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Owned held HEAD metadata fixture","kind":"web","target":target})}).unwrap();
    let aid=store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Bounded metadata finalization","kind":"web","target":target,"objective":"Observe an owned loopback response with a held completion","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&["http.headers.v1"])})}).unwrap()["id"].as_str().unwrap().to_owned();
    let service=WorkflowService::default();let supervisor=WorkSupervisor::default();let executor=Arc::new(crate::executor::Executor::default().with_arbiter(supervisor.shared_arbiter()));let mut input=draft(&aid);input.nodes=vec![native("observe",&[])];let proposal=service.prepare(&store,input,&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let p=prepare_node(&service,&store,&executor,&supervisor,&plan_json(&service,&store,&aid),0);enqueue(&service,&store,&executor,&supervisor,&p,&p["value"]["job"]["revision"],&model::id());let dispatch=supervisor.next_native(&store).unwrap().unwrap();let running_store=store.clone();let running_executor=executor.clone();
    let run=std::thread::spawn(move||running_executor.run_queued(&running_store,&dispatch.plan_id,&dispatch.digest,dispatch.token));seen_rx.recv_timeout(Duration::from_secs(30)).unwrap();assert_eq!(plan_json(&service,&store,&aid)["jobs"][0]["state"],"running");fill_command_capacity(&store,&aid);release_tx.send(()).unwrap();let completed=run.join().unwrap().unwrap();peer.join().unwrap();assert_eq!(completed.execution.status,"completed");assert_eq!(plan_json(&service,&store,&aid)["jobs"][0]["state"],"completed");assert!(supervisor.arbiter().ensure_quiescent(&store).is_ok());
    store.work_read(|conn,_|{let(used,reserved)=work_store::metadata_usage(conn)?;assert!(used+reserved<=MAX_METADATA_BYTES as u64);assert_eq!(reserved,0);Ok(())}).unwrap();
}

#[test]
fn old_boot_native_direct_without_binding_revokes_approval_and_releases_only_native(){
    let(_dir,store,_service,supervisor,executor,aid,_)=local_fixture();let plan=executor.handle("assessment.plan",json!({"id":aid}),&store).unwrap();executor.handle("plan.approve",json!({"id":plan["id"],"digest":plan["digest"]}),&store).unwrap();
    let permit=supervisor.arbiter().claim_direct(&store,crate::work_supervisor::EffectOwner{id:model::id(),assessment_id:Some(aid.clone()),kind:crate::work_supervisor::EffectOwnerKind::Native}).unwrap();drop(permit);assert!(supervisor.arbiter().ensure_quiescent(&store).is_err());
    supervisor.recover_after_boot(&store).unwrap();assert!(supervisor.arbiter().ensure_quiescent(&store).is_err());let path=store.root.clone();drop(store);let store=Store::open(path).unwrap();let recovered=WorkSupervisor::default();recovered.recover_after_boot(&store).unwrap();assert!(recovered.arbiter().ensure_quiescent(&store).is_ok());assert!(store.snapshot().unwrap().executions.is_empty());assert_eq!(store.snapshot().unwrap().plans[0].status,"revoked");
}

#[test]
fn investigation_dependency_is_only_a_human_derived_input(){
    let f=Fixture::new();let mut body=f.prepare().proposal.body;
    body.nodes[0].intent=OperationIntent::AiInvestigation{inputs:vec![WorkInput::Evidence{evidence_id:model::id(),expected_raw_sha256:crate::core::hash("synthetic")}],perspectives:vec![Perspective::Evidence]};
    body.nodes[1].intent=OperationIntent::HumanReview{inputs:vec![WorkInput::DependencyOutput{node_id:body.nodes[0].id.clone(),output_role:OutputRole::Investigation}]};
    assert!(validate_graph(&body).is_ok());
    let mut bad=body.clone();bad.nodes[1].intent=OperationIntent::HumanReview{inputs:vec![WorkInput::DependencyOutput{node_id:body.nodes[0].id.clone(),output_role:OutputRole::Evidence}]};assert!(validate_graph(&bad).is_err());
    let mut bad=body.clone();bad.nodes[1].intent=OperationIntent::AiInvestigation{inputs:vec![WorkInput::DependencyOutput{node_id:body.nodes[0].id.clone(),output_role:OutputRole::Investigation}],perspectives:vec![Perspective::Evidence]};assert!(validate_graph(&bad).is_err());
    let mut bad=body;bad.nodes[0].intent=OperationIntent::Native{adapter:NativeAdapter::HttpHeaders,target_ref:None,ports:None};assert!(validate_graph(&bad).is_err());
}

#[test]
fn backend_wire_requires_nullable_budget_and_rejects_persisted_authority(){
    let f=Fixture::new();let p=f.prepare();let job=JobRef{id:model::id(),generation:1};
    let mut input=json!({"kind":"ai_investigation","job":job,"revision":p.proposal.reference(),"compiled":{"kind":"ai_investigation","investigationId":model::id(),"digest":crate::core::hash("plan"),"expiresAt":model::now()},"dataReview":{"id":model::id(),"digest":crate::core::hash("data"),"expiresAt":model::now()},"allowRemote":true,"idempotencyKey":model::id()});
    assert!(serde_json::from_value::<EnqueueRequest>(input.clone()).is_err());input["budgetReview"]=Value::Null;assert!(serde_json::from_value::<EnqueueRequest>(input.clone()).is_ok());
    for key in ["grant","providerKey","admittedContext","argv","approved"]{let mut bad=input.clone();bad[key]=json!(true);assert!(serde_json::from_value::<EnqueueRequest>(bad).is_err());}
    assert!(serde_json::from_value::<CompiledOperationRef>(json!({"kind":"kali_evidence","packetDigest":crate::core::hash("old unused shape"),"expiresAt":model::now()})).is_err());
}

#[test]
fn backend_preparation_replays_metadata_after_restart_without_effects_or_grants(){
    let f=Fixture::new();let evidence=f.store.handle(Request{action:"evidence.import".into(),payload:json!({"assessmentId":f.aid,"name":"Own synthetic text","sourceDescription":"Synthetic fixture input","content":"Literal local fixture","acknowledgeUnverifiedSource":true})}).unwrap();
    let mut input=f.draft();input.nodes=vec![NodeDraft{key:"review".into(),existing_node_id:None,purpose:"Prepare an independent review".into(),expected_observation:"Selected evidence identifiers".into(),depends_on_keys:vec![],intent:DraftIntent::AiInvestigation{inputs:vec![DraftInput::Evidence{evidence_id:evidence["id"].as_str().unwrap().into(),expected_raw_sha256:evidence["sha256"].as_str().unwrap().into()}],perspectives:vec![Perspective::Evidence]}}];
    let p=f.service.prepare(&f.store,input,&Redactor).unwrap();let committed=f.commit(&p);let j=&committed.value.jobs[0];let request=json!({"job":{"id":j.id,"generation":j.generation},"revision":committed.value.head,"idempotencyKey":model::id()});
    let supervisor=WorkSupervisor::default();let executor=crate::executor::Executor::default();let ai=crate::ai::AiService::new(f.store.root.clone()).unwrap();let kali=crate::kali_bridge::KaliBridge::new(f.store.root.clone()).unwrap();
    let first=f.service.handle_backend("work.operation.prepare",request.clone(),&f.store,&executor,&supervisor,&ai,&kali,None,&||panic!("metadata preparation cannot inspect the environment"),&Redactor).unwrap();
    assert_eq!(first["value"]["kind"],"ai_investigation");assert_eq!(first["value"]["contextSelection"]["evidenceIds"],json!([evidence["id"]]));assert!(first["value"].get("dataReview").is_none());assert_eq!(f.count("work_attempts"),0);assert_eq!(f.count("work_leases"),0);assert!(f.store.snapshot().unwrap().executions.is_empty());
    let restarted=WorkflowService::default();let replay=restarted.handle_backend("work.operation.prepare",request,&f.store,&executor,&supervisor,&ai,&kali,None,&||panic!("replay cannot inspect environment"),&Redactor).unwrap();assert_eq!(replay["replayed"],true);assert_eq!(replay["receipt"],first["receipt"]);assert_eq!(replay["value"],first["value"]);assert!(supervisor.next_dispatch(&f.store,&restarted).unwrap().is_none());
}

#[test]
fn direct_kali_review_claim_blocks_queue_ownership_until_proven_not_started(){
    let f=Fixture::new();let supervisor=WorkSupervisor::default();let review=crate::data_policy::DataReviewRef{id:model::id(),digest:crate::core::hash("synthetic reviewed packet"),expires_at:(chrono::Utc::now()+chrono::Duration::minutes(5)).to_rfc3339()};
    let owner=crate::work_supervisor::EffectOwner{id:model::id(),assessment_id:Some(f.aid.clone()),kind:crate::work_supervisor::EffectOwnerKind::KaliRunIntent};
    let permit=supervisor.arbiter().claim_direct_kali(&f.store,owner,Some(&review)).unwrap();
    let denied=f.store.work_read(|conn,_|crate::work_supervisor::assert_kali_review_available(conn,&review)).unwrap_err();assert!(denied.starts_with("work_owned_operation"));
    supervisor.arbiter().finish(permit,crate::work_supervisor::EffectTermination::NotStarted).unwrap();f.store.work_read(|conn,_|crate::work_supervisor::assert_kali_review_available(conn,&review)).unwrap();
}

#[test]
fn pause_mutation_uses_the_same_gate_as_the_final_dispatch_marker(){
    use std::{sync::mpsc,time::Duration};
    let f=Fixture::new();let committed=f.commit(&f.prepare());let gate=f.store.scope_gate.lock().unwrap();let(start_tx,start_rx)=mpsc::channel();let(done_tx,done_rx)=mpsc::channel();let store=f.store.clone();let service=f.service.clone();
    let payload=json!({"planId":committed.value.id,"expectedGeneration":committed.value.generation,"idempotencyKey":model::id(),"acknowledgePause":true});
    let worker=std::thread::spawn(move||{start_tx.send(()).unwrap();let result=service.handle_runtime("work.pause",payload,&store,&crate::executor::Executor::default(),&WorkSupervisor::default(),&Redactor);done_tx.send(result).unwrap();});
    start_rx.recv_timeout(Duration::from_secs(1)).unwrap();assert!(done_rx.recv_timeout(Duration::from_millis(40)).is_err());drop(gate);assert_eq!(done_rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap()["value"]["control"],"paused");worker.join().unwrap();
}

#[test]
fn concurrent_backend_prepare_has_one_pending_intent_and_one_completed_response(){
    let f=Fixture::new();let evidence=f.store.handle(Request{action:"evidence.import".into(),payload:json!({"assessmentId":f.aid,"name":"Concurrent metadata fixture","sourceDescription":"Synthetic fixture","content":"Independent local input","acknowledgeUnverifiedSource":true})}).unwrap();
    let mut input=f.draft();input.nodes=vec![NodeDraft{key:"review".into(),existing_node_id:None,purpose:"Prepare selected evidence".into(),expected_observation:"Stable metadata response".into(),depends_on_keys:vec![],intent:DraftIntent::AiInvestigation{inputs:vec![DraftInput::Evidence{evidence_id:evidence["id"].as_str().unwrap().into(),expected_raw_sha256:evidence["sha256"].as_str().unwrap().into()}],perspectives:vec![Perspective::Evidence]}}];let p=f.service.prepare(&f.store,input,&Redactor).unwrap();let committed=f.commit(&p);let j=&committed.value.jobs[0];
    let request=json!({"job":{"id":j.id,"generation":j.generation},"revision":committed.value.head,"idempotencyKey":model::id()});let ai=Arc::new(crate::ai::AiService::new(f.store.root.clone()).unwrap());let kali=Arc::new(crate::kali_bridge::KaliBridge::new(f.store.root.clone()).unwrap());let barrier=Arc::new(Barrier::new(3));let mut threads=vec![];
    for _ in 0..2{let store=f.store.clone();let service=f.service.clone();let request=request.clone();let ai=ai.clone();let kali=kali.clone();let barrier=barrier.clone();threads.push(std::thread::spawn(move||{barrier.wait();service.handle_backend("work.operation.prepare",request,&store,&crate::executor::Executor::default(),&WorkSupervisor::default(),&ai,&kali,None,&||panic!("metadata cannot call identity"),&Redactor)}));}
    barrier.wait();let results=threads.into_iter().map(|t|t.join().unwrap()).collect::<Vec<_>>();assert!(results.iter().any(Result::is_ok));for result in &results{if let Err(reason)=result{assert!(reason.starts_with("work_busy"),"{reason}");}}
    let replay=f.service.handle_backend("work.operation.prepare",request,&f.store,&crate::executor::Executor::default(),&WorkSupervisor::default(),&ai,&kali,None,&||panic!("replay cannot call identity"),&Redactor).unwrap();assert_eq!(replay["replayed"],true);
    for result in results.into_iter().flatten(){assert_eq!(result["receipt"],replay["receipt"]);assert_eq!(result["value"],replay["value"]);}assert_eq!(f.count("work_commands"),2);assert_eq!(f.count("work_attempts"),0);assert_eq!(f.count("work_leases"),0);
}

#[test]
fn cancel_retry_preparation_preserves_previous_terminal_attempt_at_capacity_and_boot(){
    let(_dir,store,aid,_manifest)=crate::capture::fixture::build(b"completed synthetic observation",crate::capture::CaptureState::Sealed);
    let (before,job_id,key)=store.work_transaction(|conn,_|{
        let head=work_store::head_for_assessment(conn,&aid)?.unwrap();let mut j=work_store::jobs_for_plan(conn,&head.plan_id)?.remove(0);let a=work_store::attempt(conn,&j.current_attempt.as_ref().unwrap().id)?;assert_eq!(a.view.state,AttemptState::Completed);
        let before=encode(&a)?;j.state=JobState::Preparing;j.generation+=1;work_store::save_job(conn,&j)?;let key=model::id();let receipt=CommandReceipt{id:model::id(),action:"work.retry.prepare".into(),entity_id:j.id.clone(),entity_generation:j.generation,request_digest:crate::core::hash("bounded in-progress retry fixture"),created_at:model::now()};work_store::insert_command(conn,&work_store::StoredCommand{assessment_id:aid.clone(),idempotency_key:key.clone(),review_id:None,result:work_store::CommandResult::Pending{receipt,entity_id:j.id.clone()}})?;Ok((before,j.id,key))
    }).unwrap();fill_event_capacity(&store,&aid,0);
    let service=WorkflowService::default();let supervisor=WorkSupervisor::default();let executor=crate::executor::Executor::default();let j=store.work_read(|conn,_|work_store::job(conn,&job_id)).unwrap();
    let canceled=runtime(&service,&store,&executor,&supervisor,"work.cancel",json!({"job":{"id":j.id,"generation":j.generation},"idempotencyKey":model::id(),"acknowledgeCancel":true}));assert_eq!(canceled["value"]["job"]["state"],"cancel_requested");
    store.work_read(|conn,_|{let j=work_store::job(conn,&job_id)?;assert_eq!(encode(&work_store::attempt(conn,&j.current_attempt.unwrap().id)?)?,before);Ok(())}).unwrap();
    let path=store.root.clone();drop(store);let store=Store::open(path).unwrap();WorkSupervisor::default().recover_after_boot(&store).unwrap();store.work_read(|conn,_|{let j=work_store::job(conn,&job_id)?;assert_eq!(j.state,JobState::Canceled);assert_eq!(encode(&work_store::attempt(conn,&j.current_attempt.unwrap().id)?)?,before);assert!(matches!(work_store::command_by_key(conn,&aid,&key)?.unwrap().result,work_store::CommandResult::Failed{..}));Ok(())}).unwrap();
}

#[test]
fn cancel_after_claim_before_binding_closes_as_not_started_without_an_unknown_effect(){
    let(_dir,store,service,supervisor,executor,aid,_)=local_fixture();let proposal=service.prepare(&store,local_draft(&aid),&Redactor).unwrap();service.commit(&store,commit_input(&proposal),&Redactor).unwrap();let plan=plan_json(&service,&store,&aid);let prepared=prepare_node(&service,&store,&executor,&supervisor,&plan,0);enqueue(&service,&store,&executor,&supervisor,&prepared,&plan["head"],&model::id());
    let dispatch=supervisor.next_native(&store).unwrap().unwrap();let permit=supervisor.arbiter().claim_queued(&store,dispatch.token).unwrap();let current=plan_json(&service,&store,&aid);assert_eq!(current["jobs"][0]["state"],"dispatching");
    let canceled=runtime(&service,&store,&executor,&supervisor,"work.cancel",json!({"job":job_reference(&current["jobs"][0]),"idempotencyKey":model::id(),"acknowledgeCancel":true}));assert_eq!(canceled["value"]["attempt"]["state"],"cancel_requested");assert_eq!(canceled["value"]["attempt"]["effect"],Value::Null);
    supervisor.arbiter().finish(permit,crate::work_supervisor::EffectTermination::NotStarted).unwrap();let final_view=plan_json(&service,&store,&aid);assert_eq!(final_view["jobs"][0]["state"],"canceled");assert_eq!(final_view["attempts"][0]["state"],"not_started");assert_eq!(final_view["attempts"][0]["cleanup"],"confirmed");assert!(store.snapshot().unwrap().executions.is_empty());assert!(supervisor.arbiter().ensure_quiescent(&store).is_ok());
}
