use super::Controller;
use crate::bridge::{
    self,
    server::{ApiError, Reply, Request, Status, Violation},
    Bridge, Settings,
};
use crate::storage::history::{BlockSource, BlockedRecord};
use crate::timing::Phase;
use std::time::Instant;
use tauri::AppHandle;

impl Controller {
    pub fn configure_bridge(&mut self, app: &AppHandle) {
        let result = (|| {
            let storage = self.store()?;
            let port = storage
                .setting("bridge_port")
                .map_err(|error| error.to_string())?;
            let port = bridge::parse_port(port).unwrap_or_else(|error| {
                eprintln!("{error}");
                17321
            });
            let token = storage
                .setting("bridge_token")
                .map_err(|error| error.to_string())?
                .ok_or("The database is missing its browser pairing token.")?;
            Ok::<_, String>((port, token))
        })();
        match result {
            Ok((port, token)) => {
                self.bridge = Some(Bridge::initialize(app, port, token, self.bridge_status()))
            }
            Err(error) => eprintln!("Could not configure the localhost bridge: {error}"),
        }
    }

    pub fn browser_bridge_settings(&self) -> Result<Settings, String> {
        self.bridge.as_ref().map(Bridge::settings).ok_or(
            "Browser bridge settings are unavailable. Check Anchor storage and restart.".into(),
        )
    }

    pub(super) fn set_bridge_port(&mut self, app: &AppHandle, port: u16) -> Result<(), String> {
        let bridge = self
            .bridge
            .as_ref()
            .ok_or("Browser bridge settings are unavailable.")?;
        if let Some(server) = bridge.prepare_port(app, port)? {
            self.store()?
                .set_setting("bridge_port", &port.to_string())
                .map_err(|error| format!("Could not save the browser bridge port: {error}"))?;
            self.bridge
                .as_mut()
                .ok_or("Browser bridge closed.")?
                .use_port(port, server);
        }
        Ok(())
    }

    pub(super) fn rotate_bridge_token(&mut self) -> Result<(), String> {
        let token = bridge::new_token();
        self.store()?
            .set_setting("bridge_token", &token)
            .map_err(|error| format!("Could not save the browser pairing token: {error}"))?;
        self.bridge
            .as_mut()
            .ok_or("Browser bridge settings are unavailable.")?
            .rotate_token(token);
        Ok(())
    }

    pub(super) fn bridge_status(&self) -> Status {
        let phase = self.timing.phase;
        let running = phase.is_running();
        let seconds = if running {
            self.timing.seconds_left(Instant::now())
        } else {
            0
        };
        Status {
            state: if running { phase.name() } else { "Idle" }.into(),
            session_id: self.session_id.clone().filter(|_| running),
            goal: if running {
                self.goal.clone()
            } else {
                String::new()
            },
            remaining_seconds: seconds,
            hall_passes_remaining: if running {
                self.timing.passes_remaining
            } else {
                0
            },
            hall_pass_remaining_seconds: if phase == Phase::HallPass { seconds } else { 0 },
            allowed_domains: if running {
                self.allowed_domains.clone()
            } else {
                Vec::new()
            },
            allow_localhost_domains: !self.restrict_localhost,
        }
    }

    pub fn bridge_request(&mut self, app: &AppHandle, request: Request) -> Reply {
        self.advance(app, Instant::now());
        let result = match request {
            Request::HallPass => self.use_hall_pass(app).map_err(|_| ApiError::Conflict),
            Request::Violation(violation) => self.record_browser_block(violation),
        };
        self.publish(app);
        result.map(|()| self.bridge_status())
    }

    fn record_browser_block(&mut self, violation: Violation) -> Result<(), ApiError> {
        if self.timing.phase != Phase::ActiveFocus || self.allowed_domains.is_empty() {
            return Err(ApiError::Conflict);
        }
        let session_id = self.session_id.as_ref().ok_or_else(bridge::unavailable)?;
        if violation
            .session_id
            .as_ref()
            .is_some_and(|expected| expected != session_id)
        {
            return Err(ApiError::Conflict);
        }
        let domain = violation.domain.to_ascii_lowercase();
        let domain = domain.trim_end_matches('.');
        let domain = domain.strip_prefix("www.").unwrap_or(domain);
        if !crate::bridge::valid_hostname(domain)
            || !(violation.url.starts_with("http://") || violation.url.starts_with("https://"))
            || violation.url.len() > 8_192
        {
            return Err(bridge::invalid("invalid browser violation"));
        }
        let record = BlockedRecord {
            source: BlockSource::Browser,
            session_id: session_id.clone(),
            occurred_at: chrono::Local::now().to_rfc3339(),
            process_name: domain.to_owned(),
            window_title: violation.tab_title.chars().take(250).collect(),
        };
        let storage = self.storage.as_mut().ok_or_else(bridge::unavailable)?;
        if let Err(error) = storage.record_switches(&[record]) {
            self.report_error(format!("Could not record the browser distraction: {error}"));
            return Err(ApiError::Internal);
        }
        self.blocked_count += 1;
        Ok(())
    }
}
