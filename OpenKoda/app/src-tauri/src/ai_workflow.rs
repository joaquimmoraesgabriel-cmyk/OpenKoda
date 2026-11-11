//! Private adapter from workflow identities to the existing investigation runner.
//! Reviews are checked afresh; no serialized object here grants provider access.
use super::*;
use crate::workflow::{ResultRef,Perspective,EffectRef,AttemptState,DerivedResultProducer,Backend,CompiledOperationRef};
use crate::data_policy::{DataReviewRef,DataUseReceiptRef,DataUseOutcome,Sensitivity};
use crate::work_supervisor::{EffectTermination,QueuedAttemptToken,QueuedRecoveryCandidate,QueuedRecoveryPermit};

#[cfg(all(test,windows))]
#[path="ai_workflow_tests.rs"]
mod tests;
#[path="ai_direct_recovery.rs"]
mod direct_recovery;
pub(crate) use direct_recovery::AiTerminalProof;

#[derive(Clone)]
pub(crate) struct WorkAiExpectation {
    pub assessment_id:String,
    pub scope_ref:ScopeRef,
    pub inputs:Vec<ResultRef>,
    pub perspectives:Vec<Perspective>,
}

#[derive(Clone)]
pub(crate) struct WorkAiInspection {
    id:String,digest:String,expected:WorkAiExpectation,data_review:DataReviewRef,
    expires_at:String,journal_fingerprint:String,context_target_refs:Vec<ScopeTargetRef>,
}
impl WorkAiInspection {
    pub(crate) fn id(&self)->&str{&self.id}
    pub(crate) fn digest(&self)->&str{&self.digest}
    pub(crate) fn assessment_id(&self)->&str{&self.expected.assessment_id}
    pub(crate) fn scope_ref(&self)->&ScopeRef{&self.expected.scope_ref}
    pub(crate) fn inputs(&self)->&[ResultRef]{&self.expected.inputs}
    pub(crate) fn perspectives(&self)->&[Perspective]{&self.expected.perspectives}
    pub(crate) fn data_review(&self)->&DataReviewRef{&self.data_review}
    pub(crate) fn expires_at(&self)->&str{&self.expires_at}
    pub(crate) fn journal_fingerprint(&self)->&str{&self.journal_fingerprint}
    pub(crate) fn context_target_refs(&self)->&[ScopeTargetRef]{&self.context_target_refs}
}

/// Not Clone/Serialize/Deserialize: its cache owner moves it into one dispatch.
pub(crate) struct QueuedAiGrant {
    inspection:WorkAiInspection,budget:Option<budget::Consent>,acknowledge_sensitive:bool,
}
impl QueuedAiGrant {pub(crate) fn inspection(&self)->&WorkAiInspection{&self.inspection}}

/// Constructed only after reading exact persisted child/parent journals.
pub(crate) struct WorkAiCompletion {
    assessment_id:String,effect:EffectRef,termination:EffectTermination,
    outcome:AttemptState,results:Vec<ResultRef>,receipt_refs:Vec<DataUseReceiptRef>,
    journal_fingerprint:String,
}
impl WorkAiCompletion {
    pub(crate) fn assessment_id(&self)->&str{&self.assessment_id}
    pub(crate) fn effect(&self)->&EffectRef{&self.effect}
    pub(crate) fn termination(&self)->EffectTermination{self.termination}
    pub(crate) fn outcome(&self)->AttemptState{self.outcome}
    pub(crate) fn results(&self)->&[ResultRef]{&self.results}
    pub(crate) fn receipt_refs(&self)->&[DataUseReceiptRef]{&self.receipt_refs}
    pub(crate) fn journal_fingerprint(&self)->&str{&self.journal_fingerprint}
}

fn journal_fingerprint(plan:&Plan)->Result<String,String>{
    // JournalResolver hashes parsed JSON, not the Rust field serialization order.
    let value=serde_json::to_value(plan).map_err(failure)?;
    Ok(crate::core::hash(&value.to_string()))
}
fn roles(perspectives:&[Perspective])->Vec<&'static str>{perspectives.iter().map(|p|match p{Perspective::Evidence=>"evidence",Perspective::Counterevidence=>"counterevidence",Perspective::Remediation=>"remediation"}).collect()}

struct RecoveredChild {status:String,usage:Value,receipt:DataUseReceiptRef,not_sent:bool}

fn receipt_reference(receipt:&crate::data_policy::DataUseReceipt)->DataUseReceiptRef{DataUseReceiptRef{id:receipt.id.clone(),digest:receipt.digest.clone()}}
fn recovery_error()->String{"work_reconciliation_required: registros da investigação não comprovam término.".into()}

impl AiService {
    /// The core callback commits the command receipt and lease release together.
    /// No review is re-admitted and no provider/credential operation occurs here.
    pub(crate) fn with_work_reconciliation<T>(&self,store:&Store,attempt_id:&str,finish:impl FnOnce(&QueuedRecoveryPermit<'_>,&WorkAiCompletion)->Result<T,String>)->Result<T,String>{
        let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;
        let active=self.investigation_active.lock().map_err(failure)?;
        if active.is_some(){return Err("work_busy".into());}
        let permit=self.arbiter.claim_queued_reconciliation(store,attempt_id,Backend::Ai)?;
        let completion=self.reconcile_work_investigation(store,permit.candidate())?;
        finish(&permit,&completion)
    }
    fn reconcile_work_investigation(&self,store:&Store,candidate:&QueuedRecoveryCandidate)->Result<WorkAiCompletion,String>{
        let CompiledOperationRef::AiInvestigation{investigation_id,digest,expires_at}=&candidate.compiled else{return Err(recovery_error());};
        if candidate.owner.kind!=crate::work_supervisor::EffectOwnerKind::AiInvestigationIntent||candidate.owner.id!=*investigation_id||candidate.owner.assessment_id.as_deref()!=Some(candidate.assessment_id.as_str())||candidate.reserved_effect!=(EffectRef::AiInvestigation{id:investigation_id.clone()})||candidate.effect.as_ref().is_some_and(|effect|effect!=&candidate.reserved_effect)||candidate.kali_binding.is_some(){return Err(recovery_error());}
        let mut plan=self.load_investigation(investigation_id)?;
        let input_ids=candidate.input_refs.iter().map(|input|match input{ResultRef::Evidence{id,..}=>Ok(id.as_str()),_=>Err(recovery_error())}).collect::<Result<Vec<_>,_>>()?;
        if plan.assessment_id!=candidate.assessment_id||plan.digest!=*digest||plan.expires_at!=*expires_at||plan.data_review.as_ref()!=Some(&candidate.data_review_ref)||input_ids!=plan.evidence_ids.iter().map(String::as_str).collect::<Vec<_>>(){return Err(recovery_error());}
        // Read all receipts before changing any journal. Their immutable links,
        // rather than a human acknowledgment or financial estimate, identify use.
        let snapshot=store.snapshot()?;
        let parent_receipts=snapshot.data_use_receipts.iter().filter(|receipt|receipt.effect.id==plan.id).collect::<Vec<_>>();
        if parent_receipts.len()>1{return Err(recovery_error());}
        let parent=parent_receipts.first().copied();
        if parent.is_some_and(|receipt|receipt.effect.kind!="investigation"||receipt.assessment_id.as_deref()!=Some(plan.assessment_id.as_str())||receipt.parent_receipt_ref.is_some()||receipt.review_ref!=candidate.data_review_ref||receipt.operation!=crate::data_policy::DataOperation::AiInvestigation||receipt.destination!=crate::data_policy::DataDestination::RemoteAi||receipt.events.is_empty())||plan.data_use_receipt_ref.as_ref().is_some_and(|reference|parent.is_none_or(|receipt|receipt_reference(receipt)!=*reference)){return Err(recovery_error());}
        if candidate.effect.is_none(){
            self.inspect_unbound_investigation(&plan.id,&plan.assessment_id)?;
            if parent.is_some_and(|receipt|receipt.events.iter().any(|event|!matches!(event.outcome,DataUseOutcome::Admitted|DataUseOutcome::NotSent))){return Err(recovery_error());}
            // This also handles admission committed before saving its reference
            // in the parent journal. No child could precede the missing binding.
            self.mark_unbound_investigation_not_sent(&plan.id,&plan.assessment_id)?;
            if let Some(parent)=parent{self.data_policy.finish(store,&receipt_reference(parent),DataUseOutcome::NotSent,vec![])?;}
            plan=self.load_investigation(&plan.id)?;
            return Self::recovery_completion(&plan,EffectTermination::NotStarted,AttemptState::NotStarted,parent.into_iter().map(receipt_reference).collect());
        }
        let parent=parent.ok_or_else(recovery_error)?;
        if plan.data_use_receipt_ref.as_ref()!=Some(&receipt_reference(parent)){return Err(recovery_error());}
        let children=self.inspect_recovery_children(&plan,&snapshot,parent)?;
        // inspect_recovery_children rejects every pending/unknown or contradictory
        // request before this point; terminal money is never used as that proof.
        let mut changed=false;
        for (step,child) in plan.results.iter_mut().zip(&children){
            if let Some(child)=child{
                if step.usage!=child.usage{step.usage=child.usage.clone();changed=true;}
                if matches!(step.status.as_str(),"pending"|"unknown"){
                    step.status=if child.not_sent{"failed_before_dispatch"}else if child.status=="incomplete"{"incomplete"}else{"failed"}.into();
                    step.text.clear();step.error=Some("Solicitação anterior encerrada conforme seu journal; texto não preservado não foi reconstruído nem solicitado novamente.".into());
                    step.finished_at=Some(model::now());changed=true;
                }
            }
        }
        if matches!(plan.status.as_str(),"running"|"canceling"|"unknown"|"reconciled"){
            plan.reconciled_from=Some(plan.status.clone());
            plan.status=if plan.results.iter().any(|step|step.status=="completed"&&!step.text.is_empty()){"partial"}else{"failed"}.into();
            plan.outcome_note="Término das solicitações anteriores confirmado pelos journals locais. Resultados preservados são parciais; nenhuma perspectiva foi repetida e o registro financeiro permaneceu intacto.".into();changed=true;
        }
        if !matches!(plan.status.as_str(),"completed"|"partial"|"failed"|"canceled"){return Err(recovery_error());}
        if plan.status=="completed"&&(plan.results.len()!=plan.perspectives.len()||plan.results.iter().any(|step|step.status!="completed"||step.text.is_empty())){return Err(recovery_error());}
        if changed{self.save_investigation(&plan)?;}
        let mut receipts=vec![receipt_reference(parent)];
        for child in children.iter().flatten(){
            self.data_policy.finish(store,&child.receipt,if child.not_sent{DataUseOutcome::NotSent}else if child.status=="completed"{DataUseOutcome::Completed}else{DataUseOutcome::Failed},vec![])?;
            receipts.push(child.receipt.clone());
        }
        // A terminal data-use receipt is historical, including failure/cancel;
        // repairing an execution record never rewrites that prior disposition.
        if matches!(parent.events.last().map(|event|event.outcome),Some(DataUseOutcome::Admitted|DataUseOutcome::Dispatched|DataUseOutcome::Unknown)){
            self.data_policy.finish(store,&receipt_reference(parent),match plan.status.as_str(){"completed"=>DataUseOutcome::Completed,"canceled"=>DataUseOutcome::Canceled,_=>DataUseOutcome::Failed},vec![])?;
        }
        let outcome=match plan.status.as_str(){"completed"=>AttemptState::Completed,"partial"=>AttemptState::Partial,"canceled"=>AttemptState::Canceled,_=>AttemptState::Failed};
        Self::recovery_completion(&plan,EffectTermination::Stopped,outcome,receipts)
    }
    fn inspect_recovery_children(&self,plan:&Plan,snapshot:&crate::model::Snapshot,parent:&crate::data_policy::DataUseReceipt)->Result<Vec<Option<RecoveredChild>>,String>{
        let db=self.db.lock().map_err(failure)?;
        let mut query=db.prepare("SELECT id FROM requests WHERE json_extract(details,'$.investigationId')=?1").map_err(failure)?;
        let linked=query.query_map([&plan.id],|row|row.get::<_,String>(0)).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?;drop(query);
        if linked.iter().any(|id|!plan.results.iter().any(|step|step.request_id==*id)){return Err(recovery_error());}
        if parent.events.iter().any(|event|event.outcome==DataUseOutcome::NotSent)&&!plan.results.is_empty(){return Err(recovery_error());}
        let mut children=vec![];
        for (index,step) in plan.results.iter().enumerate(){
            let row:Option<(String,String,String,String,String,Option<i64>,Option<i64>,String)>=db.query_row("SELECT assessment,endpoint,model,revision,status,input_tokens,output_tokens,details FROM requests WHERE id=?1",[&step.request_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?))).optional().map_err(failure)?;
            let receipts=snapshot.data_use_receipts.iter().filter(|receipt|receipt.effect.id==step.request_id).collect::<Vec<_>>();
            let Some((assessment,endpoint,model,revision,status,input,output,raw))=row else{
                // A saved explicit preflight rejection is safe. A lost row for
                // pending/unknown is not transformed into proof by mere absence.
                if step.status!="failed_before_dispatch"||!receipts.is_empty(){return Err(recovery_error());}children.push(None);continue;
            };
            let details:Value=serde_json::from_str(&raw).map_err(failure)?;
            if assessment!=plan.assessment_id||endpoint!=plan.endpoint||model!=plan.model||revision!=plan.credential_revision||!matches!(status.as_str(),"completed"|"incomplete"|"failed")||details["investigationId"]!=plan.id||details["perspective"]!=step.perspective||details["dataReviewRef"]!=json!(plan.data_review)||details["scopeRef"]!=json!(plan.scope_ref)||details["maxOutputTokens"]!=plan.budget.max_output_tokens_per_request||receipts.len()!=1{return Err(recovery_error());}
            let receipt=receipts[0];let reference=receipt_reference(receipt);let not_sent=status=="failed"&&details["notDispatched"]==true;
            if receipt.effect.kind!="request"||receipt.assessment_id.as_deref()!=Some(plan.assessment_id.as_str())||receipt.parent_receipt_ref.as_ref()!=Some(&receipt_reference(parent))||receipt.review_ref!=parent.review_ref||receipt.request_index!=Some(index as u8)||details["dataUseReceiptRef"]!=json!(reference)||receipt.events.is_empty(){return Err(recovery_error());}
            let last=receipt.events.last().unwrap().outcome;
            if not_sent{
                if input!=Some(0)||output!=Some(0)||details.get("providerStatus").is_some()||details.get("responseId").is_some()||receipt.events.iter().any(|event|!matches!(event.outcome,DataUseOutcome::Admitted|DataUseOutcome::NotSent)){return Err(recovery_error());}
            }else{
                let remote=details["providerStatus"].as_str();
                if remote.is_some_and(|state|!matches!(state,"completed"|"incomplete"|"failed"|"cancelled"))||status=="completed"&&remote!=Some("completed")||status=="incomplete"&&remote!=Some("incomplete")||input.is_some_and(|value|value<0)||output.is_some_and(|value|value<0)||!receipt.events.iter().any(|event|event.outcome==DataUseOutcome::Dispatched)||!matches!(last,DataUseOutcome::Dispatched|DataUseOutcome::Unknown|DataUseOutcome::Completed|DataUseOutcome::Failed){return Err(recovery_error());}
                if last==DataUseOutcome::Completed&&status!="completed"||last==DataUseOutcome::Failed&&status=="completed"{return Err(recovery_error());}
            }
            if step.status=="completed"&&status!="completed"||step.status=="incomplete"&&status!="incomplete"||step.status=="failed_before_dispatch"&&!not_sent{return Err(recovery_error());}
            children.push(Some(RecoveredChild{status,usage:json!({"input_tokens":input,"output_tokens":output}),receipt:reference,not_sent}));
        }
        if snapshot.data_use_receipts.iter().any(|receipt|receipt.parent_receipt_ref.as_ref()==Some(&receipt_reference(parent))&&!plan.results.iter().any(|step|step.request_id==receipt.effect.id)){return Err(recovery_error());}
        Ok(children)
    }
    fn recovery_completion(plan:&Plan,termination:EffectTermination,outcome:AttemptState,receipt_refs:Vec<DataUseReceiptRef>)->Result<WorkAiCompletion,String>{
        let fingerprint=journal_fingerprint(plan)?;
        let results=if termination==EffectTermination::Stopped&&plan.results.iter().any(|step|step.status=="completed"&&!step.text.is_empty()){vec![ResultRef::Derived{producer:DerivedResultProducer::Investigation,id:plan.id.clone(),fingerprint:fingerprint.clone()}]}else{vec![]};
        Ok(WorkAiCompletion{assessment_id:plan.assessment_id.clone(),effect:EffectRef::AiInvestigation{id:plan.id.clone()},termination,outcome,results,receipt_refs,journal_fingerprint:fingerprint})
    }
    pub(crate) fn with_work_investigation<T>(&self,store:&Store,id:&str,digest:&str,expected:&WorkAiExpectation,attach:impl FnOnce(&WorkAiInspection)->Result<T,String>)->Result<T,String>{
        let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;
        let inspection=self.inspect_work_investigation_locked(store,id,digest,expected)?;
        // No database lock is held while core records the immutable binding.
        attach(&inspection)
    }
    pub(crate) fn inspect_work_investigation(&self,store:&Store,id:&str,digest:&str,expected:&WorkAiExpectation)->Result<WorkAiInspection,String>{
        let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;
        self.inspect_work_investigation_locked(store,id,digest,expected)
    }
    fn inspect_work_investigation_locked(&self,store:&Store,id:&str,digest:&str,expected:&WorkAiExpectation)->Result<WorkAiInspection,String>{
        let plan=self.load_investigation(id)?;
        if plan.status!="prepared"||!plan.results.is_empty()||plan.data_use_receipt_ref.is_some()||plan.digest!=digest||plan.assessment_id!=expected.assessment_id||plan.scope_ref.as_ref()!=Some(&expected.scope_ref)||roles(&expected.perspectives)!=plan.perspectives{return Err("work_input_changed".into());}
        let snapshot=store.snapshot()?;
        let assessment=snapshot.assessments.iter().find(|a|a.id==expected.assessment_id).ok_or("work_input_changed")?;
        let mut ids=Vec::new();
        for input in &expected.inputs {
            let ResultRef::Evidence{id,raw_sha256}=input else{return Err("work_invalid_request".into());};
            if ids.contains(id)||!assessment.evidence.iter().any(|e|e.id==*id&&e.sha256==*raw_sha256&&crate::core::hash(&e.content)==*raw_sha256){return Err("work_input_changed".into());}
            ids.push(id.clone());
        }
        // Evidence order is part of the reviewed packet and the workflow digest.
        if ids!=plan.evidence_ids{return Err("work_input_changed".into());}
        self.validate_investigation_context(&plan,store)?;
        let packet=self.investigation_data_packet(&plan,store)?;
        let review=plan.data_review.clone().ok_or("data_review_required")?;
        self.data_policy.check_review(store,&review,&packet.spec)?;
        Ok(WorkAiInspection{id:plan.id.clone(),digest:plan.digest.clone(),expected:expected.clone(),data_review:review,expires_at:plan.expires_at.clone(),journal_fingerprint:journal_fingerprint(&plan)?,context_target_refs:plan.context_target_refs.clone()})
    }
    pub(crate) fn validate_queued_investigation(&self,store:&Store,inspection:&WorkAiInspection,review:&DataReviewRef,budget:Option<&budget::Consent>,acknowledge_sensitive:bool)->Result<QueuedAiGrant,String>{
        let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;
        self.validate_queued_investigation_locked(store,inspection,review,budget,acknowledge_sensitive)
    }
    fn validate_queued_investigation_locked(&self,store:&Store,inspection:&WorkAiInspection,review:&DataReviewRef,budget:Option<&budget::Consent>,acknowledge_sensitive:bool)->Result<QueuedAiGrant,String>{
        let current=self.inspect_work_investigation_locked(store,&inspection.id,&inspection.digest,&inspection.expected)?;
        if current.journal_fingerprint!=inspection.journal_fingerprint||current.data_review!=*review{return Err("work_input_changed".into());}
        let plan=self.load_investigation(&inspection.id)?;
        let packet=self.investigation_data_packet(&plan,store)?;
        let decision=crate::data_policy::DataPolicyService::check_use(&store.snapshot()?,&packet.spec)?;
        if decision.effective_sensitivity==Sensitivity::Sensitive&&!acknowledge_sensitive{return Err("data_sensitive_confirmation_required".into());}
        let (specs,_)=self.investigation_budget_specs(json!({"id":inspection.id,"digest":inspection.digest}),store)?;
        self.budget.preflight(&specs,budget)?;
        Ok(QueuedAiGrant{inspection:current,budget:budget.cloned(),acknowledge_sensitive})
    }
    pub(crate) fn run_queued_investigation(&self,store:&Store,token:QueuedAttemptToken,grant:QueuedAiGrant)->Result<WorkAiCompletion,String>{
        let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;
        let inspection=grant.inspection.clone();
        let permit=self.arbiter.claim_queued(store,token)?;
        let mut work=work::AiWork::queued(permit,inspection.clone());
        let outcome=(||{
            work.validate_queued(store)?;
            // Enqueue preflight was not an admission. Expiry, policy, credential,
            // evidence and financial review must still match when the slot opens.
            let fresh=self.validate_queued_investigation_locked(store,&inspection,&inspection.data_review,grant.budget.as_ref(),grant.acknowledge_sensitive)?;
            work.require_active(store)?;
            self.run_investigation(store,Run{id:inspection.id.clone(),digest:inspection.digest.clone(),allow_remote:true,budget_review:fresh.budget,acknowledge_sensitive:fresh.acknowledge_sensitive,data_review:Some(inspection.data_review.clone())},&mut work)
        })();
        // Never infer NotStarted from an error alone: admission may have written
        // its journal before a different database failed to bind/finalize.
        let completion=self.work_investigation_completion(store,&inspection,work.is_bound())?;
        work.finish_queued(store,&completion)?;
        let _=outcome; // The exact durable outcome is exposed through workflow.
        Ok(completion)
    }
    fn work_investigation_completion(&self,store:&Store,inspection:&WorkAiInspection,bound:bool)->Result<WorkAiCompletion,String>{
        let mut plan=self.load_investigation(&inspection.id)?;
        if plan.assessment_id!=inspection.expected.assessment_id||plan.digest!=inspection.digest{return Err("work_reconciliation_required".into());}
        let (termination,outcome)=if !bound{
            // No child can execute before the parent binding. Inspect the exact
            // journal too, so this cannot release an unrelated/legacy effect.
            let receipt=self.inspect_unbound_investigation(&plan.id,&plan.assessment_id)?;
            if let Some(receipt)=receipt{
                self.mark_unbound_investigation_not_sent(&plan.id,&plan.assessment_id)?;
                self.data_policy.finish(store,&receipt,crate::data_policy::DataUseOutcome::NotSent,vec![])?;
                plan=self.load_investigation(&plan.id)?;
            }
            (EffectTermination::NotStarted,AttemptState::NotStarted)
        }else{
            let observed=self.investigation_work_termination(&plan.id,true)?;
            let state=if observed==EffectTermination::Unknown{AttemptState::Unknown}else{match plan.status.as_str(){"completed"=>AttemptState::Completed,"partial"=>AttemptState::Partial,"canceled"=>AttemptState::Canceled,_=>AttemptState::Failed}};
            (observed,state)
        };
        let fingerprint=journal_fingerprint(&plan)?;
        let results=if termination==EffectTermination::Stopped&&plan.results.iter().any(|step|step.status=="completed"&&!step.text.is_empty()){
            vec![ResultRef::Derived{producer:DerivedResultProducer::Investigation,id:plan.id.clone(),fingerprint:fingerprint.clone()}]
        }else{vec![]};
        let snapshot=store.snapshot()?;
        let child_ids=plan.results.iter().map(|step|step.request_id.as_str()).collect::<Vec<_>>();
        let receipt_refs=snapshot.data_use_receipts.iter().filter(|receipt|receipt.assessment_id.as_deref()==Some(plan.assessment_id.as_str())&&((receipt.effect.kind=="investigation"&&receipt.effect.id==plan.id)||(receipt.effect.kind=="request"&&child_ids.contains(&receipt.effect.id.as_str())))).map(|receipt|DataUseReceiptRef{id:receipt.id.clone(),digest:receipt.digest.clone()}).collect();
        Ok(WorkAiCompletion{assessment_id:plan.assessment_id,effect:EffectRef::AiInvestigation{id:plan.id},termination,outcome,results,receipt_refs,journal_fingerprint:fingerprint})
    }
}
