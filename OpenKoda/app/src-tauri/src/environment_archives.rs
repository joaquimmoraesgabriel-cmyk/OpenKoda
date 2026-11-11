//! Filesystem backups of the managed WSL 2 environment. The renderer supplies
//! only a receipt ID or a review decision. Paths and distro names are derived here.
use super::*;
use std::{fs::File, io::{Seek, SeekFrom}, sync::MutexGuard};
use uuid::Uuid;

const EXPORT: &str = "kali-vhd-export";
const IMPORT: &str = "kali-vhd-import";
const MAX_IMAGE: u64 = 64 * 1024 * 1024 * 1024;
const PAGE_SIZE: usize = 128;
const INSPECT: &str = include_str!("../../scripts/kali-archive-inspect.py");
const PROTOCOL_SOURCE: &str = include_str!("../../scripts/kali-executor.py");
const REKEY: &str = include_str!("../../scripts/kali-restore-marker.py");

pub(super) fn supports(action: &str) -> bool {
    matches!(action, "environment.archive.list" | "environment.archive.exportPlan"
        | "environment.archive.export" | "environment.archive.importPlan"
        | "environment.archive.import" | "environment.archive.inspectStop" | "environment.archive.activatePlan" | "environment.archive.activate")
}
pub(super) fn exclusive(action: &str) -> bool { supports(action) && action != "environment.archive.list" }

#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Selection { id: String }

#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct PageRequest { before_id: Option<String> }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct SpoolInventory { raw_captures: u32, terminal_captures: u32, session_records: u32 }
// Existing receipts implicitly describe a raw VHDX. New exports use WSL's TAR stream.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ArchiveFormat { Tar, #[default] Vhdx }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Backup {
    id: String,
    source_id: String,
    source: Registration,
    #[serde(default)]
    format: ArchiveFormat,
    // File identity binds the exact reviewed VHDX across the stop.
    disk_id: String,
    max_bytes: u64,
    spool: SpoolInventory,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Restore {
    id: String,
    backup: Backup,
    bytes: u64,
    sha256: String,
    first_wsl_distribution: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag="kind", rename_all="snake_case", deny_unknown_fields)]
enum Operation { Export { backup: Backup }, Import { restore: Restore } }
impl Operation {
    fn id(&self) -> &str {
        match self { Self::Export{backup} => &backup.id, Self::Import{restore} => &restore.id }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Record {
    operation: Operation,
    status: String,
    started_at: String,
    updated_at: String,
    bytes: Option<u64>,
    sha256: Option<String>,
    registration: Option<Registration>,
    error: Option<String>,
}

fn uuid(id: &str) -> Result<(), String> {
    if Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id) { Ok(()) }
    else { Err("Identidade inválida para a cópia do Kali.".into()) }
}
fn storage_error(_: impl std::fmt::Display) -> String { "Registro de cópias do ambiente indisponível.".into() }
fn disk_error(e: impl std::fmt::Display) -> String { format!("Falha ao acessar a cópia local do disco: {e}") }

pub(super) struct ArchiveStore(Mutex<Connection>);
impl ArchiveStore {
    pub(super) fn open(root: &Path) -> Result<Self,String> {
        let db = Connection::open(root.join("environment-archives.sqlite3")).map_err(storage_error)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS archive_ops(id TEXT PRIMARY KEY,data TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS archive_status ON archive_ops(json_extract(data,'$.status'));").map_err(storage_error)?;
        let store=Self(Mutex::new(db));
        loop {
            let interrupted = {
                let db = store.db()?;
                let mut q = db.prepare("SELECT id,data FROM archive_ops
                    WHERE json_extract(data,'$.status') IN ('stopping_source','exporting_source','copying','importing','personalizing','stopping_copy')
                    LIMIT ?1").map_err(storage_error)?;
                Self::records(&mut q, params![PAGE_SIZE])?
            };
            if interrupted.is_empty() { break; }
            for mut record in interrupted {
                // A crash can leave a partial file or a registered clone.
                record.status = match record.status.as_str() {
                    "stopping_source" | "exporting_source" => "stop_unknown",
                    "copying" => "incomplete_copy",
                    "importing" | "personalizing" | "stopping_copy" => "copy_unknown",
                    _ => return Err(storage_error("unexpected interrupted operation")),
                }.into();
                record.error=Some("O aplicativo foi interrompido. Consulte os dados da tentativa antes de preparar outra operação.".into());
                store.save(&mut record)?;
            }
        }
        Ok(store)
    }
    fn db(&self) -> Result<MutexGuard<'_,Connection>,String> { self.0.lock().map_err(storage_error) }
    fn record(id: String, raw: String) -> Result<Record, String> {
        uuid(&id)?;
        let record: Record = serde_json::from_str(&raw).map_err(storage_error)?;
        if record.operation.id() != id { return Err(storage_error("identity")); }
        Ok(record)
    }
    fn records(q: &mut rusqlite::Statement<'_>, args: impl rusqlite::Params) -> Result<Vec<Record>, String> {
        let rows = q.query_map(args, |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?))).map_err(storage_error)?;
        rows.map(|row| {
            let (id, raw) = row.map_err(storage_error)?;
            Self::record(id, raw)
        }).collect()
    }
    fn page(&self, before: Option<&str>) -> Result<(Vec<Record>, Option<String>), String> {
        let db = self.db()?;
        let mut records = if let Some(id) = before {
            uuid(id)?;
            let cursor: i64 = db.query_row("SELECT rowid FROM archive_ops WHERE id=?1", [id], |r| r.get(0))
                .optional().map_err(storage_error)?.ok_or("Página de cópias não encontrada.")?;
            let mut q = db.prepare("SELECT id,data FROM archive_ops WHERE rowid<?1 ORDER BY rowid DESC LIMIT ?2").map_err(storage_error)?;
            Self::records(&mut q, params![cursor, PAGE_SIZE + 1])?
        } else {
            let mut q = db.prepare("SELECT id,data FROM archive_ops ORDER BY rowid DESC LIMIT ?1").map_err(storage_error)?;
            Self::records(&mut q, params![PAGE_SIZE + 1])?
        };
        let next = if records.len() > PAGE_SIZE {
            records.truncate(PAGE_SIZE);
            records.last().map(|r| r.operation.id().to_owned())
        } else { None };
        Ok((records, next))
    }
    fn get(&self, id: &str) -> Result<Record,String> {
        uuid(id)?;
        let raw = self.db()?.query_row("SELECT data FROM archive_ops WHERE id=?1", [id], |r| r.get(0))
            .optional().map_err(storage_error)?.ok_or("Tentativa de cópia não encontrada.")?;
        Self::record(id.to_owned(), raw)
    }
    fn begin(&self,operation:Operation,status:&str)->Result<Record,String>{
        uuid(operation.id())?;
        let record=Record{operation,status:status.into(),started_at:now(),updated_at:now(),bytes:None,sha256:None,registration:None,error:None};
        self.db()?.execute("INSERT INTO archive_ops VALUES(?1,?2)",params![record.operation.id(),serde_json::to_string(&record).map_err(storage_error)?]).map_err(storage_error)?;
        Ok(record)
    }
    fn save(&self,record:&mut Record)->Result<(),String>{
        record.updated_at=now();
        let changed=self.db()?.execute("UPDATE archive_ops SET data=?2 WHERE id=?1",params![record.operation.id(),serde_json::to_string(record).map_err(storage_error)?]).map_err(storage_error)?;
        if changed!=1{return Err(storage_error("missing receipt"))}Ok(())
    }
    pub(super) fn stop_pending(&self)->Result<bool,String>{
        self.db()?.query_row("SELECT EXISTS(SELECT 1 FROM archive_ops
            WHERE json_extract(data,'$.status') IN ('stopping_source','exporting_source','stop_unknown'))",
            [], |r| r.get(0)).map_err(storage_error)
    }
    pub(super) fn check_source_stop(&self)->Result<(),String>{
        if self.stop_pending()? {Err("A parada anterior do Kali precisa ser conferida em Manutenção → Cópias do Kali.".into())}
        else {Ok(())}
    }
}

// On Windows keep every directory from the base down open without FILE_SHARE_DELETE.
// The input handle also denies writing throughout hashing/copying/WSL import.
fn node(path:&Path,dir:bool,attributes_only:bool)->Result<(File,String),String>{
    let mut opts=fs::OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::{io::AsRawHandle,fs::OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::*;
        opts.access_mode(if attributes_only {FILE_READ_ATTRIBUTES} else {0x80000000})
            .share_mode(if dir||attributes_only {FILE_SHARE_READ|FILE_SHARE_WRITE} else {FILE_SHARE_READ})
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT|if dir{FILE_FLAG_BACKUP_SEMANTICS}else{0});
        let file=opts.open(path).map_err(disk_error)?;
        let mut info=unsafe{std::mem::zeroed()};
        if unsafe{GetFileInformationByHandle(file.as_raw_handle(),&mut info)}==0 {return Err(disk_error(std::io::Error::last_os_error()))}
        if info.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT!=0 || (info.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY!=0)!=dir
            || (!dir && info.nNumberOfLinks!=1){return Err("O caminho contém um vínculo ou tipo de arquivo inesperado.".into())}
        let index=((info.nFileIndexHigh as u64)<<32)|info.nFileIndexLow as u64;
        Ok((file,format!("{}:{index}",info.dwVolumeSerialNumber)))
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        let _=attributes_only;
        let meta=fs::symlink_metadata(path).map_err(disk_error)?;
        if meta.is_dir()!=dir||(!dir&&(!meta.is_file()||meta.nlink()!=1)){return Err("Caminho inesperado para a cópia.".into())}
        let file=opts.open(path).map_err(disk_error)?;
        Ok((file,format!("{}:{}",meta.dev(),meta.ino())))
    }
}

fn ensure_dir(path:&Path)->Result<File,String>{
    match fs::create_dir(path){Ok(())=>(),Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>(),Err(e)=>return Err(disk_error(e))}
    Ok(node(path,true,false)?.0)
}
fn must_be_new(path:&Path)->Result<(),String>{
    match fs::symlink_metadata(path) {
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),
        Ok(_)=>Err("O destino da cópia já existe. Os dados foram preservados.".into()),Err(e)=>Err(disk_error(e))
    }
}
fn stream_archive(mut input: impl Read, mut output: impl Write, limit: u64) -> Result<(u64, String), String> {
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut bytes = 0u64;
    let mut hash = Sha256::new();
    loop {
        let read = input.read(&mut buffer).map_err(disk_error)?;
        if read == 0 { break; }
        bytes += read as u64;
        if bytes > limit { return Err("A exportação excedeu o limite revisado.".into()); }
        output.write_all(&buffer[..read]).map_err(disk_error)?;
        hash.update(&buffer[..read]);
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}

fn archive_hash(file: &mut File, format: ArchiveFormat, limit: u64) -> Result<(u64, String), String> {
    let size = file.metadata().map_err(disk_error)?.len();
    if size > limit || size > MAX_IMAGE { return Err("A cópia excede o tamanho revisado.".into()); }
    file.seek(SeekFrom::Start(0)).map_err(disk_error)?;
    match format {
        ArchiveFormat::Tar => {
            let mut header = [0; 512];
            file.read_exact(&mut header).map_err(disk_error)?;
            if &header[257..262] != b"ustar" || size % 512 != 0 || size < 1536 {
                return Err("O arquivo não é um TAR completo do WSL.".into());
            }
            file.seek(SeekFrom::End(-1024)).map_err(disk_error)?;
            let mut end = [0; 1024];
            file.read_exact(&mut end).map_err(disk_error)?;
            if end.iter().any(|byte| *byte != 0) { return Err("O final do TAR está incompleto.".into()); }
        }
        ArchiveFormat::Vhdx => {
            let mut signature = [0; 8];
            file.read_exact(&mut signature).map_err(disk_error)?;
            if &signature != b"vhdxfile" { return Err("O disco não tem a assinatura VHDX.".into()); }
        }
    }
    file.seek(SeekFrom::Start(0)).map_err(disk_error)?;
    let actual = stream_archive(file, std::io::sink(), size)?;
    if actual.0 != size { return Err("O tamanho da cópia mudou durante a leitura.".into()); }
    Ok(actual)
}

pub(super) fn system_export_tar(distribution: &str, target: File, limit: u64) -> Result<(u64, String), String> {
    let spec = CommandSpec {
        purpose: "WSL archive export", executable: system_binary("wsl.exe"),
        args: vec!["--export".into(), distribution.into(), "-".into(), "--format".into(), "tar".into()],
        input: None, timeout: Duration::from_secs(3600),
    };
    let mut child = process_command(&spec).spawn().map_err(disk_error)?;
    let pipe = child.stdout.take().ok_or("Canal do TAR indisponível.")?;
    let stderr = capture(child.stderr.take().ok_or("Diagnóstico do WSL indisponível.")?);
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(stream_archive(pipe, target, limit));
    });
    let deadline = Instant::now() + spec.timeout;
    let mut captured = None;
    let status = loop {
        if captured.is_none() {
            match receiver.try_recv() {
                Ok(Ok(digest)) => captured = Some(digest),
                Ok(Err(error)) => { let _ = child.kill(); let _ = child.wait(); return Err(error); }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = child.kill(); let _ = child.wait(); return Err("O canal do TAR foi interrompido.".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Err(error) => {let _ = child.kill(); let _ = child.wait(); return Err(disk_error(error));}
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill(); let _ = child.wait(); return Err("Tempo de exportação do TAR excedido.".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
        }
    };
    let (diagnostics, truncated) = stderr.recv_timeout(Duration::from_secs(3)).map_err(|_| "Diagnóstico do WSL incompleto.")?;
    checked(&ProcessResult {
        code: status.code().unwrap_or(-1), stdout: String::new(), stderr: output_text(&diagnostics),
        timed_out: false, truncated,
    })?;
    match captured {
        Some(value) => Ok(value),
        None => receiver.recv_timeout(Duration::from_secs(10)).map_err(|_| "O fluxo do TAR não terminou.")?,
    }
}

fn clone_name(id:&str)->String {format!("OpenKoda-Recovery-{id}")}
// wsl.exe can report this optional systemd session failure even when a fixed
// --exec command completed as the verified unprivileged user. All other stderr
// remains an error for these protocol responses.
fn user_stderr_ok(text:&str)->bool{
    text.lines().all(|line|matches!(line.trim(),""|"wsl: Failed to start the systemd user session for 'openkoda'. See journalctl for more details."))
}
fn stop_check(result:&ProcessResult,inventory:&[Registration],name:&str)->Result<(),String>{
    checked(result)?;
    let names:Vec<_>=result.stdout.lines().map(str::trim).filter(|s|!s.is_empty()).collect();
    if !result.stderr.trim().is_empty() || names.len()>1000
        || names.iter().any(|s|s.chars().any(char::is_control)||!inventory.iter().any(|r|r.name==*s))
        || names.iter().any(|s|s.eq_ignore_ascii_case(name)) {return Err("O WSL não confirmou a parada da distribuição revisada.".into())}
    Ok(())
}

impl EnvironmentManager {
    pub(super) fn archive_action(&self,action:&str,payload:Value)->Result<Value,String>{
        match action {
            "environment.archive.list" => {
                let input: PageRequest = decode(payload)?;
                let (records, next) = self.archives.page(input.before_id.as_deref())?;
                Ok(json!({"records":self.activation_rows(records)?,"nextBeforeId":next}))
            }
            "environment.archive.activatePlan" => self.activate_plan(decode(payload)?),
            "environment.archive.activate" => self.activate_restored(decode(payload)?),
            "environment.archive.inspectStop" => self.inspect_archive_stop(decode(payload)?),
            _=>{
                // A verified archive can be restored even if its original registration was lost.
                // Only exporting uses the managed source again.
                if matches!(action, "environment.archive.exportPlan" | "environment.archive.export") {
                    self.archives.check_source_stop()?;
                }
                match action {
                    "environment.archive.exportPlan"=>{empty(&payload)?;self.archive_export_plan()}
                    "environment.archive.export"=>self.archive_export(decode(payload)?),
                    "environment.archive.importPlan"=>self.archive_import_plan(decode(payload)?),
                    "environment.archive.import"=>self.archive_import(decode(payload)?),
                    _=>Err("Operação de cópia desconhecida.".into())
                }
            }
        }
    }
    fn archive_dir(&self)->PathBuf {self.root.join("kali-backups")}
    fn disk_path(&self,id:&str,format:ArchiveFormat)->Result<PathBuf,String>{
        uuid(id)?;
        let extension=match format { ArchiveFormat::Tar=>"tar", ArchiveFormat::Vhdx=>"vhdx" };
        Ok(self.archive_dir().join(format!("{id}.{extension}")))
    }
    fn recovery_dir(&self,id:&str)->Result<PathBuf,String>{uuid(id)?;Ok(self.root.join("kali-recovery").join(id))}
    fn check_stop(&self,name:&str,registration:&Registration,location:&Path)->Result<(),String>{
        let inventory=self.inventory()?;
        if !inventory.iter().any(|r|r==registration && r.name==name && r.version==2 && same_path(&r.base_path,location)){
            return Err("O registro ou o local da distribuição mudou.".into())
        }
        let current=self.wsl("WSL archive running inventory",vec!["--list".into(),"--running".into(),"--quiet".into()],30)?;
        stop_check(&current,&inventory,name)
    }

    fn inspect_spool(&self,state:&EnvironmentState)->Result<SpoolInventory,String>{
        let registration=self.owned_registration(state)?;
        let command=CommandSpec{
            purpose:"WSL archive spool inventory", executable:system_binary("wsl.exe"),
            args:vec!["--distribution".into(),registration.name,"--user".into(),"openkoda".into(),"--cd".into(),"/".into(),"--exec".into(),
                "/usr/bin/python3".into(),"-I".into(),"-c".into(),INSPECT.into(),state.id.clone()],
            input:Some(PROTOCOL_SOURCE.as_bytes().to_vec()),timeout:Duration::from_secs(60)
        };
        let result=self.runner.run(&command)?;
        if checked(&result).is_err()||!user_stderr_ok(&result.stderr){return Err("O inventário do spool não pôde ser confirmado. Confira as capturas e sessões pendentes antes de exportar.".into())}
        let inventory:SpoolInventory=serde_json::from_str(result.stdout.trim()).map_err(|_|"Inventário de capturas inválido.")?;
        if inventory.raw_captures>0{return Err("Há bytes de captura no Kali. Em Configurações → Dados e privacidade, revise a retenção das capturas no Kali antes de exportar o disco.".into())}
        if inventory.terminal_captures>256 || inventory.session_records>100000{return Err("Inventário de capturas acima do limite.".into())}
        Ok(inventory)
    }
    fn archive_export_plan(&self)->Result<Value,String>{
        let state=self.state()?;if state.phase!="ready"{return Err("O Kali gerido precisa estar pronto para a cópia.".into())}
        let source=self.owned_registration(&state)?;
        let location=self.bound_location(&state)?;let _hold=node(&location,true,false)?;
        let(file,disk_id)=node(&location.join("ext4.vhdx"),false,true)?;
        let bytes=file.metadata().map_err(disk_error)?.len();
        if bytes<8{return Err("O disco de origem é inválido.".into())}
        let spool=self.inspect_spool(&state)?;
        let backup=Backup{id:Uuid::new_v4().to_string(),source_id:state.id,source,disk_id,format:ArchiveFormat::Tar,max_bytes:MAX_IMAGE,spool};
        self.save_plan(EXPORT,json!({"backup":backup,"target":display_path(&self.disk_path(&backup.id,backup.format)?),"effects":"Encerrar o Kali gerido e exportar todos os arquivos da distribuição em um TAR local, com limite de 64 GiB."}))
    }

    fn archive_export(&self,approval:Approval)->Result<Value,String>{
        let plan=self.consume_plan(approval,EXPORT)?;
        let backup:Backup=decode(plan.payload["backup"].clone())?;uuid(&backup.id)?;
        if backup.format!=ArchiveFormat::Tar||backup.max_bytes!=MAX_IMAGE||backup.source_id!=plan.environment_id {return Err("Identidade da cópia inválida.".into())}
        let state=self.require_ready()?;
        if self.inspect_spool(&state)?!=backup.spool{return Err("O inventário do spool mudou. Prepare uma nova revisão.".into())}
        if state.id!=backup.source_id||self.owned_registration(&state)?!=backup.source{return Err("A origem da cópia mudou.".into())}
        let _base=node(&self.root,true,false)?;let location=self.bound_location(&state)?;let _source=node(&location,true,false)?;
        let disk=location.join("ext4.vhdx");let (metadata,id)=node(&disk,false,true)?;
        if id!=backup.disk_id || metadata.metadata().map_err(disk_error)?.len()<8{return Err("O VHDX mudou. Prepare outra revisão.".into())}
        let _target=ensure_dir(&self.archive_dir())?;let path=self.disk_path(&backup.id,backup.format)?;must_be_new(&path)?;
        let mut record=self.archives.begin(Operation::Export{backup:backup.clone()},"stopping_source")?;
        let outcome=(||{
            checked(&self.wsl("WSL archive stop source",vec!["--terminate".into(),backup.source.name.clone()],60)?)?;
            self.check_stop(&backup.source.name,&backup.source,&location)?;
            record.status="copying".into();self.archives.save(&mut record)?;
            let (_held,current_id)=node(&disk,false,true)?;
            if current_id!=backup.disk_id{return Err("O disco de origem mudou antes da exportação.".into())}
            let mut output=fs::OpenOptions::new();output.write(true).read(true).create_new(true);
            #[cfg(windows)] {use std::os::windows::fs::OpenOptionsExt;output.share_mode(0);}
            let mut target=output.open(&path).map_err(disk_error)?;
            // WSL may temporarily mount the source to generate TAR. A lost client
            // result must not attest that the reviewed source is still stopped.
            record.status="exporting_source".into();self.archives.save(&mut record)?;
            let exported=self.runner.export_tar(&backup.source.name,target.try_clone().map_err(disk_error)?, backup.max_bytes);
            self.check_stop(&backup.source.name,&backup.source,&location)?;
            record.status="copying".into();self.archives.save(&mut record)?;
            let copied=exported?;
            target.sync_all().map_err(disk_error)?;
            if archive_hash(&mut target,backup.format,backup.max_bytes)?!=copied{return Err("O arquivo gravado divergiu da exportação.".into())}
            record.bytes=Some(copied.0);record.sha256=Some(copied.1);
            Ok::<(),String>(())
        })();
        match outcome {
            Ok(())=>record.status="exported".into(),
            Err(error)=>{record.status=if matches!(record.status.as_str(),"stopping_source"|"exporting_source"){"stop_unknown"}else{"incomplete_copy"}.into();record.error=Some(error.clone());self.archives.save(&mut record)?;return Err(error)}
        }
        self.archives.save(&mut record)?;Ok(json!(record))
    }
    fn inspect_archive_stop(&self,input:Selection)->Result<Value,String>{
        let mut record=self.archives.get(&input.id)?;
        let Operation::Export{backup}=&record.operation else{return Err("Tentativa de exportação inválida.".into())};
        if record.status!="stop_unknown"{return Err("Esta tentativa não aguarda confirmação de parada.".into())}
        if self.state()?.id!=backup.source_id{return Err("Identidade do ambiente divergiu.".into())}
        self.check_stop(&backup.source.name,&backup.source,&self.bound_location(&self.state()?)?)?;
        record.status="incomplete_copy".into();record.error=Some("Parada conferida. Prepare outra revisão para criar uma nova cópia.".into());
        self.archives.save(&mut record)?;Ok(json!(record))
    }
    fn backup_for_restore(&self,id:&str)->Result<(Backup,u64,String),String>{
        let record=self.archives.get(id)?;
        if record.status!="exported"{return Err("Escolha uma cópia com integridade confirmada.".into())}
        let Operation::Export{backup}=record.operation else{return Err("Registro de origem inválido.".into())};
        uuid(&backup.id)?;uuid(&backup.source_id)?;
        let sha=record.sha256.ok_or("Digest da cópia ausente.")?;
        if sha.len()!=64||!sha.bytes().all(|v|v.is_ascii_hexdigit()&&!v.is_ascii_uppercase()){return Err("Digest inválido.".into())}
        Ok((backup,record.bytes.ok_or("Tamanho da cópia ausente.")?,sha))
    }
    fn archive_import_plan(&self,input:Selection)->Result<Value,String>{
        let (backup,bytes,sha256)=self.backup_for_restore(&input.id)?;
        let id=Uuid::new_v4().to_string();
        let first_wsl_distribution=self.ensure_clone_absent(&id)?;
        let restore=Restore{id,backup,bytes,sha256,first_wsl_distribution};
        let _parent=node(&self.archive_dir(),true,false)?;
        let (mut disk,_)=node(&self.disk_path(&restore.backup.id,restore.backup.format)?,false,false)?;
        if archive_hash(&mut disk,restore.backup.format,bytes)?!=(bytes,restore.sha256.clone()){return Err("O arquivo da cópia mudou.".into())}
        self.save_plan(IMPORT,json!({"restore":restore,"distribution":clone_name(&restore.id),"target":display_path(&self.recovery_dir(&restore.id)?),"effects":"Importar uma cópia WSL 2 separada, atribuir nova identidade e encerrar a cópia. O executor permanece vinculado ao ambiente atual."}))
    }
    fn ensure_clone_absent(&self,id:&str)->Result<bool,String>{
        let inventory=self.inventory()?;
        let target=self.recovery_dir(id)?;
        if inventory.iter().any(|r|r.name.eq_ignore_ascii_case(&clone_name(id))||same_path(&r.base_path,&target)){
            return Err("O destino de recuperação já está registrado.".into())
        }
        must_be_new(&target)?;Ok(inventory.is_empty())
    }
    fn clone_registration(&self,id:&str)->Result<Registration,String>{
        let target=self.recovery_dir(id)?;let name=clone_name(id);let inventory=self.inventory()?;
        let mut matches=inventory.iter().filter(|r|r.name.eq_ignore_ascii_case(&name)||same_path(&r.base_path,&target));
        let expected=matches.next().ok_or("A cópia ainda não tem um registro WSL verificável.")?;
        if matches.next().is_some()||expected.name!=name||expected.version!=2||!same_path(&expected.base_path,&target){
            return Err("O registro da cópia não corresponde ao destino revisado.".into())
        }
        Ok(expected.clone())
    }
    fn archive_import(&self,approval:Approval)->Result<Value,String>{
        let plan=self.consume_plan(approval,IMPORT)?;let restore:Restore=decode(plan.payload["restore"].clone())?;
        let expected=self.backup_for_restore(&restore.backup.id)?;
        if (restore.backup.clone(),restore.bytes,restore.sha256.clone())!=expected||restore.id==restore.backup.source_id{return Err("A origem ou a identidade da restauração mudou.".into())}
        let _base=node(&self.root,true,false)?;let _source=node(&self.archive_dir(),true,false)?;
        let archive=self.disk_path(&restore.backup.id,restore.backup.format)?;let (mut disk,_)=node(&archive,false,false)?;
        if archive_hash(&mut disk,restore.backup.format,restore.bytes)?!=(restore.bytes,restore.sha256.clone()){return Err("A integridade da cópia mudou antes da importação.".into())}
        let _parent=ensure_dir(&self.root.join("kali-recovery"))?;
        if self.ensure_clone_absent(&restore.id)? != restore.first_wsl_distribution {
            return Err("O inventário WSL mudou. Revise novamente o efeito sobre a distribuição padrão.".into());
        }
        let target=self.recovery_dir(&restore.id)?;fs::create_dir(&target).map_err(disk_error)?;let _dest=node(&target,true,false)?;
        let name=clone_name(&restore.id);
        let mut record=self.archives.begin(Operation::Import{restore:restore.clone()},"importing")?;
        let outcome=(||{
            let mut args=vec!["--import".into(),name.clone(),display_path(&target),display_path(&archive),"--version".into(),"2".into()];
            if restore.backup.format==ArchiveFormat::Vhdx {args.push("--vhd".into())}
            let result=self.wsl("WSL archive import",args,1800)?;
            checked(&result)?;
            let registration=self.clone_registration(&restore.id)?;
            if registration.guid==restore.backup.source.guid{return Err("A cópia recebeu o registro da origem.".into())}
            record.registration=Some(registration.clone());record.status="personalizing".into();self.archives.save(&mut record)?;
            if self.clone_registration(&restore.id)?!=registration{return Err("O registro mudou antes de conferir o marcador da cópia.".into())}
            let rekey=self.wsl("WSL archive new identity",vec!["--distribution".into(),name.clone(),"--user".into(),"root".into(),"--cd".into(),"/".into(),"--exec".into(),"/usr/bin/python3".into(),"-I".into(),"-c".into(),REKEY.into(),restore.backup.source_id.clone(),restore.id.clone()],45);
            // The reviewed operation ends the owned clone even after a failed
            // marker check. Never terminate a replacement registration.
            if self.clone_registration(&restore.id)?!=registration{return Err("O registro da cópia mudou durante a verificação.".into())}
            record.status="stopping_copy".into();self.archives.save(&mut record)?;
            checked(&self.wsl("WSL archive stop copy",vec!["--terminate".into(),name.clone()],60)?)?;
            self.check_stop(&name,&registration,&target)?;
            let rekey=rekey?;checked(&rekey)?;
            if rekey.stdout.trim()!=restore.id || !rekey.stderr.trim().is_empty(){return Err("O marcador da cópia não foi confirmado.".into())}
            Ok::<(),String>(())
        })();
        match outcome {
            Ok(())=>record.status="inactive".into(),
            Err(error)=>{record.status="copy_unknown".into();record.error=Some(error.clone());self.archives.save(&mut record)?;return Err(error)}
        }
        self.archives.save(&mut record)?;Ok(json!(record))
    }
}

#[cfg(test)]
#[path="environment_archive_tests.rs"]
mod tests;

#[path = "environment_switch.rs"]
mod switch;
