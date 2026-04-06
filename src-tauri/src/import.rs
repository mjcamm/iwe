// Manuscript importer — DOCX + EPUB → flat blocks + detected chapter breaks.
//
// Returns a structure the frontend can render in the /import preview, where the
// user confirms / adjusts breaks before committing the import.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::io::Read;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportBlock {
    pub text: String,
    /// Style name from the source ("Heading1", "Title", "Normal", etc.)
    pub style: String,
    /// True if a hard page break appears immediately before this paragraph
    pub page_break_before: bool,
    /// True if this paragraph is inferred to be a chapter heading by structure
    pub is_heading: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DetectedBreak {
    /// Index into the blocks array — break sits immediately before this block.
    pub block_index: usize,
    /// "heading" | "page_break" | "pattern" | "blank_lines" | "manual"
    pub source: String,
    /// Suggested chapter title
    pub title: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ImportResult {
    pub blocks: Vec<ImportBlock>,
    pub breaks: Vec<DetectedBreak>,
    /// Which detection method produced these breaks
    pub method: String,
    /// Source format ("docx" | "epub")
    pub format: String,
}

// ---------- Public entry point ----------

#[tauri::command]
pub fn parse_import_file(path: String, method: Option<String>) -> Result<ImportResult, String> {
    let lower = path.to_lowercase();
    let format = if lower.ends_with(".docx") {
        "docx"
    } else if lower.ends_with(".epub") {
        "epub"
    } else {
        return Err("Unsupported file format. Only .docx and .epub are supported.".into());
    };

    let blocks = match format {
        "docx" => parse_docx(&path)?,
        "epub" => parse_epub(&path)?,
        _ => unreachable!(),
    };

    let method = method.unwrap_or_else(|| "auto".into());
    let (breaks, used_method) = detect_breaks(&blocks, &method);

    Ok(ImportResult {
        blocks,
        breaks,
        method: used_method,
        format: format.into(),
    })
}

// ---------- Detection ----------

fn detect_breaks(blocks: &[ImportBlock], method: &str) -> (Vec<DetectedBreak>, String) {
    // Auto picks the highest-signal method that produces a "reasonable" count.
    if method == "auto" {
        for m in &["heading", "page_break", "pattern", "blank_lines"] {
            let breaks = run_detection(blocks, m);
            if breaks.len() >= 2 && breaks.len() <= 500 {
                return (breaks, (*m).to_string());
            }
        }
        // Fallback: single chapter at the start
        return (
            vec![DetectedBreak {
                block_index: 0,
                source: "manual".into(),
                title: "Chapter 1".into(),
            }],
            "none".into(),
        );
    }

    let mut breaks = run_detection(blocks, method);
    if breaks.is_empty() && !blocks.is_empty() {
        breaks.push(DetectedBreak {
            block_index: 0,
            source: "manual".into(),
            title: "Chapter 1".into(),
        });
    }
    (breaks, method.to_string())
}

fn run_detection(blocks: &[ImportBlock], method: &str) -> Vec<DetectedBreak> {
    let mut out = Vec::new();
    match method {
        "heading" => {
            for (i, b) in blocks.iter().enumerate() {
                if b.is_heading {
                    out.push(DetectedBreak {
                        block_index: i,
                        source: "heading".into(),
                        title: trim_title(&b.text, i, &out),
                    });
                }
            }
        }
        "page_break" => {
            for (i, b) in blocks.iter().enumerate() {
                if b.page_break_before || (i == 0 && !blocks.is_empty()) {
                    let title = guess_title_at(blocks, i, &out);
                    out.push(DetectedBreak {
                        block_index: i,
                        source: if i == 0 && !b.page_break_before {
                            "manual".into()
                        } else {
                            "page_break".into()
                        },
                        title,
                    });
                }
            }
        }
        "pattern" => {
            let re = regex::Regex::new(
                r"(?i)^\s*(chapter|prologue|epilogue|part|book)\b|^\s*[ivxlcdm]+\s*$|^\s*\d{1,3}\s*$",
            )
            .unwrap();
            for (i, b) in blocks.iter().enumerate() {
                let t = b.text.trim();
                if t.is_empty() {
                    continue;
                }
                if t.len() < 80 && re.is_match(t) {
                    out.push(DetectedBreak {
                        block_index: i,
                        source: "pattern".into(),
                        title: trim_title(t, i, &out),
                    });
                }
            }
        }
        "blank_lines" => {
            // Vellum-style: 3+ consecutive empty paragraphs = chapter break
            let mut blank_run = 0usize;
            for (i, b) in blocks.iter().enumerate() {
                if b.text.trim().is_empty() {
                    blank_run += 1;
                } else {
                    if blank_run >= 3 || i == 0 {
                        out.push(DetectedBreak {
                            block_index: i,
                            source: if i == 0 && blank_run < 3 {
                                "manual".into()
                            } else {
                                "blank_lines".into()
                            },
                            title: guess_title_at(blocks, i, &out),
                        });
                    }
                    blank_run = 0;
                }
            }
        }
        _ => {}
    }
    out
}

fn guess_title_at(blocks: &[ImportBlock], i: usize, existing: &[DetectedBreak]) -> String {
    // If first non-empty block at position is short, use it as the title
    if let Some(b) = blocks.get(i) {
        let t = b.text.trim();
        if !t.is_empty() && t.len() < 80 {
            return trim_title(t, i, existing);
        }
    }
    format!("Chapter {}", existing.len() + 1)
}

fn trim_title(text: &str, _i: usize, existing: &[DetectedBreak]) -> String {
    let t = text.trim();
    if t.is_empty() {
        format!("Chapter {}", existing.len() + 1)
    } else if t.chars().count() > 80 {
        let truncated: String = t.chars().take(77).collect();
        format!("{}…", truncated)
    } else {
        t.into()
    }
}

// ---------- DOCX parser ----------

fn parse_docx(path: &str) -> Result<Vec<ImportBlock>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open failed: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("not a valid docx (zip): {e}"))?;
    let mut document_xml = String::new();
    {
        let mut entry = zip
            .by_name("word/document.xml")
            .map_err(|e| format!("docx missing word/document.xml: {e}"))?;
        entry
            .read_to_string(&mut document_xml)
            .map_err(|e| format!("read document.xml: {e}"))?;
    }
    parse_docx_xml(&document_xml)
}

fn parse_docx_xml(xml: &str) -> Result<Vec<ImportBlock>, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);
    let mut buf = Vec::new();

    let mut blocks: Vec<ImportBlock> = Vec::new();

    // Per-paragraph state
    let mut in_p = false;
    let mut cur_text = String::new();
    let mut cur_style = String::from("Normal");
    let mut cur_page_break_before = false;
    let mut pending_page_break = false;
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("xml parse error at {}: {}", reader.buffer_position(), e)),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = name.as_ref();
                if local == b"w:p" {
                    in_p = true;
                    cur_text.clear();
                    cur_style = "Normal".into();
                    cur_page_break_before = pending_page_break;
                    pending_page_break = false;
                } else if local == b"w:t" && in_p {
                    in_t = true;
                }
            }
            Ok(Event::Empty(e)) => {
                let local = e.name();
                let n = local.as_ref();
                if n == b"w:pStyle" && in_p {
                    if let Some(val) = attr_value(&e, b"w:val") {
                        cur_style = val;
                    }
                } else if n == b"w:br" {
                    if let Some(t) = attr_value(&e, b"w:type") {
                        if t == "page" {
                            if in_p {
                                // Page break inside a paragraph: mark next paragraph
                                pending_page_break = true;
                            } else {
                                pending_page_break = true;
                            }
                        }
                    }
                } else if n == b"w:lastRenderedPageBreak" {
                    // soft hint, ignore
                }
            }
            Ok(Event::Text(t)) => {
                if in_t {
                    if let Ok(s) = t.unescape() {
                        cur_text.push_str(&s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = e.name();
                let n = local.as_ref();
                if n == b"w:t" {
                    in_t = false;
                } else if n == b"w:p" && in_p {
                    let style = cur_style.clone();
                    let is_heading = is_heading_style(&style);
                    blocks.push(ImportBlock {
                        text: cur_text.trim_end().to_string(),
                        style,
                        page_break_before: cur_page_break_before,
                        is_heading,
                    });
                    in_p = false;
                    cur_text.clear();
                }
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(blocks)
}

fn attr_value(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key {
            return Some(String::from_utf8_lossy(&a.value).into_owned());
        }
    }
    None
}

fn is_heading_style(style: &str) -> bool {
    let s = style.to_lowercase();
    s.starts_with("heading") || s == "title" || s == "subtitle"
}

// ---------- EPUB parser ----------

fn parse_epub(path: &str) -> Result<Vec<ImportBlock>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open failed: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("not a valid epub (zip): {e}"))?;

    // Find the OPF (manifest) — usually content.opf or similar
    let opf_path = find_opf_path(&mut zip)?;
    let opf_dir = opf_path
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();

    let opf_xml = read_zip_text(&mut zip, &opf_path)?;
    let spine_files = parse_opf_spine(&opf_xml)?;

    let mut blocks: Vec<ImportBlock> = Vec::new();
    for file_rel in spine_files {
        let full = if opf_dir.is_empty() {
            file_rel.clone()
        } else {
            format!("{}/{}", opf_dir, file_rel)
        };
        let xhtml = match read_zip_text(&mut zip, &full) {
            Ok(s) => s,
            Err(_) => continue,
        };
        // Mark the very first block of each spine file with a page-break-before
        // so the page_break detector can use spine boundaries as candidates.
        let start = blocks.len();
        parse_xhtml_into_blocks(&xhtml, &mut blocks);
        if let Some(b) = blocks.get_mut(start) {
            b.page_break_before = true;
        }
    }

    Ok(blocks)
}

fn read_zip_text(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String, String> {
    let mut entry = zip
        .by_name(name)
        .map_err(|e| format!("missing {name}: {e}"))?;
    let mut s = String::new();
    entry.read_to_string(&mut s).map_err(|e| format!("read {name}: {e}"))?;
    Ok(s)
}

fn find_opf_path(zip: &mut zip::ZipArchive<std::fs::File>) -> Result<String, String> {
    // Read META-INF/container.xml
    let container = read_zip_text(zip, "META-INF/container.xml")?;
    // Quick scan for full-path="..."
    if let Some(idx) = container.find("full-path=") {
        let rest = &container[idx + "full-path=".len()..];
        let quote = rest.chars().next().unwrap_or('"');
        if let Some(end) = rest[1..].find(quote) {
            return Ok(rest[1..1 + end].to_string());
        }
    }
    Err("could not locate OPF in container.xml".into())
}

fn parse_opf_spine(opf_xml: &str) -> Result<Vec<String>, String> {
    // Build manifest id → href, then read spine itemref order
    let mut reader = Reader::from_str(opf_xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut manifest: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut spine_order: Vec<String> = Vec::new();
    let mut in_spine = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("opf parse: {e}")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let n = name.as_ref();
                if n == b"item" || ends_with(n, b":item") {
                    let id = attr_value(&e, b"id").unwrap_or_default();
                    let href = attr_value(&e, b"href").unwrap_or_default();
                    if !id.is_empty() && !href.is_empty() {
                        manifest.insert(id, href);
                    }
                } else if n == b"spine" || ends_with(n, b":spine") {
                    in_spine = true;
                } else if in_spine && (n == b"itemref" || ends_with(n, b":itemref")) {
                    if let Some(idref) = attr_value(&e, b"idref") {
                        spine_order.push(idref);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let n = name.as_ref();
                if n == b"spine" || ends_with(n, b":spine") {
                    in_spine = false;
                }
            }
            _ => {}
        }
        buf.clear();
    }
    let files = spine_order
        .into_iter()
        .filter_map(|id| manifest.get(&id).cloned())
        .collect();
    Ok(files)
}

fn ends_with(name: &[u8], suffix: &[u8]) -> bool {
    name.len() >= suffix.len() && &name[name.len() - suffix.len()..] == suffix
}

fn parse_xhtml_into_blocks(xhtml: &str, blocks: &mut Vec<ImportBlock>) {
    let mut reader = Reader::from_str(xhtml);
    reader.trim_text(false);
    let mut buf = Vec::new();

    // Track block-level element nesting
    let block_tags: &[&[u8]] = &[
        b"p", b"h1", b"h2", b"h3", b"h4", b"h5", b"h6", b"li", b"blockquote", b"div",
    ];

    let mut cur_text = String::new();
    let mut cur_tag: Option<String> = None;
    let mut depth = 0i32;
    let mut suppress_text = false; // inside <script>/<style>

    loop {
        match reader.read_event_into(&mut buf) {
            Err(_) => break,
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == b"script" || local == b"style" {
                    suppress_text = true;
                    continue;
                }
                if block_tags.iter().any(|t| *t == local) {
                    // Flush any accumulated inline text from previous block
                    if cur_tag.is_none() && !cur_text.trim().is_empty() {
                        push_block(blocks, &mut cur_text, "p");
                    }
                    cur_tag = Some(String::from_utf8_lossy(local).into_owned());
                    cur_text.clear();
                    depth += 1;
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == b"script" || local == b"style" {
                    suppress_text = false;
                    continue;
                }
                if block_tags.iter().any(|t| *t == local) {
                    let tag = cur_tag.take().unwrap_or_else(|| "p".into());
                    push_block(blocks, &mut cur_text, &tag);
                    depth -= 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == b"br" {
                    cur_text.push(' ');
                }
            }
            Ok(Event::Text(t)) => {
                if !suppress_text {
                    if let Ok(s) = t.unescape() {
                        cur_text.push_str(&s);
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }
    let _ = depth;
    if !cur_text.trim().is_empty() {
        let tag = cur_tag.take().unwrap_or_else(|| "p".into());
        push_block(blocks, &mut cur_text, &tag);
    }
}

fn local_name(name: &[u8]) -> &[u8] {
    if let Some(idx) = name.iter().position(|c| *c == b':') {
        &name[idx + 1..]
    } else {
        name
    }
}

fn push_block(blocks: &mut Vec<ImportBlock>, buf: &mut String, tag: &str) {
    let text: String = buf.split_whitespace().collect::<Vec<_>>().join(" ");
    buf.clear();
    if text.is_empty() {
        return;
    }
    let is_heading = matches!(tag, "h1" | "h2" | "h3");
    let style = match tag {
        "h1" => "Heading1".into(),
        "h2" => "Heading2".into(),
        "h3" => "Heading3".into(),
        "blockquote" => "Quote".into(),
        _ => "Normal".into(),
    };
    blocks.push(ImportBlock {
        text,
        style,
        page_break_before: false,
        is_heading,
    });
}

