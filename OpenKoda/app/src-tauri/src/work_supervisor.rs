//! A durable single-resource arbiter. A permit coordinates effects; it never
//! substitutes scope, data, budget, or concrete-plan authority.
use crate::{core::Store, model::{self,Snapshot}, workflow::*, work_store::{self,CommandResult,WorkAttempt}};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};

#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub(crate) enum EffectOwnerKind { Native, AiRequest, AiInvestigation, AiRequestIntent, AiInvestigationIntent, AiCatalog, KaliRun, KaliDiagnostic, KaliRunIntent, KaliDiagnosticIntent, Remediation }
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct EffectOwner { pub id:String, #[serde(deserialize_with="required_nullable")] pub assessment_id:Option<String>, pub kind:EffectOwnerKind }
pub(crate) struct UnboundDirectCandidate {pub owner:EffectOwner,pub boot_id:String,pub effect:Option<EffectRef>}
#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
enum LeaseState { Claimed, Bound, Unknown }
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct LeaseRecord {
    owner:EffectOwner, boot_id:String, state:LeaseState,
    #[serde(deserialize_with="required_nullable")] attempt_id:Option<String>,
    #[serde(deserialize_with="required_nullable")] effect:Option<EffectRef>,
    #[serde(default,skip_serializing_if="Option::is_none")] data_review_ref:Option<crate::data_policy::DataReviewRef>,
}
#[derive(Clone,Debug)]
pub(crate) struct QueuedRecoveryCandidate {
    pub attempt_id:String,pub job_id:String,pub assessment_id:String,pub job_generation:u64,
    pub owner:EffectOwner,pub boot_id:String,pub effect:Option<EffectRef>,pub reserved_effect:EffectRef,
    pub compiled:CompiledOperationRef,pub input_refs:Vec<ResultRef>,pub data_review_ref:crate::data_policy::DataReviewRef,
    pub kali_binding:Option<crate::kali_bridge::KaliWorkBinding>,
}
pub(crate) struct QueuedRecoveryPermit<'a>{arbiter:&'a WorkArbiter,store:&'a Store,lease:LeaseRecord,candidate:QueuedRecoveryCandidate}
impl QueuedRecoveryPermit<'_>{pub(crate) fn candidate(&self)->&QueuedRecoveryCandidate{&self.candidate}}
impl Drop for QueuedRecoveryPermit<'_>{fn drop(&mut self){self.arbiter.reconciling.store(false,Ordering::Release);}}
pub(crate) struct AiLocalClosurePermit<'a>{arbiter:&'a WorkArbiter,candidate:crate::work_disposition::ClosureCandidate}
impl AiLocalClosurePermit<'_>{pub(crate) fn candidate(&self)->&crate::work_disposition::ClosureCandidate{&self.candidate}}
impl Drop for AiLocalClosurePermit<'_>{fn drop(&mut self){self.arbiter.reconciling.store(false,Ordering::Release);}}
pub(crate) enum DispatchOrigin { Direct, Queued(QueuedAttemptToken) }
/// Host-only token. Neither a renderer payload nor a reconstructed grant.
pub(crate) struct QueuedAttemptToken { job_id:String, attempt_id:String, generation:u64, boot_id:String }
impl QueuedAttemptToken {
    pub(crate) fn job_id(&self)->&str{&self.job_id}
    pub(crate) fn attempt_id(&self)->&str{&self.attempt_id}
    pub(crate) fn generation(&self)->u64{self.generation}
    pub(crate) fn boot_id(&self)->&str{&self.boot_id}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum WorkControl {Active,Paused,Canceled}
pub(crate) struct EffectPermit<'a> { store:&'a Store, lease:LeaseRecord, finished:bool }
impl EffectPermit<'_> {
    pub(crate) fn attempt_id(&self)->Option<&str>{self.lease.attempt_id.as_deref()}
    /// Called only after the transaction containing bind_effect_locked commits.
    pub(crate) fn acknowledge_bound(&mut self,effect:EffectRef){self.lease.effect=Some(effect);self.lease.state=LeaseState::Bound;}
    /// Called only after the transaction containing finish_permit_locked commits.
    pub(crate) fn acknowledge_finished(&mut self){self.finished=true;}
}
pub(crate) struct NativeWorkContext { pub attempt_id:String, pub max_capture_bytes:u64 }
pub(crate) struct KaliWorkContext {pub attempt_id:String,pub reserved_run_id:String,pub max_capture_bytes:u64,pub data_review_ref:crate::data_policy::DataReviewRef}
pub(crate) struct QueuedNativeDispatch {pub plan_id:String,pub digest:String,pub token:QueuedAttemptToken}
pub(crate) enum QueuedDispatch {Native(QueuedNativeDispatch),Ai{token:QueuedAttemptToken,grant:crate::ai::QueuedAiGrant},Kali{token:QueuedAttemptToken,grant:crate::kali_bridge::QueuedKaliGrant}}
impl QueuedDispatch {pub(crate) fn attempt_id(&self)->&str{match self{Self::Native(n)=>n.token.attempt_id(),Self::Ai{token,..}|Self::Kali{token,..}=>token.attempt_id()}}}
/// Observation-only control token; it neither replaces nor frees the unknown
/// resource lease and cannot be constructed from renderer fields.
pub(crate) struct ReconciliationPermit<'a>{arbiter:&'a WorkArbiter,lease:LeaseRecord}
impl ReconciliationPermit<'_>{pub(crate) fn effect(&self)->&EffectRef{self.lease.effect.as_ref().expect("reconciliation always binds its root")}}
impl Drop for ReconciliationPermit<'_>{fn drop(&mut self){self.arbiter.reconciling.store(false,Ordering::Release);}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum EffectTermination { NotStarted, Stopped, Unknown }
pub(crate) struct WorkArbiter { boot_id:String,reconciling:AtomicBool }
impl Default for WorkArbiter {fn default()->Self{Self{boot_id:model::id(),reconciling:AtomicBool::new(false)}}}

fn state_name(state:LeaseState)->&'static str{match state{LeaseState::Claimed=>"claimed",LeaseState::Bound=>"bound",LeaseState::Unknown=>"unknown"}}
fn load_lease(conn:&Connection)->Result<Option<LeaseRecord>,String>{
    let raw:Option<String>=conn.query_row("SELECT data FROM work_leases WHERE resource='assessment_work'",[],|r|r.get(0)).optional().map_err(storage_error)?;
    raw.map(|raw|serde_json::from_str(&raw).map_err(storage_error)).transpose()
}
fn save_lease(conn:&Connection,lease:&LeaseRecord)->Result<(),String>{
    conn.execute("INSERT INTO work_leases(resource,owner_id,attempt_id,effect_kind,effect_id,boot_id,state,data) VALUES('assessment_work',?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(resource) DO UPDATE SET owner_id=excluded.owner_id,attempt_id=excluded.attempt_id,effect_kind=excluded.effect_kind,effect_id=excluded.effect_id,boot_id=excluded.boot_id,state=excluded.state,data=excluded.data",params![lease.owner.id,lease.attempt_id,lease.effect.as_ref().map(EffectRef::kind),lease.effect.as_ref().map(EffectRef::id),lease.boot_id,state_name(lease.state),encode(lease)?]).map_err(storage_error)?;Ok(())
}
pub(crate) fn backend_unknown(store:&Store)->Result<bool,String>{Ok(crate::ai::has_unresolved_work(&store.root)?||crate::kali_bridge::has_unresolved_work(&store.root)?)}
fn check_backend_gate(store:&Store)->Result<(),String>{if backend_unknown(store)?{return Err(error("work_unknown_effect"));}Ok(())}
pub(crate) fn lease_reason(conn:&Connection)->Result<Option<WorkReason>,String>{Ok(load_lease(conn)?.map(|lease|WorkReason::new(if lease.state==LeaseState::Unknown{WorkReasonCode::WorkUnknownEffect}else{WorkReasonCode::WorkBusy},Some(lease.attempt_id.unwrap_or(lease.owner.id)))))}
pub(crate) fn ensure_quiescent_locked(conn:&Connection,s:&Snapshot)->Result<(),String>{
    if let Some(lease)=load_lease(conn)?{return Err(error(if lease.state==LeaseState::Unknown{"work_unknown_effect"}else{"work_busy"}));}
    if s.executions.iter().any(|e|matches!(e.status.as_str(),"running"|"canceling"|"unknown")){return Err(error("work_unknown_effect"));}
    let unresolved:u64=conn.query_row("SELECT COUNT(*) FROM work_jobs WHERE state IN ('preparing','dispatching','running','cancel_requested','unknown')",[],|r|r.get(0)).map_err(storage_error)?;
    if unresolved>0{return Err(error("work_reconciliation_required"));}Ok(())
}
pub(crate) fn deactivate_after_restore(store:&Store)->Result<(),String>{WorkSupervisor::default().recover_after_boot(store)}
pub(crate) fn validate_leases(conn:&Connection,s:&Snapshot)->Result<(),String>{
    let count:u32=conn.query_row("SELECT COUNT(*) FROM work_leases",[],|r|r.get(0)).map_err(storage_error)?;if count>1{return Err(error("work_storage_unavailable"));}
    let lease=load_lease(conn)?;
    let mut query=conn.prepare("SELECT id FROM work_attempts WHERE state IN ('dispatching','running','cancel_requested','unknown')").map_err(storage_error)?;
    let active=query.query_map([],|row|row.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;
    if active.len()>1||active.iter().any(|id|lease.as_ref().and_then(|lease|lease.attempt_id.as_ref())!=Some(id)){return Err(error("work_storage_unavailable"));}
    if let Some(lease)=lease{
        valid_id(&lease.owner.id)?;valid_id(&lease.boot_id)?;
        if let Some(aid)=&lease.owner.assessment_id{if !s.assessments.iter().any(|a|a.id==*aid){return Err(error("work_storage_unavailable"));}}
        let row:(String,Option<String>,Option<String>,Option<String>,String,String)=conn.query_row("SELECT owner_id,attempt_id,effect_kind,effect_id,boot_id,state FROM work_leases WHERE resource='assessment_work'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(storage_error)?;
        if row!=(lease.owner.id.clone(),lease.attempt_id.clone(),lease.effect.as_ref().map(|e|e.kind().into()),lease.effect.as_ref().map(|e|e.id().into()),lease.boot_id.clone(),state_name(lease.state).into())||lease.state==LeaseState::Bound&&lease.effect.is_none(){return Err(error("work_storage_unavailable"));}
        if let Some(id)=&lease.attempt_id{let a=work_store::attempt(conn,id)?;let j=work_store::job(conn,&a.view.job_id)?;if !a.view.state.active()||j.current_attempt.as_ref().map(|current|current.id.as_str())!=Some(id.as_str())||lease.owner.assessment_id.as_deref()!=Some(j.assessment_id.as_str())||a.boot_id!=lease.boot_id||a.view.effect!=lease.effect{return Err(error("work_storage_unavailable"));}}
        if let Some(effect)=&lease.effect{valid_id(effect.id())?;}
    }Ok(())
}
impl WorkArbiter {
    pub(crate) fn inspect_ai_local_closure(&self,store:&Store,target:&crate::work_disposition::AiLocalClosureTarget,assessment_id:Option<&str>)->Result<crate::work_disposition::ClosureCandidate,String>{store.work_read(|conn,s|ai_local_closure_candidate_locked(conn,s,target,assessment_id))}
    pub(crate) fn claim_ai_local_closure<'a>(&'a self,store:&Store,target:&crate::work_disposition::AiLocalClosureTarget,assessment_id:Option<&str>)->Result<AiLocalClosurePermit<'a>,String>{
        if self.reconciling.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err(error("work_busy"));}
        match self.inspect_ai_local_closure(store,target,assessment_id){Ok(candidate)=>Ok(AiLocalClosurePermit{arbiter:self,candidate}),Err(error)=>{self.reconciling.store(false,Ordering::Release);Err(error)}}
    }
    pub(crate) fn boot_id(&self)->&str{&self.boot_id}
    pub(crate) fn ensure_quiescent(&self,store:&Store)->Result<(),String>{store.work_read(ensure_quiescent_locked)?;check_backend_gate(store)?;store.work_read(ensure_quiescent_locked)}
    pub(crate) fn queued_recovery_candidate(&self,store:&Store,attempt_id:&str,backend:Backend)->Result<QueuedRecoveryCandidate,String>{store.work_read(|conn,_|queued_recovery_candidate_locked(conn,attempt_id,backend).map(|(_,candidate)|candidate))}
    pub(crate) fn claim_queued_reconciliation<'a>(&'a self,store:&'a Store,attempt_id:&str,backend:Backend)->Result<QueuedRecoveryPermit<'a>,String>{
        if self.reconciling.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err(error("work_busy"));}
        match store.work_read(|conn,_|queued_recovery_candidate_locked(conn,attempt_id,backend)){
            Ok((lease,candidate))=>Ok(QueuedRecoveryPermit{arbiter:self,store,lease,candidate}),
            Err(err)=>{self.reconciling.store(false,Ordering::Release);Err(err)},
        }
    }
    pub(crate) fn unbound_direct_candidate(&self,store:&Store)->Result<Option<UnboundDirectCandidate>,String>{self.direct_candidate(store,false)}
    pub(crate) fn direct_ai_terminal_candidate(&self,store:&Store)->Result<Option<UnboundDirectCandidate>,String>{store.work_read(|conn,_|{
        let Some(lease)=load_lease(conn)?else{return Ok(None);};
        if lease.state!=LeaseState::Unknown||lease.attempt_id.is_some()||lease.boot_id==self.boot_id||!matches!((&lease.effect,lease.owner.kind),(Some(EffectRef::AiRequest{id}),EffectOwnerKind::AiRequestIntent)|(Some(EffectRef::AiInvestigation{id}),EffectOwnerKind::AiInvestigationIntent) if *id==lease.owner.id){return Ok(None);}
        if let Some(EffectRef::AiInvestigation{id})=&lease.effect{work_store::assert_direct_investigation_unowned(conn,id)?;}
        Ok(Some(UnboundDirectCandidate{owner:lease.owner,boot_id:lease.boot_id,effect:lease.effect}))
    })}
    pub(crate) fn reconcile_ai_terminal(&self,store:&Store,proof:&crate::ai::AiTerminalProof)->Result<(),String>{
        if proof.profile_fingerprint()!=crate::work_disposition::profile_fingerprint(&store.root)?{return Err(error("work_reconciliation_required"));}
        store.work_transaction(|conn,s|{
            let lease=load_lease(conn)?.ok_or_else(||error("work_reconciliation_required"))?;
            if lease.state!=LeaseState::Unknown||lease.attempt_id.is_some()||lease.boot_id==self.boot_id||&lease.owner!=proof.owner()||lease.boot_id!=proof.boot_id()||lease.effect.as_ref()!=Some(proof.effect())||!matches!((&lease.effect,lease.owner.kind),(Some(EffectRef::AiRequest{id}),EffectOwnerKind::AiRequestIntent)|(Some(EffectRef::AiInvestigation{id}),EffectOwnerKind::AiInvestigationIntent) if *id==lease.owner.id){return Err(error("work_reconciliation_required"));}
            if let Some(EffectRef::AiInvestigation{id})=&lease.effect{work_store::assert_direct_investigation_unowned(conn,id)?;}
            proof.finish_receipts(s)?;
            conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1 AND boot_id=?2",params![lease.owner.id,lease.boot_id]).map_err(storage_error)?;Ok(())
        })
    }
    pub(crate) fn direct_kali_recovery_candidate(&self,store:&Store)->Result<Option<UnboundDirectCandidate>,String>{self.direct_candidate(store,true)}
    fn direct_candidate(&self,store:&Store,kali:bool)->Result<Option<UnboundDirectCandidate>,String>{store.work_read(|conn,_|{
        let Some(lease)=load_lease(conn)? else{return Ok(None);};
        if lease.state!=LeaseState::Unknown||lease.attempt_id.is_some()||lease.boot_id==self.boot_id{return Ok(None);}
        if kali{if !matches!(lease.owner.kind,EffectOwnerKind::KaliRunIntent|EffectOwnerKind::KaliDiagnosticIntent)||lease.effect.as_ref().is_some_and(|effect|effect!=&EffectRef::KaliRun{id:lease.owner.id.clone()}){return Ok(None);}}else if lease.effect.is_some(){return Ok(None);}
        Ok(Some(UnboundDirectCandidate{owner:lease.owner,boot_id:lease.boot_id,effect:lease.effect}))
    })}
    pub(crate) fn reconcile_ai_not_started(&self,store:&Store,proof:&crate::ai::AiNotStartedProof)->Result<(),String>{
        if !matches!(proof.owner().kind,EffectOwnerKind::AiRequestIntent|EffectOwnerKind::AiInvestigationIntent|EffectOwnerKind::AiCatalog){return Err(error("work_reconciliation_required"));}
        store.work_transaction(|conn,_|release_proven_unbound(conn,&self.boot_id,proof.owner(),proof.boot_id(),None))
    }
    pub(crate) fn reconcile_kali_not_started(&self,store:&Store,proof:&crate::kali_bridge::KaliNotStartedProof)->Result<(),String>{
        if !matches!(proof.owner().kind,EffectOwnerKind::KaliRunIntent|EffectOwnerKind::KaliDiagnosticIntent)||proof.effect().is_some_and(|effect|effect!=&EffectRef::KaliRun{id:proof.owner().id.clone()}){return Err(error("work_reconciliation_required"));}
        store.work_transaction(|conn,_|release_proven_unbound(conn,&self.boot_id,proof.owner(),proof.boot_id(),proof.effect()))
    }
    pub(crate) fn claim_reconciliation<'a>(&'a self,store:&Store,target:&crate::kali_bridge::KaliRecoveryTarget)->Result<ReconciliationPermit<'a>,String>{
        if self.reconciling.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err(error("work_busy"));}
        let result=store.work_transaction(|conn,s|{
            let effect=target.root_effect();valid_id(effect.id())?;let aid=target.assessment_id();
            if !matches!(effect,EffectRef::KaliRun{..})||!s.assessments.iter().any(|a|a.id==aid){return Err(error("work_reconciliation_required"));}
            if let Some(lease)=load_lease(conn)?{if lease.state!=LeaseState::Unknown||lease.effect.as_ref()!=Some(&effect)||lease.owner.assessment_id.as_deref()!=Some(aid){return Err(error("work_unknown_effect"));}return Ok(lease);}
            let lease=LeaseRecord{owner:EffectOwner{id:model::id(),assessment_id:Some(aid.into()),kind:EffectOwnerKind::KaliRun},boot_id:self.boot_id.clone(),state:LeaseState::Unknown,attempt_id:None,effect:Some(effect),data_review_ref:None};save_lease(conn,&lease)?;Ok(lease)
        });
        match result{Ok(lease)=>Ok(ReconciliationPermit{arbiter:self,lease}),Err(error)=>{self.reconciling.store(false,Ordering::Release);Err(error)}}
    }
    pub(crate) fn claim_direct<'a>(&self,store:&'a Store,owner:EffectOwner)->Result<EffectPermit<'a>,String>{
        self.claim_direct_with_review(store,owner,None)
    }
    pub(crate) fn claim_direct_kali<'a>(&self,store:&'a Store,owner:EffectOwner,review:Option<&crate::data_policy::DataReviewRef>)->Result<EffectPermit<'a>,String>{
        if !matches!(owner.kind,EffectOwnerKind::KaliRunIntent|EffectOwnerKind::KaliDiagnosticIntent){return Err(error("work_invalid_request"));}self.claim_direct_with_review(store,owner,review)
    }
    fn claim_direct_with_review<'a>(&self,store:&'a Store,owner:EffectOwner,review:Option<&crate::data_policy::DataReviewRef>)->Result<EffectPermit<'a>,String>{
        if self.reconciling.load(Ordering::Acquire){return Err(error("work_busy"));}
        store.work_read(|conn,_|{if let Some(lease)=load_lease(conn)?{return Err(error(if lease.state==LeaseState::Unknown{"work_unknown_effect"}else{"work_busy"}));}Ok(())})?;check_backend_gate(store)?;
        valid_id(&owner.id)?;let lease=LeaseRecord{owner,boot_id:self.boot_id.clone(),state:LeaseState::Claimed,attempt_id:None,effect:None,data_review_ref:review.cloned()};
        store.work_transaction(|conn,s|{if self.reconciling.load(Ordering::Acquire){return Err(error("work_busy"));}if let Some(existing)=load_lease(conn)?{return Err(error(if existing.state==LeaseState::Unknown{"work_unknown_effect"}else{"work_busy"}));}if s.executions.iter().any(|e|matches!(e.status.as_str(),"running"|"canceling"|"unknown")){return Err(error("work_unknown_effect"));}if matches!(lease.owner.kind,EffectOwnerKind::AiRequestIntent|EffectOwnerKind::AiInvestigationIntent){crate::work_disposition::reserve_record_slot(conn)?;}if matches!(lease.owner.kind,EffectOwnerKind::AiInvestigation|EffectOwnerKind::AiInvestigationIntent){work_store::assert_direct_investigation_unowned(conn,&lease.owner.id)?;}if let Some(review)=review{work_store::assert_kali_review_unowned(conn,review)?;}save_lease(conn,&lease)})?;
        Ok(EffectPermit{store,lease,finished:false})
    }
    pub(crate) fn claim_queued<'a>(&self,store:&'a Store,token:QueuedAttemptToken)->Result<EffectPermit<'a>,String>{
        if self.reconciling.load(Ordering::Acquire){return Err(error("work_busy"));}
        if token.boot_id!=self.boot_id{return Err(error("work_reconciliation_required"));}
        store.work_read(|conn,_|{if let Some(lease)=load_lease(conn)?{return Err(error(if lease.state==LeaseState::Unknown{"work_unknown_effect"}else{"work_busy"}));}Ok(())})?;check_backend_gate(store)?;
        let lease=store.work_transaction(|conn,s|{
            if self.reconciling.load(Ordering::Acquire){return Err(error("work_busy"));}
            if load_lease(conn)?.is_some(){return Err(error("work_busy"));}
            let mut j=work_store::job(conn,&token.job_id)?;let h=work_store::head(conn,&j.revision.plan_id)?;let mut a=work_store::attempt(conn,&token.attempt_id)?;
            if a.view.backend==Backend::Ai{crate::work_disposition::reserve_record_slot(conn)?;}
            if j.generation!=token.generation||j.state!=JobState::Queued||j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(token.attempt_id.as_str())||a.view.state!=AttemptState::Reserved||a.view.effect.is_some(){return Err(error("work_revision_changed"));}
            if h.control!=PlanControl::Active||h.head_revision_id!=j.revision.revision_id{return Err(error("work_paused"));}
            check_current_body(s,&work_store::revision(conn,&j.revision.revision_id)?.body)?;
            let (owner_id,kind)=match (a.view.backend,a.reserved_effect.as_ref()){
                (Backend::Native,None)=>(model::id(),EffectOwnerKind::Native),
                (Backend::Ai,Some(EffectRef::AiInvestigation{id}))=>(id.clone(),EffectOwnerKind::AiInvestigationIntent),
                (Backend::Kali,Some(EffectRef::KaliRun{id}))=>(id.clone(),EffectOwnerKind::KaliRunIntent),
                _=>return Err(error("work_owned_operation")),
            };
            let owner=EffectOwner{id:owner_id,assessment_id:Some(j.assessment_id.clone()),kind};
            a.view.state=AttemptState::Dispatching;a.view.dispatch_intent_at=Some(model::now());a.boot_id=self.boot_id.clone();work_store::save_attempt(conn,&a)?;
            j.state=JobState::Dispatching;j.generation+=1;work_store::save_job(conn,&j)?;
            let lease=LeaseRecord{owner,boot_id:self.boot_id.clone(),state:LeaseState::Claimed,attempt_id:Some(a.view.id.clone()),effect:None,data_review_ref:a.data_review_ref.clone()};save_lease(conn,&lease)?;
            work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),Some(a.view.id),Some(JobState::Dispatching),None,None)?;Ok(lease)
        })?;Ok(EffectPermit{store,lease,finished:false})
    }
    pub(crate) fn bind_effect(&self,permit:&mut EffectPermit<'_>,effect:EffectRef)->Result<(),String>{
        permit.store.work_transaction(|conn,_|bind_effect_locked(conn,permit,&effect))?;permit.lease.effect=Some(effect);permit.lease.state=LeaseState::Bound;Ok(())
    }
    pub(crate) fn finish(&self,mut permit:EffectPermit<'_>,observed:EffectTermination)->Result<(),String>{
        permit.store.work_transaction(|conn,s|{
            if observed==EffectTermination::NotStarted{if let Some(id)=permit.attempt_id(){let a=work_store::attempt(conn,id)?;if let CompiledOperationRef::Native{plan_id,..}=&a.compiled{
                let p=s.plans.iter_mut().find(|p|p.id==*plan_id).ok_or_else(||error("work_input_changed"))?;
                if !matches!(p.status.as_str(),"proposed"|"approved"|"revoked"){return Err(error("work_unknown_effect"));}
                p.status="revoked".into();let aid=p.assessment_id.clone();if let Some(assessment)=s.assessments.iter_mut().find(|a|a.id==aid){if matches!(assessment.status.as_str(),"awaiting_approval"|"queued"){assessment.status="blocked".into();assessment.updated_at=model::now();}}
            }}}
            finish_permit_locked(conn,&permit,observed)?;
            if observed==EffectTermination::Unknown{mark_native_unknown_locked(conn,s,&permit)?;}Ok(())
        })?;permit.finished=true;Ok(())
    }
}
fn release_proven_unbound(conn:&Connection,current_boot:&str,owner:&EffectOwner,boot_id:&str,effect:Option<&EffectRef>)->Result<(),String>{
    let lease=load_lease(conn)?.ok_or_else(||error("work_reconciliation_required"))?;
    if lease.state!=LeaseState::Unknown||lease.attempt_id.is_some()||lease.owner!=*owner||lease.boot_id!=boot_id||lease.boot_id==current_boot||lease.effect.as_ref()!=effect{return Err(error("work_reconciliation_required"));}
    conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1 AND boot_id=?2",params![owner.id,boot_id]).map_err(storage_error)?;Ok(())
}
fn queued_recovery_candidate_locked(conn:&Connection,attempt_id:&str,backend:Backend)->Result<(LeaseRecord,QueuedRecoveryCandidate),String>{
    if !matches!(backend,Backend::Ai|Backend::Kali){return Err(error("work_reconciliation_required"));}valid_id(attempt_id)?;
    let lease=load_lease(conn)?.ok_or_else(||error("work_reconciliation_required"))?;let a=work_store::attempt(conn,attempt_id)?;let j=work_store::job(conn,&a.view.job_id)?;
    if lease.state!=LeaseState::Unknown||lease.attempt_id.as_deref()!=Some(attempt_id)||lease.owner.assessment_id.as_deref()!=Some(j.assessment_id.as_str())||lease.boot_id!=a.boot_id||a.view.state!=AttemptState::Unknown||a.view.backend!=backend||a.view.effect!=lease.effect||j.state!=JobState::Unknown||j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(attempt_id){return Err(error("work_reconciliation_required"));}
    let reserved_effect=a.reserved_effect.clone().ok_or_else(||error("work_reconciliation_required"))?;
    if lease.owner.id!=reserved_effect.id()||lease.effect.as_ref().is_some_and(|effect|effect!=&reserved_effect)||!matches!((backend,&reserved_effect,lease.owner.kind),(Backend::Ai,EffectRef::AiInvestigation{..},EffectOwnerKind::AiInvestigationIntent)|(Backend::Kali,EffectRef::KaliRun{..},EffectOwnerKind::KaliRunIntent)){return Err(error("work_reconciliation_required"));}
    let data_review_ref=a.data_review_ref.ok_or_else(||error("work_reconciliation_required"))?;if lease.data_review_ref.as_ref()!=Some(&data_review_ref){return Err(error("work_reconciliation_required"));}
    let candidate=QueuedRecoveryCandidate{attempt_id:attempt_id.into(),job_id:j.id,assessment_id:j.assessment_id,job_generation:j.generation,owner:lease.owner.clone(),boot_id:lease.boot_id.clone(),effect:lease.effect.clone(),reserved_effect,compiled:a.compiled,input_refs:a.input_refs,data_review_ref,kali_binding:a.kali_binding};Ok((lease,candidate))
}
fn checked_queued_recovery(conn:&Connection,permit:&QueuedRecoveryPermit<'_>,backend:Backend)->Result<(),String>{
    let(lease,current)=queued_recovery_candidate_locked(conn,&permit.candidate.attempt_id,backend)?;let expected=&permit.candidate;
    if lease.owner!=permit.lease.owner||lease.boot_id!=permit.lease.boot_id||current.job_generation!=expected.job_generation||current.effect!=expected.effect||current.reserved_effect!=expected.reserved_effect||current.compiled!=expected.compiled||current.input_refs!=expected.input_refs||current.data_review_ref!=expected.data_review_ref||current.kali_binding!=expected.kali_binding{return Err(error("work_reconciliation_required"));}Ok(())
}
pub(crate) fn finish_ai_recovery_locked(conn:&Connection,s:&Snapshot,permit:&QueuedRecoveryPermit<'_>,proof:&crate::ai::WorkAiCompletion)->Result<(),String>{
    checked_queued_recovery(conn,permit,Backend::Ai)?;
    let descriptor=EffectPermit{store:permit.store,lease:permit.lease.clone(),finished:true};finish_ai_locked(conn,s,&descriptor,proof)?;finish_permit_impl(conn,&descriptor,proof.termination(),true)
}
fn ai_local_closure_candidate_locked(conn:&Connection,s:&Snapshot,target:&crate::work_disposition::AiLocalClosureTarget,journal_assessment:Option<&str>)->Result<crate::work_disposition::ClosureCandidate,String>{
    use crate::work_disposition::{AiLocalClosureTarget as Target,ClosureCandidate,ClosureLease,ClosureJob};
    let lease=load_lease(conn)?;
    let (effect,assessment_id,job)=match target{
        Target::Attempt{attempt_id}=>{
            valid_id(attempt_id)?;let attempt=work_store::attempt(conn,attempt_id)?;let job=work_store::job(conn,&attempt.view.job_id)?;
            if attempt.view.backend!=Backend::Ai||!matches!(attempt.view.state,AttemptState::Unknown|AttemptState::ClosedLocally)||job.current_attempt.as_ref().map(|a|a.id.as_str())!=Some(attempt_id)||attempt.view.state==AttemptState::Unknown&&job.state!=JobState::Unknown||attempt.view.state==AttemptState::ClosedLocally&&job.state!=JobState::ClosedLocally{return Err(error("work_reconciliation_required"));}
            let effect=attempt.view.effect.clone().ok_or_else(||error("work_reconciliation_required"))?;if !matches!(effect,EffectRef::AiInvestigation{..})||attempt.reserved_effect.as_ref()!=Some(&effect){return Err(error("work_reconciliation_required"));}
            if attempt.view.state==AttemptState::Unknown&&lease.as_ref().is_none_or(|l|l.attempt_id.as_deref()!=Some(attempt_id)||l.boot_id!=attempt.boot_id){return Err(error("work_reconciliation_required"));}
            (effect,Some(job.assessment_id.clone()),Some(ClosureJob{job_id:job.id,attempt_id:attempt_id.clone(),generation:job.generation,revision:job.revision}))
        },
        Target::AiRequest{request_id}=>{valid_id(request_id)?;(EffectRef::AiRequest{id:request_id.clone()},journal_assessment.map(str::to_owned),None)},
        Target::AiInvestigation{investigation_id}=>{valid_id(investigation_id)?;if work_store::investigation_owner_locked(conn,investigation_id)?.is_some(){return Err(error("work_owned_operation"));}(EffectRef::AiInvestigation{id:investigation_id.clone()},journal_assessment.map(str::to_owned),None)},
    };
    if assessment_id.as_deref()!=journal_assessment||assessment_id.as_ref().is_some_and(|id|!s.assessments.iter().any(|a|a.id==*id)){return Err(error("work_reconciliation_required"));}
    let lease=lease.map(|lease|{
        if lease.state!=LeaseState::Unknown||lease.effect.as_ref()!=Some(&effect)||lease.owner.id!=effect.id()||lease.owner.assessment_id!=assessment_id||lease.attempt_id.as_deref()!=job.as_ref().map(|j|j.attempt_id.as_str())||!matches!((&effect,lease.owner.kind),(EffectRef::AiRequest{..},EffectOwnerKind::AiRequestIntent)|(EffectRef::AiInvestigation{..},EffectOwnerKind::AiInvestigationIntent)){return Err(error("work_reconciliation_required"));}
        Ok(ClosureLease{owner:lease.owner,boot_id:lease.boot_id,effect:effect.clone(),attempt_id:lease.attempt_id})
    }).transpose()?;
    if s.executions.iter().any(|execution|matches!(execution.status.as_str(),"running"|"canceling"|"unknown")){return Err(error("work_busy"));}
    Ok(ClosureCandidate{target:target.clone(),effect,assessment_id,lease,job})
}
pub(crate) fn validate_ai_local_closure_locked(conn:&Connection,s:&Snapshot,permit:&AiLocalClosurePermit<'_>)->Result<(),String>{
    if ai_local_closure_candidate_locked(conn,s,&permit.candidate.target,permit.candidate.assessment_id.as_deref())?!=permit.candidate{return Err(error("work_revision_changed"));}Ok(())
}
pub(crate) fn release_ai_local_closure_locked(conn:&Connection,permit:&AiLocalClosurePermit<'_>)->Result<(),String>{
    if let Some(expected)=&permit.candidate.lease{let lease=load_lease(conn)?.ok_or_else(||error("work_reconciliation_required"))?;if lease.state!=LeaseState::Unknown||lease.owner!=expected.owner||lease.boot_id!=expected.boot_id||lease.effect.as_ref()!=Some(&expected.effect)||lease.attempt_id!=expected.attempt_id{return Err(error("work_reconciliation_required"));}conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1 AND boot_id=?2",params![expected.owner.id,expected.boot_id]).map_err(storage_error)?;}else if load_lease(conn)?.is_some(){return Err(error("work_busy"));}Ok(())
}
pub(crate) fn finish_kali_recovery_locked(conn:&Connection,s:&Snapshot,permit:&QueuedRecoveryPermit<'_>,proof:&crate::kali_bridge::WorkKaliCompletion)->Result<(),String>{
    checked_queued_recovery(conn,permit,Backend::Kali)?;
    let descriptor=EffectPermit{store:permit.store,lease:permit.lease.clone(),finished:true};finish_kali_locked(conn,s,&descriptor,proof)?;finish_permit_impl(conn,&descriptor,proof.termination(),true)
}
/// Only the bridge can construct the proof, after persisting the diagnostic and
/// checking cleanup of the original session and every reconciliation descendant.
pub(crate) fn finish_reconciliation_locked(conn:&Connection,permit:&ReconciliationPermit<'_>,proof:&crate::kali_bridge::KaliReconciliationProof)->Result<(),String>{
    let lease=load_lease(conn)?.ok_or_else(||error("work_reconciliation_required"))?;
    if lease.state!=LeaseState::Unknown||lease.owner!=permit.lease.owner||lease.boot_id!=permit.lease.boot_id||lease.attempt_id!=permit.lease.attempt_id||lease.effect!=permit.lease.effect||lease.effect.as_ref()!=Some(&proof.root_effect())||lease.owner.assessment_id.as_deref()!=Some(proof.assessment_id()){return Err(error("work_reconciliation_required"));}
    if let Some(id)=&lease.attempt_id{let a=work_store::attempt(conn,id)?;if a.view.state.active()||a.view.state==AttemptState::Reserved||a.view.cleanup!=CleanupState::Confirmed{return Err(error("work_reconciliation_required"));}}
    conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1 AND boot_id=?2",params![lease.owner.id,lease.boot_id]).map_err(storage_error)?;Ok(())
}
pub(crate) fn bind_effect_locked(conn:&Connection,permit:&EffectPermit<'_>,effect:&EffectRef)->Result<(),String>{
    valid_id(effect.id())?;let mut lease=checked_lease(conn,permit)?;
    if lease.effect.as_ref().is_some_and(|existing|existing!=effect){return Err(error("work_revision_changed"));}
    lease.effect=Some(effect.clone());lease.state=LeaseState::Bound;
    if let Some(id)=&lease.attempt_id{let mut a=work_store::attempt(conn,id)?;if a.reserved_effect.as_ref().is_some_and(|reserved|reserved!=effect){return Err(error("work_owned_operation"));}a.view.effect=Some(effect.clone());work_store::save_attempt(conn,&a)?;let j=work_store::job(conn,&a.view.job_id)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::EffectBound,Some(j.id),Some(a.view.id),Some(j.state),None,Some(effect.clone()))?;}
    save_lease(conn,&lease)
}
fn checked_lease(conn:&Connection,permit:&EffectPermit<'_>)->Result<LeaseRecord,String>{let lease=load_lease(conn)?.ok_or_else(||error("work_revision_changed"))?;if lease.owner.id!=permit.lease.owner.id||lease.boot_id!=permit.lease.boot_id||lease.attempt_id!=permit.lease.attempt_id{return Err(error("work_revision_changed"));}Ok(lease)}
pub(crate) fn assert_investigation_attach_available(conn:&Connection,id:&str)->Result<(),String>{
    if load_lease(conn)?.is_some_and(|lease|lease.attempt_id.is_none()&&matches!(lease.owner.kind,EffectOwnerKind::AiInvestigation|EffectOwnerKind::AiInvestigationIntent)&&(lease.owner.id==id||lease.effect.as_ref()==Some(&EffectRef::AiInvestigation{id:id.into()}))){return Err(error("work_owned_operation"));}Ok(())
}
pub(crate) fn assert_kali_review_available(conn:&Connection,review:&crate::data_policy::DataReviewRef)->Result<(),String>{
    work_store::assert_kali_review_unowned(conn,review)?;
    if load_lease(conn)?.is_some_and(|lease|lease.attempt_id.is_none()&&lease.data_review_ref.as_ref().is_some_and(|r|r.id==review.id)){return Err(error("work_owned_operation"));}Ok(())
}
/// Caller holds the existing scope gate at the final durable dispatch marker.
/// This helper itself performs no I/O or lock acquisition.
pub(crate) fn work_control_locked(conn:&Connection,permit:&EffectPermit<'_>)->Result<WorkControl,String>{
    let lease=checked_lease(conn,permit)?;let Some(id)=lease.attempt_id else{return Ok(WorkControl::Active);};
    let a=work_store::attempt(conn,&id)?;let j=work_store::job(conn,&a.view.job_id)?;let h=work_store::head(conn,&j.revision.plan_id)?;
    if !matches!(a.view.state,AttemptState::Dispatching|AttemptState::Running|AttemptState::CancelRequested){return Err(error("work_reconciliation_required"));}
    if h.control==PlanControl::Canceled||matches!(j.state,JobState::CancelRequested|JobState::Canceled)||a.view.state==AttemptState::CancelRequested{return Ok(WorkControl::Canceled);}
    if h.control==PlanControl::Paused||h.head_revision_id!=j.revision.revision_id{return Ok(WorkControl::Paused);}Ok(WorkControl::Active)
}
fn queued_backend(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,backend:Backend)->Result<(WorkAttempt,work_store::WorkJob,ActionPlanRevision),String>{
    let lease=checked_lease(conn,permit)?;let id=lease.attempt_id.as_deref().ok_or_else(||error("work_owned_operation"))?;let a=work_store::attempt(conn,id)?;let j=work_store::job(conn,&a.view.job_id)?;let r=work_store::revision(conn,&j.revision.revision_id)?;
    if lease.state!=LeaseState::Claimed||lease.effect.is_some()||a.view.state!=AttemptState::Dispatching||a.view.backend!=backend||j.state!=JobState::Dispatching||a.boot_id!=lease.boot_id||j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(id)||j.compiled.as_ref()!=Some(&a.compiled)||lease.data_review_ref!=a.data_review_ref{return Err(error("work_owned_operation"));}
    match work_control_locked(conn,permit)?{WorkControl::Active=>{},WorkControl::Paused=>return Err(error("work_paused")),WorkControl::Canceled=>return Err(error("work_canceled"))}
    check_current_body(s,&r.body)?;if work_store::resolve_inputs(conn,s,&j)?!=a.input_refs{return Err(error("work_input_changed"));}Ok((a,j,r))
}
pub(crate) fn validate_queued_ai_locked(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,inspection:&crate::ai::WorkAiInspection)->Result<(),String>{
    let(a,j,r)=queued_backend(conn,s,permit,Backend::Ai)?;
    let CompiledOperationRef::AiInvestigation{investigation_id,digest,expires_at}=&a.compiled else{return Err(error("work_owned_operation"));};
    let node=r.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_input_changed"))?;let OperationIntent::AiInvestigation{perspectives,..}=&node.intent else{return Err(error("work_owned_operation"));};
    if j.assessment_id!=inspection.assessment_id()||r.body.scope_ref!=*inspection.scope_ref()||a.input_refs!=inspection.inputs()||perspectives!=inspection.perspectives()||investigation_id!=inspection.id()||digest!=inspection.digest()||expires_at!=inspection.expires_at()||a.data_review_ref.as_ref()!=Some(inspection.data_review())||a.reserved_effect.as_ref()!=Some(&EffectRef::AiInvestigation{id:investigation_id.clone()})||work_store::investigation_binding(conn,investigation_id)?.as_ref()!=Some(&(j.id.clone(),digest.clone())){return Err(error("work_input_changed"));}Ok(())
}
pub(crate) fn validate_queued_kali_locked(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,expected:&crate::kali_bridge::WorkKaliExpectation)->Result<KaliWorkContext,String>{
    let(a,j,r)=queued_backend(conn,s,permit,Backend::Kali)?;
    let CompiledOperationRef::KaliEvidence{review_id,digest,expires_at,environment_id,operation}=&a.compiled else{return Err(error("work_owned_operation"));};
    let Some(EffectRef::KaliRun{id})=&a.reserved_effect else{return Err(error("work_owned_operation"));};let review=a.data_review_ref.as_ref().ok_or_else(||error("work_approval_required"))?;
    let cap=match operation{KaliOperation::Scan=>68*1024,KaliOperation::Hash=>4*1024};
    if j.assessment_id!=expected.assessment_id||r.body.scope_ref!=expected.scope_ref||a.input_refs!=vec![expected.input.clone()]||*operation!=expected.operation||*environment_id!=expected.environment_id||review.id!=*review_id||review.digest!=*digest||review.expires_at!=*expires_at||a.reserved_capture_bytes!=cap{return Err(error("work_input_changed"));}
    Ok(KaliWorkContext{attempt_id:a.view.id,reserved_run_id:id.clone(),max_capture_bytes:cap,data_review_ref:review.clone()})
}
fn mark_backend_running(conn:&Connection,permit:&EffectPermit<'_>,backend:Backend,receipts:&[crate::data_policy::DataUseReceiptRef],captures:&[String],binding:Option<&crate::kali_bridge::KaliWorkBinding>)->Result<(),String>{
    let lease=checked_lease(conn,permit)?;let id=lease.attempt_id.as_deref().ok_or_else(||error("work_owned_operation"))?;let mut a=work_store::attempt(conn,id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
    if a.view.backend!=backend||a.view.state!=AttemptState::Dispatching||j.state!=JobState::Dispatching||lease.effect.is_none()||lease.effect!=a.reserved_effect||receipts.len()>4||captures.len()>2{return Err(error("work_owned_operation"));}
    if let Some(binding)=binding{binding.validate()?;if binding.attempt_id()!=id||binding.run_id()!=lease.effect.as_ref().unwrap().id()||binding.assessment_id()!=j.assessment_id||binding.input_refs()!=a.input_refs||binding.capture_ids()!=captures{return Err(error("work_input_changed"));}if a.kali_binding.as_ref().is_some_and(|old|old!=binding){return Err(error("work_owned_operation"));}a.kali_binding=Some(binding.clone());}
    a.view.state=AttemptState::Running;a.view.started_at=Some(model::now());a.view.cleanup=CleanupState::Pending;a.view.data_receipt_refs=receipts.to_vec();a.view.capture_ids=captures.to_vec();j.state=JobState::Running;j.generation+=1;j.capture_ids=captures.to_vec();
    work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;work_store::enforce_caps(conn)?;
    work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),Some(a.view.id),Some(JobState::Running),None,lease.effect)?;Ok(())
}
pub(crate) fn mark_ai_running_locked(conn:&Connection,permit:&EffectPermit<'_>,receipt_refs:&[crate::data_policy::DataUseReceiptRef])->Result<(),String>{mark_backend_running(conn,permit,Backend::Ai,receipt_refs,&[],None)}
pub(crate) fn mark_kali_running_locked(conn:&Connection,permit:&EffectPermit<'_>,receipt_refs:&[crate::data_policy::DataUseReceiptRef],capture_ids:&[String],binding:&crate::kali_bridge::KaliWorkBinding)->Result<(),String>{mark_backend_running(conn,permit,Backend::Kali,receipt_refs,capture_ids,Some(binding))}
fn finish_backend(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,backend:Backend,aid:&str,effect:&EffectRef,termination:EffectTermination,outcome:AttemptState,results:&[ResultRef],receipts:&[crate::data_policy::DataUseReceiptRef],captures:&[String])->Result<(),String>{
    let lease=checked_lease(conn,permit)?;let id=lease.attempt_id.as_deref().ok_or_else(||error("work_owned_operation"))?;let mut a=work_store::attempt(conn,id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
    if a.view.backend!=backend||j.assessment_id!=aid||a.reserved_effect.as_ref()!=Some(effect)||!a.view.state.active()||results.len()>1||receipts.len()>4||captures!=a.view.capture_ids||receipts.iter().any(|r|!s.data_use_receipts.iter().any(|stored|stored.id==r.id&&stored.digest==r.digest&&stored.assessment_id.as_deref()==Some(aid))){return Err(error("work_owned_operation"));}
    if termination==EffectTermination::NotStarted{if lease.effect.is_some()||outcome!=AttemptState::NotStarted||!results.is_empty()||!captures.is_empty(){return Err(error("work_unknown_effect"));}a.view.data_receipt_refs=receipts.to_vec();work_store::save_attempt(conn,&a)?;return Ok(());}
    if lease.effect.as_ref()!=Some(effect){return Err(error("work_unknown_effect"));}
    if results.iter().any(|r|match backend{Backend::Ai=>!matches!(r,ResultRef::Derived{producer:DerivedResultProducer::Investigation,id,..} if id==effect.id()),Backend::Kali=>!matches!(r,ResultRef::Evidence{..}),_=>true}){return Err(error("work_result_missing"));}
    let state=match (termination,outcome){(EffectTermination::Unknown,AttemptState::Unknown)=>JobState::Unknown,(EffectTermination::Stopped,AttemptState::Completed) if results.len()==1=>JobState::Completed,(EffectTermination::Stopped,AttemptState::Partial)=>JobState::Partial,(EffectTermination::Stopped,AttemptState::Failed)=>JobState::Failed,(EffectTermination::Stopped,AttemptState::Canceled)=>JobState::Canceled,_=>return Err(error("work_unknown_effect"))};
    if termination==EffectTermination::Stopped{for id in captures{let m=crate::capture::load_locked(conn,id)?;if m.state==crate::capture::CaptureState::Open{return Err(error("capture_not_sealed"));}}}
    a.view.state=outcome;a.view.cleanup=if termination==EffectTermination::Unknown{CleanupState::Unknown}else{CleanupState::Confirmed};a.view.finished_at=if termination==EffectTermination::Unknown{None}else{Some(model::now())};a.view.results=results.to_vec();a.view.data_receipt_refs=receipts.to_vec();
    if termination!=EffectTermination::Unknown{a.reserved_capture_bytes=0;a.reserved_critical_events=0;}
    j.state=state;j.generation+=1;j.results=results.to_vec();j.reasons=if state==JobState::Unknown{vec![WorkReason::new(WorkReasonCode::WorkReconciliationRequired,Some(a.view.id.clone()))]}else{vec![]};work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;work_store::validate_results(conn,s,aid,results)?;
    for id in captures{let m=crate::capture::load_locked(conn,id)?;if m.state!=crate::capture::CaptureState::Open{work_store::append_capture_event(conn,&m,true)?;}}
    work_store::append_event(conn,&j.revision.plan_id,aid,WorkEventType::AttemptFinished,Some(j.id),Some(a.view.id),Some(state),None,Some(effect.clone()))?;
    if state==JobState::Completed{release_dependents_locked(conn,s,&j.revision)?;}Ok(())
}
pub(crate) fn finish_ai_locked(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,completion:&crate::ai::WorkAiCompletion)->Result<(),String>{
    if !valid_hash(completion.journal_fingerprint()){return Err(error("work_result_missing"));}
    finish_backend(conn,s,permit,Backend::Ai,completion.assessment_id(),completion.effect(),completion.termination(),completion.outcome(),completion.results(),completion.receipt_refs(),&[])
}
pub(crate) fn finish_kali_locked(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,completion:&crate::kali_bridge::WorkKaliCompletion)->Result<(),String>{finish_backend(conn,s,permit,Backend::Kali,completion.assessment_id(),completion.effect(),completion.termination(),completion.outcome(),completion.results(),completion.receipt_refs(),completion.capture_ids())}
pub(crate) fn finish_permit_locked(conn:&Connection,permit:&EffectPermit<'_>,observed:EffectTermination)->Result<(),String>{
    finish_permit_impl(conn,permit,observed,false)
}
fn finish_permit_impl(conn:&Connection,permit:&EffectPermit<'_>,observed:EffectTermination,proven_recovery:bool)->Result<(),String>{
    let mut lease=checked_lease(conn,permit)?;
    if observed==EffectTermination::NotStarted{
        if lease.effect.is_some(){return Err(error("work_unknown_effect"));}
        if let Some(id)=&lease.attempt_id{let mut a=work_store::attempt(conn,id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
            if !(matches!(a.view.state,AttemptState::Reserved|AttemptState::Dispatching|AttemptState::CancelRequested)||proven_recovery&&a.view.state==AttemptState::Unknown)||a.view.effect.is_some()||a.view.started_at.is_some()||!a.view.capture_ids.is_empty()||!a.view.results.is_empty(){return Err(error("work_unknown_effect"));}
            a.view.state=AttemptState::NotStarted;a.view.finished_at=Some(model::now());a.view.cleanup=CleanupState::Confirmed;a.reserved_capture_bytes=0;a.reserved_critical_events=0;
            let canceled=j.state==JobState::CancelRequested||work_store::head(conn,&j.revision.plan_id)?.control==PlanControl::Canceled;
            j.state=if canceled{JobState::Canceled}else{JobState::AwaitingPreparation};j.generation+=1;j.compiled=None;j.reasons=vec![WorkReason::new(if canceled{WorkReasonCode::WorkCanceled}else{WorkReasonCode::WorkPreparationRequired},Some(j.id.clone()))];work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;
            work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::AttemptFinished,Some(j.id),Some(a.view.id),Some(j.state),Some(WorkReasonCode::WorkPreparationRequired),None)?;
        }
    }
    if observed==EffectTermination::Unknown{
        lease.state=LeaseState::Unknown;save_lease(conn,&lease)?;
        if let Some(id)=&lease.attempt_id{let mut a=work_store::attempt(conn,id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
            if !a.view.state.active(){return Err(error("work_unknown_effect"));}
            if a.view.state!=AttemptState::Unknown||j.state!=JobState::Unknown{a.view.state=AttemptState::Unknown;a.view.cleanup=CleanupState::Unknown;j.state=JobState::Unknown;j.generation+=1;j.reasons=vec![WorkReason::new(WorkReasonCode::WorkReconciliationRequired,Some(a.view.id.clone()))];work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),Some(a.view.id),Some(JobState::Unknown),Some(WorkReasonCode::WorkUnknownEffect),a.view.effect)?;}
        }
    }else{conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1 AND boot_id=?2",params![lease.owner.id,lease.boot_id]).map_err(storage_error)?;}Ok(())
}
fn mark_native_unknown_locked(conn:&Connection,s:&mut Snapshot,permit:&EffectPermit<'_>)->Result<(),String>{
    let lease=checked_lease(conn,permit)?;let Some(EffectRef::NativeExecution{id})=&lease.effect else{return Ok(());};
    let execution=s.executions.iter_mut().find(|e|e.id==*id).ok_or_else(||error("work_storage_unavailable"))?;
    if matches!(execution.status.as_str(),"running"|"canceling"){execution.status="unknown".into();let aid=execution.assessment_id.clone();let plan_id=execution.plan_id.clone();if let Some(plan)=s.plans.iter_mut().find(|p|p.id==plan_id){plan.status="unknown".into();}if let Some(assessment)=s.assessments.iter_mut().find(|a|a.id==aid){assessment.status="unknown".into();assessment.updated_at=model::now();}}
    // A missing completion signal does not prove the producer stopped. Preserve
    // the open prefix; boot/backend reconciliation seals it with its own proof.
    Ok(())
}
pub(crate) fn validate_queued_native_locked(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,plan_id:&str,plan_digest:&str)->Result<NativeWorkContext,String>{
    let lease=checked_lease(conn,permit)?;let id=lease.attempt_id.as_ref().ok_or_else(||error("work_owned_operation"))?;
    let a=work_store::attempt(conn,id)?;let j=work_store::job(conn,&a.view.job_id)?;let h=work_store::head(conn,&j.revision.plan_id)?;
    if lease.state!=LeaseState::Claimed||lease.effect.is_some()||a.view.state!=AttemptState::Dispatching||j.state!=JobState::Dispatching||a.boot_id!=lease.boot_id||j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(id.as_str()){return Err(error("work_revision_changed"));}
    if h.control!=PlanControl::Active||h.head_revision_id!=j.revision.revision_id{return Err(error("work_paused"));}
    let CompiledOperationRef::Native{plan_id:expected,digest:expected_digest,..}=&a.compiled else{return Err(error("work_owned_operation"));};
    if expected!=plan_id||expected_digest!=plan_digest||j.compiled.as_ref()!=Some(&a.compiled){return Err(error("work_revision_changed"));}
    let binding:Option<(String,String)>=conn.query_row("SELECT job_id,digest FROM work_plan_bindings WHERE execution_plan_id=?1",[plan_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage_error)?;
    if binding!=Some((j.id.clone(),plan_digest.into())){return Err(error("work_owned_operation"));}
    check_current_body(s,&work_store::revision(conn,&j.revision.revision_id)?.body)?;
    let p=s.plans.iter().find(|p|p.id==plan_id&&p.digest==plan_digest&&p.assessment_id==j.assessment_id).ok_or_else(||error("work_input_changed"))?;
    let cap=a.reserved_capture_bytes.min(4*1024*1024).min(p.output_limit_ki_b as u64*1024);if cap==0{return Err(error("work_capture_quota"));}
    Ok(NativeWorkContext{attempt_id:id.clone(),max_capture_bytes:cap})
}
pub(crate) fn mark_native_running_locked(conn:&Connection,permit:&EffectPermit<'_>,execution:&crate::model::Execution,capture_id:Option<&str>)->Result<(),String>{
    let Some(id)=permit.attempt_id() else{return Ok(());};let lease=checked_lease(conn,permit)?;
    if lease.effect!=Some(EffectRef::NativeExecution{id:execution.id.clone()}){return Err(error("work_owned_operation"));}
    let mut a=work_store::attempt(conn,id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
    if a.view.state!=AttemptState::Dispatching||j.state!=JobState::Dispatching||execution.assessment_id!=j.assessment_id{return Err(error("work_revision_changed"));}
    a.view.state=AttemptState::Running;a.view.started_at=Some(execution.started_at.clone());a.view.cleanup=CleanupState::Pending;
    j.state=JobState::Running;j.generation+=1;
    if let Some(capture_id)=capture_id{valid_id(capture_id)?;a.view.capture_ids.push(capture_id.into());j.capture_ids.push(capture_id.into());}
    work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;
    work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),Some(a.view.id),Some(JobState::Running),None,Some(EffectRef::NativeExecution{id:execution.id.clone()}))?;Ok(())
}
pub(crate) fn finish_native_locked(conn:&Connection,s:&Snapshot,permit:&EffectPermit<'_>,completion:&crate::executor::NativeCompletion,capture:Option<&crate::capture::CaptureManifest>)->Result<(),String>{
    let Some(id)=permit.attempt_id() else{return Ok(());};let lease=checked_lease(conn,permit)?;let effect=EffectRef::NativeExecution{id:completion.execution.id.clone()};
    if lease.effect.as_ref()!=Some(&effect){return Err(error("work_owned_operation"));}
    let mut a=work_store::attempt(conn,id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
    if !matches!(a.view.state,AttemptState::Running|AttemptState::CancelRequested)||completion.execution.assessment_id!=j.assessment_id{return Err(error("work_revision_changed"));}
    let mut results=vec![];
    if let Some(e)=&completion.evidence{if !s.assessments.iter().find(|a|a.id==j.assessment_id).is_some_and(|assessment|assessment.evidence.iter().any(|stored|stored.id==e.id&&stored.sha256==e.sha256)){return Err(error("work_result_missing"));}results.push(ResultRef::Evidence{id:e.id.clone(),raw_sha256:e.sha256.clone()});}
    let partial=completion.partial||capture.is_some_and(|m|m.state!=crate::capture::CaptureState::Sealed||m.loss_reason.is_some());
    let (attempt_state,job_state)=match completion.execution.status.as_str(){"completed" if results.is_empty()=>(AttemptState::Failed,JobState::Failed),"completed" if partial=>(AttemptState::Partial,JobState::Partial),"completed"=>(AttemptState::Completed,JobState::Completed),"canceled"=>(AttemptState::Canceled,JobState::Canceled),"failed"=>(AttemptState::Failed,JobState::Failed),_=>return Err(error("work_unknown_effect"))};
    a.view.state=attempt_state;a.view.finished_at=completion.execution.finished_at.clone();a.view.cleanup=CleanupState::Confirmed;a.view.results=results.clone();a.reserved_capture_bytes=0;a.reserved_critical_events=0;
    j.state=job_state;j.generation+=1;j.results=results;if completion.execution.status=="completed"&&j.results.is_empty(){j.reasons=vec![WorkReason::new(WorkReasonCode::WorkResultMissing,Some(j.id.clone()))];}
    work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;
    if let Some(capture)=capture{work_store::append_capture_event(conn,capture,true)?;}
    work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::AttemptFinished,Some(j.id.clone()),Some(a.view.id),Some(j.state),None,Some(effect))?;
    release_dependents_locked(conn,s,&j.revision)?;Ok(())
}
pub(crate) fn release_dependents_locked(conn:&Connection,s:&Snapshot,revision_ref:&PlanRevisionRef)->Result<(),String>{
    let h=work_store::head(conn,&revision_ref.plan_id)?;if h.head_revision_id!=revision_ref.revision_id{return Ok(());}
    let revision=work_store::revision(conn,&revision_ref.revision_id)?;let jobs=work_store::jobs_for_plan(conn,&revision_ref.plan_id)?;
    for mut j in jobs.iter().filter(|j|j.revision==*revision_ref&&j.state==JobState::WaitingDependencies).cloned(){
        let n=revision.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_storage_unavailable"))?;
        let ready=n.depends_on.iter().all(|id|jobs.iter().any(|parent|parent.revision==*revision_ref&&parent.node_id==*id&&parent.state==JobState::Completed&&!parent.results.is_empty()));
        if !ready{continue;}
        // Inputs are resolved by identity; never choose a recent result by time.
        for input in n.intent.inputs(){if let WorkInput::DependencyOutput{node_id,..}=input{let parent=jobs.iter().find(|parent|parent.revision==*revision_ref&&parent.node_id==*node_id).ok_or_else(||error("work_result_missing"))?;for result in &parent.results{if let ResultRef::Evidence{id,raw_sha256}=result{if !s.assessments.iter().find(|a|a.id==j.assessment_id).is_some_and(|a|a.evidence.iter().any(|e|e.id==*id&&e.sha256==*raw_sha256)){return Err(error("work_input_changed"));}}}}}
        j.state=JobState::AwaitingPreparation;j.generation+=1;j.reasons.clear();work_store::save_job(conn,&j)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),None,Some(j.state),None,None)?;
    }Ok(())
}
pub(crate) fn sync_native_plan_locked(conn:&Connection,s:&Snapshot,plan_id:&str)->Result<(),String>{
    let job_id:Option<String>=conn.query_row("SELECT job_id FROM work_plan_bindings WHERE execution_plan_id=?1",[plan_id],|r|r.get(0)).optional().map_err(storage_error)?;let Some(job_id)=job_id else{return Ok(());};
    let mut j=work_store::job(conn,&job_id)?;let p=s.plans.iter().find(|p|p.id==plan_id).ok_or_else(||error("work_input_changed"))?;
    if !matches!(&j.compiled,Some(CompiledOperationRef::Native{plan_id:bound,..}) if bound==plan_id){return Ok(());}
    if p.status=="canceling"{
        if let Some(current)=&j.current_attempt{let mut a=work_store::attempt(conn,&current.id)?;if matches!(a.view.state,AttemptState::Running|AttemptState::Dispatching){a.view.state=AttemptState::CancelRequested;work_store::save_attempt(conn,&a)?;}}
        j.state=JobState::CancelRequested;
    }else if matches!(p.status.as_str(),"revoked"|"canceled"){
        if let Some(current)=&j.current_attempt{let mut a=work_store::attempt(conn,&current.id)?;if a.view.effect.is_some()||a.view.state.active(){return Err(error("work_unknown_effect"));}if a.view.state==AttemptState::Reserved{a.view.state=AttemptState::NotStarted;a.view.finished_at=Some(model::now());a.view.cleanup=CleanupState::Confirmed;a.reserved_capture_bytes=0;a.reserved_critical_events=0;work_store::save_attempt(conn,&a)?;}}
        j.state=if p.status=="canceled"{JobState::Canceled}else{JobState::AwaitingPreparation};j.compiled=None;j.reasons=vec![WorkReason::new(if p.status=="canceled"{WorkReasonCode::WorkCanceled}else{WorkReasonCode::WorkPreparationRequired},Some(j.id.clone()))];
    }else{return Ok(());}
    j.generation+=1;work_store::save_job(conn,&j)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),j.current_attempt.map(|a|a.id),Some(j.state),None,None)?;Ok(())
}
pub(crate) fn sync_native_reconcile_locked(conn:&Connection,s:&Snapshot,assessment_id:&str)->Result<(),String>{
    // Direct operations have no workflow head or attempt, but their durable
    // resource lease still needs the same canonical reconciliation proof.
    if let Some(lease)=load_lease(conn)?{if lease.attempt_id.is_none()&&lease.owner.assessment_id.as_deref()==Some(assessment_id){
        if let Some(EffectRef::NativeExecution{id})=&lease.effect{if s.executions.iter().any(|e|e.id==*id&&e.assessment_id==assessment_id&&e.status=="failed"&&e.finished_at.is_some()){
            conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1",[&lease.owner.id]).map_err(storage_error)?;
        }}
    }}
    let Some(h)=work_store::head_for_assessment(conn,assessment_id)? else{return Ok(());};
    for mut a in work_store::attempts_for_plan(conn,&h.plan_id)?.into_iter().filter(|a|a.view.state==AttemptState::Unknown){
        let Some(EffectRef::NativeExecution{id})=&a.view.effect else{continue;};
        let Some(execution)=s.executions.iter().find(|e|e.id==*id&&e.assessment_id==assessment_id&&e.status=="failed"&&e.finished_at.is_some())else{continue;};
        let mut j=work_store::job(conn,&a.view.job_id)?;a.view.state=if a.view.results.is_empty(){AttemptState::Failed}else{AttemptState::Partial};a.view.cleanup=CleanupState::Confirmed;a.view.finished_at=execution.finished_at.clone();a.reserved_capture_bytes=0;a.reserved_critical_events=0;
        j.state=if a.view.results.is_empty(){JobState::Failed}else{JobState::Partial};j.generation+=1;j.results=a.view.results.clone();j.reasons.clear();work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;
        work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::ReconciliationRecorded,Some(j.id),Some(a.view.id.clone()),Some(j.state),None,a.view.effect.clone())?;
        if load_lease(conn)?.is_some_and(|lease|lease.attempt_id.as_ref()==Some(&a.view.id)&&lease.effect==a.view.effect){conn.execute("DELETE FROM work_leases WHERE resource='assessment_work'",[]).map_err(storage_error)?;}
    }Ok(())
}
/// Native admission writes Execution + effect binding in the same transaction,
/// before adapter I/O. Their joint absence is therefore a local NotStarted proof;
/// this rule must never be applied to the separate AI/Kali journals.
pub(crate) fn reconcile_unbound_native_locked(conn:&Connection,s:&mut Snapshot,attempt_id:&str)->Result<work_store::WorkJob,String>{
    let lease=load_lease(conn)?.ok_or_else(||error("work_reconciliation_required"))?;let mut a=work_store::attempt(conn,attempt_id)?;let mut j=work_store::job(conn,&a.view.job_id)?;
    if lease.state!=LeaseState::Unknown||lease.owner.kind!=EffectOwnerKind::Native||lease.attempt_id.as_deref()!=Some(attempt_id)||lease.owner.assessment_id.as_deref()!=Some(j.assessment_id.as_str())||lease.boot_id!=a.boot_id||lease.effect.is_some()||a.view.backend!=Backend::Native||a.view.state!=AttemptState::Unknown||a.view.effect.is_some()||a.view.started_at.is_some()||!a.view.capture_ids.is_empty()||!a.view.results.is_empty()||j.state!=JobState::Unknown||j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(attempt_id){return Err(error("work_reconciliation_required"));}
    let CompiledOperationRef::Native{plan_id,digest,..}=&a.compiled else{return Err(error("work_reconciliation_required"));};
    let binding:Option<String>=conn.query_row("SELECT job_id FROM work_plan_bindings WHERE execution_plan_id=?1 AND digest=?2",params![plan_id,digest],|row|row.get(0)).optional().map_err(storage_error)?;
    if binding.as_deref()!=Some(j.id.as_str())||s.executions.iter().any(|e|e.plan_id==*plan_id){return Err(error("work_unknown_effect"));}
    let p=s.plans.iter_mut().find(|p|p.id==*plan_id&&p.assessment_id==j.assessment_id&&p.digest==*digest).ok_or_else(||error("work_reconciliation_required"))?;
    if !matches!(p.status.as_str(),"proposed"|"approved"|"revoked"){return Err(error("work_unknown_effect"));}p.status="revoked".into();
    if !s.executions.iter().any(|e|e.assessment_id==j.assessment_id&&matches!(e.status.as_str(),"running"|"canceling"|"unknown")){if let Some(assessment)=s.assessments.iter_mut().find(|assessment|assessment.id==j.assessment_id){if matches!(assessment.status.as_str(),"awaiting_approval"|"queued"|"unknown"){assessment.status="blocked".into();assessment.updated_at=model::now();}}}
    a.view.state=AttemptState::NotStarted;a.view.cleanup=CleanupState::Confirmed;a.view.finished_at=Some(model::now());a.reserved_capture_bytes=0;a.reserved_critical_events=0;
    j.state=if work_store::head(conn,&j.revision.plan_id)?.control==PlanControl::Canceled{JobState::Canceled}else{JobState::AwaitingPreparation};j.compiled=None;j.generation+=1;j.reasons=vec![WorkReason::new(WorkReasonCode::WorkPreparationRequired,Some(j.id.clone()))];work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;
    work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::ReconciliationRecorded,Some(j.id.clone()),Some(a.view.id),Some(j.state),Some(WorkReasonCode::WorkPreparationRequired),None)?;
    conn.execute("DELETE FROM work_leases WHERE resource='assessment_work' AND owner_id=?1 AND boot_id=?2",params![lease.owner.id,lease.boot_id]).map_err(storage_error)?;Ok(j)
}
fn recover_unbound_native_direct_locked(conn:&Connection,s:&mut Snapshot,current_boot:&str)->Result<(),String>{
    let Some(lease)=load_lease(conn)? else{return Ok(());};
    if lease.owner.kind!=EffectOwnerKind::Native||lease.attempt_id.is_some()||lease.effect.is_some()||lease.state!=LeaseState::Unknown||lease.boot_id==current_boot{return Ok(());}
    let Some(aid)=lease.owner.assessment_id.as_ref() else{return Ok(());};
    // Native creates its Execution and binds this lease atomically before I/O.
    // The installation lock and a different boot exclude an old local worker.
    if s.executions.iter().any(|e|matches!(e.status.as_str(),"running"|"canceling"|"unknown")){return Ok(());}
    for p in s.plans.iter_mut().filter(|p|p.assessment_id==*aid&&matches!(p.status.as_str(),"proposed"|"approved")){
        if s.executions.iter().any(|e|e.plan_id==p.id){return Err(error("work_reconciliation_required"));}p.status="revoked".into();
    }
    let assessment=s.assessments.iter_mut().find(|a|a.id==*aid).ok_or_else(||error("work_storage_unavailable"))?;
    if matches!(assessment.status.as_str(),"awaiting_approval"|"queued"|"unknown"){assessment.status="blocked".into();assessment.updated_at=model::now();}
    assessment.events.push(model::event("Admissão nativa anterior encerrada sem início: o journal não contém execução nem vínculo de efeito; aprovações pendentes foram revogadas. Nenhum adaptador foi repetido.","warning"));
    release_proven_unbound(conn,current_boot,&lease.owner,&lease.boot_id,None)
}
impl Drop for EffectPermit<'_>{fn drop(&mut self){if !self.finished{let _=self.store.work_transaction(|conn,s|{finish_permit_locked(conn,self,EffectTermination::Unknown)?;mark_native_unknown_locked(conn,s,self)});}}}

/// Startup reconciliation does not dispatch or probe a backend. Canonical
/// service journals have already recovered before this method is called.
#[derive(Default)]
pub struct WorkSupervisor { arbiter:Arc<WorkArbiter> }
impl WorkSupervisor {
    pub(crate) fn arbiter(&self)->&WorkArbiter{&self.arbiter}
    pub(crate) fn shared_arbiter(&self)->Arc<WorkArbiter>{self.arbiter.clone()}
    pub(crate) fn next_dispatch(&self,store:&Store,workflow:&WorkflowService)->Result<Option<QueuedDispatch>,String>{
        loop{
            let candidate=store.work_read(|conn,_|{
                if load_lease(conn)?.is_some(){return Ok(None);}
                let id:Option<String>=conn.query_row("SELECT j.id FROM work_jobs j JOIN work_plan_heads h ON h.plan_id=j.plan_id WHERE j.state='queued' AND h.control='active' AND h.head_revision_id=j.revision_id ORDER BY j.rowid LIMIT 1",[],|r|r.get(0)).optional().map_err(storage_error)?;
                id.map(|id|{let j=work_store::job(conn,&id)?;let current=j.current_attempt.as_ref().ok_or_else(||error("work_storage_unavailable"))?;let a=work_store::attempt(conn,&current.id)?;Ok((j,a))}).transpose()
            })?;
            let Some((candidate,attempt))=candidate else{return Ok(None);};
            if attempt.view.backend==Backend::Native{return self.next_native(store).map(|n|n.map(QueuedDispatch::Native));}
            let grant=workflow.take_backend_grant(&attempt.view.id,&self.arbiter.boot_id)?;
            let token=store.work_transaction(|conn,s|{
                if load_lease(conn)?.is_some(){return Ok(None);}
                let mut j=work_store::job(conn,&candidate.id)?;let h=work_store::head(conn,&j.revision.plan_id)?;let mut a=work_store::attempt(conn,&attempt.view.id)?;
                if j.state!=JobState::Queued||j.generation!=candidate.generation||h.control!=PlanControl::Active||h.head_revision_id!=j.revision.revision_id{return Ok(None);}
                let valid_grant=match (&grant,&a.compiled){
                    (Some(BackendGrant::Ai(grant)),CompiledOperationRef::AiInvestigation{investigation_id,digest,expires_at})=>grant.inspection().id()==investigation_id&&grant.inspection().digest()==digest&&grant.inspection().inputs()==a.input_refs&&a.data_review_ref.as_ref()==Some(grant.inspection().data_review())&&chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|t|t>chrono::Utc::now()),
                    (Some(BackendGrant::Kali(grant)),CompiledOperationRef::KaliEvidence{review_id,digest,expires_at,environment_id,operation})=>grant.review().id==*review_id&&grant.review().digest==*digest&&a.data_review_ref.as_ref()==Some(grant.review())&&grant.expectation().input==a.input_refs[0]&&grant.expectation().environment_id==*environment_id&&grant.expectation().operation==*operation&&chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|t|t>chrono::Utc::now()),_=>false,
                };
                let valid=valid_grant&&a.view.state==AttemptState::Reserved&&a.view.effect.is_none()&&a.boot_id==self.arbiter.boot_id&&j.compiled.as_ref()==Some(&a.compiled)&&check_current_body(s,&work_store::revision(conn,&j.revision.revision_id)?.body).is_ok()&&work_store::resolve_inputs(conn,s,&j).is_ok_and(|inputs|inputs==a.input_refs);
                if !valid{if a.view.state!=AttemptState::Reserved||a.view.effect.is_some(){return Err(error("work_unknown_effect"));}a.view.state=AttemptState::NotStarted;a.view.cleanup=CleanupState::Confirmed;a.view.finished_at=Some(model::now());a.reserved_capture_bytes=0;a.reserved_critical_events=0;j.state=JobState::AwaitingPreparation;j.generation+=1;j.compiled=None;j.reasons=vec![WorkReason::new(WorkReasonCode::WorkPreparationRequired,Some(j.id.clone()))];work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::AttemptFinished,Some(j.id),Some(a.view.id),Some(JobState::AwaitingPreparation),Some(WorkReasonCode::WorkPreparationRequired),None)?;return Ok(None);}
                Ok(Some(QueuedAttemptToken{job_id:j.id,attempt_id:a.view.id,generation:j.generation,boot_id:self.arbiter.boot_id.clone()}))
            })?;
            if let Some(token)=token{return match grant{Some(BackendGrant::Ai(grant))=>Ok(Some(QueuedDispatch::Ai{token,grant})),Some(BackendGrant::Kali(grant))=>Ok(Some(QueuedDispatch::Kali{token,grant})),None=>Err(error("work_storage_unavailable"))};}
            if store.work_read(|conn,_|Ok(work_store::job(conn,&candidate.id)?.state==JobState::Queued))?{return Ok(None);}
        }
    }
    pub(crate) fn next_native(&self,store:&Store)->Result<Option<QueuedNativeDispatch>,String>{
        loop{
            let candidate=store.work_read(|conn,_|{
                if load_lease(conn)?.is_some(){return Ok(None);}
                let mut q=conn.prepare("SELECT j.id FROM work_jobs j JOIN work_plan_heads h ON h.plan_id=j.plan_id WHERE j.state='queued' AND h.control='active' AND h.head_revision_id=j.revision_id ORDER BY j.rowid").map_err(storage_error)?;
                let ids=q.query_map([],|r|r.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;
                for id in ids{let j=work_store::job(conn,&id)?;if matches!(j.compiled,Some(CompiledOperationRef::Native{..})){return Ok(Some(j));}}Ok(None)
            })?;
            let Some(candidate)=candidate else{return Ok(None);};
            let dispatch=store.work_transaction(|conn,s|{
                if load_lease(conn)?.is_some(){return Ok(None);}
                let mut j=work_store::job(conn,&candidate.id)?;let h=work_store::head(conn,&j.revision.plan_id)?;
                if j.state!=JobState::Queued||j.generation!=candidate.generation||h.control!=PlanControl::Active||h.head_revision_id!=j.revision.revision_id{return Ok(None);}
                let current=j.current_attempt.as_ref().ok_or_else(||error("work_storage_unavailable"))?;let mut a=work_store::attempt(conn,&current.id)?;
                let CompiledOperationRef::Native{plan_id,digest,expires_at}=&a.compiled else{return Err(error("work_intent_unavailable"));};
                let valid=a.view.state==AttemptState::Reserved&&a.view.effect.is_none()&&a.boot_id==self.arbiter.boot_id&&j.compiled.as_ref()==Some(&a.compiled)&&chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|t|t>chrono::Utc::now())&&check_current_body(s,&work_store::revision(conn,&j.revision.revision_id)?.body).is_ok()&&work_store::resolve_inputs(conn,s,&j).is_ok_and(|inputs|inputs==a.input_refs)&&s.plans.iter().any(|p|p.id==*plan_id&&p.digest==*digest&&p.status=="approved"&&p.approved_at.is_some()&&p.timeout_seconds==s.settings.timeout_seconds&&p.output_limit_ki_b==s.settings.output_limit_ki_b);
                if !valid{
                    if a.view.effect.is_some()||a.view.state.active(){return Err(error("work_unknown_effect"));}
                    if let Some(p)=s.plans.iter_mut().find(|p|p.id==*plan_id){if matches!(p.status.as_str(),"proposed"|"approved"){p.status="revoked".into();}let aid=p.assessment_id.clone();if let Some(assessment)=s.assessments.iter_mut().find(|assessment|assessment.id==aid){if matches!(assessment.status.as_str(),"queued"|"awaiting_approval"){assessment.status="blocked".into();assessment.updated_at=model::now();}}}
                    a.view.state=AttemptState::NotStarted;a.view.cleanup=CleanupState::Confirmed;a.view.finished_at=Some(model::now());a.reserved_capture_bytes=0;a.reserved_critical_events=0;
                    j.state=JobState::AwaitingPreparation;j.generation+=1;j.compiled=None;j.reasons=vec![WorkReason::new(WorkReasonCode::WorkPreparationRequired,Some(j.id.clone()))];work_store::save_attempt(conn,&a)?;work_store::save_job(conn,&j)?;
                    work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::AttemptFinished,Some(j.id),Some(a.view.id),Some(j.state),Some(WorkReasonCode::WorkPreparationRequired),None)?;return Ok(None);
                }
                Ok(Some(QueuedNativeDispatch{plan_id:plan_id.clone(),digest:digest.clone(),token:QueuedAttemptToken{job_id:j.id,attempt_id:a.view.id,generation:j.generation,boot_id:self.arbiter.boot_id.clone()}}))
            })?;
            if dispatch.is_some(){return Ok(dispatch);}
            // No waiting or polling: only continue over the finite queue after
            // an invalid approval has been durably closed as not started.
            let still_queued=store.work_read(|conn,_|Ok(work_store::job(conn,&candidate.id)?.state==JobState::Queued))?;if still_queued{return Ok(None);}
        }
    }
    pub fn recover_after_boot(&self,store:&Store)->Result<(),String>{
        // Opening a legacy profile must not rewrite its canonical JSON merely
        // because the new supervisor exists; its additive tables can be empty.
        let has_work=store.work_read(|conn,_|conn.query_row("SELECT EXISTS(SELECT 1 FROM work_plan_heads) OR EXISTS(SELECT 1 FROM work_leases) OR EXISTS(SELECT 1 FROM work_commands)",[],|r|r.get::<_,bool>(0)).map_err(storage_error))?;
        if !has_work{return Ok(());}
        store.work_transaction(|conn,s|{
            let mut q=conn.prepare("SELECT plan_id FROM work_plan_heads ORDER BY rowid").map_err(storage_error)?;let ids=q.query_map([],|r|r.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;drop(q);
            for id in ids{
                let h=work_store::head(conn,&id)?;
                if h.control==PlanControl::Active{conn.execute("UPDATE work_plan_heads SET control='paused',generation=generation+1 WHERE plan_id=?1",[&id]).map_err(storage_error)?;work_store::append_event(conn,&id,&h.assessment_id,WorkEventType::PlanPaused,None,None,None,Some(WorkReasonCode::WorkPreparationRequired),None)?;}
                for mut j in work_store::jobs_for_plan(conn,&id)?{
                    let mut changed=false;
                    if matches!(j.state,JobState::Preparing|JobState::AwaitingApproval|JobState::Queued){
                        if let Some(CompiledOperationRef::Native{plan_id,..})=&j.compiled{
                            let p=s.plans.iter_mut().find(|p|p.id==*plan_id).ok_or_else(||error("work_storage_unavailable"))?;
                            if matches!(p.status.as_str(),"proposed"|"approved"|"revoked"){
                                p.status="revoked".into();let aid=p.assessment_id.clone();
                                let unresolved=s.executions.iter().any(|e|e.assessment_id==aid&&matches!(e.status.as_str(),"running"|"canceling"|"unknown"));
                                if !unresolved{if let Some(a)=s.assessments.iter_mut().find(|a|a.id==aid){if matches!(a.status.as_str(),"awaiting_approval"|"queued"|"unknown"){a.status="blocked".into();a.updated_at=model::now();}}}
                            }
                        }
                    }
                    if let Some(current)=&j.current_attempt{let mut a=work_store::attempt(conn,&current.id)?;
                        if a.view.state.active(){
                            if a.view.backend==Backend::Native{for id in &a.view.capture_ids{let capture=crate::capture::load_locked(conn,id)?;if capture.state==crate::capture::CaptureState::Open{let sealed=crate::capture::seal_locked(conn,id,None,Some(crate::capture::CaptureLossReason::Interrupted),true)?;work_store::append_capture_event(conn,&sealed,true)?;}}}
                            a.reserved_critical_events=work_store::attempt(conn,&a.view.id)?.reserved_critical_events;
                            if a.view.state!=AttemptState::Unknown||j.state!=JobState::Unknown{a.view.state=AttemptState::Unknown;a.view.cleanup=CleanupState::Unknown;j.state=JobState::Unknown;changed=true;}
                        }
                        else if a.view.state==AttemptState::Reserved{a.view.state=AttemptState::NotStarted;a.view.finished_at=Some(model::now());a.view.cleanup=CleanupState::Confirmed;a.reserved_capture_bytes=0;a.reserved_critical_events=0;j.state=JobState::AwaitingPreparation;j.compiled=None;changed=true;}
                        if changed{work_store::save_attempt(conn,&a)?;}
                    }
                    if matches!(j.state,JobState::Preparing|JobState::AwaitingApproval|JobState::Queued){j.state=JobState::AwaitingPreparation;j.compiled=None;changed=true;}
                    if j.state==JobState::CancelRequested&&!work_store::job_has_unsettled_attempt(conn,&j)?{j.state=JobState::Canceled;j.compiled=None;changed=true;}
                    if changed{j.generation+=1;j.reasons=vec![WorkReason::new(if j.state==JobState::Unknown{WorkReasonCode::WorkReconciliationRequired}else{WorkReasonCode::WorkPreparationRequired},Some(j.id.clone()))];work_store::save_job(conn,&j)?;work_store::append_event(conn,&id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id),j.current_attempt.map(|a|a.id),Some(j.state),Some(WorkReasonCode::WorkReconciliationRequired),None)?;}
                }
            }
            let mut q=conn.prepare("SELECT assessment_id,idempotency_key FROM work_commands ORDER BY rowid").map_err(storage_error)?;let keys=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;drop(q);
            for (aid,key) in keys{let mut command=work_store::command_by_key(conn,&aid,&key)?.ok_or_else(||error("work_storage_unavailable"))?;if let CommandResult::Pending{receipt,entity_id}=&command.result{command.result=CommandResult::Failed{receipt:receipt.clone(),entity_id:entity_id.clone(),reason_code:WorkReasonCode::WorkReconciliationRequired};work_store::save_command(conn,&command)?;}}
            if let Some(mut lease)=load_lease(conn)?{if lease.boot_id!=self.arbiter.boot_id{lease.state=LeaseState::Unknown;save_lease(conn,&lease)?;}}
            recover_unbound_native_direct_locked(conn,s,&self.arbiter.boot_id)?;
            Ok(())
        })
    }
}
