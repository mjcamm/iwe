use rusqlite::{params, Connection};
use serde::Serialize;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Option<Connection>>,
}

#[derive(Serialize, Clone)]
pub struct Chapter {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone)]
pub struct Entity {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub description: String,
    pub color: String,
    pub visible: bool,
    pub aliases: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn default_color(entity_type: &str) -> &'static str {
    match entity_type {
        "character" => "#2d6a5e",
        "place" => "#6a4c2d",
        "thing" => "#4c2d6a",
        _ => "#666666",
    }
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS chapters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS entities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            entity_type TEXT NOT NULL CHECK(entity_type IN ('character', 'place', 'thing')),
            description TEXT DEFAULT '',
            color TEXT DEFAULT '',
            visible INTEGER NOT NULL DEFAULT 1,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS aliases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            alias TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS entity_fields (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            field_name TEXT NOT NULL,
            field_value TEXT DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS ignored_words (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS entity_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            chapter_id INTEGER,
            text TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_entity_notes_entity ON entity_notes(entity_id);

        CREATE TABLE IF NOT EXISTS nav_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chapter_id INTEGER NOT NULL,
            scroll_top REAL NOT NULL DEFAULT 0,
            cursor_pos INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_aliases_entity ON aliases(entity_id);

        PRAGMA foreign_keys = ON;
    ")?;

    // Migration: add color column if missing (existing databases)
    let has_color: bool = conn
        .prepare("SELECT color FROM entities LIMIT 0")
        .is_ok();
    if !has_color {
        conn.execute_batch("ALTER TABLE entities ADD COLUMN color TEXT DEFAULT '';")?;
    }

    // Migration: add visible column if missing
    let has_visible: bool = conn
        .prepare("SELECT visible FROM entities LIMIT 0")
        .is_ok();
    if !has_visible {
        conn.execute_batch("ALTER TABLE entities ADD COLUMN visible INTEGER NOT NULL DEFAULT 1;")?;
    }

    Ok(())
}

// ---- Chapter operations ----

pub fn list_chapters(conn: &Connection) -> rusqlite::Result<Vec<Chapter>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, sort_order, created_at, updated_at FROM chapters ORDER BY sort_order ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Chapter {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            sort_order: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_chapter(conn: &Connection, id: i64) -> rusqlite::Result<Option<Chapter>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, sort_order, created_at, updated_at FROM chapters WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(Chapter {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            sort_order: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn add_chapter(conn: &Connection, title: &str) -> rusqlite::Result<i64> {
    let max_order: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) FROM chapters", [], |row| row.get(0))?;
    conn.execute(
        "INSERT INTO chapters (title, content, sort_order) VALUES (?1, '', ?2)",
        params![title, max_order + 1],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_chapter_content(conn: &Connection, id: i64, content: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE chapters SET content = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![content, id],
    )?;
    Ok(())
}

pub fn rename_chapter(conn: &Connection, id: i64, title: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE chapters SET title = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![title, id],
    )?;
    Ok(())
}

pub fn delete_chapter(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM chapters WHERE id = ?1", params![id])?;
    Ok(())
}

// ---- Entity operations ----

pub fn list_entities(conn: &Connection) -> rusqlite::Result<Vec<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, entity_type, COALESCE(description, ''), COALESCE(color, ''), visible, created_at, updated_at FROM entities ORDER BY name ASC"
    )?;
    let mut entities: Vec<Entity> = stmt.query_map([], |row| {
        let entity_type: String = row.get(2)?;
        let color: String = row.get(4)?;
        let visible: i64 = row.get(5)?;
        Ok(Entity {
            id: row.get(0)?,
            name: row.get(1)?,
            entity_type: entity_type.clone(),
            description: row.get(3)?,
            color: if color.is_empty() { default_color(&entity_type).to_string() } else { color },
            visible: visible != 0,
            aliases: Vec::new(),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    // Batch load all aliases
    let mut alias_stmt = conn.prepare("SELECT entity_id, alias FROM aliases ORDER BY entity_id")?;
    let alias_rows = alias_stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut alias_map: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    for row in alias_rows {
        let (entity_id, alias) = row?;
        alias_map.entry(entity_id).or_default().push(alias);
    }

    for entity in &mut entities {
        if let Some(aliases) = alias_map.remove(&entity.id) {
            entity.aliases = aliases;
        }
    }

    Ok(entities)
}

pub fn create_entity(conn: &Connection, name: &str, entity_type: &str, description: &str, color: &str) -> rusqlite::Result<i64> {
    let actual_color = if color.is_empty() { default_color(entity_type) } else { color };
    conn.execute(
        "INSERT INTO entities (name, entity_type, description, color) VALUES (?1, ?2, ?3, ?4)",
        params![name, entity_type, description, actual_color],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_entity(conn: &Connection, id: i64, name: &str, entity_type: &str, description: &str, color: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entities SET name = ?1, entity_type = ?2, description = ?3, color = ?4, updated_at = CURRENT_TIMESTAMP WHERE id = ?5",
        params![name, entity_type, description, color, id],
    )?;
    Ok(())
}

pub fn delete_entity(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM aliases WHERE entity_id = ?1", params![id])?;
    conn.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_entity_visible(conn: &Connection, id: i64, visible: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entities SET visible = ?1 WHERE id = ?2",
        params![visible as i64, id],
    )?;
    Ok(())
}

pub fn add_alias(conn: &Connection, entity_id: i64, alias: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO aliases (entity_id, alias) VALUES (?1, ?2)",
        params![entity_id, alias],
    )?;
    Ok(())
}

pub fn remove_alias(conn: &Connection, entity_id: i64, alias: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM aliases WHERE entity_id = ?1 AND alias = ?2",
        params![entity_id, alias],
    )?;
    Ok(())
}

// ---- Ignored words ----

pub fn list_ignored_words(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT word FROM ignored_words ORDER BY word ASC")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn add_ignored_word(conn: &Connection, word: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO ignored_words (word) VALUES (?1)",
        params![word],
    )?;
    Ok(())
}

pub fn remove_ignored_word(conn: &Connection, word: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM ignored_words WHERE word = ?1", params![word])?;
    Ok(())
}

// ---- Entity notes (pinned excerpts) ----

#[derive(Serialize, Clone)]
pub struct EntityNote {
    pub id: i64,
    pub entity_id: i64,
    pub chapter_id: Option<i64>,
    pub text: String,
    pub created_at: String,
}

pub fn get_entity_notes(conn: &Connection, entity_id: i64) -> rusqlite::Result<Vec<EntityNote>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, n.entity_id, n.chapter_id, n.text, n.created_at FROM entity_notes n WHERE n.entity_id = ?1 ORDER BY n.created_at DESC"
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        Ok(EntityNote {
            id: row.get(0)?,
            entity_id: row.get(1)?,
            chapter_id: row.get(2)?,
            text: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn add_entity_note(conn: &Connection, entity_id: i64, chapter_id: Option<i64>, text: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entity_notes (entity_id, chapter_id, text) VALUES (?1, ?2, ?3)",
        params![entity_id, chapter_id, text],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_entity_note(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entity_notes WHERE id = ?1", params![id])?;
    Ok(())
}

// ---- Navigation history ----

#[derive(Serialize, Clone)]
pub struct NavEntry {
    pub id: i64,
    pub chapter_id: i64,
    pub scroll_top: f64,
    pub cursor_pos: i64,
    pub created_at: String,
}

pub fn get_nav_history(conn: &Connection) -> rusqlite::Result<Vec<NavEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, chapter_id, scroll_top, cursor_pos, created_at FROM nav_history ORDER BY id ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(NavEntry {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            scroll_top: row.get(2)?,
            cursor_pos: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn push_nav_entry(conn: &Connection, chapter_id: i64, scroll_top: f64, cursor_pos: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO nav_history (chapter_id, scroll_top, cursor_pos) VALUES (?1, ?2, ?3)",
        params![chapter_id, scroll_top, cursor_pos],
    )?;
    let id = conn.last_insert_rowid();
    // Cap at 100 entries — delete oldest
    conn.execute(
        "DELETE FROM nav_history WHERE id NOT IN (SELECT id FROM nav_history ORDER BY id DESC LIMIT 100)",
        [],
    )?;
    Ok(id)
}

pub fn truncate_nav_after(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM nav_history WHERE id > ?1", params![id])?;
    Ok(())
}

pub fn clear_nav_history(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM nav_history", [])?;
    Ok(())
}
