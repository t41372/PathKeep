//! Shared lexical normalization for archive keyword recall.
//!
//! ## Responsibilities
//! - Normalize indexed document fields and user keyword queries through the
//!   same deterministic pipeline.
//! - Own Unicode NFKC, OpenCC-compatible Traditional-to-Simplified folding,
//!   lowercase normalization, compact text, and CJK gram generation.
//! - Build FTS-safe term, CJK gram, and compact trigram query fragments.
//! - Build bounded Latin fuzzy-candidate fragments and score those candidates
//!   after SQLite has already reduced the recall set.
//!
//! ## Not responsible for
//! - Ranking or pagination policy.
//! - Regex mode, which intentionally bypasses this analyzer.
//! - Semantic, embedding, pinyin, or user-learned query expansion.
//!
//! ## Dependencies
//! - ICU4X `icu_normalizer`, already present in the workspace dependency graph,
//!   for Unicode NFKC compatibility folding.
//! - Repo-owned OpenCC-compatible converter over official OpenCC dictionary
//!   assets for Traditional/Simplified Chinese search normalization.
//! - Standard library Unicode lowercase and character classification.
//!
//! ## Performance notes
//! - ICU4X NFKC uses compiled data already linked into the binary by the
//!   existing URL/IDNA stack.
//! - OpenCC dictionary assets are parsed once per process and reused across
//!   projection rows, avoiding per-row converter initialization.

use super::search_opencc::simplified_script_variants;
use icu_normalizer::ComposingNormalizerBorrowed;
use std::collections::BTreeSet;

const FUZZY_MIN_CHARS: usize = 4;
const FUZZY_MAX_CHARS: usize = 64;
const FUZZY_COMPACT_SCAN_CHARS: usize = 512;
const TITLE_FUZZY_PENALTY: f64 = 0.0;
const URL_FUZZY_PENALTY: f64 = 0.08;
const SEARCH_TERM_FUZZY_PENALTY: f64 = 0.12;
const COMPACT_FUZZY_PENALTY: f64 = 0.2;
const QUERY_ALIASES: &[(&str, &[&str])] =
    &[("gh", &["github"]), ("yt", &["youtube"]), ("pr", &["pull request"])];

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
    pub fuzzy_query: Option<FuzzyQuery>,
}

impl LexicalQuery {
    /// Reports whether at least one indexed recall path can evaluate this query.
    pub(super) fn is_empty(&self) -> bool {
        self.terms_query.is_none() && self.trigram_query.is_none() && self.fuzzy_query.is_none()
    }
}

/// Bounded fuzzy fallback produced from Latin query variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FuzzyQuery {
    pub candidate_query: String,
    targets: Vec<String>,
}

impl FuzzyQuery {
    /// Scores one already-bounded candidate document. Lower scores are better;
    /// `None` means the trigram candidate was too far from every query target.
    pub(super) fn score_document(&self, document: &FuzzyDocument<'_>) -> Option<f64> {
        let mut best = None;
        for target in &self.targets {
            best = best_score(
                best,
                best_token_score(target, document.normalized_title, TITLE_FUZZY_PENALTY),
            );
            best = best_score(
                best,
                best_token_score(target, document.normalized_url, URL_FUZZY_PENALTY),
            );
            best = best_score(
                best,
                best_token_score(
                    target,
                    document.normalized_search_terms,
                    SEARCH_TERM_FUZZY_PENALTY,
                ),
            );
            best = best_score(
                best,
                best_compact_score(target, document.compact_text, COMPACT_FUZZY_PENALTY),
            );
        }
        best
    }
}

/// Borrowed search projection fields used by the Rust-side fuzzy scorer.
pub(super) struct FuzzyDocument<'a> {
    pub normalized_url: &'a str,
    pub normalized_title: &'a str,
    pub normalized_search_terms: &'a str,
    pub compact_text: &'a str,
}

/// Builds all normalized projection fields for one canonical URL document.
pub(super) fn analyze_document(url: &str, title: &str, search_terms: &str) -> LexicalDocument {
    let normalized_url = normalize_text(url);
    let normalized_title = normalize_text(title);
    let normalized_search_terms = normalize_text(search_terms);
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

    LexicalDocument {
        normalized_url: normalized_url.index_text(),
        normalized_title: normalized_title.index_text(),
        normalized_search_terms: normalized_search_terms.index_text(),
        compact_text,
        cjk_grams,
    }
}

/// Parses one user keyword query into FTS-safe recall fragments.
pub(super) fn analyze_query(raw: &str) -> Option<LexicalQuery> {
    let or_segments = split_query_or(raw);
    if or_segments.len() > 1 {
        return combine_lexical_or(
            or_segments
                .into_iter()
                .filter_map(|segment| analyze_query_without_or(&segment))
                .collect(),
        );
    }
    analyze_query_without_or(raw)
}

fn analyze_query_without_or(raw: &str) -> Option<LexicalQuery> {
    let normalized = normalize_text(raw);
    let variants = expand_query_aliases(normalized.variants);
    let terms_query = variants
        .iter()
        .filter_map(|variant| terms_query_for_normalized_text(variant))
        .collect::<Vec<_>>();
    let terms_query = combine_fts_or(terms_query);
    let trigram_query = variants
        .iter()
        .map(|variant| compact_text(variant))
        .filter(|compact| compact.chars().count() >= 3)
        .map(|compact| quote_fts_term(&compact))
        .collect::<Vec<_>>();
    let trigram_query = combine_fts_or(trigram_query);
    let fuzzy_query = fuzzy_query_for_variants(&variants);
    let query = LexicalQuery { terms_query, trigram_query, fuzzy_query };

    (!query.is_empty()).then_some(query)
}

fn split_query_or(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .fold(vec![String::new()], |mut segments, token| {
            if token.eq_ignore_ascii_case("or") {
                if segments.last().is_some_and(|segment| !segment.trim().is_empty()) {
                    segments.push(String::new());
                }
                return segments;
            }
            if let Some(current) = segments.last_mut() {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(token);
            }
            segments
        })
        .into_iter()
        .map(|segment| segment.trim().to_string())
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn combine_lexical_or(queries: Vec<LexicalQuery>) -> Option<LexicalQuery> {
    if queries.is_empty() {
        return None;
    }
    let terms_query =
        combine_fts_or(queries.iter().filter_map(|query| query.terms_query.clone()).collect());
    let trigram_query =
        combine_fts_or(queries.iter().filter_map(|query| query.trigram_query.clone()).collect());
    let fuzzy_query =
        combine_fuzzy_or(queries.into_iter().filter_map(|query| query.fuzzy_query).collect());
    let query = LexicalQuery { terms_query, trigram_query, fuzzy_query };
    (!query.is_empty()).then_some(query)
}

fn combine_fuzzy_or(queries: Vec<FuzzyQuery>) -> Option<FuzzyQuery> {
    let mut candidate_fragments = Vec::new();
    let mut targets = Vec::new();
    for query in queries {
        push_unique(&mut candidate_fragments, query.candidate_query);
        for target in query.targets {
            push_unique(&mut targets, target);
        }
    }
    let candidate_query = combine_fts_or(candidate_fragments)?;
    Some(FuzzyQuery { candidate_query, targets })
}

/// Produces normalized text variants for SQL-side advanced query filters.
///
/// These filters intentionally reuse the keyword analyzer's NFKC/OpenCC/lowercase
/// path so `-設定`, exact phrases, and field operators do not silently drift from
/// the indexed keyword recall semantics.
pub(super) fn normalized_filter_terms(raw: &str) -> Vec<String> {
    normalize_text(raw).variants.into_iter().fold(Vec::<String>::new(), |mut unique, variant| {
        let trimmed = variant.trim();
        if !trimmed.is_empty() {
            push_unique(&mut unique, trimmed.to_string());
        }
        unique
    })
}

/// Produces normalized and compact variants for field-agnostic advanced filters.
///
/// Exact and exclusion filters can check the compact projection, unlike
/// `intitle:` or `inurl:` where compact variants would make a single field
/// predicate too strict.
pub(super) fn normalized_compact_filter_terms(raw: &str) -> Vec<String> {
    normalize_text(raw).variants.into_iter().fold(Vec::<String>::new(), |mut unique, variant| {
        let trimmed = variant.trim();
        if !trimmed.is_empty() {
            push_unique(&mut unique, trimmed.to_string());
        }
        let compact = compact_text(trimmed);
        if !compact.is_empty() {
            push_unique(&mut unique, compact);
        }
        unique
    })
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

fn combine_fts_or(fragments: Vec<String>) -> Option<String> {
    let mut unique = Vec::new();
    for fragment in fragments {
        if !unique.iter().any(|existing| existing == &fragment) {
            unique.push(fragment);
        }
    }
    match unique.as_slice() {
        [] => None,
        [single] => Some(single.clone()),
        _ => Some(format!("({})", unique.join(" OR "))),
    }
}

struct NormalizedText {
    variants: Vec<String>,
}

impl NormalizedText {
    fn index_text(&self) -> String {
        self.variants.join(" ")
    }
}

fn normalize_text(raw: &str) -> NormalizedText {
    let without_nuls = raw.replace('\0', "");
    let nfkc = ComposingNormalizerBorrowed::new_nfkc().normalize(&without_nuls).to_string();
    let variants = simplified_script_variants(&nfkc)
        .into_iter()
        .map(|variant| variant.to_lowercase())
        .fold(Vec::<String>::new(), |mut unique, variant| {
            if !unique.iter().any(|existing| existing == &variant) {
                unique.push(variant);
            }
            unique
        });
    NormalizedText { variants }
}

fn expand_query_aliases(variants: Vec<String>) -> Vec<String> {
    variants.into_iter().fold(Vec::<String>::new(), |mut unique, variant| {
        push_unique(&mut unique, variant.clone());
        for alias in aliases_for_variant(&variant) {
            push_unique(&mut unique, alias);
        }
        unique
    })
}

fn aliases_for_variant(variant: &str) -> Vec<String> {
    let compact = compact_text(variant);
    QUERY_ALIASES
        .iter()
        .find(|(alias, _)| *alias == compact)
        .map(|(_, expansions)| {
            expansions.iter().map(|expansion| (*expansion).to_string()).collect()
        })
        .unwrap_or_default()
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

fn fuzzy_query_for_variants(variants: &[String]) -> Option<FuzzyQuery> {
    let mut targets = Vec::<String>::new();
    let mut trigrams = Vec::<String>::new();
    for variant in variants {
        let compact = compact_text(variant);
        if !is_latin_fuzzy_target(&compact) {
            continue;
        }
        push_unique(&mut targets, compact.clone());
        for trigram in latin_trigrams(&compact) {
            push_unique(&mut trigrams, quote_fts_term(&trigram));
        }
    }
    let candidate_query = combine_fts_or(trigrams)?;
    Some(FuzzyQuery { candidate_query, targets })
}

fn is_latin_fuzzy_target(compact: &str) -> bool {
    let count = compact.chars().count();
    (FUZZY_MIN_CHARS..=FUZZY_MAX_CHARS).contains(&count)
        && compact.chars().any(|character| character.is_ascii_alphabetic())
        && compact.chars().all(|character| character.is_ascii_alphanumeric())
}

fn latin_trigrams(compact: &str) -> Vec<String> {
    let chars = compact.chars().collect::<Vec<_>>();
    if chars.len() < 3 {
        return Vec::new();
    }
    chars.windows(3).map(|window| window.iter().collect::<String>()).collect()
}

fn best_token_score(target: &str, field: &str, penalty: f64) -> Option<f64> {
    field
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .filter_map(|token| fuzzy_score(target, token, penalty))
        .min_by(|left, right| left.total_cmp(right))
}

fn best_compact_score(target: &str, compact: &str, penalty: f64) -> Option<f64> {
    let target_len = target.chars().count();
    let mut best = None;
    for run in compact.split_whitespace() {
        let chars = run.chars().take(FUZZY_COMPACT_SCAN_CHARS).collect::<Vec<_>>();
        let min_window = target_len.saturating_sub(2).max(FUZZY_MIN_CHARS);
        let max_window = (target_len + 2).min(chars.len());
        for window_len in min_window..=max_window {
            for window in chars.windows(window_len) {
                let candidate = window.iter().collect::<String>();
                best = best_score(best, fuzzy_score(target, &candidate, penalty));
            }
        }
    }
    best
}

fn fuzzy_score(target: &str, candidate: &str, penalty: f64) -> Option<f64> {
    let target_chars = target.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.chars().collect::<Vec<_>>();
    let max_distance = max_allowed_distance(target_chars.len());
    let distance = bounded_edit_distance(&target_chars, &candidate_chars, max_distance)?;
    let denominator = target_chars.len().max(candidate_chars.len()).max(1) as f64;
    Some((distance as f64 / denominator) + penalty)
}

fn max_allowed_distance(target_len: usize) -> usize {
    match target_len {
        0..=3 => 0,
        4..=5 => 1,
        6..=9 => 2,
        10..=16 => 3,
        _ => 4,
    }
}

fn bounded_edit_distance(left: &[char], right: &[char], max_distance: usize) -> Option<usize> {
    if left.len().abs_diff(right.len()) > max_distance {
        return None;
    }

    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0usize; right.len() + 1];
    for (left_index, left_character) in left.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];
        for (right_index, right_character) in right.iter().enumerate() {
            let substitution_cost = usize::from(left_character != right_character);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            let substitution = previous[right_index] + substitution_cost;
            current[right_index + 1] = insertion.min(deletion).min(substitution);
            row_min = row_min.min(current[right_index + 1]);
        }
        if row_min > max_distance {
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }

    (previous[right.len()] <= max_distance).then_some(previous[right.len()])
}

fn best_score(current: Option<f64>, candidate: Option<f64>) -> Option<f64> {
    match (current, candidate) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(score), None) | (None, Some(score)) => Some(score),
        (None, None) => None,
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
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
    fn folds_traditional_and_simplified_to_shared_query() {
        let traditional = analyze_query("設定").expect("traditional query");
        let simplified = analyze_query("设定").expect("simplified query");

        assert_eq!(traditional.terms_query.as_deref(), Some("(\"设定\" OR \"设置\")"));
        assert_eq!(simplified.terms_query.as_deref(), Some("\"设定\""));
    }

    #[test]
    fn ignores_chinese_spacing_and_punctuation_for_grams() {
        let spaced = analyze_query("设 定").expect("spaced query");
        let punctuated = analyze_query("设-定").expect("punctuated query");

        assert_eq!(spaced, punctuated);
        assert_eq!(spaced.terms_query.as_deref(), Some("\"设定\""));
    }

    #[test]
    fn folds_full_width_latin_and_lowercase() {
        let query = analyze_query("GitHUb").expect("query");
        let full_width = analyze_query("ＧｉｔＨｕｂ").expect("full-width query");

        assert_eq!(query.terms_query.as_deref(), Some("\"github\"*"));
        assert_eq!(query.trigram_query.as_deref(), Some("\"github\""));
        assert_eq!(full_width, query);
    }

    #[test]
    fn compact_query_ignores_latin_spaces() {
        let query = analyze_query("git hub").expect("query");

        assert_eq!(query.terms_query.as_deref(), Some("\"git\"* AND \"hub\"*"));
        assert_eq!(query.trigram_query.as_deref(), Some("\"github\""));
    }

    #[test]
    fn supports_google_style_or_between_rankable_terms() {
        let query = analyze_query("github OR gitlab").expect("query");

        assert_eq!(query.terms_query.as_deref(), Some("(\"github\"* OR \"gitlab\"*)"));
        assert_eq!(query.trigram_query.as_deref(), Some("(\"github\" OR \"gitlab\")"));
    }

    #[test]
    fn expands_short_aliases_into_canonical_query_forms() {
        let query = analyze_query("gh").expect("query");

        assert_eq!(query.terms_query.as_deref(), Some("(\"gh\"* OR \"github\"*)"));
        assert_eq!(query.trigram_query.as_deref(), Some("\"github\""));
        assert_eq!(
            query.fuzzy_query.as_ref().map(|fuzzy| fuzzy.candidate_query.as_str()),
            Some("(\"git\" OR \"ith\" OR \"thu\" OR \"hub\")")
        );
    }

    #[test]
    fn fuzzy_query_scores_bounded_latin_typo_candidates() {
        let query = analyze_query("gihub").expect("query");
        let fuzzy = query.fuzzy_query.expect("fuzzy query");
        let candidate = FuzzyDocument {
            normalized_url: "https github com releases",
            normalized_title: "github actions manual",
            normalized_search_terms: "",
            compact_text: "httpsgithubcomreleases githubactionsmanual",
        };
        let distant = FuzzyDocument {
            normalized_url: "https calendar example",
            normalized_title: "calendar notes",
            normalized_search_terms: "planning",
            compact_text: "httpscalendarexample calendarnotes planning",
        };

        assert!(fuzzy.candidate_query.contains("\"hub\""));
        assert!(fuzzy.score_document(&candidate).is_some_and(|score| score < 0.2));
        assert!(fuzzy.score_document(&distant).is_none());
    }

    #[test]
    fn fuzzy_helpers_keep_candidate_generation_bounded() {
        assert!(latin_trigrams("ab").is_empty());
        assert!(!is_latin_fuzzy_target("abc"));
        assert!(!is_latin_fuzzy_target("設置"));
        assert!(is_latin_fuzzy_target("github"));
        assert_eq!(max_allowed_distance(3), 0);
        assert_eq!(max_allowed_distance(5), 1);
        assert_eq!(max_allowed_distance(9), 2);
        assert_eq!(max_allowed_distance(16), 3);
        assert_eq!(max_allowed_distance(17), 4);
    }

    #[test]
    fn bounded_edit_distance_rejects_unbounded_work() {
        let short = "git".chars().collect::<Vec<_>>();
        let long = "github".chars().collect::<Vec<_>>();
        let far_left = "github".chars().collect::<Vec<_>>();
        let far_right = "zzzzzz".chars().collect::<Vec<_>>();
        let near_left = "gihub".chars().collect::<Vec<_>>();
        let near_right = "github".chars().collect::<Vec<_>>();

        assert_eq!(bounded_edit_distance(&short, &long, 1), None);
        assert_eq!(bounded_edit_distance(&far_left, &far_right, 1), None);
        assert_eq!(bounded_edit_distance(&near_left, &near_right, 1), Some(1));
        assert_eq!(best_score(Some(0.4), Some(0.2)), Some(0.2));
        assert_eq!(best_score(Some(0.4), None), Some(0.4));
        assert_eq!(best_score(None, Some(0.2)), Some(0.2));
        assert_eq!(best_score(None, None), None);
    }

    #[test]
    fn document_analysis_indexes_cjk_substrings() {
        let document = analyze_document("https://example.test", "我的瀏覽器設定頁", "");

        assert!(document.cjk_grams.contains("设定"));
        assert!(document.cjk_grams.contains("设置"));
        assert!(document.compact_text.contains("我的浏览器设定页"));
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
        let query = analyze_query("!!!");

        assert!(query.is_none());
        assert!(analyze_query("!!! OR ???").is_none());
    }
}
