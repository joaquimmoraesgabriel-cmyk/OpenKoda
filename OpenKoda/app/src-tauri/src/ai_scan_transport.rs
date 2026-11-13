//! Read-only text inference for the source reviewer. Model input cannot select
//! a URL, headers, host tools, or a filesystem path.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const NO_RATE: &str = "Tarifa indisponível para esse modelo. Configure os preços em Histórico de uso > Análises > Gerenciar limites e preços.";

#[derive(Clone, Copy)]
pub(crate) struct Rates {
    // Micro-USD per million tokens (a token is never rounded down).
    pub input: u64,
    pub output: u64,
}
impl Rates {
    pub fn charge(self, input: u64, output: u64) -> Result<u64, String> {
        let amount = input as u128 * self.input as u128 + output as u128 * self.output as u128;
        u64::try_from(amount.div_ceil(1_000_000))
            .map_err(|_| "Custo excede o intervalo suportado.".into())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ReviewModel {
    pub rates: Rates,
    format: ReviewFormat,
}

#[derive(Clone, Copy)]
enum ReviewFormat {
    Text,
    JsonObject,
    FinalReportSchema,
    DecisionSchema,
}

impl ReviewModel {
    pub(crate) fn text(rates: Rates) -> Self {
        Self {
            rates,
            format: ReviewFormat::Text,
        }
    }

    pub(crate) fn for_batch(mut self, final_report: bool) -> Self {
        if !final_report && matches!(self.format, ReviewFormat::FinalReportSchema) {
            self.format = ReviewFormat::DecisionSchema;
        }
        self
    }

    fn response_format(self) -> Option<Value> {
        let report = || {
            json!({
                        "type":"object","additionalProperties":false,
                        "properties":{
                            "findings":{"type":"array","items":{
                                "type":"object","additionalProperties":false,
                                "properties":{
                                    "path":{"type":"string"},
                                    "line":{"type":"integer"},
                                    "excerpt":{"type":"string"},
                                    "title":{"type":"string"},
                                    "severity":{"type":"string","enum":["low","medium","high","critical"]},
                                    "description":{"type":"string"}
                                },
                                "required":["path","line","excerpt","title","severity","description"]
                            }},
                            "summary":{"type":"string"}
                        },
                        "required":["findings","summary"]
            })
        };
        let schema = match self.format {
            ReviewFormat::Text => return None,
            ReviewFormat::JsonObject => return Some(json!({"type":"json_object"})),
            ReviewFormat::FinalReportSchema => report(),
            ReviewFormat::DecisionSchema => {
                // Structured Outputs requires an object at the root. Nest the
                // choice so a report and a lookup are mutually exclusive.
                let read = json!({"type":"object","additionalProperties":false,
                    "properties":{"op":{"type":"string","enum":["read"]},"path":{"type":"string"},"line":{"type":"integer"}},
                    "required":["op","path","line"]});
                let search = json!({"type":"object","additionalProperties":false,
                    "properties":{"op":{"type":"string","enum":["search"]},"query":{"type":"string"}},
                    "required":["op","query"]});
                let context = json!({"type":"object","additionalProperties":false,
                    "properties":{"queries":{"type":"array","items":{"anyOf":[read,search]}}},
                    "required":["queries"]});
                json!({"type":"object","additionalProperties":false,
                    "properties":{"decision":{"anyOf":[context,report()]}},"required":["decision"]})
            }
        };
        Some(
            json!({"type":"json_schema","json_schema":{"name":"source_review","strict":true,"schema":schema}}),
        )
    }

    pub(crate) fn format_bytes(self) -> usize {
        self.response_format()
            .map_or(0, |value| value.to_string().len())
    }
}

pub(crate) trait ScanTransport: Send + Sync {
    fn model_settings(
        &self,
        ai: &AiService,
        provider: ProviderId,
        model: &str,
        key: &str,
    ) -> Result<ReviewModel, String>;
    fn review(
        &self,
        provider: ProviderId,
        model: &str,
        key: &str,
        payload: &str,
        instructions: &str,
        maximum: u32,
        settings: ReviewModel,
        request_id: &str,
        reasoning: Option<crate::ai_provider::Reasoning>,
        stop: &AtomicBool,
        deadline: Instant,
    ) -> Result<Value, String>;
}
pub(crate) struct HttpScan;

pub(super) fn bundled_rates(provider: ProviderId, model: &str) -> Option<Rates> {
    use ProviderId::*;
    // Short standard text calls without granted tools or requested cache writes.
    // Direct OpenAI estimates from the pinned codex-security 0.10.0 cost-model.
    // Unknown IDs require an explicit tariff; never strip aliases or prefixes.
    let (input, output) = match (provider, model) {
        (OpenAI, "gpt-6.1-sol" | "gpt-6-sol") => (2., 10.),
        (OpenAI, "gpt-6-luna") => (0.1, 0.5),
        (OpenAI, "gpt-6-astra") => (10., 50.),
        (OpenAI, "gpt-5.5" | "gpt-5.5-2026-04-23") => (5., 30.),
        (OpenAI, "gpt-5.6" | "gpt-5.6-sol") => (4., 20.),
        (OpenAI, "gpt-5.6-terra") => (2., 12.),
        (OpenAI, "gpt-5.6-luna") => (0.2, 1.2),
        (OpenAI, "gpt-5.6-cyber") => (12.5, 75.),
        (OpenAI, "gpt-6-cyber") => (20., 100.),
        // Reviewed on 2026-11-13 using the direct providers' public price lists.
        // https://platform.claude.com/docs/en/about-claude/pricing
        (Anthropic, "claude-haiku-4-5" | "claude-haiku-4-5-20251001") => (1., 5.),
        (Anthropic, "claude-haiku-5-5") => (0.1, 0.5),
        (Anthropic, "claude-sonnet-5-5") => (2., 10.),
        (Anthropic, "claude-opus-5-5") => (4., 20.),
        // https://console.groq.com/docs/models
        (Groq, "openai/gpt-oss-120b") => (0.15, 0.6),
        (Groq, "openai/gpt-oss-20b") => (0.075, 0.3),
        // https://ai.google.dev/gemini-api/docs/pricing
        (Gemini, "gemini-2.5-flash-lite") => (0.1, 0.4),
        (Gemini, "gemini-2.5-flash") => (0.3, 2.5),
        // https://api-docs.deepseek.com/quick_start/pricing (peak, uncached)
        (DeepSeek, "deepseek-flash" | "deepseek-v4-flash") => (0.3, 1.2),
        (DeepSeek, "deepseek-v4-pro") => (1.32, 3.96),
        // https://docs.x.ai/developers/pricing
        (Xai, "grok-4.3") => (1.25, 2.5),
        (Xai, "grok-4.6" | "grok-4.5") => (2., 6.),
        _ => return None,
    };
    Some(Rates {
        input: (input * 1_000_000.) as u64,
        output: (output * 1_000_000.) as u64,
    })
}

fn router_model(catalog: &Value, model: &str) -> Result<ReviewModel, String> {
    let row = catalog["data"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == model))
        .ok_or(NO_RATE)?;
    router_settings(row)
}

pub(super) fn router_settings(row: &Value) -> Result<ReviewModel, String> {
    let model = row["id"].as_str().ok_or(NO_RATE)?;
    if model.ends_with(":online")
        || model.starts_with('@')
        || model == "openrouter/fusion"
        || model.starts_with("perplexity/sonar")
    {
        return Err("Escolha um modelo de texto sem pesquisa web nem preset.".into());
    }
    if !row["architecture"]["output_modalities"]
        .as_array()
        .is_some_and(|v| v.iter().any(|m| m == "text"))
    {
        return Err("Selecione um modelo que produza texto para a análise.".into());
    }
    let pricing = &row["pricing"];
    let rate = |name: &str| -> Result<u64, String> {
        let amount = pricing[name]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|p| p.is_finite() && (0.0..=1.).contains(p))
            .ok_or(NO_RATE)?;
        Ok((amount * 1_000_000_000_000.).ceil() as u64)
    };
    // OpenRouter includes reasoning in completion_tokens. Use the higher of
    // the two per-token prices as the bound without counting those tokens twice.
    // https://openrouter.ai/assets/State-of-AI.pdf (Model and Token Variants)
    if !pricing["request"].is_null() && rate("request")? != 0 {
        return Err(
            "O modelo cobra uma tarifa por solicitação. Escolha um modelo com tarifa por tokens."
                .into(),
        );
    }
    let output = rate("completion")?;
    let reasoning = if pricing["internal_reasoning"].is_null() {
        0
    } else {
        rate("internal_reasoning")?
    };
    let supports = |parameter: &str| {
        row["supported_parameters"]
            .as_array()
            .is_some_and(|values| values.iter().any(|p| p == parameter))
    };
    let format = if supports("structured_outputs") {
        ReviewFormat::FinalReportSchema
    } else if supports("response_format") {
        ReviewFormat::JsonObject
    } else {
        ReviewFormat::Text
    };
    Ok(ReviewModel {
        rates: Rates {
            input: rate("prompt")?,
            output: output.max(reasoning),
        },
        format,
    })
}

fn scan_body(
    provider: ProviderId,
    model: &str,
    prompt: &str,
    instructions: &str,
    maximum: u32,
    settings: ReviewModel,
) -> Value {
    let mut body = provider.body(model, prompt, instructions, maximum);
    if provider == ProviderId::OpenRouter {
        // Override optional search defaults. The request grants no server tools,
        // sends no images, and asks only for text, even on multimodal models.
        // https://openrouter.ai/docs/guides/features/plugins/overview
        body["modalities"] = json!(["text"]);
        body["plugins"] = json!([{"id":"web","enabled":false}]);
        let rates = settings.rates;
        body["provider"] = json!({"max_price":{"prompt":rates.input as f64/1_000_000.,"completion":rates.output as f64/1_000_000.,"request":0}});
        if let Some(format) = settings.response_format() {
            body["response_format"] = format;
            body["provider"]["require_parameters"] = json!(true);
        }
    }
    body
}

impl ScanTransport for HttpScan {
    fn model_settings(
        &self,
        ai: &AiService,
        provider: ProviderId,
        model: &str,
        key: &str,
    ) -> Result<ReviewModel, String> {
        if provider==ProviderId::OpenKoda {
            let catalog=commercial_models(key)?;
            let row=catalog["models"].as_array().and_then(|rows|rows.iter().find(|row|row["id"]==model))
                .ok_or("Modelo ausente no catálogo atual da assinatura.")?;
            let rate=|field:&str|row[field].as_f64().filter(|v|v.is_finite() && *v > 0. && *v <= 100.)
                .map(|v|(v*1_000_000.).ceil() as u64).ok_or(NO_RATE);
            return Ok(ReviewModel{rates:Rates{input:rate("inputUsdPerMillion")?,output:rate("outputUsdPerMillion")?},
                format:if row["structured"]==true{ReviewFormat::FinalReportSchema}else{ReviewFormat::JsonObject}});
        }
        if provider == ProviderId::OpenRouter {
            let response = authenticate(
                client()?
                    .get(provider.catalog_url())
                    .timeout(Duration::from_secs(15)),
                provider,
                key,
            )
            .send()
            .map_err(network_error)?;
            return router_model(&read_bounded_response(response, 16 * 1024 * 1024)?, model);
        }
        let rates = ai
            .budget
            .scan_rates(provider, model)?
            .or_else(|| bundled_rates(provider, model))
            .ok_or(NO_RATE)?;
        Ok(ReviewModel::text(rates))
    }
    fn review(
        &self,
        provider: ProviderId,
        model: &str,
        key: &str,
        prompt: &str,
        instructions: &str,
        maximum: u32,
        settings: ReviewModel,
        request_id: &str,
        reasoning: Option<crate::ai_provider::Reasoning>,
        stop: &AtomicBool,
        deadline: Instant,
    ) -> Result<Value, String> {
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(90));
        if timeout.is_zero() || stop.load(Ordering::Acquire) {
            return Err("Análise interrompida.".into());
        }
        let client = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(network_error)?;
        let request = client.post(provider.inference_url());
        let request = if provider == ProviderId::Anthropic {
            request
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
        } else {
            request.bearer_auth(key)
        };
        let request = if provider==ProviderId::OpenKoda {
            let mut body=json!({"action":"scan","model":model,"requestId":request_id,"instructions":instructions,
                "messages":[{"role":"user","content":prompt}],"maxTokens":maximum,
                "format":if matches!(settings.format,ReviewFormat::DecisionSchema){"decision"}else{"report"}});
            if let Some(level)=reasoning{body["reasoning"]=json!(level);}
            request.header("apikey",crate::commercial::PUBLISHABLE).json(&body)
        }else{request.json(&scan_body(
            provider,
            model,
            prompt,
            instructions,
            maximum,
            settings,
        ))};
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let expected_model=model.to_owned();
        let expected_request=request_id.to_owned();
        let task = tauri::async_runtime::spawn(async move {
            let result=async{
                let mut response=if provider==ProviderId::OpenKoda {
                    crate::commercial_queue::send(request,&expected_request,|_|{}).await?
                }else{request.send().await.map_err(network_error)?};
                if !response.status().is_success(){return Err(match response.status().as_u16(){
                    401|403=>"O provedor recusou a credencial ou o acesso ao modelo.",402|429=>"Saldo ou limite de requisições atingido no provedor.",
                    400|404|422=>"O provedor recusou o modelo, a entrada ou o limite de saída selecionado.",
                    _=>"O provedor não concluiu a análise.",
                }.into());}
                let mut bytes=Vec::new();
                while let Some(chunk)=response.chunk().await.map_err(network_error)?{
                    if bytes.len()+chunk.len()>MAX_RESPONSE as usize{return Err("Resposta do provedor excedeu o limite.".into());}
                    bytes.extend_from_slice(&chunk);
                }
                let body:Value=serde_json::from_slice(&bytes).map_err(|_|"Resposta do provedor inválida.")?;
                if provider==ProviderId::OpenKoda {
                    if body["status"]!="completed" || body["model"]!=expected_model || body["requestId"]!=expected_request {
                        return Err("O recibo de inferência não corresponde à etapa da análise.".into());
                    }
                    return Ok(json!({"id":body["usage"]["id"],"status":"completed","model":body["model"],
                        "reportedCostUsd":body["usage"]["costUsd"],"usage":{"input_tokens":body["usage"]["inputTokens"],"output_tokens":body["usage"]["outputTokens"]},
                        "output":[{"type":"message","content":[{"type":"output_text","text":body["text"]}]}]}));
                }
                let cost=if provider==ProviderId::OpenRouter {body["usage"]["cost"].as_f64().filter(|v|v.is_finite()&&*v>=0.)}else{None};
                let mut result=provider.normalize(body)?;
                result["reportedCostUsd"]=json!(cost);
                Ok(result)
            }.await;
            let _ = send.send(result);
        });
        loop {
            match receive.recv_timeout(Duration::from_millis(80)) {
                Ok(result) => return result,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Conexão encerrada antes do resultado.".into())
                }
                _ if stop.load(Ordering::Acquire) || Instant::now() >= deadline => {
                    task.abort();
                    return Err(
                        "Análise interrompida. O provedor pode ter recebido a solicitação.".into(),
                    );
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn router_prices_follow_exact_model_and_block_paid_extras() {
        let mut catalog = json!({"data":[{"id":"brand/model","architecture":{"output_modalities":["text"]},"supported_parameters":["response_format"],"pricing":{"prompt":"0.00000005","completion":"0.0000004","request":"0"}}]});
        let settings = router_model(&catalog, "brand/model").unwrap();
        let rates = settings.rates;
        assert_eq!(rates.charge(1000, 100).unwrap(), 90);
        assert!(router_model(&catalog, "model").is_err());
        let body = scan_body(
            ProviderId::OpenRouter,
            "brand/model",
            "abc",
            "instruções",
            100,
            settings,
        );
        assert_eq!(body["provider"]["max_price"]["request"], 0);
        assert_eq!(body["provider"]["max_price"]["prompt"], 0.05);
        catalog["data"][0]["pricing"]["web_search"] = json!("0.014");
        catalog["data"][0]["pricing"]["internal_reasoning"] = json!("0.0000004");
        assert_eq!(
            router_model(&catalog, "brand/model").unwrap().rates.output,
            400_000
        );
        assert_eq!(body["plugins"][0]["enabled"], false);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["provider"]["require_parameters"], true);
        catalog["data"][0]["supported_parameters"] =
            json!(["response_format", "structured_outputs"]);
        let structured = router_model(&catalog, "brand/model").unwrap();
        let format = structured.for_batch(true).response_format().unwrap();
        assert_eq!(format["type"], "json_schema");
        let schema = &format["json_schema"]["schema"];
        assert_eq!(schema["required"], json!(["findings", "summary"]));
        assert_eq!(
            schema["properties"]["findings"]["items"]["properties"]["line"]["type"],
            "integer"
        );
        assert_eq!(
            structured.for_batch(true).format_bytes(),
            format.to_string().len()
        );
        assert_eq!(
            structured.for_batch(false).response_format().unwrap()["type"],
            "json_schema"
        );
        catalog["data"][0]["supported_parameters"] = json!([]);
        let plain = router_model(&catalog, "brand/model").unwrap();
        assert!(scan_body(
            ProviderId::OpenRouter,
            "brand/model",
            "abc",
            "instruções",
            100,
            plain
        )
        .get("response_format")
        .is_none());
        assert!(body.get("tools").is_none());
        catalog["data"][0]["pricing"]["internal_reasoning"] = json!("0.0000008");
        assert_eq!(
            router_model(&catalog, "brand/model").unwrap().rates.output,
            800_000
        );
        catalog["data"][0]["pricing"]["request"] = json!("0.01");
        assert!(router_model(&catalog, "brand/model").is_err());
    }
}
