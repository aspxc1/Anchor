#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod controller;
#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod policy;
#[cfg(windows)]
mod storage;
#[cfg(windows)]
mod timing;
#[cfg(windows)]
mod win32;
#[cfg(windows)]
use tauri::Manager;

#[derive(Clone, serde::Serialize)]
struct OpenWindow {
    hwnd: isize,
    title: String,
    process_name: String,
    pid: u32,
}

#[tauri::command]
fn list_open_windows() -> Result<Vec<OpenWindow>, String> {
    #[cfg(windows)]
    {
        win32::list_open_windows().map_err(|error| {
            eprintln!("Could not enumerate open windows: {error}");
            format!("Could not list open windows. Refresh to try again. ({error})")
        })
    }
    #[cfg(not(windows))]
    {
        Err("Live window discovery requires Windows 10 or 11.".into())
    }
}

fn main() {
    let shortcuts = tauri_plugin_global_shortcut::Builder::new();
    #[cfg(windows)]
    let shortcuts = shortcuts.with_handler(|app, _, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            controller::capture::shortcut_pressed(app);
        }
    });
    let builder = tauri::Builder::default().plugin(shortcuts.build());
    #[cfg(windows)]
    let builder = builder
        .manage(controller::SessionState::default())
        .setup(desktop::setup)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "main" {
                    if let Err(error) = window.hide() {
                        eprintln!("Could not hide the dashboard to the tray: {error}");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_open_windows,
            controller::session_command
        ]);
    #[cfg(not(windows))]
    let builder = builder.invoke_handler(tauri::generate_handler![list_open_windows]);
    match builder.build(tauri::generate_context!()) {
        Ok(app) => app.run(|_app, _event| {
            #[cfg(windows)]
            if matches!(_event, tauri::RunEvent::Exit) {
                desktop::shutdown(_app);
                controller::capture::unregister_shortcuts(_app);
                _app.state::<controller::SessionState>()
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .before_quit(_app);
            }
        }),
        Err(error) => {
            eprintln!("Could not run Anchor: {error}");
            std::process::exit(1);
        }
    }
}
