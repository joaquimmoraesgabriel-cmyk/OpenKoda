use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub target: String,
    pub description: String,
    pub created_at: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_targets: Vec<crate::scope_targets::ProjectTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_receipts: Vec<crate::scope_targets::CatalogReceipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub at: String,
    pub message: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub name: String,
    pub content: String,
    pub sha256: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Assessment {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub kind: String,
    pub target: String,
    pub objective: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub events: Vec<Event>,
    pub evidence: Vec<Evidence>,
    pub coverage: Vec<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_selection: Option<crate::ai_provider::AiSelection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub assessment_id: String,
    pub title: String,
    pub severity: String,
    pub confidence: String,
    pub status: String,
    pub description: String,
    pub recommendation: String,
    pub evidence_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub mode: String,
    pub theme: String,
    pub language: String,
    pub retention_days: u32,
    pub timeout_seconds: u32,
    pub output_limit_ki_b: u32,
    pub has_api_key: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: "openai".into(), endpoint: "https://api.openai.com/v1".into(), model: String::new(),
            mode: "assisted".into(), theme: "light".into(), language: "pt-BR".into(),
            retention_days: 90, timeout_seconds: 600, output_limit_ki_b: 64,
            has_api_key: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub projects: Vec<Project>,
    pub assessments: Vec<Assessment>,
    pub findings: Vec<Finding>,
    pub settings: Settings,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub provider_models: std::collections::BTreeMap<crate::ai_provider::ProviderId, crate::ai_provider::ProviderModelPreference>,
    #[serde(default)]
    pub plans: Vec<ExecutionPlan>,
    #[serde(default)]
    pub executions: Vec<Execution>,
    #[serde(default)]
    pub validations: Vec<ValidationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_revisions: Vec<crate::scope::ScopeRevision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assessment_scopes: Vec<crate::scope::AssessmentScope>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_receipts: Vec<crate::scope::ScopeReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_policies: Vec<crate::data_policy::ProjectDataPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_policy_revisions: Vec<crate::data_policy::DataPolicyRevision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_bindings: Vec<crate::data_policy::AssessmentDataBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_classifications: Vec<crate::data_policy::DataClassificationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credential_references: Vec<crate::credential_refs::CredentialReferenceRevision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_use_receipts: Vec<crate::data_policy::DataUseReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_slots: Vec<crate::credential_refs::ProviderSlotMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_mutation_receipts: Vec<crate::data_policy::DataMutationReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_subject_generations: Vec<crate::data_policy::DataSubjectGeneration>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ValidatedEvidence { pub id:String, pub sha256:String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ValidationRecord {
    pub id:String, pub finding_id:String, pub assessment_id:String, pub method:String,
    pub expected:String, pub observed:String, pub evidence:Vec<ValidatedEvidence>,
    pub author:String, pub created_at:String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputFile { pub path: String, pub size: u64, pub sha256: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionPlan {
    pub id: String, pub assessment_id: String, pub adapter: String, pub target: String,
    pub timeout_seconds: u32, pub output_limit_ki_b: u32,
    pub input_files: Vec<InputFile>, pub resolved_addresses: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scope: Option<crate::source_scan::SourceScope>,
    pub digest: String, pub created_at: String, pub expires_at: String,
    pub approved_at: Option<String>, pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<crate::scope::ScopeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<crate::scope_targets::ScopeTargetRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_ports: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identity_preflight_refs: Vec<crate::scope_targets::ScopeTargetRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_omissions: Vec<crate::scope_targets::ScopeOmission>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Execution {
    pub id: String, pub plan_id: String, pub assessment_id: String, pub status: String,
    pub started_at: String, pub finished_at: Option<String>, pub evidence_id: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub scope_admission: Option<crate::scope::ScopeAdmission>,
}

pub fn now() -> String { chrono::Utc::now().to_rfc3339() }
pub fn id() -> String { uuid::Uuid::new_v4().to_string() }
pub fn event(message: impl Into<String>, kind: &str) -> Event {
    Event { id: id(), at: now(), message: message.into(), kind: kind.into() }
}
