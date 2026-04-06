use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::db::{self, AppState};
use crate::scanner::{chapter_plain_text, scan_plain, build_terms_all, SearchTerm};
use crate::syllable_data;
use crate::text_utils;
use crate::wordlists;
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

    // Extract all sentences from all chapters using shared extractor
    let mut sentences: Vec<Sentence> = Vec::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let extracted = text_utils::extract_sentences(&plain);

        for s in &extracted {
            let words = normalize_sentence(&s.text);
            if words.len() >= min_words {
                sentences.push(Sentence {
                    text: s.text.clone(),
                    words,
                    chapter_id: chapter.id,
                    chapter_title: chapter.title.clone(),
                    char_pos: s.char_start,
                });
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

        // Split into sentences using shared extractor
        let extracted = text_utils::extract_sentences(&plain);
        let sentences: Vec<(usize, usize)> = extracted.iter()
            .map(|s| (s.char_start, s.char_end))
            .collect();

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

        // Dialogue vs narrative — using shared dialogue extraction
        let spans = text_utils::extract_dialogue(&plain);
        let dialogue_words: usize = spans.iter()
            .map(|s| text_utils::count_words(&s.inner_text))
            .sum();
        let narrative_words = total_words.saturating_sub(dialogue_words);

        // Sentences — using shared extractor
        let extracted = text_utils::extract_sentences(&plain);
        let sentences: Vec<usize> = extracted.iter().map(|s| s.word_count).collect();

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
    pub sentence_lengths: Vec<usize>,  // word count per sentence, in order
    pub sentence_starts: Vec<usize>,   // char offset hint for disambiguation
    pub sentence_texts: Vec<String>,   // first ~60 chars of each sentence for JS-side matching
}

#[tauri::command]
pub fn pacing_analysis(state: tauri::State<'_, AppState>) -> Result<Vec<PacingChapter>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let mut results = Vec::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let extracted = text_utils::extract_sentences(&plain);

        let sentence_lengths = extracted.iter().map(|s| s.word_count).collect();
        let sentence_starts = extracted.iter().map(|s| s.char_start).collect();
        let sentence_texts = extracted.iter().map(|s| {
            if s.text.len() > 60 { s.text.chars().take(60).collect::<String>() } else { s.text.clone() }
        }).collect();

        results.push(PacingChapter {
            chapter_id: chapter.id,
            chapter_title: chapter.title.clone(),
            sentence_lengths,
            sentence_starts,
            sentence_texts,
        });
    }

    Ok(results)
}

// ---- Dialogue attribution adverb analysis ----

/// Words ending in -ly that are NOT adverbs.
const LY_FALSE_POSITIVES: &[&str] = &[
    "family", "holy", "only", "early", "likely", "unlikely", "lonely", "friendly",
    "ugly", "lovely", "lively", "deadly", "elderly", "ghostly", "heavenly", "homely",
    "jelly", "jolly", "lily", "belly", "bully", "curly", "daily", "fly", "folly",
    "gully", "hilly", "italy", "july", "rally", "rely", "reply", "sally", "silly",
    "sly", "supply", "tally", "wily", "woolly", "apply", "comply", "imply",
    "multiply", "ally", "anomaly", "assembly", "butterfly", "certainly",
    "emily", "molly", "holly", "kelly", "billy", "polly", "dolly", "wally",
    "firefly", "dragonfly", "butterfly", "gadfly", "horsefly", "mayfly",
];

/// Words that are grammatically adverbs but NOT the style issue this tool targets.
/// These are frequency, time, or degree words that are normal in dialogue attribution
/// and no editor would flag. e.g. "You never told him?" is fine.
const ADVERB_IGNORE: &[&str] = &[
    // Time / frequency — normal in any context
    "never", "always", "once", "soon", "often", "already", "still", "yet",
    "now", "then", "ever", "again", "today", "tomorrow", "yesterday",
    "sometimes", "seldom", "rarely", "frequently", "occasionally",
    "afterwards", "eventually", "finally", "formerly", "immediately",
    "instantly", "lately", "meanwhile", "presently", "previously",
    "recently", "shortly", "simultaneously", "subsequently", "temporarily",
    "annually", "daily", "hourly", "monthly", "yearly",
    // Degree / emphasis — too common to flag
    "just", "also", "even", "almost", "quite", "rather", "very", "really",
    "too", "enough", "only", "already", "merely", "probably",
    "actually", "apparently", "certainly", "clearly", "definitely",
    "obviously", "perhaps", "possibly", "surely", "simply",
    // Negation / affirmation
    "not", "never", "no", "yes",
    // Conjunctive — normal prose connectors
    "however", "therefore", "moreover", "furthermore", "otherwise",
    "instead", "meanwhile", "nevertheless", "nonetheless", "anyway",
];

/// Common speech/dialogue tag verbs — all conjugations.
/// Sources: comprehensive dialogue tag verb lists for fiction writing.
const SPEECH_VERBS: &[&str] = &[
    // Core speech verbs
    "said", "say", "says", "saying",
    "asked", "ask", "asks", "asking",
    "told", "tell", "tells", "telling",
    "spoke", "speak", "speaks", "speaking", "spoken",
    "replied", "reply", "replies", "replying",
    "answered", "answer", "answers", "answering",
    "responded", "respond", "responds", "responding",
    "stated", "state", "states", "stating",
    "declared", "declare", "declares", "declaring",
    "announced", "announce", "announces", "announcing",
    "proclaimed", "proclaim", "proclaims", "proclaiming",
    "pronounced", "pronounce", "pronounces", "pronouncing",
    // Volume / intensity
    "whispered", "whisper", "whispers", "whispering",
    "murmured", "murmur", "murmurs", "murmuring",
    "mumbled", "mumble", "mumbles", "mumbling",
    "muttered", "mutter", "mutters", "muttering",
    "breathed", "breathe", "breathes", "breathing",
    "mouthed", "mouth", "mouths", "mouthing",
    "shouted", "shout", "shouts", "shouting",
    "yelled", "yell", "yells", "yelling",
    "screamed", "scream", "screams", "screaming",
    "screeched", "screech", "screeches", "screeching",
    "shrieked", "shriek", "shrieks", "shrieking",
    "bellowed", "bellow", "bellows", "bellowing",
    "roared", "roar", "roars", "roaring",
    "hollered", "holler", "hollers", "hollering",
    "bawled", "bawl", "bawls", "bawling",
    "called", "call", "calls", "calling",
    // Emotion / manner
    "cried", "cry", "cries", "crying",
    "sobbed", "sob", "sobs", "sobbing",
    "wailed", "wail", "wails", "wailing",
    "wept", "weep", "weeps", "weeping",
    "whimpered", "whimper", "whimpers", "whimpering",
    "whined", "whine", "whines", "whining",
    "sniveled", "snivel", "snivels", "sniveling", "snivelled", "snivelling",
    "moaned", "moan", "moans", "moaning",
    "groaned", "groan", "groans", "groaning",
    "sighed", "sigh", "sighs", "sighing",
    "gasped", "gasp", "gasps", "gasping",
    "panted", "pant", "pants", "panting",
    "laughed", "laugh", "laughs", "laughing",
    "giggled", "giggle", "giggles", "giggling",
    "chuckled", "chuckle", "chuckles", "chuckling",
    "chortled", "chortle", "chortles", "chortling",
    "cackled", "cackle", "cackles", "cackling",
    "snickered", "snicker", "snickers", "snickering",
    "sniggered", "snigger", "sniggers", "sniggering",
    // Aggression / sharpness
    "snapped", "snap", "snaps", "snapping",
    "snarled", "snarl", "snarls", "snarling",
    "growled", "growl", "growls", "growling",
    "hissed", "hiss", "hisses", "hissing",
    "spat", "spit", "spits", "spitting",
    "barked", "bark", "barks", "barking",
    "sneered", "sneer", "sneers", "sneering",
    "taunted", "taunt", "taunts", "taunting",
    "teased", "tease", "teases", "teasing",
    "sassed", "sass", "sasses", "sassing",
    "scolded", "scold", "scolds", "scolding",
    "upbraided", "upbraid", "upbraids", "upbraiding",
    // Hesitation / faltering
    "stammered", "stammer", "stammers", "stammering",
    "stuttered", "stutter", "stutters", "stuttering",
    "faltered", "falter", "falters", "faltering",
    "spluttered", "splutter", "splutters", "spluttering",
    "sputtered", "sputter", "sputters", "sputtering",
    "babbled", "babble", "babbles", "babbling",
    "blathered", "blather", "blathers", "blathering",
    "jabbered", "jabber", "jabbers", "jabbering",
    "rambled", "ramble", "rambles", "rambling",
    "prattled", "prattle", "prattles", "prattling",
    "blabbered", "blabber", "blabbers", "blabbering",
    "blurted", "blurt", "blurts", "blurting",
    // Requesting / persuading
    "demanded", "demand", "demands", "demanding",
    "insisted", "insist", "insists", "insisting",
    "pleaded", "plead", "pleads", "pleading",
    "begged", "beg", "begs", "begging",
    "implored", "implore", "implores", "imploring",
    "entreated", "entreat", "entreats", "entreating",
    "urged", "urge", "urges", "urging",
    "coaxed", "coax", "coaxes", "coaxing",
    "persuaded", "persuade", "persuades", "persuading",
    "commanded", "command", "commands", "commanding",
    "ordered", "order", "orders", "ordering",
    "instructed", "instruct", "instructs", "instructing",
    // Informing / explaining
    "explained", "explain", "explains", "explaining",
    "described", "describe", "describes", "describing",
    "informed", "inform", "informs", "informing",
    "mentioned", "mention", "mentions", "mentioning",
    "noted", "note", "notes", "noting",
    "observed", "observe", "observes", "observing",
    "reported", "report", "reports", "reporting",
    "revealed", "reveal", "reveals", "revealing",
    "recounted", "recount", "recounts", "recounting",
    "narrated", "narrate", "narrates", "narrating",
    "clarified", "clarify", "clarifies", "clarifying",
    "illustrated", "illustrate", "illustrates", "illustrating",
    "outlined", "outline", "outlines", "outlining",
    "summarised", "summarise", "summarises", "summarising",
    "summarized", "summarize", "summarizes", "summarizing",
    // Suggesting / opining
    "suggested", "suggest", "suggests", "suggesting",
    "commented", "comment", "comments", "commenting",
    "remarked", "remark", "remarks", "remarking",
    "added", "add", "adds", "adding",
    "advised", "advise", "advises", "advising",
    "recommended", "recommend", "recommends", "recommending",
    "hinted", "hint", "hints", "hinting",
    "intimated", "intimate", "intimates", "intimating",
    "surmised", "surmise", "surmises", "surmising",
    "wondered", "wonder", "wonders", "wondering",
    // Agreement / disagreement
    "agreed", "agree", "agrees", "agreeing",
    "conceded", "concede", "concedes", "conceding",
    "objected", "object", "objects", "objecting",
    "argued", "argue", "argues", "arguing",
    "disagreed", "disagree", "disagrees", "disagreeing",
    "protested", "protest", "protests", "protesting",
    "countered", "counter", "counters", "countering",
    "retorted", "retort", "retorts", "retorting",
    "rejoined", "rejoin", "rejoins", "rejoining",
    // Admitting / confessing
    "admitted", "admit", "admits", "admitting",
    "confessed", "confess", "confesses", "confessing",
    "confirmed", "confirm", "confirms", "confirming",
    "acknowledged", "acknowledge", "acknowledges", "acknowledging",
    "professed", "profess", "professes", "professing",
    "maintained", "maintain", "maintains", "maintaining",
    // Complaining / lamenting
    "complained", "complain", "complains", "complaining",
    "grumbled", "grumble", "grumbles", "grumbling",
    "bemoaned", "bemoan", "bemoans", "bemoaning",
    "nagged", "nag", "nags", "nagging",
    "ranted", "rant", "rants", "ranting",
    "blustered", "bluster", "blusters", "blustering",
    // Boasting / praising
    "boasted", "boast", "boasts", "boasting",
    "bragged", "brag", "brags", "bragging",
    "crowed", "crow", "crows", "crowing",
    "lauded", "laud", "lauds", "lauding",
    "praised", "praise", "praises", "praising",
    // Reassuring / promising
    "reassured", "reassure", "reassures", "reassuring",
    "promised", "promise", "promises", "promising",
    "assured", "assure", "assures", "assuring",
    "comforted", "comfort", "comforts", "comforting",
    // Warning / threatening
    "warned", "warn", "warns", "warning",
    "threatened", "threaten", "threatens", "threatening",
    "cautioned", "caution", "cautions", "cautioning",
    "goaded", "goad", "goads", "goading",
    // Other dialogue-adjacent
    "exclaimed", "exclaim", "exclaims", "exclaiming",
    "interrupted", "interrupt", "interrupts", "interrupting",
    "interjected", "interject", "interjects", "interjecting",
    "continued", "continue", "continues", "continuing",
    "began", "begin", "begins", "beginning",
    "repeated", "repeat", "repeats", "repeating",
    "corrected", "correct", "corrects", "correcting",
    "reminded", "remind", "reminds", "reminding",
    "offered", "offer", "offers", "offering",
    "lied", "lie", "lies", "lying",
    "bluffed", "bluff", "bluffs", "bluffing",
    "confided", "confide", "confides", "confiding",
    "quoted", "quote", "quotes", "quoting",
    "chanted", "chant", "chants", "chanting",
    "sang", "sing", "sings", "singing", "sung",
    "hummed", "hum", "hums", "humming",
    "purred", "purr", "purrs", "purring",
    "cooed", "coo", "coos", "cooing",
    "simpered", "simper", "simpers", "simpering",
    "drawled", "drawl", "drawls", "drawling",
    "droned", "drone", "drones", "droning",
    "intoned", "intone", "intones", "intoning",
    "slurred", "slur", "slurs", "slurring",
    "trilled", "trill", "trills", "trilling",
    "warbled", "warble", "warbles", "warbling",
    "chirped", "chirp", "chirps", "chirping",
    "squeaked", "squeak", "squeaks", "squeaking",
    "squealed", "squeal", "squeals", "squealing",
    "yelped", "yelp", "yelps", "yelping",
    "grunted", "grunt", "grunts", "grunting",
    "sniffed", "sniff", "sniffs", "sniffing",
    "coughed", "cough", "coughs", "coughing",
    "chattered", "chatter", "chatters", "chattering",
    "lectured", "lecture", "lectures", "lecturing",
    "preached", "preach", "preaches", "preaching",
    "decided", "decide", "decides", "deciding",
    "queried", "query", "queries", "querying",
    "questioned", "question", "questions", "questioning",
    "checked", "check", "checks", "checking",
    // Neutral / articulation
    "uttered", "utter", "utters", "uttering",
    "voiced", "voice", "voices", "voicing",
    "articulated", "articulate", "articulates", "articulating",
    "enunciated", "enunciate", "enunciates", "enunciating",
    "recited", "recite", "recites", "reciting",
    "elaborated", "elaborate", "elaborates", "elaborating",
    "asserted", "assert", "asserts", "asserting",
    "contended", "contend", "contends", "contending",
    "claimed", "claim", "claims", "claiming",
    "alleged", "allege", "alleges", "alleging",
    "expressed", "express", "expresses", "expressing",
    "persisted", "persist", "persists", "persisting",
    "specified", "specify", "specifies", "specifying",
    "related", "relate", "relates", "relating",
    "greeted", "greet", "greets", "greeting",
    // Questioning / wondering
    "inquired", "inquire", "inquires", "inquiring",
    "interrogated", "interrogate", "interrogates", "interrogating",
    "probed", "probe", "probes", "probing",
    "speculated", "speculate", "speculates", "speculating",
    "mused", "muse", "muses", "musing",
    "pondered", "ponder", "ponders", "pondering",
    "guessed", "guess", "guesses", "guessing",
    "challenged", "challenge", "challenges", "challenging",
    "pressed", "press", "presses", "pressing",
    // Agreeing / affirming
    "affirmed", "affirm", "affirms", "affirming",
    "avowed", "avow", "avows", "avowing",
    "accepted", "accept", "accepts", "accepting",
    "approved", "approve", "approves", "approving",
    "concurred", "concur", "concurs", "concurring",
    "consented", "consent", "consents", "consenting",
    // Accusing / condemning
    "accused", "accuse", "accuses", "accusing",
    "condemned", "condemn", "condemns", "condemning",
    "criticized", "criticize", "criticizes", "criticizing",
    "criticised", "criticise", "criticises", "criticising",
    "disputed", "dispute", "disputes", "disputing",
    // Happy / enthusiastic
    "cheered", "cheer", "cheers", "cheering",
    "gushed", "gush", "gushes", "gushing",
    "joked", "joke", "jokes", "joking",
    "quipped", "quip", "quips", "quipping",
    "tittered", "titter", "titters", "tittering",
    "beamed", "beam", "beams", "beaming",
    // Sad / distressed
    "lamented", "lament", "laments", "lamenting",
    "grieved", "grieve", "grieves", "grieving",
    "fretted", "fret", "frets", "fretting",
    "apologized", "apologize", "apologizes", "apologizing",
    "apologised", "apologise", "apologises", "apologising",
    "blubbered", "blubber", "blubbers", "blubbering",
    "sniffled", "sniffle", "sniffles", "sniffling",
    "keened", "keen", "keens", "keening",
    // Angry / forceful
    "fumed", "fume", "fumes", "fuming",
    "raged", "rage", "rages", "raging",
    "cursed", "curse", "curses", "cursing",
    "swore", "swear", "swears", "swearing", "sworn",
    "thundered", "thunder", "thunders", "thundering",
    "reprimanded", "reprimand", "reprimands", "reprimanding",
    "chastised", "chastise", "chastises", "chastising",
    "insulted", "insult", "insults", "insulting",
    // Mocking / derisive
    "derided", "deride", "derides", "deriding",
    "jeered", "jeer", "jeers", "jeering",
    "heckled", "heckle", "heckles", "heckling",
    "ridiculed", "ridicule", "ridicules", "ridiculing",
    "scoffed", "scoff", "scoffs", "scoffing",
    "mocked", "mock", "mocks", "mocking",
    "smirked", "smirk", "smirks", "smirking",
    // Physical / bodily speech sounds
    "croaked", "croak", "croaks", "croaking",
    "rasped", "rasp", "rasps", "rasping",
    "wheezed", "wheeze", "wheezes", "wheezing",
    "choked", "choke", "chokes", "choking",
    "gulped", "gulp", "gulps", "gulping",
    "hiccuped", "hiccup", "hiccups", "hiccuping", "hiccupped", "hiccupping",
    "yawned", "yawn", "yawns", "yawning",
    "exhaled", "exhale", "exhales", "exhaling",
    "quavered", "quaver", "quavers", "quavering",
    "howled", "howl", "howls", "howling",
    // Loud / commanding
    "boomed", "boom", "booms", "booming",
    "trumpeted", "trumpet", "trumpets", "trumpeting",
    "dictated", "dictate", "dictates", "dictating",
    "directed", "direct", "directs", "directing",
    "encouraged", "encourage", "encourages", "encouraging",
    "exhorted", "exhort", "exhorts", "exhorting",
    // Pompous / verbose
    "sermonized", "sermonize", "sermonizes", "sermonizing",
    "moralized", "moralize", "moralizes", "moralizing",
    "gloated", "gloat", "gloats", "gloating",
    // Revelation / disclosure
    "disclosed", "disclose", "discloses", "disclosing",
    "divulged", "divulge", "divulges", "divulging",
    "testified", "testify", "testifies", "testifying",
    // Concession / supposition
    "concluded", "conclude", "concludes", "concluding",
    "supposed", "suppose", "supposes", "supposing",
    "reckoned", "reckon", "reckons", "reckoning",
    "predicted", "predict", "predicts", "predicting",
    "implied", "imply", "implies", "implying",
    // Persuasion
    "cajoled", "cajole", "cajoles", "cajoling",
    "proposed", "propose", "proposes", "proposing",
    "invited", "invite", "invites", "inviting",
];

#[derive(Serialize)]
pub struct AdverbInstance {
    pub adverb: String,
    pub speech_verb: String,
    pub dialogue_snippet: String, // the dialogue text nearby
    pub context: String,          // full sentence/attribution for display
    pub chapter_id: i64,
    pub chapter_title: String,
    pub char_position: usize,     // position of the adverb
    pub redundant: bool,          // adverb is redundant with the verb (e.g. "whispered softly")
}

#[derive(Serialize)]
pub struct AdverbFrequency {
    pub word: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct AdverbAnalysis {
    pub total_dialogue_spans: usize,
    pub attributions_with_adverbs: usize,
    pub total_instances: usize,
    pub redundant_count: usize,
    pub top_adverbs: Vec<AdverbFrequency>,
    pub instances: Vec<AdverbInstance>,
}

/// Verb groups by what they already imply, paired with adverbs that are redundant.
const REDUNDANT_GROUPS: &[(&[&str], &[&str])] = &[
    // Quiet verbs → quiet adverbs
    (&["whisper", "whispered", "whispers", "whispering",
      "murmur", "murmured", "murmurs", "murmuring",
      "mumble", "mumbled", "mumbles", "mumbling",
      "mutter", "muttered", "mutters", "muttering",
      "breathe", "breathed", "breathes", "breathing",
      "mouth", "mouthed", "mouths", "mouthing"],
     &["softly", "quietly", "silently", "gently"]),
    // Loud verbs → loud adverbs
    (&["shout", "shouted", "shouts", "shouting",
      "yell", "yelled", "yells", "yelling",
      "scream", "screamed", "screams", "screaming",
      "screech", "screeched", "screeches", "screeching",
      "shriek", "shrieked", "shrieks", "shrieking",
      "bellow", "bellowed", "bellows", "bellowing",
      "roar", "roared", "roars", "roaring",
      "holler", "hollered", "hollers", "hollering",
      "bawl", "bawled", "bawls", "bawling",
      "boom", "boomed", "booms", "booming",
      "thunder", "thundered", "thunders", "thundering"],
     &["loudly", "noisily", "shrilly"]),
    // Angry verbs → angry adverbs
    (&["snap", "snapped", "snaps", "snapping",
      "snarl", "snarled", "snarls", "snarling",
      "growl", "growled", "growls", "growling",
      "hiss", "hissed", "hisses", "hissing",
      "bark", "barked", "barks", "barking",
      "spit", "spat", "spits", "spitting",
      "fume", "fumed", "fumes", "fuming",
      "rage", "raged", "rages", "raging"],
     &["angrily", "furiously", "fiercely", "sharply", "viciously"]),
    // Sad verbs → sad adverbs
    (&["sob", "sobbed", "sobs", "sobbing",
      "weep", "wept", "weeps", "weeping",
      "wail", "wailed", "wails", "wailing",
      "whimper", "whimpered", "whimpers", "whimpering",
      "whine", "whined", "whines", "whining",
      "snivel", "sniveled", "snivels", "sniveling", "snivelled", "snivelling",
      "moan", "moaned", "moans", "moaning",
      "lament", "lamented", "laments", "lamenting",
      "grieve", "grieved", "grieves", "grieving",
      "blubber", "blubbered", "blubbers", "blubbering",
      "keen", "keened", "keens", "keening"],
     &["sadly", "miserably", "sorrowfully", "unhappily", "woefully"]),
    // Happy verbs → happy adverbs
    (&["giggle", "giggled", "giggles", "giggling",
      "chuckle", "chuckled", "chuckles", "chuckling",
      "chortle", "chortled", "chortles", "chortling",
      "laugh", "laughed", "laughs", "laughing",
      "cackle", "cackled", "cackles", "cackling",
      "titter", "tittered", "titters", "tittering",
      "gush", "gushed", "gushes", "gushing"],
     &["happily", "cheerfully", "gleefully", "joyfully", "merrily"]),
    // Hesitant verbs → hesitant adverbs
    (&["stammer", "stammered", "stammers", "stammering",
      "stutter", "stuttered", "stutters", "stuttering",
      "falter", "faltered", "falters", "faltering",
      "splutter", "spluttered", "splutters", "spluttering",
      "sputter", "sputtered", "sputters", "sputtering"],
     &["nervously", "anxiously", "hesitantly", "uncertainly"]),
    // Desperate verbs → desperate adverbs
    (&["plead", "pleaded", "pleads", "pleading",
      "beg", "begged", "begs", "begging",
      "implore", "implored", "implores", "imploring",
      "entreat", "entreated", "entreats", "entreating"],
     &["desperately", "urgently", "frantically"]),
    // Fast verbs → fast adverbs
    (&["rush", "rushed", "rushes", "rushing",
      "blurt", "blurted", "blurts", "blurting"],
     &["quickly", "hastily", "hurriedly", "rapidly"]),
    // Breathless verbs → breathless adverbs
    (&["gasp", "gasped", "gasps", "gasping",
      "pant", "panted", "pants", "panting",
      "wheeze", "wheezed", "wheezes", "wheezing",
      "choke", "choked", "chokes", "choking"],
     &["breathlessly"]),
    // Monotone verbs → monotone adverbs
    (&["drone", "droned", "drones", "droning",
      "intone", "intoned", "intones", "intoning"],
     &["monotonously", "flatly", "dully"]),
    // Mocking verbs → mocking adverbs
    (&["sneer", "sneered", "sneers", "sneering",
      "taunt", "taunted", "taunts", "taunting",
      "mock", "mocked", "mocks", "mocking",
      "jeer", "jeered", "jeers", "jeering",
      "deride", "derided", "derides", "deriding",
      "ridicule", "ridiculed", "ridicules", "ridiculing",
      "scoff", "scoffed", "scoffs", "scoffing",
      "heckle", "heckled", "heckles", "heckling"],
     &["mockingly", "scornfully", "derisively", "contemptuously"]),
];

fn is_redundant(verb: &str, adverb: &str) -> bool {
    let v = verb.to_lowercase();
    let a = adverb.to_lowercase();
    REDUNDANT_GROUPS.iter().any(|(verbs, adverbs)| {
        verbs.contains(&v.as_str()) && adverbs.contains(&a.as_str())
    })
}

/// Tokenize a string slice into words with their char positions relative to `base_offset`.
fn tokenize_with_positions(text: &str, base_offset: usize) -> Vec<(String, String, usize)> {
    // Returns (original_word, lowercase, char_pos)
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphanumeric() || chars[i] == '\'' || chars[i] == '\u{2019}' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '\'' || chars[i] == '\u{2019}') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let lower = word.to_lowercase();
            tokens.push((word, lower, base_offset + start));
        } else {
            i += 1;
        }
    }
    tokens
}

#[derive(Serialize)]
pub struct DialogueRange {
    pub char_start: usize,
    pub char_end: usize,
}

#[tauri::command]
pub fn get_chapter_dialogue(
    state: tauri::State<'_, AppState>,
    chapter_id: i64,
) -> Result<Vec<DialogueRange>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;
    let chapter = chapters.iter().find(|c| c.id == chapter_id).ok_or("Chapter not found")?;
    let plain = chapter_plain_text(chapter)?;
    let spans = text_utils::extract_dialogue(&plain);
    Ok(spans.iter().map(|s| DialogueRange { char_start: s.char_start, char_end: s.char_end }).collect())
}

#[tauri::command]
pub fn debug_dialogue_spans(
    state: tauri::State<'_, AppState>,
    chapter_id: i64,
    around_pos: usize,
) -> Result<serde_json::Value, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;
    let chapter = chapters.iter().find(|c| c.id == chapter_id).ok_or("Chapter not found")?;

    let plain = chapter_plain_text(chapter)?;
    let chars: Vec<char> = plain.chars().collect();
    let spans = text_utils::extract_dialogue(&plain);

    // Show chars around the position
    let ctx_start = around_pos.saturating_sub(100);
    let ctx_end = (around_pos + 100).min(chars.len());
    let context: String = chars[ctx_start..ctx_end].iter().collect();

    // Show the raw char codes around the position
    let code_start = around_pos.saturating_sub(20);
    let code_end = (around_pos + 20).min(chars.len());
    let char_codes: Vec<String> = chars[code_start..code_end].iter()
        .enumerate()
        .map(|(i, c)| format!("{}:U+{:04X}({})", code_start + i, *c as u32, c))
        .collect();

    // Find nearby spans
    let nearby: Vec<serde_json::Value> = spans.iter()
        .filter(|s| s.char_end > around_pos.saturating_sub(200) && s.char_start < around_pos + 200)
        .map(|s| serde_json::json!({
            "start": s.char_start,
            "end": s.char_end,
            "text": if s.text.chars().count() > 80 { format!("{}...", s.text.chars().take(77).collect::<String>()) } else { s.text.clone() },
        }))
        .collect();

    // Is the position inside any span?
    let inside = spans.iter().any(|s| around_pos >= s.char_start && around_pos < s.char_end);

    Ok(serde_json::json!({
        "position": around_pos,
        "inside_dialogue": inside,
        "context": context,
        "char_codes": char_codes,
        "total_spans_in_chapter": spans.len(),
        "nearby_spans": nearby,
    }))
}

#[tauri::command]
pub fn adverb_analysis(state: tauri::State<'_, AppState>) -> Result<AdverbAnalysis, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let known_adverbs: HashSet<&str> = wordlists::ADVERBS.iter().copied().collect();
    let false_positives: HashSet<&str> = LY_FALSE_POSITIVES.iter().copied().collect();
    let speech_verbs: HashSet<&str> = SPEECH_VERBS.iter().copied().collect();

    let mut total_dialogue_spans: usize = 0;
    let mut instances: Vec<AdverbInstance> = Vec::new();
    let mut adverb_counts: HashMap<String, usize> = HashMap::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let chars: Vec<char> = plain.chars().collect();
        let dialogue_spans = text_utils::extract_dialogue(&plain);
        let mut seen_positions: HashSet<usize> = HashSet::new();

        total_dialogue_spans += dialogue_spans.len();

        // Build narration segments — everything NOT inside dialogue
        let mut narration_segments: Vec<(usize, usize)> = Vec::new();
        let mut pos = 0;
        for span in &dialogue_spans {
            if span.char_start > pos {
                narration_segments.push((pos, span.char_start));
            }
            pos = span.char_end;
        }
        if pos < chars.len() {
            narration_segments.push((pos, chars.len()));
        }

        // Tokenize all narration into words with positions
        let mut narration_tokens: Vec<(String, String, usize)> = Vec::new();
        for &(seg_start, seg_end) in &narration_segments {
            let seg_text: String = chars[seg_start..seg_end].iter().collect();
            narration_tokens.extend(tokenize_with_positions(&seg_text, seg_start));
        }

        // For each dialogue span, examine the narration within 60 chars before/after
        for span in &dialogue_spans {
            let zone_start = span.char_start.saturating_sub(60);
            let zone_end = (span.char_end + 60).min(chars.len());

            // Get narration tokens in this zone
            let zone_tokens: Vec<(usize, &(String, String, usize))> = narration_tokens.iter()
                .enumerate()
                .filter(|(_, (_, _, cp))| *cp >= zone_start && *cp < zone_end)
                .collect();

            // Check for speech verbs in the zone
            let has_verb = zone_tokens.iter()
                .any(|(_, (_, lower, _))| speech_verbs.contains(lower.as_str()));
            if !has_verb { continue; }

            // Find adverbs in the zone
            for &(ti, &(ref word, ref lower, char_pos)) in &zone_tokens {
                let is_adverb = if known_adverbs.contains(lower.as_str()) {
                    true
                } else if lower.ends_with("ly") && lower.len() > 3 {
                    !false_positives.contains(lower.as_str())
                } else {
                    false
                };

                if !is_adverb { continue; }
                if ADVERB_IGNORE.contains(&lower.as_str()) { continue; }
                if seen_positions.contains(&char_pos) { continue; }

                // Find nearest speech verb within 5 tokens in the full narration token list
                let window_start = ti.saturating_sub(5);
                let window_end = (ti + 6).min(narration_tokens.len());
                let mut closest_verb: Option<&(String, String, usize)> = None;
                let mut closest_dist = usize::MAX;

                for j in window_start..window_end {
                    if j == ti { continue; }
                    let (_, ref jlower, jpos) = narration_tokens[j];
                    if speech_verbs.contains(jlower.as_str()) {
                        let dist = (char_pos as i64 - jpos as i64).unsigned_abs() as usize;
                        if dist < closest_dist {
                            closest_dist = dist;
                            closest_verb = Some(&narration_tokens[j]);
                        }
                    }
                }

                let Some(verb) = closest_verb else { continue; };

                seen_positions.insert(char_pos);

                let redundant = is_redundant(&verb.1, lower);

                // Build context centered on the adverb, ~80 chars each side
                let raw_start = char_pos.saturating_sub(80);
                let raw_end = (char_pos + word.len() + 80).min(chars.len());
                let ctx_start = if raw_start == 0 { 0 } else {
                    (raw_start..char_pos).find(|&k| chars[k].is_whitespace())
                        .map(|k| k + 1).unwrap_or(raw_start)
                };
                let ctx_end = if raw_end >= chars.len() { chars.len() } else {
                    (char_pos + word.len()..raw_end).rev().find(|&k| chars[k].is_whitespace())
                        .unwrap_or(raw_end)
                };
                let context: String = chars[ctx_start..ctx_end].iter().collect();

                let dialogue_snippet = if span.inner_text.chars().count() > 50 {
                    format!("{}...", span.inner_text.chars().take(47).collect::<String>())
                } else {
                    span.inner_text.clone()
                };

                *adverb_counts.entry(lower.clone()).or_insert(0) += 1;

                instances.push(AdverbInstance {
                    adverb: word.clone(),
                    speech_verb: verb.0.clone(),
                    dialogue_snippet,
                    context,
                    chapter_id: chapter.id,
                    chapter_title: chapter.title.clone(),
                    char_position: char_pos,
                    redundant,
                });
            }
        }
    }

    let total_instances = instances.len();
    let redundant_count = instances.iter().filter(|i| i.redundant).count();
    let attributions_with_adverbs = {
        // Count unique (chapter_id, dialogue char_start) pairs
        let mut seen = HashSet::new();
        for inst in &instances {
            // Group by approximate dialogue position
            seen.insert((inst.chapter_id, inst.char_position / 100));
        }
        seen.len()
    };

    let mut top_adverbs: Vec<AdverbFrequency> = adverb_counts.into_iter()
        .map(|(word, count)| AdverbFrequency { word, count })
        .collect();
    top_adverbs.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(AdverbAnalysis {
        total_dialogue_spans,
        attributions_with_adverbs,
        total_instances,
        redundant_count,
        top_adverbs,
        instances,
    })
}

// ---- Readability analysis (Flesch-Kincaid) ----

/// Count syllables for a word. Uses the compiled phf lookup first, falls back to
/// vowel-group heuristic with silent-e adjustment.
fn count_syllables(word: &str) -> usize {
    let lower = word.to_lowercase();
    let clean: &str = lower.trim_matches(|c: char| !c.is_alphabetic());
    if clean.is_empty() { return 1; }

    if let Some(&count) = syllable_data::SYLLABLE_MAP.get(clean) {
        return count as usize;
    }

    // Vowel-group heuristic
    let chars: Vec<char> = clean.chars().collect();
    let len = chars.len();
    let mut count = 0usize;
    let mut prev_vowel = false;

    for (i, &ch) in chars.iter().enumerate() {
        let is_vowel = matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if is_vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_vowel;
        let _ = i; // used below
    }

    // Silent-e: if word ends with 'e' (not "le") and we counted > 1, subtract 1
    if len > 2 && chars[len - 1] == 'e' && !matches!(chars[len - 2], 'a' | 'e' | 'i' | 'o' | 'u')
        && !(chars[len - 2] == 'l')
    {
        if count > 1 { count -= 1; }
    }

    // Words ending in -ed: usually not a separate syllable unless preceded by t or d
    if len > 3 && chars[len - 2] == 'e' && chars[len - 1] == 'd' {
        let before_ed = chars[len - 3];
        if before_ed != 't' && before_ed != 'd' {
            if count > 1 { count -= 1; }
        }
    }

    if count == 0 { 1 } else { count }
}

#[derive(Serialize)]
pub struct ReadabilityChapter {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub grade_level: f64,
    pub total_words: usize,
    pub total_sentences: usize,
    pub total_syllables: usize,
    pub avg_words_per_sentence: f64,
    pub avg_syllables_per_word: f64,
    /// Per-sentence data for the detail chart
    pub sentence_grades: Vec<f64>,
    pub sentence_starts: Vec<usize>,
    pub sentence_texts: Vec<String>,
}

#[derive(Serialize)]
pub struct ReadabilityAnalysis {
    pub manuscript_grade: f64,
    pub manuscript_words: usize,
    pub manuscript_sentences: usize,
    pub manuscript_syllables: usize,
    pub chapters: Vec<ReadabilityChapter>,
}

fn flesch_kincaid_grade(words: usize, sentences: usize, syllables: usize) -> f64 {
    if sentences == 0 || words == 0 { return 0.0; }
    let wps = words as f64 / sentences as f64;
    let spw = syllables as f64 / words as f64;
    0.39 * wps + 11.8 * spw - 15.59
}

#[tauri::command]
pub fn readability_analysis(state: tauri::State<'_, AppState>) -> Result<ReadabilityAnalysis, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let mut total_words = 0usize;
    let mut total_sentences = 0usize;
    let mut total_syllables = 0usize;
    let mut chapter_results = Vec::new();

    for chapter in &chapters {
        let plain = chapter_plain_text(chapter)?;
        let sentences = text_utils::extract_sentences(&plain);

        let mut ch_words = 0usize;
        let mut ch_syllables = 0usize;
        let mut sentence_grades = Vec::new();
        let mut sentence_starts = Vec::new();
        let mut sentence_texts = Vec::new();

        for sent in &sentences {
            let words_in_sent: Vec<&str> = sent.text
                .split(|c: char| !c.is_alphanumeric() && c != '\'')
                .filter(|w| !w.is_empty())
                .collect();
            let wc = words_in_sent.len();
            let sc: usize = words_in_sent.iter().map(|w| count_syllables(w)).sum();

            ch_words += wc;
            ch_syllables += sc;

            // Per-sentence FK grade (using 1 sentence)
            let grade = flesch_kincaid_grade(wc, 1, sc);
            sentence_grades.push(grade);
            sentence_starts.push(sent.char_start);
            let text = if sent.text.chars().count() > 60 {
                sent.text.chars().take(60).collect::<String>()
            } else {
                sent.text.clone()
            };
            sentence_texts.push(text);
        }

        let ch_sentences = sentences.len();
        let grade = flesch_kincaid_grade(ch_words, ch_sentences, ch_syllables);

        total_words += ch_words;
        total_sentences += ch_sentences;
        total_syllables += ch_syllables;

        chapter_results.push(ReadabilityChapter {
            chapter_id: chapter.id,
            chapter_title: chapter.title.clone(),
            grade_level: (grade * 10.0).round() / 10.0,
            total_words: ch_words,
            total_sentences: ch_sentences,
            total_syllables: ch_syllables,
            avg_words_per_sentence: if ch_sentences > 0 { (ch_words as f64 / ch_sentences as f64 * 10.0).round() / 10.0 } else { 0.0 },
            avg_syllables_per_word: if ch_words > 0 { (ch_syllables as f64 / ch_words as f64 * 100.0).round() / 100.0 } else { 0.0 },
            sentence_grades,
            sentence_starts,
            sentence_texts,
        });
    }

    let manuscript_grade = flesch_kincaid_grade(total_words, total_sentences, total_syllables);

    Ok(ReadabilityAnalysis {
        manuscript_grade: (manuscript_grade * 10.0).round() / 10.0,
        manuscript_words: total_words,
        manuscript_sentences: total_sentences,
        manuscript_syllables: total_syllables,
        chapters: chapter_results,
    })
}

// ---- Paragraph length analysis ----

#[derive(Serialize)]
pub struct ParagraphInfo {
    pub word_count: usize,
    pub char_start: usize,
    pub preview: String,
    pub monotonous: bool, // part of a 3+ run within 15% of each other
}

#[derive(Serialize)]
pub struct ParagraphChapter {
    pub chapter_id: i64,
    pub chapter_title: String,
    pub paragraphs: Vec<ParagraphInfo>,
    pub total_paragraphs: usize,
    pub avg_length: f64,
    pub std_dev: f64,
    pub variation_pct: f64, // std_dev / avg * 100
}

#[derive(Serialize)]
pub struct ParagraphAnalysis {
    pub chapters: Vec<ParagraphChapter>,
}

fn flag_monotonous_runs(paragraphs: &mut [ParagraphInfo]) {
    let len = paragraphs.len();
    if len < 3 { return; }

    // Find runs of 3+ paragraphs where each is within 15% of the run's average
    let mut i = 0;
    while i < len {
        let mut j = i + 1;
        // Extend the run as long as consecutive paragraphs are within 15% of each other
        while j < len {
            let wc_j = paragraphs[j].word_count as f64;
            let wc_prev = paragraphs[j - 1].word_count as f64;
            if wc_prev == 0.0 || wc_j == 0.0 {
                break;
            }
            let ratio = if wc_j > wc_prev { wc_j / wc_prev } else { wc_prev / wc_j };
            if ratio <= 1.15 {
                j += 1;
            } else {
                break;
            }
        }
        if j - i >= 3 {
            for k in i..j {
                paragraphs[k].monotonous = true;
            }
        }
        i = if j > i + 1 { j } else { i + 1 };
    }
}

#[tauri::command]
pub fn paragraph_length_analysis(state: tauri::State<'_, AppState>) -> Result<ParagraphAnalysis, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let chapters = db::list_chapters(conn).map_err(|e| e.to_string())?;

    let mut chapter_results = Vec::new();

    for chapter in &chapters {
        let doc = ydoc::load_doc(&chapter.content)?;
        let text = ydoc::extract_text_with_breaks(&doc);

        // Split on double newlines (extract_text_with_breaks uses \n\n between blocks)
        // Also handle \r\n\r\n for Windows compatibility
        let raw_paragraphs: Vec<&str> = text.split("\n\n")
            .flat_map(|s| s.split("\r\n\r\n"))
            .collect();

        let mut char_offset = 0usize;
        let mut paragraphs: Vec<ParagraphInfo> = Vec::new();

        for raw in &raw_paragraphs {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                char_offset += raw.len() + 2; // +2 for the \n\n separator
                continue;
            }

            let wc = text_utils::count_words(trimmed);
            if wc == 0 {
                char_offset += raw.len() + 2;
                continue;
            }

            let preview = if trimmed.chars().count() > 80 {
                trimmed.chars().take(80).collect::<String>()
            } else {
                trimmed.to_string()
            };

            paragraphs.push(ParagraphInfo {
                word_count: wc,
                char_start: char_offset,
                preview,
                monotonous: false,
            });

            char_offset += raw.len() + 2;
        }

        // Flag monotonous runs
        flag_monotonous_runs(&mut paragraphs);

        // Stats
        let total = paragraphs.len();
        let avg = if total > 0 {
            paragraphs.iter().map(|p| p.word_count as f64).sum::<f64>() / total as f64
        } else { 0.0 };

        let std_dev = if total > 1 {
            let variance = paragraphs.iter()
                .map(|p| (p.word_count as f64 - avg).powi(2))
                .sum::<f64>() / total as f64;
            variance.sqrt()
        } else { 0.0 };

        let variation_pct = if avg > 0.0 { std_dev / avg * 100.0 } else { 0.0 };

        chapter_results.push(ParagraphChapter {
            chapter_id: chapter.id,
            chapter_title: chapter.title.clone(),
            paragraphs,
            total_paragraphs: total,
            avg_length: (avg * 10.0).round() / 10.0,
            std_dev: (std_dev * 10.0).round() / 10.0,
            variation_pct: (variation_pct * 10.0).round() / 10.0,
        });
    }

    Ok(ParagraphAnalysis {
        chapters: chapter_results,
    })
}
