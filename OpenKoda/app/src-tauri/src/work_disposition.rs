//! Local ownership closure is not a statement about remote termination or cost.
//! Compact receipts replace a lease's reserved metadata; they never grant work.
use crate::{model::{self,Snapshot},workflow::*,work_store};
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};

pub(crate) const MAX_RECORDS:u64=1000;
// Direct leases already reserve 4 KiB. Keep 512 bytes for the bounded
// generation/state growth of up to 100 pending jobs/heads during invalidation.
pub(crate) const MAX_RECORD_BYTES:usize=3584;
pub(crate) const NOTICE:&str="Posse local encerrada. Resultado remoto e cobrança continuam desconhecidos; nenhuma chamada foi repetida, cancelada remotamente ou estornada. A fila anterior foi pausada e exige novas preparações.";
pub(crate) const PREPARATION_NOTICE:&str="Esta consulta não encerra operações. Ao confirmar, somente a posse local será encerrada; resultado remoto e cobrança continuarão desconhecidos. A fila anterior será pausada, sem repetição automática, cancelamento remoto ou estorno; novos trabalhos exigirão novas preparações e admissões.";

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum AiLocalClosureTarget{Attempt{attempt_id:String},AiRequest{request_id:String},AiInvestigation{investigation_id:String}}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ClosureLease{pub owner:crate::work_supervisor::EffectOwner,pub boot_id:String,pub effect:EffectRef,#[serde(deserialize_with="required_nullable")]pub attempt_id:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ClosureJob{pub job_id:String,pub attempt_id:String,pub generation:u64,pub revision:PlanRevisionRef}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ClosureCandidate{
    pub target:AiLocalClosureTarget,pub effect:EffectRef,
    #[serde(deserialize_with="required_nullable")]pub assessment_id:Option<String>,
    #[serde(deserialize_with="required_nullable")]pub lease:Option<ClosureLease>,
    #[serde(deserialize_with="required_nullable")]pub job:Option<ClosureJob>,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct QueueClosureSnapshot{pub digest:String,pub active_plans:u32,pub prepared_jobs:u32,pub reserved_attempts:u32,pub native_plans:u32}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct AiLocalDisposition{
    pub schema:String,pub id:String,pub digest:String,pub created_at:String,pub author:String,pub note:String,
    pub owner_profile_fingerprint:String,pub candidate:ClosureCandidate,pub journal_digest:String,
    pub queue:QueueClosureSnapshot,pub review:WorkReviewRef,pub idempotency_key:String,pub request_digest:String,
    pub local_ownership_closed:bool,pub remote_outcome_unknown:bool,pub financial_disposition:String,
}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct LocalClosurePrepare{pub target:AiLocalClosureTarget}
#[derive(Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct LocalClosureCommit{pub review:WorkReviewRef,pub idempotency_key:String,pub note:String,pub acknowledge_remote_unknown:bool,pub acknowledge_no_replay:bool,pub acknowledge_queue_invalidation:bool}

pub(crate) fn profile_fingerprint(root:&std::path::Path)->Result<String,String>{
    let path=std::fs::canonicalize(root).map_err(storage_error)?;let path=path.to_str().ok_or_else(||error("work_storage_unavailable"))?;
    #[cfg(windows)]let path=path.to_lowercase();
    Ok(crate::core::hash(&format!("openkoda.ai-local-profile.v1\n{path}")))
}
pub(crate) fn record_digest(record:&AiLocalDisposition)->Result<String,String>{let mut copy=record.clone();copy.digest.clear();digest(&copy)}
pub(crate) fn records(conn:&Connection)->Result<Vec<AiLocalDisposition>,String>{
    let mut q=conn.prepare("SELECT data FROM work_ai_dispositions ORDER BY rowid").map_err(storage_error)?;
    let rows=q.query_map([],|row|row.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;
    if rows.len()>MAX_RECORDS as usize{return Err(error("work_limit"));}
    rows.into_iter().map(|raw|{if raw.len()>MAX_RECORD_BYTES{return Err(error("work_limit"));}serde_json::from_str(&raw).map_err(storage_error)}).collect()
}
pub(crate) fn replay(conn:&Connection,key:&str,request_digest:&str)->Result<Option<Value>,String>{
    let raw:Option<String>=conn.query_row("SELECT data FROM work_ai_dispositions WHERE idempotency_key=?1",[key],|row|row.get(0)).optional().map_err(storage_error)?;
    raw.map(|raw|{let record:AiLocalDisposition=serde_json::from_str(&raw).map_err(storage_error)?;if record.request_digest!=request_digest{return Err(error("work_idempotency_conflict"));}Ok(public_record(&record,true))}).transpose()
}
pub(crate) fn public_record(record:&AiLocalDisposition,replayed:bool)->Value{json!({"replayed":replayed,"receipt":{"id":record.id,"action":"work.aiLocalClosure.commit","createdAt":record.created_at,"requestDigest":record.request_digest},"value":{"id":record.id,"digest":record.digest,"createdAt":record.created_at,"target":record.candidate.target,"effect":record.candidate.effect,"assessmentId":record.candidate.assessment_id,"localOwnershipClosed":true,"remoteOutcomeUnknown":true,"financialDisposition":"unchanged","queue":record.queue,"notice":NOTICE}})}
pub(crate) fn reserve_record_slot(conn:&Connection)->Result<(),String>{let count:u64=conn.query_row("SELECT count(*) FROM work_ai_dispositions",[],|row|row.get(0)).map_err(storage_error)?;if count>=MAX_RECORDS{return Err(error("work_limit"));}Ok(())}
fn strings(conn:&Connection,sql:&str)->Result<Vec<String>,String>{let mut q=conn.prepare(sql).map_err(storage_error)?;let rows=q.query_map([],|row|row.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;Ok(rows)}
pub(crate) fn queue_snapshot(conn:&Connection,s:&Snapshot)->Result<QueueClosureSnapshot,String>{
    let heads=strings(conn,"SELECT json_array(plan_id,assessment_id,head_revision_id,generation,control) FROM work_plan_heads ORDER BY plan_id")?;
    let jobs=strings(conn,"SELECT data FROM work_jobs ORDER BY id")?;let attempts=strings(conn,"SELECT data FROM work_attempts ORDER BY id")?;
    let commands=strings(conn,"SELECT result FROM work_commands ORDER BY id")?;
    let native=s.plans.iter().filter(|plan|matches!(plan.status.as_str(),"proposed"|"approved")).collect::<Vec<_>>();
    Ok(QueueClosureSnapshot{digest:digest(&json!({"heads":heads,"jobs":jobs,"attempts":attempts,"commands":commands,"nativePlans":native}))?,active_plans:conn.query_row("SELECT count(*) FROM work_plan_heads h WHERE h.control='active' AND EXISTS(SELECT 1 FROM work_jobs j WHERE j.plan_id=h.plan_id AND j.state NOT IN ('completed','partial','failed','canceled','revoked','superseded','closed_locally'))",[],|r|r.get(0)).map_err(storage_error)?,prepared_jobs:conn.query_row("SELECT count(*) FROM work_jobs WHERE state IN ('preparing','awaiting_approval','queued')",[],|r|r.get(0)).map_err(storage_error)?,reserved_attempts:conn.query_row("SELECT count(*) FROM work_attempts WHERE state='reserved'",[],|r|r.get(0)).map_err(storage_error)?,native_plans:native.len() as u32})
}
pub(crate) fn commit_locked(conn:&Connection,s:&mut Snapshot,permit:&crate::work_supervisor::AiLocalClosurePermit<'_>,proof:&crate::ai::AiLocalClosureProof,record:&AiLocalDisposition)->Result<Value,String>{
    if let Some(value)=replay(conn,&record.idempotency_key,&record.request_digest)?{return Ok(value);}
    reserve_record_slot(conn)?;
    if records(conn)?.iter().any(|old|old.review.id==record.review.id){return Err(error("work_review_used"));}
    if &record.candidate!=permit.candidate()||record.journal_digest!=proof.journal_digest()||record.owner_profile_fingerprint!=proof.profile_fingerprint()||record.queue!=queue_snapshot(conn,s)?||record.digest!=record_digest(record)?{return Err(error("work_revision_changed"));}
    crate::work_supervisor::validate_ai_local_closure_locked(conn,s,permit)?;
    // Pausing all heads is part of the reviewed decision. No dispatcher can
    // inherit authority from a formerly queued grant after this transaction.
    for id in strings(conn,"SELECT plan_id FROM work_plan_heads ORDER BY plan_id")?{
        let head=work_store::head(conn,&id)?;
        let jobs=work_store::jobs_for_plan(conn,&id)?;
        if head.control==PlanControl::Active&&jobs.iter().any(|job|!job.state.terminal()){conn.execute("UPDATE work_plan_heads SET control='paused',generation=generation+1 WHERE plan_id=?1",[&id]).map_err(storage_error)?;work_store::append_event(conn,&id,&head.assessment_id,WorkEventType::PlanPaused,None,None,None,Some(WorkReasonCode::WorkPreparationRequired),None)?;}
        for mut job in jobs{
            if record.candidate.job.as_ref().is_some_and(|owned|owned.job_id==job.id){
                let owned=record.candidate.job.as_ref().unwrap();let mut attempt=work_store::attempt(conn,&owned.attempt_id)?;
                attempt.view.state=AttemptState::ClosedLocally;attempt.view.cleanup=CleanupState::Unknown;attempt.reserved_capture_bytes=0;attempt.reserved_critical_events=0;
                attempt.view.reasons=vec![WorkReason::new(WorkReasonCode::WorkRemoteEffectUnresolved,Some(record.id.clone()))];
                job.state=JobState::ClosedLocally;job.reasons=attempt.view.reasons.clone();job.generation+=1;
                work_store::save_attempt(conn,&attempt)?;work_store::save_job(conn,&job)?;
                work_store::append_event(conn,&id,&job.assessment_id,WorkEventType::ReconciliationRecorded,Some(job.id),Some(attempt.view.id),Some(JobState::ClosedLocally),Some(WorkReasonCode::WorkRemoteEffectUnresolved),Some(record.candidate.effect.clone()))?;
            }else{
                if job.state.active(){return Err(error("work_busy"));}
                if job.state==JobState::Preparing{let pending:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM work_commands WHERE json_extract(result,'$.phase')='pending' AND json_extract(result,'$.entityId')=?1)",[&job.id],|r|r.get(0)).map_err(storage_error)?;if !pending{return Err(error("work_reconciliation_required"));}}
                if job.state==JobState::Queued&&job.current_attempt.as_ref().is_none_or(|current|!work_store::attempt(conn,&current.id).is_ok_and(|attempt|attempt.view.state==AttemptState::Reserved)){return Err(error("work_reconciliation_required"));}
                if let Some(current)=&job.current_attempt{let mut attempt=work_store::attempt(conn,&current.id)?;if attempt.view.state==AttemptState::Reserved{if attempt.view.effect.is_some(){return Err(error("work_unknown_effect"));}attempt.view.state=AttemptState::NotStarted;attempt.view.cleanup=CleanupState::Confirmed;attempt.view.finished_at=Some(model::now());attempt.reserved_capture_bytes=0;attempt.reserved_critical_events=0;work_store::save_attempt(conn,&attempt)?;}}
                if matches!(job.state,JobState::Preparing|JobState::AwaitingApproval|JobState::Queued){job.state=JobState::AwaitingPreparation;job.compiled=None;job.generation+=1;job.reasons.clear();work_store::save_job(conn,&job)?;}
            }
        }
    }
    // Pending preparation intents cannot be replayed as fresh approvals.
    for raw in strings(conn,"SELECT result FROM work_commands")?{let result:work_store::CommandResult=serde_json::from_str(&raw).map_err(storage_error)?;if let work_store::CommandResult::Pending{receipt,entity_id}=result{let failed=work_store::CommandResult::Failed{receipt:receipt.clone(),entity_id,reason_code:WorkReasonCode::WorkPreparationRequired};conn.execute("UPDATE work_commands SET result=?1 WHERE id=?2",params![encode(&failed)?,receipt.id]).map_err(storage_error)?;}}
    for plan in &mut s.plans{if matches!(plan.status.as_str(),"proposed"|"approved"){plan.status="revoked".into();}}
    for assessment in &mut s.assessments{if matches!(assessment.status.as_str(),"awaiting_approval"|"queued"){assessment.status="blocked".into();assessment.updated_at=model::now();}}
    let encoded=encode(record)?;if encoded.len()>MAX_RECORD_BYTES{return Err(error("work_limit"));}
    conn.execute("INSERT INTO work_ai_dispositions(id,idempotency_key,review_id,assessment_id,data) VALUES(?1,?2,?3,?4,?5)",params![record.id,record.idempotency_key,record.review.id,record.candidate.assessment_id,encoded]).map_err(storage_error)?;
    crate::work_supervisor::release_ai_local_closure_locked(conn,permit)?;
    Ok(public_record(record,false))
}
pub(crate) fn validate(conn:&Connection,s:&Snapshot)->Result<(),String>{
    let records=records(conn)?;let mut ids=std::collections::BTreeSet::new();
    for record in &records{
        validate_record_shape(record)?;validate_record_links(conn,record)?;
        if !ids.insert(&record.id){return Err(error("work_storage_unavailable"));}
        if let Some(aid)=&record.candidate.assessment_id{valid_id(aid)?;if !s.assessments.iter().any(|a|a.id==*aid){return Err(error("work_storage_unavailable"));}}
    }
    for raw in strings(conn,"SELECT data FROM work_attempts WHERE state='closed_locally'")?{let a:work_store::WorkAttempt=serde_json::from_str(&raw).map_err(storage_error)?;if a.view.backend!=Backend::Ai||a.view.cleanup!=CleanupState::Unknown||!records.iter().any(|r|r.candidate.job.as_ref().is_some_and(|j|j.attempt_id==a.view.id)){return Err(error("work_storage_unavailable"));}}
    Ok(())
}
fn validate_record_shape(record:&AiLocalDisposition)->Result<(),String>{
    for id in [&record.id,&record.idempotency_key,&record.review.id]{valid_id(id)?;}valid_id(record.candidate.effect.id())?;
    if record.schema!="openkoda.ai-local-disposition.v1"||record.author!="local-user"||encode(record)?.len()>MAX_RECORD_BYTES||record.digest!=record_digest(record)?||!valid_hash(&record.journal_digest)||!valid_hash(&record.owner_profile_fingerprint)||!valid_hash(&record.queue.digest)||!valid_hash(&record.request_digest)||!valid_hash(&record.review.digest)||!valid_time(&record.created_at)||!valid_time(&record.review.expires_at)||!plain_text(&record.note,256)||record.note.len()>1024||!record.local_ownership_closed||!record.remote_outcome_unknown||record.financial_disposition!="unchanged"{return Err(error("work_storage_unavailable"));}
    let candidate=&record.candidate;
    match (&candidate.target,&candidate.effect,&candidate.job){
        (AiLocalClosureTarget::AiRequest{request_id},EffectRef::AiRequest{id},None) if request_id==id=>{},
        (AiLocalClosureTarget::AiInvestigation{investigation_id},EffectRef::AiInvestigation{id},None) if investigation_id==id&&candidate.assessment_id.is_some()=>{},
        (AiLocalClosureTarget::Attempt{attempt_id},EffectRef::AiInvestigation{..},Some(job)) if attempt_id==&job.attempt_id&&candidate.assessment_id.is_some()=>{},
        _=>return Err(error("work_storage_unavailable")),
    }
    if let Some(aid)=&candidate.assessment_id{valid_id(aid)?;}
    if let Some(job)=&candidate.job{for id in [&job.job_id,&job.attempt_id,&job.revision.plan_id,&job.revision.revision_id]{valid_id(id)?;}if job.generation==0||job.generation>MAX_SAFE_INTEGER||job.revision.number==0||job.revision.number>MAX_REVISIONS||!valid_hash(&job.revision.digest){return Err(error("work_storage_unavailable"));}}
    if let Some(lease)=&candidate.lease{
        valid_id(&lease.boot_id)?;valid_id(&lease.owner.id)?;
        if lease.owner.id!=candidate.effect.id()||lease.effect!=candidate.effect||lease.owner.assessment_id!=candidate.assessment_id||lease.attempt_id.as_deref()!=candidate.job.as_ref().map(|j|j.attempt_id.as_str())||!matches!((&candidate.effect,lease.owner.kind),(EffectRef::AiRequest{..},crate::work_supervisor::EffectOwnerKind::AiRequestIntent)|(EffectRef::AiInvestigation{..},crate::work_supervisor::EffectOwnerKind::AiInvestigationIntent)){return Err(error("work_storage_unavailable"));}
    }
    let input=LocalClosureCommit{review:record.review.clone(),idempotency_key:record.idempotency_key.clone(),note:record.note.clone(),acknowledge_remote_unknown:true,acknowledge_no_replay:true,acknowledge_queue_invalidation:true};
    if record.request_digest!=digest(&json!({"action":"work.aiLocalClosure.commit","payload":input}))?{return Err(error("work_storage_unavailable"));}Ok(())
}
fn validate_record_links(conn:&Connection,record:&AiLocalDisposition)->Result<(),String>{
    if let Some(job)=&record.candidate.job{let a=work_store::attempt(conn,&job.attempt_id)?;let j=work_store::job(conn,&job.job_id)?;if a.view.backend!=Backend::Ai||a.view.state!=AttemptState::ClosedLocally||a.view.cleanup!=CleanupState::Unknown||a.view.job_id!=job.job_id||j.state!=JobState::ClosedLocally||j.revision!=job.revision||a.view.effect.as_ref()!=Some(&record.candidate.effect)||Some(&j.assessment_id)!=record.candidate.assessment_id.as_ref(){return Err(error("work_storage_unavailable"));}}
    else if let EffectRef::AiInvestigation{id}=&record.candidate.effect{if work_store::investigation_owner_locked(conn,id)?.is_some(){return Err(error("work_storage_unavailable"));}}
    let row:(String,String,Option<String>)=conn.query_row("SELECT idempotency_key,review_id,assessment_id FROM work_ai_dispositions WHERE id=?1",[&record.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(storage_error)?;if row!=(record.idempotency_key.clone(),record.review.id.clone(),record.candidate.assessment_id.clone()){return Err(error("work_storage_unavailable"));}
    let live:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM work_leases WHERE effect_kind=?1 AND effect_id=?2)",params![record.candidate.effect.kind(),record.candidate.effect.id()],|r|r.get(0)).map_err(storage_error)?;if live{return Err(error("work_storage_unavailable"));}Ok(())
}
pub(crate) fn read_records(root:&std::path::Path)->Result<Vec<AiLocalDisposition>,String>{
    let path=root.join("openkoda.sqlite3");crate::recovery::require_plain(&path,false)?;let conn=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(storage_error)?;conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(storage_error)?;conn.execute_batch("BEGIN").map_err(storage_error)?;
    let expected=work_store::WORK_SCHEMA.iter().find(|(name,_)|*name=="work_ai_dispositions").ok_or_else(||error("work_storage_unavailable"))?.1.replace("CREATE TABLE IF NOT EXISTS ","CREATE TABLE ");
    let actual:Option<String>=conn.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='work_ai_dispositions'",[],|r|r.get(0)).optional().map_err(storage_error)?;if actual.as_deref()!=Some(expected.as_str()){return Err(error("work_storage_unavailable"));}
    let records=records(&conn)?;
    for record in &records{
        validate_record_shape(record)?;validate_record_links(&conn,record)?;
        // Evaluate membership in SQLite, without loading evidence, notes,
        // credentials or the canonical document into this metadata-only gate.
        if let Some(aid)=&record.candidate.assessment_id{let belongs:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM app_state,json_each(app_state.json,'$.assessments') a WHERE json_extract(a.value,'$.id')=?1)",[aid],|r|r.get(0)).map_err(storage_error)?;if !belongs{return Err(error("work_storage_unavailable"));}}
    }
    Ok(records)
}
