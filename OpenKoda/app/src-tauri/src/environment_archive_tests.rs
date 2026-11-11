// These fixtures implement the fixed WSL verbs in memory. The source and recovered
// VHDX paths are fresh temp files containing a recognizable synthetic signature.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct Laboratory {
    source: Registration,
    clone: Mutex<Option<Registration>>,
    calls: Mutex<Vec<(String,Vec<String>)>>,
    running: Mutex<Vec<String>>,
    lose_stop: AtomicBool,
    bad_clone: AtomicBool,
    raw: AtomicBool,
    wrong_marker: AtomicBool,
    export_loses_stop: AtomicBool,
    remove_original_and_others: AtomicBool,
}
fn response(stdout:String)->ProcessResult{
    ProcessResult{stdout,stderr:String::new(),code:0,timed_out:false,truncated:false}
}
impl Runner for Laboratory {
    fn export_tar(&self, distribution: &str, target: File, limit: u64) -> Result<(u64, String), String> {
        self.calls.lock().unwrap().push(("WSL archive export".into(), vec![distribution.into()]));
        assert!(!self.running.lock().unwrap().contains(&DISTRO.to_string()));
        if self.export_loses_stop.load(Ordering::SeqCst) {
            self.running.lock().unwrap().push(DISTRO.to_string());
            return Err("fixture interrupted".into());
        }
        let mut tar = vec![0u8; 1536];
        tar[257..263].copy_from_slice(b"ustar\0");
        stream_archive(tar.as_slice(),target,limit)
    }
    fn run(&self,spec:&CommandSpec)->Result<ProcessResult,String>{
        self.calls.lock().unwrap().push((spec.purpose.into(),spec.args.clone()));
        let text=match spec.purpose {
            "WSL registry"=>{
                let mut inventory=vec![self.source.clone(),Registration{guid:"external".into(),name:"PersonalLinux".into(),base_path:"C:\\personal".into(),version:2}];
                if self.remove_original_and_others.load(Ordering::SeqCst){inventory.clear();}
                if let Some(r)=self.clone.lock().unwrap().as_ref(){inventory.push(r.clone())}
                json!(inventory).to_string()
            }
            "WSL archive spool inventory"=>json!({"rawCaptures":u32::from(self.raw.load(Ordering::SeqCst)),"terminalCaptures":1,"sessionRecords":3}).to_string(),
            "probe"=>{
                let wire:Value=serde_json::from_slice(spec.input.as_deref().unwrap()).unwrap();
                let id=if self.wrong_marker.load(Ordering::SeqCst){Value::Null}else{wire["identity"].clone()};
                json!({"ok":true,"result":{"environmentId":id,"user":"openkoda","maintenance":null,"uid":1000,"interopEnabled":false,"windowsMounts":[]}}).to_string()
            }
            "WSL archive stop source"|"WSL archive stop copy"=>{
                assert_eq!(spec.args[0],"--terminate");
                assert_ne!(spec.args[1],"PersonalLinux");
                self.running.lock().unwrap().retain(|n|n!=&spec.args[1]);
                if self.lose_stop.swap(false,Ordering::SeqCst){
                    return Ok(ProcessResult{timed_out:true,..response(String::new())})
                }
                String::new()
            }
            "WSL archive running inventory"=>self.running.lock().unwrap().join("\n"),
            "WSL archive import"=>{
                assert_eq!(spec.args[0],"--import");assert!(spec.args[1].starts_with("OpenKoda-Recovery-"));
                if spec.args[3].ends_with(".vhdx") {assert_eq!(&spec.args[4..], ["--version","2","--vhd"]);}
                else {assert_eq!(&spec.args[4..], ["--version","2"]);}
                assert!(fs::metadata(&spec.args[3]).unwrap().len()>8);
                self.clone.lock().unwrap().replace(Registration{name:spec.args[1].clone(),guid:"new-registration".into(),base_path:if self.bad_clone.load(Ordering::SeqCst){"C:\\personal".into()}else{spec.args[2].clone()},version:2});
                String::new()
            }
            "WSL activation runtime check"=>{
                assert_eq!(spec.args[0],"--distribution");
                let id=spec.args[1].strip_prefix("OpenKoda-Recovery-").unwrap();
                assert_eq!(&spec.args[2..4],["--user","openkoda"]);
                return Ok(ProcessResult{stderr:"wsl: Failed to start the systemd user session for 'openkoda'. See journalctl for more details.".into(),..response(json!({"uid":1000,"user":"openkoda","marker":id}).to_string())})
            }
            "WSL archive new identity"=>{
                assert_ne!(spec.args[1],DISTRO);
                assert_eq!(self.clone.lock().unwrap().as_ref().unwrap().name,spec.args[1]);
                assert_ne!(spec.args[spec.args.len()-2],*spec.args.last().unwrap());
                self.running.lock().unwrap().push(spec.args[1].clone());
                spec.args.last().unwrap().clone()
            }
            _=>return Err(format!("Processo inesperado: {}",spec.purpose)),
        };
        Ok(response(text))
    }
}
struct Fixture { temp:tempfile::TempDir, manager:EnvironmentManager, runner:Arc<Laboratory> }
impl Fixture{
    fn new()->Self{
        let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();
        let location=root.join("kali-distro");fs::create_dir(&location).unwrap();
        fs::write(location.join("ext4.vhdx"),b"vhdxfileSYNTHETIC_DISK").unwrap();
        let source=Registration{guid:"owned".into(),name:DISTRO.into(),base_path:display_path(&location),version:2};
        let runner=Arc::new(Laboratory{source:source.clone(),clone:Mutex::new(None),calls:Mutex::new(Vec::new()),
            running:Mutex::new(vec!["PersonalLinux".into(),DISTRO.into()]),lose_stop:AtomicBool::new(false),bad_clone:AtomicBool::new(false),raw:AtomicBool::new(false),wrong_marker:AtomicBool::new(false),export_loses_stop:AtomicBool::new(false),remove_original_and_others:AtomicBool::new(false)});
        let manager=EnvironmentManager::with_runner(root,runner.clone()).unwrap();
        let mut state=manager.state().unwrap();state.phase="ready".into();state.registration=Some(source);manager.save_state(&state).unwrap();
        Self{temp,manager,runner}
    }
    fn prepare(&self)->Value{self.manager.handle("environment.archive.exportPlan",json!({})).unwrap()}
    fn approve(&self,action:&str,plan:&Value)->Result<Value,String>{self.manager.handle(action,json!({"planId":plan["id"],"digest":plan["digest"]}))}
    fn export(&self)->Value{self.approve("environment.archive.export",&self.prepare()).unwrap()}
    fn import_plan(&self,copy:&Value)->Value{self.manager.handle("environment.archive.importPlan",json!({"id":copy["operation"]["backup"]["id"]})).unwrap()}
    fn calls(&self,purpose:&str)->usize{self.runner.calls.lock().unwrap().iter().filter(|item|item.0==purpose).count()}
}
#[test]
fn archive_restores_a_verified_copy_with_new_marker_and_inactive_registration(){
    let f=Fixture::new();let state=f.manager.state().unwrap();let plan=f.prepare();
    assert_eq!(f.calls("WSL archive stop source"),0);assert_eq!(f.calls("WSL archive import"),0);
    assert!(!f.manager.archive_dir().exists());let archived=f.approve("environment.archive.export",&plan).unwrap();
    assert_eq!(archived["status"],"exported");
    assert_eq!(archived["operation"]["backup"]["format"],"tar"); assert_eq!(f.calls("WSL archive export"),1);
    assert!(f.approve("environment.archive.export",&plan).is_err());assert_eq!(f.calls("WSL archive stop source"),1);
    let restore_plan=f.import_plan(&archived);assert_eq!(f.calls("WSL archive import"),0);
    let result=f.approve("environment.archive.import",&restore_plan).unwrap();
    assert_eq!(result["status"],"inactive");
    assert_ne!(result["operation"]["restore"]["id"],state.id);
    assert_eq!(f.manager.state().unwrap().id,state.id);assert_eq!(f.manager.state().unwrap().registration,state.registration);
    assert!(f.approve("environment.archive.import",&restore_plan).is_err());
    assert_eq!(f.calls("WSL archive import"),1);assert_eq!(f.calls("WSL archive new identity"),1);
    assert_eq!(&*f.runner.running.lock().unwrap(),&["PersonalLinux".to_string()]);
    assert_eq!(f.manager.archives.page(None).unwrap().0.len(),2);
}
#[test]
fn changed_archive_refuses_import_before_any_wsl_process(){
    let f=Fixture::new();let saved=f.export();let plan=f.import_plan(&saved);
    let id=saved["operation"]["backup"]["id"].as_str().unwrap();fs::write(f.manager.disk_path(id,ArchiveFormat::Tar).unwrap(),b"vhdxfileFORGED_PAYLOAD").unwrap();
    let before=f.runner.calls.lock().unwrap().len();
    assert!(f.approve("environment.archive.import",&plan).is_err());
    assert_eq!(f.runner.calls.lock().unwrap().len(),before);assert_eq!(f.calls("WSL archive import"),0);
}
#[test]
fn uncertain_stop_is_persistent_and_can_be_observed_without_repeating_termination(){
    let f=Fixture::new();let plan=f.prepare();f.runner.lose_stop.store(true,Ordering::SeqCst);
    assert!(f.approve("environment.archive.export",&plan).is_err());assert!(f.manager.has_unresolved_maintenance().unwrap());
    let reopened=EnvironmentManager::with_runner(f.temp.path().into(),f.runner.clone()).unwrap();
    assert!(reopened.executor_owned_identity().is_err());assert_eq!(reopened.handle("environment.status",json!({})).unwrap()["phase"],"archive_stop_unknown");
    let id=plan["payload"]["backup"]["id"].clone();
    assert_eq!(reopened.handle("environment.archive.inspectStop",json!({"id":id})).unwrap()["status"],"incomplete_copy");
    assert_eq!(f.calls("WSL archive stop source"),1);assert!(!reopened.has_unresolved_maintenance().unwrap());
    assert!(reopened.handle("environment.archive.importPlan",json!({"id":id})).is_err());
}
#[test]
fn archive_rechecks_the_registration_before_entering_a_restored_clone(){
    let f=Fixture::new();let export=f.export();let plan=f.import_plan(&export);f.runner.bad_clone.store(true,Ordering::SeqCst);
    assert!(f.approve("environment.archive.import",&plan).is_err());
    assert_eq!(f.calls("WSL archive new identity"),0);assert_eq!(f.calls("WSL archive stop copy"),0);
    assert_eq!(f.manager.archives.page(None).unwrap().0[0].status,"copy_unknown");assert_eq!(f.manager.state().unwrap().phase,"ready");
}
#[test]
fn preexisting_archive_target_and_caller_paths_are_refused(){
    let f=Fixture::new();
    assert!(f.manager.handle("environment.archive.exportPlan",json!({"path":"C:\\personal"})).is_err());
    let plan=f.prepare();let target=PathBuf::from(plan["payload"]["target"].as_str().unwrap());
    fs::create_dir_all(target.parent().unwrap()).unwrap();fs::write(&target,b"original").unwrap();
    assert!(f.approve("environment.archive.export",&plan).is_err());
    assert_eq!(fs::read(target).unwrap(),b"original");assert_eq!(f.calls("WSL archive stop source"),0);
}

#[test]
fn saved_disk_can_create_an_inactive_recovery_registration_when_original_is_gone(){
    let f=Fixture::new();let exported=f.export();
    f.runner.remove_original_and_others.store(true,Ordering::SeqCst);f.runner.running.lock().unwrap().clear();
    let mut state=f.manager.state().unwrap();state.phase="degraded".into();f.manager.save_state(&state).unwrap();
    let probe_count=f.calls("probe");
    let plan=f.import_plan(&exported);
    assert_eq!(plan["payload"]["restore"]["firstWslDistribution"],true);
    let restored=f.approve("environment.archive.import",&plan).unwrap();
    assert_eq!(restored["status"],"inactive");assert_eq!(f.calls("probe"),probe_count);
    assert_eq!(f.manager.state().unwrap().phase,"degraded");assert!(f.manager.executor_owned_identity().is_err());
}

#[test]
fn earlier_verified_disk_restores_after_another_export_lost_its_source() {
    let f = Fixture::new();
    let exported = f.export();
    let interrupted = f.prepare();
    f.runner.lose_stop.store(true, Ordering::SeqCst);
    assert!(f.approve("environment.archive.export", &interrupted).is_err());
    let id = &interrupted["payload"]["backup"]["id"];

    f.runner.remove_original_and_others.store(true, Ordering::SeqCst);
    f.runner.running.lock().unwrap().clear();
    assert!(f.manager.handle("environment.archive.inspectStop", json!({"id": id})).is_err());
    assert!(f.manager.handle("environment.archive.exportPlan", json!({})).is_err());
    assert!(f.manager.handle("environment.archive.importPlan", json!({"id": id})).is_err());

    let probes = f.calls("probe");
    let source_stops = f.calls("WSL archive stop source");
    let restore = f.import_plan(&exported);
    assert_eq!(f.approve("environment.archive.import", &restore).unwrap()["status"], "inactive");
    assert_eq!(f.calls("probe"), probes);
    assert_eq!(f.calls("WSL archive stop source"), source_stops);
    assert!(f.manager.has_unresolved_maintenance().unwrap());
    assert!(f.manager.executor_owned_identity().is_err());
}

#[test]
fn raw_bytes_appearing_after_preview_stop_export_before_source_termination(){
    let f=Fixture::new();let plan=f.prepare();assert_eq!(plan["payload"]["backup"]["spool"]["terminalCaptures"],1);
    f.runner.raw.store(true,Ordering::SeqCst);
    assert!(f.approve("environment.archive.export",&plan).unwrap_err().contains("retenção"));
    assert_eq!(f.calls("WSL archive stop source"),0);assert!(f.manager.archives.page(None).unwrap().0.is_empty());
}

#[test]
fn streaming_limit_prevents_writing_the_oversized_block() {
    let mut target = Vec::new();
    assert!(stream_archive(&b"unreviewed extra bytes"[..], &mut target, 4).is_err());
    assert!(target.is_empty());
}

#[test]
fn previous_archive_receipts_preserve_the_vhdx_format() {
    let f=Fixture::new();
    let plan=f.prepare();
    let mut saved=plan["payload"]["backup"].clone();
    saved.as_object_mut().unwrap().remove("format");
    let previous:Backup=serde_json::from_value(saved).unwrap();
    assert_eq!(previous.format,ArchiveFormat::Vhdx);
    let path=f.manager.disk_path(&previous.id,previous.format).unwrap();
    assert_eq!(path.extension().unwrap(), "vhdx");
}

#[test]
fn tar_export_interrupted_while_source_is_running_requires_stop_inspection() {
    let f=Fixture::new();let review=f.prepare();
    f.runner.export_loses_stop.store(true,Ordering::SeqCst);
    assert!(f.approve("environment.archive.export",&review).is_err());
    assert!(f.manager.has_unresolved_maintenance().unwrap());
    assert_eq!(f.manager.archives.page(None).unwrap().0[0].status,"stop_unknown");
    assert_eq!(f.calls("WSL archive stop source"),1);
}

#[test]
fn archived_attempts_are_pageable_and_do_not_displace_a_verified_restore() {
    let f = Fixture::new();
    let export = f.export();
    let original = export["operation"]["backup"]["id"].as_str().unwrap();
    let mut filler = export.clone();
    filler["status"] = json!("incomplete_copy");
    let db = f.manager.archives.db().unwrap();
    for _ in 0..PAGE_SIZE {
        let id = Uuid::new_v4().to_string();
        filler["operation"]["backup"]["id"] = json!(id);
        db.execute("INSERT INTO archive_ops(id,data) VALUES(?1,?2)", params![id,filler.to_string()]).unwrap();
    }
    drop(db);
    let list = |args| f.manager.handle("environment.archive.list", args);
    let first = list(json!({})).unwrap();
    assert_eq!(first["records"].as_array().unwrap().len(), PAGE_SIZE);
    let plan = f.import_plan(&export);
    assert_eq!(f.approve("environment.archive.import", &plan).unwrap()["status"], "inactive");
    let next = list(json!({"beforeId": first["nextBeforeId"]})).unwrap();
    assert_eq!(next["records"].as_array().unwrap().len(), 1);
    assert_eq!(next["records"][0]["operation"]["backup"]["id"], original);
    assert!(next["nextBeforeId"].is_null());
    assert!(list(json!({"beforeId":Uuid::new_v4().to_string()})).is_err());
    assert!(list(json!({"beforeId":"invalid"})).is_err());
    // An interrupted export older than the visible page still blocks another
    // stop. Reopening the journal reconciles it without touching the source.
    let mut interrupted = f.manager.archives.get(original).unwrap();
    interrupted.status = "exporting_source".into();
    f.manager.archives.save(&mut interrupted).unwrap();
    let reopened = ArchiveStore::open(f.temp.path()).unwrap();
    assert_eq!(reopened.get(original).unwrap().status, "stop_unknown");
    assert!(reopened.stop_pending().unwrap());
    assert!(f.manager.handle("environment.archive.exportPlan", json!({})).is_err());
}

#[test]
fn switch_revalidates_the_clone_and_binds_executor_and_refuses_old_grants_after_restart() {
    let f=Fixture::new();
    let original=f.manager.state().unwrap();
    let exported=f.export();let import=f.import_plan(&exported);
    let clone=f.approve("environment.archive.import",&import).unwrap();
    let id=clone["operation"]["restore"]["id"].as_str().unwrap();
    let name=clone["registration"]["name"].as_str().unwrap();
    let call_count=f.calls("probe");
    let plan=f.manager.handle("environment.archive.activatePlan",json!({"id":id})).unwrap();
    assert_eq!(call_count,f.calls("probe"));
    assert_eq!(f.manager.launch_distribution(&original.id).unwrap(),DISTRO);
    assert_eq!(plan["payload"]["targetDistribution"],name);
    let activated=f.approve("environment.archive.activate",&plan).unwrap();
    assert_eq!(activated["environmentId"],id);
    assert!(f.approve("environment.archive.activate",&plan).is_err());
    assert!(f.manager.launch_distribution(&original.id).is_err());
    assert_eq!(f.manager.data_review_identity().unwrap(),id);
    let reopened=EnvironmentManager::with_runner(f.temp.path().into(),f.runner.clone()).unwrap();
    assert_eq!(reopened.launch_distribution(id).unwrap(),name);
    assert!(reopened.launch_distribution(&original.id).is_err());
    let status=reopened.handle("environment.diagnose",json!({})).unwrap();
    assert_eq!(status["distro"],name);assert_eq!(status["environmentId"],id);assert_eq!(status["status"],"ready");
    let calls=f.runner.calls.lock().unwrap();
    assert!(calls.iter().rev().find(|(purpose,_)|purpose=="probe").unwrap().1.windows(2).any(|p|p==["--distribution",name]));
    assert!(!calls.iter().any(|(_,a)|a.iter().any(|v|v=="--set-default"||v=="--unregister")));drop(calls);
    let rows=reopened.handle("environment.archive.list",json!({})).unwrap();
    assert!(rows["records"].as_array().unwrap().iter().any(|r|r["activationState"]=="active"));
    assert_eq!(reopened.inventory().unwrap()[0],original.registration.unwrap());
}

#[test]
fn a_different_clone_marker_blocks_switch_and_preserves_previous_executor() {
    let f=Fixture::new();let original=f.manager.state().unwrap();
    let exported=f.export();let restored=f.approve("environment.archive.import",&f.import_plan(&exported)).unwrap();
    let id=restored["operation"]["restore"]["id"].as_str().unwrap();
    let plan=f.manager.handle("environment.archive.activatePlan",json!({"id":id})).unwrap();
    f.runner.wrong_marker.store(true,Ordering::SeqCst);
    assert!(f.approve("environment.archive.activate",&plan).is_err());
    assert_eq!(f.manager.launch_distribution(&original.id).unwrap(),DISTRO);
    assert!(f.manager.launch_distribution(id).is_err());
}

#[test]
fn activating_after_loss_uses_only_the_receipt_and_the_recovery_registration() {
    let f=Fixture::new();let saved=f.export();
    f.runner.remove_original_and_others.store(true,Ordering::SeqCst);f.runner.running.lock().unwrap().clear();
    let mut source=f.manager.state().unwrap();source.phase="degraded".into();f.manager.save_state(&source).unwrap();
    let restored=f.approve("environment.archive.import",&f.import_plan(&saved)).unwrap();
    let id=restored["operation"]["restore"]["id"].as_str().unwrap();
    let review=f.manager.handle("environment.archive.activatePlan",json!({"id":id})).unwrap();
    assert_eq!(f.approve("environment.archive.activate",&review).unwrap()["environmentId"],id);
    assert!(f.manager.launch_distribution(&source.id).is_err());
}

#[test]
fn environment_migration_preserves_v0_state_and_rejects_unknown_versions_without_writing() {
    let f=Fixture::new();let existing=f.manager.state().unwrap();
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("environment.sqlite3");
    let db=Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE environment_state(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);").unwrap();
    db.execute("INSERT INTO environment_state VALUES(1,?1)",[serde_json::to_string(&existing).unwrap()]).unwrap();
    drop(db);
    let migrated=EnvironmentManager::with_runner(dir.path().into(),f.runner.clone()).unwrap();assert_eq!(migrated.state().unwrap().id,existing.id);drop(migrated);
    let db=Connection::open(&path).unwrap();let version:i64=db.pragma_query_value(None,"user_version",|r|r.get(0)).unwrap();
    assert_eq!(version,2);db.pragma_update(None,"user_version",3).unwrap();drop(db);
    let before=fs::read(&path).unwrap();assert!(EnvironmentManager::with_runner(dir.path().into(),f.runner.clone()).is_err());
    assert_eq!(before,fs::read(&path).unwrap());
}


#[test]
fn unrelated_wsl_stderr_still_blocks_an_unprivileged_archive_response(){
    let notice="wsl: Failed to start the systemd user session for 'openkoda'. See journalctl for more details.";
    assert!(user_stderr_ok(notice));assert!(user_stderr_ok(""));
    assert!(!user_stderr_ok(&format!("{notice}\naccess denied")));
    assert!(!user_stderr_ok(&notice.replace("openkoda","root")));
}
