//! Host-only provider connection and per-assessment context. Model output has no execution authority.
use crate::{core::Store, model::{self, Finding}};
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf, sync::{Arc, Mutex}, time::Duration};
use zeroize::Zeroizing;
use crate::scope::{self, ScopeRef};
use crate::scope_targets::{self, ScopeTargetRef};
use crate::ai_provider::{ProviderId,AiSelection};
const AI_METHOD:&str="ai.remote-review.v1";

#[path = "ai_chat.rs"]
mod chat;
#[path = "ai_reasoning.rs"]
mod reasoning;
#[path = "chat_compaction.rs"]
mod chat_compaction;
#[path = "chat_skill_match.rs"]
mod chat_skill_match;
#[path = "investigation.rs"]
mod investigation;
#[path = "ai_planning.rs"]
mod planning;
#[path = "ai_budget.rs"]
mod budget;
#[path = "ai_scan_transport.rs"]
mod scan_transport;
pub(crate) use scan_transport::{HttpScan, Rates as ScanRates, ReviewModel as ScanModel, ScanTransport};
#[path = "ai_data.rs"]
mod data;
#[path = "ai_work.rs"]
mod work;
#[path = "ai_local_closure.rs"]
mod local_closure;
pub(crate) use local_closure::AiLocalClosureProof;
pub(crate) use work::{has_unresolved_work,AiNotStartedProof};
pub(crate) use investigation::workflow::{WorkAiExpectation,WorkAiInspection,QueuedAiGrant,WorkAiCompletion,AiTerminalProof};
pub(crate) use budget::Consent as BudgetConsent;
pub(crate) use data::AiDataRedactor;
pub(crate) fn validate_investigation_record(value:&Value)->Result<(),String>{investigation::validate_record(value)}
#[cfg(all(test,windows))]
#[path = "combined_ai_tests.rs"]
mod combined_tests;
#[cfg(all(test,windows))]
#[path="provider_selection_tests.rs"]
mod provider_selection_tests;

#[cfg(test)]
const ENDPOINT: &str = "https://api.openai.com/v1";
const MAX_RESPONSE: u64 = 2 * 1024 * 1024;
const MAX_CONTEXT: usize = 48 * 1024;

pub struct AiService { pub(crate) account:crate::commercial::Account, chat:chat::ChatControl, db: Mutex<Connection>, inference: Mutex<()>, provider:Arc<dyn Provider>, investigation_active:Mutex<Option<investigation::Active>>, budget:budget::Ledger, data_policy:Arc<crate::data_policy::DataPolicyService>, data_reviews:Mutex<std::collections::HashMap<String,data::PreparedAiData>>, local_closure_reviews:Mutex<std::collections::HashMap<String,local_closure::PreparedClosure>>, arbiter:Arc<crate::work_supervisor::WorkArbiter> }

pub(crate) const VAULT_SCHEMA:&str="CREATE TABLE vault (id INTEGER PRIMARY KEY CHECK(id IN (1,2,3)), cipher BLOB NOT NULL, revision TEXT NOT NULL)";
pub(crate) const CHECK_SCHEMA:&str="CREATE TABLE connection_check (id INTEGER PRIMARY KEY CHECK(id IN (1,2,3)), model TEXT NOT NULL, endpoint TEXT NOT NULL, revision TEXT NOT NULL, checked TEXT NOT NULL)";

pub(crate) const VAULT_SCHEMA_V2:&str="CREATE TABLE vault (id INTEGER PRIMARY KEY CHECK(id BETWEEN 1 AND 7), cipher BLOB NOT NULL, revision TEXT NOT NULL)";
pub(crate) const CHECK_SCHEMA_V2:&str="CREATE TABLE connection_check (id INTEGER PRIMARY KEY CHECK(id BETWEEN 1 AND 7), model TEXT NOT NULL, endpoint TEXT NOT NULL, revision TEXT NOT NULL, checked TEXT NOT NULL)";

fn expand_provider_slots(tx:&rusqlite::Transaction<'_>,provider:ProviderId)->Result<(),String>{
    let version:i64=tx.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(failure)?;
    if !(0..=2).contains(&version){return Err("Versão do vault de IA não suportada.".into());}
    let required=if provider.slot()>3{2}else{1};
    if version>=required{return Ok(());}
    tx.execute_batch("ALTER TABLE vault RENAME TO old_vault; ALTER TABLE connection_check RENAME TO old_check;").map_err(failure)?;
    tx.execute_batch(if required==2{VAULT_SCHEMA_V2}else{VAULT_SCHEMA}).map_err(failure)?;
    tx.execute_batch(if required==2{CHECK_SCHEMA_V2}else{CHECK_SCHEMA}).map_err(failure)?;
    tx.execute_batch("INSERT INTO vault SELECT * FROM old_vault; INSERT INTO connection_check SELECT * FROM old_check;
        DROP TABLE old_vault; DROP TABLE old_check;").map_err(failure)?;
    tx.pragma_update(None,"user_version",required).map_err(failure)
}

fn authenticate(request:reqwest::blocking::RequestBuilder, provider:ProviderId, key:&str) -> reqwest::blocking::RequestBuilder {
    if provider == ProviderId::Anthropic {
        request.header("x-api-key",key).header("anthropic-version","2023-06-01")
    } else { request.bearer_auth(key) }
}

trait Provider:Send+Sync {
    fn reasoning_response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,reasoning:Option<crate::ai_provider::Reasoning>)->Result<Value,String>{
        if reasoning.is_some(){return Err("O transporte não oferece raciocínio configurável.".into());}
        self.response(provider,model,key,prompt,instructions,maximum)
    }
    fn reasoning_chat_response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,reasoning:Option<crate::ai_provider::Reasoning>,stop:&std::sync::atomic::AtomicBool,progress:Arc<Mutex<chat::Progress>>)->Result<Value,String>{
        if reasoning.is_some(){return Err("O transporte não oferece raciocínio configurável.".into());}
        self.chat_response(provider,model,key,prompt,instructions,maximum,stop,progress)
    }
    fn response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,max_output_tokens:u32)->Result<Value,String>;
    fn chat_response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,_stop:&std::sync::atomic::AtomicBool,_progress:Arc<Mutex<chat::Progress>>)->Result<Value,String>{self.response(provider,model,key,prompt,instructions,maximum)}
    fn models(&self,provider:ProviderId,key:&str,scan_prices:Option<&std::collections::HashSet<String>>)->Result<Vec<String>,String>{
        if provider == ProviderId::OpenKoda {
            let catalog = commercial_models(key)?;
            return catalog["models"].as_array().ok_or("Catálogo da assinatura inválido.")?.iter()
                .map(|row| row["id"].as_str().filter(|id|crate::ai_provider::valid_model(id)).map(str::to_owned)
                    .ok_or_else(||"Modelo da assinatura inválido.".into())).collect();
        }
        let client=client()?;
        let mut models=std::collections::BTreeSet::new();
        let mut cursors=std::collections::HashSet::new();
        let mut cursor=None::<String>;
        loop {
            let mut page=client.get(provider.catalog_url());
            match provider {
                ProviderId::Anthropic => {
                    page=authenticate(page,provider,key).query(&[("limit","1000")]);
                    if let Some(id)=cursor.as_deref(){page=page.query(&[("after_id",id)]);}
                }
                ProviderId::Gemini => {
                    page=page.header("x-goog-api-key",key).query(&[("pageSize","1000")]);
                    if let Some(token)=cursor.as_deref(){page=page.query(&[("pageToken",token)]);}
                }
                _ => page=authenticate(page,provider,key),
            }
            let response=page.send().map_err(network_error)?;
            let body=read_bounded_response(response,16*1024*1024)?;
            let (names,next)=provider.catalog_page(&body)?;
            if let Some(prices)=scan_prices {
                if provider==ProviderId::OpenRouter {
                    models.extend(body["data"].as_array().into_iter().flatten()
                        .filter(|row|scan_transport::router_settings(row).is_ok())
                        .filter_map(|row|row["id"].as_str()).map(str::to_owned));
                } else {
                    let mut names=names;
                    if provider==ProviderId::Gemini {
                        let generative:std::collections::HashSet<_>=body["models"].as_array().into_iter().flatten()
                            .filter(|row|row["supportedGenerationMethods"].as_array().is_some_and(|methods|methods.iter().any(|m|m=="generateContent")))
                            .filter_map(|row|row["name"].as_str()?.strip_prefix("models/")).collect();
                        names.retain(|id|generative.contains(id.as_str()));
                    }
                    models.extend(names.into_iter().filter(|id|prices.contains(id)||scan_transport::bundled_rates(provider,id).is_some()));
                }
            } else { models.extend(names); }
            let Some(next)=next else{return Ok(models.into_iter().collect());};
            if cursors.len()>=100 || !cursors.insert(next.clone()){return Err("O catálogo excedeu o limite de páginas ou retornou um cursor repetido.".into());}
            cursor=Some(next);
        }
    }
}
struct HttpProvider;
fn commercial_models(key:&str)->Result<Value,String>{
    let response=client()?.post(crate::commercial::EDGE).header("apikey",crate::commercial::PUBLISHABLE)
        .bearer_auth(key).json(&json!({"action":"models"})).send().map_err(network_error)?;
    read_bounded_response(response,2*1024*1024)
}
impl Provider for HttpProvider {
    fn reasoning_response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,reasoning:Option<crate::ai_provider::Reasoning>)->Result<Value,String>{
        let mut body=provider.body(model,prompt,instructions,maximum);
        self::reasoning::apply(&mut body,provider,reasoning)?;
        let response=authenticate(client()?.post(provider.inference_url()),provider,key).json(&body).send().map_err(network_error)?;
        provider.normalize(read_bounded_response(response,MAX_RESPONSE)?)
    }
    fn reasoning_chat_response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,reasoning:Option<crate::ai_provider::Reasoning>,stop:&std::sync::atomic::AtomicBool,progress:Arc<Mutex<chat::Progress>>)->Result<Value,String>{chat::http_chat(provider,model,key,prompt,instructions,maximum,reasoning,stop,progress)}
    fn chat_response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,maximum:u32,stop:&std::sync::atomic::AtomicBool,progress:Arc<Mutex<chat::Progress>>)->Result<Value,String>{chat::http_chat(provider,model,key,prompt,instructions,maximum,None,stop,progress)}
    fn response(&self,provider:ProviderId,model:&str,key:&str,prompt:&str,instructions:&str,max_output_tokens:u32)->Result<Value,String>{
        let body=provider.body(model,prompt,instructions,max_output_tokens);
        let response=authenticate(client()?.post(provider.inference_url()),provider,key).json(&body).send().map_err(network_error)?;
        provider.normalize(read_bounded_response(response,MAX_RESPONSE)?)
    }
}

fn failure(_: impl std::fmt::Display) -> String { "Falha no armazenamento protegido da IA; os dados foram preservados.".into() }
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|_| "Campos inválidos na solicitação de IA.".into())
}
fn digest(text: &str) -> String { format!("{:x}", Sha256::digest(text.as_bytes())) }

#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct ContextInput { assessment_id: String, #[serde(default)] evidence_ids: Vec<String>, #[serde(default)] context_target_refs: Vec<ScopeTargetRef> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct AskInput { assessment_id: String, evidence_ids: Vec<String>, context_digest: String, message: String, allow_remote: bool, #[serde(default)] budget_review:Option<budget::Consent>, #[serde(default)] context_target_refs:Vec<ScopeTargetRef>, #[serde(default)] data_review:Option<crate::data_policy::DataReviewRef>, #[serde(default)] acknowledge_sensitive:bool }
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
struct TestInput {#[serde(default)]budget_review:Option<budget::Consent>}
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct KeyInput { key: String, provider:Option<ProviderId>, #[serde(default)] prepare_activation: bool }
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct ConfigureProvider {
    provider: ProviderId,
    credential_revision: String,
    models: std::collections::BTreeSet<String>,
    default_model: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderInput { provider:Option<ProviderId> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct CatalogInput {provider:Option<ProviderId>,#[serde(default)] for_analysis:bool,#[serde(default)] reasoning_model:Option<String>}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct AssessmentSelection { assessment_id:String, ai_selection:Option<AiSelection> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct CandidateInput { assessment_id: String, title: String, description: String, severity: String, evidence_ids: Vec<String>, #[serde(default)] recommendation: String }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct RequestResolutionInput {assessment_id:String,request_id:String,note:String,acknowledge_unknown_usage:bool}

/// Acknowledgment does not change unknown usage to zero or remove the request
/// from request limits. It only records the user's disposition of the history.
pub(crate) fn request_unknown_acknowledged(details:&str)->bool {
    let Ok(value)=serde_json::from_str::<Value>(details)else{return false;};
    let r=&value["resolution"];
    r["acknowledgeUnknownUsage"]==true&&r["usageRemainsUnknown"]==true&&r["author"]=="local-user"
        &&r["id"].as_str().is_some_and(|s|crate::core::valid_id(s).is_ok())
        &&r["recordedAt"].as_str().is_some_and(|s|chrono::DateTime::parse_from_rfc3339(s).is_ok())
        &&r["note"].as_str().is_some_and(|s|!s.trim().is_empty()&&s.len()<=2000)
}

impl AiService {
    pub fn new(root: PathBuf) -> Result<Self,String> {
        Self::open_profile(root,Arc::new(HttpProvider),budget::Ledger::profile_only())
    }
    pub fn new_with_budget_root(root:PathBuf,installation_root:PathBuf)->Result<Self,String>{
        Self::open_profile(root,Arc::new(HttpProvider),budget::Ledger::open(&installation_root))
    }
    fn with_provider(root:PathBuf,provider:Arc<dyn Provider>)->Result<Self,String>{
        let ledger=budget::Ledger::open(&root);Self::open_profile(root,provider,ledger)
    }
    fn open_profile(root:PathBuf,provider:Arc<dyn Provider>,budget:budget::Ledger)->Result<Self,String>{
        let db = Connection::open(root.join("assistant.sqlite3")).map_err(failure)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(failure)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS vault (id INTEGER PRIMARY KEY CHECK(id=1), cipher BLOB NOT NULL, revision TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS messages (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, role TEXT NOT NULL, text TEXT NOT NULL, created TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS requests (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, endpoint TEXT NOT NULL, model TEXT NOT NULL, revision TEXT NOT NULL, context_hash TEXT NOT NULL, status TEXT NOT NULL, created TEXT NOT NULL, input_tokens INTEGER, output_tokens INTEGER);
            CREATE TABLE IF NOT EXISTS connection_check (id INTEGER PRIMARY KEY CHECK(id=1), model TEXT NOT NULL, endpoint TEXT NOT NULL, revision TEXT NOT NULL, checked TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS investigations (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, data TEXT NOT NULL);
            UPDATE requests SET status='unknown' WHERE status='pending';").map_err(failure)?;
        let details_column:i64=db.query_row("SELECT COUNT(*) FROM pragma_table_info('requests') WHERE name='details'",[],|r|r.get(0)).map_err(failure)?;
        if details_column==0 {db.execute("ALTER TABLE requests ADD COLUMN details TEXT NOT NULL DEFAULT '{}'",[]).map_err(failure)?;}
        db.execute("UPDATE requests SET details=json_set(details,'$.operationProposal.status','interrupted') WHERE json_type(details,'$.operationProposal')='object' AND json_extract(details,'$.operationProposal.status')='pending'",[]).map_err(failure)?;
        let service=Self { account:crate::commercial::Account::new(&root), chat:chat::ChatControl::default(), db: Mutex::new(db), inference: Mutex::new(()), provider, investigation_active:Mutex::new(None), budget, data_policy:Arc::new(crate::data_policy::DataPolicyService::new()), data_reviews:Mutex::new(std::collections::HashMap::new()), local_closure_reviews:Mutex::new(std::collections::HashMap::new()), arbiter:Arc::new(crate::work_supervisor::WorkArbiter::default()) };
        service.recover_investigations()?;
        Ok(service)
    }

    pub fn with_data_policy(mut self, service:Arc<crate::data_policy::DataPolicyService>)->Self { self.data_policy=service; self }

    pub fn supports(action: &str) -> bool {
        matches!(action, "ai.chat.stop"|"ai.chat.status"|"ai.selection.save"|"ai.provider.configure"|"ai.status"|"ai.key.save"|"ai.key.remove"|"ai.models"|"ai.test"|"ai.context"|"ai.ask"|"ai.history"|"ai.request.reconcile"|"candidate.record")||investigation::supports(action)||planning::supports(action)||budget::supports(action)
    }

    fn selected_provider(&self,requested:Option<ProviderId>,store:&Store)->Result<ProviderId,String>{
        match requested {Some(p)=>Ok(p),None=>ProviderId::parse(&self.config_for(store,None,false)?.provider)}
    }
    fn revision(&self,provider:ProviderId)->Result<Option<String>,String>{
        if provider==ProviderId::OpenKoda{return self.account.identity_revision();}
        self.db.lock().map_err(failure)?.query_row("SELECT revision FROM vault WHERE id=?1",[provider.slot()],|r|r.get(0)).optional().map_err(failure)
    }
    pub fn has_selected_key(&self,store:&Store)->bool{
        self.selected_provider(None,store).and_then(|p|self.revision(p)).ok().flatten().is_some()
    }
    fn all_keys(&self)->Result<Vec<Zeroizing<String>>,String>{
        let db=self.db.lock().map_err(failure)?;
        let mut q=db.prepare("SELECT cipher FROM vault ORDER BY id").map_err(failure)?;
        let keys=q.query_map([],|r|r.get::<_,Vec<u8>>(0)).map_err(failure)?;
        let mut keys=keys.map(|cipher|Self::decrypt_key(&cipher.map_err(failure)?)).collect::<Result<Vec<_>,String>>()?;
        keys.sort_by_key(|key|std::cmp::Reverse(key.len()));
        Ok(keys)
    }
    fn redact_keys(text:&str,keys:&[Zeroizing<String>])->String{
        let result=keys.iter().fold(text.to_owned(),|s,key|s.replace(key.as_str(),"[REDACTED]"));
        redact(&result,None)
    }

    /// Provider configuration shares the dispatch lease with inference and key
    /// rotation. The native command router must use this for settings.save.
    pub fn save_settings(&self,payload:Value,store:&Store)->Result<Value,String>{
        let _lease=self.inference.try_lock().map_err(|_|"Aguarde a inferência em andamento antes de mudar as configurações.")?;
        store.handle(crate::core::Request{action:"settings.save".into(),payload})
    }

    /// Local derivation only. The callback receives a redactor, never the key.
    /// Sharing this lease prevents credential rotation during preview/capture.
    pub(crate) fn with_local_redactor<T>(&self,operation:impl FnOnce(&dyn Fn(&str)->String)->Result<T,String>)->Result<T,String>{
        let _lease=self.inference.try_lock().map_err(|_|"Aguarde a solicitação de IA antes de revisar uma captura local.")?;
        let keys=self.all_keys()?;
        operation(&|text|Self::redact_keys(text,&keys))
    }

    /// Revocation must remain reachable while inference owns its long lease.
    /// Only copy the managed credentials for local redaction; this grants no
    /// provider/capture capability and never returns credential bytes.
    pub(crate) fn with_revocation_redactor<T>(&self,operation:impl FnOnce(&dyn Fn(&str)->String)->Result<T,String>)->Result<T,String>{
        // Losing the provider credential must not prevent withdrawing authority.
        // A damaged vault suppresses the free-text rationale while the
        // independent scope store can still withdraw authority.
        let redactor=self.data_redactor();
        operation(&|text|crate::data_projection::DataRedactor::redact(&redactor,text).text)
    }

    pub fn handle(&self, action: &str, payload: Value, store: &Store) -> Result<Value,String> {
        if !payload.is_object() || payload.to_string().len() > 128 * 1024 { return Err("Solicitação de IA muito grande.".into()); }
        if work::effect_action(action){return self.handle_work(action,payload,store);}
        self.handle_inner(action,payload,store,None)
    }
    fn handle_inner(&self,action:&str,payload:Value,store:&Store,mut work:Option<&mut work::AiWork<'_>>)->Result<Value,String>{
        if investigation::supports(action){return self.investigation_handle(action,payload,store,work);}
        if planning::supports(action){return self.operation_propose(payload,store,work.ok_or("work_owned_operation")?);}
        if budget::supports(action){return self.budget_handle(action,payload,store);}
        match action {
            "ai.chat.status"|"ai.chat.stop" => self.chat.inspect_with_policy(payload,action=="ai.chat.stop",store),
            "ai.key.save" => {
                let _lease=self.inference.try_lock().map_err(|_|"Aguarde a solicitação em andamento antes de trocar a credencial.".to_string())?;
                let input: KeyInput = decode(payload)?;
                let provider=self.selected_provider(input.provider,store)?;
                if provider==ProviderId::OpenKoda{return Err("Use a sessão da conta para acessar a assinatura.".into());}
                let pasted = Zeroizing::new(input.key);
                let key = pasted.trim();
                if key.len() < 16 || key.len() > 512 || !key.bytes().all(|c| c.is_ascii_graphic()) { return Err("Informe uma chave de API válida, sem espaços.".into()); }
                let cipher = protect(key.as_bytes(), false)?;
                crate::credential_refs::ensure_provider_slot(store,provider)?;
                if input.prepare_activation {
                    // Install the guard first: if the vault write fails or the process stops,
                    // the old key keeps its exact behavior. A new revision starts pending.
                    let previous = self.revision(provider)?;
                    store.mutate(|state| {
                        state.provider_models.entry(provider).or_insert(crate::ai_provider::ProviderModelPreference {
                            credential_revision: previous, models: None,
                        });
                        Ok(())
                    })?;
                }
                let mut db = self.db.lock().map_err(failure)?;
                let tx = db.transaction().map_err(failure)?;
                if provider!=ProviderId::OpenAI {expand_provider_slots(&tx,provider)?;}
                tx.execute("INSERT INTO vault VALUES(?3,?1,?2) ON CONFLICT(id) DO UPDATE SET cipher=excluded.cipher, revision=excluded.revision", params![cipher,model::id(),provider.slot()]).map_err(failure)?;
                tx.execute("DELETE FROM connection_check WHERE id=?1", [provider.slot()]).map_err(failure)?;
                tx.commit().map_err(failure)?;
                Ok(json!({"hasApiKey":true,"verified":false}))
            }
            "ai.key.remove" => {
                let _lease=self.inference.try_lock().map_err(|_|"Aguarde a solicitação em andamento antes de remover a credencial.".to_string())?;
                let input:ProviderInput=decode(payload)?;
                let provider=self.selected_provider(input.provider,store)?;
                if provider==ProviderId::OpenKoda{return Err("Use a opção Sair na conta da assinatura.".into());}
                let mut db = self.db.lock().map_err(failure)?;
                let tx = db.transaction().map_err(failure)?;
                tx.execute("DELETE FROM vault WHERE id=?1", [provider.slot()]).map_err(failure)?;
                tx.execute("DELETE FROM connection_check WHERE id=?1", [provider.slot()]).map_err(failure)?;
                tx.commit().map_err(failure)?;
                Ok(json!({"hasApiKey":false,"verified":false}))
            }
            "ai.provider.configure" => {
                let _lease = self.inference.try_lock().map_err(|_| "Aguarde a solicitação em andamento antes de ativar os modelos.".to_string())?;
                let input: ConfigureProvider = decode(payload)?;
                let provider = input.provider;
                if provider==ProviderId::OpenKoda{return Err("Escolha o modelo da assinatura no projeto.".into());}
                if self.revision(provider)?.as_deref() != Some(input.credential_revision.as_str()) {
                    return Err("A chave mudou. Consulte os modelos novamente antes de ativar o provedor.".into());
                }
                if input.models.is_empty() || input.models.iter().any(|id| !crate::ai_provider::valid_model(id)) {
                    return Err("Selecione pelo menos um modelo válido.".into());
                }
                if input.default_model.as_ref().is_some_and(|id| !input.models.contains(id)) {
                    return Err("Escolha o modelo padrão entre os modelos selecionados.".into());
                }
                store.mutate(|state| {
                    if let Some(id) = input.default_model {
                        state.settings.provider = provider.id().into();
                        state.settings.endpoint = provider.endpoint().into();
                        state.settings.model = id;
                    } else if state.settings.provider == provider.id() && !input.models.contains(&state.settings.model) {
                        return Err("Escolha um modelo padrão entre os selecionados para este provedor.".into());
                    }
                    state.provider_models.insert(provider, crate::ai_provider::ProviderModelPreference {
                        credential_revision: Some(input.credential_revision), models: Some(input.models),
                    });
                    Ok(())
                })?;
                self.provider_status(store, Some(provider))
            }
            "ai.selection.save"=>{
                let _lease=self.inference.try_lock().map_err(|_|"Aguarde a solicitação de IA antes de alterar o modelo.")?;
                if payload.get("aiSelection").is_none(){return Err("Escolha um modelo ou o padrão da instalação.".into());}
                let input:AssessmentSelection=decode(payload)?;
                if let Some(selection)=&input.ai_selection {
                    selection.validate()?;
                    if selection.provider==ProviderId::OpenKoda {crate::credential_refs::ensure_provider_slot(store,selection.provider)?;}
                    self.require_enabled_model(&store.snapshot()?, selection.provider, &selection.model)?;
                }
                store.mutate(|state|{
                    let a=state.assessments.iter_mut().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
                    if !state.projects.iter().any(|p|p.id==a.project_id&&!p.archived){return Err("Projeto arquivado.".into());}
                    a.ai_selection=input.ai_selection;
                    Ok(json!({"assessmentId":a.id,"aiSelection":a.ai_selection}))
                })
            }
            "ai.status" => { let input:ProviderInput=decode(payload)?; self.provider_status(store,input.provider) }
            "ai.models" => {
                let input:CatalogInput=decode(payload)?;
                let provider=self.selected_provider(input.provider,store)?;
                let _work=work.as_ref().ok_or("work_owned_operation")?;
                let (key,revision) = self.provider_credential(provider)?;
                if let Some(model)=input.reasoning_model {
                    if !crate::ai_provider::valid_model(&model){return Err("Modelo inválido.".into());}
                    self.require_enabled_model(&store.snapshot()?,provider,&model)?;
                    let subscribed=reasoning::subscribed(&self.account.edge("account",json!({}))?,chrono::Utc::now());
                    let levels=if subscribed{reasoning::model_levels(provider,&model,&key)?}else{vec![]};
                    return Ok(json!({"provider":provider,"model":model,"subscribed":subscribed,"reasoningLevels":levels}));
                }
                let prices=if input.for_analysis {Some(self.budget.priced_scan_models(provider)?)}else{None};
                let models=self.provider.models(provider,&key,prices.as_ref())?;
                Ok(json!({"provider":provider,"models":models,"credentialRevision":revision,"note":"O catálogo inclui os modelos disponíveis para a chave. O teste de conexão verifica a geração de texto pelo modelo selecionado."}))
            }
            "ai.test" => {
                let input:TestInput=decode(payload)?;
                let cfg = self.config_for(store,None,true)?;
                let provider=ProviderId::parse(&cfg.provider)?;
                if provider==ProviderId::OpenKoda{return Err("A conexão da assinatura é verificada pela sessão da conta.".into());}
                let (key, revision) = self.provider_credential(provider)?;
                let answer = self.complete_locked(provider,&cfg.model, &key, &revision, "connection-test", budget::TEST_PROMPT, budget::TEST_INSTRUCTIONS, None,2048,None,json!({}),input.budget_review.as_ref(),0,None,None,work::RequestWork::Live(work.as_deref_mut().ok_or("work_owned_operation")?))?;
                // Never attach a successful result to settings or credentials changed during the request.
                let current = self.config_for(store,None,true)?;
                let (_, current_revision) = self.provider_credential(provider)?;
                if current.provider!=cfg.provider || current.model != cfg.model || current_revision != revision { return Err("A configuração mudou durante o teste. Teste a conexão novamente.".into()); }
                self.db.lock().map_err(failure)?.execute("INSERT INTO connection_check VALUES(?5,?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET model=excluded.model, endpoint=excluded.endpoint, revision=excluded.revision, checked=excluded.checked", params![cfg.model, cfg.endpoint, revision, model::now(),provider.slot()]).map_err(failure)?;
                Ok(json!({"verified":true,"model":cfg.model,"endpoint":cfg.endpoint,"capability":"text","usage":answer["usage"],"budget":answer["budget"],"checkedAt":model::now()}))
            }
            "ai.context" => if payload.get("operation").is_some(){self.prepare_ai_data(payload,store)}else{Err("data_review_required: escolha a operação e revise o pacote de dados antes de obter o contexto.".into())},
            "ai.ask" => {
                let input: AskInput = decode(payload)?;
                if !input.allow_remote { return Err("Revise e autorize o contexto antes de enviá-lo ao provedor.".into()); }
                if input.data_review.is_none(){return Err("data_review_required: revise o pacote completo antes de enviar.".into());}
                let cfg=self.config_for(store,Some(&input.assessment_id),true)?;
                let provider=ProviderId::parse(&cfg.provider)?;
                let (key,revision)=self.provider_credential(provider)?;
                let reviewed=self.reviewed_ai_data(store,input.data_review.as_ref(),&input.assessment_id,&input.evidence_ids,&input.context_target_refs,&input.message,"ask")?;
                if reviewed.packet.context["digest"]!=input.context_digest{return Err("data_review_changed: o contexto mudou.".into());}
                let scope=context_scope(&reviewed.packet.context)?;
                let (prompt,instructions,maximum)=&reviewed.packet.prompts[0];
                let answer = self.complete_locked(provider,&cfg.model, &key, &revision, &input.assessment_id, prompt,instructions, Some(&reviewed.packet.message),*maximum,None,json!({}),input.budget_review.as_ref(),0,Some((store,&scope,&input.context_target_refs)),Some(data::DataDispatch{review:&reviewed.prepared.review,spec:&reviewed.packet.spec,acknowledge_sensitive:input.acknowledge_sensitive,admitted_context:None}),work::RequestWork::Live(work.as_deref_mut().ok_or("work_owned_operation")?))?;
                Ok(answer)
            }
            "ai.history" => {
                let input: ContextInput = decode(payload)?;
                check_assessment(store, &input.assessment_id)?;
                let db = self.db.lock().map_err(failure)?;
                let mut q = db.prepare("SELECT id,role,text,created FROM messages WHERE assessment=?1 ORDER BY rowid DESC LIMIT 100").map_err(failure)?;
                let rows = q.query_map([&input.assessment_id], |r| Ok(json!({"id":r.get::<_,String>(0)?,"role":r.get::<_,String>(1)?,"text":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?}))).map_err(failure)?;
                let mut messages = rows.collect::<Result<Vec<_>,_>>().map_err(failure)?;
                messages.reverse();
                let mut q=db.prepare("SELECT id,model,status,created,input_tokens,output_tokens,details,endpoint FROM requests WHERE assessment=?1 ORDER BY rowid DESC LIMIT 100").map_err(failure)?;
                let requests=q.query_map([&input.assessment_id],|r|{
                    let details:String=r.get(6)?;let body:Value=serde_json::from_str(&details).unwrap_or(Value::Null);
                    if r.get::<_,String>(2)?=="incomplete" && is_output_limit_reason(&body["incomplete"]["reason"]) {
                        let last_id=body["chatMessageIds"].as_array().and_then(|ids|ids.last());
                        if let Some(message)=messages.iter_mut().find(|message|Some(&message["id"])==last_id){
                            message["outputLimited"]=json!(true);
                            message["outputLimitTokens"]=body["maxOutputTokens"].clone();
                        }
                    }
                    Ok(json!({"id":r.get::<_,String>(0)?,"endpoint":r.get::<_,String>(7)?,"model":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?,"inputTokens":r.get::<_,Option<i64>>(4)?,"outputTokens":r.get::<_,Option<i64>>(5)?,"unknownAcknowledged":request_unknown_acknowledged(&details),"resolution":body.get("resolution"),"operationProposal":body.get("operationProposal"),"budget":body.get("budget"),"scopeRef":body.get("scopeRef"),"contextTargetRefs":body.get("contextTargetRefs"),"dataReviewRef":body.get("dataReviewRef"),"dataUseReceiptRef":body.get("dataUseReceiptRef"),"dataPolicyRef":body.get("dataPolicyRef"),"dataBinding":body.get("dataBinding"),"providerCredentialRef":body.get("providerCredentialRef"),"chatMessageIds":body.get("chatMessageIds"),"dataReceiptStatus":if body.get("dataUseReceiptRef").is_some(){"recorded"}else{"legacy_no_data_policy_receipt"}}))
                }).map_err(failure)?.collect::<Result<Vec<_>,_>>().map_err(failure)?;
                Ok(json!({"assessmentId":input.assessment_id,"messages":messages,"requests":requests,"limit":100,"historySentToProvider":false}))
            }
            "ai.request.reconcile"=>self.reconcile_request(decode(payload)?,store),
            "candidate.record" => self.candidate(store, decode(payload)?),
            _ => Err("Ação de IA desconhecida.".into())
        }
    }

    fn reconcile_request(&self,input:RequestResolutionInput,store:&Store)->Result<Value,String>{
        check_assessment(store,&input.assessment_id)?;crate::core::valid_id(&input.request_id)?;
        let note=clean_text(&input.note,2000,"Justificativa")?;
        if !input.acknowledge_unknown_usage{return Err("Reconheça que resultado e cobrança permanecem desconhecidos; nenhuma chamada será repetida.".into());}
        let _lease=self.inference.try_lock().map_err(|_|"Aguarde a inferência ativa antes de reconciliar o histórico.")?;
        let mut db=self.db.lock().map_err(failure)?;let tx=db.transaction().map_err(failure)?;
        let row:Option<(String,String)>=tx.query_row("SELECT status,details FROM requests WHERE id=?1 AND assessment=?2",params![input.request_id,input.assessment_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(failure)?;
        let (status,raw)=row.ok_or("Solicitação não encontrada nesta avaliação.")?;
        if status!="unknown"{return Err("Somente uma solicitação com efeito desconhecido aceita esta revisão.".into());}
        let mut details:Value=serde_json::from_str(&raw).map_err(failure)?;
        if !details.is_object(){return Err("Detalhes da solicitação inválidos; histórico preservado.".into());}
        if details.get("resolution").is_some(){
            if !request_unknown_acknowledged(&raw)||details["resolution"]["note"]!=note{return Err("A revisão existente foi preservada; use a mesma justificativa para consultar o recibo.".into());}
        }else{
            details["resolution"]=json!({"id":model::id(),"recordedAt":model::now(),"author":"local-user","note":note,"acknowledgeUnknownUsage":true,"usageRemainsUnknown":true});
            tx.execute("UPDATE requests SET details=?1 WHERE id=?2",params![details.to_string(),input.request_id]).map_err(failure)?;
        }
        tx.commit().map_err(failure)?;
        Ok(json!({"id":input.request_id,"assessmentId":input.assessment_id,"status":"unknown","unknownAcknowledged":true,"resolution":details["resolution"],"repeated":false}))
    }

    fn decrypt_key(cipher:&[u8])->Result<Zeroizing<String>,String>{
        let clear=Zeroizing::new(protect(cipher,true)?);
        Ok(Zeroizing::new(String::from_utf8(clear.to_vec()).map_err(|_|"Credencial inválida no vault.")?))
    }
    fn provider_credential(&self,provider:ProviderId)->Result<(Zeroizing<String>,String),String>{
        if provider==ProviderId::OpenKoda{return self.account.credential();}
        let record:Option<(Vec<u8>,String)>=self.db.lock().map_err(failure)?.query_row(
            "SELECT cipher,revision FROM vault WHERE id=?1",[provider.slot()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(failure)?;
        let (cipher,revision)=record.ok_or("Configure uma chave para este provedor nas Configurações.")?;
        Ok((Self::decrypt_key(&cipher)?,revision))
    }
    #[cfg(test)]
    fn credential(&self)->Result<(Zeroizing<String>,String),String>{ self.provider_credential(ProviderId::OpenAI) }

    pub(crate) fn security_selection(&self,store:&Store,assessment:Option<&str>)->Result<(ProviderId,String,bool),String>{
        let cfg=self.config_for(store,assessment,false)?;
        let provider=ProviderId::parse(&cfg.provider)?;
        Ok((provider,cfg.model,self.revision(provider)?.is_some()))
    }
    pub(crate) fn security_key(&self,store:&Store,assessment:&str,expected:ProviderId,model:&str)->Result<(Zeroizing<String>,String),String>{
        let cfg=self.config_for(store,Some(assessment),true)?;
        if cfg.provider!=expected.id()||cfg.model!=model{
            return Err("O provedor ou modelo mudou. Revise a seleção desta avaliação.".into());
        }
        self.provider_credential(expected)
    }
    pub(crate) fn security_lease(&self)->Result<std::sync::MutexGuard<'_,()>,String>{
        self.inference.try_lock().map_err(|_|"Aguarde a operação de IA antes de iniciar o SDK.".into())
    }
    pub(crate) fn reserve_security(&self,id:&str,assessment:&str,provider:ProviderId,model:&str,cap:u64)->Result<(),String>{
        self.budget.reserve_security(id,assessment,provider,model,cap)
    }
    pub(crate) fn security_not_sent(&self,id:&str)->Result<(),String>{self.budget.not_sent(id)}
    pub(crate) fn security_usage(&self,id:&str,requests:u64,partial:u64,input:u64,output:u64)->Result<(),String>{
        self.budget.security_usage(id,requests,partial,input,output)
    }
    pub(crate) fn security_settle(&self,id:&str,estimate:Option<u64>)->Result<(),String>{self.budget.security_settle(id,estimate)}

    fn config_for(&self,store:&Store,assessment:Option<&str>,require_model:bool)->Result<model::Settings,String>{
        let state = store.snapshot()?;
        let settings = Self::config_in(&state,assessment,require_model)?;
        if require_model {
            self.require_enabled_model(&state, ProviderId::parse(&settings.provider)?, &settings.model)?;
        }
        Ok(settings)
    }
    fn require_enabled_model(&self, state: &model::Snapshot, provider: ProviderId, model: &str) -> Result<(), String> {
        if let Some(preference) = state.provider_models.get(&provider) {
            if !preference.permits(self.revision(provider)?.as_deref(), model) {
                return Err("O modelo escolhido está desativado para esta chave. Ative-o em IA e modelos ou selecione outro para a avaliação.".into());
            }
        }
        Ok(())
    }
    fn config_in(state:&model::Snapshot,assessment:Option<&str>,require_model:bool)->Result<model::Settings,String>{
        let mut cfg=state.settings.clone();
        if let Some(id)=assessment {
            let a=state.assessments.iter().find(|a|a.id==id).ok_or("Avaliação não encontrada.")?;
            if let Some(selection)=&a.ai_selection {
                selection.validate()?;
                cfg.provider=selection.provider.id().into();
                cfg.endpoint=selection.provider.endpoint().into();
                cfg.model=selection.model.clone();
            }
        }
        let provider=ProviderId::parse(&cfg.provider)?;
        if cfg.endpoint.trim_end_matches('/')!=provider.endpoint(){return Err("O endpoint não corresponde ao provedor selecionado.".into());}
        cfg.endpoint=provider.endpoint().into();
        if require_model&&!crate::ai_provider::valid_model(&cfg.model){return Err("Selecione e salve um modelo antes de conectar.".into());}
        Ok(cfg)
    }
    fn provider_status(&self,store:&Store,requested:Option<ProviderId>)->Result<Value,String>{
        let state=store.snapshot()?;
        let cfg=&state.settings;
        let provider=match requested{Some(p)=>p,None=>ProviderId::parse(&cfg.provider)?};
        let selected_model=if cfg.provider==provider.id(){cfg.model.as_str()}else{""};
        let revision=self.revision(provider)?;
        let enabled = state.provider_models.get(&provider).map(|pref| pref.enabled_models(revision.as_deref()));
        let db=self.db.lock().map_err(failure)?;
        let checked:Option<String>=db.query_row("SELECT checked FROM connection_check WHERE id=?1 AND model=?2 AND endpoint=?3 AND revision=?4",
            params![provider.slot(),selected_model,provider.endpoint(),revision.as_deref().unwrap_or("")],|r|r.get(0)).optional().map_err(failure)?;
        Ok(json!({"provider":provider,"hasApiKey":revision.is_some(),"credentialRevision":revision,"enabledModels":enabled,"verified":checked.is_some(),"checkedAt":checked,"endpoint":provider.endpoint(),"model":selected_model,"capability":"text","vault":"Windows DPAPI (usuário atual)","maxOutputTokens":2048,"maxRequestsPerAssessment":100}))
    }
    #[cfg(test)]
    pub(crate) fn test_context_projection(&self,store:&Store,payload:Value)->Result<Value,String>{
        // Tests of the pure internal projection remain separate from the IPC
        // authorization contract; this helper is never part of the dispatcher.
        self.context(store,decode(payload)?)
    }

    fn context(&self, store:&Store, input:ContextInput) -> Result<Value,String> {
        if input.evidence_ids.len() > 20 { return Err("Selecione até 20 evidências por envio.".into()); }
        let state = store.snapshot()?;
        let a = state.assessments.iter().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
        let combined=scope_targets::is_combined(&state,&a.id);
        let mut selected=input.context_target_refs;
        selected.sort_by(|a,b|a.entry_id.cmp(&b.entry_id));
        let mut selected_targets=vec![];
        if combined {
            if selected.is_empty()||selected.len()>8||selected.windows(2).any(|w|w[0].entry_id==w[1].entry_id){return Err("Selecione de 1 a 8 fontes distintas para o contexto da IA.".into());}
            scope::check_selected_work(&state,&a.id,AI_METHOD,None,&selected)?;
            for reference in &selected {
                let target=scope_targets::resolve_current_target(&state,&a.id,Some(reference))?;
                selected_targets.push(json!({"targetRef":reference,"kind":target.kind,"target":redact(&target.target,None),"label":redact(&target.label,None)}));
            }
            let contributors=crate::evidence_lineage::evidence_refs(store,&state,&a.id,&input.evidence_ids)?;
            if contributors.iter().any(|r|!selected.contains(r)){return Err("Uma evidência inclui fontes fora da seleção de contexto. Revise todas as origens antes do envio.".into());}
        } else if !selected.is_empty(){return Err("Esta avaliação simples não aceita referências de outro conjunto de alvos.".into());}
        let mut evidence = vec![];
        let mut source_files_included=false;
        let mut seen = std::collections::HashSet::new();
        for id in input.evidence_ids {
            if !seen.insert(id.clone()) { return Err("Evidência repetida.".into()); }
            let e = a.evidence.iter().find(|e|e.id==id).ok_or("Evidência não pertence a esta avaliação.")?;
            if digest(&e.content) != e.sha256 { return Err("Evidência alterada: contexto recusado.".into()); }
            source_files_included|=crate::source_context::is_source_capture(e,a,&state.plans);
            evidence.push(json!({"id":e.id,"name":redact(&e.name,None),"sha256":e.sha256,"content":redact(&e.content,None)}));
        }
        let mut context = if combined {
            // Aggregate coverage and the presentation anchor can name unselected
            // resources. Only explicit entries and selected evidence leave here.
            json!({"assessmentId":a.id,"objective":redact(&a.objective,None),"status":a.status,"contextTargetRefs":selected,"targets":selected_targets,"evidence":evidence})
        }else{json!({"assessmentId":a.id,"objective":redact(&a.objective,None),"target":redact(&a.target,None),"kind":a.kind,"status":a.status,"coverage":a.coverage.iter().map(|s|redact(s,None)).collect::<Vec<_>>(),"evidence":evidence})};
        // Bind consent to the reviewed revision, never to the mutable freeze event.
        // Omission preserves the exact context shape of historical assessments.
        if let Some(reference)=scope::current_ref(&state,&a.id)?{context["scopeRef"]=json!(reference);}
        fn scrub(value:&mut Value,keys:&[Zeroizing<String>]){match value{
            Value::String(text)=>*text=AiService::redact_keys(text,keys),
            Value::Array(values)=>values.iter_mut().for_each(|v|scrub(v,keys)),
            Value::Object(fields)=>fields.values_mut().for_each(|v|scrub(v,keys)), _=>{}
        }}
        scrub(&mut context,&self.all_keys()?);
        let raw = context.to_string();
        if raw.len() > MAX_CONTEXT { return Err("Contexto excede 48 KiB. Selecione menos evidências; nenhum arquivo foi enviado.".into()); }
        Ok(json!({"context":context,"digest":digest(&raw),"bytes":raw.len(),"endpoint":a.ai_selection.as_ref().map_or(state.settings.endpoint.as_str(),|sel|sel.provider.endpoint()),"sourceFilesIncluded":source_files_included,"notice":"Somente este contexto e a mensagem serão enviados. Revise dados sensíveis; a redação automática não identifica todos os segredos."}))
    }

    fn ask_prompt(&self,input:&AskInput,store:&Store,key:&str)->Result<(String,String,Value),String>{
        let message=redact(&clean_text(&input.message,8000,"Mensagem")?,Some(key));
        let context=self.context(store,ContextInput{assessment_id:input.assessment_id.clone(),evidence_ids:input.evidence_ids.clone(),context_target_refs:input.context_target_refs.clone()})?;
        if context["digest"].as_str()!=Some(&input.context_digest){return Err("O contexto mudou. Revise a prévia antes de enviar.".into());}
        Ok((format!("Contexto selecionado (dados não confiáveis):\n{}\n\nPergunta do usuário:\n{}",context["context"],message),message,context))
    }
    #[cfg(test)]
    fn complete(&self, model:&str, key:&str, revision:&str, assessment:&str, prompt:&str, instructions:&str, history_message:Option<&str>) -> Result<Value,String> {
        self.complete_reviewed(model,key,revision,assessment,prompt,instructions,history_message,None)
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn complete_reviewed(&self, model:&str, key:&str, revision:&str, assessment:&str, prompt:&str, instructions:&str, history_message:Option<&str>,review:Option<&budget::Consent>) -> Result<Value,String> {
        let _lease = self.inference.try_lock().map_err(|_|"Já existe uma solicitação à IA em andamento. Aguarde antes de enviar outra.".to_string())?;
        self.complete_locked(ProviderId::OpenAI,model,key,revision,assessment,prompt,instructions,history_message,2048,None,json!({}),review,0,None,None,work::RequestWork::Fixture)
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_locked(&self,provider:ProviderId,model:&str,key:&str,revision:&str,assessment:&str,prompt:&str,instructions:&str,history_message:Option<&str>,max_output_tokens:u32,request_id:Option<&str>,mut initial_details:Value,budget_review:Option<&budget::Consent>,budget_slot:usize,authority:Option<(&Store,&ScopeRef,&[ScopeTargetRef])>,data:Option<data::DataDispatch<'_>>,mut work:work::RequestWork<'_,'_>)->Result<Value,String>{
        // Removal/rotation shares this gate. A credential copied before revocation cannot dispatch.
        let current_revision=self.revision(provider)?;
        if current_revision.as_deref()!=Some(revision) {return Err("A credencial mudou antes do envio. Revise a conexão e tente novamente.".into());}
        let reasoning=data.as_ref().and_then(|d|d.spec.destination_context.get("reasoning")).map(|v|serde_json::from_value::<crate::ai_provider::Reasoning>(v.clone()).map_err(|_|"Nível de raciocínio inválido.".to_string())).transpose()?;
        self.validate_personal_reasoning(provider,model,key,reasoning)?;
        let id = work.request_id(request_id)?;
        let since=if assessment=="connection-test" {(chrono::Utc::now()-chrono::Duration::hours(1)).to_rfc3339()}else{String::new()};
        let count:i64 = self.db.lock().map_err(failure)?.query_row("SELECT COUNT(*) FROM requests WHERE assessment=?1 AND created>=?2",params![assessment,since],|r|r.get(0)).map_err(failure)?;
        if count >= if assessment=="connection-test" {20}else{100} { return Err(if assessment=="connection-test" {"Limite de 20 testes de conexão por hora atingido. Aguarde antes de tentar novamente."}else{"Limite de 100 solicitações por avaliação atingido. Revise o consumo antes de abrir outra avaliação."}.into()); }
        if Self::redact_keys(model,&self.all_keys()?)!=model{return Err("Identificador do modelo contém dado sensível.".into());}
        let operation=if assessment=="connection-test"{"test"}else if initial_details.get("investigationId").is_some(){"investigation"}else if initial_details.get("operationProposal").is_some(){"operation_proposal"}else{"ask"};
        if assessment!="connection-test"&&authority.is_none(){return Err("scope_unreviewed: revise o escopo antes de enviar ao provedor.".into());}
        if assessment!="connection-test"&&data.is_none(){return Err("data_review_required: revise o pacote completo antes de enviar.".into());}
        if let Some((store,reference,targets))=authority{
            if reference.assessment_id!=assessment{return Err("scope_state_changed: referência fora da avaliação.".into());}
            scope::recheck_selected(store,reference,AI_METHOD,targets)?;
            initial_details["scopeRef"]=json!(reference);
            if !targets.is_empty(){initial_details["contextTargetRefs"]=json!(targets);}
        }
        initial_details["maxOutputTokens"]=json!(max_output_tokens);
        let mut spec=budget::Spec::for_provider(provider,operation,assessment,model,revision,prompt,instructions,max_output_tokens)?;
        if let Some(data)=&data{
            let index=data.admitted_context.map(|(_,i)|i).unwrap_or(0);
            let request=data.spec.requests.iter().find(|r|r.index==index).ok_or("data_review_changed: perspectiva fora do pacote.")?;
            if request.output_sha256!=digest(prompt)||request.output_bytes!=prompt.len() as u64||data.spec.credential.as_ref().is_none_or(|c|c.revision!=revision)||data.spec.destination_context["model"]!=model||data.spec.destination_context["endpoint"]!=provider.endpoint(){return Err("data_review_changed: pacote ou credencial mudou antes da reserva.".into());}
            spec=spec.bind_data(&data.review.digest)?;
        }
        // A missing or stale financial confirmation is a pure rejection. It
        // must not consume the independent data review before the user can
        // finish reviewing the estimate. Reservation still rechecks atomically.
        if data.as_ref().is_some_and(|d|d.admitted_context.is_none()) {self.budget.preflight(std::slice::from_ref(&spec),budget_review)?;}
        work.intent(&id)?;
        let mut persist=|admission:Option<&crate::data_policy::DataAdmission>|->Result<(),String>{
            if let Some(admission)=admission{
                initial_details["dataReviewRef"]=json!(admission.review_ref);
                initial_details["dataUseReceiptRef"]=json!(admission.receipt_ref);
                initial_details["dataPolicyRef"]=json!(admission.policy_versions);
                if let Some(data)=&data{initial_details["dataBinding"]=json!({"selection":data.spec.selection,"subjects":data.spec.subjects,"humanText":data.spec.human_text});initial_details["providerCredentialRef"]=json!(data.spec.credential);}
            }
            initial_details["budget"]=self.budget.reserve(&id,&spec,budget_review,budget_slot)?;
            let intent=self.db.lock().map_err(failure)?.execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created,details) VALUES(?1,?2,?3,?4,?5,?6,'pending',?7,?8)",params![id,assessment,provider.endpoint(),model,revision,digest(prompt),model::now(),initial_details.to_string()]).map_err(failure);
            if let Err(error)=intent{let _=self.budget.not_sent(&id);return Err(error);}Ok(())
        };
        let receipt=if let (Some((store,reference,targets)),Some(data))=(authority,data.as_ref()){
            let operational=crate::data_policy::OperationalScope{assessment_id:assessment,method:AI_METHOD,expected:reference,targets};
            let effect=crate::data_policy::EffectLink{kind:"request".into(),id:id.clone()};
            let admitted=if let Some((context,index))=data.admitted_context{self.data_policy.with_admitted_context(store,context,index,data.spec,operational,effect,|a|{persist(Some(a))?;Ok(a.receipt_ref.clone())})?}else{self.data_policy.with_admission(store,data.review,data.spec,Some(operational),effect,data.acknowledge_sensitive,|a|{persist(Some(a))?;Ok(a.receipt_ref.clone())})?};
            match admitted{crate::data_policy::AdmissionResult::Admitted(reference)=>Some(reference),crate::data_policy::AdmissionResult::AlreadyUsed(_)=>return Err("data_review_used: revisão já consumida; consulte o histórico sem repetir.".into())}
        }else{persist(None)?;None};
        // Last local checkpoint. Once HTTP starts, revocation cannot prove that
        // the provider stopped or refunded a request; its receipt is retained.
        let mut admitted_message_id=None;
        let mut dispatch=||->Result<(),String>{
            work.bind(self,&id,initial_details.get("investigationId").and_then(Value::as_str))?;
            if let (Some((store,reference,targets)),Some(data),Some(receipt))=(authority,data.as_ref(),receipt.as_ref()){
                let identity=self.identity_for(store,Some(assessment))?;
                if !identity.available||identity.revision!=revision||data.spec.credential.as_ref().is_none_or(|c|c.slot_id!=identity.slot_id){return Err("credential_reference_changed: conexão mudou antes do envio.".into());}
                self.data_policy.before_dispatch(store,receipt,data.spec,Some(crate::data_policy::OperationalScope{assessment_id:assessment,method:AI_METHOD,expected:reference,targets}),||{work.before_dispatch(store)?;admitted_message_id=self.record_chat_question(&id,assessment,history_message)?;self.budget.dispatched(&id)})
            }else{admitted_message_id=self.record_chat_question(&id,assessment,history_message)?;self.budget.dispatched(&id)}
        };
        if let Err(error)=dispatch(){
            let _=self.budget.not_sent(&id);
            self.db.lock().map_err(failure)?.execute("UPDATE requests SET status='failed',input_tokens=0,output_tokens=0,details=json_set(details,'$.notDispatched',json('true'),'$.endedAt',?1) WHERE id=?2",params![model::now(),id]).map_err(failure)?;
            if let (Some((store,_,_)),Some(receipt))=(authority,receipt.as_ref()){self.data_policy.finish(store,receipt,crate::data_policy::DataUseOutcome::NotSent,vec![])?;}
            return Err(error);
        }
        let mut interrupted_generation = None;
        let raw_result=if matches!(operation,"ask"|"operation_proposal"){
            self.chat.start(assessment,&id,data.as_ref().map(|data|data.review)).and_then(|live|{
                live.admit(admitted_message_id.clone())?;
                let result=if provider==ProviderId::OpenKoda {
                    chat::commercial_chat(&id,model,key,prompt,instructions,max_output_tokens,
                        data.as_ref().and_then(|d|d.spec.destination_context["reasoning"].as_str()),&live.token,live.progress.clone())
                }else{self.provider.reasoning_chat_response(provider,model,key,prompt,instructions,max_output_tokens,reasoning,&live.token,live.progress.clone())};
                if result.is_err() {
                    interrupted_generation=live.progress.lock().map_err(failure)?.generation_id.clone();
                }
                result
            })
        }else if provider==ProviderId::OpenKoda {
            let progress=Arc::new(Mutex::new(chat::Progress::default()));
            chat::commercial_chat(&id,model,key,prompt,instructions,max_output_tokens,
                data.as_ref().and_then(|d|d.spec.destination_context["reasoning"].as_str()),&std::sync::atomic::AtomicBool::new(false),progress)
        }else{self.provider.reasoning_response(provider,model,key,prompt,instructions,max_output_tokens,reasoning)};
        let result=self.commit_response(&id,assessment,key,history_message,interrupted_generation.as_deref(),raw_result);
        if let (Some((store,_,_)),Some(receipt))=(authority,receipt.as_ref()){
            let status:String=self.db.lock().map_err(failure)?.query_row("SELECT status FROM requests WHERE id=?1",[&id],|r|r.get(0)).map_err(failure)?;
            let outcome=match status.as_str(){
                "completed"=>crate::data_policy::DataUseOutcome::Completed,
                "incomplete" if result.is_ok()=>crate::data_policy::DataUseOutcome::Completed,
                "unknown"|"pending"=>crate::data_policy::DataUseOutcome::Unknown,
                _=>crate::data_policy::DataUseOutcome::Failed
            };
            self.data_policy.finish(store,receipt,outcome,vec![])?;
        }
        result.map(|mut result|{result["requestId"]=json!(id);if let Some(receipt)=receipt{result["dataUseReceiptRef"]=json!(receipt);}result})
    }

    // The accepted user turn survives provider errors, process interruption and
    // closing the window. Its request identity also prevents an accidental replay.
    fn record_chat_question(&self,id:&str,assessment:&str,question:Option<&str>)->Result<Option<String>,String>{
        let Some(question)=question else{return Ok(None)};
        let message_id=scope_targets::derived_id("openkoda.chat.message.v1",&[id,"user"]);
        let mut db=self.db.lock().map_err(failure)?;
        let tx=db.transaction().map_err(failure)?;
        tx.execute("INSERT OR IGNORE INTO messages VALUES(?1,?2,'user',?3,?4)",params![message_id,assessment,question,model::now()]).map_err(failure)?;
        let (stored_assessment,text):(String,String)=tx.query_row("SELECT assessment,text FROM messages WHERE id=?1",[&message_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(failure)?;
        if stored_assessment!=assessment||text!=question{return Err("A identidade da mensagem mudou.".into());}
        let updated=tx.execute("UPDATE requests SET details=json_set(details,'$.chatMessageIds',json(?2)) WHERE id=?1",params![id,json!([message_id]).to_string()]).map_err(failure)?;
        if updated!=1{return Err("Solicitação da conversa indisponível.".into());}
        tx.commit().map_err(failure)?;Ok(Some(message_id))
    }

    fn commit_response(&self,id:&str,assessment:&str,key:&str,history_message:Option<&str>,interrupted_generation:Option<&str>,raw_result:Result<Value,String>)->Result<Value,String> {
        // A provider can consume/bill tokens even when no complete visible answer exists.
        // A transport timeout does not prove the provider canceled the request.
        let (input,output)=raw_result.as_ref().ok().map(|v|(v["usage"]["input_tokens"].as_i64(),v["usage"]["output_tokens"].as_i64())).unwrap_or((None,None));
        let accounting=self.budget.settle(id,input,output);
        let mut details=raw_result.as_ref().ok().map(|v|json!({"responseId":v["id"],"providerStatus":v["status"],"incomplete":v["incomplete_details"],"endedAt":model::now()})).unwrap_or(json!({"endedAt":model::now(),"usageUnknown":true}));
        if let Some(id)=interrupted_generation { details["responseId"]=json!(id); }
        let provider_incomplete=raw_result.as_ref().ok().is_some_and(|v|v["status"]=="incomplete");
        let provider_not_terminal=raw_result.as_ref().ok().is_some_and(|v|!matches!(v["status"].as_str(),Some("completed"|"incomplete"|"failed"|"cancelled")));
        let transport_unknown=raw_result.as_ref().err().is_some_and(|e:&String|e.starts_with("Falha de conexão")||e.starts_with("Resposta"));
        let output_limited=history_message.is_some() && raw_result.as_ref().is_ok_and(output_limit_reached);
        let result=raw_result.and_then(|body|{
            if output_limited {completion_text(&body,key)} else {parse_completion(body,key)}
        }).and_then(|mut answer|{
            let keys=self.all_keys()?;
            if let Some(text)=answer["text"].as_str(){answer["text"]=json!(Self::redact_keys(text,&keys));}
            Ok(answer)
        });
        let status=if provider_incomplete{"incomplete"}else if result.is_ok(){"completed"}else if transport_unknown||provider_not_terminal{"unknown"}else{"failed"};
        let mut db=self.db.lock().map_err(failure)?;
        let tx=db.transaction().map_err(failure)?;
        let original:String=tx.query_row("SELECT details FROM requests WHERE id=?1",[id],|row|row.get(0)).map_err(failure)?;
        let original:Value=serde_json::from_str(&original).map_err(failure)?;
        for field in ["investigationId","perspective","maxOutputTokens","operationProposal","budget","scopeRef","contextTargetRefs","dataPolicyRef","dataReviewRef","dataUseReceiptRef","dataBinding","providerCredentialRef","chatMessageIds"]{if let Some(value)=original.get(field){details[field]=value.clone();}}
        if let Ok(Some(budget))=&accounting{details["budget"]=budget.clone();}
        tx.execute("UPDATE requests SET status=?1,input_tokens=?2,output_tokens=?3,details=?4 WHERE id=?5",params![status,input,output,details.to_string(),id]).map_err(failure)?;
        if let (Some(question),Ok(answer))=(history_message,&result) {
            let answer=answer["text"].as_str().ok_or("Resposta sem texto.")?;
            let mut message_ids=Vec::new();
            for (role,text) in [("user",question),("assistant",answer)] {
                let message_id=scope_targets::derived_id("openkoda.chat.message.v1",&[id,role]);
                tx.execute("INSERT OR IGNORE INTO messages VALUES(?1,?2,?3,?4,?5)",params![message_id,assessment,role,text,model::now()]).map_err(failure)?;
                let stored:(String,String,String)=tx.query_row("SELECT assessment,role,text FROM messages WHERE id=?1",[&message_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(failure)?;
                if stored!=(assessment.into(),role.into(),text.into()){return Err("O conteúdo da mensagem mudou.".into());}
                message_ids.push(message_id);
            }
            tx.execute("UPDATE requests SET details=json_set(details,'$.chatMessageIds',json(?2)) WHERE id=?1",params![id,json!(message_ids).to_string()]).map_err(failure)?;
        }
        tx.commit().map_err(failure)?;
        let budget=accounting?;
        result.map(|mut answer|{if let Some(budget)=budget{answer["budget"]=budget;}answer})
    }

    fn candidate(&self, store:&Store, input:CandidateInput) -> Result<Value,String> {
        let title=clean_text(&input.title,160,"Título")?;
        let description=clean_text(&input.description,8000,"Descrição")?;
        if !["low","medium","high","critical"].contains(&input.severity.as_str()) || input.evidence_ids.is_empty() || input.evidence_ids.len()>20 || input.recommendation.len()>8000 { return Err("Candidato exige severidade e de 1 a 20 evidências.".into()); }
        let mut evidence_ids=input.evidence_ids;evidence_ids.sort();
        if evidence_ids.windows(2).any(|pair|pair[0]==pair[1]){return Err("Evidência repetida no candidato.".into());}
        store.mutate(|s| {
            let a=s.assessments.iter().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
            for id in &evidence_ids { if !a.evidence.iter().any(|e| &e.id==id && digest(&e.content)==e.sha256) { return Err("Evidência inválida ou fora desta avaliação.".into()); } }
            if let Some(existing)=s.findings.iter().find(|f|{
                let mut previous=f.evidence_ids.clone();previous.sort();
                f.assessment_id==input.assessment_id&&f.title==title&&f.description==description&&f.severity==input.severity&&f.recommendation==input.recommendation&&previous==evidence_ids
            }){return serde_json::to_value(existing).map_err(failure);}
            scope::freeze_review(s,&input.assessment_id,"candidate.record")?;
            let f=Finding {id:model::id(),assessment_id:input.assessment_id,title,description,severity:input.severity,confidence:"A validar".into(),status:"candidate".into(),recommendation:input.recommendation,evidence_ids,created_at:model::now()};
            s.findings.push(f.clone());
            serde_json::to_value(f).map_err(failure)
        })
    }
}

fn context_scope(context:&Value)->Result<ScopeRef,String>{serde_json::from_value(context["context"]["scopeRef"].clone()).map_err(|_|"scope_unreviewed: revise o escopo antes de preparar o envio.".into())}
fn empty(v:&Value)->Result<(),String> { if v.as_object().is_some_and(|v|v.is_empty()) {Ok(())} else {Err("Esta ação não recebe campos.".into())} }
fn check_assessment(store:&Store,id:&str)->Result<(),String> { if store.snapshot()?.assessments.iter().any(|a|a.id==id) {Ok(())} else {Err("Avaliação não encontrada.".into())} }
fn clean_text(s:&str,max:usize,label:&str)->Result<String,String> { let s=s.trim(); if s.is_empty() || s.len()>max || s.contains('\0') {Err(format!("{label} vazio ou muito longo."))} else {Ok(s.into())} }
fn client()->Result<Client,String> { Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).no_proxy().timeout(Duration::from_secs(90)).connect_timeout(Duration::from_secs(15)).build().map_err(|_|"Não foi possível preparar a conexão HTTPS.".into()) }
fn network_error(_:reqwest::Error)->String { "Falha de conexão com o provedor de IA. Confira a rede e tente novamente; nenhuma solicitação será repetida automaticamente.".into() }
fn read_bounded_response(mut response:reqwest::blocking::Response,limit:u64)->Result<Value,String> {
    let status=response.status();
    if !status.is_success() { return Err(match status.as_u16() {401=>"O provedor recusou a credencial.",403=>"A credencial não tem acesso ao recurso.",429=>"Limite ou saldo da API atingido; confira sua conta.",_=>"O provedor não concluiu a solicitação."}.into()); }
    let mut raw=String::new();
    (&mut response).take(limit+1).read_to_string(&mut raw).map_err(|_|"Resposta inválida do provedor.")?;
    if raw.len() as u64>limit {return Err("Resposta excedeu o limite de tamanho.".into());}
    serde_json::from_str(&raw).map_err(|_|"Resposta do provedor não é JSON válido.".into())
}
fn parse_completion(body:Value,key:&str)->Result<Value,String> {
    if body["status"]!="completed" {return Err("A resposta ficou incompleta ou foi interrompida. Nenhuma conclusão foi registrada.".into());}
    completion_text(&body,key)
}
fn output_limit_reached(body:&Value)->bool{
    body["status"]=="incomplete" && is_output_limit_reason(&body["incomplete_details"]["reason"])
}
fn is_output_limit_reason(reason:&Value)->bool{
    matches!(reason.as_str(),Some("length"|"max_tokens"|"max_output_tokens"))
}
fn completion_text(body:&Value,key:&str)->Result<Value,String>{
    let mut output=String::new();
    for item in body["output"].as_array().ok_or("Resposta sem conteúdo.")? {
        if item["type"]=="message" { if let Some(content)=item["content"].as_array() {for block in content {if block["type"]=="output_text" {if let Some(text)=block["text"].as_str() {output.push_str(text);output.push('\n');}}}} }
    }
    if output.trim().is_empty() {
        return Err(if output_limit_reached(body){"O modelo atingiu o limite de saída antes de produzir texto visível."}else{"O modelo não retornou texto utilizável. Nenhuma ferramenta foi executada."}.into());
    }
    Ok(json!({"text":redact(output.trim(),Some(key)),"usage":body["usage"],"responseId":body["id"],"model":body["model"],"status":body["status"],"toolsExecuted":false}))
}

/// Conservative derivative redaction. Raw evidence remains intact in the assessment store.
pub fn redact(text:&str, key:Option<&str>)->String {
    let mut result=if let Some(key)=key.filter(|k|!k.is_empty()) {text.replace(key,"[REDACTED]")} else {text.to_owned()};
    let mut private_key=false;
    result=result.lines().map(|line| {
        let lower=line.to_ascii_lowercase();
        if line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {private_key=true;}
        if private_key {
            if line.contains("-----END ") && line.contains("PRIVATE KEY-----") {private_key=false;}
            return "[REDACTED: chave privada]".into();
        }
        let normalized=lower.replace(['\"','\'', ' '], "");
        if ["authorization:","cookie:","set-cookie:","api_key=","api_key:","apikey:","password=","password:","token=","token:","secret=","secret:"].iter().any(|s|normalized.contains(s)) || line.split(|c:char|c.is_whitespace()||['\"','\'',':','=','(',')'].contains(&c)).any(|w| (w.starts_with("sk-")&&w.len()>16) || w.starts_with("ghp_") || w.starts_with("github_pat_")) {"[REDACTED: dado sensível]".into()} else {line.to_owned()}
    }).collect::<Vec<String>>().join("\n");
    result
}

#[cfg(windows)]
pub(crate) fn protect(bytes:&[u8], decrypt:bool)->Result<Vec<u8>,String> {
    use windows_sys::Win32::{Security::Cryptography::{CryptProtectData,CryptUnprotectData,CRYPT_INTEGER_BLOB,CRYPTPROTECT_UI_FORBIDDEN},Foundation::LocalFree};
    let input=CRYPT_INTEGER_BLOB {cbData:bytes.len() as u32,pbData:bytes.as_ptr() as *mut u8};
    let mut output=CRYPT_INTEGER_BLOB {cbData:0,pbData:std::ptr::null_mut()};
    // Buffers remain valid for the call; DPAPI owns output, released with LocalFree after copying.
    let ok=unsafe {if decrypt {CryptUnprotectData(&input,std::ptr::null_mut(),std::ptr::null(),std::ptr::null(),std::ptr::null(),CRYPTPROTECT_UI_FORBIDDEN,&mut output)} else {CryptProtectData(&input,std::ptr::null(),std::ptr::null(),std::ptr::null(),std::ptr::null(),CRYPTPROTECT_UI_FORBIDDEN,&mut output)}};
    if ok==0 {return Err("O vault do Windows recusou a credencial. Configure novamente neste usuário do Windows.".into());}
    let value=unsafe {std::slice::from_raw_parts(output.pbData,output.cbData as usize).to_vec()};
    unsafe { if decrypt {for i in 0..output.cbData as usize {std::ptr::write_volatile(output.pbData.add(i),0);}} LocalFree(output.pbData.cast()); }
    Ok(value)
}
#[cfg(not(windows))]
pub(crate) fn protect(_: &[u8], _:bool)->Result<Vec<u8>,String> {Err("Vault DPAPI disponível somente no Windows.".into())}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn unknown_request_review_keeps_unknown_usage_and_never_replays_provider(){
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().to_owned()).unwrap();let ai=AiService::new(dir.path().to_owned()).unwrap();
        let p=store.handle(crate::core::Request{action:"project.create".into(),payload:json!({"name":"Synthetic request","kind":"web","target":"https://example.test"})}).unwrap();
        let a=store.handle(crate::core::Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"kind":"web","target":p["target"],"title":"Fixture","objective":"Only history"})}).unwrap();
        let id=model::id();ai.db.lock().unwrap().execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created,details) VALUES(?1,?2,?3,'fixture','fixture','fixture','unknown',?4,'{}')",params![id,a["id"].as_str().unwrap(),ENDPOINT,model::now()]).unwrap();
        let input=json!({"assessmentId":a["id"],"requestId":id,"note":"Reviewed provider dashboard; billing remains unknown.","acknowledgeUnknownUsage":true});
        let mut denied=input.clone();denied["acknowledgeUnknownUsage"]=json!(false);assert!(ai.handle("ai.request.reconcile",denied,&store).is_err());
        let mut crossed=input.clone();crossed["assessmentId"]=json!(model::id());assert!(ai.handle("ai.request.reconcile",crossed,&store).is_err());
        let lease=ai.inference.lock().unwrap();assert!(ai.handle("ai.request.reconcile",input.clone(),&store).is_err());drop(lease);
        let result=ai.handle("ai.request.reconcile",input.clone(),&store).unwrap();assert_eq!(result["status"],"unknown");assert_eq!(result["repeated"],false);
        assert_eq!(ai.handle("ai.request.reconcile",input.clone(),&store).unwrap(),result);
        let mut changed=input;changed["note"]=json!("Another note");assert!(ai.handle("ai.request.reconcile",changed,&store).is_err());
        let history=ai.handle("ai.history",json!({"assessmentId":a["id"]}),&store).unwrap();assert_eq!(history["requests"].as_array().unwrap().len(),1);assert_eq!(history["requests"][0]["unknownAcknowledged"],true);assert!(history["requests"][0]["inputTokens"].is_null());assert!(history["messages"].as_array().unwrap().is_empty());
        drop(ai);let ai=AiService::new(dir.path().to_owned()).unwrap();let db=ai.db.lock().unwrap();
        let (status,raw,count):(String,String,i64)=db.query_row("SELECT status,details,(SELECT count(*) FROM requests) FROM requests WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();assert_eq!(status,"unknown");assert_eq!(count,1);assert!(request_unknown_acknowledged(&raw));
    }
    #[test] fn unknown_usage_acknowledgment_requires_complete_receipt(){
        for raw in ["{}","null","{\"resolution\":{\"acknowledgeUnknownUsage\":true}}","not-json"]{assert!(!request_unknown_acknowledged(raw));}
    }
    #[test] fn incomplete_is_not_success() {assert!(parse_completion(json!({"status":"incomplete","output":[]}),"sentinel").is_err());}
    #[test] fn model_function_call_cannot_execute() {assert!(parse_completion(json!({"status":"completed","output":[{"type":"function_call","name":"approval.accept"}]}),"sentinel").is_err());}
    #[test] fn secrets_are_redacted_from_derivatives() {assert_eq!(redact("normal\nAuthorization: Bearer secret\nvalue sentinel",Some("sentinel")),"normal\n[REDACTED: dado sensível]\nvalue [REDACTED]");}
    #[test] fn private_key_body_and_json_secrets_are_redacted() {let r=redact("-----BEGIN RSA PRIVATE KEY-----\nbase64-secret\n-----END RSA PRIVATE KEY-----\n{\"password\": \"sentinel\"}\nnormal",None);assert!(!r.contains("base64-secret"));assert!(!r.contains("sentinel"));assert!(r.ends_with("normal"));}
    #[test] fn output_preserves_text_but_not_key() {let result=parse_completion(json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"test-key"}]}]}),"test-key").unwrap();assert_eq!(result["text"],"[REDACTED]");assert_eq!(result["toolsExecuted"],false);}
    #[test] fn cross_assessment_context_is_rejected() {
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().to_owned()).unwrap();let ai=AiService::new(dir.path().to_owned()).unwrap();
        assert!(ai.context(&store,ContextInput{assessment_id:model::id(),evidence_ids:vec![],context_target_refs:vec![]}).is_err());
        assert!(ai.handle("ai.key.save",json!({"key":"short"}),&store).is_err());
        assert!(ai.handle("ai.models",json!({"endpoint":"https://example.org"}),&store).is_err());
    }
    #[test] fn revoked_credential_cannot_dispatch_or_create_request() {
        let dir=tempfile::tempdir().unwrap();let ai=AiService::new(dir.path().to_owned()).unwrap();
        assert!(ai.complete("fixture","sentinel","revoked","connection-test","test","test",None).unwrap_err().contains("credencial mudou"));
        let count:i64=ai.db.lock().unwrap().query_row("SELECT COUNT(*) FROM requests",[],|r|r.get(0)).unwrap();assert_eq!(count,0);
    }
    fn pending(ai:&AiService,id:&str) {
        ai.db.lock().unwrap().execute("INSERT INTO requests(id,assessment,endpoint,model,revision,context_hash,status,created) VALUES(?1,'fixture',?2,'fixture','fixture','fixture','pending',?3)",params![id,ENDPOINT,model::now()]).unwrap();
    }
    #[test] fn incomplete_usage_and_uncertain_transport_are_durable() {
        let dir=tempfile::tempdir().unwrap();let ai=AiService::new(dir.path().to_owned()).unwrap();pending(&ai,"incomplete");pending(&ai,"network");
        assert!(ai.commit_response("incomplete","fixture","sentinel",None,None,Ok(json!({"id":"fixture-response","status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"usage":{"input_tokens":42,"output_tokens":2048}}))).is_err());
        assert!(ai.commit_response("network","fixture","sentinel",None,Some("gen-123-fixture"),Err("Falha de conexão com o provedor de IA.".into())).is_err());
        let db=ai.db.lock().unwrap();
        let row:(String,i64,i64)=db.query_row("SELECT status,input_tokens,output_tokens FROM requests WHERE id='incomplete'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();assert_eq!(row,("incomplete".into(),42,2048));
        let interrupted:(String,String,bool,Option<i64>,Option<i64>)=db.query_row("SELECT status,json_extract(details,'$.responseId'),json_extract(details,'$.usageUnknown'),input_tokens,output_tokens FROM requests WHERE id='network'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
        assert_eq!(interrupted,("unknown".into(),"gen-123-fixture".into(),true,None,None));
    }
    #[test] fn failed_history_commit_cannot_claim_completed_request() {
        let dir=tempfile::tempdir().unwrap();let ai=AiService::new(dir.path().to_owned()).unwrap();pending(&ai,"atomic");
        ai.db.lock().unwrap().execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON messages BEGIN SELECT RAISE(ABORT,'fixture full disk'); END;").unwrap();
        let body=json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"fixture answer"}]}],"usage":{"input_tokens":1,"output_tokens":2}});
        assert!(ai.commit_response("atomic","fixture","sentinel",Some("fixture question"),None,Ok(body)).is_err());
        assert_eq!(ai.db.lock().unwrap().query_row("SELECT status FROM requests WHERE id='atomic'",[],|r|r.get::<_,String>(0)).unwrap(),"pending");
    }
    #[cfg(windows)]
    #[test] fn dpapi_roundtrip_and_tamper_rejection() {
        let key=b"sentinel-test-key-never-a-real-credential";let mut cipher=protect(key,false).unwrap();assert!(!cipher.windows(key.len()).any(|v|v==key));assert_eq!(protect(&cipher,true).unwrap(),key);let n=cipher.len()-1;cipher[n]^=1;assert!(protect(&cipher,true).is_err());
    }
}
