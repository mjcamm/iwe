use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}

pub fn resolve_by_hash(conn: &Connection, hash: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM image_blobs WHERE hash = ?1",
        params![hash],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

pub fn upload_image(conn: &Connection, bytes: &[u8], mime: &str) -> rusqlite::Result<i64> {
    let hash = hash_bytes(bytes);
    if let Some(id) = resolve_by_hash(conn, &hash)? {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO image_blobs (data, mime, byte_size, hash) VALUES (?1, ?2, ?3, ?4)",
        params![bytes, mime, bytes.len() as i64, hash],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_image_blob(conn: &Connection, id: i64) -> rusqlite::Result<Option<(Vec<u8>, String)>> {
    conn.query_row(
        "SELECT data, mime FROM image_blobs WHERE id = ?1",
        params![id],
        |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

pub fn delete_image_blob(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM image_blobs WHERE id = ?1", params![id])?;
    Ok(())
}

/// Scans known reference sites for image ids that point at image_blobs, returns
/// any blob ids that no column/JSON references. Stretch goal — wired up as a
/// no-op command for now; can power a future "clean up unused images" UI.
pub fn list_orphan_images(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut referenced = std::collections::HashSet::<i64>::new();

    // chapters.chapter_image_id
    {
        let mut stmt = conn.prepare(
            "SELECT chapter_image_id FROM chapters WHERE chapter_image_id IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        for r in rows {
            referenced.insert(r?);
        }
    }

    // project_settings: cover_image_id
    {
        let mut stmt = conn.prepare(
            "SELECT value FROM project_settings WHERE key = 'cover_image_id'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for r in rows {
            if let Ok(id) = r?.parse::<i64>() {
                referenced.insert(id);
            }
        }
    }

    // format_profiles JSON categories — scan for integer image ids under known keys.
    {
        let mut stmt = conn.prepare(
            "SELECT chapter_headings_json, breaks_json FROM format_profiles",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for r in rows {
            let (ch_head, breaks) = r?;
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&ch_head) {
                if let Some(id) = v.get("image_default_id").and_then(|x| x.as_i64()) {
                    referenced.insert(id);
                }
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&breaks) {
                if let Some(id) = v.get("image_id").and_then(|x| x.as_i64()) {
                    referenced.insert(id);
                }
            }
        }
    }

    // format_pages.content — look for image_id (map pages) and ProseMirror image nodes
    // with attrs.imageId (page-content-editor free-form pages).
    {
        let mut stmt = conn.prepare("SELECT content FROM format_pages")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for r in rows {
            let content = r?;
            if content.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                collect_image_ids_recursive(&v, &mut referenced);
            }
        }
    }

    // All blob ids minus referenced ids
    let mut orphans = Vec::new();
    let mut all_stmt = conn.prepare("SELECT id FROM image_blobs")?;
    let all_rows = all_stmt.query_map([], |row| row.get::<_, i64>(0))?;
    for r in all_rows {
        let id = r?;
        if !referenced.contains(&id) {
            orphans.push(id);
        }
    }
    Ok(orphans)
}

fn collect_image_ids_recursive(v: &serde_json::Value, out: &mut std::collections::HashSet<i64>) {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(id) = map.get("imageId").and_then(|x| x.as_i64()) {
                out.insert(id);
            }
            if let Some(id) = map.get("image_id").and_then(|x| x.as_i64()) {
                out.insert(id);
            }
            if let Some(id) = map.get("image_default_id").and_then(|x| x.as_i64()) {
                out.insert(id);
            }
            for (_, child) in map {
                collect_image_ids_recursive(child, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                collect_image_ids_recursive(child, out);
            }
        }
        _ => {}
    }
}

pub fn cleanup_orphan_images(conn: &Connection) -> rusqlite::Result<usize> {
    let orphans = list_orphan_images(conn)?;
    for id in &orphans {
        conn.execute("DELETE FROM image_blobs WHERE id = ?1", params![id])?;
    }
    Ok(orphans.len())
}
