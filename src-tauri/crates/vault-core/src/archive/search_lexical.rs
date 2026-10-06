//! Shared lexical normalization for archive keyword recall.
//!
//! ## Responsibilities
//! - Normalize indexed document fields and user keyword queries through the
//!   same deterministic pipeline.
//! - Own Unicode NFKC, OpenCC-compatible Traditional-to-Simplified folding,
//!   lowercase normalization, compact text, and CJK gram generation.
//! - Validate Latin typo distance over every indexed candidate, without input windows.
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
const TITLE_FUZZY_PENALTY: f64 = 0.0;
const URL_FUZZY_PENALTY: f64 = 0.08;
const SEARCH_TERM_FUZZY_PENALTY: f64 = 0.12;
const COMPACT_FUZZY_PENALTY: f64 = 0.2;
const QUERY_ALIASES: &[(&str, &[&str])] =
    &[("gh", &["github"]), ("yt", &["youtube"]), ("pr", &["pull request"])];

/// Derived document fields stored in the rebuildable search projection.
pub(crate) struct LexicalDocument {
    pub normalized_url: String,
    pub normalized_title: String,
    pub normalized_search_terms: String,
    pub compact_text: String,
    pub cjk_grams: String,
}

/// Latin typo targets evaluated without a candidate or text window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FuzzyQuery {
    targets: Vec<String>,
}

impl FuzzyQuery {
    /// Scores one candidate document. Lower scores are better;
    /// `None` means the document was too far from every query target.
    pub(crate) fn score_document(&self, document: &FuzzyDocument<'_>) -> Option<f64> {
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
pub(crate) struct FuzzyDocument<'a> {
    pub normalized_url: &'a str,
    pub normalized_title: &'a str,
    pub normalized_search_terms: &'a str,
    pub compact_text: &'a str,
}

/// Builds all normalized projection fields for one canonical URL document.
///
/// `notes` + `tags` are the user-authored annotation text. They do NOT feed the field-specific
/// `normalized_url`/`normalized_title`/`normalized_search_terms` columns (those stay url/title/search-term
/// only, so `intitle:`/`inurl:` filters never leak into notes), but they DO fold into the field-agnostic
/// `compact_text` (trigram plane) and `cjk_grams` (term-FTS gram plane). That is what makes a
/// contiguous-CJK note like "meta 的设计系统" recallable by a plain "设计系统" search — PathKeep's CJK
/// recall is entirely gram/trigram-based, so the unicode61 `notes_text`/`tags_text` columns alone could
/// only ever serve Latin. Empty notes/tags contribute nothing.
pub(crate) fn analyze_document(
    url: &str,
    title: &str,
    search_terms: &str,
    notes: &str,
    tags: &str,
) -> LexicalDocument {
    let normalized_url = normalize_text(url);
    let normalized_title = normalize_text(title);
    let normalized_search_terms = normalize_text(search_terms);
    let normalized_notes = normalize_text(notes);
    let normalized_tags = normalize_text(tags);
    let compact_variants = normalized_url
        .variants
        .iter()
        .chain(normalized_title.variants.iter())
        .chain(normalized_search_terms.variants.iter())
        .chain(normalized_notes.variants.iter())
        .chain(normalized_tags.variants.iter())
        .flat_map(|value| value.split('|').map(compact_text))
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

/// Enrichment joins the same compact/gram projection without changing field-specific text.
pub(crate) fn append_enrichment(document: &mut LexicalDocument, text: &str) {
    for variant in normalize_text(text).variants {
        for part in variant.split('|') {
            let compact = compact_text(part);
            if !compact.is_empty() {
                document.compact_text.push(' ');
                document.compact_text.push_str(&compact);
            }
        }
    }
    document.cjk_grams = cjk_grams_for_fields(document.compact_text.split_whitespace());
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

/// Produces normalized text variants for SQL-side advanced query filters.
///
/// These filters intentionally reuse the keyword analyzer's NFKC/OpenCC/lowercase
/// path so `-設定`, exact phrases, and field operators do not silently drift from
/// the indexed keyword recall semantics.
pub(crate) fn normalized_filter_terms(raw: &str) -> Vec<String> {
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
pub(crate) fn normalized_compact_filter_terms(raw: &str) -> Vec<String> {
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

fn compact_text(normalized: &str) -> String {
    normalized.chars().filter(|character| character.is_alphanumeric()).collect()
}

/// Keeps every eligible query target; candidate retrieval belongs to vault-search.
pub(crate) fn fuzzy_query(raw: &str) -> Option<FuzzyQuery> {
    let mut targets = Vec::new();
    for segment in split_query_or(raw) {
        for variant in query_variants(&segment) {
            let compact = compact_text(&normalized_tokens(&variant));
            if is_latin_fuzzy_target(&compact) {
                push_unique(&mut targets, compact);
            }
        }
    }
    (!targets.is_empty()).then_some(FuzzyQuery { targets })
}

fn is_latin_fuzzy_target(compact: &str) -> bool {
    let count = compact.chars().count();
    count >= FUZZY_MIN_CHARS
        && compact.chars().any(|character| character.is_ascii_alphabetic())
        && compact.chars().all(|character| character.is_ascii_alphanumeric())
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
        let chars = run.chars().collect::<Vec<_>>();
        if run.contains(target) {
            best = best_score(best, Some(penalty));
            continue;
        }
        let distance = max_allowed_distance(target_len);
        let min_window = target_len.saturating_sub(distance).max(FUZZY_MIN_CHARS);
        let max_window = target_len.saturating_add(distance).min(chars.len());
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

/// Shares the edit bound so the index adapter cannot choose a lossy gram filter.
pub(crate) fn max_allowed_distance(target_len: usize) -> usize {
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
    if left == right {
        return Some(0);
    }
    let outside = max_distance + 1;
    let mut previous = vec![outside; right.len() + 1];
    let mut current = vec![outside; right.len() + 1];
    for (column, value) in previous.iter_mut().enumerate().take(max_distance + 1) {
        *value = column;
    }
    for (index, ch) in left.iter().enumerate() {
        let row = index + 1;
        let first = row.saturating_sub(max_distance).max(1);
        let last = (row + max_distance).min(right.len());
        current[0] = row.min(outside);
        if first > 1 {
            current[first - 1] = outside;
        }
        if last < right.len() {
            current[last + 1] = outside;
        }
        let mut row_min = current[0];
        for column in first..=last {
            current[column] = (current[column - 1] + 1)
                .min(previous[column] + 1)
                .min(previous[column - 1] + usize::from(*ch != right[column - 1]));
            row_min = row_min.min(current[column]);
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
        for run in cjk_runs(field) {
            add_cjk_grams(&run, &mut grams);
        }
    }
    grams.into_iter().collect::<Vec<_>>().join(" ")
}
/// Shares the projection's script boundaries with the typed query adapter.
pub(crate) fn cjk_runs(text: &str) -> Vec<Vec<char>> {
    let mut runs = Vec::new();
    let mut current = Vec::new();
    for ch in text.chars() {
        if is_cjk(ch) {
            current.push(ch);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
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

/// Shared tokenization for Tantivy's whitespace-only analyzer. ICU owns accent
/// decomposition; the existing NFKC/OpenCC pipeline still owns script folding.
pub(crate) fn index_tokens(raw: &str) -> String {
    normalize_text(raw)
        .variants
        .iter()
        .map(|variant| normalized_tokens(variant))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Tokenizes an already folded variant without making phrase positions cross variants.
pub(crate) fn normalized_tokens(raw: &str) -> String {
    let ccc = icu_normalizer::properties::CanonicalCombiningClassMapBorrowed::new();
    let decomposer = icu_normalizer::DecomposingNormalizerBorrowed::new_nfd();
    let mut result = String::new();
    for ch in decomposer.normalize(raw).chars().filter(|ch| ccc.get_u8(*ch) == 0) {
        if is_cjk(ch) {
            if !result.ends_with(' ') && !result.is_empty() {
                result.push(' ');
            }
            result.push(ch);
            result.push(' ');
        } else if ch == '|' {
            if !result.ends_with(' ') && !result.is_empty() {
                result.push(' ');
            }
            result.push_str("| ");
        } else if ch.is_alphanumeric() {
            result.push(ch);
        } else if !result.ends_with(' ') && !result.is_empty() {
            result.push(' ');
        }
    }
    result.trim().to_owned()
}

pub(crate) fn query_variants(raw: &str) -> Vec<String> {
    expand_query_aliases(normalize_text(raw).variants)
}
pub(crate) fn contains_cjk(raw: &str) -> bool {
    raw.chars().any(is_cjk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_traditional_and_simplified_to_shared_query() {
        assert_eq!(query_variants("設定"), vec!["设定", "设置"]);
        assert_eq!(query_variants("设定"), vec!["设定"]);
    }

    #[test]
    fn ignores_chinese_spacing_and_punctuation_for_grams() {
        let spaced = analyze_document("", "设 定", "", "", "");
        let punctuated = analyze_document("", "设-定", "", "", "");
        assert_eq!(spaced.cjk_grams, punctuated.cjk_grams);
        assert_eq!(spaced.cjk_grams, "设定");
    }

    #[test]
    fn folds_full_width_latin_and_lowercase() {
        assert_eq!(index_tokens("GitHUb"), "github");
        assert_eq!(index_tokens("ＧｉｔＨｕｂ"), "github");
        assert_eq!(index_tokens("CAFÉ"), "cafe");
    }

    #[test]
    fn compact_query_ignores_latin_spaces() {
        assert_eq!(
            analyze_document("", "git hub", "", "", "").compact_text,
            analyze_document("", "github", "", "", "").compact_text
        );
    }

    #[test]
    fn supports_google_style_or_between_rankable_terms() {
        assert_eq!(split_query_or("github OR gitlab"), vec!["github", "gitlab"]);
    }

    #[test]
    fn expands_short_aliases_into_canonical_query_forms() {
        assert_eq!(query_variants("gh"), vec!["gh", "github"]);
        assert_eq!(fuzzy_query("gh").unwrap().targets, vec!["github"]);
    }

    #[test]
    fn fuzzy_query_scores_bounded_latin_typo_candidates() {
        let fuzzy = fuzzy_query("gihub").expect("fuzzy query");
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

        assert!(fuzzy.score_document(&candidate).is_some_and(|score| score < 0.2));
        assert!(fuzzy.score_document(&distant).is_none());
    }

    #[test]
    fn fuzzy_policy_keeps_the_edit_distance_bound_without_candidate_limits() {
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
        let document = analyze_document("https://example.test", "我的瀏覽器設定頁", "", "", "");

        assert!(document.cjk_grams.contains("设定"));
        assert!(document.cjk_grams.contains("设置"));
        assert!(document.compact_text.contains("我的浏览器设定页"));
        assert!(document.compact_text.contains("我的浏览器设置页"));
    }

    #[test]
    fn document_analysis_folds_cjk_notes_and_tags_into_grams_and_compact() {
        // The user's failing case: a contiguous-CJK note/tag must produce the same grams a CJK title
        // would, so it is recallable through the gram/trigram plane (not just the unicode61 columns).
        let document =
            analyze_document("https://example.test", "o/r", "", "meta 的设计系统", "参考资料");

        // Notes fold into BOTH the term-FTS gram plane and the trigram compact plane.
        assert!(document.cjk_grams.contains("设计"));
        assert!(document.cjk_grams.contains("计系"));
        assert!(document.cjk_grams.contains("系统"));
        assert!(document.compact_text.contains("的设计系统"));
        // Tags fold in too.
        assert!(document.cjk_grams.contains("参考"));
        assert!(document.compact_text.contains("参考资料"));
        // Notes/tags do NOT leak into the field-specific title index (intitle:/inurl: stay clean).
        assert!(!document.normalized_title.contains("设计"));
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
        assert!(index_tokens("!!!").is_empty());
        assert!(fuzzy_query("!!! OR ???").is_none());
    }
    #[test]
    fn fuzzy_validation_has_no_query_length_or_document_prefix_window() {
        let target = "a".repeat(80) + "bcdef";
        let candidate = "a".repeat(80) + "bcxef";
        let fuzzy = fuzzy_query(&target).unwrap();
        assert!(
            fuzzy
                .score_document(&FuzzyDocument {
                    normalized_url: "",
                    normalized_title: &candidate,
                    normalized_search_terms: "",
                    compact_text: ""
                })
                .is_some()
        );
        let text = "z".repeat(600) + " gihub";
        assert!(
            fuzzy_query("github")
                .unwrap()
                .score_document(&FuzzyDocument {
                    normalized_url: "",
                    normalized_title: "",
                    normalized_search_terms: "",
                    compact_text: &text
                })
                .is_some()
        );
    }
    #[test]
    fn distance_bands_equal_an_exhaustive_reference() {
        let words = (0..81usize)
            .map(|mut value| {
                let mut word = Vec::new();
                for _ in 0..4 {
                    word.push(['a', 'b', 'c'][value % 3]);
                    value /= 3;
                }
                word
            })
            .collect::<Vec<_>>();
        for left in &words {
            for right in &words {
                let mut table = vec![vec![0; right.len() + 1]; left.len() + 1];
                for (i, row) in table.iter_mut().enumerate() {
                    row[0] = i;
                }
                for (j, value) in table[0].iter_mut().enumerate() {
                    *value = j;
                }
                for i in 1..=left.len() {
                    for j in 1..=right.len() {
                        table[i][j] = (table[i - 1][j] + 1)
                            .min(table[i][j - 1] + 1)
                            .min(table[i - 1][j - 1] + usize::from(left[i - 1] != right[j - 1]));
                    }
                }
                let expected = table[left.len()][right.len()];
                for bound in 0..=4 {
                    assert_eq!(
                        bounded_edit_distance(left, right, bound),
                        (expected <= bound).then_some(expected)
                    );
                }
            }
        }
    }
}
