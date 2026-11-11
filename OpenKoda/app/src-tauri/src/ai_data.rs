//! Reviewed data packages. The renderer chooses inputs; only this host module
//! resolves their immutable subjects, classification and provider identity.
use super::*;
use crate::data_policy::{AdmittedDataContext,DataCategory,DataDestination,DataOperation,DataOriginRef,DataReviewRef,DataSelection,DataSubject,DataUseSpec,HumanTextDecision,RequestFingerprint,Sensitivity,SubjectVersion};
use crate::data_projection::{DataRedactor, RedactedText, RedactionMode};
use serde::{Deserialize, Serialize};

pub(crate) struct AiDataRedactor { keys:Vec<Zeroizing<String>>, mode:RedactionMode }
impl DataRedactor for AiDataRedactor {
    fn mode(&self)->RedactionMode { self.mode.clone() }
    fn contains_managed_secret(&self,text:&str)->bool { self.keys.iter().any(|key|text.contains(key.as_str())) }
    fn redact(&self,text:&str)->RedactedText {
        if matches!(self.mode,RedactionMode::MetadataOnly) {
            return RedactedText{text:"[Texto indisponível: proteção local da credencial não pôde ser verificada.]".into(),changed:!text.is_empty(),secret_material:false};
        }
        let projected=AiService::redact_keys(text,&self.keys);
        let changed=projected!=text;
        let secret_material=self.keys.iter().any(|key|text.contains(key.as_str()))
            ||(changed&&projected.contains("[REDACTED"));
        RedactedText{text:projected,changed,secret_material}
    }
}

#[cfg(all(test,windows))]
impl AiService {
    /// Fixtures adopt policy and classification through the same public core
    /// contract as the UI. This is test setup, never a production fallback.
    pub(crate) fn test_data_authority(&self,store:&Store,aid:&str)->String {
        let service=&self.data_policy;let redactor=self.data_redactor();let external=CanonicalOnly;
        let state=store.snapshot().unwrap();let assessment=state.assessments.iter().find(|a|a.id==aid).unwrap();let pid=assessment.project_id.clone();
        crate::data_policy::test_support::authorize(store,service,aid,&redactor,&external,None).unwrap();
        let state=store.snapshot().unwrap();
        let identity=self.identity_for(store,Some(aid)).unwrap();
        if let Some(reference)=state.credential_references.iter().rev().find(|r|r.project_id==pid&&r.provider_slot_ref.as_ref().is_some_and(|p|p.slot_id==identity.slot_id)&&!r.revoked){return reference.reference_id.clone();}
        let result=service.handle("credential.reference.create",json!({"projectId":pid,"expectedStateDigest":crate::credential_refs::state_digest(&state,&pid),"idempotencyKey":model::id(),"kind":"provider_reference","label":"Synthetic provider reference","purpose":"Offline fake provider tests","providerSlotId":identity.slot_id}),store,&redactor,&external,Some(&identity)).unwrap();
        result["reference"]["id"].as_str().or_else(||result["reference"]["referenceId"].as_str()).expect("reference identity").into()
    }

    pub(crate) fn test_data_input(&self,store:&Store,operation:&str,mut payload:Value)->Value {
        let aid=payload["assessmentId"].as_str().unwrap();let credential=self.test_data_authority(store,aid);
        let mut preview=json!({"assessmentId":aid,"evidenceIds":payload.get("evidenceIds").cloned().unwrap_or(json!([])),"contextTargetRefs":payload.get("contextTargetRefs").cloned().unwrap_or(json!([])),"message":payload["message"],"operation":operation,"messageSensitivity":"internal","packageSensitivity":"internal","categories":["human_text","target_metadata","execution_summary","evidence_excerpt","source_excerpt"],"credentialReferenceId":credential});
        for field in ["allowedAdapters","operationTargetRef","perspectives","maxOutputTokensPerRequest","maxTotalTokens"]{if let Some(value)=payload.get(field){preview[field]=value.clone();}}
        let Ok(prepared)=self.handle("ai.context",preview,store)else{return payload;};
        payload["contextDigest"]=prepared["digest"].clone();payload["dataReview"]=prepared["dataReview"].clone();payload
    }
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize,Ordering};
    const KEY:&str="synthetic-data-policy-key-not-real-555";
    struct Fake{calls:AtomicUsize,prompts:Mutex<Vec<String>>}
    impl Provider for Fake{fn response(&self,_:ProviderId,_:&str,key:&str,prompt:&str,_:&str,_:u32)->Result<Value,String>{assert_eq!(key,KEY);self.calls.fetch_add(1,Ordering::SeqCst);self.prompts.lock().unwrap().push(prompt.into());Ok(json!({"id":"synthetic-data-response","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Synthetic bounded response"}]}],"usage":{"input_tokens":3,"output_tokens":4}}))}}
    struct Fixture{_dir:tempfile::TempDir,store:Store,ai:AiService,fake:Arc<Fake>,aid:String}
    impl Fixture{
        fn new()->Self{
            let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();
            let fake=Arc::new(Fake{calls:AtomicUsize::new(0),prompts:Mutex::new(vec![])});let ai=AiService::with_provider(dir.path().into(),fake.clone()).unwrap();
            ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
            let p=store.handle(crate::core::Request{action:"project.create".into(),payload:json!({"name":"Data fixture","kind":"web","target":"https://example.invalid/fixture"})}).unwrap();
            let a=store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"title":"Data review","kind":"web","target":p["target"],"objective":"Review synthetic package only","scope":crate::scope::test_creation(&store,p["id"].as_str().unwrap(),&[AI_METHOD])})}).unwrap();
            let aid=a["id"].as_str().unwrap().into();Self{_dir:dir,store,ai,fake,aid}
        }
        fn raw_input(&self)->Value{let context=self.ai.context(&self.store,ContextInput{assessment_id:self.aid.clone(),evidence_ids:vec![],context_target_refs:vec![]}).unwrap();json!({"assessmentId":self.aid,"evidenceIds":[],"message":"Review the synthetic metadata","contextDigest":context["digest"],"allowRemote":true})}
        fn input(&self)->Value{self.ai.test_data_input(&self.store,"ask",self.raw_input())}
        fn requests(&self)->i64{self.ai.db.lock().unwrap().query_row("SELECT count(*) FROM requests",[],|r|r.get(0)).unwrap()}
    }
    #[test]fn legacy_consent_cannot_reserve_or_dispatch_project_data(){
        let f=Fixture::new();let before=f.store.snapshot().unwrap().data_use_receipts.len();
        let error=f.ai.handle("ai.ask",f.raw_input(),&f.store).unwrap_err();assert!(error.starts_with("data_review_required"));
        assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),0);assert_eq!(f.store.snapshot().unwrap().data_use_receipts.len(),before);
        assert_eq!(f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap()["recentCharges"].as_array().unwrap().len(),0);
    }
    #[test]fn legacy_context_cannot_bypass_explicit_data_review_or_local_read(){
        let f=Fixture::new();let eid=model::id();let content="CANARY-LEGACY-CONTEXT-MUST-NOT-RETURN";
        f.store.mutate(|s|{s.assessments[0].evidence.push(model::Evidence{id:eid.clone(),name:"legacy context fixture".into(),content:content.into(),sha256:digest(content),created_at:model::now()});Ok(())}).unwrap();
        let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
        let error=f.ai.handle("ai.context",json!({"assessmentId":f.aid,"evidenceIds":[eid]}),&f.store).unwrap_err();
        assert!(error.starts_with("data_review_required"));assert!(!error.contains(content));
        assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    }
    #[test]fn exact_packet_is_one_use_and_keeps_receipt_and_cost_binding(){
        let f=Fixture::new();let input=f.input();let review=input["dataReview"].clone();
        let prepared=f.ai.data_reviews.lock().unwrap().get(review["id"].as_str().unwrap()).cloned().unwrap();
        let result=f.ai.handle("ai.ask",input.clone(),&f.store).unwrap();assert!(result["dataUseReceiptRef"].is_object());
        assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);let prompts=f.fake.prompts.lock().unwrap();assert_eq!(digest(&prompts[0]),prepared.spec.requests[0].output_sha256);assert_eq!(prompts[0].len() as u64,prepared.spec.requests[0].output_bytes);drop(prompts);
        assert!(f.ai.handle("ai.ask",input,&f.store).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.requests(),1);
        let history=f.ai.handle("ai.history",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(history["requests"][0]["dataReviewRef"],review);assert_eq!(history["requests"][0]["dataReceiptStatus"],"recorded");assert!(!history.to_string().contains(KEY));
    }
    #[test]fn message_edit_reclassification_revocation_and_rotation_block_before_reservation(){
        for change in ["message","classification","policy","key","credential_reference"]{
            let f=Fixture::new();let mut input=f.input();
            match change{
                "message"=>input["message"]=json!("A different human decision"),
                "key"=>{f.ai.handle("ai.key.save",json!({"key":"another-synthetic-data-key-not-real"}),&f.store).unwrap();},
                "credential_reference"=>{let state=f.store.snapshot().unwrap();let pid=&state.assessments[0].project_id;let reference=&state.credential_references[0];f.ai.data_policy.handle("credential.reference.revoke",json!({"projectId":pid,"id":reference.reference_id,"expectedStateDigest":crate::credential_refs::state_digest(&state,pid),"idempotencyKey":model::id(),"acknowledgeRevocation":true}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();},
                "policy"=>{let state=f.store.snapshot().unwrap();let pid=&state.assessments[0].project_id;let status=f.ai.data_policy.handle("data.policy.status",json!({"projectId":pid}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();f.ai.data_policy.handle("data.policy.revoke",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"acknowledgeRevocation":true}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();},
                _=>{let subject=DataSubject::AssessmentMetadata{assessment_id:f.aid.clone(),category:DataCategory::HumanText};let state=f.store.snapshot().unwrap();let resolved=crate::data_projection::resolve_subject(&f.store,&state,&subject,&CanonicalOnly).unwrap();let preview=f.ai.data_policy.handle("data.classification.prepare",json!({"subject":subject,"expectedSourceFingerprint":resolved.fingerprint,"sensitivity":"sensitive","reason":"Synthetic stricter review"}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();f.ai.data_policy.handle("data.classification.commit",json!({"review":preview["review"],"idempotencyKey":model::id(),"acknowledgeClassification":true}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();}
            }
            assert!(f.ai.handle("ai.ask",input,&f.store).is_err(),"{change}");assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.requests(),0);
            assert!(f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap()["recentCharges"].as_array().unwrap().is_empty());
        }
    }
    #[test]fn raw_secret_in_new_message_is_rejected_without_persisting_its_hash(){
        let f=Fixture::new();let credential=f.ai.test_data_authority(&f.store,&f.aid);
        let input=json!({"assessmentId":f.aid,"evidenceIds":[],"operation":"ask","message":KEY,"messageSensitivity":"internal","packageSensitivity":"internal","categories":["human_text","target_metadata"],"credentialReferenceId":credential});
        assert!(f.ai.handle("ai.context",input,&f.store).unwrap_err().starts_with("data_secret_material"));assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
        let snapshot=serde_json::to_string(&f.store.snapshot().unwrap()).unwrap();assert!(!snapshot.contains(KEY));assert!(!snapshot.contains(&digest(KEY)));
    }
    #[test]fn data_revocation_during_provider_keeps_first_charge_and_blocks_next_perspective(){
        struct Blocking{calls:AtomicUsize,started:std::sync::mpsc::Sender<()>,release:Mutex<std::sync::mpsc::Receiver<()>>}
        impl Provider for Blocking{fn response(&self,_:ProviderId,_:&str,_:&str,_:&str,_:&str,_:u32)->Result<Value,String>{self.calls.fetch_add(1,Ordering::SeqCst);self.started.send(()).unwrap();self.release.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();Ok(json!({"id":"inflight-data-response","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Result admitted before revocation"}]}],"usage":{"input_tokens":7,"output_tokens":8}}))}}
        let mut f=Fixture::new();let(started,ready)=std::sync::mpsc::channel();let(release,wait)=std::sync::mpsc::channel();let provider=Arc::new(Blocking{calls:AtomicUsize::new(0),started,release:Mutex::new(wait)});
        f.ai=AiService::with_provider(f.store.root.clone(),provider.clone()).unwrap();
        let input=f.ai.test_data_input(&f.store,"investigation",json!({"assessmentId":f.aid,"evidenceIds":[],"message":"Independent synthetic views","perspectives":["evidence","counterevidence"],"maxOutputTokensPerRequest":512,"maxTotalTokens":100000}));
        let plan=f.ai.handle("ai.investigation.plan",input,&f.store).unwrap();
        std::thread::scope(|threads|{
            let task=threads.spawn(||f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store));
            ready.recv_timeout(Duration::from_secs(10)).unwrap();
            let state=f.store.snapshot().unwrap();let pid=&state.assessments[0].project_id;
            let status=f.ai.data_policy.handle("data.policy.status",json!({"projectId":pid}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();
            // If revocation waits for inference this would deadlock until the
            // bounded fake timeout; it must complete while the provider waits.
            f.ai.data_policy.handle("data.policy.revoke",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"acknowledgeRevocation":true}),&f.store,&f.ai.data_redactor(),&CanonicalOnly,None).unwrap();
            release.send(()).unwrap();let result=task.join().unwrap().unwrap();
            assert_eq!(result["status"],"partial");assert_eq!(result["knownTokens"],15);assert_eq!(result["results"].as_array().unwrap().len(),1);
        });
        assert_eq!(provider.calls.load(Ordering::SeqCst),1);let history=f.ai.handle("ai.history",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(history["requests"][0]["status"],"completed");assert_eq!(history["requests"][0]["outputTokens"],8);assert!(history["requests"][0]["dataUseReceiptRef"].is_object());
    }
}

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct AiDataOptions {
    pub operation:String,
    pub message_sensitivity:Sensitivity,
    pub package_sensitivity:Sensitivity,
    pub categories:Vec<DataCategory>,
    pub credential_reference_id:String,
    #[serde(default)] pub operation_target_ref:Option<ScopeTargetRef>,
    #[serde(default)] pub allowed_adapters:Vec<String>,
    #[serde(default)] pub perspectives:Vec<String>,
    #[serde(default)] pub max_output_tokens_per_request:Option<u32>,
    #[serde(default)] pub max_total_tokens:Option<u64>,
}

#[derive(Clone)]
pub(super) struct PreparedAiData {
    pub review:DataReviewRef,
    pub options:AiDataOptions,
    pub spec:DataUseSpec,
    pub assessment_id:String,
    pub context_digest:String,
    pub raw_message_sha256:String,
    pub projected_message_sha256:String,
    metadata_bytes:usize,
}

pub(super) struct AiPacket {
    pub spec:DataUseSpec, pub context:Value, pub message:String,
    pub prompts:Vec<(String,String,u32)>, pub options:AiDataOptions,
    pub raw_message_sha256:String,
}
pub(super) struct ReviewedAiData { pub prepared:PreparedAiData, pub packet:AiPacket }
pub(super) struct DataDispatch<'a> {
    pub review:&'a DataReviewRef, pub spec:&'a DataUseSpec,
    pub acknowledge_sensitive:bool, pub admitted_context:Option<(&'a AdmittedDataContext,u8)>,
}

struct CanonicalOnly;
impl crate::data_projection::ExternalSubjectResolver for CanonicalOnly {
    fn resolve(&self,_:&DataSubject)->Result<crate::data_projection::ResolvedDataSubject,String>{Err("data_source_unknown: derivado externo não faz parte deste pacote.".into())}
    fn list(&self,_:&str)->Result<Vec<DataSubject>,String>{Ok(vec![])}
}

impl AiService {
    /// Snapshot a redactor without the inference lease. Status, cancellation
    /// and revocation remain reachable during a provider call or vault failure.
    pub(crate) fn data_redactor(&self)->AiDataRedactor {
        match self.all_keys() {
            Ok(keys)=>{let mode=if keys.is_empty(){RedactionMode::NoCredentialConfigured}else{RedactionMode::Ready};AiDataRedactor{keys,mode}},
            Err(_)=>AiDataRedactor{keys:vec![],mode:RedactionMode::MetadataOnly}
        }
    }
    pub(crate) fn provider_identities(&self,store:&Store)->Result<Vec<crate::credential_refs::ProviderCredentialIdentity>,String>{
        let snapshot=store.snapshot()?;
        let db=self.db.lock().map_err(failure)?;
        snapshot.provider_slots.iter().map(|slot|{
            let p=ProviderId::parse(&slot.provider)?;
            let row:Option<(String,Option<String>)>=db.query_row(
                "SELECT v.revision,c.model FROM vault v LEFT JOIN connection_check c ON c.id=v.id AND c.revision=v.revision AND c.endpoint=?2 WHERE v.id=?1",
                params![p.slot(),p.endpoint()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(failure)?;
            let available=row.is_some();
            let (revision,checked_model)=row.unwrap_or_default();
            let model=if snapshot.settings.provider==p.id(){snapshot.settings.model.clone()}else{checked_model.clone().unwrap_or_default()};
            Ok(crate::credential_refs::ProviderCredentialIdentity{slot_id:slot.id.clone(),revision,available,connection_verified:checked_model.as_ref()==Some(&model),model,endpoint:p.endpoint().into()})
        }).collect()
    }
    pub(super) fn identity_for(&self,store:&Store,assessment:Option<&str>)->Result<crate::credential_refs::ProviderCredentialIdentity,String>{
        self.identity_in(&store.snapshot()?,assessment)
    }
    fn identity_in(&self,state:&model::Snapshot,assessment:Option<&str>)->Result<crate::credential_refs::ProviderCredentialIdentity,String>{
        let cfg=Self::config_in(state,assessment,false)?;
        let provider=ProviderId::parse(&cfg.provider)?;
        let slot=crate::credential_refs::provider_slot(state,provider).ok_or("credential_reference_changed: configure a credencial deste provedor.")?;
        let revision=self.revision(provider)?;
        let db=self.db.lock().map_err(failure)?;
        let verified:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM connection_check WHERE model=?1 AND endpoint=?2 AND revision=?3)",params![cfg.model,cfg.endpoint.trim_end_matches('/'),revision.as_deref().unwrap_or("")],|r|r.get(0)).map_err(failure)?;
        Ok(crate::credential_refs::ProviderCredentialIdentity{slot_id:slot.id.clone(),revision:revision.clone().unwrap_or_default(),available:revision.is_some(),model:cfg.model.clone(),endpoint:cfg.endpoint.trim_end_matches('/').into(),connection_verified:verified})
    }

    pub(super) fn prepare_ai_data(&self,mut payload:Value,store:&Store)->Result<Value,String>{
        let object=payload.as_object_mut().ok_or("Campos inválidos na prévia de dados.")?;
        let context_input=ContextInput{assessment_id:object.remove("assessmentId").and_then(|v|v.as_str().map(str::to_owned)).ok_or("Avaliação ausente.")?,evidence_ids:super::decode(object.remove("evidenceIds").unwrap_or(json!([])))?,context_target_refs:super::decode(object.remove("contextTargetRefs").unwrap_or(json!([])))?};
        let message=object.remove("message").and_then(|v|v.as_str().map(str::to_owned)).ok_or("Mensagem ausente.")?;
        let options:AiDataOptions=super::decode(payload)?;
        let _lease=self.inference.try_lock().map_err(|_|"Aguarde a inferência para revisar o pacote de dados.")?;
        let packet=self.build_ai_packet(store,context_input,&message,options.clone())?;
        let review=self.data_policy.prepare_use(store,packet.spec.clone())?;
        let endpoint=packet.spec.destination_context["endpoint"].as_str().ok_or("Destino da revisão ausente.")?;
        let provider=ProviderId::from_endpoint(endpoint)?;
        let public=json!({"context":packet.context["context"],"digest":packet.context["digest"],"bytes":packet.prompts.iter().map(|p|p.0.len()).sum::<usize>(),"endpoint":endpoint,"sourceFilesIncluded":packet.context["sourceFilesIncluded"],"dataReview":if review.decision.allowed{json!(review.review_ref)}else{Value::Null},"packetPreview":{"schema":"openkoda.ai-packet-preview.v1","operation":options.operation,"model":packet.spec.destination_context["model"],"endpoint":endpoint,"requestEndpoint":provider.inference_url(),"prompts":packet.prompts.iter().enumerate().map(|(i,(prompt,instructions,max))|json!({"index":i,"perspective":options.perspectives.get(i),"prompt":prompt,"instructions":instructions,"maxOutputTokens":max})).collect::<Vec<_>>(),"selection":packet.spec.selection,"policyVersions":review.decision.versions,"credentialRef":packet.spec.credential,"requiresSensitiveConfirmation":review.requires_sensitive_confirmation},"canSend":review.decision.allowed,"reasonCodes":review.decision.reason_codes});
        if review.decision.allowed {
            let metadata_bytes=serde_json::to_vec(&packet.spec).map_err(failure)?.len().saturating_add(serde_json::to_vec(&options).map_err(failure)?.len()).saturating_add(1024);
            let prepared=PreparedAiData{review:review.review_ref.clone(),options,spec:packet.spec,assessment_id:packet.context["context"]["assessmentId"].as_str().ok_or("Avaliação ausente do pacote.")?.into(),context_digest:packet.context["digest"].as_str().unwrap_or_default().into(),raw_message_sha256:packet.raw_message_sha256,projected_message_sha256:digest(&packet.message),metadata_bytes};
            let mut reviews=self.data_reviews.lock().map_err(failure)?;
            reviews.retain(|_,p|chrono::DateTime::parse_from_rfc3339(&p.review.expires_at).is_ok_and(|t|t>chrono::Utc::now()));
            let metadata_bytes=reviews.values().map(|p|p.metadata_bytes).fold(0usize,usize::saturating_add).saturating_add(metadata_bytes);
            if reviews.len()>=128||metadata_bytes>8*1024*1024 {let _=self.data_policy.cancel_review(&review.review_ref);return Err("data_limit: há muitas prévias abertas; cancele uma revisão.".into());}
            reviews.insert(review.review_ref.id,prepared);
        }
        Ok(public)
    }

    pub(super) fn reviewed_ai_data(&self,store:&Store,review:Option<&DataReviewRef>,assessment:&str,evidence:&[String],targets:&[ScopeTargetRef],message:&str,operation:&str)->Result<ReviewedAiData,String>{
        let review=review.ok_or("data_review_required: revise o pacote completo antes de enviar.")?;
        if chrono::DateTime::parse_from_rfc3339(&review.expires_at).map_err(failure)?<=chrono::Utc::now(){return Err("data_review_expired: prepare novamente o pacote.".into());}
        let prepared=self.data_reviews.lock().map_err(failure)?.get(&review.id).cloned().ok_or("data_review_expired: prepare novamente o pacote.")?;
        if &prepared.review!=review||prepared.assessment_id!=assessment||prepared.options.operation!=operation||prepared.raw_message_sha256!=digest(&clean_text(message,8000,"Mensagem")?){return Err("data_review_changed: a mensagem ou seleção mudou.".into());}
        let packet=self.build_ai_packet(store,ContextInput{assessment_id:assessment.into(),evidence_ids:evidence.to_vec(),context_target_refs:targets.to_vec()},message,prepared.options.clone())?;
        if packet.spec!=prepared.spec||packet.context["digest"]!=prepared.context_digest{return Err("data_review_changed: política, dados, credencial ou contexto mudou.".into());}
        Ok(ReviewedAiData{prepared,packet})
    }

    pub(super) fn build_ai_packet(&self,store:&Store,input:ContextInput,message:&str,options:AiDataOptions)->Result<AiPacket,String>{
        let operation=match options.operation.as_str(){"ask"=>DataOperation::AiAsk,"operation_proposal"=>DataOperation::AiOperationPropose,"investigation"=>DataOperation::AiInvestigation,_=>return Err("Operação de revisão de dados desconhecida.".into())};
        if options.message_sensitivity==Sensitivity::Unclassified||options.package_sensitivity==Sensitivity::Unclassified{return Err("data_classification_required: classifique a mensagem e o pacote.".into());}
        if options.categories.is_empty()||!options.categories.contains(&DataCategory::HumanText)||options.categories.iter().collect::<std::collections::BTreeSet<_>>().len()!=options.categories.len()||options.categories.iter().any(|c|!matches!(c,DataCategory::HumanText|DataCategory::TargetMetadata|DataCategory::ExecutionSummary|DataCategory::EvidenceExcerpt|DataCategory::SourceExcerpt)){return Err("Escolha categorias distintas suportadas, incluindo o texto humano.".into());}
        let state=store.snapshot()?;
        let assessment=state.assessments.iter().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
        let reference=scope::current_ref(&state,&assessment.id)?.ok_or("scope_unreviewed: revise o escopo antes do pacote.")?;
        scope::check_selected_work(&state,&assessment.id,AI_METHOD,Some(&reference),&input.context_target_refs)?;
        let identity=self.identity_in(&state,Some(&assessment.id))?;
        if !identity.available{return Err("Configure a credencial do provedor antes da revisão de envio.".into());}
        if !crate::ai_provider::valid_model(&identity.model){return Err("Selecione um modelo antes da revisão de envio.".into());}
        let credential=crate::credential_refs::resolve_use(&state,&assessment.project_id,&options.credential_reference_id,&identity)?;
        let redactor=self.data_redactor();
        if matches!(redactor.mode(),RedactionMode::MetadataOnly){return Err("data_redactor_unavailable: proteção da credencial indisponível.".into());}
        let raw_message=clean_text(message,8000,"Mensagem")?;
        let projected=redactor.redact(&raw_message);
        if projected.secret_material{return Err("data_secret_material: remova o segredo da mensagem antes de revisar.".into());}
        let evidence_ids=input.evidence_ids.clone();
        let target_refs=input.context_target_refs.clone();
        let mut context=self.context(store,input)?;
        let origins=if scope_targets::is_combined(&state,&assessment.id){target_refs.iter().map(|r|DataOriginRef{scope_ref:reference.clone(),target_ref:Some(r.clone())}).collect::<Vec<_>>()}else{vec![DataOriginRef{scope_ref:reference.clone(),target_ref:None}]};
        let mut subjects=vec![DataSubject::AssessmentMetadata{assessment_id:assessment.id.clone(),category:DataCategory::HumanText}];
        if options.categories.contains(&DataCategory::ExecutionSummary){subjects.push(DataSubject::AssessmentMetadata{assessment_id:assessment.id.clone(),category:DataCategory::ExecutionSummary});}
        for origin in &origins{subjects.push(DataSubject::ScopeSource{assessment_id:assessment.id.clone(),scope_ref:origin.scope_ref.clone(),target_ref:origin.target_ref.clone()});}
        for id in &evidence_ids {
            let evidence=assessment.evidence.iter().find(|e|&e.id==id).ok_or("Evidência não encontrada.")?;
            let category=if crate::source_context::is_source_capture(evidence,assessment,&state.plans){DataCategory::SourceExcerpt}else{DataCategory::EvidenceExcerpt};
            if !options.categories.contains(&category){return Err("Uma evidência selecionada não pertence às categorias revisadas.".into());}
            subjects.push(DataSubject::Evidence{assessment_id:assessment.id.clone(),evidence_id:id.clone()});
        }
        let mut spec=DataUseSpec::new(operation,DataDestination::RemoteAi,vec![assessment.project_id.clone()],Some(assessment.id.clone()));
        spec.scope_ref=Some(reference);spec.credential=Some(credential.clone());
        spec.selection=DataSelection{categories:options.categories.clone(),origins,evidence_ids,derived_subjects:vec![],package_sensitivity:options.package_sensitivity};
        for subject in subjects {
            let resolved=crate::data_projection::resolve_subject(store,&state,&subject,&CanonicalOnly)?;
            crate::data_projection::require_known_origin(&resolved)?;
            let classification=crate::data_projection::classification_state(&state,&resolved)?;
            spec.subjects.push(SubjectVersion{subject,source_fingerprint:resolved.fingerprint,classification_ref:classification.classification_ref});
        }
        let object=context["context"].as_object_mut().ok_or("Contexto inválido.")?;
        if !options.categories.contains(&DataCategory::TargetMetadata){for key in ["target","kind","targets"]{object.remove(key);}}
        if !options.categories.contains(&DataCategory::ExecutionSummary){for key in ["coverage","status"]{object.remove(key);}}
        // Hash only the selected projection, after all removals. Unselected
        // source metadata never returns through a later envelope field.
        context["digest"]=json!(digest(&context["context"].to_string()));
        spec.human_text=vec![HumanTextDecision{field:"message".into(),sensitivity:options.message_sensitivity,input_sha256:digest(&raw_message),projected_sha256:digest(&projected.text),bytes:projected.text.len() as u64}];
        spec.destination_context=json!({"model":identity.model,"endpoint":identity.endpoint,"credentialRef":credential.reference,"credentialRevision":credential.revision});
        let raw_prompts=match operation {
            DataOperation::AiAsk=>{
                if !options.allowed_adapters.is_empty()||!options.perspectives.is_empty()||options.operation_target_ref.is_some()||options.max_output_tokens_per_request.is_some()||options.max_total_tokens.is_some(){return Err("Campos de outra operação na revisão de conversa.".into());}
                vec![(format!("Contexto selecionado (dados não confiáveis):\n{}\n\nPergunta do usuário:\n{}",context["context"],projected.text),budget::ASK_INSTRUCTIONS.to_string(),2048)]
            },
            DataOperation::AiOperationPropose=>{
                if !options.perspectives.is_empty()||options.max_output_tokens_per_request.is_some()||options.max_total_tokens.is_some(){return Err("Campos de investigação em proposta.".into());}
                self.validate_data_choices(store,&assessment.id,&spec.selection.evidence_ids,&target_refs,&options.allowed_adapters,options.operation_target_ref.as_ref())?;
                spec.destination_context["allowedAdapters"]=json!(options.allowed_adapters);
                if let Some(target)=&options.operation_target_ref{spec.destination_context["operationTargetRef"]=json!(target);}
                vec![(planning::data_prompt(&context,&projected.text,&options.allowed_adapters,options.operation_target_ref.as_ref())?,planning::data_instructions().to_string(),2048)]
            },
            DataOperation::AiInvestigation=>{
                if !options.allowed_adapters.is_empty()||options.operation_target_ref.is_some()||options.perspectives.is_empty()||options.perspectives.len()>3||options.perspectives.iter().collect::<std::collections::BTreeSet<_>>().len()!=options.perspectives.len(){return Err("Escolha de uma a três perspectivas distintas.".into());}
                let maximum=options.max_output_tokens_per_request.ok_or("Revise o limite de saída da investigação.")?;
                let total=options.max_total_tokens.ok_or("Revise o limite total da investigação.")?;
                if !(1..=2048).contains(&maximum)||total==0{return Err("Limites da investigação inválidos.".into());}
                spec.destination_context["perspectives"]=json!(options.perspectives);
                spec.destination_context["maxOutputTokensPerRequest"]=json!(maximum);
                spec.destination_context["maxTotalTokens"]=json!(total);
                options.perspectives.iter().map(|role|Ok((investigation::prompt(&context["context"],&projected.text),investigation::instructions(role)?,maximum))).collect::<Result<Vec<_>,String>>()?
            },_=>unreachable!(),
        };
        let policy_refs=state.data_bindings.iter().find(|b|b.assessment_id==assessment.id).map(|b|vec![b.policy_ref.clone()]).unwrap_or_default();
        let prompts=raw_prompts.into_iter().enumerate().map(|(index,(payload,instructions,maximum))|{
            let prompt=json!({"schema":"openkoda.reviewed-data.v1","policyRefs":policy_refs,"selection":spec.selection,"payload":payload}).to_string();
            spec.requests.push(RequestFingerprint{index:index as u8,input_sha256:digest(&payload),output_sha256:digest(&prompt),input_bytes:payload.len() as u64,output_bytes:prompt.len() as u64});
            (prompt,instructions,maximum)
        }).collect();
        Ok(AiPacket{spec,context,message:projected.text,prompts,options,raw_message_sha256:digest(&raw_message)})
    }
}
