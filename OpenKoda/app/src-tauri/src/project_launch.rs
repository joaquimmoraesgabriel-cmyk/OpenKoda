//! A project and the first scoped conversation are committed together. Replaying
//! the creation returns the same conversation without starting a second scan.
use super::{self as scope, ScopeWindow};
use crate::ai_provider::AiSelection;
use crate::core::{self, Store};
use crate::model::{self, Assessment, Project, Snapshot};
use crate::scope_targets::{self as targets, ProjectTarget, TargetSelection, TargetSetInput};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

const ACTION: &str = "project.launch";
const NEW_CHAT: &str = "project.newConversation";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewConversation {
    project_id: String,
    source_assessment_id: Option<String>,
    title: String,
    idempotency_key: String,
}

fn chat_methods(methods: &[String]) -> Vec<String> {
    methods.iter().filter(|m| matches!(m.as_str(),
        "ai.remote-review.v1" | "source.rules.v1" | "local.inventory.v1" |
        "http.headers.v1" | "network.tcp-connect.v1")).cloned().collect()
}

pub(crate) fn new_conversation(store: &Store, payload: Value) -> Result<Value,String> {
    let input:NewConversation = serde_json::from_value(payload).map_err(|_|"Solicitação de conversa inválida.")?;
    core::valid_id(&input.project_id)?;
    core::valid_id(&input.idempotency_key)?;
    if let Some(id)=&input.source_assessment_id {core::valid_id(id)?;}
    let aid=targets::derived_id("openkoda.project-conversation.v1",&[&input.project_id,&input.idempotency_key]);
    let digest=scope::hash(&input);
    let title=core::text(&input.title,"Título",160,true)?;
    store.mutate(|s|{
        if scope::replay(s,NEW_CHAT,&aid,&input.idempotency_key,&digest)?.is_some() {
            return Ok(json!(scope::assessment(s,&aid)?));
        }
        let project=s.projects.iter().find(|p|p.id==input.project_id&&!p.archived).ok_or("Projeto indisponível.")?;
        let preferences=crate::project_options::current(project);preferences.validate()?;
        let objective=preferences.objective("Revisar este projeto em conversa.")?;
        let title=if preferences.advanced_enabled&&!preferences.title.trim().is_empty(){preferences.title.trim().to_string()}else{title};
        let source=match &input.source_assessment_id {
            Some(id)=>Some(s.assessments.iter().find(|a|a.id==*id&&a.project_id==project.id)
                .ok_or("A conversa de origem não pertence a este projeto.")?),
            None=>s.assessments.iter().filter(|a|a.project_id==project.id).max_by_key(|a|&a.updated_at),
        };
        let original=if let Some(a)=source {
            if project.analysis_options.is_some()&&a.scope_id.is_some(){scope::check_new_local_input(s,&a.id)?;}
            match scope::binding(s,a)? {
                Some(binding) if binding.revocation.is_some()=>return Err(scope::err("scope_revoked")),
                Some(binding)=>match scope::current(s,binding)? {
                    Some(revision) if revision.review.is_none()=>return Err(scope::err("scope_unreviewed")),
                    revision=>revision,
                },
                None=>None,
            }
        } else {None};
        let ai=source.and_then(|a|a.ai_selection.clone());
        let window=if project.analysis_options.is_some(){ScopeWindow{starts_at:None,ends_at:preferences.deadline()}}else{original.map(|r|r.body.window.clone()).unwrap_or(ScopeWindow{starts_at:None,ends_at:None})};
        let result=if let Some(set)=original.and_then(|r|r.body.target_set.as_ref()) {
            let mut selection=targets::selection_from_set(set);
            selection.common_methods.clear();
            for entry in &mut selection.entries {
                entry.methods=chat_methods(&entry.methods);
                if entry.methods.is_empty(){return Err("A entrada selecionada exige uma nova revisão de escopo.".into());}
            }
            let body=scope::combined_body(project,&aid,&objective,&window,&selection)?;
            scope::insert_combined(s,&aid,&input.project_id,&title,body,None,ai)?
        }else{
            if source.is_some_and(|a|a.kind!=project.kind||!same_target(&project.kind,&a.target,&project.target)){
                return Err("O alvo da conversa precisa de revisão.".into());
            }
            let methods=original.map(|r|chat_methods(&r.body.methods)).unwrap_or_else(||vec!["ai.remote-review.v1".into()]);
            let creation=scope::CreationScope{methods,window,project_digest:scope::project_digest(project),
                catalog_version:scope::CATALOG_VERSION.into(),acknowledge_scope:true};
            let mut a=Assessment{id:aid.clone(),project_id:project.id.clone(),title,kind:project.kind.clone(),target:project.target.clone(),
                objective:objective.clone(),status:"draft".into(),created_at:model::now(),updated_at:model::now(),events:vec![],
                evidence:vec![],coverage:vec![],summary:String::new(),ai_selection:ai,scope_id:None};
            scope::initialize(s,&mut a,Some(&creation))?;
            s.assessments.push(a);
            scope::scope_result(s,&aid)?
        };
        let created=s.assessments.iter_mut().find(|a|a.id==aid).ok_or("Conversa indisponível.")?;
        created.events=vec![model::event("Conversa criada. Toda nova verificação exige plano próprio.","info")];
        let response=json!(created);
        scope::receipt(s,NEW_CHAT,&aid,&input.idempotency_key,&digest,result);
        Ok(response)
    })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Launch {
    idempotency_key: String,
    project_id: Option<String>,
    name: String,
    kind: String,
    root: String,
    relative_target: Option<String>,
    title: String,
    objective: String,
    ai_selection: Option<AiSelection>,
    ports: Vec<u16>,
    use_ripgrep: bool,
    minutes: Option<u16>,
    analysis_options: Option<crate::project_options::AnalysisOptions>,
    github_import_id: Option<String>,
}

fn same_target(kind: &str, left: &str, right: &str) -> bool {
    if cfg!(windows) && matches!(kind, "code" | "files") {
        let normalize = |s: &str| s.trim_end_matches(['/', '\\']).replace('/', "\\").to_uppercase();
        normalize(left) == normalize(right)
    } else {
        left == right
    }
}

fn selected_entry(project: &mut Project, relative: &str) -> Result<String, String> {
    let relative = targets::normalize_relative(relative)?;
    let target = Path::new(&project.target).join(&relative);
    let target = core::validate_target(&project.kind, &target.to_string_lossy())?;
    if let Some(existing) = project.additional_targets.iter().find(|entry| same_target(&entry.kind, &entry.target, &target)) {
        if existing.inactive { return Err("O alvo escolhido está inativo neste projeto.".into()); }
        return Ok(existing.id.clone());
    }
    if project.additional_targets.len() >= 63 { return Err("Limite de alvos do projeto atingido.".into()); }
    let id = model::id();
    let label = relative.rsplit('/').next().unwrap_or(&relative).chars().take(200).collect();
    project.additional_targets.push(ProjectTarget {
        id: id.clone(), label, target, kind: project.kind.clone(), created_at: model::now(), inactive: false,
    });
    Ok(id)
}

fn response(state: &Snapshot, aid: &str, replayed: bool) -> Result<Value, String> {
    let assessment = scope::assessment(state, aid)?;
    let reference = scope::current_ref(state, aid)?;
    let entry = reference.as_ref()
        .and_then(|scope| state.scope_revisions.iter().find(|r| r.id == scope.revision_id))
        .and_then(|revision| revision.body.target_set.as_ref())
        .and_then(|set| set.entries.iter().find(|entry| entry.entry_id == set.primary_entry_id))
        .map(targets::entry_ref);
    Ok(json!({"projectId":assessment.project_id, "assessmentId":aid,
              "scopeRef":reference, "targetRef":entry, "replayed":replayed}))
}

pub(crate) fn create(store: &Store, payload: Value) -> Result<Value, String> {
    let input: Launch = serde_json::from_value(payload).map_err(|_| "Formulário de projeto inválido.")?;
    core::valid_id(&input.idempotency_key)?;
    if let Some(id) = &input.project_id { core::valid_id(id)?; }
    let aid = targets::derived_id("openkoda.project-launch.v1", &[&input.idempotency_key]);
    let digest = scope::hash(&input);
    let name = core::text(&input.name, "Nome", 120, true)?;
    let title = core::text(&input.title, "Título", 160, true)?;
    let objective = core::text(&input.objective, "Descrição", 4000, true)?;
    let root = core::validate_target(&input.kind, &input.root)?;
    if let Some(options)=&input.analysis_options{options.validate()?;}
    let github_source=match &input.github_import_id{
        Some(id)=>{if input.kind!="code"{return Err("A importação GitHub exige um projeto de código.".into());}Some(crate::github_repository::source(&store.root,id,&root)?)},None=>None,
    };
    let local = matches!(input.kind.as_str(), "code" | "files");
    if !local && (input.relative_target.is_some() || input.use_ripgrep) {
        return Err("A seleção de arquivos exige análise local.".into());
    }
    if let Some(selection) = &input.ai_selection { selection.validate()?; }
    let mut methods: Vec<String> = match input.kind.as_str() {
        "code" | "files" => vec!["source.rules.v1".into(), "source.capture.v1".into(), "local.inventory.v1".into()],
        "web" => vec!["http.headers.v1".into()],
        "network" => {
            crate::network::plan_addresses(&root, &input.ports)?;
            vec![crate::network::ADAPTER.into()]
        }
        _ => return Err("Modalidade inválida.".into()),
    };
    if input.kind != "network" && !input.ports.is_empty() { return Err("Portas exigem análise de rede.".into()); }
    if input.use_ripgrep { methods.push("kali.rg.signals.v1".into()); }
    methods.push("ai.remote-review.v1".into());
    if input.minutes.is_some_and(|v| !(1..=1440).contains(&v)) {
        return Err("O limite da análise deve estar entre 1 e 1440 minutos.".into());
    }
    store.mutate(|state| {
        if scope::replay(state, ACTION, &aid, &input.idempotency_key, &digest)?.is_some() {
            return response(state, &aid, true);
        }
        let existing = if let Some(id) = &input.project_id {
            Some(state.projects.iter().position(|p| p.id == *id && !p.archived && p.kind == input.kind && same_target(&p.kind,&p.target,&root))
                .ok_or("O projeto selecionado foi alterado ou arquivado.")?)
        } else {
            state.projects.iter().position(|p| !p.archived && p.kind == input.kind && same_target(&p.kind,&p.target,&root))
        };
        let index = match existing {
            Some(index) => index,
            None => {
                if state.projects.len() >= 10000 { return Err("Limite de projetos atingido.".into()); }
                state.projects.push(Project {
                    id:model::id(), name,kind:input.kind.clone(),target:root,description:objective.clone(),
                    created_at:model::now(),archived:false,additional_targets:vec![],target_receipts:vec![],analysis_options:input.analysis_options.clone(),github_source:github_source.clone(),
                });
                state.projects.len()-1
            }
        };
        let project = &mut state.projects[index];
        if let Some(options)=&input.analysis_options{project.analysis_options=Some(options.clone());}
        if let Some(source)=&github_source{
            if project.github_source.as_ref().is_some_and(|old|old.import_id!=source.import_id){return Err("O projeto já está ligado a outra importação GitHub.".into());}
            project.github_source=Some(source.clone());
        }
        let pid = project.id.clone();
        let window = ScopeWindow {starts_at:None, ends_at: input.minutes.map(|n| (chrono::Utc::now()+chrono::Duration::minutes(i64::from(n))).to_rfc3339())};
        let result = if let Some(relative) = input.relative_target.as_deref() {
            let entry = selected_entry(project, relative)?;
            let selection = TargetSetInput {
                primary_project_target_id:entry.clone(),entries:vec![TargetSelection{project_target_id:entry,methods}],
                common_methods:vec![],excluded_project_targets:vec![],local_exclusions:vec![],connection_exclusions:vec![],
            };
            let body = scope::combined_body(project, &aid, &objective, &window, &selection)?;
            scope::insert_combined(state,&aid,&pid,&title,body,None,input.ai_selection)?
        } else {
            let reviewed = scope::CreationScope {methods,window,project_digest:scope::project_digest(project),catalog_version:scope::CATALOG_VERSION.into(),acknowledge_scope:true};
            let mut assessment = Assessment {
                id:aid.clone(),project_id:pid,title,kind:project.kind.clone(),target:project.target.clone(),objective,
                status:"draft".into(),created_at:model::now(),updated_at:model::now(),events:vec![model::event("Análise criada com o alvo e os métodos escolhidos.","info")],
                evidence:vec![],coverage:vec![],summary:String::new(),scope_id:None,ai_selection:input.ai_selection,
            };
            scope::initialize(state,&mut assessment,Some(&reviewed))?;
            state.assessments.push(assessment);
            scope::scope_result(state,&aid)?
        };
        scope::receipt(state,ACTION,&aid,&input.idempotency_key,&digest,result);
        response(state,&aid,false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: &str, root: &str) -> Value {
        json!({"idempotencyKey":model::id(),"name":"Laboratório","kind":kind,"root":root,
            "title":"Primeira revisão","objective":"Procurar riscos no alvo autorizado.",
            "ports":[],"useRipgrep":false})
    }

    #[test]
    fn retry_and_later_conversation_share_project_not_assessment() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path().join("db")).unwrap();
        let mut body = input("web","https://example.test");
        let first = create(&store,body.clone()).unwrap();
        let replay = create(&store,body.clone()).unwrap();
        assert_eq!(first["assessmentId"],replay["assessmentId"]);
        assert_eq!(replay["replayed"],true);
        body["title"] = json!("Outra conversa");
        assert!(create(&store,body.clone()).is_err());
        body["idempotencyKey"] = json!(model::id());
        let second = create(&store,body).unwrap();
        assert_eq!(first["projectId"],second["projectId"]);
        assert_ne!(first["assessmentId"],second["assessmentId"]);
        assert_eq!(store.snapshot().unwrap().projects.len(),1);
    }

    #[test]
    fn narrow_local_scope_has_only_the_selected_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path().join("db")).unwrap();
        let mut body = input("code",tmp.path().to_str().unwrap());
        body["relativeTarget"] = json!("../elsewhere");
        assert!(create(&store,body.clone()).is_err());
        assert!(store.snapshot().unwrap().projects.is_empty());
        body["relativeTarget"] = json!("src/example.rs");
        let result = create(&store,body.clone()).unwrap();
        let s = store.snapshot().unwrap();
        assert_eq!(s.projects[0].target,tmp.path().to_str().unwrap());
        let set = s.scope_revisions[0].body.target_set.as_ref().unwrap();
        assert_eq!(set.entries.len(),1);
        assert_eq!(Path::new(&set.entries[0].target), tmp.path().join("src/example.rs"));
        assert_eq!(result["targetRef"]["entryId"],set.primary_entry_id);
        assert_eq!(create(&store,body).unwrap()["replayed"],true);
    }

    #[test]
    fn invalid_network_scope_never_creates_a_project() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path().join("db")).unwrap();
        let mut body = input("network","127.0.0.0/8");
        body["ports"] = json!([443]);
        assert!(create(&store,body.clone()).is_err());
        body["root"] = json!("127.0.0.1");
        body["useRipgrep"] = json!(true);
        assert!(create(&store,body.clone()).is_err());
        assert!(store.snapshot().unwrap().projects.is_empty());
        body["useRipgrep"] = json!(false);
        assert!(create(&store,body).is_ok());
    }

    #[test]
    fn new_chat_keeps_scoped_entry_and_deadline_without_reusing_work() {
        let tmp=tempfile::tempdir().unwrap();
        let store=Store::open(tmp.path().join("db")).unwrap();
        let mut body=input("code",tmp.path().to_str().unwrap());
        body["relativeTarget"]=json!("only.py");
        body["minutes"]=json!(10);
        body["useRipgrep"]=json!(true);
        let first=create(&store,body).unwrap();
        let intent=json!({"projectId":first["projectId"],"sourceAssessmentId":first["assessmentId"],
            "title":"Nova revisão","idempotencyKey":model::id()});
        let chat=new_conversation(&store,intent.clone()).unwrap();
        assert_eq!(new_conversation(&store,intent).unwrap()["id"],chat["id"]);
        let s=store.snapshot().unwrap();
        let old=&s.scope_revisions[0].body;
        let new=&s.scope_revisions[1].body;
        let selected=&new.target_set.as_ref().unwrap().entries;
        assert_eq!(old.window,new.window);
        assert_eq!(selected.len(),1);
        assert_eq!(selected[0].target,old.target_set.as_ref().unwrap().entries[0].target);
        assert!(selected[0].methods.iter().any(|m|m=="source.rules.v1"));
        assert!(!selected[0].methods.iter().any(|m|m.starts_with("kali.")||m=="source.capture.v1"));
        assert!(s.plans.is_empty()&&s.executions.is_empty()&&s.data_bindings.is_empty());
        assert!(chat["evidence"].as_array().unwrap().is_empty());
    }

    #[test]
    fn options_changes_are_prospective_and_preserve_frozen_scope_and_digest() {
        let tmp=tempfile::tempdir().unwrap();let store=Store::open(tmp.path().join("db")).unwrap();
        let created=create(&store,input("code",tmp.path().to_str().unwrap())).unwrap();let aid=created["assessmentId"].as_str().unwrap();
        store.mutate(|state|scope::freeze_review(state,aid,"test:first-review")).unwrap();
        let before=store.snapshot().unwrap();let old_digest=scope::project_digest(&before.projects[0]);let old_scope=scope::current_ref(&before,aid).unwrap();
        let options=crate::project_options::AnalysisOptions{advanced_enabled:true,additional_context_enabled:true,context:["Authentication".into(),"Uploads".into(),String::new()],max_cost_usd:Some(1.0),minutes:Some(10),title:"Focused review".into()};
        crate::project_options::save(&store,json!({"projectId":created["projectId"],"options":options})).unwrap();
        let after=store.snapshot().unwrap();assert_eq!(old_digest,scope::project_digest(&after.projects[0]));assert_eq!(old_scope,scope::current_ref(&after,aid).unwrap());assert_eq!(before.assessments[0].objective,after.assessments[0].objective);
        let next=new_conversation(&store,json!({"projectId":created["projectId"],"sourceAssessmentId":aid,"title":"Conversa 2","idempotencyKey":model::id()})).unwrap();
        assert_eq!(next["title"],"Focused review");assert!(next["objective"].as_str().unwrap().contains("Authentication"));
        let state=store.snapshot().unwrap();assert_eq!(state.assessments.len(),2);assert!(state.executions.is_empty()&&state.plans.is_empty());
    }

    #[test]
    fn opening_analysis_scans_only_the_selected_file_once_and_retains_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("source");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("sample.py"),"x = eval(user_input)").unwrap();
        std::fs::write(root.join("other.py"),"print('excluded')").unwrap();
        let store = Store::open(tmp.path().join("db")).unwrap();
        let mut body = input("code", root.to_str().unwrap());
        body["relativeTarget"] = json!("sample.py");
        let first = create(&store,body.clone()).unwrap();
        let executor=crate::executor::Executor::default();
        let prepare = |r:&Value| executor.handle("assessment.launchPlan",json!({"assessmentId":r["assessmentId"],
            "scopeRef":r["scopeRef"],"targetRef":r["targetRef"]}),&store);
        let plan=prepare(&first).unwrap();
        assert_eq!(plan["inputFiles"].as_array().unwrap().len(),1);
        let decision=json!({"id":plan["id"],"digest":plan["digest"]});
        executor.handle("plan.approve",decision.clone(),&store).unwrap();
        let execution=executor.handle("plan.run",decision,&store).unwrap();
        assert_eq!(execution["status"],"completed");
        assert!(execution["message"].as_str().unwrap().contains("Arquivos revisados (1/1), sinais candidatos (1)"));
        assert!(prepare(&first).is_err());
        assert_eq!(store.snapshot().unwrap().executions.len(),1);
        body["idempotencyKey"]=json!(model::id());
        body["relativeTarget"]=json!("missing.py");
        let missing=create(&store,body).unwrap();
        assert!(prepare(&missing).is_err());
        let state=store.snapshot().unwrap();
        let blocked=state.assessments.iter().find(|a|a.id==missing["assessmentId"]).unwrap();
        assert_eq!(blocked.status,"blocked");
        assert!(blocked.events.last().unwrap().message.contains("não pôde começar"));
    }
}
