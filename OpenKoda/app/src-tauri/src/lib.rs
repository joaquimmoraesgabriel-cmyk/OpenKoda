pub mod core;
mod close_guard;
pub mod model;
pub mod ai;
pub mod environment;
pub mod executor;
pub mod network;
pub mod source_scan;
pub mod source_context;
pub mod mcp;
pub mod recovery;
pub mod storage;
pub mod kali_bridge;
pub mod remediation;
pub mod official_image;
pub mod activation;
pub mod retention;
pub mod project_purge;
pub mod http_validation;
pub mod scope;
pub mod scope_targets;
pub mod local_scope;
pub mod evidence_lineage;
pub mod data_policy;
pub mod data_projection;
pub mod credential_refs;
mod data_host;
pub(crate) mod workflow;
pub(crate) mod work_store;
pub(crate) mod work_supervisor;
pub(crate) mod capture;
mod capture_access;
mod work_runtime;
mod work_disposition;
#[cfg(test)]
mod storage_workflow_tests;
#[cfg(test)]
mod workflow_tests;
#[cfg(test)]
mod capture_access_tests;

use std::sync::Arc;
use tauri::Manager;
use core::{Request, Store};

struct Services { close_gate: Arc<close_guard::CloseGate>, ai: ai::AiService, environment: Arc<environment::EnvironmentManager>, executor: executor::Executor, recovery:recovery::RecoveryService, kali:kali_bridge::KaliBridge, remediation:remediation::RemediationService, activation:activation::ActivationService, retention:retention::RetentionService, project_purge:project_purge::ProjectPurgeService, data_policy:Arc<data_policy::DataPolicyService>, workflow:workflow::WorkflowService, supervisor:work_supervisor::WorkSupervisor, work_wakeup:work_runtime::WakeGate, environment_gate: std::sync::RwLock<()> }

// Check under the operation lease, so a purge cannot begin between this test
// and another mutation. Read-only views remain available for human inspection.
fn ensure_purge_access(services:&Services,action:&str)->Result<(),String>{
    if matches!(action,"work.plan.get"|"work.queue"|"work.events"|"work.pause"|"work.cancel"|"work.cancelPlan"|"work.reconcile"|"work.aiLocalClosure.list"|"work.aiLocalClosure.prepare"|"work.aiLocalClosure.commit"|"capture.list"|"capture.status"){return Ok(());}
    if matches!(action,"data.policy.status"|"data.policy.revoke"|"data.classification.subjects"|"credential.reference.list"|"credential.reference.revoke"|"data.session.select"|"data.review.cancel"){return Ok(());}
    if matches!(action,"project.targetOptions"|"assessment.combinedOptions"|"evidence.origins"){return Ok(());}
    if project_purge::supports(action)||matches!(action,"snapshot"|"assessment.scopeStatus"|"assessment.scopeOptions"|"ai.status"|"ai.history"|"ai.context"|"ai.investigation.list"|"ai.budget.status"|"ai.key.remove"|"environment.status"|"kali.mcp.status"|"remediation.list"|"remediation.get"|"finding.httpStatus"|"activation.status"|"activation.list"|"activation.cancel"|"backup.list"|"backup.verify"|"retention.inventory") {return Ok(());}
    if services.project_purge.has_unresolved()? {return Err("Há um expurgo de projeto incompleto. Inspecione o registro e conclua ou encerre explicitamente a exclusão parcial antes de iniciar outra alteração.".into());}
    Ok(())
}

// Call only while holding the same lease that excludes activation.schedule.
fn allow_new_work(services:&Services)->Result<(),String>{
    if services.activation.blocks_new_effects()? {
        Err("Há uma troca de perfil agendada. Cancele o agendamento ou reinicie o aplicativo antes de iniciar novas operações.".into())
    }else{Ok(())}
}

#[tauri::command]
async fn request(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<Store>>,
    services: tauri::State<'_, Arc<Services>>,
    request: Request,
) -> Result<serde_json::Value, String> {
    if window.label() != "main" {
        return Err("Janela não autorizada.".into());
    }
    let admission = services.close_gate.admit(close_guard::Activity::request(&request.action, &request.payload))?;
    let store = state.inner().clone();
    let services = services.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _admission = admission;
        if request.action == "host.connectivity" {
            return environment::connectivity::observe(&request.payload);
        }
        // Support is an allowlisted metadata-only document, even when the credential vault is unavailable.
        if matches!(request.action.as_str(), "support.review" | "support.export") {
            let _lease = services.environment_gate.try_read()
                .map_err(|_| "Diagnóstico indisponível durante manutenção local.")?;
            if request.action == "support.export" {
                ensure_purge_access(&services, &request.action)?;
                allow_new_work(&services)?;
            }
            return services.recovery.support(&request.action, request.payload);
        }
        let public_action=request.action.clone();
        let result=(||{
        if capture_access::supports(&request.action){
            let _lease=services.environment_gate.try_read().map_err(|_|"work_busy: captura indisponível durante manutenção local.")?;
            ensure_purge_access(&services,&request.action)?;
            return capture_access::handle(&request.action,request.payload,&store,&services.ai.data_redactor());
        }
        if workflow::supports(&request.action){
            if request.action=="work.aiLocalClosure.commit"{
                let _lease=services.environment_gate.try_write().map_err(|_|"work_busy: aguarde o término das operações locais antes de encerrar esta tentativa.")?;
                ensure_purge_access(&services,&request.action)?;
                let environment_id=services.environment.data_review_identity().ok();
                return services.workflow.handle_backend(&request.action,request.payload,&store,&services.executor,&services.supervisor,&services.ai,&services.kali,environment_id.as_deref(),&||services.environment.executor_owned_identity(),&services.ai.data_redactor());
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"work_busy: trabalho indisponível durante manutenção local.")?;
            ensure_purge_access(&services,&request.action)?;
            if !matches!(request.action.as_str(),"work.plan.get"|"work.queue"|"work.events"|"work.pause"|"work.cancel"|"work.cancelPlan"|"work.reconcile"|"work.aiLocalClosure.list"|"work.aiLocalClosure.prepare"){allow_new_work(&services)?;}
            let environment_id=services.environment.data_review_identity().ok();
            return services.workflow.handle_backend(&request.action,request.payload,&store,&services.executor,&services.supervisor,&services.ai,&services.kali,environment_id.as_deref(),&||services.environment.executor_owned_identity(),&services.ai.data_redactor());
        }
        if data_policy::DataPolicyService::supports(&request.action)||credential_refs::supports(&request.action){
            let _lease=services.environment_gate.try_read().map_err(|_|"Dados indisponíveis durante manutenção local.")?;
            ensure_purge_access(&services,&request.action)?;
            if data_policy::DataPolicyService::mutates(&request.action)&&!matches!(request.action.as_str(),"data.policy.revoke"|"credential.reference.revoke"|"data.review.cancel"|"data.session.select"){allow_new_work(&services)?;}
            let redactor=services.ai.data_redactor();
            let provider=services.ai.provider_identity(&store).ok();
            return services.data_policy.handle(&request.action,request.payload,&store,&redactor,&data_host::JournalResolver{store:&store},provider.as_ref());
        }
        if request.action=="kali.data.prepare"{
            let _lease=services.environment_gate.try_read().map_err(|_|"Revisão de dados indisponível durante manutenção.")?;
            ensure_purge_access(&services,&request.action)?;allow_new_work(&services)?;
            let identity=services.environment.data_review_identity()?;
            return services.kali.handle_data_prepare(&request.action,request.payload,&store,&identity,&services.ai.data_redactor());
        }
        if request.action=="evidence.origins" {
            let _lease=services.environment_gate.try_read().map_err(|_|"Origem indisponível durante manutenção ou escrita local.")?;
            ensure_purge_access(&services,&request.action)?;
            return evidence_lineage::projection(&store,request.payload);
        }
        if scope_targets::catalog_supports(&request.action) {
            if scope_targets::catalog_mutates(&request.action) {
                let _lease=services.environment_gate.try_write().map_err(|_|"Aguarde a operação antes de alterar o catálogo de alvos.")?;
                ensure_purge_access(&services,&request.action)?;allow_new_work(&services)?;
                return scope_targets::handle_catalog(&request.action,request.payload,&store);
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"Catálogo indisponível durante manutenção ou escrita local.")?;
            ensure_purge_access(&services,&request.action)?;
            return scope_targets::handle_catalog(&request.action,request.payload,&store);
        }
        if scope::supports(&request.action) {
            if request.action == scope::COMBINED_DERIVE_CREATE {
                return scope::dispatch_combined_derivation(
                    request.payload, &store, &services.environment_gate, || {
                        ensure_purge_access(&services, scope::COMBINED_DERIVE_CREATE)?;
                        allow_new_work(&services)
                    },
                );
            }
            if scope::mutates(&request.action)&&request.action!="assessment.scopeRevoke" {
                let _lease=services.environment_gate.try_write().map_err(|_|"scope_busy: aguarde ou reconcilie o trabalho antes de revisar o escopo.")?;
                ensure_purge_access(&services,&request.action)?;allow_new_work(&services)?;
                return scope::handle(&request.action,request.payload,&store,str::to_owned);
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"scope_busy: revisão indisponível durante manutenção ou escrita local.")?;
            ensure_purge_access(&services,&request.action)?;
            // Revocation remains possible while long inference/execution holds
            // a read lease, and after profile activation has been scheduled.
            if request.action=="assessment.scopeRevoke" {return services.ai.with_revocation_redactor(|redact|scope::handle(&request.action,request.payload,&store,redact));}
            return scope::handle(&request.action,request.payload,&store,str::to_owned);
        }
        if http_validation::supports(&request.action) {
            let _lease=services.environment_gate.try_read().map_err(|_|"Revisão HTTP indisponível durante manutenção ou recuperação.")?;
            ensure_purge_access(&services,&request.action)?;
            if http_validation::mutates(&request.action){allow_new_work(&services)?;}
            return http_validation::handle(&request.action,request.payload,&store,&services.ai);
        }
        if source_context::supports(&request.action) {
            let _lease=services.environment_gate.try_write().map_err(|_|"Leitura de fontes indisponível durante outra operação.")?;
            ensure_purge_access(&services,&request.action)?;
            allow_new_work(&services)?;
            if services.kali.has_unresolved()||services.remediation.has_unresolved()?||store.snapshot()?.assessments.iter().any(|a|core::active(&a.status)) {
                return Err("Encerre ou reconcilie operações antes de revisar uma captura de fontes.".into());
            }
            return source_context::handle(&request.action,request.payload,&store,&services.ai);
        }
        if project_purge::supports(&request.action) {
            if matches!(request.action.as_str(),"projectPurge.status"|"projectPurge.get") {
                ensure_purge_access(&services,&request.action)?;
                return services.project_purge.handle(&request.action,request.payload,&store);
            }
            let _lease=services.environment_gate.try_write().map_err(|_|"Expurgo indisponível durante outra operação.")?;
            ensure_purge_access(&services,&request.action)?;
            if matches!(request.action.as_str(),"projectPurge.plan"|"projectPurge.apply") {
                allow_new_work(&services)?;
                // The purge service separately validates every backend journal
                // and the explicit acknowledgment of legacy unknown AI usage.
                // A durable workflow lease/job still blocks deletion; history
                // deletion neither proves remote termination nor clears charges.
                store.work_read(work_supervisor::ensure_quiescent_locked)?;
                if services.environment.has_unresolved_maintenance()?||services.kali.has_unresolved()||services.remediation.has_unresolved()?||services.retention.has_unresolved()?||store.snapshot()?.assessments.iter().any(|a|core::active(&a.status)) {
                    return Err("Encerre ou reconcilie operações e manutenção antes de revisar ou continuar um expurgo.".into());
                }
            }
            return services.project_purge.handle(&request.action,request.payload,&store);
        }
        if activation::supports(&request.action) {
            if activation::mutates(&request.action) {
                let _lease=services.environment_gate.try_write().map_err(|_|"Troca de perfil bloqueada por operação ativa.")?;
                ensure_purge_access(&services,&request.action)?;
                if request.action!="activation.cancel"&&services.environment.has_unresolved_maintenance()? {
                    return Err("Reconcilie a manutenção do ambiente antes de agendar uma troca de perfil.".into());
                }
                if request.action!="activation.cancel"{services.supervisor.shared_arbiter().ensure_quiescent(&store)?;}
                if request.action!="activation.cancel"&&services.retention.has_unresolved()?{return Err("Inspecione o expurgo interrompido antes de trocar de perfil.".into());}
                if request.action!="activation.cancel"&&(services.kali.has_unresolved()||store.snapshot()?.assessments.iter().any(|a|core::active(&a.status))) {
                    return Err("Encerre ou reconcilie operações ativas antes de preparar uma troca de perfil.".into());
                }
                return services.activation.handle(&request.action,request.payload,&store);
            }
            return services.activation.handle(&request.action,request.payload,&store);
        }
        if retention::supports(&request.action) {
            let _lease=services.environment_gate.try_write().map_err(|_|"Retenção indisponível durante outra operação.")?;
            ensure_purge_access(&services,&request.action)?;
            if matches!(request.action.as_str(),"retention.plan"|"retention.apply") {
                allow_new_work(&services)?;
                services.supervisor.shared_arbiter().ensure_quiescent(&store)?;
                if services.environment.has_unresolved_maintenance()?||services.kali.has_unresolved()||services.remediation.has_unresolved()?||store.snapshot()?.assessments.iter().any(|a|core::active(&a.status)) {
                    return Err("Reconcilie operações e manutenção antes de revisar uma exclusão.".into());
                }
            }
            let identity=||services.environment.executor_owned_identity();
            let spool=retention::KaliRetention{bridge:&services.kali,store:&store,identity:&identity};
            return services.retention.handle_with_spool(&request.action,request.payload,&store,Some(&spool));
        }
        if remediation::supports(&request.action) {
            if matches!(request.action.as_str(),"remediation.apply"|"remediation.retest"|"remediation.confirm"|"remediation.reconcile") {
                let _lease=services.environment_gate.try_write().map_err(|_|"Correção bloqueada por operação ativa. Aguarde o término e revise novamente.")?;
                ensure_purge_access(&services,&request.action)?;
                allow_new_work(&services)?;
                services.supervisor.shared_arbiter().ensure_quiescent(&store)?;
                if services.kali.has_unresolved()||store.snapshot()?.assessments.iter().any(|a|core::active(&a.status)) {
                    return Err("Encerre ou reconcilie as operações ativas antes de aplicar ou confirmar uma correção.".into());
                }
                return services.remediation.handle(&request.action,request.payload,&store);
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"Revisão de correções indisponível durante manutenção ou backup.")?;
            ensure_purge_access(&services,&request.action)?;
            return services.remediation.handle(&request.action,request.payload,&store);
        }
        if kali_bridge::supports(&request.action) {
            if !kali_bridge::needs_identity(&request.action) {
                return services.kali.handle(&request.action,request.payload,&store,"");
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"MCP indisponível durante manutenção ou recuperação.")?;
            ensure_purge_access(&services,&request.action)?;
            allow_new_work(&services)?;
            return services.kali.handle_with_identity(&request.action,request.payload,&store,||services.environment.executor_owned_identity(),Some(&services.ai));
        }
        if ai::AiService::supports(&request.action) {
            if matches!(request.action.as_str(),"ai.budget.configure"|"ai.budget.resolveUnknown") {
                let _lease=services.environment_gate.try_write().map_err(|_|"Revisão de orçamento indisponível durante outra operação.")?;
                ensure_purge_access(&services,&request.action)?;
                allow_new_work(&services)?;
                return services.ai.handle(&request.action,request.payload,&store);
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"IA indisponível durante manutenção ou backup.")?;
            ensure_purge_access(&services,&request.action)?;
            if matches!(request.action.as_str(),"ai.ask"|"ai.test"|"ai.models"|"ai.operation.propose"|"ai.investigation.plan"|"ai.investigation.run"|"ai.budget.preview"){allow_new_work(&services)?;}
            return services.ai.handle(&request.action,request.payload,&store);
        }
        if recovery::supports(&request.action) {
            if matches!(request.action.as_str(),"backup.create"|"backup.prepare"|"backup.commit"|"backup.restore") {
                let _lease=services.environment_gate.try_write().map_err(|_|"Backup/recuperação bloqueado por operação ou manutenção ativa.")?;
                ensure_purge_access(&services,&request.action)?;
                services.supervisor.shared_arbiter().ensure_quiescent(&store)?;
                if services.retention.has_unresolved()?{return Err("Inspecione o expurgo interrompido antes de preparar backup ou recuperação.".into());}
                if services.kali.has_unresolved(){return Err("Reconcilie sessões MCP com término não confirmado antes de backup ou recuperação.".into());}
                if store.snapshot()?.assessments.iter().any(|a|core::active(&a.status)){return Err("Reconcilie avaliações ativas antes de criar ou preparar restauração de backup.".into());}
                if matches!(request.action.as_str(),"backup.prepare"|"backup.commit"){
                    allow_new_work(&services)?;
                    return services.recovery.handle_reviewed(&request.action,request.payload,&store,&services.data_policy,&services.ai.data_redactor());
                }
                return services.recovery.handle(&request.action,request.payload,&store);
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"Consulta ou relatório indisponível durante alteração de dados.")?;
            ensure_purge_access(&services,&request.action)?;
            if matches!(request.action.as_str(),"report.prepare"|"report.commit"){
                allow_new_work(&services)?;
                return services.recovery.handle_reviewed(&request.action,request.payload,&store,&services.data_policy,&services.ai.data_redactor());
            }
            return services.recovery.handle(&request.action,request.payload,&store);
        }
        if environment::supports(&request.action) {
            if environment::is_maintenance(&request.action) {
                let _lease=services.environment_gate.try_write().map_err(|_|"Manutenção bloqueada por operação ativa.")?;
                ensure_purge_access(&services,&request.action)?;
                allow_new_work(&services)?;
                // A degraded environment can prevent the MCP reconciler from
                // launching. The reviewed distro repair may stop that owned
                // environment; it preserves unknown MCP records for later audit.
                if matches!(request.action.as_str(),"environment.repair.plan"|"environment.repair.apply") {
                    if ai::has_unresolved_work(&store.root)? {return Err("work_unknown_effect: reconcilie o efeito de IA antes de reparar o ambiente.".into());}
                } else {
                    services.supervisor.shared_arbiter().ensure_quiescent(&store)?;
                }
                if services.kali.has_unresolved()&&!matches!(request.action.as_str(),"environment.repair.plan"|"environment.repair.apply"){return Err("Reconcilie sessões MCP com término não confirmado antes da manutenção.".into());}
                if matches!(request.action.as_str(),"environment.archive.activatePlan"|"environment.archive.activate"|"environment.removal.plan"|"environment.removal.detach")
                    && (services.retention.has_unresolved()? || services.remediation.has_unresolved()?) {
                    return Err("Reconcilie a retenção e a correção pendentes antes de alterar o executor.".into());
                }
                if store.snapshot()?.assessments.iter().any(|a| core::active(&a.status)) {
                    return Err("Manutenção bloqueada enquanto há avaliações ativas ou com efeito desconhecido.".into());
                }
                return services.environment.handle(&request.action,request.payload);
            }
            let _lease=services.environment_gate.try_read().map_err(|_|"Diagnóstico indisponível durante backup.")?;
            ensure_purge_access(&services,&request.action)?;
            return services.environment.handle(&request.action,request.payload);
        }
        if executor::supports(&request.action) {
            let _lease=services.environment_gate.try_read().map_err(|_|"Operação bloqueada durante manutenção.")?;
            ensure_purge_access(&services,&request.action)?;
            if !matches!(request.action.as_str(),"assessment.cancel"|"assessment.reconcile"|"plan.revoke"){allow_new_work(&services)?;}
            return services.executor.handle(&request.action,request.payload,&store);
        }
        if request.action=="mcp.request" {
            let _lease=services.environment_gate.try_read().map_err(|_|"Ferramentas indisponíveis durante manutenção.")?;
            ensure_purge_access(&services,&request.action)?;
            allow_new_work(&services)?;
            return mcp::handle_with_redactor(request.payload,&store,&services.executor,&services.ai.data_redactor());
        }
        let action=request.action.clone();
        let _lease=services.environment_gate.try_read().map_err(|_|"Dados temporariamente ocupados por manutenção ou backup.")?;
        ensure_purge_access(&services,&request.action)?;
        if matches!(action.as_str(),"assessment.create"|"evidence.import"){allow_new_work(&services)?;}
        let mut result=if action=="snapshot"{
            if !request.payload.as_object().is_some_and(|payload|payload.is_empty()){return Err("Esta ação não recebe campos.".into());}
            serde_json::json!({"settings":{"hasApiKey":services.ai.has_key()}})
        }else if action=="settings.save"{services.ai.save_settings(request.payload,&store)?}else{store.handle(request)?};
        if action=="snapshot" { result["settings"]["hasApiKey"]=serde_json::Value::Bool(services.ai.has_key()); }
        if action=="settings.save" { result["hasApiKey"]=serde_json::Value::Bool(services.ai.has_key()); }
        Ok(result)
        })();
        if work_runtime::should_wake(&public_action,result.is_ok()){work_runtime::wake(services.clone(),store.clone());}
        let redactor=services.ai.data_redactor();
        match result{
            Ok(value)=>data_host::project_response(&public_action,value,&store,&redactor),
            Err(error)=>Err(data_host::project_error(&error,&redactor)),
        }
    })
        .await
        .map_err(|_| "A operação local foi interrompida.".to_owned())?
}

pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() != "main" { return; }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let Some(services) = window.try_state::<Arc<Services>>() else { return };
                if let Some(activity) = services.close_gate.request_close() {
                    api.prevent_close();
                    if let Some(webview) = window.get_webview_window("main") {
                        // Only enum names and a parsed UUID enter this presentation signal.
                        // The WebView cannot acknowledge or override the host close decision.
                        if let Ok(detail) = serde_json::to_string(&activity) {
                            let _ = webview.eval(format!("window.dispatchEvent(new CustomEvent('openkoda-close-review',{{detail:{detail}}}))"));
                        }
                    }
                }
            }
        })
        .setup(|app| {
            let legacy = app.path().app_data_dir()?;
            #[cfg(debug_assertions)]
            let override_root = std::env::var_os("OPENKODA_DATA_DIR").map(std::path::PathBuf::from);
            #[cfg(not(debug_assertions))]
            let override_root:Option<std::path::PathBuf> = None;
            let storage=storage::open(&app.path().home_dir()?,&legacy,override_root.as_deref()).map_err(std::io::Error::other)?;
            let store = Store::open(storage.root.clone()).map_err(std::io::Error::other)?;
            let data_policy=Arc::new(data_policy::DataPolicyService::new());
            let supervisor=work_supervisor::WorkSupervisor::default();
            let environment=Arc::new(environment::EnvironmentManager::new(storage.base.clone()).map_err(std::io::Error::other)?);
            let services=Services{
                close_gate: Arc::default(),
                ai:ai::AiService::new_with_budget_root(store.root.clone(),storage.base.clone()).map_err(std::io::Error::other)?.with_data_policy(data_policy.clone()).with_arbiter(supervisor.shared_arbiter()),
                environment:environment.clone(),
                executor:executor::Executor::default().with_arbiter(supervisor.shared_arbiter()),
                recovery:recovery::RecoveryService::new(store.root.clone()).map_err(std::io::Error::other)?,
                kali:kali_bridge::KaliBridge::new(store.root.clone()).map_err(std::io::Error::other)?.with_environment(environment).with_data_policy(data_policy.clone()).with_arbiter(supervisor.shared_arbiter()),
                remediation:remediation::RemediationService::new(store.root.clone()).map_err(std::io::Error::other)?,
                activation:activation::ActivationService::new(storage.base.clone(),store.root.clone()).map_err(std::io::Error::other)?,
                retention:retention::RetentionService::new(store.root.clone()).map_err(std::io::Error::other)?,
                project_purge:project_purge::ProjectPurgeService::new(store.root.clone()).map_err(std::io::Error::other)?,
                data_policy,workflow:workflow::WorkflowService::default(),supervisor,work_wakeup:work_runtime::WakeGate::default(),environment_gate:std::sync::RwLock::new(())
            };
            services.supervisor.recover_after_boot(&store).map_err(std::io::Error::other)?;
            services.ai.recover_unbound_work(&store).map_err(std::io::Error::other)?;
            services.kali.recover_work_after_boot(&store).map_err(std::io::Error::other)?;
            app.manage(storage);
            app.manage(Arc::new(services));
            app.manage(Arc::new(store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![request])
        .run(tauri::generate_context!())
        .expect("Não foi possível iniciar o OpenKoda. Dados existentes foram preservados.");
}
