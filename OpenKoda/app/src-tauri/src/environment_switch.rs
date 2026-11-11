//! The selected executor is committed with its activation history in one SQLite
//! transaction. A restored environment UUID can acquire authority only once.
use super::*;

const ACTIVATE: &str = "kali-restored-activation";

impl EnvironmentManager {
    fn switched_before(&self,id:&str)->Result<bool,String>{
        self.connection.lock().map_err(storage_error)?.query_row(
            "SELECT EXISTS(SELECT 1 FROM environment_switches WHERE environment_id=?1)",[id],|row|row.get(0)
        ).map_err(storage_error)
    }

    pub(super) fn activation_rows(&self, records:Vec<Record>)->Result<Vec<Value>,String>{
        let current=self.state()?;
        let mut result=Vec::with_capacity(records.len());
        for record in records {
            let mut row=serde_json::to_value(&record).map_err(storage_error)?;
            if let Operation::Import{restore}=&record.operation {
                if record.status=="inactive" {
                    row["activationState"]=json!(if current.id==restore.id {"active"} else if self.switched_before(&restore.id)? {"previous"} else {"available"});
                }
            }
            result.push(row);
        }
        Ok(result)
    }

    fn switch_origin(&self)->Result<EnvironmentState,String>{
        self.archives.check_source_stop()?;
        let current=self.state()?;
        match current.phase.as_str(){
            "ready" | "absent" => Ok(current),
            // A disappeared original cannot run a pending Linux process. Do not
            // contact or stop any registration that replaced it at the same name.
            "degraded" if current.registration.is_some() => {
                let original=current.registration.as_ref().ok_or("Vínculo da origem ausente.")?;
                if self.inventory()?.iter().any(|r|r.guid.eq_ignore_ascii_case(&original.guid)){
                    return Err("Reconcilie a manutenção da origem antes de trocar o executor.".into())
                }
                Ok(current)
            }
            _=>Err("Reconcilie a preparação ou a manutenção da origem antes de trocar o executor.".into()),
        }
    }

    fn restored_candidate(&self,id:&str)->Result<EnvironmentState,String>{
        uuid(id)?;
        if self.switched_before(id)?{return Err("Esta restauração já foi ativada. Uma nova restauração fornece uma identidade nova.".into())}
        let record=self.archives.get(id)?;
        let Operation::Import{restore}=&record.operation else{return Err("Selecione uma restauração conferida.".into())};
        if record.status!="inactive" || restore.id!=id || restore.backup.source_id==id{return Err("A restauração ainda não tem um marcador novo e confirmado.".into())}
        let registered=self.clone_registration(id)?;
        if Some(&registered)!=record.registration.as_ref(){return Err("O registro WSL diverge da restauração conferida.".into())}
        Ok(EnvironmentState{id:id.into(),phase:"ready".into(),registration:Some(registered),last_error:None,last_output:String::new(),updated_at:now(),install_exit:None})
    }

    fn switch_payload(&self,current:&EnvironmentState,target:&EnvironmentState)->Result<Value,String>{
        let registration=target.registration.as_ref().ok_or("Registro de recuperação ausente.")?;
        if target.id==current.id{return Err("Este ambiente já é o executor ativo.".into())}
        Ok(json!({"previousEnvironmentId":current.id,
            "previousDistribution":current.registration.as_ref().map_or(DISTRO,|r|r.name.as_str()),
            "previousStateSha256":digest(&serde_json::to_value(current).map_err(storage_error)?),
            "targetEnvironmentId":target.id,"targetDistribution":registration.name,"registration":registration,
            "effects":["Iniciar o clone para conferir marcador, usuário openkoda, isolamento e manutenção.",
                "Após a validação, usar esse clone para diagnóstico, ferramentas, pacotes e novas execuções.",
                "Preservar a distribuição anterior, as evidências e a distribuição padrão do Windows. Autorizações anteriores continuam associadas às identidades em que foram emitidas."]}))
    }

    pub(super) fn activate_plan(&self,selection:Selection)->Result<Value,String>{
        let current=self.switch_origin()?;
        let target=self.restored_candidate(&selection.id)?;
        self.save_plan(ACTIVATE,self.switch_payload(&current,&target)?)
    }

    pub(super) fn activate_restored(&self,approval:Approval)->Result<Value,String>{
        let plan=self.consume_plan(approval,ACTIVATE)?;
        let current=self.switch_origin()?;
        let id=plan.payload["targetEnvironmentId"].as_str().ok_or("Identidade do clone ausente.")?;
        let target=self.restored_candidate(id)?;
        if self.switch_payload(&current,&target)?!=plan.payload {return Err("O vínculo mudou. Revise outra troca de executor.".into())}
        let probe=self.helper(&target,"probe",vec![],45)?;
        if probe["environmentId"]!=target.id || probe["user"]!="openkoda" || !healthy_probe(&probe) {
            return Err("O clone não confirmou marcador, usuário, isolamento e manutenção. O executor anterior continua selecionado.".into())
        }
        let selected=self.owned_registration(&target)?;
        const USER_CHECK:&str="import json,os,pathlib,pwd; uid=os.geteuid(); print(json.dumps({'uid':uid,'user':pwd.getpwuid(uid).pw_name,'marker':pathlib.Path('/etc/openkoda/environment-id').read_text().strip()}))";
        let observed=self.wsl("WSL activation runtime check",vec!["--distribution".into(),selected.name.clone(),
            "--user".into(),"openkoda".into(),"--cd".into(),"/".into(),"--exec".into(),"/usr/bin/python3".into(),
            "-I".into(),"-c".into(),USER_CHECK.into()],30)?;
        checked(&observed)?;
        let user:Value=serde_json::from_str(observed.stdout.trim()).map_err(|_|"Resposta inválida do usuário do executor.")?;
        if user["marker"]!=target.id || user["user"]!="openkoda" || user["uid"]!=probe["uid"] || !user_stderr_ok(&observed.stderr) {
            return Err("O usuário efetivo do executor não confirmou o marcador restaurado.".into())
        }
        self.owned_registration(&target)?;
        let mut connection=self.connection.lock().map_err(storage_error)?;
        let tx=connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(storage_error)?;
        let raw:String=tx.query_row("SELECT data FROM environment_state WHERE id=1",[],|row|row.get(0)).map_err(storage_error)?;
        let previous:EnvironmentState=serde_json::from_str(&raw).map_err(storage_error)?;
        let stored=serde_json::to_value(previous).map_err(storage_error)?;
        if digest(&stored)!=plan.payload["previousStateSha256"] {return Err("O ambiente mudou durante a validação.".into())}
        tx.execute("INSERT INTO environment_switches(environment_id,plan_id,previous_state,selected_registration,activated_at) VALUES (?1,?2,?3,?4,?5)",
            params![target.id,plan.id,raw,serde_json::to_string(&selected).map_err(storage_error)?,target.updated_at]).map_err(storage_error)?;
        tx.execute("UPDATE environment_state SET data=?1 WHERE id=1",[serde_json::to_string(&target).map_err(storage_error)?]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;
        Ok(json!({"status":"activated","environmentId":target.id,"distro":selected.name,
            "previousEnvironmentId":current.id,"previousDistribution":plan.payload["previousDistribution"]}))
    }
}
