// When the "format" feature is enabled, the real Typst-based implementation
// is compiled.  When disabled, lightweight stubs let lib.rs compile without
// pulling in typst / typst-pdf / typst-svg / typst-kit.

#[cfg(feature = "format")]
mod real {
use crate::db::{self, AppState, FormatPage, FormatProfile};
use crate::ydoc;

use rusqlite::Connection;
use std::sync::Arc;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst_kit::fonts::{FontSearcher, FontSlot};
use typst_kit::package::PackageStorage;
use typst_kit::download::{Downloader, ProgressSink};

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

/// Typst World implementation backed by FontCache + in-memory images + package storage.
struct IweWorld {
    library: LazyHash<Library>,
    book: Arc<LazyHash<FontBook>>,
    slots: Arc<Vec<FontSlot>>,
    embedded: Vec<Font>,
    source: Source,
    images: std::collections::HashMap<String, Vec<u8>>,
    packages: Arc<PackageStorage>,
}

impl IweWorld {
    fn new(
        markup: String,
        images: std::collections::HashMap<String, Vec<u8>>,
        cache: &FontCache,
        packages: Arc<PackageStorage>,
    ) -> Self {
        IweWorld {
            library: LazyHash::new(Library::default()),
            book: cache.book.clone(),
            slots: cache.slots.clone(),
            embedded: embedded_fonts(),
            source: Source::detached(markup),
            images,
            packages,
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
            return Ok(self.source.clone());
        }
        // Try resolving as a package file
        let bytes = self.file(id)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text.into()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        // Check in-memory images first
        let vpath = id.vpath();
        let path = vpath.as_rooted_path();
        let key = path.to_string_lossy().replace('\\', "/");
        if let Some(bytes) = self.images.get(&key) {
            return Ok(Bytes::new(bytes.clone()));
        }
        // Try resolving as a package file
        if let Some(package) = id.package() {
            let package_dir = self.packages.prepare_package(package, &mut ProgressSink)
                .map_err(|e| FileError::Other(Some(typst::diag::eco_format!("{e}"))))?;
            let resolved = id.vpath().resolve(&package_dir)
                .ok_or(FileError::AccessDenied)?;
            let data = std::fs::read(&resolved)
                .map_err(|e| FileError::from_io(e, &resolved))?;
            return Ok(Bytes::new(data));
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

/// Snap a byte offset to the nearest char boundary at or before `offset`.
/// Prevents panics when slicing UTF-8 strings at offsets that land inside
/// multi-byte characters (e.g. en-dash U+2013 is 3 bytes).
fn snap_to_char_boundary(text: &str, offset: usize) -> usize {
    let mut pos = offset.min(text.len());
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Extract the primary font name from a CSS font-family value.
/// TipTap stores full CSS fallback chains like `"Times New Roman", serif`.
/// Typst only wants a single name like `Times New Roman`.
fn extract_font_name(css_family: &str) -> &str {
    // Take the first entry before any comma (discard fallback fonts)
    let first = css_family.split(',').next().unwrap_or(css_family).trim();
    // Strip surrounding quotes (CSS uses either " or ')
    first.trim_matches(|c: char| c == '"' || c == '\'')
}

/// Escape text for Typst markup — backslash-escape special chars.
fn escape_typst(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '#' | '$' | '\\' | '*' | '_' | '<' | '>' | '@' | '~' | '`' | '[' | ']' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    // In Typst, `- `, `+ `, `/ ` at the start of a content block (even with
    // leading whitespace) are list/term markers. Text like " - Grandmother"
    // must have the marker character escaped so it renders as literal text.
    // This matters because inline text nodes end up inside `[...]` content blocks.
    let trimmed = out.trim_start();
    if trimmed.starts_with("- ") || trimmed.starts_with("+ ") || trimmed.starts_with("/ ")
        || trimmed.starts_with("– ") || trimmed.starts_with("— ")
    {
        let marker_pos = out.len() - trimmed.len();
        out.insert(marker_pos, '\\');
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
    image_src: &str,
    image_width_pct: f64,
    images: &mut ImageMap,
    conn: &Connection,
    preview_quality: bool,
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
            if image_src.is_empty() {
                // No image uploaded yet — fall back to dinkus
                "\\* \\* \\*".to_string()
            } else if let Some((path, _ext)) = ingest_image(image_src, images, conn, preview_quality) {
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

// ---- Header/Footer helpers ----

const SLOTS: &[&str] = &[
    "verso_header_left", "verso_header_center", "verso_header_right",
    "verso_footer_left", "verso_footer_center", "verso_footer_right",
    "recto_header_left", "recto_header_center", "recto_header_right",
    "recto_footer_left", "recto_footer_center", "recto_footer_right",
];

fn slot_has_content(slots: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    slots.get(key)
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .map(|s| s != "none")
        .unwrap_or(false)
}

fn get_slot(slots: &serde_json::Map<String, serde_json::Value>, key: &str) -> (String, String, String, f64, String) {
    let slot = slots.get(key).and_then(|v| v.as_object());
    let content = slot.and_then(|s| s.get("content")).and_then(|v| v.as_str()).unwrap_or("none").to_string();
    let custom  = slot.and_then(|s| s.get("custom")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let font    = slot.and_then(|s| s.get("font")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let size_pt = slot.and_then(|s| s.get("size_pt")).and_then(|v| v.as_f64()).unwrap_or(9.0);
    let style   = slot.and_then(|s| s.get("style")).and_then(|v| v.as_str()).unwrap_or("normal").to_string();
    (content, custom, font, size_pt, style)
}

/// Emit the content for a single header/footer slot. Called inside `[...]` content
/// blocks (grid cells), so Typst function calls need `#` prefixes.
fn emit_slot_content(out: &mut String, content: &str, custom: &str, font: &str, size_pt: f64, style: &str) {
    if content == "none" { return; }

    // Build the dynamic content expression.
    // Book metadata are Typst variables set at the document top.
    // Chapter title uses a query to find the last level-1 heading before the current page.
    let body = match content {
        "page_number"   => "#counter(page).display()".to_string(),
        "book_title"    => "#iwe-book-title".to_string(),
        "author_name"   => "#iwe-author-name".to_string(),
        "series_name"   => "#iwe-series-name".to_string(),
        "book_number"   => "#iwe-series-number".to_string(),
        "chapter_title" => {
            // Query the most recent level-1 heading before the current position.
            // We're already inside a `context { }` block so query() works.
            "#{ let hdgs = query(heading.where(level: 1).before(here())); if hdgs.len() > 0 { hdgs.last().body } }".to_string()
        }
        "custom"        => escape_typst(custom),
        _ => return,
    };

    // Wrap with text styling — all inside a content block so we use `#` prefix
    let mut attrs = vec![format!("size: {}pt", size_pt)];
    if !font.is_empty() {
        attrs.push(format!("font: \"{}\"", escape_typst(font)));
    }
    if style == "italic" {
        attrs.push("style: \"italic\"".to_string());
    }

    let styled = format!("#text({})[{}]", attrs.join(", "), body);
    let result = match style {
        "smallcaps" => format!("#smallcaps[{}]", styled),
        "uppercase" => format!("#upper[{}]", styled),
        _           => styled,
    };
    out.push_str(&result);
}

/// Emit a header or footer bar with recto/verso branching.
/// Uses code blocks `{ }` throughout so `let`/`if` statements work without `#` prefixes.
fn emit_hf_bar(
    out: &mut String,
    slots: &serde_json::Map<String, serde_json::Value>,
    position: &str, // "header" | "footer"
    separator: bool,
    sep_thickness_pt: f64,
    margin_left_in: f64,
    margin_right_in: f64,
) {
    out.push_str("    let is-even = calc.even(here().page())\n");

    let vl = format!("verso_{}_left", position);
    let vc = format!("verso_{}_center", position);
    let vr = format!("verso_{}_right", position);
    let rl = format!("recto_{}_left", position);
    let rc = format!("recto_{}_center", position);
    let rr = format!("recto_{}_right", position);

    let is_footer = position == "footer";
    out.push_str("    if is-even {\n");
    emit_hf_row(out, slots, &vl, &vc, &vr, separator, sep_thickness_pt, margin_left_in, margin_right_in, is_footer);
    out.push_str("    } else {\n");
    emit_hf_row(out, slots, &rl, &rc, &rr, separator, sep_thickness_pt, margin_left_in, margin_right_in, is_footer);
    out.push_str("    }\n");
}

fn emit_hf_row(
    out: &mut String,
    slots: &serde_json::Map<String, serde_json::Value>,
    left_key: &str,
    center_key: &str,
    right_key: &str,
    separator: bool,
    sep_thickness_pt: f64,
    margin_left_in: f64,
    margin_right_in: f64,
    is_footer: bool,
) {
    let (lc, lcust, lf, ls, lsty) = get_slot(slots, left_key);
    let (cc, ccust, cf, cs, csty) = get_slot(slots, center_key);
    let (rc, rcust, rf, rs, rsty) = get_slot(slots, right_key);

    let has_left   = lc != "none";
    let has_center = cc != "none";
    let has_right  = rc != "none";

    if !has_left && !has_center && !has_right {
        return;
    }

    // Footer separator goes ABOVE the content; header separator goes BELOW.
    if separator && is_footer {
        out.push_str(&format!("      line(length: 100%, stroke: {}pt)\n      v(4pt)\n", sep_thickness_pt));
    }

    // Use a grid with 3 equal columns for consistent alignment.
    // We're inside a code block `{ }`, so grid() is called without `#`.
    // Wrap in pad() if inset margins are configured.
    let has_pad = margin_left_in > 0.0 || margin_right_in > 0.0;
    if has_pad {
        out.push_str(&format!(
            "      pad(left: {}in, right: {}in,\n      ",
            margin_left_in, margin_right_in
        ));
    }
    out.push_str("      grid(columns: (1fr, 1fr, 1fr), align: (left, center, right),\n");

    // Left
    out.push_str("        [");
    if has_left { emit_slot_content(out, &lc, &lcust, &lf, ls, &lsty); }
    out.push_str("],\n");

    // Center
    out.push_str("        [");
    if has_center { emit_slot_content(out, &cc, &ccust, &cf, cs, &csty); }
    out.push_str("],\n");

    // Right
    out.push_str("        [");
    if has_right { emit_slot_content(out, &rc, &rcust, &rf, rs, &rsty); }
    out.push_str("],\n");

    out.push_str("      )\n");
    if has_pad {
        out.push_str("      )\n"); // close pad()
    }

    // Header separator goes BELOW the content
    if separator && !is_footer {
        out.push_str(&format!("      v(4pt)\n      line(length: 100%, stroke: {}pt)\n", sep_thickness_pt));
    }
}

/// Emit an opener paragraph with drop cap and/or small caps lead-in.
/// Emit an opener paragraph with drop cap (via the droplet package) and/or small caps lead-in.
fn emit_opener_paragraph(
    out: &mut String,
    raw_text: &str,
    drop_enabled: bool,
    drop_lines: usize,
    drop_font: &str,
    drop_color: &str,
    quote_mode: &str,
    _drop_fill_pct: f64,
    sc_enabled: bool,
    sc_words: i32,
) {
    let chars: Vec<char> = raw_text.chars().collect();
    if chars.is_empty() {
        return;
    }

    // Determine if text starts with a quote mark
    let quote_chars = ['"', '\u{201C}', '\u{201D}', '\'', '\u{2018}', '\u{2019}', '\u{00AB}', '\u{00BB}'];
    let starts_with_quote = quote_chars.contains(&chars[0]);

    // Determine the drop cap character(s) and remaining text based on quote handling mode.
    let (drop_cap_text, body_start_idx): (String, usize) = if drop_enabled && drop_lines > 0 {
        if starts_with_quote {
            match quote_mode {
                "disable_on_dialogue" => {
                    // No drop cap for dialogue — emit as flush paragraph
                    let escaped = escape_typst(raw_text);
                    out.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", escaped));
                    return;
                }
                "first_char" => {
                    // Literally the first character only — even if it's a quote mark
                    (chars[0].to_string(), 1)
                }
                "both_together" => {
                    // Quote + first letter together as a unit
                    let end = if chars.len() > 1 { 2 } else { 1 };
                    (chars[..end].iter().collect(), end)
                }
                _ => {
                    // "letter_only" — skip to first alphabetic char, quote goes into body
                    let mut idx = 0;
                    while idx < chars.len() && !chars[idx].is_alphabetic() {
                        idx += 1;
                    }
                    if idx < chars.len() {
                        (chars[idx].to_string(), idx + 1)
                    } else {
                        let escaped = escape_typst(raw_text);
                        out.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", escaped));
                        return;
                    }
                }
            }
        } else {
            // Normal text (no quote) — first character is the drop cap
            (chars[0].to_string(), 1)
        }
    } else {
        (String::new(), 0)
    };

    let remaining: String = chars[body_start_idx..].iter().collect();
    // For "letter_only" mode, the leading punctuation is simply discarded —
    // it doesn't appear in the drop cap or the body text.
    let prefix = String::new();

    let body_text = if sc_enabled && !remaining.is_empty() {
        apply_small_caps_lead(&remaining, sc_words)
    } else {
        escape_typst(&remaining)
    };

    if drop_enabled && drop_lines > 0 && !drop_cap_text.is_empty() {
        // Use the droplet package's dropcap() function.
        // It handles all the text splitting, measurement, and layout correctly.
        let mut args = vec![format!("height: {}", drop_lines)];
        args.push("gap: 4pt".to_string());
        if !drop_font.is_empty() {
            args.push(format!("font: \"{}\"", escape_typst(drop_font)));
        }
        if drop_color != "#000000" && !drop_color.is_empty() {
            args.push(format!("fill: rgb(\"{}\")", drop_color));
        }

        // For "letter_only" mode, the quote/punctuation goes at the start of the
        // body text so it flows naturally on the first line beside the drop cap.
        let full_body = if !prefix.is_empty() {
            format!("{}{}", escape_typst(&prefix), body_text)
        } else {
            body_text.clone()
        };
        out.push_str(&format!(
            "#dropcap({})[{}][{}]\n\n",
            args.join(", "),
            escape_typst(&drop_cap_text),
            full_body,
        ));
    } else {
        // No drop cap — flush left paragraph
        out.push_str("#par(first-line-indent: 0em)[\n");
        if !prefix.is_empty() {
            out.push_str(&format!("  {}", escape_typst(&prefix)));
        }
        out.push_str(&format!("  {}\n", body_text));
        out.push_str("]\n\n");
    }
}

/// Generate the Typst preamble that imports the droplet package.
fn dropcap_preamble() -> String {
    "#import \"@preview/droplet:0.3.1\": dropcap\n\n".to_string()
}

/// Apply small caps to the first N words of text. Returns Typst markup.
fn apply_small_caps_lead(text: &str, word_count: i32) -> String {
    if word_count == 0 {
        return escape_typst(text);
    }

    let mut rest_start = 0;
    let mut in_word = false;
    let mut count = 0;

    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if in_word {
                in_word = false;
                if word_count > 0 && count >= word_count {
                    rest_start = i;
                    break;
                }
            }
        } else {
            if !in_word {
                in_word = true;
                count += 1;
            }
        }
        rest_start = i + ch.len_utf8();
    }

    let lead = &text[..rest_start];
    let rest = &text[rest_start..];

    if lead.is_empty() {
        return escape_typst(text);
    }

    let mut result = format!("#smallcaps[{}]", escape_typst(lead));
    if !rest.is_empty() {
        result.push_str(&escape_typst(rest));
    }
    result
}

/// Formatted-span-aware opener paragraph. Same logic as `emit_opener_paragraph`
/// but preserves inline formatting marks from FmtSpan data.
fn emit_opener_paragraph_fmt(
    out: &mut String,
    spans: &[FmtSpan],
    drop_enabled: bool,
    drop_lines: usize,
    drop_font: &str,
    drop_color: &str,
    quote_mode: &str,
    _drop_fill_pct: f64,
    sc_enabled: bool,
    sc_words: i32,
) {
    let plain = spans_plain_text(spans);
    let chars: Vec<char> = plain.chars().collect();
    if chars.is_empty() {
        return;
    }

    let quote_chars = ['"', '\u{201C}', '\u{201D}', '\'', '\u{2018}', '\u{2019}', '\u{00AB}', '\u{00BB}'];
    let starts_with_quote = quote_chars.contains(&chars[0]);

    // Determine the drop cap character(s) and the char index where the body starts.
    let (drop_cap_text, body_start_idx): (String, usize) = if drop_enabled && drop_lines > 0 {
        if starts_with_quote {
            match quote_mode {
                "disable_on_dialogue" => {
                    out.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", spans_to_typst(spans)));
                    return;
                }
                "first_char" => (chars[0].to_string(), 1),
                "both_together" => {
                    let end = if chars.len() > 1 { 2 } else { 1 };
                    (chars[..end].iter().collect(), end)
                }
                _ => {
                    // "letter_only" — skip to first alphabetic
                    let mut idx = 0;
                    while idx < chars.len() && !chars[idx].is_alphabetic() { idx += 1; }
                    if idx < chars.len() {
                        (chars[idx].to_string(), idx + 1)
                    } else {
                        out.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", spans_to_typst(spans)));
                        return;
                    }
                }
            }
        } else {
            (chars[0].to_string(), 1)
        }
    } else {
        (String::new(), 0)
    };

    // Split spans at the body start index to get the body with formatting preserved
    let (_pre_spans, body_spans) = split_spans_at(spans, body_start_idx);

    let body_text = if sc_enabled && !body_spans.is_empty() {
        apply_small_caps_lead_fmt(&body_spans, sc_words)
    } else {
        spans_to_typst(&body_spans)
    };

    if drop_enabled && drop_lines > 0 && !drop_cap_text.is_empty() {
        let mut args = vec![format!("height: {}", drop_lines)];
        args.push("gap: 4pt".to_string());
        if !drop_font.is_empty() {
            args.push(format!("font: \"{}\"", escape_typst(drop_font)));
        }
        if drop_color != "#000000" && !drop_color.is_empty() {
            args.push(format!("fill: rgb(\"{}\")", drop_color));
        }
        out.push_str(&format!(
            "#dropcap({})[{}][{}]\n\n",
            args.join(", "),
            escape_typst(&drop_cap_text),
            body_text,
        ));
    } else {
        out.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", body_text));
    }
}

/// Apply small caps to the first N words, preserving formatting on the rest.
fn apply_small_caps_lead_fmt(spans: &[FmtSpan], word_count: i32) -> String {
    if word_count == 0 {
        return spans_to_typst(spans);
    }

    // Find the character index where the first N words end
    let plain = spans_plain_text(spans);
    let mut rest_start = 0usize;
    let mut in_word = false;
    let mut count = 0i32;

    for (i, ch) in plain.char_indices() {
        if ch.is_whitespace() {
            if in_word {
                in_word = false;
                if word_count > 0 && count >= word_count {
                    rest_start = i;
                    break;
                }
            }
        } else if !in_word {
            in_word = true;
            count += 1;
        }
        rest_start = i + ch.len_utf8();
    }

    if rest_start == 0 {
        return spans_to_typst(spans);
    }

    let (lead_spans, rest_spans) = split_spans_at(spans, rest_start);
    let mut result = format!("#smallcaps[{}]", spans_to_typst(&lead_spans));
    if !rest_spans.is_empty() {
        result.push_str(&spans_to_typst(&rest_spans));
    }
    result
}

/// Format a chapter number in the chosen style.
fn format_chapter_number(num: usize, format: &str) -> String {
    let word = match num {
        1 => "One", 2 => "Two", 3 => "Three", 4 => "Four", 5 => "Five",
        6 => "Six", 7 => "Seven", 8 => "Eight", 9 => "Nine", 10 => "Ten",
        11 => "Eleven", 12 => "Twelve", 13 => "Thirteen", 14 => "Fourteen",
        15 => "Fifteen", 16 => "Sixteen", 17 => "Seventeen", 18 => "Eighteen",
        19 => "Nineteen", 20 => "Twenty",
        21..=29 => { return format_chapter_number_compound(num, "Twenty", format); }
        30..=39 => { return format_chapter_number_compound(num, "Thirty", format); }
        40..=49 => { return format_chapter_number_compound(num, "Forty", format); }
        50..=59 => { return format_chapter_number_compound(num, "Fifty", format); }
        _ => &format!("{}", num), // fallback to numeric for large numbers
    };
    match format {
        "numeric" => format!("{}", num),
        "chapter_numeric" => format!("Chapter {}", num),
        "word" => word.to_string(),
        "chapter_word" => format!("Chapter {}", word),
        "roman" => to_roman(num),
        "chapter_roman" => format!("Chapter {}", to_roman(num)),
        _ => format!("Chapter {}", num),
    }
}

fn format_chapter_number_compound(num: usize, tens: &str, format: &str) -> String {
    let ones = num % 10;
    let ones_word = match ones {
        1 => "-One", 2 => "-Two", 3 => "-Three", 4 => "-Four", 5 => "-Five",
        6 => "-Six", 7 => "-Seven", 8 => "-Eight", 9 => "-Nine",
        _ => "",
    };
    let word = format!("{}{}", tens, ones_word);
    match format {
        "word" => word,
        "chapter_word" => format!("Chapter {}", word),
        "numeric" => format!("{}", num),
        "chapter_numeric" => format!("Chapter {}", num),
        "roman" => to_roman(num),
        "chapter_roman" => format!("Chapter {}", to_roman(num)),
        _ => format!("Chapter {}", num),
    }
}

fn to_roman(mut n: usize) -> String {
    let vals = [(1000,"M"),(900,"CM"),(500,"D"),(400,"CD"),(100,"C"),(90,"XC"),
                (50,"L"),(40,"XL"),(10,"X"),(9,"IX"),(5,"V"),(4,"IV"),(1,"I")];
    let mut s = String::new();
    for (val, sym) in &vals {
        while n >= *val { s.push_str(sym); n -= val; }
    }
    s
}

/// Emit a styled heading element (number, title, or subtitle).
fn emit_heading_element(
    out: &mut String,
    settings: &serde_json::Map<String, serde_json::Value>,
    prefix: &str,
    content: &str,
) {
    let font = settings.get(&format!("{}_font", prefix)).and_then(|v| v.as_str()).unwrap_or("");
    let size = settings.get(&format!("{}_size_pt", prefix)).and_then(|v| v.as_f64()).unwrap_or(16.0);
    let text_style = settings.get(&format!("{}_style", prefix)).and_then(|v| v.as_str()).unwrap_or("regular");
    let tracking = settings.get(&format!("{}_tracking_em", prefix)).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let align = settings.get(&format!("{}_align", prefix)).and_then(|v| v.as_str()).unwrap_or("center");

    // Use a content block with set rules for robust styling
    out.push_str(&format!("#align({})[\n", align));
    out.push_str("  #{\n");
    out.push_str(&format!("    set text(size: {}pt)\n", size));
    if !font.is_empty() {
        out.push_str(&format!("    set text(font: \"{}\")\n", escape_typst(font)));
    }
    if text_style == "bold" {
        out.push_str("    set text(weight: \"bold\")\n");
    }
    if text_style == "italic" {
        out.push_str("    set text(style: \"italic\")\n");
    }
    if tracking > 0.0 {
        out.push_str(&format!("    set text(tracking: {}em)\n", tracking));
    }

    // Apply wrapper transforms and emit the content
    match text_style {
        "smallcaps" => out.push_str(&format!("    smallcaps[{}]\n", content)),
        "uppercase" => out.push_str(&format!("    upper[{}]\n", content)),
        _ => out.push_str(&format!("    [{}]\n", content)),
    }
    out.push_str("  }\n");
    out.push_str("]\n");
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
fn pm_json_to_typst(
    json_str: &str,
    images: &mut ImageMap,
    conn: &Connection,
    preview_quality: bool,
) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    if !value.is_object() {
        return None;
    }
    let mut out = String::new();
    if let Some(content) = value.get("content").and_then(|c| c.as_array()) {
        for node in content {
            convert_pm_node(node, &mut out, images, conn, preview_quality);
        }
    }
    Some(out)
}

fn convert_pm_node(
    node: &serde_json::Value,
    out: &mut String,
    images: &mut ImageMap,
    conn: &Connection,
    preview_quality: bool,
) {
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
            // Post-migration: image nodes carry an integer `imageId` attr.
            let image_id = attrs.and_then(|a| a.get("imageId")).and_then(|v| v.as_i64());
            let width = attrs
                .and_then(|a| a.get("width"))
                .and_then(|w| w.as_str());
            if let Some(id) = image_id {
                let uri = format!("iwe-image://{}", id);
                if let Some((path, _ext)) = ingest_image(&uri, images, conn, preview_quality) {
                    let width_attr = match width {
                        Some(w) if !w.is_empty() => format!(", width: {}", normalize_width(w)),
                        _ => String::new(),
                    };
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
                    convert_pm_node(child, out, images, conn, preview_quality);
                }
            }
        }
    }
}

/// Resolve an `iwe-image://{id}` URI (or its localhost http form) to a Typst
/// virtual path, fetching the BLOB from `image_blobs` by id and registering
/// the bytes in the shared `ImageMap`. Returns (typst_path, extension).
///
/// Non-URI sources (legacy data: URLs, remote http:// images, etc.) are
/// ignored — all image references in this codebase go through the
/// `image_blobs` table after the migration.
fn ingest_image(
    src: &str,
    images: &mut ImageMap,
    conn: &Connection,
    preview_quality: bool,
) -> Option<(String, String)> {
    let id = parse_iwe_image_uri(src)?;

    // Preview mode reads the pre-generated downscaled blob (created at
    // upload time in `images::upload_image`), falling back to the original
    // when no preview was stored. Full-quality mode always uses the original.
    let (bytes, mime) = if preview_quality {
        crate::images::get_image_preview_or_original(conn, id).ok().flatten()?
    } else {
        crate::images::get_image_blob(conn, id).ok().flatten()?
    };

    let ext = ext_for_mime(&mime);
    let hash = simple_hash(&bytes);
    let path = format!("/iwe-img-{:x}.{}", hash, ext);
    images.entry(path.clone()).or_insert(bytes);
    Some((path, ext.to_string()))
}

fn parse_iwe_image_uri(src: &str) -> Option<i64> {
    let rest = src
        .strip_prefix("iwe-image://")
        .or_else(|| src.strip_prefix("http://iwe-image.localhost/"))
        .or_else(|| src.strip_prefix("https://iwe-image.localhost/"))?;
    // Allow an optional trailing slash or query string
    let id_str = rest.split(|c: char| c == '/' || c == '?' || c == '#').next()?;
    id_str.parse().ok()
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "png",
    }
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
            "bold" | "strong" => s = format!("#strong[{}]", s),
            "italic" | "em" => s = format!("#emph[{}]", s),
            "underline" => s = format!("#underline[{}]", s),
            "strike" | "s" => s = format!("#strike[{}]", s),
            "superscript" => s = format!("#super[{}]", s),
            "subscript" => s = format!("#sub[{}]", s),
            "textStyle" => {
                let attrs = mark.get("attrs");
                if let Some(size) = attrs
                    .and_then(|a| a.get("fontSize"))
                    .and_then(|f| f.as_str())
                    .filter(|s| !s.is_empty())
                {
                    font_size = Some(size.to_string());
                }
                if let Some(family) = attrs
                    .and_then(|a| a.get("fontFamily"))
                    .and_then(|f| f.as_str())
                    .filter(|s| !s.is_empty())
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
        if let Some(ref family) = font_family {
            let name = extract_font_name(family);
            if !name.is_empty() {
                args.push(format!("font: \"{}\"", escape_typst(name)));
            }
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
fn render_page_content(
    content: &str,
    images: &mut ImageMap,
    conn: &Connection,
    preview_quality: bool,
) -> String {
    if content.trim().is_empty() {
        return String::new();
    }
    // Try ProseMirror JSON first
    if content.trim_start().starts_with('{') {
        if let Some(typst) = pm_json_to_typst(content, images, conn, preview_quality) {
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

// ---- Formatted span → Typst utilities (for chapter content with inline marks) ----

use crate::ydoc::FmtSpan;

/// Render a single FmtSpan to Typst markup with inline formatting.
fn apply_fmt_span(span: &FmtSpan) -> String {
    // hardBreak spans contain raw Typst markup — pass through without escaping
    if span.text == " \\\n" {
        return span.text.clone();
    }

    let mut s = escape_typst(&span.text);
    if span.bold { s = format!("#strong[{}]", s); }
    if span.italic { s = format!("#emph[{}]", s); }
    if span.underline { s = format!("#underline[{}]", s); }
    if span.strike { s = format!("#strike[{}]", s); }
    if span.superscript { s = format!("#super[{}]", s); }
    if span.subscript { s = format!("#sub[{}]", s); }

    if span.font_size.is_some() || span.font_family.is_some() {
        let mut args: Vec<String> = Vec::new();
        if let Some(ref family) = span.font_family {
            let name = extract_font_name(family);
            if !name.is_empty() {
                args.push(format!("font: \"{}\"", escape_typst(&name)));
            }
        }
        if let Some(ref size) = span.font_size {
            if !size.is_empty() {
                args.push(format!("size: {}", size));
            }
        }
        if !args.is_empty() {
            s = format!("#text({})[{}]", args.join(", "), s);
        }
    }
    s
}

/// Render a slice of FmtSpans to a Typst inline string.
fn spans_to_typst(spans: &[FmtSpan]) -> String {
    let mut out = String::new();
    for span in spans {
        out.push_str(&apply_fmt_span(span));
    }
    out
}

/// Extract plain text from spans (for logic that needs raw text: drop cap detection, etc.)
fn spans_plain_text(spans: &[FmtSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Split a span list at a character offset. If the offset falls mid-span, that span is
/// split into two. Returns (before, after) where before contains exactly `char_idx` chars.
fn split_spans_at(spans: &[FmtSpan], char_idx: usize) -> (Vec<FmtSpan>, Vec<FmtSpan>) {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut consumed = 0usize;
    let mut split_done = false;

    for span in spans {
        if split_done {
            after.push(span.clone());
            continue;
        }
        let len = span.text.chars().count();
        if consumed + len <= char_idx {
            before.push(span.clone());
            consumed += len;
        } else {
            // This span straddles the split point
            let split_at = char_idx - consumed;
            let (left, right) = split_span_text(span, split_at);
            if !left.text.is_empty() {
                before.push(left);
            }
            if !right.text.is_empty() {
                after.push(right);
            }
            split_done = true;
        }
    }
    (before, after)
}

/// Split a single span at a character offset, preserving marks on both halves.
fn split_span_text(span: &FmtSpan, at_char: usize) -> (FmtSpan, FmtSpan) {
    let text: Vec<char> = span.text.chars().collect();
    let left_text: String = text[..at_char].iter().collect();
    let right_text: String = text[at_char..].iter().collect();
    let mut left = span.clone();
    let mut right = span.clone();
    left.text = left_text;
    right.text = right_text;
    (left, right)
}

/// Parsed TOC page settings.
struct TocSettings {
    title: String,
    leader_style: String,
    title_font: String,       // empty = use body font
    item_spacing_em: f64,
    vertical_align: String,   // "top" or "center"
}

/// Parse TOC page settings from the content column JSON.
fn parse_toc_settings(content: &str) -> TocSettings {
    let trimmed = content.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return TocSettings::default();
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        TocSettings {
            title: val.get("toc_title").and_then(|v| v.as_str()).unwrap_or("Contents").to_string(),
            leader_style: val.get("leader_style").and_then(|v| v.as_str()).unwrap_or("dots").to_string(),
            title_font: val.get("title_font").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            item_spacing_em: val.get("item_spacing_em").and_then(|v| v.as_f64()).unwrap_or(0.5),
            vertical_align: val.get("vertical_align").and_then(|v| v.as_str()).unwrap_or("top").to_string(),
        }
    } else {
        TocSettings::default()
    }
}

impl Default for TocSettings {
    fn default() -> Self {
        TocSettings {
            title: "Contents".to_string(),
            leader_style: "dots".to_string(),
            title_font: String::new(),
            item_spacing_em: 0.5,
            vertical_align: "top".to_string(),
        }
    }
}

/// Build Typst markup for a map page. Returns None if no image is set.
/// For single pages, renders a full-bleed image with zero margins.
/// For spreads, emits two full-bleed pages — left half then right half —
/// using Typst clipping (no Rust-side image manipulation needed).
fn build_map_page(
    content: &str,
    images: &mut ImageMap,
    conn: &Connection,
    preview_quality: bool,
) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return None;
    }
    let val: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let image_id = val.get("image_id").and_then(|v| v.as_i64())?;
    let spread = val.get("spread").and_then(|v| v.as_bool()).unwrap_or(false);
    let sizing = val.get("sizing").and_then(|v| v.as_str()).unwrap_or("fit-width");
    let gutter_overlap_in = val.get("gutter_overlap_in").and_then(|v| v.as_f64()).unwrap_or(0.0);

    let (img_path, _ext) = ingest_image(&format!("iwe-image://{}", image_id), images, conn, preview_quality)?;

    let mut out = String::new();

    // Image sizing: fit-width fills the page width (clips top/bottom centered),
    // fit-height fills the page height (clips left/right centered).
    // We use align(center + horizon) so the clipped axis is always centered.
    let (img_width, img_height) = match sizing {
        "fit-height" => ("auto".to_string(), "100%".to_string()),
        _ => ("100%".to_string(), "auto".to_string()), // fit-width default
    };

    if spread {
        // Two full-bleed pages with gutter overlap compensation.
        // Each half extends past center by `gutter_overlap_in` so the overlapping
        // strip is duplicated on both pages — the binding fold hides it and the
        // image appears seamless when the book is opened.
        //
        // Without overlap: left shows 0%–50%, right shows 50%–100%.
        // With overlap O: left shows 0%–(50%+O), right shows (50%-O)–100%.
        //
        // In Typst terms: the image is sized to (200% + 2*overlap) of page width
        // so it's slightly wider than two pages. The left page is left-aligned
        // (no shift needed — the extra width extends rightward past the clip).
        // The right page shifts left by (100% - overlap) to start from (50%-O).
        let overlap = gutter_overlap_in.max(0.0);

        let spread_img_w = match sizing {
            "fit-height" => "auto".to_string(),
            _ => format!("200% + {}in", overlap * 2.0),
        };

        // Left page: image left-aligned, extends overlap past the right edge (clipped)
        out.push_str("#page(margin: 0pt)[\n");
        out.push_str(&format!(
            "  #box(clip: true, width: 100%, height: 100%)[#align(left + horizon)[#image(\"{}\", width: {}, height: {})]]\n",
            img_path, spread_img_w, img_height
        ));
        out.push_str("]\n");

        // Right page: shift image left by (page width - overlap) so the overlap
        // region from center is duplicated on this page.
        out.push_str("#page(margin: 0pt)[\n");
        out.push_str(&format!(
            "  #box(clip: true, width: 100%, height: 100%)[#align(left + horizon)[#move(dx: -100% + {}in)[#image(\"{}\", width: {}, height: {})]]]\n",
            overlap, img_path, spread_img_w, img_height
        ));
        out.push_str("]\n");
    } else {
        // Single full-bleed page — center on both axes, clip the overflow
        out.push_str("#page(margin: 0pt)[\n");
        out.push_str(&format!(
            "  #box(clip: true, width: 100%, height: 100%)[#align(center + horizon)[#image(\"{}\", width: {}, height: {})]]\n",
            img_path, img_width, img_height
        ));
        out.push_str("]\n");
    }

    Some(out)
}

/// Build a Typst markup string for a front/back matter page.
/// If the page has rich content (PM JSON), render it directly.
/// Otherwise apply role-based defaults using the title.
fn build_front_matter_page(
    page: &FormatPage,
    images: &mut ImageMap,
    body_font: &str,
    body_size_pt: f64,
    conn: &Connection,
    preview_quality: bool,
) -> String {
    let mut out = String::new();

    // TOC pages store settings JSON in content, not ProseMirror — handle separately.
    // In Typst 0.14, `fill` is on `outline.entry`, not `outline` itself.
    if page.page_role == "toc" {
        let toc = parse_toc_settings(&page.content);

        // Vertical centering: #v(1fr) gutters above and below the content block
        if toc.vertical_align == "center" {
            out.push_str("#v(1fr)\n");
        }

        // Title — rendered manually because the global show rule
        // `heading.where(level: 1): it => {}` suppresses all H1 headings,
        // including the one Typst generates for `outline(title: ...)`.
        let title_markup = escape_typst(&toc.title);
        let title_font = if toc.title_font.is_empty() { body_font } else { &toc.title_font };
        out.push_str(&format!(
            "#align(center)[#text(font: \"{}\", size: 1.4em, weight: \"bold\")[{}]]\n#v(1em)\n",
            escape_typst(title_font), title_markup
        ));

        // Items use the body font at body size
        out.push_str(&format!(
            "#set text(font: \"{}\", size: {}pt)\n",
            escape_typst(body_font), body_size_pt
        ));

        // Entry spacing via a show rule — #set par(spacing) doesn't propagate
        // into outline entries, so we pad each entry with vertical space.
        out.push_str(&format!(
            "#show outline.entry: it => {{ v({}em); it }}\n",
            toc.item_spacing_em
        ));

        // Leader style
        match toc.leader_style.as_str() {
            "dashes" => {
                out.push_str("#set outline.entry(fill: repeat[–])\n");
            }
            "none" => {
                out.push_str("#set outline.entry(fill: none)\n");
            }
            _ => {} // dots is the Typst default
        }

        out.push_str("#outline(title: none, depth: 1)\n");

        if toc.vertical_align == "center" {
            out.push_str("#v(1fr)\n");
        }

        return out;
    }

    // Map pages: full-bleed image, optionally split across a two-page spread.
    if page.page_role == "map" {
        if let Some(map_markup) = build_map_page(&page.content, images, conn, preview_quality) {
            return map_markup;
        }
        // No image — fall through to blank page
        return out;
    }

    let content_typst = render_page_content(&page.content, images, conn, preview_quality);
    let has_content = !content_typst.trim().is_empty();

    // If user has authored content, render it directly without role-based wrapping.
    // Wrap in a block that fills the page so #v(1fr) gutters can expand for alignment.
    if has_content {
        out.push_str("#block(width: 100%, height: 100%)[\n");
        out.push_str(&format!(
            "#set text(font: \"{}\", size: {}pt, hyphenate: false)\n",
            escape_typst(body_font), body_size_pt,
        ));
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

    // Empty free-form page = blank page.

    out
}

/// Build complete Typst markup for the entire book.
/// Returns the markup string, section ID list, and image map for embedded images.
fn build_typst_markup(
    profile: &FormatProfile,
    front_pages: &[FormatPage],
    chapters: &[(i64, String, String, Option<i64>, Vec<crate::ydoc::ChapterBlock>)], // (chapter_id, title, subtitle, chapter_image_id, blocks)
    back_pages: &[FormatPage],
    book_title: &str,
    author_name: &str,
    series_name: &str,
    series_number: &str,
    conn: &Connection,
    preview_quality: bool,
) -> (String, Vec<String>, ImageMap) {
    let mut doc = String::new();
    let mut section_ids: Vec<String> = Vec::new();
    let mut images: ImageMap = std::collections::HashMap::new();

    // Book metadata as Typst variables — referenced by header/footer slots
    doc.push_str(&format!(
        "#let iwe-book-title = [{}]\n#let iwe-author-name = [{}]\n#let iwe-series-name = [{}]\n#let iwe-series-number = [{}]\n\n",
        escape_typst(book_title),
        escape_typst(author_name),
        escape_typst(series_name),
        escape_typst(series_number),
    ));

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

    // Bleed: extra area beyond the trim for edge-to-edge printing.
    // When enabled, page dimensions grow by 2*bleed and all margins increase by bleed
    // so content stays at the same position relative to the trim line.
    let bleed_enabled = print_layout
        .get("bleed_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let bleed_in = if bleed_enabled {
        print_layout
            .get("bleed_in")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.125)
    } else {
        0.0
    };

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
    let break_image_src: String = breaks
        .get("image_id")
        .and_then(|v| v.as_i64())
        .map(|id| format!("iwe-image://{}", id))
        .unwrap_or_default();
    let break_image_width_pct = breaks
        .get("image_width_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(25.0);

    // Page setup is deferred until after header/footer analysis (see below)

    // Header/footer: read slot configuration from header_footer_json
    let hf = parse_category_json(&profile.header_footer_json);
    let hf_slots: serde_json::Map<String, serde_json::Value> = hf
        .get("slots")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let hf_suppress = hf
        .get("suppress_on_chapter_start")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let hf_suppress_header_on_pages = hf
        .get("suppress_header_on_pages")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let hf_suppress_footer_on_pages = hf
        .get("suppress_footer_on_pages")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let hf_header_sep = hf
        .get("header_separator")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let hf_footer_sep = hf
        .get("footer_separator")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let hf_sep_pt = hf
        .get("separator_thickness_pt")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let hf_margin_left = hf
        .get("margin_left_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let hf_margin_right = hf
        .get("margin_right_in")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let extend_no_header = hf
        .get("extend_no_header")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let extend_no_footer = hf
        .get("extend_no_footer")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let has_any_header = ["verso_header_left","verso_header_center","verso_header_right",
        "recto_header_left","recto_header_center","recto_header_right"]
        .iter().any(|k| slot_has_content(&hf_slots, k));
    let has_any_footer = ["verso_footer_left","verso_footer_center","verso_footer_right",
        "recto_footer_left","recto_footer_center","recto_footer_right"]
        .iter().any(|k| slot_has_content(&hf_slots, k));
    let uses_page_numbers = SLOTS.iter().any(|k| {
        hf_slots.get(*k)
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            == Some("page_number")
    });

    // Page setup — margins may be reduced when extend_no_header/footer is on
    let effective_margin_top = if extend_no_header && !has_any_header {
        (margin_top * 0.6).max(0.375)
    } else {
        margin_top
    };
    let effective_margin_bottom = if extend_no_footer && !has_any_footer {
        (margin_bottom * 0.6).max(0.375)
    } else {
        margin_bottom
    };

    // Apply bleed: the inside edge (spine) is never trimmed — only the outside,
    // top, and bottom edges are cut. So bleed adds to those three sides only.
    // Width grows by one bleed (outside only), height grows by two (top + bottom).
    let page_width = profile.trim_width_in + bleed_in;
    let page_height = profile.trim_height_in + 2.0 * bleed_in;

    doc.push_str(&format!(
        "#set page(\n  width: {}in,\n  height: {}in,\n  margin: (top: {}in, bottom: {}in, outside: {}in, inside: {}in),\n)\n\n",
        page_width,
        page_height,
        effective_margin_top + bleed_in,
        effective_margin_bottom + bleed_in,
        margin_outside + bleed_in,
        margin_inside, // no bleed on the spine side
    ));

    // Suppress-on-chapter-start: query all H1 heading pages and skip the
    // header/footer when the current page contains a chapter opening.
    let suppress_open = "  let ch-pages = query(heading.where(level: 1)).map(h => h.location().page())\n  if not ch-pages.contains(here().page()) {\n";
    let suppress_close = "  }\n";

    if has_any_header {
        doc.push_str("#set page(header: context {\n");
        if hf_suppress {
            doc.push_str(suppress_open);
        }
        emit_hf_bar(&mut doc, &hf_slots, "header", hf_header_sep, hf_sep_pt, hf_margin_left, hf_margin_right);
        if hf_suppress {
            doc.push_str(suppress_close);
        }
        doc.push_str("})\n\n");
    }

    if has_any_footer {
        doc.push_str("#set page(footer: context {\n");
        if hf_suppress {
            doc.push_str(suppress_open);
        }
        emit_hf_bar(&mut doc, &hf_slots, "footer", hf_footer_sep, hf_sep_pt, hf_margin_left, hf_margin_right);
        if hf_suppress {
            doc.push_str(suppress_close);
        }
        doc.push_str("})\n\n");
    } else if uses_page_numbers {
        // No custom footer slots, but numbering is active (for header slots).
        // Typst's built-in behavior adds a default centered page number when
        // numbering is set — suppress it explicitly.
        doc.push_str("#set page(footer: none)\n\n");
    }

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

    // Paragraph settings from paragraph_json
    let para = parse_category_json(&profile.paragraph_json);
    let p_drop_enabled = para.get("drop_cap_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let p_drop_lines = para.get("drop_cap_lines").and_then(|v| v.as_i64()).unwrap_or(2) as usize;
    let p_drop_font = para.get("drop_cap_font").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let p_drop_color = para.get("drop_cap_color").and_then(|v| v.as_str()).unwrap_or("#000000").to_string();
    let p_drop_quote = para.get("drop_cap_quote_mode").and_then(|v| v.as_str()).unwrap_or("letter_only").to_string();
    let p_drop_fill_pct = para.get("drop_cap_fill_pct").and_then(|v| v.as_f64()).unwrap_or(100.0);
    let p_sc_enabled = para.get("small_caps_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let p_sc_words = para.get("small_caps_words").and_then(|v| v.as_i64()).unwrap_or(5) as i32;
    let p_apply_when = "chapter".to_string(); // always chapter-start only
    let p_style = para.get("paragraph_style").and_then(|v| v.as_str()).unwrap_or("indented").to_string();
    let p_indent_em = para.get("indent_em").and_then(|v| v.as_f64()).unwrap_or(1.5);
    let p_spacing_em = para.get("spacing_em").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let p_prevent_widows = para.get("prevent_widows").and_then(|v| v.as_bool()).unwrap_or(true);
    let p_prevent_orphans = para.get("prevent_orphans").and_then(|v| v.as_bool()).unwrap_or(true);

    // Build the first-line-indent and spacing values based on paragraph style.
    // For "indented", the indent signals a new paragraph — no extra vertical gap needed,
    // lines flow at uniform spacing throughout. Typst's par `spacing` controls the gap
    // between paragraphs; setting it equal to `leading` makes it seamless.
    let body_indent_em = if p_style == "indented" || p_style == "both" { p_indent_em } else { 0.0 };
    let body_spacing_em = match p_style.as_str() {
        "spaced" | "both" => p_spacing_em,
        _ => body_leading_em, // matches line spacing — uniform vertical rhythm
    };

    // Document-wide paragraph settings (chapter body only — wrappers reset for format pages).
    doc.push_str(&format!("#set par(first-line-indent: {}em, spacing: {}em)\n", body_indent_em, body_spacing_em));

    // Widow/orphan control via Typst's cost system.
    // Default 100% = prevent widows/orphans. 0% = allow them.
    let widow_cost = if p_prevent_widows { "100%" } else { "0%" };
    let orphan_cost = if p_prevent_orphans { "100%" } else { "0%" };
    doc.push_str(&format!(
        "#set text(costs: (widow: {}, orphan: {}))\n\n",
        widow_cost, orphan_cost,
    ));

    // Helper to emit an invisible labeled anchor for a section.
    // We use #metadata wrapped with a label so the introspector can find it.
    let emit_anchor = |out: &mut String, id: &str| {
        out.push_str(&format!("#[#metadata(none) <{}>]\n", id));
    };

    let front_numbering = if uses_page_numbers { "\"i\"" } else { "none" };
    let body_numbering = if uses_page_numbers { "\"1\"" } else { "none" };

    // ---- Front matter ----
    if !front_pages.is_empty() {
        doc.push_str(&format!("#set page(numbering: {})\n", front_numbering));
        if uses_page_numbers {
            doc.push_str("#counter(page).update(1)\n");
        }
        // Suppress header/footer on custom pages if configured
        if hf_suppress_header_on_pages && has_any_header {
            doc.push_str("#set page(header: none)\n");
        }
        if hf_suppress_footer_on_pages && (has_any_footer || uses_page_numbers) {
            doc.push_str("#set page(footer: none)\n");
        }
        doc.push('\n');

        for page in front_pages {
            let sid = format!("iwe-fp-{}", page.id);
            emit_anchor(&mut doc, &sid);
            section_ids.push(sid);
            doc.push_str(&build_front_matter_page(page, &mut images, &body_font, body_size_pt, conn, preview_quality));
            doc.push_str("#pagebreak()\n\n");
        }
    }

    // ---- Body (chapters) — restore header/footer if suppressed on front matter ----
    doc.push_str(&format!("#set page(numbering: {})\n", body_numbering));
    if uses_page_numbers {
        doc.push_str("#counter(page).update(1)\n");
    }
    // Restore header if it was suppressed on front matter pages
    if hf_suppress_header_on_pages && has_any_header {
        doc.push_str("#set page(header: context {\n");
        if hf_suppress {
            doc.push_str(suppress_open);
        }
        emit_hf_bar(&mut doc, &hf_slots, "header", hf_header_sep, hf_sep_pt, hf_margin_left, hf_margin_right);
        if hf_suppress {
            doc.push_str(suppress_close);
        }
        doc.push_str("})\n");
    }
    // Restore footer if it was suppressed on front matter pages
    if hf_suppress_footer_on_pages {
        if has_any_footer {
            doc.push_str("#set page(footer: context {\n");
            if hf_suppress {
                doc.push_str(suppress_open);
            }
            emit_hf_bar(&mut doc, &hf_slots, "footer", hf_footer_sep, hf_sep_pt, hf_margin_left, hf_margin_right);
            if hf_suppress {
                doc.push_str(suppress_close);
            }
            doc.push_str("})\n");
        } else if uses_page_numbers {
            doc.push_str("#set page(footer: none)\n");
        }
    }
    doc.push('\n');

    // Chapter heading settings — parsed here so it's in scope for the chapter loop
    let ch_head = parse_category_json(&profile.chapter_headings_json);

    // Chapter image settings
    let img_enabled = ch_head.get("image_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let img_individual = ch_head.get("image_individual").and_then(|v| v.as_bool()).unwrap_or(true);
    let img_default_id = ch_head.get("image_default_id").and_then(|v| v.as_i64());
    let img_position = ch_head.get("image_position").and_then(|v| v.as_str()).unwrap_or("below_heading").to_string();
    let img_width_pct = ch_head.get("image_width_pct").and_then(|v| v.as_f64()).unwrap_or(50.0);
    let img_align = ch_head.get("image_align").and_then(|v| v.as_str()).unwrap_or("center").to_string();
    let img_light_text = ch_head.get("image_light_text").and_then(|v| v.as_bool()).unwrap_or(false);

    for (i, (chapter_id, title, subtitle, chapter_image_id, blocks)) in chapters.iter().enumerate() {
        // Page break / start behavior
        if i > 0 {
            let start_on = ch_head.get("start_on").and_then(|v| v.as_str()).unwrap_or("any");
            match start_on {
                "recto" => doc.push_str("#pagebreak(to: \"odd\")\n\n"),
                "verso" => doc.push_str("#pagebreak(to: \"even\")\n\n"),
                _ => doc.push_str("#pagebreak()\n\n"),
            }
        }

        let sid = format!("iwe-ch-{}", chapter_id);
        emit_anchor(&mut doc, &sid);
        section_ids.push(sid);

        let escaped_title = escape_typst(title);

        // Resolve which image id to use for this chapter — per-chapter override
        // when enabled, else the profile default.
        let ch_img_id: Option<i64> = if img_enabled {
            if img_individual { *chapter_image_id } else { img_default_id }
        } else {
            None
        };
        let ch_img_uri: String = ch_img_id
            .map(|id| format!("iwe-image://{}", id))
            .unwrap_or_default();

        // Helper: emit the chapter image at a given position
        let emit_img = |doc: &mut String, images: &mut ImageMap, pos: &str| -> bool {
            if ch_img_uri.is_empty() || img_position != pos {
                return false;
            }
            if let Some((path, _ext)) = ingest_image(&ch_img_uri, images, conn, preview_quality) {
                let w = img_width_pct.clamp(1.0, 100.0);
                doc.push_str(&format!("#align({})[#image(\"{}\", width: {}%)]\n#v(0.5em)\n",
                    img_align, path, w));
                return true;
            }
            false
        };

        // Resolve cover image path early (needed before the heading block)
        let cover_img_path: Option<String> = if !ch_img_uri.is_empty() &&
            (img_position == "dedicated_page" || img_position == "cover_heading") {
            ingest_image(&ch_img_uri, &mut images, conn, preview_quality).map(|(path, _)| path)
        } else {
            None
        };

        if let Some(ref path) = cover_img_path {
            if img_position == "dedicated_page" {
                // Dedicated image page: a full-bleed page with JUST the image, no text.
                // The hidden H1 heading is emitted AFTER this so it lands on the
                // content page — this makes the TOC page number correct and lets
                // the header/footer suppress logic detect this page as a chapter start.
                // Use the full page dimensions (trim + bleed) so the image fills edge to edge.
                doc.push_str(&format!(
                    "#page(margin: 0pt)[#image(\"{}\", width: {}in, height: {}in, fit: \"cover\")]\n\n",
                    path, page_width, page_height,
                ));
                // Hidden heading for TOC/outline — now on the content page
                doc.push_str(&format!("#heading(level: 1, outlined: true)[{}]\n", escaped_title));
            } else {
                // Cover heading area — image takes real space, text overlaid on top
                doc.push_str(&format!("#heading(level: 1, outlined: true)[{}]\n", escaped_title));
                doc.push_str(&format!(
                    "#block(width: 100%, clip: false)[\n  #align({})[#image(\"{}\", width: {}%)]\n",
                    img_align, path, img_width_pct.clamp(1.0, 200.0),
                ));
                doc.push_str("  #place(top + center)[\n");
                if img_light_text {
                    doc.push_str("    #set text(fill: white)\n");
                }
            }
        } else {
            // No cover image — heading on the same page as content
            doc.push_str(&format!("#heading(level: 1, outlined: true)[{}]\n", escaped_title));
        }

        // Chapter sink
        let sink = ch_head.get("sink_em").and_then(|v| v.as_f64()).unwrap_or(6.0);
        if sink > 0.0 {
            doc.push_str(&format!("#v({}em)\n", sink));
        }

        // Rule above
        if ch_head.get("rule_above").and_then(|v| v.as_bool()).unwrap_or(false) {
            let t = ch_head.get("rule_thickness_pt").and_then(|v| v.as_f64()).unwrap_or(0.5);
            doc.push_str(&format!("#line(length: 100%, stroke: {}pt)\n#v(0.5em)\n", t));
        }

        // Image: above number
        emit_img(&mut doc, &mut images, "above_number");

        // Chapter number
        if ch_head.get("number_enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            let num_format = ch_head.get("number_format").and_then(|v| v.as_str()).unwrap_or("chapter_numeric");
            if num_format != "none" {
                let number_text = format_chapter_number(i + 1, num_format);
                emit_heading_element(&mut doc, &ch_head, "number", &number_text);
                let space = ch_head.get("space_number_title_em").and_then(|v| v.as_f64()).unwrap_or(1.5);
                doc.push_str(&format!("#v({}em)\n", space));
            }
        }

        // Image: between number and title
        emit_img(&mut doc, &mut images, "between_number_title");

        // Chapter title
        if ch_head.get("title_enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            emit_heading_element(&mut doc, &ch_head, "title", &escaped_title);
        }

        // Image: between title and subtitle
        emit_img(&mut doc, &mut images, "between_title_subtitle");

        // Chapter subtitle
        if ch_head.get("subtitle_enabled").and_then(|v| v.as_bool()).unwrap_or(true) && !subtitle.is_empty() {
            let space = ch_head.get("space_title_subtitle_em").and_then(|v| v.as_f64()).unwrap_or(0.8);
            doc.push_str(&format!("#v({}em)\n", space));
            emit_heading_element(&mut doc, &ch_head, "subtitle", &escape_typst(subtitle));
        }

        // Image: below heading
        emit_img(&mut doc, &mut images, "below_heading");

        // Rule below
        if ch_head.get("rule_below").and_then(|v| v.as_bool()).unwrap_or(false) {
            let t = ch_head.get("rule_thickness_pt").and_then(|v| v.as_f64()).unwrap_or(0.5);
            doc.push_str(&format!("#v(0.5em)\n#line(length: 100%, stroke: {}pt)\n", t));
        }

        // Close cover wrappers if active
        // Close cover_heading wrappers if active
        // (dedicated_page already closed its #page() inline — nothing to close here)
        if cover_img_path.is_some() && img_position == "cover_heading" {
            doc.push_str("  ]\n"); // close #place
            doc.push_str("]\n");   // close #block
        }

        // Space after heading
        let space_after = ch_head.get("space_after_heading_em").and_then(|v| v.as_f64()).unwrap_or(3.0);
        doc.push_str(&format!("#v({}em)\n", space_after));

        // Chapter body — wrapped in a content block that scopes the body typography.
        doc.push_str("#[\n");
        doc.push_str(&format!(
            "#set text(font: \"{}\", size: {}pt, hyphenate: {})\n",
            escape_typst(&body_font),
            body_size_pt,
            hyphens,
        ));
        doc.push_str(&format!(
            "#set par(leading: {}em, first-line-indent: {}em, justify: {}, spacing: {}em)\n",
            body_leading_em,
            body_indent_em,
            justify,
            body_spacing_em,
        ));

        // Structured blocks with inline formatting preserved
        let mut next_is_opener = true; // first paragraph after chapter heading
        let mut next_no_indent = true;
        let apply_after_breaks = p_apply_when == "breaks" || p_apply_when == "both";

        for block in blocks {
            match block {
                crate::ydoc::ChapterBlock::SceneBreak => {
                    emit_scene_break(
                        &mut doc,
                        &break_style,
                        &break_custom_text,
                        break_space_above_em,
                        break_space_below_em,
                        break_keep_with_content,
                        &break_image_src,
                        break_image_width_pct,
                        &mut images,
                        conn,
                        preview_quality,
                    );
                    next_no_indent = true;
                    if apply_after_breaks {
                        next_is_opener = true;
                    }
                }
                crate::ydoc::ChapterBlock::Paragraph(spans) => {
                    let plain = spans_plain_text(spans);
                    if plain.trim().is_empty() {
                        continue;
                    }

                    if next_is_opener && (p_drop_enabled || p_sc_enabled) {
                        emit_opener_paragraph_fmt(
                            &mut doc, spans,
                            p_drop_enabled, p_drop_lines,
                            &p_drop_font, &p_drop_color, &p_drop_quote, p_drop_fill_pct,
                            p_sc_enabled, p_sc_words,
                        );
                        next_is_opener = false;
                        next_no_indent = false;
                    } else if next_no_indent {
                        doc.push_str(&format!("#par(first-line-indent: 0em)[{}]\n\n", spans_to_typst(spans)));
                        next_no_indent = false;
                        next_is_opener = false;
                    } else {
                        doc.push_str(&format!("{}\n\n", spans_to_typst(spans)));
                    }
                }
            }
        }
        doc.push_str("]\n"); // close body #[
    }

    // ---- Back matter ----
    if !back_pages.is_empty() {
        doc.push_str("#pagebreak()\n\n");
        // Switch to roman numerals for back matter page numbers
        if uses_page_numbers {
            doc.push_str("#set page(numbering: \"i\")\n");
            doc.push_str("#counter(page).update(1)\n");
        }
        // Suppress header/footer on custom pages if configured
        if hf_suppress_header_on_pages && has_any_header {
            doc.push_str("#set page(header: none)\n");
        }
        if hf_suppress_footer_on_pages && (has_any_footer || uses_page_numbers) {
            doc.push_str("#set page(footer: none)\n");
        }
        doc.push('\n');
        for page in back_pages {
            let sid = format!("iwe-fp-{}", page.id);
            emit_anchor(&mut doc, &sid);
            section_ids.push(sid);
            doc.push_str(&build_front_matter_page(page, &mut images, &body_font, body_size_pt, conn, preview_quality));
            doc.push_str("#pagebreak()\n\n");
        }
    }

    // We suppress the default H1 rendering — emitted manually in the chapter loop.
    let mut heading_style = "#show heading.where(level: 1): it => {}\n\n".to_string();

    // In-chapter heading styles (H2, H3, H4) from headings_json
    let hdg = parse_category_json(&profile.headings_json);
    let hdg_no_indent_after = hdg.get("no_indent_after").and_then(|v| v.as_bool()).unwrap_or(true);

    for (level, key) in [(2, "h2"), (3, "h3"), (4, "h4")] {
        let enabled = hdg.get(&format!("{}_enabled", key)).and_then(|v| v.as_bool()).unwrap_or(true);
        if !enabled { continue; }

        let font = hdg.get(&format!("{}_font", key)).and_then(|v| v.as_str()).unwrap_or("");
        let size = hdg.get(&format!("{}_size_pt", key)).and_then(|v| v.as_f64()).unwrap_or(if level == 2 { 16.0 } else if level == 3 { 13.0 } else { 11.0 });
        let align = hdg.get(&format!("{}_align", key)).and_then(|v| v.as_str()).unwrap_or("left");
        let text_style = hdg.get(&format!("{}_style", key)).and_then(|v| v.as_str()).unwrap_or("bold");
        let tracking = hdg.get(&format!("{}_tracking_em", key)).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let space_above = hdg.get(&format!("{}_space_above_em", key)).and_then(|v| v.as_f64()).unwrap_or(1.5);
        let space_below = hdg.get(&format!("{}_space_below_em", key)).and_then(|v| v.as_f64()).unwrap_or(0.8);
        let keep_next = hdg.get(&format!("{}_keep_with_next", key)).and_then(|v| v.as_bool()).unwrap_or(true);
        let rule_above = hdg.get(&format!("{}_rule_above", key)).and_then(|v| v.as_bool()).unwrap_or(false);
        let rule_below = hdg.get(&format!("{}_rule_below", key)).and_then(|v| v.as_bool()).unwrap_or(false);

        // Build a show rule for this heading level
        heading_style.push_str(&format!("#show heading.where(level: {}): it => {{\n", level));
        heading_style.push_str(&format!("  v({}em)\n", space_above));
        if rule_above {
            heading_style.push_str("  line(length: 100%, stroke: 0.5pt)\n  v(0.3em)\n");
        }

        // Styled text block
        heading_style.push_str("  {\n");
        heading_style.push_str(&format!("    set text(size: {}pt)\n", size));
        if !font.is_empty() {
            heading_style.push_str(&format!("    set text(font: \"{}\")\n", escape_typst(font)));
        }
        if text_style == "bold" { heading_style.push_str("    set text(weight: \"bold\")\n"); }
        if text_style == "italic" { heading_style.push_str("    set text(style: \"italic\")\n"); }
        if tracking > 0.0 { heading_style.push_str(&format!("    set text(tracking: {}em)\n", tracking)); }

        let content_expr = match text_style {
            "smallcaps" => "smallcaps(it.body)",
            "uppercase" => "upper(it.body)",
            _ => "it.body",
        };
        heading_style.push_str(&format!("    align({}, {})\n", align, content_expr));
        heading_style.push_str("  }\n");

        if rule_below {
            heading_style.push_str("  v(0.3em)\n  line(length: 100%, stroke: 0.5pt)\n");
        }
        heading_style.push_str(&format!("  v({}em)\n", space_below));

        // Keep with next via block sticky
        if keep_next {
            // Wrap the whole thing to stick with the following content
            // Actually, Typst handles this via the heading's block properties
        }

        heading_style.push_str("}\n\n");
    }

    // Dropcap function preamble (only if drop caps are enabled)
    let dc_preamble = if p_drop_enabled { dropcap_preamble() } else { String::new() };

    // Prepend preambles before the document body
    (format!("{}{}{}", dc_preamble, heading_style, doc), section_ids, images)
}

// ---- Compile & SVG render ----

use std::sync::Mutex;
use std::time::Instant;
use serde::Serialize;

fn compile_document(markup: &str, images: ImageMap, cache: &FontCache, packages: Arc<PackageStorage>) -> Result<PagedDocument, String> {
    let t_dump = Instant::now();
    // Always dump markup for debugging — helps diagnose rendering issues
    // even when compilation succeeds.
    let dump_path = std::env::temp_dir().join("iwe_typst_debug.typ");
    let _ = std::fs::write(&dump_path, markup);
    let dump_ms = t_dump.elapsed().as_secs_f64() * 1000.0;

    let image_count = images.len();
    let image_bytes: usize = images.values().map(|v| v.len()).sum();

    let t_world = Instant::now();
    let world = IweWorld::new(markup.to_string(), images, cache, packages);
    let world_ms = t_world.elapsed().as_secs_f64() * 1000.0;

    let t_compile = Instant::now();
    let warned = typst::compile::<PagedDocument>(&world);
    let compile_ms = t_compile.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "[format] compile_document: dump={:.0}ms world={:.0}ms typst::compile={:.0}ms | {} images ({:.1}MB) markup={}KB",
        dump_ms, world_ms, compile_ms,
        image_count, image_bytes as f64 / 1024.0 / 1024.0,
        markup.len() / 1024,
    );

    match warned.output {
        Ok(doc) => Ok(doc),
        Err(diagnostics) => {
            let mut errors = Vec::new();
            for diag in diagnostics {
                // Extract line/column from the span if possible
                let span_info = if let Some(id) = diag.span.id() {
                    use typst::World;
                    let source = world.source(id);
                    if let Ok(src) = source {
                        let raw_offset: usize = src.range(diag.span).map(|r| r.start).unwrap_or(0);
                        let text = src.text();
                        // Snap byte offsets to char boundaries so slicing
                        // never panics on multi-byte characters (e.g. en-dash).
                        let byte_offset = snap_to_char_boundary(text, raw_offset);
                        let before = &text[..byte_offset];
                        let line = before.matches('\n').count() + 1;
                        let col = before.rfind('\n').map(|p| byte_offset - p).unwrap_or(byte_offset + 1);
                        // Grab surrounding context
                        let ctx_start = snap_to_char_boundary(text, raw_offset.saturating_sub(100));
                        let ctx_end = snap_to_char_boundary(text, (raw_offset + 100).min(text.len()));
                        let context = &text[ctx_start..ctx_end];
                        format!(" at line {}:{} | context: ...{}...", line, col, context.replace('\n', "\\n"))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                errors.push(format!("{:?}{}", diag.message, span_info));
            }
            // Dump full markup to temp file for debugging
            let dump_path = std::env::temp_dir().join("iwe_typst_debug.typ");
            let _ = std::fs::write(&dump_path, markup);
            log::error!("[format] Typst markup dumped to {:?}", dump_path);
            Err(format!("Typst compilation failed: {}", errors.join("; ")))
        }
    }
}

// ---- Cached document state ----

pub struct FormatState {
    document: Mutex<Option<PagedDocument>>,
    /// Lazily-initialized font cache (system + embedded)
    font_cache: Mutex<Option<Arc<FontCache>>>,
    /// Package storage for resolving Typst package imports (e.g. droplet)
    packages: Arc<PackageStorage>,
}

impl FormatState {
    pub fn new() -> Self {
        // PackageStorage handles downloading and caching Typst packages.
        // Uses the system temp dir for the cache.
        let package_cache_dir = std::env::temp_dir().join("iwe-typst-packages");
        let downloader = Downloader::new("iwe/0.1.0");
        let packages = PackageStorage::new(
            Some(package_cache_dir),
            None,
            downloader,
        );
        FormatState {
            document: Mutex::new(None),
            font_cache: Mutex::new(None),
            packages: Arc::new(packages),
        }
    }

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

/// Internal compile entrypoint. Both `compile_preview` (preview_quality=true)
/// and `export_format_pdf` (preview_quality=false) call this.
fn compile_internal(
    state: &AppState,
    format_state: &FormatState,
    profile_id: i64,
    preview_quality: bool,
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

    // 2. Y.Doc extraction — structured blocks with inline formatting preserved
    let t = Instant::now();
    let mut chapters: Vec<(i64, String, String, Option<i64>, Vec<crate::ydoc::ChapterBlock>)> = Vec::new();
    for ch in &chapters_raw {
        let blocks = ydoc::extract_chapter_blocks_from_bytes(&ch.content);
        chapters.push((ch.id, ch.title.clone(), ch.subtitle.clone(), ch.chapter_image_id, blocks));
    }
    let ydoc_extract_ms = t.elapsed().as_secs_f64() * 1000.0;
    let chapter_count = chapters.len();

    let front: Vec<FormatPage> = format_pages.iter().filter(|p| p.position == "front").cloned().collect();
    let back: Vec<FormatPage> = format_pages.iter().filter(|p| p.position == "back").cloned().collect();

    // Read book metadata from project_settings
    let book_title: String = conn
        .query_row("SELECT value FROM project_settings WHERE key = 'book_title'", [], |r| r.get(0))
        .unwrap_or_default();
    let author_name: String = conn
        .query_row("SELECT value FROM project_settings WHERE key = 'author_name'", [], |r| r.get(0))
        .unwrap_or_default();
    let series_name: String = conn
        .query_row("SELECT value FROM project_settings WHERE key = 'series_name'", [], |r| r.get(0))
        .unwrap_or_default();
    let series_number: String = conn
        .query_row("SELECT value FROM project_settings WHERE key = 'series_number'", [], |r| r.get(0))
        .unwrap_or_default();

    // 3. Markup generation
    let t = Instant::now();
    let (markup, section_ids, images) = build_typst_markup(&profile, &front, &chapters, &back, &book_title, &author_name, &series_name, &series_number, conn, preview_quality);
    let markup_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let markup_len = markup.len();

    // 4. Typst compilation
    let t = Instant::now();
    let cache = format_state.get_or_init_fonts();
    let document = compile_document(&markup, images, &cache, format_state.packages.clone())?;
    let typst_compile_ms = t.elapsed().as_secs_f64() * 1000.0;

    let page_count = document.pages.len();

    // 5. Resolve section labels to page numbers
    let t = Instant::now();
    let section_pages = resolve_section_pages(&document, &section_ids);
    let section_resolve_ms = t.elapsed().as_secs_f64() * 1000.0;

    // 6. Cache the compiled document
    let t = Instant::now();
    {
        let mut doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
        *doc_guard = Some(document);
    }
    let cache_store_ms = t.elapsed().as_secs_f64() * 1000.0;

    let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "[format] compile ({}): {}ms total | db:{}ms ydoc:{}ms markup:{}ms compile:{}ms section_resolve:{}ms cache_store:{}ms | {} pages, {} chapters, {} sections resolved, {}KB markup",
        if preview_quality { "preview" } else { "full" },
        total_ms as u32, db_load_ms as u32, ydoc_extract_ms as u32, markup_build_ms as u32,
        typst_compile_ms as u32, section_resolve_ms as u32, cache_store_ms as u32,
        page_count, chapter_count, section_pages.len(), markup_len / 1024,
    );

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

/// Compile the document and cache it. Returns page count + timing. No SVGs yet.
///
/// Runs in preview quality: source images are downscaled (and cached) so the
/// Typst layout pass and per-page SVG output stay snappy. The PDF export
/// command (`export_format_pdf`) reruns this as a full-quality compile.
#[tauri::command]
pub fn compile_preview(
    state: tauri::State<'_, AppState>,
    format_state: tauri::State<'_, FormatState>,
    profile_id: i64,
) -> Result<CompileResult, String> {
    compile_internal(state.inner(), format_state.inner(), profile_id, true)
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
    let t_lock = Instant::now();
    let doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
    let document = doc_guard.as_ref().ok_or("No compiled document")?;
    let page = document.pages.get(page_index).ok_or("Page index out of range")?;
    let lock_ms = t_lock.elapsed().as_secs_f64() * 1000.0;

    let t_svg = Instant::now();
    let svg = typst_svg::svg(page);
    let svg_ms = t_svg.elapsed().as_secs_f64() * 1000.0;

    log::info!(
        "[format] render_page_svg p{}: lock={:.0}ms svg={:.0}ms ({:.0}KB)",
        page_index, lock_ms, svg_ms, svg.len() as f64 / 1024.0,
    );

    Ok(svg)
}

/// Export the compiled document as a PDF.
///
/// Recompiles at full image quality before exporting — the cached
/// `PagedDocument` is built at preview quality (downscaled images), which
/// is fine for on-screen scrolling but not for a PDF the user wants to
/// print. The full-quality recompile takes a few seconds but only happens
/// when the user clicks Export.
#[tauri::command]
pub fn export_format_pdf(
    state: tauri::State<'_, AppState>,
    format_state: tauri::State<'_, FormatState>,
    profile_id: i64,
) -> Result<Vec<u8>, String> {
    // Recompile at full quality. This replaces the cached document; the
    // next preview compile will rebuild it at preview quality.
    compile_internal(state.inner(), format_state.inner(), profile_id, false)?;

    let doc_guard = format_state.document.lock().map_err(|e| e.to_string())?;
    let document = doc_guard.as_ref().ok_or("No compiled document.")?;

    let options = typst_pdf::PdfOptions {
        ident: typst::foundations::Smart::Auto,
        timestamp: None,
        page_ranges: None,
        standards: typst_pdf::PdfStandards::default(),
        tagged: false,
    };

    let pdf_bytes = typst_pdf::pdf(document, &options)
        .map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| format!("{:?}", e.message)).collect();
            format!("PDF export failed: {}", msgs.join("; "))
        })?;

    Ok(pdf_bytes)
}
} // end mod real (cfg feature = "format")

// ---- Stubs when "format" feature is disabled ----

#[cfg(not(feature = "format"))]
mod real {
    use serde::Serialize;
    use std::sync::Mutex;

    pub struct FormatState {
        _dummy: Mutex<()>,
    }

    impl FormatState {
        pub fn new() -> Self {
            FormatState { _dummy: Mutex::new(()) }
        }
    }

    pub fn render_page_svg(_format_state: &FormatState, _page_index: usize) -> Result<String, String> {
        Err("Format features not enabled (build with --features format)".into())
    }

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
        pub section_pages: std::collections::HashMap<String, usize>,
    }

    #[tauri::command]
    pub fn compile_preview(_profile_id: i64) -> Result<CompileResult, String> {
        Err("Format features not enabled (build with --features format)".into())
    }

    #[tauri::command]
    pub fn list_system_fonts() -> Result<Vec<String>, String> {
        Err("Format features not enabled (build with --features format)".into())
    }

    #[tauri::command]
    pub fn export_format_pdf(_profile_id: i64) -> Result<Vec<u8>, String> {
        Err("Format features not enabled (build with --features format)".into())
    }
}

pub use real::*;
