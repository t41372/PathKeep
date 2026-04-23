//! Text normalization and tokenizer helpers for deterministic analysis.
//!
//! ## Responsibilities
//! - Normalize whitespace consistently before query and title matching.
//! - Tokenize Latin, CJK, kana, and Hangul text for deterministic grouping.
//! - Keep low-value Latin stop words out of lexicon matching inputs.
//!
//! ## Not responsible for
//! - Parsing URLs or extracting query parameters.
//! - Applying taxonomy rule packs.
//! - Persisting tokens or building search indexes.
//!
//! ## Dependencies
//! - Unicode scalar inspection from the Rust standard library.
//!
//! ## Performance notes
//! - Tokenization is linear in the input string and does not retain references
//!   beyond the returned token vector.

const LATIN_STOP_WORDS: &[&str] = &[
    "the", "and", "for", "that", "with", "from", "into", "this", "your", "what", "how", "why",
    "when", "where", "about", "http", "https", "www", "com", "org", "net", "html",
];

/// Tokenizes text for deterministic grouping and similarity checks.
pub fn tokenize_text(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut cjk_run = String::new();

    for ch in input.chars() {
        if is_cjk_like(ch) {
            flush_word(&mut word, &mut tokens);
            cjk_run.push(ch);
            continue;
        }
        if !cjk_run.is_empty() {
            flush_cjk_run(&mut cjk_run, &mut tokens);
        }
        if ch.is_alphanumeric() {
            word.extend(ch.to_lowercase());
        } else {
            flush_word(&mut word, &mut tokens);
        }
    }

    flush_word(&mut word, &mut tokens);
    flush_cjk_run(&mut cjk_run, &mut tokens);
    tokens
}

/// Collapses repeated whitespace without changing non-whitespace content.
pub(super) fn normalize_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut saw_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !saw_space {
                output.push(' ');
                saw_space = true;
            }
        } else {
            output.push(ch);
            saw_space = false;
        }
    }
    output.trim().to_string()
}

/// Flushes the current Latin-like word when it is meaningful for matching.
fn flush_word(word: &mut String, tokens: &mut Vec<String>) {
    if word.is_empty() {
        return;
    }
    if word.len() > 1 && !LATIN_STOP_WORDS.contains(&word.as_str()) {
        tokens.push(word.clone());
    }
    word.clear();
}

/// Emits whole-run and bigram tokens for CJK-like contiguous text.
fn flush_cjk_run(run: &mut String, tokens: &mut Vec<String>) {
    if run.is_empty() {
        return;
    }
    let chars = run.chars().collect::<Vec<_>>();
    if chars.len() == 1 {
        tokens.push(chars[0].to_string());
    } else {
        tokens.push(chars.iter().collect());
        for window in chars.windows(2) {
            tokens.push(window.iter().collect());
        }
    }
    run.clear();
}

/// Detects scripts that need run/bigram tokenization instead of Latin splitting.
fn is_cjk_like(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x30ff // Hiragana + Katakana
            | 0x3400..=0x4dbf // CJK Extension A
            | 0x4e00..=0x9fff // CJK Unified Ideographs
            | 0xac00..=0xd7af // Hangul syllables
            | 0xf900..=0xfaff // CJK compatibility ideographs
    )
}
