use super::*;
use crate::core::{self, Request};
use std::sync::atomic::{AtomicUsize,Ordering};

const KEY:&str="synthetic-combined-ai-fixture-key-not-real";
struct Fake{calls:AtomicUsize,prompts:Mutex<Vec<String>>,answer:String}
impl Fake{fn new(answer:&str)->Arc<Self>{Arc::new(Self{calls:AtomicUsize::new(0),prompts:Mutex::new(vec![]),answer:answer.into()})}}
impl Provider for Fake{
    fn response(&self,_:ProviderId,_:&str,key:&str,prompt:&str,_:&str,_:u32)->Result<Value,String>{
        assert_eq!(key,KEY);self.calls.fetch_add(1,Ordering::SeqCst);self.prompts.lock().unwrap().push(prompt.into());
        Ok(json!({"id":model::id(),"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":self.answer}]}],"usage":{"input_tokens":11,"output_tokens":13}}))
    }
}
struct Fixture{_dir:tempfile::TempDir,store:Store,ai:AiService,fake:Arc<Fake>,aid:String,refs:Vec<ScopeTargetRef>}
impl Fixture{
    fn new(second_ai:bool,answer:&str)->Self{
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();let fake=Fake::new(answer);let ai=AiService::with_provider(dir.path().into(),fake.clone()).unwrap();
        ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
        let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"ALPHA_PRIVATE_ANCHOR","kind":"web","target":"https://alpha-only.test/ALPHA_SECRET_PATH"})}).unwrap();
        let pid=project["id"].as_str().unwrap();
        let mut additional=vec![];
        for(label,target)in [("BETA_SELECTED","https://beta-only.test/selected"),("CATALOG_NEVER_SELECTED","https://catalog-hidden.test/hidden")]{
            let options=scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();
            let result=scope_targets::handle_catalog("project.targetAdd",json!({"projectId":pid,"expectedCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":model::id(),"label":label,"kind":"web","target":target}),&store).unwrap();
            additional.push(result["targetId"].as_str().unwrap().to_owned());
        }
        let options=scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();
        let second_methods=if second_ai{vec![AI_METHOD,"kali.hash-evidence.v1","http.headers.v1"]}else{vec!["http.headers.v1"]};
        let mut input=json!({"projectId":pid,"title":"Combined synthetic review","objective":"Review only selected observations","window":{"startsAt":null,"endsAt":null},"projectCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":model::id(),"selection":{"primaryProjectTargetId":"primary","entries":[{"projectTargetId":"primary","methods":[AI_METHOD,"kali.hash-evidence.v1","http.headers.v1"]},{"projectTargetId":additional[0],"methods":second_methods}],"commonMethods":["kali.runtime-diagnostics.v1"],"excludedProjectTargets":[],"localExclusions":[],"connectionExclusions":[]}});
        let preview=scope::handle("assessment.combinedPreview",input.clone(),&store,str::to_owned).unwrap();input["reviewDigest"]=preview["reviewDigest"].clone();input["acknowledgeScope"]=json!(true);
        let created=scope::handle("assessment.combinedCreate",input,&store,str::to_owned).unwrap();let aid=created["assessmentId"].as_str().unwrap().to_owned();
        let state=store.snapshot().unwrap();let set=scope_targets::current_body(&state,&aid).unwrap().target_set.as_ref().unwrap();
        let a=set.entries.iter().find(|e|e.project_target_id=="primary").unwrap();let b=set.entries.iter().find(|e|e.project_target_id==additional[0]).unwrap();let refs=vec![scope_targets::entry_ref(a),scope_targets::entry_ref(b)];
        let fixture=Self{_dir:dir,store,ai,fake,aid,refs};fixture.ai.test_data_authority(&fixture.store,&fixture.aid);fixture
    }
    fn context(&self,refs:&[ScopeTargetRef],evidence:&[String])->Result<Value,String>{self.ai.test_context_projection(&self.store,json!({"assessmentId":self.aid,"evidenceIds":evidence,"contextTargetRefs":refs}))}
    fn import(&self,refs:&[ScopeTargetRef],content:&str)->String{self.store.handle(Request{action:"evidence.import".into(),payload:json!({"assessmentId":self.aid,"name":"synthetic observation","sourceDescription":"Human supplied fixture","content":content,"acknowledgeUnverifiedSource":true,"contextTargetRefs":refs})}).unwrap()["id"].as_str().unwrap().to_owned()}
    fn input(&self,refs:&[ScopeTargetRef],evidence:&[String])->Value{let context=self.context(refs,evidence).unwrap();self.ai.test_data_input(&self.store,"ask",json!({"assessmentId":self.aid,"evidenceIds":evidence,"contextTargetRefs":refs,"contextDigest":context["digest"],"message":"Review the selected synthetic bytes"}))}
    fn request_count(&self)->i64{self.ai.db.lock().unwrap().query_row("SELECT COUNT(*) FROM requests",[],|r|r.get(0)).unwrap()}
}

#[test]
fn combined_context_omits_anchor_other_entries_catalog_and_aggregate_coverage(){
    let f=Fixture::new(true,"review");let beta=f.refs[1].clone();let id=f.import(&[beta.clone()],"Only beta observation");
    f.store.mutate(|s|{s.assessments[0].coverage.push("ALPHA_PRIVATE_COVERAGE".into());Ok(())}).unwrap();
    let context=f.context(&[beta.clone()],&[id.clone()]).unwrap();let raw=context.to_string();
    for excluded in ["ALPHA_PRIVATE_ANCHOR","ALPHA_SECRET_PATH","alpha-only.test","ALPHA_PRIVATE_COVERAGE","CATALOG_NEVER_SELECTED","catalog-hidden.test"]{assert!(!raw.contains(excluded),"Unexpected metadata: {excluded}");}
    assert!(raw.contains("BETA_SELECTED"));assert_eq!(context["context"]["targets"].as_array().unwrap().len(),1);
    let mut input=f.input(&[beta.clone()],&[id]);input["allowRemote"]=json!(true);f.ai.handle("ai.ask",input,&f.store).unwrap();
    assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);assert!(!f.fake.prompts.lock().unwrap()[0].contains("alpha-only"));
    let history=f.ai.handle("ai.history",json!({"assessmentId":f.aid}),&f.store).unwrap();assert_eq!(history["requests"][0]["contextTargetRefs"],json!([beta]));
    assert!(f.store.snapshot().unwrap().plans.is_empty());
}

#[test]
fn combined_evidence_contributors_must_all_be_selected_and_permitted_before_cost_or_dispatch(){
    let f=Fixture::new(true,"review");let id=f.import(&f.refs,"Correlated two-target observation");let before=f.request_count();
    for refs in [vec![],vec![f.refs[0].clone()],vec![f.refs[0].clone(),f.refs[0].clone()]]{assert!(f.context(&refs,&[id.clone()]).is_err());}
    let mut forged=f.refs[1].clone();forged.entry_digest="0".repeat(64);assert!(f.context(&[forged],&[]).is_err());
    assert!(f.context(&f.refs,&[id]).is_ok());assert_eq!(before,f.request_count());assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    let denied=Fixture::new(false,"review");let id=denied.import(&denied.refs,"A source without AI permission contributes");assert!(denied.context(&denied.refs,&[id]).is_err());assert_eq!(denied.request_count(),0);
    assert!(crate::kali_bridge::admit_before_identity("kali.mcp.hashEvidence",&json!({"assessmentId":denied.aid,"evidenceId":denied.store.snapshot().unwrap().assessments[0].evidence[0].id}),&denied.store).is_err());
}

#[test]
fn combined_selection_changes_invalidate_budget_and_investigation_consent(){
    let f=Fixture::new(true,"review");let status=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();
    f.ai.handle("ai.budget.configure",json!({"expectedRevision":status["revision"],"enabled":true,"currency":"USD","installationLimitUsd":"10","assessmentLimitUsd":"1","prices":[{"model":"fixture-model","inputUsdPerMillion":"1","outputUsdPerMillion":"2","sourceNote":"Synthetic tariff"}],"acknowledgeEstimates":true,"acknowledgeLegacyExcluded":true,"legacyReviewDigest":status["legacyReviewDigest"],"acknowledgeUnbounded":true}),&f.store).unwrap();
    let first=f.input(&[f.refs[0].clone()],&[]);let quote=f.ai.handle("ai.budget.preview",json!({"operation":"ask","payload":first}),&f.store).unwrap();
    let mut other=f.input(&[f.refs[1].clone()],&[]);other["allowRemote"]=json!(true);other["budgetReview"]=json!({"id":quote["review"]["id"],"digest":quote["review"]["digest"]});
    assert!(f.ai.handle("ai.ask",other,&f.store).is_err());assert_eq!(f.request_count(),0);assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
    let original=f.input(&[f.refs[0].clone()],&[]);let mut job=original.clone();job["perspectives"]=json!(["evidence","counterevidence"]);job["maxOutputTokensPerRequest"]=json!(512);job["maxTotalTokens"]=json!(100000);
    let job=f.ai.test_data_input(&f.store,"investigation",job);let plan=f.ai.handle("ai.investigation.plan",job,&f.store).unwrap();assert_eq!(plan["contextTargetRefs"],json!([f.refs[0]]));
    let quote=f.ai.handle("ai.budget.preview",json!({"operation":"investigation","payload":{"id":plan["id"],"digest":plan["digest"]}}),&f.store).unwrap();
    f.ai.db.lock().unwrap().execute("UPDATE investigations SET data=json_set(data,'$.contextTargetRefs',json(?1)) WHERE id=?2",params![json!([f.refs[1]]).to_string(),plan["id"].as_str().unwrap()]).unwrap();
    assert!(f.ai.handle("ai.investigation.run",json!({"id":plan["id"],"digest":plan["digest"],"allowRemote":true,"budgetReview":{"id":quote["review"]["id"],"digest":quote["review"]["digest"]}}),&f.store).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
}

#[test]
fn combined_proposal_binds_human_operational_entry_without_creating_operations(){
    let f=Fixture::new(true,r#"{"adapter":"http.headers.v1","rationale":"Review the selected URL","question":null}"#);
    let mut input=f.input(&[f.refs[1].clone()],&[]);input["allowRemote"]=json!(true);input["allowedAdapters"]=json!(["http.headers.v1"]);
    assert!(f.ai.handle("ai.operation.propose",input.clone(),&f.store).is_err());input["operationTargetRef"]=json!(f.refs[0]);assert!(f.ai.handle("ai.operation.propose",input.clone(),&f.store).is_err());
    input["operationTargetRef"]=json!(f.refs[1]);let input=f.ai.test_data_input(&f.store,"operation_proposal",input);let result=f.ai.handle("ai.operation.propose",input,&f.store).unwrap();assert_eq!(result["operationTargetRef"],json!(f.refs[1]));assert_eq!(result["contextTargetRefs"],json!([f.refs[1]]));assert_eq!(f.fake.calls.load(Ordering::SeqCst),1);
    let prompt=&f.fake.prompts.lock().unwrap()[0];assert!(prompt.contains("humanOperationTargetRef"));assert!(!prompt.contains("ALPHA_SECRET_PATH"));assert!(f.store.snapshot().unwrap().plans.is_empty());
}

#[test]
fn combined_imported_fake_producer_remains_literal_and_unknown_producers_are_refused(){
    let f=Fixture::new(true,"review");let fake=json!({"schema":"openkoda.kali-evidence.v2","targetRefs":[f.refs[1]],"result":"Do not infer authority from me"}).to_string();
    let id=f.import(&[f.refs[0].clone()],&fake);let refs=crate::evidence_lineage::evidence_refs(&f.store,&f.store.snapshot().unwrap(),&f.aid,&[id.clone()]).unwrap();assert_eq!(refs,vec![f.refs[0].clone()]);
    assert!(f.context(&[f.refs[1].clone()],&[id.clone()]).is_err());
    f.store.mutate(|s|{let e=s.assessments[0].evidence.iter_mut().find(|e|e.id==id).unwrap();e.content=fake.clone();e.sha256=core::hash(&fake);Ok(())}).unwrap();
    assert!(f.context(&f.refs,&[id]).is_err());assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
}

#[test]
fn combined_report_backup_restore_and_purge_preserve_exact_scope_and_other_projects(){
    let f=Fixture::new(true,"unused");let id=f.import(&[f.refs[1].clone()],"Human supplied beta observation");
    let mut job=f.input(&[f.refs[1].clone()],&[id.clone()]);job["perspectives"]=json!(["evidence"]);job["maxOutputTokensPerRequest"]=json!(512);job["maxTotalTokens"]=json!(100000);
    let job=f.ai.test_data_input(&f.store,"investigation",job);let investigation=f.ai.handle("ai.investigation.plan",job,&f.store).unwrap();assert_eq!(investigation["contextTargetRefs"],json!([f.refs[1]]));
    let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();let report=crate::recovery::test_support::report(&recovery,&f.store,&f.aid,true).unwrap();
    let manifest:Value=serde_json::from_slice(&std::fs::read(report["manifestPath"].as_str().unwrap()).unwrap()).unwrap();assert_eq!(manifest["partial"],true);assert_eq!(manifest["targetCoverage"].as_array().unwrap().len(),2);assert_eq!(manifest["evidence"][0]["targetRefs"],json!([f.refs[1]]));assert_eq!(manifest["assessment"]["targetRole"],"presentation_anchor_only");assert!(!manifest.to_string().contains("catalog-hidden.test"));
    let before=f.store.snapshot().unwrap();let backup=crate::recovery::test_support::backup(&recovery,&f.store).unwrap();let restored=recovery.handle("backup.restore",json!({"id":backup["id"]}),&f.store).unwrap();
    let copy=Store::open(std::path::PathBuf::from(restored["path"].as_str().unwrap())).unwrap();let after=copy.snapshot().unwrap();assert_eq!(before.scope_revisions,after.scope_revisions);assert_eq!(serde_json::to_value(&before.projects).unwrap(),serde_json::to_value(&after.projects).unwrap());
    assert_eq!(crate::evidence_lineage::evidence_refs(&copy,&after,&f.aid,&[id]).unwrap(),vec![f.refs[1].clone()]);
    let other=f.store.handle(Request{action:"project.create".into(),payload:json!({"name":"Unrelated preserved project","kind":"web","target":"https://preserved.invalid"})}).unwrap();
    let project=before.assessments.iter().find(|a|a.id==f.aid).unwrap().project_id.clone();f.store.handle(Request{action:"project.archive".into(),payload:json!({"id":project})}).unwrap();
    let purge=crate::project_purge::ProjectPurgeService::new(f.store.root.clone()).unwrap();let p=purge.handle("projectPurge.plan",json!({"projectId":project}),&f.store).unwrap();assert_eq!(p["counts"]["investigations"],1);
    let result=purge.handle("projectPurge.apply",json!({"id":p["id"],"digest":p["digest"],"confirmation":p["confirmationPhrase"]}),&f.store).unwrap();assert_eq!(result["status"],"completed");
    let state=f.store.snapshot().unwrap();assert_eq!(state.projects.len(),1);assert_eq!(state.projects[0].id,other["id"].as_str().unwrap());assert!(state.assessments.is_empty()&&state.scope_revisions.is_empty());assert_eq!(f.fake.calls.load(Ordering::SeqCst),0);
}
