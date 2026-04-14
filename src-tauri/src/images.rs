use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

// ---- Preview generation (downscale) parameters ----
//
// The preview is what the Typst print preview uses. Originals stay full-size
// so PDF export gets the full resolution. 1500px on the longest edge gives
// roughly 187 PPI on a full 8-inch page image and ~375 PPI on a 4-inch
// chapter image — far less than a 4MB JPEG for Typst to decode and embed.

// Preview targets are sized for on-screen viewing of the print/ebook
// preview. A typical chapter image renders ~4 inches wide on a high-DPI
// laptop screen — that's ~700–800 source px even when the user zooms
// generously. 1000px gives comfortable headroom while halving the encoded
// size vs 1500px. Q75 JPEG keeps file size down without visible chroma
// noise at preview zoom.
const PREVIEW_MAX_DIM: u32 = 1000;
const PREVIEW_JPEG_QUALITY: u8 = 75;
/// Skip generating a preview if the source is already this small — the
/// preview wouldn't save anything and we'd just be re-encoding for no gain.
/// Lower threshold = re-encode more inputs (catches small-but-not-tiny
/// images that still benefit from Q75 JPEG re-encoding).
const PREVIEW_MIN_BYTES: usize = 96 * 1024;

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

/// Apply EXIF orientation to the raw bytes if the source carries a
/// non-trivial orientation tag (typical for phone-camera JPEGs). Returns
/// (possibly re-encoded) bytes + mime that, when displayed verbatim, will
/// appear correctly oriented. If decode fails or the orientation is the
/// identity, returns the original bytes unchanged.
///
/// This is run once at upload time so every downstream consumer (browser
/// `<img>`, Typst preview, Typst PDF export) sees pixel-correct bytes
/// without anyone needing to know about EXIF.
fn normalize_orientation(bytes: &[u8], mime: &str) -> (Vec<u8>, String) {
    let passthrough = || (bytes.to_vec(), mime.to_string());

    // Vector / non-photo formats: pass through.
    if mime == "image/svg+xml" || mime == "image/gif" {
        return passthrough();
    }
    use image::ImageDecoder;
    let reader = match image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format() {
        Ok(r) => r,
        Err(_) => return passthrough(),
    };
    let mut decoder = match reader.into_decoder() {
        Ok(d) => d,
        Err(_) => return passthrough(),
    };
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    if matches!(orientation, image::metadata::Orientation::NoTransforms) {
        return passthrough();
    }
    let mut img = match image::DynamicImage::from_decoder(decoder) {
        Ok(i) => i,
        Err(_) => return passthrough(),
    };
    img.apply_orientation(orientation);
    encode_image(&img, mime, 95)
}

/// Generate a preview of the canonical bytes. ALWAYS returns a (bytes, mime)
/// pair — for small/vector/already-tiny images we just clone the originals so
/// the preview column is reliably non-null and downstream code can use a
/// single "fetch preview" path without a fallback branch.
///
/// Pass-through (clone of input) for: SVG (vector), bytes already under
/// PREVIEW_MIN_BYTES, images that are already <= PREVIEW_MAX_DIM on both
/// edges, or any decode failure.
///
/// Otherwise: Triangle-filter downscale to PREVIEW_MAX_DIM and re-encode.
fn generate_preview(bytes: &[u8], mime: &str) -> (Vec<u8>, String) {
    let passthrough = || (bytes.to_vec(), mime.to_string());

    if mime == "image/svg+xml" {
        return passthrough();
    }
    if bytes.len() < PREVIEW_MIN_BYTES {
        return passthrough();
    }
    let img = match image::load_from_memory(bytes) {
        Ok(i) => i,
        Err(e) => {
            log::warn!("[images] generate_preview: decode failed ({}): {} — storing original as preview", mime, e);
            return passthrough();
        }
    };
    use image::GenericImageView;
    let (w, h) = img.dimensions();
    if w <= PREVIEW_MAX_DIM && h <= PREVIEW_MAX_DIM {
        return passthrough();
    }
    // Triangle is ~4× faster than Lanczos3 in pure-Rust and visually
    // indistinguishable at preview sizes (Lanczos shines at print resolution).
    let resized = img.resize(
        PREVIEW_MAX_DIM,
        PREVIEW_MAX_DIM,
        image::imageops::FilterType::Triangle,
    );
    encode_image(&resized, mime, PREVIEW_JPEG_QUALITY)
}

/// Encode a `DynamicImage` to bytes, preserving alpha by staying in PNG
/// when present and otherwise flattening to JPEG at the requested quality.
fn encode_image(img: &image::DynamicImage, source_mime: &str, jpeg_quality: u8) -> (Vec<u8>, String) {
    let has_alpha = matches!(
        img.color(),
        image::ColorType::Rgba8
            | image::ColorType::Rgba16
            | image::ColorType::La8
            | image::ColorType::La16
    );
    if has_alpha {
        let mut out: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut out);
        if img.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
            return (out, "image/png".to_string());
        }
        // Failed PNG encode shouldn't happen — fall back to whatever JPEG can manage.
    }
    let rgb = img.to_rgb8();
    let mut out: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, jpeg_quality);
    match encoder.encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    ) {
        Ok(_) => (out, "image/jpeg".to_string()),
        // Last-resort failure: hand back something with the original mime.
        Err(_) => (Vec::new(), source_mime.to_string()),
    }
}

pub fn upload_image(conn: &Connection, bytes: &[u8], mime: &str) -> rusqlite::Result<i64> {
    let t_total = std::time::Instant::now();
    log::info!(
        "[images] upload_image entered: {:.1}MB ({})",
        bytes.len() as f64 / 1024.0 / 1024.0,
        mime,
    );

    // 1. Normalize orientation so the canonical bytes display correctly
    //    everywhere (browser <img>, Typst, etc.) without anyone having to
    //    know about EXIF tags.
    let t = std::time::Instant::now();
    let (canonical_bytes, canonical_mime) = normalize_orientation(bytes, mime);
    let normalize_ms = t.elapsed().as_secs_f64() * 1000.0;

    // 2. Hash + dedup against the canonical (post-EXIF) bytes — two uploads
    //    of the same photo with different EXIF wrappers should land on the
    //    same row.
    //
    //    If we hit an existing row that's missing its preview blob (because
    //    it was uploaded before preview generation existed, or because the
    //    earlier preview attempt failed), backfill it now. Without this,
    //    re-uploading the same image produces no observable effect — the
    //    user thinks the upload didn't take.
    let hash = hash_bytes(&canonical_bytes);
    if let Some(id) = resolve_by_hash(conn, &hash)? {
        let has_preview: bool = conn
            .query_row(
                "SELECT preview_data IS NOT NULL FROM image_blobs WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n != 0)
            .unwrap_or(false);
        if !has_preview {
            let (preview_bytes, preview_mime) =
                generate_preview(&canonical_bytes, &canonical_mime);
            conn.execute(
                "UPDATE image_blobs SET preview_data = ?1, preview_mime = ?2 WHERE id = ?3",
                params![preview_bytes, preview_mime, id],
            )?;
            log::info!(
                "[images] backfilled preview for existing id={} ({:.1}MB preview)",
                id,
                preview_bytes.len() as f64 / 1024.0 / 1024.0,
            );
        }
        return Ok(id);
    }

    // 3. Generate a preview for the Typst print preview path. Always
    //    returns bytes — for small/vector inputs that don't need
    //    downscaling, the preview is just a copy of the canonical bytes,
    //    so the column is reliably non-null.
    let t = std::time::Instant::now();
    let (preview_bytes, preview_mime) = generate_preview(&canonical_bytes, &canonical_mime);
    let preview_ms = t.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "[images] upload id-pending: orig={:.1}MB ({}) -> canonical={:.1}MB ({}) preview={:.1}MB ({}) | normalize={:.0}ms preview_gen={:.0}ms total={:.0}ms",
        bytes.len() as f64 / 1024.0 / 1024.0,
        mime,
        canonical_bytes.len() as f64 / 1024.0 / 1024.0,
        canonical_mime,
        preview_bytes.len() as f64 / 1024.0 / 1024.0,
        preview_mime,
        normalize_ms,
        preview_ms,
        t_total.elapsed().as_secs_f64() * 1000.0,
    );

    conn.execute(
        "INSERT INTO image_blobs (data, mime, byte_size, hash, preview_data, preview_mime)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            canonical_bytes,
            canonical_mime,
            canonical_bytes.len() as i64,
            hash,
            preview_bytes,
            preview_mime,
        ],
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

/// Get the preview blob (downscaled) for an image. Falls back to the original
/// blob when no preview was stored (small image, vector, decode failure, or
/// pre-migration row). Used by the Typst print preview path.
pub fn get_image_preview_or_original(
    conn: &Connection,
    id: i64,
) -> rusqlite::Result<Option<(Vec<u8>, String)>> {
    conn.query_row(
        "SELECT preview_data, preview_mime, data, mime FROM image_blobs WHERE id = ?1",
        params![id],
        |row| {
            let preview_data: Option<Vec<u8>> = row.get(0)?;
            let preview_mime: Option<String> = row.get(1)?;
            match (preview_data, preview_mime) {
                (Some(d), Some(m)) => Ok((d, m)),
                _ => Ok((row.get::<_, Vec<u8>>(2)?, row.get::<_, String>(3)?)),
            }
        },
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
