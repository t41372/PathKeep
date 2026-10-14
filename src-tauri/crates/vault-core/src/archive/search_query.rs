//! Advanced query parsing for Explorer keyword recall.
//!
//! ## Responsibilities
//! - Parse Google-like keyword operators that can be evaluated locally against
//!   archived URL/title/search-term evidence.
//! - Keep the fast positive keyword text separate from SQL-side field, date,
//!   site, exact-phrase, filetype, and exclusion constraints.
//! - Normalize filter values through the same analyzer used by lexical recall
//!   so advanced filters do not drift from normal keyword behavior.
//!
//! ## Not responsible for
//! - Regex mode; regex remains an explicit Rust `regex` post-filter path.
//! - Semantic ranking, embedding, language/region/licence filters, or web
//!   content that PathKeep does not archive in v0.1.
//! - Building SQL statements; this module only returns deterministic parse
//!   output for the read owner to apply.
//!
//! ## Dependencies
//! - `archive::search_lexical` for repo-owned Unicode/OpenCC normalization.
//! - `chrono` for visit-date operator normalization; dates are days in the computer's time zone.
//!
//! ## Performance notes
//! - Parsing is linear in query length and all SQL filter vectors are capped so
//!   a pasted query cannot create unbounded temp-table work.

use super::search_lexical::{normalized_compact_filter_terms, normalized_filter_terms};
use crate::utils::local_day_range_ms;
use chrono::NaiveDate;

/// Parsed local-history equivalent of Google-style advanced search operators.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ParsedHistorySearchQuery {
    pub keyword_text: Option<String>,
    pub exact_terms: Vec<String>,
    pub excluded_terms: Vec<String>,
    pub required_title_terms: Vec<String>,
    pub excluded_title_terms: Vec<String>,
    pub required_url_terms: Vec<String>,
    pub excluded_url_terms: Vec<String>,
    pub required_sites: Vec<String>,
    pub excluded_sites: Vec<String>,
    pub required_filetypes: Vec<String>,
    pub excluded_filetypes: Vec<String>,
    /// User-authored tags (`tag:foo`, `-tag:foo`) — matched exactly
    /// against `url_tags.tag` after lowercase + trim normalisation.
    /// Multiple `tag:` operators require ALL listed tags (AND join);
    /// `-tag:` excludes any URL that carries that tag.
    pub required_tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    /// User-authored notes (`note:bar`, `-note:bar`) — matched as a
    /// case-insensitive substring against `url_annotations.notes`.
    /// Multiple `note:` operators require ALL listed substrings to
    /// appear in the same URL's notes; `-note:` excludes any URL
    /// whose notes contain the substring.
    pub required_notes: Vec<String>,
    pub excluded_notes: Vec<String>,
    pub after_ms: Option<i64>,
    pub before_ms: Option<i64>,
}

/// Parses the Explorer keyword box into local advanced-search constraints.
pub(crate) fn parse_history_search_query(raw: &str) -> ParsedHistorySearchQuery {
    let tokens = tokenize_query(raw);
    let mut parsed = ParsedHistorySearchQuery::default();
    let mut keyword_terms = Vec::<String>::new();

    for token in tokens {
        let raw_value = token.value.trim();
        let (negated, value) = match raw_value.strip_prefix('-') {
            Some(stripped) if !stripped.is_empty() => (true, stripped.trim()),
            _ => (false, raw_value),
        };

        let (operator, operand) = split_operator(value);
        match operator.as_deref() {
            // Page search owns the star predicate; it is not part of a typo target.
            Some("is") if !negated && operand.eq_ignore_ascii_case("starred") => {}
            Some("site") => {
                push_filter_values(
                    if negated { &mut parsed.excluded_sites } else { &mut parsed.required_sites },
                    normalized_site_filter(operand),
                );
            }
            Some("intitle") | Some("title") => {
                push_normalized_filter_values(
                    if negated {
                        &mut parsed.excluded_title_terms
                    } else {
                        &mut parsed.required_title_terms
                    },
                    operand,
                );
            }
            Some("inurl") | Some("url") => {
                push_normalized_filter_values(
                    if negated {
                        &mut parsed.excluded_url_terms
                    } else {
                        &mut parsed.required_url_terms
                    },
                    operand,
                );
            }
            Some("filetype") | Some("ext") => {
                push_filter_values(
                    if negated {
                        &mut parsed.excluded_filetypes
                    } else {
                        &mut parsed.required_filetypes
                    },
                    normalized_filetype_filter(operand),
                );
            }
            Some("tag") => {
                push_filter_values(
                    if negated { &mut parsed.excluded_tags } else { &mut parsed.required_tags },
                    normalized_tag_filter(operand),
                );
            }
            Some("note") => {
                push_filter_values(
                    if negated { &mut parsed.excluded_notes } else { &mut parsed.required_notes },
                    normalized_note_filter(operand),
                );
            }
            Some("after") if !negated => {
                parsed.after_ms = parse_after_date(operand).or(parsed.after_ms);
            }
            Some("before") if !negated => {
                parsed.before_ms = parse_before_date(operand).or(parsed.before_ms);
            }
            _ if negated => {
                push_normalized_compact_filter_values(&mut parsed.excluded_terms, value);
            }
            _ if token.quoted => {
                keyword_terms.push(value.to_string());
                push_normalized_compact_filter_values(&mut parsed.exact_terms, value);
            }
            _ => keyword_terms.push(value.to_string()),
        }
    }

    parsed.keyword_text = normalized_keyword_text(keyword_terms);
    parsed
}

pub(crate) fn tokenize_query(raw: &str) -> Vec<QueryToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut current_was_quoted = false;
    let mut characters = raw.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' | '“' | '”' => {
                if quoted {
                    quoted = false;
                    current_was_quoted = true;
                    continue;
                }
                if current.is_empty() || current == "-" || current.ends_with(':') {
                    quoted = true;
                    continue;
                }
                current.push(character);
            }
            '\\' if quoted => {
                if let Some(next) = characters.next() {
                    current.push(next);
                }
            }
            character if character.is_whitespace() && !quoted => {
                push_token(&mut tokens, &mut current, current_was_quoted);
                current_was_quoted = false;
            }
            _ => current.push(character),
        }
    }
    push_token(&mut tokens, &mut current, current_was_quoted || quoted);
    tokens
}

fn push_token(tokens: &mut Vec<QueryToken>, current: &mut String, quoted: bool) {
    let value = current.trim();
    if !value.is_empty() {
        tokens.push(QueryToken { value: value.to_string(), quoted });
    }
    current.clear();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryToken {
    pub value: String,
    pub quoted: bool,
}

pub(crate) fn split_operator(value: &str) -> (Option<String>, &str) {
    let Some(separator) = value.find(':') else {
        return (None, value);
    };
    let operator = value[..separator].trim().to_ascii_lowercase();
    if operator.is_empty() || !operator.chars().all(|character| character.is_ascii_alphabetic()) {
        return (None, value);
    }
    (Some(operator), value[separator + 1..].trim())
}

fn normalized_keyword_text(terms: Vec<String>) -> Option<String> {
    let text = terms
        .into_iter()
        .map(|term| term.trim().to_string())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

fn normalized_site_filter(raw: &str) -> Vec<String> {
    let trimmed = raw
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_lowercase();
    (!trimmed.is_empty()).then_some(vec![trimmed]).unwrap_or_default()
}

fn normalized_filetype_filter(raw: &str) -> Vec<String> {
    let trimmed = raw.trim().trim_start_matches('.').to_lowercase();
    let normalized =
        trimmed.chars().filter(|character| character.is_ascii_alphanumeric()).collect::<String>();
    (!normalized.is_empty()).then_some(vec![normalized]).unwrap_or_default()
}

/// Normalises a `tag:` operand for exact-match against `url_tags.tag`.
/// Tags are stored case-sensitively in the schema, but the Browse
/// detail panel persists them as the user typed them — so a search
/// for `tag:rust` should still match a stored `rust` or `Rust`. We
/// lowercase + trim and let the SQL clause apply `LOWER(url_tags.tag)`
/// on the join side.
fn normalized_tag_filter(raw: &str) -> Vec<String> {
    let trimmed = raw.trim().to_lowercase();
    (!trimmed.is_empty()).then_some(vec![trimmed]).unwrap_or_default()
}

/// Normalises a `note:` operand for case-insensitive substring match
/// against `url_annotations.notes`. The substring path matches the
/// existing `searchAnnotations` backend behaviour the detail panel
/// already exposes, so users get consistent results whether they reach
/// the note via Browse search or via the dedicated annotations API.
fn normalized_note_filter(raw: &str) -> Vec<String> {
    let trimmed = raw.trim().to_lowercase();
    (!trimmed.is_empty()).then_some(vec![trimmed]).unwrap_or_default()
}

fn push_normalized_filter_values(target: &mut Vec<String>, raw: &str) {
    push_filter_values(target, normalized_filter_terms(raw));
}

fn push_normalized_compact_filter_values(target: &mut Vec<String>, raw: &str) {
    push_filter_values(target, normalized_compact_filter_terms(raw));
}

fn push_filter_values(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() && !target.iter().any(|existing| existing == trimmed) {
            target.push(trimmed.to_string());
        }
    }
}

/// From the start of the day (or year) in the computer's time zone, the zone History shows days in.
fn parse_after_date(raw: &str) -> Option<i64> {
    let (first, _) = parse_query_days(raw)?;
    Some(local_day_range_ms(first)?.0)
}

/// To the end of the day (or year) in the computer's time zone.
fn parse_before_date(raw: &str) -> Option<i64> {
    let (_, last) = parse_query_days(raw)?;
    Some(local_day_range_ms(last)?.1)
}

/// The first and last day a date operand names: one day, or a whole year.
fn parse_query_days(raw: &str) -> Option<(NaiveDate, NaiveDate)> {
    let trimmed = raw.trim().replace('/', "-");
    if trimmed.len() == 4 && trimmed.chars().all(|character| character.is_ascii_digit()) {
        let year = trimmed.parse::<i32>().ok()?;
        return Some((
            NaiveDate::from_ymd_opt(year, 1, 1)?,
            NaiveDate::from_ymd_opt(year, 12, 31)?,
        ));
    }
    let day = NaiveDate::parse_from_str(&trimmed, "%Y-%m-%d").ok()?;
    Some((day, day))
}
