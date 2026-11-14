//! The webview selects a verb, never a URL, executable, public key or path.
//! Updater signatures bind the version; installation additionally freezes work
//! and snapshots SQLite with the online backup API before starting NSIS.
use crate::{close_guard::Activity, core::Store, recovery, storage::StorageLease, Services};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use tauri::WebviewWindow;
use tauri_plugin_updater::{Update, UpdaterExt};

const MAX_DOWNLOAD: usize = 384 * 1024 * 1024;
const REPOSITORY_DOWNLOAD: &str = "/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Input { action: Action }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Action { Status, Check, Download, Install, Defer }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    phase: &'static str,
    current_version: String,
    version: Option<String>,
    notes: Option<String>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    message: String,
    deferred: bool,
    backup_path: Option<String>,
}
impl Default for Status {
    fn default() -> Self { Self { phase: "idle", current_version: env!("CARGO_PKG_VERSION").into(), version: None,
        notes: None, downloaded_bytes: 0, total_bytes: None, message: "Verifique se há uma nova versão do aplicativo.".into(), deferred: false, backup_path: None } }
}
struct State { view: Status, update: Option<Update>, cached: Option<PathBuf>, last_check: Option<Instant> }
pub(crate) struct UpdateManager { state: Mutex<State> }
impl Default for UpdateManager {
    fn default() -> Self { Self { state: Mutex::new(State { view: Status::default(), update: None, cached: None, last_check: None }) } }
}
impl UpdateManager {
    pub(crate) fn new(version:String)->Self { let manager=Self::default(); manager.change(|s|s.view.current_version=version); manager }
    fn view(&self) -> Status { self.state.lock().unwrap_or_else(|e|e.into_inner()).view.clone() }
    fn change(&self, change: impl FnOnce(&mut State)) { change(&mut self.state.lock().unwrap_or_else(|e|e.into_inner())); }
    fn begin(&self, phase: &'static str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap_or_else(|e|e.into_inner());
        if matches!(state.view.phase, "checking"|"downloading"|"installing") { return Err("Já existe uma operação de atualização em andamento.".into()); }
        state.view.phase = phase;
        state.view.deferred = false;
        Ok(())
    }
    fn failed(&self, message: String) -> Status {
        self.change(|s| { s.view.phase = if s.cached.is_some() { "ready" } else if s.update.is_some() { "available" } else { "error" }; s.view.message = message; });
        self.view()
    }
}

#[tauri::command]
pub(crate) async fn desktop_update(window: WebviewWindow, app: tauri::AppHandle,
    manager: tauri::State<'_, Arc<UpdateManager>>, services: tauri::State<'_, Arc<Services>>,
    store: tauri::State<'_, Arc<Store>>, storage: tauri::State<'_, StorageLease>, input: Input) -> Result<Status, String> {
    if window.label() != "main" { return Err("Janela não autorizada.".into()); }
    let manager = manager.inner().clone();
    match input.action {
        Action::Status => Ok(manager.view()),
        Action::Defer => { manager.change(|s|s.view.deferred = true); Ok(manager.view()) },
        Action::Check => {
            { let state = manager.state.lock().unwrap_or_else(|e|e.into_inner());
              if state.last_check.is_some_and(|at|at.elapsed()<Duration::from_secs(15)) { return Ok(state.view.clone()); } }
            manager.begin("checking")?;
            manager.change(|s| { s.last_check = Some(Instant::now()); s.view.message = "Consultando atualizações…".into(); });
            let result = async {
                let updater = app.updater_builder().timeout(Duration::from_secs(20)).build().map_err(|_|"Não foi possível preparar o atualizador.")?;
                let update = updater.check().await.map_err(|_|"Não foi possível consultar o GitHub. Você pode continuar trabalhando e tentar novamente.")?;
                let update = match update {
                    Some(update) if eligible_rollout(&storage.base, &update.raw_json)? => { validate_download_url(&update.download_url)?; Some(update) },
                    _ => None,
                };
                manager.change(|s| {
                    if let Some(path) = s.cached.take() { let _ = fs::remove_file(path); }
                    s.view.version = update.as_ref().map(|u|u.version.clone());
                    s.view.notes = update.as_ref().and_then(|u|u.body.as_ref().map(|s|s.chars().take(8000).collect()));
                    s.view.phase = if update.is_some() { "available" } else { "current" };
                    s.view.message = if update.is_some() { "Há uma atualização disponível. Seus projetos e conversas permanecem no computador." } else { "Você está usando a versão disponível para esta instalação." }.into();
                    s.view.downloaded_bytes = 0; s.view.total_bytes = None; s.update = update;
                });
                Ok::<_, String>(manager.view())
            }.await;
            Ok(result.unwrap_or_else(|error|manager.failed(error)))
        },
        Action::Download => {
            let update = manager.state.lock().unwrap_or_else(|e|e.into_inner()).update.clone().ok_or("Verifique as atualizações primeiro.")?;
            let _admission = services.close_gate.admit(Some(Activity::request("update.download", &json!({})).unwrap()))?;
            manager.begin("downloading")?;
            manager.change(|s| { s.view.downloaded_bytes = 0; s.view.total_bytes = None; s.view.message = "Baixando a atualização. Você pode continuar trabalhando.".into(); });
            let base = storage.base.clone();
            let result = async {
                let bytes = download_bounded(&update.download_url, |received,total|manager.change(|s| { s.view.downloaded_bytes = received; s.view.total_bytes = total; })).await?;
                update.verify(&bytes).map_err(|_|"A assinatura da atualização não confere. O arquivo não será instalado.".to_string())?;
                let cache = prepare_directory(&base, "update-cache")?;
                let path = cache.join(format!("{}.bin", uuid::Uuid::new_v4()));
                use std::io::Write;
                let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_|"Não foi possível salvar a atualização.".to_string())?;
                file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|"Não foi possível concluir o arquivo da atualização.".to_string())?;
                manager.change(|s| { if let Some(old) = s.cached.replace(path) { let _ = fs::remove_file(old); } s.view.phase = "ready"; s.view.message = "Download verificado. Instale quando terminar seu trabalho; o aplicativo será reiniciado.".into(); });
                Ok::<_,String>(manager.view())
            }.await;
            Ok(result.unwrap_or_else(|error|manager.failed(error)))
        },
        Action::Install => {
            let (update, cached) = { let state = manager.state.lock().unwrap_or_else(|e|e.into_inner());
                (state.update.clone().ok_or("Verifique as atualizações primeiro.")?, state.cached.clone().ok_or("Baixe a atualização antes de instalar.")?) };
            manager.begin("installing")?;
            manager.change(|s|s.view.message = "Verificando operações e criando a cópia de segurança…".into());
            let services = services.inner().clone(); let store = store.inner().clone(); let base = storage.base.clone(); let result_manager = manager.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let _freeze = services.close_gate.begin_update()?;
                let _maintenance = services.environment_gate.try_write().map_err(|_|"Aguarde o término das operações antes de atualizar.")?;
                services.supervisor.shared_arbiter().ensure_quiescent(&store)?;
                crate::ensure_purge_access(&services, "update.install")?;
                if services.activation.blocks_new_effects()? || services.environment.has_unresolved_maintenance()? || services.kali.has_unresolved() || services.remediation.has_unresolved()? {
                    return Err("Reconcilie a manutenção pendente antes de instalar uma atualização.".into());
                }
                recovery::require_plain(&cached, false)?;
                if fs::metadata(&cached).map_err(|_|"Arquivo de atualização indisponível.")?.len() > MAX_DOWNLOAD as u64 { return Err("Arquivo de atualização excede o limite.".into()); }
                let bytes = fs::read(cached).map_err(|_|"Arquivo de atualização indisponível.")?;
                update.verify(&bytes).map_err(|_|"A atualização foi alterada ou tem assinatura inválida. Baixe novamente.")?;
                let backup = backup_before_update(&base, &store.root, &update.current_version, &update.version)?;
                result_manager.change(|s| { s.view.backup_path = Some(backup.display().to_string()); s.view.message = "Cópia de segurança verificada. Reiniciando para instalar…".into(); });
                update.restart_after_install(true).install(bytes).map_err(|_|"O instalador não iniciou. Os dados e a cópia de segurança foram preservados.".to_string())?;
                Ok::<_,String>(result_manager.view())
            }).await.map_err(|_|"A instalação foi interrompida; seus dados foram preservados.".to_string()).and_then(|r|r);
            Ok(result.unwrap_or_else(|error|manager.failed(error)))
        },
    }
}

fn validate_download_url(url: &url::Url) -> Result<(), String> {
    if url.scheme() != "https" || url.host_str() != Some("github.com") || url.port().is_some() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || !url.path().starts_with(REPOSITORY_DOWNLOAD) {
        return Err("O endereço da atualização não pertence às releases oficiais do OpenKoda.".into());
    }
    Ok(())
}
fn eligible_rollout(base:&Path, manifest:&serde_json::Value) -> Result<bool,String> {
    let percent = match manifest.get("rolloutPercent") { None => 100, Some(n) => n.as_u64().filter(|n|*n<=100).ok_or("Percentual de distribuição da atualização inválido.")? };
    // Local-only deterministic cohort: neither device identity nor path is sent.
    let digest = Sha256::digest(base.as_os_str().to_string_lossy().as_bytes());
    Ok((u16::from_le_bytes([digest[0],digest[1]]) % 100) < percent as u16)
}
async fn download_bounded(url:&url::Url, mut progress:impl FnMut(u64,Option<u64>)) -> Result<Vec<u8>,String> {
    validate_download_url(url)?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).connect_timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let target = attempt.url();
            if attempt.previous().len()<5 && target.scheme()=="https" && matches!(target.host_str(),Some("github.com"|"release-assets.githubusercontent.com"|"objects.githubusercontent.com")) { attempt.follow() } else { attempt.stop() }
        })).build().map_err(|_|"Não foi possível iniciar o download.")?;
    let response = client.get(url.clone()).header("Accept","application/octet-stream").send().await.map_err(|_|"Download interrompido. Confira a conexão e tente novamente.")?;
    receive_bounded(response, &mut progress).await
}
async fn receive_bounded(mut response:reqwest::Response, progress:&mut impl FnMut(u64,Option<u64>)) -> Result<Vec<u8>,String> {
    if !response.status().is_success() { return Err("O GitHub não disponibilizou o arquivo desta atualização. Tente novamente mais tarde.".into()); }
    let total = response.content_length();
    if total.is_some_and(|n|n>MAX_DOWNLOAD as u64) { return Err("A atualização excede o tamanho máximo permitido.".into()); }
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_|"Download interrompido. Tente novamente.")? {
        if bytes.len().saturating_add(chunk.len())>MAX_DOWNLOAD { return Err("A atualização excede o tamanho máximo permitido.".into()); }
        bytes.extend_from_slice(&chunk); progress(bytes.len() as u64,total);
    }
    Ok(bytes)
}
fn prepare_directory(base:&Path, name:&str)->Result<PathBuf,String> {
    recovery::require_plain(base,true)?;
    let directory=base.join(name);
    if !directory.exists() { fs::create_dir(&directory).map_err(|_|"Não foi possível preparar a pasta da atualização.")?; }
    recovery::require_plain(&directory,true)?;
    Ok(directory)
}

/// This private, same-device recovery copy retains encrypted local credentials;
/// it is not the reviewed/exportable backup and is never uploaded or synced.
pub(crate) fn backup_before_update(base:&Path, root:&Path, current_version:&str, next_version:&str)->Result<PathBuf,String> {
    recovery::require_plain(base,true)?; recovery::require_plain(root,true)?;
    let directory=prepare_directory(base,"update-backups")?;
    let destination=directory.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&destination).map_err(|_|"Não foi possível criar a cópia anterior à atualização.")?;
    let mut files=Vec::new();
    let sources=if root==base { vec![("profile",root)] } else { vec![("installation",base),("profile",root)] };
    for (area, source) in sources {
        let target=destination.join(area); fs::create_dir(&target).map_err(|_|"Falha ao preparar cópia dos dados.")?;
        let entries=fs::read_dir(source).map_err(|_|"Não foi possível ler os dados antes de atualizar.")?;
        for entry in entries {
            let entry=entry.map_err(|_|"Não foi possível listar os dados antes de atualizar.")?;
            let file=entry.path();
            let Some(name)=file.file_name().and_then(|s|s.to_str()) else {continue};
            if file.extension().is_some_and(|e|e=="sqlite3") {
                recovery::require_plain(&file,false)?;
                let copy=target.join(name); recovery::backup_database(&file,&copy)?;
                let db=rusqlite::Connection::open_with_flags(&copy,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_|"Não foi possível validar a cópia.")?;
                let integrity:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(|_|"Não foi possível validar a cópia.")?;
                if integrity!="ok" { return Err("A cópia de segurança não passou na verificação; a atualização não foi iniciada.".into()); }
                files.push(json!({"area":area,"name":name,"sha256":recovery::hash_file(&copy)?}));
            } else if matches!(name,"active-profile.json"|"ai-budget.installation.json"|"window-behavior.json") {
                recovery::require_plain(&file,false)?;
                if fs::metadata(&file).map_err(|_|"Metadados indisponíveis.")?.len()>1024*1024 { return Err("Metadados locais excedem o limite de segurança.".into()); }
                fs::copy(&file,target.join(name)).map_err(|_|"Não foi possível preservar os metadados locais.")?;
            }
        }
    }
    if !files.iter().any(|f|f["area"]=="profile" && f["name"]=="openkoda.sqlite3") { return Err("Banco principal ausente. Atualização não iniciada.".into()); }
    let manifest=json!({"schema":"openkoda.update-backup.v1","createdAt":crate::model::now(),"fromVersion":current_version,"toVersion":next_version,"profileDirectory":root,"files":files,"complete":true,"purpose":"Recuperação local; não compartilhar. Credenciais permanecem criptografadas para este usuário do Windows."});
    fs::write(destination.join("manifest.json"),serde_json::to_vec_pretty(&manifest).map_err(|_|"Falha ao registrar a cópia.")?).map_err(|_|"Falha ao finalizar a cópia de segurança.")?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn forbids_arbitrary_installer_origins_and_redirect_input() {
        for url in ["http://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/v/x.exe","https://evil.invalid/x.exe","https://github.com/other/repo/releases/download/v/x.exe","https://user@github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/v/x.exe"] { assert!(validate_download_url(&url::Url::parse(url).unwrap()).is_err()); }
        assert!(validate_download_url(&url::Url::parse("https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/desktop-v1.1.0/OpenKoda_1.1.0_x64-setup.exe").unwrap()).is_ok());
        assert!(serde_json::from_value::<Input>(json!({"action":"install","url":"https://evil.invalid"})).is_err());
    }
    #[test] fn staged_rollout_can_pause_or_reach_everyone() {
        let root=Path::new("test"); assert!(!eligible_rollout(root,&json!({"rolloutPercent":0})).unwrap()); assert!(eligible_rollout(root,&json!({"rolloutPercent":100})).unwrap()); assert!(eligible_rollout(root,&json!({"rolloutPercent":101})).is_err());
    }
    #[test] fn backup_preserves_wal_projects_conversations_and_settings_without_touching_originals() {
        let dir=tempfile::tempdir().unwrap(); let store=Store::open(dir.path().to_path_buf()).unwrap();
        store.handle(crate::core::Request { action:"project.create".into(),payload:json!({"name":"Update preservation","kind":"web","target":"https://test.invalid","description":"preserve"}) }).unwrap();
        let original=serde_json::to_value(store.snapshot().unwrap()).unwrap();
        let chat=rusqlite::Connection::open(dir.path().join("assistant.sqlite3")).unwrap();
        chat.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE messages(id TEXT,text TEXT); INSERT INTO messages VALUES('1','Conversation before upgrade'); CREATE TABLE vault(cipher BLOB); INSERT INTO vault VALUES(X'010203');").unwrap();
        let backup=backup_before_update(dir.path(),dir.path(),"1.0.9","1.1.0").unwrap();
        let manifest:serde_json::Value=serde_json::from_slice(&fs::read(backup.join("manifest.json")).unwrap()).unwrap(); assert_eq!(manifest["fromVersion"],"1.0.9"); assert_eq!(manifest["toVersion"],"1.1.0");
        assert_eq!(serde_json::to_value(store.snapshot().unwrap()).unwrap(),original);
        let copy=Store::open(backup.join("profile")).unwrap(); assert_eq!(serde_json::to_value(copy.snapshot().unwrap()).unwrap(),original);
        let saved=rusqlite::Connection::open(backup.join("profile/assistant.sqlite3")).unwrap(); assert_eq!(saved.query_row("SELECT text FROM messages",[],|r|r.get::<_,String>(0)).unwrap(),"Conversation before upgrade");
        assert_eq!(saved.query_row("SELECT hex(cipher) FROM vault",[],|r|r.get::<_,String>(0)).unwrap(),"010203");
        chat.execute("INSERT INTO messages VALUES('2','Still works')",[]).unwrap();
        assert_eq!(saved.query_row("SELECT count(*) FROM messages",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    }
    #[test] fn installation_freeze_rejects_running_jobs_and_releases_after_failure() {
        let gate=Arc::new(crate::close_guard::CloseGate::default());
        let job=gate.admit(Some(Activity::queue(None))).unwrap(); assert!(gate.begin_update().is_err()); drop(job);
        let freeze=gate.begin_update().unwrap(); assert!(gate.admit(Some(Activity::queue(None))).is_err()); assert!(gate.check_close(true).is_some()); drop(freeze);
        assert!(gate.admit(Some(Activity::queue(None))).is_ok());
    }
    #[test] fn download_size_limit_rejects_oversized_http_response_without_allocating_payload() {
        use std::io::{Read,Write};
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let server=std::thread::spawn(move|| {let (mut stream,_)=listener.accept().unwrap();let mut buffer=[0u8;2048];let _=stream.read(&mut buffer);write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",MAX_DOWNLOAD+1).unwrap();});
        let result=tauri::async_runtime::block_on(async { let response=reqwest::Client::new().get(format!("http://{address}/fixture")).send().await.unwrap(); receive_bounded(response,&mut|_,_|panic!("Oversized payload must be refused before consuming bytes")).await });
        assert!(result.unwrap_err().contains("tamanho"));server.join().unwrap();
    }
}
