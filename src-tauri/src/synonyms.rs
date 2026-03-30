use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Serialize)]
pub struct SynonymResult {
    pub word: String,
    pub synonyms: Vec<String>,
    pub found: bool,
}

/// In-memory synonym lookup loaded from the bundled Moby Thesaurus file.
pub struct SynonymState {
    pub entries: Mutex<HashMap<String, Vec<String>>>,
}

/// Build the synonym map from the embedded Moby Thesaurus text file.
/// Format: each line is "root_word,syn1,syn2,syn3,..."
pub fn init_synonyms() -> SynonymState {
    let content = include_str!("../resources/mthesaur.txt");
    let mut entries: HashMap<String, Vec<String>> = HashMap::with_capacity(31000);

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, ',');
        let word = match parts.next() {
            Some(w) => w.trim().to_lowercase(),
            None => continue,
        };
        let syns_str = match parts.next() {
            Some(s) => s,
            None => continue,
        };

        let syns: Vec<String> = syns_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !syns.is_empty() {
            entries.insert(word, syns);
        }
    }

    SynonymState {
        entries: Mutex::new(entries),
    }
}

/// Try to find synonyms, with fallback to base forms.
fn lookup(entries: &HashMap<String, Vec<String>>, word: &str) -> Option<Vec<String>> {
    let lower = word.to_lowercase();

    // Exact match
    if let Some(syns) = entries.get(&lower) {
        return Some(syns.clone());
    }

    // Try stripping common suffixes to find base form
    let suffixes = ["s", "es", "ed", "d", "ing", "ly", "er", "est", "ness", "ment"];
    for suffix in &suffixes {
        if lower.ends_with(suffix) {
            let base = &lower[..lower.len() - suffix.len()];
            if base.len() >= 2 {
                if let Some(syns) = entries.get(base) {
                    return Some(syns.clone());
                }
            }
            // Try adding back 'e' (e.g., "making" -> "mak" -> "make")
            if *suffix == "ing" || *suffix == "ed" {
                let with_e = format!("{}e", base);
                if let Some(syns) = entries.get(&with_e) {
                    return Some(syns.clone());
                }
            }
            // Try changing 'i' back to 'y' (e.g., "happier" -> "happi" -> "happy")
            if (*suffix == "er" || *suffix == "est" || *suffix == "ed" || *suffix == "es") && base.ends_with('i') {
                let with_y = format!("{}y", &base[..base.len() - 1]);
                if let Some(syns) = entries.get(&with_y) {
                    return Some(syns.clone());
                }
            }
        }
    }

    None
}

#[tauri::command]
pub fn get_synonyms(
    syn_state: tauri::State<'_, SynonymState>,
    word: String,
) -> Result<SynonymResult, String> {
    let entries = syn_state.entries.lock().map_err(|e| e.to_string())?;

    match lookup(&entries, &word) {
        Some(syns) => {
            let limited: Vec<String> = syns.into_iter().take(200).collect();
            Ok(SynonymResult {
                word: word.to_lowercase(),
                synonyms: limited,
                found: true,
            })
        }
        None => Ok(SynonymResult {
            word: word.to_lowercase(),
            synonyms: Vec::new(),
            found: false,
        }),
    }
}
