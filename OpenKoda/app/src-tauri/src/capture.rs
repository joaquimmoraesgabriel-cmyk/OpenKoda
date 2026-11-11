//! Append-only adapter observations. Raw bytes never belong in list/status DTOs.
use crate::{core::{self, Store}, data_policy::DataOriginRef, model::{self, Execution, Snapshot}, workflow::{self, EffectRef, ResultRef}};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub const MAX_CHUNK_BYTES: usize = 32 * 1024;
pub const MAX_CHUNKS: u64 = 128;
pub const MAX_CAPTURE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_PROFILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PROJECTION_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 64 * 1024;

#[cfg(test)]
#[path="capture_fixture.rs"]
pub(crate) mod fixture;

macro_rules! schema {
    ($($name:literal => $ddl:literal),+ $(,)?) => {
        pub(crate) const CAPTURE_SCHEMA: &[(&str, &str)] = &[$(($name, $ddl)),+];
        pub(crate) const CAPTURE_SCHEMA_SQL: &str = concat!($($ddl, ";",)+);
    }
}
schema! {
    "capture_manifests" => "CREATE TABLE IF NOT EXISTS capture_manifests (id TEXT PRIMARY KEY NOT NULL, assessment_id TEXT NOT NULL, attempt_id TEXT NOT NULL REFERENCES work_attempts(id), state TEXT NOT NULL CHECK(state IN ('open','sealed','interrupted','purged')), generation INTEGER NOT NULL CHECK(generation>0), max_bytes INTEGER NOT NULL CHECK(max_bytes>0 AND max_bytes<=4194304), data TEXT NOT NULL)",
    "capture_chunks" => "CREATE TABLE IF NOT EXISTS capture_chunks (capture_id TEXT NOT NULL REFERENCES capture_manifests(id), chunk_index INTEGER NOT NULL CHECK(chunk_index>=0), offset INTEGER NOT NULL CHECK(offset>=0), sha256 TEXT NOT NULL CHECK(length(sha256)=64), bytes BLOB NOT NULL CHECK(length(bytes)>0 AND length(bytes)<=32768), PRIMARY KEY(capture_id,chunk_index))",
    "capture_cursors" => "CREATE TABLE IF NOT EXISTS capture_cursors (id TEXT PRIMARY KEY NOT NULL, assessment_id TEXT NOT NULL, capture_id TEXT NOT NULL REFERENCES capture_manifests(id), manifest_digest TEXT NOT NULL, projection_digest TEXT NOT NULL, offset INTEGER NOT NULL CHECK(offset>=0), created_at TEXT NOT NULL)"
}

macro_rules! enumeration {
    ($name:ident {$($variant:ident),+}) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all="snake_case")]
        pub enum $name {$($variant),+}
    }
}
enumeration!(CaptureState { Open, Sealed, Interrupted, Purged });
enumeration!(CaptureStream { AdapterRecords, ToolStdout, ToolStderr, SupervisorDiagnostic });
enumeration!(CaptureEncoding { Utf8, Unknown });
enumeration!(CaptureRepresentation { AdapterObservationRecords, ToolBytes, DiagnosticBytes });
enumeration!(CaptureLossReason { OutputLimit, DiskError, Interrupted, ParserError, EncodingUnknown });
impl CaptureState { fn as_str(self)->&'static str { match self {Self::Open=>"open",Self::Sealed=>"sealed",Self::Interrupted=>"interrupted",Self::Purged=>"purged"} } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct OmittedRange {
    pub start:u64,
    #[serde(deserialize_with="workflow::required_nullable")] pub end:Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CaptureManifest {
    pub schema:String, pub id:String, pub assessment_id:String, pub attempt_id:String,
    pub effect:EffectRef, pub producer_version:String,
    #[serde(deserialize_with="workflow::required_nullable")] pub parser_version:Option<String>,
    pub stream:CaptureStream, pub encoding:CaptureEncoding, pub representation:CaptureRepresentation,
    pub state:CaptureState, pub generation:u64, pub preserved_bytes:u64,
    #[serde(deserialize_with="workflow::required_nullable")] pub preserved_sha256:Option<String>,
    pub chunk_count:u64,
    #[serde(deserialize_with="workflow::required_nullable")] pub observed_bytes:Option<u64>,
    pub omitted_ranges:Vec<OmittedRange>,
    #[serde(deserialize_with="workflow::required_nullable")] pub loss_reason:Option<CaptureLossReason>,
    pub input_refs:Vec<ResultRef>, pub origin_refs:Vec<DataOriginRef>, pub source_verified:bool,
    pub started_at:String,
    #[serde(deserialize_with="workflow::required_nullable")] pub sealed_at:Option<String>,
    pub digest:String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CaptureCursor { pub id:String,pub capture_id:String,pub manifest_digest:String,pub projection_sha256:String,pub offset:u64 }

fn error(code:&str)->String {format!("{code}: captura indisponível ou inconsistente; nenhum adaptador foi repetido.")}
fn db_error(_:impl std::fmt::Display)->String {error("work_storage_unavailable")}
fn integrity()->String {error("capture_integrity")}
fn bytes_hash(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
pub(crate) fn manifest_digest(manifest:&CaptureManifest)->Result<String,String> {
    let mut value=serde_json::to_value(manifest).map_err(db_error)?;
    value.as_object_mut().ok_or_else(integrity)?.remove("digest");
    Ok(core::hash(&serde_json::to_string(&value).map_err(db_error)?))
}
fn encode_manifest(manifest:&CaptureManifest)->Result<String,String> {
    let text=serde_json::to_string(manifest).map_err(db_error)?;
    if text.len()>MAX_MANIFEST_BYTES{return Err(error("work_capture_quota"));}
    Ok(text)
}
fn persist(conn:&Connection,manifest:&mut CaptureManifest)->Result<(),String> {
    manifest.digest=manifest_digest(manifest)?;
    let changed=conn.execute("UPDATE capture_manifests SET state=?1,generation=?2,data=?3 WHERE id=?4",params![manifest.state.as_str(),manifest.generation,encode_manifest(manifest)?,manifest.id]).map_err(db_error)?;
    if changed!=1{return Err(integrity());} Ok(())
}
pub(crate) fn load_locked(conn:&Connection,capture_id:&str)->Result<CaptureManifest,String> {
    core::valid_id(capture_id)?;
    let text:Option<String>=conn.query_row("SELECT data FROM capture_manifests WHERE id=?1",[capture_id],|row|row.get(0)).optional().map_err(db_error)?;
    let text=text.ok_or_else(||error("capture_unavailable"))?;
    if text.len()>MAX_MANIFEST_BYTES{return Err(integrity());}
    serde_json::from_str(&text).map_err(|_|integrity())
}
pub(crate) fn list_locked(conn:&Connection,assessment_id:&str,attempt_id:Option<&str>)->Result<Vec<CaptureManifest>,String> {
    core::valid_id(assessment_id)?; if let Some(attempt)=attempt_id {core::valid_id(attempt)?;}
    let mut statement=conn.prepare("SELECT data FROM capture_manifests WHERE assessment_id=?1 AND (?2 IS NULL OR attempt_id=?2) ORDER BY id").map_err(db_error)?;
    let rows=statement.query_map(params![assessment_id,attempt_id],|row|row.get::<_,String>(0)).map_err(db_error)?;
    rows.map(|row|{let text=row.map_err(db_error)?;if text.len()>MAX_MANIFEST_BYTES{return Err(integrity());}serde_json::from_str(&text).map_err(|_|integrity())}).collect()
}

/// Called only in the same domain transaction which binds a durable attempt.
pub(crate) fn create_native_locked(conn:&Connection,state:&Snapshot,execution:&Execution,attempt_id:&str,max_bytes:u64)->Result<CaptureManifest,String> {
    if max_bytes==0||max_bytes>MAX_CAPTURE_BYTES{return Err(error("work_capture_quota"));}
    core::valid_id(attempt_id)?;
    let plan=state.plans.iter().find(|p|p.id==execution.plan_id&&p.assessment_id==execution.assessment_id).ok_or_else(integrity)?;
    if max_bytes>u64::from(plan.output_limit_ki_b)*1024||!state.executions.iter().any(|e|e.id==execution.id&&e.status=="running") {return Err(integrity());}
    let admission=execution.scope_admission.as_ref().ok_or_else(integrity)?;
    let binding:Option<(String,String,String)>=conn.query_row("SELECT j.assessment_id,a.effect_kind,a.effect_id FROM work_attempts a JOIN work_jobs j ON j.id=a.job_id WHERE a.id=?1",[attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
    if binding!=Some((execution.assessment_id.clone(),"native_execution".into(),execution.id.clone())){return Err(integrity());}
    let reserved:u64=conn.query_row("SELECT COALESCE(SUM(CASE WHEN state='open' THEN max_bytes WHEN state='purged' THEN 0 ELSE json_extract(data,'$.preservedBytes') END),0) FROM capture_manifests",[],|r|r.get(0)).map_err(db_error)?;
    if reserved.checked_add(max_bytes).is_none_or(|n|n>MAX_PROFILE_BYTES){return Err(error("work_capture_quota"));}
    let duplicate:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM capture_manifests WHERE attempt_id=?1)",[attempt_id],|r|r.get(0)).map_err(db_error)?;
    if duplicate{return Err(integrity());}
    let parser=match plan.adapter.as_str(){crate::source_scan::ADAPTER=>crate::source_scan::RULES_VERSION,crate::network::ADAPTER=>"openkoda.tcp-observation.v1","http.headers.v1"=>crate::http_validation::OBSERVATION_SCHEMA,_=>plan.adapter.as_str()};
    let mut manifest=CaptureManifest {schema:"openkoda.capture.v1".into(),id:model::id(),assessment_id:execution.assessment_id.clone(),attempt_id:attempt_id.into(),effect:EffectRef::NativeExecution{id:execution.id.clone()},producer_version:format!("{}@{}",plan.adapter,env!("CARGO_PKG_VERSION")),parser_version:Some(parser.into()),stream:CaptureStream::AdapterRecords,encoding:CaptureEncoding::Utf8,representation:CaptureRepresentation::AdapterObservationRecords,state:CaptureState::Open,generation:1,preserved_bytes:0,preserved_sha256:None,chunk_count:0,observed_bytes:None,omitted_ranges:vec![],loss_reason:None,input_refs:vec![],origin_refs:vec![DataOriginRef{scope_ref:admission.scope_ref.clone(),target_ref:admission.target_ref.clone()}],source_verified:true,started_at:model::now(),sealed_at:None,digest:String::new()};
    manifest.digest=manifest_digest(&manifest)?;
    conn.execute("INSERT INTO capture_manifests(id,assessment_id,attempt_id,state,generation,max_bytes,data) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![manifest.id,manifest.assessment_id,manifest.attempt_id,manifest.state.as_str(),manifest.generation,max_bytes,encode_manifest(&manifest)?]).map_err(db_error)?;
    Ok(manifest)
}

/// An identical committed chunk may be retried; gaps, conflicting retries and
pub(crate) fn create_kali_locked(conn:&Connection,binding:&crate::kali_bridge::KaliWorkBinding)->Result<Vec<CaptureManifest>,String>{
    binding.validate()?;
    let attempt=crate::work_store::attempt(conn,binding.attempt_id())?;
    if attempt.view.effect!=Some(EffectRef::KaliRun{id:binding.run_id().into()})||attempt.input_refs.as_slice()!=binding.input_refs(){return Err(integrity());}
    let duplicate:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM capture_manifests WHERE attempt_id=?1)",[binding.attempt_id()],|r|r.get(0)).map_err(db_error)?;
    if duplicate{return Err(integrity());}
    let reserved:u64=conn.query_row("SELECT COALESCE(SUM(CASE WHEN state='open' THEN max_bytes WHEN state='purged' THEN 0 ELSE json_extract(data,'$.preservedBytes') END),0) FROM capture_manifests",[],|r|r.get(0)).map_err(db_error)?;
    let bytes=binding.captures.iter().map(|slot|slot.max_bytes).sum::<u64>();
    if reserved.checked_add(bytes).is_none_or(|n|n>MAX_PROFILE_BYTES){return Err(error("work_capture_quota"));}
    let mut out=vec![];
    for slot in &binding.captures {
        let mut m=CaptureManifest{schema:"openkoda.capture.v1".into(),id:slot.capture_id.clone(),assessment_id:binding.assessment_id().into(),attempt_id:binding.attempt_id().into(),effect:EffectRef::KaliRun{id:binding.run_id().into()},producer_version:format!("kali.executor@{}",binding.executor_source_sha256),parser_version:binding.parser_version.clone(),stream:slot.stream,encoding:CaptureEncoding::Utf8,representation:match slot.stream{CaptureStream::ToolStdout=>CaptureRepresentation::ToolBytes,CaptureStream::SupervisorDiagnostic=>CaptureRepresentation::DiagnosticBytes,CaptureStream::AdapterRecords=>CaptureRepresentation::AdapterObservationRecords,_=>return Err(integrity())},state:CaptureState::Open,generation:1,preserved_bytes:0,preserved_sha256:None,chunk_count:0,observed_bytes:None,omitted_ranges:vec![],loss_reason:None,input_refs:binding.input_refs.clone(),origin_refs:binding.origin_refs.clone(),source_verified:true,started_at:model::now(),sealed_at:None,digest:String::new()};
        m.digest=manifest_digest(&m)?;
        conn.execute("INSERT INTO capture_manifests(id,assessment_id,attempt_id,state,generation,max_bytes,data) VALUES(?1,?2,?3,'open',1,?4,?5)",params![m.id,m.assessment_id,m.attempt_id,slot.max_bytes,encode_manifest(&m)?]).map_err(db_error)?;
        out.push(m);
    }Ok(out)
}
pub(crate) fn mark_encoding_unknown_locked(conn:&Connection,capture_id:&str)->Result<(),String>{let mut m=load_locked(conn,capture_id)?;if m.state!=CaptureState::Open{return Err(integrity());}if m.encoding!=CaptureEncoding::Unknown{m.encoding=CaptureEncoding::Unknown;m.generation+=1;persist(conn,&mut m)?;}Ok(())}

/// An identical committed chunk may be retried; gaps, conflicting retries and
/// appends after sealing are refused before changing any data.
pub(crate) fn append_locked(conn:&Connection,capture_id:&str,index:u64,offset:u64,bytes:&[u8])->Result<CaptureManifest,String> {
    if bytes.is_empty()||bytes.len()>MAX_CHUNK_BYTES{return Err(integrity());}
    let mut manifest=load_locked(conn,capture_id)?;
    if manifest.state!=CaptureState::Open{return Err(error("capture_not_sealed"));}
    let existing:Option<(u64,String,Vec<u8>)>=conn.query_row("SELECT offset,sha256,bytes FROM capture_chunks WHERE capture_id=?1 AND chunk_index=?2",params![capture_id,index],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
    if let Some((old_offset,hash,old_bytes))=existing {return if old_offset==offset&&hash==bytes_hash(bytes)&&old_bytes==bytes {Ok(manifest)}else{Err(integrity())};}
    let cap:u64=conn.query_row("SELECT max_bytes FROM capture_manifests WHERE id=?1",[capture_id],|r|r.get(0)).map_err(db_error)?;
    if index!=manifest.chunk_count||index>=MAX_CHUNKS||offset!=manifest.preserved_bytes||offset.checked_add(bytes.len() as u64).is_none_or(|n|n>cap){return Err(integrity());}
    conn.execute("INSERT INTO capture_chunks(capture_id,chunk_index,offset,sha256,bytes) VALUES(?1,?2,?3,?4,?5)",params![capture_id,index,offset,bytes_hash(bytes),bytes]).map_err(db_error)?;
    manifest.chunk_count+=1; manifest.preserved_bytes+=bytes.len() as u64; manifest.generation+=1;
    persist(conn,&mut manifest)?; Ok(manifest)
}

/// Appends a byte range, filling the open tail chunk without changing its
/// prefix. The caller's expected offset makes a lost acknowledgement safe to
/// retry. A sealed stream cannot be extended or rewritten.
pub(crate) fn append_stream_locked(conn:&Connection,capture_id:&str,offset:u64,bytes:&[u8])->Result<CaptureManifest,String> {
    if bytes.is_empty()||bytes.len()>MAX_CHUNK_BYTES{return Err(integrity());}
    let mut manifest=load_locked(conn,capture_id)?;
    if manifest.state!=CaptureState::Open{return Err(integrity());}
    let end=offset.checked_add(bytes.len() as u64).ok_or_else(integrity)?;
    if offset<manifest.preserved_bytes {
        if end>manifest.preserved_bytes{return Err(integrity());}
        let existing=preserved_locked(conn,&manifest)?;
        return if existing.get(offset as usize..end as usize)==Some(bytes){Ok(manifest)}else{Err(integrity())};
    }
    let cap:u64=conn.query_row("SELECT max_bytes FROM capture_manifests WHERE id=?1",[capture_id],|r|r.get(0)).map_err(db_error)?;
    if offset!=manifest.preserved_bytes||end>cap{return Err(integrity());}
    let mut remaining=bytes;
    if manifest.chunk_count>0 {
        let index=manifest.chunk_count-1;
        let (tail_offset,hash,mut tail):(u64,String,Vec<u8>)=conn.query_row("SELECT offset,sha256,bytes FROM capture_chunks WHERE capture_id=?1 AND chunk_index=?2",params![capture_id,index],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_error)?;
        if tail.is_empty()||tail.len()>MAX_CHUNK_BYTES||tail_offset+tail.len() as u64!=offset||hash!=bytes_hash(&tail){return Err(integrity());}
        let len=remaining.len().min(MAX_CHUNK_BYTES-tail.len());
        if len>0 {
            tail.extend_from_slice(&remaining[..len]);remaining=&remaining[len..];
            conn.execute("UPDATE capture_chunks SET sha256=?1,bytes=?2 WHERE capture_id=?3 AND chunk_index=?4 AND sha256=?5",params![bytes_hash(&tail),tail,capture_id,index,hash]).map_err(db_error)?;
        }
    }
    if !remaining.is_empty() {
        if manifest.chunk_count>=MAX_CHUNKS{return Err(integrity());}
        let next_offset=end-remaining.len() as u64;
        conn.execute("INSERT INTO capture_chunks(capture_id,chunk_index,offset,sha256,bytes) VALUES(?1,?2,?3,?4,?5)",params![capture_id,manifest.chunk_count,next_offset,bytes_hash(remaining),remaining]).map_err(db_error)?;
        manifest.chunk_count+=1;
    }
    manifest.preserved_bytes=end;manifest.generation+=1;persist(conn,&mut manifest)?;Ok(manifest)
}

/// Raw accessor for the data-policy resolver and backup scanner only. The
/// caller must admit a fresh, subject-bound data review before exposing bytes.
pub(crate) fn preserved_locked(conn:&Connection,manifest:&CaptureManifest)->Result<Vec<u8>,String> {
    if manifest.state==CaptureState::Purged{return Err(error("capture_unavailable"));}
    if manifest.preserved_bytes>MAX_CAPTURE_BYTES{return Err(integrity());}
    let mut output=Vec::with_capacity(manifest.preserved_bytes as usize);
    let mut statement=conn.prepare("SELECT chunk_index,offset,sha256,bytes FROM capture_chunks WHERE capture_id=?1 ORDER BY chunk_index").map_err(db_error)?;
    let rows=statement.query_map([&manifest.id],|r|Ok((r.get::<_,u64>(0)?,r.get::<_,u64>(1)?,r.get::<_,String>(2)?,r.get::<_,Vec<u8>>(3)?))).map_err(db_error)?;
    let mut count=0;
    for row in rows {let(index,offset,hash,bytes)=row.map_err(db_error)?;if index!=count||offset!=output.len() as u64||bytes.is_empty()||bytes.len()>MAX_CHUNK_BYTES||hash!=bytes_hash(&bytes)||output.len()+bytes.len()>MAX_CAPTURE_BYTES as usize{return Err(integrity());}output.extend_from_slice(&bytes);count+=1;}
    if count!=manifest.chunk_count||output.len() as u64!=manifest.preserved_bytes||manifest.preserved_sha256.as_ref().is_some_and(|h|h!=&bytes_hash(&output)){return Err(integrity());}
    Ok(output)
}

/// The execution, evidence, capture seal and workflow result commit together.
pub(crate) fn seal_locked(conn:&Connection,capture_id:&str,observed_bytes:Option<u64>,loss:Option<CaptureLossReason>,interrupted:bool)->Result<CaptureManifest,String> {
    let mut manifest=load_locked(conn,capture_id)?;
    if manifest.state!=CaptureState::Open{return Err(integrity());}
    if observed_bytes.is_some_and(|n|n<manifest.preserved_bytes||n>workflow::MAX_SAFE_INTEGER){return Err(integrity());}
    let bytes=preserved_locked(conn,&manifest)?;
    // A byte quota may cut through a UTF-8 scalar. Preserve that exact prefix,
    // but never advertise it as text which the guarded reader cannot decode.
    if std::str::from_utf8(&bytes).is_err(){manifest.encoding=CaptureEncoding::Unknown;}
    manifest.preserved_sha256=Some(bytes_hash(&bytes));manifest.observed_bytes=observed_bytes;
    manifest.state=if interrupted {CaptureState::Interrupted}else{CaptureState::Sealed};
    manifest.loss_reason=loss.or(if interrupted {Some(CaptureLossReason::Interrupted)}else if manifest.encoding==CaptureEncoding::Unknown {Some(CaptureLossReason::EncodingUnknown)}else{None});
    if observed_bytes.is_some_and(|n|n>manifest.preserved_bytes)||observed_bytes.is_none() {
        manifest.omitted_ranges=vec![OmittedRange{start:manifest.preserved_bytes,end:observed_bytes}];
    }
    if manifest.loss_reason.is_none()&&!manifest.omitted_ranges.is_empty(){return Err(integrity());}
    manifest.sealed_at=Some(model::now());manifest.generation+=1;persist(conn,&mut manifest)?;Ok(manifest)
}

/// Retention removes bytes only after a quiescent, integrity-checked terminal
/// attempt. The caller commits its retention receipt in this same transaction.
pub(crate) fn purge_locked(conn:&Connection,capture_id:&str,expected_manifest_digest:&str)->Result<CaptureManifest,String> {
    let mut manifest=load_locked(conn,capture_id)?;
    if !workflow::valid_hash(expected_manifest_digest)||manifest.digest!=expected_manifest_digest||manifest.digest!=manifest_digest(&manifest)? {return Err(integrity());}
    if manifest.state==CaptureState::Open{return Err(error("capture_not_sealed"));}
    let lease:Option<String>=conn.query_row("SELECT state FROM work_leases WHERE resource='assessment_work'",[],|r|r.get(0)).optional().map_err(db_error)?;
    if let Some(state)=lease{return Err(error(if state=="unknown"{"work_unknown_effect"}else{"work_busy"}));}
    let attempt=crate::work_store::attempt(conn,&manifest.attempt_id)?;
    if attempt.view.state.active()||attempt.view.state==workflow::AttemptState::Reserved||attempt.view.cleanup!=workflow::CleanupState::Confirmed{return Err(error("work_reconciliation_required"));}
    if manifest.state==CaptureState::Purged{return Ok(manifest);}
    preserved_locked(conn,&manifest)?;
    conn.execute("DELETE FROM capture_cursors WHERE capture_id=?1",[capture_id]).map_err(db_error)?;
    conn.execute("DELETE FROM capture_chunks WHERE capture_id=?1",[capture_id]).map_err(db_error)?;
    // Counts and hashes describe the historical seal, never available bytes.
    manifest.state=CaptureState::Purged;manifest.generation+=1;persist(conn,&mut manifest)?;Ok(manifest)
}

pub(crate) fn validate_capture_tables(conn:&Connection)->Result<(),String> {
    for (name,ddl) in CAPTURE_SCHEMA {let actual:Option<String>=conn.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",[name],|r|r.get(0)).optional().map_err(db_error)?;let expected=ddl.replace("CREATE TABLE IF NOT EXISTS ","CREATE TABLE ");if actual.as_deref()!=Some(expected.as_str()){return Err(integrity());}}
    let mut statement=conn.prepare("SELECT id,assessment_id,attempt_id,state,generation,max_bytes,data FROM capture_manifests ORDER BY id").map_err(db_error)?;
    let rows=statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,u64>(4)?,r.get::<_,u64>(5)?,r.get::<_,String>(6)?))).map_err(db_error)?;
    let mut reserved=0_u64;let mut metadata=0_usize;
    for row in rows {
        let(id,assessment,attempt,state,generation,cap,text)=row.map_err(db_error)?;
        metadata=metadata.checked_add(text.len()).ok_or_else(integrity)?;
        if text.len()>MAX_MANIFEST_BYTES||metadata>workflow::MAX_METADATA_BYTES{return Err(integrity());}
        let m:CaptureManifest=serde_json::from_str(&text).map_err(|_|integrity())?;
        if m.schema!="openkoda.capture.v1"||m.id!=id||m.assessment_id!=assessment||m.attempt_id!=attempt||m.state.as_str()!=state||m.generation!=generation||generation==0||generation>workflow::MAX_SAFE_INTEGER||cap==0||cap>MAX_CAPTURE_BYTES||m.preserved_bytes>cap||m.chunk_count>MAX_CHUNKS||m.producer_version.is_empty()||m.producer_version.len()>120||m.parser_version.as_ref().is_some_and(|p|p.is_empty()||p.len()>120)||!workflow::valid_time(&m.started_at)||m.digest!=manifest_digest(&m)?{return Err(integrity());}
        for id in [&m.id,&m.assessment_id,&m.attempt_id,m.effect.id()] {core::valid_id(id).map_err(|_|integrity())?;}
        if m.observed_bytes.is_some_and(|n|n<m.preserved_bytes||n>workflow::MAX_SAFE_INTEGER)||m.input_refs.len()>100||m.origin_refs.len()>32||m.omitted_ranges.len()>1{return Err(integrity());}
        for range in &m.omitted_ranges {if range.start!=m.preserved_bytes||range.end.is_some_and(|end|end<=range.start)||range.end!=m.observed_bytes{return Err(integrity());}}
        let binding:Option<(String,String,String)>=conn.query_row("SELECT j.assessment_id,a.effect_kind,a.effect_id FROM work_attempts a JOIN work_jobs j ON j.id=a.job_id WHERE a.id=?1",[&m.attempt_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
        if binding!=Some((m.assessment_id.clone(),m.effect.kind().into(),m.effect.id().into())){return Err(integrity());}
        if m.state==CaptureState::Open {if m.preserved_sha256.is_some()||m.sealed_at.is_some()||m.observed_bytes.is_some()||m.loss_reason.is_some()||!m.omitted_ranges.is_empty(){return Err(integrity());}reserved+=cap;}
        else {if !m.preserved_sha256.as_ref().is_some_and(|h|workflow::valid_hash(h))||!m.sealed_at.as_ref().is_some_and(|t|workflow::valid_time(t))||(!m.omitted_ranges.is_empty()&&m.loss_reason.is_none())||m.state==CaptureState::Interrupted&&m.loss_reason.is_none(){return Err(integrity());}if m.state!=CaptureState::Purged{reserved+=m.preserved_bytes;}}
        if reserved>MAX_PROFILE_BYTES{return Err(integrity());}
        if m.state==CaptureState::Purged {let count:u64=conn.query_row("SELECT COUNT(*) FROM capture_chunks WHERE capture_id=?1",[&m.id],|r|r.get(0)).map_err(db_error)?;if count!=0{return Err(integrity());}}
        else {preserved_locked(conn,&m)?;}
    }
    let orphan:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM capture_chunks c LEFT JOIN capture_manifests m ON m.id=c.capture_id WHERE m.id IS NULL)",[],|r|r.get(0)).map_err(db_error)?;
    if orphan{return Err(integrity());}
    let mut cursors=conn.prepare("SELECT id,assessment_id,capture_id,manifest_digest,projection_digest,offset,created_at FROM capture_cursors").map_err(db_error)?;
    let rows=cursors.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,u64>(5)?,r.get::<_,String>(6)?))).map_err(db_error)?;
    for row in rows {let(id,aid,cid,digest,projection,offset,created)=row.map_err(db_error)?;core::valid_id(&id).map_err(|_|integrity())?;let m=load_locked(conn,&cid)?;if m.assessment_id!=aid||m.digest!=digest||!workflow::valid_hash(&projection)||offset>MAX_PROJECTION_BYTES||!workflow::valid_time(&created)||m.state==CaptureState::Open{return Err(integrity());}}
    Ok(())
}

pub(crate) fn validate_capture_domain(conn:&Connection,state:&Snapshot)->Result<(),String> {
    let mut statement=conn.prepare("SELECT data FROM capture_manifests").map_err(db_error)?;
    let rows=statement.query_map([],|r|r.get::<_,String>(0)).map_err(db_error)?;
    for row in rows {
        let manifest:CaptureManifest=serde_json::from_str(&row.map_err(db_error)?).map_err(|_|integrity())?;
        if !state.assessments.iter().any(|a|a.id==manifest.assessment_id){return Err(integrity());}
        if let EffectRef::NativeExecution{id}=&manifest.effect {
            if manifest.stream!=CaptureStream::AdapterRecords||manifest.representation!=CaptureRepresentation::AdapterObservationRecords||!manifest.input_refs.is_empty(){return Err(integrity());}
            let execution=state.executions.iter().find(|e|e.id==*id&&e.assessment_id==manifest.assessment_id).ok_or_else(integrity)?;
            let admission=execution.scope_admission.as_ref().ok_or_else(integrity)?;
            let origins=vec![DataOriginRef{scope_ref:admission.scope_ref.clone(),target_ref:admission.target_ref.clone()}];
            if manifest.origin_refs!=origins||!manifest.source_verified{return Err(integrity());}
            if manifest.state==CaptureState::Open&&!matches!(execution.status.as_str(),"running"|"canceling"|"unknown"){return Err(integrity());}
            if manifest.state!=CaptureState::Open&&matches!(execution.status.as_str(),"running"|"canceling"){return Err(integrity());}
        }else if let EffectRef::KaliRun{id}=&manifest.effect {
            let attempt=crate::work_store::attempt(conn,&manifest.attempt_id)?;
            let binding=attempt.kali_binding.as_ref().ok_or_else(integrity)?;binding.validate()?;
            let slot=binding.captures.iter().find(|slot|slot.capture_id==manifest.id).ok_or_else(integrity)?;
            let representation=match slot.stream{CaptureStream::ToolStdout=>CaptureRepresentation::ToolBytes,CaptureStream::SupervisorDiagnostic=>CaptureRepresentation::DiagnosticBytes,CaptureStream::AdapterRecords=>CaptureRepresentation::AdapterObservationRecords,_=>return Err(integrity())};
            if binding.run_id()!=id||binding.assessment_id()!=manifest.assessment_id||binding.attempt_id()!=manifest.attempt_id||slot.stream!=manifest.stream||manifest.representation!=representation
                ||manifest.input_refs!=binding.input_refs||manifest.origin_refs!=binding.origin_refs||!manifest.source_verified
                ||manifest.producer_version!=format!("kali.executor@{}",binding.executor_source_sha256)||manifest.parser_version!=binding.parser_version
                ||manifest.state==CaptureState::Open&&!attempt.view.state.active(){return Err(integrity());}
        }else{return Err(integrity());}
        for origin in &manifest.origin_refs {crate::scope::validate_historical_ref(state,&manifest.assessment_id,&origin.scope_ref)?;crate::scope_targets::resolve_historical_target(state,&origin.scope_ref,origin.target_ref.as_ref())?;}
    } Ok(())
}

/// Backup inspects the complete bounded stream, not individual chunks. A
/// credential split over two blobs must be detected exactly like one blob.
pub(crate) fn validate_backup_streams(conn:&Connection,recognized_secret:&dyn Fn(&str)->bool,managed_secret:&dyn Fn(&str)->bool)->Result<(),String> {
    validate_capture_tables(conn)?;
    let mut statement=conn.prepare("SELECT data FROM capture_manifests").map_err(db_error)?;
    let rows=statement.query_map([],|r|r.get::<_,String>(0)).map_err(db_error)?;
    for row in rows {
        let manifest:CaptureManifest=serde_json::from_str(&row.map_err(db_error)?).map_err(|_|integrity())?;
        if manifest.state==CaptureState::Purged{continue;}
        let attempt_state:String=conn.query_row("SELECT state FROM work_attempts WHERE id=?1",[&manifest.attempt_id],|r|r.get(0)).map_err(db_error)?;
        if manifest.state==CaptureState::Open||matches!(attempt_state.as_str(),"reserved"|"dispatching"|"running"|"cancel_requested"|"unknown"){return Err(error("work_unknown_effect"));}
        if manifest.encoding!=CaptureEncoding::Utf8{return Err(error("capture_encoding_unavailable"));}
        let bytes=preserved_locked(conn,&manifest)?;
        let text=std::str::from_utf8(&bytes).map_err(|_|error("capture_encoding_unavailable"))?;
        if recognized_secret(text)||managed_secret(text){return Err(error("data_secret_material"));}
    }
    Ok(())
}

/// A bounded writer flushes each observation before the adapter aggregates or
/// truncates it. It holds no Store lock during adapter I/O or serialization.
pub(crate) struct CaptureSink<'a> { store:&'a Store,manifest:CaptureManifest,cap:u64,pending:Vec<u8>,observed:u64,loss:Option<CaptureLossReason>,failed:bool }
impl<'a> CaptureSink<'a> {
    pub(crate) fn new(store:&'a Store,manifest:CaptureManifest,cap:u64)->Result<Self,String> {
        if manifest.state!=CaptureState::Open||cap==0||cap>MAX_CAPTURE_BYTES||manifest.preserved_bytes>cap{return Err(integrity());}
        let observed=manifest.preserved_bytes;
        Ok(Self{store,manifest,cap,pending:Vec::with_capacity(MAX_CHUNK_BYTES),observed,loss:None,failed:false})
    }
    pub(crate) fn record(&mut self,value:&serde_json::Value)->Result<(),String> {
        if self.failed{return Err(error("work_storage_unavailable"));}
        // This marker is emitted only by the adapter for a selected opaque
        // header value. JSON's UTF-8 envelope does not make payload bytes text.
        if value["encoding"]=="unknown"&&self.manifest.encoding!=CaptureEncoding::Unknown {
            self.manifest=self.store.capture_transaction(|conn|{let mut manifest=load_locked(conn,&self.manifest.id)?;if manifest.state!=CaptureState::Open{return Err(integrity());}manifest.encoding=CaptureEncoding::Unknown;manifest.generation+=1;persist(conn,&mut manifest)?;Ok(manifest)})?;
            self.loss=Some(CaptureLossReason::EncodingUnknown);
        }
        if serde_json::to_writer(&mut *self,value).is_err()||self.write_all(b"\n").is_err()||self.flush().is_err(){self.failed=true;self.loss=Some(CaptureLossReason::DiskError);return Err(error("work_storage_unavailable"));}Ok(())
    }
    pub(crate) fn capture_id(&self)->&str {&self.manifest.id}
    pub(crate) fn seal_info(&self)->(Option<u64>,Option<CaptureLossReason>) {(Some(self.observed),self.loss)}
    fn flush_chunk(&mut self)->io::Result<()> {
        if self.pending.is_empty(){return Ok(());}
        match self.store.capture_transaction(|conn|{
            let manifest=append_stream_locked(conn,&self.manifest.id,self.manifest.preserved_bytes,&self.pending)?;
            if manifest.preserved_bytes/MAX_CHUNK_BYTES as u64>self.manifest.preserved_bytes/MAX_CHUNK_BYTES as u64 {crate::work_store::append_capture_event(conn,&manifest,false)?;}
            Ok(manifest)
        }) {Ok(manifest)=>{self.manifest=manifest;self.pending.clear();Ok(())},Err(_)=>{self.failed=true;self.loss=Some(CaptureLossReason::DiskError);Err(io::Error::other("capture persistence failed"))}}
    }
}
impl Write for CaptureSink<'_> {
    fn write(&mut self,bytes:&[u8])->io::Result<usize> {
        if self.failed{return Err(io::Error::other("capture persistence failed"));}
        self.observed=self.observed.checked_add(bytes.len() as u64).filter(|n|*n<=workflow::MAX_SAFE_INTEGER).ok_or_else(||io::Error::other("capture observation limit"))?;
        let available=(self.cap-self.manifest.preserved_bytes-self.pending.len() as u64) as usize;
        let preserve=available.min(bytes.len());if preserve<bytes.len(){self.loss=Some(CaptureLossReason::OutputLimit);}
        let mut rest=&bytes[..preserve];
        while !rest.is_empty() {let len=rest.len().min(MAX_CHUNK_BYTES-self.pending.len());self.pending.extend_from_slice(&rest[..len]);rest=&rest[len..];if self.pending.len()==MAX_CHUNK_BYTES{self.flush_chunk()?;}}
        Ok(bytes.len())
    }
    fn flush(&mut self)->io::Result<()> {self.flush_chunk()}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(cap:u64)->(Connection,CaptureManifest) {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE work_jobs(id TEXT PRIMARY KEY,assessment_id TEXT NOT NULL); CREATE TABLE work_attempts(id TEXT PRIMARY KEY,job_id TEXT NOT NULL,effect_kind TEXT NOT NULL,effect_id TEXT NOT NULL,state TEXT NOT NULL);").unwrap();
        conn.execute_batch(CAPTURE_SCHEMA_SQL).unwrap();
        let aid=model::id();let attempt=model::id();let effect=model::id();let job=model::id();
        conn.execute("INSERT INTO work_jobs VALUES(?1,?2)",params![job,aid]).unwrap();
        conn.execute("INSERT INTO work_attempts VALUES(?1,?2,'native_execution',?3,'completed')",params![attempt,job,effect]).unwrap();
        let mut m=CaptureManifest{schema:"openkoda.capture.v1".into(),id:model::id(),assessment_id:aid,attempt_id:attempt,effect:EffectRef::NativeExecution{id:effect},producer_version:"source.rules.v1".into(),parser_version:Some("source.rules.v1".into()),stream:CaptureStream::AdapterRecords,encoding:CaptureEncoding::Utf8,representation:CaptureRepresentation::AdapterObservationRecords,state:CaptureState::Open,generation:1,preserved_bytes:0,preserved_sha256:None,chunk_count:0,observed_bytes:None,omitted_ranges:vec![],loss_reason:None,input_refs:vec![],origin_refs:vec![],source_verified:true,started_at:model::now(),sealed_at:None,digest:String::new()};
        m.digest=manifest_digest(&m).unwrap();
        conn.execute("INSERT INTO capture_manifests VALUES(?1,?2,?3,'open',1,?4,?5)",params![m.id,m.assessment_id,m.attempt_id,cap,encode_manifest(&m).unwrap()]).unwrap();
        validate_capture_tables(&conn).unwrap();(conn,m)
    }
    #[test]
    fn chunk_retry_is_idempotent_but_conflicts_gaps_and_postseal_writes_fail() {
        let(conn,m)=fixture(1024);
        let first=append_locked(&conn,&m.id,0,0,b"first\n").unwrap();
        assert_eq!(append_locked(&conn,&m.id,0,0,b"first\n").unwrap(),first);
        assert!(append_locked(&conn,&m.id,0,0,b"other\n").is_err());
        assert!(append_locked(&conn,&m.id,2,6,b"gap").is_err());
        assert!(append_locked(&conn,&m.id,1,7,b"offset").is_err());
        append_locked(&conn,&m.id,1,6,b"last\n").unwrap();
        let seal=seal_locked(&conn,&m.id,Some(11),None,false).unwrap();
        assert_eq!(preserved_locked(&conn,&seal).unwrap(),b"first\nlast\n");
        assert_eq!(seal.preserved_sha256,Some(bytes_hash(b"first\nlast\n")));
        assert!(append_locked(&conn,&m.id,2,11,b"late").is_err());
        assert!(seal_locked(&conn,&m.id,Some(11),None,false).is_err());
        validate_capture_tables(&conn).unwrap();
    }
    #[test]
    fn byte_limit_cutting_utf8_seals_unknown_without_changing_prefix_or_loss(){
        let(conn,m)=fixture(10);let record="{\"name\":\"é\"}".as_bytes();let prefix=&record[..10];
        assert!(std::str::from_utf8(prefix).is_err());append_stream_locked(&conn,&m.id,0,prefix).unwrap();
        let sealed=seal_locked(&conn,&m.id,Some(record.len() as u64),Some(CaptureLossReason::OutputLimit),false).unwrap();
        assert_eq!(sealed.encoding,CaptureEncoding::Unknown);assert_eq!(sealed.loss_reason,Some(CaptureLossReason::OutputLimit));assert_eq!(sealed.preserved_sha256,Some(bytes_hash(prefix)));assert_eq!(preserved_locked(&conn,&sealed).unwrap(),prefix);validate_capture_tables(&conn).unwrap();
        assert!(validate_backup_streams(&conn,&|_|false,&|_|false).is_err());
    }
    #[test]
    fn incremental_tail_preserves_prefix_and_lost_ack_does_not_duplicate_bytes() {
        let(conn,m)=fixture(MAX_CAPTURE_BYTES);
        let first=vec![b'a';MAX_CHUNK_BYTES-3];
        append_stream_locked(&conn,&m.id,0,&first).unwrap();
        let second=append_stream_locked(&conn,&m.id,first.len() as u64,b"bcdef").unwrap();
        assert_eq!(second.chunk_count,2);
        assert_eq!(append_stream_locked(&conn,&m.id,first.len() as u64,b"bcdef").unwrap(),second);
        assert!(append_stream_locked(&conn,&m.id,first.len() as u64,b"BCDEF").is_err());
        let bytes=preserved_locked(&conn,&second).unwrap();assert_eq!(&bytes[..first.len()],first.as_slice());assert_eq!(&bytes[first.len()..],b"bcdef");
        let mut current=second;
        for _ in 0..300 {current=append_stream_locked(&conn,&m.id,current.preserved_bytes,b"x").unwrap();}
        assert_eq!(current.chunk_count,2);validate_capture_tables(&conn).unwrap();
    }
    #[test]
    fn four_mib_stream_uses_exactly_128_chunks_and_cannot_grow() {
        let(conn,m)=fixture(MAX_CAPTURE_BYTES);let chunk=vec![b'x';MAX_CHUNK_BYTES];let mut current=m;
        for _ in 0..MAX_CHUNKS {current=append_stream_locked(&conn,&current.id,current.preserved_bytes,&chunk).unwrap();}
        assert_eq!(current.chunk_count,MAX_CHUNKS);assert_eq!(current.preserved_bytes,MAX_CAPTURE_BYTES);
        assert!(append_stream_locked(&conn,&current.id,current.preserved_bytes,b"overflow").is_err());
        seal_locked(&conn,&current.id,Some(MAX_CAPTURE_BYTES),None,false).unwrap();validate_capture_tables(&conn).unwrap();
    }
    #[test]
    fn open_capture_reservations_cannot_exceed_the_profile_quota() {
        let(conn,m)=fixture(MAX_CAPTURE_BYTES);
        let job:String=conn.query_row("SELECT job_id FROM work_attempts WHERE id=?1",[&m.attempt_id],|r|r.get(0)).unwrap();
        for index in 1..5 {
            let mut next=m.clone();next.id=model::id();next.attempt_id=model::id();next.effect=EffectRef::NativeExecution{id:model::id()};next.digest=manifest_digest(&next).unwrap();
            conn.execute("INSERT INTO work_attempts VALUES(?1,?2,'native_execution',?3,'running')",params![next.attempt_id,job,next.effect.id()]).unwrap();
            conn.execute("INSERT INTO capture_manifests VALUES(?1,?2,?3,'open',1,?4,?5)",params![next.id,next.assessment_id,next.attempt_id,MAX_CAPTURE_BYTES,encode_manifest(&next).unwrap()]).unwrap();
            if index<4 {validate_capture_tables(&conn).unwrap();}else{assert!(validate_capture_tables(&conn).is_err());}
        }
    }
    #[test]
    fn bounded_chunks_preserve_prefix_and_failed_transaction_rolls_back() {
        let(mut conn,m)=fixture(8);
        append_locked(&conn,&m.id,0,0,b"prefix").unwrap();
        {let tx=conn.transaction().unwrap();append_locked(&tx,&m.id,1,6,b"ok").unwrap();assert!(append_locked(&tx,&m.id,2,8,b"overflow").is_err());}
        let after=load_locked(&conn,&m.id).unwrap();assert_eq!(preserved_locked(&conn,&after).unwrap(),b"prefix");
        let sealed=seal_locked(&conn,&m.id,Some(20),Some(CaptureLossReason::OutputLimit),true).unwrap();
        assert_eq!(sealed.omitted_ranges,vec![OmittedRange{start:6,end:Some(20)}]);
        assert_eq!(sealed.state,CaptureState::Interrupted);validate_capture_tables(&conn).unwrap();
    }
    #[test]
    fn corrupted_blob_digest_and_empty_wrong_schema_are_rejected() {
        let(conn,m)=fixture(1024);append_locked(&conn,&m.id,0,0,b"original").unwrap();
        seal_locked(&conn,&m.id,Some(8),None,false).unwrap();
        conn.execute("UPDATE capture_chunks SET bytes=?1 WHERE capture_id=?2",params![b"modified".as_slice(),m.id]).unwrap();
        assert!(validate_capture_tables(&conn).is_err());
        let(empty,_)=fixture(1024);empty.execute_batch("ALTER TABLE capture_chunks ADD COLUMN unexpected TEXT;").unwrap();assert!(validate_capture_tables(&empty).is_err());
    }
    #[test]
    fn backup_scans_reconstructed_stream_and_rejects_open_or_unknown_effects() {
        let(conn,m)=fixture(1024);
        append_locked(&conn,&m.id,0,0,b"prefix PRIVATE_").unwrap();append_locked(&conn,&m.id,1,15,b"CANARY suffix").unwrap();
        assert!(validate_backup_streams(&conn,&|_|false,&|_|false).is_err());
        seal_locked(&conn,&m.id,Some(28),None,false).unwrap();
        assert!(validate_backup_streams(&conn,&|text|text.contains("PRIVATE_CANARY"),&|_|false).unwrap_err().contains("data_secret_material"));
        validate_backup_streams(&conn,&|_|false,&|_|false).unwrap();
        conn.execute("UPDATE work_attempts SET state='unknown' WHERE id=?1",[&m.attempt_id]).unwrap();
        assert!(validate_backup_streams(&conn,&|_|false,&|_|false).unwrap_err().contains("work_unknown_effect"));
    }
    #[test]
    fn manifest_nullable_fields_are_required_and_unknown_fields_rejected() {
        let(_,m)=fixture(1024);let value=serde_json::to_value(m).unwrap();
        for field in ["parserVersion","preservedSha256","observedBytes","lossReason","sealedAt"] {let mut missing=value.clone();missing.as_object_mut().unwrap().remove(field);assert!(serde_json::from_value::<CaptureManifest>(missing).is_err(),"{field}");}
        let mut extra=value;extra["bytes"]=serde_json::json!("never in metadata");assert!(serde_json::from_value::<CaptureManifest>(extra).is_err());
    }
    #[test]
    fn backup_refuses_undecodable_preserved_bytes_without_exporting_them() {
        let(conn,m)=fixture(1024);append_locked(&conn,&m.id,0,0,&[0xff,0xfe]).unwrap();seal_locked(&conn,&m.id,Some(2),None,false).unwrap();
        assert!(validate_backup_streams(&conn,&|_|false,&|_|false).unwrap_err().contains("capture_encoding_unavailable"));
    }
    #[test]
    fn real_sink_preserves_exact_bounded_prefix_and_coalesces_progress_events() {
        let(_dir,store,_aid,manifest)=super::fixture::build(b"",CaptureState::Open);
        let cap=MAX_CHUNK_BYTES as u64+3;let mut sink=CaptureSink::new(&store,manifest.clone(),cap).unwrap();
        let record=serde_json::json!({"kind":"fixture_record","value":"x".repeat(MAX_CHUNK_BYTES+100)});
        let mut full=serde_json::to_vec(&record).unwrap();full.push(b'\n');sink.record(&record).unwrap();sink.record(&record).unwrap();
        assert_eq!(sink.seal_info(),(Some((full.len()*2) as u64),Some(CaptureLossReason::OutputLimit)));
        store.work_read(|conn,_|{let current=load_locked(conn,&manifest.id)?;assert_eq!(current.chunk_count,2);assert_eq!(preserved_locked(conn,&current)?,&full[..cap as usize]);
            let progress:u64=conn.query_row("SELECT COUNT(*) FROM work_events WHERE json_extract(data,'$.type')='capture_progress'",[],|r|r.get(0)).map_err(db_error)?;assert_eq!(progress,1);Ok(())}).unwrap();
    }
    #[test]
    fn opaque_adapter_marker_makes_the_whole_capture_unknown_before_bytes_append() {
        let(_dir,store,_aid,manifest)=super::fixture::build(b"",CaptureState::Open);let mut sink=CaptureSink::new(&store,manifest.clone(),MAX_CAPTURE_BYTES).unwrap();
        sink.record(&serde_json::json!({"kind":"http_header","name":"server","encoding":"unknown","valueBase64":"/w=="})).unwrap();
        assert_eq!(sink.seal_info().1,Some(CaptureLossReason::EncodingUnknown));
        assert_eq!(store.work_read(|conn,_|load_locked(conn,&manifest.id)).unwrap().encoding,CaptureEncoding::Unknown);
    }
    #[test]
    fn retention_preserves_the_seal_identity_but_removes_bytes_and_cursors() {
        let(_dir,store,_aid,manifest)=super::fixture::build(b"retention fixture bytes",CaptureState::Sealed);
        store.capture_transaction(|conn|conn.execute("INSERT INTO capture_cursors VALUES(?1,?2,?3,?4,?5,4,?6)",params![model::id(),manifest.assessment_id,manifest.id,manifest.digest,core::hash("retention fixture bytes"),model::now()]).map_err(db_error)).unwrap();
        let purged=store.work_transaction(|conn,_|purge_locked(conn,&manifest.id,&manifest.digest)).unwrap();
        assert_eq!(purged.state,CaptureState::Purged);assert_eq!(purged.preserved_sha256,manifest.preserved_sha256);assert_eq!(purged.preserved_bytes,manifest.preserved_bytes);assert_ne!(purged.digest,manifest.digest);
        store.work_read(|conn,_|{assert!(preserved_locked(conn,&purged).is_err());let chunks:u64=conn.query_row("SELECT COUNT(*) FROM capture_chunks WHERE capture_id=?1",[&manifest.id],|r|r.get(0)).map_err(db_error)?;let cursors:u64=conn.query_row("SELECT COUNT(*) FROM capture_cursors WHERE capture_id=?1",[&manifest.id],|r|r.get(0)).map_err(db_error)?;assert_eq!((chunks,cursors),(0,0));assert_eq!(purge_locked(conn,&manifest.id,&purged.digest)?,purged);Ok(())}).unwrap();
        let(_dir,store,_aid,open)=super::fixture::build(b"live bytes",CaptureState::Open);assert!(store.work_transaction(|conn,_|purge_locked(conn,&open.id,&open.digest)).is_err());
    }
}
