//! Integration of the additive workflow schema with the existing Store boundary.
use crate::{core::{Store,STORE_VERSION},model::Snapshot};
use rusqlite::Connection;

fn legacy(root:&std::path::Path,raw:&str)->Connection{
    let db=Connection::open(root.join("openkoda.sqlite3")).unwrap();
    db.execute_batch("CREATE TABLE app_state (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL); PRAGMA user_version=6;").unwrap();
    db.execute("INSERT INTO app_state VALUES(1,?1)",[raw]).unwrap();
    db
}
fn raw(db:&Connection)->String{db.query_row("SELECT json FROM app_state WHERE id=1",[],|r|r.get(0)).unwrap()}
fn version(db:&Connection)->i64{db.query_row("PRAGMA user_version",[],|r|r.get(0)).unwrap()}

#[test]
fn workflow_migration_preserves_exact_legacy_json_and_creates_all_tables(){
    let dir=tempfile::tempdir().unwrap();
    let original=serde_json::to_string_pretty(&Snapshot::default()).unwrap()+"\n\n";
    drop(legacy(dir.path(),&original));
    let store=Store::open(dir.path().into()).unwrap();
    store.work_read(|db,_|{
        assert_eq!(raw(db),original);
        assert_eq!(version(db),STORE_VERSION);
        for (table,_) in crate::work_store::WORK_SCHEMA.iter().chain(crate::capture::CAPTURE_SCHEMA){
            assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",[table],|r|r.get::<_,i64>(0)).unwrap(),1);
        }
        Ok(())
    }).unwrap();
    drop(store);
    let reopened=Store::open(dir.path().into()).unwrap();
    reopened.work_read(|db,_|{assert_eq!(raw(db),original);Ok(())}).unwrap();
}

#[test]
fn workflow_failed_migration_rolls_back_new_tables_and_keeps_legacy_version(){
    let dir=tempfile::tempdir().unwrap();
    let original=serde_json::to_string(&Snapshot::default()).unwrap();
    let db=legacy(dir.path(),&original);
    // A pre-existing incompatible object cannot be silently adopted or replaced.
    db.execute_batch("CREATE TABLE capture_chunks (untrusted TEXT);").unwrap();drop(db);
    assert!(Store::open(dir.path().into()).is_err());
    let db=Connection::open(dir.path().join("openkoda.sqlite3")).unwrap();
    assert_eq!(version(&db),6);assert_eq!(raw(&db),original);
    let objects:Vec<String>=db.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap().query_map([],|r|r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(objects,vec!["app_state","capture_chunks"]);
}

#[test]
fn workflow_transaction_error_rolls_back_domain_and_sql_together(){
    let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().into()).unwrap();
    let before=store.work_read(|db,_|Ok(raw(db))).unwrap();
    let result:Result<(),String>=store.work_transaction(|tx,state|{
        state.settings.retention_days=31;
        tx.execute_batch("CREATE TABLE rollback_probe (id INTEGER);").unwrap();
        Err("synthetic callback failure".into())
    });
    assert!(result.is_err());
    store.work_read(|db,_|{
        assert_eq!(raw(db),before);
        assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE name='rollback_probe'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        Ok(())
    }).unwrap();
}

#[test]
fn capture_boundary_preserves_exact_canonical_bytes_and_rejects_domain_writes(){
    let dir=tempfile::tempdir().unwrap();
    let original=serde_json::to_string_pretty(&Snapshot::default()).unwrap()+"\n";
    drop(legacy(dir.path(),&original));
    let store=Store::open(dir.path().into()).unwrap();
    store.capture_transaction(|_|Ok(())).unwrap();
    let result=store.capture_transaction(|tx|{
        tx.execute("UPDATE app_state SET json='{}' WHERE id=1",[]).unwrap();Ok(())
    });
    assert!(result.unwrap_err().starts_with("work_storage_unavailable"));
    store.work_read(|db,_|{assert_eq!(raw(db),original);Ok(())}).unwrap();
}

#[test]
fn version_seven_migration_preserves_completed_work_and_capture_bytes(){
    let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"schema seven preserved capture",crate::capture::CaptureState::Sealed);
    let root=store.root.clone();let original=store.work_read(|db,_|Ok(raw(db)+"\n\n")).unwrap();drop(store);
    let db=Connection::open(root.join("openkoda.sqlite3")).unwrap();
    db.execute_batch("DROP TABLE work_ai_dispositions; PRAGMA user_version=7;").unwrap();
    db.execute("UPDATE app_state SET json=?1 WHERE id=1",[&original]).unwrap();drop(db);
    let upgraded=Store::open(root).unwrap();
    upgraded.work_read(|db,state|{
        assert_eq!(version(db),STORE_VERSION);assert_eq!(raw(db),original);
        assert!(state.assessments.iter().any(|a|a.id==aid));
        assert_eq!(crate::capture::load_locked(db,&manifest.id)?,manifest);
        assert_eq!(crate::capture::preserved_locked(db,&manifest)?,b"schema seven preserved capture");
        assert_eq!(db.query_row("SELECT COUNT(*) FROM work_ai_dispositions",[],|r|r.get::<_,i64>(0)).unwrap(),0);Ok(())
    }).unwrap();
}

#[test]
fn invalid_disposition_table_rolls_back_version_seven_migration(){
    let dir=tempfile::tempdir().unwrap();let original=serde_json::to_string_pretty(&Snapshot::default()).unwrap()+"\n";
    let db=legacy(dir.path(),&original);
    db.execute_batch(crate::work_store::WORK_SCHEMA_SQL).unwrap();db.execute_batch(crate::capture::CAPTURE_SCHEMA_SQL).unwrap();
    db.execute_batch("DROP TABLE work_ai_dispositions; CREATE TABLE work_ai_dispositions(untrusted TEXT); PRAGMA user_version=7;").unwrap();drop(db);
    assert!(Store::open(dir.path().into()).is_err());
    let db=Connection::open(dir.path().join("openkoda.sqlite3")).unwrap();
    assert_eq!(version(&db),7);assert_eq!(raw(&db),original);
    assert_eq!(db.query_row("SELECT sql FROM sqlite_master WHERE name='work_ai_dispositions'",[],|r|r.get::<_,String>(0)).unwrap(),"CREATE TABLE work_ai_dispositions(untrusted TEXT)");
}
