//! Token history belongs to the active profile, including requests from before the budget ledger.
use super::{failure, json, Connection, ProviderId, Value};

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

#[cfg(test)]
mod tests {
    use super::*;
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
