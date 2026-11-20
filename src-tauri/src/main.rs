#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod win32;

#[derive(serde::Serialize)]
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
    if let Err(error) = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![list_open_windows])
        .run(tauri::generate_context!())
    {
        eprintln!("Could not run Anchor: {error}");
        std::process::exit(1);
    }
}
