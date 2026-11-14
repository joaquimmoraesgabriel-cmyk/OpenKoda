use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId { OpenAI, OpenRouter, Groq, Anthropic, Gemini, DeepSeek, Xai, OpenKoda }

impl ProviderId {
    pub const ALL: [Self; 7] = [Self::OpenAI, Self::OpenRouter, Self::Groq, Self::Anthropic, Self::Gemini, Self::DeepSeek, Self::Xai];
    pub fn parse(id: &str) -> Result<Self, String> {
        Self::ALL.into_iter().chain([Self::OpenKoda]).find(|p| p.id() == id)
            .ok_or_else(|| "Provedor de IA não suportado.".into())
    }
    pub fn from_endpoint(endpoint: &str) -> Result<Self, String> {
        Self::ALL.into_iter().chain([Self::OpenKoda]).find(|p| p.endpoint() == endpoint)
            .ok_or_else(|| "O destino da conexão não corresponde a um provedor suportado.".into())
    }
    pub fn id(self) -> &'static str { match self {
        Self::OpenAI => "openai", Self::OpenRouter => "openrouter", Self::Groq => "groq",
        Self::Anthropic => "anthropic", Self::Gemini => "gemini", Self::DeepSeek => "deepseek", Self::Xai => "xai", Self::OpenKoda => "openkoda",
    }}
    pub fn slot(self) -> i64 { match self { Self::OpenAI => 1, Self::OpenRouter => 2, Self::Groq => 3,
        Self::Anthropic => 4, Self::Gemini => 5, Self::DeepSeek => 6, Self::Xai => 7, Self::OpenKoda => 8 } }
    pub fn endpoint(self) -> &'static str { match self {
        Self::OpenAI => "https://api.openai.com/v1",
        Self::OpenRouter => "https://openrouter.ai/api/v1",
        Self::Groq => "https://api.groq.com/openai/v1",
        Self::Anthropic => "https://api.anthropic.com/v1",
        Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
        Self::DeepSeek => "https://api.deepseek.com",
        Self::Xai => "https://api.x.ai/v1",
        Self::OpenKoda => crate::commercial::EDGE,
    }}
    pub fn catalog_url(self) -> String {
        if self == Self::Gemini { return "https://generativelanguage.googleapis.com/v1beta/models".into(); }
        format!("{}{}", self.endpoint(), if self == Self::OpenRouter { "/models/user?output_modalities=all" } else { "/models" })
    }
    pub fn catalog_page(self, body: &Value) -> Result<(Vec<String>, Option<String>), String> {
        if self == Self::Gemini {
            let mut names = Vec::new();
            for row in body["models"].as_array().ok_or("Catálogo de modelos inválido.")? {
                let name = row["name"].as_str().and_then(|id| id.strip_prefix("models/"))
                    .filter(|id| valid_model(id)).ok_or("Nome de modelo Gemini inválido.")?;
                names.push(name.to_owned());
            }
            let next = match body.get("nextPageToken") {
                None | Some(Value::Null) => None,
                Some(Value::String(token)) if token.is_empty() => None,
                Some(Value::String(token)) if token.len() <= 4096 && token.bytes().all(|b| b.is_ascii_graphic()) => Some(token.clone()),
                _ => return Err("Cursor de modelos Gemini inválido.".into()),
            };
            return Ok((names, next));
        }
        let next = if self == Self::Anthropic && body["has_more"].as_bool().ok_or("Paginação Anthropic inválida.")? {
            let id = body["last_id"].as_str().filter(|id| valid_model(id)).ok_or("Cursor de modelos Anthropic inválido.")?;
            Some(id.to_owned())
        } else { None };
        Ok((catalog(body)?, next))
    }
    pub fn inference_url(self) -> String {
        if self == Self::OpenKoda { return self.endpoint().into(); }
        format!("{}{}", self.endpoint(), match self {
            Self::OpenAI => "/responses", Self::Anthropic => "/messages", _ => "/chat/completions"
        })
    }
    pub fn body(self, model: &str, prompt: &str, instructions: &str, maximum: u32) -> Value {
        if self == Self::OpenAI {
            return json!({"model":model,"instructions":instructions,"input":[{"role":"user","content":prompt}],"store":false,"max_output_tokens":maximum});
        }
        if self == Self::Anthropic {
            return json!({"model":model,"system":instructions,"messages":[{"role":"user","content":prompt}],"max_tokens":maximum});
        }
        let mut body = json!({"model":model,"messages":[{"role":"system","content":instructions},{"role":"user","content":prompt}],"stream":false});
        if self==Self::Groq {body["tool_choice"]=json!("none");}
        body[if self == Self::Groq { "max_completion_tokens" } else { "max_tokens" }] = json!(maximum);
        body
    }
    pub fn normalize(self, body: Value) -> Result<Value, String> {
        if self == Self::OpenAI { return Ok(body); }
        if self == Self::Anthropic {
            let reason = body["stop_reason"].as_str().ok_or("Resposta sem término identificável.")?;
            let status = match reason {
                "end_turn" | "stop_sequence" => "completed",
                "max_tokens" | "model_context_window_exceeded" => "incomplete",
                _ => "failed",
            };
            let content: Vec<_> = body["content"].as_array().ok_or("Resposta sem conteúdo.")?.iter()
                .filter(|block| block["type"] == "text")
                .map(|block| json!({"type":"output_text","text": block["text"]})).collect();
            let usage = &body["usage"];
            // Messages reports uncached, cache-read and cache-write input as disjoint counts.
            let input_tokens = usage["input_tokens"].as_u64().and_then(|base| {
                ["cache_read_input_tokens", "cache_creation_input_tokens"].iter().try_fold(base, |sum, key| {
                    match usage.get(*key) { None | Some(Value::Null) => Some(sum), Some(count) => sum.checked_add(count.as_u64()?) }
                })
            });
            return Ok(json!({"id":body["id"],"model":body["model"],"status":status,
                "incomplete_details":if status=="incomplete"{json!({"reason":reason})}else{Value::Null},
                "output":[{"type":"message","content":content}],
                "usage":{"input_tokens":input_tokens,"output_tokens":usage["output_tokens"]}}));
        }
        let choice = &body["choices"][0];
        let reason = choice["finish_reason"].as_str().ok_or("Resposta sem término identificável.")?;
        let status = match reason {
            "stop" => "completed", "length" => "incomplete", "tool_calls" | "function_call" | "content_filter" | "error" => "failed", _ => "unknown",
        };
        Ok(json!({"id":body["id"],"model":body["model"],"status":status,
            "incomplete_details":if status=="incomplete"{json!({"reason":reason})}else{Value::Null},
            "output":[{"type":"message","content":[{"type":"output_text","text":choice["message"]["content"]}]}],
            "usage":{"input_tokens":body["usage"]["prompt_tokens"],"output_tokens":body["usage"]["completion_tokens"]}}))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSelection { pub provider: ProviderId, pub model: String, #[serde(default,skip_serializing_if="Option::is_none")] pub reasoning: Option<Reasoning> }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reasoning { Low, Medium, High }

/// A missing selection for a matching, older key preserves its original catalog behavior.
/// A newer key must complete setup before it can use any of the previous key's models.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderModelPreference {
    pub credential_revision: Option<String>,
    pub models: Option<std::collections::BTreeSet<String>>,
}

impl ProviderModelPreference {
    pub fn permits(&self, revision: Option<&str>, model: &str) -> bool {
        revision.is_some() && self.credential_revision.as_deref() == revision
            && self.models.as_ref().is_none_or(|enabled| enabled.contains(model))
    }

    pub fn enabled_models(&self, revision: Option<&str>) -> Value {
        if revision.is_some() && self.credential_revision.as_deref() == revision {
            json!(self.models)
        } else {
            json!([])
        }
    }
}
impl AiSelection {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_model(&self.model) { return Err("Identificador de modelo inválido.".into()); }
        if self.reasoning.is_some() && !matches!(self.provider,ProviderId::OpenKoda|ProviderId::OpenRouter|ProviderId::OpenAI) { return Err("Esta conexão não oferece ajuste de raciocínio.".into()); }
        Ok(())
    }
}
pub fn valid_model(model: &str) -> bool {
    !model.is_empty() && model.len() <= 160 && model.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._:/@+~".contains(&b))
}

pub fn catalog(body: &Value) -> Result<Vec<String>, String> {
    let rows = body["data"].as_array().ok_or("Catálogo de modelos inválido.")?;
    let names = rows.iter().map(|row| row["id"].as_str().filter(|id| valid_model(id))
        .map(str::to_owned).ok_or("O catálogo contém um identificador de modelo inválido."))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    Ok(names.into_iter().collect())
}
