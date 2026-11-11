//! Reviewed, offline support documents. Only fields constructed below may reach the file.
use super::{require_plain, RecoveryService, DATABASES};
use crate::core::STORE_VERSION;
use chrono::{Duration as ClockDuration, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::Path, time::{Duration, Instant}};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(600);
const MAX_REVIEWS: usize = 32;
const OUTPUT_ERROR: &str = "Não foi possível confirmar a gravação do pacote de suporte no destino local.";
const COLLISION: &str = "O arquivo de suporte foi removido ou diverge da revisão. Revise um novo pacote.";

pub(super) struct SupportReview {
    deadline: Instant,
    bytes: Vec<u8>,
    sha256: String,
    published: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirmation {
    review_id: Uuid,
    confirmation: bool,
}

fn local_database_files(root: &Path) -> Value {
    let (mut present, mut absent, mut unavailable) = (0, 0, 0);
    for name in DATABASES {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => absent += 1,
            Ok(_) if require_plain(&path, false).is_ok() => present += 1,
            _ => unavailable += 1,
        }
    }
    json!({"present": present, "absent": absent, "unavailable": unavailable})
}

fn receipt(id: Uuid, review: &SupportReview) -> Value {
    let file = format!("suporte-{id}.json");
    json!({"fileName": file, "relativePath": format!("exports/{file}"),
           "bytes": review.bytes.len(), "sha256": review.sha256})
}

// Pin a directory against replacement while resolving its children on Windows.
fn reserve_directory(path: &Path) -> Result<fs::File, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3).custom_flags(0x0200_0000 | 0x0020_0000); // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
    }
    let handle = options.open(path).map_err(|_| OUTPUT_ERROR)?;
    require_plain(path, true).map_err(|_| OUTPUT_ERROR)?;
    Ok(handle)
}

fn verify_destination(path: &Path, expected: &[u8]) -> Result<(), String> {
    require_plain(path, false).map_err(|_| COLLISION)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x0020_0000);
    }
    let file = options.open(path).map_err(|_| COLLISION)?;
    let mut actual = Vec::with_capacity(expected.len());
    file.take(expected.len() as u64 + 1).read_to_end(&mut actual).map_err(|_| COLLISION)?;
    if actual != expected { return Err(COLLISION.into()); }
    Ok(())
}

fn publish(folder: &Path, final_path: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = folder.join(format!(".suporte-{}.incomplete", Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut writer = options.open(&staging).map_err(|_| OUTPUT_ERROR)?;
    let result = (|| {
        writer.write_all(bytes).and_then(|_| writer.sync_all()).map_err(|_| OUTPUT_ERROR)?;
        drop(writer);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
            let from: Vec<_> = staging.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<_> = final_path.as_os_str().encode_wide().chain(Some(0)).collect();
            // No MOVEFILE_REPLACE_EXISTING: a pre-existing destination always wins.
            if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
                return Err(OUTPUT_ERROR.into());
            }
        }
        #[cfg(not(windows))]
        fs::hard_link(&staging, final_path).map_err(|_| OUTPUT_ERROR)?;
        Ok(())
    })();
    let _ = fs::remove_file(staging);
    result
}

impl RecoveryService {
    pub(crate) fn support(&self, action: &str, payload: Value) -> Result<Value, String> {
        let mut reviews = self.support_reviews.lock().map_err(|_| "Revisão de suporte indisponível.")?;
        let now = Instant::now();
        reviews.retain(|_, review| review.deadline > now);
        match action {
            "support.review" => {
                if payload != json!({}) {
                    return Err("A revisão de suporte não aceita campos ou caminhos externos.".into());
                }
                if reviews.len() >= MAX_REVIEWS {
                    return Err("Limite de revisões recentes atingido. Aguarde a expiração antes de revisar.".into());
                }
                let contents = json!({
                    "formatVersion": 1, "appVersion": env!("CARGO_PKG_VERSION"),
                    "platform": std::env::consts::OS, "architecture": std::env::consts::ARCH,
                    "coreSchema": STORE_VERSION, "localDatabaseFiles": local_database_files(&self.root),
                    "telemetry": "disabled", "remoteTransmission": false
                });
                let mut preview = serde_json::to_string_pretty(&contents).map_err(|_| "Diagnóstico indisponível.")?;
                preview.push('\n');
                let bytes = preview.as_bytes().to_owned();
                let id = Uuid::new_v4();
                let review = SupportReview {
                    deadline: now + TTL, sha256: format!("{:x}", Sha256::digest(&bytes)), bytes, published: false,
                };
                let mut result = receipt(id, &review);
                result["reviewId"] = json!(id);
                result["expiresAt"] = json!((Utc::now() + ClockDuration::seconds(TTL.as_secs() as i64))
                    .to_rfc3339_opts(SecondsFormat::Millis, true));
                result["contents"] = contents;
                result["preview"] = json!(preview);
                reviews.insert(id, review);
                Ok(result)
            }
            "support.export" => {
                let input: Confirmation = serde_json::from_value(payload).map_err(|_| "Confirmação de suporte inválida.")?;
                if !input.confirmation { return Err("Confirme a revisão antes de exportar.".into()); }
                let review = reviews.get_mut(&input.review_id).ok_or("Revisão ausente ou expirada. Revise o pacote novamente.")?;
                let _root = reserve_directory(&self.root)?;
                let folder = self.directory("exports").map_err(|_| OUTPUT_ERROR)?;
                let _folder = reserve_directory(&folder)?;
                let final_path = folder.join(format!("suporte-{}.json", input.review_id));
                if !review.published {
                    publish(&folder, &final_path, &review.bytes)?;
                    review.published = true;
                }
                verify_destination(&final_path, &review.bytes)?;
                Ok(receipt(input.review_id, review))
            }
            _ => Err("Ação de suporte desconhecida.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, RecoveryService) {
        let dir = tempfile::tempdir().unwrap();
        let service = RecoveryService::new(dir.path().to_owned()).unwrap();
        (dir, service)
    }
    fn prepare(service: &RecoveryService) -> Value {
        service.support("support.review", json!({})).unwrap()
    }
    fn commit(service: &RecoveryService, plan: &Value) -> Result<Value, String> {
        service.support("support.export", json!({"reviewId": plan["reviewId"], "confirmation": true}))
    }

    #[test]
    fn preview_contains_only_known_metadata_and_never_reads_or_exports_vault_contents() {
        let (dir, service) = fixture();
        fs::write(dir.path().join("assistant.sqlite3"), b"private-token-project.example").unwrap();
        fs::create_dir(dir.path().join("environment.sqlite3")).unwrap();
        fs::write(dir.path().join("employee-name.log"), b"sensitive").unwrap();
        let plan = prepare(&service);
        assert!(!dir.path().join("exports").exists());
        let contents = &plan["contents"];
        assert_eq!(contents.as_object().unwrap().len(), 8);
        assert_eq!(contents["localDatabaseFiles"], json!({"present":1,"absent":6,"unavailable":1}));
        assert_eq!(contents["telemetry"], "disabled");
        assert_eq!(contents["remoteTransmission"], false);
        let preview = plan["preview"].as_str().unwrap();
        assert_eq!(serde_json::from_str::<Value>(preview).unwrap(), *contents);
        for private in ["private-token", "employee", "sqlite3", plan["reviewId"].as_str().unwrap()] {
            assert!(!preview.contains(private));
        }
        assert_eq!(plan["bytes"], preview.len());
        assert_eq!(plan["sha256"], format!("{:x}", Sha256::digest(preview.as_bytes())));
    }

    #[test]
    fn export_preserves_reviewed_bytes_and_repeated_confirmation_observes_the_same_file() {
        let (dir, service) = fixture();
        let plan = prepare(&service);
        fs::write(dir.path().join("remediation.sqlite3"), b"changed after review").unwrap();
        let receipt = commit(&service, &plan).unwrap();
        let path = dir.path().join(receipt["relativePath"].as_str().unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), plan["preview"].as_str().unwrap());
        assert_eq!(commit(&service, &plan).unwrap(), receipt);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        fs::write(&path, "newer file").unwrap();
        assert!(commit(&service, &plan).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "newer file");
        fs::remove_file(&path).unwrap();
        assert!(commit(&service, &plan).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn malformed_input_wrong_confirmation_expiry_and_foreign_review_cannot_write() {
        let (dir, service) = fixture();
        assert!(service.support("support.review", json!({"note": "external"})).is_err());
        let plan = prepare(&service);
        let id = plan["reviewId"].as_str().unwrap();
        for bad in [json!({"reviewId":id}), json!({"reviewId":id,"confirmation":false}),
                    json!({"reviewId":id,"confirmation":true,"path":"elsewhere"}),
                    json!({"reviewId":Uuid::new_v4(),"confirmation":true})] {
            assert!(service.support("support.export", bad).is_err());
        }
        let (_other_dir, other) = fixture();
        assert!(commit(&other, &plan).is_err());
        service.support_reviews.lock().unwrap().get_mut(&Uuid::parse_str(id).unwrap()).unwrap().deadline = Instant::now();
        assert!(commit(&service, &plan).is_err());
        assert!(!dir.path().join("exports").exists());
    }

    #[test]
    fn review_capacity_expires_and_no_existing_destination_is_replaced() {
        let (dir, service) = fixture();
        let plan = prepare(&service);
        for _ in 1..MAX_REVIEWS { prepare(&service); }
        assert!(service.support("support.review", json!({})).is_err());
        let relative = plan["relativePath"].as_str().unwrap();
        fs::create_dir(dir.path().join("exports")).unwrap();
        fs::write(dir.path().join(relative), b"existing user file").unwrap();
        assert!(commit(&service, &plan).is_err());
        assert_eq!(fs::read(dir.path().join(relative)).unwrap(), b"existing user file");
        assert_eq!(fs::read_dir(dir.path().join("exports")).unwrap().count(), 1);
        service.support_reviews.lock().unwrap().values_mut().for_each(|review| review.deadline = Instant::now());
        prepare(&service);
    }

    #[test]
    fn concurrent_confirmations_publish_exactly_one_document() {
        let (dir, service) = fixture();
        let plan = prepare(&service);
        std::thread::scope(|scope| {
            let a = scope.spawn(|| commit(&service, &plan).unwrap());
            let b = scope.spawn(|| commit(&service, &plan).unwrap());
            assert_eq!(a.join().unwrap(), b.join().unwrap());
        });
        assert_eq!(fs::read_dir(dir.path().join("exports")).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn redirected_exports_directory_is_refused_without_writing_outside_the_profile() {
        use std::os::windows::process::CommandExt;
        let (dir, service) = fixture();
        let external = tempfile::tempdir().unwrap();
        let link = dir.path().join("exports");
        let status = std::process::Command::new("cmd.exe").args(["/C", "mklink", "/J"])
            .arg(&link).arg(external.path()).creation_flags(0x08000000).output().unwrap();
        assert!(status.status.success());
        assert!(commit(&service, &prepare(&service)).is_err());
        fs::remove_dir(link).unwrap();
        assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
    }
}

