//! Pure preparation of original evidence transfers to the owned Kali runtime.
//! This module never starts a process, reads a target path or changes evidence.
use super::*;
use crate::data_policy::{DataCategory,DataDestination,DataOperation,DataOriginRef,DataReviewRef,DataSelection,DataSubject,DataUseSpec,RequestFingerprint,Sensitivity,SubjectVersion};
use crate::data_projection::{DataRedactor,RedactionMode};

#[derive(Clone)] pub(super) struct PreparedKaliData {pub review:DataReviewRef,pub spec:DataUseSpec,pub assessment_id:String,pub evidence_id:String,pub operation:String,pub environment_id:String,pub package_sensitivity:Sensitivity,metadata_bytes:usize}
pub(super) struct KaliAdmission<'a>{pub prepared:PreparedKaliData,pub acknowledge_sensitive:bool,pub ai:&'a crate::ai::AiService}
struct CanonicalOnly;
impl crate::data_projection::ExternalSubjectResolver for CanonicalOnly {
    fn resolve(&self,_:&DataSubject)->Result<crate::data_projection::ResolvedDataSubject,String>{Err("data_source_unknown".into())}
    fn list(&self,_:&str)->Result<Vec<DataSubject>,String>{Ok(vec![])}
}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Prepare{assessment_id:String,evidence_id:String,operation:String,package_sensitivity:Sensitivity}

impl KaliBridge {
    pub fn with_data_policy(mut self,service:Arc<crate::data_policy::DataPolicyService>)->Self{self.data_policy=service;self}
    pub fn handle_data_prepare(&self,action:&str,payload:Value,store:&Store,environment_id:&str,redactor:&dyn DataRedactor)->Result<Value,String>{
        if action!="kali.data.prepare"{return Err("Ação de dados Kali desconhecida.".into());}
        let input:Prepare=decode(payload)?;
        let spec=self.kali_data_spec(store,&input.assessment_id,&input.evidence_id,&input.operation,environment_id,input.package_sensitivity,redactor)?;
        let review=self.data_policy.prepare_use(store,spec.clone())?;
        let result=json!({"dataReview":if review.decision.allowed{json!(review.review_ref)}else{Value::Null},"rawSha256":spec.requests[0].input_sha256,"rawBytes":spec.requests[0].input_bytes,"origins":spec.selection.origins,"decision":review.decision,"canTransfer":review.decision.allowed,"reasonCodes":review.decision.reason_codes,"requiresSensitiveConfirmation":review.requires_sensitive_confirmation,"representation":"original_evidence","notice":"Somente os bytes originais desta evidência serão transferidos ao Kali gerido; não serão executados."});
        if review.decision.allowed {
            let entry_bytes=serde_json::to_vec(&spec).map_err(db_error)?.len().saturating_add(1024);
            let mut reviews=self.data_reviews.lock().map_err(db_error)?;
            reviews.retain(|_,p|chrono::DateTime::parse_from_rfc3339(&p.review.expires_at).is_ok_and(|t|t>chrono::Utc::now()));
            let metadata_bytes=reviews.values().map(|p|p.metadata_bytes).fold(0usize,usize::saturating_add).saturating_add(entry_bytes);
            if reviews.len()>=128||metadata_bytes>8*1024*1024{let _=self.data_policy.cancel_review(&review.review_ref);return Err("data_limit: há muitas prévias locais abertas.".into());}
            reviews.insert(review.review_ref.id.clone(),PreparedKaliData{review:review.review_ref,spec,assessment_id:input.assessment_id,evidence_id:input.evidence_id,operation:input.operation,environment_id:environment_id.into(),package_sensitivity:input.package_sensitivity,metadata_bytes:entry_bytes});
        }
        Ok(result)
    }
    pub fn admit_data_before_identity(&self,action:&str,payload:&Value,store:&Store,redactor:&dyn DataRedactor)->Result<(),String>{
        let (aid,eid,op,review,ack)=match action {
            "kali.mcp.hashEvidence"=>{let p:HashInput=decode(payload.clone())?;(p.assessment_id,p.evidence_id,"hash",p.data_review,p.acknowledge_local_transfer)},
            "kali.mcp.scanEvidence"=>{let p:ScanInput=decode(payload.clone())?;(p.assessment_id,p.evidence_id,"scan",p.data_review,p.acknowledge_local_transfer)},
            _=>return Ok(()),
        };
        if !ack{return Err("Confirme a transferência local da evidência.".into());}
        let reference=review.as_ref().ok_or("data_review_required: revise o pacote de transferência local.")?;
        let environment=self.data_reviews.lock().map_err(db_error)?.get(&reference.id).map(|p|p.environment_id.clone()).ok_or("data_review_expired: prepare novamente a transferência.")?;
        self.reviewed_kali_data(store,Some(reference),&aid,&eid,op,&environment,redactor).map(|_|())
    }
    pub(super) fn reviewed_kali_data(&self,store:&Store,review:Option<&DataReviewRef>,aid:&str,eid:&str,operation:&str,environment:&str,redactor:&dyn DataRedactor)->Result<PreparedKaliData,String>{
        let review=review.ok_or("data_review_required: revise o pacote local.")?;
        let prepared=self.data_reviews.lock().map_err(db_error)?.get(&review.id).cloned().ok_or("data_review_expired: prepare novamente o pacote local.")?;
        if prepared.review!=*review||prepared.assessment_id!=aid||prepared.evidence_id!=eid||prepared.operation!=operation||prepared.environment_id!=environment{return Err("data_review_changed: seleção local mudou.".into());}
        if chrono::DateTime::parse_from_rfc3339(&review.expires_at).map_err(db_error)?<=chrono::Utc::now(){return Err("data_review_expired: prepare novamente o pacote local.".into());}
        self.recheck_kali_data(store,&prepared,redactor)?;
        self.data_policy.check_review(store,review,&prepared.spec)?;Ok(prepared)
    }
    pub(super) fn recheck_kali_data(&self,store:&Store,prepared:&PreparedKaliData,redactor:&dyn DataRedactor)->Result<(),String>{
        let current=self.kali_data_spec(store,&prepared.assessment_id,&prepared.evidence_id,&prepared.operation,&prepared.environment_id,prepared.package_sensitivity,redactor)?;
        if current!=prepared.spec{return Err("data_review_changed: bytes, classe ou escopo local mudou.".into());}
        let decision=crate::data_policy::DataPolicyService::check_use(&store.snapshot()?,&current)?;
        if !decision.allowed{return Err(format!("{}: transferência local recusada antes dos bytes.",decision.reason_codes.first().map(String::as_str).unwrap_or("data_policy_changed")));}
        Ok(())
    }
    fn kali_data_spec(&self,store:&Store,aid:&str,eid:&str,operation:&str,environment:&str,sensitivity:Sensitivity,redactor:&dyn DataRedactor)->Result<DataUseSpec,String>{
        valid_id(environment)?;valid_id(aid)?;valid_id(eid)?;
        if redactor.mode()==RedactionMode::MetadataOnly{return Err("data_redactor_unavailable: proteção local indisponível.".into());}
        let (operation,tool)=match operation{"hash"=>(DataOperation::KaliHashEvidence,"hash_bytes"),"scan"=>(DataOperation::KaliScanEvidence,"scan_bytes"),_=>return Err("Operação local de dados desconhecida.".into())};
        let state=store.snapshot()?;let a=state.assessments.iter().find(|a|a.id==aid).ok_or("Avaliação não encontrada.")?;
        let e=a.evidence.iter().find(|e|e.id==eid).ok_or("Evidência fora da avaliação.")?;
        if hash(&e.content)!=e.sha256||e.content.len()>MAX_BYTES||(tool=="scan_bytes"&&e.content.contains('\0')){return Err("Evidência íntegra de até 32 KiB exigida; análise não aceita NUL.".into());}
        if redactor.redact(&e.content).secret_material||redactor.redact(&e.name).secret_material{return Err("data_secret_material: bytes originais com segredo reconhecido não podem ser transferidos.".into());}
        let targets=source_refs(store,aid,Some(eid))?;
        let reference=crate::scope::current_ref(&state,aid)?.ok_or("scope_unreviewed")?;
        crate::scope::check_selected_work(&state,aid,scope_method(tool)?,Some(&reference),&targets)?;
        let origins=if crate::scope_targets::is_combined(&state,aid){targets.iter().map(|r|DataOriginRef{scope_ref:reference.clone(),target_ref:Some(r.clone())}).collect::<Vec<_>>()}else{vec![DataOriginRef{scope_ref:reference.clone(),target_ref:None}]};
        let category=if crate::source_context::is_source_capture(e,a,&state.plans){DataCategory::SourceExcerpt}else{DataCategory::EvidenceExcerpt};
        let mut spec=DataUseSpec::new(operation,DataDestination::LocalKali,vec![a.project_id.clone()],Some(aid.into()));
        spec.scope_ref=Some(reference);
        spec.selection=DataSelection{categories:vec![category],origins:origins.clone(),evidence_ids:vec![eid.into()],derived_subjects:vec![],package_sensitivity:sensitivity};
        let mut subjects=vec![DataSubject::Evidence{assessment_id:aid.into(),evidence_id:eid.into()}];
        for origin in origins{subjects.push(DataSubject::ScopeSource{assessment_id:aid.into(),scope_ref:origin.scope_ref,target_ref:origin.target_ref});}
        for subject in subjects{let resolved=crate::data_projection::resolve_subject(store,&state,&subject,&CanonicalOnly)?;crate::data_projection::require_known_origin(&resolved)?;let classification=crate::data_projection::classification_state(&state,&resolved)?;spec.subjects.push(SubjectVersion{subject,source_fingerprint:resolved.fingerprint,classification_ref:classification.classification_ref});}
        spec.destination_context=json!({"environmentId":environment,"tool":tool,"evidenceId":eid,"rawSha256":e.sha256});
        spec.requests=vec![RequestFingerprint{index:0,input_sha256:e.sha256.clone(),output_sha256:e.sha256.clone(),input_bytes:e.content.len() as u64,output_bytes:e.content.len() as u64}];
        Ok(spec)
    }
}
