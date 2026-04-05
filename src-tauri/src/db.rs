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
    pub content: Vec<u8>,
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
            content BLOB NOT NULL DEFAULT x'',
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
            y_start BLOB NOT NULL,
            y_end BLOB NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_entity_notes_entity ON entity_notes(entity_id);

        CREATE TABLE IF NOT EXISTS entity_free_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            text TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_entity_free_notes_entity ON entity_free_notes(entity_id);

        CREATE TABLE IF NOT EXISTS writing_activity (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')),
            chapter_id INTEGER,
            chapter_words INTEGER NOT NULL DEFAULT 0,
            manuscript_words INTEGER NOT NULL DEFAULT 0,
            words_delta INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_writing_activity_ts ON writing_activity(timestamp);

        CREATE TABLE IF NOT EXISTS daily_stats (
            date TEXT PRIMARY KEY,
            words_added INTEGER NOT NULL DEFAULT 0,
            words_deleted INTEGER NOT NULL DEFAULT 0,
            net_words INTEGER NOT NULL DEFAULT 0,
            active_minutes INTEGER NOT NULL DEFAULT 0,
            chapters_touched TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS writing_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            daily_goal INTEGER NOT NULL DEFAULT 1000,
            session_gap_minutes INTEGER NOT NULL DEFAULT 30
        );

        INSERT OR IGNORE INTO writing_settings (id) VALUES (1);

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

    // Migration: custom_words table for spell checker
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS custom_words (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL UNIQUE,
            source TEXT NOT NULL DEFAULT 'user'
        );
    ")?;

    // Comments / notes table
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chapter_id INTEGER NOT NULL,
            note_text TEXT NOT NULL DEFAULT '',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_comments_chapter ON comments(chapter_id);
    ")?;

    // Entity state tracking — markers (checkpoints) with child values
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS state_markers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            chapter_id INTEGER NOT NULL,
            note TEXT NOT NULL DEFAULT '',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_state_markers_entity ON state_markers(entity_id);
        CREATE INDEX IF NOT EXISTS idx_state_markers_chapter ON state_markers(chapter_id);

        CREATE TABLE IF NOT EXISTS state_marker_values (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            marker_id INTEGER NOT NULL REFERENCES state_markers(id) ON DELETE CASCADE,
            value_type TEXT NOT NULL DEFAULT 'fact',
            fact_key TEXT NOT NULL DEFAULT '',
            fact_value TEXT NOT NULL DEFAULT '',
            ref_entity_id INTEGER REFERENCES entities(id) ON DELETE SET NULL,
            ref_active INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX IF NOT EXISTS idx_state_marker_values_marker ON state_marker_values(marker_id);
    ")?;

    // Time section ordering (for non-linear narratives)
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS time_section_order (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chapter_id INTEGER NOT NULL,
            section_index INTEGER NOT NULL DEFAULT 0,
            label TEXT NOT NULL DEFAULT '',
            story_order INTEGER NOT NULL DEFAULT 0,
            UNIQUE(chapter_id, section_index)
        );
    ")?;

    // Migration: add title column to entity_free_notes
    let has_free_note_title: bool = conn
        .prepare("SELECT title FROM entity_free_notes LIMIT 0")
        .is_ok();
    if !has_free_note_title {
        conn.execute_batch("ALTER TABLE entity_free_notes ADD COLUMN title TEXT NOT NULL DEFAULT '';")?;
    }

    // Kanban: chapter planning notes
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS chapter_planning_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chapter_id INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
            title TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_chapter_planning_notes_chapter ON chapter_planning_notes(chapter_id);
    ")?;

    // Kanban: freeform columns
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS kanban_columns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    ")?;

    // Kanban: freeform cards
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS kanban_cards (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            column_id INTEGER NOT NULL REFERENCES kanban_columns(id) ON DELETE CASCADE,
            title TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_kanban_cards_column ON kanban_cards(column_id);
    ")?;

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
    let empty: Vec<u8> = Vec::new();
    conn.execute(
        "INSERT INTO chapters (title, content, sort_order) VALUES (?1, ?2, ?3)",
        params![title, empty, max_order + 1],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_chapter_content(conn: &Connection, id: i64, content: &[u8]) -> rusqlite::Result<()> {
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

pub fn reorder_chapters(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE chapters SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
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
    let trimmed = alias.trim();
    if trimmed.is_empty() { return Ok(()); }
    conn.execute(
        "INSERT INTO aliases (entity_id, alias) VALUES (?1, ?2)",
        params![entity_id, trimmed],
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

// ---- Custom words (spell checker) ----

pub fn list_custom_words(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT word FROM custom_words ORDER BY word ASC")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn get_custom_words_full(conn: &Connection) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare("SELECT id, word, source FROM custom_words ORDER BY word ASC")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    rows.collect()
}

pub fn add_custom_word(conn: &Connection, word: &str, source: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO custom_words (word, source) VALUES (?1, ?2)",
        params![word, source],
    )?;
    Ok(())
}

pub fn remove_custom_word(conn: &Connection, word: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM custom_words WHERE word = ?1", params![word])?;
    Ok(())
}

/// Sync entity names/aliases into custom_words with source='entity'.
/// Called when entities change.
pub fn sync_entity_words(conn: &Connection) -> rusqlite::Result<()> {
    // Remove old entity-sourced words
    conn.execute("DELETE FROM custom_words WHERE source = 'entity'", [])?;

    // Add current entity names and aliases
    let entities = list_entities(conn)?;
    for ent in &entities {
        for word in ent.name.split_whitespace() {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO custom_words (word, source) VALUES (?1, 'entity')",
                params![word],
            );
        }
        for alias in &ent.aliases {
            for word in alias.split_whitespace() {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO custom_words (word, source) VALUES (?1, 'entity')",
                    params![word],
                );
            }
        }
    }

    Ok(())
}

// ---- Entity notes (pinned excerpts) ----

#[derive(Serialize, Clone)]
pub struct EntityNote {
    pub id: i64,
    pub entity_id: i64,
    pub chapter_id: Option<i64>,
    pub y_start: Vec<u8>,
    pub y_end: Vec<u8>,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct EntityFreeNote {
    pub id: i64,
    pub entity_id: i64,
    pub title: String,
    pub text: String,
    pub sort_order: i64,
    pub created_at: String,
}

pub fn get_entity_notes(conn: &Connection, entity_id: i64) -> rusqlite::Result<Vec<EntityNote>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, n.entity_id, n.chapter_id, n.y_start, n.y_end, COALESCE(n.sort_order, 0), n.created_at FROM entity_notes n WHERE n.entity_id = ?1 ORDER BY n.sort_order ASC, n.created_at DESC"
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        Ok(EntityNote {
            id: row.get(0)?,
            entity_id: row.get(1)?,
            chapter_id: row.get(2)?,
            y_start: row.get(3)?,
            y_end: row.get(4)?,
            sort_order: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn add_entity_note(conn: &Connection, entity_id: i64, chapter_id: Option<i64>, y_start: &[u8], y_end: &[u8]) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entity_notes (entity_id, chapter_id, y_start, y_end) VALUES (?1, ?2, ?3, ?4)",
        params![entity_id, chapter_id, y_start, y_end],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_entity_note(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entity_notes WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn reorder_entity_notes(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE entity_notes SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}

// ---- Entity free notes (user-written cards) ----

pub fn get_entity_free_notes(conn: &Connection, entity_id: i64) -> rusqlite::Result<Vec<EntityFreeNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_id, title, text, sort_order, created_at FROM entity_free_notes WHERE entity_id = ?1 ORDER BY sort_order ASC, created_at ASC"
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        Ok(EntityFreeNote {
            id: row.get(0)?,
            entity_id: row.get(1)?,
            title: row.get(2)?,
            text: row.get(3)?,
            sort_order: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn add_entity_free_note(conn: &Connection, entity_id: i64, title: &str, text: &str) -> rusqlite::Result<i64> {
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM entity_free_notes WHERE entity_id = ?1",
        params![entity_id], |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO entity_free_notes (entity_id, title, text, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![entity_id, title, text, max_order + 1],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_entity_free_note(conn: &Connection, id: i64, title: &str, text: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE entity_free_notes SET title = ?1, text = ?2 WHERE id = ?3", params![title, text, id])?;
    Ok(())
}

pub fn delete_entity_free_note(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entity_free_notes WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn move_entity_free_note(conn: &Connection, id: i64, new_entity_id: i64) -> rusqlite::Result<()> {
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM entity_free_notes WHERE entity_id = ?1",
        params![new_entity_id], |row| row.get(0),
    )?;
    conn.execute(
        "UPDATE entity_free_notes SET entity_id = ?1, sort_order = ?2 WHERE id = ?3",
        params![new_entity_id, max_order + 1, id],
    )?;
    Ok(())
}

pub fn reorder_entity_free_notes(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE entity_free_notes SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}

// ---- Writing stats ----

#[derive(Serialize, Clone)]
pub struct WritingActivity {
    pub id: i64,
    pub timestamp: String,
    pub chapter_id: Option<i64>,
    pub chapter_words: i64,
    pub manuscript_words: i64,
    pub words_delta: i64,
}

#[derive(Serialize, Clone)]
pub struct DailyStats {
    pub date: String,
    pub words_added: i64,
    pub words_deleted: i64,
    pub net_words: i64,
    pub active_minutes: i64,
    pub chapters_touched: String,
}

#[derive(Serialize, Clone)]
pub struct WritingSettings {
    pub daily_goal: i64,
    pub session_gap_minutes: i64,
}

pub fn log_writing_activity(conn: &Connection, chapter_id: Option<i64>, chapter_words: i64, manuscript_words: i64, words_delta: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO writing_activity (chapter_id, chapter_words, manuscript_words, words_delta) VALUES (?1, ?2, ?3, ?4)",
        params![chapter_id, chapter_words, manuscript_words, words_delta],
    )?;

    // Update daily stats
    let today: String = conn.query_row(
        "SELECT strftime('%Y-%m-%d', 'now', 'localtime')", [], |row| row.get(0)
    )?;

    let added = if words_delta > 0 { words_delta } else { 0 };
    let deleted = if words_delta < 0 { -words_delta } else { 0 };

    conn.execute(
        "INSERT INTO daily_stats (date, words_added, words_deleted, net_words, active_minutes, chapters_touched)
         VALUES (?1, ?2, ?3, ?4, 0, '[]')
         ON CONFLICT(date) DO UPDATE SET
           words_added = words_added + ?2,
           words_deleted = words_deleted + ?3,
           net_words = net_words + ?4",
        params![today, added, deleted, words_delta],
    )?;

    // Update active minutes: check time since last activity
    let last_ts: Option<String> = conn.query_row(
        "SELECT timestamp FROM writing_activity WHERE id != last_insert_rowid() ORDER BY id DESC LIMIT 1",
        [], |row| row.get(0)
    ).ok();

    if let Some(last) = last_ts {
        // Parse timestamps and compute gap
        let gap_secs: Option<i64> = conn.query_row(
            "SELECT CAST((julianday('now', 'localtime') - julianday(?1)) * 86400 AS INTEGER)",
            params![last], |row| row.get(0)
        ).ok();

        if let Some(gap) = gap_secs {
            let settings = get_writing_settings(conn)?;
            let max_gap = settings.session_gap_minutes * 60;
            if gap > 0 && gap < max_gap {
                let minutes = (gap + 30) / 60; // round to nearest minute
                conn.execute(
                    "UPDATE daily_stats SET active_minutes = active_minutes + ?1 WHERE date = ?2",
                    params![minutes.max(1), today],
                )?;
            }
        }
    }

    // Update chapters touched
    if let Some(ch_id) = chapter_id {
        let current: String = conn.query_row(
            "SELECT chapters_touched FROM daily_stats WHERE date = ?1",
            params![today], |row| row.get(0)
        ).unwrap_or_else(|_| "[]".to_string());

        let ch_str = ch_id.to_string();
        if !current.contains(&ch_str) {
            let updated = if current == "[]" {
                format!("[{}]", ch_id)
            } else {
                format!("{}, {}]", &current[..current.len()-1], ch_id)
            };
            conn.execute(
                "UPDATE daily_stats SET chapters_touched = ?1 WHERE date = ?2",
                params![updated, today],
            )?;
        }
    }

    Ok(())
}

pub fn get_daily_stats(conn: &Connection, days: i64) -> rusqlite::Result<Vec<DailyStats>> {
    let mut stmt = conn.prepare(
        "SELECT date, words_added, words_deleted, net_words, active_minutes, chapters_touched
         FROM daily_stats
         WHERE date >= date('now', 'localtime', ?1)
         ORDER BY date ASC"
    )?;
    let offset = format!("-{} days", days);
    let rows = stmt.query_map(params![offset], |row| {
        Ok(DailyStats {
            date: row.get(0)?,
            words_added: row.get(1)?,
            words_deleted: row.get(2)?,
            net_words: row.get(3)?,
            active_minutes: row.get(4)?,
            chapters_touched: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_all_daily_stats(conn: &Connection) -> rusqlite::Result<Vec<DailyStats>> {
    let mut stmt = conn.prepare(
        "SELECT date, words_added, words_deleted, net_words, active_minutes, chapters_touched
         FROM daily_stats ORDER BY date ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(DailyStats {
            date: row.get(0)?,
            words_added: row.get(1)?,
            words_deleted: row.get(2)?,
            net_words: row.get(3)?,
            active_minutes: row.get(4)?,
            chapters_touched: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_writing_settings(conn: &Connection) -> rusqlite::Result<WritingSettings> {
    conn.query_row(
        "SELECT daily_goal, session_gap_minutes FROM writing_settings WHERE id = 1",
        [], |row| Ok(WritingSettings {
            daily_goal: row.get(0)?,
            session_gap_minutes: row.get(1)?,
        })
    )
}

pub fn update_writing_settings(conn: &Connection, daily_goal: i64, session_gap_minutes: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE writing_settings SET daily_goal = ?1, session_gap_minutes = ?2 WHERE id = 1",
        params![daily_goal, session_gap_minutes],
    )?;
    Ok(())
}

pub fn get_writing_activity(conn: &Connection, date: &str) -> rusqlite::Result<Vec<WritingActivity>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, chapter_id, chapter_words, manuscript_words, words_delta
         FROM writing_activity
         WHERE timestamp LIKE ?1
         ORDER BY timestamp ASC"
    )?;
    let pattern = format!("{}%", date);
    let rows = stmt.query_map(params![pattern], |row| {
        Ok(WritingActivity {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            chapter_id: row.get(2)?,
            chapter_words: row.get(3)?,
            manuscript_words: row.get(4)?,
            words_delta: row.get(5)?,
        })
    })?;
    rows.collect()
}

#[derive(Serialize, Clone)]
pub struct HourlyStats {
    pub hour: i64,
    pub words_added: i64,
    pub words_deleted: i64,
    pub net_words: i64,
    pub events: i64,
}

pub fn get_hourly_breakdown(conn: &Connection, date: &str) -> rusqlite::Result<Vec<HourlyStats>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', timestamp) AS INTEGER) as hour,
                SUM(CASE WHEN words_delta > 0 THEN words_delta ELSE 0 END) as added,
                SUM(CASE WHEN words_delta < 0 THEN -words_delta ELSE 0 END) as deleted,
                SUM(words_delta) as net,
                COUNT(*) as events
         FROM writing_activity
         WHERE timestamp LIKE ?1
         GROUP BY hour
         ORDER BY hour ASC"
    )?;
    let pattern = format!("{}%", date);
    let rows = stmt.query_map(params![pattern], |row| {
        Ok(HourlyStats {
            hour: row.get(0)?,
            words_added: row.get(1)?,
            words_deleted: row.get(2)?,
            net_words: row.get(3)?,
            events: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn get_manuscript_word_history(conn: &Connection) -> rusqlite::Result<Vec<(String, i64)>> {
    // Get the last activity per day to track manuscript growth
    let mut stmt = conn.prepare(
        "SELECT date(timestamp) as d, manuscript_words
         FROM writing_activity
         WHERE id IN (SELECT MAX(id) FROM writing_activity GROUP BY date(timestamp))
         ORDER BY d ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect()
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

// ---- Comments / notes ----

#[derive(Serialize, Clone)]
pub struct Comment {
    pub id: i64,
    pub chapter_id: i64,
    pub note_text: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn get_chapter_comments(conn: &Connection, chapter_id: i64) -> rusqlite::Result<Vec<Comment>> {
    let mut stmt = conn.prepare(
        "SELECT id, chapter_id, note_text, created_at, updated_at FROM comments WHERE chapter_id = ?1 ORDER BY created_at ASC"
    )?;
    let rows = stmt.query_map(params![chapter_id], |row| {
        Ok(Comment {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            note_text: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn add_comment(conn: &Connection, chapter_id: i64, note_text: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO comments (chapter_id, note_text) VALUES (?1, ?2)",
        params![chapter_id, note_text],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_comment(conn: &Connection, id: i64, note_text: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE comments SET note_text = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![note_text, id],
    )?;
    Ok(())
}

pub fn delete_comment(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM comments WHERE id = ?1", params![id])?;
    Ok(())
}

// ---- Entity state tracking (checkpoint model) ----

#[derive(Serialize, Clone)]
pub struct StateMarker {
    pub id: i64,
    pub entity_id: i64,
    pub chapter_id: i64,
    pub note: String,
    pub created_at: String,
    pub values: Vec<StateMarkerValue>,
}

#[derive(Serialize, Clone)]
pub struct StateMarkerValue {
    pub id: i64,
    pub marker_id: i64,
    pub value_type: String,
    pub fact_key: String,
    pub fact_value: String,
    pub ref_entity_id: Option<i64>,
    pub ref_entity_name: Option<String>,
    pub ref_active: bool,
}

pub fn get_entity_markers(conn: &Connection, entity_id: i64) -> rusqlite::Result<Vec<StateMarker>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_id, chapter_id, note, created_at
         FROM state_markers WHERE entity_id = ?1 ORDER BY created_at ASC"
    )?;
    let mut markers: Vec<StateMarker> = stmt.query_map(params![entity_id], |row| {
        Ok(StateMarker {
            id: row.get(0)?,
            entity_id: row.get(1)?,
            chapter_id: row.get(2)?,
            note: row.get(3)?,
            created_at: row.get(4)?,
            values: Vec::new(),
        })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    // Batch load values
    if !markers.is_empty() {
        let ids: Vec<i64> = markers.iter().map(|m| m.id).collect();
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT v.id, v.marker_id, v.value_type, v.fact_key, v.fact_value, v.ref_entity_id, e.name, v.ref_active
             FROM state_marker_values v
             LEFT JOIN entities e ON e.id = v.ref_entity_id
             WHERE v.marker_id IN ({}) ORDER BY v.id ASC",
            placeholders.join(",")
        );
        let mut val_stmt = conn.prepare(&sql)?;
        let val_rows = val_stmt.query_map(rusqlite::params_from_iter(&ids), |row| {
            let ref_active_val: i64 = row.get(7)?;
            Ok(StateMarkerValue {
                id: row.get(0)?,
                marker_id: row.get(1)?,
                value_type: row.get(2)?,
                fact_key: row.get(3)?,
                fact_value: row.get(4)?,
                ref_entity_id: row.get(5)?,
                ref_entity_name: row.get(6)?,
                ref_active: ref_active_val != 0,
            })
        })?;

        let mut val_map: std::collections::HashMap<i64, Vec<StateMarkerValue>> = std::collections::HashMap::new();
        for row in val_rows {
            let v = row?;
            val_map.entry(v.marker_id).or_default().push(v);
        }
        for marker in &mut markers {
            if let Some(vals) = val_map.remove(&marker.id) {
                marker.values = vals;
            }
        }
    }

    Ok(markers)
}

pub fn add_state_marker(conn: &Connection, entity_id: i64, chapter_id: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO state_markers (entity_id, chapter_id) VALUES (?1, ?2)",
        params![entity_id, chapter_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_state_marker_note(conn: &Connection, id: i64, note: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE state_markers SET note = ?1 WHERE id = ?2",
        params![note, id],
    )?;
    Ok(())
}

pub fn delete_state_marker(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    // Cascade deletes child values
    conn.execute("DELETE FROM state_markers WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn add_state_marker_value(conn: &Connection, marker_id: i64, fact_key: &str, fact_value: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO state_marker_values (marker_id, value_type, fact_key, fact_value) VALUES (?1, 'fact', ?2, ?3)",
        params![marker_id, fact_key, fact_value],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn add_state_marker_entity_ref(conn: &Connection, marker_id: i64, ref_entity_id: i64, ref_active: bool) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO state_marker_values (marker_id, value_type, ref_entity_id, ref_active) VALUES (?1, 'entity_ref', ?2, ?3)",
        params![marker_id, ref_entity_id, ref_active as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_state_marker_value(conn: &Connection, id: i64, fact_key: &str, fact_value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE state_marker_values SET fact_key = ?1, fact_value = ?2 WHERE id = ?3",
        params![fact_key, fact_value, id],
    )?;
    Ok(())
}

pub fn update_state_marker_entity_ref(conn: &Connection, id: i64, ref_active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE state_marker_values SET ref_active = ?1 WHERE id = ?2",
        params![ref_active as i64, id],
    )?;
    Ok(())
}

pub fn delete_state_marker_value(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM state_marker_values WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_state_marker(conn: &Connection, id: i64) -> rusqlite::Result<Option<StateMarker>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_id, chapter_id, note, created_at FROM state_markers WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(StateMarker {
            id: row.get(0)?,
            entity_id: row.get(1)?,
            chapter_id: row.get(2)?,
            note: row.get(3)?,
            created_at: row.get(4)?,
            values: Vec::new(),
        })
    })?;
    match rows.next() {
        Some(Ok(m)) => Ok(Some(m)),
        _ => Ok(None),
    }
}

pub fn get_distinct_state_keys(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT fact_key FROM state_marker_values WHERE value_type = 'fact' AND fact_key != '' ORDER BY fact_key ASC"
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn get_incoming_entity_refs(conn: &Connection, entity_id: i64) -> rusqlite::Result<Vec<(i64, String, bool, i64, i64)>> {
    // Returns (source_entity_id, source_entity_name, ref_active, chapter_id, marker_id)
    let mut stmt = conn.prepare(
        "SELECT m.entity_id, e.name, v.ref_active, m.chapter_id, m.id
         FROM state_marker_values v
         JOIN state_markers m ON m.id = v.marker_id
         JOIN entities e ON e.id = m.entity_id
         WHERE v.value_type = 'entity_ref' AND v.ref_entity_id = ?1
         ORDER BY e.name ASC, m.chapter_id ASC"
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        let active: i64 = row.get(2)?;
        Ok((row.get(0)?, row.get(1)?, active != 0, row.get(3)?, row.get(4)?))
    })?;
    rows.collect()
}

pub fn get_entity_state_keys(conn: &Connection, entity_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT v.fact_key FROM state_marker_values v
         JOIN state_markers m ON m.id = v.marker_id
         WHERE m.entity_id = ?1 AND v.value_type = 'fact' AND v.fact_key != ''
         ORDER BY v.fact_key ASC"
    )?;
    let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
    rows.collect()
}

// ---- Time section ordering ----

#[derive(Serialize, Clone)]
pub struct TimeSectionOrder {
    pub id: i64,
    pub chapter_id: i64,
    pub section_index: i64,
    pub label: String,
    pub story_order: i64,
}

pub fn get_time_section_order(conn: &Connection) -> rusqlite::Result<Vec<TimeSectionOrder>> {
    let mut stmt = conn.prepare(
        "SELECT id, chapter_id, section_index, label, story_order
         FROM time_section_order ORDER BY story_order ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TimeSectionOrder {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            section_index: row.get(2)?,
            label: row.get(3)?,
            story_order: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn save_time_section_order(
    conn: &Connection,
    entries: Vec<(i64, i64, String, i64)>, // (chapter_id, section_index, label, story_order)
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM time_section_order", [])?;
    let mut stmt = conn.prepare(
        "INSERT INTO time_section_order (chapter_id, section_index, label, story_order)
         VALUES (?1, ?2, ?3, ?4)"
    )?;
    for (chapter_id, section_index, label, story_order) in &entries {
        stmt.execute(params![chapter_id, section_index, label, story_order])?;
    }
    Ok(())
}

pub fn reset_time_section_order(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM time_section_order", [])?;
    Ok(())
}

// ---- Story-time resolution ----

#[derive(Serialize, Clone)]
pub struct ResolvedFact {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Clone)]
pub struct ResolvedEntityState {
    pub facts: Vec<ResolvedFact>,
}

/// Resolve entity state at a given point in story time.
/// Walks all markers for this entity in story-time order up to target_story_pos,
/// accumulating key-value pairs (later values overwrite earlier ones for the same key).
pub fn resolve_entity_state(
    conn: &Connection,
    entity_id: i64,
    state_section_map: &std::collections::HashMap<i64, (i64, i64)>,
    story_order_map: &std::collections::HashMap<(i64, i64), i64>,
    target_story_pos: i64,
) -> rusqlite::Result<ResolvedEntityState> {
    let markers = get_entity_markers(conn, entity_id)?;

    // Position each marker in story time
    let mut positioned: Vec<(i64, &StateMarker)> = Vec::new();
    for m in &markers {
        let story_pos = if let Some((ch_id, sec_idx)) = state_section_map.get(&m.id) {
            *story_order_map.get(&(*ch_id, *sec_idx)).unwrap_or(&0)
        } else {
            let sort_order: i64 = conn.query_row(
                "SELECT sort_order FROM chapters WHERE id = ?1",
                params![m.chapter_id],
                |row| row.get(0),
            ).unwrap_or(0);
            sort_order * 1000
        };

        if story_pos <= target_story_pos {
            positioned.push((story_pos, m));
        }
    }

    positioned.sort_by(|a, b| a.0.cmp(&b.0));

    // Accumulate facts — later markers overwrite earlier values for the same key
    let mut fact_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (_, m) in &positioned {
        for v in &m.values {
            fact_map.insert(v.fact_key.clone(), v.fact_value.clone());
        }
    }

    Ok(ResolvedEntityState {
        facts: fact_map.into_iter().map(|(k, v)| ResolvedFact { key: k, value: v }).collect(),
    })
}

// ---- Chapter planning notes (kanban) ----

#[derive(Serialize, Clone)]
pub struct ChapterPlanningNote {
    pub id: i64,
    pub chapter_id: i64,
    pub title: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
}

pub fn get_chapter_planning_notes(conn: &Connection, chapter_id: i64) -> rusqlite::Result<Vec<ChapterPlanningNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, chapter_id, title, description, sort_order, created_at FROM chapter_planning_notes WHERE chapter_id = ?1 ORDER BY sort_order ASC, created_at ASC"
    )?;
    let rows = stmt.query_map(params![chapter_id], |row| {
        Ok(ChapterPlanningNote {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            sort_order: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_all_chapter_planning_notes(conn: &Connection) -> rusqlite::Result<Vec<ChapterPlanningNote>> {
    let mut stmt = conn.prepare(
        "SELECT id, chapter_id, title, description, sort_order, created_at FROM chapter_planning_notes ORDER BY chapter_id ASC, sort_order ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ChapterPlanningNote {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            sort_order: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn add_chapter_planning_note(conn: &Connection, chapter_id: i64, title: &str, description: &str) -> rusqlite::Result<i64> {
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM chapter_planning_notes WHERE chapter_id = ?1",
        params![chapter_id], |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO chapter_planning_notes (chapter_id, title, description, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![chapter_id, title, description, max_order + 1],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_chapter_planning_note(conn: &Connection, id: i64, title: &str, description: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE chapter_planning_notes SET title = ?1, description = ?2 WHERE id = ?3",
        params![title, description, id],
    )?;
    Ok(())
}

pub fn delete_chapter_planning_note(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM chapter_planning_notes WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn reorder_chapter_planning_notes(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE chapter_planning_notes SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}

pub fn move_chapter_planning_note(conn: &Connection, id: i64, new_chapter_id: i64) -> rusqlite::Result<()> {
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM chapter_planning_notes WHERE chapter_id = ?1",
        params![new_chapter_id], |row| row.get(0),
    )?;
    conn.execute(
        "UPDATE chapter_planning_notes SET chapter_id = ?1, sort_order = ?2 WHERE id = ?3",
        params![new_chapter_id, max_order + 1, id],
    )?;
    Ok(())
}

// ---- Kanban freeform board ----

#[derive(Serialize, Clone)]
pub struct KanbanColumn {
    pub id: i64,
    pub title: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct KanbanCard {
    pub id: i64,
    pub column_id: i64,
    pub title: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: String,
}

pub fn get_kanban_columns(conn: &Connection) -> rusqlite::Result<Vec<KanbanColumn>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, sort_order, created_at FROM kanban_columns ORDER BY sort_order ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(KanbanColumn {
            id: row.get(0)?,
            title: row.get(1)?,
            sort_order: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn add_kanban_column(conn: &Connection, title: &str) -> rusqlite::Result<i64> {
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM kanban_columns",
        [], |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO kanban_columns (title, sort_order) VALUES (?1, ?2)",
        params![title, max_order + 1],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_kanban_column(conn: &Connection, id: i64, title: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE kanban_columns SET title = ?1 WHERE id = ?2", params![title, id])?;
    Ok(())
}

pub fn delete_kanban_column(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM kanban_columns WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn reorder_kanban_columns(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE kanban_columns SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}

pub fn get_kanban_cards(conn: &Connection, column_id: i64) -> rusqlite::Result<Vec<KanbanCard>> {
    let mut stmt = conn.prepare(
        "SELECT id, column_id, title, description, sort_order, created_at FROM kanban_cards WHERE column_id = ?1 ORDER BY sort_order ASC"
    )?;
    let rows = stmt.query_map(params![column_id], |row| {
        Ok(KanbanCard {
            id: row.get(0)?,
            column_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            sort_order: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_all_kanban_cards(conn: &Connection) -> rusqlite::Result<Vec<KanbanCard>> {
    let mut stmt = conn.prepare(
        "SELECT id, column_id, title, description, sort_order, created_at FROM kanban_cards ORDER BY column_id ASC, sort_order ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(KanbanCard {
            id: row.get(0)?,
            column_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            sort_order: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn add_kanban_card(conn: &Connection, column_id: i64, title: &str, description: &str) -> rusqlite::Result<i64> {
    let max_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM kanban_cards WHERE column_id = ?1",
        params![column_id], |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO kanban_cards (column_id, title, description, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![column_id, title, description, max_order + 1],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_kanban_card(conn: &Connection, id: i64, title: &str, description: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE kanban_cards SET title = ?1, description = ?2 WHERE id = ?3",
        params![title, description, id],
    )?;
    Ok(())
}

pub fn delete_kanban_card(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM kanban_cards WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn move_kanban_card(conn: &Connection, id: i64, new_column_id: i64, new_sort_order: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE kanban_cards SET column_id = ?1, sort_order = ?2 WHERE id = ?3",
        params![new_column_id, new_sort_order, id],
    )?;
    Ok(())
}

pub fn reorder_kanban_cards(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE kanban_cards SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}
