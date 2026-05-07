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
//! - `chrono` for visit-date operator normalization.
//!
//! ## Performance notes
//! - Parsing is linear in query length and all SQL filter vectors are capped so
//!   a pasted query cannot create unbounded temp-table work.

use super::search_lexical::{normalized_compact_filter_terms, normalized_filter_terms};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};

const MAX_FILTER_VALUES: usize = 16;

/// Parsed local-history equivalent of Google-style advanced search operators.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ParsedHistorySearchQuery {
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
    pub after_ms: Option<i64>,
    pub before_ms: Option<i64>,
}

/// Parses the Explorer keyword box into local advanced-search constraints.
pub(super) fn parse_history_search_query(raw: &str) -> ParsedHistorySearchQuery {
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

fn tokenize_query(raw: &str) -> Vec<QueryToken> {
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
struct QueryToken {
    value: String,
    quoted: bool,
}

fn split_operator(value: &str) -> (Option<String>, &str) {
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

fn push_normalized_filter_values(target: &mut Vec<String>, raw: &str) {
    push_filter_values(target, normalized_filter_terms(raw));
}

fn push_normalized_compact_filter_values(target: &mut Vec<String>, raw: &str) {
    push_filter_values(target, normalized_compact_filter_terms(raw));
}

fn push_filter_values(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if target.len() >= MAX_FILTER_VALUES {
            return;
        }
        let trimmed = value.trim();
        if !trimmed.is_empty() && !target.iter().any(|existing| existing == trimmed) {
            target.push(trimmed.to_string());
        }
    }
}

fn parse_after_date(raw: &str) -> Option<i64> {
    let date = parse_query_date(raw)?;
    Utc.with_ymd_and_hms(date.year, date.month, date.day, 0, 0, 0)
        .single()
        .map(|value| value.timestamp_millis())
}

fn parse_before_date(raw: &str) -> Option<i64> {
    let date = parse_query_date(raw)?;
    let (month, day) = if date.year_only { (12, 31) } else { (date.month, date.day) };
    Utc.with_ymd_and_hms(date.year, month, day, 23, 59, 59)
        .single()
        .map(|value| value.timestamp_millis() + 999)
}

fn parse_query_date(raw: &str) -> Option<QueryDate> {
    let trimmed = raw.trim().replace('/', "-");
    if trimmed.len() == 4 && trimmed.chars().all(|character| character.is_ascii_digit()) {
        return trimmed.parse::<i32>().ok().map(|year| QueryDate {
            year,
            month: 1,
            day: 1,
            year_only: true,
        });
    }
    NaiveDate::parse_from_str(&trimmed, "%Y-%m-%d").ok().map(|date| QueryDate {
        year: date.year(),
        month: date.month(),
        day: date.day(),
        year_only: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueryDate {
    year: i32,
    month: u32,
    day: u32,
    year_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_site_exclusion_and_exact_phrase_without_regex() {
        let parsed = parse_history_search_query(r#"site:github.com "release notes" -pathkeep"#);

        assert_eq!(parsed.keyword_text.as_deref(), Some("release notes"));
        assert_eq!(parsed.required_sites, vec!["github.com"]);
        assert!(parsed.exact_terms.iter().any(|term| term == "release notes"));
        assert!(parsed.excluded_terms.iter().any(|term| term == "pathkeep"));
    }

    #[test]
    fn parses_field_filetype_and_date_operators() {
        let parsed = parse_history_search_query(
            r#"intitle:"release notes" inurl:github filetype:pdf after:2026-05-01 before:2026/05/07 -site:docs.github.com -ext:doc"#,
        );

        assert!(parsed.required_title_terms.iter().any(|term| term == "release notes"));
        assert!(parsed.required_url_terms.iter().any(|term| term == "github"));
        assert_eq!(parsed.required_filetypes, vec!["pdf"]);
        assert_eq!(parsed.excluded_sites, vec!["docs.github.com"]);
        assert_eq!(parsed.excluded_filetypes, vec!["doc"]);
        assert!(parsed.after_ms.is_some());
        assert!(parsed.before_ms.is_some());
    }

    #[test]
    fn parses_negated_fields_year_dates_and_tokenizer_edges() {
        let parsed = parse_history_search_query(
            r#"-intitle:pathkeep -inurl:issues foo"bar "release \"notes\"" 123:abc after:2026 before:2026"#,
        );

        assert!(parsed.excluded_title_terms.iter().any(|term| term == "pathkeep"));
        assert!(parsed.excluded_url_terms.iter().any(|term| term == "issues"));
        assert_eq!(parsed.after_ms, parse_after_date("2026"));
        assert_eq!(parsed.before_ms, parse_before_date("2026"));
        assert_eq!(parsed.keyword_text.as_deref(), Some(r#"foo"bar release "notes" 123:abc"#));
    }

    #[test]
    fn caps_filter_values_and_ignores_or_as_keyword_noise() {
        let parsed = parse_history_search_query(
            "-a -b -c -d -e -f -g -h -i -j -k -l -m -n -o -p -q github OR gitlab",
        );

        assert_eq!(parsed.excluded_terms.len(), MAX_FILTER_VALUES);
        assert_eq!(parsed.keyword_text.as_deref(), Some("github OR gitlab"));
    }
}
