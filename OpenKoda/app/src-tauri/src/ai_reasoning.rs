//! Subscription feature checks happen in the native host before personal-key inference.
//! The provider still bills the user's key; no managed quota is consumed here.
use super::*;
use crate::ai_provider::Reasoning;

pub(super) fn subscribed(value:&Value, now:chrono::DateTime<chrono::Utc>)->bool {
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
    let allowed=row.get("supported_efforts").or_else(||row.get("supported_reasoning_efforts"));
    standard().into_iter().filter(|level|allowed.and_then(Value::as_array).is_none_or(|values|values.contains(&json!(level)))).collect()
}
pub(super) fn model_levels(provider:ProviderId,model:&str,key:&str)->Result<Vec<Reasoning>,String>{
    if provider==ProviderId::OpenAI{return Ok(openai_levels(model));}
    if provider!=ProviderId::OpenRouter{return Ok(vec![]);}
    let response=authenticate(client()?.get(provider.catalog_url()).timeout(Duration::from_secs(15)),provider,key).send().map_err(network_error)?;
    Ok(router_levels(&read_bounded_response(response,16*1024*1024)?,model))
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
    #[test] fn capabilities_fail_closed_and_transport_preserves_output_cap(){
        assert!(openai_levels("gpt-4o").is_empty());assert!(openai_levels("gpt-5-pro").is_empty());assert_eq!(openai_levels("gpt-5-mini").len(),3);
        let mut catalog=json!({"data":[{"id":"brand/model","supported_parameters":["reasoning"],"supported_efforts":["low","high"]}]});
        assert_eq!(router_levels(&catalog,"brand/model"),vec![Reasoning::Low,Reasoning::High]);
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
}
