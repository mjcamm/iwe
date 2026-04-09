use serde::Serialize;

/// A dialogue span found in plain text.
#[derive(Serialize, Clone)]
pub struct DialogueSpan {
    pub text: String,         // the quoted text including quote marks
    pub inner_text: String,   // text inside the quotes (without quote marks)
    pub char_start: usize,    // char offset of opening quote in the plain text
    pub char_end: usize,      // char offset past closing quote
}

/// Extract all dialogue spans from plain text.
///
/// Handles all common quotation mark styles across platforms and locales:
/// - Straight double quotes: "..."
/// - Curly/smart double quotes: \u{201C}...\u{201D} (English)
/// - Low-high double quotes: \u{201E}...\u{201C} or \u{201E}...\u{201D} (German, some European)
/// - Guillemets: \u{00AB}...\u{00BB} and \u{00BB}...\u{00AB} (French, Russian)
/// - Single curly quotes: \u{2018}...\u{2019} (British English dialogue)
/// - CJK quotation marks: \u{300C}...\u{300D}
///
/// The function walks the text character by character, matching opening marks
/// to their corresponding closing marks. This avoids regex edge cases with
/// nested or mismatched quotes.
pub fn extract_dialogue(plain: &str) -> Vec<DialogueSpan> {
    let chars: Vec<char> = plain.chars().collect();
    let len = chars.len();
    let mut spans = Vec::new();

    // Define quote pairs: (opening, closing)
    // Order matters — check multi-purpose characters last
    let pairs: &[(char, char)] = &[
        ('\u{201C}', '\u{201D}'), // " "  curly double (English, most common smart quotes)
        ('\u{201E}', '\u{201D}'), // „ "  low-high double (German, Polish, Romanian)
        ('\u{201E}', '\u{201C}'), // „ "  low-high double (alternate German)
        ('\u{00AB}', '\u{00BB}'), // « »  guillemets (French, Russian, Spanish)
        ('\u{00BB}', '\u{00AB}'), // » «  reversed guillemets (Danish, some German)
        ('\u{2018}', '\u{2019}'), // ' '  curly single (British English dialogue)
        ('\u{201A}', '\u{2018}'), // ‚ '  low-high single (German)
        ('\u{201A}', '\u{2019}'), // ‚ '  low-high single (alternate)
        ('\u{300C}', '\u{300D}'), // 「  」 CJK corner brackets
        ('\u{300E}', '\u{300F}'), // 『  』 CJK double corner brackets
        ('"', '"'),               // "  "  straight double (plain ASCII)
    ];

    let mut i = 0;
    while i < len {
        let ch = chars[i];

        // Check if this character is an opening quote
        let mut matched = false;

        // For single curly quotes, skip if it looks like an apostrophe
        if ch == '\u{2018}' && is_likely_apostrophe(&chars, i) {
            i += 1;
            continue;
        }

        // Collect all possible closing characters for this opener
        let mut closers: Vec<char> = Vec::new();
        for &(open, close) in pairs {
            if ch == open && !closers.contains(&close) {
                closers.push(close);
            }
        }

        // For curly openers, also accept straight quote as closer (mixed quote handling)
        if ch == '\u{201C}' && !closers.contains(&'"') {
            closers.push('"');
        }

        if !closers.is_empty() {
            // For straight double quotes: check if this looks like an opening quote.
            // A closing quote is typically preceded by punctuation or a letter and followed
            // by a space/punctuation. An opening quote is typically preceded by whitespace
            // or start of text and followed by a letter.
            if ch == '"' {
                // Skip if this looks like a closing quote, not an opener:
                // preceded by a letter/punctuation and followed by space/punctuation/end
                let next_char = if i + 1 < len { chars[i + 1] } else { ' ' };
                if !next_char.is_alphabetic() && next_char != '\u{2014}' && next_char != '\'' {
                    i += 1;
                    continue;
                }
            }

            // Scan forward for the nearest valid closer
            if let Some(end_idx) = find_any_closing_quote(&chars, i + 1, &closers) {
                let text: String = chars[i..=end_idx].iter().collect();
                let inner: String = chars[i + 1..end_idx].iter().collect();

                // Only accept if there's actual content inside
                let trimmed = inner.trim();
                if !trimmed.is_empty() && trimmed.len() > 1 {
                    spans.push(DialogueSpan {
                        text,
                        inner_text: inner,
                        char_start: i,
                        char_end: end_idx + 1,
                    });
                }
                i = end_idx + 1;
                matched = true;
            }
        }

        if !matched {
            i += 1;
        }
    }

    spans
}

/// Find the nearest closing quote from a set of possible closers.
/// Caps at 2000 chars to prevent a mismatched quote from consuming the rest of the chapter.
///
/// Disambiguates the curly single closer ’ vs apostrophe (Bilbo’s, isn’t):
/// a real closer is preceded by sentence punctuation OR followed by a non-letter.
fn find_any_closing_quote(chars: &[char], start: usize, closers: &[char]) -> Option<usize> {
    let len = chars.len();
    let mut j = start;
    while j < len {
        if closers.contains(&chars[j]) {
            if chars[j] == '\u{2019}' {
                let before = if j > 0 { chars[j - 1] } else { ' ' };
                let after = if j + 1 < len { chars[j + 1] } else { '\n' };
                let punct_before = matches!(before, '.' | ',' | '!' | '?' | ';' | ':' | '\u{2014}' | '\u{2013}');
                if !punct_before && after.is_alphabetic() {
                    j += 1;
                    continue;
                }
            }
            return Some(j);
        }
        if j - start > 2000 {
            return None;
        }
        j += 1;
    }
    None
}

/// Find the closing quote, handling the special case where open == close (straight quotes).
fn find_closing_quote(chars: &[char], start: usize, close: char, open: char) -> Option<usize> {
    let len = chars.len();
    let mut j = start;

    // For straight double quotes where open == close, we need heuristics
    if open == close {
        // Look for the next occurrence of the same character
        while j < len {
            if chars[j] == close {
                return Some(j);
            }
            // Don't span across more than ~2000 chars (safety limit for runaway quotes)
            if j - start > 2000 {
                return None;
            }
            j += 1;
        }
        return None;
    }

    // For distinct open/close pairs, simple scan
    while j < len {
        if chars[j] == close {
            return Some(j);
        }
        // Don't span across more than ~2000 chars
        if j - start > 2000 {
            return None;
        }
        j += 1;
    }

    None
}

/// Check if a single curly opening quote is likely an apostrophe rather than dialogue.
/// Apostrophes appear mid-word (e.g. don't, it's, 'twas) or after a letter.
fn is_likely_apostrophe(chars: &[char], pos: usize) -> bool {
    // If preceded by a letter with no space, it's likely an apostrophe
    if pos > 0 && chars[pos - 1].is_alphabetic() {
        return true;
    }
    // If followed immediately by a lowercase letter and the char before is a letter or start,
    // check for contractions like 'twas, 'tis
    // Actually for opening single curly quote at start of word after whitespace, it IS dialogue
    // So only flag as apostrophe if preceded by a letter
    false
}

/// Count words in a string.
pub fn count_words(text: &str) -> usize {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .count()
}

/// A sentence extracted from plain text.
#[derive(Serialize, Clone)]
pub struct Sentence {
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    pub word_count: usize,
}

/// Common abbreviations that end with a period but don't end a sentence.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "ave", "blvd",
    "gen", "gov", "sgt", "cpl", "pvt", "lt", "col", "capt", "maj", "cmdr",
    "adm", "rev", "hon", "pres", "dept", "est", "approx",
    "vs", "etc", "inc", "ltd", "co", "corp", "assn",
    "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    "mon", "tue", "wed", "thu", "fri", "sat", "sun",
    "vol", "ch", "fig", "no", "op", "ed", "trans",
    "i.e", "e.g", "a.m", "p.m",
];

/// Extract sentences from plain text with robust handling of:
/// - Abbreviations (Mr. Mrs. Dr. etc.)
/// - Ellipsis (... and \u{2026})
/// - Decimal numbers (3.5, $1.99)
/// - Initials (J.K. Rowling, U.S.A.)
/// - Multiple punctuation (!!, ?!, ...)
/// - Dialogue attribution ("Hello," she said. → one sentence)
pub fn extract_sentences(plain: &str) -> Vec<Sentence> {
    let chars: Vec<char> = plain.chars().collect();
    let len = chars.len();
    let mut sentences = Vec::new();
    let mut sent_start: usize = 0;

    // Skip leading whitespace
    while sent_start < len && chars[sent_start].is_whitespace() {
        sent_start += 1;
    }

    let mut i = 0;
    while i < len {
        let ch = chars[i];

        if ch == '.' || ch == '?' || ch == '!' || ch == '\u{2026}' {
            // Skip ellipsis: multiple dots or the unicode ellipsis character
            if ch == '.' {
                // Check for ellipsis (... or ..)
                let mut dot_end = i;
                while dot_end + 1 < len && chars[dot_end + 1] == '.' {
                    dot_end += 1;
                }
                if dot_end > i {
                    // Multiple dots — treat as ellipsis, not sentence end
                    i = dot_end + 1;
                    continue;
                }

                // Check if this period follows a digit (decimal number like 3.5)
                if i > 0 && chars[i - 1].is_ascii_digit() {
                    if i + 1 < len && chars[i + 1].is_ascii_digit() {
                        i += 1;
                        continue;
                    }
                }

                // Check for abbreviation: extract the word before the period
                if is_abbreviation(&chars, i) {
                    i += 1;
                    continue;
                }

                // Check for initials pattern: single uppercase letter followed by period
                // e.g. "J. K. Rowling" or "U.S.A."
                if i > 0 && chars[i - 1].is_uppercase() {
                    // Look back to see if it's a single letter after whitespace/start/period
                    let before = if i >= 2 { chars[i - 2] } else { ' ' };
                    if before.is_whitespace() || before == '.' || i == 1 {
                        i += 1;
                        continue;
                    }
                }
            }

            if ch == '\u{2026}' {
                // Unicode ellipsis — not a sentence end
                i += 1;
                continue;
            }

            // Consume any additional punctuation (!! ?? ?! etc.)
            let mut end = i;
            while end + 1 < len && (chars[end + 1] == '.' || chars[end + 1] == '?' || chars[end + 1] == '!') {
                end += 1;
            }

            // Consume closing quotes after the punctuation
            let mut after = end + 1;
            while after < len && is_closing_quote(chars[after]) {
                after += 1;
            }

            // Check if what follows looks like a new sentence:
            // whitespace followed by an uppercase letter, a quote, or end of text
            let looks_like_end = if after >= len {
                true
            } else {
                // Skip whitespace to find the next non-space character
                let mut next = after;
                while next < len && chars[next].is_whitespace() {
                    next += 1;
                }
                if next >= len {
                    true
                } else {
                    let nc = chars[next];
                    nc.is_uppercase() || is_opening_quote(nc) || nc == '\u{2014}' || nc == '\u{2013}'
                }
            };

            if looks_like_end {
                let sent_text: String = chars[sent_start..after].iter().collect();
                let trimmed = sent_text.trim();
                if !trimmed.is_empty() {
                    let wc = count_words(trimmed);
                    if wc > 0 {
                        sentences.push(Sentence {
                            text: trimmed.to_string(),
                            char_start: sent_start,
                            char_end: after,
                            word_count: wc,
                        });
                    }
                }
                sent_start = after;
                // Skip whitespace for next sentence start
                while sent_start < len && chars[sent_start].is_whitespace() {
                    sent_start += 1;
                }
                i = sent_start;
                continue;
            }

            i = end + 1;
        } else {
            i += 1;
        }
    }

    // Trailing text without terminator
    if sent_start < len {
        let sent_text: String = chars[sent_start..].iter().collect();
        let trimmed = sent_text.trim();
        if !trimmed.is_empty() {
            let wc = count_words(trimmed);
            if wc > 0 {
                sentences.push(Sentence {
                    text: trimmed.to_string(),
                    char_start: sent_start,
                    char_end: len,
                    word_count: wc,
                });
            }
        }
    }

    sentences
}

/// Check if the period at position `dot_pos` follows a known abbreviation.
fn is_abbreviation(chars: &[char], dot_pos: usize) -> bool {
    // Walk backwards from the period to find the word
    let mut word_start = dot_pos;
    while word_start > 0 && chars[word_start - 1].is_alphabetic() {
        word_start -= 1;
    }
    // Also handle abbreviations with internal dots like "i.e", "e.g", "a.m"
    if word_start > 1 && chars[word_start - 1] == '.' && chars[word_start - 2].is_alphabetic() {
        word_start -= 2;
        while word_start > 0 && (chars[word_start - 1].is_alphabetic() || chars[word_start - 1] == '.') {
            word_start -= 1;
        }
    }

    if word_start == dot_pos {
        return false;
    }

    let word: String = chars[word_start..dot_pos].iter().collect();
    let lower = word.to_lowercase().replace('.', "");

    ABBREVIATIONS.contains(&lower.as_str())
}

fn is_closing_quote(ch: char) -> bool {
    matches!(ch, '"' | '\u{201D}' | '\u{2019}' | '\u{00BB}' | '\u{300D}' | '\u{300F}' | '\'' | '\u{00AB}')
}

fn is_opening_quote(ch: char) -> bool {
    matches!(ch, '"' | '\u{201C}' | '\u{2018}' | '\u{201E}' | '\u{00AB}' | '\u{00BB}' | '\u{300C}' | '\u{300E}' | '\'' )
}
