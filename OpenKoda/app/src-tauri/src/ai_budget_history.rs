//! Token history belongs to the active profile, including requests from before the budget ledger.
use super::{failure, json, Connection, ProviderId, Value};
use std::collections::HashMap;
use rusqlite::OptionalExtension;

pub(super) fn read(db: &Connection) -> Result<Value, String> {
    // Use each record's local calendar day, including the offset in effect on that date.
    // Keep missing measurements distinct from a measured zero.
    let mut statement = db.prepare(
        "SELECT date(created, 'localtime'), endpoint, model, assessment, COUNT(*),
                SUM(input_tokens), SUM(output_tokens),
                SUM(CASE WHEN input_tokens IS NULL OR output_tokens IS NULL THEN 1 ELSE 0 END)
         FROM requests
         WHERE COALESCE(json_extract(details, '$.notDispatched'), 0) = 0
         GROUP BY 1, 2, 3, 4 ORDER BY 1, 2, 3, 4"
    ).map_err(failure)?;
    let rows = statement.query_map([], |row| {
        let endpoint: String = row.get(1)?;
        let model: String = row.get(2)?;
        let assessment: String = row.get(3)?;
        Ok(json!({
            "day": row.get::<_,String>(0)?,
            "provider": ProviderId::from_endpoint(&endpoint).ok().map(ProviderId::id),
            "model": if crate::ai_provider::valid_model(&model) { model.as_str() } else { "Modelo desconhecido" },
            "assessmentId": uuid::Uuid::parse_str(&assessment).ok().map(|id| id.to_string()),
            "requests": row.get::<_,u64>(4)?,
            "input": row.get::<_,Option<u64>>(5)?,
            "output": row.get::<_,Option<u64>>(6)?,
            "partial": row.get::<_,u64>(7)?
        }))
    }).map_err(failure)?.collect::<Result<Vec<_>, _>>().map_err(failure)?;
    Ok(json!({"rows": rows}))
}

/// The installation ledger outlives a profile. Only charges independently bound
/// to a scan receipt and assessment in this profile belong in its token history.
pub(super) fn append_scans(result: &mut Value, ledger: &super::Ledger, scans: &HashMap<&str, &str>) {
    if scans.is_empty() { return; }
    let read_rows = || -> Result<Vec<Value>, String> {
        let db = ledger.lock()?;
        let identity = super::meta(&db)?;
        let mut query = db.prepare(
            "SELECT date(json_extract(data,'$.createdAt'),'localtime'), data
             FROM budget_charges WHERE id=?1 AND scope=?2 AND state NOT IN ('reserved','not_dispatched')"
        ).map_err(failure)?;
        let mut rows = Vec::new();
        for (&id, &assessment) in scans {
            let row: Option<(String, String)> = query.query_row(
                rusqlite::params![id, super::scope(&identity, assessment)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional().map_err(failure)?;
            let Some((day, data)) = row else { continue };
            let charge: super::Charge = serde_json::from_str(&data).map_err(failure)?;
            if charge.id != id || charge.scope != super::scope(&identity, assessment) { return Err(failure("scan scope")); }
            let resolution = charge.resolution.as_ref();
            // Historical SDK receipts never recorded individual calls.
            let Some(calls) = resolution.and_then(|r|r["requests"].as_u64()).filter(|n| *n > 0) else { continue };
            let partial = resolution.and_then(|r|r["partialRequests"].as_u64())
                .unwrap_or(if charge.input_tokens.is_none() || charge.output_tokens.is_none() {calls} else {0});
            rows.push(json!({"day": day, "assessmentId": assessment, "model": charge.model,
                "provider": ProviderId::from_endpoint(&charge.endpoint).ok().map(ProviderId::id),
                "requests": calls, "input": charge.input_tokens, "output": charge.output_tokens, "partial": partial}));
        }
        Ok(rows)
    };
    match read_rows() {
        Ok(rows) => result["rows"].as_array_mut().expect("history rows").extend(rows),
        Err(_) => result["scanHistoryUnavailable"] = json!(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_tokens_follow_the_active_assessment_once_and_preserve_missing_measurements() {
        let temp = tempfile::tempdir().unwrap();
        let ledger = super::super::Ledger::open(temp.path());
        let provider = ProviderId::OpenRouter;
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        let id = uuid::Uuid::new_v4().to_string();
        let unused = uuid::Uuid::new_v4().to_string();
        ledger.reserve_security(&id,&a,provider,"brand/model",1000).unwrap();
        ledger.reserve_security(&unused,&a,provider,"brand/model",1000).unwrap();
        ledger.dispatched(&id).unwrap();
        ledger.security_usage(&id,2,1,12,4).unwrap();
        ledger.security_settle(&id,None).unwrap();
        let local = HashMap::from([(id.as_str(),a.as_str()),(unused.as_str(),a.as_str())]);
        let mut out = json!({"rows":[]});
        append_scans(&mut out,&ledger,&local);
        let rows = out["rows"].as_array().unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0]["requests"],2);
        assert_eq!(rows[0]["partial"],1);
        assert_eq!(rows[0]["assessmentId"],a);
        assert_eq!(rows[0]["input"],12);
        assert_eq!(rows[0]["output"],4);
        let mut other = json!({"rows":[]});
        append_scans(&mut other,&ledger,&HashMap::from([(id.as_str(),b.as_str())]));
        assert_eq!(other["rows"],json!([]));
    }
    #[test]
    fn includes_legacy_usage_and_excludes_definite_non_dispatches() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE requests(created, endpoint, model, assessment, details, input_tokens, output_tokens);
            WITH RECURSIVE n(v) AS (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<105)
            INSERT INTO requests SELECT '2026-01-15T12:00:00Z', 'https://api.openai.com/v1','model','connection-test','{}',1,0 FROM n;
            INSERT INTO requests VALUES('2026-01-15T12:00:00Z','https://api.openai.com/v1','model','connection-test','{}',2,NULL),
            ('2026-01-15T12:00:00Z','https://api.openai.com/v1','model','connection-test','{\"notDispatched\":true}',80,50),
            ('2026-01-15T12:00:00Z','https://api.groq.com/openai/v1','model','connection-test','{}',NULL,NULL);").unwrap();
        let result = read(&db).unwrap();
        let rows = result["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        let openai = rows.iter().find(|r|r["provider"] == "openai").unwrap();
        assert_eq!(openai["requests"], 106);
        assert_eq!(openai["input"], 107);
        assert_eq!(openai["output"], 0);
        assert_eq!(openai["partial"], 1);
        assert!(openai["assessmentId"].is_null());
        assert!(rows.iter().find(|r|r["provider"] == "groq").unwrap()["input"].is_null());
    }
}
