//! Versioned, prospective assessment scope. A scope is a ceiling, never a grant.
//! Historical validation is pure; expiry/revocation only guard new work.
#[path = "project_launch.rs"]
pub(crate) mod project_launch;
use crate::{core::{self,Store},model::{self,Assessment,Snapshot}};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::HashSet,time::{Duration,Instant}};
use crate::scope_targets::{self,ScopeTargetRef,TargetSetInput};

pub const CATALOG_VERSION:&str="openkoda.scope-methods.v1";
const LIMITS:&[&str]=&["Escopo limita objetivo, métodos e janela para iniciar operações; não aprova planos ou envios.","Modalidade e alvo continuam iguais aos do projeto. Fontes combinadas, exclusões e múltiplos alvos não estão disponíveis.","Revogação impede novas admissões; operações já admitidas exigem cancelamento ou reconciliação próprios."];
const METHODS:&[(&str,&str,&[&str],&str)]=&[
    ("local.inventory.v1","Inventário local",&["code","files"],"local_deadline"),
    ("source.rules.v1","Análise lexical local",&["code","files"],"local_deadline"),
    ("source.capture.v1","Captura de fontes selecionadas",&["code","files"],"admission_only"),
    ("http.headers.v1","Observação HTTP HEAD",&["web"],"local_deadline"),
    ("network.tcp-connect.v1","Conexões TCP delimitadas",&["network"],"local_deadline"),
    ("kali.runtime-diagnostics.v1","Diagnóstico Kali gerido",&["code","files","web","network"],"local_deadline"),
    ("kali.hash-evidence.v1","Hash de evidência no Kali",&["code","files","web","network"],"local_deadline"),
    ("kali.rg.signals.v1","Sinais lexicais no Kali",&["code","files","web","network"],"local_deadline"),
    ("ai.remote-review.v1","Revisão remota pela IA",&["code","files","web","network"],"admission_only"),
    ("local.text-remediation.v1","Correção e reteste textual local",&["code","files"],"admission_only"),
];
fn err(code:&str)->String{format!("{code}: revise o escopo e o estado atual da avaliação.")}
fn decode<T:serde::de::DeserializeOwned>(v:Value)->Result<T,String>{serde_json::from_value(v).map_err(|_|err("scope_invalid_input"))}
fn hash<T:Serialize>(v:&T)->String{core::hash(&serde_json::to_string(v).expect("scope serializable"))}
fn stamp(v:&str)->Result<chrono::DateTime<chrono::FixedOffset>,String>{chrono::DateTime::parse_from_rfc3339(v).map_err(|_|err("scope_invalid_time"))}
fn text(v:&str,max:usize)->Result<String,String>{let v=v.trim();if v.is_empty()||v.chars().count()>max||v.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\t')){Err(err("scope_invalid_input"))}else{Ok(v.into())}}
fn sha(v:&str)->bool{v.len()==64&&v.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}

#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeWindow {pub starts_at:Option<String>,pub ends_at:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeBody {pub objective:String,pub kind:String,pub target:String,pub methods:Vec<String>,pub window:ScopeWindow,#[serde(default,skip_serializing_if="Option::is_none")]pub target_set:Option<scope_targets::ScopeTargetSet>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeRef {pub assessment_id:String,pub revision_id:String,pub digest:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeAdmission {pub scope_ref:ScopeRef,pub admitted_at:String,pub configured_timeout_ms:u64,pub effective_timeout_ms:u64,pub limiting_factor:String,#[serde(default,skip_serializing_if="Option::is_none")]pub target_ref:Option<ScopeTargetRef>,#[serde(default,skip_serializing_if="Vec::is_empty")]pub identity_preflight_refs:Vec<ScopeTargetRef>}
pub fn admission_receipt(s:&Snapshot,aid:&str,method:&str,configured_timeout_ms:u64)->Result<ScopeAdmission,String>{
    check_work(s,aid,method,None)?;admission_receipt_selected(s,aid,method,configured_timeout_ms,&[])
}
pub fn admission_receipt_selected(s:&Snapshot,aid:&str,method:&str,configured_timeout_ms:u64,refs:&[ScopeTargetRef])->Result<ScopeAdmission,String>{
    let scope_ref=check_selected_work(s,aid,method,None,refs)?;let r=s.scope_revisions.iter().find(|r|r.id==scope_ref.revision_id).ok_or_else(||err("scope_integrity_error"))?;
    let at=chrono::Utc::now();let remaining=r.body.window.ends_at.as_ref().map(|end|(stamp(end)?.with_timezone(&chrono::Utc)-at).to_std().map(|d|d.as_millis().min(u64::MAX as u128) as u64).map_err(|_|err("scope_expired"))).transpose()?;
    let effective_timeout_ms=remaining.map_or(configured_timeout_ms,|ms|ms.min(configured_timeout_ms));if effective_timeout_ms==0{return Err(err("scope_expired"));}
    Ok(ScopeAdmission{scope_ref,admitted_at:at.to_rfc3339(),configured_timeout_ms,effective_timeout_ms,limiting_factor:if effective_timeout_ms<configured_timeout_ms{"scope_window_end"}else{"plan_timeout"}.into(),target_ref:if refs.len()==1{Some(refs[0].clone())}else{None},identity_preflight_refs:vec![]})
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeReview {pub author:String,pub reviewed_at:String,pub acknowledge_scope:bool}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeRevision {pub id:String,pub assessment_id:String,pub number:u32,pub previous_revision_id:Option<String>,pub body:ScopeBody,pub created_at:String,pub origin:String,pub review:Option<ScopeReview>,pub digest:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeFreeze {pub revision_id:String,pub digest:String,pub recorded_at:String,pub trigger_action:String,pub reason:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeRevocation {pub id:String,pub revision_id:String,pub at:String,pub author:String,pub rationale:String,pub prevent_new_admissions:bool}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeOrigin {pub source_assessment_id:String,pub source_scope:Option<ScopeRef>,pub source_legacy_digest:Option<String>,pub copy_fields:Vec<String>,pub created_at:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct AssessmentScope {pub id:String,pub assessment_id:String,pub generation:u64,pub current_revision_id:Option<String>,pub frozen:Option<ScopeFreeze>,pub revocation:Option<ScopeRevocation>,pub origin:Option<ScopeOrigin>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeResult {pub assessment_id:String,pub scope_id:String,pub revision_id:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]pub conversation_source:Option<ConversationSource>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ConversationSource {
    pub scope:ScopeRef,
    pub scan_id:Option<String>,
    pub policy:Option<crate::data_policy::DataPolicyRef>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeReceipt {pub idempotency_key:String,pub action:String,pub assessment_id:String,pub request_digest:String,pub created_at:String,pub result:ScopeResult}
#[derive(Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CreationScope {pub methods:Vec<String>,pub window:ScopeWindow,pub project_digest:String,pub catalog_version:String,pub acknowledge_scope:bool}

fn body(a:&Assessment,objective:&str,methods:&[String],window:&ScopeWindow)->Result<ScopeBody,String>{
    Ok(ScopeBody{objective:text(objective,4000)?,kind:a.kind.clone(),target:a.target.clone(),methods:normalize_methods(&a.kind,methods)?,window:normalize_window(window)?,target_set:None})
}
pub(crate) fn normalize_methods(kind:&str,methods:&[String])->Result<Vec<String>,String>{
    let mut methods=methods.to_vec();let count=methods.len();methods.sort();methods.dedup();
    if count>10||count!=methods.len(){return Err(err("scope_invalid_methods"));}
    for method in &methods{if !METHODS.iter().any(|(id,_,kinds,_)|*id==method&&kinds.contains(&kind)){return Err(err("scope_method_incompatible"));}}
    if methods.iter().any(|m|m=="source.capture.v1")&&!methods.iter().any(|m|m=="source.rules.v1"){return Err(err("scope_method_dependency_missing"));}
    Ok(methods)
}
fn normalize_window(window:&ScopeWindow)->Result<ScopeWindow,String>{
    let normalize=|v:&Option<String>|v.as_ref().map(|v|stamp(v).map(|v|v.with_timezone(&chrono::Utc).to_rfc3339())).transpose();
    let window=ScopeWindow{starts_at:normalize(&window.starts_at)?,ends_at:normalize(&window.ends_at)?};
    if let (Some(start),Some(end))=(&window.starts_at,&window.ends_at){if stamp(start)?>=stamp(end)?{return Err(err("scope_invalid_window"));}}
    Ok(window)
}
fn revision_digest(r:&ScopeRevision)->String{hash(&json!({"schema":if r.body.target_set.is_some(){"openkoda.scope-revision.v2"}else{"openkoda.scope-revision.v1"},"id":r.id,"assessmentId":r.assessment_id,"number":r.number,"previousRevisionId":r.previous_revision_id,"body":r.body,"createdAt":r.created_at,"origin":r.origin,"review":r.review}))}
fn revision(aid:&str,body:ScopeBody,previous:Option<&ScopeRevision>,reviewed:bool)->ScopeRevision{
    let at=model::now();let mut r=ScopeRevision{id:model::id(),assessment_id:aid.into(),number:previous.map_or(1,|p|p.number+1),previous_revision_id:previous.map(|p|p.id.clone()),body,created_at:at.clone(),origin:if reviewed{"human_review"}else{"derived_prefill"}.into(),review:reviewed.then(||ScopeReview{author:"local-user".into(),reviewed_at:at,acknowledge_scope:true}),digest:String::new()};r.digest=revision_digest(&r);r
}
fn assessment<'a>(s:&'a Snapshot,id:&str)->Result<&'a Assessment,String>{core::valid_id(id)?;s.assessments.iter().find(|a|a.id==id).ok_or_else(||err("scope_assessment_missing"))}
fn binding<'a>(s:&'a Snapshot,a:&Assessment)->Result<Option<&'a AssessmentScope>,String>{a.scope_id.as_ref().map(|id|s.assessment_scopes.iter().find(|b|&b.id==id&&b.assessment_id==a.id).ok_or_else(||err("scope_integrity_error"))).transpose()}
fn current<'a>(s:&'a Snapshot,b:&AssessmentScope)->Result<Option<&'a ScopeRevision>,String>{b.current_revision_id.as_ref().map(|id|s.scope_revisions.iter().find(|r|&r.id==id&&r.assessment_id==b.assessment_id).ok_or_else(||err("scope_integrity_error"))).transpose()}
fn reference(r:&ScopeRevision)->ScopeRef{ScopeRef{assessment_id:r.assessment_id.clone(),revision_id:r.id.clone(),digest:r.digest.clone()}}
pub fn current_ref(s:&Snapshot,id:&str)->Result<Option<ScopeRef>,String>{let a=assessment(s,id)?;let Some(b)=binding(s,a)?else{return Ok(None)};Ok(current(s,b)?.filter(|r|r.review.is_some()).map(reference))}
fn project_digest(p:&model::Project)->String{hash(&json!({"projectId":p.id,"kind":p.kind,"target":p.target,"archived":p.archived,"catalogVersion":CATALOG_VERSION}))}
pub(crate) fn catalog(kind:&str)->Vec<Value>{METHODS.iter().map(|(id,label,kinds,enforcement)|{let allowed=kinds.contains(&kind);json!({"id":id,"label":label,"allowed":allowed,"reasonCodes":if allowed{vec![]}else{vec!["scope_method_incompatible"]},"effects":label,"windowEnforcement":enforcement})}).collect()}
pub fn options(s:&Snapshot,pid:&str)->Result<Value,String>{core::valid_id(pid)?;let p=s.projects.iter().find(|p|p.id==pid).ok_or_else(||err("scope_project_missing"))?;Ok(json!({"schema":"openkoda.scope-options.v1","projectId":p.id,"kind":p.kind,"target":p.target,"projectDigest":project_digest(p),"catalogVersion":CATALOG_VERSION,"methodCatalog":catalog(&p.kind),"defaultProposal":{"methods":[],"window":{"startsAt":null,"endsAt":null}},"canCreate":!p.archived,"reasonCodes":if p.archived{vec!["project_archived"]}else{vec![]},"limits":LIMITS}))}
pub fn initialize(s:&mut Snapshot,a:&mut Assessment,input:Option<&CreationScope>)->Result<(),String>{
    let mut b=AssessmentScope{id:model::id(),assessment_id:a.id.clone(),generation:1,current_revision_id:None,frozen:None,revocation:None,origin:None};
    if let Some(input)=input{let p=s.projects.iter().find(|p|p.id==a.project_id).ok_or_else(||err("scope_project_missing"))?;
        if !input.acknowledge_scope{return Err(err("scope_ack_required"));}if input.catalog_version!=CATALOG_VERSION{return Err(err("scope_catalog_changed"));}if input.project_digest!=project_digest(p){return Err(err("scope_state_changed"));}
        let r=revision(&a.id,body(a,&a.objective,&input.methods,&input.window)?,None,true);b.current_revision_id=Some(r.id.clone());s.scope_revisions.push(r);
    }
    a.scope_id=Some(b.id.clone());s.assessment_scopes.push(b);Ok(())
}
pub fn validate_historical_ref(s:&Snapshot,aid:&str,r:&ScopeRef)->Result<(),String>{
    let a=assessment(s,aid)?;let b=binding(s,a)?.ok_or_else(||err("scope_integrity_error"))?;
    let actual=s.scope_revisions.iter().find(|v|v.id==r.revision_id&&v.assessment_id==aid&&v.digest==r.digest&&v.review.is_some()).ok_or_else(||err("scope_integrity_error"))?;
    if r.assessment_id!=aid||b.frozen.as_ref().is_none_or(|f|f.revision_id!=actual.id||f.digest!=actual.digest){return Err(err("scope_integrity_error"));}Ok(())
}
pub fn validate_state(s:&Snapshot)->Result<(),String>{
    let mut ids=HashSet::new();let mut owners=HashSet::new();
    for b in &s.assessment_scopes{core::valid_id(&b.id)?;let a=assessment(s,&b.assessment_id)?;
        if !ids.insert(&b.id)||!owners.insert(&b.assessment_id)||a.scope_id.as_ref()!=Some(&b.id)||b.generation==0{return Err(err("scope_integrity_error"));}
        let revisions=s.scope_revisions.iter().filter(|r|r.assessment_id==a.id).collect::<Vec<_>>();if revisions.len()>100{return Err(err("scope_limit_reached"));}
        let mut previous:Option<&ScopeRevision>=None;for r in &revisions{core::valid_id(&r.id)?;stamp(&r.created_at)?;
            if r.number!=previous.map_or(1,|p|p.number+1)||r.previous_revision_id!=previous.map(|p|p.id.clone())||r.digest!=revision_digest(r){return Err(err("scope_integrity_error"));}
            if r.body.target_set.is_some(){scope_targets::validate_body(s,&a.id,&r.body)?;if r.body.objective!=text(&r.body.objective,4000)?||r.body.window!=normalize_window(&r.body.window)?||previous.is_some_and(|p|p.body.target_set.is_none()){return Err(err("scope_integrity_error"));}}
            else if r.body!=body(a,&r.body.objective,&r.body.methods,&r.body.window)?||previous.is_some_and(|p|p.body.target_set.is_some()){return Err(err("scope_integrity_error"));}
            match &r.review{Some(review)=>{stamp(&review.reviewed_at)?;if review.author!="local-user"||!review.acknowledge_scope||r.origin!="human_review"{return Err(err("scope_integrity_error"));}},None=>if r.origin!="derived_prefill"||r.number!=1||b.origin.is_none(){return Err(err("scope_integrity_error"));}}
            previous=Some(r);
        }
        if b.current_revision_id!=revisions.last().map(|r|r.id.clone()){return Err(err("scope_integrity_error"));}
        if let Some(r)=current(s,b)?{if r.body.objective!=a.objective||r.body.kind!=a.kind||r.body.target!=a.target{return Err(err("scope_integrity_error"));}}
        if let Some(f)=&b.frozen{let r=current(s,b)?.ok_or_else(||err("scope_integrity_error"))?;stamp(&f.recorded_at)?;if f.revision_id!=r.id||f.digest!=r.digest||r.review.is_none()||!["first_work","legacy_adoption"].contains(&f.reason.as_str())||f.trigger_action.is_empty(){return Err(err("scope_integrity_error"));}}
        if let Some(v)=&b.revocation{core::valid_id(&v.id)?;stamp(&v.at)?;if b.frozen.as_ref().is_none_or(|f|f.revision_id!=v.revision_id)||v.author!="local-user"||!v.prevent_new_admissions||v.rationale.trim().is_empty()||v.rationale.len()>2000{return Err(err("scope_integrity_error"));}}
        if let Some(o)=&b.origin{stamp(&o.created_at)?;let source=assessment(s,&o.source_assessment_id)?;let allowed=if scope_targets::is_combined(s,&a.id){vec!["objective","window","targets","exclusions"]}else{vec!["objective","methods","window"]};if source.id==a.id||source.project_id!=a.project_id||o.copy_fields.iter().any(|f|!allowed.contains(&f.as_str()))||o.copy_fields.iter().collect::<HashSet<_>>().len()!=o.copy_fields.len(){return Err(err("scope_integrity_error"));}
            if let Some(r)=&o.source_scope{let source_r=s.scope_revisions.iter().find(|v|v.id==r.revision_id&&v.assessment_id==source.id&&v.digest==r.digest).ok_or_else(||err("scope_integrity_error"))?;if r.assessment_id!=source.id||source_r.review.is_none()||o.source_legacy_digest.is_some(){return Err(err("scope_integrity_error"));}}else if o.source_legacy_digest.as_deref().is_none_or(|v|!sha(v)){return Err(err("scope_integrity_error"));}
            let mut seen=HashSet::from([a.id.as_str()]);let mut cursor=source;loop{if !seen.insert(cursor.id.as_str()){return Err(err("scope_integrity_error"));}let Some(next)=binding(s,cursor)?.and_then(|b|b.origin.as_ref())else{break};cursor=assessment(s,&next.source_assessment_id)?;}
        }
    }
    for a in &s.assessments{binding(s,a)?;}
    ids.clear();for r in &s.scope_revisions{if !ids.insert(&r.id)||!owners.contains(&r.assessment_id){return Err(err("scope_integrity_error"));}}
    let mut keys=HashSet::new();for receipt in &s.scope_receipts{core::valid_id(&receipt.idempotency_key)?;stamp(&receipt.created_at)?;let source=assessment(s,&receipt.assessment_id)?;let result=assessment(s,&receipt.result.assessment_id)?;
        if let Some(parent)=&receipt.result.conversation_source{
            if receipt.action!="project.newConversation"||source.id!=result.id||parent.scope.assessment_id==source.id||
                assessment(s,&parent.scope.assessment_id)?.project_id!=source.project_id{return Err(err("scope_integrity_error"));}
            if !s.scope_revisions.iter().any(|r|r.assessment_id==parent.scope.assessment_id&&r.id==parent.scope.revision_id&&r.digest==parent.scope.digest&&r.review.is_some()){
                return Err(err("scope_integrity_error"));
            }
            if let Some(pin)=&parent.policy{
                if pin.project_id!=source.project_id||!s.data_policy_revisions.iter().any(|r|r.id==pin.revision_id&&r.digest==pin.digest&&r.project_id==pin.project_id){return Err(err("scope_integrity_error"));}
            }
        }
        if receipt.action == "assessment.create" && (source.id != result.id || source.id != simple_creation_id(&source.project_id, &receipt.idempotency_key)) {
            return Err(err("scope_integrity_error"));
        }
        if !keys.insert((&receipt.action,&receipt.assessment_id,&receipt.idempotency_key))||!sha(&receipt.request_digest)||source.project_id!=result.project_id||result.scope_id.as_ref()!=Some(&receipt.result.scope_id)||receipt.result.revision_id.as_ref().is_some_and(|id|!s.scope_revisions.iter().any(|r|&r.id==id&&r.assessment_id==result.id)){return Err(err("scope_integrity_error"));}
    }Ok(())
}
fn window_state(w:&ScopeWindow,now:chrono::DateTime<chrono::Utc>)->Result<&'static str,String>{if w.ends_at.as_ref().is_some_and(|v|stamp(v).is_ok_and(|v|v<=now)){return Ok("expired")};if w.starts_at.as_ref().is_some_and(|v|stamp(v).is_ok_and(|v|v>now)){return Ok("not_yet_open")};Ok(if w.starts_at.is_none()&&w.ends_at.is_none(){"unbounded"}else{"open"})}
pub fn check_work(s:&Snapshot,aid:&str,method:&str,expected:Option<&ScopeRef>)->Result<ScopeRef,String>{
    let actual=check_base_work(s,aid,method,expected)?;if scope_targets::is_combined(s,aid){return Err(err("scope_target_required"));}Ok(actual)
}
fn check_base_work(s:&Snapshot,aid:&str,method:&str,expected:Option<&ScopeRef>)->Result<ScopeRef,String>{
    let a=assessment(s,aid)?;if !s.projects.iter().any(|p|p.id==a.project_id&&!p.archived){return Err(err("project_archived"));}
    let b=binding(s,a)?.ok_or_else(||err("scope_legacy_requires_adoption"))?;let r=current(s,b)?.filter(|r|r.review.is_some()).ok_or_else(||err("scope_unreviewed"))?;
    if b.revocation.is_some(){return Err(err("scope_revoked"));}if !r.body.methods.iter().any(|m|m==method){return Err(err("scope_method_not_selected"));}
    match window_state(&r.body.window,chrono::Utc::now())?{"not_yet_open"=>return Err(err("scope_not_yet_open")),"expired"=>return Err(err("scope_expired")),_=>{}}
    let actual=reference(r);if expected.is_some_and(|e|e!=&actual){return Err(err("scope_state_changed"));}Ok(actual)
}
pub fn check_selected_work(s:&Snapshot,aid:&str,method:&str,expected:Option<&ScopeRef>,refs:&[ScopeTargetRef])->Result<ScopeRef,String>{let actual=check_base_work(s,aid,method,expected)?;let body=&s.scope_revisions.iter().find(|r|r.id==actual.revision_id).ok_or_else(||err("scope_integrity_error"))?.body;scope_targets::validate_selection(body,refs,Some(method))?;Ok(actual)}
fn freeze(s:&mut Snapshot,aid:&str,action:&str,reason:&str)->Result<ScopeRef,String>{
    let a=assessment(s,aid)?;let b=binding(s,a)?.ok_or_else(||err("scope_legacy_requires_adoption"))?;let r=current(s,b)?.filter(|r|r.review.is_some()).ok_or_else(||err("scope_unreviewed"))?;let scope=reference(r);let bid=b.id.clone();
    if b.frozen.is_none(){let b=s.assessment_scopes.iter_mut().find(|b|b.id==bid).unwrap();b.frozen=Some(ScopeFreeze{revision_id:scope.revision_id.clone(),digest:scope.digest.clone(),recorded_at:model::now(),trigger_action:action.into(),reason:reason.into()});b.generation+=1;s.assessments.iter_mut().find(|a|a.id==aid).unwrap().events.push(model::event(format!("Escopo {} congelado antes de {action}; nenhuma aprovação operacional foi criada.",scope.revision_id),"info"));}Ok(scope)
}
pub fn check_new_local_input(s:&Snapshot,aid:&str)->Result<(),String>{
    let a=assessment(s,aid)?;if !s.projects.iter().any(|p|p.id==a.project_id&&!p.archived){return Err(err("project_archived"));}
    let Some(b)=binding(s,a)?else{return Err(err("scope_legacy_requires_adoption"))};let r=current(s,b)?.filter(|r|r.review.is_some()).ok_or_else(||err("scope_unreviewed"))?;
    if b.revocation.is_some(){return Err(err("scope_revoked"));}match window_state(&r.body.window,chrono::Utc::now())?{"not_yet_open"=>Err(err("scope_not_yet_open")),"expired"=>Err(err("scope_expired")),_=>Ok(())}
}
pub fn freeze_review(s:&mut Snapshot,aid:&str,action:&str)->Result<(),String>{let a=assessment(s,aid)?;if let Some(b)=binding(s,a)?{if b.frozen.is_none(){check_new_local_input(s,aid)?;}freeze(s,aid,action,"first_work")?;}Ok(())}
pub fn with_admission<T>(store:&Store,aid:&str,method:&str,expected:Option<&ScopeRef>,action:&str,persist:impl FnOnce(&ScopeRef)->Result<T,String>)->Result<T,String>{
    let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;
    let reference=store.mutate(|s|{check_work(s,aid,method,expected)?;freeze(s,aid,action,"first_work")})?;persist(&reference)
}
pub fn with_selected_admission<T>(store:&Store,aid:&str,method:&str,expected:Option<&ScopeRef>,refs:&[ScopeTargetRef],action:&str,persist:impl FnOnce(&ScopeRef)->Result<T,String>)->Result<T,String>{let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;let reference=store.mutate(|s|{check_selected_work(s,aid,method,expected,refs)?;freeze(s,aid,action,"first_work")})?;persist(&reference)}
pub fn recheck_selected(store:&Store,scope:&ScopeRef,method:&str,refs:&[ScopeTargetRef])->Result<(),String>{check_selected_work(&store.snapshot()?,&scope.assessment_id,method,Some(scope),refs).map(|_|())}
pub fn admit(store:&Store,aid:&str,method:&str,expected:Option<&ScopeRef>,action:&str)->Result<ScopeRef,String>{with_admission(store,aid,method,expected,action,|r|Ok(r.clone()))}
pub fn recheck(store:&Store,scope:&ScopeRef,method:&str)->Result<(),String>{check_work(&store.snapshot()?,&scope.assessment_id,method,Some(scope)).map(|_|())}
pub fn deadline(store:&Store,scope:&ScopeRef,max:Duration)->Result<Instant,String>{let s=store.snapshot()?;let b=binding(&s,assessment(&s,&scope.assessment_id)?)?.ok_or_else(||err("scope_integrity_error"))?;let r=current(&s,b)?.ok_or_else(||err("scope_integrity_error"))?;let remaining=r.body.window.ends_at.as_ref().map(|end|(stamp(end)?.with_timezone(&chrono::Utc)-chrono::Utc::now()).to_std().map_err(|_|err("scope_expired"))).transpose()?;Ok(Instant::now()+remaining.map_or(max,|d|d.min(max)))}

// Read only known journals in this profile, never vault/cipher or arbitrary paths.
pub fn admitted_work(store:&Store,aid:&str)->Result<Vec<Value>,String>{
    let s=store.snapshot()?;assessment(&s,aid)?;let mut result=s.executions.iter().filter(|e|e.assessment_id==aid&&["running","canceling","unknown"].contains(&e.status.as_str())).map(|e|json!({"kind":"execution","id":e.id,"status":e.status})).collect::<Vec<_>>();
    for(file,table)in [("assistant.sqlite3","investigations"),("kali-bridge.sqlite3","bridge_runs"),("remediation.sqlite3","remediation_plans")]{let path=store.root.join(file);if !path.exists(){continue;}let db=rusqlite::Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_|err("scope_activity_unavailable"))?;let mut q=db.prepare(&format!("SELECT data FROM {table}")).map_err(|_|err("scope_activity_unavailable"))?;let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|_|err("scope_activity_unavailable"))?;for row in rows{let v:Value=serde_json::from_str(&row.map_err(|_|err("scope_activity_unavailable"))?).map_err(|_|err("scope_activity_unavailable"))?;if v["assessmentId"]==aid&&["starting","running","canceling","unknown","applying","retesting","confirming"].contains(&v["status"].as_str().unwrap_or("")){result.push(json!({"kind":table,"id":v["id"],"status":v["status"]}));}}
        if table=="investigations"{let mut q=db.prepare("SELECT id,status,details FROM requests WHERE assessment=?1 AND status IN ('pending','unknown')").map_err(|_|err("scope_activity_unavailable"))?;let rows=q.query_map([aid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(|_|err("scope_activity_unavailable"))?;for row in rows{let(id,status,details)=row.map_err(|_|err("scope_activity_unavailable"))?;if status!="unknown"||!crate::ai::request_unknown_acknowledged(&details){result.push(json!({"kind":"request","id":id,"status":status}));}}}
    }result.sort_by_key(|v|v["id"].as_str().unwrap_or("").to_string());Ok(result)
}
fn state_digest(s:&Snapshot,a:&Assessment,work:&[Value])->Result<String,String>{Ok(hash(&json!({"schema":"openkoda.scope-state.v1","assessment":a,"project":s.projects.iter().find(|p|p.id==a.project_id),"binding":binding(s,a)?,"revisions":s.scope_revisions.iter().filter(|r|r.assessment_id==a.id).collect::<Vec<_>>(),"work":work})))}
pub fn status(store:&Store,aid:&str)->Result<Value,String>{let s=store.snapshot()?;let work=admitted_work(store,aid)?;status_from(&s,aid,&work)}
fn status_from(s:&Snapshot,aid:&str,work:&[Value])->Result<Value,String>{
    let a=assessment(s,aid)?;let b=binding(s,a)?;let r=b.map(|b|current(s,b)).transpose()?.flatten();let archived=s.projects.iter().find(|p|p.id==a.project_id).is_none_or(|p|p.archived);let active=core::active(&a.status)||a.status=="awaiting_approval"||!work.is_empty();
    let phase=if b.is_none(){"legacy"}else if b.unwrap().revocation.is_some(){"revoked"}else if b.unwrap().frozen.is_some(){"frozen"}else if r.is_none_or(|r|r.review.is_none()){"unreviewed"}else{"draft"};
    let mut reasons=vec![];if archived{reasons.push("project_archived");}if active{reasons.push(if a.status=="unknown"||work.iter().any(|w|w["status"]=="unknown"){"assessment_work_unknown"}else{"assessment_work_active"});}if phase=="frozen"{reasons.push("scope_frozen");}if phase=="revoked"{reasons.push("scope_revoked");}
    let gates=METHODS.iter().map(|(method,_,_,_)|work_gate(s,aid,method,None)).collect::<Vec<_>>();
    let target_options=r.and_then(|r|r.body.target_set.as_ref()).map(|set|set.entries.iter().map(|entry|{let target=scope_targets::entry_ref(entry);json!({"targetRef":target,"entry":entry,"methodCatalog":catalog(&entry.kind),"workGates":METHODS.iter().filter(|(id,_,_,_)|*id!="kali.runtime-diagnostics.v1").map(|(method,_,_,_)|work_gate(s,aid,method,Some(std::slice::from_ref(&target)))).collect::<Vec<_>>()})}).collect::<Vec<_>>()).unwrap_or_default();
    let common_gates=if r.is_some_and(|r|r.body.target_set.is_some()){vec![work_gate(s,aid,"kali.runtime-diagnostics.v1",Some(&[]))]}else{vec![]};
    let can_prepare=gates.iter().chain(common_gates.iter()).any(|g|g["allowedByScope"]==true)||target_options.iter().any(|t|t["workGates"].as_array().is_some_and(|g|g.iter().any(|g|g["allowedByScope"]==true)));
    let freezes=b.and_then(|b|b.frozen.as_ref()).map(|f|vec![f.reason.clone(),f.trigger_action.clone()]).unwrap_or_else(||if phase=="legacy"{vec!["legacy_scope_unknown".into()]}else{vec![]});
    Ok(json!({"schema":"openkoda.scope-status.v1","assessmentId":a.id,"projectId":a.project_id,"stateDigest":state_digest(s,a,work)?,"phase":phase,"revision":r,"legacySummary":if b.is_none(){Some(json!({"objective":a.objective,"kind":a.kind,"target":a.target}))}else{None},"frozen":b.and_then(|b|b.frozen.as_ref()),"revocation":b.and_then(|b|b.revocation.as_ref()),"origin":b.and_then(|b|b.origin.as_ref()),"windowStatus":r.map(|r|window_state(&r.body.window,chrono::Utc::now())).transpose()?.unwrap_or("unbounded"),"evaluatedAt":model::now(),"methodCatalog":catalog(&a.kind),"actions":{"canSave":!archived&&!active&&matches!(phase,"unreviewed"|"draft"),"canAdopt":!archived&&!active&&phase=="legacy","canDerive":!archived&&!active,"canRevoke":!archived&&phase=="frozen","canPrepareWork":can_prepare,"canReviewStoredEvidence":!archived&&(b.is_none()||b.is_some_and(|b|b.frozen.is_some())||check_new_local_input(s,aid).is_ok()),"canImportEvidence":check_new_local_input(s,aid).is_ok(),"reasonCodes":reasons},"freezeReasons":freezes,"workGates":gates,"targetOptions":target_options,"commonWorkGates":common_gates,"targetCoverage":target_coverage(s,aid),"limits":if scope_targets::is_combined(s,aid){scope_targets::LIMITS}else{LIMITS}}))
}
fn work_gate(s:&Snapshot,aid:&str,method:&str,refs:Option<&[ScopeTargetRef]>)->Value{let checked=match refs{Some(refs)=>check_selected_work(s,aid,method,None,refs),None=>check_work(s,aid,method,None)};match checked{Ok(_)=>json!({"method":method,"allowedByScope":true,"reasonCodes":[]}),Err(e)=>json!({"method":method,"allowedByScope":false,"reasonCodes":[e.split(':').next().unwrap_or("scope_integrity_error")]})}}
pub fn target_coverage(s:&Snapshot,aid:&str)->Vec<Value>{
    let Some(set)=s.scope_revisions.iter().rev().find(|r|r.assessment_id==aid).and_then(|r|r.body.target_set.as_ref())else{return vec![]};
    set.entries.iter().map(|entry|{
        let reference=scope_targets::entry_ref(entry);
        let methods=entry.methods.iter().map(|method|{
            let applicable=["local.inventory.v1","source.rules.v1","http.headers.v1","network.tcp-connect.v1"].contains(&method.as_str());
            let plans=s.plans.iter().filter(|p|p.assessment_id==aid&&p.target_ref.as_ref()==Some(&reference)&&p.adapter==*method).collect::<Vec<_>>();
            let executions=s.executions.iter().filter(|e|e.assessment_id==aid&&plans.iter().any(|p|p.id==e.plan_id)).collect::<Vec<_>>();
            // Persisted order is authoritative even if the system clock recedes.
            let latest=executions.last().copied();
            let partial=latest.and_then(|e|plans.iter().find(|p|p.id==e.plan_id).and_then(|p|execution_partial(s,p,e)));
            let state=if !applicable{"separate_record"}else if executions.iter().any(|e|e.status=="unknown"){"inconclusive"}else{match latest.map(|e|e.status.as_str()){
                Some("completed")=>match partial{Some(true)=>"partial",Some(false)=>"executed",None=>"inconclusive"},
                Some("failed")=>"failed",Some("canceled")=>"canceled",Some("running"|"canceling")=>"in_progress",Some(_)=>"inconclusive",
                None=>if plans.is_empty(){"not_attempted"}else{"prepared"},
            }};
            json!({"method":method,"state":state,"partial":partial,"latestExecutionId":latest.map(|e|&e.id),"applicableExecutor":applicable,"recordNotice":if applicable{Value::Null}else{json!("Consultar registro próprio; este painel não infere ausência de trabalho.")},"planIds":plans.iter().map(|p|&p.id).collect::<Vec<_>>(),"executionIds":executions.iter().map(|e|&e.id).collect::<Vec<_>>()})
        }).collect::<Vec<_>>();
        json!({"targetRef":reference,"label":entry.label,"kind":entry.kind,"target":entry.target,"methods":methods,"limits":["Estado da última execução do método; os identificadores preservam também tentativas anteriores.","Cobertura de executores; IA, captura e correção têm registros próprios e não são inferidas como execução do alvo."]})
    }).collect()
}

fn execution_partial(s:&Snapshot,plan:&model::ExecutionPlan,execution:&model::Execution)->Option<bool>{
    if plan.digest!=crate::executor::plan_digest(plan){return None;}
    let a=s.assessments.iter().find(|a|a.id==execution.assessment_id)?;
    let evidence=a.evidence.iter().find(|e|Some(&e.id)==execution.evidence_id.as_ref())?;
    if evidence.sha256!=core::hash(&evidence.content){return None;}
    let omissions=!plan.scope_omissions.is_empty()||plan.source_scope.as_ref().is_some_and(|source|!source.excluded_directories.is_empty());
    if plan.adapter==crate::source_scan::ADAPTER{
        let output:crate::source_scan::ScanResult=serde_json::from_str(&evidence.content).ok()?;
        let source=plan.source_scope.as_ref()?;
        if output.adapter!=plan.adapter||output.manifest_sha256!=source.manifest_sha256||output.rules_version!=source.rules_version{return None;}
        return Some(omissions||output.partial);
    }
    let output:Value=serde_json::from_str(&evidence.content).ok()?;
    if output["adapter"]!=plan.adapter||output["planDigest"]!=plan.digest||output["target"]!=plan.target{return None;}
    match plan.adapter.as_str(){
        "local.inventory.v1"=>if output["files"]==serde_json::to_value(&plan.input_files).ok()?{Some(omissions)}else{None},
        "network.tcp-connect.v1"=>if output["plannedEndpoints"]==json!(plan.resolved_addresses){Some(omissions||output["capturePartial"].as_bool()?)}else{None},
        "http.headers.v1"=>if output["schema"]==crate::http_validation::OBSERVATION_SCHEMA{Some(omissions||!output["captureComplete"].as_bool()?)}else{None},
        _=>None,
    }
}

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct IdInput{assessment_id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct ProjectInput{project_id:String}
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Proposal{assessment_id:String,expected_state_digest:String,objective:String,methods:Vec<String>,window:ScopeWindow}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Commit{assessment_id:String,expected_state_digest:String,objective:String,methods:Vec<String>,window:ScopeWindow,review_digest:String,idempotency_key:String,acknowledge_scope:bool}
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Derive{assessment_id:String,expected_state_digest:String,title:String,copy_fields:Vec<String>,#[serde(default,skip_serializing_if="Option::is_none")]objective:Option<String>,#[serde(default,skip_serializing_if="Option::is_none")]methods:Option<Vec<String>>,#[serde(default,skip_serializing_if="Option::is_none")]window:Option<ScopeWindow>}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct DeriveCommit{assessment_id:String,expected_state_digest:String,title:String,copy_fields:Vec<String>,#[serde(default)]objective:Option<String>,#[serde(default)]methods:Option<Vec<String>>,#[serde(default)]window:Option<ScopeWindow>,review_digest:String,idempotency_key:String,acknowledge_new_assessment:bool}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Revoke{assessment_id:String,expected_state_digest:String,idempotency_key:String,rationale:String,acknowledge_revocation:bool}
pub fn supports(action:&str)->bool{matches!(action,"assessment.scopeOptions"|"assessment.scopeStatus"|"assessment.scopePreview"|"assessment.scopeSave"|"assessment.scopeAdopt"|"assessment.derivePreview"|"assessment.deriveCreate"|"assessment.scopeRevoke")||combined_supports(action)}
pub fn mutates(action:&str)->bool{matches!(action,"assessment.scopeSave"|"assessment.scopeAdopt"|"assessment.deriveCreate"|"assessment.scopeRevoke"|"assessment.combinedCreate"|"assessment.combinedScopeSave"|"assessment.combinedDeriveCreate")}
fn expected(actual:&Value,wanted:&str)->Result<(),String>{if actual["stateDigest"].as_str()!=Some(wanted){Err(err("scope_state_changed"))}else{Ok(())}}
fn preview(s:&Snapshot,p:&Proposal,work:&[Value])->Result<Value,String>{let a=assessment(s,&p.assessment_id)?;if scope_targets::is_combined(s,&a.id){return Err(err("scope_combined_action_required"));}let status=status_from(s,&a.id,work)?;expected(&status,&p.expected_state_digest)?;let legacy=a.scope_id.is_none();let proposed=body(a,&p.objective,&p.methods,&p.window)?;if legacy&&proposed.objective!=a.objective{return Err(err("scope_legacy_objective_immutable"));}let action=if legacy{"assessment.scopeAdopt"}else{"assessment.scopeSave"};Ok(json!({"assessmentId":a.id,"expectedStateDigest":p.expected_state_digest,"commitAction":action,"proposal":proposed,"reviewDigest":hash(&json!({"action":action,"stateDigest":p.expected_state_digest,"proposal":proposed})),"canCommit":status["actions"][if legacy{"canAdopt"}else{"canSave"}],"willFreeze":legacy,"reasonCodes":status["actions"]["reasonCodes"],"limits":LIMITS}))}
fn derive_preview(s:&Snapshot,p:&Derive,work:&[Value])->Result<Value,String>{
    let a=assessment(s,&p.assessment_id)?;if scope_targets::is_combined(s,&a.id){return Err(err("scope_combined_action_required"));}let status=status_from(s,&a.id,work)?;expected(&status,&p.expected_state_digest)?;let title=text(&p.title,160)?;
    let copies=p.copy_fields.iter().map(String::as_str).collect::<HashSet<_>>();if copies.len()!=p.copy_fields.len()||copies.iter().any(|s|!["objective","methods","window"].contains(s)){return Err(err("scope_invalid_copy"));}
    let r=binding(s,a)?.map(|b|current(s,b)).transpose()?.flatten().filter(|r|r.review.is_some());
    let objective=if copies.contains("objective"){if p.objective.is_some(){return Err(err("scope_invalid_copy"));}a.objective.clone()}else{p.objective.clone().ok_or_else(||err("scope_invalid_copy"))?};
    let methods=if copies.contains("methods"){if p.methods.is_some(){return Err(err("scope_invalid_copy"));}r.ok_or_else(||err("scope_legacy_copy_unavailable"))?.body.methods.clone()}else{p.methods.clone().ok_or_else(||err("scope_invalid_copy"))?};
    let window=if copies.contains("window"){if p.window.is_some(){return Err(err("scope_invalid_copy"));}r.ok_or_else(||err("scope_legacy_copy_unavailable"))?.body.window.clone()}else{p.window.clone().ok_or_else(||err("scope_invalid_copy"))?};
    let proposal=json!({"projectId":a.project_id,"title":title,"body":body(a,&objective,&methods,&window)?,"copyFields":p.copy_fields});
    Ok(json!({"sourceAssessmentId":a.id,"expectedStateDigest":p.expected_state_digest,"reviewDigest":hash(&json!({"action":"assessment.deriveCreate","stateDigest":p.expected_state_digest,"proposal":proposal})),"proposal":proposal,"canCreate":status["actions"]["canDerive"],"reasonCodes":status["actions"]["reasonCodes"],"limits":LIMITS}))
}
pub(crate) fn simple_creation_id(project_id: &str, key: &str) -> String {
    scope_targets::derived_id("openkoda.single-assessment.v1", &[project_id, key])
}
pub(crate) fn replay(s:&Snapshot,action:&str,aid:&str,key:&str,request:&str)->Result<Option<ScopeResult>,String>{core::valid_id(key)?;if let Some(r)=s.scope_receipts.iter().find(|r|r.action==action&&r.assessment_id==aid&&r.idempotency_key==key){if r.request_digest!=request{return Err(err("scope_idempotency_conflict"));}return Ok(Some(r.result.clone()));}if s.scope_receipts.iter().filter(|r|r.assessment_id==aid).count()>=1000{return Err(err("scope_limit_reached"));}Ok(None)}
pub(crate) fn receipt(s:&mut Snapshot,action:&str,aid:&str,key:&str,request:&str,result:ScopeResult){s.scope_receipts.push(ScopeReceipt{idempotency_key:key.into(),action:action.into(),assessment_id:aid.into(),request_digest:request.into(),created_at:model::now(),result});}
pub(crate) fn scope_result(s:&Snapshot,aid:&str)->Result<ScopeResult,String>{let a=assessment(s,aid)?;let b=binding(s,a)?.ok_or_else(||err("scope_integrity_error"))?;Ok(ScopeResult{assessment_id:aid.into(),scope_id:b.id.clone(),revision_id:b.current_revision_id.clone(),conversation_source:None})}
pub fn handle(action:&str,payload:Value,store:&Store,redact:impl Fn(&str)->String)->Result<Value,String>{
    if combined_supports(action){return handle_combined(action,payload,store);}
    if !payload.is_object()||payload.to_string().len()>16384{return Err(err("scope_invalid_input"));}
    match action{
        "assessment.scopeOptions"=>{let p:ProjectInput=decode(payload)?;options(&store.snapshot()?,&p.project_id)},
        "assessment.scopeStatus"=>{let p:IdInput=decode(payload)?;status(store,&p.assessment_id)},
        "assessment.scopePreview"=>{let p:Proposal=decode(payload)?;preview(&store.snapshot()?,&p,&admitted_work(store,&p.assessment_id)?)},
        "assessment.derivePreview"=>{let p:Derive=decode(payload)?;derive_preview(&store.snapshot()?,&p,&admitted_work(store,&p.assessment_id)?)},
        "assessment.scopeSave"|"assessment.scopeAdopt"=>{
            let request=hash(&payload);let p:Commit=decode(payload)?;if !p.acknowledge_scope{return Err(err("scope_ack_required"));}let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;let work=admitted_work(store,&p.assessment_id)?;
            let (result,replayed)=store.mutate(|s|{if let Some(r)=replay(s,action,&p.assessment_id,&p.idempotency_key,&request)?{return Ok((r,true));}
                let input=Proposal{assessment_id:p.assessment_id.clone(),expected_state_digest:p.expected_state_digest.clone(),objective:p.objective.clone(),methods:p.methods.clone(),window:p.window.clone()};let v=preview(s,&input,&work)?;
                if v["commitAction"]!=action||v["canCommit"]!=true{return Err(err("scope_frozen"));}if v["reviewDigest"]!=p.review_digest{return Err(err("scope_review_changed"));}
                let proposed:ScopeBody=decode(v["proposal"].clone())?;let a=assessment(s,&p.assessment_id)?.clone();
                if a.scope_id.is_none(){let b=AssessmentScope{id:model::id(),assessment_id:a.id.clone(),generation:1,current_revision_id:None,frozen:None,revocation:None,origin:None};s.assessments.iter_mut().find(|v|v.id==a.id).unwrap().scope_id=Some(b.id.clone());s.assessment_scopes.push(b);}
                let b=binding(s,assessment(s,&a.id)?)?.unwrap();let previous=current(s,b)?.cloned();if previous.as_ref().is_some_and(|r|r.number>=100){return Err(err("scope_limit_reached"));}
                let r=revision(&a.id,proposed,previous.as_ref(),true);let rid=r.id.clone();s.assessments.iter_mut().find(|v|v.id==a.id).unwrap().objective=r.body.objective.clone();s.scope_revisions.push(r);let b=s.assessment_scopes.iter_mut().find(|b|b.assessment_id==a.id).unwrap();b.current_revision_id=Some(rid);b.generation+=1;
                if action=="assessment.scopeAdopt"{freeze(s,&a.id,action,"legacy_adoption")?;}
                let result=scope_result(s,&a.id)?;receipt(s,action,&a.id,&p.idempotency_key,&request,result.clone());Ok((result,false))
            })?;Ok(json!({"saved":!replayed,"replayed":replayed,"revisionId":result.revision_id,"scopeStatus":status(store,&result.assessment_id)?}))
        },
        "assessment.deriveCreate"=>{
            let request=hash(&payload);let p:DeriveCommit=decode(payload)?;if !p.acknowledge_new_assessment{return Err(err("scope_ack_required"));}let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;let work=admitted_work(store,&p.assessment_id)?;
            let(result,replayed)=store.mutate(|s|{if let Some(r)=replay(s,action,&p.assessment_id,&p.idempotency_key,&request)?{return Ok((r,true));}let input=Derive{assessment_id:p.assessment_id.clone(),expected_state_digest:p.expected_state_digest.clone(),title:p.title.clone(),copy_fields:p.copy_fields.clone(),objective:p.objective.clone(),methods:p.methods.clone(),window:p.window.clone()};let v=derive_preview(s,&input,&work)?;
                if v["canCreate"]!=true{return Err(err("assessment_work_active"));}if v["reviewDigest"]!=p.review_digest{return Err(err("scope_review_changed"));}let source=assessment(s,&p.assessment_id)?.clone();let proposed:ScopeBody=decode(v["proposal"]["body"].clone())?;let source_ref=current_ref(s,&source.id)?;let at=model::now();
                let mut a=Assessment{id:model::id(),project_id:source.project_id.clone(),title:v["proposal"]["title"].as_str().unwrap().into(),kind:source.kind.clone(),target:source.target.clone(),objective:proposed.objective.clone(),status:"draft".into(),created_at:at.clone(),updated_at:at.clone(),events:vec![model::event(format!("Rascunho derivado da avaliação {}; configuração ainda não revisada e nenhuma prova ou autorização copiada.",source.id),"info")],evidence:vec![],coverage:vec![],summary:String::new(),scope_id:None,ai_selection:None};
                initialize(s,&mut a,None)?;let r=revision(&a.id,proposed,None,false);let b=s.assessment_scopes.iter_mut().find(|b|b.assessment_id==a.id).unwrap();b.current_revision_id=Some(r.id.clone());b.origin=Some(ScopeOrigin{source_assessment_id:source.id.clone(),source_legacy_digest:if source_ref.is_none(){Some(hash(&json!({"id":source.id,"projectId":source.project_id,"objective":source.objective,"kind":source.kind,"target":source.target})))}else{None},source_scope:source_ref,copy_fields:p.copy_fields.clone(),created_at:at});s.scope_revisions.push(r);let aid=a.id.clone();s.assessments.push(a);let result=scope_result(s,&aid)?;receipt(s,action,&source.id,&p.idempotency_key,&request,result.clone());Ok((result,false))
            })?;Ok(json!({"created":!replayed,"replayed":replayed,"assessmentId":result.assessment_id,"scopeStatus":status(store,&result.assessment_id)?}))
        },
        "assessment.scopeRevoke"=>{
            let request=hash(&payload);let p:Revoke=decode(payload)?;if !p.acknowledge_revocation{return Err(err("scope_ack_required"));}let rationale=redact(&text(&p.rationale,2000)?);if rationale.len()>2000{return Err(err("scope_invalid_input"));}let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;let work=admitted_work(store,&p.assessment_id)?;
            let(result,replayed)=store.mutate(|s|{if let Some(r)=replay(s,action,&p.assessment_id,&p.idempotency_key,&request)?{return Ok((r,true));}let public=status_from(s,&p.assessment_id,&work)?;expected(&public,&p.expected_state_digest)?;if public["actions"]["canRevoke"]!=true{return Err(err("scope_frozen"));}let b=s.assessment_scopes.iter_mut().find(|b|b.assessment_id==p.assessment_id).unwrap();b.revocation=Some(ScopeRevocation{id:model::id(),revision_id:b.current_revision_id.clone().unwrap(),at:model::now(),author:"local-user".into(),rationale:rationale.clone(),prevent_new_admissions:true});b.generation+=1;s.assessments.iter_mut().find(|a|a.id==p.assessment_id).unwrap().events.push(model::event("Escopo revogado para novas admissões. Trabalho já admitido não foi declarado encerrado.","warning"));let result=scope_result(s,&p.assessment_id)?;receipt(s,action,&p.assessment_id,&p.idempotency_key,&request,result.clone());Ok((result,false))})?;
            Ok(json!({"revoked":!replayed,"replayed":replayed,"scopeStatus":status(store,&result.assessment_id)?,"admittedWork":admitted_work(store,&result.assessment_id)?,"cancellationRequested":false,"limits":LIMITS}))
        },_=>Err(err("scope_unknown_action"))
    }
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CombinedCreate {#[serde(default,skip_serializing_if="Option::is_none")]ai_selection:Option<crate::ai_provider::AiSelection>,project_id:String,title:String,objective:String,window:ScopeWindow,selection:TargetSetInput,project_catalog_digest:String,idempotency_key:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CombinedEdit {assessment_id:String,expected_state_digest:String,project_catalog_digest:String,objective:String,window:ScopeWindow,selection:TargetSetInput}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CombinedDerive {assessment_id:String,expected_state_digest:String,project_catalog_digest:String,idempotency_key:String,title:String,copy_fields:Vec<String>,#[serde(default,skip_serializing_if="Option::is_none")]objective:Option<String>,#[serde(default,skip_serializing_if="Option::is_none")]window:Option<ScopeWindow>,#[serde(default,skip_serializing_if="Option::is_none")]selection:Option<TargetSetInput>}
fn combined_supports(action:&str)->bool{matches!(action,"assessment.combinedOptions"|"assessment.combinedPreview"|"assessment.combinedCreate"|"assessment.combinedScopePreview"|"assessment.combinedScopeSave"|"assessment.combinedDerivePreview"|"assessment.combinedDeriveCreate")}
fn combined_body(p:&model::Project,aid:&str,objective:&str,window:&ScopeWindow,selection:&TargetSetInput)->Result<ScopeBody,String>{let set=scope_targets::normalize_target_set(p,aid,selection)?;let primary=set.entries.iter().find(|t|t.entry_id==set.primary_entry_id).ok_or_else(||err("scope_primary_not_selected"))?;Ok(ScopeBody{objective:text(objective,4000)?,kind:primary.kind.clone(),target:primary.target.clone(),methods:scope_targets::aggregate_methods(&set),window:normalize_window(window)?,target_set:Some(set)})}
fn catalog_expected(p:&model::Project,wanted:&str)->Result<(),String>{if scope_targets::catalog_digest(p)!=wanted{return Err(err("scope_catalog_changed"));}if p.archived{return Err(err("project_archived"));}Ok(())}
fn combined_create_preview(s:&Snapshot,p:&CombinedCreate)->Result<Value,String>{
    core::valid_id(&p.project_id)?;core::valid_id(&p.idempotency_key)?;let project=s.projects.iter().find(|v|v.id==p.project_id).ok_or_else(||err("scope_project_missing"))?;catalog_expected(project,&p.project_catalog_digest)?;
    let aid=scope_targets::derived_id("openkoda.combined-assessment.v1",&[&p.project_id,&p.idempotency_key]);let proposed=combined_body(project,&aid,&p.objective,&p.window,&p.selection)?;let title=text(&p.title,160)?;
    let mut proposal=json!({"projectId":p.project_id,"title":title,"body":proposed});
    if let Some(selection)=&p.ai_selection{selection.validate()?;proposal["aiSelection"]=json!(selection);}
    let review=hash(&json!({"action":"assessment.combinedCreate","assessmentId":aid,"catalogDigest":p.project_catalog_digest,"proposal":proposal}));
    Ok(json!({"assessmentId":aid,"proposal":proposal,"reviewDigest":review,"canCommit":!s.assessments.iter().any(|a|a.id==aid),"reasonCodes":[],"limits":scope_targets::LIMITS}))
}
fn combined_edit_preview(s:&Snapshot,p:&CombinedEdit,work:&[Value])->Result<Value,String>{
    let a=assessment(s,&p.assessment_id)?;if !scope_targets::is_combined(s,&a.id){return Err(err("scope_combined_derivation_required"));}let status=status_from(s,&a.id,work)?;expected(&status,&p.expected_state_digest)?;
    let project=s.projects.iter().find(|v|v.id==a.project_id).ok_or_else(||err("scope_project_missing"))?;catalog_expected(project,&p.project_catalog_digest)?;
    let proposed=combined_body(project,&a.id,&p.objective,&p.window,&p.selection)?;
    Ok(json!({"assessmentId":a.id,"expectedStateDigest":p.expected_state_digest,"proposal":proposed,"reviewDigest":hash(&json!({"action":"assessment.combinedScopeSave","stateDigest":p.expected_state_digest,"catalogDigest":p.project_catalog_digest,"proposal":proposed})),"canCommit":status["actions"]["canSave"],"reasonCodes":status["actions"]["reasonCodes"],"limits":scope_targets::LIMITS}))
}
fn combined_derive_preview(s:&Snapshot,p:&CombinedDerive,work:&[Value])->Result<Value,String>{
    let a=assessment(s,&p.assessment_id)?;let status=status_from(s,&a.id,work)?;expected(&status,&p.expected_state_digest)?;core::valid_id(&p.idempotency_key)?;
    let project=s.projects.iter().find(|v|v.id==a.project_id).ok_or_else(||err("scope_project_missing"))?;catalog_expected(project,&p.project_catalog_digest)?;
    let copies=p.copy_fields.iter().map(String::as_str).collect::<HashSet<_>>();if copies.len()!=p.copy_fields.len()||copies.iter().any(|f|!["objective","window","targets","exclusions"].contains(f)){return Err(err("scope_invalid_copy"));}
    let r=binding(s,a)?.map(|b|current(s,b)).transpose()?.flatten().filter(|r|r.review.is_some());
    let objective=if copies.contains("objective"){if p.objective.is_some(){return Err(err("scope_invalid_copy"));}a.objective.clone()}else{p.objective.clone().ok_or_else(||err("scope_invalid_copy"))?};
    let window=if copies.contains("window"){if p.window.is_some(){return Err(err("scope_invalid_copy"));}r.ok_or_else(||err("scope_legacy_copy_unavailable"))?.body.window.clone()}else{p.window.clone().ok_or_else(||err("scope_invalid_copy"))?};
    let mut selection=p.selection.clone().unwrap_or(TargetSetInput{primary_project_target_id:String::new(),entries:vec![],common_methods:vec![],excluded_project_targets:vec![],local_exclusions:vec![],connection_exclusions:vec![]});
    if copies.contains("targets")||copies.contains("exclusions"){
        let source=scope_targets::selection_from_set(r.and_then(|r|r.body.target_set.as_ref()).ok_or_else(||err("scope_legacy_copy_unavailable"))?);
        if copies.contains("targets"){if !selection.entries.is_empty()||!selection.primary_project_target_id.is_empty()||!selection.common_methods.is_empty(){return Err(err("scope_invalid_copy"));}selection.entries=source.entries;selection.primary_project_target_id=source.primary_project_target_id;selection.common_methods=source.common_methods;}
        if copies.contains("exclusions"){if !selection.local_exclusions.is_empty()||!selection.connection_exclusions.is_empty()||!selection.excluded_project_targets.is_empty(){return Err(err("scope_invalid_copy"));}selection.local_exclusions=source.local_exclusions;selection.connection_exclusions=source.connection_exclusions;selection.excluded_project_targets=source.excluded_project_targets;}
    }
    let aid=scope_targets::derived_id("openkoda.combined-derived.v1",&[&a.id,&p.idempotency_key]);let body=combined_body(project,&aid,&objective,&window,&selection)?;
    let proposal=json!({"projectId":a.project_id,"title":text(&p.title,160)?,"body":body,"copyFields":p.copy_fields});
    Ok(json!({"sourceAssessmentId":a.id,"assessmentId":aid,"expectedStateDigest":p.expected_state_digest,"proposal":proposal,"reviewDigest":hash(&json!({"action":"assessment.combinedDeriveCreate","assessmentId":aid,"stateDigest":p.expected_state_digest,"catalogDigest":p.project_catalog_digest,"proposal":proposal})),"canCreate":status["actions"]["canDerive"]==true&&!s.assessments.iter().any(|a|a.id==aid),"reasonCodes":status["actions"]["reasonCodes"],"limits":scope_targets::LIMITS}))
}
fn insert_combined(s:&mut Snapshot,aid:&str,pid:&str,title:&str,body:ScopeBody,origin:Option<ScopeOrigin>,selection:Option<crate::ai_provider::AiSelection>)->Result<ScopeResult,String>{
    if s.assessments.iter().any(|a|a.id==aid){return Err(err("scope_idempotency_conflict"));}
    let at=model::now();let reviewed=origin.is_none();let r=revision(aid,body.clone(),None,reviewed);let b=AssessmentScope{id:model::id(),assessment_id:aid.into(),generation:1,current_revision_id:Some(r.id.clone()),frozen:None,revocation:None,origin};
    let a=Assessment{id:aid.into(),project_id:pid.into(),title:title.into(),kind:body.kind,target:body.target,objective:body.objective,status:"draft".into(),created_at:at.clone(),updated_at:at,events:vec![model::event(if reviewed{"Escopo combinado revisado. Nenhum plano, aprovação ou operação foi criado."}else{"Rascunho combinado derivado. Configuração ainda não revisada; nenhuma prova ou autorização herdada."},"info")],evidence:vec![],coverage:vec![],summary:String::new(),ai_selection:selection,scope_id:Some(b.id.clone())};s.assessments.push(a);s.assessment_scopes.push(b);s.scope_revisions.push(r);scope_result(s,aid)
}
fn commit_fields(payload:&mut Value,ack:&str,has_key_in_proposal:bool)->Result<(String,String),String>{
    let object=payload.as_object_mut().ok_or_else(||err("scope_invalid_input"))?;
    if object.remove(ack)!=Some(Value::Bool(true)){return Err(err("scope_ack_required"));}
    let review=object.remove("reviewDigest").and_then(|v|v.as_str().map(str::to_owned)).ok_or_else(||err("scope_review_changed"))?;
    let key=if has_key_in_proposal{object.get("idempotencyKey").cloned()}else{object.remove("idempotencyKey")}.and_then(|v|v.as_str().map(str::to_owned)).ok_or_else(||err("scope_invalid_input"))?;core::valid_id(&key)?;Ok((review,key))
}
pub(crate) const COMBINED_DERIVE_CREATE: &str = "assessment.combinedDeriveCreate";

pub(crate) fn dispatch_combined_derivation(
    payload: Value,
    store: &Store,
    environment: &std::sync::RwLock<()>,
    authorize: impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    let commit = CombinedDerivationCommit::parse(payload)?;
    {
        // Committed retries share a lease with each other and with workers.
        let _read = environment.try_read().map_err(|_| err("scope_busy"))?;
        authorize()?;
        if let Some(result) = commit.receipt(&store.snapshot()?)? {
            return derivation_response(store, &result, true);
        }
    }
    let _write = environment.try_write().map_err(|_| err("scope_busy"))?;
    // The environment and the receipt may have changed across the lease switch.
    authorize()?;
    commit.create(store)
}

struct CombinedDerivationCommit {
    proposal: CombinedDerive,
    review: String,
    digest: String,
}

impl CombinedDerivationCommit {
    fn parse(mut payload: Value) -> Result<Self, String> {
        if !payload.is_object() || payload.to_string().len() > 65536 {
            return Err(err("scope_invalid_input"));
        }
        let digest = hash(&payload);
        let (review, _) = commit_fields(&mut payload, "acknowledgeNewAssessment", true)?;
        Ok(Self { proposal: decode(payload)?, review, digest })
    }

    fn receipt(&self, snapshot: &Snapshot) -> Result<Option<ScopeResult>, String> {
        replay(snapshot, COMBINED_DERIVE_CREATE, &self.proposal.assessment_id,
               &self.proposal.idempotency_key, &self.digest)
    }

    fn create(self, store: &Store) -> Result<Value, String> {
        let input = &self.proposal;
        let source_id = &input.assessment_id;
        let _scope = store.scope_gate.lock().map_err(|_| err("scope_busy"))?;
        let work = admitted_work(store, source_id)?;
        let (result, replayed) = store.mutate(|snapshot| {
            if let Some(result) = self.receipt(snapshot)? {
                return Ok((result, true));
            }
            let preview = combined_derive_preview(snapshot, input, &work)?;
            if preview["canCreate"] != true {
                return Err(err("assessment_work_active"));
            }
            if preview["reviewDigest"] != self.review {
                return Err(err("scope_review_changed"));
            }
            let source = assessment(snapshot, source_id)?;
            let source_scope = current_ref(snapshot, source_id)?;
            let source_legacy_digest = source_scope.is_none().then(|| hash(&json!({
                "id":source.id,"projectId":source.project_id,"objective":source.objective,
                "kind":source.kind,"target":source.target
            })));
            let origin = ScopeOrigin {
                source_assessment_id: source_id.clone(),
                source_legacy_digest,
                source_scope,
                copy_fields: input.copy_fields.clone(),
                created_at: model::now(),
            };
            let project_id = source.project_id.clone();
            let body = decode(preview["proposal"]["body"].clone())?;
            let result = insert_combined(
                snapshot, preview["assessmentId"].as_str().unwrap(), &project_id,
                preview["proposal"]["title"].as_str().unwrap(), body, Some(origin), None,
            )?;
            receipt(snapshot, COMBINED_DERIVE_CREATE, source_id, &input.idempotency_key, &self.digest, result.clone());
            Ok((result, false))
        })?;
        derivation_response(store, &result, replayed)
    }
}

fn derivation_response(store: &Store, result: &ScopeResult, replayed: bool) -> Result<Value, String> {
    Ok(json!({"created":!replayed,"replayed":replayed,"assessmentId":result.assessment_id,
              "scopeStatus":status(store, &result.assessment_id)?}))
}

fn handle_combined(action:&str,mut payload:Value,store:&Store)->Result<Value,String>{
    if action == COMBINED_DERIVE_CREATE {
        return CombinedDerivationCommit::parse(payload)?.create(store);
    }
    if !payload.is_object()||payload.to_string().len()>65536{return Err(err("scope_invalid_input"));}
    match action{
        "assessment.combinedOptions"=>{let p:ProjectInput=decode(payload)?;scope_targets::catalog_options(&store.snapshot()?,&p.project_id)},
        "assessment.combinedPreview"=>{let p:CombinedCreate=decode(payload)?;combined_create_preview(&store.snapshot()?,&p)},
        "assessment.combinedScopePreview"=>{let p:CombinedEdit=decode(payload)?;combined_edit_preview(&store.snapshot()?,&p,&admitted_work(store,&p.assessment_id)?)},
        "assessment.combinedDerivePreview"=>{let p:CombinedDerive=decode(payload)?;combined_derive_preview(&store.snapshot()?,&p,&admitted_work(store,&p.assessment_id)?)},
        "assessment.combinedCreate"=>{
            let request=hash(&payload);let(review,key)=commit_fields(&mut payload,"acknowledgeScope",true)?;let p:CombinedCreate=decode(payload)?;let aid=scope_targets::derived_id("openkoda.combined-assessment.v1",&[&p.project_id,&p.idempotency_key]);let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;
            let(result,replayed)=store.mutate(|s|{if let Some(r)=replay(s,action,&aid,&key,&request)?{return Ok((r,true));}let v=combined_create_preview(s,&p)?;if v["canCommit"]!=true{return Err(err("scope_idempotency_conflict"));}if v["reviewDigest"]!=review{return Err(err("scope_review_changed"));}let body:ScopeBody=decode(v["proposal"]["body"].clone())?;let result=insert_combined(s,&aid,&p.project_id,v["proposal"]["title"].as_str().unwrap(),body,None,p.ai_selection.clone())?;receipt(s,action,&aid,&key,&request,result.clone());Ok((result,false))})?;
            Ok(json!({"created":!replayed,"replayed":replayed,"assessmentId":result.assessment_id,"scopeStatus":status(store,&result.assessment_id)?}))
        },
        "assessment.combinedScopeSave"=>{
            let request=hash(&payload);let(review,key)=commit_fields(&mut payload,"acknowledgeScope",false)?;let p:CombinedEdit=decode(payload)?;let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;let work=admitted_work(store,&p.assessment_id)?;
            let(result,replayed)=store.mutate(|s|{if let Some(r)=replay(s,action,&p.assessment_id,&key,&request)?{return Ok((r,true));}let v=combined_edit_preview(s,&p,&work)?;if v["canCommit"]!=true{return Err(err("scope_frozen"));}if v["reviewDigest"]!=review{return Err(err("scope_review_changed"));}let body:ScopeBody=decode(v["proposal"].clone())?;let a=assessment(s,&p.assessment_id)?;let b=binding(s,a)?.ok_or_else(||err("scope_integrity_error"))?;let previous=current(s,b)?.cloned();if previous.as_ref().is_some_and(|r|r.number>=100){return Err(err("scope_limit_reached"));}let r=revision(&a.id,body,previous.as_ref(),true);let aid=a.id.clone();let rid=r.id.clone();let a=s.assessments.iter_mut().find(|a|a.id==aid).unwrap();a.objective=r.body.objective.clone();a.kind=r.body.kind.clone();a.target=r.body.target.clone();a.updated_at=model::now();s.scope_revisions.push(r);let b=s.assessment_scopes.iter_mut().find(|b|b.assessment_id==aid).unwrap();b.current_revision_id=Some(rid);b.generation+=1;let result=scope_result(s,&aid)?;receipt(s,action,&aid,&key,&request,result.clone());Ok((result,false))})?;
            Ok(json!({"saved":!replayed,"replayed":replayed,"assessmentId":result.assessment_id,"revisionId":result.revision_id,"scopeStatus":status(store,&result.assessment_id)?}))
        },
        _=>Err(err("scope_unknown_action"))
    }
}

pub fn markdown_metadata(v:&Value,escape:impl Fn(&str)->String)->String{
    let mut out=String::from("## Escopo versionado\n\n");
    if v["legacyUnversioned"]==true{out.push_str("Escopo legado sem revisão versionada; provas antigas não recebem autorização retroativa.\n\n");}
    else if v["binding"]["currentRevisionId"].is_null(){out.push_str("Rascunho sem revisão de escopo. Nenhuma operação foi autorizada.\n\n");}
    else{out.push_str(&format!("Revisão atual: {}. Estado: {}.\n\n",escape(v["binding"]["currentRevisionId"].as_str().unwrap_or("")),if v["binding"]["revocation"].is_object(){"revogado"}else if v["binding"]["frozen"].is_object(){"congelado"}else{"rascunho"}));}
    if let Some(origin)=v["binding"]["origin"].as_object(){out.push_str(&format!("Derivada da avaliação {}. Configuração copiada não herda evidências ou aprovações.\n\n",escape(origin["sourceAssessmentId"].as_str().unwrap_or(""))));}
    if let Some(revisions)=v["revisions"].as_array(){for r in revisions{out.push_str(&format!("Revisão {}: {}.\n\n",r["number"],if r["review"].is_null(){"configuração pré-preenchida, não revisada"}else{"revisão humana registrada; não é aprovação operacional"}));}}
    if let Some(revisions)=v["revisions"].as_array(){for r in revisions{out.push_str(&format!("- Revisão {} · {} · SHA-256: {}\n  Objetivo: {}\n  Métodos: {}\n  Janela para iniciar operações (UTC): {} até {}\n",r["number"],escape(r["id"].as_str().unwrap_or("")),escape(r["digest"].as_str().unwrap_or("")),escape(r["body"]["objective"].as_str().unwrap_or("")),escape(&r["body"]["methods"].to_string()),escape(r["body"]["window"]["startsAt"].as_str().unwrap_or("sem início")),escape(r["body"]["window"]["endsAt"].as_str().unwrap_or("sem fim"))));}}
    if let Some(revisions)=v["revisions"].as_array(){for r in revisions{if let Some(set)=r["body"]["targetSet"].as_object(){
        out.push_str(&format!("\n### Entradas e exclusões da revisão {}\n\nEntrada principal {} serve apenas à apresentação; nenhuma operação recebe alvo implícito.\n\n",r["number"],escape(set.get("primaryEntryId").and_then(Value::as_str).unwrap_or(""))));
        if let Some(entries)=set.get("entries").and_then(Value::as_array){for e in entries{out.push_str(&format!("- {} · {} · entrada {} · alvo {} · métodos {}\n",escape(e["label"].as_str().unwrap_or("")),escape(e["kind"].as_str().unwrap_or("")),escape(e["entryId"].as_str().unwrap_or("")),escape(e["target"].as_str().unwrap_or("")),escape(&e["methods"].to_string())));}}
        out.push_str(&format!("\nMétodos comuns: {}.\n",escape(&set.get("commonMethods").cloned().unwrap_or(json!([])).to_string())));
        if let Some(rules)=set.get("localExclusions").and_then(Value::as_array){for rule in rules{out.push_str(&format!("- Exclusão local: entrada {} · caminho {} · regra {} · motivo {}\n",escape(rule["entryId"].as_str().unwrap_or("")),escape(rule["relativePath"].as_str().unwrap_or("")),escape(rule["match"].as_str().unwrap_or("")),escape(rule["reason"].as_str().unwrap_or(""))));}}
        if let Some(rules)=set.get("connectionExclusions").and_then(Value::as_array){for rule in rules{out.push_str(&format!("- Exclusão de conexão para toda a avaliação: {} · portas {} · motivo {}\n",escape(rule["address"].as_str().unwrap_or("")),if rule["ports"].is_null(){"todas".into()}else{escape(&rule["ports"].to_string())},escape(rule["reason"].as_str().unwrap_or(""))));}}
        if let Some(rules)=set.get("excludedProjectTargets").and_then(Value::as_array){for rule in rules{out.push_str(&format!("- Alvo explicitamente excluído: {} · {} · motivo {}\n",escape(rule["label"].as_str().unwrap_or("")),escape(rule["target"].as_str().unwrap_or("")),escape(rule["reason"].as_str().unwrap_or(""))));}}
        out.push_str("\nAlvos não selecionados não recebem autoridade. Execução de uma entrada não conclui a avaliação das demais.\n");
    }}}
    if v["binding"]["revocation"].is_object(){out.push_str("\nEscopo revogado para novas admissões. O histórico permanece preservado.\n");}
    out.push_str("\nA revisão do escopo não aprova planos, pedidos pagos ou execução. Revogação não desfaz trabalho já admitido.\n\n");out
}
pub fn dispatch_deadline(store:&Store,scope:&ScopeRef,max_seconds:u64)->Result<u64,String>{
    let s=store.snapshot()?;let r=s.scope_revisions.iter().find(|r|r.id==scope.revision_id&&r.assessment_id==scope.assessment_id&&r.digest==scope.digest).ok_or_else(||err("scope_integrity_error"))?;
    let now=chrono::Utc::now().timestamp();let deadline=r.body.window.ends_at.as_ref().map(|end|stamp(end).map(|v|v.timestamp())).transpose()?.unwrap_or(now+max_seconds as i64).min(now+max_seconds as i64);
    if deadline<=now{return Err(err("scope_expired"));}Ok(deadline as u64)
}
pub fn report_metadata(s:&Snapshot,aid:&str)->Result<Value,String>{let a=assessment(s,aid)?;let b=binding(s,a)?;let mut report=json!({"schema":"openkoda.scope-report.v1","assessmentId":aid,"legacyUnversioned":b.is_none(),"binding":b,"revisions":s.scope_revisions.iter().filter(|r|r.assessment_id==aid).collect::<Vec<_>>(),"limits":if scope_targets::is_combined(s,aid){scope_targets::LIMITS}else{LIMITS}});if scope_targets::is_combined(s,aid){report["targetCoverage"]=json!(target_coverage(s,aid));}Ok(report)}

#[cfg(test)]
pub(crate) fn test_creation(store:&Store,project_id:&str,methods:&[&str])->Value{
    // Test fixture uses the public read contract and explicit methods, never an
    // authority bypass. The production create path validates this same payload.
    let v=handle("assessment.scopeOptions",json!({"projectId":project_id}),store,str::to_owned).unwrap();json!({"methods":methods,"window":{"startsAt":null,"endsAt":null},"projectDigest":v["projectDigest"],"catalogVersion":v["catalogVersion"],"acknowledgeScope":true})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    struct Fixture{_dir:tempfile::TempDir,store:Store,project:Value,assessment:String}
    fn call(store:&Store,action:&str,payload:Value)->Result<Value,String>{if supports(action){handle(action,payload,store,str::to_owned)}else{store.handle(core::Request{action:action.into(),payload})}}
    fn fixture(methods:Option<&[&str]>,window:Option<Value>)->Fixture{
        let dir=tempfile::tempdir().unwrap();let target=dir.path().join("source");std::fs::create_dir(&target).unwrap();std::fs::write(target.join("owned.txt"),"fixture").unwrap();
        let store=Store::open(dir.path().join("profile")).unwrap();let project=call(&store,"project.create",json!({"name":"Scope fixture","kind":"files","target":target})).unwrap();
        let mut input=json!({"projectId":project["id"],"title":"Scope fixture","kind":"files","target":project["target"],"objective":"Observe owned fixture"});
        if let Some(methods)=methods{input["scope"]=test_creation(&store,project["id"].as_str().unwrap(),methods);if let Some(window)=window{input["scope"]["window"]=window;}}
        let assessment=call(&store,"assessment.create",input).unwrap()["id"].as_str().unwrap().to_owned();Fixture{_dir:dir,store,project,assessment}
    }
    fn public(f:&Fixture)->Value{status(&f.store,&f.assessment).unwrap()}
    fn proposal(f:&Fixture,objective:&str,methods:&[&str])->Value{json!({"assessmentId":f.assessment,"expectedStateDigest":public(f)["stateDigest"],"objective":objective,"methods":methods,"window":{"startsAt":null,"endsAt":null}})}
    fn commit(store:&Store,mut p:Value)->(String,Value){let preview=call(store,"assessment.scopePreview",p.clone()).unwrap();let action=preview["commitAction"].as_str().unwrap().to_owned();p["reviewDigest"]=preview["reviewDigest"].clone();p["idempotencyKey"]=json!(model::id());p["acknowledgeScope"]=json!(true);(action,p)}
    fn import(f:&Fixture)->Value{call(&f.store,"evidence.import",json!({"assessmentId":f.assessment,"name":"owned","sourceDescription":"human fixture","content":"Synthetic observation","acknowledgeUnverifiedSource":true})).unwrap()}
    fn revoke(f:&Fixture)->Value{call(&f.store,"assessment.scopeRevoke",json!({"assessmentId":f.assessment,"expectedStateDigest":public(f)["stateDigest"],"idempotencyKey":model::id(),"rationale":"End synthetic scope","acknowledgeRevocation":true})).unwrap()}

    #[test]fn atomic_creation_validates_catalog_project_ack_methods_and_window(){
        let f=fixture(None,None);let base=json!({"projectId":f.project["id"],"title":"Explicit","kind":"files","target":f.project["target"],"objective":"Reviewed creation","scope":test_creation(&f.store,f.project["id"].as_str().unwrap(),&["local.inventory.v1"])});
        for (field,value) in [("acknowledgeScope",json!(false)),("catalogVersion",json!("future")),("projectDigest",json!("forged")),("methods",json!(["http.headers.v1"])),("methods",json!(["source.capture.v1"])),("methods",json!(["local.inventory.v1","local.inventory.v1"])),("window",json!({"startsAt":"2026-01-02T00:00:00Z","endsAt":"2026-01-01T00:00:00Z"}))]{let mut p=base.clone();p["scope"][field]=value;let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();assert!(call(&f.store,"assessment.create",p).is_err());assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);}
        let created=call(&f.store,"assessment.create",base).unwrap();let s=status(&f.store,created["id"].as_str().unwrap()).unwrap();assert_eq!(s["phase"],"draft");assert!(s["frozen"].is_null());assert!(f.store.snapshot().unwrap().plans.is_empty());
    }
    #[test]fn unreviewed_denies_before_target_read_and_import_without_mutation(){
        let f=fixture(None,None);std::fs::remove_file(f._dir.path().join("source/owned.txt")).unwrap();std::fs::remove_dir(f._dir.path().join("source")).unwrap();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
        assert!(Executor::default().handle("assessment.plan",json!({"id":f.assessment}),&f.store).unwrap_err().contains("scope_unreviewed"));
        assert!(call(&f.store,"evidence.import",json!({"assessmentId":f.assessment,"name":"input","sourceDescription":"human","content":"data","acknowledgeUnverifiedSource":true})).unwrap_err().contains("scope_unreviewed"));assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert_eq!(public(&f)["actions"]["canImportEvidence"],false);
    }
    #[test]fn scope_save_is_reviewed_stale_safe_and_exactly_idempotent(){
        let f=fixture(None,None);let (action,p)=commit(&f.store,proposal(&f,"Updated objective",&["local.inventory.v1"]));let mut denied=p.clone();denied["acknowledgeScope"]=json!(false);assert!(call(&f.store,&action,denied).is_err());
        let saved=call(&f.store,&action,p.clone()).unwrap();assert_eq!(saved["saved"],true);assert_eq!(public(&f)["revision"]["body"]["objective"],"Updated objective");
        let replay=call(&f.store,&action,p.clone()).unwrap();assert_eq!(replay["replayed"],true);assert_eq!(replay["revisionId"],saved["revisionId"]);assert_eq!(f.store.snapshot().unwrap().scope_revisions.len(),1);
        let mut conflict=p.clone();conflict["objective"]=json!("Changed replay");assert!(call(&f.store,&action,conflict).unwrap_err().contains("scope_idempotency_conflict"));let mut stale=p;stale["idempotencyKey"]=json!(model::id());assert!(call(&f.store,&action,stale).unwrap_err().contains("scope_state_changed"));
    }
    #[test]fn first_local_input_freezes_durably_and_objective_cannot_be_rewritten(){
        let f=fixture(Some(&[]),None);let original=public(&f)["revision"].clone();import(&f);assert_eq!(public(&f)["phase"],"frozen");assert_eq!(public(&f)["revision"],original);
        let preview=call(&f.store,"assessment.scopePreview",proposal(&f,"Rewritten objective",&[])).unwrap();assert_eq!(preview["canCommit"],false);assert!(preview["reasonCodes"].as_array().unwrap().contains(&json!("scope_frozen")));
        f.store.mutate(|s|{s.assessments[0].evidence.clear();Ok(())}).unwrap();assert_eq!(public(&f)["phase"],"frozen");let path=f.store.root.clone();drop(f.store);let restored=Store::open(path).unwrap();assert_eq!(status(&restored,&f.assessment).unwrap()["phase"],"frozen");
    }
    #[test]fn revision_history_is_append_only_before_first_work(){
        let f=fixture(Some(&["local.inventory.v1"]),None);let old=public(&f)["revision"].clone();let(action,p)=commit(&f.store,proposal(&f,"Second review",&[]));call(&f.store,&action,p).unwrap();let s=f.store.snapshot().unwrap();assert_eq!(s.scope_revisions.len(),2);assert_eq!(serde_json::to_value(&s.scope_revisions[0]).unwrap(),old);assert_eq!(s.scope_revisions[1].previous_revision_id.as_deref(),old["id"].as_str());
        assert!(f.store.mutate(|s|{s.scope_revisions[0].body.objective="tampered".into();Ok(())}).is_err());
    }
    #[test]fn method_and_future_expired_window_deny_before_io_with_no_new_records(){
        let now=chrono::Utc::now();for(methods,window,code)in [(vec![],json!({"startsAt":null,"endsAt":null}),"scope_method_not_selected"),(vec!["local.inventory.v1"],json!({"startsAt":(now+chrono::Duration::hours(1)).to_rfc3339(),"endsAt":null}),"scope_not_yet_open"),(vec!["local.inventory.v1"],json!({"startsAt":null,"endsAt":(now-chrono::Duration::hours(1)).to_rfc3339()}),"scope_expired")]{let f=fixture(Some(&methods),Some(window));let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();assert!(Executor::default().handle("assessment.plan",json!({"id":f.assessment}),&f.store).unwrap_err().contains(code));assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);}
    }
    #[test]fn window_boundaries_and_effective_deadline_are_explicit(){
        let at=chrono::Utc::now();let w=ScopeWindow{starts_at:Some(at.to_rfc3339()),ends_at:Some((at+chrono::Duration::seconds(10)).to_rfc3339())};assert_eq!(window_state(&w,at).unwrap(),"open");assert_eq!(window_state(&w,at+chrono::Duration::seconds(10)).unwrap(),"expired");
        let f=fixture(Some(&["local.inventory.v1"]),Some(json!({"startsAt":null,"endsAt":(chrono::Utc::now()+chrono::Duration::seconds(20)).to_rfc3339()})));let executor=Executor::default();let p=executor.handle("assessment.plan",json!({"id":f.assessment}),&f.store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});executor.handle("plan.approve",d.clone(),&f.store).unwrap();let e=executor.handle("plan.run",d,&f.store).unwrap();assert_eq!(e["status"],"completed");assert_eq!(e["scopeAdmission"]["limitingFactor"],"scope_window_end");assert!(e["scopeAdmission"]["effectiveTimeoutMs"].as_u64().unwrap()<=20000);assert_eq!(e["scopeAdmission"]["scopeRef"],p["scopeRef"]);
        assert!(f.store.mutate(|s|{s.executions[0].scope_admission=None;Ok(())}).is_err());assert!(f.store.mutate(|s|{s.executions[0].scope_admission.as_mut().unwrap().effective_timeout_ms=u64::MAX;Ok(())}).is_err());
    }
    #[test]fn revocation_blocks_dispatch_but_plan_revoke_and_history_still_work(){
        let f=fixture(Some(&["local.inventory.v1"]),None);let executor=Executor::default();let p=executor.handle("assessment.plan",json!({"id":f.assessment}),&f.store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});executor.handle("plan.approve",d.clone(),&f.store).unwrap();let r=revoke(&f);assert_eq!(r["cancellationRequested"],false);assert_eq!(public(&f)["phase"],"revoked");assert_eq!(public(&f)["actions"]["canReviewStoredEvidence"],true);assert_eq!(public(&f)["actions"]["canImportEvidence"],false);
        assert!(executor.handle("plan.run",d.clone(),&f.store).unwrap_err().contains("scope_revoked"));assert!(f.store.snapshot().unwrap().executions.is_empty());executor.handle("plan.revoke",d,&f.store).unwrap();core::validate_snapshot(&f.store.snapshot().unwrap()).unwrap();assert_eq!(f.store.snapshot().unwrap().plans[0].status,"revoked");
    }
    #[test]fn derived_prefill_is_unreviewed_and_copies_no_evidence_plan_or_grant(){
        let f=fixture(Some(&["local.inventory.v1"]),None);import(&f);let mut p=json!({"assessmentId":f.assessment,"expectedStateDigest":public(&f)["stateDigest"],"title":"Derived","copyFields":["objective","methods","window"]});let preview=call(&f.store,"assessment.derivePreview",p.clone()).unwrap();p["reviewDigest"]=preview["reviewDigest"].clone();p["idempotencyKey"]=json!(model::id());p["acknowledgeNewAssessment"]=json!(true);let created=call(&f.store,"assessment.deriveCreate",p.clone()).unwrap();let aid=created["assessmentId"].as_str().unwrap();assert_ne!(aid,f.assessment);assert_eq!(created["scopeStatus"]["phase"],"unreviewed");assert!(created["scopeStatus"]["revision"]["review"].is_null());assert_eq!(created["scopeStatus"]["origin"]["sourceAssessmentId"],f.assessment);
        let state=f.store.snapshot().unwrap();let a=state.assessments.iter().find(|a|a.id==aid).unwrap();assert_eq!(a.project_id,f.project["id"]);assert!(a.evidence.is_empty());assert!(a.coverage.is_empty());assert!(!state.plans.iter().any(|p|p.assessment_id==aid));assert!(Executor::default().handle("assessment.plan",json!({"id":aid}),&f.store).unwrap_err().contains("scope_unreviewed"));assert!(call(&f.store,"report.export",json!({"id":aid})).unwrap_err().contains("data_review_required"));let markdown=markdown_metadata(&report_metadata(&f.store.snapshot().unwrap(),aid).unwrap(),str::to_owned);assert!(markdown.contains("configuração pré-preenchida, não revisada"));assert!(markdown.contains("Revisão atual:"));assert!(markdown.contains("Derivada da avaliação"));let repeat=call(&f.store,"assessment.deriveCreate",p).unwrap();assert_eq!(repeat["assessmentId"],aid);assert_eq!(repeat["replayed"],true);
    }
    #[test]fn active_unknown_or_archived_sources_cannot_derive_or_revise(){
        for state in ["running","unknown","awaiting_approval"]{let f=fixture(Some(&[]),None);f.store.mutate(|s|{s.assessments[0].status=state.into();Ok(())}).unwrap();assert_eq!(public(&f)["actions"]["canSave"],false);assert_eq!(public(&f)["actions"]["canDerive"],false);}
        let f=fixture(Some(&[]),None);call(&f.store,"project.archive",json!({"id":f.project["id"]})).unwrap();assert_eq!(public(&f)["actions"]["canDerive"],false);assert_eq!(options(&f.store.snapshot().unwrap(),f.project["id"].as_str().unwrap()).unwrap()["canCreate"],false);
    }
    #[test]fn v3_migration_and_prospective_adoption_preserve_legacy_json_and_evidence(){
        let f=fixture(Some(&[]),None);import(&f);f.store.mutate(|s|{s.assessments[0].scope_id=None;s.scope_revisions.clear();s.assessment_scopes.clear();s.scope_receipts.clear();Ok(())}).unwrap();let raw_before=serde_json::to_string(&f.store.snapshot().unwrap()).unwrap();let legacy_evidence=serde_json::to_value(&f.store.snapshot().unwrap().assessments[0].evidence).unwrap();let root=f.store.root.clone();drop(f.store);
        let db=rusqlite::Connection::open(root.join("openkoda.sqlite3")).unwrap();db.pragma_update(None,"user_version",3).unwrap();drop(db);let store=Store::open(root).unwrap();assert_eq!(serde_json::to_string(&store.snapshot().unwrap()).unwrap(),raw_before);let legacy=status(&store,&f.assessment).unwrap();assert_eq!(legacy["phase"],"legacy");assert_eq!(legacy["actions"]["canReviewStoredEvidence"],true);assert_eq!(legacy["actions"]["canImportEvidence"],false);
        let mut p=json!({"assessmentId":f.assessment,"expectedStateDigest":legacy["stateDigest"],"objective":"Changed old objective","methods":["local.inventory.v1"],"window":{"startsAt":null,"endsAt":null}});assert!(call(&store,"assessment.scopePreview",p.clone()).unwrap_err().contains("scope_legacy_objective_immutable"));p["objective"]=json!("Observe owned fixture");let(action,p)=commit(&store,p);assert_eq!(action,"assessment.scopeAdopt");call(&store,&action,p).unwrap();let current=status(&store,&f.assessment).unwrap();assert_eq!(current["phase"],"frozen");assert_eq!(current["frozen"]["reason"],"legacy_adoption");assert_eq!(serde_json::to_value(&store.snapshot().unwrap().assessments[0].evidence).unwrap(),legacy_evidence);
    }
    #[test]fn malformed_cross_assessment_refs_and_foreign_fields_fail_closed(){
        let f=fixture(Some(&["local.inventory.v1"]),None);let reference=admit(&f.store,&f.assessment,"local.inventory.v1",None,"fixture.admit").unwrap();let mut forged=reference.clone();forged.assessment_id=model::id();assert!(check_work(&f.store.snapshot().unwrap(),&f.assessment,"local.inventory.v1",Some(&forged)).is_err());assert!(f.store.mutate(|s|{s.assessment_scopes[0].frozen.as_mut().unwrap().digest="0".repeat(64);Ok(())}).is_err());assert!(call(&f.store,"assessment.scopeStatus",json!({"assessmentId":f.assessment,"methods":[]})).is_err());
        let p=proposal(&f,"Observe owned fixture",&[]);let preview=call(&f.store,"assessment.scopePreview",p).unwrap();assert_eq!(preview["canCommit"],false);assert!(f.store.snapshot().unwrap().executions.is_empty());
    }
}
