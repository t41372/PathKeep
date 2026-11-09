//! Responsibilities: keep SQL filters and index queries faithful to the same quoted syntax.
//! Not responsible for: ranking, storage or changing normalization.
//!
//! Ways this can fail:
//! - Any operator or alias inside whole-token quotes becomes a filter or a field query.
//! - Quoting only an operand disables a filter, including a tag with spaces or a title phrase.
//! - A minus inside whole-token quotes becomes an exclusion; one before quotes is lost.
//! - `-"site:x"` excludes a site instead of the text, or `-site:"x"` excludes text instead of a site.
//! - An unclosed quote loses its literal or operand meaning; an empty quoted token leaks state.
//! - OR treats a quoted operator-looking word as a filter and breaks its neighbouring alternatives.
//! - Curly quotes, escaped quotes or backslashes lose where the quoted run began.
//! - A phrase the Advanced search form writes with escaped quotes (`words-query.ts`) reads as
//!   several words, so "any" matches one of them and "none" excludes each.
//! - CJK after `Note:` becomes a note filter even when the whole token is quoted.
//! - Unknown operators, unquoted operators or ordinary phrases change meaning.

use super::{build, matches_document};
use crate::archive::{
    search_lexical::{analyze_document, normalized_compact_filter_terms, normalized_tokens},
    search_query::{ParsedHistorySearchQuery, parse_history_search_query, tokenize_query},
};
use crate::utils::local_day_range_ms;
use chrono::NaiveDate;
use vault_search::{PageDocument, TextField};

fn document(text: &str, field: TextField) -> PageDocument {
    let mut document = PageDocument::new(1);
    let analyzed = analyze_document("", text, "", "", "");
    document.text[field as usize] = normalized_tokens(&analyzed.normalized_title);
    document.compact_text = analyzed.compact_text;
    document
}

fn assert_literal(raw: &str, value: &str) {
    let filters = ParsedHistorySearchQuery {
        keyword_text: Some(value.into()),
        exact_terms: normalized_compact_filter_terms(value),
        ..Default::default()
    };
    assert_eq!(parse_history_search_query(raw), filters, "SQL: {raw}");
    let query = build(raw, false);
    assert_eq!(query.filters, filters, "index filters: {raw}");
    assert!(!query.starred, "{raw}");
    assert!(matches_document(&query.ast, &document(value, TextField::Title)), "{raw}");
    assert!(!matches_document(&query.ast, &document("unrelated", TextField::Title)), "{raw}");
    let tokens = normalized_tokens(value);
    if tokens.split_whitespace().count() > 1 {
        let separated = tokens.split_whitespace().collect::<Vec<_>>().join(" unrelated ");
        assert!(!matches_document(&query.ast, &document(&separated, TextField::Title)), "{raw}");
    }
}

#[test]
fn every_operator_distinguishes_whole_quotes_from_operand_quotes_in_both_parsers() {
    let day = local_day_range_ms(NaiveDate::from_ymd_opt(2026, 5, 7).unwrap()).unwrap();
    // All names handled by either parser, including aliases and the star predicate.
    let cases = [
        (
            "site",
            "github.com",
            ParsedHistorySearchQuery {
                required_sites: vec!["github.com".into()],
                ..Default::default()
            },
            None,
        ),
        (
            "intitle",
            "manual",
            ParsedHistorySearchQuery {
                required_title_terms: vec!["manual".into()],
                ..Default::default()
            },
            Some(TextField::Title),
        ),
        (
            "title",
            "manual",
            ParsedHistorySearchQuery {
                required_title_terms: vec!["manual".into()],
                ..Default::default()
            },
            Some(TextField::Title),
        ),
        (
            "inurl",
            "manual",
            ParsedHistorySearchQuery {
                required_url_terms: vec!["manual".into()],
                ..Default::default()
            },
            Some(TextField::Url),
        ),
        (
            "url",
            "manual",
            ParsedHistorySearchQuery {
                required_url_terms: vec!["manual".into()],
                ..Default::default()
            },
            Some(TextField::Url),
        ),
        (
            "filetype",
            "pdf",
            ParsedHistorySearchQuery {
                required_filetypes: vec!["pdf".into()],
                ..Default::default()
            },
            None,
        ),
        (
            "ext",
            "pdf",
            ParsedHistorySearchQuery {
                required_filetypes: vec!["pdf".into()],
                ..Default::default()
            },
            None,
        ),
        (
            "tag",
            "reading",
            ParsedHistorySearchQuery {
                required_tags: vec!["reading".into()],
                ..Default::default()
            },
            Some(TextField::Tags),
        ),
        (
            "note",
            "rust",
            ParsedHistorySearchQuery { required_notes: vec!["rust".into()], ..Default::default() },
            None,
        ),
        (
            "after",
            "2026-05-07",
            ParsedHistorySearchQuery { after_ms: Some(day.0), ..Default::default() },
            None,
        ),
        (
            "before",
            "2026-05-07",
            ParsedHistorySearchQuery { before_ms: Some(day.1), ..Default::default() },
            None,
        ),
        ("is", "starred", ParsedHistorySearchQuery::default(), None),
    ];
    for (operator, operand, filters, field) in cases {
        let value = format!("{operator}:{operand}");
        assert_literal(&format!("\"{value}\""), &value);
        assert_literal(&format!("\"-{value}\""), &format!("-{value}"));
        let excluded = format!("-\"{value}\"");
        let excluded_filters = ParsedHistorySearchQuery {
            excluded_terms: normalized_compact_filter_terms(&value),
            ..Default::default()
        };
        assert_eq!(parse_history_search_query(&excluded), excluded_filters, "SQL: {excluded}");
        let query = build(&excluded, false);
        assert_eq!(query.filters, excluded_filters, "index filters: {excluded}");
        assert!(!query.starred, "{excluded}");
        assert!(matches_document(&query.ast, &PageDocument::new(1)), "{excluded}");
        for raw in [value.clone(), format!("{operator}:\"{operand}\"")] {
            assert_eq!(parse_history_search_query(&raw), filters, "SQL: {raw}");
            let query = build(&raw, false);
            assert_eq!(query.filters, filters, "index filters: {raw}");
            assert_eq!(query.starred, operator == "is", "{raw}");
            if let Some(field) = field {
                assert!(matches_document(&query.ast, &document(operand, field)), "{raw}");
                assert!(!matches_document(&query.ast, &document("unrelated", field)), "{raw}");
                if field == TextField::Tags {
                    assert!(
                        !matches_document(&query.ast, &document(operand, TextField::Title)),
                        "{raw}"
                    );
                }
            } else {
                assert!(matches_document(&query.ast, &PageDocument::new(1)), "{raw}");
            }
        }
    }
}

#[test]
fn quoted_minuses_and_excluded_quoted_text_keep_their_meaning() {
    for (raw, value) in [("\"-foo\"", "-foo"), ("\"-site:x\"", "-site:x")] {
        assert_literal(raw, value);
    }
    for (raw, filters) in [
        (
            "-\"release candidate\"",
            ParsedHistorySearchQuery {
                excluded_terms: vec!["release candidate".into(), "releasecandidate".into()],
                ..Default::default()
            },
        ),
        (
            "-\"site:x\"",
            ParsedHistorySearchQuery {
                excluded_terms: vec!["site:x".into(), "sitex".into()],
                ..Default::default()
            },
        ),
        (
            "-\"-foo\"",
            ParsedHistorySearchQuery {
                excluded_terms: vec!["-foo".into(), "foo".into()],
                ..Default::default()
            },
        ),
        (
            "-site:\"x\"",
            ParsedHistorySearchQuery { excluded_sites: vec!["x".into()], ..Default::default() },
        ),
        (
            "-site:x",
            ParsedHistorySearchQuery { excluded_sites: vec!["x".into()], ..Default::default() },
        ),
    ] {
        assert_eq!(parse_history_search_query(raw), filters, "SQL: {raw}");
        let query = build(raw, false);
        assert_eq!(query.filters, filters, "index filters: {raw}");
        assert!(matches_document(&query.ast, &PageDocument::new(1)), "{raw}");
    }
}

#[test]
fn curly_and_escaped_quotes_preserve_the_literal_boundary() {
    for (raw, value) in [
        ("“site:x”", "site:x"),
        ("”site:x“", "site:x"),
        (r#""site:\"x\"""#, "site:\"x\""),
        (r#""site:x\\y""#, r"site:x\y"),
    ] {
        assert_literal(raw, value);
    }
}

#[test]
fn a_phrase_the_form_writes_with_escaped_quotes_is_one_phrase() {
    // What `composeWordsQuery` writes for `"release \"candidate\" notes"` in "any" and "none".
    let phrase = r#""release \"candidate\" notes""#;
    let any = build(&format!("{phrase} OR other"), false);
    assert!(matches_document(
        &any.ast,
        &document(r#"release "candidate" notes"#, TextField::Title)
    ));
    assert!(!matches_document(&any.ast, &document("release notes", TextField::Title)));
    // One excluded phrase, in both parsers, not three excluded words.
    let none = format!("-{phrase}");
    let excluded = ParsedHistorySearchQuery {
        excluded_terms: normalized_compact_filter_terms(r#"release "candidate" notes"#),
        ..Default::default()
    };
    assert_eq!(parse_history_search_query(&none), excluded);
    assert_eq!(build(&none, false).filters, excluded);
}

#[test]
fn a_quoted_note_heading_is_text_including_its_cjk_words() {
    assert_literal("\"Note: 會議\"", "Note: 會議");
}

#[test]
fn an_empty_quote_does_not_leak_into_the_next_token() {
    for raw in ["\"\"", "“”", "\"   \""] {
        assert!(tokenize_query(raw).is_empty(), "{raw}");
        assert_eq!(parse_history_search_query(raw), ParsedHistorySearchQuery::default());
        assert!(matches_document(&build(raw, false).ast, &PageDocument::new(1)));
    }
    assert_literal("\"\" \"site:x\"", "site:x");
    let filters =
        ParsedHistorySearchQuery { required_sites: vec!["x".into()], ..Default::default() };
    assert_eq!(parse_history_search_query("\"\" site:x"), filters);
    assert_eq!(build("\"\" site:x", false).filters, filters);
}

#[test]
fn an_unclosed_quote_still_quotes_the_whole_token_or_its_operand() {
    assert_literal("\"site:github.com", "site:github.com");
    for raw in ["tag:\"a b\"", "tag:\"a b"] {
        let filters =
            ParsedHistorySearchQuery { required_tags: vec!["a b".into()], ..Default::default() };
        assert_eq!(parse_history_search_query(raw), filters);
        let query = build(raw, false);
        assert_eq!(query.filters, filters);
        assert!(matches_document(&query.ast, &document("a b", TextField::Tags)));
        assert!(!matches_document(&query.ast, &document("unrelated", TextField::Tags)));
    }
}

#[test]
fn quoting_a_title_operand_keeps_the_field_filter() {
    let query = build("intitle:\"release notes\"", false);
    assert_eq!(query.filters.required_title_terms, vec!["release notes"]);
    assert!(matches_document(&query.ast, &document("release notes", TextField::Title)));
    assert_eq!(query.filters, parse_history_search_query("intitle:\"release notes\""));
}

#[test]
fn an_ordinary_quoted_phrase_still_requires_adjacent_words() {
    assert_literal("\"release notes\"", "release notes");
}

#[test]
fn or_can_join_a_quoted_operator_looking_word() {
    let raw = "required \"site:x\" OR fallback";
    let filters = parse_history_search_query(raw);
    assert!(filters.required_sites.is_empty());
    assert_eq!(filters.keyword_text.as_deref(), Some("required site:x OR fallback"));
    let query = build(raw, false);
    assert_eq!(query.filters, filters);
    for (text, expected) in [
        ("required site:x", true),
        ("required fallback", true),
        ("fallback", false),
        ("required site unrelated x", false),
        ("required unrelated", false),
    ] {
        assert_eq!(
            matches_document(&query.ast, &document(text, TextField::Title)),
            expected,
            "{text}"
        );
    }
}

#[test]
fn an_unknown_operator_is_still_plain_text() {
    let unknown = build("custom:value", false);
    assert_eq!(unknown.filters.keyword_text.as_deref(), Some("custom:value"));
    assert!(matches_document(&unknown.ast, &document("custom value", TextField::Title)));
}
