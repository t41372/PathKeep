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
use crate::visit_taxonomy::{
    DomainCategory, InteractionKind, PageCategory, TaxonomyOverrideTarget,
};
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

    upsert_site_dictionary_override(
        &connection,
        &SiteDictionaryOverrideUpsert {
            target_kind: SiteDictionaryOverrideTargetKind::Host,
            target_value: "app.hosted.example".to_string(),
            domain_category: Some("docs".to_string()),
            page_category: Some("docs_page".to_string()),
            interaction_kind: Some("learn".to_string()),
            display_name: Some("Hosted App Docs".to_string()),
            search_engine: None,
            is_noisy: false,
            note: None,
        },
    )
    .expect("insert host override");
    upsert_site_dictionary_override(
        &connection,
        &SiteDictionaryOverrideUpsert {
            target_kind: SiteDictionaryOverrideTargetKind::UrlPrefix,
            target_value: "https://prefix.example/search".to_string(),
            domain_category: Some("search".to_string()),
            page_category: Some("search_results".to_string()),
            interaction_kind: Some("discover".to_string()),
            display_name: Some("Prefix Search".to_string()),
            search_engine: Some("prefix".to_string()),
            is_noisy: false,
            note: None,
        },
    )
    .expect("insert url prefix override");
    let overrides = load_site_dictionary_overrides(&connection).expect("reload overrides");
    let docs_entry = classify_visit(
        "https://app.hosted.example/docs",
        Some("Actions"),
        None,
        false,
        None,
        None,
        &overrides,
        &rules,
    );
    assert_eq!(docs_entry.display_name.as_deref(), Some("Hosted App Docs"));
    let search_entry = classify_visit(
        "https://prefix.example/search?q=pathkeep",
        Some("Search"),
        None,
        false,
        None,
        None,
        &overrides,
        &rules,
    );
    assert_eq!(search_entry.display_name.as_deref(), Some("Prefix Search"));
    assert_eq!(search_entry.search_engine.as_deref(), Some("prefix"));
    assert_eq!(SiteDictionaryOverrideTargetKind::from_str("unsupported"), None);
}

#[test]
fn persisted_overrides_parse_all_taxonomy_keys_and_target_kinds() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    let cases = [
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "ai.example",
            "ai",
            "search-results",
            "compare",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Ai,
            PageCategory::SearchResults,
            InteractionKind::Compare,
        ),
        (
            SiteDictionaryOverrideTargetKind::Host,
            "community.example",
            "community",
            "docs-page",
            "discover",
            TaxonomyOverrideTarget::Host,
            DomainCategory::Community,
            PageCategory::DocsPage,
            InteractionKind::Discover,
        ),
        (
            SiteDictionaryOverrideTargetKind::UrlPrefix,
            "https://developer.example/repo",
            "developer",
            "repo",
            "discuss",
            TaxonomyOverrideTarget::UrlPrefix,
            DomainCategory::Developer,
            PageCategory::Repo,
            InteractionKind::Discuss,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "docs.example",
            "docs",
            "issue",
            "learn",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Docs,
            PageCategory::Issue,
            InteractionKind::Learn,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "education.example",
            "education",
            "pull_request",
            "manage",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Education,
            PageCategory::PullRequest,
            InteractionKind::Manage,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "entertainment.example",
            "entertainment",
            "forum_thread",
            "resolve",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Entertainment,
            PageCategory::ForumThread,
            InteractionKind::Resolve,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "finance.example",
            "finance",
            "product_page",
            "transact",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Finance,
            PageCategory::ProductPage,
            InteractionKind::Transact,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "news.example",
            "news",
            "category_page",
            "watch",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::News,
            PageCategory::CategoryPage,
            InteractionKind::Watch,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "search.example",
            "search",
            "video_page",
            "unknown",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Search,
            PageCategory::VideoPage,
            InteractionKind::Unknown,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "shopping.example",
            "shopping",
            "article_page",
            "compare",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Shopping,
            PageCategory::ArticlePage,
            InteractionKind::Compare,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "social.example",
            "social",
            "profile",
            "discover",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Social,
            PageCategory::Profile,
            InteractionKind::Discover,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "travel.example",
            "travel",
            "dashboard",
            "discuss",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Travel,
            PageCategory::Dashboard,
            InteractionKind::Discuss,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "video.example",
            "video",
            "home",
            "learn",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Video,
            PageCategory::Home,
            InteractionKind::Learn,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "work.example",
            "work",
            "unknown",
            "manage",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Work,
            PageCategory::Unknown,
            InteractionKind::Manage,
        ),
        (
            SiteDictionaryOverrideTargetKind::ExactDomain,
            "unknown.example",
            "unknown",
            "search_results",
            "resolve",
            TaxonomyOverrideTarget::ExactDomain,
            DomainCategory::Unknown,
            PageCategory::SearchResults,
            InteractionKind::Resolve,
        ),
    ];

    for (
        target_kind,
        target_value,
        domain_category,
        page_category,
        interaction_kind,
        expected_target,
        expected_domain,
        expected_page,
        expected_interaction,
    ) in cases
    {
        upsert_site_dictionary_override(
            &connection,
            &SiteDictionaryOverrideUpsert {
                target_kind,
                target_value: target_value.to_string(),
                domain_category: Some(domain_category.to_string()),
                page_category: Some(page_category.to_string()),
                interaction_kind: Some(interaction_kind.to_string()),
                display_name: None,
                search_engine: None,
                is_noisy: false,
                note: Some("taxonomy coverage".to_string()),
            },
        )
        .expect("insert taxonomy override");
        let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
        let override_rule = overrides
            .iter()
            .find(|candidate| candidate.target_value == target_value)
            .expect("loaded override");
        let taxonomy = override_rule.taxonomy_override.as_ref().expect("taxonomy override");
        assert_eq!(taxonomy.target, expected_target);
        assert_eq!(taxonomy.domain_category, expected_domain);
        assert_eq!(taxonomy.page_category, expected_page);
        assert_eq!(taxonomy.interaction_kind, expected_interaction);
    }
}

#[test]
fn persisted_overrides_ignore_partial_or_invalid_taxonomy_rows() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    for (target, domain_category, page_category, interaction_kind) in [
        ("partial.example", Some("work"), None, Some("manage")),
        ("bad-domain.example", Some("bad-domain"), Some("home"), Some("manage")),
        ("bad-page.example", Some("work"), Some("bad-page"), Some("manage")),
        ("bad-interaction.example", Some("work"), Some("home"), Some("bad-interaction")),
    ] {
        upsert_site_dictionary_override(
            &connection,
            &SiteDictionaryOverrideUpsert {
                target_kind: SiteDictionaryOverrideTargetKind::ExactDomain,
                target_value: target.to_string(),
                domain_category: domain_category.map(str::to_string),
                page_category: page_category.map(str::to_string),
                interaction_kind: interaction_kind.map(str::to_string),
                display_name: None,
                search_engine: None,
                is_noisy: false,
                note: None,
            },
        )
        .expect("insert invalid override");
    }

    let overrides = load_site_dictionary_overrides(&connection).expect("load overrides");
    assert_eq!(overrides.len(), 4);
    assert!(overrides.iter().all(|override_rule| override_rule.taxonomy_override.is_none()));
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
            rule_id: None,
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
    assert!(rules.iter().any(|rule| {
        rule.rule_id.starts_with("custom-search-rule-") && rule.engine_id == "amazon-de"
    }));
}
