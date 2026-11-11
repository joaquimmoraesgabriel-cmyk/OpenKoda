//! Verified local backups, non-destructive recovery staging and report bundles.
//! No export sends data to an external service, and no restore overwrites live data.
use crate::{ai::redact, core::{self, Store}, model::{self, Snapshot}};
use rusqlite::{backup::{Backup, StepResult}, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::{Read, Write}, path::{Path, PathBuf}, sync::Mutex, time::{Duration, Instant}};
#[path="recovery_data.rs"]
mod data;
#[path="recovery_review.rs"]
mod review;

const MAX_FILE: u64 = 128 * 1024 * 1024;
const MAX_MANIFEST: u64 = 1024 * 1024;
pub(crate) const DATABASES: [&str; 8] = ["openkoda.sqlite3", "environment.sqlite3", "environment-history.sqlite3", "assistant.sqlite3", "kali-bridge.sqlite3", "remediation.sqlite3", "retention.sqlite3", "project-purge.sqlite3"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct BackupFile { name: String, bytes: u64, sha256: String, user_version: i64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct BackupManifest {
    format: u32, id: String, created_at: String, app_version: String,
    consistency: String, files: Vec<BackupFile>, exclusions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdInput { id: String }

#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct ReportInput { id: String, #[serde(default)] include_evidence: bool }

pub struct RecoveryService { root: PathBuf, operation: Mutex<()>, reviews:Mutex<std::collections::HashMap<String,review::OutputReview>> }

pub fn supports(action: &str) -> bool {
    matches!(action, "backup.create" | "backup.prepare" | "backup.commit" | "backup.list" | "backup.verify" | "backup.restore" | "report.bundle" | "report.prepare" | "report.commit")
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "Campos inválidos ou não permitidos na recuperação/exportação.".into())
}
fn empty(value: &Value) -> Result<(), String> {
    if value.as_object().is_some_and(|v| v.is_empty()) { Ok(()) } else { Err("Esta ação não recebe campos.".into()) }
}

impl RecoveryService {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self { root:root.canonicalize().map_err(|e|e.to_string())?, operation:Mutex::new(()), reviews:Mutex::new(std::collections::HashMap::new()) })
    }

    pub fn handle(&self, action: &str, payload: Value, store: &Store) -> Result<Value, String> {
        if !payload.is_object() || payload.to_string().len() > 16384 { return Err("Solicitação de recuperação inválida.".into()); }
        let _lock=self.operation.try_lock().map_err(|_|"Uma exportação ou recuperação já está em andamento.")?;
        match action {
            "backup.create" | "report.bundle" => Err("data_review_required: prepare e confirme o pacote antes de exportar ou criar backup.".into()),
            "backup.list" => { empty(&payload)?; self.list_backups() }
            "backup.verify" => { let input:IdInput=decode(payload)?; let (manifest,path)=self.verify(&input.id)?; Ok(json!({"verified":true,"manifest":manifest,"path":path})) }
            "backup.restore" => { let input:IdInput=decode(payload)?; self.restore(&input.id) }
            _ => Err("Ação de recuperação não disponível.".into()),
        }
    }

    fn directory(&self, name: &str) -> Result<PathBuf, String> {
        let path=self.root.join(name);
        if !path.exists() { fs::create_dir(&path).map_err(|e|e.to_string())?; }
        require_plain(&path, true)?;
        let resolved=path.canonicalize().map_err(|e|e.to_string())?;
        if !resolved.starts_with(&self.root) || resolved.parent()!=Some(self.root.as_path()) { return Err("Destino local fora da área gerida.".into()); }
        Ok(resolved)
    }

    fn create_backup(&self, store: &Store) -> Result<Value, String> {
        let state=store.snapshot()?;
        if state.assessments.iter().any(|a|core::active(&a.status)) { return Err("Reconcilie ou encerre avaliações ativas antes de criar o backup.".into()); }
        if self.root.join("environment.sqlite3").exists()&&self.root.join("environment-history.sqlite3").exists(){return Err("A pasta contém ambiente ativo e histórico com destinos de recuperação conflitantes. Preserve ambos e revise a origem antes de criar o backup.".into());}
        let base=self.directory("backups")?;
        let id=model::id();
        let staging=base.join(format!("{id}.incomplete"));
        fs::create_dir(&staging).map_err(|e|e.to_string())?;
        let mut files=vec![];
        let mut exclusions=vec!["Credenciais e confirmação da conexão com o provedor".into(),
            "Orçamento, marcador de identidade e registros monetários mínimos da instalação: ficam na base estável e não são restaurados por cópias de perfil".into(),
            "Sistema de arquivos e processos da distribuição Kali; backup do aplicativo não é backup WSL".into(),
            "Fontes originais dos projetos, arquivos externos e cópias de relatórios exportadas".into()];
        for name in DATABASES {
            let source=self.root.join(name);
            if !source.exists() { continue; }
            require_plain(&source,false)?;
            let target=staging.join(name);
            if name=="assistant.sqlite3" {
                if !backup_assistant(&source,&target)? {
                    exclusions.push("Histórico da IA: schema desconhecido, omitido para não copiar tabelas de segredos inesperadas".into());
                    continue;
                }
            } else { backup_database(&source,&target)?; }
            let user_version=verify_database(&target)?;
            files.push(BackupFile { name:name.into(), bytes:fs::metadata(&target).map_err(|e|e.to_string())?.len(), sha256:hash_file(&target)?, user_version });
        }
        if !files.iter().any(|file|file.name=="openkoda.sqlite3") { return Err("Banco principal não encontrado; backup não finalizado.".into()); }
        validate_core_database(&staging.join("openkoda.sqlite3"))?;
        let manifest=BackupManifest { format:1,id:id.clone(),created_at:model::now(),app_version:env!("CARGO_PKG_VERSION").into(),
            consistency:"Snapshot consistente de cada banco SQLite via Online Backup API. Não é snapshot distribuído do Windows/WSL.".into(),files,exclusions };
        write_new(&staging.join("manifest.json"),&serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?)?;
        let destination=base.join(&id);
        fs::rename(&staging,&destination).map_err(|e|e.to_string())?;
        let (verified,_) = self.verify(&id)?;
        Ok(json!({"id":id,"path":destination,"verified":true,"manifest":verified}))
    }

    fn list_backups(&self) -> Result<Value, String> {
        let base=self.directory("backups")?;
        let mut items=vec![];
        for entry in fs::read_dir(base).map_err(|e|e.to_string())?.take(1000) {
            let entry=entry.map_err(|e|e.to_string())?;
            let name=entry.file_name().to_string_lossy().to_string();
            if uuid::Uuid::parse_str(&name).is_err() { continue; }
            if require_plain(&entry.path(),true).is_err() { continue; }
            // Listing does not silently spend time verifying every large backup.
            if let Ok(manifest)=read_manifest(&entry.path().join("manifest.json")) {
                items.push(json!({"id":name,"createdAt":manifest.created_at,"bytes":manifest.files.iter().map(|f|f.bytes).sum::<u64>(),"verification":"requires-check"}));
            }
        }
        items.sort_by(|a,b|b["createdAt"].as_str().cmp(&a["createdAt"].as_str()));
        Ok(json!({"backups":items}))
    }

    fn verify(&self, id: &str) -> Result<(BackupManifest,PathBuf), String> {
        core::valid_id(id)?;
        let folder=self.directory("backups")?.join(id);
        require_plain(&folder,true)?;
        let manifest=read_manifest(&folder.join("manifest.json"))?;
        if manifest.format!=1 || manifest.id!=id || manifest.files.is_empty() || manifest.files.len()>DATABASES.len() { return Err("Manifest de backup inválido ou incompatível.".into()); }
        let mut names=HashSet::new();
        for item in &manifest.files {
            if !DATABASES.contains(&item.name.as_str()) || !names.insert(item.name.clone()) || item.bytes>MAX_FILE { return Err("Arquivo de backup inválido ou fora dos limites.".into()); }
            let file=folder.join(&item.name);
            require_plain(&file,false)?;
            if fs::metadata(&file).map_err(|e|e.to_string())?.len()!=item.bytes || hash_file(&file)?!=item.sha256 { return Err(format!("Integridade do backup falhou: {}. O arquivo original foi preservado.",item.name)); }
            if verify_database(&file)?!=item.user_version { return Err("Versão do banco difere do manifest.".into()); }
            if item.name=="assistant.sqlite3" {
                let db=Connection::open_with_flags(&file,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
                let secrets:i64=db.query_row("SELECT count(*) FROM vault",[],|row|row.get(0)).map_err(|e|e.to_string())?;
                let confirmations:i64=db.query_row("SELECT count(*) FROM connection_check",[],|row|row.get(0)).map_err(|e|e.to_string())?;
                if secrets!=0 || confirmations!=0 { return Err("Backup contém credenciais/confirmações que deveriam ser excluídas.".into()); }
            }
        }
        if !names.contains("openkoda.sqlite3") { return Err("Backup não contém o banco principal.".into()); }
        if names.contains("environment.sqlite3")&&names.contains("environment-history.sqlite3"){return Err("Backup contém dois registros de ambiente que colidiriam no destino histórico. Nada será sobrescrito.".into());}
        validate_core_database(&folder.join("openkoda.sqlite3"))?;
        Ok((manifest,folder))
    }

    fn restore(&self, id: &str) -> Result<Value, String> {
        let (manifest,source)=self.verify(id)?;
        let recovery_id=model::id();
        let target=self.directory("recovery")?.join(format!("restored-{recovery_id}"));
        fs::create_dir(&target).map_err(|e|e.to_string())?;
        for item in &manifest.files {
            // Environment ownership is historical evidence, never automatically
            // reactivated against the live distro after a recovery into a new root.
            let name=if item.name=="environment.sqlite3" { "environment-history.sqlite3" } else { &item.name };
            let original=source.join(&item.name); let destination=target.join(name);
            require_plain(&original,false)?;
            fs::copy(&original,&destination).map_err(|e|e.to_string())?;
            if hash_file(&destination)?!=item.sha256 { return Err("A cópia de recuperação não passou na verificação.".into()); }
            fs::OpenOptions::new().write(true).open(&destination).and_then(|file|file.sync_all()).map_err(|e|e.to_string())?;
        }
        // Store::open validates the domain, revokes unused approvals, and marks
        // interrupted executions unknown without dispatching anything.
        let restored=Store::open(target.clone())?;
        crate::work_supervisor::deactivate_after_restore(&restored)?;
        restored.mutate(|state| { state.settings.has_api_key=false; crate::data_policy::deactivate_after_restore(state) })?;
        let summary=restored.snapshot()?;
        drop(restored);
        let receipt=json!({"id":recovery_id,"sourceBackupId":id,"path":target,"verified":true,"activated":false,
            "projects":summary.projects.len(),"assessments":summary.assessments.len(),"findings":summary.findings.len(),
            "credentialsRequireSetup":true,"environmentRequiresRevalidation":true,
            "message":"Dados restaurados e verificados em uma nova pasta. A instalação em uso foi preservada. Credenciais, ambiente e permissões precisam de nova configuração antes de ativar esta cópia."});
        write_new(&target.join("recovery.json"),&serde_json::to_vec_pretty(&receipt).map_err(|e|e.to_string())?)?;
        Ok(receipt)
    }

    fn report(&self, input: ReportInput, store: &Store) -> Result<Value, String> {
        core::valid_id(&input.id)?;
        let state=store.snapshot()?;
        let assessment=state.assessments.iter().find(|a|a.id==input.id).ok_or("Avaliação não encontrada.")?;
        let combined=crate::scope_targets::is_combined(&state,&assessment.id);
        let project=state.projects.iter().find(|p|p.id==assessment.project_id).ok_or("Projeto de origem não encontrado.")?;
        let report_id=model::id();
        let folder=self.directory("exports")?.join(format!("report-{report_id}"));
        fs::create_dir(&folder).map_err(|e|e.to_string())?;
        let mut evidence=vec![];
        for item in &assessment.evidence {
            if core::hash(&item.content)!=item.sha256 { return Err("Uma evidência não passou na verificação de integridade. Exportação interrompida.".into()); }
            let mut record=json!({"id":item.id,"name":redact(&item.name,None),"createdAt":item.created_at,"rawSha256":item.sha256,
                "rawBytes":item.content.len(),"exported":input.include_evidence,"representation":"redacted derivative; original hash identifies preserved raw evidence"});
            if combined{record["targetRefs"]=json!(crate::evidence_lineage::evidence_refs(store,&state,&assessment.id,&[item.id.clone()])?);}
            if input.include_evidence {
                let content=redact(&item.content,None);
                let name=format!("evidence-{}.txt",item.id);
                write_new(&folder.join(&name),content.as_bytes())?;
                record["file"]=json!(name); record["exportedSha256"]=json!(core::hash(&content));
                record["exportedBytes"]=json!(content.len()); record["redacted"]=json!(content!=item.content);
            }
            evidence.push(record);
        }
        let findings=state.findings.iter().filter(|f|f.assessment_id==assessment.id).collect::<Vec<_>>();
        let plans=state.plans.iter().filter(|p|p.assessment_id==assessment.id).collect::<Vec<_>>();
        let executions=state.executions.iter().filter(|e|e.assessment_id==assessment.id).collect::<Vec<_>>();
        let validations=state.validations.iter().filter(|v|v.assessment_id==assessment.id).collect::<Vec<_>>();
        let http_validations=crate::http_validation::report_rows(&state,&assessment.id)?;
        let remediation_database=self.root.join("remediation.sqlite3");
        let remediations=if remediation_database.exists() {
            require_plain(&remediation_database,false)?;
            crate::remediation::report_metadata(&remediation_database,&state,&assessment.id)?
        } else {vec![]};
        let missing=findings.iter().flat_map(|f|&f.evidence_ids).filter(|id|!assessment.evidence.iter().any(|e|e.id==**id)).cloned().collect::<Vec<_>>();
        let finding_records=findings.iter().map(|finding|{
            let mut value=serde_json::to_value(finding).map_err(|e|e.to_string())?;
            if combined{value["targetRefs"]=json!(crate::evidence_lineage::finding_target_refs(store,&state,&finding.id)?);}
            Ok(value)
        }).collect::<Result<Vec<Value>,String>>()?;
        let partial=combined || assessment.status!="completed" || assessment.coverage.is_empty() || !missing.is_empty();
        let mut manifest=json!({"format":1,"reportId":report_id,"createdAt":model::now(),"appVersion":env!("CARGO_PKG_VERSION"),
            "assessment":{"id":assessment.id,"projectId":assessment.project_id,"title":assessment.title,"objective":assessment.objective,"kind":assessment.kind,
                "target":assessment.target,"status":assessment.status,"summary":assessment.summary,"createdAt":assessment.created_at,"updatedAt":assessment.updated_at},
            "project":{"id":project.id,"name":project.name},"coverage":assessment.coverage,"partial":partial,
            "limitations":["Uma avaliação ou execução bem-sucedida não prova ausência de vulnerabilidades","A redação automática é conservadora e não substitui revisão humana antes do compartilhamento","O pacote não inclui credenciais, fontes externas ou uma imagem da distribuição Kali","Correções incluem somente metadados e hashes; antes/depois, predicado textual e bytes de backup não são exportados automaticamente"],
            "redaction":{"policy":"conservative-lines-v1","reviewRequired":true,"rawEvidenceIncluded":false},
            "scope":crate::scope::report_metadata(&state,&assessment.id)?,"evidence":evidence,"missingEvidenceIds":missing,"findings":finding_records,"validations":validations,"httpValidations":http_validations,"remediations":remediations,"plans":plans,"executions":executions,"activity":assessment.events});
        if combined{manifest["targetCoverage"]=json!(crate::scope::target_coverage(&state,&assessment.id));manifest["assessment"]["targetRole"]=json!("presentation_anchor_only");}
        let manifest=redact_value(manifest);
        let json_bytes=serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?;
        write_new(&folder.join("manifest.json"),&json_bytes)?;
        let markdown=markdown_report(&manifest);
        write_new(&folder.join("report.md"),markdown.as_bytes())?;
        let checksum=json!({"manifest.json":core::hash(&String::from_utf8_lossy(&json_bytes)),"report.md":core::hash(&markdown)});
        write_new(&folder.join("checksums.json"),&serde_json::to_vec_pretty(&checksum).map_err(|e|e.to_string())?)?;
        Ok(json!({"id":report_id,"path":folder.join("report.md"),"manifestPath":folder.join("manifest.json"),"directory":folder,"partial":partial,"reviewRequired":true}))
    }
}

pub(crate) fn require_plain(path: &Path, directory: bool) -> Result<(), String> {
    let metadata=fs::symlink_metadata(path).map_err(|e|e.to_string())?;
    let mut reparse=metadata.file_type().is_symlink();
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; reparse|=metadata.file_attributes()&0x400!=0; }
    if reparse || metadata.is_dir()!=directory || (!directory && !metadata.is_file()) { return Err("Links ou tipos de arquivo inesperados não são aceitos na recuperação.".into()); }
    Ok(())
}
pub(crate) fn hash_file(path: &Path) -> Result<String, String> {
    require_plain(path,false)?;
    let mut file=fs::File::open(path).map_err(|e|e.to_string())?;
    if file.metadata().map_err(|e|e.to_string())?.len()>MAX_FILE { return Err("Arquivo excede o limite desta versão de recuperação.".into()); }
    let mut hash=Sha256::new(); let mut buffer=[0;65536]; let mut total=0;
    loop { let read=file.read(&mut buffer).map_err(|e|e.to_string())?;if read==0 { break; }total+=read as u64;if total>MAX_FILE { return Err("Arquivo mudou/excedeu limite durante leitura.".into()); }hash.update(&buffer[..read]); }
    Ok(format!("{:x}",hash.finalize()))
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e|e.to_string())?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())
}
fn read_manifest(path: &Path) -> Result<BackupManifest, String> {
    require_plain(path,false)?;
    if fs::metadata(path).map_err(|e|e.to_string())?.len()>MAX_MANIFEST { return Err("Manifest excede o limite.".into()); }
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|_|"Manifest de backup inválido.".into())
}
fn copy_sqlite(source: &Connection, destination: &mut Connection) -> Result<(), String> {
    let pages:u64=source.query_row("PRAGMA page_count",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    let page_size:u64=source.query_row("PRAGMA page_size",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if pages.saturating_mul(page_size)>MAX_FILE { return Err("Banco excede o limite desta versão de backup.".into()); }
    let backup=Backup::new(source,destination).map_err(|e|e.to_string())?;
    let deadline=Instant::now()+Duration::from_secs(30);
    loop {
        if Instant::now()>deadline { return Err("Backup não obteve uma janela consistente no prazo. A cópia incompleta não será usada.".into()); }
        match backup.step(128).map_err(|e|e.to_string())? {
            StepResult::Done=>break,
            StepResult::More=>{},
            StepResult::Busy|StepResult::Locked=>std::thread::sleep(Duration::from_millis(20)),
            _=>return Err("Estado de backup SQLite desconhecido.".into()),
        }
    }
    Ok(())
}
pub(crate) fn backup_database(source: &Path, destination: &Path) -> Result<(), String> {
    let input=Connection::open_with_flags(source,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
    let mut output=Connection::open(destination).map_err(|e|e.to_string())?;
    output.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;").map_err(|e|e.to_string())?;
    copy_sqlite(&input,&mut output)?;
    output.execute_batch("PRAGMA journal_mode=DELETE;").map_err(|e|e.to_string())?;
    drop(output);
    fs::OpenOptions::new().write(true).open(destination).and_then(|file|file.sync_all()).map_err(|e|e.to_string())
}
pub(crate) fn backup_assistant(source: &Path, destination: &Path) -> Result<bool, String> {
    let input=Connection::open_with_flags(source,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
    let mut memory=Connection::open_in_memory().map_err(|e|e.to_string())?;
    copy_sqlite(&input,&mut memory)?;
    let mut statement=memory.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").map_err(|e|e.to_string())?;
    let tables=statement.query_map([],|row|row.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    drop(statement);
    if tables.iter().any(|name|!["vault","messages","requests","connection_check","investigations"].contains(&name.as_str())) || !tables.iter().any(|name|name=="vault") { return Ok(false); }
    memory.execute_batch("PRAGMA secure_delete=ON; DELETE FROM vault; DELETE FROM connection_check; UPDATE requests SET status='unknown' WHERE status='pending'; VACUUM;").map_err(|e|e.to_string())?;
    let mut output=Connection::open(destination).map_err(|e|e.to_string())?;
    copy_sqlite(&memory,&mut output)?;
    output.execute_batch("PRAGMA journal_mode=DELETE;").map_err(|e|e.to_string())?;
    drop(output);
    fs::OpenOptions::new().write(true).open(destination).and_then(|file|file.sync_all()).map_err(|e|e.to_string())?;
    Ok(true)
}
pub(crate) fn verify_database(path: &Path) -> Result<i64, String> {
    let connection=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
    let result:String=connection.query_row("PRAGMA integrity_check",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if result!="ok" { return Err("A cópia SQLite não passou na verificação de integridade.".into()); }
    connection.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(|e|e.to_string())
}
pub(crate) fn validate_core_database(path: &Path) -> Result<(), String> {
    let connection=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|e.to_string())?;
    connection.busy_timeout(Duration::from_secs(2)).map_err(|e|e.to_string())?;
    connection.execute_batch("PRAGMA trusted_schema=OFF; BEGIN;").map_err(|e|e.to_string())?;
    let version:i64=connection.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if version>core::STORE_VERSION{return Err("Versão do banco principal mais recente que este aplicativo.".into());}
    let raw:String=connection.query_row("SELECT json FROM app_state WHERE id=1",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if raw.len()>64*1024*1024 { return Err("Snapshot excede o limite de leitura.".into()); }
    let state:Snapshot=serde_json::from_str(&raw).map_err(|_|"Snapshot de backup inválido.")?;
    core::validate_snapshot(&state)?;
    if version>=7 {
        if version==7{
            // Verify the historical DDL before adding the V8 table in memory.
            // The source backup remains read-only and keeps its exact bytes.
            data::verify_schema(&connection,"openkoda.sqlite3")?;
            let pages:u64=connection.query_row("PRAGMA page_count",[],|r|r.get(0)).map_err(|e|e.to_string())?;
            let page_size:u64=connection.query_row("PRAGMA page_size",[],|r|r.get(0)).map_err(|e|e.to_string())?;
            if pages.checked_mul(page_size).is_none_or(|bytes|bytes>MAX_FILE){return Err("Banco histórico excede o limite de validação de recuperação.".into());}
            let mut memory=Connection::open_in_memory().map_err(|e|e.to_string())?;
            {
                let backup=Backup::new(&connection,&mut memory).map_err(|e|e.to_string())?;
                let deadline=Instant::now()+Duration::from_secs(30);
                loop{
                    if Instant::now()>deadline{return Err("Validação histórica excedeu o limite de tempo; backup preservado.".into());}
                    match backup.step(128).map_err(|e|e.to_string())?{StepResult::Done=>break,StepResult::More=>{},StepResult::Busy|StepResult::Locked=>std::thread::sleep(Duration::from_millis(10)),_=>return Err("Estado de cópia SQLite não reconhecido.".into())}
                }
            }
            memory.execute_batch(crate::work_store::WORK_SCHEMA_SQL).map_err(|e|e.to_string())?;
            memory.pragma_update(None,"user_version",core::STORE_VERSION).map_err(|e|e.to_string())?;
            data::verify_schema(&memory,"openkoda.sqlite3")?;
            crate::work_store::validate_store(&memory,&state)?;
            crate::capture::validate_capture_tables(&memory)?;
            crate::capture::validate_capture_domain(&memory,&state)?;
        }else{
            crate::work_store::validate_store(&connection,&state)?;
            crate::capture::validate_capture_tables(&connection)?;
            crate::capture::validate_capture_domain(&connection,&state)?;
        }
    }
    Ok(())
}
fn redact_value(value: Value) -> Value {
    match value {
        Value::String(value)=>json!(redact(&value,None)),
        Value::Array(items)=>Value::Array(items.into_iter().map(redact_value).collect()),
        Value::Object(items)=>Value::Object(items.into_iter().map(|(key,value)|(key,redact_value(value))).collect()),
        other=>other,
    }
}
fn markdown_text(value: &str) -> String {
    let mut escaped=String::new();
    for ch in value.chars() {
        match ch {
            '&'=>escaped.push_str("&amp;"), '<'=>escaped.push_str("&lt;"), '>'=>escaped.push_str("&gt;"),
            '\n'|'\r'=>escaped.push_str(" "),
            '\\'|'`'|'*'|'_'|'{'|'}'|'['|']'|'('|')'|'#'|'+'|'-'|'!'|'|'=>{escaped.push('\\');escaped.push(ch);},
            ch if ch.is_control()=>{}, ch=>escaped.push(ch),
        }
    }
    escaped
}
fn markdown_report(manifest: &Value) -> String {
    let text=|value:&Value|markdown_text(value.as_str().unwrap_or(""));
    let assessment=&manifest["assessment"];
    let mut output=format!("# OpenKoda — {}\n\nProjeto: {}\n\nAlvo: {}\n\nEstado: {}\n\nRelatório: {}\n\n",text(&assessment["title"]),text(&manifest["project"]["name"]),text(&assessment["target"]),text(&assessment["status"]),text(&manifest["reportId"]));
    if manifest["partial"]==true { output.push_str("**Avaliação parcial. Este relatório não declara ausência de vulnerabilidades.**\n\n"); }
    output.push_str(&format!("## Objetivo e resumo\n\n{}\n\n{}\n\n## Cobertura\n\n",text(&assessment["objective"]),text(&assessment["summary"])));
    if let Some(items)=manifest["coverage"].as_array() { if items.is_empty() {output.push_str("Nenhuma cobertura registrada.\n\n");}for item in items {output.push_str(&format!("- {}\n",text(item)));} }
    if let Some(targets)=manifest["targetCoverage"].as_array(){
        output.push_str("\n### Cobertura por entrada\n\nO alvo do cabeçalho é uma referência de apresentação. Cada operação cobre somente a entrada indicada; ausência de achado não comprova segurança do conjunto.\n\n");
        for target in targets{
            output.push_str(&format!("- {} · {} · {}\n",text(&target["label"]),text(&target["kind"]),text(&target["target"])));
            if let Some(methods)=target["methods"].as_array(){for method in methods{output.push_str(&format!("  Método {}: {}.\n",text(&method["method"]),text(&method["state"])));}}
        }
    }
    output.push_str("\n## Candidatos e achados\n\n");
    if let Some(items)=manifest["findings"].as_array() { for item in items {
        output.push_str(&format!("### {}\n\nEstado: {} · Severidade: {} · Confiança: {}\n\n{}\n\nRecomendação: {}\n\n",text(&item["title"]),text(&item["status"]),text(&item["severity"]),text(&item["confidence"]),text(&item["description"]),text(&item["recommendation"])));
    } }
    output.push_str("## Registros de validação\n\n");
    if let Some(items)=manifest["validations"].as_array() { for item in items {
        output.push_str(&format!("- Achado: {} · Método: {} · Autor: {} · Data: {}\n  - Esperado: {}\n  - Observado: {}\n",text(&item["findingId"]),text(&item["method"]),text(&item["author"]),text(&item["createdAt"]),text(&item["expected"]),text(&item["observed"])));
    } }
    output.push_str("\n## Correções e retestes\n\nSomente metadados e hashes. Conteúdo antes/depois, valor do predicado e backup de bytes permanecem locais. Reteste textual/hash não é auditoria completa.\n\n");
    if let Some(items)=manifest["remediations"].as_array() { for item in items {
        output.push_str(&format!("- Correção: {} · Achado: {} · Arquivo relativo: {} · Estado: {}\n  - SHA-256 base: {} · Após alteração: {} · Digest revisado: {}\n  - Reteste: {} · Evidência: {} · Confirmação humana em: {}\n",
            text(&item["id"]),text(&item["findingId"]),text(&item["relativePath"]),text(&item["status"]),text(&item["baseSha256"]),text(&item["afterSha256"]),text(&item["planDigest"]),text(&item["retest"]["kind"]),text(&item["evidenceId"]),text(&item["confirmedAt"])));
    } }
    output.push_str(&crate::scope::markdown_metadata(&manifest["scope"],markdown_text));
    if let Some(rows)=manifest["httpValidations"].as_array(){output.push_str(&crate::http_validation::markdown_rows(rows,markdown_text));}
    output.push_str("## Evidências\n\nO manifest JSON registra origem, hash do bruto preservado e hash da cópia redigida, quando incluída.\n\n");
    if let Some(items)=manifest["evidence"].as_array() { for item in items {output.push_str(&format!("- {} · {} · SHA-256 bruto: {}\n",text(&item["id"]),text(&item["name"]),text(&item["rawSha256"])));if let Some(refs)=item["targetRefs"].as_array(){output.push_str(&format!("  Entradas de origem: {}.\n",if refs.is_empty(){"registro comum do ambiente; não comprova cobertura de alvo".into()}else{refs.iter().map(|r|text(&r["entryId"])).collect::<Vec<_>>().join(", ")}));}} }
    output.push_str("\n## Limites e revisão\n\n");
    if let Some(items)=manifest["limitations"].as_array() {for item in items {output.push_str(&format!("- {}\n",text(item)));}}
    output.push_str("\nRevise os dados sensíveis antes de compartilhar. Nenhum conteúdo foi enviado externamente por esta exportação.\n");
    output
}

#[cfg(test)]
pub(crate) mod test_support{
    use super::*;
    /// Exercise the public preparation/admission boundary for report fixtures.
    /// This only creates ordinary metadata reviews using synthetic test data.
    pub fn report(service:&RecoveryService,store:&Store,assessment_id:&str,include_evidence:bool)->Result<Value,String>{
        use crate::{data_policy::{DataPolicyService,DataSubject},data_projection::ExternalSubjectResolver};
        let policy=DataPolicyService::new();let ai=crate::ai::AiService::new(store.root.clone())?;let redactor=ai.data_redactor();let resolver=crate::data_host::JournalResolver{store};
        crate::data_policy::test_support::authorize(store,&policy,assessment_id,&redactor,&resolver,None)?;
        let state=store.snapshot()?;let assessment=state.assessments.iter().find(|a|a.id==assessment_id).ok_or("Avaliação não encontrada.")?;
        let body=crate::scope_targets::current_body(&state,assessment_id)?;
        let refs=body.target_set.as_ref().map(|set|set.entries.iter().map(crate::scope_targets::entry_ref).collect::<Vec<_>>()).unwrap_or_default();
        let mut derived=state.findings.iter().filter(|f|f.assessment_id==assessment_id).map(|f|DataSubject::Derived{assessment_id:assessment_id.into(),producer:crate::data_policy::DerivedProducer::Finding,object_id:f.id.clone()}).collect::<Vec<_>>();
        derived.extend(state.validations.iter().filter(|v|v.assessment_id==assessment_id).map(|v|DataSubject::Derived{assessment_id:assessment_id.into(),producer:crate::data_policy::DerivedProducer::Validation,object_id:v.id.clone()}));
        derived.extend(resolver.list(assessment_id)?);
        let evidence=if include_evidence{assessment.evidence.iter().map(|e|e.id.clone()).collect::<Vec<_>>()}else{vec![]};
        let input=json!({"assessmentId":assessment_id,"format":"markdown_bundle","evidenceIds":evidence,"derivedSubjects":derived,"categories":["human_text","target_metadata","execution_summary","evidence_excerpt","source_excerpt","finding_text","activity_metadata"],"contextTargetRefs":refs,"packageSensitivity":"public"});
        let preview=service.handle_reviewed("report.prepare",input,store,&policy,&redactor)?;
        if preview["canExport"]!=true{return Err(format!("Report fixture requires review: {}",preview["reasonCodes"]));}
        service.handle_reviewed("report.commit",json!({"dataReview":preview["dataReview"],"acknowledgeExport":true}),store,&policy,&redactor)
    }
    pub fn backup(service:&RecoveryService,store:&Store)->Result<Value,String>{
        use crate::data_policy::*;
        let policy=DataPolicyService::new();let ai=crate::ai::AiService::new(store.root.clone())?;let redactor=ai.data_redactor();let resolver=crate::data_host::JournalResolver{store};
        for assessment in store.snapshot()?.assessments{crate::data_policy::test_support::authorize(store,&policy,&assessment.id,&redactor,&resolver,None)?;}
        for project in store.snapshot()?.projects{
            let status=policy.handle("data.policy.status",json!({"projectId":project.id}),store,&redactor,&resolver,None)?;
            if status["currentRevision"].is_null(){
                let mut body=default_body();body.recovery_backup.enabled=true;body.recovery_backup.sensitivities=vec![Sensitivity::Public,Sensitivity::Internal,Sensitivity::Sensitive];
                let preview=policy.handle("data.policy.prepare",json!({"projectId":project.id,"expectedStateDigest":status["stateDigest"],"body":body}),store,&redactor,&resolver,None)?;
                policy.handle("data.policy.commit",json!({"review":preview["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}),store,&redactor,&resolver,None)?;
            }
        }
        let projects=store.snapshot()?.projects.into_iter().map(|p|p.id).collect::<Vec<_>>();
        let preview=service.handle_reviewed("backup.prepare",json!({"projectIds":projects,"packageSensitivity":"internal","acknowledgeRecoveryPurpose":true}),store,&policy,&redactor)?;
        if preview["canBackup"]!=true{return Err(format!("Backup fixture requires review: {}",preview["reasonCodes"]));}
        service.handle_reviewed("backup.commit",json!({"dataReview":preview["dataReview"],"acknowledgeRecoveryBackup":true,"acknowledgeSensitive":true}),store,&policy,&redactor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Request;
    #[test]
    fn schema_seven_backup_validates_without_mutation_and_migrates_only_the_copy(){
        let(dir,store,_aid,manifest)=crate::capture::fixture::build(b"historical UTF-8 capture",crate::capture::CaptureState::Sealed);
        let source=store.root.join("openkoda.sqlite3");drop(store);
        let db=Connection::open(&source).unwrap();
        db.execute_batch("DROP TABLE work_ai_dispositions; PRAGMA user_version=7; PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
        let original:String=db.query_row("SELECT json FROM app_state WHERE id=1",[],|r|r.get(0)).unwrap();drop(db);
        let before=hash_file(&source).unwrap();validate_core_database(&source).unwrap();
        let prepared=data::prepare_database(&source,"openkoda.sqlite3",&|_|false).unwrap();assert_eq!(prepared.inventory.user_version,7);
        let destination=dir.path().join("version-eight-copy");fs::create_dir(&destination).unwrap();prepared.write_new(&destination.join("openkoda.sqlite3")).unwrap();
        assert_eq!(hash_file(&source).unwrap(),before);assert_eq!(verify_database(&source).unwrap(),7);
        let copy=Store::open(destination).unwrap();copy.work_read(|db,_|{
            let raw:String=db.query_row("SELECT json FROM app_state WHERE id=1",[],|r|r.get(0)).unwrap();assert_eq!(raw,original);
            assert_eq!(db.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),core::STORE_VERSION);
            assert_eq!(crate::capture::load_locked(db,&manifest.id)?,manifest);assert_eq!(crate::capture::preserved_locked(db,&manifest)?,b"historical UTF-8 capture");Ok(())
        }).unwrap();
    }
    fn seed(store:&Store)->(String,String) {
        let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Backup lab","kind":"web","target":"https://lab.invalid","description":""})}).unwrap();
        let assessment=store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Coverage lab","kind":"web","target":project["target"],"objective":"Fixture only","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&[])})}).unwrap();
        (project["id"].as_str().unwrap().into(),assessment["id"].as_str().unwrap().into())
    }
    #[test]
    fn backup_captures_wal_and_restore_preserves_live_database() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();let service=RecoveryService::new(root.path().into()).unwrap();let (project,assessment)=seed(&store);
        let backup=service.create_backup(&store).unwrap();
        store.handle(Request{action:"project.archive".into(),payload:json!({"id":project})}).unwrap();
        let restored=service.handle("backup.restore",json!({"id":backup["id"]}),&store).unwrap();
        assert_eq!(restored["activated"],false);
        assert!(store.snapshot().unwrap().projects[0].archived);
        let copy=Store::open(PathBuf::from(restored["path"].as_str().unwrap())).unwrap();
        assert!(!copy.snapshot().unwrap().projects[0].archived);
        assert_eq!(copy.snapshot().unwrap().assessments[0].id,assessment);
    }
    #[test]
    fn backup_accepts_all_registered_service_databases() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();
        let _assistant=crate::ai::AiService::new(root.path().into()).unwrap();
        for name in ["environment.sqlite3","kali-bridge.sqlite3","remediation.sqlite3"] {
            Connection::open(root.path().join(name)).unwrap().execute_batch("CREATE TABLE history(id INTEGER PRIMARY KEY, value TEXT);").unwrap();
        }
        let service=RecoveryService::new(root.path().into()).unwrap();
        let backup=service.create_backup(&store).unwrap();
        assert_eq!(backup["manifest"]["files"].as_array().unwrap().len(),5);
        assert!(service.verify(backup["id"].as_str().unwrap()).is_ok());
    }
    #[test]
    fn recovered_profile_environment_history_survives_another_backup_and_restore() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();seed(&store);
        let history=Connection::open(root.path().join("environment-history.sqlite3")).unwrap();history.execute_batch("CREATE TABLE audit(value TEXT); INSERT INTO audit VALUES('historical managed identity only');").unwrap();drop(history);
        let service=RecoveryService::new(root.path().into()).unwrap();let backup=service.create_backup(&store).unwrap();
        assert!(backup["manifest"]["files"].as_array().unwrap().iter().any(|f|f["name"]=="environment-history.sqlite3"));
        let first=service.restore(backup["id"].as_str().unwrap()).unwrap();let path=PathBuf::from(first["path"].as_str().unwrap());assert!(!path.join("environment.sqlite3").exists());
        let copy=Store::open(path.clone()).unwrap();let next=RecoveryService::new(path.clone()).unwrap();let second=next.create_backup(&copy).unwrap();
        let restored=next.restore(second["id"].as_str().unwrap()).unwrap();let target=PathBuf::from(restored["path"].as_str().unwrap());
        assert!(!target.join("environment.sqlite3").exists());
        assert_eq!(Connection::open(target.join("environment-history.sqlite3")).unwrap().query_row("SELECT value FROM audit",[],|r|r.get::<_,String>(0)).unwrap(),"historical managed identity only");
    }
    #[test]
    fn conflicting_active_and_historical_environment_records_are_not_overwritten() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();
        for name in ["environment.sqlite3","environment-history.sqlite3"] {Connection::open(root.path().join(name)).unwrap().execute_batch("CREATE TABLE audit(value TEXT)").unwrap();}
        let service=RecoveryService::new(root.path().into()).unwrap();assert!(service.create_backup(&store).is_err());
        assert!(root.path().join("environment.sqlite3").exists());assert!(root.path().join("environment-history.sqlite3").exists());
    }
    #[test]
    fn tampered_backup_is_rejected_and_never_restored() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();let service=RecoveryService::new(root.path().into()).unwrap();seed(&store);
        let backup=service.create_backup(&store).unwrap();let path=PathBuf::from(backup["path"].as_str().unwrap()).join("openkoda.sqlite3");
        let mut file=fs::OpenOptions::new().append(true).open(path).unwrap();file.write_all(b"tampered").unwrap();drop(file);
        assert!(service.restore(backup["id"].as_str().unwrap()).is_err());
        assert!(!root.path().join("recovery").exists());
    }
    #[test]
    fn backup_ids_cannot_supply_paths_and_manifest_cannot_escape() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();let service=RecoveryService::new(root.path().into()).unwrap();
        assert!(service.handle("backup.restore",json!({"id":"../../outside"}),&store).is_err());
        let backup=service.create_backup(&store).unwrap();let path=PathBuf::from(backup["path"].as_str().unwrap()).join("manifest.json");
        let mut manifest:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();manifest["files"][0]["name"]=json!("../outside");fs::write(path,manifest.to_string()).unwrap();
        assert!(service.verify(backup["id"].as_str().unwrap()).is_err());
    }
    #[test]
    fn assistant_backup_excludes_vault_bytes_and_keeps_history() {
        let root=tempfile::tempdir().unwrap();let source=root.path().join("assistant.sqlite3");let target=root.path().join("copy.sqlite3");
        let db=Connection::open(&source).unwrap();db.execute_batch("CREATE TABLE vault(id INTEGER,cipher BLOB); CREATE TABLE messages(text TEXT); CREATE TABLE requests(status TEXT); CREATE TABLE connection_check(id INTEGER); INSERT INTO vault VALUES(1,'SENTINEL-CIPHER-DO-NOT-COPY'); INSERT INTO messages VALUES('preserved message'); INSERT INTO requests VALUES('pending'); INSERT INTO connection_check VALUES(1);").unwrap();
        assert!(backup_assistant(&source,&target).unwrap());
        assert!(!String::from_utf8_lossy(&fs::read(&target).unwrap()).contains("SENTINEL-CIPHER-DO-NOT-COPY"));
        let restored=Connection::open(&target).unwrap();assert_eq!(restored.query_row("SELECT text FROM messages",[],|r|r.get::<_,String>(0)).unwrap(),"preserved message");
        assert_eq!(restored.query_row("SELECT status FROM requests",[],|r|r.get::<_,String>(0)).unwrap(),"unknown");
    }
    #[test]
    fn reports_are_partial_escaped_redacted_and_preserve_raw_evidence() {
        let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();let service=RecoveryService::new(root.path().into()).unwrap();let (_,id)=seed(&store);
        let content="Authorization: Bearer SENTINEL\n<script>active()</script>";
        store.mutate(|state| {let a=&mut state.assessments[0];a.evidence.push(model::Evidence{id:model::id(),name:"<script>name</script>".into(),content:content.into(),sha256:core::hash(content),created_at:model::now()});a.title="Lab\n# forged-complete <script>".into();Ok(())}).unwrap();
        let report=service.report(ReportInput{id,include_evidence:true},&store).unwrap();
        assert_eq!(report["partial"],true);
        let markdown=fs::read_to_string(report["path"].as_str().unwrap()).unwrap();assert!(!markdown.contains("\n# forged-complete"));assert!(!markdown.contains("<script>"));
        let manifest:Value=serde_json::from_slice(&fs::read(report["manifestPath"].as_str().unwrap()).unwrap()).unwrap();
        let derived=fs::read_to_string(PathBuf::from(report["directory"].as_str().unwrap()).join(manifest["evidence"][0]["file"].as_str().unwrap())).unwrap();
        assert!(!derived.contains("SENTINEL"));assert_eq!(store.snapshot().unwrap().assessments[0].evidence[0].content,content);
        assert_eq!(manifest["evidence"][0]["rawSha256"],core::hash(content));assert_eq!(manifest["evidence"][0]["exportedSha256"],core::hash(&derived));
    }
}
