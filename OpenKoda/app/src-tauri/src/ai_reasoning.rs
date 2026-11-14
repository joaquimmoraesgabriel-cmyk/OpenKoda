//! Subscription feature checks happen in the native host before personal-key inference.
//! The provider still bills the user's key; no managed quota is consumed here.
use super::*;
use crate::ai_provider::Reasoning;

pub(super) fn subscribed(value:&Value, now:chrono::DateTime<chrono::Utc>)->bool {
    // This value is obtained by the native host from the pinned account API,
    // never supplied by the renderer. Test access cannot spend managed credit.
    let simulation=&value["simulation"];
    if value["entitlements"]["reasoning"]==true && simulation["mode"]=="internal" &&
        simulation["status"]=="active" && matches!(simulation["plan"].as_str(),Some("plus"|"pro"|"ultra")) &&
        simulation["periodEnd"].as_str().and_then(|v|chrono::DateTime::parse_from_rfc3339(v).ok()).is_some_and(|end|end>now) {
        return true;
    }
    let sub=&value["subscription"];
    matches!(value["plan"]["id"].as_str(),Some("plus"|"pro"|"ultra"))
        && sub["status"]=="active" && sub["mode"]!="mock"
        && sub["periodEnd"].as_str().and_then(|v|chrono::DateTime::parse_from_rfc3339(v).ok()).is_some_and(|end|end>now)
}
fn standard()->Vec<Reasoning>{vec![Reasoning::Low,Reasoning::Medium,Reasoning::High]}
pub(super) fn openai_levels(model:&str)->Vec<Reasoning>{
    let supported=["o3","o3-mini","o4-mini","gpt-5","gpt-5-mini","gpt-5-nano","gpt-5.1","gpt-5.2","gpt-5.4","gpt-5.5"].iter()
        .any(|base| model==*base || model.strip_prefix(&format!("{base}-")).is_some_and(|suffix|suffix.len()==10 && suffix.chars().all(|c|c.is_ascii_digit()||c=='-')));
    if supported{standard()}else{vec![]}
}
pub(super) fn router_levels(catalog:&Value,model:&str)->Vec<Reasoning>{
    let Some(row)=catalog["data"].as_array().and_then(|rows|rows.iter().find(|r|r["id"]==model)) else{return vec![];};
    if !row["supported_parameters"].as_array().is_some_and(|p|p.iter().any(|v|v=="reasoning"||v=="reasoning_effort")){return vec![];}
    let allowed=row.get("reasoning").and_then(|v|v.get("supported_efforts"));
    [Reasoning::Minimal,Reasoning::Low,Reasoning::Medium,Reasoning::High,Reasoning::Xhigh,Reasoning::Max].into_iter().filter(|level|match allowed {
        Some(Value::Null)=>true,
        Some(Value::Array(values))=>values.contains(&json!(level)),
        _=>false,
    }).collect()
}
pub(super) fn model_levels(provider:ProviderId,model:&str,key:&str)->Result<Vec<Reasoning>,String>{
    if provider==ProviderId::OpenAI{return Ok(openai_levels(model));}
    if provider!=ProviderId::OpenRouter{return Ok(vec![]);}
    Ok(router_levels(&catalog::router_catalog(key)?,model))
}
pub(super) fn apply(body:&mut Value,provider:ProviderId,level:Option<Reasoning>)->Result<(),String>{
    let Some(level)=level else{return Ok(());};
    match provider{
        ProviderId::OpenAI=>body["reasoning"]=json!({"effort":level}),
        ProviderId::OpenRouter=>{body["reasoning"]=json!({"effort":level,"exclude":true});body["provider"]["require_parameters"]=json!(true);},
        _=>return Err("Este modelo não oferece ajuste de raciocínio nesta conexão.".into()),
    }
    Ok(())
}
impl AiService {
    pub(crate) fn require_reasoning_subscription(&self,provider:ProviderId,level:Option<Reasoning>)->Result<(),String>{
        if level.is_none()||provider==ProviderId::OpenKoda{return Ok(());}
        if !subscribed(&self.account.edge("account",json!({}))?,chrono::Utc::now()){
            return Err("O ajuste de raciocínio com chave própria requer uma assinatura ativa. Escolha Automático para continuar sem o ajuste.".into());
        }
        Ok(())
    }
    pub(crate) fn validate_personal_reasoning(&self,provider:ProviderId,model:&str,key:&str,level:Option<Reasoning>)->Result<(),String>{
        let Some(level)=level else{return Ok(());};
        if provider==ProviderId::OpenKoda{return Ok(());}
        self.require_reasoning_subscription(provider,Some(level))?;
        if !model_levels(provider,model,key)?.contains(&level){return Err("O modelo não aceita este nível de raciocínio. Escolha Automático ou outro modelo.".into());}
        Ok(())
    }
}
#[cfg(test)] mod tests{
    use super::*;
    #[test] fn entitlement_is_current_and_not_a_simulation(){
        let now=chrono::Utc::now();
        let mut v=json!({"plan":{"id":"plus"},"subscription":{"status":"active","periodEnd":(now+chrono::Duration::days(1)).to_rfc3339()}});
        assert!(subscribed(&v,now));v["subscription"]["mode"]=json!("mock");assert!(!subscribed(&v,now));
        v["subscription"]["mode"]=Value::Null;v["subscription"]["periodEnd"]=json!(now.to_rfc3339());assert!(!subscribed(&v,now));
        assert!(!subscribed(&json!({}),now));
    }
    #[test] fn internal_reasoning_requires_server_entitlement_and_current_test_period(){
        let now=chrono::Utc::now();
        let mut value=json!({"entitlements":{"reasoning":true,"hostedInference":false},
            "simulation":{"plan":"plus","mode":"internal","status":"active","periodEnd":(now+chrono::Duration::days(1)).to_rfc3339()}});
        assert!(subscribed(&value,now));value["entitlements"]["reasoning"]=json!(false);assert!(!subscribed(&value,now));
        value["entitlements"]["reasoning"]=json!(true);value["simulation"]["status"]=json!("expired");assert!(!subscribed(&value,now));
    }
    #[test] fn capabilities_fail_closed_and_transport_preserves_output_cap(){
        assert!(openai_levels("gpt-4o").is_empty());assert!(openai_levels("gpt-5-pro").is_empty());assert_eq!(openai_levels("gpt-5-mini").len(),3);
        let mut catalog=json!({"data":[{"id":"brand/model","supported_parameters":["reasoning"],"reasoning":{"supported_efforts":["low","high"]}}]});
        assert_eq!(router_levels(&catalog,"brand/model"),vec![Reasoning::Low,Reasoning::High]);
        catalog["data"][0]["reasoning"]=json!({"mandatory":true});assert!(router_levels(&catalog,"brand/model").is_empty());
        catalog["data"][0]["reasoning"]=json!({"supported_efforts":null});assert_eq!(router_levels(&catalog,"brand/model").len(),6);
        catalog["data"][0]["supported_parameters"]=json!([]);assert!(router_levels(&catalog,"brand/model").is_empty());
        let mut body=ProviderId::OpenRouter.body("brand/model","fixture","system",128);
        apply(&mut body,ProviderId::OpenRouter,Some(Reasoning::Low)).unwrap();
        assert_eq!(body["reasoning"]["effort"],"low");assert_eq!(body["reasoning"]["exclude"],true);assert_eq!(body["max_tokens"],128);
        assert_eq!(body["provider"]["require_parameters"],true);
        body["provider"]["max_price"]=json!({"completion":2});
        apply(&mut body,ProviderId::OpenRouter,Some(Reasoning::High)).unwrap();
        assert_eq!(body["provider"]["max_price"]["completion"],2);
        let mut direct=ProviderId::OpenAI.body("gpt-5-mini","fixture","system",128);
        apply(&mut direct,ProviderId::OpenAI,Some(Reasoning::Medium)).unwrap();
        assert_eq!(direct["reasoning"],json!({"effort":"medium"}));assert_eq!(direct["max_output_tokens"],128);
        let before=direct.clone();apply(&mut direct,ProviderId::OpenAI,None).unwrap();assert_eq!(direct,before);
        assert!(apply(&mut body,ProviderId::Groq,Some(Reasoning::High)).is_err());
    }
    #[test]
    #[ignore = "Paid opt-in: one capped native OpenRouter request, never run by CI"]
    fn live_native_reasoning_transport() {
        assert_eq!(std::env::var("OPENKODA_LIVE_REASONING").as_deref(),Ok("1"));
        let key_file=std::env::var_os("OPENKODA_LIVE_KEY_FILE").expect("authorized key file required");
        let secret=Zeroizing::new(std::fs::read_to_string(key_file).expect("cannot read authorized key file"));
        let key=secret.trim();assert!(key.starts_with("sk-or-") && !key.contains(char::is_whitespace));
        let model="openai/gpt-oss-20b";
        let catalog=catalog::router_catalog(key).expect("catalog unavailable");
        let row=catalog["data"].as_array().unwrap().iter().find(|row|row["id"]==model).expect("test model unavailable");
        assert!(router_levels(&catalog,model).contains(&Reasoning::Low));
        let price=|field:&str|row["pricing"][field].as_str().unwrap_or("0").parse::<f64>().unwrap();
        assert!(price("prompt")>0. && price("prompt")<=0.0000002);
        assert!(price("completion")>0. && price("completion")<=0.0000005);
        assert!(price("request")==0. && price("internal_reasoning")<=0.0000005);
        let answer=HttpProvider.reasoning_response(ProviderId::OpenRouter,model,key,
            "Quanto é 2 + 2? Responda apenas o número.","Responda brevemente.",128,Some(Reasoning::Low)).expect("native inference failed");
        assert_eq!(answer["status"],"completed");
        assert!(answer["output"][0]["content"][0]["text"].as_str().is_some_and(|text|text.contains('4')));
        let id=answer["id"].as_str().expect("generation receipt missing");
        if let Some(path)=std::env::var_os("OPENKODA_LIVE_RECEIPT") {
            let pending=json!({"passed":false,"test":"native_reasoning_transport","model":model,"generationId":id,"costUsd":null,"status":answer["status"],"usage":answer["usage"]});
            std::fs::write(path,serde_json::to_vec_pretty(&pending).unwrap()).unwrap();
        }
        // Generation receipts can lag the completed response. Retry only this
        // read-only lookup, never the inference request above.
        let mut reconciled=None;
        for attempt in 0..5 {
            if attempt>0 { std::thread::sleep(Duration::from_secs(2)); }
            let response=authenticate(client().unwrap().get("https://openrouter.ai/api/v1/generation").query(&[("id",id)]),ProviderId::OpenRouter,key).send().unwrap();
            if let Ok(receipt)=read_bounded_response(response,65536) {
                if receipt["data"]["total_cost"].as_f64().is_some() { reconciled=Some(receipt); break; }
            }
        }
        let receipt=reconciled.expect("receipt pending; reconcile saved generation ID without repeating inference");
        let cost=receipt["data"]["total_cost"].as_f64().expect("measured cost missing");
        assert!((0.0..=0.001).contains(&cost));
        let evidence=json!({"passed":true,"test":"native_reasoning_transport","model":model,"reasoning":"low",
            "maxOutputTokens":128,"generationId":id,"costUsd":cost,"status":answer["status"],"usage":answer["usage"]});
        if let Some(path)=std::env::var_os("OPENKODA_LIVE_RECEIPT") { std::fs::write(path,serde_json::to_vec_pretty(&evidence).unwrap()).unwrap(); }
        println!("{}",evidence);
    }
}
