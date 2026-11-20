use super::{Storage, StoreResult};
use chrono::{Local, NaiveDate};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io, path::Path};

#[derive(Serialize)]
pub struct SessionSummary {
    id: String,
    goal: String,
    started_at: String,
    ended_at: String,
    planned_focus_minutes: u32,
    actual_focus_seconds: i64,
    status: String,
    hall_passes_used: u8,
    blocked_count: u32,
    note_count: u32,
}

#[derive(Serialize)]
pub struct History {
    sessions: Vec<SessionSummary>,
    streak: u32,
    total_focus_seconds: i64,
}

#[derive(Serialize)]
pub struct BlockedApp {
    process_name: String,
    count: u32,
}

#[derive(Serialize)]
pub struct Note {
    id: i64,
    session_id: Option<String>,
    created_at: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct AllowedApp {
    title: String,
    process_name: String,
}

#[derive(Serialize)]
pub struct SessionDetail {
    summary: SessionSummary,
    blocked: Vec<BlockedApp>,
    notes: Vec<Note>,
    allowed: Vec<AllowedApp>,
}

pub struct BlockedRecord {
    pub session_id: String,
    pub occurred_at: String,
    pub process_name: String,
    pub window_title: String,
}

const SUMMARY: &str = "
    SELECT s.id, s.goal, s.started_at, s.ended_at, s.planned_focus_minutes,
           s.actual_focus_seconds, s.status, s.hall_passes_used,
           (SELECT count(*) FROM blocked_switches WHERE session_id = s.id),
           (SELECT count(*) FROM brain_dumps WHERE session_id = s.id)
    FROM sessions s WHERE s.ended_at IS NOT NULL";

impl Storage {
    pub fn record_switches(&mut self, switches: &[BlockedRecord]) -> StoreResult<()> {
        if switches.is_empty() {
            return Ok(());
        }
        let tx = self.connection.transaction()?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO blocked_switches (session_id, occurred_at, process_name, window_title)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for switch in switches {
                insert.execute(params![
                    switch.session_id,
                    switch.occurred_at,
                    switch.process_name,
                    switch.window_title
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn history(&self) -> StoreResult<History> {
        let sql = format!("{SUMMARY} ORDER BY julianday(s.ended_at) DESC, s.id");
        let mut query = self.connection.prepare(&sql)?;
        let sessions = query
            .query_map([], summary)?
            .collect::<Result<Vec<_>, _>>()?;
        let days = sessions
            .iter()
            .filter(|session| session.status == "completed")
            .map(|session| {
                chrono::DateTime::parse_from_rfc3339(&session.ended_at)
                    .map(|ended_at| ended_at.date_naive())
            })
            .collect::<Result<HashSet<_>, _>>()?;
        let streak = current_streak(&days, Local::now().date_naive());
        let total_focus_seconds = sessions
            .iter()
            .map(|session| session.actual_focus_seconds)
            .sum();
        Ok(History {
            sessions,
            streak,
            total_focus_seconds,
        })
    }

    pub fn session_detail(&self, id: &str) -> StoreResult<SessionDetail> {
        let sql = format!("{SUMMARY} AND s.id = ?1");
        let summary = self
            .connection
            .query_row(&sql, [id], summary)
            .optional()?
            .ok_or_else(|| io::Error::other("This session is unavailable in history."))?;
        let mut query = self.connection.prepare(
            "SELECT min(process_name), count(*) FROM blocked_switches
             WHERE session_id = ?1 GROUP BY process_name COLLATE NOCASE
             ORDER BY count(*) DESC, min(process_name) COLLATE NOCASE",
        )?;
        let blocked = query
            .query_map([id], |row| {
                Ok(BlockedApp {
                    process_name: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let notes = self.notes(Some(id))?;
        let json: String = self.connection.query_row(
            "SELECT allowed_apps_json FROM sessions WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(SessionDetail {
            summary,
            blocked,
            notes,
            allowed: serde_json::from_str(&json)?,
        })
    }

    pub fn notes(&self, session: Option<&str>) -> StoreResult<Vec<Note>> {
        let mut query = self.connection.prepare(
            "SELECT id, session_id, created_at, content FROM brain_dumps
             WHERE (?1 IS NULL OR session_id = ?1) ORDER BY id DESC",
        )?;
        let notes = query
            .query_map([session], |row| {
                Ok(Note {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    created_at: row.get(2)?,
                    content: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(notes)
    }

    pub fn delete_note(&mut self, id: i64) -> StoreResult<()> {
        let tx = self.connection.transaction()?;
        let day: Option<String> = tx
            .query_row(
                "SELECT substr(created_at, 1, 10) FROM brain_dumps WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(day) = day {
            tx.execute("DELETE FROM brain_dumps WHERE id = ?1", [id])?;
            tx.execute(
                "INSERT OR IGNORE INTO pending_markdown_dates (day) VALUES (?1)",
                [day],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn markdown_folder(&self) -> &Path {
        &self.markdown
    }
}

fn summary(row: &Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        id: row.get(0)?,
        goal: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
        planned_focus_minutes: row.get(4)?,
        actual_focus_seconds: row.get(5)?,
        status: row.get(6)?,
        hall_passes_used: row.get(7)?,
        blocked_count: row.get(8)?,
        note_count: row.get(9)?,
    })
}

fn current_streak(days: &HashSet<NaiveDate>, today: NaiveDate) -> u32 {
    let mut date = if days.contains(&today) {
        Some(today)
    } else {
        today.pred_opt()
    };
    let mut streak = 0;
    while let Some(day) = date.filter(|day| days.contains(day)) {
        streak += 1;
        date = day.pred_opt();
    }
    streak
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn grouped_switches_and_notes_are_tied_to_finished_sessions() {
        let root = std::env::temp_dir().join(format!("anchor-history-test-{}", Uuid::new_v4()));
        let mut storage = Storage::open(&root).unwrap();
        let id = storage.start_session("Read", 25, "[]").unwrap();
        let now = Local::now();
        let time = now.to_rfc3339();
        let switches = ["edge.exe", "Edge.exe", "notepad.exe"].map(|name| BlockedRecord {
            session_id: id.clone(),
            occurred_at: time.clone(),
            process_name: name.into(),
            window_title: "Test".into(),
        });
        storage.record_switches(&switches).unwrap();
        storage.save_note(Some(&id), "Page 2", now).unwrap();
        storage.sync_markdown().unwrap();
        assert!(storage.history().unwrap().sessions.is_empty());
        storage
            .finish_session(&id, "completed", 1500, 1, "[]")
            .unwrap();
        let history = storage.history().unwrap();
        assert_eq!(history.streak, 1);
        assert_eq!(history.total_focus_seconds, 1500);
        let detail = storage.session_detail(&id).unwrap();
        assert_eq!(detail.summary.blocked_count, 3);
        assert_eq!(detail.blocked[0].count, 2);
        assert_eq!(detail.notes.len(), 1);
        storage.delete_note(detail.notes[0].id).unwrap();
        storage.sync_markdown().unwrap();
        assert!(storage.session_detail(&id).unwrap().notes.is_empty());
        let file = storage
            .markdown_folder()
            .join(format!("{}.md", now.date_naive()));
        assert_eq!(fs::read_to_string(file).unwrap(), "");
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }
}
