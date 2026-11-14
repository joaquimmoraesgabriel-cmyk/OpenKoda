fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // The mock Tauri runtime links TaskDialogIndirect, which requires
        // Common Controls v6. Tauri's normal manifest is only linked to bins.
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}", std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("tests/windows.manifest").display());
        println!("cargo:rerun-if-changed=tests/windows.manifest");
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["request", "desktop_update"])),
    ).expect("Tauri manifest build failed");
}
