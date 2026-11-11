// Workflow, policy and core journals are real; the only process boundary is a fake.
use crate::work_supervisor::{WorkSupervisor,QueuedDispatch};
use crate::workflow::{WorkflowService,AttemptState};

struct BoundLauncher {store:Arc<Store>,calls:std::sync::atomic::AtomicUsize}
impl Launcher for BoundLauncher {
    fn launch(&self,run:&BridgeRun,_:&AtomicBool)->Result<Box<dyn Transport>,String>{
        self.store.work_read(|db,_|{
            let bound:u32=db.query_row("SELECT count(*) FROM work_leases WHERE state='bound' AND effect_kind='kali_run' AND effect_id=?1",[&run.id],|r|r.get(0)).map_err(db_error)?;
            assert_eq!(bound,1,"no Linux spawn before the native binding");
            let captures:u32=db.query_row("SELECT count(*) FROM capture_manifests WHERE state='open'",[],|r|r.get(0)).map_err(db_error)?;assert_eq!(captures,1);Ok(())
        })?;
        self.calls.fetch_add(1,Ordering::SeqCst);
        Ok(Box::new(TestTransport{inner:Fake{frames:frames(run,diagnostic(run)),sent:vec![],clean:true},block:false}))
    }
}
struct QueuedFixture {
    _dir:tempfile::TempDir,store:Arc<Store>,aid:String,eid:String,environment:String,
    bridge:KaliBridge,ai:crate::ai::AiService,launcher:Arc<BoundLauncher>,supervisor:WorkSupervisor,service:WorkflowService,executor:crate::executor::Executor,
}
impl QueuedFixture {
    fn new()->Self{
        let dir=tempfile::tempdir().unwrap();let(store,aid)=setup_store(dir.path());let eid=model::id();let environment=model::id();
        store.mutate(|s|{let content="eval(synthetic_fixture)\n".to_owned();s.assessments[0].evidence.push(Evidence{id:eid.clone(),name:"fixture.txt".into(),sha256:hash(&content),content,created_at:model::now()});Ok(())}).unwrap();
        let supervisor=WorkSupervisor::default();let launcher=Arc::new(BoundLauncher{store:store.clone(),calls:0.into()});
        let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap().with_arbiter(supervisor.shared_arbiter());
        let ai=crate::ai::AiService::new(store.root.clone()).unwrap().with_data_policy(bridge.data_policy.clone()).with_arbiter(supervisor.shared_arbiter());
        let redactor=ai.data_redactor();crate::data_policy::test_support::authorize(&store,&bridge.data_policy,&aid,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
        let executor=crate::executor::Executor::default().with_arbiter(supervisor.shared_arbiter());
        Self{_dir:dir,store,aid,eid,environment,bridge,ai,launcher,supervisor,service:WorkflowService::default(),executor}
    }
    fn call(&self,action:&str,payload:Value)->Result<Value,String>{
        let redactor=self.ai.data_redactor();
        if matches!(action,"work.plan.prepare"|"work.plan.commit"|"work.plan.get"|"work.queue"|"work.events"){return self.service.handle(action,payload,&self.store,&redactor);}
        self.service.handle_backend(action,payload,&self.store,&self.executor,&self.supervisor,&self.ai,&self.bridge,Some(&self.environment),&||Ok(self.environment.clone()),&redactor)
    }
    fn queued(&self)->(Value,Value){
        self.queued_operation("hash")
    }
    fn queued_operation(&self,operation:&str)->(Value,Value){
        let sha=self.store.snapshot().unwrap().assessments.iter().find(|a|a.id==self.aid).unwrap().evidence.iter().find(|e|e.id==self.eid).unwrap().sha256.clone();
        let p=self.call("work.plan.prepare",json!({"assessmentId":self.aid,"expectedHead":null,"expectedGeneration":0,"goal":"Preserve one selected hash with a dependent human review","nodes":[
            {"key":"hash","existingNodeId":null,"purpose":"Analyze selected fixture bytes","expectedObservation":"Bounded observation","dependsOnKeys":[],"intent":{"kind":"kali_evidence","operation":operation,"input":{"kind":"evidence","evidenceId":self.eid,"expectedRawSha256":sha}}},
            {"key":"human","existingNodeId":null,"purpose":"Inspect the hash observation","expectedObservation":"Explicit decision","dependsOnKeys":["hash"],"intent":{"kind":"human_review","inputs":[{"kind":"dependency_output","nodeKey":"hash","outputRole":"evidence"}]}}
        ],"limits":{"maxAttemptsTotal":4,"maxCaptureBytesTotal":if operation=="scan"{68*1024}else{4096}}})).unwrap();
        let committed=self.call("work.plan.commit",json!({"review":p["review"],"idempotencyKey":model::id(),"acknowledgePlan":true})).unwrap();let revision=&committed["value"]["head"];let job=&committed["value"]["jobs"][0];
        let prepared=self.call("work.operation.prepare",json!({"job":{"id":job["id"],"generation":job["generation"]},"revision":revision,"idempotencyKey":model::id()})).unwrap();let job=&prepared["value"]["job"];
        let review=self.bridge.handle_data_prepare("kali.data.prepare",json!({"assessmentId":self.aid,"evidenceId":self.eid,"operation":operation,"packageSensitivity":"internal"}),&self.store,&self.environment,&self.ai.data_redactor()).unwrap();
        let input=json!({"kind":"kali_evidence","job":{"id":job["id"],"generation":job["generation"]},"revision":revision,"dataReview":review["dataReview"],"operation":operation,"packageSensitivity":"internal","idempotencyKey":model::id(),"acknowledgeLocalTransfer":true});
        self.call("work.operation.enqueue",input.clone()).unwrap();(input,review)
    }
    fn next(&self)->(crate::work_supervisor::QueuedAttemptToken,QueuedKaliGrant){let Some(QueuedDispatch::Kali{token,grant})=self.supervisor.next_dispatch(&self.store,&self.service).unwrap()else{panic!("queued Kali dispatch expected")};(token,grant)}
    fn view(&self)->Value{self.call("work.plan.get",json!({"assessmentId":self.aid,"revisionId":null})).unwrap()["plan"].clone()}
}

#[test]
fn queued_hash_binds_before_spawn_seals_with_result_and_releases_dependent_once(){
    let f=QueuedFixture::new();let(input,_)=f.queued();let(token,grant)=f.next();
    let completion=f.bridge.run_queued_evidence(&f.store,token,grant,||Ok(f.environment.clone()),&f.ai).unwrap();
    assert_eq!(completion.outcome(),AttemptState::Completed);assert_eq!(f.launcher.calls.load(Ordering::SeqCst),1);
    let view=f.view();assert_eq!(view["jobs"][0]["state"],"completed");assert_eq!(view["jobs"][1]["state"],"awaiting_preparation");
    f.store.work_read(|db,s|{let captures=crate::capture::list_locked(db,&f.aid,None)?;assert_eq!(captures.len(),1);assert_eq!(captures[0].state,crate::capture::CaptureState::Sealed);assert_eq!(captures[0].stream,crate::capture::CaptureStream::AdapterRecords);assert!(captures[0].preserved_bytes>0);assert_eq!(s.assessments[0].evidence.len(),2);assert_eq!(s.data_use_receipts.len(),1);Ok(())}).unwrap();
    assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());assert_eq!(f.call("work.operation.enqueue",input).unwrap()["replayed"],true);assert_eq!(f.launcher.calls.load(Ordering::SeqCst),1);
}

#[test]
fn queued_review_is_not_direct_authority_even_before_dispatch(){
    let f=QueuedFixture::new();let(_,review)=f.queued();let identities=std::sync::atomic::AtomicUsize::new(0);
    let error=f.bridge.handle_with_identity("kali.mcp.hashEvidence",json!({"assessmentId":f.aid,"evidenceId":f.eid,"dataReview":review["dataReview"],"acknowledgeLocalTransfer":true}),&f.store,||{identities.fetch_add(1,Ordering::SeqCst);Ok(f.environment.clone())},Some(&f.ai)).unwrap_err();
    assert!(error.contains("work_owned_operation"),"{error}");assert_eq!(identities.load(Ordering::SeqCst),0);assert_eq!(f.launcher.calls.load(Ordering::SeqCst),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
}

#[test]
fn queued_identity_failure_keeps_data_review_unconsumed_and_dependency_blocked(){
    let f=QueuedFixture::new();f.queued();let(token,grant)=f.next();
    assert_eq!(f.bridge.run_queued_evidence(&f.store,token,grant,||Err("fixture_identity".into()),&f.ai).err().unwrap(),"fixture_identity");
    assert_eq!(f.launcher.calls.load(Ordering::SeqCst),0);assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());assert!(f.bridge.runs().unwrap().is_empty());assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
    assert_ne!(f.view()["jobs"][1]["state"],"awaiting_preparation");
}

#[test]
fn failed_atomic_final_commit_retains_unknown_binding_and_does_not_publish_evidence(){
    let f=QueuedFixture::new();f.queued();let(token,grant)=f.next();
    f.store.fail_commit_when(|s|s.assessments[0].evidence.len()>1);
    assert!(f.bridge.run_queued_evidence(&f.store,token,grant,||Ok(f.environment.clone()),&f.ai).is_err());
    assert_eq!(f.launcher.calls.load(Ordering::SeqCst),1);assert_eq!(f.store.snapshot().unwrap().assessments[0].evidence.len(),1);assert_eq!(f.view()["jobs"][0]["state"],"unknown");assert_ne!(f.view()["jobs"][1]["state"],"awaiting_preparation");
    f.store.work_read(|db,_|{let captures=crate::capture::list_locked(db,&f.aid,None)?;assert_eq!(captures[0].state,crate::capture::CaptureState::Open);assert_eq!(captures[0].preserved_bytes,0);Ok(())}).unwrap();assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_err());
    let attempt=f.view()["attempts"][0]["id"].as_str().unwrap().to_owned();
    let permit=f.supervisor.arbiter().claim_queued_reconciliation(&f.store,&attempt,crate::workflow::Backend::Kali).unwrap();
    let rejected=f.bridge.reconcile_work_evidence(&f.store,&permit,&||panic!("terminal journal must not relaunch a process"),&f.ai,|db,s,p,c|{crate::work_supervisor::finish_kali_recovery_locked(db,s,p,c)?;Err("fixture_command_commit_failed".into())});drop(permit);
    assert_eq!(rejected.err().unwrap(),"fixture_command_commit_failed");assert_eq!(f.view()["jobs"][0]["state"],"unknown");assert_eq!(f.store.snapshot().unwrap().assessments[0].evidence.len(),1);
    let permit=f.supervisor.arbiter().claim_queued_reconciliation(&f.store,&attempt,crate::workflow::Backend::Kali).unwrap();
    let completion=f.bridge.reconcile_work_evidence(&f.store,&permit,&||panic!("terminal journal must not relaunch a process"),&f.ai,|db,s,p,c|crate::work_supervisor::finish_kali_recovery_locked(db,s,p,c)).unwrap();drop(permit);
    assert_eq!(completion.outcome(),AttemptState::Completed);assert_eq!(f.launcher.calls.load(Ordering::SeqCst),1);assert_eq!(f.store.snapshot().unwrap().assessments[0].evidence.len(),2);assert_eq!(f.view()["jobs"][1]["state"],"awaiting_preparation");assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
}

#[test]
fn compressed_bootstrap_keeps_full_source_hash_and_wire_bounds(){
    let envelope=bootstrap(&run()).unwrap();let bytes=STANDARD.decode(envelope["sourceZlibBase64"].as_str().unwrap()).unwrap();let mut decoded=String::new();flate2::read::ZlibDecoder::new(bytes.as_slice()).read_to_string(&mut decoded).unwrap();assert_eq!(decoded,SOURCE);assert_eq!(hash(&decoded),envelope["sourceSha256"]);assert_eq!(decoded.len() as u64,envelope["sourceBytes"].as_u64().unwrap());assert!(serde_json::to_vec(&envelope).unwrap().len()+1<=65536);assert!(decoded.len()<=128*1024);
}

#[derive(Clone,Copy,PartialEq,Eq)]
enum ScanBehavior {Complete,CancelProducer,CancelCollector,UnknownCollector,Absent,RetirePreCommit,RetirePartial,RetireLostResponse,RetireUnknownCleanup}
struct SpoolState {raw:Vec<u8>,manifest:Value,producer:Option<BridgeRun>,receipt:Value,calls:Vec<String>,behavior:ScanBehavior,commitment:Value,metadata_retired:bool,retire_apply_count:usize}
struct SpoolLauncher {store:Arc<Store>,state:Arc<Mutex<SpoolState>>}
struct SpoolTransport {inner:Fake,run:BridgeRun,state:Arc<Mutex<SpoolState>>,cancel_after_read:bool}
impl Launcher for SpoolLauncher {
    fn launch(&self,run:&BridgeRun,cancel:&AtomicBool)->Result<Box<dyn Transport>,String>{
        assert!(!cancel.load(Ordering::SeqCst),"a new bounded collection phase has its own cancellation signal");
        let root=run.reconciliation_of.as_deref().unwrap_or(&run.id);
        self.store.work_read(|db,_|{let bound:u32=db.query_row("SELECT count(*) FROM work_leases WHERE state IN ('bound','unknown') AND effect_kind='kali_run' AND effect_id=?1",[root],|row|row.get(0)).map_err(db_error)?;assert_eq!(bound,1,"every producer/control needs the same durable resource owner");Ok(())})?;
        let mut state=self.state.lock().unwrap();state.calls.push(run.tool.clone());
        if run.tool=="scan_bytes" {
            self.store.work_read(|db,_|{let captures=crate::capture::list_locked(db,&run.assessment_id,None)?;assert_eq!(captures.len(),2);assert!(captures.iter().all(|m|m.state==crate::capture::CaptureState::Open));Ok(())})?;
            let binding=capture_io::producer_binding(run)?;let crate::workflow::ResultRef::Evidence{id,raw_sha256}=&binding.input_refs[0]else{panic!("fixture input")};
            let body=json!({"schema":"openkoda.kali-spool.v1","captureId":binding.captures[0].capture_id,"attemptId":binding.attempt_id,"assessmentId":binding.assessment_id,"environmentId":binding.environment_id,"producerSessionId":binding.run_id,"ownerProfileFingerprint":binding.owner_profile_fingerprint,"executorSourceSha256":binding.executor_source_sha256,"catalogVersion":binding.catalog_version,"catalogSha256":binding.catalog_sha256,"parserVersion":binding.parser_version,"inputEvidenceId":id,"inputSha256":raw_sha256,"stream":"tool_stdout","childRole":"scan","toolBinarySha256":"a".repeat(64),"producer":{"pid":2000,"startTicks":"1000","bootId":model::id()},"child":{"pid":null,"startTicks":null},"phase":"sealed","generation":5,"maxBytes":65536,"preservedBytes":state.raw.len(),"prefixSha256":hash(std::str::from_utf8(&state.raw).unwrap()),"observedBytes":state.raw.len(),"reachedEof":true,"lossReason":null,"encoding":"utf8"});
            state.manifest=json!({"digest":hash(&body.to_string()),"body":body});state.producer=Some(run.clone());
        }
        let mut initial=frames(run,diagnostic(run));initial.pop_back();
        let clean=!(run.tool=="capture_collect"&&state.behavior==ScanBehavior::UnknownCollector||matches!(&run.work_context,Some(capture_io::KaliRunContext::RetireControl{intent:capture_io::KaliControlIntent::RetireApply{..},..}))&&state.behavior==ScanBehavior::RetireUnknownCleanup);
        Ok(Box::new(SpoolTransport{inner:Fake{frames:initial,sent:vec![],clean},run:run.clone(),state:self.state.clone(),cancel_after_read:false}))
    }
}
impl Transport for SpoolTransport {
    fn send(&mut self,value:Value,deadline:Instant,cancel:&AtomicBool)->Result<(),String>{
        check_wait(deadline,cancel)?;
        if value["method"]=="tools/call" {
            let mut state=self.state.lock().unwrap();let args=&value["params"]["arguments"];let mut result=metadata(&self.run);
            result["operation"]=json!(self.run.tool);
            match self.run.tool.as_str() {
                "scan_bytes"=>{
                    result=scanned(&metadata(&self.run),args);
                    if state.behavior==ScanBehavior::CancelProducer {result["termination"]=json!("canceled");result["partial"]=json!(true);result["countsComplete"]=json!(false);}
                },
                "capture_collect"=>{
                    result["kind"]=args["kind"].clone();
                    if args["kind"]=="describe" {
                        result["producerQuiescent"]=json!(true);result["purgeReceipt"]=state.receipt.clone();
                        if state.behavior==ScanBehavior::Absent {
                            let producer=state.producer.as_ref().unwrap();let binding=capture_io::producer_binding(producer)?;
                            let body=json!({"schema":"openkoda.kali-spool-absence.v1","assessmentId":producer.assessment_id,"environmentId":producer.environment_id,"captureId":binding.captures[0].capture_id,"producerSessionId":producer.id,"ownerProfileFingerprint":binding.owner_profile_fingerprint,"producerDispatchDeadline":producer.dispatch_before_unix_seconds,"directoryAbsent":true});
                            result["availability"]=json!("absent");result["manifest"]=Value::Null;result["absence"]=json!({"digest":hash(&body.to_string()),"body":body});
                        }else{result["availability"]=json!("present");result["manifest"]=state.manifest.clone();result["absence"]=Value::Null;}
                    }else{
                        assert_eq!(args["expectedManifestDigest"],state.manifest["digest"]);let start=args["offset"].as_u64().unwrap()as usize;let end=(start+4096).min(state.raw.len());
                        result["manifestDigest"]=state.manifest["digest"].clone();result["offset"]=json!(start);result["bytes"]=json!(end-start);result["base64"]=json!(STANDARD.encode(&state.raw[start..end]));result["eof"]=json!(end==state.raw.len());
                        self.cancel_after_read=state.behavior==ScanBehavior::CancelCollector;
                    }
                },
                "capture_purge"=>{
                    let body=&state.manifest["body"];
                    let receipt=json!({"schema":"openkoda.kali-spool-purge.v2","captureId":args["captureId"],"producerSessionId":args["producerSessionId"],"environmentId":self.run.environment_id,"assessmentId":self.run.assessment_id,"manifestDigest":args["expectedManifestDigest"],"purgeReceiptId":args["purgeReceiptId"],"preservedBytes":body["preservedBytes"],"prefixSha256":body["prefixSha256"],"state":"purged","ownerProfileFingerprint":body["ownerProfileFingerprint"],"previousReceiptId":args["previousReceiptId"],"previousReceiptDigest":args["previousReceiptDigest"]});
                    state.receipt=receipt.clone();result["receipt"]=receipt;
                },
                "capture_retire"=>{
                    let Some(capture_io::KaliRunContext::RetireControl{intent,..})=&self.run.work_context else{panic!("retire control context")};assert_eq!(*args,intent.control());
                    if args["kind"]=="apply"{
                        state.retire_apply_count+=1;
                        assert_eq!(args["expectedStateDigest"],hash(&retire_fixture_observation(&state).to_string()));
                        if state.behavior==ScanBehavior::RetirePreCommit{return Err("fixture_retire_before_commit".into());}
                        let body=&state.manifest["body"];
                        state.commitment=json!({"schema":"openkoda.kali-retired-capture.v1","proof":{"captureId":args["captureId"],"attemptId":body["attemptId"],"ownerProfileFingerprint":args["ownerProfileFingerprint"],"manifestDigest":args["expectedManifestDigest"],"purgeReceiptId":args["purgeReceiptId"],"purgeReceiptSha256":args["expectedPurgeReceiptSha256"]},"attempt":{"retireReceiptId":args["retireReceiptId"],"previousReceiptId":args["previousReceiptId"],"previousReceiptDigest":args["previousReceiptDigest"],"reviewedStateDigest":args["expectedStateDigest"]}});
                        state.metadata_retired=state.behavior!=ScanBehavior::RetirePartial;
                        if state.behavior==ScanBehavior::RetireLostResponse{return Err("fixture_retire_response_lost".into());}
                    }
                    let observation=retire_fixture_observation(&state);
                    result["kind"]=args["kind"].clone();result["producerQuiescent"]=json!(true);result["stateDigest"]=json!(hash(&observation.to_string()));result["observation"]=observation;
                    result["linuxPurgeProof"]=if state.metadata_retired{Value::Null}else{json!({"schema":"openkoda.kali-purge-proof.v1","receipt":state.receipt,"sha256":hash(&state.receipt.to_string())})};
                },
                "runtime_diagnostics"=>{result=diagnostic(&self.run);result["inspectedSession"]=json!({"sessionId":args["inspectSessionId"],"known":true,"active":false,"reason":"process_absent"});},
                _=>panic!("unexpected fixture tool"),
            }
            self.inner.frames.push_back(json!({"jsonrpc":"2.0","id":value["id"],"result":{"isError":false,"structuredContent":result}}));
            self.inner.sent.push(value);
            if self.run.tool=="scan_bytes"&&state.behavior==ScanBehavior::CancelProducer{cancel.store(true,Ordering::SeqCst);}
            return Ok(());
        }
        self.inner.send(value,deadline,cancel)
    }
    fn receive(&mut self,deadline:Instant,cancel:&AtomicBool)->Result<Value,String>{let reply=self.inner.receive(deadline,cancel)?;if self.cancel_after_read{cancel.store(true,Ordering::SeqCst);self.cancel_after_read=false;}Ok(reply)}
    fn finish(&mut self,run:&BridgeRun,deadline:Instant)->FinishOutcome{self.inner.finish(run,deadline)}
}
fn retire_fixture_observation(state:&SpoolState)->Value{
    let body=&state.manifest["body"];let entries=if state.metadata_retired{vec![]}else{vec![json!({"name":"manifest.json","bytes":state.manifest.to_string().len(),"sha256":hash(&state.manifest.to_string()),"identity":{"device":"1","inode":"3"}}),json!({"name":"purge.json","bytes":state.receipt.to_string().len(),"sha256":hash(&state.receipt.to_string()),"identity":{"device":"1","inode":"4"}})]};
    json!({"schema":"openkoda.kali-retire-observation.v1","environmentId":body["environmentId"],"assessmentId":body["assessmentId"],"producerSessionId":body["producerSessionId"],"captureId":body["captureId"],"attemptId":body["attemptId"],"ownerProfileFingerprint":body["ownerProfileFingerprint"],"manifestDigest":state.manifest["digest"],"purgeReceiptId":state.receipt["purgeReceiptId"],"purgeReceiptSha256":hash(&state.receipt.to_string()),"producerRecordSha256":hash(&state.commitment.to_string()),"directory":if state.metadata_retired{Value::Null}else{json!({"device":"1","inode":"2"})},"metadataBytes":entries.iter().map(|e|e["bytes"].as_u64().unwrap()).sum::<u64>(),"entries":entries,"pendingProducerWrite":null,"commitment":state.commitment,"state":if state.metadata_retired{"retired"}else if state.commitment.is_null(){"ready"}else{"metadata_remaining"},"slotPresent":!state.metadata_retired})
}
fn scan_fixture(behavior:ScanBehavior)->(QueuedFixture,Arc<Mutex<SpoolState>>){
    let mut f=QueuedFixture::new();let state=Arc::new(Mutex::new(SpoolState{raw:b"1:eval(synthetic_fixture)\n".repeat(500),manifest:Value::Null,producer:None,receipt:Value::Null,calls:vec![],behavior,commitment:Value::Null,metadata_retired:false,retire_apply_count:0}));
    f.bridge.launcher=Arc::new(SpoolLauncher{store:f.store.clone(),state:state.clone()});(f,state)
}
fn dispatch_scan(f:&QueuedFixture)->WorkKaliCompletion {f.queued_operation("scan");let(token,grant)=f.next();f.bridge.run_queued_evidence(&f.store,token,grant,||Ok(f.environment.clone()),&f.ai).unwrap()}
fn stdout_capture(f:&QueuedFixture)->(crate::capture::CaptureManifest,Vec<u8>){f.store.work_read(|db,_|{let capture=crate::capture::list_locked(db,&f.aid,None)?.into_iter().find(|m|m.stream==crate::capture::CaptureStream::ToolStdout).unwrap();let bytes=crate::capture::preserved_locked(db,&capture)?;Ok((capture,bytes))}).unwrap()}

#[test]
fn queued_scan_collects_exact_prefix_without_repeating_the_producer_and_retains_ownership_until_purge(){
    let(f,state)=scan_fixture(ScanBehavior::Complete);assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Completed);
    let(capture,bytes)=stdout_capture(&f);assert_eq!(capture.state,crate::capture::CaptureState::Sealed);assert_eq!(bytes,state.lock().unwrap().raw);assert_eq!(state.lock().unwrap().calls,["scan_bytes","capture_collect"]);
    assert_eq!(f.view()["jobs"][1]["state"],"awaiting_preparation");assert!(ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap_err().contains("kali_capture_retention_required"));
    let reference=f.bridge.spool_metadata(&f.store,Some(&f.aid)).unwrap().remove(0);let inspection=f.bridge.inspect_capture_purge(&f.store,&reference,&||Ok(f.environment.clone())).unwrap();let receipt_id=model::id();
    let receipt=f.bridge.execute_capture_purge(&f.store,&inspection,&receipt_id,&inspection.digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(receipt.state,"purged");
    assert!(ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap_err().contains("kali_capture_metadata_retention_required"));
    let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();let retire_id=model::id();
    let retired=f.bridge.execute_capture_retire(&f.store,&preview,&retire_id,&preview.inspection_digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(retired.state,KaliRetireState::Retired);
    ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap();
    assert!(f.bridge.execute_capture_purge(&f.store,&inspection,&receipt_id,&inspection.digest,&||panic!("replay must not launch")).is_err());
    assert_eq!(state.lock().unwrap().calls.iter().filter(|name|*name=="scan_bytes").count(),1);
}

#[test]
fn canceled_scan_collects_prefix_but_cannot_complete_or_release_a_dependency(){
    let(f,state)=scan_fixture(ScanBehavior::CancelProducer);assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Canceled);
    assert_eq!(stdout_capture(&f).1,state.lock().unwrap().raw);assert_eq!(state.lock().unwrap().calls,["scan_bytes","capture_collect"]);assert_ne!(f.view()["jobs"][1]["state"],"awaiting_preparation");
}

#[test]
fn cancellation_during_collection_seals_only_durable_prefix_as_interrupted(){
    let(f,state)=scan_fixture(ScanBehavior::CancelCollector);let completion=dispatch_scan(&f);assert_ne!(completion.outcome(),AttemptState::Completed);
    let(capture,bytes)=stdout_capture(&f);assert_eq!(capture.state,crate::capture::CaptureState::Interrupted);assert_eq!(bytes,state.lock().unwrap().raw[..4096]);assert_ne!(f.view()["jobs"][1]["state"],"awaiting_preparation");assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
}

#[test]
fn unknown_collector_retains_lease_and_recovery_probes_known_session_without_rescan(){
    let(f,state)=scan_fixture(ScanBehavior::UnknownCollector);assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Unknown);assert_eq!(stdout_capture(&f).0.state,crate::capture::CaptureState::Open);assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_err());
    state.lock().unwrap().behavior=ScanBehavior::Complete;let attempt=f.view()["attempts"][0]["id"].as_str().unwrap().to_owned();
    let permit=f.supervisor.arbiter().claim_queued_reconciliation(&f.store,&attempt,crate::workflow::Backend::Kali).unwrap();let completion=f.bridge.reconcile_work_evidence(&f.store,&permit,&||Ok(f.environment.clone()),&f.ai,|db,s,p,c|crate::work_supervisor::finish_kali_recovery_locked(db,s,p,c)).unwrap();drop(permit);
    assert_eq!(completion.outcome(),AttemptState::Completed);assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());assert_eq!(state.lock().unwrap().calls,["scan_bytes","capture_collect","runtime_diagnostics"]);
}

#[test]
fn proven_absent_spool_is_interrupted_and_does_not_strand_project_history(){
    let(f,state)=scan_fixture(ScanBehavior::Absent);assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Partial);assert_eq!(stdout_capture(&f).0.state,crate::capture::CaptureState::Interrupted);
    ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap();assert_eq!(f.bridge.spool_metadata(&f.store,Some(&f.aid)).unwrap()[0].availability,"unavailable");assert_eq!(state.lock().unwrap().calls,["scan_bytes","capture_collect"]);
}

/// Real validators/journals; only the fixed Linux process boundary is faked.
pub(crate) fn completed_scan_fixture()->(tempfile::TempDir,Arc<Store>,KaliBridge,String,String){let(f,_)=scan_fixture(ScanBehavior::Complete);assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Completed);(f._dir,f.store,f.bridge,f.aid,f.environment)}

fn purged_scan_fixture()->(QueuedFixture,Arc<Mutex<SpoolState>>,KaliSpoolRef){
    let(f,state)=scan_fixture(ScanBehavior::Complete);assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Completed);
    let reference=f.bridge.spool_metadata(&f.store,Some(&f.aid)).unwrap().remove(0);let preview=f.bridge.inspect_capture_purge(&f.store,&reference,&||Ok(f.environment.clone())).unwrap();
    assert_eq!(f.bridge.execute_capture_purge(&f.store,&preview,&model::id(),&preview.digest,&||Ok(f.environment.clone())).unwrap().state,"purged");(f,state,reference)
}
#[test]
fn retire_catalog_preserves_v2_and_pins_v3_schema(){
    assert_eq!(catalog_digest(),"98432aa4668fd905254b950b976a167d94e4382e12bcd4654a94c503bacedc2a");
    let mut legacy=expected_tools();legacy.as_array_mut().unwrap().pop();assert_eq!(hash(&legacy.to_string()),"26907e2c27bf7c28b9a30f6be1e6243f7d1dde346c6706e136d1f112a3567e7e");
    assert!(serde_json::to_vec(&json!({"jsonrpc":"2.0","id":2,"result":{"tools":expected_tools()}})).unwrap().len()<MAX_FRAME);
    let(f,_,reference)=purged_scan_fixture();let producer=f.bridge.find(&reference.run_id).unwrap();let mut historical=capture_io::producer_binding(&producer).unwrap().clone();historical.catalog_version=queued::KALI_LEGACY_CATALOG_VERSION.into();historical.catalog_sha256=hash(&legacy.to_string());historical.validate().unwrap();historical.catalog_sha256=catalog_digest();assert!(historical.validate().is_err());
}
#[test]
fn retire_proofs_reject_duplicate_fields_and_unknown_shapes(){
    let duplicate=br#"{"schema":"openkoda.kali-purge-proof.v1","receipt":{"state":"purged","state":"remaining"},"sha256":"a"}"#;
    assert!(serde_json::from_slice::<capture_io::retire::LinuxPurgeProof>(duplicate).is_err());
    let frames=read_frames(std::io::Cursor::new(b"{\"result\":{\"state\":\"purged\",\"state\":\"remaining\"}}\n".to_vec()));assert!(matches!(frames.recv_timeout(Duration::from_secs(1)).unwrap(),Frame::Error));
    let(f,_,reference)=purged_scan_fixture();let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();
    let inspected=f.bridge.runs().unwrap().into_iter().find(|r|r.tool=="capture_retire").unwrap();
    let mut context=serde_json::to_value(inspected.work_context.unwrap()).unwrap();
    assert_eq!(context.get("receipt"),Some(&Value::Null));
    assert!(serde_json::from_value::<capture_io::KaliRunContext>(context.clone()).is_ok());
    context.as_object_mut().unwrap().remove("receipt");
    assert!(serde_json::from_value::<capture_io::KaliRunContext>(context).is_err());
    for field in ["directory","pendingProducerWrite","commitment"]{let mut value=serde_json::to_value(&preview.observation).unwrap();value.as_object_mut().unwrap().remove(field);assert!(serde_json::from_value::<capture_io::retire::RetireObservation>(value).is_err());}
    let mut invalid=preview.clone();invalid.observation.metadata_bytes+=1;assert!(invalid.validate().is_err());let mut invalid=preview.clone();invalid.observation.entries[0].identity.inode="01".into();assert!(invalid.observation.validate().is_err());let mut invalid=preview.clone();invalid.observation.entries[0].name="raw.bin".into();assert!(invalid.observation.validate().is_err());
    let mut invalid=serde_json::to_value(&preview).unwrap();invalid["authority"]=json!(true);assert!(serde_json::from_value::<KaliRetirePreview>(invalid).is_err());
}
#[test]
fn retire_materializes_legacy_purge_proof_only_from_readonly_inspection(){
    let(f,state,reference)=purged_scan_fixture();let runs=f.bridge.runs().unwrap();
    let old=runs.iter().find(|r|r.tool=="capture_purge").unwrap();let mut value=serde_json::to_value(old).unwrap();value["workContext"]["receipt"].as_object_mut().unwrap().remove("linuxPurgeProof");f.bridge.save(&serde_json::from_value(value).unwrap()).unwrap();
    let calls=state.lock().unwrap().calls.len();let item=f.bridge.terminal_metadata(&f.store).unwrap().remove(0);assert_eq!(item.state,KaliMetadataState::Uninspected);assert!(item.purge_proof_sha256.is_none());assert_eq!(state.lock().unwrap().calls.len(),calls);
    let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();assert_eq!(preview.item.purge_proof_sha256,Some(hash(&state.lock().unwrap().receipt.to_string())));
    let saved=serde_json::to_value(f.bridge.find(&old.id).unwrap()).unwrap();assert_eq!(saved["workContext"]["receipt"]["linuxPurgeProof"]["receipt"],state.lock().unwrap().receipt);
    assert_eq!(state.lock().unwrap().calls.iter().filter(|s|s.as_str()=="capture_purge").count(),1);assert_eq!(state.lock().unwrap().retire_apply_count,0);
}
#[test]
fn retire_legacy_invalid_linux_proof_does_not_materialize_authority(){
    for key in ["prefixSha256","unexpected","state"]{
        let(f,state,reference)=purged_scan_fixture();let old=f.bridge.runs().unwrap().into_iter().find(|r|r.tool=="capture_purge").unwrap();let mut value=serde_json::to_value(&old).unwrap();value["workContext"]["receipt"].as_object_mut().unwrap().remove("linuxPurgeProof");f.bridge.save(&serde_json::from_value(value).unwrap()).unwrap();
        state.lock().unwrap().receipt[key]=if key=="state"{json!("remaining")}else{json!("a".repeat(64))};
        assert!(f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).is_err());
        assert!(f.bridge.terminal_metadata(&f.store).unwrap()[0].purge_proof_sha256.is_none());assert_eq!(state.lock().unwrap().retire_apply_count,0);
    }
}
#[test]
fn retire_success_survives_restart_and_replay_never_dispatches_apply(){
    let(f,state,reference)=purged_scan_fixture();let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();let id=model::id();
    let receipt=f.bridge.execute_capture_retire(&f.store,&preview,&id,&preview.inspection_digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(receipt.state,KaliRetireState::Retired);assert!(state.lock().unwrap().metadata_retired);
    assert!(f.bridge.execute_capture_retire(&f.store,&preview,&id,&preview.inspection_digest,&||panic!("replay cannot launch")).unwrap_err().contains("already_dispatched"));
    let reopened=KaliBridge::with_launcher(f.store.root.clone(),f.bridge.launcher.clone()).unwrap().with_arbiter(f.supervisor.shared_arbiter());
    assert_eq!(reopened.terminal_metadata(&f.store).unwrap()[0].state,KaliMetadataState::Retired);
    assert_eq!(reopened.reconcile_capture_retire(&f.store,&id,&||Ok(f.environment.clone())).unwrap(),receipt);assert_eq!(state.lock().unwrap().retire_apply_count,1);
    ensure_capture_history_purgeable(&f.store.root,&reopened.runs().unwrap(),&[f.aid.clone()]).unwrap();
}
#[test]
fn retire_durable_success_reconciles_with_a_full_bridge_journal(){
    let (f,state,reference)=purged_scan_fixture();
    let identity=||Ok(f.environment.clone());
    let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&identity).unwrap();
    let id=model::id();
    let receipt=f.bridge.execute_capture_retire(&f.store,&preview,&id,&preview.inspection_digest,&identity).unwrap();
    assert_eq!(receipt.state,KaliRetireState::Retired);
    let calls=state.lock().unwrap().calls.len();

    // Put older, completed independent diagnostics before the real family.
    // The newest row is now a durable success in the final journal slot.
    let mut filler=run();
    filler.status="completed".into();
    filler.finished_at=Some(model::now());
    filler.cleanup_confirmed=true;
    {
        let mut db=f.bridge.db.lock().unwrap();
        let tx=db.transaction().unwrap();
        let count:i64=tx.query_row("SELECT count(*) FROM bridge_runs",[],|r|r.get(0)).unwrap();
        {
            let mut insert=tx.prepare("INSERT INTO bridge_runs(rowid,id,data) VALUES(?1,?2,?3)").unwrap();
            for index in 1..=10000-count{
                filler.id=model::id();
                insert.execute(rusqlite::params![-index,&filler.id,serde_json::to_string(&filler).unwrap()]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    assert_eq!(f.bridge.require_metadata_capacity(1).unwrap_err(),"work_journal_quota");
    let recovered=f.bridge.reconcile_capture_retire(&f.store,&id,&||panic!("durable success must not launch Kali")).unwrap();
    assert_eq!(recovered,receipt);
    assert_eq!(state.lock().unwrap().calls.len(),calls);
    assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
}
#[test]
fn retire_partial_and_precommit_failures_require_new_ids_and_readonly_reconcile(){
    for behavior in [ScanBehavior::RetirePartial,ScanBehavior::RetirePreCommit]{
        let(f,state,reference)=purged_scan_fixture();let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();let first=model::id();state.lock().unwrap().behavior=behavior;
        let result=f.bridge.execute_capture_retire(&f.store,&preview,&first,&preview.inspection_digest,&||Ok(f.environment.clone()));
        if behavior==ScanBehavior::RetirePartial{assert_eq!(result.unwrap().state,KaliRetireState::Remaining);}else{assert!(result.is_err());}
        state.lock().unwrap().behavior=ScanBehavior::Complete;
        let remaining=f.bridge.reconcile_capture_retire(&f.store,&first,&||Ok(f.environment.clone())).unwrap();
        assert_eq!(remaining.state,KaliRetireState::Remaining);assert_eq!(state.lock().unwrap().retire_apply_count,1);
        assert!(ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap_err().contains("metadata_retention_required"));
        let next=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();assert_eq!(next.previous_receipt_id,if behavior==ScanBehavior::RetirePartial{Some(first.clone())}else{None});
        assert!(f.bridge.execute_capture_retire(&f.store,&next,&first,&next.inspection_digest,&||panic!("old identity is consumed")).is_err());
        let retired=f.bridge.execute_capture_retire(&f.store,&next,&model::id(),&next.inspection_digest,&||Ok(f.environment.clone())).unwrap();
        assert_eq!(retired.state,KaliRetireState::Retired);assert_eq!(state.lock().unwrap().retire_apply_count,2);
        assert_eq!(f.bridge.reconcile_capture_retire(&f.store,&first,&||panic!("superseded control cannot launch")).unwrap_err(),"capture_retire_superseded");
        #[cfg(windows)]
        if behavior==ScanBehavior::RetirePartial{
            // IDs are deliberately ordered oldest-first; SQLite rowid still
            // records the actual chronology. ProjectPurge must use the newest apply.
            for (receipt,ordered_id) in [(&remaining,"00000000-0000-4000-8000-000000000001"),(&retired,"ffffffff-ffff-4fff-bfff-ffffffffffff")]{
                let mut row=f.bridge.find(&receipt.control_run_id).unwrap();
                row.id=ordered_id.into();
                let Some(capture_io::KaliRunContext::RetireControl{receipt:Some(record),..})=&mut row.work_context else{panic!("expected durable apply");};
                record.receipt.control_run_id=ordered_id.into();
                let updated=f.bridge.db.lock().unwrap().execute("UPDATE bridge_runs SET id=?1,data=?2 WHERE id=?3",rusqlite::params![ordered_id,serde_json::to_string(&row).unwrap(),&receipt.control_run_id]).unwrap();
                assert_eq!(updated,1);
            }
            let project=f.store.snapshot().unwrap().assessments.iter().find(|a|a.id==f.aid).unwrap().project_id.clone();
            f.store.handle(crate::core::Request{action:"project.archive".into(),payload:json!({"id":project})}).unwrap();
            let purge=crate::project_purge::ProjectPurgeService::new(f.store.root.clone()).unwrap();
            let plan=purge.handle("projectPurge.plan",json!({"projectId":project}),&f.store).unwrap();
            assert!(plan["rows"].as_array().unwrap().iter().any(|row|row["table"]=="bridge_runs"));
        }
    }
}
#[test]
fn retire_lost_response_and_unknown_cleanup_reconcile_without_repeating_the_effect(){
    for behavior in [ScanBehavior::RetireLostResponse,ScanBehavior::RetireUnknownCleanup]{
        let(f,state,reference)=purged_scan_fixture();let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();let id=model::id();state.lock().unwrap().behavior=behavior;
        assert!(f.bridge.execute_capture_retire(&f.store,&preview,&id,&preview.inspection_digest,&||Ok(f.environment.clone())).is_err());assert_eq!(f.bridge.terminal_metadata(&f.store).unwrap()[0].state,KaliMetadataState::Unknown);
        assert!(f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||panic!("unknown must reconcile first")).is_err());state.lock().unwrap().behavior=ScanBehavior::Complete;
        assert_eq!(f.bridge.reconcile_capture_retire(&f.store,&id,&||Ok(f.environment.clone())).unwrap().state,KaliRetireState::Retired);assert_eq!(state.lock().unwrap().retire_apply_count,1);assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
    }
}
#[test]
fn retire_old_history_cannot_adopt_a_newer_linux_commitment(){
    let(f,state,reference)=purged_scan_fixture();let retained=f.bridge.runs().unwrap();let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();let id=model::id();
    assert_eq!(f.bridge.execute_capture_retire(&f.store,&preview,&id,&preview.inspection_digest,&||Ok(f.environment.clone())).unwrap().state,KaliRetireState::Retired);
    // Simulate restoring the older host bridge journal at the same profile path.
    {let db=f.bridge.db.lock().unwrap();for run in work::read_runs(&db).unwrap(){if !retained.iter().any(|old|old.id==run.id){db.execute("DELETE FROM bridge_runs WHERE id=?1",[&run.id]).unwrap();}}}
    assert_eq!(f.bridge.terminal_metadata(&f.store).unwrap()[0].state,KaliMetadataState::Uninspected);
    assert!(f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap_err().contains("history_unavailable"));assert_eq!(state.lock().unwrap().retire_apply_count,1);
    assert!(ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).is_err());
}
#[test]
fn retire_foreign_profile_and_capacity_refuse_before_launch(){
    let(f,state,reference)=purged_scan_fixture();let calls=state.lock().unwrap().calls.len();let other=tempfile::tempdir().unwrap();let(foreign,_)=setup_store(other.path());
    assert_eq!(f.bridge.terminal_metadata(&foreign).unwrap()[0].state,KaliMetadataState::ForeignProfile);
    assert!(f.bridge.inspect_capture_retire(&foreign,&reference.capture_id,&||panic!("foreign profile cannot launch")).is_err());
    assert!(f.bridge.require_metadata_capacity(10000).unwrap_err().contains("quota"));let mut oversized=f.bridge.find(&reference.run_id).unwrap();oversized.error=Some("x".repeat(16384));assert_eq!(f.bridge.save(&oversized).unwrap_err(),"work_journal_record_quota");
    assert_eq!(state.lock().unwrap().calls.len(),calls);assert_ne!(f.bridge.find(&reference.run_id).unwrap().error,oversized.error);
}
#[test]
fn retire_inventory_is_host_only_and_same_capture_mixed_selection_is_refused(){
    let(f,state,reference)=purged_scan_fixture();let service=crate::retention::RetentionService::new(f.store.root.clone()).unwrap();let identity=||Ok(f.environment.clone());let backend=crate::retention::KaliRetention{bridge:&f.bridge,store:&f.store,identity:&identity};let calls=state.lock().unwrap().calls.len();
    let inventory=service.handle_with_spool("retention.inventory",json!({}),&f.store,Some(&backend)).unwrap();let item=inventory["items"].as_array().unwrap().iter().find(|i|i["category"]=="kali_capture_metadata").unwrap();assert_eq!(item["kaliMetadataItem"]["state"],"uninspected");assert!(item.get("kaliMetadata").is_none());assert_eq!(state.lock().unwrap().calls.len(),calls);
    assert!(service.handle_with_spool("retention.plan",json!({"items":[{"category":"kali_capture","id":reference.capture_id},{"category":"kali_capture_metadata","id":reference.capture_id}]}),&f.store,Some(&backend)).is_err());assert_eq!(state.lock().unwrap().calls.len(),calls);
}
#[cfg(windows)]
#[test]
fn retire_retention_partial_restarts_reconciles_and_requires_new_review(){
    let(f,state,reference)=purged_scan_fixture();let identity=||Ok(f.environment.clone());let backend=crate::retention::KaliRetention{bridge:&f.bridge,store:&f.store,identity:&identity};let service=crate::retention::RetentionService::new(f.store.root.clone()).unwrap();let selection=json!({"items":[{"category":"kali_capture_metadata","id":reference.capture_id}]});
    let preview=service.handle_with_spool("retention.plan",selection.clone(),&f.store,Some(&backend)).unwrap();assert!(preview["totalBytes"].as_u64().unwrap()>0);assert!(preview["items"][0].get("kaliMetadataItem").is_none());assert_eq!(preview["items"][0]["kaliMetadata"]["observation"]["slotPresent"],true);
    let apply=json!({"id":preview["id"],"digest":preview["digest"],"confirmation":"EXCLUIR PERMANENTEMENTE"});state.lock().unwrap().behavior=ScanBehavior::RetirePartial;
    assert_eq!(service.handle_with_spool("retention.apply",apply.clone(),&f.store,Some(&backend)).unwrap()["status"],"unknown");drop(service);let service=crate::retention::RetentionService::new(f.store.root.clone()).unwrap();assert!(service.handle_with_spool("retention.apply",apply,&f.store,Some(&backend)).is_err());
    state.lock().unwrap().behavior=ScanBehavior::Complete;let observed=service.handle_with_spool("retention.reconcile",json!({"id":preview["id"]}),&f.store,Some(&backend)).unwrap();assert_eq!(observed["status"],"partial");assert_eq!(observed["progress"][0]["status"],"present");assert_eq!(state.lock().unwrap().retire_apply_count,1);
    let next=service.handle_with_spool("retention.plan",selection,&f.store,Some(&backend)).unwrap();assert!(next["items"][0]["kaliMetadata"]["previousReceiptId"].is_string());assert_ne!(next["id"],preview["id"]);
    assert_eq!(service.handle_with_spool("retention.apply",json!({"id":next["id"],"digest":next["digest"],"confirmation":"EXCLUIR PERMANENTEMENTE"}),&f.store,Some(&backend)).unwrap()["status"],"applied");assert_eq!(state.lock().unwrap().retire_apply_count,2);
}

struct AuditedSystemLauncher;
impl Launcher for AuditedSystemLauncher {
    fn launch(&self,run:&BridgeRun,cancel:&AtomicBool)->Result<Box<dyn Transport>,String>{
        println!("{}",json!({"test":"managed_kali_workflow","stage":"before_launch","runId":run.id,"rootRunId":run.reconciliation_of.as_deref().unwrap_or(&run.id),"assessmentId":run.assessment_id,"environmentId":run.environment_id,"tool":run.tool}));
        SystemLauncher.launch(run,cancel)
    }
}

#[test]
#[ignore="requires explicit root-coordinated validation of the managed Kali identity"]
fn managed_kali_queued_scan_capture_and_retention_opt_in(){
    assert_eq!(std::env::var("OPENKODA_RUN_KALI_WORKFLOW_SMOKE").as_deref(),Ok("1"));
    let environment=std::env::var("OPENKODA_KALI_WORKFLOW_ENVIRONMENT_ID").expect("root must provide the independently validated managed environment identity");valid_id(&environment).unwrap();
    let mut f=QueuedFixture::new();f._dir.disable_cleanup(true);f.environment=environment;f.bridge.launcher=Arc::new(AuditedSystemLauncher);
    println!("{}",json!({"test":"managed_kali_workflow","stage":"profile_preserved","profileRoot":f.store.root,"assessmentId":f.aid,"environmentId":f.environment}));
    let completion=dispatch_scan(&f);assert_eq!(completion.outcome(),AttemptState::Completed);let(capture,bytes)=stdout_capture(&f);assert_eq!(capture.state,crate::capture::CaptureState::Sealed);assert!(!bytes.is_empty());assert_eq!(f.view()["jobs"][1]["state"],"awaiting_preparation");
    let reference=f.bridge.spool_metadata(&f.store,Some(&f.aid)).unwrap().remove(0);println!("{}",json!({"test":"managed_kali_workflow","stage":"captured","spool":reference}));let inspection=f.bridge.inspect_capture_purge(&f.store,&reference,&||Ok(f.environment.clone())).unwrap();let receipt_id=model::id();let receipt=f.bridge.execute_capture_purge(&f.store,&inspection,&receipt_id,&inspection.digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(receipt.state,"purged");assert_eq!(f.bridge.reconcile_capture_purge(&f.store,&receipt_id,&||Ok(f.environment.clone())).unwrap(),receipt);
    assert!(ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap_err().contains("kali_capture_metadata_retention_required"));
    let preview=f.bridge.inspect_capture_retire(&f.store,&reference.capture_id,&||Ok(f.environment.clone())).unwrap();assert!(preview.observation.slot_present);println!("{}",json!({"test":"managed_kali_workflow","stage":"metadata_reviewed","preview":preview}));
    let retire_id=model::id();let retired=f.bridge.execute_capture_retire(&f.store,&preview,&retire_id,&preview.inspection_digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(retired.state,KaliRetireState::Retired);
    assert_eq!(f.bridge.reconcile_capture_retire(&f.store,&retire_id,&||Ok(f.environment.clone())).unwrap(),retired);
    let observation=f.bridge.inspect_retired_for_test(&f.store,&retired,&||Ok(f.environment.clone())).unwrap();
    assert!(!observation.slot_present);
    println!("{}",json!({"test":"managed_kali_workflow","stage":"first_slot_absent","observation":observation}));
    ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[f.aid.clone()]).unwrap();assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
    let first_assessment=f.aid.clone();let project=f.store.snapshot().unwrap().projects[0].clone();
    let next=f.store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":project.id,"kind":"web","target":project.target,"title":"Capture after retired slot","objective":"Second synthetic offline scan in the same managed profile","scope":crate::scope::test_creation(&f.store,&project.id,&["kali.runtime-diagnostics.v1","kali.hash-evidence.v1","kali.rg.signals.v1"])})}).unwrap();
    f.aid=next["id"].as_str().unwrap().to_owned();f.eid=model::id();
    f.store.mutate(|s|{let content="eval(second_synthetic_fixture)\n".to_owned();s.assessments.iter_mut().find(|a|a.id==f.aid).unwrap().evidence.push(Evidence{id:f.eid.clone(),name:"second-fixture.txt".into(),sha256:hash(&content),content,created_at:model::now()});Ok(())}).unwrap();
    crate::data_policy::test_support::authorize(&f.store,&f.bridge.data_policy,&f.aid,&f.ai.data_redactor(),&crate::data_projection::RejectExternal,None).unwrap();
    println!("{}",json!({"test":"managed_kali_workflow","stage":"second_scan_prepared","assessmentId":f.aid,"environmentId":f.environment,"profileRoot":f.store.root,"firstRetireReceipt":retired}));
    assert_eq!(dispatch_scan(&f).outcome(),AttemptState::Completed);let(second_capture,second_bytes)=stdout_capture(&f);assert_eq!(second_capture.state,crate::capture::CaptureState::Sealed);assert!(!second_bytes.is_empty());assert_ne!(second_capture.id,capture.id);
    let second_reference=f.bridge.spool_metadata(&f.store,Some(&f.aid)).unwrap().remove(0);assert_ne!(second_reference.run_id,reference.run_id);assert_eq!(second_reference.environment_id,reference.environment_id);assert_eq!(second_reference.owner_profile_fingerprint,reference.owner_profile_fingerprint);
    println!("{}",json!({"test":"managed_kali_workflow","stage":"second_scan_captured","spool":second_reference,"preservedBytes":second_bytes.len()}));
    let second_purge=f.bridge.inspect_capture_purge(&f.store,&second_reference,&||Ok(f.environment.clone())).unwrap();let second_purge_id=model::id();let second_receipt=f.bridge.execute_capture_purge(&f.store,&second_purge,&second_purge_id,&second_purge.digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(second_receipt.state,"purged");
    let second_preview=f.bridge.inspect_capture_retire(&f.store,&second_reference.capture_id,&||Ok(f.environment.clone())).unwrap();assert!(second_preview.observation.slot_present);let second_retire_id=model::id();let second_retired=f.bridge.execute_capture_retire(&f.store,&second_preview,&second_retire_id,&second_preview.inspection_digest,&||Ok(f.environment.clone())).unwrap();assert_eq!(second_retired.state,KaliRetireState::Retired);assert_eq!(f.bridge.reconcile_capture_retire(&f.store,&second_retire_id,&||Ok(f.environment.clone())).unwrap(),second_retired);
    let second_observation=f.bridge.inspect_retired_for_test(&f.store,&second_retired,&||Ok(f.environment.clone())).unwrap();
    assert!(!second_observation.slot_present);
    println!("{}",json!({"test":"managed_kali_workflow","stage":"second_slot_absent","observation":second_observation}));
    let metadata=f.bridge.terminal_metadata(&f.store).unwrap();assert_eq!(metadata.len(),2);assert!(metadata.iter().all(|item|item.state==KaliMetadataState::Retired));ensure_capture_history_purgeable(&f.store.root,&f.bridge.runs().unwrap(),&[first_assessment,f.aid.clone()]).unwrap();assert!(f.supervisor.arbiter().ensure_quiescent(&f.store).is_ok());
    println!("{}",json!({"test":"managed_kali_workflow","stage":"completed","receipt":receipt,"retireReceipt":retired,"secondReceipt":second_receipt,"secondRetireReceipt":second_retired,"profileRoot":f.store.root}));
}
