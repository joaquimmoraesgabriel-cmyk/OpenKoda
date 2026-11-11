//! MCP JSON-RPC adapter bound to one assessment. No approval or arbitrary shell tool is exposed.
//! This is the in-process transport used by the native broker, not a public HTTP listener.
use crate::{core::Store, executor::Executor};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Input { assessment_id:String, message:Message }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message { jsonrpc:String, #[serde(default)] id:Value, method:String, #[serde(default)] params:Value }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call { name:String, #[serde(default)] arguments:Value }

pub fn handle(payload:Value,store:&Store,executor:&Executor)->Result<Value,String>{
    struct LocalRedactor;
    impl crate::data_projection::DataRedactor for LocalRedactor{
        fn mode(&self)->crate::data_projection::RedactionMode{crate::data_projection::RedactionMode::MetadataOnly}
        fn redact(&self,_:&str)->crate::data_projection::RedactedText{crate::data_projection::RedactedText{text:"[Conteúdo requer o redator do host.]".into(),changed:true,secret_material:false}}
    }
    handle_with_redactor(payload,store,executor,&LocalRedactor)
}
pub(crate) fn handle_with_redactor(payload:Value,store:&Store,executor:&Executor,redactor:&dyn crate::data_projection::DataRedactor)->Result<Value,String>{
    if payload.to_string().len()>64*1024{return Err("Mensagem MCP excedeu o limite.".into());}
    let input:Input=serde_json::from_value(payload).map_err(|_|"Mensagem MCP inválida.")?;
    crate::core::valid_id(&input.assessment_id)?;
    let state=store.snapshot()?;
    let a=state.assessments.iter().find(|a|a.id==input.assessment_id).ok_or("Avaliação MCP não encontrada.")?;
    let combined=crate::scope_targets::is_combined(&state,&a.id);
    let plan_schema=if combined{json!({"type":"object","properties":{"scopeRef":{"type":"object"},"targetRef":{"type":"object"},"adapter":{"type":"string","enum":["local.inventory.v1","source.rules.v1","http.headers.v1","network.tcp-connect.v1"]},"ports":{"type":"array","items":{"type":"integer","minimum":1,"maximum":65535},"minItems":1,"maxItems":16}},"required":["scopeRef","targetRef","adapter"],"additionalProperties":false})}else{json!({"type":"object","properties":{},"additionalProperties":false})};
    let m=input.message;
    if m.jsonrpc!="2.0" || !(m.id.is_null() || m.id.is_string() || m.id.is_number()){return Err("Envelope JSON-RPC inválido.".into());}
    let result:Result<Value,String>=match m.method.as_str(){
        "initialize"=>Ok(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"openkoda-scoped-broker","version":env!("CARGO_PKG_VERSION")},"instructions":"Tools only propose plans or inspect this assessment. Human approval is a separate native action."})),
        "notifications/initialized"=>Ok(Value::Null),
        "ping"=>Ok(json!({})),
        "tools/list"=>Ok(json!({"tools":[
            {"name":"assessment_plan","description":"Propose a bounded native observation for this assessment. Does not approve or execute it. Combined assessments require one explicit reviewed target and scope reference.","inputSchema":plan_schema},
            {"name":"assessment_status","description":"Read status and coverage of the bound assessment.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
            {"name":"evidence_read","description":"Read a redacted evidence projection belonging to this assessment. Raw windows require a separate human review in the native app.","inputSchema":{"type":"object","properties":{"evidenceId":{"type":"string"}},"required":["evidenceId"],"additionalProperties":false}}
        ]})),
        "tools/call"=>{
            let call:Call=serde_json::from_value(m.params).map_err(|_|"Chamada MCP inválida.")?;
            let content=match call.name.as_str(){
                "assessment_plan"=>{
                    if combined{
                        #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
                        struct TargetPlan{scope_ref:crate::scope::ScopeRef,target_ref:crate::scope_targets::ScopeTargetRef,adapter:String,ports:Option<Vec<u16>>}
                        let args:TargetPlan=serde_json::from_value(call.arguments).map_err(|_|"Selecione uma entrada revisada e um adaptador limitado.")?;
                        let mut payload=json!({"assessmentId":a.id,"scopeRef":args.scope_ref,"targetRef":args.target_ref,"adapter":args.adapter});
                        if let Some(ports)=args.ports{payload["ports"]=json!(ports);}
                        executor.handle("assessment.targetPlan",payload,store)
                    }else{empty_arguments(&call.arguments)?;executor.handle("assessment.plan",json!({"id":a.id}),store)}
                },
                "assessment_status"=>{
                    empty_arguments(&call.arguments)?;
                    let mut status=json!({"id":a.id,"status":a.status,"summary":a.summary,"executions":state.executions.iter().filter(|e|e.assessment_id==a.id).collect::<Vec<_>>()});
                    if combined{status["targetCoverage"]=json!(crate::scope::target_coverage(&state,&a.id));status["scopeStatus"]=crate::scope::status(store,&a.id)?;}
                    else{status["coverage"]=json!(a.coverage);}
                    Ok(status)
                },
                "evidence_read"=>{
                    #[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct EvidenceInput{evidence_id:String}
                    let args:EvidenceInput=serde_json::from_value(call.arguments).map_err(|_|"Argumentos de evidência inválidos.")?;
                    let e=a.evidence.iter().find(|e|e.id==args.evidence_id).ok_or("Evidência fora desta avaliação ou inexistente.")?;
                    if crate::core::hash(&e.content)!=e.sha256{return Err("A evidência não passou na verificação de integridade.".into());}
                    crate::data_projection::evidence_view(store,&state,&a.id,&e.id,redactor)
                }
                _=>Err("Ferramenta não autorizada nesta sessão MCP.".into())
            };
            Ok(match content{Ok(value)=>json!({"content":[{"type":"text","text":crate::data_projection::project_value(value,redactor).to_string()}],"isError":false}),Err(error)=>json!({"content":[{"type":"text","text":crate::data_host::project_error(&error,redactor)}],"isError":true})})
        }
        _=>Err("Método MCP não suportado.".into())
    };
    Ok(match result{Ok(result)=>json!({"jsonrpc":"2.0","id":m.id,"result":result}),Err(error)=>json!({"jsonrpc":"2.0","id":m.id,"error":{"code":-32601,"message":error}})})
}
fn empty_arguments(v:&Value)->Result<(),String>{if v.is_null()||v.as_object().is_some_and(|o|o.is_empty()){Ok(())}else{Err("A ferramenta não aceita alteração do escopo.".into())}}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn combined_mcp_requires_explicit_reference_and_never_approves_or_runs(){
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();let(aid,refs)=crate::evidence_lineage::test_scope(&store,&["http.headers.v1"],&[]);let executor=Executor::default();
        let request=|method:&str,params:Value|handle(json!({"assessmentId":aid,"message":{"jsonrpc":"2.0","id":1,"method":method,"params":params}}),&store,&executor);
        let listed=request("tools/list",json!({})).unwrap();assert_eq!(listed["result"]["tools"].as_array().unwrap().len(),3);assert_eq!(listed["result"]["tools"][0]["inputSchema"]["required"],json!(["scopeRef","targetRef","adapter"]));
        assert!(request("tools/call",json!({"name":"assessment_plan","arguments":{}})).is_err());assert!(store.snapshot().unwrap().plans.is_empty());
        let status=crate::scope::status(&store,&aid).unwrap();let reference=json!({"assessmentId":aid,"revisionId":status["revision"]["id"],"digest":status["revision"]["digest"]});
        let selected=refs.iter().find(|r|crate::scope_targets::resolve_current_target(&store.snapshot().unwrap(),&aid,Some(r)).unwrap().target.contains("47992")).unwrap();
        let args=json!({"scopeRef":reference,"targetRef":selected,"adapter":"http.headers.v1"});
        let plan=request("tools/call",json!({"name":"assessment_plan","arguments":args})).unwrap();assert_eq!(plan["result"]["isError"],false);
        let state=store.snapshot().unwrap();assert_eq!(state.plans.len(),1);assert_eq!(state.plans[0].target_ref.as_ref(),Some(selected));assert_eq!(state.plans[0].status,"proposed");assert!(state.executions.is_empty());
        let observed=request("tools/call",json!({"name":"assessment_status","arguments":{}})).unwrap();let content:Value=serde_json::from_str(observed["result"]["content"][0]["text"].as_str().unwrap()).unwrap();assert!(content.get("coverage").is_none());assert_eq!(content["targetCoverage"].as_array().unwrap().len(),2);
        let status=crate::scope::status(&store,&aid).unwrap();crate::scope::handle("assessment.scopeRevoke",json!({"assessmentId":aid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":crate::model::id(),"rationale":"No further plans","acknowledgeRevocation":true}),&store,str::to_owned).unwrap();
        let refused=request("tools/call",json!({"name":"assessment_plan","arguments":args})).unwrap();assert_eq!(refused["result"]["isError"],true);assert_eq!(store.snapshot().unwrap().plans.len(),1);
    }
    #[test]fn mcp_does_not_offer_approval_or_shell(){
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();
        let p=store.handle(crate::core::Request{action:"project.create".into(),payload:json!({"name":"Lab","kind":"web","target":"http://127.0.0.1:9"})}).unwrap();
        let a=store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"kind":"web","target":p["target"],"title":"Lab","objective":"fixture","scope":crate::scope::test_creation(&store,p["id"].as_str().unwrap(),&[])})}).unwrap();
        let executor=Executor::default();
        let listed=handle(json!({"assessmentId":a["id"],"message":{"jsonrpc":"2.0","id":1,"method":"tools/list"}}),&store,&executor).unwrap();
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(),3);
        let denied=handle(json!({"assessmentId":a["id"],"message":{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"plan.approve","arguments":{}}}}),&store,&executor).unwrap();
        assert_eq!(denied["result"]["isError"],true);assert!(store.snapshot().unwrap().plans.is_empty());
        let before=serde_json::to_value(store.snapshot().unwrap()).unwrap();
        let denied=handle(json!({"assessmentId":a["id"],"message":{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"assessment_plan","arguments":{}}}}),&store,&executor).unwrap();
        assert_eq!(denied["result"]["isError"],true);assert!(denied["result"]["content"][0]["text"].as_str().unwrap().contains("scope_method_not_selected"));assert_eq!(serde_json::to_value(store.snapshot().unwrap()).unwrap(),before);
    }
}
