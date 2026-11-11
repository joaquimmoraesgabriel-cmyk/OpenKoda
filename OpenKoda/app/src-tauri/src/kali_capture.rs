//! Private producer/collector continuity. No raw stream or filesystem path is
//! returned by these types to IPC. Every collector is a journaled descendant.
use super::*;
use crate::{capture::{CaptureEncoding,CaptureLossReason,CaptureState,CaptureStream},data_policy::DataUseReceiptRef,workflow::ResultRef};
use sha2::{Digest,Sha256};
#[path="kali_retire.rs"]
pub(super) mod retire;
pub(crate) use retire::{KaliMetadataItem,KaliMetadataState,KaliRetirePreview,KaliRetireReceipt,KaliRetireState};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliSpoolRef {
    pub(crate) capture_id:String,pub(crate) assessment_id:String,pub(crate) run_id:String,pub(crate) environment_id:String,
    pub(crate) owner_profile_fingerprint:String,pub(crate) manifest_digest:Option<String>,pub(crate) preserved_bytes:Option<u64>,pub(crate) availability:String,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct KaliPurgeInspection {pub(crate) reference:KaliSpoolRef,pub(crate) digest:String,pub(crate) previous_receipt_id:Option<String>,pub(crate) previous_receipt_digest:Option<String>}
fn inspection(reference:KaliSpoolRef,previous_receipt_id:Option<String>,previous_receipt_digest:Option<String>)->KaliPurgeInspection{
    let digest=hash(&json!({"schema":"openkoda.kali-purge-inspection.v1","reference":reference,"previousReceiptId":previous_receipt_id,"previousReceiptDigest":previous_receipt_digest}).to_string());
    KaliPurgeInspection{reference,digest,previous_receipt_id,previous_receipt_digest}
}
#[derive(Clone,Debug,Serialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub(crate) struct KaliPurgeReceipt {pub(crate) plan_receipt_id:String,pub(crate) capture_id:String,pub(crate) run_id:String,pub(crate) control_run_id:String,pub(crate) manifest_digest:String,pub(crate) state:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct PurgeRecord {plan_receipt_id:String,capture_id:String,run_id:String,control_run_id:String,manifest_digest:String,state:String,
    #[serde(default,skip_serializing_if="Option::is_none")]linux_purge_proof:Option<retire::LinuxPurgeProof>}
impl PurgeRecord {
    fn view(&self)->KaliPurgeReceipt{KaliPurgeReceipt{plan_receipt_id:self.plan_receipt_id.clone(),capture_id:self.capture_id.clone(),run_id:self.run_id.clone(),control_run_id:self.control_run_id.clone(),manifest_digest:self.manifest_digest.clone(),state:self.state.clone()}}
    fn validate(&self,run:&BridgeRun,intent:&KaliControlIntent)->Result<(),String>{
        let KaliControlIntent::Purge{producer_session_id,capture_id,expected_manifest_digest,purge_receipt_id,..}=intent else{return Err(integrity());};
        if self.plan_receipt_id!=*purge_receipt_id||self.capture_id!=*capture_id||self.run_id!=*producer_session_id||self.control_run_id!=run.id||self.manifest_digest!=*expected_manifest_digest||!matches!(self.state.as_str(),"purged"|"remaining"|"unknown")||(self.state!="unknown"&&!run.cleanup_confirmed){return Err(integrity());}
        if let Some(proof)=&self.linux_purge_proof{if self.state!="purged"{return Err(integrity());}proof.validate_shape(intent)?;}Ok(())
    }
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub(crate) enum KaliRunContext {
    Producer{binding:KaliWorkBinding,spool_manifest:Option<KaliSpoolManifest>,#[serde(default)]spool_absence:Option<KaliSpoolAbsence>,receipt_refs:Vec<DataUseReceiptRef>,observed:Option<Value>},
    Control{intent:KaliControlIntent,receipt:Option<PurgeRecord>},
    RetireControl{
        intent:KaliControlIntent,
        #[serde(deserialize_with="crate::workflow::required_nullable")]
        reviewed:Option<retire::RetireObservation>,
        #[serde(deserialize_with="crate::workflow::required_nullable")]
        receipt:Option<retire::RetireRecord>,
    },
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub(crate) enum KaliControlIntent {
    Collect{producer_session_id:String,capture_id:String,owner_profile_fingerprint:String,producer_dispatch_deadline:u64},
    Purge{producer_session_id:String,capture_id:String,owner_profile_fingerprint:String,producer_dispatch_deadline:u64,expected_manifest_digest:String,purge_receipt_id:String,#[serde(default)]previous_receipt_id:Option<String>,#[serde(default)]previous_receipt_digest:Option<String>},
    RetireInspect{request:retire::RetireInspect},
    RetireApply{request:retire::RetireApply},
}
impl KaliControlIntent {
    pub(super) fn producer(&self)->&str{match self{Self::Collect{producer_session_id,..}|Self::Purge{producer_session_id,..}=>producer_session_id,Self::RetireInspect{request}=>&request.producer_session_id,Self::RetireApply{request}=>&request.producer_session_id}}
    pub(super) fn capture_id(&self)->&str{match self{Self::Collect{capture_id,..}|Self::Purge{capture_id,..}=>capture_id,Self::RetireInspect{request}=>&request.capture_id,Self::RetireApply{request}=>&request.capture_id}}
    pub(super) fn fingerprint(&self)->&str{match self{Self::Collect{owner_profile_fingerprint,..}|Self::Purge{owner_profile_fingerprint,..}=>owner_profile_fingerprint,Self::RetireInspect{request}=>&request.owner_profile_fingerprint,Self::RetireApply{request}=>&request.owner_profile_fingerprint}}
    fn mode(&self)->&'static str{match self{Self::Collect{..}=>"collect",Self::Purge{..}=>"purge",Self::RetireInspect{..}|Self::RetireApply{..}=>"retire"}}
    fn tool(&self)->&'static str{match self{Self::Collect{..}=>"capture_collect",Self::Purge{..}=>"capture_purge",Self::RetireInspect{..}|Self::RetireApply{..}=>"capture_retire"}}
    pub(super) fn control(&self)->Value{match self{
        Self::RetireInspect{request}=>{let mut v=serde_json::to_value(request).expect("closed inspect control");v["kind"]=json!("inspect");v},
        Self::RetireApply{request}=>{let mut v=serde_json::to_value(request).expect("closed retire control");v["kind"]=json!("apply");v},
        _=>{let mut v=serde_json::to_value(self).expect("closed capture control");v.as_object_mut().unwrap().remove("kind");v}
    }}
    pub(super) fn producer_deadline(&self)->u64{match self{Self::Collect{producer_dispatch_deadline,..}|Self::Purge{producer_dispatch_deadline,..}=>*producer_dispatch_deadline,Self::RetireInspect{request}=>request.producer_dispatch_deadline,Self::RetireApply{request}=>request.producer_dispatch_deadline}}
    pub(super) fn validate(&self)->Result<(),String>{valid_id(self.producer())?;valid_id(self.capture_id())?;if !crate::workflow::valid_hash(self.fingerprint())||self.producer_deadline()==0||self.producer_deadline()>crate::workflow::MAX_SAFE_INTEGER{return Err(integrity());}if let Self::Purge{expected_manifest_digest,purge_receipt_id,previous_receipt_id,previous_receipt_digest,..}=self{valid_id(purge_receipt_id)?;if !crate::workflow::valid_hash(expected_manifest_digest)||previous_receipt_id.is_some()!=previous_receipt_digest.is_some(){return Err(integrity());}if let Some(id)=previous_receipt_id{valid_id(id)?;if id==purge_receipt_id||!crate::workflow::valid_hash(previous_receipt_digest.as_deref().unwrap_or("")){return Err(integrity());}}}match self{Self::RetireInspect{request}=>request.validate(),Self::RetireApply{request}=>request.validate(),_=>Ok(())}}
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliSpoolAbsence {body:AbsenceBody,digest:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct AbsenceBody{schema:String,assessment_id:String,environment_id:String,capture_id:String,producer_session_id:String,owner_profile_fingerprint:String,producer_dispatch_deadline:u64,directory_absent:bool}
impl KaliSpoolAbsence {
    fn validate(&self,binding:&KaliWorkBinding,deadline:u64)->Result<(),String>{let b=&self.body;if b.schema!="openkoda.kali-spool-absence.v1"||b.assessment_id!=binding.assessment_id||b.environment_id!=binding.environment_id||b.producer_session_id!=binding.run_id||b.owner_profile_fingerprint!=binding.owner_profile_fingerprint||b.producer_dispatch_deadline!=deadline||!b.directory_absent||!binding.captures.iter().any(|slot|slot.stream==CaptureStream::ToolStdout&&slot.capture_id==b.capture_id)||hash(&serde_json::to_value(b).map_err(db_error)?.to_string())!=self.digest{return Err(integrity());}Ok(())}
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliSpoolManifest {pub(super) body:SpoolBody,pub(super) digest:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct ProducerIdentity{pid:u32,start_ticks:String,boot_id:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct ChildIdentity{
    #[serde(deserialize_with="crate::workflow::required_nullable")] pid:Option<u32>,
    #[serde(deserialize_with="crate::workflow::required_nullable")] start_ticks:Option<String>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct SpoolBody {
    schema:String,capture_id:String,attempt_id:String,assessment_id:String,environment_id:String,producer_session_id:String,owner_profile_fingerprint:String,
    executor_source_sha256:String,catalog_version:String,catalog_sha256:String,parser_version:String,input_evidence_id:String,input_sha256:String,
    stream:String,child_role:String,tool_binary_sha256:String,producer:ProducerIdentity,child:ChildIdentity,
    pub(super) phase:String,generation:u64,max_bytes:u64,pub(super) preserved_bytes:u64,pub(super) prefix_sha256:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")] pub(super) observed_bytes:Option<u64>,
    reached_eof:bool,
    #[serde(deserialize_with="crate::workflow::required_nullable")] pub(super) loss_reason:Option<CaptureLossReason>,
    pub(super) encoding:CaptureEncoding,
}
fn integrity()->String{"capture_integrity: captura Kali indisponível ou divergente; nenhum produtor foi repetido.".into()}
fn raw_hash(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
pub(super) fn profile_fingerprint(store:&Store)->Result<String,String>{
    profile_fingerprint_root(&store.root)
}
fn profile_fingerprint_root(root:&std::path::Path)->Result<String,String>{
    let path=fs::canonicalize(root).map_err(db_error)?;
    let text=path.to_str().ok_or_else(integrity)?;
    #[cfg(windows)] let text=text.to_lowercase();
    Ok(hash(&format!("openkoda.owner-profile.v1\n{text}")))
}
/// Pure journal eligibility check. Foreign-profile metadata carries no Linux
/// authority. A launched owned producer must retain its binding until a matching
/// durable retention receipt confirms removal of the original spool.
pub(crate) fn ensure_capture_history_purgeable(root:&std::path::Path,runs:&[BridgeRun],assessment_ids:&[String])->Result<(),String>{
    let owner=profile_fingerprint_root(root)?;
    for id in assessment_ids{valid_id(id)?;}
    for producer in runs.iter().filter(|r|assessment_ids.contains(&r.assessment_id)){
        validate_context(producer)?;
        let Some(KaliRunContext::Producer{binding,spool_manifest,spool_absence,..})=&producer.work_context else{continue;};
        if binding.owner_profile_fingerprint!=owner||!producer.launch_attempted||binding.operation!=crate::workflow::KaliOperation::Scan{continue;}
        if work::unresolved(producer)||runs.iter().any(|r|r.reconciliation_of.as_deref()==Some(&producer.id)&&work::unresolved(r)){return Err("work_reconciliation_required".into());}
        let has_retire=runs.iter().any(|r|r.reconciliation_of.as_deref()==Some(&producer.id)&&matches!(&r.work_context,Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{..},..})));
        if let Some(absence)=spool_absence{if !has_retire{absence.validate(binding,producer.dispatch_before_unix_seconds)?;continue;}}
        let manifest=spool_manifest.as_ref().ok_or("kali_capture_retention_required")?;manifest.validate(binding)?;
        let done=runs.iter().any(|control|{
            if !control.cleanup_confirmed||control.reconciliation_of.as_deref()!=Some(&producer.id)||control.assessment_id!=producer.assessment_id||control.environment_id!=producer.environment_id{return false;}
            let Some(KaliRunContext::Control{intent,receipt:Some(receipt)})=&control.work_context else{return false;};
            validate_context(control).is_ok()&&receipt.state=="purged"&&receipt.capture_id==manifest.body.capture_id&&receipt.manifest_digest==manifest.digest&&intent.fingerprint()==owner
        });
        if !done{return Err("kali_capture_retention_required: remova ou inspecione o spool Linux antes de expurgar seu histórico.".into());}
        if !retire::history_retired(producer,runs)?{return Err("kali_capture_metadata_retention_required: consolide os metadados terminais no Kali antes de expurgar seu histórico.".into());}
    }Ok(())
}
pub(super) fn require_owned(store:&Store,binding:&KaliWorkBinding)->Result<(),String>{if binding.owner_profile_fingerprint!=profile_fingerprint(store)?{return Err("capture_owner_unavailable: este perfil contém histórico, mas não possui o spool Linux original.".into());}Ok(())}
impl KaliSpoolManifest {
    pub(super) fn validate(&self,binding:&KaliWorkBinding)->Result<(),String>{
        binding.validate()?;let b=&self.body;
        let Some(slot)=binding.captures.iter().find(|s|s.stream==CaptureStream::ToolStdout) else{return Err(integrity());};
        let ResultRef::Evidence{id,raw_sha256}=&binding.input_refs[0] else{return Err(integrity());};
        if b.schema!="openkoda.kali-spool.v1"||b.capture_id!=slot.capture_id||b.attempt_id!=binding.attempt_id||b.assessment_id!=binding.assessment_id
            ||b.environment_id!=binding.environment_id||b.producer_session_id!=binding.run_id||b.owner_profile_fingerprint!=binding.owner_profile_fingerprint
            ||b.executor_source_sha256!=binding.executor_source_sha256||b.catalog_version!=binding.catalog_version||b.catalog_sha256!=binding.catalog_sha256
            ||Some(&b.parser_version)!=binding.parser_version.as_ref()||b.input_evidence_id!=*id||b.input_sha256!=*raw_sha256
            ||b.stream!="tool_stdout"||b.child_role!="scan"||!matches!(b.phase.as_str(),"open"|"captured"|"sealed"|"interrupted")
            ||b.max_bytes!=65536||b.preserved_bytes>b.max_bytes||b.generation==0||b.generation>1_000_000
            ||b.observed_bytes.is_some_and(|n|n<b.preserved_bytes||n>b.max_bytes)||!crate::workflow::valid_hash(&b.tool_binary_sha256)||!crate::workflow::valid_hash(&b.prefix_sha256)
            ||b.producer.pid==0||!digits(&b.producer.start_ticks)||b.child.pid.is_some()!=b.child.start_ticks.is_some()
            ||b.child.pid==Some(0)||b.child.start_ticks.as_ref().is_some_and(|s|!digits(s)) {return Err(integrity());}
        valid_id(&b.producer.boot_id)?;
        if hash(&serde_json::to_value(b).map_err(db_error)?.to_string())!=self.digest{return Err(integrity());}Ok(())
    }
}
fn digits(s:&str)->bool{!s.is_empty()&&s.len()<=32&&s.bytes().all(|b|b.is_ascii_digit())}
pub(super) fn producer_binding(run:&BridgeRun)->Result<&KaliWorkBinding,String>{match &run.work_context{Some(KaliRunContext::Producer{binding,..})=>Ok(binding),_=>Err(integrity())}}
pub(super) fn validate_context(run:&BridgeRun)->Result<(),String>{
    match &run.work_context {
        None if !matches!(run.tool.as_str(),"capture_collect"|"capture_purge"|"capture_retire")=>Ok(()),
        Some(KaliRunContext::Producer{binding,spool_manifest,spool_absence,receipt_refs,observed})=>{
            binding.validate()?;
            if binding.run_id!=run.id||binding.assessment_id!=run.assessment_id||binding.environment_id!=run.environment_id||binding.executor_source_sha256!=run.source_sha256
                ||run.reconciliation_of.is_some()||run.tool!=match binding.operation{crate::workflow::KaliOperation::Scan=>"scan_bytes",crate::workflow::KaliOperation::Hash=>"hash_bytes"}
                ||receipt_refs.len()!=1||observed.as_ref().is_some_and(|v|!v.is_object()||v.to_string().len()>8192){return Err(integrity());}
            for receipt in receipt_refs{valid_id(&receipt.id)?;if !crate::workflow::valid_hash(&receipt.digest){return Err(integrity());}}
            if let Some(m)=spool_manifest{m.validate(binding)?;}if let Some(absence)=spool_absence{absence.validate(binding,run.dispatch_before_unix_seconds)?;}Ok(())
        },
        Some(KaliRunContext::Control{intent,receipt})=>{intent.validate()?;if !matches!(intent,KaliControlIntent::Collect{..}|KaliControlIntent::Purge{..})||run.reconciliation_of.as_deref()!=Some(intent.producer())||run.tool!=intent.tool(){return Err(integrity());}if let Some(receipt)=receipt{receipt.validate(run,intent)?;}Ok(())},
        Some(KaliRunContext::RetireControl{intent,reviewed,receipt})=>retire::validate_control(run,intent,reviewed.as_ref(),receipt.as_ref()),
        _=>Err(integrity()),
    }
}
pub(super) fn bootstrap_context(run:&BridgeRun)->Result<(&'static str,Value,Value),String>{
    validate_context(run)?;
    match &run.work_context {
        Some(KaliRunContext::Producer{binding,..})=>{
            let capture=match binding.operation{crate::workflow::KaliOperation::Hash=>Value::Null,crate::workflow::KaliOperation::Scan=>{
                let slot=&binding.captures[0];let ResultRef::Evidence{id,raw_sha256}=&binding.input_refs[0] else{return Err(integrity());};
                json!({"schema":"openkoda.kali-spool-binding.v1","captureId":slot.capture_id,"attemptId":binding.attempt_id,"inputEvidenceId":id,"inputSha256":raw_sha256,"maxBytes":slot.max_bytes,"ownerProfileFingerprint":binding.owner_profile_fingerprint})
            }};Ok(("execute",capture,Value::Null))
        },
        Some(KaliRunContext::Control{intent,..}|KaliRunContext::RetireControl{intent,..})=>Ok((intent.mode(),Value::Null,intent.control())),
        None=>Ok((if run.reconciliation_of.is_some(){"inspect"}else{"execute"},Value::Null,Value::Null)),
    }
}

fn control_answer(value:Value,run:&BridgeRun,id:i64,operation:&str)->Result<Value,String>{
    let reply=response(value,id)?;
    if reply["isError"]!=false{return Err("capture_unavailable: controle Kali recusou a captura registrada.".into());}
    let observed=reply["structuredContent"].clone();
    if observed["environmentId"]!=run.environment_id||observed["sessionId"]!=run.id||observed["assessmentId"]!=run.assessment_id||observed["operation"]!=operation{return Err(integrity());}Ok(observed)
}
fn fields(value:&Value,expected:&[&str])->bool{value.as_object().is_some_and(|object|object.len()==expected.len()&&object.keys().all(|key|expected.contains(&key.as_str())))}
impl KaliBridge {
    pub(crate) fn spool_metadata(&self,store:&Store,assessment_id:Option<&str>)->Result<Vec<KaliSpoolRef>,String>{
        if let Some(id)=assessment_id{valid_id(id)?;}
        let owner=profile_fingerprint(store)?;let runs=self.runs()?;let mut out=vec![];
        for run in &runs {
            if assessment_id.is_some_and(|id|id!=run.assessment_id){continue;}
            let Some(KaliRunContext::Producer{binding,spool_manifest,spool_absence,..})=&run.work_context else{continue;};
            let Some(slot)=binding.captures.iter().find(|s|s.stream==CaptureStream::ToolStdout) else{continue;};
            let purged=runs.iter().any(|control|matches!(&control.work_context,Some(KaliRunContext::Control{receipt:Some(r),..}) if r.capture_id==slot.capture_id&&r.state=="purged")&&control.cleanup_confirmed);
            let availability=if owner!=binding.owner_profile_fingerprint{"foreign_profile"}else if purged{"purged"}else if !run.launch_attempted||spool_absence.is_some(){"unavailable"}else{"owned"};
            out.push(KaliSpoolRef{capture_id:slot.capture_id.clone(),assessment_id:run.assessment_id.clone(),run_id:run.id.clone(),environment_id:run.environment_id.clone(),owner_profile_fingerprint:binding.owner_profile_fingerprint.clone(),manifest_digest:spool_manifest.as_ref().map(|m|m.digest.clone()),preserved_bytes:spool_manifest.as_ref().map(|m|m.body.preserved_bytes),availability:availability.into()});
        }Ok(out)
    }
    pub(crate) fn inspect_capture_purge(&self,store:&Store,reference:&KaliSpoolRef,identity:&dyn Fn()->Result<String,String>)->Result<KaliPurgeInspection,String>{
        let current=self.spool_metadata(store,Some(&reference.assessment_id))?.into_iter().find(|r|r.capture_id==reference.capture_id).ok_or_else(integrity)?;
        if current!=*reference||current.availability!="owned"{return Err("capture_owner_unavailable".into());}
        let mut producer=self.find(&reference.run_id)?;let binding=producer_binding(&producer)?.clone();require_owned(store,&binding)?;
        self.require_capture_terminal(store,reference)?;
        let intent=KaliControlIntent::Collect{producer_session_id:producer.id.clone(),capture_id:reference.capture_id.clone(),owner_profile_fingerprint:reference.owner_profile_fingerprint.clone(),producer_dispatch_deadline:producer.dispatch_before_unix_seconds};
        let (observed,_)=self.maintenance_control(store,&producer,intent,identity)?;
        if observed["availability"]=="absent"{self.record_absence(&mut producer,&binding,&observed)?;let reference=self.spool_metadata(store,Some(&reference.assessment_id))?.into_iter().find(|r|r.capture_id==reference.capture_id).ok_or_else(integrity)?;return Ok(inspection(reference,None,None));}
        let manifest:KaliSpoolManifest=serde_json::from_value(observed["manifest"].clone()).map_err(db_error)?;manifest.validate(&binding)?;
        if observed["kind"]!="describe"||observed["producerQuiescent"]!=true||observed["availability"]!="present"||!observed["absence"].is_null(){return Err("capture_unavailable".into());}
        let previous=if observed["purgeReceipt"].is_null(){None}else{
            let receipt=&observed["purgeReceipt"];let id=receipt["purgeReceiptId"].as_str().ok_or_else(integrity)?;
            let old=self.runs()?.into_iter().find(|run|matches!(&run.work_context,Some(KaliRunContext::Control{intent:KaliControlIntent::Purge{purge_receipt_id,..},receipt:Some(record)}) if purge_receipt_id==id&&record.state=="remaining")).ok_or("capture_purge_reconciliation_required")?;
            let Some(KaliRunContext::Control{intent,..})=&old.work_context else{return Err(integrity());};
            if !old.cleanup_confirmed||validate_linux_purge(receipt,&producer,intent)?{return Err("capture_unavailable".into());}
            Some((id.to_owned(),hash(&receipt.to_string())))
        };
        if let Some(KaliRunContext::Producer{spool_manifest,..})=&mut producer.work_context{if spool_manifest.as_ref().is_some_and(|m|m!=&manifest){return Err(integrity());}*spool_manifest=Some(manifest);}
        self.save(&producer)?;
        let reference=self.spool_metadata(store,Some(&reference.assessment_id))?.into_iter().find(|r|r.capture_id==reference.capture_id).ok_or_else(integrity)?;
        let(previous_id,previous_digest)=previous.map_or((None,None),|(id,digest)|(Some(id),Some(digest)));Ok(inspection(reference,previous_id,previous_digest))
    }
    pub(crate) fn execute_capture_purge(&self,store:&Store,inspection:&KaliPurgeInspection,plan_receipt_id:&str,expected_inspection_digest:&str,identity:&dyn Fn()->Result<String,String>)->Result<KaliPurgeReceipt,String>{
        valid_id(plan_receipt_id)?;
        if inspection.digest!=expected_inspection_digest||inspection.digest!=self::inspection(inspection.reference.clone(),inspection.previous_receipt_id.clone(),inspection.previous_receipt_digest.clone()).digest{return Err("capture_inspection_changed".into());}
        let current=self.spool_metadata(store,Some(&inspection.reference.assessment_id))?.into_iter().find(|r|r.capture_id==inspection.reference.capture_id).ok_or_else(integrity)?;
        if current!=inspection.reference||current.availability!="owned"{return Err("capture_inspection_changed".into());}
        if self.runs()?.iter().any(|r|matches!(&r.work_context,Some(KaliRunContext::Control{intent:KaliControlIntent::Purge{purge_receipt_id,..},..}) if purge_receipt_id==plan_receipt_id)){return Err("capture_purge_already_dispatched: inspecione o resultado registrado.".into());}
        self.require_capture_terminal(store,&current)?;
        let producer=self.find(&current.run_id)?;require_owned(store,producer_binding(&producer)?)?;
        let intent=KaliControlIntent::Purge{producer_session_id:producer.id.clone(),capture_id:current.capture_id,owner_profile_fingerprint:current.owner_profile_fingerprint,producer_dispatch_deadline:producer.dispatch_before_unix_seconds,expected_manifest_digest:current.manifest_digest.ok_or_else(integrity)?,purge_receipt_id:plan_receipt_id.into(),previous_receipt_id:inspection.previous_receipt_id.clone(),previous_receipt_digest:inspection.previous_receipt_digest.clone()};
        let (_,run)=self.maintenance_control(store,&producer,intent,identity)?;
        let Some(KaliRunContext::Control{receipt:Some(receipt),..})=&run.work_context else{return Err(integrity());};Ok(receipt.view())
    }
    pub(crate) fn reconcile_capture_purge(&self,store:&Store,plan_receipt_id:&str,identity:&dyn Fn()->Result<String,String>)->Result<KaliPurgeReceipt,String>{
        valid_id(plan_receipt_id)?;
        let mut control=self.runs()?.into_iter().find(|r|matches!(&r.work_context,Some(KaliRunContext::Control{intent:KaliControlIntent::Purge{purge_receipt_id,..},..}) if purge_receipt_id==plan_receipt_id)).ok_or("capture_purge_missing")?;
        let Some(KaliRunContext::Control{intent,..})=&control.work_context else{return Err(integrity());};let intent=intent.clone();
        let producer=self.find(intent.producer())?;require_owned(store,producer_binding(&producer)?)?;
        // Reconcile only known process identities. Never dispatch capture_purge
        // again, even if the Linux tombstone says the first deletion is partial.
        for pending in self.runs()?.into_iter().filter(|r|r.reconciliation_of.as_deref()==Some(&producer.id)&&work::unresolved(r)){
            self.handle_with_identity("kali.mcp.reconcile",json!({"assessmentId":pending.assessment_id,"runId":pending.id}),store,identity,None)?;
        }
        if self.runs()?.iter().any(|r|r.reconciliation_of.as_deref()==Some(&producer.id)&&work::unresolved(r)){return Err("work_reconciliation_required".into());}
        // A lost final host acknowledgement can leave only the resource lease
        // unknown. Reuse the durable family proof; no diagnostic is launched.
        self.handle_with_identity("kali.mcp.reconcile",json!({"assessmentId":producer.assessment_id,"runId":producer.id}),store,identity,None)?;
        control=self.find(&control.id)?;
        if let Some(KaliRunContext::Control{receipt:Some(receipt),..})=&control.work_context{if receipt.state=="purged"&&control.cleanup_confirmed{return Ok(receipt.view());}}
        let read=KaliControlIntent::Collect{producer_session_id:producer.id.clone(),capture_id:intent.capture_id().into(),owner_profile_fingerprint:intent.fingerprint().into(),producer_dispatch_deadline:producer.dispatch_before_unix_seconds};
        let (observed,_)=self.maintenance_control(store,&producer,read,identity)?;
        let confirmed=validate_linux_purge(&observed["purgeReceipt"],&producer,&intent)?;
        control=self.find(&control.id)?;
        let mut record=purge_record_for(&control,&intent,confirmed&&control.cleanup_confirmed)?;
        if confirmed&&control.cleanup_confirmed{record.linux_purge_proof=Some(retire::LinuxPurgeProof::observed(&observed["purgeReceipt"],&producer,&intent)?);}
        if !confirmed&&control.cleanup_confirmed{record.state="remaining".into();}
        if let Some(KaliRunContext::Control{receipt,..})=&mut control.work_context{*receipt=Some(record.clone());}self.save(&control)?;Ok(record.view())
    }
    fn require_capture_terminal(&self,store:&Store,reference:&KaliSpoolRef)->Result<(),String>{store.work_read(|conn,_|{let m=crate::capture::load_locked(conn,&reference.capture_id)?;let a=crate::work_store::attempt(conn,&m.attempt_id)?;if m.state==CaptureState::Open||a.view.state.active()||a.view.cleanup!=crate::workflow::CleanupState::Confirmed{return Err("work_reconciliation_required".into());}Ok(())})}
    fn maintenance_control(&self,store:&Store,producer:&BridgeRun,intent:KaliControlIntent,identity:&dyn Fn()->Result<String,String>)->Result<(Value,BridgeRun),String>{
        use crate::work_supervisor::{EffectOwner,EffectOwnerKind,EffectTermination};
        let mut permit=self.arbiter.claim_direct(store,EffectOwner{id:producer.id.clone(),assessment_id:Some(producer.assessment_id.clone()),kind:EffectOwnerKind::KaliDiagnosticIntent})?;
        let mut bound=false;let mut final_run=None;
        let result=(||{
            if identity()?!=producer.environment_id{return Err("environment_identity_changed".into());}
            let mut active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;if active.is_some(){return Err("work_busy".into());}
            let mut run=self.new_control(producer,intent.clone())?;self.save(&run)?;final_run=Some(run.clone());
            self.arbiter.bind_effect(&mut permit,crate::workflow::EffectRef::KaliRun{id:producer.id.clone()})?;bound=true;
            let cancel=Arc::new(AtomicBool::new(false));*active=Some(Active{run_id:producer.id.clone(),assessment_id:producer.assessment_id.clone(),cancel:cancel.clone()});drop(active);
            run.launch_attempted=true;self.save(&run)?;final_run=Some(run.clone());
            let mut transport=self.launcher.launch(&run,&cancel)?;
            let deadline=Instant::now()+Duration::from_secs(45);
            let args=match &intent{KaliControlIntent::Collect{producer_session_id,capture_id,..}=>json!({"kind":"describe","producerSessionId":producer_session_id,"captureId":capture_id}),KaliControlIntent::Purge{producer_session_id,capture_id,expected_manifest_digest,purge_receipt_id,previous_receipt_id,previous_receipt_digest,..}=>json!({"producerSessionId":producer_session_id,"captureId":capture_id,"expectedManifestDigest":expected_manifest_digest,"purgeReceiptId":purge_receipt_id,"previousReceiptId":previous_receipt_id,"previousReceiptDigest":previous_receipt_digest}),_=>return Err(integrity())};
            let observed=(||{negotiate(&mut *transport,&run,deadline,&cancel)?;transport.send(request(&run,3,"tools/call",json!({"name":run.tool,"arguments":args})),deadline,&cancel)?;control_answer(transport.receive(deadline,&cancel)?,&run,3,&run.tool)})();
            let finish=transport.finish(&run,Instant::now()+Duration::from_secs(5));
            run.cleanup_confirmed=finish.cleanup_confirmed;run.finished_at=if finish.cleanup_confirmed{Some(model::now())}else{None};run.status=if !finish.cleanup_confirmed{"unknown"}else if observed.is_ok(){"completed"}else{"failed"}.into();
            if matches!(intent,KaliControlIntent::Purge{..}){let confirmed=observed.as_ref().ok().map(|o|validate_linux_purge(&o["receipt"],producer,&intent)).transpose()?.unwrap_or(false)&&finish.cleanup_confirmed;let mut record=purge_record_for(&run,&intent,confirmed)?;if confirmed{record.linux_purge_proof=Some(retire::LinuxPurgeProof::observed(&observed.as_ref().map_err(|e|e.clone())?["receipt"],producer,&intent)?);}if let Some(KaliRunContext::Control{receipt,..})=&mut run.work_context{*receipt=Some(record);}}
            self.save(&run)?;final_run=Some(run.clone());
            if !finish.cleanup_confirmed{return Err("work_unknown_effect".into());}
            Ok((observed?,run))
        })();
        *self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?=None;
        if result.is_err(){if let Some(run)=final_run.as_mut(){if !run.cleanup_confirmed{run.status=if run.launch_attempted{"unknown"}else{"failed"}.into();run.cleanup_confirmed=!run.launch_attempted;run.finished_at=if run.cleanup_confirmed{Some(model::now())}else{None};self.save(run)?;}}}
        let termination=if !bound{EffectTermination::NotStarted}else if final_run.as_ref().is_some_and(|r|r.cleanup_confirmed){EffectTermination::Stopped}else{EffectTermination::Unknown};
        self.arbiter.finish(permit,termination)?;result
    }
    pub(super) fn collect_spool(&self,store:&Store,producer:&mut BridgeRun,cancel:&AtomicBool)->Result<KaliSpoolManifest,String>{
        let binding=producer_binding(producer)?.clone();require_owned(store,&binding)?;
        if work::unresolved(producer){return Err("work_reconciliation_required".into());}
        check_wait(Instant::now()+Duration::from_secs(1),cancel)?;
        let capture_id=binding.captures.iter().find(|slot|slot.stream==CaptureStream::ToolStdout).ok_or_else(integrity)?.capture_id.clone();
        if self.runs()?.iter().any(|run|run.reconciliation_of.as_deref()==Some(&producer.id)&&work::unresolved(run)){return Err("work_reconciliation_required".into());}
        let intent=KaliControlIntent::Collect{producer_session_id:producer.id.clone(),capture_id:capture_id.clone(),owner_profile_fingerprint:binding.owner_profile_fingerprint.clone(),producer_dispatch_deadline:producer.dispatch_before_unix_seconds};
        let mut run=self.new_control(producer,intent)?;
        // This intent is durable before any Linux launch. The original permit
        // remains owned by the caller throughout every collection page.
        self.save(&run)?;
        run.launch_attempted=true;self.save(&run)?;
        let mut transport=match self.launcher.launch(&run,cancel){Ok(t)=>t,Err(e)=>{run.status="unknown".into();run.error=Some("O lançamento do coletor não obteve confirmação de término.".into());self.save(&run)?;return Err(e);}};
        run.status="running".into();self.save(&run)?;
        let deadline=Instant::now()+Duration::from_secs(45);
        let collected=(||{
            negotiate(&mut *transport,&run,deadline,cancel)?;
            transport.send(request(&run,3,"tools/call",json!({"name":"capture_collect","arguments":{"kind":"describe","producerSessionId":producer.id,"captureId":capture_id}})),deadline,cancel)?;
            let reply=control_answer(transport.receive(deadline,cancel)?,&run,3,"capture_collect")?;
            if !fields(&reply,&["environmentId","sessionId","assessmentId","operation","kind","manifest","producerQuiescent","purgeReceipt","availability","absence"])||reply["kind"]!="describe"||reply["producerQuiescent"]!=true||!reply["purgeReceipt"].is_null(){return Err(integrity());}
            if reply["availability"]=="absent"{self.record_absence(producer,&binding,&reply)?;return Err("capture_absent_confirmed".into());}
            if reply["availability"]!="present"||!reply["absence"].is_null(){return Err(integrity());}
            let manifest:KaliSpoolManifest=serde_json::from_value(reply["manifest"].clone()).map_err(db_error)?;manifest.validate(&binding)?;
            if let Some(KaliRunContext::Producer{spool_manifest,..})=&mut producer.work_context{if spool_manifest.as_ref().is_some_and(|m|m!=&manifest){return Err(integrity());}*spool_manifest=Some(manifest.clone());}else{return Err(integrity());}
            self.save(producer)?;
            let mut offset=0_u64;let mut request_id=4_i64;
            while offset<manifest.body.preserved_bytes {
                transport.send(request(&run,request_id,"tools/call",json!({"name":"capture_collect","arguments":{"kind":"read","producerSessionId":producer.id,"captureId":capture_id,"expectedManifestDigest":manifest.digest,"offset":offset,"maxBytes":4096}})),deadline,cancel)?;
                let page=control_answer(transport.receive(deadline,cancel)?,&run,request_id,"capture_collect")?;
                if !fields(&page,&["environmentId","sessionId","assessmentId","operation","kind","manifestDigest","offset","bytes","base64","eof"])||page["kind"]!="read"||page["manifestDigest"]!=manifest.digest||page["offset"].as_u64()!=Some(offset){return Err(integrity());}
                let encoded=page["base64"].as_str().filter(|s|s.len()<=5464).ok_or_else(integrity)?;
                let bytes=STANDARD.decode(encoded).map_err(|_|integrity())?;
                let expected=(manifest.body.preserved_bytes-offset).min(4096) as usize;
                if bytes.len()!=expected||STANDARD.encode(&bytes)!=encoded||page["bytes"].as_u64()!=Some(expected as u64)||page["eof"].as_bool()!=Some(offset+expected as u64==manifest.body.preserved_bytes){return Err(integrity());}
                store.capture_transaction(|conn|{let m=crate::capture::append_stream_locked(conn,&capture_id,offset,&bytes)?;crate::work_store::append_capture_event(conn,&m,false)?;Ok(())})?;
                offset+=expected as u64;request_id+=1;
            }
            store.work_read(|conn,_|{let m=crate::capture::load_locked(conn,&capture_id)?;let bytes=crate::capture::preserved_locked(conn,&m)?;if bytes.len() as u64!=manifest.body.preserved_bytes||raw_hash(&bytes)!=manifest.body.prefix_sha256{return Err(integrity());}Ok(())})?;
            Ok(manifest)
        })();
        let finished=transport.finish(&run,Instant::now()+Duration::from_secs(5));
        run.cleanup_confirmed=finished.cleanup_confirmed;run.finished_at=if finished.cleanup_confirmed{Some(model::now())}else{None};
        run.status=if !finished.cleanup_confirmed{"unknown"}else if cancel.load(Ordering::SeqCst){"canceled"}else if collected.is_ok(){"completed"}else{"failed"}.into();
        run.error=if run.status=="completed"{None}else{Some("Coleta incompleta; somente o prefixo confirmado foi preservado. O produtor não será repetido.".into())};self.save(&run)?;
        if !finished.cleanup_confirmed{return Err("work_unknown_effect: término do coletor Linux não confirmado.".into());}
        collected
    }
    fn new_control(&self,producer:&BridgeRun,intent:KaliControlIntent)->Result<BridgeRun,String>{
        if self.runs()?.len()>=10000{return Err("work_journal_quota".into());}
        let context=if matches!(intent,KaliControlIntent::RetireInspect{..}|KaliControlIntent::RetireApply{..}){KaliRunContext::RetireControl{intent:intent.clone(),reviewed:None,receipt:None}}else{KaliRunContext::Control{intent:intent.clone(),receipt:None}};
        Ok(BridgeRun{id:model::id(),assessment_id:producer.assessment_id.clone(),environment_id:producer.environment_id.clone(),tool:intent.tool().into(),status:"starting".into(),started_at:model::now(),finished_at:None,source_sha256:hash(SOURCE),evidence_id:None,error:None,cleanup_confirmed:false,launch_attempted:false,dispatch_before_unix_seconds:(chrono::Utc::now().timestamp()+30) as u64,source_evidence_id:None,scope_ref:None,source_target_refs:vec![],reconciliation_of:Some(producer.id.clone()),work_context:Some(context)})
    }
    fn record_absence(&self,producer:&mut BridgeRun,binding:&KaliWorkBinding,observed:&Value)->Result<(),String>{
        if self.runs()?.iter().any(|r|r.reconciliation_of.as_deref()==Some(&producer.id)&&matches!(&r.work_context,Some(KaliRunContext::RetireControl{..}))){return Err("capture_retire_reconciliation_required".into());}
        if observed["availability"]!="absent"||observed["producerQuiescent"]!=true||!observed["manifest"].is_null()||!observed["purgeReceipt"].is_null(){return Err(integrity());}
        let absence:KaliSpoolAbsence=serde_json::from_value(observed["absence"].clone()).map_err(db_error)?;absence.validate(binding,producer.dispatch_before_unix_seconds)?;
        if let Some(KaliRunContext::Producer{spool_absence,..})=&mut producer.work_context{*spool_absence=Some(absence);}else{return Err(integrity());}self.save(producer)
    }
    pub(super) fn complete_capture_copy(&self,store:&Store,producer:&BridgeRun)->Result<bool,String>{
        let Some(KaliRunContext::Producer{binding,spool_manifest:Some(manifest),..})=&producer.work_context else{return Ok(false);};manifest.validate(binding)?;
        store.work_read(|conn,_|{let m=crate::capture::load_locked(conn,&manifest.body.capture_id)?;let bytes=crate::capture::preserved_locked(conn,&m)?;Ok(bytes.len() as u64==manifest.body.preserved_bytes&&raw_hash(&bytes)==manifest.body.prefix_sha256)})
    }
}

fn purge_record_for(run:&BridgeRun,intent:&KaliControlIntent,confirmed:bool)->Result<PurgeRecord,String>{let KaliControlIntent::Purge{producer_session_id,capture_id,expected_manifest_digest,purge_receipt_id,..}=intent else{return Err(integrity());};Ok(PurgeRecord{plan_receipt_id:purge_receipt_id.clone(),capture_id:capture_id.clone(),run_id:producer_session_id.clone(),control_run_id:run.id.clone(),manifest_digest:expected_manifest_digest.clone(),state:if confirmed{"purged"}else{"unknown"}.into(),linux_purge_proof:None})}
fn validate_linux_purge(value:&Value,producer:&BridgeRun,intent:&KaliControlIntent)->Result<bool,String>{
    if value.is_null(){return Ok(false);}
    let KaliControlIntent::Purge{producer_session_id,capture_id,owner_profile_fingerprint,expected_manifest_digest,purge_receipt_id,previous_receipt_id,previous_receipt_digest,..}=intent else{return Err(integrity());};
    let Some(KaliRunContext::Producer{spool_manifest:Some(manifest),..})=&producer.work_context else{return Err(integrity());};
    let v2=value["schema"]=="openkoda.kali-spool-purge.v2";let mut keys=vec!["schema","captureId","producerSessionId","environmentId","assessmentId","manifestDigest","purgeReceiptId","preservedBytes","prefixSha256","state","ownerProfileFingerprint"];if v2{keys.extend(["previousReceiptId","previousReceiptDigest"]);}
    if !fields(value,&keys)||(!v2&&value["schema"]!="openkoda.kali-spool-purge.v1")||value["previousReceiptId"]!=json!(previous_receipt_id)||value["previousReceiptDigest"]!=json!(previous_receipt_digest)
        ||value["captureId"]!=*capture_id||value["producerSessionId"]!=*producer_session_id||value["environmentId"]!=producer.environment_id||value["assessmentId"]!=producer.assessment_id
        ||value["manifestDigest"]!=*expected_manifest_digest||value["purgeReceiptId"]!=*purge_receipt_id||value["preservedBytes"].as_u64()!=Some(manifest.body.preserved_bytes)||value["prefixSha256"]!=manifest.body.prefix_sha256||value["ownerProfileFingerprint"]!=*owner_profile_fingerprint
        ||!matches!(value["state"].as_str(),Some("purged"|"intent")){return Err(integrity());}
    Ok(value["state"]=="purged")
}
