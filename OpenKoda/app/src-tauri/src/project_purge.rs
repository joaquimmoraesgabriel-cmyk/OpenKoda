//! Explicit logical deletion of one archived project's live profile graph.
//! Each SQLite database commits separately. Durable intent precedes each step;
//! startup and reconciliation never replay deletion. Journal data is metadata.
use crate::{core::{self,Store},model::{self,Snapshot}};
use rusqlite::{params,Connection,OptionalExtension,types::ValueRef};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet},fs::{self,File},path::{Component,Path,PathBuf},sync::Mutex,time::Duration};

const JOURNAL:&str="project-purge.sqlite3";
const PHRASE:&str="EXCLUIR PROJETO E HISTÓRICO";
const MAX_ROWS:usize=10000;
const MAX_BYTES:u64=128*1024*1024;
const MAX_PLANS:usize=100;
const LIMITS:[&str;6]=[
    "Exclusão lógica do projeto arquivado e de seu histórico no perfil ativo: avaliações/evidências/eventos, achados/validações, planos/execuções, conversas/requisições/investigações IA, correções/backups internos e sessões MCP vinculadas.",
    "Não remove arquivos fonte/alvos, Kali, credencial/configurações, testes globais de conexão, backups/exportações, outros perfis, cópias externas nem dados retidos pelo provedor.",
    "Os bancos SQLite confirmam etapas separadamente; a operação não é atômica entre bancos. Falha pode deixar histórico parcialmente removido. Não existe rollback automático.",
    "Uma interrupção bloqueia novas mutações até inspeção e decisão explícitas. Continuar exige o mesmo plano, digest e frase; nenhuma exclusão é repetida no reinício ou no diagnóstico.",
    "Esta exclusão não é apagamento seguro e não promete remover páginas livres/WAL, caches ou cópias. O recibo conserva somente identidades, hashes, contagens e estados.",
    "Histórico de consumo desconhecido reconhecido pelo usuário continua historicamente desconhecido; excluir seu registro local não cancela solicitações nem elimina cobranças.",
];

#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
#[serde(rename_all="camelCase")]
enum Database {Assistant,Remediation,Bridge,Core}
impl Database {fn name(self)->&'static str {match self {Self::Assistant=>"assistant.sqlite3",Self::Remediation=>"remediation.sqlite3",Self::Bridge=>"kali-bridge.sqlite3",Self::Core=>"openkoda.sqlite3"}}}
const DATABASES:[Database;4]=[Database::Assistant,Database::Remediation,Database::Bridge,Database::Core];
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Identity {volume:String,index:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Profile {path_sha256:String,identity:Identity}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Binding {database:Database,identity:Option<Identity>,schema_sha256:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct RowDigest {database:Database,table:String,id:String,sha256:String,bytes:u64}
#[derive(Clone,Debug,Serialize,Deserialize,Default,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Counts {projects:usize,assessments:usize,evidence:usize,events:usize,findings:usize,validations:usize,plans:usize,executions:usize,messages:usize,requests:usize,requests_unknown_acknowledged:usize,investigations:usize,remediations:usize,bridge_runs:usize,total_rows:usize,total_bytes:u64,#[serde(default,skip_serializing_if="zero")]data_metadata_records:usize,#[serde(default,skip_serializing_if="zero")]shared_data_receipts:usize}
fn zero(value:&usize)->bool{*value==0}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Step {database:Database,status:String,planned_rows:usize,removed_rows:usize}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Inspection {at:String,present:usize,absent:usize,changed:usize,new_rows:usize,unobserved:usize,complete:bool,conflict:bool,message:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Closure {at:String,author:String,note_sha256:String,note_bytes:usize,acknowledge_partial_deletion:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct PurgePlan {
    id:String,project_id:String,assessment_ids:Vec<String>,finding_ids:Vec<String>,profile:Profile,
    created_at:String,expires_at:String,digest:String,status:String,bindings:Vec<Binding>,rows:Vec<RowDigest>,counts:Counts,
    confirmation_phrase:String,limits:Vec<String>,steps:Vec<Step>,inspection:Option<Inspection>,closure:Option<Closure>,error:Option<String>,
}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Selection {project_id:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Apply {id:String,digest:String,confirmation:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Id {id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Close {id:String,digest:String,note:String,acknowledge_partial_deletion:bool}

pub struct ProjectPurgeService {root:PathBuf,profile:Profile,db:Mutex<Connection>,operation:Mutex<()>,_guards:Vec<File>,#[cfg(test)]fail_after_commit:Mutex<Option<Database>>}
struct OpenDatabase {kind:Database,db:Connection,_guard:File,in_transaction:bool}
impl Drop for OpenDatabase {fn drop(&mut self){if self.in_transaction{let _=self.db.execute_batch("ROLLBACK");}}}
struct Inventory {rows:Vec<RowDigest>,bindings:Vec<Binding>,counts:Counts,assessment_ids:Vec<String>,finding_ids:Vec<String>}

pub fn supports(action:&str)->bool {matches!(action,"projectPurge.status"|"projectPurge.get"|"projectPurge.plan"|"projectPurge.apply"|"projectPurge.reconcile"|"projectPurge.closePartial")}
fn failure(_:impl std::fmt::Display)->String {"Falha no expurgo do projeto. O journal foi preservado; consulte a inspeção antes de qualquer continuação explícita.".into()}
fn decode<T:serde::de::DeserializeOwned>(value:Value)->Result<T,String>{serde_json::from_value(value).map_err(|_|"Campos de expurgo inválidos; somente identidades tipadas são aceitas.".into())}
fn encode<T:Serialize>(value:T)->Result<Value,String>{serde_json::to_value(value).map_err(failure)}
fn uuid(value:&str)->Result<(),String>{if uuid::Uuid::parse_str(value).is_ok_and(|id|id.to_string()==value){Ok(())}else{Err("Identidade de expurgo inválida.".into())}}
fn sha(value:&str)->bool{value.len()==64&&value.bytes().all(|v|v.is_ascii_digit()||(b'a'..=b'f').contains(&v))}
fn unresolved(status:&str)->bool{matches!(status,"applying"|"unknown"|"partial"|"conflict")}
fn plan_digest(p:&PurgePlan)->String {core::hash(&json!({"id":p.id,"projectId":p.project_id,"assessmentIds":p.assessment_ids,"findingIds":p.finding_ids,
    "profile":p.profile,"createdAt":p.created_at,"expiresAt":p.expires_at,"bindings":p.bindings,"rows":p.rows,"counts":p.counts,
    "confirmationPhrase":p.confirmation_phrase,"limits":p.limits}).to_string())}
fn key(row:&RowDigest)->(Database,&str,&str){(row.database,&row.table,&row.id)}

// Table DDL is checked, not merely the columns used in DELETE. Unknown triggers,
// views, constraints, extra columns/tables or user-defined indexes fail closed.
const ASSISTANT_SCHEMA:[(&str,&str);5]=[
    ("vault","CREATE TABLE vault (id INTEGER PRIMARY KEY CHECK(id=1), cipher BLOB NOT NULL, revision TEXT NOT NULL)"),
    ("messages","CREATE TABLE messages (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, role TEXT NOT NULL, text TEXT NOT NULL, created TEXT NOT NULL)"),
    ("requests","CREATE TABLE requests (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, endpoint TEXT NOT NULL, model TEXT NOT NULL, revision TEXT NOT NULL, context_hash TEXT NOT NULL, status TEXT NOT NULL, created TEXT NOT NULL, input_tokens INTEGER, output_tokens INTEGER, details TEXT NOT NULL DEFAULT '{}')"),
    ("connection_check","CREATE TABLE connection_check (id INTEGER PRIMARY KEY CHECK(id=1), model TEXT NOT NULL, endpoint TEXT NOT NULL, revision TEXT NOT NULL, checked TEXT NOT NULL)"),
    ("investigations","CREATE TABLE investigations (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, data TEXT NOT NULL)"),
];
const REMEDIATION_SCHEMA:[(&str,&str);1]=[("remediation_plans","CREATE TABLE remediation_plans(id TEXT PRIMARY KEY, finding_id TEXT NOT NULL, data TEXT NOT NULL, backup BLOB)")];
const BRIDGE_SCHEMA:[(&str,&str);1]=[("bridge_runs","CREATE TABLE bridge_runs(id TEXT PRIMARY KEY,data TEXT NOT NULL)")];
const CORE_SCHEMA:[(&str,&str);1]=[("app_state","CREATE TABLE app_state (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL)")];
const JOURNAL_SCHEMA:[(&str,&str);1]=[("project_purge_plans","CREATE TABLE project_purge_plans(id TEXT PRIMARY KEY,data TEXT NOT NULL)")];
fn normalize_sql(sql:&str)->String {sql.chars().filter(|c|!c.is_ascii_whitespace()).flat_map(char::to_lowercase).collect::<String>().replace("ifnotexists","").trim_end_matches(';').into()}
fn schema(db:&Connection,expected:&[(&str,&str)])->Result<String,String>{
    let version:i64=db.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(failure)?;
    let expected_version=if expected.iter().any(|(name,_)|*name=="app_state"){core::STORE_VERSION}else{0};
    let multi_provider=version==1&&expected.iter().any(|(name,_)|*name=="vault");
    if version!=expected_version&&!multi_provider{return Err("Versão de banco não reconhecida para expurgo; dados preservados.".into());}
    let integrity:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(failure)?;
    if integrity!="ok"{return Err("Banco inconsistente; nenhum registro será expurgado.".into());}
    // SQLite itself derives the exact implicit indexes from trusted, versioned
    // DDL. Accepting names by prefix would miss changed UNIQUE constraints.
    let reference=Connection::open_in_memory().map_err(failure)?;
    for (table,ddl) in expected {
        let ddl=match (*table,multi_provider){
            ("vault",true)=>crate::ai::VAULT_SCHEMA,
            ("connection_check",true)=>crate::ai::CHECK_SCHEMA,
            _=>ddl
        };
        reference.execute_batch(ddl).map_err(failure)?;
    }
    fn objects(db:&Connection)->Result<Vec<(String,String,String,Option<String>)>,String>{
        let mut statement=db.prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").map_err(failure)?;
        let rows=statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?.map(|s|normalize_sql(&s))))).map_err(failure)?;
        rows.collect::<Result<Vec<_>,_>>().map_err(failure)
    }
    let actual=objects(db)?;
    if actual!=objects(&reference)?{return Err("Schema de banco contém objeto desconhecido ou alterado; dados preservados.".into());}
    // Keep the historical fingerprint representation for pre-workflow tables.
    let fingerprint=actual.into_iter().filter(|(kind,_,_,_)|kind=="table").map(|(_,name,_,sql)|(name,sql.unwrap_or_default())).collect::<Vec<_>>();
    Ok(core::hash(&serde_json::to_string(&fingerprint).map_err(failure)?))
}
fn expected_schema(kind:Database)->Vec<(&'static str,&'static str)>{match kind{Database::Assistant=>ASSISTANT_SCHEMA.to_vec(),Database::Remediation=>REMEDIATION_SCHEMA.to_vec(),Database::Bridge=>BRIDGE_SCHEMA.to_vec(),Database::Core=>CORE_SCHEMA.iter().chain(crate::work_store::WORK_SCHEMA).chain(crate::capture::CAPTURE_SCHEMA).copied().collect()}}

#[cfg(windows)]
fn node(path:&Path,directory:bool)->Result<(File,Identity),String>{
    use std::os::windows::{fs::OpenOptionsExt,io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS,FILE_FLAG_OPEN_REPARSE_POINT,FILE_ATTRIBUTE_DIRECTORY,FILE_ATTRIBUTE_REPARSE_POINT,GetFileInformationByHandle};
    let flags=FILE_FLAG_OPEN_REPARSE_POINT|if directory{FILE_FLAG_BACKUP_SEMANTICS}else{0};
    let file=fs::OpenOptions::new().read(true).share_mode(3).custom_flags(flags).open(path).map_err(failure)?;
    let mut info=unsafe{std::mem::zeroed()};if unsafe{GetFileInformationByHandle(file.as_raw_handle(),&mut info)}==0{return Err(failure("identity"));}
    if info.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT!=0||(info.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY!=0)!=directory||(!directory&&info.nNumberOfLinks!=1){return Err("Expurgo recusa links, reparse points, hardlinks e tipos inesperados.".into());}
    Ok((file,Identity{volume:info.dwVolumeSerialNumber.to_string(),index:(((info.nFileIndexHigh as u64)<<32)|info.nFileIndexLow as u64).to_string()}))
}
#[cfg(not(windows))]
fn node(_: &Path,_:bool)->Result<(File,Identity),String>{Err("Expurgo de projeto disponível somente no Windows com identidades e handles protegidos.".into())}
fn root_guards(root:&Path)->Result<(Vec<File>,Profile),String>{
    if !root.is_absolute(){return Err("Perfil precisa de caminho local absoluto.".into());}
    #[cfg(windows)] {use std::path::Prefix;let mut parts=root.components();if !matches!(parts.next(),Some(Component::Prefix(p))if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_))){return Err("Perfil fora de disco local Windows.".into());}}
    let mut guards=vec![];let mut current=PathBuf::new();let mut identity=None;
    for part in root.components(){current.push(part.as_os_str());if matches!(part,Component::Prefix(_)){continue;}if matches!(part,Component::ParentDir|Component::CurDir){return Err("Caminho ambíguo de perfil.".into());}let (guard,id)=node(&current,true)?;guards.push(guard);identity=Some(id);}
    let normalized=root.to_string_lossy().trim_start_matches("\\\\?\\").replace('/',"\\").to_lowercase();
    Ok((guards,Profile{path_sha256:core::hash(&normalized),identity:identity.ok_or("Perfil sem identidade verificável.")?}))
}

fn core_idle(snapshot:&Snapshot)->Result<(),String>{
    if snapshot.assessments.iter().any(|a|core::active(&a.status)||a.status=="awaiting_approval")
        ||snapshot.plans.iter().any(|p|matches!(p.status.as_str(),"approved"|"running"|"canceling"|"unknown"))
        ||snapshot.executions.iter().any(|e|matches!(e.status.as_str(),"running"|"canceling"|"unknown")){
        return Err("Encerre ou reconcilie avaliações, planos e execuções antes de expurgar histórico.".into());
    }Ok(())
}
fn hash_entity<T:Serialize>(table:&str,id:&str,value:&T)->Result<RowDigest,String>{
    uuid(id)?;let bytes=serde_json::to_vec(value).map_err(failure)?;
    Ok(RowDigest{database:Database::Core,table:table.into(),id:id.into(),sha256:format!("{:x}",Sha256::digest(&bytes)),bytes:bytes.len() as u64})
}
fn core_graph(snapshot:&Snapshot,project_id:&str,previous:Option<&PurgePlan>)->Result<Inventory,String>{
    core::validate_snapshot(snapshot)?;
    let mut rows=vec![];let mut counts=Counts::default();
    if let Some(project)=snapshot.projects.iter().find(|p|p.id==project_id){
        if !project.archived{return Err("Somente projeto arquivado pode ter seu histórico expurgado.".into());}
        rows.push(hash_entity("projects",&project.id,project)?);counts.projects=1;
    }else if previous.is_none(){return Err("Projeto arquivado não encontrado.".into());}
    let known_assessments:BTreeSet<&str>=previous.map(|p|p.assessment_ids.iter().map(String::as_str).collect()).unwrap_or_default();
    let mut assessment_ids=vec![];
    for item in snapshot.assessments.iter().filter(|a|a.project_id==project_id||known_assessments.contains(a.id.as_str())){
        if item.project_id!=project_id{return Err("Uma avaliação revisada mudou de projeto; nada fora do vínculo original será excluído.".into());}
        assessment_ids.push(item.id.clone());counts.assessments+=1;counts.evidence+=item.evidence.len();counts.events+=item.events.len();rows.push(hash_entity("assessments",&item.id,item)?);
    }
    let selected:BTreeSet<&str>=assessment_ids.iter().map(String::as_str).chain(known_assessments.iter().copied()).collect();
    let known_findings:BTreeSet<&str>=previous.map(|p|p.finding_ids.iter().map(String::as_str).collect()).unwrap_or_default();
    let mut finding_ids=vec![];
    for item in snapshot.findings.iter().filter(|f|selected.contains(f.assessment_id.as_str())||known_findings.contains(f.id.as_str())){
        if !selected.contains(item.assessment_id.as_str()){return Err("Um achado revisado mudou de avaliação; escopo preservado.".into());}
        finding_ids.push(item.id.clone());counts.findings+=1;rows.push(hash_entity("findings",&item.id,item)?);
    }
    for item in snapshot.validations.iter().filter(|v|selected.contains(v.assessment_id.as_str())){counts.validations+=1;rows.push(hash_entity("validations",&item.id,item)?);}
    for item in snapshot.plans.iter().filter(|p|selected.contains(p.assessment_id.as_str())){counts.plans+=1;rows.push(hash_entity("plans",&item.id,item)?);}
    for item in snapshot.executions.iter().filter(|e|selected.contains(e.assessment_id.as_str())){counts.executions+=1;rows.push(hash_entity("executions",&item.id,item)?);}
    for item in snapshot.scope_revisions.iter().filter(|r|selected.contains(r.assessment_id.as_str())){rows.push(hash_entity("scopeRevisions",&item.id,item)?);}
    for item in snapshot.assessment_scopes.iter().filter(|b|selected.contains(b.assessment_id.as_str())){rows.push(hash_entity("assessmentScopes",&item.id,item)?);}
    for aid in &selected{let receipts=snapshot.scope_receipts.iter().filter(|r|r.assessment_id==*aid).collect::<Vec<_>>();if !receipts.is_empty(){rows.push(hash_entity("scopeReceipts",aid,&receipts)?);}}
    for item in snapshot.data_policies.iter().filter(|r|r.project_id==project_id){rows.push(hash_entity("dataPolicies",&item.project_id,item)?);counts.data_metadata_records+=1;}
    for item in snapshot.data_policy_revisions.iter().filter(|r|r.project_id==project_id){rows.push(hash_entity("dataPolicyRevisions",&item.id,item)?);counts.data_metadata_records+=1;}
    for item in snapshot.data_bindings.iter().filter(|r|r.project_id==project_id){rows.push(hash_entity("dataBindings",&item.assessment_id,item)?);counts.data_metadata_records+=1;}
    for item in snapshot.data_classifications.iter().filter(|r|r.project_id==project_id){rows.push(hash_entity("dataClassifications",&item.id,item)?);counts.data_metadata_records+=1;}
    for item in snapshot.credential_references.iter().filter(|r|r.project_id==project_id){rows.push(hash_entity("credentialReferences",&item.id,item)?);counts.data_metadata_records+=1;}
    for item in snapshot.data_mutation_receipts.iter().filter(|r|r.project_id==project_id){rows.push(hash_entity("dataMutationReceipts",&item.id,item)?);counts.data_metadata_records+=1;}
    for aid in &selected{let generations=snapshot.data_subject_generations.iter().filter(|g|g.subject.assessment_id()==*aid).collect::<Vec<_>>();if !generations.is_empty(){counts.data_metadata_records+=generations.len();rows.push(hash_entity("dataSubjectGenerations",aid,&generations)?);}}
    for item in snapshot.data_use_receipts.iter().filter(|r|r.project_ids.iter().any(|id|id==project_id)){rows.push(hash_entity("dataUseReceipts",&item.id,item)?);counts.data_metadata_records+=1;if item.project_ids.len()>1{counts.shared_data_receipts+=1;}}
    if let Some(previous)=previous {
        // Existing primary keys may not silently move outside the selected graph.
        for old in previous.rows.iter().filter(|r|r.database==Database::Core){
            let exists_elsewhere=match old.table.as_str(){
                "projects"=>false,
                "assessments"=>snapshot.assessments.iter().any(|v|v.id==old.id),
                "findings"=>snapshot.findings.iter().any(|v|v.id==old.id),
                "validations"=>snapshot.validations.iter().any(|v|v.id==old.id),
                "plans"=>snapshot.plans.iter().any(|v|v.id==old.id),
                "executions"=>snapshot.executions.iter().any(|v|v.id==old.id),
                "scopeRevisions"=>snapshot.scope_revisions.iter().any(|v|v.id==old.id),
                "assessmentScopes"=>snapshot.assessment_scopes.iter().any(|v|v.id==old.id),
                "scopeReceipts"=>snapshot.scope_receipts.iter().any(|v|v.assessment_id==old.id),
                "dataPolicies"=>snapshot.data_policies.iter().any(|v|v.project_id==old.id),
                "dataPolicyRevisions"=>snapshot.data_policy_revisions.iter().any(|v|v.id==old.id),
                "dataBindings"=>snapshot.data_bindings.iter().any(|v|v.assessment_id==old.id),
                "dataClassifications"=>snapshot.data_classifications.iter().any(|v|v.id==old.id),
                "credentialReferences"=>snapshot.credential_references.iter().any(|v|v.id==old.id),
                "dataUseReceipts"=>snapshot.data_use_receipts.iter().any(|v|v.id==old.id),
                "dataMutationReceipts"=>snapshot.data_mutation_receipts.iter().any(|v|v.id==old.id),
                "dataSubjectGenerations"=>snapshot.data_subject_generations.iter().any(|v|v.subject.assessment_id()==old.id),
                table if WORK_TABLES.contains(&table)=>false, // checked against SQL ownership below
                _=>return Err("Tipo de entidade desconhecido no journal.".into()),
            };
            if exists_elsewhere&&!rows.iter().any(|r|key(r)==key(old)){return Err("Uma entidade revisada mudou para fora do projeto; escopo preservado.".into());}
        }
    }
    assessment_ids.sort();finding_ids.sort();rows.sort();
    Ok(Inventory{rows,bindings:vec![],counts,assessment_ids,finding_ids})
}
fn typed_row(row:&rusqlite::Row<'_>,columns:usize,database:Database,table:&str)->Result<RowDigest,String>{
    let id:String=row.get(1).map_err(failure)?;uuid(&id)?;
    let mut digest=Sha256::new();let mut bytes=0u64;
    for column in 0..columns {
        let value=row.get_ref(column).map_err(failure)?;
        let length=match value{ValueRef::Null=>0,ValueRef::Integer(_)|ValueRef::Real(_)=>8,ValueRef::Text(v)|ValueRef::Blob(v)=>v.len()};
        bytes=bytes.checked_add(length as u64).ok_or("Linha excede limite de tamanho.")?;if bytes>16*1024*1024{return Err("Uma linha excede o limite de expurgo; nenhum dado foi apagado.".into());}
        let (tag,data):(u8,Vec<u8>)=match value {
            ValueRef::Null=>(0,vec![]),ValueRef::Integer(v)=>(1,v.to_le_bytes().to_vec()),ValueRef::Real(v)=>(2,v.to_bits().to_le_bytes().to_vec()),
            ValueRef::Text(v)=>(3,v.to_vec()),ValueRef::Blob(v)=>(4,v.to_vec()),
        };
        digest.update([tag]);digest.update((data.len() as u64).to_le_bytes());digest.update(&data);
    }
    Ok(RowDigest{database,table:table.into(),id,sha256:format!("{:x}",digest.finalize()),bytes})
}
// A chunk stream is one reviewed entity keyed by its capture UUID; its digest
// includes every typed chunk row in order, including offsets and bytes.
const WORK_TABLES:[&str;12]=["work_plan_heads","work_plan_revisions","work_jobs","work_attempts","work_plan_bindings","work_events","work_commands","work_leases","capture_manifests","capture_chunks","capture_cursors","work_ai_dispositions"];
fn work_query(table:&str)->Result<&'static str,String>{Ok(match table{
    "work_plan_heads"=>"SELECT t.rowid,t.plan_id,t.assessment_id,t.* FROM work_plan_heads t ORDER BY t.rowid",
    "work_plan_revisions"=>"SELECT t.rowid,t.id,t.assessment_id,t.* FROM work_plan_revisions t ORDER BY t.rowid",
    "work_jobs"=>"SELECT t.rowid,t.id,t.assessment_id,t.* FROM work_jobs t ORDER BY t.rowid",
    "work_attempts"=>"SELECT t.rowid,t.id,j.assessment_id,t.* FROM work_attempts t JOIN work_jobs j ON j.id=t.job_id ORDER BY t.rowid",
    "work_plan_bindings"=>"SELECT t.rowid,t.execution_plan_id,j.assessment_id,t.* FROM work_plan_bindings t JOIN work_jobs j ON j.id=t.job_id ORDER BY t.rowid",
    "work_events"=>"SELECT t.rowid,t.id,h.assessment_id,t.* FROM work_events t JOIN work_plan_heads h ON h.plan_id=t.stream_id ORDER BY t.rowid",
    "work_commands"=>"SELECT t.rowid,t.id,t.assessment_id,t.* FROM work_commands t ORDER BY t.rowid",
    "work_ai_dispositions"=>"SELECT t.rowid,t.id,t.assessment_id,t.* FROM work_ai_dispositions t ORDER BY t.rowid",
    "work_leases"=>"SELECT t.rowid,t.owner_id,json_extract(t.data,'$.owner.assessmentId'),t.* FROM work_leases t ORDER BY t.rowid",
    "capture_manifests"=>"SELECT t.rowid,t.id,t.assessment_id,t.* FROM capture_manifests t ORDER BY t.rowid",
    "capture_chunks"=>"SELECT t.rowid,t.capture_id,m.assessment_id,t.* FROM capture_chunks t JOIN capture_manifests m ON m.id=t.capture_id ORDER BY t.capture_id,t.chunk_index",
    "capture_cursors"=>"SELECT t.rowid,t.id,t.assessment_id,t.* FROM capture_cursors t ORDER BY t.rowid",
    _=>return Err("Tabela de trabalho fora do contrato de expurgo.".into()),
})}
fn work_graph(conn:&Connection,assessments:&[String],previous:Option<&PurgePlan>)->Result<Vec<RowDigest>,String>{
    let mut out=vec![];
    for table in WORK_TABLES{
        let prior=previous.map(|p|p.rows.iter().filter(|r|r.database==Database::Core&&r.table==table).map(|r|r.id.as_str()).collect::<BTreeSet<_>>()).unwrap_or_default();
        let mut query=conn.prepare(work_query(table)?).map_err(failure)?;let columns=query.column_count();
        let mut rows=query.query([]).map_err(failure)?;let mut chunks:BTreeMap<String,Vec<RowDigest>>=BTreeMap::new();
        while let Some(row)=rows.next().map_err(failure)?{
            let id:String=row.get(1).map_err(failure)?;let owner:Option<String>=row.get(2).map_err(failure)?;
            let selected=owner.as_ref().is_some_and(|aid|assessments.contains(aid));
            if !selected&&!prior.contains(id.as_str()){continue;}
            if !selected{return Err("Registro de trabalho revisado mudou para outra avaliação; dados preservados.".into());}
            let record=typed_row(row,columns,Database::Core,table)?;
            if table=="capture_chunks"{chunks.entry(id).or_default().push(record);}else{out.push(record);}
            if out.len()+chunks.values().map(Vec::len).sum::<usize>()>MAX_ROWS{return Err("Grafo de trabalho excede limite de expurgo.".into());}
        }
        for (id,records) in chunks{
            let bytes=records.iter().map(|r|r.bytes).sum();
            out.push(RowDigest{database:Database::Core,table:table.into(),id,sha256:core::hash(&serde_json::to_string(&records).map_err(failure)?),bytes});
        }
    }
    out.sort();Ok(out)
}
fn delete_work_graph(conn:&Connection,rows:&[RowDigest])->Result<(),String>{
    // Both ends of the head/revision cycle are deferred by the schema. All
    // other FKs are removed child-first inside the same canonical transaction.
    for (table,key_column) in [("work_leases","owner_id"),("work_ai_dispositions","id"),("capture_cursors","id"),("capture_chunks","capture_id"),("capture_manifests","id"),("work_events","id"),("work_plan_bindings","execution_plan_id"),("work_attempts","id"),("work_jobs","id"),("work_plan_revisions","id"),("work_plan_heads","plan_id"),("work_commands","id")]{
        let sql=format!("DELETE FROM {table} WHERE {key_column}=?1");
        for row in rows.iter().filter(|r|r.table==table){let removed=conn.execute(&sql,[&row.id]).map_err(failure)?;if removed==0||(table!="capture_chunks"&&removed!=1){return Err("Grafo de trabalho mudou antes da exclusão; transação revertida.".into());}}
    }
    if conn.prepare("PRAGMA foreign_key_check").map_err(failure)?.exists([]).map_err(failure)?{return Err("Exclusão deixaria vínculo inválido; transação revertida.".into());}
    Ok(())
}
fn json_identity(value:&Value,id:&str,assessment:&str)->Result<(),String>{
    if value["id"]!=id||value["assessmentId"]!=assessment{return Err("Identidade/vínculo JSON de histórico inconsistente; dados preservados.".into());}Ok(())
}
fn auxiliary_idle(open:&OpenDatabase,root:&Path,assessments:&[String])->Result<(),String>{
    match open.kind {
        Database::Assistant=>{
            let mut query=open.db.prepare("SELECT status,details FROM requests").map_err(failure)?;
            let statuses=query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(failure)?;
            for (index,status) in statuses.enumerate() {
                if index>=MAX_ROWS{return Err("Histórico de requisições excede limite de inspeção.".into());}
                let (status,details)=status.map_err(failure)?;
                if status=="unknown"&&crate::ai::request_unknown_acknowledged(&details){continue;}
                if !matches!(status.as_str(),"completed"|"failed"|"incomplete"|"failed_before_dispatch") {return Err("Há requisição IA pendente, desconhecida sem reconhecimento, ou com estado não reconhecido; preserve seu histórico.".into());}
            }
            let mut query=open.db.prepare("SELECT id,assessment,data FROM investigations").map_err(failure)?;let mut rows=query.query([]).map_err(failure)?;let mut count=0;
            while let Some(row)=rows.next().map_err(failure)? {count+=1;if count>MAX_ROWS{return Err("Histórico IA excedeu o limite de inspeção.".into());}
                let id:String=row.get(0).map_err(failure)?;let assessment:String=row.get(1).map_err(failure)?;let raw:String=row.get(2).map_err(failure)?;
                if raw.len()>1024*1024{return Err("Investigação excede limite de leitura.".into());}let value:Value=serde_json::from_str(&raw).map_err(failure)?;json_identity(&value,&id,&assessment)?;
                crate::ai::validate_investigation_record(&value).map_err(|_|"Schema ou integridade de investigação desconhecido; histórico preservado.")?;
                if !matches!(value["status"].as_str(),Some("prepared"|"completed"|"partial"|"failed"|"canceled"|"reconciled"|"revoked")){return Err("Há investigação IA ativa/desconhecida; reconcilie antes do expurgo.".into());}
            }
        },
        Database::Remediation=>{
            let mut query=open.db.prepare("SELECT id,finding_id,data FROM remediation_plans").map_err(failure)?;let mut rows=query.query([]).map_err(failure)?;let mut count=0;
            while let Some(row)=rows.next().map_err(failure)? {count+=1;if count>MAX_ROWS{return Err("Histórico de correções excedeu o limite.".into());}
                let id:String=row.get(0).map_err(failure)?;let finding:String=row.get(1).map_err(failure)?;let raw:String=row.get(2).map_err(failure)?;
                if raw.len()>4*1024*1024{return Err("Correção excede limite de leitura.".into());}
                let parsed:crate::remediation::RemediationPlan=serde_json::from_str(&raw).map_err(failure)?;let value=serde_json::to_value(parsed).map_err(failure)?;
                if value["id"]!=id||value["findingId"]!=finding{return Err("Vínculo de correção inconsistente.".into());}
                if !matches!(value["status"].as_str(),Some("proposed"|"applied"|"retested"|"confirmed"|"revoked"|"invalidated"|"rolled_back"|"failed"|"reconciled")){return Err("Há correção aprovada/ativa/desconhecida; encerre ou reconcilie antes do expurgo.".into());}
            }
        },
        Database::Bridge=>{
            // The newest retire attempt determines whether its producer's
            // history can be removed. Use the same order and family validation
            // as the live bridge. Row digests are computed separately below.
            let runs=crate::kali_bridge::read_history_runs(&open.db)?;
            if runs.iter().any(|run|!run.cleanup_confirmed||!matches!(run.status.as_str(),"completed"|"failed"|"canceled")){
                return Err("Há sessão MCP sem término confirmado; preserve a trilha antes do expurgo.".into());
            }
            crate::kali_bridge::ensure_capture_history_purgeable(root,&runs,assessments)?;
        },Database::Core=>{},
    }Ok(())
}
fn table_query(table:&str)->Result<(&'static str,usize),String>{match table {
    "messages"=>Ok(("SELECT rowid,id,assessment,role,text,created FROM messages WHERE assessment IN (SELECT value FROM json_each(?1)) OR id IN (SELECT value FROM json_each(?2)) ORDER BY id",6)),
    "requests"=>Ok(("SELECT rowid,id,assessment,endpoint,model,revision,context_hash,status,created,input_tokens,output_tokens,details FROM requests WHERE assessment IN (SELECT value FROM json_each(?1)) OR id IN (SELECT value FROM json_each(?2)) ORDER BY id",12)),
    "investigations"=>Ok(("SELECT rowid,id,assessment,data FROM investigations WHERE assessment IN (SELECT value FROM json_each(?1)) OR id IN (SELECT value FROM json_each(?2)) ORDER BY id",4)),
    "remediation_plans"=>Ok(("SELECT rowid,id,finding_id,data,backup FROM remediation_plans WHERE json_extract(data,'$.assessmentId') IN (SELECT value FROM json_each(?1)) OR id IN (SELECT value FROM json_each(?2)) OR json_extract(data,'$.projectId')=?3 OR finding_id IN (SELECT value FROM json_each(?4)) ORDER BY id",5)),
    "bridge_runs"=>Ok(("SELECT rowid,id,data FROM bridge_runs WHERE json_extract(data,'$.assessmentId') IN (SELECT value FROM json_each(?1)) OR id IN (SELECT value FROM json_each(?2)) ORDER BY id",3)),
    _=>Err("Tabela fora do contrato de expurgo.".into()),
}}
fn scan_auxiliary(open:&OpenDatabase,project_id:&str,assessments:&[String],findings:&[String],previous:Option<&PurgePlan>,counts:&mut Counts)->Result<Vec<RowDigest>,String>{
    let tables:&[&str]=match open.kind{Database::Assistant=>&["messages","requests","investigations"],Database::Remediation=>&["remediation_plans"],Database::Bridge=>&["bridge_runs"],Database::Core=>&[]};
    let mut out=vec![];let owners=serde_json::to_string(assessments).map_err(failure)?;let finding_json=serde_json::to_string(findings).map_err(failure)?;
    for table in tables {
        let prior:Vec<&str>=previous.map(|p|p.rows.iter().filter(|r|r.database==open.kind&&r.table==*table).map(|r|r.id.as_str()).collect()).unwrap_or_default();
        let prior=serde_json::to_string(&prior).map_err(failure)?;let (sql,columns)=table_query(table)?;let mut statement=open.db.prepare(sql).map_err(failure)?;
        let mut rows=if *table=="remediation_plans"{statement.query(params![owners,prior,project_id,finding_json])}else{statement.query(params![owners,prior])}.map_err(failure)?;
        while let Some(row)=rows.next().map_err(failure)? {
            if out.len()>=MAX_ROWS{return Err("Escopo excedeu o limite de linhas de expurgo.".into());}
            let record=typed_row(row,columns,open.kind,table)?;
            let owner=match *table {
                "messages"|"requests"=>row.get::<_,String>(2).map_err(failure)?,
                "investigations"=>{let owner:String=row.get(2).map_err(failure)?;let raw:String=row.get(3).map_err(failure)?;let value:Value=serde_json::from_str(&raw).map_err(failure)?;json_identity(&value,&record.id,&owner)?;owner},
                "remediation_plans"=>{let finding:String=row.get(2).map_err(failure)?;let raw:String=row.get(3).map_err(failure)?;let value:Value=serde_json::from_str(&raw).map_err(failure)?;
                    if value["id"]!=record.id||value["findingId"]!=finding||!findings.contains(&finding)||value["projectId"]!=project_id{return Err("Correção revisada tem vínculo fora do projeto; nada foi removido dessa etapa.".into());}
                    value["assessmentId"].as_str().ok_or("Correção sem avaliação vinculada.")?.to_owned()},
                "bridge_runs"=>{let raw:String=row.get(2).map_err(failure)?;let run:crate::kali_bridge::BridgeRun=serde_json::from_str(&raw).map_err(failure)?;if run.id!=record.id{return Err("Sessão MCP tem identidade divergente.".into());}run.assessment_id},
                _=>return Err("Tabela fora do contrato de expurgo.".into()),
            };
            if !assessments.contains(&owner){return Err("Uma linha revisada mudou para outra avaliação; dados de outro projeto preservados.".into());}
            match *table{"messages"=>counts.messages+=1,"requests"=>{counts.requests+=1;if row.get::<_,String>(7).map_err(failure)?=="unknown"{counts.requests_unknown_acknowledged+=1;}},"investigations"=>counts.investigations+=1,"remediation_plans"=>counts.remediations+=1,"bridge_runs"=>counts.bridge_runs+=1,_=>{}}
            out.push(record);
        }
    }Ok(out)
}

fn validate_plan(p:&PurgePlan)->Result<(),String>{
    uuid(&p.id)?;uuid(&p.project_id)?;
    for ids in [&p.assessment_ids,&p.finding_ids]{let mut previous=None;for id in ids{uuid(id)?;if previous.is_some_and(|prev:&String|prev>=id){return Err("Identidades do journal não são únicas/ordenadas.".into());}previous=Some(id);}}
    if p.digest!=plan_digest(p)||p.confirmation_phrase!=PHRASE||p.limits!=LIMITS||!sha(&p.profile.path_sha256)
        ||chrono::DateTime::parse_from_rfc3339(&p.created_at).is_err()||chrono::DateTime::parse_from_rfc3339(&p.expires_at).is_err()
        ||!matches!(p.status.as_str(),"prepared"|"revoked"|"rejected"|"applying"|"unknown"|"partial"|"conflict"|"completed"|"abandoned_partial"){
        return Err("Integridade/estado do journal de expurgo inválido; nenhum efeito autorizado.".into());
    }
    if p.bindings.len()!=4||p.steps.len()!=4||p.rows.len()>MAX_ROWS||p.counts.total_rows!=p.rows.len(){return Err("Escopo do journal excede limites ou está incompleto.".into());}
    for (index,kind) in DATABASES.iter().enumerate(){let b=&p.bindings[index];let step=&p.steps[index];
        if b.database!=*kind||step.database!=*kind||b.identity.is_some()!=b.schema_sha256.is_some()||b.schema_sha256.as_deref().is_some_and(|v|!sha(v))
            ||(*kind==Database::Core&&b.identity.is_none())||step.planned_rows!=p.rows.iter().filter(|r|r.database==*kind).count()
            ||step.removed_rows>step.planned_rows||!matches!(step.status.as_str(),"pending"|"deleting"|"completed"|"observed_absent") {return Err("Etapa ou vínculo de banco inválido no journal.".into());}}
    let mut bytes=0u64;let mut keys=BTreeSet::new();
    for row in &p.rows {uuid(&row.id)?;if !sha(&row.sha256)||!keys.insert(key(row)){return Err("Identidade/hash de linha inválido no journal.".into());}
        let allowed=match row.database {Database::Core=>WORK_TABLES.contains(&row.table.as_str())||["projects","assessments","findings","validations","plans","executions","scopeRevisions","assessmentScopes","scopeReceipts","dataPolicies","dataPolicyRevisions","dataBindings","dataClassifications","credentialReferences","dataUseReceipts","dataMutationReceipts","dataSubjectGenerations"].contains(&row.table.as_str()),Database::Assistant=>["messages","requests","investigations"].contains(&row.table.as_str()),Database::Remediation=>row.table=="remediation_plans",Database::Bridge=>row.table=="bridge_runs"};
        if !allowed{return Err("Journal referencia tabela fora do contrato.".into());}bytes=bytes.checked_add(row.bytes).ok_or("Limite de bytes excedido.")?;
    }
    if bytes>MAX_BYTES||bytes!=p.counts.total_bytes||p.counts.projects!=1{return Err("Contagens do journal de expurgo inválidas.".into());}
    if matches!(p.status.as_str(),"prepared"|"revoked"|"rejected")&&p.steps.iter().any(|s|s.status!="pending"||s.removed_rows!=0){return Err("Plano não aplicado contém efeitos registrados.".into());}
    if p.status=="completed"&&p.steps.iter().any(|s|!matches!(s.status.as_str(),"completed"|"observed_absent")){return Err("Conclusão sem todas as etapas observadas.".into());}
    if p.status=="abandoned_partial"&&!p.closure.as_ref().is_some_and(|c|c.author=="local-user"&&sha(&c.note_sha256)&&c.note_bytes>0&&c.note_bytes<=4000&&c.acknowledge_partial_deletion){return Err("Encerramento parcial sem reconhecimento humano íntegro.".into());}
    Ok(())
}

impl ProjectPurgeService {
    pub fn new(root:PathBuf)->Result<Self,String>{
        let (mut guards,profile)=root_guards(&root)?;let journal=root.join(JOURNAL);
        let existing=match fs::symlink_metadata(&journal){Ok(_)=>true,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>false,Err(e)=>return Err(failure(e))};
        if !existing{fs::OpenOptions::new().create_new(true).write(true).open(&journal).map_err(failure)?;}
        let (guard,_)=node(&journal,false)?;guards.push(guard);
        let db=Connection::open_with_flags(&journal,rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(failure)?;db.busy_timeout(Duration::from_secs(5)).map_err(failure)?;
        if existing{schema(&db,&JOURNAL_SCHEMA)?;}
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS project_purge_plans(id TEXT PRIMARY KEY,data TEXT NOT NULL);").map_err(failure)?;schema(&db,&JOURNAL_SCHEMA)?;
        let service=Self{root,profile,db:Mutex::new(db),operation:Mutex::new(()),_guards:guards,#[cfg(test)]fail_after_commit:Mutex::new(None)};
        for mut p in service.plans()? {
            if p.status=="prepared"{p.status="revoked".into();p.error=Some("Revisão não utilizada revogada após reinício; prepare outro plano antes de excluir.".into());service.save(&p)?;}
            else if p.status=="applying"{p.status="unknown".into();p.error=Some("Aplicativo interrompido durante expurgo. Nenhuma exclusão foi repetida; inspecione antes de continuar ou encerrar parcialmente.".into());service.save(&p)?;}
        }Ok(service)
    }
    pub fn has_unresolved(&self)->Result<bool,String>{Ok(self.plans()?.iter().any(|p|unresolved(&p.status)))}
    pub fn handle(&self,action:&str,payload:Value,store:&Store)->Result<Value,String>{
        if !supports(action)||!payload.is_object()||payload.to_string().len()>16384{return Err("Solicitação de expurgo inválida.".into());}
        let _operation=self.operation.try_lock().map_err(|_|"Outra operação de expurgo está em andamento.")?;
        match action {
            "projectPurge.status"=>{if payload.as_object().is_none_or(|o|!o.is_empty()){return Err("Status não recebe campos.".into());}let plans=self.plans()?;Ok(json!({"hasUnresolved":plans.iter().any(|p|unresolved(&p.status)),"hasPartialHistory":plans.iter().any(|p|unresolved(&p.status)||p.status=="abandoned_partial"),"journals":plans,"limits":LIMITS,"confirmationPhrase":PHRASE}))},
            "projectPurge.get"=>{let input:Id=decode(payload)?;encode(self.load(&input.id)?)},
            "projectPurge.plan"=>self.plan(decode(payload)?,store).and_then(encode),
            "projectPurge.apply"=>self.apply(decode(payload)?,store).and_then(encode),
            "projectPurge.reconcile"=>self.reconcile(decode(payload)?,store).and_then(encode),
            "projectPurge.closePartial"=>self.close(decode(payload)?,store).and_then(encode),
            _=>Err("Ação de expurgo indisponível.".into()),
        }
    }
    fn plans(&self)->Result<Vec<PurgePlan>,String>{
        let db=self.db.lock().map_err(failure)?;let mut q=db.prepare("SELECT id,data FROM project_purge_plans ORDER BY rowid DESC LIMIT 101").map_err(failure)?;
        let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?;
        if rows.len()>MAX_PLANS{return Err("Limite de journals de expurgo atingido; preserve os registros.".into());}
        rows.into_iter().map(|(id,raw)|{let p=Self::parse(&raw)?;if p.id!=id{return Err("Identidade do journal divergente.".into());}Ok(p)}).collect()
    }
    fn parse(raw:&str)->Result<PurgePlan,String>{if raw.len()>8*1024*1024{return Err("Journal excede limite de leitura.".into());}let p=serde_json::from_str(raw).map_err(failure)?;validate_plan(&p)?;Ok(p)}
    fn load(&self,id:&str)->Result<PurgePlan,String>{uuid(id)?;let raw:Option<String>=self.db.lock().map_err(failure)?.query_row("SELECT data FROM project_purge_plans WHERE id=?1",[id],|r|r.get(0)).optional().map_err(failure)?;let p=Self::parse(&raw.ok_or("Plano de expurgo não encontrado.")?)?;if p.id!=id{return Err("Identidade do journal divergente.".into());}Ok(p)}
    fn save(&self,p:&PurgePlan)->Result<(),String>{validate_plan(p)?;self.db.lock().map_err(failure)?.execute("INSERT INTO project_purge_plans(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![p.id,serde_json::to_string(p).map_err(failure)?]).map_err(failure)?;Ok(())}
    fn same_profile(&self,store:&Store,p:Option<&PurgePlan>)->Result<(),String>{
        let (_guards,current)=root_guards(&store.root)?;
        if current!=self.profile||p.is_some_and(|p|p.profile!=current){return Err("O plano pertence a outro perfil ou identidade de armazenamento. Cópias/restaurações não reutilizam autoridade de exclusão.".into());}Ok(())
    }
    fn open_databases(&self,write:bool)->Result<(Vec<OpenDatabase>,Vec<Binding>),String>{
        let mut opened=vec![];let mut bindings=vec![];
        for kind in DATABASES {
            let path=self.root.join(kind.name());let present=match fs::symlink_metadata(&path){Ok(_)=>true,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>false,Err(e)=>return Err(failure(e))};
            if !present {if kind==Database::Core{return Err("Banco principal não encontrado.".into());}bindings.push(Binding{database:kind,identity:None,schema_sha256:None});continue;}
            let (guard,identity)=node(&path,false)?;
            let flags=if write&&kind!=Database::Core{rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE}else{rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY};
            let db=Connection::open_with_flags(path,flags).map_err(failure)?;db.busy_timeout(Duration::from_secs(5)).map_err(failure)?;
            let in_transaction=write&&kind!=Database::Core;
            if in_transaction{db.execute_batch("PRAGMA synchronous=FULL; BEGIN IMMEDIATE").map_err(failure)?;}
            let binding=Binding{database:kind,identity:Some(identity),schema_sha256:Some(schema(&db,&expected_schema(kind))?)};
            bindings.push(binding);opened.push(OpenDatabase{kind,db,_guard:guard,in_transaction});
        }Ok((opened,bindings))
    }
    fn inventory(&self,store:&Store,project_id:&str,previous:Option<&PurgePlan>,opened:&[OpenDatabase],bindings:Vec<Binding>,require_idle:bool)->Result<Inventory,String>{
        let mut inventory=store.work_read(|conn,snapshot|{
            if require_idle{core_idle(snapshot)?;crate::work_supervisor::ensure_quiescent_locked(conn,snapshot)?;}
            let mut inventory=core_graph(snapshot,project_id,previous)?;
            let assessments=previous.map(|p|p.assessment_ids.as_slice()).unwrap_or(&inventory.assessment_ids);
            inventory.rows.extend(work_graph(conn,assessments,previous)?);Ok(inventory)
        })?;
        let assessments=previous.map(|p|p.assessment_ids.as_slice()).unwrap_or(&inventory.assessment_ids);
        let findings=previous.map(|p|p.finding_ids.as_slice()).unwrap_or(&inventory.finding_ids);
        for db in opened {
            if require_idle{auxiliary_idle(db,&store.root,assessments)?;}
            inventory.rows.extend(scan_auxiliary(db,project_id,assessments,findings,previous,&mut inventory.counts)?);
        }
        inventory.rows.sort();inventory.bindings=bindings;inventory.counts.total_rows=inventory.rows.len();inventory.counts.total_bytes=inventory.rows.iter().try_fold(0u64,|total,r|total.checked_add(r.bytes).ok_or("Escopo excede limite de bytes."))?;
        if inventory.rows.len()>MAX_ROWS||inventory.counts.total_bytes>MAX_BYTES{return Err("Escopo excede 10.000 linhas ou 128 MiB; nenhum lote parcial foi autorizado.".into());}Ok(inventory)
    }
    fn plan(&self,input:Selection,store:&Store)->Result<PurgePlan,String>{
        uuid(&input.project_id)?;self.same_profile(store,None)?;if self.has_unresolved()?{return Err("Reconcilie ou encerre o expurgo interrompido antes de outra revisão.".into());}
        if self.plans()?.len()>=MAX_PLANS{return Err("Limite de 100 journals de expurgo atingido; histórico preservado.".into());}
        let (opened,bindings)=self.open_databases(false)?;let inventory=self.inventory(store,&input.project_id,None,&opened,bindings,true)?;
        let steps=DATABASES.iter().map(|kind|Step{database:*kind,status:"pending".into(),planned_rows:inventory.rows.iter().filter(|r|r.database==*kind).count(),removed_rows:0}).collect();
        let mut p=PurgePlan{id:model::id(),project_id:input.project_id,assessment_ids:inventory.assessment_ids,finding_ids:inventory.finding_ids,profile:self.profile.clone(),created_at:model::now(),expires_at:(chrono::Utc::now()+chrono::Duration::minutes(15)).to_rfc3339(),digest:String::new(),status:"prepared".into(),bindings:inventory.bindings,rows:inventory.rows,counts:inventory.counts,confirmation_phrase:PHRASE.into(),limits:LIMITS.iter().map(|s|s.to_string()).collect(),steps,inspection:None,closure:None,error:None};
        p.digest=plan_digest(&p);self.save(&p)?;Ok(p)
    }
    fn inspect(&self,p:&PurgePlan,store:&Store)->Result<Inspection,String>{
        self.same_profile(store,Some(p))?;let (opened,bindings)=self.open_databases(false)?;
        let inventory=self.inventory(store,&p.project_id,Some(p),&opened,bindings,false)?;
        compare(p,&inventory)
    }
    fn observe(&self,p:&PurgePlan,store:&Store)->Inspection {
        self.inspect(p,store).unwrap_or_else(|message|Inspection{at:model::now(),present:0,absent:0,changed:0,new_rows:0,unobserved:p.rows.len(),complete:false,conflict:true,message})
    }
    fn apply(&self,input:Apply,store:&Store)->Result<PurgePlan,String>{
        let mut p=self.load(&input.id)?;
        if input.digest!=p.digest||input.confirmation!=PHRASE{return Err("Confira o digest e digite exatamente a frase de exclusão do projeto e histórico.".into());}
        if !matches!(p.status.as_str(),"prepared"|"applying"|"unknown"|"partial"|"conflict"){return Err("Plano encerrado/revogado não pode excluir dados novamente. Prepare outra revisão se necessário.".into());}
        if p.status=="prepared"&&chrono::DateTime::parse_from_rfc3339(&p.expires_at).map_err(failure)?<=chrono::Utc::now(){return Err("Revisão expirou antes do primeiro efeito; prepare novo plano.".into());}
        self.same_profile(store,Some(&p))?;
        if self.plans()?.iter().any(|other|other.id!=p.id&&unresolved(&other.status)){return Err("Outro expurgo interrompido exige reconciliação antes desta operação.".into());}
        let before=p.status.clone();
        let result=(||{
            // Hold IMMEDIATE transactions in every auxiliary DB throughout the
            // initial whole-graph check. No selected row can change inside a
            // stage between its typed-byte check and its DELETE.
            let (mut opened,bindings)=self.open_databases(true)?;
            let inventory=self.inventory(store,&p.project_id,Some(&p),&opened,bindings.clone(),true)?;
            let inspection=compare(&p,&inventory)?;
            if inspection.conflict{p.inspection=Some(inspection);return Err("O grafo ou uma linha mudou depois da revisão. Nenhuma nova exclusão foi iniciada; preserve os dados e encerre parcialmente ou revise novo plano.".into());}
            p.status="applying".into();p.error=None;p.inspection=Some(inspection);self.save(&p)?;
            for index in 0..DATABASES.len() {
                let kind=DATABASES[index];
                let current=self.inventory(store,&p.project_id,Some(&p),&opened,bindings.clone(),false)?;
                let inspected=compare(&p,&current)?;
                if inspected.conflict{p.inspection=Some(inspected);return Err("O grafo mudou entre etapas. Exclusões posteriores foram interrompidas; o histórico pode estar parcial.".into());}
                let remaining:Vec<RowDigest>=current.rows.iter().filter(|r|r.database==kind).cloned().collect();
                if matches!(p.steps[index].status.as_str(),"completed"|"observed_absent") {
                    if let Some(db)=opened.iter_mut().find(|db|db.kind==kind&&db.in_transaction){db.db.execute_batch("COMMIT").map_err(failure)?;db.in_transaction=false;}
                    continue;
                }
                if remaining.is_empty()&&p.steps[index].status=="deleting" {
                    p.steps[index].status="observed_absent".into();self.save(&p)?;
                    if let Some(db)=opened.iter_mut().find(|db|db.kind==kind&&db.in_transaction){db.db.execute_batch("COMMIT").map_err(failure)?;db.in_transaction=false;}
                    continue;
                }
                p.steps[index].status="deleting".into();self.save(&p)?;
                if kind==Database::Core {
                    store.work_transaction(|conn,snapshot|{
                        core_idle(snapshot)?;crate::work_supervisor::ensure_quiescent_locked(conn,snapshot)?;
                        let mut actual=core_graph(snapshot,&p.project_id,Some(&p))?;
                        actual.rows.extend(work_graph(conn,&p.assessment_ids,Some(&p))?);actual.rows.sort();
                        if actual.rows!=remaining{return Err("Grafo principal mudou antes de sua exclusão. As etapas anteriores podem estar confirmadas; nada novo será apagado.".into());}
                        delete_work_graph(conn,&remaining)?;
                        let assessments:BTreeSet<&str>=p.assessment_ids.iter().map(String::as_str).collect();let findings:BTreeSet<&str>=p.finding_ids.iter().map(String::as_str).collect();
                        snapshot.data_policies.retain(|v|v.project_id!=p.project_id);snapshot.data_policy_revisions.retain(|v|v.project_id!=p.project_id);snapshot.data_bindings.retain(|v|v.project_id!=p.project_id);snapshot.data_classifications.retain(|v|v.project_id!=p.project_id);snapshot.credential_references.retain(|v|v.project_id!=p.project_id);snapshot.data_mutation_receipts.retain(|v|v.project_id!=p.project_id);snapshot.data_use_receipts.retain(|v|!v.project_ids.contains(&p.project_id));
                        snapshot.data_subject_generations.retain(|g|!assessments.contains(g.subject.assessment_id()));
                        snapshot.scope_receipts.retain(|v|!assessments.contains(v.assessment_id.as_str()));snapshot.scope_revisions.retain(|v|!assessments.contains(v.assessment_id.as_str()));snapshot.assessment_scopes.retain(|v|!assessments.contains(v.assessment_id.as_str()));snapshot.validations.retain(|v|!assessments.contains(v.assessment_id.as_str()));snapshot.executions.retain(|v|!assessments.contains(v.assessment_id.as_str()));snapshot.plans.retain(|v|!assessments.contains(v.assessment_id.as_str()));snapshot.findings.retain(|v|!findings.contains(v.id.as_str()));snapshot.assessments.retain(|v|!assessments.contains(v.id.as_str()));snapshot.projects.retain(|v|v.id!=p.project_id);
                        Ok(())
                    })?;
                }else if let Some(db)=opened.iter_mut().find(|db|db.kind==kind){
                    for row in &remaining {
                        let sql=delete_sql(&row.table)?;
                        if db.db.execute(sql,[&row.id]).map_err(failure)?!=1{return Err("Linha revisada desapareceu antes da exclusão; etapa SQLite revertida, histórico anterior preservado.".into());}
                    }
                    db.db.execute_batch("COMMIT").map_err(failure)?;db.in_transaction=false;
                }else if !remaining.is_empty(){return Err("Banco revisado desapareceu antes de sua etapa.".into());}
                #[cfg(test)] {let mut stop=self.fail_after_commit.lock().unwrap();if *stop==Some(kind){*stop=None;return Err("Interrupção sintética após commit e antes do recibo da etapa.".into());}}
                p.steps[index].status="completed".into();p.steps[index].removed_rows+=remaining.len();self.save(&p)?;
            }
            let inspection=self.inspect(&p,store)?;
            if !inspection.complete{p.inspection=Some(inspection);return Err("A inspeção final não confirmou ausência de todo o escopo; o expurgo permanece pendente.".into());}
            p.inspection=Some(inspection);p.status="completed".into();p.error=None;self.save(&p)?;
            Ok::<(),String>(())
        })();
        if let Err(error)=result {
            p.status=if before=="prepared"&&p.steps.iter().all(|s|s.status=="pending"){"rejected"}else if p.inspection.as_ref().is_some_and(|i|i.conflict){"conflict"}else{"partial"}.into();
            p.error=Some(error.clone());let _=self.save(&p);return Err(error);
        }
        Ok(p)
    }
    fn reconcile(&self,input:Id,store:&Store)->Result<PurgePlan,String>{
        let mut p=self.load(&input.id)?;if !unresolved(&p.status){return Err("Somente expurgo interrompido exige reconciliação. Registros encerrados continuam disponíveis para consulta.".into());}
        let inspection=self.observe(&p,store);
        if inspection.complete {
            for step in &mut p.steps{if step.status!="completed"{step.status="observed_absent".into();}}
            p.status="completed".into();p.error=None;
        }else{p.status=if inspection.conflict{"conflict"}else{"partial"}.into();p.error=Some("Inspeção não repetiu exclusões. Continuar exige o mesmo plano/digest/frase; encerrar parcialmente exige reconhecimento separado.".into());}
        p.inspection=Some(inspection);self.save(&p)?;Ok(p)
    }
    fn close(&self,input:Close,store:&Store)->Result<PurgePlan,String>{
        let mut p=self.load(&input.id)?;let note=input.note.trim();
        if p.digest!=input.digest||!unresolved(&p.status)||!input.acknowledge_partial_deletion||note.is_empty()||note.len()>4000||note.contains('\0'){
            return Err("Encerrar parcialmente exige digest exato, justificativa de 1 a 4000 bytes e reconhecimento explícito de exclusão parcial/efeitos desconhecidos.".into());
        }
        p.inspection=Some(self.observe(&p,store));p.status="abandoned_partial".into();
        p.closure=Some(Closure{at:model::now(),author:"local-user".into(),note_sha256:core::hash(note),note_bytes:note.len(),acknowledge_partial_deletion:true});
        p.error=Some("Expurgo encerrado sem continuar exclusões nem restaurar histórico removido. Dados restantes foram preservados; a trilha deste projeto pode estar incompleta. O plano não pode ser retomado.".into());self.save(&p)?;Ok(p)
    }
}

fn delete_sql(table:&str)->Result<&'static str,String>{match table {
    "messages"=>Ok("DELETE FROM messages WHERE id=?1"),"requests"=>Ok("DELETE FROM requests WHERE id=?1"),"investigations"=>Ok("DELETE FROM investigations WHERE id=?1"),
    "remediation_plans"=>Ok("DELETE FROM remediation_plans WHERE id=?1"),"bridge_runs"=>Ok("DELETE FROM bridge_runs WHERE id=?1"),_=>Err("Tabela fora do contrato de exclusão.".into()),
}}
fn compare(p:&PurgePlan,current:&Inventory)->Result<Inspection,String>{
    if p.bindings!=current.bindings{return Err("Identidade, presença ou schema de um banco mudou. O plano não autoriza uma cópia/substituição ou nova tabela.".into());}
    let original:BTreeMap<_,_>=p.rows.iter().map(|r|(key(r),r)).collect();let actual:BTreeMap<_,_>=current.rows.iter().map(|r|(key(r),r)).collect();
    let mut observation=Inspection{at:model::now(),present:0,absent:0,changed:0,new_rows:0,unobserved:0,complete:false,conflict:false,message:String::new()};
    for (key,old) in &original {
        let step=p.steps.iter().find(|step|step.database==old.database).ok_or("Etapa ausente no journal.")?;
        match actual.get(key){
            Some(now) if *now==*old=>{observation.present+=1;if matches!(step.status.as_str(),"completed"|"observed_absent"){observation.conflict=true;}},
            Some(_)=>{observation.changed+=1;observation.conflict=true;},
            None=>{observation.absent+=1;if step.status=="pending"{observation.conflict=true;}},
        }
    }
    observation.new_rows=actual.keys().filter(|key|!original.contains_key(*key)).count();observation.conflict|=observation.new_rows!=0;
    observation.complete=!observation.conflict&&observation.absent==p.rows.len()&&current.rows.is_empty();
    observation.message=if observation.complete{"Todas as linhas do escopo revisado estão ausentes nesta inspeção. Ausência observada não atribui a causa de cada remoção; nenhum dado externo ou cópia foi verificado."}else if observation.conflict{"O escopo contém alteração, linha nova, substituição ou presença incompatível com etapa já encerrada. Nada foi repetido; linhas novas/modificadas não estão autorizadas."}else{"Histórico parcialmente removido ou etapa ainda não confirmada. Esta inspeção não executou exclusões; a continuação exige confirmação explícita do plano original."}.into();
    Ok(observation)
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    use crate::{ai::AiService,executor::Executor,kali_bridge::{KaliBridge,BridgeRun},remediation::RemediationService};
    struct Fixture {_directory:tempfile::TempDir,store:Store,purge:ProjectPurgeService,project:String,other:String,assessment:String,other_assessment:String,message:String,request:String,files:Vec<PathBuf>}
    fn call(store:&Store,action:&str,payload:Value)->Value{store.handle(core::Request{action:action.into(),payload}).unwrap()}
    #[test]
    fn owned_kali_capture_requires_reviewed_retention_before_its_history_can_be_purged(){
        let(_dir,store,bridge,aid,environment)=crate::kali_bridge::completed_scan_fixture();
        let project=store.snapshot().unwrap().assessments.iter().find(|a|a.id==aid).unwrap().project_id.clone();
        call(&store,"project.archive",json!({"id":project}));
        let purge=ProjectPurgeService::new(store.root.clone()).unwrap();
        let before=store.snapshot().unwrap();
        let error=purge.handle("projectPurge.plan",json!({"projectId":project}),&store).unwrap_err();
        assert!(error.contains("kali_capture_retention_required"),"{error}");
        assert_eq!(serde_json::to_value(store.snapshot().unwrap()).unwrap(),serde_json::to_value(before).unwrap());
        let metadata=bridge.spool_metadata(&store,Some(&aid)).unwrap();assert_eq!(metadata.len(),1);
        let identity=||Ok(environment.clone());
        let backend=crate::retention::KaliRetention{bridge:&bridge,store:&store,identity:&identity};
        let retention=crate::retention::RetentionService::new(store.root.clone()).unwrap();
        let review=retention.handle_with_spool("retention.plan",json!({"items":[{"category":"kali_capture","id":metadata[0].capture_id}]}),&store,Some(&backend)).unwrap();
        assert_eq!(retention.handle_with_spool("retention.apply",json!({"id":review["id"],"digest":review["digest"],"confirmation":"EXCLUIR PERMANENTEMENTE"}),&store,Some(&backend)).unwrap()["status"],"applied");
        assert!(purge.handle("projectPurge.plan",json!({"projectId":project}),&store).unwrap_err().contains("kali_capture_metadata_retention_required"));
        let review=retention.handle_with_spool("retention.plan",json!({"items":[{"category":"kali_capture_metadata","id":metadata[0].capture_id}]}),&store,Some(&backend)).unwrap();
        assert_eq!(retention.handle_with_spool("retention.apply",json!({"id":review["id"],"digest":review["digest"],"confirmation":"EXCLUIR PERMANENTEMENTE"}),&store,Some(&backend)).unwrap()["status"],"applied");
        let plan=purge.handle("projectPurge.plan",json!({"projectId":project}),&store).unwrap();
        assert!(plan["rows"].as_array().unwrap().iter().any(|row|row["table"]=="bridge_runs"));
        assert_eq!(purge.handle("projectPurge.apply",confirmation(&plan),&store).unwrap()["status"],"completed");
        assert!(!store.snapshot().unwrap().projects.iter().any(|p|p.id==project));
        assert!(bridge.spool_metadata(&store,Some(&aid)).unwrap().is_empty());
    }
    #[test]
    fn workflow_capture_graph_is_atomic_and_preserves_other_project_and_global_ledger(){
        let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"selected bounded capture",crate::capture::CaptureState::Sealed);
        let selected=store.snapshot().unwrap().assessments.iter().find(|a|a.id==aid).unwrap().project_id.clone();
        let other=call(&store,"project.create",json!({"name":"Other preserved project","kind":"web","target":"https://preserved.invalid"}))["id"].as_str().unwrap().to_owned();
        let other_aid=call(&store,"assessment.create",json!({"projectId":other,"title":"Other workflow","kind":"web","target":"https://preserved.invalid","objective":"Preserve this independent workflow","scope":crate::scope::test_creation(&store,&other,&["http.headers.v1"])}))["id"].as_str().unwrap().to_owned();
        let workflow=crate::workflow::WorkflowService::default();let redactor=crate::data_policy::test_support::TestRedactor;
        let draft=workflow.prepare(&store,crate::workflow::PlanPrepareRequest{assessment_id:other_aid.clone(),expected_head:None,expected_generation:0,goal:"Preserve this independent plan".into(),nodes:vec![crate::workflow::NodeDraft{key:"observe".into(),existing_node_id:None,purpose:"Review selected header metadata".into(),expected_observation:"Future bounded observation".into(),depends_on_keys:vec![],intent:crate::workflow::DraftIntent::Native{adapter:crate::workflow::NativeAdapter::HttpHeaders,target_ref:None,ports:None}}],limits:crate::workflow::PlanLimits{max_attempts_total:1,max_capture_bytes_total:crate::capture::MAX_CAPTURE_BYTES}},&redactor).unwrap();
        workflow.commit(&store,crate::workflow::PlanCommitRequest{review:draft.review,idempotency_key:model::id(),acknowledge_plan:true},&redactor).unwrap();
        call(&store,"project.archive",json!({"id":selected}));call(&store,"project.archive",json!({"id":other}));
        let other_before=core_graph(&store.snapshot().unwrap(),&other,None).unwrap().rows;
        let other_work_before=store.work_read(|conn,_|work_graph(conn,&[other_aid.clone()],None)).unwrap();
        fs::write(store.root.join("ai-budget.sqlite3"),b"global ledger must remain").unwrap();
        store.capture_transaction(|conn|{conn.execute("INSERT INTO capture_cursors VALUES(?1,?2,?3,?4,?5,0,?6)",params![model::id(),aid,manifest.id,manifest.digest,core::hash("projection"),model::now()]).map_err(failure)?;Ok(())}).unwrap();
        let purge=ProjectPurgeService::new(store.root.clone()).unwrap();
        let plan=purge.handle("projectPurge.plan",json!({"projectId":selected}),&store).unwrap();
        for table in ["work_plan_heads","work_plan_revisions","work_jobs","work_attempts","work_plan_bindings","work_commands","work_events","capture_manifests","capture_chunks","capture_cursors"]{assert!(plan["rows"].as_array().unwrap().iter().any(|r|r["table"]==table),"missing {table}");}
        *purge.fail_after_commit.lock().unwrap()=Some(Database::Core);
        assert!(purge.handle("projectPurge.apply",confirmation(&plan),&store).is_err());
        store.work_read(|conn,s|{assert!(work_graph(conn,&[aid.clone()],None)?.is_empty());assert_eq!(work_graph(conn,&[other_aid.clone()],None)?,other_work_before);assert!(!s.assessments.iter().any(|a|a.id==aid));assert!(!conn.prepare("PRAGMA foreign_key_check").unwrap().exists([]).unwrap());Ok(())}).unwrap();
        assert_eq!(core_graph(&store.snapshot().unwrap(),&other,None).unwrap().rows,other_before);
        assert_eq!(fs::read(store.root.join("ai-budget.sqlite3")).unwrap(),b"global ledger must remain");
        let root=store.root.clone();drop(purge);let resumed=ProjectPurgeService::new(root).unwrap();
        let inspected=resumed.handle("projectPurge.reconcile",json!({"id":plan["id"]}),&store).unwrap();
        assert_eq!(inspected["status"],"completed");assert_eq!(inspected["steps"][3]["status"],"observed_absent");
        assert!(resumed.handle("projectPurge.apply",confirmation(&plan),&store).is_err());
    }
    #[test]
    fn new_capture_cursor_after_purge_review_blocks_the_entire_graph(){
        let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"capture preserved after stale review",crate::capture::CaptureState::Sealed);
        let pid=store.snapshot().unwrap().assessments.iter().find(|a|a.id==aid).unwrap().project_id.clone();call(&store,"project.archive",json!({"id":pid}));
        let purge=ProjectPurgeService::new(store.root.clone()).unwrap();let plan=purge.handle("projectPurge.plan",json!({"projectId":pid}),&store).unwrap();
        store.capture_transaction(|conn|{conn.execute("INSERT INTO capture_cursors VALUES(?1,?2,?3,?4,?5,0,?6)",params![model::id(),aid,manifest.id,manifest.digest,core::hash("new projection"),model::now()]).map_err(failure)?;Ok(())}).unwrap();
        assert!(purge.handle("projectPurge.apply",confirmation(&plan),&store).is_err());
        store.work_read(|conn,s|{assert!(s.projects.iter().any(|p|p.id==pid));assert_eq!(crate::capture::preserved_locked(conn,&manifest)?,b"capture preserved after stale review");Ok(())}).unwrap();
    }
    #[test]
    fn schema_matches_all_implicit_unique_indexes_and_rejects_any_extra_object(){
        let db=Connection::open_in_memory().unwrap();for (_,ddl) in expected_schema(Database::Core){db.execute_batch(ddl).unwrap();}db.pragma_update(None,"user_version",core::STORE_VERSION).unwrap();
        assert!(schema(&db,&expected_schema(Database::Core)).is_ok());
        db.execute_batch("CREATE INDEX unreviewed_work_index ON work_jobs(assessment_id)").unwrap();assert!(schema(&db,&expected_schema(Database::Core)).is_err());
        db.execute_batch("DROP INDEX unreviewed_work_index; CREATE TRIGGER unreviewed_work_trigger AFTER DELETE ON work_jobs BEGIN DELETE FROM work_commands; END").unwrap();assert!(schema(&db,&expected_schema(Database::Core)).is_err());
    }
    fn fixture()->Fixture {
        let directory=tempfile::tempdir().unwrap();let root=directory.path().join("profile");let store=Store::open(root.clone()).unwrap();
        let ai=AiService::new(root.clone()).unwrap();let remediation=RemediationService::new(root.clone()).unwrap();let bridge=KaliBridge::new(root.clone()).unwrap();let executor=Executor::default();
        ai.handle("ai.key.save",json!({"key":"synthetic-key-for-offline-purge-fixture"}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
        let mut projects=vec![];let mut assessments=vec![];let mut messages=vec![];let mut requests=vec![];let mut files=vec![];
        for index in 0..2 {
            let target=directory.path().join(format!("source-{index}"));fs::create_dir(&target).unwrap();let file=target.join("settings.txt");fs::write(&file,format!("secure=false\nSOURCE-SECRET-SENTINEL-{index}\n")).unwrap();files.push(file);
            let p=call(&store,"project.create",json!({"name":format!("Synthetic {index}"),"kind":"code","target":target.to_string_lossy()}));
            let a=call(&store,"assessment.create",json!({"projectId":p["id"],"title":"Fixture","kind":"code","target":target.to_string_lossy(),"objective":"Only synthetic local bytes","scope":crate::scope::test_creation(&store,p["id"].as_str().unwrap(),&["local.inventory.v1","local.text-remediation.v1","ai.remote-review.v1"])}));
            let project=p["id"].as_str().unwrap().to_owned();let assessment=a["id"].as_str().unwrap().to_owned();
            let execution_plan=executor.handle("assessment.plan",json!({"id":assessment}),&store).unwrap();let decision=json!({"id":execution_plan["id"],"digest":execution_plan["digest"]});
            executor.handle("plan.approve",decision.clone(),&store).unwrap();executor.handle("plan.run",decision,&store).unwrap();
            let finding=model::id();let evidence=model::id();let content=format!("EVIDENCE-SECRET-SENTINEL-{index}");
            store.mutate(|s|{
                s.assessments.iter_mut().find(|a|a.id==assessment).unwrap().evidence.push(model::Evidence{id:evidence.clone(),name:"fixture.txt".into(),content:content.clone(),sha256:core::hash(&content),created_at:model::now()});
                s.findings.push(model::Finding{id:finding.clone(),assessment_id:assessment.clone(),title:"Synthetic predicate".into(),severity:"low".into(),confidence:"Human fixture review".into(),status:"validated".into(),description:"Fixture only".into(),recommendation:String::new(),evidence_ids:vec![evidence.clone()],created_at:model::now()});
                s.validations.push(model::ValidationRecord{id:model::id(),finding_id:finding.clone(),assessment_id:assessment.clone(),method:"human_review".into(),expected:"Synthetic observation".into(),observed:"Only this fixture".into(),evidence:vec![model::ValidatedEvidence{id:evidence.clone(),sha256:core::hash(&content)}],author:"local-user".into(),created_at:model::now()});Ok(())
            }).unwrap();
            let patch=remediation.handle("remediation.prepare",json!({"findingId":finding,"relativePath":"settings.txt","after":format!("secure=true\nSOURCE-SECRET-SENTINEL-{index}\n"),"retest":{"kind":"contains","value":"secure=true"}}),&store).unwrap();
            let decision=json!({"id":patch["id"],"digest":patch["digest"]});remediation.handle("remediation.approve",decision.clone(),&store).unwrap();remediation.handle("remediation.apply",decision,&store).unwrap();
            let input=ai.test_data_input(&store,"investigation",json!({"assessmentId":assessment,"evidenceIds":[evidence],"message":"Offline fixture review","perspectives":["evidence"],"maxOutputTokensPerRequest":512,"maxTotalTokens":100000}));
            ai.handle("ai.investigation.plan",input,&store).unwrap();
            let assistant=Connection::open(root.join("assistant.sqlite3")).unwrap();let message=model::id();let request=model::id();
            assistant.execute("INSERT INTO messages VALUES(?1,?2,'user',?3,?4)",params![message,assessment,format!("MESSAGE-SECRET-SENTINEL-{index}"),model::now()]).unwrap();
            assistant.execute("INSERT INTO requests VALUES(?1,?2,'https://api.openai.com/v1','fixture-model','fixture-revision','fixture-context','completed',?3,5,7,'{}')",params![request,assessment,model::now()]).unwrap();
            let run=BridgeRun{work_context:None,reconciliation_of:None,scope_ref:None,source_target_refs:vec![],id:model::id(),assessment_id:assessment.clone(),environment_id:model::id(),tool:"runtime_diagnostics".into(),status:"completed".into(),started_at:model::now(),finished_at:Some(model::now()),source_sha256:"a".repeat(64),evidence_id:Some(evidence),error:None,cleanup_confirmed:true,launch_attempted:true,dispatch_before_unix_seconds:1,source_evidence_id:None};
            Connection::open(root.join("kali-bridge.sqlite3")).unwrap().execute("INSERT INTO bridge_runs VALUES(?1,?2)",params![run.id,serde_json::to_string(&run).unwrap()]).unwrap();
            let scope=crate::scope::status(&store,&assessment).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":assessment,"expectedStateDigest":scope["stateDigest"],"idempotencyKey":model::id(),"rationale":"End fixture admissions before purge","acknowledgeRevocation":true}),&store,str::to_owned).unwrap();
            call(&store,"project.archive",json!({"id":project}));projects.push(project);assessments.push(assessment);messages.push(message);requests.push(request);
        }
        let assistant=Connection::open(root.join("assistant.sqlite3")).unwrap();assistant.execute("INSERT INTO requests VALUES(?1,'connection-test','https://api.openai.com/v1','fixture-model','fixture-revision','fixture-context','completed',?2,1,1,'{}')",params![model::id(),model::now()]).unwrap();
        assistant.execute("INSERT INTO connection_check VALUES(1,'fixture-model','https://api.openai.com/v1','fixture-revision',?1)",[model::now()]).unwrap();drop(assistant);drop(ai);drop(remediation);drop(bridge);
        for name in ["backups","exports"]{fs::create_dir(root.join(name)).unwrap();fs::write(root.join(name).join("preserve-fixture.txt"),b"excluded copy").unwrap();}
        let purge=ProjectPurgeService::new(root).unwrap();
        Fixture{_directory:directory,store,purge,project:projects[0].clone(),other:projects[1].clone(),assessment:assessments[0].clone(),other_assessment:assessments[1].clone(),message:messages[0].clone(),request:requests[0].clone(),files}
    }
    fn plan(f:&Fixture)->Value {f.purge.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).unwrap()}
    fn confirmation(p:&Value)->Value{json!({"id":p["id"],"digest":p["digest"],"confirmation":PHRASE})}
    fn inventory(f:&Fixture,project:&str)->Vec<RowDigest>{let (opened,bindings)=f.purge.open_databases(false).unwrap();f.purge.inventory(&f.store,project,None,&opened,bindings,true).unwrap().rows}
    fn db(f:&Fixture,kind:Database)->Connection{Connection::open(f.store.root.join(kind.name())).unwrap()}
    fn row_count(f:&Fixture,table:&str)->i64 {let kind=match table{"messages"|"requests"|"investigations"=>Database::Assistant,"remediation_plans"=>Database::Remediation,"bridge_runs"=>Database::Bridge,_=>panic!("fixture table")};db(f,kind).query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get(0)).unwrap()}

    #[test]
    fn reviewed_purge_removes_complete_selected_graph_preserves_other_graph_and_exclusions() {
        let f=fixture();let originals=f.files.iter().map(|p|fs::read(p).unwrap()).collect::<Vec<_>>();
        let ai=AiService::new(f.store.root.clone()).unwrap();
        ai.handle("ai.key.save",json!({"provider":"groq","key":"synthetic-purge-groq-key"}),&f.store).unwrap();drop(ai);
        let other=inventory(&f,&f.other);
        let vault:Vec<u8>=db(&f,Database::Assistant).query_row("SELECT cipher FROM vault WHERE id=1",[],|r|r.get(0)).unwrap();let p=plan(&f);
        for sentinel in ["SECRET-SENTINEL","secure=false","synthetic-key-for-offline-purge-fixture"]{assert!(!p.to_string().contains(sentinel));}
        assert_eq!(p["counts"]["projects"],1);assert_eq!(p["counts"]["assessments"],1);assert_eq!(p["counts"]["plans"],1);assert_eq!(p["counts"]["executions"],1);assert_eq!(p["counts"]["remediations"],1);assert_eq!(p["counts"]["investigations"],1);
        let result=f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).unwrap();assert_eq!(result["status"],"completed");assert_eq!(result["inspection"]["complete"],true);assert!(!f.purge.has_unresolved().unwrap());
        assert_eq!(inventory(&f,&f.other),other);let snapshot=f.store.snapshot().unwrap();assert!(!snapshot.projects.iter().any(|v|v.id==f.project));assert!(!snapshot.assessments.iter().any(|v|v.id==f.assessment));assert!(!snapshot.data_subject_generations.iter().any(|g|g.subject.assessment_id()==f.assessment));core::validate_snapshot(&snapshot).unwrap();
        for table in ["messages","investigations","remediation_plans","bridge_runs"]{assert_eq!(row_count(&f,table),1);}assert_eq!(row_count(&f,"requests"),2);
        let after:Vec<u8>=db(&f,Database::Assistant).query_row("SELECT cipher FROM vault WHERE id=1",[],|r|r.get(0)).unwrap();assert_eq!(after,vault);
        assert_eq!(db(&f,Database::Assistant).query_row("SELECT COUNT(*) FROM vault",[],|r|r.get::<_,u32>(0)).unwrap(),2);
        assert_eq!(db(&f,Database::Assistant).query_row("SELECT count(*) FROM connection_check",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        for (path,bytes) in f.files.iter().zip(originals){assert_eq!(fs::read(path).unwrap(),bytes);}for name in ["backups","exports"]{assert_eq!(fs::read(f.store.root.join(name).join("preserve-fixture.txt")).unwrap(),b"excluded copy");}
        assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert_eq!(inventory(&f,&f.other),other);
    }
    #[test]
    fn archive_digest_and_exact_confirmation_are_required_before_any_deletion() {
        let f=fixture();f.store.mutate(|s|{s.projects.iter_mut().find(|v|v.id==f.project).unwrap().archived=false;Ok(())}).unwrap();assert!(f.purge.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).is_err());
        call(&f.store,"project.archive",json!({"id":f.project}));let p=plan(&f);let before=inventory(&f,&f.project);
        for payload in [json!({"id":p["id"],"digest":"forged","confirmation":PHRASE}),json!({"id":p["id"],"digest":p["digest"],"confirmation":"EXCLUIR"}),json!({"id":p["id"],"digest":p["digest"],"confirmation":PHRASE,"database":"outside.sqlite3"})]{assert!(f.purge.handle("projectPurge.apply",payload,&f.store).is_err());}
        assert_eq!(inventory(&f,&f.project),before);assert!(!f.purge.has_unresolved().unwrap());
    }
    #[test]
    fn any_selected_core_or_auxiliary_change_rejects_whole_scope_before_first_delete() {
        for change in ["core","message","new-row","moved-owner"] {
            let f=fixture();let p=plan(&f);let other=inventory(&f,&f.other);
            match change {
                "core"=>{f.store.mutate(|s|{s.projects.iter_mut().find(|v|v.id==f.project).unwrap().description="new reviewed scope required".into();Ok(())}).unwrap();},
                "message"=>{db(&f,Database::Assistant).execute("UPDATE messages SET text='new content' WHERE id=?1",[&f.message]).unwrap();},
                "new-row"=>{db(&f,Database::Assistant).execute("INSERT INTO messages VALUES(?1,?2,'user','new row',?3)",params![model::id(),f.assessment,model::now()]).unwrap();},
                _=>{db(&f,Database::Assistant).execute("UPDATE messages SET assessment=?1 WHERE id=?2",params![f.other_assessment,f.message]).unwrap();},
            }
            let counts=[row_count(&f,"messages"),row_count(&f,"requests"),row_count(&f,"remediation_plans")];assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());
            assert_eq!([row_count(&f,"messages"),row_count(&f,"requests"),row_count(&f,"remediation_plans")],counts);assert_eq!(f.purge.load(p["id"].as_str().unwrap()).unwrap().status,"rejected");assert!(!f.purge.has_unresolved().unwrap());
            if change!="moved-owner"{assert_eq!(inventory(&f,&f.other),other);}
        }
    }
    #[test]
    fn scope_ref_does_not_replace_required_investigation_fields_before_purge(){
        for replacement in [json!("invalid scope ref"),json!({"assessmentId":model::id(),"revisionId":model::id(),"digest":"a".repeat(64)})]{let f=fixture();let database=db(&f,Database::Assistant);let(id,raw):(String,String)=database.query_row("SELECT id,data FROM investigations WHERE assessment=?1",[&f.assessment],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();let mut value:Value=serde_json::from_str(&raw).unwrap();value.as_object_mut().unwrap().remove("budget");value["scopeRef"]=replacement;database.execute("UPDATE investigations SET data=?1 WHERE id=?2",params![value.to_string(),id]).unwrap();
            let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();let counts=[row_count(&f,"messages"),row_count(&f,"remediation_plans"),row_count(&f,"bridge_runs")];assert!(f.purge.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).is_err(),"scopeRef must never substitute a required investigation field");assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert_eq!([row_count(&f,"messages"),row_count(&f,"remediation_plans"),row_count(&f,"bridge_runs")],counts);assert!(f.purge.plans().unwrap().is_empty());}
    }
    #[test]
    fn unknown_tables_columns_views_triggers_and_versions_block_purge() {
        for ddl in ["CREATE TABLE unknown_extension(id TEXT)","ALTER TABLE messages ADD COLUMN unreviewed TEXT","CREATE VIEW hidden_copy AS SELECT * FROM messages","CREATE TRIGGER arbitrary_effect AFTER DELETE ON messages BEGIN DELETE FROM requests; END","PRAGMA user_version=99"] {
            let f=fixture();db(&f,Database::Assistant).execute_batch(ddl).unwrap();assert!(f.purge.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).is_err());assert_eq!(row_count(&f,"messages"),2);assert_eq!(row_count(&f,"requests"),3);
        }
    }
    #[test]
    fn global_active_and_unknown_operations_block_even_an_archived_project() {
        for active in ["core","request","investigation","remediation","bridge"] {
            let f=fixture();match active {
                "core"=>f.store.mutate(|s|{s.assessments.iter_mut().find(|v|v.id==f.other_assessment).unwrap().status="unknown".into();Ok(())}).unwrap(),
                "request"=>{db(&f,Database::Assistant).execute("UPDATE requests SET status='unknown' WHERE id=?1",[&f.request]).unwrap();},
                "investigation"=>{db(&f,Database::Assistant).execute("UPDATE investigations SET data=json_set(data,'$.status','unknown') WHERE assessment=?1",[&f.other_assessment]).unwrap();},
                "remediation"=>{db(&f,Database::Remediation).execute("UPDATE remediation_plans SET data=json_set(data,'$.status','unknown')",[]).unwrap();},
                _=>{db(&f,Database::Bridge).execute("UPDATE bridge_runs SET data=json_set(data,'$.cleanupConfirmed',json('false'))",[]).unwrap();},
            }
            assert!(f.purge.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).is_err());assert_eq!(row_count(&f,"messages"),2);
        }
    }
    #[test]
    fn acknowledged_unknown_usage_is_counted_and_is_never_reclassified_as_zero() {
        let f=fixture();db(&f,Database::Assistant).execute("UPDATE requests SET status='unknown',input_tokens=NULL,output_tokens=NULL WHERE id=?1",[&f.request]).unwrap();
        assert!(f.purge.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).is_err());
        let ai=AiService::new(f.store.root.clone()).unwrap();ai.handle("ai.request.reconcile",json!({"assessmentId":f.assessment,"requestId":f.request,"note":"Provider usage remains unknown after human review.","acknowledgeUnknownUsage":true}),&f.store).unwrap();drop(ai);
        let p=plan(&f);assert_eq!(p["counts"]["requestsUnknownAcknowledged"],1);
        let state:(String,Option<i64>,Option<i64>)=db(&f,Database::Assistant).query_row("SELECT status,input_tokens,output_tokens FROM requests WHERE id=?1",[&f.request],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();assert_eq!(state,("unknown".into(),None,None));
    }
    #[test]
    fn interrupted_auxiliary_commit_is_inspected_without_replay_then_explicitly_resumed() {
        let f=fixture();let p=plan(&f);let other=inventory(&f,&f.other);*f.purge.fail_after_commit.lock().unwrap()=Some(Database::Assistant);
        assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert!(f.purge.has_unresolved().unwrap());assert_eq!(row_count(&f,"messages"),1);assert_eq!(row_count(&f,"remediation_plans"),2);assert!(f.store.snapshot().unwrap().projects.iter().any(|v|v.id==f.project));
        let root=f.store.root.clone();drop(f.purge);let restarted=ProjectPurgeService::new(root).unwrap();assert!(restarted.has_unresolved().unwrap());
        let inspected=restarted.handle("projectPurge.reconcile",json!({"id":p["id"]}),&f.store).unwrap();assert_eq!(inspected["status"],"partial");assert_eq!(Connection::open(f.store.root.join("remediation.sqlite3")).unwrap().query_row("SELECT count(*) FROM remediation_plans",[],|r|r.get::<_,i64>(0)).unwrap(),2);
        let result=restarted.handle("projectPurge.apply",confirmation(&p),&f.store).unwrap();assert_eq!(result["status"],"completed");assert!(!restarted.has_unresolved().unwrap());assert_eq!(result["steps"][0]["status"],"observed_absent");assert_eq!(result["steps"][0]["removedRows"],0);
        let (opened,bindings)=restarted.open_databases(false).unwrap();assert_eq!(restarted.inventory(&f.store,&f.other,None,&opened,bindings,true).unwrap().rows,other);
    }
    #[test]
    fn interrupted_purge_journal_survives_v2_to_v3_semantic_domain_migration() {
        let f=fixture();let p=plan(&f);let other_rows=inventory(&f,&f.other);let original_files=f.files.iter().map(|path|fs::read(path).unwrap()).collect::<Vec<_>>();
        *f.purge.fail_after_commit.lock().unwrap()=Some(Database::Assistant);
        assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());
        let root=f.store.root.clone();let other_id=f.other.clone();let selected_id=f.project.clone();
        let core_path=root.join("openkoda.sqlite3");let db=Connection::open(&core_path).unwrap();
        let before:String=db.query_row("SELECT json FROM app_state WHERE id=1",[],|row|row.get(0)).unwrap();
        // The v1 purge journal/DDL/row digests are unchanged between domain2/3.
        // Model a v2 installation stopped after its assistant DB commit, then
        // let the real Store::open migration advance only semantic user_version.
        db.pragma_update(None,"user_version",2).unwrap();drop(db);drop(f.purge);drop(f.store);
        let store=Store::open(root.clone()).unwrap();
        let db=Connection::open(&core_path).unwrap();let after:String=db.query_row("SELECT json FROM app_state WHERE id=1",[],|row|row.get(0)).unwrap();let version:i64=db.query_row("PRAGMA user_version",[],|row|row.get(0)).unwrap();assert_eq!(before,after);assert_eq!(version,core::STORE_VERSION);drop(db);
        let resumed=ProjectPurgeService::new(root.clone()).unwrap();let inspected=resumed.handle("projectPurge.reconcile",json!({"id":p["id"]}),&store).unwrap();assert_eq!(inspected["status"],"partial");
        assert_eq!(Connection::open(root.join("assistant.sqlite3")).unwrap().query_row("SELECT count(*) FROM messages",[],|row|row.get::<_,i64>(0)).unwrap(),1);
        let result=resumed.handle("projectPurge.apply",confirmation(&p),&store).unwrap();assert_eq!(result["status"],"completed");assert_eq!(result["steps"][0]["removedRows"],0);assert_eq!(result["steps"][0]["status"],"observed_absent");assert!(!resumed.has_unresolved().unwrap());
        assert!(!store.snapshot().unwrap().projects.iter().any(|project|project.id==selected_id));let(opened,bindings)=resumed.open_databases(false).unwrap();assert_eq!(resumed.inventory(&store,&other_id,None,&opened,bindings,true).unwrap().rows,other_rows);
        for(path,original)in f.files.iter().zip(original_files){assert_eq!(fs::read(path).unwrap(),original);}
    }
    #[test]
    fn legacy_v3_purge_resumes_after_v4_without_rewriting_legacy_rows(){
        let dir=tempfile::tempdir().unwrap();let root=dir.path().join("profile");let store=Store::open(root.clone()).unwrap();let ai=AiService::new(root.clone()).unwrap();
        let p=call(&store,"project.create",json!({"name":"Legacy purge","kind":"web","target":"https://legacy.invalid/"}));let a=call(&store,"assessment.create",json!({"projectId":p["id"],"title":"Old draft","kind":"web","target":p["target"],"objective":"Legacy unchanged objective"}));
        store.mutate(|s|{s.assessments[0].scope_id=None;s.assessment_scopes.clear();Ok(())}).unwrap();call(&store,"project.archive",json!({"id":p["id"]}));
        let db=Connection::open(root.join("assistant.sqlite3")).unwrap();db.execute("INSERT INTO messages VALUES(?1,?2,'user','legacy text',?3)",params![model::id(),a["id"].as_str().unwrap(),model::now()]).unwrap();drop(db);drop(ai);
        let purge=ProjectPurgeService::new(root.clone()).unwrap();let plan=purge.handle("projectPurge.plan",json!({"projectId":p["id"]}),&store).unwrap();assert!(!plan["rows"].as_array().unwrap().iter().any(|r|r["table"].as_str().unwrap().contains("cope")));*purge.fail_after_commit.lock().unwrap()=Some(Database::Assistant);assert!(purge.handle("projectPurge.apply",confirmation(&plan),&store).is_err());
        let before=serde_json::to_string(&store.snapshot().unwrap()).unwrap();drop(purge);drop(store);let db=Connection::open(root.join("openkoda.sqlite3")).unwrap();db.pragma_update(None,"user_version",3).unwrap();drop(db);
        let store=Store::open(root.clone()).unwrap();assert_eq!(serde_json::to_string(&store.snapshot().unwrap()).unwrap(),before);let purge=ProjectPurgeService::new(root).unwrap();assert_eq!(purge.handle("projectPurge.reconcile",json!({"id":plan["id"]}),&store).unwrap()["status"],"partial");assert_eq!(purge.handle("projectPurge.apply",confirmation(&plan),&store).unwrap()["status"],"completed");assert!(store.snapshot().unwrap().projects.is_empty());
    }
    #[test]
    fn changed_remaining_scope_requires_human_partial_close_preserving_new_data() {
        let f=fixture();let p=plan(&f);*f.purge.fail_after_commit.lock().unwrap()=Some(Database::Assistant);assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());
        f.store.mutate(|s|{s.assessments.iter_mut().find(|v|v.id==f.assessment).unwrap().summary="changed after interruption".into();Ok(())}).unwrap();
        let counts=[row_count(&f,"messages"),row_count(&f,"remediation_plans"),row_count(&f,"bridge_runs")];let inspected=f.purge.handle("projectPurge.reconcile",json!({"id":p["id"]}),&f.store).unwrap();assert_eq!(inspected["status"],"conflict");
        assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert_eq!([row_count(&f,"messages"),row_count(&f,"remediation_plans"),row_count(&f,"bridge_runs")],counts);
        let close=json!({"id":p["id"],"digest":p["digest"],"note":"CLOSE-NOTE-SENTINEL: retain the changed project with partial history.","acknowledgePartialDeletion":true});let mut denied=close.clone();denied["acknowledgePartialDeletion"]=json!(false);assert!(f.purge.handle("projectPurge.closePartial",denied,&f.store).is_err());
        let result=f.purge.handle("projectPurge.closePartial",close,&f.store).unwrap();assert_eq!(result["status"],"abandoned_partial");assert!(!result.to_string().contains("CLOSE-NOTE-SENTINEL"));assert!(!f.purge.has_unresolved().unwrap());assert_eq!(f.purge.handle("projectPurge.status",json!({}),&f.store).unwrap()["hasPartialHistory"],true);assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert_eq!(row_count(&f,"remediation_plans"),2);
    }
    #[test]
    fn recreated_exact_row_after_completed_stage_is_not_deleted_again() {
        let f=fixture();let assistant=db(&f,Database::Assistant);let original:(i64,String,String,String,String,String)=assistant.query_row("SELECT rowid,id,assessment,role,text,created FROM messages WHERE id=?1",[&f.message],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();drop(assistant);
        let p=plan(&f);*f.purge.fail_after_commit.lock().unwrap()=Some(Database::Remediation);assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());
        db(&f,Database::Assistant).execute("INSERT INTO messages(rowid,id,assessment,role,text,created) VALUES(?1,?2,?3,?4,?5,?6)",params![original.0,original.1,original.2,original.3,original.4,original.5]).unwrap();
        let inspected=f.purge.handle("projectPurge.reconcile",json!({"id":p["id"]}),&f.store).unwrap();assert_eq!(inspected["status"],"conflict");assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert_eq!(row_count(&f,"messages"),2);assert_eq!(row_count(&f,"bridge_runs"),2);
    }
    #[test]
    fn interruption_after_core_commit_needs_inspection_not_replay() {
        let f=fixture();let p=plan(&f);*f.purge.fail_after_commit.lock().unwrap()=Some(Database::Core);assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert!(!f.store.snapshot().unwrap().projects.iter().any(|v|v.id==f.project));
        assert!(f.purge.has_unresolved().unwrap());let result=f.purge.handle("projectPurge.reconcile",json!({"id":p["id"]}),&f.store).unwrap();assert_eq!(result["status"],"completed");assert_eq!(result["steps"][3]["status"],"observed_absent");assert_eq!(result["steps"][3]["removedRows"],0);assert!(!f.purge.has_unresolved().unwrap());
    }
    #[test]
    fn restart_revokes_unused_review_and_other_profile_cannot_reuse_unfinished_authority() {
        let f=fixture();let unused=plan(&f);let root=f.store.root.clone();drop(f.purge);let reopened=ProjectPurgeService::new(root.clone()).unwrap();assert_eq!(reopened.load(unused["id"].as_str().unwrap()).unwrap().status,"revoked");assert!(reopened.handle("projectPurge.apply",confirmation(&unused),&f.store).is_err());
        let p=reopened.handle("projectPurge.plan",json!({"projectId":f.project}),&f.store).unwrap();*reopened.fail_after_commit.lock().unwrap()=Some(Database::Assistant);assert!(reopened.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());
        let copy=f._directory.path().join("copied-profile");fs::create_dir(&copy).unwrap();for name in ["openkoda.sqlite3","assistant.sqlite3","remediation.sqlite3","kali-bridge.sqlite3",JOURNAL]{crate::recovery::backup_database(&root.join(name),&copy.join(name)).unwrap();}
        let copied_store=Store::open(copy.clone()).unwrap();let copied=ProjectPurgeService::new(copy).unwrap();assert!(copied.has_unresolved().unwrap());assert!(copied.handle("projectPurge.apply",confirmation(&p),&copied_store).is_err());
        let inspected=copied.handle("projectPurge.reconcile",json!({"id":p["id"]}),&copied_store).unwrap();assert_eq!(inspected["status"],"conflict");assert!(inspected["inspection"]["unobserved"].as_u64().unwrap()>0);assert_eq!(copied_store.snapshot().unwrap().projects.len(),2);
        copied.handle("projectPurge.closePartial",json!({"id":p["id"],"digest":p["digest"],"note":"Copied profile retains partial history; no old file identity is reused.","acknowledgePartialDeletion":true}),&copied_store).unwrap();assert!(!copied.has_unresolved().unwrap());assert!(reopened.has_unresolved().unwrap());
    }
    #[test]
    fn identical_database_copy_cannot_replace_the_reviewed_database_identity() {
        let f=fixture();let p=plan(&f);let source=f.store.root.join("assistant.sqlite3");let copy=f.store.root.join("assistant-copy.sqlite3");crate::recovery::backup_database(&source,&copy).unwrap();
        fs::rename(&source,f.store.root.join("assistant-original.sqlite3")).unwrap();fs::rename(&copy,&source).unwrap();
        assert!(f.purge.handle("projectPurge.apply",confirmation(&p),&f.store).is_err());assert_eq!(row_count(&f,"messages"),2);assert_eq!(row_count(&f,"remediation_plans"),2);
    }
}

