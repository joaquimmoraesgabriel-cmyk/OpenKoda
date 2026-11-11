use crate::{capture::{self,CaptureManifest,CaptureState},capture_access,core::Store,data_host::{self,JournalResolver},data_policy::{self,DataPolicyService},data_projection::{DataRedactor,RedactedText,RedactionMode},model};
use serde_json::{json,Value};

const SECRET:&str="SYNTHETIC-CAPTURE-CREDENTIAL";
struct Redactor;
impl DataRedactor for Redactor{
    fn mode(&self)->RedactionMode{RedactionMode::Ready}
    fn redact(&self,text:&str)->RedactedText{let text2=text.replace(SECRET,"[REDACTED]");RedactedText{changed:text2!=text,secret_material:text.contains(SECRET),text:text2}}
    fn contains_managed_secret(&self,text:&str)->bool{text.contains(SECRET)}
}
fn call(service:&DataPolicyService,store:&Store,action:&str,payload:Value)->Result<Value,String>{
    let response=service.handle(action,payload,store,&Redactor,&JournalResolver{store},None)?;
    data_host::project_response(action,response,store,&Redactor)
}
fn selected(aid:&str)->DataPolicyService{let service=DataPolicyService::new();service.select_assessment(Some(aid)).unwrap();service}
fn prepare(service:&DataPolicyService,store:&Store,m:&CaptureManifest,cursor:Value,maximum:usize)->Result<Value,String>{call(service,store,"capture.access.prepare",json!({"assessmentId":m.assessment_id,"captureId":m.id,"expectedManifestDigest":m.digest,"cursorId":cursor,"maxBytes":maximum}))}
fn read(service:&DataPolicyService,store:&Store,preview:&Value)->Result<Value,String>{call(service,store,"capture.access.read",json!({"review":preview["review"],"acknowledgeLocalRead":true}))}

#[test]
fn complete_stream_is_redacted_before_chunk_and_window_boundaries_and_replay_has_no_text(){
    let text=format!("{}{}é🚀 fim", "x".repeat(capture::MAX_CHUNK_BYTES-10),SECRET);
    let(_dir,store,aid,m)=capture::fixture::build(text.as_bytes(),CaptureState::Sealed);
    let service=selected(&aid);let mut cursor=Value::Null;let mut collected=String::new();let mut offset=0;
    loop{
        let preview=prepare(&service,&store,&m,cursor.clone(),32768).unwrap();
        assert!(preview.get("text").is_none());assert_eq!(preview["rangeStart"],offset);
        let response=read(&service,&store,&preview).unwrap();let page=response["text"].as_str().unwrap();
        assert!(page.len()<=32768);assert!(!page.contains("SYNTHETIC-CAPTURE"));collected.push_str(page);
        assert_eq!(response["captureId"],m.id);assert_eq!(response["rawSha256"],m.preserved_sha256.as_ref().unwrap().as_str());
        let repeated=read(&service,&store,&preview).unwrap();assert_eq!(repeated["alreadyUsed"],true);assert!(repeated.get("text").is_none());assert!(repeated["cursor"].is_null());
        if response["hasMore"]==false{assert!(response["cursor"].is_null());break;}
        offset=response["rangeEnd"].as_u64().unwrap();cursor=response["cursor"]["id"].clone();assert!(cursor.is_string());
    }
    assert_eq!(collected,Redactor.redact(&text).text);
    store.work_read(|db,_|{assert_eq!(capture::preserved_locked(db,&m)?,text.as_bytes());assert_eq!(capture::load_locked(db,&m.id)?.digest,m.digest);Ok(())}).unwrap();
}

#[test]
fn capture_metadata_and_subject_lists_never_include_raw_stream(){
    let text=format!("CAPTURE-ONLY-RAW-MARKER {SECRET}");
    let(_dir,store,aid,m)=capture::fixture::build(text.as_bytes(),CaptureState::Sealed);
    let service=selected(&aid);
    for(action,payload)in[("capture.list",json!({"assessmentId":aid,"attemptId":null})),("capture.status",json!({"assessmentId":aid,"captureId":m.id}))]{
        let output=capture_access::handle(action,payload,&store,&Redactor).unwrap();let projected=data_host::project_response(action,output,&store,&Redactor).unwrap();
        assert!(!projected.to_string().contains("CAPTURE-ONLY-RAW-MARKER"));assert!(!projected.to_string().contains(SECRET));
    }
    let subjects=call(&service,&store,"data.classification.subjects",json!({"assessmentId":aid})).unwrap();
    assert!(subjects["subjects"].as_array().unwrap().iter().any(|s|s["subject"]["producer"]=="capture"));
    assert!(!subjects.to_string().contains("CAPTURE-ONLY-RAW-MARKER"));
    data_policy::test_support::authorize(&store,&service,&aid,&Redactor,&JournalResolver{store:&store},None).unwrap();
    let state=store.snapshot().unwrap();let record=state.data_classifications.iter().find(|r|matches!(&r.subject,data_policy::DataSubject::Derived{producer:data_policy::DerivedProducer::Capture,object_id,..} if object_id==&m.id)).unwrap();
    assert!(record.secret_material,"classification must inspect reconstructed original, not only the manifest");
}

#[test]
fn open_and_opaque_captures_have_metadata_but_no_raw_grant(){
    for(bytes,state)in[(b"still capturing".as_slice(),CaptureState::Open),(&[0xff,0xfe],CaptureState::Sealed)]{
        let(_dir,store,aid,m)=capture::fixture::build(bytes,state);let service=selected(&aid);
        let status=capture_access::handle("capture.status",json!({"assessmentId":aid,"captureId":m.id}),&store,&Redactor).unwrap();
        assert_eq!(status["canPrepareRead"],false);assert!(prepare(&service,&store,&m,Value::Null,32).is_err());
        assert!(store.snapshot().unwrap().data_use_receipts.is_empty());
    }
}

#[test]
fn capture_prepare_refuses_offset_bypass_unknown_cursors_cross_assessment_and_changed_manifest(){
    let(_dir,store,aid,m)=capture::fixture::build(b"fixture stream",CaptureState::Sealed);let service=selected(&aid);
    let subject=json!({"kind":"derived","assessmentId":aid,"producer":"capture","objectId":m.id});
    assert!(call(&service,&store,"data.access.prepare",json!({"subject":subject,"expectedSourceFingerprint":crate::core::hash(&json!(m).to_string()),"offset":1,"maxBytes":32})).unwrap_err().starts_with("capture_cursor_invalid"));
    assert!(prepare(&service,&store,&m,json!(model::id()),32).unwrap_err().starts_with("capture_cursor_invalid"));
    for(field,value)in[("assessmentId",json!(model::id())),("expectedManifestDigest",json!(crate::core::hash("different"))),("maxBytes",json!(32769)),("offset",json!(1))]{
        let mut request=json!({"assessmentId":aid,"captureId":m.id,"expectedManifestDigest":m.digest,"cursorId":null,"maxBytes":32});request[field]=value;
        assert!(call(&service,&store,"capture.access.prepare",request).is_err(),"{field}");
    }
    let missing=json!({"assessmentId":aid,"captureId":m.id,"expectedManifestDigest":m.digest,"maxBytes":32});
    assert!(call(&service,&store,"capture.access.prepare",missing).is_err());assert!(store.snapshot().unwrap().data_use_receipts.is_empty());
}

#[test]
fn byte_limit_inside_utf8_preserves_prefix_and_never_offers_an_invalid_text_grant(){
    let(_dir,store,aid,m)=capture::fixture::build(b"",CaptureState::Open);
    let mut sink=capture::CaptureSink::new(&store,m.clone(),10).unwrap();
    sink.record(&json!({"name":"é"})).unwrap();let(observed,loss)=sink.seal_info();drop(sink);
    let sealed=store.capture_transaction(|db|capture::seal_locked(db,&m.id,observed,loss,false)).unwrap();
    assert_eq!(sealed.encoding,capture::CaptureEncoding::Unknown);
    assert_eq!(sealed.loss_reason,Some(capture::CaptureLossReason::OutputLimit));
    let service=selected(&aid);
    let status=capture_access::handle("capture.status",json!({"assessmentId":aid,"captureId":sealed.id}),&store,&Redactor).unwrap();
    assert_eq!(status["canPrepareRead"],false);
    assert!(prepare(&service,&store,&sealed,Value::Null,32).is_err());
    assert!(store.snapshot().unwrap().data_use_receipts.is_empty());
    store.work_read(|db,_|{let bytes=capture::preserved_locked(db,&sealed)?;assert_eq!(bytes,&"{\"name\":\"é\"}".as_bytes()[..10]);assert!(std::str::from_utf8(&bytes).is_err());Ok(())}).unwrap();
}

#[test]
fn cursor_survives_reopen_but_prepared_grant_does_not(){
    let(dir,store,aid,m)=capture::fixture::build(b"abcdefghijklmnopqrstuvwxyz",CaptureState::Sealed);let service=selected(&aid);
    let first=prepare(&service,&store,&m,Value::Null,8).unwrap();let read_first=read(&service,&store,&first).unwrap();
    let cursor=read_first["cursor"]["id"].clone();let unused=prepare(&service,&store,&m,cursor.clone(),8).unwrap();
    let root=store.root.clone();drop(service);drop(store);
    let store=Store::open(root).unwrap();let service=selected(&aid);
    assert!(read(&service,&store,&unused).unwrap_err().starts_with("data_review_required"));
    let replay=read(&service,&store,&first).unwrap();assert_eq!(replay["alreadyUsed"],true);assert!(replay.get("text").is_none());
    let next=prepare(&service,&store,&m,cursor,8).unwrap();assert_eq!(next["rangeStart"],8);
    assert_eq!(read(&service,&store,&next).unwrap()["text"],"ijklmnop");drop(store);drop(dir);
}

#[test]
fn view_change_blocks_capture_read_without_adding_receipt_or_cursor(){
    let(_dir,store,aid,m)=capture::fixture::build(b"long enough for cursor",CaptureState::Sealed);let service=selected(&aid);
    let preview=prepare(&service,&store,&m,Value::Null,8).unwrap();service.select_assessment(None).unwrap();
    assert!(read(&service,&store,&preview).unwrap_err().starts_with("data_view_changed"));
    assert!(store.snapshot().unwrap().data_use_receipts.is_empty());
    store.work_read(|db,_|{assert_eq!(db.query_row("SELECT COUNT(*) FROM capture_cursors",[],|r|r.get::<_,u64>(0)).unwrap(),0);Ok(())}).unwrap();
}
