//! Reviewed local source derivatives. No provider call, executor, approval or
//! finding mutation exists here. Original bytes stay outside the evidence DB.
use crate::{ai::AiService,core::{self,Store},model::{self,Assessment,Evidence,ExecutionPlan,Snapshot},source_scan};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::HashSet,path::Path};

const SCHEMA:&str="openkoda.source-context.v1";
const ORIGIN:&str="host_selected_source_redacted_copy";
const NOTICE:&str="Cópia local redigida dos arquivos selecionados, verificados contra o manifest. A redação automática não identifica todos os segredos. Revise todo o texto. Nenhum conteúdo foi enviado à OpenAI; selecione depois esta evidência e autorize o contexto separadamente.";

#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct PreviewInput {assessment_id:String,plan_id:String,paths:Vec<String>}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CaptureInput {assessment_id:String,plan_id:String,paths:Vec<String>,digest:String,acknowledge_local_capture:bool}
#[derive(Clone,Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct SourceFile {path:String,bytes:u64,source_sha256:String,redacted_sha256:String,text:String,redaction_changed:bool}
#[derive(Clone,Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Source {assessment_id:String,plan_id:String,plan_digest:String,manifest_sha256:String,total_source_bytes:u64,files:Vec<SourceFile>}
#[derive(Deserialize,Serialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Document {schema:String,origin:String,capture_digest:String,source:Source}

pub fn supports(action:&str)->bool {matches!(action,"evidence.sourcePreview"|"evidence.sourceCapture")}
pub fn handle(action:&str,payload:Value,store:&Store,ai:&AiService)->Result<Value,String>{
    if !payload.is_object()||payload.to_string().len()>16384{return Err("Solicitação de captura local inválida.".into());}
    let (input,expected)=match action {
        "evidence.sourcePreview"=>(decode::<PreviewInput>(payload)?,None),
        "evidence.sourceCapture"=>{let input:CaptureInput=decode(payload)?;if !input.acknowledge_local_capture{return Err("Confirme a revisão e a gravação local desta cópia redigida.".into());}(PreviewInput{assessment_id:input.assessment_id,plan_id:input.plan_id,paths:input.paths},Some(input.digest))},
        _=>return Err("Ação de captura local desconhecida.".into()),
    };
    let snapshot=store.snapshot()?;let plan=reviewed_plan(&snapshot,&input)?;
    let expected_scope=plan.scope_ref.as_ref().ok_or("scope_legacy_plan_requires_reprepare: prepare novo manifesto após revisar o escopo.")?;
    let refs=plan.target_ref.iter().cloned().collect::<Vec<_>>();
    let scope=crate::scope::with_selected_admission(store,&input.assessment_id,"source.capture.v1",Some(expected_scope),&refs,action,|reference|Ok(reference.clone()))?;
    crate::scope::recheck_selected(store,&scope,"source.capture.v1",&refs)?;
    let _preflight=crate::local_scope::preflight(&store.snapshot()?,&scope,plan.target_ref.as_ref())?;
    ai.with_local_redactor(|redact|{
        // Exact provenance paths must remain usable. Do not return/store a
        // secret-looking path under a misleading redacted filename.
        if input.paths.iter().any(|path|redact(path)!=*path){return Err("Um caminho selecionado contém dado sensível; renomeie e prepare novo manifest antes da captura.".into());}
        crate::scope::recheck_selected(store,&scope,"source.capture.v1",&refs)?;
        source_scan::with_selected_sources(Path::new(&plan.target),&plan.input_files,plan.source_scope.as_ref().ok_or("Manifest ausente.")?,&input.paths,|files|{
            let source=Source{assessment_id:input.assessment_id.clone(),plan_id:plan.id.clone(),plan_digest:plan.digest.clone(),manifest_sha256:plan.source_scope.as_ref().unwrap().manifest_sha256.clone(),
                total_source_bytes:files.iter().map(|file|file.bytes).sum(),files:files.into_iter().map(|file|{let text=redact(&file.text);SourceFile{path:file.path,bytes:file.bytes,source_sha256:file.sha256,redacted_sha256:core::hash(&text),redaction_changed:text!=file.text,text}}).collect()};
            let digest=source_digest(&source)?;
            if let Some(expected)=&expected {
                if expected!=&digest{return Err("A prévia mudou ou não corresponde à seleção. Revise novamente antes de capturar; nenhum conteúdo foi gravado.".into());}
                let document=Document{schema:SCHEMA.into(),origin:ORIGIN.into(),capture_digest:digest.clone(),source};
                let content=serde_json::to_string(&document).map_err(error)?;let sha256=core::hash(&content);
                // Root, parent and selected-file handles remain locked through
                // this transaction, including the second domain-state check.
                store.mutate(|current|{
                    let current_plan=reviewed_plan(current,&input)?;
                    if current_plan.digest!=plan.digest{return Err("O plano mudou durante a captura; revise novamente.".into());}
                    let assessment=current.assessments.iter_mut().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
                    if let Some(existing)=assessment.evidence.iter().find(|e|e.sha256==sha256&&e.content==content){return serde_json::to_value(existing).map_err(error);}
                    if assessment.evidence.len()>=1000{return Err("Limite de evidências desta avaliação atingido.".into());}
                    let evidence=Evidence{id:model::id(),name:format!("Fontes selecionadas · {} arquivo(s)",document.source.files.len()),content,sha256,created_at:model::now()};
                    assessment.evidence.push(evidence.clone());assessment.updated_at=model::now();
                    assessment.events.push(model::event(format!("Captura local redigida {} vinculada ao plano {}: {} arquivo(s), {} bytes de origem. Nenhum envio à IA ou candidato criado.",evidence.id,plan.id,document.source.files.len(),document.source.total_source_bytes),"info"));
                    serde_json::to_value(evidence).map_err(error)
                })
            }else{
                // Preview must also not return a now-stale domain binding.
                let current_plan=reviewed_plan(&store.snapshot()?,&input)?;if current_plan.digest!=plan.digest{return Err("O plano mudou durante a leitura da prévia.".into());}
                Ok(json!({"assessmentId":source.assessment_id,"planId":source.plan_id,"planDigest":source.plan_digest,"manifestSha256":source.manifest_sha256,"paths":source.files.iter().map(|file|file.path.as_str()).collect::<Vec<_>>(),"files":source.files,"totalSourceBytes":source.total_source_bytes,"digest":digest,"sourceFilesIncluded":true,"sentToProvider":false,"notice":NOTICE}))
            }
        })
    })
}

fn reviewed_plan(snapshot:&Snapshot,input:&PreviewInput)->Result<ExecutionPlan,String>{
    core::valid_id(&input.assessment_id)?;core::valid_id(&input.plan_id)?;
    if input.paths.is_empty()||input.paths.len()>8||input.paths.iter().collect::<HashSet<_>>().len()!=input.paths.len(){return Err("Escolha de 1 a 8 caminhos distintos do plano.".into());}
    let assessment=snapshot.assessments.iter().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
    let project=snapshot.projects.iter().find(|p|p.id==assessment.project_id).ok_or("Projeto não encontrado.")?;
    if project.archived||core::active(&assessment.status){return Err("Captura exige projeto ativo e avaliação sem operação em andamento ou desconhecida.".into());}
    if snapshot.plans.iter().any(|p|p.assessment_id==assessment.id&&matches!(p.status.as_str(),"approved"|"running"|"canceling"|"unknown")){return Err("Revogue ou encerre o plano autorizado/ativo antes de capturar fontes.".into());}
    let plan=snapshot.plans.iter().find(|p|p.id==input.plan_id&&p.assessment_id==assessment.id).ok_or("Plano não pertence a esta avaliação.")?;
    let selected=crate::executor::plan_target(snapshot,plan)?.ok_or("Prepare novo manifesto após revisar o escopo.")?;
    let scope=plan.scope_ref.as_ref().ok_or("Manifesto sem revisão de escopo.")?;
    crate::scope::check_selected_work(snapshot,&assessment.id,"source.capture.v1",Some(scope),&plan.target_ref.iter().cloned().collect::<Vec<_>>())?;
    if !matches!(selected.kind.as_str(),"code"|"files")||plan.adapter!=source_scan::ADAPTER||plan.digest!=crate::executor::plan_digest(plan)||!matches!(plan.status.as_str(),"proposed"|"completed"|"failed"|"canceled"|"revoked"){return Err("Plano estático inválido, alterado ou em estado incompatível com captura.".into());}
    source_scan::validate(Path::new(&plan.target),&plan.input_files,plan.source_scope.as_ref().ok_or("Plano estático sem manifest.")?)?;
    source_scan::validate_selected_paths(&plan.input_files,&selected.local_exclusions)?;
    for path in &input.paths{if !plan.input_files.iter().any(|file|&file.path==path){return Err("Seleção contém caminho fora do manifest; nenhum novo arquivo será descoberto.".into());}}
    Ok(plan.clone())
}
fn source_digest(source:&Source)->Result<String,String>{Ok(core::hash(&serde_json::to_string(&json!({"schema":SCHEMA,"origin":ORIGIN,"source":source})).map_err(error)?))}
fn decode<T:serde::de::DeserializeOwned>(value:Value)->Result<T,String>{serde_json::from_value(value).map_err(|_|"Campos inválidos na captura local.".into())}
fn error(_:impl std::fmt::Display)->String{"Falha ao serializar a captura local; dados existentes preservados.".into()}

/// Informational only. A manual-evidence wrapper or a bare marker is not a
/// host source capture. This metadata never authorizes a filesystem read.
pub(crate) fn is_source_capture(evidence:&Evidence,assessment:&Assessment,plans:&[ExecutionPlan])->bool{
    if evidence.content.len()>256*1024{return false;}
    let Ok(document)=serde_json::from_str::<Document>(&evidence.content)else{return false;};
    let source=&document.source;
    if document.schema!=SCHEMA||document.origin!=ORIGIN||source.assessment_id!=assessment.id||evidence.sha256!=core::hash(&evidence.content)||source.files.is_empty()||source.files.len()>8||source.total_source_bytes>32768||source.files.iter().try_fold(0u64,|sum,f|sum.checked_add(f.bytes))!=Some(source.total_source_bytes)||source_digest(source).ok().as_ref()!=Some(&document.capture_digest){return false;}
    let Some(plan)=plans.iter().find(|p|p.id==source.plan_id&&p.assessment_id==assessment.id&&(p.target_ref.is_some()||p.target==assessment.target)&&p.adapter==source_scan::ADAPTER&&p.digest==source.plan_digest&&p.digest==crate::executor::plan_digest(p))else{return false;};
    if plan.source_scope.as_ref().is_none_or(|scope|scope.manifest_sha256!=source.manifest_sha256){return false;}
    let mut paths=HashSet::new();source.files.iter().all(|file|paths.insert(&file.path)&&file.redacted_sha256==core::hash(&file.text)&&!file.text.contains('\0')&&plan.input_files.iter().any(|input|input.path==file.path&&input.size==file.bytes&&input.sha256==file.source_sha256))
}

#[cfg(all(test,windows))]
mod tests {
    #[test]
    fn combined_capture_is_bound_to_nonprimary_entry_and_excludes_locked_paths(){
        use std::os::windows::fs::OpenOptionsExt;
        let dir=tempfile::tempdir().unwrap();let first=dir.path().join("first");let second=dir.path().join("second");std::fs::create_dir(&first).unwrap();std::fs::create_dir(&second).unwrap();std::fs::write(first.join("file.txt"),b"PRIMARY-SENTINEL").unwrap();std::fs::write(second.join("file.txt"),b"selected second text").unwrap();std::fs::write(second.join("secret.txt"),b"EXCLUDED-SENTINEL").unwrap();
        let _locked=std::fs::OpenOptions::new().read(true).share_mode(0).open(second.join("secret.txt")).unwrap();let methods=["source.rules.v1","source.capture.v1"];
        let f=crate::executor::combined_fixture::Fixture::new(&[("code",first.to_str().unwrap(),&methods),("code",second.to_str().unwrap(),&methods)],&[(1,"secret.txt","exact")],serde_json::json!([]));let plan=f.plan(1,"source.rules.v1");let ai=crate::ai::AiService::new(f.store.root.clone()).unwrap();
        let input=serde_json::json!({"assessmentId":f.aid,"planId":plan["id"],"paths":["file.txt"]});let preview=super::handle("evidence.sourcePreview",input.clone(),&f.store,&ai).unwrap();assert_eq!(preview["files"][0]["text"],"selected second text");assert!(!preview.to_string().contains("PRIMARY-SENTINEL"));
        let mut denied=input.clone();denied["paths"]=serde_json::json!(["secret.txt"]);assert!(super::handle("evidence.sourcePreview",denied,&f.store,&ai).is_err());let mut capture=input;capture["digest"]=preview["digest"].clone();capture["acknowledgeLocalCapture"]=serde_json::json!(true);let evidence=super::handle("evidence.sourceCapture",capture,&f.store,&ai).unwrap();
        let snapshot=f.store.snapshot().unwrap();let refs=crate::evidence_lineage::evidence_refs(&f.store,&snapshot,&f.aid,&[evidence["id"].as_str().unwrap().into()]).unwrap();assert_eq!(refs,vec![f.refs[1].clone()]);assert!(!evidence.to_string().contains("EXCLUDED-SENTINEL"));assert!(snapshot.executions.is_empty());
        let mut forged=snapshot.clone();forged.plans[0].target_ref=Some(f.refs[0].clone());forged.plans[0].digest=crate::executor::plan_digest(&forged.plans[0]);assert!(crate::executor::validate_state(&forged).is_err());
    }
    use super::*;
    use crate::{core::Request,executor::Executor};
    use std::{fs,path::PathBuf};
    struct Fixture {_dir:tempfile::TempDir,target:PathBuf,store:Store,ai:AiService,assessment:String,plan:Value}
    impl Fixture {
        fn new(files:&[(&str,&[u8])],file_target:bool)->Self{Self::with_methods(files,file_target,&["source.rules.v1","source.capture.v1"])}
        fn with_methods(files:&[(&str,&[u8])],file_target:bool,methods:&[&str])->Self{
            let dir=tempfile::tempdir().unwrap();let target=dir.path().join("source");fs::create_dir(&target).unwrap();
            for(path,bytes)in files{let path=target.join(path);fs::create_dir_all(path.parent().unwrap()).unwrap();fs::write(path,bytes).unwrap();}
            let target=if file_target{target.join(files[0].0)}else{target};let root=dir.path().join("profile");let store=Store::open(root.clone()).unwrap();let ai=AiService::new(root).unwrap();
            let project=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Source capture fixture","kind":"code","target":target})}).unwrap();
            let assessment=store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"title":"Capture fixture","kind":"code","target":target,"objective":"Review only selected source","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),methods)})}).unwrap()["id"].as_str().unwrap().to_string();
            let plan=Executor::default().handle("assessment.sourcePlan",json!({"id":assessment}),&store).unwrap();Self{_dir:dir,target,store,ai,assessment,plan}
        }
        fn input(&self,paths:&[&str])->Value{json!({"assessmentId":self.assessment,"planId":self.plan["id"],"paths":paths})}
        fn preview(&self,paths:&[&str])->Result<Value,String>{handle("evidence.sourcePreview",self.input(paths),&self.store,&self.ai)}
        fn capture(&self,paths:&[&str],digest:&Value)->Result<Value,String>{let mut input=self.input(paths);input["digest"]=digest.clone();input["acknowledgeLocalCapture"]=json!(true);handle("evidence.sourceCapture",input,&self.store,&self.ai)}
        fn history(&self)->Value{self.ai.handle("ai.history",json!({"assessmentId":self.assessment}),&self.store).unwrap()}
    }
    #[test]
    fn method_omission_and_revocation_deny_before_selected_file_read(){
        for revoked in [false,true]{let methods=if revoked{vec!["source.rules.v1","source.capture.v1"]}else{vec!["source.rules.v1"]};let f=Fixture::with_methods(&[("chosen.rs",b"fn owned() {}")],false,&methods);
            if revoked{let status=crate::scope::status(&f.store,&f.assessment).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":f.assessment,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"rationale":"End capture scope","acknowledgeRevocation":true}),&f.store,str::to_owned).unwrap();}
            fs::remove_file(f.target.join("chosen.rs")).unwrap();let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();assert!(f.preview(&["chosen.rs"]).unwrap_err().contains(if revoked{"scope_revoked"}else{"scope_method_not_selected"}));assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert!(f.history()["requests"].as_array().unwrap().is_empty());}
    }
    #[test]
    fn preview_reads_selected_only_is_readonly_and_never_contacts_provider(){
        let f=Fixture::new(&[("chosen.rs",b"fn selected() {}\n"),("private.rs",b"UNSELECTED-SENTINEL")],false);let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
        fs::remove_file(f.target.join("private.rs")).unwrap();let preview=f.preview(&["chosen.rs"]).unwrap();
        assert_eq!(preview["files"].as_array().unwrap().len(),1);assert!(preview["files"][0]["text"].as_str().unwrap().contains("selected"));assert!(!preview.to_string().contains("UNSELECTED"));assert_eq!(preview["sentToProvider"],false);assert_eq!(preview["totalSourceBytes"],17);assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);assert!(f.history()["requests"].as_array().unwrap().is_empty());
    }
    #[test]
    fn capture_is_exact_idempotent_and_preserves_provenance_without_execution_or_findings(){
        let f=Fixture::new(&[("a.rs",b"fn a() {}"),("b.rs",b"fn b() {}")],false);let before=f.store.snapshot().unwrap();let preview=f.preview(&["b.rs","a.rs"]).unwrap();assert_eq!(preview,f.preview(&["a.rs","b.rs"]).unwrap());
        let first=f.capture(&["b.rs","a.rs"],&preview["digest"]).unwrap();let duplicate=f.capture(&["a.rs","b.rs"],&preview["digest"]).unwrap();assert_eq!(first,duplicate);let document:Document=serde_json::from_str(first["content"].as_str().unwrap()).unwrap();assert_eq!(document.capture_digest,preview["digest"]);assert_eq!(document.source.files[0].source_sha256,core::hash("fn a() {}"));assert_eq!(first["sha256"],core::hash(first["content"].as_str().unwrap()));
        let after=f.store.snapshot().unwrap();assert_eq!(after.assessments[0].evidence.len(),1);assert_eq!(after.assessments[0].status,before.assessments[0].status);assert_eq!(serde_json::to_value(&after.plans).unwrap(),serde_json::to_value(&before.plans).unwrap());assert!(after.findings.is_empty()&&after.executions.is_empty());assert!(f.history()["requests"].as_array().unwrap().is_empty());assert_eq!(fs::read(f.target.join("a.rs")).unwrap(),b"fn a() {}");
        let context=f.ai.test_context_projection(&f.store,json!({"assessmentId":f.assessment,"evidenceIds":[first["id"]]})).unwrap();assert_eq!(context["sourceFilesIncluded"],true);assert_eq!(f.ai.test_context_projection(&f.store,json!({"assessmentId":f.assessment,"evidenceIds":[]})).unwrap()["sourceFilesIncluded"],false);
    }
    #[test]
    fn stale_bytes_and_wrong_preview_digest_cannot_be_captured(){
        let f=Fixture::new(&[("code.rs",b"before")],false);let preview=f.preview(&["code.rs"]).unwrap();assert!(f.capture(&["code.rs"],&json!("wrong")).is_err());fs::write(f.target.join("code.rs"),b"after!").unwrap();assert!(f.capture(&["code.rs"],&preview["digest"]).unwrap_err().contains("Conteúdo diferente"));assert!(f.store.snapshot().unwrap().assessments[0].evidence.is_empty());
    }
    #[test]
    fn identical_replacement_file_and_replaced_root_are_refused(){
        let f=Fixture::new(&[("code.rs",b"identical")],false);fs::rename(f.target.join("code.rs"),f._dir.path().join("original.rs")).unwrap();fs::write(f.target.join("code.rs"),b"identical").unwrap();assert!(f.preview(&["code.rs"]).unwrap_err().contains("Identidade"));
        let f=Fixture::new(&[("code.rs",b"identical")],false);fs::rename(&f.target,f._dir.path().join("original-source")).unwrap();fs::create_dir(&f.target).unwrap();fs::write(f.target.join("code.rs"),b"identical").unwrap();assert!(f.preview(&["code.rs"]).unwrap_err().contains("alvo foi substituído"));
    }
    #[test]
    fn new_paths_cross_assessment_duplicates_extra_fields_and_missing_ack_are_refused(){
        let f=Fixture::new(&[("code.rs",b"source")],false);fs::write(f.target.join("new.rs"),b"not planned").unwrap();
        for paths in [vec!["new.rs"],vec!["../code.rs"],vec!["code.rs","code.rs"],vec![]]{assert!(f.preview(&paths).is_err());}
        let mut cross=f.input(&["code.rs"]);cross["assessmentId"]=json!(model::id());assert!(handle("evidence.sourcePreview",cross,&f.store,&f.ai).is_err());
        let mut extra=f.input(&["code.rs"]);extra["target"]=json!(f.target);assert!(handle("evidence.sourcePreview",extra,&f.store,&f.ai).is_err());
        let mut capture=f.input(&["code.rs"]);capture["digest"]=f.preview(&["code.rs"]).unwrap()["digest"].clone();capture["acknowledgeLocalCapture"]=json!(false);assert!(handle("evidence.sourceCapture",capture,&f.store,&f.ai).is_err());assert!(f.store.snapshot().unwrap().assessments[0].evidence.is_empty());
    }
    #[test]
    fn quotas_binary_and_nul_are_refused_without_truncation(){
        let large=vec![b'x';32769];let f=Fixture::new(&[("large.txt",&large)],false);assert!(f.preview(&["large.txt"]).unwrap_err().contains("32 KiB"));
        let part=vec![b'x';17000];let f=Fixture::new(&[("a.txt",&part),("b.txt",&part)],false);assert!(f.preview(&["a.txt","b.txt"]).is_err());
        for bytes in [&[0xff,0xfe][..],&b"a\0b"[..]]{let f=Fixture::new(&[("binary",bytes)],false);assert!(f.preview(&["binary"]).is_err());}
        let files=(0..9).map(|i|(format!("{i}.txt"),b"x".as_slice())).collect::<Vec<_>>();let refs=files.iter().map(|(p,b)|(p.as_str(),*b)).collect::<Vec<_>>();let f=Fixture::new(&refs,false);let paths=refs.iter().map(|(p,_)|*p).collect::<Vec<_>>();assert!(f.preview(&paths).is_err());
    }
    #[test]
    fn hardlinks_alternate_streams_and_junction_substitution_are_refused(){
        let f=Fixture::new(&[("code.rs",b"source")],false);fs::hard_link(f.target.join("code.rs"),f._dir.path().join("alias.rs")).unwrap();assert!(f.preview(&["code.rs"]).is_err());fs::remove_file(f._dir.path().join("alias.rs")).unwrap();fs::write(format!("{}:private",f.target.join("code.rs").display()),b"STREAM").unwrap();assert!(f.preview(&["code.rs"]).is_err());
        use std::os::windows::process::CommandExt;
        let f=Fixture::new(&[("child/code.rs",b"source")],false);let outside=f._dir.path().join("outside");fs::create_dir(&outside).unwrap();fs::write(outside.join("code.rs"),b"OUTSIDE-SENTINEL").unwrap();fs::rename(f.target.join("child"),f._dir.path().join("old-child")).unwrap();let linked=f.target.join("child");let output=std::process::Command::new("cmd.exe").args(["/C","mklink","/J"]).arg(&linked).arg(&outside).creation_flags(0x08000000).output().unwrap();assert!(output.status.success());assert!(f.preview(&["child/code.rs"]).is_err());fs::remove_dir(&linked).unwrap();assert_eq!(fs::read(outside.join("code.rs")).unwrap(),b"OUTSIDE-SENTINEL");
    }
    #[test]
    fn current_key_and_known_secret_labels_are_redacted_before_preview_and_storage(){
        const KEY:&str="synthetic-source-context-key-not-real-123456";let source=format!("const harmless = '{KEY}';\npassword = 'PRIVATE-SENTINEL'\nlet useful = 7;\n");let f=Fixture::new(&[("source.rs",source.as_bytes())],false);f.ai.handle("ai.key.save",json!({"key":KEY}),&f.store).unwrap();let preview=f.preview(&["source.rs"]).unwrap();assert!(!preview.to_string().contains(KEY));assert!(!preview.to_string().contains("PRIVATE-SENTINEL"));assert!(preview["files"][0]["text"].as_str().unwrap().contains("useful"));let evidence=f.capture(&["source.rs"],&preview["digest"]).unwrap();assert!(!evidence.to_string().contains(KEY));assert!(!evidence.to_string().contains("PRIVATE-SENTINEL"));assert_eq!(fs::read(f.target.join("source.rs")).unwrap(),source.as_bytes());assert_eq!(preview["files"][0]["sourceSha256"],core::hash(&source));assert_eq!(preview["files"][0]["redactionChanged"],true);
    }
    #[test]
    fn active_unknown_archived_and_approved_states_block_capture(){
        for status in ["running","unknown","planning","validating"]{let f=Fixture::new(&[("source.rs",b"source")],false);f.store.mutate(|s|{s.assessments[0].status=status.into();Ok(())}).unwrap();assert!(f.preview(&["source.rs"]).is_err());}
        let f=Fixture::new(&[("source.rs",b"source")],false);f.store.mutate(|s|{s.projects[0].archived=true;Ok(())}).unwrap();assert!(f.preview(&["source.rs"]).is_err());
        let f=Fixture::new(&[("source.rs",b"source")],false);let preview=f.preview(&["source.rs"]).unwrap();Executor::default().handle("plan.approve",json!({"id":f.plan["id"],"digest":f.plan["digest"]}),&f.store).unwrap();assert!(f.capture(&["source.rs"],&preview["digest"]).is_err());
    }
    #[test]
    fn file_target_and_read_locks_hold_through_persistence_callback(){
        let f=Fixture::new(&[("source.rs",b"source")],true);assert_eq!(f.preview(&["source.rs"]).unwrap()["files"][0]["text"],"source");let plan:ExecutionPlan=serde_json::from_value(f.plan.clone()).unwrap();
        source_scan::with_selected_sources(&f.target,&plan.input_files,plan.source_scope.as_ref().unwrap(),&["source.rs".into()],|_|{assert!(fs::write(&f.target,b"modified").is_err());assert!(fs::rename(&f.target,f._dir.path().join("renamed.rs")).is_err());Ok(())}).unwrap();fs::write(&f.target,b"modified").unwrap();
    }
    #[test]
    fn manual_wrapped_or_tampered_metadata_does_not_claim_host_source_capture(){
        let f=Fixture::new(&[("source.rs",b"source")],false);let preview=f.preview(&["source.rs"]).unwrap();let captured=f.capture(&["source.rs"],&preview["digest"]).unwrap();let mut evidence:Evidence=serde_json::from_value(captured).unwrap();let snapshot=f.store.snapshot().unwrap();assert!(is_source_capture(&evidence,&snapshot.assessments[0],&snapshot.plans));
        let original=evidence.content.clone();evidence.content=json!({"schema":"openkoda.manual-evidence.v1","text":original}).to_string();evidence.sha256=core::hash(&evidence.content);assert!(!is_source_capture(&evidence,&snapshot.assessments[0],&snapshot.plans));
        let mut altered:Value=serde_json::from_str(&original).unwrap();altered["source"]["files"][0]["text"]=json!("different");evidence.content=altered.to_string();evidence.sha256=core::hash(&evidence.content);assert!(!is_source_capture(&evidence,&snapshot.assessments[0],&snapshot.plans));
    }
}
