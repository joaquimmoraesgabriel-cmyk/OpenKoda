//! Exercise the public commands and the durable dispatch/credential boundaries offline.
use super::*;
use crate::core::Request;
use crate::data_projection::DataRedactor;

const KEYS:[&str;7]=["synthetic-openai-provider-key","synthetic-openrouter-provider-key","synthetic-groq-provider-key",
    "synthetic-anthropic-provider-key","synthetic-gemini-provider-key","synthetic-deepseek-provider-key","synthetic-xai-provider-key"];
struct Transport { calls:Mutex<Vec<(ProviderId,String)>> }
impl Provider for Transport {
    fn models(&self,p:ProviderId,key:&str,_:Option<&std::collections::HashSet<String>>)->Result<Vec<String>,String>{
        assert_eq!(key,KEYS[p.slot() as usize-1]);
        Ok(vec!["shared-model".into(),format!("{}-model",p.id())])
    }
    fn response(&self,p:ProviderId,model:&str,key:&str,prompt:&str,_:&str,_:u32)->Result<Value,String>{
        assert_eq!(key,KEYS[p.slot() as usize-1]);
        for key in KEYS {assert!(!prompt.contains(key));}
        self.calls.lock().unwrap().push((p,model.into()));
        Ok(json!({"id":"synthetic-reply","status":"completed","model":model,
            "usage":{"input_tokens":3,"output_tokens":4},
            "output":[{"type":"message","content":[{"type":"output_text","text":format!("Reply {}",KEYS[0])}]}]}))
    }
}
#[test]
fn per_assessment_dispatch_uses_its_slot_and_rejects_stale_reviews(){
    let dir=tempfile::tempdir().unwrap();
    let store=Store::open(dir.path().into()).unwrap();
    let transport=Arc::new(Transport{calls:Mutex::default()});
    let ai=AiService::with_provider(dir.path().into(),transport.clone()).unwrap();
    for (p,key) in ProviderId::ALL.into_iter().zip(KEYS){
        ai.handle("ai.key.save",json!({"provider":p,"key":key}),&store).unwrap();
        let catalog=ai.handle("ai.models",json!({"provider":p}),&store).unwrap();
        assert_eq!(catalog["models"][1],format!("{}-model",p.id()));
    }
    ai.save_settings(json!({"model":"default-model"}),&store).unwrap();
    let redactor=ai.data_redactor();
    for key in KEYS {assert!(redactor.contains_managed_secret(key));}
    let p=store.handle(Request{action:"project.create".into(),payload:json!({"name":"Providers","kind":"web","target":"https://example.invalid/fixture"})}).unwrap();
    let a=store.handle(Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"title":"Scoped provider","kind":"web","target":p["target"],"objective":"Verify provider isolation","scope":crate::scope::test_creation(&store,p["id"].as_str().unwrap(),&[AI_METHOD]),"aiSelection":{"provider":"openrouter","model":"shared-model"}})}).unwrap();
    let id=a["id"].as_str().unwrap();
    let input=json!({"assessmentId":id,"message":"Review synthetic context","allowRemote":true,"evidenceIds":[]});
    let first=ai.test_data_input(&store,"ask",input.clone());
    assert!(first["dataReview"].is_object());
    let save=|provider|ai.handle("ai.selection.save",json!({"assessmentId":id,"aiSelection":{"provider":provider,"model":"shared-model"}}),&store).unwrap();
    save("groq");
    assert!(ai.handle("ai.ask",first,&store).is_err());
    assert!(transport.calls.lock().unwrap().is_empty());
    let reviewed=ai.test_data_input(&store,"ask",input.clone());
    let answer=ai.handle("ai.ask",reviewed.clone(),&store).unwrap();
    assert!(!answer["text"].as_str().unwrap().contains(KEYS[0]));
    assert!(ai.handle("ai.ask",reviewed,&store).is_err());
    assert_eq!(*transport.calls.lock().unwrap(),[(ProviderId::Groq,"shared-model".into())]);
    let history=ai.handle("ai.history",json!({"assessmentId":id}),&store).unwrap();
    assert_eq!(history["requests"][0]["endpoint"],ProviderId::Groq.endpoint());
    assert_eq!(history["requests"][0]["budget"]["endpoint"],ProviderId::Groq.endpoint());
    ai.handle("ai.key.remove",json!({"provider":"openrouter"}),&store).unwrap();
    assert_eq!(ai.handle("ai.status",json!({"provider":"groq"}),&store).unwrap()["hasApiKey"],true);
    assert_eq!(ai.handle("ai.status",json!({"provider":"openrouter"}),&store).unwrap()["hasApiKey"],false);
    let reopened=AiService::with_provider(dir.path().into(),transport).unwrap();
    assert_eq!(reopened.provider_credential(ProviderId::OpenAI).unwrap().0.as_str(),KEYS[0]);
    assert_eq!(reopened.provider_credential(ProviderId::Groq).unwrap().0.as_str(),KEYS[2]);
    assert_eq!(store.snapshot().unwrap().settings.model,"default-model");
}
#[test]
fn catalog_does_not_truncate_and_chat_usage_is_normalized(){
    let claude=ProviderId::Anthropic;
    assert_eq!(claude.catalog_page(&json!({"data":[{"id":"claude-fixture"}],"has_more":true,"last_id":"claude-fixture"})).unwrap().1.as_deref(),Some("claude-fixture"));
    assert!(claude.catalog_page(&json!({"data":[],"has_more":true})).is_err());
    let gemini=ProviderId::Gemini;
    assert_eq!(gemini.catalog_page(&json!({"models":[{"name":"models/gemini-fixture"}],"nextPageToken":"AQ=="})).unwrap(),
        (vec!["gemini-fixture".to_string()],Some("AQ==".to_string())));
    assert_eq!(gemini.catalog_page(&json!({"models":[]})).unwrap().1,None);
    assert_eq!(claude.inference_url(),"https://api.anthropic.com/v1/messages");
    assert_eq!(claude.body("claude","user","rules",300),json!({"model":"claude","system":"rules","max_tokens":300,"messages":[{"role":"user","content":"user"}]}));
    let reply=claude.normalize(json!({"id":"abc","model":"claude","stop_reason":"end_turn","usage":{"input_tokens":9,"cache_read_input_tokens":1,"cache_creation_input_tokens":3,"output_tokens":5},
        "content":[{"type":"thinking","thinking":"not a message"},{"type":"text","text":"hello"}]})).unwrap();
    let parsed=parse_completion(reply,"secret").unwrap();
    assert_eq!(parsed["text"],"hello");
    assert_eq!(parsed["usage"],json!({"input_tokens":13,"output_tokens":5}));
    let aliases=crate::ai_provider::catalog(&json!({"data":[{"id":"org/model:v1"},{"id":"~org/alias-latest"},{"id":"org/model:v1"}]})).unwrap();
    assert_eq!(aliases,["org/model:v1","~org/alias-latest"]);
    let rows=(0..1200).map(|n|json!({"id":format!("org/model-{n}")})).collect::<Vec<_>>();
    assert_eq!(crate::ai_provider::catalog(&json!({"data":rows})).unwrap().len(),1200);
    for p in [ProviderId::OpenRouter,ProviderId::Groq,ProviderId::Gemini,ProviderId::DeepSeek,ProviderId::Xai]{
        let body=p.body("org/model","user","system",50);
        assert_eq!(body["model"],"org/model");
        if p==ProviderId::Groq {assert_eq!(body["tool_choice"],"none");}
        assert_eq!(body["messages"][1]["content"],"user");
        assert_eq!(p.inference_url(),format!("{}/chat/completions",p.endpoint()));
        let body=p.normalize(json!({"id":"abc","model":"org/model","choices":[{"finish_reason":"length","message":{"content":"partial"}}],"usage":{"prompt_tokens":15,"completion_tokens":50}})).unwrap();
        assert_eq!(body["status"],"incomplete");
        assert_eq!(body["usage"],json!({"input_tokens":15,"output_tokens":50}));
        assert!(parse_completion(body,"key").is_err());
    }
}

#[test]
fn activating_models_binds_the_catalog_to_the_key_and_keeps_legacy_selections_working() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().into()).unwrap();
    let transport = Arc::new(Transport { calls: Mutex::default() });
    let ai = AiService::with_provider(dir.path().into(), transport.clone()).unwrap();
    let status = || ai.handle("ai.status", json!({"provider":"openai"}), &store).unwrap();
    ai.handle("ai.key.save", json!({"provider":"openai","key":KEYS[0]}), &store).unwrap();
    ai.save_settings(json!({"model":"shared-model"}), &store).unwrap();
    let legacy_revision = status()["credentialRevision"].clone();
    assert!(status()["enabledModels"].is_null());
    assert!(ai.config_for(&store, None, true).is_ok());

    let pasted = format!(" {}\r\n", KEYS[0]);
    ai.handle("ai.key.save", json!({"provider":"openai","key":pasted,"prepareActivation":true}), &store).unwrap();
    let catalog = ai.handle("ai.models", json!({"provider":"openai"}), &store).unwrap();
    let revision = &catalog["credentialRevision"];
    assert_eq!(revision, &status()["credentialRevision"]);
    assert_ne!(revision, &legacy_revision);
    assert_eq!(status()["enabledModels"], json!([]));
    assert!(ai.config_for(&store, None, true).is_err());
    let configure = |revision: &Value, models: Value, default_model: Value| ai.handle("ai.provider.configure",
        json!({"provider":"openai","credentialRevision":revision,"models":models,"defaultModel":default_model}), &store);
    let enabled = json!(["openai-model","shared-model"]);
    assert!(configure(&legacy_revision, enabled.clone(), json!("openai-model")).is_err());
    assert!(configure(revision, json!([]), json!(null)).is_err());
    assert!(configure(revision, enabled.clone(), json!("outside")).is_err());
    for _ in 0..2 {
        assert_eq!(configure(revision, enabled.clone(), json!("openai-model")).unwrap()["enabledModels"], enabled);
    }
    assert_eq!(ai.config_for(&store, None, true).unwrap().model, "openai-model");
    let restarted = AiService::with_provider(dir.path().into(),transport.clone()).unwrap();
    assert_eq!(restarted.handle("ai.status",json!({"provider":"openai"}),&store).unwrap()["enabledModels"], enabled);
    assert!(configure(revision, json!(["shared-model"]), json!(null)).is_err());
    assert_eq!(configure(revision, json!(["shared-model"]), json!("shared-model")).unwrap()["model"], "shared-model");
    assert!(ai.require_enabled_model(&store.snapshot().unwrap(), ProviderId::OpenAI, "openai-model").is_err());
    assert!(crate::data_host::project_response("snapshot", json!({"settings":{"hasApiKey":true}}), &store, &ai.data_redactor())
        .unwrap().get("providerModels").is_none());
    ai.handle("ai.key.remove", json!({"provider":"openai"}), &store).unwrap();
    assert_eq!(status()["enabledModels"], json!([]));
    assert!(configure(revision,enabled.clone(),json!("shared-model")).is_err());
    ai.handle("ai.key.save", json!({"provider":"openai","key":KEYS[0]}), &store).unwrap();
    assert_eq!(status()["enabledModels"], json!([]));
    assert!(ai.config_for(&store, None, true).is_err());
    assert!(transport.calls.lock().unwrap().is_empty());
}
#[test]
fn budget_distinguishes_identically_named_models_on_different_providers(){
    let dir=tempfile::tempdir().unwrap();
    let store=Store::open(dir.path().into()).unwrap();
    let ai=AiService::with_provider(dir.path().into(),Arc::new(Transport{calls:Mutex::default()})).unwrap();
    for (p,key) in ProviderId::ALL.into_iter().zip(KEYS){ai.handle("ai.key.save",json!({"provider":p,"key":key}),&store).unwrap();}
    let status=ai.handle("ai.budget.status",json!({}),&store).unwrap();
    let price=|rate:&str|json!({"model":"shared-model","inputUsdPerMillion":rate,"outputUsdPerMillion":rate,"sourceNote":"Synthetic price"});
    let mut groq=price("10");groq["provider"]=json!("groq");
    let config=json!({"expectedRevision":null,"enabled":true,"currency":"USD","installationLimitUsd":"10","assessmentLimitUsd":"1","prices":[price("1"),groq],"acknowledgeEstimates":true,"acknowledgeLegacyExcluded":true,"legacyReviewDigest":status["legacyReviewDigest"]});
    ai.handle("ai.budget.configure",config,&store).unwrap();
    let quote=||ai.handle("ai.budget.preview",json!({"operation":"test","payload":{}}),&store);
    ai.save_settings(json!({"model":"shared-model"}),&store).unwrap();
    let old=quote().unwrap()["review"].clone();
    for p in [ProviderId::OpenRouter,ProviderId::Groq]{
        ai.save_settings(json!({"model":"shared-model","provider":p,"endpoint":p.endpoint()}),&store).unwrap();
        assert!(ai.handle("ai.test",json!({"budgetReview":{"id":old["id"],"digest":old["digest"]}}),&store).is_err());
        if p==ProviderId::OpenRouter{assert!(quote().is_err());}
        else {let current=quote().unwrap();assert_eq!(current["review"]["endpoint"],p.endpoint());assert_eq!(current["review"]["rates"]["inputUsdPerMillion"],"10.000000");}
    }
}

#[test]
fn restored_model_preferences_cannot_activate_another_profiles_credential() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().into()).unwrap();
    let transport = Arc::new(Transport { calls: Mutex::default() });
    let ai = AiService::with_provider(dir.path().into(), transport.clone()).unwrap();
    store.handle(Request {action:"project.create".into(), payload:json!({"name":"Fixture","kind":"web","target":"https://example.invalid"})}).unwrap();
    let params = json!({"provider":"groq"});
    let key = json!({"provider":"groq","key":KEYS[2],"prepareActivation":true});
    ai.handle("ai.key.save", key.clone(), &store).unwrap();
    let catalog = ai.handle("ai.models", params.clone(), &store).unwrap();
    let activation = json!({"provider":"groq","models":["groq-model"],"defaultModel":"groq-model","credentialRevision":catalog["credentialRevision"]});
    ai.handle("ai.provider.configure", activation.clone(), &store).unwrap();

    let recovery = crate::recovery::RecoveryService::new(dir.path().into()).unwrap();
    let backup = crate::recovery::test_support::backup(&recovery,&store).unwrap();
    let restored = recovery.handle("backup.restore",json!({"id":backup["id"]}),&store).unwrap();
    let root = PathBuf::from(restored["path"].as_str().unwrap());
    let copy = Store::open(root.clone()).unwrap();
    let other = AiService::with_provider(root,transport).unwrap();
    let status = other.handle("ai.status",params.clone(),&copy).unwrap();
    assert_eq!(status["hasApiKey"], false);
    assert_eq!(status["enabledModels"], json!([]));
    assert_eq!(copy.snapshot().unwrap().settings.model, "groq-model");
    other.handle("ai.key.save",key,&copy).unwrap();
    assert!(other.handle("ai.provider.configure",activation,&copy).is_err());
    assert!(other.config_for(&copy,None,true).is_err());
    let fresh = other.handle("ai.models",params.clone(),&copy).unwrap();
    other.handle("ai.provider.configure",json!({"provider":"groq","models":["shared-model"],"defaultModel":"shared-model","credentialRevision":fresh["credentialRevision"]}),&copy).unwrap();
    assert_eq!(other.config_for(&copy,None,true).unwrap().model, "shared-model");
    assert_eq!(ai.handle("ai.status", params, &store).unwrap()["enabledModels"], json!(["groq-model"]));
}
