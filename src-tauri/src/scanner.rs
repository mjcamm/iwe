use aho_corasick::{AhoCorasick, MatchKind};
use regex::RegexBuilder;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::db::{self, AppState};
use crate::wordlists;
use crate::ydoc;

/// Extract plain text from a chapter's Y.Doc content.
pub fn chapter_plain_text(chapter: &db::Chapter) -> Result<String, String> {
    let doc = ydoc::load_doc(&chapter.content)?;
    Ok(ydoc::extract_plain_text(&doc))
}

pub struct SearchTerm {
    pub entity_id: i64,
    pub entity_name: String,
    pub entity_type: String,
    pub color: String,
    pub text: String,
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

pub fn build_terms_all(entities: &[db::Entity]) -> Vec<SearchTerm> {
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
pub fn scan_plain(plain: &str, terms: &[SearchTerm]) -> Vec<Match> {
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
pub fn scan_text(state: tauri::State<'_, AppState>, text: String) -> Result<Vec<Match>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    let entities = db::list_entities(conn).map_err(|e| e.to_string())?;
    let terms = build_terms(&entities);

    Ok(scan_plain(&text, &terms))
}

/// Debug: dump a section of the Y.Doc extracted text for a chapter
#[tauri::command]
pub fn debug_stripped_text(state: tauri::State<'_, AppState>, chapter_id: i64, start: usize, length: usize) -> Result<String, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapter = db::get_chapter(conn, chapter_id).map_err(|e| e.to_string())?
        .ok_or("Chapter not found")?;
    let plain = chapter_plain_text(&chapter)?;
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
        let plain = chapter_plain_text(chapter)?;
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

    // Build entity search terms so we can find which positions are already highlighted
    let terms = build_terms_all(&entities);

    // Count occurrences and track locations (locations capped at 20 for memory, count is exact)
    let mut locations_map: HashMap<String, Vec<CandidateLocation>> = HashMap::new();
    let mut count_map: HashMap<String, usize> = HashMap::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let chars: Vec<char> = plain.chars().collect();

        // Run entity scan to get all matched ranges in this chapter
        let entity_matches = scan_plain(&plain, &terms);
        let entity_ranges: Vec<(usize, usize)> = entity_matches.iter().map(|m| (m.start, m.end)).collect();

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
                    // Skip if this position is already covered by an entity highlight
                    let cand_end = word_start + candidate.chars().count();
                    let overlaps_entity = entity_ranges.iter().any(|(es, ee)| word_start < *ee && cand_end > *es);
                    if overlaps_entity { continue; }
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
    pub char_position: usize,
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
        let plain = chapter_plain_text(chapter)?;
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

                        results.push(RelationshipResult {
                            chapter_id: chapter.id,
                            chapter_title: chapter.title.clone(),
                            lead_in,
                            middle,
                            lead_out,
                            entity_a_match: ma.matched_text.clone(),
                            entity_b_match: mb.matched_text.clone(),
                            distance,
                            char_position: first.start,
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

                        results.push(RelationshipResult {
                            chapter_id: chapter.id,
                            chapter_title: chapter.title.clone(),
                            lead_in,
                            middle: String::new(),
                            lead_out: String::new(),
                            entity_a_match: ma.matched_text.clone(),
                            entity_b_match: String::new(),
                            distance: 0,
                            char_position: ma.start,
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
        let plain = chapter_plain_text(chapter)?;
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
    pub char_position: usize,
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
        let plain = chapter_plain_text(chapter)?;
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

                results.push(DialogueResult {
                    chapter_id: chapter.id,
                    chapter_title: chapter.title.clone(),
                    dialogue,
                    matched_text,
                    context,
                    char_position: quote_char_start,
                });
            }
        }
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
        let plain = chapter_plain_text(chapter)?;
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
