mod analysis;
mod db;
mod epub;
mod epub_validate;
mod famous_books;
mod format;
mod import;
mod semantic;
mod text_utils;
mod palettes;
mod scanner;
mod spellcheck;
mod syllable_data;
mod synonyms;
mod wordlists;
mod ydoc;

use db::AppState;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

// ---- Backup helpers ----

fn chrono_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    // Format as ISO-ish: YYYY-MM-DD-HH-MM-SS (using simple arithmetic, no chrono crate)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Approximate date from epoch days (good enough for comparison)
    // Use a proper calculation
    let (year, month, day) = epoch_days_to_date(days as i64);
    format!("{:04}-{:02}-{:02}-{:02}-{:02}-{:02}", year, month, day, hours, minutes, seconds)
}

fn epoch_days_to_date(days: i64) -> (i64, i64, i64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as i64, d as i64)
}

fn time_diff_minutes(a: &str, b: &str) -> i64 {
    // Parse YYYY-MM-DD-HH-MM-SS timestamps and compute rough difference in minutes
    fn parse_ts(s: &str) -> Option<i64> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() < 6 { return None; }
        let y: i64 = parts[0].parse().ok()?;
        let mo: i64 = parts[1].parse().ok()?;
        let d: i64 = parts[2].parse().ok()?;
        let h: i64 = parts[3].parse().ok()?;
        let mi: i64 = parts[4].parse().ok()?;
        // Rough: days since epoch * 1440 + hours*60 + minutes
        let days = y * 365 + mo * 30 + d; // approximate, good enough for interval comparison
        Some(days * 1440 + h * 60 + mi)
    }
    match (parse_ts(a), parse_ts(b)) {
        (Some(a), Some(b)) => (b - a).abs(),
        _ => i64::MAX, // if parse fails, trigger backup
    }
}

fn run_backup(db_path: &str) -> Result<(), String> {
    use std::path::Path;

    let path = Path::new(db_path);
    let parent = path.parent().ok_or("No parent directory")?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("project");

    // Create backup dir: {parent}/backups/{bookname}/
    let backup_dir = parent.join("backups").join(stem);
    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("mkdir failed: {e}"))?;

    // Timestamp for filename
    let timestamp = chrono_now();
    let backup_filename = format!("{}-{}.iwe", stem, timestamp);
    let backup_path = backup_dir.join(&backup_filename);

    // Copy the file
    std::fs::copy(db_path, &backup_path).map_err(|e| format!("copy failed: {e}"))?;
    log::info!("[backup] created: {}", backup_path.display());

    // Open the copy and flush semantic data
    {
        let conn = Connection::open(&backup_path).map_err(|e| format!("open backup: {e}"))?;
        conn.execute("DELETE FROM semantic_embeddings", []).ok();
        conn.execute("DELETE FROM semantic_index_status", []).ok();
        conn.execute("VACUUM", []).ok();
    }
    log::info!("[backup] flushed semantic data from backup");

    // Cleanup old backups
    cleanup_old_backups(&backup_dir);

    Ok(())
}

fn cleanup_old_backups(backup_dir: &std::path::Path) {
    let now = chrono_now();
    let now_parts: Vec<&str> = now.split('-').collect();
    if now_parts.len() < 3 { return; }

    let entries: Vec<_> = match std::fs::read_dir(backup_dir) {
        Ok(e) => e.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };

    // Collect backup files with their dates
    let mut files_by_date: std::collections::HashMap<String, Vec<(String, std::path::PathBuf)>> =
        std::collections::HashMap::new();

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".iwe") { continue; }

        // Extract timestamp: {stem}-YYYY-MM-DD-HH-MM-SS.iwe
        // Find the date part by looking for the pattern
        let parts: Vec<&str> = name.trim_end_matches(".iwe").split('-').collect();
        if parts.len() < 6 { continue; }
        let len = parts.len();
        let date_key = format!("{}-{}-{}", parts[len - 6], parts[len - 5], parts[len - 4]);
        let time_key = format!("{}-{}-{}", parts[len - 3], parts[len - 2], parts[len - 1]);
        let full_ts = format!("{}-{}", date_key, time_key);

        // Check if older than 7 days (rough: compare date strings)
        let days_old = rough_days_old(&date_key, &now);
        if days_old <= 7 { continue; } // Keep all backups within 7 days

        files_by_date
            .entry(date_key)
            .or_default()
            .push((full_ts, entry.path()));
    }

    // For each date with multiple backups, keep the earliest, delete the rest
    for (_date, mut files) in files_by_date {
        if files.len() <= 1 { continue; }
        files.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by timestamp
        // Keep first (earliest), delete rest
        for (_, path) in files.into_iter().skip(1) {
            if let Err(e) = std::fs::remove_file(&path) {
                log::warn!("[backup] cleanup failed for {}: {}", path.display(), e);
            } else {
                log::info!("[backup] cleaned up old backup: {}", path.display());
            }
        }
    }
}

fn rough_days_old(date_str: &str, now_str: &str) -> i64 {
    fn parse_date(s: &str) -> Option<i64> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() < 3 { return None; }
        let y: i64 = parts[0].parse().ok()?;
        let m: i64 = parts[1].parse().ok()?;
        let d: i64 = parts[2].parse().ok()?;
        Some(y * 365 + m * 30 + d)
    }
    match (parse_date(date_str), parse_date(&now_str[..10])) {
        (Some(a), Some(b)) => (b - a).abs(),
        _ => 0,
    }
}

// ---- Backup commands ----

#[tauri::command]
fn get_backup_interval(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let (interval, _) = db::get_backup_settings(conn).map_err(|e| e.to_string())?;
    Ok(interval)
}

#[tauri::command]
fn set_backup_interval(state: tauri::State<'_, AppState>, minutes: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_backup_interval(conn, minutes).map_err(|e| e.to_string())
}

// ---- Project commands ----

#[tauri::command]
fn open_project(state: tauri::State<'_, AppState>, filepath: String) -> Result<(), String> {
    let conn = Connection::open(&filepath).map_err(|e| e.to_string())?;
    db::init_schema(&conn).map_err(|e| e.to_string())?;
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    let mut path_guard = state.db_path.lock().map_err(|e| e.to_string())?;
    *path_guard = Some(filepath);
    Ok(())
}

#[tauri::command]
fn get_project_setting(state: tauri::State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let mut stmt = conn
        .prepare("SELECT value FROM project_settings WHERE key = ?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(rusqlite::params![key]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let v: String = row.get(0).map_err(|e| e.to_string())?;
        Ok(Some(v))
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn set_project_setting(state: tauri::State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    conn.execute(
        "INSERT INTO project_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn close_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = None;
    let mut path_guard = state.db_path.lock().map_err(|e| e.to_string())?;
    *path_guard = None;
    Ok(())
}

// ---- Book cover commands ----

#[derive(serde::Serialize)]
pub struct BookCoverData {
    pub data: Vec<u8>,
    pub mime_type: String,
}

/// Run the lightweight EPUB sanity checker on a byte buffer. Returns a
/// list of issues (empty = passes). Called by the frontend after every
/// EPUB export so the user gets an immediate warning if the file is
/// malformed, rather than finding out later in a reader.
#[tauri::command]
fn validate_epub_bytes(bytes: Vec<u8>) -> Vec<epub_validate::EpubIssue> {
    epub_validate::validate(&bytes)
}

#[tauri::command]
fn get_book_cover(state: tauri::State<'_, AppState>) -> Result<Option<BookCoverData>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    match db::get_book_cover(conn).map_err(|e| e.to_string())? {
        Some((data, mime_type)) => Ok(Some(BookCoverData { data, mime_type })),
        None => Ok(None),
    }
}

#[tauri::command]
fn set_book_cover(
    state: tauri::State<'_, AppState>,
    data: Vec<u8>,
    mime_type: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::set_book_cover(conn, &data, &mime_type).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_book_cover(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::clear_book_cover(conn).map_err(|e| e.to_string())
}

/// Peek the cover of an arbitrary project file by path, without going through
/// the open_project / AppState machinery. Used by the home page to show cover
/// thumbnails next to each project in the list.
///
/// Uses a short-lived read-only connection so it doesn't fight with the main
/// project connection for file locks. Tolerant of schemas that predate the
/// book_cover table (returns None on the error instead of propagating).
#[tauri::command]
fn get_project_cover_by_path(filepath: String) -> Result<Option<BookCoverData>, String> {
    use rusqlite::OpenFlags;
    let conn = match Connection::open_with_flags(
        &filepath,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    match db::get_book_cover(&conn) {
        Ok(Some((data, mime_type))) => Ok(Some(BookCoverData { data, mime_type })),
        Ok(None) => Ok(None),
        // Table doesn't exist yet on pre-migration projects — not an error.
        Err(_) => Ok(None),
    }
}

// ---- Chapter commands ----

#[tauri::command]
fn get_chapters(state: tauri::State<'_, AppState>) -> Result<Vec<db::Chapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_chapters(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_chapter(state: tauri::State<'_, AppState>, id: i64) -> Result<Option<db::Chapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_chapter(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_chapter(state: tauri::State<'_, AppState>, title: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_chapter(conn, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_chapter_content(state: tauri::State<'_, AppState>, id: i64, content: Vec<u8>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_chapter_content(conn, id, &content).map_err(|e| e.to_string())?;

    // Check if backup is due
    if let Ok((interval, last_backup)) = db::get_backup_settings(conn) {
        if interval > 0 {
            let now = chrono_now();
            let should_backup = if last_backup.is_empty() {
                true
            } else {
                let elapsed = time_diff_minutes(&last_backup, &now);
                elapsed >= interval
            };
            if should_backup {
                db::set_last_backup_at(conn, &now).ok();
                let db_path = state.db_path.lock().ok().and_then(|g| g.clone());
                if let Some(path) = db_path {
                    std::thread::spawn(move || {
                        if let Err(e) = run_backup(&path) {
                            log::warn!("[backup] failed: {}", e);
                        }
                    });
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
fn rename_chapter(state: tauri::State<'_, AppState>, id: i64, title: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::rename_chapter(conn, id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_chapter_metadata(
    state: tauri::State<'_, AppState>,
    id: i64, title: String, subtitle: String, chapter_image: String,
    ornament_above: String, ornament_mid: String, ornament_below: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_chapter_metadata(conn, id, &title, &subtitle, &chapter_image, &ornament_above, &ornament_mid, &ornament_below)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_chapter(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_chapter(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_chapters(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_chapters(conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_deleted_chapters(state: tauri::State<'_, AppState>) -> Result<Vec<db::Chapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_deleted_chapters(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn restore_chapter(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::restore_chapter(conn, id).map_err(|e| e.to_string())
}

// ---- Entity commands ----

#[tauri::command]
fn get_entities(state: tauri::State<'_, AppState>) -> Result<Vec<db::Entity>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_entities(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_entity(state: tauri::State<'_, AppState>, name: String, entity_type: String, description: String, color: Option<String>) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let id = db::create_entity(conn, &name, &entity_type, &description, &color.unwrap_or_default()).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(id)
}

#[tauri::command]
fn update_entity(state: tauri::State<'_, AppState>, id: i64, name: String, entity_type: String, description: String, color: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_entity(conn, id, &name, &entity_type, &description, &color).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

#[tauri::command]
fn delete_entity(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity(conn, id).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

#[tauri::command]
fn set_entity_visible(state: tauri::State<'_, AppState>, id: i64, visible: bool) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::set_entity_visible(conn, id, visible).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_alias(state: tauri::State<'_, AppState>, entity_id: i64, alias: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_alias(conn, entity_id, &alias).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

#[tauri::command]
fn remove_alias(state: tauri::State<'_, AppState>, entity_id: i64, alias: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_alias(conn, entity_id, &alias).map_err(|e| e.to_string())?;
    let _ = db::sync_entity_words(conn);
    Ok(())
}

// ---- Entity notes ----

#[tauri::command]
fn get_entity_notes(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<db::EntityNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_notes(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_entity_note(state: tauri::State<'_, AppState>, entity_id: i64, chapter_id: Option<i64>, y_start: Vec<u8>, y_end: Vec<u8>) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_entity_note(conn, entity_id, chapter_id, &y_start, &y_end).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entity_note(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity_note(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_entity_notes(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_entity_notes(conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_entity_free_notes(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<db::EntityFreeNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_free_notes(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_entity_free_note(state: tauri::State<'_, AppState>, entity_id: i64, title: String, text: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_entity_free_note(conn, entity_id, &title, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_entity_free_note(state: tauri::State<'_, AppState>, id: i64, title: String, text: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_entity_free_note(conn, id, &title, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entity_free_note(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity_free_note(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_entity_free_note(state: tauri::State<'_, AppState>, id: i64, new_entity_id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::move_entity_free_note(conn, id, new_entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_entity_free_notes(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_entity_free_notes(conn, &ids).map_err(|e| e.to_string())
}

// ---- Chapter planning notes (kanban) ----

#[tauri::command]
fn get_chapter_planning_notes(state: tauri::State<'_, AppState>, chapter_id: i64) -> Result<Vec<db::ChapterPlanningNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_chapter_planning_notes(conn, chapter_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_chapter_planning_notes(state: tauri::State<'_, AppState>) -> Result<Vec<db::ChapterPlanningNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_all_chapter_planning_notes(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_chapter_planning_note(state: tauri::State<'_, AppState>, chapter_id: i64, title: String, description: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_chapter_planning_note(conn, chapter_id, &title, &description).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_chapter_planning_note(state: tauri::State<'_, AppState>, id: i64, title: String, description: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_chapter_planning_note(conn, id, &title, &description).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_chapter_planning_note(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_chapter_planning_note(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_chapter_planning_notes(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_chapter_planning_notes(conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_chapter_planning_note(state: tauri::State<'_, AppState>, id: i64, new_chapter_id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::move_chapter_planning_note(conn, id, new_chapter_id).map_err(|e| e.to_string())
}

// ---- Kanban freeform board ----

#[tauri::command]
fn get_kanban_columns(state: tauri::State<'_, AppState>) -> Result<Vec<db::KanbanColumn>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_kanban_columns(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_kanban_column(state: tauri::State<'_, AppState>, title: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_kanban_column(conn, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_kanban_column(state: tauri::State<'_, AppState>, id: i64, title: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_kanban_column(conn, id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_kanban_column(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_kanban_column(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_kanban_columns(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_kanban_columns(conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_kanban_cards(state: tauri::State<'_, AppState>) -> Result<Vec<db::KanbanCard>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_all_kanban_cards(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_kanban_card(state: tauri::State<'_, AppState>, column_id: i64, title: String, description: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_kanban_card(conn, column_id, &title, &description).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_kanban_card(state: tauri::State<'_, AppState>, id: i64, title: String, description: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_kanban_card(conn, id, &title, &description).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_kanban_card(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_kanban_card(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_kanban_card(state: tauri::State<'_, AppState>, id: i64, new_column_id: i64, new_sort_order: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::move_kanban_card(conn, id, new_column_id, new_sort_order).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_kanban_cards(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_kanban_cards(conn, &ids).map_err(|e| e.to_string())
}

// ---- Writing stats ----

#[tauri::command]
fn log_writing_activity(state: tauri::State<'_, AppState>, chapter_id: Option<i64>, chapter_words: i64, manuscript_words: i64, words_delta: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::log_writing_activity(conn, chapter_id, chapter_words, manuscript_words, words_delta).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_daily_stats(state: tauri::State<'_, AppState>, days: i64) -> Result<Vec<db::DailyStats>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_daily_stats(conn, days).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_daily_stats(state: tauri::State<'_, AppState>) -> Result<Vec<db::DailyStats>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_all_daily_stats(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_writing_settings(state: tauri::State<'_, AppState>) -> Result<db::WritingSettings, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_writing_settings(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_writing_settings(state: tauri::State<'_, AppState>, daily_goal: i64, session_gap_minutes: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_writing_settings(conn, daily_goal, session_gap_minutes).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_writing_activity(state: tauri::State<'_, AppState>, date: String) -> Result<Vec<db::WritingActivity>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_writing_activity(conn, &date).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_manuscript_word_history(state: tauri::State<'_, AppState>) -> Result<Vec<(String, i64)>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_manuscript_word_history(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_hourly_breakdown(state: tauri::State<'_, AppState>, date: String) -> Result<Vec<db::HourlyStats>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_hourly_breakdown(conn, &date).map_err(|e| e.to_string())
}

// ---- Navigation history ----

#[tauri::command]
fn get_nav_history(state: tauri::State<'_, AppState>) -> Result<Vec<db::NavEntry>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_nav_history(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn push_nav_entry(state: tauri::State<'_, AppState>, chapter_id: i64, scroll_top: f64, cursor_pos: i64) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::push_nav_entry(conn, chapter_id, scroll_top, cursor_pos).map_err(|e| e.to_string())
}

#[tauri::command]
fn truncate_nav_after(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::truncate_nav_after(conn, id).map_err(|e| e.to_string())
}

// ---- PDF export ----

fn chapter_to_text(chapter: &db::Chapter) -> Result<String, String> {
    let doc = ydoc::load_doc(&chapter.content)?;
    Ok(ydoc::extract_text_with_breaks(&doc))
}

#[tauri::command]
fn export_pdf(state: tauri::State<'_, AppState>, path: String, title: String, format: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let is_book = format == "book";

    // Embedded Liberation Serif fonts — bundled in the binary, full Unicode support
    let regular = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-Regular.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;
    let bold = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-Bold.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;
    let italic = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-Italic.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;
    let bold_italic = genpdf::fonts::FontData::new(
        include_bytes!("../fonts/LiberationSerif-BoldItalic.ttf").to_vec(),
        None,
    ).map_err(|e| format!("Font error: {}", e))?;

    let font_family = genpdf::fonts::FontFamily {
        regular,
        bold,
        italic,
        bold_italic,
    };
    let mut doc = genpdf::Document::new(font_family);

    // Page setup
    let page_size = if is_book {
        genpdf::Size::new(127, 203) // 5x8 inches in mm
    } else {
        genpdf::Size::new(210, 297) // A4
    };

    struct NumberedPageDecorator {
        margins: genpdf::Margins,
        page: usize,
    }

    impl genpdf::PageDecorator for NumberedPageDecorator {
        fn decorate_page<'a>(
            &mut self,
            context: &genpdf::Context,
            mut area: genpdf::render::Area<'a>,
            style: genpdf::style::Style,
        ) -> Result<genpdf::render::Area<'a>, genpdf::error::Error> {
            self.page += 1;
            area.add_margins(self.margins);

            // Render page number at bottom (skip page 1 = title page)
            if self.page > 1 {
                // Shrink content area to leave room for footer
                let footer_height = genpdf::Mm::from(8);
                let content_height = area.size().height - footer_height;

                // Print footer at the bottom
                let mut footer_area = area.clone();
                footer_area.add_offset(genpdf::Position::new(0, content_height));
                let page_str = format!("{}", self.page - 1);
                let mut para = genpdf::elements::Paragraph::new(&page_str);
                para.set_alignment(genpdf::Alignment::Center);
                use genpdf::Element;
                para.render(context, footer_area, style.with_font_size(9))?;

                // Reduce content area so text doesn't overlap footer
                area.set_height(content_height);
            }

            Ok(area)
        }
    }

    let margins = if is_book {
        genpdf::Margins::trbl(15, 12, 20, 12)
    } else {
        genpdf::Margins::trbl(25, 25, 30, 25)
    };

    doc.set_paper_size(page_size);
    doc.set_page_decorator(NumberedPageDecorator { margins, page: 0 });
    doc.set_font_size(if is_book { 10 } else { 12 });
    doc.set_line_spacing(1.5);
    doc.set_title(&title);

    // Title page
    doc.push(genpdf::elements::Break::new(5.0));
    let mut title_para = genpdf::elements::Paragraph::new("");
    title_para.push(genpdf::style::StyledString::new(
        &title,
        genpdf::style::Style::new().bold().with_font_size(if is_book { 18 } else { 24 }),
    ));
    title_para.set_alignment(genpdf::Alignment::Center);
    doc.push(title_para);
    doc.push(genpdf::elements::Break::new(20.0));
    doc.push(genpdf::elements::PageBreak::new());

    // Chapters
    for chapter in &chapters {
        // Chapter title
        let mut ch_title = genpdf::elements::Paragraph::new("");
        ch_title.push(genpdf::style::StyledString::new(
            &chapter.title,
            genpdf::style::Style::new().bold().with_font_size(if is_book { 14 } else { 16 }),
        ));
        ch_title.set_alignment(genpdf::Alignment::Center);
        doc.push(ch_title);
        doc.push(genpdf::elements::Break::new(1.0));

        // Chapter content
        let plain = chapter_to_text(chapter).unwrap_or_default();
        for paragraph in plain.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() { continue; }
            if trimmed == "* * *" {
                let mut sep = genpdf::elements::Paragraph::new("* * *");
                sep.set_alignment(genpdf::Alignment::Center);
                doc.push(genpdf::elements::Break::new(0.5));
                doc.push(sep);
                doc.push(genpdf::elements::Break::new(0.5));
            } else {
                doc.push(genpdf::elements::Paragraph::new(trimmed));
            }
        }

        doc.push(genpdf::elements::PageBreak::new());
    }

    doc.render_to_file(&path).map_err(|e| e.to_string())?;

    Ok(())
}

// ---- Chapter word count (from Y.Doc) ----

#[tauri::command]
fn get_chapter_word_count(state: tauri::State<'_, AppState>, id: i64) -> Result<usize, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapter = db::get_chapter(conn, id).map_err(|e| e.to_string())?
        .ok_or("Chapter not found")?;
    let doc = ydoc::load_doc(&chapter.content)?;
    Ok(ydoc::word_count(&doc))
}

#[tauri::command]
fn get_all_chapter_word_counts(state: tauri::State<'_, AppState>) -> Result<Vec<(i64, usize)>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;
    let mut counts = Vec::new();
    for ch in &chapters {
        let doc = ydoc::load_doc(&ch.content)?;
        counts.push((ch.id, ydoc::word_count(&doc)));
    }
    Ok(counts)
}

// ---- Ignored words ----

#[tauri::command]
fn add_ignored_word(state: tauri::State<'_, AppState>, word: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_ignored_word(conn, &word).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_ignored_word(state: tauri::State<'_, AppState>, word: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_ignored_word(conn, &word).map_err(|e| e.to_string())
}

// ---- Comments / notes ----

#[tauri::command]
fn get_chapter_comments(state: tauri::State<'_, AppState>, chapter_id: i64) -> Result<Vec<db::Comment>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_chapter_comments(conn, chapter_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_comment(state: tauri::State<'_, AppState>, chapter_id: i64, note_text: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_comment(conn, chapter_id, &note_text).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_comment(state: tauri::State<'_, AppState>, id: i64, note_text: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_comment(conn, id, &note_text).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_comment(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_comment(conn, id).map_err(|e| e.to_string())
}

// ---- Entity state tracking (checkpoint model) ----

#[tauri::command]
fn get_entity_markers(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<db::StateMarker>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_markers(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_state_marker(state: tauri::State<'_, AppState>, entity_id: i64, chapter_id: i64) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_state_marker(conn, entity_id, chapter_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_state_marker_note(state: tauri::State<'_, AppState>, id: i64, note: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_state_marker_note(conn, id, &note).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_state_marker(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_state_marker(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_state_marker_value(state: tauri::State<'_, AppState>, marker_id: i64, fact_key: String, fact_value: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_state_marker_value(conn, marker_id, &fact_key, &fact_value).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_state_marker_value(state: tauri::State<'_, AppState>, id: i64, fact_key: String, fact_value: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_state_marker_value(conn, id, &fact_key, &fact_value).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_state_marker_value(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_state_marker_value(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_state_marker_entity_ref(state: tauri::State<'_, AppState>, marker_id: i64, ref_entity_id: i64, ref_active: bool) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_state_marker_entity_ref(conn, marker_id, ref_entity_id, ref_active).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_state_marker_entity_ref(state: tauri::State<'_, AppState>, id: i64, ref_active: bool) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_state_marker_entity_ref(conn, id, ref_active).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_state_marker(state: tauri::State<'_, AppState>, id: i64) -> Result<Option<db::StateMarker>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_state_marker(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_incoming_entity_refs(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<(i64, String, bool, i64, i64)>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_incoming_entity_refs(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_entity_state_keys(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_state_keys(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_distinct_state_keys(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_distinct_state_keys(conn).map_err(|e| e.to_string())
}

// ---- Time sections ----

#[tauri::command]
fn get_chapter_time_sections(state: tauri::State<'_, AppState>, chapter_id: i64) -> Result<Vec<ydoc::TimeSection>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapter = db::get_chapter(conn, chapter_id).map_err(|e| e.to_string())?
        .ok_or("Chapter not found")?;
    let doc = ydoc::load_doc(&chapter.content)?;
    Ok(ydoc::extract_time_sections(&doc))
}

#[tauri::command]
fn get_all_time_sections(state: tauri::State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for ch in &chapters {
        let doc = ydoc::load_doc(&ch.content)?;
        let sections = ydoc::extract_time_sections(&doc);
        result.push(serde_json::json!({
            "chapter_id": ch.id,
            "chapter_title": ch.title,
            "sort_order": ch.sort_order,
            "sections": sections,
        }));
    }
    Ok(result)
}

#[tauri::command]
fn get_time_section_order(state: tauri::State<'_, AppState>) -> Result<Vec<db::TimeSectionOrder>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_time_section_order(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_time_section_order(
    state: tauri::State<'_, AppState>,
    entries: Vec<(i64, i64, String, i64)>,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::save_time_section_order(conn, entries).map_err(|e| e.to_string())
}

#[tauri::command]
fn reset_time_section_order(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reset_time_section_order(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn resolve_entity_state_at(
    state: tauri::State<'_, AppState>,
    entity_id: i64,
    target_chapter_id: i64,
    target_section_index: i64,
) -> Result<db::ResolvedEntityState, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    // 1. Build story_order_map from DB or default
    let saved_order = db::get_time_section_order(conn).map_err(|e| e.to_string())?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let mut story_order_map: std::collections::HashMap<(i64, i64), i64> = std::collections::HashMap::new();

    if saved_order.is_empty() {
        // Default: chapter sort_order * 1000 + section_index
        for ch in &chapters {
            let doc = ydoc::load_doc(&ch.content)?;
            let sections = ydoc::extract_time_sections(&doc);
            for sec in &sections {
                story_order_map.insert(
                    (ch.id, sec.section_index),
                    ch.sort_order * 1000 + sec.section_index,
                );
            }
        }
    } else {
        for entry in &saved_order {
            story_order_map.insert(
                (entry.chapter_id, entry.section_index),
                entry.story_order,
            );
        }
    }

    // 2. Build state_section_map by loading Y.Docs for chapters that have state markers
    let all_markers = db::get_entity_markers(conn, entity_id).map_err(|e| e.to_string())?;
    let chapter_ids: std::collections::HashSet<i64> = all_markers.iter().map(|m| m.chapter_id).collect();

    let mut state_section_map: std::collections::HashMap<i64, (i64, i64)> = std::collections::HashMap::new();
    for ch_id in &chapter_ids {
        if let Some(ch) = chapters.iter().find(|c| c.id == *ch_id) {
            let doc = ydoc::load_doc(&ch.content)?;
            let markers = ydoc::locate_state_markers_in_sections(&doc);
            for (state_id, section_idx) in markers {
                state_section_map.insert(state_id, (*ch_id, section_idx));
            }
        }
    }

    // 3. Determine target story position
    let target_story_pos = *story_order_map
        .get(&(target_chapter_id, target_section_index))
        .unwrap_or(&(target_chapter_id * 1000));

    // 4. Resolve
    db::resolve_entity_state(conn, entity_id, &state_section_map, &story_order_map, target_story_pos)
        .map_err(|e| e.to_string())
}

// ---- Format profiles & pages ----

#[tauri::command]
fn get_format_profiles(state: tauri::State<'_, AppState>) -> Result<Vec<db::FormatProfile>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_format_profiles(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_format_profile(state: tauri::State<'_, AppState>, id: i64) -> Result<Option<db::FormatProfile>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_format_profile(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_format_profile(state: tauri::State<'_, AppState>, name: String, target_type: String, trim_width_in: f64, trim_height_in: f64) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_format_profile(conn, &name, &target_type, trim_width_in, trim_height_in).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_format_profile(
    state: tauri::State<'_, AppState>,
    id: i64, name: String, target_type: String,
    trim_width_in: f64, trim_height_in: f64,
    margin_top_in: f64, margin_bottom_in: f64,
    margin_outside_in: f64, margin_inside_in: f64,
    font_body: String, font_size_pt: f64, line_spacing: f64,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_format_profile(conn, id, &name, &target_type, trim_width_in, trim_height_in, margin_top_in, margin_bottom_in, margin_outside_in, margin_inside_in, &font_body, font_size_pt, line_spacing).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_format_profile(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_format_profile(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn seed_format_profiles(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::seed_default_profiles(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_format_pages(state: tauri::State<'_, AppState>) -> Result<Vec<db::FormatPage>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_format_pages(conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_format_page(state: tauri::State<'_, AppState>, page_role: String, title: String, position: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_format_page(conn, &page_role, &title, &position).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_format_page(state: tauri::State<'_, AppState>, id: i64, page_role: String, title: String, content: String, position: String, include_in: String, vertical_align: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_format_page(conn, id, &page_role, &title, &content, &position, &include_in, &vertical_align).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_format_page(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_format_page(conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_format_pages(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::reorder_format_pages(conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn duplicate_format_profile(state: tauri::State<'_, AppState>, source_id: i64, new_name: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::duplicate_format_profile(conn, source_id, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_profile_category(
    state: tauri::State<'_, AppState>,
    profile_id: i64,
    category: String,
    json: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_profile_category(conn, profile_id, &category, &json).map_err(|e| e.to_string())
}

#[tauri::command]
fn paste_format_profile_settings(
    state: tauri::State<'_, AppState>,
    target_id: i64,
    settings: serde_json::Value,
) -> Result<(), String> {
    let map = settings
        .as_object()
        .cloned()
        .ok_or("settings must be a JSON object")?;
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::paste_format_profile_settings(conn, target_id, map).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_page_exclusion(state: tauri::State<'_, AppState>, page_id: i64, profile_id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_page_exclusion(conn, page_id, profile_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_page_exclusion(state: tauri::State<'_, AppState>, page_id: i64, profile_id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_page_exclusion(conn, page_id, profile_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_page_exclusions(state: tauri::State<'_, AppState>) -> Result<Vec<db::PageExclusion>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::list_all_page_exclusions(conn).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize spell checker and synonym state at startup
    let spell_state = spellcheck::init_spellcheck();
    let synonym_state = synonyms::init_synonyms();

    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(None),
            db_path: Mutex::new(None),
        })
        .manage(semantic::SemanticState::new())
        .manage(format::FormatState::new())
        .manage(spell_state)
        .manage(synonym_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .register_uri_scheme_protocol("iwe", |ctx, request| {
            let path = request.uri().path();

            // Parse /preview/page/{index}
            if let Some(rest) = path.strip_prefix("/preview/page/") {
                let index_str = rest.trim_end_matches(".svg");
                if let Ok(page_index) = index_str.parse::<usize>() {
                    let format_state = ctx.app_handle().state::<format::FormatState>();

                    // Wrap in catch_unwind: typst_svg::svg() can panic on
                    // certain content (font issues, edge-case glyphs, etc.).
                    // Since this callback runs inside a webview2 COM handler
                    // that is extern "C", an unwinding panic would abort the
                    // entire process. Catching it lets us return a 500 instead.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        format::render_page_svg(format_state.inner(), page_index)
                    }));

                    match result {
                        Ok(Ok(svg)) => {
                            return tauri::http::Response::builder()
                                .status(200)
                                .header("Content-Type", "image/svg+xml")
                                .header("Access-Control-Allow-Origin", "*")
                                .body(svg.into_bytes())
                                .unwrap();
                        }
                        Ok(Err(e)) => {
                            eprintln!("[iwe://] render error for page {}: {}", page_index, e);
                            return tauri::http::Response::builder()
                                .status(500)
                                .header("Content-Type", "text/plain")
                                .header("Access-Control-Allow-Origin", "*")
                                .body(e.into_bytes())
                                .unwrap();
                        }
                        Err(_panic) => {
                            eprintln!("[iwe://] PANIC in render_page_svg for page {}", page_index);
                            return tauri::http::Response::builder()
                                .status(500)
                                .header("Content-Type", "text/plain")
                                .header("Access-Control-Allow-Origin", "*")
                                .body(b"Internal error: page render panicked".to_vec())
                                .unwrap();
                        }
                    }
                }
            }

            tauri::http::Response::builder()
                .status(404)
                .header("Content-Type", "text/plain")
                .body(b"Not found".to_vec())
                .unwrap()
        })
        .invoke_handler(tauri::generate_handler![
            get_backup_interval,
            set_backup_interval,
            open_project,
            close_project,
            get_project_setting,
            set_project_setting,
            get_book_cover,
            set_book_cover,
            clear_book_cover,
            get_project_cover_by_path,
            get_chapters,
            get_chapter,
            add_chapter,
            update_chapter_content,
            rename_chapter,
            update_chapter_metadata,
            delete_chapter,
            reorder_chapters,
            get_deleted_chapters,
            restore_chapter,
            get_entities,
            create_entity,
            update_entity,
            delete_entity,
            set_entity_visible,
            add_alias,
            get_entity_notes,
            add_entity_note,
            delete_entity_note,
            reorder_entity_notes,
            get_entity_free_notes,
            add_entity_free_note,
            update_entity_free_note,
            delete_entity_free_note,
            move_entity_free_note,
            reorder_entity_free_notes,
            get_chapter_planning_notes,
            get_all_chapter_planning_notes,
            add_chapter_planning_note,
            update_chapter_planning_note,
            delete_chapter_planning_note,
            reorder_chapter_planning_notes,
            move_chapter_planning_note,
            get_kanban_columns,
            add_kanban_column,
            update_kanban_column,
            delete_kanban_column,
            reorder_kanban_columns,
            get_all_kanban_cards,
            add_kanban_card,
            update_kanban_card,
            delete_kanban_card,
            move_kanban_card,
            reorder_kanban_cards,
            log_writing_activity,
            get_daily_stats,
            get_all_daily_stats,
            get_writing_settings,
            update_writing_settings,
            get_writing_activity,
            get_hourly_breakdown,
            get_manuscript_word_history,
            get_nav_history,
            push_nav_entry,
            truncate_nav_after,
            export_pdf,
            get_chapter_word_count,
            get_all_chapter_word_counts,
            add_ignored_word,
            remove_ignored_word,
            remove_alias,
            get_chapter_comments,
            add_comment,
            update_comment,
            delete_comment,
            get_entity_markers,
            add_state_marker,
            update_state_marker_note,
            delete_state_marker,
            get_state_marker,
            add_state_marker_value,
            update_state_marker_value,
            delete_state_marker_value,
            add_state_marker_entity_ref,
            update_state_marker_entity_ref,
            get_incoming_entity_refs,
            get_entity_state_keys,
            get_distinct_state_keys,
            get_chapter_time_sections,
            get_all_time_sections,
            get_time_section_order,
            save_time_section_order,
            reset_time_section_order,
            resolve_entity_state_at,
            scanner::scan_text,
            scanner::scan_all_chapters,
            scanner::detect_entities,
            scanner::check_word,
            scanner::debug_search_terms,
            scanner::find_references,
            scanner::relationship_search,
            scanner::text_search,
            scanner::dialogue_search,
            analysis::word_frequency,
            analysis::find_similar_phrases,
            analysis::generate_heatmap,
            analysis::chapter_analysis,
            analysis::pacing_analysis,
            analysis::adverb_analysis,
            analysis::readability_analysis,
            analysis::paragraph_length_analysis,
            analysis::get_chapter_dialogue,
            analysis::extract_dialogue_in_text,
            analysis::debug_dialogue_spans,
            scanner::debug_stripped_text,
            spellcheck::check_spelling,
            spellcheck::get_spell_suggestions,
            spellcheck::debug_spell_check,
            spellcheck::add_to_dictionary,
            spellcheck::remove_from_dictionary,
            spellcheck::get_custom_words,
            spellcheck::set_spell_language,
            spellcheck::get_spell_language,
            synonyms::get_synonyms,
            palettes::get_palettes,
            palettes::create_palette,
            palettes::update_palette,
            palettes::delete_palette,
            palettes::toggle_palette,
            palettes::copy_palette,
            palettes::get_word_groups,
            palettes::get_word_group,
            palettes::create_word_group,
            palettes::update_word_group,
            palettes::delete_word_group,
            palettes::get_all_section_names,
            palettes::add_section,
            palettes::add_all_sections,
            palettes::rename_section,
            palettes::delete_section,
            palettes::add_word_entry,
            palettes::add_word_entries,
            palettes::remove_word_entry,
            palettes::search_word_groups,
            palettes::search_palette_entries,
            palettes::get_active_groups,
            semantic::mark_chapter_dirty,
            semantic::run_semantic_indexing,
            semantic::rebuild_semantic_index,
            semantic::semantic_search,
            semantic::get_semantic_index_status,
            format::compile_preview,
            epub::export_epub,
            validate_epub_bytes,
            format::export_format_pdf,
            format::list_system_fonts,
            import::parse_import_file,
            famous_books::list_library_books,
            famous_books::get_library_book,
            famous_books::save_library_book,
            famous_books::delete_library_book,
            get_format_profiles,
            get_format_profile,
            add_format_profile,
            update_format_profile,
            delete_format_profile,
            duplicate_format_profile,
            update_profile_category,
            paste_format_profile_settings,
            seed_format_profiles,
            get_format_pages,
            add_format_page,
            update_format_page,
            delete_format_page,
            reorder_format_pages,
            add_page_exclusion,
            remove_page_exclusion,
            list_page_exclusions,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Initialize palettes DB in app data directory
            let app_data = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data).ok();
            let palette_path = app_data.join("palettes.db");
            let palette_conn = palettes::init_palette_db(
                palette_path.to_str().expect("Invalid palette path")
            ).expect("Failed to init palettes DB");
            app.manage(palettes::PaletteState {
                db: Mutex::new(palette_conn),
            });

            // Initialize famous-books library DB
            let library_path = app_data.join("famous_books.db");
            let library_conn = famous_books::init_library_db(
                library_path.to_str().expect("Invalid library path")
            ).expect("Failed to init library DB");
            app.manage(famous_books::LibraryState {
                db: Mutex::new(library_conn),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
