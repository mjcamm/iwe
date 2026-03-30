use rusqlite::{params, Connection};
use serde::Serialize;
use std::sync::Mutex;

pub struct PaletteState {
    pub db: Mutex<Connection>,
}

// ---- Structs ----

#[derive(Serialize, Clone)]
pub struct Palette {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub is_active: bool,
    pub sort_order: i64,
    pub group_count: i64,
    pub entry_count: i64,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct WordGroup {
    pub id: i64,
    pub palette_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub entry_count: i64,
}

#[derive(Serialize, Clone)]
pub struct WordSection {
    pub id: i64,
    pub group_id: i64,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Serialize, Clone)]
pub struct WordEntry {
    pub id: i64,
    pub group_id: i64,
    pub section_id: Option<i64>,
    pub word: String,
    pub sort_order: i64,
}

#[derive(Serialize)]
pub struct WordGroupDetail {
    pub group: WordGroup,
    pub sections: Vec<WordSection>,
    pub entries: Vec<WordEntry>,
}

#[derive(Serialize)]
pub struct PaletteDetail {
    pub palette: Palette,
    pub groups: Vec<WordGroup>,
}

// ---- Init & Schema ----

pub fn init_palette_db(path: &str) -> rusqlite::Result<Connection> {
    let is_new = !std::path::Path::new(path).exists();
    let conn = Connection::open(path)?;

    conn.execute_batch("
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS palettes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            is_system INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS word_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            palette_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (palette_id) REFERENCES palettes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS word_sections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (group_id) REFERENCES word_groups(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS word_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER NOT NULL,
            section_id INTEGER,
            word TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (group_id) REFERENCES word_groups(id) ON DELETE CASCADE,
            FOREIGN KEY (section_id) REFERENCES word_sections(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_word_groups_palette ON word_groups(palette_id);
        CREATE INDEX IF NOT EXISTS idx_word_entries_group ON word_entries(group_id);
        CREATE INDEX IF NOT EXISTS idx_word_entries_section ON word_entries(section_id);
        CREATE INDEX IF NOT EXISTS idx_word_sections_group ON word_sections(group_id);
    ")?;

    if is_new {
        seed_starter_data(&conn)?;
    }

    Ok(conn)
}

// ---- Seed Data ----

fn seed_starter_data(conn: &Connection) -> rusqlite::Result<()> {
    // Create the system default palette
    conn.execute(
        "INSERT INTO palettes (name, description, is_system, is_active, sort_order) VALUES (?1, ?2, 1, 1, 0)",
        params!["Emotion Thesaurus", "A writer's reference for emotion beats, physical signals, and sensory words"],
    )?;
    let palette_id = conn.last_insert_rowid();

    let groups: Vec<(&str, &str, Vec<(&str, Vec<&str>)>)> = vec![
        ("Anger", "An intense emotional response to perceived injustice, frustration, or threat", vec![
            ("Physical Signals", vec!["clenched jaw", "flared nostrils", "rigid posture", "pounding fists", "grinding teeth", "speaking through clenched teeth", "slamming doors", "aggressive gestures", "pacing", "invading personal space"]),
            ("Internal Sensations", vec!["heat flooding chest", "racing pulse", "pressure behind eyes", "muscles tightening", "blood pounding in ears", "shaking hands", "surge of adrenaline"]),
            ("Mental Responses", vec!["tunnel vision", "inability to focus", "urge to lash out", "replaying the offense", "racing thoughts", "desire for confrontation", "snap judgments"]),
            ("Suppression Cues", vec!["forced smile", "measured breathing", "white-knuckled grip under table", "changing the subject", "counting silently", "excusing oneself", "overly controlled voice"]),
        ]),
        ("Joy", "A feeling of great pleasure, happiness, or elation", vec![
            ("Physical Signals", vec!["beaming", "bright eyes", "laughing freely", "light step", "flushed cheeks", "open posture", "bouncing", "dancing", "hugging", "clapping hands"]),
            ("Internal Sensations", vec!["warmth spreading through chest", "heart swelling", "lightness", "tingling", "buzzing energy", "breathlessness"]),
            ("Mental Responses", vec!["optimistic thinking", "gratitude", "sense of invincibility", "desire to share", "heightened creativity", "time flying"]),
            ("Suppression Cues", vec!["biting lip to hide smile", "looking away", "downplaying good news", "measured tone", "fidgeting with contained energy"]),
        ]),
        ("Sadness", "A deep emotional pain associated with loss, disappointment, or helplessness", vec![
            ("Physical Signals", vec!["slumped shoulders", "downcast eyes", "slow movements", "trembling lip", "wiping eyes", "hugging oneself", "staring blankly", "sighing", "withdrawing physically"]),
            ("Internal Sensations", vec!["hollow chest", "heaviness in limbs", "lump in throat", "aching", "numbness", "fatigue", "tightness behind eyes"]),
            ("Mental Responses", vec!["replaying memories", "self-blame", "feeling of emptiness", "questioning purpose", "withdrawal from plans", "difficulty concentrating"]),
            ("Suppression Cues", vec!["forced cheerfulness", "staying busy", "avoiding eye contact", "changing the subject", "isolating when alone"]),
        ]),
        ("Fear", "An emotional response to perceived danger or threat, real or imagined", vec![
            ("Physical Signals", vec!["wide eyes", "trembling", "backing away", "flinching", "frozen stance", "pale face", "rapid shallow breathing", "sweating", "scanning surroundings", "gripping nearby objects"]),
            ("Internal Sensations", vec!["stomach dropping", "cold spreading through body", "heart hammering", "throat tightening", "legs turning to jelly", "hair standing on end", "nausea"]),
            ("Mental Responses", vec!["catastrophic thinking", "hyper-awareness", "tunnel vision", "urge to flee", "inability to think clearly", "time slowing", "calculating escape routes"]),
            ("Suppression Cues", vec!["forced calm", "overly still posture", "controlled breathing", "gripping hands together", "rationalizing aloud", "nervous laughter"]),
        ]),
        ("Disgust", "A strong aversion or revulsion toward something offensive", vec![
            ("Physical Signals", vec!["lip curling", "wrinkling nose", "turning away", "recoiling", "gagging", "shuddering", "covering mouth", "stepping back", "grimacing", "pushing away"]),
            ("Internal Sensations", vec!["stomach churning", "bile rising", "skin crawling", "taste in back of throat", "wave of nausea", "prickling skin"]),
            ("Mental Responses", vec!["desire to distance", "judgment", "contamination anxiety", "fixating on the source", "urge to cleanse"]),
            ("Suppression Cues", vec!["swallowing hard", "neutral expression", "breathing through mouth", "looking elsewhere", "polite deflection"]),
        ]),
        ("Surprise", "A sudden, unexpected emotional reaction to something unanticipated", vec![
            ("Physical Signals", vec!["gasping", "jaw dropping", "eyebrows shooting up", "stumbling back", "hand to chest", "blinking rapidly", "freezing mid-action", "double-taking", "speechless"]),
            ("Internal Sensations", vec!["breath catching", "jolt of adrenaline", "heart skipping", "momentary blankness", "rush of blood to head"]),
            ("Mental Responses", vec!["disorientation", "rapid reassessment", "disbelief", "scrambling to make sense", "replaying the moment"]),
            ("Suppression Cues", vec!["quick recovery to neutral", "controlled exhale", "playing it cool", "delayed reaction", "understated acknowledgment"]),
        ]),
    ];

    let flat_groups: Vec<(&str, Vec<&str>)> = vec![
        ("Movement", vec!["lunging", "staggering", "prowling", "skulking", "darting", "ambling", "trudging", "sprinting", "shuffling", "gliding", "stumbling", "charging", "creeping", "bolting", "limping", "swaggering", "weaving", "vaulting", "scrambling", "reeling"]),
        ("Sensory — Sight", vec!["glinting", "shadowy", "hazy", "vivid", "blinding", "flickering", "muted", "gleaming", "stark", "translucent", "murky", "luminous", "dappled", "piercing", "washed-out", "incandescent"]),
        ("Sensory — Sound", vec!["thundering", "whispering", "crackling", "droning", "piercing", "muffled", "echoing", "hissing", "rumbling", "tinkling", "rasping", "humming", "keening", "grating", "lilting", "reverberating"]),
        ("Sensory — Touch", vec!["gritty", "silken", "coarse", "slick", "porous", "velvety", "scorching", "clammy", "prickly", "smooth", "rough", "yielding", "bristly", "feathery", "jagged", "tepid"]),
    ];

    let mut sort = 0i64;
    for (name, desc, sections) in &groups {
        conn.execute("INSERT INTO word_groups (palette_id, name, description, sort_order) VALUES (?1, ?2, ?3, ?4)", params![palette_id, name, desc, sort])?;
        let group_id = conn.last_insert_rowid();
        sort += 1;
        let mut sec_sort = 0i64;
        for (sec_name, words) in sections {
            conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![group_id, sec_name, sec_sort])?;
            let section_id = conn.last_insert_rowid();
            sec_sort += 1;
            for (i, word) in words.iter().enumerate() {
                conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![group_id, section_id, word, i as i64])?;
            }
        }
    }
    for (name, words) in &flat_groups {
        conn.execute("INSERT INTO word_groups (palette_id, name, sort_order) VALUES (?1, ?2, ?3)", params![palette_id, name, sort])?;
        let group_id = conn.last_insert_rowid();
        sort += 1;
        for (i, word) in words.iter().enumerate() {
            conn.execute("INSERT INTO word_entries (group_id, word, sort_order) VALUES (?1, ?2, ?3)", params![group_id, word, i as i64])?;
        }
    }

    Ok(())
}

// ---- Query helpers ----

fn read_palette(row: &rusqlite::Row) -> rusqlite::Result<Palette> {
    Ok(Palette {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        is_system: row.get::<_, i64>(3)? != 0,
        is_active: row.get::<_, i64>(4)? != 0,
        sort_order: row.get(5)?,
        group_count: row.get(6)?,
        entry_count: row.get(7)?,
        created_at: row.get(8)?,
    })
}

const PALETTE_SELECT: &str = "
    SELECT p.id, p.name, p.description, p.is_system, p.is_active, p.sort_order,
           (SELECT COUNT(*) FROM word_groups WHERE palette_id = p.id) as group_count,
           (SELECT COUNT(*) FROM word_entries e JOIN word_groups g ON e.group_id = g.id WHERE g.palette_id = p.id) as entry_count,
           p.created_at
    FROM palettes p";

fn read_group(row: &rusqlite::Row) -> rusqlite::Result<WordGroup> {
    Ok(WordGroup {
        id: row.get(0)?,
        palette_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        sort_order: row.get(4)?,
        entry_count: row.get(5)?,
    })
}

const GROUP_SELECT: &str = "
    SELECT g.id, g.palette_id, g.name, g.description, g.sort_order,
           (SELECT COUNT(*) FROM word_entries WHERE group_id = g.id) as entry_count
    FROM word_groups g";

// ---- Palette Commands ----

#[tauri::command]
pub fn get_palettes(state: tauri::State<'_, PaletteState>) -> Result<Vec<Palette>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!("{} ORDER BY p.sort_order ASC", PALETTE_SELECT)).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| read_palette(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_palette(state: tauri::State<'_, PaletteState>, name: String, description: Option<String>) -> Result<Palette, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM palettes", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO palettes (name, description, sort_order) VALUES (?1, ?2, ?3)", params![name, description, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(&format!("{} WHERE p.id = ?1", PALETTE_SELECT), params![id], |row| read_palette(row)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_palette(state: tauri::State<'_, PaletteState>, id: i64, name: String, description: Option<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE palettes SET name = ?1, description = ?2 WHERE id = ?3 AND is_system = 0", params![name, description, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_palette(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM palettes WHERE id = ?1 AND is_system = 0", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_palette(state: tauri::State<'_, PaletteState>, id: i64, active: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE palettes SET is_active = ?1 WHERE id = ?2", params![active as i64, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn copy_palette(state: tauri::State<'_, PaletteState>, id: i64, new_name: String) -> Result<Palette, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Get source palette
    let (desc,): (Option<String>,) = conn.query_row(
        "SELECT description FROM palettes WHERE id = ?1", params![id],
        |row| Ok((row.get(0)?,))
    ).map_err(|e| e.to_string())?;

    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM palettes", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO palettes (name, description, is_system, is_active, sort_order) VALUES (?1, ?2, 0, 1, ?3)", params![new_name, desc, max_sort + 1]).map_err(|e| e.to_string())?;
    let new_palette_id = conn.last_insert_rowid();

    // Copy groups
    let mut group_ids: Vec<(i64, String, Option<String>, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, name, description, sort_order FROM word_groups WHERE palette_id = ?1 ORDER BY sort_order").map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            group_ids.push((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap(), row.get(3).unwrap()));
        }
    }

    for (old_group_id, gname, gdesc, gsort) in &group_ids {
        conn.execute("INSERT INTO word_groups (palette_id, name, description, sort_order) VALUES (?1, ?2, ?3, ?4)", params![new_palette_id, gname, gdesc, gsort]).map_err(|e| e.to_string())?;
        let new_group_id = conn.last_insert_rowid();

        // Copy sections
        let mut old_sections: Vec<(i64, String, i64)> = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT id, name, sort_order FROM word_sections WHERE group_id = ?1 ORDER BY sort_order").map_err(|e| e.to_string())?;
            let mut rows = stmt.query(params![old_group_id]).map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                old_sections.push((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()));
            }
        }

        let mut section_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for (old_sec_id, sname, ssort) in &old_sections {
            conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![new_group_id, sname, ssort]).map_err(|e| e.to_string())?;
            section_map.insert(*old_sec_id, conn.last_insert_rowid());
        }

        // Copy entries
        let mut old_entries: Vec<(Option<i64>, String, i64)> = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT section_id, word, sort_order FROM word_entries WHERE group_id = ?1 ORDER BY sort_order").map_err(|e| e.to_string())?;
            let mut rows = stmt.query(params![old_group_id]).map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                old_entries.push((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()));
            }
        }

        for (old_sec_id, word, wsort) in &old_entries {
            let new_sec_id = old_sec_id.and_then(|sid| section_map.get(&sid).copied());
            conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![new_group_id, new_sec_id, word, wsort]).map_err(|e| e.to_string())?;
        }
    }

    conn.query_row(&format!("{} WHERE p.id = ?1", PALETTE_SELECT), params![new_palette_id], |row| read_palette(row)).map_err(|e| e.to_string())
}

// ---- Group Commands ----

#[tauri::command]
pub fn get_word_groups(state: tauri::State<'_, PaletteState>, palette_id: i64) -> Result<Vec<WordGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!("{} WHERE g.palette_id = ?1 ORDER BY g.sort_order ASC", GROUP_SELECT)).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![palette_id], |row| read_group(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_word_group(state: tauri::State<'_, PaletteState>, id: i64) -> Result<WordGroupDetail, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let group = conn.query_row(&format!("{} WHERE g.id = ?1", GROUP_SELECT), params![id], |row| read_group(row)).map_err(|e| e.to_string())?;

    let mut sec_stmt = conn.prepare("SELECT id, group_id, name, sort_order FROM word_sections WHERE group_id = ?1 ORDER BY sort_order ASC").map_err(|e| e.to_string())?;
    let sections = sec_stmt.query_map(params![id], |row| Ok(WordSection { id: row.get(0)?, group_id: row.get(1)?, name: row.get(2)?, sort_order: row.get(3)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    let mut entry_stmt = conn.prepare("SELECT id, group_id, section_id, word, sort_order FROM word_entries WHERE group_id = ?1 ORDER BY sort_order ASC").map_err(|e| e.to_string())?;
    let entries = entry_stmt.query_map(params![id], |row| Ok(WordEntry { id: row.get(0)?, group_id: row.get(1)?, section_id: row.get(2)?, word: row.get(3)?, sort_order: row.get(4)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    Ok(WordGroupDetail { group, sections, entries })
}

#[tauri::command]
pub fn create_word_group(state: tauri::State<'_, PaletteState>, palette_id: i64, name: String, description: Option<String>) -> Result<WordGroup, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    // Block edits to system palettes
    let is_sys: i64 = conn.query_row("SELECT is_system FROM palettes WHERE id = ?1", params![palette_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if is_sys != 0 { return Err("Cannot modify system palette".to_string()); }

    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_groups WHERE palette_id = ?1", params![palette_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO word_groups (palette_id, name, description, sort_order) VALUES (?1, ?2, ?3, ?4)", params![palette_id, name, description, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(&format!("{} WHERE g.id = ?1", GROUP_SELECT), params![id], |row| read_group(row)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_word_group(state: tauri::State<'_, PaletteState>, id: i64, name: String, description: Option<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE word_groups SET name = ?1, description = ?2 WHERE id = ?3", params![name, description, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_word_group(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM word_groups WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Section Commands ----

#[tauri::command]
pub fn get_all_section_names(state: tauri::State<'_, PaletteState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT DISTINCT name FROM word_sections ORDER BY name ASC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<String>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_section(state: tauri::State<'_, PaletteState>, group_id: i64, name: String) -> Result<WordSection, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_sections WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![group_id, name, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(WordSection { id, group_id, name, sort_order: max_sort + 1 })
}

#[tauri::command]
pub fn add_all_sections(state: tauri::State<'_, PaletteState>, group_id: i64, names: Vec<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut existing_stmt = conn.prepare("SELECT name FROM word_sections WHERE group_id = ?1").map_err(|e| e.to_string())?;
    let existing: std::collections::HashSet<String> = existing_stmt.query_map(params![group_id], |row| row.get(0))
        .map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_sections WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut sort = max_sort + 1;
    for name in &names {
        if !existing.contains(name) {
            conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![group_id, name, sort]).map_err(|e| e.to_string())?;
            sort += 1;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_section(state: tauri::State<'_, PaletteState>, id: i64, name: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE word_sections SET name = ?1 WHERE id = ?2", params![name, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_section(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE word_entries SET section_id = NULL WHERE section_id = ?1", params![id]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM word_sections WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Entry Commands ----

#[tauri::command]
pub fn add_word_entry(state: tauri::State<'_, PaletteState>, group_id: i64, section_id: Option<i64>, word: String) -> Result<WordEntry, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_entries WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![group_id, section_id, word, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(WordEntry { id, group_id, section_id, word, sort_order: max_sort + 1 })
}

#[tauri::command]
pub fn add_word_entries(state: tauri::State<'_, PaletteState>, group_id: i64, section_id: Option<i64>, words: Vec<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut existing = std::collections::HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT LOWER(word) FROM word_entries WHERE group_id = ?1 AND (?2 IS NULL AND section_id IS NULL OR section_id = ?2)").map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![group_id, section_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
        for row in rows { if let Ok(w) = row { existing.insert(w); } }
    }
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_entries WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut sort = max_sort + 1;
    for word in &words {
        let trimmed = word.trim();
        if !trimmed.is_empty() && !existing.contains(&trimmed.to_lowercase()) {
            conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![group_id, section_id, trimmed, sort]).map_err(|e| e.to_string())?;
            sort += 1;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn remove_word_entry(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM word_entries WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Search (across active palettes) ----

#[tauri::command]
pub fn search_word_groups(state: tauri::State<'_, PaletteState>, query: String) -> Result<Vec<WordGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(&format!(
        "{} JOIN palettes p2 ON g.palette_id = p2.id WHERE p2.is_active = 1 AND LOWER(g.name) LIKE ?1 ORDER BY g.sort_order ASC",
        GROUP_SELECT
    )).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![pattern], |row| read_group(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Get all groups from all active palettes (for the editor picker)
#[tauri::command]
pub fn get_active_groups(state: tauri::State<'_, PaletteState>) -> Result<Vec<WordGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!(
        "{} JOIN palettes p2 ON g.palette_id = p2.id WHERE p2.is_active = 1 ORDER BY g.name ASC",
        GROUP_SELECT
    )).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| read_group(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
