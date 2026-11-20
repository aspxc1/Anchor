use super::{ProfileApp, ProfileDraft};
use crate::OpenWindow;
use std::{
    collections::HashSet,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn normalized_path(path: &str) -> String {
    path.trim().replace('/', "\\").to_lowercase()
}

fn matching_windows<'a>(app: &ProfileApp, windows: &'a [OpenWindow]) -> Vec<&'a OpenWindow> {
    let desired = normalized_path(&app.executable_path);
    let hint = app.title_hint.as_deref().map(str::to_lowercase);
    let candidates: Vec<_> = windows
        .iter()
        .filter(|window| {
            window
                .process_name
                .trim()
                .eq_ignore_ascii_case(app.process_name.trim())
                && hint
                    .as_ref()
                    .is_none_or(|hint| window.title.to_lowercase().contains(hint))
                && (desired.is_empty()
                    || window.executable_path.is_empty()
                    || path_matches(&desired, &window.executable_path))
        })
        .collect();
    let verified = !desired.is_empty()
        && candidates
            .iter()
            .any(|window| path_matches(&desired, &window.executable_path));
    candidates
        .into_iter()
        .filter(|window| !verified || path_matches(&desired, &window.executable_path))
        .collect()
}

fn path_matches(desired: &str, actual: &str) -> bool {
    let actual = normalized_path(actual);
    desired == actual
        || (desired.ends_with("\\system32\\notepad.exe")
            && actual.contains("\\windowsapps\\microsoft.windowsnotepad_")
            && actual.ends_with("\\notepad.exe"))
}

fn collect_matches(
    app: &ProfileApp,
    windows: &[OpenWindow],
    used: &mut HashSet<isize>,
    matched: &mut Vec<OpenWindow>,
) -> bool {
    let found = matching_windows(app, windows);
    for window in &found {
        if used.insert(window.hwnd) {
            matched.push((*window).clone());
        }
    }
    !found.is_empty()
}

pub async fn bind_profile(
    profile: &ProfileDraft,
) -> Result<(Vec<OpenWindow>, Vec<String>), String> {
    let mut windows = crate::list_open_windows()?;
    let mut matched = Vec::new();
    let mut used = HashSet::new();
    let mut pending = Vec::new();
    let mut warnings = Vec::new();
    let mut launched = HashSet::new();
    for app in &profile.allowed_apps {
        if collect_matches(app, &windows, &mut used, &mut matched) {
            continue;
        } else if app.launch_if_closed {
            if !launched.insert(normalized_path(&app.executable_path)) {
                pending.push(app);
                continue;
            }
            match Command::new(&app.executable_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(_) => pending.push(app),
                Err(error) => {
                    warnings.push(format!("Could not launch {}: {error}", app.process_name))
                }
            }
        } else {
            warnings.push(format!(
                "No matching window found for {}.",
                app.process_name
            ));
        }
    }
    let deadline = Instant::now() + Duration::from_secs(6);
    while !pending.is_empty() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
        match crate::list_open_windows() {
            Ok(found) => windows = found,
            Err(error) => {
                warnings.push(format!("Could not refresh windows: {error}"));
                break;
            }
        }
        pending.retain(|app| !collect_matches(app, &windows, &mut used, &mut matched));
    }
    for app in pending {
        warnings.push(format!(
            "No matching window appeared for {}.",
            app.process_name
        ));
    }
    Ok((matched, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window(path: &str, title: &str, hwnd: isize) -> OpenWindow {
        OpenWindow {
            hwnd,
            title: title.into(),
            process_name: "EDITOR.EXE".into(),
            executable_path: path.into(),
            pid: 1,
        }
    }
    #[test]
    fn matching_prefers_verified_path_and_rejects_collisions() {
        let app = ProfileApp {
            process_name: "editor.exe".into(),
            executable_path: "C:\\App\\editor.exe".into(),
            launch_if_closed: false,
            title_hint: Some("document".into()),
        };
        let windows = vec![
            window("", "Document", 1),
            window("C:\\Other\\editor.exe", "Document", 2),
            window("c:/app/editor.exe", "DOCUMENT", 3),
        ];
        assert_eq!(matching_windows(&app, &windows)[0].hwnd, 3);
        assert!(matching_windows(&app, &windows[1..2]).is_empty());
    }
}
