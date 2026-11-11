//! Metadata references only. This module never reads or accepts a credential value.
use crate::{core::{self,Store},model::{self,Snapshot},data_policy::{self,decode,error,hash,sha},data_projection::{DataRedactor,RedactionMode}};
pub use crate::data_policy::{CredentialRef,ProviderCredentialIdentity,ProviderCredentialUse};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]#[serde(rename_all="snake_case")]
pub enum CredentialKind{ProviderReference,TargetReference}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProviderSlotMetadata{pub id:String,pub provider:String,pub created_at:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProviderSlotRef{pub provider:String,pub slot_id:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CredentialReferenceRevision{pub id:String,pub reference_id:String,pub project_id:String,pub number:u32,pub previous_revision_id:Option<String>,pub kind:CredentialKind,pub label:String,pub purpose:String,pub target_catalog_id:Option<String>,pub provider_slot_ref:Option<ProviderSlotRef>,pub revoked:bool,pub created_at:String,pub digest:String,pub idempotency_key:String,pub request_digest:String}
pub fn provider_slot(s:&Snapshot)->Option<&ProviderSlotMetadata>{s.provider_slots.first()}
pub fn ensure_provider_slot(store:&Store)->Result<ProviderSlotMetadata,String>{let _gate=store.scope_gate.lock().map_err(|_|error("data_busy"))?;if let Some(slot)=provider_slot(&store.snapshot()?){return Ok(slot.clone());}store.mutate(|s|{let slot=ProviderSlotMetadata{id:model::id(),provider:"openai".into(),created_at:model::now()};s.provider_slots.push(slot.clone());Ok(slot)})}
pub fn current<'a>(s:&'a Snapshot,pid:&str,id:&str)->Option<&'a CredentialReferenceRevision>{s.credential_references.iter().rev().find(|r|r.project_id==pid&&r.reference_id==id)}
pub fn reference(r:&CredentialReferenceRevision)->CredentialRef{CredentialRef{id:r.reference_id.clone(),revision_id:r.id.clone(),digest:r.digest.clone()}}
pub fn resolve_use(s:&Snapshot,pid:&str,id:&str,identity:&ProviderCredentialIdentity)->Result<ProviderCredentialUse,String>{
    let r=current(s,pid,id).ok_or_else(||error("credential_reference_changed"))?;
    if r.revoked||r.kind!=CredentialKind::ProviderReference||r.provider_slot_ref.as_ref().is_none_or(|slot|slot.provider!="openai"||slot.slot_id!=identity.slot_id)||provider_slot(s).is_none_or(|slot|slot.id!=identity.slot_id)||!identity.available||identity.revision.is_empty(){return Err(error("credential_reference_changed"));}
    Ok(ProviderCredentialUse{reference:reference(r),slot_id:identity.slot_id.clone(),revision:identity.revision.clone()})
}
pub fn validate_use(s:&Snapshot,pid:&str,value:&ProviderCredentialUse)->Result<(),String>{let r=current(s,pid,&value.reference.id).ok_or_else(||error("credential_reference_changed"))?;if r.revoked||reference(r)!=value.reference||r.provider_slot_ref.as_ref().is_none_or(|p|p.slot_id!=value.slot_id)||provider_slot(s).is_none_or(|p|p.id!=value.slot_id)||value.revision.is_empty(){return Err(error("credential_reference_changed"));}Ok(())}
fn revision_digest(r:&CredentialReferenceRevision)->String{let mut value=serde_json::to_value(r).expect("credential metadata");value.as_object_mut().unwrap().remove("digest");hash(&json!({"schema":"openkoda.credential-reference.v1","record":value}))}
pub fn validate_snapshot(s:&Snapshot)->Result<(),String>{
    if s.provider_slots.len()>1{return Err(error("credential_reference_integrity"));}for slot in &s.provider_slots{core::valid_id(&slot.id)?;if slot.provider!="openai"||chrono::DateTime::parse_from_rfc3339(&slot.created_at).is_err(){return Err(error("credential_reference_integrity"));}}
    let mut ids=std::collections::HashSet::new();let mut heads=std::collections::HashMap::<&str,&CredentialReferenceRevision>::new();
    for r in &s.credential_references{core::valid_id(&r.id)?;core::valid_id(&r.reference_id)?;core::valid_id(&r.idempotency_key)?;if !ids.insert(&r.id)||!s.projects.iter().any(|p|p.id==r.project_id)||r.digest!=revision_digest(r)||!sha(&r.request_digest)||r.label.trim().is_empty()||r.label.len()>200||r.purpose.len()>1000{return Err(error("credential_reference_integrity"));}
        let old=heads.get(r.reference_id.as_str());if r.number!=old.map_or(1,|p|p.number+1)||r.previous_revision_id.as_deref()!=old.map(|p|p.id.as_str())||old.is_some_and(|p|p.project_id!=r.project_id||p.kind!=r.kind||p.target_catalog_id!=r.target_catalog_id||p.provider_slot_ref!=r.provider_slot_ref||p.revoked){return Err(error("credential_reference_integrity"));}
        match r.kind{CredentialKind::ProviderReference=>if r.target_catalog_id.is_some()||r.provider_slot_ref.as_ref().is_none_or(|p|p.provider!="openai"||provider_slot(s).is_none_or(|v|v.id!=p.slot_id)){return Err(error("credential_reference_integrity"));},CredentialKind::TargetReference=>if r.provider_slot_ref.is_some()||r.target_catalog_id.is_none(){return Err(error("credential_reference_integrity"));}}
        heads.insert(&r.reference_id,r);
    }
    for p in &s.projects{if heads.values().filter(|r|r.project_id==p.id).count()>100||heads.values().filter(|r|r.project_id==p.id).any(|r|r.number>20){return Err(error("data_limit"));}}Ok(())
}
pub fn state_digest(s:&Snapshot,pid:&str)->String{hash(&json!({"projectId":pid,"references":s.credential_references.iter().filter(|r|r.project_id==pid).collect::<Vec<_>>(),"slot":provider_slot(s),"catalog":s.projects.iter().find(|p|p.id==pid).map(crate::scope_targets::catalog_digest)}))}
pub fn list(s:&Snapshot,pid:&str,redactor:&dyn DataRedactor,provider:Option<&ProviderCredentialIdentity>)->Result<Value,String>{
    if !s.projects.iter().any(|p|p.id==pid){return Err(error("data_project_missing"));}
    let refs=s.credential_references.iter().filter(|r|r.project_id==pid&&current(s,pid,&r.reference_id).is_some_and(|h|h.id==r.id)).map(|r|json!({"id":r.reference_id,"revisionId":r.id,"digest":r.digest,"kind":r.kind,"label":crate::data_projection::project_text(&r.label,redactor),"purpose":crate::data_projection::project_text(&r.purpose,redactor),"targetCatalogId":r.target_catalog_id,"providerSlotRef":r.provider_slot_ref,"revoked":r.revoked,"capability":if r.kind==CredentialKind::TargetReference{"metadata_only"}else{"provider_slot_reference"},"providerState":provider.filter(|_|r.kind==CredentialKind::ProviderReference).map(|i|json!({"available":i.available&&r.provider_slot_ref.as_ref().is_some_and(|p|p.slot_id==i.slot_id),"credentialRevision":i.revision,"connectionVerified":i.connection_verified}))})).collect::<Vec<_>>();
    Ok(json!({"projectId":pid,"stateDigest":state_digest(s,pid),"references":refs,"providerSlots":provider_slot(s).map(|p|vec![json!({"id":p.id,"provider":p.provider,"available":provider.is_some_and(|i|i.available&&i.slot_id==p.id),"credentialRevision":provider.filter(|i|i.slot_id==p.id).map(|i|&i.revision),"connectionVerified":provider.is_some_and(|i|i.slot_id==p.id&&i.connection_verified)})]).unwrap_or_default(),"limits":["Referências de alvo são somente metadados; nenhum adaptador autenticado está disponível."]}))
}
pub fn supports(action:&str)->bool{matches!(action,"credential.reference.list"|"credential.reference.create"|"credential.reference.update"|"credential.reference.revoke")}
pub fn handle(action:&str,payload:Value,store:&Store,redactor:&dyn DataRedactor,provider:Option<&ProviderCredentialIdentity>)->Result<Value,String>{
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct List{project_id:String}
    if action=="credential.reference.list"{let p:List=decode(payload)?;return list(&store.snapshot()?,&p.project_id,redactor,provider);}
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Create{project_id:String,expected_state_digest:String,idempotency_key:String,kind:CredentialKind,label:String,purpose:String,target_catalog_id:Option<String>,provider_slot_id:Option<String>}
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Update{project_id:String,id:String,expected_state_digest:String,idempotency_key:String,label:String,purpose:String}
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Revoke{project_id:String,id:String,expected_state_digest:String,idempotency_key:String,acknowledge_revocation:bool}
    let request=hash(&json!({"action":action,"payload":payload}));
    let (pid,expected,key,id,kind,label,purpose,target,slot,revoke)=match action{
        "credential.reference.create"=>{let p:Create=decode(payload)?;(p.project_id,p.expected_state_digest,p.idempotency_key,None,Some(p.kind),Some(p.label),Some(p.purpose),p.target_catalog_id,p.provider_slot_id,false)},
        "credential.reference.update"=>{let p:Update=decode(payload)?;(p.project_id,p.expected_state_digest,p.idempotency_key,Some(p.id),None,Some(p.label),Some(p.purpose),None,None,false)},
        "credential.reference.revoke"=>{let p:Revoke=decode(payload)?;if !p.acknowledge_revocation{return Err(error("data_confirmation_required"));}(p.project_id,p.expected_state_digest,p.idempotency_key,Some(p.id),None,None,None,None,None,true)},_=>return Err(error("data_action_unknown"))};
    core::valid_id(&pid)?;core::valid_id(&key)?;
    if !revoke{if redactor.mode()==RedactionMode::MetadataOnly{return Err(error("data_redactor_unavailable"));}for(text,max)in [(label.as_deref().unwrap_or(""),200),(purpose.as_deref().unwrap_or(""),1000)]{if text.trim().is_empty()||text.len()>max||text.chars().any(char::is_control)||redactor.redact(text).secret_material{return Err(error("data_secret_material"));}}}
    let _gate=store.scope_gate.lock().map_err(|_|error("data_busy"))?;
    let result=store.mutate(|s|{
        let project=s.projects.iter().find(|p|p.id==pid&&!p.archived).ok_or_else(||error("data_project_unavailable"))?;
        if let Some(r)=s.credential_references.iter().find(|r|r.project_id==pid&&r.idempotency_key==key){if r.request_digest!=request{return Err(error("data_idempotency_conflict"));}return Ok((reference(r),true));}
        if state_digest(s,&pid)!=expected{return Err(error("credential_reference_changed"));}
        let old=id.as_ref().map(|id|current(s,&pid,id).cloned().ok_or_else(||error("credential_reference_changed"))).transpose()?;
        if old.as_ref().is_some_and(|r|r.revoked){return Err(error("credential_reference_changed"));}
        let mut next=if let Some(old)=old{CredentialReferenceRevision{id:model::id(),number:old.number+1,previous_revision_id:Some(old.id.clone()),label:label.clone().unwrap_or(old.label.clone()),purpose:purpose.clone().unwrap_or(old.purpose.clone()),revoked:revoke,created_at:model::now(),idempotency_key:key.clone(),request_digest:request.clone(),..old}}else{
            let kind=kind.clone().ok_or_else(||error("data_invalid_request"))?;
            match kind{CredentialKind::ProviderReference=>{if target.is_some()||slot.as_ref().is_none_or(|id|provider_slot(s).is_none_or(|p|&p.id!=id)){return Err(error("credential_reference_changed"));}},CredentialKind::TargetReference=>{if slot.is_some()||target.as_ref().is_none_or(|id|!crate::scope_targets::catalog_targets(project).iter().any(|t|&t.id==id&&!t.inactive)){return Err(error("credential_reference_changed"));}}}
            CredentialReferenceRevision{id:model::id(),reference_id:model::id(),project_id:pid.clone(),number:1,previous_revision_id:None,kind,label:label.clone().unwrap(),purpose:purpose.clone().unwrap(),target_catalog_id:target.clone(),provider_slot_ref:slot.clone().map(|slot_id|ProviderSlotRef{provider:"openai".into(),slot_id}),revoked:false,created_at:model::now(),digest:String::new(),idempotency_key:key.clone(),request_digest:request.clone()}
        };next.digest=revision_digest(&next);let r=reference(&next);s.credential_references.push(next);Ok((r,false))
    })?;
    Ok(json!({"created":action.ends_with("create"),"updated":action.ends_with("update"),"revoked":revoke,"replayed":result.1,"reference":result.0,"list":list(&store.snapshot()?,&pid,redactor,provider)?}))
}
