//! Explicit retention of managed copies and sealed capture bytes.
//! Windows deletion uses the already-verified file handle, never a reused path.
//! A batch is not a filesystem transaction: each deletion has a durable intent,
//! interrupted batches require inspection and are never replayed on startup.
use crate::{core::{self, Store}, model};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::{self, File}, io::{Read, Seek, SeekFrom}, path::{Component, Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};
#[path="retention_spool.rs"]
mod spool;
pub(crate) use spool::{KaliRetention,SpoolBackend,SpoolOutcome,SpoolPreview};
#[cfg(test)]
use spool::SpoolItem;

const MAX_FILE:u64=128*1024*1024;
const MAX_BATCH:u64=1024*1024*1024;
const MAX_FILES:usize=256;
const PHRASE:&str="EXCLUIR PERMANENTEMENTE";
const LIMITS:[&str;5]=[
    "Remove somente as cópias geridas e os bytes de capturas selecionados neste perfil. Cópias locais e capturas Linux são itens separados. Capturas mantêm manifesto, origem e hashes históricos; projetos e demais evidências não são expurgados aqui.",
    "Não remove cópias externas, outros perfis, restaurações, arquivos fonte, dados do provedor, memória ou caches do sistema operacional. Não é apagamento seguro.",
    "O lote não é atômico. Falha pode deixar remoção parcial; o registro preserva identidades e hashes, sem repetir exclusões automaticamente.",
    "Hashes identificam os bytes selecionados; este plano não certifica que um backup pode ser restaurado. Revise antes de excluir a última cópia.",
    "O prazo de retenção é uma preferência; nenhuma seleção ou exclusão automática acontece nesta versão.",
];

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq,Hash)]
#[serde(rename_all="lowercase")]
enum Category { Backup, Report, Markdown, Capture, #[serde(rename="kali_capture")] KaliCapture, #[serde(rename="kali_capture_metadata")] KaliCaptureMetadata }
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq,Hash)]
#[serde(deny_unknown_fields)]
struct ItemRef {category:Category,id:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
// Decimal strings keep the full filesystem identity in JavaScript previews.
struct Identity {volume:String,index:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct FilePreview {name:String,bytes:u64,sha256:String,identity:Identity}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Artifact {category:Category,id:String,label:String,bytes:u64,file_count:usize,files:Vec<FilePreview>,identity:Identity,#[serde(default,skip_serializing_if="Option::is_none")]capture:Option<CapturePreview>,#[serde(default,skip_serializing_if="Option::is_none")]kali_capture:Option<SpoolPreview>,#[serde(default,skip_serializing_if="Option::is_none")]kali_metadata_item:Option<crate::kali_bridge::KaliMetadataItem>,#[serde(default,skip_serializing_if="Option::is_none")]kali_metadata:Option<crate::kali_bridge::KaliRetirePreview>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CapturePreview {assessment_id:String,manifest_digest:String,purged_manifest_digest:String,generation:u64,preserved_sha256:String,chunk_count:u64}
impl Artifact {fn reference(&self)->ItemRef {ItemRef{category:self.category.clone(),id:self.id.clone()}}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Progress {category:Category,id:String,name:String,status:String,sha256:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct RetentionPlan {
    id:String,digest:String,status:String,created_at:String,expires_at:String,
    items:Vec<Artifact>,total_bytes:u64,total_files:usize,confirmation_phrase:String,
    limits:Vec<String>,progress:Vec<Progress>,error:Option<String>,reconciled:bool,
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct Selection {items:Vec<ItemRef>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct Apply {id:String,digest:String,confirmation:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct IdInput {id:String}

pub struct RetentionService {root:PathBuf,db:Mutex<Connection>,operation:Mutex<()>,deleter:Arc<dyn Deleter>}
trait Deleter:Send+Sync {fn delete(&self,file:File)->Result<(),String>;}
struct NativeDeleter;
impl Deleter for NativeDeleter {fn delete(&self,file:File)->Result<(),String>{delete_handle(file)}}
struct LockedFile {preview:FilePreview,file:File}
struct LockedArtifact {preview:Artifact,files:Vec<LockedFile>,directory:Option<File>,_ancestors:Vec<File>,path:PathBuf}

pub fn supports(action:&str)->bool {matches!(action,"retention.inventory"|"retention.plan"|"retention.apply"|"retention.reconcile")}
pub fn mutates(action:&str)->bool {matches!(action,"retention.plan"|"retention.apply"|"retention.reconcile")}
fn err(_:impl std::fmt::Display)->String {"Falha no registro ou arquivo de retenção. Consulte o inventário; nenhuma exclusão será repetida automaticamente.".into()}
fn decode<T:serde::de::DeserializeOwned>(v:Value)->Result<T,String>{serde_json::from_value(v).map_err(|_|"Campos de retenção inválidos; caminhos livres não são aceitos.".into())}
fn encode<T:Serialize>(v:T)->Result<Value,String>{serde_json::to_value(v).map_err(err)}
fn empty(v:&Value)->Result<(),String>{if v.as_object().is_some_and(|o|o.is_empty()){Ok(())}else{Err("Esta ação não recebe campos.".into())}}
fn digest(p:&RetentionPlan)->String {core::hash(&json!({"id":p.id,"createdAt":p.created_at,"expiresAt":p.expires_at,"items":p.items,"totalBytes":p.total_bytes,"totalFiles":p.total_files,"confirmationPhrase":p.confirmation_phrase,"limits":p.limits}).to_string())}
fn valid_uuid(id:&str)->bool {uuid::Uuid::parse_str(id).is_ok_and(|uuid|uuid.to_string()==id)}
fn managed_manifest(value:&Value,reference:&ItemRef)->bool {
    // A schema marker cannot fall back to the legacy format, and identities
    // belonging to a different artifact category never establish ownership.
    match reference.category {
        Category::Backup=>value.get("schema").is_none()&&value["format"]==1&&value["id"]==reference.id&&value.get("reportId").is_none(),
        Category::Report=>value["reportId"]==reference.id&&value.get("id").is_none()&&match value.get("schema") {
            None=>value["format"]==1,
            Some(schema)=>schema=="openkoda.reviewed-report.v1"&&value.get("format").is_none(),
        },
        Category::Markdown|Category::Capture|Category::KaliCapture|Category::KaliCaptureMetadata=>false,
    }
}
fn idle(store:&Store)->Result<(),String>{store.work_read(|conn,s|{crate::work_supervisor::ensure_quiescent_locked(conn,s)?;if s.assessments.iter().any(|a|core::active(&a.status)){Err("Encerre ou reconcilie avaliações ativas/desconhecidas antes do expurgo.".into())}else{Ok(())}})}
fn capture_preview_locked(conn:&Connection,id:&str,identity:Identity)->Result<Artifact,String>{
    use crate::{capture::{self,CaptureState},workflow::{AttemptState,CleanupState}};
    let manifest=capture::load_locked(conn,id)?;
    if !matches!(manifest.state,CaptureState::Sealed|CaptureState::Interrupted){return Err("capture_unavailable: somente bytes de uma captura selada podem ser selecionados.".into());}
    let attempt=crate::work_store::attempt(conn,&manifest.attempt_id)?;
    if attempt.view.state.active()||attempt.view.state==AttemptState::Reserved||attempt.view.cleanup!=CleanupState::Confirmed{return Err("work_reconciliation_required: preserve a captura até confirmar o término.".into());}
    capture::preserved_locked(conn,&manifest)?;
    let mut purged=manifest.clone();purged.state=CaptureState::Purged;purged.generation=purged.generation.checked_add(1).filter(|g|*g<=crate::workflow::MAX_SAFE_INTEGER).ok_or("capture_integrity: geração esgotada.")?;
    let preview=CapturePreview{assessment_id:manifest.assessment_id,manifest_digest:manifest.digest,purged_manifest_digest:capture::manifest_digest(&purged)?,generation:manifest.generation,preserved_sha256:manifest.preserved_sha256.ok_or("capture_integrity: hash selado ausente.")?,chunk_count:manifest.chunk_count};
    Ok(Artifact{category:Category::Capture,id:id.into(),label:format!("captures/{id}"),bytes:manifest.preserved_bytes,file_count:0,files:vec![],identity,capture:Some(preview),kali_capture:None,kali_metadata_item:None,kali_metadata:None})
}

fn capture_database(path:&Path)->Result<(File,Identity),String>{
    #[cfg(windows)]{
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        plain(path,false)?;only_default_stream(path)?;
        // Keep the canonical DB identity pinned while allowing its existing
        // SQLite connection to commit. Deletion/rename sharing stays disabled.
        let file=fs::OpenOptions::new().read(true).share_mode(3).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path).map_err(err)?;
        let identity=handle_identity(&file,false)?;Ok((file,identity))
    }
    #[cfg(not(windows))]{open_node(path,false,false)}
}

impl RetentionService {
    pub fn new(root:PathBuf)->Result<Self,String>{Self::with_deleter(root,Arc::new(NativeDeleter))}
    fn with_deleter(root:PathBuf,deleter:Arc<dyn Deleter>)->Result<Self,String>{
        plain(&root,true)?;let root=root.canonicalize().map_err(err)?;
        let _parents=lock_ancestors(&root.join("retention.sqlite3"))?;
        let path=root.join("retention.sqlite3");if fs::symlink_metadata(&path).is_ok(){let _checked=open_node(&path,false,false)?;}
        let db=Connection::open(path).map_err(err)?;db.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS retention_plans(id TEXT PRIMARY KEY,data TEXT NOT NULL);").map_err(err)?;
        let service=Self{root,db:Mutex::new(db),operation:Mutex::new(()),deleter};
        for mut plan in service.plans()? {
            if plan.status=="applying" {plan.status="unknown".into();plan.error=Some("Interrupção durante o expurgo. Inspecione o registro; nenhuma exclusão foi repetida.".into());service.save(&plan)?;}
            else if plan.status=="prepared" {plan.status="revoked".into();plan.error=Some("Plano anterior revogado na abertura. Calcule nova prévia antes de excluir.".into());service.save(&plan)?;}
        }Ok(service)
    }
    pub fn has_unresolved(&self)->Result<bool,String>{Ok(self.plans()?.iter().any(|p|matches!(p.status.as_str(),"applying"|"unknown")))}
    pub fn handle(&self,action:&str,payload:Value,store:&Store)->Result<Value,String>{
        self.handle_with_spool(action,payload,store,None)
    }
    pub(crate) fn handle_with_spool(&self,action:&str,payload:Value,store:&Store,spool:Option<&dyn SpoolBackend>)->Result<Value,String>{
        if !payload.is_object()||payload.to_string().len()>16384{return Err("Solicitação de retenção excede os limites.".into());}
        let _operation=self.operation.try_lock().map_err(|_|"Outra operação de retenção está em andamento.")?;
        match action {
            "retention.inventory"=>{empty(&payload)?;self.inventory(store,spool)},
            "retention.plan"=>self.plan(decode(payload)?,store,spool),
            "retention.apply"=>self.apply(decode(payload)?,store,spool),
            "retention.reconcile"=>self.reconcile(decode(payload)?,store,spool),
            _=>Err("Ação de retenção indisponível.".into()),
        }
    }
    fn plans(&self)->Result<Vec<RetentionPlan>,String>{
        let db=self.db.lock().map_err(err)?;let mut statement=db.prepare("SELECT data FROM retention_plans ORDER BY rowid DESC").map_err(err)?;
        let values=statement.query_map([],|row|row.get::<_,String>(0)).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
        values.into_iter().map(|raw|{let plan:RetentionPlan=serde_json::from_str(&raw).map_err(err)?;validate_plan(&plan)?;Ok(plan)}).collect()
    }
    fn load(&self,id:&str)->Result<RetentionPlan,String>{
        if !valid_uuid(id){return Err("Identidade de plano inválida.".into());}
        let raw:Option<String>=self.db.lock().map_err(err)?.query_row("SELECT data FROM retention_plans WHERE id=?1",[id],|r|r.get(0)).optional().map_err(err)?;
        let plan:RetentionPlan=serde_json::from_str(&raw.ok_or("Plano de retenção não encontrado.")?).map_err(err)?;
        if plan.id!=id{return Err("Plano de retenção alterado.".into());}validate_plan(&plan)?;Ok(plan)
    }
    fn save(&self,plan:&RetentionPlan)->Result<(),String>{
        self.db.lock().map_err(err)?.execute("INSERT INTO retention_plans(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![plan.id,serde_json::to_string(plan).map_err(err)?]).map_err(err)?;Ok(())
    }
    fn references(&self)->Result<(Vec<ItemRef>,usize),String>{
        let mut refs=vec![];let mut ignored=0;
        for base_name in ["backups","exports"] {
            let base=self.root.join(base_name);if missing(&base)?{continue;}plain(&base,true)?;
            for entry in fs::read_dir(&base).map_err(err)?.take(1001){
                if refs.len()+ignored>=1000{return Err("Inventário excede 1000 entradas. Nenhuma seleção parcial foi autorizada.".into());}
                let name=entry.map_err(err)?.file_name().to_string_lossy().into_owned();
                let reference=if base_name=="backups"&&valid_uuid(&name){Some(ItemRef{category:Category::Backup,id:name})}
                else if base_name=="exports" {if let Some(id)=name.strip_prefix("report-").filter(|id|valid_uuid(id)){Some(ItemRef{category:Category::Report,id:id.into()})}
                    else {markdown_ids(&name).map(|(_,id)|ItemRef{category:Category::Markdown,id})}}
                else {None};
                if let Some(reference)=reference{refs.push(reference);}else{ignored+=1;}
            }
        }let mut unique=HashSet::new();if refs.iter().any(|r|!unique.insert(r.clone())){return Err("Identidade duplicada de artefato gerido; inventário recusado.".into());}Ok((refs,ignored))
    }
    fn location(&self,reference:&ItemRef)->Result<PathBuf,String>{
        if !valid_uuid(&reference.id){return Err("Selecione somente identidades do inventário, sem caminhos.".into());}
        match reference.category {
            Category::Backup=>Ok(self.root.join("backups").join(&reference.id)),
            Category::Report=>Ok(self.root.join("exports").join(format!("report-{}",reference.id))),
            Category::Capture|Category::KaliCapture|Category::KaliCaptureMetadata=>Err("Captura é selecionada por identidade; não tem caminho livre de arquivo.".into()),
            Category::Markdown=>{
                let base=self.root.join("exports");plain(&base,true)?;
                let mut matches=vec![];
                for entry in fs::read_dir(base).map_err(err)?.take(1001){let entry=entry.map_err(err)?;if markdown_ids(&entry.file_name().to_string_lossy()).is_some_and(|(_,id)|id==reference.id){matches.push(entry.path());}}
                if matches.len()!=1{return Err("Exportação Markdown ausente ou ambígua.".into());}Ok(matches.remove(0))
            }
        }
    }
    fn scan(&self,reference:&ItemRef,hashes:bool,deleting:bool)->Result<LockedArtifact,String>{
        let path=self.location(reference)?;let ancestors=lock_ancestors(&path)?;
        if !path.starts_with(&self.root){return Err("Artefato fora do perfil gerido.".into());}
        let is_directory=reference.category!=Category::Markdown;
        let (node,identity)=open_node(&path,is_directory,deleting)?;
        let mut files=vec![];
        if is_directory {
            for entry in fs::read_dir(&path).map_err(err)? {
                if files.len()>=MAX_FILES{return Err("Artefato excede o limite de arquivos.".into());}
                let entry=entry.map_err(err)?;let name=entry.file_name().into_string().map_err(|_|err("non-Unicode filename"))?;
                if !allowed_file(&reference.category,&name){return Err("Artefato contém arquivo não reconhecido; nada será excluído.".into());}
                let (file,file_identity)=open_node(&entry.path(),false,deleting)?;let bytes=file.metadata().map_err(err)?.len();if bytes>MAX_FILE{return Err("Arquivo excede 128 MiB.".into());}
                files.push(LockedFile{preview:FilePreview{name,bytes,sha256:String::new(),identity:file_identity},file});
            }
            // Keep the ownership manifest until every other file is removed.
            files.sort_by(|a,b|(a.preview.name=="manifest.json",&a.preview.name).cmp(&(b.preview.name=="manifest.json",&b.preview.name)));
            if let Some(manifest)=files.iter_mut().find(|f|f.preview.name=="manifest.json") {
                let value:Value=serde_json::from_slice(&read_small(&mut manifest.file,1024*1024)?).map_err(|_|"Manifest do artefato não é reconhecido.".to_string())?;
                if !managed_manifest(&value,reference){return Err("Manifest não corresponde ao formato, à categoria ou à identidade selecionada.".into());}
            }else if !files.is_empty()||!self.plans()?.iter().flat_map(|p|&p.items).any(|old|old.reference()==*reference&&old.identity==identity){
                return Err("Artefato sem manifest gerido; exclusão recusada.".into());
            }
        }else{
            let mut file=node;let bytes=file.metadata().map_err(err)?.len();if bytes>MAX_FILE{return Err("Exportação excede 128 MiB.".into());}
            let name=path.file_name().and_then(|s|s.to_str()).ok_or("Nome da exportação inválido.")?.to_owned();
            let (assessment,_)=markdown_ids(&name).ok_or("Nome da exportação inválido.")?;
            let header=String::from_utf8_lossy(&read_prefix(&mut file,2048)?).into_owned();
            if !header.starts_with("# OpenKoda — ")||!header.contains(&format!("Avaliação: {assessment}")){return Err("Arquivo não corresponde à exportação Markdown gerida.".into());}
            files.push(LockedFile{preview:FilePreview{name,bytes,sha256:String::new(),identity:identity.clone()},file});
            if hashes{for f in &mut files{f.preview.sha256=hash_handle(&mut f.file)?;}}
            let preview=Artifact{category:reference.category.clone(),id:reference.id.clone(),label:relative_label(&self.root,&path)?,bytes,file_count:1,files:files.iter().map(|f|f.preview.clone()).collect(),identity,capture:None,kali_capture:None,kali_metadata_item:None,kali_metadata:None};
            return Ok(LockedArtifact{preview,files,directory:None,_ancestors:ancestors,path});
        }
        if hashes{for file in &mut files{file.preview.sha256=hash_handle(&mut file.file)?;}}
        let bytes=files.iter().map(|f|f.preview.bytes).sum();
        let preview=Artifact{category:reference.category.clone(),id:reference.id.clone(),label:relative_label(&self.root,&path)?,bytes,file_count:files.len(),files:files.iter().map(|f|f.preview.clone()).collect(),identity,capture:None,kali_capture:None,kali_metadata_item:None,kali_metadata:None};
        Ok(LockedArtifact{preview,files,directory:Some(node),_ancestors:ancestors,path})
    }
    fn scan_managed(&self,reference:&ItemRef,hashes:bool,deleting:bool,store:&Store,spool:Option<&dyn SpoolBackend>)->Result<LockedArtifact,String>{
        if reference.category==Category::KaliCaptureMetadata{
            use crate::kali_bridge::KaliMetadataState;
            let backend=spool.ok_or("Metadados Linux indisponíveis nesta sessão.")?;
            let (item,preview)=if hashes{let preview=backend.inspect_metadata(&reference.id)?;preview.validate()?;(preview.item.clone(),Some(preview))}else{(backend.metadata_inventory()?.into_iter().find(|r|r.reference.capture_id==reference.id).ok_or("Metadados Linux ausentes do inventário.")?,None)};
            item.validate()?;if item.reference.capture_id!=reference.id||!matches!(item.state,KaliMetadataState::Uninspected|KaliMetadataState::Remaining){return Err("capture_retire_reconciliation_required".into());}
            let artifact=Artifact{category:Category::KaliCaptureMetadata,id:reference.id.clone(),label:format!("kali-capture-metadata/{}",reference.id),bytes:preview.as_ref().map_or(0,|p|p.observation.metadata_bytes),file_count:preview.as_ref().map_or(0,metadata_file_count),files:vec![],identity:Identity{volume:item.reference.environment_id.clone(),index:item.reference.owner_profile_fingerprint.clone()},capture:None,kali_capture:None,kali_metadata_item:if hashes{None}else{Some(item)},kali_metadata:preview};
            return Ok(LockedArtifact{preview:artifact,files:vec![],directory:None,_ancestors:vec![],path:PathBuf::new()});
        }
        if reference.category==Category::KaliCapture{
            let backend=spool.ok_or("Capturas Linux indisponíveis nesta sessão.")?;
            let preview=if hashes{backend.inspect(&reference.id)?}else{
                let item=backend.inventory()?.into_iter().find(|r|r.capture_id==reference.id).ok_or("Captura Linux ausente do inventário deste perfil.")?;
                SpoolPreview{reference:item,inspection_digest:String::new(),previous_receipt_id:None,previous_receipt_digest:None}
            };
            spool::validate(&preview,hashes)?;
            if preview.reference.capture_id!=reference.id{return Err("Identidade da captura Linux mudou durante a inspeção.".into());}
            let item=&preview.reference;
            let artifact=Artifact{category:Category::KaliCapture,id:reference.id.clone(),label:format!("kali-captures/{}",reference.id),bytes:item.preserved_bytes.unwrap(),file_count:0,files:vec![],identity:Identity{volume:item.environment_id.clone(),index:item.owner_profile_fingerprint.clone()},capture:None,kali_capture:Some(preview),kali_metadata_item:None,kali_metadata:None};
            return Ok(LockedArtifact{preview:artifact,files:vec![],directory:None,_ancestors:vec![],path:PathBuf::new()});
        }
        if reference.category!=Category::Capture{return self.scan(reference,hashes,deleting);}
        if !valid_uuid(&reference.id)||store.root.canonicalize().map_err(err)?!=self.root{return Err("Captura pertence a outro perfil ou tem identidade inválida.".into());}
        let path=self.root.join("openkoda.sqlite3");let mut ancestors=lock_ancestors(&path)?;
        let (guard,identity)=capture_database(&path)?;ancestors.push(guard);
        let preview=store.work_read(|conn,s|{crate::work_supervisor::ensure_quiescent_locked(conn,s)?;capture_preview_locked(conn,&reference.id,identity)})?;
        Ok(LockedArtifact{preview,files:vec![],directory:None,_ancestors:ancestors,path})
    }
    fn inventory(&self,store:&Store,spool:Option<&dyn SpoolBackend>)->Result<Value,String>{
        let (mut references,ignored)=self.references()?;let mut items=vec![];let mut rejected=vec![];
        references.extend(store.work_read(|conn,_|{let mut q=conn.prepare("SELECT id FROM capture_manifests WHERE state!='purged' ORDER BY id").map_err(err)?;let ids=q.query_map([],|r|r.get::<_,String>(0)).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;Ok(ids.into_iter().map(|id|ItemRef{category:Category::Capture,id}).collect::<Vec<_>>())})?);
        if let Some(backend)=spool{references.extend(backend.inventory()?.into_iter().filter(|item|item.availability!="purged").map(|item|ItemRef{category:Category::KaliCapture,id:item.capture_id}));references.extend(backend.metadata_inventory()?.into_iter().filter(|item|item.state!=crate::kali_bridge::KaliMetadataState::Retired).map(|item|ItemRef{category:Category::KaliCaptureMetadata,id:item.reference.capture_id}));}
        if references.len()+ignored>1000{return Err("Inventário excede 1000 entradas; nenhuma seleção parcial foi autorizada.".into());}
        for reference in references {match self.scan_managed(&reference,false,false,store,spool){Ok(item)=>items.push(item.preview),Err(error)=>rejected.push(json!({"category":reference.category,"id":reference.id,"reason":error}))}}
        let journals=self.plans()?;let unresolved=journals.iter().any(|p|matches!(p.status.as_str(),"applying"|"unknown"));
        Ok(json!({"items":items,"journals":journals,"hasUnresolved":unresolved,"blocked":unresolved||idle(store).is_err(),"limits":LIMITS,"rejected":rejected,"ignoredEntries":ignored,
            "projectPurgeSupported":false,"automaticPurge":false,"retentionDays":store.snapshot()?.settings.retention_days,"hashesVerified":false}))
    }
    fn plan(&self,input:Selection,store:&Store,spool:Option<&dyn SpoolBackend>)->Result<Value,String>{
        idle(store)?;if self.has_unresolved()?{return Err("Reconcilie o expurgo interrompido antes de criar outro plano.".into());}
        if input.items.is_empty()||input.items.len()>20{return Err("Selecione explicitamente entre 1 e 20 artefatos.".into());}
        let mut unique=HashSet::new();if input.items.iter().any(|r|!unique.insert(r.clone())){return Err("Seleção contém identidades repetidas.".into());}
        validate_metadata_selection(&input.items)?;
        metadata_capacity(&input.items,1,spool)?;
        let locked=input.items.iter().map(|r|self.scan_managed(r,true,false,store,spool)).collect::<Result<Vec<_>,_>>()?;
        let total_bytes=locked.iter().map(|i|i.preview.bytes).sum::<u64>();let total_files=locked.iter().map(|i|i.preview.file_count).sum::<usize>();
        if total_bytes>MAX_BATCH||total_files>MAX_FILES{return Err("Plano excede 1 GiB ou 256 arquivos; revise um lote menor.".into());}
        let mut plan=RetentionPlan{id:model::id(),digest:String::new(),status:"prepared".into(),created_at:model::now(),expires_at:(chrono::Utc::now()+chrono::Duration::minutes(15)).to_rfc3339(),
            items:locked.into_iter().map(|i|i.preview).collect(),total_bytes,total_files,confirmation_phrase:PHRASE.into(),limits:LIMITS.iter().map(|s|s.to_string()).collect(),progress:vec![],error:None,reconciled:false};
        plan.digest=digest(&plan);self.save(&plan)?;encode(plan)
    }
    fn apply(&self,input:Apply,store:&Store,spool:Option<&dyn SpoolBackend>)->Result<Value,String>{
        idle(store)?;let mut plan=self.load(&input.id)?;
        if input.confirmation!=PHRASE||input.digest!=plan.digest{return Err("Confirmação e digest devem corresponder exatamente à prévia.".into());}
        if plan.status!="prepared"||chrono::DateTime::parse_from_rfc3339(&plan.expires_at).map_err(err)?<=chrono::Utc::now(){return Err("Plano expirado ou já consumido; não será repetido.".into());}
        if self.has_unresolved()?{return Err("Reconcilie o expurgo interrompido antes de excluir.".into());}
        #[cfg(not(windows))] {return Err("Expurgo protegido por handle está disponível somente no Windows.".into());}
        #[cfg(windows)] {
            metadata_capacity(&plan.items.iter().map(Artifact::reference).collect::<Vec<_>>(),3,spool)?;
            let preflight=(||{let mut locked=vec![];for expected in &plan.items{let item=self.scan_managed(&expected.reference(),true,true,store,spool)?;if item.preview!=*expected{return Err("Os bytes ou suas identidades mudaram após a prévia. Nenhum item deste lote foi excluído.".into());}locked.push(item);}Ok::<_,String>(locked)})();
            let locked=match preflight {Ok(value)=>value,Err(error)=>{plan.status="rejected".into();plan.error=Some(error.clone());self.save(&plan)?;return Err(error);}};
            plan.status="applying".into();self.save(&plan)?;
            for mut item in locked {
                if let Some(metadata)=&item.preview.kali_metadata{
                    let receipt_id=spool::metadata_receipt_id(&plan.id,&item.preview.id);
                    plan.progress.push(Progress{category:Category::KaliCaptureMetadata,id:item.preview.id.clone(),name:"linux_capture_metadata".into(),status:"deleting".into(),sha256:metadata.observation.manifest_digest.clone()});self.save(&plan)?;
                    let result=spool.ok_or_else(||"Metadados Linux indisponíveis.".to_string()).and_then(|backend|backend.retire_metadata(metadata,&receipt_id));
                    match result{Ok(spool::MetadataOutcome::Retired)=>{plan.progress.last_mut().unwrap().status="deleted".into();self.save(&plan)?;continue;},result=>{plan.status="unknown".into();plan.error=Some(match result{Err(error)=>error,_=>"Consolidação não comprovada. Inspecione o resultado antes de preparar outro plano.".into()});self.save(&plan)?;return encode(plan);}}
                }
                if let Some(capture)=&item.preview.kali_capture{
                    let receipt_id=spool::receipt_id(&plan.id,&item.preview.id);
                    plan.progress.push(Progress{category:Category::KaliCapture,id:item.preview.id.clone(),name:"linux_spool".into(),status:"deleting".into(),sha256:capture.reference.manifest_digest.clone().unwrap()});self.save(&plan)?;
                    let result=spool.ok_or_else(||"Capturas Linux indisponíveis.".to_string()).and_then(|backend|backend.purge(capture,&receipt_id));
                    match result{
                        Ok(SpoolOutcome::Purged)=>{plan.progress.last_mut().unwrap().status="deleted".into();self.save(&plan)?;continue;},
                        result=>{plan.status="unknown".into();plan.error=Some(match result{Err(error)=>error,_=>"A remoção no Linux não foi comprovada. Inspecione o resultado; nenhuma exclusão será repetida.".into()});self.save(&plan)?;return encode(plan);}
                    }
                }
                if let Some(capture)=&item.preview.capture{
                    plan.progress.push(Progress{category:Category::Capture,id:item.preview.id.clone(),name:"capture_bytes".into(),status:"deleting".into(),sha256:capture.preserved_sha256.clone()});self.save(&plan)?;
                    let removed=store.work_transaction(|conn,s|{
                        crate::work_supervisor::ensure_quiescent_locked(conn,s)?;
                        if capture_preview_locked(conn,&item.preview.id,item.preview.identity.clone())?!=item.preview{return Err("Captura mudou depois da revisão; bytes preservados.".into());}
                        let purged=crate::capture::purge_locked(conn,&item.preview.id,&capture.manifest_digest)?;
                        if purged.digest!=capture.purged_manifest_digest{return Err("Manifesto de retenção divergente; transação revertida.".into());}Ok(())
                    });
                    if let Err(error)=removed{return self.failed(plan,error);}
                    plan.progress.last_mut().unwrap().status="deleted".into();self.save(&plan)?;continue;
                }
                for file in item.files.drain(..) {
                    let progress=Progress{category:item.preview.category.clone(),id:item.preview.id.clone(),name:file.preview.name.clone(),status:"deleting".into(),sha256:file.preview.sha256.clone()};
                    plan.progress.push(progress);self.save(&plan)?;
                    if let Err(error)=self.deleter.delete(file.file){return self.failed(plan,error);}
                    plan.progress.last_mut().unwrap().status="deleted".into();self.save(&plan)?;
                }
                if let Some(directory)=item.directory.take(){
                    if fs::read_dir(&item.path).map_err(err)?.next().is_some(){return self.failed(plan,"O diretório recebeu um arquivo novo; esse arquivo foi preservado.".into());}
                    plan.progress.push(Progress{category:item.preview.category.clone(),id:item.preview.id.clone(),name:"(directory)".into(),status:"deleting".into(),sha256:String::new()});self.save(&plan)?;
                    if let Err(error)=self.deleter.delete(directory){return self.failed(plan,error);}
                    plan.progress.last_mut().unwrap().status="deleted".into();self.save(&plan)?;
                }
            }
            plan.status="applied".into();self.save(&plan)?;encode(plan)
        }
    }
    fn failed(&self,mut plan:RetentionPlan,error:String)->Result<Value,String>{
        plan.status=if plan.progress.iter().any(|p|p.status=="deleted"){"partial"}else{"failed"}.into();plan.error=Some(error);self.save(&plan)?;encode(plan)
    }
    fn reconcile(&self,input:IdInput,store:&Store,spool:Option<&dyn SpoolBackend>)->Result<Value,String>{
        let mut plan=self.load(&input.id)?;if plan.status!="unknown"{return Err("Somente um expurgo interrompido exige reconciliação.".into());}
        let mut progress=vec![];let mut present=false;let mut unknown=false;
        for item in &plan.items {
            if let Some(metadata)=&item.kali_metadata{
                let attempted=plan.progress.iter().any(|p|p.category==Category::KaliCaptureMetadata&&p.id==item.id&&p.name=="linux_capture_metadata");
                let outcome=if attempted{spool.ok_or_else(||"Metadados Linux indisponíveis.".to_string()).and_then(|backend|backend.reconcile_metadata(metadata,&spool::metadata_receipt_id(&plan.id,&item.id)))}else{Ok(spool::MetadataOutcome::Remaining)};
                let state=match outcome{Ok(spool::MetadataOutcome::Retired)=>"absent",Ok(spool::MetadataOutcome::Remaining)=>{present=true;"present"},_=>{unknown=true;"unknown"}};
                progress.push(Progress{category:Category::KaliCaptureMetadata,id:item.id.clone(),name:"linux_capture_metadata".into(),status:state.into(),sha256:metadata.observation.manifest_digest.clone()});continue;
            }
            if let Some(capture)=&item.kali_capture{
                let attempted=plan.progress.iter().any(|p|p.category==Category::KaliCapture&&p.id==item.id&&p.name=="linux_spool");
                let outcome=if attempted{spool.ok_or_else(||"Capturas Linux indisponíveis.".to_string()).and_then(|backend|backend.reconcile(capture,&spool::receipt_id(&plan.id,&item.id)))}else{Ok(SpoolOutcome::Present)};
                let state=match outcome{Ok(SpoolOutcome::Purged)=>"absent",Ok(SpoolOutcome::Present)=>{present=true;"present"},_=>{unknown=true;"unknown"}};
                progress.push(Progress{category:Category::KaliCapture,id:item.id.clone(),name:"linux_spool".into(),status:state.into(),sha256:capture.reference.manifest_digest.clone().unwrap()});continue;
            }
            if let Some(capture)=&item.capture{
                let state=if store.root.canonicalize().map_err(err)?!=self.root||observe_identity(&self.root.join("openkoda.sqlite3"),&item.identity)?!="present"{"replaced"}else{store.work_read(|conn,_|{
                    let exists=conn.query_row("SELECT 1 FROM capture_manifests WHERE id=?1",[&item.id],|r|r.get::<_,i32>(0)).optional().map_err(err)?.is_some();
                    if !exists{return Ok("absent");}let current=crate::capture::load_locked(conn,&item.id)?;
                    Ok(if current.state==crate::capture::CaptureState::Purged&&current.digest==capture.purged_manifest_digest{"absent"}else if current.digest==capture.manifest_digest{"present"}else{"replaced"})
                })?};
                present|=state=="present";progress.push(Progress{category:Category::Capture,id:item.id.clone(),name:"capture_bytes".into(),status:state.into(),sha256:capture.preserved_sha256.clone()});continue;
            }
            // Resolve a host-generated relative label from the signed plan, not
            // from the caller. Reject every component before accessing it.
            let path=self.root.join(&item.label);safe_relative(&item.label)?;
            for file in &item.files {
                let candidate=if item.category==Category::Markdown{path.clone()}else{path.join(&file.name)};
                let state=observe_identity(&candidate,&file.identity)?;present|=state=="present";
                progress.push(Progress{category:item.category.clone(),id:item.id.clone(),name:file.name.clone(),status:state.into(),sha256:file.sha256.clone()});
            }
            if item.category!=Category::Markdown{let state=observe_identity(&path,&item.identity)?;present|=state=="present";progress.push(Progress{category:item.category.clone(),id:item.id.clone(),name:"(directory)".into(),status:state.into(),sha256:String::new()});}
        }
        plan.progress=progress;plan.reconciled=!unknown;plan.status=if unknown{"unknown"}else if present{"partial"}else{"applied"}.into();
        plan.error=Some(if unknown{"Ainda não foi possível comprovar o resultado da captura no Linux. As identidades e a intenção original foram preservadas; nenhuma exclusão foi repetida."}else{"Inspeção confirmou o estado atual das identidades originais. Substituições foram preservadas; nenhuma exclusão foi repetida. Restantes exigem nova seleção e plano."}.into());self.save(&plan)?;encode(plan)
    }
}

fn validate_metadata_selection(items:&[ItemRef])->Result<(),String>{
    if items.iter().any(|r|r.category==Category::KaliCaptureMetadata&&items.iter().any(|other|other.id==r.id&&other.category==Category::KaliCapture)){return Err("Selecione bruto e metadados da mesma captura em planos separados, após confirmar a purga.".into());}Ok(())
}
fn metadata_capacity(items:&[ItemRef],factor:usize,spool:Option<&dyn SpoolBackend>)->Result<(),String>{
    if items.iter().any(|r|r.category==Category::KaliCaptureMetadata){let count=items.iter().filter(|r|matches!(r.category,Category::KaliCaptureMetadata|Category::KaliCapture)).count();spool.ok_or("Metadados Linux indisponíveis.")?.metadata_capacity(count*factor)?;}Ok(())
}
fn metadata_file_count(preview:&crate::kali_bridge::KaliRetirePreview)->usize {
    preview.observation.entries.len() + usize::from(preview.observation.pending_producer_write.is_some())
}
fn allowed_file(category:&Category,name:&str)->bool{
    match category {
        Category::Backup=>name=="manifest.json"||crate::recovery::DATABASES.contains(&name),
        Category::Report=>["manifest.json","checksums.json","report.md"].contains(&name)||name.strip_prefix("evidence-").and_then(|s|s.strip_suffix(".txt")).is_some_and(valid_uuid),
        Category::Markdown|Category::Capture|Category::KaliCapture|Category::KaliCaptureMetadata=>false,
    }
}
fn validate_plan(plan:&RetentionPlan)->Result<(),String>{
    if !valid_uuid(&plan.id)||plan.digest!=digest(plan)||plan.confirmation_phrase!=PHRASE||plan.items.is_empty()||plan.items.len()>20
        ||!["prepared","applying","applied","rejected","failed","partial","unknown","revoked"].contains(&plan.status.as_str()){
        return Err("Registro de retenção inválido ou alterado; revisão necessária.".into());
    }
    validate_metadata_selection(&plan.items.iter().map(Artifact::reference).collect::<Vec<_>>())?;
    let mut ids=HashSet::new();let mut bytes=0u64;let mut files=0usize;
    for item in &plan.items {
        if !valid_uuid(&item.id)||!ids.insert(item.reference()){return Err("Identidade do artefato no plano é inválida.".into());}
        safe_relative(&item.label)?;
        let expected=match item.category{Category::Backup=>format!("backups/{}",item.id),Category::Report=>format!("exports/report-{}",item.id),Category::Capture=>format!("captures/{}",item.id),Category::KaliCapture=>format!("kali-captures/{}",item.id),Category::KaliCaptureMetadata=>format!("kali-capture-metadata/{}",item.id),Category::Markdown=>{
            let name=item.label.strip_prefix("exports/").ok_or("Exportação fora da área gerida.")?;
            if !markdown_ids(name).is_some_and(|(_,id)|id==item.id)||item.files.len()!=1||item.files[0].name!=name{return Err("Exportação divergente do plano.".into());}item.label.clone()
        }};
        if item.label!=expected{return Err("Destino do plano foi alterado.".into());}
        if item.kali_metadata_item.is_some(){return Err("Inventário sem inspeção não autoriza um plano.".into());}
        if item.category==Category::KaliCaptureMetadata{
            let metadata=item.kali_metadata.as_ref().ok_or("Metadados Linux sem inspeção revisada.")?;metadata.validate()?;
            if metadata.item.reference.capture_id!=item.id||item.bytes!=metadata.observation.metadata_bytes||item.identity.volume!=metadata.item.reference.environment_id||item.identity.index!=metadata.item.reference.owner_profile_fingerprint||item.capture.is_some()||item.kali_capture.is_some()||item.file_count!=metadata_file_count(metadata)||!item.files.is_empty(){return Err("Compromisso dos metadados Linux divergente.".into());}
            bytes+=item.bytes;files+=item.file_count;continue;
        }
        if item.file_count!=item.files.len(){return Err("Contagem de arquivos do artefato divergente.".into());}
        if item.kali_metadata.is_some(){return Err("Artefato contém autoridade de metadados inesperada.".into());}
        if item.category==Category::KaliCapture{
            let capture=item.kali_capture.as_ref().ok_or("Captura Linux sem inspeção revisada.")?;spool::validate(capture,true)?;
            if capture.reference.capture_id!=item.id||item.bytes!=capture.reference.preserved_bytes.unwrap()||item.identity.volume!=capture.reference.environment_id||item.identity.index!=capture.reference.owner_profile_fingerprint||item.capture.is_some()||item.file_count!=0||!item.files.is_empty(){return Err("Compromisso da captura Linux divergente.".into());}
            bytes+=item.bytes;continue;
        }
        if item.kali_capture.is_some(){return Err("Artefato local contém captura Linux inesperada.".into());}
        if item.category==Category::Capture{
            let capture=item.capture.as_ref().ok_or("Captura sem compromisso de manifesto.")?;
            if !valid_uuid(&capture.assessment_id)||![&capture.manifest_digest,&capture.purged_manifest_digest,&capture.preserved_sha256].iter().all(|hash|crate::workflow::valid_hash(hash))||capture.generation==0||capture.generation>=crate::workflow::MAX_SAFE_INTEGER||capture.chunk_count>128||item.bytes>crate::capture::MAX_CAPTURE_BYTES||item.file_count!=0||!item.files.is_empty(){return Err("Compromisso de captura inválido; nenhuma retenção autorizada.".into());}
            bytes+=item.bytes;continue;
        }
        if item.capture.is_some(){return Err("Artefato de arquivo contém autoridade de captura inesperada.".into());}
        let mut names=HashSet::new();let mut item_bytes=0u64;
        for file in &item.files {if !names.insert(&file.name)||file.bytes>MAX_FILE||file.sha256.len()!=64||!file.sha256.bytes().all(|b|b.is_ascii_hexdigit())
            ||(item.category!=Category::Markdown&&!allowed_file(&item.category,&file.name)){return Err("Arquivo do plano inválido.".into());}item_bytes=item_bytes.checked_add(file.bytes).ok_or("Tamanho inválido.")?;}
        if item.bytes!=item_bytes{return Err("Tamanho do artefato mudou no plano.".into());}bytes+=item_bytes;files+=item.file_count;
    }
    if bytes!=plan.total_bytes||files!=plan.total_files||bytes>MAX_BATCH||files>MAX_FILES{return Err("Totais do plano inválidos.".into());}Ok(())
}
fn markdown_ids(name:&str)->Option<(String,String)>{let value=name.strip_prefix("avaliacao-")?.strip_suffix(".md")?;if value.len()!=73{return None;}let (assessment,id)=(value.get(..36)?,value.get(37..)?);if value.get(36..37)?!="-"||!valid_uuid(assessment)||!valid_uuid(id){return None;}Some((assessment.into(),id.into()))}
fn relative_label(root:&Path,path:&Path)->Result<String,String>{let label=path.strip_prefix(root).map_err(err)?.to_string_lossy().replace('\\',"/");safe_relative(&label)?;Ok(label)}
fn safe_relative(value:&str)->Result<(),String>{if value.is_empty()||value.contains([':', '\\'])||value.split('/').any(|s|s.is_empty()||s=="."||s==".."||s.ends_with(['.',' '])||s.chars().any(|c|c.is_control()||"<>\"|?*".contains(c))){return Err("Caminho gerido inválido.".into());}Ok(())}
fn missing(path:&Path)->Result<bool,String>{match fs::symlink_metadata(path){Ok(_)=>Ok(false),Err(error) if error.kind()==std::io::ErrorKind::NotFound=>Ok(true),Err(error)=>Err(err(error))}}
fn plain(path:&Path,directory:bool)->Result<(),String>{let meta=fs::symlink_metadata(path).map_err(err)?;let mut link=meta.file_type().is_symlink();#[cfg(windows)]{use std::os::windows::fs::MetadataExt;link|=meta.file_attributes()&0x400!=0;}if link||meta.is_dir()!=directory||(!directory&&!meta.is_file()){return Err("Retenção recusa links, reparse points e tipos inesperados.".into());}Ok(())}
fn read_prefix(file:&mut File,max:u64)->Result<Vec<u8>,String>{file.seek(SeekFrom::Start(0)).map_err(err)?;let mut bytes=vec![];file.take(max).read_to_end(&mut bytes).map_err(err)?;Ok(bytes)}
fn read_small(file:&mut File,max:u64)->Result<Vec<u8>,String>{let bytes=read_prefix(file,max+1)?;if bytes.len() as u64>max{return Err("Manifest excede o limite.".into());}Ok(bytes)}
fn hash_handle(file:&mut File)->Result<String,String>{file.seek(SeekFrom::Start(0)).map_err(err)?;let mut hash=Sha256::new();let mut bytes=0u64;let mut buffer=[0u8;65536];loop{let n=file.read(&mut buffer).map_err(err)?;if n==0{break;}bytes+=n as u64;if bytes>MAX_FILE{return Err("Arquivo excedeu o limite durante a leitura.".into());}hash.update(&buffer[..n]);}Ok(format!("{:x}",hash.finalize()))}

#[cfg(windows)]
fn handle_identity(file:&File,directory:bool)->Result<Identity,String>{
    use std::os::windows::io::AsRawHandle;use windows_sys::Win32::Storage::FileSystem::*;
    let mut info=unsafe{std::mem::zeroed()};if unsafe{GetFileInformationByHandle(file.as_raw_handle(),&mut info)}==0{return Err(err(std::io::Error::last_os_error()));}
    if info.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT!=0||(info.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY!=0)!=directory||(!directory&&info.nNumberOfLinks!=1){return Err("Retenção recusa reparse points, tipos inesperados e arquivos com hardlinks.".into());}
    Ok(Identity{volume:info.dwVolumeSerialNumber.to_string(),index:(((info.nFileIndexHigh as u64)<<32)|info.nFileIndexLow as u64).to_string()})
}
#[cfg(windows)]
fn open_node(path:&Path,directory:bool,deleting:bool)->Result<(File,Identity),String>{
    use std::os::windows::fs::OpenOptionsExt;use windows_sys::Win32::Storage::FileSystem::*;
    plain(path,directory)?;
    let access=0x80000000|if deleting{0x00010000}else{0};
    let file=fs::OpenOptions::new().access_mode(access).share_mode(if directory{3}else if deleting{0}else{1})
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT|if directory{FILE_FLAG_BACKUP_SEMANTICS}else{0}).open(path).map_err(|_|"Artefato em uso ou sem acesso exclusivo; exclusão recusada.".to_string())?;
    let identity=handle_identity(&file,directory)?;if !directory{only_default_stream(path)?;}Ok((file,identity))
}
#[cfg(not(windows))]
fn open_node(path:&Path,directory:bool,_:bool)->Result<(File,Identity),String>{
    use std::os::unix::fs::MetadataExt;plain(path,directory)?;let file=File::open(path).map_err(err)?;let meta=file.metadata().map_err(err)?;
    if !directory&&meta.nlink()!=1{return Err("Retenção recusa arquivos com hardlinks.".into());}Ok((file,Identity{volume:meta.dev().to_string(),index:meta.ino().to_string()}))
}
fn lock_ancestors(path:&Path)->Result<Vec<File>,String>{
    let mut current=PathBuf::new();let mut handles=vec![];
    #[cfg(windows)] {use std::path::Prefix;let mut parts=path.components();if !matches!(parts.next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_))){return Err("Retenção aceita somente disco local, sem UNC/device.".into());}}
    for component in path.parent().ok_or("Artefato sem diretório pai.")?.components(){current.push(component.as_os_str());if matches!(component,Component::Prefix(_)){continue;}handles.push(open_node(&current,true,false)?.0);}Ok(handles)
}
fn observe_identity(path:&Path,expected:&Identity)->Result<&'static str,String>{
    // Inspection must not traverse a substituted ancestor (including a junction).
    let mut current=PathBuf::new();for component in path.components(){current.push(component.as_os_str());if matches!(component,Component::Prefix(_)){continue;}if missing(&current)?{return Ok("absent");}let meta=fs::symlink_metadata(&current).map_err(err)?;if plain(&current,meta.is_dir()).is_err(){return Ok("replaced");}}
    let _ancestors=lock_ancestors(path)?;let meta=fs::symlink_metadata(path).map_err(err)?;
    Ok(if observed_identity(path,meta.is_dir())?.as_ref()==Some(expected){"present"}else{"replaced"})
}
#[cfg(windows)]
fn observed_identity(path:&Path,directory:bool)->Result<Option<Identity>,String>{
    // Reconciliation only inspects identity. A new hardlink/stream must not
    // keep the journal blocked or make us delete the replacement object.
    use std::os::windows::{fs::OpenOptionsExt,io::AsRawHandle};use windows_sys::Win32::Storage::FileSystem::*;
    let file=fs::OpenOptions::new().read(true).share_mode(3).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT|if directory{FILE_FLAG_BACKUP_SEMANTICS}else{0}).open(path).map_err(err)?;
    let mut info=unsafe{std::mem::zeroed()};if unsafe{GetFileInformationByHandle(file.as_raw_handle(),&mut info)}==0{return Err(err(std::io::Error::last_os_error()));}
    if info.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT!=0||(info.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY!=0)!=directory{return Ok(None);}
    Ok(Some(Identity{volume:info.dwVolumeSerialNumber.to_string(),index:(((info.nFileIndexHigh as u64)<<32)|info.nFileIndexLow as u64).to_string()}))
}
#[cfg(not(windows))]
fn observed_identity(path:&Path,directory:bool)->Result<Option<Identity>,String>{use std::os::unix::fs::MetadataExt;let metadata=fs::symlink_metadata(path).map_err(err)?;if metadata.file_type().is_symlink()||metadata.is_dir()!=directory{return Ok(None);}Ok(Some(Identity{volume:metadata.dev().to_string(),index:metadata.ino().to_string()}))}
#[cfg(windows)]
fn delete_handle(file:File)->Result<(),String>{
    use std::os::windows::io::AsRawHandle;use windows_sys::Win32::Storage::FileSystem::*;
    let directory=file.metadata().map_err(err)?.is_dir();handle_identity(&file,directory)?;
    let disposition=FILE_DISPOSITION_INFO{DeleteFile:true};
    if unsafe{SetFileInformationByHandle(file.as_raw_handle(),FileDispositionInfo,&disposition as *const _ as *const _,std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32)}==0{return Err("O Windows recusou a remoção do objeto verificado. Revise o resultado parcial; nenhum caminho substituto foi apagado.".into());}
    drop(file);Ok(())
}
#[cfg(not(windows))]
fn delete_handle(_:File)->Result<(),String>{Err("Expurgo protegido disponível somente no Windows.".into())}

#[cfg(windows)]
fn only_default_stream(path:&Path)->Result<(),String>{
    use std::os::windows::ffi::OsStrExt;use windows_sys::Win32::{Foundation::{GetLastError,INVALID_HANDLE_VALUE,ERROR_HANDLE_EOF},Storage::FileSystem::*};
    let name=path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();let mut data:WIN32_FIND_STREAM_DATA=unsafe{std::mem::zeroed()};
    let search=unsafe{FindFirstStreamW(name.as_ptr(),0,&mut data as *mut _ as *mut _,0)};
    if search==INVALID_HANDLE_VALUE {return if unsafe{GetLastError()}==ERROR_HANDLE_EOF{Ok(())}else{Err("Não foi possível verificar streams do arquivo; expurgo recusado.".into())};}
    let result=(||{loop{let end=data.cStreamName.iter().position(|&c|c==0).unwrap_or(data.cStreamName.len());if String::from_utf16_lossy(&data.cStreamName[..end])!="::$DATA"{return Err("Arquivo possui stream alternativo não incluído na prévia; expurgo recusado.".into());}
        if unsafe{FindNextStreamW(search,&mut data as *mut _ as *mut _)}==0 {return if unsafe{GetLastError()}==ERROR_HANDLE_EOF{Ok(())}else{Err("Inspeção de streams incompleta.".into())};}
    }})();unsafe{FindClose(search)};result
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {_dir:tempfile::TempDir,root:PathBuf,store:Store,service:RetentionService}
    impl Fixture {
        fn new()->Self{let dir=tempfile::tempdir().unwrap();let root=dir.path().to_path_buf();let store=Store::open(root.clone()).unwrap();let service=RetentionService::new(root.clone()).unwrap();Self{_dir:dir,root,store,service}}
        fn backup(&self)->ItemRef{let id=model::id();let path=self.root.join("backups").join(&id);fs::create_dir_all(&path).unwrap();fs::write(path.join("openkoda.sqlite3"),b"synthetic backup bytes").unwrap();fs::write(path.join("manifest.json"),json!({"format":1,"id":id}).to_string()).unwrap();ItemRef{category:Category::Backup,id}}
        fn markdown(&self)->ItemRef{let id=model::id();let assessment=model::id();fs::create_dir_all(self.root.join("exports")).unwrap();fs::write(self.root.join("exports").join(format!("avaliacao-{assessment}-{id}.md")),format!("# OpenKoda — fixture\n\nAvaliação: {assessment}\n")).unwrap();ItemRef{category:Category::Markdown,id}}
        fn reviewed_report(&self)->ItemRef{
            let project=self.store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Retention report fixture","kind":"web","target":"https://retention.invalid","description":""})}).unwrap();
            let assessment=self.store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Synthetic report","kind":"web","target":project["target"],"objective":"Retention fixture only","scope":crate::scope::test_creation(&self.store,project["id"].as_str().unwrap(),&[])})}).unwrap();
            let content="synthetic preserved evidence";
            self.store.mutate(|s|{s.assessments[0].evidence.push(model::Evidence{id:model::id(),name:"Synthetic evidence".into(),content:content.into(),sha256:core::hash(content),created_at:model::now()});Ok(())}).unwrap();
            let recovery=crate::recovery::RecoveryService::new(self.root.clone()).unwrap();
            let report=crate::recovery::test_support::report(&recovery,&self.store,assessment["id"].as_str().unwrap(),true).unwrap();
            ItemRef{category:Category::Report,id:report["reportId"].as_str().unwrap().into()}
        }
        fn plan(&self,items:Vec<ItemRef>)->Value{self.service.handle("retention.plan",json!({"items":items}),&self.store).unwrap()}
        fn apply(&self,plan:&Value)->Result<Value,String>{self.service.handle("retention.apply",json!({"id":plan["id"],"digest":plan["digest"],"confirmation":PHRASE}),&self.store)}
    }
    struct FakeSpool {preview:Mutex<SpoolPreview>,inspections:Mutex<usize>,purges:Mutex<Vec<String>>,reconciles:Mutex<Vec<String>>,unknown:Mutex<bool>,remaining:Mutex<bool>}
    impl FakeSpool{
        fn new()->Self{Self{preview:Mutex::new(SpoolPreview{reference:SpoolItem{capture_id:model::id(),assessment_id:model::id(),run_id:model::id(),environment_id:model::id(),owner_profile_fingerprint:core::hash("owned synthetic profile"),manifest_digest:Some(core::hash("linux spool manifest")),preserved_bytes:Some(17),availability:"owned".into()},inspection_digest:core::hash("inspected exact spool"),previous_receipt_id:None,previous_receipt_digest:None}),inspections:Mutex::new(0),purges:Mutex::new(vec![]),reconciles:Mutex::new(vec![]),unknown:Mutex::new(false),remaining:Mutex::new(false)}}
        fn item(&self)->ItemRef{ItemRef{category:Category::KaliCapture,id:self.preview.lock().unwrap().reference.capture_id.clone()}}
    }
    impl SpoolBackend for FakeSpool{
        fn inventory(&self)->Result<Vec<SpoolItem>,String>{Ok(vec![self.preview.lock().unwrap().reference.clone()])}
        fn inspect(&self,id:&str)->Result<SpoolPreview,String>{*self.inspections.lock().unwrap()+=1;let p=self.preview.lock().unwrap().clone();assert_eq!(p.reference.capture_id,id);Ok(p)}
        fn purge(&self,p:&SpoolPreview,id:&str)->Result<SpoolOutcome,String>{assert_eq!(*p,*self.preview.lock().unwrap());self.purges.lock().unwrap().push(id.into());Ok(if *self.unknown.lock().unwrap(){SpoolOutcome::Unknown}else{SpoolOutcome::Purged})}
        fn reconcile(&self,p:&SpoolPreview,id:&str)->Result<SpoolOutcome,String>{assert_eq!(*p,*self.preview.lock().unwrap());self.reconciles.lock().unwrap().push(id.into());Ok(if *self.unknown.lock().unwrap(){SpoolOutcome::Unknown}else if *self.remaining.lock().unwrap(){SpoolOutcome::Present}else{SpoolOutcome::Purged})}
    }
    #[test]fn linux_spool_inventory_does_not_start_inspection_and_rejects_foreign_profile(){
        let f=Fixture::new();let backend=FakeSpool::new();
        let inventory=f.service.handle_with_spool("retention.inventory",json!({}),&f.store,Some(&backend)).unwrap();
        assert_eq!(inventory["items"][0]["category"],"kali_capture");assert_eq!(*backend.inspections.lock().unwrap(),0);
        backend.preview.lock().unwrap().reference.availability="foreign_profile".into();
        let inventory=f.service.handle_with_spool("retention.inventory",json!({}),&f.store,Some(&backend)).unwrap();
        assert_eq!(inventory["items"],json!([]));assert_eq!(inventory["rejected"].as_array().unwrap().len(),1);
        assert!(f.service.handle_with_spool("retention.plan",json!({"items":[backend.item()]}),&f.store,Some(&backend)).is_err());
        assert!(backend.purges.lock().unwrap().is_empty());
    }
    #[cfg(windows)]
    #[test]fn linux_spool_changed_preflight_preserves_entire_mixed_batch(){
        let f=Fixture::new();let backend=FakeSpool::new();let local=f.backup();let path=f.service.location(&local).unwrap();
        let plan=f.service.handle_with_spool("retention.plan",json!({"items":[local,backend.item()]}),&f.store,Some(&backend)).unwrap();
        backend.preview.lock().unwrap().inspection_digest=core::hash("changed Linux identity");
        assert!(f.service.handle_with_spool("retention.apply",json!({"id":plan["id"],"digest":plan["digest"],"confirmation":PHRASE}),&f.store,Some(&backend)).is_err());
        assert!(path.join("openkoda.sqlite3").exists());assert!(backend.purges.lock().unwrap().is_empty());
    }
    #[cfg(windows)]
    #[test]fn linux_spool_unknown_survives_restart_and_reconcile_never_repeats_delete(){
        let f=Fixture::new();let backend=FakeSpool::new();*backend.unknown.lock().unwrap()=true;let local=f.backup();let path=f.service.location(&local).unwrap();
        let plan=f.service.handle_with_spool("retention.plan",json!({"items":[backend.item(),local]}),&f.store,Some(&backend)).unwrap();
        let apply=json!({"id":plan["id"],"digest":plan["digest"],"confirmation":PHRASE});
        let result=f.service.handle_with_spool("retention.apply",apply.clone(),&f.store,Some(&backend)).unwrap();assert_eq!(result["status"],"unknown");assert!(path.exists());
        assert!(f.service.handle_with_spool("retention.apply",apply.clone(),&f.store,Some(&backend)).is_err());
        let root=f.root.clone();drop(f.service);let service=RetentionService::new(root).unwrap();assert!(service.has_unresolved().unwrap());
        let unknown=service.handle_with_spool("retention.reconcile",json!({"id":plan["id"]}),&f.store,Some(&backend)).unwrap();assert_eq!(unknown["status"],"unknown");
        *backend.unknown.lock().unwrap()=false;
        let observed=service.handle_with_spool("retention.reconcile",json!({"id":plan["id"]}),&f.store,Some(&backend)).unwrap();assert_eq!(observed["status"],"partial");assert!(path.exists());
        assert_eq!(backend.purges.lock().unwrap().len(),1);let receipts=backend.reconciles.lock().unwrap();assert_eq!(receipts.len(),2);assert!(receipts.iter().all(|id|id==&backend.purges.lock().unwrap()[0]));
        assert!(service.handle_with_spool("retention.apply",apply,&f.store,Some(&backend)).is_err());
    }
    #[cfg(windows)]
    #[test]fn linux_spool_success_has_one_intent_per_item_and_no_replay(){
        let f=Fixture::new();let backend=FakeSpool::new();let item=backend.item();
        let plan=f.service.handle_with_spool("retention.plan",json!({"items":[item]}),&f.store,Some(&backend)).unwrap();assert_eq!(plan["totalBytes"],17);assert_eq!(plan["totalFiles"],0);
        let apply=json!({"id":plan["id"],"digest":plan["digest"],"confirmation":PHRASE});
        assert_eq!(f.service.handle_with_spool("retention.apply",apply.clone(),&f.store,Some(&backend)).unwrap()["status"],"applied");
        assert!(f.service.handle_with_spool("retention.apply",apply,&f.store,Some(&backend)).is_err());
        let receipts=backend.purges.lock().unwrap();assert_eq!(receipts.len(),1);assert_eq!(receipts[0],spool::receipt_id(plan["id"].as_str().unwrap(),&backend.item().id));
        assert!(backend.reconciles.lock().unwrap().is_empty());assert!(f.root.join("openkoda.sqlite3").exists());
    }
    #[cfg(windows)]
    #[test]fn linux_spool_confirmed_partial_requires_a_new_review_and_new_receipt(){
        let f=Fixture::new();let backend=FakeSpool::new();let item=backend.item();
        let first=f.service.handle_with_spool("retention.plan",json!({"items":[item.clone()]}),&f.store,Some(&backend)).unwrap();
        let approval=json!({"id":first["id"],"digest":first["digest"],"confirmation":PHRASE});
        *backend.unknown.lock().unwrap()=true;
        assert_eq!(f.service.handle_with_spool("retention.apply",approval.clone(),&f.store,Some(&backend)).unwrap()["status"],"unknown");
        assert!(f.service.handle_with_spool("retention.plan",json!({"items":[item.clone()]}),&f.store,Some(&backend)).is_err());
        *backend.unknown.lock().unwrap()=false;*backend.remaining.lock().unwrap()=true;
        let reconciled=f.service.handle_with_spool("retention.reconcile",json!({"id":first["id"]}),&f.store,Some(&backend)).unwrap();
        assert_eq!(reconciled["status"],"partial");assert_eq!(backend.purges.lock().unwrap().len(),1);
        assert!(f.service.handle_with_spool("retention.apply",approval,&f.store,Some(&backend)).is_err());
        let predecessor=backend.purges.lock().unwrap()[0].clone();
        {let mut next=backend.preview.lock().unwrap();next.inspection_digest=core::hash("new inspection of confirmed remaining bytes");next.previous_receipt_id=Some(predecessor.clone());next.previous_receipt_digest=Some(core::hash("confirmed predecessor tombstone"));}
        let second=f.service.handle_with_spool("retention.plan",json!({"items":[item]}),&f.store,Some(&backend)).unwrap();
        assert_eq!(second["items"][0]["kaliCapture"]["previousReceiptId"],predecessor);
        assert_ne!(second["id"],first["id"]);assert_ne!(second["digest"],first["digest"]);assert_eq!(backend.purges.lock().unwrap().len(),1);
        assert_eq!(f.service.handle_with_spool("retention.apply",json!({"id":second["id"],"digest":second["digest"],"confirmation":PHRASE}),&f.store,Some(&backend)).unwrap()["status"],"applied");
        let receipts=backend.purges.lock().unwrap();assert_eq!(receipts.len(),2);assert_ne!(receipts[0],receipts[1]);assert_eq!(backend.reconciles.lock().unwrap().as_slice(),&[receipts[0].clone()]);
    }
    #[test]fn linux_spool_predecessor_requires_both_exact_identifier_and_digest(){
        let backend=FakeSpool::new();let original=backend.preview.lock().unwrap().clone();
        assert!(spool::validate(&original,true).is_ok());
        let mut incomplete=original.clone();incomplete.previous_receipt_id=Some(model::id());assert!(spool::validate(&incomplete,true).is_err());
        incomplete.previous_receipt_digest=Some("wrong digest".into());assert!(spool::validate(&incomplete,true).is_err());
        incomplete.previous_receipt_digest=Some(core::hash("confirmed partial tombstone"));assert!(spool::validate(&incomplete,true).is_ok());
        incomplete.previous_receipt_id=Some("../unreviewed".into());assert!(spool::validate(&incomplete,true).is_err());
        let wire=serde_json::to_value(&original).unwrap();assert!(wire.get("previousReceiptId").is_none());assert!(wire.get("previousReceiptDigest").is_none());
    }
    #[test]fn metadata_capacity_accounts_for_all_linux_controls_in_a_mixed_batch(){
        struct Capacity(std::cell::Cell<usize>);
        impl SpoolBackend for Capacity{
            fn inventory(&self)->Result<Vec<SpoolItem>,String>{Ok(vec![])}
            fn inspect(&self,_:&str)->Result<SpoolPreview,String>{panic!("capacity check cannot inspect")}
            fn purge(&self,_:&SpoolPreview,_:&str)->Result<SpoolOutcome,String>{panic!("capacity check cannot mutate")}
            fn reconcile(&self,_:&SpoolPreview,_:&str)->Result<SpoolOutcome,String>{panic!("capacity check cannot reconcile")}
            fn metadata_capacity(&self,n:usize)->Result<(),String>{self.0.set(n);Err("work_journal_quota".into())}
        }
        let backend=Capacity(std::cell::Cell::new(0));let items=vec![ItemRef{category:Category::KaliCaptureMetadata,id:model::id()},ItemRef{category:Category::KaliCapture,id:model::id()},ItemRef{category:Category::Backup,id:model::id()}];
        assert_eq!(metadata_capacity(&items,3,Some(&backend)).unwrap_err(),"work_journal_quota");assert_eq!(backend.0.get(),6);assert!(metadata_capacity(&items,1,Some(&backend)).is_err());assert_eq!(backend.0.get(),2);
        let plan=model::id();let capture=model::id();assert_ne!(spool::receipt_id(&plan,&capture),spool::metadata_receipt_id(&plan,&capture));assert!(valid_uuid(&spool::metadata_receipt_id(&plan,&capture)));
    }
    #[cfg(windows)]
    #[test]
    fn metadata_files_count_toward_the_mixed_batch_limit() {
        let (_directory,store,bridge,assessment,environment)=crate::kali_bridge::completed_scan_fixture();
        let identity=||Ok(environment.clone());
        let backend=KaliRetention{bridge:&bridge,store:&store,identity:&identity};
        let retention=RetentionService::new(store.root.clone()).unwrap();
        let capture=bridge.spool_metadata(&store,Some(&assessment)).unwrap().remove(0);
        let purge=bridge.inspect_capture_purge(&store,&capture,&identity).unwrap();
        bridge.execute_capture_purge(&store,&purge,&model::id(),&purge.digest,&identity).unwrap();

        let report=model::id();
        let directory=store.root.join("exports").join(format!("report-{report}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("manifest.json"), json!({"format":1,"reportId":report}).to_string()).unwrap();
        for _ in 0..253 {
            fs::write(directory.join(format!("evidence-{}.txt",model::id())), b"owned fixture").unwrap();
        }
        let selection=json!({"items":[{"category":"report","id":report},{"category":"kali_capture_metadata","id":capture.capture_id}]});
        let plan=retention.handle_with_spool("retention.plan",selection.clone(),&store,Some(&backend)).unwrap();
        assert_eq!(plan["items"][0]["fileCount"],254);
        assert_eq!(plan["items"][1]["fileCount"],2);
        assert_eq!(plan["items"][1]["files"],json!([]));
        assert_eq!(plan["totalFiles"],256);
        let mut stored=retention.load(plan["id"].as_str().unwrap()).unwrap();

        // A rewritten total cannot conceal one of the reviewed Linux files.
        stored.items[1].file_count-=1;
        stored.total_files-=1;
        stored.digest=digest(&stored);
        assert!(validate_plan(&stored).is_err());

        // A recognized producer temporary is outside the capture directory
        // and therefore counts in addition to its two observed entries.
        let mut preview=stored.items[1].kali_metadata.clone().unwrap();
        preview.observation.pending_producer_write=Some(serde_json::from_value(json!({
            "identity":{"device":"1","inode":"5"},"bytes":8,"sha256":core::hash("fragment"),
            "attemptId":model::id(),"complete":false
        })).unwrap());
        preview.observation.metadata_bytes+=8;
        preview.observation.validate().unwrap();
        assert_eq!(metadata_file_count(&preview),3);

        let extra=directory.join(format!("evidence-{}.txt",model::id()));
        fs::write(&extra,b"still present").unwrap();
        let error=retention.handle_with_spool("retention.plan",selection,&store,Some(&backend)).unwrap_err();
        assert!(error.contains("256 arquivos"),"{error}");
        assert_eq!(fs::read(extra).unwrap().as_slice(),b"still present");
        assert_eq!(retention.plans().unwrap().len(),1);
    }
    #[test]fn inventory_only_offers_managed_artifacts_and_never_selects_by_age(){
        let f=Fixture::new();f.backup();f.markdown();fs::write(f.root.join("exports/private.txt"),b"outside selection").unwrap();
        let inventory=f.service.handle("retention.inventory",json!({}),&f.store).unwrap();assert_eq!(inventory["items"].as_array().unwrap().len(),2);assert_eq!(inventory["ignoredEntries"],1);assert_eq!(inventory["automaticPurge"],false);assert_eq!(inventory["projectPurgeSupported"],false);assert_eq!(inventory["journals"],json!([]));assert!(f.root.join("exports/private.txt").exists());
    }
    #[cfg(windows)]
    #[test]fn capture_retention_reviews_hashes_removes_cursors_and_preserves_historical_manifest(){
        let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"bounded private capture bytes",crate::capture::CaptureState::Sealed);
        let root=store.root.clone();fs::write(root.join("ai-budget.sqlite3"),b"global budget unchanged").unwrap();
        store.capture_transaction(|conn|{conn.execute("INSERT INTO capture_cursors VALUES(?1,?2,?3,?4,?5,0,?6)",params![model::id(),aid,manifest.id,manifest.digest,core::hash("projection"),model::now()]).map_err(err)?;Ok(())}).unwrap();
        let service=RetentionService::new(root.clone()).unwrap();
        let plan=service.handle("retention.plan",json!({"items":[{"category":"capture","id":manifest.id}]}),&store).unwrap();
        assert_eq!(plan["items"][0]["capture"]["manifestDigest"],manifest.digest);assert_eq!(plan["totalBytes"],manifest.preserved_bytes);assert_eq!(plan["totalFiles"],0);
        let request=json!({"id":plan["id"],"digest":plan["digest"],"confirmation":PHRASE});
        assert_eq!(service.handle("retention.apply",request.clone(),&store).unwrap()["status"],"applied");
        assert!(service.handle("retention.apply",request,&store).is_err());
        store.work_read(|conn,s|{
            let current=crate::capture::load_locked(conn,&manifest.id)?;assert_eq!(current.state,crate::capture::CaptureState::Purged);
            assert_eq!(current.preserved_sha256,manifest.preserved_sha256);assert_eq!(current.preserved_bytes,manifest.preserved_bytes);assert_eq!(current.origin_refs,manifest.origin_refs);assert_eq!(current.effect,manifest.effect);
            for table in ["capture_chunks","capture_cursors"]{assert_eq!(conn.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE capture_id=?1"),[&manifest.id],|r|r.get::<_,i64>(0)).unwrap(),0);}
            assert!(s.assessments.iter().any(|a|a.id==aid));assert!(crate::capture::preserved_locked(conn,&current).is_err());Ok(())
        }).unwrap();
        assert_eq!(fs::read(root.join("ai-budget.sqlite3")).unwrap(),b"global budget unchanged");
    }
    #[cfg(windows)]
    #[test]fn capture_generation_change_rejects_retention_before_deletion(){
        let(_dir,store,_aid,manifest)=crate::capture::fixture::build(b"preserve on stale capture review",crate::capture::CaptureState::Sealed);
        let service=RetentionService::new(store.root.clone()).unwrap();let plan=service.handle("retention.plan",json!({"items":[{"category":"capture","id":manifest.id}]}),&store).unwrap();
        let mut changed=manifest.clone();changed.generation+=1;changed.digest=crate::capture::manifest_digest(&changed).unwrap();
        store.capture_transaction(|conn|{conn.execute("UPDATE capture_manifests SET generation=?1,data=?2 WHERE id=?3",params![changed.generation,serde_json::to_string(&changed).unwrap(),changed.id]).map_err(err)?;Ok(())}).unwrap();
        assert!(service.handle("retention.apply",json!({"id":plan["id"],"digest":plan["digest"],"confirmation":PHRASE}),&store).is_err());
        store.work_read(|conn,_|{assert_eq!(crate::capture::preserved_locked(conn,&changed)?,b"preserve on stale capture review");Ok(())}).unwrap();
    }
    #[cfg(windows)]
    #[test]fn interrupted_capture_commit_is_inspected_without_replaying_or_losing_the_seal(){
        let(_dir,store,_aid,manifest)=crate::capture::fixture::build(b"retain historical seal",crate::capture::CaptureState::Sealed);
        let service=RetentionService::new(store.root.clone()).unwrap();let plan=service.handle("retention.plan",json!({"items":[{"category":"capture","id":manifest.id}]}),&store).unwrap();
        let mut journal=service.load(plan["id"].as_str().unwrap()).unwrap();journal.status="applying".into();service.save(&journal).unwrap();
        store.work_transaction(|conn,_|crate::capture::purge_locked(conn,&manifest.id,&manifest.digest)).unwrap();drop(service);
        let service=RetentionService::new(store.root.clone()).unwrap();assert!(service.has_unresolved().unwrap());
        let inspected=service.handle("retention.reconcile",json!({"id":plan["id"]}),&store).unwrap();assert_eq!(inspected["status"],"applied");assert_eq!(inspected["progress"][0]["status"],"absent");
        let purged=store.work_read(|conn,_|crate::capture::load_locked(conn,&manifest.id)).unwrap();assert_eq!(purged.generation,manifest.generation+1);assert_eq!(purged.preserved_sha256,manifest.preserved_sha256);
    }
    #[test]fn open_or_unknown_capture_is_not_a_retention_candidate(){
        let(_dir,store,_aid,manifest)=crate::capture::fixture::build(b"live stream must stay",crate::capture::CaptureState::Open);
        let service=RetentionService::new(store.root.clone()).unwrap();assert!(service.handle("retention.plan",json!({"items":[{"category":"capture","id":manifest.id}]}),&store).is_err());
        store.work_read(|conn,_|{assert_eq!(crate::capture::preserved_locked(conn,&manifest)?,b"live stream must stay");Ok(())}).unwrap();
    }
    #[test]fn plan_has_exact_byte_hashes_without_deleting_anything(){
        let f=Fixture::new();let item=f.backup();let plan=f.plan(vec![item.clone()]);assert_eq!(plan["status"],"prepared");assert_eq!(plan["totalFiles"],2);assert!(plan["totalBytes"].as_u64().unwrap()>0);let value:RetentionPlan=serde_json::from_value(plan).unwrap();validate_plan(&value).unwrap();assert_eq!(value.items[0].files[0].sha256,core::hash("synthetic backup bytes"));assert!(f.service.location(&item).unwrap().exists());
    }
    #[test]fn reviewed_report_is_inventoried_and_retained_by_exact_file_hashes(){
        let f=Fixture::new();let item=f.reviewed_report();let path=f.service.location(&item).unwrap();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
        let manifest:Value=serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();assert_eq!(manifest["schema"],"openkoda.reviewed-report.v1");assert!(manifest.get("format").is_none());
        let inventory=f.service.handle("retention.inventory",json!({}),&f.store).unwrap();assert_eq!(inventory["rejected"],json!([]));assert!(inventory["items"].as_array().unwrap().iter().any(|entry|entry["category"]=="report"&&entry["id"]==item.id));
        let plan=f.plan(vec![item]);assert_eq!(plan["totalFiles"],4);
        for file in plan["items"][0]["files"].as_array().unwrap(){let bytes=fs::read(path.join(file["name"].as_str().unwrap())).unwrap();assert_eq!(file["bytes"],bytes.len());assert_eq!(file["sha256"],format!("{:x}",Sha256::digest(bytes)));}
        #[cfg(windows)] {assert_eq!(f.apply(&plan).unwrap()["status"],"applied");assert!(!path.exists());assert!(f.apply(&plan).is_err());}
        assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert!(f.root.join("openkoda.sqlite3").exists());
    }
    #[test]fn manifest_recognition_is_closed_by_category_schema_and_identity(){
        let f=Fixture::new();let id=model::id();let other=model::id();
        let cases=[
            (Category::Backup,json!({"format":1,"id":id}),true),
            (Category::Report,json!({"format":1,"reportId":id}),true),
            (Category::Report,json!({"schema":"openkoda.reviewed-report.v1","reportId":id}),true),
            (Category::Backup,json!({"format":1,"id":other}),false),
            (Category::Report,json!({"format":1,"reportId":other}),false),
            (Category::Report,json!({"schema":"openkoda.reviewed-report.v1","reportId":other}),false),
            (Category::Report,json!({"schema":"openkoda.reviewed-report.v2","reportId":id}),false),
            (Category::Report,json!({"schema":"unrecognized","format":1,"reportId":id}),false),
            (Category::Backup,json!({"schema":"unrecognized","format":1,"id":id}),false),
            (Category::Backup,json!({"schema":"openkoda.reviewed-report.v1","id":id,"reportId":id}),false),
            (Category::Report,json!({"schema":"openkoda.reviewed-report.v1","format":1,"reportId":id}),false),
            (Category::Report,json!({"format":1,"id":id,"reportId":id}),false),
            (Category::Backup,json!({"format":1,"id":id,"reportId":id}),false),
            (Category::Report,json!({"format":2,"reportId":id}),false),
            (Category::Backup,json!({"format":"1","id":id}),false),
            (Category::Report,json!({"reportId":id}),false),
            (Category::Report,json!({"format":1,"schema":null,"reportId":id}),false),
        ];
        for (category,manifest,accepted) in cases{
            let item=ItemRef{category,id:id.clone()};let path=f.service.location(&item).unwrap();fs::create_dir_all(&path).unwrap();fs::write(path.join("manifest.json"),manifest.to_string()).unwrap();
            let result=f.service.scan(&item,false,false);assert_eq!(result.is_ok(),accepted,"category {:?}, manifest {manifest}",item.category);drop(result);
            if !accepted {assert!(f.service.handle("retention.plan",json!({"items":[item]}),&f.store).is_err());assert!(path.join("manifest.json").exists());}
        }
    }
    #[test]fn free_paths_duplicate_ids_unknown_fields_and_categories_are_rejected(){
        let f=Fixture::new();let item=f.backup();for payload in [json!({"items":[{"category":"backup","id":"../outside"}]}),json!({"items":[item.clone(),item.clone()]}),json!({"items":[{"category":"project","id":model::id()}]}),json!({"items":[item],"path":"C:/private"})]{assert!(f.service.handle("retention.plan",payload,&f.store).is_err());}
    }
    #[test]fn hardlinked_files_and_unrecognized_children_are_preserved(){
        let f=Fixture::new();let item=f.backup();let path=f.service.location(&item).unwrap();fs::hard_link(path.join("openkoda.sqlite3"),f.root.join("outside-link")).unwrap();assert!(f.service.handle("retention.plan",json!({"items":[item]}),&f.store).is_err());assert!(f.root.join("outside-link").exists());
        let second=f.backup();let path=f.service.location(&second).unwrap();fs::write(path.join("unmanaged.txt"),b"preserve").unwrap();assert!(f.service.handle("retention.plan",json!({"items":[second]}),&f.store).is_err());assert!(path.join("unmanaged.txt").exists());
    }
    #[test]fn wrong_confirmation_or_digest_does_not_consume_plan(){
        let f=Fixture::new();let plan=f.plan(vec![f.backup()]);for (confirmation,digest) in [("yes",plan["digest"].as_str().unwrap()),(PHRASE,"wrong")]{assert!(f.service.handle("retention.apply",json!({"id":plan["id"],"digest":digest,"confirmation":confirmation}),&f.store).is_err());}assert_eq!(f.service.load(plan["id"].as_str().unwrap()).unwrap().status,"prepared");
    }
    #[test]fn tampered_plan_is_refused_before_any_file_access(){
        let f=Fixture::new();let item=f.backup();let plan=f.plan(vec![item.clone()]);let mut tampered=plan.clone();tampered["items"][0]["label"]=json!("../outside");
        f.service.db.lock().unwrap().execute("UPDATE retention_plans SET data=?1 WHERE id=?2",params![tampered.to_string(),plan["id"].as_str().unwrap()]).unwrap();assert!(f.apply(&plan).is_err());assert!(f.service.location(&item).unwrap().exists());
    }
    #[test]fn active_or_unknown_assessment_blocks_planning(){
        let f=Fixture::new();let item=f.backup();let project=f.store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"fixture","kind":"web","target":"https://example.test/","description":""})}).unwrap();
        f.store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"fixture","kind":"web","target":project["target"],"objective":"headers","scope":crate::scope::test_creation(&f.store,project["id"].as_str().unwrap(),&["http.headers.v1"])})}).unwrap();
        f.store.mutate(|s|{s.assessments[0].status="unknown".into();Ok(())}).unwrap();assert!(f.service.handle("retention.plan",json!({"items":[item]}),&f.store).is_err());
    }
    #[test]fn startup_revokes_previews_and_preserves_interrupted_intent_without_replay(){
        let f=Fixture::new();let item=f.backup();let prepared=f.plan(vec![item.clone()]);let interrupted=f.plan(vec![item.clone()]);let mut plan=f.service.load(interrupted["id"].as_str().unwrap()).unwrap();plan.status="applying".into();f.service.save(&plan).unwrap();
        let path=f.service.location(&item).unwrap();drop(f.service);let reopened=RetentionService::new(f.root.clone()).unwrap();assert_eq!(reopened.load(prepared["id"].as_str().unwrap()).unwrap().status,"revoked");assert!(reopened.has_unresolved().unwrap());assert!(path.exists());assert!(reopened.handle("retention.plan",json!({"items":[item]}),&f.store).is_err());
    }
    #[test]fn reconcile_inspects_and_preserves_replacements_without_deleting(){
        let f=Fixture::new();let item=f.backup();let value=f.plan(vec![item.clone()]);let path=f.service.location(&item).unwrap();let mut plan=f.service.load(value["id"].as_str().unwrap()).unwrap();plan.status="unknown".into();f.service.save(&plan).unwrap();
        fs::rename(path.join("openkoda.sqlite3"),f.root.join("original-preserved")).unwrap();fs::write(path.join("openkoda.sqlite3"),b"replacement").unwrap();
        let result=f.service.handle("retention.reconcile",json!({"id":value["id"]}),&f.store).unwrap();assert_eq!(result["status"],"partial");assert!(result["progress"].as_array().unwrap().iter().any(|p|p["status"]=="replaced"));assert_eq!(fs::read(path.join("openkoda.sqlite3")).unwrap(),b"replacement");assert!(!f.service.has_unresolved().unwrap());
    }
    #[test]fn hardlinked_replacement_can_be_reconciled_without_purging_it(){
        let f=Fixture::new();let item=f.backup();let value=f.plan(vec![item.clone()]);let path=f.service.location(&item).unwrap();let mut plan=f.service.load(value["id"].as_str().unwrap()).unwrap();plan.status="unknown".into();f.service.save(&plan).unwrap();
        fs::rename(path.join("openkoda.sqlite3"),f.root.join("original-preserved")).unwrap();fs::write(f.root.join("replacement"),b"replacement").unwrap();fs::hard_link(f.root.join("replacement"),path.join("openkoda.sqlite3")).unwrap();
        let result=f.service.handle("retention.reconcile",json!({"id":value["id"]}),&f.store).unwrap();assert!(result["progress"].as_array().unwrap().iter().any(|p|p["status"]=="replaced"));assert!(!f.service.has_unresolved().unwrap());assert_eq!(fs::read(f.root.join("replacement")).unwrap(),b"replacement");
    }
    #[cfg(windows)]
    #[test]fn all_items_are_revalidated_before_first_delete(){
        let f=Fixture::new();let first=f.backup();let second=f.backup();let plan=f.plan(vec![first.clone(),second.clone()]);fs::write(f.service.location(&second).unwrap().join("openkoda.sqlite3"),b"changed").unwrap();
        assert!(f.apply(&plan).is_err());assert!(f.service.location(&first).unwrap().join("manifest.json").exists());assert_eq!(f.service.load(plan["id"].as_str().unwrap()).unwrap().status,"rejected");
    }
    #[cfg(windows)]
    #[test]fn same_bytes_replacement_invalidates_preview_by_identity(){
        let f=Fixture::new();let item=f.backup();let plan=f.plan(vec![item.clone()]);let path=f.service.location(&item).unwrap().join("openkoda.sqlite3");fs::rename(&path,f.root.join("old-bytes")).unwrap();fs::write(&path,b"synthetic backup bytes").unwrap();assert!(f.apply(&plan).is_err());assert!(path.exists());
    }
    #[cfg(windows)]
    #[test]fn confirmed_handle_purge_deletes_only_reviewed_artifacts_once(){
        let f=Fixture::new();let backup=f.backup();let markdown=f.markdown();let keep=f.backup();let path=f.service.location(&backup).unwrap();let mdpath=f.service.location(&markdown).unwrap();let plan=f.plan(vec![backup,markdown]);let result=f.apply(&plan).unwrap();assert_eq!(result["status"],"applied");assert!(!path.exists());assert!(!mdpath.exists());assert!(f.service.location(&keep).unwrap().exists());assert!(f.apply(&plan).is_err());assert!(f.root.join("openkoda.sqlite3").exists());
    }
    #[cfg(windows)]
    #[test]fn partial_failure_preserves_journal_and_manifest_and_allows_new_review(){
        use std::sync::atomic::{AtomicUsize,Ordering};struct FailAfterOne(AtomicUsize);impl Deleter for FailAfterOne{fn delete(&self,file:File)->Result<(),String>{if self.0.fetch_add(1,Ordering::SeqCst)==1{Err("injected failure".into())}else{delete_handle(file)}}}
        let mut f=Fixture::new();f.service.deleter=Arc::new(FailAfterOne(AtomicUsize::new(0)));let item=f.backup();let plan=f.plan(vec![item.clone()]);let result=f.apply(&plan).unwrap();assert_eq!(result["status"],"partial");let path=f.service.location(&item).unwrap();assert!(!path.join("openkoda.sqlite3").exists());assert!(path.join("manifest.json").exists());assert!(f.apply(&plan).is_err());let next=f.plan(vec![item]);assert_eq!(next["totalFiles"],1);
    }
    #[cfg(windows)]
    #[test]fn alternate_data_stream_is_not_silently_removed(){let f=Fixture::new();let item=f.backup();let path=f.service.location(&item).unwrap().join("openkoda.sqlite3");fs::write(format!("{}:unreviewed",path.display()),b"private stream").unwrap();assert!(f.service.handle("retention.plan",json!({"items":[item]}),&f.store).is_err());assert_eq!(fs::read(format!("{}:unreviewed",path.display())).unwrap(),b"private stream");}
    #[cfg(windows)]
    #[test]fn complete_backup_including_purge_history_remains_selectable(){
        let f=Fixture::new();let item=f.backup();let path=f.service.location(&item).unwrap();
        for name in crate::recovery::DATABASES {if name!="openkoda.sqlite3"{fs::write(path.join(name),b"synthetic auxiliary database").unwrap();}}
        let inventory=f.service.handle("retention.inventory",json!({}),&f.store).unwrap();
        assert!(inventory["items"].as_array().unwrap().iter().any(|entry|entry["id"]==item.id));
        let plan=f.plan(vec![item]);assert_eq!(plan["totalFiles"],9);
        assert!(plan["items"][0]["files"].as_array().unwrap().iter().any(|file|file["name"]=="project-purge.sqlite3"));
        assert_eq!(f.apply(&plan).unwrap()["status"],"applied");assert!(!path.exists());
        assert!(f.root.join("openkoda.sqlite3").exists());
    }
    #[cfg(windows)]
    #[test]fn directory_junction_cannot_redirect_a_managed_id_outside_profile(){
        use std::os::windows::process::CommandExt;
        let f=Fixture::new();let outside=tempfile::tempdir().unwrap();let id=model::id();fs::create_dir(f.root.join("backups")).unwrap();let link=f.root.join("backups").join(&id);
        fs::write(outside.path().join("manifest.json"),json!({"format":1,"id":id}).to_string()).unwrap();fs::write(outside.path().join("openkoda.sqlite3"),b"outside").unwrap();
        let result=std::process::Command::new("cmd.exe").args(["/C","mklink","/J"]).arg(&link).arg(outside.path()).creation_flags(0x08000000).output().unwrap();assert!(result.status.success(),"temporary junction fixture failed");
        assert!(f.service.handle("retention.plan",json!({"items":[{"category":"backup","id":id}]}),&f.store).is_err());fs::remove_dir(&link).unwrap();assert_eq!(fs::read(outside.path().join("openkoda.sqlite3")).unwrap(),b"outside");
    }
}
