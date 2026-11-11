//! Retire only the active, owned executor. Windows performs any unregistration
//! under the user's control. Reconciliation never launches a removal command.
use super::*;

const KIND: &str = "kali-guided-removal";
pub(crate) const REMOVAL_TABLE: (&str, &str) = ("environment_removals",
    "CREATE TABLE environment_removals(plan_id TEXT PRIMARY KEY, environment_id TEXT NOT NULL UNIQUE, data TEXT NOT NULL)");
pub(super) fn supports(action: &str) -> bool {
    matches!(action, "environment.removal.status" | "environment.removal.plan" |
        "environment.removal.detach" | "environment.removal.inspect" | "environment.removal.keep")
}

pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> Result<(), String> {
    tx.execute_batch(REMOVAL_TABLE.1).map_err(db_error)?;
    tx.pragma_update(None, "user_version", 2).map_err(db_error)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Removal {
    plan_id: String,
    digest: String,
    environment_id: String,
    registration: Registration,
    detached_at: String,
    observed_at: Option<String>,
    observation: String,
    path_status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Confirm {
    plan_id: String,
    digest: String,
    confirm_name: String,
    accept_linux_data_loss: bool,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Keep { environment_id: String, keep_distribution: bool }

fn vacant_state(state: &EnvironmentState) -> EnvironmentState {
    let mut vacant=state.clone();
    vacant.id=uuid::Uuid::new_v4().to_string();
    vacant.phase="absent".into();
    vacant.registration=None;
    vacant.last_error=None;
    vacant.last_output.clear();
    vacant.updated_at=now();
    vacant.install_exit=None;
    vacant.host_checkpoint=None;
    vacant
}

fn db_error(error: impl std::fmt::Display) -> String { error.to_string() }
fn path_key(path: &str) -> String {
    path.trim_start_matches(r"\\?\").trim_end_matches(['/', '\\']).replace('/', "\\").to_lowercase()
}

impl EnvironmentManager {
    fn last_removal(&self) -> Result<Option<Removal>, String> {
        let raw: Option<String> = self.connection.lock().map_err(db_error)?.query_row(
            "SELECT data FROM environment_removals ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)
        ).optional().map_err(db_error)?;
        raw.map(|r| serde_json::from_str(&r).map_err(db_error)).transpose()
    }

    fn removal_identity(&self, state: &EnvironmentState) -> Result<Registration, String> {
        self.archives.check_source_stop()?;
        let registration = self.owned_registration(state)?;
        uuid::Uuid::parse_str(registration.guid.trim_matches(['{','}']))
            .map_err(|_| "GUID de registro WSL inválido.")?;
        let probe = self.helper(state, "probe", vec![], 45)?;
        if probe["environmentId"] != state.id || probe["user"] != "openkoda" || !healthy_probe(&probe) {
            return Err("O marcador, o usuário ou a manutenção do Kali não conferem.".into());
        }
        self.owned_registration(state)
    }

    fn removal_payload(&self, state: &EnvironmentState) -> Result<Value, String> {
        if state.phase != "ready" { return Err("Selecione um executor gerido pronto para revisar a retirada.".into()); }
        let registration = self.removal_identity(state)?;
        let spool=self.spool_inventory(state)?;
        Ok(json!({"environmentId":state.id,"registration":registration,"spool":spool,"stateSha256":digest(&serde_json::to_value(state).map_err(db_error)?),
            "effects":["O OpenKoda suspende o executor e as permissões vinculadas à sua identidade.",
                "A exclusão no Windows pode apagar os arquivos, pacotes e configurações Linux desta distribuição.",
                "As capturas e os registros presentes apenas no Linux podem ser perdidos. Os bancos, as evidências já salvas e as cópias TAR no Windows permanecem disponíveis."],
            "exportAction":"environment.archive.exportPlan"}))
    }

    fn store_removal(&self, row: &Removal, before: &EnvironmentState, after: Option<&EnvironmentState>, first: bool) -> Result<(), String> {
        let mut conn = self.connection.lock().map_err(db_error)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(db_error)?;
        let state: String = tx.query_row("SELECT data FROM environment_state WHERE id=1", [], |r| r.get(0)).map_err(db_error)?;
        let stored: EnvironmentState = serde_json::from_str(&state).map_err(db_error)?;
        if serde_json::to_value(stored).map_err(db_error)? != serde_json::to_value(before).map_err(db_error)? {
            return Err("O estado do executor mudou durante a conferência.".into());
        }
        let data = serde_json::to_string(row).map_err(db_error)?;
        if first {
            tx.execute("INSERT INTO environment_removals(plan_id,environment_id,data) VALUES(?1,?2,?3)",
                params![row.plan_id,row.environment_id,data]).map_err(db_error)?;
        } else if tx.execute("UPDATE environment_removals SET data=?1 WHERE plan_id=?2 AND environment_id=?3",
            params![data,row.plan_id,row.environment_id]).map_err(db_error)? != 1 {
            return Err("Registro de retirada ausente.".into());
        }
        if let Some(state) = after {
            tx.execute("UPDATE environment_state SET data=?1 WHERE id=1",
                [serde_json::to_string(state).map_err(db_error)?]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    }

    fn detach_executor(&self, p: Confirm) -> Result<Value, String> {
        if !p.accept_linux_data_loss { return Err("Confirme o efeito da exclusão dos dados Linux.".into()); }
        if let Some(row) = self.last_removal()?.filter(|r|r.plan_id==p.plan_id) {
            if row.digest!=p.digest || row.registration.name!=p.confirm_name { return Err("A confirmação diverge da retirada registrada.".into()); }
            return Ok(json!({"record":row,"confirmedNow":false}));
        }
        let mut state = self.state()?;
        if state.registration.as_ref().map(|r|r.name.as_str()) != Some(p.confirm_name.as_str()) {
            return Err("Digite exatamente o nome da distribuição apresentada.".into());
        }
        let plan = self.consume_plan(Approval{plan_id:p.plan_id,digest:p.digest}, KIND)?;
        if self.removal_payload(&state)? != plan.payload {return Err("Identidade alterada. Revise outra retirada.".into());}
        let row = Removal{plan_id:plan.id,digest:plan.digest,environment_id:state.id.clone(),
            registration:state.registration.clone().ok_or("Registro gerido ausente.")?,detached_at:now(),
            observed_at:None,observation:"awaiting_windows".into(),path_status:None};
        let original = state.clone();
        state.phase="retiring".into();
        state.updated_at=now();
        self.store_removal(&row,&original,Some(&state),true)?;
        Ok(json!({"record":row,"confirmedNow":false}))
    }

    fn inspect_removal(&self) -> Result<Value,String> {
        let mut row=self.last_removal()?.ok_or("Nenhuma retirada foi registrada.")?;
        if matches!(row.observation.as_str(),"registration_absent"|"owner_released") {return Ok(json!({"record":row,"confirmedNow":false}));}
        let state=self.state()?;
        if state.phase!="retiring" || state.id!=row.environment_id || state.registration.as_ref()!=Some(&row.registration) {
            return Err("O executor não corresponde à retirada pendente.".into());
        }
        let registered=self.inventory()?;
        let binding=&row.registration;
        let mut matches=registered.iter().filter(|r| r.guid.eq_ignore_ascii_case(&binding.guid)
            || r.name.eq_ignore_ascii_case(&binding.name) || path_key(&r.base_path)==path_key(&binding.base_path));
        let found=matches.next();
        let mut next_state=None;
        let mut confirmed_now=false;
        row.observed_at=Some(now());
        if found.is_none() {
            let path=self.bound_location(&state)?;
            row.path_status=Some(match fs::symlink_metadata(path) {
                Ok(_)=>"remaining",Err(e) if e.kind()==std::io::ErrorKind::NotFound=>"absent",Err(_)=>"unknown",
            }.into());
            row.observation="registration_absent".into();
            // A future installation always has a fresh identity. Historical grants cannot apply to it.
            next_state=Some(vacant_state(&state));
        } else if found!=Some(binding) || matches.next().is_some() {
            row.observation="registration_conflict".into();
        } else {
            self.removal_identity(&state)?;
            row.observation="still_registered".into();
            confirmed_now=true;
        }
        self.store_removal(&row,&state,next_state.as_ref(),false)?;
        Ok(json!({"record":row,"confirmedNow":confirmed_now}))
    }

    fn keep_distribution(&self, input:Keep)->Result<Value,String>{
        let mut row=self.last_removal()?.ok_or("Nenhuma retirada registrada.")?;
        if row.environment_id!=input.environment_id || !input.keep_distribution {return Err("Confirme o encerramento do vínculo deste ambiente.".into());}
        if row.observation=="owner_released" {return Ok(json!({"record":row,"confirmedNow":false}));}
        let state=self.state()?;
        if state.phase!="retiring" || state.id!=row.environment_id || state.registration.as_ref()!=Some(&row.registration) {
            return Err("A retirada não pertence ao vínculo pendente.".into());
        }
        row.observation="owner_released".into();
        row.observed_at=Some(now());
        self.store_removal(&row,&state,Some(&vacant_state(&state)),false)?;
        Ok(json!({"record":row,"confirmedNow":false}))
    }

    pub(super) fn removal_action(&self, action: &str, payload: Value) -> Result<Value,String> {
        match action {
            "environment.removal.status"=>{empty(&payload)?;Ok(json!({"record":self.last_removal()?,"confirmedNow":false}))},
            "environment.removal.plan"=>{empty(&payload)?;self.save_plan(KIND,self.removal_payload(&self.state()?)?)},
            "environment.removal.detach"=>self.detach_executor(decode(payload)?),
            "environment.removal.inspect"=>{empty(&payload)?;self.inspect_removal()},
            "environment.removal.keep"=>self.keep_distribution(decode(payload)?),
            _=>Err("Ação de retirada desconhecida.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct SimulatedWsl {
        registry: Mutex<Vec<Registration>>,
        wrong_marker: Mutex<bool>,
        inventory_unavailable: Mutex<bool>,
        calls: Mutex<Vec<&'static str>>,
    }
    impl Runner for SimulatedWsl {
        fn run(&self, spec: &CommandSpec) -> Result<ProcessResult, String> {
            self.calls.lock().unwrap().push(spec.purpose);
            let value=match spec.purpose {
                "WSL registry" if *self.inventory_unavailable.lock().unwrap() => return Err("inventário indisponível".into()),
                "WSL registry"=>json!(*self.registry.lock().unwrap()),
                "WSL archive spool inventory"=>json!({"rawCaptures":1,"terminalCaptures":2,"sessionRecords":3}),
                "probe"=>{
                    let request:Value=serde_json::from_slice(spec.input.as_ref().unwrap()).unwrap();
                    assert!(spec.args.iter().all(|a| !matches!(a.as_str(),"--unregister"|"--terminate"|"--shutdown")));
                    let id=if *self.wrong_marker.lock().unwrap(){Value::Null}else{request["identity"].clone()};
                    json!({"ok":true,"result":{"environmentId":id,"user":"openkoda","uid":1000,"interopEnabled":false,"windowsMounts":[],"maintenance":null}})
                },
                other=>panic!("unexpected external effect: {other}"),
            };
            Ok(ProcessResult{code:0,stdout:value.to_string(),stderr:String::new(),timed_out:false,truncated:false})
        }
    }
    struct Lab {
        dir:tempfile::TempDir,
        wsl:Arc<SimulatedWsl>,
        manager:EnvironmentManager,
        registration:Registration,
    }
    impl Lab {
        fn new()->Self{
            let dir=tempfile::tempdir().unwrap();
            let wsl=Arc::new(SimulatedWsl::default());
            let manager=EnvironmentManager::with_runner(dir.path().into(),wsl.clone()).unwrap();
            fs::create_dir(manager.install_location()).unwrap();
            let registration=Registration{guid:uuid::Uuid::new_v4().to_string(),name:DISTRO.into(),version:2,base_path:display_path(&manager.install_location())};
            wsl.registry.lock().unwrap().push(registration.clone());
            let mut state=manager.state().unwrap();
            state.registration=Some(registration.clone());
            state.phase="ready".into();
            manager.save_state(&state).unwrap();
            Self{dir,wsl,manager,registration}
        }
        fn plan(&self)->Value{
            self.manager.handle("environment.removal.plan",json!({})).unwrap()
        }
        fn confirm(&self,p:&Value)->Value{
            json!({"planId":p["id"],"digest":p["digest"],"confirmName":DISTRO,"acceptLinuxDataLoss":true})
        }
        fn detach(&self)->Value{
            let p=self.plan();self.manager.handle("environment.removal.detach",self.confirm(&p)).unwrap()
        }
        fn inspect(&self)->Result<Value,String>{self.manager.handle("environment.removal.inspect",json!({}))}
    }
    #[test]
    fn refuses_arbitrary_input_foreign_registration_wrong_marker_and_missing_confirmation(){
        let f=Lab::new();
        assert!(f.manager.handle("environment.removal.plan",json!({"distro":"Ubuntu"})).is_err());
        let p=f.plan();
        for (field,value) in [("confirmName",json!("Ubuntu")),("acceptLinuxDataLoss",json!(false)),("distribution",json!(DISTRO))]{
            let mut input=f.confirm(&p);input[field]=value;
            assert!(f.manager.handle("environment.removal.detach",input).is_err());
        }
        *f.wsl.wrong_marker.lock().unwrap()=true;
        assert!(f.manager.handle("environment.removal.plan",json!({})).is_err());
        *f.wsl.wrong_marker.lock().unwrap()=false;
        f.wsl.registry.lock().unwrap()[0].guid=uuid::Uuid::new_v4().to_string();
        assert!(f.manager.handle("environment.removal.detach",f.confirm(&p)).is_err());
        assert!(f.manager.last_removal().unwrap().is_none());
        assert_eq!(f.manager.state().unwrap().phase,"ready");
    }
    #[test]
    fn withdrawal_is_durable_and_replay_never_reregisters_or_removes(){
        let mut f=Lab::new();
        let p=f.plan();let payload=f.confirm(&p);
        let first=f.manager.handle("environment.removal.detach",payload.clone()).unwrap();
        let old_id=first["record"]["environmentId"].as_str().unwrap();
        assert_eq!(first["confirmedNow"],false);
        assert!(f.manager.launch_distribution(old_id).is_err());
        assert!(f.manager.data_review_identity().is_err());
        assert!(f.manager.has_unresolved_maintenance().unwrap());
        let before=f.wsl.calls.lock().unwrap().len();
        f.manager=EnvironmentManager::with_runner(f.dir.path().into(),f.wsl.clone()).unwrap();
        assert_eq!(f.manager.handle("environment.removal.status",json!({})).unwrap(),first);
        assert_eq!(f.manager.handle("environment.removal.detach",payload).unwrap(),first);
        assert!(f.manager.handle("environment.provision",json!({})).is_err());
        assert_eq!(f.wsl.calls.lock().unwrap().len(),before);
        assert_eq!(f.inspect().unwrap()["confirmedNow"],true);
        assert_eq!(f.manager.handle("environment.removal.status",json!({})).unwrap()["confirmedNow"],false);
        f.wsl.registry.lock().unwrap().clear();
        let completed=f.inspect().unwrap();
        assert_eq!(completed["record"]["observation"],"registration_absent");
        assert_eq!(completed["record"]["pathStatus"],"remaining");
        assert_ne!(f.manager.state().unwrap().id,old_id);
        assert!(f.manager.install_location().is_dir());
        assert!(!f.manager.has_unresolved_maintenance().unwrap());
        assert_eq!(f.inspect().unwrap(),completed);
    }
    #[test]
    fn a_version_one_environment_with_missing_optional_fields_can_be_retired(){
        let mut f=Lab::new();
        let id=f.manager.state().unwrap().id;
        {
            let db=f.manager.connection.lock().unwrap();
            let raw:String=db.query_row("SELECT data FROM environment_state WHERE id=1",[],|r|r.get(0)).unwrap();
            let mut legacy:Value=serde_json::from_str(&raw).unwrap();
            legacy.as_object_mut().unwrap().remove("installExit");
            legacy.as_object_mut().unwrap().remove("hostCheckpoint");
            db.execute("UPDATE environment_state SET data=?1 WHERE id=1",[legacy.to_string()]).unwrap();
            db.execute_batch("DROP TABLE environment_removals; PRAGMA user_version=1;").unwrap();
        }
        f.manager=EnvironmentManager::with_runner(f.dir.path().into(),f.wsl.clone()).unwrap();
        assert_eq!(f.manager.state().unwrap().id,id);
        assert_eq!(f.detach()["record"]["environmentId"],id);
    }
    #[test]
    fn collision_in_guid_name_or_location_and_failed_inventory_never_confirm_absence(){
        let f=Lab::new();f.detach();
        *f.wsl.inventory_unavailable.lock().unwrap()=true;
        assert!(f.inspect().is_err());
        assert_eq!(f.manager.state().unwrap().phase,"retiring");
        *f.wsl.inventory_unavailable.lock().unwrap()=false;
        for mutation in 0..4 {
            let mut foreign=f.registration.clone();
            if mutation!=1 { foreign.guid=uuid::Uuid::new_v4().to_string(); }
            if mutation!=0 { foreign.name="Personal".into(); }
            if mutation!=2 { foreign.base_path="C:\\Personal".into(); }
            *f.wsl.registry.lock().unwrap()=if mutation==3 {vec![f.registration.clone(),Registration{name:DISTRO.into(),..foreign}]} else {vec![foreign]};
            let result=f.inspect().unwrap();
            assert_eq!(result["record"]["observation"],"registration_conflict");
            assert_eq!(result["confirmedNow"],false);
        }
        assert!(f.manager.has_unresolved_maintenance().unwrap());
        let id=f.manager.state().unwrap().id;
        let before=f.wsl.calls.lock().unwrap().len();
        assert!(f.manager.handle("environment.removal.keep",json!({"environmentId":id,"keepDistribution":false})).is_err());
        let preserve=json!({"environmentId":id,"keepDistribution":true});
        let kept=f.manager.handle("environment.removal.keep",preserve.clone()).unwrap();
        assert_eq!(kept["record"]["observation"],"owner_released");
        assert_eq!(f.manager.handle("environment.removal.keep",preserve).unwrap(),kept);
        assert_eq!(f.inspect().unwrap(),kept);
        assert!(f.manager.launch_distribution(&id).is_err());
        assert!(!f.manager.has_unresolved_maintenance().unwrap());
        assert_eq!(f.wsl.calls.lock().unwrap().len(),before);
    }
    #[test]
    fn reviewed_backup_restores_retirement_as_history_after_detach_and_after_inspection(){
        use crate::core::{Request,Store};
        use crate::recovery::{self,RecoveryService};
        let f=Lab::new();
        let root=f.dir.path().to_path_buf();
        let store=Store::open(root.clone()).unwrap();
        store.handle(Request{action:"project.create".into(),payload:json!({"name":"Removal fixture","kind":"web","target":"https://example.invalid"})}).unwrap();
        let recovery=RecoveryService::new(root).unwrap();
        f.detach();
        for expected in ["awaiting_windows","registration_absent"]{
            if expected=="registration_absent"{f.wsl.registry.lock().unwrap().clear();f.inspect().unwrap();}
            let backup=recovery::test_support::backup(&recovery,&store).unwrap();
            let restored=recovery.handle("backup.restore",json!({"id":backup["id"]}),&store).unwrap();
            let copy=Path::new(restored["path"].as_str().unwrap());
            assert_eq!(restored["activated"],false);
            assert!(!copy.join("environment.sqlite3").exists());
            let history=Connection::open_with_flags(copy.join("environment-history.sqlite3"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            let raw:String=history.query_row("SELECT data FROM environment_removals",[],|row|row.get(0)).unwrap();
            assert_eq!(serde_json::from_str::<Removal>(&raw).unwrap().observation,expected);
        }
    }
}
