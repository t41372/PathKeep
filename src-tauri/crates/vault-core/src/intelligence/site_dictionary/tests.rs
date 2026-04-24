//! Regression coverage for site-dictionary overrides and search rules.
//!
//! ## Responsibilities
//! - Keep search-query extraction behavior stable across the module split.
//! - Verify persisted user overrides still beat built-in taxonomy rules.
//! - Cover custom search-rule merging and precedence.
//!
//! ## Not responsible for
//! - Exhaustively retesting `visit_taxonomy` rule packs.
//! - Testing route-level Core Intelligence read models.
//! - Exercising frontend Settings forms.
//!
//! ## Dependencies
//! - In-memory SQLite for schema and persistence checks.
//! - `models::SearchEngineRuleInput` for Settings-facing rule upserts.
//!
//! ## Performance notes
//! - Tests use tiny fixtures but cover the hot-path invariant that callers load
//!   rules/overrides once and classify visits without further database access.

use super::{
    SiteDictionaryOverrideTargetKind, SiteDictionaryOverrideUpsert, classify_visit,
    ensure_search_engine_rule_schema, ensure_site_dictionary_override_schema,
    list_search_engine_rules, load_enabled_search_engine_rules, load_site_dictionary_overrides,
    normalize_query, upsert_search_engine_rule, upsert_site_dictionary_override,
};
use crate::models::SearchEngineRuleInput;
use rusqlite::Connection;

#[test]
fn classify_visit_extracts_search_engine_and_query() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    ensure_site_dictionary_override_schema(&connection).expect("override schema");
    ensure_search_engine_rule_schema(&connection).expect("search engine schema");
    let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
    let rules = load_enabled_search_engine_rules(&connection).expect("load rules");
    let entry = classify_visit(
        "https://www.google.com/search?q=sqlite+wal+checkpoint&utm_source=test",
        Some("sqlite wal checkpoint - Google Search"),
        None,
        true,
        None,
        None,
        &overrides,
        &rules,
    );
    assert_eq!(entry.registrable_domain, "google.com");
    assert_eq!(entry.search_engine.as_deref(), Some("google"));
    assert_eq!(entry.search_query.as_deref(), Some("sqlite wal checkpoint"));
    assert!(entry.canonical_url.contains("q=sqlite+wal+checkpoint"));
    assert!(!entry.canonical_url.contains("utm_source"));
}

#[test]
fn normalize_query_canonicalizes_whitespace() {
    assert_eq!(normalize_query("  Tauri   v2   Docs "), "tauri v2 docs");
}

#[test]
fn persisted_user_override_beats_builtin_rule_pack() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    upsert_site_dictionary_override(
        &connection,
        &SiteDictionaryOverrideUpsert {
            target_kind: SiteDictionaryOverrideTargetKind::ExactDomain,
            target_value: "github.com".to_string(),
            domain_category: Some("work".to_string()),
            page_category: Some("dashboard".to_string()),
            interaction_kind: Some("manage".to_string()),
            display_name: Some("GitHub Work".to_string()),
            search_engine: None,
            is_noisy: false,
            note: Some("tests".to_string()),
        },
    )
    .expect("insert override");
    let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
    let rules = load_enabled_search_engine_rules(&connection).expect("load rules");

    let entry = classify_visit(
        "https://github.com/example/repo/issues/42",
        Some("Issue 42"),
        None,
        false,
        None,
        Some(1),
        &overrides,
        &rules,
    );

    assert_eq!(entry.taxonomy_source, "user-override");
    assert_eq!(entry.domain_category, "work");
    assert_eq!(entry.page_category, "dashboard");
    assert_eq!(entry.display_name.as_deref(), Some("GitHub Work"));
}

#[test]
fn builtin_rules_cover_bilibili_and_github_search() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    ensure_site_dictionary_override_schema(&connection).expect("override schema");
    ensure_search_engine_rule_schema(&connection).expect("search engine schema");
    let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
    let rules = load_enabled_search_engine_rules(&connection).expect("load rules");

    let bilibili = classify_visit(
        "https://search.bilibili.com/all?keyword=sqlite+wal",
        Some("sqlite wal - 哔哩哔哩"),
        None,
        false,
        None,
        None,
        &overrides,
        &rules,
    );
    assert_eq!(bilibili.search_engine.as_deref(), Some("bilibili"));
    assert_eq!(bilibili.search_query.as_deref(), Some("sqlite wal"));

    let github = classify_visit(
        "https://github.com/search?q=pathkeep+sqlite",
        Some("Repository search results"),
        None,
        false,
        None,
        None,
        &overrides,
        &rules,
    );
    assert_eq!(github.search_engine.as_deref(), Some("github"));
    assert_eq!(github.search_query.as_deref(), Some("pathkeep sqlite"));
}

#[test]
fn custom_search_rules_override_builtin_matching() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    ensure_site_dictionary_override_schema(&connection).expect("override schema");
    upsert_search_engine_rule(
        &connection,
        &SearchEngineRuleInput {
            rule_id: Some("custom-github-code".to_string()),
            engine_id: "github-code".to_string(),
            display_name: "GitHub Code".to_string(),
            host_pattern: "github.com".to_string(),
            path_prefix: Some("/search".to_string()),
            query_param_key: "q".to_string(),
            enabled: true,
            note: Some("custom".to_string()),
            example_url: Some("https://github.com/search?q=pathkeep".to_string()),
        },
    )
    .expect("upsert custom rule");
    let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
    let rules = load_enabled_search_engine_rules(&connection).expect("load rules");

    let entry = classify_visit(
        "https://github.com/search?q=pathkeep+sqlite",
        Some("Repository search results"),
        None,
        false,
        None,
        None,
        &overrides,
        &rules,
    );
    assert_eq!(entry.search_engine.as_deref(), Some("github-code"));
    assert_eq!(entry.search_query.as_deref(), Some("pathkeep sqlite"));
}

#[test]
fn list_rules_merges_builtin_and_custom_rules() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    upsert_search_engine_rule(
        &connection,
        &SearchEngineRuleInput {
            rule_id: Some("custom-amazon-de".to_string()),
            engine_id: "amazon-de".to_string(),
            display_name: "Amazon DE".to_string(),
            host_pattern: "amazon.de".to_string(),
            path_prefix: Some("/s".to_string()),
            query_param_key: "k".to_string(),
            enabled: true,
            note: None,
            example_url: None,
        },
    )
    .expect("upsert custom rule");

    let rules = list_search_engine_rules(&connection).expect("list rules");
    assert!(rules.iter().any(|rule| rule.rule_id == "builtin:bilibili"));
    assert!(rules.iter().any(|rule| rule.rule_id == "custom-amazon-de"));
}
