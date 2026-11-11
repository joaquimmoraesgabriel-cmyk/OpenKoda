//! Explicit terminal-metadata maintenance. Every mutation is a distinct,
//! journaled intent; inspecting an old intent never repeats its deletion.
use super::*;

fn canonical<T:Serialize>(value:&T)->Result<String,String>{
    let value=serde_json::to_value(value).map_err(db_error)?;
    let text=value.to_string();
    if !text.is_ascii(){return Err(integrity());}
    Ok(text)
}
fn digest<T:Serialize>(value:&T)->Result<String,String>{Ok(hash(&canonical(value)?))}
fn uuid(value:&str)->Result<(),String>{
    if uuid::Uuid::parse_str(value).map(|id|id.to_string()).ok().as_deref()!=Some(value){return Err(integrity());}Ok(())
}
fn hash_valid(value:&str)->bool{crate::workflow::valid_hash(value)}
fn pair(id:&Option<String>,sha:&Option<String>)->Result<(),String>{
    if id.is_some()!=sha.is_some(){return Err(integrity());}
    if let Some(id)=id{uuid(id)?;if !hash_valid(sha.as_deref().unwrap_or("")){return Err(integrity());}}Ok(())
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct LinuxPurgeProof {
    schema:String,
    #[serde(deserialize_with="strict_json::deserialize")]
    receipt:Value,
    sha256:String,
}
impl LinuxPurgeProof {
    pub(super) fn observed(receipt:&Value,producer:&BridgeRun,intent:&KaliControlIntent)->Result<Self,String>{
        if !validate_linux_purge(receipt,producer,intent)?{return Err(integrity());}
        let text=canonical(receipt)?;if text.len()>4096{return Err(integrity());}
        Ok(Self{schema:"openkoda.kali-purge-proof.v1".into(),receipt:receipt.clone(),sha256:hash(&text)})
    }
    pub(super) fn validate(&self,producer:&BridgeRun,intent:&KaliControlIntent)->Result<(),String>{
        if *self!=Self::observed(&self.receipt,producer,intent)?{return Err(integrity());}Ok(())
    }
    pub(super) fn validate_shape(&self,intent:&KaliControlIntent)->Result<(),String>{
        let KaliControlIntent::Purge{producer_session_id,capture_id,owner_profile_fingerprint,expected_manifest_digest,purge_receipt_id,previous_receipt_id,previous_receipt_digest,..}=intent else{return Err(integrity());};
        let v=&self.receipt;let mut keys=vec!["schema","captureId","producerSessionId","environmentId","assessmentId","manifestDigest","purgeReceiptId","preservedBytes","prefixSha256","state","ownerProfileFingerprint"];
        if v["schema"]=="openkoda.kali-spool-purge.v2"{keys.extend(["previousReceiptId","previousReceiptDigest"]);}else if v["schema"]!="openkoda.kali-spool-purge.v1"{return Err(integrity());}
        let text=canonical(v)?;
        if self.schema!="openkoda.kali-purge-proof.v1"||text.len()>4096||self.sha256!=hash(&text)||!fields(v,&keys)
            ||v["state"]!="purged"||v["producerSessionId"]!=*producer_session_id||v["captureId"]!=*capture_id||v["ownerProfileFingerprint"]!=*owner_profile_fingerprint
            ||v["manifestDigest"]!=*expected_manifest_digest||v["purgeReceiptId"]!=*purge_receipt_id||v["previousReceiptId"]!=json!(previous_receipt_id)||v["previousReceiptDigest"]!=json!(previous_receipt_digest)
            ||!v["preservedBytes"].as_u64().is_some_and(|n|n<=65536)||!v["prefixSha256"].as_str().is_some_and(hash_valid){return Err(integrity());}
        for key in ["environmentId","assessmentId"]{uuid(v[key].as_str().ok_or_else(integrity)?)?;}Ok(())
    }
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub(crate) enum KaliMetadataState {Uninspected,Remaining,Retired,Unknown,ForeignProfile}
#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub(crate) enum KaliRetireState {Retired,Remaining,Unknown}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliMetadataItem {
    pub reference:KaliSpoolRef,pub purge_control_run_id:String,pub purge_receipt_id:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub purge_proof_sha256:Option<String>,
    pub state:KaliMetadataState,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliRetirePreview {
    pub item:KaliMetadataItem,pub observation:RetireObservation,pub state_digest:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub previous_receipt_id:Option<String>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub previous_receipt_digest:Option<String>,
    pub inspection_digest:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliRetireReceipt {
    pub plan_receipt_id:String,pub capture_id:String,pub run_id:String,pub control_run_id:String,
    pub manifest_digest:String,pub purge_receipt_id:String,pub purge_proof_sha256:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub commitment_digest:Option<String>,
    pub state:KaliRetireState,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct PendingRetireAttempt {
    retire_receipt_id:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]previous_receipt_id:Option<String>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]previous_receipt_digest:Option<String>,
    reviewed_state_digest:String,producer_record_sha256:String,
}
impl PendingRetireAttempt {
    fn validate(&self)->Result<(),String>{uuid(&self.retire_receipt_id)?;pair(&self.previous_receipt_id,&self.previous_receipt_digest)?;
        if self.previous_receipt_id.as_deref()==Some(&self.retire_receipt_id)||!hash_valid(&self.reviewed_state_digest)||!hash_valid(&self.producer_record_sha256){return Err(integrity());}Ok(())}
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetireInspect {
    pub producer_session_id:String,pub capture_id:String,pub owner_profile_fingerprint:String,pub producer_dispatch_deadline:u64,
    pub expected_manifest_digest:String,pub purge_receipt_id:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub expected_purge_receipt_sha256:Option<String>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pending_attempt:Option<PendingRetireAttempt>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetireApply {
    pub producer_session_id:String,pub capture_id:String,pub owner_profile_fingerprint:String,pub producer_dispatch_deadline:u64,
    pub expected_manifest_digest:String,pub purge_receipt_id:String,pub expected_purge_receipt_sha256:String,
    pub retire_receipt_id:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub previous_receipt_id:Option<String>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub previous_receipt_digest:Option<String>,
    pub expected_state_digest:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pending_attempt:Option<PendingRetireAttempt>,
}
impl RetireInspect {
    pub(super) fn validate(&self)->Result<(),String>{
        for id in [&self.producer_session_id,&self.capture_id,&self.purge_receipt_id]{uuid(id)?;}
        if !hash_valid(&self.owner_profile_fingerprint)||!hash_valid(&self.expected_manifest_digest)||self.expected_purge_receipt_sha256.as_ref().is_some_and(|s|!hash_valid(s))||self.producer_dispatch_deadline==0||self.producer_dispatch_deadline>crate::workflow::MAX_SAFE_INTEGER{return Err(integrity());}
        if let Some(pending)=&self.pending_attempt{pending.validate()?;}Ok(())
    }
}
impl RetireApply {
    fn as_inspect(&self)->RetireInspect{RetireInspect{producer_session_id:self.producer_session_id.clone(),capture_id:self.capture_id.clone(),owner_profile_fingerprint:self.owner_profile_fingerprint.clone(),producer_dispatch_deadline:self.producer_dispatch_deadline,expected_manifest_digest:self.expected_manifest_digest.clone(),purge_receipt_id:self.purge_receipt_id.clone(),expected_purge_receipt_sha256:Some(self.expected_purge_receipt_sha256.clone()),pending_attempt:self.pending_attempt.clone()}}
    pub(super) fn validate(&self)->Result<(),String>{
        self.as_inspect().validate()?;uuid(&self.retire_receipt_id)?;pair(&self.previous_receipt_id,&self.previous_receipt_digest)?;
        if !hash_valid(&self.expected_state_digest)||self.retire_receipt_id==self.purge_receipt_id||self.previous_receipt_id.as_deref()==Some(&self.retire_receipt_id)||self.pending_attempt.as_ref().is_some_and(|p|p.retire_receipt_id==self.retire_receipt_id){return Err(integrity());}Ok(())
    }
    fn pending(&self,reviewed:&RetireObservation)->PendingRetireAttempt{PendingRetireAttempt{retire_receipt_id:self.retire_receipt_id.clone(),previous_receipt_id:self.previous_receipt_id.clone(),previous_receipt_digest:self.previous_receipt_digest.clone(),reviewed_state_digest:self.expected_state_digest.clone(),producer_record_sha256:reviewed.producer_record_sha256.clone()}}
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct FileIdentity {pub device:String,pub inode:String}
impl FileIdentity{fn validate(&self)->Result<(),String>{for s in [&self.device,&self.inode]{if s.is_empty()||s.len()>20||!s.bytes().all(|b|b.is_ascii_digit())||s.len()>1&&s.starts_with('0')||s.parse::<u64>().is_err(){return Err(integrity());}}Ok(())}}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct MetadataEntry {pub name:String,pub identity:FileIdentity,pub bytes:u64,pub sha256:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct PendingWriteObservation {pub identity:FileIdentity,pub bytes:u64,pub sha256:String,pub attempt_id:String,pub complete:bool}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetireProof {
    pub capture_id:String,pub attempt_id:String,pub owner_profile_fingerprint:String,
    pub manifest_digest:String,pub purge_receipt_id:String,pub purge_receipt_sha256:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetireAttempt {
    pub retire_receipt_id:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub previous_receipt_id:Option<String>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub previous_receipt_digest:Option<String>,
    pub reviewed_state_digest:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetiredCapture {pub schema:String,pub proof:RetireProof,pub attempt:RetireAttempt}
impl RetiredCapture {
    fn validate(&self)->Result<(),String>{
        let p=&self.proof;let a=&self.attempt;
        for id in [&p.capture_id,&p.attempt_id,&p.purge_receipt_id,&a.retire_receipt_id]{uuid(id)?;}
        pair(&a.previous_receipt_id,&a.previous_receipt_digest)?;
        if self.schema!="openkoda.kali-retired-capture.v1"||canonical(self)?.len()>1024
            ||[&p.owner_profile_fingerprint,&p.manifest_digest,&p.purge_receipt_sha256,&a.reviewed_state_digest].iter().any(|h|!hash_valid(h))
            ||a.retire_receipt_id==p.purge_receipt_id||a.previous_receipt_id.as_deref()==Some(&a.retire_receipt_id){return Err(integrity());}Ok(())
    }
}
#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub(crate) enum RetireObservationState {Ready,MetadataRemaining,Retired}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetireObservation {
    pub schema:String,pub environment_id:String,pub assessment_id:String,pub producer_session_id:String,
    pub capture_id:String,pub attempt_id:String,pub owner_profile_fingerprint:String,
    pub manifest_digest:String,pub purge_receipt_id:String,pub purge_receipt_sha256:String,pub producer_record_sha256:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub directory:Option<FileIdentity>,
    pub entries:Vec<MetadataEntry>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub pending_producer_write:Option<PendingWriteObservation>,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub commitment:Option<RetiredCapture>,
    pub state:RetireObservationState,pub metadata_bytes:u64,pub slot_present:bool,
}
impl RetireObservation {
    pub(crate) fn validate(&self)->Result<(),String>{
        for id in [&self.environment_id,&self.assessment_id,&self.producer_session_id,&self.capture_id,&self.attempt_id,&self.purge_receipt_id]{uuid(id)?;}
        if self.schema!="openkoda.kali-retire-observation.v1"||[&self.owner_profile_fingerprint,&self.manifest_digest,&self.purge_receipt_sha256,&self.producer_record_sha256].iter().any(|h|!hash_valid(h))||self.entries.len()>4||self.directory.is_some()!=self.slot_present{return Err(integrity());}
        if let Some(dir)=&self.directory{dir.validate()?;}
        let mut previous="";let mut bytes=0_u64;
        for entry in &self.entries{
            entry.identity.validate()?;
            if !["manifest.json","manifest.json.next","purge.json","purge.json.next"].contains(&entry.name.as_str())||entry.name.as_str()<=previous||entry.bytes>4096||!hash_valid(&entry.sha256){return Err(integrity());}
            previous=&entry.name;bytes+=entry.bytes;
        }
        if let Some(pending)=&self.pending_producer_write{pending.identity.validate()?;uuid(&pending.attempt_id)?;if pending.bytes>4096||!hash_valid(&pending.sha256){return Err(integrity());}bytes+=pending.bytes;}
        if bytes!=self.metadata_bytes||!self.slot_present&&!self.entries.is_empty(){return Err(integrity());}
        if let Some(commitment)=&self.commitment{
            commitment.validate()?;let p=&commitment.proof;
            if p.capture_id!=self.capture_id||p.attempt_id!=self.attempt_id||p.owner_profile_fingerprint!=self.owner_profile_fingerprint||p.manifest_digest!=self.manifest_digest||p.purge_receipt_id!=self.purge_receipt_id||p.purge_receipt_sha256!=self.purge_receipt_sha256{return Err(integrity());}
        }
        match self.state {
            RetireObservationState::Ready if self.commitment.is_none()&&self.slot_present=>{},
            RetireObservationState::MetadataRemaining if self.commitment.is_some()&&self.slot_present=>{},
            RetireObservationState::Retired if self.commitment.is_some()&&!self.slot_present&&self.entries.is_empty()&&self.pending_producer_write.is_none()&&self.metadata_bytes==0=>{},
            _=>return Err(integrity()),
        }Ok(())
    }
    fn matches(&self,producer:&BridgeRun,inspect:&RetireInspect)->Result<(),String>{
        self.validate()?;inspect.validate()?;let binding=producer_binding(producer)?;
        if self.environment_id!=producer.environment_id||self.assessment_id!=producer.assessment_id||self.producer_session_id!=inspect.producer_session_id||self.capture_id!=inspect.capture_id
            ||self.attempt_id!=binding.attempt_id||self.owner_profile_fingerprint!=inspect.owner_profile_fingerprint||self.manifest_digest!=inspect.expected_manifest_digest||self.purge_receipt_id!=inspect.purge_receipt_id
            ||inspect.expected_purge_receipt_sha256.as_ref().is_some_and(|sha|*sha!=self.purge_receipt_sha256){return Err(integrity());}
        if let Some(pending)=&self.pending_producer_write{if inspect.pending_attempt.as_ref().is_none_or(|p|p.retire_receipt_id!=pending.attempt_id){return Err(integrity());}}
        Ok(())
    }
}
impl KaliMetadataItem {
    pub(crate) fn validate(&self)->Result<(),String>{
        let r=&self.reference;
        for id in [&r.capture_id,&r.assessment_id,&r.run_id,&r.environment_id,&self.purge_control_run_id,&self.purge_receipt_id]{uuid(id)?;}
        if !hash_valid(&r.owner_profile_fingerprint)||r.manifest_digest.as_ref().is_none_or(|h|!hash_valid(h))||r.preserved_bytes.is_none_or(|n|n>65536)
            ||self.purge_proof_sha256.as_ref().is_some_and(|h|!hash_valid(h))||!matches!(r.availability.as_str(),"purged"|"foreign_profile"){return Err(integrity());}Ok(())
    }
}
impl KaliRetirePreview {
    fn digest(&self)->Result<String,String>{digest(&json!({"schema":"openkoda.kali-retire-preview.v1","item":self.item,"observation":self.observation,"stateDigest":self.state_digest,"previousReceiptId":self.previous_receipt_id,"previousReceiptDigest":self.previous_receipt_digest}))}
    pub(crate) fn validate(&self)->Result<(),String>{
        self.item.validate()?;self.observation.validate()?;pair(&self.previous_receipt_id,&self.previous_receipt_digest)?;
        let r=&self.item.reference;let o=&self.observation;
        if !matches!(self.item.state,KaliMetadataState::Uninspected|KaliMetadataState::Remaining)||r.availability!="purged"
            ||o.state==RetireObservationState::Retired||o.capture_id!=r.capture_id||o.assessment_id!=r.assessment_id||o.producer_session_id!=r.run_id||o.environment_id!=r.environment_id||o.owner_profile_fingerprint!=r.owner_profile_fingerprint
            ||Some(&o.manifest_digest)!=r.manifest_digest.as_ref()||o.purge_receipt_id!=self.item.purge_receipt_id||Some(&o.purge_receipt_sha256)!=self.item.purge_proof_sha256.as_ref()
            ||digest(o)?!=self.state_digest||self.digest()?!=self.inspection_digest{return Err(integrity());}
        let predecessor=o.commitment.as_ref().map(|c|Ok::<_,String>((c.attempt.retire_receipt_id.clone(),digest(c)?))).transpose()?;
        if predecessor.as_ref().map(|p|&p.0)!=self.previous_receipt_id.as_ref()||predecessor.as_ref().map(|p|&p.1)!=self.previous_receipt_digest.as_ref(){return Err(integrity());}Ok(())
    }
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct RetireRecord {
    pub receipt:KaliRetireReceipt,
    #[serde(deserialize_with="crate::workflow::required_nullable")]pub observation:Option<RetireObservation>,
}
impl RetireRecord {
    fn unknown(run:&BridgeRun,apply:&RetireApply)->Self{Self{receipt:KaliRetireReceipt{plan_receipt_id:apply.retire_receipt_id.clone(),capture_id:apply.capture_id.clone(),run_id:apply.producer_session_id.clone(),control_run_id:run.id.clone(),manifest_digest:apply.expected_manifest_digest.clone(),purge_receipt_id:apply.purge_receipt_id.clone(),purge_proof_sha256:apply.expected_purge_receipt_sha256.clone(),commitment_digest:None,state:KaliRetireState::Unknown},observation:None}}
    pub(super) fn validate(&self,run:&BridgeRun,apply:&RetireApply)->Result<(),String>{
        let mut expected=Self::unknown(run,apply);expected.receipt.state=self.receipt.state;expected.receipt.commitment_digest=self.receipt.commitment_digest.clone();
        if expected.receipt!=self.receipt||self.receipt.state!=KaliRetireState::Unknown&&!run.cleanup_confirmed{return Err(integrity());}
        if let Some(o)=&self.observation{
            o.validate()?;
            if o.capture_id!=apply.capture_id||o.producer_session_id!=apply.producer_session_id||o.environment_id!=run.environment_id||o.assessment_id!=run.assessment_id||o.owner_profile_fingerprint!=apply.owner_profile_fingerprint||o.manifest_digest!=apply.expected_manifest_digest||o.purge_receipt_id!=apply.purge_receipt_id||o.purge_receipt_sha256!=apply.expected_purge_receipt_sha256{return Err(integrity());}
            if o.commitment.as_ref().map(digest).transpose()?!=self.receipt.commitment_digest{return Err(integrity());}
            if self.receipt.state==KaliRetireState::Retired&&(o.state!=RetireObservationState::Retired||o.commitment.as_ref().is_none_or(|c|c.attempt!=apply.attempt())){return Err(integrity());}
            if self.receipt.state==KaliRetireState::Remaining{
                if o.state==RetireObservationState::Retired{return Err(integrity());}
                if let Some(c)=&o.commitment{if c.attempt!=apply.attempt()&&(Some(&c.attempt.retire_receipt_id)!=apply.previous_receipt_id.as_ref()||Some(digest(c)?)!=apply.previous_receipt_digest){return Err(integrity());}}
                else if apply.previous_receipt_id.is_some(){return Err(integrity());}
            }
        }else if self.receipt.state!=KaliRetireState::Unknown||self.receipt.commitment_digest.is_some(){return Err(integrity());}Ok(())
    }
}
impl RetireApply {fn attempt(&self)->RetireAttempt{RetireAttempt{retire_receipt_id:self.retire_receipt_id.clone(),previous_receipt_id:self.previous_receipt_id.clone(),previous_receipt_digest:self.previous_receipt_digest.clone(),reviewed_state_digest:self.expected_state_digest.clone()}}}

pub(super) fn validate_control(run:&BridgeRun,intent:&KaliControlIntent,reviewed:Option<&RetireObservation>,receipt:Option<&RetireRecord>)->Result<(),String>{
    intent.validate()?;
    if run.tool!="capture_retire"||run.reconciliation_of.as_deref()!=Some(intent.producer()){return Err(integrity());}
    match intent {
        KaliControlIntent::RetireInspect{..} if reviewed.is_none()&&receipt.is_none()=>Ok(()),
        KaliControlIntent::RetireApply{request}=>{
            let reviewed=reviewed.ok_or_else(integrity)?;reviewed.validate()?;
            if digest(reviewed)?!=request.expected_state_digest||reviewed.capture_id!=request.capture_id||reviewed.purge_receipt_sha256!=request.expected_purge_receipt_sha256||reviewed.state==RetireObservationState::Retired{return Err(integrity());}
            let predecessor=reviewed.commitment.as_ref().map(|c|Ok::<_,String>((c.attempt.retire_receipt_id.clone(),digest(c)?))).transpose()?;
            if predecessor.as_ref().map(|p|&p.0)!=request.previous_receipt_id.as_ref()||predecessor.as_ref().map(|p|&p.1)!=request.previous_receipt_digest.as_ref(){return Err(integrity());}
            if let Some(receipt)=receipt{
                receipt.validate(run,request)?;
                if receipt.observation.as_ref().is_some_and(|o|o.attempt_id!=reviewed.attempt_id){return Err(integrity());}
            }Ok(())
        },_=>Err(integrity()),
    }
}
pub(in crate::kali_bridge) fn validate_family(run:&BridgeRun,producer:&BridgeRun,runs:&[BridgeRun])->Result<(),String>{
    if let Some(KaliRunContext::Control{intent,receipt:Some(record)})=&run.work_context{
        if let Some(proof)=&record.linux_purge_proof{proof.validate(producer,intent)?;}
    }
    let Some(KaliRunContext::RetireControl{intent,reviewed,..})=&run.work_context else{return Ok(());};
    let request=match intent{KaliControlIntent::RetireInspect{request}=>request.clone(),KaliControlIntent::RetireApply{request}=>request.as_inspect(),_=>return Err(integrity())};
    let Some(KaliRunContext::Producer{spool_manifest:Some(manifest),..})=&producer.work_context else{return Err(integrity());};
    if request.expected_manifest_digest!=manifest.digest{return Err(integrity());}
    let (_,purge_intent,purge)=purge_source(runs,&producer.id,&request.capture_id,&request.purge_receipt_id)?;
    if let Some(expected)=&request.expected_purge_receipt_sha256{
        let proof=purge.linux_purge_proof.as_ref().ok_or_else(integrity)?;proof.validate(producer,purge_intent)?;
        if *expected!=proof.sha256{return Err(integrity());}
    }
    if let Some(reviewed)=reviewed{
        reviewed.matches(producer,&request)?;
        if let Some(commitment)=&reviewed.commitment{
            if !runs.iter().any(|old|old.id!=run.id&&old.reconciliation_of.as_deref()==Some(&producer.id)&&matches!(&old.work_context,Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{request},..}) if request.attempt()==commitment.attempt)){return Err(integrity());}
        }
    }
    if let Some(pending)=&request.pending_attempt{
        let old=runs.iter().find_map(|r|match &r.work_context{Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{request:old},reviewed:Some(base),..}) if old.retire_receipt_id==pending.retire_receipt_id&&r.id!=run.id=>Some((old,base)),_=>None}).ok_or_else(integrity)?;
        if old.0.producer_session_id!=producer.id||old.0.pending(old.1)!=*pending{return Err(integrity());}
    }Ok(())
}
fn purge_source<'a>(runs:&'a[BridgeRun],producer:&str,capture:&str,id:&str)->Result<(&'a BridgeRun,&'a KaliControlIntent,&'a PurgeRecord),String>{
    runs.iter().find_map(|r|match &r.work_context{Some(KaliRunContext::Control{intent,receipt:Some(p)}) if r.reconciliation_of.as_deref()==Some(producer)&&r.cleanup_confirmed&&p.state=="purged"&&p.capture_id==capture&&p.plan_receipt_id==id=>Some((r,intent,p)),_=>None}).ok_or_else(integrity)
}
fn latest_apply<'a>(runs:&'a[BridgeRun],producer:&str)->Option<(&'a BridgeRun,&'a RetireApply,&'a RetireObservation,Option<&'a RetireRecord>)>{
    runs.iter().find_map(|r|match &r.work_context{Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{request},reviewed:Some(base),receipt}) if r.reconciliation_of.as_deref()==Some(producer)=>Some((r,request,base,receipt.as_ref())),_=>None})
}
pub(super) fn history_retired(producer:&BridgeRun,runs:&[BridgeRun])->Result<bool,String>{
    let Some((control,request,_,Some(record)))=latest_apply(runs,&producer.id) else{return Ok(false);};
    validate_context(control)?;validate_family(control,producer,runs)?;
    Ok(control.cleanup_confirmed&&record.receipt.state==KaliRetireState::Retired&&record.observation.as_ref().is_some_and(|o|o.commitment.as_ref().is_some_and(|c|c.attempt==request.attempt())))
}
fn accept_commitment(observation:&RetireObservation,known:Option<&RetireApply>)->Result<(),String>{
    if let Some(commitment)=&observation.commitment{
        let known=known.ok_or("capture_retire_history_unavailable")?;
        if commitment.attempt!=known.attempt()&&(Some(&commitment.attempt.retire_receipt_id)!=known.previous_receipt_id.as_ref()||Some(digest(commitment)?)!=known.previous_receipt_digest){return Err("capture_retire_history_unavailable".into());}
    }Ok(())
}
impl KaliBridge {
    pub(crate) fn require_metadata_capacity(&self,additional:usize)->Result<(),String>{if self.runs()?.len().checked_add(additional).is_none_or(|n|n>10000){return Err("work_journal_quota".into());}Ok(())}
    pub(crate) fn terminal_metadata(&self,store:&Store)->Result<Vec<KaliMetadataItem>,String>{
        let runs=self.runs()?;let owner=profile_fingerprint(store)?;let mut out=vec![];
        for reference in self.spool_metadata(store,None)?{
            let Some((control,purge))=runs.iter().find_map(|r|match &r.work_context{Some(KaliRunContext::Control{receipt:Some(p),..}) if p.capture_id==reference.capture_id&&p.run_id==reference.run_id&&p.state=="purged"&&r.cleanup_confirmed=>Some((r,p)),_=>None}) else{continue;};
            let state=if reference.owner_profile_fingerprint!=owner{KaliMetadataState::ForeignProfile}else if runs.iter().any(|r|(r.id==reference.run_id||r.reconciliation_of.as_deref()==Some(&reference.run_id))&&work::unresolved(r)){KaliMetadataState::Unknown}else if let Some((_,_,_,receipt))=latest_apply(&runs,&reference.run_id){match receipt.map(|r|r.receipt.state){Some(KaliRetireState::Retired)=>KaliMetadataState::Retired,Some(KaliRetireState::Remaining)=>KaliMetadataState::Remaining,_=>KaliMetadataState::Unknown}}else{KaliMetadataState::Uninspected};
            let item=KaliMetadataItem{reference,purge_control_run_id:control.id.clone(),purge_receipt_id:purge.plan_receipt_id.clone(),purge_proof_sha256:purge.linux_purge_proof.as_ref().map(|p|p.sha256.clone()),state};item.validate()?;out.push(item);
        }Ok(out)
    }
    fn metadata_item(&self,store:&Store,capture_id:&str)->Result<KaliMetadataItem,String>{uuid(capture_id)?;self.terminal_metadata(store)?.into_iter().find(|item|item.reference.capture_id==capture_id).ok_or("capture_metadata_missing".into())}
    fn retire_read(&self,store:&Store,item:&KaliMetadataItem,pending:Option<PendingRetireAttempt>,identity:&dyn Fn()->Result<String,String>)->Result<RetireObservation,String>{
        let producer=self.find(&item.reference.run_id)?;require_owned(store,producer_binding(&producer)?)?;
        let request=RetireInspect{producer_session_id:producer.id.clone(),capture_id:item.reference.capture_id.clone(),owner_profile_fingerprint:item.reference.owner_profile_fingerprint.clone(),producer_dispatch_deadline:producer.dispatch_before_unix_seconds,expected_manifest_digest:item.reference.manifest_digest.clone().ok_or_else(integrity)?,purge_receipt_id:item.purge_receipt_id.clone(),expected_purge_receipt_sha256:item.purge_proof_sha256.clone(),pending_attempt:pending};
        let (observed,_)=self.retire_control(store,&producer,KaliControlIntent::RetireInspect{request:request.clone()},None,identity)?;
        let (observation,proof)=retire_answer(&observed,&producer,&request,"inspect")?;
        // The old summary is never authority: persist the original canonical Linux receipt first.
        let runs=self.runs()?;let (source,intent,record)=purge_source(&runs,&producer.id,&request.capture_id,&request.purge_receipt_id)?;
        let verified=match (&record.linux_purge_proof,proof){(Some(old),Some(new)) if *old==new=>old.clone(),(Some(old),None)=>old.clone(),(None,Some(new))=>new,_=>return Err(integrity())};
        verified.validate(&producer,intent)?;if verified.sha256!=observation.purge_receipt_sha256{return Err(integrity());}
        if record.linux_purge_proof.is_none(){let mut updated=source.clone();if let Some(KaliRunContext::Control{receipt:Some(record),..})=&mut updated.work_context{record.linux_purge_proof=Some(verified);}self.save(&updated)?;}
        Ok(observation)
    }
    pub(crate) fn inspect_capture_retire(&self,store:&Store,capture_id:&str,identity:&dyn Fn()->Result<String,String>)->Result<KaliRetirePreview,String>{
        self.require_metadata_capacity(1)?;let item=self.metadata_item(store,capture_id)?;
        if !matches!(item.state,KaliMetadataState::Uninspected|KaliMetadataState::Remaining){return Err("capture_retire_reconciliation_required".into());}
        self.require_capture_terminal(store,&item.reference)?;
        let runs=self.runs()?;let previous=latest_apply(&runs,&item.reference.run_id);
        let observation=self.retire_read(store,&item,previous.map(|(_,a,b,_)|a.pending(b)),identity)?;
        accept_commitment(&observation,previous.map(|(_,a,_,_)|a))?;
        if observation.state==RetireObservationState::Retired{return Err("capture_retire_reconciliation_required".into());}
        let predecessor=observation.commitment.as_ref().map(|c|Ok::<_,String>((c.attempt.retire_receipt_id.clone(),digest(c)?))).transpose()?;
        let mut preview=KaliRetirePreview{item:self.metadata_item(store,capture_id)?,state_digest:digest(&observation)?,observation,previous_receipt_id:predecessor.as_ref().map(|p|p.0.clone()),previous_receipt_digest:predecessor.map(|p|p.1),inspection_digest:String::new()};preview.inspection_digest=preview.digest()?;preview.validate()?;Ok(preview)
    }
    pub(crate) fn execute_capture_retire(&self,store:&Store,preview:&KaliRetirePreview,retire_receipt_id:&str,expected_inspection_digest:&str,identity:&dyn Fn()->Result<String,String>)->Result<KaliRetireReceipt,String>{
        uuid(retire_receipt_id)?;preview.validate()?;self.require_metadata_capacity(2)?;
        if preview.inspection_digest!=expected_inspection_digest{return Err("capture_inspection_changed".into());}
        let runs=self.runs()?;if runs.iter().any(|r|matches!(&r.work_context,Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{request},..}) if request.retire_receipt_id==retire_receipt_id)){return Err("capture_retire_already_dispatched".into());}
        if self.inspect_capture_retire(store,&preview.item.reference.capture_id,identity)?!=*preview{return Err("capture_inspection_changed".into());}
        let producer=self.find(&preview.item.reference.run_id)?;let previous=latest_apply(&runs,&producer.id);
        let request=RetireApply{producer_session_id:producer.id.clone(),capture_id:preview.item.reference.capture_id.clone(),owner_profile_fingerprint:preview.item.reference.owner_profile_fingerprint.clone(),producer_dispatch_deadline:producer.dispatch_before_unix_seconds,expected_manifest_digest:preview.observation.manifest_digest.clone(),purge_receipt_id:preview.item.purge_receipt_id.clone(),expected_purge_receipt_sha256:preview.observation.purge_receipt_sha256.clone(),retire_receipt_id:retire_receipt_id.into(),previous_receipt_id:preview.previous_receipt_id.clone(),previous_receipt_digest:preview.previous_receipt_digest.clone(),expected_state_digest:preview.state_digest.clone(),pending_attempt:previous.map(|(_,a,b,_)|a.pending(b))};
        let (_,run)=self.retire_control(store,&producer,KaliControlIntent::RetireApply{request},Some(preview.observation.clone()),identity)?;
        let Some(KaliRunContext::RetireControl{receipt:Some(record),..})=run.work_context else{return Err(integrity());};Ok(record.receipt)
    }
    pub(crate) fn reconcile_capture_retire(&self,store:&Store,id:&str,identity:&dyn Fn()->Result<String,String>)->Result<KaliRetireReceipt,String>{
        uuid(id)?;let runs=self.runs()?;
        let control=runs.iter().find(|r|matches!(&r.work_context,Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{request},..}) if request.retire_receipt_id==id)).ok_or("capture_retire_missing")?;
        let Some(KaliRunContext::RetireControl{intent:KaliControlIntent::RetireApply{request},..})=&control.work_context else{return Err(integrity());};
        let producer=runs.iter().find(|run|run.id==request.producer_session_id).ok_or_else(integrity)?;
        require_owned(store,producer_binding(producer)?)?;
        if latest_apply(&runs,&producer.id).is_none_or(|(r,_,_,_)|r.id!=control.id){return Err("capture_retire_superseded".into());}
        for pending in runs.iter().filter(|r|r.reconciliation_of.as_deref()==Some(&producer.id)&&work::unresolved(r)){self.handle_with_identity("kali.mcp.reconcile",json!({"assessmentId":pending.assessment_id,"runId":pending.id}),store,identity,None)?;}
        self.handle_with_identity("kali.mcp.reconcile",json!({"assessmentId":producer.assessment_id,"runId":producer.id}),store,identity,None)?;
        // Re-read the validated family after closing any outstanding lease. A
        // terminal receipt survives loss of the retention journal acknowledgement,
        // including when its apply consumed the last bridge record.
        let stopped=self.runs()?;
        if stopped.iter().any(|r|(r.id==producer.id||r.reconciliation_of.as_deref()==Some(&producer.id))&&work::unresolved(r)){
            return Err("work_reconciliation_required".into());
        }
        let (current,current_request,base,record)=latest_apply(&stopped,&producer.id).ok_or_else(integrity)?;
        if current.id!=control.id||current_request!=request{return Err("capture_retire_superseded".into());}
        if let Some(terminal)=record.filter(|r|r.receipt.state==KaliRetireState::Retired){
            return Ok(terminal.receipt.clone());
        }
        self.require_metadata_capacity(1)?;
        let item=self.metadata_item(store,&request.capture_id)?;
        let observation=self.retire_read(store,&item,Some(request.pending(base)),identity)?;
        // A pre-commit failure can leave the reviewed predecessor or no commitment.
        if let Some(c)=&observation.commitment{if c.attempt!=request.attempt()&&(Some(&c.attempt.retire_receipt_id)!=request.previous_receipt_id.as_ref()||Some(digest(c)?)!=request.previous_receipt_digest){return Err("capture_retire_history_unavailable".into());}}
        let mut control=self.find(&control.id)?;let record=retire_record(&control,request,observation)?;
        if let Some(KaliRunContext::RetireControl{receipt,..})=&mut control.work_context{*receipt=Some(record.clone());}self.save(&control)?;Ok(record.receipt)
    }
    #[cfg(test)]
    pub(in crate::kali_bridge) fn inspect_retired_for_test(&self,store:&Store,receipt:&KaliRetireReceipt,identity:&dyn Fn()->Result<String,String>)->Result<RetireObservation,String>{
        let item=self.metadata_item(store,&receipt.capture_id)?;
        if item.state!=KaliMetadataState::Retired{return Err(integrity());}
        let observed=self.retire_read(store,&item,None,identity)?;
        let commitment=observed.commitment.as_ref().ok_or_else(integrity)?;
        if observed.state!=RetireObservationState::Retired||commitment.attempt.retire_receipt_id!=receipt.plan_receipt_id
            ||Some(digest(commitment)?)!=receipt.commitment_digest{return Err(integrity());}
        Ok(observed)
    }
    fn retire_control(&self,store:&Store,producer:&BridgeRun,intent:KaliControlIntent,reviewed:Option<RetireObservation>,identity:&dyn Fn()->Result<String,String>)->Result<(Value,BridgeRun),String>{
        use crate::work_supervisor::{EffectOwner,EffectOwnerKind,EffectTermination};
        let mut permit=self.arbiter.claim_direct(store,EffectOwner{id:producer.id.clone(),assessment_id:Some(producer.assessment_id.clone()),kind:EffectOwnerKind::KaliDiagnosticIntent})?;
        let mut bound=false;let mut final_run:Option<BridgeRun>=None;
        let result=(||{
            if identity()?!=producer.environment_id{return Err("environment_identity_changed".into());}
            let mut active=self.active.lock().map_err(db_error)?;if active.is_some(){return Err("work_busy".into());}
            let mut run=self.new_control(producer,intent.clone())?;
            if let Some(KaliRunContext::RetireControl{reviewed:stored,..})=&mut run.work_context{*stored=reviewed;}
            validate_context(&run)?;validate_family(&run,producer,&self.runs()?)?;self.save(&run)?;final_run=Some(run.clone());
            self.arbiter.bind_effect(&mut permit,crate::workflow::EffectRef::KaliRun{id:producer.id.clone()})?;bound=true;
            let cancel=Arc::new(AtomicBool::new(false));*active=Some(Active{run_id:producer.id.clone(),assessment_id:producer.assessment_id.clone(),cancel:cancel.clone()});drop(active);
            run.launch_attempted=true;self.save(&run)?;final_run=Some(run.clone());
            let mut transport=self.launcher.launch(&run,&cancel)?;let deadline=Instant::now()+Duration::from_secs(45);
            let observed=(||{negotiate(&mut *transport,&run,deadline,&cancel)?;transport.send(request(&run,3,"tools/call",json!({"name":"capture_retire","arguments":intent.control()})),deadline,&cancel)?;control_answer(transport.receive(deadline,&cancel)?,&run,3,"capture_retire")})();
            let finish=transport.finish(&run,Instant::now()+Duration::from_secs(5));
            run.cleanup_confirmed=finish.cleanup_confirmed;run.finished_at=if finish.cleanup_confirmed{Some(model::now())}else{None};run.status=if !finish.cleanup_confirmed{"unknown"}else if observed.is_ok(){"completed"}else{"failed"}.into();
            if let KaliControlIntent::RetireApply{request}=&intent{
                let mut record=RetireRecord::unknown(&run,request);
                if finish.cleanup_confirmed{if let Ok(value)=&observed{if let Ok((observation,_))=retire_answer(value,producer,&request.as_inspect(),"apply"){if observation.commitment.as_ref().is_some_and(|c|c.attempt==request.attempt()){record=retire_record(&run,request,observation)?;}}}}
                if let Some(KaliRunContext::RetireControl{receipt,..})=&mut run.work_context{*receipt=Some(record);}
            }
            self.save(&run)?;final_run=Some(run.clone());if !finish.cleanup_confirmed{return Err("work_unknown_effect".into());}Ok((observed?,run))
        })();
        *self.active.lock().map_err(db_error)?=None;
        if result.is_err(){if let Some(run)=final_run.as_mut(){if !run.cleanup_confirmed{run.status=if run.launch_attempted{"unknown"}else{"failed"}.into();run.cleanup_confirmed=!run.launch_attempted;run.finished_at=if run.cleanup_confirmed{Some(model::now())}else{None};self.save(run)?;}}}
        let termination=if !bound{EffectTermination::NotStarted}else if final_run.as_ref().is_some_and(|r|r.cleanup_confirmed){EffectTermination::Stopped}else{EffectTermination::Unknown};self.arbiter.finish(permit,termination)?;result
    }
}
fn retire_answer(value:&Value,producer:&BridgeRun,request:&RetireInspect,kind:&str)->Result<(RetireObservation,Option<LinuxPurgeProof>),String>{
    if !fields(value,&["environmentId","sessionId","assessmentId","operation","kind","producerQuiescent","observation","stateDigest","linuxPurgeProof"])||value["kind"]!=kind||value["producerQuiescent"]!=true||canonical(value)?.len()>32768{return Err(integrity());}
    let observation:RetireObservation=serde_json::from_value(value["observation"].clone()).map_err(db_error)?;observation.matches(producer,request)?;
    if value["stateDigest"]!=digest(&observation)?{return Err(integrity());}
    let proof=serde_json::from_value(value["linuxPurgeProof"].clone()).map_err(db_error)?;Ok((observation,proof))
}
fn retire_record(run:&BridgeRun,request:&RetireApply,observation:RetireObservation)->Result<RetireRecord,String>{
    let mut record=RetireRecord::unknown(run,request);
    record.receipt.commitment_digest=observation.commitment.as_ref().map(digest).transpose()?;
    record.receipt.state=if run.cleanup_confirmed{if observation.state==RetireObservationState::Retired{KaliRetireState::Retired}else{KaliRetireState::Remaining}}else{KaliRetireState::Unknown};
    record.observation=Some(observation);record.validate(run,request)?;Ok(record)
}

/// JSON objects reaching a new proof must not silently lose duplicate fields.
pub(in crate::kali_bridge) mod strict_json {
    use serde::de::{self,Deserialize,Deserializer,MapAccess,SeqAccess,Visitor};
    use serde_json::{Map,Value};
    struct Strict(Value);
    impl<'de> Deserialize<'de> for Strict {
        fn deserialize<D:Deserializer<'de>>(d:D)->Result<Self,D::Error>{d.deserialize_any(V).map(Strict)}
    }
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value=Value;
        fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("JSON without duplicate fields")}
        fn visit_bool<E:de::Error>(self,v:bool)->Result<Value,E>{Ok(Value::Bool(v))}
        fn visit_i64<E:de::Error>(self,v:i64)->Result<Value,E>{Ok(v.into())}
        fn visit_u64<E:de::Error>(self,v:u64)->Result<Value,E>{Ok(v.into())}
        fn visit_f64<E:de::Error>(self,v:f64)->Result<Value,E>{serde_json::Number::from_f64(v).map(Value::Number).ok_or_else(||E::custom("nonfinite JSON"))}
        fn visit_str<E:de::Error>(self,v:&str)->Result<Value,E>{Ok(v.into())}
        fn visit_string<E:de::Error>(self,v:String)->Result<Value,E>{Ok(v.into())}
        fn visit_none<E:de::Error>(self)->Result<Value,E>{Ok(Value::Null)}
        fn visit_unit<E:de::Error>(self)->Result<Value,E>{Ok(Value::Null)}
        fn visit_seq<A:SeqAccess<'de>>(self,mut a:A)->Result<Value,A::Error>{let mut values=vec![];while let Some(Strict(v))=a.next_element()?{values.push(v);}Ok(Value::Array(values))}
        fn visit_map<A:MapAccess<'de>>(self,mut a:A)->Result<Value,A::Error>{let mut values=Map::new();while let Some((k,Strict(v)))=a.next_entry::<String,Strict>()?{if values.insert(k,v).is_some(){return Err(de::Error::custom("duplicate JSON field"));}}Ok(Value::Object(values))}
    }
    pub(super) fn deserialize<'de,D:Deserializer<'de>>(d:D)->Result<Value,D::Error>{Strict::deserialize(d).map(|v|v.0)}
    pub(in crate::kali_bridge) fn parse(bytes:&[u8])->Result<Value,serde_json::Error>{serde_json::from_slice::<Strict>(bytes).map(|v|v.0)}
}
