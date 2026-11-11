//! Managed WSL environment and package maintenance. No arbitrary shell API.
//!
//! Registration GUID + resolved storage location + Linux marker bind ownership.
//! A package plan is immutable, expires, and is re-simulated before installation.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::official_image::{ImageSource, OfficialImageSource, VerifiedImage};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

#[path = "environment_archives.rs"]
mod archives;
#[path = "environment_host.rs"]
mod host;
#[path = "host_connectivity.rs"]
pub(crate) mod connectivity;
#[path = "environment_removal.rs"]
mod removal;
pub(crate) use removal::REMOVAL_TABLE;

pub const DISTRO: &str = "OpenKoda-Kali";
const HELPER: &str = include_str!("../../scripts/environment-helper.py");
const HELPER_BOOT: &str = "import json,sys; r=json.load(sys.stdin); sys.argv=['openkoda-maintenance',r['action'],r['identity'],*r['args']]; exec(compile(r['helper'],'openkoda-maintenance','exec'))";
const CAP: usize = 2 * 1024 * 1024;
const REGISTRY: &str = r#"$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $items=@(); if(Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss'){ $items=@(Get-ChildItem -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss' | ForEach-Object { $p=Get-ItemProperty -LiteralPath $_.PSPath; [PSCustomObject]@{guid=$_.PSChildName; name=$p.DistributionName; basePath=$p.BasePath; version=$p.Version} }) }; ConvertTo-Json -InputObject $items -Compress"#;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    guid: String,
    name: String,
    base_path: String,
    version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnvironmentState {
    id: String,
    phase: String,
    registration: Option<Registration>,
    last_error: Option<String>,
    last_output: String,
    updated_at: String,
    #[serde(default)]
    install_exit: Option<InstallExit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_checkpoint: Option<host::Checkpoint>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct InstallExit { confirmed:bool, code:Option<i32> }

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct RepairAttempt {
    id:String,plan_id:String,environment_id:String,registration:Registration,
    maintenance_sha256:String,status:String,created_at:String,finished_at:Option<String>,
    termination_receipt:Option<Value>,error:Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    id: String,
    kind: String,
    environment_id: String,
    created_at: String,
    expires_at: String,
    payload: Value,
    digest: String,
    consumed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Approval {
    plan_id: String,
    digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageSelection {
    packages: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ImageSelection { #[serde(default)] local_file:Option<String> }

#[derive(Clone, Debug)]
struct CommandSpec {
    purpose: &'static str,
    executable: PathBuf,
    args: Vec<String>,
    input: Option<Vec<u8>>,
    timeout: Duration,
}

#[derive(Clone, Debug)]
struct ProcessResult {
    code: i32,
    stdout: String,
    stderr: String,
    timed_out: bool,
    truncated: bool,
}

trait Runner: Send + Sync {
    fn run(&self, command: &CommandSpec) -> Result<ProcessResult, String>;
    fn export_tar(&self, _distribution: &str, _target: fs::File, _limit: u64) -> Result<(u64, String), String> {
        Err("O exportador TAR não está disponível neste executor.".into())
    }

}

struct SystemRunner;

fn system_binary(name: &str) -> PathBuf {
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(windows).join("System32").join(name)
}

fn capture(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buffer = [0; 8192];
        let mut truncated = false;
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Err(_) => {
                    // A read failure is not EOF: even a complete-looking JSON
                    // prefix cannot confirm that the process output was captured.
                    truncated = true;
                    break;
                }
                Ok(length) => {
                    let remaining = CAP.saturating_sub(collected.len());
                    collected.extend_from_slice(&buffer[..length.min(remaining)]);
                    truncated |= length > remaining;
                }
            }
        }
        let _ = sender.send((collected, truncated));
    });
    receiver
}

fn output_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.iter().skip(1).step_by(2).take(100).filter(|v| **v == 0).count() > 5 {
        let start = usize::from(bytes.starts_with(&[0xff, 0xfe])) * 2;
        String::from_utf16_lossy(&bytes[start..].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(bytes).trim_start_matches('\u{feff}').to_owned()
    }
}

fn process_command(spec: &CommandSpec) -> Command {
    let mut command = Command::new(&spec.executable);
    command.args(&spec.args).stdin(if spec.input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped()).stderr(Stdio::piped()).env_clear();
    // WSLENV and provider secrets are deliberately not inherited.
    for name in ["SystemRoot", "WINDIR", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) { command.env(name, value); }
    }
    command.env("PATH", system_binary("")).env("WSLENV", "");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

impl Runner for SystemRunner {
    fn export_tar(&self, distribution: &str, target: fs::File, limit: u64) -> Result<(u64, String), String> {
        archives::system_export_tar(distribution, target, limit)
    }

    fn run(&self, spec: &CommandSpec) -> Result<ProcessResult, String> {
        let mut command = process_command(spec);
        let mut child = command.spawn().map_err(|e| format!("Não foi possível iniciar {}: {e}", spec.purpose))?;
        let stdout = capture(child.stdout.take().ok_or("Saída de processo indisponível.")?);
        let stderr = capture(child.stderr.take().ok_or("Diagnóstico de processo indisponível.")?);
        if let Some(bytes) = spec.input.clone() {
            if let Some(mut stdin) = child.stdin.take() {
                std::thread::spawn(move || { let _ = stdin.write_all(&bytes); });
            }
        }
        let deadline = Instant::now() + spec.timeout;
        let mut timed_out = false;
        let code = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status.code().unwrap_or(-1),
                None if Instant::now() >= deadline => {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break -1;
                }
                None => std::thread::sleep(Duration::from_millis(25)),
            }
        };
        // Pipe disappearance is bounded too. A timed-out WSL client does not prove
        // its Linux operation stopped; callers persist unknown maintenance state.
        let out = stdout.recv_timeout(Duration::from_secs(3));
        let err = stderr.recv_timeout(Duration::from_secs(3));
        let missing = out.is_err() || err.is_err();
        let (out, out_cap) = out.unwrap_or_default();
        let (err, err_cap) = err.unwrap_or_default();
        Ok(ProcessResult { code, stdout: output_text(&out), stderr: output_text(&err), timed_out,
            truncated: missing || out_cap || err_cap })
    }
}

pub struct EnvironmentManager {
    root: PathBuf,
    connection: Mutex<Connection>,
    operation: Mutex<()>,
    runner: Arc<dyn Runner>,
    images: Arc<dyn ImageSource>,
    archives: archives::ArchiveStore,
}

pub fn supports(action: &str) -> bool {
    removal::supports(action) || archives::supports(action) || matches!(action, "environment.host.check" | "environment.host.resume" | "environment.status" | "environment.diagnose" | "environment.plan" |
        "environment.provision" | "environment.resume" | "packages.catalog" | "packages.refresh" |
        "packages.plan" | "packages.apply" | "environment.image.plan" | "environment.image.provision" |
        "environment.repair.plan" | "environment.repair.apply")
}

pub fn is_maintenance(action: &str) -> bool {
    (removal::supports(action) && action != "environment.removal.status") || archives::exclusive(action) || matches!(action, "environment.provision" | "environment.resume" | "packages.refresh" | "packages.apply" | "environment.image.provision" | "environment.repair.apply")
}

fn now() -> String { chrono::Utc::now().to_rfc3339() }
fn digest(value: &Value) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap_or_default())) }
fn decode<T: serde::de::DeserializeOwned>(payload: Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|_| "Campos de manutenção inválidos ou não permitidos.".to_owned())
}
fn empty(payload: &Value) -> Result<(), String> {
    if payload.as_object().is_some_and(|v| v.is_empty()) { Ok(()) } else { Err("Esta ação não recebe campos.".into()) }
}

impl EnvironmentManager {
    pub fn new(root: PathBuf) -> Result<Self, String> { Self::with_runner(root, Arc::new(SystemRunner)) }

    /// Revalidates managed ownership and effective Linux configuration for a
    /// native executor bridge. Never expose an identity supplied by the WebView.
    /// The caller must hold the shared environment gate for its whole operation.
    pub fn executor_identity(&self) -> Result<String, String> {
        let _operation = self.operation.try_lock().map_err(|_| "Outra operação do ambiente está em andamento.")?;
        let state = self.require_ready()?;
        Ok(state.id)
    }

    /// Resolve only the owned Windows registration. This never starts Linux;
    /// the session-bound MCP executor must verify the live runtime before its
    /// handshake. The caller holds the environment gate through that session.
    pub(crate) fn executor_owned_identity(&self) -> Result<String,String> {
        self.archives.check_source_stop()?;
        let _operation=self.operation.try_lock().map_err(|_|"Outra operação do ambiente está em andamento.")?;
        let state=self.state()?;
        crate::core::valid_id(&state.id)?;
        if state.phase!="ready"{return Err("Prepare e reconcilie o ambiente antes de iniciar uma sessão MCP.".into());}
        self.owned_registration(&state)?;
        Ok(state.id)
    }

    /// Identity for a pure data preview. This reads only the local ownership
    /// record; it is not a readiness probe or permission to launch a tool.
    pub(crate) fn data_review_identity(&self) -> Result<String,String> {
        self.archives.check_source_stop()?;
        let state=self.state()?;
        crate::core::valid_id(&state.id)?;
        if state.phase!="ready"||state.registration.as_ref().is_none_or(|r|r.version!=2)||self.bound_location(&state).is_err(){
            return Err("Prepare o ambiente gerido antes de revisar uma transferência local.".into());
        }
        Ok(state.id)
    }

    /// Persistent-only gate for profile activation. No WSL process or probe is
    /// started here; a completed failed install without registration is settled.
    pub fn has_unresolved_maintenance(&self) -> Result<bool, String> {
        if self.archives.stop_pending()? { return Ok(true); }
        let state = self.state()?;
        Ok(match state.phase.as_str() {
            "absent" | "ready" => false,
            "degraded" => state.registration.is_some() || !state.install_exit.as_ref()
                .is_some_and(|exit| exit.confirmed && exit.code.is_some_and(|code| code != 0)),
            _ => true,
        })
    }

    fn with_runner(root: PathBuf, runner: Arc<dyn Runner>) -> Result<Self, String> {
        Self::with_sources(root,runner,Arc::new(OfficialImageSource))
    }

    fn with_sources(root: PathBuf, runner: Arc<dyn Runner>, images:Arc<dyn ImageSource>) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let mut connection = Connection::open(root.join("environment.sqlite3")).map_err(|e| e.to_string())?;
        connection.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
        let supported=|version:i64|if matches!(version,0..=2){Ok(())}else{Err("Versão de registro de ambiente não reconhecida.".to_string())};
        let version=connection.pragma_query_value(None,"user_version",|row|row.get(0)).map_err(|e|e.to_string())?;
        supported(version)?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(|e| e.to_string())?;
        {
            let migration=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
            let version=migration.pragma_query_value(None,"user_version",|row|row.get(0)).map_err(|e|e.to_string())?;
            supported(version)?;
            if version==0 {
                migration.execute_batch("CREATE TABLE IF NOT EXISTS environment_state(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
                    CREATE TABLE IF NOT EXISTS environment_plans(id TEXT PRIMARY KEY, data TEXT NOT NULL);
                    CREATE TABLE IF NOT EXISTS environment_repairs(id TEXT PRIMARY KEY,data TEXT NOT NULL);
                    CREATE TABLE IF NOT EXISTS environment_switches(environment_id TEXT PRIMARY KEY, plan_id TEXT UNIQUE NOT NULL, previous_state TEXT NOT NULL, selected_registration TEXT NOT NULL, activated_at TEXT NOT NULL);
                    PRAGMA user_version=1;").map_err(|e|e.to_string())?;
            }
            if version < 2 { removal::migrate(&migration)?; }
            migration.prepare("SELECT environment_id,plan_id,previous_state,selected_registration,activated_at FROM environment_switches LIMIT 0").map_err(|e|e.to_string())?;
            migration.commit().map_err(|e|e.to_string())?;
        }
        let initial = EnvironmentState { id: uuid::Uuid::new_v4().to_string(), phase: "absent".into(), registration: None,
            last_error: None, last_output: String::new(), updated_at: now(), install_exit:None, host_checkpoint:None };
        connection.execute("INSERT OR IGNORE INTO environment_state(id,data) VALUES(1,?1)", [serde_json::to_string(&initial).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
        let archives = archives::ArchiveStore::open(&root)?;
        let manager = Self { root, archives, connection: Mutex::new(connection), operation: Mutex::new(()), runner, images };
        manager.state()?;
        for mut repair in manager.repairs()? {
            if ["terminating","terminated","auditing"].contains(&repair.status.as_str()) {
                repair.status="unknown".into();repair.error=Some("Recuperação interrompida; nenhum encerramento ou manutenção foi repetido.".into());manager.save_repair(&repair)?;
            }
        }
        Ok(manager)
    }

    fn state(&self) -> Result<EnvironmentState, String> {
        let connection = self.connection.lock().map_err(|_| "Registro de ambiente indisponível.")?;
        let raw: String = connection.query_row("SELECT data FROM environment_state WHERE id=1", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let state: EnvironmentState = serde_json::from_str(&raw).map_err(|_| "Registro de ambiente inválido; nada foi apagado.")?;
        uuid::Uuid::parse_str(&state.id).map_err(|_| "Identidade de ambiente inválida.")?;
        if !["absent", "installing", "bootstrap", "ready", "maintenance", "degraded", "retiring"].contains(&state.phase.as_str()) {
            return Err("Estado de ambiente desconhecido; é necessário diagnóstico.".into());
        }
        Ok(state)
    }

    fn save_state(&self, state: &EnvironmentState) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|_| "Registro indisponível.")?;
        connection.execute("UPDATE environment_state SET data=?1 WHERE id=1", [serde_json::to_string(state).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn wsl(&self, purpose: &'static str, args: Vec<String>, timeout: u64) -> Result<ProcessResult, String> {
        self.runner.run(&CommandSpec { purpose, executable: system_binary("wsl.exe"), args, input: None, timeout: Duration::from_secs(timeout) })
    }

    fn inventory(&self) -> Result<Vec<Registration>, String> {
        let result = self.runner.run(&CommandSpec { purpose: "WSL registry", executable: system_binary("WindowsPowerShell\\v1.0\\powershell.exe"),
            args: vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), REGISTRY.into()], input: None, timeout: Duration::from_secs(15) })?;
        checked(&result)?;
        let items: Vec<Registration> = serde_json::from_str(result.stdout.trim()).map_err(|_| "Não foi possível interpretar o inventário WSL.")?;
        if items.len() > 1000 { return Err("Inventário WSL excedeu o limite.".into()); }
        Ok(items)
    }

    fn bound_location(&self,state:&EnvironmentState)->Result<PathBuf,String>{
        let registration=state.registration.as_ref().ok_or("Ambiente sem registro gerido.")?;
        if registration.name==DISTRO {return Ok(self.install_location());}
        let id=uuid::Uuid::parse_str(&state.id).map_err(|_|"Identidade de recuperação inválida.")?;
        if registration.name==format!("OpenKoda-Recovery-{id}") && id.to_string()==state.id {
            return Ok(self.root.join("kali-recovery").join(&state.id));
        }
        Err("O registro não corresponde a uma distribuição gerida desta instalação.".into())
    }

    fn owned_registration(&self, state: &EnvironmentState) -> Result<Registration, String> {
        let expected = state.registration.as_ref().ok_or("O ambiente ainda não tem propriedade gerida comprovada.")?;
        let path = self.bound_location(state)?;
        let inventory = self.inventory()?;
        let mut matches = inventory.iter().filter(|r|r.name.eq_ignore_ascii_case(&expected.name)||same_path(&r.base_path,&path));
        let current = matches.next().ok_or("A distribuição gerida não foi encontrada.")?;
        if current!=expected || matches.next().is_some() || current.version!=2 || !same_path(&current.base_path,&path) {
            return Err("A identidade, versão ou destino da distribuição mudou. Nenhuma manutenção foi iniciada.".into());
        }
        Ok(current.clone())
    }

    /// Resolve the WSL argv directly before spawning a session bound to this UUID.
    pub(crate) fn launch_distribution(&self, identity:&str)->Result<String,String>{
        self.archives.check_source_stop()?;
        let state=self.state()?;
        if state.phase!="ready" || state.id!=identity {return Err("A sessão pertence a outro ambiente executor. Revise a operação no ambiente ativo.".into())}
        Ok(self.owned_registration(&state)?.name)
    }

    /// Read fixed file metadata as the bound user. Never start the parser or send evidence.
    pub(crate) fn queued_scan_available(&self, identity: &str) -> Result<bool, String> {
        let distro = self.launch_distribution(identity)?;
        const PROBE: &str = r#"import os, pwd, stat, sys
uid = os.geteuid()
marker = os.open('/etc/openkoda/environment-id', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
with os.fdopen(marker, 'rb') as stream:
    if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode) or stream.read(65).strip() != sys.argv[1].encode('ascii'):
        raise ValueError('identity')
if uid == 0 or uid != pwd.getpwnam('openkoda').pw_uid:
    raise ValueError('user')
try:
    info = os.stat('/usr/bin/rg', follow_symlinks=False)
except FileNotFoundError:
    ready = False
else:
    ready = stat.S_ISREG(info.st_mode) and info.st_uid == 0 and not (info.st_mode & 0o6022) and os.access('/usr/bin/rg', os.X_OK) and 0 < info.st_size <= 32 * 1024 * 1024
print('ready' if ready else 'unavailable')"#;
        let args = ["--distribution", &distro, "--user", "openkoda", "--cd", "/", "--exec",
                    "/usr/bin/python3", "-I", "-c", PROBE, identity];
        let output = self.wsl("queued scan prerequisites", args.iter().map(|s| s.to_string()).collect(), 10)?;
        complete_capture(&output)?;
        if output.code == 0 {
            match output.stdout.trim() {
                "ready" => return Ok(true),
                "unavailable" => return Ok(false),
                _ => {}
            }
        }
        Err("Não foi possível verificar o ripgrep no ambiente Kali ativo.".into())
    }
    fn install_location(&self) -> PathBuf { self.root.join("kali-distro") }

    fn helper(&self, state: &EnvironmentState, action: &'static str, extra: Vec<String>, timeout: u64) -> Result<Value, String> {
        let registration=self.owned_registration(state)?;
        // Source and structured arguments travel over the owned process pipe,
        // avoiding Windows command-line length limits for metapackage plans.
        let args = vec!["--distribution".into(), registration.name, "--user".into(), "root".into(), "--cd".into(), "/".into(), "--exec".into(),
            "/usr/bin/python3".into(), "-I".into(), "-c".into(), HELPER_BOOT.into()];
        let wire = json!({"action":action,"identity":state.id,"args":extra,"helper":HELPER});
        let result = self.runner.run(&CommandSpec { purpose: action, executable: system_binary("wsl.exe"), args,
            input: Some(serde_json::to_vec(&wire).map_err(|e| e.to_string())?), timeout:Duration::from_secs(timeout) })?;
        if result.timed_out || result.truncated { return Err("A manutenção perdeu confirmação ou excedeu seus limites. O processo Linux pode continuar; execute diagnóstico antes de qualquer repetição.".into()); }
        let response: Value = serde_json::from_str(result.stdout.trim()).map_err(|error| format!(
            "Resposta inválida do ambiente: saída não é JSON completo (exit={}, stdout={} bytes, stderr={} bytes, categoria={:?}, linha={}, coluna={}).",
            result.code, result.stdout.len(), result.stderr.len(), error.classify(), error.line(), error.column()
        ))?;
        if result.code != 0 || response.get("ok") != Some(&Value::Bool(true)) {
            return Err(response.get("error").and_then(Value::as_str).unwrap_or("Falha de manutenção no ambiente gerido.").to_owned());
        }
        response.get("result").cloned().ok_or_else(|| "Resposta de manutenção incompleta.".into())
    }

    pub fn handle(&self, action: &str, payload: Value) -> Result<Value, String> {
        if !payload.is_object() || payload.to_string().len() > 65536 { return Err("Solicitação de ambiente inválida.".into()); }
        // During a backup, even catalog/diagnose would start Linux and break
        // the confirmed offline interval. Serialize them with maintenance.
        let _operation = self.operation.try_lock().map_err(|_| "Outra operação do ambiente está em andamento.")?;
        if matches!(action, "environment.host.check" | "environment.host.resume") {
            empty(&payload)?;
            return self.host_readiness(action == "environment.host.check");
        }
        if removal::supports(action) { return self.removal_action(action, payload); }
        let state = self.state()?;
        if state.phase == "retiring" {
            if matches!(action, "environment.status" | "environment.diagnose") {
                empty(&payload)?;
                return Ok(json!({"status":"degraded","phase":"retiring","distro":state.registration.map(|r|r.name),"environmentId":state.id,"executorReady":false,"managedRegistration":true,"tools":[],"message":"Retirada em andamento. Consulte Remoção do Kali em Manutenção."}));
            }
            if action != "environment.archive.list" { return Err("Confira a retirada pendente em Manutenção antes de usar o executor.".into()); }
        }
        if archives::supports(action) { return self.archive_action(action, payload); }
        if self.archives.stop_pending()? && matches!(action, "environment.status" | "environment.diagnose") {
            let state=self.state()?;
            return Ok(json!({"status":"degraded","distro":state.registration.as_ref().map_or(DISTRO,|r|r.name.as_str()),"message":"Confira o estado da parada em Manutenção → Cópias do Kali.","tools":[],"phase":"archive_stop_unknown","executorReady":false}));
        }
        self.archives.check_source_stop()?;
        match action {
            "environment.status" | "environment.diagnose" => { empty(&payload)?; self.diagnose() }
            "packages.catalog" => { empty(&payload)?; self.catalog() }
            _ => {
                match action {
                    "environment.plan" => { empty(&payload)?; self.provision_plan() }
                    "environment.provision" => self.provision(decode(payload)?),
                    "environment.image.plan" => self.image_plan(decode(payload)?),
                    "environment.image.provision" => self.image_provision(decode(payload)?),
                    "environment.repair.plan" => { empty(&payload)?; self.repair_plan() }
                    "environment.repair.apply" => self.repair_apply(decode(payload)?),
                    "environment.resume" => { empty(&payload)?; self.resume() }
                    "packages.refresh" => { empty(&payload)?; self.refresh() }
                    "packages.plan" => self.package_plan(decode(payload)?),
                    "packages.apply" => self.apply_packages(decode(payload)?),
                    _ => Err("Ação de ambiente não disponível.".into()),
                }
            }
        }
    }

    fn diagnose(&self) -> Result<Value, String> {
        let state = self.state()?;
        let name=state.registration.as_ref().map_or(DISTRO,|r|r.name.as_str());
        let base = |status: &str, message: String, details: Value| json!({"status":status,"distro":name,"message":message,
            "tools":[],"environmentId":state.id,"phase":state.phase,"updatedAt":state.updated_at,"details":details,
            "executorReady":false,"containment":"WSL gerido; não é sandbox para código hostil","lastError":state.last_error,
            "managedRegistration":state.registration.is_some(),"imageProvisionAvailable":false,"repairAvailable":false});
        let inventory = match self.inventory() {
            Ok(value) => value,
            Err(error) => return Ok(base("unavailable", error, json!({"diagnosticFailed":true}))),
        };
        let found = inventory.iter().find(|r| r.name == name);
        let repair_available=matches!(state.phase.as_str(),"degraded"|"maintenance")
            && found.is_some_and(|current|state.registration.as_ref()==Some(current)&&current.version==2&&self.bound_location(&state).is_ok_and(|p|same_path(&current.base_path,&p)));
        if state.registration.is_none() {
            let mut result=base(if found.is_some() { "degraded" } else { "unavailable" },
                if found.is_some() { "Existe uma distribuição com esse nome sem vínculo gerido. Ela não será adotada ou alterada automaticamente.".into() }
                else { "O Kali dedicado ainda não foi preparado. As distribuições existentes serão preservadas.".into() }, json!({"distributions":inventory}));
            result["imageProvisionAvailable"]=json!(found.is_none()&&!self.install_location().exists()&&image_retryable(&state));
            return Ok(result);
        }
        if ["installing", "bootstrap", "maintenance"].contains(&state.phase.as_str()) {
            let mut result=base("degraded", "Há preparação/manutenção pendente de confirmação. Não iniciar avaliações até reconciliar.".into(), json!({"distributions":inventory}));result["repairAvailable"]=json!(repair_available);result["repairs"]=serde_json::to_value(self.repairs()?).map_err(|e|e.to_string())?;return Ok(result);
        }
        match self.helper(&state, "probe", vec![], 30) {
            Ok(probe) => {
                let healthy = healthy_probe(&probe) && state.phase == "ready";
                let mut result=base(if healthy { "ready" } else { "degraded" },
                    if healthy { "Base Kali preparada. A prontidão do executor e de cada perfil é verificada separadamente.".into() }
                    else { "O ambiente exige diagnóstico: configuração ou manutenção ainda não confirmada.".into() }, probe);result["repairAvailable"]=json!(repair_available);result["repairs"]=serde_json::to_value(self.repairs()?).map_err(|e|e.to_string())?;Ok(result)
            }
            Err(error) => {let mut result=base("degraded",error,json!({"distributions":inventory}));result["repairAvailable"]=json!(repair_available);Ok(result)},
        }
    }

    fn save_plan(&self, kind: &str, payload: Value) -> Result<Value, String> {
        let state = self.state()?;
        let mut plan = Plan { id: uuid::Uuid::new_v4().to_string(), kind: kind.into(), environment_id: state.id,
            created_at: now(), expires_at: (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339(),
            payload, digest: String::new(), consumed: false };
        plan.digest = plan_digest(&plan);
        self.connection.lock().map_err(|_| "Registro indisponível.")?.execute("INSERT INTO environment_plans(id,data) VALUES(?1,?2)",
            params![plan.id, serde_json::to_string(&plan).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
        serde_json::to_value(plan).map_err(|e| e.to_string())
    }

    fn consume_plan(&self, approval: Approval, kind: &str) -> Result<Plan, String> {
        uuid::Uuid::parse_str(&approval.plan_id).map_err(|_| "Plano inválido.")?;
        let state = self.state()?;
        let mut connection = self.connection.lock().map_err(|_| "Registro indisponível.")?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        let raw: String = tx.query_row("SELECT data FROM environment_plans WHERE id=?1", [&approval.plan_id], |row| row.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Plano não encontrado.")?;
        let mut plan: Plan = serde_json::from_str(&raw).map_err(|_| "Plano armazenado inválido.")?;
        if plan.kind != kind || plan.environment_id != state.id || plan.digest != approval.digest || plan.digest != plan_digest(&plan) || plan.consumed {
            return Err("O plano mudou, pertence a outro ambiente ou já foi utilizado. Calcule e revise um novo plano.".into());
        }
        if chrono::DateTime::parse_from_rfc3339(&plan.expires_at).map_err(|_| "Prazo inválido.")? <= chrono::Utc::now() {
            return Err("O plano expirou. Calcule e revise novamente.".into());
        }
        plan.consumed = true;
        tx.execute("UPDATE environment_plans SET data=?1 WHERE id=?2", params![serde_json::to_string(&plan).map_err(|e| e.to_string())?, plan.id]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(plan)
    }

    fn provision_plan(&self) -> Result<Value, String> {
        let state = self.state()?;
        if state.phase != "absent" { return Err("Já existe preparação registrada. Use diagnóstico e retomada; não reinstale por cima.".into()); }
        if self.inventory()?.iter().any(|r| r.name.eq_ignore_ascii_case(DISTRO)) {
            return Err("O nome OpenKoda-Kali já existe e não será adotado automaticamente.".into());
        }
        if self.install_location().exists() { return Err("O destino da distribuição já existe. Preserve os dados e revise a recuperação.".into()); }
        let help = self.wsl("WSL capabilities", vec!["--help".into()], 15)?;
        installation_capabilities(&help)?;
        self.save_plan("provision", json!({"distro":DISTRO,"source":"kali-linux","location":display_path(&self.install_location()),
            "effects":["Baixar Kali pelo catálogo oficial WSL","Criar apenas a nova distribuição OpenKoda-Kali","Criar usuário Linux sem root openkoda","Desabilitar automount e interoperabilidade Windows nessa distribuição","Reiniciar apenas essa distribuição"],
            "provenance":"Aquisição delegada ao catálogo oficial WSL; o OpenKoda não afirma verificação independente da assinatura da imagem.",
            "limits":"Não fornece isolamento forte do host. Downloads e espaço dependem da imagem oficial disponível."}))
    }

    fn provision(&self, approval: Approval) -> Result<Value, String> {
        let plan = self.consume_plan(approval, "provision")?;
        let mut state = self.state()?;
        if state.phase != "absent" || self.install_location().exists() || self.inventory()?.iter().any(|r| r.name.eq_ignore_ascii_case(DISTRO)) {
            return Err("O ambiente ou destino mudou após o cálculo. Nenhuma distribuição foi alterada.".into());
        }
        if plan.payload["location"] != display_path(&self.install_location()) { return Err("Destino do plano divergente.".into()); }
        state.phase = "installing".into(); state.install_exit=Some(InstallExit{confirmed:false,code:None});state.updated_at = now(); self.save_state(&state)?;
        let result = self.wsl("WSL dedicated installation", vec!["--install".into(), "kali-linux".into(), "--name".into(), DISTRO.into(),
            "--location".into(), display_path(&self.install_location()), "--no-launch".into()], 1200);
        match result {
            Ok(output) => {
                state.last_output = safe_tail(&(output.stdout.clone() + &output.stderr));
                state.install_exit=Some(InstallExit{confirmed:!output.timed_out&&!output.truncated,code:if output.timed_out||output.truncated{None}else{Some(output.code)}});self.save_state(&state)?;
                if let Err(error) = checked(&output) { return self.fail(state, error); }
            }
            Err(error) => return self.fail(state, error),
        }
        self.bind_and_bootstrap(state)
    }

    fn image_retry_allowed(&self,state:&EnvironmentState)->Result<(),String>{
        if state.registration.is_some() {return Err("A instalação já possui registro gerido; use diagnóstico e retomada, não outra imagem.".into());}
        if !image_retryable(state){return Err("A instalação anterior não tem término confirmado. Diagnostique antes de solicitar outra aquisição; nenhuma repetição automática será feita.".into());}
        if self.install_location().exists(){return Err("O destino dedicado já existe; a imagem não será instalada sobre dados anteriores.".into());}
        if self.inventory()?.iter().any(|r|r.name.eq_ignore_ascii_case(DISTRO)){return Err("Já existe distribuição OpenKoda-Kali; ela não será adotada nem substituída.".into());}
        let process=self.runner.run(&CommandSpec{purpose:"WSL installation process check",executable:system_binary("WindowsPowerShell\\v1.0\\powershell.exe"),args:vec!["-NoProfile".into(),"-NonInteractive".into(),"-Command".into(),"$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $ids=@(Get-Process -Name wsl -ErrorAction SilentlyContinue | ForEach-Object { [int]$_.Id }); ConvertTo-Json -InputObject $ids -Compress".into()],input:None,timeout:Duration::from_secs(15)})?;
        checked(&process)?;let ids:Vec<u32>=serde_json::from_str(process.stdout.trim()).map_err(|_|"Não foi possível confirmar ausência de clientes WSL ativos.".to_string())?;
        if !ids.is_empty(){return Err("Há cliente wsl.exe em execução. Aguarde sua conclusão antes desta recuperação; nenhum processo ou distro pessoal foi encerrado.".into());}Ok(())
    }

    fn image_plan(&self,input:ImageSelection)->Result<Value,String>{
        let state=self.state()?;self.image_retry_allowed(&state)?;
        let help=self.wsl("WSL image capabilities",vec!["--help".into()],15)?;image_installation_capabilities(&help)?;
        let image=self.images.prepare(&self.root,input.local_file.as_deref())?;
        let _verified=crate::official_image::verify_pinned(&self.root,&image)?;
        self.image_retry_allowed(&self.state()?)?;
        self.save_plan("provision-image",json!({"distro":DISTRO,"location":display_path(&self.install_location()),"image":image,
            "effects":["Instalar somente OpenKoda-Kali a partir de imagem .wsl conferida pelo SHA-256 do catálogo Microsoft","Criar usuário openkoda sem root","Desabilitar automount e interoperabilidade Windows nessa distribuição","Reiniciar somente OpenKoda-Kali"],
            "provenance":"Imagem Kali amd64 do manifest WSL oficial Microsoft; SHA-256 verificado no cache gerido antes da aprovação e do despacho.",
            "limits":"Não fornece isolamento forte do host; assinatura independente do publicador não é declarada pelo hash."}))
    }

    fn image_provision(&self,approval:Approval)->Result<Value,String>{
        let plan=self.consume_plan(approval,"provision-image")?;let mut state=self.state()?;self.image_retry_allowed(&state)?;
        if plan.payload["distro"]!=DISTRO||plan.payload["location"]!=display_path(&self.install_location()){return Err("Destino da imagem diverge da instalação gerida.".into());}
        let image:VerifiedImage=serde_json::from_value(plan.payload["image"].clone()).map_err(|_|"Plano de imagem inválido.".to_string())?;
        // This handle denies write/delete sharing on Windows until --install
        // returns, closing the content replacement race after approval.
        let _image_guard=crate::official_image::verify_pinned(&self.root,&image)?;
        state.phase="installing".into();state.install_exit=Some(InstallExit{confirmed:false,code:None});state.updated_at=now();self.save_state(&state)?;
        let outcome=self.wsl("WSL verified image installation",vec!["--install".into(),"--from-file".into(),display_path(Path::new(&image.local_file)),"--name".into(),DISTRO.into(),"--location".into(),display_path(&self.install_location()),"--no-launch".into()],1200);
        match outcome {
            Ok(output)=>{state.last_output=safe_tail(&(output.stdout.clone()+&output.stderr));state.install_exit=Some(InstallExit{confirmed:!output.timed_out&&!output.truncated,code:if output.timed_out||output.truncated{None}else{Some(output.code)}});self.save_state(&state)?;if let Err(error)=checked(&output){return self.fail(state,error);}},
            Err(error)=>return self.fail(state,error),
        }
        self.bind_and_bootstrap(state)
    }

    fn bind_and_bootstrap(&self, mut state: EnvironmentState) -> Result<Value, String> {
        let result = (|| {
            let registration = self.inventory()?.into_iter().find(|r| r.name == DISTRO).ok_or("A instalação não registrou a distribuição dedicada. Retome após verificar WSL/reboot.")?;
            if registration.version != 2 || !same_path(&registration.base_path, &self.install_location()) {
                return Err("Registro WSL não corresponde ao destino dedicado ou não é WSL 2.".into());
            }
            if state.registration.as_ref().is_some_and(|old| old != &registration) {
                return Err("Registro gerido mudou. Retomada automática recusada.".into());
            }
            state.registration = Some(registration); state.phase = "bootstrap".into(); state.updated_at = now(); self.save_state(&state)?;
            self.helper(&state, "bootstrap", vec![], 90)?;
            let stopped = self.wsl("WSL dedicated restart", vec!["--terminate".into(), DISTRO.into()], 30)?;
            checked(&stopped)?;
            let probe = self.helper(&state, "probe", vec![], 45)?;
            if !healthy_probe(&probe) {
                return Err("A configuração efetiva não confirmou usuário comum e integração Windows desabilitada.".into());
            }
            state.phase = "ready".into(); state.last_error = None; state.updated_at = now(); self.save_state(&state)?;
            self.diagnose()
        })();
        match result { Ok(value) => Ok(value), Err(error) => self.fail(state, error) }
    }

    fn fail(&self, mut state: EnvironmentState, error: String) -> Result<Value, String> {
        state.phase = "degraded".into(); state.last_error = Some(error.clone()); state.updated_at = now(); self.save_state(&state)?;
        Err(error)
    }

    fn repairs(&self)->Result<Vec<RepairAttempt>,String>{
        let connection=self.connection.lock().map_err(|_|"Registro de recuperação indisponível.")?;
        let mut statement=connection.prepare("SELECT data FROM environment_repairs ORDER BY rowid DESC LIMIT 100").map_err(|e|e.to_string())?;
        let rows=statement.query_map([],|row|row.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        rows.into_iter().map(|raw|serde_json::from_str(&raw).map_err(|_|"Registro de recuperação inválido.".into())).collect()
    }

    fn save_repair(&self,repair:&RepairAttempt)->Result<(),String>{
        self.connection.lock().map_err(|_|"Registro de recuperação indisponível.")?.execute("INSERT INTO environment_repairs(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![repair.id,serde_json::to_string(repair).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(())
    }

    fn repair_plan(&self)->Result<Value,String>{
        let state=self.state()?;
        if !matches!(state.phase.as_str(),"degraded"|"maintenance"){return Err("Recuperação com encerramento só está disponível para manutenção pendente.".into());}
        let registration=state.registration.clone().ok_or("Recuperação exige distribuição gerida já vinculada.")?;
        let probe=self.helper(&state,"probe",vec![],30)?;
        let (sha,journal)=repair_snapshot(&probe)?;
        self.save_plan("environment-repair",json!({"distro":registration.name,"environmentId":state.id,"registration":registration,
            "stateSha256":digest(&serde_json::to_value(&state).map_err(|e|e.to_string())?),"maintenanceSha256":sha,"maintenance":journal,
            "effects":["Encerrar somente a distribuição gerida identificada nesta revisão, incluindo processos pendentes nela","Confirmar parada pela lista de distribuições em execução e reconferir GUID/destino","Preservar o journal original, reiniciar o helper gerido e auditar locks APT/dpkg e integridade de pacotes"],
            "limits":["Não encerra outras distribuições e não executa shutdown global","Não repete apt update/install nem executa reparo de pacotes automaticamente","Pode interromper manutenção ainda ativa na distribuição revisada; estado parcial exige revisão","ACK incerto mantém o ambiente bloqueado; uma nova tentativa exige um novo plano","Parada comprovada não declara que a atualização de índices foi bem-sucedida"]}))
    }

    fn repair_apply(&self,approval:Approval)->Result<Value,String>{
        let plan=self.consume_plan(approval,"environment-repair")?;let mut state=self.state()?;
        let registration=state.registration.clone().ok_or("Ambiente sem vínculo gerido.")?;
        if !matches!(state.phase.as_str(),"degraded"|"maintenance")||plan.payload["distro"]!=registration.name||plan.payload["environmentId"]!=state.id
            ||plan.payload["registration"]!=serde_json::to_value(&registration).map_err(|e|e.to_string())?
            ||plan.payload["stateSha256"]!=digest(&serde_json::to_value(&state).map_err(|e|e.to_string())?){return Err("Estado ou identidade mudou desde a revisão. Calcule outro plano; nada foi encerrado.".into());}
        let probe=self.helper(&state,"probe",vec![],30)?;let (sha,_)=repair_snapshot(&probe)?;
        if plan.payload["maintenanceSha256"]!=sha{return Err("O journal mudou desde a revisão. Nenhum processo foi encerrado; revise um novo plano.".into());}
        self.owned_registration(&state)?;
        let mut repair=RepairAttempt{id:uuid::Uuid::new_v4().to_string(),plan_id:plan.id,environment_id:state.id.clone(),registration:registration.clone(),
            maintenance_sha256:sha.clone(),status:"terminating".into(),created_at:now(),finished_at:None,termination_receipt:None,error:None};
        self.save_repair(&repair)?;
        state.phase="maintenance".into();state.updated_at=now();self.save_state(&state)?;
        let outcome=(||{
            let termination=self.wsl("WSL repair termination",vec!["--terminate".into(),registration.name.clone()],30)?;
            state.last_output=safe_tail(&(termination.stdout.clone()+&termination.stderr));checked(&termination)?;
            let inventory=self.inventory()?;
            if !inventory.iter().any(|current|current==&repair.registration&&self.bound_location(&state).is_ok_and(|p|same_path(&current.base_path,&p))){return Err("Registro mudou após o encerramento. Nenhum helper de recuperação foi iniciado.".into());}
            let running=self.wsl("WSL repair running inventory",vec!["--list".into(),"--running".into(),"--quiet".into()],30)?;
            require_stopped_name(&running,&inventory,&registration.name)?;self.owned_registration(&state)?;
            let receipt=json!({"repairId":repair.id,"planId":repair.plan_id,"environmentId":state.id,"distro":registration.name,
                "maintenanceSha256":sha,"terminationVerified":true,"verifiedAtUnixSeconds":chrono::Utc::now().timestamp()});
            repair.status="terminated".into();repair.termination_receipt=Some(receipt.clone());self.save_repair(&repair)?;
            repair.status="auditing".into();self.save_repair(&repair)?;
            let audit=self.helper(&state,"repair",vec![receipt.to_string()],90)?;
            if audit["clean"]!=true||audit["originalSha256"]!=sha||audit["repairId"]!=repair.id{return Err("Auditoria não confirmou o recibo e o journal revisados; ambiente segue bloqueado.".into());}
            let probe=self.helper(&state,"probe",vec![],30)?;
            if !healthy_probe(&probe){return Err("A base Kali não confirmou configuração e manutenção reconciliadas após a recuperação.".into());}
            Ok::<_,String>((audit,probe))
        })();
        match outcome {
            Ok((audit,probe))=>{
                repair.status="completed".into();repair.finished_at=Some(now());self.save_repair(&repair)?;
                state.phase="ready".into();state.last_error=None;state.updated_at=now();self.save_state(&state)?;
                Ok(json!({"status":"ready","phase":"ready","distro":registration.name,"environmentId":state.id,"managedRegistration":true,
                    "repairAvailable":false,"imageProvisionAvailable":false,"executorReady":false,"tools":[],"details":probe,"repair":repair,"repairAudit":audit,
                    "message":"Parada e auditoria confirmadas. Nenhuma atualização ou instalação foi repetida; confira o resultado da manutenção original.","containment":"WSL gerido; não é sandbox para código hostil"}))
            },
            Err(error)=>{repair.status="unknown".into();repair.error=Some(error.clone());repair.finished_at=Some(now());self.save_repair(&repair)?;self.fail(state,error)},
        }
    }

    fn resume(&self) -> Result<Value, String> {
        let mut state = self.state()?;
        if state.phase == "absent" { return Err("Calcule e revise um plano de preparação antes de instalar.".into()); }
        if state.registration.is_none() || state.phase == "bootstrap" {
            // Only a journaled installation intent can bind this location. Never
            // restart a download on missing ACK; inspect existing registration.
            return self.bind_and_bootstrap(state);
        }
        let audit = self.helper(&state, "reconcile", vec![], 60)?;
        if audit["clean"] != true { return self.fail(state, format!("dpkg exige reparo: {}", audit["output"].as_str().unwrap_or("estado incompleto"))); }
        state.phase = "ready".into(); state.last_error = None; state.updated_at = now(); self.save_state(&state)?;
        self.diagnose()
    }

    fn require_ready(&self) -> Result<EnvironmentState, String> {
        self.archives.check_source_stop()?;
        let state = self.state()?;
        if state.phase != "ready" { return Err("O ambiente precisa estar preparado e reconciliado antes da manutenção de pacotes.".into()); }
        let probe = self.helper(&state, "probe", vec![], 30)?;
        if !healthy_probe(&probe) { return Err("O ambiente não confirmou usuário comum, integração Windows desabilitada e manutenção reconciliada.".into()); }
        Ok(state)
    }

    fn catalog(&self) -> Result<Value, String> {
        let state = self.state()?;
        let known = catalog_entries();
        let observed = if state.phase == "ready" { self.helper(&state, "catalog", known.iter().map(|p| p["package"].as_str().unwrap().to_owned()).collect(), 90).ok() } else { None };
        let tools = known.into_iter().map(|mut item| {
            let observation = observed.as_ref().and_then(|o| o["packages"].as_array()).and_then(|items| items.iter().find(|p| p["package"] == item["package"]));
            item["installed"] = observation.map(|p| p["installed"].clone()).unwrap_or(Value::Null);
            item["version"] = observation.map(|p| p["version"].clone()).unwrap_or(Value::Null);
            item
        }).collect::<Vec<_>>();
        Ok(json!({"version":"1","tools":tools,"inventoryVerified":observed.is_some(),"distro":state.registration.as_ref().map_or(DISTRO,|r|r.name.as_str())}))
    }

    fn refresh(&self) -> Result<Value, String> {
        let mut state = self.require_ready()?;
        state.phase = "maintenance".into(); state.updated_at = now(); self.save_state(&state)?;
        match self.helper(&state, "refresh", vec![], 645) {
            Ok(result) => { state.phase = "ready".into(); state.last_error = None; state.updated_at = now(); self.save_state(&state)?; Ok(result) }
            Err(error) => self.fail(state, error),
        }
    }

    fn package_plan(&self, input: PackageSelection) -> Result<Value, String> {
        let state = self.require_ready()?;
        let packages = validate_packages(input.packages)?;
        let simulation = self.helper(&state, "simulate", packages.clone(), 150)?;
        self.save_plan("packages", json!({"packages":packages,"resolved":simulation["resolved"],"fingerprint":simulation["fingerprint"],
            "output":simulation["output"],"effects":["Instalar os pacotes e dependências fixados na simulação como root Linux","Preservar configurações locais existentes","Bloquear avaliações durante manutenção"],
            "source":"Kali oficial com signed-by kali-archive-keyring.gpg; nenhum repositório adicional"}))
    }

    fn apply_packages(&self, approval: Approval) -> Result<Value, String> {
        let mut state = self.require_ready()?;
        let plan = self.consume_plan(approval, "packages")?;
        let arguments = json!({"id":plan.id,"packages":plan.payload["packages"],"resolved":plan.payload["resolved"],"fingerprint":plan.payload["fingerprint"]});
        state.phase = "maintenance".into(); state.updated_at = now(); self.save_state(&state)?;
        match self.helper(&state, "apply", vec![arguments.to_string()], 945) {
            Ok(result) => { state.phase = "ready".into(); state.last_error = None; state.updated_at = now(); self.save_state(&state)?; Ok(result) }
            Err(error) => self.fail(state, error),
        }
    }
}

fn complete_capture(result: &ProcessResult) -> Result<(), String> {
    if result.timed_out { return Err("O WSL não confirmou o término no prazo. O estado permanece pendente de diagnóstico; não repetir a operação automaticamente.".into()); }
    if result.truncated { return Err("A captura do diagnóstico ficou incompleta ou excedeu o limite; não foi considerada completa.".into()); }
    Ok(())
}
fn repair_snapshot(probe:&Value)->Result<(String,Value),String>{
    let sha=probe["maintenanceSha256"].as_str().ok_or("Journal não tem SHA-256 verificável para recuperação.")?;
    let journal=&probe["maintenance"];
    if sha.len()!=64||!sha.bytes().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase())
        ||!matches!(journal["action"].as_str(),Some("refresh"|"install"))||!matches!(journal["status"].as_str(),Some("running"|"unknown")){
        return Err("O journal não descreve manutenção interrompida recuperável. Use diagnóstico/retomada para estado já reconciliado.".into());
    }Ok((sha.into(),journal.clone()))
}
#[cfg(test)]
fn require_stopped(result:&ProcessResult,inventory:&[Registration])->Result<(),String>{require_stopped_name(result,inventory,DISTRO)}
fn require_stopped_name(result:&ProcessResult,inventory:&[Registration],name:&str)->Result<(),String>{
    checked(result)?;
    if !result.stderr.trim().is_empty(){return Err("A lista WSL contém diagnóstico inesperado; parada não confirmada.".into());}
    let names=result.stdout.lines().map(str::trim).filter(|name|!name.is_empty()).collect::<Vec<_>>();
    if names.len()>1000||names.iter().any(|name|name.chars().any(char::is_control)||!inventory.iter().any(|r|r.name==*name)){
        return Err("Não foi possível interpretar a lista completa de distribuições em execução.".into());
    }
    if names.iter().any(|v|v.eq_ignore_ascii_case(name)){return Err("A distribuição revisada ainda consta em execução; recuperação não será despachada.".into());}Ok(())
}
fn image_retryable(state:&EnvironmentState)->bool{
    if state.registration.is_some(){return false;}
    let completed_failure=state.install_exit.as_ref().is_some_and(|exit|exit.confirmed&&exit.code.is_some_and(|code|code!=0));
    // Only checked() emitted this legacy prefix after complete_capture() passed.
    // Supervisor timeout/unknown never emitted a numeric completed-exit prefix.
    let legacy_completed_failure=state.install_exit.is_none()&&state.last_error.as_deref().and_then(|message|message.strip_prefix("WSL retornou erro ")).and_then(|message|message.split_once(':')).and_then(|(code,_)|code.parse::<i32>().ok()).is_some_and(|code|code!=0)&&!state.last_output.is_empty();
    state.phase=="absent"||(state.phase=="degraded"&&(completed_failure||legacy_completed_failure))
}
fn checked(result: &ProcessResult) -> Result<(), String> {
    complete_capture(result)?;
    if result.code != 0 { return Err(format!("WSL retornou erro {}: {}", result.code, safe_tail(&(result.stderr.clone() + &result.stdout)))); }
    Ok(())
}
fn healthy_probe(probe: &Value) -> bool {
    let maintenance_complete = match probe.get("maintenance") {
        // The first bootstrap has no package journal. Missing/malformed fields
        // are not equivalent to an explicit assertion of no prior maintenance.
        Some(Value::Null) => true,
        Some(Value::Object(operation)) => operation.get("status").and_then(Value::as_str) == Some("completed")
            || (operation.get("status").and_then(Value::as_str) == Some("reconciled")
                && operation.get("action").and_then(Value::as_str) == Some("refresh")
                && operation.get("quiescent") == Some(&Value::Bool(true))
                && operation.get("refreshSucceeded") == Some(&Value::Bool(false))),
        _ => false,
    };
    probe["uid"].as_u64().is_some_and(|uid| uid > 0)
        && probe["interopEnabled"] == false
        && probe["windowsMounts"].as_array().is_some_and(|mounts| mounts.is_empty())
        && maintenance_complete
}
fn installation_capabilities(result: &ProcessResult) -> Result<(), String> {
    // WSL versions on real Windows hosts can return -1 for a complete --help.
    // Only this read-only capability query uses output as its success contract;
    // installation, termination, registry and Linux helpers still require 0.
    complete_capture(result)?;
    let text = format!("{}\n{}", result.stdout, result.stderr);
    let lines: Vec<&str> = text.lines().collect();
    let option = |line: &str| line.trim_start().split(|ch: char| ch.is_whitespace() || ch == ',').next().unwrap_or("").to_owned();
    if ["--help", "--exec", "--distribution", "--user", "--list", "--terminate", "--unregister"].iter()
        .any(|flag| !lines.iter().any(|line| option(line) == *flag)) {
        return Err("A ajuda do WSL não confirmou um inventário completo de capacidades. Verifique o diagnóstico antes de preparar.".into());
    }
    let start = lines.iter().position(|line| option(line) == "--install")
        .ok_or("A ajuda do WSL não confirmou a instalação de distribuições.")?;
    let indent = lines[start].len() - lines[start].trim_start().len();
    let install_options: Vec<String> = lines[start + 1..].iter().take_while(|line| {
        let trimmed = line.trim_start();
        !(trimmed.starts_with("--") && line.len() - trimmed.len() <= indent)
    }).map(|line| option(line)).collect();
    if ["--name", "--location", "--no-launch"].iter().any(|flag| !install_options.iter().any(|option| option == flag)) {
        return Err("Esta versão do WSL não oferece instalação dedicada por nome e destino. Atualize o WSL antes de preparar.".into());
    }
    Ok(())
}
fn image_installation_capabilities(result:&ProcessResult)->Result<(),String>{
    installation_capabilities(result)?;
    let text=format!("{}\n{}",result.stdout,result.stderr);let lines=text.lines().collect::<Vec<_>>();
    let option=|line:&str|line.trim_start().split(|c:char|c.is_whitespace()||c==',').next().unwrap_or("").to_owned();
    let start=lines.iter().position(|line|option(line)=="--install").ok_or("Instalação WSL indisponível.")?;
    let indent=lines[start].len()-lines[start].trim_start().len();
    if !lines[start+1..].iter().take_while(|line|{let trimmed=line.trim_start();!(trimmed.starts_with("--")&&line.len()-trimmed.len()<=indent)}).any(|line|option(line)=="--from-file"){return Err("Esta versão do WSL não confirmou --install --from-file; nenhuma importação alternativa foi executada.".into());}Ok(())
}
fn safe_tail(value: &str) -> String { value.chars().rev().take(12000).collect::<String>().chars().rev().collect() }
fn display_path(path: &Path) -> String { path.to_string_lossy().trim_start_matches("\\\\?\\").to_owned() }
fn same_path(value: &str, expected: &Path) -> bool {
    let normalize = |value: &str| value.trim_start_matches("\\\\?\\").replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase();
    if normalize(value) != normalize(&display_path(expected)) { return false; }
    // Existing path must resolve exactly; a junction cannot redirect ownership.
    match (Path::new(value).canonicalize(), expected.canonicalize()) {
        (Ok(a), Ok(b)) => a == b && normalize(&display_path(&a)) == normalize(&display_path(expected)),
        _ => false,
    }
}
fn plan_digest(plan: &Plan) -> String {
    digest(&json!({"id":plan.id,"kind":plan.kind,"environmentId":plan.environment_id,"createdAt":plan.created_at,"expiresAt":plan.expires_at,"payload":plan.payload}))
}
fn catalog_entries() -> Vec<Value> {
    [
        ("curl", "Web e API", "Cliente HTTP", "typed-http"),
        ("jq", "Arquivos", "Processamento JSON", "offline-json"),
        ("ripgrep", "Código", "Sinais estáticos em evidência selecionada", "kali.rg.signals.v1"),
        ("file", "Arquivos", "Identificação de formatos", "offline-file"),
        ("binutils", "Arquivos", "Análise estática de binários", "not-integrated"),
        ("nmap", "Rede", "Inventário de rede autorizado", "not-integrated"),
        ("tshark", "Arquivos", "Leitura de capturas", "not-integrated"),
        ("openssl", "Web e API", "Inspeção de certificados", "not-integrated"),
        ("python3", "Ambiente", "Runtime do executor", "runtime"),
        ("kali-tools-web", "Conjuntos", "Ferramentas Web Kali", "not-integrated"),
        ("kali-tools-information-gathering", "Conjuntos", "Coleta de informações", "not-integrated"),
        ("kali-tools-forensics", "Conjuntos", "Análise forense", "not-integrated"),
    ].into_iter().map(|(package, group, description, adapter)| json!({"package":package,"name":package,"group":group,"description":description,"adapter":adapter,
        "adapterImplemented":package=="ripgrep","executionReady":false,"availability":if package=="ripgrep"{"Adaptador de sinais disponível na avaliação. Identidade, instalação e limites são verificados antes de analisar a evidência selecionada."}else{"Instalação não implica compatibilidade ou autorização para executar"}})).collect()
}
fn validate_packages(mut packages: Vec<String>) -> Result<Vec<String>, String> {
    let catalog = catalog_entries();
    if packages.is_empty() || packages.len() > 12 || packages.iter().any(|name| !catalog.iter().any(|p| p["package"] == *name)) {
        return Err("Selecione apenas pacotes do catálogo conhecido (até 12).".into());
    }
    packages.sort(); packages.dedup(); Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct FakeRunner { outputs: Mutex<VecDeque<(&'static str, ProcessResult)>>, calls: Mutex<Vec<CommandSpec>> }
    impl Runner for FakeRunner {
        fn run(&self, command: &CommandSpec) -> Result<ProcessResult, String> {
            self.calls.lock().unwrap().push(command.clone());
            let (purpose, mut output) = self.outputs.lock().unwrap().pop_front().expect("unexpected subprocess");
            assert_eq!(purpose, command.purpose);
            if purpose=="repair"&&output.stdout.contains("echo-receipt") {
                let wire:Value=serde_json::from_slice(command.input.as_ref().unwrap()).unwrap();let receipt:Value=serde_json::from_str(wire["args"][0].as_str().unwrap()).unwrap();
                let mut response:Value=serde_json::from_str(&output.stdout).unwrap();response["result"]["repairId"]=receipt["repairId"].clone();output.stdout=response.to_string();
            }
            if purpose=="WSL dedicated installation" && output.code==0 && !output.timed_out {
                let position=command.args.iter().position(|value|value=="--location").unwrap();
                fs::create_dir_all(&command.args[position+1]).unwrap();
            }
            Ok(output)
        }
    }
    fn success(value: impl Into<String>) -> ProcessResult { ProcessResult { code:0, stdout:value.into(), stderr:String::new(), timed_out:false, truncated:false } }
    fn fake(outputs: Vec<(&'static str, ProcessResult)>) -> Arc<FakeRunner> { Arc::new(FakeRunner { outputs:Mutex::new(outputs.into()), calls:Mutex::new(vec![]) }) }
    // Option outline from the complete Portuguese help observed on Windows,
    // whose process exit code was -1. Capability parsing is locale independent.
    const WSL_HELP: &str = "Uso: wsl.exe\n    --exec, -e <CommandLine>\n    --distribution, -d <DistroName>\n    --user, -u <UserName>\n    --help\n    --install [Distribuição] [Opções...]\n        Opções:\n            --location <Location>\n            --name <Name>\n            --no-launch, -n\n    --mount <Disk>\n            --name <Name>\n    --list, -l [Opções]\n    --terminate, -t <Distro>\n    --unregister <Distro>\n";
    #[test]
    fn guided_host_preparation_reopens_checkpoint_and_rechecks_windows_without_installing() {
        let dir = tempfile::tempdir().unwrap();
        let before = json!({"windowsBuild":22631,"architecture":"x64","hypervisor":true,
            "virtualMachinePlatform":false,"wslClient":false,"freeBytes":1024,"boot":"first-boot"});
        let runner = fake(vec![("Windows prerequisites",success(before.to_string())),("WSL registry",success("[]"))]);
        let manager = EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        assert!(manager.handle("environment.host.resume",json!({})).unwrap().is_null());
        assert!(manager.handle("environment.host.check",json!({"shell":"bad"})).is_err());
        assert!(runner.calls.lock().unwrap().is_empty());
        let first = manager.handle("environment.host.check",json!({})).unwrap();
        assert_eq!(first["step"],"prepare_wsl");
        assert_eq!(first["rebootObserved"],false);
        assert_eq!(manager.state().unwrap().phase,"absent");
        drop(manager);

        let mut after = before;
        after["boot"] = json!("next-boot");
        after["virtualMachinePlatform"] = json!(true);
        after["wslClient"] = json!(true);
        let pending = ProcessResult {timed_out:true,..success("incomplete")};
        let runner = fake(vec![
            ("Windows prerequisites",success(after.to_string())),("WSL host status",pending),
            ("WSL host capabilities",success(WSL_HELP)),("WSL host version",success("Versão do WSL: 2.6.2.0")),
            ("WSL registry",success("[]")),
            ("Windows prerequisites",success(after.to_string())),("WSL host status",success("Ok")),
            ("WSL host capabilities",success(WSL_HELP)),("WSL host version",success("WSL version: 2.6.2.0")),
            ("WSL registry",success("[]")),
        ]);
        let reopened = EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let incomplete = reopened.handle("environment.host.resume",json!({})).unwrap();
        assert_eq!(incomplete["startedAt"],first["startedAt"]);
        assert_eq!(incomplete["rebootObserved"],true);
        assert_eq!(incomplete["step"],"inspect");
        let ready = reopened.handle("environment.host.check",json!({})).unwrap();
        assert_eq!(ready["startedAt"],first["startedAt"]);
        assert_eq!(ready["step"],"review_kali");
        assert_eq!(ready["wslVersion"],"2.6.2.0");
        for probe in runner.calls.lock().unwrap().iter().filter(|call| call.executable.ends_with("wsl.exe")) {
            assert_eq!(probe.args.len(),1);
            assert!(matches!(probe.args[0].as_str(),"--status"|"--help"|"--version"));
        }
        assert_eq!(reopened.state().unwrap().phase,"absent");
        assert!(!reopened.install_location().exists());
    }

    #[test]
    fn complete_wsl_help_with_exit_minus_one_allows_only_a_reviewable_plan() {
        let dir=tempfile::tempdir().unwrap();
        let mut help=success(WSL_HELP);help.code=-1;
        assert!(checked(&help).is_err()); // Other operations retain strict exit handling.
        let runner=fake(vec![("WSL registry",success("[]")),("WSL capabilities",help)]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let plan=manager.handle("environment.plan",json!({})).unwrap();
        assert_eq!(plan["kind"],"provision");assert_eq!(plan["consumed"],false);
        assert_eq!(manager.state().unwrap().phase,"absent");
        assert_eq!(runner.calls.lock().unwrap().len(),2);
        assert!(!manager.install_location().exists());
    }
    #[test]
    fn incomplete_help_and_flags_outside_install_cannot_enable_provisioning() {
        let mut help=success(WSL_HELP);help.code=-1;
        help.timed_out=true;assert!(installation_capabilities(&help).is_err());
        help.timed_out=false;help.truncated=true;assert!(installation_capabilities(&help).is_err());
        help.truncated=false;help.stdout=WSL_HELP.replace("            --name <Name>\n            --no-launch", "            --no-launch");
        assert!(help.stdout.contains("--name")); // The mount flag cannot authorize install --name.
        assert!(installation_capabilities(&help).is_err());
        help.stdout="Access denied --name --location --no-launch".into();assert!(installation_capabilities(&help).is_err());
        help.stdout=WSL_HELP.replace("    --unregister <Distro>\n", "");assert!(installation_capabilities(&help).is_err());
    }
    #[test]
    fn executor_identity_requires_ready_before_any_linux_dispatch() {
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        for phase in ["absent","installing","bootstrap","maintenance","degraded"] {
            let mut state=manager.state().unwrap();state.phase=phase.into();manager.save_state(&state).unwrap();
            assert!(manager.executor_identity().is_err());
            assert!(manager.executor_owned_identity().is_err());
        }
        assert!(runner.calls.lock().unwrap().is_empty());
    }
    #[test]
    fn executor_identity_returns_only_verified_managed_identity() {
        for maintenance in [Value::Null,json!({"status":"completed"})] {
            let dir=tempfile::tempdir().unwrap();let location=dir.path().canonicalize().unwrap().join("kali-distro");fs::create_dir(&location).unwrap();
            let registration=Registration {guid:"managed-guid".into(),name:DISTRO.into(),base_path:display_path(&location),version:2};
            let probe=json!({"ok":true,"result":{"uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenance":maintenance}});
            let runner=fake(vec![("WSL registry",success(serde_json::to_string(&vec![registration.clone()]).unwrap())),("probe",success(probe.to_string()))]);
            let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
            let mut state=manager.state().unwrap();state.phase="ready".into();state.registration=Some(registration);manager.save_state(&state).unwrap();
            assert_eq!(manager.executor_identity().unwrap(),state.id);
            assert_eq!(runner.calls.lock().unwrap().last().unwrap().purpose,"probe");
        }
    }
    #[test]
    fn executor_owned_identity_checks_registration_without_starting_linux() {
        for replaced in [false,true] {
            let dir=tempfile::tempdir().unwrap();let location=dir.path().canonicalize().unwrap().join("kali-distro");fs::create_dir(&location).unwrap();
            let registration=Registration{guid:"owned-registration".into(),name:DISTRO.into(),base_path:display_path(&location),version:2};
            let mut observed=registration.clone();if replaced{observed.guid="replacement".into();}
            let runner=fake(vec![("WSL registry",success(serde_json::to_string(&vec![observed]).unwrap()))]);
            let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
            let mut state=manager.state().unwrap();state.phase="ready".into();state.registration=Some(registration);manager.save_state(&state).unwrap();
            let result=manager.executor_owned_identity();if replaced{assert!(result.is_err());}else{assert_eq!(result.unwrap(),state.id);}
            let calls=runner.calls.lock().unwrap();assert_eq!(calls.len(),1);assert_eq!(calls[0].purpose,"WSL registry");
            assert!(!calls[0].executable.to_string_lossy().to_ascii_lowercase().ends_with("wsl.exe"));
        }
    }
    #[test]
    fn queued_scan_checks_the_bound_clone_and_accepts_only_confirmed_availability() {
        for (reply,expected) in [("unavailable",Some(false)),("ready",Some(true)),("unexpected",None)] {
            let dir=tempfile::tempdir().unwrap();
            let id=uuid::Uuid::new_v4().to_string();
            let path=dir.path().canonicalize().unwrap().join("kali-recovery").join(&id);
            fs::create_dir_all(&path).unwrap();
            let registration=Registration{guid:"fixture-clone".into(),name:format!("OpenKoda-Recovery-{id}"),base_path:display_path(&path),version:2};
            let runner=fake(vec![("WSL registry",success(json!([registration]).to_string())),("queued scan prerequisites",success(reply))]);
            let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
            let mut state=manager.state().unwrap();state.id=id.clone();state.phase="ready".into();state.registration=Some(registration.clone());manager.save_state(&state).unwrap();
            assert_eq!(manager.queued_scan_available(&id).ok(),expected);
            let calls=runner.calls.lock().unwrap();assert_eq!(calls.len(),2);
            assert_eq!(calls[1].args[..10],["--distribution",&registration.name,"--user","openkoda","--cd","/","--exec","/usr/bin/python3","-I","-c"]);
            assert_eq!(calls[1].args.last(),Some(&id));assert!(calls[1].input.is_none());
        }
    }
    #[test]
    fn effective_probe_rejects_root_mounts_interop_and_unconfirmed_maintenance() {
        let healthy=json!({"uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenance":null});
        assert!(healthy_probe(&healthy));
        let mut missing=healthy.clone();missing.as_object_mut().unwrap().remove("maintenance");assert!(!healthy_probe(&missing));
        for (field,value) in [("uid",json!(0)),("uid",json!(null)),("interopEnabled",json!(true)),("interopEnabled",json!(null)),("windowsMounts",json!(["/mnt/c"])),("windowsMounts",json!(null)),("maintenance",json!({})),("maintenance",json!({"status":"running"})),("maintenance",json!({"status":"unknown"})),("maintenance",json!(false))] {
            let mut probe=healthy.clone();probe[field]=value;assert!(!healthy_probe(&probe),"accepted {field}: {probe}");
        }
    }
    #[test]
    fn executor_identity_refuses_replaced_registration_before_probe() {
        let dir=tempfile::tempdir().unwrap();let location=dir.path().canonicalize().unwrap().join("kali-distro");fs::create_dir(&location).unwrap();
        let registration=Registration {guid:"original".into(),name:DISTRO.into(),base_path:display_path(&location),version:2};
        let mut replacement=registration.clone();replacement.guid="replacement".into();
        let runner=fake(vec![("WSL registry",success(serde_json::to_string(&vec![replacement]).unwrap()))]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let mut state=manager.state().unwrap();state.phase="ready".into();state.registration=Some(registration);manager.save_state(&state).unwrap();
        assert!(manager.executor_identity().is_err());assert_eq!(runner.calls.lock().unwrap().len(),1);
    }
    #[test]
    fn package_arguments_are_a_closed_catalog() {
        assert!(validate_packages(vec!["curl; rm -rf /".into()]).is_err());
        assert!(validate_packages(vec!["--allow-unauthenticated".into()]).is_err());
        assert_eq!(validate_packages(vec!["jq".into(),"curl".into(),"jq".into()]).unwrap(),vec!["curl","jq"]);
    }
    #[test]
    fn unmanaged_existing_distro_is_never_adopted_or_mutated() {
        let dir=tempfile::tempdir().unwrap();
        let runner=fake(vec![("WSL registry",success(json!([{"guid":"other","name":DISTRO,"basePath":"C:\\personal","version":2}]).to_string()))]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        assert!(manager.handle("environment.plan",json!({})).is_err());
        assert_eq!(runner.calls.lock().unwrap().len(),1);
        assert_eq!(manager.state().unwrap().phase,"absent");
    }
    #[test]
    fn plans_bind_environment_digest_expiration_and_single_use() {
        let dir=tempfile::tempdir().unwrap(); let manager=EnvironmentManager::with_runner(dir.path().into(),fake(vec![])).unwrap();
        let plan=manager.save_plan("packages",json!({"packages":["curl"]})).unwrap();
        assert!(manager.consume_plan(Approval {plan_id:plan["id"].as_str().unwrap().into(),digest:"forged".into()},"packages").is_err());
        let approval=|| Approval {plan_id:plan["id"].as_str().unwrap().into(),digest:plan["digest"].as_str().unwrap().into()};
        assert!(manager.consume_plan(approval(),"provision").is_err());
        assert!(manager.consume_plan(approval(),"packages").is_ok());
        assert!(manager.consume_plan(approval(),"packages").is_err());
    }
    #[test]
    fn plan_mutation_is_detected_before_dispatch() {
        let dir=tempfile::tempdir().unwrap(); let manager=EnvironmentManager::with_runner(dir.path().into(),fake(vec![])).unwrap();
        let plan=manager.save_plan("packages",json!({"packages":["curl"]})).unwrap();
        let mut altered=plan.clone(); altered["payload"]["packages"]=json!(["nmap"]);
        manager.connection.lock().unwrap().execute("UPDATE environment_plans SET data=?1 WHERE id=?2",params![altered.to_string(),plan["id"].as_str().unwrap()]).unwrap();
        assert!(manager.consume_plan(Approval {plan_id:plan["id"].as_str().unwrap().into(),digest:plan["digest"].as_str().unwrap().into()},"packages").is_err());
    }
    #[test]
    fn inventory_failure_is_not_missing_or_ready() {
        let dir=tempfile::tempdir().unwrap(); let mut failure=success(""); failure.code=1; failure.stderr="Access denied".into();
        let manager=EnvironmentManager::with_runner(dir.path().into(),fake(vec![("WSL registry",failure)])).unwrap();
        let status=manager.diagnose().unwrap(); assert_eq!(status["status"],"unavailable"); assert_eq!(status["details"]["diagnosticFailed"],true);
    }
    #[test]
    fn timeout_and_truncation_never_report_success() {
        let mut result=success("completed"); result.timed_out=true; assert!(checked(&result).is_err());
        result.timed_out=false;result.truncated=true;assert!(checked(&result).is_err());
    }
    #[test]
    fn malformed_payload_and_unknown_action_never_start_a_process() {
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![]);let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        assert!(manager.handle("environment.provision",json!({"distro":"Ubuntu","command":"whoami"})).is_err());
        assert!(manager.handle("environment.exec",json!({})).is_err());
        assert!(manager.handle("environment.status",json!({"root":true})).is_err());
        assert!(runner.calls.lock().unwrap().is_empty());
    }
    #[test]
    fn installation_without_ack_is_journaled_and_not_repeated() {
        let dir=tempfile::tempdir().unwrap();let mut timeout=success("");timeout.timed_out=true;
        let runner=fake(vec![("WSL registry",success("[]")),("WSL dedicated installation",timeout),("WSL registry",success("[]"))]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let plan=manager.save_plan("provision",json!({"location":display_path(&manager.install_location())})).unwrap();
        assert!(manager.provision(Approval{plan_id:plan["id"].as_str().unwrap().into(),digest:plan["digest"].as_str().unwrap().into()}).is_err());
        assert_eq!(manager.state().unwrap().phase,"degraded");
        assert!(manager.resume().is_err());
        assert_eq!(runner.calls.lock().unwrap().iter().filter(|c|c.purpose=="WSL dedicated installation").count(),1);
    }
    #[test]
    fn managed_provisioning_binds_identity_and_restarts_only_its_distro() {
        let dir=tempfile::tempdir().unwrap();
        let location=dir.path().canonicalize().unwrap().join("kali-distro");
        let registration=json!([{"guid":"managed-guid","name":DISTRO,"basePath":display_path(&location),"version":2}]).to_string();
        let probe=json!({"ok":true,"result":{"uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenance":null}}).to_string();
        let runner=fake(vec![
            ("WSL registry",success("[]")),("WSL dedicated installation",success("installed")),
            ("WSL registry",success(registration.clone())),("WSL registry",success(registration.clone())),
            ("bootstrap",success("{\"ok\":true,\"result\":{\"bootstrapped\":true}}")),
            ("WSL dedicated restart",success("")),("WSL registry",success(registration.clone())),("probe",success(probe.clone())),
            ("WSL registry",success(registration.clone())),("WSL registry",success(registration)),("probe",success(probe)),
        ]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let plan=manager.save_plan("provision",json!({"location":display_path(&location)})).unwrap();
        let status=manager.provision(Approval{plan_id:plan["id"].as_str().unwrap().into(),digest:plan["digest"].as_str().unwrap().into()}).unwrap();
        assert_eq!(status["status"],"ready");assert_eq!(status["executorReady"],false);
        assert_eq!(manager.state().unwrap().registration.unwrap().guid,"managed-guid");
        for command in runner.calls.lock().unwrap().iter().filter(|command|command.purpose!="WSL registry") {
            assert!(!command.args.iter().any(|arg|arg=="--shutdown"||arg=="--unregister"||arg=="Ubuntu"));
            assert!(command.args.iter().any(|arg|arg==DISTRO));
        }
    }
    #[test]
    fn changed_registration_blocks_linux_dispatch() {
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![("WSL registry",success(json!([{"guid":"replacement","name":DISTRO,"basePath":"C:\\other","version":2}]).to_string()))]);
        let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let mut state=manager.state().unwrap();state.registration=Some(Registration{guid:"original".into(),name:DISTRO.into(),base_path:"C:\\managed".into(),version:2});
        assert!(manager.helper(&state,"probe",vec![],30).is_err());assert_eq!(runner.calls.lock().unwrap().len(),1);
    }
    #[test]
    fn utf16_wsl_output_is_decoded() {
        let bytes="Kali\r\n".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        assert_eq!(output_text(&bytes),"Kali\r\n");
    }
    #[test]
    fn malformed_helper_response_reports_metadata_without_contents_or_retry() {
        for (stdout, stderr, code, category) in [
            ("", "", 0, "Eof"),
            ("private-stdout-fixture", "private-stderr-fixture", 42, "Syntax"),
        ] {
            let (_dir, runner, manager, registration) = repair_fixture();
            let mut result = success(stdout);
            result.stderr = stderr.into();
            result.code = code;
            runner.outputs.lock().unwrap().extend([
                ("WSL registry", repair_registry(&registration)), ("probe", result),
            ]);
            let state = manager.state().unwrap();
            let error = manager.helper(&state, "probe", vec![], 30).unwrap_err();
            assert!(error.contains(&format!("exit={code}")));
            assert!(error.contains(&format!("stdout={} bytes", stdout.len())));
            assert!(error.contains(&format!("stderr={} bytes", stderr.len())));
            assert!(error.contains(&format!("categoria={category}")));
            assert!(!error.contains("private-stdout-fixture"));
            assert!(!error.contains("private-stderr-fixture"));
            assert_eq!(runner.calls.lock().unwrap().len(), 2);
            assert_eq!(manager.state().unwrap().last_error, state.last_error);
        }
    }
    #[test]
    fn pipe_read_errors_reject_even_complete_looking_json_without_retry() {
        struct FailingRead { prefix: Option<Vec<u8>> }
        impl Read for FailingRead {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if let Some(prefix) = self.prefix.take() {
                    if !prefix.is_empty() {
                        buffer[..prefix.len()].copy_from_slice(&prefix);
                        return Ok(prefix.len());
                    }
                }
                Err(std::io::Error::other("private read failure fixture"))
            }
        }
        for prefix in ["", r#"{"ok":true,"result":{"clean":true}}"#] {
            let (bytes, incomplete) = capture(FailingRead { prefix: Some(prefix.as_bytes().to_vec()) })
                .recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(bytes, prefix.as_bytes());
            assert!(incomplete);
            let (_dir, runner, manager, registration) = repair_fixture();
            let mut result = success(output_text(&bytes));
            result.truncated = incomplete;
            assert!(checked(&result).is_err());
            runner.outputs.lock().unwrap().extend([
                ("WSL registry", repair_registry(&registration)), ("probe", result),
            ]);
            let error = manager.helper(&manager.state().unwrap(), "probe", vec![], 30).unwrap_err();
            assert!(error.contains("perdeu confirmação"));
            assert!(!error.contains("private read failure fixture"));
            assert_eq!(runner.calls.lock().unwrap().len(), 2);
        }
    }
    struct FakeImages{calls:Mutex<usize>}
    impl ImageSource for FakeImages{
        fn prepare(&self,root:&Path,_:Option<&str>)->Result<VerifiedImage,String>{
            *self.calls.lock().unwrap()+=1;let bytes=b"synthetic verified image";let sha=format!("{:x}",Sha256::digest(bytes));let folder=root.join("image-cache");fs::create_dir_all(&folder).unwrap();let path=folder.join(format!("{sha}.wsl"));fs::write(&path,bytes).unwrap();
            Ok(VerifiedImage{manifest_url:crate::official_image::MANIFEST_URL.into(),url:"https://kali.download/wsl-images/kali-2026.2/kali-linux-2026.2-wsl-rootfs-amd64.wsl".into(),sha256:sha,bytes:bytes.len() as u64,local_file:path.to_string_lossy().into_owned()})
        }
    }
    fn image_help()->ProcessResult{success(WSL_HELP.replace("            --location <Location>","            --from-file <File>\n            --location <Location>"))}
    fn image_plan_checks()->Vec<(&'static str,ProcessResult)>{vec![("WSL registry",success("[]")),("WSL installation process check",success("[]")),("WSL image capabilities",image_help()),("WSL registry",success("[]")),("WSL installation process check",success("[]"))]}
    #[test]fn image_plan_can_recover_only_a_confirmed_absent_failed_installation(){
        let dir=tempfile::tempdir().unwrap();let runner=fake(image_plan_checks());let images=Arc::new(FakeImages{calls:Mutex::new(0)});let manager=EnvironmentManager::with_sources(dir.path().into(),runner.clone(),images.clone()).unwrap();
        let mut state=manager.state().unwrap();state.phase="degraded".into();state.last_error=Some("WSL retornou erro -1: WinINet timeout".into());state.last_output="WinINet timeout".into();manager.save_state(&state).unwrap();
        let plan=manager.handle("environment.image.plan",json!({"localFile":"C:\\fixture.wsl"})).unwrap();assert_eq!(plan["kind"],"provision-image");assert_eq!(plan["payload"]["distro"],DISTRO);assert_eq!(plan["payload"]["image"]["manifestUrl"],crate::official_image::MANIFEST_URL);assert_eq!(*images.calls.lock().unwrap(),1);assert_eq!(manager.state().unwrap().phase,"degraded");assert!(runner.calls.lock().unwrap().iter().all(|call|call.purpose!="WSL verified image installation"));
    }
    #[test]fn unknown_installation_is_never_retried_by_image_fallback(){
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![]);let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();
        let mut state=manager.state().unwrap();state.phase="degraded".into();state.last_error=Some("O WSL não confirmou o término no prazo.".into());state.last_output="timeout".into();manager.save_state(&state).unwrap();
        assert!(manager.handle("environment.image.plan",json!({})).is_err());state.install_exit=Some(InstallExit{confirmed:false,code:Some(-1)});manager.save_state(&state).unwrap();assert!(manager.handle("environment.image.plan",json!({})).is_err());assert!(runner.calls.lock().unwrap().is_empty());
    }
    #[test]fn live_wsl_client_blocks_fallback_without_terminating_it(){
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![("WSL registry",success("[]")),("WSL installation process check",success("[42]"))]);let images=Arc::new(FakeImages{calls:Mutex::new(0)});let manager=EnvironmentManager::with_sources(dir.path().into(),runner.clone(),images.clone()).unwrap();
        assert!(manager.handle("environment.image.plan",json!({})).unwrap_err().contains("wsl.exe"));assert_eq!(*images.calls.lock().unwrap(),0);assert_eq!(runner.calls.lock().unwrap().len(),2);
    }
    #[test]fn image_changed_after_approval_cannot_reach_installation(){
        let dir=tempfile::tempdir().unwrap();let mut outputs=image_plan_checks();outputs.extend([("WSL registry",success("[]")),("WSL installation process check",success("[]"))]);let runner=fake(outputs);let manager=EnvironmentManager::with_sources(dir.path().into(),runner.clone(),Arc::new(FakeImages{calls:Mutex::new(0)})).unwrap();
        let plan=manager.handle("environment.image.plan",json!({})).unwrap();fs::write(plan["payload"]["image"]["localFile"].as_str().unwrap(),b"modified after approval").unwrap();
        assert!(manager.handle("environment.image.provision",json!({"planId":plan["id"],"digest":plan["digest"]})).is_err());assert!(runner.calls.lock().unwrap().iter().all(|call|call.purpose!="WSL verified image installation"));assert_eq!(manager.state().unwrap().phase,"absent");
    }
    #[test]fn verified_image_install_uses_only_reviewed_file_and_managed_destination(){
        let dir=tempfile::tempdir().unwrap();let mut outputs=image_plan_checks();let mut failure=success("installation fixture failure");failure.code=7;outputs.extend([("WSL registry",success("[]")),("WSL installation process check",success("[]")),("WSL verified image installation",failure)]);let runner=fake(outputs);let manager=EnvironmentManager::with_sources(dir.path().into(),runner.clone(),Arc::new(FakeImages{calls:Mutex::new(0)})).unwrap();
        let plan=manager.handle("environment.image.plan",json!({})).unwrap();assert!(manager.handle("environment.image.provision",json!({"planId":plan["id"],"digest":plan["digest"]})).is_err());
        let calls=runner.calls.lock().unwrap();let install=calls.iter().find(|call|call.purpose=="WSL verified image installation").unwrap();assert_eq!(install.args[0],"--install");assert_eq!(install.args[1],"--from-file");assert_eq!(install.args[2],display_path(Path::new(plan["payload"]["image"]["localFile"].as_str().unwrap())));assert!(install.args.iter().any(|arg|arg==DISTRO));assert!(!install.args.iter().any(|arg|arg=="--unregister"||arg=="--shutdown"||arg=="Ubuntu"));
        let state=manager.state().unwrap();assert_eq!(state.phase,"degraded");let exit=state.install_exit.unwrap();assert!(exit.confirmed);assert_eq!(exit.code,Some(7));
    }
    #[test]fn image_installation_requires_from_file_inside_install_section(){assert!(image_installation_capabilities(&image_help()).is_ok());assert!(image_installation_capabilities(&success(WSL_HELP)).is_err());let wrong=WSL_HELP.replace("    --mount <Disk>","    --mount <Disk>\n        --from-file <File>");assert!(image_installation_capabilities(&success(wrong)).is_err());}

    #[test]
    fn reconciled_refresh_is_usable_only_with_explicit_quiescence_and_no_success_claim() {
        let healthy=json!({"uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenance":{"action":"refresh","status":"reconciled","quiescent":true,"refreshSucceeded":false}});
        assert!(healthy_probe(&healthy));
        for (field,value) in [("action",json!("install")),("status",json!("unknown")),("quiescent",json!(false)),("refreshSucceeded",json!(true)),("refreshSucceeded",Value::Null)] {
            let mut changed=healthy.clone();changed["maintenance"][field]=value;assert!(!healthy_probe(&changed),"{field}");
        }
    }

    #[test]
    fn persistent_maintenance_gate_requires_confirmed_failed_install_before_activation() {
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![]);let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();let mut state=manager.state().unwrap();
        for phase in ["absent","ready"] {state.phase=phase.into();manager.save_state(&state).unwrap();assert!(!manager.has_unresolved_maintenance().unwrap());}
        for phase in ["installing","bootstrap","maintenance","degraded"] {state.phase=phase.into();manager.save_state(&state).unwrap();assert!(manager.has_unresolved_maintenance().unwrap());}
        for exit in [InstallExit{confirmed:false,code:Some(-1)},InstallExit{confirmed:true,code:None},InstallExit{confirmed:true,code:Some(0)}] {state.install_exit=Some(exit);manager.save_state(&state).unwrap();assert!(manager.has_unresolved_maintenance().unwrap());}
        state.install_exit=Some(InstallExit{confirmed:true,code:Some(-1)});manager.save_state(&state).unwrap();assert!(!manager.has_unresolved_maintenance().unwrap());
        state.registration=Some(Registration{guid:"fixture".into(),name:DISTRO.into(),base_path:"C:\\fixture".into(),version:2});manager.save_state(&state).unwrap();assert!(manager.has_unresolved_maintenance().unwrap());
        assert!(runner.calls.lock().unwrap().is_empty());
    }

    fn repair_fixture()->(tempfile::TempDir,Arc<FakeRunner>,EnvironmentManager,Registration){
        let dir=tempfile::tempdir().unwrap();let runner=fake(vec![]);let manager=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();fs::create_dir(manager.install_location()).unwrap();
        let registration=Registration{guid:"managed-guid".into(),name:DISTRO.into(),base_path:display_path(&manager.install_location()),version:2};
        let mut state=manager.state().unwrap();state.phase="degraded".into();state.last_error=Some("legacy maintenance without confirmed process identity".into());state.registration=Some(registration.clone());manager.save_state(&state).unwrap();
        (dir,runner,manager,registration)
    }
    fn repair_registry(registration:&Registration)->ProcessResult{success(json!([registration,{"guid":"personal-guid","name":"Ubuntu","basePath":"C:\\personal","version":2}]).to_string())}
    fn pending_probe(sha:&str)->ProcessResult{success(json!({"ok":true,"result":{"uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenanceSha256":sha,"maintenance":{"action":"refresh","status":"unknown","error":"legacy interrupted update"}}}).to_string())}
    fn repaired_probe()->ProcessResult{success(json!({"ok":true,"result":{"uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenance":{"action":"refresh","status":"reconciled","quiescent":true,"refreshSucceeded":false}}}).to_string())}
    fn queue_repair_probe(runner:&FakeRunner,registration:&Registration,sha:&str){runner.outputs.lock().unwrap().extend([("WSL registry",repair_registry(registration)),("probe",pending_probe(sha))]);}
    fn plan_repair(manager:&EnvironmentManager)->Value{manager.handle("environment.repair.plan",json!({})).unwrap()}
    fn apply_repair(manager:&EnvironmentManager,plan:&Value)->Result<Value,String>{manager.handle("environment.repair.apply",json!({"planId":plan["id"],"digest":plan["digest"]}))}
    #[test]
    fn repair_preview_binds_owned_identity_and_journal_without_stopping_any_distro(){
        let (_dir,runner,manager,registration)=repair_fixture();queue_repair_probe(&runner,&registration,&"a".repeat(64));let plan=plan_repair(&manager);
        assert_eq!(plan["kind"],"environment-repair");assert_eq!(plan["payload"]["distro"],DISTRO);assert_eq!(plan["payload"]["registration"]["guid"],registration.guid);assert_eq!(runner.calls.lock().unwrap().len(),2);assert!(manager.repairs().unwrap().is_empty());
    }
    #[test]
    fn changed_repair_journal_consumes_stale_plan_without_termination(){
        let (_dir,runner,manager,registration)=repair_fixture();queue_repair_probe(&runner,&registration,&"a".repeat(64));let plan=plan_repair(&manager);queue_repair_probe(&runner,&registration,&"b".repeat(64));
        assert!(apply_repair(&manager,&plan).unwrap_err().contains("journal mudou"));assert!(manager.repairs().unwrap().is_empty());assert!(runner.calls.lock().unwrap().iter().all(|c|c.purpose!="WSL repair termination"));assert!(apply_repair(&manager,&plan).is_err());
    }
    #[test]
    fn changed_native_state_prevents_repair_before_any_new_process(){
        let (_dir,runner,manager,registration)=repair_fixture();queue_repair_probe(&runner,&registration,&"a".repeat(64));let plan=plan_repair(&manager);let mut state=manager.state().unwrap();state.last_error=Some("a different failure".into());manager.save_state(&state).unwrap();
        assert!(apply_repair(&manager,&plan).is_err());assert_eq!(runner.calls.lock().unwrap().len(),2);
    }
    #[test]
    fn uncertain_termination_keeps_blocked_and_never_dispatches_repair(){
        let (_dir,runner,manager,registration)=repair_fixture();let sha="a".repeat(64);queue_repair_probe(&runner,&registration,&sha);let plan=plan_repair(&manager);queue_repair_probe(&runner,&registration,&sha);
        let mut timeout=success("");timeout.timed_out=true;runner.outputs.lock().unwrap().extend([("WSL registry",repair_registry(&registration)),("WSL repair termination",timeout)]);
        assert!(apply_repair(&manager,&plan).is_err());let repairs=manager.repairs().unwrap();assert_eq!(repairs[0].status,"unknown");assert!(repairs[0].termination_receipt.is_none());assert_eq!(manager.state().unwrap().phase,"degraded");assert!(manager.has_unresolved_maintenance().unwrap());assert!(runner.calls.lock().unwrap().iter().all(|c|c.purpose!="repair"));
    }
    #[test]
    fn running_inventory_fails_closed_for_incomplete_error_or_unrecognized_output(){
        let (_dir,_runner,_manager,registration)=repair_fixture();let inventory=vec![registration];
        assert!(require_stopped(&success(""),&inventory).is_ok());assert!(require_stopped(&success(DISTRO),&inventory).is_err());assert!(require_stopped(&success("no running distributions"),&inventory).is_err());
        for (timed_out,truncated,code) in [(true,false,0),(false,true,0),(false,false,-1)]{let mut result=success("");result.timed_out=timed_out;result.truncated=truncated;result.code=code;assert!(require_stopped(&result,&inventory).is_err());}
        let mut warning=success("");warning.stderr="partial result".into();assert!(require_stopped(&warning,&inventory).is_err());
    }
    #[test]
    fn changed_registration_after_termination_prevents_linux_repair(){
        let (_dir,runner,manager,registration)=repair_fixture();let sha="a".repeat(64);queue_repair_probe(&runner,&registration,&sha);let plan=plan_repair(&manager);queue_repair_probe(&runner,&registration,&sha);
        let mut changed=registration.clone();changed.guid="replacement".into();runner.outputs.lock().unwrap().extend([("WSL registry",repair_registry(&registration)),("WSL repair termination",success("")),("WSL registry",repair_registry(&changed))]);
        assert!(apply_repair(&manager,&plan).is_err());assert!(manager.repairs().unwrap()[0].termination_receipt.is_none());assert!(runner.calls.lock().unwrap().iter().all(|c|c.purpose!="repair"));
    }
    #[test]
    fn confirmed_repair_stops_only_owned_distro_and_preserves_refresh_failure(){
        let (_dir,runner,manager,registration)=repair_fixture();let sha="a".repeat(64);queue_repair_probe(&runner,&registration,&sha);let plan=plan_repair(&manager);queue_repair_probe(&runner,&registration,&sha);
        runner.outputs.lock().unwrap().extend([
            ("WSL registry",repair_registry(&registration)),("WSL repair termination",success("")),
            ("WSL registry",repair_registry(&registration)),("WSL repair running inventory",success("Ubuntu\r\n")),
            ("WSL registry",repair_registry(&registration)),("WSL registry",repair_registry(&registration)),
            ("repair",success(json!({"ok":true,"result":{"clean":true,"originalSha256":sha,"repairId":"echo-receipt"}}).to_string())),
            ("WSL registry",repair_registry(&registration)),("probe",repaired_probe()),
        ]);
        let result=apply_repair(&manager,&plan).unwrap();assert_eq!(result["status"],"ready");assert_eq!(result["details"]["maintenance"]["refreshSucceeded"],false);assert_eq!(manager.repairs().unwrap()[0].status,"completed");assert!(!manager.has_unresolved_maintenance().unwrap());
        let calls=runner.calls.lock().unwrap();let terminate=calls.iter().find(|c|c.purpose=="WSL repair termination").unwrap();assert_eq!(terminate.args,vec!["--terminate",DISTRO]);assert!(calls.iter().all(|c|!c.args.iter().any(|a|["--shutdown","--unregister","apt-get"].contains(&a.as_str()))));
        let wire:Value=serde_json::from_slice(calls.iter().find(|c|c.purpose=="repair").unwrap().input.as_ref().unwrap()).unwrap();let receipt:Value=serde_json::from_str(wire["args"][0].as_str().unwrap()).unwrap();assert_eq!(receipt["terminationVerified"],true);assert_eq!(receipt["distro"],DISTRO);drop(calls);assert!(apply_repair(&manager,&plan).is_err());
    }
    #[test]
    fn interrupted_repair_is_marked_unknown_without_process_replay_on_open(){
        let (dir,runner,manager,registration)=repair_fixture();let state=manager.state().unwrap();manager.save_repair(&RepairAttempt{id:uuid::Uuid::new_v4().to_string(),plan_id:uuid::Uuid::new_v4().to_string(),environment_id:state.id,registration,
            maintenance_sha256:"a".repeat(64),status:"terminating".into(),created_at:now(),finished_at:None,termination_receipt:None,error:None}).unwrap();drop(manager);
        let reopened=EnvironmentManager::with_runner(dir.path().into(),runner.clone()).unwrap();assert_eq!(reopened.repairs().unwrap()[0].status,"unknown");assert!(runner.calls.lock().unwrap().is_empty());
    }
}
