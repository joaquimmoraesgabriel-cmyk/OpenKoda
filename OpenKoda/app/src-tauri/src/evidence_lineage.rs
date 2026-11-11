//! Resolve selected evidence to host-recorded resource origins. This never opens
//! a target or interprets imported text as a producer record or authority.
use crate::{core::{self, Store}, model::{Evidence, Snapshot}, scope::{self, ScopeRef}, scope_targets::{self, ScopeTargetRef}};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

/// UI projection of host-validated bindings. `verified` confirms the binding,
/// never the truth of imported assertions or the absence of vulnerabilities.
pub fn projection(store:&Store,payload:Value)->Result<Value,String>{
    #[derive(Deserialize)]
    #[serde(rename_all="camelCase",deny_unknown_fields)]
    struct Input{assessment_id:String}
    let input:Input=serde_json::from_value(payload).map_err(error)?;
    core::valid_id(&input.assessment_id)?;
    let state=store.snapshot()?;
    let a=state.assessments.iter().find(|a|a.id==input.assessment_id).ok_or_else(||error("assessment"))?;
    let combined=scope_targets::is_combined(&state,&a.id);
    let row=|id_key:&str,id:&str,result:Result<Vec<ScopeTargetRef>,String>,kind:&str|{
        let mut value=serde_json::json!({"targetRefs":[],"verified":false,"originKind":kind});value[id_key]=Value::String(id.into());
        if !combined{value["reason"]=Value::String("legacy_single_target".into());}
        else{match result{Ok(refs)=>{value["verified"]=Value::Bool(true);value["targetRefs"]=serde_json::json!(refs);},Err(_)=>{value["reason"]=Value::String("origin_unverified".into());}}}
        value
    };
    let evidence=a.evidence.iter().map(|e|{
        let raw:Value=serde_json::from_str(&e.content).unwrap_or(Value::Null);
        let kind=if raw["schema"]=="openkoda.manual-evidence.v2"{"user_association"}else{"host_record"};
        row("evidenceId",&e.id,evidence_refs(store,&state,&a.id,&[e.id.clone()]),kind)
    }).collect::<Vec<_>>();
    let findings=state.findings.iter().filter(|f|f.assessment_id==a.id).map(|f|row("findingId",&f.id,finding_target_refs(store,&state,&f.id),"evidence_union")).collect::<Vec<_>>();
    Ok(serde_json::json!({"assessmentId":a.id,"combined":combined,"evidence":evidence,"findings":findings}))
}

fn error(_:impl std::fmt::Display)->String{"A origem da evidência não pôde ser vinculada às entradas revisadas. Nenhum envio foi autorizado.".into()}

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Manual {
    schema:String,origin:String,source_description:String,source_verified:bool,text:String,
    scope_ref:ScopeRef,target_refs:Vec<ScopeTargetRef>,
}

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct KaliDocument {
    schema:String,run_id:String,environment_id:String,source_sha256:String,source_evidence_id:Option<String>,
    scope_ref:ScopeRef,protocol_version:String,tool:String,result:Value,limits:String,target_refs:Vec<ScopeTargetRef>,
}

fn canonical(refs:Vec<ScopeTargetRef>)->Result<Vec<ScopeTargetRef>,String>{
    let mut out=BTreeMap::new();
    for reference in refs {
        core::valid_id(&reference.entry_id)?;
        if reference.entry_digest.len()!=64||!reference.entry_digest.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err(error("invalid origin digest"));}
        if let Some(previous)=out.insert(reference.entry_id.clone(),reference.clone()) {if previous!=reference{return Err(error("conflicting origin"));}}
    }
    if out.len()>8{return Err(error("too many origins"));}Ok(out.into_values().collect())
}

fn bound(s:&Snapshot,aid:&str,scope_ref:&ScopeRef,refs:Vec<ScopeTargetRef>)->Result<Vec<ScopeTargetRef>,String>{
    if scope_ref.assessment_id!=aid{return Err(error("cross assessment origin"));}
    scope::validate_historical_ref(s,aid,scope_ref)?;
    let expected=refs.len();let refs=canonical(refs)?;
    if expected!=refs.len(){return Err(error("duplicate origin"));}
    for target in &refs{scope_targets::resolve_historical_target(s,scope_ref,Some(target))?;}
    Ok(refs)
}

pub fn evidence_refs(store:&Store,s:&Snapshot,aid:&str,ids:&[String])->Result<Vec<ScopeTargetRef>,String>{
    core::valid_id(aid)?;
    if !s.assessments.iter().any(|a|a.id==aid){return Err(error("missing assessment"));}
    if !scope_targets::is_combined(s,aid){return Ok(vec![]);}
    if ids.len()>20||ids.iter().collect::<HashSet<_>>().len()!=ids.len(){return Err(error("invalid evidence selection"));}
    let mut refs=vec![];
    for id in ids{refs.extend(one(Some(store),s,aid,id,&mut HashSet::new())?);}
    canonical(refs)
}

/// Structural origins for invariant checking: external journal integrity is
/// additionally required by finding_target_refs before any new file access.
pub fn finding_source_refs(s:&Snapshot,finding_id:&str)->Result<Vec<ScopeTargetRef>,String>{finding_refs(None,s,finding_id)}
pub fn finding_target_refs(store:&Store,s:&Snapshot,finding_id:&str)->Result<Vec<ScopeTargetRef>,String>{finding_refs(Some(store),s,finding_id)}
fn finding_refs(store:Option<&Store>,s:&Snapshot,finding_id:&str)->Result<Vec<ScopeTargetRef>,String>{
    let finding=s.findings.iter().find(|f|f.id==finding_id).ok_or_else(||error("finding"))?;
    if !scope_targets::is_combined(s,&finding.assessment_id){return Ok(vec![]);}
    let assessment=s.assessments.iter().find(|a|a.id==finding.assessment_id).ok_or_else(||error("assessment"))?;
    let mut refs=vec![];
    for id in &finding.evidence_ids {
        let evidence=assessment.evidence.iter().find(|e|&e.id==id).ok_or_else(||error("missing finding evidence"))?;
        if evidence.sha256!=core::hash(&evidence.content){return Err(error("altered finding evidence"));}
        let raw:Value=serde_json::from_str(&evidence.content).unwrap_or(Value::Null);
        if raw["findingId"]==finding.id&&matches!(raw["schema"].as_str(),Some("openkoda.remediation.retest.v1"|"openkoda.remediation.confirmation.v1"|"openkoda.remediation.reconciliation.v1")){continue;}
        refs.extend(one(store,s,&assessment.id,id,&mut HashSet::new())?);
    }
    if refs.is_empty(){return Err(error("finding has no original resource evidence"));}canonical(refs)
}

fn one(store:Option<&Store>,s:&Snapshot,aid:&str,id:&str,seen:&mut HashSet<String>)->Result<Vec<ScopeTargetRef>,String>{
    core::valid_id(id)?;
    if seen.len()>=32||!seen.insert(id.into()){return Err(error("cyclic or excessive lineage"));}
    let result=resolve_one(store,s,aid,id,seen);seen.remove(id);result
}

fn resolve_one(store:Option<&Store>,s:&Snapshot,aid:&str,id:&str,seen:&mut HashSet<String>)->Result<Vec<ScopeTargetRef>,String>{
    let a=s.assessments.iter().find(|a|a.id==aid).ok_or_else(||error("assessment"))?;
    let e=a.evidence.iter().find(|e|e.id==id).ok_or_else(||error("evidence outside assessment"))?;
    if e.sha256!=core::hash(&e.content){return Err(error("altered evidence"));}
    let executions=s.executions.iter().filter(|r|r.assessment_id==aid&&r.evidence_id.as_deref()==Some(id)).collect::<Vec<_>>();
    if executions.len()>1{return Err(error("ambiguous producer"));}
    if let Some(execution)=executions.first(){
        if !matches!(execution.status.as_str(),"completed"|"failed"|"canceled"){return Err(error("unconfirmed producer"));}
        let plan=s.plans.iter().find(|p|p.id==execution.plan_id&&p.assessment_id==aid).ok_or_else(||error("missing plan"))?;
        if plan.digest!=crate::executor::plan_digest(plan){return Err(error("altered plan"));}
        let reference=plan.scope_ref.as_ref().ok_or_else(||error("missing producer scope"))?;
        let target=plan.target_ref.as_ref().ok_or_else(||error("missing producer target"))?;
        let resolved=scope_targets::resolve_historical_target(s,reference,Some(target))?;
        if resolved.target!=plan.target{return Err(error("producer target changed"));}
        return bound(s,aid,reference,vec![target.clone()]);
    }
    let raw:Value=serde_json::from_str(&e.content).map_err(error)?;
    if crate::source_context::is_source_capture(e,a,&s.plans){
        let plan=s.plans.iter().find(|p|Some(p.id.as_str())==raw["source"]["planId"].as_str()&&p.assessment_id==aid).ok_or_else(||error("capture plan"))?;
        return bound(s,aid,plan.scope_ref.as_ref().ok_or_else(||error("capture scope"))?,vec![plan.target_ref.clone().ok_or_else(||error("capture target"))?]);
    }
    if raw["schema"]=="openkoda.manual-evidence.v2"{
        let manual:Manual=serde_json::from_value(raw).map_err(error)?;
        if manual.schema!="openkoda.manual-evidence.v2"||manual.origin!="user_supplied"||manual.source_verified||manual.source_description.trim().is_empty()||manual.source_description.len()>4000||manual.text.trim().is_empty()||manual.text.len()>32768||manual.text.contains('\0')||manual.target_refs.is_empty(){return Err(error("invalid human association"));}
        // Only the outer host wrapper is decoded. Nested text stays literal.
        return bound(s,aid,&manual.scope_ref,manual.target_refs);
    }
    if raw["schema"]=="openkoda.kali-evidence.v2"{return kali(store,s,aid,e,raw,seen);}
    if let Some(refs)=crate::http_validation::evidence_target_refs(s,aid,e)?{return canonical(refs);}
    let remediation=match store{
        Some(store)=>crate::remediation::evidence_target_refs(&store.root.join("remediation.sqlite3"),s,aid,e)?,
        None=>crate::remediation::structural_evidence_target_refs(s,aid,e)?,
    };
    if let Some(refs)=remediation{return canonical(refs);}
    Err(error("unrecognized combined producer"))
}

fn kali(store:Option<&Store>,s:&Snapshot,aid:&str,e:&Evidence,raw:Value,seen:&mut HashSet<String>)->Result<Vec<ScopeTargetRef>,String>{
    let document:KaliDocument=serde_json::from_value(raw).map_err(error)?;
    core::valid_id(&document.run_id)?;
    if document.schema!="openkoda.kali-evidence.v2"||document.protocol_version!="2025-11-25"||document.result.is_null()||document.limits.is_empty(){return Err(error("invalid bridge document"));}
    if let Some(store)=store{
        let database=Connection::open_with_flags(store.root.join("kali-bridge.sqlite3"),OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
        let raw:Option<String>=database.query_row("SELECT data FROM bridge_runs WHERE id=?1",[&document.run_id],|row|row.get(0)).optional().map_err(error)?;
        let run:crate::kali_bridge::BridgeRun=serde_json::from_str(&raw.ok_or_else(||error("bridge producer missing"))?).map_err(error)?;
        if run.id!=document.run_id||run.assessment_id!=aid||run.environment_id!=document.environment_id||run.source_sha256!=document.source_sha256||run.tool!=document.tool||run.source_evidence_id!=document.source_evidence_id||run.evidence_id.as_deref()!=Some(e.id.as_str())||run.scope_ref.as_ref()!=Some(&document.scope_ref)||run.source_target_refs!=document.target_refs||!run.cleanup_confirmed||!matches!(run.status.as_str(),"completed"|"canceled") {return Err(error("bridge producer mismatch"));}
    }
    let refs=bound(s,aid,&document.scope_ref,document.target_refs)?;
    if document.tool=="runtime_diagnostics"{
        if document.source_evidence_id.is_some()||!refs.is_empty(){return Err(error("diagnostics cannot claim target coverage"));}
        let revision=s.scope_revisions.iter().find(|r|r.id==document.scope_ref.revision_id).ok_or_else(||error("scope"))?;
        if revision.body.target_set.as_ref().is_none_or(|set|!set.common_methods.iter().any(|m|m=="kali.runtime-diagnostics.v1")){return Err(error("diagnostic scope"));}
    }else{
        if !matches!(document.tool.as_str(),"hash_bytes"|"scan_bytes")||refs.is_empty(){return Err(error("bridge target missing"));}
        let parent=one(store,s,aid,document.source_evidence_id.as_deref().ok_or_else(||error("bridge input missing"))?,seen)?;
        if parent!=refs{return Err(error("bridge contributors changed"));}
    }
    Ok(refs)
}

#[cfg(test)]
pub(crate) fn test_scope(store:&Store,methods:&[&str],common:&[&str])->(String,Vec<ScopeTargetRef>){
    use serde_json::json;
    let project=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Combined host fixture","kind":"web","target":"http://127.0.0.1:47991/first"})}).unwrap();
    let pid=project["id"].as_str().unwrap();
    let options=scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();
    let added=scope_targets::handle_catalog("project.targetAdd",json!({"projectId":pid,"expectedCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":crate::model::id(),"label":"Second fixture","kind":"web","target":"http://127.0.0.1:47992/second"}),store).unwrap();
    let options=scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();
    let mut input=json!({"projectId":pid,"title":"Explicit combined fixture","objective":"Only selected local synthetic observations","window":{"startsAt":null,"endsAt":null},"projectCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":crate::model::id(),"selection":{"primaryProjectTargetId":"primary","entries":[{"projectTargetId":"primary","methods":methods},{"projectTargetId":added["targetId"],"methods":methods}],"commonMethods":common,"excludedProjectTargets":[],"localExclusions":[],"connectionExclusions":[]}});
    let preview=scope::handle("assessment.combinedPreview",input.clone(),store,str::to_owned).unwrap();input["reviewDigest"]=preview["reviewDigest"].clone();input["acknowledgeScope"]=json!(true);
    let created=scope::handle("assessment.combinedCreate",input,store,str::to_owned).unwrap();let aid=created["assessmentId"].as_str().unwrap().to_owned();
    let state=store.snapshot().unwrap();let refs=scope_targets::current_body(&state,&aid).unwrap().target_set.as_ref().unwrap().entries.iter().map(scope_targets::entry_ref).collect();
    (aid,refs)
}

#[cfg(all(test,windows))]
pub(crate) fn test_remediation_receipt()->(tempfile::TempDir,crate::executor::combined_fixture::Fixture,String,String){
    use serde_json::json;
    let source=tempfile::tempdir().unwrap();std::fs::write(source.path().join("settings.txt"),"secure=false\n").unwrap();
    let f=crate::executor::combined_fixture::Fixture::new(&[("files",source.path().to_str().unwrap(),&["local.text-remediation.v1","kali.rg.signals.v1"])],&[],json!([]));
    let ai=crate::ai::AiService::new(f.store.root.clone()).unwrap();
    let original=f.store.handle(core::Request{action:"evidence.import".into(),payload:json!({"assessmentId":f.aid,"name":"Owned synthetic condition","sourceDescription":"Human observation of the selected fixture","content":"secure=false","acknowledgeUnverifiedSource":true,"contextTargetRefs":[f.refs[0]]})}).unwrap();
    let finding=ai.handle("candidate.record",json!({"assessmentId":f.aid,"title":"Synthetic setting","description":"Only a fixture condition","severity":"low","evidenceIds":[original["id"]]}),&f.store).unwrap();let fid=finding["id"].as_str().unwrap().to_owned();
    f.store.handle(core::Request{action:"finding.validate".into(),payload:json!({"id":fid,"method":"human_review","expected":"secure=true","observed":"secure=false in owned fixture","evidenceIds":[original["id"]]})}).unwrap();
    let service=crate::remediation::RemediationService::new(f.store.root.clone()).unwrap();
    let plan=service.handle("remediation.prepare",json!({"findingId":fid,"scopeRef":scope::current_ref(&f.store.snapshot().unwrap(),&f.aid).unwrap().unwrap(),"targetRef":f.refs[0],"relativePath":"settings.txt","after":"secure=true\n","retest":{"kind":"contains","value":"secure=true"}}),&f.store).unwrap();
    let decision=json!({"id":plan["id"],"digest":plan["digest"]});service.handle("remediation.approve",decision.clone(),&f.store).unwrap();service.handle("remediation.apply",decision.clone(),&f.store).unwrap();let retest=service.handle("remediation.retest",decision,&f.store).unwrap();
    service.handle("remediation.confirm",json!({"id":plan["id"],"digest":plan["digest"],"evidenceId":retest["evidenceId"],"rationale":"Confirmed only the selected synthetic predicate"}),&f.store).unwrap();
    (source,f,retest["evidenceId"].as_str().unwrap().to_owned(),fid)
}

#[cfg(all(test,windows))]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]
    fn separate_finding_can_reference_an_authenticated_remediation_receipt(){
        let(_source,f,receipt,original_finding)=test_remediation_receipt();let ai=crate::ai::AiService::new(f.store.root.clone()).unwrap();
        let candidate=ai.handle("candidate.record",json!({"assessmentId":f.aid,"title":"Review a prior receipt","description":"A distinct hypothesis based on stored history","severity":"low","evidenceIds":[receipt]}),&f.store).unwrap();
        let s=f.store.snapshot().unwrap();for fid in [original_finding.as_str(),candidate["id"].as_str().unwrap()]{assert_eq!(finding_source_refs(&s,fid).unwrap(),vec![f.refs[0].clone()]);assert_eq!(finding_target_refs(&f.store,&s,fid).unwrap(),vec![f.refs[0].clone()]);}
        let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();assert!(crate::recovery::test_support::report(&recovery,&f.store,&f.aid,false).is_ok());
        let report=crate::recovery::test_support::report(&recovery,&f.store,&f.aid,false).unwrap();let text=std::fs::read_to_string(report["path"].as_str().unwrap()).unwrap();assert!(text.contains("Entradas de origem:")&&text.contains(&f.refs[0].entry_id)&&text.contains("Associação declarada pelo usuário"));
        let mut altered=s.clone();let e=altered.assessments[0].evidence.iter_mut().find(|e|e.id==receipt).unwrap();let mut raw:Value=serde_json::from_str(&e.content).unwrap();raw["targetRef"]["entryDigest"]=json!("0".repeat(64));e.content=raw.to_string();e.sha256=core::hash(&e.content);
        assert!(evidence_refs(&f.store,&altered,&f.aid,&[receipt]).is_err());
    }
    #[test]
    fn completed_adapter_with_omissions_reports_partial_target_coverage(){
        for binary in [true,false]{
            let source=tempfile::tempdir().unwrap();std::fs::write(source.path().join("included.txt"),if binary{b"\0binary".as_slice()}else{b"plain synthetic text\n".as_slice()}).unwrap();
            std::fs::write(source.path().join("excluded.txt"),"Unselected bytes").unwrap();
            let rules=if binary{vec![]}else{vec![(0,"excluded.txt","exact")]};
            let f=crate::executor::combined_fixture::Fixture::new(&[("code",source.path().to_str().unwrap(),&["source.rules.v1"])],&rules,json!([]));
            let plan=f.plan(0,"source.rules.v1");let execution=f.run(&plan);assert_eq!(execution["status"],"completed");
            let coverage=scope::target_coverage(&f.store.snapshot().unwrap(),&f.aid);assert_eq!(coverage[0]["methods"][0]["state"],"partial");
            let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();let report=crate::recovery::test_support::report(&recovery,&f.store,&f.aid,false).unwrap();
            let manifest:Value=serde_json::from_slice(&std::fs::read(report["manifestPath"].as_str().unwrap()).unwrap()).unwrap();assert_eq!(manifest["targetCoverage"][0]["methods"][0]["state"],"partial");
            let simple=crate::recovery::test_support::report(&recovery,&f.store,&f.aid,false).unwrap();let text=std::fs::read_to_string(simple["path"].as_str().unwrap()).unwrap();assert!(text.contains("source.rules.v1: partial")&&text.contains("apenas a referência de apresentação"));
        }
    }
    #[test]
    fn failed_execution_with_preserved_partial_keeps_origin_without_claiming_completion(){
        let source=tempfile::tempdir().unwrap();std::fs::write(source.path().join("example.py"),"eval(fixture)\n").unwrap();
        let f=crate::executor::combined_fixture::Fixture::new(&[("code",source.path().to_str().unwrap(),&["source.rules.v1"])],&[],json!([]));
        let planned=f.plan(0,"source.rules.v1");let execution=f.run(&planned);assert_eq!(execution["status"],"completed");let eid=execution["evidenceId"].as_str().unwrap().to_owned();
        let state=f.store.snapshot().unwrap();let plan=&state.plans[0];
        let partial=crate::source_scan::scan(source.path(),&plan.input_files,plan.source_scope.as_ref().unwrap(),&std::sync::atomic::AtomicBool::new(false),std::time::Instant::now()-std::time::Duration::from_secs(1),plan.output_limit_ki_b as usize*1024).unwrap();
        assert!(partial.partial&&partial.timed_out);let content=partial.to_json().unwrap();
        // Reconstruct the journal shape produced after a bounded adapter timeout.
        f.store.mutate(|s|{let a=&mut s.assessments[0];let e=a.evidence.iter_mut().find(|e|e.id==eid).unwrap();e.content=content.clone();e.sha256=core::hash(&content);a.status="failed".into();s.plans[0].status="failed".into();s.executions[0].status="failed".into();Ok(())}).unwrap();
        let state=f.store.snapshot().unwrap();assert_eq!(evidence_refs(&f.store,&state,&f.aid,&[eid.clone()]).unwrap(),vec![f.refs[0].clone()]);
        let coverage=scope::target_coverage(&state,&f.aid);assert_eq!(coverage[0]["methods"][0]["state"],"failed");assert_eq!(coverage[0]["methods"][0]["partial"],true);
        let view=projection(&f.store,json!({"assessmentId":f.aid})).unwrap();assert_eq!(view["evidence"][0]["verified"],true);
        let recovery=crate::recovery::RecoveryService::new(f.store.root.clone()).unwrap();let report=crate::recovery::test_support::report(&recovery,&f.store,&f.aid,true).unwrap();assert_eq!(report["partial"],true);
        let manifest:Value=serde_json::from_slice(&std::fs::read(report["manifestPath"].as_str().unwrap()).unwrap()).unwrap();assert_eq!(manifest["executions"][0]["status"],"failed");assert_eq!(manifest["evidence"][0]["targetRefs"],json!([f.refs[0]]));
    }
}
