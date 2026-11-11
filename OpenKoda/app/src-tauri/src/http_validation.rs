//! Local factual review of host HEAD observations. This module never sends HTTP.
use crate::{core::{self, Store}, executor, model::{self, Assessment, Evidence, Execution, ExecutionPlan, Finding, Snapshot, ValidatedEvidence, ValidationRecord}};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const RULE: &str = "http.x-content-type-options-nosniff.v1";
pub const OBSERVATION_SCHEMA: &str = "openkoda.http-observation.v2";
const ADAPTER: &str = "http.headers.v1";
const CLAIM: &str = "openkoda.http-claim.v1";
const VALIDATION: &str = "openkoda.http-validation.v1";
const RETEST: &str = "openkoda.http-retest.v1";
const CONFIRMATION: &str = "openkoda.http-confirmation.v1";
const TITLE: &str = "Resposta HEAD sem X-Content-Type-Options";
const CONFIDENCE: &str = "Fato da resposta observada; impacto não determinado";
const RECOMMENDATION: &str = "Revisar a configuração de X-Content-Type-Options para esta resposta e, se adequado, configurar nosniff. Preparar e aprovar uma nova observação HEAD antes de confirmar o reteste.";
pub const LIMITS: &str = "Conclusão limitada à resposta HEAD, URL, destinos, peer e instante registrados. Não demonstra exploração, impacto, comportamento de navegador, respostas GET, cobertura do site inteiro ou permanência da configuração. Nenhuma mudança remota é aplicada por esta revisão.";
pub const HEADER_NAMES: [&str; 10] = ["content-type", "content-length", "server", "cache-control", "location", "strict-transport-security", "content-security-policy", "x-content-type-options", "x-frame-options", "referrer-policy"];

fn failure() -> String { "Prova HTTP inconsistente ou fora da avaliação; nenhum registro foi alterado.".into() }
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> { serde_json::from_value(value).map_err(|_| "Campos inválidos na revisão HTTP.".into()) }
fn digest<T: Serialize>(value: &T) -> Result<String, String> { Ok(core::hash(&serde_json::to_string(value).map_err(|_| failure())?)) }
fn stamp(value: &str) -> Result<chrono::DateTime<chrono::FixedOffset>, String> { chrono::DateTime::parse_from_rfc3339(value).map_err(|_| failure()) }
fn sha(value: &str) -> bool { value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) }
fn note(value: &str, redact: &dyn Fn(&str) -> String) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 2000 || value.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { return Err("Registre justificativa de 1 a 2.000 bytes UTF-8, sem controles inesperados.".into()); }
    Ok(redact(value))
}
fn check_rule(value: &str) -> Result<(), String> { if value == RULE { Ok(()) } else { Err("Regra HTTP não suportada.".into()) } }
fn require_digest(actual: &str, expected: &str) -> Result<(), String> { if sha(actual) && actual == expected { Ok(()) } else { Err("A revisão HTTP mudou. Atualize os fatos e revise novamente.".into()) } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome { ControlMissing, ControlPresent, Inconclusive }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeaderFact { pub text: Option<String>, pub bytes: usize, pub sha256: String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub execution_id: String, pub plan_id: String, pub plan_digest: String,
    pub assessment_id: String, pub project_id: String, pub target: String,
    pub execution_status: String, pub started_at: String, pub finished_at: Option<String>,
    pub approved_at: Option<String>, pub plan_created_at: String,
    pub evidence_id: Option<String>, pub evidence_sha256: Option<String>,
    pub schema: Option<String>, pub status_code: Option<u16>,
    pub resolved_addresses: Vec<String>, pub remote_address: Option<String>,
    pub header_values: Vec<HeaderFact>, pub outcome: Outcome, pub reasons: Vec<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub target_ref:Option<crate::scope_targets::ScopeTargetRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Comparison { pub different_execution: bool, pub different_plan: bool, pub after_validation: bool, pub same_target: bool, pub same_resolved_addresses: bool, pub same_peer: bool }
impl Comparison { fn comparable(&self) -> bool { self.different_execution && self.different_plan && self.after_validation && self.same_target && self.same_resolved_addresses && self.same_peer } }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reference { id: String, sha256: String }
impl Reference { fn of(value: &Evidence) -> Self { Self { id: value.id.clone(), sha256: value.sha256.clone() } } }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Claim { schema: String, rule_id: String, finding_id: String, assessment_id: String, baseline: Observation, created_at: String, limits: String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Validation { schema: String, finding_id: String, assessment_id: String, claim: Reference, record_id: String, rationale: String, reviewed_state_digest: String, created_at: String, author: String, acknowledge_limits: bool }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Retest { schema: String, finding_id: String, assessment_id: String, claim: Reference, validation: Reference, observation: Observation, comparison: Comparison, outcome: Outcome, reasons: Vec<String>, review_digest: String, created_at: String, limits: String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirmation { schema: String, finding_id: String, assessment_id: String, claim: Reference, validation: Reference, retest: Reference, record_id: String, rationale: String, reviewed_state_digest: String, created_at: String, author: String, acknowledge_limits: bool }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Capture {
    schema: String, adapter: String, plan_digest: String, target: String, status: u16,
    headers: BTreeMap<String, String>, resolved_addresses: Vec<String>, redirect_followed: bool, limits: String,
    method: String, capture_complete: bool, remote_address: Option<String>, header_values: BTreeMap<String, Vec<String>>,
}
struct Bound<'a> { assessment: &'a Assessment, execution: &'a Execution, plan: &'a ExecutionPlan, evidence: Option<&'a Evidence> }

fn bound<'a>(s: &'a Snapshot, execution_id: &str) -> Result<Bound<'a>, String> {
    core::valid_id(execution_id)?;
    let e = s.executions.iter().find(|e| e.id == execution_id).ok_or_else(failure)?;
    let p = s.plans.iter().find(|p| p.id == e.plan_id && p.assessment_id == e.assessment_id).ok_or_else(failure)?;
    let a = s.assessments.iter().find(|a| a.id == e.assessment_id).ok_or_else(failure)?;
    let project = s.projects.iter().find(|project| project.id == a.project_id).ok_or_else(failure)?;
    let selected=executor::plan_target(s,p)?;
    let kind=selected.as_ref().map_or(a.kind.as_str(),|target|target.kind.as_str());
    if p.adapter != ADAPTER || kind != "web" || (p.target_ref.is_none()&&(project.kind != "web" || p.target != a.target || project.target != a.target)) || p.digest != executor::plan_digest(p) || p.resolved_addresses.is_empty() || p.resolved_addresses.len() > 32 { return Err(failure()); }
    for endpoint in &p.resolved_addresses { endpoint.parse::<std::net::SocketAddr>().map_err(|_| failure())?; }
    let created = stamp(&p.created_at)?; let started = stamp(&e.started_at)?;
    let approved = stamp(p.approved_at.as_deref().ok_or_else(failure)?)?;
    if created > approved || approved > started || started > stamp(&p.expires_at)? { return Err(failure()); }
    if let Some(finished) = &e.finished_at { if started > stamp(finished)? { return Err(failure()); } }
    let evidence = e.evidence_id.as_deref().map(|id| {
        if s.executions.iter().filter(|other| other.evidence_id.as_deref() == Some(id)).count() != 1 { return Err(failure()); }
        let raw = a.evidence.iter().find(|raw| raw.id == id).ok_or_else(failure)?;
        if raw.sha256 != core::hash(&raw.content) || raw.content.len() > p.output_limit_ki_b as usize * 1024 { return Err(failure()); }
        let at = stamp(&raw.created_at)?;
        if at < started || e.finished_at.as_deref().is_some_and(|end| stamp(end).is_ok_and(|end| at > end)) { return Err(failure()); }
        Ok(raw)
    }).transpose()?;
    if e.status == "completed" && (p.status != "completed" || e.finished_at.is_none() || evidence.is_none()) { return Err(failure()); }
    Ok(Bound { assessment: a, execution: e, plan: p, evidence })
}

pub fn observe(s: &Snapshot, execution_id: &str) -> Result<Observation, String> {
    let Bound { assessment: a, execution: e, plan: p, evidence } = bound(s, execution_id)?;
    let mut out = Observation { execution_id: e.id.clone(), plan_id: p.id.clone(), plan_digest: p.digest.clone(), assessment_id: a.id.clone(), project_id: a.project_id.clone(), target: p.target.clone(), execution_status: e.status.clone(), started_at: e.started_at.clone(), finished_at: e.finished_at.clone(), approved_at: p.approved_at.clone(), plan_created_at: p.created_at.clone(), evidence_id: evidence.map(|raw| raw.id.clone()), evidence_sha256: evidence.map(|raw| raw.sha256.clone()), schema: None, status_code: None, resolved_addresses: p.resolved_addresses.clone(), remote_address: None, header_values: vec![], outcome: Outcome::Inconclusive, reasons: vec![],target_ref:p.target_ref.clone() };
    if e.status != "completed" { out.reasons.push(if e.finished_at.is_none() { "execution_incomplete" } else { "execution_not_completed" }.into()); return Ok(out); }
    let raw = evidence.ok_or_else(failure)?;
    let value: Value = serde_json::from_str(&raw.content).map_err(|_| failure())?;
    out.schema = value["schema"].as_str().map(str::to_owned);
    if out.schema.as_deref() != Some(OBSERVATION_SCHEMA) { out.reasons.push("legacy_capture".into()); return Ok(out); }
    let capture: Capture = serde_json::from_value(value).map_err(|_| failure())?;
    if capture.schema != OBSERVATION_SCHEMA || capture.adapter != ADAPTER || capture.plan_digest != p.digest || capture.target != p.target || capture.resolved_addresses != p.resolved_addresses || capture.method != "HEAD" || capture.limits.is_empty() { return Err(failure()); }
    let keys: BTreeSet<_> = capture.header_values.keys().map(String::as_str).collect();
    if keys != HEADER_NAMES.iter().copied().collect() { return Err(failure()); }
    if capture.headers.keys().any(|name| !HEADER_NAMES.contains(&name.as_str())) { return Err(failure()); }
    // Preserve all raw occurrences in the Evidence. Public facts are bounded;
    // overlarge values are identified by bytes/hash and an explicit reason.
    let mut occurrences = 0usize;
    for (name, values) in &capture.header_values {
        occurrences = occurrences.checked_add(values.len()).ok_or_else(failure)?;
        if occurrences > 1024 { out.reasons.push("header_capture_limit".into()); return Ok(out); }
        for (index, encoded) in values.iter().enumerate() {
            let bytes = STANDARD.decode(encoded).map_err(|_| failure())?;
            if STANDARD.encode(&bytes) != *encoded { return Err(failure()); }
            let text = std::str::from_utf8(&bytes).ok().filter(|value| value.bytes().all(|b| b == b'\t' || (32..127).contains(&b)));
            if index == 0 && capture.headers.get(name).map(String::as_str) != Some(text.unwrap_or("[valor não textual]")) { return Err(failure()); }
            if name == "x-content-type-options" {
                if bytes.len() > 4096 { out.reasons.push("header_value_too_large".into()); }
                out.header_values.push(HeaderFact { text: text.filter(|_| bytes.len() <= 4096).map(str::to_owned), bytes: bytes.len(), sha256: format!("{:x}", Sha256::digest(&bytes)) });
            }
        }
        if values.is_empty() && capture.headers.contains_key(name) { return Err(failure()); }
    }
    out.status_code = Some(capture.status); out.remote_address = capture.remote_address.clone();
    if !capture.capture_complete { out.reasons.push("capture_incomplete".into()); }
    if capture.redirect_followed || (300..400).contains(&capture.status) { out.reasons.push("redirect_response".into()); }
    if !(200..300).contains(&capture.status) { out.reasons.push("invalid_status".into()); }
    match capture.remote_address.as_deref() {
        None => out.reasons.push("peer_unavailable".into()),
        Some(peer) => { let peer = peer.parse::<std::net::SocketAddr>().map_err(|_| failure())?; if !p.resolved_addresses.iter().any(|address| address.parse::<std::net::SocketAddr>().ok() == Some(peer)) { out.reasons.push("unapproved_peer".into()); } }
    }
    if out.header_values.len() > 1 { out.reasons.push("duplicate_header".into()); }
    if out.header_values.iter().any(|value| value.text.is_none()) { out.reasons.push("nontext_header".into()); }
    if out.reasons.is_empty() {
        out.outcome = match out.header_values.first() {
            None => Outcome::ControlMissing,
            Some(value) if value.text.as_deref().is_some_and(|value| value.trim_matches([' ', '\t']).eq_ignore_ascii_case("nosniff")) => Outcome::ControlPresent,
            Some(_) => { out.reasons.push("unsupported_header_value".into()); Outcome::Inconclusive },
        };
    }
    Ok(out)
}

fn compare(baseline: &Observation, observation: &Observation, validation_at: &str) -> Result<Comparison, String> {
    let at = stamp(validation_at)?;
    let after = stamp(&observation.plan_created_at)? > at && observation.approved_at.as_deref().is_some_and(|time| stamp(time).is_ok_and(|time| time > at)) && stamp(&observation.started_at)? > at && observation.finished_at.as_deref().is_some_and(|time| stamp(time).is_ok_and(|time| time > at));
    let addresses = |items: &[String]| items.iter().cloned().collect::<BTreeSet<_>>();
    Ok(Comparison { different_execution: baseline.execution_id != observation.execution_id, different_plan: baseline.plan_id != observation.plan_id, after_validation: after, same_target: baseline.target == observation.target&&baseline.target_ref==observation.target_ref, same_resolved_addresses: addresses(&baseline.resolved_addresses) == addresses(&observation.resolved_addresses), same_peer: baseline.remote_address.is_some() && baseline.remote_address == observation.remote_address })
}
fn result(observation: &Observation, comparison: &Comparison) -> (Outcome, Vec<String>) {
    let mut reasons = observation.reasons.clone();
    for (valid, reason) in [(comparison.different_execution, "same_execution"), (comparison.different_plan, "same_plan"), (comparison.after_validation, "predates_validation"), (comparison.same_target, "changed_target"), (comparison.same_resolved_addresses, "changed_destinations"), (comparison.same_peer, "changed_peer")] { if !valid { reasons.push(reason.into()); } }
    (if comparison.comparable() { observation.outcome.clone() } else { Outcome::Inconclusive }, reasons)
}
fn idle(s: &Snapshot, assessment_id: &str) -> Result<(), String> {
    let a = s.assessments.iter().find(|a| a.id == assessment_id).ok_or_else(failure)?;
    if s.projects.iter().find(|p| p.id == a.project_id).ok_or_else(failure)?.archived { return Err("Projeto arquivado não recebe novas revisões HTTP.".into()); }
    if core::active(&a.status) || s.plans.iter().any(|p| p.assessment_id == a.id && ["approved", "running", "canceling"].contains(&p.status.as_str())) { return Err("Encerre ou reconcilie operações antes da revisão HTTP.".into()); }
    Ok(())
}
fn bind_evidence<'a>(a: &'a Assessment, reference: &Reference) -> Result<&'a Evidence, String> {
    let e = a.evidence.iter().find(|e| e.id == reference.id && e.sha256 == reference.sha256).ok_or_else(failure)?;
    if core::hash(&e.content) != e.sha256 { return Err(failure()); } Ok(e)
}
fn tagged<'a>(a: &'a Assessment, finding_id: &str, schema: &str) -> Result<Vec<&'a Evidence>, String> {
    let mut out = vec![];
    for e in &a.evidence {
        if let Ok(raw) = serde_json::from_str::<Value>(&e.content) {
            if raw["schema"] == schema && raw["findingId"] == finding_id { if e.sha256 != core::hash(&e.content) { return Err(failure()); } out.push(e); }
        }
    }
    Ok(out)
}
fn parse<T: serde::de::DeserializeOwned>(e: &Evidence) -> Result<T, String> { serde_json::from_str(&e.content).map_err(|_| failure()) }
fn typed_evidence<T: Serialize>(name: &str, proof: &T, at: &str) -> Result<Evidence, String> { let content = serde_json::to_string(proof).map_err(|_| failure())?; Ok(Evidence { id: model::id(), name: name.into(), sha256: core::hash(&content), content, created_at: at.into() }) }
fn validation_record(s: &Snapshot, f: &Finding, record_id: &str, method: &str, created: &str, expected: &str, observed: &str, refs: &[Reference]) -> Result<(), String> {
    let record = s.validations.iter().find(|v| v.id == record_id).ok_or_else(failure)?;
    let matches = record.evidence.len() == refs.len() && refs.iter().all(|reference| record.evidence.iter().any(|value| value.id == reference.id && value.sha256 == reference.sha256));
    if record.finding_id != f.id || record.assessment_id != f.assessment_id || record.method != method || record.author != "local-user" || record.created_at != created || record.expected != expected || record.observed != observed || !matches { return Err(failure()); }
    Ok(())
}
fn expected_validation() -> String { "Registrar somente a ausência de X-Content-Type-Options na resposta HEAD da baseline.".into() }
fn expected_confirmation() -> String { "Registrar somente a ocorrência única nosniff na nova resposta HEAD comparável.".into() }
fn observed_review(rationale: &str) -> String { format!("{rationale}\n\n{LIMITS}") }
fn description(baseline: &Observation) -> String { format!("A resposta HEAD observada em {} para {} não incluiu X-Content-Type-Options. O status registrado foi {}. Este candidato descreve o header ausente nessa resposta; não comprova exploração ou impacto. {LIMITS}", baseline.finished_at.as_deref().unwrap_or("instante indisponível"), baseline.target, baseline.status_code.unwrap_or(0)) }

struct State<'a> {
    finding: &'a Finding, assessment: &'a Assessment, claim_evidence: &'a Evidence, claim: Claim,
    validation: Option<(&'a Evidence, Validation)>, retests: Vec<(&'a Evidence, Retest)>, confirmation: Option<(&'a Evidence, Confirmation)>,
}
fn state<'a>(s: &'a Snapshot, finding_id: &str) -> Result<Option<State<'a>>, String> {
    core::valid_id(finding_id)?;
    let f = s.findings.iter().find(|f| f.id == finding_id).ok_or_else(failure)?;
    let a = s.assessments.iter().find(|a| a.id == f.assessment_id).ok_or_else(failure)?;
    let claims = tagged(a, finding_id, CLAIM)?;
    let validations = tagged(a, finding_id, VALIDATION)?; let retests = tagged(a, finding_id, RETEST)?; let confirmations = tagged(a, finding_id, CONFIRMATION)?;
    if claims.is_empty() { if !validations.is_empty() || !retests.is_empty() || !confirmations.is_empty() { return Err(failure()); } return Ok(None); }
    if claims.len() != 1 || validations.len() > 1 || confirmations.len() > 1 || retests.len() > 100 { return Err(failure()); }
    let claim_evidence = claims[0]; let claim: Claim = parse(claim_evidence)?;
    if claim.schema != CLAIM || claim.rule_id != RULE || claim.finding_id != f.id || claim.assessment_id != a.id || claim.created_at != claim_evidence.created_at || claim.limits != LIMITS || claim.baseline.assessment_id != a.id || claim.baseline.outcome != Outcome::ControlMissing || observe(s, &claim.baseline.execution_id)? != claim.baseline { return Err(failure()); }
    if claim.baseline.finished_at.as_deref().is_none_or(|end| stamp(end).is_err() || stamp(&claim.created_at).is_err() || stamp(end).unwrap() > stamp(&claim.created_at).unwrap()) { return Err(failure()); }
    if f.title != TITLE || f.severity != "low" || f.confidence != CONFIDENCE || f.description != description(&claim.baseline) || f.recommendation != RECOMMENDATION || f.created_at != claim.created_at { return Err(failure()); }
    let baseline_ref = Reference { id: claim.baseline.evidence_id.clone().ok_or_else(failure)?, sha256: claim.baseline.evidence_sha256.clone().ok_or_else(failure)? };
    bind_evidence(a, &baseline_ref)?;
    let claim_ref = Reference::of(claim_evidence);
    let mut required_ids = vec![claim_ref.id.clone(), baseline_ref.id.clone()];
    let validation = validations.first().map(|e| -> Result<_, String> {
        let proof: Validation = parse(e)?;
        if proof.schema != VALIDATION || proof.finding_id != f.id || proof.assessment_id != a.id || proof.claim != claim_ref || proof.created_at != e.created_at || stamp(&proof.created_at)? < stamp(&claim.created_at)? || proof.author != "local-user" || !proof.acknowledge_limits || !sha(&proof.reviewed_state_digest) || proof.rationale.trim().is_empty() || proof.rationale.len() > 2000 { return Err(failure()); }
        validation_record(s, f, &proof.record_id, "passive_observation", &proof.created_at, &expected_validation(), &observed_review(&proof.rationale), &[baseline_ref.clone(), claim_ref.clone(), Reference::of(e)])?;
        required_ids.push(e.id.clone()); Ok((*e, proof))
    }).transpose()?;
    if validation.is_none() && (!retests.is_empty() || !confirmations.is_empty() || matches!(f.status.as_str(), "validated" | "fixed")) { return Err(failure()); }
    if validation.is_some() && f.status == "candidate" { return Err(failure()); }
    let mut checked_retests = vec![]; let mut executions = BTreeSet::new(); let mut previous_at = None;
    for e in retests {
        let (validation_evidence, validated) = validation.as_ref().ok_or_else(failure)?;
        let proof: Retest = parse(e)?;
        let observation = observe(s, &proof.observation.execution_id)?;
        let comparison = compare(&claim.baseline, &observation, &validated.created_at)?;
        let (outcome, reasons) = result(&observation, &comparison);
        if proof.schema != RETEST || proof.finding_id != f.id || proof.assessment_id != a.id || proof.claim != claim_ref || proof.validation != Reference::of(validation_evidence) || proof.observation != observation || proof.comparison != comparison || proof.outcome != outcome || proof.reasons != reasons || !comparison.different_execution || !comparison.different_plan || !comparison.after_validation || proof.created_at != e.created_at || !sha(&proof.review_digest) || proof.limits != LIMITS || !executions.insert(observation.execution_id.clone()) { return Err(failure()); }
        let at = stamp(&proof.created_at)?;
        if at < stamp(observation.finished_at.as_deref().ok_or_else(failure)?)? || previous_at.is_some_and(|previous| at < previous) { return Err(failure()); }
        previous_at = Some(at); required_ids.push(e.id.clone());
        if let Some(id) = &observation.evidence_id { required_ids.push(id.clone()); }
        checked_retests.push((e, proof));
    }
    let confirmation = confirmations.first().map(|e| -> Result<_, String> {
        let (validation_evidence, _) = validation.as_ref().ok_or_else(failure)?;
        let (retest_evidence, retest) = checked_retests.last().ok_or_else(failure)?;
        let proof: Confirmation = parse(e)?;
        if proof.schema != CONFIRMATION || proof.finding_id != f.id || proof.assessment_id != a.id || proof.claim != claim_ref || proof.validation != Reference::of(validation_evidence) || proof.retest != Reference::of(retest_evidence) || proof.created_at != e.created_at || stamp(&proof.created_at)? < stamp(&retest.created_at)? || proof.author != "local-user" || !proof.acknowledge_limits || !sha(&proof.reviewed_state_digest) || proof.rationale.trim().is_empty() || proof.rationale.len() > 2000 || retest.outcome != Outcome::ControlPresent || !retest.comparison.comparable() { return Err(failure()); }
        validation_record(s, f, &proof.record_id, "human_review", &proof.created_at, &expected_confirmation(), &observed_review(&proof.rationale), &[claim_ref.clone(), Reference::of(validation_evidence), Reference::of(retest_evidence), Reference::of(e)])?;
        required_ids.push(e.id.clone()); Ok((*e, proof))
    }).transpose()?;
    if (f.status == "fixed") != confirmation.is_some() || required_ids.iter().any(|id| !f.evidence_ids.contains(id)) { return Err(failure()); }
    Ok(Some(State { finding: f, assessment: a, claim_evidence, claim, validation, retests: checked_retests, confirmation }))
}

pub fn is_typed(s: &Snapshot, finding_id: &str) -> Result<bool, String> { Ok(state(s, finding_id)?.is_some()) }
/// Exact producer schemas only; user-imported JSON remains literal text. The
/// complete historical chain is validated by state(), including plan/peer/hash.
pub(crate) fn evidence_target_refs(s:&Snapshot,assessment_id:&str,evidence:&Evidence)->Result<Option<Vec<crate::scope_targets::ScopeTargetRef>>,String>{
    let Ok(raw)=serde_json::from_str::<Value>(&evidence.content)else{return Ok(None)};
    let schema=raw["schema"].as_str().unwrap_or("");if ![CLAIM,VALIDATION,RETEST,CONFIRMATION].contains(&schema){return Ok(None);}
    let finding_id=raw["findingId"].as_str().ok_or_else(failure)?;let current=state(s,finding_id)?.ok_or_else(failure)?;
    if current.assessment.id!=assessment_id||raw["assessmentId"]!=assessment_id||evidence.sha256!=core::hash(&evidence.content){return Err(failure());}
    let stored=match schema{CLAIM=>Some(current.claim_evidence),VALIDATION=>current.validation.as_ref().map(|(e,_)|*e),RETEST=>current.retests.iter().find(|(e,_)|e.id==evidence.id).map(|(e,_)|*e),CONFIRMATION=>current.confirmation.as_ref().map(|(e,_)|*e),_=>None}.ok_or_else(failure)?;
    if stored.id!=evidence.id||stored.sha256!=evidence.sha256||stored.content!=evidence.content{return Err(failure());}
    Ok(Some(current.claim.baseline.target_ref.iter().cloned().collect()))
}
pub fn has_valid_confirmation(s: &Snapshot, finding_id: &str) -> Result<bool, String> { Ok(state(s, finding_id)?.is_some_and(|state| state.confirmation.is_some())) }
pub fn validate_state(s: &Snapshot) -> Result<(), String> {
    for a in &s.assessments { for evidence in &a.evidence {
        if let Ok(raw) = serde_json::from_str::<Value>(&evidence.content) {
            if raw["schema"].as_str().is_some_and(|schema| [CLAIM, VALIDATION, RETEST, CONFIRMATION].contains(&schema)) && !s.findings.iter().any(|f| raw["findingId"] == f.id && f.assessment_id == a.id) { return Err(failure()); }
        }
    } }
    let mut claims = BTreeSet::new();
    for f in &s.findings { if let Some(state) = state(s, &f.id)? { if !claims.insert((state.claim.rule_id, state.claim.baseline.execution_id)) { return Err(failure()); } } }
    Ok(())
}

fn report_observation(observation:&Observation)->Value{let mut value=json!({"executionId":observation.execution_id,"planId":observation.plan_id,"planDigest":observation.plan_digest,
    "evidenceId":observation.evidence_id,"evidenceSha256":observation.evidence_sha256,"target":observation.target,"resolvedAddresses":observation.resolved_addresses,
    "remoteAddress":observation.remote_address,"startedAt":observation.started_at,"finishedAt":observation.finished_at,"statusCode":observation.status_code,
    "schema":observation.schema,"outcome":observation.outcome,"reasons":observation.reasons,"headerOccurrences":observation.header_values.len(),
    "headerValues":observation.header_values.iter().map(|value|json!({"bytes":value.bytes,"sha256":value.sha256})).collect::<Vec<_>>(),"headerTextIncluded":false});if let Some(target)=&observation.target_ref{value["targetRef"]=json!(target);}value}
pub(crate) fn report_rows(s:&Snapshot,assessment_id:&str)->Result<Vec<Value>,String>{
    let mut rows=vec![];
    for finding in s.findings.iter().filter(|finding|finding.assessment_id==assessment_id){if let Some(current)=state(s,&finding.id)?{
        rows.push(json!({"findingId":finding.id,"findingStatus":finding.status,"ruleId":RULE,"claim":Reference::of(current.claim_evidence),
            "baseline":report_observation(&current.claim.baseline),"validation":current.validation.as_ref().map(|(e,p)|json!({"evidenceId":e.id,"sha256":e.sha256,"recordId":p.record_id,"createdAt":p.created_at})),
            "retests":current.retests.iter().map(|(e,p)|json!({"evidenceId":e.id,"sha256":e.sha256,"createdAt":p.created_at,"observation":report_observation(&p.observation),"comparison":p.comparison,"outcome":p.outcome,"reasons":p.reasons})).collect::<Vec<_>>(),
            "confirmation":current.confirmation.as_ref().map(|(e,p)|json!({"evidenceId":e.id,"sha256":e.sha256,"recordId":p.record_id,"createdAt":p.created_at,"retestEvidenceId":p.retest.id})),"limits":LIMITS}));
    }}Ok(rows)
}
pub(crate) fn markdown_rows(rows:&[Value],escape:impl Fn(&str)->String)->String{
    if rows.is_empty(){return String::new();}
    let text=|value:&Value|escape(&match value{Value::String(text)=>text.clone(),Value::Number(number)=>number.to_string(),_=>"não informado".into()});
    let mut output=String::from("\n## Validação factual e retestes HTTP\n\nFatos de respostas HEAD, com revisão humana. Não representam reprodução de exploração nem alteração aplicada ao servidor. Textos dos headers não são incluídos neste resumo.\n\n");
    for row in rows{
        let baseline=&row["baseline"];
        output.push_str(&format!("- Achado: {} · Estado: {} · Regra: {}\n  - Baseline: execução {} · evidência {} · SHA-256 {}\n  - URL: {} · peer: {} · HTTP {} · observada em {}\n  - Resultado factual: {}\n",text(&row["findingId"]),text(&row["findingStatus"]),text(&row["ruleId"]),text(&baseline["executionId"]),text(&baseline["evidenceId"]),text(&baseline["evidenceSha256"]),text(&baseline["target"]),text(&baseline["remoteAddress"]),text(&baseline["statusCode"]),text(&baseline["finishedAt"]),text(&baseline["outcome"])));
        if let Some(retests)=row["retests"].as_array(){for retest in retests{output.push_str(&format!("  - Reteste: {} · SHA-256 {} · resultado {}\n    - Nova execução: {} · peer: {} · observada em {}\n",text(&retest["evidenceId"]),text(&retest["sha256"]),text(&retest["outcome"]),text(&retest["observation"]["executionId"]),text(&retest["observation"]["remoteAddress"]),text(&retest["observation"]["finishedAt"])));}}
        if !row["confirmation"].is_null(){output.push_str(&format!("  - Confirmação humana: {} · reteste confirmado: {} · em {}\n",text(&row["confirmation"]["recordId"]),text(&row["confirmation"]["retestEvidenceId"]),text(&row["confirmation"]["createdAt"])));}
        output.push_str(&format!("  - Limites: {}\n\n",text(&row["limits"])));
    }output
}

fn state_digest(s: &Snapshot, state: &State<'_>) -> Result<String, String> {
    let plans = s.plans.iter().filter(|p| p.assessment_id == state.assessment.id && p.adapter == ADAPTER&&p.target_ref==state.claim.baseline.target_ref).collect::<Vec<_>>();
    let plan_ids = plans.iter().map(|p| p.id.as_str()).collect::<BTreeSet<_>>();
    digest(&json!({"ruleId":RULE,"finding":state.finding,"assessmentStatus":state.assessment.status,
        "projectArchived":s.projects.iter().find(|p|p.id==state.assessment.project_id).ok_or_else(failure)?.archived,
        "claim":Reference::of(state.claim_evidence),"validation":state.validation.as_ref().map(|(e,_)|Reference::of(e)),
        "retests":state.retests.iter().map(|(e,_)|Reference::of(e)).collect::<Vec<_>>(),
        "confirmation":state.confirmation.as_ref().map(|(e,_)|Reference::of(e)),
        "reviews":s.validations.iter().filter(|v|v.finding_id==state.finding.id).collect::<Vec<_>>(),
        "plans":plans,"executions":s.executions.iter().filter(|e|plan_ids.contains(e.plan_id.as_str())).collect::<Vec<_>>()}))
}
fn latest_execution<'a>(s: &'a Snapshot, assessment_id: &str,target_ref:Option<&crate::scope_targets::ScopeTargetRef>) -> Result<Option<&'a Execution>, String> {
    let mut latest: Option<&Execution> = None;
    // Executor appends dispatches to this canonical vector; persistence and
    // project purge preserve its order. Wall clocks can move backwards.
    for e in s.executions.iter().filter(|e| e.assessment_id == assessment_id && s.plans.iter().any(|p| p.id == e.plan_id && p.adapter == ADAPTER&&p.target_ref.as_ref()==target_ref)) {
        stamp(&e.started_at)?;
        latest = Some(e);
    }
    Ok(latest)
}
fn reviewable(s: &Snapshot, current: &State<'_>, observation: &Observation) -> Result<(Comparison, Vec<String>), String> {
    let validation_at = current.validation.as_ref().map(|(_,proof)|proof.created_at.as_str()).unwrap_or(current.claim.created_at.as_str());
    if observation.assessment_id != current.assessment.id||observation.target_ref!=current.claim.baseline.target_ref { return Err(failure()); }
    let comparison = compare(&current.claim.baseline, observation, validation_at)?;
    let mut reasons = vec![];
    if current.finding.status != "validated" || current.validation.is_none() { reasons.push("wrong_finding_state".into()); }
    if !comparison.different_execution { reasons.push("same_execution".into()); }
    if !comparison.different_plan { reasons.push("same_plan".into()); }
    if !comparison.after_validation { reasons.push("predates_validation".into()); }
    if observation.finished_at.is_none() || !["completed","failed","canceled"].contains(&observation.execution_status.as_str()) { reasons.push("execution_incomplete".into()); }
    if s.projects.iter().find(|p|p.id==current.assessment.project_id).ok_or_else(failure)?.archived { reasons.push("archived_project".into()); }
    if idle(s,&current.assessment.id).is_err() && !reasons.iter().any(|v|v=="archived_project") { reasons.push("active_assessment".into()); }
    Ok((comparison,reasons))
}
fn retest_public(evidence: &Evidence, proof: &Retest) -> Value { json!({"evidenceId":evidence.id,"sha256":evidence.sha256,"createdAt":proof.created_at,
    "executionId":proof.observation.execution_id,"observation":proof.observation,"comparison":proof.comparison,"outcome":proof.outcome,"reasons":proof.reasons}) }
fn status(s: &Snapshot, finding_id: &str) -> Result<Value, String> {
    let Some(current)=state(s,finding_id)? else{return Ok(json!({"supported":false,"findingId":finding_id}));};
    let mut reasons=vec![];
    if s.projects.iter().find(|p|p.id==current.assessment.project_id).ok_or_else(failure)?.archived {reasons.push("archived_project");}
    else if idle(s,&current.assessment.id).is_err(){reasons.push("active_assessment");}
    let stable=reasons.is_empty();
    let can_validate=stable&&current.finding.status=="candidate"&&current.validation.is_none();
    let can_retest=stable&&current.finding.status=="validated"&&current.validation.is_some();
    let latest=current.retests.last();
    let mut can_confirm=can_retest&&latest.is_some_and(|(_,proof)|proof.outcome==Outcome::ControlPresent&&proof.comparison.comparable());
    if can_confirm&&latest_execution(s,&current.assessment.id,current.claim.baseline.target_ref.as_ref())?.map(|e|e.id.as_str())!=latest.map(|(_,proof)|proof.observation.execution_id.as_str()) {can_confirm=false;reasons.push("newer_unreviewed_execution");}
    let mut available=vec![];
    if current.validation.is_some(){for execution in s.executions.iter().filter(|e|e.assessment_id==current.assessment.id&&s.plans.iter().any(|p|p.id==e.plan_id&&p.adapter==ADAPTER&&p.target_ref==current.claim.baseline.target_ref)).rev(){
        let observation=observe(s,&execution.id)?;let(_,reasons)=reviewable(s,&current,&observation)?;
        available.push(json!({"executionId":execution.id,"status":execution.status,"startedAt":execution.started_at,"finishedAt":execution.finished_at,"canReview":reasons.is_empty(),"reasons":reasons}));
    }}
    Ok(json!({"supported":true,"findingId":finding_id,"ruleId":RULE,"findingStatus":current.finding.status,"stateDigest":state_digest(s,&current)?,
        "claim":{"evidenceId":current.claim_evidence.id,"sha256":current.claim_evidence.sha256,"createdAt":current.claim.created_at},"baseline":current.claim.baseline,
        "validation":current.validation.as_ref().map(|(e,p)|json!({"evidenceId":e.id,"sha256":e.sha256,"recordId":p.record_id,"createdAt":p.created_at,"rationale":p.rationale})),
        "retests":current.retests.iter().map(|(e,p)|retest_public(e,p)).collect::<Vec<_>>(),
        "confirmation":current.confirmation.as_ref().map(|(e,p)|json!({"evidenceId":e.id,"sha256":e.sha256,"recordId":p.record_id,"createdAt":p.created_at,"retestEvidenceId":p.retest.id,"rationale":p.rationale})),
        "actions":{"canValidate":can_validate,"canRetest":can_retest,"canConfirm":can_confirm,"confirmRetestEvidenceId":if can_confirm{latest.map(|(e,_)|e.id.as_str())}else{None},"reasons":reasons},
        "availableExecutions":available,"limits":LIMITS}))
}
fn candidate_preview(s:&Snapshot,execution_id:&str,rule_id:&str)->Result<Value,String>{
    check_rule(rule_id)?;let observation=observe(s,execution_id)?;
    let a=s.assessments.iter().find(|a|a.id==observation.assessment_id).ok_or_else(failure)?;
    let archived=s.projects.iter().find(|p|p.id==a.project_id).ok_or_else(failure)?.archived;
    let mut reasons=observation.reasons.clone();if archived{reasons.push("archived_project".into());}else if idle(s,&a.id).is_err(){reasons.push("active_assessment".into());}
    let mut existing=None;
    for f in s.findings.iter().filter(|f|f.assessment_id==a.id){if let Some(current)=state(s,&f.id)?{if current.claim.baseline.execution_id==execution_id{existing=Some(f.id.clone());}}}
    let review=digest(&json!({"operation":"httpCandidate","ruleId":RULE,"observation":observation,"assessmentStatus":a.status,"projectArchived":archived}))?;
    Ok(json!({"ruleId":RULE,"observation":observation,"reviewDigest":review,"canCreateCandidate":observation.outcome==Outcome::ControlMissing&&reasons.is_empty()&&existing.is_none(),"existingFindingId":existing,"reasons":reasons,"limits":LIMITS}))
}
fn retest_preview(s:&Snapshot,finding_id:&str,execution_id:&str)->Result<Value,String>{
    let current=state(s,finding_id)?.ok_or("Este achado não tem uma alegação HTTP emitida pelo host.")?;
    let observation=observe(s,execution_id)?;let(comparison,reasons)=reviewable(s,&current,&observation)?;
    let(outcome,fact_reasons)=result(&observation,&comparison);
    let existing=current.retests.iter().find(|(_,proof)|proof.observation.execution_id==execution_id).map(|(e,_)|e.id.clone());
    let review=digest(&json!({"operation":"httpRetest","stateDigest":state_digest(s,&current)?,"observation":observation,"comparison":comparison,"outcome":outcome,"reasons":fact_reasons}))?;
    Ok(json!({"findingId":finding_id,"observation":observation,"comparison":comparison,"outcome":outcome,"reviewDigest":review,"canRecord":reasons.is_empty()&&existing.is_none(),"reasons":reasons,"factReasons":fact_reasons,"existingRetestEvidenceId":existing,"limits":LIMITS}))
}
fn append(s:&mut Snapshot,finding_id:&str,evidence:Evidence,extra_ids:&[String],message:&str)->Result<(),String>{
    let finding=s.findings.iter_mut().find(|f|f.id==finding_id).ok_or_else(failure)?;
    for id in extra_ids.iter().chain(std::iter::once(&evidence.id)){if !finding.evidence_ids.contains(id){finding.evidence_ids.push(id.clone());}}
    let assessment=s.assessments.iter_mut().find(|a|a.id==finding.assessment_id).ok_or_else(failure)?;
    assessment.updated_at=evidence.created_at.clone();assessment.evidence.push(evidence);assessment.events.push(model::event(message,"info"));Ok(())
}
fn record(finding_id:&str,assessment_id:&str,id:&str,method:&str,at:&str,expected:String,observed:String,refs:&[Reference])->ValidationRecord{
    ValidationRecord{id:id.into(),finding_id:finding_id.into(),assessment_id:assessment_id.into(),method:method.into(),expected,observed,evidence:refs.iter().map(|r|ValidatedEvidence{id:r.id.clone(),sha256:r.sha256.clone()}).collect(),author:"local-user".into(),created_at:at.into()}
}

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct PreviewInput{execution_id:String,rule_id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct CandidateInput{execution_id:String,rule_id:String,review_digest:String,acknowledge_limits:bool}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct StatusInput{finding_id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct ValidateInput{finding_id:String,state_digest:String,rationale:String,acknowledge_limits:bool}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct RetestPreviewInput{finding_id:String,execution_id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct RetestInput{finding_id:String,execution_id:String,review_digest:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct ConfirmInput{finding_id:String,retest_evidence_id:String,state_digest:String,rationale:String,acknowledge_limits:bool}
pub fn supports(action:&str)->bool{matches!(action,"finding.httpPreview"|"finding.httpCandidate"|"finding.httpStatus"|"finding.httpValidate"|"finding.httpRetestPreview"|"finding.httpRetest"|"finding.httpConfirm")}
pub fn mutates(action:&str)->bool{matches!(action,"finding.httpCandidate"|"finding.httpValidate"|"finding.httpRetest"|"finding.httpConfirm")}
pub fn handle(action:&str,payload:Value,store:&Store,ai:&crate::ai::AiService)->Result<Value,String>{
    if !supports(action)||!payload.is_object()||payload.to_string().len()>16384{return Err("Solicitação de revisão HTTP inválida ou grande demais.".into());}
    match action{
        "finding.httpPreview"=>{let input:PreviewInput=decode(payload)?;candidate_preview(&store.snapshot()?,&input.execution_id,&input.rule_id)},
        "finding.httpStatus"=>{let input:StatusInput=decode(payload)?;status(&store.snapshot()?,&input.finding_id)},
        "finding.httpRetestPreview"=>{let input:RetestPreviewInput=decode(payload)?;retest_preview(&store.snapshot()?,&input.finding_id,&input.execution_id)},
        _=>ai.with_local_redactor(|redact|mutate(action,payload,store,redact)),
    }
}
fn mutate(action:&str,payload:Value,store:&Store,redact:&dyn Fn(&str)->String)->Result<Value,String>{match action{
    "finding.httpCandidate"=>{
        let input:CandidateInput=decode(payload)?;if !input.acknowledge_limits{return Err("Reconheça os limites da observação antes de criar o candidato.".into());}
        store.mutate(|s|{
            let preview=candidate_preview(s,&input.execution_id,&input.rule_id)?;require_digest(&input.review_digest,preview["reviewDigest"].as_str().ok_or_else(failure)?)?;
            if let Some(existing)=preview["existingFindingId"].as_str(){let finding=s.findings.iter().find(|f|f.id==existing).ok_or_else(failure)?;return Ok(json!({"created":false,"finding":finding,"status":status(s,existing)?}));}
            if preview["canCreateCandidate"]!=true{return Err("Esta observação não sustenta a criação de candidato HTTP.".into());}
            let observation:Observation=decode(preview["observation"].clone())?;idle(s,&observation.assessment_id)?;crate::scope::freeze_review(s,&observation.assessment_id,"finding.httpCandidate")?;
            let id=model::id();let at=model::now();let proof=Claim{schema:CLAIM.into(),rule_id:RULE.into(),finding_id:id.clone(),assessment_id:observation.assessment_id.clone(),baseline:observation.clone(),created_at:at.clone(),limits:LIMITS.into()};
            let evidence=typed_evidence(&format!("http-claim-{id}.json"),&proof,&at)?;
            let finding=Finding{id:id.clone(),assessment_id:observation.assessment_id.clone(),title:TITLE.into(),severity:"low".into(),confidence:CONFIDENCE.into(),status:"candidate".into(),description:description(&observation),recommendation:RECOMMENDATION.into(),evidence_ids:vec![observation.evidence_id.ok_or_else(failure)?],created_at:at};
            s.findings.push(finding);append(s,&id,evidence,&[],"Candidato HTTP factual criado a partir de execução HEAD aprovada. A ausência de header não comprova exploração.")?;
            Ok(json!({"created":true,"finding":s.findings.iter().find(|f|f.id==id).ok_or_else(failure)?,"status":status(s,&id)?}))
        })
    },
    "finding.httpValidate"=>{
        let input:ValidateInput=decode(payload)?;let rationale=note(&input.rationale,redact)?;if !input.acknowledge_limits{return Err("Reconheça os limites da validação factual.".into());}
        store.mutate(|s|{
            let public=status(s,&input.finding_id)?;require_digest(&input.state_digest,public["stateDigest"].as_str().ok_or_else(failure)?)?;
            if public["actions"]["canValidate"]!=true{return Err("Somente candidato HTTP ainda não validado pode receber esta revisão.".into());}
            let current=state(s,&input.finding_id)?.ok_or_else(failure)?;idle(s,&current.assessment.id)?;
            let at=model::now();let record_id=model::id();let claim=Reference::of(current.claim_evidence);let baseline=Reference{id:current.claim.baseline.evidence_id.clone().ok_or_else(failure)?,sha256:current.claim.baseline.evidence_sha256.clone().ok_or_else(failure)?};let aid=current.assessment.id.clone();
            let proof=Validation{schema:VALIDATION.into(),finding_id:input.finding_id.clone(),assessment_id:aid.clone(),claim:claim.clone(),record_id:record_id.clone(),rationale:rationale.clone(),reviewed_state_digest:input.state_digest,created_at:at.clone(),author:"local-user".into(),acknowledge_limits:true};
            let evidence=typed_evidence(&format!("http-validation-{}.json",input.finding_id),&proof,&at)?;
            let review=record(&input.finding_id,&aid,&record_id,"passive_observation",&at,expected_validation(),observed_review(&rationale),&[baseline,claim,Reference::of(&evidence)]);
            append(s,&input.finding_id,evidence,&[],"Revisão humana validou somente o fato HTTP da baseline, com origem e limites preservados.")?;s.validations.push(review);s.findings.iter_mut().find(|f|f.id==input.finding_id).ok_or_else(failure)?.status="validated".into();status(s,&input.finding_id)
        })
    },
    "finding.httpRetest"=>{
        let input:RetestInput=decode(payload)?;
        store.mutate(|s|{
            let current=state(s,&input.finding_id)?.ok_or_else(failure)?;
            if let Some((e,p))=current.retests.iter().find(|(_,p)|p.observation.execution_id==input.execution_id){if p.review_digest!=input.review_digest{return Err("Esta execução já tem um reteste; a revisão não pode ser substituída.".into());}return Ok(json!({"recorded":false,"retest":retest_public(e,p),"status":status(s,&input.finding_id)?}));}
            let preview=retest_preview(s,&input.finding_id,&input.execution_id)?;require_digest(&input.review_digest,preview["reviewDigest"].as_str().ok_or_else(failure)?)?;if preview["canRecord"]!=true{return Err("Reteste exige nova execução encerrada e aprovada depois da validação, no mesmo achado ainda validado.".into());}
            let current=state(s,&input.finding_id)?.ok_or_else(failure)?;idle(s,&current.assessment.id)?;
            let observation:Observation=decode(preview["observation"].clone())?;let comparison:Comparison=decode(preview["comparison"].clone())?;let(outcome,reasons)=result(&observation,&comparison);let at=model::now();
            let proof=Retest{schema:RETEST.into(),finding_id:input.finding_id.clone(),assessment_id:current.assessment.id.clone(),claim:Reference::of(current.claim_evidence),validation:Reference::of(current.validation.as_ref().ok_or_else(failure)?.0),observation:observation.clone(),comparison,outcome,reasons,review_digest:input.review_digest,created_at:at.clone(),limits:LIMITS.into()};
            let evidence=typed_evidence(&format!("http-retest-{}.json",input.execution_id),&proof,&at)?;let public=retest_public(&evidence,&proof);let extras=observation.evidence_id.into_iter().collect::<Vec<_>>();
            append(s,&input.finding_id,evidence,&extras,"Reteste HTTP registrado sem confirmar correção automaticamente. A conclusão vale somente para a resposta observada.")?;
            Ok(json!({"recorded":true,"retest":public,"status":status(s,&input.finding_id)?}))
        })
    },
    "finding.httpConfirm"=>{
        let input:ConfirmInput=decode(payload)?;let rationale=note(&input.rationale,redact)?;if !input.acknowledge_limits{return Err("Reconheça os limites da confirmação do reteste HTTP.".into());}
        store.mutate(|s|{
            let public=status(s,&input.finding_id)?;require_digest(&input.state_digest,public["stateDigest"].as_str().ok_or_else(failure)?)?;
            if public["actions"]["canConfirm"]!=true||public["actions"]["confirmRetestEvidenceId"]!=input.retest_evidence_id{return Err("Confirmação exige o último reteste positivo, comparável e sem execução posterior não revisada.".into());}
            let current=state(s,&input.finding_id)?.ok_or_else(failure)?;idle(s,&current.assessment.id)?;
            let aid=current.assessment.id.clone();let claim=Reference::of(current.claim_evidence);let validation=Reference::of(current.validation.as_ref().ok_or_else(failure)?.0);let retest=Reference::of(current.retests.last().ok_or_else(failure)?.0);let at=model::now();let record_id=model::id();
            let proof=Confirmation{schema:CONFIRMATION.into(),finding_id:input.finding_id.clone(),assessment_id:aid.clone(),claim:claim.clone(),validation:validation.clone(),retest:retest.clone(),record_id:record_id.clone(),rationale:rationale.clone(),reviewed_state_digest:input.state_digest,created_at:at.clone(),author:"local-user".into(),acknowledge_limits:true};
            let evidence=typed_evidence(&format!("http-confirmation-{}.json",input.finding_id),&proof,&at)?;
            let review=record(&input.finding_id,&aid,&record_id,"human_review",&at,expected_confirmation(),observed_review(&rationale),&[claim,validation,retest,Reference::of(&evidence)]);
            append(s,&input.finding_id,evidence,&[],"Confirmação humana registrada para o controle observado no reteste HEAD. Nenhuma alteração remota foi aplicada por esta ação.")?;s.validations.push(review);s.findings.iter_mut().find(|f|f.id==input.finding_id).ok_or_else(failure)?.status="fixed".into();status(s,&input.finding_id)
        })
    },_=>Err("Operação HTTP desconhecida.".into())}}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiService;
    fn combined_observation(f:&crate::executor::combined_fixture::Fixture,index:usize,nosniff:bool)->String{
        let raw_plan=f.plan(index,ADAPTER);f.executor.handle("plan.approve",json!({"id":raw_plan["id"],"digest":raw_plan["digest"]}),&f.store).unwrap();let execution_id=model::id();
        f.store.mutate(|s|{let p=s.plans.iter().find(|p|p.id==raw_plan["id"]).unwrap().clone();let receipt=crate::scope::admission_receipt_selected(s,&f.aid,ADAPTER,p.timeout_seconds as u64*1000,&[f.refs[index].clone()])?;let at=model::now();let mut values=BTreeMap::<String,Vec<String>>::new();for name in HEADER_NAMES{values.insert(name.into(),vec![]);}let mut headers=BTreeMap::<String,String>::new();if nosniff{values.insert("x-content-type-options".into(),vec![STANDARD.encode(b"nosniff")]);headers.insert("x-content-type-options".into(),"nosniff".into());}
            let content=json!({"schema":OBSERVATION_SCHEMA,"adapter":ADAPTER,"planDigest":p.digest,"target":p.target,"status":200,"headers":headers,"headerValues":values,"resolvedAddresses":p.resolved_addresses,"remoteAddress":p.resolved_addresses[0],"method":"HEAD","captureComplete":true,"redirectFollowed":false,"limits":"Synthetic recorded observation"}).to_string();let evidence=Evidence{id:model::id(),name:"Synthetic selected HTTP observation".into(),sha256:core::hash(&content),content,created_at:at.clone()};s.executions.push(Execution{id:execution_id.clone(),plan_id:p.id.clone(),assessment_id:f.aid.clone(),status:"completed".into(),started_at:at.clone(),finished_at:Some(at),evidence_id:Some(evidence.id.clone()),message:"Synthetic recording without socket".into(),scope_admission:Some(receipt)});let a=s.assessments.iter_mut().find(|a|a.id==f.aid).unwrap();a.status="partial".into();a.evidence.push(evidence);s.plans.iter_mut().find(|plan|plan.id==p.id).unwrap().status="completed".into();Ok(())}).unwrap();execution_id
    }
    #[test]
    fn combined_http_latest_and_retests_are_per_entry_even_with_clock_rollback(){
        let f=crate::executor::combined_fixture::Fixture::new(&[("web","http://127.0.0.1:43123/first",&[ADAPTER]),("web","http://127.0.0.1:43123/second",&[ADAPTER])],&[],json!([]));let ai=AiService::new(f.store.root.clone()).unwrap();let call=|action:&str,input:Value|handle(action,input,&f.store,&ai);
        let baseline=combined_observation(&f,1,false);let preview=call("finding.httpPreview",json!({"executionId":baseline,"ruleId":RULE})).unwrap();let candidate=call("finding.httpCandidate",json!({"executionId":baseline,"ruleId":RULE,"reviewDigest":preview["reviewDigest"],"acknowledgeLimits":true})).unwrap();let fid=candidate["finding"]["id"].as_str().unwrap();let before=call("finding.httpStatus",json!({"findingId":fid})).unwrap();assert_eq!(before["baseline"]["targetRef"],json!(f.refs[1]));
        call("finding.httpValidate",json!({"findingId":fid,"stateDigest":before["stateDigest"],"rationale":"The second URL observation lacks the header","acknowledgeLimits":true})).unwrap();let good=combined_observation(&f,1,true);let review=call("finding.httpRetestPreview",json!({"findingId":fid,"executionId":good})).unwrap();call("finding.httpRetest",json!({"findingId":fid,"executionId":good,"reviewDigest":review["reviewDigest"]})).unwrap();let ready=call("finding.httpStatus",json!({"findingId":fid})).unwrap();assert_eq!(ready["actions"]["canConfirm"],true);
        let unrelated=combined_observation(&f,0,false);let unchanged=call("finding.httpStatus",json!({"findingId":fid})).unwrap();assert_eq!(unchanged["actions"]["canConfirm"],true);assert_eq!(unchanged["stateDigest"],ready["stateDigest"]);assert!(unchanged["availableExecutions"].as_array().unwrap().iter().all(|row|row["executionId"]!=unrelated));assert!(call("finding.httpRetestPreview",json!({"findingId":fid,"executionId":unrelated})).is_err());
        let newer=combined_observation(&f,1,false);f.store.mutate(|s|{let e=s.executions.iter_mut().find(|e|e.id==newer).unwrap();e.started_at="2000-01-01T00:00:01Z".into();e.finished_at=Some("2000-01-01T00:00:02Z".into());let plan_id=e.plan_id.clone();let evidence_id=e.evidence_id.clone().unwrap();let p=s.plans.iter_mut().find(|p|p.id==plan_id).unwrap();p.created_at="2000-01-01T00:00:00Z".into();p.approved_at=Some(p.created_at.clone());p.digest=executor::plan_digest(p);let raw=s.assessments[0].evidence.iter_mut().find(|e|e.id==evidence_id).unwrap();let mut capture:Value=serde_json::from_str(&raw.content).unwrap();capture["planDigest"]=json!(p.digest);raw.content=capture.to_string();raw.sha256=core::hash(&raw.content);raw.created_at="2000-01-01T00:00:02Z".into();Ok(())}).unwrap();assert_eq!(call("finding.httpStatus",json!({"findingId":fid})).unwrap()["actions"]["canConfirm"],false);assert_eq!(latest_execution(&f.store.snapshot().unwrap(),&f.aid,Some(&f.refs[1])).unwrap().unwrap().id,newer);
        assert_eq!(report_rows(&f.store.snapshot().unwrap(),&f.aid).unwrap()[0]["baseline"]["targetRef"],json!(f.refs[1]));
    }
    #[test]
    fn combined_http_four_proofs_keep_origins_for_context_and_report(){
        let methods=[ADAPTER,"ai.remote-review.v1"];let f=crate::executor::combined_fixture::Fixture::new(&[("web","http://127.0.0.1:43123/first",&methods),("web","http://127.0.0.1:43123/second",&methods)],&[],json!([]));let ai=AiService::new(f.store.root.clone()).unwrap();let call=|action:&str,input:Value|handle(action,input,&f.store,&ai);
        let baseline=combined_observation(&f,1,false);let preview=call("finding.httpPreview",json!({"executionId":baseline,"ruleId":RULE})).unwrap();let created=call("finding.httpCandidate",json!({"executionId":baseline,"ruleId":RULE,"reviewDigest":preview["reviewDigest"],"acknowledgeLimits":true})).unwrap();let fid=created["finding"]["id"].as_str().unwrap();let current=call("finding.httpStatus",json!({"findingId":fid})).unwrap();call("finding.httpValidate",json!({"findingId":fid,"stateDigest":current["stateDigest"],"rationale":"Only this selected response lacked the header","acknowledgeLimits":true})).unwrap();let next=combined_observation(&f,1,true);let preview=call("finding.httpRetestPreview",json!({"findingId":fid,"executionId":next})).unwrap();call("finding.httpRetest",json!({"findingId":fid,"executionId":next,"reviewDigest":preview["reviewDigest"]})).unwrap();let current=call("finding.httpStatus",json!({"findingId":fid})).unwrap();call("finding.httpConfirm",json!({"findingId":fid,"stateDigest":current["stateDigest"],"retestEvidenceId":current["actions"]["confirmRetestEvidenceId"],"rationale":"Only the second URL returned the expected observed control","acknowledgeLimits":true})).unwrap();
        let snapshot=f.store.snapshot().unwrap();let typed=snapshot.assessments[0].evidence.iter().filter(|e|serde_json::from_str::<Value>(&e.content).ok().is_some_and(|raw|raw["schema"].as_str().is_some_and(|schema|[CLAIM,VALIDATION,RETEST,CONFIRMATION].contains(&schema)))).collect::<Vec<_>>();assert_eq!(typed.len(),4);for evidence in &typed{assert_eq!(evidence_target_refs(&snapshot,&f.aid,evidence).unwrap(),Some(vec![f.refs[1].clone()]));assert!(evidence_target_refs(&snapshot,&model::id(),evidence).is_err());}
        let evidence_ids=typed.iter().map(|e|e.id.clone()).collect::<Vec<_>>();assert_eq!(crate::evidence_lineage::evidence_refs(&f.store,&snapshot,&f.aid,&evidence_ids).unwrap(),vec![f.refs[1].clone()]);assert_eq!(crate::evidence_lineage::finding_target_refs(&f.store,&snapshot,fid).unwrap(),vec![f.refs[1].clone()]);let context=ai.test_context_projection(&f.store,json!({"assessmentId":f.aid,"evidenceIds":evidence_ids,"contextTargetRefs":[f.refs[1]]})).unwrap();assert_eq!(context["context"]["targets"].as_array().unwrap().len(),1);assert!(ai.test_context_projection(&f.store,json!({"assessmentId":f.aid,"evidenceIds":evidence_ids,"contextTargetRefs":[f.refs[0]]})).is_err());
        let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();let report=crate::recovery::test_support::report(&recovery,&f.store,&f.aid,true).unwrap();let manifest:Value=serde_json::from_slice(&std::fs::read(report["manifestPath"].as_str().unwrap()).unwrap()).unwrap();assert_eq!(manifest["findings"][0]["targetRefs"],json!([f.refs[1]]));for evidence in manifest["evidence"].as_array().unwrap(){assert_eq!(evidence["targetRefs"],json!([f.refs[1]]));}
    }
    struct Fixture { _dir:tempfile::TempDir, store:Store, ai:AiService, assessment:String, project:String }
    impl Fixture {
        fn new()->Self{
            let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().join("data")).unwrap();let ai=AiService::new(store.root.clone()).unwrap();
            let p=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Synthetic HTTP factual fixture","kind":"web","target":"http://127.0.0.1:43123/"})}).unwrap();
            let a=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"title":"Synthetic HTTP evidence","kind":"web","target":p["target"],"objective":"Only observe recorded fixture facts","scope":crate::scope::test_creation(&store,p["id"].as_str().unwrap(),&["http.headers.v1"])})}).unwrap();
            Self{_dir:dir,store,ai,assessment:a["id"].as_str().unwrap().into(),project:p["id"].as_str().unwrap().into()}
        }
        fn call(&self,action:&str,payload:Value)->Result<Value,String>{handle(action,payload,&self.store,&self.ai)}
        fn observation(&self,values:&[&[u8]])->String{self.execution(values,200,Some("127.0.0.1:43123"),"completed",true)}
        fn execution(&self,values:&[&[u8]],status:u16,peer:Option<&str>,execution_status:&str,v2:bool)->String{
            let at=model::now();let pid=model::id();let eid=model::id();let raw_id=model::id();
            self.store.mutate(|s|{
                let a=s.assessments.iter_mut().find(|a|a.id==self.assessment).unwrap();
                let mut plan=ExecutionPlan{target_ref:None,requested_ports:None,identity_preflight_refs:vec![],scope_omissions:vec![],scope_ref:None,id:pid.clone(),assessment_id:a.id.clone(),adapter:ADAPTER.into(),target:a.target.clone(),timeout_seconds:10,output_limit_ki_b:64,input_files:vec![],resolved_addresses:vec!["127.0.0.1:43123".into()],source_scope:None,digest:String::new(),created_at:at.clone(),expires_at:(chrono::Utc::now()+chrono::Duration::minutes(10)).to_rfc3339(),approved_at:Some(at.clone()),status:execution_status.into()};plan.digest=executor::plan_digest(&plan);
                let mut headers=BTreeMap::<String,String>::new();let mut all=BTreeMap::<String,Vec<String>>::new();for name in HEADER_NAMES{all.insert(name.into(),vec![]);}
                if let Some(first)=values.first(){let text=std::str::from_utf8(first).ok().filter(|value|value.bytes().all(|b|b==b'\t'||(32..127).contains(&b))).unwrap_or("[valor não textual]");headers.insert("x-content-type-options".into(),text.into());}
                all.insert("x-content-type-options".into(),values.iter().map(|bytes|STANDARD.encode(bytes)).collect());
                let mut raw=json!({"adapter":ADAPTER,"planDigest":plan.digest,"target":a.target,"status":status,"headers":headers,"resolvedAddresses":plan.resolved_addresses,"redirectFollowed":false,"limits":"Synthetic recorded HEAD only."});
                if v2{raw["schema"]=json!(OBSERVATION_SCHEMA);raw["method"]=json!("HEAD");raw["captureComplete"]=json!(true);raw["remoteAddress"]=json!(peer);raw["headerValues"]=json!(all);}
                let evidence_id=if execution_status=="completed"{let content=raw.to_string();a.evidence.push(Evidence{id:raw_id.clone(),name:"synthetic-head.json".into(),sha256:core::hash(&content),content,created_at:at.clone()});Some(raw_id.clone())}else{None};
                let execution=Execution{scope_admission:None,id:eid.clone(),plan_id:pid.clone(),assessment_id:a.id.clone(),status:execution_status.into(),started_at:at.clone(),finished_at:if matches!(execution_status,"running"|"unknown"){None}else{Some(at.clone())},evidence_id,message:"Synthetic fixture; no network was sent.".into()};
                a.status=if execution_status=="completed"{"partial"}else{execution_status}.into();a.updated_at=at.clone();s.plans.push(plan);s.executions.push(execution);Ok(())
            }).unwrap();eid
        }
        fn candidate(&self,execution:&str)->Value{let preview=self.call("finding.httpPreview",json!({"executionId":execution,"ruleId":RULE})).unwrap();self.call("finding.httpCandidate",json!({"executionId":execution,"ruleId":RULE,"reviewDigest":preview["reviewDigest"],"acknowledgeLimits":true})).unwrap()}
        fn status(&self,id:&str)->Value{self.call("finding.httpStatus",json!({"findingId":id})).unwrap()}
        fn validate(&self,id:&str)->Value{let s=self.status(id);self.call("finding.httpValidate",json!({"findingId":id,"stateDigest":s["stateDigest"],"rationale":"Only this recorded HEAD response lacked the selected header.","acknowledgeLimits":true})).unwrap()}
        fn retest(&self,id:&str,execution:&str)->Value{let p=self.call("finding.httpRetestPreview",json!({"findingId":id,"executionId":execution})).unwrap();self.call("finding.httpRetest",json!({"findingId":id,"executionId":execution,"reviewDigest":p["reviewDigest"]})).unwrap()}
        fn confirm(&self,id:&str)->Value{let s=self.status(id);self.call("finding.httpConfirm",json!({"findingId":id,"stateDigest":s["stateDigest"],"retestEvidenceId":s["actions"]["confirmRetestEvidenceId"],"rationale":"Only the new comparable response contains one nosniff value.","acknowledgeLimits":true})).unwrap()}
        fn edit_capture(&self,execution:&str,change:impl FnOnce(&mut Value)){
            self.store.mutate(|s|{let id=s.executions.iter().find(|e|e.id==execution).unwrap().evidence_id.clone().unwrap();let raw=s.assessments.iter_mut().find(|a|a.id==self.assessment).unwrap().evidence.iter_mut().find(|e|e.id==id).unwrap();let mut value:Value=serde_json::from_str(&raw.content).unwrap();change(&mut value);raw.content=value.to_string();raw.sha256=core::hash(&raw.content);Ok(())}).unwrap();
        }
    }
    #[test]fn finding_only_report_omits_unselected_http_observation_categories(){
        let f=Fixture::new();let candidate=f.candidate(&f.observation(&[]));
        let policy=crate::data_policy::DataPolicyService::new();let redactor=f.ai.data_redactor();let resolver=crate::data_host::JournalResolver{store:&f.store};
        crate::data_policy::test_support::authorize(&f.store,&policy,&f.assessment,&redactor,&resolver,None).unwrap();
        let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();
        let preview=recovery.handle_reviewed("report.prepare",json!({"assessmentId":f.assessment,"format":"markdown_bundle","evidenceIds":[],"derivedSubjects":[{"kind":"derived","assessmentId":f.assessment,"producer":"finding","objectId":candidate["finding"]["id"]}],"categories":["finding_text"],"contextTargetRefs":[],"packageSensitivity":"public"}),&f.store,&policy,&redactor).unwrap();
        assert_eq!(preview["canExport"],true);assert_eq!(preview["manifestPreview"]["findings"].as_array().unwrap().len(),1);
        assert_eq!(preview["manifestPreview"]["httpValidations"],json!([]));assert!(preview["manifestPreview"].get("targetCoverage").is_none());
        assert!(preview["manifestPreview"]["assessment"].get("target").is_none());
        assert!(preview["omissions"].as_array().unwrap().iter().any(|v|v["reasonCode"]=="http_observation_categories_not_selected"));
        assert!(!preview["markdownPreview"].as_str().unwrap().contains("Validação factual e retestes HTTP"));
        assert!(!f.store.root.join("exports").exists());
    }
    #[test]fn factual_header_matrix_never_confuses_duplicate_invalid_or_missing_values(){
        for (values,expected) in [(vec![],Outcome::ControlMissing),(vec![b"nosniff".as_slice()],Outcome::ControlPresent),(vec![b" \tNoSnIfF\t ".as_slice()],Outcome::ControlPresent),(vec![b"nosniff".as_slice(),b"nosniff".as_slice()],Outcome::Inconclusive),(vec![b"nosniff".as_slice(),b"other".as_slice()],Outcome::Inconclusive),(vec![b"other".as_slice()],Outcome::Inconclusive),(vec![b"\xff".as_slice()],Outcome::Inconclusive)]{
            let f=Fixture::new();let id=f.observation(&values);assert_eq!(observe(&f.store.snapshot().unwrap(),&id).unwrap().outcome,expected);
        }
    }
    #[test]fn redirects_errors_unknown_peer_and_legacy_are_inconclusive(){
        for (status,peer,v2) in [(301,Some("127.0.0.1:43123"),true),(404,Some("127.0.0.1:43123"),true),(500,Some("127.0.0.1:43123"),true),(200,None,true),(200,Some("127.0.0.2:43123"),true),(200,Some("127.0.0.1:43123"),false)]{
            let f=Fixture::new();let id=f.execution(&[],status,peer,"completed",v2);let p=f.call("finding.httpPreview",json!({"executionId":id,"ruleId":RULE})).unwrap();assert_eq!(p["observation"]["outcome"],"inconclusive");assert_eq!(p["canCreateCandidate"],false);
        }
    }
    #[test]fn invalid_capture_or_cross_bound_identifiers_are_rejected_without_mutation(){
        for mode in 0..5{let f=Fixture::new();let id=f.observation(&[]);f.edit_capture(&id,|raw|match mode{0=>raw["planDigest"]=json!("0".repeat(64)),1=>raw["target"]=json!("http://elsewhere.invalid/"),2=>raw["headerValues"]["x-content-type-options"]=json!(["not base64"]),3=>{raw["headerValues"].as_object_mut().unwrap().remove("server");},_=>raw["headers"]["x-content-type-options"]=json!("nosniff")});let before=serde_json::to_string(&f.store.snapshot().unwrap()).unwrap();assert!(f.call("finding.httpPreview",json!({"executionId":id,"ruleId":RULE})).is_err());assert_eq!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap(),before);}
    }
    #[test]fn host_candidate_is_deduplicated_and_manual_wrappers_cannot_impersonate_it(){
        let f=Fixture::new();let execution=f.observation(&[]);let candidate=f.candidate(&execution);let id=candidate["finding"]["id"].as_str().unwrap();assert_eq!(candidate["finding"]["title"],TITLE);assert_eq!(candidate["finding"]["status"],"candidate");let before=serde_json::to_string(&f.store.snapshot().unwrap()).unwrap();assert_eq!(f.candidate(&execution)["created"],false);assert_eq!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap(),before);
        let raw=f.store.snapshot().unwrap().assessments[0].evidence.iter().find(|e|e.id==candidate["status"]["claim"]["evidenceId"]).unwrap().content.clone();
        let imported=f.store.handle(core::Request{action:"evidence.import".into(),payload:json!({"assessmentId":f.assessment,"name":"User supplied claim imitation","sourceDescription":"Synthetic unverified user text","content":raw,"acknowledgeUnverifiedSource":true})}).unwrap();
        let generic=f.ai.handle("candidate.record",json!({"assessmentId":f.assessment,"title":"Unrelated arbitrary allegation","description":"This is not a host HTTP claim","severity":"high","recommendation":"Review human evidence","evidenceIds":[imported["id"]]}),&f.store).unwrap();let generic_id=generic["id"].as_str().unwrap();assert_eq!(f.status(generic_id)["supported"],false);assert_eq!(f.status(id)["supported"],true);
        let invalid=f.call("finding.httpCandidate",json!({"executionId":execution,"ruleId":RULE,"reviewDigest":candidate["status"]["stateDigest"],"acknowledgeLimits":true,"title":"SQL injection"}));assert!(invalid.is_err());
    }
    #[test]fn generic_validation_and_reopening_cannot_replace_a_typed_claim(){
        let f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();let snapshot=f.store.snapshot().unwrap();let evidence=snapshot.findings[0].evidence_ids.clone();
        assert!(f.store.handle(core::Request{action:"finding.validate".into(),payload:json!({"id":id,"method":"controlled_reproduction","expected":"Arbitrary claim","observed":"Arbitrary text","evidenceIds":evidence})}).is_err());
        f.validate(id);assert!(f.store.handle(core::Request{action:"finding.update".into(),payload:json!({"id":id,"status":"candidate"})}).is_err());
        assert!(f.store.mutate(|s|{s.findings[0].title="Unrelated exploit claim".into();Ok(())}).is_err());
        assert!(f.store.mutate(|s|{s.findings[0].evidence_ids.retain(|eid|eid!=c["status"]["claim"]["evidenceId"].as_str().unwrap());Ok(())}).is_err());
        assert!(f.store.handle(core::Request{action:"finding.update".into(),payload:json!({"id":id,"status":"dismissed"})}).is_ok());assert_eq!(f.status(id)["actions"]["canRetest"],false);
    }
    #[test]fn reviewed_positive_cycle_preserves_baseline_and_requires_separate_human_confirmation(){
        let f=Fixture::new();let baseline=f.observation(&[]);let c=f.candidate(&baseline);let id=c["finding"]["id"].as_str().unwrap();let v=f.validate(id);assert_eq!(v["findingStatus"],"validated");
        let execution=f.observation(&[b"nosniff"]);let r=f.retest(id,&execution);assert_eq!(r["recorded"],true);assert_eq!(r["retest"]["outcome"],"control_present");assert_eq!(r["status"]["findingStatus"],"validated");
        let actual=f.confirm(id);assert_eq!(actual["findingStatus"],"fixed");assert_eq!(actual["baseline"]["executionId"],baseline);assert_eq!(actual["baseline"]["outcome"],"control_missing");assert_eq!(actual["confirmation"]["retestEvidenceId"],r["retest"]["evidenceId"]);assert_eq!(actual["actions"]["canConfirm"],false);
        let s=f.store.snapshot().unwrap();assert_eq!(s.validations.len(),2);assert!(has_valid_confirmation(&s,id).unwrap());assert!(crate::remediation::validate_fixed_findings(&s).is_ok());
        assert!(f.store.mutate(|s|{s.assessments[0].evidence.retain(|e|e.id!=actual["confirmation"]["evidenceId"]);Ok(())}).is_err());
    }
    #[test]fn costless_reviews_require_current_state_and_explicit_acknowledgment(){
        let f=Fixture::new();let e=f.observation(&[]);let p=f.call("finding.httpPreview",json!({"executionId":e,"ruleId":RULE})).unwrap();assert!(f.call("finding.httpCandidate",json!({"executionId":e,"ruleId":RULE,"reviewDigest":p["reviewDigest"],"acknowledgeLimits":false})).is_err());
        let c=f.candidate(&e);let id=c["finding"]["id"].as_str().unwrap();let old=f.status(id);f.observation(&[]);
        assert!(f.call("finding.httpValidate",json!({"findingId":id,"stateDigest":old["stateDigest"],"rationale":"Stale review","acknowledgeLimits":true})).is_err());let fresh=f.status(id);assert!(f.call("finding.httpValidate",json!({"findingId":id,"stateDigest":fresh["stateDigest"],"rationale":" ","acknowledgeLimits":true})).is_err());assert!(f.call("finding.httpValidate",json!({"findingId":id,"stateDigest":fresh["stateDigest"],"rationale":"Valid text","acknowledgeLimits":false})).is_err());assert_eq!(f.store.snapshot().unwrap().validations.len(),0);
    }
    #[test]fn retest_requires_new_plan_execution_and_chronology_after_validation(){
        let f=Fixture::new();let e=f.observation(&[]);let c=f.candidate(&e);let id=c["finding"]["id"].as_str().unwrap();let before_validation=f.observation(&[b"nosniff"]);f.validate(id);
        for selected in [&e,&before_validation]{let p=f.call("finding.httpRetestPreview",json!({"findingId":id,"executionId":selected})).unwrap();assert_eq!(p["canRecord"],false);assert!(f.call("finding.httpRetest",json!({"findingId":id,"executionId":selected,"reviewDigest":p["reviewDigest"]})).is_err());}
        let peer=f.observation(&[b"nosniff"]);let mut s=f.store.snapshot().unwrap();let baseline=s.executions.iter().find(|e|e.id==c["status"]["baseline"]["executionId"]).unwrap().clone();let newer=s.executions.iter_mut().find(|e|e.id==peer).unwrap();newer.plan_id=baseline.plan_id;assert!(observe(&s,&peer).is_err());
    }
    #[test]fn failed_missing_or_inconclusive_retests_never_allow_fixed(){
        for mode in 0..4{let f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();f.validate(id);let e=match mode{0=>f.observation(&[]),1=>f.observation(&[b"nosniff",b"other"]),2=>f.execution(&[],200,None,"failed",true),_=>f.execution(&[],200,None,"canceled",true)};let r=f.retest(id,&e);assert_ne!(r["retest"]["outcome"],"control_present");assert_eq!(r["status"]["findingStatus"],"validated");assert_eq!(r["status"]["actions"]["canConfirm"],false);assert!(f.call("finding.httpConfirm",json!({"findingId":id,"retestEvidenceId":r["retest"]["evidenceId"],"stateDigest":r["status"]["stateDigest"],"rationale":"Cannot confirm this result","acknowledgeLimits":true})).is_err());}
    }
    #[test]fn changed_peer_and_destinations_are_preserved_as_inconclusive_not_same_environment(){
        let f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();f.validate(id);let e=f.observation(&[b"nosniff"]);
        f.store.mutate(|s|{let plan_id=s.executions.iter().find(|x|x.id==e).unwrap().plan_id.clone();let p=s.plans.iter_mut().find(|p|p.id==plan_id).unwrap();p.resolved_addresses=vec!["127.0.0.2:43123".into()];p.digest=executor::plan_digest(p);let digest=p.digest.clone();let eid=s.executions.iter().find(|x|x.id==e).unwrap().evidence_id.clone().unwrap();let raw=s.assessments[0].evidence.iter_mut().find(|r|r.id==eid).unwrap();let mut v:Value=serde_json::from_str(&raw.content).unwrap();v["planDigest"]=json!(digest);v["resolvedAddresses"]=json!(["127.0.0.2:43123"]);v["remoteAddress"]=json!("127.0.0.2:43123");raw.content=v.to_string();raw.sha256=core::hash(&raw.content);Ok(())}).unwrap();
        let r=f.retest(id,&e);assert_eq!(r["retest"]["outcome"],"inconclusive");assert_eq!(r["retest"]["comparison"]["sameResolvedAddresses"],false);assert_eq!(r["retest"]["comparison"]["samePeer"],false);
    }
    #[test]fn newer_unreviewed_execution_blocks_confirmation_and_recording_is_idempotent(){
        let f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();f.validate(id);let positive=f.observation(&[b"nosniff"]);let preview=f.call("finding.httpRetestPreview",json!({"findingId":id,"executionId":positive})).unwrap();let request=json!({"findingId":id,"executionId":positive,"reviewDigest":preview["reviewDigest"]});let r=f.call("finding.httpRetest",request.clone()).unwrap();let before=serde_json::to_string(&f.store.snapshot().unwrap()).unwrap();assert_eq!(f.call("finding.httpRetest",request).unwrap()["recorded"],false);assert_eq!(before,serde_json::to_string(&f.store.snapshot().unwrap()).unwrap());
        f.observation(&[]);let latest=f.status(id);assert_eq!(latest["actions"]["canConfirm"],false);assert!(latest["actions"]["reasons"].as_array().unwrap().contains(&json!("newer_unreviewed_execution")));assert!(f.call("finding.httpConfirm",json!({"findingId":id,"retestEvidenceId":r["retest"]["evidenceId"],"stateDigest":r["status"]["stateDigest"],"rationale":"Stale after newer response","acknowledgeLimits":true})).is_err());
    }
    #[test]fn clock_rollback_cannot_hide_a_later_missing_failed_or_canceled_execution(){
        for execution_status in ["completed","failed","canceled"]{
            let f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();let validated=f.validate(id);
            let positive=f.observation(&[b"nosniff"]);let r=f.retest(id,&positive);assert_eq!(r["status"]["actions"]["canConfirm"],true);
            let validation_at=stamp(validated["validation"]["createdAt"].as_str().unwrap()).unwrap();let positive_at=stamp(r["retest"]["observation"]["startedAt"].as_str().unwrap()).unwrap();
            let offset=(positive_at-validation_at).num_nanoseconds().unwrap()/2;assert!(offset>0);let earlier=(validation_at+chrono::Duration::nanoseconds(offset)).to_rfc3339();
            let later=f.execution(&[],200,Some("127.0.0.1:43123"),execution_status,true);
            // Simulate a host clock correction between two dispatches. Keep each
            // plan/evidence chronology valid; only their wall-clock order differs.
            f.store.mutate(|s|{
                let execution=s.executions.iter_mut().find(|e|e.id==later).unwrap();execution.started_at=earlier.clone();execution.finished_at=Some(earlier.clone());
                let plan=s.plans.iter_mut().find(|p|p.id==execution.plan_id).unwrap();plan.created_at=earlier.clone();plan.approved_at=Some(earlier.clone());plan.digest=executor::plan_digest(plan);
                if let Some(evidence_id)=&execution.evidence_id{let raw=s.assessments.iter_mut().find(|a|a.id==f.assessment).unwrap().evidence.iter_mut().find(|e|&e.id==evidence_id).unwrap();let mut body:Value=serde_json::from_str(&raw.content).unwrap();body["planDigest"]=json!(plan.digest);raw.content=body.to_string();raw.sha256=core::hash(&raw.content);raw.created_at=earlier.clone();}
                Ok(())
            }).unwrap();
            let snapshot=f.store.snapshot().unwrap();assert_eq!(snapshot.executions.last().unwrap().id,later);assert!(stamp(&earlier).unwrap()<positive_at);
            let preview=f.call("finding.httpRetestPreview",json!({"findingId":id,"executionId":later})).unwrap();assert_eq!(preview["canRecord"],true);assert_eq!(preview["comparison"]["afterValidation"],true);
            let latest=f.status(id);assert_ne!(latest["stateDigest"],r["status"]["stateDigest"]);assert_eq!(latest["actions"]["canConfirm"],false,"{execution_status}");assert!(latest["actions"]["confirmRetestEvidenceId"].is_null());assert!(latest["actions"]["reasons"].as_array().unwrap().contains(&json!("newer_unreviewed_execution")));
            let before=serde_json::to_string(&snapshot).unwrap();assert!(f.call("finding.httpConfirm",json!({"findingId":id,"retestEvidenceId":r["retest"]["evidenceId"],"stateDigest":latest["stateDigest"],"rationale":"Fresh review must not ignore the later registered observation","acknowledgeLimits":true})).is_err());assert_eq!(before,serde_json::to_string(&f.store.snapshot().unwrap()).unwrap());
        }
    }
    #[test]fn archive_blocks_new_review_and_cross_assessment_execution_is_refused(){
        let mut f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();f.validate(id);
        let other=f.store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":f.project,"title":"Another assessment","kind":"web","target":"http://127.0.0.1:43123/","objective":"Separate owned fixture scope"})}).unwrap();
        let original=f.assessment.clone();f.assessment=other["id"].as_str().unwrap().into();let foreign=f.observation(&[b"nosniff"]);f.assessment=original;
        assert!(f.call("finding.httpRetestPreview",json!({"findingId":id,"executionId":foreign})).is_err());
        f.store.handle(core::Request{action:"project.archive".into(),payload:json!({"id":f.project})}).unwrap();let status=f.status(id);assert_eq!(status["actions"]["canRetest"],false);assert!(status["actions"]["reasons"].as_array().unwrap().contains(&json!("archived_project")));
    }
    #[test]fn proof_survives_backup_restore_and_human_rationale_is_redacted(){
        let f=Fixture::new();let c=f.candidate(&f.observation(&[]));let id=c["finding"]["id"].as_str().unwrap();let key="SYNTHETIC_HTTP_LOCAL_KEY_NO_PROVIDER_0123456789";f.ai.handle("ai.key.save",json!({"key":key}),&f.store).unwrap();let current=f.status(id);let validated=f.call("finding.httpValidate",json!({"findingId":id,"stateDigest":current["stateDigest"],"rationale":format!("Review contains {key} and is redacted locally."),"acknowledgeLimits":true})).unwrap();assert!(!validated.to_string().contains(key));
        let execution=f.observation(&[b"nosniff"]);f.retest(id,&execution);f.confirm(id);
        let service=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();let backup=crate::recovery::test_support::backup(&service,&f.store).unwrap();let restored=service.handle("backup.restore",json!({"id":backup["id"]}),&f.store).unwrap();let copy=Store::open(std::path::PathBuf::from(restored["path"].as_str().unwrap())).unwrap();let snapshot=copy.snapshot().unwrap();assert!(has_valid_confirmation(&snapshot,id).unwrap());assert_eq!(status(&snapshot,id).unwrap()["findingStatus"],"fixed");assert!(!serde_json::to_string(&snapshot).unwrap().contains(key));
        let bundle=crate::recovery::test_support::report(&service,&f.store,&f.assessment,true).unwrap();let manifest_bytes=std::fs::read(bundle["manifestPath"].as_str().unwrap()).unwrap();let manifest:Value=serde_json::from_slice(&manifest_bytes).unwrap();let report=std::fs::read_to_string(bundle["path"].as_str().unwrap()).unwrap();let rows=manifest["httpValidations"].as_array().unwrap();assert_eq!(rows.len(),1);assert_eq!(rows[0]["findingId"],id);assert_eq!(rows[0]["baseline"]["outcome"],"control_missing");assert_eq!(rows[0]["retests"][0]["outcome"],"control_present");assert_eq!(rows[0]["retests"][0]["observation"]["headerTextIncluded"],false);assert!(rows[0]["retests"][0]["observation"]["headerValues"][0].get("text").is_none());assert!(!manifest.to_string().contains(key));assert!(!report.contains(key));assert!(report.contains("Validação factual e retestes HTTP"));assert!(report.contains("127.0.0.1:43123"));
        let checksums:Value=serde_json::from_slice(&std::fs::read(std::path::Path::new(bundle["directory"].as_str().unwrap()).join("checksums.json")).unwrap()).unwrap();assert_eq!(checksums.as_array().unwrap().iter().find(|row|row["name"]=="manifest.json").unwrap()["sha256"],core::hash(std::str::from_utf8(&manifest_bytes).unwrap()));assert_eq!(checksums.as_array().unwrap().iter().find(|row|row["name"]=="report.md").unwrap()["sha256"],core::hash(&report));
        let simple=crate::recovery::test_support::report(&service,&f.store,&f.assessment,false).unwrap();let text=std::fs::read_to_string(simple["path"].as_str().unwrap()).unwrap();assert!(text.contains("Validação factual e retestes HTTP"));assert!(!text.contains(key));
    }
    #[test]fn actual_loopback_head_preserves_duplicate_bytes_and_peer(){
        use std::{io::{Read,Write},net::TcpListener,time::{Duration,Instant}};
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().join("data")).unwrap();let listener=TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();let address=listener.local_addr().unwrap();
        let server=std::thread::spawn(move||{let deadline=Instant::now()+Duration::from_secs(5);let mut stream=loop{match listener.accept(){Ok((stream,_))=>break stream,Err(error)if error.kind()==std::io::ErrorKind::WouldBlock&&Instant::now()<deadline=>std::thread::sleep(Duration::from_millis(5)),other=>panic!("loopback accept failed: {other:?}")}};stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut request=vec![];let mut buffer=[0u8;1024];while !request.windows(4).any(|w|w==b"\r\n\r\n"){let n=stream.read(&mut buffer).unwrap();assert!(n>0);request.extend_from_slice(&buffer[..n]);assert!(request.len()<8192);}assert!(request.starts_with(b"HEAD / HTTP/1.1\r\n"));stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-Content-Type-Options: nosniff\r\nX-Content-Type-Options: other\r\nConnection: close\r\n\r\n").unwrap();});
        let project=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Owned loopback HEAD","kind":"web","target":format!("http://{address}/")})}).unwrap();let assessment=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Duplicate HEAD values","kind":"web","target":project["target"],"objective":"Only the owned bounded loopback listener","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&["http.headers.v1"])})}).unwrap();let executor=executor::Executor::default();let plan=executor.handle("assessment.plan",json!({"id":assessment["id"]}),&store).unwrap();let decision=json!({"id":plan["id"],"digest":plan["digest"]});assert!(executor.handle("plan.run",decision.clone(),&store).is_err());executor.handle("plan.approve",decision.clone(),&store).unwrap();let execution=executor.handle("plan.run",decision,&store).unwrap();server.join().unwrap();assert_eq!(execution["status"],"completed");let facts=observe(&store.snapshot().unwrap(),execution["id"].as_str().unwrap()).unwrap();assert_eq!(facts.remote_address,Some(address.to_string()));assert_eq!(facts.header_values.len(),2);assert_eq!(facts.outcome,Outcome::Inconclusive);assert!(facts.reasons.contains(&"duplicate_header".into()));
    }
}
