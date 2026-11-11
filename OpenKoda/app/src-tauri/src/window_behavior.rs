use serde_json::{json, Value};
use std::{fs, io, path::{Path, PathBuf}, sync::Mutex};

#[cfg(windows)]
pub(crate) fn set_taskbar_icon(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{LoadIconW, SendMessageW, ICON_BIG, WM_SETICON},
    };
    let Ok(hwnd) = window.hwnd() else { return };
    // Tauri sets the small title-bar icon. Windows requests ICON_BIG separately for task switching.
    // LoadIconW returns a shared icon from our own module, valid until the application exits.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() { return; }
        let resource = tauri::utils::platform::WINDOWS_APP_ICON_RESOURCE_ID as usize as *const u16;
        let icon = LoadIconW(module, resource);
        if !icon.is_null() {
            SendMessageW(hwnd.0, WM_SETICON, ICON_BIG as usize, icon as isize);
        }
    }
}

pub(crate) struct WindowBehavior {
    marker: PathBuf,
    minimize: Mutex<bool>,
}

impl WindowBehavior {
    pub fn load(base: &Path) -> Self {
        let marker = base.join("minimize-on-close");
        let enabled = marker.is_file();
        Self { marker, minimize: Mutex::new(enabled) }
    }

    pub fn minimizes(&self) -> bool {
        *self.minimize.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn handle(&self, action: &str, payload: &Value) -> Result<Value, String> {
        if action == "window.behavior.get" && payload == &json!({}) {
            return Ok(json!({"minimize": self.minimizes()}));
        }
        let enabled = payload.get("minimize").and_then(Value::as_bool)
            .filter(|_| action == "window.behavior.set" && payload.as_object().is_some_and(|p| p.len() == 1))
            .ok_or("Preferência de janela inválida.")?;
        let mut state = self.minimize.lock().unwrap_or_else(|e| e.into_inner());
        if enabled {
            fs::File::create(&self.marker).map_err(|_| "Não foi possível salvar a preferência de janela.")?;
        } else if let Err(e) = fs::remove_file(&self.marker) {
            if e.kind() != io::ErrorKind::NotFound { return Err("Não foi possível salvar a preferência de janela.".into()); }
        }
        *state = enabled;
        Ok(json!({"minimize": enabled}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_behavior_survives_restart_and_rejects_unrecognized_payloads() {
        let root = tempfile::tempdir().unwrap();
        let pref = WindowBehavior::load(root.path());
        assert!(!pref.minimizes());
        pref.handle("window.behavior.set", &json!({"minimize":true})).unwrap();
        let reopened = WindowBehavior::load(root.path());
        assert!(reopened.minimizes());
        assert!(reopened.handle("window.behavior.set", &json!({"minimize":false,"extra":1})).is_err());
        assert!(reopened.minimizes());
        reopened.handle("window.behavior.set", &json!({"minimize":false})).unwrap();
        assert!(!WindowBehavior::load(root.path()).minimizes());
    }
}
