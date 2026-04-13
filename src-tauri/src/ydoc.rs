use yrs::{Any, Doc, GetString, Out, ReadTxn, Text, Transact, Update, XmlFragment, XmlTextRef};
use yrs::types::text::{Diff, YChange};
use yrs::types::xml::{XmlFragmentRef, XmlOut, Xml};
use yrs::updates::decoder::Decode;
use std::sync::Arc;

// ---- Formatted chapter block extraction ----

/// A formatted text span: text content + inline marks.
#[derive(Debug, Clone)]
pub struct FmtSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub font_size: Option<String>,
    pub font_family: Option<String>,
}

impl FmtSpan {
    fn plain(text: String) -> Self {
        FmtSpan {
            text, bold: false, italic: false, underline: false,
            strike: false, superscript: false, subscript: false,
            font_size: None, font_family: None,
        }
    }
}

/// A block-level element extracted from a chapter Y.Doc.
#[derive(Debug, Clone)]
pub enum ChapterBlock {
    Paragraph(Vec<FmtSpan>),
    SceneBreak,
}

/// Load a Y.Doc from encoded state bytes (from SQLite BLOB).
pub fn load_doc(state_bytes: &[u8]) -> Result<Doc, String> {
    let doc = Doc::new();
    if !state_bytes.is_empty() {
        let update = Update::decode_v1(state_bytes)
            .map_err(|e| format!("Failed to decode Y.Doc state: {}", e))?;
        let mut txn = doc.transact_mut();
        txn.apply_update(update)
            .map_err(|e| format!("Failed to apply Y.Doc update: {:?}", e))?;
    }
    Ok(doc)
}

/// Extract plain text from Y.Doc's prosemirror XmlFragment.
/// Walks all children depth-first, concatenating text from XmlText nodes.
/// NO separators between blocks — matches frontend buildTextMap() behavior.
pub fn extract_plain_text(doc: &Doc) -> String {
    let txn = doc.transact();
    match txn.get_xml_fragment("prosemirror") {
        Some(fragment) => {
            let mut text = String::new();
            walk_fragment(&txn, &fragment, &mut text);
            text
        }
        None => String::new(),
    }
}

/// Extract text with paragraph breaks (for PDF/plain-text export).
/// Inserts \n\n between block-level elements.
pub fn extract_text_with_breaks(doc: &Doc) -> String {
    let txn = doc.transact();
    match txn.get_xml_fragment("prosemirror") {
        Some(fragment) => {
            let mut text = String::new();
            let mut first = true;
            for child in fragment.children(&txn) {
                if !first && !text.is_empty() {
                    text.push_str("\n\n");
                }
                first = false;
                walk_xml_out(&txn, &child, &mut text);
            }
            text
        }
        None => String::new(),
    }
}

/// Extract text for the format renderer. Same as extract_text_with_breaks but also
/// emits `* * *` scene-break sentinels for top-level horizontal rule nodes so the
/// format builder can recognise them as scene breaks. Keeps extract_plain_text
/// untouched so scanner offsets aren't affected.
pub fn extract_text_for_format_from_bytes(state_bytes: &[u8]) -> String {
    match load_doc(state_bytes) {
        Ok(doc) => extract_text_for_format(&doc),
        Err(_) => String::new(),
    }
}

fn extract_text_for_format(doc: &Doc) -> String {
    let txn = doc.transact();
    let fragment = match txn.get_xml_fragment("prosemirror") {
        Some(f) => f,
        None => return String::new(),
    };
    let mut text = String::new();
    let mut first = true;
    for child in fragment.children(&txn) {
        if !first && !text.is_empty() {
            text.push_str("\n\n");
        }
        first = false;
        // Top-level horizontal rule → emit scene break sentinel
        if let XmlOut::Element(ref el) = child {
            let tag = el.tag();
            if tag.as_ref() == "horizontalRule" {
                text.push_str("* * *");
                continue;
            }
        }
        walk_xml_out(&txn, &child, &mut text);
    }
    text
}

fn walk_fragment<T: ReadTxn>(txn: &T, fragment: &XmlFragmentRef, out: &mut String) {
    for child in fragment.children(txn) {
        walk_xml_out(txn, &child, out);
    }
}

fn walk_xml_out<T: ReadTxn>(txn: &T, node: &XmlOut, out: &mut String) {
    match node {
        XmlOut::Element(el) => {
            for inner in el.children(txn) {
                walk_xml_out(txn, &inner, out);
            }
        }
        XmlOut::Text(txt) => {
            out.push_str(&txt.get_string(txn));
        }
        XmlOut::Fragment(frag) => {
            walk_fragment(txn, frag, out);
        }
    }
}

// ---- Formatted chapter block extraction ----

/// Extract chapter content as structured blocks with inline formatting preserved.
/// Uses yrs `diff()` API to read formatting attributes from XmlText nodes.
pub fn extract_chapter_blocks_from_bytes(state_bytes: &[u8]) -> Vec<ChapterBlock> {
    match load_doc(state_bytes) {
        Ok(doc) => extract_chapter_blocks(&doc),
        Err(_) => vec![],
    }
}

fn extract_chapter_blocks(doc: &Doc) -> Vec<ChapterBlock> {
    let txn = doc.transact();
    let fragment = match txn.get_xml_fragment("prosemirror") {
        Some(f) => f,
        None => return vec![],
    };
    let mut blocks = Vec::new();
    for child in fragment.children(&txn) {
        collect_blocks_from_node(&txn, &child, &mut blocks);
    }
    blocks
}

/// Recursively collect ChapterBlocks from an XmlOut node.
fn collect_blocks_from_node<T: ReadTxn>(txn: &T, node: &XmlOut, blocks: &mut Vec<ChapterBlock>) {
    match node {
        XmlOut::Element(el) => {
            let tag = el.tag();
            match tag.as_ref() {
                "horizontalRule" => {
                    blocks.push(ChapterBlock::SceneBreak);
                }
                "paragraph" => {
                    let spans = extract_spans_from_element(txn, el);
                    blocks.push(ChapterBlock::Paragraph(spans));
                }
                // timeBreak is a wrapping node — recurse into its children
                "timeBreak" => {
                    for inner in el.children(txn) {
                        collect_blocks_from_node(txn, &inner, blocks);
                    }
                }
                // Skip editor-only atom nodes
                "noteMarker" | "stateMarker" => {}
                // Unknown block elements — recurse looking for paragraphs
                _ => {
                    for inner in el.children(txn) {
                        collect_blocks_from_node(txn, &inner, blocks);
                    }
                }
            }
        }
        XmlOut::Text(txt) => {
            // Direct XmlText child of fragment (unusual but defensive)
            let spans = extract_spans_from_xmltext(txn, txt);
            if !spans.is_empty() {
                blocks.push(ChapterBlock::Paragraph(spans));
            }
        }
        XmlOut::Fragment(frag) => {
            for inner in frag.children(txn) {
                collect_blocks_from_node(txn, &inner, blocks);
            }
        }
    }
}

/// Extract formatted spans from all XmlText children of an element (typically a paragraph).
fn extract_spans_from_element<T: ReadTxn>(txn: &T, el: &yrs::types::xml::XmlElementRef) -> Vec<FmtSpan> {
    let mut spans = Vec::new();
    for child in el.children(txn) {
        match child {
            XmlOut::Text(txt) => {
                spans.extend(extract_spans_from_xmltext(txn, &txt));
            }
            XmlOut::Element(inner) => {
                let tag = inner.tag();
                match tag.as_ref() {
                    // hardBreak → Typst line break
                    "hardBreak" => {
                        spans.push(FmtSpan::plain(" \\\n".to_string()));
                    }
                    // Skip atom nodes inside paragraphs
                    "noteMarker" | "stateMarker" => {}
                    _ => {}
                }
            }
            _ => {}
        }
    }
    spans
}

/// Extract formatted text spans from an XmlTextRef using the diff() API.
/// Each Diff chunk has text content + optional formatting attributes.
fn extract_spans_from_xmltext<T: ReadTxn>(txn: &T, txt: &XmlTextRef) -> Vec<FmtSpan> {
    let diffs: Vec<Diff<YChange>> = txt.diff(txn, YChange::identity);
    let mut spans = Vec::new();
    for d in diffs {
        // Only process string content — skip embedded objects
        let text = match &d.insert {
            Out::Any(Any::String(s)) => s.to_string(),
            _ => continue,
        };
        if text.is_empty() { continue; }

        let mut span = FmtSpan::plain(text);

        if let Some(ref attrs) = d.attributes {
            for (key, val) in attrs.iter() {
                match key.as_ref() {
                    "bold" | "strong" => span.bold = any_is_truthy(val),
                    "italic" | "em" => span.italic = any_is_truthy(val),
                    "underline" => span.underline = any_is_truthy(val),
                    "strike" | "s" => span.strike = any_is_truthy(val),
                    "superscript" => span.superscript = any_is_truthy(val),
                    "subscript" => span.subscript = any_is_truthy(val),
                    "textStyle" => {
                        // textStyle is stored as Any::Map with fontSize/fontFamily keys
                        if let Any::Map(ref map) = val {
                            if let Some(Any::String(s)) = map.get("fontSize") {
                                span.font_size = Some(s.to_string());
                            }
                            if let Some(Any::String(s)) = map.get("fontFamily") {
                                span.font_family = Some(s.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        spans.push(span);
    }
    spans
}

/// Check if an Any value is truthy (used for boolean mark attributes).
fn any_is_truthy(val: &Any) -> bool {
    match val {
        Any::Bool(b) => *b,
        Any::Null | Any::Undefined => false,
        _ => true, // ProseMirror stores marks as `true` or as a non-null value
    }
}

/// Extract time sections from a Y.Doc chapter.
/// Walks top-level children of the prosemirror XmlFragment, splitting on timeBreak nodes.
/// Returns a list of sections with their index, label, and preview text.
#[derive(serde::Serialize, Clone)]
pub struct TimeSection {
    pub section_index: i64,
    pub label: String,
    pub is_flow: bool,
    pub preview_text: String,
    pub word_count: i64,
}

pub fn extract_time_sections(doc: &Doc) -> Vec<TimeSection> {
    let txn = doc.transact();
    let fragment = match txn.get_xml_fragment("prosemirror") {
        Some(f) => f,
        None => return vec![TimeSection {
            section_index: 0,
            label: String::new(),
            is_flow: true,
            preview_text: String::new(),
            word_count: 0,
        }],
    };

    let mut sections = Vec::new();
    let mut flow_text = String::new();
    let mut section_index: i64 = 0;

    // Walk top-level children. timeBreak is now a wrapping node:
    // - Regular blocks (paragraphs, headings) are flow text
    // - timeBreak nodes contain the time-jumped text as children
    for child in fragment.children(&txn) {
        match &child {
            XmlOut::Element(el) => {
                let tag = el.tag();
                if tag.as_ref() == "timeBreak" {
                    // Finalize flow text accumulated so far as a flow section
                    if !flow_text.is_empty() || section_index == 0 {
                        let wc = flow_text.split(|c: char| !c.is_alphanumeric() && c != '\'')
                            .filter(|w| !w.is_empty()).count() as i64;
                        let preview = flow_text.chars().take(200).collect::<String>();
                        sections.push(TimeSection {
                            section_index,
                            label: String::new(),
                            is_flow: true,
                            preview_text: preview.trim().to_string(),
                            word_count: wc,
                        });
                        section_index += 1;
                        flow_text.clear();
                    }

                    // Extract the timeBreak's content as a time-jumped section
                    let label = el.get_attribute(&txn, "label").unwrap_or_default();
                    let mut tb_text = String::new();
                    for inner in el.children(&txn) {
                        walk_xml_out(&txn, &inner, &mut tb_text);
                    }
                    let wc = tb_text.split(|c: char| !c.is_alphanumeric() && c != '\'')
                        .filter(|w| !w.is_empty()).count() as i64;
                    let preview = tb_text.chars().take(200).collect::<String>();
                    sections.push(TimeSection {
                        section_index,
                        label,
                        is_flow: false,
                        preview_text: preview.trim().to_string(),
                        word_count: wc,
                    });
                    section_index += 1;
                } else {
                    // Accumulate flow text
                    if !flow_text.is_empty() {
                        flow_text.push(' ');
                    }
                    walk_xml_out(&txn, &child, &mut flow_text);
                }
            }
            _ => {
                walk_xml_out(&txn, &child, &mut flow_text);
            }
        }
    }

    // Finalize remaining flow text
    if !flow_text.is_empty() || sections.is_empty() {
        let wc = flow_text.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| !w.is_empty()).count() as i64;
        let preview = flow_text.chars().take(200).collect::<String>();
        sections.push(TimeSection {
            section_index,
            label: String::new(),
            is_flow: true,
            preview_text: preview.trim().to_string(),
            word_count: wc,
        });
    }

    sections
}

/// Map state marker IDs to their containing time section index.
/// Walks top-level children, tracks section boundaries (timeBreak nodes),
/// and for each stateMarker node found, records (stateId, section_index).
pub fn locate_state_markers_in_sections(doc: &Doc) -> Vec<(i64, i64)> {
    let txn = doc.transact();
    let fragment = match txn.get_xml_fragment("prosemirror") {
        Some(f) => f,
        None => return Vec::new(),
    };

    let mut results = Vec::new();
    let mut current_section: i64 = 0;

    for child in fragment.children(&txn) {
        match &child {
            XmlOut::Element(el) => {
                let tag = el.tag();
                if tag.as_ref() == "timeBreak" {
                    current_section += 1;
                } else {
                    // Walk descendants looking for stateMarker nodes
                    find_state_markers_in(&txn, &child, current_section, &mut results);
                }
            }
            _ => {}
        }
    }

    results
}

fn find_state_markers_in<T: ReadTxn>(txn: &T, node: &XmlOut, section: i64, out: &mut Vec<(i64, i64)>) {
    match node {
        XmlOut::Element(el) => {
            let tag = el.tag();
            if tag.as_ref() == "stateMarker" {
                if let Some(id_str) = el.get_attribute(txn, "stateId") {
                    if let Ok(id) = id_str.parse::<i64>() {
                        out.push((id, section));
                    }
                }
            }
            for inner in el.children(txn) {
                find_state_markers_in(txn, &inner, section, out);
            }
        }
        XmlOut::Fragment(frag) => {
            for inner in frag.children(txn) {
                find_state_markers_in(txn, &inner, section, out);
            }
        }
        _ => {}
    }
}

/// Count words in a Y.Doc chapter.
pub fn word_count(doc: &Doc) -> usize {
    let text = extract_plain_text(doc);
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .count()
}
