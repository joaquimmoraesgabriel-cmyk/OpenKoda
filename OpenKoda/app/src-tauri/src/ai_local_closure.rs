//! Human disposition of local ownership; remote journals and money never change.
use super::*;
use crate::{work_disposition as disposition,workflow::{EffectRef,WorkReviewRef}};
use disposition::{AiLocalClosureTarget,ClosureCandidate,QueueClosureSnapshot,LocalClosurePrepare,LocalClosureCommit,AiLocalDisposition};

#[derive(Clone)]
pub(super) struct PreparedClosure{review:WorkReviewRef,candidate:ClosureCandidate,journal_digest:String,profile_fingerprint:String,queue:QueueClosureSnapshot}
pub(crate) struct AiLocalClosureProof{effect:EffectRef,assessment_id:Option<String>,journal_digest:String,profile_fingerprint:String,unknown_requests:u32}
impl AiLocalClosureProof{
    pub(crate) fn journal_digest(&self)->&str{&self.journal_digest}
    pub(crate) fn profile_fingerprint(&self)->&str{&self.profile_fingerprint}
}
fn err()->String{crate::workflow::error("work_reconciliation_required")}
fn terminal_provider_status(request:&Value)->bool{matches!(request["details"]["providerStatus"].as_str(),Some("completed"|"incomplete"|"failed"|"cancelled"))}
fn read_request(db:&Connection,id:&str)->Result<Option<Value>,String>{
    let row:Option<(String,String,String,String,String,String,String,String,Option<i64>,Option<i64>)>=db.query_row("SELECT assessment,endpoint,model,revision,context_hash,status,created,details,input_tokens,output_tokens FROM requests WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).optional().map_err(failure)?;
    row.map(|(assessment,endpoint,model,revision,context,status,created,raw,input,output)|{
        if !matches!(status.as_str(),"pending"|"unknown"|"completed"|"incomplete"|"failed"){return Err(err());}
        let mut details:Value=serde_json::from_str(&raw).map_err(failure)?;let object=details.as_object_mut().ok_or_else(err)?;
        // Historical human acknowledgment is neither a remote observation nor
        // authority. It may be recorded later without changing the effect.
        object.remove("resolution");
        if let Some(proposal)=object.get_mut("operationProposal").and_then(Value::as_object_mut){if proposal.get("status").is_some_and(|v|v=="pending"||v=="interrupted"){proposal.insert("status".into(),json!("interrupted"));}}
        Ok(json!({"id":id,"assessment":assessment,"endpoint":endpoint,"model":model,"revision":revision,"contextHash":context,"status":if status=="pending"{"unknown"}else{status.as_str()},"createdAt":created,"details":details,"input":input,"output":output}))
    }).transpose()
}
fn inspect_journal(db:&Connection,effect:&EffectRef,profile_fingerprint:String)->Result<AiLocalClosureProof,String>{
    let (assessment_id,body,unknown_requests)=match effect{
        EffectRef::AiRequest{id}=>{
            let request=read_request(db,id)?.ok_or_else(err)?;
            if request["status"]!="unknown"||terminal_provider_status(&request)||request["details"].get("investigationId").is_some(){return Err(err());}
            let aid=request["assessment"].as_str().ok_or_else(err)?;
            ((aid!="connection-test").then(||aid.to_owned()),request,1)
        },
        EffectRef::AiInvestigation{id}=>{
            let(assessment,raw):(String,String)=db.query_row("SELECT assessment,data FROM investigations WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(failure)?;
            let mut parent:Value=serde_json::from_str(&raw).map_err(failure)?;super::validate_investigation_record(&parent)?;
            if parent["id"]!=*id||parent["assessmentId"]!=assessment{return Err(err());}
            let steps=parent["results"].as_array().ok_or_else(err)?;let mut requests=vec![];let mut unknown=0;
            for step in steps{
                let request_id=step["requestId"].as_str().ok_or_else(err)?;let request=read_request(db,request_id)?;
                if let Some(request)=&request{if request["assessment"]!=assessment||request["details"]["investigationId"]!=*id||request["details"]["perspective"]!=step["perspective"]{return Err(err());}if request["status"]=="unknown"{if terminal_provider_status(request){return Err(err());}unknown+=1;}}
                else if step["status"]=="pending"||step["status"]=="unknown"{unknown+=1;}
                requests.push(json!({"id":request_id,"record":request}));
            }
            let mut q=db.prepare("SELECT id FROM requests WHERE json_extract(details,'$.investigationId')=?1").map_err(failure)?;
            let linked=q.query_map([id],|r|r.get::<_,String>(0)).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?;
            if linked.iter().any(|request_id|!requests.iter().any(|r|r["id"]==*request_id))||unknown==0{return Err(err());}
            let object=parent.as_object_mut().ok_or_else(err)?;object.remove("outcomeNote");object.remove("reconciledFrom");
            if object.get("status").is_some_and(|v|v=="running"||v=="canceling"||v=="unknown"||v=="reconciled"){object.insert("status".into(),json!("unknown"));}
            for step in object.get_mut("results").and_then(Value::as_array_mut).ok_or_else(err)?{let step=step.as_object_mut().ok_or_else(err)?;step.remove("error");if step.get("status").is_some_and(|v|v=="pending"){step.insert("status".into(),json!("unknown"));}}
            (Some(assessment),json!({"parent":parent,"requests":requests}),unknown)
        },_=>return Err(err()),
    };
    Ok(AiLocalClosureProof{effect:effect.clone(),assessment_id,journal_digest:digest(&json!({"schema":"openkoda.ai-remote-unknown-identity.v1","body":body}).to_string()),profile_fingerprint,unknown_requests})
}

fn root_effect(store:&Store,target:&AiLocalClosureTarget)->Result<EffectRef,String>{match target{
    AiLocalClosureTarget::AiRequest{request_id}=>{crate::core::valid_id(request_id)?;Ok(EffectRef::AiRequest{id:request_id.clone()})},
    AiLocalClosureTarget::AiInvestigation{investigation_id}=>{crate::core::valid_id(investigation_id)?;Ok(EffectRef::AiInvestigation{id:investigation_id.clone()})},
    AiLocalClosureTarget::Attempt{attempt_id}=>store.work_read(|conn,_|{let attempt=crate::work_store::attempt(conn,attempt_id)?;if attempt.view.backend!=crate::workflow::Backend::Ai{return Err(err());}attempt.view.effect.ok_or_else(err)}),
}}

impl AiService{
    fn inspect_local_closure(&self,store:&Store,target:&AiLocalClosureTarget)->Result<(AiLocalClosureProof,ClosureCandidate),String>{
        let effect=root_effect(store,target)?;let profile=disposition::profile_fingerprint(&store.root)?;
        let proof=inspect_journal(&*self.db.lock().map_err(failure)?,&effect,profile)?;
        let candidate=self.arbiter.inspect_ai_local_closure(store,target,proof.assessment_id.as_deref())?;
        if candidate.effect!=proof.effect{return Err(err());}
        // A receipt copied into another profile remains historical. This exact
        // profile may not close the same identity twice with a different key.
        if store.work_read(|conn,_|Ok(disposition::records(conn)?.iter().any(|record|record.owner_profile_fingerprint==proof.profile_fingerprint&&record.candidate.effect==effect&&record.journal_digest==proof.journal_digest)))?{return Err(crate::workflow::error("work_review_used"));}
        Ok((proof,candidate))
    }
    pub(crate) fn handle_local_closure(&self,action:&str,payload:Value,store:&Store,invalidate_work_reviews:impl FnOnce()->Result<(),String>)->Result<Value,String>{
        if action=="work.aiLocalClosure.commit"{
            let input:LocalClosureCommit=crate::workflow::decode(payload)?;crate::core::valid_id(&input.idempotency_key)?;
            if !input.acknowledge_remote_unknown||!input.acknowledge_no_replay||!input.acknowledge_queue_invalidation||!crate::workflow::plain_text(&input.note,256)||input.note.len()>1024{return Err(crate::workflow::error("work_invalid_request"));}
            let redactor=self.data_redactor();use crate::data_projection::DataRedactor;let redacted=redactor.redact(&input.note);
            if redactor.mode()==crate::data_projection::RedactionMode::MetadataOnly||redacted.changed||redacted.secret_material||redactor.contains_managed_secret(&input.note){return Err(crate::workflow::error("data_secret_material"));}
            let request_digest=crate::workflow::digest(&json!({"action":action,"payload":input}))?;
            if let Some(mut value)=store.work_read(|conn,_|disposition::replay(conn,&input.idempotency_key,&request_digest))?{value["value"]["appliesToCurrentProfile"]=json!(self.local_closure_applies(store,value["receipt"]["id"].as_str().ok_or_else(err)?)?);return Ok(value);}
            let prepared=self.local_closure_reviews.lock().map_err(failure)?.get(&input.review.id).cloned().ok_or_else(||crate::workflow::error("work_review_expired"))?;
            if prepared.review!=input.review||chrono::DateTime::parse_from_rfc3339(&input.review.expires_at).map_err(failure)?<=chrono::Utc::now(){return Err(crate::workflow::error("work_review_expired"));}
            let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;let active=self.investigation_active.lock().map_err(failure)?;if active.is_some(){return Err("work_busy".into());}
            let(proof,current)=self.inspect_local_closure(store,&prepared.candidate.target)?;
            let permit=self.arbiter.claim_ai_local_closure(store,&prepared.candidate.target,proof.assessment_id.as_deref())?;
            let queue=store.work_read(disposition::queue_snapshot)?;
            if current!=prepared.candidate||permit.candidate()!=&current||proof.journal_digest!=prepared.journal_digest||proof.profile_fingerprint!=prepared.profile_fingerprint||queue!=prepared.queue{return Err(crate::workflow::error("work_revision_changed"));}
            let mut record=AiLocalDisposition{schema:"openkoda.ai-local-disposition.v1".into(),id:model::id(),digest:String::new(),created_at:model::now(),author:"local-user".into(),note:input.note,owner_profile_fingerprint:proof.profile_fingerprint.clone(),candidate:current,journal_digest:proof.journal_digest.clone(),queue,review:input.review.clone(),idempotency_key:input.idempotency_key,request_digest,local_ownership_closed:true,remote_outcome_unknown:true,financial_disposition:"unchanged".into()};record.digest=disposition::record_digest(&record)?;
            if serde_json::to_vec(&record).map_err(failure)?.len()>disposition::MAX_RECORD_BYTES{return Err(crate::workflow::error("work_limit"));}
            // Revocation precedes the canonical commit, under the host's
            // exclusive gate. Failure can lose reviews but never grant work.
            self.data_policy.invalidate_pending_reviews()?;self.data_reviews.lock().map_err(failure)?.clear();self.budget.invalidate_pending_reviews()?;invalidate_work_reviews()?;
            let _scope=store.scope_gate.lock().map_err(failure)?;
            let mut result=store.work_transaction(|conn,s|disposition::commit_locked(conn,s,&permit,&proof,&record))?;result["value"]["appliesToCurrentProfile"]=json!(true);
            self.local_closure_reviews.lock().map_err(failure)?.remove(&input.review.id);return Ok(result);
        }
        if action=="work.aiLocalClosure.prepare"{
            let input:LocalClosurePrepare=crate::workflow::decode(payload)?;
            let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;let active=self.investigation_active.lock().map_err(failure)?;if active.is_some(){return Err("work_busy".into());}
            let(proof,candidate)=self.inspect_local_closure(store,&input.target)?;let queue=store.work_read(disposition::queue_snapshot)?;
            let review=WorkReviewRef{id:model::id(),digest:crate::workflow::digest(&json!({"candidate":candidate,"journalDigest":proof.journal_digest,"profile":proof.profile_fingerprint,"queue":queue}))?,expires_at:(chrono::Utc::now()+chrono::Duration::minutes(5)).to_rfc3339()};
            let prepared=PreparedClosure{review:review.clone(),candidate:candidate.clone(),journal_digest:proof.journal_digest,profile_fingerprint:proof.profile_fingerprint,queue:queue.clone()};
            let mut reviews=self.local_closure_reviews.lock().map_err(failure)?;reviews.retain(|_,p|chrono::DateTime::parse_from_rfc3339(&p.review.expires_at).is_ok_and(|t|t>chrono::Utc::now()));if reviews.len()>=32{return Err(crate::workflow::error("work_limit"));}reviews.insert(review.id.clone(),prepared);
            return Ok(json!({"review":review,"target":input.target,"effect":candidate.effect,"assessmentId":candidate.assessment_id,"attemptId":candidate.job.as_ref().map(|j|&j.attempt_id),"jobId":candidate.job.as_ref().map(|j|&j.job_id),"unknownRequests":proof.unknown_requests,"queue":queue,"notice":disposition::PREPARATION_NOTICE}));
        }
        if action!="work.aiLocalClosure.list"||!payload.as_object().is_some_and(|v|v.is_empty()){return Err(crate::workflow::error("work_invalid_request"));}
        let records=store.work_read(|conn,_|disposition::records(conn))?;let mut targets=vec![];
        {
            let db=self.db.lock().map_err(failure)?;let mut q=db.prepare("SELECT id,details FROM requests WHERE status IN ('unknown','pending') ORDER BY created,id").map_err(failure)?;
            let rows=q.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?;
            for(id,raw)in rows{let details:Value=serde_json::from_str(&raw).map_err(failure)?;let target=if let Some(parent)=details["investigationId"].as_str(){AiLocalClosureTarget::AiInvestigation{investigation_id:parent.into()}}else{AiLocalClosureTarget::AiRequest{request_id:id}};if !targets.contains(&target){targets.push(target);}}
            let mut q=db.prepare("SELECT id FROM investigations WHERE json_extract(data,'$.status') IN ('running','canceling','unknown','reconciled')").map_err(failure)?;
            for id in q.query_map([],|row|row.get::<_,String>(0)).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?{let target=AiLocalClosureTarget::AiInvestigation{investigation_id:id};if !targets.contains(&target){targets.push(target);}}
        }
        let _inference=self.inference.try_lock().map_err(|_|"work_busy")?;let active=self.investigation_active.lock().map_err(failure)?;if active.is_some(){return Err("work_busy".into());}
        let mut candidates=vec![];
        for mut target in targets{
            if let AiLocalClosureTarget::AiInvestigation{investigation_id}=&target{if let Some(attempt)=store.work_read(|conn,_|{let Some(owner)=crate::work_store::investigation_owner_locked(conn,investigation_id)? else{return Ok(None)};let job=crate::work_store::job(conn,&owner.job_id)?;Ok(job.current_attempt.map(|a|a.id))})?{target=AiLocalClosureTarget::Attempt{attempt_id:attempt};}}
            let effect=root_effect(store,&target)?;
            match self.inspect_local_closure(store,&target){Ok((proof,_))=>candidates.push(json!({"target":target,"effect":effect,"assessmentId":proof.assessment_id,"eligible":true,"reasonCode":null})),Err(reason)=>{let code=reason.split(':').next().unwrap_or("work_reconciliation_required");if code!="work_review_used"{candidates.push(json!({"target":target,"effect":effect,"assessmentId":null,"eligible":false,"reasonCode":if crate::workflow::known_reason_code(code){code}else{"work_reconciliation_required"}}));}}}
        }
        let profile=disposition::profile_fingerprint(&store.root)?;
        let items=records.iter().map(|record|{let mut value=disposition::public_record(record,false)["value"].clone();value["appliesToCurrentProfile"]=json!(record.owner_profile_fingerprint==profile&&inspect_journal(&*self.db.lock().map_err(failure)?,&record.candidate.effect,profile.clone()).is_ok_and(|proof|proof.journal_digest==record.journal_digest));Ok(value)}).collect::<Result<Vec<_>,String>>()?;Ok(json!({"items":items,"candidates":candidates,"notice":disposition::PREPARATION_NOTICE}))
    }
    fn local_closure_applies(&self,store:&Store,id:&str)->Result<bool,String>{let profile=disposition::profile_fingerprint(&store.root)?;let record=store.work_read(|conn,_|Ok(disposition::records(conn)?.into_iter().find(|r|r.id==id)))?.ok_or_else(err)?;Ok(record.owner_profile_fingerprint==profile&&inspect_journal(&*self.db.lock().map_err(failure)?,&record.candidate.effect,profile).is_ok_and(|proof|proof.journal_digest==record.journal_digest))}
}

/// Metadata-only coverage for the arbiter's read-only journal gate. A copied,
/// changed, missing or acknowledged journal cannot inherit another receipt.
pub(super) fn covered_roots(root:&std::path::Path,db:&Connection)->Result<Vec<EffectRef>,String>{
    let records=disposition::read_records(root)?;let profile=disposition::profile_fingerprint(root)?;let mut roots=vec![];
    for record in records{
        if record.owner_profile_fingerprint!=profile||record.digest!=disposition::record_digest(&record)?{continue;}
        if let Ok(proof)=inspect_journal(db,&record.candidate.effect,profile.clone()){if proof.journal_digest==record.journal_digest&&proof.assessment_id==record.candidate.assessment_id{roots.push(record.candidate.effect);}}
    }Ok(roots)
}
