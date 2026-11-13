//! Code-review admission, scoped source transfer and durable review receipts.
use crate::{ai::{AiService, HttpScan, ScanTransport}, ai_provider::ProviderId, core::{self, Store}, model::{self, Assessment, Evidence, Snapshot}, scope, scope_targets};
use serde::{Deserialize, Serialize};
use serde_json::{json,Value};
use std::{collections::HashSet, fs, path::{Path,PathBuf}, sync::{Arc,Mutex,atomic::{AtomicBool,Ordering},mpsc},time::{Duration,Instant}};
use zeroize::Zeroizing;

#[path="security_review.rs"]
mod review;
const INTERRUPTED:&str="Execução interrompida. O resultado remoto e o custo final podem ser desconhecidos.";

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Progress {at:String,stage:String,message:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Receipt {
    id:String,assessment_id:String,idempotency_key:String,request_digest:String,status:String,stage:String,message:String,
    created_at:String,updated_at:String,provider:String,model:String,max_cost_usd:f64,events:Vec<Progress>,
    result_evidence_id:Option<String>,findings_total:Option<u64>,error:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]selected_paths:Option<Vec<String>>,
}
impl Receipt {
    pub(crate) fn id(&self) -> &str { &self.id }
    fn active(&self)->bool{matches!(self.status.as_str(),"starting"|"running"|"cancelling")}
    fn update(&mut self,status:&str,stage:&str,message:&str){
        self.status=status.into();self.stage=stage.into();self.message=message.into();self.updated_at=model::now();
        if self.events.len()>=64{self.events.remove(1);}
        self.events.push(Progress{at:self.updated_at.clone(),stage:stage.into(),message:message.into()});
    }
}
#[derive(Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Start{assessment_id:String,idempotency_key:String,prompt:Option<String>,target:Option<Targets>,max_cost_usd:f64,#[serde(default,skip_serializing_if="std::ops::Not::not")]acknowledge_sensitive:bool,#[serde(default,skip_serializing_if="Option::is_none")]retry_of:Option<String>}
#[derive(Serialize,Deserialize)]
#[serde(untagged)]
enum Targets{One(String),Many(Vec<String>)}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Lookup {assessment_id:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Status {assessment_id:Option<String>}

#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Finding{#[serde(default,skip_serializing_if="Option::is_none")]id:Option<String>,title:String,severity:String,description:String,location:Option<String>}
#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Coverage{
    #[serde(default,skip_serializing_if="Vec::is_empty")]reviewed_ranges:Vec<String>,
    scope:Vec<String>,excluded_paths:Vec<String>,additional_exclusions:u64,findings_total:u64,report_truncated:bool,
    #[serde(default,skip_serializing_if="String::is_empty")]summary:String,
    #[serde(default,skip_serializing_if="Vec::is_empty")]limitations:Vec<String>,
}
#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Cost{estimated_usd:Option<f64>}
#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ResultData {scan_id:Option<String>,report:String,findings:Vec<Finding>,coverage:Coverage,cost:Option<Cost>}
struct Terminal{status:String,data:Option<ResultData>,error:Option<String>}
impl Terminal {
    fn failure(message:&str)->Self{Self{status:"failed".into(),data:None,error:Some(message.into())}}
}
struct Scope {repo:PathBuf,targets:Vec<String>,deadline:Instant,_locks:Vec<crate::source_scan::MetadataLease>}
struct Job<'a> {id:String,aid:String,prompt:String,objective_digest:String,acknowledge_sensitive:bool,scope:Scope,provider:ProviderId,model:String,cap:f64,key:Zeroizing<String>,cancel:Arc<AtomicBool>,_inference:std::sync::MutexGuard<'a,()>}
struct Active {aid:String,cancel:Arc<AtomicBool>}
pub(crate) struct Service{transport:Arc<dyn ScanTransport>,active:Mutex<Option<Active>>}

fn decode<T:serde::de::DeserializeOwned>(value:Value)->Result<T,String>{serde_json::from_value(value).map_err(|_|"Campos inválidos para a análise OpenKoda.".into())}
fn lookup<'a>(s:&'a Snapshot,aid:&str)->Result<&'a Assessment,String>{s.assessments.iter().find(|a|a.id==aid).ok_or_else(||"Avaliação não encontrada.".into())}
fn latest(a:&Assessment)->Option<&Receipt>{a.events.iter().rev().find_map(|e|e.security_scan.as_ref())}
fn public(a:&Assessment,r:&Receipt)->Result<Value,String>{
    let mut v=json!(r);
    let o=v.as_object_mut().unwrap();o.remove("requestDigest");o.remove("idempotencyKey");
    let result=if let Some(id)=&r.result_evidence_id{
        let ev=a.evidence.iter().find(|e|&e.id==id).ok_or("O recibo não possui a evidência vinculada.")?;
        let data=serde_json::from_str::<ResultData>(&ev.content).map_err(|_|"Relatório de análise inválido no armazenamento.")?;
        json!(data)
    }else{Value::Null};
    o.insert("result".into(),result);Ok(json!({"scan":v}))
}
fn local_subject_reasons(store:&Store,s:&Snapshot,a:&Assessment,subject:&crate::data_policy::DataSubject)->Result<Vec<String>,String>{
    use crate::data_policy::{self,DataDestination,DataOperation,DataPolicyService,DataSubject,DataUseSpec,RequestFingerprint,Sensitivity};
    use crate::data_projection::{resolve_subject,subject_versions,RejectExternal};
    let resolved=resolve_subject(store,s,subject,&RejectExternal)?;
    let mut reasons=Vec::new();
    // Check each contributor separately: an unclassified activity must not
    // dilute a Sensitive source into a separately allowed Unclassified package.
    for version in subject_versions(store,s,subject,&RejectExternal)?{
        let classification=data_policy::current_classification(s,&version.subject);
        let retained=classification.filter(|c|c.source_fingerprint!=version.source_fingerprint).map(|c|c.sensitivity);
        let sensitivity=classification.filter(|c|c.source_fingerprint==version.source_fingerprint).map_or(Sensitivity::Unclassified,|c|c.sensitivity);
        let mut spec=DataUseSpec::new(DataOperation::DerivedRead,DataDestination::LocalRawView,vec![a.project_id.clone()],Some(a.id.clone()));
        spec.selection.package_sensitivity=sensitivity;spec.selection.categories=resolved.categories.clone();spec.selection.origins=resolved.origins.clone();
        if let DataSubject::Evidence{evidence_id,..}=&version.subject{spec.selection.evidence_ids.push(evidence_id.clone());}
        spec.requests=vec![RequestFingerprint{index:0,input_sha256:version.source_fingerprint.clone(),output_sha256:version.source_fingerprint.clone(),input_bytes:0,output_bytes:0}];
        spec.subjects=vec![version];reasons.extend(DataPolicyService::check_use(s,&spec)?.reason_codes);
        if let Some(sensitivity)=retained{
            // A progress append changes the activity fingerprint while retaining
            // older bytes. Keep its last observed sensitivity as a policy floor
            // until a human explicitly classifies the current Subject again.
            // This extra policy-only check creates no new classification ref.
            spec.subjects.clear();spec.selection.package_sensitivity=sensitivity;
            reasons.extend(DataPolicyService::check_use(s,&spec)?.reason_codes);
        }
    }
    Ok(reasons)
}
fn content_reasons(store:&Store,s:&Snapshot,a:&Assessment,r:&Receipt)->Vec<String>{
    use crate::data_policy::{DataCategory,DataSubject};
    let decision=(||{
        let reference=scope::current_ref(s,&a.id)?.ok_or("data_source_unknown")?;
        let body=scope_targets::current_body(s,&a.id)?;
        let mut subjects=vec![
            DataSubject::AssessmentMetadata{assessment_id:a.id.clone(),category:DataCategory::ActivityMetadata},
            DataSubject::AssessmentMetadata{assessment_id:a.id.clone(),category:DataCategory::HumanText},
        ];
        if let Some(set)=&body.target_set{
            for entry in &set.entries{subjects.push(DataSubject::ScopeSource{assessment_id:a.id.clone(),scope_ref:reference.clone(),target_ref:Some(scope_targets::entry_ref(entry))});}
        }else{subjects.push(DataSubject::ScopeSource{assessment_id:a.id.clone(),scope_ref:reference,target_ref:None});}
        if let Some(id)=&r.result_evidence_id{subjects.push(DataSubject::Evidence{assessment_id:a.id.clone(),evidence_id:id.clone()});}
        let mut reasons=Vec::new();for subject in subjects{reasons.extend(local_subject_reasons(store,s,a,&subject)?);}
        for parent in source_ancestors(s,&a.id)?{
            let source=lookup(s,&parent.scope.assessment_id)?;
            let subject=source_subject(s,&parent.scope)?;
            reasons.extend(local_subject_reasons(store,s,source,&subject)?);
            if let Some(pin)=&parent.policy{
                let rule=&s.data_policy_revisions.iter().find(|r|crate::data_policy::policy_ref(r)==*pin)
                    .ok_or("Revisão da política da origem ausente.")?.body.local_raw_view;
                let level=crate::data_policy::current_classification(s,&subject).map_or(crate::data_policy::Sensitivity::Unclassified,|c|c.sensitivity);
                if !rule.enabled||!rule.sensitivities.contains(&level){reasons.push("data_destination_denied".into());}
            }
        }
        reasons.sort();reasons.dedup();Ok::<_,String>(reasons)
    })();
    decision.unwrap_or_else(|_|vec!["data_source_unknown".into()])
}
/// Projections of a scan report and its candidates share the receipt's local
/// disclosure decision. Validation can replace a candidate's evidence links,
/// so also recover its stable ID from the original report.
#[derive(Default)]
pub(crate) struct RestrictedScanContent {
    pub evidence: HashSet<String>,
    pub findings: HashSet<String>,
}
pub(crate) fn restricted_results(store:&Store,s:&Snapshot,a:&Assessment)->RestrictedScanContent {
    let mut hidden=RestrictedScanContent::default();
    for receipt in a.events.iter().filter_map(|e|e.security_scan.as_ref()) {
        let Some(id)=receipt.result_evidence_id.as_ref() else{continue};
        if content_reasons(store,s,a,receipt).is_empty(){continue;}
        hidden.evidence.insert(id.clone());
        let report=a.evidence.iter().find(|e|e.id==*id)
            .and_then(|e|serde_json::from_str::<ResultData>(&e.content).ok());
        if let Some(report)=report {
            hidden.findings.extend(report.findings.iter().enumerate()
                .map(|(index,f)|candidate_id(&a.id,&receipt.id,index,f)));
        }
        hidden.findings.extend(s.findings.iter().filter(|f|f.assessment_id==a.id && f.evidence_ids.contains(id)).map(|f|f.id.clone()));
    }
    hidden
}
fn candidate_id(aid:&str,scan_id:&str,index:usize,f:&Finding)->String{
    let origin=f.id.clone().unwrap_or_else(||format!("{index}:{}",core::hash(&json!(f).to_string())));
    scope_targets::derived_id("openkoda.codex-security.finding.v1",&[aid,scan_id,&origin])
}
pub(crate) fn check_report_read(store:&Store,s:&Snapshot,subject:&crate::data_policy::DataSubject)->Result<(),String>{
    use crate::data_policy::{DataSubject,DerivedProducer};
    let a=lookup(s,subject.assessment_id())?;
    let hidden=restricted_results(store,s,a);
    let denied=match subject {
        DataSubject::Evidence{evidence_id,..}=>hidden.evidence.contains(evidence_id),
        DataSubject::Derived{producer:DerivedProducer::Finding,object_id,..}=>hidden.findings.contains(object_id),
        DataSubject::AssessmentMetadata{category:crate::data_policy::DataCategory::ActivityMetadata,..}=>
            a.events.iter().filter_map(|e|e.security_scan.as_ref()).any(|r|!content_reasons(store,s,a,r).is_empty()),
        _=>false,
    };
    if denied {Err("data_destination_denied: a política restringe a leitura desta análise.".into())} else {Ok(())}
}
fn project_content(store:&Store,s:&Snapshot,a:&Assessment,r:&Receipt,value:&mut Value){
    let reasons=content_reasons(store,s,a,r);if reasons.is_empty(){return;}
    // SDK stages, errors and public assistant messages can contain repository
    // code. Keep control metadata, but never treat those strings as metadata.
    const HIDDEN:&str="Conteúdo indisponível pela política de dados deste projeto.";
    value["message"]=json!(HIDDEN);value["stage"]=json!("restricted");
    if !value["error"].is_null(){value["error"]=json!(HIDDEN);}
    if let Some(events)=value["events"].as_array_mut(){for event in events{event["message"]=json!(HIDDEN);event["stage"]=json!("restricted");}}
    if let Some(o)=value.as_object_mut(){o.remove("selectedPaths");}
    value["contentUnavailable"]=json!(true);value["contentReasonCodes"]=json!(reasons);
    if value.get("result").is_some(){value["result"]=Value::Null;value["resultUnavailable"]=json!(true);value["resultReasonCodes"]=value["contentReasonCodes"].clone();}
}
fn public_checked(store:&Store,s:&Snapshot,a:&Assessment,r:&Receipt)->Result<Value,String>{
    let mut value=public(a,r)?;project_content(store,s,a,r,&mut value["scan"]);Ok(value)
}
/// Same policy boundary for the duplicated Event.message and nested receipt.
/// Snapshot keeps its receipt shape: full reports remain exclusive to scan.get.
pub(crate) fn event_view(store:&Store,s:&Snapshot,a:&Assessment,event:&model::Event)->Result<Value,String>{
    let mut value=serde_json::to_value(event).map_err(|_|"data_projection_failed")?;
    if let Some(receipt)=&event.security_scan{
        project_content(store,s,a,receipt,&mut value["securityScan"]);
        value["message"]=value["securityScan"]["message"].clone();
    }
    Ok(value)
}
fn record(store:&Store,aid:&str,id:&str,edit:impl FnOnce(&mut Receipt)->Result<(),String>)->Result<(),String>{
    store.mutate(|s|{
        let a=s.assessments.iter_mut().find(|a|a.id==aid).ok_or("Avaliação ausente.")?;
        let e=a.events.iter_mut().find(|e|e.security_scan.as_ref().is_some_and(|r|r.id==id)).ok_or("Recibo do SDK ausente.")?;
        let r=e.security_scan.as_mut().unwrap();edit(r)?;e.message=r.message.clone();a.updated_at=r.updated_at.clone();Ok(())
    })
}
fn currency(value:f64)->Result<u64,String>{
    if !value.is_finite()||value<0.0||value>1_000_000.0{return Err("Valor monetário inválido.".into());}
    Ok((value*1_000_000.0).ceil() as u64)
}
fn source_ancestors<'a>(s:&'a Snapshot,aid:&str)->Result<Vec<&'a scope::ConversationSource>,String>{
    let mut parents=Vec::new();
    let mut seen=HashSet::new();
    let mut cursor=aid;
    while let Some(parent)=s.scope_receipts.iter().find(|r|r.action=="project.newConversation"&&r.result.assessment_id==cursor)
        .and_then(|r|r.result.conversation_source.as_ref()) {
        if !seen.insert(parent.scope.assessment_id.as_str()){return Err("Origem circular da conversa.".into());}
        parents.push(parent);
        cursor=&parent.scope.assessment_id;
    }
    Ok(parents)
}

fn source_subject(s:&Snapshot,reference:&scope::ScopeRef)->Result<crate::data_policy::DataSubject,String>{
    let revision=s.scope_revisions.iter().find(|r|r.id==reference.revision_id&&r.digest==reference.digest&&r.assessment_id==reference.assessment_id)
        .ok_or("Escopo de origem ausente.")?;
    let entry=revision.body.target_set.as_ref().and_then(|set|set.entries.iter().find(|e|e.entry_id==set.primary_entry_id));
    Ok(crate::data_policy::DataSubject::ScopeSource{assessment_id:reference.assessment_id.clone(),scope_ref:reference.clone(),target_ref:entry.map(scope_targets::entry_ref)})
}

fn check_data(s:&Snapshot,aid:&str,provider:ProviderId)->Result<bool,String>{
    let mut sensitive=check_source_data(s,aid,provider,None,true)?;
    for parent in source_ancestors(s,aid)? {
        let source=&parent.scope.assessment_id;
        scope::check_new_local_input(s,source)?;
        if scope::current_ref(s,source)?.as_ref()!=Some(&parent.scope){
            return Err("O escopo da conversa de origem mudou. Selecione novamente o alvo autorizado.".into());
        }
        let classified=crate::data_policy::current_classification(s,&source_subject(s,&parent.scope)?).is_some();
        if parent.policy.is_some()||classified||s.data_bindings.iter().any(|b|&b.assessment_id==source) {
            sensitive|=check_source_data(s,source,provider,parent.policy.as_ref(),false)?;
        }
    }
    Ok(sensitive)
}

fn check_source_data(s:&Snapshot,aid:&str,provider:ProviderId,pinned:Option<&crate::data_policy::DataPolicyRef>,check_objective:bool)->Result<bool,String>{
    use crate::data_policy::{self,DataCategory,DataDestination,DataSubject,Sensitivity};
    let a=lookup(s,aid)?;
    let Some(policy)=s.data_policies.iter().find(|p|p.project_id==a.project_id)else{return Ok(false)};
    if !policy.active{return Err("A política de dados deste projeto foi revogada.".into());}
    let binding=s.data_bindings.iter().find(|b|b.assessment_id==aid).ok_or("A avaliação precisa adotar a política de dados vigente antes de enviar fontes.")?;
    let scope=scope::current_ref(s,aid)?.ok_or("Revise o escopo da avaliação.")?;
    let body=scope_targets::current_body(s,aid)?;
    let entry=body.target_set.as_ref().and_then(|set|set.entries.iter().find(|e|e.entry_id==set.primary_entry_id));
    let target=entry.map_or("primary",|e|e.project_target_id.as_str());
    let mut subjects=vec![source_subject(s,&scope)?];
    if check_objective{subjects.push(DataSubject::AssessmentMetadata{assessment_id:aid.into(),category:DataCategory::HumanText});}
    let mut sensitivity=Sensitivity::Internal;
    for subject in subjects{
        let fingerprint=crate::data_projection::canonical_fingerprint(s,&subject)?;
        let c=data_policy::current_classification(s,&subject).filter(|c|Some(&c.source_fingerprint)==fingerprint.as_ref())
            .ok_or("Revise a classificação da fonte e do objetivo desta avaliação.")?;
        if c.secret_material||c.sensitivity==Sensitivity::Unclassified{return Err("A classificação vigente impede o envio desta fonte.".into());}
        sensitivity=sensitivity.max(c.sensitivity);
    }
    for id in [&policy.current_revision_id,&binding.policy_ref.revision_id].into_iter().chain(pinned.map(|r|&r.revision_id)){
        let rev=s.data_policy_revisions.iter().find(|r|&r.id==id&&r.project_id==a.project_id).ok_or("Revisão da política de dados ausente.")?;
        let rule=rev.body.destinations.iter().find(|r|r.destination==DataDestination::RemoteAi).ok_or("Destino de IA remota ausente.")?;
        if !rule.enabled||!rule.target_catalog_ids.iter().any(|t|t==target)||!rule.sensitivities.contains(&sensitivity)
            ||[DataCategory::HumanText,DataCategory::TargetMetadata,DataCategory::SourceExcerpt].iter().any(|cat|!rule.categories.contains(cat)){
            return Err("A política de dados vigente não permite enviar o objetivo e as fontes deste alvo ao provedor.".into());
        }
    }
    let slot=crate::credential_refs::provider_slot(s,provider).ok_or("Configure a credencial deste provedor.")?;
    let permitted=s.credential_references.iter().any(|r|r.project_id==a.project_id&&!r.revoked
        &&r.provider_slot_ref.as_ref().is_some_and(|p|p.slot_id==slot.id)
        &&crate::credential_refs::current(s,&a.project_id,&r.reference_id).is_some_and(|latest|latest.id==r.id));
    if !permitted{return Err("A referência deste provedor está ausente ou revogada no projeto.".into());}
    Ok(sensitivity==Sensitivity::Sensitive)
}
fn check_disclosure(s:&Snapshot,aid:&str,provider:ProviderId,prompt_digest:&str,acknowledge_sensitive:bool)->Result<(),String>{
    let a=lookup(s,aid)?;
    if s.data_policies.iter().any(|p|p.project_id==a.project_id)&&core::hash(a.objective.trim())!=prompt_digest{
        return Err("Este projeto usa classificação de dados. Use o objetivo revisado desta avaliação para a varredura.".into());
    }
    if check_data(s,aid,provider)?&&!acknowledge_sensitive{
        return Err("Confirme explicitamente o envio da fonte classificada como sensível ao provedor.".into());
    }
    Ok(())
}
fn approved_scope(s:&Snapshot,input:&Start)->Result<Scope,String>{
    let aid=&input.assessment_id;
    let a=lookup(s,aid)?;let p=s.projects.iter().find(|p|p.id==a.project_id&&!p.archived).ok_or("Projeto ausente ou arquivado.")?;
    if !matches!(a.kind.as_str(),"code"|"files")||!matches!(p.kind.as_str(),"code"|"files"){return Err("OpenKoda exige um repositório local de código ou arquivos.".into());}
    scope::check_new_local_input(s,aid)?;
    let body=scope_targets::current_body(s,aid)?;
    let refs=if let Some(set)=&body.target_set{
        if set.entries.len()!=1||!set.local_exclusions.is_empty()||!set.excluded_project_targets.is_empty(){return Err("A análise precisa de uma seleção local única que corresponda às exclusões do escopo.".into());}
        vec![scope_targets::entry_ref(&set.entries[0])]
    }else{vec![]};
    for method in ["source.rules.v1","ai.remote-review.v1"]{
        scope::check_selected_work(s,aid,method,None,&refs)?;
    }
    let repo=PathBuf::from(&p.target);
    let mut locks=vec![crate::source_scan::lock_metadata_target(&repo)?];
    if !repo.is_dir(){return Err("Selecione a pasta do repositório como raiz do projeto.".into());}
    let target=Path::new(&a.target);
    let relative=target.strip_prefix(&repo).map_err(|_|"A avaliação não pertence à raiz deste projeto.")?;
    let mut paths=match &input.target{
        Some(Targets::One(v))=>vec![v.clone()],Some(Targets::Many(v)) if !v.is_empty()=>v.clone(),
        Some(_)=>return Err("Selecione pelo menos um caminho relativo.".into()),
        None if relative.as_os_str().is_empty()=>vec![],None=>vec![relative.to_string_lossy().into_owned()],
    };
    if paths.len()>64{return Err("Selecione no máximo 64 caminhos.".into());}
    let mut seen=HashSet::new();
    for path in &mut paths{
        *path=scope_targets::normalize_relative(path)?;
        if path.len()>512||path.contains(['[',']','{','}'])||!seen.insert(path.to_lowercase()){return Err("Caminho repetido, longo ou com expressão de seleção.".into());}
        let selected=repo.join(&*path);
        if !selected.starts_with(target){return Err("Seleção fora do alvo autorizado nesta avaliação.".into());}
        locks.push(crate::source_scan::lock_metadata_target(&selected)?);
    }
    for parent in source_ancestors(s,aid)?{
        let original=lookup(s,&parent.scope.assessment_id)?;
        let source_root=Path::new(&original.target);
        if !target.starts_with(source_root){return Err("A seleção excede o alvo da conversa de origem.".into());}
        let Some(id)=&parent.scan_id else{continue};
        let allowed=original.events.iter().filter_map(|event|event.security_scan.as_ref()).find(|r|&r.id==id)
            .and_then(|r|r.selected_paths.as_ref()).ok_or("Os caminhos da análise de origem estão indisponíveis. Selecione o alvo em uma nova análise.")?;
        if !allowed.is_empty() && (paths.is_empty()||paths.iter().any(|path|!allowed.iter().any(|p|Path::new(path).starts_with(p)))) {
            return Err("A seleção excede os arquivos da análise de origem.".into());
        }
    }
    let mut timeout=Duration::from_secs(2*60*60);
    if let Some(end)=&body.window.ends_at{let time=chrono::DateTime::parse_from_rfc3339(end).map_err(|_|"Janela de escopo inválida.")?;timeout=timeout.min((time.with_timezone(&chrono::Utc)-chrono::Utc::now()).to_std().map_err(|_|"Escopo expirado.")?);}
    Ok(Scope{repo,targets:paths,deadline:Instant::now()+timeout,_locks:locks})
}
fn check_repository_boundary(profile:&Path,repo:&Path,project:&model::Project)->Result<(),String>{
    let root=fs::canonicalize(profile).map_err(|_|"Perfil ausente.")?;
    let canonical_repo=fs::canonicalize(repo).map_err(|_|"Repositório ausente.")?;
    // The profile itself and its ancestors can never be an analysis root.
    // A descendant is allowed only for the exact managed GitHub checkout whose
    // ready import receipt still matches every persisted source identity field.
    if root.starts_with(&canonical_repo){return Err("O diretório de dados do aplicativo e o repositório da análise precisam ser separados.".into());}
    if canonical_repo.starts_with(&root){
        let declared=project.github_source.as_ref().ok_or("A pasta dentro do perfil não corresponde a uma importação GitHub revisada.")?;
        let attested=crate::github_repository::source(profile,&declared.import_id,&project.target)?;
        let target=fs::canonicalize(&project.target).map_err(|_|"A pasta importada não está disponível.")?;
        if canonical_repo!=target||attested.import_id!=declared.import_id||attested.url!=declared.url||attested.revision!=declared.revision{
            return Err("A identidade do repositório GitHub mudou. Crie uma nova importação antes de analisar.".into());
        }
    }
    Ok(())
}
impl Service {
    pub(crate) fn new(_resources:&Path)->Self{Self{transport:Arc::new(HttpScan),active:Mutex::new(None)}}
    pub(crate) fn status(&self,store:&Store,ai:&AiService,payload:Value)->Result<Value,String>{
        let input:Status=decode(payload)?;
        let (provider,model,key)=ai.security_selection(store,input.assessment_id.as_deref())?;
        let runtime=json!({"available":true,"engine":"isolated-source","version":env!("CARGO_PKG_VERSION")});
        let reason=if !key{Some("Configure a chave deste provedor.")}
            else if !crate::ai_provider::valid_model(&model){Some("Selecione e salve um modelo.")}else{None};
        let sensitive=if let Some(aid)=input.assessment_id.as_deref(){check_data(&store.snapshot()?,aid,provider).unwrap_or(false)}else{false};
        Ok(json!({"available":reason.is_none(),"reason":reason,"runtime":runtime,"provider":provider,"model":model,"hasApiKey":key,"requiresSensitiveConfirmation":sensitive}))
    }
    pub(crate) fn get(&self,store:&Store,payload:Value)->Result<Value,String>{
        let q:Lookup=decode(payload)?;let s=store.snapshot()?;let a=lookup(&s,&q.assessment_id)?;
        latest(a).map_or(Ok(json!({"scan":null})),|r|public_checked(store,&s,a,r))
    }
    pub(crate) fn cancel(&self,store:&Store,payload:Value)->Result<Value,String>{
        let q:Lookup=decode(payload.clone())?;let active=self.active.lock().map_err(|_|"Estado do SDK indisponível.")?;
        if let Some(job)=active.as_ref().filter(|job|job.aid==q.assessment_id){
            let s=store.snapshot()?;let a=lookup(&s,&q.assessment_id)?;
            if let Some(r)=latest(a).filter(|r|r.active()){
                record(store,&q.assessment_id,&r.id,|r|{if r.active()&&r.status!="cancelling"{r.update("cancelling","cancel","Cancelamento da análise solicitado.");}Ok(())})?;
                job.cancel.store(true,Ordering::Release);
            }
        }
        drop(active);self.get(store,payload)
    }
    pub(crate) fn recover(store:&Store)->Result<(),String>{
        if !store.snapshot()?.assessments.iter().any(|a|a.events.iter().any(|e|e.security_scan.as_ref().is_some_and(Receipt::active))){return Ok(());}
        store.mutate(|s|{
            for a in &mut s.assessments{for e in &mut a.events{if let Some(r)=e.security_scan.as_mut().filter(|r|r.active()){
                r.update("interrupted","interrupted",INTERRUPTED);r.error=Some(INTERRUPTED.into());e.message=r.message.clone();e.kind="warning".into();a.updated_at=r.updated_at.clone();
            }}}
            Ok(())
        })
    }
    fn prepare<'a>(&self,store:&Store,ai:&'a AiService,mut input:Start)->Result<(Value,Option<Job<'a>>),String>{
        core::valid_id(&input.assessment_id)?;core::valid_id(&input.idempotency_key)?;
        let digest=core::hash(&serde_json::to_string(&input).map_err(|_|"Pedido inválido.")?);
        let mut active=self.active.lock().map_err(|_|"Estado da varredura indisponível.")?;
        let state=store.snapshot()?;let a=lookup(&state,&input.assessment_id)?;
        if let Some(r)=a.events.iter().filter_map(|e|e.security_scan.as_ref()).find(|r|r.idempotency_key==input.idempotency_key){
            if r.request_digest!=digest{return Err("A mesma chave de execução já representa outra solicitação.".into());}
            return Ok((public_checked(store,&state,a,r)?,None));
        }
        if active.is_some(){return Err("Já existe uma varredura OpenKoda em andamento.".into());}
        if !(0.01..=100.0).contains(&input.max_cost_usd){return Err("Informe um limite de US$ 0,01 a US$ 100.".into());}
        let prompt=input.prompt.as_deref().filter(|p|!p.trim().is_empty()).unwrap_or(&a.objective).trim();
        if prompt.len()>8192||prompt.contains('\0'){return Err("Descrição da varredura inválida.".into());}
        let inference=ai.security_lease()?;
        let _gate=store.scope_gate.lock().map_err(|_|"Escopo indisponível.")?;
        let state=store.snapshot()?;
        if let Some(previous_id)=&input.retry_of {
            let prior=latest(lookup(&state,&input.assessment_id)?).filter(|r|r.id==*previous_id&&!r.active()).ok_or("O registro da análise mudou. Consulte o estado atual.")?;
            let paths=prior.selected_paths.as_ref().ok_or("Esta versão histórica não registrou a seleção de arquivos da análise anterior.")?;
            if input.target.is_some(){return Err("Uma repetição deve preservar os caminhos anteriores.".into());}
            if !paths.is_empty(){input.target=Some(Targets::Many(paths.clone()));}
        }
        let scope=approved_scope(&state,&input)?;
        let (provider,model,_)=ai.security_selection(store,Some(&input.assessment_id))?;
        let objective_digest=core::hash(prompt);
        check_disclosure(&state,&input.assessment_id,provider,&objective_digest,input.acknowledge_sensitive)?;
        let key=ai.security_key(store,&input.assessment_id,provider,&model)?;
        use crate::data_projection::{DataRedactor,RedactionMode};
        let redactor=ai.data_redactor();
        if redactor.mode()==RedactionMode::MetadataOnly{return Err("Credenciais indisponíveis para revisar o objetivo da varredura.".into());}
        let prompt=redactor.redact(prompt).text;
        let project=state.projects.iter().find(|p|p.id==a.project_id).ok_or("Projeto ausente.")?;
        check_repository_boundary(&store.root,&scope.repo,project)?;
        let id=model::id();let at=model::now();
        let mut r=Receipt{id:id.clone(),assessment_id:input.assessment_id.clone(),idempotency_key:input.idempotency_key,request_digest:digest,status:String::new(),stage:String::new(),message:String::new(),created_at:at.clone(),updated_at:at,provider:provider.id().into(),model:model.clone(),max_cost_usd:input.max_cost_usd,events:vec![],error:None,result_evidence_id:None,findings_total:None,selected_paths:Some(scope.targets.clone())};
        r.update("starting","starting","Preparando o OpenKoda.");
        ai.reserve_security(&id,&input.assessment_id,provider,&model,currency(input.max_cost_usd)?)?;
        let receipt=store.mutate(|s|{
            scope::check_new_local_input(s,&input.assessment_id)?;scope::freeze_review(s,&input.assessment_id,"security.scan.start")?;
            let a=s.assessments.iter_mut().find(|a|a.id==input.assessment_id).ok_or("Avaliação ausente.")?;
            let mut e=model::event(&r.message,"info");e.security_scan=Some(r.clone());a.events.push(e);
            a.updated_at=r.updated_at.clone();public(a,&r)
        });
        let receipt=match receipt{Ok(v)=>v,Err(e)=>{ai.security_not_sent(&id)?;return Err(e);}};
        let cancel=Arc::new(AtomicBool::new(false));
        *active=Some(Active{aid:input.assessment_id.clone(),cancel:cancel.clone()});
        Ok((receipt,Some(Job{id,aid:input.assessment_id,prompt,objective_digest,acknowledge_sensitive:input.acknowledge_sensitive,scope,provider,model,cap:input.max_cost_usd,key,cancel,_inference:inference})))
    }
    pub(crate) fn start_worker(&self,store:&Store,ai:&AiService,input:Value,ready:&mpsc::SyncSender<Result<Value,String>>)->Result<(),String>{
        let (receipt,job)=self.prepare(store,ai,decode(input)?)?;
        let _=ready.send(Ok(receipt));
        if let Some(job)=job{
            let mut dispatched=false;
            let mut accounting=review::Usage::default();
            let result=review::run(self.transport.as_ref(),store,ai,&job,&mut dispatched,&mut accounting);
            let mut terminal=result.unwrap_or_else(|e|Terminal::failure(&e));
            if job.cancel.load(Ordering::Acquire) && terminal.status!="cancelled" {
                terminal.status="cancelled".into();terminal.error=Some("Análise cancelada.".into());
            }
            let settled=if dispatched{ai.security_settle(&job.id,accounting.estimate())}else{ai.security_not_sent(&job.id)};
            if settled.is_err(){terminal.error=Some("A conciliação financeira da execução ainda está pendente.".into());}
            let saved=finish(store,&job.aid,&job.id,terminal);
            if let Ok(mut slot)=self.active.lock(){*slot=None;}
            saved?;
        }
        Ok(())
    }
}
fn finish(store:&Store,aid:&str,id:&str,terminal:Terminal)->Result<(),String>{
    store.mutate(|s|{
        let a=s.assessments.iter_mut().find(|a|a.id==aid).ok_or("Avaliação ausente.")?;
        let e=a.events.iter_mut().find(|e|e.security_scan.as_ref().is_some_and(|r|r.id==id)).ok_or("Recibo ausente.")?;
        let r=e.security_scan.as_mut().unwrap();if !r.active(){return Err("Esta execução já terminou.".into());}
        if let Some(data)=terminal.data {
            let content=serde_json::to_string_pretty(&data).map_err(|_|"Relatório inválido.")?;
            let ev=Evidence{id:model::id(),name:"Relatório do OpenKoda".into(),sha256:core::hash(&content),content,created_at:model::now()};
            if matches!(terminal.status.as_str(),"completed"|"partial"){
                for (index,f) in data.findings.iter().enumerate(){
                    let severity=f.severity.to_lowercase();
                    // Unknown or informational severities stay verbatim in the
                    // source report. The domain has four risk levels.
                    if f.title.trim().is_empty()||!matches!(severity.as_str(),"low"|"medium"|"high"|"critical"){continue;}
                    let fid=candidate_id(aid,id,index,f);
                    if s.findings.iter().any(|f|f.id==fid){continue;}
                    let description=match f.location.as_deref(){Some(path)=>format!("{}\n\nLocal no código: {path}",f.description),None=>f.description.clone()};
                    s.findings.push(model::Finding{id:fid,assessment_id:aid.into(),title:f.title.clone(),severity,confidence:"candidate".into(),status:"candidate".into(),description,recommendation:String::new(),evidence_ids:vec![ev.id.clone()],created_at:model::now()});
                }
            }
            r.result_evidence_id=Some(ev.id.clone());r.findings_total=Some(data.coverage.findings_total);a.evidence.push(ev);
        }
        let message=terminal.error.as_deref().unwrap_or(match terminal.status.as_str(){"completed"=>"Análise concluída pelo OpenKoda.","partial"=>"Análise parcial. Consulte a cobertura.","cancelled"=>"Análise cancelada.",_=>INTERRUPTED});
        r.update(&terminal.status,&terminal.status,message);e.message=r.message.clone();
        e.kind=if r.status=="completed"{"success"}else{"warning"}.into();a.updated_at=r.updated_at.clone();r.error=terminal.error;Ok(())
    })
}
pub(crate) fn project(mut value:Value,redactor:&dyn crate::data_projection::DataRedactor)->Value{
    // The SDK report is already bounded by its wire schema. The generic
    // 2 KiB preview would silently discard most of a reviewed Markdown report.
    if redactor.mode()==crate::data_projection::RedactionMode::MetadataOnly{
        return crate::data_projection::project_value(value,redactor);
    }
    fn scrub(v:&mut Value,r:&dyn crate::data_projection::DataRedactor){match v{
        Value::String(s)=>*s=r.redact(s).text,
        Value::Array(rows)=>rows.iter_mut().for_each(|v|scrub(v,r)),
        Value::Object(o)=>o.values_mut().for_each(|v|scrub(v,r)),
        _=>{}
    }}
    scrub(&mut value,redactor);value
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    use std::thread;
    struct OfflineReview;
    impl ScanTransport for OfflineReview {
        fn model_settings(&self,_:&AiService,_:ProviderId,_:&str,_:&str)->Result<crate::ai::ScanModel,String>{Ok(crate::ai::ScanModel::text(crate::ai::ScanRates{input:1_000_000,output:2_000_000}))}
        fn review(&self,_:ProviderId,_:&str,key:&str,input:&str,_:&str,_:u32,_:crate::ai::ScanModel,stop:&AtomicBool,_:Instant)->Result<Value,String>{
            assert!(!input.contains(key));
            assert!(!input.contains("fake-private-key-of-second-configured-provider"));
            assert!(!input.contains("UNSELECTED_SOURCE_CANARY"));
            let prompt:Value=serde_json::from_str(input).unwrap();
            if prompt["objective"]=="wait"{while !stop.load(Ordering::Acquire){thread::sleep(Duration::from_millis(10));}return Err("Cancelado.".into());}
            if prompt["objective"]=="missing-terminal"{return Err("Resposta interrompida.".into());}
            let has_test=prompt["sources"].as_array().unwrap().iter().any(|source|source["path"]=="src/test.txt");
            let mut findings=vec![];
            if has_test || prompt["objective"]=="unverifiable"{
                let path=if prompt["objective"]=="unverifiable"{"../../secret.txt"}else{"src/test.txt"};
                findings.push(json!({"path":path,"line":1,"excerpt":"fixture","title":"Fixture finding","description":"Fixture details","severity":"high"}));
            }
            let answer=if prompt["objective"]=="cross-file" && prompt["previousQueries"].as_array().unwrap().is_empty(){
                json!({"queries":[{"op":"search","query":"separate"},{"op":"read","path":"outside.txt","line":1}]})
            }else{json!({"summary":"Fixture report","findings":findings})};
            Ok(json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":answer.to_string()}]}],"usage":{"input_tokens":20,"output_tokens":10}}))
        }
    }
    const KEY:&str="fixture-security-key-not-for-a-provider";
    struct Fixture{dir:tempfile::TempDir,store:Store,ai:AiService,service:Service,aid:String}
    impl Fixture{
        fn new(relative:Option<&str>)->Self{
            Self::with_managed_source(relative,false)
        }
        fn with_managed_source(relative:Option<&str>,managed:bool)->Self{
            let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().join("profile")).unwrap();
            let import_id=managed.then(model::id);
            let repo=import_id.as_ref().map(|id|store.root.join("github-repositories").join(id).join("repository")).unwrap_or_else(||dir.path().join("repo"));
            fs::create_dir_all(repo.join("src")).unwrap();
            fs::write(repo.join("src/test.txt"),b"fixture").unwrap();fs::write(repo.join("outside.txt"),b"separate").unwrap();
            if let Some(id)=&import_id{
                let receipt=json!({"id":id,"url":"https://github.com/octocat/Hello-World","name":"Hello-World","status":"ready","message":"Synthetic ready import","revision":"a".repeat(40),"createdAt":model::now(),"updatedAt":model::now()});
                fs::write(repo.parent().unwrap().join("receipt.json"),serde_json::to_vec(&receipt).unwrap()).unwrap();
            }
            let ai=AiService::new_with_budget_root(store.root.clone(),store.root.clone()).unwrap();
            ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
            let body=json!({"idempotencyKey":model::id(),"name":"Synthetic source","kind":"code","root":repo,"title":"Review","objective":"Local synthetic fixture","useRipgrep":false,"ports":[],"relativeTarget":relative,"githubImportId":import_id});
            let created=store.handle(core::Request{action:"project.launch".into(),payload:body}).unwrap();
            let aid=created["assessmentId"].as_str().unwrap().to_owned();
            let service=Service{transport:Arc::new(OfflineReview),active:Mutex::new(None)};
            Self{dir,store,ai,service,aid}
        }
        fn input(&self,prompt:&str)->Value{json!({"assessmentId":self.aid,"idempotencyKey":model::id(),"prompt":prompt,"maxCostUsd":1.0})}
        fn run(&self,body:Value)->Value{let(tx,rx)=mpsc::sync_channel(1);self.service.start_worker(&self.store,&self.ai,body,&tx).unwrap();rx.recv().unwrap().unwrap()}
        fn get(&self)->Value{self.service.get(&self.store,json!({"assessmentId":self.aid})).unwrap()}
    }
    #[test]
    fn composer_analysis_publishes_only_after_admission_and_reuses_the_scoped_objective(){
        let mut f=Fixture::new(Some("src"));
        let state=f.store.snapshot().unwrap();
        let key=model::id();
        let create=json!({"projectId":state.projects[0].id,"sourceAssessmentId":f.aid,"firstTurn":true,
            "title":"Revisão da API","analysisObjective":"Revisar somente a API selecionada.","idempotencyKey":key});
        let child=f.store.handle(core::Request{action:"project.newConversation".into(),payload:create.clone()}).unwrap();
        assert_eq!(f.store.handle(core::Request{action:"project.newConversation".into(),payload:create}).unwrap()["id"],child["id"]);
        assert_eq!(child["objective"],"Revisar somente a API selecionada.");
        assert_eq!(child["target"],state.assessments[0].target);
        let snapshot=||crate::data_host::project_response("snapshot",json!({}),&f.store,&f.ai.data_redactor()).unwrap();
        assert_eq!(snapshot()["assessments"].as_array().unwrap().len(),1);
        let mut body=json!({"assessmentId":child["id"],"idempotencyKey":key,"maxCostUsd":0.0});
        let (tx,_)=mpsc::sync_channel(1);
        assert!(f.service.start_worker(&f.store,&f.ai,body.clone(),&tx).is_err());
        assert_eq!(snapshot()["assessments"].as_array().unwrap().len(),1);
        body["maxCostUsd"]=json!(0.1);
        f.run(body.clone());
        assert_eq!(snapshot()["assessments"].as_array().unwrap().len(),2);
        f.aid=child["id"].as_str().unwrap().to_owned();
        assert_eq!(f.get()["scan"]["status"],"completed");
        assert_eq!(f.run(body),f.get());
    }
    #[test]fn managed_github_checkout_runs_without_authorizing_profile_or_unattested_paths(){
        let f=Fixture::with_managed_source(None,true);
        let state=f.store.snapshot().unwrap();let project=&state.projects[0];let repo=Path::new(&project.target);
        check_repository_boundary(&f.store.root,repo,project).unwrap();
        for forbidden in [f.store.root.clone(),f.dir.path().to_path_buf(),f.store.root.join("github-repositories"),repo.join("src")]{
            assert!(check_repository_boundary(&f.store.root,&forbidden,project).is_err(),"{}",forbidden.display());
        }
        let mut changed=project.clone();changed.github_source=None;
        assert!(check_repository_boundary(&f.store.root,repo,&changed).is_err());
        for field in ["url","revision","importId"]{
            let mut changed=project.clone();let source=changed.github_source.as_mut().unwrap();
            match field{"url"=>source.url="https://github.com/octocat/Other".into(),"revision"=>source.revision="b".repeat(40),_=>source.import_id=model::id()};
            assert!(check_repository_boundary(&f.store.root,repo,&changed).is_err(),"{field}");
        }
        f.run(f.input("complete"));assert_eq!(f.get()["scan"]["status"],"completed");
        assert_eq!(f.store.snapshot().unwrap().findings.len(),1);
    }
    #[test]fn sdk_activity_text_obeys_local_policy_before_and_after_result(){
        use crate::data_policy::{DataPolicyService,test_support::TestRedactor};
        use crate::data_projection::{self,RejectExternal};
        const CANARY:&str="SDK_PRIVATE_SOURCE_CANARY_a7f24";
        for completed in [false,true]{
            let f=Fixture::new(None);let input=f.input("complete");
            if completed{f.run(input.clone());}else{let(_,job)=f.service.prepare(&f.store,&f.ai,decode(input.clone()).unwrap()).unwrap();drop(job);}
            let id=latest(&f.store.snapshot().unwrap().assessments[0]).unwrap().id.clone();
            record(&f.store,&f.aid,&id,|r|{r.update(if completed{"completed"}else{"running"},CANARY,CANARY);r.error=Some(CANARY.into());Ok(())}).unwrap();
            f.ai.test_data_authority(&f.store,&f.aid);
            let views=||{
                let state=f.store.snapshot().unwrap();
                [f.get(),f.service.cancel(&f.store,json!({"assessmentId":f.aid})).unwrap(),f.run(input.clone()),data_projection::snapshot_view(&f.store,&state,&TestRedactor).unwrap()]
            };
            for view in views(){assert!(view.to_string().contains(CANARY),"allowed activity must remain available");}
            let policy=DataPolicyService::new();let state=f.store.snapshot().unwrap();let pid=state.projects[0].id.clone();
            let call=|action:&str,payload:Value|policy.handle(action,payload,&f.store,&TestRedactor,&RejectExternal,None).unwrap();
            let original=state.data_policy_revisions.last().unwrap().body.clone();
            let change=|body:crate::data_policy::DataPolicyBody|{let status=call("data.policy.status",json!({"projectId":pid}));let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":body}));call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));};
            let mut denied=original.clone();denied.local_raw_view.enabled=false;change(denied);
            for view in views(){assert!(!view.to_string().contains(CANARY),"SDK activity bypassed local read policy: {view}");}
            if completed {
                let state=f.store.snapshot().unwrap();
                let evidence=&state.assessments[0].evidence[0];
                let finding=&state.findings[0];
                let snapshot=data_projection::snapshot_view(&f.store,&state,&TestRedactor).unwrap();
                assert!(snapshot["assessments"][0]["evidence"][0]["preview"].is_null());
                assert!(!snapshot["findings"][0]["description"].as_str().unwrap().contains("Fixture details"));
                let mutation=crate::data_host::project_response("finding.update",json!(finding),&f.store,&TestRedactor).unwrap();
                assert!(!mutation.to_string().contains("Fixture details"));
                let subject=crate::data_policy::DataSubject::Evidence{assessment_id:f.aid.clone(),evidence_id:evidence.id.clone()};
                assert!(check_report_read(&f.store,&state,&subject).is_err());
            }
            change(original);
            for view in views(){assert!(view.to_string().contains(CANARY),"reallow must not erase canonical activity");}
            let status=call("data.policy.status",json!({"projectId":pid}));call("data.policy.revoke",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"acknowledgeRevocation":true}));
            for view in views(){assert!(!view.to_string().contains(CANARY),"SDK activity bypassed revoked policy");}
            assert!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap().contains(CANARY));
        }
    }
    #[test]fn sdk_activity_inherits_each_sensitive_subject_and_adopted_local_policy(){
        use crate::data_policy::{self,DataPolicyService,DataSubject,DataCategory,test_support::TestRedactor};
        use crate::data_projection::{self,RejectExternal};
        const CANARY:&str="SDK_SENSITIVE_ACTIVITY_CANARY_3db21";
        for selected in ["source","activity","human","result","adopted_source"]{
            let f=Fixture::new(None);let input=f.input("complete");f.run(input.clone());
            let id=latest(&f.store.snapshot().unwrap().assessments[0]).unwrap().id.clone();
            record(&f.store,&f.aid,&id,|r|{r.update("completed",CANARY,CANARY);Ok(())}).unwrap();
            f.ai.test_data_authority(&f.store,&f.aid);
            let policy=DataPolicyService::new();let state=f.store.snapshot().unwrap();let pid=state.projects[0].id.clone();
            let call=|action:&str,payload:Value|policy.handle(action,payload,&f.store,&TestRedactor,&RejectExternal,None).unwrap();
            let change=|body:data_policy::DataPolicyBody|{let status=call("data.policy.status",json!({"projectId":pid}));let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":body}));call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));};
            let adopt=||{let status=call("data.policy.status",json!({"projectId":pid,"assessmentId":f.aid}));let current=&status["currentRevision"];let reference=json!({"projectId":pid,"revisionId":current["id"],"digest":current["digest"]});let review=call("data.binding.prepare",json!({"assessmentId":f.aid,"policyRef":reference,"expectedStateDigest":status["stateDigest"]}));call("data.binding.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeProspectiveAdoption":true}));};
            let original=state.data_policy_revisions.last().unwrap().body.clone();let mut limited=original.clone();limited.local_raw_view.sensitivities.retain(|s|*s!=data_policy::Sensitivity::Sensitive);change(limited);
            if selected=="adopted_source"{adopt();change(original.clone());}
            let state=f.store.snapshot().unwrap();let a=&state.assessments[0];
            let subject=match selected{
                "source"|"adopted_source"=>state.data_classifications.iter().find(|c|matches!(c.subject,DataSubject::ScopeSource{..})).unwrap().subject.clone(),
                "activity"=>DataSubject::AssessmentMetadata{assessment_id:f.aid.clone(),category:DataCategory::ActivityMetadata},
                "human"=>DataSubject::AssessmentMetadata{assessment_id:f.aid.clone(),category:DataCategory::HumanText},
                _=>DataSubject::Evidence{assessment_id:f.aid.clone(),evidence_id:latest(a).unwrap().result_evidence_id.clone().unwrap()},
            };
            let fingerprint=data_projection::canonical_fingerprint(&state,&subject).unwrap().unwrap();
            let review=call("data.classification.prepare",json!({"subject":subject,"expectedSourceFingerprint":fingerprint,"sensitivity":"sensitive","reason":"Synthetic source-bearing SDK activity"}));
            call("data.classification.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeClassification":true}));
            let views=||{let state=f.store.snapshot().unwrap();[f.get(),f.service.cancel(&f.store,json!({"assessmentId":f.aid})).unwrap(),f.run(input.clone()),data_projection::snapshot_view(&f.store,&state,&TestRedactor).unwrap()]};
            for view in views(){assert!(!view.to_string().contains(CANARY),"{selected}: sensitive SDK activity leaked");}
            if selected=="adopted_source"{adopt();}else{change(original);}
            for view in views(){assert!(view.to_string().contains(CANARY),"{selected}: permitted activity was lost");}
            let state=f.store.snapshot().unwrap();let snapshot=data_projection::snapshot_view(&f.store,&state,&TestRedactor).unwrap();
            let event=snapshot["assessments"][0]["events"].as_array().unwrap().iter().find(|e|e["securityScan"].is_object()).unwrap();
            assert!(event["securityScan"].get("result").is_none(),"snapshot must never gain a full SDK report");
        }
    }
    #[test]fn sdk_activity_retains_sensitive_floor_after_progress_or_metadata_changes(){
        use crate::data_policy::{self,DataPolicyService,DataSubject,DataCategory,test_support::TestRedactor};
        use crate::data_projection::{self,RejectExternal};
        const CANARY:&str="SDK_STALE_CLASSIFICATION_CANARY_b94da";
        for selected in ["activity","human"]{
            let f=Fixture::new(None);let input=f.input("complete");f.run(input.clone());
            let id=latest(&f.store.snapshot().unwrap().assessments[0]).unwrap().id.clone();
            record(&f.store,&f.aid,&id,|r|{r.update("completed","message",CANARY);Ok(())}).unwrap();
            f.ai.test_data_authority(&f.store,&f.aid);
            let policy=DataPolicyService::new();let state=f.store.snapshot().unwrap();let pid=state.projects[0].id.clone();
            let call=|action:&str,payload:Value|policy.handle(action,payload,&f.store,&TestRedactor,&RejectExternal,None).unwrap();
            let status=call("data.policy.status",json!({"projectId":pid}));
            let mut limited=state.data_policy_revisions.last().unwrap().body.clone();limited.local_raw_view.sensitivities.retain(|s|*s!=data_policy::Sensitivity::Sensitive);
            let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":limited}));
            call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));
            let subject=DataSubject::AssessmentMetadata{assessment_id:f.aid.clone(),category:if selected=="activity"{DataCategory::ActivityMetadata}else{DataCategory::HumanText}};
            let classify=|sensitivity:&str|{let state=f.store.snapshot().unwrap();let fingerprint=data_projection::canonical_fingerprint(&state,&subject).unwrap().unwrap();let review=call("data.classification.prepare",json!({"subject":subject,"expectedSourceFingerprint":fingerprint,"sensitivity":sensitivity,"reason":"Explicit synthetic activity classification"}));call("data.classification.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeClassification":true,"acknowledgeDowngrade":true}));};
            let views=||{let state=f.store.snapshot().unwrap();[f.get(),f.service.cancel(&f.store,json!({"assessmentId":f.aid})).unwrap(),f.run(input.clone()),data_projection::snapshot_view(&f.store,&state,&TestRedactor).unwrap()]};
            classify("sensitive");
            for view in views(){assert!(!view.to_string().contains(CANARY),"current Sensitive classification must deny activity");}
            if selected=="activity"{record(&f.store,&f.aid,&id,|r|{r.update("completed","message","Next public progress message");Ok(())}).unwrap();}
            else{f.store.mutate(|s|{s.assessments[0].summary.push_str(" Updated reviewed metadata");Ok(())}).unwrap();}
            let state=f.store.snapshot().unwrap();let prior=data_policy::current_classification(&state,&subject).unwrap();
            assert_ne!(Some(&prior.source_fingerprint),data_projection::canonical_fingerprint(&state,&subject).unwrap().as_ref());
            for view in views(){assert!(!view.to_string().contains(CANARY),"{selected}: new progress/metadata diluted Sensitive into Unclassified");}
            classify("internal");
            for view in views(){assert!(view.to_string().contains(CANARY),"explicit current Internal classification must restore permitted activity");}
            assert!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap().contains(CANARY));
        }
    }
    #[test]fn stored_report_respects_local_read_policy_and_revocation_on_get_and_replay(){
        use crate::data_policy::{DataPolicyService,test_support::TestRedactor};
        use crate::data_projection::RejectExternal;
        let f=Fixture::new(None);let input=f.input("complete");f.run(input.clone());
        f.ai.test_data_authority(&f.store,&f.aid);
        assert!(f.get()["scan"]["result"]["report"].as_str().unwrap().contains("Fixture report"));
        let policy=DataPolicyService::new();let state=f.store.snapshot().unwrap();let pid=state.projects[0].id.clone();
        let call=|action:&str,payload:Value|policy.handle(action,payload,&f.store,&TestRedactor,&RejectExternal,None).unwrap();
        let status=call("data.policy.status",json!({"projectId":pid}));
        let mut body=state.data_policy_revisions.last().unwrap().body.clone();body.local_raw_view.enabled=false;
        let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":body}));
        call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));
        let blocked=f.get();assert!(blocked["scan"]["result"].is_null(),"{blocked}");
        assert_eq!(blocked["scan"]["resultUnavailable"],true);
        assert!(!blocked.to_string().contains("Fixture report"));
        assert!(f.run(input.clone())["scan"]["result"].is_null());
        let status=call("data.policy.status",json!({"projectId":pid}));
        let mut allowed=state.data_policy_revisions.last().unwrap().body.clone();allowed.local_raw_view.enabled=true;
        let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":allowed}));
        call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));
        assert!(f.get()["scan"]["result"]["report"].as_str().unwrap().contains("Fixture report"));
        let status=call("data.policy.status",json!({"projectId":pid}));
        call("data.policy.revoke",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"acknowledgeRevocation":true}));
        assert!(f.get()["scan"]["result"].is_null());assert!(f.run(input)["scan"]["result"].is_null());
        let stored=f.store.snapshot().unwrap();assert!(stored.assessments[0].evidence[0].content.contains("Fixture report"));
        assert_eq!(stored.findings.len(),1);
    }
    #[test]fn synthetic_sdk_receipt_import_budget_and_retry(){
        let f=Fixture::new(None);let body=f.input("complete");assert!(f.get()["scan"].is_null());
        f.run(body.clone());let stored=f.get();let scan=&stored["scan"];
        assert_eq!(scan["status"],"completed","{scan}");assert_eq!(scan["findingsTotal"],1);
        assert_eq!(scan["result"]["scanId"],scan["id"]);assert!(!stored.to_string().contains(KEY));
        let state=f.store.snapshot().unwrap();assert_eq!(state.findings.len(),1);assert_eq!(state.findings[0].status,"candidate");
        assert_eq!(state.findings[0].evidence_ids[0],scan["resultEvidenceId"]);assert_eq!(state.assessments[0].evidence.len(),1);
        let ledger=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();
        assert_eq!(ledger["recentChargesTotal"],1);assert_eq!(ledger["recentCharges"][0]["assumedChargeUsd"].as_str().unwrap(),"0.000040");
        assert_eq!(f.run(body.clone()),stored);let mut changed=body;changed["prompt"]=json!("changed");
        assert!(f.service.prepare(&f.store,&f.ai,decode(changed).unwrap()).is_err());
        assert_eq!(f.store.snapshot().unwrap().findings.len(),1);
        assert!(!f.dir.path().join("profile/security-scans").exists());
    }
    #[test]fn all_configured_providers_reach_scoped_review_and_preserve_idempotency(){
        for provider in ProviderId::ALL{
            let f=Fixture::new(None);
            f.ai.handle("ai.key.save",json!({"provider":provider,"key":KEY}),&f.store).unwrap();
            f.ai.handle("ai.selection.save",json!({"assessmentId":f.aid,"aiSelection":{"provider":provider,"model":"fixture-model"}}),&f.store).unwrap();
            let input=f.input("complete");
            assert_eq!(f.service.status(&f.store,&f.ai,json!({"assessmentId":f.aid})).unwrap()["available"],true);
            f.run(input.clone());
            let receipt=f.get();assert_eq!(receipt["scan"]["provider"],provider.id());assert_eq!(receipt["scan"]["status"],"completed","{receipt}");
            assert_eq!(f.run(input),receipt);assert_eq!(f.store.snapshot().unwrap().findings.len(),1);
        }
    }
    #[test]fn openai_code_scan_keeps_cap_and_legacy_router_receipts_remain_replayable(){
        let f=Fixture::new(None);let mut body=f.input("complete");body["maxCostUsd"]=json!(5.0);
        assert_eq!(f.service.status(&f.store,&f.ai,json!({"assessmentId":f.aid})).unwrap()["available"],true);
        f.run(body.clone());let completed=f.get();
        assert_eq!(completed["scan"]["provider"],"openai");assert_eq!(completed["scan"]["maxCostUsd"],5.0);
        let id=completed["scan"]["id"].as_str().unwrap();
        // An existing historical receipt is retained when the supported new-scan providers change.
        record(&f.store,&f.aid,id,|r|{r.provider="openrouter".into();r.model="openai/gpt-6-luna".into();Ok(())}).unwrap();
        f.ai.handle("ai.key.save",json!({"provider":"openrouter","key":KEY}),&f.store).unwrap();
        f.ai.handle("ai.selection.save",json!({"assessmentId":f.aid,"aiSelection":{"provider":"openrouter","model":"openai/gpt-6-luna"}}),&f.store).unwrap();

        let expected=f.get();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
        let budget=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();
        assert_eq!(f.run(body.clone()),expected);
        assert_eq!(f.service.cancel(&f.store,json!({"assessmentId":f.aid})).unwrap(),expected);
        assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);
        assert_eq!(f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap(),budget);
        body["prompt"]=json!("different request");
        let error=f.service.prepare(&f.store,&f.ai,decode(body).unwrap()).err().unwrap();
        assert!(error.contains("mesma chave"));
    }
    #[test]fn follow_up_queries_only_read_the_snapshot_and_each_inference_is_metered(){
        let f=Fixture::new(None);
        let body=f.input("cross-file");f.run(body.clone());
        let scan=&f.get()["scan"];assert_eq!(scan["status"],"completed","{scan}");
        assert!(scan["events"].as_array().unwrap().iter().any(|e|e["stage"]=="source_query" && e["message"].as_str().unwrap().contains("2 trecho")));
        let charges=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();
        let charge=&charges["recentCharges"][0];
        assert_eq!(charge["resolution"]["requests"],2);
        assert_eq!(charge["resolution"]["partialRequests"],0);
        assert_eq!(charge["inputTokens"],40);
        assert_eq!(charge["assumedChargeUsd"],"0.000080");
        assert_eq!(f.run(body)["scan"]["id"],scan["id"]);
    }

    #[test]
    fn model_queries_cannot_expand_an_assessment_to_a_sibling_file() {
        let f = Fixture::new(Some("src"));
        fs::write(f.dir.path().join("repo/src/second.rs"), "fn local_only() {}").unwrap();
        fs::write(f.dir.path().join("repo/outside.txt"), "separate UNSELECTED_SOURCE_CANARY").unwrap();
        let mut broader = f.input("cross-file");
        broader["target"] = json!("outside.txt");
        assert!(f.service.prepare(&f.store, &f.ai, decode(broader).unwrap()).is_err());

        f.run(f.input("cross-file"));
        let receipt = f.get();
        let scan = &receipt["scan"];
        assert_eq!(scan["status"], "completed", "{scan}");
        assert!(scan["events"].as_array().unwrap().iter().any(|event|
            event["stage"] == "source_query" && event["message"].as_str().unwrap().contains("0 trecho")));
        assert!(!receipt.to_string().contains("UNSELECTED_SOURCE_CANARY"));
    }
    #[test]fn rejects_unlocated_output_while_retaining_measured_cost(){
        let f=Fixture::new(None);f.run(f.input("unverifiable"));
        assert_eq!(f.get()["scan"]["status"],"failed");
        assert!(f.store.snapshot().unwrap().findings.is_empty());
        let ledger=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();
        assert_eq!(ledger["recentCharges"][0]["assumedChargeUsd"],"0.000040");
    }
    #[test]fn enforces_the_cap_before_subsequent_batches_and_reports_partial_coverage(){
        let f=Fixture::new(None);
        fs::write(f.dir.path().join("repo/zz-large.txt"),"x".repeat(40*1024)).unwrap();
        let mut body=f.input("complete");body["maxCostUsd"]=json!(0.01);
        f.run(body);let scan=&f.get()["scan"];
        assert_eq!(scan["status"],"partial","{scan}");
        let ledger=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();
        assert_eq!(ledger["recentCharges"][0]["resolution"]["requests"],1);
    }
    #[test]fn scrubs_other_provider_keys_even_when_found_in_source(){
        let f=Fixture::new(None);
        let secret="fake-private-key-of-second-configured-provider";
        f.ai.handle("ai.key.save",json!({"provider":"openrouter","key":secret}),&f.store).unwrap();
        fs::write(f.dir.path().join("repo/secret-in-code.rs"),format!("let k = \"{secret}\";")).unwrap();
        f.run(f.input("complete"));
        let scan=&f.get()["scan"];assert_eq!(scan["status"],"partial","{scan}");
        assert!(!scan.to_string().contains(secret));
        assert!(scan["result"]["coverage"]["limitations"].as_array().unwrap().iter().any(|v|v.as_str().unwrap().contains("ocultados")));
    }
    #[test]fn explicit_retry_preserves_previous_target_and_replaying_it_does_not_bill_twice(){
        let f=Fixture::new(None);
        let mut body=f.input("unverifiable");body["target"]=json!("src/test.txt");
        f.run(body);let first=f.get();
        assert_eq!(first["scan"]["status"],"failed");
        let mut retry=f.input("complete");retry["retryOf"]=first["scan"]["id"].clone();
        let mut broader=retry.clone();broader["target"]=json!("outside.txt");
        assert!(f.service.prepare(&f.store,&f.ai,decode(broader).unwrap()).is_err());
        f.run(retry.clone());let done=f.get();
        assert_eq!(done["scan"]["status"],"completed");assert_eq!(done["scan"]["selectedPaths"],json!(["src/test.txt"]));
        assert_eq!(f.run(retry),done);
        let ledger=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();
        assert_eq!(ledger["recentChargesTotal"],2);
    }
    #[test]fn empty_exit_cannot_confirm_and_unknown_charge_remains(){
        let f=Fixture::new(None);f.run(f.input("missing-terminal"));assert_eq!(f.get()["scan"]["status"],"failed");
        assert!(f.store.snapshot().unwrap().findings.is_empty());
        let ledger=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();assert_eq!(ledger["unknownChargesTotal"],1);
    }
    #[test]fn cancellation_waits_for_terminal_and_never_invents_results(){
        let f=Fixture::new(None);let p=f.input("wait");
        thread::scope(|t|{
            let(tx,rx)=mpsc::sync_channel(1);let f=&f;t.spawn(move||f.service.start_worker(&f.store,&f.ai,p,&tx).unwrap());rx.recv().unwrap().unwrap();
            let deadline=Instant::now()+Duration::from_secs(15);
            while f.get()["scan"]["stage"]!="review"{assert!(Instant::now()<deadline,"{}",f.get());thread::sleep(Duration::from_millis(25));}
            assert!(f.ai.handle("ai.key.remove",json!({}),&f.store).is_err());
            let pending=f.service.cancel(&f.store,json!({"assessmentId":f.aid})).unwrap();
            assert!(matches!(pending["scan"]["status"].as_str(),Some("cancelling"|"cancelled")));
        });assert_eq!(f.get()["scan"]["status"],"cancelled");assert!(f.store.snapshot().unwrap().findings.is_empty());
    }
    #[test]fn subpath_cannot_broaden_and_restart_never_replays(){
        let f=Fixture::new(Some("src"));let mut denied=f.input("complete");denied["target"]=json!("outside.txt");
        assert!(f.service.prepare(&f.store,&f.ai,decode(denied).unwrap()).is_err());assert!(f.get()["scan"].is_null());
        let body=f.input("wait");let (_,job)=f.service.prepare(&f.store,&f.ai,decode(body.clone()).unwrap()).unwrap();assert_eq!(job.as_ref().unwrap().scope.targets,vec!["src"]);
        drop(job);Service::recover(&f.store).unwrap();assert_eq!(f.get()["scan"]["status"],"interrupted");
        let again=Service::new(f.dir.path());let (_,job)=again.prepare(&f.store,&f.ai,decode(body).unwrap()).unwrap();assert!(job.is_none());
    }
    #[test]
    fn derived_scans_keep_source_paths_sensitivity_revocation_and_pinned_policy() {
        use crate::data_policy::{DataCategory,DataDestination,DataPolicyService,Sensitivity,test_support::TestRedactor};
        let f=Fixture::new(None);
        let mut first=f.input("complete");
        first["target"]=json!("src");
        f.run(first);
        f.ai.test_data_authority(&f.store,&f.aid);
        let base=f.store.snapshot().unwrap();
        let pid=&base.projects[0].id;
        let svc=DataPolicyService::new();
        let call=|method:&str,payload:Value|svc.handle(method,payload,&f.store,&TestRedactor,&crate::data_projection::RejectExternal,None).unwrap();
        let scope=scope::current_ref(&base,&f.aid).unwrap().unwrap();
        let subject=source_subject(&base,&scope).unwrap();
        let fingerprint=crate::data_projection::canonical_fingerprint(&base,&subject).unwrap().unwrap();
        let review=call("data.classification.prepare",json!({"subject":subject,"expectedSourceFingerprint":fingerprint,"sensitivity":"sensitive","reason":"Private source"}));
        call("data.classification.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeClassification":true}));
        let policy=base.data_policy_revisions.last().unwrap().body.clone();
        let update=|body:&crate::data_policy::DataPolicyBody|{
            let status=call("data.policy.status",json!({"projectId":pid}));
            let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":body}));
            call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));
        };
        let adopt=||{
            let status=call("data.policy.status",json!({"projectId":pid,"assessmentId":f.aid}));
            let r=&status["currentRevision"];
            let review=call("data.binding.prepare",json!({"assessmentId":f.aid,"policyRef":{"projectId":pid,"revisionId":r["id"],"digest":r["digest"]},"expectedStateDigest":status["stateDigest"]}));
            call("data.binding.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeProspectiveAdoption":true}));
        };
        const OBJECTIVE:&str="Derived private objective canary";
        let derive=|parent:&str|{
            let chat=f.store.handle(core::Request{action:"project.newConversation".into(),payload:json!({"projectId":pid,"sourceAssessmentId":parent,"analysisObjective":OBJECTIVE,"firstTurn":true,"title":"Synthetic analysis","idempotencyKey":model::id()})}).unwrap();
            let id=chat["id"].as_str().unwrap().to_owned();
            f.ai.test_data_authority(&f.store,&id);
            id
        };
        let child=derive(&f.aid);
        let grandchild=derive(&child);
        let state=f.store.snapshot().unwrap();
        let input=|target:Option<&str>,ack:bool|decode::<Start>(json!({"assessmentId":grandchild,"idempotencyKey":model::id(),"maxCostUsd":1.0,"target":target,"acknowledgeSensitive":ack})).unwrap();
        assert!(approved_scope(&state,&input(None,true)).is_err());
        assert!(approved_scope(&state,&input(Some("outside.txt"),true)).is_err());
        assert!(approved_scope(&state,&input(Some("src/test.txt"),true)).is_ok());
        assert_eq!(f.service.status(&f.store,&f.ai,json!({"assessmentId":grandchild})).unwrap()["requiresSensitiveConfirmation"],true);
        assert!(f.service.prepare(&f.store,&f.ai,input(Some("src"),false)).is_err());
        let mut revoked=state.clone();
        revoked.assessment_scopes.iter_mut().find(|b|b.assessment_id==f.aid).unwrap().revocation=Some(scope::ScopeRevocation{
            id:model::id(),at:model::now(),author:"local-user".into(),rationale:"Revoked source".into(),prevent_new_admissions:true,revision_id:scope.revision_id.clone()
        });
        assert!(check_data(&revoked,&grandchild,ProviderId::OpenAI).is_err());
        f.run(json!({"assessmentId":grandchild,"idempotencyKey":model::id(),"maxCostUsd":1.0,"target":"src","acknowledgeSensitive":true}));
        assert_eq!(f.service.get(&f.store,json!({"assessmentId":grandchild})).unwrap()["scan"]["status"],"completed");
        let mut restricted=policy.clone();
        restricted.local_raw_view.sensitivities.retain(|s|*s!=Sensitivity::Sensitive);
        restricted.destinations.iter_mut().find(|r|r.destination==DataDestination::RemoteAi).unwrap().categories.retain(|c|*c!=DataCategory::SourceExcerpt);
        update(&restricted);adopt();
        assert!(check_data(&f.store.snapshot().unwrap(),&grandchild,ProviderId::OpenAI).is_err());
        let view=crate::data_host::project_response("snapshot",json!({}),&f.store,&TestRedactor).unwrap();
        assert!(!view.to_string().contains(OBJECTIVE));
        let pinned=derive(&f.aid);
        update(&policy);adopt();
        assert!(check_data(&f.store.snapshot().unwrap(),&pinned,ProviderId::OpenAI).is_err());
    }

    #[test]fn existing_chat_policy_controls_project_source_and_revocation(){
        let f=Fixture::new(None);f.ai.test_data_authority(&f.store,&f.aid);
        let mut s=f.store.snapshot().unwrap();check_data(&s,&f.aid,ProviderId::OpenAI).unwrap();
        let objective=core::hash(s.assessments[0].objective.trim());
        assert!(check_disclosure(&s,&f.aid,ProviderId::OpenAI,&core::hash("other text"),true).is_err());
        let mut sensitive=s.clone();
        for c in &mut sensitive.data_classifications{c.sensitivity=crate::data_policy::Sensitivity::Sensitive;}
        assert!(check_disclosure(&sensitive,&f.aid,ProviderId::OpenAI,&objective,false).is_err());
        assert!(check_disclosure(&sensitive,&f.aid,ProviderId::OpenAI,&objective,true).is_ok());
        s.data_policies[0].active=false;assert!(check_data(&s,&f.aid,ProviderId::OpenAI).is_err());s.data_policies[0].active=true;
        let mut changed=s.clone();changed.data_bindings.clear();assert!(check_data(&changed,&f.aid,ProviderId::OpenAI).is_err());
        let mut changed=s.clone();for r in &mut changed.credential_references{r.revoked=true;}assert!(check_data(&changed,&f.aid,ProviderId::OpenAI).is_err());
        let remote=s.data_policy_revisions[0].body.destinations.iter_mut().find(|d|d.destination==crate::data_policy::DataDestination::RemoteAi).unwrap();
        remote.categories.retain(|c|*c!=crate::data_policy::DataCategory::SourceExcerpt);
        assert!(check_data(&s,&f.aid,ProviderId::OpenAI).is_err());
    }
}
