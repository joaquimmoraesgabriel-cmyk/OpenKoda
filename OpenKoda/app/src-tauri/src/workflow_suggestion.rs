//! Compile the small graph change offered in a reviewed provider response.
//! Only the local request ledger supplies its source, adapter and evidence.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestionBase {
    #[serde(deserialize_with = "required_nullable")]
    pub head: Option<PlanRevisionRef>,
    pub generation: u64,
    pub last_event: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestionSource {
    pub request_id: String,
    pub proposal_id: String,
    pub context_digest: String,
    pub scope_ref: ScopeRef,
    pub evidence_id: String,
    pub evidence_sha256: String,
    pub adapter: NativeAdapter,
    pub base: SuggestionBase,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SuggestionRequest {
    pub assessment_id: String,
    pub request_id: String,
    pub proposal_id: String,
}

/// An existing active revision must have finished every job. This proposal
/// cannot displace pending reviews, even if a job changes without a new head.
pub fn current_base(
    conn: &rusqlite::Connection, state: &Snapshot, assessment: &str,
) -> Result<Option<SuggestionBase>, String> {
    match work_store::plan_view(conn, state, assessment, None)? {
        None => {
            let legacy_attempts = state.executions.iter().filter(|e| e.assessment_id == assessment).count();
            if legacy_attempts >= MAX_ATTEMPTS as usize || work_store::capture_usage(conn, None)? >= MAX_PROFILE_CAPTURE_BYTES {
                return Ok(None);
            }
            Ok(Some(SuggestionBase { head: None, generation: 0, last_event: 0 }))
        }
        Some(plan) if plan.control == PlanControl::Active && plan.outcome == PlanOutcome::Completed => {
            let (attempts, plan_bytes, profile_bytes) = work_store::remaining_quotas(conn, state, &plan.head)?;
            if plan.head.number >= MAX_REVISIONS || attempts == 0 || plan_bytes == 0 || profile_bytes == 0 {
                return Ok(None);
            }
            Ok(Some(SuggestionBase {
                head: Some(plan.head), generation: plan.generation, last_event: plan.event_cursor.after_sequence,
            }))
        }
        _ => Ok(None),
    }
}

impl SuggestionSource {
    fn validate_fields(&self) -> Result<(), String> {
        for id in [&self.request_id, &self.proposal_id, &self.evidence_id, &self.scope_ref.assessment_id, &self.scope_ref.revision_id] {
            valid_id(id)?;
        }
        if !valid_hash(&self.context_digest) || !valid_hash(&self.evidence_sha256) || !valid_hash(&self.scope_ref.digest)
            || self.adapter == NativeAdapter::TcpConnect {
            return Err(error("work_invalid_request"));
        }
        if let Some(head) = &self.base.head {
            valid_id(&head.plan_id)?;
            valid_id(&head.revision_id)?;
            if !valid_hash(&head.digest) || head.number==0 || head.number>=MAX_REVISIONS
                || self.base.generation<u64::from(head.number) || self.base.last_event==0 {
                return Err(error("work_revision_changed"));
            }
        } else if self.base.generation!=0 || self.base.last_event!=0 {
            return Err(error("work_revision_changed"));
        }
        Ok(())
    }

    pub(crate) fn validate_live(&self, conn: &rusqlite::Connection, state: &Snapshot) -> Result<(), String> {
        self.validate_fields()?;
        let aid = &self.scope_ref.assessment_id;
        if current_base(conn, state, aid)?.as_ref() != Some(&self.base) {
            return Err(error("work_revision_changed"));
        }
        if crate::scope_targets::is_combined(state, aid) || scope::current_ref(state, aid)?.as_ref() != Some(&self.scope_ref) {
            return Err(error("work_revision_changed"));
        }
        scope::check_selected_work(state, aid, self.adapter.as_str(), Some(&self.scope_ref), &[])?;
        let evidence = state.assessments.iter().find(|a| a.id == *aid)
            .and_then(|a| a.evidence.iter().find(|e| e.id == self.evidence_id))
            .ok_or_else(|| error("work_input_changed"))?;
        if evidence.sha256 != self.evidence_sha256 || core::hash(&evidence.content) != self.evidence_sha256 {
            return Err(error("work_input_changed"));
        }
        Ok(())
    }

    pub(crate) fn validate_revision(&self, revision: &ActionPlanRevision) -> Result<(), String> {
        self.validate_fields()?;
        let [review, observation] = revision.body.nodes.as_slice() else { return Err(error("work_storage_unavailable")); };
        let expected = WorkInput::Evidence { evidence_id: self.evidence_id.clone(), expected_raw_sha256: self.evidence_sha256.clone() };
        if revision.body.scope_ref != self.scope_ref || revision.assessment_id != self.scope_ref.assessment_id
            || revision.previous_revision_id.as_deref() != self.base.head.as_ref().map(|r| r.revision_id.as_str())
            || !review.depends_on.is_empty() || observation.depends_on.len() != 1 || observation.depends_on.first() != Some(&review.id)
            || !matches!(&review.intent, OperationIntent::HumanReview { inputs } if inputs.as_slice() == [expected])
            || !matches!(&observation.intent, OperationIntent::Native { adapter, target_ref: None, ports: None } if *adapter == self.adapter) {
            return Err(error("work_storage_unavailable"));
        }
        Ok(())
    }

    fn draft(&self, limits: PlanLimits) -> Result<PlanPrepareRequest,String> {
        let (purpose, observation) = match self.adapter {
            NativeAdapter::HttpHeaders => ("Observar os cabeçalhos HTTP do alvo", "Cabeçalhos da resposta HEAD do alvo autorizado."),
            NativeAdapter::LocalInventory => ("Inventariar os arquivos do alvo", "Manifesto dos arquivos selecionados com tamanho e SHA-256."),
            NativeAdapter::SourceRules => ("Observar padrões no código selecionado", "Ocorrências das regras locais com localização e cobertura."),
            NativeAdapter::TcpConnect => return Err(error("work_intent_unavailable")),
        };
        let review = NodeDraft {
            key: "review".into(), existing_node_id: None, purpose: "Revisar a evidência selecionada".into(),
            expected_observation: "Decisão humana sobre prosseguir com a observação sugerida.".into(),
            depends_on_keys: vec![],
            intent: DraftIntent::HumanReview { inputs: vec![DraftInput::Evidence {
                evidence_id: self.evidence_id.clone(), expected_raw_sha256: self.evidence_sha256.clone(),
            }] },
        };
        let next = NodeDraft {
            key: "observation".into(), existing_node_id: None, purpose: purpose.into(), expected_observation: observation.into(),
            depends_on_keys: vec!["review".into()],
            intent: DraftIntent::Native { adapter: self.adapter, target_ref: None, ports: None },
        };
        Ok(PlanPrepareRequest {
            assessment_id: self.scope_ref.assessment_id.clone(), expected_head: self.base.head.clone(),
            expected_generation: self.base.generation, goal: "Revisar a evidência e realizar a observação escolhida.".into(),
            nodes: vec![review, next], limits,
        })
    }
}

impl WorkflowService {
    pub(crate) fn prepare_suggestion(
        &self, store: &Store, ai: &crate::ai::AiService, aid: &str, request: &str, proposal: &str,
        redactor: &dyn DataRedactor,
    ) -> Result<PlanPreparation, String> {
        for id in [aid, request, proposal] { valid_id(id)?; }
        let source = ai.plan_suggestion_source(aid, request, proposal)?;
        let limits = store.work_read(|conn, state| {
            source.validate_live(conn, state)?;
            match &source.base.head {
                Some(head) => Ok(work_store::revision(conn, &head.revision_id)?.body.limits),
                None => Ok(PlanLimits { max_attempts_total: 2, max_capture_bytes_total: 1024 * 1024 }),
            }
        })?;
        self.prepare_bound(store, source.draft(limits)?, redactor, Some(source))
    }
}
