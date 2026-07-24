use rusqlite::Connection;
use std::path::PathBuf;

pub fn init_db(app_data_dir: PathBuf) -> rusqlite::Result<Connection> {
    std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
    let db_path = app_data_dir.join("jarvis.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reminders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            due_at TEXT NOT NULL,
            fired INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    Ok(conn)
}
