//! Metadata routes and reviewed-window cursors. Cursors never grant data access.
use crate::{capture::{self,CaptureManifest,CaptureState,CaptureEncoding,CaptureCursor},core::{self,Store},data_host::JournalResolver,data_policy::{self,DataSubject,DerivedProducer},data_projection::{self,DataRedactor,RedactionMode},model,workflow};
use rusqlite::{params,Connection,OptionalExtension};
use serde::Deserialize;
use serde_json::{json,Value};
pub(crate) const MAX_PROJECTION_BYTES:usize=capture::MAX_PROJECTION_BYTES as usize;

pub(crate) fn supports(action:&str)->bool{matches!(action,"capture.list"|"capture.status")}
fn failure(_:impl std::fmt::Display)->String{workflow::error("capture_integrity")}
fn subject(aid:&str,cid:&str)->DataSubject{DataSubject::Derived{assessment_id:aid.into(),producer:DerivedProducer::Capture,object_id:cid.into()}}
fn view(store:&Store,manifest:CaptureManifest,redactor:&dyn DataRedactor)->Result<Value,String>{
    let source=subject(&manifest.assessment_id,&manifest.id);
    let state=store.snapshot()?;
    let available=manifest.state!=CaptureState::Purged;
    let sealed=matches!(manifest.state,CaptureState::Sealed|CaptureState::Interrupted);
    let fingerprint=core::hash(&json!(manifest).to_string());
    let classification=if sealed{
        let resolved=data_projection::resolve_subject(store,&state,&source,&JournalResolver{store})?;
        data_projection::classification_state(&state,&resolved)?
    }else{data_projection::ClassificationState{sensitivity:data_policy::Sensitivity::Unclassified,classification_ref:None,source_fingerprint:fingerprint.clone(),secret_material:false,contributor_refs:vec![],reason_codes:vec!["data_classification_required".into()]}};
    let can_prepare=sealed&&manifest.encoding==CaptureEncoding::Utf8&&redactor.mode()!=RedactionMode::MetadataOnly;
    let reasons=if !available{vec![workflow::WorkReason::new(workflow::WorkReasonCode::CaptureUnavailable,Some(manifest.id.clone()))]}
        else if !sealed{vec![workflow::WorkReason::new(workflow::WorkReasonCode::CaptureNotSealed,Some(manifest.id.clone()))]}
        else if manifest.encoding!=CaptureEncoding::Utf8{vec![workflow::WorkReason::new(workflow::WorkReasonCode::CaptureEncodingUnavailable,Some(manifest.id.clone()))]}else{vec![]};
    Ok(data_projection::project_value(json!({"manifest":manifest,"sourceFingerprint":fingerprint,"classification":classification,"available":available,"canPrepareRead":can_prepare,"reasons":reasons}),redactor))
}
pub(crate) fn handle(action:&str,payload:Value,store:&Store,redactor:&dyn DataRedactor)->Result<Value,String>{
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct List{assessment_id:String,#[serde(deserialize_with="workflow::required_nullable")]attempt_id:Option<String>}
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Status{assessment_id:String,capture_id:String}
    match action{
        "capture.list"=>{let input:List=workflow::decode(payload)?;let manifests=store.work_read(|db,s|{if !s.assessments.iter().any(|a|a.id==input.assessment_id){return Err(workflow::error("capture_unavailable"));}capture::list_locked(db,&input.assessment_id,input.attempt_id.as_deref())})?;Ok(json!({"captures":manifests.into_iter().map(|m|view(store,m,redactor)).collect::<Result<Vec<_>,_>>()?}))},
        "capture.status"=>{let input:Status=workflow::decode(payload)?;let manifest=store.work_read(|db,_|{let m=capture::load_locked(db,&input.capture_id)?;if m.assessment_id!=input.assessment_id{return Err(workflow::error("capture_unavailable"));}Ok(m)})?;view(store,manifest,redactor)},
        _=>Err(workflow::error("work_invalid_request")),
    }
}

pub(crate) struct AccessSelection{pub subject:DataSubject,pub fingerprint:String,pub offset:usize,pub maximum:usize,pub projection:Option<String>,pub manifest_digest:String}
pub(crate) fn prepare_selection(store:&Store,payload:Value)->Result<AccessSelection,String>{
    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Input{assessment_id:String,capture_id:String,expected_manifest_digest:String,#[serde(deserialize_with="workflow::required_nullable")]cursor_id:Option<String>,max_bytes:usize}
    let input:Input=workflow::decode(payload)?;
    core::valid_id(&input.assessment_id)?;core::valid_id(&input.capture_id)?;
    if input.max_bytes==0||input.max_bytes>capture::MAX_CHUNK_BYTES||!workflow::valid_hash(&input.expected_manifest_digest){return Err(workflow::error("work_invalid_request"));}
    store.work_read(|db,_|{
        let m=capture::load_locked(db,&input.capture_id)?;
        if m.assessment_id!=input.assessment_id||m.digest!=input.expected_manifest_digest{return Err(workflow::error("capture_integrity"));}
        if !matches!(m.state,CaptureState::Sealed|CaptureState::Interrupted){return Err(workflow::error(if m.state==CaptureState::Purged{"capture_unavailable"}else{"capture_not_sealed"}));}
        if m.encoding!=CaptureEncoding::Utf8{return Err(workflow::error("capture_encoding_unavailable"));}
        let (offset,projection)=if let Some(id)=&input.cursor_id{
            core::valid_id(id)?;
            let row:Option<(String,String,String,String,usize)>=db.query_row("SELECT assessment_id,capture_id,manifest_digest,projection_digest,offset FROM capture_cursors WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(failure)?;
            let(aid,cid,digest,projection,offset)=row.ok_or_else(||workflow::error("capture_cursor_invalid"))?;
            if aid!=input.assessment_id||cid!=input.capture_id||digest!=m.digest{return Err(workflow::error("capture_cursor_invalid"));}
            (offset,Some(projection))
        }else{(0,None)};
        Ok(AccessSelection{subject:subject(&input.assessment_id,&input.capture_id),fingerprint:core::hash(&json!(m).to_string()),offset,maximum:input.max_bytes,projection,manifest_digest:m.digest})
    })
}

/// Runs in the same Store transaction as the completed read receipt.
pub(crate) fn record_cursor(db:&Connection,source:&DataSubject,context:&Value,has_more:bool)->Result<Option<CaptureCursor>,String>{
    let DataSubject::Derived{assessment_id,producer:DerivedProducer::Capture,object_id}=source else{return Err(workflow::error("capture_cursor_invalid"));};
    let m=capture::load_locked(db,object_id)?;
    if m.assessment_id!=*assessment_id||context["captureManifestDigest"]!=m.digest||!matches!(m.state,CaptureState::Sealed|CaptureState::Interrupted){return Err(workflow::error("capture_integrity"));}
    if !has_more{return Ok(None);}
    let count:u64=db.query_row("SELECT COUNT(*) FROM capture_cursors",[],|r|r.get(0)).map_err(failure)?;
    if count>=1000{return Err(workflow::error("work_limit"));}
    let offset=context["rangeEnd"].as_u64().ok_or_else(||workflow::error("capture_cursor_invalid"))?;
    let total=context["totalBytes"].as_u64().ok_or_else(||workflow::error("capture_cursor_invalid"))?;
    if offset>=total||total>MAX_PROJECTION_BYTES as u64{return Err(workflow::error("capture_cursor_invalid"));}
    let projection=context["projectionSha256"].as_str().filter(|v|workflow::valid_hash(v)).ok_or_else(||workflow::error("capture_cursor_invalid"))?;
    let cursor=CaptureCursor{id:model::id(),capture_id:object_id.clone(),manifest_digest:m.digest,projection_sha256:projection.into(),offset};
    db.execute("INSERT INTO capture_cursors(id,assessment_id,capture_id,manifest_digest,projection_digest,offset,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![cursor.id,assessment_id,cursor.capture_id,cursor.manifest_digest,cursor.projection_sha256,cursor.offset,model::now()]).map_err(failure)?;
    Ok(Some(cursor))
}
