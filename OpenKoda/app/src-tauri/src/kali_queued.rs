//! Typed queued bridge context. References are history; the reviewed packet is
//! held only by the opaque in-memory grant and cannot be restored from SQLite.
use super::*;
use crate::{capture::CaptureStream,data_policy::{DataOriginRef,DataReviewRef,DataUseReceiptRef,Sensitivity},workflow::{AttemptState,EffectRef,KaliOperation,ResultRef},work_supervisor::EffectTermination};

pub(crate) const SCAN_CAPTURE_BYTES:u64=68*1024;
pub(crate) const HASH_CAPTURE_BYTES:u64=4*1024;
pub(crate) const KALI_CATALOG_VERSION:&str="openkoda.kali-tools.v3";
pub(crate) const KALI_LEGACY_CATALOG_VERSION:&str="openkoda.kali-tools.v2";
pub(crate) const KALI_PARSER_VERSION:&str="kali.rg.signals.v1/2026-11-07.1";

#[derive(Clone,Debug)]
pub(crate) struct WorkKaliExpectation {
    pub assessment_id:String,pub scope_ref:crate::scope::ScopeRef,pub input:ResultRef,
    pub operation:KaliOperation,pub environment_id:String,
}
pub(crate) struct QueuedKaliGrant {
    pub(super) prepared:data::PreparedKaliData,
    pub(super) expected:WorkKaliExpectation,
    pub(super) acknowledge_sensitive:bool,
}
impl QueuedKaliGrant {
    pub(crate) fn review(&self)->&DataReviewRef{&self.prepared.review}
    pub(crate) fn expectation(&self)->&WorkKaliExpectation{&self.expected}
    pub(crate) fn max_capture_bytes(&self)->u64{match self.expected.operation{KaliOperation::Scan=>SCAN_CAPTURE_BYTES,KaliOperation::Hash=>HASH_CAPTURE_BYTES}}
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliCaptureSlot {pub(crate) capture_id:String,pub(crate) stream:CaptureStream,pub(crate) max_bytes:u64}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct KaliWorkBinding {
    pub(crate) schema:String,pub(crate) attempt_id:String,pub(crate) assessment_id:String,pub(crate) run_id:String,pub(crate) environment_id:String,pub(crate) owner_profile_fingerprint:String,
    pub(crate) operation:KaliOperation,pub(crate) input_refs:Vec<ResultRef>,pub(crate) origin_refs:Vec<DataOriginRef>,
    pub(crate) executor_source_sha256:String,pub(crate) catalog_version:String,pub(crate) catalog_sha256:String,
    #[serde(deserialize_with="crate::workflow::required_nullable")] pub(crate) parser_version:Option<String>,
    pub(crate) captures:Vec<KaliCaptureSlot>,
}
impl KaliWorkBinding {
    pub(crate) fn attempt_id(&self)->&str{&self.attempt_id}
    pub(crate) fn assessment_id(&self)->&str{&self.assessment_id}
    pub(crate) fn run_id(&self)->&str{&self.run_id}
    pub(crate) fn environment_id(&self)->&str{&self.environment_id}
    pub(crate) fn owner_profile_fingerprint(&self)->&str{&self.owner_profile_fingerprint}
    pub(crate) fn operation(&self)->KaliOperation{self.operation}
    pub(crate) fn capture_ids(&self)->Vec<String>{self.captures.iter().map(|slot|slot.capture_id.clone()).collect()}
    pub(crate) fn input_refs(&self)->&[ResultRef]{&self.input_refs}
    pub(crate) fn validate(&self)->Result<(),String>{
        let invalid=||"capture_integrity: vínculo Kali inválido.".to_owned();
        for id in [&self.attempt_id,&self.assessment_id,&self.run_id,&self.environment_id]{valid_id(id)?;}
        if self.schema!="openkoda.kali-work-binding.v1"||!matches!(self.catalog_version.as_str(),KALI_CATALOG_VERSION|KALI_LEGACY_CATALOG_VERSION)
            ||!crate::workflow::valid_hash(&self.executor_source_sha256)||self.catalog_sha256!=if self.catalog_version==KALI_LEGACY_CATALOG_VERSION{"26907e2c27bf7c28b9a30f6be1e6243f7d1dde346c6706e136d1f112a3567e7e".to_owned()}else{catalog_digest()}||!crate::workflow::valid_hash(&self.owner_profile_fingerprint)
            ||self.input_refs.len()!=1||self.origin_refs.is_empty()||self.origin_refs.len()>8{return Err(invalid());}
        match &self.input_refs[0]{ResultRef::Evidence{id,raw_sha256} if crate::workflow::valid_hash(raw_sha256)=>valid_id(id)?,_=>return Err(invalid())}
        for origin in &self.origin_refs{
            valid_id(&origin.scope_ref.revision_id)?;
            if origin.scope_ref.assessment_id!=self.assessment_id||!crate::workflow::valid_hash(&origin.scope_ref.digest){return Err(invalid());}
            if let Some(target)=&origin.target_ref{valid_id(&target.entry_id)?;if !crate::workflow::valid_hash(&target.entry_digest){return Err(invalid());}}
        }
        let expected=match self.operation{KaliOperation::Scan=>vec![(CaptureStream::ToolStdout,65536),(CaptureStream::SupervisorDiagnostic,4096)],KaliOperation::Hash=>vec![(CaptureStream::AdapterRecords,4096)]};
        if self.captures.len()!=expected.len()||self.parser_version.as_deref()!=match self.operation{KaliOperation::Scan=>Some(KALI_PARSER_VERSION),KaliOperation::Hash=>None}{return Err(invalid());}
        let mut ids=std::collections::HashSet::new();for (slot,(stream,cap)) in self.captures.iter().zip(expected){valid_id(&slot.capture_id)?;if slot.stream!=stream||slot.max_bytes!=cap||!ids.insert(&slot.capture_id){return Err(invalid());}}
        Ok(())
    }
}

pub(crate) struct WorkKaliCompletion {
    pub(super) effect:EffectRef,pub(super) termination:EffectTermination,pub(super) outcome:AttemptState,
    pub(super) results:Vec<ResultRef>,pub(super) receipt_refs:Vec<DataUseReceiptRef>,pub(super) assessment_id:String,pub(super) capture_ids:Vec<String>,
}
impl WorkKaliCompletion {
    pub(crate) fn effect(&self)->&EffectRef{&self.effect}
    pub(crate) fn termination(&self)->EffectTermination{self.termination}
    pub(crate) fn outcome(&self)->AttemptState{self.outcome}
    pub(crate) fn results(&self)->&[ResultRef]{&self.results}
    pub(crate) fn receipt_refs(&self)->&[DataUseReceiptRef]{&self.receipt_refs}
    pub(crate) fn assessment_id(&self)->&str{&self.assessment_id}
    pub(crate) fn capture_ids(&self)->&[String]{&self.capture_ids}
}

impl KaliBridge {
    pub(crate) fn reconcile_work_evidence(&self,store:&Store,permit:&crate::work_supervisor::QueuedRecoveryPermit<'_>,identity:&dyn Fn()->Result<String,String>,_ai:&crate::ai::AiService,commit:impl FnOnce(&rusqlite::Transaction<'_>,&mut model::Snapshot,&crate::work_supervisor::QueuedRecoveryPermit<'_>,&WorkKaliCompletion)->Result<(),String>)->Result<WorkKaliCompletion,String>{
        let candidate=permit.candidate();let EffectRef::KaliRun{id:run_id}=&candidate.reserved_effect else{return Err("work_owned_operation".into());};
        let mut active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;if active.is_some(){return Err("work_busy".into());}
        let records=self.runs()?;let found=records.iter().find(|r|r.id==*run_id).cloned();
        if found.as_ref().is_none_or(|run|!run.launch_attempted)&&candidate.effect.is_none(){
            if candidate.kali_binding.is_some(){return Err("work_unknown_effect".into());}
            if let Some(run)=&found{let binding=capture_io::producer_binding(run)?;if binding.attempt_id!=candidate.attempt_id||binding.assessment_id!=candidate.assessment_id||binding.input_refs!=candidate.input_refs||records.iter().any(|r|r.reconciliation_of.as_deref()==Some(run_id)&&work::unresolved(r)){return Err("work_reconciliation_required".into());}}
            let receipts=store.snapshot()?.data_use_receipts.iter().filter(|r|r.effect.kind=="kali_run"&&r.effect.id==*run_id&&r.review_ref==candidate.data_review_ref).map(|r|DataUseReceiptRef{id:r.id.clone(),digest:r.digest.clone()}).collect::<Vec<_>>();
            for receipt in &receipts{self.data_policy.finish(store,receipt,crate::data_policy::DataUseOutcome::NotSent,vec![])?;}
            if let Some(mut run)=found{run.status="failed".into();run.cleanup_confirmed=true;run.finished_at=Some(model::now());run.error=Some("Intenção recuperada antes do despacho; nenhum processo foi iniciado.".into());self.save(&run)?;}
            let completion=WorkKaliCompletion{effect:candidate.reserved_effect.clone(),termination:EffectTermination::NotStarted,outcome:AttemptState::NotStarted,results:vec![],receipt_refs:receipts,assessment_id:candidate.assessment_id.clone(),capture_ids:vec![]};
            store.work_transaction(|conn,s|commit(conn,s,permit,&completion))?;return Ok(completion);
        }
        let mut run=found.ok_or("work_unknown_effect")?;let binding=capture_io::producer_binding(&run)?.clone();
        if candidate.effect.as_ref()!=Some(&candidate.reserved_effect)||candidate.kali_binding.as_ref()!=Some(&binding)||binding.attempt_id!=candidate.attempt_id||binding.assessment_id!=candidate.assessment_id||binding.input_refs!=candidate.input_refs{return Err("work_owned_operation".into());}
        capture_io::require_owned(store,&binding)?;
        let cancel=Arc::new(AtomicBool::new(false));*active=Some(Active{run_id:run.id.clone(),assessment_id:run.assessment_id.clone(),cancel:cancel.clone()});drop(active);
        let result=(||{
            let pending=records.iter().filter(|r|(r.id==run.id||r.reconciliation_of.as_deref()==Some(&run.id))&&work::unresolved(r)).cloned().collect::<Vec<_>>();
            let already_collected=binding.operation==KaliOperation::Scan&&self.complete_capture_copy(store,&run)?;
            if pending.iter().any(|r|r.launch_attempted)||(binding.operation==KaliOperation::Scan&&run.launch_attempted&&!already_collected){if identity()?!=binding.environment_id{return Err("environment_identity_changed".into());}}
            for mut target in pending {if !self.probe_work_session(&run,&mut target,&cancel)?{return Err("work_reconciliation_required".into());}if target.id==run.id{run=target;}}
            run=self.find(&run.id)?;
            if work::unresolved(&run)||self.runs()?.iter().any(|r|r.reconciliation_of.as_deref()==Some(&run.id)&&work::unresolved(r)){return Err("work_reconciliation_required".into());}
            if let Some(capture_io::KaliRunContext::Producer{receipt_refs,..})=&run.work_context{
                for receipt in receipt_refs {
                    let last=store.snapshot()?.data_use_receipts.iter().find(|r|r.id==receipt.id&&r.digest==receipt.digest).and_then(|r|r.events.last()).map(|e|e.outcome).ok_or("data_receipt_missing")?;
                    if matches!(last,crate::data_policy::DataUseOutcome::Admitted|crate::data_policy::DataUseOutcome::Dispatched|crate::data_policy::DataUseOutcome::Unknown){
                        let outcome=if !run.launch_attempted{crate::data_policy::DataUseOutcome::NotSent}else if run_has_observation(&run){crate::data_policy::DataUseOutcome::Completed}else if last==crate::data_policy::DataUseOutcome::Admitted{crate::data_policy::DataUseOutcome::NotSent}else{crate::data_policy::DataUseOutcome::Failed};
                        self.data_policy.finish(store,receipt,outcome,vec![])?;
                    }
                }
            }
            let collected=if already_collected{true}else if binding.operation==KaliOperation::Scan&&run.launch_attempted{self.collect_spool(store,&mut run,&cancel).is_ok()}else{binding.operation==KaliOperation::Hash};
            let clean=!self.runs()?.iter().any(|r|(r.id==run.id||r.reconciliation_of.as_deref()==Some(&run.id))&&work::unresolved(r));
            if clean&&run_has_observation(&run)&&run.evidence_id.is_none(){run.evidence_id=Some(model::id());self.save(&run)?;}
            store.work_transaction(|conn,s|{let completion=finalize_locked(conn,s,&run,&binding,clean,collected)?;commit(conn,s,permit,&completion)?;Ok(completion)})
        })();
        *self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?=None;result
    }
    pub(crate) fn validate_queued_evidence(&self,store:&Store,expected:&WorkKaliExpectation,review:&DataReviewRef,sensitivity:Sensitivity,acknowledge_sensitive:bool,redactor:&dyn crate::data_projection::DataRedactor)->Result<QueuedKaliGrant,String>{
        let ResultRef::Evidence{id,raw_sha256}=&expected.input else{return Err("work_input_changed".into());};
        let operation=match expected.operation{KaliOperation::Hash=>"hash",KaliOperation::Scan=>"scan"};
        let prepared=self.reviewed_kali_data(store,Some(review),&expected.assessment_id,id,operation,&expected.environment_id,redactor)?;
        if prepared.spec.scope_ref.as_ref()!=Some(&expected.scope_ref)||prepared.package_sensitivity!=sensitivity
            ||prepared.spec.requests.len()!=1||prepared.spec.requests[0].input_sha256!=*raw_sha256{return Err("work_input_changed".into());}
        if sensitivity==Sensitivity::Sensitive&&!acknowledge_sensitive{return Err("data_sensitive_confirmation_required".into());}
        Ok(QueuedKaliGrant{prepared,expected:expected.clone(),acknowledge_sensitive})
    }

    pub(crate) fn run_queued_evidence(&self,store:&Store,token:crate::work_supervisor::QueuedAttemptToken,grant:QueuedKaliGrant,identity:impl FnOnce()->Result<String,String>,ai:&crate::ai::AiService)->Result<WorkKaliCompletion,String>{
        use crate::work_supervisor as supervisor;
        let mut permit=self.arbiter.claim_queued(store,token)?;
        let mut bound=false;let mut run_saved:Option<BridgeRun>=None;
        let result=(||{
            let context=store.work_read(|conn,s|supervisor::validate_queued_kali_locked(conn,s,&permit,&grant.expected))?;
            if context.data_review_ref!=grant.prepared.review{return Err("work_input_changed".into());}
            if identity()?!=grant.expected.environment_id{return Err("environment_identity_changed".into());}
            self.recheck_kali_data(store,&grant.prepared,&ai.data_redactor())?;
            let mut active=self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?;
            if active.is_some()||self.has_unresolved(){return Err("work_busy".into());}
            let slots=match grant.expected.operation{KaliOperation::Scan=>vec![(CaptureStream::ToolStdout,65536),(CaptureStream::SupervisorDiagnostic,4096)],KaliOperation::Hash=>vec![(CaptureStream::AdapterRecords,4096)]};
            let binding=KaliWorkBinding{schema:"openkoda.kali-work-binding.v1".into(),attempt_id:context.attempt_id,assessment_id:grant.expected.assessment_id.clone(),run_id:context.reserved_run_id,environment_id:grant.expected.environment_id.clone(),owner_profile_fingerprint:capture_io::profile_fingerprint(store)?,operation:grant.expected.operation,input_refs:vec![grant.expected.input.clone()],origin_refs:grant.prepared.spec.selection.origins.clone(),executor_source_sha256:hash(SOURCE),catalog_version:KALI_CATALOG_VERSION.into(),catalog_sha256:catalog_digest(),parser_version:if grant.expected.operation==KaliOperation::Scan{Some(KALI_PARSER_VERSION.into())}else{None},captures:slots.into_iter().map(|(stream,max_bytes)|KaliCaptureSlot{capture_id:model::id(),stream,max_bytes}).collect()};binding.validate()?;
            let tool=if binding.operation==KaliOperation::Scan{"scan_bytes"}else{"hash_bytes"};
            let targets=source_refs(store,&binding.assessment_id,Some(&grant.prepared.evidence_id))?;
            let mut run=BridgeRun{id:binding.run_id.clone(),assessment_id:binding.assessment_id.clone(),environment_id:binding.environment_id.clone(),tool:tool.into(),status:"starting".into(),started_at:model::now(),finished_at:None,source_sha256:binding.executor_source_sha256.clone(),evidence_id:None,error:None,cleanup_confirmed:false,launch_attempted:false,dispatch_before_unix_seconds:crate::scope::dispatch_deadline(store,&grant.expected.scope_ref,30)?,source_evidence_id:Some(grant.prepared.evidence_id.clone()),scope_ref:Some(grant.expected.scope_ref.clone()),source_target_refs:targets.clone(),reconciliation_of:None,work_context:None};
            let admitted=self.data_policy.with_admission_checked(store,&grant.prepared.review,&grant.prepared.spec,Some(crate::data_policy::OperationalScope{assessment_id:&binding.assessment_id,method:scope_method(tool)?,expected:&grant.expected.scope_ref,targets:&targets}),crate::data_policy::EffectLink{kind:"kali_run".into(),id:run.id.clone()},grant.acknowledge_sensitive,
                ||store.work_read(|conn,s|supervisor::validate_queued_kali_locked(conn,s,&permit,&grant.expected).map(|_|())),
                |admission|{run.work_context=Some(capture_io::KaliRunContext::Producer{binding:binding.clone(),spool_manifest:None,spool_absence:None,receipt_refs:vec![admission.receipt_ref.clone()],observed:None});self.save(&run)?;Ok(admission.receipt_ref.clone())})?;
            let receipt=match admitted{crate::data_policy::AdmissionResult::Admitted(r)=>r,crate::data_policy::AdmissionResult::AlreadyUsed(_)=>return Err("data_review_used".into())};
            run_saved=Some(run.clone());
            let effect=EffectRef::KaliRun{id:run.id.clone()};
            store.work_transaction(|conn,s|{
                supervisor::validate_queued_kali_locked(conn,s,&permit,&grant.expected)?;
                supervisor::bind_effect_locked(conn,&permit,&effect)?;
                crate::capture::create_kali_locked(conn,&binding)?;
                supervisor::mark_kali_running_locked(conn,&permit,std::slice::from_ref(&receipt),&binding.capture_ids(),&binding)
            })?;
            permit.acknowledge_bound(effect);bound=true;
            let cancel=Arc::new(AtomicBool::new(false));
            *active=Some(Active{run_id:run.id.clone(),assessment_id:run.assessment_id.clone(),cancel:cancel.clone()});drop(active);
            let args=queued_arguments(&store.snapshot()?,&binding)?;
            let deadline=crate::scope::deadline(store,&grant.expected.scope_ref,Duration::from_secs(45))?;
            self.recheck_kali_data(store,&grant.prepared,&ai.data_redactor())?;
            check_work(store,&permit)?;
            if binding.operation==KaliOperation::Scan {
                let unavailable=match self.launcher.queued_scan_available(&binding.environment_id) {
                    Ok(true)=>None,
                    Ok(false)=>Some("O ripgrep não está disponível no Kali ativo. Confira o pacote em Kali e ferramentas e prepare uma nova análise.".to_owned()),
                    Err(_)=>Some("Não foi possível verificar o ripgrep no Kali ativo. Execute o diagnóstico e prepare uma nova análise.".to_owned()),
                };
                if let Some(reason)=unavailable {
                    run.status="failed".into();run.error=Some(reason);run.cleanup_confirmed=true;run.finished_at=Some(model::now());
                    self.save(&run)?;run_saved=Some(run.clone());
                    self.data_policy.finish(store,&receipt,crate::data_policy::DataUseOutcome::NotSent,vec![])?;
                    return settle_queued(store,&mut permit,&run,&binding,true,false);
                }
                check_wait(deadline,&cancel)?;
                check_work(store,&permit)?;
            }
            run.launch_attempted=true;self.save(&run)?;run_saved=Some(run.clone());
            let mut transport=self.launcher.launch(&run,&cancel)?;
            run.status="running".into();self.save(&run)?;
            let mut state=ToolCallState::NotSent;
            let mut observation=exchange_tracked(&mut *transport,&run,tool,&args,deadline,&cancel,&mut state,||{
                self.recheck_kali_data(store,&grant.prepared,&ai.data_redactor())?;
                self.data_policy.before_dispatch(store,&receipt,&grant.prepared.spec,Some(crate::data_policy::OperationalScope{assessment_id:&run.assessment_id,method:scope_method(tool)?,expected:&grant.expected.scope_ref,targets:&targets}),||check_work(store,&permit))
            });
            let finish=transport.finish(&run,Instant::now()+Duration::from_secs(5));
            if observation.is_err()&&tool=="scan_bytes"&&cancel.load(Ordering::SeqCst)&&state==ToolCallState::Pending&&finish.tool_dispatched&&finish.cleanup_confirmed{if let Some(reply)=finish.tool_reply{if let Ok(partial)=tool_response(reply,&run,tool,&args){observation=Ok(partial);}}}
            run.cleanup_confirmed=finish.cleanup_confirmed;run.finished_at=if finish.cleanup_confirmed{Some(model::now())}else{None};
            run.status=if !finish.cleanup_confirmed{"unknown"}else if cancel.load(Ordering::SeqCst){"canceled"}else if observation.is_ok(){"completed"}else{"failed"}.into();
            run.error=if observation.is_ok()&&finish.cleanup_confirmed{None}else{Some("Operação Kali incompleta; a sessão registrada não será repetida.".into())};
            if let Some(capture_io::KaliRunContext::Producer{observed,..})=&mut run.work_context{*observed=observation.ok();}
            self.save(&run)?;run_saved=Some(run.clone());
            let receipt_outcome=if !finish.tool_dispatched{crate::data_policy::DataUseOutcome::NotSent}else{match run.status.as_str(){"completed"=>crate::data_policy::DataUseOutcome::Completed,"canceled"=>crate::data_policy::DataUseOutcome::Canceled,"unknown"=>crate::data_policy::DataUseOutcome::Unknown,_=>crate::data_policy::DataUseOutcome::Failed}};
            self.data_policy.finish(store,&receipt,receipt_outcome,vec![])?;
            let collected=if binding.operation==KaliOperation::Scan&&run.cleanup_confirmed{
                // Cancellation stops the producer. Closing its known spool is
                // a distinct bounded phase, reachable by a further stop signal.
                let collect_cancel=Arc::new(AtomicBool::new(false));
                if let Some(active)=self.active.lock().map_err(db_error)?.as_mut(){if active.run_id==run.id{active.cancel=collect_cancel.clone();}else{return Err("work_owned_operation".into());}}
                self.collect_spool(store,&mut run,&collect_cancel).is_ok()
            }else{binding.operation==KaliOperation::Hash};
            run_saved=Some(run.clone());
            let family_clean=run.cleanup_confirmed&&!self.runs()?.iter().any(|r|r.reconciliation_of.as_deref()==Some(&run.id)&&work::unresolved(r));
            if family_clean&&run_has_observation(&run){run.evidence_id=Some(model::id());self.save(&run)?;}
            settle_queued(store,&mut permit,&run,&binding,family_clean,collected)
        })();
        *self.active.lock().map_err(|_|"Supervisor MCP indisponível.")?=None;
        if result.is_err(){
            if let Some(mut run)=run_saved {
                if let Ok(durable)=self.find(&run.id){run=durable;}
                if !run.launch_attempted{run.status="failed".into();run.cleanup_confirmed=true;run.finished_at=Some(model::now());run.error=Some("Admissão interrompida antes do despacho Linux.".into());let _=self.save(&run);}
                else if work::unresolved(&run){run.status="unknown".into();run.finished_at=None;let _=self.save(&run);}
                if let Some(capture_io::KaliRunContext::Producer{receipt_refs,..})=&run.work_context{for receipt in receipt_refs{let _=self.data_policy.finish(store,receipt,if run.launch_attempted{crate::data_policy::DataUseOutcome::Unknown}else{crate::data_policy::DataUseOutcome::NotSent},vec![]);}}
            }
            self.arbiter.finish(permit,if bound{EffectTermination::Unknown}else{EffectTermination::NotStarted})?;
        }
        result
    }
}

fn check_work(store:&Store,permit:&crate::work_supervisor::EffectPermit<'_>)->Result<(),String>{store.work_read(|conn,_|match crate::work_supervisor::work_control_locked(conn,permit)?{crate::work_supervisor::WorkControl::Active=>Ok(()),crate::work_supervisor::WorkControl::Paused=>Err("work_paused".into()),crate::work_supervisor::WorkControl::Canceled=>Err("work_canceled".into())})}
fn settle_queued(store:&Store, permit:&mut crate::work_supervisor::EffectPermit<'_>, run:&BridgeRun, binding:&KaliWorkBinding, clean:bool, collected:bool)->Result<WorkKaliCompletion,String>{
    let mut completion=store.work_transaction(|conn,s|{
        let completion=finalize_locked(conn,s,run,binding,clean,collected)?;
        crate::work_supervisor::finish_kali_locked(conn,s,permit,&completion)?;
        crate::work_supervisor::finish_permit_locked(conn,permit,completion.termination)?;
        Ok(completion)
    })?;
    permit.acknowledge_finished();
    completion.capture_ids=binding.capture_ids();
    Ok(completion)
}
fn run_has_observation(run:&BridgeRun)->bool{matches!(&run.work_context,Some(capture_io::KaliRunContext::Producer{observed:Some(_),..}))}
fn queued_arguments(s:&model::Snapshot,binding:&KaliWorkBinding)->Result<Value,String>{
    let ResultRef::Evidence{id,raw_sha256}=&binding.input_refs[0] else{return Err("work_input_changed".into());};
    let evidence=s.assessments.iter().find(|a|a.id==binding.assessment_id).and_then(|a|a.evidence.iter().find(|e|e.id==*id)).ok_or("work_input_changed")?;
    if evidence.sha256!=*raw_sha256||hash(&evidence.content)!=*raw_sha256||evidence.content.len()>MAX_BYTES{return Err("work_input_changed".into());}
    Ok(json!({"base64":STANDARD.encode(evidence.content.as_bytes()),"expectedSha256":raw_sha256}))
}

fn finalize_locked(conn:&Connection,s:&mut model::Snapshot,run:&BridgeRun,binding:&KaliWorkBinding,clean:bool,collected:bool)->Result<WorkKaliCompletion,String>{
    let Some(capture_io::KaliRunContext::Producer{receipt_refs,observed,spool_manifest,..})=&run.work_context else{return Err("capture_integrity".into());};
    let mut completion=WorkKaliCompletion{effect:EffectRef::KaliRun{id:run.id.clone()},termination:if clean{EffectTermination::Stopped}else{EffectTermination::Unknown},outcome:if clean{AttemptState::Failed}else{AttemptState::Unknown},results:vec![],receipt_refs:receipt_refs.clone(),assessment_id:run.assessment_id.clone(),capture_ids:binding.capture_ids()};
    if !clean{return Ok(completion);}
    let mut capture_complete=collected;
    for slot in &binding.captures {
        let m=crate::capture::load_locked(conn,&slot.capture_id)?;
        if m.state!=crate::capture::CaptureState::Open{return Err("capture_integrity".into());}
        match slot.stream {
            CaptureStream::ToolStdout=>{
                let bytes=crate::capture::preserved_locked(conn,&m)?;
                let full=spool_manifest.as_ref().filter(|manifest|collected&&manifest.body.preserved_bytes==m.preserved_bytes);
                let (count,loss,interrupted)=if let Some(manifest)=full{
                    manifest.validate(binding)?;
                    if manifest.body.encoding==crate::capture::CaptureEncoding::Unknown||std::str::from_utf8(&bytes).is_err(){crate::capture::mark_encoding_unknown_locked(conn,&m.id)?;}
                    let interrupted=manifest.body.phase!="sealed";
                    let loss=manifest.body.loss_reason.or(if interrupted{Some(crate::capture::CaptureLossReason::Interrupted)}else{None});
                    capture_complete&=!interrupted&&loss.is_none()&&manifest.body.observed_bytes==Some(m.preserved_bytes);
                    (manifest.body.observed_bytes,loss,interrupted)
                }else if !run.launch_attempted && m.preserved_bytes==0 {
                    (Some(0),None,false)
                }else{capture_complete=false;(None,Some(crate::capture::CaptureLossReason::Interrupted),true)};
                if std::str::from_utf8(&bytes).is_err(){crate::capture::mark_encoding_unknown_locked(conn,&m.id)?;capture_complete=false;}
                crate::capture::seal_locked(conn,&m.id,count,loss,interrupted)?;
            },
            CaptureStream::SupervisorDiagnostic|CaptureStream::AdapterRecords=>{
                let record=if slot.stream==CaptureStream::AdapterRecords{json!({"schema":"openkoda.kali-hash-observation.v1","runId":run.id,"result":observed})}else{json!({"schema":"openkoda.kali-supervisor-diagnostic.v1","runId":run.id,"producerStatus":run.status,"producerCleanupConfirmed":run.cleanup_confirmed,"collectionComplete":collected,"stderr":"discarded"})};
                let bytes=serde_json::to_vec(&record).map_err(db_error)?;if bytes.len() as u64>slot.max_bytes{return Err("work_capture_quota".into());}
                crate::capture::append_stream_locked(conn,&m.id,0,&bytes)?;crate::capture::seal_locked(conn,&m.id,Some(bytes.len() as u64),None,false)?;
            },_=>return Err("capture_integrity".into()),
        }
    }
    if let (Some(observed),Some(evidence_id))=(observed,&run.evidence_id){
        let args=queued_arguments(s,binding)?;
        tool_response(json!({"jsonrpc":"2.0","id":3,"result":{"isError":false,"structuredContent":observed}}),run,&run.tool,&args)?;
        let mut document=json!({"runId":run.id,"environmentId":run.environment_id,"sourceSha256":run.source_sha256,"sourceEvidenceId":run.source_evidence_id,"scopeRef":run.scope_ref,"protocolVersion":PROTOCOL,"tool":run.tool,"result":observed,"limits":"MCP diagnóstico/hash/análise lexical fixa de bytes selecionados; candidatos podem ser falsos positivos. Nenhuma execução do alvo, shell, rede ou acesso a seus caminhos."});
        if crate::scope_targets::is_combined(s,&run.assessment_id){document["sourceTargetRefs"]=json!(run.source_target_refs);}
        let content=serde_json::to_string_pretty(&document).map_err(db_error)?;let digest=hash(&content);
        let a=s.assessments.iter_mut().find(|a|a.id==run.assessment_id).ok_or("work_input_changed")?;
        if let Some(old)=a.evidence.iter().find(|e|e.id==*evidence_id){if old.content!=content||old.sha256!=digest{return Err("work_result_missing".into());}}else{a.evidence.push(Evidence{id:evidence_id.clone(),name:format!("kali-mcp-{}-{}.json",run.tool,run.id),sha256:digest.clone(),content,created_at:run.finished_at.clone().unwrap_or_else(model::now)});}
        completion.results.push(ResultRef::Evidence{id:evidence_id.clone(),raw_sha256:digest});
        completion.outcome=if run.status=="canceled"{AttemptState::Canceled}else if run.status=="completed"&&capture_complete&&(run.tool!="scan_bytes"||observed["termination"]=="completed"){AttemptState::Completed}else{AttemptState::Partial};
        a.updated_at=model::now();
    }else if run.status=="canceled"{completion.outcome=AttemptState::Canceled;}
    Ok(completion)
}
