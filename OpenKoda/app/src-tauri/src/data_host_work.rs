//! Data identities for workflow metadata and sealed captures, never raw list DTOs.
use super::*;
use crate::{capture::{self,CaptureState},work_store,workflow::{DerivedResultProducer,OperationIntent,ResultRef,WorkInput}};

fn subject(assessment:&str,producer:DerivedProducer,id:&str)->DataSubject{
    DataSubject::Derived{assessment_id:assessment.into(),producer,object_id:id.into()}
}
fn evidence_subject(assessment:&str,id:&str)->DataSubject{DataSubject::Evidence{assessment_id:assessment.into(),evidence_id:id.into()}}
fn result_subject(assessment:&str,result:&ResultRef)->Option<DataSubject>{
    match result{
        ResultRef::Evidence{id,..}=>Some(evidence_subject(assessment,id)),
        ResultRef::Derived{producer,id,..}=>Some(subject(assessment,match producer{DerivedResultProducer::Investigation=>DerivedProducer::Investigation,DerivedResultProducer::KaliRun=>DerivedProducer::KaliRun},id)),
        ResultRef::HumanReview{..}=>None,
    }
}

pub(super) fn resolve(resolver:&JournalResolver<'_>,requested:&DataSubject)->Result<ResolvedDataSubject,String>{
    let DataSubject::Derived{assessment_id,producer,object_id}=requested else{return Err(failure("not workflow"));};
    core::valid_id(assessment_id)?;core::valid_id(object_id)?;
    let (state,value,mut origins,mut sources,categories,verified)=resolver.store.work_read(|db,s|{
        let mut origins=vec![];let mut sources=vec![];
        let (value,categories,verified)=match producer{
            DerivedProducer::WorkPlan=>{
                let revision=work_store::revision(db,object_id)?;
                if revision.assessment_id!=*assessment_id{return Err(failure("ownership"));}
                for node in &revision.body.nodes{
                    if let OperationIntent::Native{target_ref,..}=&node.intent{
                        origins.push(DataOriginRef{scope_ref:revision.body.scope_ref.clone(),target_ref:target_ref.clone()});
                    }
                    for input in node.intent.inputs(){
                        if let WorkInput::Evidence{evidence_id,..}=input{sources.push(evidence_subject(assessment_id,evidence_id));}
                    }
                }
                (json!(revision),vec![DataCategory::HumanText,DataCategory::TargetMetadata,DataCategory::ExecutionSummary],true)
            },
            DerivedProducer::WorkAttempt=>{
                let attempt=work_store::attempt(db,object_id)?;
                let job=work_store::job(db,&attempt.view.job_id)?;
                if job.assessment_id!=*assessment_id{return Err(failure("ownership"));}
                sources.push(subject(assessment_id,DerivedProducer::WorkPlan,&job.revision.revision_id));
                for input in &attempt.input_refs{
                    if let Some(source)=result_subject(assessment_id,input){sources.push(source);}
                }
                (json!({"schema":"openkoda.work-attempt-data.v1","id":attempt.view.id,"assessmentId":assessment_id,"revision":job.revision,"attempt":attempt.view,"inputRefs":attempt.input_refs}),vec![DataCategory::ExecutionSummary,DataCategory::ActivityMetadata],true)
            },
            DerivedProducer::Capture=>{
                let manifest=capture::load_locked(db,object_id)?;
                if manifest.assessment_id!=*assessment_id{return Err(failure("ownership"));}
                if manifest.state==CaptureState::Open{return Err(crate::workflow::error("capture_not_sealed"));}
                if manifest.state==CaptureState::Purged{return Err(crate::workflow::error("capture_unavailable"));}
                // Stream validity is checked by Store and again before raw access.
                origins=manifest.origin_refs.clone();
                for input in &manifest.input_refs{
                    if let Some(source)=result_subject(assessment_id,input){sources.push(source);}
                }
                let verified=manifest.source_verified;
                (json!(manifest),vec![DataCategory::EvidenceExcerpt,DataCategory::ExecutionSummary],verified)
            },
            _=>return Err(failure("producer")),
        };
        Ok((s.clone(),value,origins,sources,categories,verified))
    })?;
    let assessment=state.assessments.iter().find(|a|a.id==*assessment_id).ok_or_else(||failure("assessment"))?;
    let mut contributors=vec![];
    let mut origin_verified=verified;
    // Never hold Store's transaction while resolving transitive subjects.
    sources.sort_by_key(crate::data_policy::hash);sources.dedup();
    for source in sources{
        let resolved=resolve_subject(resolver.store,&state,&source,resolver)?;
        origins.extend(resolved.origins);origin_verified&=resolved.origin_verified;
        contributors.push(SubjectFingerprint{subject:source,fingerprint:resolved.fingerprint});
    }
    origins.sort_by_key(crate::data_policy::hash);origins.dedup();
    for origin in &origins{
        let source=origin_subject(origin);
        let resolved=resolve_subject(resolver.store,&state,&source,resolver)?;
        if !contributors.iter().any(|c|c.subject==source){contributors.push(SubjectFingerprint{subject:source,fingerprint:resolved.fingerprint});}
    }
    Ok(ResolvedDataSubject{subject:requested.clone(),project_id:assessment.project_id.clone(),assessment_id:assessment_id.clone(),fingerprint:core::hash(&value.to_string()),categories,origin_verified:origin_verified&&!origins.is_empty(),origins,contributors,value})
}

pub(super) fn list(store:&Store,assessment:&str)->Result<Vec<DataSubject>,String>{
    store.work_read(|db,_|{
        let mut result=vec![];
        for (producer,sql) in [
            (DerivedProducer::WorkPlan,"SELECT id FROM work_plan_revisions WHERE assessment_id=?1 ORDER BY rowid LIMIT 1001"),
            (DerivedProducer::WorkAttempt,"SELECT a.id FROM work_attempts a JOIN work_jobs j ON j.id=a.job_id WHERE j.assessment_id=?1 ORDER BY a.rowid LIMIT 1001"),
            (DerivedProducer::Capture,"SELECT id FROM capture_manifests WHERE assessment_id=?1 AND state IN ('sealed','interrupted') ORDER BY rowid LIMIT 1001"),
        ]{
            let mut query=db.prepare(sql).map_err(failure)?;
            let rows=query.query_map([assessment],|r|r.get::<_,String>(0)).map_err(failure)?;
            for id in rows{let id=id.map_err(failure)?;core::valid_id(&id)?;result.push(subject(assessment,producer,&id));if result.len()>1000{return Err("data_limit: excesso de registros para revisão local.".into());}}
        }
        Ok(result)
    })
}

pub(super) fn capture_text(store:&Store,resolved:&ResolvedDataSubject)->Result<String,String>{
    let DataSubject::Derived{assessment_id,producer:DerivedProducer::Capture,object_id}=&resolved.subject else{return Err(failure("not capture"));};
    store.work_read(|db,_|{
        let manifest=capture::load_locked(db,object_id)?;
        if manifest.assessment_id!=*assessment_id||core::hash(&json!(manifest).to_string())!=resolved.fingerprint{return Err(crate::workflow::error("capture_integrity"));}
        if !matches!(manifest.state,CaptureState::Sealed|CaptureState::Interrupted){return Err(crate::workflow::error("capture_not_sealed"));}
        if manifest.encoding!=capture::CaptureEncoding::Utf8{return Err(crate::workflow::error("capture_encoding_unavailable"));}
        String::from_utf8(capture::preserved_locked(db,&manifest)?).map_err(|_|crate::workflow::error("capture_encoding_unavailable"))
    })
}
