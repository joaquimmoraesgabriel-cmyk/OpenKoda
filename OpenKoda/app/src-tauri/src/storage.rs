//! Stable application data outside a parent MSIX application's virtualized AppData.
//! Legacy SQLite data is copied, checked and atomically installed; never deleted.
use std::{fs, path::{Path, PathBuf}, time::{Duration, Instant}};
use rusqlite::{backup::{Backup, StepResult}, Connection, OpenFlags};

const DATABASES: [&str;5] = ["openkoda.sqlite3", "assistant.sqlite3", "environment.sqlite3", "kali-bridge.sqlite3", "remediation.sqlite3"];

/// Keep this value alive for the entire application lifetime (app.manage).
/// It serializes profile selection/activation across all profiles in this base.
pub struct StorageLease {
    pub base:PathBuf,
    pub root:PathBuf,
    pub boot_notice:Option<String>,
    _global_lock:fs::File,
}

pub fn open(home:&Path,legacy:&Path,override_root:Option<&Path>)->Result<StorageLease,String> {
    let base=if let Some(path)=override_root {
        if !path.is_absolute(){return Err("Diretório de dados de teste deve ser absoluto.".into());}
        fs::create_dir_all(path).map_err(|_|"Não foi possível preparar os dados de teste.")?;
        plain(path,true)?;path.to_path_buf()
    }else{prepare(home,legacy)?};
    let base=base.canonicalize().map_err(|_|"Não foi possível resolver a base de perfis.")?;
    let lock_path=base.join("profiles.lock");
    if lock_path.exists(){plain(&lock_path,false)?;}
    let mut options=fs::OpenOptions::new();options.create(true).read(true).write(true);
    #[cfg(windows)] {use std::os::windows::fs::OpenOptionsExt;options.share_mode(0);}
    let global_lock=options.open(lock_path).map_err(|_|"A base de perfis está em uso por outra instância; nenhuma ativação foi iniciada.")?;
    let selected=crate::activation::boot(&base)?;
    Ok(StorageLease{base,root:selected.root,boot_notice:selected.notice,_global_lock:global_lock})
}

pub fn prepare(home: &Path, legacy: &Path) -> Result<PathBuf,String> {
    let destination=home.join(".openkoda");
    if destination.exists() {
        plain(&destination, true)?;
        return Ok(destination);
    }
    if !legacy.join("openkoda.sqlite3").exists() {
        fs::create_dir(&destination).map_err(|_|"Não foi possível criar a pasta de dados no perfil do usuário.")?;
        return Ok(destination);
    }
    plain(legacy,true)?;
    let mut options=fs::OpenOptions::new();options.create(true).read(true).write(true);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let _legacy_lock=options.open(legacy.join("instance.lock")).map_err(|_|"A instalação anterior está em uso. Feche-a antes de importar seus dados.")?;
    let staging=home.join(format!(".openkoda-import-{}",uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|_|"Não foi possível preparar a importação local.")?;
    for name in DATABASES {
        let source=legacy.join(name);
        if !source.exists(){continue;}
        plain(&source,false)?;
        let source=Connection::open_with_flags(source,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_|"Banco anterior indisponível. A origem foi preservada.")?;
        let mut target=Connection::open(staging.join(name)).map_err(|_|"Não foi possível criar a cópia de dados.")?;
        let backup=Backup::new(&source,&mut target).map_err(|_|"Falha ao iniciar cópia consistente do banco.")?;
        let deadline=Instant::now()+Duration::from_secs(30);
        loop {
            if Instant::now()>deadline{return Err("A importação não concluiu no prazo; a origem foi preservada.".into());}
            match backup.step(128).map_err(|_|"Falha na cópia consistente; origem preservada.")? {
                StepResult::Done=>break,
                StepResult::More=>{},
                StepResult::Busy|StepResult::Locked=>std::thread::sleep(Duration::from_millis(20)),
                _=>return Err("Resultado inesperado na importação.".into()),
            }
        }
        drop(backup);
        let integrity:String=target.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(|_|"Falha ao verificar a cópia do banco.")?;
        if integrity!="ok" {return Err("A cópia do banco não passou na verificação. A origem foi preservada.".into());}
    }
    // Validates the canonical domain and marks interrupted operations unknown.
    // No executor or inference is started by opening a Store.
    drop(crate::core::Store::open(staging.clone())?);
    let receipt=serde_json::json!({"importedAt":crate::model::now(),"legacyDirectory":legacy,
        "sourcePreserved":true,"databases":DATABASES,
        "limits":"Exportações, backups anteriores e caches permanecem na pasta de origem. A distro Kali exige revalidação do destino antes de operar."});
    fs::write(staging.join("import.json"),serde_json::to_vec_pretty(&receipt).map_err(|_|"Falha ao registrar importação.")?).map_err(|_|"Falha ao registrar importação.")?;
    if destination.exists(){return Err("A pasta de destino foi criada durante a importação. Nenhum dado existente foi substituído.".into());}
    fs::rename(&staging,&destination).map_err(|_|"Não foi possível finalizar a importação. Origem e cópia de preparação foram preservadas.")?;
    Ok(destination)
}

fn plain(path:&Path,directory:bool)->Result<(),String>{
    let metadata=fs::symlink_metadata(path).map_err(|_|"Não foi possível verificar o caminho de dados.")?;
    let mut link=metadata.file_type().is_symlink();
    #[cfg(windows)] {use std::os::windows::fs::MetadataExt;link|=metadata.file_attributes()&0x400!=0;}
    if link||metadata.is_dir()!=directory||(!directory&&!metadata.is_file()){return Err("O caminho de dados não pode ser um link ou tipo de arquivo inesperado.".into());}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn imports_wal_snapshot_without_removing_source_or_reimporting(){
        let temp=tempfile::tempdir().unwrap();let home=temp.path().join("home");let legacy=temp.path().join("legacy");fs::create_dir(&home).unwrap();
        let store=crate::core::Store::open(legacy.clone()).unwrap();
        store.mutate(|s|{s.settings.model="first-model".into();Ok(())}).unwrap();drop(store);
        let assistant=Connection::open(legacy.join("assistant.sqlite3")).unwrap();assistant.execute_batch("CREATE TABLE vault (cipher BLOB); INSERT INTO vault VALUES(X'010203');").unwrap();drop(assistant);
        let destination=prepare(&home,&legacy).unwrap();assert_eq!(destination,home.join(".openkoda"));
        assert!(legacy.join("openkoda.sqlite3").exists());
        let copy=crate::core::Store::open(destination.clone()).unwrap();assert_eq!(copy.snapshot().unwrap().settings.model,"first-model");
        copy.mutate(|s|{s.settings.model="edited-model".into();Ok(())}).unwrap();drop(copy);
        assert_eq!(prepare(&home,&legacy).unwrap(),destination);
        assert_eq!(crate::core::Store::open(destination).unwrap().snapshot().unwrap().settings.model,"edited-model");
    }
    #[test] fn invalid_legacy_is_preserved_without_installing_destination(){
        let temp=tempfile::tempdir().unwrap();let home=temp.path().join("home");let legacy=temp.path().join("legacy");fs::create_dir(&home).unwrap();fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("openkoda.sqlite3"),b"invalid database").unwrap();
        assert!(prepare(&home,&legacy).is_err());assert!(!home.join(".openkoda").exists());
        assert_eq!(fs::read(legacy.join("openkoda.sqlite3")).unwrap(),b"invalid database");
    }
    #[test] fn empty_installation_uses_stable_home(){
        let temp=tempfile::tempdir().unwrap();assert_eq!(prepare(temp.path(),&temp.path().join("absent")).unwrap(),temp.path().join(".openkoda"));
    }
    #[cfg(windows)]
    #[test] fn live_legacy_instance_blocks_import(){
        let temp=tempfile::tempdir().unwrap();let home=temp.path().join("home");fs::create_dir(&home).unwrap();let legacy=temp.path().join("legacy");
        let _store=crate::core::Store::open(legacy.clone()).unwrap();assert!(prepare(&home,&legacy).is_err());assert!(!home.join(".openkoda").exists());
    }
}
