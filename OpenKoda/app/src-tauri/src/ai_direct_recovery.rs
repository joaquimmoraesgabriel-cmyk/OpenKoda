//! Startup recovery of a previously bound Direct effect. No renderer proof,
//! provider request, accounting mutation or human local-closure disposition.
use super::*;
use crate::data_policy::{DataUseReceipt,DataPolicyService,DataOperation,DataDestination};
use crate::work_supervisor::{EffectOwner,EffectOwnerKind,UnboundDirectCandidate};

pub(crate) struct AiTerminalProof{
    owner:EffectOwner,boot_id:String,effect:EffectRef,profile_fingerprint:String,
    receipts:Vec<(DataUseReceipt,DataUseOutcome)>,
}
impl AiTerminalProof{
    pub(crate) fn owner(&self)->&EffectOwner{&self.owner}
    pub(crate) fn boot_id(&self)->&str{&self.boot_id}
    pub(crate) fn effect(&self)->&EffectRef{&self.effect}
    pub(crate) fn profile_fingerprint(&self)->&str{&self.profile_fingerprint}
    pub(crate) fn finish_receipts(&self,s:&mut crate::model::Snapshot)->Result<(),String>{
        // Compare the complete observed records again, including their event
        // history, before changing any event in the lease-release transaction.
        for(expected,_)in &self.receipts{if s.data_use_receipts.iter().find(|r|r.id==expected.id)!=Some(expected){return Err(recovery_error());}}
        for(receipt,outcome)in &self.receipts{
            if matches!(receipt.events.last().map(|e|e.outcome),Some(DataUseOutcome::Admitted|DataUseOutcome::Dispatched|DataUseOutcome::Unknown)){
                DataPolicyService::finish_in_snapshot(s,&receipt_reference(receipt),*outcome,vec![])?;
            }
        }Ok(())
    }
}
struct TerminalRequest{assessment:String,endpoint:String,model:String,revision:String,context:String,status:String,details:Value,not_sent:bool}
fn terminal_request(ai:&AiService,id:&str)->Result<Option<TerminalRequest>,String>{
    let row:Option<(String,String,String,String,String,String,String,Option<i64>,Option<i64>)>=ai.db.lock().map_err(failure)?.query_row("SELECT assessment,endpoint,model,revision,context_hash,status,details,input_tokens,output_tokens FROM requests WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))).optional().map_err(failure)?;
    let Some((assessment,endpoint,model,revision,context,status,raw,input,output))=row else{return Ok(None);};
    if !matches!(status.as_str(),"completed"|"incomplete"|"failed"){return Ok(None);}
    let details:Value=serde_json::from_str(&raw).map_err(failure)?;
    if !details.is_object()||ProviderId::from_endpoint(&endpoint).is_err()||model.is_empty()||revision.is_empty()||!crate::workflow::valid_hash(&context)||input.is_some_and(|n|n<0)||output.is_some_and(|n|n<0){return Err(recovery_error());}
    let not_sent=status=="failed"&&details["notDispatched"]==true;
    if not_sent{
        if input!=Some(0)||output!=Some(0)||details.get("providerStatus").is_some()||details.get("responseId").is_some(){return Err(recovery_error());}
    }else{
        let remote=details["providerStatus"].as_str();
        if !matches!(remote,Some("completed"|"incomplete"|"failed"|"cancelled")){return Ok(None);}
        if status=="completed"&&remote!=Some("completed")||status=="incomplete"&&remote!=Some("incomplete")||details["responseId"].as_str().is_none_or(|id|id.is_empty()||id.len()>256||!id.bytes().all(|b|b.is_ascii_graphic()))||!details["endedAt"].as_str().is_some_and(crate::workflow::valid_time){return Err(recovery_error());}
    }
    Ok(Some(TerminalRequest{assessment,endpoint,model,revision,context,status,details,not_sent}))
}
fn outcome(request:&TerminalRequest)->DataUseOutcome{if request.not_sent{DataUseOutcome::NotSent}else if request.status=="completed"{DataUseOutcome::Completed}else{DataUseOutcome::Failed}}

impl AiService{
    /// The caller holds inference and the active-investigation mutex. This hook
    /// is invoked only by startup recovery and never accepts renderer fields.
    pub(crate) fn recover_bound_terminal_work_locked(&self,store:&Store)->Result<bool,String>{
        let Some(candidate)=self.arbiter.direct_ai_terminal_candidate(store)?else{return Ok(false);};
        let _scope=store.scope_gate.lock().map_err(failure)?;
        let snapshot=store.snapshot()?;
        let Some(proof)=self.prove_direct_terminal(store,&snapshot,candidate)?else{return Ok(false);};
        self.arbiter.reconcile_ai_terminal(store,&proof)?;Ok(true)
    }
    fn prove_direct_terminal(&self,store:&Store,snapshot:&crate::model::Snapshot,candidate:UnboundDirectCandidate)->Result<Option<AiTerminalProof>,String>{
        let effect=candidate.effect.clone().ok_or_else(recovery_error)?;let mut receipts=vec![];
        match (&effect,candidate.owner.kind){
            (EffectRef::AiRequest{id},EffectOwnerKind::AiRequestIntent)=>{
                let Some(request)=terminal_request(self,id)?else{return Ok(None);};
                if request.assessment!=candidate.owner.assessment_id.as_deref().unwrap_or("connection-test")||request.details.get("investigationId").is_some(){return Err(recovery_error());}
                let found=snapshot.data_use_receipts.iter().filter(|r|r.effect.id==*id).collect::<Vec<_>>();
                if candidate.owner.assessment_id.is_none(){
                    if !found.is_empty()||request.details.get("dataUseReceiptRef").is_some()||request.details.get("dataReviewRef").is_some(){return Err(recovery_error());}
                }else{
                    if found.len()!=1{return Err(recovery_error());}let receipt=found[0];let last=receipt.events.last().ok_or_else(recovery_error)?.outcome;
                    let operation=if request.details.get("operationProposal").is_some(){DataOperation::AiOperationPropose}else{DataOperation::AiAsk};
                    if receipt.effect.kind!="request"||receipt.assessment_id!=candidate.owner.assessment_id||receipt.parent_receipt_ref.is_some()||receipt.request_index.is_some()||receipt.operation!=operation||receipt.destination!=DataDestination::RemoteAi||request.details["dataUseReceiptRef"]!=json!(receipt_reference(receipt))||request.details["dataReviewRef"]!=json!(receipt.review_ref)||request.details["scopeRef"]!=json!(receipt.scope_ref)||request.details["providerCredentialRef"]!=json!(receipt.credential)||receipt.credential.as_ref().is_none_or(|c|c.revision!=request.revision)||receipt.output_sha256!=request.context{return Err(recovery_error());}
                    if request.not_sent{if receipt.events.iter().any(|e|!matches!(e.outcome,DataUseOutcome::Admitted|DataUseOutcome::NotSent)){return Err(recovery_error());}}
                    else if !receipt.events.iter().any(|e|e.outcome==DataUseOutcome::Dispatched)||!matches!(last,DataUseOutcome::Dispatched|DataUseOutcome::Unknown|DataUseOutcome::Completed|DataUseOutcome::Failed)||last==DataUseOutcome::Completed&&request.status!="completed"||last==DataUseOutcome::Failed&&request.status=="completed"{return Err(recovery_error());}
                    receipts.push((receipt.clone(),outcome(&request)));
                }
            }
            (EffectRef::AiInvestigation{id},EffectOwnerKind::AiInvestigationIntent)=>{
                let mut plan=self.load_investigation(id)?;
                if plan.assessment_id!=candidate.owner.assessment_id.as_deref().ok_or_else(recovery_error)?||ProviderId::from_endpoint(&plan.endpoint).is_err(){return Err(recovery_error());}
                let found=snapshot.data_use_receipts.iter().filter(|r|r.effect.id==*id).collect::<Vec<_>>();if found.len()!=1{return Err(recovery_error());}let parent=found[0];
                if parent.effect.kind!="investigation"||parent.assessment_id!=candidate.owner.assessment_id||parent.operation!=DataOperation::AiInvestigation||parent.destination!=DataDestination::RemoteAi||parent.parent_receipt_ref.is_some()||parent.request_index.is_some()||Some(&parent.review_ref)!=plan.data_review.as_ref()||Some(&receipt_reference(parent))!=plan.data_use_receipt_ref.as_ref()||parent.scope_ref!=plan.scope_ref||parent.credential.as_ref().is_none_or(|c|c.revision!=plan.credential_revision)||parent.events.is_empty(){return Err(recovery_error());}
                // Require explicit provider termination (or explicit NotSent)
                // for every persisted request; 'failed' alone is insufficient.
                for step in &plan.results{
                    if step.status=="failed_before_dispatch"{continue;}
                    let Some(request)=terminal_request(self,&step.request_id)?else{return Ok(None);};
                    if request.assessment!=plan.assessment_id||request.endpoint!=plan.endpoint||request.model!=plan.model||request.revision!=plan.credential_revision{return Err(recovery_error());}
                }
                let children=self.inspect_recovery_children(&plan,snapshot,parent)?;
                let mut changed=false;
                for(step,child)in plan.results.iter_mut().zip(&children){if let Some(child)=child{
                    if matches!(step.status.as_str(),"pending"|"unknown"){
                        step.status=if child.not_sent{"failed_before_dispatch"}else if child.status=="incomplete"{"incomplete"}else{"failed"}.into();step.text.clear();step.error=Some("Término comprovado pelo journal anterior; texto ausente não foi recriado nem solicitado novamente.".into());step.finished_at=Some(model::now());step.usage=child.usage.clone();changed=true;
                    }
                    let receipt=snapshot.data_use_receipts.iter().find(|r|receipt_reference(r)==child.receipt).ok_or_else(recovery_error)?;
                    receipts.push((receipt.clone(),if child.not_sent{DataUseOutcome::NotSent}else if child.status=="completed"{DataUseOutcome::Completed}else{DataUseOutcome::Failed}));
                }}
                if matches!(plan.status.as_str(),"running"|"canceling"|"unknown"|"reconciled"){
                    plan.reconciled_from=Some(plan.status.clone());plan.status=if plan.results.iter().any(|s|s.status=="completed"&&!s.text.is_empty()){"partial"}else{"failed"}.into();plan.outcome_note="Término das solicitações anteriores comprovado pelos journals. Nenhuma perspectiva foi repetida; resultados e custos desconhecidos permanecem preservados.".into();changed=true;
                }
                if !matches!(plan.status.as_str(),"completed"|"partial"|"failed"|"canceled")||plan.status=="completed"&&(plan.results.len()!=plan.perspectives.len()||plan.results.iter().any(|s|s.status!="completed"||s.text.is_empty())){return Err(recovery_error());}
                if changed{self.save_investigation(&plan)?;}
                receipts.push((parent.clone(),match plan.status.as_str(){"completed"=>DataUseOutcome::Completed,"canceled"=>DataUseOutcome::Canceled,_=>DataUseOutcome::Failed}));
            }
            _=>return Err(recovery_error()),
        }
        Ok(Some(AiTerminalProof{owner:candidate.owner,boot_id:candidate.boot_id,effect,profile_fingerprint:crate::work_disposition::profile_fingerprint(&store.root)?,receipts}))
    }
}
