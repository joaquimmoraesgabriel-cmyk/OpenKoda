//! Narrow, host-owned MCP stdio bridge. No shell, paths, credentials or grants
//! are accepted from the caller. Killing wsl.exe is never proof of Linux cleanup.
use crate::{core::{hash, valid_id, Store}, model::{self, Evidence}};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, io::{Read, Write}, path::PathBuf, process::{Child, Command, Stdio},
    sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex}, time::{Duration, Instant}};

const SOURCE: &str = include_str!("../../scripts/kali-executor.py");
const PROTOCOL: &str = "2025-11-25";
const MAX_BYTES: usize = 32768;
const MAX_FRAME: usize = 8192;
const MAX_OUTPUT: usize = 65536;
#[path="kali_data.rs"] mod data;
#[path="kali_work.rs"] mod work;
#[path="kali_queued.rs"] mod queued;
#[path="kali_capture.rs"] mod capture_io;
pub(crate) use work::{KaliRecoveryTarget,KaliReconciliationProof,KaliNotStartedProof,has_unresolved_work};
pub(crate) use work::read_runs as read_history_runs;
pub(crate) use queued::{WorkKaliExpectation,QueuedKaliGrant,WorkKaliCompletion,KaliWorkBinding,KaliCaptureSlot};
pub(crate) use capture_io::{KaliSpoolRef,KaliPurgeInspection,KaliPurgeReceipt};
pub(crate) use capture_io::{KaliMetadataItem,KaliMetadataState,KaliRetirePreview,KaliRetireReceipt,KaliRetireState};
pub(crate) use capture_io::ensure_capture_history_purgeable;
const BOOT: &str = include_str!("../../scripts/kali-bootstrap.py");

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct BridgeRun {
    pub id:String, pub assessment_id:String, pub environment_id:String, pub tool:String,
    pub status:String, pub started_at:String, pub finished_at:Option<String>,
    pub source_sha256:String, pub evidence_id:Option<String>, pub error:Option<String>,
    pub cleanup_confirmed:bool,
    #[serde(default)] pub launch_attempted:bool,
    #[serde(default)] pub dispatch_before_unix_seconds:u64,
    #[serde(default,skip_serializing_if="Option::is_none")] pub source_evidence_id:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub scope_ref:Option<crate::scope::ScopeRef>,
    #[serde(default,skip_serializing_if="Vec::is_empty")] pub source_target_refs:Vec<crate::scope_targets::ScopeTargetRef>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub reconciliation_of:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub(crate) work_context:Option<capture_io::KaliRunContext>,
}
struct Active { run_id:String, assessment_id:String, cancel:Arc<AtomicBool> }
pub struct KaliBridge { db:Mutex<Connection>, active:Mutex<Option<Active>>, launcher:Arc<dyn Launcher>, data_policy:Arc<crate::data_policy::DataPolicyService>, data_reviews:Mutex<std::collections::HashMap<String,data::PreparedKaliData>>, arbiter:Arc<crate::work_supervisor::WorkArbiter> }

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct AssessmentInput { assessment_id:String }
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct HashInput { assessment_id:String, evidence_id:String, #[serde(default)] data_review:Option<crate::data_policy::DataReviewRef>,#[serde(default)] acknowledge_local_transfer:bool,#[serde(default)] acknowledge_sensitive:bool }
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ScanInput { assessment_id:String, evidence_id:String, acknowledge_local_transfer:bool, #[serde(default)] data_review:Option<crate::data_policy::DataReviewRef>,#[serde(default)] acknowledge_sensitive:bool }
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReconcileInput { assessment_id:String, run_id:String }

pub fn supports(action:&str)->bool {matches!(action,"kali.mcp.status"|"kali.mcp.diagnose"|"kali.mcp.hashEvidence"|"kali.mcp.scanEvidence"|"kali.mcp.cancel"|"kali.mcp.reconcile")}
pub fn needs_identity(action:&str)->bool {matches!(action,"kali.mcp.diagnose"|"kali.mcp.hashEvidence"|"kali.mcp.scanEvidence"|"kali.mcp.reconcile")}
fn scope_method(tool:&str)->Result<&'static str,String>{match tool{"runtime_diagnostics"=>Ok("kali.runtime-diagnostics.v1"),"hash_bytes"=>Ok("kali.hash-evidence.v1"),"scan_bytes"=>Ok("kali.rg.signals.v1"),_=>Err("Método MCP sem escopo suportado.".into())}}
pub fn admit_before_identity(action:&str,payload:&Value,store:&Store)->Result<(),String>{
    let(aid,tool,evidence)=match action{
        "kali.mcp.diagnose"=>(decode::<AssessmentInput>(payload.clone())?.assessment_id,"runtime_diagnostics",None),
        "kali.mcp.hashEvidence"=>{let p:HashInput=decode(payload.clone())?;if p.data_review.is_none(){return Err("data_review_required: revise o pacote local antes de transferir.".into());}if !p.acknowledge_local_transfer{return Err("Confirme a transferência local da evidência.".into());}(p.assessment_id,"hash_bytes",Some(p.evidence_id))},
        "kali.mcp.scanEvidence"=>{let p:ScanInput=decode(payload.clone())?;if p.data_review.is_none(){return Err("data_review_required: revise o pacote local antes de transferir.".into());}if !p.acknowledge_local_transfer{return Err("Confirme a transferência local da evidência.".into());}(p.assessment_id,"scan_bytes",Some(p.evidence_id))},
        _=>return Ok(()),
    };
    let refs=source_refs(store,&aid,evidence.as_deref())?;
    crate::scope::with_selected_admission(store,&aid,scope_method(tool)?,None,&refs,action,|_|Ok(()))
}
fn source_refs(store:&Store,assessment:&str,evidence:Option<&str>)->Result<Vec<crate::scope_targets::ScopeTargetRef>,String>{
    let state=store.snapshot()?;
    if !crate::scope_targets::is_combined(&state,assessment){return Ok(vec![]);}
    match evidence{Some(id)=>crate::evidence_lineage::evidence_refs(store,&state,assessment,&[id.to_owned()]),None=>Ok(vec![])}
}
fn decode<T:serde::de::DeserializeOwned>(v:Value)->Result<T,String>{serde_json::from_value(v).map_err(|_|"Campos inválidos na solicitação MCP Kali.".into())}
fn db_error(_:impl std::fmt::Display)->String{"Falha no registro de sessões MCP; dados existentes preservados.".into()}
fn check_assessment(store:&Store,id:&str)->Result<(),String>{valid_id(id)?;if store.snapshot()?.assessments.iter().any(|a|a.id==id){Ok(())}else{Err("Avaliação não encontrada.".into())}}

impl KaliBridge {
    /// Host supervisor cancellation only. The durable work command is committed
    /// by the caller before this signal; no policy/store lock is acquired here.
    pub(crate) fn signal_work_cancel(&self,run_id:&str)->Result<(),String>{
        valid_id(run_id)?;
        let active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;
        if let Some(current)=active.as_ref().filter(|active|active.run_id==run_id){current.cancel.store(true,Ordering::SeqCst);}
        Ok(())
    }
    pub fn new(root:PathBuf)->Result<Self,String>{Self::with_launcher(root,Arc::new(SystemLauncher))}
    fn with_launcher(root:PathBuf,launcher:Arc<dyn Launcher>)->Result<Self,String>{
        fs::create_dir_all(&root).map_err(db_error)?;
        let db=Connection::open(root.join("kali-bridge.sqlite3")).map_err(db_error)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;
        let healthy:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(db_error)?;
        if healthy!="ok"{return Err("Registro MCP inconsistente; nenhuma sessão foi apagada.".into());}
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS bridge_runs(id TEXT PRIMARY KEY,data TEXT NOT NULL);").map_err(db_error)?;
        let bridge=Self{db:Mutex::new(db),active:Mutex::new(None),launcher,data_policy:Arc::new(crate::data_policy::DataPolicyService::new()),data_reviews:Mutex::new(std::collections::HashMap::new()),arbiter:Arc::new(crate::work_supervisor::WorkArbiter::default())};
        for mut run in bridge.runs()? {
            if ["starting","running","canceling"].contains(&run.status.as_str()) {
                if run.status=="starting"&&!run.launch_attempted {
                    run.status="failed".into();run.cleanup_confirmed=true;run.finished_at=Some(model::now());run.error=Some("Interrupção anterior ao ponto de despacho durável; nenhum processo foi iniciado.".into());
                }else{
                    run.status="unknown".into();run.error=Some("Supervisor reiniciado. A sessão Linux exige diagnóstico; nenhuma chamada será repetida.".into());
                }bridge.save(&run)?;
            }
        }Ok(bridge)
    }
    fn save(&self,run:&BridgeRun)->Result<(),String>{
        let encoded=serde_json::to_string(run).map_err(db_error)?;
        if encoded.len()>16384{return Err("work_journal_record_quota".into());}
        self.db.lock().map_err(db_error)?.execute("INSERT INTO bridge_runs VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",[run.id.clone(),encoded]).map_err(db_error)?;Ok(())
    }
    fn runs(&self)->Result<Vec<BridgeRun>,String>{
        let db=self.db.lock().map_err(db_error)?;work::read_runs(&db)
    }
    fn find(&self,id:&str)->Result<BridgeRun,String>{valid_id(id)?;self.runs()?.into_iter().find(|r|r.id==id).ok_or("Sessão MCP não encontrada.".into())}
    pub fn has_unresolved(&self)->bool {
        self.runs().map(|runs|runs.iter().any(work::unresolved)).unwrap_or(true)
    }
    pub fn handle(&self,action:&str,payload:Value,store:&Store,environment_id:&str)->Result<Value,String>{
        self.handle_with_identity(action,payload,store,||Ok(environment_id.into()),None)
    }
    pub fn handle_reviewed(&self,action:&str,payload:Value,store:&Store,environment_id:&str,ai:&crate::ai::AiService)->Result<Value,String>{
        self.handle_with_identity(action,payload,store,||Ok(environment_id.into()),Some(ai))
    }
    fn handle_internal(&self,action:&str,payload:Value,store:&Store,environment_id:&str,ai:Option<&crate::ai::AiService>,mut dispatch:Option<&mut work::KaliDispatch<'_>>)->Result<Value,String>{
        if !payload.is_object()||payload.to_string().len()>4096{return Err("Solicitação MCP inválida ou grande demais.".into());}
        match action {
            "kali.mcp.status"=>{
                if payload.as_object().is_none_or(|o|!o.is_empty()){return Err("Status não recebe campos.".into());}
                let active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;
                let runs=self.runs()?;let current=active.as_ref().and_then(|a|runs.iter().find(|r|r.id==a.run_id));
                Ok(json!({"active":current,"recent":runs.iter().take(50).collect::<Vec<_>>(),"hasUnresolved":runs.iter().any(work::unresolved),"capabilities":["runtime_diagnostics","hash_bytes","scan_bytes"],"executorReady":false}))
            }
            "kali.mcp.cancel"=>{
                let input:AssessmentInput=decode(payload)?;check_assessment(store,&input.assessment_id)?;
                let active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;
                let current=active.as_ref().filter(|a|a.assessment_id==input.assessment_id).ok_or("Não há sessão MCP ativa nesta avaliação.")?;
                current.cancel.store(true,Ordering::SeqCst);
                let mut run=self.find(&current.run_id)?;run.status="canceling".into();self.save(&run)?;
                Ok(json!({"requested":true,"runId":run.id,"status":"canceling"}))
            }
            "kali.mcp.diagnose"=>{
                let input:AssessmentInput=decode(payload)?;self.perform(store,environment_id,&input.assessment_id,"runtime_diagnostics",json!({}),false,None,None,dispatch.as_deref_mut().ok_or("work_owned_operation")?)
            }
            "kali.mcp.hashEvidence"=>{
                let input:HashInput=decode(payload)?;check_assessment(store,&input.assessment_id)?;valid_id(&input.evidence_id)?;
                if !input.acknowledge_local_transfer{return Err("Confirme a transferência local da evidência.".into());}
                let ai=ai.ok_or("data_review_required: use a revisão de transferência local.")?;
                let prepared=self.reviewed_kali_data(store,input.data_review.as_ref(),&input.assessment_id,&input.evidence_id,"hash",environment_id,&ai.data_redactor())?;
                let state=store.snapshot()?;let a=state.assessments.iter().find(|a|a.id==input.assessment_id).unwrap();
                let evidence=a.evidence.iter().find(|e|e.id==input.evidence_id).ok_or("Evidência fora desta avaliação ou inexistente.")?;
                if evidence.content.len()>MAX_BYTES||hash(&evidence.content)!=evidence.sha256{return Err("O hash MCP aceita somente evidência íntegra de até 32 KiB.".into());}
                self.perform(store,environment_id,&input.assessment_id,"hash_bytes",json!({"base64":STANDARD.encode(evidence.content.as_bytes()),"expectedSha256":evidence.sha256}),false,Some(&input.evidence_id),Some(data::KaliAdmission{prepared,acknowledge_sensitive:input.acknowledge_sensitive,ai}),dispatch.as_deref_mut().ok_or("work_owned_operation")?)
            }
            "kali.mcp.scanEvidence"=>{
                let input:ScanInput=decode(payload)?;
                if !input.acknowledge_local_transfer{return Err("Confirme explicitamente a transferência local desta evidência ao Kali gerido.".into());}
                let ai=ai.ok_or("data_review_required: use a revisão de transferência local.")?;
                let prepared=self.reviewed_kali_data(store,input.data_review.as_ref(),&input.assessment_id,&input.evidence_id,"scan",environment_id,&ai.data_redactor())?;
                check_assessment(store,&input.assessment_id)?;valid_id(&input.evidence_id)?;
                let state=store.snapshot()?;let a=state.assessments.iter().find(|a|a.id==input.assessment_id).unwrap();
                let evidence=a.evidence.iter().find(|e|e.id==input.evidence_id).ok_or("Evidência fora desta avaliação ou inexistente.")?;
                if evidence.content.len()>MAX_BYTES||evidence.content.contains('\0')||hash(&evidence.content)!=evidence.sha256{return Err("A análise Kali aceita somente evidência UTF-8 íntegra, sem NUL e de até 32 KiB.".into());}
                self.perform(store,environment_id,&input.assessment_id,"scan_bytes",json!({"base64":STANDARD.encode(evidence.content.as_bytes()),"expectedSha256":evidence.sha256}),false,Some(&input.evidence_id),Some(data::KaliAdmission{prepared,acknowledge_sensitive:input.acknowledge_sensitive,ai}),dispatch.as_deref_mut().ok_or("work_owned_operation")?)
            }
            "kali.mcp.reconcile"=>{
                let input:ReconcileInput=decode(payload)?;check_assessment(store,&input.assessment_id)?;
                let mut previous=self.find(&input.run_id)?;
                if previous.assessment_id!=input.assessment_id||previous.environment_id!=environment_id||previous.status!="unknown" {return Err("Somente sessão desconhecida desta avaliação e do mesmo ambiente pode ser reconciliada.".into());}
                let dispatch=dispatch.as_deref_mut().ok_or("work_owned_operation")?;
                let probe=self.perform(store,environment_id,&input.assessment_id,"runtime_diagnostics",json!({"inspectSessionId":previous.id,"dispatchDeadline":previous.dispatch_before_unix_seconds}),true,None,None,dispatch)?;
                let observation=&probe["result"]["inspectedSession"];
                if probe["cleanupConfirmed"]==true&&probe["run"]["status"]=="completed"&&observation["sessionId"]==previous.id&&observation["known"]==true&&observation["active"]==false {
                    previous.status="failed".into();previous.cleanup_confirmed=true;previous.finished_at=Some(model::now());
                    previous.error=Some("Diagnóstico confirmou ausência do processo original por sessão/boot/início. A chamada interrompida não foi repetida.".into());self.save(&previous)?;
                    self.finish_reconciliation(store,dispatch)?;
                    Ok(json!({"run":previous,"result":observation,"cleanupConfirmed":true,"probeRunId":probe["run"]["id"]}))
                } else {Ok(json!({"run":previous,"result":observation,"cleanupConfirmed":false,"probeRunId":probe["run"]["id"]}))}
            }
            _=>Err("Ação MCP Kali indisponível.".into())
        }
    }
    fn perform(&self,store:&Store,environment_id:&str,assessment_id:&str,tool:&str,arguments:Value,reconcile:bool,source_evidence_id:Option<&str>,data:Option<data::KaliAdmission<'_>>,dispatch:&mut work::KaliDispatch<'_>)->Result<Value,String>{
        valid_id(environment_id)?;check_assessment(store,assessment_id)?;
        dispatch.validate(assessment_id,environment_id,tool,&arguments,reconcile,source_evidence_id.is_some()||data.is_some())?;
        let mut active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;
        if active.is_some(){return Err("Há uma sessão MCP ativa. Aguarde ou solicite cancelamento.".into());}
        if !reconcile&&self.has_unresolved(){return Err("Existe sessão Linux com término não confirmado. Reconcilie antes de iniciar outra chamada.".into());}
        if self.runs()?.len()>=10000{return Err("Limite de registros MCP atingido; exporte o histórico antes de continuar.".into());}
        let source_target_refs=if reconcile{vec![]}else{source_refs(store,assessment_id,source_evidence_id)?};
        let mut run=BridgeRun{id:dispatch.run_id(),assessment_id:assessment_id.into(),environment_id:environment_id.into(),tool:tool.into(),status:"starting".into(),started_at:model::now(),finished_at:None,source_sha256:hash(SOURCE),evidence_id:None,error:None,cleanup_confirmed:false,launch_attempted:false,dispatch_before_unix_seconds:(chrono::Utc::now().timestamp()+30) as u64,source_evidence_id:source_evidence_id.map(str::to_owned),scope_ref:None,source_target_refs,reconciliation_of:dispatch.reconciliation_root(),work_context:None};
        let mut data_receipt=None;
        if let Some(data)=&data {
            let scope=data.prepared.spec.scope_ref.as_ref().ok_or("scope_unreviewed")?;
            run.scope_ref=Some(scope.clone());run.dispatch_before_unix_seconds=crate::scope::dispatch_deadline(store,scope,30)?;
            let admission=self.data_policy.with_admission(store,&data.prepared.review,&data.prepared.spec,Some(crate::data_policy::OperationalScope{assessment_id,method:scope_method(tool)?,expected:scope,targets:&run.source_target_refs}),crate::data_policy::EffectLink{kind:"kali_run".into(),id:run.id.clone()},data.acknowledge_sensitive,|a|{self.save(&run)?;Ok(a.receipt_ref.clone())})?;
            data_receipt=Some(match admission{crate::data_policy::AdmissionResult::Admitted(r)=>r,crate::data_policy::AdmissionResult::AlreadyUsed(_)=>return Err("data_review_used: consulte a sessão já registrada.".into())});
        }else if reconcile{self.save(&run)?;}else{
            let targets=run.source_target_refs.clone();
            crate::scope::with_selected_admission(store,assessment_id,scope_method(tool)?,None,&targets,"kali.mcp.dispatch",|reference|{run.scope_ref=Some(reference.clone());run.dispatch_before_unix_seconds=crate::scope::dispatch_deadline(store,reference,30)?;self.save(&run)})?;
        }
        dispatch.bind(&self.arbiter,&run)?;
        let cancel=Arc::new(AtomicBool::new(false));
        *active=Some(Active{run_id:run.id.clone(),assessment_id:assessment_id.into(),cancel:cancel.clone()});drop(active);
        let mut data_dispatched=false;
        let result=(||{
            if let Some(scope)=&run.scope_ref{crate::scope::recheck_selected(store,scope,scope_method(tool)?,&run.source_target_refs)?;}
            if let Some(data)=&data{self.recheck_kali_data(store,&data.prepared,&data.ai.data_redactor())?;}
            store.mutate(|s|{let a=s.assessments.iter_mut().find(|a|a.id==assessment_id).ok_or("Avaliação não encontrada.")?;a.events.push(model::event(format!("Sessão MCP {} solicitada: {} no Kali gerido, sem shell ou acesso a caminhos do alvo.",run.id,tool),"info"));Ok(())})?;
            let work_deadline=if let Some(scope)=&run.scope_ref{crate::scope::deadline(store,scope,Duration::from_secs(45))?}else{Instant::now()+Duration::from_secs(45)};
            if let Some(scope)=&run.scope_ref{crate::scope::recheck_selected(store,scope,scope_method(tool)?,&run.source_target_refs)?;}
            if let Some(data)=&data{self.recheck_kali_data(store,&data.prepared,&data.ai.data_redactor())?;}
            check_wait(work_deadline,&cancel)?;
            run.launch_attempted=true;self.save(&run)?;
            let mut transport=match self.launcher.launch(&run,&cancel){Ok(t)=>t,Err(message)=>{run.status=if cancel.load(Ordering::SeqCst){"canceled"}else{"failed"}.into();run.error=Some(message);run.cleanup_confirmed=true;run.finished_at=Some(model::now());self.save(&run)?;return Ok(json!({"run":run,"result":Value::Null,"cleanupConfirmed":true}));}};
            run.status="running".into();self.save(&run)?;
            let deadline=work_deadline;
            let mut call_state=ToolCallState::NotSent;
            let scope_check=run.scope_ref.as_ref().map(|scope|crate::scope::recheck_selected(store,scope,scope_method(tool)?,&run.source_target_refs)).unwrap_or(Ok(()));
            let mut observation=scope_check.and_then(|_|exchange_tracked(&mut *transport,&run,tool,&arguments,deadline,&cancel,&mut call_state,||{
                if let (Some(data),Some(receipt))=(&data,&data_receipt){
                    self.recheck_kali_data(store,&data.prepared,&data.ai.data_redactor())?;
                    let scope=run.scope_ref.as_ref().ok_or("scope_unreviewed")?;
                    self.data_policy.before_dispatch(store,receipt,&data.prepared.spec,Some(crate::data_policy::OperationalScope{assessment_id,method:scope_method(tool)?,expected:scope,targets:&run.source_target_refs}),||Ok(()))?;
                    data_dispatched=true;Ok(())
                }else{run.scope_ref.as_ref().map(|scope|crate::scope::recheck_selected(store,scope,scope_method(tool)?,&run.source_target_refs)).unwrap_or(Ok(()))}
            }));
            let finished=transport.finish(&run,Instant::now()+Duration::from_secs(5));
            let cleanup=finished.cleanup_confirmed;
            // Cancellation closes stdin; the fixed parser can still report its
            // bounded observations while stopping. Accept only a pending call's
            // unique validated reply after confirmed Linux cleanup.
            if observation.is_err()&&tool=="scan_bytes"&&cancel.load(Ordering::SeqCst)
                &&call_state==ToolCallState::Pending&&finished.tool_dispatched&&cleanup {
                if let Some(reply)=finished.tool_reply {
                    if let Ok(partial)=tool_response(reply,&run,tool,&arguments){observation=Ok(partial);}
                }
            }
            run.cleanup_confirmed=cleanup;
            run.finished_at=if cleanup{Some(model::now())}else{None};
            run.status=if !cleanup{"unknown"}else if cancel.load(Ordering::SeqCst){"canceled"}else if observation.is_ok(){"completed"}else{"failed"}.into();
            run.error=if !cleanup{Some("O cliente WSL encerrou ou perdeu comunicação sem comprovar término do processo Linux. Não repetir; use reconciliação.".into())}else{observation.as_ref().err().cloned()};
            let observed=observation.unwrap_or(Value::Null);
            let recorded=store.mutate(|s|{
                let combined=crate::scope_targets::is_combined(s,assessment_id);
                let a=s.assessments.iter_mut().find(|a|a.id==assessment_id).ok_or("Avaliação não encontrada.")?;
                // A reconciliation probe only confirms termination of prior
                // work. It has no resource scope and cannot become V2 evidence.
                if !(combined&&reconcile)&&(run.status=="completed"||(run.status=="canceled"&&cleanup&&tool=="scan_bytes"&&!observed.is_null())) {
                    let mut document=json!({"runId":run.id,"environmentId":environment_id,"sourceSha256":run.source_sha256,"sourceEvidenceId":run.source_evidence_id,"scopeRef":run.scope_ref,"protocolVersion":PROTOCOL,"tool":tool,"result":observed,"limits":"MCP diagnóstico/hash/análise lexical fixa de bytes selecionados; candidatos podem ser falsos positivos. Nenhuma execução do alvo, shell, rede ou acesso a seus caminhos."});
                    if combined{document["schema"]=json!("openkoda.kali-evidence.v2");document["targetRefs"]=json!(run.source_target_refs);}
                    let content=serde_json::to_string_pretty(&document).map_err(db_error)?;
                    let evidence=Evidence{id:model::id(),name:format!("kali-mcp-{}-{}.json",tool,run.id),sha256:hash(&content),content,created_at:model::now()};run.evidence_id=Some(evidence.id.clone());a.evidence.push(evidence);
                }
                a.events.push(model::event(format!("Sessão MCP {}: {}. Término Linux confirmado: {}.",run.id,run.status,cleanup),if run.status=="completed"{"success"}else{"warning"}));a.updated_at=model::now();Ok(())
            });
            if recorded.is_err(){run.status=if cleanup{"failed"}else{"unknown"}.into();run.evidence_id=None;run.error=Some("Resultado MCP não pôde ser registrado na avaliação. A chamada não será repetida.".into());}
            self.save(&run)?;
            Ok(json!({"run":run,"result":observed,"cleanupConfirmed":cleanup}))
        })();
        *self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?=None;
        if result.is_err(){
            if run.launch_attempted{run.status="unknown".into();run.error=Some("Registro interrompido; confirme o estado antes de repetir a sessão.".into());}
            else{run.status="failed".into();run.cleanup_confirmed=true;run.finished_at=Some(model::now());run.error=Some("Sessão recusada antes do despacho; nenhum processo Linux foi iniciado.".into());}
            let _=self.save(&run);
        }
        if let Some(receipt)=&data_receipt {
            let outcome=if !data_dispatched{crate::data_policy::DataUseOutcome::NotSent}else{match run.status.as_str(){"completed"=>crate::data_policy::DataUseOutcome::Completed,"unknown"=>crate::data_policy::DataUseOutcome::Unknown,"canceled"=>crate::data_policy::DataUseOutcome::Canceled,_=>crate::data_policy::DataUseOutcome::Failed}};
            self.data_policy.finish(store,receipt,outcome,vec![])?;
        }
        result
    }
}

trait Launcher:Send+Sync{fn launch(&self,run:&BridgeRun,cancel:&AtomicBool)->Result<Box<dyn Transport>,String>;}
struct FinishOutcome {cleanup_confirmed:bool,tool_dispatched:bool,tool_reply:Option<Value>}
#[derive(PartialEq,Eq)]
enum ToolCallState {NotSent,Pending,Replied}
#[derive(Default)]
struct DrainedReply {reply:Option<Value>,invalid:bool}
impl DrainedReply {
    fn observe(&mut self,value:&Value){
        if value.get("id").is_some(){
            if value["jsonrpc"]!="2.0"||value["id"]!=3||self.reply.is_some(){self.invalid=true;}
            else{self.reply=Some(value.clone());}
        }
    }
    fn finish(self)->Option<Value>{if self.invalid{None}else{self.reply}}
}
trait Transport:Send{
    fn send(&mut self,value:Value,deadline:Instant,cancel:&AtomicBool)->Result<(),String>;
    fn receive(&mut self,deadline:Instant,cancel:&AtomicBool)->Result<Value,String>;
    fn finish(&mut self,run:&BridgeRun,deadline:Instant)->FinishOutcome;
}
fn check_wait(deadline:Instant,cancel:&AtomicBool)->Result<(),String>{
    if cancel.load(Ordering::SeqCst){return Err("Cancelamento solicitado; aguardando término observado da sessão Linux.".into());}
    if Instant::now()>=deadline{return Err("Tempo limite da comunicação MCP atingido; nenhuma chamada foi repetida.".into());}Ok(())
}
fn metadata(run:&BridgeRun)->Value{json!({"environmentId":run.environment_id,"sessionId":run.id,"assessmentId":run.assessment_id})}
fn request(run:&BridgeRun,id:i64,method:&str,mut params:Value)->Value{params["_meta"]=json!({"openkoda":metadata(run)});json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})}
fn response(value:Value,id:i64)->Result<Value,String>{
    if value["jsonrpc"]!="2.0"||value["id"]!=id||value.get("error").is_some()||value.get("result").is_none(){return Err("Resposta MCP inválida ou incompatível; sessão interrompida.".into());}Ok(value["result"].clone())
}
fn expected_tools()->Value {
    let bytes=json!({"type":"object","properties":{"base64":{"type":"string"},"expectedSha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}},"required":["base64","expectedSha256"],"additionalProperties":false});
    json!([
        {"name":"runtime_diagnostics","inputSchema":{"type":"object","properties":{"inspectSessionId":{"type":"string"},"dispatchDeadline":{"type":"integer","minimum":1}},"additionalProperties":false}},
        {"name":"hash_bytes","inputSchema":bytes},
        {"name":"scan_bytes","inputSchema":bytes},
        {"name":"capture_collect","inputSchema":{"oneOf":[
            {"type":"object","properties":{"kind":{"const":"describe"},"producerSessionId":{"type":"string"},"captureId":{"type":"string"}},"required":["kind","producerSessionId","captureId"],"additionalProperties":false},
            {"type":"object","properties":{"kind":{"const":"read"},"producerSessionId":{"type":"string"},"captureId":{"type":"string"},"expectedManifestDigest":{"type":"string","pattern":"^[0-9a-f]{64}$"},"offset":{"type":"integer","minimum":0},"maxBytes":{"type":"integer","minimum":1,"maximum":4096}},"required":["kind","producerSessionId","captureId","expectedManifestDigest","offset","maxBytes"],"additionalProperties":false}]}},
        {"name":"capture_purge","inputSchema":{"type":"object","properties":{"producerSessionId":{"type":"string"},"captureId":{"type":"string"},"expectedManifestDigest":{"type":"string","pattern":"^[0-9a-f]{64}$"},"purgeReceiptId":{"type":"string"},"previousReceiptId":{"type":["string","null"]},"previousReceiptDigest":{"type":["string","null"],"pattern":"^[0-9a-f]{64}$"}},"required":["producerSessionId","captureId","expectedManifestDigest","purgeReceiptId","previousReceiptId","previousReceiptDigest"],"additionalProperties":false}},
        {"name":"capture_retire","inputSchema":retire_schema()}
    ])
}
fn retire_schema()->Value{
    let id=json!({"type":"string"});let sha=json!({"type":"string","pattern":"^[0-9a-f]{64}$"});let nullable_id=json!({"type":["string","null"]});let nullable_sha=json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"});
    let pending=json!({"type":["object","null"],"properties":{"retireReceiptId":id,"previousReceiptId":nullable_id,"previousReceiptDigest":nullable_sha,"reviewedStateDigest":sha,"producerRecordSha256":sha},"required":["retireReceiptId","previousReceiptId","previousReceiptDigest","reviewedStateDigest","producerRecordSha256"],"additionalProperties":false});
    let common=json!({"producerSessionId":id,"captureId":id,"ownerProfileFingerprint":sha,"producerDispatchDeadline":{"type":"integer","minimum":1},"expectedManifestDigest":sha,"purgeReceiptId":id,"pendingAttempt":pending});
    let base=["kind","producerSessionId","captureId","ownerProfileFingerprint","producerDispatchDeadline","expectedManifestDigest","purgeReceiptId","pendingAttempt","expectedPurgeReceiptSha256"];
    let mut variants=vec![];for kind in ["inspect","apply"]{let mut properties=common.clone();properties["kind"]=json!({"const":kind});properties["expectedPurgeReceiptSha256"]=if kind=="inspect"{nullable_sha.clone()}else{sha.clone()};let mut required=base.to_vec();if kind=="apply"{properties["retireReceiptId"]=id.clone();properties["previousReceiptId"]=nullable_id.clone();properties["previousReceiptDigest"]=nullable_sha.clone();properties["expectedStateDigest"]=sha.clone();required.extend(["retireReceiptId","previousReceiptId","previousReceiptDigest","expectedStateDigest"]);}variants.push(json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}));}json!({"oneOf":variants})
}
fn scan_result_valid(observed:&Value,args:&Value)->bool {
    let Some(raw)=args["base64"].as_str().and_then(|s|STANDARD.decode(s).ok()) else{return false;};
    let sha_valid=|v:&Value|v.as_str().is_some_and(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)));
    if observed["operation"]!="scan_bytes"||observed["adapter"]!="kali.rg.signals.v1"||observed["rulesVersion"]!="2026-11-07.1"||observed["inputSha256"]!=args["expectedSha256"]||observed["bytes"].as_u64()!=Some(raw.len() as u64)||observed["tool"]["name"]!="ripgrep"||!sha_valid(&observed["tool"]["binarySha256"]){return false;}
    if !observed["tool"]["version"].as_str().is_some_and(|s|s.starts_with("ripgrep ")&&s.len()<=128&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b" .()+_-".contains(&b))){return false;}
    let rules=[("private-key-marker","Private key marker"),("dynamic-evaluation","Dynamic evaluation signal"),("credential-identifier","Credential assignment identifier")];
    let Some(candidates)=observed["candidates"].as_array() else{return false;};
    let fields=["environmentId","sessionId","assessmentId","operation","adapter","rulesVersion","inputSha256","bytes","tool","candidates","counts","observedSignals","omittedCandidates","countsComplete","partial","termination","effects","limitations"];
    if observed.as_object().is_none_or(|o|o.len()!=fields.len()||o.keys().any(|k|!fields.contains(&k.as_str())))||observed["tool"].as_object().is_none_or(|o|o.len()!=3){return false;}
    if observed["effects"]!="Fixed offline parsing of selected bytes; no target execution, paths or network"||observed["limitations"]!="Lexical candidates can be false positives; absence is not proof of security"{return false;}
    if candidates.len()>16||observed["counts"].as_object().is_none_or(|o|o.len()!=3){return false;}
    let mut total=0u64;
    for (rule,label) in rules {
        let Some(count)=observed["counts"][rule].as_u64().filter(|n|*n<=32769) else{return false;};
        total+=count;
        if candidates.iter().filter(|c|c["ruleId"]==rule).count() as u64>count{return false;}
        for c in candidates.iter().filter(|c|c["ruleId"]==rule) {
            if c.as_object().is_none_or(|o|o.len()!=4)||c["label"]!=label||c["status"]!="candidate"||!c["line"].as_u64().is_some_and(|n|n>0&&n<=raw.iter().filter(|b|**b==b'\n').count() as u64+1){return false;}
        }
    }
    if candidates.iter().any(|c|!rules.iter().any(|(r,_)|c["ruleId"]==*r)){return false;}
    let Some(termination)=observed["termination"].as_str() else{return false;};
    if !["completed","canceled","timed_out","output_limited","parser_error"].contains(&termination){return false;}
    observed["observedSignals"].as_u64()==Some(total)&&observed["omittedCandidates"].as_u64()==Some(total-candidates.len() as u64)&&observed["countsComplete"].as_bool()==Some(termination=="completed")&&observed["partial"].as_bool()==Some(termination!="completed"||total>candidates.len() as u64)
}
#[cfg(test)]
fn exchange(transport:&mut dyn Transport,run:&BridgeRun,tool:&str,args:&Value,deadline:Instant,cancel:&AtomicBool)->Result<Value,String>{
    exchange_tracked(transport,run,tool,args,deadline,cancel,&mut ToolCallState::NotSent,||Ok(()))
}
fn exchange_tracked(transport:&mut dyn Transport,run:&BridgeRun,tool:&str,args:&Value,deadline:Instant,cancel:&AtomicBool,call_state:&mut ToolCallState,before_tool:impl FnOnce()->Result<(),String>)->Result<Value,String>{
    negotiate(transport,run,deadline,cancel)?;
    // Negotiation can wait while another thread withdraws the scope. Check
    // again at the data-bearing boundary, before marking a tool call pending.
    before_tool()?;
    check_wait(deadline,cancel)?;
    *call_state=ToolCallState::Pending;
    transport.send(request(run,3,"tools/call",json!({"name":tool,"arguments":args})),deadline,cancel)?;
    let reply=transport.receive(deadline,cancel)?;
    *call_state=ToolCallState::Replied;
    tool_response(reply,run,tool,args)
}
fn negotiate(transport:&mut dyn Transport,run:&BridgeRun,deadline:Instant,cancel:&AtomicBool)->Result<(),String>{
    transport.send(request(run,1,"initialize",json!({"protocolVersion":PROTOCOL,"capabilities":{},"clientInfo":{"name":"OpenKoda","version":env!("CARGO_PKG_VERSION")}})),deadline,cancel)?;
    let hello=response(transport.receive(deadline,cancel)?,1)?;
    if hello["protocolVersion"]!=PROTOCOL||hello["serverInfo"]["name"]!="openkoda-kali-bounded"||hello["_meta"]["openkoda"]!=metadata(run)||hello["_meta"]["executorSourceSha256"]!=run.source_sha256||hello["_meta"]["catalogVersion"]!=queued::KALI_CATALOG_VERSION||hello["_meta"]["catalogSha256"]!=catalog_digest() {return Err("Identidade, protocolo, catálogo ou código do executor MCP não corresponde ao esperado.".into());}
    transport.send(json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{"_meta":{"openkoda":metadata(run)}}}),deadline,cancel)?;
    transport.send(request(run,2,"tools/list",json!({})),deadline,cancel)?;
    let catalog=response(transport.receive(deadline,cancel)?,2)?;
    let tools=catalog["tools"].as_array().ok_or("Catálogo MCP inválido.")?;
    let expected=expected_tools();let expected=expected.as_array().unwrap();
    if tools.len()!=expected.len()||tools.iter().zip(expected).any(|(actual,wanted)|actual["name"]!=wanted["name"]||actual["inputSchema"]!=wanted["inputSchema"]){return Err("O catálogo ou esquema MCP mudou; ferramentas não autorizadas foram recusadas.".into());}
    Ok(())
}
fn tool_response(reply:Value,run:&BridgeRun,tool:&str,args:&Value)->Result<Value,String>{
    let answer=response(reply,3)?;
    if answer["isError"]!=false{return Err("A ferramenta MCP recusou os parâmetros ou a identidade; nenhuma chamada foi repetida.".into());}
    let observed=answer["structuredContent"].clone();
    if observed["environmentId"]!=run.environment_id||observed["sessionId"]!=run.id||observed["assessmentId"]!=run.assessment_id {return Err("Resultado MCP pertence a outro ambiente ou sessão.".into());}
    match tool {
        "runtime_diagnostics"=>if observed["user"]!="openkoda"||!observed["uid"].as_u64().is_some_and(|id|id>0)||observed["markerVerified"]!=true||observed["executorSourceSha256"]!=run.source_sha256||observed["interopEnabled"]!=false||observed["windowsMounts"]!=json!([]){return Err("O processo MCP não comprovou usuário dedicado, código e condições do ambiente esperados.".into());},
        "hash_bytes"=>{
            let bytes=STANDARD.decode(args["base64"].as_str().ok_or("Entrada de hash inválida.")?).map_err(|_|"Entrada de hash inválida.")?;
            if observed["sha256"]!=args["expectedSha256"]||observed["bytes"].as_u64()!=Some(bytes.len() as u64)||observed["operation"]!="hash_bytes"{return Err("O hash retornado não corresponde aos bytes selecionados no host.".into());}
        }
        "scan_bytes"=>if !scan_result_valid(&observed,args){return Err("O analisador Kali não comprovou os bytes, a ferramenta ou o esquema limitado de candidatos esperado.".into());},
        _=>return Err("Ferramenta não permitida na ponte.".into())
    }Ok(observed)
}

struct SystemLauncher;
fn catalog_digest()->String{hash(&expected_tools().to_string())}
fn bootstrap(run:&BridgeRun)->Result<Value,String>{
    if SOURCE.len()>128*1024{return Err("Código do executor excede o envelope fixo.".into());}
    let mut encoder=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::default());
    encoder.write_all(SOURCE.as_bytes()).map_err(db_error)?;
    let compressed=encoder.finish().map_err(db_error)?;
    let(mode,capture,control)=capture_io::bootstrap_context(run)?;
    let envelope=json!({"version":"openkoda.executor-bootstrap.v2","sourceZlibBase64":STANDARD.encode(compressed),"sourceBytes":SOURCE.len(),"sourceSha256":hash(SOURCE),"environmentId":run.environment_id,"sessionId":run.id,"assessmentId":run.assessment_id,"mode":mode,"capture":capture,"control":control});
    if serde_json::to_vec(&envelope).map_err(db_error)?.len()+1>65536{return Err("Envelope do executor excede o transporte fixo.".into());}
    Ok(envelope)
}
enum Frame{Message(Value),End,Error}
type WriteTask=(Vec<u8>,mpsc::SyncSender<bool>);
struct ProcessTransport{child:Child,writer:Option<mpsc::Sender<WriteTask>>,frames:mpsc::Receiver<Frame>,saw_closed:bool,tool_dispatched:bool}
fn system_binary()->PathBuf{PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(||"C:\\Windows".into())).join("System32/wsl.exe")}
impl Launcher for SystemLauncher {
    fn launch(&self,run:&BridgeRun,cancel:&AtomicBool)->Result<Box<dyn Transport>,String>{
        check_wait(Instant::now()+Duration::from_secs(1),cancel)?;
        let bootstrap=bootstrap(run)?;
        let mut command=Command::new(system_binary());
        command.args(["--distribution","OpenKoda-Kali","--user","openkoda","--cd","/","--exec","/usr/bin/python3","-I","-c",BOOT,"--openkoda-session",&run.id,"--openkoda-deadline",&run.dispatch_before_unix_seconds.to_string()]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).env_clear();
        for name in ["SystemRoot","WINDIR","USERPROFILE","LOCALAPPDATA","APPDATA","TEMP","TMP"]{if let Some(value)=std::env::var_os(name){command.env(name,value);}}
        command.env("PATH",system_binary().parent().ok_or("Caminho do WSL inválido.")?).env("WSLENV","");
        #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        let mut child=command.spawn().map_err(|_|"Não foi possível iniciar o cliente WSL da distro gerida.")?;
        let mut stdin=child.stdin.take().ok_or("Entrada do cliente WSL indisponível.")?;
        let stdout=child.stdout.take().ok_or("Saída do cliente WSL indisponível.")?;
        let mut stderr=child.stderr.take().ok_or("Diagnóstico do cliente WSL indisponível.")?;
        let (writes,write_queue)=mpsc::channel::<WriteTask>();
        std::thread::spawn(move||{while let Ok((bytes,ack))=write_queue.recv(){let ok=stdin.write_all(&bytes).and_then(|_|stdin.flush()).is_ok();let _=ack.send(ok);if !ok{break;}}});
        let frames=read_frames_bounded(stdout,if run.tool=="capture_collect"{128*1024}else{MAX_OUTPUT});
        std::thread::spawn(move||{let mut buffer=[0u8;4096];let mut total=0;while total<MAX_OUTPUT {match stderr.read(&mut buffer){Ok(0)|Err(_)=>break,Ok(n)=>total+=n}}});
        let mut transport=ProcessTransport{child,writer:Some(writes),frames,saw_closed:false,tool_dispatched:false};
        // A failed write can still leave Linux alive: return the transport and let
        // normal handshake/cleanup observation determine the durable outcome.
        let _=transport.send(bootstrap,Instant::now()+Duration::from_secs(20),cancel);
        Ok(Box::new(transport))
    }
}
fn read_frames(reader:impl Read+Send+'static)->mpsc::Receiver<Frame>{
    read_frames_bounded(reader,MAX_OUTPUT)
}
fn read_frames_bounded(mut reader:impl Read+Send+'static,limit:usize)->mpsc::Receiver<Frame>{
    let (tx,rx)=mpsc::channel();
    std::thread::spawn(move||{
        let mut buffer=[0u8;4096];let mut line=Vec::new();let mut total=0;
        loop {let n=match reader.read(&mut buffer){Ok(0)=>{let _=tx.send(if line.is_empty(){Frame::End}else{Frame::Error});break;},Ok(n)=>n,Err(_)=>{let _=tx.send(Frame::Error);break;}};
            total+=n;if total>limit{let _=tx.send(Frame::Error);break;}
            for byte in &buffer[..n] {if *byte==b'\n' {match capture_io::retire::strict_json::parse(&line){Ok(v)=>{if tx.send(Frame::Message(v)).is_err(){return;}},Err(_)=>{let _=tx.send(Frame::Error);return;}}line.clear();}else{line.push(*byte);if line.len()>MAX_FRAME{let _=tx.send(Frame::Error);return;}}}
        }
    });rx
}
impl Transport for ProcessTransport {
    fn send(&mut self,value:Value,deadline:Instant,cancel:&AtomicBool)->Result<(),String>{
        let mut bytes=serde_json::to_vec(&value).map_err(|_|"Mensagem MCP inválida.")?;bytes.push(b'\n');
        if bytes.len()>65536{return Err("Mensagem MCP excedeu o limite de entrada.".into());}
        check_wait(deadline,cancel)?;let (tx,rx)=mpsc::sync_channel(1);
        self.writer.as_ref().ok_or("Entrada MCP fechada.")?.send((bytes,tx)).map_err(|_|"Entrada MCP interrompida.")?;
        if value["id"]==3&&value["method"]=="tools/call"{self.tool_dispatched=true;}
        loop {check_wait(deadline,cancel)?;match rx.recv_timeout(Duration::from_millis(50)){Ok(true)=>return Ok(()),Ok(false)|Err(mpsc::RecvTimeoutError::Disconnected)=>return Err("Não foi possível enviar a mensagem MCP.".into()),Err(mpsc::RecvTimeoutError::Timeout)=>{}}}
    }
    fn receive(&mut self,deadline:Instant,cancel:&AtomicBool)->Result<Value,String>{
        loop {check_wait(deadline,cancel)?;match self.frames.recv_timeout(Duration::from_millis(50)){Ok(Frame::Message(value))=>return Ok(value),Ok(Frame::End)|Err(mpsc::RecvTimeoutError::Disconnected)=>return Err("Sessão MCP encerrou antes da resposta.".into()),Ok(Frame::Error)=>return Err("Saída MCP inválida ou acima do limite.".into()),Err(mpsc::RecvTimeoutError::Timeout)=>{}}}
    }
    fn finish(&mut self,run:&BridgeRun,deadline:Instant)->FinishOutcome{
        self.writer.take();let mut end=false;let mut drained=DrainedReply::default();
        loop {
            match self.frames.recv_timeout(Duration::from_millis(25)){
                Ok(Frame::Message(value))=>{drained.observe(&value);if closed_notification(&value,run){self.saw_closed=true;}},
                Ok(Frame::End)=>end=true,Ok(Frame::Error)=>return FinishOutcome{cleanup_confirmed:false,tool_dispatched:self.tool_dispatched,tool_reply:None},
                Err(mpsc::RecvTimeoutError::Disconnected)=>end=true,Err(mpsc::RecvTimeoutError::Timeout)=>{}
            }
            if self.child.try_wait().ok().flatten().is_some()&&end{return FinishOutcome{cleanup_confirmed:self.saw_closed,tool_dispatched:self.tool_dispatched,tool_reply:drained.finish()};}
            if Instant::now()>=deadline{let _=self.child.kill();return FinishOutcome{cleanup_confirmed:false,tool_dispatched:self.tool_dispatched,tool_reply:None};}
        }
    }
}
fn closed_notification(value:&Value,run:&BridgeRun)->bool{
    value["jsonrpc"]=="2.0"&&value["method"]=="notifications/openkoda/closed"&&value["params"]["environmentId"]==run.environment_id&&value["params"]["sessionId"]==run.id&&value["params"]["assessmentId"]==run.assessment_id&&value["params"]["executorSourceSha256"]==run.source_sha256&&value["params"]["childrenQuiescent"]==true
}
impl Drop for ProcessTransport{fn drop(&mut self){self.writer.take();if self.child.try_wait().ok().flatten().is_none(){let _=self.child.kill();}}}

#[cfg(test)]
pub(crate) use tests::queued_tests::completed_scan_fixture;
#[cfg(test)]
mod tests{
    use super::*;
    use std::collections::VecDeque;
    mod work_tests {use super::*;include!("kali_work_tests.rs");}
    pub(crate) mod queued_tests {use super::*;include!("kali_queued_tests.rs");}
    struct Fake{frames:VecDeque<Value>,sent:Vec<Value>,clean:bool}
    impl Transport for Fake{
        fn send(&mut self,v:Value,d:Instant,c:&AtomicBool)->Result<(),String>{check_wait(d,c)?;self.sent.push(v);Ok(())}
        fn receive(&mut self,d:Instant,c:&AtomicBool)->Result<Value,String>{check_wait(d,c)?;self.frames.pop_front().ok_or("fixture disconnected".into())}
        fn finish(&mut self,_:&BridgeRun,_:Instant)->FinishOutcome{let mut drained=DrainedReply::default();for reply in &self.frames{drained.observe(reply);}FinishOutcome{cleanup_confirmed:self.clean,tool_dispatched:self.sent.iter().any(|v|v["method"]=="tools/call"&&v["id"]==3),tool_reply:drained.finish()}}
    }
    fn run()->BridgeRun{BridgeRun{scope_ref:None,source_target_refs:vec![],reconciliation_of:None,work_context:None,id:model::id(),assessment_id:model::id(),environment_id:model::id(),tool:"runtime_diagnostics".into(),status:"running".into(),started_at:model::now(),finished_at:None,source_sha256:hash(SOURCE),evidence_id:None,error:None,cleanup_confirmed:false,launch_attempted:true,dispatch_before_unix_seconds:(chrono::Utc::now().timestamp()+30) as u64,source_evidence_id:None}}
    fn frames(r:&BridgeRun,result:Value)->VecDeque<Value>{VecDeque::from([
        json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":PROTOCOL,"serverInfo":{"name":"openkoda-kali-bounded"},"_meta":{"openkoda":metadata(r),"executorSourceSha256":r.source_sha256,"catalogVersion":queued::KALI_CATALOG_VERSION,"catalogSha256":catalog_digest()}}}),
        json!({"jsonrpc":"2.0","id":2,"result":{"tools":expected_tools()}}),
        json!({"jsonrpc":"2.0","id":3,"result":{"isError":false,"structuredContent":result}})])}
    fn diagnostic(r:&BridgeRun)->Value{json!({"environmentId":r.environment_id,"sessionId":r.id,"assessmentId":r.assessment_id,"uid":1000,"user":"openkoda","markerVerified":true,"executorSourceSha256":r.source_sha256,"interopEnabled":false,"windowsMounts":[]})}
    fn scanned(meta:&Value,args:&Value)->Value {
        json!({"environmentId":meta["environmentId"],"sessionId":meta["sessionId"],"assessmentId":meta["assessmentId"],
            "operation":"scan_bytes","adapter":"kali.rg.signals.v1","rulesVersion":"2026-11-07.1","inputSha256":args["expectedSha256"],"bytes":STANDARD.decode(args["base64"].as_str().unwrap()).unwrap().len(),
            "tool":{"name":"ripgrep","version":"ripgrep 15.2.0","binarySha256":"a".repeat(64)},
            "candidates":[{"ruleId":"dynamic-evaluation","line":1,"label":"Dynamic evaluation signal","status":"candidate"}],
            "counts":{"private-key-marker":0,"dynamic-evaluation":1,"credential-identifier":0},"observedSignals":1,"omittedCandidates":0,
            "countsComplete":true,"partial":false,"termination":"completed","effects":"Fixed offline parsing of selected bytes; no target execution, paths or network",
            "limitations":"Lexical candidates can be false positives; absence is not proof of security"})
    }
    #[test]fn handshake_is_ordered_and_binds_runtime_identity(){let r=run();let mut t=Fake{frames:frames(&r,diagnostic(&r)),sent:vec![],clean:true};let v=exchange(&mut t,&r,"runtime_diagnostics",&json!({}),Instant::now()+Duration::from_secs(1),&AtomicBool::new(false)).unwrap();assert_eq!(v["sessionId"],r.id);assert_eq!(t.sent.iter().map(|v|v["method"].as_str().unwrap()).collect::<Vec<_>>(),["initialize","notifications/initialized","tools/list","tools/call"]);}
    #[test]fn forged_session_and_unexpected_catalog_are_refused(){let r=run();let mut wrong=frames(&r,diagnostic(&r));wrong[0]["result"]["_meta"]["openkoda"]["sessionId"]=json!(model::id());let mut t=Fake{frames:wrong,sent:vec![],clean:true};assert!(exchange(&mut t,&r,"runtime_diagnostics",&json!({}),Instant::now()+Duration::from_secs(1),&AtomicBool::new(false)).is_err());assert_eq!(t.sent.len(),1);let mut bad=frames(&r,diagnostic(&r));bad[1]["result"]["tools"][0]["name"]=json!("shell");let mut t=Fake{frames:bad,sent:vec![],clean:true};assert!(exchange(&mut t,&r,"runtime_diagnostics",&json!({}),Instant::now()+Duration::from_secs(1),&AtomicBool::new(false)).is_err());assert_eq!(t.sent.len(),3);}
    #[test]fn hash_is_checked_against_host_selected_bytes(){let r=run();let args=json!({"base64":STANDARD.encode(b"fixture"),"expectedSha256":hash("fixture")});let mut result=metadata(&r);result["operation"]=json!("hash_bytes");result["sha256"]=json!("forged");result["bytes"]=json!(7);let mut t=Fake{frames:frames(&r,result),sent:vec![],clean:true};assert!(exchange(&mut t,&r,"hash_bytes",&args,Instant::now()+Duration::from_secs(1),&AtomicBool::new(false)).is_err());}
    #[test]fn cancellation_and_timeout_stop_before_sending(){let r=run();for (deadline,cancel) in [(Instant::now()+Duration::from_secs(1),true),(Instant::now(),false)]{let mut t=Fake{frames:frames(&r,diagnostic(&r)),sent:vec![],clean:true};assert!(exchange(&mut t,&r,"runtime_diagnostics",&json!({}),deadline,&AtomicBool::new(cancel)).is_err());assert!(t.sent.is_empty());}}
    #[test]fn frame_reader_rejects_oversized_and_malformed_output(){for bytes in [vec![b'x';MAX_FRAME+1],b"{invalid}\n".to_vec()]{let rx=read_frames(std::io::Cursor::new(bytes));assert!(matches!(rx.recv_timeout(Duration::from_secs(1)).unwrap(),Frame::Error));}}
    #[test]fn restarting_marks_pending_session_unknown(){let dir=tempfile::tempdir().unwrap();let b=KaliBridge::new(dir.path().into()).unwrap();let r=run();b.save(&r).unwrap();drop(b);let reopened=KaliBridge::new(dir.path().into()).unwrap();assert_eq!(reopened.find(&r.id).unwrap().status,"unknown");assert!(reopened.has_unresolved());}
    struct TestLauncher{launches:std::sync::atomic::AtomicUsize,clean:bool,block:bool,started:Option<mpsc::Sender<()>>}
    struct TestTransport{inner:Fake,block:bool}
    impl Launcher for TestLauncher{
        fn launch(&self,r:&BridgeRun,_:&AtomicBool)->Result<Box<dyn Transport>,String>{
            self.launches.fetch_add(1,Ordering::SeqCst);if let Some(started)=&self.started{let _=started.send(());}
            Ok(Box::new(TestTransport{inner:Fake{frames:frames(r,diagnostic(r)),sent:vec![],clean:self.clean},block:self.block}))
        }
    }
    impl Transport for TestTransport{
        fn send(&mut self,v:Value,d:Instant,c:&AtomicBool)->Result<(),String>{
            if v["method"]=="tools/call"&&v["params"]["name"]=="scan_bytes" {
                self.inner.frames.front_mut().unwrap()["result"]["structuredContent"]=scanned(&v["params"]["_meta"]["openkoda"],&v["params"]["arguments"]);
            }
            if v["method"]=="tools/call"&&v["params"]["name"]=="hash_bytes" {
                let mut result=v["params"]["_meta"]["openkoda"].clone();
                result["operation"]=json!("hash_bytes");result["sha256"]=v["params"]["arguments"]["expectedSha256"].clone();
                result["bytes"]=json!(STANDARD.decode(v["params"]["arguments"]["base64"].as_str().unwrap()).unwrap().len());
                self.inner.frames.front_mut().unwrap()["result"]["structuredContent"]=result;
            }
            if v["method"]=="tools/call"&&v["params"]["arguments"]["inspectSessionId"].is_string(){
                self.inner.frames.front_mut().unwrap()["result"]["structuredContent"]["inspectedSession"]=json!({"sessionId":v["params"]["arguments"]["inspectSessionId"],"known":true,"active":false,"reason":"process_absent"});
            }self.inner.send(v,d,c)
        }
        fn receive(&mut self,d:Instant,c:&AtomicBool)->Result<Value,String>{if self.block{loop{check_wait(d,c)?;std::thread::sleep(Duration::from_millis(5));}}self.inner.receive(d,c)}
        fn finish(&mut self,r:&BridgeRun,d:Instant)->FinishOutcome{self.inner.finish(r,d)}
    }
    #[derive(Clone,Copy,Debug,PartialEq,Eq)]
    enum CancelReplyKind {Valid,Malformed,Foreign,WrongId,Duplicate,MissingCleanup,BeforeTool}
    struct CancelScanLauncher {kind:CancelReplyKind}
    struct CancelScanTransport {inner:Fake,kind:CancelReplyKind,run:BridgeRun}
    impl Launcher for CancelScanLauncher {
        fn launch(&self,r:&BridgeRun,_:&AtomicBool)->Result<Box<dyn Transport>,String>{
            Ok(Box::new(CancelScanTransport{inner:Fake{frames:frames(r,diagnostic(r)),sent:vec![],clean:true},kind:self.kind,run:r.clone()}))
        }
    }
    impl Transport for CancelScanTransport {
        fn send(&mut self,v:Value,d:Instant,c:&AtomicBool)->Result<(),String>{
            if v["method"]=="tools/call" {
                let mut partial=scanned(&v["params"]["_meta"]["openkoda"],&v["params"]["arguments"]);
                partial["termination"]=json!("canceled");partial["partial"]=json!(true);partial["countsComplete"]=json!(false);
                if self.kind==CancelReplyKind::Malformed{partial["rawSource"]=json!("SENTINEL-must-not-be-persisted");}
                if self.kind==CancelReplyKind::Foreign{partial["assessmentId"]=json!(model::id());}
                let reply=json!({"jsonrpc":"2.0","id":if self.kind==CancelReplyKind::WrongId{4}else{3},"result":{"isError":false,"structuredContent":partial}});
                self.inner.frames=VecDeque::from([reply.clone()]);
                if self.kind==CancelReplyKind::Duplicate{self.inner.frames.push_back(reply);}
                let mut closed=metadata(&self.run);closed["executorSourceSha256"]=json!(self.run.source_sha256);closed["childrenQuiescent"]=json!(true);
                if self.kind==CancelReplyKind::MissingCleanup{closed["sessionId"]=json!(model::id());}
                self.inner.frames.push_back(json!({"jsonrpc":"2.0","method":"notifications/openkoda/closed","params":closed}));
                if self.kind==CancelReplyKind::BeforeTool{c.store(true,Ordering::SeqCst);}
                self.inner.send(v,d,c)?;
                c.store(true,Ordering::SeqCst);return Ok(());
            }
            self.inner.send(v,d,c)
        }
        fn receive(&mut self,d:Instant,c:&AtomicBool)->Result<Value,String>{self.inner.receive(d,c)}
        fn finish(&mut self,r:&BridgeRun,_:Instant)->FinishOutcome{
            let mut drained=DrainedReply::default();let mut closed=false;
            for reply in &self.inner.frames{drained.observe(reply);closed|=closed_notification(reply,r);}
            FinishOutcome{cleanup_confirmed:closed,tool_dispatched:self.inner.sent.iter().any(|v|v["id"]==3&&v["method"]=="tools/call"),tool_reply:drained.finish()}
        }
    }
    fn setup_store(root:&std::path::Path)->(Arc<Store>,String){
        setup_scoped_store(root,&["kali.runtime-diagnostics.v1","kali.hash-evidence.v1","kali.rg.signals.v1"],json!({"startsAt":null,"endsAt":null}))
    }
    fn test_scan(bridge:&KaliBridge,mut payload:Value,store:&Store,environment:&str)->Result<Value,String>{
        let ai=crate::ai::AiService::new(store.root.clone())?.with_data_policy(bridge.data_policy.clone());
        let redactor=ai.data_redactor();
        let aid=payload["assessmentId"].as_str().ok_or("fixture assessment")?;
        crate::data_policy::test_support::authorize(store,&bridge.data_policy,aid,&redactor,&crate::data_projection::RejectExternal,None)?;
        let preview=bridge.handle_data_prepare("kali.data.prepare",json!({"assessmentId":aid,"evidenceId":payload["evidenceId"],"operation":"scan","packageSensitivity":"internal"}),store,environment,&redactor)?;
        payload["dataReview"]=preview["dataReview"].clone();
        bridge.handle_reviewed("kali.mcp.scanEvidence",payload,store,environment,&ai)
    }
    fn setup_scoped_store(root:&std::path::Path,methods:&[&str],window:Value)->(Arc<Store>,String){
        let store=Arc::new(Store::open(root.join("state")).unwrap());
        let p=store.handle(crate::core::Request{action:"project.create".into(),payload:json!({"name":"MCP fixture","kind":"web","target":"http://127.0.0.1:9"})}).unwrap();
        let mut scope=crate::scope::test_creation(&store,p["id"].as_str().unwrap(),methods);scope["window"]=window;
        let a=store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"kind":"web","target":p["target"],"title":"MCP fixture","objective":"No external target","scope":scope})}).unwrap();
        (store,a["id"].as_str().unwrap().into())
    }
    #[test]fn scope_denies_identity_and_service_dispatch_without_launch_or_journal(){
        for variant in ["method","future","expired","revoked"]{
            let dir=tempfile::tempdir().unwrap();let mut window=json!({"startsAt":null,"endsAt":null});
            if variant=="future"{window["startsAt"]=json!((chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339());}
            if variant=="expired"{window["endsAt"]=json!((chrono::Utc::now()-chrono::Duration::hours(1)).to_rfc3339());}
            let methods=if variant=="method"{vec![]}else{vec!["kali.runtime-diagnostics.v1"]};let(store,assessment)=setup_scoped_store(dir.path(),&methods,window);
            let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();
            if variant=="revoked"{crate::scope::admit(&store,&assessment,"kali.runtime-diagnostics.v1",None,"fixture.preparation").unwrap();}
            if variant=="revoked"{let status=crate::scope::status(&store,&assessment).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":assessment,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"rationale":"Synthetic withdrawal","acknowledgeRevocation":true}),&store,str::to_owned).unwrap();}
            let before=serde_json::to_value(store.snapshot().unwrap()).unwrap();let payload=json!({"assessmentId":assessment});
            assert!(admit_before_identity("kali.mcp.diagnose",&payload,&store).unwrap_err().starts_with("scope_"));
            assert!(bridge.handle("kali.mcp.diagnose",payload,&store,&model::id()).unwrap_err().starts_with("scope_"));
            assert_eq!(launcher.launches.load(Ordering::SeqCst),0);assert!(bridge.runs().unwrap().is_empty());assert_eq!(serde_json::to_value(store.snapshot().unwrap()).unwrap(),before);
        }
    }
    #[test]fn scope_journal_is_visible_before_launch_and_revocation_blocks_tool_exchange(){
        struct GateLauncher{store:Arc<Store>,started:mpsc::Sender<()>,release:Mutex<mpsc::Receiver<()>>,sends:Arc<std::sync::atomic::AtomicUsize>}
        struct GateTransport{fake:Fake,sends:Arc<std::sync::atomic::AtomicUsize>}
        impl Transport for GateTransport{
            fn send(&mut self,v:Value,d:Instant,c:&AtomicBool)->Result<(),String>{self.sends.fetch_add(1,Ordering::SeqCst);self.fake.send(v,d,c)}
            fn receive(&mut self,d:Instant,c:&AtomicBool)->Result<Value,String>{self.fake.receive(d,c)}
            fn finish(&mut self,r:&BridgeRun,d:Instant)->FinishOutcome{self.fake.finish(r,d)}
        }
        impl Launcher for GateLauncher{fn launch(&self,r:&BridgeRun,_:&AtomicBool)->Result<Box<dyn Transport>,String>{
            let status=crate::scope::status(&self.store,&r.assessment_id).unwrap();assert_eq!(status["phase"],"frozen");assert!(r.scope_ref.is_some());
            assert!(crate::scope::admitted_work(&self.store,&r.assessment_id).unwrap().iter().any(|w|w["id"]==r.id));
            self.started.send(()).unwrap();self.release.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(Box::new(GateTransport{fake:Fake{frames:frames(r,diagnostic(r)),sent:vec![],clean:true},sends:self.sends.clone()}))
        }}
        let dir=tempfile::tempdir().unwrap();let(store,assessment)=setup_store(dir.path());let(started,ready)=mpsc::channel();let(release,wait)=mpsc::channel();let sends=Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let bridge=KaliBridge::with_launcher(store.root.clone(),Arc::new(GateLauncher{store:store.clone(),started,release:Mutex::new(wait),sends:sends.clone()})).unwrap();let environment=model::id();
        std::thread::scope(|threads|{let task=threads.spawn(||bridge.handle("kali.mcp.diagnose",json!({"assessmentId":assessment}),&store,&environment));ready.recv_timeout(Duration::from_secs(10)).unwrap();
            let status=crate::scope::status(&store,&assessment).unwrap();let revoked=crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":assessment,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"rationale":"Stop before tool exchange","acknowledgeRevocation":true}),&store,str::to_owned).unwrap();assert!(!revoked["admittedWork"].as_array().unwrap().is_empty());
            release.send(()).unwrap();let result=task.join().unwrap().unwrap();assert_eq!(result["run"]["status"],"failed");assert_eq!(result["cleanupConfirmed"],true);
        });assert_eq!(sends.load(Ordering::SeqCst),0);assert!(!bridge.has_unresolved());assert!(store.snapshot().unwrap().assessments[0].evidence.is_empty());
    }
    #[test]fn revocation_during_catalog_handshake_prevents_evidence_transfer(){
        struct CatalogLauncher{ready:mpsc::Sender<()>,release:Arc<Mutex<mpsc::Receiver<()>>>,calls:Arc<std::sync::atomic::AtomicUsize>}
        struct CatalogTransport{fake:Fake,ready:mpsc::Sender<()>,release:Arc<Mutex<mpsc::Receiver<()>>>,calls:Arc<std::sync::atomic::AtomicUsize>}
        impl Launcher for CatalogLauncher{fn launch(&self,r:&BridgeRun,_:&AtomicBool)->Result<Box<dyn Transport>,String>{Ok(Box::new(CatalogTransport{fake:Fake{frames:frames(r,diagnostic(r)),sent:vec![],clean:true},ready:self.ready.clone(),release:self.release.clone(),calls:self.calls.clone()}))}}
        impl Transport for CatalogTransport{
            fn send(&mut self,v:Value,d:Instant,c:&AtomicBool)->Result<(),String>{if v["method"]=="tools/call"{self.calls.fetch_add(1,Ordering::SeqCst);}self.fake.send(v,d,c)}
            fn receive(&mut self,d:Instant,c:&AtomicBool)->Result<Value,String>{if self.fake.frames.front().is_some_and(|v|v["id"]==2){self.ready.send(()).unwrap();self.release.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();}self.fake.receive(d,c)}
            fn finish(&mut self,r:&BridgeRun,d:Instant)->FinishOutcome{self.fake.finish(r,d)}
        }
        for revocation in ["scope","data_policy"] {
        let dir=tempfile::tempdir().unwrap();let(store,assessment)=setup_store(dir.path());
        let evidence=store.handle(crate::core::Request{action:"evidence.import".into(),payload:json!({"assessmentId":assessment,"name":"synthetic.txt","sourceDescription":"Owned fixture bytes","content":"synthetic selected evidence","acknowledgeUnverifiedSource":true})}).unwrap();
        let(ready,arrived)=mpsc::channel();let(release,wait)=mpsc::channel();let calls=Arc::new(std::sync::atomic::AtomicUsize::new(0));let environment=model::id();
        let bridge=KaliBridge::with_launcher(store.root.clone(),Arc::new(CatalogLauncher{ready,release:Arc::new(Mutex::new(wait)),calls:calls.clone()})).unwrap();
        std::thread::scope(|threads|{let task=threads.spawn(||test_scan(&bridge,json!({"assessmentId":assessment,"evidenceId":evidence["id"],"acknowledgeLocalTransfer":true}),&store,&environment));arrived.recv_timeout(Duration::from_secs(10)).unwrap();
            if revocation=="scope" {
                let status=crate::scope::status(&store,&assessment).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":assessment,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"rationale":"Withdraw while tools/list is pending","acknowledgeRevocation":true}),&store,str::to_owned).unwrap();
            } else {
                let ai=crate::ai::AiService::new(store.root.clone()).unwrap();let redactor=ai.data_redactor();let pid=store.snapshot().unwrap().assessments[0].project_id.clone();
                let status=bridge.data_policy.handle("data.policy.status",json!({"projectId":pid}),&store,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
                bridge.data_policy.handle("data.policy.revoke",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"acknowledgeRevocation":true}),&store,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
            }
            release.send(()).unwrap();let result=task.join().unwrap().unwrap();assert_eq!(result["run"]["status"],"failed");assert_eq!(result["cleanupConfirmed"],true);
        });assert_eq!(calls.load(Ordering::SeqCst),0,"selected evidence must never be sent after observed {revocation} revocation");assert!(!bridge.has_unresolved());assert_eq!(store.snapshot().unwrap().assessments[0].evidence.len(),1);
        assert_eq!(store.snapshot().unwrap().data_use_receipts[0].events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::NotSent);
        }
    }
    #[test]fn complete_diagnostic_is_journaled_without_promoting_assessment(){
        let dir=tempfile::tempdir().unwrap();let (store,assessment)=setup_store(dir.path());
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(dir.path().into(),launcher.clone()).unwrap();
        let response=bridge.handle("kali.mcp.diagnose",json!({"assessmentId":assessment}),&store,&model::id()).unwrap();
        assert_eq!(response["run"]["status"],"completed");assert_eq!(response["cleanupConfirmed"],true);assert!(!bridge.has_unresolved());assert_eq!(launcher.launches.load(Ordering::SeqCst),1);
        let state=store.snapshot().unwrap();assert_eq!(state.assessments[0].status,"draft");assert_eq!(state.assessments[0].evidence.len(),1);assert!(state.assessments[0].evidence[0].content.contains("runtime_diagnostics"));
    }
    #[test]fn local_data_permission_is_independent_and_hash_and_scan_reviews_are_single_use(){
        use crate::data_policy::{DataCategory,DataDestination,Sensitivity};
        for local_enabled in [false,true] {for (operation,action) in [("hash","kali.mcp.hashEvidence"),("scan","kali.mcp.scanEvidence")] {
            let dir=tempfile::tempdir().unwrap();let(store,assessment)=setup_store(dir.path());
            let imported=json!({"id":model::id()});
            store.mutate(|s|{s.assessments[0].evidence.push(Evidence{id:imported["id"].as_str().unwrap().into(),name:"fixture.txt".into(),content:"eval(fixture)\r\n".into(),sha256:hash("eval(fixture)\r\n"),created_at:model::now()});Ok(())}).unwrap();
            let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();
            let ai=crate::ai::AiService::new(store.root.clone()).unwrap().with_data_policy(bridge.data_policy.clone());let redactor=ai.data_redactor();
            let snapshot=store.snapshot().unwrap();let pid=&snapshot.assessments[0].project_id;
            let targets=crate::scope_targets::catalog_targets(&snapshot.projects[0]).into_iter().map(|t|t.id).collect::<Vec<_>>();
            let mut body=crate::data_policy::default_body();
            for rule in &mut body.destinations {
                rule.enabled=if rule.destination==DataDestination::LocalKali{local_enabled}else{!local_enabled};
                rule.categories=vec![DataCategory::EvidenceExcerpt,DataCategory::SourceExcerpt,DataCategory::TargetMetadata];
                rule.sensitivities=vec![Sensitivity::Public,Sensitivity::Internal];rule.target_catalog_ids=targets.clone();
                rule.allow_original_evidence=rule.destination==DataDestination::LocalKali;
            }
            let status=bridge.data_policy.handle("data.policy.status",json!({"projectId":pid}),&store,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
            let policy=bridge.data_policy.handle("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":body}),&store,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
            bridge.data_policy.handle("data.policy.commit",json!({"review":policy["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}),&store,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
            crate::data_policy::test_support::authorize(&store,&bridge.data_policy,&assessment,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
            let environment=model::id();let legacy=json!({"assessmentId":assessment,"evidenceId":imported["id"],"acknowledgeLocalTransfer":true});
            assert!(bridge.admit_data_before_identity(action,&legacy,&store,&redactor).unwrap_err().contains("data_review_required"));
            assert!(bridge.handle_reviewed(action,legacy.clone(),&store,&environment,&ai).unwrap_err().contains("data_review_required"));
            assert!(bridge.runs().unwrap().is_empty());assert_eq!(launcher.launches.load(Ordering::SeqCst),0);
            let preview=bridge.handle_data_prepare("kali.data.prepare",json!({"assessmentId":assessment,"evidenceId":imported["id"],"operation":operation,"packageSensitivity":"internal"}),&store,&environment,&redactor);
            if !local_enabled {
                assert!(preview.unwrap_err().contains("data_destination_denied"));
                assert!(store.snapshot().unwrap().data_use_receipts.is_empty());assert_eq!(launcher.launches.load(Ordering::SeqCst),0);continue;
            }
            let preview=preview.unwrap();assert_eq!(preview["rawSha256"],hash("eval(fixture)\r\n"));assert_eq!(preview["rawBytes"],15);
            let mut payload=legacy;payload["dataReview"]=preview["dataReview"].clone();
            bridge.admit_data_before_identity(action,&payload,&store,&redactor).unwrap();
            let result=bridge.handle_reviewed(action,payload.clone(),&store,&environment,&ai).unwrap();assert_eq!(result["run"]["status"],"completed");
            assert!(bridge.admit_data_before_identity(action,&payload,&store,&redactor).unwrap_err().contains("data_review_used"));
            assert!(bridge.handle_reviewed(action,payload,&store,&environment,&ai).is_err());assert_eq!(launcher.launches.load(Ordering::SeqCst),1);
            let state=store.snapshot().unwrap();let original=state.assessments[0].evidence.iter().find(|e|e.id==imported["id"]).unwrap();
            assert_eq!(original.content,"eval(fixture)\r\n");assert_eq!(original.sha256,hash(&original.content));
            assert_eq!(state.data_use_receipts.len(),1);assert_eq!(state.data_use_receipts[0].events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::Completed);
        }}
    }
    #[cfg(windows)]
    #[test]fn recognized_credential_in_original_kali_bytes_never_receives_a_review_or_launch(){
        let dir=tempfile::tempdir().unwrap();let(store,assessment)=setup_store(dir.path());
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();
        let ai=crate::ai::AiService::new(store.root.clone()).unwrap();let key="synthetic-kali-data-credential-0123456789";
        ai.handle("ai.key.save",json!({"key":key}),&store).unwrap();
        let content=format!("unusual label: {key}");let eid=model::id();
        store.mutate(|s|{s.assessments[0].evidence.push(Evidence{id:eid.clone(),name:"fixture.txt".into(),content:content.clone(),sha256:hash(&content),created_at:model::now()});Ok(())}).unwrap();
        let redactor=ai.data_redactor();crate::data_policy::test_support::authorize(&store,&bridge.data_policy,&assessment,&redactor,&crate::data_projection::RejectExternal,None).unwrap();
        for operation in ["hash","scan"] {
            assert!(bridge.handle_data_prepare("kali.data.prepare",json!({"assessmentId":assessment,"evidenceId":eid,"operation":operation,"packageSensitivity":"internal"}),&store,&model::id(),&redactor).unwrap_err().contains("data_secret_material"));
        }
        assert!(bridge.runs().unwrap().is_empty());assert!(store.snapshot().unwrap().data_use_receipts.is_empty());assert_eq!(launcher.launches.load(Ordering::SeqCst),0);
        let source=&store.snapshot().unwrap().assessments[0].evidence[0];assert_eq!(source.content,content);assert_eq!(source.sha256,hash(&content));
    }
    #[test]fn combined_kali_keeps_all_contributors_and_runtime_diagnostics_claim_no_target(){
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();
        let(aid,refs)=crate::evidence_lineage::test_scope(&store,&["kali.rg.signals.v1"],&["kali.runtime-diagnostics.v1"]);
        let imported=store.handle(crate::core::Request{action:"evidence.import".into(),payload:json!({"assessmentId":aid,"name":"fixture.txt","sourceDescription":"Human fixture spanning both selected resources","content":"eval(fixture)\n","acknowledgeUnverifiedSource":true,"contextTargetRefs":refs})}).unwrap();
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();let environment=model::id();
        let result=test_scan(&bridge,json!({"assessmentId":aid,"evidenceId":imported["id"],"acknowledgeLocalTransfer":true}),&store,&environment).unwrap();
        assert_eq!(result["run"]["status"],"completed");let eid=result["run"]["evidenceId"].as_str().unwrap().to_owned();
        let mut expected=refs.clone();expected.sort_by(|a,b|a.entry_id.cmp(&b.entry_id));
        assert_eq!(crate::evidence_lineage::evidence_refs(&store,&store.snapshot().unwrap(),&aid,&[eid.clone()]).unwrap(),expected);
        let projection=crate::evidence_lineage::projection(&store,json!({"assessmentId":aid})).unwrap();
        let manual=projection["evidence"].as_array().unwrap().iter().find(|e|e["evidenceId"]==imported["id"]).unwrap();assert_eq!(manual["originKind"],"user_association");assert_eq!(manual["verified"],true);
        let diagnostic=bridge.handle("kali.mcp.diagnose",json!({"assessmentId":aid}),&store,&environment).unwrap();let diagnostic_id=diagnostic["run"]["evidenceId"].as_str().unwrap().to_owned();
        assert!(crate::evidence_lineage::evidence_refs(&store,&store.snapshot().unwrap(),&aid,&[diagnostic_id]).unwrap().is_empty());
        let mut run=bridge.find(result["run"]["id"].as_str().unwrap()).unwrap();run.source_target_refs.pop();bridge.save(&run).unwrap();
        assert!(crate::evidence_lineage::evidence_refs(&store,&store.snapshot().unwrap(),&aid,&[eid]).is_err());assert_eq!(launcher.launches.load(Ordering::SeqCst),2);
    }
    #[cfg(windows)]
    #[test]fn combined_kali_receipt_chain_keeps_origin_and_rejects_cycles(){
        let(_source,f,receipt,_)=crate::evidence_lineage::test_remediation_receipt();let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(f.store.root.clone(),launcher.clone()).unwrap();
        let scan=test_scan(&bridge,json!({"assessmentId":f.aid,"evidenceId":receipt,"acknowledgeLocalTransfer":true}),&f.store,&model::id()).unwrap();assert_eq!(scan["run"]["status"],"completed");let eid=scan["run"]["evidenceId"].as_str().unwrap().to_owned();
        let ai=crate::ai::AiService::new(f.store.root.clone()).unwrap();let finding=ai.handle("candidate.record",json!({"assessmentId":f.aid,"title":"Review stored parser result","description":"Synthetic provenance chain only","severity":"low","evidenceIds":[eid]}),&f.store).unwrap();
        assert_eq!(crate::evidence_lineage::finding_target_refs(&f.store,&f.store.snapshot().unwrap(),finding["id"].as_str().unwrap()).unwrap(),vec![f.refs[0].clone()]);
        let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();crate::recovery::test_support::report(&recovery,&f.store,&f.aid,false).unwrap();
        let mut cyclic=f.store.snapshot().unwrap();let e=cyclic.assessments[0].evidence.iter_mut().find(|e|e.id==eid).unwrap();let mut raw:Value=serde_json::from_str(&e.content).unwrap();raw["sourceEvidenceId"]=json!(eid);e.content=raw.to_string();e.sha256=hash(&e.content);
        let mut run=bridge.find(scan["run"]["id"].as_str().unwrap()).unwrap();run.source_evidence_id=Some(eid.clone());bridge.save(&run).unwrap();
        assert!(crate::evidence_lineage::evidence_refs(&f.store,&cyclic,&f.aid,&[eid]).is_err());assert_eq!(launcher.launches.load(Ordering::SeqCst),1);
    }
    #[test]fn combined_reconciliation_after_revocation_is_journal_only_and_never_new_scope_evidence(){
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();let(aid,_)=crate::evidence_lineage::test_scope(&store,&["http.headers.v1"],&["kali.runtime-diagnostics.v1"]);let environment=model::id();
        let bridge=KaliBridge::with_launcher(store.root.clone(),Arc::new(TestLauncher{launches:0.into(),clean:false,block:false,started:None})).unwrap();
        let original=bridge.handle("kali.mcp.diagnose",json!({"assessmentId":aid}),&store,&environment).unwrap();assert_eq!(original["run"]["status"],"unknown");drop(bridge);
        let status=crate::scope::status(&store,&aid).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":aid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"rationale":"End resource admissions while retaining reconciliation","acknowledgeRevocation":true}),&store,str::to_owned).unwrap();
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(store.root.clone(),launcher.clone()).unwrap();
        let result=bridge.handle("kali.mcp.reconcile",json!({"assessmentId":aid,"runId":original["run"]["id"]}),&store,&environment).unwrap();assert_eq!(result["cleanupConfirmed"],true);
        assert_eq!(bridge.runs().unwrap().len(),2);assert!(store.snapshot().unwrap().assessments[0].evidence.is_empty());assert_eq!(launcher.launches.load(Ordering::SeqCst),1);
    }
    #[test]fn unknown_cleanup_blocks_new_dispatch_and_reconcile_does_not_replay(){
        let dir=tempfile::tempdir().unwrap();let (store,assessment)=setup_store(dir.path());let environment=model::id();
        let first=Arc::new(TestLauncher{launches:0.into(),clean:false,block:false,started:None});let bridge=KaliBridge::with_launcher(dir.path().into(),first.clone()).unwrap();
        let response=bridge.handle("kali.mcp.diagnose",json!({"assessmentId":assessment}),&store,&environment).unwrap();assert_eq!(response["run"]["status"],"unknown");assert!(bridge.has_unresolved());
        assert!(bridge.handle("kali.mcp.diagnose",json!({"assessmentId":assessment}),&store,&environment).is_err());assert_eq!(first.launches.load(Ordering::SeqCst),1);assert!(store.snapshot().unwrap().assessments[0].evidence.is_empty());drop(bridge);
        let next=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(dir.path().into(),next.clone()).unwrap();
        let result=bridge.handle("kali.mcp.reconcile",json!({"assessmentId":assessment,"runId":response["run"]["id"]}),&store,&environment).unwrap();
        assert_eq!(result["run"]["status"],"failed");assert_eq!(result["cleanupConfirmed"],true);assert!(!bridge.has_unresolved());assert_eq!(next.launches.load(Ordering::SeqCst),1);assert_eq!(bridge.runs().unwrap().len(),2);
    }
    #[test]fn concurrent_cancel_waits_for_cleanup_confirmation(){
        let dir=tempfile::tempdir().unwrap();let (store,assessment)=setup_store(dir.path());let (started,ready)=mpsc::channel();
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:true,started:Some(started)});let bridge=Arc::new(KaliBridge::with_launcher(dir.path().into(),launcher).unwrap());
        let worker_bridge=bridge.clone();let worker_store=store.clone();let worker_assessment=assessment.clone();
        let worker=std::thread::spawn(move||worker_bridge.handle("kali.mcp.diagnose",json!({"assessmentId":worker_assessment}),&worker_store,&model::id()).unwrap());
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        let requested=bridge.handle("kali.mcp.cancel",json!({"assessmentId":assessment}),&store,"").unwrap();assert_eq!(requested["status"],"canceling");
        let final_result=worker.join().unwrap();assert_eq!(final_result["run"]["status"],"canceled");assert_eq!(final_result["cleanupConfirmed"],true);assert!(!bridge.has_unresolved());
    }
    fn cancel_scan_fixture(kind:CancelReplyKind)->(tempfile::TempDir,Arc<Store>,Value,String){
        let dir=tempfile::tempdir().unwrap();let (store,assessment)=setup_store(dir.path());let source_id=model::id();
        store.mutate(|s|{let content="eval(first)\neval(second)\n".to_owned();s.assessments[0].evidence.push(Evidence{id:source_id.clone(),name:"selected.txt".into(),sha256:hash(&content),content,created_at:model::now()});Ok(())}).unwrap();
        let bridge=KaliBridge::with_launcher(dir.path().into(),Arc::new(CancelScanLauncher{kind})).unwrap();
        let result=test_scan(&bridge,json!({"assessmentId":assessment,"evidenceId":source_id,"acknowledgeLocalTransfer":true}),&store,&model::id()).unwrap();
        (dir,store,result,source_id)
    }
    #[test]fn canceled_scan_preserves_a_verified_partial_reply_drained_before_closed_ack(){
        let (_dir,store,result,source_id)=cancel_scan_fixture(CancelReplyKind::Valid);
        assert_eq!(result["run"]["status"],"canceled");assert_eq!(result["cleanupConfirmed"],true);
        assert_eq!(result["result"]["partial"],true);assert_eq!(result["result"]["countsComplete"],false);assert_eq!(result["result"]["observedSignals"],1);
        assert_eq!(result["run"]["sourceEvidenceId"],source_id);
        let snapshot=store.snapshot().unwrap();assert!(snapshot.findings.is_empty());assert_eq!(snapshot.assessments[0].status,"draft");assert_eq!(snapshot.assessments[0].evidence.len(),2);
        let evidence=snapshot.assessments[0].evidence.iter().find(|e|Some(e.id.as_str())==result["run"]["evidenceId"].as_str()).unwrap();
        assert_eq!(evidence.sha256,hash(&evidence.content));let retained:Value=serde_json::from_str(&evidence.content).unwrap();
        assert_eq!(retained["sourceEvidenceId"],source_id);assert_eq!(retained["result"],result["result"]);
    }
    #[test]fn cancellation_never_retains_malformed_foreign_duplicate_or_undispatched_scan_results(){
        for kind in [CancelReplyKind::Malformed,CancelReplyKind::Foreign,CancelReplyKind::WrongId,CancelReplyKind::Duplicate,CancelReplyKind::MissingCleanup,CancelReplyKind::BeforeTool] {
            let (_dir,store,result,_)=cancel_scan_fixture(kind);
            assert_eq!(result["run"]["status"],if kind==CancelReplyKind::MissingCleanup{"unknown"}else{"canceled"},"{kind:?}");
            assert!(result["run"]["evidenceId"].is_null(),"{kind:?}");assert!(result["result"].is_null(),"{kind:?}");
            assert_eq!(store.snapshot().unwrap().assessments[0].evidence.len(),1,"{kind:?}");
        }
    }
    #[test]fn restart_before_dispatch_cannot_create_unknown_linux_work(){
        let dir=tempfile::tempdir().unwrap();let bridge=KaliBridge::new(dir.path().into()).unwrap();let mut r=run();r.status="starting".into();r.launch_attempted=false;bridge.save(&r).unwrap();drop(bridge);
        let reopened=KaliBridge::new(dir.path().into()).unwrap();let restored=reopened.find(&r.id).unwrap();assert_eq!(restored.status,"failed");assert!(restored.cleanup_confirmed);assert!(!reopened.has_unresolved());
    }
    #[test]fn scan_requires_explicit_consent_selected_evidence_and_byte_limit_before_launch(){
        let dir=tempfile::tempdir().unwrap();let (store,assessment)=setup_store(dir.path());
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(dir.path().into(),launcher.clone()).unwrap();
        let environment=model::id();let id=model::id();
        for payload in [json!({"assessmentId":assessment,"evidenceId":id}),json!({"assessmentId":assessment,"evidenceId":id,"acknowledgeLocalTransfer":false}),json!({"assessmentId":assessment,"evidenceId":id,"acknowledgeLocalTransfer":true}),json!({"assessmentId":assessment,"evidenceId":id,"acknowledgeLocalTransfer":true,"regex":".*"})] {
            assert!(test_scan(&bridge,payload,&store,&environment).is_err());
        }
        for content in ["a".repeat(MAX_BYTES+1),"a\0b".into()] {
            store.mutate(|s|{s.assessments[0].evidence=vec![Evidence{id:id.clone(),name:"fixture.txt".into(),sha256:hash(&content),content,created_at:model::now()}];Ok(())}).unwrap();
            assert!(test_scan(&bridge,json!({"assessmentId":assessment,"evidenceId":id,"acknowledgeLocalTransfer":true}),&store,&environment).is_err());
        }
        assert_eq!(launcher.launches.load(Ordering::SeqCst),0);
    }
    #[test]fn scan_records_source_provenance_and_candidates_without_promoting_findings(){
        let dir=tempfile::tempdir().unwrap();let (store,assessment)=setup_store(dir.path());
        let id=model::id();let content="eval('fixture-secret')".to_string();
        store.mutate(|s|{s.assessments[0].evidence.push(Evidence{id:id.clone(),name:"selected.txt".into(),sha256:hash(&content),content,created_at:model::now()});Ok(())}).unwrap();
        let launcher=Arc::new(TestLauncher{launches:0.into(),clean:true,block:false,started:None});let bridge=KaliBridge::with_launcher(dir.path().into(),launcher).unwrap();
        let result=test_scan(&bridge,json!({"assessmentId":assessment,"evidenceId":id,"acknowledgeLocalTransfer":true}),&store,&model::id()).unwrap();
        assert_eq!(result["run"]["status"],"completed");assert_eq!(result["run"]["sourceEvidenceId"],id);assert_eq!(result["result"]["candidates"][0]["status"],"candidate");
        assert!(!result.to_string().contains("fixture-secret"));let state=store.snapshot().unwrap();assert_eq!(state.assessments[0].status,"draft");assert!(state.findings.is_empty());assert_eq!(state.assessments[0].evidence.len(),2);
    }
    #[test]fn scan_result_rejects_digest_raw_values_unbounded_lines_and_forged_counts(){
        let r=run();let args=json!({"base64":STANDARD.encode(b"eval(1)"),"expectedSha256":hash("eval(1)")});let result=scanned(&metadata(&r),&args);assert!(scan_result_valid(&result,&args));
        for (pointer,value) in [("/inputSha256",json!("0".repeat(64))),("/candidates/0/line",json!(2)),("/candidates/0/label",json!("secret-value")),("/counts/dynamic-evaluation",json!(0)),("/partial",json!(true)),("/tool/binarySha256",json!("invalid"))] {
            let mut invalid=result.clone();*invalid.pointer_mut(pointer).unwrap()=value;assert!(!scan_result_valid(&invalid,&args),"{pointer}");
        }
        let mut invalid=result.clone();invalid["rawSource"]=json!("do not return this");assert!(!scan_result_valid(&invalid,&args));
    }
    #[test]fn unexpected_scan_schema_stops_before_tool_call_and_legacy_runs_load(){
        let r=run();let mut replies=frames(&r,diagnostic(&r));replies[1]["result"]["tools"][2]["inputSchema"]["properties"]["path"]=json!({"type":"string"});
        let mut t=Fake{frames:replies,sent:vec![],clean:true};assert!(exchange(&mut t,&r,"runtime_diagnostics",&json!({}),Instant::now()+Duration::from_secs(1),&AtomicBool::new(false)).is_err());assert_eq!(t.sent.len(),3);
        let mut legacy=serde_json::to_value(&r).unwrap();legacy.as_object_mut().unwrap().remove("sourceEvidenceId");let decoded:BridgeRun=serde_json::from_value(legacy).unwrap();assert!(decoded.source_evidence_id.is_none());
        assert!(serde_json::to_vec(&bootstrap(&r).unwrap()).unwrap().len()<65536);
    }
}
