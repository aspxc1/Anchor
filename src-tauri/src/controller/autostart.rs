use super::SessionState;
use tauri::{Manager, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;

const SETTING: &str = "autostart_enabled";

#[tauri::command]
pub fn read_autostart(window: WebviewWindow) -> Result<bool, String> {
    dashboard(&window)?;
    window
        .autolaunch()
        .is_enabled()
        .map_err(|error| report("read the Windows startup setting", error))
}

#[tauri::command]
pub fn set_autostart(window: WebviewWindow, enabled: bool) -> Result<bool, String> {
    dashboard(&window)?;
    let state = window.state::<SessionState>();
    let session = state.lock().unwrap_or_else(|error| error.into_inner());
    let storage = session.store()?;
    let manager = window.autolaunch();
    let was_enabled = manager
        .is_enabled()
        .map_err(|error| report("read the Windows startup setting", error))?;

    if enabled != was_enabled {
        let update = if enabled {
            manager.enable()
        } else {
            manager.disable()
        };
        update.map_err(|error| report("update the Windows startup setting", error))?;
    }
    if let Err(error) = storage.set_setting(SETTING, if enabled { "true" } else { "false" }) {
        if enabled != was_enabled {
            let rollback = if was_enabled {
                manager.enable()
            } else {
                manager.disable()
            };
            if let Err(rollback) = rollback {
                eprintln!("Could not restore the previous startup setting: {rollback}");
            }
        }
        return Err(report("persist the Windows startup setting", error));
    }
    Ok(enabled)
}

fn dashboard(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Use Dashboard Settings to change Windows startup.".into())
    }
}

fn report(context: &str, error: impl std::fmt::Display) -> String {
    let message = format!("Could not {context}: {error}");
    eprintln!("{message}");
    message
}
