//! Reviewed profile activation at next boot; never replaces live databases.
//! Original profiles remain intact. Flush requests are not a power-loss proof.
use crate::{core::{self,Store}, model, recovery};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use std::{fs::{self,File},io::{Read,Write},path::{Path,PathBuf},sync::Mutex,time::Duration};

const FILES:[&str;7]=["openkoda.sqlite3","assistant.sqlite3","remediation.sqlite3","kali-bridge.sqlite3","environment-history.sqlite3","retention.sqlite3","project-purge.sqlite3"];
const MAX_PLANS:i64=100;
const LIMITS:[&str;5]=[
    "A ativação ocorre somente no próximo início do aplicativo, depois de verificar novamente hashes e schemas.",
    "O perfil anterior e a cópia de recuperação permanecem intactos. Nenhum dado é apagado ou substituído por esta ação.",
    "Credenciais e confirmação do provedor são excluídas; configure e teste a conexão novamente no novo perfil.",
    "O ambiente atual da instalação é preservado e revalidado pelo gerenciador; o backup não restaura Kali. O perfil guarda apenas histórico de ambiente.",
    "Arquivos externos dos projetos, mudanças já aplicadas, processos e disco Kali não são restaurados. Flush/rename no Windows não equivale a ensaio de queda de energia.",
];
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="camelCase",deny_unknown_fields)]
pub enum ProfileRef {Base,Profile{id:String}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ProfileFile {name:String,bytes:u64,sha256:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ActivationPlan {
    id:String,kind:String,source_recovery_id:Option<String>,from_profile:ProfileRef,source_profile:ProfileRef,target_profile:ProfileRef,
    files:Vec<ProfileFile>,digest:String,created_at:String,expires_at:String,status:String,message:String,
    effects:Vec<String>,limits:Vec<String>,restart_required:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ActivePointer {format:u32,profile:ProfileRef,previous:ProfileRef,activation_id:String,committed_at:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct RestoreInput {recovery_id:String}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {id:String,digest:String}
pub struct ActivationService {base:PathBuf,current:PathBuf,db:Mutex<Connection>,operation:Mutex<()>}
pub struct BootSelection {pub root:PathBuf,pub notice:Option<String>}

pub fn supports(action:&str)->bool {matches!(action,"activation.plan"|"activation.rollbackPlan"|"activation.schedule"|"activation.cancel"|"activation.status"|"activation.list")}
pub fn mutates(action:&str)->bool {matches!(action,"activation.plan"|"activation.rollbackPlan"|"activation.schedule"|"activation.cancel")}
fn error(_:impl std::fmt::Display)->String {"Falha no controle de perfis. Os perfis e as cópias anteriores foram preservados.".into()}
fn decode<T:serde::de::DeserializeOwned>(v:Value)->Result<T,String> {serde_json::from_value(v).map_err(|_|"Campos de ativação inválidos ou não permitidos.".into())}
fn encode<T:Serialize>(v:T)->Result<Value,String> {serde_json::to_value(v).map_err(error)}
fn empty(v:&Value)->Result<(),String> {if v.as_object().is_some_and(|o|o.is_empty()){Ok(())}else{Err("Esta ação não recebe campos.".into())}}
fn digest(p:&ActivationPlan)->String {core::hash(&json!({"id":p.id,"kind":p.kind,"sourceRecoveryId":p.source_recovery_id,"fromProfile":p.from_profile,
    "sourceProfile":p.source_profile,"targetProfile":p.target_profile,"files":p.files,"createdAt":p.created_at,"expiresAt":p.expires_at,
    "effects":p.effects,"limits":p.limits,"restartRequired":p.restart_required}).to_string())}
fn expired(p:&ActivationPlan)->Result<bool,String> {Ok(chrono::DateTime::parse_from_rfc3339(&p.expires_at).map_err(error)?<=chrono::Utc::now())}
fn profile_path(base:&Path,profile:&ProfileRef)->Result<PathBuf,String> {
    let path=match profile {ProfileRef::Base=>base.to_path_buf(),ProfileRef::Profile{id}=>{core::valid_id(id)?;base.join("profiles").join(id)}};
    if let ProfileRef::Profile{..}=profile {recovery::require_plain(&base.join("profiles"),true)?;}
    recovery::require_plain(&path,true)?;
    let resolved=path.canonicalize().map_err(error)?;
    if !resolved.starts_with(base) || (matches!(profile,ProfileRef::Base)&&resolved!=base) {return Err("Perfil saiu da base local permitida.".into());}
    Ok(resolved)
}
fn current_profile(base:&Path,current:&Path)->Result<ProfileRef,String> {
    if base==current {return Ok(ProfileRef::Base);}
    let parent=base.join("profiles");
    if current.parent()!=Some(parent.as_path()) {return Err("Perfil atual não pertence a esta base de ativação.".into());}
    let id=current.file_name().and_then(|s|s.to_str()).ok_or("Identidade de perfil inválida.")?.to_owned();core::valid_id(&id)?;
    Ok(ProfileRef::Profile{id})
}
fn read_pointer(base:&Path)->Result<Option<ActivePointer>,String> {
    let path=base.join("active-profile.json");if !path.exists(){return Ok(None);}
    recovery::require_plain(&path,false)?;
    let raw=read_small(&path,16384)?;
    let pointer:ActivePointer=serde_json::from_slice(&raw).map_err(|_|"Ponteiro de perfil inválido; nenhum perfil foi trocado.".to_string())?;
    if pointer.format!=1 {return Err("Versão de ponteiro de perfil não suportada.".into());}
    core::valid_id(&pointer.activation_id)?;profile_path(base,&pointer.profile)?;profile_path(base,&pointer.previous)?;
    chrono::DateTime::parse_from_rfc3339(&pointer.committed_at).map_err(error)?;Ok(Some(pointer))
}
fn read_small(path:&Path,max:u64)->Result<Vec<u8>,String> {let mut bytes=Vec::new();File::open(path).map_err(error)?.take(max+1).read_to_end(&mut bytes).map_err(error)?;if bytes.len() as u64>max{return Err("Registro de perfil excede o limite.".into());}Ok(bytes)}
fn write_new(path:&Path,bytes:&[u8])->Result<(),String> {let mut f=fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(error)?;f.write_all(bytes).and_then(|_|f.sync_all()).map_err(error)}
fn create_child(base:&Path,name:&str)->Result<PathBuf,String> {let path=base.join(name);if !path.exists(){fs::create_dir(&path).map_err(error)?;}recovery::require_plain(&path,true)?;let resolved=path.canonicalize().map_err(error)?;if resolved.parent()!=Some(base){return Err("Diretório de perfis fora da base permitida.".into());}Ok(resolved)}

impl ActivationService {
    pub fn new(base:PathBuf,current:PathBuf)->Result<Self,String> {
        recovery::require_plain(&base,true)?;recovery::require_plain(&current,true)?;
        let base=base.canonicalize().map_err(error)?;let current=current.canonicalize().map_err(error)?;current_profile(&base,&current)?;
        let path=base.join("activation.sqlite3");if path.exists(){recovery::require_plain(&path,false)?;}
        let db=Connection::open(path).map_err(error)?;db.busy_timeout(Duration::from_secs(5)).map_err(error)?;
        let check:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(error)?;if check!="ok"{return Err("Registro de ativação inconsistente; nenhum perfil foi trocado.".into());}
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS activation_plans(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS activation_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);").map_err(error)?;
        Ok(Self{base,current,db:Mutex::new(db),operation:Mutex::new(())})
    }
    pub fn handle(&self,action:&str,payload:Value,store:&Store)->Result<Value,String> {
        if !payload.is_object()||payload.to_string().len()>16384{return Err("Solicitação de ativação inválida.".into());}
        if store.root.canonicalize().map_err(error)?!=self.current{return Err("A solicitação não pertence ao perfil atual.".into());}
        let _lock=self.operation.try_lock().map_err(|_|"Outra operação de ativação está em andamento.")?;
        match action {
            "activation.status"=>{empty(&payload)?;self.status()},
            "activation.list"=>{empty(&payload)?;Ok(json!({"plans":self.plans()?}))},
            "activation.plan"=>{let input:RestoreInput=decode(payload)?;self.prepare_restore(&input.recovery_id,store).and_then(encode)},
            "activation.rollbackPlan"=>{empty(&payload)?;self.prepare_rollback(store).and_then(encode)},
            "activation.schedule"=>self.schedule(decode(payload)?,store).and_then(encode),
            "activation.cancel"=>self.cancel(decode(payload)?).and_then(encode),
            _=>Err("Ação de ativação não disponível.".into()),
        }
    }
    /// Call while holding the same shared environment gate as a new dispatch.
    /// Scheduling owns its exclusive side, so approval and effects cannot race.
    pub fn blocks_new_effects(&self)->Result<bool,String> {
        Ok(self.pending()?.is_some_and(|plan|plan.status=="scheduled"))
    }
    fn active(&self)->Result<ProfileRef,String> {Ok(read_pointer(&self.base)?.map(|p|p.profile).unwrap_or(ProfileRef::Base))}
    fn pending(&self)->Result<Option<ActivationPlan>,String> {
        let id:Option<String>=self.db.lock().map_err(error)?.query_row("SELECT value FROM activation_meta WHERE key='pending'",[],|r|r.get(0)).optional().map_err(error)?;
        id.map(|id|self.load(&id)).transpose()
    }
    fn load(&self,id:&str)->Result<ActivationPlan,String> {
        core::valid_id(id)?;let raw:Option<String>=self.db.lock().map_err(error)?.query_row("SELECT data FROM activation_plans WHERE id=?1",[id],|r|r.get(0)).optional().map_err(error)?;
        let p=parse_plan(&raw.ok_or("Plano de ativação não encontrado.")?)?;if p.id!=id{return Err("Identidade do plano de ativação inconsistente.".into());}Ok(p)
    }
    fn save(&self,p:&ActivationPlan)->Result<(),String> {
        let count=self.db.lock().map_err(error)?.execute("UPDATE activation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(p).map_err(error)?,p.id]).map_err(error)?;
        if count!=1{return Err("Plano de ativação não encontrado; nada foi repetido.".into());}Ok(())
    }
    fn plans(&self)->Result<Vec<ActivationPlan>,String> {
        let db=self.db.lock().map_err(error)?;let mut query=db.prepare("SELECT data FROM activation_plans ORDER BY rowid DESC LIMIT 101").map_err(error)?;
        let rows=query.query_map([],|r|r.get::<_,String>(0)).map_err(error)?;let mut result=Vec::new();
        for row in rows {if result.len()>=MAX_PLANS as usize{return Err("Registro de ativação excede o limite.".into());}result.push(parse_plan(&row.map_err(error)?)?);}Ok(result)
    }
    fn status(&self)->Result<Value,String> {
        let pointer=read_pointer(&self.base)?;let pending=self.pending()?;
        let restart=pending.as_ref().is_some_and(|p|p.status=="scheduled");
        Ok(json!({"currentProfile":current_profile(&self.base,&self.current)?,"activeProfile":pointer.as_ref().map(|p|p.profile.clone()).unwrap_or(ProfileRef::Base),
            "previousProfile":pointer.map(|p|p.previous),"pendingPlan":pending,"restartRequired":restart,
            "notice":"Agendar não reinicia nem troca o perfil aberto. O novo perfil exige configuração de credenciais. O ambiente atual da instalação é preservado e revalidado; o backup não restaura Kali."}))
    }
    fn ensure_idle(&self,store:&Store)->Result<(),String> {
        if self.pending()?.is_some(){return Err("Já existe ativação pendente; revise ou cancele antes de preparar outra.".into());}
        if store.snapshot()?.assessments.iter().any(|a|core::active(&a.status)){return Err("Encerre ou reconcilie avaliações antes de preparar uma ativação.".into());}
        ensure_source_idle(&self.current)?;
        ensure_environment_idle(&self.base)?;
        if self.active()!=Ok(current_profile(&self.base,&self.current)?){return Err("O perfil ativo mudou; reinicie antes de preparar outra ativação.".into());}
        Ok(())
    }
    fn prepare_restore(&self,recovery_id:&str,store:&Store)->Result<ActivationPlan,String> {
        self.ensure_idle(store)?;core::valid_id(recovery_id)?;
        let recovery_base=self.current.join("recovery");recovery::require_plain(&recovery_base,true)?;
        let source=recovery_base.join(format!("restored-{recovery_id}"));recovery::require_plain(&source,true)?;
        let receipt_path=source.join("recovery.json");recovery::require_plain(&receipt_path,false)?;
        let receipt:Value=serde_json::from_slice(&read_small(&receipt_path,65536)?).map_err(error)?;
        if receipt["id"]!=recovery_id||receipt["verified"]!=true||receipt["activated"]!=false {return Err("Cópia de recuperação não tem recibo válido de staging.".into());}
        let source=source.canonicalize().map_err(error)?;
        if source.parent()!=Some(recovery_base.canonicalize().map_err(error)?.as_path()){return Err("Cópia de recuperação fora do destino gerido.".into());}
        self.prepare_copy(&source,"restore",Some(recovery_id.into()),current_profile(&self.base,&self.current)?)
    }
    fn prepare_rollback(&self,store:&Store)->Result<ActivationPlan,String> {
        self.ensure_idle(store)?;let pointer=read_pointer(&self.base)?.ok_or("Ainda não há perfil anterior registrado para rollback.")?;
        let source=profile_path(&self.base,&pointer.previous)?;
        self.prepare_copy(&source,"rollback",None,pointer.previous)
    }
    fn prepare_copy(&self,source:&Path,kind:&str,recovery_id:Option<String>,source_profile:ProfileRef)->Result<ActivationPlan,String> {
        if self.plans()?.len()>=MAX_PLANS as usize{return Err("Limite de 100 planos de ativação atingido; preserve o histórico antes de ampliar.".into());}
        let profiles=create_child(&self.base,"profiles")?;let profile_id=model::id();let target=profiles.join(&profile_id);fs::create_dir(&target).map_err(error)?;
        let _source_lock=instance_lock(source)?;
        recovery::validate_core_database(&source.join("openkoda.sqlite3"))?;
        for name in FILES {
            let original=source.join(name);let original=if name=="environment-history.sqlite3"&&source.join("environment.sqlite3").exists(){source.join("environment.sqlite3")}else{original};
            if !original.exists(){continue;}
            recovery::require_plain(&original,false)?;
            if name=="assistant.sqlite3" {
                if !recovery::backup_assistant(&original,&target.join(name))?{return Err("Schema do histórico da IA desconhecido; ativação recusada sem perder a origem.".into());}
            }else{recovery::backup_database(&original,&target.join(name))?;}
        }
        if !target.join("openkoda.sqlite3").exists(){return Err("A cópia não contém o banco principal; não será ativada.".into());}
        // Only local schema/state initialization. These constructors never run
        // inference, commands, target writes, or adopt a WSL installation.
        let copied=Store::open(target.clone())?;
        crate::work_supervisor::deactivate_after_restore(&copied)?;
        copied.mutate(|s|{s.settings.has_api_key=false;crate::data_policy::deactivate_after_restore(s)})?;
        drop(copied);
        drop(crate::ai::AiService::new(target.clone())?);
        let uncertain_requests=merge_unknown_requests(&self.current,&target)?;
        drop(crate::remediation::RemediationService::new(target.clone())?);
        drop(crate::retention::RetentionService::new(target.clone())?);
        drop(crate::kali_bridge::KaliBridge::new(target.clone())?);
        let purges=crate::project_purge::ProjectPurgeService::new(target.clone())?;
        if purges.has_unresolved()?{return Err("A cópia contém exclusão de projeto incompleta. Ela foi preservada sem agendar ativação.".into());}
        drop(purges);
        let mut files=Vec::new();
        for name in FILES {let path=target.join(name);if !path.exists(){continue;}
            let db=Connection::open(&path).map_err(error)?;db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;").map_err(error)?;drop(db);
            fs::OpenOptions::new().write(true).open(&path).map_err(error)?.sync_all().map_err(error)?;
            files.push(ProfileFile{name:name.into(),bytes:fs::metadata(&path).map_err(error)?.len(),sha256:recovery::hash_file(&path)?});
        }
        let mut p=ActivationPlan{id:model::id(),kind:kind.into(),source_recovery_id:recovery_id,from_profile:current_profile(&self.base,&self.current)?,source_profile,
            target_profile:ProfileRef::Profile{id:profile_id},files,digest:String::new(),created_at:model::now(),expires_at:(chrono::Utc::now()+chrono::Duration::hours(24)).to_rfc3339(),status:"prepared".into(),
            message:"Cópia de perfil preparada sem alterar o perfil aberto. Revise os bancos, exclusões e digest antes de agendar para o próximo início.".into(),
            effects:vec!["Ativar uma nova pasta de perfil no próximo início".into(),"Preservar perfil anterior, staging e backup sem substituição".into(),"Restaurar domínio e históricos, com aprovações revogadas e operações interrompidas desconhecidas".into(),"Excluir credenciais/confirmações do novo perfil; preservar ambiente atual da instalação, que será revalidado sem restaurar Kali do backup".into()],
            limits:LIMITS.iter().map(|s|s.to_string()).collect(),restart_required:true};
        if uncertain_requests>0 {p.effects.push(format!("Preservar {uncertain_requests} registros de solicitações de IA com resultado/consumo desconhecido do perfil atual, sem copiar credenciais ou mensagens"));}
        p.digest=digest(&p);validate_target(&self.base,&p)?;
        self.db.lock().map_err(error)?.execute("INSERT INTO activation_plans(id,data) VALUES(?1,?2)",params![p.id,serde_json::to_string(&p).map_err(error)?]).map_err(error)?;
        Ok(p)
    }
    fn schedule(&self,input:Decision,store:&Store)->Result<ActivationPlan,String> {
        self.ensure_idle(store)?;let mut p=self.load(&input.id)?;
        if p.digest!=input.digest||p.status!="prepared"||p.from_profile!=current_profile(&self.base,&self.current)?||expired(&p)?{return Err("Plano de ativação mudou, expirou ou já foi usado; prepare nova revisão.".into());}
        validate_target(&self.base,&p)?;
        p.status="scheduled".into();p.message="Ativação agendada para o próximo início. O perfil aberto permanece em uso até encerrar o aplicativo.".into();
        let mut db=self.db.lock().map_err(error)?;let tx=db.transaction().map_err(error)?;
        tx.execute("INSERT INTO activation_meta(key,value) VALUES('pending',?1)",[&p.id]).map_err(error)?;
        tx.execute("UPDATE activation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(&p).map_err(error)?,p.id]).map_err(error)?;tx.commit().map_err(error)?;Ok(p)
    }
    fn cancel(&self,input:Decision)->Result<ActivationPlan,String> {
        let mut p=self.load(&input.id)?;if p.digest!=input.digest||!["prepared","scheduled","blocked"].contains(&p.status.as_str()){return Err("Plano não pode ser cancelado neste estado/digest.".into());}
        p.status="canceled".into();p.message="Agendamento cancelado; perfis e cópias permanecem preservados.".into();
        let mut db=self.db.lock().map_err(error)?;let tx=db.transaction().map_err(error)?;
        tx.execute("DELETE FROM activation_meta WHERE key='pending' AND value=?1",[&p.id]).map_err(error)?;
        tx.execute("UPDATE activation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(&p).map_err(error)?,p.id]).map_err(error)?;tx.commit().map_err(error)?;Ok(p)
    }
}

fn parse_plan(raw:&str)->Result<ActivationPlan,String> {
    if raw.len()>65536{return Err("Plano de ativação excede o limite.".into());}let p:ActivationPlan=serde_json::from_str(raw).map_err(error)?;
    core::valid_id(&p.id)?;if p.digest!=digest(&p)||!matches!(p.kind.as_str(),"restore"|"rollback")||!["prepared","scheduled","blocked","canceled","committed"].contains(&p.status.as_str())||!p.restart_required{return Err("Integridade ou estado do plano de ativação inválido.".into());}Ok(p)
}
fn instance_lock(root:&Path)->Result<File,String> {
    let path=root.join("instance.lock");if path.exists(){recovery::require_plain(&path,false)?;}
    let mut opts=fs::OpenOptions::new();opts.create(true).read(true).write(true);
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;opts.share_mode(0);}
    opts.open(path).map_err(|_|"O perfil de origem/destino está aberto em outra instância; ativação recusada.".into())
}
fn ensure_source_idle(root:&Path)->Result<(),String> {
    let path=root.join("openkoda.sqlite3");recovery::require_plain(&path,false)?;
    let db=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let raw:String=db.query_row("SELECT json FROM app_state WHERE id=1",[],|row|row.get(0)).map_err(error)?;
    if raw.len()>64*1024*1024{return Err("Estado de origem excede o limite; ativação bloqueada.".into());}
    let snapshot:crate::model::Snapshot=serde_json::from_str(&raw).map_err(error)?;
    core::validate_snapshot(&snapshot)?;
    let version:i64=db.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(error)?;
    if version>core::STORE_VERSION{return Err("Versão de origem desconhecida; ativação bloqueada.".into());}
    if version>=7{
        crate::work_store::validate_store(&db,&snapshot)?;
        crate::capture::validate_capture_tables(&db)?;
        crate::capture::validate_capture_domain(&db,&snapshot)?;
        crate::work_supervisor::validate_leases(&db,&snapshot)?;
        crate::work_supervisor::ensure_quiescent_locked(&db,&snapshot)?;
    }
    if snapshot.assessments.iter().any(|a|core::active(&a.status)||a.status=="awaiting_approval")
        ||snapshot.plans.iter().any(|p|matches!(p.status.as_str(),"approved"|"running"|"canceling"|"unknown"))
        ||snapshot.executions.iter().any(|e|matches!(e.status.as_str(),"running"|"canceling"|"unknown")) {
        return Err("O perfil de origem contém avaliação/plano/execução pendente ou com efeito desconhecido. Reconcilie antes de trocar perfis.".into());
    }
    for (name,query,stable) in [
        ("kali-bridge.sqlite3","SELECT json_extract(data,'$.status') FROM bridge_runs",&["completed","failed","canceled"][..]),
        ("remediation.sqlite3","SELECT json_extract(data,'$.status') FROM remediation_plans",&["proposed","applied","retested","confirmed","revoked","invalidated","rolled_back","failed","reconciled"][..]),
        ("retention.sqlite3","SELECT json_extract(data,'$.status') FROM retention_plans",&["prepared","applied","partial","rejected","revoked"][..]),
        ("project-purge.sqlite3","SELECT json_extract(data,'$.status') FROM project_purge_plans",&["prepared","completed","rejected","revoked","abandoned_partial"][..]),
    ] {
        let path=root.join(name);if !path.exists(){continue;}recovery::require_plain(&path,false)?;
        let db=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
        let mut statement=db.prepare(query).map_err(error)?;let rows=statement.query_map([],|row|row.get::<_,String>(0)).map_err(error)?;
        for status in rows {let status=status.map_err(error)?;if !stable.contains(&status.as_str()){return Err("O perfil de origem contém sessão MCP ou correção pendente/desconhecida. O histórico não será ocultado por uma troca de perfil.".into());}}
    }
    let assistant=root.join("assistant.sqlite3");
    if assistant.exists(){recovery::require_plain(&assistant,false)?;let db=Connection::open_with_flags(assistant,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
        let pending:i64=db.query_row("SELECT count(*) FROM requests WHERE status='pending'",[],|r|r.get(0)).map_err(error)?;
        if pending!=0{return Err("Há solicitação de IA pendente no perfil de origem. Reconcilie o registro antes de trocar perfis.".into());}
        let investigations:i64=db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='investigations'",[],|r|r.get(0)).map_err(error)?;
        if investigations!=0 {
            let unresolved:i64=db.query_row("SELECT count(*) FROM investigations WHERE json_extract(data,'$.status') IN ('running','canceling','unknown')",[],|r|r.get(0)).map_err(error)?;
            if unresolved!=0{return Err("Há investigação de IA pendente ou desconhecida. Reconcilie seu histórico antes de trocar de perfil.".into());}
        }
    }
    Ok(())
}
fn ensure_environment_idle(base:&Path)->Result<(),String> {
    let path=base.join("environment.sqlite3");if !path.exists(){return Ok(());}recovery::require_plain(&path,false)?;
    let db=Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let raw:String=db.query_row("SELECT data FROM environment_state WHERE id=1",[],|r|r.get(0)).map_err(error)?;
    if raw.len()>1024*1024{return Err("Registro de ambiente excede o limite; ativação bloqueada.".into());}
    let state:Value=serde_json::from_str(&raw).map_err(error)?;
    let safe=match state["phase"].as_str(){Some("absent"|"ready")=>true,Some("degraded")=>state.get("registration")==Some(&Value::Null)
        &&state["installExit"]["confirmed"]==true&&state["installExit"]["code"].as_i64().is_some_and(|code|code!=0),_=>false};
    if !safe{return Err("Manutenção do ambiente da instalação está pendente ou com efeito incerto. Reconcilie antes da ativação.".into());}Ok(())
}
fn preserve_unknown_inference(source:&Path,target:&Path)->Result<(),String> {
    let source=source.join("assistant.sqlite3");if !source.exists(){return Ok(());}
    let input=Connection::open_with_flags(source,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let output=Connection::open_with_flags(target.join("assistant.sqlite3"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let mut query=input.prepare("SELECT id FROM requests WHERE status='unknown'").map_err(error)?;
    let rows=query.query_map([],|r|r.get::<_,String>(0)).map_err(error)?;
    for id in rows {let id=id.map_err(error)?;let included:i64=output.query_row("SELECT count(*) FROM requests WHERE id=?1 AND status='unknown'",[id],|r|r.get(0)).map_err(error)?;
        if included!=1{return Err("Uma solicitação de IA com resultado/consumo desconhecido não está na cópia revisada. Prepare nova cópia para preservar esse histórico no perfil ativo.".into());}}
    Ok(())
}
fn merge_unknown_requests(source:&Path,target:&Path)->Result<usize,String> {
    let source=source.join("assistant.sqlite3");if !source.exists(){return Ok(0);}recovery::require_plain(&source,false)?;
    let input=Connection::open_with_flags(source,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let mut query=input.prepare("SELECT id,assessment,endpoint,model,revision,context_hash,status,created,input_tokens,output_tokens,details FROM requests WHERE status='unknown' LIMIT 10001").map_err(error)?;
    let rows=query.query_map([],|row|(0..11).map(|i|row.get::<_,rusqlite::types::Value>(i)).collect::<rusqlite::Result<Vec<_>>>()).map_err(error)?;
    let mut output=Connection::open(target.join("assistant.sqlite3")).map_err(error)?;let tx=output.transaction().map_err(error)?;let mut count=0;
    for row in rows {
        let row=row.map_err(error)?;count+=1;if count>10000{return Err("Histórico de solicitações incertas excede o limite de cópia revisada.".into());}
        let existing:Option<Vec<rusqlite::types::Value>>=tx.query_row("SELECT id,assessment,endpoint,model,revision,context_hash,status,created,input_tokens,output_tokens,details FROM requests WHERE id=?1",[&row[0]],|r|(0..11).map(|i|r.get(i)).collect()).optional().map_err(error)?;
        if existing.is_some_and(|record|[0,1,2,3,4,5,7].iter().any(|i|record[*i]!=row[*i])){return Err("Conflito de identidade em solicitação de IA incerta; nenhum histórico será substituído silenciosamente.".into());}
        tx.execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created,input_tokens,output_tokens,details) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(id) DO UPDATE SET status='unknown',input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,details=excluded.details",rusqlite::params_from_iter(row.iter())).map_err(error)?;
    }
    tx.commit().map_err(error)?;Ok(count)
}
fn validate_target(base:&Path,p:&ActivationPlan)->Result<PathBuf,String> {
    if !matches!(p.target_profile,ProfileRef::Profile{..}){return Err("Ativação exige uma nova pasta de perfil; a base nunca será sobrescrita.".into());}
    let root=profile_path(base,&p.target_profile)?;
    if root.join("environment.sqlite3").exists(){return Err("A cópia contém propriedade ativa de ambiente; revalidação separada é obrigatória.".into());}
    if p.files.is_empty()||p.files.len()>FILES.len(){return Err("Inventário de bancos do perfil inválido.".into());}
    let mut names=std::collections::HashSet::new();
    for item in &p.files {
        if !FILES.contains(&item.name.as_str())||!names.insert(item.name.clone())||item.bytes>128*1024*1024{return Err("Banco fora do inventário permitido de ativação.".into());}
        let path=root.join(&item.name);recovery::require_plain(&path,false)?;
        if path.with_extension("sqlite3-wal").exists()&&fs::metadata(path.with_extension("sqlite3-wal")).map_err(error)?.len()>0{return Err("Cópia preparada ganhou journal WAL; recalcule a ativação.".into());}
        if fs::metadata(&path).map_err(error)?.len()!=item.bytes||recovery::hash_file(&path)?!=item.sha256{return Err("Hash/tamanho da cópia preparada mudou; nenhum perfil será trocado.".into());}
        recovery::verify_database(&path)?;
    }
    if !names.contains("openkoda.sqlite3")||!names.contains("assistant.sqlite3"){return Err("Cópia sem domínio ou histórico de IA validado.".into());}
    recovery::validate_core_database(&root.join("openkoda.sqlite3"))?;
    let db=Connection::open_with_flags(root.join("assistant.sqlite3"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let secrets:i64=db.query_row("SELECT count(*) FROM vault",[],|r|r.get(0)).map_err(error)?;
    let verified:i64=db.query_row("SELECT count(*) FROM connection_check",[],|r|r.get(0)).map_err(error)?;
    if secrets!=0||verified!=0{return Err("A cópia de ativação contém credenciais/confirmações indevidas.".into());}
    Ok(root)
}

/// Called only while storage::StorageLease owns the global base lock, before
/// opening any Store/services. Does not silently roll back an already-active root.
pub fn boot(base:&Path)->Result<BootSelection,String> {
    let base=base.canonicalize().map_err(error)?;let pointer=read_pointer(&base)?;
    let active=pointer.as_ref().map(|p|p.profile.clone()).unwrap_or(ProfileRef::Base);let current=profile_path(&base,&active)?;
    let service=ActivationService::new(base.clone(),current.clone())?;
    let Some(mut pending)=service.pending()? else{return Ok(BootSelection{root:current,notice:None});};
    let already_committed=pointer.as_ref().is_some_and(|pointer|pointer.activation_id==pending.id&&pointer.profile==pending.target_profile);
    if pending.status=="blocked" {return Ok(BootSelection{root:current,notice:Some(pending.message)});}
    if pending.status!="scheduled" {return Err("Intenção de ativação em estado inválido. Nenhum perfil foi substituído.".into());}
    // An older app build might not own the global profile lock. Still require
    // the live Store to be closed before switching away from its directory.
    let _current_lock=instance_lock(&current)?;
    let mut _target_lock=None;
    let mut _source_lock=None;
    let allowed:Result<PathBuf,String>=(||{
        let source=profile_path(&base,&pending.from_profile)?;
        if source!=current {_source_lock=Some(instance_lock(&source)?);}
        // A prepared copy must never hide work started after scheduling,
        // including work whose supervisor disappeared before the next boot.
        ensure_source_idle(&source)?;
        ensure_environment_idle(&base)?;
        let candidate=profile_path(&base,&pending.target_profile)?;
        if candidate!=current {_target_lock=Some(instance_lock(&candidate)?);}
        let target=validate_target(&base,&pending)?;
        preserve_unknown_inference(&source,&target)?;
        if !already_committed&&(expired(&pending)?||pending.from_profile!=active){return Err("Ativação expirada ou perfil de origem mudou; revise novamente.".into());}
        if !already_committed {
            let next=ActivePointer{format:1,profile:pending.target_profile.clone(),previous:active.clone(),activation_id:pending.id.clone(),committed_at:model::now()};
            commit_pointer(&base,&next,pointer.as_ref())?;
        }
        Ok(target)
    })();
    let root=match allowed {
        Ok(root)=>root,
        Err(reason)=>{
            let observed=read_pointer(&base)?;
            let pointer_changed=observed.as_ref().map(|p|p.profile.clone()).unwrap_or(ProfileRef::Base)!=active;
            if already_committed||pointer_changed{return Err(format!("Ponteiro de ativação já mudou, mas o término não foi confirmado. Intenção e perfis anteriores preservados; nenhum rollback automático. {reason}"));}
            pending.status="blocked".into();pending.message=format!("Ativação bloqueada antes da troca. O perfil anterior permanece ativo. {reason}");service.save(&pending)?;
            return Ok(BootSelection{root:current,notice:Some(pending.message)});
        }
    };
    pending.status="committed".into();pending.message="Novo perfil ativado e origem preservada. Configure credenciais; o ambiente atual da instalação é preservado e deve passar novamente pelo diagnóstico.".into();
    let mut db=service.db.lock().map_err(error)?;let tx=db.transaction().map_err(error)?;
    tx.execute("UPDATE activation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(&pending).map_err(error)?,pending.id]).map_err(error)?;
    tx.execute("DELETE FROM activation_meta WHERE key='pending' AND value=?1",[&pending.id]).map_err(error)?;tx.commit().map_err(error)?;
    Ok(BootSelection{root,notice:Some(pending.message)})
}
fn commit_pointer(base:&Path,next:&ActivePointer,previous:Option<&ActivePointer>)->Result<(),String> {
    let history=create_child(base,"profile-pointer-history")?;
    if let Some(previous)=previous {write_new(&history.join(format!("{}-{}.json",next.activation_id,model::id())),&serde_json::to_vec_pretty(previous).map_err(error)?)?;}
    let temporary=base.join(format!("active-profile-{}.pending",model::id()));
    write_new(&temporary,&serde_json::to_vec_pretty(next).map_err(error)?)?;
    let destination=base.join("active-profile.json");if destination.exists(){recovery::require_plain(&destination,false)?;}
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};
        let from:Vec<u16>=temporary.as_os_str().encode_wide().chain(Some(0)).collect();let to:Vec<u16>=destination.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe{MoveFileExW(from.as_ptr(),to.as_ptr(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)}==0{return Err("Windows não confirmou a troca do ponteiro; intenção e perfis foram preservados para diagnóstico.".into());}
    }
    #[cfg(not(windows))] {return Err("Ativação de perfis liberada somente para Windows; intenção preservada.".into());}
    #[allow(unreachable_code)] {
        let actual=read_pointer(base)?.ok_or("Ponteiro de ativação não encontrado após gravação.")?;
        if actual.activation_id!=next.activation_id||actual.profile!=next.profile{return Err("A troca do ponteiro não foi confirmada; intenção preservada.".into());}Ok(())
    }
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    struct Fixture {_temp:tempfile::TempDir,base:PathBuf,store:Store,service:ActivationService,recovery_id:String}
    fn fixture()->Fixture {
        let temp=tempfile::tempdir().unwrap();let base=temp.path().join("base");let store=Store::open(base.clone()).unwrap();
        store.mutate(|s|{s.settings.model="snapshot-model".into();Ok(())}).unwrap();
        let assistant=crate::ai::AiService::new(base.clone()).unwrap();
        assistant.handle("ai.key.save",json!({"key":"SYNTHETIC-ACTIVATION-KEY-NO-PROVIDER-0123456789"}),&store).unwrap();drop(assistant);
        let ai=Connection::open(base.join("assistant.sqlite3")).unwrap();
        ai.execute("INSERT INTO messages VALUES(?1,?2,'assistant','preserved historical text',?3)",params![model::id(),model::id(),model::now()]).unwrap();drop(ai);
        drop(crate::environment::EnvironmentManager::new(base.clone()).unwrap());
        Connection::open(base.join("environment.sqlite3")).unwrap().execute("UPDATE environment_state SET data=json_set(data,'$.lastOutput','installation-owned-state') WHERE id=1",[]).unwrap();
        drop(crate::remediation::RemediationService::new(base.clone()).unwrap());drop(crate::kali_bridge::KaliBridge::new(base.clone()).unwrap());
        let recovery=recovery::RecoveryService::new(base.clone()).unwrap();
        let backup=crate::recovery::test_support::backup(&recovery,&store).unwrap();
        let restored=recovery.handle("backup.restore",json!({"id":backup["id"]}),&store).unwrap();
        store.mutate(|s|{s.settings.model="live-original-model".into();Ok(())}).unwrap();
        let base=base.canonicalize().unwrap();let service=ActivationService::new(base.clone(),base.clone()).unwrap();
        Fixture{_temp:temp,base,store,service,recovery_id:restored["id"].as_str().unwrap().into()}
    }
    fn plan(f:&Fixture)->ActivationPlan {f.service.prepare_restore(&f.recovery_id,&f.store).unwrap()}
    fn decision(p:&ActivationPlan)->Decision {Decision{id:p.id.clone(),digest:p.digest.clone()}}
    fn database_model(path:&Path)->String {
        let db=Connection::open_with_flags(path.join("openkoda.sqlite3"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let raw:String=db.query_row("SELECT json FROM app_state",[],|r|r.get(0)).unwrap();serde_json::from_str::<Value>(&raw).unwrap()["settings"]["model"].as_str().unwrap().into()
    }
    #[test]
    fn scheduled_restore_activates_only_at_boot_and_preserves_original_history_without_credentials() {
        let f=fixture();let p=plan(&f);let target=profile_path(&f.base,&p.target_profile).unwrap();
        assert!(target.join("environment-history.sqlite3").exists());assert!(!target.join("environment.sqlite3").exists());
        assert_eq!(Connection::open(target.join("environment-history.sqlite3")).unwrap().query_row("SELECT json_extract(data,'$.lastOutput') FROM environment_state WHERE id=1",[],|r|r.get::<_,String>(0)).unwrap(),"installation-owned-state");
        assert!(f.service.schedule(Decision{id:p.id.clone(),digest:"forged".into()},&f.store).is_err());
        f.service.schedule(decision(&p),&f.store).unwrap();
        assert_eq!(f.store.snapshot().unwrap().settings.model,"live-original-model");assert!(!f.base.join("active-profile.json").exists());
        assert!(f.service.status().unwrap()["restartRequired"].as_bool().unwrap());
        drop(f.store);drop(f.service);
        let lease=crate::storage::open(&f.base,&f.base.join("unused-legacy"),Some(&f.base)).unwrap();assert_eq!(lease.root,target);
        let activated=Store::open(lease.root.clone()).unwrap();assert_eq!(activated.snapshot().unwrap().settings.model,"snapshot-model");
        assert_eq!(database_model(&f.base),"live-original-model");
        let ai=Connection::open(lease.root.join("assistant.sqlite3")).unwrap();
        assert_eq!(ai.query_row("SELECT count(*) FROM vault",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(ai.query_row("SELECT text FROM messages",[],|r|r.get::<_,String>(0)).unwrap(),"preserved historical text");
        let original=Connection::open(f.base.join("assistant.sqlite3")).unwrap();assert_eq!(original.query_row("SELECT count(*) FROM vault",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert!(f.base.join("environment.sqlite3").exists());
        let service=ActivationService::new(lease.base.clone(),lease.root.clone()).unwrap();assert_eq!(service.load(&p.id).unwrap().status,"committed");assert!(service.pending().unwrap().is_none());
        let pointer=read_pointer(&f.base).unwrap().unwrap();assert_eq!(pointer.previous,ProfileRef::Base);assert_eq!(pointer.profile,p.target_profile);
    }
    #[test]
    fn changed_prepared_snapshot_blocks_boot_without_switching_or_deleting_profiles() {
        let f=fixture();let p=plan(&f);let target=profile_path(&f.base,&p.target_profile).unwrap();f.service.schedule(decision(&p),&f.store).unwrap();
        let db=Connection::open(target.join("openkoda.sqlite3")).unwrap();db.execute("UPDATE app_state SET json=json_set(json,'$.settings.model','tampered-copy')",[]).unwrap();drop(db);
        drop(f.store);let result=boot(&f.base).unwrap();assert_eq!(result.root,f.base);assert!(result.notice.unwrap().contains("bloqueada"));
        assert!(!f.base.join("active-profile.json").exists());assert!(target.exists());assert_eq!(database_model(&f.base),"live-original-model");assert_eq!(f.service.load(&p.id).unwrap().status,"blocked");
    }
    #[test]
    fn cancellation_keeps_prepared_copy_and_current_profile_unchanged() {
        let f=fixture();let p=plan(&f);let target=profile_path(&f.base,&p.target_profile).unwrap();f.service.schedule(decision(&p),&f.store).unwrap();
        assert!(f.service.cancel(Decision{id:p.id.clone(),digest:"wrong".into()}).is_err());
        assert_eq!(f.service.cancel(decision(&p)).unwrap().status,"canceled");drop(f.store);
        assert_eq!(boot(&f.base).unwrap().root,f.base);assert!(target.exists());assert!(!f.base.join("active-profile.json").exists());
    }
    #[test]
    fn rollback_is_a_new_reviewed_sanitized_copy_of_the_previous_profile() {
        let f=fixture();let first=plan(&f);f.service.schedule(decision(&first),&f.store).unwrap();drop(f.store);drop(f.service);
        let lease=crate::storage::open(&f.base,&f.base.join("unused"),Some(&f.base)).unwrap();let first_root=lease.root.clone();
        let active=Store::open(first_root.clone()).unwrap();active.mutate(|s|{s.settings.model="edited-recovered-profile".into();Ok(())}).unwrap();
        let service=ActivationService::new(f.base.clone(),first_root.clone()).unwrap();let rollback=service.prepare_rollback(&active).unwrap();
        assert_eq!(rollback.kind,"rollback");assert_eq!(rollback.source_profile,ProfileRef::Base);assert_ne!(rollback.target_profile,first.target_profile);
        service.schedule(decision(&rollback),&active).unwrap();assert_eq!(active.snapshot().unwrap().settings.model,"edited-recovered-profile");
        drop(active);drop(service);drop(lease);
        let next=crate::storage::open(&f.base,&f.base.join("unused"),Some(&f.base)).unwrap();
        assert_ne!(next.root,first_root);assert_ne!(next.root,f.base);assert_eq!(database_model(&next.root),"live-original-model");
        assert_eq!(database_model(&first_root),"edited-recovered-profile");assert_eq!(database_model(&f.base),"live-original-model");
        assert_eq!(Connection::open(next.root.join("assistant.sqlite3")).unwrap().query_row("SELECT count(*) FROM vault",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert!(f.base.join("profile-pointer-history").read_dir().unwrap().next().is_some());
    }
    #[test]
    fn interrupted_pointer_commit_is_reconciled_without_creating_another_profile() {
        let f=fixture();let p=plan(&f);f.service.schedule(decision(&p),&f.store).unwrap();drop(f.store);
        let pointer=ActivePointer{format:1,profile:p.target_profile.clone(),previous:ProfileRef::Base,activation_id:p.id.clone(),committed_at:model::now()};
        commit_pointer(&f.base,&pointer,None).unwrap(); // Simulated crash before the SQLite intent was acknowledged.
        let count=fs::read_dir(f.base.join("profiles")).unwrap().count();
        let result=boot(&f.base).unwrap();assert_eq!(result.root,profile_path(&f.base,&p.target_profile).unwrap());
        assert_eq!(fs::read_dir(f.base.join("profiles")).unwrap().count(),count);assert_eq!(f.service.load(&p.id).unwrap().status,"committed");assert!(f.service.pending().unwrap().is_none());
    }
    #[test]
    fn live_instance_and_global_lease_block_profile_switching() {
        let f=fixture();let p=plan(&f);f.service.schedule(decision(&p),&f.store).unwrap();
        assert!(boot(&f.base).is_err());assert!(!f.base.join("active-profile.json").exists());drop(f.store);
        let lease=crate::storage::open(&f.base,&f.base.join("unused"),Some(&f.base)).unwrap();
        assert!(crate::storage::open(&f.base,&f.base.join("unused"),Some(&f.base)).is_err());drop(lease);
    }
    #[test]
    fn expired_or_escaped_activation_never_switches_to_an_unreviewed_path() {
        let f=fixture();assert!(f.service.prepare_restore("../../outside",&f.store).is_err());let mut p=plan(&f);
        p.expires_at=(chrono::Utc::now()-chrono::Duration::seconds(1)).to_rfc3339();p.digest=digest(&p);f.service.save(&p).unwrap();
        assert!(f.service.schedule(decision(&p),&f.store).is_err());
        p.expires_at=(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339();p.target_profile=ProfileRef::Profile{id:"../../outside".into()};p.digest=digest(&p);f.service.save(&p).unwrap();
        assert!(f.service.schedule(decision(&p),&f.store).is_err());assert!(!f.base.join("active-profile.json").exists());
    }
    #[test]
    fn prepared_copy_cannot_smuggle_live_environment_or_vault_back_into_activation() {
        let f=fixture();let mut p=plan(&f);let target=profile_path(&f.base,&p.target_profile).unwrap();
        Connection::open(target.join("environment.sqlite3")).unwrap().execute_batch("CREATE TABLE test(value TEXT)").unwrap();assert!(validate_target(&f.base,&p).is_err());
        fs::remove_file(target.join("environment.sqlite3")).unwrap();
        let ai=Connection::open(target.join("assistant.sqlite3")).unwrap();ai.execute("INSERT INTO vault VALUES(1,?1,'x')",[b"unexpected-cipher".as_slice()]).unwrap();drop(ai);
        let file=p.files.iter_mut().find(|f|f.name=="assistant.sqlite3").unwrap();file.sha256=recovery::hash_file(&target.join(&file.name)).unwrap();file.bytes=fs::metadata(target.join(&file.name)).unwrap().len();p.digest=digest(&p);
        assert!(validate_target(&f.base,&p).is_err());assert_eq!(database_model(&f.base),"live-original-model");
    }
    #[test]
    fn scheduling_blocks_new_effects_and_cancel_or_blocked_state_releases_the_gate() {
        let f=fixture();assert!(!f.service.blocks_new_effects().unwrap());let p=plan(&f);
        f.service.schedule(decision(&p),&f.store).unwrap();assert!(f.service.blocks_new_effects().unwrap());
        f.service.cancel(decision(&p)).unwrap();assert!(!f.service.blocks_new_effects().unwrap());
        let p=plan(&f);f.service.schedule(decision(&p),&f.store).unwrap();let mut blocked=f.service.load(&p.id).unwrap();blocked.status="blocked".into();f.service.save(&blocked).unwrap();
        assert!(!f.service.blocks_new_effects().unwrap());
    }
    #[test]
    fn work_started_after_scheduling_blocks_boot_and_preserves_the_original_unknown_work() {
        let f=fixture();let p=plan(&f);f.service.schedule(decision(&p),&f.store).unwrap();
        let project=f.store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Late fixture","kind":"web","target":"https://lab.invalid","description":""})}).unwrap();
        f.store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Late work","kind":"web","target":"https://lab.invalid/","objective":"Synthetic operation after schedule","scope":crate::scope::test_creation(&f.store,project["id"].as_str().unwrap(),&["http.headers.v1"])})}).unwrap();
        f.store.mutate(|s|{s.assessments[0].status="running".into();Ok(())}).unwrap();drop(f.store);
        let selected=boot(&f.base).unwrap();assert_eq!(selected.root,f.base);assert!(!f.base.join("active-profile.json").exists());assert_eq!(f.service.load(&p.id).unwrap().status,"blocked");
        let reopened=Store::open(selected.root).unwrap();assert_eq!(reopened.snapshot().unwrap().assessments[0].status,"unknown");assert!(!f.service.blocks_new_effects().unwrap());
    }
    #[test]
    fn source_mcp_patch_and_purge_journals_are_rechecked_at_boot_after_scheduling() {
        for (name,table,insert) in [
            ("kali-bridge.sqlite3","bridge_runs","INSERT INTO bridge_runs(id,data) VALUES(?1,?2)"),
            ("remediation.sqlite3","remediation_plans","INSERT INTO remediation_plans(id,finding_id,data) VALUES(?1,'synthetic-fixture',?2)"),
            ("project-purge.sqlite3","project_purge_plans","INSERT INTO project_purge_plans(id,data) VALUES(?1,?2)"),
        ] {
            let f=fixture();drop(crate::project_purge::ProjectPurgeService::new(f.base.clone()).unwrap());let p=plan(&f);f.service.schedule(decision(&p),&f.store).unwrap();
            let db=Connection::open(f.base.join(name)).unwrap();db.execute(insert,params![model::id(),json!({"status":"unknown"}).to_string()]).unwrap();drop(db);drop(f.store);
            let selected=boot(&f.base).unwrap();assert_eq!(selected.root,f.base,"source {table} was hidden");assert_eq!(f.service.load(&p.id).unwrap().status,"blocked");assert!(!f.base.join("active-profile.json").exists());
        }
    }
    #[test]
    fn install_base_maintenance_and_pending_inference_block_boot_without_hiding_state() {
        for environment in [true,false] {
            let f=fixture();let p=plan(&f);f.service.schedule(decision(&p),&f.store).unwrap();
            if environment {let db=Connection::open(f.base.join("environment.sqlite3")).unwrap();db.execute("UPDATE environment_state SET data=json_set(data,'$.phase','maintenance') WHERE id=1",[]).unwrap();}
            else {let db=Connection::open(f.base.join("assistant.sqlite3")).unwrap();db.execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created) VALUES(?1,'fixture','https://api.openai.com/v1','fixture','fixture','fixture','pending',?2)",params![model::id(),model::now()]).unwrap();}
            drop(f.store);assert_eq!(boot(&f.base).unwrap().root,f.base);assert_eq!(f.service.load(&p.id).unwrap().status,"blocked");assert!(!f.base.join("active-profile.json").exists());
        }
    }
    #[test]
    fn degraded_environment_requires_confirmed_nonzero_exit_without_owned_registration() {
        let f=fixture();let db=Connection::open(f.base.join("environment.sqlite3")).unwrap();
        for state in [json!({"phase":"degraded","registration":null}),json!({"phase":"degraded","registration":null,"installExit":{"confirmed":false,"code":1}}),json!({"phase":"degraded","registration":null,"installExit":{"confirmed":true,"code":0}}),json!({"phase":"degraded","registration":{"guid":"owned"},"installExit":{"confirmed":true,"code":1}})] {
            db.execute("UPDATE environment_state SET data=?1 WHERE id=1",[state.to_string()]).unwrap();assert!(ensure_environment_idle(&f.base).is_err());
        }
        db.execute("UPDATE environment_state SET data=?1 WHERE id=1",[json!({"phase":"degraded","registration":null,"installExit":{"confirmed":true,"code":1}}).to_string()]).unwrap();assert!(ensure_environment_idle(&f.base).is_ok());
    }
    #[test]
    fn unknown_provider_result_must_remain_in_the_reviewed_history_copy() {
        let f=fixture();let p=plan(&f);let target=profile_path(&f.base,&p.target_profile).unwrap();
        let db=Connection::open(f.base.join("assistant.sqlite3")).unwrap();let id=model::id();
        db.execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created) VALUES(?1,'fixture','https://api.openai.com/v1','fixture','fixture','fixture','unknown',?2)",params![id,model::now()]).unwrap();
        assert!(ensure_source_idle(&f.base).is_ok());assert!(preserve_unknown_inference(&f.base,&target).is_err());
        assert_eq!(merge_unknown_requests(&f.base,&target).unwrap(),1);
        assert!(preserve_unknown_inference(&f.base,&target).is_ok());
    }
}
