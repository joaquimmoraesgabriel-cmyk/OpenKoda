//! Host projections: original bytes stay in canonical storage, never in DTOs.
use crate::{core::{self,Store},model::Snapshot,scope,scope_targets,data_policy::{self,DataSubject,DataCategory,DataOriginRef,DerivedProducer,ClassificationRef,Sensitivity,error,hash}};
pub use crate::data_policy::SubjectFingerprint;
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="snake_case")]
pub enum RedactionMode{Ready,NoCredentialConfigured,MetadataOnly}
pub struct RedactedText{pub text:String,pub changed:bool,pub secret_material:bool}
pub trait DataRedactor{fn mode(&self)->RedactionMode;fn redact(&self,text:&str)->RedactedText;fn contains_managed_secret(&self,_text:&str)->bool{false}}
pub struct ResolvedDataSubject{pub subject:DataSubject,pub project_id:String,pub assessment_id:String,pub fingerprint:String,pub categories:Vec<DataCategory>,pub origins:Vec<DataOriginRef>,pub origin_verified:bool,pub contributors:Vec<SubjectFingerprint>,pub value:Value}
pub trait ExternalSubjectResolver{
    fn resolve(&self,subject:&DataSubject)->Result<ResolvedDataSubject,String>;
    fn list(&self,assessment_id:&str)->Result<Vec<DataSubject>,String>;
    /// The default preserves the reviewed representation of existing producers.
    /// Capture overrides it with the complete verified stream before redaction.
    fn access_text(&self,resolved:&ResolvedDataSubject)->Result<String,String>{
        serde_json::to_string_pretty(&resolved.value).map_err(|_|error("data_projection_failed"))
    }
}
pub struct RejectExternal;
impl ExternalSubjectResolver for RejectExternal{fn resolve(&self,_:&DataSubject)->Result<ResolvedDataSubject,String>{Err(error("data_subject_unavailable"))}fn list(&self,_:&str)->Result<Vec<DataSubject>,String>{Ok(vec![])}}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ClassificationState{pub sensitivity:Sensitivity,pub classification_ref:Option<ClassificationRef>,pub source_fingerprint:String,pub secret_material:bool,pub contributor_refs:Vec<ClassificationRef>,pub reason_codes:Vec<String>}
pub type SnapshotView=Value;
pub type EvidenceView=Value;

pub fn origin_subject(reference:&DataOriginRef)->DataSubject{DataSubject::ScopeSource{assessment_id:reference.scope_ref.assessment_id.clone(),scope_ref:reference.scope_ref.clone(),target_ref:reference.target_ref.clone()}}
pub fn require_known_origin(resolved:&ResolvedDataSubject)->Result<(),String>{
    if matches!(resolved.subject,DataSubject::Evidence{..}|DataSubject::Derived{..})&&!resolved.origin_verified&&resolved.origins.is_empty(){return Err(error("data_source_unknown"));}Ok(())
}
fn apply_data_association(s:&Snapshot,resolved:&mut ResolvedDataSubject)->Result<(),String>{
    if resolved.origin_verified||!resolved.origins.is_empty(){return Ok(());}
    let Some(association)=data_policy::current_classification(s,&resolved.subject).filter(|c|c.source_fingerprint==resolved.fingerprint).and_then(|c|c.association.as_ref())else{return Ok(());};
    // A user association is prospective data metadata only. It never replaces
    // a known producer origin, and never becomes operational evidence lineage.
    let mut sources=vec![];
    for origin in &association.refs{
        if scope::current_ref(s,&resolved.assessment_id)?.as_ref()!=Some(&origin.scope_ref)||scope_targets::resolve_current_target(s,&resolved.assessment_id,origin.target_ref.as_ref()).is_err(){return Ok(());}
        let subject=origin_subject(origin);let fingerprint=canonical_fingerprint(s,&subject)?.ok_or_else(||error("data_source_unknown"))?;sources.push(SubjectFingerprint{subject,fingerprint});
    }
    resolved.origins=association.refs.clone();resolved.origin_verified=false;
    for source in sources{if !resolved.contributors.iter().any(|c|c.subject==source.subject){resolved.contributors.push(source);}}Ok(())
}
pub fn canonical_fingerprint(s:&Snapshot,subject:&DataSubject)->Result<Option<String>,String>{Ok(canonical_fingerprint_raw(s,subject)?.map(|fingerprint|versioned_fingerprint(s,subject,fingerprint)))}
fn versioned_fingerprint(s:&Snapshot,subject:&DataSubject,fingerprint:String)->String{
    let generation=s.data_subject_generations.iter().find(|g|g.subject==*subject).map_or(0,|g|g.generation);
    if generation==0{fingerprint}else{hash(&json!({"schema":"openkoda.data-subject-version.v1","fingerprint":fingerprint,"generation":generation}))}
}
pub(crate) fn generation_subject(subject:&DataSubject)->bool{matches!(subject,DataSubject::AssessmentMetadata{category:DataCategory::HumanText|DataCategory::ExecutionSummary|DataCategory::ActivityMetadata,..}|DataSubject::Evidence{..}|DataSubject::Derived{producer:DerivedProducer::Finding|DerivedProducer::Validation,..})}
pub(crate) fn canonical_generation_inputs(s:&Snapshot)->Result<Vec<(DataSubject,String)>,String>{
    let mut subjects=vec![];
    for a in &s.assessments{for category in [DataCategory::HumanText,DataCategory::ExecutionSummary,DataCategory::ActivityMetadata]{subjects.push(DataSubject::AssessmentMetadata{assessment_id:a.id.clone(),category});}for e in &a.evidence{subjects.push(DataSubject::Evidence{assessment_id:a.id.clone(),evidence_id:e.id.clone()});}}
    for f in &s.findings{subjects.push(DataSubject::Derived{assessment_id:f.assessment_id.clone(),producer:DerivedProducer::Finding,object_id:f.id.clone()});}
    for v in &s.validations{subjects.push(DataSubject::Derived{assessment_id:v.assessment_id.clone(),producer:DerivedProducer::Validation,object_id:v.id.clone()});}
    subjects.into_iter().map(|subject|{let fingerprint=canonical_fingerprint_raw(s,&subject)?.ok_or_else(||error("data_subject_changed"))?;Ok((subject,fingerprint))}).collect()
}
pub(crate) fn advance_subject_generations(before:Vec<(DataSubject,String)>,s:&mut Snapshot)->Result<(),String>{
    let after=canonical_generation_inputs(s)?;
    let old=before.iter().map(|(subject,fingerprint)|(hash(subject),fingerprint)).collect::<std::collections::HashMap<_,_>>();
    let new=after.iter().map(|(subject,fingerprint)|(hash(subject),fingerprint)).collect::<std::collections::HashMap<_,_>>();
    let mut visited=std::collections::HashSet::new();
    for (subject,_) in before.iter().chain(after.iter()){
        let key=hash(subject);if !visited.insert(key.clone())||old.get(&key)==new.get(&key)||!s.assessments.iter().any(|a|a.id==subject.assessment_id()){continue;}
        if let Some(g)=s.data_subject_generations.iter_mut().find(|g|g.subject==*subject){g.generation=g.generation.checked_add(1).ok_or_else(||error("data_limit"))?;}
        else if old.contains_key(&key){s.data_subject_generations.push(data_policy::DataSubjectGeneration{subject:subject.clone(),generation:1});}
    }
    s.data_subject_generations.retain(|g|s.assessments.iter().any(|a|a.id==g.subject.assessment_id()));Ok(())
}
fn canonical_fingerprint_raw(s:&Snapshot,subject:&DataSubject)->Result<Option<String>,String>{
    let aid=subject.assessment_id();let a=s.assessments.iter().find(|a|a.id==aid).ok_or_else(||error("data_subject_missing"))?;let p=s.projects.iter().find(|p|p.id==a.project_id).ok_or_else(||error("data_project_missing"))?;
    let value=match subject{
        DataSubject::AssessmentMetadata{category:DataCategory::HumanText,..}=>json!({"projectName":p.name,"projectDescription":p.description,"title":a.title,"objective":a.objective,"summary":a.summary}),
        DataSubject::AssessmentMetadata{category:DataCategory::ExecutionSummary,..}=>json!({"executions":s.executions.iter().filter(|r|r.assessment_id==aid).collect::<Vec<_>>(),"coverage":a.coverage}),
        DataSubject::AssessmentMetadata{category:DataCategory::ActivityMetadata,..}=>json!({"events":a.events}),
        DataSubject::AssessmentMetadata{..}=>return Err(error("data_invalid_subject")),
        DataSubject::ScopeSource{scope_ref,target_ref,..}=>source_value(s,&DataOriginRef{scope_ref:scope_ref.clone(),target_ref:target_ref.clone()})?,
        DataSubject::Evidence{evidence_id,..}=>{let e=a.evidence.iter().find(|e|e.id==*evidence_id).ok_or_else(||error("data_subject_missing"))?;if core::hash(&e.content)!=e.sha256{return Err(error("data_subject_changed"));}json!({"id":e.id,"name":e.name,"rawSha256":e.sha256,"rawBytes":e.content.len()})},
        DataSubject::Derived{producer:DerivedProducer::Finding,object_id,..}=>serde_json::to_value(s.findings.iter().find(|f|f.id==*object_id&&f.assessment_id==aid).ok_or_else(||error("data_subject_missing"))?).map_err(|_|error("data_projection_failed"))?,
        DataSubject::Derived{producer:DerivedProducer::Validation,object_id,..}=>serde_json::to_value(s.validations.iter().find(|f|f.id==*object_id&&f.assessment_id==aid).ok_or_else(||error("data_subject_missing"))?).map_err(|_|error("data_projection_failed"))?,
        DataSubject::Derived{..}=>return Ok(None),
    };Ok(Some(hash(&value)))
}
fn source_value(s:&Snapshot,reference:&DataOriginRef)->Result<Value,String>{
    let aid=&reference.scope_ref.assessment_id;
    let body=if scope::current_ref(s,aid)?.as_ref()==Some(&reference.scope_ref){scope_targets::current_body(s,aid)?}else{scope::validate_historical_ref(s,aid,&reference.scope_ref)?;&s.scope_revisions.iter().find(|r|r.id==reference.scope_ref.revision_id).ok_or_else(||error("data_source_unknown"))?.body};
    let target=scope_targets::resolve_body(body,reference.target_ref.as_ref())?;
    Ok(json!({"origin":reference,"kind":target.kind,"target":target.target,"label":target.label,"methods":target.methods,"localExclusions":target.local_exclusions,"connectionExclusions":target.connection_exclusions}))
}
pub fn evidence_origins(store:&Store,s:&Snapshot,aid:&str,eid:&str)->Result<(Vec<DataOriginRef>,bool),String>{
    if scope_targets::is_combined(s,aid){
        if let Ok(refs)=crate::evidence_lineage::evidence_refs(store,s,aid,&[eid.into()]){let reference=scope::current_ref(s,aid)?.ok_or_else(||error("data_source_unknown"))?;let verified=s.assessments.iter().find(|a|a.id==aid).and_then(|a|a.evidence.iter().find(|e|e.id==eid)).and_then(|e|serde_json::from_str::<Value>(&e.content).ok()).is_none_or(|raw|raw["schema"]!="openkoda.manual-evidence.v2");return Ok((refs.into_iter().map(|target_ref|DataOriginRef{scope_ref:reference.clone(),target_ref:Some(target_ref)}).collect(),verified));}
    }else if let Some(plan)=s.executions.iter().find(|e|e.assessment_id==aid&&e.evidence_id.as_deref()==Some(eid)).and_then(|e|s.plans.iter().find(|p|p.id==e.plan_id&&p.assessment_id==aid)){
        if let Some(reference)=&plan.scope_ref{scope::validate_historical_ref(s,aid,reference)?;return Ok((vec![DataOriginRef{scope_ref:reference.clone(),target_ref:None}],true));}
    }
    let subject=DataSubject::Evidence{assessment_id:aid.into(),evidence_id:eid.into()};
    if let Some(association)=data_policy::current_classification(s,&subject).filter(|r|canonical_fingerprint(s,&subject).ok().flatten().as_ref()==Some(&r.source_fingerprint)).and_then(|r|r.association.as_ref()){if association.refs.iter().all(|r|scope::current_ref(s,aid).ok().flatten().as_ref()==Some(&r.scope_ref)&&source_value(s,r).is_ok()){return Ok((association.refs.clone(),false));}}
    Ok((vec![],false))
}
pub fn resolve_subject(store:&Store,s:&Snapshot,subject:&DataSubject,external:&dyn ExternalSubjectResolver)->Result<ResolvedDataSubject,String>{
    let aid=subject.assessment_id();core::valid_id(aid)?;
    let a=s.assessments.iter().find(|a|a.id==aid).ok_or_else(||error("data_subject_missing"))?;
    let p=s.projects.iter().find(|p|p.id==a.project_id).ok_or_else(||error("data_project_missing"))?;
    let mut origins=vec![];let mut contributors=vec![];let mut origin_verified=true;
    let (categories,value,fingerprint)=match subject{
        DataSubject::AssessmentMetadata{category,..}=>{let v=match category{
            DataCategory::HumanText=>json!({"projectName":p.name,"projectDescription":p.description,"title":a.title,"objective":a.objective,"summary":a.summary}),
            DataCategory::ExecutionSummary=>json!({"executions":s.executions.iter().filter(|r|r.assessment_id==aid).collect::<Vec<_>>(),"coverage":a.coverage}),
            DataCategory::ActivityMetadata=>json!({"events":a.events}),_=>return Err(error("data_invalid_subject"))};(vec![*category],v.clone(),hash(&v))},
        DataSubject::ScopeSource{scope_ref,target_ref,..}=>{if scope_ref.assessment_id!=aid{return Err(error("data_source_unknown"));}let r=DataOriginRef{scope_ref:scope_ref.clone(),target_ref:target_ref.clone()};let v=source_value(s,&r)?;origins.push(r);(vec![DataCategory::TargetMetadata],v.clone(),hash(&v))},
        DataSubject::Evidence{evidence_id,..}=>{let e=a.evidence.iter().find(|e|e.id==*evidence_id).ok_or_else(||error("data_subject_missing"))?;if core::hash(&e.content)!=e.sha256{return Err(error("data_subject_changed"));}
            let (refs,verified)=evidence_origins(store,s,aid,evidence_id)?;origins=refs;origin_verified=verified;
            for r in &origins{contributors.push(SubjectFingerprint{subject:origin_subject(r),fingerprint:hash(&source_value(s,r)?)});}
            let fingerprint=hash(&json!({"id":e.id,"name":e.name,"rawSha256":e.sha256,"rawBytes":e.content.len()}));
            let capture=crate::source_context::is_source_capture(e,a,&s.plans);(vec![if capture{DataCategory::SourceExcerpt}else{DataCategory::EvidenceExcerpt}],json!({"name":e.name,"content":e.content}),fingerprint)},
        DataSubject::Derived{producer:DerivedProducer::Finding,object_id,..}=>{let f=s.findings.iter().find(|f|f.id==*object_id&&f.assessment_id==aid).ok_or_else(||error("data_subject_missing"))?;
            for id in &f.evidence_ids{let e=resolve_subject(store,s,&DataSubject::Evidence{assessment_id:aid.into(),evidence_id:id.clone()},external)?;origins.extend(e.origins);contributors.push(SubjectFingerprint{subject:e.subject,fingerprint:e.fingerprint});origin_verified&=e.origin_verified;}
            let v=serde_json::to_value(f).map_err(|_|error("data_projection_failed"))?;(vec![DataCategory::FindingText],v.clone(),hash(&v))},
        DataSubject::Derived{producer:DerivedProducer::Validation,object_id,..}=>{let v=s.validations.iter().find(|v|v.id==*object_id&&v.assessment_id==aid).ok_or_else(||error("data_subject_missing"))?;for e in &v.evidence{let r=resolve_subject(store,s,&DataSubject::Evidence{assessment_id:aid.into(),evidence_id:e.id.clone()},external)?;origins.extend(r.origins);contributors.push(SubjectFingerprint{subject:r.subject,fingerprint:r.fingerprint});origin_verified&=r.origin_verified;}let value=serde_json::to_value(v).map_err(|_|error("data_projection_failed"))?;(vec![DataCategory::FindingText],value.clone(),hash(&value))},
        DataSubject::Derived{..}=>{let mut resolved=external.resolve(subject)?;if resolved.subject!=*subject||resolved.assessment_id!=aid||resolved.project_id!=p.id||!data_policy::sha(&resolved.fingerprint){return Err(error("data_subject_changed"));}apply_data_association(s,&mut resolved)?;return Ok(resolved)},
    };
    origins.sort_by_key(hash);origins.dedup();
    let fingerprint=versioned_fingerprint(s,subject,fingerprint);
    let mut resolved=ResolvedDataSubject{subject:subject.clone(),project_id:p.id.clone(),assessment_id:aid.into(),fingerprint,categories,origins,origin_verified,contributors,value};apply_data_association(s,&mut resolved)?;Ok(resolved)
}
pub fn classification_state(s:&Snapshot,resolved:&ResolvedDataSubject)->Result<ClassificationState,String>{
    let current=data_policy::current_classification(s,&resolved.subject).filter(|r|r.source_fingerprint==resolved.fingerprint);
    let mut sensitivity=current.map_or(Sensitivity::Unclassified,|r|r.sensitivity);let mut secret=current.is_some_and(|r|r.secret_material);let mut refs=vec![];
    for contributor in &resolved.contributors{if let Some(r)=data_policy::current_classification(s,&contributor.subject).filter(|r|r.source_fingerprint==contributor.fingerprint){refs.push(data_policy::classification_ref(r));secret|=r.secret_material;sensitivity=if sensitivity==Sensitivity::Unclassified||r.sensitivity==Sensitivity::Unclassified{Sensitivity::Unclassified}else{sensitivity.max(r.sensitivity)};}else{sensitivity=Sensitivity::Unclassified;}}
    let mut reasons=vec![];if sensitivity==Sensitivity::Unclassified{reasons.push("data_classification_required".into());}if secret{reasons.push("data_secret_material".into());}
    Ok(ClassificationState{sensitivity,classification_ref:current.map(data_policy::classification_ref),source_fingerprint:resolved.fingerprint.clone(),secret_material:secret,contributor_refs:refs,reason_codes:reasons})
}
/// Bounded transitive closure for local windows and other host builders. Every
/// contributor is resolved from storage, never accepted as a renderer assertion.
pub fn subject_versions(store:&Store,s:&Snapshot,subject:&DataSubject,external:&dyn ExternalSubjectResolver)->Result<Vec<data_policy::SubjectVersion>,String>{
    fn visit(store:&Store,s:&Snapshot,subject:&DataSubject,external:&dyn ExternalSubjectResolver,path:&mut Vec<String>,out:&mut Vec<data_policy::SubjectVersion>)->Result<(),String>{
        let key=hash(subject);if path.contains(&key)||path.len()>=16||out.len()>=128{return Err(error("data_lineage_limit"));}if out.iter().any(|v|v.subject==*subject){return Ok(());}path.push(key);
        let r=resolve_subject(store,s,subject,external)?;let c=data_policy::current_classification(s,subject).filter(|c|c.source_fingerprint==r.fingerprint);
        out.push(data_policy::SubjectVersion{subject:subject.clone(),source_fingerprint:r.fingerprint,classification_ref:c.map(data_policy::classification_ref)});
        for child in r.contributors{visit(store,s,&child.subject,external,path,out)?;if out.iter().find(|v|v.subject==child.subject).is_none_or(|v|v.source_fingerprint!=child.fingerprint){return Err(error("data_subject_changed"));}}
        path.pop();Ok(())
    }
    let mut result=vec![];visit(store,s,subject,external,&mut vec![],&mut result)?;Ok(result)
}
fn limited(mut text:String)->String{let mut limit=2048.min(text.len());while !text.is_char_boundary(limit){limit-=1;}text.truncate(limit);text}
pub fn project_text(text:&str,redactor:&dyn DataRedactor)->String{if redactor.mode()==RedactionMode::MetadataOnly{"[Texto indisponível: confira o armazenamento protegido da credencial.]".into()}else{limited(redactor.redact(text).text)}}
fn workflow_control(key:&str,value:&str)->bool{
    use crate::workflow as w;
    fn enum_value<T:serde::de::DeserializeOwned>(value:&str)->bool{serde_json::from_value::<T>(Value::String(value.into())).is_ok()}
    match key {
        "control"=>enum_value::<w::PlanControl>(value),
        "outcome"=>enum_value::<w::PlanOutcome>(value),
        "state"=>enum_value::<w::JobState>(value)||enum_value::<w::AttemptState>(value)||enum_value::<crate::capture::CaptureState>(value),
        "cleanup"=>enum_value::<w::CleanupState>(value),
        "backend"=>enum_value::<w::Backend>(value),
        "supportedIntents"=>enum_value::<w::IntentKind>(value),
        "outputRole"=>enum_value::<w::OutputRole>(value),
        "perspectives"=>enum_value::<w::Perspective>(value),
        "producer"=>enum_value::<data_policy::DerivedProducer>(value),
        "stream"=>enum_value::<crate::capture::CaptureStream>(value),
        "encoding"=>enum_value::<crate::capture::CaptureEncoding>(value),
        "representation"=>enum_value::<crate::capture::CaptureRepresentation>(value),
        "lossReason"=>enum_value::<crate::capture::CaptureLossReason>(value),
        "type"=>enum_value::<w::WorkEventType>(value),
        "code"|"reasonCode"|"reasonCodes"=>w::known_reason_code(value),
        "kind"=>enum_value::<w::IntentKind>(value)||matches!(value,"native_execution"|"ai_request"|"kali_run"|"remediation"|"dependency_output"),
        "schema"=>matches!(value,"openkoda.action-plan.v1"|"openkoda.action-plan-revision.v1"|"openkoda.work-event.v1"|"openkoda.work-capabilities.v1"|"openkoda.work-attempt-data.v1"|"openkoda.capture.v1"),
        "action"=>matches!(value,"work.plan.commit"|"work.operation.prepare"|"work.operation.enqueue"|"work.review.commit"|"work.pause"|"work.resume.commit"|"work.cancel"|"work.retry.prepare"|"work.reconcile"|"work.operation.attachInvestigation"),
        "decision"=>matches!(value,"continue"|"needs_more_work"|"stop"),
        _=>false,
    }
}
fn control(key:&str,value:&str)->bool{
    if workflow_control(key,value){return true;}
    let is_identity=key=="id"||key.ends_with("Id")||key.ends_with("Ids");
    if is_identity&&core::valid_id(value).is_ok(){return true;}
    if matches!(key,"projectTargetId"|"targetCatalogId"|"targetCatalogIds"|"primaryProjectTargetId")&&value=="primary"{return true;}
    if (key.to_ascii_lowercase().contains("sha256")||key.to_ascii_lowercase().ends_with("digest")||key=="contextHash"||key=="fingerprint"||key=="sourceFingerprint")&&data_policy::sha(value){return true;}
    if (key.ends_with("At")||key=="at"||key=="created"||key=="expiresAt")&&chrono::DateTime::parse_from_rfc3339(value).is_ok(){return true;}
    // These are control *values*, not a permission to expose arbitrary text
    // merely because an untrusted object names its property "status".
    let values:&[&str]=match key{
        "provider"=>&["openai","openrouter","groq","anthropic","gemini","deepseek","xai"],
        "kind"=>&["web","network","code","files","combined","info","warning","error","success","source_rules","source_capture","provider_reference","target_reference","metadata_only","user_association","host_record","unknown","local","connection","local_raw_view","evidence","derived","scope_source","assessment_metadata"],
        "status"|"phase"=>&["draft","preparing","planning","awaiting_approval","queued","running","pausing","paused","validating","completed","partial","canceling","canceled","blocked","failed","unknown","candidate","validated","dismissed","fixed","proposed","approved","revoked","pending","ready","unavailable","starting","stopped","enabled","disabled","frozen","legacy","unreviewed","reviewed","admitted","dispatched","not_sent","failed_before_send","needs_review","retained","purged"],
        "severity"=>&["info","low","medium","high","critical"],
        "confidence"=>&["low","medium","high","candidate","confirmed"],
        "author"=>&["local-user","system","user","assistant"],
        "adapter"|"method"|"methods"|"commonMethods"=>&["http.headers.v1","network.tcp-connect.v1","local.inventory.v1","source.rules.v1","source.capture.v1","ai.remote-review.v1","kali.hash-evidence.v1","kali.rg.signals.v1","kali.runtime-diagnostics.v1","local.text-remediation.v1","manual","manual_review","static_review","passive_observation","controlled_reproduction","human_review","http.fixed-proof.v1"],
        "operation"=>&["evidence_read","derived_read","ai_ask","ai_operation_propose","ai_investigation","kali_hash_evidence","kali_scan_evidence","report_export","recovery_backup","ask","propose","investigation"],
        "outcome"=>&["admitted","dispatched","completed","not_sent","failed","unknown","canceled"],
        "destination"=>&["local_raw_view","remote_ai","local_kali","report_export","recovery_backup"],
        "sensitivity"|"sensitivities"|"effectiveSensitivity"|"packageSensitivity"=>&["unclassified","public","internal","sensitive"],
        "category"|"categories"=>&["human_text","target_metadata","execution_summary","evidence_excerpt","source_excerpt","finding_text","activity_metadata"],
        "representation"=>&["raw_view","redacted_raw_view"],
        "capability"=>&["text","metadata_only","provider_slot_reference"],
        "protocolVersion"=>&["2024-11-05","2025-03-26","2025-06-18","2025-11-25"],
        "catalogVersion"=>&["openkoda.scope-methods.v1"],
        "redactorVersion"=>&["openkoda.redactor.v1"],
        "rulesVersion"|"ruleVersion"=>&["2026-11-07.1"],
        "originKind"=>&["host_record","user_association","unknown"],
        "origin"=>&["user_supplied","explicit_adoption","host_selected_source_redacted_copy","host_record","user_association"],
        "tool"=>&["runtime_diagnostics","hash_evidence","scan_evidence"],
        "mode"=>&["assisted","automatic","manual","ready","no_credential_configured","metadata_only"],
        "theme"=>&["light","dark","system"],
        "language"=>&["pt-BR","en-US"],
        "termination"=>&["completed","failed","canceled","timeout","output_limit","unknown"],
        "schema"=>&["openkoda.snapshot-view.v1","openkoda.projection.v1","openkoda.data-policy.v1","openkoda.data-policy-status.v1","openkoda.scope-options.v1","openkoda.scope-status.v1","openkoda.scope-report.v1","openkoda.scope.v1","openkoda.scope.v2","openkoda.scope-target-set.v1","openkoda.project-targets.v1","openkoda.manual-evidence.v1","openkoda.manual-evidence.v2","openkoda.kali-evidence.v2","openkoda.source-context.v1","openkoda.report.v1","openkoda.report-manifest.v1","openkoda.backup-manifest.v1","openkoda.restore-plan.v1"],
        "reasonCodes"|"code"=>&["data_redactor_unavailable","data_shape_unavailable","data_classification_required","data_secret_material","data_source_unknown","data_destination_denied","data_category_required","data_policy_required","data_policy_changed","data_policy_inactive","data_binding_required","data_subject_changed","data_view_changed","data_review_required","data_review_used","scope_unreviewed","scope_revoked","scope_expired","project_archived"],
        _=>&[],
    };values.contains(&value)
}
pub fn project_value(value:Value,redactor:&dyn DataRedactor)->Value{
    fn walk(value:Value,key:&str,r:&dyn DataRedactor)->Value{match value{Value::String(v)=>{let projected=r.redact(&v);if r.mode()!=RedactionMode::MetadataOnly&&(projected.changed||projected.secret_material||r.contains_managed_secret(&v)){Value::String(limited(projected.text))}else if control(key,&v){Value::String(v)}else{Value::String(project_text(&v,r))}},Value::Array(v)=>Value::Array(v.into_iter().map(|v|walk(v,key,r)).collect()),Value::Object(v)=>Value::Object(v.into_iter().filter(|(k,_)|!matches!(k.as_str(),"content"|"context"|"before"|"after"|"sourceText"|"raw"|"rawOutput"|"base64"|"password"|"secret"|"apiKey"|"cipher")).map(|(k,v)|{let next=walk(v,&k,r);(k,next)}).collect()),other=>other}}
    walk(value,"",redactor)
}
pub fn projection_info(redactor:&dyn DataRedactor,redacted:bool,omitted:Vec<&str>)->Value{json!({"schema":"openkoda.projection.v1","redactorVersion":"openkoda.redactor.v1","mode":redactor.mode(),"redacted":redacted,"omittedFields":omitted,"reasonCodes":if redactor.mode()==RedactionMode::MetadataOnly{vec!["data_redactor_unavailable"]}else{vec![]}})}
pub fn evidence_view(store:&Store,s:&Snapshot,aid:&str,eid:&str,redactor:&dyn DataRedactor)->Result<EvidenceView,String>{
    let a=s.assessments.iter().find(|a|a.id==aid).ok_or_else(||error("data_subject_missing"))?;let e=a.evidence.iter().find(|e|e.id==eid).ok_or_else(||error("data_subject_missing"))?;
    let resolved=resolve_subject(store,s,&DataSubject::Evidence{assessment_id:aid.into(),evidence_id:eid.into()},&RejectExternal)?;let mut class=classification_state(s,&resolved)?;
    let redacted=redactor.redact(&e.content);class.secret_material|=redacted.secret_material;
    let preview=if redactor.mode()==RedactionMode::MetadataOnly{None}else{Some(limited(redacted.text.clone()))};let bytes=preview.as_ref().map_or(0,String::len);
    let raw:Value=serde_json::from_str(&e.content).unwrap_or(Value::Null);
    let structured=if raw["adapter"]=="source.rules.v1"&&s.executions.iter().any(|x|x.assessment_id==aid&&x.evidence_id.as_deref()==Some(eid)&&s.plans.iter().any(|p|p.id==x.plan_id&&p.adapter=="source.rules.v1")){Some(json!({"kind":"source_rules","data":project_value(raw,redactor)}))}else if crate::source_context::is_source_capture(e,a,&s.plans){let mut v=raw; if let Some(files)=v["source"]["files"].as_array_mut(){for f in files{if let Some(o)=f.as_object_mut(){o.remove("text");}}}Some(json!({"kind":"source_capture","data":project_value(v,redactor)}))}else{None};
    let shape=|allowed:bool|json!({"allowedByShape":allowed,"reasonCodes":if allowed{vec![]}else{vec!["data_shape_unavailable"]},"requiresDataReview":true});
    Ok(json!({"id":e.id,"name":project_text(&e.name,redactor),"createdAt":e.created_at,"rawSha256":e.sha256,"rawBytes":e.content.len(),"hasRaw":true,"classification":class,"preview":preview,"previewBytes":bytes,"previewTruncated":bytes<redacted.text.len(),"projectionSha256":preview.as_ref().map(|v|core::hash(v)),"projection":projection_info(redactor,redacted.changed,vec!["content"]),"originRefs":resolved.origins,"originKind":if resolved.origin_verified{"host_record"}else if resolved.origins.is_empty(){"unknown"}else{"user_association"},"structuredView":structured,"eligibility":{"ai":shape(e.content.len()<=48*1024),"kaliHash":shape(e.content.len()<=32768),"kaliScan":shape(e.content.len()<=32768&&!e.content.contains('\0')),"rawRead":shape(redactor.mode()!=RedactionMode::MetadataOnly)}}))
}
pub fn snapshot_view(store:&Store,s:&Snapshot,redactor:&dyn DataRedactor)->Result<SnapshotView,String>{
    let mut value=serde_json::to_value(s).map_err(|_|error("data_projection_failed"))?;
    for key in ["dataPolicies","dataPolicyRevisions","dataBindings","dataClassifications","credentialReferences","dataUseReceipts","providerSlots","providerModels","dataMutationReceipts","dataSubjectGenerations"]{value.as_object_mut().unwrap().remove(key);}
    let mut value=project_value(value,redactor);
    // Only the validated project preference DTO can retain its three context
    // fields. Generic evidence/source contexts remain omitted by project_value.
    for (project,original) in value["projects"].as_array_mut().ok_or_else(||error("data_projection_failed"))?.iter_mut().zip(&s.projects){
        if let Some(options)=&original.analysis_options{project["analysisOptions"]=crate::project_options::view(options,redactor);}
    }
    for a in value["assessments"].as_array_mut().ok_or_else(||error("data_projection_failed"))?{let aid=a["id"].as_str().ok_or_else(||error("data_projection_failed"))?.to_owned();let original=s.assessments.iter().find(|a|a.id==aid).unwrap();a["evidence"]=json!(original.evidence.iter().map(|e|evidence_view(store,s,&aid,&e.id,redactor)).collect::<Result<Vec<_>,_>>()?);}
    value["schema"]=json!("openkoda.snapshot-view.v1");value["projection"]=projection_info(redactor,true,vec!["assessments[].evidence[].content"]);Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SECRET:&str="SYNTHETIC-CREDENTIAL-123456";
    struct Redactor(RedactionMode);
    impl DataRedactor for Redactor {
        fn mode(&self)->RedactionMode{self.0}
        fn redact(&self,text:&str)->RedactedText{let text2=text.replace(SECRET,"[REDACTED]");RedactedText{changed:text2!=text,secret_material:text.contains(SECRET),text:text2}}
        fn contains_managed_secret(&self,text:&str)->bool{text.contains(SECRET)}
    }
    #[test]
    fn control_named_text_never_bypasses_managed_redaction_or_metadata_only() {
        for mode in [RedactionMode::Ready,RedactionMode::MetadataOnly] {
            let input=json!({"provider":SECRET,"kind":SECRET,"status":SECRET,"schema":SECRET,"code":SECRET});
            let projected=project_value(input,&Redactor(mode));
            assert!(!projected.to_string().contains(SECRET));
            for key in ["provider","kind","status","schema","code"]{assert!(projected[key].is_string());}
        }
        let source=json!({"provider":"custom_provider_name","kind":"made_up_kind","status":"free_text","schema":"openkoda.unreviewed_schema.v99","code":"custom_opaque_code"});
        let projected=project_value(source,&Redactor(RedactionMode::MetadataOnly));
        for key in ["provider","kind","status","schema","code"]{assert!(projected[key].as_str().unwrap().starts_with("[Texto indisponível"));}
    }
    #[test]
    fn metadata_only_retains_validated_identity_and_closed_control_values() {
        let id=crate::model::id();let at=crate::model::now();let digest=core::hash("fixture");
        let input=json!({"id":id,"assessmentId":id,"createdAt":at,"digest":digest,"provider":"openai","kind":"code","status":"completed","schema":"openkoda.snapshot-view.v1","code":"data_redactor_unavailable","message":"free text","methods":["source.rules.v1","kali.rg.signals.v1"],"commonMethods":["ai.remote-review.v1"],"catalogVersion":"openkoda.scope-methods.v1","targetCatalogId":"primary"});
        let projected=project_value(input.clone(),&Redactor(RedactionMode::MetadataOnly));
        for key in ["id","assessmentId","createdAt","digest","provider","kind","status","schema","code","methods","commonMethods","catalogVersion","targetCatalogId"]{assert_eq!(projected[key],input[key],"{key}");}
        assert_ne!(projected["message"],input["message"]);
    }
    #[test]
    fn workflow_and_capture_controls_remain_closed_in_metadata_only_mode(){
        let input=json!({"control":"paused","outcome":"partial","state":"waiting_dependencies","backend":"native","cleanup":"unknown","type":"capture_sealed","producer":"capture","schema":"openkoda.capture.v1","stream":"adapter_records","encoding":"utf8","representation":"adapter_observation_records","lossReason":"interrupted","code":"work_owned_operation","action":"work.operation.enqueue","sourceFingerprint":core::hash("capture"),"supportedIntents":["native","human_review"]});
        assert_eq!(project_value(input.clone(),&Redactor(RedactionMode::MetadataOnly)),input);
        let object=input.as_object().unwrap();
        for key in object.keys(){
            let poisoned=json!({key:format!("work_{SECRET}")});
            let projected=project_value(poisoned,&Redactor(RedactionMode::MetadataOnly));
            assert!(!projected.to_string().contains(SECRET));
            assert!(projected[key].as_str().unwrap().starts_with("[Texto indisponível"),"{key}");
        }
    }
}
