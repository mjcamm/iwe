use crate::db::{self, AppState, FormatPage, FormatProfile};
use crate::ydoc;

use typst::diag::{FileError, FileResult, SourceResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};

// Embedded Liberation Serif fonts
static FONT_REGULAR: &[u8] = include_bytes!("../fonts/LiberationSerif-Regular.ttf");
static FONT_BOLD: &[u8] = include_bytes!("../fonts/LiberationSerif-Bold.ttf");
static FONT_ITALIC: &[u8] = include_bytes!("../fonts/LiberationSerif-Italic.ttf");
static FONT_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/LiberationSerif-BoldItalic.ttf");

/// Minimal Typst World implementation with embedded fonts and an in-memory image store.
struct IweWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    source: Source,
    images: std::collections::HashMap<String, Vec<u8>>,
}

impl IweWorld {
    fn new(markup: String, images: std::collections::HashMap<String, Vec<u8>>) -> Self {
        // Load embedded fonts
        let font_data: Vec<&[u8]> = vec![FONT_REGULAR, FONT_BOLD, FONT_ITALIC, FONT_BOLD_ITALIC];
        let fonts: Vec<Font> = font_data
            .into_iter()
            .filter_map(|data| {
                let bytes = Bytes::new(data.to_vec());
                Font::new(bytes, 0)
            })
            .collect();

        let book = FontBook::from_fonts(fonts.iter());
        let source = Source::detached(markup);

        IweWorld {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            source,
            images,
        }
    }
}

impl typst::World for IweWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        // Match by string path against the embedded image store
        let vpath = id.vpath();
        let path = vpath.as_rooted_path();
        let key = path.to_string_lossy().replace('\\', "/");
        if let Some(bytes) = self.images.get(&key) {
            return Ok(Bytes::new(bytes.clone()));
        }
        Err(FileError::NotFound(vpath.as_rootless_path().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

// ---- Markup builder ----

/// Escape text for Typst markup — backslash-escape special chars.
fn escape_typst(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '#' | '$' | '\\' | '*' | '_' | '<' | '>' | '@' | '~' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Image storage collected during PM JSON conversion.
/// Maps a virtual file path to raw image bytes. Typst's World serves these on demand.
type ImageMap = std::collections::HashMap<String, Vec<u8>>;

/// Convert a ProseMirror/TipTap JSON document to Typst markup.
/// Supports paragraphs, headings, text marks (bold/italic/underline/strike/font-size),
/// text alignment, and embedded images.
/// Image bytes are added to `images` and referenced from markup as virtual paths.
fn pm_json_to_typst(json_str: &str, images: &mut ImageMap) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    if !value.is_object() {
        return None;
    }
    let mut out = String::new();
    if let Some(content) = value.get("content").and_then(|c| c.as_array()) {
        for node in content {
            convert_pm_node(node, &mut out, images);
        }
    }
    Some(out)
}

fn convert_pm_node(node: &serde_json::Value, out: &mut String, images: &mut ImageMap) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let children = node.get("content").and_then(|c| c.as_array());

    match node_type {
        "paragraph" => {
            let align = node
                .get("attrs")
                .and_then(|a| a.get("textAlign"))
                .and_then(|t| t.as_str());
            let inner = collect_pm_inline(children);
            if inner.trim().is_empty() {
                out.push_str("#v(1em)\n\n");
                return;
            }
            // The enclosing format page already disables first-line-indent,
            // so we only need to handle alignment here.
            match align {
                Some("center") => out.push_str(&format!("#align(center)[{}]\n\n", inner)),
                Some("right") => out.push_str(&format!("#align(right)[{}]\n\n", inner)),
                Some("justify") => out.push_str(&format!("#par(justify: true)[{}]\n\n", inner)),
                _ => out.push_str(&format!("{}\n\n", inner)),
            }
        }
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(|l| l.as_i64())
                .unwrap_or(2)
                .clamp(1, 6) as usize;
            let align = node
                .get("attrs")
                .and_then(|a| a.get("textAlign"))
                .and_then(|t| t.as_str());
            let inner = collect_pm_inline(children);
            let heading = format!("#heading(level: {}, outlined: false)[{}]\n\n", level, inner);
            match align {
                Some("center") => out.push_str(&format!("#align(center)[{}]\n", heading)),
                Some("right") => out.push_str(&format!("#align(right)[{}]\n", heading)),
                _ => out.push_str(&heading),
            }
        }
        "horizontalRule" => {
            out.push_str("#line(length: 100%)\n\n");
        }
        "hardBreak" => {
            out.push_str(" \\\n");
        }
        "image" => {
            let attrs = node.get("attrs");
            let src = attrs.and_then(|a| a.get("src")).and_then(|s| s.as_str());
            let width = attrs
                .and_then(|a| a.get("width"))
                .and_then(|w| w.as_str());
            if let Some(src) = src {
                if let Some((path, ext)) = ingest_image(src, images) {
                    let width_attr = match width {
                        Some(w) if !w.is_empty() => format!(", width: {}", normalize_width(w)),
                        _ => String::new(),
                    };
                    let _ = ext; // currently unused; could be used for format hint
                    out.push_str(&format!(
                        "#align(center)[#image(\"{}\"{})]\n\n",
                        path, width_attr
                    ));
                }
            }
        }
        _ => {
            // Unknown node — recurse into children
            if let Some(children) = children {
                for child in children {
                    convert_pm_node(child, out, images);
                }
            }
        }
    }
}

/// Decode a data URL or pass-through a virtual path. Returns (typst_path, extension).
fn ingest_image(src: &str, images: &mut ImageMap) -> Option<(String, String)> {
    if let Some(rest) = src.strip_prefix("data:") {
        // data:image/png;base64,XXXXX
        let (meta, data) = rest.split_once(',')?;
        let mime = meta.split(';').next().unwrap_or("");
        let ext = match mime {
            "image/png" => "png",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            _ => "png",
        };
        // Only handle base64 for now
        if !meta.contains("base64") {
            return None;
        }
        let bytes = base64_decode(data)?;
        // Use a hash so identical images dedupe
        let hash = simple_hash(&bytes);
        let path = format!("/iwe-img-{:x}.{}", hash, ext);
        images.entry(path.clone()).or_insert(bytes);
        Some((path, ext.to_string()))
    } else {
        None
    }
}

/// Minimal base64 decoder. Avoids pulling in a dep for one use.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for c in cleaned.chars() {
        let v: u32 = match c {
            'A'..='Z' => (c as u32) - ('A' as u32),
            'a'..='z' => (c as u32) - ('a' as u32) + 26,
            '0'..='9' => (c as u32) - ('0' as u32) + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            '=' => break,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

/// FNV-1a hash for image deduplication and stable virtual paths.
fn simple_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn normalize_width(w: &str) -> String {
    // Accept "300px", "50%", "3in" etc. Pass through if valid Typst length.
    if w.ends_with('%') {
        w.to_string()
    } else if w.ends_with("px") {
        // Convert px to pt approximately (1px = 0.75pt)
        if let Ok(n) = w.trim_end_matches("px").parse::<f64>() {
            format!("{}pt", n * 0.75)
        } else {
            "auto".to_string()
        }
    } else {
        w.to_string()
    }
}

fn collect_pm_inline(children: Option<&Vec<serde_json::Value>>) -> String {
    let mut out = String::new();
    let Some(children) = children else { return out; };
    for child in children {
        let node_type = child.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match node_type {
            "text" => {
                if let Some(text) = child.get("text").and_then(|t| t.as_str()) {
                    out.push_str(&apply_pm_marks(text, child.get("marks")));
                }
            }
            "hardBreak" => {
                out.push_str(" \\\n");
            }
            _ => {}
        }
    }
    out
}

fn apply_pm_marks(text: &str, marks: Option<&serde_json::Value>) -> String {
    let mut s = escape_typst(text);
    let Some(marks) = marks.and_then(|m| m.as_array()) else { return s; };

    // Track the textStyle font size separately so we can wrap once with #text(size:)
    let mut font_size: Option<String> = None;

    for mark in marks {
        let mtype = mark.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match mtype {
            "bold" | "strong" => s = format!("*{}*", s),
            "italic" | "em" => s = format!("_{}_", s),
            "underline" => s = format!("#underline[{}]", s),
            "strike" | "s" => s = format!("#strike[{}]", s),
            "textStyle" => {
                if let Some(size) = mark
                    .get("attrs")
                    .and_then(|a| a.get("fontSize"))
                    .and_then(|f| f.as_str())
                {
                    // Accept "12pt", "14pt", "16px" — pass through to Typst
                    font_size = Some(size.to_string());
                }
            }
            _ => {}
        }
    }

    if let Some(size) = font_size {
        s = format!("#text(size: {})[{}]", size, s);
    }
    s
}

/// Render a page's content field — JSON if it parses, otherwise plain text.
/// Any embedded images are added to the shared image map.
fn render_page_content(content: &str, images: &mut ImageMap) -> String {
    if content.trim().is_empty() {
        return String::new();
    }
    // Try ProseMirror JSON first
    if content.trim_start().starts_with('{') {
        if let Some(typst) = pm_json_to_typst(content, images) {
            return typst;
        }
    }
    // Fallback: treat as plain text with paragraph breaks
    let mut out = String::new();
    for para in content.split("\n\n") {
        let trimmed = para.trim();
        if !trimmed.is_empty() {
            out.push_str(&format!("{}\n\n", escape_typst(trimmed)));
        }
    }
    out
}

/// Build a Typst markup string for a front/back matter page.
/// If the page has rich content (PM JSON), render it directly.
/// Otherwise apply role-based defaults using the title.
fn build_front_matter_page(page: &FormatPage, images: &mut ImageMap) -> String {
    let mut out = String::new();
    let title_escaped = escape_typst(&page.title);
    let content_typst = render_page_content(&page.content, images);
    let has_content = !content_typst.trim().is_empty();

    // If user has authored content, render it directly without role-based wrapping.
    // Wrap in a content block that resets par settings — format pages should never
    // inherit the chapter body's first-line indent.
    if has_content {
        out.push_str("#[\n");
        out.push_str("#set par(first-line-indent: 0em, justify: false)\n");
        // Apply vertical alignment using #v(1fr) gutters
        match page.vertical_align.as_str() {
            "center" => {
                out.push_str("#v(1fr)\n");
                out.push_str(&content_typst);
                out.push_str("#v(1fr)\n");
            }
            "bottom" => {
                out.push_str("#v(1fr)\n");
                out.push_str(&content_typst);
            }
            _ => {
                // top (default)
                out.push_str(&content_typst);
            }
        }
        out.push_str("]\n");
        return out;
    }

    // Empty content — apply role-based default placeholders
    match page.page_role.as_str() {
        "title" => {
            out.push_str("#align(center + horizon)[\n");
            out.push_str(&format!("  #text(size: 24pt, weight: \"bold\")[{}]\n", title_escaped));
            out.push_str("]\n");
        }
        "copyright" => {
            out.push_str("#set text(size: 8pt)\n");
            out.push_str("#align(bottom)[\n");
            out.push_str(&format!("  {}\n", title_escaped));
            out.push_str("]\n");
        }
        "dedication" => {
            out.push_str("#align(center + horizon)[\n");
            out.push_str("#set text(style: \"italic\")\n");
            out.push_str(&format!("  {}\n", title_escaped));
            out.push_str("]\n");
        }
        "toc" => {
            // TOC is auto-generated by Typst outline
            out.push_str("#outline(title: \"Contents\", depth: 1)\n");
        }
        "half-title" => {
            out.push_str("#align(center + horizon)[\n");
            out.push_str(&format!("  #text(size: 18pt)[{}]\n", title_escaped));
            out.push_str("]\n");
        }
        _ => {
            out.push_str("#align(center)[\n");
            out.push_str(&format!("  #text(size: 16pt, weight: \"bold\")[{}]\n", title_escaped));
            out.push_str("]\n");
        }
    }

    out
}

/// Build complete Typst markup for the entire book.
/// Returns the markup string, section ID list, and image map for embedded images.
fn build_typst_markup(
    profile: &FormatProfile,
    front_pages: &[FormatPage],
    chapters: &[(i64, String, String)], // (chapter_id, title, text)
    back_pages: &[FormatPage],
) -> (String, Vec<String>, ImageMap) {
    let mut doc = String::new();
    let mut section_ids: Vec<String> = Vec::new();
    let mut images: ImageMap = std::collections::HashMap::new();

    // Page setup
    doc.push_str(&format!(
        "#set page(\n  width: {}in,\n  height: {}in,\n  margin: (top: {}in, bottom: {}in, outside: {}in, inside: {}in),\n)\n\n",
        profile.trim_width_in,
        profile.trim_height_in,
        profile.margin_top_in,
        profile.margin_bottom_in,
        profile.margin_outside_in,
        profile.margin_inside_in,
    ));

    // Font and paragraph setup
    let font_name = escape_typst(&profile.font_body);
    doc.push_str(&format!(
        "#set text(font: \"{}\", size: {}pt)\n",
        font_name, profile.font_size_pt,
    ));
    // Convert line spacing multiplier to Typst leading (em units).
    // Typst leading is extra space between lines, not the line-height multiplier.
    // leading = (multiplier - 1) * font_size, expressed in em = (multiplier - 1)
    let leading_em = profile.line_spacing - 1.0;
    doc.push_str(&format!(
        "#set par(leading: {}em, first-line-indent: 1.5em)\n\n",
        leading_em,
    ));

    // Helper to emit an invisible labeled anchor for a section.
    // We use #metadata wrapped with a label so the introspector can find it.
    let emit_anchor = |out: &mut String, id: &str| {
        out.push_str(&format!("#[#metadata(none) <{}>]\n", id));
    };

    // ---- Front matter ----
    // Front matter uses roman numeral page numbering and no heading numbering
    if !front_pages.is_empty() {
        doc.push_str("#set page(numbering: \"i\")\n");
        doc.push_str("#counter(page).update(1)\n\n");

        for page in front_pages {
            let sid = format!("iwe-fp-{}", page.id);
            emit_anchor(&mut doc, &sid);
            section_ids.push(sid);
            doc.push_str(&build_front_matter_page(page, &mut images));
            doc.push_str("#pagebreak()\n\n");
        }
    }

    // ---- Body (chapters) ----
    // Switch to arabic page numbering
    doc.push_str("#set page(numbering: \"1\")\n");
    doc.push_str("#counter(page).update(1)\n\n");

    for (i, (chapter_id, title, text)) in chapters.iter().enumerate() {
        if i > 0 {
            doc.push_str("#pagebreak()\n\n");
        }

        let sid = format!("iwe-ch-{}", chapter_id);
        emit_anchor(&mut doc, &sid);
        section_ids.push(sid);

        // Chapter heading — centered, no indent for first paragraph after
        let escaped_title = escape_typst(title);
        doc.push_str(&format!(
            "#heading(level: 1, outlined: true)[{}]\n",
            escaped_title,
        ));
        doc.push_str("#v(2em)\n");

        // Paragraphs from extracted text (\n\n separated)
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        for (pi, para) in paragraphs.iter().enumerate() {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                continue;
            }
            let escaped = escape_typst(trimmed);
            if pi == 0 {
                // First paragraph: no indent
                doc.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", escaped));
            } else {
                doc.push_str(&format!("{}\n\n", escaped));
            }
        }
    }

    // ---- Back matter ----
    if !back_pages.is_empty() {
        doc.push_str("#pagebreak()\n\n");
        for page in back_pages {
            let sid = format!("iwe-fp-{}", page.id);
            emit_anchor(&mut doc, &sid);
            section_ids.push(sid);
            doc.push_str(&build_front_matter_page(page, &mut images));
            doc.push_str("#pagebreak()\n\n");
        }
    }

    // Chapter heading style — centered, no numbering
    // Put this at the top level so it applies to all headings
    let heading_style = format!(
        "#show heading.where(level: 1): it => {{\n  set align(center)\n  set text(size: 16pt, weight: \"regular\")\n  v(3em)\n  it.body\n  v(1em)\n}}\n\n"
    );

    // Prepend heading style after page/font setup
    (format!("{}{}", heading_style, doc), section_ids, images)
}

// ---- Compile & SVG render ----

use std::sync::Mutex;
use std::time::Instant;
use serde::Serialize;

fn compile_document(markup: &str, images: ImageMap) -> Result<PagedDocument, String> {
    let world = IweWorld::new(markup.to_string(), images);
    let warned = typst::compile::<PagedDocument>(&world);
    match warned.output {
        Ok(doc) => Ok(doc),
        Err(diagnostics) => {
            let mut errors = Vec::new();
            for diag in diagnostics {
                errors.push(format!("{:?}", diag.message));
            }
            Err(format!("Typst compilation failed: {}", errors.join("; ")))
        }
    }
}

// ---- Cached document state ----

pub struct FormatState {
    document: Mutex<Option<PagedDocument>>,
}

impl FormatState {
    pub fn new() -> Self {
        FormatState {
            document: Mutex::new(None),
        }
    }
}

// ---- Response types ----

#[derive(Serialize, Clone)]
pub struct CompileTiming {
    pub db_load_ms: f64,
    pub ydoc_extract_ms: f64,
    pub markup_build_ms: f64,
    pub typst_compile_ms: f64,
    pub total_ms: f64,
    pub page_count: usize,
    pub chapter_count: usize,
    pub markup_len: usize,
}

#[derive(Serialize, Clone)]
pub struct CompileResult {
    pub page_count: usize,
    pub timing: CompileTiming,
    /// section_id -> 0-based page index
    pub section_pages: std::collections::HashMap<String, usize>,
}


// ---- Tauri commands ----

/// Compile the document and cache it. Returns page count + timing. No SVGs yet.
#[tauri::command]
pub fn compile_preview(
    state: tauri::State<'_, AppState>,
    format_state: tauri::State<'_, FormatState>,
    profile_id: i64,
) -> Result<CompileResult, String> {
    let total_start = Instant::now();

    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let profile = db::get_format_profile(conn, profile_id)
        .map_err(|e| e.to_string())?
        .ok_or("Profile not found")?;

    if profile.target_type == "ebook" {
        // Clear cached doc
        let mut doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
        *doc_guard = None;
        return Ok(CompileResult {
            page_count: 0,
            timing: CompileTiming {
                db_load_ms: 0.0, ydoc_extract_ms: 0.0, markup_build_ms: 0.0,
                typst_compile_ms: 0.0, total_ms: 0.0, page_count: 0,
                chapter_count: 0, markup_len: 0,
            },
            section_pages: std::collections::HashMap::new(),
        });
    }

    // 1. DB load
    let t = Instant::now();
    let all_format_pages = db::list_format_pages(conn).map_err(|e| e.to_string())?;
    let excluded_ids: std::collections::HashSet<i64> =
        db::list_excluded_page_ids_for_profile(conn, profile_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
    let format_pages: Vec<FormatPage> = all_format_pages
        .into_iter()
        .filter(|p| !excluded_ids.contains(&p.id))
        .collect();
    let chapters_raw = db::list_chapters(conn).map_err(|e| e.to_string())?;
    let db_load_ms = t.elapsed().as_secs_f64() * 1000.0;

    // 2. Y.Doc text extraction
    let t = Instant::now();
    let mut chapters: Vec<(i64, String, String)> = Vec::new();
    for ch in &chapters_raw {
        let text = ydoc::extract_text_with_breaks_from_bytes(&ch.content);
        chapters.push((ch.id, ch.title.clone(), text));
    }
    let ydoc_extract_ms = t.elapsed().as_secs_f64() * 1000.0;
    let chapter_count = chapters.len();

    let front: Vec<FormatPage> = format_pages.iter().filter(|p| p.position == "front").cloned().collect();
    let back: Vec<FormatPage> = format_pages.iter().filter(|p| p.position == "back").cloned().collect();

    // 3. Markup generation
    let t = Instant::now();
    let (markup, section_ids, images) = build_typst_markup(&profile, &front, &chapters, &back);
    let markup_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let markup_len = markup.len();

    // 4. Typst compilation
    let t = Instant::now();
    let document = compile_document(&markup, images)?;
    let typst_compile_ms = t.elapsed().as_secs_f64() * 1000.0;

    let page_count = document.pages.len();

    // 5. Resolve section labels to page numbers
    let section_pages = resolve_section_pages(&document, &section_ids);

    let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "[format] compile: {}ms total | db:{}ms ydoc:{}ms markup:{}ms compile:{}ms | {} pages, {} chapters, {} sections resolved, {}KB markup",
        total_ms as u32, db_load_ms as u32, ydoc_extract_ms as u32, markup_build_ms as u32,
        typst_compile_ms as u32, page_count, chapter_count, section_pages.len(), markup_len / 1024,
    );

    // Cache the compiled document
    let mut doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
    *doc_guard = Some(document);

    Ok(CompileResult {
        page_count,
        timing: CompileTiming {
            db_load_ms,
            ydoc_extract_ms,
            markup_build_ms,
            typst_compile_ms,
            total_ms,
            page_count,
            chapter_count,
            markup_len,
        },
        section_pages,
    })
}

/// Walk the introspector and resolve each section label to a 0-based page index.
fn resolve_section_pages(
    document: &PagedDocument,
    section_ids: &[String],
) -> std::collections::HashMap<String, usize> {
    use typst::foundations::Label;
    let mut map = std::collections::HashMap::new();
    for sid in section_ids {
        let Some(label) = Label::new(typst::utils::PicoStr::intern(sid)) else {
            continue;
        };
        if let Ok(content) = document.introspector.query_label(label) {
            if let Some(loc) = content.location() {
                let page = document.introspector.page(loc).get();
                map.insert(sid.clone(), page - 1); // 0-indexed
            }
        }
    }
    map
}

/// Export a single page to SVG from the cached document (called by URI protocol handler).
pub fn render_page_svg(format_state: &FormatState, page_index: usize) -> Result<String, String> {
    let doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
    let document = doc_guard.as_ref().ok_or("No compiled document")?;
    let page = document.pages.get(page_index).ok_or("Page index out of range")?;
    Ok(typst_svg::svg(page))
}
