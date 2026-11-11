//! Human-reviewed, single-file Windows remediation with durable unknown states.
//! Writes use one exclusive handle; they are NOT an atomic file replacement.
//! A crash can leave partial bytes. No startup or error path replays the patch.
use crate::{core::{self, Store}, model::{self, Evidence, Snapshot, ValidatedEvidence, ValidationRecord}};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs::{self, File}, io::{Read, Seek, SeekFrom, Write}, path::{Component, Path, PathBuf}, sync::Mutex, time::Duration};

const MAX_FILE: usize = 256 * 1024;
const MAX_PLANS: i64 = 100;
const LIMITS: [&str; 4] = [
    "Somente um arquivo UTF-8 de até 256 KiB dentro do alvo local code/files; sem execução de código.",
    "O reteste verifica apenas a condição textual ou SHA-256 escolhida; não prova ausência de outras vulnerabilidades.",
    "A escrita usa handle exclusivo Windows, mas não é substituição atômica; interrupção pode deixar bytes parciais e exige revisão.",
    "Antes/depois e backup permanecem no armazenamento local de correções; não são enviados automaticamente à IA.",
];

pub struct RemediationService { db: Mutex<Connection>, operation: Mutex<()> }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct FileIdentity { volume: u32, index: u64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct RetestRule { kind: String, value: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct RemediationPlan {
    id:String, project_id:String, assessment_id:String, finding_id:String,
    target:String, relative_path:String, file_path:String, identity:FileIdentity,
    before:String, after:String, base_sha256:String, after_sha256:String,
    retest:RetestRule, digest:String, created_at:String, expires_at:String,
    status:String, approved_at:Option<String>, applied_at:Option<String>,
    retested_at:Option<String>, confirmed_at:Option<String>, evidence_id:Option<String>,
    passed:Option<bool>, message:String, limits:Vec<String>,
    #[serde(default)]
    interrupted_operation:Option<String>,
    #[serde(default)]
    reconciliation:Option<ReconciliationReceipt>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    scope_ref:Option<crate::scope::ScopeRef>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    retest_scope_ref:Option<crate::scope::ScopeRef>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    target_ref:Option<crate::scope_targets::ScopeTargetRef>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    identity_preflight_refs:Vec<crate::scope_targets::ScopeTargetRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReconciliationObservation {
    state:String, observed_at:String, observed_sha256:Option<String>, observed_bytes:Option<u64>,
    observed_identity:Option<FileIdentity>, identity_matches:Option<bool>, message:String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReconciliationReceipt {
    id:String, evidence_id:String, previous_status:String, interrupted_operation:Option<String>,
    recorded_at:String, author:String, note:String, acknowledge_unknown_effects:bool,
    finding_status:String, observation:ReconciliationObservation,
}

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Prepare { finding_id:String, relative_path:String, after:String, retest:RetestRule,#[serde(default)]scope_ref:Option<crate::scope::ScopeRef>,#[serde(default)]target_ref:Option<crate::scope_targets::ScopeTargetRef> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct FindingInput { finding_id:String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdInput { id:String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision { id:String, digest:String }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Confirmation { id:String, digest:String, evidence_id:String, rationale:String }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Reconcile { id:String, note:String, acknowledge_unknown_effects:bool }

#[derive(Clone)]
struct Scope { project_id:String, assessment_id:String, finding_id:String, target:String,local_exclusions:Vec<crate::scope_targets::LocalExclusion>,combined:bool }

pub fn supports(action:&str)->bool {
    matches!(action,"remediation.prepare"|"remediation.list"|"remediation.get"|"remediation.approve"|"remediation.apply"|"remediation.retest"|"remediation.confirm"|"remediation.reconcile")
}
fn decode<T:serde::de::DeserializeOwned>(v:Value)->Result<T,String> {serde_json::from_value(v).map_err(|_|"Campos de correção inválidos ou não permitidos.".into())}
fn encode<T:Serialize>(v:T)->Result<Value,String> {serde_json::to_value(v).map_err(|_|"Falha ao apresentar a correção.".into())}
fn db_error(_:impl std::fmt::Display)->String {"Falha no registro de correção. Os dados foram preservados; não repita uma escrita com efeito desconhecido.".into()}
fn platform()->Result<(),String> {if cfg!(windows){Ok(())}else{Err("Correções locais exigem Windows com bloqueio exclusivo; esta plataforma não foi liberada.".into())}}
fn hash(s:&str)->String {core::hash(s)}
fn valid_sha(value:&Value)->bool {value.as_str().is_some_and(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()))}
fn plan_digest(p:&RemediationPlan)->String {
    let mut value=json!({"id":p.id,"projectId":p.project_id,"assessmentId":p.assessment_id,"findingId":p.finding_id,
        "target":p.target,"relativePath":p.relative_path,"filePath":p.file_path,"identity":p.identity,
        "baseSha256":p.base_sha256,"afterSha256":p.after_sha256,"retest":p.retest,
        "createdAt":p.created_at,"expiresAt":p.expires_at,"limits":p.limits});
    if let Some(scope)=&p.scope_ref{value["scopeRef"]=json!(scope);}if let Some(target)=&p.target_ref{value["targetRef"]=json!(target);}if !p.identity_preflight_refs.is_empty(){value["identityPreflightRefs"]=json!(p.identity_preflight_refs);}hash(&value.to_string())
}
fn rule_valid(rule:&RetestRule)->Result<(),String> {
    match rule.kind.as_str() {
        JSON_DEBUG_RULE if rule.value == "false" => Ok(()),
        "contains"|"absent" if !rule.value.is_empty() && rule.value.len()<=4096 && !rule.value.contains('\0') => Ok(()),
        "sha256" if rule.value.len()==64 && rule.value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) => Ok(()),
        _=>Err("Condição de reteste inválida. Confira o texto, o SHA-256 em minúsculas ou a regra de depuração JSON.".into()),
    }
}
const JSON_DEBUG_RULE: &str = "json.debug-disabled.v1";
const RETEST_RULE_KINDS: [&str; 4] = ["contains", "absent", "sha256", JSON_DEBUG_RULE];

fn supported_retest_proof(proof: &Value) -> bool {
    proof["ruleKind"].as_str().is_some_and(|kind| RETEST_RULE_KINDS.contains(&kind))
}

fn json_debug_flag(text: &str) -> Option<bool> {
    #[derive(Deserialize)]
    struct Config { debug: bool }
    // Derived structs accept positional arrays too. Config files here must be
    // objects with exactly one top-level boolean field named "debug".
    if !text.trim_start().starts_with('{') { return None; }
    serde_json::from_str::<Config>(text).ok().map(|cfg| cfg.debug)
}

fn retest_limits(kind: &str) -> [&'static str; 4] {
    let mut limits = LIMITS;
    if kind == JSON_DEBUG_RULE {
        limits[1] = "Verifica o booleano debug na raiz do JSON selecionado. O carregamento desse arquivo pelo aplicativo exige confirmação separada.";
    }
    limits
}

fn evaluate(rule:&RetestRule,text:&str)->bool {
    match rule.kind.as_str() {
        "contains" => text.contains(&rule.value),
        "absent" => !text.contains(&rule.value),
        "sha256" => hash(text) == rule.value,
        JSON_DEBUG_RULE => json_debug_flag(text) == Some(false),
        _ => false,
    }
}

fn scope(snapshot:&Snapshot,finding_id:&str,history:bool)->Result<Scope,String> {
    selected_scope(snapshot,finding_id,history,None)
}
fn selected_scope(snapshot:&Snapshot,finding_id:&str,history:bool,target:Option<&crate::scope_targets::ScopeTargetRef>)->Result<Scope,String>{
    core::valid_id(finding_id)?;
    let f=snapshot.findings.iter().find(|f|f.id==finding_id).ok_or("Achado não encontrado.")?;
    if crate::http_validation::is_typed(snapshot,finding_id)? {return Err("Achado HTTP usa nova observação e reteste factual; o fluxo de patch local não altera o servidor remoto.".into());}
    if !history && f.status!="validated" {return Err("A correção exige achado validado e revisão humana prévia.".into());}
    let a=snapshot.assessments.iter().find(|a|a.id==f.assessment_id).ok_or("Avaliação do achado não encontrada.")?;
    let p=snapshot.projects.iter().find(|p|p.id==a.project_id).ok_or("Projeto da avaliação não encontrado.")?;
    let combined=snapshot.scope_revisions.iter().any(|revision|revision.assessment_id==a.id&&revision.body.target_set.is_some());
    let selected=if combined&&(target.is_some()||!history){Some(crate::scope_targets::resolve_current_target(snapshot,&a.id,target)?)}else{None};
    if target.is_some()&&!combined{return Err("Referência de alvo extra em correção de escopo simples.".into());}
    let kind=selected.as_ref().map_or(p.kind.as_str(),|value|value.kind.as_str());
    if (!history && p.archived) || (!combined&&(!matches!(kind,"code"|"files") || a.kind!=p.kind || a.target!=p.target)) || (selected.is_some()&&!matches!(kind,"code"|"files")) {
        return Err("Correção exige projeto local code/files ativo com o mesmo alvo da avaliação.".into());
    }
    if !history && (core::active(&a.status) || a.status=="awaiting_approval" || snapshot.plans.iter().any(|p|p.assessment_id==a.id&&matches!(p.status.as_str(),"approved"|"running"|"canceling"|"unknown"))) {
        return Err("Encerre ou reconcilie a avaliação e seus planos antes de corrigir arquivos.".into());
    }
    Ok(Scope {project_id:p.id.clone(),assessment_id:a.id.clone(),finding_id:f.id.clone(),target:selected.as_ref().map_or_else(||a.target.clone(),|value|value.target.clone()),local_exclusions:selected.map(|value|value.local_exclusions).unwrap_or_default(),combined})
}
fn bound_scope(s:&Snapshot,p:&RemediationPlan)->Result<Scope,String> {
    if let Some(reference)=&p.scope_ref{crate::scope::validate_historical_ref(s,&p.assessment_id,reference)?;}
    let current=selected_scope(s,&p.finding_id,false,p.target_ref.as_ref())?;
    if let Some(target)=&p.target_ref{if !crate::evidence_lineage::finding_source_refs(s,&p.finding_id)?.contains(target){return Err("A correção aponta para entrada sem evidência no achado.".into());}}
    check_relative_policy(&current,&p.relative_path)?;
    if let Some(reference)=&p.scope_ref{crate::local_scope::validate_refs(s,reference,p.target_ref.as_ref(),&p.identity_preflight_refs)?;}
    if current.project_id!=p.project_id || current.assessment_id!=p.assessment_id || current.target!=p.target {
        return Err("O projeto, avaliação ou alvo mudou desde a revisão; recalcule o patch.".into());
    }
    Ok(current)
}

fn reconciliation_scope(s:&Snapshot,p:&RemediationPlan)->Result<(),String> {
    // Archived projects and a confirmation already committed before a crash
    // remain inspectable. Reconciliation never changes the finding's status.
    let current=selected_scope(s,&p.finding_id,true,p.target_ref.as_ref())?;
    check_relative_policy(&current,&p.relative_path)?;
    if p.target_ref.is_some(){let reference=p.scope_ref.as_ref().ok_or("Reconciliação combinada sem revisão.")?;crate::local_scope::validate_refs(s,reference,p.target_ref.as_ref(),&p.identity_preflight_refs)?;}
    if current.project_id!=p.project_id || current.assessment_id!=p.assessment_id || current.target!=p.target {
        return Err("O vínculo original da correção mudou; a reconciliação não pode observar outro alvo.".into());
    }
    let assessment=s.assessments.iter().find(|a|a.id==p.assessment_id).ok_or("Avaliação não encontrada.")?;
    if core::active(&assessment.status) || assessment.status=="awaiting_approval" || s.plans.iter().any(|plan|plan.assessment_id==assessment.id&&matches!(plan.status.as_str(),"approved"|"running"|"canceling"|"unknown")) {
        return Err("Encerre ou reconcilie a avaliação antes de inspecionar a correção interrompida.".into());
    }
    Ok(())
}

fn check_relative_policy(scope:&Scope,relative:&str)->Result<(),String>{
    relative_path(relative)?;
    let normalized=relative.replace('\\',"/");
    if crate::scope_targets::local_excluded(&scope.local_exclusions,&normalized).is_some(){return Err("scope_local_path_excluded: arquivo excluído não pode ser lido, corrigido ou retestado.".into());}
    if scope.combined&&normalized.rsplit_once('/').is_some_and(|(parents,_)|parents.split('/').any(|part|[".git","node_modules","target"].iter().any(|name|part.eq_ignore_ascii_case(name)))){return Err("Arquivo está em diretório excluído pela política local.".into());}Ok(())
}

fn selected_refs(p:&RemediationPlan)->Vec<crate::scope_targets::ScopeTargetRef>{p.target_ref.iter().cloned().collect()}
fn verify_identity_handoff(expected:Option<&crate::source_scan::FileIdentity>,actual:&FileIdentity)->Result<(),String>{
    if expected.is_some_and(|identity|identity.volume!=actual.volume.to_string()||identity.index!=actual.index.to_string()){return Err("Identidade mudou durante transferência do preflight ao handle exclusivo; nenhum conteúdo foi lido ou escrito.".into());}Ok(())
}

impl RemediationService {
    pub fn new(root:PathBuf)->Result<Self,String> {
        fs::create_dir_all(&root).map_err(db_error)?;
        let mut db=Connection::open(root.join("remediation.sqlite3")).map_err(db_error)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;
        let integrity:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(db_error)?;
        if integrity!="ok" {return Err("Registro de correções inconsistente; nenhum dado foi apagado.".into());}
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS remediation_plans(id TEXT PRIMARY KEY, finding_id TEXT NOT NULL, data TEXT NOT NULL, backup BLOB);").map_err(db_error)?;
        let rows:Vec<(String,String)>={let mut q=db.prepare("SELECT id,data FROM remediation_plans").map_err(db_error)?;
            let rows=q.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
            rows.collect::<Result<_,_>>().map_err(db_error)?};
        if rows.len()>MAX_PLANS as usize {return Err("Registro de correções excede o limite; preserve os dados e revise o armazenamento.".into());}
        let tx=db.transaction().map_err(db_error)?;
        for (id,raw) in rows {
            let mut p=read_plan(&raw)?;
            if p.id!=id {return Err("Identidade do plano de correção inconsistente.".into());}
            if matches!(p.status.as_str(),"applying"|"retesting"|"confirming") {
                p.interrupted_operation=Some(p.status.clone());
                p.status="unknown".into();p.message="Aplicativo interrompido durante uma operação. O arquivo pode conter bytes parciais; nenhum patch foi repetido. Revise o arquivo e o backup local.".into();
            } else if p.status=="approved" {
                p.status="revoked".into();p.message="Aprovação revogada após reinício; prepare nova revisão antes de escrever.".into();
            } else {continue;}
            tx.execute("UPDATE remediation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(&p).map_err(db_error)?,id]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)?;
        Ok(Self {db:Mutex::new(db),operation:Mutex::new(())})
    }

    pub fn has_unresolved(&self)->Result<bool,String>{
        let db=self.db.lock().map_err(db_error)?;
        let count:i64=db.query_row("SELECT count(*) FROM remediation_plans WHERE json_extract(data,'$.status') IN ('applying','retesting','confirming','unknown')",[],|r|r.get(0)).map_err(db_error)?;
        Ok(count>0)
    }
    pub fn handle(&self,action:&str,payload:Value,store:&Store)->Result<Value,String> {
        if !supports(action) || !payload.is_object() || payload.to_string().len()>2*1024*1024 {return Err("Solicitação de correção inválida.".into());}
        let _operation=self.operation.try_lock().map_err(|_|"Outra operação de correção está em andamento.".to_string())?;
        match action {
            "remediation.list"=>{let input:FindingInput=decode(payload)?;self.list(&input.finding_id,store)},
            "remediation.get"=>{let input:IdInput=decode(payload)?;let p=self.load(&input.id)?;scope(&store.snapshot()?,&p.finding_id,true)?;encode(p)},
            "remediation.prepare"=>self.prepare(decode(payload)?,store).and_then(encode),
            "remediation.approve"=>self.approve(decode(payload)?,store).and_then(encode),
            "remediation.apply"=>self.apply(decode(payload)?,store).and_then(encode),
            "remediation.retest"=>self.retest(decode(payload)?,store).and_then(encode),
            "remediation.confirm"=>self.confirm(decode(payload)?,store).and_then(encode),
            "remediation.reconcile"=>self.reconcile(decode(payload)?,store).and_then(encode),
            _=>Err("Ação de correção não disponível.".into()),
        }
    }
    fn load(&self,id:&str)->Result<RemediationPlan,String> {
        core::valid_id(id)?;
        let raw:Option<String>=self.db.lock().map_err(db_error)?.query_row("SELECT data FROM remediation_plans WHERE id=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
        let p=read_plan(&raw.ok_or("Plano de correção não encontrado.")?)?;
        if p.id!=id {return Err("Identidade do plano de correção inconsistente.".into());}Ok(p)
    }
    fn save(&self,p:&RemediationPlan)->Result<(),String> {
        let updated=self.db.lock().map_err(db_error)?.execute("UPDATE remediation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(p).map_err(db_error)?,p.id]).map_err(db_error)?;
        if updated!=1{return Err("Plano de correção desapareceu; não repetir a operação.".into());}Ok(())
    }
    fn decision(&self,d:Decision,expected:&str,check_expiry:bool)->Result<RemediationPlan,String> {
        let p=self.load(&d.id)?;
        if p.digest!=d.digest || p.status!=expected {return Err("Digest ou estado da correção mudou; a aprovação não pode ser reutilizada.".into());}
        if check_expiry && chrono::DateTime::parse_from_rfc3339(&p.expires_at).map_err(db_error)?<=chrono::Utc::now(){return Err("Plano de correção expirado; prepare e revise novamente.".into());}
        Ok(p)
    }
    fn list(&self,finding_id:&str,store:&Store)->Result<Value,String> {
        scope(&store.snapshot()?,finding_id,true)?;
        let db=self.db.lock().map_err(db_error)?;let mut query=db.prepare("SELECT data FROM remediation_plans WHERE finding_id=?1 ORDER BY rowid DESC").map_err(db_error)?;
        let mut plans=Vec::new();let rows=query.query_map([finding_id],|r|r.get::<_,String>(0)).map_err(db_error)?;
        for row in rows {plans.push(read_plan(&row.map_err(db_error)?)?);}
        Ok(json!({"plans":plans}))
    }
    fn prepare(&self,input:Prepare,store:&Store)->Result<RemediationPlan,String> {
        platform()?;rule_valid(&input.retest)?;
        if input.after.len()>MAX_FILE || input.after.contains('\0') {return Err("Novo conteúdo exige UTF-8 sem NUL e até 256 KiB.".into());}
        let snapshot=store.snapshot()?;
        let current=selected_scope(&snapshot,&input.finding_id,false,input.target_ref.as_ref())?;
        check_relative_policy(&current,&input.relative_path)?;
        if let Some(target)=&input.target_ref{if input.scope_ref.is_none()||!crate::evidence_lineage::finding_target_refs(store,&snapshot,&input.finding_id)?.contains(target){return Err("Escolha uma revisão e uma entrada comprovadas pela origem do achado.".into());}}
        let refs=input.target_ref.iter().cloned().collect::<Vec<_>>();
        let reference=crate::scope::with_selected_admission(store,&current.assessment_id,"local.text-remediation.v1",input.scope_ref.as_ref(),&refs,"remediation.prepare",|reference|Ok(reference.clone()))?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&refs)?;
        let mut preflight=crate::local_scope::preflight(&store.snapshot()?,&reference,input.target_ref.as_ref())?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&refs)?;
        let identity=preflight.release_selected_file(input.target_ref.as_ref())?;
        let mut locked=LockedTarget::open(&current.target,&input.relative_path,false)?;
        verify_identity_handoff(identity.as_ref(),&locked.identity)?;
        let before=locked.read()?;
        if before==input.after {return Err("O patch não contém alteração.".into());}
        if input.retest.kind == JSON_DEBUG_RULE && json_debug_flag(&before) != Some(true) {
            return Err("A base precisa conter o campo debug com o valor booleano true na raiz do objeto JSON.".into());
        }
        let limits = retest_limits(&input.retest.kind);
        if evaluate(&input.retest,&before) || !evaluate(&input.retest,&input.after) {return Err("A condição do reteste deve falhar na base e passar no conteúdo proposto.".into());}
        let mut p=RemediationPlan {id:model::id(),project_id:current.project_id,assessment_id:current.assessment_id,finding_id:current.finding_id,
            target:current.target,relative_path:input.relative_path,file_path:locked.path.to_string_lossy().into_owned(),identity:locked.identity.clone(),
            base_sha256:hash(&before),after_sha256:hash(&input.after),before,after:input.after,retest:input.retest,digest:String::new(),
            created_at:model::now(),expires_at:(chrono::Utc::now()+chrono::Duration::minutes(15)).to_rfc3339(),status:"proposed".into(),
            approved_at:None,applied_at:None,retested_at:None,confirmed_at:None,evidence_id:None,passed:None,
            interrupted_operation:None,reconciliation:None,scope_ref:Some(reference.clone()),retest_scope_ref:None,target_ref:input.target_ref,identity_preflight_refs:preflight.refs.clone(),
            message:"Revise antes/depois, alvo e condição do reteste. Preparar não aprova nem aplica a escrita.".into(),limits:limits.iter().map(|s|s.to_string()).collect()};
        p.digest=plan_digest(&p);
        store.mutate(|s|{
            bound_scope(s,&p)?;
            crate::scope::check_selected_work(s,&p.assessment_id,"local.text-remediation.v1",Some(&reference),&selected_refs(&p))?;
            let db=self.db.lock().map_err(db_error)?;
            let count:i64=db.query_row("SELECT count(*) FROM remediation_plans",[],|r|r.get(0)).map_err(db_error)?;
            if count>=MAX_PLANS {return Err("Limite local de 100 planos de correção atingido; preserve o histórico antes de ampliar.".into());}
            db.execute("INSERT INTO remediation_plans(id,finding_id,data) VALUES(?1,?2,?3)",params![p.id,p.finding_id,serde_json::to_string(&p).map_err(db_error)?]).map_err(db_error)?;
            Ok(())
        })?;
        Ok(p)
    }
    fn approve(&self,d:Decision,store:&Store)->Result<RemediationPlan,String> {
        platform()?;let mut p=self.decision(d,"proposed",true)?;
        let reference=p.scope_ref.clone().ok_or("scope_legacy_plan_requires_reprepare")?;
        store.mutate(|s|{bound_scope(s,&p)?;crate::scope::check_selected_work(s,&p.assessment_id,"local.text-remediation.v1",Some(&reference),&selected_refs(&p))?;p.status="approved".into();p.approved_at=Some(model::now());
            p.message="Escrita exata aprovada uma vez. Aplicar é uma ação separada; a base será conferida novamente.".into();self.save(&p)})?;
        Ok(p)
    }
    fn apply(&self,d:Decision,store:&Store)->Result<RemediationPlan,String> {
        platform()?;let mut p=self.decision(d,"approved",true)?;
        bound_scope(&store.snapshot()?,&p)?;
        let reference=crate::scope::with_selected_admission(store,&p.assessment_id,"local.text-remediation.v1",Some(p.scope_ref.as_ref().ok_or("scope_legacy_plan_requires_reprepare")?),&selected_refs(&p),"remediation.apply",|reference|Ok(reference.clone()))?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&selected_refs(&p))?;
        let mut _preflight=crate::local_scope::preflight(&store.snapshot()?,&reference,p.target_ref.as_ref())?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&selected_refs(&p))?;
        let identity=_preflight.release_selected_file(p.target_ref.as_ref())?;
        let mut locked=LockedTarget::open(&p.target,&p.relative_path,true)?;
        verify_identity_handoff(identity.as_ref(),&locked.identity)?;
        if locked.identity!=p.identity || locked.path.to_string_lossy()!=p.file_path || hash(&locked.read()?)!=p.base_sha256 {
            p.status="invalidated".into();p.message="Base ou identidade do arquivo mudou. Nenhum byte foi escrito; prepare um novo patch.".into();self.save(&p)?;return Err(p.message);
        }
        let mut wrote=false;
        let outcome=store.mutate(|s|{
            bound_scope(s,&p)?;
            crate::scope::check_selected_work(s,&p.assessment_id,"local.text-remediation.v1",Some(&reference),&selected_refs(&p))?;
            if chrono::DateTime::parse_from_rfc3339(&p.expires_at).map_err(db_error)?<=chrono::Utc::now() {return Err("A aprovação expirou antes da escrita; prepare nova revisão.".into());}
            // This FULL-synchronous journal and original-byte backup commit
            // before the first write, while the target handle is exclusive.
            p.status="applying".into();p.message="Escrita iniciada; interrupção exige revisão, nunca replay automático.".into();
            self.db.lock().map_err(db_error)?.execute("UPDATE remediation_plans SET data=?1,backup=?2 WHERE id=?3",params![serde_json::to_string(&p).map_err(db_error)?,p.before.as_bytes(),p.id]).map_err(db_error)?;
            wrote=true;
            locked.write(&p.after)?;
            if locked.identity!=p.identity || hash(&locked.read()?)!=p.after_sha256 {return Err("Conteúdo escrito não passou na verificação.".into());}
            let a=s.assessments.iter_mut().find(|a|a.id==p.assessment_id).ok_or("Avaliação não encontrada.")?;
            a.updated_at=model::now();a.events.push(model::event(format!("Correção {} aplicada ao arquivo selecionado; aguarda reteste e confirmação humana. Não houve execução de código.",p.id),"warning"));
            Ok(())
        });
        if let Err(error)=outcome {
            if wrote {
                // Never close/reopen the file to roll back. A third party cannot
                // edit through ordinary Windows file handles during this lease.
                let rollback=locked.write(&p.before).and_then(|_|locked.read()).map(|s|hash(&s)==p.base_sha256).unwrap_or(false);
                p.status=if rollback{"rolled_back"}else{"unknown"}.into();
                p.message=if rollback {format!("A escrita falhou e os bytes originais foram restaurados e verificados sob o mesmo handle exclusivo. Motivo: {error}")}
                    else {format!("Falha de escrita e restauração não confirmada. O arquivo pode conter bytes parciais; preserve o backup e revise manualmente. Motivo: {error}")};
                self.save(&p)?;return Err(p.message);
            }
            return Err(error);
        }
        p.status="applied".into();p.applied_at=Some(model::now());p.message="Alteração aplicada e hash verificado. O achado não foi marcado corrigido; execute o reteste delimitado.".into();
        self.save(&p)?;Ok(p)
    }
    fn retest(&self,d:Decision,store:&Store)->Result<RemediationPlan,String> {
        platform()?;let loaded=self.load(&d.id)?;
        let expected=if loaded.scope_ref.is_none()&&loaded.status=="retested"{"retested"}else{"applied"};
        let mut p=self.decision(d,expected,false)?;
        bound_scope(&store.snapshot()?,&p)?;
        // A legacy patch already applied may only be reread after prospective
        // scope adoption. Its approval/digest never gain authority to write again.
        let reference=crate::scope::with_selected_admission(store,&p.assessment_id,"local.text-remediation.v1",p.scope_ref.as_ref(),&selected_refs(&p),"remediation.retest",|reference|Ok(reference.clone()))?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&selected_refs(&p))?;
        let mut _preflight=crate::local_scope::preflight(&store.snapshot()?,&reference,p.target_ref.as_ref())?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&selected_refs(&p))?;
        let identity=_preflight.release_selected_file(p.target_ref.as_ref())?;
        let mut locked=LockedTarget::open(&p.target,&p.relative_path,false)?;
        verify_identity_handoff(identity.as_ref(),&locked.identity)?;
        let current=locked.read()?;let actual=hash(&current);
        let passed=locked.identity==p.identity && locked.path.to_string_lossy()==p.file_path && actual==p.after_sha256 && evaluate(&p.retest,&current);
        p.status="retesting".into();p.message="Reteste local em andamento; sem execução de código.".into();self.save(&p)?;
        let mut proof=json!({"schema":"openkoda.remediation.retest.v1","scopeRef":reference,"remediationId":p.id,"projectId":p.project_id,"assessmentId":p.assessment_id,
            "findingId":p.finding_id,"planDigest":p.digest,"target":p.target,"relativePath":p.relative_path,
            "baseSha256":p.base_sha256,"appliedSha256":p.after_sha256,"observedSha256":actual,"passed":passed,
            "ruleKind":p.retest.kind,"ruleSha256":hash(&serde_json::to_string(&p.retest).map_err(db_error)?),"checkedAt":model::now(),"limits":p.limits});
        if let Some(target)=&p.target_ref{proof["targetRef"]=json!(target);proof["identityPreflightRefs"]=json!(p.identity_preflight_refs);}
        let content=proof.to_string();
        let evidence=Evidence {id:model::id(),name:format!("remediation-retest-{}.json",p.id),sha256:hash(&content),content,created_at:model::now()};
        if let Err(error)=store.mutate(|s|{
            bound_scope(s,&p)?;let a=s.assessments.iter_mut().find(|a|a.id==p.assessment_id).ok_or("Avaliação não encontrada.")?;
            a.evidence.push(evidence.clone());a.updated_at=model::now();a.events.push(model::event(if passed{"A condição de reteste passou; confirmação humana separada ainda é necessária."}else{"Reteste falhou ou arquivo mudou; achado permanece aberto."},if passed{"info"}else{"warning"}));Ok(())
        }) {p.status="unknown".into();p.message="Não foi possível confirmar a persistência do reteste; nada foi marcado corrigido.".into();self.save(&p)?;return Err(error);}
        p.status="retested".into();p.retested_at=Some(model::now());p.evidence_id=Some(evidence.id);p.passed=Some(passed);p.retest_scope_ref=Some(reference);
        p.message=if passed{"Reteste delimitado passou. Revise a evidência e os limites antes de confirmar a correção."}else{"Reteste falhou ou arquivo difere da alteração aprovada. Confirmação de corrigido bloqueada."}.into();self.save(&p)?;Ok(p)
    }
    fn reconcile(&self,input:Reconcile,store:&Store)->Result<RemediationPlan,String> {
        platform()?;
        let note=input.note.trim();
        if !input.acknowledge_unknown_effects || note.is_empty() || note.len()>4000 || note.contains('\0') {
            return Err("Reconciliação exige justificativa humana de 1 a 4000 bytes e reconhecimento explícito de que os efeitos históricos podem continuar desconhecidos.".into());
        }
        let mut p=self.load(&input.id)?;
        if !matches!(p.status.as_str(),"applying"|"retesting"|"confirming"|"unknown"|"reconciled") {
            return Err("Somente uma correção interrompida ou com efeito desconhecido pode ser reconciliada; nenhum patch será repetido.".into());
        }
        reconciliation_scope(&store.snapshot()?,&p)?;
        // Preserve the original snapshot and note after a cross-database crash.
        // Only an explicit repeat completes its evidence record; no startup
        // path finalizes it and no repeat reopens or writes the target.
        let mut _inspection_lease=None;
        let mut _identity_lease=None;
        if let Some(receipt)=&p.reconciliation {
            if receipt.note!=note {return Err("A reconciliação interrompida já tem uma justificativa registrada. Conclua usando a mesma justificativa exibida no recibo; a observação conserva sua data original.".into());}
            if p.status=="reconciled" {return Ok(p);}
        } else {
            if p.status=="reconciled" {return Err("Correção reconciliada sem recibo íntegro.".into());}
            // Pure reference/exclusion guards already passed above. An absent
            // or inaccessible root must remain reconcilable, but cannot grant
            // a fallback content read. Inspect only the original recorded path.
            let mut content_allowed=true;
            if p.target_ref.is_some(){let reference=p.scope_ref.as_ref().ok_or("Reconciliação combinada sem revisão.")?;match crate::local_scope::preflight(&store.snapshot()?,reference,p.target_ref.as_ref()){Ok(lease)=>_identity_lease=Some(lease),Err(_)=>content_allowed=false}}
            let identity=_identity_lease.as_mut().map(|lease|lease.release_selected_file(p.target_ref.as_ref())).transpose()?.flatten();
            let (observation,lease)=observe_original_target(&p,identity.as_ref(),content_allowed);
            _inspection_lease=lease;
            let snapshot=store.snapshot()?;
            reconciliation_scope(&snapshot,&p)?;
            let finding=snapshot.findings.iter().find(|f|f.id==p.finding_id).ok_or("Achado não encontrado.")?;
            let interrupted_operation=if matches!(p.status.as_str(),"applying"|"retesting"|"confirming") {Some(p.status.clone())} else {p.interrupted_operation.clone()};
            p.reconciliation=Some(ReconciliationReceipt {id:model::id(),evidence_id:model::id(),previous_status:p.status.clone(),
                interrupted_operation:interrupted_operation.clone(),recorded_at:model::now(),author:"local-user".into(),note:note.into(),
                acknowledge_unknown_effects:true,finding_status:finding.status.clone(),observation});
            p.interrupted_operation=interrupted_operation;
            p.status="unknown".into();
            p.message="Observação de reconciliação preservada; registro da evidência ainda não confirmado. Nenhum arquivo foi escrito e nenhum patch foi repetido.".into();
            self.save(&p)?;
        }
        let receipt=p.reconciliation.as_ref().ok_or("Recibo de reconciliação indisponível.")?;
        let content=reconciliation_content(&p,receipt);
        let evidence=Evidence {id:receipt.evidence_id.clone(),name:format!("remediation-reconciliation-{}.json",p.id),
            sha256:hash(&content),content,created_at:receipt.recorded_at.clone()};
        store.mutate(|s|{
            reconciliation_scope(s,&p)?;
            let a=s.assessments.iter_mut().find(|a|a.id==p.assessment_id).ok_or("Avaliação não encontrada.")?;
            if let Some(existing)=a.evidence.iter().find(|e|e.id==evidence.id) {
                if existing.sha256!=evidence.sha256 || existing.content!=evidence.content {return Err("O identificador do recibo já aponta para outra evidência; histórico preservado.".into());}
            } else {
                a.evidence.push(evidence.clone());a.updated_at=model::now();
                a.events.push(model::event(format!("Correção {} reconciliada por observação local e reconhecimento humano. Nenhum patch foi repetido, arquivo restaurado ou status de achado alterado; efeitos históricos podem continuar desconhecidos.",p.id),"warning"));
            }
            let finding=s.findings.iter_mut().find(|f|f.id==p.finding_id).ok_or("Achado não encontrado.")?;
            if !finding.evidence_ids.contains(&evidence.id) {finding.evidence_ids.push(evidence.id.clone());}
            Ok(())
        })?;
        p.status="reconciled".into();
        p.message="Reconciliação registrada com observação datada e reconhecimento humano. Nenhum patch foi repetido ou arquivo restaurado; o status do achado foi preservado. Efeitos históricos podem continuar desconhecidos. Uma nova alteração exige outro plano revisado.".into();
        self.save(&p)?;Ok(p)
    }
    fn confirm(&self,input:Confirmation,store:&Store)->Result<RemediationPlan,String> {
        platform()?;let mut p=self.decision(Decision{id:input.id,digest:input.digest},"retested",false)?;
        let rationale=input.rationale.trim();
        if rationale.is_empty() || rationale.len()>4000 || rationale.contains('\0') {return Err("Registre justificativa humana de 1 a 4000 bytes, incluindo os limites do reteste.".into());}
        if p.passed!=Some(true) || p.evidence_id.as_deref()!=Some(&input.evidence_id) {return Err("Confirmação exige o reteste aprovado e sua evidência exata.".into());}
        bound_scope(&store.snapshot()?,&p)?;
        let snapshot=store.snapshot()?;
        let raw=snapshot.assessments.iter().find(|a|a.id==p.assessment_id).and_then(|a|a.evidence.iter().find(|e|e.id==input.evidence_id)).ok_or("Reteste ausente.")?;
        if raw.sha256!=hash(&raw.content){return Err("Reteste alterado.".into());}
        let proof:Value=serde_json::from_str(&raw.content).map_err(db_error)?;
        let reviewed:crate::scope::ScopeRef=serde_json::from_value(proof["scopeRef"].clone()).map_err(|_|"scope_legacy_plan_requires_reprepare: realize novo reteste após adotar o escopo.")?;
        let proof_target=proof.get("targetRef").map(|value|serde_json::from_value::<crate::scope_targets::ScopeTargetRef>(value.clone()).map_err(db_error)).transpose()?;
        if proof_target!=p.target_ref{return Err("Reteste associado a outra entrada de escopo.".into());}
        let reference=crate::scope::with_selected_admission(store,&p.assessment_id,"local.text-remediation.v1",Some(&reviewed),&selected_refs(&p),"remediation.confirm",|reference|Ok(reference.clone()))?;
        if p.scope_ref.as_ref().is_some_and(|r|r!=&reference){return Err("scope_state_changed".into());}
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&selected_refs(&p))?;
        let mut _preflight=crate::local_scope::preflight(&store.snapshot()?,&reference,p.target_ref.as_ref())?;
        crate::scope::recheck_selected(store,&reference,"local.text-remediation.v1",&selected_refs(&p))?;
        let identity=_preflight.release_selected_file(p.target_ref.as_ref())?;
        let mut locked=LockedTarget::open(&p.target,&p.relative_path,false)?;
        verify_identity_handoff(identity.as_ref(),&locked.identity)?;
        if locked.identity!=p.identity || locked.path.to_string_lossy()!=p.file_path || hash(&locked.read()?)!=p.after_sha256 {return Err("O arquivo mudou depois do reteste; a correção não pode ser confirmada.".into());}
        p.status="confirming".into();p.message="Confirmação humana em registro; nenhuma escrita no alvo.".into();self.save(&p)?;
        let result=store.mutate(|s|{
            bound_scope(s,&p)?;
            crate::scope::check_selected_work(s,&p.assessment_id,"local.text-remediation.v1",Some(&reference),&selected_refs(&p))?;
            let a=s.assessments.iter().find(|a|a.id==p.assessment_id).ok_or("Avaliação não encontrada.")?;
            let retest=a.evidence.iter().find(|e|e.id==input.evidence_id).ok_or("Evidência de reteste não encontrada nesta avaliação.")?.clone();
            if retest.sha256!=hash(&retest.content) {return Err("Evidência de reteste alterada.".into());}
            let proof:Value=serde_json::from_str(&retest.content).map_err(db_error)?;
            if proof["schema"]!="openkoda.remediation.retest.v1" || proof["passed"]!=true || proof["remediationId"]!=p.id || proof["planDigest"]!=p.digest || proof["findingId"]!=p.finding_id || proof["observedSha256"]!=p.after_sha256 {return Err("Evidência não corresponde ao reteste desta correção.".into());}
            if raw_target(s,&p.assessment_id,&proof)?!=p.target||proof.get("targetRef").map(|value|serde_json::from_value::<crate::scope_targets::ScopeTargetRef>(value.clone()).map_err(db_error)).transpose()?!=p.target_ref{return Err("Vínculo do alvo de reteste inconsistente.".into());}
            let mut proof=json!({"schema":"openkoda.remediation.confirmation.v1","scopeRef":reference,"remediationId":p.id,"planDigest":p.digest,"findingId":p.finding_id,
                "assessmentId":p.assessment_id,"retestEvidenceId":retest.id,"retestSha256":retest.sha256,"observedSha256":p.after_sha256,
                "confirmedAt":model::now(),"author":"local-user","limits":p.limits});
            if let Some(target)=&p.target_ref{proof["targetRef"]=json!(target);proof["identityPreflightRefs"]=json!(p.identity_preflight_refs);}
            let content=proof.to_string();
            let confirmation=Evidence {id:model::id(),name:format!("remediation-confirmation-{}.json",p.id),sha256:hash(&content),content,created_at:model::now()};
            let refs=vec![ValidatedEvidence{id:retest.id.clone(),sha256:retest.sha256.clone()},ValidatedEvidence{id:confirmation.id.clone(),sha256:confirmation.sha256.clone()}];
            s.validations.push(ValidationRecord{id:model::id(),finding_id:p.finding_id.clone(),assessment_id:p.assessment_id.clone(),method:"human_review".into(),
                expected:"A condição revisada deve passar após a alteração exata.".into(),observed:rationale.into(),evidence:refs,author:"local-user".into(),created_at:model::now()});
            let a=s.assessments.iter_mut().find(|a|a.id==p.assessment_id).unwrap();a.evidence.push(confirmation.clone());a.updated_at=model::now();
            a.events.push(model::event("Correção confirmada por revisão humana do reteste delimitado. A conclusão vale para o arquivo/hash e condição registrados.","info"));
            let f=s.findings.iter_mut().find(|f|f.id==p.finding_id).unwrap();
            for id in [retest.id,confirmation.id] {if !f.evidence_ids.contains(&id){f.evidence_ids.push(id);}}
            f.status="fixed".into();validate_fixed_findings(s)
        });
        if let Err(error)=result {p.status="unknown".into();p.message="Não foi possível confirmar o registro humano; confira a trilha antes de nova ação.".into();self.save(&p)?;return Err(error);}
        p.status="confirmed".into();p.confirmed_at=Some(model::now());p.message="Correção confirmada para o arquivo, hash e condição de reteste registrados.".into();self.save(&p)?;Ok(p)
    }
}

fn reconciliation_content(p:&RemediationPlan,r:&ReconciliationReceipt)->String {
    let mut value=json!({"schema":"openkoda.remediation.reconciliation.v1","remediationId":p.id,"planDigest":p.digest,
        "projectId":p.project_id,"assessmentId":p.assessment_id,"findingId":p.finding_id,"relativePath":p.relative_path,
        "baseSha256":p.base_sha256,"afterSha256":p.after_sha256,"receipt":r,
        "targetWritten":false,"patchReplayed":false,"findingStatusChanged":false,"historicalEffectsKnown":false,
        "limits":p.limits});
    if let Some(target)=&p.target_ref{value["targetRef"]=json!(target);value["scopeRef"]=json!(p.scope_ref);value["identityPreflightRefs"]=json!(p.identity_preflight_refs);}
    value.to_string()
}

fn validate_reconciliation(p:&RemediationPlan)->Result<(),String> {
    let invalid=||"Recibo de reconciliação inconsistente; histórico preservado.".to_string();
    let valid_operation=|value:&str|matches!(value,"applying"|"retesting"|"confirming");
    if p.interrupted_operation.as_deref().is_some_and(|value|!valid_operation(value)) {return Err(invalid());}
    let Some(r)=&p.reconciliation else {return if p.status=="reconciled" {Err(invalid())} else {Ok(())};};
    core::valid_id(&r.id)?;core::valid_id(&r.evidence_id)?;
    if !matches!(p.status.as_str(),"unknown"|"reconciled") || !(r.previous_status=="unknown"||valid_operation(&r.previous_status))
        || r.interrupted_operation!=p.interrupted_operation || r.author!="local-user" || !r.acknowledge_unknown_effects
        || r.note.trim()!=r.note || r.note.is_empty() || r.note.len()>4000 || r.note.contains('\0')
        || chrono::DateTime::parse_from_rfc3339(&r.recorded_at).is_err()
        || chrono::DateTime::parse_from_rfc3339(&r.observation.observed_at).is_err()
        || r.finding_status.is_empty() || r.finding_status.len()>64 {return Err(invalid());}
    let o=&r.observation;
    if o.message.is_empty() || o.message.len()>2000 {return Err(invalid());}
    match o.state.as_str() {
        "before"|"after"|"other"=>{
            let sha=o.observed_sha256.as_ref().ok_or_else(invalid)?;
            if !valid_sha(&json!(sha)) || !o.observed_bytes.is_some_and(|bytes|bytes<=MAX_FILE as u64)
                || o.observed_identity.is_none() || o.identity_matches!=Some(o.observed_identity.as_ref()==Some(&p.identity))
                || (o.state=="before" && sha!=&p.base_sha256) || (o.state=="after" && sha!=&p.after_sha256)
                || (o.state=="other" && (sha==&p.base_sha256 || sha==&p.after_sha256)) {return Err(invalid());}
        },
        "missing"|"unavailable"=>{
            if o.observed_sha256.is_some() || o.observed_bytes.is_some() || o.observed_identity.is_some() || o.identity_matches.is_some() {return Err(invalid());}
        },
        _=>return Err(invalid()),
    }
    Ok(())
}

fn read_plan(raw:&str)->Result<RemediationPlan,String> {
    if raw.len()>4*1024*1024{return Err("Plano de correção excedeu o limite de leitura.".into());}
    let p:RemediationPlan=serde_json::from_str(raw).map_err(db_error)?;
    for id in [&p.id,&p.project_id,&p.assessment_id,&p.finding_id] {core::valid_id(id)?;}
    if p.target_ref.is_some()&&(p.scope_ref.is_none()||p.identity_preflight_refs.is_empty())||p.target_ref.is_none()&&!p.identity_preflight_refs.is_empty(){return Err("Vínculo de alvo/preflight da correção é inválido.".into());}
    rule_valid(&p.retest)?;
    if p.limits != retest_limits(&p.retest.kind) { return Err("Limites da condição de reteste divergentes.".into()); }
    if p.retest.kind == JSON_DEBUG_RULE && (json_debug_flag(&p.before) != Some(true) || json_debug_flag(&p.after) != Some(false)) {
        return Err("A configuração JSON diverge da mudança de depuração revisada.".into());
    }
    if p.before.len()>MAX_FILE || p.after.len()>MAX_FILE || hash(&p.before)!=p.base_sha256 || hash(&p.after)!=p.after_sha256 || plan_digest(&p)!=p.digest {
        return Err("Integridade do plano de correção falhou; nada será aplicado.".into());
    }
    if !["proposed","approved","applying","applied","retesting","retested","confirming","confirmed","revoked","invalidated","rolled_back","failed","unknown","reconciled"].contains(&p.status.as_str()) {return Err("Estado de correção inválido.".into());}
    validate_reconciliation(&p)?;
    Ok(p)
}

/// Whitelisted report projection. Never serialize a full plan into an export:
/// before/after, the predicate value and the byte backup can contain source data.
pub fn report_metadata(database:&Path,snapshot:&Snapshot,assessment_id:&str)->Result<Vec<Value>,String> {
    let assessment=snapshot.assessments.iter().find(|a|a.id==assessment_id).ok_or("Avaliação do relatório não encontrada.")?;
    let db=Connection::open_with_flags(database,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(db_error)?;
    let mut query=db.prepare("SELECT data FROM remediation_plans ORDER BY rowid LIMIT 101").map_err(db_error)?;
    let rows=query.query_map([],|row|row.get::<_,String>(0)).map_err(db_error)?;
    let mut result=Vec::new();let mut count=0;
    for raw in rows {
        count+=1;if count>MAX_PLANS {return Err("Catálogo de correções excede o limite verificado.".into());}
        let p=read_plan(&raw.map_err(db_error)?)?;
        if p.assessment_id!=assessment_id {continue;}
        if p.project_id!=assessment.project_id || (p.target_ref.is_none()&&p.target!=assessment.target) || !snapshot.findings.iter().any(|f|f.id==p.finding_id&&f.assessment_id==assessment_id) {
            return Err("Correção do relatório tem vínculo de projeto/achado inconsistente.".into());
        }
        for reference in [&p.scope_ref,&p.retest_scope_ref].into_iter().flatten(){crate::scope::validate_historical_ref(snapshot,&p.assessment_id,reference)?;}
        if let Some(target)=&p.target_ref{let scope=p.scope_ref.as_ref().ok_or("Correção combinada sem revisão.")?;if crate::scope_targets::resolve_historical_target(snapshot,scope,Some(target))?.target!=p.target{return Err("Alvo da correção diverge da origem histórica.".into());}crate::local_scope::validate_refs(snapshot,scope,Some(target),&p.identity_preflight_refs)?;}
        let mut projected=json!({"scopeRef":p.scope_ref,"retestScopeRef":p.retest_scope_ref,"id":p.id,"projectId":p.project_id,"assessmentId":p.assessment_id,"findingId":p.finding_id,"relativePath":p.relative_path,
            "baseSha256":p.base_sha256,"afterSha256":p.after_sha256,"planDigest":p.digest,"createdAt":p.created_at,"expiresAt":p.expires_at,
            "status":p.status,"approvedAt":p.approved_at,"appliedAt":p.applied_at,"retestedAt":p.retested_at,"confirmedAt":p.confirmed_at,
            "evidenceId":p.evidence_id,"passed":p.passed,"message":p.message,"limits":p.limits,
            "interruptedOperation":p.interrupted_operation,
            "reconciliation":p.reconciliation.as_ref().map(|r|json!({"id":r.id,"evidenceId":r.evidence_id,"previousStatus":r.previous_status,
                "interruptedOperation":r.interrupted_operation,"recordedAt":r.recorded_at,"author":r.author,
                "acknowledgeUnknownEffects":r.acknowledge_unknown_effects,"findingStatusAtObservation":r.finding_status,"observation":r.observation,
                "noteSha256":hash(&r.note),"targetWritten":false,"patchReplayed":false,"findingStatusChanged":false,"historicalEffectsKnown":false})),
            "retest":{"kind":p.retest.kind,"ruleSha256":hash(&serde_json::to_string(&p.retest).map_err(db_error)?)},
            "sourceContentIncluded":false,"backupBytesIncluded":false});if let Some(target)=&p.target_ref{projected["targetRef"]=json!(target);projected["identityPreflightRefs"]=json!(p.identity_preflight_refs);}result.push(projected);
    }
    Ok(result)
}

/// Pure Store invariant hook. Call from core::validate_snapshot so a persisted
/// `fixed` status always requires both typed retest proof and a human record.
pub fn validate_fixed_findings(s:&Snapshot)->Result<(),String> {
    for f in s.findings.iter().filter(|f|f.status=="fixed") {
        let a=s.assessments.iter().find(|a|a.id==f.assessment_id).ok_or("Achado corrigido sem avaliação.")?;
        let supported=s.validations.iter().filter(|v|v.finding_id==f.id && v.assessment_id==a.id && v.method=="human_review" && v.author=="local-user").any(|v| {
            v.evidence.iter().filter_map(|r|a.evidence.iter().find(|e|e.id==r.id&&e.sha256==r.sha256&&hash(&e.content)==e.sha256&&f.evidence_ids.contains(&e.id)))
                .filter_map(|e|serde_json::from_str::<Value>(&e.content).ok()).any(|c|{
                    if c["schema"]!="openkoda.remediation.confirmation.v1" || c["findingId"]!=f.id || c["assessmentId"]!=a.id || c["author"]!="local-user" {return false;}
                    let Some(retest)=a.evidence.iter().find(|e|Some(e.id.as_str())==c["retestEvidenceId"].as_str() && Some(e.sha256.as_str())==c["retestSha256"].as_str() && hash(&e.content)==e.sha256) else{return false;};
                    if !f.evidence_ids.contains(&retest.id) || !v.evidence.iter().any(|r|r.id==retest.id && r.sha256==retest.sha256) {return false;}
                    let Ok(r)=serde_json::from_str::<Value>(&retest.content) else{return false;};
                    let scoped=match(c.get("scopeRef"),r.get("scopeRef")){(None,None)=>true,(Some(c),Some(r))=>c==r&&serde_json::from_value::<crate::scope::ScopeRef>(r.clone()).is_ok_and(|reference|crate::scope::validate_historical_ref(s,&a.id,&reference).is_ok()),_=>false};
                    scoped&&c.get("targetRef")==r.get("targetRef")&&c.get("identityPreflightRefs")==r.get("identityPreflightRefs") && r["schema"]=="openkoda.remediation.retest.v1" && r["passed"]==true && r["findingId"]==f.id && r["assessmentId"]==a.id && raw_target(s,&a.id,&r).is_ok_and(|target|r["target"]==target)
                        && r["projectId"]==a.project_id && r["remediationId"].as_str().is_some_and(|id|core::valid_id(id).is_ok())
                        && r["remediationId"]==c["remediationId"] && valid_sha(&r["planDigest"]) && valid_sha(&r["baseSha256"]) && valid_sha(&r["appliedSha256"])
                        && valid_sha(&r["ruleSha256"]) && supported_retest_proof(&r)
                        && r["baseSha256"]!=r["appliedSha256"] && r["planDigest"]==c["planDigest"]
                        && r["observedSha256"]==r["appliedSha256"] && r["observedSha256"]==c["observedSha256"]
                })
        });
        if !supported && !crate::http_validation::has_valid_confirmation(s,&f.id)? {return Err("Achado corrigido exige evidência de reteste delimitado e confirmação humana vinculados e íntegros.".into());}
    }
    Ok(())
}

/// Validate the target carried by a raw retest without consulting today's
/// authorization state or the producer database (required by Store invariants).
pub(crate) fn raw_target(s:&Snapshot,assessment_id:&str,proof:&Value)->Result<String,String>{
    let resolved=raw_target_binding(s,assessment_id,proof)?;
    if let Some(value)=proof.get("targetRef"){
        let target:crate::scope_targets::ScopeTargetRef=serde_json::from_value(value.clone()).map_err(db_error)?;let finding=proof["findingId"].as_str().ok_or("Prova sem achado de origem.")?;
        if !s.findings.iter().any(|row|row.id==finding&&row.assessment_id==assessment_id)||!crate::evidence_lineage::finding_source_refs(s,finding)?.contains(&target){return Err("Alvo da prova não corresponde às fontes originais do achado.".into());}
    }
    Ok(resolved)
}
fn raw_target_binding(s:&Snapshot,assessment_id:&str,proof:&Value)->Result<String,String>{
    let a=s.assessments.iter().find(|a|a.id==assessment_id).ok_or("Avaliação da prova ausente.")?;
    let target=proof.get("targetRef").map(|value|serde_json::from_value::<crate::scope_targets::ScopeTargetRef>(value.clone()).map_err(db_error)).transpose()?;
    let Some(value)=proof.get("scopeRef") else{
        if target.is_some()||proof.get("identityPreflightRefs").is_some()||s.scope_revisions.iter().any(|revision|revision.assessment_id==a.id&&revision.body.target_set.is_some()){return Err("Prova legada não pode representar alvo combinado.".into());}
        return Ok(a.target.clone());
    };
    let scope:crate::scope::ScopeRef=serde_json::from_value(value.clone()).map_err(db_error)?;
    if scope.assessment_id!=a.id{return Err("Prova vinculada a outra avaliação.".into());}
    let selected=crate::scope_targets::resolve_historical_target(s,&scope,target.as_ref())?;
    if !matches!(selected.kind.as_str(),"code"|"files"){return Err("Prova de correção exige alvo local.".into());}
    if let Some(target)=&target{
        let refs:Vec<crate::scope_targets::ScopeTargetRef>=serde_json::from_value(proof["identityPreflightRefs"].clone()).map_err(db_error)?;
        crate::local_scope::validate_refs(s,&scope,Some(target),&refs)?;
        let revision=s.scope_revisions.iter().find(|revision|revision.id==scope.revision_id).ok_or("Revisão da prova ausente.")?;
        if !revision.body.target_set.as_ref().is_some_and(|set|set.entries.iter().any(|entry|entry.entry_id==target.entry_id&&entry.methods.iter().any(|method|method=="local.text-remediation.v1"))){return Err("Prova sem método de correção na entrada.".into());}
        let relative=proof["relativePath"].as_str().ok_or("Prova sem caminho relativo.")?;
        check_relative_policy(&Scope{project_id:a.project_id.clone(),assessment_id:a.id.clone(),finding_id:String::new(),target:selected.target.clone(),local_exclusions:selected.local_exclusions,combined:true},relative)?;
    }else if proof.get("identityPreflightRefs").is_some(){return Err("Referências extras em prova simples.".into());}
    Ok(selected.target)
}

/// Pure structural leaf for Store invariants. It never follows finding origins
/// or opens a producer DB. Dispatch/report consumers must still use the journal
/// verifier below; structural validity alone does not prove a producer commit.
pub(crate) fn structural_evidence_target_refs(s:&Snapshot,assessment_id:&str,evidence:&Evidence)->Result<Option<Vec<crate::scope_targets::ScopeTargetRef>>,String>{
    let Ok(proof)=serde_json::from_str::<Value>(&evidence.content)else{return Ok(None)};
    let schema=proof["schema"].as_str().unwrap_or("");if !matches!(schema,"openkoda.remediation.retest.v1"|"openkoda.remediation.confirmation.v1"|"openkoda.remediation.reconciliation.v1"){return Ok(None);}
    let invalid=||"Estrutura de proveniência da correção é inválida.".to_string();
    let assessment=s.assessments.iter().find(|a|a.id==assessment_id).ok_or_else(invalid)?;
    if evidence.sha256!=hash(&evidence.content)||!assessment.evidence.iter().any(|item|item.id==evidence.id&&item.sha256==evidence.sha256&&item.content==evidence.content)||proof["assessmentId"]!=assessment_id||!s.findings.iter().any(|f|f.assessment_id==assessment_id&&proof["findingId"]==f.id)||!valid_sha(&proof["planDigest"])|| !(proof["limits"]==json!(LIMITS)||proof["limits"]==json!(retest_limits(JSON_DEBUG_RULE))){return Err(invalid());}
    core::valid_id(proof["remediationId"].as_str().ok_or_else(invalid)?)?;
    let target=proof.get("targetRef").map(|value|serde_json::from_value::<crate::scope_targets::ScopeTargetRef>(value.clone()).map_err(db_error)).transpose()?;
    match schema{
        "openkoda.remediation.retest.v1"=>{
            let resolved=raw_target_binding(s,assessment_id,&proof)?;
            if proof["target"]!=resolved||proof["projectId"]!=assessment.project_id||!["baseSha256","appliedSha256","observedSha256","ruleSha256"].iter().all(|name|valid_sha(&proof[*name]))||!proof["passed"].is_boolean()||proof["passed"]==true&&proof["observedSha256"]!=proof["appliedSha256"]||proof["limits"]!=json!(retest_limits(proof["ruleKind"].as_str().unwrap_or_default()))||!supported_retest_proof(&proof)||proof["checkedAt"].as_str().is_none_or(|at|chrono::DateTime::parse_from_rfc3339(at).is_err()){return Err(invalid());}
        },
        "openkoda.remediation.confirmation.v1"=>{
            let retest=assessment.evidence.iter().find(|e|Some(e.id.as_str())==proof["retestEvidenceId"].as_str()&&Some(e.sha256.as_str())==proof["retestSha256"].as_str()).ok_or_else(invalid)?;let raw:Value=serde_json::from_str(&retest.content).map_err(db_error)?;
            if raw["schema"]!="openkoda.remediation.retest.v1"||raw["passed"]!=true||["assessmentId","findingId","remediationId","planDigest","observedSha256","scopeRef","targetRef","identityPreflightRefs","limits"].iter().any(|field|proof.get(*field)!=raw.get(*field))||proof["author"]!="local-user"||proof["confirmedAt"].as_str().is_none_or(|at|chrono::DateTime::parse_from_rfc3339(at).is_err()){return Err(invalid());}
            if structural_evidence_target_refs(s,assessment_id,retest)?!=Some(target.iter().cloned().collect()){return Err(invalid());}
            if !s.validations.iter().any(|v|v.assessment_id==assessment_id&&proof["findingId"]==v.finding_id&&v.method=="human_review"&&v.author=="local-user"&&v.evidence.iter().any(|e|e.id==evidence.id&&e.sha256==evidence.sha256)&&v.evidence.iter().any(|e|e.id==retest.id&&e.sha256==retest.sha256)){return Err(invalid());}
        },
        _=>{
            raw_target_binding(s,assessment_id,&proof)?;let receipt:ReconciliationReceipt=serde_json::from_value(proof["receipt"].clone()).map_err(db_error)?;core::valid_id(&receipt.id)?;
            if proof["projectId"]!=assessment.project_id||!valid_sha(&proof["baseSha256"])||!valid_sha(&proof["afterSha256"])||receipt.evidence_id!=evidence.id||receipt.author!="local-user"||!receipt.acknowledge_unknown_effects||receipt.note.trim().is_empty()||receipt.note.len()>4000||chrono::DateTime::parse_from_rfc3339(&receipt.recorded_at).is_err()||chrono::DateTime::parse_from_rfc3339(&receipt.observation.observed_at).is_err()||["targetWritten","patchReplayed","findingStatusChanged","historicalEffectsKnown"].iter().any(|field|proof[*field]!=false){return Err(invalid());}
            let observed=&receipt.observation;match observed.state.as_str(){"before"|"after"|"other"=>{if observed.observed_sha256.as_ref().is_none_or(|sha|!valid_sha(&json!(sha)))||!observed.observed_bytes.is_some_and(|bytes|bytes<=MAX_FILE as u64)||observed.observed_identity.is_none()||observed.identity_matches.is_none(){return Err(invalid());}},"missing"|"unavailable"=>{if observed.observed_sha256.is_some()||observed.observed_bytes.is_some()||observed.observed_identity.is_some()||observed.identity_matches.is_some(){return Err(invalid());}},_=>return Err(invalid())}
        }
    }
    Ok(Some(target.into_iter().collect()))
}

/// Verify producer lineage for remote context/report use. Only exact host
/// schemas are recognized; imported text is not decoded recursively.
pub(crate) fn evidence_target_refs(database:&Path,s:&Snapshot,assessment_id:&str,evidence:&Evidence)->Result<Option<Vec<crate::scope_targets::ScopeTargetRef>>,String>{
    let Some(structural_refs)=structural_evidence_target_refs(s,assessment_id,evidence)?else{return Ok(None)};
    let Ok(proof)=serde_json::from_str::<Value>(&evidence.content)else{return Ok(None)};
    let schema=proof["schema"].as_str().unwrap_or("");
    if !matches!(schema,"openkoda.remediation.retest.v1"|"openkoda.remediation.confirmation.v1"|"openkoda.remediation.reconciliation.v1"){return Ok(None);}
    let invalid=||"Proveniência da correção não pôde ser confirmada no registro produtor.".to_string();
    let assessment=s.assessments.iter().find(|a|a.id==assessment_id).ok_or_else(invalid)?;
    if evidence.sha256!=hash(&evidence.content)||!assessment.evidence.iter().any(|item|item.id==evidence.id&&item.content==evidence.content&&item.sha256==evidence.sha256){return Err(invalid());}
    let id=proof["remediationId"].as_str().ok_or_else(invalid)?;core::valid_id(id)?;
    let db=Connection::open_with_flags(database,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(db_error)?;
    let raw:String=db.query_row("SELECT data FROM remediation_plans WHERE id=?1",[id],|row|row.get(0)).map_err(db_error)?;
    let p=read_plan(&raw)?;
    if p.id!=id||p.assessment_id!=assessment_id||p.project_id!=assessment.project_id||proof["planDigest"]!=p.digest||proof["findingId"]!=p.finding_id||proof["assessmentId"]!=p.assessment_id{return Err(invalid());}
    let target=proof.get("targetRef").map(|value|serde_json::from_value::<crate::scope_targets::ScopeTargetRef>(value.clone()).map_err(db_error)).transpose()?;
    if target!=p.target_ref{return Err(invalid());}
    if p.target_ref.is_some()&&(proof.get("scopeRef")!=Some(&json!(p.scope_ref))||proof.get("identityPreflightRefs")!=Some(&json!(p.identity_preflight_refs))){return Err(invalid());}
    match schema{
        "openkoda.remediation.retest.v1"=>{
            if raw_target_binding(s,assessment_id,&proof)?!=p.target||proof["target"]!=p.target||proof["relativePath"]!=p.relative_path||proof["baseSha256"]!=p.base_sha256||proof["appliedSha256"]!=p.after_sha256||proof["ruleKind"]!=p.retest.kind||proof["ruleSha256"]!=hash(&serde_json::to_string(&p.retest).map_err(db_error)?)||!valid_sha(&proof["observedSha256"])||!proof["passed"].is_boolean()||proof["passed"]==true&&proof["observedSha256"]!=p.after_sha256||proof["limits"]!=json!(p.limits){return Err(invalid());}
        },
        "openkoda.remediation.confirmation.v1"=>{
            let retest=assessment.evidence.iter().find(|item|Some(item.id.as_str())==proof["retestEvidenceId"].as_str()&&Some(item.sha256.as_str())==proof["retestSha256"].as_str()).ok_or_else(invalid)?;
            let retest_proof:Value=serde_json::from_str(&retest.content).map_err(db_error)?;
            if retest_proof["schema"]!="openkoda.remediation.retest.v1"{return Err(invalid());}
            if evidence_target_refs(database,s,assessment_id,retest)?.as_ref()!=Some(&selected_refs(&p))||proof["observedSha256"]!=p.after_sha256||proof["author"]!="local-user"||proof["limits"]!=json!(p.limits){return Err(invalid());}
            if retest_proof["schema"]!="openkoda.remediation.retest.v1"||retest_proof["passed"]!=true||retest_proof["remediationId"]!=p.id||proof.get("scopeRef")!=retest_proof.get("scopeRef")||proof.get("identityPreflightRefs")!=retest_proof.get("identityPreflightRefs")||!s.validations.iter().any(|record|record.finding_id==p.finding_id&&record.assessment_id==assessment_id&&record.author=="local-user"&&record.method=="human_review"&&record.evidence.iter().any(|item|item.id==evidence.id&&item.sha256==evidence.sha256)&&record.evidence.iter().any(|item|item.id==retest.id&&item.sha256==retest.sha256)){return Err(invalid());}
        },
        _=>{let receipt=p.reconciliation.as_ref().ok_or_else(invalid)?;if receipt.evidence_id!=evidence.id||reconciliation_content(&p,receipt)!=evidence.content{return Err(invalid());}}
    }
    if let Some(target)=&p.target_ref{let scope=p.scope_ref.as_ref().ok_or_else(invalid)?;if crate::scope_targets::resolve_historical_target(s,scope,Some(target))?.target!=p.target{return Err(invalid());}}
    if structural_refs!=selected_refs(&p){return Err(invalid());}Ok(Some(structural_refs))
}

struct LockedTarget { file:File, _parents:Vec<File>, path:PathBuf, identity:FileIdentity }
enum TargetLease { Present(LockedTarget), Missing { _parents:Vec<File> } }

fn observe_original_target(p:&RemediationPlan,expected_identity:Option<&crate::source_scan::FileIdentity>,content_allowed:bool)->(ReconciliationObservation,Option<TargetLease>) {
    let mut observation=ReconciliationObservation {state:"unavailable".into(),observed_at:model::now(),observed_sha256:None,
        observed_bytes:None,observed_identity:None,identity_matches:None,message:String::new()};
    match LockedTarget::open_recorded(p) {
        Err(error)=>{observation.message=format!("Inspeção indisponível: {error} Não foi concluído que o arquivo está ausente nem que uma escrita foi desfeita.");(observation,None)},
        Ok(mut lease)=>{
            match &mut lease {
                TargetLease::Missing{..}=>{
                    observation.state="missing".into();
                    observation.message="O arquivo não foi encontrado no caminho original durante a inspeção, com ancestrais protegidos contra substituição. Isso não demonstra quando ou por quem foi removido, nem desfaz efeitos anteriores.".into();
                },
                TargetLease::Present(_) if !content_allowed=>{observation.message="Preflight de identidade indisponível. Somente a presença no caminho original foi verificada sob ancestrais protegidos; nenhum conteúdo foi lido/hashado nem efeito histórico concluído.".into();},
                TargetLease::Present(locked)=>match verify_identity_handoff(expected_identity,&locked.identity).and_then(|()|locked.read_bytes()) {
                    Err(error)=>observation.message=format!("Inspeção indisponível: {error} O arquivo existe, mas seus bytes não foram classificados."),
                    Ok(bytes)=>{
                        let actual=format!("{:x}",Sha256::digest(&bytes));
                        observation.state=if actual==p.base_sha256 {"before"} else if actual==p.after_sha256 {"after"} else {"other"}.into();
                        observation.observed_sha256=Some(actual);observation.observed_bytes=Some(bytes.len() as u64);
                        observation.identity_matches=Some(locked.identity==p.identity);observation.observed_identity=Some(locked.identity.clone());
                        observation.message="Hash dos bytes atuais observado sob handle exclusivo somente de leitura. A identidade original é conferida separadamente; igualdade de bytes não prova o histórico de escrita nem que o achado foi corrigido.".into();
                    },
                },
            }
            (observation,Some(lease))
        },
    }
}
impl LockedTarget {
    #[cfg(not(windows))]
    fn open(_: &str,_:&str,_:bool)->Result<Self,String> {Err("Bloqueio de escrita de correções disponível somente no Windows.".into())}
    #[cfg(not(windows))]
    fn open_recorded(_: &RemediationPlan)->Result<TargetLease,String> {Err("Inspeção protegida de correções disponível somente no Windows.".into())}
    #[cfg(windows)]
    fn open(target:&str,relative:&str,write:bool)->Result<Self,String> {
        let root=Path::new(target);windows_path(root)?;
        let relative=relative_path(relative)?;
        let root_meta=fs::symlink_metadata(root).map_err(|_|"Alvo local indisponível.".to_string())?;
        let candidate=if root_meta.is_dir(){root.join(&relative)}else if root_meta.is_file() && relative.components().count()==1 && root.file_name()==relative.file_name(){root.to_path_buf()}
            else{return Err("Selecione um arquivo regular dentro do alvo; para alvo arquivo use somente seu nome.".into());};
        match Self::open_candidate(&candidate,write,false)? {
            TargetLease::Present(locked)=>Ok(locked),
            TargetLease::Missing{..}=>Err("Arquivo não disponível para correção.".into()),
        }
    }
    #[cfg(windows)]
    fn open_recorded(p:&RemediationPlan)->Result<TargetLease,String> {
        let root=Path::new(&p.target);windows_path(root)?;
        let relative=relative_path(&p.relative_path)?;
        let candidate=PathBuf::from(&p.file_path);windows_path(&candidate)?;
        let file_target=relative.components().count()==1 && root.file_name()==relative.file_name() && normalized(&candidate)==normalized(root);
        if !file_target && normalized(&candidate)!=normalized(&root.join(relative)) {
            return Err("Caminho registrado diverge do alvo e caminho relativo originais; nenhum outro destino foi aberto.".into());
        }
        Self::open_candidate(&candidate,false,true)
    }
    #[cfg(windows)]
    fn open_candidate(candidate:&Path,write:bool,allow_missing:bool)->Result<TargetLease,String> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS,FILE_FLAG_OPEN_REPARSE_POINT,FILE_ATTRIBUTE_DIRECTORY,FILE_ATTRIBUTE_REPARSE_POINT};
        windows_path(candidate)?;
        let mut parents=Vec::new();let mut path=PathBuf::new();
        for component in candidate.parent().ok_or("Arquivo sem diretório pai.")?.components() {
            path.push(component.as_os_str());
            if matches!(component,Component::Prefix(_)){continue;}
            let directory=fs::OpenOptions::new().read(true).share_mode(3).custom_flags(FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT).open(&path)
                .map_err(|_|"Não foi possível proteger um diretório ancestral contra substituição.".to_string())?;
            let info=handle_info(&directory)?;
            if info.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT!=0 || info.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY==0 {return Err("Link/reparse point em diretório não é aceito para correção.".into());}
            parents.push(directory);
        }
        let file=match fs::OpenOptions::new().read(true).write(write).share_mode(0).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(candidate) {
            Ok(file)=>file,
            // FILE_NOT_FOUND only, after every ancestor was opened safely.
            // Access/sharing errors and missing ancestors are not absence proof.
            Err(error) if allow_missing && error.raw_os_error()==Some(2)=>return Ok(TargetLease::Missing{_parents:parents}),
            Err(_)=>return Err("Arquivo em uso, sem acesso ou não disponível para bloqueio exclusivo. Nenhuma escrita começou.".into()),
        };
        let info=handle_info(&file)?;
        if info.dwFileAttributes&(FILE_ATTRIBUTE_REPARSE_POINT|FILE_ATTRIBUTE_DIRECTORY)!=0 || info.nNumberOfLinks!=1 {return Err("Correção recusa symlink/reparse point, diretório e arquivo com hardlinks.".into());}
        let effective=handle_path(&file)?;
        if normalized(&effective)!=normalized(candidate) {return Err("Identidade ou destino efetivo do arquivo diverge do caminho selecionado.".into());}
        let identity=FileIdentity {volume:info.dwVolumeSerialNumber,index:((info.nFileIndexHigh as u64)<<32)|info.nFileIndexLow as u64};
        Ok(TargetLease::Present(Self {file,_parents:parents,path:effective,identity}))
    }
    fn read_bytes(&mut self)->Result<Vec<u8>,String> {
        self.file.seek(SeekFrom::Start(0)).map_err(db_error)?;
        let mut bytes=Vec::new();(&mut self.file).take(MAX_FILE as u64+1).read_to_end(&mut bytes).map_err(db_error)?;
        if bytes.len()>MAX_FILE {return Err("Arquivo excedeu o limite de leitura de 256 KiB.".into());}
        Ok(bytes)
    }
    fn read(&mut self)->Result<String,String> {
        let bytes=self.read_bytes()?;
        if bytes.contains(&0) {return Err("Arquivo exige UTF-8 sem NUL e até 256 KiB.".into());}
        String::from_utf8(bytes).map_err(|_|"Arquivo não é UTF-8 válido; nenhuma conversão automática será aplicada.".into())
    }
    fn write(&mut self,content:&str)->Result<(),String> {
        #[cfg(windows)] {
            let info=handle_info(&self.file)?;
            if info.nNumberOfLinks!=1 || info.dwVolumeSerialNumber!=self.identity.volume || (((info.nFileIndexHigh as u64)<<32)|info.nFileIndexLow as u64)!=self.identity.index {return Err("Identidade/hardlinks mudou sob a correção; escrita recusada.".into());}
        }
        self.file.seek(SeekFrom::Start(0)).map_err(db_error)?;
        self.file.write_all(content.as_bytes()).map_err(db_error)?;
        self.file.set_len(content.len() as u64).map_err(db_error)?;
        self.file.sync_all().map_err(db_error)
    }
}
fn relative_path(value:&str)->Result<PathBuf,String> {
    if value.is_empty() || value.len()>4096 || value.chars().any(|c|c.is_control()||c==':') {return Err("Caminho relativo inválido; ADS e prefixos não são permitidos.".into());}
    let path=PathBuf::from(value);
    if path.components().any(|c|!matches!(c,Component::Normal(_))) {return Err("Caminho deve conter somente nomes relativos, sem raiz, '.' ou '..'.".into());}
    for component in value.split(['\\','/']) {safe_component(component)?;}
    Ok(path)
}
fn safe_component(component:&str)->Result<(),String> {
    let stem=component.split('.').next().unwrap_or("").to_ascii_uppercase();
    if component.is_empty() || component.ends_with(['.',' ']) || component.chars().any(|c|c.is_control()||"<>:\"|?*".contains(c))
        || ["CON","PRN","AUX","NUL","CLOCK$"].contains(&stem.as_str()) || (stem.len()==4&&(stem.starts_with("COM")||stem.starts_with("LPT"))&&stem.as_bytes()[3].is_ascii_digit()) {
        return Err("Nome de arquivo ambíguo ou reservado pelo Windows.".into());
    }Ok(())
}
#[cfg(windows)]
fn windows_path(path:&Path)->Result<(),String> {
    use std::path::Prefix;
    let mut components=path.components();
    if !matches!(components.next(),Some(Component::Prefix(prefix)) if matches!(prefix.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_))) || !matches!(components.next(),Some(Component::RootDir)) {
        return Err("Correções aceitam somente caminho absoluto em disco local Windows, sem UNC/device.".into());
    }
    for part in components {match part {Component::Normal(s)=>safe_component(s.to_str().ok_or("Caminho não é texto Unicode válido.")?)?,_=>return Err("Caminho contém segmento não permitido.".into())}}
    Ok(())
}
#[cfg(windows)]
fn normalized(path:&Path)->String {path.to_string_lossy().trim_start_matches("\\\\?\\").replace('/',"\\").to_lowercase()}
#[cfg(windows)]
fn handle_info(file:&File)->Result<windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION,String> {
    use std::os::windows::io::AsRawHandle;
    let mut info=unsafe{std::mem::zeroed()};
    if unsafe{windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(file.as_raw_handle(),&mut info)}==0 {return Err("Não foi possível verificar identidade e hardlinks do arquivo.".into());}Ok(info)
}
#[cfg(windows)]
fn handle_path(file:&File)->Result<PathBuf,String> {
    use std::os::windows::{io::AsRawHandle,ffi::OsStringExt};
    let mut buffer=vec![0u16;32768];
    let len=unsafe{windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW(file.as_raw_handle(),buffer.as_mut_ptr(),buffer.len() as u32,0)};
    if len==0 || len as usize>=buffer.len(){return Err("Não foi possível verificar o caminho efetivo do arquivo.".into());}
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&buffer[..len as usize])))
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    use crate::model::Finding;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn metadata_only_handoff_refuses_replacement_before_source_read(){
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("selected.txt");fs::write(&path,b"ORIGINAL").unwrap();let mut lease=crate::source_scan::lock_metadata_target(&path).unwrap();let identity=lease.release_file_for_exclusive().unwrap();fs::rename(&path,dir.path().join("preserved.txt")).unwrap();fs::write(&path,b"REPLACEMENT-SENTINEL").unwrap();let locked=LockedTarget::open(path.to_str().unwrap(),"selected.txt",false).unwrap();assert!(verify_identity_handoff(Some(&identity),&locked.identity).is_err());drop(locked);assert_eq!(fs::read(path).unwrap(),b"REPLACEMENT-SENTINEL");assert_eq!(fs::read(dir.path().join("preserved.txt")).unwrap(),b"ORIGINAL");
    }
    #[test]
    fn combined_reconciliation_of_unavailable_file_root_records_no_content_or_reinterpretation(){
        for condition in ["missing","busy","folder"]{
            let dir=tempfile::tempdir().unwrap();let file=dir.path().join("settings.txt");fs::write(&file,b"secure=false\n").unwrap();let f=crate::executor::combined_fixture::Fixture::new(&[("files",file.to_str().unwrap(),&["local.text-remediation.v1"])],&[],json!([]));let service=RemediationService::new(f.store.root.clone()).unwrap();
            let imported=f.store.handle(core::Request{action:"evidence.import".into(),payload:json!({"assessmentId":f.aid,"name":"Synthetic selected file","sourceDescription":"Human file fixture","content":"secure=false","acknowledgeUnverifiedSource":true,"contextTargetRefs":[f.refs[0]]})}).unwrap();let finding=model::id();
            f.store.mutate(|s|{let e=s.assessments[0].evidence.iter().find(|e|e.id==imported["id"]).unwrap().clone();s.findings.push(Finding{id:finding.clone(),assessment_id:f.aid.clone(),title:"Synthetic file condition".into(),severity:"low".into(),confidence:"Human fixture".into(),status:"validated".into(),description:"Selected file only".into(),recommendation:String::new(),evidence_ids:vec![e.id.clone()],created_at:model::now()});s.validations.push(ValidationRecord{id:model::id(),finding_id:finding.clone(),assessment_id:f.aid.clone(),method:"human_review".into(),expected:"secure=false".into(),observed:"Synthetic observed condition".into(),evidence:vec![ValidatedEvidence{id:e.id,sha256:e.sha256}],author:"local-user".into(),created_at:model::now()});Ok(())}).unwrap();
            let plan=service.handle("remediation.prepare",json!({"findingId":finding,"relativePath":"settings.txt","after":"secure=true\n","retest":{"kind":"contains","value":"secure=true"},"scopeRef":crate::scope::current_ref(&f.store.snapshot().unwrap(),&f.aid).unwrap().unwrap(),"targetRef":f.refs[0]}),&f.store).unwrap();let mut interrupted=service.load(plan["id"].as_str().unwrap()).unwrap();interrupted.status="unknown".into();service.save(&interrupted).unwrap();let mut busy=None;
            match condition{"missing"=>fs::remove_file(&file).unwrap(),"busy"=>busy=Some(fs::OpenOptions::new().read(true).share_mode(0).open(&file).unwrap()),_=>{fs::remove_file(&file).unwrap();fs::create_dir(&file).unwrap();fs::write(file.join("settings.txt"),b"UNAUTHORIZED-DESCENDANT").unwrap();}}
            let reconciled=service.handle("remediation.reconcile",reconcile_payload(&plan),&f.store).unwrap();let observation=&reconciled["reconciliation"]["observation"];assert_eq!(observation["state"],if condition=="missing"{"missing"}else{"unavailable"});assert!(observation["observedSha256"].is_null());assert!(observation["observedBytes"].is_null());assert!(observation["observedIdentity"].is_null());assert_eq!(f.store.snapshot().unwrap().findings[0].status,"validated");assert!(!service.has_unresolved().unwrap());drop(busy);if condition=="busy"{assert_eq!(fs::read(&file).unwrap(),b"secure=false\n");}if condition=="folder"{assert_eq!(fs::read(file.join("settings.txt")).unwrap(),b"UNAUTHORIZED-DESCENDANT");}if condition=="missing"{assert!(!file.exists());}
        }
    }

    #[test]
    fn combined_remediation_requires_finding_origin_exclusions_and_producer_lineage(){
        let dir=tempfile::tempdir().unwrap();let first=dir.path().join("first");let second=dir.path().join("second");fs::create_dir(&first).unwrap();fs::create_dir(&second).unwrap();fs::write(first.join("settings.txt"),b"PRIMARY-SENTINEL").unwrap();fs::write(second.join("settings.txt"),b"secure=false\n").unwrap();fs::write(second.join("secret.txt"),b"EXCLUDED-SENTINEL").unwrap();let _locked=fs::OpenOptions::new().read(true).share_mode(0).open(second.join("secret.txt")).unwrap();
        let methods=["local.text-remediation.v1"];let f=crate::executor::combined_fixture::Fixture::new(&[("code",first.to_str().unwrap(),&methods),("files",second.to_str().unwrap(),&methods)],&[(1,"secret.txt","exact")],json!([]));let service=RemediationService::new(f.store.root.clone()).unwrap();
        let imported=f.store.handle(core::Request{action:"evidence.import".into(),payload:json!({"assessmentId":f.aid,"name":"Synthetic local condition","sourceDescription":"Human selected second entry","content":"The selected settings file contains secure=false","acknowledgeUnverifiedSource":true,"contextTargetRefs":[f.refs[1]]})}).unwrap();let finding=model::id();let evidence_id=imported["id"].as_str().unwrap().to_owned();
        f.store.mutate(|s|{let evidence=s.assessments[0].evidence.iter().find(|e|e.id==evidence_id).unwrap().clone();s.findings.push(Finding{id:finding.clone(),assessment_id:f.aid.clone(),title:"Synthetic second entry condition".into(),severity:"low".into(),confidence:"Human reviewed synthetic file".into(),status:"validated".into(),description:"Only the selected fixture".into(),recommendation:String::new(),evidence_ids:vec![evidence.id.clone()],created_at:model::now()});s.validations.push(ValidationRecord{id:model::id(),finding_id:finding.clone(),assessment_id:f.aid.clone(),method:"human_review".into(),expected:"secure=false".into(),observed:"Synthetic condition".into(),evidence:vec![ValidatedEvidence{id:evidence.id,sha256:evidence.sha256}],author:"local-user".into(),created_at:model::now()});Ok(())}).unwrap();
        let mut input=json!({"findingId":finding,"relativePath":"settings.txt","after":"secure=true\n","retest":{"kind":"contains","value":"secure=true"}});assert!(service.handle("remediation.prepare",input.clone(),&f.store).is_err());input["scopeRef"]=json!(crate::scope::current_ref(&f.store.snapshot().unwrap(),&f.aid).unwrap().unwrap());input["targetRef"]=json!(f.refs[0]);assert!(service.handle("remediation.prepare",input.clone(),&f.store).is_err());input["targetRef"]=json!(f.refs[1]);let mut excluded=input.clone();excluded["relativePath"]=json!("secret.txt");assert!(service.handle("remediation.prepare",excluded,&f.store).unwrap_err().contains("scope_local_path_excluded"));
        let plan=service.handle("remediation.prepare",input,&f.store).unwrap();assert_eq!(plan["target"],second.to_str().unwrap());assert_eq!(plan["identityPreflightRefs"].as_array().unwrap().len(),2);service.handle("remediation.approve",decision(&plan),&f.store).unwrap();service.handle("remediation.apply",decision(&plan),&f.store).unwrap();let retest=service.handle("remediation.retest",decision(&plan),&f.store).unwrap();assert_eq!(retest["passed"],true);
        service.handle("remediation.confirm",json!({"id":plan["id"],"digest":plan["digest"],"evidenceId":retest["evidenceId"],"rationale":"Verified only the second synthetic file"}),&f.store).unwrap();let snapshot=f.store.snapshot().unwrap();assert_eq!(snapshot.findings[0].status,"fixed");validate_fixed_findings(&snapshot).unwrap();assert_eq!(fs::read(first.join("settings.txt")).unwrap(),b"PRIMARY-SENTINEL");assert_eq!(fs::read(second.join("settings.txt")).unwrap(),b"secure=true\n");
        let database=f.store.root.join("remediation.sqlite3");for e in snapshot.assessments[0].evidence.iter().filter(|e|e.content.contains("openkoda.remediation.")){assert_eq!(evidence_target_refs(&database,&snapshot,&f.aid,e).unwrap(),Some(vec![f.refs[1].clone()]));}
        let raw=snapshot.assessments[0].evidence.iter().find(|e|Some(e.id.as_str())==retest["evidenceId"].as_str()).unwrap();let mut proof:Value=serde_json::from_str(&raw.content).unwrap();proof["targetRef"]=json!(f.refs[0]);proof["target"]=json!(first);assert!(raw_target(&snapshot,&f.aid,&proof).is_err());proof["targetRef"]=json!(f.refs[1]);proof.as_object_mut().unwrap().remove("identityPreflightRefs");assert!(raw_target(&snapshot,&f.aid,&proof).is_err());
        service.db.lock().unwrap().execute("DELETE FROM remediation_plans WHERE id=?1",[plan["id"].as_str().unwrap()]).unwrap();assert!(evidence_target_refs(&database,&snapshot,&f.aid,raw).is_err());
    }

    struct Fixture { _root:tempfile::TempDir, store:Store, service:RemediationService, target:PathBuf, file:PathBuf, finding:String }
    fn fixture()->Fixture {fixture_target(false)}
    fn fixture_target(file_target:bool)->Fixture {
        let root=tempfile::tempdir().unwrap();let target=root.path().join("source");fs::create_dir(&target).unwrap();
        let file=target.join("settings.txt");fs::write(&file,"secure=false\n").unwrap();
        let store=Store::open(root.path().join("data")).unwrap();let service=RemediationService::new(store.root.clone()).unwrap();
        let project=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Local fixture","kind":"code","target":if file_target{file.to_string_lossy()}else{target.to_string_lossy()},"description":"Synthetic only"})}).unwrap();
        let assessment=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Fixture assessment","kind":"code","target":project["target"],"objective":"Validate bounded text change","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&["local.text-remediation.v1"])})}).unwrap();
        let finding=model::id();let content="Synthetic initial observation".to_owned();
        let evidence=Evidence{id:model::id(),name:"fixture.txt".into(),sha256:hash(&content),content,created_at:model::now()};
        store.mutate(|s|{
            let a=s.assessments.iter_mut().find(|a|a.id==assessment["id"]).unwrap();a.status="partial".into();a.evidence.push(evidence.clone());
            s.findings.push(Finding{id:finding.clone(),assessment_id:a.id.clone(),title:"Synthetic text condition".into(),severity:"low".into(),confidence:"Human reviewed fixture".into(),status:"validated".into(),description:"Only a test fixture".into(),recommendation:String::new(),evidence_ids:vec![evidence.id.clone()],created_at:model::now()});
            s.validations.push(ValidationRecord{id:model::id(),finding_id:finding.clone(),assessment_id:a.id.clone(),method:"human_review".into(),expected:"secure=false".into(),observed:"Synthetic observation only".into(),evidence:vec![ValidatedEvidence{id:evidence.id.clone(),sha256:evidence.sha256.clone()}],author:"local-user".into(),created_at:model::now()});Ok(())
        }).unwrap();
        Fixture {_root:root,store,service,target,file,finding}
    }
    fn prepare(f:&Fixture)->Value {f.service.handle("remediation.prepare",json!({"findingId":f.finding,"relativePath":"settings.txt","after":"secure=true\n","retest":{"kind":"contains","value":"secure=true"}}),&f.store).unwrap()}
    fn decision(p:&Value)->Value {json!({"id":p["id"],"digest":p["digest"]})}
    fn approve(f:&Fixture,p:&Value) {f.service.handle("remediation.approve",decision(p),&f.store).unwrap();}
    fn apply(f:&Fixture,p:&Value)->Value {f.service.handle("remediation.apply",decision(p),&f.store).unwrap()}
    fn retest(f:&Fixture,p:&Value)->Value {f.service.handle("remediation.retest",decision(p),&f.store).unwrap()}
    fn confirm(f:&Fixture,p:&Value,r:&Value)->Result<Value,String> {f.service.handle("remediation.confirm",json!({"id":p["id"],"digest":p["digest"],"evidenceId":r["evidenceId"],"rationale":"Verified only the synthetic text predicate; no broader security conclusion."}),&f.store)}

    #[test]
    fn local_patch_requires_exact_approval_retest_and_separate_human_confirmation() {
        let f=fixture();let p=prepare(&f);
        assert_eq!(p["before"],"secure=false\n");assert_eq!(p["after"],"secure=true\n");
        assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());
        assert!(f.service.handle("remediation.approve",json!({"id":p["id"],"digest":"forged"}),&f.store).is_err());
        approve(&f,&p);assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
        let applied=apply(&f,&p);assert_eq!(applied["status"],"applied");assert_eq!(f.store.snapshot().unwrap().findings[0].status,"validated");
        assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());
        assert!(confirm(&f,&p,&json!({"evidenceId":model::id()})).is_err());
        let r=retest(&f,&p);assert_eq!(r["passed"],true);assert_eq!(f.store.snapshot().unwrap().findings[0].status,"validated");
        assert!(confirm(&f,&p,&json!({"evidenceId":model::id()})).is_err());
        let confirmed=confirm(&f,&p,&r).unwrap();assert_eq!(confirmed["status"],"confirmed");
        let state=f.store.snapshot().unwrap();assert_eq!(state.findings[0].status,"fixed");validate_fixed_findings(&state).unwrap();
        assert!(confirm(&f,&p,&r).is_err());
        let backup:Vec<u8>=f.service.db.lock().unwrap().query_row("SELECT backup FROM remediation_plans WHERE id=?1",[p["id"].as_str().unwrap()],|row|row.get(0)).unwrap();
        assert_eq!(backup,b"secure=false\n");
        assert_eq!(f.service.handle("remediation.list",json!({"findingId":f.finding}),&f.store).unwrap()["plans"][0]["status"],"confirmed");
    }
    #[test]
    fn legacy_applied_plan_requires_adoption_then_new_scoped_retest_without_reapplying(){
        let f=fixture();let p=prepare(&f);approve(&f,&p);apply(&f,&p);
        // A v0.1.2-shaped applied record has no scope ref or new retest receipt.
        let mut legacy=f.service.load(p["id"].as_str().unwrap()).unwrap();legacy.scope_ref=None;legacy.retest_scope_ref=None;legacy.digest=plan_digest(&legacy);f.service.save(&legacy).unwrap();let legacy=encode(&legacy).unwrap();
        f.store.mutate(|s|{s.assessments[0].scope_id=None;s.scope_revisions.clear();s.assessment_scopes.clear();s.scope_receipts.clear();Ok(())}).unwrap();
        let exact=f.service.db.lock().unwrap().query_row("SELECT data FROM remediation_plans WHERE id=?1",[legacy["id"].as_str().unwrap()],|r|r.get::<_,String>(0)).unwrap();
        assert!(f.service.handle("remediation.retest",decision(&legacy),&f.store).unwrap_err().contains("scope_legacy_requires_adoption"));let aid=f.store.snapshot().unwrap().assessments[0].id.clone();
        let status=crate::scope::status(&f.store,&aid).unwrap();let mut input=json!({"assessmentId":aid,"expectedStateDigest":status["stateDigest"],"objective":"Validate bounded text change","methods":["local.text-remediation.v1"],"window":{"startsAt":null,"endsAt":null}});let preview=crate::scope::handle("assessment.scopePreview",input.clone(),&f.store,str::to_owned).unwrap();input["reviewDigest"]=preview["reviewDigest"].clone();input["idempotencyKey"]=json!(model::id());input["acknowledgeScope"]=json!(true);crate::scope::handle("assessment.scopeAdopt",input,&f.store,str::to_owned).unwrap();
        assert_eq!(f.service.db.lock().unwrap().query_row("SELECT data FROM remediation_plans WHERE id=?1",[legacy["id"].as_str().unwrap()],|r|r.get::<_,String>(0)).unwrap(),exact);
        assert!(f.service.handle("remediation.apply",decision(&legacy),&f.store).is_err());let before=fs::read(&f.file).unwrap();let r=retest(&f,&legacy);assert!(r.get("scopeRef").is_none());assert!(r["retestScopeRef"].is_object());assert_eq!(r["passed"],true);assert_eq!(fs::read(&f.file).unwrap(),before);confirm(&f,&legacy,&r).unwrap();assert_eq!(fs::read(&f.file).unwrap(),before);validate_fixed_findings(&f.store.snapshot().unwrap()).unwrap();
    }
    #[test]
    fn revoked_scope_prevents_apply_retest_and_confirmation_but_preserves_history(){
        for stage in ["approved","applied","retested"]{let f=fixture();let p=prepare(&f);approve(&f,&p);if stage!="approved"{apply(&f,&p);}let r=if stage=="retested"{Some(retest(&f,&p))}else{None};let aid=f.store.snapshot().unwrap().assessments[0].id.clone();let scope=crate::scope::status(&f.store,&aid).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":aid,"expectedStateDigest":scope["stateDigest"],"idempotencyKey":model::id(),"rationale":"End fixture scope","acknowledgeRevocation":true}),&f.store,str::to_owned).unwrap();
            let file=fs::read(&f.file).unwrap();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();let result=match stage{"approved"=>f.service.handle("remediation.apply",decision(&p),&f.store),"applied"=>f.service.handle("remediation.retest",decision(&p),&f.store),_=>confirm(&f,&p,r.as_ref().unwrap())};assert!(result.unwrap_err().contains("scope_revoked"));assert_eq!(fs::read(&f.file).unwrap(),file);assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert_eq!(f.store.snapshot().unwrap().findings[0].status,"validated");}
    }
    #[test]
    fn scoped_fixed_proof_rejects_cross_revision_even_when_all_hashes_are_recomputed(){
        let f=fixture();let p=prepare(&f);approve(&f,&p);apply(&f,&p);let r=retest(&f,&p);confirm(&f,&p,&r).unwrap();
        let original=f.store.snapshot().unwrap();for legacy in [false,true]{let mut s=original.clone();let a=&mut s.assessments[0];let retest=a.evidence.iter_mut().find(|e|e.id==r["evidenceId"]).unwrap();let mut value:Value=serde_json::from_str(&retest.content).unwrap();if legacy{value.as_object_mut().unwrap().remove("scopeRef");}else{value["scopeRef"]["revisionId"]=json!(model::id());}let reference=value.get("scopeRef").cloned();retest.content=value.to_string();retest.sha256=hash(&retest.content);let retest_sha=retest.sha256.clone();
            let confirmation=a.evidence.iter_mut().find(|e|serde_json::from_str::<Value>(&e.content).is_ok_and(|v|v["schema"]=="openkoda.remediation.confirmation.v1")).unwrap();let mut value:Value=serde_json::from_str(&confirmation.content).unwrap();value["retestSha256"]=json!(retest_sha);if let Some(reference)=reference{value["scopeRef"]=reference;}else{value.as_object_mut().unwrap().remove("scopeRef");}confirmation.content=value.to_string();confirmation.sha256=hash(&confirmation.content);
            for validation in &mut s.validations{for reference in &mut validation.evidence{if let Some(e)=a.evidence.iter().find(|e|e.id==reference.id){reference.sha256=e.sha256.clone();}}}
            assert_eq!(core::validate_snapshot(&s).is_ok(),legacy,"legacy absence remains valid; a forged new scope ref does not");}
    }
    #[test]
    fn concurrent_base_change_invalidates_patch_without_overwriting_user_work() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);fs::write(&f.file,"user edits\n").unwrap();
        assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(),"user edits\n");assert_eq!(f.service.load(p["id"].as_str().unwrap()).unwrap().status,"invalidated");
    }
    #[test]
    fn changed_file_after_retest_cannot_be_confirmed_fixed() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);apply(&f,&p);let r=retest(&f,&p);
        fs::write(&f.file,"changed after retest\n").unwrap();assert!(confirm(&f,&p,&r).is_err());
        assert_eq!(f.store.snapshot().unwrap().findings[0].status,"validated");assert_eq!(fs::read_to_string(&f.file).unwrap(),"changed after retest\n");
    }
    #[test]
    fn retest_records_failed_condition_without_promoting_finding() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);apply(&f,&p);fs::write(&f.file,"secure=false\n").unwrap();
        let r=retest(&f,&p);assert_eq!(r["passed"],false);assert!(confirm(&f,&p,&r).is_err());
        let state=f.store.snapshot().unwrap();assert_eq!(state.findings[0].status,"validated");
        let e=state.assessments[0].evidence.iter().find(|e|e.id==r["evidenceId"]).unwrap();assert_eq!(serde_json::from_str::<Value>(&e.content).unwrap()["passed"],false);
    }
    #[test]
    fn archived_or_active_scope_cannot_write_a_previously_approved_patch() {
        for archive in [false,true] {
            let f=fixture();let p=prepare(&f);approve(&f,&p);
            f.store.mutate(|s|{if archive{s.projects[0].archived=true;}else{s.assessments[0].status="unknown".into();}Ok(())}).unwrap();
            assert_eq!(f.service.handle("remediation.list",json!({"findingId":f.finding}),&f.store).unwrap()["plans"].as_array().unwrap().len(),1);
            assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
        }
    }
    #[test]
    fn paths_ads_hardlinks_and_busy_handles_are_refused() {
        let f=fixture();
        for path in ["../outside.txt","settings.txt:stream","C:\\outside.txt",".","NUL","folder/../settings.txt","settings.txt.","settings.txt "] {
            assert!(f.service.handle("remediation.prepare",json!({"findingId":f.finding,"relativePath":path,"after":"secure=true\n","retest":{"kind":"contains","value":"secure=true"}}),&f.store).is_err(),"accepted {path}");
        }
        let busy=fs::OpenOptions::new().read(true).share_mode(0).open(&f.file).unwrap();
        assert!(LockedTarget::open(f.target.to_str().unwrap(),"settings.txt",true).is_err());drop(busy);
        let link=f.target.join("alias.txt");fs::hard_link(&f.file,&link).unwrap();
        assert!(LockedTarget::open(f.target.to_str().unwrap(),"settings.txt",true).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
    }
    #[test]
    fn exclusive_target_and_parent_handles_prevent_external_writes_and_renames() {
        let f=fixture();let _locked=LockedTarget::open(f.target.to_str().unwrap(),"settings.txt",true).unwrap();
        assert!(fs::write(&f.file,"outside writer").is_err());
        assert!(fs::rename(&f.file,f.target.join("renamed.txt")).is_err());
        assert!(fs::rename(&f.target,f._root.path().join("renamed-source")).is_err());
    }
    #[test]
    fn junction_ancestor_cannot_redirect_patch_outside_selected_tree() {
        let f=fixture();let outside=f._root.path().join("outside");fs::create_dir(&outside).unwrap();fs::write(outside.join("secret.txt"),"secure=false\n").unwrap();
        let link=f.target.join("linked");
        let status=std::process::Command::new("cmd.exe").arg("/C").arg("mklink").arg("/J").arg(&link).arg(&outside).output().unwrap();
        assert!(status.status.success(),"junction fixture creation failed");
        assert!(LockedTarget::open(f.target.to_str().unwrap(),"linked/secret.txt",true).is_err());
        assert_eq!(fs::read_to_string(outside.join("secret.txt")).unwrap(),"secure=false\n");
    }
    #[test]
    fn json_debug_retest_requires_the_enabled_flag_and_an_exact_disabled_file() {
        let f = fixture();
        let after = r#"{"debug":false,"description":"debug=true"}"#;
        let request = json!({"findingId":f.finding,"relativePath":"settings.txt",
            "after":after,"retest":{"kind":JSON_DEBUG_RULE,"value":"false"}});
        for ambiguous in [r#"{"nested":{"debug":true}}"#, r#"{"debug":"true"}"#,
            r#"{"debug":true,"debug":false}"#, "[true]", "invalid JSON"] {
            fs::write(&f.file, ambiguous).unwrap();
            assert!(f.service.handle("remediation.prepare", request.clone(), &f.store).is_err());
        }
        fs::write(&f.file, r#"{"debug":true}"#).unwrap();
        let plan = f.service.handle("remediation.prepare", request, &f.store).unwrap();
        approve(&f, &plan);
        apply(&f, &plan);
        let result = retest(&f, &plan);
        assert_eq!(result["passed"], true);
        assert_eq!(f.store.snapshot().unwrap().findings[0].status, "validated");
        confirm(&f, &plan, &result).unwrap();
        let state = f.store.snapshot().unwrap();
        validate_fixed_findings(&state).unwrap();
        assert_eq!(state.findings[0].status, "fixed");
    }
    #[test]
    fn invalid_utf8_oversize_and_tautological_retests_cannot_prepare() {
        let f=fixture();
        assert!(f.service.handle("remediation.prepare",json!({"findingId":f.finding,"relativePath":"settings.txt","after":"secure=true\n","retest":{"kind":"contains","value":"secure="}}),&f.store).is_err());
        assert!(f.service.handle("remediation.prepare",json!({"findingId":f.finding,"relativePath":"settings.txt","after":"x".repeat(MAX_FILE+1),"retest":{"kind":"absent","value":"secure=false"}}),&f.store).is_err());
        fs::write(&f.file,[0xff,0xfe]).unwrap();assert!(LockedTarget::open(f.target.to_str().unwrap(),"settings.txt",false).unwrap().read().is_err());
        fs::write(&f.file,vec![b'x';MAX_FILE+1]).unwrap();assert!(LockedTarget::open(f.target.to_str().unwrap(),"settings.txt",false).unwrap().read().is_err());
        let contains=RetestRule{kind:"contains".into(),value:"ok".into()};let absent=RetestRule{kind:"absent".into(),value:"bad".into()};let sha=RetestRule{kind:"sha256".into(),value:hash("ok")};
        assert!(evaluate(&contains,"ok"));assert!(evaluate(&absent,"ok"));assert!(evaluate(&sha,"ok"));assert!(!evaluate(&sha,"bad"));
    }
    #[test]
    fn failed_domain_commit_rolls_back_under_the_same_exclusive_handle() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);
        f.store.fail_commit_when(|state|state.assessments.iter().flat_map(|a|&a.events).any(|event|event.message.starts_with("Correção ")&&event.message.contains("aplicada ao arquivo selecionado")));
        assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
        assert_eq!(f.service.load(p["id"].as_str().unwrap()).unwrap().status,"rolled_back");
        assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());
    }
    #[test]
    fn unexpected_core_trigger_blocks_patch_before_any_source_write(){
        let f=fixture();let p=prepare(&f);approve(&f,&p);
        let db=Connection::open(f.store.root.join("openkoda.sqlite3")).unwrap();
        db.execute_batch("CREATE TRIGGER reject_fixture_update BEFORE UPDATE ON app_state BEGIN SELECT RAISE(ABORT, 'unexpected schema'); END;").unwrap();
        assert!(f.service.handle("remediation.apply",decision(&p),&f.store).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
        assert_eq!(f.service.load(p["id"].as_str().unwrap()).unwrap().status,"approved");
    }
    #[test]
    fn restart_marks_interrupted_write_unknown_and_preserves_partial_bytes_without_replay() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);
        let mut interrupted=f.service.load(p["id"].as_str().unwrap()).unwrap();interrupted.status="applying".into();f.service.save(&interrupted).unwrap();
        fs::write(&f.file,"partial").unwrap();let root=f.store.root.clone();drop(f.service);
        let restarted=RemediationService::new(root).unwrap();assert_eq!(restarted.load(p["id"].as_str().unwrap()).unwrap().status,"unknown");
        assert!(restarted.handle("remediation.apply",decision(&p),&f.store).is_err());assert_eq!(fs::read_to_string(&f.file).unwrap(),"partial");
    }
    fn interrupt(f:&Fixture,p:&Value,phase:&str) {
        let mut interrupted=f.service.load(p["id"].as_str().unwrap()).unwrap();interrupted.status=phase.into();
        f.service.db.lock().unwrap().execute("UPDATE remediation_plans SET data=?1,backup=?2 WHERE id=?3",
            params![serde_json::to_string(&interrupted).unwrap(),interrupted.before.as_bytes(),interrupted.id]).unwrap();
    }
    fn reconcile_payload(p:&Value)->Value {json!({"id":p["id"],"note":"I reviewed this synthetic interruption and acknowledge that historical effects remain unknown.","acknowledgeUnknownEffects":true})}
    #[test]
    fn reconciliation_observes_before_after_and_partial_bytes_without_replay_or_promotion() {
        for (phase,bytes,expected) in [("applying",b"secure=false\n".as_slice(),"before"),("retesting",b"secure=true\n".as_slice(),"after"),("confirming",b"partial\0\xff".as_slice(),"other")] {
            let f=fixture();let p=prepare(&f);approve(&f,&p);interrupt(&f,&p,phase);fs::write(&f.file,bytes).unwrap();
            let root=f.store.root.clone();drop(f.service);let restarted=RemediationService::new(root).unwrap();
            let interrupted=restarted.load(p["id"].as_str().unwrap()).unwrap();assert_eq!(interrupted.status,"unknown");assert_eq!(interrupted.interrupted_operation.as_deref(),Some(phase));assert!(restarted.has_unresolved().unwrap());
            let response=restarted.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap();
            assert_eq!(response["status"],"reconciled");assert_eq!(response["reconciliation"]["observation"]["state"],expected);
            assert_eq!(response["reconciliation"]["observation"]["identityMatches"],true);
            assert_eq!(response["reconciliation"]["observation"]["observedSha256"],format!("{:x}",Sha256::digest(bytes)));
            assert_eq!(response["reconciliation"]["interruptedOperation"],phase);assert!(!restarted.has_unresolved().unwrap());
            assert_eq!(fs::read(&f.file).unwrap(),bytes);let snapshot=f.store.snapshot().unwrap();assert_eq!(snapshot.findings[0].status,"validated");
            let evidence=snapshot.assessments[0].evidence.iter().find(|e|e.id==response["reconciliation"]["evidenceId"]).unwrap();
            let receipt:Value=serde_json::from_str(&evidence.content).unwrap();assert_eq!(receipt["targetWritten"],false);assert_eq!(receipt["patchReplayed"],false);assert_eq!(receipt["findingStatusChanged"],false);assert_eq!(receipt["historicalEffectsKnown"],false);
            assert!(snapshot.findings[0].evidence_ids.contains(&evidence.id));
            let backup:Vec<u8>=restarted.db.lock().unwrap().query_row("SELECT backup FROM remediation_plans WHERE id=?1",[p["id"].as_str().unwrap()],|row|row.get(0)).unwrap();assert_eq!(backup,b"secure=false\n");
            for action in ["remediation.apply","remediation.retest"] {assert!(restarted.handle(action,decision(&p),&f.store).is_err());}
            assert_eq!(restarted.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap(),response);
            assert_eq!(f.store.snapshot().unwrap().assessments[0].evidence.len(),snapshot.assessments[0].evidence.len());
            drop(restarted);let reopened=RemediationService::new(f.store.root.clone()).unwrap();assert!(!reopened.has_unresolved().unwrap());assert_eq!(reopened.load(p["id"].as_str().unwrap()).unwrap().status,"reconciled");assert_eq!(fs::read(&f.file).unwrap(),bytes);
        }
    }
    #[test]
    fn reconciliation_requires_explicit_acknowledgment_note_and_unresolved_state() {
        let f=fixture();let p=prepare(&f);
        assert!(f.service.handle("remediation.reconcile",reconcile_payload(&p),&f.store).is_err());
        interrupt(&f,&p,"unknown");
        for (note,ack) in [("",true),("  ",true),("reviewed",false),("nul\0note",true)] {
            assert!(f.service.handle("remediation.reconcile",json!({"id":p["id"],"note":note,"acknowledgeUnknownEffects":ack}),&f.store).is_err());
        }
        assert!(f.service.handle("remediation.reconcile",json!({"id":p["id"],"note":"reviewed","acknowledgeUnknownEffects":true,"restore":true}),&f.store).is_err());
        assert!(f.service.has_unresolved().unwrap());assert!(f.service.load(p["id"].as_str().unwrap()).unwrap().reconciliation.is_none());assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
    }
    #[test]
    fn reconciliation_distinguishes_missing_replacement_and_unavailable_paths() {
        for condition in ["missing","replacement","busy","hardlink","ancestor"] {
            let f=fixture();let p=prepare(&f);interrupt(&f,&p,"unknown");let mut busy=None;
            match condition {
                "missing"=>fs::remove_file(&f.file).unwrap(),
                "replacement"=>{fs::rename(&f.file,f.target.join("original.txt")).unwrap();fs::write(&f.file,b"secure=false\n").unwrap();},
                "busy"=>busy=Some(fs::OpenOptions::new().read(true).share_mode(0).open(&f.file).unwrap()),
                "hardlink"=>fs::hard_link(&f.file,f.target.join("alias.txt")).unwrap(),
                "ancestor"=>fs::rename(&f.target,f._root.path().join("moved-source")).unwrap(),
                _=>unreachable!(),
            }
            let response=f.service.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap();let observation=&response["reconciliation"]["observation"];
            match condition {
                "missing"=>{assert_eq!(observation["state"],"missing");assert!(!f.file.exists());assert!(observation["observedSha256"].is_null());},
                "replacement"=>{assert_eq!(observation["state"],"before");assert_eq!(observation["identityMatches"],false);assert_eq!(fs::read(&f.file).unwrap(),b"secure=false\n");},
                _=>{assert_eq!(observation["state"],"unavailable");assert!(observation["identityMatches"].is_null());assert!(observation["observedSha256"].is_null());},
            }
            assert_eq!(f.store.snapshot().unwrap().findings[0].status,"validated");drop(busy);
        }
    }
    #[test]
    fn reconciliation_of_file_target_missing_does_not_reinterpret_it_as_directory() {
        let f=fixture_target(true);
        let p=prepare(&f);interrupt(&f,&p,"unknown");fs::remove_file(&f.file).unwrap();
        let response=f.service.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap();assert_eq!(response["reconciliation"]["observation"]["state"],"missing");assert!(!f.file.exists());
    }
    #[test]
    fn reconciliation_preserves_a_confirmation_committed_before_interruption() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);apply(&f,&p);let r=retest(&f,&p);confirm(&f,&p,&r).unwrap();interrupt(&f,&p,"confirming");
        let root=f.store.root.clone();drop(f.service);let restarted=RemediationService::new(root).unwrap();
        let before=f.store.snapshot().unwrap();let response=restarted.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap();
        assert_eq!(response["reconciliation"]["findingStatus"],"fixed");let after=f.store.snapshot().unwrap();assert_eq!(after.findings[0].status,"fixed");assert_eq!(after.validations.len(),before.validations.len());validate_fixed_findings(&after).unwrap();assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=true\n");
    }
    #[test]
    fn reconciliation_recovers_cross_database_interruption_without_duplicate_evidence() {
        for fail_after_domain_commit in [false,true] {
            let f=fixture();let p=prepare(&f);interrupt(&f,&p,"unknown");
            let failure_db=Connection::open(f.store.root.join(if fail_after_domain_commit {"remediation.sqlite3"}else{"openkoda.sqlite3"})).unwrap();
            failure_db.execute_batch(if fail_after_domain_commit {
                "CREATE TRIGGER reject_reconciliation BEFORE UPDATE ON remediation_plans WHEN json_extract(NEW.data,'$.status')='reconciled' BEGIN SELECT RAISE(ABORT,'synthetic final receipt failure'); END;"
            }else{"CREATE TRIGGER reject_reconciliation BEFORE UPDATE ON app_state BEGIN SELECT RAISE(ABORT,'synthetic domain failure'); END;"}).unwrap();
            assert!(f.service.handle("remediation.reconcile",reconcile_payload(&p),&f.store).is_err());
            let pending=f.service.load(p["id"].as_str().unwrap()).unwrap();assert_eq!(pending.status,"unknown");assert!(pending.reconciliation.is_some());let recorded=serde_json::to_value(&pending.reconciliation).unwrap();
            failure_db.execute_batch("DROP TRIGGER reject_reconciliation").unwrap();drop(failure_db);fs::write(&f.file,b"changed after the recorded observation").unwrap();
            let root=f.store.root.clone();drop(f.service);let restarted=RemediationService::new(root).unwrap();assert!(restarted.has_unresolved().unwrap());
            let mut changed_note=reconcile_payload(&p);changed_note["note"]=json!("different note");assert!(restarted.handle("remediation.reconcile",changed_note,&f.store).is_err());
            let response=restarted.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap();assert_eq!(response["reconciliation"],recorded);
            assert_eq!(response["reconciliation"]["observation"]["state"],"before");assert_eq!(fs::read(&f.file).unwrap(),b"changed after the recorded observation");
            let snapshot=f.store.snapshot().unwrap();let id=response["reconciliation"]["evidenceId"].as_str().unwrap();assert_eq!(snapshot.assessments[0].evidence.iter().filter(|e|e.id==id).count(),1);assert_eq!(snapshot.findings[0].status,"validated");assert!(!restarted.has_unresolved().unwrap());
        }
    }
    #[test]
    fn reconciliation_preserves_legacy_plan_digest_and_rejects_receipt_tampering() {
        let f=fixture();let p=prepare(&f);let mut legacy=p.clone();legacy.as_object_mut().unwrap().remove("interruptedOperation");legacy.as_object_mut().unwrap().remove("reconciliation");
        assert_eq!(read_plan(&legacy.to_string()).unwrap().digest,p["digest"]);interrupt(&f,&p,"unknown");let mut response=f.service.handle("remediation.reconcile",reconcile_payload(&p),&f.store).unwrap();
        assert_eq!(response["digest"],p["digest"]);response["reconciliation"]["observation"]["state"]=json!("after");assert!(read_plan(&response.to_string()).is_err());
    }
    #[test]
    fn restart_revokes_unused_approval_and_tampering_is_preserved_but_rejected() {
        let f=fixture();let p=prepare(&f);approve(&f,&p);let root=f.store.root.clone();drop(f.service);
        let restarted=RemediationService::new(root).unwrap();assert_eq!(restarted.load(p["id"].as_str().unwrap()).unwrap().status,"revoked");
        let mut altered=restarted.load(p["id"].as_str().unwrap()).unwrap();altered.after="unreviewed".into();
        restarted.db.lock().unwrap().execute("UPDATE remediation_plans SET data=?1 WHERE id=?2",params![serde_json::to_string(&altered).unwrap(),altered.id]).unwrap();
        assert!(restarted.handle("remediation.get",json!({"id":p["id"]}),&f.store).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(),"secure=false\n");
    }
    #[test]
    fn fixed_invariant_rejects_missing_or_altered_retest_evidence() {
        let f=fixture();let mut state=f.store.snapshot().unwrap();state.findings[0].status="fixed".into();assert!(validate_fixed_findings(&state).is_err());
        let p=prepare(&f);approve(&f,&p);apply(&f,&p);let r=retest(&f,&p);confirm(&f,&p,&r).unwrap();
        let mut state=f.store.snapshot().unwrap();let e=state.assessments[0].evidence.iter_mut().find(|e|e.id==r["evidenceId"]).unwrap();e.content.push(' ');
        assert!(validate_fixed_findings(&state).is_err());
    }
    #[test]
    fn report_omits_source_and_predicate_bytes_while_restore_keeps_local_history() {
        let f=fixture();let before="BEFORE-SOURCE-SENTINEL\n";let after="AFTER-SOURCE-SENTINEL\n";fs::write(&f.file,before).unwrap();
        let p=f.service.handle("remediation.prepare",json!({"findingId":f.finding,"relativePath":"settings.txt","after":after,"retest":{"kind":"contains","value":"AFTER-SOURCE-SENTINEL"}}),&f.store).unwrap();
        approve(&f,&p);apply(&f,&p);let r=retest(&f,&p);confirm(&f,&p,&r).unwrap();
        let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();
        let report=crate::recovery::test_support::report(&recovery,&f.store,p["assessmentId"].as_str().unwrap(),true).unwrap();
        let raw=fs::read_to_string(report["manifestPath"].as_str().unwrap()).unwrap();
        assert!(!raw.contains("BEFORE-SOURCE-SENTINEL"));assert!(!raw.contains("AFTER-SOURCE-SENTINEL"));
        let manifest:Value=serde_json::from_str(&raw).unwrap();let record=&manifest["remediations"][0];
        assert_eq!(record["id"],p["id"]);assert_eq!(record["status"],"confirmed");assert_eq!(record["baseSha256"],hash(before));assert_eq!(record["afterSha256"],hash(after));
        assert!(record.get("before").is_none());assert!(record.get("after").is_none());assert!(record["retest"].get("value").is_none());
        let markdown=fs::read_to_string(report["path"].as_str().unwrap()).unwrap();assert!(markdown.contains("Correções e retestes"));assert!(!markdown.contains("SOURCE-SENTINEL"));
        let backup=crate::recovery::test_support::backup(&recovery,&f.store).unwrap();
        assert!(backup["manifest"]["files"].as_array().unwrap().iter().any(|v|v["name"]=="remediation.sqlite3"));
        let restored=recovery.handle("backup.restore",json!({"id":backup["id"]}),&f.store).unwrap();
        let root=PathBuf::from(restored["path"].as_str().unwrap());let restored_store=Store::open(root.clone()).unwrap();let restored_service=RemediationService::new(root.clone()).unwrap();
        let copy=restored_service.handle("remediation.get",json!({"id":p["id"]}),&restored_store).unwrap();
        assert_eq!(copy["status"],"confirmed");assert_eq!(copy["before"],before);assert_eq!(copy["after"],after);
        let original:Vec<u8>=Connection::open(root.join("remediation.sqlite3")).unwrap().query_row("SELECT backup FROM remediation_plans WHERE id=?1",[p["id"].as_str().unwrap()],|row|row.get(0)).unwrap();assert_eq!(original,before.as_bytes());
        assert_eq!(fs::read_to_string(&f.file).unwrap(),after);assert_eq!(restored["activated"],false);
    }
}
