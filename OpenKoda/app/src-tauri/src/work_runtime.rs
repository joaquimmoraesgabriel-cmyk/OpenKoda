//! Process-local wakeups only. Durable admission and authority live in the host
//! services; losing a wakeup or restarting never grants approval or repeats I/O.
use crate::{Services,core::Store};
use std::sync::{Arc,Mutex};

#[derive(Default)]
pub(crate) struct WakeState{running:bool,pending:bool}
pub(crate) type WakeGate=Mutex<WakeState>;

pub(crate) fn should_wake(action:&str,success:bool)->bool{
    (success&&matches!(action,"work.operation.enqueue"|"work.resume.commit"|"work.reconcile"))
        ||matches!(action,"plan.run"|"assessment.reconcile"|"ai.ask"|"ai.test"|"ai.models"|"ai.operation.propose"|"ai.investigation.run"|"kali.mcp.diagnose"|"kali.mcp.hashEvidence"|"kali.mcp.scanEvidence"|"kali.mcp.reconcile")
        // A worker may have yielded to the exclusive environment gate after
        // enqueue. Releasing that gate must give already-reviewed work another
        // chance; all current scope, approval and resource checks still apply.
        ||crate::environment::is_maintenance(action)
        ||crate::activation::mutates(action)
        ||crate::scope_targets::catalog_mutates(action)
        ||(crate::scope::mutates(action)&&action!="assessment.scopeRevoke")
        ||crate::source_context::supports(action)
        ||matches!(action,"ai.budget.configure"|"ai.budget.resolveUnknown")
        ||matches!(action,"activation.cancel"|"backup.prepare"|"backup.commit"|"backup.restore"|"retention.plan"|"retention.apply"|"retention.reconcile"|"projectPurge.plan"|"projectPurge.apply"|"projectPurge.reconcile"|"projectPurge.closePartial"|"remediation.apply"|"remediation.retest"|"remediation.confirm"|"remediation.reconcile")
}

pub(crate) fn wake(services:Arc<Services>,store:Arc<Store>){
    // Maintenance completion can wake the dispatcher even when every plan is
    // paused or finished. Do not acquire the environment read lease for an
    // empty queue: that could reject the next reviewed maintenance commit.
    // Enqueue/resume provide their own wakeup after committing new work.
    if !matches!(has_queued_work(&store),Ok(true)){return;}
    let Ok(Some(admission)) = services.close_gate.admit(Some(crate::close_guard::Activity::queue(None))) else {return;};
    {
        let mut state=services.work_wakeup.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
        if state.running{state.pending=true;return;}
        state.running=true;state.pending=false;
    }
    tauri::async_runtime::spawn_blocking(move ||{
        loop{
            let drained=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||drain(&services,&store,&admission)));
            let mut state=services.work_wakeup.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
            // A panic is not evidence of effect termination. Permit Drop retains
            // its durable unknown state; no polling or retry follows here.
            if drained.is_err(){state.running=false;state.pending=false;break;}
            if state.pending{state.pending=false;drop(state);continue;}
            state.running=false;break;
        }
    });
}

fn has_queued_work(store:&Store)->Result<bool,String>{
    store.work_read(|conn,_|conn.query_row("SELECT EXISTS(SELECT 1 FROM work_jobs j JOIN work_plan_heads h ON h.plan_id=j.plan_id WHERE j.state='queued' AND h.control='active' AND h.head_revision_id=j.revision_id)",[],|row|row.get(0)).map_err(crate::workflow::storage_error))
}

fn drain(services:&Services,store:&Store,admission:&crate::close_guard::Admission)->Result<(),String>{
    let mut attempted=std::collections::HashSet::new();
    loop{
        if !has_queued_work(store)?{return Ok(());}
        let _maintenance=services.environment_gate.try_read().map_err(|_|"work_busy: aguarde a manutenção local.".to_string())?;
        crate::ensure_purge_access(services,"work.dispatch")?;
        crate::allow_new_work(services)?;
        let Some(dispatch)=services.supervisor.next_dispatch(store,&services.workflow)? else{return Ok(());};
        // A rejected concrete attempt is never retried by this wakeup, even if
        // an unexpected service failure left its queue entry unchanged.
        admission.identify_job(dispatch.assessment_id());
        if !attempted.insert(dispatch.attempt_id().to_owned()){return Err("work_reconciliation_required".into());}
        // The same backend services and shared arbiter handle both queued and
        // direct work. Keep maintenance excluded through the complete attempt.
        let result=match dispatch{
            crate::work_supervisor::QueuedDispatch::Native(dispatch)=>services.executor.run_queued(store,&dispatch.plan_id,&dispatch.digest,dispatch.token).map(|_|()),
            crate::work_supervisor::QueuedDispatch::Ai{token,grant}=>services.ai.run_queued_investigation(store,token,grant).map(|_|()),
            crate::work_supervisor::QueuedDispatch::Kali{token,grant}=>services.kali.run_queued_evidence(store,token,grant,||services.environment.executor_owned_identity(),&services.ai).map(|_|()),
        };
        if let Err(error)=result{
            // A known local failure may leave other jobs eligible. Unknown or
            // unresolved work keeps the resource and ends this drain.
            if services.supervisor.shared_arbiter().ensure_quiescent(store).is_err(){return Err(error);}
        }
    }
}
