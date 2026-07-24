use chrono::{Local, TimeZone, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
#[cfg(feature = "desktop")]
use tauri::State;

use crate::commands::ollama::{self, ChatMessage};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: i64,
    pub text: String,
    pub due_at: String,
    pub fired: bool,
}

/// LLM output and user input may express due_at in varied formats/timezones.
/// Normalize everything to canonical UTC RFC3339 so the scheduler can compare
/// due_at strings lexicographically without a timezone-mismatch bug.
fn normalize_due_at(due_at: &str) -> Result<String, String> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(due_at) {
        return Ok(dt.with_timezone(&Utc).to_rfc3339());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(due_at, fmt) {
            let local = Local
                .from_local_datetime(&naive)
                .single()
                .ok_or_else(|| format!("Ambiguous local time: {due_at}"))?;
            return Ok(local.with_timezone(&Utc).to_rfc3339());
        }
    }
    Err(format!("Unrecognized due_at format: {due_at}"))
}

// `*_impl` functions take a plain `&AppState` so they're reusable from both
// the Tauri app (whose commands wrap them below) and the standalone Axum
// server (`bin/server.rs`), which has no `tauri::State`.

pub fn create_reminder_impl(
    state: &AppState,
    text: String,
    due_at: String,
) -> Result<Reminder, String> {
    let normalized_due_at = normalize_due_at(&due_at)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO reminders (text, due_at, fired) VALUES (?1, ?2, 0)",
        params![text, normalized_due_at],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(Reminder {
        id,
        text,
        due_at: normalized_due_at,
        fired: false,
    })
}

pub fn list_reminders_impl(state: &AppState) -> Result<Vec<Reminder>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, text, due_at, fired FROM reminders ORDER BY due_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                text: row.get(1)?,
                due_at: row.get(2)?,
                fired: row.get::<_, i64>(3)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn delete_reminder_impl(state: &AppState, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM reminders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ParsedReminder {
    text: String,
    due_at: String,
}

fn extract_json_object(raw: &str) -> Result<&str, String> {
    let start = raw
        .find('{')
        .ok_or_else(|| format!("No JSON object found in: {raw}"))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| format!("No JSON object found in: {raw}"))?;
    if end < start {
        return Err(format!("No JSON object found in: {raw}"));
    }
    Ok(&raw[start..=end])
}

pub async fn parse_and_create_reminder_impl(
    state: &AppState,
    model: String,
    utterance: String,
) -> Result<Reminder, String> {
    let now_utc = Utc::now().to_rfc3339();
    let system_prompt = format!(
        "You extract reminders from natural language. The current UTC date/time is {now_utc}. \
         Compute the reminder's due date/time relative to that UTC reference and respond with \
         ONLY a JSON object of the form \
         {{\"text\": \"<reminder text>\", \"due_at\": \"<UTC ISO8601 datetime, e.g. 2026-07-04T20:52:00Z>\"}}. \
         Always express due_at in UTC. Respond with no other text."
    );
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: utterance,
        },
    ];
    let raw = ollama::chat_json(model, messages).await?;
    // Ollama's JSON mode should make `raw` itself the JSON object, but fall
    // back to scanning for embedded braces in case a backend (e.g. a
    // cloud-forwarded model) doesn't fully honor that mode.
    let parsed: ParsedReminder = serde_json::from_str(raw.trim())
        .or_else(|_| {
            let json_str = extract_json_object(&raw)?;
            serde_json::from_str(json_str).map_err(|e| e.to_string())
        })
        .map_err(|e| format!("Failed to parse reminder from model output '{raw}': {e}"))?;
    create_reminder_impl(state, parsed.text, parsed.due_at)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn create_reminder(
    state: State<AppState>,
    text: String,
    due_at: String,
) -> Result<Reminder, String> {
    create_reminder_impl(&state, text, due_at)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_reminders(state: State<AppState>) -> Result<Vec<Reminder>, String> {
    list_reminders_impl(&state)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn delete_reminder(state: State<AppState>, id: i64) -> Result<(), String> {
    delete_reminder_impl(&state, id)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn parse_and_create_reminder(
    state: State<'_, AppState>,
    model: String,
    utterance: String,
) -> Result<Reminder, String> {
    parse_and_create_reminder_impl(&state, model, utterance).await
}
