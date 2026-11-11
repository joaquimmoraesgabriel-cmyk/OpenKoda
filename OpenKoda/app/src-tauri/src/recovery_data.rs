//! Reconstruct known SQLite schemas from live rows only. No source pages,
//! freelist, vault rows or connection confirmations enter an output file.
use super::*;
use rusqlite::{params_from_iter, types::Value as SqlValue};

const MAX_LOGICAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ROWS: u64 = 100_000;
const CORE: &[(&str, &str)] = &[("app_state", "CREATE TABLE app_state (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL)")];
const ASSISTANT: &[(&str, &str)] = &[
    ("vault", "CREATE TABLE vault (id INTEGER PRIMARY KEY CHECK(id=1), cipher BLOB NOT NULL, revision TEXT NOT NULL)"),
    ("messages", "CREATE TABLE messages (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, role TEXT NOT NULL, text TEXT NOT NULL, created TEXT NOT NULL)"),
    ("requests", "CREATE TABLE requests (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, endpoint TEXT NOT NULL, model TEXT NOT NULL, revision TEXT NOT NULL, context_hash TEXT NOT NULL, status TEXT NOT NULL, created TEXT NOT NULL, input_tokens INTEGER, output_tokens INTEGER, details TEXT NOT NULL DEFAULT '{}')"),
    ("connection_check", "CREATE TABLE connection_check (id INTEGER PRIMARY KEY CHECK(id=1), model TEXT NOT NULL, endpoint TEXT NOT NULL, revision TEXT NOT NULL, checked TEXT NOT NULL)"),
    ("investigations", "CREATE TABLE investigations (id TEXT PRIMARY KEY, assessment TEXT NOT NULL, data TEXT NOT NULL)"),
];
const ENVIRONMENT: &[(&str, &str)] = &[
    ("environment_state", "CREATE TABLE environment_state(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL)"),
    ("environment_plans", "CREATE TABLE environment_plans(id TEXT PRIMARY KEY, data TEXT NOT NULL)"),
    ("environment_repairs", "CREATE TABLE environment_repairs(id TEXT PRIMARY KEY,data TEXT NOT NULL)"),
];
const ENVIRONMENT_SWITCH: (&str, &str) = ("environment_switches",
    "CREATE TABLE environment_switches(environment_id TEXT PRIMARY KEY, plan_id TEXT UNIQUE NOT NULL, previous_state TEXT NOT NULL, selected_registration TEXT NOT NULL, activated_at TEXT NOT NULL)");
const BRIDGE: &[(&str, &str)] = &[("bridge_runs", "CREATE TABLE bridge_runs(id TEXT PRIMARY KEY,data TEXT NOT NULL)")];
const REMEDIATION: &[(&str, &str)] = &[("remediation_plans", "CREATE TABLE remediation_plans(id TEXT PRIMARY KEY, finding_id TEXT NOT NULL, data TEXT NOT NULL, backup BLOB)")];
const RETENTION: &[(&str, &str)] = &[("retention_plans", "CREATE TABLE retention_plans(id TEXT PRIMARY KEY,data TEXT NOT NULL)")];
const PURGE: &[(&str, &str)] = &[("project_purge_plans", "CREATE TABLE project_purge_plans(id TEXT PRIMARY KEY,data TEXT NOT NULL)")];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(super) struct DatabaseInventory {
    pub name: String,
    pub user_version: i64,
    pub rows: u64,
    pub logical_sha256: String,
    pub logical_bytes: u64,
}

pub(super) struct LogicalDatabase {
    pub inventory: DatabaseInventory,
    memory: Connection,
}

fn failure(_: impl std::fmt::Display) -> String {
    "Não foi possível revisar o banco de recuperação; nenhum conteúdo foi exportado.".into()
}

fn normalize(sql: &str) -> String {
    sql.chars().filter(|c| !c.is_ascii_whitespace()).flat_map(char::to_lowercase)
        .collect::<String>().replace("ifnotexists", "").trim_end_matches(';').into()
}

fn expected(name: &str, version: i64) -> Result<Vec<(&'static str, &'static str)>, String> {
    let base = match name {
        "openkoda.sqlite3" => CORE,
        "assistant.sqlite3" => ASSISTANT,
        "environment.sqlite3" | "environment-history.sqlite3" => ENVIRONMENT,
        "kali-bridge.sqlite3" => BRIDGE,
        "remediation.sqlite3" => REMEDIATION,
        "retention.sqlite3" => RETENTION,
        "project-purge.sqlite3" => PURGE,
        _ => return Err("Banco fora do conjunto de recuperação revisado.".into()),
    };
    let mut schema=base.to_vec();
    if matches!(name, "environment.sqlite3" | "environment-history.sqlite3") && version >= 1 {
        schema.push(ENVIRONMENT_SWITCH);
        if version >= 2 { schema.push(crate::environment::REMOVAL_TABLE); }
    }
    if name=="assistant.sqlite3" && version==1 {
        for (table,ddl) in &mut schema {match *table {
            "vault" => *ddl=crate::ai::VAULT_SCHEMA,
            "connection_check" => *ddl=crate::ai::CHECK_SCHEMA,
            _ => {}
        }}
    }
    if name=="openkoda.sqlite3" && version>=7 {
        schema.extend(crate::work_store::WORK_SCHEMA.iter().copied().filter(|(table,_)|version>=8||*table!="work_ai_dispositions"));
        schema.extend_from_slice(crate::capture::CAPTURE_SCHEMA);
    }
    Ok(schema)
}

pub(super) fn verify_schema(db: &Connection, name: &str) -> Result<i64, String> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(failure)?;
    let valid_version = match name {
        "openkoda.sqlite3" => (1..=core::STORE_VERSION).contains(&version),
        "environment.sqlite3" | "environment-history.sqlite3" => (0..=2).contains(&version),
        "assistant.sqlite3" => (0..=1).contains(&version),
        _ => version == 0,
    };
    if !valid_version { return Err("Versão de banco não reconhecida para a cópia de recuperação.".into()); }
    let schema = expected(name,version)?;
    let integrity: String = db.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(failure)?;
    if integrity != "ok" { return Err("Banco inconsistente; cópia de recuperação recusada.".into()); }
    // Compare against trusted DDL, including all implicit UNIQUE indexes.
    // Arbitrary indexes, triggers, views and schema extensions stay forbidden.
    let reference=Connection::open_in_memory().map_err(failure)?;
    for (_,ddl) in &schema { reference.execute_batch(ddl).map_err(failure)?; }
    let schema_rows=|connection:&Connection|->Result<Vec<(String,String,String,Option<String>)>,String>{
        let mut query=connection.prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").map_err(failure)?;
        let rows=query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?.map(|sql|normalize(&sql))))).map_err(failure)?;
        rows.map(|row|row.map_err(failure)).collect()
    };
    if schema_rows(db)?!=schema_rows(&reference)? {
        return Err("Schema desconhecido ou incompleto; tabelas, colunas ou código adicionais não serão copiados.".into());
    }
    Ok(version)
}

fn digest_value(hash: &mut Sha256, value: &SqlValue) -> u64 {
    match value {
        SqlValue::Null => { hash.update([0]); 0 }
        SqlValue::Integer(v) => { hash.update([1]); hash.update(v.to_le_bytes()); 8 }
        SqlValue::Real(v) => { hash.update([2]); hash.update(v.to_bits().to_le_bytes()); 8 }
        SqlValue::Text(v) => { hash.update([3]); hash.update((v.len() as u64).to_le_bytes()); hash.update(v.as_bytes()); v.len() as u64 }
        SqlValue::Blob(v) => { hash.update([4]); hash.update((v.len() as u64).to_le_bytes()); hash.update(v); v.len() as u64 }
    }
}

fn contains_secret_value(text:&str,detector:&dyn Fn(&str)->bool,managed:&dyn Fn(&str)->bool,depth:u8)->bool{
    if depth>32{return true;}
    // Check exact managed material before parsing can normalize numbers or
    // escapes. The generic redactor runs on values, not the complete object:
    // hasApiKey:false is metadata, not an API credential assignment.
    if managed(text){return true;}
    fn inspect(value:&Value,detector:&dyn Fn(&str)->bool,managed:&dyn Fn(&str)->bool,depth:u8)->bool{if depth>32{return true;}match value{
        Value::String(text)=>contains_secret_value(text,detector,managed,depth+1),
        Value::Number(number)=>detector(&number.to_string())||managed(&number.to_string()),
        Value::Array(items)=>items.iter().any(|item|inspect(item,detector,managed,depth+1)),
        Value::Object(fields)=>fields.iter().any(|(key,value)|{
            let normalized=key.to_ascii_lowercase().replace(['_','-'],"");
            // Only an actual secret-valued field is prohibited here. Booleans
            // such as hasApiKey and classification flags are control metadata.
            let secret_field=matches!(normalized.as_str(),"password"|"passwd"|"apikey"|"token"|"secret"|"authorization"|"cookie"|"setcookie"|"privatekey");
            detector(key)||managed(key)||secret_field&&value.as_str().is_some_and(|value|!value.is_empty())||inspect(value,detector,managed,depth+1)
        }),
        _=>false,
    }}
    match serde_json::from_str::<Value>(text){Ok(value)=>inspect(&value,detector,managed,depth+1),Err(_)=>detector(text)}
}

#[cfg(test)]
pub(super) fn prepare_database(source: &Path, name: &str, contains_secret: &dyn Fn(&str)->bool) -> Result<LogicalDatabase,String> {
    prepare_database_with_managed(source,name,contains_secret,contains_secret)
}
pub(super) fn prepare_database_with_managed(source: &Path, name: &str, contains_secret: &dyn Fn(&str)->bool, contains_managed_secret:&dyn Fn(&str)->bool) -> Result<LogicalDatabase,String> {
    require_plain(source, false)?;
    let input = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(failure)?;
    input.busy_timeout(Duration::from_secs(2)).map_err(failure)?;
    input.execute_batch("PRAGMA trusted_schema=OFF; BEGIN;").map_err(failure)?;
    let version = verify_schema(&input, name)?;
    if name=="openkoda.sqlite3" && version>=7 {
        crate::capture::validate_backup_streams(&input,contains_secret,contains_managed_secret)?;
    }
    let schema = expected(name,version)?;
    let memory = Connection::open_in_memory().map_err(failure)?;
    memory.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA secure_delete=ON; BEGIN;").map_err(failure)?;
    // All referenced tables must exist before SQLite prepares the first INSERT.
    // Heads/revisions intentionally use deferred circular foreign keys.
    for (_,ddl) in &schema{memory.execute_batch(ddl).map_err(failure)?;}
    let mut digest = Sha256::new();
    digest.update(b"openkoda.logical-backup.v1");
    digest.update(name.as_bytes());
    digest.update(version.to_le_bytes());
    let mut logical_bytes = 0u64;
    let mut row_count = 0u64;
    let deadline = Instant::now() + Duration::from_secs(30);
    for (table, ddl) in &schema {
        digest.update(normalize(ddl).as_bytes());
        if name == "assistant.sqlite3" && matches!(*table, "vault" | "connection_check") { continue; }
        // Table names and schemas are constants, never caller-controlled SQL.
        let mut select = input.prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid")).map_err(failure)?;
        let columns = select.column_count();
        let status_column = if name == "assistant.sqlite3" && *table == "requests" { select.column_index("status").ok() } else { None };
        let placeholders = vec!["?"; columns].join(",");
        let mut insert = memory.prepare(&format!("INSERT INTO \"{table}\" VALUES({placeholders})")).map_err(failure)?;
        let mut rows = select.query([]).map_err(failure)?;
        while let Some(row) = rows.next().map_err(failure)? {
            row_count += 1;
            if row_count > MAX_ROWS || Instant::now() > deadline { return Err("Cópia lógica excede o limite de registros ou tempo.".into()); }
            let mut values = (0..columns).map(|index| row.get::<_,SqlValue>(index).map_err(failure)).collect::<Result<Vec<_>,_>>()?;
            if let Some(index) = status_column {
                if values[index] == SqlValue::Text("pending".into()) { values[index] = SqlValue::Text("unknown".into()); }
            }
            digest.update((columns as u64).to_le_bytes());
            for value in &values {
                let forbidden = match value {
                    SqlValue::Text(text) => contains_secret_value(text,contains_secret,contains_managed_secret,0),
                    SqlValue::Blob(bytes) => contains_secret_value(&String::from_utf8_lossy(bytes),contains_secret,contains_managed_secret,0),
                    SqlValue::Integer(number) => contains_secret(&number.to_string())||contains_managed_secret(&number.to_string()),
                    SqlValue::Real(number) => contains_secret(&number.to_string())||contains_managed_secret(&number.to_string()),
                    _ => false,
                };
                if forbidden { return Err(format!("data_secret_material: registro {row_count} da tabela {table} em {name}; original preservado.")); }
                logical_bytes = logical_bytes.checked_add(digest_value(&mut digest, value)).ok_or("Cópia lógica excede o limite.")?;
                if logical_bytes > MAX_LOGICAL_BYTES { return Err("Cópia lógica excede o limite de bytes.".into()); }
            }
            insert.execute(params_from_iter(values.iter())).map_err(failure)?;
        }
    }
    memory.pragma_update(None, "user_version", version).map_err(failure)?;
    memory.execute_batch("COMMIT;").map_err(failure)?;
    input.execute_batch("COMMIT;").map_err(failure)?;
    Ok(LogicalDatabase { inventory: DatabaseInventory { name:name.into(), user_version:version, rows:row_count, logical_sha256:format!("{:x}",digest.finalize()), logical_bytes }, memory })
}

impl LogicalDatabase {
    pub fn write_new(&self, destination: &Path) -> Result<(),String> {
        // Exclusive reservation prevents replacing a previous artifact.
        fs::OpenOptions::new().write(true).create_new(true).open(destination).map_err(failure)?;
        let mut output = Connection::open(destination).map_err(failure)?;
        output.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;").map_err(failure)?;
        copy_sqlite(&self.memory, &mut output)?;
        output.execute_batch("PRAGMA journal_mode=DELETE;").map_err(failure)?;
        drop(output);
        fs::OpenOptions::new().write(true).open(destination).and_then(|file|file.sync_all()).map_err(failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_environment_copies_keep_legacy_and_switched_schemas_distinct() {
        let dir = tempfile::tempdir().unwrap();
        for version in [0, 1, 2] {
            let source = dir.path().join(format!("source-{version}.sqlite3"));
            let db = Connection::open(&source).unwrap();
            for (_, ddl) in ENVIRONMENT { db.execute_batch(ddl).unwrap(); }
            if version >= 1 {
                db.execute_batch(ENVIRONMENT_SWITCH.1).unwrap();
                db.execute("INSERT INTO environment_switches VALUES (?1,?2,?3,?4,?5)",
                    ["target-id","plan-id","previous-state","selected-registration","recorded-time"]).unwrap();
            }
            if version == 2 {
                db.execute_batch(crate::environment::REMOVAL_TABLE.1).unwrap();
                db.execute("INSERT INTO environment_removals VALUES (?1,?2,?3)", ["removal-plan","original","removal-record"]).unwrap();
            }
            db.pragma_update(None, "user_version", version).unwrap();
            let packet = prepare_database(&source, "environment.sqlite3", &|_| false).unwrap();
            let destination = dir.path().join(format!("history-{version}.sqlite3"));
            packet.write_new(&destination).unwrap();
            let historical = Connection::open(&destination).unwrap();
            assert_eq!(verify_schema(&historical, "environment-history.sqlite3").unwrap(), version);
            if version >= 1 {
                let previous: String = historical.query_row("SELECT previous_state FROM environment_switches", [], |r| r.get(0)).unwrap();
                assert_eq!(previous, "previous-state");
            }
            if version == 2 {
                let retired: String = historical.query_row("SELECT data FROM environment_removals", [], |r| r.get(0)).unwrap();
                assert_eq!(retired, "removal-record");
            }
            historical.pragma_update(None, "user_version", (version + 1) % 3).unwrap();
            assert!(verify_schema(&historical, "environment-history.sqlite3").is_err());
        }
    }
    fn core_db(path: &Path) -> Connection {
        let db = Connection::open(path).unwrap();
        db.execute_batch(CORE[0].1).unwrap();
        db.execute_batch(crate::work_store::WORK_SCHEMA_SQL).unwrap();
        db.execute_batch(crate::capture::CAPTURE_SCHEMA_SQL).unwrap();
        db.pragma_update(None,"user_version",core::STORE_VERSION).unwrap();
        db
    }
    #[test]
    fn logical_copy_drops_deleted_page_material_and_preserves_live_rows() {
        let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.sqlite3");let destination=dir.path().join("result.sqlite3");
        let db=core_db(&source);db.execute_batch("PRAGMA secure_delete=OFF;").unwrap();
        let old="SYNTHETIC-DELETED-PAGE-SECRET".repeat(1000);
        db.execute("INSERT INTO app_state VALUES(1,?1)",[&old]).unwrap();
        db.execute("UPDATE app_state SET json='preserved exact bytes' WHERE id=1",[]).unwrap();drop(db);
        assert!(String::from_utf8_lossy(&fs::read(&source).unwrap()).contains("SYNTHETIC-DELETED-PAGE-SECRET"));
        let prepared=prepare_database(&source,"openkoda.sqlite3",&|text|text.contains("SYNTHETIC-DELETED-PAGE-SECRET")).unwrap();
        assert!(!destination.exists());prepared.write_new(&destination).unwrap();
        assert!(!String::from_utf8_lossy(&fs::read(&destination).unwrap()).contains("SYNTHETIC-DELETED-PAGE-SECRET"));
        assert_eq!(Connection::open(&destination).unwrap().query_row("SELECT json FROM app_state",[],|r|r.get::<_,String>(0)).unwrap(),"preserved exact bytes");
        assert!(prepared.write_new(&destination).is_err());
    }
    #[test]
    fn secret_live_column_and_unknown_schema_are_rejected_without_output() {
        let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.sqlite3");let db=core_db(&source);
        db.execute("INSERT INTO app_state VALUES(1,'SYNTHETIC-MANAGED-SECRET')",[]).unwrap();
        let error=match prepare_database(&source,"openkoda.sqlite3",&|text|text.contains("SYNTHETIC-MANAGED-SECRET")){Err(error)=>error,Ok(_)=>panic!("secret accepted")};
        assert!(error.contains("data_secret_material"));assert!(!error.contains("SYNTHETIC-MANAGED-SECRET"));
        db.execute("UPDATE app_state SET json='safe'",[]).unwrap();db.execute_batch("CREATE TRIGGER extra AFTER UPDATE ON app_state BEGIN DELETE FROM app_state; END;").unwrap();
        assert!(prepare_database(&source,"openkoda.sqlite3",&|_|false).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(),1);
    }
    #[test]
    fn assistant_excludes_vault_before_scanning_or_writing_and_keeps_history() {
        for version in [0,1] {
            let directory=tempfile::tempdir().unwrap();
            let source=directory.path().join("source.sqlite3");
            let db=Connection::open(&source).unwrap();
            for (_,ddl) in expected("assistant.sqlite3",version).unwrap(){db.execute_batch(ddl).unwrap();}
            db.pragma_update(None,"user_version",version).unwrap();
            for id in 1..=if version==0{1}else{3}{
                db.execute("INSERT INTO vault VALUES(?1,?2,'revision')",rusqlite::params![id,b"SYNTHETIC-CIPHER".as_slice()]).unwrap();
                db.execute("INSERT INTO connection_check VALUES(?1,'model','endpoint','revision','date')",[id]).unwrap();
            }
            db.execute("INSERT INTO messages VALUES('id','assessment','assistant','historical text','date')",[]).unwrap();
            drop(db);
            let prepared=prepare_database(&source,"assistant.sqlite3",&|text|text.contains("SYNTHETIC-CIPHER")).unwrap();
            let destination=directory.path().join("copy.sqlite3");
            prepared.write_new(&destination).unwrap();
            let copy=Connection::open(&destination).unwrap();
            for table in ["vault","connection_check"]{
                assert_eq!(copy.query_row(&format!("SELECT count(*) FROM {table}"),[],|r|r.get::<_,u32>(0)).unwrap(),0);
            }
            assert_eq!(copy.query_row("SELECT text FROM messages",[],|r|r.get::<_,String>(0)).unwrap(),"historical text");
            assert!(!String::from_utf8_lossy(&fs::read(&destination).unwrap()).contains("SYNTHETIC-CIPHER"));
        }
    }
    #[test]
    fn managed_material_in_json_keys_nested_strings_and_numbers_is_rejected() {
        const SENTINEL:&str="SYNTHETIC-CREDENTIAL-123456";
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.sqlite3");let db=core_db(&path);
        db.execute("INSERT INTO app_state VALUES(1,'safe')",[]).unwrap();
        let keyed=Value::Object(std::iter::once((SENTINEL.to_owned(),Value::Bool(false))).collect());
        for raw in [keyed.to_string(),json!({"data":SENTINEL}).to_string(),json!({"data":keyed.to_string()}).to_string(),"{\"data\":123456789012345678901234567890}".into(),"123456789012345678901234567890".into()] {
            db.execute("UPDATE app_state SET json=?1 WHERE id=1",[&raw]).unwrap();
            let detect=|text:&str|text.contains(SENTINEL)||text.contains("123456789012345678901234567890");
            let error=match prepare_database(&path,"openkoda.sqlite3",&detect){Ok(_)=>panic!("managed material was copied"),Err(error)=>error};
            assert!(error.starts_with("data_secret_material"));assert!(!error.contains(SENTINEL));
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(),1);
    }
    #[test]
    fn structural_scan_checks_keys_and_numbers_without_treating_has_api_key_as_secret() {
        let generic=|text:&str|crate::ai::redact(text,None)!=text;
        for raw in [r#"{"hasApiKey":false,"secretMaterial":false}"#,r#"{"hasApiKey":true,"classification":{"secretMaterial":false}}"#] {
            assert!(generic(raw),"fixture exercises the former JSON-line false positive");
            assert!(!contains_secret_value(raw,&generic,&|_|false,0));
        }
        assert!(contains_secret_value(r#"{"SYNTHETIC-CREDENTIAL-123456":false}"#,&|s|s.contains("SYNTHETIC-CREDENTIAL-123456"),&|_|false,0));
        assert!(contains_secret_value(r#"{"number":1234567890123456}"#,&|s|s.contains("1234567890123456"),&|_|false,0));
        assert!(contains_secret_value(r#"{"data":123456789012345678901234567890}"#,&|_|false,&|s|s.contains("123456789012345678901234567890"),0));
    }
}
