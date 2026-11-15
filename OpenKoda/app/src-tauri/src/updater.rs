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
use zeroize::Zeroizing;

const MAX_DOWNLOAD: usize = 384 * 1024 * 1024;
const REPOSITORY_DOWNLOAD: &str = "/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/";
const GITHUB_FEED: &str = "https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/desktop-updates/latest.json";
const UPDATE_ORIGIN: &str = "https://updates.openglitch.com.br";
// Release gate: enable only after the production origin, storage and policies
// pass their deployment checks. No renderer/environment override is accepted.
const CLOUD_UPDATES_ENABLED: bool = false;
const MAX_MANIFEST: usize = 128 * 1024;
fn cloud_enabled()->bool { CLOUD_UPDATES_ENABLED && !cfg!(feature="updater-test-harness") }

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Channel { #[default] Stable, Early, Experimental }
impl Channel {
    fn name(self)->&'static str { match self { Self::Stable=>"stable", Self::Early=>"early", Self::Experimental=>"experimental" } }
    fn private(self)->bool { self!=Self::Stable }
}
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Preferences { installation_id: uuid::Uuid, channel: Channel, experimental_opt_in: bool }
impl Default for Preferences { fn default()->Self { Self { installation_id:uuid::Uuid::new_v4(), channel:Channel::Stable, experimental_opt_in:false } } }
impl Preferences {
    fn validate(&self)->Result<(),String> {
        if self.installation_id.is_nil() || (self.channel==Channel::Experimental && !self.experimental_opt_in) {
            return Err("A participação experimental exige sua confirmação explícita.".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Input { action: Action, channel: Option<Channel>, experimental_opt_in: Option<bool> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Action { Status, Check, Download, Install, Defer, Channel }

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
    channel: Channel,
    experimental_opt_in: bool,
    channels_available: bool,
}
impl Default for Status {
    fn default() -> Self { Self { phase: "idle", current_version: env!("CARGO_PKG_VERSION").into(), version: None,
        notes: None, downloaded_bytes: 0, total_bytes: None, message: "Verifique se há uma nova versão do aplicativo.".into(), deferred: false, backup_path: None,
        channel:Channel::Stable, experimental_opt_in:false, channels_available:cloud_enabled() } }
}
struct State { view: Status, update: Option<Update>, cached: Option<PathBuf>, last_check: Option<Instant>, preferences: Option<Preferences>, account_revision: Option<String> }
pub(crate) struct UpdateManager { state: Mutex<State> }
impl Default for UpdateManager {
    fn default() -> Self { Self { state: Mutex::new(State { view: Status::default(), update: None, cached: None, last_check: None, preferences:None, account_revision:None }) } }
}
impl UpdateManager {
    pub(crate) fn new(version:String)->Self { let manager=Self::default(); manager.change(|s|s.view.current_version=version); manager }
    fn view(&self) -> Status { self.state.lock().unwrap_or_else(|e|e.into_inner()).view.clone() }
    fn change(&self, change: impl FnOnce(&mut State)) { change(&mut self.state.lock().unwrap_or_else(|e|e.into_inner())); }
    fn begin(&self, phase: &'static str, expected:&Preferences) -> Result<(), String> {
        let mut state = self.state.lock().unwrap_or_else(|e|e.into_inner());
        if matches!(state.view.phase, "checking"|"downloading"|"installing") { return Err("Já existe uma operação de atualização em andamento.".into()); }
        if state.preferences.as_ref()!=Some(expected){return Err("O canal mudou. Verifique novamente as atualizações.".into());}
        state.view.phase = phase;
        state.view.deferred = false;
        Ok(())
    }
    fn failed(&self, message: String) -> Status {
        self.change(|s| { s.view.phase = if s.cached.is_some() { "ready" } else if s.update.is_some() { "available" } else { "error" }; s.view.message = message; });
        self.view()
    }
    fn preferences(&self,base:&Path)->Result<Preferences,String> {
        let mut state=self.state.lock().unwrap_or_else(|e|e.into_inner());
        if state.preferences.is_none() {
            let mut preferences=read_preferences(base)?;
            // A binary that has not passed the distribution release gate cannot
            // activate a private channel from an old preference file.
            if !cloud_enabled() { preferences.channel=Channel::Stable; preferences.experimental_opt_in=false; }
            state.view.channel=preferences.channel; state.view.experimental_opt_in=preferences.experimental_opt_in;
            state.preferences=Some(preferences);
        }
        Ok(state.preferences.as_ref().unwrap().clone())
    }
    fn select_channel(&self,base:&Path,channel:Channel,opt_in:bool)->Result<Status,String> {
        if !cloud_enabled() && channel.private() {return Err("Os canais antecipado e experimental ainda não estão disponíveis nesta versão.".into());}
        let mut state=self.state.lock().unwrap_or_else(|e|e.into_inner());
        if matches!(state.view.phase,"checking"|"downloading"|"installing") {return Err("Aguarde a operação de atualização atual.".into());}
        let mut preferences=state.preferences.clone().ok_or("Preferências de atualização indisponíveis.")?;
        preferences.channel=channel; preferences.experimental_opt_in=channel==Channel::Experimental&&opt_in;
        preferences.validate()?; write_preferences(base,&preferences)?;
        if let Some(path)=state.cached.take(){let _=fs::remove_file(path);}
        state.update=None;state.account_revision=None;state.last_check=None;
        state.view=Status { current_version:state.view.current_version.clone(),channel,experimental_opt_in:preferences.experimental_opt_in,
            message:"Canal selecionado. Verifique as atualizações; o serviço confirmará o acesso da sua conta.".into(),..Status::default() };
        state.preferences=Some(preferences);Ok(state.view.clone())
    }
}

#[tauri::command]
pub(crate) async fn desktop_update(window: WebviewWindow, app: tauri::AppHandle,
    manager: tauri::State<'_, Arc<UpdateManager>>, services: tauri::State<'_, Arc<Services>>,
    store: tauri::State<'_, Arc<Store>>, storage: tauri::State<'_, StorageLease>, input: Input) -> Result<Status, String> {
    if window.label() != "main" { return Err("Janela não autorizada.".into()); }
    let manager = manager.inner().clone();
    let preferences=manager.preferences(&storage.base)?;
    if !matches!(input.action,Action::Channel) && (input.channel.is_some()||input.experimental_opt_in.is_some()) {
        return Err("Parâmetros de atualização inválidos.".into());
    }
    match input.action {
        Action::Status => Ok(manager.view()),
        Action::Defer => { manager.change(|s|s.view.deferred = true); Ok(manager.view()) },
        Action::Channel => manager.select_channel(&storage.base,input.channel.ok_or("Escolha um canal de atualização.")?,input.experimental_opt_in.unwrap_or(false)),
        Action::Check => {
            { let state = manager.state.lock().unwrap_or_else(|e|e.into_inner());
              if state.last_check.is_some_and(|at|at.elapsed()<Duration::from_secs(15)) { return Ok(state.view.clone()); } }
            manager.begin("checking",&preferences)?;
            manager.change(|s| { s.last_check = Some(Instant::now()); s.view.message = "Consultando atualizações…".into(); });
            let result = async {
                let credentials=channel_credentials(services.inner().clone(),preferences.channel,None).await?;
                let builder=app.updater_builder();
                #[cfg(not(feature="updater-test-harness"))]
                let builder=builder.endpoints(vec![url::Url::parse(GITHUB_FEED).unwrap()]).map_err(|_|"Endereço de atualização inválido.")?;
                // The isolated, compile-time harness retains its separately
                // signed rehearsal feed. Production has no endpoint override.
                let updater = builder.timeout(Duration::from_secs(20)).build().map_err(|_|"Não foi possível preparar o atualizador.")?;
                let update = if cloud_enabled() {
                    match cloud_manifest(&preferences,&manager.view().current_version,credentials.as_ref().map(|c|c.0.as_str())).await {
                        Ok(Some(manifest))=>updater.restore_update(manifest).map_err(|_|"O serviço retornou um manifesto de atualização inválido.".to_string())?,
                        Ok(None)=>None,
                        Err(failure) if preferences.channel==Channel::Stable && failure.retry_legacy => updater.check().await.map_err(|_|"Não foi possível consultar os serviços de atualização. Você pode continuar trabalhando e tentar novamente.".to_string())?,
                        Err(failure)=>return Err(failure.message),
                    }
                } else { updater.check().await.map_err(|_|"Não foi possível consultar o GitHub. Você pode continuar trabalhando e tentar novamente.".to_string())? };
                let update = match update {
                    Some(update) if eligible_rollout(&storage.base, &update.raw_json)? => { validate_candidate(&update,preferences.channel)?; Some(update) },
                    _ => None,
                };
                manager.change(|s| {
                    if let Some(path) = s.cached.take() { let _ = fs::remove_file(path); }
                    s.view.version = update.as_ref().map(|u|u.version.clone());
                    s.view.notes = update.as_ref().and_then(|u|u.body.as_ref().map(|s|s.chars().take(8000).collect()));
                    s.view.phase = if update.is_some() { "available" } else { "current" };
                    s.view.message = if update.is_some() { "Há uma atualização disponível. Seus projetos e conversas permanecem no computador." } else { "Você está usando a versão disponível para esta instalação." }.into();
                    s.view.downloaded_bytes = 0; s.view.total_bytes = None; s.update = update;
                    s.account_revision=credentials.map(|(_,revision)|revision);
                });
                Ok::<_, String>(manager.view())
            }.await;
            Ok(result.unwrap_or_else(|error|manager.failed(error)))
        },
        Action::Download => {
            let (update,revision) = {let state=manager.state.lock().unwrap_or_else(|e|e.into_inner());(state.update.clone().ok_or("Verifique as atualizações primeiro.")?,state.account_revision.clone())};
            let _admission = services.close_gate.admit(Some(Activity::request("update.download", &json!({})).unwrap()))?;
            manager.begin("downloading",&preferences)?;
            manager.change(|s| { s.view.downloaded_bytes = 0; s.view.total_bytes = None; s.view.message = "Baixando a atualização. Você pode continuar trabalhando.".into(); });
            let base = storage.base.clone();
            let result = async {
                validate_candidate(&update,preferences.channel)?;
                let credentials=channel_credentials(services.inner().clone(),preferences.channel,revision).await?;
                let bytes = download_bounded(&update.download_url,&preferences,credentials.as_ref().map(|c|c.0.as_str()), |received,total|manager.change(|s| { s.view.downloaded_bytes = received; s.view.total_bytes = total; })).await?;
                if cloud_url(&update.download_url) {
                    let advertised=update.download_url.path().split('/').nth(7).ok_or("Identidade do arquivo inválida.")?;
                    if format!("{:x}",Sha256::digest(&bytes))!=advertised {return Err("O arquivo recebido não corresponde à atualização anunciada.".into());}
                }
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
            let (update, cached,revision) = { let state = manager.state.lock().unwrap_or_else(|e|e.into_inner());
                (state.update.clone().ok_or("Verifique as atualizações primeiro.")?, state.cached.clone().ok_or("Baixe a atualização antes de instalar.")?,state.account_revision.clone()) };
            manager.begin("installing",&preferences)?;
            manager.change(|s|s.view.message = "Verificando operações e criando a cópia de segurança…".into());
            if preferences.channel.private() {
                let authorization=async {
                    let credentials=channel_credentials(services.inner().clone(),preferences.channel,revision.clone()).await?;
                    authorize_cached(&update.download_url,&preferences,credentials.as_ref().map(|c|c.0.as_str())).await
                }.await;
                if let Err(reason)=authorization{return Ok(manager.failed(reason));}
            }
            let services = services.inner().clone(); let store = store.inner().clone(); let base = storage.base.clone(); let result_manager = manager.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                let _freeze = services.close_gate.begin_update()?;
                let _maintenance = services.environment_gate.try_write().map_err(|_|"Aguarde o término das operações antes de atualizar.")?;
                services.supervisor.shared_arbiter().ensure_quiescent(&store)?;
                crate::ensure_purge_access(&services, "update.install")?;
                if services.activation.blocks_new_effects()? || services.environment.has_unresolved_maintenance()? || services.kali.has_unresolved() || services.remediation.has_unresolved()? {
                    return Err("Reconcilie a manutenção pendente antes de instalar uma atualização.".into());
                }
                // Account replacement/sign-out after the server check cannot
                // install a cached private-channel package under another account.
                if preferences.channel.private() && services.ai.account.identity_revision()?!=revision {
                    return Err("A conta mudou. Verifique novamente as atualizações.".into());
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

fn read_preferences(base:&Path)->Result<Preferences,String> {
    recovery::require_plain(base,true)?;
    let path=base.join("update-preferences.json");
    if !path.try_exists().map_err(|_|"Preferências de atualização indisponíveis.")? {
        let preferences=Preferences::default();write_preferences(base,&preferences)?;return Ok(preferences);
    }
    recovery::require_plain(&path,false)?;
    if fs::metadata(&path).map_err(|_|"Preferências de atualização indisponíveis.")?.len()>4096 {return Err("Preferências de atualização inválidas.".into());}
    let preferences:Preferences=serde_json::from_slice(&fs::read(path).map_err(|_|"Preferências de atualização indisponíveis.")?).map_err(|_|"Preferências de atualização inválidas.")?;
    preferences.validate()?;Ok(preferences)
}
fn write_preferences(base:&Path,preferences:&Preferences)->Result<(),String> {
    recovery::require_plain(base,true)?;preferences.validate()?;
    let path=base.join("update-preferences.json");if path.exists(){recovery::require_plain(&path,false)?;}
    let temp=base.join(format!("update-preferences-{}.tmp",uuid::Uuid::new_v4()));
    use std::io::Write;
    let result=(||{
        let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|_|"Não foi possível guardar o canal de atualização.")?;
        file.write_all(&serde_json::to_vec(preferences).map_err(|_|"Preferências inválidas.")?).and_then(|_|file.sync_all()).map_err(|_|"Não foi possível guardar o canal de atualização.")?;
        drop(file);fs::rename(&temp,&path).map_err(|_|"Não foi possível concluir as preferências de atualização.".to_string())
    })();
    if result.is_err(){let _=fs::remove_file(temp);}result
}
type Credentials=(Zeroizing<String>,String);
async fn channel_credentials(services:Arc<Services>,channel:Channel,expected:Option<String>)->Result<Option<Credentials>,String> {
    if !channel.private(){return Ok(None);}
    tauri::async_runtime::spawn_blocking(move||{
        let credentials=services.ai.account.credential()?;
        if expected.as_ref().is_some_and(|revision|revision!=&credentials.1){return Err("A conta mudou. Verifique novamente as atualizações.".into());}
        Ok(Some(credentials))
    }).await.map_err(|_|"Não foi possível verificar a sessão de atualização.".to_string())?
}
fn cloud_url(url:&url::Url)->bool {url.origin().ascii_serialization()==UPDATE_ORIGIN}
fn validate_download_url(url: &url::Url) -> Result<(), String> {
    let github=url.host_str()==Some("github.com")&&url.path().starts_with(REPOSITORY_DOWNLOAD);
    let parts:Vec<_>=url.path().split('/').collect();
    let cloud=cloud_url(url)&&parts.len()==9&&parts[1]=="v1"&&parts[2]=="downloads"
        &&matches!(parts[3],"stable"|"early"|"experimental")&&parts[5]=="windows"&&parts[6]=="x86_64"
        &&parts[7].len()==64&&parts[7].bytes().all(|b|b.is_ascii_hexdigit())
        &&parts[4].bytes().all(|b|b.is_ascii_alphanumeric()||b==b'.'||b==b'-')&&!parts[4].is_empty()
        &&parts[8].ends_with(".exe")&&parts[8].bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'-'|b'_'));
    if url.scheme() != "https" || url.port().is_some() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || !(github||cloud) {
        return Err("O endereço da atualização não pertence às releases oficiais do OpenKoda.".into());
    }
    Ok(())
}
fn validate_candidate(update:&Update,channel:Channel)->Result<(),String> {
    validate_download_url(&update.download_url)?;
    if cloud_url(&update.download_url) {
        let segments:Vec<_>=update.download_url.path().split('/').collect();
        if segments[3]!=channel.name()||segments[4]!=update.version {return Err("A atualização não corresponde ao canal ou à versão anunciada.".into());}
    } else if channel.private(){return Err("Canais privados exigem o serviço autenticado de atualização.".into());}
    Ok(())
}
struct FeedFailure {message:String,retry_legacy:bool}
fn feed_failure(message:&str,retry_legacy:bool)->FeedFailure {FeedFailure {message:message.into(),retry_legacy}}
fn private_request(client:&reqwest::Client,method:reqwest::Method,url:url::Url,preferences:&Preferences,bearer:Option<&str>)->Result<reqwest::RequestBuilder,String> {
    if !cloud_url(&url)||url.scheme()!="https"||url.port().is_some()||url.query().is_some()||!url.username().is_empty()||url.password().is_some()||url.fragment().is_some(){return Err("Origem de atualização inválida.".into());}
    let mut request=client.request(method,url).header("X-Koda-Installation-Id",preferences.installation_id.to_string());
    if preferences.channel.private(){request=request.bearer_auth(bearer.ok_or("Entre na sua conta para acessar este canal.")?);}
    if preferences.channel==Channel::Experimental {
        if !preferences.experimental_opt_in{return Err("Confirme a participação experimental.".into());}
        request=request.header("X-Koda-Experimental","true");
    }
    Ok(request)
}
fn cloud_client(timeout:Duration)->Result<reqwest::Client,String> {
    reqwest::Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).timeout(timeout).connect_timeout(Duration::from_secs(20))
        .build().map_err(|_|"Não foi possível preparar a conexão de atualização.".into())
}
fn response_error(status:reqwest::StatusCode)->String {
    match status.as_u16(){
        401=>"Entre novamente na sua conta para consultar este canal.",
        403=>"Este canal não está disponível para sua assinatura. Use o canal estável ou consulte seu plano.",
        409|410=>"A atualização mudou desde a consulta. Verifique novamente antes de baixar.",
        429=>"Aguarde um pouco antes de consultar novamente as atualizações.",
        _=>"O serviço não disponibilizou esta atualização. Tente novamente mais tarde."
    }.into()
}
async fn cloud_manifest(preferences:&Preferences,current_version:&str,bearer:Option<&str>)->Result<Option<serde_json::Value>,FeedFailure> {
    let url=url::Url::parse(&format!("{UPDATE_ORIGIN}/v1/updates/{}/windows/x86_64/{current_version}",preferences.channel.name())).map_err(|_|feed_failure("Endereço de atualização inválido.",false))?;
    let client=cloud_client(Duration::from_secs(20)).map_err(|e|feed_failure(&e,false))?;
    let mut response=private_request(&client,reqwest::Method::GET,url,preferences,bearer).map_err(|e|feed_failure(&e,false))?
        .header("Accept","application/json").send().await.map_err(|_|feed_failure("Não foi possível consultar o serviço de atualização.",true))?;
    read_manifest_response(&mut response).await
}
async fn read_manifest_response(response:&mut reqwest::Response)->Result<Option<serde_json::Value>,FeedFailure> {
    if response.status()==reqwest::StatusCode::NO_CONTENT {return Ok(None);}
    if !response.status().is_success(){return Err(feed_failure(&response_error(response.status()),response.status().is_server_error()));}
    if response.content_length().is_some_and(|n|n>MAX_MANIFEST as u64){return Err(feed_failure("Manifesto de atualização excede o limite.",false));}
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_|feed_failure("Manifesto de atualização interrompido.",true))? {
        if bytes.len().saturating_add(chunk.len())>MAX_MANIFEST{return Err(feed_failure("Manifesto de atualização excede o limite.",false));}
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map(Some).map_err(|_|feed_failure("Manifesto de atualização inválido.",false))
}
async fn authorize_cached(url:&url::Url,preferences:&Preferences,bearer:Option<&str>)->Result<(),String> {
    validate_download_url(url)?;
    let client=cloud_client(Duration::from_secs(20))?;
    let response=private_request(&client,reqwest::Method::HEAD,url.clone(),preferences,bearer)?.send().await.map_err(|_|"Não foi possível confirmar o acesso à atualização. Tente novamente.")?;
    if !response.status().is_success(){return Err(response_error(response.status()));}Ok(())
}
fn eligible_rollout(base:&Path, manifest:&serde_json::Value) -> Result<bool,String> {
    let percent = match manifest.get("rolloutPercent") { None => 100, Some(n) => n.as_u64().filter(|n|*n<=100).ok_or("Percentual de distribuição da atualização inválido.")? };
    // Local-only deterministic cohort: neither device identity nor path is sent.
    let digest = Sha256::digest(base.as_os_str().to_string_lossy().as_bytes());
    Ok((u16::from_le_bytes([digest[0],digest[1]]) % 100) < percent as u16)
}
async fn download_bounded(url:&url::Url, preferences:&Preferences,bearer:Option<&str>,mut progress:impl FnMut(u64,Option<u64>)) -> Result<Vec<u8>,String> {
    validate_download_url(url)?;
    if cloud_url(url) {
        let client=cloud_client(Duration::from_secs(600))?;
        let response=private_request(&client,reqwest::Method::GET,url.clone(),preferences,bearer)?.header("Accept","application/octet-stream")
            .send().await.map_err(|_|"Download interrompido. Confira a conexão e tente novamente.")?;
        return receive_bounded(response,&mut progress).await;
    }
    if preferences.channel.private()||bearer.is_some(){return Err("Credenciais privadas não podem ser enviadas ao canal legado.".into());}
    let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).connect_timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let target = attempt.url();
            if attempt.previous().len()<5 && target.scheme()=="https" && matches!(target.host_str(),Some("github.com"|"release-assets.githubusercontent.com"|"objects.githubusercontent.com")) { attempt.follow() } else { attempt.stop() }
        })).build().map_err(|_|"Não foi possível iniciar o download.")?;
    let response = client.get(url.clone()).header("Accept","application/octet-stream").send().await.map_err(|_|"Download interrompido. Confira a conexão e tente novamente.")?;
    receive_bounded(response, &mut progress).await
}
async fn receive_bounded(mut response:reqwest::Response, progress:&mut impl FnMut(u64,Option<u64>)) -> Result<Vec<u8>,String> {
    if !response.status().is_success() { return Err(response_error(response.status())); }
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
            } else if matches!(name,"active-profile.json"|"ai-budget.installation.json"|"window-behavior.json"|"update-preferences.json") {
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
    #[test] fn cloud_downloads_pin_origin_platform_and_asset_path() {
        let good=format!("{UPDATE_ORIGIN}/v1/downloads/early/1.1.2/windows/x86_64/{}/OpenKoda_1.1.2_x64-setup.exe","a".repeat(64));
        assert!(validate_download_url(&url::Url::parse(&good).unwrap()).is_ok());
        for bad in [good.replace("updates.openglitch.com.br","updates.openglitch.com.br.evil.invalid"),good.replace("https:","http:"),format!("{good}?token=secret"),good.replace("/windows/","/linux/"),good.replace("/early/","/unknown/"),good.replace("/OpenKoda_","/%2FOpenKoda_")] {
            assert!(validate_download_url(&url::Url::parse(&bad).unwrap()).is_err(),"untrusted download accepted");
        }
    }
    #[test] fn private_credentials_are_native_origin_only_and_experimental_is_explicit() {
        let client=cloud_client(Duration::from_secs(1)).unwrap();
        let mut preferences=Preferences::default();
        let url=url::Url::parse(&format!("{UPDATE_ORIGIN}/v1/updates/experimental/windows/x86_64/1.1.2")).unwrap();
        let stable=private_request(&client,reqwest::Method::GET,url.clone(),&preferences,Some("not-sent-for-stable")).unwrap().build().unwrap();
        assert!(!stable.headers().contains_key("authorization"));
        preferences.channel=Channel::Experimental;
        assert!(preferences.validate().is_err());
        assert!(private_request(&client,reqwest::Method::GET,url.clone(),&preferences,Some("fixture-only")).is_err());
        preferences.experimental_opt_in=true;
        let private=private_request(&client,reqwest::Method::GET,url.clone(),&preferences,Some("fixture-only")).unwrap().build().unwrap();
        assert_eq!(private.headers()["authorization"],"Bearer fixture-only");
        assert_eq!(private.headers()["x-koda-experimental"],"true");
        assert!(private.url().query().is_none());
        assert!(private_request(&client,reqwest::Method::GET,url.clone(),&preferences,None).is_err());
        assert!(private_request(&client,reqwest::Method::GET,url::Url::parse(GITHUB_FEED).unwrap(),&preferences,Some("fixture-only")).is_err());
        assert!(serde_json::from_value::<Input>(json!({"action":"channel","channel":"admin"})).is_err());
        assert!(serde_json::from_value::<Input>(json!({"action":"check","token":"fixture-only"})).is_err());
    }
    #[test] fn channel_preferences_preserve_identity_and_reject_interleaved_operations() {
        let dir=tempfile::tempdir().unwrap();let manager=UpdateManager::new("1.1.2".into());
        let preferences=manager.preferences(dir.path()).unwrap();
        assert!(!preferences.installation_id.is_nil());
        assert_eq!(read_preferences(dir.path()).unwrap().installation_id,preferences.installation_id);
        manager.select_channel(dir.path(),Channel::Stable,false).unwrap();
        assert_eq!(read_preferences(dir.path()).unwrap().installation_id,preferences.installation_id);
        manager.begin("downloading",&preferences).unwrap();
        assert!(manager.select_channel(dir.path(),Channel::Stable,false).is_err());
        assert!(manager.begin("installing",&preferences).is_err());
        manager.change(|s|{s.view.phase="idle";s.preferences.as_mut().unwrap().experimental_opt_in=true;});
        assert!(manager.begin("checking",&preferences).is_err());
        assert!(fs::read_dir(dir.path()).unwrap().all(|entry|entry.unwrap().path().extension().is_none_or(|ext|ext!="tmp")));
    }
    #[test] fn paused_denied_and_oversized_manifests_never_fallback_to_legacy() {
        use std::io::{Read,Write};
        for status in [204,401,403,409,429,503,200] {
            let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
            let server=std::thread::spawn(move||{let (mut stream,_)=listener.accept().unwrap();let mut buffer=[0;2048];let _=stream.read(&mut buffer);
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",if status==200{MAX_MANIFEST+1}else{0}).unwrap();});
            let result=tauri::async_runtime::block_on(async {let mut response=reqwest::Client::new().get(format!("http://{address}")).send().await.unwrap();read_manifest_response(&mut response).await});
            if status==204 {assert!(matches!(result,Ok(None)));}
            else {assert_eq!(result.err().unwrap().retry_legacy,status==503);}
            server.join().unwrap();
        }
    }
}
