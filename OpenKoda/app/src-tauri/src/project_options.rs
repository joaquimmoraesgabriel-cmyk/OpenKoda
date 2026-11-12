//! Prospective project defaults. Saving preferences never edits an assessment,
//! grants data access, reactivates an expired scope, or starts a scan.
use crate::{core::{self,Store},model::Project};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};

#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct AnalysisOptions {
    pub advanced_enabled:bool,
    pub additional_context_enabled:bool,
    pub context:[String;3],
    pub max_cost_usd:Option<f64>,
    pub minutes:Option<u16>,
    #[serde(default)]pub title:String,
}
impl AnalysisOptions {
    pub(crate) fn validate(&self)->Result<(),String>{
        for text in &self.context{core::text(text,"Contexto adicional",2000,false)?;}
        if self.context.iter().map(String::len).sum::<usize>()>6000{return Err("O contexto adicional excede o limite do projeto.".into());}
        core::text(&self.title,"Título",160,false)?;
        if self.max_cost_usd.is_some_and(|v|!v.is_finite()||!(0.01..=100.0).contains(&v)) {return Err("Informe um limite entre US$ 0,01 e US$ 100.".into());}
        if self.minutes.is_some_and(|v|!(1..=1440).contains(&v)){return Err("Informe um prazo entre 1 e 1440 minutos.".into());}
        Ok(())
    }
    pub(crate) fn objective(&self,base:&str)->Result<String,String>{
        let labels=["Vetores de ataque","Áreas de foco","Contexto adicional de segurança"];
        let mut value=base.to_string();
        if self.additional_context_enabled{for (label,text) in labels.iter().zip(&self.context){if !text.trim().is_empty(){value.push_str(&format!("\n\n{label} — {}",text.trim()));}}}
        core::text(&value,"Descrição e contexto",4000,true)
    }
    pub(crate) fn deadline(&self)->Option<String>{
        self.advanced_enabled.then_some(self.minutes).flatten().map(|n|(chrono::Utc::now()+chrono::Duration::minutes(i64::from(n))).to_rfc3339())
    }
}
pub(crate) fn current(project:&Project)->AnalysisOptions{project.analysis_options.clone().unwrap_or_default()}
/// These three user-authored preference fields are not a captured source
/// context. Preserve their typed shape without exempting arbitrary `context`
/// properties from the general data projection. Validation bounds their size;
/// do not silently truncate a valid preference and later save that truncation.
pub(crate) fn view(options:&AnalysisOptions,redactor:&dyn crate::data_projection::DataRedactor)->Value{
    use crate::data_projection::{self,RedactionMode};
    let mut projected=data_projection::project_value(json!(options),redactor);
    projected["context"]=json!(options.context.iter().map(|text|{
        if redactor.mode()==RedactionMode::MetadataOnly{data_projection::project_text(text,redactor)}
        else{redactor.redact(text).text}
    }).collect::<Vec<_>>());
    projected
}
pub(crate) fn response(value:Value,redactor:&dyn crate::data_projection::DataRedactor)->Result<Value,String>{
    #[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Response{options:AnalysisOptions}
    let response:Response=serde_json::from_value(value).map_err(|_|"Resposta das opções do projeto inválida.")?;
    response.options.validate()?;
    Ok(json!({"options":view(&response.options,redactor),"projection":crate::data_projection::projection_info(redactor,true,vec![])}))
}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Lookup{project_id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct Save{project_id:String,options:AnalysisOptions}
pub(crate) fn get(store:&Store,payload:Value)->Result<Value,String>{
    let input:Lookup=serde_json::from_value(payload).map_err(|_|"Projeto inválido.")?;core::valid_id(&input.project_id)?;
    let state=store.snapshot()?;let project=state.projects.iter().find(|p|p.id==input.project_id).ok_or("Projeto não encontrado.")?;
    Ok(json!({"options":current(project)}))
}
pub(crate) fn save(store:&Store,payload:Value)->Result<Value,String>{
    let input:Save=serde_json::from_value(payload).map_err(|_|"Opções do projeto inválidas.")?;core::valid_id(&input.project_id)?;input.options.validate()?;
    store.mutate(|state|{
        let project=state.projects.iter_mut().find(|p|p.id==input.project_id&&!p.archived).ok_or("Projeto indisponível.")?;
        if !matches!(project.kind.as_str(),"code"|"files"){return Err("As opções exigem um projeto de código local.".into());}
        project.analysis_options=Some(input.options.clone());Ok(json!({"options":input.options}))
    })
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn options_persist_without_changing_authority_or_starting_work(){
        let tmp=tempfile::tempdir().unwrap();let root=tmp.path().join("db");let store=Store::open(root.clone()).unwrap();
        let p=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Source","kind":"code","target":tmp.path().to_str().unwrap(),"description":"Review"})}).unwrap();
        let before=store.snapshot().unwrap();let options=AnalysisOptions{advanced_enabled:true,additional_context_enabled:true,context:["Auth".into(),"Uploads".into(),String::new()],max_cost_usd:Some(2.0),minutes:Some(30),title:"Focused review".into()};
        save(&store,json!({"projectId":p["id"],"options":options})).unwrap();
        let after=store.snapshot().unwrap();assert_eq!(before.assessments.len(),after.assessments.len());assert!(after.scope_revisions.is_empty()&&after.data_policies.is_empty());
        drop(store);let reopened=Store::open(root).unwrap();assert_eq!(get(&reopened,json!({"projectId":p["id"]})).unwrap()["options"]["context"][0],"Auth");
    }
    #[test]fn invalid_limits_and_unbounded_context_are_rejected(){
        for cap in [0.0,100.01,f64::INFINITY]{assert!(AnalysisOptions{max_cost_usd:Some(cap),..Default::default()}.validate().is_err());}
        let v=AnalysisOptions{additional_context_enabled:true,context:["x".repeat(2000),"x".repeat(2000),"x".repeat(2000)],..Default::default()};
        assert!(v.objective("Review").is_err());assert_eq!(AnalysisOptions::default().objective("Review").unwrap(),"Review");
    }
}
