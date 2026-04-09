use crate::db::{self, AppState, FormatPage, FormatProfile};
use crate::ydoc;

use std::sync::Arc;
use typst::diag::{FileError, FileResult, SourceResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst_kit::fonts::{FontSearcher, FontSlot};

// Embedded fallback fonts (used in addition to system fonts)
static FONT_REGULAR: &[u8] = include_bytes!("../fonts/LiberationSerif-Regular.ttf");
static FONT_BOLD: &[u8] = include_bytes!("../fonts/LiberationSerif-Bold.ttf");
static FONT_ITALIC: &[u8] = include_bytes!("../fonts/LiberationSerif-Italic.ttf");
static FONT_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/LiberationSerif-BoldItalic.ttf");

/// Lazy-initialized font cache holding system fonts + embedded fallbacks.
/// Discovered once at first compile and reused thereafter.
pub struct FontCache {
    pub book: Arc<LazyHash<FontBook>>,
    pub slots: Arc<Vec<FontSlot>>,
}

impl FontCache {
    fn discover() -> Self {
        let t = std::time::Instant::now();
        let fonts = FontSearcher::new().include_system_fonts(true).search();
        log::info!(
            "[format] font discovery: {} families, {} slots in {}ms",
            fonts.book.families().count(),
            fonts.fonts.len(),
            t.elapsed().as_millis()
        );
        FontCache {
            book: Arc::new(LazyHash::new(fonts.book)),
            slots: Arc::new(fonts.fonts),
        }
    }
}

/// Get embedded fonts as Font instances (used as last-resort fallbacks).
fn embedded_fonts() -> Vec<Font> {
    [FONT_REGULAR, FONT_BOLD, FONT_ITALIC, FONT_BOLD_ITALIC]
        .iter()
        .filter_map(|data| Font::new(Bytes::new(data.to_vec()), 0))
        .collect()
}

/// Typst World implementation backed by the FontCache + an in-memory image store.
struct IweWorld {
    library: LazyHash<Library>,
    book: Arc<LazyHash<FontBook>>,
    slots: Arc<Vec<FontSlot>>,
    embedded: Vec<Font>,
    source: Source,
    images: std::collections::HashMap<String, Vec<u8>>,
}

impl IweWorld {
    fn new(
        markup: String,
        images: std::collections::HashMap<String, Vec<u8>>,
        cache: &FontCache,
    ) -> Self {
        IweWorld {
            library: LazyHash::new(Library::default()),
            book: cache.book.clone(),
            slots: cache.slots.clone(),
            embedded: embedded_fonts(),
            source: Source::detached(markup),
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
        // First N indices are system fonts via FontSlot
        if let Some(slot) = self.slots.get(index) {
            if let Some(font) = slot.get() {
                return Some(font);
            }
        }
        // Fall back to embedded fonts (offset by slot count)
        let fallback_idx = index.saturating_sub(self.slots.len());
        self.embedded.get(fallback_idx).cloned()
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

/// Detect if a plain-text paragraph is a scene-break sentinel (e.g. `***` or `* * *`).
/// Case-insensitive to be forgiving. Only asterisks + whitespace counts.
fn is_scene_break_sentinel(para: &str) -> bool {
    let trimmed = para.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Must contain at least one star and no non-star/whitespace characters
    let mut star_count = 0;
    for ch in trimmed.chars() {
        if ch == '*' {
            star_count += 1;
        } else if !ch.is_whitespace() {
            return false;
        }
    }
    star_count >= 3
}

/// Emit the Typst markup for a scene break with the given style, spacing, and keep behavior.
/// Takes `&mut ImageMap` so the `image` style can register decoded bytes for Typst's World.
fn emit_scene_break(
    out: &mut String,
    style: &str,
    custom_text: &str,
    space_above_em: f64,
    space_below_em: f64,
    keep_with_content: bool,
    image_data: &str,
    image_width_pct: f64,
    images: &mut ImageMap,
) {
    // The content of the break depends on the style
    let inner: String = match style {
        "none" => String::new(),
        "blank" => String::new(),
        "dinkus" => "\\* \\* \\*".to_string(),
        "asterism" => "⁂".to_string(),
        "rule" => "#line(length: 25%, stroke: 0.6pt)".to_string(),
        "custom" => escape_typst(custom_text),
        "image" => {
            if image_data.is_empty() {
                // No image uploaded yet — fall back to dinkus
                "\\* \\* \\*".to_string()
            } else if let Some((path, _ext)) = ingest_image(image_data, images) {
                let w = image_width_pct.clamp(1.0, 100.0);
                format!("#image(\"{}\", width: {}%)", path, w)
            } else {
                "\\* \\* \\*".to_string()
            }
        }
        _ => "\\* \\* \\*".to_string(),
    };

    // Wrap in a full-width block that resets the paragraph indent so the centered
    // content is actually centered (the chapter body has first-line-indent: 1.5em
    // which would otherwise push the ornament right of center).
    if keep_with_content {
        out.push_str("#block(sticky: true, width: 100%)[\n");
    } else {
        out.push_str("#block(width: 100%)[\n");
    }
    out.push_str("  #set par(first-line-indent: 0em, justify: false)\n");
    out.push_str(&format!("  #v({}em)\n", space_above_em));
    if !inner.is_empty() {
        out.push_str(&format!("  #align(center)[{}]\n", inner));
    }
    out.push_str(&format!("  #v({}em)\n", space_below_em));
    out.push_str("]\n\n");
}

/// Parse a category JSON column. Returns an empty map if the JSON is missing/invalid.
fn parse_category_json(s: &str) -> serde_json::Map<String, serde_json::Value> {
    if s.trim().is_empty() {
        return serde_json::Map::new();
    }
    serde_json::from_str::<serde_json::Value>(s)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
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

    // Track textStyle attributes separately so we wrap once with one #text() call
    let mut font_size: Option<String> = None;
    let mut font_family: Option<String> = None;

    for mark in marks {
        let mtype = mark.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match mtype {
            "bold" | "strong" => s = format!("*{}*", s),
            "italic" | "em" => s = format!("_{}_", s),
            "underline" => s = format!("#underline[{}]", s),
            "strike" | "s" => s = format!("#strike[{}]", s),
            "textStyle" => {
                let attrs = mark.get("attrs");
                if let Some(size) = attrs
                    .and_then(|a| a.get("fontSize"))
                    .and_then(|f| f.as_str())
                {
                    font_size = Some(size.to_string());
                }
                if let Some(family) = attrs
                    .and_then(|a| a.get("fontFamily"))
                    .and_then(|f| f.as_str())
                {
                    font_family = Some(family.to_string());
                }
            }
            _ => {}
        }
    }

    // Build a single #text(...) call with whichever attributes were set
    if font_size.is_some() || font_family.is_some() {
        let mut args: Vec<String> = Vec::new();
        if let Some(family) = font_family {
            args.push(format!("font: \"{}\"", escape_typst(&family)));
        }
        if let Some(size) = font_size {
            args.push(format!("size: {}", size));
        }
        s = format!("#text({})[{}]", args.join(", "), s);
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

    // Page setup — margins read from print_layout_json with fallback to scalar columns.
    // Canonical unit is inches. Frontend may store values entered in mm after converting.
    let print_layout = parse_category_json(&profile.print_layout_json);
    let margin_top = print_layout
        .get("margin_top_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(profile.margin_top_in);
    let margin_bottom = print_layout
        .get("margin_bottom_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(profile.margin_bottom_in);
    let margin_outside = print_layout
        .get("margin_outside_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(profile.margin_outside_in);
    let margin_inside = print_layout
        .get("margin_inside_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(profile.margin_inside_in);

    // Justified text + hyphenation: default true (standard book typography)
    let justify = print_layout
        .get("justify")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let hyphens = print_layout
        .get("hyphens")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Scene break (in-chapter) settings from breaks_json
    let breaks = parse_category_json(&profile.breaks_json);
    let break_style = breaks
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("dinkus")
        .to_string();
    let break_custom_text = breaks
        .get("custom_text")
        .and_then(|v| v.as_str())
        .unwrap_or("* * *")
        .to_string();
    let break_space_above_em = breaks
        .get("space_above_em")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.2);
    let break_space_below_em = breaks
        .get("space_below_em")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.2);
    let break_keep_with_content = breaks
        .get("keep_with_content")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let break_image_data = breaks
        .get("image_data")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let break_image_width_pct = breaks
        .get("image_width_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(25.0);

    doc.push_str(&format!(
        "#set page(\n  width: {}in,\n  height: {}in,\n  margin: (top: {}in, bottom: {}in, outside: {}in, inside: {}in),\n)\n\n",
        profile.trim_width_in,
        profile.trim_height_in,
        margin_top,
        margin_bottom,
        margin_outside,
        margin_inside,
    ));

    // Typography: read from typography_json with fallback to legacy scalar columns.
    // The body font/size/leading is scoped to chapter body content only — chapter
    // headings, format pages, headers/footers, page numbers etc. will get their
    // own font settings as those features are added.
    let typo = parse_category_json(&profile.typography_json);
    let body_font = typo
        .get("font")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| profile.font_body.clone());
    let body_size_pt = typo
        .get("size_pt")
        .and_then(|v| v.as_f64())
        .unwrap_or(profile.font_size_pt);
    let body_line_spacing = typo
        .get("line_spacing")
        .and_then(|v| v.as_f64())
        .unwrap_or(profile.line_spacing);
    let body_leading_em = body_line_spacing - 1.0;

    // Document-wide paragraph indent (chapter body only — wrappers reset for format pages).
    doc.push_str("#set par(first-line-indent: 1.5em)\n\n");

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

        // Chapter body — wrapped in a content block that scopes the body typography.
        // Only chapter body paragraphs use the picked body font/size/leading + justify/hyphens.
        doc.push_str("#[\n");
        doc.push_str(&format!(
            "#set text(font: \"{}\", size: {}pt, hyphenate: {})\n",
            escape_typst(&body_font),
            body_size_pt,
            hyphens,
        ));
        doc.push_str(&format!(
            "#set par(leading: {}em, first-line-indent: 1.5em, justify: {})\n",
            body_leading_em,
            justify,
        ));

        // Paragraphs from extracted text (\n\n separated)
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        // Track whether the *next* visible paragraph should be first-line-indent-free.
        // Reset after a scene break so the paragraph following the break isn't indented.
        let mut next_no_indent = true; // first paragraph after chapter heading
        for para in &paragraphs {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                continue;
            }

            if is_scene_break_sentinel(trimmed) {
                emit_scene_break(
                    &mut doc,
                    &break_style,
                    &break_custom_text,
                    break_space_above_em,
                    break_space_below_em,
                    break_keep_with_content,
                    &break_image_data,
                    break_image_width_pct,
                    &mut images,
                );
                next_no_indent = true; // paragraph after a scene break has no indent
                continue;
            }

            let escaped = escape_typst(trimmed);
            if next_no_indent {
                doc.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", escaped));
                next_no_indent = false;
            } else {
                doc.push_str(&format!("{}\n\n", escaped));
            }
        }
        doc.push_str("]\n");
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

fn compile_document(markup: &str, images: ImageMap, cache: &FontCache) -> Result<PagedDocument, String> {
    let world = IweWorld::new(markup.to_string(), images, cache);
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
    /// Lazily-initialized font cache (system + embedded)
    font_cache: Mutex<Option<Arc<FontCache>>>,
}

impl FormatState {
    pub fn new() -> Self {
        FormatState {
            document: Mutex::new(None),
            font_cache: Mutex::new(None),
        }
    }

    /// Get or initialize the font cache.
    pub fn get_or_init_fonts(&self) -> Arc<FontCache> {
        let mut guard = self.font_cache.lock().unwrap();
        if guard.is_none() {
            *guard = Some(Arc::new(FontCache::discover()));
        }
        guard.as_ref().unwrap().clone()
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

    // 2. Y.Doc text extraction (format-aware: emits * * * for horizontal rules)
    let t = Instant::now();
    let mut chapters: Vec<(i64, String, String)> = Vec::new();
    for ch in &chapters_raw {
        let text = ydoc::extract_text_for_format_from_bytes(&ch.content);
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
    let cache = format_state.get_or_init_fonts();
    let document = compile_document(&markup, images, &cache)?;
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

/// Return the list of available font family names (system + embedded fallbacks).
/// Sorted alphabetically and deduplicated.
#[tauri::command]
pub fn list_system_fonts(
    format_state: tauri::State<'_, FormatState>,
) -> Result<Vec<String>, String> {
    let cache = format_state.get_or_init_fonts();
    let mut families: Vec<String> = cache
        .book
        .families()
        .map(|(name, _)| name.to_string())
        .collect();
    families.sort_unstable();
    families.dedup();
    Ok(families)
}

/// Export a single page to SVG from the cached document (called by URI protocol handler).
pub fn render_page_svg(format_state: &FormatState, page_index: usize) -> Result<String, String> {
    let doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
    let document = doc_guard.as_ref().ok_or("No compiled document")?;
    let page = document.pages.get(page_index).ok_or("Page index out of range")?;
    Ok(typst_svg::svg(page))
}
