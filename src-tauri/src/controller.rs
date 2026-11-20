use crate::{
    policy::{self, Decision, WindowId},
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

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Phase {
    Idle,
    ActiveFocus,
    Aborted,
}

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
    phase: Phase,
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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Snapshot,
    Start {
        goal: String,
        windows: Vec<SelectedWindow>,
    },
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

pub struct Controller {
    phase: Phase,
    goal: String,
    allowed: Vec<OpenWindow>,
    last_allowed: Option<WindowId>,
    blocked: Vec<BlockedSwitch>,
    violation: Option<BlockedSwitch>,
    challenge: Option<String>,
    hold_started: Option<Instant>,
    error: Option<String>,
    hook: Option<ForegroundHook>,
    generation: u64,
    revision: u64,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            goal: String::new(),
            allowed: Vec::new(),
            last_allowed: None,
            blocked: Vec::new(),
            violation: None,
            challenge: None,
            hold_started: None,
            error: None,
            hook: None,
            generation: 0,
            revision: 0,
        }
    }
}

impl Controller {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            revision: self.revision,
            phase: self.phase,
            goal: self.goal.clone(),
            blocked_count: self.blocked.len(),
            blocked: self.violation.clone(),
            needs_replacement: self.phase == Phase::ActiveFocus && self.focus_target().is_none(),
            challenge: self.challenge.clone(),
            error: self.error.clone(),
        }
    }

    fn publish(&mut self, app: &AppHandle) -> Snapshot {
        self.revision += 1;
        let snapshot = self.snapshot();
        if let Err(error) = app.emit("session-state", &snapshot) {
            eprintln!("Could not broadcast session state: {error}");
        }
        snapshot
    }

    fn start(
        &mut self,
        app: &AppHandle,
        goal: String,
        selected: Vec<SelectedWindow>,
    ) -> Result<(), String> {
        if self.phase == Phase::ActiveFocus {
            return Err("A focus session is already active.".into());
        }
        let goal = goal.trim().to_owned();
        if goal.is_empty() || goal.chars().count() > 240 {
            return Err("Enter a session goal between 1 and 240 characters.".into());
        }
        let allowed = validate_selection(selected)?;
        let main = view(app, "main")?;
        let first = allowed[0].id();
        self.start_hook(app)?;
        self.phase = Phase::ActiveFocus;
        self.goal = goal;
        self.allowed = allowed;
        self.last_allowed = Some(first);
        self.blocked.clear();
        self.violation = None;
        self.error = None;
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
                if session.generation != generation || session.phase != Phase::ActiveFocus {
                    return;
                }
                match event {
                    HookEvent::Foreground(hwnd) => session.inspect(&app, hwnd),
                    HookEvent::Failed(error) => {
                        eprintln!("{error}");
                        session.finish(&app);
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
        if self.phase != Phase::ActiveFocus {
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
                self.blocked.push(blocked.clone());
                if foreground::current() == hwnd {
                    self.violation = Some(blocked);
                    if let Err(error) = show_curtain(app, hwnd) {
                        self.error = Some(error.clone());
                        eprintln!("{error}");
                        self.finish(app);
                    }
                }
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

    fn finish(&mut self, app: &AppHandle) {
        self.phase = Phase::Aborted;
        self.hook.take();
        self.close_emergency();
        self.violation = None;
        hide(app, "curtain");
        hide(app, "hud");
        if let Ok(main) = view(app, "main") {
            report_window(main.show(), "show the dashboard");
            report_window(main.set_focus(), "focus the dashboard");
        }
    }

    fn apply(&mut self, app: &AppHandle, source: &str, action: Action) -> Result<(), String> {
        if matches!(action, Action::Snapshot) {
            return Ok(());
        }
        if let Action::Start { goal, windows } = action {
            if source != "main" {
                return Err("Start a session from the dashboard.".into());
            }
            return self.start(app, goal, windows);
        }
        if self.phase != Phase::ActiveFocus || source != "curtain" {
            return Err("That control is unavailable right now.".into());
        }
        match action {
            Action::ReturnToFocus => self.restore_focus(app)?,
            Action::ReplaceWindow { window } => {
                if self.focus_target().is_some() {
                    return Err("An allowed window is still available.".into());
                }
                self.allowed = validate_selection(vec![window])?;
                self.last_allowed = None;
                self.restore_focus(app)?;
            }
            Action::OpenEmergency => {
                self.challenge = Some(Alphanumeric.sample_string(&mut rand::rng(), 24));
                self.hold_started = None;
            }
            Action::CloseEmergency => self.close_emergency(),
            Action::TypeChallenge { value } => {
                if self.challenge.as_deref() != Some(value.as_str()) {
                    return Err(
                        "The code does not match. Check each character and try again.".into(),
                    );
                }
                self.finish(app);
            }
            Action::BeginHold => {
                if self.challenge.is_none() {
                    return Err("Open Emergency Unlock before holding the button.".into());
                }
                self.hold_started = Some(Instant::now());
            }
            Action::CancelHold => self.hold_started = None,
            Action::ConfirmHold => {
                if self.challenge.is_none()
                    || !policy::hold_complete(self.hold_started.take(), Instant::now())
                {
                    return Err("Hold the button continuously for 10 seconds.".into());
                }
                self.finish(app);
            }
            Action::Start { .. } | Action::Snapshot => unreachable!(),
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.hook.take();
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

fn show_curtain(app: &AppHandle, target: isize) -> Result<(), String> {
    let window = view(app, "curtain")?;
    let hwnd = window
        .hwnd()
        .map_err(|error| format!("Could not access the curtain window: {error}"))?;
    foreground::show_overlay(hwnd.0 as isize, target)
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
