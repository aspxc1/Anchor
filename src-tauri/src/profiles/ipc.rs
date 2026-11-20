use super::{bind_profile, Profile, ProfileApp, ProfileDraft};
use crate::{
    controller::{SessionState, StartRequest},
    storage::{Audio, Noise, Storage},
    win32::file_dialog,
};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Manager, WebviewWindow};

fn dashboard(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Manage study profiles from the dashboard.".into())
    }
}

fn from_storage<T>(
    window: &WebviewWindow,
    context: &str,
    operation: impl FnOnce(&Storage) -> crate::storage::StoreResult<T>,
) -> Result<T, String> {
    dashboard(window)?;
    let state = window.state::<SessionState>();
    let session = state.lock().unwrap_or_else(|error| error.into_inner());
    operation(session.store()?).map_err(|error| report(context, error))
}

#[tauri::command]
pub fn list_profiles(window: WebviewWindow) -> Result<Vec<Profile>, String> {
    from_storage(&window, "list study profiles", Storage::list_profiles)
}

#[tauri::command]
pub fn save_profile(
    window: WebviewWindow,
    id: Option<String>,
    draft: ProfileDraft,
) -> Result<Profile, String> {
    dashboard(&window)?;
    let draft = draft.validate()?;
    from_storage(&window, "save the study profile", |storage| {
        storage.save_profile(id.as_deref(), draft)
    })
}

#[tauri::command]
pub fn delete_profile(window: WebviewWindow, id: String) -> Result<(), String> {
    from_storage(&window, "delete the study profile", |storage| {
        storage.delete_profile(&id)
    })
}

#[tauri::command]
pub fn pick_profile_app(window: WebviewWindow) -> Result<Option<ProfileApp>, String> {
    dashboard(&window)?;
    let owner = window
        .hwnd()
        .map_err(|error| report("open the executable picker", error))?
        .0 as isize;
    let Some(executable_path) = file_dialog::pick_executable(owner)
        .map_err(|error| report("choose the executable", error))?
    else {
        return Ok(None);
    };
    let process_name = Path::new(&executable_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.to_ascii_lowercase().ends_with(".exe"))
        .ok_or("Choose a Windows .exe file.")?
        .to_owned();
    Ok(Some(ProfileApp {
        process_name,
        executable_path,
        launch_if_closed: true,
        title_hint: None,
    }))
}

struct LaunchGuard(AppHandle);

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        if let Some(state) = self.0.try_state::<SessionState>() {
            state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .profile_launching = false;
        }
    }
}

#[derive(Serialize)]
pub struct LaunchResult {
    warnings: Vec<String>,
}

#[tauri::command]
pub async fn launch_profile(window: WebviewWindow, id: String) -> Result<LaunchResult, String> {
    dashboard(&window)?;
    let app = window.app_handle().clone();
    let profile = {
        let state = app.state::<SessionState>();
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        if state.profile_launching || state.is_running() {
            return Err("Finish the current session or profile launch first.".into());
        }
        let profile = state
            .store()?
            .get_profile(&id)
            .map_err(|error| report("read the study profile", error))?
            .ok_or("This profile was deleted. Refresh the profile list.")?;
        state.profile_launching = true;
        profile
    };
    let _guard = LaunchGuard(app.clone());
    let draft = profile.draft.validate()?;
    let (matched, mut warnings) = bind_profile(&draft).await?;
    let (respond, result) = tokio::sync::oneshot::channel();
    app.clone()
        .run_on_main_thread(move || {
            let result = (|| {
                let windows = crate::list_open_windows()?;
                let allowed = windows
                    .into_iter()
                    .filter(|window| {
                        matched.iter().any(|original| {
                            original.hwnd == window.hwnd && original.pid == window.pid
                        })
                    })
                    .collect::<Vec<_>>();
                if allowed.len() < matched.len() {
                    warnings.push(
                        "One or more matched windows closed before the session started.".into(),
                    );
                }
                if allowed.is_empty() {
                    let details = warnings.join(" ");
                    return Err(format!("No profile windows are available. {details}")
                        .trim()
                        .to_owned());
                }

                let audio = Audio {
                    enabled: draft.ambient_sound != "none",
                    noise: if draft.ambient_sound == "pink" {
                        Noise::Pink
                    } else {
                        Noise::Brown
                    },
                    volume: (draft.ambient_volume * 100.0).round() as u8,
                };
                let request = StartRequest {
                    goal: if draft.default_goal.is_empty() {
                        draft.name
                    } else {
                        draft.default_goal
                    },
                    windows: Vec::new(),
                    focus_minutes: draft.focus_minutes,
                    break_minutes: draft.break_minutes,
                    hall_passes: draft.hall_passes,
                    audio,
                    allowed_domains: draft.allowed_domains,
                    allow_localhost_domains: draft.allow_localhost_domains,
                };
                let state = app.state::<SessionState>();
                let mut session = state.lock().unwrap_or_else(|error| error.into_inner());
                let started = session.start_matched(&app, request, allowed);
                if started.is_ok() {
                    if let Err(error) = session.store()?.mark_profile_used(&profile.id) {
                        warnings.push(report("update the profile's last-used time", error));
                    }
                    session.profile_warning = (!warnings.is_empty()).then(|| warnings.join(" "));
                }
                session.publish(&app);
                started.map(|()| LaunchResult { warnings })
            })();
            let _ = respond.send(result);
        })
        .map_err(|error| report("reach the session controller", error))?;
    result
        .await
        .map_err(|error| report("start the study profile", error))?
}

fn report(context: &str, error: impl std::fmt::Display) -> String {
    let message = format!("Could not {context}: {error}");
    eprintln!("{message}");
    message
}
