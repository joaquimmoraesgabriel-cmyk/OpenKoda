//! Short-lived, credential-separated catalog. No API key or catalog is persisted.
use super::*;
use std::{collections::HashMap, sync::OnceLock, time::Instant};

type CatalogCache = HashMap<String, (Instant, Value)>;
static CACHE: OnceLock<Mutex<CatalogCache>> = OnceLock::new();

pub(super) fn router_catalog(key: &str) -> Result<Value, String> {
    let fingerprint = format!("{:x}", Sha256::digest(key.as_bytes()));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((at, body)) = cache.lock().map_err(failure)?.get(&fingerprint) {
        if at.elapsed() < Duration::from_secs(60) { return Ok(body.clone()); }
    }
    let response = authenticate(client()?.get(ProviderId::OpenRouter.catalog_url())
        .timeout(Duration::from_secs(15)), ProviderId::OpenRouter, key)
        .send().map_err(network_error)?;
    let body = read_bounded_response(response, 20 * 1024 * 1024)?;
    if !body["data"].is_array() { return Err("Catálogo do provedor inválido. Tente consultar novamente.".into()); }
    let mut cache = cache.lock().map_err(failure)?;
    cache.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(60));
    if cache.len() >= 8 { cache.clear(); }
    cache.insert(fingerprint, (Instant::now(), body.clone()));
    Ok(body)
}

pub(super) fn entries(body: &Value, for_analysis: bool) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    body["data"].as_array().into_iter().flatten().filter_map(|row| {
        let id = row["id"].as_str().filter(|id| crate::ai_provider::valid_model(id))?;
        if !seen.insert(id) { return None; }
        let modalities = |field: &str| row["architecture"][field].as_array()
            .is_some_and(|list| list.iter().any(|v| v == "text"));
        let chat = modalities("input_modalities") && modalities("output_modalities") &&
            row["endpoints"].as_array().is_none_or(|list| list.iter().any(|v| v == "/api/v1/chat/completions"));
        let analysis = chat && scan_transport::router_settings(row).is_ok();
        let selectable = if for_analysis { analysis } else { chat };
        Some(json!({"id":id,"name":row["name"].as_str().unwrap_or(id),"chat":chat,"analysis":analysis,
            "selectable":selectable,"availability":if selectable{"eligible"}else{"unsupported"},
            "reason":if !chat{"Este modelo não oferece chat de texto nesta integração."}
                else if for_analysis && !analysis{"Este modelo não oferece a configuração de custo e formato necessária à análise."}
                else{"Disponível no catálogo desta chave. A disponibilidade da geração é confirmada ao enviar."}}))
    }).collect()
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn multimodal_and_batch_only_entries_do_not_become_chat_choices() {
        let base = json!({"id":"brand/chat","architecture":{"input_modalities":["text"],"output_modalities":["text"]},
            "endpoints":["/api/v1/chat/completions"],"pricing":{"prompt":"0.000001","completion":"0.000002"}});
        let mut image=base.clone();image["id"]=json!("brand/image");image["architecture"]["output_modalities"]=json!(["image"]);
        let mut batch=base.clone();batch["id"]=json!("brand/batch");batch["endpoints"]=json!(["/api/v1/batches"]);
        let rows=entries(&json!({"data":[base.clone(),image,batch,base]}),false);
        assert_eq!(rows.len(),3);assert_eq!(rows[0]["selectable"],true);
        assert_eq!(rows[1]["selectable"],false);assert_eq!(rows[2]["selectable"],false);
    }
}
