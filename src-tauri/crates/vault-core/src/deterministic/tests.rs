//! Regression coverage for deterministic analysis contracts.
//!
//! ## Responsibilities
//! - Lock URL normalization, query extraction, evidence tiers, and taxonomy
//!   rule ordering.
//! - Prove the directory-module split preserves the public API surface.
//! - Guard user override precedence over built-in rule packs.
//!
//! ## Not responsible for
//! - Testing Core Intelligence persistence or rebuild orchestration.
//! - Testing site-dictionary database overrides.
//! - Benchmarking archive-scale streaming behavior.
//!
//! ## Dependencies
//! - Public `crate::deterministic` exports re-exported by the module façade.
//!
//! ## Performance notes
//! - Tests use single-visit fixtures and do not allocate large archive-shaped
//!   datasets.

use super::*;

#[test]
fn normalizes_search_urls_and_strips_tracking_params() {
    let normalized = normalize_visit_url(
        "https://www.google.com/search?q=sqlite+wal&utm_source=newsletter&gclid=abc123",
    )
    .expect("normalized search url");

    assert_eq!(normalized.host, "www.google.com");
    assert_eq!(normalized.registrable_domain, "google.com");
    assert_eq!(normalized.subdomain.as_deref(), Some("www"));
    assert_eq!(normalized.search_query.as_deref(), Some("sqlite wal"));
    assert!(normalized.is_search_results);
    assert_eq!(normalized.dropped_tracking_params, vec!["utm_source", "gclid"]);
    assert_eq!(normalized.canonical_url, "https://www.google.com/search?q=sqlite+wal");
}

#[test]
fn keeps_semantic_ids_and_extracts_cjk_search_terms() {
    let normalized =
        normalize_visit_url("https://www.baidu.com/s?wd=%E6%9C%AC%E5%9C%B0+AI&spm=track")
            .expect("normalized baidu url");

    assert_eq!(normalized.registrable_domain, "baidu.com");
    assert_eq!(normalized.search_query.as_deref(), Some("本地 AI"));
    assert_eq!(normalized.canonical_url, "https://www.baidu.com/s?wd=%E6%9C%AC%E5%9C%B0+AI");
}

#[test]
fn registrable_domain_handles_common_multi_label_suffixes() {
    assert_eq!(
        registrable_domain_for_url("https://docs.news.bbc.co.uk/path").as_deref(),
        Some("bbc.co.uk")
    );
    assert_eq!(
        registrable_domain_for_url("https://subdomain.example.com.cn/path").as_deref(),
        Some("example.com.cn")
    );
}

#[test]
fn non_search_urls_keep_semantic_query_params() {
    let normalized = normalize_visit_url(
        "https://github.com/example/repo/issues/42?tab=comments&utm_campaign=tracker",
    )
    .expect("normalized issue url");

    assert!(!normalized.is_search_results);
    assert!(normalized.search_query.is_none());
    assert_eq!(normalized.registrable_domain, "github.com");
    assert_eq!(normalized.canonical_url, "https://github.com/example/repo/issues/42?tab=comments");
}

#[test]
fn script_aware_tokenization_handles_latin_and_cjk() {
    let tokens = tokenize_text("SQLite WAL 文档 教學");
    assert!(tokens.contains(&"sqlite".to_string()));
    assert!(tokens.contains(&"wal".to_string()));
    assert!(tokens.contains(&"文档".to_string()));
    assert!(tokens.contains(&"教學".to_string()));
}

#[test]
fn evidence_tier_prefers_canonical_search_and_referrer_chain() {
    let analysis = analyze_visit(
        VisitAnalysisInput {
            url: "https://www.google.com/search?q=sqlite+checkpoint",
            title: Some("Google Search"),
            query: Some("sqlite checkpoint"),
            has_canonical_search_term: true,
            external_referrer_url: Some("https://example.com"),
            from_visit: Some(41),
        },
        &[],
    );

    assert_eq!(analysis.evidence.tier, EvidenceTier::TierA);
    assert!(analysis.evidence.reasons.iter().any(|reason| reason == "canonical-search-term"));
    assert!(analysis.evidence.reasons.iter().any(|reason| reason == "navigation-anchor"));
}

#[test]
fn evidence_tier_falls_back_to_structural_and_then_time_only() {
    let structural = analyze_visit(
        VisitAnalysisInput {
            url: "https://example.com/docs/sqlite",
            title: Some("SQLite docs"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(structural.evidence.tier, EvidenceTier::TierB);

    let weak = analyze_visit(
        VisitAnalysisInput {
            url: "https://example.com/",
            title: None,
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(weak.evidence.tier, EvidenceTier::TierC);
    assert_eq!(weak.evidence.reasons, vec!["time-adjacency-only"]);
}

#[test]
fn taxonomy_exact_domain_and_host_path_rules_cover_core_sites() {
    let search = analyze_visit(
        VisitAnalysisInput {
            url: "https://www.google.com/search?q=sqlite+wal",
            title: Some("Google Search"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(search.taxonomy.domain_category, DomainCategory::Search);
    assert_eq!(search.taxonomy.page_category, PageCategory::SearchResults);

    let issue = analyze_visit(
        VisitAnalysisInput {
            url: "https://github.com/example/repo/issues/42",
            title: Some("Issue 42"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(issue.taxonomy.domain_category, DomainCategory::Developer);
    assert_eq!(issue.taxonomy.page_category, PageCategory::Issue);

    let pr = analyze_visit(
        VisitAnalysisInput {
            url: "https://github.com/example/repo/pull/9",
            title: Some("PR 9"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(pr.taxonomy.page_category, PageCategory::PullRequest);
    assert_eq!(pr.taxonomy.interaction_kind, InteractionKind::Resolve);
}

#[test]
fn taxonomy_cn_and_us_packs_cover_regional_sites() {
    let zhihu = analyze_visit(
        VisitAnalysisInput {
            url: "https://www.zhihu.com/question/123456",
            title: Some("如何做好备份？"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(zhihu.taxonomy.domain_category, DomainCategory::Community);

    let amazon = analyze_visit(
        VisitAnalysisInput {
            url: "https://www.amazon.com/dp/B0TEST1234",
            title: Some("Archive drive"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(amazon.taxonomy.domain_category, DomainCategory::Shopping);
    assert_eq!(amazon.taxonomy.page_category, PageCategory::ProductPage);
    assert_eq!(amazon.taxonomy.interaction_kind, InteractionKind::Compare);
}

#[test]
fn taxonomy_lexicon_and_unknown_fallback_are_honest() {
    let docs = analyze_visit(
        VisitAnalysisInput {
            url: "https://example.com/opaque-path",
            title: Some("SQLite WAL 文档"),
            query: Some("sqlite 教學"),
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(docs.taxonomy.domain_category, DomainCategory::Docs);
    assert_eq!(docs.taxonomy.source, TaxonomyDecisionSource::LexiconRule);

    let unknown = analyze_visit(
        VisitAnalysisInput {
            url: "https://mystery.example",
            title: Some("Untitled"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(unknown.taxonomy.domain_category, DomainCategory::Unknown);
    assert_eq!(unknown.taxonomy.page_category, PageCategory::Unknown);
}

#[test]
fn user_override_beats_pack_rules() {
    let override_rule = TaxonomyOverride {
        target: TaxonomyOverrideTarget::ExactDomain,
        value: "google.com".to_string(),
        domain_category: DomainCategory::Work,
        page_category: PageCategory::Dashboard,
        interaction_kind: InteractionKind::Manage,
        note: Some("manual-review".to_string()),
    };
    let analysis = analyze_visit(
        VisitAnalysisInput {
            url: "https://www.google.com/search?q=sqlite",
            title: Some("Google Search"),
            query: Some("sqlite"),
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[override_rule],
    );

    assert_eq!(analysis.taxonomy.source, TaxonomyDecisionSource::UserOverride);
    assert_eq!(analysis.taxonomy.domain_category, DomainCategory::Work);
    assert_eq!(analysis.taxonomy.page_category, PageCategory::Dashboard);
}
