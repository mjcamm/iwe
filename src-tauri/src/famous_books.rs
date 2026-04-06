// Library of analysed famous books — separate SQLite DB in app data dir,
// mirrors the palettes.rs pattern. Stores raw JSON results from each analysis
// command so they can be compared against the user's manuscripts.

use rusqlite::{params, Connection};
use serde::Serialize;
use std::sync::Mutex;

pub struct LibraryState {
    pub db: Mutex<Connection>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBook {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub source: String,
    pub word_count: i64,
    pub imported_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBookDetail {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub source: String,
    pub word_count: i64,
    pub imported_at: String,
    /// JSON map of analysisKey → raw analysis result JSON
    pub analyses_json: String,
}

pub fn init_library_db(path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS famous_books (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            title        TEXT NOT NULL,
            author       TEXT DEFAULT '',
            source       TEXT DEFAULT '',
            word_count   INTEGER DEFAULT 0,
            analyses_json TEXT NOT NULL DEFAULT '{}',
            imported_at  DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )?;
    Ok(conn)
}

#[tauri::command]
pub fn list_library_books(
    state: tauri::State<'_, LibraryState>,
) -> Result<Vec<LibraryBook>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, title, author, source, word_count, imported_at FROM famous_books ORDER BY title")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(LibraryBook {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                source: row.get(3)?,
                word_count: row.get(4)?,
                imported_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn get_library_book(
    state: tauri::State<'_, LibraryState>,
    id: i64,
) -> Result<Option<LibraryBookDetail>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, title, author, source, word_count, imported_at, analyses_json FROM famous_books WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(params![id], |row| {
            Ok(LibraryBookDetail {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                source: row.get(3)?,
                word_count: row.get(4)?,
                imported_at: row.get(5)?,
                analyses_json: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    if let Some(r) = rows.next() {
        Ok(Some(r.map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn save_library_book(
    state: tauri::State<'_, LibraryState>,
    title: String,
    author: String,
    source: String,
    word_count: i64,
    analyses_json: String,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    // Upsert by source filename so re-saving (even after renaming the title) overwrites
    conn.execute(
        "DELETE FROM famous_books WHERE source = ?1",
        params![&source],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO famous_books (title, author, source, word_count, analyses_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![&title, &author, &source, word_count, &analyses_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_library_book(
    state: tauri::State<'_, LibraryState>,
    id: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM famous_books WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
