use serde_json::json;
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;
#[test]
#[ignore = "requires an explicitly selected local installer; no network"]
fn updater_verify_packaged_candidate() {
    let file=std::path::PathBuf::from(std::env::var_os("OPENKODA_VERIFY_UPDATE_FILE").expect("Set OPENKODA_VERIFY_UPDATE_FILE"));
    let version=std::env::var("OPENKODA_VERIFY_UPDATE_VERSION").expect("Set OPENKODA_VERIFY_UPDATE_VERSION");
    let config:serde_json::Value=serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let mut context=tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert("updater".into(),config["plugins"]["updater"].clone());
    let app=tauri::test::mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).unwrap();
    let updater=app.updater_builder().target("test").build().unwrap();
    let signature=std::fs::read_to_string(format!("{}.sig",file.display())).expect("Signature file missing");
    let update=updater.restore_update(json!({"version":version,"platforms":{"test":{"url":"https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/candidate/package.exe","signature":signature.trim()}}})).unwrap().unwrap();
    update.verify(&std::fs::read(file).unwrap()).expect("Candidate does not match the production public key/version");
}
    fn test_updater()->(tauri::App<tauri::test::MockRuntime>,tauri_plugin_updater::Updater) {
        let mut context=tauri::test::mock_context(tauri::test::noop_assets());
        let config:serde_json::Value=serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let mut plugin_config=config["plugins"]["updater"].clone();
        plugin_config["pubkey"]=json!(include_str!("fixtures/update-public-key.txt").trim());
        context.config_mut().plugins.0.insert("updater".into(),plugin_config);
        let app=tauri::test::mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).unwrap();
        let updater=app.updater_builder().target("test").build().unwrap(); (app,updater)
    }
    fn signed_manifest()->serde_json::Value { json!({"version":"9.9.9","platforms":{"test":{"url":"https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/fixture/update.exe","signature":include_str!("fixtures/update-payload.txt.sig").trim()}}}) }
    // Tauri >=2.14 verifies app_version in the minisign trusted comment, which
    // CLI >=2.12 signs alongside the artifact. Manifest version spoofing fails.
    #[test] fn updater_official_plugin_accepts_signature_and_rejects_corruption_and_version_spoofing() {
        let (_app,updater)=test_updater();
        let payload=include_bytes!("fixtures/update-payload.txt");
        let update=updater.restore_update(signed_manifest()).unwrap().unwrap();
        update.verify(payload).unwrap();
        assert!(update.verify(b"modified payload").is_err());
        let mut false_version=signed_manifest(); false_version["version"]=json!("9.9.10");
        assert!(updater.restore_update(false_version).unwrap().unwrap().verify(payload).is_err());
        let mut unsigned=signed_manifest(); unsigned["platforms"]["test"]["signature"]=json!("");
        assert!(updater.restore_update(unsigned).unwrap().unwrap().verify(payload).is_err());
        let mut old=signed_manifest(); old["version"]=json!("0.0.1"); assert!(updater.restore_update(old).unwrap().is_none());
    }
    #[test] fn updater_official_plugin_network_failure_is_recoverable() {
        let (app,_)=test_updater();
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let address=listener.local_addr().unwrap();drop(listener);
        let updater=app.updater_builder().endpoints(vec![url::Url::parse(&format!("http://{address}/unavailable")).unwrap()]).unwrap().timeout(Duration::from_millis(250)).build().unwrap();
        assert!(tauri::async_runtime::block_on(updater.check()).is_err());
    }
