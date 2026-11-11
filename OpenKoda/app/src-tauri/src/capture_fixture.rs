//! Synthetic capture bytes in a real Store, with all production validators on.
//! No provider, network, WSL or target program is invoked by this fixture.
use super::*;
use crate::{core::Request, data_projection::{DataRedactor,RedactedText,RedactionMode}, executor::{self,Executor}, workflow::*, work_supervisor::{self,WorkSupervisor,EffectTermination}};
use serde_json::json;

struct FixedPlanText;
impl DataRedactor for FixedPlanText {
    fn mode(&self)->RedactionMode {RedactionMode::NoCredentialConfigured}
    fn redact(&self,text:&str)->RedactedText {RedactedText{text:text.into(),changed:false,secret_material:false}}
}

pub(crate) fn build(bytes:&[u8],state:CaptureState)->(tempfile::TempDir,Store,String,CaptureManifest) {
    assert!(bytes.len()<=MAX_CAPTURE_BYTES as usize);
    assert!(matches!(state,CaptureState::Open|CaptureState::Sealed|CaptureState::Interrupted));
    let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input");std::fs::create_dir(&input).unwrap();std::fs::write(input.join("selected.txt"),b"selected synthetic input").unwrap();
    let store=Store::open(dir.path().join("state")).unwrap();
    store.handle(Request{action:"settings.save".into(),payload:json!({"outputLimitKiB":4096})}).unwrap();
    let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Capture policy fixture","kind":"files","target":input.to_string_lossy()})}).unwrap();
    let aid=store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Synthetic stream fixture","kind":"files","target":project["target"],"objective":"Validate local capture policy without tool output","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&["local.inventory.v1"])})}).unwrap()["id"].as_str().unwrap().to_owned();
    let workflow=WorkflowService::default();
    let prepared=workflow.prepare(&store,PlanPrepareRequest{assessment_id:aid.clone(),expected_head:None,expected_generation:0,goal:"Review a synthetic capture fixture".into(),nodes:vec![NodeDraft{key:"observe".into(),existing_node_id:None,purpose:"Observe the explicitly selected fixture".into(),expected_observation:"A bounded capture for policy tests".into(),depends_on_keys:vec![],intent:DraftIntent::Native{adapter:NativeAdapter::LocalInventory,target_ref:None,ports:None}}],limits:PlanLimits{max_attempts_total:1,max_capture_bytes_total:MAX_CAPTURE_BYTES}},&FixedPlanText).unwrap();
    let committed=workflow.commit(&store,PlanCommitRequest{review:prepared.review,idempotency_key:model::id(),acknowledge_plan:true},&FixedPlanText).unwrap();
    let supervisor=WorkSupervisor::default();let executor=Executor::default().with_arbiter(supervisor.shared_arbiter());let job=&committed.value.jobs[0];
    // Real preparation journals Pending before its bounded local manifest read.
    let prepared=workflow.handle_runtime("work.operation.prepare",json!({"job":{"id":job.id,"generation":job.generation},"revision":job.revision,"idempotencyKey":model::id()}),&store,&executor,&supervisor,&FixedPlanText).unwrap();
    let native=&prepared["value"];
    workflow.handle_runtime("work.operation.enqueue",json!({"kind":"native","job":{"id":native["job"]["id"],"generation":native["job"]["generation"]},"revision":native["job"]["revision"],"compiled":native["compiled"],"idempotencyKey":model::id(),"acknowledgeExecution":true}),&store,&executor,&supervisor,&FixedPlanText).unwrap();
    let dispatch=supervisor.next_native(&store).unwrap().expect("queued native fixture");
    let mut permit=supervisor.arbiter().claim_queued(&store,dispatch.token).unwrap();
    let (plan,execution,manifest)=store.work_transaction(|conn,s|{
        let context=work_supervisor::validate_queued_native_locked(conn,s,&permit,&dispatch.plan_id,&dispatch.digest)?;
        let(plan,execution)=executor::admit_locked(s,&dispatch.plan_id,&dispatch.digest,&model::id())?;
        work_supervisor::bind_effect_locked(conn,&permit,&EffectRef::NativeExecution{id:execution.id.clone()})?;
        let mut manifest=create_native_locked(conn,s,&execution,&context.attempt_id,context.max_capture_bytes)?;
        if std::str::from_utf8(bytes).is_err(){manifest.encoding=CaptureEncoding::Unknown;manifest.generation+=1;persist(conn,&mut manifest)?;}
        work_supervisor::mark_native_running_locked(conn,&permit,&execution,Some(&manifest.id))?;
        Ok((plan,execution,manifest))
    }).unwrap();
    permit.acknowledge_bound(EffectRef::NativeExecution{id:execution.id.clone()});
    for (index,chunk) in bytes.chunks(MAX_CHUNK_BYTES).enumerate(){store.capture_transaction(|conn|append_stream_locked(conn,&manifest.id,(index*MAX_CHUNK_BYTES) as u64,chunk)).unwrap();}
    let manifest=if state==CaptureState::Open {
        let current=store.work_read(|conn,_|load_locked(conn,&manifest.id)).unwrap();
        supervisor.arbiter().finish(permit,EffectTermination::Unknown).unwrap();current
    }else{
        let sealed=store.work_transaction(|conn,s|{
            let interrupted=state==CaptureState::Interrupted;
            let sealed=seal_locked(conn,&manifest.id,if interrupted{None}else{Some(bytes.len() as u64)},None,interrupted)?;
            let completion=executor::finish_locked(s,&plan,&execution,if interrupted{Err("Synthetic interrupted capture fixture".into())}else{Ok(json!({"adapter":"local.inventory.v1","syntheticCaptureFixture":true}).to_string())},false)?;
            work_supervisor::finish_native_locked(conn,s,&permit,&completion,Some(&sealed))?;
            work_supervisor::finish_permit_locked(conn,&permit,EffectTermination::Stopped)?;Ok(sealed)
        }).unwrap();
        permit.acknowledge_finished();drop(permit);sealed
    };
    (dir,store,aid,manifest)
}

#[test]
fn fixture_roundtrips_through_store_and_preserves_live_and_sealed_states() {
    for state in [CaptureState::Open,CaptureState::Sealed,CaptureState::Interrupted] {
        let(_dir,store,aid,manifest)=build(b"synthetic captured bytes",state);
        assert_eq!(manifest.state,state);
        store.work_read(|conn,s|{validate_capture_domain(conn,s)?;assert_eq!(list_locked(conn,&aid,None)?.len(),1);assert_eq!(preserved_locked(conn,&manifest)?,b"synthetic captured bytes");Ok(())}).unwrap();
    }
}
