use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId { OpenAI, OpenRouter, Groq }

impl ProviderId {
    pub const ALL: [Self; 3] = [Self::OpenAI, Self::OpenRouter, Self::Groq];
    pub fn parse(id: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|p| p.id() == id)
            .ok_or_else(|| "Provedor de IA não suportado.".into())
    }
    pub fn from_endpoint(endpoint: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|p| p.endpoint() == endpoint)
            .ok_or_else(|| "O destino da conexão não corresponde a um provedor suportado.".into())
    }
    pub fn id(self) -> &'static str { match self {
        Self::OpenAI => "openai", Self::OpenRouter => "openrouter", Self::Groq => "groq",
    }}
    pub fn slot(self) -> i64 { match self { Self::OpenAI => 1, Self::OpenRouter => 2, Self::Groq => 3 } }
    pub fn endpoint(self) -> &'static str { match self {
        Self::OpenAI => "https://api.openai.com/v1",
        Self::OpenRouter => "https://openrouter.ai/api/v1",
        Self::Groq => "https://api.groq.com/openai/v1",
    }}
    pub fn catalog_url(self) -> String {
        format!("{}{}", self.endpoint(), if self == Self::OpenRouter { "/models/user?output_modalities=all" } else { "/models" })
    }
    pub fn inference_url(self) -> String {
        format!("{}{}", self.endpoint(), if self == Self::OpenAI { "/responses" } else { "/chat/completions" })
    }
    pub fn body(self, model: &str, prompt: &str, instructions: &str, maximum: u32) -> Value {
        if self == Self::OpenAI {
            return json!({"model":model,"instructions":instructions,"input":[{"role":"user","content":prompt}],"store":false,"max_output_tokens":maximum});
        }
        let mut body = json!({"model":model,"messages":[{"role":"system","content":instructions},{"role":"user","content":prompt}],"stream":false});
        if self==Self::Groq {body["tool_choice"]=json!("none");}
        body[if self == Self::Groq { "max_completion_tokens" } else { "max_tokens" }] = json!(maximum);
        body
    }
    pub fn normalize(self, body: Value) -> Result<Value, String> {
        if self == Self::OpenAI { return Ok(body); }
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
pub struct AiSelection { pub provider: ProviderId, pub model: String }
impl AiSelection {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_model(&self.model) { return Err("Identificador de modelo inválido.".into()); }
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
