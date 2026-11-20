use super::{Profile, ProfileApp, ProfileDraft};
use crate::storage::{Storage, StoreResult};
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use std::io;
use uuid::Uuid;

fn read_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    fn json<T: serde::de::DeserializeOwned>(
        row: &rusqlite::Row<'_>,
        index: usize,
    ) -> rusqlite::Result<T> {
        let value: String = row.get(index)?;
        serde_json::from_str(&value).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })
    }
    let apps: Vec<ProfileApp> = json(row, 9)?;
    Ok(Profile {
        id: row.get(0)?,
        draft: ProfileDraft {
            name: row.get(1)?,
            color: row.get(2)?,
            default_goal: row.get(3)?,
            focus_minutes: row.get(4)?,
            break_minutes: row.get(5)?,
            hall_passes: row.get(6)?,
            ambient_sound: row.get(7)?,
            ambient_volume: row.get(8)?,
            allowed_apps: apps,
            allowed_domains: json(row, 10)?,
            allow_localhost_domains: row.get(11)?,
        },
        created_at: row.get(12)?,
        last_used_at: row.get(13)?,
    })
}
const COLUMNS: &str = "id, name, color, default_goal, focus_minutes, break_minutes, hall_passes, ambient_sound, ambient_volume, allowed_apps_json, allowed_domains_json, allow_localhost_domains, created_at, last_used_at";
impl Storage {
    pub fn list_profiles(&self) -> StoreResult<Vec<Profile>> {
        let mut stmt = self.connection().prepare(&format!(
            "SELECT {COLUMNS} FROM profiles ORDER BY name COLLATE NOCASE, id"
        ))?;
        let profiles = stmt
            .query_map([], read_profile)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(profiles)
    }
    pub fn get_profile(&self, id: &str) -> StoreResult<Option<Profile>> {
        Ok(self
            .connection()
            .query_row(
                &format!("SELECT {COLUMNS} FROM profiles WHERE id = ?1"),
                [id],
                read_profile,
            )
            .optional()?)
    }
    pub fn save_profile(&self, id: Option<&str>, draft: ProfileDraft) -> StoreResult<Profile> {
        let apps = serde_json::to_string(&draft.allowed_apps)?;
        let domains = serde_json::to_string(&draft.allowed_domains)?;
        let supplied = id.is_some();
        let id = id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Local::now().to_rfc3339();
        let values = params![
            id,
            draft.name,
            draft.color,
            draft.default_goal,
            draft.focus_minutes,
            draft.break_minutes,
            draft.hall_passes,
            draft.ambient_sound,
            draft.ambient_volume,
            apps,
            domains,
            draft.allow_localhost_domains
        ];
        let changed = if supplied {
            self.connection().execute("UPDATE profiles SET name=?2, color=?3, default_goal=?4, focus_minutes=?5, break_minutes=?6, hall_passes=?7, ambient_sound=?8, ambient_volume=?9, allowed_apps_json=?10, allowed_domains_json=?11, allow_localhost_domains=?12 WHERE id=?1", values)?
        } else {
            self.connection().execute("INSERT INTO profiles (id, name, color, default_goal, focus_minutes, break_minutes, hall_passes, ambient_sound, ambient_volume, allowed_apps_json, allowed_domains_json, allow_localhost_domains, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)", params![id, draft.name, draft.color, draft.default_goal, draft.focus_minutes, draft.break_minutes, draft.hall_passes, draft.ambient_sound, draft.ambient_volume, apps, domains, draft.allow_localhost_domains, now])?
        };
        if changed == 0 {
            return Err(io::Error::other("Profile not found").into());
        }
        self.get_profile(&id)?
            .ok_or_else(|| io::Error::other("Could not find saved profile").into())
    }
    pub fn delete_profile(&self, id: &str) -> StoreResult<()> {
        self.connection()
            .execute("DELETE FROM profiles WHERE id = ?1", [id])?;
        Ok(())
    }
    pub fn mark_profile_used(&self, id: &str) -> StoreResult<()> {
        let changed = self.connection().execute(
            "UPDATE profiles SET last_used_at = ?2 WHERE id = ?1",
            params![id, Local::now().to_rfc3339()],
        )?;
        if changed == 0 {
            return Err(io::Error::other("The profile was deleted during launch.").into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn study_profile_round_trips_without_resurrecting_deleted_rows() {
        let root = std::env::temp_dir().join(format!("anchor-profile-test-{}", Uuid::new_v4()));
        let storage = Storage::open(&root).unwrap();
        let draft = ProfileDraft {
            name: "Research".into(),
            color: "#ffbb77".into(),
            default_goal: "Read chapter 7".into(),
            focus_minutes: 50,
            break_minutes: 10,
            hall_passes: 3,
            ambient_sound: "pink".into(),
            ambient_volume: 0.42,
            allowed_apps: vec![ProfileApp {
                process_name: "editor.exe".into(),
                executable_path: r"C:\Study\editor.exe".into(),
                launch_if_closed: false,
                title_hint: Some("Chapter 7".into()),
            }],
            allowed_domains: vec!["docs.rs".into(), "*.mit.edu".into()],
            allow_localhost_domains: false,
        }
        .validate()
        .unwrap();
        let profile = storage.save_profile(None, draft.clone()).unwrap();
        assert_eq!(profile.draft.allowed_domains, ["docs.rs", "*.mit.edu"]);
        assert_eq!(
            profile.draft.allowed_apps[0].title_hint.as_deref(),
            Some("Chapter 7")
        );
        assert!(!profile.draft.allow_localhost_domains);
        assert_eq!(storage.list_profiles().unwrap().len(), 1);
        storage.mark_profile_used(&profile.id).unwrap();
        let mut changed = draft.clone();
        changed.name = "Papers".into();
        let updated = storage.save_profile(Some(&profile.id), changed).unwrap();
        assert_eq!(updated.draft.name, "Papers");
        assert_eq!(updated.created_at, profile.created_at);
        assert!(updated.last_used_at.is_some());
        storage.delete_profile(&profile.id).unwrap();
        assert!(storage.save_profile(Some(&profile.id), draft).is_err());
        assert!(storage.list_profiles().unwrap().is_empty());
        drop(storage);
        std::fs::remove_dir_all(root).unwrap();
    }
}
