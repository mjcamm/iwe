use regex::RegexBuilder;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::db::{self, AppState};
use crate::scanner::{chapter_plain_text, scan_plain, build_terms_all, SearchTerm};
use crate::ydoc;

// ---- Word frequency / cluster detection ----

#[derive(Serialize)]
pub struct WordFrequency {
    pub word: String,
    pub total_count: usize,
    pub chapters: Vec<WordChapterCount>,
    pub clusters: Vec<WordCluster>,
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
    pub char_start: usize,  // char offset of first mention in cluster
    pub char_end: usize,    // char offset past last mention in cluster
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
        word_index: usize,  // nth word in the chapter
        char_pos: usize,    // char offset for context/anchor
    }

    // word -> [(chapter_id, chapter_title, [WordInfo])]
    let mut global_counts: HashMap<String, Vec<(i64, String, Vec<WordInfo>)>> = HashMap::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let lower = plain.to_lowercase();

        let mut word_infos: HashMap<String, Vec<WordInfo>> = HashMap::new();
        let mut word_index: usize = 0;
        let mut char_pos: usize = 0;

        for word in lower.split(|c: char| !c.is_alphanumeric() && c != '\'') {
            let clean = word.trim_matches('\'');
            if !clean.is_empty() {
                if clean.len() >= min_length {
                    word_infos.entry(clean.to_string()).or_default().push(WordInfo {
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
                let plain = chapter_plain_text(chapter.unwrap()).map_err(|e| e.to_string())?;
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

                        clusters.push(WordCluster {
                            chapter_id: *chapter_id,
                            chapter_title: chapter_title.clone(),
                            count: nearby_count + 1,
                            window_words: ws,
                            context,
                            char_start: cluster_min_pos,
                            char_end: cluster_max_pos,
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
    pub char_position: usize,
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
        let plain = chapter_plain_text(chapter)?;
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
                char_position: s.char_pos,
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
        let plain = chapter_plain_text(chapter)?;
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
        let plain = chapter_plain_text(chapter)?;
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

        // Count paragraphs from Y.Doc structure (top-level block elements)
        let paragraph_count = {
            use yrs::{Transact as _, XmlFragment as _, ReadTxn as _};
            let ydoc = ydoc::load_doc(&chapter.content).unwrap_or_else(|_| yrs::Doc::new());
            let txn = ydoc.transact();
            match txn.get_xml_fragment("prosemirror") {
                Some(frag) => frag.len(&txn) as usize,
                None => 1,
            }
        }.max(1);
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

// ---- Pacing analysis (sentence length waveform) ----

#[derive(Serialize)]
pub struct PacingChapter {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub sentence_lengths: Vec<usize>, // word count per sentence, in order
}

#[tauri::command]
pub fn pacing_analysis(state: tauri::State<'_, AppState>) -> Result<Vec<PacingChapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let mut results = Vec::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let chars: Vec<char> = plain.chars().collect();

        let mut sentence_lengths = Vec::new();
        let mut sent_start = 0;

        for (i, &ch) in chars.iter().enumerate() {
            if ch == '.' || ch == '?' || ch == '!' {
                let sent_text: String = chars[sent_start..=i].iter().collect();
                let wc = sent_text.split(|c: char| !c.is_alphanumeric() && c != '\'')
                    .filter(|w| !w.is_empty())
                    .count();
                if wc > 0 {
                    sentence_lengths.push(wc);
                }
                sent_start = i + 1;
            }
        }
        // Trailing text without terminator
        if sent_start < chars.len() {
            let sent_text: String = chars[sent_start..].iter().collect();
            let wc = sent_text.split(|c: char| !c.is_alphanumeric() && c != '\'')
                .filter(|w| !w.is_empty())
                .count();
            if wc > 0 {
                sentence_lengths.push(wc);
            }
        }

        results.push(PacingChapter {
            chapter_id: chapter.id,
            chapter_title: chapter.title.clone(),
            sentence_lengths,
        });
    }

    Ok(results)
}
