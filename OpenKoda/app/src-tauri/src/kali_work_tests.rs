use crate::work_supervisor::{EffectOwner,EffectOwnerKind,EffectTermination,WorkArbiter};

fn owner(aid:&str,kind:EffectOwnerKind)->EffectOwner{EffectOwner{id:model::id(),assessment_id:Some(aid.into()),kind}}

#[test]
fn native_and_ai_ownership_refuse_kali_before_identity_or_launch(){
    let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());
    let arbiter=Arc::new(WorkArbiter::default());let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});
    let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap().with_arbiter(arbiter.clone());
    for kind in [EffectOwnerKind::Native,EffectOwnerKind::AiRequest,EffectOwnerKind::AiInvestigation] {
        let permit=arbiter.claim_direct(&store,owner(&aid,kind)).unwrap();let identity_calls=std::sync::atomic::AtomicUsize::new(0);
        let error=bridge.handle_with_identity("kali.mcp.diagnose",json!({"assessmentId":aid}),&store,||{identity_calls.fetch_add(1,Ordering::SeqCst);Ok(model::id())},None).unwrap_err();
        assert!(error.contains("work_busy"));assert_eq!(identity_calls.load(Ordering::SeqCst),0);
        assert_eq!(launcher.launches.load(Ordering::SeqCst),0);assert!(bridge.runs().unwrap().is_empty());
        arbiter.finish(permit,EffectTermination::NotStarted).unwrap();
    }
}

#[test]
fn active_kali_lease_excludes_native_and_ai_until_confirmed_cleanup(){
    let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());let(started,ready)=mpsc::channel();
    let arbiter=Arc::new(WorkArbiter::default());
    let bridge=Arc::new(KaliBridge::with_launcher(store.root.clone(),Arc::new(TestLauncher{launches:0.into(),clean:true,block:true,started:Some(started)})).unwrap().with_arbiter(arbiter.clone()));
    let worker_bridge=bridge.clone();let worker_store=store.clone();let worker_aid=aid.clone();
    let thread=std::thread::spawn(move||worker_bridge.handle("kali.mcp.diagnose",json!({"assessmentId":worker_aid}),&worker_store,&model::id()).unwrap());
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    for kind in [EffectOwnerKind::Native,EffectOwnerKind::AiRequest,EffectOwnerKind::AiInvestigation]{assert!(arbiter.claim_direct(&store,owner(&aid,kind)).is_err());}
    bridge.handle("kali.mcp.cancel",json!({"assessmentId":aid}),&store,"").unwrap();
    assert_eq!(thread.join().unwrap()["cleanupConfirmed"],true);
    let permit=arbiter.claim_direct(&store,owner(&aid,EffectOwnerKind::Native)).unwrap();arbiter.finish(permit,EffectTermination::NotStarted).unwrap();
}

#[test]
fn identity_failure_preserves_data_review_and_does_not_create_linux_session(){
    let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());let eid=model::id();let environment=model::id();
    store.mutate(|s|{let content="eval(fixture)\n".to_owned();s.assessments[0].evidence.push(Evidence{id:eid.clone(),name:"fixture.txt".into(),sha256:hash(&content),content,created_at:model::now()});Ok(())}).unwrap();
    let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});
    let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();
    let ai=crate::ai::AiService::new(store.root.clone()).unwrap().with_data_policy(bridge.data_policy.clone());let redactor=ai.data_redactor();
    crate::data_policy::test_support::authorize(&store,&bridge.data_policy,&aid,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
    let preview=bridge.handle_data_prepare("kali.data.prepare",json!({"assessmentId":aid,"evidenceId":eid,"operation":"scan","packageSensitivity":"internal"}),&store,&environment,&redactor).unwrap();
    let payload=json!({"assessmentId":aid,"evidenceId":eid,"dataReview":preview["dataReview"],"acknowledgeLocalTransfer":true});
    let error=bridge.handle_with_identity("kali.mcp.scanEvidence",payload.clone(),&store,||Err("fixture_not_ready".into()),Some(&ai)).unwrap_err();
    assert_eq!(error,"fixture_not_ready");assert_eq!(launcher.launches.load(Ordering::SeqCst),0);
    assert!(bridge.runs().unwrap().is_empty());assert!(store.snapshot().unwrap().data_use_receipts.is_empty());
    bridge.admit_data_before_identity("kali.mcp.scanEvidence",&payload,&store,&redactor).unwrap();
    let result=bridge.handle_reviewed("kali.mcp.scanEvidence",payload,&store,&environment,&ai).unwrap();
    assert_eq!(result["run"]["status"],"completed");assert_eq!(launcher.launches.load(Ordering::SeqCst),1);
    assert_eq!(store.snapshot().unwrap().data_use_receipts.len(),1);
}

struct SequenceLauncher {clean:Mutex<VecDeque<bool>>,calls:Arc<Mutex<Vec<Value>>>}
struct RecordingTransport {inner:TestTransport,calls:Arc<Mutex<Vec<Value>>>}
impl Launcher for SequenceLauncher {
    fn launch(&self,run:&BridgeRun,_:&AtomicBool)->Result<Box<dyn Transport>,String>{
        let clean=self.clean.lock().unwrap().pop_front().ok_or("unexpected extra Linux launch")?;
        Ok(Box::new(RecordingTransport{inner:TestTransport{inner:Fake{frames:frames(run,diagnostic(run)),sent:vec![],clean},block:false},calls:self.calls.clone()}))
    }
}
impl Transport for RecordingTransport {
    fn send(&mut self,value:Value,deadline:Instant,cancel:&AtomicBool)->Result<(),String>{
        if value["method"]=="tools/call"{self.calls.lock().unwrap().push(value["params"].clone());}
        self.inner.send(value,deadline,cancel)
    }
    fn receive(&mut self,deadline:Instant,cancel:&AtomicBool)->Result<Value,String>{self.inner.receive(deadline,cancel)}
    fn finish(&mut self,run:&BridgeRun,deadline:Instant)->FinishOutcome{self.inner.finish(run,deadline)}
}

#[test]
fn unknown_probe_keeps_original_lease_until_entire_family_is_stopped_without_repeating_scan(){
    let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());let environment=model::id();let eid=model::id();
    store.mutate(|s|{let content="eval(fixture)\n".to_owned();s.assessments[0].evidence.push(Evidence{id:eid.clone(),name:"fixture.txt".into(),sha256:hash(&content),content,created_at:model::now()});Ok(())}).unwrap();
    let calls=Arc::new(Mutex::new(vec![]));let launcher=Arc::new(SequenceLauncher{clean:Mutex::new(VecDeque::from([false,false,true,true])),calls:calls.clone()});
    let arbiter=Arc::new(WorkArbiter::default());let bridge=KaliBridge::with_launcher(store.root.clone(),launcher).unwrap().with_arbiter(arbiter.clone());
    let initial=test_scan(&bridge,json!({"assessmentId":aid,"evidenceId":eid,"acknowledgeLocalTransfer":true}),&store,&environment).unwrap();let root=initial["run"]["id"].as_str().unwrap();
    assert_eq!(initial["run"]["status"],"unknown");
    let first=bridge.handle("kali.mcp.reconcile",json!({"assessmentId":aid,"runId":root}),&store,&environment).unwrap();
    let child=first["probeRunId"].as_str().unwrap();assert_eq!(first["cleanupConfirmed"],false);
    assert!(arbiter.claim_direct(&store,owner(&aid,EffectOwnerKind::AiRequest)).is_err());
    let second=bridge.handle("kali.mcp.reconcile",json!({"assessmentId":aid,"runId":root}),&store,&environment).unwrap();assert_eq!(second["cleanupConfirmed"],true);
    assert!(arbiter.claim_direct(&store,owner(&aid,EffectOwnerKind::Native)).is_err());
    let third=bridge.handle("kali.mcp.reconcile",json!({"assessmentId":aid,"runId":child}),&store,&environment).unwrap();assert_eq!(third["cleanupConfirmed"],true);
    assert!(!bridge.has_unresolved());assert_eq!(bridge.runs().unwrap().len(),4);
    let calls=calls.lock().unwrap();assert_eq!(calls.len(),4);assert_eq!(calls[0]["name"],"scan_bytes");
    for (i,call) in calls.iter().enumerate().skip(1){assert_eq!(call["name"],"runtime_diagnostics");assert_eq!(call["arguments"]["inspectSessionId"],if i==3{child}else{root});assert!(call["arguments"].get("base64").is_none());}
    let permit=arbiter.claim_direct(&store,owner(&aid,EffectOwnerKind::AiRequest)).unwrap();arbiter.finish(permit,EffectTermination::NotStarted).unwrap();
}

#[test]
fn recorded_cleanup_recovers_lost_final_commit_without_identity_probe(){
    let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());let environment=model::id();
    let launcher=Arc::new(TestLauncher{launches:0.into(),clean:false,block:false,started:None});let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();
    let result=bridge.handle("kali.mcp.diagnose",json!({"assessmentId":aid}),&store,&environment).unwrap();
    let mut run=bridge.find(result["run"]["id"].as_str().unwrap()).unwrap();run.status="failed".into();run.cleanup_confirmed=true;run.finished_at=Some(model::now());bridge.save(&run).unwrap();
    let recovered=bridge.handle_with_identity("kali.mcp.reconcile",json!({"assessmentId":aid,"runId":run.id}),&store,||panic!("durable cleanup needs no fresh process"),None).unwrap();
    assert_eq!(recovered["cleanupConfirmed"],true);assert!(recovered["probeRunId"].is_null());assert_eq!(launcher.launches.load(Ordering::SeqCst),1);
    let permit=bridge.arbiter.claim_direct(&store,owner(&aid,EffectOwnerKind::Native)).unwrap();bridge.arbiter.finish(permit,EffectTermination::NotStarted).unwrap();
}

#[test]
fn legacy_journal_gate_is_read_only_and_fails_closed_on_malformed_or_other_unknown(){
    let dir=tempfile::tempdir().unwrap();assert!(!has_unresolved_work(dir.path()).unwrap());assert!(!dir.path().join("kali-bridge.sqlite3").exists());
    let bridge=KaliBridge::new(dir.path().into()).unwrap();let mut first=run();first.status="unknown".into();bridge.save(&first).unwrap();
    let mut second=run();second.status="unknown".into();bridge.save(&second).unwrap();assert!(has_unresolved_work(dir.path()).unwrap());
    first.status="failed".into();first.cleanup_confirmed=true;first.finished_at=Some(model::now());bridge.save(&first).unwrap();assert!(has_unresolved_work(dir.path()).unwrap());
    second.status="failed".into();second.cleanup_confirmed=true;second.finished_at=Some(model::now());bridge.save(&second).unwrap();assert!(!has_unresolved_work(dir.path()).unwrap());
    bridge.db.lock().unwrap().execute("UPDATE bridge_runs SET data=json_set(data,'$.unexpectedField',1) WHERE id=?1",[second.id]).unwrap();assert!(has_unresolved_work(dir.path()).is_err());
}

#[test]
fn restart_proves_new_intent_not_started_but_never_infers_cleanup_after_launch(){
    for stage in ["identity","prepared","bound","launched","legacy"] {
        let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());let reserved=model::id();
        let old=Arc::new(WorkArbiter::default());let bridge=KaliBridge::new(store.root.clone()).unwrap().with_arbiter(old.clone());
        let kind=if stage=="legacy"{EffectOwnerKind::KaliDiagnostic}else{EffectOwnerKind::KaliDiagnosticIntent};
        let mut permit=old.claim_direct(&store,EffectOwner{id:reserved.clone(),assessment_id:Some(aid.clone()),kind}).unwrap();
        if matches!(stage,"prepared"|"bound"|"launched") {
            let mut record=run();record.id=reserved.clone();record.assessment_id=aid.clone();record.status="starting".into();record.launch_attempted=stage=="launched";bridge.save(&record).unwrap();
            if stage!="prepared"{old.bind_effect(&mut permit,crate::workflow::EffectRef::KaliRun{id:reserved.clone()}).unwrap();}
        }
        drop(permit);drop(bridge);
        let supervisor=crate::work_supervisor::WorkSupervisor::default();supervisor.recover_after_boot(&store).unwrap();
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});
        let reopened=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap().with_arbiter(supervisor.shared_arbiter());
        reopened.recover_work_after_boot(&store).unwrap();assert_eq!(launcher.launches.load(Ordering::SeqCst),0);
        let available=supervisor.shared_arbiter().claim_direct(&store,owner(&aid,EffectOwnerKind::Native));
        if matches!(stage,"launched"|"legacy") {
            assert!(available.is_err(),"{stage}: lack of proof must retain the lease");
            if stage=="launched"{assert_eq!(reopened.find(&reserved).unwrap().status,"unknown");}
        }else{
            supervisor.shared_arbiter().finish(available.unwrap(),EffectTermination::NotStarted).unwrap();
            if stage!="identity"{let record=reopened.find(&reserved).unwrap();assert_eq!(record.status,"failed");assert!(record.cleanup_confirmed);assert!(!record.launch_attempted);}
        }
    }
}
