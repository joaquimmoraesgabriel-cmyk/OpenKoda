//! Native admission for the source-code edition. Keep the old database readers
//! and history formats available while removing the previous execution paths.
use crate::core::Store;
use serde_json::Value;

// Enabled after the native SDK self-scan completed and survived a profile reopen.
pub(crate) const ENABLED: bool = true;
const UNAVAILABLE: &str = "Esta operação do motor anterior está indisponível na edição de análise de código. O histórico local continua disponível.";
const SOURCE_KIND: &str = "Esta edição aceita projetos de código e arquivos com alvo local.";

fn source_kind(kind: Option<&str>) -> Result<(), String> {
    if matches!(kind, Some("code" | "files")) {
        Ok(())
    } else {
        Err(SOURCE_KIND.into())
    }
}

pub(crate) fn guard(store: &Store, action: &str, payload: &Value) -> Result<(), String> {
    if !ENABLED {
        return Ok(());
    }
    admit(action, payload, |id| {
        let snapshot = store.snapshot()?;
        let project = snapshot.projects.iter().find(|p| p.id == id).ok_or("Projeto não encontrado.")?;
        source_kind(Some(&project.kind))?;
        // A historical code project can contain a combined assessment whose
        // primary target is a URL or whose secondary targets include networks.
        let mut conversations=snapshot.assessments.iter().filter(|a|a.project_id==id);
        let source=match payload.get("sourceAssessmentId").and_then(Value::as_str){
            Some(aid)=>Some(conversations.find(|a|a.id==aid).ok_or("Conversa de origem ausente.")?),
            None=>conversations.max_by_key(|a|&a.updated_at),
        };
        if let Some(source)=source{
            source_kind(Some(&source.kind))?;
            if let Some(reference)=crate::scope::current_ref(&snapshot,&source.id)?{
                if let Some(set)=snapshot.scope_revisions.iter().find(|r|r.id==reference.revision_id).and_then(|r|r.body.target_set.as_ref()){
                    for entry in &set.entries{source_kind(Some(&entry.kind))?;}
                }
            }
        }
        Ok(project.kind.clone())
    })
}

fn admit(
    action: &str,
    payload: &Value,
    project_kind: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), String> {
    match action {
        "project.launch" | "project.create" | "assessment.create" => {
            source_kind(payload.get("kind").and_then(Value::as_str))?;
            if payload.get("useRipgrep").and_then(Value::as_bool) == Some(true) {
                return Err(UNAVAILABLE.into());
            }
            return Ok(());
        }
        "project.newConversation" => {
            let id = payload.get("projectId").and_then(Value::as_str).ok_or("Projeto ausente.")?;
            return source_kind(Some(&project_kind(id)?));
        }
        // The reviewed metadata, model catalog, budget and plain conversation
        // are also required to configure and use the official security SDK.
        "security.status" | "security.scan.get" | "security.scan.start" | "security.scan.cancel"
        | "window.behavior.get" | "window.behavior.set" | "host.connectivity"
        | "snapshot" | "settings.save" | "support.review" | "support.export"
        | "ai.status" | "ai.key.save" | "ai.key.remove" | "ai.models" | "ai.test"
        | "ai.provider.configure" | "ai.selection.save" | "ai.ask" | "ai.context"
        | "ai.history" | "ai.chat.status" | "ai.chat.stop" | "ai.request.reconcile"
        | "ai.budget.status" | "ai.budget.history" | "ai.budget.configure"
        | "ai.budget.preview" | "ai.budget.resolveUnknown"
        | "project.pickTarget" | "project.archive" | "project.targetOptions"
        | "assessment.scopeStatus" | "assessment.scopeOptions" | "assessment.combinedOptions"
        | "assessment.scopeRevoke" | "evidence.origins" | "evidence.import"
        | "finding.update" | "finding.validate" | "finding.httpStatus"
        | "work.plan.get" | "work.events" | "work.queue" | "work.aiLocalClosure.list"
        | "capture.list" | "capture.status" | "remediation.list" | "remediation.get"
        | "ai.investigation.list" => return Ok(()),
        _ => {}
    }
    if crate::data_policy::DataPolicyService::supports(action)
        || crate::recovery::supports(action)
        || crate::activation::supports(action)
        || crate::project_purge::supports(action)
        || crate::retention::supports(action)
    {
        return Ok(());
    }
    Err(UNAVAILABLE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retired_operations_cannot_be_requested_through_an_alternate_dispatcher() {
        for action in [
            "assessment.start", "assessment.plan", "assessment.sourcePlan", "assessment.networkPlan",
            "plan.approve", "plan.run", "evidence.sourceCapture", "mcp.request", "ai.operation.propose",
            "ai.investigation.run", "work.operation.enqueue", "work.plan.commit", "work.resume.commit",
            "work.retry.prepare", "environment.status", "environment.provision", "packages.apply",
            "kali.data.prepare", "kali.mcp.reconcile", "remediation.apply", "finding.httpRetest",
            "assessment.combinedCreate", "assessment.deriveCreate", "project.targetAdd",
        ] {
            assert!(admit(action, &json!({}), |_| panic!("unexpected store access")).is_err(), "{action}");
        }
    }
    #[test]
    fn legacy_projects_can_be_read_but_cannot_launch_new_network_targets() {
        for kind in ["web", "network", "combined"] {
            for action in ["project.launch", "project.create", "assessment.create"] {
                assert!(admit(action, &json!({"kind":kind}), |_| unreachable!()).is_err());
            }
            assert!(admit("project.newConversation", &json!({"projectId":"old"}), |_| Ok(kind.into())).is_err());
        }
        assert!(admit("project.launch", &json!({"kind":"code","useRipgrep":true}), |_| unreachable!()).is_err());
        for kind in ["code","files"] {
            assert!(admit("project.launch", &json!({"kind":kind}), |_| unreachable!()).is_ok());
            assert!(admit("project.newConversation", &json!({"projectId":"existing"}), |_| Ok(kind.into())).is_ok());
        }
    }
    #[test]
    fn review_chat_persistence_and_sdk_remain_available() {
        for action in ["snapshot","security.status","security.scan.start","security.scan.get","security.scan.cancel",
            "ai.ask","ai.context","ai.history","ai.test","ai.key.save","ai.budget.configure","work.plan.get",
            "data.binding.prepare","data.classification.subjects","credential.reference.create","backup.prepare",
            "backup.commit","backup.restore","project.pickTarget","projectPurge.apply","activation.status"] {
            assert!(admit(action, &json!({}), |_| unreachable!()).is_ok(), "{action}");
        }
    }
}
