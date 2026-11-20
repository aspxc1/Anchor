use super::StoreResult;
use rand::random;
use rusqlite::Connection;
use std::{fmt::Write as _, io};

const VERSION: i64 = 2;

pub(super) fn migrate(connection: &mut Connection) -> StoreResult<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > VERSION {
        return Err(io::Error::other(format!(
            "Database version {version} is newer than supported version {VERSION}"
        ))
        .into());
    }
    if version == VERSION {
        return Ok(());
    }
    let tx = connection.transaction()?;
    if version == 0 {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
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
    }
    tx.execute_batch("CREATE TABLE profiles (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        color TEXT NOT NULL,
        default_goal TEXT NOT NULL DEFAULT '',
        focus_minutes INTEGER NOT NULL,
        break_minutes INTEGER NOT NULL,
        hall_passes INTEGER NOT NULL,
        ambient_sound TEXT NOT NULL DEFAULT 'none' CHECK (ambient_sound IN ('none', 'brown', 'pink')),
        ambient_volume REAL NOT NULL DEFAULT 0.5,
        allowed_apps_json TEXT NOT NULL,
        allowed_domains_json TEXT NOT NULL,
        allow_localhost_domains INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        last_used_at TEXT
    );
    ALTER TABLE blocked_switches ADD COLUMN source TEXT NOT NULL DEFAULT 'desktop';")?;
    let mut token = String::with_capacity(32);
    for byte in random::<[u8; 16]>() {
        write!(&mut token, "{byte:02x}")?;
    }
    for (key, value) in [
        ("bridge_port", "17321"),
        ("bridge_token", token.as_str()),
        ("autostart_enabled", "false"),
    ] {
        tx.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            [key, value],
        )?;
    }
    tx.pragma_update(None, "user_version", VERSION)?;
    tx.commit()?;
    Ok(())
}
