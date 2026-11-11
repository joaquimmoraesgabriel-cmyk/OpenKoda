use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Selection {
    Folder,
    File,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PickerRequest {
    selection: Selection,
}

pub(crate) async fn pick(window: tauri::WebviewWindow, payload: Value) -> Result<Value, String> {
    let request: PickerRequest = serde_json::from_value(payload)
        .map_err(|_| "Escolha uma pasta ou um arquivo.")?;
    #[cfg(windows)]
    {
        // The Windows dialog is modal to the existing WebView and runs in its STA.
        let (sender, mut receiver) = tauri::async_runtime::channel(1);
        let owner = window.clone();
        window.run_on_main_thread(move || {
            let result = owner.hwnd().map_err(|_| "Janela principal indisponível.".to_owned())
                .and_then(|handle| show_dialog(handle, request.selection));
            let _ = sender.try_send(result);
        }).map_err(|_| "Não foi possível abrir o seletor na janela principal.")?;
        let path = receiver.recv().await.ok_or("O seletor foi fechado inesperadamente.")??;
        Ok(path.map(Value::String).unwrap_or(Value::Null))
    }
    #[cfg(not(windows))]
    {
        let _ = (window, request.selection);
        Err("Informe o caminho completo do alvo neste sistema.".into())
    }
}

#[cfg(windows)]
fn show_dialog(owner: windows::Win32::Foundation::HWND, selection: Selection) -> Result<Option<String>, String> {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
        FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    // The owning STA is initialized by the WebView. Keep every COM interface on that thread.
    let result = unsafe {
        (|| -> windows::core::Result<Option<String>> {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            let options = dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
                | FOS_DONTADDTORECENT | FOS_NOCHANGEDIR;
            let (title, options) = match selection {
                Selection::Folder => (w!("Escolher pasta do projeto"), (options & !FOS_FILEMUSTEXIST) | FOS_PICKFOLDERS),
                Selection::File => (w!("Escolher arquivo do projeto"), options | FOS_FILEMUSTEXIST),
            };
            dialog.SetTitle(title)?;
            dialog.SetOptions(options)?;
            if let Err(error) = dialog.Show(Some(owner)) {
                if error.code() == ERROR_CANCELLED.to_hresult() {
                    return Ok(None);
                }
                return Err(error);
            }
            let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let text = path.to_string();
            CoTaskMemFree(Some(path.0.cast()));
            Ok(Some(text?))
        })()
    };
    result.map_err(|_| "O Windows não conseguiu obter o caminho selecionado. Tente novamente ou informe o caminho.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_only_a_file_or_folder_without_renderer_supplied_paths() {
        for mode in ["folder", "file"] {
            assert!(serde_json::from_value::<PickerRequest>(json!({"selection": mode})).is_ok());
        }
        for invalid in [json!({}), json!({"selection": "save"}), json!({"selection": "folder", "path": "C:/extra"})] {
            assert!(serde_json::from_value::<PickerRequest>(invalid).is_err());
        }
    }
}
