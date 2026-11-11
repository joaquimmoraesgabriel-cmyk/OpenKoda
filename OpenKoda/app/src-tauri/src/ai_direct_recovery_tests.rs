use super::*;

fn restart(f:Fixture)->Fixture{
    let Fixture{_dir,store,ai,fake,arbiter,aid}=f;let root=store.root.clone();
    assert!(fake.answers.lock().unwrap().is_empty());drop(ai);drop(fake);drop(store);drop(arbiter);
    let store=Arc::new(Store::open(root.clone()).unwrap());let arbiter=Arc::new(WorkArbiter::default());
    let fake=Arc::new(Fake{store:store.clone(),arbiter:arbiter.clone(),answers:Mutex::new(VecDeque::new()),calls:AtomicUsize::new(0),bindings:Mutex::new(vec![])});
    let ai=AiService::with_provider(root,fake.clone()).unwrap().with_arbiter(arbiter.clone());
    crate::work_supervisor::WorkSupervisor::default().recover_after_boot(&store).unwrap();
    Fixture{_dir,store,ai,fake,arbiter,aid}
}
fn fail_finish(f:&Fixture){f.store.work_transaction(|db,_|db.execute_batch("CREATE TEMP TRIGGER direct_finish_fault BEFORE DELETE ON work_leases BEGIN SELECT RAISE(ABORT,'synthetic direct crash after journal'); END;").map_err(failure)).unwrap();}
fn charges(f:&Fixture)->Value{f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap()["recentCharges"].clone()}
fn raw_requests(f:&Fixture)->Vec<String>{let db=f.ai.db.lock().unwrap();let mut q=db.prepare("SELECT json_array(id,assessment,endpoint,model,revision,context_hash,status,created,details,input_tokens,output_tokens) FROM requests ORDER BY id").unwrap();let rows=q.query_map([],|r|r.get(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();rows}

#[test]
fn direct_terminal_startup_recovery_preserves_journals_and_unknown_accounting_without_retry(){
    for action in ["ai.test","ai.ask","ai.investigation.run"]{
        for usage in [true,false]{
            let mut value=answer().unwrap();if !usage{value.as_object_mut().unwrap().remove("usage");}
            let answers=if action=="ai.investigation.run"&&usage{vec![Ok(value.clone()),Ok(value)]}else{vec![Ok(value)]};let f=Fixture::new(answers);fail_finish(&f);
            let payload=match action{"ai.test"=>json!({}),"ai.ask"=>f.ask(),_=>{let p=f.plan();json!({"id":p["id"],"digest":p["digest"],"allowRemote":true})}};
            assert!(f.ai.handle(action,payload,&f.store).is_err());assert_eq!(f.leases(),1);let journal=raw_requests(&f);let money=charges(&f);let receipts=f.store.snapshot().unwrap().data_use_receipts;
            // Current process ownership cannot use startup recovery, even
            // though the fake has returned and its terminal row is durable.
            assert!(!f.ai.recover_unbound_work(&f.store).unwrap());assert_eq!(f.leases(),1);
            let f=restart(f);assert!(f.ai.recover_unbound_work(&f.store).unwrap());assert_eq!(f.leases(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
            assert_eq!(raw_requests(&f),journal);assert_eq!(charges(&f),money);assert_eq!(f.store.snapshot().unwrap().data_use_receipts,receipts);assert!(!f.ai.recover_unbound_work(&f.store).unwrap());
            if !usage{assert!(money.as_array().unwrap().iter().all(|v|v["state"]=="unknown"));}
        }
    }
}

#[test]
fn direct_terminal_receipt_repair_and_lease_release_rollback_together(){
    let f=Fixture::new(vec![answer()]);
    // DataPolicy writes app_state after the independent response journal.
    f.store.work_transaction(|db,_|db.execute_batch("CREATE TEMP TRIGGER direct_receipt_fault BEFORE UPDATE ON app_state WHEN EXISTS(SELECT 1 FROM json_each(NEW.json,'$.dataUseReceipts') r,json_each(r.value,'$.events') e WHERE json_extract(e.value,'$.outcome')='completed') BEGIN SELECT RAISE(ABORT,'synthetic crash before receipt finalization'); END;").map_err(failure)).unwrap();fail_finish(&f);
    assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());let money=charges(&f);let journal=raw_requests(&f);assert_eq!(f.leases(),1);
    let f=restart(f);let before=f.store.snapshot().unwrap().data_use_receipts;assert_eq!(before[0].events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::Dispatched);
    fail_finish(&f);assert!(f.ai.recover_unbound_work(&f.store).is_err());assert_eq!(f.leases(),1);assert_eq!(f.store.snapshot().unwrap().data_use_receipts,before);
    f.store.work_transaction(|db,_|db.execute_batch("DROP TRIGGER direct_finish_fault;").map_err(failure)).unwrap();assert!(f.ai.recover_unbound_work(&f.store).unwrap());assert_eq!(f.leases(),0);
    assert_eq!(f.store.snapshot().unwrap().data_use_receipts[0].events.last().unwrap().outcome,crate::data_policy::DataUseOutcome::Completed);assert_eq!(raw_requests(&f),journal);assert_eq!(charges(&f),money);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
}

#[test]
fn direct_startup_recovery_keeps_remote_nonterminal_and_transport_unknown_held(){
    for answer in [Ok(json!({"id":"synthetic-response","status":"in_progress"})),Ok(json!({"id":"synthetic-response","status":"queued"})),Err("Falha de conexão: synthetic timeout".into())]{
        let f=Fixture::new(vec![answer]);assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());let journal=raw_requests(&f);let money=charges(&f);let f=restart(f);
        assert!(!f.ai.recover_unbound_work(&f.store).unwrap());assert_eq!(f.leases(),1);assert_eq!(raw_requests(&f),journal);assert_eq!(charges(&f),money);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);assert!(f.arbiter.ensure_quiescent(&f.store).is_err());
    }
}

#[test]
fn direct_terminal_recovery_rejects_wrong_identity_and_failed_without_provider_proof(){
    for mutation in ["assessment","endpoint","receipt","response","failed"]{
        let f=Fixture::new(vec![answer()]);fail_finish(&f);assert!(f.ai.handle("ai.ask",f.ask(),&f.store).is_err());let f=restart(f);let money=charges(&f);
        let sql=match mutation{
            "assessment"=>"UPDATE requests SET assessment='connection-test'",
            "endpoint"=>"UPDATE requests SET endpoint='https://never-dispatch.invalid/'",
            "receipt"=>"UPDATE requests SET details=json_set(details,'$.dataUseReceiptRef.id','not-the-receipt')",
            "response"=>"UPDATE requests SET details=json_remove(details,'$.responseId')",
            _=>"UPDATE requests SET status='failed',details=json_remove(details,'$.providerStatus','$.responseId')",
        };f.ai.db.lock().unwrap().execute_batch(sql).unwrap();let result=f.ai.recover_unbound_work(&f.store);assert!(!matches!(result,Ok(true)),"{mutation}");assert_eq!(f.leases(),1);assert_eq!(charges(&f),money);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    }
}

#[test]
fn direct_parent_repair_is_idempotent_after_response_journals_then_core_commit_failure(){
    let f=Fixture::new(vec![answer(),answer()]);let plan=f.plan();
    f.ai.db.lock().unwrap().execute_batch("CREATE TEMP TRIGGER direct_parent_finish_fault BEFORE UPDATE ON investigations WHEN json_extract(NEW.data,'$.status')='completed' BEGIN SELECT RAISE(ABORT,'synthetic parent journal interruption'); END;").unwrap();
    assert!(f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true}),&f.store).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),2);assert_eq!(f.leases(),1);
    let requests=raw_requests(&f);let money=charges(&f);let f=restart(f);fail_finish(&f);
    assert!(f.ai.recover_unbound_work(&f.store).is_err());assert_eq!(f.leases(),1);
    let partial:String=f.ai.db.lock().unwrap().query_row("SELECT data FROM investigations WHERE id=?1",[plan["id"].as_str().unwrap()],|r|r.get(0)).unwrap();let preserved:Value=serde_json::from_str(&partial).unwrap();assert_eq!(preserved["status"],"partial");assert!(preserved["results"].as_array().unwrap().iter().all(|s|s["status"]=="completed"&&!s["text"].as_str().unwrap().is_empty()));
    f.store.work_transaction(|db,_|db.execute_batch("DROP TRIGGER direct_finish_fault;").map_err(failure)).unwrap();assert!(f.ai.recover_unbound_work(&f.store).unwrap());assert_eq!(f.leases(),0);
    let after:String=f.ai.db.lock().unwrap().query_row("SELECT data FROM investigations WHERE id=?1",[plan["id"].as_str().unwrap()],|r|r.get(0)).unwrap();assert_eq!(after,partial);assert_eq!(raw_requests(&f),requests);assert_eq!(charges(&f),money);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
}
