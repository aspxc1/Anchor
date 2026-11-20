use super::{hide, view, Controller};
use crate::{storage::Storage, win32::foreground};
use std::{io, path::PathBuf};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

impl Controller {
    pub fn configure(&mut self, app: &AppHandle) {
        match open_storage() {
            Ok(storage) => {
                match storage.preferences() {
                    Ok(preferences) => self.preferences = preferences,
                    Err(error) => self.report_error(format!(
                        "Could not read Anchor preferences: {error}. Defaults are loaded."
                    )),
                }
                if let Err(error) = storage.sync_markdown() {
                    self.report_error(format!(
                        "Could not retry a pending Markdown export: {error}"
                    ));
                }
                self.storage = Some(storage);
            }
            Err(error) => self.report_error(format!(
                "Could not open Anchor storage in %APPDATA%\\Anchor: {error}"
            )),
        }
        let hotkey = self.preferences.hotkey.clone();
        if let Err(error) = self.register_hotkey(app, &hotkey) {
            eprintln!("{error}");
            self.shortcut_error = Some(error);
        }
    }

    pub(super) fn show_capture(&mut self, app: &AppHandle) -> Result<(), String> {
        if self.capture_open {
            self.dismiss_capture(app);
            return Ok(());
        }
        self.store()?;
        let window = view(app, "braindump")?;
        let monitor = window
            .primary_monitor()
            .map_err(|error| format!("Could not locate the primary monitor: {error}"))?
            .ok_or("Windows did not report a primary monitor.")?;
        let area = monitor.work_area();
        let size = window
            .outer_size()
            .map_err(|error| format!("Could not measure quick capture: {error}"))?;
        let position = tauri::PhysicalPosition::new(
            area.position.x + (area.size.width as i32 - size.width as i32).max(0) / 2,
            area.position.y + (area.size.height as i32 - size.height as i32).max(0) / 2,
        );
        window
            .set_position(position)
            .map_err(|error| format!("Could not position quick capture: {error}"))?;
        self.return_after_capture = foreground::identify(foreground::current());
        window
            .show()
            .map_err(|error| format!("Could not show quick capture: {error}"))?;
        if let Err(error) = window.set_focus() {
            hide(app, "braindump");
            return Err(format!("Could not focus quick capture: {error}"));
        }
        self.capture_open = true;
        Ok(())
    }

    pub(super) fn dismiss_capture(&mut self, app: &AppHandle) {
        if !self.capture_open {
            return;
        }
        self.capture_open = false;
        let previous = self.return_after_capture.take();
        if self.challenge.is_some() {
            if let Ok(curtain) = view(app, "curtain") {
                if let Err(error) = curtain.set_focus() {
                    self.report_error(format!("Could not return to Emergency Unlock: {error}"));
                }
            }
        } else if self.timing.phase.is_running() {
            if let Err(error) = self.restore_focus(app) {
                self.report_error(error);
                if self.timing.phase == crate::timing::Phase::ActiveFocus {
                    if let Err(error) = super::show_curtain(app, foreground::current()) {
                        self.report_error(error);
                    }
                }
            }
        } else if let Some(id) = previous {
            if foreground::identify(id.hwnd) == Some(id) {
                if let Err(error) = foreground::activate(id) {
                    self.report_error(error);
                }
            }
        }
        hide(app, "braindump");
    }

    pub(super) fn save_capture(&mut self, app: &AppHandle, content: &str) -> Result<(), String> {
        if !self.capture_open {
            return Err("Open Brain Dump before saving a note.".into());
        }
        let normalized = content
            .trim()
            .replace("\r\n", "\n")
            .replace(['\r', '\n'], " ");
        if normalized.is_empty() || normalized.chars().count() > 1000 {
            return Err("Enter a note between 1 and 1,000 characters.".into());
        }
        let session_id = self
            .session_id
            .clone()
            .filter(|_| self.timing.phase.is_running());
        let storage = self
            .storage
            .as_mut()
            .ok_or("Anchor storage is unavailable. Check %APPDATA%\\Anchor and restart.")?;
        storage
            .save_note(session_id.as_deref(), &normalized, chrono::Local::now())
            .map_err(|error| format!("Could not save the note in SQLite: {error}"))?;
        if let Err(error) = storage.sync_markdown() {
            self.report_error(format!("The note is saved in SQLite. Anchor could not export it to Markdown and will retry: {error}"));
        }
        self.notes_revision += 1;
        self.dismiss_capture(app);
        Ok(())
    }

    fn register_hotkey(&mut self, app: &AppHandle, requested: &str) -> Result<(), String> {
        let requested = requested.trim();
        let shortcut: Shortcut = requested.parse().map_err(|error| {
            format!("Invalid shortcut. Use a combination like Ctrl+Shift+Space. ({error})")
        })?;
        if self.registered_hotkey == Some(shortcut) {
            return Ok(());
        }
        if shortcut.mods.is_empty() || requested.len() > 80 {
            return Err(
                "Choose a shortcut with at least one modifier, such as Ctrl or Alt.".into(),
            );
        }
        let manager = app.global_shortcut();
        manager.register(shortcut).map_err(|error| {
            format!("Could not register {requested}. Try an unused shortcut. ({error})")
        })?;
        if let Some(previous) = self.registered_hotkey {
            if let Err(error) = manager.unregister(previous) {
                if let Err(rollback) = manager.unregister(shortcut) {
                    eprintln!("Could not undo shortcut registration after an error: {rollback}");
                }
                return Err(format!("Could not release the previous shortcut: {error}"));
            }
        }
        self.registered_hotkey = Some(shortcut);
        Ok(())
    }

    pub(super) fn change_hotkey(&mut self, app: &AppHandle, hotkey: &str) -> Result<(), String> {
        self.store()?;
        self.register_hotkey(app, hotkey)?;
        let mut preferences = self.preferences.clone();
        let previous = preferences.hotkey.clone();
        preferences.hotkey = hotkey.trim().into();
        if let Err(error) = self.persist_preferences(preferences) {
            if let Err(rollback) = self.register_hotkey(app, &previous) {
                self.report_error(format!("Could not restore the old shortcut: {rollback}"));
            }
            return Err(error);
        }
        self.shortcut_error = None;
        Ok(())
    }
}

pub fn shortcut_pressed(app: &AppHandle) {
    let state = app.state::<super::SessionState>();
    let mut controller = state.lock().unwrap_or_else(|error| error.into_inner());
    if let Err(error) = controller.show_capture(app) {
        controller.report_error(error);
    }
    controller.publish(app);
}

pub fn unregister_shortcuts(app: &AppHandle) {
    if let Err(error) = app.global_shortcut().unregister_all() {
        eprintln!("Could not unregister Anchor shortcuts: {error}");
    }
}

fn open_storage() -> Result<Storage, Box<dyn std::error::Error + Send + Sync>> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| io::Error::other("Windows did not provide APPDATA"))?;
    let storage = Storage::open(&PathBuf::from(appdata).join("Anchor"))?;
    storage.recover_sessions()?;
    Ok(storage)
}
