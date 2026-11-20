use crate::controller::{self, SessionState};
use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub struct Clock {
    stop: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Clock {
    fn start(app: AppHandle) -> std::io::Result<Self> {
        let (stop, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("anchor-clock".into())
            .spawn(move || {
                while let Err(mpsc::RecvTimeoutError::Timeout) =
                    receiver.recv_timeout(Duration::from_secs(1))
                {
                    let app = app.clone();
                    if let Err(error) = app.clone().run_on_main_thread(move || {
                        let state = app.state::<SessionState>();
                        state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .tick(&app);
                    }) {
                        eprintln!("Could not deliver timer tick: {error}");
                        break;
                    }
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            if self.stop.send(()).is_err() && !thread.is_finished() {
                eprintln!("The session timer stop channel closed");
            }
            if thread.join().is_err() {
                eprintln!("The session timer panicked");
            }
        }
    }
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.state::<SessionState>()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .configure(app.handle());
    let dashboard = MenuItem::with_id(app, "dashboard", "Show Dashboard", true, None::<&str>)?;
    let unlock = MenuItem::with_id(app, "unlock", "Emergency Unlock", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&dashboard, &unlock, &quit])?;
    let icon = app
        .default_window_icon()
        .ok_or("Anchor's tray icon is missing")?
        .clone();
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Anchor")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "dashboard" => controller::show_dashboard(app),
            "unlock" => {
                let state = app.state::<SessionState>();
                let mut session = state.lock().unwrap_or_else(|error| error.into_inner());
                session.tray_unlock(app);
            }
            "quit" => {
                let state = app.state::<SessionState>();
                state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .before_quit(app);
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    app.manage(tray);
    app.manage(std::sync::Mutex::new(Some(Clock::start(
        app.handle().clone(),
    )?)));
    Ok(())
}

pub fn shutdown(app: &AppHandle) {
    if let Some(clock) = app.try_state::<std::sync::Mutex<Option<Clock>>>() {
        clock
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}
