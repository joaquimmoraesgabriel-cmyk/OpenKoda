//! Retention metadata is not a Linux path or authority to dispatch a control.
//! The backend reconstructs ownership and re-inspects the exact spool on apply.
use serde::{Deserialize, Serialize};
use crate::kali_bridge::{KaliMetadataItem,KaliRetirePreview,KaliRetireReceipt,KaliRetireState};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct SpoolItem {
    pub capture_id:String,
    pub assessment_id:String,
    pub run_id:String,
    pub environment_id:String,
    pub owner_profile_fingerprint:String,
    pub manifest_digest:Option<String>,
    pub preserved_bytes:Option<u64>,
    pub availability:String,
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct SpoolPreview {
    pub reference:SpoolItem,pub inspection_digest:String,
    #[serde(default,skip_serializing_if="Option::is_none")]pub previous_receipt_id:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]pub previous_receipt_digest:Option<String>,
}

pub(crate) enum SpoolOutcome {Purged,Present,Unknown}
pub(crate) enum MetadataOutcome {Retired,Remaining,Unknown}

pub(crate) trait SpoolBackend {
    /// Reads host metadata only; inventory never starts Linux.
    fn inventory(&self)->Result<Vec<SpoolItem>,String>;
    fn inspect(&self,capture_id:&str)->Result<SpoolPreview,String>;
    fn purge(&self,preview:&SpoolPreview,receipt_id:&str)->Result<SpoolOutcome,String>;
    /// Observes the previously recorded intent. Never retries deletion.
    fn reconcile(&self,preview:&SpoolPreview,receipt_id:&str)->Result<SpoolOutcome,String>;
    fn metadata_inventory(&self)->Result<Vec<KaliMetadataItem>,String>{Ok(vec![])}
    fn metadata_capacity(&self,_:usize)->Result<(),String>{Err("capture_metadata_unsupported".into())}
    fn inspect_metadata(&self,_:&str)->Result<KaliRetirePreview,String>{Err("capture_metadata_unsupported".into())}
    fn retire_metadata(&self,_:&KaliRetirePreview,_:&str)->Result<MetadataOutcome,String>{Err("capture_metadata_unsupported".into())}
    fn reconcile_metadata(&self,_:&KaliRetirePreview,_:&str)->Result<MetadataOutcome,String>{Err("capture_metadata_unsupported".into())}
}

pub(crate) struct KaliRetention<'a>{
    pub bridge:&'a crate::kali_bridge::KaliBridge,
    pub store:&'a crate::core::Store,
    pub identity:&'a dyn Fn()->Result<String,String>,
}
fn item(reference:&crate::kali_bridge::KaliSpoolRef)->SpoolItem{
    SpoolItem{capture_id:reference.capture_id.clone(),assessment_id:reference.assessment_id.clone(),run_id:reference.run_id.clone(),environment_id:reference.environment_id.clone(),owner_profile_fingerprint:reference.owner_profile_fingerprint.clone(),manifest_digest:reference.manifest_digest.clone(),preserved_bytes:reference.preserved_bytes,availability:reference.availability.clone()}
}
impl KaliRetention<'_>{
    fn metadata_outcome(&self,preview:&KaliRetirePreview,id:&str,receipt:KaliRetireReceipt)->Result<MetadataOutcome,String>{
        preview.validate()?;let expected=&preview.item;
        if receipt.plan_receipt_id!=id||receipt.capture_id!=expected.reference.capture_id||receipt.run_id!=expected.reference.run_id||Some(&receipt.manifest_digest)!=expected.reference.manifest_digest.as_ref()||receipt.purge_receipt_id!=expected.purge_receipt_id||Some(&receipt.purge_proof_sha256)!=expected.purge_proof_sha256.as_ref()||!super::valid_uuid(&receipt.control_run_id)||receipt.commitment_digest.as_ref().is_some_and(|s|!crate::workflow::valid_hash(s)){return Err("capture_retire_receipt_mismatch".into());}
        Ok(match receipt.state{KaliRetireState::Retired if receipt.commitment_digest.is_some()=>MetadataOutcome::Retired,KaliRetireState::Retired=>return Err("capture_retire_receipt_mismatch".into()),KaliRetireState::Remaining=>MetadataOutcome::Remaining,KaliRetireState::Unknown=>MetadataOutcome::Unknown})
    }
    fn reference(&self,id:&str)->Result<crate::kali_bridge::KaliSpoolRef,String>{
        if !super::valid_uuid(id){return Err("Identidade da captura Linux inválida.".into());}
        self.bridge.spool_metadata(self.store,None)?.into_iter().find(|r|r.capture_id==id).ok_or("Captura Linux ausente do perfil atual.".into())
    }
    fn outcome(&self,preview:&SpoolPreview,id:&str,receipt:crate::kali_bridge::KaliPurgeReceipt)->Result<SpoolOutcome,String>{
        let expected=&preview.reference;
        if receipt.plan_receipt_id!=id||receipt.capture_id!=expected.capture_id||receipt.run_id!=expected.run_id
            ||Some(&receipt.manifest_digest)!=expected.manifest_digest.as_ref()||!super::valid_uuid(&receipt.control_run_id){return Err("Recibo de exclusão Linux não corresponde ao item revisado.".into());}
        match receipt.state.as_str(){"purged"=>Ok(SpoolOutcome::Purged),"remaining"=>Ok(SpoolOutcome::Present),"unknown"=>Ok(SpoolOutcome::Unknown),_=>Err("Estado de exclusão Linux inválido.".into())}
    }
}
impl SpoolBackend for KaliRetention<'_>{
    fn metadata_inventory(&self)->Result<Vec<KaliMetadataItem>,String>{self.bridge.terminal_metadata(self.store)}
    fn metadata_capacity(&self,count:usize)->Result<(),String>{self.bridge.require_metadata_capacity(count)}
    fn inspect_metadata(&self,id:&str)->Result<KaliRetirePreview,String>{self.bridge.inspect_capture_retire(self.store,id,self.identity)}
    fn retire_metadata(&self,preview:&KaliRetirePreview,id:&str)->Result<MetadataOutcome,String>{let receipt=self.bridge.execute_capture_retire(self.store,preview,id,&preview.inspection_digest,self.identity)?;self.metadata_outcome(preview,id,receipt)}
    fn reconcile_metadata(&self,preview:&KaliRetirePreview,id:&str)->Result<MetadataOutcome,String>{
        preview.validate()?;let reference=self.reference(&preview.item.reference.capture_id)?;
        if reference!=preview.item.reference||reference.availability=="foreign_profile"{return Err("capture_owner_unavailable".into());}
        match self.bridge.reconcile_capture_retire(self.store,id,self.identity){Ok(receipt)=>self.metadata_outcome(preview,id,receipt),Err(error) if error=="capture_retire_missing"=>Ok(MetadataOutcome::Remaining),Err(error)=>Err(error)}
    }
    fn inventory(&self)->Result<Vec<SpoolItem>,String>{Ok(self.bridge.spool_metadata(self.store,None)?.iter().map(item).collect())}
    fn inspect(&self,id:&str)->Result<SpoolPreview,String>{
        let reference=self.reference(id)?;
        let inspection=self.bridge.inspect_capture_purge(self.store,&reference,self.identity)?;
        Ok(SpoolPreview{reference:item(&inspection.reference),inspection_digest:inspection.digest,previous_receipt_id:inspection.previous_receipt_id,previous_receipt_digest:inspection.previous_receipt_digest})
    }
    fn purge(&self,preview:&SpoolPreview,id:&str)->Result<SpoolOutcome,String>{
        validate(preview,true)?;
        let reference=self.reference(&preview.reference.capture_id)?;
        if item(&reference)!=preview.reference{return Err("A propriedade ou os bytes da captura Linux mudaram após a revisão.".into());}
        let inspection=self.bridge.inspect_capture_purge(self.store,&reference,self.identity)?;
        if inspection.digest!=preview.inspection_digest||item(&inspection.reference)!=preview.reference||inspection.previous_receipt_id!=preview.previous_receipt_id||inspection.previous_receipt_digest!=preview.previous_receipt_digest{return Err("A inspeção da captura Linux mudou após a revisão.".into());}
        let receipt=self.bridge.execute_capture_purge(self.store,&inspection,id,&preview.inspection_digest,self.identity)?;
        self.outcome(preview,id,receipt)
    }
    fn reconcile(&self,preview:&SpoolPreview,id:&str)->Result<SpoolOutcome,String>{
        validate(preview,true)?;
        let reference=self.reference(&preview.reference.capture_id)?;
        let mut current=item(&reference);current.availability=preview.reference.availability.clone();
        if current!=preview.reference||reference.availability=="foreign_profile"{return Err("A captura Linux pertence a outro perfil ou mudou de identidade; nenhum controle foi iniciado.".into());}
        match self.bridge.reconcile_capture_purge(self.store,id,self.identity){
            Ok(receipt)=>self.outcome(preview,id,receipt),
            // The bridge persists the control before launching a purge. An
            // absent journal therefore proves this exact receipt never sent it.
            Err(error) if error=="capture_purge_missing"=>Ok(SpoolOutcome::Present),
            Err(error)=>Err(error),
        }
    }
}

pub(super) fn validate(preview:&SpoolPreview,inspected:bool)->Result<(),String>{
    let r=&preview.reference;
    if ![&r.capture_id,&r.assessment_id,&r.run_id].iter().all(|id|super::valid_uuid(id))
        ||r.environment_id.is_empty()||r.environment_id.len()>128
        ||r.environment_id.chars().any(char::is_control)
        ||!crate::workflow::valid_hash(&r.owner_profile_fingerprint)
        ||r.manifest_digest.as_ref().is_none_or(|hash|!crate::workflow::valid_hash(hash))
        ||r.preserved_bytes.is_none_or(|n|n>crate::capture::MAX_CAPTURE_BYTES)
        ||r.availability!="owned"
        ||(inspected&&!crate::workflow::valid_hash(&preview.inspection_digest))
        ||preview.previous_receipt_id.is_some()!=preview.previous_receipt_digest.is_some()
        ||preview.previous_receipt_id.as_ref().is_some_and(|id|!super::valid_uuid(id))
        ||preview.previous_receipt_digest.as_ref().is_some_and(|hash|!crate::workflow::valid_hash(hash))
    {return Err("Captura Linux sem identidade, propriedade ou manifesto verificável neste perfil.".into());}Ok(())
}

pub(super) fn receipt_id(plan_id:&str,capture_id:&str)->String{
    use sha2::{Digest,Sha256};
    let digest=Sha256::digest(format!("openkoda.retention-kali.v1:{plan_id}:{capture_id}").as_bytes());
    uuid::Uuid::from_slice(&digest[..16]).expect("fixed digest length").to_string()
}
pub(super) fn metadata_receipt_id(plan_id:&str,capture_id:&str)->String{
    use sha2::{Digest,Sha256};let digest=Sha256::digest(format!("openkoda.retention-kali-metadata.v1:{plan_id}:{capture_id}").as_bytes());
    uuid::Uuid::from_slice(&digest[..16]).expect("fixed digest length").to_string()
}
