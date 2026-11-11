//! Host-only execution ownership for the fixed Kali bridge. Recovery observes
//! known sessions while retaining the original resource lease throughout I/O.
use super::*;
use crate::workflow::EffectRef;
use crate::work_supervisor::{EffectOwner,EffectOwnerKind,EffectPermit,EffectTermination,ReconciliationPermit,WorkArbiter};
use rusqlite::OpenFlags;

pub(crate) struct KaliRecoveryTarget {
    root_id:String,
    assessment_id:String,
    environment_id:String,
    inspected_id:String,
    dispatch_deadline:u64,
}
impl KaliRecoveryTarget {
    pub(crate) fn root_effect(&self)->EffectRef{EffectRef::KaliRun{id:self.root_id.clone()}}
    pub(crate) fn assessment_id(&self)->&str{&self.assessment_id}
}
/// Constructed only after the bridge durably records cleanup for the original
/// session and every diagnostic descendant. No caller can deserialize a proof.
pub(crate) struct KaliReconciliationProof {root_id:String,assessment_id:String}
impl KaliReconciliationProof {
    pub(crate) fn root_effect(&self)->EffectRef{EffectRef::KaliRun{id:self.root_id.clone()}}
    pub(crate) fn assessment_id(&self)->&str{&self.assessment_id}
}
/// Old-boot proof that a new-format reserved run never reached the Linux launch
/// marker. Older owner kinds deliberately cannot obtain this proof.
pub(crate) struct KaliNotStartedProof {owner:EffectOwner,boot_id:String,effect:Option<EffectRef>}
impl KaliNotStartedProof {
    pub(crate) fn owner(&self)->&EffectOwner{&self.owner}
    pub(crate) fn boot_id(&self)->&str{&self.boot_id}
    pub(crate) fn effect(&self)->Option<&EffectRef>{self.effect.as_ref()}
}
pub(super) enum KaliDispatch<'a> {
    Direct{permit:EffectPermit<'a>,reserved_id:String,bound_run:Option<String>},
    Recovery{permit:ReconciliationPermit<'a>,target:KaliRecoveryTarget},
}
impl KaliDispatch<'_> {
    pub(super) fn run_id(&self)->String{match self{Self::Direct{reserved_id,..}=>reserved_id.clone(),Self::Recovery{..}=>model::id()}}
    pub(super) fn reconciliation_root(&self)->Option<String>{match self{Self::Direct{..}=>None,Self::Recovery{target,..}=>Some(target.root_id.clone())}}
    pub(super) fn validate(&self,assessment:&str,environment:&str,tool:&str,args:&Value,reconcile:bool,has_data:bool)->Result<(),String>{
        match self {
            Self::Direct{..} if !reconcile=>Ok(()),
            Self::Recovery{permit,target} if reconcile&&!has_data&&tool=="runtime_diagnostics"
                &&assessment==target.assessment_id&&environment==target.environment_id
                &&*permit.effect()==target.root_effect()
                &&*args==json!({"inspectSessionId":target.inspected_id,"dispatchDeadline":target.dispatch_deadline})=>Ok(()),
            _=>Err("work_owned_operation: o controle de recuperação aceita somente diagnóstico da sessão registrada.".into()),
        }
    }
    pub(super) fn bind(&mut self,arbiter:&WorkArbiter,run:&BridgeRun)->Result<(),String>{
        if let Self::Direct{permit,bound_run,..}=self {
            if bound_run.is_some(){return Err("work_owned_operation".into());}
            arbiter.bind_effect(permit,EffectRef::KaliRun{id:run.id.clone()})?;
            *bound_run=Some(run.id.clone());
        }Ok(())
    }
}

pub(super) fn unresolved(run:&BridgeRun)->bool{
    matches!(run.status.as_str(),"starting"|"running"|"canceling"|"unknown")||!run.cleanup_confirmed
}
pub(crate) fn read_runs(db:&Connection)->Result<Vec<BridgeRun>,String>{
    let sql:String=db.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='bridge_runs'",[],|r|r.get(0)).map_err(db_error)?;
    let normalized=sql.chars().filter(|c|!c.is_ascii_whitespace()).collect::<String>().to_ascii_lowercase().replace("ifnotexists","");
    let triggers:u32=db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND tbl_name='bridge_runs'",[],|r|r.get(0)).map_err(db_error)?;
    if normalized!="createtablebridge_runs(idtextprimarykey,datatextnotnull)"||triggers!=0{return Err(db_error("schema"));}
    let mut schema=db.prepare("PRAGMA table_info(bridge_runs)").map_err(db_error)?;
    let columns=schema.query_map([],|r|Ok((r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,u32>(3)?,r.get::<_,u32>(5)?))).map_err(db_error)?.collect::<Result<Vec<_>,_>>().map_err(db_error)?;
    if columns!=vec![("id".into(),"TEXT".into(),0,1),("data".into(),"TEXT".into(),1,0)]{return Err(db_error("schema"));}
    let mut query=db.prepare("SELECT id,CASE WHEN length(CAST(data AS BLOB))<=16384 THEN data ELSE NULL END FROM bridge_runs ORDER BY rowid DESC LIMIT 10001").map_err(db_error)?;
    let rows=query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?))).map_err(db_error)?;
    let mut out=vec![];
    for row in rows {
        let (id,raw)=row.map_err(db_error)?;let raw=raw.ok_or_else(||db_error("size"))?;
        let run:BridgeRun=serde_json::from_str(&raw).map_err(db_error)?;
        valid_id(&run.id)?;valid_id(&run.assessment_id)?;valid_id(&run.environment_id)?;
        for id in [&run.source_evidence_id,&run.evidence_id,&run.reconciliation_of].into_iter().flatten(){valid_id(id)?;}
        if id!=run.id||!matches!(run.status.as_str(),"starting"|"running"|"canceling"|"completed"|"failed"|"canceled"|"unknown")
            ||!matches!(run.tool.as_str(),"runtime_diagnostics"|"hash_bytes"|"scan_bytes"|"capture_collect"|"capture_purge"|"capture_retire")
            ||run.source_sha256.len()!=64||!run.source_sha256.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c))
            ||chrono::DateTime::parse_from_rfc3339(&run.started_at).is_err()
            ||run.finished_at.as_ref().is_some_and(|t|chrono::DateTime::parse_from_rfc3339(t).is_err())
            ||run.cleanup_confirmed&&(run.finished_at.is_none()||matches!(run.status.as_str(),"starting"|"running"|"canceling"|"unknown")) {
            return Err(db_error("record"));
        }
        capture_io::validate_context(&run)?;out.push(run);
    }
    if out.len()>10000{return Err(db_error("capacity"));}
    for run in &out {
        if let Some(root)=&run.reconciliation_of {
            let original=out.iter().find(|candidate|candidate.id==*root).ok_or_else(||db_error("root"))?;
            if root==&run.id||original.reconciliation_of.is_some()||original.assessment_id!=run.assessment_id
                ||original.environment_id!=run.environment_id||!matches!(run.tool.as_str(),"runtime_diagnostics"|"capture_collect"|"capture_purge"|"capture_retire")
                ||run.source_evidence_id.is_some()||run.scope_ref.is_some()||!run.source_target_refs.is_empty(){return Err(db_error("family"));}
            if let Some(capture_io::KaliRunContext::Control{intent,..}|capture_io::KaliRunContext::RetireControl{intent,..})=&run.work_context{let binding=capture_io::producer_binding(original)?;if !binding.captures.iter().any(|slot|slot.capture_id==intent.capture_id()&&slot.stream==crate::capture::CaptureStream::ToolStdout)||intent.fingerprint()!=binding.owner_profile_fingerprint()||intent.producer_deadline()!=original.dispatch_before_unix_seconds{return Err(db_error("family"));}}
            capture_io::retire::validate_family(run,original,&out)?;
        }
    }
    Ok(out)
}

/// Read-only pre-claim gate, including legacy sessions that predate work_leases.
/// Missing journals mean no prior Kali work; malformed journals fail closed.
pub(crate) fn has_unresolved_work(root:&std::path::Path)->Result<bool,String>{
    let path=root.join("kali-bridge.sqlite3");
    match fs::symlink_metadata(&path){
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(error)=>return Err(db_error(error)),
        Ok(meta) if !meta.is_file()||meta.file_type().is_symlink()=>return Err(db_error("file")),
        Ok(_)=>{},
    }
    let db=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(db_error)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(db_error)?;
    Ok(read_runs(&db)?.iter().any(unresolved))
}

impl KaliBridge {
    /// Diagnostic child under an already-held queued recovery permit. The
    /// target and root come only from validated durable bridge records.
    pub(super) fn probe_work_session(&self,root:&BridgeRun,target:&mut BridgeRun,cancel:&AtomicBool)->Result<bool,String>{
        if target.id!=root.id&&target.reconciliation_of.as_deref()!=Some(&root.id){return Err("work_owned_operation".into());}
        if target.assessment_id!=root.assessment_id||target.environment_id!=root.environment_id{return Err("work_owned_operation".into());}
        if !unresolved(target){return Ok(true);}
        if !target.launch_attempted{target.status="failed".into();target.cleanup_confirmed=true;target.finished_at=Some(model::now());target.error=Some("Intenção durável encerrada antes da criação do processo Linux.".into());self.save(target)?;return Ok(true);}
        if self.runs()?.len()>=10000{return Err("work_journal_quota".into());}
        let mut probe=BridgeRun{id:model::id(),assessment_id:root.assessment_id.clone(),environment_id:root.environment_id.clone(),tool:"runtime_diagnostics".into(),status:"starting".into(),started_at:model::now(),finished_at:None,source_sha256:hash(SOURCE),evidence_id:None,error:None,cleanup_confirmed:false,launch_attempted:false,dispatch_before_unix_seconds:(chrono::Utc::now().timestamp()+30) as u64,source_evidence_id:None,scope_ref:None,source_target_refs:vec![],reconciliation_of:Some(root.id.clone()),work_context:None};
        self.save(&probe)?;probe.launch_attempted=true;self.save(&probe)?;
        let mut transport=match self.launcher.launch(&probe,cancel){Ok(t)=>t,Err(error)=>{probe.status="unknown".into();probe.error=Some("Lançamento da inspeção sem confirmação de término.".into());self.save(&probe)?;return Err(error);}};let mut state=ToolCallState::NotSent;
        let observed=exchange_tracked(&mut *transport,&probe,"runtime_diagnostics",&json!({"inspectSessionId":target.id,"dispatchDeadline":target.dispatch_before_unix_seconds}),Instant::now()+Duration::from_secs(45),cancel,&mut state,||Ok(()));
        let finish=transport.finish(&probe,Instant::now()+Duration::from_secs(5));
        probe.cleanup_confirmed=finish.cleanup_confirmed;probe.finished_at=if finish.cleanup_confirmed{Some(model::now())}else{None};probe.status=if !finish.cleanup_confirmed{"unknown"}else if observed.is_ok(){"completed"}else{"failed"}.into();
        probe.error=if observed.is_ok(){None}else{Some("Inspeção da sessão registrada não confirmou o término.".into())};self.save(&probe)?;
        let confirmed=finish.cleanup_confirmed&&observed.as_ref().is_ok_and(|v|v["inspectedSession"]["sessionId"]==target.id&&v["inspectedSession"]["known"]==true&&v["inspectedSession"]["active"]==false);
        if confirmed{target.cleanup_confirmed=true;target.finished_at=Some(model::now());target.status=if matches!(&target.work_context,Some(capture_io::KaliRunContext::Producer{observed:Some(_),..})){"completed"}else{"failed"}.into();target.error=Some("Término comprovado pela sessão registrada; nenhuma operação foi repetida.".into());self.save(target)?;}
        Ok(confirmed)
    }
    pub(crate) fn with_arbiter(mut self,arbiter:Arc<WorkArbiter>)->Self{self.arbiter=arbiter;self}

    /// The identity resolver is host-owned and checks the Windows registration
    /// only. Linux readiness is checked inside the journaled MCP process before
    /// initialize. Failed preflight does not consume a data review.
    pub(crate) fn handle_with_identity(&self,action:&str,payload:Value,store:&Store,identity:impl FnOnce()->Result<String,String>,ai:Option<&crate::ai::AiService>)->Result<Value,String>{
        if !payload.is_object()||payload.to_string().len()>4096{return Err("Solicitação MCP inválida ou grande demais.".into());}
        if !needs_identity(action){return self.handle_internal(action,payload,store,"",ai,None);}
        if let Some(ai)=ai{self.admit_data_before_identity(action,&payload,store,&ai.data_redactor())?;}
        admit_before_identity(action,&payload,store)?;
        let mut dispatch=if action=="kali.mcp.reconcile" {
            let input:ReconcileInput=decode(payload.clone())?;check_assessment(store,&input.assessment_id)?;
            let runs=self.runs()?;let previous=runs.iter().find(|r|r.id==input.run_id).ok_or("Sessão MCP não encontrada.")?;
            if previous.assessment_id!=input.assessment_id||(previous.status!="unknown"&&unresolved(previous)){return Err("Somente sessão desconhecida ou término já comprovado desta avaliação pode ser reconciliada.".into());}
            let root_id=previous.reconciliation_of.as_ref().unwrap_or(&previous.id);
            let target=KaliRecoveryTarget{root_id:root_id.clone(),assessment_id:previous.assessment_id.clone(),environment_id:previous.environment_id.clone(),inspected_id:previous.id.clone(),dispatch_deadline:previous.dispatch_before_unix_seconds};
            let permit=self.arbiter.claim_reconciliation(store,&target)?;
            KaliDispatch::Recovery{permit,target}
        }else{
            let assessment=match action {
                "kali.mcp.diagnose"=>decode::<AssessmentInput>(payload.clone())?.assessment_id,
                "kali.mcp.hashEvidence"=>decode::<HashInput>(payload.clone())?.assessment_id,
                "kali.mcp.scanEvidence"=>decode::<ScanInput>(payload.clone())?.assessment_id,
                _=>return Err("Ação MCP Kali indisponível.".into()),
            };
            check_assessment(store,&assessment)?;
            let reserved_id=model::id();
            let review=match action{"kali.mcp.hashEvidence"=>decode::<HashInput>(payload.clone())?.data_review,"kali.mcp.scanEvidence"=>decode::<ScanInput>(payload.clone())?.data_review,_=>None};
            let permit=self.arbiter.claim_direct_kali(store,EffectOwner{id:reserved_id.clone(),assessment_id:Some(assessment),kind:if action=="kali.mcp.diagnose"{EffectOwnerKind::KaliDiagnosticIntent}else{EffectOwnerKind::KaliRunIntent}},review.as_ref())?;
            KaliDispatch::Direct{permit,reserved_id,bound_run:None}
        };
        // Recover a lost final commit without opening a second process merely
        // to re-prove cleanup that this journal already durably records.
        let already_stopped=match &dispatch{KaliDispatch::Recovery{target,..}=>!unresolved(&self.find(&target.inspected_id)?),_=>false};
        let result=if already_stopped {
            if self.finish_reconciliation(store,&dispatch)? {
                let KaliDispatch::Recovery{target,..}=&dispatch else{unreachable!()};
                Ok(json!({"run":self.find(&target.inspected_id)?,"result":Value::Null,"cleanupConfirmed":true,"probeRunId":Value::Null}))
            }else{Err("work_reconciliation_required: um diagnóstico descendente ainda exige confirmação de término.".into())}
        }else{identity().and_then(|environment|self.handle_internal(action,payload,store,&environment,ai,Some(&mut dispatch)))};
        if let KaliDispatch::Direct{permit,bound_run,..}=dispatch {
            let termination=match bound_run {
                None=>EffectTermination::NotStarted,
                Some(id)=>if self.find(&id).is_ok_and(|run|!unresolved(&run)){EffectTermination::Stopped}else{EffectTermination::Unknown},
            };
            self.arbiter.finish(permit,termination)?;
        }
        result
    }

    /// Startup recovery is observation-only. A pre-launch journal (or no row
    /// for an unbound reserved id) proves that no Linux session was dispatched.
    pub(crate) fn recover_work_after_boot(&self,store:&Store)->Result<(),String>{
        let Some(candidate)=self.arbiter.direct_kali_recovery_candidate(store)? else{return Ok(());};
        if self.active.lock().map_err(db_error)?.is_some(){return Err("work_busy".into());}
        let runs=self.runs()?;let existing=runs.iter().find(|r|r.id==candidate.owner.id);
        if let Some(run)=existing {
            if candidate.owner.assessment_id.as_deref()!=Some(run.assessment_id.as_str())||run.reconciliation_of.is_some(){return Err("work_reconciliation_required".into());}
            if run.launch_attempted {
                if !unresolved(run) {
                    let target=KaliRecoveryTarget{root_id:run.id.clone(),assessment_id:run.assessment_id.clone(),environment_id:run.environment_id.clone(),inspected_id:run.id.clone(),dispatch_deadline:run.dispatch_before_unix_seconds};
                    let permit=self.arbiter.claim_reconciliation(store,&target)?;
                    self.finish_reconciliation(store,&KaliDispatch::Recovery{permit,target})?;
                }
                return Ok(());
            }
            if unresolved(run) {
                let mut stopped=run.clone();stopped.status="failed".into();stopped.cleanup_confirmed=true;stopped.finished_at=Some(model::now());stopped.error=Some("Intenção anterior encerrada antes do marcador de lançamento Linux; a chamada não foi repetida.".into());self.save(&stopped)?;
            }
        }else if candidate.effect.is_some(){return Err("work_reconciliation_required: sessão vinculada ausente do journal.".into());}
        let proof=KaliNotStartedProof{owner:candidate.owner,boot_id:candidate.boot_id,effect:candidate.effect};
        self.arbiter.reconcile_kali_not_started(store,&proof)
    }

    pub(super) fn finish_reconciliation(&self,store:&Store,dispatch:&KaliDispatch<'_>)->Result<bool,String>{
        let KaliDispatch::Recovery{permit,target}=dispatch else{return Err("work_owned_operation".into());};
        let runs=self.runs()?;
        let root=runs.iter().find(|r|r.id==target.root_id).ok_or_else(||db_error("root"))?;
        if unresolved(root)||runs.iter().any(|r|r.reconciliation_of.as_deref()==Some(target.root_id.as_str())&&unresolved(r)){return Ok(false);}
        let proof=KaliReconciliationProof{root_id:root.id.clone(),assessment_id:root.assessment_id.clone()};
        store.work_transaction(|conn,_|crate::work_supervisor::finish_reconciliation_locked(conn,permit,&proof))?;Ok(true)
    }
}
