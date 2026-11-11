//! Host-only coordination for provider effects. A lease never replaces a data,
//! scope or budget review. Journal uncertainty retains the durable resource.
use super::*;
use crate::work_supervisor::{EffectOwner,EffectOwnerKind,EffectPermit,EffectTermination,WorkArbiter};
use crate::workflow::EffectRef;

/// Read-only metadata gate for profiles predating the shared arbiter. An
/// acknowledged charge is not proof that its remote request has terminated.
pub(crate) fn has_unresolved_work(root:&std::path::Path)->Result<bool,String>{
    let path=root.join("assistant.sqlite3");
    match std::fs::symlink_metadata(&path){
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(error)=>return Err(failure(error)),Ok(_)=>{},
    }
    crate::recovery::require_plain(&path,false)?;
    let db=Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(failure)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(failure)?;
    // Values are observed inside SQLite; neither prompts, response text nor
    // vault values are selected or loaded into a diagnostic result.
    let invalid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM requests WHERE status NOT IN ('pending','completed','incomplete','failed','unknown')) OR EXISTS(SELECT 1 FROM investigations WHERE json_valid(data)=0 OR json_type(data,'$.status') IS NOT 'text' OR json_extract(data,'$.status') NOT IN ('prepared','running','canceling','completed','partial','failed','canceled','unknown','revoked','reconciled'))",[],|r|r.get(0)).map_err(failure)?;
    if invalid{return Err(failure("unrecognized AI journal state"));}
    let unresolved:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM requests WHERE status IN ('pending','unknown')) OR EXISTS(SELECT 1 FROM investigations WHERE json_extract(data,'$.status') IN ('running','canceling','unknown'))",[],|r|r.get(0)).map_err(failure)?;
    if !unresolved{return Ok(false);}
    let covered=local_closure::covered_roots(root,&db)?;
    let mut query=db.prepare("SELECT id,details FROM requests WHERE status IN ('pending','unknown')").map_err(failure)?;
    for row in query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(failure)?{
        let(id,raw)=row.map_err(failure)?;let details:Value=serde_json::from_str(&raw).map_err(failure)?;
        let root=details["investigationId"].as_str().map(|id|EffectRef::AiInvestigation{id:id.into()}).unwrap_or(EffectRef::AiRequest{id});
        if !covered.contains(&root){return Ok(true);}
    }
    let mut query=db.prepare("SELECT id FROM investigations WHERE json_extract(data,'$.status') IN ('running','canceling','unknown')").map_err(failure)?;
    for id in query.query_map([],|r|r.get::<_,String>(0)).map_err(failure)?{if !covered.contains(&EffectRef::AiInvestigation{id:id.map_err(failure)?}){return Ok(true);}}
    Ok(false)
}

enum Kind { Request, Investigation(String), Catalog }
/// Only this backend constructs this proof, after checking the exact old
/// unbound lease and its journals. It is never serialized or accepted over IPC.
pub(crate) struct AiNotStartedProof {owner:EffectOwner,boot_id:String,request_model:Option<(String,String)>}
impl AiNotStartedProof {
    pub(crate) fn owner(&self)->&EffectOwner{&self.owner}
    pub(crate) fn boot_id(&self)->&str{&self.boot_id}
    pub(super) fn request_identity(&self)->Option<(&str,&str,Option<(&str,&str)>)>{
        (self.owner.kind==EffectOwnerKind::AiRequestIntent).then(||(self.owner.id.as_str(),self.owner.assessment_id.as_deref().unwrap_or("connection-test"),self.request_model.as_ref().map(|(e,m)|(e.as_str(),m.as_str()))))
    }
    pub(super) fn fingerprint(&self)->String{digest(&json!({"schema":"openkoda.ai-not-started-proof.v1","owner":self.owner,"bootId":self.boot_id}).to_string())}
}
pub(super) struct AiWork<'a> {
    permit:EffectPermit<'a>,kind:Kind,request_id:Option<String>,bound:bool,
    queued:Option<WorkAiInspection>,
}

/// Production callers must carry the live parent lease. The fixture variant is
/// compiled only for existing low-level unit tests without a profile Store.
pub(super) enum RequestWork<'a,'store> { Live(&'a mut AiWork<'store>), #[cfg(test)] Fixture }
impl RequestWork<'_,'_> {
    pub(super) fn request_id(&self,suggested:Option<&str>)->Result<String,String>{
        match self {
            Self::Live(work)=>match &work.kind{
                Kind::Request=>{let id=work.direct_request_id()?;if suggested.is_some_and(|other|other!=id){return Err("work_owned_operation".into());}Ok(id)},
                Kind::Investigation(_)=>{let id=suggested.ok_or("work_owned_operation")?;crate::core::valid_id(id)?;Ok(id.into())},
                Kind::Catalog=>Err("work_owned_operation".into()),
            },
            #[cfg(test)]Self::Fixture=>Ok(suggested.map(str::to_owned).unwrap_or_else(model::id)),
        }
    }
    pub(super) fn intent(&mut self,id:&str)->Result<(),String>{
        match self {Self::Live(work)=>work.request_intent(id),#[cfg(test)]Self::Fixture=>Ok(())}
    }
    pub(super) fn bind(&mut self,ai:&AiService,id:&str,parent:Option<&str>)->Result<(),String>{
        match self {Self::Live(work)=>work.bind_request(ai,id,parent),#[cfg(test)]Self::Fixture=>Ok(())}
    }
    pub(super) fn before_dispatch(&self,store:&Store)->Result<(),String>{
        match self{Self::Live(work)=>work.require_active(store),#[cfg(test)]Self::Fixture=>Ok(())}
    }
}

impl<'a> AiWork<'a> {
    pub(super) fn queued(permit:EffectPermit<'a>,inspection:WorkAiInspection)->Self{
        Self{kind:Kind::Investigation(inspection.id().into()),permit,request_id:None,bound:false,queued:Some(inspection)}
    }
    pub(super) fn is_bound(&self)->bool{self.bound}
    pub(super) fn control(&self,store:&Store)->Result<crate::work_supervisor::WorkControl,String>{
        if self.queued.is_none(){return Ok(crate::work_supervisor::WorkControl::Active);}
        store.work_read(|conn,_|crate::work_supervisor::work_control_locked(conn,&self.permit))
    }
    pub(super) fn require_active(&self,store:&Store)->Result<(),String>{
        match self.control(store)?{crate::work_supervisor::WorkControl::Active=>Ok(()),crate::work_supervisor::WorkControl::Paused=>Err("work_paused".into()),crate::work_supervisor::WorkControl::Canceled=>Err("work_canceled".into())}
    }
    pub(super) fn validate_queued(&self,store:&Store)->Result<(),String>{
        if let Some(inspection)=&self.queued{store.work_read(|conn,s|crate::work_supervisor::validate_queued_ai_locked(conn,s,&self.permit,inspection))?;}Ok(())
    }
    pub(super) fn finish_queued(mut self,store:&Store,completion:&WorkAiCompletion)->Result<(),String>{
        if self.queued.is_none(){return Err("work_owned_operation".into());}
        store.work_transaction(|conn,s|{
            crate::work_supervisor::finish_ai_locked(conn,s,&self.permit,completion)?;
            crate::work_supervisor::finish_permit_locked(conn,&self.permit,completion.termination())
        })?;
        self.permit.acknowledge_finished();Ok(())
    }
    pub(super) fn direct_request_id(&self)->Result<String,String>{
        if !matches!(self.kind,Kind::Request){return Err("work_owned_operation".into());}
        self.request_id.clone().ok_or_else(||"work_owned_operation".into())
    }
    fn request_intent(&mut self,id:&str)->Result<(),String>{
        crate::core::valid_id(id)?;
        if matches!(self.kind,Kind::Request){
            if self.request_id.as_deref()!=Some(id){return Err("work_owned_operation: solicitação fora da reserva de IA.".into());}
        }
        Ok(())
    }
    fn bind_request(&mut self,ai:&AiService,id:&str,parent:Option<&str>)->Result<(),String>{
        match &self.kind {
            Kind::Request if parent.is_none()&&self.request_id.as_deref()==Some(id)=>{
                ai.arbiter.bind_effect(&mut self.permit,EffectRef::AiRequest{id:id.into()})?;self.bound=true;Ok(())
            }
            Kind::Investigation(expected) if self.bound&&parent==Some(expected.as_str())=>Ok(()),
            _=>Err("work_owned_operation: solicitação fora da investigação reservada.".into()),
        }
    }
    pub(super) fn bind_investigation(&mut self,ai:&AiService,store:&Store,id:&str,receipt:&crate::data_policy::DataUseReceiptRef)->Result<(),String>{
        if !matches!(&self.kind,Kind::Investigation(expected) if expected==id){return Err("work_owned_operation: investigação fora da reserva.".into());}
        let effect=EffectRef::AiInvestigation{id:id.into()};
        if let Some(inspection)=&self.queued{
            store.work_transaction(|conn,s|{
                crate::work_supervisor::validate_queued_ai_locked(conn,s,&self.permit,inspection)?;
                crate::work_supervisor::bind_effect_locked(conn,&self.permit,&effect)?;
                crate::work_supervisor::mark_ai_running_locked(conn,&self.permit,std::slice::from_ref(receipt))
            })?;
            self.permit.acknowledge_bound(effect);
        }else{ai.arbiter.bind_effect(&mut self.permit,effect)?;}
        self.bound=true;Ok(())
    }
    fn termination(&self,ai:&AiService)->Result<EffectTermination,String>{
        match &self.kind {
            Kind::Catalog=>Ok(EffectTermination::Stopped),
            Kind::Request=>{
                let Some(id)=self.request_id.as_ref()else{return Ok(EffectTermination::NotStarted);};
                let status:Option<String>=ai.db.lock().map_err(failure)?.query_row("SELECT status FROM requests WHERE id=?1",[id],|r|r.get(0)).optional().map_err(failure)?;
                Ok(match status.as_deref(){
                    None if !self.bound=>EffectTermination::NotStarted,
                    Some("completed"|"incomplete"|"failed")=>EffectTermination::Stopped,
                    _=>EffectTermination::Unknown,
                })
            }
            Kind::Investigation(id)=>ai.investigation_work_termination(id,self.bound),
        }
    }
    fn finish(self,ai:&AiService,result:Result<Value,String>)->Result<Value,String>{
        let observed=self.termination(ai);
        ai.arbiter.finish(self.permit,observed.as_ref().copied().unwrap_or(EffectTermination::Unknown))?;
        observed?;result
    }
}

pub(super) fn effect_action(action:&str)->bool{matches!(action,"ai.ask"|"ai.test"|"ai.models"|"ai.operation.propose"|"ai.investigation.run")}
impl AiService {
    pub(crate) fn with_arbiter(mut self,arbiter:Arc<WorkArbiter>)->Self{self.arbiter=arbiter;self}
    /// Startup-only recovery: either a proven pre-dispatch interruption or a
    /// bound Direct terminal journal. Current-boot and remote Unknown stay held.
    pub(crate) fn recover_unbound_work(&self,store:&Store)->Result<bool,String>{
        let _inference=self.inference.try_lock().map_err(|_|"Já existe uma solicitação à IA em andamento.")?;
        let active=self.investigation_active.lock().map_err(failure)?;if active.is_some(){return Err("work_busy".into());}
        let Some(candidate)=self.arbiter.unbound_direct_candidate(store)?else{return self.recover_bound_terminal_work_locked(store);};
        if !matches!(candidate.owner.kind,EffectOwnerKind::AiRequestIntent|EffectOwnerKind::AiInvestigationIntent|EffectOwnerKind::AiCatalog){return Ok(false);}
        let mut request_model=None;
        let reference=match candidate.owner.kind {
            EffectOwnerKind::AiRequestIntent=>{
                let expected=candidate.owner.assessment_id.as_deref().unwrap_or("connection-test");
                let row:Option<(String,String,String,String,String,Option<i64>,Option<i64>)>=self.db.lock().map_err(failure)?.query_row("SELECT assessment,endpoint,model,status,details,input_tokens,output_tokens FROM requests WHERE id=?1",[&candidate.owner.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(failure)?;
                if let Some((assessment,endpoint,model,status,raw,input,output))=row{
                    let details:Value=serde_json::from_str(&raw).map_err(failure)?;
                    let recorded_not_sent=status=="failed"&&details["notDispatched"]==true;
                    if assessment!=expected||ProviderId::from_endpoint(&endpoint).is_err()||(!matches!(status.as_str(),"pending"|"unknown")&&!recorded_not_sent)||details.get("providerStatus").is_some()||details.get("responseId").is_some()||details.get("usageUnknown").is_some()||details.get("investigationId").is_some()||(input.is_some()||output.is_some())&&!(recorded_not_sent&&input==Some(0)&&output==Some(0)){return Err("work_reconciliation_required: histórico da IA não comprova ausência de envio.".into());}
                    request_model=Some((endpoint,model));
                    let reference=details.get("dataUseReceiptRef").map(|v|serde_json::from_value::<crate::data_policy::DataUseReceiptRef>(v.clone()).map_err(failure)).transpose()?;
                    if candidate.owner.assessment_id.is_some()&&reference.is_none(){return Err("work_reconciliation_required: recibo de dados da intenção não encontrado.".into());}
                    reference
                }else{None}
            }
            EffectOwnerKind::AiInvestigationIntent=>{
                let aid=candidate.owner.assessment_id.as_deref().ok_or("work_reconciliation_required")?;
                self.inspect_unbound_investigation(&candidate.owner.id,aid)?
            }
            EffectOwnerKind::AiCatalog=>{
                let collision:bool=self.db.lock().map_err(failure)?.query_row("SELECT EXISTS(SELECT 1 FROM requests WHERE id=?1) OR EXISTS(SELECT 1 FROM investigations WHERE id=?1)",[&candidate.owner.id],|r|r.get(0)).map_err(failure)?;
                if candidate.owner.assessment_id.is_some()||collision{return Err("work_reconciliation_required".into());}None
            }
            _=>return Ok(false),
        };
        let kind=if candidate.owner.kind==EffectOwnerKind::AiInvestigationIntent{"investigation"}else{"request"};
        let snapshot=store.snapshot()?;
        let receipts=snapshot.data_use_receipts.iter().filter(|receipt|receipt.effect.id==candidate.owner.id).collect::<Vec<_>>();
        if receipts.len()>1||receipts.iter().any(|r|r.effect.kind!=kind||r.assessment_id!=candidate.owner.assessment_id||r.events.iter().any(|e|!matches!(e.outcome,crate::data_policy::DataUseOutcome::Admitted|crate::data_policy::DataUseOutcome::NotSent))||r.events.last().is_none())||reference.as_ref().is_some_and(|reference|!receipts.iter().any(|r|r.id==reference.id&&r.digest==reference.digest))||candidate.owner.kind==EffectOwnerKind::AiCatalog&&!receipts.is_empty(){return Err("work_reconciliation_required: recibos não comprovam ausência de envio.".into());}
        let receipt_refs=receipts.iter().map(|r|crate::data_policy::DataUseReceiptRef{id:r.id.clone(),digest:r.digest.clone()}).collect::<Vec<_>>();
        let proof=AiNotStartedProof{owner:candidate.owner,boot_id:candidate.boot_id,request_model};
        match proof.owner.kind {
            EffectOwnerKind::AiRequestIntent=>{
                self.budget.not_sent_with_proof(&proof)?;
                self.db.lock().map_err(failure)?.execute("UPDATE requests SET status='failed',input_tokens=0,output_tokens=0,details=json_set(details,'$.notDispatched',json('true'),'$.notDispatchedProof',?1,'$.endedAt',?2) WHERE id=?3",params![proof.fingerprint(),model::now(),proof.owner.id]).map_err(failure)?;
            }
            EffectOwnerKind::AiInvestigationIntent=>self.mark_unbound_investigation_not_sent(&proof.owner.id,proof.owner.assessment_id.as_deref().ok_or("work_reconciliation_required")?)?,
            EffectOwnerKind::AiCatalog=>{},_=>return Err("work_reconciliation_required".into()),
        }
        for receipt in receipt_refs{self.data_policy.finish(store,&receipt,crate::data_policy::DataUseOutcome::NotSent,vec![])?;}
        // Recheck the complete lease identity in the final core transaction.
        // Crashes before this commit keep Unknown; the writes above are replay-safe.
        self.arbiter.reconcile_ai_not_started(store,&proof)?;Ok(true)
    }
    pub(super) fn handle_work(&self,action:&str,payload:Value,store:&Store)->Result<Value,String>{
        // Keep inference held until the exact journal outcome and durable lease
        // have been committed; key rotation and another request cannot interleave.
        let _inference=self.inference.try_lock().map_err(|_|"Já existe uma solicitação à IA em andamento.")?;
        let (kind,assessment_id,owner_kind)=match action{
            "ai.models"=>(Kind::Catalog,None,EffectOwnerKind::AiCatalog),
            "ai.test"=>(Kind::Request,None,EffectOwnerKind::AiRequestIntent),
            "ai.investigation.run"=>{let (id,aid)=self.investigation_work_owner(&payload,store)?;(Kind::Investigation(id),Some(aid),EffectOwnerKind::AiInvestigationIntent)},
            "ai.ask"|"ai.operation.propose"=>{
                let aid=payload.get("assessmentId").and_then(Value::as_str).ok_or("Campos inválidos na solicitação de IA.")?;
                crate::core::valid_id(aid)?;check_assessment(store,aid)?;(Kind::Request,Some(aid.into()),EffectOwnerKind::AiRequestIntent)
            }
            _=>return Err("work_owned_operation: ação sem classe de execução.".into()),
        };
        let owner_id=match &kind{Kind::Investigation(id)=>id.clone(),_=>model::id()};
        let request_id=matches!(kind,Kind::Request).then(||owner_id.clone());
        // New owner kinds identify this ordering: the durable owner ID is the
        // exact request/parent ID, even if the process stops before its INSERT.
        let permit=self.arbiter.claim_direct(store,EffectOwner{id:owner_id,assessment_id,kind:owner_kind})?;
        let mut work=AiWork{permit,kind,request_id,bound:false,queued:None};
        let result=self.handle_inner(action,payload,store,Some(&mut work));
        work.finish(self,result)
    }
}

#[cfg(all(test,windows))]
mod tests {
    mod direct_recovery { include!("ai_direct_recovery_tests.rs"); }
    use super::*;
    use std::{collections::VecDeque,sync::atomic::{AtomicUsize,Ordering}};
    const KEY:&str="synthetic-ai-work-fixture-key-not-real";
    struct Fake {store:Arc<Store>,arbiter:Arc<WorkArbiter>,answers:Mutex<VecDeque<Result<Value,String>>>,calls:AtomicUsize,bindings:Mutex<Vec<(String,String)>>}
    impl Provider for Fake {
        fn response(&self,_:ProviderId,_:&str,key:&str,_:&str,_:&str,_:u32)->Result<Value,String>{
            assert_eq!(key,KEY);self.calls.fetch_add(1,Ordering::SeqCst);
            let binding=self.store.work_read(|db,_|db.query_row("SELECT effect_kind,effect_id FROM work_leases WHERE state='bound'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(failure)).unwrap();
            self.bindings.lock().unwrap().push(binding);
            let concurrent=self.arbiter.claim_direct(&self.store,EffectOwner{id:model::id(),assessment_id:None,kind:EffectOwnerKind::Native});
            assert!(concurrent.is_err_and(|e|e.starts_with("work_busy")||e.starts_with("work_unknown_effect")),"native dispatch must be blocked while provider owns the lease");
            self.answers.lock().unwrap().pop_front().expect("unplanned provider call")
        }
    }
    fn answer()->Result<Value,String>{Ok(json!({"id":model::id(),"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Synthetic bounded observation"}]}],"usage":{"input_tokens":7,"output_tokens":9}}))}
    struct Fixture {_dir:tempfile::TempDir,store:Arc<Store>,ai:AiService,fake:Arc<Fake>,arbiter:Arc<WorkArbiter>,aid:String}
    impl Fixture {
        fn new(answers:Vec<Result<Value,String>>)->Self{
            let dir=tempfile::tempdir().unwrap();let store=Arc::new(Store::open(dir.path().into()).unwrap());let arbiter=Arc::new(WorkArbiter::default());
            let fake=Arc::new(Fake{store:store.clone(),arbiter:arbiter.clone(),answers:Mutex::new(answers.into()),calls:AtomicUsize::new(0),bindings:Mutex::new(vec![])});
            let ai=AiService::with_provider(store.root.clone(),fake.clone()).unwrap().with_arbiter(arbiter.clone());
            ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
            let project=store.handle(crate::core::Request{action:"project.create".into(),payload:json!({"name":"Arbiter fixture","kind":"web","target":"https://never-dispatch.invalid/"})}).unwrap();
            let aid=store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Arbiter fixture","kind":"web","target":project["target"],"objective":"Review synthetic data only","scope":scope::test_creation(&store,project["id"].as_str().unwrap(),&[AI_METHOD])})}).unwrap()["id"].as_str().unwrap().to_owned();
            ai.test_data_authority(&store,&aid);Self{_dir:dir,store,ai,fake,arbiter,aid}
        }
        fn ask(&self)->Value{let mut input=self.ai.test_data_input(&self.store,"ask",json!({"assessmentId":self.aid,"evidenceIds":[],"message":"Review the synthetic observation"}));input["allowRemote"]=json!(true);input}
        fn plan(&self)->Value{let input=self.ai.test_data_input(&self.store,"investigation",json!({"assessmentId":self.aid,"evidenceIds":[],"message":"Independent reviews of synthetic data","perspectives":["evidence","counterevidence"],"maxOutputTokensPerRequest":512,"maxTotalTokens":100000}));self.ai.handle("ai.investigation.plan",input,&self.store).unwrap()}
        fn requests(&self)->i64{self.ai.db.lock().unwrap().query_row("SELECT count(*) FROM requests",[],|r|r.get(0)).unwrap()}
        fn leases(&self)->u64{self.store.work_read(|db,_|db.query_row("SELECT count(*) FROM work_leases",[],|r|r.get(0)).map_err(failure)).unwrap()}
        fn restart_without_provider_calls(self)->Self{
            let Self{_dir,store,ai,fake,arbiter,aid}=self;let root=store.root.clone();
            assert_eq!(fake.calls.load(Ordering::SeqCst),0);assert!(fake.answers.lock().unwrap().is_empty());
            // Both the fixture and its fake provider own an Arc<Store>. Release
            // every owner before opening a new installation lock, as a real
            // process restart would do.
            drop(ai);drop(fake);drop(store);drop(arbiter);
            let store=Arc::new(Store::open(root.clone()).unwrap());let arbiter=Arc::new(WorkArbiter::default());
            let fake=Arc::new(Fake{store:store.clone(),arbiter:arbiter.clone(),answers:Mutex::new(VecDeque::new()),calls:AtomicUsize::new(0),bindings:Mutex::new(vec![])});
            let ai=AiService::with_provider(root,fake.clone()).unwrap().with_arbiter(arbiter.clone());
            Self{_dir,store,ai,fake,arbiter,aid}
        }
        fn interrupt_request(&self,insert:bool,dispatched_marker:bool)->String{
            let input:AskInput=decode(self.ask()).unwrap();let id=model::id();
            let permit=self.arbiter.claim_direct(&self.store,EffectOwner{id:id.clone(),assessment_id:Some(self.aid.clone()),kind:EffectOwnerKind::AiRequestIntent}).unwrap();
            let reviewed=self.ai.reviewed_ai_data(&self.store,input.data_review.as_ref(),&self.aid,&input.evidence_ids,&input.context_target_refs,&input.message,"ask").unwrap();
            let scope=context_scope(&reviewed.packet.context).unwrap();let (_,revision)=self.ai.credential().unwrap();let(prompt,instructions,maximum)=&reviewed.packet.prompts[0];
            let spec=budget::Spec::new("ask",&self.aid,"fixture-model",&revision,prompt,instructions,*maximum).unwrap().bind_data(&reviewed.prepared.review.digest).unwrap();
            self.ai.data_policy.with_admission(&self.store,&reviewed.prepared.review,&reviewed.packet.spec,Some(crate::data_policy::OperationalScope{assessment_id:&self.aid,method:AI_METHOD,expected:&scope,targets:&[]}),crate::data_policy::EffectLink{kind:"request".into(),id:id.clone()},false,|admission|{
                let charge=self.ai.budget.reserve(&id,&spec,None,0)?;
                if insert{self.ai.db.lock().map_err(failure)?.execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created,details) VALUES(?1,?2,?3,'fixture-model',?4,?5,'pending',?6,?7)",params![id,self.aid,ENDPOINT,revision,digest(prompt),model::now(),json!({"budget":charge,"dataUseReceiptRef":admission.receipt_ref}).to_string()]).map_err(failure)?;}
                Ok(())
            }).unwrap();
            if dispatched_marker{self.ai.budget.dispatched(&id).unwrap();}
            // Stop precisely after the independently durable reservation/intent,
            // before the binding that production requires before HTTP.
            drop(permit);id
        }
    }
    #[test]
    fn native_lease_blocks_all_provider_routes_without_consuming_reviews(){
        let f=Fixture::new(vec![answer()]);let input=f.ask();let plan=f.plan();let before=f.store.snapshot().unwrap().data_use_receipts.len();
        let permit=f.arbiter.claim_direct(&f.store,EffectOwner{id:model::id(),assessment_id:Some(f.aid.clone()),kind:EffectOwnerKind::Native}).unwrap();
        for(action,payload)in[("ai.ask",input.clone()),("ai.test",json!({})),("ai.models",json!({})),("ai.operation.propose",json!({"assessmentId":f.aid})),("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}))]{
            assert!(f.ai.handle(action,payload,&f.store).unwrap_err().starts_with("work_busy"),"{action}");
        }
        assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert_eq!(f.store.snapshot().unwrap().data_use_receipts.len(),before);
        f.arbiter.finish(permit,EffectTermination::NotStarted).unwrap();
        f.ai.handle("ai.ask",input,&f.store).unwrap();assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.leases(),0);
    }
    #[test]
    fn terminal_request_is_bound_before_provider_and_releases_only_its_resource(){
        let f=Fixture::new(vec![answer()]);let result=f.ai.handle("ai.ask",f.ask(),&f.store).unwrap();
        assert_eq!(f.fake.bindings.lock().unwrap().as_slice(),&[("ai_request".into(),result["requestId"].as_str().unwrap().into())]);
        assert_eq!(f.requests(),1);assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
        f.arbiter.ensure_quiescent(&f.store).unwrap();
    }
    #[test]
    fn transport_unknown_remains_reserved_after_acknowledgment_and_service_restart(){
        let f=Fixture::new(vec![Err("Falha de conexão com a OpenAI.".into())]);assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());
        assert_eq!(f.leases(),1);let history=f.ai.handle("ai.history",json!({"assessmentId":f.aid}),&f.store).unwrap();let request=&history["requests"][0];assert_eq!(request["status"],"unknown");
        f.ai.handle("ai.request.reconcile",json!({"assessmentId":f.aid,"requestId":request["id"],"note":"Inspected provider uncertainty; no termination evidence","acknowledgeUnknownUsage":true}),&f.store).unwrap();
        assert_eq!(f.leases(),1);let reopened=AiService::with_provider(f.store.root.clone(),f.fake.clone()).unwrap().with_arbiter(Arc::new(WorkArbiter::default()));
        assert!(reopened.handle("ai.test",json!({}),&f.store).unwrap_err().starts_with("work_unknown_effect"));
        assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert_eq!(f.requests(),1);
    }
    #[test]
    fn nonterminal_or_unrecognized_provider_response_never_releases_the_resource(){
        for body in [json!({"id":"synthetic-in-progress","status":"in_progress"}),json!({"id":"synthetic-missing-status"})]{
            let f=Fixture::new(vec![Ok(body)]);assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());assert_eq!(f.leases(),1);
            let status:String=f.ai.db.lock().unwrap().query_row("SELECT status FROM requests",[],|r|r.get(0)).unwrap();assert_eq!(status,"unknown");
        }
    }
    #[test]
    fn failed_preflight_and_rejected_intent_leave_no_effect_or_resource(){
        let f=Fixture::new(vec![]);let mut input=f.ask();input["allowRemote"]=json!(false);assert!(f.ai.handle("ai.ask",input,&f.store).is_err());assert_eq!(f.leases(),0);
        f.ai.db.lock().unwrap().execute_batch("CREATE TRIGGER synthetic_intent_failure BEFORE INSERT ON requests BEGIN SELECT RAISE(ABORT,'synthetic journal failure'); END;").unwrap();
        assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());assert_eq!(f.leases(),0);assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    }
    #[test]
    fn pending_intent_after_failed_binding_and_failed_status_write_stays_unknown(){
        let f=Fixture::new(vec![]);let input=f.ask();
        f.store.work_transaction(|db,_|db.execute_batch("CREATE TEMP TRIGGER synthetic_bind_failure BEFORE UPDATE ON work_leases WHEN NEW.state='bound' BEGIN SELECT RAISE(ABORT,'synthetic binding failure'); END;").map_err(failure)).unwrap();
        f.ai.db.lock().unwrap().execute_batch("CREATE TRIGGER synthetic_status_failure BEFORE UPDATE ON requests WHEN NEW.status='failed' BEGIN SELECT RAISE(ABORT,'synthetic status failure'); END;").unwrap();
        assert!(f.ai.handle("ai.ask",input,&f.store).is_err());assert_eq!(f.requests(),1);assert_eq!(f.leases(),1);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
        let state:String=f.store.work_read(|db,_|db.query_row("SELECT state FROM work_leases",[],|r|r.get(0)).map_err(failure)).unwrap();assert_eq!(state,"unknown");
        assert!(f.arbiter.ensure_quiescent(&f.store).is_err());
    }
    #[test]
    fn investigation_children_share_parent_and_unknown_prevents_next_child(){
        for unknown in [false,true]{
            let answers=if unknown{vec![Err("Falha de conexão com a OpenAI.".into())]}else{vec![answer(),answer()]};let f=Fixture::new(answers);let plan=f.plan();
            let result=f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).unwrap();
            assert_eq!(result["status"],if unknown{"unknown"}else{"completed"});assert_eq!(f.fake.calls.load(Ordering::SeqCst),if unknown{1}else{2});
            assert!(f.fake.bindings.lock().unwrap().iter().all(|(kind,id)|kind=="ai_investigation"&&Some(id.as_str())==plan["id"].as_str()));
            assert_eq!(f.leases(),u64::from(unknown));
        }
    }
    #[test]
    fn legacy_unknown_without_lease_is_found_by_readonly_status_gate(){
        let empty=tempfile::tempdir().unwrap();assert!(!has_unresolved_work(empty.path()).unwrap());assert!(!empty.path().join("assistant.sqlite3").exists());
        let f=Fixture::new(vec![]);assert!(!has_unresolved_work(&f.store.root).unwrap());let plan=f.plan();assert!(!has_unresolved_work(&f.store.root).unwrap());
        f.ai.db.lock().unwrap().execute("UPDATE investigations SET data=json_set(data,'$.status','running') WHERE id=?1",[plan["id"].as_str().unwrap()]).unwrap();
        assert!(has_unresolved_work(&f.store.root).unwrap());assert_eq!(f.leases(),0);
        f.ai.db.lock().unwrap().execute("UPDATE investigations SET data=json_set(data,'$.status','reconciled') WHERE id=?1",[plan["id"].as_str().unwrap()]).unwrap();
        assert!(!has_unresolved_work(&f.store.root).unwrap());let id=model::id();
        f.ai.db.lock().unwrap().execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created,details) VALUES(?1,?2,?3,'fixture-model','fixture-revision','fixture','unknown',?4,'{}')",params![id,f.aid,ENDPOINT,model::now()]).unwrap();
        f.ai.handle("ai.request.reconcile",json!({"assessmentId":f.aid,"requestId":id,"note":"Legacy provider uncertainty remains","acknowledgeUnknownUsage":true}),&f.store).unwrap();
        assert!(has_unresolved_work(&f.store.root).unwrap());assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
        f.ai.db.lock().unwrap().execute("UPDATE requests SET status='unexpected' WHERE id=?1",[id]).unwrap();assert!(has_unresolved_work(&f.store.root).is_err());
    }
    #[test]
    fn restart_recovers_only_new_unbound_claims_and_catalog_without_inventing_requests(){
        for kind in [EffectOwnerKind::AiRequestIntent,EffectOwnerKind::AiCatalog,EffectOwnerKind::AiRequest]{
            let f=Fixture::new(vec![]);let id=model::id();let permit=f.arbiter.claim_direct(&f.store,EffectOwner{id,assessment_id:None,kind}).unwrap();drop(permit);
            assert!(!f.ai.recover_unbound_work(&f.store).unwrap(),"same boot cannot prove its worker is gone");
            let reopened=AiService::with_provider(f.store.root.clone(),f.fake.clone()).unwrap();let recoverable=kind!=EffectOwnerKind::AiRequest;
            assert_eq!(reopened.recover_unbound_work(&f.store).unwrap(),recoverable);assert_eq!(f.leases(),u64::from(!recoverable));assert_eq!(f.requests(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
            assert!(!reopened.recover_unbound_work(&f.store).unwrap());
        }
    }
    #[test]
    fn restart_proves_reservation_and_pending_intent_before_binding_were_not_sent(){
        for insert in [false,true]{
            let f=Fixture::new(vec![]);let id=f.interrupt_request(insert,false);
            let f=f.restart_without_provider_calls();
            let before=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(before["unknownChargesTotal"],1);
            let mut committed_charge=None;
            if insert{
                f.store.fail_commit_when(|state|state.data_use_receipts.iter().any(|receipt|receipt.events.last().is_some_and(|event|event.outcome==crate::data_policy::DataUseOutcome::NotSent)));
                assert!(f.ai.recover_unbound_work(&f.store).is_err());assert_eq!(f.leases(),1);
                let raw:String=f.ai.db.lock().unwrap().query_row("SELECT details FROM requests WHERE id=?1",[&id],|r|r.get(0)).unwrap();let details:Value=serde_json::from_str(&raw).unwrap();assert_eq!(details["notDispatched"],json!(true));
                let status=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(status["recentCharges"][0]["state"],"not_dispatched");committed_charge=Some(status["recentCharges"][0].clone());
                assert_eq!(f.store.snapshot().unwrap().data_use_receipts.iter().find(|receipt|receipt.effect.id==id).unwrap().events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::Admitted);
            }
            assert!(f.ai.recover_unbound_work(&f.store).unwrap());assert!(!f.ai.recover_unbound_work(&f.store).unwrap());
            let after=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(after["unknownChargesTotal"],0);assert_eq!(after["recentCharges"][0]["state"],"not_dispatched");assert_eq!(after["recentCharges"][0]["chargeId"],id);if let Some(charge)=committed_charge{assert_eq!(after["recentCharges"][0],charge,"recovery retry must preserve the committed financial history");}
            assert_eq!(f.requests(),i64::from(insert));if insert{let status:String=f.ai.db.lock().unwrap().query_row("SELECT status FROM requests WHERE id=?1",[&id],|r|r.get(0)).unwrap();assert_eq!(status,"failed");}
            let receipts=f.store.snapshot().unwrap().data_use_receipts;let receipt=receipts.iter().find(|r|r.effect.id==id).unwrap();assert_eq!(receipt.events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::NotSent);
            assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
        }
    }
    #[test]
    fn recovery_refuses_bound_remote_unknown_and_contradictory_dispatch_metadata(){
        let f=Fixture::new(vec![Err("Falha de conexão com a OpenAI.".into())]);assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());
        let reopened=AiService::with_provider(f.store.root.clone(),f.fake.clone()).unwrap();assert!(!reopened.recover_unbound_work(&f.store).unwrap());assert_eq!(f.leases(),1);
        for marker in [false,true]{
            let f=Fixture::new(vec![]);let id=f.interrupt_request(true,marker);
            if !marker{f.ai.db.lock().unwrap().execute("UPDATE requests SET details=json_set(details,'$.responseId','synthetic-response','$.providerStatus','in_progress') WHERE id=?1",[id]).unwrap();}
            let reopened=AiService::with_provider(f.store.root.clone(),f.fake.clone()).unwrap();assert!(reopened.recover_unbound_work(&f.store).is_err());assert_eq!(f.leases(),1);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
            let status=reopened.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(status["unknownChargesTotal"],1);
        }
    }
    #[test]
    fn interrupted_investigation_before_binding_is_closed_without_sending_children(){
        let f=Fixture::new(vec![]);let plan=f.plan();let id=plan["id"].as_str().unwrap().to_owned();
        let permit=f.arbiter.claim_direct(&f.store,EffectOwner{id:id.clone(),assessment_id:Some(f.aid.clone()),kind:EffectOwnerKind::AiInvestigationIntent}).unwrap();drop(permit);
        let reopened=AiService::with_provider(f.store.root.clone(),f.fake.clone()).unwrap();assert!(reopened.recover_unbound_work(&f.store).unwrap());
        let plans=reopened.handle("ai.investigation.list",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(plans["items"][0]["status"],"failed");assert_eq!(plans["items"][0]["results"],json!([]));assert_eq!(f.requests(),0);assert_eq!(f.leases(),0);
    }
}
