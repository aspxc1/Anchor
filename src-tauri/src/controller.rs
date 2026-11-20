mod browser;
pub(crate) mod capture;
pub(crate) mod hud;
pub(crate) mod library;

use crate::{
    policy::{self, Decision, WindowId},
    storage::{history::BlockedRecord, Audio, Preferences, Storage},
    timing::{Phase, Timing},
    win32::{
        foreground,
        hook::{ForegroundHook, HookEvent},
    },
    OpenWindow,
};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Instant, SystemTime},
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[derive(Clone, Serialize)]
pub struct BlockedSwitch {
    pub process_name: String,
    pub window_title: String,
    #[serde(skip)]
    pub occurred_at: SystemTime,
}

#[derive(Serialize, Clone)]
pub struct Snapshot {
    revision: u64,
    last_finished_session: Option<String>,
    notes_revision: u64,
    profile_warning: Option<String>,
    capture_open: bool,
    preferences: Preferences,
    shortcut_error: Option<String>,
    phase: &'static str,
    seconds_left: u64,
    hall_passes_remaining: u8,
    allowed_domains: Vec<String>,
    allow_localhost_domains: bool,
    actual_focus_seconds: u64,
    allowed: Vec<OpenWindow>,
    goal: String,
    blocked_count: usize,
    blocked: Option<BlockedSwitch>,
    needs_replacement: bool,
    challenge: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct SelectedWindow {
    hwnd: isize,
    pid: u32,
}

#[derive(Deserialize)]
pub struct StartRequest {
    pub(crate) goal: String,
    pub(crate) windows: Vec<SelectedWindow>,
    pub(crate) focus_minutes: u32,
    pub(crate) break_minutes: u32,
    pub(crate) hall_passes: u8,
    pub(crate) audio: Audio,
    #[serde(default)]
    pub(crate) allowed_domains: Vec<String>,
    #[serde(default = "default_allow_localhost")]
    pub(crate) allow_localhost_domains: bool,
}

fn default_allow_localhost() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Snapshot,
    Start(StartRequest),
    UpdateDomains {
        domains: Vec<String>,
        allow_localhost: bool,
    },
    SetBridgePort {
        port: u16,
    },
    RotateBridgeToken,
    OpenCapture,
    DismissCapture,
    SaveCapture {
        content: String,
    },
    ChangeHotkey {
        hotkey: String,
    },
    ChangeAudio {
        audio: Audio,
    },
    ToggleAudio,
    AddWindow {
        window: SelectedWindow,
    },
    UseHallPass,
    EndHallPass,
    NextSprint,
    EndSession,
    ShowRecovery,
    ShowDashboard,
    ReturnToFocus,
    ReplaceWindow {
        window: SelectedWindow,
    },
    OpenEmergency,
    CloseEmergency,
    TypeChallenge {
        value: String,
    },
    BeginHold,
    CancelHold,
    ConfirmHold,
}

pub type SessionState = Mutex<Controller>;

#[derive(Default)]
pub struct Controller {
    timing: Timing,
    pub(crate) profile_launching: bool,
    pub(crate) profile_warning: Option<String>,
    bridge: Option<crate::bridge::Bridge>,
    allowed_domains: Vec<String>,
    restrict_localhost: bool,
    hud_drag: Option<hud::Drag>,
    storage: Option<Storage>,
    session_id: Option<String>,
    last_finished_session: Option<String>,
    notes_revision: u64,
    preferences: Preferences,
    shortcut_error: Option<String>,
    registered_hotkey: Option<tauri_plugin_global_shortcut::Shortcut>,
    capture_open: bool,
    return_after_capture: Option<WindowId>,
    goal: String,
    allowed: Vec<OpenWindow>,
    last_allowed: Option<WindowId>,
    blocked_count: usize,
    pending_switches: Vec<BlockedRecord>,
    violation: Option<BlockedSwitch>,
    challenge: Option<String>,
    hold_started: Option<Instant>,
    error: Option<String>,
    hook: Option<ForegroundHook>,
    generation: u64,
    revision: u64,
}

impl Controller {
    fn snapshot(&self) -> Snapshot {
        let now = Instant::now();
        Snapshot {
            revision: self.revision,
            last_finished_session: self.last_finished_session.clone(),
            notes_revision: self.notes_revision,
            profile_warning: self.profile_warning.clone(),
            capture_open: self.capture_open,
            preferences: self.preferences.clone(),
            shortcut_error: self.shortcut_error.clone(),
            phase: self.timing.phase.name(),
            seconds_left: self.timing.seconds_left(now),
            hall_passes_remaining: self.timing.passes_remaining,
            allowed_domains: self.allowed_domains.clone(),
            allow_localhost_domains: !self.restrict_localhost,
            actual_focus_seconds: self.timing.actual_focus(now).as_secs(),
            allowed: self.allowed.clone(),
            goal: self.goal.clone(),
            blocked_count: self.blocked_count,
            blocked: self.violation.clone(),
            needs_replacement: self.timing.phase == Phase::ActiveFocus
                && self.focus_target().is_none(),
            challenge: self.challenge.clone(),
            error: self.error.clone(),
        }
    }

    pub(crate) fn publish(&mut self, app: &AppHandle) -> Snapshot {
        self.revision += 1;
        let snapshot = self.snapshot();
        if let Some(bridge) = &self.bridge {
            bridge.publish(self.bridge_status());
        }
        if let Err(error) = app.emit("session-state", &snapshot) {
            eprintln!("Could not broadcast session state: {error}");
        }
        snapshot
    }

    fn start(&mut self, app: &AppHandle, mut request: StartRequest) -> Result<(), String> {
        if self.profile_launching || self.is_running() {
            return Err("Finish the current session or profile launch first.".into());
        }
        let allowed = validate_selection(std::mem::take(&mut request.windows))?;
        self.start_matched(app, request, allowed)
    }

    pub(crate) fn is_running(&self) -> bool {
        self.timing.phase.is_running()
    }

    pub(crate) fn start_matched(
        &mut self,
        app: &AppHandle,
        request: StartRequest,
        allowed: Vec<OpenWindow>,
    ) -> Result<(), String> {
        let StartRequest {
            goal,
            windows: _,
            focus_minutes,
            break_minutes,
            hall_passes,
            audio,
            allowed_domains,
            allow_localhost_domains,
        } = request;
        if self.timing.phase.is_running() {
            return Err("A focus session is already active.".into());
        }
        let goal = goal.trim().to_owned();
        if goal.is_empty() || goal.chars().count() > 240 {
            return Err("Enter a session goal between 1 and 240 characters.".into());
        }
        if !(1..=180).contains(&focus_minutes)
            || !(1..=60).contains(&break_minutes)
            || hall_passes > 20
            || audio.volume > 100
        {
            return Err(
                "Choose 1–180 focus minutes, 1–60 break minutes, and up to 20 Hall Passes.".into(),
            );
        }
        let allowed_domains = crate::bridge::validate_domains(allowed_domains)?;
        let first = allowed
            .first()
            .ok_or("Select at least one open window.")?
            .id();
        let main = view(app, "main")?;
        let hud = view(app, "hud")?;
        hud::position(&hud, self.store()?)?;
        let mut preferences = self.preferences.clone();
        preferences.focus_minutes = focus_minutes;
        preferences.break_minutes = break_minutes;
        preferences.hall_passes = hall_passes;
        preferences.audio = audio;
        self.persist_preferences(preferences)?;
        self.start_hook(app)?;
        let started = (|| {
            let json = serde_json::to_string(&allowed)
                .map_err(|error| format!("Could not prepare allowlist: {error}"))?;
            self.store()?
                .start_session(&goal, focus_minutes, &json)
                .map_err(|error| format!("Could not create the session record: {error}"))
        })();
        match started {
            Ok(id) => self.session_id = Some(id),
            Err(error) => {
                self.hook.take();
                return Err(error);
            }
        }
        self.timing = Timing::new(focus_minutes, break_minutes, hall_passes, Instant::now());
        self.last_finished_session = None;
        self.profile_warning = None;
        self.allowed_domains = allowed_domains;
        self.restrict_localhost = !allow_localhost_domains;
        self.goal = goal;
        self.allowed = allowed;
        self.last_allowed = Some(first);
        self.blocked_count = 0;
        self.violation = None;
        self.error = None;
        if let Err(error) = hud.show() {
            self.finish(app, Phase::Aborted);
            return Err(format!("Could not show the session HUD: {error}"));
        }
        if let Err(error) = foreground::activate(first) {
            eprintln!("Could not activate the first allowed window: {error}");
        }
        report_window(main.hide(), "hide the dashboard");
        self.inspect(app, foreground::current());
        Ok(())
    }

    fn start_hook(&mut self, app: &AppHandle) -> Result<(), String> {
        self.generation += 1;
        let generation = self.generation;
        let app = app.clone();
        self.hook = Some(ForegroundHook::start(move |event| {
            let app = app.clone();
            if let Err(error) = app.clone().run_on_main_thread(move || {
                let state = app.state::<SessionState>();
                let mut session = state.lock().unwrap_or_else(|error| error.into_inner());
                if session.generation != generation || session.timing.phase != Phase::ActiveFocus {
                    return;
                }
                match event {
                    HookEvent::Foreground(hwnd) => session.inspect(&app, hwnd),
                    HookEvent::Failed(error) => {
                        eprintln!("{error}");
                        session.finish(&app, Phase::Aborted);
                        session.error = Some(error);
                    }
                }
                session.publish(&app);
            }) {
                eprintln!("Could not dispatch the foreground event: {error}");
            }
        })?);
        Ok(())
    }

    fn inspect(&mut self, app: &AppHandle, hwnd: isize) {
        if self.timing.phase != Phase::ActiveFocus {
            return;
        }
        let Some(window) = foreground::identify(hwnd) else {
            return;
        };
        if window.pid == std::process::id() {
            return;
        }
        let class = foreground::class_name(window);
        if matches!(
            class.as_str(),
            "TaskManagerWindow" | "TaskManagerWindowModern"
        ) {
            self.dismiss_curtain(app);
            return;
        }
        let allowed = self.allowed.iter().map(OpenWindow::id).collect::<Vec<_>>();
        match policy::evaluate(
            &allowed,
            window,
            &foreground::owners(hwnd),
            foreground::shell_class(&class),
        ) {
            Decision::Ignore => {}
            Decision::Allow(id) => {
                self.last_allowed = Some(id);
                if foreground::current() == hwnd && self.challenge.is_none() {
                    self.violation = None;
                    hide(app, "curtain");
                }
            }
            Decision::Block => {
                let (process_name, window_title) = foreground::describe(window);
                if process_name.eq_ignore_ascii_case("taskmgr.exe") {
                    self.dismiss_curtain(app);
                    return;
                }
                let blocked = BlockedSwitch {
                    process_name,
                    window_title,
                    occurred_at: SystemTime::now(),
                };
                eprintln!(
                    "Blocked switch at {:?} to {}",
                    blocked.occurred_at, blocked.process_name
                );
                self.record_blocked_switch(&blocked);
                if foreground::current() == hwnd {
                    self.violation = Some(blocked);
                    if let Err(error) = show_curtain(app, hwnd) {
                        self.error = Some(error.clone());
                        eprintln!("{error}");
                        self.finish(app, Phase::Aborted);
                    }
                }
            }
        }
    }

    fn record_blocked_switch(&mut self, blocked: &BlockedSwitch) {
        self.blocked_count += 1;
        if let Some(session_id) = self.session_id.clone() {
            self.pending_switches.push(BlockedRecord {
                session_id,
                occurred_at: chrono::DateTime::<chrono::Local>::from(blocked.occurred_at)
                    .to_rfc3339(),
                source: crate::storage::history::BlockSource::Desktop,
                process_name: blocked.process_name.clone(),
                window_title: blocked.window_title.clone(),
            });
            self.flush_switches();
        }
    }

    fn flush_switches(&mut self) {
        if self.pending_switches.is_empty() {
            return;
        }
        if let Some(storage) = self.storage.as_mut() {
            match storage.record_switches(&self.pending_switches) {
                Ok(()) => self.pending_switches.clear(),
                Err(error) => self.report_error(format!(
                    "Could not record blocked switches in SQLite. Anchor will retry: {error}"
                )),
            }
        }
    }

    fn focus_target(&self) -> Option<WindowId> {
        self.last_allowed
            .into_iter()
            .chain(self.allowed.iter().map(OpenWindow::id))
            .find(|id| foreground::identify(id.hwnd) == Some(*id))
    }

    fn restore_focus(&mut self, app: &AppHandle) -> Result<(), String> {
        let target = self
            .focus_target()
            .ok_or("All allowed windows have closed. Pick a replacement or unlock the session.")?;
        let result = foreground::activate(target);
        if result.is_ok() {
            self.last_allowed = Some(target);
            self.violation = None;
            hide(app, "curtain");
        }
        result
    }

    fn dismiss_curtain(&mut self, app: &AppHandle) {
        self.close_emergency();
        self.violation = None;
        hide(app, "curtain");
    }

    fn close_emergency(&mut self) {
        self.challenge = None;
        self.hold_started = None;
    }

    pub(crate) fn store(&self) -> Result<&Storage, String> {
        self.storage.as_ref().ok_or_else(|| {
            "Anchor storage is unavailable. Check %APPDATA%\\Anchor and restart.".into()
        })
    }

    fn report_error(&mut self, error: String) {
        eprintln!("{error}");
        self.error = Some(error);
    }

    fn persist_preferences(&mut self, preferences: Preferences) -> Result<(), String> {
        self.store()?
            .save_preferences(&preferences)
            .map_err(|error| format!("Could not save your preferences: {error}"))?;
        self.preferences = preferences;
        Ok(())
    }

    fn change_audio(&mut self, audio: Audio) -> Result<(), String> {
        if audio.volume > 100 {
            return Err("Choose a volume from 0 to 100.".into());
        }
        let mut preferences = self.preferences.clone();
        preferences.audio = audio;
        self.persist_preferences(preferences)
    }

    fn finish(&mut self, app: &AppHandle, phase: Phase) {
        self.complete_session(app, phase);
        show_dashboard(app);
    }

    fn complete_session(&mut self, app: &AppHandle, phase: Phase) {
        let now = Instant::now();
        self.timing.stop(phase, now);
        self.hook.take();
        self.close_emergency();
        self.violation = None;
        self.hud_drag = None;
        self.capture_open = false;
        self.return_after_capture = None;
        hide(app, "braindump");
        hide(app, "curtain");
        hide(app, "hud");
        self.flush_switches();
        if let Some(id) = self.session_id.take() {
            let result = (|| {
                let apps =
                    serde_json::to_string(&self.allowed).map_err(|error| error.to_string())?;
                self.store()?
                    .finish_session(
                        &id,
                        if phase == Phase::Completed {
                            "completed"
                        } else {
                            "aborted"
                        },
                        self.timing.actual_focus(now).as_secs(),
                        self.timing.passes_used,
                        &apps,
                    )
                    .map_err(|error| error.to_string())
            })();
            match result {
                Ok(()) => self.last_finished_session = Some(id),
                Err(error) => self.report_error(format!(
                    "The session ended, but Anchor could not update its database record: {error}"
                )),
            }
        }
    }

    fn apply(&mut self, app: &AppHandle, source: &str, action: Action) -> Result<(), String> {
        if !matches!(action, Action::Snapshot) && self.timing.phase.is_running() {
            self.advance(app, Instant::now());
        }
        match action {
            Action::Snapshot => return Ok(()),
            Action::UpdateDomains {
                domains,
                allow_localhost,
            } if source == "main" && self.timing.phase == Phase::Break => {
                self.allowed_domains = crate::bridge::validate_domains(domains)?;
                self.restrict_localhost = !allow_localhost;
            }
            Action::SetBridgePort { port } if source == "main" => {
                self.set_bridge_port(app, port)?
            }
            Action::RotateBridgeToken if source == "main" => self.rotate_bridge_token()?,
            Action::OpenCapture if matches!(source, "main" | "hud") => self.show_capture(app)?,
            Action::DismissCapture if source == "braindump" => self.dismiss_capture(app),
            Action::SaveCapture { content } if source == "braindump" => {
                self.save_capture(app, &content)?
            }
            Action::ChangeHotkey { hotkey } if source == "main" => {
                self.change_hotkey(app, &hotkey)?
            }
            Action::ChangeAudio { audio } if source == "main" => self.change_audio(audio)?,
            Action::ToggleAudio if source == "hud" => {
                let mut audio = self.preferences.audio;
                audio.enabled = !audio.enabled;
                self.change_audio(audio)?;
            }
            Action::ShowDashboard if source == "hud" => show_dashboard(app),
            Action::Start(request) if source == "main" => self.start(app, request)?,
            Action::ReturnToFocus
                if source == "curtain" && self.timing.phase == Phase::ActiveFocus =>
            {
                self.restore_focus(app)?
            }
            Action::ShowRecovery if source == "hud" && self.timing.phase == Phase::ActiveFocus => {
                if self.focus_target().is_none() {
                    show_curtain(app, foreground::current())?;
                }
            }
            Action::ReplaceWindow { window }
                if source == "curtain" && self.timing.phase == Phase::ActiveFocus =>
            {
                if self.focus_target().is_some() {
                    return Err("An allowed window is still available.".into());
                }
                self.allowed = validate_selection(vec![window])?;
                self.last_allowed = None;
                self.restore_focus(app)?;
            }
            Action::AddWindow { window }
                if source == "main" && self.timing.phase == Phase::Break =>
            {
                let mut additional = validate_selection(vec![window])?;
                let window = additional.pop().ok_or("Choose an open window.")?;
                if !self.allowed.iter().any(|item| item.id() == window.id()) {
                    self.allowed.push(window);
                }
            }
            Action::UseHallPass if matches!(source, "hud" | "curtain") => {
                self.use_hall_pass(app)?;
            }
            Action::EndHallPass if source == "hud" && self.timing.phase == Phase::HallPass => {
                self.timing.resume(Instant::now());
                self.resume_enforcement(app, false)?;
            }
            Action::NextSprint
                if matches!(source, "hud" | "main") && self.timing.phase == Phase::Break =>
            {
                self.timing.resume(Instant::now());
                hide(app, "main");
                self.resume_enforcement(app, true)?;
            }
            Action::EndSession
                if matches!(source, "hud" | "main") && self.timing.phase == Phase::Break =>
            {
                self.finish(app, Phase::Completed);
            }
            Action::OpenEmergency
                if matches!(source, "curtain" | "hud" | "main")
                    && self.timing.phase.is_running() =>
            {
                self.open_emergency(app, source != "curtain")?;
            }
            Action::CloseEmergency if source == "curtain" => {
                self.close_emergency();
                if self.violation.is_none()
                    && (self.timing.phase != Phase::ActiveFocus || self.focus_target().is_some())
                {
                    hide(app, "curtain");
                    self.inspect(app, foreground::current());
                }
            }
            Action::TypeChallenge { value }
                if source == "curtain" && self.timing.phase.is_running() =>
            {
                if self.challenge.as_deref() != Some(value.as_str()) {
                    return Err(
                        "The code does not match. Check each character and try again.".into(),
                    );
                }
                self.finish(app, Phase::Aborted);
            }
            Action::BeginHold if source == "curtain" => {
                if self.challenge.is_none() {
                    return Err("Open Emergency Unlock before holding the button.".into());
                }
                self.hold_started = Some(Instant::now());
            }
            Action::CancelHold if source == "curtain" => self.hold_started = None,
            Action::ConfirmHold if source == "curtain" && self.timing.phase.is_running() => {
                if self.challenge.is_none()
                    || !policy::hold_complete(self.hold_started.take(), Instant::now())
                {
                    return Err("Hold the button continuously for 10 seconds.".into());
                }
                self.finish(app, Phase::Aborted);
            }
            _ => return Err("That control is unavailable in the current phase.".into()),
        }
        Ok(())
    }

    fn open_emergency(&mut self, app: &AppHandle, show: bool) -> Result<(), String> {
        if !self.timing.phase.is_running() {
            return Ok(());
        }
        if show {
            show_curtain(app, foreground::current())?;
        }
        self.challenge = Some(Alphanumeric.sample_string(&mut rand::rng(), 24));
        self.hold_started = None;
        Ok(())
    }

    pub fn tray_unlock(&mut self, app: &AppHandle) {
        if let Err(error) = self.open_emergency(app, true) {
            eprintln!("{error}");
            self.error = Some(error);
            show_dashboard(app);
        }
        self.publish(app);
    }

    fn use_hall_pass(&mut self, app: &AppHandle) -> Result<(), String> {
        if !self.timing.use_pass(Instant::now()) {
            return Err("Hall Passes are available during focus with a pass remaining.".into());
        }
        self.hook.take();
        self.dismiss_curtain(app);
        Ok(())
    }

    fn resume_enforcement(&mut self, app: &AppHandle, restore: bool) -> Result<(), String> {
        if let Err(error) = self.start_hook(app) {
            self.error = Some(error.clone());
            self.finish(app, Phase::Aborted);
            return Err(error);
        }
        if let Some(target) = self.focus_target() {
            if restore && self.challenge.is_none() {
                if let Err(error) = foreground::activate(target) {
                    eprintln!("{error}");
                }
            }
            self.inspect(app, foreground::current());
        } else if let Err(error) = show_curtain(app, foreground::current()) {
            self.error = Some(error.clone());
            self.finish(app, Phase::Aborted);
            return Err(error);
        }
        Ok(())
    }

    fn advance(&mut self, app: &AppHandle, now: Instant) {
        let phase = self.timing.phase;
        self.timing.tick(now);
        if phase == self.timing.phase {
            return;
        }
        match self.timing.phase {
            Phase::Break => {
                self.hook.take();
                self.violation = None;
                if self.challenge.is_none() {
                    hide(app, "curtain");
                }
            }
            Phase::ActiveFocus => {
                if phase == Phase::Break {
                    hide(app, "main");
                }
                if let Err(error) = self.resume_enforcement(app, false) {
                    eprintln!("{error}");
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, app: &AppHandle) {
        if self.timing.phase.is_running() {
            self.advance(app, Instant::now());
            self.publish(app);
        }
    }

    pub fn before_quit(&mut self, app: &AppHandle) {
        self.bridge.take();
        if self.timing.phase.is_running() {
            self.complete_session(app, Phase::Aborted);
        } else {
            self.flush_switches();
        }
    }
}

impl OpenWindow {
    fn id(&self) -> WindowId {
        WindowId {
            hwnd: self.hwnd,
            pid: self.pid,
        }
    }
}

fn validate_selection(selected: Vec<SelectedWindow>) -> Result<Vec<OpenWindow>, String> {
    if selected.is_empty() {
        return Err("Select at least one open window.".into());
    }
    let discovered = crate::list_open_windows()?;
    let mut allowed = Vec::new();
    for window in selected {
        let found = discovered
            .iter()
            .find(|item| item.hwnd == window.hwnd && item.pid == window.pid)
            .ok_or(
                "A selected window is no longer available. Refresh the list and select it again.",
            )?;
        if !allowed
            .iter()
            .any(|window: &OpenWindow| window.id() == found.id())
        {
            allowed.push(found.clone());
        }
    }
    Ok(allowed)
}

fn view(app: &AppHandle, label: &str) -> Result<WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("Anchor could not find its {label} window. Restart the app."))
}

pub fn show_dashboard(app: &AppHandle) {
    match view(app, "main") {
        Ok(main) => {
            report_window(main.show(), "show the dashboard");
            report_window(main.unminimize(), "restore the dashboard");
            report_window(main.set_focus(), "focus the dashboard");
        }
        Err(error) => eprintln!("{error}"),
    }
}

fn show_curtain(app: &AppHandle, target: isize) -> Result<(), String> {
    let window = view(app, "curtain")?;
    let hwnd = window
        .hwnd()
        .map_err(|error| format!("Could not access the curtain window: {error}"))?;
    if !window
        .is_visible()
        .map_err(|error| format!("Could not inspect the curtain: {error}"))?
    {
        foreground::position_overlay(hwnd.0 as isize, target)?;
        // Let Tauri track visibility so its later hide call updates the native window.
        window
            .show()
            .map_err(|error| format!("Could not show the focus curtain: {error}"))?;
    }
    foreground::position_overlay(hwnd.0 as isize, target)?;
    foreground::focus_overlay(hwnd.0 as isize);
    Ok(())
}

fn hide(app: &AppHandle, label: &str) {
    match view(app, label) {
        Ok(window) => report_window(window.hide(), &format!("hide the {label} window")),
        Err(error) => eprintln!("{error}"),
    }
}

fn report_window(result: tauri::Result<()>, context: &str) {
    if let Err(error) = result {
        eprintln!("Could not {context}: {error}");
    }
}

#[tauri::command]
pub async fn session_command(window: WebviewWindow, action: Action) -> Result<Snapshot, String> {
    let app = window.app_handle().clone();
    let source = window.label().to_owned();
    let (sender, receiver) = tauri::async_runtime::channel(1);
    app.clone()
        .run_on_main_thread(move || {
            let state = app.state::<SessionState>();
            let mut session = state.lock().unwrap_or_else(|error| error.into_inner());
            let read_only = matches!(action, Action::Snapshot);
            let result = session.apply(&app, &source, action);
            let snapshot = if read_only {
                session.snapshot()
            } else {
                session.publish(&app)
            };
            if let Err(error) = result.as_ref() {
                eprintln!("Session command from {source} failed: {error}");
            }
            if sender.try_send(result.map(|()| snapshot)).is_err() {
                eprintln!("Session command receiver closed");
            }
        })
        .map_err(|error| format!("Could not reach the session controller: {error}"))?;
    let mut receiver = receiver;
    receiver
        .recv()
        .await
        .ok_or("The session controller closed unexpectedly.")?
}
