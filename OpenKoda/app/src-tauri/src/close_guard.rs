//! Process-local admission for native window close. No SQLite or long-running work under this mutex.
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Area { Queue, Assessment, Kali, Settings, Current }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Activity { pub area: Area, pub assessment_id: Option<Uuid> }

impl Activity {
    pub fn queue(assessment_id: Option<&str>) -> Self {
        Self { area: Area::Queue, assessment_id: assessment_id.and_then(|s| Uuid::parse_str(s).ok()) }
    }

    pub fn request(action: &str, payload: &Value) -> Option<Self> {
        // Known observations cannot dispatch effects. Unknown/new actions default to tracked.
        if matches!(action, "snapshot" | "projectPurge.status" | "activation.status" | "activation.list"
            | "environment.status" | "host.connectivity" | "kali.mcp.status" | "kali.mcp.assessmentEnvironments" | "ai.status" | "ai.context" | "ai.history"
            | "ai.budget.status" | "ai.budget.history" | "ai.investigation.list" | "data.session.select" | "data.policy.status"
            | "assessment.scopeStatus" | "assessment.scopeOptions" | "assessment.combinedOptions"
            | "project.targetOptions" | "work.plan.get" | "work.queue" | "work.events" | "work.aiLocalClosure.list"
            | "capture.list" | "capture.status" | "backup.list" | "support.review" | "evidence.origins"
            | "remediation.list" | "remediation.get" | "finding.httpStatus" | "environment.archive.list"
            | "credential.reference.list" | "retention.inventory" | "data.classification.subjects") {
            return None;
        }
        let area = if action.starts_with("environment.") { Area::Kali }
            else if action.starts_with("work.") { Area::Queue }
            else if action.starts_with("assessment.") || action.starts_with("plan.") || action.starts_with("kali.mcp.")
                || matches!(action,"ai.ask"|"ai.operation.propose"|"ai.investigation.run") { Area::Assessment }
            else if action.starts_with("support.") || action.starts_with("settings.") || action.starts_with("backup.")
                || matches!(action, "ai.test"|"ai.models"|"ai.key.save"|"ai.key.remove") { Area::Settings }
            else { Area::Current };
        Some(Self { area, assessment_id: payload.get("assessmentId").and_then(Value::as_str).and_then(|s| Uuid::parse_str(s).ok()) })
    }
}

#[derive(Default)]
struct State { closing: bool, next: u64, live: BTreeMap<u64, Activity> }

#[derive(Default)]
pub(crate) struct CloseGate(Mutex<State>);

pub(crate) struct Admission { gate: Arc<CloseGate>, id: u64 }

impl CloseGate {
    pub fn admit(self: &Arc<Self>, activity: Option<Activity>) -> Result<Option<Admission>, String> {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if state.closing { return Err("A janela está fechando. A operação não foi iniciada.".into()); }
        let Some(activity) = activity else { return Ok(None) };
        state.next = state.next.checked_add(1).ok_or("Limite de operações da sessão atingido.")?;
        let id = state.next;
        state.live.insert(id, activity);
        Ok(Some(Admission { gate: self.clone(), id }))
    }

    #[cfg(test)]
    pub fn request_close(&self) -> Option<Activity> { self.check_close(true) }

    pub fn check_close(&self, exiting: bool) -> Option<Activity> {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        // A real dispatched job has a known assessment. Prefer it over a concurrent short-lived request.
        let current = state.live.values().find(|task| task.area == Area::Queue && task.assessment_id.is_some())
            .or_else(|| state.live.values().next()).copied();
        if exiting && current.is_none() { state.closing = true; }
        current
    }
}

impl Admission {
    pub fn identify_job(&self, assessment_id: &str) {
        let mut state = self.gate.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some(activity) = state.live.get_mut(&self.id) { *activity = Activity::queue(Some(assessment_id)); }
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        self.gate.0.lock().unwrap_or_else(|poison| poison.into_inner()).live.remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn passive_polls_and_historical_states_do_not_prevent_idle_close() {
        let gate = Arc::new(CloseGate::default());
        for action in ["snapshot","work.queue","work.events","work.plan.get","environment.status","host.connectivity"] {
            assert!(gate.admit(Activity::request(action,&json!({"status":"unknown"}))).unwrap().is_none());
        }
        assert_eq!(gate.request_close(), None);
        assert!(gate.admit(Activity::request("plan.run",&json!({}))).is_err());
    }
    #[test]
    fn worker_outlives_enqueue_and_close_never_cancels_it() {
        let gate = Arc::new(CloseGate::default());
        let ipc = gate.admit(Activity::request("work.operation.enqueue",&json!({}))).unwrap();
        let worker = gate.admit(Some(Activity::queue(None))).unwrap().unwrap();
        let id = Uuid::new_v4();
        worker.identify_job(&id.to_string());
        drop(ipc);
        for _ in 0..2 { assert_eq!(gate.request_close().unwrap().assessment_id, Some(id)); }
        let cancel = gate.admit(Activity::request("work.cancel",&json!({}))).unwrap();
        drop(worker);
        assert!(gate.request_close().is_some());
        drop(cancel);
        assert!(gate.request_close().is_none());
    }
    #[test]
    fn concurrent_admission_and_close_have_a_single_winner() {
        for _ in 0..32 {
            let gate = Arc::new(CloseGate::default());
            let peer = gate.clone();
            let signal = std::sync::Arc::new(std::sync::Barrier::new(2));
            let signal2 = signal.clone();
            let worker = std::thread::spawn(move || { signal2.wait(); peer.admit(Some(Activity::queue(None))) });
            signal.wait();
            let refused_close = gate.request_close().is_some();
            assert_eq!(refused_close, worker.join().unwrap().is_ok());
        }
    }
}

