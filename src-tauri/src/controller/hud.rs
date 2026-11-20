use super::{SessionState, Storage};
use crate::win32::{foreground, hud};
use serde::{Deserialize, Serialize};
use tauri::{Manager, Monitor, WebviewWindow};

const SETTING: &str = "hud_placement";
const TOP_PADDING: i32 = 8;

#[derive(Deserialize, Serialize)]
struct Placement {
    monitor_index: usize,
    monitor_name: Option<String>,
    offset_x: i32,
}

pub(super) struct Drag {
    cursor_x: i32,
    left: i32,
    top: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DragAction {
    Begin,
    Move,
    End,
    Cancel,
}

fn handle(window: &WebviewWindow) -> Result<isize, String> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|error| format!("Could not access the HWND for the HUD: {error}"))
}

fn monitors(window: &WebviewWindow) -> Result<Vec<Monitor>, String> {
    let mut monitors = window
        .available_monitors()
        .map_err(|error| format!("Could not list displays: {error}"))?;
    monitors.sort_by_key(|monitor| (monitor.position().x, monitor.position().y));
    Ok(monitors)
}

pub(super) fn position(window: &WebviewWindow, storage: &Storage) -> Result<(), String> {
    let stored = storage
        .setting(SETTING)
        .map_err(|error| format!("Could not read the HUD position: {error}"))?;
    let stored: Option<Placement> = stored.and_then(|json| match serde_json::from_str(&json) {
        Ok(placement) => Some(placement),
        Err(error) => {
            eprintln!("Could not decode the stored HUD position: {error}");
            None
        }
    });
    let monitors = monitors(window)?;
    let saved = stored.as_ref().and_then(|placement| {
        let indexed = monitors.get(placement.monitor_index);
        if placement.monitor_name.is_none()
            || indexed.is_some_and(|monitor| monitor.name() == placement.monitor_name.as_ref())
        {
            indexed.map(|monitor| (monitor, placement.offset_x))
        } else {
            monitors
                .iter()
                .find(|monitor| monitor.name() == placement.monitor_name.as_ref())
                .map(|monitor| (monitor, placement.offset_x))
        }
    });
    if let Some((monitor, offset)) = saved {
        snap(handle(window)?, monitor, Some(offset))
    } else {
        let primary = window
            .primary_monitor()
            .map_err(|error| format!("Could not find the primary monitor: {error}"))?
            .ok_or("Windows did not report a primary monitor.")?;
        snap(handle(window)?, &primary, None)
    }
}

fn snap(raw: isize, monitor: &Monitor, offset: Option<i32>) -> Result<(), String> {
    let area = monitor.work_area();
    // Moving across displays may resize the HUD in WM_DPICHANGED. Clamp again
    // with its actual width on the destination display.
    for _ in 0..2 {
        let rect = hud::rect(raw)?;
        let available = (area.size.width as i32 - (rect.right - rect.left)).max(0);
        let x = area.position.x + offset.unwrap_or(available / 2).clamp(0, available);
        hud::move_to(raw, x, area.position.y + TOP_PADDING)?;
    }
    Ok(())
}

fn snap_and_save(window: &WebviewWindow, storage: &Storage, raw: isize) -> Result<(), String> {
    let rect = hud::rect(raw)?;
    let center_x = f64::from(rect.left + (rect.right - rect.left) / 2);
    let center_y = f64::from(rect.top + (rect.bottom - rect.top) / 2);
    let area = foreground::monitor_rect(raw, true)?;
    let monitors = monitors(window)?;
    let center = window
        .monitor_from_point(center_x, center_y)
        .map_err(|error| format!("Could not find the HUD display: {error}"))?;
    let (index, monitor) = monitors
        .iter()
        .enumerate()
        .find(|(_, monitor)| {
            if let Some(center) = &center {
                return monitor.position() == center.position();
            }
            monitor.work_area().position.x == area.left
                && monitor.work_area().position.y == area.top
        })
        .ok_or("The HUD's monitor changed during the drag. Try again.")?;
    snap(
        raw,
        monitor,
        Some(rect.left - monitor.work_area().position.x),
    )?;
    let placement = Placement {
        monitor_index: index,
        monitor_name: monitor.name().cloned(),
        offset_x: hud::rect(raw)?.left - monitor.work_area().position.x,
    };
    let value = serde_json::to_string(&placement).map_err(|error| error.to_string())?;
    storage
        .set_setting(SETTING, &value)
        .map_err(|error| format!("Could not save the HUD position: {error}"))
}

#[tauri::command]
pub fn hud_drag(window: WebviewWindow, action: DragAction) -> Result<(), String> {
    if window.label() != "hud" {
        return Err("Drag the HUD from its grip.".into());
    }
    let app = window.app_handle();
    let state = app.state::<SessionState>();
    let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
    let raw = handle(&window)?;
    let result = match action {
        DragAction::Begin => {
            let rect = hud::rect(raw)?;
            state.hud_drag = Some(Drag {
                cursor_x: hud::cursor_x()?,
                left: rect.left,
                top: rect.top,
            });
            Ok(())
        }
        DragAction::Move => {
            if let Some(drag) = &state.hud_drag {
                move_drag(raw, drag)?;
            }
            Ok(())
        }
        DragAction::End | DragAction::Cancel => {
            if let Some(drag) = state.hud_drag.take() {
                if matches!(action, DragAction::End) {
                    move_drag(raw, &drag)?;
                }
                snap_and_save(&window, state.store()?, raw)?;
            }
            Ok(())
        }
    };
    result
}

fn move_drag(raw: isize, drag: &Drag) -> Result<(), String> {
    let delta = hud::cursor_x()?.saturating_sub(drag.cursor_x);
    hud::move_to(raw, drag.left.saturating_add(delta), drag.top)
}
