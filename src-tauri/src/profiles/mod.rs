pub(crate) mod ipc;
mod launch;
mod store;

pub use launch::bind_profile;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    #[serde(flatten)]
    pub draft: ProfileDraft,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileDraft {
    pub name: String,
    pub color: String,
    pub default_goal: String,
    pub focus_minutes: u32,
    pub break_minutes: u32,
    pub hall_passes: u8,
    pub ambient_sound: String,
    pub ambient_volume: f64,
    pub allowed_apps: Vec<ProfileApp>,
    pub allowed_domains: Vec<String>,
    pub allow_localhost_domains: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileApp {
    pub process_name: String,
    pub executable_path: String,
    pub launch_if_closed: bool,
    pub title_hint: Option<String>,
}

impl ProfileDraft {
    pub fn validate(mut self) -> Result<Self, String> {
        self.name = self.name.trim().to_owned();
        self.default_goal = self.default_goal.trim().to_owned();
        if self.name.is_empty() || self.name.chars().count() > 80 {
            return Err("Enter a profile name between 1 and 80 characters.".into());
        }
        if self.default_goal.chars().count() > 240 {
            return Err("Keep the default goal to 240 characters or fewer.".into());
        }
        if self.allowed_apps.is_empty() || self.allowed_apps.len() > 100 {
            return Err("Add between 1 and 100 apps to this profile.".into());
        }
        if self.color.len() != 7
            || !self.color.starts_with('#')
            || !self.color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err("Profile color must be a hex color such as #123456.".into());
        }
        if !(1..=180).contains(&self.focus_minutes)
            || !(1..=60).contains(&self.break_minutes)
            || self.hall_passes > 20
        {
            return Err("Invalid focus, break, or hall pass settings.".into());
        }
        if !matches!(self.ambient_sound.as_str(), "none" | "brown" | "pink")
            || !self.ambient_volume.is_finite()
            || !(0.0..=1.0).contains(&self.ambient_volume)
        {
            return Err("Invalid ambient sound or volume.".into());
        }
        self.allowed_domains = crate::bridge::validate_domains(self.allowed_domains)?;
        for app in &mut self.allowed_apps {
            app.process_name = app.process_name.trim().to_owned();
            app.executable_path = app.executable_path.trim().to_owned();
            app.title_hint = app.title_hint.take().and_then(|hint| {
                let hint = hint.trim().to_owned();
                (!hint.is_empty()).then_some(hint)
            });
            if app.process_name.is_empty() || app.process_name.contains(['/', '\\']) {
                return Err("Each app needs a process name without a path.".into());
            }
            if app.launch_if_closed
                && (!std::path::Path::new(&app.executable_path).is_absolute()
                    || !app.executable_path.to_ascii_lowercase().ends_with(".exe"))
            {
                return Err(format!(
                    "{} needs an absolute .exe path to launch.",
                    app.process_name
                ));
            }
        }
        Ok(self)
    }
}
