mod domains;
pub(crate) mod server;

pub use domains::{hostname_valid as valid_hostname, validate_domains};

use crate::controller::SessionState;
use rand::RngCore;
use serde::Serialize;
use server::{ApiError, Dispatch, Server, Status};
use std::sync::Arc;
use tauri::{AppHandle, Manager, WebviewWindow};
use tokio::sync::watch;

const DEFAULT_PORT: u16 = 17321;

#[derive(Serialize)]
pub struct Settings {
    port: u16,
    token: String,
    running: bool,
    connected: bool,
    error: Option<String>,
}

pub struct Bridge {
    port: u16,
    token: String,
    server: Option<Server>,
    error: Option<String>,
    status: watch::Sender<Status>,
}

impl Bridge {
    pub fn initialize(app: &AppHandle, port: u16, token: String, initial: Status) -> Self {
        let (status, _) = watch::channel(initial);
        let mut bridge = Self {
            port,
            token,
            server: None,
            error: None,
            status,
        };
        match bridge.bind(app, port) {
            Ok(server) => bridge.server = Some(server),
            Err(error) => {
                eprintln!("{error}");
                bridge.error = Some(error);
            }
        }
        bridge
    }

    fn bind(&self, app: &AppHandle, port: u16) -> Result<Server, String> {
        let app = app.clone();
        let dispatch: Dispatch = Arc::new(move |request, reply| {
            let app = app.clone();
            if let Err(error) = app.clone().run_on_main_thread(move || {
                let state = app.state::<SessionState>();
                let mut session = state.lock().unwrap_or_else(|error| error.into_inner());
                let result = session.bridge_request(&app, request);
                if reply.send(result).is_err() {
                    eprintln!(
                        "The localhost bridge client disconnected before receiving its result"
                    );
                }
            }) {
                eprintln!("Could not dispatch a localhost bridge request: {error}");
            }
        });
        let runtime = tauri::async_runtime::handle();
        let _runtime = runtime.inner().enter();
        server::start(port, self.token.clone(), self.status.subscribe(), dispatch)
            .map_err(|error| format!("Could not open the localhost bridge on port {port}: {error}. Choose a different port in Settings."))
    }

    pub fn publish(&self, status: Status) {
        self.status.send_replace(status);
    }

    pub fn settings(&self) -> Settings {
        Settings {
            port: self.port,
            token: self.token.clone(),
            running: self.server.is_some(),
            connected: self.server.as_ref().is_some_and(Server::connected),
            error: self.error.clone(),
        }
    }

    pub fn prepare_port(&self, app: &AppHandle, port: u16) -> Result<Option<Server>, String> {
        if port < 1024 {
            return Err("Choose a port between 1024 and 65535.".into());
        }
        if self.port == port && self.server.is_some() {
            return Ok(None);
        }
        self.bind(app, port).map(Some)
    }

    pub fn use_port(&mut self, port: u16, server: Server) {
        self.port = port;
        self.server = Some(server);
        self.error = None;
    }

    pub fn rotate_token(&mut self, token: String) {
        if let Some(server) = &self.server {
            server.rotate_token(token.clone());
        }
        self.token = token;
    }
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn parse_port(value: Option<String>) -> Result<u16, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_PORT);
    };
    value
        .parse()
        .ok()
        .filter(|port| *port >= 1024)
        .ok_or("Bridge port in SQLite is invalid. Enter a valid port in Settings.".into())
}

#[tauri::command]
pub fn bridge_settings(window: WebviewWindow) -> Result<Settings, String> {
    if window.label() != "main" {
        return Err("Open Dashboard Settings to manage the browser bridge.".into());
    }
    let state = window.state::<SessionState>();
    let session = state.lock().unwrap_or_else(|error| error.into_inner());
    session.browser_bridge_settings()
}

#[tauri::command]
pub fn open_extension_folder(window: WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Open Dashboard Settings to find the browser extension.".into());
    }
    let path = window
        .path()
        .resolve("extensions/chromium", tauri::path::BaseDirectory::Resource)
        .map_err(|error| format!("Could not locate the bundled extension: {error}"))?;
    #[cfg(debug_assertions)]
    let path = if path.is_dir() {
        path
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../extensions/chromium")
    };
    if !path.join("manifest.json").is_file() {
        return Err("Anchor could not find its unpacked extension folder.".into());
    }
    crate::controller::library::open_folder(&path).map_err(|error| {
        eprintln!("Could not open the unpacked extension folder: {error}");
        format!("Could not open the unpacked extension folder: {error}")
    })
}

pub fn invalid(error: impl std::fmt::Display) -> ApiError {
    eprintln!("Rejected a localhost bridge request: {error}");
    ApiError::Invalid
}

pub fn unavailable() -> ApiError {
    ApiError::Internal
}
