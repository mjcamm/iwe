use crate::db::{self, AppState};
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ReferenceType, ZipLibrary};
use image::codecs::jpeg::JpegEncoder;
use image::{GenericImageView, ImageFormat};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

#[derive(Deserialize)]
pub struct EpubChapter {
    pub title: String,
    pub subtitle: String,
    pub html: String,
}

#[derive(Deserialize)]
pub struct EpubPage {
    pub title: String,
    pub role: String,
    pub html: String,
    pub position: String,
}

#[derive(Deserialize)]
pub struct EpubExportRequest {
    pub title: String,
    pub author: String,
    pub language: String,
    pub description: String,
    pub chapters: Vec<EpubChapter>,
    pub front_pages: Vec<EpubPage>,
    pub back_pages: Vec<EpubPage>,
    pub css: String,
    /// Image compression level: "none", "balanced", or "compact".
    /// Defaults to "none" if missing or unknown.
    #[serde(default)]
    pub compression_level: String,
}

/// Wrap chapter/page HTML in a valid XHTML document.
///
/// The body content is additionally wrapped in `<div class="ebook-body">`
/// so the exact same CSS selectors the in-app ebook preview uses (which
/// all prefix `.ebook-body`) apply to the exported XHTML too. Without this,
/// we'd have to maintain two parallel CSS builders — one prefixed and one
/// rooted at `body` — which is how we ended up with the hardcoded serif
/// block that ignored profile settings in the first place.
fn wrap_xhtml(title: &str, css: &str, body_html: &str) -> String {
    let _ = css; // CSS is attached via the separate stylesheet.css file
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head>
  <meta charset="UTF-8" />
  <title>{}</title>
  <link rel="stylesheet" type="text/css" href="stylesheet.css" />
</head>
<body>
<div class="ebook-body">
{}
</div>
</body>
</html>"#,
        html_escape(title),
        body_html
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---- Inline image extraction ----
//
// Chapter and page HTML coming from the frontend contains `<img>` tags
// with `iwe-image://{id}` src attributes that point at rows in the
// `image_blobs` table. For EPUB output we pull each referenced BLOB
// into its own zip entry under `images/img-{id}.{ext}` and rewrite the
// src to a relative path. Identical ids across chapters dedupe naturally
// so we never ship the same bytes twice.

/// One extracted image, ready to be added as an EPUB resource.
struct ExtractedImage {
    path: String,     // relative path inside OEBPS, e.g. "images/img-42.png"
    bytes: Vec<u8>,
    mime: String,
}

/// Pick a filename extension for a given mime type. Falls back to "bin"
/// for unknown types rather than guessing.
fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

/// Compression parameters for a given preset.
struct CompressionParams {
    /// Longest-edge pixel cap. Images larger than this are downscaled
    /// with a Lanczos3 filter before encoding.
    max_dim: u32,
    /// JPEG quality 0-100. Ignored for PNG outputs (we keep alpha intact
    /// by re-saving PNGs as PNG rather than flattening to JPEG).
    jpeg_quality: u8,
}

fn params_for_level(level: &str) -> Option<CompressionParams> {
    match level {
        "minor"    => Some(CompressionParams { max_dim: 2500, jpeg_quality: 90 }),
        "balanced" => Some(CompressionParams { max_dim: 2000, jpeg_quality: 85 }),
        "compact"  => Some(CompressionParams { max_dim: 1500, jpeg_quality: 70 }),
        _ => None, // "none" or unknown — skip compression entirely
    }
}

/// Re-encode an image at the target compression level. Returns the new
/// bytes + mime type. If the input can't be decoded (e.g. SVG which is
/// vector and handled separately, or a corrupt image), returns the
/// original bytes unchanged so the export doesn't fail.
///
/// Rules:
///   - SVG, unknown formats, or decode failures → pass through unchanged
///   - Images with alpha (RGBA/LA) → re-save as PNG to preserve transparency
///   - Everything else → re-encode as JPEG at the preset's quality
///   - Images larger than `max_dim` on the longest edge → downscale first
fn compress_image(bytes: &[u8], mime: &str, params: &CompressionParams) -> (Vec<u8>, String) {
    // SVG is vector — never re-encode. Keeps the original bytes.
    if mime == "image/svg+xml" {
        return (bytes.to_vec(), mime.to_string());
    }

    // Decode. On any error, fall back to the original bytes.
    let img = match image::load_from_memory(bytes) {
        Ok(i) => i,
        Err(_) => return (bytes.to_vec(), mime.to_string()),
    };

    // Downscale to fit within max_dim on the longest edge. Uses Lanczos3
    // for best quality on downscales. If the image is already smaller
    // than max_dim, resize() leaves it alone (it only shrinks, never grows).
    let (w, h) = img.dimensions();
    let img = if w > params.max_dim || h > params.max_dim {
        img.resize(params.max_dim, params.max_dim, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Preserve alpha by staying in PNG for images that have it. Flattening
    // a PNG with transparency to JPEG would give an ugly black background.
    let color = img.color();
    let has_alpha = matches!(
        color,
        image::ColorType::Rgba8
            | image::ColorType::Rgba16
            | image::ColorType::La8
            | image::ColorType::La16
    );

    if has_alpha {
        // Re-save as PNG (default encoder). Preserves alpha. Note: the image
        // crate's default PNG encoder is decent but not as small as a dedicated
        // optimizer like oxipng — most savings here come from the downscale.
        let mut out: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut out);
        match img.write_to(&mut cursor, ImageFormat::Png) {
            Ok(_) => (out, "image/png".to_string()),
            Err(_) => (bytes.to_vec(), mime.to_string()),
        }
    } else {
        // Flatten to RGB and encode as JPEG at the target quality.
        let rgb = img.to_rgb8();
        let mut out: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut out);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, params.jpeg_quality);
        match encoder.encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        ) {
            Ok(_) => (out, "image/jpeg".to_string()),
            Err(_) => (bytes.to_vec(), mime.to_string()),
        }
    }
}

/// Scan an HTML string for `<img src="iwe-image://{id}">` patterns (or their
/// `http://iwe-image.localhost/{id}` canonicalized form), pull each unique
/// BLOB out of the `image_blobs` table, add it to the accumulator, and return
/// a rewritten HTML string where the URI has been replaced with the relative
/// path to the extracted file inside the EPUB zip.
///
/// `accumulator` is shared across all chapters/pages in the export so a
/// single image used in multiple places only gets added to the zip once.
/// The map key is the image_blobs id itself — IDs are already unique and
/// stable, so no extra hashing is needed for dedup.
///
/// If `compress_params` is Some, every extracted image is re-encoded at
/// the given preset. The path extension reflects the POST-compression
/// format (e.g. a PNG flattened to JPEG lands as `.jpg`).
fn extract_inline_images(
    html: &str,
    accumulator: &mut HashMap<u64, ExtractedImage>,
    compress_params: Option<&CompressionParams>,
    conn: &rusqlite::Connection,
) -> String {
    // Match either `iwe-image://{id}` or `http://iwe-image.localhost/{id}`
    // inside an <img src="..."> attribute. The id is the only capture group
    // we need; src-attribute structure is captured for rewriting.
    let re = match Regex::new(
        r#"(?i)(<img[^>]*?\ssrc=")(?:iwe-image://|https?://iwe-image\.localhost/)(\d+)(")"#,
    ) {
        Ok(r) => r,
        Err(_) => return html.to_string(),
    };

    re.replace_all(html, |caps: &regex::Captures| -> String {
        let prefix = &caps[1];
        let id_str = &caps[2];
        let suffix = &caps[3];

        let id: i64 = match id_str.parse() {
            Ok(n) => n,
            Err(_) => return caps.get(0).unwrap().as_str().to_string(),
        };

        // Use the blob id as the dedup key so the same image referenced
        // from multiple pages only gets decoded/compressed once.
        let key = id as u64;
        let path = if let Some(existing) = accumulator.get(&key) {
            existing.path.clone()
        } else {
            match crate::images::get_image_blob(conn, id) {
                Ok(Some((bytes, mime))) => {
                    let (final_bytes, final_mime) = match compress_params {
                        Some(params) => compress_image(&bytes, &mime, params),
                        None => (bytes, mime),
                    };
                    let ext = ext_for_mime(&final_mime);
                    let path = format!("images/img-{}.{}", id, ext);
                    accumulator.insert(
                        key,
                        ExtractedImage {
                            path: path.clone(),
                            bytes: final_bytes,
                            mime: final_mime,
                        },
                    );
                    path
                }
                // Unknown id or DB error — leave the src as-is so the failure
                // is visible in the reader (broken image icon) rather than
                // silently producing a zip with a missing resource.
                _ => return caps.get(0).unwrap().as_str().to_string(),
            }
        };

        format!("{}{}{}", prefix, path, suffix)
    })
    .into_owned()
}

/// Map page roles to epub:type semantic annotations.
fn epub_type_for_role(role: &str) -> &'static str {
    match role {
        "title" => "titlepage",
        "copyright" => "copyright-page",
        "dedication" => "dedication",
        "epigraph" => "epigraph",
        "toc" => "toc",
        "foreword" => "foreword",
        "preface" => "preface",
        "prologue" => "prologue",
        "epilogue" => "epilogue",
        "afterword" => "afterword",
        "acknowledgments" => "acknowledgments",
        "glossary" => "glossary",
        "bibliography" => "bibliography",
        _ => "bodymatter",
    }
}

/// Build chapter body HTML. Only emits the outer `<section>` wrapper —
/// the JS side is responsible for rendering the chapter heading (number /
/// title / subtitle / image / rules) via `renderChapterHeadingHtml`, which
/// honors the profile's chapter_headings_json settings. Adding a redundant
/// `<h1>` here would duplicate the heading and ignore the user's styling.
///
/// `body` is the pre-rewritten HTML (inline images already extracted).
fn build_chapter_html(_chapter: &EpubChapter, index: usize, body: &str) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        "<section epub:type=\"chapter\" id=\"chapter-{}\">\n",
        index + 1
    ));
    html.push_str(body);
    html.push_str("\n</section>");
    html
}

/// Build front/back matter page HTML. Pages still get their title rendered
/// as an `<h1>` — there's no per-page "headings" settings panel yet, so the
/// simple generic rendering is fine here.
///
/// `body` is the pre-rewritten HTML (inline images already extracted).
fn build_page_html(page: &EpubPage, body: &str) -> String {
    let epub_type = epub_type_for_role(&page.role);
    let mut html = String::new();
    html.push_str(&format!(
        "<section epub:type=\"{}\" id=\"page-{}\">\n",
        epub_type,
        page.role
    ));
    if !page.title.is_empty() && page.role != "toc" {
        html.push_str(&format!("<h1>{}</h1>\n", html_escape(&page.title)));
    }
    html.push_str(body);
    html.push_str("\n</section>");
    html
}

#[tauri::command]
pub fn export_epub(
    state: tauri::State<'_, AppState>,
    request: EpubExportRequest,
) -> Result<Vec<u8>, String> {
    use std::time::Instant;
    let t_start = Instant::now();

    // Hold the DB lock for the whole export. We need it for the cover
    // lookup and then again for every image_blobs fetch inside
    // extract_inline_images — grabbing it once avoids lock churn and keeps
    // the code simpler.
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    // Cover: read cover_image_id from project_settings, fetch the blob.
    let cover: Option<(Vec<u8>, String)> = (|| -> Option<(Vec<u8>, String)> {
        let id_str = db::get_project_setting(conn, "cover_image_id").ok().flatten()?;
        let id: i64 = id_str.parse().ok()?;
        crate::images::get_image_blob(conn, id).ok().flatten()
    })();
    let t_cover_load = t_start.elapsed();

    let mut builder = EpubBuilder::new(ZipLibrary::new().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    // NOTE: order matters. epub-builder 0.8 has a bug where if `lang` is set
    // after `author`, the generated `<dc:language>` element inherits the
    // creator's `epub-creator-0` id, producing a duplicate-id validation
    // failure (epubcheck RSC-005). Setting lang BEFORE author avoids it —
    // lang gets no id, author gets the fresh id for its `role` refines.
    let lang = if request.language.is_empty() { "en" } else { request.language.as_str() };
    builder
        .epub_version(EpubVersion::V30)
        .metadata("title", &request.title)
        .map_err(|e| e.to_string())?
        .metadata("lang", lang)
        .map_err(|e| e.to_string())?
        .metadata("author", &request.author)
        .map_err(|e| e.to_string())?;

    if !request.description.is_empty() {
        builder
            .metadata("description", &request.description)
            .map_err(|e| e.to_string())?;
    }

    // Add stylesheet
    builder
        .stylesheet(request.css.as_bytes())
        .map_err(|e| e.to_string())?;

    // Resolve the compression preset once — passed to both cover and
    // inline image handling.
    let compress_params = params_for_level(&request.compression_level);

    // Cover image (from the book_cover BLOB in the current project DB).
    // Cover is also run through compression when a preset is selected.
    if let Some((ref bytes, ref mime)) = cover {
        let (cover_bytes, cover_mime) = match compress_params.as_ref() {
            Some(params) => compress_image(bytes, mime, params),
            None => (bytes.clone(), mime.clone()),
        };
        let ext = ext_for_mime(&cover_mime);
        builder
            .add_cover_image(format!("cover.{}", ext), &cover_bytes[..], cover_mime.as_str())
            .map_err(|e| e.to_string())?;
    }

    let t_setup = t_start.elapsed();

    let mut image_accumulator: HashMap<u64, ExtractedImage> = HashMap::new();
    let front_rewritten: Vec<String> = request
        .front_pages
        .iter()
        .map(|p| extract_inline_images(&p.html, &mut image_accumulator, compress_params.as_ref(), conn))
        .collect();
    let t_front_images = t_start.elapsed();

    let chapter_rewritten: Vec<String> = request
        .chapters
        .iter()
        .map(|c| extract_inline_images(&c.html, &mut image_accumulator, compress_params.as_ref(), conn))
        .collect();
    let t_chapter_images = t_start.elapsed();

    let back_rewritten: Vec<String> = request
        .back_pages
        .iter()
        .map(|p| extract_inline_images(&p.html, &mut image_accumulator, compress_params.as_ref(), conn))
        .collect();
    let t_back_images = t_start.elapsed();

    let total_image_bytes: usize = image_accumulator.values().map(|img| img.bytes.len()).sum();
    let image_count = image_accumulator.len();

    for img in image_accumulator.values() {
        builder
            .add_resource(img.path.as_str(), img.bytes.as_slice(), img.mime.as_str())
            .map_err(|e| e.to_string())?;
    }
    let t_add_resources = t_start.elapsed();

    // Front matter pages
    for (i, page) in request.front_pages.iter().enumerate() {
        let body = build_page_html(page, &front_rewritten[i]);
        let xhtml = wrap_xhtml(&page.title, &request.css, &body);
        let filename = format!("front_{:02}.xhtml", i);

        let ref_type = match page.role.as_str() {
            "toc" => ReferenceType::Toc,
            "title" => ReferenceType::TitlePage,
            _ => ReferenceType::Text,
        };

        builder
            .add_content(
                EpubContent::new(&filename, xhtml.as_bytes())
                    .title(&page.title)
                    .reftype(ref_type),
            )
            .map_err(|e| e.to_string())?;
    }

    // Chapters
    for (i, chapter) in request.chapters.iter().enumerate() {
        let body = build_chapter_html(chapter, i, &chapter_rewritten[i]);
        let xhtml = wrap_xhtml(&chapter.title, &request.css, &body);
        let filename = format!("chapter_{:02}.xhtml", i);

        builder
            .add_content(
                EpubContent::new(&filename, xhtml.as_bytes())
                    .title(&chapter.title)
                    .reftype(ReferenceType::Text),
            )
            .map_err(|e| e.to_string())?;
    }

    // Back matter pages
    for (i, page) in request.back_pages.iter().enumerate() {
        let body = build_page_html(page, &back_rewritten[i]);
        let xhtml = wrap_xhtml(&page.title, &request.css, &body);
        let filename = format!("back_{:02}.xhtml", i);

        builder
            .add_content(
                EpubContent::new(&filename, xhtml.as_bytes())
                    .title(&page.title)
                    .reftype(ReferenceType::Text),
            )
            .map_err(|e| e.to_string())?;
    }

    let t_add_content = t_start.elapsed();

    // Generate EPUB
    let mut output: Vec<u8> = Vec::new();
    builder.generate(&mut output).map_err(|e| e.to_string())?;
    let t_generate = t_start.elapsed();

    let result = fix_opf_ids(output);
    let t_total = t_start.elapsed();

    log::info!(
        "[epub] timing: cover_load={:.0}ms setup={:.0}ms front_images={:.0}ms chapter_images={:.0}ms back_images={:.0}ms add_resources={:.0}ms add_content={:.0}ms generate={:.0}ms fix_opf={:.0}ms total={:.0}ms | {} images ({:.1}MB)",
        t_cover_load.as_secs_f64() * 1000.0,
        (t_setup - t_cover_load).as_secs_f64() * 1000.0,
        (t_front_images - t_setup).as_secs_f64() * 1000.0,
        (t_chapter_images - t_front_images).as_secs_f64() * 1000.0,
        (t_back_images - t_chapter_images).as_secs_f64() * 1000.0,
        (t_add_resources - t_back_images).as_secs_f64() * 1000.0,
        (t_add_content - t_add_resources).as_secs_f64() * 1000.0,
        (t_generate - t_add_content).as_secs_f64() * 1000.0,
        (t_total - t_generate).as_secs_f64() * 1000.0,
        t_total.as_secs_f64() * 1000.0,
        image_count,
        total_image_bytes as f64 / 1024.0 / 1024.0,
    );

    result
}

/// Unzip the generated EPUB in memory, patch `OEBPS/content.opf` to remove
/// the buggy `id="epub-creator-0"` attribute from `<dc:language>`, and rezip.
///
/// Preserves:
///   - file order (mimetype MUST stay first per EPUB spec)
///   - per-entry compression method (mimetype MUST remain STORED/uncompressed)
///   - all other file contents byte-for-byte
fn fix_opf_ids(epub_bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let reader = Cursor::new(&epub_bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("zip read: {}", e))?;

    let mut out: Vec<u8> = Vec::with_capacity(epub_bytes.len());
    {
        let writer_cursor = Cursor::new(&mut out);
        let mut writer = zip::ZipWriter::new(writer_cursor);

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("zip entry {}: {}", i, e))?;
            let name = file.name().to_string();
            let compression = file.compression();
            let options = zip::write::FileOptions::default().compression_method(compression);

            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("zip read entry {}: {}", name, e))?;

            // Patch the OPF. Use a plain string replace — the bug is a
            // literal hardcoded attribute in epub-builder's template, not
            // a dynamic value we need to regex for.
            if name == "OEBPS/content.opf" {
                let text = String::from_utf8_lossy(&contents).into_owned();
                let patched = text.replace(
                    r#"<dc:language id="epub-creator-0">"#,
                    "<dc:language>",
                );
                contents = patched.into_bytes();
            }

            writer
                .start_file(&name, options)
                .map_err(|e| format!("zip write start {}: {}", name, e))?;
            writer
                .write_all(&contents)
                .map_err(|e| format!("zip write data {}: {}", name, e))?;
        }

        writer.finish().map_err(|e| format!("zip finish: {}", e))?;
    }

    Ok(out)
}
