use chrono::{DateTime, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

type StoreResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Noise {
    Brown,
    Pink,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Audio {
    pub enabled: bool,
    pub noise: Noise,
    pub volume: u8,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub hotkey: String,
    pub focus_minutes: u32,
    pub break_minutes: u32,
    pub hall_passes: u8,
    pub audio: Audio,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+Space".into(),
            focus_minutes: 25,
            break_minutes: 5,
            hall_passes: 3,
            audio: Audio {
                enabled: false,
                noise: Noise::Brown,
                volume: 22,
            },
        }
    }
}

pub struct Storage {
    connection: Connection,
    _lock: fs::File,
    markdown: PathBuf,
}

impl Storage {
    pub fn open(root: &Path) -> StoreResult<Self> {
        fs::create_dir_all(root)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("anchor.lock"))?;
        lock.try_lock().map_err(|error| {
            io::Error::other(format!(
                "Anchor storage is already in use or cannot be locked: {error}"
            ))
        })?;
        let markdown = root.join("braindump");
        fs::create_dir_all(&markdown)?;
        let connection = Connection::open(root.join("anchor.db"))?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                planned_focus_minutes INTEGER NOT NULL,
                actual_focus_seconds INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL CHECK (status IN ('completed', 'aborted')),
                hall_passes_used INTEGER NOT NULL DEFAULT 0,
                allowed_apps_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS blocked_switches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                occurred_at TEXT NOT NULL,
                process_name TEXT NOT NULL,
                window_title TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS brain_dumps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT REFERENCES sessions(id),
                created_at TEXT NOT NULL,
                content TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS pending_markdown_dates (day TEXT PRIMARY KEY);
             CREATE INDEX IF NOT EXISTS brain_dumps_created_at ON brain_dumps(created_at);
             CREATE INDEX IF NOT EXISTS brain_dumps_session ON brain_dumps(session_id);
             CREATE INDEX IF NOT EXISTS blocked_switches_session ON blocked_switches(session_id);",
        )?;
        Ok(Self {
            connection,
            markdown,
            _lock: lock,
        })
    }

    pub fn recover_sessions(&self) -> StoreResult<()> {
        self.connection.execute(
            "UPDATE sessions SET ended_at = started_at WHERE ended_at IS NULL",
            [],
        )?;
        Ok(())
    }

    pub fn preferences(&self) -> StoreResult<Preferences> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'preferences'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match value {
            Some(value) => Ok(serde_json::from_str(&value)?),
            None => Ok(Preferences::default()),
        }
    }

    pub fn save_preferences(&self, preferences: &Preferences) -> StoreResult<()> {
        self.connection.execute(
            "INSERT INTO settings (key, value) VALUES ('preferences', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::to_string(preferences)?],
        )?;
        Ok(())
    }

    pub fn start_session(&self, goal: &str, focus_minutes: u32, apps: &str) -> StoreResult<String> {
        let id = Uuid::new_v4().to_string();
        self.connection.execute(
            "INSERT INTO sessions (id, goal, started_at, planned_focus_minutes, status, allowed_apps_json)
             VALUES (?1, ?2, ?3, ?4, 'aborted', ?5)",
            params![id, goal, Local::now().to_rfc3339(), focus_minutes, apps],
        )?;
        Ok(id)
    }

    pub fn finish_session(
        &self,
        id: &str,
        status: &str,
        focus_seconds: u64,
        passes: u8,
        apps: &str,
    ) -> StoreResult<()> {
        let updated = self.connection.execute(
            "UPDATE sessions SET ended_at = ?2, status = ?3, actual_focus_seconds = ?4,
             hall_passes_used = ?5, allowed_apps_json = ?6 WHERE id = ?1",
            params![
                id,
                Local::now().to_rfc3339(),
                status,
                i64::try_from(focus_seconds)?,
                passes,
                apps
            ],
        )?;
        if updated != 1 {
            return Err(
                io::Error::other("The active session is missing from the database.").into(),
            );
        }
        Ok(())
    }

    pub fn save_note(
        &mut self,
        session_id: Option<&str>,
        content: &str,
        now: DateTime<Local>,
    ) -> StoreResult<()> {
        let day = now.date_naive().to_string();
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT INTO brain_dumps (session_id, created_at, content) VALUES (?1, ?2, ?3)",
            params![session_id, now.to_rfc3339(), content],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO pending_markdown_dates (day) VALUES (?1)",
            [day],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn sync_markdown(&self) -> StoreResult<()> {
        let mut statement = self
            .connection
            .prepare("SELECT day FROM pending_markdown_dates ORDER BY day")?;
        let days = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for day in days {
            self.write_day(NaiveDate::parse_from_str(&day, "%Y-%m-%d")?)?;
            self.connection
                .execute("DELETE FROM pending_markdown_dates WHERE day = ?1", [day])?;
        }
        Ok(())
    }

    fn write_day(&self, date: NaiveDate) -> StoreResult<()> {
        let date = date.to_string();
        let destination = self.markdown.join(format!("{date}.md"));
        let temporary = self
            .markdown
            .join(format!(".{date}-{}.tmp", Uuid::new_v4()));
        let mut statement = self.connection.prepare(
            "SELECT created_at, content FROM brain_dumps WHERE created_at LIKE ?1 ORDER BY id",
        )?;
        let notes = statement.query_map([format!("{date}T%")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let result = (|| -> StoreResult<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            for note in notes {
                let (time, content) = note?;
                let time = DateTime::parse_from_rfc3339(&time)?;
                writeln!(file, "- [{}] {}", time.format("%H:%M"), content)?;
            }
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &destination)?;
            Ok(())
        })();
        if result.is_err() {
            if let Err(error) = fs::remove_file(&temporary) {
                if error.kind() != io::ErrorKind::NotFound {
                    eprintln!("Could not remove incomplete Markdown export: {error}");
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn capture_has_a_session_and_retries_a_failed_daily_export_without_duplication() {
        let root = std::env::temp_dir().join(format!("anchor-test-{}", Uuid::new_v4()));
        let mut storage = Storage::open(&root).unwrap();
        let id = storage.start_session("Read", 25, "[]").unwrap();
        let time = Local
            .with_ymd_and_hms(2026, 1, 10, 10, 30, 0)
            .single()
            .unwrap();
        storage
            .save_note(Some(&id), "Check chapter 3", time)
            .unwrap();
        let destination = root.join("braindump/2026-01-10.md");
        fs::create_dir(&destination).unwrap();
        assert!(storage.sync_markdown().is_err());
        fs::remove_dir(&destination).unwrap();
        storage.sync_markdown().unwrap();
        storage.save_note(None, "Another note", time).unwrap();
        storage.sync_markdown().unwrap();
        storage.sync_markdown().unwrap();
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            "- [10:30] Check chapter 3\n- [10:30] Another note\n"
        );
        storage
            .finish_session(&id, "completed", 1500, 1, "[]")
            .unwrap();
        assert_eq!(
            storage
                .connection
                .query_row(
                    "SELECT session_id FROM brain_dumps WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            id
        );
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }
}
