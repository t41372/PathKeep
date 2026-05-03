//! Shared lexical normalization for archive keyword recall.
//!
//! ## Responsibilities
//! - Normalize indexed document fields and user keyword queries through the
//!   same deterministic pipeline.
//! - Own OpenCC traditional/simplified Chinese folding for local recall.
//! - Build FTS-safe term, CJK gram, and compact trigram query fragments.
//!
//! ## Not responsible for
//! - Ranking or pagination policy.
//! - Regex mode, which intentionally bypasses this analyzer.
//! - Semantic, embedding, fuzzy, pinyin, or alias expansion.
//!
//! ## Dependencies
//! - `ferrous-opencc` for pure-Rust OpenCC conversion assets.
//! - `unicode-normalization` for NFKC folding before OpenCC conversion.
//!
//! ## Performance notes
//! - OpenCC rules are initialized once per process and reused across projection
//!   rebuild rows so imports never pay converter setup cost per URL document.

use anyhow::{Context, Result, anyhow};
use ferrous_opencc::{OpenCC, config::BuiltinConfig};
use std::{collections::BTreeSet, sync::OnceLock};
use unicode_normalization::UnicodeNormalization;

static OPENCC: OnceLock<Result<ChineseConverters, String>> = OnceLock::new();

struct ChineseConverters {
    tw2sp: OpenCC,
    t2s: OpenCC,
}

/// Derived document fields stored in the rebuildable search projection.
pub(super) struct LexicalDocument {
    pub normalized_url: String,
    pub normalized_title: String,
    pub normalized_search_terms: String,
    pub compact_text: String,
    pub cjk_grams: String,
}

/// Parsed keyword query fragments used by the FTS-backed recall path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LexicalQuery {
    pub terms_query: Option<String>,
    pub trigram_query: Option<String>,
}

impl LexicalQuery {
    /// Reports whether at least one indexed recall path can evaluate this query.
    pub(super) fn is_empty(&self) -> bool {
        self.terms_query.is_none() && self.trigram_query.is_none()
    }
}

/// Builds all normalized projection fields for one canonical URL document.
pub(super) fn analyze_document(
    url: &str,
    title: &str,
    search_terms: &str,
) -> Result<LexicalDocument> {
    let normalized_url = normalize_text(url).context("normalizing search URL")?;
    let normalized_title = normalize_text(title).context("normalizing search title")?;
    let normalized_search_terms =
        normalize_text(search_terms).context("normalizing search terms")?;
    let compact_variants = normalized_url
        .variants
        .iter()
        .chain(normalized_title.variants.iter())
        .chain(normalized_search_terms.variants.iter())
        .map(|value| compact_text(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let compact_text = compact_variants
        .iter()
        .filter(|value| !value.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let cjk_grams = cjk_grams_for_fields(compact_variants.iter().map(String::as_str));

    Ok(LexicalDocument {
        normalized_url: normalized_url.canonical,
        normalized_title: normalized_title.canonical,
        normalized_search_terms: normalized_search_terms.canonical,
        compact_text,
        cjk_grams,
    })
}

/// Parses one user keyword query into FTS-safe recall fragments.
pub(super) fn analyze_query(raw: &str) -> Result<Option<LexicalQuery>> {
    let normalized = normalize_text(raw).context("normalizing search query")?;
    let term_clauses = normalized
        .variants
        .iter()
        .filter_map(|variant| terms_query_for_normalized_text(variant))
        .collect::<Vec<_>>();
    let terms_query = join_variant_clauses(term_clauses);
    let trigram_clauses = normalized
        .variants
        .iter()
        .map(|variant| compact_text(variant))
        .filter(|compact| compact.chars().count() >= 3)
        .map(|compact| quote_fts_term(&compact))
        .collect::<Vec<_>>();
    let trigram_query = join_variant_clauses(trigram_clauses);
    let query = LexicalQuery { terms_query, trigram_query };

    Ok((!query.is_empty()).then_some(query))
}

fn terms_query_for_normalized_text(normalized: &str) -> Option<String> {
    let compact = compact_text(normalized);
    let terms = latin_prefix_terms(normalized)
        .into_iter()
        .map(|term| format!("{}*", quote_fts_term(&term)))
        .collect::<Vec<_>>();
    let cjk_grams = cjk_grams_for_fields([compact.as_str()])
        .split_whitespace()
        .map(quote_fts_term)
        .collect::<Vec<_>>();
    let terms_query = terms.into_iter().chain(cjk_grams).collect::<Vec<_>>().join(" AND ");
    (!terms_query.is_empty()).then_some(terms_query)
}

fn join_variant_clauses(clauses: Vec<String>) -> Option<String> {
    let clauses = clauses.into_iter().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    match clauses.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        _ => Some(
            clauses
                .into_iter()
                .map(|clause| format!("({clause})"))
                .collect::<Vec<_>>()
                .join(" OR "),
        ),
    }
}

struct NormalizedText {
    canonical: String,
    variants: Vec<String>,
}

fn normalize_text(raw: &str) -> Result<NormalizedText> {
    let nfkc = raw.replace('\0', "").nfkc().collect::<String>();
    let converters = opencc()?;
    let canonical = converters.convert_tw_phrase_to_simplified(&nfkc).to_lowercase();
    let standard = converters.convert_traditional_to_simplified(&nfkc).to_lowercase();
    let mut variants = vec![canonical.clone()];
    if standard != canonical {
        variants.push(standard);
    }
    Ok(NormalizedText { canonical, variants })
}

fn opencc() -> Result<&'static ChineseConverters> {
    OPENCC
        .get_or_init(|| {
            let tw2sp =
                OpenCC::from_config(BuiltinConfig::Tw2sp).map_err(|error| error.to_string())?;
            let t2s = OpenCC::from_config(BuiltinConfig::T2s).map_err(|error| error.to_string())?;
            Ok(ChineseConverters { tw2sp, t2s })
        })
        .as_ref()
        .map_err(|error| anyhow!("initializing OpenCC converter failed: {error}"))
}

impl ChineseConverters {
    fn convert_tw_phrase_to_simplified(&self, input: &str) -> String {
        self.t2s.convert(&self.tw2sp.convert(input))
    }

    fn convert_traditional_to_simplified(&self, input: &str) -> String {
        self.t2s.convert(input)
    }
}

fn latin_prefix_terms(normalized: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for token in normalized.split(|character: char| !character.is_alphanumeric()) {
        if token.is_empty() || token.chars().all(is_cjk) {
            continue;
        }
        terms.insert(token.to_string());
    }
    terms.into_iter().collect()
}

fn compact_text(normalized: &str) -> String {
    normalized.chars().filter(|character| character.is_alphanumeric()).collect()
}

fn cjk_grams_for_fields<'a>(fields: impl IntoIterator<Item = &'a str>) -> String {
    let mut grams = BTreeSet::new();
    for field in fields {
        let mut run = Vec::new();
        for character in field.chars() {
            if is_cjk(character) {
                run.push(character);
            } else {
                add_cjk_grams(&run, &mut grams);
                run.clear();
            }
        }
        add_cjk_grams(&run, &mut grams);
    }
    grams.into_iter().collect::<Vec<_>>().join(" ")
}

fn add_cjk_grams(run: &[char], grams: &mut BTreeSet<String>) {
    for size in [2usize, 3usize] {
        if run.len() < size {
            continue;
        }
        for window in run.windows(size) {
            grams.insert(window.iter().collect::<String>());
        }
    }
}

fn quote_fts_term(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2CEB0}'..='\u{2EBEF}'
            | '\u{30000}'..='\u{3134F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_traditional_queries_across_opencc_phrase_variants() {
        let traditional = analyze_query("設定").expect("traditional query").expect("query");
        let simplified = analyze_query("设定").expect("simplified query").expect("query");

        assert!(
            traditional.terms_query.as_deref().is_some_and(|query| {
                query.contains("\"设置\"") && query.contains("\"设定\"")
            })
        );
        assert_eq!(simplified.terms_query.as_deref(), Some("\"设定\""));
    }

    #[test]
    fn ignores_chinese_spacing_and_punctuation_for_grams() {
        let spaced = analyze_query("設 定").expect("spaced query").expect("query");
        let punctuated = analyze_query("设-定").expect("punctuated query").expect("query");

        assert_eq!(spaced, punctuated);
        assert_eq!(spaced.terms_query.as_deref(), Some("\"设定\""));
    }

    #[test]
    fn folds_full_width_latin_and_lowercase() {
        let query = analyze_query("ＧｉｔＨｕｂ").expect("query").expect("query");

        assert_eq!(query.terms_query.as_deref(), Some("\"github\"*"));
        assert_eq!(query.trigram_query.as_deref(), Some("\"github\""));
    }

    #[test]
    fn compact_query_ignores_latin_spaces() {
        let query = analyze_query("git hub").expect("query").expect("query");

        assert_eq!(query.terms_query.as_deref(), Some("\"git\"* AND \"hub\"*"));
        assert_eq!(query.trigram_query.as_deref(), Some("\"github\""));
    }

    #[test]
    fn document_analysis_indexes_cjk_substrings() {
        let document =
            analyze_document("https://example.test", "我的瀏覽器設定頁", "").expect("document");

        assert!(document.cjk_grams.contains("设置"));
        assert!(document.compact_text.contains("我的浏览器设置页"));
    }

    #[test]
    fn cjk_detection_covers_extension_blocks() {
        assert!(is_cjk('\u{F900}'));
        assert!(is_cjk('\u{20000}'));
        assert!(is_cjk('\u{2A700}'));
        assert!(is_cjk('\u{2B740}'));
        assert!(is_cjk('\u{2B820}'));
        assert!(is_cjk('\u{2CEB0}'));
        assert!(is_cjk('\u{30000}'));
    }

    #[test]
    fn punctuation_only_query_has_no_recall_path() {
        let query = analyze_query("!!!").expect("query");

        assert!(query.is_none());
    }
}
