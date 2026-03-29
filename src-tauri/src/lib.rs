mod db;
mod scanner;
mod wordlists;

use db::AppState;
use rusqlite::Connection;
use std::sync::Mutex;

// ---- Project commands ----

#[tauri::command]
fn open_project(state: tauri::State<'_, AppState>, filepath: String) -> Result<(), String> {
    let conn = Connection::open(&filepath).map_err(|e| e.to_string())?;
    db::init_schema(&conn).map_err(|e| e.to_string())?;
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    Ok(())
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
fn update_chapter_content(state: tauri::State<'_, AppState>, id: i64, content: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_chapter_content(conn, id, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_chapter(state: tauri::State<'_, AppState>, id: i64, title: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::rename_chapter(conn, id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_chapter(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_chapter(conn, id).map_err(|e| e.to_string())
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
    db::create_entity(conn, &name, &entity_type, &description, &color.unwrap_or_default()).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_entity(state: tauri::State<'_, AppState>, id: i64, name: String, entity_type: String, description: String, color: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::update_entity(conn, id, &name, &entity_type, &description, &color).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entity(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity(conn, id).map_err(|e| e.to_string())
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
    db::add_alias(conn, entity_id, &alias).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_alias(state: tauri::State<'_, AppState>, entity_id: i64, alias: String) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_alias(conn, entity_id, &alias).map_err(|e| e.to_string())
}

// ---- Entity notes ----

#[tauri::command]
fn get_entity_notes(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<Vec<db::EntityNote>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::get_entity_notes(conn, entity_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_entity_note(state: tauri::State<'_, AppState>, entity_id: i64, chapter_id: Option<i64>, text: String) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_entity_note(conn, entity_id, chapter_id, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entity_note(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::delete_entity_note(conn, id).map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(None),
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            open_project,
            get_chapters,
            get_chapter,
            add_chapter,
            update_chapter_content,
            rename_chapter,
            delete_chapter,
            get_entities,
            create_entity,
            update_entity,
            delete_entity,
            set_entity_visible,
            add_alias,
            get_entity_notes,
            add_entity_note,
            delete_entity_note,
            get_nav_history,
            push_nav_entry,
            truncate_nav_after,
            add_ignored_word,
            remove_ignored_word,
            remove_alias,
            scanner::scan_text,
            scanner::scan_all_chapters,
            scanner::detect_entities,
            scanner::check_word,
            scanner::find_references,
            scanner::relationship_search,
            scanner::text_search,
            scanner::dialogue_search,
            scanner::word_frequency,
            scanner::find_similar_phrases,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
