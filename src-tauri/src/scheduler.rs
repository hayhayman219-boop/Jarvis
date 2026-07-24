use std::time::Duration;

use chrono::Utc;
use rusqlite::params;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(30);

pub fn start(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            check_due_reminders(&app_handle);
        }
    });
}

fn check_due_reminders(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let now = Utc::now().to_rfc3339();

    let due: Vec<(i64, String)> = {
        let conn = state.db.lock().expect("reminder db lock poisoned");
        let mut stmt = conn
            .prepare("SELECT id, text FROM reminders WHERE fired = 0 AND due_at <= ?1")
            .expect("failed to prepare due-reminders query");
        let rows = stmt
            .query_map(params![now], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .expect("failed to run due-reminders query");
        rows.filter_map(|r| r.ok()).collect()
    };

    for (id, text) in due {
        let _ = app_handle
            .notification()
            .builder()
            .title("Jarvis Reminder")
            .body(&text)
            .show();

        let conn = state.db.lock().expect("reminder db lock poisoned");
        let _ = conn.execute("UPDATE reminders SET fired = 1 WHERE id = ?1", params![id]);
    }
}
