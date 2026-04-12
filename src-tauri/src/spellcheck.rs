use std::collections::HashSet;
use std::sync::Mutex;
use serde::Serialize;
use spellbook::Dictionary;
use crate::db;
use crate::db::AppState;

/// Holds one or two spellbook dictionaries (en_US + en_GB).
/// The active language determines which is used for checking.
/// Dictionaries are loaded on a background thread to avoid blocking app startup.
pub struct SpellState {
    inner: std::sync::OnceLock<SpellInner>,
    pub language: Mutex<String>, // "en_US" or "en_GB"
}

struct SpellInner {
    en_us: Dictionary,
    en_gb: Dictionary,
}

impl SpellState {
    fn get(&self) -> &SpellInner {
        self.inner.get_or_init(|| {
            let en_us = Dictionary::new(EN_US_AFF, EN_US_DIC)
                .expect("Failed to load en_US dictionary");
            let en_gb = Dictionary::new(EN_GB_AFF, EN_GB_DIC)
                .expect("Failed to load en_GB dictionary");
            SpellInner { en_us, en_gb }
        })
    }
}

// Embedded dictionary files (SCOWL large variants)
static EN_US_AFF: &str = include_str!("../resources/dictionaries/en_US-large.aff");
static EN_US_DIC: &str = include_str!("../resources/dictionaries/en_US-large.dic");
static EN_GB_AFF: &str = include_str!("../resources/dictionaries/en_GB-large.aff");
static EN_GB_DIC: &str = include_str!("../resources/dictionaries/en_GB-large.dic");

#[derive(Serialize)]
pub struct CustomWord {
    pub id: i64,
    pub word: String,
    pub source: String,
}

/// Normalize curly apostrophes to straight ones for dictionary lookup.
fn normalize_apostrophes(word: &str) -> String {
    word.replace('\u{2019}', "'").replace('\u{2018}', "'")
}

/// Create spell state (dictionaries load lazily on first use).
pub fn init_spellcheck() -> SpellState {
    SpellState {
        inner: std::sync::OnceLock::new(),
        language: Mutex::new("en_US".to_string()),
    }
}

/// Check a word against the active dictionary.
fn is_correct(word: &str, spell: &SpellState, custom: &HashSet<String>) -> bool {
    let lower = word.to_lowercase();

    // Single characters are fine
    if lower.len() <= 1 {
        return true;
    }

    // All digits or contains digits — skip
    if lower.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }

    // Check custom words first (case-insensitive)
    let normalized = normalize_apostrophes(&lower);
    if custom.contains(&lower) || custom.contains(&normalized) {
        return true;
    }

    // Get the active dictionary
    let lang = spell.language.lock().unwrap_or_else(|e| e.into_inner());
    let dict = if *lang == "en_GB" { &spell.get().en_gb } else { &spell.get().en_us };

    // Check the dictionary — try original case, lowercase, and normalized apostrophes
    if dict.check(word) || dict.check(&lower) || dict.check(&normalized) {
        return true;
    }

    // Also check the word with first letter capitalized (proper nouns)
    if word.len() > 1 {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            let title_case: String = first.to_uppercase().chain(chars).collect();
            if dict.check(&title_case) {
                return true;
            }
        }
    }

    false
}

/// Get spelling suggestions using spellbook's built-in suggest.
fn get_suggestions(word: &str, spell: &SpellState, max: usize) -> Vec<String> {
    let normalized = normalize_apostrophes(word);
    let lang = spell.language.lock().unwrap_or_else(|e| e.into_inner());
    let dict = if *lang == "en_GB" { &spell.get().en_gb } else { &spell.get().en_us };

    let mut suggestions = Vec::new();
    dict.suggest(&normalized, &mut suggestions);
    suggestions.truncate(max);
    suggestions
}

// ---- Tauri Commands ----

#[tauri::command]
pub fn check_spelling(
    state: tauri::State<'_, AppState>,
    spell: tauri::State<'_, SpellState>,
    words: Vec<String>,
) -> Result<Vec<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;

    // Build a combined custom word set: custom_words + entity names + ignored words
    let mut custom: HashSet<String> = HashSet::new();

    for w in db::list_custom_words(conn).map_err(|e| e.to_string())? {
        custom.insert(w.to_lowercase());
    }

    for ent in db::list_entities(conn).map_err(|e| e.to_string())? {
        for w in ent.name.split_whitespace() {
            custom.insert(w.to_lowercase());
        }
        for alias in &ent.aliases {
            for w in alias.split_whitespace() {
                custom.insert(w.to_lowercase());
            }
        }
    }

    for w in db::list_ignored_words(conn).map_err(|e| e.to_string())? {
        custom.insert(w.to_lowercase());
    }

    // Only check correctness — no suggestions (fast path)
    let mut results = Vec::new();
    for word in &words {
        if !is_correct(word, &spell, &custom) {
            results.push(word.clone());
        }
    }

    Ok(results)
}

/// Debug: check a single word against both dictionaries directly (for troubleshooting)
#[tauri::command]
pub fn debug_spell_check(
    spell: tauri::State<'_, SpellState>,
    word: String,
) -> Result<String, String> {
    let lower = word.to_lowercase();
    let normalized = normalize_apostrophes(&lower);
    let us_check = spell.get().en_us.check(&lower);
    let gb_check = spell.get().en_gb.check(&lower);
    let us_orig = spell.get().en_us.check(&word);
    let gb_orig = spell.get().en_gb.check(&word);
    let lang = spell.language.lock().unwrap_or_else(|e| e.into_inner());
    Ok(format!(
        "word={:?} lower={:?} normalized={:?} lang={} us_lower={} gb_lower={} us_orig={} gb_orig={}",
        word, lower, normalized, *lang, us_check, gb_check, us_orig, gb_orig
    ))
}

/// Get spelling suggestions for a single word (on-demand, called from right-click)
#[tauri::command]
pub fn get_spell_suggestions(
    spell: tauri::State<'_, SpellState>,
    word: String,
) -> Result<Vec<String>, String> {
    Ok(get_suggestions(&word, &spell, 10))
}

#[tauri::command]
pub fn set_spell_language(
    spell: tauri::State<'_, SpellState>,
    language: String,
) -> Result<(), String> {
    if language != "en_US" && language != "en_GB" {
        return Err(format!("Unsupported language: {}. Use 'en_US' or 'en_GB'.", language));
    }
    let mut lang = spell.language.lock().map_err(|e| e.to_string())?;
    *lang = language;
    Ok(())
}

#[tauri::command]
pub fn get_spell_language(
    spell: tauri::State<'_, SpellState>,
) -> Result<String, String> {
    let lang = spell.language.lock().map_err(|e| e.to_string())?;
    Ok(lang.clone())
}

#[tauri::command]
pub fn add_to_dictionary(
    state: tauri::State<'_, AppState>,
    word: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::add_custom_word(conn, &word, "user").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_from_dictionary(
    state: tauri::State<'_, AppState>,
    word: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    db::remove_custom_word(conn, &word).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_custom_words(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CustomWord>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("No project open")?;
    let rows = db::get_custom_words_full(conn).map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|(id, word, source)| CustomWord { id, word, source }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spell() -> SpellState {
        init_spellcheck()
    }

    #[test]
    fn common_words_pass() {
        let spell = make_spell();
        let custom = HashSet::new();
        let words = vec!["the", "one", "lived", "boy", "who", "hello", "world",
                         "walked", "replied", "quickly", "after", "actually"];
        for w in words {
            assert!(is_correct(w, &spell, &custom), "Expected '{}' to be correct", w);
        }
    }

    #[test]
    fn allcaps_words_pass() {
        let spell = make_spell();
        let custom = HashSet::new();
        let words = vec!["THE", "ONE", "LIVED", "BOY", "WHO", "CHAPTER"];
        for w in words {
            assert!(is_correct(w, &spell, &custom), "Expected '{}' to be correct", w);
        }
    }

    #[test]
    fn misspelled_words_fail() {
        let spell = make_spell();
        let custom = HashSet::new();
        let words = vec!["asdfgh", "blorpx", "wonderous", "furios"];
        for w in words {
            assert!(!is_correct(w, &spell, &custom), "Expected '{}' to be misspelled", w);
        }
    }

    #[test]
    fn contractions_pass() {
        let spell = make_spell();
        let custom = HashSet::new();
        // Curly apostrophes (what TipTap produces)
        assert!(is_correct("she\u{2019}d", &spell, &custom));
        assert!(is_correct("don\u{2019}t", &spell, &custom));
        assert!(is_correct("won\u{2019}t", &spell, &custom));
        // Straight apostrophes
        assert!(is_correct("she'd", &spell, &custom));
        assert!(is_correct("don't", &spell, &custom));
    }

    #[test]
    fn custom_words_pass() {
        let spell = make_spell();
        let mut custom = HashSet::new();
        custom.insert("hogwarts".to_string());
        custom.insert("dumbledore".to_string());
        assert!(is_correct("Hogwarts", &spell, &custom));
        assert!(is_correct("dumbledore", &spell, &custom));
    }

    #[test]
    fn gb_dictionary_works() {
        let spell = make_spell();
        let custom = HashSet::new();
        // Switch to GB
        *spell.language.lock().unwrap() = "en_GB".to_string();
        assert!(is_correct("colour", &spell, &custom));
        assert!(is_correct("neighbour", &spell, &custom));
        assert!(!is_correct("color", &spell, &custom));
    }

    #[test]
    fn us_dictionary_works() {
        let spell = make_spell();
        let custom = HashSet::new();
        // Default is US
        assert!(is_correct("color", &spell, &custom));
        assert!(!is_correct("colour", &spell, &custom));
        assert!(is_correct("neighbor", &spell, &custom));
    }
}
