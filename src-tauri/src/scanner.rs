use aho_corasick::{AhoCorasick, MatchKind};
use regex::RegexBuilder;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::db::{self, AppState};
use crate::wordlists;

struct SearchTerm {
    entity_id: i64,
    entity_name: String,
    entity_type: String,
    color: String,
    text: String,
}

#[derive(Serialize)]
pub struct Match {
    pub entity_id: i64,
    pub entity_name: String,
    pub entity_type: String,
    pub color: String,
    pub matched_text: String,
    pub start: usize,
    pub end: usize,
    pub context: String,
}

#[derive(Serialize)]
pub struct SnippetHighlight {
    pub offset: usize,  // char offset within the snippet text
    pub length: usize,
    pub matched_text: String,
}

#[derive(Serialize)]
pub struct EntityReference {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub matched_text: String,
    pub context: String,
    pub highlights: Vec<SnippetHighlight>,
    pub position: usize,
    pub occurrence: usize,
    pub anchor: String, // ~30 chars ending with the match, for precise JS-side search
}

#[derive(Serialize)]
pub struct EntityReferences {
    pub entity_id: i64,
    pub entity_name: String,
    pub total: usize,
    pub by_chapter: Vec<ChapterReferences>,
}

#[derive(Serialize)]
pub struct ChapterReferences {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub references: Vec<EntityReference>,
}

#[derive(Serialize)]
pub struct ChapterCounts {
    pub chapter_id: i64,
    pub entity_counts: Vec<EntityCount>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct EntityCount {
    pub entity_id: i64,
    pub entity_name: String,
    pub color: String,
    pub count: usize,
}

fn strip_html(html: &str) -> String {
    let mut plain = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_entity = false;
    let mut entity_buf = String::new();

    for ch in html.chars() {
        if ch == '<' {
            // Trim trailing whitespace before opening a tag — ProseMirror
            // trims trailing spaces from text nodes, so "text </p>" produces
            // "text" not "text ". Without this, char positions drift.
            while plain.ends_with(' ') || plain.ends_with('\t') {
                plain.pop();
            }
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            if ch == '&' {
                in_entity = true;
                entity_buf.clear();
                entity_buf.push(ch);
            } else if in_entity {
                entity_buf.push(ch);
                if ch == ';' {
                    let decoded = match entity_buf.as_str() {
                        "&amp;" => "&",
                        "&lt;" => "<",
                        "&gt;" => ">",
                        "&quot;" => "\"",
                        "&#39;" | "&apos;" => "'",
                        "&nbsp;" => " ",
                        "&#x27;" => "'",
                        "&#x2F;" => "/",
                        _ => {
                            plain.push_str(&entity_buf);
                            in_entity = false;
                            continue;
                        }
                    };
                    plain.push_str(decoded);
                    in_entity = false;
                } else if entity_buf.len() > 10 {
                    plain.push_str(&entity_buf);
                    in_entity = false;
                }
            } else {
                plain.push(ch);
            }
        }
    }

    if in_entity {
        plain.push_str(&entity_buf);
    }

    plain
}

fn is_word_boundary(ch: char) -> bool {
    !ch.is_alphanumeric() && ch != '\''
}

fn extract_context(chars: &[char], start: usize, end: usize) -> String {
    let ctx_start = start.saturating_sub(150);
    let ctx_end = (end + 150).min(chars.len());

    let actual_start = if ctx_start == 0 {
        0
    } else {
        // Find next whitespace after ctx_start
        (ctx_start..start).find(|&i| chars[i].is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(ctx_start)
    };

    let actual_end = if ctx_end >= chars.len() {
        chars.len()
    } else {
        // Find last whitespace before ctx_end
        (end..ctx_end).rev().find(|&i| chars[i].is_whitespace())
            .unwrap_or(ctx_end)
    };

    chars[actual_start..actual_end].iter().collect()
}

fn build_terms(entities: &[db::Entity]) -> Vec<SearchTerm> {
    build_terms_filtered(entities, true)
}

fn build_terms_all(entities: &[db::Entity]) -> Vec<SearchTerm> {
    build_terms_filtered(entities, false)
}

fn build_terms_filtered(entities: &[db::Entity], only_visible: bool) -> Vec<SearchTerm> {
    let mut terms = Vec::new();
    for entity in entities.iter().filter(|e| !only_visible || e.visible) {
        terms.push(SearchTerm {
            entity_id: entity.id,
            entity_name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            color: entity.color.clone(),
            text: entity.name.clone(),
        });
        for alias in &entity.aliases {
            let trimmed = alias.trim().to_string();
            if trimmed.is_empty() { continue; }
            terms.push(SearchTerm {
                entity_id: entity.id,
                entity_name: entity.name.clone(),
                entity_type: entity.entity_type.clone(),
                color: entity.color.clone(),
                text: trimmed,
            });
        }
    }
    terms
}

/// Core scan on character-indexed text. Returns matches with char-index positions.
fn scan_plain(plain: &str, terms: &[SearchTerm]) -> Vec<Match> {
    if terms.is_empty() || plain.is_empty() {
        return Vec::new();
    }

    // Work with chars for safe Unicode handling
    let chars: Vec<char> = plain.chars().collect();

    let plain_lower = plain.to_lowercase();
    let patterns: Vec<String> = terms.iter().map(|t| t.text.to_lowercase()).collect();

    let ac = match AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns)
    {
        Ok(ac) => ac,
        Err(_) => return Vec::new(),
    };

    // Build byte-offset to char-index map
    let mut byte_to_char: Vec<usize> = vec![0; plain_lower.len() + 1];
    for (char_idx, (byte_idx, _)) in plain_lower.char_indices().enumerate() {
        byte_to_char[byte_idx] = char_idx;
    }
    // Set the end sentinel
    byte_to_char[plain_lower.len()] = chars.len();

    let mut matches = Vec::new();

    for mat in ac.find_iter(&plain_lower) {
        let byte_start = mat.start();
        let byte_end = mat.end();
        let term = &terms[mat.pattern().as_usize()];

        // Convert byte offsets to char indices
        let char_start = byte_to_char[byte_start];
        let char_end = if byte_end <= plain_lower.len() {
            byte_to_char[byte_end]
        } else {
            chars.len()
        };

        // Word boundary check using char indices
        let before_ok = char_start == 0 || is_word_boundary(chars[char_start - 1]);
        let after_ok = char_end >= chars.len()
            || is_word_boundary(chars[char_end])
            // Allow possessive: 's or 's (curly apostrophe)
            || (chars[char_end] == '\'' || chars[char_end] == '\u{2019}')
                && char_end + 1 < chars.len()
                && (chars[char_end + 1] == 's' || chars[char_end + 1] == 'S')
                && (char_end + 2 >= chars.len() || is_word_boundary(chars[char_end + 2]));

        if !before_ok || !after_ok {
            continue;
        }

        let matched_text: String = chars[char_start..char_end].iter().collect();
        let context = extract_context(&chars, char_start, char_end);

        matches.push(Match {
            entity_id: term.entity_id,
            entity_name: term.entity_name.clone(),
            entity_type: term.entity_type.clone(),
            color: term.color.clone(),
            matched_text,
            start: char_start,
            end: char_end,
            context,
        });
    }

    matches
}

#[tauri::command]
pub fn scan_text(state: tauri::State<'_, AppState>, html: String) -> Result<Vec<Match>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let terms = build_terms(&entities);
    let plain = strip_html(&html);

    Ok(scan_plain(&plain, &terms))
}

/// Debug: dump a section of the Rust strip_html output for a chapter
#[tauri::command]
pub fn debug_stripped_text(state: tauri::State<'_, AppState>, chapter_id: i64, start: usize, length: usize) -> Result<String, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapter = db::get_chapter(conn, chapter_id).map_err(|e| e.to_string())?
        .ok_or("Chapter not found")?;
    let plain = strip_html(&chapter.content);
    let chars: Vec<char> = plain.chars().collect();
    let end = (start + length).min(chars.len());
    let total_len = chars.len();
    let snippet: String = chars[start.saturating_sub(30)..end.min(chars.len())].iter().collect();
    Ok(format!("total_chars={} snippet[{}-{}]={:?}", total_len, start.saturating_sub(30), end, snippet))
}

#[tauri::command]
pub fn debug_search_terms(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let terms = build_terms_all(&entities);
    Ok(terms.iter().map(|t| format!("{}: {} ({})", t.entity_name, t.text, t.entity_type)).collect())
}

#[tauri::command]
pub fn scan_all_chapters(state: tauri::State<'_, AppState>) -> Result<Vec<ChapterCounts>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let terms = build_terms(&entities);
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let mut results = Vec::new();

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let matches = scan_plain(&plain, &terms);

        let mut counts_map: std::collections::HashMap<i64, (String, String, usize)> =
            std::collections::HashMap::new();

        for m in &matches {
            let entry = counts_map.entry(m.entity_id)
                .or_insert_with(|| (m.entity_name.clone(), m.color.clone(), 0));
            entry.2 += 1;
        }

        let entity_counts: Vec<EntityCount> = counts_map.into_iter()
            .map(|(id, (name, color, count))| EntityCount {
                entity_id: id,
                entity_name: name,
                color,
                count,
            })
            .collect();

        let total = matches.len();
        results.push(ChapterCounts { chapter_id: chapter.id, entity_counts, total });
    }

    Ok(results)
}

// ---- Entity auto-detection ----

#[derive(Serialize)]
pub struct CandidateLocation {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub context: String,
    pub char_position: usize,
}

#[derive(Serialize)]
pub struct EntityCandidate {
    pub text: String,
    pub count: usize,
    pub locations: Vec<CandidateLocation>,
}

/// Check if a position is at the start of a sentence.
/// Walk backwards, skip anything that isn't alphanumeric or a sentence terminator.
/// If we hit a sentence terminator (. ? ! : —) before any letter, it's a sentence start.
/// If we hit a letter first, it's mid-sentence.
fn is_sentence_start(chars: &[char], word_start: usize) -> bool {
    if word_start == 0 {
        return true;
    }

    let mut i = word_start;
    while i > 0 {
        i -= 1;
        let ch = chars[i];
        // Sentence terminators → this is a sentence start
        if ch == '.' || ch == '?' || ch == '!' || ch == ':' || ch == '\u{2014}' {
            return true;
        }
        // Hit a letter or digit → mid-sentence
        if ch.is_alphanumeric() {
            return false;
        }
        // Everything else (whitespace, quotes, punctuation like commas, parens, etc.) → keep walking
    }

    // Reached beginning of text
    true
}

/// Extract a word starting at `start` (sequence of alphanumeric + apostrophe chars).
fn extract_word(chars: &[char], start: usize) -> (String, usize) {
    let mut end = start;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '\'' || chars[end] == '\u{2019}') {
        end += 1;
    }
    let word: String = chars[start..end].iter().collect();
    (word, end)
}

/// Strip possessive suffix ('s / 's) and normalize curly apostrophes to straight ones.
const HARD_EXCLUDE: &[&str] = &[
    "I", "I'm", "I'll", "I'd", "I've", "It", "It's",
    "He", "He's", "He'd", "He'll",
    "She", "She's", "She'd", "She'll",
    "We", "We're", "We'll", "We've", "We'd",
    "They", "They're", "They'll", "They've", "They'd",
    "You", "You're", "You'll", "You've", "You'd",
    "That", "That's", "This", "There", "There's",
    "What", "What's", "Where", "Where's", "When", "When's",
    "Who", "Who's", "Why", "How", "How's",
    "Don't", "Didn't", "Doesn't", "Won't", "Wouldn't",
    "Can't", "Couldn't", "Shouldn't", "Wasn't", "Weren't",
    "Isn't", "Aren't", "Haven't", "Hasn't", "Hadn't",
    "Let", "Let's", "Just", "But", "And", "The", "Then",
    "Not", "No", "Yes", "So", "Or", "If", "My", "His", "Her",
    "Its", "Our", "Your", "Their", "Some", "Any", "All",
    "Do", "Did", "Does", "Has", "Have", "Had", "Was", "Were",
    "Will", "Would", "Could", "Should", "May", "Might",
    "Been", "Being", "Going", "Come", "Said", "Tell", "Told",
    "Chapter", "Part", "Scene", "Act",
    "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday",
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
    "Mr", "Mrs", "Ms", "Miss", "Dr", "Prof", "Sir", "Lord", "Lady",
    "King", "Queen", "Prince", "Princess", "Captain", "General",
    "Uncle", "Aunt", "Dear",
    "English", "French", "German", "Spanish", "American", "British",
    "God", "Oh", "Ah", "Well", "Now", "Here", "Right", "OK", "Okay",
    "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
    "First", "Second", "Third", "Last", "Next", "New", "Old",
    "Much", "Many", "Most", "More", "Very", "Really", "Still",
    "After", "Before", "During", "Since", "Until", "While",
    "About", "Above", "Below", "Between", "Through",
    "Every", "Each", "Both", "Either", "Neither",
    "Never", "Always", "Sometimes", "Perhaps", "Maybe",
    "Even", "Already", "Enough", "Too", "Also",
    "Again", "Away", "Back", "Down", "Off", "Out", "Over", "Up",
];

/// Build the set of known words (entities, aliases, ignored, hard excludes).
fn build_known_set(entities: &[db::Entity], ignored: &[String]) -> HashSet<String> {
    let mut known: HashSet<String> = HashSet::new();

    for entity in entities {
        known.insert(entity.name.to_lowercase());
        for alias in &entity.aliases {
            known.insert(alias.to_lowercase());
        }
    }
    for word in ignored {
        known.insert(word.to_lowercase());
    }

    // Add normalized (possessive-stripped) versions
    let extras: Vec<String> = known.iter()
        .map(|n| normalize_candidate(n).to_lowercase())
        .filter(|n| !known.contains(n))
        .collect();
    for n in extras {
        known.insert(n);
    }

    // Hard excludes with both apostrophe variants
    for word in HARD_EXCLUDE {
        let lower = word.to_lowercase();
        known.insert(lower.replace('\'', "\u{2019}"));
        known.insert(lower);
    }

    known
}

/// Check if a word is a potential entity candidate (not in any known/exclude list).
fn is_unknown_candidate(word: &str, known: &HashSet<String>) -> bool {
    if word.len() < 2 || !word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        return false;
    }
    let lower = word.to_lowercase();
    let normalized = normalize_candidate(&lower);
    !known.contains(&lower) && !known.contains(&normalized)
}

fn normalize_candidate(text: &str) -> String {
    let s = text.replace('\u{2019}', "'").replace('\u{2018}', "'");
    // Strip trailing 's
    if s.ends_with("'s") || s.ends_with("'S") {
        s[..s.len() - 2].to_string()
    } else {
        s
    }
}

/// Detect potential entity candidates across all chapters.
/// Finds capitalized words appearing 3+ times that are NOT at sentence-starting positions,
/// not already known entities/aliases, and not in the ignored words list.
/// Also detects multi-word capitalized sequences ("The Red Lion").
#[tauri::command]
pub fn detect_entities(state: tauri::State<'_, AppState>, min_count: Option<usize>) -> Result<Vec<EntityCandidate>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let min_count = min_count.unwrap_or(3);
    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let ignored = db::list_ignored_words(conn).map_err(|e| e.to_string())?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;
    let known = build_known_set(&entities, &ignored);

    // Count occurrences and track locations (locations capped at 20 for memory, count is exact)
    let mut locations_map: HashMap<String, Vec<CandidateLocation>> = HashMap::new();
    let mut count_map: HashMap<String, usize> = HashMap::new();

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if !chars[i].is_alphabetic() {
                i += 1;
                continue;
            }

            if chars[i].is_uppercase() {
                let word_start = i;
                let mut words = Vec::new();
                let mut pos = word_start;

                loop {
                    let (word, word_end) = extract_word(&chars, pos);
                    if word.is_empty() { break; }

                    if !word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !words.is_empty() {
                        break;
                    }
                    if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        words.push(word);
                        pos = word_end;
                        if pos < chars.len() && chars[pos] == ' ' && pos + 1 < chars.len() && chars[pos + 1].is_uppercase() {
                            pos += 1;
                            continue;
                        }
                    }
                    break;
                }

                let at_sentence_start = is_sentence_start(&chars, word_start);
                let mut candidates_to_add = Vec::new();

                if at_sentence_start {
                    // Skip the first word (sentence starter), but evaluate the rest individually
                    for w in words.iter().skip(1) {
                        if w.len() >= 2 {
                            candidates_to_add.push(w.clone());
                        }
                    }
                } else if words.len() >= 2 {
                    // Multi-word: add the full sequence
                    candidates_to_add.push(words.join(" "));
                } else if !words.is_empty() {
                    let single = &words[0];
                    if single.len() >= 2 {
                        candidates_to_add.push(single.clone());
                    }
                }

                for candidate in candidates_to_add {
                    // Normalize: strip possessive 's
                    let candidate = normalize_candidate(&candidate);
                    if candidate.len() < 2 { continue; }
                    if !is_unknown_candidate(&candidate, &known) {
                        continue;
                    }
                    *count_map.entry(candidate.clone()).or_insert(0) += 1;
                    let locations = locations_map.entry(candidate.clone()).or_default();
                    if locations.len() < 20 {
                        // Build a wider context: ~20 words before and after
                        let mut ctx_start = word_start;
                        let mut words_before = 0;
                        while ctx_start > 0 && words_before < 20 {
                            ctx_start -= 1;
                            if chars[ctx_start].is_whitespace() { words_before += 1; }
                        }
                        if ctx_start > 0 { ctx_start += 1; } // skip past the space

                        let mut ctx_end = pos.min(chars.len());
                        let mut words_after = 0;
                        while ctx_end < chars.len() && words_after < 20 {
                            if chars[ctx_end].is_whitespace() { words_after += 1; }
                            ctx_end += 1;
                        }

                        let context: String = chars[ctx_start..ctx_end].iter().collect();
                        locations.push(CandidateLocation {
                            chapter_id: chapter.id,
                            chapter_title: chapter.title.clone(),
                            context,
                            char_position: word_start,
                        });
                    }
                }

                i = pos;
            } else {
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '\'') {
                    i += 1;
                }
            }
        }
    }

    let mut candidates: Vec<EntityCandidate> = count_map
        .into_iter()
        .filter(|(_, count)| *count >= min_count)
        .map(|(text, count)| {
            let locations = locations_map.remove(&text).unwrap_or_default();
            EntityCandidate { text, count, locations }
        })
        .collect();

    candidates.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(candidates)
}

// ---- Relationship search ----

#[derive(Serialize)]
pub struct RelationshipResult {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub lead_in: String,
    pub middle: String,
    pub lead_out: String,
    pub entity_a_match: String,
    pub entity_b_match: String,
    pub distance: usize,
    pub anchor: String,  // unique context for precise jumping
}

#[tauri::command]
pub fn relationship_search(
    state: tauri::State<'_, AppState>,
    entity_a_id: i64,
    entity_b_id: i64,
    search_type: String,   // "near" | "without"
    max_distance: usize,
) -> Result<Vec<RelationshipResult>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let entity_a = entities.iter().find(|e| e.id == entity_a_id).ok_or("Entity A not found")?;
    let entity_b = entities.iter().find(|e| e.id == entity_b_id).ok_or("Entity B not found")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    // Build search terms for each entity
    let terms_a = build_terms_all(&[entity_a.clone()]);
    let terms_b = build_terms_all(&[entity_b.clone()]);

    let mut results = Vec::new();
    let context_padding: usize = 80;

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();

        let matches_a = scan_plain(&plain, &terms_a);
        let matches_b = scan_plain(&plain, &terms_b);

        if matches_a.is_empty() { continue; }

        match search_type.as_str() {
            "near" => {
                let lead_size: usize = 200;

                for ma in &matches_a {
                    for mb in &matches_b {
                        // Determine first and second match
                        let (first, second, distance) = if mb.start >= ma.end && mb.start - ma.end <= max_distance {
                            (ma, mb, mb.start - ma.end)
                        } else if ma.start >= mb.end && ma.start - mb.end <= max_distance {
                            (mb, ma, ma.start - mb.end)
                        } else {
                            continue;
                        };

                        // Lead-in: ~200 chars centered on first match
                        let li_start = first.start.saturating_sub(lead_size / 2);
                        let li_end = (first.end + lead_size / 2).min(chars.len());
                        // Clean to word boundaries
                        let li_actual_start = if li_start == 0 { 0 } else {
                            (li_start..first.start).find(|&i| chars[i].is_whitespace())
                                .map(|i| i + 1).unwrap_or(li_start)
                        };
                        let li_actual_end = if li_end >= second.start { second.start } else {
                            (first.end..li_end).rev().find(|&i| chars[i].is_whitespace())
                                .unwrap_or(li_end)
                        };
                        let lead_in: String = chars[li_actual_start..li_actual_end.min(chars.len())].iter().collect();

                        // Lead-out: ~200 chars centered on second match
                        let lo_start = if second.start > li_actual_end { second.start } else { second.start };
                        let lo_end = (second.end + lead_size / 2).min(chars.len());
                        let lo_actual_start = if lo_start <= li_actual_end { li_actual_end } else {
                            // Only if there's a gap
                            lo_start
                        };
                        let lo_actual_end = if lo_end >= chars.len() { chars.len() } else {
                            (second.end..lo_end).rev().find(|&i| chars[i].is_whitespace())
                                .unwrap_or(lo_end)
                        };
                        let lead_out: String = chars[lo_actual_start..lo_actual_end].iter().collect();

                        // Middle: text between lead_in end and lead_out start
                        let middle: String = if li_actual_end < lo_actual_start {
                            chars[li_actual_end..lo_actual_start].iter().collect()
                        } else {
                            String::new()
                        };

                        // Build anchor: ~25 chars before the first match + the match
                        let anchor_start = first.start.saturating_sub(25);
                        let anchor: String = chars[anchor_start..first.end].iter().collect();

                        results.push(RelationshipResult {
                            chapter_id: chapter.id,
                            chapter_title: chapter.title.clone(),
                            lead_in,
                            middle,
                            lead_out,
                            entity_a_match: ma.matched_text.clone(),
                            entity_b_match: mb.matched_text.clone(),
                            distance,
                            anchor,
                        });
                    }
                }
            }
            "without" => {
                for ma in &matches_a {
                    let has_b_nearby = matches_b.iter().any(|mb| {
                        let dist = if mb.start >= ma.end { mb.start - ma.end }
                                   else if ma.start >= mb.end { ma.start - mb.end }
                                   else { 0 };
                        dist <= max_distance
                    });

                    if !has_b_nearby {
                        let slab_start = ma.start.saturating_sub(150);
                        let slab_end = (ma.end + 150).min(chars.len());

                        let actual_start = if slab_start == 0 { 0 } else {
                            (slab_start..ma.start).find(|&i| chars[i].is_whitespace())
                                .map(|i| i + 1).unwrap_or(slab_start)
                        };
                        let actual_end = if slab_end >= chars.len() { chars.len() } else {
                            (ma.end..slab_end).rev().find(|&i| chars[i].is_whitespace())
                                .unwrap_or(slab_end)
                        };

                        let lead_in: String = chars[actual_start..actual_end].iter().collect();

                        let anchor_start = ma.start.saturating_sub(25);
                        let anchor: String = chars[anchor_start..ma.end].iter().collect();

                        results.push(RelationshipResult {
                            chapter_id: chapter.id,
                            chapter_title: chapter.title.clone(),
                            lead_in,
                            middle: String::new(),
                            lead_out: String::new(),
                            entity_a_match: ma.matched_text.clone(),
                            entity_b_match: String::new(),
                            distance: 0,
                            anchor,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Sort by distance (nearest first for "near", doesn't matter for "without")
    results.sort_by_key(|r| r.distance);

    Ok(results)
}

// ---- Text search ----

#[derive(Serialize)]
pub struct TextSearchResult {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub matched_text: String,
    pub context: String,
    pub anchor: String,
    pub char_position: usize, // char offset in stripped plain text
    pub match_count: usize,
}

#[derive(Serialize)]
pub struct TextSearchResponse {
    pub total_matches: usize,
    pub results: Vec<TextSearchResult>,
}

/// Generate fuzzy regex pattern from a search term.
/// Allows 1 character substitution, insertion, or deletion per word.
fn fuzzy_pattern(term: &str) -> String {
    let chars: Vec<char> = term.chars().collect();
    if chars.len() <= 2 {
        return regex::escape(term);
    }

    let mut alternatives = vec![regex::escape(term)];

    for i in 0..chars.len() {
        // Substitution: replace char at i with any char
        let mut sub = String::new();
        for (j, ch) in chars.iter().enumerate() {
            if j == i {
                sub.push('.');
            } else {
                sub.push_str(&regex::escape(&ch.to_string()));
            }
        }
        alternatives.push(sub);

        // Deletion: skip char at i (finds words with a missing letter)
        // e.g. "hapened" matches pattern for "happened" with one 'p' deleted
        let mut del = String::new();
        for (j, ch) in chars.iter().enumerate() {
            if j == i { continue; }
            del.push_str(&regex::escape(&ch.to_string()));
        }
        alternatives.push(del);

        // Insertion: allow an extra char at position i
        // e.g. "hapenned" matches pattern for "happened" with extra 'n'
        let mut ins = String::new();
        for (j, ch) in chars.iter().enumerate() {
            if j == i {
                ins.push_str(&regex::escape(&ch.to_string()));
                ins.push('.'); // extra char after this position
            } else {
                ins.push_str(&regex::escape(&ch.to_string()));
            }
        }
        alternatives.push(ins);
    }

    format!("(?:{})", alternatives.join("|"))
}

#[tauri::command]
pub fn text_search(
    state: tauri::State<'_, AppState>,
    query: String,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
    fuzzy: bool,
) -> Result<TextSearchResponse, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    if query.is_empty() {
        return Ok(TextSearchResponse { total_matches: 0, results: Vec::new() });
    }

    // Expand POS tags like {verb}, {adjective}, {adverb}
    let expanded = wordlists::expand_pos_tags(&query);

    // Build the regex pattern
    let has_pos_tags = expanded != query; // POS tags were expanded
    let pattern = if fuzzy {
        let words: Vec<&str> = expanded.split_whitespace().collect();
        let fuzzy_words: Vec<String> = words.iter().map(|w| fuzzy_pattern(w)).collect();
        fuzzy_words.join(r"\s+")
    } else if use_regex || has_pos_tags {
        expanded
    } else {
        regex::escape(&expanded)
    };

    let pattern = if whole_word && !has_pos_tags {
        format!(r"\b{}\b", pattern)
    } else if has_pos_tags {
        format!(r"\b{}\b", pattern) // POS tags always use word boundaries
    } else {
        pattern
    };

    let re = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("Invalid search pattern: {}", e))?;

    let mut all_results = Vec::new();
    let mut total_matches: usize = 0;
    let context_radius: usize = 150;

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();

        // Find all matches
        let matches: Vec<(usize, usize, String)> = re.find_iter(&plain).map(|m| {
            // Convert byte positions to char positions
            let char_start = plain[..m.start()].chars().count();
            let char_end = plain[..m.end()].chars().count();
            let matched = chars[char_start..char_end].iter().collect::<String>();
            (char_start, char_end, matched)
        }).collect();

        if matches.is_empty() { continue; }

        let chapter_match_count = matches.len();
        total_matches += chapter_match_count;

        // Merge nearby matches and build results (similar to find_references)
        let mut i = 0;
        while i < matches.len() {
            let (start, end, ref matched) = matches[i];

            // Merge with subsequent matches that are within context range
            let mut group_end = end;
            let mut j = i + 1;
            while j < matches.len() && matches[j].0 <= group_end + context_radius {
                group_end = matches[j].1;
                j += 1;
            }

            let ctx_start = start.saturating_sub(context_radius);
            let ctx_end = (group_end + context_radius).min(chars.len());

            let actual_start = if ctx_start == 0 { 0 } else {
                (ctx_start..start).find(|&k| chars[k].is_whitespace())
                    .map(|k| k + 1).unwrap_or(ctx_start)
            };
            let actual_end = if ctx_end >= chars.len() { chars.len() } else {
                (group_end..ctx_end).rev().find(|&k| chars[k].is_whitespace())
                    .unwrap_or(ctx_end)
            };

            let context: String = chars[actual_start..actual_end].iter().collect();

            // Build anchor
            let anchor_start = start.saturating_sub(25);
            let anchor: String = chars[anchor_start..end].iter().collect();

            all_results.push(TextSearchResult {
                chapter_id: chapter.id,
                chapter_title: chapter.title.clone(),
                matched_text: matched.clone(),
                context,
                anchor,
                char_position: start,
                match_count: chapter_match_count,
            });

            i = j;
        }
    }

    Ok(TextSearchResponse {
        total_matches,
        results: all_results,
    })
}

// ---- Dialogue search ----

#[derive(Serialize)]
pub struct DialogueResult {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub dialogue: String,       // the full quoted text
    pub matched_text: String,   // what was matched within it
    pub context: String,        // surrounding text including who's speaking
    pub anchor: String,
}

#[tauri::command]
pub fn dialogue_search(
    state: tauri::State<'_, AppState>,
    query: String,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
    fuzzy: bool,
) -> Result<Vec<DialogueResult>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    if query.is_empty() {
        return Ok(Vec::new());
    }

    // Expand POS tags
    let expanded = wordlists::expand_pos_tags(&query);
    let has_pos_tags = expanded != query;

    let pattern = if fuzzy {
        let words: Vec<&str> = expanded.split_whitespace().collect();
        words.iter().map(|w| fuzzy_pattern(w)).collect::<Vec<_>>().join(r"\s+")
    } else if use_regex || has_pos_tags {
        expanded
    } else {
        regex::escape(&expanded)
    };

    let pattern = if whole_word || has_pos_tags {
        format!(r"\b{}\b", pattern)
    } else {
        pattern
    };

    let search_re = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("Invalid search pattern: {}", e))?;

    // Regex to find quoted text — handles "..." and "..." (curly quotes)
    let quote_re = RegexBuilder::new(r#"[""\u{201C}](.*?)[""\u{201D}]"#)
        .dot_matches_new_line(true)
        .build()
        .map_err(|e| format!("Quote regex error: {}", e))?;

    let mut results = Vec::new();
    let context_radius: usize = 80;

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();

        // Find all quoted passages
        for quote_match in quote_re.find_iter(&plain) {
            let quote_text = quote_match.as_str();

            // Search within this quoted passage
            if let Some(inner_match) = search_re.find(quote_text) {
                let matched_text = inner_match.as_str().to_string();

                // Get char position of the quote in the full text
                let quote_char_start = plain[..quote_match.start()].chars().count();
                let quote_char_end = plain[..quote_match.end()].chars().count();

                // Build context — include text before the quote to show who's speaking
                let ctx_start = quote_char_start.saturating_sub(context_radius);
                let ctx_end = (quote_char_end + context_radius / 2).min(chars.len());

                let actual_start = if ctx_start == 0 { 0 } else {
                    (ctx_start..quote_char_start).find(|&i| chars[i].is_whitespace())
                        .map(|i| i + 1).unwrap_or(ctx_start)
                };
                let actual_end = if ctx_end >= chars.len() { chars.len() } else {
                    (quote_char_end..ctx_end).rev().find(|&i| chars[i].is_whitespace())
                        .unwrap_or(ctx_end)
                };

                let context: String = chars[actual_start..actual_end].iter().collect();
                let dialogue: String = chars[quote_char_start..quote_char_end].iter().collect();

                // Anchor for jumping
                let anchor_start = quote_char_start.saturating_sub(25);
                let match_char_start = quote_char_start + plain[quote_match.start()..].chars()
                    .take_while(|c| *c != quote_text.chars().nth(inner_match.start()).unwrap_or(' '))
                    .count();
                let anchor_end = (quote_char_start + 30).min(chars.len());
                let anchor: String = chars[anchor_start..anchor_end].iter().collect();

                results.push(DialogueResult {
                    chapter_id: chapter.id,
                    chapter_title: chapter.title.clone(),
                    dialogue,
                    matched_text,
                    context,
                    anchor,
                });
            }
        }
    }

    Ok(results)
}

// ---- Repetition / Word frequency analysis ----

#[derive(Serialize)]
pub struct WordFrequency {
    pub word: String,
    pub total_count: usize,
    pub chapters: Vec<WordChapterCount>,
    pub clusters: Vec<WordCluster>, // only populated if window_size is set
}

#[derive(Serialize)]
pub struct WordChapterCount {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct WordCluster {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub count: usize,       // how many times in this window
    pub window_words: usize, // size of the window
    pub context: String,
    pub anchor: String,
}

#[tauri::command]
pub fn word_frequency(
    state: tauri::State<'_, AppState>,
    min_length: Option<usize>,
    min_count: Option<usize>,
    window_size: Option<usize>, // word count window; if set, only show words with clusters
) -> Result<Vec<WordFrequency>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let min_length = min_length.unwrap_or(4);
    let min_count = min_count.unwrap_or(2);

    // No stop words — let the author see everything and decide what matters

    // For each chapter, build a list of (word, word_index, char_position)
    struct WordInfo {
        word: String,
        word_index: usize,  // nth word in the chapter
        char_pos: usize,    // char offset for context/anchor
    }

    // word -> [(chapter_id, chapter_title, [WordInfo])]
    let mut global_counts: HashMap<String, Vec<(i64, String, Vec<WordInfo>)>> = HashMap::new();

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let lower = plain.to_lowercase();

        let mut word_infos: HashMap<String, Vec<WordInfo>> = HashMap::new();
        let mut word_index: usize = 0;
        let mut char_pos: usize = 0;

        for word in lower.split(|c: char| !c.is_alphanumeric() && c != '\'') {
            let clean = word.trim_matches('\'');
            if !clean.is_empty() {
                if clean.len() >= min_length {
                    word_infos.entry(clean.to_string()).or_default().push(WordInfo {
                        word: clean.to_string(),
                        word_index,
                        char_pos,
                    });
                }
                word_index += 1;
            }
            char_pos += word.len() + 1;
        }

        for (word, infos) in word_infos {
            global_counts.entry(word)
                .or_default()
                .push((chapter.id, chapter.title.clone(), infos));
        }
    }

    let mut results: Vec<WordFrequency> = Vec::new();

    for (word, chapter_data) in &global_counts {
        let total_count: usize = chapter_data.iter().map(|(_, _, infos)| infos.len()).sum();
        if total_count < min_count { continue; }

        let chapters_info: Vec<WordChapterCount> = chapter_data.iter()
            .map(|(id, title, infos)| WordChapterCount {
                chapter_id: *id,
                chapter_title: title.clone(),
                count: infos.len(),
            })
            .filter(|c| c.count > 0)
            .collect();

        let mut clusters = Vec::new();

        if let Some(ws) = window_size {
            for (chapter_id, chapter_title, infos) in chapter_data {
                if infos.len() < min_count { continue; }

                let chapter = chapters.iter().find(|c| c.id == *chapter_id);
                if chapter.is_none() { continue; }
                let plain = strip_html(&chapter.unwrap().content);
                let chars: Vec<char> = plain.chars().collect();

                // Sliding window by word index
                // For each occurrence, count how many other occurrences are within ws words
                let mut used = vec![false; infos.len()];

                for (i, info) in infos.iter().enumerate() {
                    if used[i] { continue; }

                    let nearby_count = infos.iter().enumerate()
                        .filter(|(j, other)| {
                            *j != i &&
                            (other.word_index as i64 - info.word_index as i64).unsigned_abs() as usize <= ws
                        })
                        .count();

                    if nearby_count + 1 >= min_count {
                        // Find the full range of this cluster — earliest to latest occurrence
                        let mut cluster_min_pos = info.char_pos;
                        let mut cluster_max_pos = info.char_pos + word.len();

                        for (j, other) in infos.iter().enumerate() {
                            if (other.word_index as i64 - info.word_index as i64).unsigned_abs() as usize <= ws {
                                used[j] = true;
                                if other.char_pos < cluster_min_pos {
                                    cluster_min_pos = other.char_pos;
                                }
                                let other_end = other.char_pos + word.len();
                                if other_end > cluster_max_pos {
                                    cluster_max_pos = other_end;
                                }
                            }
                        }

                        // Build context: full cluster range plus 50 chars padding
                        let ctx_start = cluster_min_pos.saturating_sub(50);
                        let ctx_end = (cluster_max_pos + 50).min(chars.len());
                        let actual_start = if ctx_start == 0 { 0 } else {
                            (ctx_start..cluster_min_pos).find(|&k| chars[k].is_whitespace())
                                .map(|k| k + 1).unwrap_or(ctx_start)
                        };
                        let actual_end = if ctx_end >= chars.len() { chars.len() } else {
                            (cluster_max_pos..ctx_end).rev().find(|&k| chars[k].is_whitespace())
                                .unwrap_or(ctx_end)
                        };

                        let context: String = chars[actual_start..actual_end].iter().collect();
                        let anchor_start = info.char_pos.saturating_sub(25);
                        let anchor_end = (info.char_pos + word.len() + 1).min(chars.len());
                        let anchor: String = chars[anchor_start..anchor_end].iter().collect();

                        clusters.push(WordCluster {
                            chapter_id: *chapter_id,
                            chapter_title: chapter_title.clone(),
                            count: nearby_count + 1,
                            window_words: ws,
                            context,
                            anchor,
                        });
                    }
                }
            }
        }

        // If window mode is active, only include words that have clusters
        if window_size.is_some() && clusters.is_empty() {
            continue;
        }

        results.push(WordFrequency {
            word: word.clone(),
            total_count,
            chapters: chapters_info,
            clusters,
        });
    }

    // Sort: if window mode, sort by cluster severity; otherwise by total count
    if window_size.is_some() {
        results.sort_by(|a, b| {
            let a_max = a.clusters.iter().map(|c| c.count).max().unwrap_or(0);
            let b_max = b.clusters.iter().map(|c| c.count).max().unwrap_or(0);
            b_max.cmp(&a_max).then(b.total_count.cmp(&a.total_count))
        });
    } else {
        results.sort_by(|a, b| b.total_count.cmp(&a.total_count));
    }

    Ok(results)
}

// ---- Similar phrasing detection ----

#[derive(Serialize)]
pub struct SimilarGroup {
    pub representative: String,  // the "best" example sentence
    pub count: usize,
    pub avg_similarity: f64,
    pub occurrences: Vec<SimilarOccurrence>,
}

#[derive(Serialize, Clone)]
pub struct SimilarOccurrence {
    pub sentence: String,
    pub chapter_id: i64,
    pub chapter_title: String,
    pub anchor: String,
    pub similarity: f64,
}

/// Normalize a sentence for comparison: lowercase, strip punctuation, collapse whitespace
fn normalize_sentence(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Calculate Jaccard similarity between two word sets (intersection / union)
fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() { return 0.0; }
    let set_a: HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 { return 0.0; }
    intersection as f64 / union as f64
}

/// Also check word order similarity using longest common subsequence ratio
fn order_similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() { return 0.0; }
    let m = a.len();
    let n = b.len();
    // LCS with DP (capped to avoid huge allocations)
    if m > 200 || n > 200 { return jaccard_similarity(a, b); }
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a[i-1] == b[j-1] {
                dp[i][j] = dp[i-1][j-1] + 1;
            } else {
                dp[i][j] = dp[i-1][j].max(dp[i][j-1]);
            }
        }
    }
    let lcs = dp[m][n];
    (2.0 * lcs as f64) / (m + n) as f64
}

#[tauri::command]
pub fn find_similar_phrases(
    state: tauri::State<'_, AppState>,
    min_words: Option<usize>,      // minimum sentence length to consider
    min_similarity: Option<f64>,    // 0.0-1.0, default 0.6
) -> Result<Vec<SimilarGroup>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let min_words = min_words.unwrap_or(5);
    let min_sim = min_similarity.unwrap_or(0.6);

    struct Sentence {
        text: String,
        words: Vec<String>,
        chapter_id: i64,
        chapter_title: String,
        char_pos: usize,
    }

    // Extract all sentences from all chapters
    let mut sentences: Vec<Sentence> = Vec::new();

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();

        // Split on sentence terminators
        let mut start = 0;
        for (i, ch) in chars.iter().enumerate() {
            if *ch == '.' || *ch == '?' || *ch == '!' {
                let sentence_chars: String = chars[start..=i].iter().collect();
                let trimmed = sentence_chars.trim();
                if !trimmed.is_empty() {
                    let words = normalize_sentence(trimmed);
                    if words.len() >= min_words {
                        let anchor_start = start.saturating_sub(10);
                        let anchor_end = (start + 35).min(chars.len());
                        let anchor: String = chars[anchor_start..anchor_end].iter().collect();

                        sentences.push(Sentence {
                            text: trimmed.to_string(),
                            words,
                            chapter_id: chapter.id,
                            chapter_title: chapter.title.clone(),
                            char_pos: start,
                        });
                    }
                }
                start = i + 1;
            }
        }
    }

    // Build a simple fingerprint: set of words in the sentence
    // Only compare sentences that share at least some words
    let mut word_to_sentences: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, sentence) in sentences.iter().enumerate() {
        for word in &sentence.words {
            if word.len() >= 4 { // skip short words for fingerprinting
                word_to_sentences.entry(word.clone()).or_default().push(idx);
            }
        }
    }

    // Find candidate pairs and build similarity edges
    let mut checked: HashSet<(usize, usize)> = HashSet::new();
    // edges: (i, j, similarity)
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();

    for indices in word_to_sentences.values() {
        if indices.len() > 100 { continue; }
        for &i in indices {
            for &j in indices {
                if i >= j { continue; }
                if checked.contains(&(i, j)) { continue; }
                checked.insert((i, j));

                let a = &sentences[i];
                let b = &sentences[j];

                if a.chapter_id == b.chapter_id && a.char_pos == b.char_pos { continue; }

                let len_ratio = a.words.len().min(b.words.len()) as f64
                    / a.words.len().max(b.words.len()) as f64;
                if len_ratio < 0.5 { continue; }

                let jaccard = jaccard_similarity(&a.words, &b.words);
                let order = order_similarity(&a.words, &b.words);
                let similarity = (jaccard * 0.4 + order * 0.6).min(1.0);

                if similarity >= min_sim {
                    edges.push((i, j, similarity));
                }
            }
        }
    }

    // Group similar sentences using union-find (connected components)
    let mut parent: Vec<usize> = (0..sentences.len()).collect();
    fn find(parent: &mut Vec<usize>, i: usize) -> usize {
        if parent[i] != i { parent[i] = find(parent, parent[i]); }
        parent[i]
    }
    for &(i, j, _) in &edges {
        let pi = find(&mut parent, i);
        let pj = find(&mut parent, j);
        if pi != pj { parent[pi] = pj; }
    }

    // Build groups
    let mut group_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for idx in 0..sentences.len() {
        let root = find(&mut parent, idx);
        // Only add if this sentence is involved in at least one edge
        if edges.iter().any(|&(i, j, _)| i == idx || j == idx) {
            group_map.entry(root).or_default().push(idx);
        }
    }

    // Precompute chapter plain text cache
    let chapter_plains: HashMap<i64, Vec<char>> = chapters.iter()
        .map(|c| (c.id, strip_html(&c.content).chars().collect()))
        .collect();

    let mut groups: Vec<SimilarGroup> = Vec::new();

    for (_, members) in &group_map {
        if members.len() < 2 { continue; }

        // Find average similarity within the group
        let mut sim_sum = 0.0;
        let mut sim_count = 0;
        for &(i, j, sim) in &edges {
            if members.contains(&i) && members.contains(&j) {
                sim_sum += sim;
                sim_count += 1;
            }
        }
        let avg_similarity = if sim_count > 0 { sim_sum / sim_count as f64 } else { 0.0 };

        // Pick the longest sentence as representative
        let rep_idx = members.iter()
            .max_by_key(|&&idx| sentences[idx].text.len())
            .copied()
            .unwrap_or(members[0]);

        let occurrences: Vec<SimilarOccurrence> = members.iter().map(|&idx| {
            let s = &sentences[idx];
            let chars = chapter_plains.get(&s.chapter_id).unwrap();
            let anchor_start = s.char_pos.saturating_sub(15);
            let anchor_end = (s.char_pos + 35).min(chars.len());
            let anchor: String = chars[anchor_start..anchor_end].iter().collect();

            // Calculate this sentence's similarity to the representative
            let sim = if idx == rep_idx { 1.0 } else {
                let rep = &sentences[rep_idx];
                let j = jaccard_similarity(&s.words, &rep.words);
                let o = order_similarity(&s.words, &rep.words);
                (j * 0.4 + o * 0.6).min(1.0)
            };

            SimilarOccurrence {
                sentence: s.text.clone(),
                chapter_id: s.chapter_id,
                chapter_title: s.chapter_title.clone(),
                anchor,
                similarity: sim,
            }
        }).collect();

        groups.push(SimilarGroup {
            representative: sentences[rep_idx].text.clone(),
            count: members.len(),
            avg_similarity,
            occurrences,
        });
    }

    // Sort by count (most duplicates first), then by similarity
    groups.sort_by(|a, b| {
        b.count.cmp(&a.count)
            .then(b.avg_similarity.partial_cmp(&a.avg_similarity).unwrap_or(std::cmp::Ordering::Equal))
    });

    groups.truncate(100);

    Ok(groups)
}

// ---- Heatmap data ----

#[derive(Serialize)]
pub struct HeatmapData {
    pub chapters: Vec<HeatmapChapter>,
    pub entities: Vec<HeatmapEntity>,
    pub chapter_grid: Vec<Vec<usize>>,      // [entity_idx][chapter_idx] = mention count
    pub sentence_grid: Vec<Vec<u8>>,         // [entity_idx][sentence_idx] = presence (0 or 1)
    pub sentence_chapter_breaks: Vec<usize>, // sentence index where each chapter starts
    pub total_sentences: usize,
}

#[derive(Serialize)]
pub struct HeatmapChapter {
    pub id: i64,
    pub title: String,
}

#[derive(Serialize)]
pub struct HeatmapEntity {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub entity_type: String,
}

#[tauri::command]
pub fn generate_heatmap(
    state: tauri::State<'_, AppState>,
    entity_ids: Vec<i64>,
) -> Result<HeatmapData, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let all_entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    // Filter to requested entities
    let entities: Vec<&db::Entity> = entity_ids.iter()
        .filter_map(|id| all_entities.iter().find(|e| e.id == *id))
        .collect();

    let heatmap_chapters: Vec<HeatmapChapter> = chapters.iter()
        .map(|c| HeatmapChapter { id: c.id, title: c.title.clone() })
        .collect();

    let heatmap_entities: Vec<HeatmapEntity> = entities.iter()
        .map(|e| HeatmapEntity {
            id: e.id, name: e.name.clone(),
            color: e.color.clone(), entity_type: e.entity_type.clone(),
        })
        .collect();

    // Build search terms per entity
    let entity_terms: Vec<Vec<SearchTerm>> = entities.iter()
        .map(|e| build_terms_all(&[(*e).clone()]))
        .collect();

    // Chapter-level grid
    let mut chapter_grid: Vec<Vec<usize>> = vec![vec![0; chapters.len()]; entities.len()];

    // Sentence-level grid
    let mut sentence_grid: Vec<Vec<u8>> = Vec::new();
    let mut sentence_chapter_breaks: Vec<usize> = Vec::new();
    let mut total_sentences: usize = 0;

    for (ch_idx, chapter) in chapters.iter().enumerate() {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();

        // Split into sentences
        let mut sentences: Vec<(usize, usize)> = Vec::new(); // (start, end) char positions
        let mut sent_start = 0;
        for (i, &ch) in chars.iter().enumerate() {
            if ch == '.' || ch == '?' || ch == '!' {
                if i > sent_start {
                    sentences.push((sent_start, i + 1));
                }
                sent_start = i + 1;
            }
        }
        // Last sentence (no terminator)
        if sent_start < chars.len() {
            let trimmed: String = chars[sent_start..].iter().collect();
            if trimmed.trim().len() > 5 {
                sentences.push((sent_start, chars.len()));
            }
        }

        sentence_chapter_breaks.push(total_sentences);

        for (ent_idx, terms) in entity_terms.iter().enumerate() {
            let matches = scan_plain(&plain, terms);

            // Chapter-level count
            chapter_grid[ent_idx][ch_idx] = matches.len();

            // Sentence-level presence
            // Ensure the grid row exists
            while sentence_grid.len() <= ent_idx {
                sentence_grid.push(Vec::new());
            }

            for (sent_idx, &(sent_start, sent_end)) in sentences.iter().enumerate() {
                let global_sent_idx = total_sentences + sent_idx;

                // Ensure the row is long enough
                while sentence_grid[ent_idx].len() <= global_sent_idx {
                    sentence_grid[ent_idx].push(0);
                }

                // Check if any match falls within this sentence
                let has_match = matches.iter().any(|m| {
                    m.start >= sent_start && m.start < sent_end
                });

                sentence_grid[ent_idx][global_sent_idx] = if has_match { 1 } else { 0 };
            }
        }

        total_sentences += sentences.len();
    }

    // Pad all entity rows to total_sentences length
    for row in &mut sentence_grid {
        while row.len() < total_sentences {
            row.push(0);
        }
    }

    Ok(HeatmapData {
        chapters: heatmap_chapters,
        entities: heatmap_entities,
        chapter_grid,
        sentence_grid,
        sentence_chapter_breaks,
        total_sentences,
    })
}

// ---- Chapter analysis ----

#[derive(Serialize)]
pub struct ChapterAnalysis {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub total_words: usize,
    pub dialogue_words: usize,
    pub narrative_words: usize,
    pub sentence_count: usize,
    pub paragraph_count: usize,
    pub avg_sentence_length: f64,
    pub avg_paragraph_length: f64,
    pub longest_sentence: usize,
    pub shortest_sentence: usize,
    pub unique_words: usize,
    pub vocabulary_density: f64, // unique_words / total_words
}

fn count_words_in(text: &str) -> usize {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .count()
}

#[tauri::command]
pub fn chapter_analysis(state: tauri::State<'_, AppState>) -> Result<Vec<ChapterAnalysis>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let quote_re = RegexBuilder::new(r#"[""\u{201C}](.*?)[""\u{201D}]"#)
        .dot_matches_new_line(true)
        .build()
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        if plain.trim().is_empty() {
            results.push(ChapterAnalysis {
                chapter_id: chapter.id,
                chapter_title: chapter.title.clone(),
                total_words: 0, dialogue_words: 0, narrative_words: 0,
                sentence_count: 0, paragraph_count: 0,
                avg_sentence_length: 0.0, avg_paragraph_length: 0.0,
                longest_sentence: 0, shortest_sentence: 0,
                unique_words: 0, vocabulary_density: 0.0,
            });
            continue;
        }

        let total_words = count_words_in(&plain);

        // Dialogue vs narrative
        let mut dialogue_text = String::new();
        for m in quote_re.find_iter(&plain) {
            dialogue_text.push_str(m.as_str());
            dialogue_text.push(' ');
        }
        let dialogue_words = count_words_in(&dialogue_text);
        let narrative_words = total_words.saturating_sub(dialogue_words);

        // Sentences
        let chars: Vec<char> = plain.chars().collect();
        let mut sentences: Vec<usize> = Vec::new(); // word counts per sentence
        let mut sent_start = 0;
        for (i, &ch) in chars.iter().enumerate() {
            if ch == '.' || ch == '?' || ch == '!' {
                let sent_text: String = chars[sent_start..=i].iter().collect();
                let wc = count_words_in(&sent_text);
                if wc > 0 { sentences.push(wc); }
                sent_start = i + 1;
            }
        }
        // Trailing text
        if sent_start < chars.len() {
            let sent_text: String = chars[sent_start..].iter().collect();
            let wc = count_words_in(&sent_text);
            if wc > 0 { sentences.push(wc); }
        }

        let sentence_count = sentences.len();
        let longest_sentence = sentences.iter().copied().max().unwrap_or(0);
        let shortest_sentence = sentences.iter().copied().min().unwrap_or(0);
        let avg_sentence_length = if sentence_count > 0 {
            sentences.iter().sum::<usize>() as f64 / sentence_count as f64
        } else { 0.0 };

        // Paragraphs (split by double newlines or block boundaries — in stripped HTML, these become runs of text)
        // Since we strip HTML, paragraphs are separated by where block tags were.
        // Use the HTML content to count <p> tags
        let paragraph_count = chapter.content.matches("<p>").count()
            .max(chapter.content.matches("<p ").count())
            .max(1);
        let avg_paragraph_length = if paragraph_count > 0 {
            total_words as f64 / paragraph_count as f64
        } else { 0.0 };

        // Vocabulary density
        let lower = plain.to_lowercase();
        let unique: HashSet<&str> = lower.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| w.len() >= 2)
            .collect();
        let unique_words = unique.len();
        let vocabulary_density = if total_words > 0 {
            unique_words as f64 / total_words as f64
        } else { 0.0 };

        results.push(ChapterAnalysis {
            chapter_id: chapter.id,
            chapter_title: chapter.title.clone(),
            total_words,
            dialogue_words,
            narrative_words,
            sentence_count,
            paragraph_count,
            avg_sentence_length,
            avg_paragraph_length,
            longest_sentence,
            shortest_sentence,
            unique_words,
            vocabulary_density,
        });
    }

    Ok(results)
}

/// Check a single word against known entities, aliases, ignored words, and hard excludes.
/// Returns true if the word is unknown and could be an entity candidate.
#[tauri::command]
pub fn check_word(state: tauri::State<'_, AppState>, word: String) -> Result<bool, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let ignored = db::list_ignored_words(conn).map_err(|e| e.to_string())?;
    let known = build_known_set(&entities, &ignored);

    Ok(is_unknown_candidate(&word, &known))
}

/// Find all references for a specific entity across all chapters.
/// Returns matches grouped by chapter with context snippets.
#[tauri::command]
pub fn find_references(state: tauri::State<'_, AppState>, entity_id: i64) -> Result<EntityReferences, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let entity = entities.iter().find(|e| e.id == entity_id)
        .ok_or("Entity not found")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    // Build search terms for just this entity
    let mut terms = Vec::new();
    terms.push(SearchTerm {
        entity_id: entity.id,
        entity_name: entity.name.clone(),
        entity_type: entity.entity_type.clone(),
        color: entity.color.clone(),
        text: entity.name.clone(),
    });
    for alias in &entity.aliases {
        terms.push(SearchTerm {
            entity_id: entity.id,
            entity_name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            color: entity.color.clone(),
            text: alias.clone(),
        });
    }

    let mut by_chapter: Vec<ChapterReferences> = Vec::new();
    let mut total = 0;

    for chapter in &chapters {
        let plain = strip_html(&chapter.content);
        let chars: Vec<char> = plain.chars().collect();
        let matches = scan_plain(&plain, &terms);

        if matches.is_empty() { continue; }

        // Merge overlapping/adjacent matches into combined snippets
        // Each match has a ~200 char context window. If windows overlap, merge them.
        let context_radius: usize = 150;

        struct MatchInfo {
            start: usize,
            end: usize,
            matched_text: String,
        }

        let match_infos: Vec<MatchInfo> = matches.iter().map(|m| MatchInfo {
            start: m.start,
            end: m.end,
            matched_text: m.matched_text.clone(),
        }).collect();

        // Build merged snippet ranges
        let mut snippets: Vec<(usize, usize, Vec<usize>)> = Vec::new(); // (ctx_start, ctx_end, match_indices)

        for (i, mi) in match_infos.iter().enumerate() {
            let ctx_start = mi.start.saturating_sub(context_radius);
            let ctx_end = (mi.end + context_radius).min(chars.len());

            // Try to merge with the last snippet
            if let Some(last) = snippets.last_mut() {
                if ctx_start <= last.1 {
                    // Overlapping — extend the snippet
                    last.1 = last.1.max(ctx_end);
                    last.2.push(i);
                    continue;
                }
            }
            snippets.push((ctx_start, ctx_end, vec![i]));
        }

        let mut references: Vec<EntityReference> = Vec::new();
        // Track occurrence count per matched text for precise jumping
        let mut occurrence_counts: HashMap<String, usize> = HashMap::new();

        for (ctx_start, ctx_end, match_indices) in &snippets {
            // Clean snippet boundaries to word edges
            let actual_start = if *ctx_start == 0 {
                0
            } else {
                (*ctx_start..match_infos[match_indices[0]].start)
                    .find(|&i| chars[i].is_whitespace())
                    .map(|i| i + 1)
                    .unwrap_or(*ctx_start)
            };
            let actual_end = if *ctx_end >= chars.len() {
                chars.len()
            } else {
                (match_infos[*match_indices.last().unwrap()].end..*ctx_end)
                    .rev()
                    .find(|&i| chars[i].is_whitespace())
                    .unwrap_or(*ctx_end)
            };

            let snippet_text: String = chars[actual_start..actual_end].iter().collect();

            // Build highlights relative to snippet
            let highlights: Vec<SnippetHighlight> = match_indices.iter().map(|&mi| {
                let info = &match_infos[mi];
                SnippetHighlight {
                    offset: info.start.saturating_sub(actual_start),
                    length: info.end - info.start,
                    matched_text: info.matched_text.clone(),
                }
            }).collect();

            let first = &match_infos[match_indices[0]];
            let occ = occurrence_counts.entry(first.matched_text.to_lowercase()).or_insert(0);
            let current_occ = *occ;
            *occ += match_indices.len();

            // Build anchor: ~30 chars ending with the matched text for precise JS search
            let anchor_start = first.start.saturating_sub(25);
            let anchor: String = chars[anchor_start..first.end].iter().collect();

            references.push(EntityReference {
                chapter_id: chapter.id,
                chapter_title: chapter.title.clone(),
                matched_text: first.matched_text.clone(),
                context: snippet_text,
                highlights,
                position: first.start,
                occurrence: current_occ,
                anchor,
            });
        }

        total += matches.len();

        by_chapter.push(ChapterReferences {
            chapter_id: chapter.id,
            chapter_title: chapter.title.clone(),
            references,
        });
    }

    Ok(EntityReferences {
        entity_id: entity.id,
        entity_name: entity.name.clone(),
        total,
        by_chapter,
    })
}
