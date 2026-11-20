use super::SessionState;
use crate::storage::history::{History, Note, SessionDetail};
use tauri::{State, WebviewWindow};

#[tauri::command]
pub fn session_history(
    window: WebviewWindow,
    state: State<'_, SessionState>,
) -> Result<History, String> {
    require_dashboard(&window)?;
    let session = state.lock().unwrap_or_else(|error| error.into_inner());
    session
        .store()?
        .history()
        .map_err(|error| report("read session history", error))
}

#[tauri::command]
pub fn session_detail(
    window: WebviewWindow,
    state: State<'_, SessionState>,
    id: String,
) -> Result<SessionDetail, String> {
    require_dashboard(&window)?;
    let session = state.lock().unwrap_or_else(|error| error.into_inner());
    session
        .store()?
        .session_detail(&id)
        .map_err(|error| report("read the session", error))
}

#[tauri::command]
pub fn list_notes(
    window: WebviewWindow,
    state: State<'_, SessionState>,
) -> Result<Vec<Note>, String> {
    require_dashboard(&window)?;
    let session = state.lock().unwrap_or_else(|error| error.into_inner());
    session
        .store()?
        .notes(None)
        .map_err(|error| report("list Brain Dumps", error))
}

#[tauri::command]
pub fn delete_note(
    window: WebviewWindow,
    state: State<'_, SessionState>,
    id: i64,
) -> Result<Option<String>, String> {
    require_dashboard(&window)?;
    let mut session = state.lock().unwrap_or_else(|error| error.into_inner());
    let storage = session
        .storage
        .as_mut()
        .ok_or("Anchor storage is unavailable. Restart Anchor and try again.")?;
    storage
        .delete_note(id)
        .map_err(|error| report("delete the note", error))?;
    Ok(storage.sync_markdown().err().map(|error| report(
        "update the Markdown export. The note was deleted from SQLite and the export will retry", error,
    )))
}

#[tauri::command]
pub fn open_markdown_folder(
    window: WebviewWindow,
    state: State<'_, SessionState>,
) -> Result<(), String> {
    require_dashboard(&window)?;
    let folder = {
        let session = state.lock().unwrap_or_else(|error| error.into_inner());
        session.store()?.markdown_folder().to_owned()
    };
    let windows =
        std::env::var_os("WINDIR").ok_or("Windows did not report its installation folder.")?;
    std::process::Command::new(std::path::PathBuf::from(windows).join("explorer.exe"))
        .arg(folder)
        .spawn()
        .map_err(|error| report("open the Markdown folder in Explorer", error))?;
    Ok(())
}

fn require_dashboard(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Open the Anchor dashboard to use the session library.".into())
    }
}

fn report(context: &str, error: impl std::fmt::Display) -> String {
    let message = format!("Could not {context}: {error}");
    eprintln!("{message}");
    message
}
