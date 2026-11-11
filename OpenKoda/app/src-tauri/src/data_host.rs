//! The data resolver only reads known local journals. A subject never supplies
//! a database path, SQL statement, source path, provider credential or grant.
use crate::{core::{self,Store},model::Snapshot,data_policy::*,data_projection::*};
use rusqlite::{Connection,OpenFlags,OptionalExtension};
use serde_json::{json,Value};
use std::time::Duration;
#[path="data_host_work.rs"]
mod work;

pub(crate) struct JournalResolver<'a>{pub store:&'a Store}
fn failure(_:impl std::fmt::Display)->String{"data_source_unknown: registro local indisponível para revisão.".into()}

impl JournalResolver<'_>{
    fn database(&self,name:&str)->Result<Option<Connection>,String>{
        let path=self.store.root.join(name);
        if !path.exists(){return Ok(None);}
        crate::recovery::require_plain(&path,false)?;
        let db=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(failure)?;
        db.busy_timeout(Duration::from_millis(500)).map_err(failure)?;
        db.execute_batch("PRAGMA trusted_schema=OFF;").map_err(failure)?;
        Ok(Some(db))
    }
    fn record(&self,producer:DerivedProducer,id:&str,assessment:&str)->Result<Value,String>{
        core::valid_id(id)?;core::valid_id(assessment)?;
        let (file,sql)=match producer{
            DerivedProducer::AiRequest=>("assistant.sqlite3","SELECT json_object('id',id,'assessmentId',assessment,'endpoint',endpoint,'model',model,'credentialRevision',revision,'contextHash',context_hash,'status',status,'createdAt',created,'inputTokens',input_tokens,'outputTokens',output_tokens,'details',json(details)) FROM requests WHERE id=?1 AND assessment=?2"),
            DerivedProducer::Investigation=>("assistant.sqlite3","SELECT data FROM investigations WHERE id=?1 AND assessment=?2"),
            DerivedProducer::Remediation=>("remediation.sqlite3","SELECT data FROM remediation_plans WHERE id=?1 AND json_extract(data,'$.assessmentId')=?2"),
            DerivedProducer::KaliRun=>("kali-bridge.sqlite3","SELECT data FROM bridge_runs WHERE id=?1 AND json_extract(data,'$.assessmentId')=?2"),
            _=>return Err(failure("unsupported producer")),
        };
        let db=self.database(file)?.ok_or_else(||failure("missing journal"))?;
        let text:Option<String>=db.query_row(sql,[id,assessment],|r|r.get(0)).optional().map_err(failure)?;
        let text=text.ok_or_else(||failure("missing subject"))?;
        if text.len()>2*1024*1024{return Err("data_limit: registro excede a janela de revisão.".into());}
        let value:Value=serde_json::from_str(&text).map_err(failure)?;
        if value["id"]!=id||value["assessmentId"]!=assessment{return Err(failure("identity mismatch"));}
        Ok(value)
    }
}

impl ExternalSubjectResolver for JournalResolver<'_>{
    fn resolve(&self,subject:&DataSubject)->Result<ResolvedDataSubject,String>{
        let DataSubject::Derived{assessment_id,producer,object_id}=subject else{return Err(failure("not journal"));};
        if matches!(producer,DerivedProducer::WorkPlan|DerivedProducer::WorkAttempt|DerivedProducer::Capture){return work::resolve(self,subject);}
        let state=self.store.snapshot()?;
        let assessment=state.assessments.iter().find(|a|a.id==*assessment_id).ok_or_else(||failure("assessment"))?;
        let value=self.record(*producer,object_id,assessment_id)?;
        let record=if *producer==DerivedProducer::AiRequest{&value["details"]}else{&value};
        let mut contributors=vec![];
        let evidence_ids=match producer{
            DerivedProducer::KaliRun=>record.get("sourceEvidenceId").and_then(Value::as_str).map(|id|vec![id.to_owned()]).unwrap_or_default(),
            _=>record.get("evidenceIds").and_then(Value::as_array).map(|items|items.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
        };
        for id in evidence_ids{
            let source=DataSubject::Evidence{assessment_id:assessment_id.clone(),evidence_id:id};
            let resolved=resolve_subject(self.store,&state,&source,self)?;
            contributors.push(SubjectFingerprint{subject:source,fingerprint:resolved.fingerprint});
        }
        let mut origins=vec![];
        if let Some(reference)=record.get("scopeRef").filter(|v|!v.is_null()){
            let scope_ref:crate::scope::ScopeRef=serde_json::from_value(reference.clone()).map_err(failure)?;
            let refs=record.get("contextTargetRefs").or_else(||record.get("sourceTargetRefs")).and_then(Value::as_array);
            if let Some(refs)=refs.filter(|refs|!refs.is_empty()){
                for target in refs{
                    let target_ref=serde_json::from_value(target.clone()).map_err(failure)?;
                    origins.push(DataOriginRef{scope_ref:scope_ref.clone(),target_ref:Some(target_ref)});
                }
            }else if let Some(target)=record.get("targetRef").filter(|v|!v.is_null()){
                origins.push(DataOriginRef{scope_ref,target_ref:Some(serde_json::from_value(target.clone()).map_err(failure)?)});
            }else if !crate::scope_targets::is_combined(&state,assessment_id){origins.push(DataOriginRef{scope_ref,target_ref:None});}
        }
        for origin in &origins{
            let source=origin_subject(origin);let resolved=resolve_subject(self.store,&state,&source,self)?;
            if !contributors.iter().any(|c|c.subject==source){contributors.push(SubjectFingerprint{subject:source,fingerprint:resolved.fingerprint});}
        }
        if *producer==DerivedProducer::Remediation{
            if let Some(finding_id)=record.get("findingId").and_then(Value::as_str){
                let source=DataSubject::Derived{assessment_id:assessment_id.clone(),producer:DerivedProducer::Finding,object_id:finding_id.into()};
                let resolved=resolve_subject(self.store,&state,&source,self)?;
                contributors.push(SubjectFingerprint{subject:source,fingerprint:resolved.fingerprint});
            }
        }
        let categories=match producer{
            DerivedProducer::Investigation=>vec![DataCategory::ExecutionSummary,DataCategory::HumanText,DataCategory::TargetMetadata,DataCategory::EvidenceExcerpt,DataCategory::SourceExcerpt,DataCategory::FindingText,DataCategory::ActivityMetadata],
            DerivedProducer::Remediation=>vec![DataCategory::ExecutionSummary,DataCategory::SourceExcerpt,DataCategory::TargetMetadata,DataCategory::FindingText],
            DerivedProducer::AiRequest=>vec![DataCategory::ExecutionSummary,DataCategory::HumanText,DataCategory::TargetMetadata],
            _=>vec![DataCategory::ExecutionSummary],
        };
        Ok(ResolvedDataSubject{subject:subject.clone(),project_id:assessment.project_id.clone(),assessment_id:assessment_id.clone(),fingerprint:core::hash(&value.to_string()),categories,origin_verified:!origins.is_empty(),origins,contributors,value})
    }
    fn list(&self,assessment_id:&str)->Result<Vec<DataSubject>,String>{
        core::valid_id(assessment_id)?;
        let mut subjects=vec![];
        for (producer,file,sql) in [
            (DerivedProducer::AiRequest,"assistant.sqlite3","SELECT id FROM requests WHERE assessment=?1 ORDER BY id LIMIT 1001"),
            (DerivedProducer::Investigation,"assistant.sqlite3","SELECT id FROM investigations WHERE assessment=?1 ORDER BY id LIMIT 1001"),
            (DerivedProducer::Remediation,"remediation.sqlite3","SELECT id FROM remediation_plans WHERE json_extract(data,'$.assessmentId')=?1 ORDER BY id LIMIT 1001"),
            (DerivedProducer::KaliRun,"kali-bridge.sqlite3","SELECT id FROM bridge_runs WHERE json_extract(data,'$.assessmentId')=?1 ORDER BY id LIMIT 1001"),
        ]{
            let Some(db)=self.database(file)? else{continue;};
            let mut query=db.prepare(sql).map_err(failure)?;
            let rows=query.query_map([assessment_id],|row|row.get::<_,String>(0)).map_err(failure)?;
            for row in rows{
                let object_id=row.map_err(failure)?;core::valid_id(&object_id)?;
                subjects.push(DataSubject::Derived{assessment_id:assessment_id.into(),producer,object_id});
                if subjects.len()>1000{return Err("data_limit: excesso de registros para revisão local.".into());}
            }
        }
        subjects.extend(work::list(self.store,assessment_id)?);
        if subjects.len()>1000{return Err("data_limit: excesso de registros para revisão local.".into());}
        Ok(subjects)
    }
    fn access_text(&self,resolved:&ResolvedDataSubject)->Result<String,String>{
        if matches!(resolved.subject,DataSubject::Derived{producer:DerivedProducer::Capture,..}){return work::capture_text(self.store,resolved);}
        serde_json::to_string_pretty(&resolved.value).map_err(failure)
    }
}

/// Keep the canonical evidence object from crossing any mutation or nested
/// result boundary. Replacement happens before any generic text projection.
pub(crate) fn replace_evidence(store:&Store,s:&Snapshot,value:Value,redactor:&dyn DataRedactor)->Result<Value,String>{
    match value{
        Value::Array(items)=>Ok(Value::Array(items.into_iter().map(|v|replace_evidence(store,s,v,redactor)).collect::<Result<_,_>>()?)),
        Value::Object(mut object)=>{
            if object.contains_key("content")&&object.contains_key("sha256"){
                if let Some(id)=object.get("id").and_then(Value::as_str){
                    if let Some(a)=s.assessments.iter().find(|a|a.evidence.iter().any(|e|e.id==id)){
                        return serde_json::to_value(evidence_view(store,s,&a.id,id,redactor)?).map_err(failure);
                    }
                }
                // A malformed/unrecognized evidence-shaped result is metadata
                // only. Never fall back to the raw object after lookup failure.
                object.remove("content");object.insert("contentUnavailable".into(),json!(true));
            }
            let items=object.into_iter().map(|(key,value)|Ok((key,replace_evidence(store,s,value,redactor)?))).collect::<Result<_,String>>()?;
            Ok(Value::Object(items))
        },
        other=>Ok(other),
    }
}

fn archive_receipt_controls(raw: &Value, view: &mut Value) {
    let op = &raw["operation"];
    let Some(status) = raw["status"].as_str() else { return };
    let (kind, id, known_status) = match op["kind"].as_str() {
        Some("export") => ("export", &op["backup"]["id"], matches!(status,
            "stopping_source" | "exporting_source" | "stop_unknown" | "copying" | "incomplete_copy" | "exported")),
        Some("import") => ("import", &op["restore"]["id"], matches!(status,
            "importing" | "personalizing" | "stopping_copy" | "inactive" | "copy_unknown")),
        _ => return,
    };
    if !id.as_str().is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|u| u.to_string() == s)) {
        return;
    }
    view["operation"]["kind"] = json!(kind);
    if known_status { view["status"] = json!(status); }
}

fn archive_view(action: &str, raw: Value, redactor: &dyn DataRedactor) -> Value {
    let mut view = project_value(raw.clone(), redactor);
    match action {
        "environment.archive.exportPlan" | "environment.archive.importPlan" => {
            // Do not issue a usable review digest if the user cannot see the whole reviewed payload.
            if redactor.mode() == RedactionMode::MetadataOnly || view["payload"] != raw["payload"] {
                view["digest"] = Value::Null;
            }
        }
        "environment.archive.list" if redactor.mode() == RedactionMode::MetadataOnly => {
            if let (Some(records), Some(views)) = (raw["records"].as_array(), view["records"].as_array_mut()) {
                for (record, row) in records.iter().zip(views) { archive_receipt_controls(record, row); }
            }
        }
        "environment.archive.export" | "environment.archive.import" | "environment.archive.inspectStop"
            if redactor.mode() == RedactionMode::MetadataOnly => archive_receipt_controls(&raw, &mut view),
        _ => (),
    }
    view
}

pub(crate) fn project_response(action:&str,value:Value,store:&Store,redactor:&dyn DataRedactor)->Result<Value,String>{
    if action=="snapshot"{
        let has_key=value["settings"]["hasApiKey"].clone();
        let (state,bindings)=store.work_read(|db,state|Ok((state.clone(),crate::work_store::plan_bindings_view(db)?)))?;
        let mut output=snapshot_view(store,&state,redactor)?;
        output["settings"]["hasApiKey"]=has_key;
        output["workPlanBindings"]=project_value(json!(bindings),redactor);
        return Ok(output);
    }
    // These handlers construct their own exact reviewed representation (or
    // metadata DTO). A second truncation would misrepresent the approved bytes.
    if action.starts_with("data.")||action.starts_with("credential.reference.")||action.starts_with("evidence.access.")
        ||matches!(action,"ai.context"|"ai.budget.preview"|"evidence.sourcePreview"|"report.prepare"|"backup.prepare"|"kali.data.prepare"|"mcp.request"|"capture.access.prepare"|"capture.access.read"){
        return Ok(value);
    }
    let state=store.snapshot()?;
    let replaced=replace_evidence(store,&state,value,redactor)?;
    let mut projected=if action.starts_with("environment.archive.") {
        archive_view(action, replaced, redactor)
    } else { project_value(replaced,redactor) };
    if let Some(object)=projected.as_object_mut(){
        // An evidence mutation returns the same EvidenceView as snapshot/MCP.
        // Keep its precise redaction and omitted-field facts instead of replacing
        // them with the generic outer-response projection marker.
        let evidence_view=object.get("rawSha256").is_some_and(Value::is_string)
            &&object.get("rawBytes").is_some_and(Value::is_u64)
            &&object.get("projection").is_some_and(|p|p["schema"]=="openkoda.projection.v1");
        if !evidence_view{object.insert("projection".into(),projection_info(redactor,true,vec!["content","context","before","after"]));}
    }
    Ok(projected)
}

pub(crate) fn project_error(error:&str,redactor:&dyn DataRedactor)->String{
    if redactor.mode()!=RedactionMode::MetadataOnly{return project_text(error,redactor);}
    let code=error.split(':').next().unwrap_or_default();
    let known=(code.starts_with("data_")||code.starts_with("scope_")||code.starts_with("credential_")||crate::workflow::known_reason_code(code))&&code.len()<=80&&code.bytes().all(|b|b.is_ascii_lowercase()||b==b'_');
    if known{format!("{code}: conteúdo da mensagem indisponível; confira o armazenamento protegido da credencial.")}else{"Mensagem detalhada indisponível; confira o armazenamento protegido da credencial e o histórico da ação.".into()}
}

#[cfg(test)]
mod tests{
    use super::*;
    struct FixtureRedactor;
    impl DataRedactor for FixtureRedactor{
        fn mode(&self)->RedactionMode{RedactionMode::NoCredentialConfigured}
        fn redact(&self,text:&str)->RedactedText{let value=crate::ai::redact(text,None);RedactedText{changed:value!=text,secret_material:value.contains("[REDACTED"),text:value}}
    }
    fn store_with_evidence()->(tempfile::TempDir,Store,String,String){
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();
        let project=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Projection fixture","kind":"web","target":"https://fixture.invalid","description":""})}).unwrap();
        let assessment=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Projection","kind":"web","target":"https://fixture.invalid","objective":"Local fixture","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&[])})}).unwrap();
        let aid=assessment["id"].as_str().unwrap().to_owned();let eid=crate::model::id();
        let content=format!("Authorization: Bearer SYNTHETIC-SECRET\n{}\nUNRETURNED-END-MARKER","safe ".repeat(1200));
        store.mutate(|state|{state.assessments.iter_mut().find(|a|a.id==aid).unwrap().evidence.push(crate::model::Evidence{id:eid.clone(),name:"Stored evidence".into(),sha256:core::hash(&content),content,created_at:crate::model::now()});Ok(())}).unwrap();
        (root,store,aid,eid)
    }
    #[test]
    fn archive_projection_preserves_only_known_receipt_controls_and_reviewable_plans() {
        struct ArchiveRedactor(RedactionMode);
        impl DataRedactor for ArchiveRedactor {
            fn mode(&self) -> RedactionMode { self.0 }
            fn redact(&self, text: &str) -> RedactedText {
                let changed = text.contains("PRIVATE-SENTINEL");
                RedactedText { changed, secret_material: changed, text: text.replace("PRIVATE-SENTINEL", "[REDACTED]") }
            }
        }
        let (_dir, store, _, _) = store_with_evidence();
        let metadata = ArchiveRedactor(RedactionMode::MetadataOnly);
        let id = crate::model::id();
        let receipt = json!({"operation":{"kind":"export","backup":{"id":id,"source":{"name":"PRIVATE-SENTINEL"}}},
            "status":"exported","error":"PRIVATE-SENTINEL"});
        let clone = json!({"operation":{"kind":"import","restore":{"id":id}},"status":"inactive"});
        let invalid = json!({"operation":{"kind":"export","backup":{"id":"PRIVATE-SENTINEL"}},"status":"exported"});
        let mut bad_status = receipt.clone();
        bad_status["status"] = json!("PRIVATE-SENTINEL");
        let mut exporting = receipt.clone();
        exporting["status"] = json!("exporting_source");
        let records = json!({"records":[receipt,clone,invalid,bad_status,exporting]});
        let response = project_response("environment.archive.list", records, &store, &metadata).unwrap();
        assert_eq!(response["records"][0]["operation"]["kind"], "export");
        assert_eq!(response["records"][0]["status"], "exported");
        assert_eq!(response["records"][1]["operation"]["kind"], "import");
        assert_eq!(response["records"][1]["status"], "inactive");
        assert_ne!(response["records"][2]["operation"]["kind"], "export");
        assert_eq!(response["records"][4]["operation"]["kind"], "export");
        assert_eq!(response["records"][4]["status"], "exporting_source");
        assert!(!response.to_string().contains("PRIVATE-SENTINEL"));
        let other = project_response("other.action", receipt, &store, &metadata).unwrap();
        assert_ne!(other["operation"]["kind"], "export");
        assert_ne!(other["status"], "exported");

        let digest = core::hash("test review");
        let mut review = json!({"digest":digest,"payload":{"target":"safe destination","effects":"copy disk"}});
        let ready = ArchiveRedactor(RedactionMode::Ready);
        assert_eq!(archive_view("environment.archive.exportPlan", review.clone(), &ready)["digest"], digest);
        assert!(archive_view("environment.archive.importPlan", review.clone(), &metadata)["digest"].is_null());
        review["payload"]["target"] = json!("PRIVATE-SENTINEL");
        let hidden = archive_view("environment.archive.exportPlan", review, &ready);
        assert!(hidden["digest"].is_null());
        assert!(!hidden.to_string().contains("PRIVATE-SENTINEL"));
    }
    #[test]
    fn nested_mutations_and_snapshot_never_return_canonical_content(){
        let (_root,store,aid,eid)=store_with_evidence();let state=store.snapshot().unwrap();let original=&state.assessments[0].evidence[0];
        let raw=json!({"result":{"evidence":original}});
        let view=project_response("evidence.import",raw,&store,&FixtureRedactor).unwrap();
        assert!(view["result"]["evidence"].get("content").is_none());assert_eq!(view["result"]["evidence"]["rawSha256"],original.sha256);
        assert!(!view.to_string().contains("SYNTHETIC-SECRET"));assert!(!view.to_string().contains("UNRETURNED-END-MARKER"));
        let snapshot=project_response("snapshot",json!({"settings":{"hasApiKey":false}}),&store,&FixtureRedactor).unwrap();assert_eq!(snapshot["schema"],"openkoda.snapshot-view.v1");assert!(snapshot["assessments"][0]["evidence"][0].get("content").is_none());
        let mutation=project_response("evidence.import",json!(original),&store,&FixtureRedactor).unwrap();
        assert_eq!(mutation,snapshot["assessments"][0]["evidence"][0],"EvidenceView metadata is stable across native routes");
        assert_eq!(state.assessments[0].id,aid);assert_eq!(state.assessments[0].evidence[0].id,eid);
        assert_eq!(store.snapshot().unwrap().assessments[0].evidence[0].content,original.content);
    }
    #[test]
    fn mcp_has_no_raw_read_argument_or_fallback(){
        let (_root,store,aid,eid)=store_with_evidence();let executor=crate::executor::Executor::default();
        let payload=json!({"assessmentId":aid,"message":{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"evidence_read","arguments":{"evidenceId":eid}}}});
        let response=crate::mcp::handle_with_redactor(payload.clone(),&store,&executor,&FixtureRedactor).unwrap();
        let text=response["result"]["content"][0]["text"].as_str().unwrap();let view:Value=serde_json::from_str(text).unwrap();assert!(view.get("content").is_none());assert!(view["rawBytes"].as_u64().unwrap()>2048);assert!(!text.contains("SYNTHETIC-SECRET"));
        let mut bypass=payload;bypass["message"]["params"]["arguments"]["acknowledgeLocalRead"]=json!(true);assert!(crate::mcp::handle_with_redactor(bypass,&store,&executor,&FixtureRedactor).is_err());
    }
}
