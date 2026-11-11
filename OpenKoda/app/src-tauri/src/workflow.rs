//! Versioned intentions and bounded workflow DTOs. No adapter or provider I/O.
use crate::{core::{self, Store}, data_projection::{DataRedactor, RedactionMode}, model::{self, Snapshot}, scope::{self, ScopeRef}, scope_targets::ScopeTargetRef, work_store};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet, HashMap}, sync::Mutex};

pub const MAX_NODES: usize = 32;
pub const MAX_EDGES: usize = 64;
pub const MAX_REVISIONS: u32 = 20;
pub const MAX_ATTEMPTS: u32 = 100;
pub const MAX_QUEUED: usize = 100;
pub const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROFILE_CAPTURE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub(crate) fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> { Option::<T>::deserialize(d) }
pub(crate) fn error(code: &str) -> String { format!("{code}: revise o estado e a decisão do trabalho.") }
pub(crate) fn storage_error(_: impl std::fmt::Display) -> String { error("work_storage_unavailable") }
pub(crate) fn encode<T: Serialize>(v: &T) -> Result<String, String> { serde_json::to_string(v).map_err(storage_error) }
pub(crate) fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> { serde_json::from_value(v).map_err(|_| error("work_invalid_request")) }
pub(crate) fn digest<T: Serialize>(v: &T) -> Result<String, String> { Ok(core::hash(&encode(v)?)) }
pub(crate) fn valid_id(v: &str) -> Result<(), String> { core::valid_id(v).map_err(|_| error("work_invalid_request")) }
pub(crate) fn valid_hash(v: &str) -> bool { v.len() == 64 && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
pub(crate) fn valid_time(v: &str) -> bool { chrono::DateTime::parse_from_rfc3339(v).is_ok_and(|t| t.offset().local_minus_utc() == 0) }
pub(crate) fn plain_text(v: &str, limit: usize) -> bool { !v.trim().is_empty() && v.chars().count() <= limit && !v.chars().any(|c| c.is_control() && c != '\n' && c != '\t') }
fn checked_text(v: &str, limit: usize, redactor: &dyn DataRedactor) -> Result<(), String> {
    if !plain_text(v, limit) { return Err(error("work_invalid_request")); }
    if redactor.mode() == RedactionMode::MetadataOnly { return Err(error("data_redactor_unavailable")); }
    let projected = redactor.redact(v);
    if projected.secret_material || projected.changed || redactor.contains_managed_secret(v) { return Err(error("data_secret_material")); }
    Ok(())
}

macro_rules! closed_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all="snake_case")]
        pub enum $name { $($variant),+ }
    };
}
closed_enum!(PlanControl { Active, Paused, Canceled });
closed_enum!(PlanOutcome { Pending, Running, Completed, Partial, Unknown });
closed_enum!(JobState { WaitingDependencies, AwaitingPreparation, Preparing, AwaitingApproval, Queued, Dispatching, Running, CancelRequested, Completed, Partial, Failed, Canceled, Unknown, Revoked, Superseded, ClosedLocally });
closed_enum!(AttemptState { Reserved, Dispatching, Running, CancelRequested, Completed, Partial, Failed, Canceled, Unknown, NotStarted, ClosedLocally });
closed_enum!(CleanupState { NotApplicable, Pending, Confirmed, Unknown });
closed_enum!(Backend { Native, Kali, Ai, Human });
closed_enum!(OutputRole { Evidence, Investigation });
closed_enum!(KaliOperation { Hash, Scan });
closed_enum!(Perspective { Evidence, Counterevidence, Remediation });
closed_enum!(DerivedResultProducer { Investigation, KaliRun });
closed_enum!(IntentKind { Native, HumanReview, KaliEvidence, AiInvestigation });
closed_enum!(WorkEventType { PlanCommitted, PlanPaused, PlanResumed, PlanCanceled, JobStateChanged, AttemptReserved, EffectBound, CaptureProgress, CaptureSealed, AttemptFinished, ReconciliationRecorded, HumanReviewRecorded });
closed_enum!(WorkReasonCode { WorkInvalidRequest, WorkLimit, WorkRevisionChanged, WorkIdempotencyConflict, WorkReviewExpired, WorkReviewUsed, WorkDependencyPending, WorkDependencyFailed, WorkResultMissing, WorkOwnedOperation, WorkPaused, WorkCanceled, WorkBusy, WorkUnknownEffect, WorkPreparationRequired, WorkApprovalRequired, WorkOperationExpired, WorkInputChanged, WorkIntentUnavailable, WorkCaptureQuota, WorkStorageUnavailable, WorkReconciliationRequired, WorkRemoteEffectUnresolved, CaptureUnavailable, CaptureNotSealed, CaptureIntegrity, CaptureCursorInvalid, CaptureEncodingUnavailable });
pub fn known_reason_code(value:&str)->bool { serde_json::from_value::<WorkReasonCode>(Value::String(value.into())).is_ok() }
impl JobState {
    pub fn active(self) -> bool { matches!(self, Self::Dispatching | Self::Running | Self::CancelRequested | Self::Unknown) }
    pub fn terminal(self) -> bool { matches!(self, Self::Completed | Self::Partial | Self::Failed | Self::Canceled | Self::Revoked | Self::Superseded | Self::ClosedLocally) }
}
impl AttemptState { pub fn active(self) -> bool { matches!(self, Self::Dispatching | Self::Running | Self::CancelRequested | Self::Unknown) } }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NativeAdapter {
    #[serde(rename="local.inventory.v1")] LocalInventory,
    #[serde(rename="source.rules.v1")] SourceRules,
    #[serde(rename="http.headers.v1")] HttpHeaders,
    #[serde(rename="network.tcp-connect.v1")] TcpConnect,
}
impl NativeAdapter { pub fn as_str(self) -> &'static str { match self { Self::LocalInventory => "local.inventory.v1", Self::SourceRules => "source.rules.v1", Self::HttpHeaders => "http.headers.v1", Self::TcpConnect => "network.tcp-connect.v1" } } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct WorkReviewRef { pub id: String, pub digest: String, pub expires_at: String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanRevisionRef { pub plan_id: String, pub revision_id: String, pub number: u32, pub digest: String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct JobRef { pub id: String, pub generation: u64 }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct AttemptRef { pub id: String, pub number: u32 }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct CommandReceipt { pub id: String, pub action: String, pub entity_id: String, pub entity_generation: u64, pub request_digest: String, pub created_at: String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct MutationResult<T> { pub replayed: bool, pub receipt: CommandReceipt, pub value: T }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct WorkReason { pub code: WorkReasonCode, #[serde(deserialize_with="required_nullable")] pub entity_id: Option<String> }
impl WorkReason { pub fn new(code: WorkReasonCode, entity_id: Option<String>) -> Self { Self { code, entity_id } } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum WorkInput { Evidence { evidence_id: String, expected_raw_sha256: String }, DependencyOutput { node_id: String, output_role: OutputRole } }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum OperationIntent {
    Native { adapter: NativeAdapter, #[serde(deserialize_with="required_nullable")] target_ref: Option<ScopeTargetRef>, #[serde(deserialize_with="required_nullable")] ports: Option<Vec<u16>> },
    HumanReview { inputs: Vec<WorkInput> },
    KaliEvidence { operation: KaliOperation, input: WorkInput },
    AiInvestigation { inputs: Vec<WorkInput>, perspectives: Vec<Perspective> },
}
impl OperationIntent {
    pub fn kind(&self) -> IntentKind { match self { Self::Native {..} => IntentKind::Native, Self::HumanReview {..} => IntentKind::HumanReview, Self::KaliEvidence {..} => IntentKind::KaliEvidence, Self::AiInvestigation {..} => IntentKind::AiInvestigation } }
    pub fn inputs(&self) -> Vec<&WorkInput> { match self { Self::Native {..} => vec![], Self::HumanReview { inputs } | Self::AiInvestigation { inputs, .. } => inputs.iter().collect(), Self::KaliEvidence { input, .. } => vec![input] } }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ActionNode { pub id: String, pub purpose: String, pub expected_observation: String, pub depends_on: Vec<String>, pub intent: OperationIntent }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanLimits { pub max_attempts_total: u32, pub max_capture_bytes_total: u64 }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ActionPlanBody { pub schema: String, pub assessment_id: String, pub scope_ref: ScopeRef, pub goal: String, pub nodes: Vec<ActionNode>, pub limits: PlanLimits }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ActionPlanRevision {
    pub id: String, pub plan_id: String, pub assessment_id: String, pub number: u32,
    #[serde(deserialize_with="required_nullable")] pub previous_revision_id: Option<String>,
    pub body: ActionPlanBody, pub created_at: String, pub author: String, pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion_source: Option<SuggestionSource>,
}
impl ActionPlanRevision { pub fn reference(&self) -> PlanRevisionRef { PlanRevisionRef { plan_id: self.plan_id.clone(), revision_id: self.id.clone(), number: self.number, digest: self.digest.clone() } } }
pub fn revision_digest(r: &ActionPlanRevision) -> Result<String, String> {
    let mut record = json!({"schema":"openkoda.action-plan-revision.v1","id":r.id,"planId":r.plan_id,"assessmentId":r.assessment_id,"number":r.number,"previousRevisionId":r.previous_revision_id,"body":r.body,"createdAt":r.created_at,"author":r.author});
    if let Some(source) = &r.suggestion_source { record["suggestionSource"] = json!(source); }
    digest(&record)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum EffectRef { NativeExecution { id: String }, AiInvestigation { id: String }, AiRequest { id: String }, KaliRun { id: String }, HumanReview { id: String }, Remediation { id: String } }
impl EffectRef {
    pub fn id(&self) -> &str { match self { Self::NativeExecution {id}|Self::AiInvestigation {id}|Self::AiRequest {id}|Self::KaliRun {id}|Self::HumanReview {id}|Self::Remediation {id} => id } }
    pub fn kind(&self) -> &'static str { match self { Self::NativeExecution {..}=>"native_execution", Self::AiInvestigation {..}=>"ai_investigation", Self::AiRequest {..}=>"ai_request", Self::KaliRun {..}=>"kali_run", Self::HumanReview {..}=>"human_review", Self::Remediation {..}=>"remediation" } }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum ResultRef { Evidence { id: String, raw_sha256: String }, HumanReview { id: String, digest: String }, Derived { producer: DerivedResultProducer, id: String, fingerprint: String } }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum CompiledOperationRef { Native { plan_id: String, digest: String, expires_at: String }, HumanReview { review_id: String, digest: String, expires_at: String }, KaliEvidence { review_id:String, digest:String, expires_at:String, environment_id:String, operation:KaliOperation }, AiInvestigation { investigation_id: String, digest: String, expires_at: String } }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct WorkJobView {
    pub id: String, pub assessment_id: String, pub revision: PlanRevisionRef, pub node_id: String, pub generation: u64, pub state: JobState, pub reasons: Vec<WorkReason>,
    #[serde(deserialize_with="required_nullable")] pub compiled: Option<CompiledOperationRef>,
    #[serde(deserialize_with="required_nullable")] pub current_attempt: Option<AttemptRef>,
    pub results: Vec<ResultRef>, pub capture_ids: Vec<String>, pub can_prepare: bool, pub can_enqueue: bool, pub can_cancel: bool, pub can_retry: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct AttemptView {
    pub id: String, pub job_id: String, pub number: u32,
    #[serde(deserialize_with="required_nullable")] pub previous_attempt_id: Option<String>,
    pub state: AttemptState, pub backend: Backend,
    #[serde(deserialize_with="required_nullable")] pub effect: Option<EffectRef>,
    pub cleanup: CleanupState, pub created_at: String,
    #[serde(deserialize_with="required_nullable")] pub dispatch_intent_at: Option<String>,
    #[serde(deserialize_with="required_nullable")] pub started_at: Option<String>,
    #[serde(deserialize_with="required_nullable")] pub finished_at: Option<String>,
    pub results: Vec<ResultRef>, pub capture_ids: Vec<String>, pub data_receipt_refs: Vec<crate::data_policy::DataUseReceiptRef>, pub reasons: Vec<WorkReason>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct WorkEvent {
    pub id: String, pub stream_id: String, pub sequence: u64, pub schema: String, pub assessment_id: String,
    #[serde(deserialize_with="required_nullable")] pub job_id: Option<String>,
    #[serde(deserialize_with="required_nullable")] pub attempt_id: Option<String>,
    #[serde(rename="type")] pub event_type: WorkEventType,
    pub observed_at: String, pub recorded_at: String,
    #[serde(deserialize_with="required_nullable")] pub state: Option<JobState>,
    #[serde(deserialize_with="required_nullable")] pub reason_code: Option<WorkReasonCode>,
    #[serde(deserialize_with="required_nullable")] pub effect: Option<EffectRef>,
    #[serde(deserialize_with="required_nullable")] pub capture_id: Option<String>,
    #[serde(deserialize_with="required_nullable")] pub preserved_bytes: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct EventCursor { pub stream_id: String, pub after_sequence: u64 }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct WorkflowCapabilities {
    pub schema: String, pub supported_intents: Vec<IntentKind>, pub max_concurrent: u32, pub max_nodes: usize, pub max_edges: usize, pub max_revisions: u32, pub max_attempts_total: u32, pub max_queued_jobs: usize,
    pub max_chunk_bytes: usize, pub max_page_bytes: usize, pub max_capture_bytes: u64, pub max_profile_capture_bytes: u64,
}
pub fn capabilities() -> WorkflowCapabilities { WorkflowCapabilities { schema:"openkoda.work-capabilities.v1".into(), supported_intents:vec![IntentKind::Native,IntentKind::HumanReview,IntentKind::AiInvestigation,IntentKind::KaliEvidence], max_concurrent:1, max_nodes:MAX_NODES, max_edges:MAX_EDGES, max_revisions:MAX_REVISIONS, max_attempts_total:MAX_ATTEMPTS, max_queued_jobs:MAX_QUEUED, max_chunk_bytes:32768, max_page_bytes:32768, max_capture_bytes:4194304, max_profile_capture_bytes:MAX_PROFILE_CAPTURE_BYTES } }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanView { pub id: String, pub assessment_id: String, pub head: PlanRevisionRef, pub generation: u64, pub control: PlanControl, pub outcome: PlanOutcome, pub revision: ActionPlanRevision, pub jobs: Vec<WorkJobView>, pub attempts: Vec<AttemptView>, pub reasons: Vec<WorkReason>, pub event_cursor: EventCursor, pub capabilities: WorkflowCapabilities }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum DraftInput { Evidence { evidence_id: String, expected_raw_sha256: String }, DependencyOutput { node_key: String, output_role: OutputRole } }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag="kind", rename_all="snake_case", rename_all_fields="camelCase", deny_unknown_fields)]
pub enum DraftIntent {
    Native { adapter: NativeAdapter, #[serde(deserialize_with="required_nullable")] target_ref: Option<ScopeTargetRef>, #[serde(deserialize_with="required_nullable")] ports: Option<Vec<u16>> },
    HumanReview { inputs: Vec<DraftInput> }, KaliEvidence { operation: KaliOperation, input: DraftInput }, AiInvestigation { inputs: Vec<DraftInput>, perspectives: Vec<Perspective> },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct NodeDraft { pub key: String, #[serde(deserialize_with="required_nullable")] pub existing_node_id: Option<String>, pub purpose: String, pub expected_observation: String, pub depends_on_keys: Vec<String>, pub intent: DraftIntent }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanPrepareRequest { pub assessment_id: String, #[serde(deserialize_with="required_nullable")] pub expected_head: Option<PlanRevisionRef>, pub expected_generation: u64, pub goal: String, pub nodes: Vec<NodeDraft>, pub limits: PlanLimits }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanCommitRequest { pub review: WorkReviewRef, pub idempotency_key: String, pub acknowledge_plan: bool }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanGetRequest { pub assessment_id: String, #[serde(deserialize_with="required_nullable")] pub revision_id: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct QueueRequest { #[serde(deserialize_with="required_nullable")] pub assessment_id: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct EventsRequest { pub assessment_id: String, pub stream_id: String, pub after_sequence: u64, pub limit: u32 }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct NodeMapping { pub key: String, pub node_id: String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PlanPreparation { pub review: WorkReviewRef, pub proposal: ActionPlanRevision, pub node_map: Vec<NodeMapping>, pub superseded_job_ids: Vec<String>, pub reused_results: Vec<ResultRef>, pub warnings: Vec<WorkReason> }
#[derive(Clone)]
pub(crate) struct PreparedPlan { pub response: PlanPreparation, pub expected_head: Option<PlanRevisionRef>, pub expected_generation: u64 }

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PrepareOperationRequest {pub job:JobRef,pub revision:PlanRevisionRef,pub idempotency_key:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum EnqueueRequest {
    Native{job:JobRef,revision:PlanRevisionRef,compiled:CompiledOperationRef,idempotency_key:String,acknowledge_execution:bool},
    AiInvestigation{job:JobRef,revision:PlanRevisionRef,compiled:CompiledOperationRef,data_review:crate::data_policy::DataReviewRef,#[serde(deserialize_with="required_nullable")]budget_review:Option<crate::ai::BudgetConsent>,allow_remote:bool,idempotency_key:String,#[serde(default)]acknowledge_sensitive:bool},
    KaliEvidence{job:JobRef,revision:PlanRevisionRef,data_review:crate::data_policy::DataReviewRef,operation:KaliOperation,package_sensitivity:crate::data_policy::Sensitivity,idempotency_key:String,acknowledge_local_transfer:bool,#[serde(default)]acknowledge_sensitive:bool},
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct AttachInvestigationRequest {pub job:JobRef,pub revision:PlanRevisionRef,pub investigation_id:String,pub digest:String,pub idempotency_key:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct WorkContextSelection {pub evidence_ids:Vec<String>,pub context_target_refs:Vec<ScopeTargetRef>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct EffectiveLimits {pub timeout_seconds:u32,pub output_limit_ki_b:u32,pub max_capture_bytes:u64,pub remaining_attempts:u32,pub remaining_capture_bytes:u64}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum Preparation {
    Native{job:WorkJobView,compiled:CompiledOperationRef,plan:crate::model::ExecutionPlan,resolved_inputs:Vec<ResultRef>,effective_limits:EffectiveLimits,reasons:Vec<WorkReason>},
    HumanReview{job:WorkJobView,#[serde(deserialize_with="required_nullable")]review:Option<WorkReviewRef>,resolved_inputs:Vec<ResultRef>,input_digest:String,prompt:String,can_record:bool,reasons:Vec<WorkReason>},
    AiInvestigation{job:WorkJobView,inputs:Vec<ResultRef>,context_selection:WorkContextSelection,requires_data_review:bool},
    KaliEvidence{job:WorkJobView,input:ResultRef,operation:KaliOperation,environment_id:String,requires_data_review:bool,#[serde(deserialize_with="required_nullable")]packet_preview:Option<Value>},
}
closed_enum!(HumanDecision{Continue,NeedsMoreWork,Stop});
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct HumanReviewRecord {pub id:String,pub job_id:String,pub assessment_id:String,pub revision:PlanRevisionRef,pub inputs:Vec<ResultRef>,pub input_digest:String,pub decision:HumanDecision,pub note:String,pub author:String,pub created_at:String,pub digest:String}
pub(crate) fn human_record_digest(record:&HumanReviewRecord)->Result<String,String>{digest(&json!({"schema":"openkoda.human-review.v1","id":record.id,"jobId":record.job_id,"assessmentId":record.assessment_id,"revision":record.revision,"inputs":record.inputs,"inputDigest":record.input_digest,"decision":record.decision,"note":record.note,"author":record.author,"createdAt":record.created_at}))}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct HumanCommitRequest {pub job:JobRef,pub revision:PlanRevisionRef,pub review:WorkReviewRef,pub input_digest:String,pub decision:HumanDecision,pub note:String,pub idempotency_key:String,pub acknowledge_review:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PauseRequest {pub plan_id:String,pub expected_generation:u64,pub idempotency_key:String,pub acknowledge_pause:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CancelPlanRequest {pub plan_id:String,pub expected_generation:u64,pub idempotency_key:String,pub acknowledge_cancel:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CancelRequest {pub job:JobRef,pub idempotency_key:String,pub acknowledge_cancel:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ResumePrepareRequest {pub plan_id:String,pub expected_generation:u64}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ResumeCommitRequest {pub review:WorkReviewRef,pub idempotency_key:String,pub acknowledge_resume:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct RetryRequest {pub job:JobRef,pub previous_attempt_id:String,pub idempotency_key:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ReconcileRequest {pub attempt_id:String,pub expected_state:ReconcileState,pub note:String,pub idempotency_key:String,pub acknowledge_unknown_effects:bool}
closed_enum!(ReconcileState{Unknown});
#[derive(Clone)]
struct PreparedHuman {review:WorkReviewRef,job:JobRef,revision:PlanRevisionRef,inputs:Vec<ResultRef>,input_digest:String}
#[derive(Clone)]
struct PreparedResume {review:WorkReviewRef,plan_id:String,generation:u64}
#[derive(Clone)]
enum PendingReview {Plan(PreparedPlan),Human(PreparedHuman),Resume(PreparedResume)}
impl PendingReview {fn reference(&self)->&WorkReviewRef{match self{Self::Plan(p)=>&p.response.review,Self::Human(p)=>&p.review,Self::Resume(p)=>&p.review}}}

/// Validates shape/topology without reading a target, resolving DNS, or checking
/// mutable approval. Also used when validating historical database records.
pub fn validate_graph(body: &ActionPlanBody) -> Result<Vec<usize>, String> {
    if body.schema != "openkoda.action-plan.v1" || body.nodes.is_empty() || body.nodes.len()>MAX_NODES || !plain_text(&body.goal,2000) { return Err(error("work_invalid_request")); }
    valid_id(&body.assessment_id)?;
    if body.scope_ref.assessment_id != body.assessment_id || !valid_hash(&body.scope_ref.digest) { return Err(error("work_invalid_request")); }
    valid_id(&body.scope_ref.revision_id)?;
    if body.limits.max_attempts_total == 0 || body.limits.max_attempts_total>MAX_ATTEMPTS || body.limits.max_capture_bytes_total == 0 || body.limits.max_capture_bytes_total>MAX_PROFILE_CAPTURE_BYTES { return Err(error("work_limit")); }
    let mut indices=BTreeMap::new(); let mut edges=0usize;
    for (i,n) in body.nodes.iter().enumerate() { valid_id(&n.id)?; if indices.insert(n.id.as_str(),i).is_some() || !plain_text(&n.purpose,500) || !plain_text(&n.expected_observation,500) { return Err(error("work_invalid_request")); } edges+=n.depends_on.len(); }
    if edges>MAX_EDGES { return Err(error("work_limit")); }
    let mut done=BTreeSet::new(); let mut order=vec![];
    for n in &body.nodes {
        let mut seen=BTreeSet::new();
        for dependency in &n.depends_on { if dependency==&n.id || !indices.contains_key(dependency.as_str()) || !seen.insert(dependency) { return Err(error("work_invalid_request")); } }
        match &n.intent {
            OperationIntent::Native {adapter,ports,..} => {
                if (*adapter==NativeAdapter::TcpConnect)!=ports.is_some() { return Err(error("work_invalid_request")); }
                if let Some(ports)=ports { if ports.is_empty()||ports.len()>16||ports.contains(&0)||ports.iter().collect::<BTreeSet<_>>().len()!=ports.len(){return Err(error("work_invalid_request"));} }
            },
            OperationIntent::HumanReview {inputs} if inputs.is_empty()||inputs.len()>32 => return Err(error("work_limit")),
            OperationIntent::AiInvestigation {inputs,perspectives} if inputs.is_empty()||inputs.len()>32||perspectives.is_empty()||perspectives.len()>3 => return Err(error("work_limit")),
            _=>{},
        }
        for input in n.intent.inputs() { match input { WorkInput::Evidence {evidence_id,expected_raw_sha256}=>{valid_id(evidence_id)?;if !valid_hash(expected_raw_sha256){return Err(error("work_invalid_request"));}},WorkInput::DependencyOutput {node_id,..}=>{if !indices.contains_key(node_id.as_str()){return Err(error("work_invalid_request"));}} } }
    }
    while order.len()<body.nodes.len() { let Some(i)=body.nodes.iter().enumerate().find_map(|(i,n)|(!done.contains(&i)&&n.depends_on.iter().all(|id|done.contains(&indices[id.as_str()]))).then_some(i)) else{return Err(error("work_invalid_request"));};done.insert(i);order.push(i); }
    for (index,n) in body.nodes.iter().enumerate() {
        let mut ancestors=BTreeSet::new(); let mut pending=n.depends_on.iter().map(|id|indices[id.as_str()]).collect::<Vec<_>>();
        while let Some(i)=pending.pop(){if ancestors.insert(i){pending.extend(body.nodes[i].depends_on.iter().map(|id|indices[id.as_str()]));}}
        for input in n.intent.inputs(){if let WorkInput::DependencyOutput {node_id,output_role}=input{let source=indices[node_id.as_str()];let allowed=match output_role{OutputRole::Evidence=>matches!(body.nodes[source].intent,OperationIntent::Native{..}|OperationIntent::KaliEvidence{..}),OutputRole::Investigation=>matches!(n.intent,OperationIntent::HumanReview{..})&&matches!(body.nodes[source].intent,OperationIntent::AiInvestigation{..})};if source==index||!ancestors.contains(&source)||!allowed{return Err(error("work_invalid_request"));}}}
    }
    Ok(order)
}
pub(crate) fn check_current_body(s:&Snapshot,body:&ActionPlanBody)->Result<(),String>{
    validate_graph(body)?;
    let a=s.assessments.iter().find(|a|a.id==body.assessment_id).ok_or_else(||error("work_invalid_request"))?;
    if !s.projects.iter().any(|p|p.id==a.project_id&&!p.archived){return Err("project_archived: projeto arquivado não aceita novo trabalho.".into());}
    if scope::current_ref(s,&a.id)?.as_ref()!=Some(&body.scope_ref){return Err(error("work_revision_changed"));}
    if s.assessment_scopes.iter().any(|binding|binding.assessment_id==a.id&&binding.revocation.is_some()){return Err("scope_revoked: escopo revogado para novo trabalho.".into());}
    for n in &body.nodes {
        if !capabilities().supported_intents.contains(&n.intent.kind()){return Err(error("work_intent_unavailable"));}
        if let OperationIntent::Native {adapter,target_ref,..}=&n.intent{
            scope::check_selected_work(s,&a.id,adapter.as_str(),Some(&body.scope_ref),&target_ref.iter().cloned().collect::<Vec<_>>())?;
            let target=crate::scope_targets::resolve_current_target(s,&a.id,target_ref.as_ref())?;
            let compatible=match adapter{NativeAdapter::LocalInventory|NativeAdapter::SourceRules=>matches!(target.kind.as_str(),"code"|"files"),NativeAdapter::HttpHeaders=>target.kind=="web",NativeAdapter::TcpConnect=>target.kind=="network"};
            if !compatible{return Err(error("work_invalid_request"));}
        }
        for input in n.intent.inputs(){if let WorkInput::Evidence {evidence_id,expected_raw_sha256}=input{if !a.evidence.iter().any(|e|e.id==*evidence_id&&e.sha256==*expected_raw_sha256&&core::hash(&e.content)==e.sha256){return Err(error("work_input_changed"));}}}
    }
    Ok(())
}

#[derive(Default)]
pub struct WorkflowService { reviews: Mutex<HashMap<String,PendingReview>>, backend_grants:Mutex<HashMap<String,PendingBackendGrant>> }
pub(crate) enum BackendGrant {Ai(crate::ai::QueuedAiGrant),Kali(crate::kali_bridge::QueuedKaliGrant)}
struct PendingBackendGrant {boot_id:String,expires_at:String,grant:BackendGrant}
#[path="workflow_suggestion.rs"]
mod suggestion;
pub(crate) use suggestion::{current_base as suggestion_base, SuggestionSource};
pub fn supports(action:&str)->bool{matches!(action,"work.plan.prepareSuggestion"|"work.plan.prepare"|"work.plan.commit"|"work.plan.get"|"work.queue"|"work.events"|"work.operation.prepare"|"work.operation.attachInvestigation"|"work.operation.enqueue"|"work.review.commit"|"work.pause"|"work.resume.prepare"|"work.resume.commit"|"work.cancel"|"work.cancelPlan"|"work.reconcile"|"work.retry.prepare"|"work.aiLocalClosure.prepare"|"work.aiLocalClosure.commit"|"work.aiLocalClosure.list")}
pub fn mutates(action:&str)->bool{supports(action)&&!matches!(action,"work.plan.prepareSuggestion"|"work.plan.prepare"|"work.plan.get"|"work.queue"|"work.events"|"work.resume.prepare")}
impl WorkflowService {
    pub(crate) fn handle_backend(&self,action:&str,payload:Value,store:&Store,executor:&crate::executor::Executor,supervisor:&crate::work_supervisor::WorkSupervisor,ai:&crate::ai::AiService,kali:&crate::kali_bridge::KaliBridge,environment_id:Option<&str>,_identity:&dyn Fn()->Result<String,String>,redactor:&dyn DataRedactor)->Result<Value,String>{
        if !payload.is_object()||payload.to_string().len()>128*1024{return Err(error("work_invalid_request"));}
        match action{
            "work.plan.prepareSuggestion" => {
                let input: suggestion::SuggestionRequest = decode(payload)?;
                Ok(json!(self.prepare_suggestion(store, ai, &input.assessment_id, &input.request_id, &input.proposal_id, redactor)?))
            }
            "work.aiLocalClosure.prepare"|"work.aiLocalClosure.commit"|"work.aiLocalClosure.list"=>ai.handle_local_closure(action,payload,store,||{self.reviews.lock().map_err(storage_error)?.clear();self.backend_grants.lock().map_err(storage_error)?.clear();Ok(())}),
            "work.operation.attachInvestigation"=>self.attach_investigation(store,ai,decode(payload)?),
            "work.reconcile"=>{
                let input:ReconcileRequest=decode(payload.clone())?;
                let backend=store.work_read(|conn,_|Ok(work_store::attempt(conn,&input.attempt_id)?.view.backend))?;
                match backend{Backend::Ai=>self.reconcile_ai_attempt(store,ai,input,redactor),Backend::Kali=>self.reconcile_kali_attempt(store,supervisor,kali,ai,_identity,input,redactor),_=>self.handle_runtime(action,payload,store,executor,supervisor,redactor)}
            },
            "work.operation.enqueue" if payload["kind"]!="native"=>self.enqueue_backend(store,supervisor,ai,kali,environment_id,redactor,decode(payload)?),
            "work.cancel"|"work.cancelPlan"=>{
                let value=self.handle_runtime(action,payload.clone(),store,executor,supervisor,redactor)?;
                // The mutation has released Store and the scope gate. Signals
                // are best-effort accelerators; the durable control remains the
                // authority checked before every subsequent dispatch.
                let effects=store.work_read(|conn,_|{let plan_id=if action=="work.cancelPlan"{payload["planId"].as_str().ok_or_else(||error("work_invalid_request"))?.to_owned()}else{work_store::job(conn,payload["job"]["id"].as_str().ok_or_else(||error("work_invalid_request"))?)?.revision.plan_id};let mut effects=vec![];for j in work_store::jobs_for_plan(conn,&plan_id)?{if j.state==JobState::CancelRequested&&(action=="work.cancelPlan"||payload["job"]["id"].as_str()==Some(j.id.as_str())){if let Some(current)=&j.current_attempt{if let Some(effect)=work_store::attempt(conn,&current.id)?.view.effect{effects.push(effect);}}}}Ok(effects)})?;
                for effect in effects{match effect{EffectRef::AiInvestigation{id}=>{ai.signal_work_cancel(&id)?;},EffectRef::KaliRun{id}=>kali.signal_work_cancel(&id)?,_=>{}}}Ok(value)
            },
            "work.operation.prepare"|"work.retry.prepare"=>{
                let (input,previous)=if action=="work.retry.prepare"{let input:RetryRequest=decode(payload.clone())?;let revision=store.work_read(|conn,_|Ok(work_store::job(conn,&input.job.id)?.revision))?;(PrepareOperationRequest{job:input.job,revision,idempotency_key:input.idempotency_key},Some(input.previous_attempt_id))}else{(decode::<PrepareOperationRequest>(payload.clone())?,None)};
                let backend=store.work_read(|conn,_|{let j=work_store::job(conn,&input.job.id)?;let r=work_store::revision(conn,&j.revision.revision_id)?;Ok(r.body.nodes.iter().any(|n|n.id==j.node_id&&matches!(n.intent,OperationIntent::AiInvestigation{..}|OperationIntent::KaliEvidence{..})))})?;
                if backend{self.prepare_backend(store,input,previous,environment_id)}else{self.handle_runtime(action,payload,store,executor,supervisor,redactor)}
            },
            _=>self.handle_runtime(action,payload,store,executor,supervisor,redactor),
        }
    }
    pub(crate) fn take_backend_grant(&self,attempt_id:&str,boot_id:&str)->Result<Option<BackendGrant>,String>{
        let mut cache=self.backend_grants.lock().map_err(storage_error)?;let now=chrono::Utc::now();
        cache.retain(|_,p|chrono::DateTime::parse_from_rfc3339(&p.expires_at).is_ok_and(|t|t>now));
        Ok(cache.remove(attempt_id).filter(|p|p.boot_id==boot_id).map(|p|p.grant))
    }
    fn backend_preparation_view(&self,conn:&rusqlite::Connection,s:&Snapshot,c:&work_store::StoredCommand,replayed:bool)->Result<Value,String>{
        let work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::BackendPreparation{job_id,backend,resolved_inputs,context_selection,environment_id,..}}=&c.result else{return Err(error(match c.result{work_store::CommandResult::Pending{..}=>"work_busy",_=>"work_preparation_required"}));};
        let j=work_store::job(conn,job_id)?;let r=work_store::revision(conn,&j.revision.revision_id)?;let node=r.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_storage_unavailable"))?;
        let value=match (backend,&node.intent){
            (Backend::Ai,OperationIntent::AiInvestigation{..})=>Preparation::AiInvestigation{job:work_store::job_view(conn,s,&j)?,inputs:resolved_inputs.clone(),context_selection:context_selection.clone(),requires_data_review:true},
            (Backend::Kali,OperationIntent::KaliEvidence{operation,..})=>Preparation::KaliEvidence{job:work_store::job_view(conn,s,&j)?,input:resolved_inputs.first().cloned().ok_or_else(||error("work_result_missing"))?,operation:*operation,environment_id:environment_id.clone().ok_or_else(||error("work_input_changed"))?,requires_data_review:true,packet_preview:None},
            _=>return Err(error("work_storage_unavailable")),
        };Ok(json!(MutationResult{replayed,receipt:receipt.clone(),value}))
    }
    fn prepare_backend(&self,store:&Store,input:PrepareOperationRequest,previous:Option<String>,environment_id:Option<&str>)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;let action=if previous.is_some(){"work.retry.prepare"}else{"work.operation.prepare"};let hash=digest(&json!({"action":action,"job":input.job,"revision":input.revision,"idempotencyKey":input.idempotency_key,"previousAttemptId":previous}))?;
        if let Some(replay)=store.work_read(|conn,s|{let j=work_store::job(conn,&input.job.id)?;work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?.map(|c|{check_command(&c,action,&hash)?;self.backend_preparation_view(conn,s,&c,true)}).transpose()})?{return Ok(replay);}
        let start=store.work_transaction(|conn,s|{
            let current=work_store::job(conn,&input.job.id)?;
            if let Some(command)=work_store::command_by_key(conn,&current.assessment_id,&input.idempotency_key)?{check_command(&command,action,&hash)?;return Ok(BackendPreparationStart::Existing(command));}
            let mut j=work_store::check_job_request(conn,s,&input.job,&input.revision,true)?;
            if let Some(previous)=&previous{let a=work_store::attempt(conn,previous)?;if j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(previous.as_str())||a.view.cleanup!=CleanupState::Confirmed||a.view.state.active()||a.view.state==AttemptState::Reserved{return Err(error("work_reconciliation_required"));}}
            else if j.state.terminal()||j.state.active()||matches!(j.state,JobState::Preparing|JobState::Queued){return Err(error("work_preparation_required"));}
            if let Some(current)=&j.current_attempt{let a=work_store::attempt(conn,&current.id)?;if a.view.state.active()||a.view.state==AttemptState::Reserved{return Err(error("work_reconciliation_required"));}}
            let r=work_store::revision(conn,&j.revision.revision_id)?;let intent=r.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_storage_unavailable"))?.intent.clone();
            if !matches!(intent,OperationIntent::AiInvestigation{..}|OperationIntent::KaliEvidence{..}){return Err(error("work_intent_unavailable"));}
            if matches!(intent,OperationIntent::KaliEvidence{..}){valid_id(environment_id.ok_or_else(||error("work_preparation_required"))?)?;}
            let inputs=work_store::resolve_inputs(conn,s,&j)?;if work_store::remaining_quotas(conn,s,&j.revision)?.0==0{return Err(error("work_limit"));}
            // A new metadata preparation preserves an attached live investigation;
            // it does not recreate either its data review or budget consent.
            j.state=JobState::Preparing;j.generation+=1;j.reasons.clear();if previous.is_some(){j.compiled=None;}work_store::save_job(conn,&j)?;
            let command=work_store::StoredCommand{assessment_id:j.assessment_id.clone(),idempotency_key:input.idempotency_key.clone(),review_id:None,result:work_store::CommandResult::Pending{receipt:new_receipt(action,&j.id,j.generation,&hash),entity_id:j.id.clone()}};
            work_store::insert_command(conn,&command)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id.clone()),None,Some(j.state),None,None)?;Ok(BackendPreparationStart::Compile{job:j,intent,inputs,command})
        })?;
        let BackendPreparationStart::Compile{job,intent,inputs,mut command}=start else{let BackendPreparationStart::Existing(command)=start else{unreachable!()};return store.work_read(|conn,s|self.backend_preparation_view(conn,s,&command,true));};
        let finish=(||{
            let evidence_ids=inputs.iter().map(|r|match r{ResultRef::Evidence{id,..}=>Ok(id.clone()),_=>Err(error("work_input_changed"))}).collect::<Result<Vec<_>,_>>()?;
            let snapshot=store.snapshot()?;let targets=crate::evidence_lineage::evidence_refs(store,&snapshot,&job.assessment_id,&evidence_ids)?;
            let selection=WorkContextSelection{evidence_ids,context_target_refs:targets};
            store.work_transaction(|conn,s|{
                let mut current=work_store::job(conn,&job.id)?;let h=work_store::head(conn,&current.revision.plan_id)?;
                if current.generation!=job.generation||current.state!=JobState::Preparing||h.control!=PlanControl::Active||h.head_revision_id!=current.revision.revision_id{return Err(error("work_revision_changed"));}
                check_current_body(s,&work_store::revision(conn,&current.revision.revision_id)?.body)?;if work_store::resolve_inputs(conn,s,&current)?!=inputs{return Err(error("work_input_changed"));}
                let backend=if matches!(intent,OperationIntent::AiInvestigation{..}){Backend::Ai}else{Backend::Kali};current.state=JobState::AwaitingApproval;current.generation+=1;work_store::save_job(conn,&current)?;
                let mut receipt=command.result.receipt().clone();receipt.entity_generation=current.generation;command.result=work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::BackendPreparation{job_id:job.id.clone(),backend,intent_digest:digest(&intent)?,resolved_inputs:inputs.clone(),context_selection:selection.clone(),environment_id:if backend==Backend::Kali{environment_id.map(str::to_owned)}else{None}}};work_store::save_command(conn,&command)?;
                work_store::append_event(conn,&current.revision.plan_id,&current.assessment_id,WorkEventType::JobStateChanged,Some(current.id),None,Some(JobState::AwaitingApproval),None,None)?;self.backend_preparation_view(conn,s,&command,false)
            })
        })();
        if let Err(original)=finish{store.work_transaction(|conn,_|{let mut j=work_store::job(conn,&job.id)?;if j.state==JobState::Preparing&&j.generation==job.generation{j.state=JobState::AwaitingPreparation;j.generation+=1;j.compiled=None;work_store::save_job(conn,&j)?;}else if j.state==JobState::CancelRequested&&!work_store::job_has_unsettled_attempt(conn,&j)?{j.state=JobState::Canceled;j.generation+=1;work_store::save_job(conn,&j)?;}let mut c=work_store::command_by_key(conn,&job.assessment_id,&input.idempotency_key)?.ok_or_else(||error("work_storage_unavailable"))?;if matches!(c.result,work_store::CommandResult::Pending{..}){c.result=work_store::CommandResult::Failed{receipt:c.result.receipt().clone(),entity_id:job.id.clone(),reason_code:WorkReasonCode::WorkPreparationRequired};work_store::save_command(conn,&c)?;}Ok(())})?;return Err(original);}finish
    }
    fn ai_expectation(conn:&rusqlite::Connection,s:&Snapshot,job:&JobRef,revision:&PlanRevisionRef)->Result<(work_store::WorkJob,crate::ai::WorkAiExpectation),String>{
        let j=work_store::check_job_request(conn,s,job,revision,true)?;let r=work_store::revision(conn,&j.revision.revision_id)?;let OperationIntent::AiInvestigation{perspectives,..}=&r.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_input_changed"))?.intent else{return Err(error("work_intent_unavailable"));};
        if j.state!=JobState::AwaitingApproval{return Err(error("work_preparation_required"));}let expected=crate::ai::WorkAiExpectation{assessment_id:j.assessment_id.clone(),scope_ref:r.body.scope_ref.clone(),inputs:work_store::resolve_inputs(conn,s,&j)?,perspectives:perspectives.clone()};Ok((j,expected))
    }
    fn attach_investigation(&self,store:&Store,ai:&crate::ai::AiService,input:AttachInvestigationRequest)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;valid_id(&input.investigation_id)?;if !valid_hash(&input.digest){return Err(error("work_invalid_request"));}let hash=digest(&json!({"action":"work.operation.attachInvestigation","payload":input}))?;
        if let Some(replay)=store.work_read(|conn,s|{let j=work_store::job(conn,&input.job.id)?;work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?.map(|c|{check_command(&c,"work.operation.attachInvestigation",&hash)?;binding_command_result(conn,s,&c,true)}).transpose()})?{return Ok(replay);}
        let (_,expected)=store.work_read(|conn,s|Self::ai_expectation(conn,s,&input.job,&input.revision))?;
        ai.with_work_investigation(store,&input.investigation_id,&input.digest,&expected,|inspection|{
            let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
            store.work_transaction(|conn,s|{
                let current=work_store::job(conn,&input.job.id)?;if let Some(c)=work_store::command_by_key(conn,&current.assessment_id,&input.idempotency_key)?{check_command(&c,"work.operation.attachInvestigation",&hash)?;return binding_command_result(conn,s,&c,true);}
                let(mut j,current)=Self::ai_expectation(conn,s,&input.job,&input.revision)?;if current.inputs!=expected.inputs||current.scope_ref!=expected.scope_ref||current.perspectives!=expected.perspectives{return Err(error("work_input_changed"));}
                crate::work_supervisor::assert_investigation_attach_available(conn,inspection.id())?;
                if work_store::investigation_binding(conn,inspection.id())?.as_ref().is_some_and(|owner|owner!=&(j.id.clone(),inspection.digest().into())){return Err(error("work_owned_operation"));}
                let compiled=CompiledOperationRef::AiInvestigation{investigation_id:inspection.id().into(),digest:inspection.digest().into(),expires_at:inspection.expires_at().into()};j.compiled=Some(compiled.clone());j.generation+=1;work_store::save_job(conn,&j)?;
                let c=completed_command(&j,"work.operation.attachInvestigation",&input.idempotency_key,&hash,work_store::CommandOutput::InvestigationBinding{job_id:j.id.clone(),compiled,resolved_inputs:expected.inputs.clone()},None);work_store::insert_command(conn,&c)?;binding_command_result(conn,s,&c,false)
            })
        })
    }
    fn enqueue_backend(&self,store:&Store,supervisor:&crate::work_supervisor::WorkSupervisor,ai:&crate::ai::AiService,kali:&crate::kali_bridge::KaliBridge,environment_id:Option<&str>,redactor:&dyn DataRedactor,input:EnqueueRequest)->Result<Value,String>{
        let hash=digest(&json!({"action":"work.operation.enqueue","payload":input}))?;
        let(job,revision,key,review)=match &input{EnqueueRequest::AiInvestigation{job,revision,idempotency_key,data_review,..}|EnqueueRequest::KaliEvidence{job,revision,idempotency_key,data_review,..}=>(job,revision,idempotency_key,data_review),_=>return Err(error("work_intent_unavailable"))};valid_id(key)?;
        if let Some(replay)=store.work_read(|conn,s|{let j=work_store::job(conn,&job.id)?;work_store::command_by_key(conn,&j.assessment_id,key)?.map(|c|{check_command(&c,"work.operation.enqueue",&hash)?;job_command_result(conn,s,&c,true)}).transpose()})?{return Ok(replay);}
        let (grant,compiled,inputs,reserved_effect,cap)=match &input{
            EnqueueRequest::AiInvestigation{compiled,data_review,budget_review,allow_remote,acknowledge_sensitive,..}=>{
                if !allow_remote{return Err(error("work_approval_required"));}let(j,expected)=store.work_read(|conn,s|Self::ai_expectation(conn,s,job,revision))?;if j.compiled.as_ref()!=Some(compiled){return Err(error("work_approval_required"));}
                let CompiledOperationRef::AiInvestigation{investigation_id,digest,..}=compiled else{return Err(error("work_invalid_request"));};let inspection=ai.inspect_work_investigation(store,investigation_id,digest,&expected)?;let grant=ai.validate_queued_investigation(store,&inspection,data_review,budget_review.as_ref(),*acknowledge_sensitive)?;
                (BackendGrant::Ai(grant),compiled.clone(),expected.inputs,EffectRef::AiInvestigation{id:investigation_id.clone()},0)
            },
            EnqueueRequest::KaliEvidence{operation,package_sensitivity,acknowledge_local_transfer,acknowledge_sensitive,data_review,..}=>{
                if !acknowledge_local_transfer{return Err(error("work_approval_required"));}let environment=environment_id.ok_or_else(||error("work_preparation_required"))?;valid_id(environment)?;
                let expected=store.work_read(|conn,s|{let j=work_store::check_job_request(conn,s,job,revision,true)?;if j.state!=JobState::AwaitingApproval{return Err(error("work_preparation_required"));}let r=work_store::revision(conn,&j.revision.revision_id)?;if !r.body.nodes.iter().any(|n|n.id==j.node_id&&matches!(n.intent,OperationIntent::KaliEvidence{operation:expected,..} if expected==*operation)){return Err(error("work_input_changed"));}let inputs=work_store::resolve_inputs(conn,s,&j)?;if inputs.len()!=1{return Err(error("work_input_changed"));}Ok(crate::kali_bridge::WorkKaliExpectation{assessment_id:j.assessment_id,scope_ref:r.body.scope_ref,input:inputs[0].clone(),operation:*operation,environment_id:environment.into()})})?;
                let grant=kali.validate_queued_evidence(store,&expected,data_review,*package_sensitivity,*acknowledge_sensitive,redactor)?;let cap=grant.max_capture_bytes();
                (BackendGrant::Kali(grant),CompiledOperationRef::KaliEvidence{review_id:data_review.id.clone(),digest:data_review.digest.clone(),expires_at:data_review.expires_at.clone(),environment_id:environment.into(),operation:*operation},vec![expected.input],EffectRef::KaliRun{id:model::id()},cap)
            },_=>return Err(error("work_invalid_request")),
        };
        let mut expires=review.expires_at.clone();if let CompiledOperationRef::AiInvestigation{expires_at,..}=&compiled{let compiled_at=chrono::DateTime::parse_from_rfc3339(expires_at).map_err(storage_error)?;let review_at=chrono::DateTime::parse_from_rfc3339(&expires).map_err(storage_error)?;if compiled_at<review_at{expires=expires_at.clone();}}
        // Keep the cache lock across commit so a concurrent drain cannot observe
        // Reserved before its one-use context has been installed. No Store path
        // acquires this mutex while holding a database transaction.
        let mut cache=self.backend_grants.lock().map_err(storage_error)?;let now=chrono::Utc::now();cache.retain(|_,p|chrono::DateTime::parse_from_rfc3339(&p.expires_at).is_ok_and(|t|t>now));if cache.len()>=32{return Err(error("work_limit"));}
        let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        let(value,attempt_id)=store.work_transaction(|conn,s|{
            let current=work_store::job(conn,&job.id)?;if let Some(c)=work_store::command_by_key(conn,&current.assessment_id,key)?{check_command(&c,"work.operation.enqueue",&hash)?;return Ok((job_command_result(conn,s,&c,true)?,None));}
            let mut j=work_store::check_job_request(conn,s,job,revision,true)?;if j.state!=JobState::AwaitingApproval||work_store::resolve_inputs(conn,s,&j)?!=inputs{return Err(error("work_input_changed"));}
            if !chrono::DateTime::parse_from_rfc3339(&expires).is_ok_and(|t|t>chrono::Utc::now()){return Err(error("work_operation_expired"));}
            let backend=match &compiled{CompiledOperationRef::AiInvestigation{investigation_id,digest,..}=>{if j.compiled.as_ref()!=Some(&compiled)||work_store::investigation_binding(conn,investigation_id)?.as_ref()!=Some(&(j.id.clone(),digest.clone())){return Err(error("work_owned_operation"));}crate::work_supervisor::assert_investigation_attach_available(conn,investigation_id)?;Backend::Ai},CompiledOperationRef::KaliEvidence{..}=>{crate::work_supervisor::assert_kali_review_available(conn,review)?;Backend::Kali},_=>return Err(error("work_invalid_request"))};
            let(attempts,remaining,profile)=work_store::remaining_quotas(conn,s,&j.revision)?;if attempts==0{return Err(error("work_limit"));}if cap>remaining||cap>profile{return Err(error("work_capture_quota"));}
            let attempt_id=model::id();let number=j.current_attempt.as_ref().map_or(1,|a|a.number+1);let a=work_store::WorkAttempt{view:AttemptView{id:attempt_id.clone(),job_id:j.id.clone(),number,previous_attempt_id:j.current_attempt.as_ref().map(|a|a.id.clone()),state:AttemptState::Reserved,backend,effect:None,cleanup:CleanupState::NotApplicable,created_at:model::now(),dispatch_intent_at:None,started_at:None,finished_at:None,results:vec![],capture_ids:vec![],data_receipt_refs:vec![],reasons:vec![]},boot_id:supervisor.arbiter().boot_id().into(),reserved_capture_bytes:cap,reserved_critical_events:256,compiled:compiled.clone(),input_refs:inputs.clone(),reserved_effect:Some(reserved_effect.clone()),data_review_ref:Some(review.clone()),kali_binding:None};
            work_store::insert_attempt(conn,&a)?;j.compiled=Some(compiled.clone());j.current_attempt=Some(AttemptRef{id:attempt_id.clone(),number});j.state=JobState::Queued;j.generation+=1;work_store::save_job(conn,&j)?;
            let c=completed_command(&j,"work.operation.enqueue",key,&hash,work_store::CommandOutput::Job{job_id:j.id.clone()},None);work_store::insert_command(conn,&c)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::AttemptReserved,Some(j.id),Some(attempt_id.clone()),Some(JobState::Queued),None,None)?;work_store::enforce_caps(conn)?;Ok((job_command_result(conn,s,&c,false)?,Some(attempt_id)))
        })?;
        if let Some(id)=attempt_id{cache.insert(id,PendingBackendGrant{boot_id:supervisor.arbiter().boot_id().into(),expires_at:expires,grant});}Ok(value)
    }
    pub fn handle(&self,action:&str,payload:Value,store:&Store,redactor:&dyn DataRedactor)->Result<Value,String>{
        if !payload.is_object()||payload.to_string().len()>128*1024{return Err(error("work_invalid_request"));}
        match action {
            "work.plan.prepare"=>{let request=decode(payload)?;serde_json::to_value(self.prepare(store,request,redactor)?).map_err(storage_error)},
            "work.plan.commit"=>{let request=decode(payload)?;serde_json::to_value(self.commit(store,request,redactor)?).map_err(storage_error)},
            "work.plan.get"=>{let input:PlanGetRequest=decode(payload)?;valid_id(&input.assessment_id)?;if let Some(id)=&input.revision_id{valid_id(id)?;}store.work_read(|conn,s|{if !s.assessments.iter().any(|a|a.id==input.assessment_id){return Err(error("work_invalid_request"));}Ok(json!({"plan":work_store::plan_view(conn,s,&input.assessment_id,input.revision_id.as_deref())?,"capabilities":capabilities()}))})},
            "work.queue"=>{let input:QueueRequest=decode(payload)?;let mut response=store.work_read(|conn,s|work_store::queue(conn,s,input.assessment_id.as_deref()))?;if response["blockedBy"].as_array().is_some_and(Vec::is_empty)&&crate::work_supervisor::backend_unknown(store)?{response["blockedBy"]=json!([WorkReason::new(WorkReasonCode::WorkUnknownEffect,None)]);}Ok(response)},
            "work.events"=>{let input:EventsRequest=decode(payload)?;store.work_read(|conn,s|work_store::events(conn,s,&input))},
            _=>Err(error("work_invalid_request")),
        }
    }
    pub fn prepare(&self,store:&Store,input:PlanPrepareRequest,redactor:&dyn DataRedactor)->Result<PlanPreparation,String>{
        self.prepare_bound(store, input, redactor, None)
    }
    fn prepare_bound(&self,store:&Store,input:PlanPrepareRequest,redactor:&dyn DataRedactor,source:Option<SuggestionSource>)->Result<PlanPreparation,String>{
        valid_id(&input.assessment_id)?;checked_text(&input.goal,2000,redactor)?;
        if input.nodes.is_empty()||input.nodes.len()>MAX_NODES||input.expected_generation>MAX_SAFE_INTEGER{return Err(error("work_limit"));}
        let prepared=store.work_read(|conn,s|{
            if let Some(source) = &source { source.validate_live(conn, s)?; }
            let head=work_store::head_for_assessment(conn,&input.assessment_id)?;
            work_store::check_expected_head(conn,head.as_ref(),input.expected_head.as_ref(),input.expected_generation)?;
            let previous=head.as_ref().map(|h|work_store::revision(conn,&h.head_revision_id)).transpose()?;
            let mut mapping=BTreeMap::new();let mut used_ids=BTreeSet::new();let mut node_map=vec![];
            for n in &input.nodes{
                checked_text(&n.purpose,500,redactor)?;checked_text(&n.expected_observation,500,redactor)?;
                if n.key.is_empty()||n.key.len()>32||!n.key.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_'||b==b'-')||mapping.contains_key(&n.key){return Err(error("work_invalid_request"));}
                let node_id=if let Some(id)=&n.existing_node_id{valid_id(id)?;if !previous.as_ref().is_some_and(|r|r.body.nodes.iter().any(|old|old.id==*id))||work_store::node_dispatched(conn,head.as_ref().map(|h|h.plan_id.as_str()).unwrap_or(""),id)?{return Err(error("work_revision_changed"));}id.clone()}else{model::id()};
                if !used_ids.insert(node_id.clone()){return Err(error("work_invalid_request"));}mapping.insert(n.key.clone(),node_id.clone());node_map.push(NodeMapping{key:n.key.clone(),node_id});
            }
            let resolve=|input:&DraftInput|->Result<WorkInput,String>{Ok(match input{DraftInput::Evidence{evidence_id,expected_raw_sha256}=>WorkInput::Evidence{evidence_id:evidence_id.clone(),expected_raw_sha256:expected_raw_sha256.clone()},DraftInput::DependencyOutput{node_key,output_role}=>WorkInput::DependencyOutput{node_id:mapping.get(node_key).ok_or_else(||error("work_invalid_request"))?.clone(),output_role:*output_role}})};
            let mut nodes=vec![];
            for n in &input.nodes{
                let intent=match &n.intent{
                    DraftIntent::Native{adapter,target_ref,ports}=>OperationIntent::Native{adapter:*adapter,target_ref:target_ref.clone(),ports:ports.clone()},
                    DraftIntent::HumanReview{inputs}=>OperationIntent::HumanReview{inputs:inputs.iter().map(&resolve).collect::<Result<_,_>>()?},
                    DraftIntent::AiInvestigation{inputs,perspectives}=>OperationIntent::AiInvestigation{inputs:inputs.iter().map(&resolve).collect::<Result<_,_>>()?,perspectives:perspectives.clone()},
                    DraftIntent::KaliEvidence{operation,input}=>OperationIntent::KaliEvidence{operation:*operation,input:resolve(input)?},
                };
                let depends_on=n.depends_on_keys.iter().map(|key|mapping.get(key).cloned().ok_or_else(||error("work_invalid_request"))).collect::<Result<_,_>>()?;
                nodes.push(ActionNode{id:mapping[&n.key].clone(),purpose:n.purpose.clone(),expected_observation:n.expected_observation.clone(),depends_on,intent});
            }
            let scope_ref=scope::current_ref(s,&input.assessment_id)?.ok_or("scope_unreviewed: revise o escopo da avaliação.")?;
            let body=ActionPlanBody{schema:"openkoda.action-plan.v1".into(),assessment_id:input.assessment_id.clone(),scope_ref,goal:input.goal.clone(),nodes,limits:input.limits.clone()};check_current_body(s,&body)?;
            let mut proposal=ActionPlanRevision{id:model::id(),plan_id:head.as_ref().map(|h|h.plan_id.clone()).unwrap_or_else(model::id),assessment_id:input.assessment_id.clone(),number:previous.as_ref().map_or(1,|r|r.number+1),previous_revision_id:previous.as_ref().map(|r|r.id.clone()),body,created_at:model::now(),author:"local-user".into(),digest:String::new(),suggestion_source:source};
            if let Some(origin) = &proposal.suggestion_source { origin.validate_revision(&proposal)?; }
            if proposal.number>MAX_REVISIONS{return Err(error("work_limit"));}proposal.digest=revision_digest(&proposal)?;
            let superseded_job_ids=if let Some(h)=&head{work_store::jobs_for_plan(conn,&h.plan_id)?.into_iter().filter(|j|!j.state.terminal()&&!j.state.active()).map(|j|j.id).collect()}else{vec![]};
            let reused_results=proposal.body.nodes.iter().flat_map(|n|n.intent.inputs()).filter_map(|i|if let WorkInput::Evidence{evidence_id,expected_raw_sha256}=i{Some(ResultRef::Evidence{id:evidence_id.clone(),raw_sha256:expected_raw_sha256.clone()})}else{None}).collect();
            let review=WorkReviewRef{id:model::id(),digest:digest(&json!({"proposal":proposal,"expectedHead":input.expected_head,"expectedGeneration":input.expected_generation}))?,expires_at:(chrono::Utc::now()+chrono::Duration::minutes(5)).to_rfc3339()};
            Ok(PreparedPlan{response:PlanPreparation{review,proposal,node_map,superseded_job_ids,reused_results,warnings:vec![]},expected_head:input.expected_head.clone(),expected_generation:input.expected_generation})
        })?;
        let response=prepared.response.clone();self.cache_review(PendingReview::Plan(prepared))?;Ok(response)
    }
    pub fn commit(&self,store:&Store,input:PlanCommitRequest,redactor:&dyn DataRedactor)->Result<MutationResult<PlanView>,String>{
        valid_id(&input.idempotency_key)?;valid_id(&input.review.id)?;if !input.acknowledge_plan||!valid_hash(&input.review.digest)||!valid_time(&input.review.expires_at){return Err(error("work_invalid_request"));}
        let request_digest=digest(&json!({"action":"work.plan.commit","payload":input}))?;
        // Durable receipt first: replay must survive cache loss and boot.
        if let Some(result)=store.work_read(|conn,s|work_store::replay_plan_commit(conn,s,&input,&request_digest))?{return Ok(result);}
        let prepared=match self.reviews.lock().map_err(storage_error)?.get(&input.review.id).cloned(){Some(PendingReview::Plan(prepared))=>prepared,_=>return Err(error("work_review_expired"))};
        if prepared.response.review!=input.review||chrono::DateTime::parse_from_rfc3339(&input.review.expires_at).map_err(storage_error)?<=chrono::Utc::now(){return Err(error("work_review_expired"));}
        checked_text(&prepared.response.proposal.body.goal,2000,redactor)?;for n in &prepared.response.proposal.body.nodes{checked_text(&n.purpose,500,redactor)?;checked_text(&n.expected_observation,500,redactor)?;}
        let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        let result=store.work_transaction(|tx,s|work_store::commit_plan(tx,s,&prepared,&input,&request_digest))?;
        self.reviews.lock().map_err(storage_error)?.remove(&input.review.id);Ok(result)
    }
    fn cache_review(&self,review:PendingReview)->Result<(),String>{let mut cache=self.reviews.lock().map_err(storage_error)?;let now=chrono::Utc::now();cache.retain(|_,p|chrono::DateTime::parse_from_rfc3339(&p.reference().expires_at).is_ok_and(|t|t>now));if cache.len()>=32{return Err(error("work_limit"));}cache.insert(review.reference().id.clone(),review);Ok(())}

    pub(crate) fn handle_runtime(&self,action:&str,payload:Value,store:&Store,executor:&crate::executor::Executor,supervisor:&crate::work_supervisor::WorkSupervisor,redactor:&dyn DataRedactor)->Result<Value,String>{
        if !payload.is_object()||payload.to_string().len()>128*1024{return Err(error("work_invalid_request"));}
        match action{
            "work.operation.prepare"=>serde_json::to_value(self.prepare_operation(store,executor,decode(payload)?,None)?).map_err(storage_error),
            "work.operation.enqueue"=>self.enqueue_native(store,supervisor,decode(payload)?),
            "work.review.commit"=>self.commit_human(store,decode(payload)?,redactor),
            "work.pause"=>self.pause(store,decode(payload)?),
            "work.resume.prepare"=>self.prepare_resume(store,decode(payload)?),
            "work.resume.commit"=>self.commit_resume(store,decode(payload)?),
            "work.cancel"=>self.cancel_job(store,executor,decode(payload)?),
            "work.cancelPlan"=>self.cancel_plan(store,executor,decode(payload)?),
            "work.reconcile"=>self.reconcile_attempt(store,executor,decode(payload)?,redactor),
            "work.retry.prepare"=>{let input:RetryRequest=decode(payload)?;let revision=store.work_read(|conn,_|Ok(work_store::job(conn,&input.job.id)?.revision))?;let request=PrepareOperationRequest{job:input.job,revision,idempotency_key:input.idempotency_key};serde_json::to_value(self.prepare_operation(store,executor,request,Some(input.previous_attempt_id))?).map_err(storage_error)},
            _=>self.handle(action,payload,store,redactor),
        }
    }
    fn projected_preparation(&self,conn:&rusqlite::Connection,s:&Snapshot,command:&work_store::StoredCommand,replayed:bool)->Result<MutationResult<Preparation>,String>{
        let work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::Preparation{job_id,compiled,resolved_inputs}}=&command.result else{return Err(error(match &command.result{work_store::CommandResult::Pending{..}=>"work_busy",_=>"work_preparation_required"}));};
        let j=work_store::job(conn,job_id)?;let mut view=work_store::job_view(conn,s,&j)?;let current=j.compiled.as_ref()==Some(compiled)&&j.state==JobState::AwaitingApproval;
        if !current{view.can_enqueue=false;}
        let value=match compiled{
            CompiledOperationRef::Native{plan_id,digest,..}=>{
                let p=s.plans.iter().find(|p|p.id==*plan_id&&p.digest==*digest).ok_or_else(||error("work_input_changed"))?.clone();
                let mut reasons=vec![];if !current||!native_compiled_valid(s,&j,&p){view.can_enqueue=false;reasons.push(WorkReason::new(WorkReasonCode::WorkPreparationRequired,Some(j.id.clone())));}
                Preparation::Native{effective_limits:work_store::effective_limits(conn,s,&j,&p)?,job:view,compiled:compiled.clone(),plan:p,resolved_inputs:resolved_inputs.clone(),reasons}
            },
            CompiledOperationRef::HumanReview{review_id,..}=>{
                let cached=self.reviews.lock().map_err(storage_error)?.get(review_id).cloned();
                let review=if current{match cached{Some(PendingReview::Human(h)) if h.job.id==j.id&&h.job.generation==j.generation&&h.inputs==*resolved_inputs&&review_alive(&h.review)=>Some(h.review),_=>None}}else{None};
                let can_record=review.is_some()&&view.can_enqueue;let reasons=if can_record{vec![]}else{vec![WorkReason::new(WorkReasonCode::WorkPreparationRequired,Some(j.id.clone()))]};
                Preparation::HumanReview{job:view,review,resolved_inputs:resolved_inputs.clone(),input_digest:digest(resolved_inputs)?,prompt:"Revise os registros identificados e registre sua decisão. A decisão não aprova outra operação nem confirma um achado.".into(),can_record,reasons}
            },
            _=>return Err(error("work_intent_unavailable")),
        };Ok(MutationResult{replayed,receipt:receipt.clone(),value})
    }
    fn prepare_operation(&self,store:&Store,executor:&crate::executor::Executor,input:PrepareOperationRequest,previous_attempt:Option<String>)->Result<MutationResult<Preparation>,String>{
        valid_id(&input.idempotency_key)?;valid_id(&input.job.id)?;let action=if previous_attempt.is_some(){"work.retry.prepare"}else{"work.operation.prepare"};
        let request_digest=digest(&json!({"action":action,"job":input.job,"revision":input.revision,"idempotencyKey":input.idempotency_key,"previousAttemptId":previous_attempt}))?;
        let replay=store.work_read(|conn,s|{let j=work_store::job(conn,&input.job.id)?;if let Some(command)=work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?{check_command(&command,action,&request_digest)?;Ok(Some(self.projected_preparation(conn,s,&command,true)?))}else{Ok(None)}})?;
        if let Some(replay)=replay{return Ok(replay);}
        let start=store.work_transaction(|conn,s|{
            let current=work_store::job(conn,&input.job.id)?;
            if let Some(command)=work_store::command_by_key(conn,&current.assessment_id,&input.idempotency_key)?{check_command(&command,action,&request_digest)?;return Ok(PreparationStart::Existing(command));}
            let mut j=work_store::check_job_request(conn,s,&input.job,&input.revision,true)?;
            if let Some(previous)=&previous_attempt{
                let a=work_store::attempt(conn,previous)?;if j.current_attempt.as_ref().map(|r|r.id.as_str())!=Some(previous.as_str())||a.view.job_id!=j.id||!matches!(a.view.state,AttemptState::Completed|AttemptState::Partial|AttemptState::Failed|AttemptState::Canceled|AttemptState::NotStarted)||a.view.cleanup!=CleanupState::Confirmed{return Err(error("work_reconciliation_required"));}
            }else if j.state.terminal()||j.state.active()||matches!(j.state,JobState::Preparing|JobState::Queued){return Err(error("work_preparation_required"));}
            let inputs=work_store::resolve_inputs(conn,s,&j)?;
            if previous_attempt.is_none(){if let Some(CompiledOperationRef::Native{plan_id,..})=&j.compiled{
                if j.state==JobState::AwaitingApproval {if let Some(p)=s.plans.iter().find(|p|p.id==*plan_id){if native_compiled_valid(s,&j,p){
                    let receipt=new_receipt(action,&j.id,j.generation,&request_digest);let command=work_store::StoredCommand{assessment_id:j.assessment_id.clone(),idempotency_key:input.idempotency_key.clone(),review_id:None,result:work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::Preparation{job_id:j.id.clone(),compiled:j.compiled.clone().unwrap(),resolved_inputs:inputs}}};work_store::insert_command(conn,&command)?;return Ok(PreparationStart::Existing(command));
                }}}
            }}
            if let Some(current)=&j.current_attempt{let a=work_store::attempt(conn,&current.id)?;if a.view.state.active()||a.view.state==AttemptState::Reserved{return Err(error("work_reconciliation_required"));}}
            if let Some(CompiledOperationRef::Native{plan_id,..})=&j.compiled{invalidate_native_plan(s,plan_id)?;}
            if work_store::remaining_quotas(conn,s,&j.revision)?.0==0{return Err(error("work_limit"));}
            let revision=work_store::revision(conn,&j.revision.revision_id)?;let intent=revision.body.nodes.iter().find(|n|n.id==j.node_id).ok_or_else(||error("work_storage_unavailable"))?.intent.clone();
            if !matches!(intent,OperationIntent::Native{..}|OperationIntent::HumanReview{..}){return Err(error("work_intent_unavailable"));}
            j.state=JobState::Preparing;j.generation+=1;j.compiled=None;j.reasons.clear();work_store::save_job(conn,&j)?;
            let reserved_id=model::id();let receipt=new_receipt(action,&j.id,j.generation,&request_digest);
            let command=work_store::StoredCommand{assessment_id:j.assessment_id.clone(),idempotency_key:input.idempotency_key.clone(),review_id:None,result:work_store::CommandResult::Pending{receipt,entity_id:reserved_id.clone()}};work_store::insert_command(conn,&command)?;
            work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id.clone()),None,Some(j.state),None,None)?;
            Ok(PreparationStart::Compile{job:j,intent,inputs,reserved_id,command})
        })?;
        let PreparationStart::Compile{job,intent,inputs,reserved_id,mut command}=start else{let PreparationStart::Existing(command)=start else{unreachable!()};return store.work_read(|conn,s|self.projected_preparation(conn,s,&command,false));};
        let compiled:Result<(CompiledOperationRef,Option<crate::model::ExecutionPlan>,Option<PreparedHuman>),String>=(||{
            match intent{
                OperationIntent::Native{adapter,target_ref,ports}=>{
                    let input=crate::executor::NativeCompileInput{assessment_id:job.assessment_id.clone(),adapter:adapter.as_str().into(),scope_ref:store.work_read(|conn,_|Ok(work_store::revision(conn,&job.revision.revision_id)?.body.scope_ref))?,target_ref,ports};
                    let p=executor.compile_selected(store,&input,&reserved_id)?;Ok((CompiledOperationRef::Native{plan_id:p.id.clone(),digest:p.digest.clone(),expires_at:p.expires_at.clone()},Some(p),None))
                },
                OperationIntent::HumanReview{..}=>{
                    let input_digest=digest(&inputs)?;let review=WorkReviewRef{id:model::id(),digest:digest(&json!({"jobId":job.id,"generation":job.generation+1,"revision":job.revision,"inputDigest":input_digest}))?,expires_at:(chrono::Utc::now()+chrono::Duration::minutes(5)).to_rfc3339()};
                    let human=PreparedHuman{review:review.clone(),job:JobRef{id:job.id.clone(),generation:job.generation+1},revision:job.revision.clone(),inputs:inputs.clone(),input_digest};
                    self.cache_review(PendingReview::Human(human.clone()))?;Ok((CompiledOperationRef::HumanReview{review_id:review.id,digest:review.digest,expires_at:review.expires_at},None,Some(human)))
                },_=>Err(error("work_intent_unavailable")),
            }
        })();
        let finish=(||{
            let (compiled,plan,_)=compiled?;
            store.work_transaction(|conn,s|{
                let mut current=work_store::job(conn,&job.id)?;let h=work_store::head(conn,&current.revision.plan_id)?;
                if current.state!=JobState::Preparing||current.generation!=job.generation||h.head_revision_id!=job.revision.revision_id||h.control!=PlanControl::Active{return Err(error("work_revision_changed"));}
                check_current_body(s,&work_store::revision(conn,&job.revision.revision_id)?.body)?;if work_store::resolve_inputs(conn,s,&current)?!=inputs{return Err(error("work_input_changed"));}
                if let Some(p)=&plan{crate::executor::persist_prepared_locked(s,p)?;work_store::insert_plan_binding(conn,&p.id,&job.id,&input.idempotency_key,&p.digest)?;}
                current.state=JobState::AwaitingApproval;current.generation+=1;current.compiled=Some(compiled.clone());work_store::save_job(conn,&current)?;
                let mut receipt=command.result.receipt().clone();receipt.entity_generation=current.generation;command.result=work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::Preparation{job_id:job.id.clone(),compiled,resolved_inputs:inputs.clone()}};work_store::save_command(conn,&command)?;
                work_store::append_event(conn,&current.revision.plan_id,&current.assessment_id,WorkEventType::JobStateChanged,Some(current.id),None,Some(JobState::AwaitingApproval),None,None)?;Ok(())
            })?;store.work_read(|conn,s|self.projected_preparation(conn,s,&command,false))
        })();
        if let Err(original)=finish{
            store.work_transaction(|conn,_|{let mut current=work_store::job(conn,&job.id)?;
                if current.state==JobState::Preparing&&current.generation==job.generation{current.state=JobState::AwaitingPreparation;current.generation+=1;current.reasons=vec![WorkReason::new(WorkReasonCode::WorkPreparationRequired,Some(current.id.clone()))];work_store::save_job(conn,&current)?;}
                else if current.state==JobState::CancelRequested&&!work_store::job_has_unsettled_attempt(conn,&current)?{current.state=JobState::Canceled;current.generation+=1;work_store::save_job(conn,&current)?;}
                let saved=work_store::command_by_key(conn,&job.assessment_id,&input.idempotency_key)?.ok_or_else(||error("work_storage_unavailable"))?;
                if matches!(saved.result,work_store::CommandResult::Pending{..}){let mut failed=saved;failed.result=work_store::CommandResult::Failed{receipt:failed.result.receipt().clone(),entity_id:job.id.clone(),reason_code:WorkReasonCode::WorkPreparationRequired};work_store::save_command(conn,&failed)?;}
                Ok(())
            })?;return Err(original);
        }finish
    }
    fn enqueue_native(&self,store:&Store,supervisor:&crate::work_supervisor::WorkSupervisor,input:EnqueueRequest)->Result<Value,String>{
        let request_digest=digest(&json!({"action":"work.operation.enqueue","payload":input}))?;
        let EnqueueRequest::Native{job,revision,compiled,idempotency_key,acknowledge_execution}=input else{return Err(error("work_intent_unavailable"));};
        valid_id(&idempotency_key)?;if !acknowledge_execution{return Err(error("work_approval_required"));}
        let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        store.work_transaction(|conn,s|{
            let current=work_store::job(conn,&job.id)?;
            if let Some(command)=work_store::command_by_key(conn,&current.assessment_id,&idempotency_key)?{check_command(&command,"work.operation.enqueue",&request_digest)?;return job_command_result(conn,s,&command,true);}
            let mut j=work_store::check_job_request(conn,s,&job,&revision,true)?;
            if j.state!=JobState::AwaitingApproval||j.compiled.as_ref()!=Some(&compiled){return Err(error("work_approval_required"));}
            let CompiledOperationRef::Native{plan_id,digest,..}=&compiled else{return Err(error("work_intent_unavailable"));};
            let p=s.plans.iter().find(|p|p.id==*plan_id).ok_or_else(||error("work_input_changed"))?.clone();
            if !native_compiled_valid(s,&j,&p){return Err(error("work_operation_expired"));}
            let inputs=work_store::resolve_inputs(conn,s,&j)?;let limits=work_store::effective_limits(conn,s,&j,&p)?;
            if limits.remaining_attempts==0{return Err(error("work_limit"));}if limits.max_capture_bytes==0{return Err(error("work_capture_quota"));}
            crate::executor::approve_locked(s,plan_id,digest)?;
            let number=j.current_attempt.as_ref().map_or(1,|a|a.number+1);let attempt_id=model::id();
            let attempt=work_store::WorkAttempt{view:AttemptView{id:attempt_id.clone(),job_id:j.id.clone(),number,previous_attempt_id:j.current_attempt.as_ref().map(|a|a.id.clone()),state:AttemptState::Reserved,backend:Backend::Native,effect:None,cleanup:CleanupState::NotApplicable,created_at:model::now(),dispatch_intent_at:None,started_at:None,finished_at:None,results:vec![],capture_ids:vec![],data_receipt_refs:vec![],reasons:vec![]},boot_id:supervisor.arbiter().boot_id().into(),reserved_capture_bytes:limits.max_capture_bytes,reserved_critical_events:256,compiled:compiled.clone(),input_refs:inputs,reserved_effect:None,data_review_ref:None,kali_binding:None};
            work_store::insert_attempt(conn,&attempt)?;j.current_attempt=Some(AttemptRef{id:attempt_id.clone(),number});j.state=JobState::Queued;j.generation+=1;work_store::save_job(conn,&j)?;
            let command=completed_command(&j,"work.operation.enqueue",&idempotency_key,&request_digest,work_store::CommandOutput::Job{job_id:j.id.clone()},None);
            work_store::insert_command(conn,&command)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::AttemptReserved,Some(j.id.clone()),Some(attempt_id),Some(j.state),None,None)?;
            work_store::enforce_caps(conn)?;job_command_result(conn,s,&command,false)
        })
    }
    fn commit_human(&self,store:&Store,input:HumanCommitRequest,redactor:&dyn DataRedactor)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_review{return Err(error("work_approval_required"));}let request_digest=digest(&json!({"action":"work.review.commit","payload":input}))?;
        let replay=store.work_read(|conn,s|{let j=work_store::job(conn,&input.job.id)?;if let Some(c)=work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?{check_command(&c,"work.review.commit",&request_digest)?;Ok(Some(human_command_result(conn,s,&c,true)?))}else{Ok(None)}})?;if let Some(replay)=replay{return Ok(replay);}
        checked_text(&input.note,2000,redactor)?;
        let human=match self.reviews.lock().map_err(storage_error)?.get(&input.review.id).cloned(){Some(PendingReview::Human(h))=>h,_=>return Err(error("work_review_expired"))};
        if human.review!=input.review||human.job!=input.job||human.revision!=input.revision||human.input_digest!=input.input_digest||!review_alive(&human.review){return Err(error("work_review_expired"));}
        let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        let result=store.work_transaction(|conn,s|{
            let current=work_store::job(conn,&input.job.id)?;if let Some(c)=work_store::command_by_key(conn,&current.assessment_id,&input.idempotency_key)?{check_command(&c,"work.review.commit",&request_digest)?;return human_command_result(conn,s,&c,true);}
            if work_store::command_by_review(conn,&input.review.id)?.is_some(){return Err(error("work_review_used"));}if !review_alive(&human.review){return Err(error("work_review_expired"));}
            let mut j=work_store::check_job_request(conn,s,&input.job,&input.revision,true)?;if j.state!=JobState::AwaitingApproval||!matches!(&j.compiled,Some(CompiledOperationRef::HumanReview{review_id,digest,..}) if review_id==&input.review.id&&digest==&input.review.digest){return Err(error("work_approval_required"));}
            let inputs=work_store::resolve_inputs(conn,s,&j)?;if inputs!=human.inputs||digest(&inputs)?!=input.input_digest{return Err(error("work_input_changed"));}
            let mut record=HumanReviewRecord{id:model::id(),job_id:j.id.clone(),assessment_id:j.assessment_id.clone(),revision:j.revision.clone(),inputs,input_digest:input.input_digest.clone(),decision:input.decision,note:input.note.clone(),author:"local-user".into(),created_at:model::now(),digest:String::new()};record.digest=human_record_digest(&record)?;
            j.state=match input.decision{HumanDecision::Continue=>JobState::Completed,HumanDecision::NeedsMoreWork=>JobState::Partial,HumanDecision::Stop=>JobState::Canceled};j.generation+=1;j.results=vec![ResultRef::HumanReview{id:record.id.clone(),digest:record.digest.clone()}];work_store::save_job(conn,&j)?;
            let command=completed_command(&j,"work.review.commit",&input.idempotency_key,&request_digest,work_store::CommandOutput::HumanReview{record:record.clone()},Some(input.review.id.clone()));work_store::insert_command(conn,&command)?;
            work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::HumanReviewRecorded,Some(j.id.clone()),None,Some(j.state),None,Some(EffectRef::HumanReview{id:record.id}))?;
            crate::work_supervisor::release_dependents_locked(conn,s,&j.revision)?;human_command_result(conn,s,&command,false)
        })?;self.reviews.lock().map_err(storage_error)?.remove(&input.review.id);Ok(result)
    }
    fn pause(&self,store:&Store,input:PauseRequest)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_pause{return Err(error("work_invalid_request"));}let hash=digest(&json!({"action":"work.pause","payload":input}))?;
        let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        store.work_transaction(|conn,s|{let h=work_store::head(conn,&input.plan_id)?;if let Some(c)=work_store::command_by_key(conn,&h.assessment_id,&input.idempotency_key)?{check_command(&c,"work.pause",&hash)?;return plan_command_result(conn,s,&c,true);}
            if h.generation!=input.expected_generation{return Err(error("work_revision_changed"));}if h.control==PlanControl::Canceled{return Err(error("work_canceled"));}
            conn.execute("UPDATE work_plan_heads SET control='paused',generation=generation+1 WHERE plan_id=?1",[&h.plan_id]).map_err(storage_error)?;
            work_store::append_event(conn,&h.plan_id,&h.assessment_id,WorkEventType::PlanPaused,None,None,None,Some(WorkReasonCode::WorkPaused),None)?;
            let c=plan_control_command(&h,"work.pause",&input.idempotency_key,&hash,None);work_store::insert_command(conn,&c)?;plan_command_result(conn,s,&c,false)
        })
    }
    fn prepare_resume(&self,store:&Store,input:ResumePrepareRequest)->Result<Value,String>{
        let (prepared,value)=store.work_read(|conn,s|{
            let h=work_store::head(conn,&input.plan_id)?;if h.generation!=input.expected_generation||h.control!=PlanControl::Paused{return Err(error("work_revision_changed"));}
            let r=work_store::revision(conn,&h.head_revision_id)?;check_current_body(s,&r.body)?;
            let plan=work_store::plan_view(conn,s,&h.assessment_id,None)?.ok_or_else(||error("work_invalid_request"))?;
            let review=WorkReviewRef{id:model::id(),digest:digest(&json!({"planId":h.plan_id,"generation":h.generation,"head":plan.head,"jobs":plan.jobs}))?,expires_at:(chrono::Utc::now()+chrono::Duration::minutes(5)).to_rfc3339()};
            let ready=plan.jobs.iter().filter(|j|j.state==JobState::Queued).map(|j|j.id.clone()).collect::<Vec<_>>();let needs=plan.jobs.iter().filter(|j|matches!(j.state,JobState::AwaitingPreparation|JobState::AwaitingApproval|JobState::Revoked)).map(|j|j.id.clone()).collect::<Vec<_>>();
            Ok((PreparedResume{review:review.clone(),plan_id:h.plan_id,generation:h.generation},json!({"review":review,"plan":plan,"readyJobIds":ready,"requiresNewReview":needs,"reasons":[]})))
        })?;self.cache_review(PendingReview::Resume(prepared))?;Ok(value)
    }
    fn commit_resume(&self,store:&Store,input:ResumeCommitRequest)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_resume{return Err(error("work_invalid_request"));}let hash=digest(&json!({"action":"work.resume.commit","payload":input}))?;
        if let Some(value)=store.work_read(|conn,s|{if let Some(c)=work_store::command_by_review(conn,&input.review.id)?{if c.idempotency_key!=input.idempotency_key{return Err(error("work_review_used"));}check_command(&c,"work.resume.commit",&hash)?;Ok(Some(plan_command_result(conn,s,&c,true)?))}else{Ok(None)}})?{return Ok(value);}
        let prepared=match self.reviews.lock().map_err(storage_error)?.get(&input.review.id).cloned(){Some(PendingReview::Resume(p))=>p,_=>return Err(error("work_review_expired"))};
        if prepared.review!=input.review||!review_alive(&input.review){return Err(error("work_review_expired"));}
        let _scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        let result=store.work_transaction(|conn,s|{let h=work_store::head(conn,&prepared.plan_id)?;if let Some(c)=work_store::command_by_key(conn,&h.assessment_id,&input.idempotency_key)?{check_command(&c,"work.resume.commit",&hash)?;return plan_command_result(conn,s,&c,true);}
            if work_store::command_by_review(conn,&input.review.id)?.is_some(){return Err(error("work_review_used"));}if h.generation!=prepared.generation||h.control!=PlanControl::Paused||!review_alive(&input.review){return Err(error("work_revision_changed"));}
            check_current_body(s,&work_store::revision(conn,&h.head_revision_id)?.body)?;
            conn.execute("UPDATE work_plan_heads SET control='active',generation=generation+1 WHERE plan_id=?1",[&h.plan_id]).map_err(storage_error)?;
            work_store::append_event(conn,&h.plan_id,&h.assessment_id,WorkEventType::PlanResumed,None,None,None,None,None)?;
            let c=plan_control_command(&h,"work.resume.commit",&input.idempotency_key,&hash,Some(input.review.id.clone()));work_store::insert_command(conn,&c)?;plan_command_result(conn,s,&c,false)
        })?;self.reviews.lock().map_err(storage_error)?.remove(&input.review.id);Ok(result)
    }
    fn cancel_job(&self,store:&Store,executor:&crate::executor::Executor,input:CancelRequest)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_cancel{return Err(error("work_invalid_request"));}let hash=digest(&json!({"action":"work.cancel","payload":input}))?;
        let scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        let (value,aid)=store.work_transaction(|conn,s|{
            let mut j=work_store::job(conn,&input.job.id)?;if let Some(c)=work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?{check_command(&c,"work.cancel",&hash)?;return Ok((job_command_result(conn,s,&c,true)?,None));}
            if j.generation!=input.job.generation{return Err(error("work_revision_changed"));}cancel_locked(conn,s,&mut j)?;
            let signal=if j.state==JobState::CancelRequested{Some(j.assessment_id.clone())}else{None};
            let c=completed_command(&j,"work.cancel",&input.idempotency_key,&hash,work_store::CommandOutput::Job{job_id:j.id.clone()},None);work_store::insert_command(conn,&c)?;Ok((job_command_result(conn,s,&c,false)?,signal))
        })?;drop(scope);if let Some(aid)=aid{executor.signal_cancel(&aid)?;}Ok(value)
    }
    fn cancel_plan(&self,store:&Store,executor:&crate::executor::Executor,input:CancelPlanRequest)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_cancel{return Err(error("work_invalid_request"));}let hash=digest(&json!({"action":"work.cancelPlan","payload":input}))?;
        let scope=store.scope_gate.lock().map_err(|_|error("work_busy"))?;
        let (value,aid)=store.work_transaction(|conn,s|{let h=work_store::head(conn,&input.plan_id)?;if let Some(c)=work_store::command_by_key(conn,&h.assessment_id,&input.idempotency_key)?{check_command(&c,"work.cancelPlan",&hash)?;return Ok((plan_command_result(conn,s,&c,true)?,None));}
            if h.generation!=input.expected_generation{return Err(error("work_revision_changed"));}let mut signal=false;
            for mut j in work_store::jobs_for_plan(conn,&h.plan_id)?{if !j.state.terminal()&&j.state!=JobState::Unknown{cancel_locked(conn,s,&mut j)?;signal|=j.state==JobState::CancelRequested;}}
            conn.execute("UPDATE work_plan_heads SET control='canceled',generation=generation+1 WHERE plan_id=?1",[&h.plan_id]).map_err(storage_error)?;work_store::append_event(conn,&h.plan_id,&h.assessment_id,WorkEventType::PlanCanceled,None,None,None,Some(WorkReasonCode::WorkCanceled),None)?;
            let c=plan_control_command(&h,"work.cancelPlan",&input.idempotency_key,&hash,None);work_store::insert_command(conn,&c)?;Ok((plan_command_result(conn,s,&c,false)?,signal.then_some(h.assessment_id)))
        })?;drop(scope);if let Some(aid)=aid{executor.signal_cancel(&aid)?;}Ok(value)
    }
    fn reconcile_ai_attempt(&self,store:&Store,ai:&crate::ai::AiService,input:ReconcileRequest,redactor:&dyn DataRedactor)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_unknown_effects{return Err(error("work_invalid_request"));}checked_text(&input.note,2000,redactor)?;
        let hash=digest(&json!({"action":"work.reconcile","payload":input}))?;
        // A completed command replays even after its lease has been released.
        if let Some(value)=store.work_read(|conn,s|{let a=work_store::attempt(conn,&input.attempt_id)?;let j=work_store::job(conn,&a.view.job_id)?;work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?.map(|command|{check_command(&command,"work.reconcile",&hash)?;let mut value=job_command_result(conn,s,&command,true)?;value["value"]["reconciliationRequired"]=json!(value["value"]["attempt"]["state"]=="unknown");Ok(value)}).transpose()})?{return Ok(value);}
        ai.with_work_reconciliation(store,&input.attempt_id,|permit,proof|store.work_transaction(|conn,s|{
            let j=work_store::job(conn,&permit.candidate().job_id)?;
            if let Some(command)=work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?{check_command(&command,"work.reconcile",&hash)?;return job_command_result(conn,s,&command,true);}
            // Backend journal/receipt repair is idempotent. The final command,
            // attempt transition and lease release are a single durable commit.
            crate::work_supervisor::finish_ai_recovery_locked(conn,s,permit,proof)?;
            let j=work_store::job(conn,&j.id)?;
            let command=completed_command(&j,"work.reconcile",&input.idempotency_key,&hash,work_store::CommandOutput::Job{job_id:j.id.clone()},None);
            work_store::insert_command(conn,&command)?;
            let mut value=job_command_result(conn,s,&command,false)?;value["value"]["reconciliationRequired"]=json!(false);Ok(value)
        }))
    }
    fn reconcile_kali_attempt(&self,store:&Store,supervisor:&crate::work_supervisor::WorkSupervisor,kali:&crate::kali_bridge::KaliBridge,ai:&crate::ai::AiService,identity:&dyn Fn()->Result<String,String>,input:ReconcileRequest,redactor:&dyn DataRedactor)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_unknown_effects{return Err(error("work_invalid_request"));}checked_text(&input.note,2000,redactor)?;
        let hash=digest(&json!({"action":"work.reconcile","payload":input}))?;
        if let Some(value)=store.work_read(|conn,s|{let a=work_store::attempt(conn,&input.attempt_id)?;let j=work_store::job(conn,&a.view.job_id)?;work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?.map(|command|{check_command(&command,"work.reconcile",&hash)?;let mut value=job_command_result(conn,s,&command,true)?;value["value"]["reconciliationRequired"]=json!(value["value"]["attempt"]["state"]=="unknown");Ok(value)}).transpose()})?{return Ok(value);}
        let permit=supervisor.arbiter().claim_queued_reconciliation(store,&input.attempt_id,Backend::Kali)?;
        let mut command=None;
        kali.reconcile_work_evidence(store,&permit,identity,ai,|conn,s,permit,completion|{
            let j=work_store::job(conn,&permit.candidate().job_id)?;
            if work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?.is_some(){return Err(error("work_idempotency_conflict"));}
            // Evidence/seals, attempt, lease and the user's command receipt
            // share the transaction owned by the bridge's recovery callback.
            crate::work_supervisor::finish_kali_recovery_locked(conn,s,permit,completion)?;
            let j=work_store::job(conn,&j.id)?;let completed=completed_command(&j,"work.reconcile",&input.idempotency_key,&hash,work_store::CommandOutput::Job{job_id:j.id.clone()},None);
            work_store::insert_command(conn,&completed)?;command=Some(completed);Ok(())
        })?;
        let command=command.ok_or_else(||error("work_reconciliation_required"))?;
        store.work_read(|conn,s|{let mut value=job_command_result(conn,s,&command,false)?;value["value"]["reconciliationRequired"]=json!(value["value"]["attempt"]["state"]=="unknown");Ok(value)})
    }
    fn reconcile_attempt(&self,store:&Store,executor:&crate::executor::Executor,input:ReconcileRequest,redactor:&dyn DataRedactor)->Result<Value,String>{
        valid_id(&input.idempotency_key)?;if !input.acknowledge_unknown_effects{return Err(error("work_invalid_request"));}checked_text(&input.note,2000,redactor)?;let hash=digest(&json!({"action":"work.reconcile","payload":input}))?;
        let begin=store.work_transaction(|conn,s|{let a=work_store::attempt(conn,&input.attempt_id)?;let j=work_store::job(conn,&a.view.job_id)?;if let Some(c)=work_store::command_by_key(conn,&j.assessment_id,&input.idempotency_key)?{check_command(&c,"work.reconcile",&hash)?;return Ok((c,false,true));}
            if a.view.state!=AttemptState::Unknown||a.view.backend!=Backend::Native{return Err(error("work_reconciliation_required"));}
            if a.view.effect.is_none(){let reconciled=crate::work_supervisor::reconcile_unbound_native_locked(conn,s,&a.view.id)?;let c=completed_command(&reconciled,"work.reconcile",&input.idempotency_key,&hash,work_store::CommandOutput::Job{job_id:reconciled.id.clone()},None);work_store::insert_command(conn,&c)?;return Ok((c,false,false));}
            let c=work_store::StoredCommand{assessment_id:j.assessment_id.clone(),idempotency_key:input.idempotency_key.clone(),review_id:None,result:work_store::CommandResult::Pending{receipt:new_receipt("work.reconcile",&j.id,j.generation,&hash),entity_id:a.view.id}};work_store::insert_command(conn,&c)?;Ok((c,true,false))
        })?;
        let (mut command,run,replayed)=begin;
        if run{
            let result=executor.handle("assessment.reconcile",json!({"id":command.assessment_id,"note":input.note,"acknowledgeUnknownEffects":true}),store);
            store.work_transaction(|conn,_|{let j=work_store::job(conn,&command.result.receipt().entity_id)?;let mut receipt=command.result.receipt().clone();receipt.entity_generation=j.generation;
                command.result=if result.is_ok(){work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::Job{job_id:j.id}}}else{work_store::CommandResult::Failed{receipt,entity_id:j.id,reason_code:WorkReasonCode::WorkReconciliationRequired}};work_store::save_command(conn,&command)
            })?;result?;
        }
        store.work_read(|conn,s|{let mut result=job_command_result(conn,s,&command,replayed)?;let unknown=result["value"]["attempt"]["state"]=="unknown";result["value"]["reconciliationRequired"]=json!(unknown);Ok(result)})
    }
}

enum PreparationStart {Existing(work_store::StoredCommand),Compile{job:work_store::WorkJob,intent:OperationIntent,inputs:Vec<ResultRef>,reserved_id:String,command:work_store::StoredCommand}}
enum BackendPreparationStart {Existing(work_store::StoredCommand),Compile{job:work_store::WorkJob,intent:OperationIntent,inputs:Vec<ResultRef>,command:work_store::StoredCommand}}
fn review_alive(review:&WorkReviewRef)->bool{chrono::DateTime::parse_from_rfc3339(&review.expires_at).is_ok_and(|t|t>chrono::Utc::now())}
fn check_command(command:&work_store::StoredCommand,action:&str,digest:&str)->Result<(),String>{let receipt=command.result.receipt();if receipt.action!=action||receipt.request_digest!=digest{return Err(error("work_idempotency_conflict"));}Ok(())}
fn new_receipt(action:&str,entity_id:&str,generation:u64,request_digest:&str)->CommandReceipt{CommandReceipt{id:model::id(),action:action.into(),entity_id:entity_id.into(),entity_generation:generation,request_digest:request_digest.into(),created_at:model::now()}}
fn native_compiled_valid(s:&Snapshot,j:&work_store::WorkJob,p:&crate::model::ExecutionPlan)->bool{p.assessment_id==j.assessment_id&&p.status=="proposed"&&p.approved_at.is_none()&&p.digest==crate::executor::plan_digest(p)&&chrono::DateTime::parse_from_rfc3339(&p.expires_at).is_ok_and(|t|t>chrono::Utc::now())&&p.timeout_seconds==s.settings.timeout_seconds&&p.output_limit_ki_b==s.settings.output_limit_ki_b}
fn invalidate_native_plan(s:&mut Snapshot,plan_id:&str)->Result<(),String>{let p=s.plans.iter_mut().find(|p|p.id==plan_id).ok_or_else(||error("work_input_changed"))?;if matches!(p.status.as_str(),"running"|"canceling"|"unknown"){return Err(error("work_unknown_effect"));}if matches!(p.status.as_str(),"proposed"|"approved"){p.status="revoked".into();}let aid=p.assessment_id.clone();if let Some(a)=s.assessments.iter_mut().find(|a|a.id==aid){if matches!(a.status.as_str(),"awaiting_approval"|"queued"){a.status="blocked".into();a.updated_at=model::now();}}Ok(())}
fn completed_command(j:&work_store::WorkJob,action:&str,key:&str,hash:&str,output:work_store::CommandOutput,review_id:Option<String>)->work_store::StoredCommand{work_store::StoredCommand{assessment_id:j.assessment_id.clone(),idempotency_key:key.into(),review_id,result:work_store::CommandResult::Completed{receipt:new_receipt(action,&j.id,j.generation,hash),output}}}
fn plan_control_command(h:&work_store::PlanHead,action:&str,key:&str,hash:&str,review_id:Option<String>)->work_store::StoredCommand{work_store::StoredCommand{assessment_id:h.assessment_id.clone(),idempotency_key:key.into(),review_id,result:work_store::CommandResult::Completed{receipt:new_receipt(action,&h.plan_id,h.generation+1,hash),output:work_store::CommandOutput::Plan{plan_id:h.plan_id.clone(),revision_id:h.head_revision_id.clone()}}}}
fn plan_command_result(conn:&rusqlite::Connection,s:&Snapshot,c:&work_store::StoredCommand,replayed:bool)->Result<Value,String>{let work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::Plan{revision_id,..}}=&c.result else{return Err(error("work_reconciliation_required"));};let value=work_store::plan_view(conn,s,&c.assessment_id,Some(revision_id))?.ok_or_else(||error("work_storage_unavailable"))?;Ok(json!(MutationResult{replayed,receipt:receipt.clone(),value}))}
fn job_command_result(conn:&rusqlite::Connection,s:&Snapshot,c:&work_store::StoredCommand,replayed:bool)->Result<Value,String>{let work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::Job{job_id}}=&c.result else{return Err(error("work_reconciliation_required"));};let j=work_store::job(conn,job_id)?;let attempt=j.current_attempt.as_ref().map(|r|work_store::attempt(conn,&r.id).map(|a|a.view)).transpose()?;let terminated=attempt.as_ref().is_none_or(|a|a.cleanup==CleanupState::Confirmed)&&j.state!=JobState::CancelRequested;Ok(json!(MutationResult{replayed,receipt:receipt.clone(),value:json!({"job":work_store::job_view(conn,s,&j)?,"attempt":attempt,"terminationConfirmed":terminated})}))}
fn binding_command_result(conn:&rusqlite::Connection,s:&Snapshot,c:&work_store::StoredCommand,replayed:bool)->Result<Value,String>{let work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::InvestigationBinding{job_id,compiled,..}}=&c.result else{return Err(error("work_preparation_required"));};let j=work_store::job(conn,job_id)?;Ok(json!(MutationResult{replayed,receipt:receipt.clone(),value:json!({"job":work_store::job_view(conn,s,&j)?,"compiled":compiled})}))}
fn human_command_result(conn:&rusqlite::Connection,s:&Snapshot,c:&work_store::StoredCommand,replayed:bool)->Result<Value,String>{let work_store::CommandResult::Completed{receipt,output:work_store::CommandOutput::HumanReview{record}}=&c.result else{return Err(error("work_reconciliation_required"));};let j=work_store::job(conn,&record.job_id)?;Ok(json!(MutationResult{replayed,receipt:receipt.clone(),value:json!({"job":work_store::job_view(conn,s,&j)?,"record":record})}))}
fn cancel_locked(conn:&rusqlite::Connection,s:&mut Snapshot,j:&mut work_store::WorkJob)->Result<(),String>{
    if j.state==JobState::Unknown{return Err(error("work_reconciliation_required"));}if j.state.terminal(){return Err(error("work_canceled"));}
    if j.state==JobState::CancelRequested{return Ok(());}
    if j.state.active()||j.state==JobState::Preparing{j.state=JobState::CancelRequested;if let Some(current)=&j.current_attempt{let mut a=work_store::attempt(conn,&current.id)?;if matches!(a.view.state,AttemptState::Dispatching|AttemptState::Running){a.view.state=AttemptState::CancelRequested;work_store::save_attempt(conn,&a)?;}}}
    else{
        if let Some(current)=&j.current_attempt{let mut a=work_store::attempt(conn,&current.id)?;if a.view.state==AttemptState::Reserved&&a.view.effect.is_none(){a.view.state=AttemptState::NotStarted;a.view.finished_at=Some(model::now());a.view.cleanup=CleanupState::Confirmed;a.reserved_capture_bytes=0;a.reserved_critical_events=0;work_store::save_attempt(conn,&a)?;}else if a.view.state.active(){return Err(error("work_unknown_effect"));}}
        if let Some(CompiledOperationRef::Native{plan_id,..})=&j.compiled{invalidate_native_plan(s,plan_id)?;}j.state=JobState::Canceled;j.compiled=None;
    }
    j.generation+=1;j.reasons=vec![WorkReason::new(WorkReasonCode::WorkCanceled,Some(j.id.clone()))];work_store::save_job(conn,j)?;work_store::append_event(conn,&j.revision.plan_id,&j.assessment_id,WorkEventType::JobStateChanged,Some(j.id.clone()),j.current_attempt.as_ref().map(|a|a.id.clone()),Some(j.state),Some(WorkReasonCode::WorkCanceled),None)?;Ok(())
}
