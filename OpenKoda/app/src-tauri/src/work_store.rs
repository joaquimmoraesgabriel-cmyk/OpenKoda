//! Closed workflow journal in the canonical SQLite database. Callers already
//! hold Store's transaction; these helpers never reacquire Store or perform I/O.
use crate::{model::{self, Snapshot}, workflow::*};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

macro_rules! schema {
    ($(($name:literal,$sql:literal)),+ $(,)?) => {
        pub(crate) const WORK_SCHEMA: &[(&str,&str)] = &[$(($name,$sql)),+];
        pub(crate) const WORK_SCHEMA_SQL: &str = concat!($($sql,";\n"),+);
    };
}
schema!(
    ("work_plan_heads", "CREATE TABLE IF NOT EXISTS work_plan_heads(plan_id TEXT PRIMARY KEY,assessment_id TEXT NOT NULL UNIQUE,head_revision_id TEXT NOT NULL REFERENCES work_plan_revisions(id) DEFERRABLE INITIALLY DEFERRED,generation INTEGER NOT NULL CHECK(generation>0),control TEXT NOT NULL CHECK(control IN ('active','paused','canceled')))"),
    ("work_plan_revisions", "CREATE TABLE IF NOT EXISTS work_plan_revisions(id TEXT PRIMARY KEY,plan_id TEXT NOT NULL REFERENCES work_plan_heads(plan_id) DEFERRABLE INITIALLY DEFERRED,number INTEGER NOT NULL CHECK(number BETWEEN 1 AND 20),assessment_id TEXT NOT NULL,data TEXT NOT NULL,UNIQUE(plan_id,number))"),
    ("work_jobs", "CREATE TABLE IF NOT EXISTS work_jobs(id TEXT PRIMARY KEY,plan_id TEXT NOT NULL REFERENCES work_plan_heads(plan_id),revision_id TEXT NOT NULL REFERENCES work_plan_revisions(id),node_id TEXT NOT NULL,assessment_id TEXT NOT NULL,generation INTEGER NOT NULL CHECK(generation>0),state TEXT NOT NULL,data TEXT NOT NULL,UNIQUE(revision_id,node_id))"),
    ("work_attempts", "CREATE TABLE IF NOT EXISTS work_attempts(id TEXT PRIMARY KEY,job_id TEXT NOT NULL REFERENCES work_jobs(id),number INTEGER NOT NULL CHECK(number BETWEEN 1 AND 100),state TEXT NOT NULL,effect_kind TEXT,effect_id TEXT,data TEXT NOT NULL,UNIQUE(job_id,number),UNIQUE(effect_kind,effect_id),CHECK((effect_kind IS NULL)=(effect_id IS NULL)))"),
    ("work_plan_bindings", "CREATE TABLE IF NOT EXISTS work_plan_bindings(execution_plan_id TEXT PRIMARY KEY,job_id TEXT NOT NULL REFERENCES work_jobs(id),preparation_key TEXT NOT NULL,digest TEXT NOT NULL)"),
    ("work_events", "CREATE TABLE IF NOT EXISTS work_events(stream_id TEXT NOT NULL REFERENCES work_plan_heads(plan_id),sequence INTEGER NOT NULL CHECK(sequence>0),id TEXT NOT NULL UNIQUE,data TEXT NOT NULL,PRIMARY KEY(stream_id,sequence))"),
    ("work_commands", "CREATE TABLE IF NOT EXISTS work_commands(id TEXT PRIMARY KEY,assessment_id TEXT NOT NULL,idempotency_key TEXT NOT NULL,action TEXT NOT NULL,request_digest TEXT NOT NULL,review_id TEXT UNIQUE,result TEXT NOT NULL,UNIQUE(assessment_id,idempotency_key))"),
    ("work_leases", "CREATE TABLE IF NOT EXISTS work_leases(resource TEXT PRIMARY KEY CHECK(resource='assessment_work'),owner_id TEXT NOT NULL,attempt_id TEXT REFERENCES work_attempts(id),effect_kind TEXT,effect_id TEXT,boot_id TEXT NOT NULL,state TEXT NOT NULL,data TEXT NOT NULL,CHECK((effect_kind IS NULL)=(effect_id IS NULL)))"),
    ("work_ai_dispositions", "CREATE TABLE IF NOT EXISTS work_ai_dispositions(id TEXT PRIMARY KEY,idempotency_key TEXT NOT NULL UNIQUE,review_id TEXT NOT NULL UNIQUE,assessment_id TEXT,data TEXT NOT NULL)")
);

#[derive(Clone, Debug)]
pub(crate) struct PlanHead { pub plan_id:String, pub assessment_id:String, pub head_revision_id:String, pub generation:u64, pub control:PlanControl }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct WorkJob {
    pub id:String, pub assessment_id:String, pub revision:PlanRevisionRef, pub node_id:String,
    pub generation:u64, pub state:JobState, pub created_at:String, pub reasons:Vec<WorkReason>,
    #[serde(deserialize_with="required_nullable")] pub compiled:Option<CompiledOperationRef>,
    #[serde(deserialize_with="required_nullable")] pub current_attempt:Option<AttemptRef>,
    pub results:Vec<ResultRef>, pub capture_ids:Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct WorkAttempt {
    pub view:AttemptView, pub boot_id:String, pub reserved_capture_bytes:u64,
    pub reserved_critical_events:u32, pub compiled:CompiledOperationRef, pub input_refs:Vec<ResultRef>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub reserved_effect:Option<EffectRef>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub data_review_ref:Option<crate::data_policy::DataReviewRef>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub kali_binding:Option<crate::kali_bridge::KaliWorkBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkPlanBindingView { pub execution_plan_id:String, pub job_id:String, pub plan_id:String, pub revision_id:String }
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct WorkInvestigationOwner {pub job_id:String,pub plan_id:String,pub revision_id:String}

// A pending command is a durable preparation intent, never a resumable grant.
// Completed payloads contain only references; responses are projected afresh.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub(crate) enum CommandOutput {
    Plan { plan_id:String, revision_id:String },
    Preparation { job_id:String, compiled:CompiledOperationRef, resolved_inputs:Vec<ResultRef> },
    BackendPreparation {job_id:String,backend:Backend,intent_digest:String,resolved_inputs:Vec<ResultRef>,context_selection:WorkContextSelection,#[serde(deserialize_with="required_nullable")]environment_id:Option<String>},
    InvestigationBinding {job_id:String,compiled:CompiledOperationRef,resolved_inputs:Vec<ResultRef>},
    Job { job_id:String }, HumanReview { record:HumanReviewRecord }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag="phase",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub(crate) enum CommandResult {
    Pending { receipt:CommandReceipt, entity_id:String },
    Completed { receipt:CommandReceipt, output:CommandOutput },
    Failed { receipt:CommandReceipt, entity_id:String, reason_code:WorkReasonCode },
}
impl CommandResult { pub fn receipt(&self)->&CommandReceipt { match self {Self::Pending{receipt,..}|Self::Completed{receipt,..}|Self::Failed{receipt,..}=>receipt} } }
#[derive(Clone, Debug)]
pub(crate) struct StoredCommand { pub assessment_id:String, pub idempotency_key:String, pub review_id:Option<String>, pub result:CommandResult }

fn parse<T:serde::de::DeserializeOwned>(raw:String)->Result<T,String>{if raw.len()>MAX_METADATA_BYTES{return Err(error("work_limit"));}serde_json::from_str(&raw).map_err(storage_error)}
fn db_enum<T:serde::de::DeserializeOwned>(v:&str)->Result<T,String>{serde_json::from_value(json!(v)).map_err(storage_error)}
fn enum_text<T:Serialize>(v:&T)->Result<String,String>{serde_json::to_value(v).map_err(storage_error)?.as_str().map(str::to_owned).ok_or_else(||error("work_storage_unavailable"))}
fn raws(conn:&Connection,sql:&str)->Result<Vec<String>,String>{let mut q=conn.prepare(sql).map_err(storage_error)?;let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(storage_error)?;let mut result=vec![];let mut bytes=0;for row in rows{let raw=row.map_err(storage_error)?;bytes+=raw.len();if bytes>MAX_METADATA_BYTES{return Err(error("work_limit"));}result.push(raw);}Ok(result)}
pub(crate) fn head_for_assessment(conn:&Connection,aid:&str)->Result<Option<PlanHead>,String>{
    let row=conn.query_row("SELECT plan_id,assessment_id,head_revision_id,generation,control FROM work_plan_heads WHERE assessment_id=?1",[aid],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,u64>(3)?,r.get::<_,String>(4)?))).optional().map_err(storage_error)?;
    row.map(|(plan_id,assessment_id,head_revision_id,generation,control)|Ok(PlanHead{plan_id,assessment_id,head_revision_id,generation,control:db_enum(&control)?})).transpose()
}
pub(crate) fn head(conn:&Connection,plan_id:&str)->Result<PlanHead,String>{let aid=conn.query_row("SELECT assessment_id FROM work_plan_heads WHERE plan_id=?1",[plan_id],|r|r.get::<_,String>(0)).optional().map_err(storage_error)?.ok_or_else(||error("work_invalid_request"))?;head_for_assessment(conn,&aid)?.ok_or_else(||error("work_storage_unavailable"))}
pub(crate) fn revision(conn:&Connection,id:&str)->Result<ActionPlanRevision,String>{parse(conn.query_row("SELECT data FROM work_plan_revisions WHERE id=?1",[id],|r|r.get(0)).optional().map_err(storage_error)?.ok_or_else(||error("work_revision_changed"))?)}
pub(crate) fn job(conn:&Connection,id:&str)->Result<WorkJob,String>{parse(conn.query_row("SELECT data FROM work_jobs WHERE id=?1",[id],|r|r.get(0)).optional().map_err(storage_error)?.ok_or_else(||error("work_invalid_request"))?)}
pub(crate) fn jobs_for_plan(conn:&Connection,id:&str)->Result<Vec<WorkJob>,String>{let mut q=conn.prepare("SELECT data FROM work_jobs WHERE plan_id=?1 ORDER BY rowid").map_err(storage_error)?;let rows=q.query_map([id],|r|r.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;rows.into_iter().map(parse).collect()}
pub(crate) fn attempt(conn:&Connection,id:&str)->Result<WorkAttempt,String>{parse(conn.query_row("SELECT data FROM work_attempts WHERE id=?1",[id],|r|r.get(0)).optional().map_err(storage_error)?.ok_or_else(||error("work_invalid_request"))?)}
pub(crate) fn job_has_unsettled_attempt(conn:&Connection,j:&WorkJob)->Result<bool,String>{j.current_attempt.as_ref().map(|r|attempt(conn,&r.id).map(|a|a.view.state.active()||a.view.state==AttemptState::Reserved)).transpose().map(|v|v.unwrap_or(false))}
pub(crate) fn attempts_for_plan(conn:&Connection,id:&str)->Result<Vec<WorkAttempt>,String>{let mut q=conn.prepare("SELECT a.data FROM work_attempts a JOIN work_jobs j ON j.id=a.job_id WHERE j.plan_id=?1 ORDER BY a.rowid").map_err(storage_error)?;let rows=q.query_map([id],|r|r.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;rows.into_iter().map(parse).collect()}
pub(crate) fn save_job(conn:&Connection,j:&WorkJob)->Result<(),String>{conn.execute("UPDATE work_jobs SET generation=?2,state=?3,data=?4 WHERE id=?1",params![j.id,j.generation,enum_text(&j.state)?,encode(j)?]).map_err(storage_error).and_then(|n|if n==1{Ok(())}else{Err(error("work_storage_unavailable"))})}
pub(crate) fn save_attempt(conn:&Connection,a:&WorkAttempt)->Result<(),String>{conn.execute("UPDATE work_attempts SET state=?2,effect_kind=?3,effect_id=?4,data=?5 WHERE id=?1",params![a.view.id,enum_text(&a.view.state)?,a.view.effect.as_ref().map(EffectRef::kind),a.view.effect.as_ref().map(EffectRef::id),encode(a)?]).map_err(storage_error).and_then(|n|if n==1{Ok(())}else{Err(error("work_storage_unavailable"))})}
pub(crate) fn node_dispatched(conn:&Connection,plan_id:&str,node_id:&str)->Result<bool,String>{
    let jobs=jobs_for_plan(conn,plan_id)?;for j in jobs.iter().filter(|j|j.node_id==node_id){for a in attempts_for_plan(conn,plan_id)?.iter().filter(|a|a.view.job_id==j.id){if a.view.dispatch_intent_at.is_some()||a.view.effect.is_some()||!matches!(a.view.state,AttemptState::Reserved|AttemptState::NotStarted){return Ok(true);}}}Ok(false)
}
pub(crate) fn check_expected_head(conn:&Connection,head:Option<&PlanHead>,expected:Option<&PlanRevisionRef>,generation:u64)->Result<(),String>{
    match (head,expected){(None,None) if generation==0=>Ok(()),(Some(h),Some(r)) if generation==h.generation&&revision(conn,&h.head_revision_id)?.reference()==*r=>Ok(()),_=>Err(error("work_revision_changed"))}
}
pub fn plan_bindings_view(conn:&Connection)->Result<Vec<WorkPlanBindingView>,String>{
    let mut q=conn.prepare("SELECT b.execution_plan_id,b.job_id,j.plan_id,j.revision_id FROM work_plan_bindings b JOIN work_jobs j ON j.id=b.job_id ORDER BY b.rowid").map_err(storage_error)?;
    let rows=q.query_map([],|r|Ok(WorkPlanBindingView{execution_plan_id:r.get(0)?,job_id:r.get(1)?,plan_id:r.get(2)?,revision_id:r.get(3)?})).map_err(storage_error)?;rows.collect::<Result<Vec<_>,_>>().map_err(storage_error)
}
pub(crate) fn assert_direct_unowned(conn:&Connection,execution_plan_id:&str)->Result<(),String>{if conn.query_row("SELECT 1 FROM work_plan_bindings WHERE execution_plan_id=?1",[execution_plan_id],|r|r.get::<_,i32>(0)).optional().map_err(storage_error)?.is_some(){Err(error("work_owned_operation"))}else{Ok(())}}
pub(crate) fn investigation_binding(conn:&Connection,id:&str)->Result<Option<(String,String)>,String>{
    let mut owner=None;
    for raw in raws(conn,"SELECT result FROM work_commands WHERE action='work.operation.attachInvestigation'")?{
        if let CommandResult::Completed{output:CommandOutput::InvestigationBinding{job_id,compiled:CompiledOperationRef::AiInvestigation{investigation_id,digest,..},..},..}=parse(raw)?{if investigation_id==id{if owner.as_ref().is_some_and(|old|old!=&(job_id.clone(),digest.clone())){return Err(error("work_storage_unavailable"));}owner=Some((job_id,digest));}}
    }Ok(owner)
}
pub(crate) fn assert_direct_investigation_unowned(conn:&Connection,id:&str)->Result<(),String>{if investigation_binding(conn,id)?.is_some(){Err(error("work_owned_operation"))}else{Ok(())}}
pub(crate) fn investigation_owner_locked(conn:&Connection,id:&str)->Result<Option<WorkInvestigationOwner>,String>{investigation_binding(conn,id)?.map(|(id,_)|{let j=job(conn,&id)?;Ok(WorkInvestigationOwner{job_id:j.id,plan_id:j.revision.plan_id,revision_id:j.revision.revision_id})}).transpose()}
pub(crate) fn assert_kali_review_unowned(conn:&Connection,review:&crate::data_policy::DataReviewRef)->Result<(),String>{
    for raw in raws(conn,"SELECT data FROM work_attempts")?{let attempt:WorkAttempt=parse(raw)?;if attempt.view.backend==Backend::Kali&&attempt.data_review_ref.as_ref().is_some_and(|r|r.id==review.id){return Err(error("work_owned_operation"));}}Ok(())
}
pub(crate) fn insert_plan_binding(conn:&Connection,execution_plan_id:&str,job_id:&str,preparation_key:&str,digest:&str)->Result<(),String>{conn.execute("INSERT INTO work_plan_bindings(execution_plan_id,job_id,preparation_key,digest) VALUES(?1,?2,?3,?4)",params![execution_plan_id,job_id,preparation_key,digest]).map_err(storage_error)?;Ok(())}

pub(crate) const MAX_CRITICAL_EVENT_BYTES:u64=1024;
pub(crate) fn event_usage(conn:&Connection)->Result<(u64,u64),String>{
    let bytes:u64=conn.query_row("SELECT COALESCE(SUM(length(CAST(data AS BLOB))),0) FROM work_events",[],|r|r.get(0)).map_err(storage_error)?;
    let attempts:u64=raws(conn,"SELECT data FROM work_attempts")?.into_iter().map(parse::<WorkAttempt>).collect::<Result<Vec<_>,_>>()?.iter().map(|a|u64::from(a.reserved_critical_events)*MAX_CRITICAL_EVENT_BYTES).sum();
    let heads:u64=conn.query_row("SELECT COALESCE(SUM((CASE WHEN h.control='active' THEN 2 ELSE 0 END)+(SELECT COALESCE(SUM(CASE WHEN j.state='preparing' THEN 2 WHEN j.state IN ('awaiting_approval','queued') OR (j.state='cancel_requested' AND NOT EXISTS(SELECT 1 FROM work_attempts a WHERE a.id=json_extract(j.data,'$.currentAttempt.id') AND a.state IN ('reserved','dispatching','running','cancel_requested','unknown'))) THEN 1 ELSE 0 END),0) FROM work_jobs j WHERE j.plan_id=h.plan_id)),0) FROM work_plan_heads h",[],|row|row.get(0)).map_err(storage_error)?;
    Ok((bytes,attempts.saturating_add(heads*MAX_CRITICAL_EVENT_BYTES)))
}
fn store_event(conn:&Connection,event:&WorkEvent)->Result<bool,String>{
    let data=encode(event)?;if data.len() as u64>MAX_CRITICAL_EVENT_BYTES{return Err(error("work_limit"));}
    let progress=event.event_type==WorkEventType::CaptureProgress;
    if !progress {if let Some(id)=&event.attempt_id{let mut a=attempt(conn,id)?;
        let emitted:u32=conn.query_row("SELECT count(*) FROM work_events WHERE json_extract(data,'$.attemptId')=?1 AND json_extract(data,'$.type')<>'capture_progress'",[id],|row|row.get(0)).map_err(storage_error)?;
        if emitted>=256||a.reserved_critical_events==0&&(a.view.state.active()||a.view.state==AttemptState::Reserved){return Err(error("work_limit"));}
        if a.reserved_critical_events>0{a.reserved_critical_events-=1;save_attempt(conn,&a)?;}}}
    let (used,reserved)=event_usage(conn)?;
    if used.saturating_add(reserved).saturating_add(data.len() as u64)>MAX_EVENT_BYTES as u64{if progress{return Ok(false);}return Err(error("work_limit"));}
    conn.execute("INSERT INTO work_events(stream_id,sequence,id,data) VALUES(?1,?2,?3,?4)",params![event.stream_id,event.sequence,event.id,data]).map_err(storage_error)?;Ok(true)
}

pub(crate) fn append_event(conn:&Connection,plan_id:&str,assessment_id:&str,event_type:WorkEventType,job_id:Option<String>,attempt_id:Option<String>,state:Option<JobState>,reason_code:Option<WorkReasonCode>,effect:Option<EffectRef>)->Result<WorkEvent,String>{
    let latest=latest_sequence(conn,plan_id)?;if latest>=MAX_SAFE_INTEGER{return Err(error("work_limit"));}let now=model::now();
    let e=WorkEvent{id:model::id(),stream_id:plan_id.into(),sequence:latest+1,schema:"openkoda.work-event.v1".into(),assessment_id:assessment_id.into(),job_id,attempt_id,event_type,observed_at:now.clone(),recorded_at:now,state,reason_code,effect,capture_id:None,preserved_bytes:None};
    store_event(conn,&e)?;Ok(e)
}
fn latest_sequence(conn:&Connection,plan_id:&str)->Result<u64,String>{conn.query_row("SELECT COALESCE(MAX(sequence),0) FROM work_events WHERE stream_id=?1",[plan_id],|r|r.get(0)).map_err(storage_error)}
pub(crate) fn append_capture_event(conn:&Connection,manifest:&crate::capture::CaptureManifest,sealed:bool)->Result<(),String>{
    let a=attempt(conn,&manifest.attempt_id)?;let j=job(conn,&a.view.job_id)?;
    if j.assessment_id!=manifest.assessment_id||a.view.effect.as_ref()!=Some(&manifest.effect){return Err(error("capture_integrity"));}
    let sequence=latest_sequence(conn,&j.revision.plan_id)?.checked_add(1).filter(|n|*n<=MAX_SAFE_INTEGER).ok_or_else(||error("work_limit"))?;let now=model::now();
    let e=WorkEvent{id:model::id(),stream_id:j.revision.plan_id,sequence,schema:"openkoda.work-event.v1".into(),assessment_id:j.assessment_id,job_id:Some(j.id),attempt_id:Some(a.view.id),event_type:if sealed{WorkEventType::CaptureSealed}else{WorkEventType::CaptureProgress},observed_at:now.clone(),recorded_at:now,state:Some(j.state),reason_code:None,effect:Some(manifest.effect.clone()),capture_id:Some(manifest.id.clone()),preserved_bytes:Some(manifest.preserved_bytes)};
    store_event(conn,&e)?;Ok(())
}

fn read_command(conn:&Connection,sql:&str,arg1:&str,arg2:Option<&str>)->Result<Option<StoredCommand>,String>{
    let read=|r:&rusqlite::Row<'_>|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?));
    let row=if let Some(arg2)=arg2{conn.query_row(sql,params![arg1,arg2],read)}else{conn.query_row(sql,[arg1],read)}.optional().map_err(storage_error)?;
    row.map(|(assessment_id,idempotency_key,review_id,result)|Ok(StoredCommand{assessment_id,idempotency_key,review_id,result:parse(result)?})).transpose()
}
pub(crate) fn command_by_key(conn:&Connection,assessment_id:&str,key:&str)->Result<Option<StoredCommand>,String>{read_command(conn,"SELECT assessment_id,idempotency_key,review_id,result FROM work_commands WHERE assessment_id=?1 AND idempotency_key=?2",assessment_id,Some(key))}
pub(crate) fn command_by_review(conn:&Connection,review_id:&str)->Result<Option<StoredCommand>,String>{read_command(conn,"SELECT assessment_id,idempotency_key,review_id,result FROM work_commands WHERE review_id=?1",review_id,None)}
pub(crate) fn insert_command(conn:&Connection,c:&StoredCommand)->Result<(),String>{let r=c.result.receipt();conn.execute("INSERT INTO work_commands(id,assessment_id,idempotency_key,action,request_digest,review_id,result) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![r.id,c.assessment_id,c.idempotency_key,r.action,r.request_digest,c.review_id,encode(&c.result)?]).map_err(storage_error)?;Ok(())}
pub(crate) fn save_command(conn:&Connection,c:&StoredCommand)->Result<(),String>{let r=c.result.receipt();let changed=conn.execute("UPDATE work_commands SET result=?2 WHERE id=?1 AND request_digest=?3",params![r.id,encode(&c.result)?,r.request_digest]).map_err(storage_error)?;if changed!=1{return Err(error("work_storage_unavailable"));}Ok(())}
pub(crate) fn replay_plan_commit(conn:&Connection,s:&Snapshot,input:&PlanCommitRequest,request_digest:&str)->Result<Option<MutationResult<PlanView>>,String>{
    let Some(command)=command_by_review(conn,&input.review.id)? else{return Ok(None);};
    if command.idempotency_key!=input.idempotency_key{return Err(error("work_review_used"));}
    if command.result.receipt().request_digest!=request_digest||command.result.receipt().action!="work.plan.commit"{return Err(error("work_idempotency_conflict"));}
    let CommandResult::Completed{receipt,output:CommandOutput::Plan{revision_id,..}}=command.result else{return Err(error("work_reconciliation_required"));};
    let value=plan_view(conn,s,&command.assessment_id,Some(&revision_id))?.ok_or_else(||error("work_storage_unavailable"))?;Ok(Some(MutationResult{replayed:true,receipt,value}))
}
pub(crate) fn commit_plan(tx:&Transaction<'_>,s:&mut Snapshot,prepared:&PreparedPlan,input:&PlanCommitRequest,request_digest:&str)->Result<MutationResult<PlanView>,String>{
    if let Some(replay)=replay_plan_commit(tx,s,input,request_digest)?{return Ok(replay);}
    if input.review!=prepared.response.review||chrono::DateTime::parse_from_rfc3339(&input.review.expires_at).map_err(storage_error)?<=chrono::Utc::now(){return Err(error("work_review_expired"));}
    let r=&prepared.response.proposal;
    if command_by_key(tx,&r.assessment_id,&input.idempotency_key)?.is_some(){return Err(error("work_idempotency_conflict"));}
    let head=head_for_assessment(tx,&r.assessment_id)?;check_expected_head(tx,head.as_ref(),prepared.expected_head.as_ref(),prepared.expected_generation)?;
    if let Some(source) = &r.suggestion_source { source.validate_live(tx, s)?; }
    check_current_body(s,&r.body)?;
    if let Some(h)=&head{let old=revision(tx,&h.head_revision_id)?;for n in &r.body.nodes{if old.body.nodes.iter().any(|previous|previous.id==n.id)&&node_dispatched(tx,&h.plan_id,&n.id)?{return Err(error("work_revision_changed"));}}}
    let generation=head.as_ref().map_or(1,|h|h.generation+1);if generation>MAX_SAFE_INTEGER{return Err(error("work_limit"));}
    if let Some(h)=&head{
        for mut old in jobs_for_plan(tx,&h.plan_id)?.into_iter().filter(|j|!j.state.terminal()&&!j.state.active()){
            if let Some(current)=&old.current_attempt{let mut a=attempt(tx,&current.id)?;if a.view.state!=AttemptState::Reserved||a.view.effect.is_some(){return Err(error("work_unknown_effect"));}a.view.state=AttemptState::NotStarted;a.view.finished_at=Some(model::now());a.view.cleanup=CleanupState::Confirmed;a.reserved_capture_bytes=0;a.reserved_critical_events=0;save_attempt(tx,&a)?;}
            old.state=JobState::Superseded;old.generation+=1;save_job(tx,&old)?;
            append_event(tx,&old.revision.plan_id,&old.assessment_id,WorkEventType::JobStateChanged,Some(old.id.clone()),old.current_attempt.as_ref().map(|a|a.id.clone()),Some(old.state),None,None)?;
        }
        tx.execute("UPDATE work_plan_heads SET head_revision_id=?2,generation=?3 WHERE plan_id=?1",params![h.plan_id,r.id,generation]).map_err(storage_error)?;
    }else{tx.execute("INSERT INTO work_plan_heads(plan_id,assessment_id,head_revision_id,generation,control) VALUES(?1,?2,?3,1,'active')",params![r.plan_id,r.assessment_id,r.id]).map_err(storage_error)?;}
    tx.execute("INSERT INTO work_plan_revisions(id,plan_id,number,assessment_id,data) VALUES(?1,?2,?3,?4,?5)",params![r.id,r.plan_id,r.number,r.assessment_id,encode(r)?]).map_err(storage_error)?;
    for n in &r.body.nodes{
        let state=if n.depends_on.is_empty(){JobState::AwaitingPreparation}else{JobState::WaitingDependencies};
        let reasons=n.depends_on.iter().map(|id|WorkReason::new(WorkReasonCode::WorkDependencyPending,Some(id.clone()))).collect();
        let j=WorkJob{id:model::id(),assessment_id:r.assessment_id.clone(),revision:r.reference(),node_id:n.id.clone(),generation:1,state,created_at:model::now(),reasons,compiled:None,current_attempt:None,results:vec![],capture_ids:vec![]};
        tx.execute("INSERT INTO work_jobs(id,plan_id,revision_id,node_id,assessment_id,generation,state,data) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![j.id,j.revision.plan_id,j.revision.revision_id,j.node_id,j.assessment_id,j.generation,enum_text(&j.state)?,encode(&j)?]).map_err(storage_error)?;
    }
    append_event(tx,&r.plan_id,&r.assessment_id,WorkEventType::PlanCommitted,None,None,None,None,None)?;
    let receipt=CommandReceipt{id:model::id(),action:"work.plan.commit".into(),entity_id:r.plan_id.clone(),entity_generation:generation,request_digest:request_digest.into(),created_at:model::now()};
    insert_command(tx,&StoredCommand{assessment_id:r.assessment_id.clone(),idempotency_key:input.idempotency_key.clone(),review_id:Some(input.review.id.clone()),result:CommandResult::Completed{receipt:receipt.clone(),output:CommandOutput::Plan{plan_id:r.plan_id.clone(),revision_id:r.id.clone()}}})?;
    enforce_caps(tx)?;
    Ok(MutationResult{replayed:false,receipt,value:plan_view(tx,s,&r.assessment_id,None)?.ok_or_else(||error("work_storage_unavailable"))?})
}

pub(crate) fn job_view(conn:&Connection,s:&Snapshot,j:&WorkJob)->Result<WorkJobView,String>{
    let h=head(conn,&j.revision.plan_id)?;let live=h.head_revision_id==j.revision.revision_id&&h.control==PlanControl::Active;
    let a=s.assessments.iter().find(|a|a.id==j.assessment_id).ok_or_else(||error("work_storage_unavailable"))?;
    let authorized=live&&s.projects.iter().any(|p|p.id==a.project_id&&!p.archived)&&check_current_body(s,&revision(conn,&j.revision.revision_id)?.body).is_ok();
    let mut reasons=j.reasons.clone();if h.control==PlanControl::Paused{reasons.push(WorkReason::new(WorkReasonCode::WorkPaused,Some(h.plan_id.clone())));}if h.control==PlanControl::Canceled{reasons.push(WorkReason::new(WorkReasonCode::WorkCanceled,Some(h.plan_id.clone())));}
    let previous=j.current_attempt.as_ref().map(|a|attempt(conn,&a.id)).transpose()?;
    let can_retry=authorized&&j.state.terminal()&&j.state!=JobState::Superseded&&previous.as_ref().is_some_and(|a|a.view.cleanup==CleanupState::Confirmed)&&remaining_quotas(conn,s,&j.revision)?.0>0;
    let kali_ready=revision(conn,&j.revision.revision_id)?.body.nodes.iter().any(|n|n.id==j.node_id&&matches!(n.intent,OperationIntent::KaliEvidence{..}));
    let can_enqueue=authorized&&j.state==JobState::AwaitingApproval&&(kali_ready&&j.compiled.is_none()||j.compiled.as_ref().is_some_and(|compiled|match compiled{CompiledOperationRef::Native{plan_id,digest,expires_at}=>chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|t|t>chrono::Utc::now())&&s.plans.iter().any(|p|p.id==*plan_id&&p.digest==*digest&&p.status=="proposed"&&p.timeout_seconds==s.settings.timeout_seconds&&p.output_limit_ki_b==s.settings.output_limit_ki_b),CompiledOperationRef::HumanReview{expires_at,..}|CompiledOperationRef::AiInvestigation{expires_at,..}=>chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|t|t>chrono::Utc::now()),_=>false}));
    Ok(WorkJobView{id:j.id.clone(),assessment_id:j.assessment_id.clone(),revision:j.revision.clone(),node_id:j.node_id.clone(),generation:j.generation,state:j.state,reasons,compiled:j.compiled.clone(),current_attempt:j.current_attempt.clone(),results:j.results.clone(),capture_ids:j.capture_ids.clone(),can_prepare:authorized&&matches!(j.state,JobState::AwaitingPreparation|JobState::AwaitingApproval|JobState::Revoked),can_enqueue,can_cancel:!j.state.terminal()&&j.state!=JobState::Unknown,can_retry})
}
pub fn plan_view(conn:&Connection,s:&Snapshot,assessment_id:&str,revision_id:Option<&str>)->Result<Option<PlanView>,String>{
    let Some(h)=head_for_assessment(conn,assessment_id)? else {if revision_id.is_some(){return Err(error("work_revision_changed"));}return Ok(None);};
    let current=revision(conn,&h.head_revision_id)?;let selected=revision_id.map(|id|revision(conn,id)).transpose()?.unwrap_or_else(||current.clone());
    if selected.plan_id!=h.plan_id||selected.assessment_id!=assessment_id{return Err(error("work_revision_changed"));}
    let all_jobs=jobs_for_plan(conn,&h.plan_id)?;let relevant=all_jobs.iter().filter(|j|j.revision.revision_id==selected.id||j.state.active()).collect::<Vec<_>>();
    let current_jobs=all_jobs.iter().filter(|j|j.revision.revision_id==selected.id).collect::<Vec<_>>();
    let outcome=if relevant.iter().any(|j|matches!(j.state,JobState::Unknown|JobState::ClosedLocally)){PlanOutcome::Unknown}else if relevant.iter().any(|j|j.state.active()){PlanOutcome::Running}else if !current_jobs.is_empty()&&current_jobs.iter().all(|j|j.state==JobState::Completed){PlanOutcome::Completed}else if current_jobs.iter().any(|j|j.state.terminal()){PlanOutcome::Partial}else{PlanOutcome::Pending};
    let jobs=relevant.into_iter().map(|j|job_view(conn,s,j)).collect::<Result<_,_>>()?;
    let attempts=attempts_for_plan(conn,&h.plan_id)?.into_iter().map(|a|a.view).collect();
    let event_cursor=EventCursor{stream_id:h.plan_id.clone(),after_sequence:latest_sequence(conn,&h.plan_id)?};
    Ok(Some(PlanView{id:h.plan_id,assessment_id:h.assessment_id,head:current.reference(),generation:h.generation,control:h.control,outcome,revision:selected,jobs,attempts,reasons:vec![],event_cursor,capabilities:capabilities()}))
}
pub(crate) fn queue(conn:&Connection,s:&Snapshot,assessment_id:Option<&str>)->Result<Value,String>{
    if let Some(id)=assessment_id{valid_id(id)?;if !s.assessments.iter().any(|a|a.id==id){return Err(error("work_invalid_request"));}}
    let jobs=raws(conn,"SELECT data FROM work_jobs ORDER BY rowid")?.into_iter().map(parse::<WorkJob>).collect::<Result<Vec<_>,_>>()?;
    let jobs=jobs.iter().filter(|j|!j.state.terminal()&&assessment_id.is_none_or(|id|j.assessment_id==id)).map(|j|job_view(conn,s,j)).collect::<Result<Vec<_>,_>>()?;
    if jobs.len()>MAX_QUEUED{return Err(error("work_limit"));}
    let active=raws(conn,"SELECT data FROM work_attempts WHERE state IN ('dispatching','running','cancel_requested','unknown') ORDER BY rowid")?.into_iter().map(parse::<WorkAttempt>).collect::<Result<Vec<_>,_>>()?;
    if active.len()>1{return Err(error("work_storage_unavailable"));}
    let blocked_by=if let Some(reason)=crate::work_supervisor::lease_reason(conn)?{vec![reason]}else if active.iter().any(|a|a.view.state==AttemptState::Unknown){vec![WorkReason::new(WorkReasonCode::WorkUnknownEffect,active.first().map(|a|a.view.id.clone()))]}else if !active.is_empty(){vec![WorkReason::new(WorkReasonCode::WorkBusy,active.first().map(|a|a.view.id.clone()))]}else{vec![]};
    Ok(json!({"jobs":jobs,"active":active.first().map(|a|&a.view),"blockedBy":blocked_by,"capabilities":capabilities()}))
}
pub(crate) fn events(conn:&Connection,s:&Snapshot,input:&EventsRequest)->Result<Value,String>{
    valid_id(&input.assessment_id)?;valid_id(&input.stream_id)?;
    if input.limit==0||input.limit>100||input.after_sequence>MAX_SAFE_INTEGER||!s.assessments.iter().any(|a|a.id==input.assessment_id){return Err(error("work_invalid_request"));}
    let h=head(conn,&input.stream_id)?;if h.assessment_id!=input.assessment_id{return Err(error("work_invalid_request"));}
    let latest=latest_sequence(conn,&input.stream_id)?;
    let mut q=conn.prepare("SELECT data FROM work_events WHERE stream_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3").map_err(storage_error)?;
    let rows=q.query_map(params![input.stream_id,input.after_sequence,input.limit],|r|r.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;
    let events=rows.into_iter().map(parse::<WorkEvent>).collect::<Result<Vec<_>,_>>()?;let next=events.last().map_or(input.after_sequence.min(latest),|e|e.sequence);
    let gap=input.after_sequence>latest||events.first().is_some_and(|e|e.sequence!=input.after_sequence+1);
    Ok(json!({"events":events,"next":EventCursor{stream_id:input.stream_id.clone(),after_sequence:next},"latestSequence":latest,"hasMore":next<latest,"gap":gap}))
}

const ATTEMPT_METADATA_ENVELOPE:u64=64*1024;
const PENDING_COMMAND_ENVELOPE:u64=16*1024;
const DIRECT_LEASE_ENVELOPE:u64=4*1024;
/// Actual bytes plus disjoint, automatically debited envelopes. Critical writes
/// grow their existing protected rows; unrelated commands cannot spend this space.
pub(crate) fn metadata_usage(conn:&Connection)->Result<(u64,u64),String>{
    let actual:u64=conn.query_row("SELECT COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM work_plan_revisions),0)+COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM work_jobs),0)+COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM work_attempts),0)+COALESCE((SELECT SUM(length(CAST(result AS BLOB))) FROM work_commands),0)+COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM work_leases),0)+COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM capture_manifests),0)+COALESCE((SELECT SUM(length(id)+length(assessment_id)+length(capture_id)+length(manifest_digest)+length(projection_digest)+length(created_at)+8) FROM capture_cursors),0)",[],|r|r.get(0)).map_err(storage_error)?;
    let disposition_bytes:u64=conn.query_row("SELECT COALESCE(SUM(length(CAST(data AS BLOB))),0) FROM work_ai_dispositions",[],|r|r.get(0)).map_err(storage_error)?;let actual=actual.saturating_add(disposition_bytes);
    let mut reserved=0u64;let mut protected_jobs=BTreeSet::new();
    for raw in raws(conn,"SELECT data FROM work_attempts")?{let a:WorkAttempt=parse(raw.clone())?;if !a.view.state.active()&&a.view.state!=AttemptState::Reserved{continue;}
        let j=job(conn,&a.view.job_id)?;if j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(a.view.id.as_str())||!protected_jobs.insert(j.id.clone()){return Err(error("work_storage_unavailable"));}
        let other:u64=conn.query_row("SELECT (SELECT length(CAST(data AS BLOB)) FROM work_jobs WHERE id=?1)+COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM work_leases WHERE attempt_id=?2),0)+COALESCE((SELECT SUM(length(CAST(data AS BLOB))) FROM capture_manifests WHERE attempt_id=?2),0)",params![j.id,a.view.id],|row|row.get(0)).map_err(storage_error)?;
        let size=other.saturating_add(raw.len() as u64);if size>ATTEMPT_METADATA_ENVELOPE{return Err(error("work_limit"));}reserved+=ATTEMPT_METADATA_ENVELOPE-size;
    }
    for raw in raws(conn,"SELECT result FROM work_commands")?{if matches!(parse::<CommandResult>(raw.clone())?,CommandResult::Pending{..}){if raw.len() as u64>PENDING_COMMAND_ENVELOPE{return Err(error("work_limit"));}reserved+=PENDING_COMMAND_ENVELOPE-raw.len() as u64;}}
    for raw in raws(conn,"SELECT data FROM work_leases WHERE attempt_id IS NULL")?{if raw.len() as u64>DIRECT_LEASE_ENVELOPE{return Err(error("work_limit"));}reserved+=DIRECT_LEASE_ENVELOPE-raw.len() as u64;}
    Ok((actual,reserved))
}
pub(crate) fn enforce_caps(conn:&Connection)->Result<(),String>{
    let (metadata,reserved_metadata)=metadata_usage(conn)?;
    let (events,reserved_events)=event_usage(conn)?;
    let pending:u64=conn.query_row("SELECT COUNT(*) FROM work_jobs WHERE state NOT IN ('completed','partial','failed','canceled','revoked','superseded','closed_locally')",[],|r|r.get(0)).map_err(storage_error)?;
    if metadata.saturating_add(reserved_metadata)>MAX_METADATA_BYTES as u64||events.saturating_add(reserved_events)>MAX_EVENT_BYTES as u64||pending>MAX_QUEUED as u64{return Err(error("work_limit"));}
    if capture_usage(conn,None)?>MAX_PROFILE_CAPTURE_BYTES{return Err(error("work_capture_quota"));}Ok(())
}

/// Full structural validation. Historical records do not become invalid merely
/// because their scope, approval, policy, or credential is no longer current.
pub(crate) fn validate_store(conn:&Connection,s:&Snapshot)->Result<(),String>{
    for (name,ddl) in WORK_SCHEMA {
        let actual:Option<String>=conn.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",[name],|r|r.get(0)).optional().map_err(storage_error)?;
        if actual.as_deref()!=Some(ddl.replace("CREATE TABLE IF NOT EXISTS ","CREATE TABLE ").as_str()){return Err(error("work_storage_unavailable"));}
    }
    let mut schema=conn.prepare("SELECT type,name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'").map_err(storage_error)?;
    let objects=schema.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;
    for (kind,name) in objects{if kind!="table"||name!="app_state"&&!WORK_SCHEMA.iter().any(|(known,_)|*known==name)&&!crate::capture::CAPTURE_SCHEMA.iter().any(|(known,_)|*known==name){return Err(error("work_storage_unavailable"));}}
    enforce_caps(conn)?;
    crate::work_disposition::validate(conn,s)?;
    let revisions=raws(conn,"SELECT data FROM work_plan_revisions ORDER BY plan_id,number")?.into_iter().map(parse::<ActionPlanRevision>).collect::<Result<Vec<_>,_>>()?;
    let mut known=BTreeMap::new();let mut chain:BTreeMap<&str,&ActionPlanRevision>=BTreeMap::new();
    for r in &revisions{
        for id in [&r.id,&r.plan_id,&r.assessment_id]{valid_id(id)?;}validate_graph(&r.body)?;
        if let Some(source) = &r.suggestion_source {
            source.validate_revision(r)?;
            if source.base.head != chain.get(r.plan_id.as_str()).map(|prior|prior.reference()) {
                return Err(error("work_storage_unavailable"));
            }
        }
        if r.assessment_id!=r.body.assessment_id||r.author!="local-user"||!valid_time(&r.created_at)||r.digest!=revision_digest(r)?||r.number==0||r.number>MAX_REVISIONS{return Err(error("work_storage_unavailable"));}
        if !s.assessments.iter().any(|a|a.id==r.assessment_id)||!s.scope_revisions.iter().any(|scope|scope.id==r.body.scope_ref.revision_id&&scope.assessment_id==r.assessment_id&&scope.digest==r.body.scope_ref.digest&&scope.review.is_some()){return Err(error("work_storage_unavailable"));}
        match chain.get(r.plan_id.as_str()){None if r.number==1&&r.previous_revision_id.is_none()=>{},Some(previous) if r.number==previous.number+1&&r.previous_revision_id.as_deref()==Some(previous.id.as_str())&&r.assessment_id==previous.assessment_id=>{},_=>return Err(error("work_storage_unavailable"))}
        chain.insert(&r.plan_id,r);known.insert(r.id.as_str(),r);
        let row:(String,u32,String)=conn.query_row("SELECT plan_id,number,assessment_id FROM work_plan_revisions WHERE id=?1",[&r.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(storage_error)?;
        if row!=(r.plan_id.clone(),r.number,r.assessment_id.clone()){return Err(error("work_storage_unavailable"));}
    }
    let mut q=conn.prepare("SELECT assessment_id FROM work_plan_heads").map_err(storage_error)?;let aids=q.query_map([],|r|r.get::<_,String>(0)).map_err(storage_error)?.collect::<Result<Vec<_>,_>>().map_err(storage_error)?;
    if aids.len()!=chain.len(){return Err(error("work_storage_unavailable"));}
    for aid in aids{let h=head_for_assessment(conn,&aid)?.ok_or_else(||error("work_storage_unavailable"))?;let r=known.get(h.head_revision_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;if h.plan_id!=r.plan_id||h.assessment_id!=r.assessment_id||h.generation<r.number as u64||h.generation>MAX_SAFE_INTEGER||chain[h.plan_id.as_str()].id!=r.id{return Err(error("work_storage_unavailable"));}}
    let jobs=raws(conn,"SELECT data FROM work_jobs ORDER BY rowid")?.into_iter().map(parse::<WorkJob>).collect::<Result<Vec<_>,_>>()?;
    let mut job_map=BTreeMap::new();
    for j in &jobs{
        valid_id(&j.id)?;let r=known.get(j.revision.revision_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;
        if j.revision!=r.reference()||j.assessment_id!=r.assessment_id||!r.body.nodes.iter().any(|n|n.id==j.node_id)||j.generation==0||j.generation>MAX_SAFE_INTEGER||!valid_time(&j.created_at)||j.results.len()>100||j.capture_ids.len()>256{return Err(error("work_storage_unavailable"));}
        let row:(String,String,String,String,u64,String)=conn.query_row("SELECT plan_id,revision_id,node_id,assessment_id,generation,state FROM work_jobs WHERE id=?1",[&j.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).map_err(storage_error)?;
        if row!=(j.revision.plan_id.clone(),j.revision.revision_id.clone(),j.node_id.clone(),j.assessment_id.clone(),j.generation,enum_text(&j.state)?){return Err(error("work_storage_unavailable"));}
        validate_results(conn,s,&j.assessment_id,&j.results)?;for id in &j.capture_ids{valid_id(id)?;}job_map.insert(j.id.as_str(),j);
    }
    for r in &revisions{if jobs.iter().filter(|j|j.revision.revision_id==r.id).count()!=r.body.nodes.len(){return Err(error("work_storage_unavailable"));}}
    let attempts=raws(conn,"SELECT data FROM work_attempts ORDER BY job_id,number")?.into_iter().map(parse::<WorkAttempt>).collect::<Result<Vec<_>,_>>()?;
    let mut attempts_by_job:BTreeMap<&str,&WorkAttempt>=BTreeMap::new();let mut attempt_ids=BTreeMap::new();let mut counts=BTreeMap::new();let mut reserved_effects=BTreeSet::new();let mut kali_reviews=BTreeSet::new();
    for a in &attempts{
        let v=&a.view;valid_id(&v.id)?;valid_id(&a.boot_id)?;let j=job_map.get(v.job_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;
        if v.number==0||v.number>MAX_ATTEMPTS||!valid_time(&v.created_at)||a.reserved_capture_bytes>MAX_PROFILE_CAPTURE_BYTES||a.reserved_critical_events>256||!v.state.active()&&v.state!=AttemptState::Reserved&&(a.reserved_capture_bytes!=0||a.reserved_critical_events!=0){return Err(error("work_storage_unavailable"));}
        if v.backend==Backend::Native&&(v.capture_ids.len()>1||v.results.len()>1||!v.data_receipt_refs.is_empty()||!a.input_refs.is_empty()||v.results.iter().any(|r|!matches!(r,ResultRef::Evidence{..}))||a.reserved_effect.is_some()||a.data_review_ref.is_some()||a.kali_binding.is_some()){return Err(error("work_storage_unavailable"));}
        if matches!(v.backend,Backend::Ai|Backend::Kali){
            let review=a.data_review_ref.as_ref().ok_or_else(||error("work_storage_unavailable"))?;valid_id(&review.id)?;if !valid_hash(&review.digest)||!valid_time(&review.expires_at)||a.input_refs.is_empty()||a.input_refs.len()>32||a.input_refs.iter().any(|r|!matches!(r,ResultRef::Evidence{..}))||v.results.len()>1||v.data_receipt_refs.len()>4{return Err(error("work_storage_unavailable"));}
            let effect=a.reserved_effect.as_ref().ok_or_else(||error("work_storage_unavailable"))?;valid_id(effect.id())?;
            if !reserved_effects.insert((effect.kind(),effect.id()))||v.effect.as_ref().is_some_and(|bound|bound!=effect){return Err(error("work_storage_unavailable"));}
            match (&a.compiled,effect,v.backend){
                (CompiledOperationRef::AiInvestigation{investigation_id,digest,expires_at},EffectRef::AiInvestigation{id},Backend::Ai)=>{
                    if id!=investigation_id||!valid_hash(digest)||!valid_time(expires_at)||work_store_binding_owner(conn,investigation_id)?!=j.id||!v.capture_ids.is_empty()||a.reserved_capture_bytes!=0||a.kali_binding.is_some()||v.results.iter().any(|r|!matches!(r,ResultRef::Derived{producer:DerivedResultProducer::Investigation,id:result_id,..} if result_id==id)){return Err(error("work_storage_unavailable"));}
                },
                (CompiledOperationRef::KaliEvidence{review_id,digest,expires_at,environment_id,operation},EffectRef::KaliRun{..},Backend::Kali)=>{
                    valid_id(environment_id)?;if review_id!=&review.id||digest!=&review.digest||expires_at!=&review.expires_at||!kali_reviews.insert(review_id)||a.input_refs.len()!=1||v.capture_ids.len()>2||v.results.iter().any(|r|!matches!(r,ResultRef::Evidence{..})){return Err(error("work_storage_unavailable"));}
                    let cap=match operation{KaliOperation::Scan=>68*1024,KaliOperation::Hash=>4*1024};if (v.state.active()||v.state==AttemptState::Reserved)&&a.reserved_capture_bytes!=cap{return Err(error("work_storage_unavailable"));}
                    if let Some(binding)=&a.kali_binding{binding.validate()?;if binding.attempt_id()!=v.id||binding.assessment_id()!=j.assessment_id||binding.run_id()!=effect.id()||binding.environment_id()!=environment_id||binding.operation()!=*operation||binding.input_refs()!=a.input_refs||binding.capture_ids()!=v.capture_ids||v.effect.is_none(){return Err(error("work_storage_unavailable"));}}
                    else if !v.capture_ids.is_empty()||v.started_at.is_some(){return Err(error("work_storage_unavailable"));}
                },_=>return Err(error("work_storage_unavailable")),
            }
        }
        for stamp in [&v.dispatch_intent_at,&v.started_at,&v.finished_at].into_iter().flatten(){if !valid_time(stamp){return Err(error("work_storage_unavailable"));}}
        match attempts_by_job.get(v.job_id.as_str()){None if v.number==1&&v.previous_attempt_id.is_none()=>{},Some(previous) if v.number==previous.view.number+1&&v.previous_attempt_id.as_deref()==Some(previous.view.id.as_str())=>{},_=>return Err(error("work_storage_unavailable"))}
        let row:(String,u32,String,Option<String>,Option<String>)=conn.query_row("SELECT job_id,number,state,effect_kind,effect_id FROM work_attempts WHERE id=?1",[&v.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(storage_error)?;
        if row!=(v.job_id.clone(),v.number,enum_text(&v.state)?,v.effect.as_ref().map(|e|e.kind().into()),v.effect.as_ref().map(|e|e.id().into())){return Err(error("work_storage_unavailable"));}
        if let Some(effect)=&v.effect{valid_id(effect.id())?;if let EffectRef::NativeExecution{id}=effect{if !s.executions.iter().any(|e|e.id==*id&&e.assessment_id==j.assessment_id){return Err(error("work_storage_unavailable"));}}}
        validate_results(conn,s,&j.assessment_id,&v.results)?;validate_results(conn,s,&j.assessment_id,&a.input_refs)?;
        for r in &v.data_receipt_refs{if !s.data_use_receipts.iter().any(|stored|stored.id==r.id&&stored.digest==r.digest){return Err(error("work_storage_unavailable"));}}
        attempts_by_job.insert(&v.job_id,a);attempt_ids.insert(v.id.as_str(),a);*counts.entry(j.assessment_id.as_str()).or_insert(0u32)+=1;
    }
    for (aid,count) in counts{let legacy=s.executions.iter().filter(|e|e.assessment_id==aid&&!attempts.iter().any(|a|a.view.effect==Some(EffectRef::NativeExecution{id:e.id.clone()}))).count();if count as usize+legacy>MAX_ATTEMPTS as usize{return Err(error("work_limit"));}}
    for j in &jobs{if let Some(current)=&j.current_attempt{let a=attempt_ids.get(current.id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;if a.view.job_id!=j.id||a.view.number!=current.number{return Err(error("work_storage_unavailable"));}}}
    for binding in plan_bindings_view(conn)?{
        let j=job_map.get(binding.job_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;
        let p=s.plans.iter().find(|p|p.id==binding.execution_plan_id).ok_or_else(||error("work_storage_unavailable"))?;
        let (key,stored_digest):(String,String)=conn.query_row("SELECT preparation_key,digest FROM work_plan_bindings WHERE execution_plan_id=?1",[&binding.execution_plan_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage_error)?;valid_id(&key)?;
        if p.assessment_id!=j.assessment_id||p.digest!=stored_digest{return Err(error("work_storage_unavailable"));}
    }
    validate_events(conn,&job_map,&attempt_ids)?;validate_commands(conn,s,&known,&job_map)?;
    crate::work_supervisor::validate_leases(conn,s)?;
    let broken:Option<i64>=conn.query_row("SELECT 1 FROM pragma_foreign_key_check LIMIT 1",[],|r|r.get(0)).optional().map_err(storage_error)?;if broken.is_some(){return Err(error("work_storage_unavailable"));}Ok(())
}
pub(crate) fn validate_results(conn:&Connection,s:&Snapshot,aid:&str,results:&[ResultRef])->Result<(),String>{
    for result in results{match result{
        ResultRef::Evidence{id,raw_sha256}=>{valid_id(id)?;if !valid_hash(raw_sha256)||!s.assessments.iter().find(|a|a.id==aid).is_some_and(|a|a.evidence.iter().any(|e|e.id==*id&&e.sha256==*raw_sha256)){return Err(error("work_storage_unavailable"));}},
        ResultRef::HumanReview{id,digest}=>{
            valid_id(id)?;let records=human_records(conn)?;let matches=records.iter().filter(|r|r.id==*id&&r.assessment_id==aid&&r.digest==*digest).collect::<Vec<_>>();
            if matches.len()!=1||matches[0].digest!=human_record_digest(matches[0])?{return Err(error("work_storage_unavailable"));}
            // A human record can only review observed evidence, not recursively
            // cite another human decision as if it were tool output.
            if matches[0].inputs.iter().any(|input|!matches!(input,ResultRef::Evidence{..}|ResultRef::Derived{producer:DerivedResultProducer::Investigation,..})){return Err(error("work_storage_unavailable"));}
            validate_results(conn,s,aid,&matches[0].inputs)?;
        },
        ResultRef::Derived{producer,id,fingerprint}=>{
            valid_id(id)?;if !valid_hash(fingerprint)||*producer!=DerivedResultProducer::Investigation{return Err(error("work_storage_unavailable"));}
            let Some((owner,_))=investigation_binding(conn,id)? else{return Err(error("work_storage_unavailable"));};let j=job(conn,&owner)?;
            if j.assessment_id!=aid{return Err(error("work_storage_unavailable"));}
            let matching=attempts_for_plan(conn,&j.revision.plan_id)?.into_iter().filter(|a|a.view.job_id==owner&&a.view.backend==Backend::Ai&&a.view.effect.as_ref()==Some(&EffectRef::AiInvestigation{id:id.clone()})&&a.view.results.iter().any(|r|r==result)).count();
            if matching!=1{return Err(error("work_storage_unavailable"));}
        },
    }}Ok(())
}
fn work_store_binding_owner(conn:&Connection,id:&str)->Result<String,String>{investigation_binding(conn,id)?.map(|(owner,_)|owner).ok_or_else(||error("work_storage_unavailable"))}
fn validate_events(conn:&Connection,jobs:&BTreeMap<&str,&WorkJob>,attempts:&BTreeMap<&str,&WorkAttempt>)->Result<(),String>{
    let mut q=conn.prepare("SELECT stream_id,sequence,id,data FROM work_events ORDER BY stream_id,sequence").map_err(storage_error)?;let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).map_err(storage_error)?;let mut next=BTreeMap::new();let mut critical:BTreeMap<String,u32>=BTreeMap::new();
    for row in rows{let(stream,sequence,id,raw)=row.map_err(storage_error)?;if raw.len() as u64>MAX_CRITICAL_EVENT_BYTES{return Err(error("work_limit"));}let e:WorkEvent=parse(raw)?;valid_id(&e.id)?;let h=head(conn,&stream)?;let expected=next.entry(stream.clone()).or_insert(1u64);
        if e.id!=id||e.stream_id!=stream||e.sequence!=sequence||e.sequence!=*expected||sequence>MAX_SAFE_INTEGER||e.assessment_id!=h.assessment_id||e.schema!="openkoda.work-event.v1"||!valid_time(&e.recorded_at)||!valid_time(&e.observed_at){return Err(error("work_storage_unavailable"));}*expected+=1;
        if let Some(id)=&e.job_id{if !jobs.get(id.as_str()).is_some_and(|j|j.revision.plan_id==stream&&j.assessment_id==e.assessment_id){return Err(error("work_storage_unavailable"));}}
        if let Some(id)=&e.attempt_id{if !attempts.get(id.as_str()).is_some_and(|a|e.job_id.as_ref()==Some(&a.view.job_id)){return Err(error("work_storage_unavailable"));}if e.event_type!=WorkEventType::CaptureProgress{let count=critical.entry(id.clone()).or_default();*count+=1;if *count>256{return Err(error("work_limit"));}}}
        if let Some(effect)=&e.effect{valid_id(effect.id())?;}if let Some(id)=&e.capture_id{valid_id(id)?;}if e.preserved_bytes.is_some_and(|n|n>MAX_PROFILE_CAPTURE_BYTES){return Err(error("work_limit"));}
    }Ok(())
}
fn validate_commands(conn:&Connection,s:&Snapshot,revisions:&BTreeMap<&str,&ActionPlanRevision>,jobs:&BTreeMap<&str,&WorkJob>)->Result<(),String>{
    let mut q=conn.prepare("SELECT id,assessment_id,idempotency_key,action,request_digest,review_id,result FROM work_commands").map_err(storage_error)?;
    let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?))).map_err(storage_error)?;
    for row in rows{let(id,aid,key,action,request_digest,review_id,raw)=row.map_err(storage_error)?;let result:CommandResult=parse(raw)?;let receipt=result.receipt();for id in [&id,&aid,&key,&receipt.entity_id]{valid_id(id)?;}if let Some(id)=review_id{valid_id(&id)?;}
        if !s.assessments.iter().any(|a|a.id==aid)||receipt.id!=id||receipt.action!=action||receipt.request_digest!=request_digest||!valid_hash(&request_digest)||!valid_time(&receipt.created_at)||receipt.entity_generation>MAX_SAFE_INTEGER||!matches!(action.as_str(),"work.plan.commit"|"work.operation.prepare"|"work.operation.enqueue"|"work.review.commit"|"work.pause"|"work.resume.commit"|"work.cancel"|"work.cancelPlan"|"work.reconcile"|"work.retry.prepare"|"work.operation.attachInvestigation"){return Err(error("work_storage_unavailable"));}
        match &result{
            CommandResult::Completed{output:CommandOutput::Plan{plan_id,revision_id},..}=>{if !revisions.get(revision_id.as_str()).is_some_and(|r|r.plan_id==*plan_id&&r.assessment_id==aid){return Err(error("work_storage_unavailable"));}},
            CommandResult::Completed{output:CommandOutput::Preparation{job_id,..}|CommandOutput::Job{job_id},..}=>{if !jobs.get(job_id.as_str()).is_some_and(|j|j.assessment_id==aid){return Err(error("work_storage_unavailable"));}},
            CommandResult::Completed{output:CommandOutput::BackendPreparation{job_id,backend,intent_digest,resolved_inputs,context_selection,environment_id},..}=>{
                let j=jobs.get(job_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;let r=revisions[j.revision.revision_id.as_str()];let node=r.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_storage_unavailable"))?;
                if j.assessment_id!=aid||!matches!(action.as_str(),"work.operation.prepare"|"work.retry.prepare")||*intent_digest!=digest(&node.intent)?||!matches!((backend,&node.intent),(Backend::Ai,OperationIntent::AiInvestigation{..})|(Backend::Kali,OperationIntent::KaliEvidence{..}))||resolved_inputs.iter().any(|r|!matches!(r,ResultRef::Evidence{..}))||context_selection.evidence_ids!=resolved_inputs.iter().filter_map(|r|if let ResultRef::Evidence{id,..}=r{Some(id.clone())}else{None}).collect::<Vec<_>>()||(*backend==Backend::Kali)!=environment_id.is_some(){return Err(error("work_storage_unavailable"));}
                if let Some(id)=environment_id{valid_id(id)?;}validate_results(conn,s,&aid,resolved_inputs)?;
            },
            CommandResult::Completed{output:CommandOutput::InvestigationBinding{job_id,compiled,resolved_inputs},..}=>{
                let j=jobs.get(job_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;let CompiledOperationRef::AiInvestigation{investigation_id,digest,expires_at}=compiled else{return Err(error("work_storage_unavailable"));};valid_id(investigation_id)?;
                if action!="work.operation.attachInvestigation"||j.assessment_id!=aid||!valid_hash(digest)||!valid_time(expires_at)||investigation_binding(conn,investigation_id)?.as_ref()!=Some(&(j.id.clone(),digest.clone()))||resolved_inputs.iter().any(|r|!matches!(r,ResultRef::Evidence{..})){return Err(error("work_storage_unavailable"));}validate_results(conn,s,&aid,resolved_inputs)?;
            },
            CommandResult::Completed{output:CommandOutput::HumanReview{record},..}=>{
                let j=jobs.get(record.job_id.as_str()).ok_or_else(||error("work_storage_unavailable"))?;
                valid_id(&record.id)?;
                if record.assessment_id!=aid||j.assessment_id!=aid||record.revision!=j.revision||record.author!="local-user"||!valid_time(&record.created_at)||!plain_text(&record.note,2000)||record.inputs.is_empty()||record.inputs.len()>32||record.digest!=human_record_digest(record)?||record.input_digest!=digest(&record.inputs)?||record.inputs.iter().any(|r|!matches!(r,ResultRef::Evidence{..}|ResultRef::Derived{producer:DerivedResultProducer::Investigation,..})){return Err(error("work_storage_unavailable"));}
                validate_results(conn,s,&aid,&record.inputs)?;
            },
            CommandResult::Pending{entity_id,..}|CommandResult::Failed{entity_id,..}=>valid_id(entity_id)?,
        }
    }Ok(())
}
pub(crate) fn human_records(conn:&Connection)->Result<Vec<HumanReviewRecord>,String>{
    let commands=raws(conn,"SELECT result FROM work_commands WHERE action='work.review.commit'")?;
    let mut result=vec![];for raw in commands{if let CommandResult::Completed{output:CommandOutput::HumanReview{record},..}=parse(raw)?{result.push(record);}}Ok(result)
}

pub(crate) fn resolve_inputs(conn:&Connection,s:&Snapshot,j:&WorkJob)->Result<Vec<ResultRef>,String>{
    let revision=revision(conn,&j.revision.revision_id)?;let n=revision.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_storage_unavailable"))?;let jobs=jobs_for_plan(conn,&j.revision.plan_id)?;let mut results=vec![];
    for dependency in &n.depends_on{let parent=jobs.iter().find(|p|p.revision==j.revision&&p.node_id==*dependency).ok_or_else(||error("work_result_missing"))?;if parent.state!=JobState::Completed{return Err(error(if parent.state.terminal()||parent.state==JobState::Unknown{"work_dependency_failed"}else{"work_dependency_pending"}));}validate_results(conn,s,&j.assessment_id,&parent.results)?;}
    for input in n.intent.inputs(){let result=match input{
        WorkInput::Evidence{evidence_id,expected_raw_sha256}=>ResultRef::Evidence{id:evidence_id.clone(),raw_sha256:expected_raw_sha256.clone()},
        WorkInput::DependencyOutput{node_id,output_role}=>{let parent=jobs.iter().find(|p|p.revision==j.revision&&p.node_id==*node_id&&p.state==JobState::Completed).ok_or_else(||error("work_dependency_pending"))?;let outputs=parent.results.iter().filter(|r|match output_role{OutputRole::Evidence=>matches!(r,ResultRef::Evidence{..}),OutputRole::Investigation=>matches!(r,ResultRef::Derived{producer:DerivedResultProducer::Investigation,..})}).collect::<Vec<_>>();if outputs.len()!=1{return Err(error("work_result_missing"));}outputs[0].clone()},
    };if !results.contains(&result){results.push(result);}}
    validate_results(conn,s,&j.assessment_id,&results)?;Ok(results)
}
pub(crate) fn check_job_request(conn:&Connection,s:&Snapshot,reference:&JobRef,revision_ref:&PlanRevisionRef,require_active:bool)->Result<WorkJob,String>{
    valid_id(&reference.id)?;let j=job(conn,&reference.id)?;let h=head(conn,&j.revision.plan_id)?;
    if j.generation!=reference.generation||j.revision!=*revision_ref||h.head_revision_id!=j.revision.revision_id{return Err(error("work_revision_changed"));}
    if require_active&&h.control!=PlanControl::Active{return Err(error(if h.control==PlanControl::Canceled{"work_canceled"}else{"work_paused"}));}
    check_current_body(s,&revision(conn,&j.revision.revision_id)?.body)?;Ok(j)
}
pub(crate) fn remaining_quotas(conn:&Connection,s:&Snapshot,revision_ref:&PlanRevisionRef)->Result<(u32,u64,u64),String>{
    let r=revision(conn,&revision_ref.revision_id)?;let attempts=attempts_for_plan(conn,&r.plan_id)?;
    let legacy=s.executions.iter().filter(|e|e.assessment_id==r.assessment_id&&!attempts.iter().any(|a|a.view.effect==Some(EffectRef::NativeExecution{id:e.id.clone()}))).count() as u32;
    let remaining_attempts=r.body.limits.max_attempts_total.saturating_sub(attempts.len() as u32).min(MAX_ATTEMPTS.saturating_sub(attempts.len() as u32+legacy));
    let used_plan=capture_usage(conn,Some(&r.plan_id))?;let used_profile=capture_usage(conn,None)?;
    Ok((remaining_attempts,r.body.limits.max_capture_bytes_total.saturating_sub(used_plan),MAX_PROFILE_CAPTURE_BYTES.saturating_sub(used_profile)))
}
/// Unknown retains its original reservation even after a partial stream is sealed.
/// Purge returns profile storage, but never resets a plan's lifetime byte budget.
pub(crate) fn capture_usage(conn:&Connection,plan_id:Option<&str>)->Result<u64,String>{
    let mut allocated:BTreeMap<String,u64>=BTreeMap::new();
    let mut query=conn.prepare("SELECT c.state,c.max_bytes,c.data,j.plan_id,c.attempt_id FROM capture_manifests c JOIN work_attempts a ON a.id=c.attempt_id JOIN work_jobs j ON j.id=a.job_id").map_err(storage_error)?;
    let rows=query.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,u64>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?))).map_err(storage_error)?;
    for row in rows{let(state,cap,raw,owner,attempt_id)=row.map_err(storage_error)?;if plan_id.is_some_and(|id|id!=owner){continue;}let manifest:crate::capture::CaptureManifest=parse(raw)?;let bytes=if state=="purged"&&plan_id.is_none(){0}else if state=="open"{cap}else{manifest.preserved_bytes};let total=allocated.entry(attempt_id).or_default();*total=total.saturating_add(bytes);}
    for raw in raws(conn,"SELECT data FROM work_attempts")?{let a:WorkAttempt=parse(raw)?;if let Some(id)=plan_id{if job(conn,&a.view.job_id)?.revision.plan_id!=id{continue;}}
        if a.view.state.active()||a.view.state==AttemptState::Reserved{let total=allocated.entry(a.view.id).or_default();*total=(*total).max(a.reserved_capture_bytes);}}
    Ok(allocated.values().copied().fold(0u64,u64::saturating_add))
}
pub(crate) fn effective_limits(conn:&Connection,s:&Snapshot,j:&WorkJob,p:&crate::model::ExecutionPlan)->Result<EffectiveLimits,String>{
    let (remaining_attempts,remaining_capture_bytes,profile)=remaining_quotas(conn,s,&j.revision)?;
    Ok(EffectiveLimits{timeout_seconds:p.timeout_seconds,output_limit_ki_b:p.output_limit_ki_b,max_capture_bytes:remaining_capture_bytes.min(profile).min(4*1024*1024).min(p.output_limit_ki_b as u64*1024),remaining_attempts,remaining_capture_bytes})
}
pub(crate) fn insert_attempt(conn:&Connection,a:&WorkAttempt)->Result<(),String>{conn.execute("INSERT INTO work_attempts(id,job_id,number,state,effect_kind,effect_id,data) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![a.view.id,a.view.job_id,a.view.number,enum_text(&a.view.state)?,a.view.effect.as_ref().map(EffectRef::kind),a.view.effect.as_ref().map(EffectRef::id),encode(a)?]).map_err(storage_error)?;Ok(())}
