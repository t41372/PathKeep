//! Regression coverage for visit-taxonomy analysis contracts.
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
//! - Public `crate::visit_taxonomy` exports re-exported by the module façade.
//!
//! ## Performance notes
//! - Tests use single-visit fixtures and do not allocate large archive-shaped
//!   datasets.

use super::types::TAXONOMY_VERSION;
use super::url::host_and_parents;
use super::*;

#[test]
fn taxonomy_serialized_identifiers_cover_every_public_variant() {
    assert!(taxonomy_version().starts_with(&format!("{TAXONOMY_VERSION}|")));
    assert_eq!(EvidenceTier::TierA.as_str(), "tier-a");
    assert_eq!(EvidenceTier::TierB.as_str(), "tier-b");
    assert_eq!(EvidenceTier::TierC.as_str(), "tier-c");

    let domain_categories = [
        (DomainCategory::Ai, "ai"),
        (DomainCategory::Community, "community"),
        (DomainCategory::Developer, "developer"),
        (DomainCategory::Docs, "docs"),
        (DomainCategory::Education, "education"),
        (DomainCategory::Entertainment, "entertainment"),
        (DomainCategory::Finance, "finance"),
        (DomainCategory::News, "news"),
        (DomainCategory::Search, "search"),
        (DomainCategory::Shopping, "shopping"),
        (DomainCategory::Social, "social"),
        (DomainCategory::Travel, "travel"),
        (DomainCategory::Video, "video"),
        (DomainCategory::Work, "work"),
        (DomainCategory::Unknown, "unknown"),
    ];
    for (category, expected) in domain_categories {
        assert_eq!(category.as_str(), expected);
    }

    let page_categories = [
        (PageCategory::ArticlePage, "article-page"),
        (PageCategory::CategoryPage, "category-page"),
        (PageCategory::Dashboard, "dashboard"),
        (PageCategory::DocsPage, "docs-page"),
        (PageCategory::ForumThread, "forum-thread"),
        (PageCategory::Home, "home"),
        (PageCategory::Issue, "issue"),
        (PageCategory::ProductPage, "product-page"),
        (PageCategory::Profile, "profile"),
        (PageCategory::PullRequest, "pull-request"),
        (PageCategory::Repo, "repo"),
        (PageCategory::SearchResults, "search-results"),
        (PageCategory::VideoPage, "video-page"),
        (PageCategory::Unknown, "unknown"),
    ];
    for (category, expected) in page_categories {
        assert_eq!(category.as_str(), expected);
    }

    let interaction_kinds = [
        (InteractionKind::Compare, "compare"),
        (InteractionKind::Discover, "discover"),
        (InteractionKind::Discuss, "discuss"),
        (InteractionKind::Learn, "learn"),
        (InteractionKind::Manage, "manage"),
        (InteractionKind::Resolve, "resolve"),
        (InteractionKind::Transact, "transact"),
        (InteractionKind::Watch, "watch"),
        (InteractionKind::Unknown, "unknown"),
    ];
    for (kind, expected) in interaction_kinds {
        assert_eq!(kind.as_str(), expected);
    }

    let decision_sources = [
        (TaxonomyDecisionSource::UserOverride, "user-override"),
        (TaxonomyDecisionSource::ExactDomainRule, "exact-domain-rule"),
        (TaxonomyDecisionSource::HostPathRule, "host-path-query-rule"),
        (TaxonomyDecisionSource::LexiconRule, "title-query-lexicon-rule"),
        (TaxonomyDecisionSource::OptionalModelFallback, "optional-model-fallback"),
        (TaxonomyDecisionSource::Unknown, "unknown"),
    ];
    for (source, expected) in decision_sources {
        assert_eq!(source.as_str(), expected);
    }
}

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

// What a site is, the way a browser splits them (the full public suffix list). The ways it goes
// wrong, each checked below:
// - the private section left out, so every `*.github.io` is one site;
// - a suffix of several labels (`co.uk`), a wildcard (`*.ck`) or an exception (`!www.ck`) split
//   by label count;
// - a host that is itself a public suffix (`github.io`), a single label or an address split into
//   pieces (`127.0.0.1` as the site `0.1`);
// - a refreshed list that leaves the version builds record unchanged, so Insights keep old sites.
#[test]
fn sites_are_split_as_a_browser_splits_them() {
    for (host, site) in [
        ("rust-lang.github.io", "rust-lang.github.io"),
        ("docs.tokio-rs.github.io", "tokio-rs.github.io"),
        ("docs.news.bbc.co.uk", "bbc.co.uk"),
        ("subdomain.example.com.cn", "example.com.cn"),
        ("a.b.c.ck", "b.c.ck"),
        ("www.ck", "www.ck"),
        ("mail.google.com", "google.com"),
        ("github.io", "github.io"),
        ("co.uk", "co.uk"),
        ("localhost", "localhost"),
        ("nas.home.arpa", "nas.home.arpa"),
        ("127.0.0.1", "127.0.0.1"),
        ("[::1]", "[::1]"),
        ("GitHub.COM.", "github.com"),
        ("", ""),
    ] {
        assert_eq!(registrable_domain_for_host(host), site, "{host}");
    }
    assert_eq!(
        registrable_domain_for_url("https://docs.news.bbc.co.uk/path").as_deref(),
        Some("bbc.co.uk")
    );
    let version = super::public_suffix_list_version();
    assert!(!version.is_empty() && super::url_rules_version().ends_with(version));
    assert_eq!(
        extract_search_query_from_url("https://duckduckgo.com/?q=PathKeep+archive").as_deref(),
        Some("PathKeep archive")
    );
    assert!(extract_search_query_from_url("https://example.com/?q=not-search").is_none());
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
    assert_eq!(tokenize_text("猫"), vec!["猫".to_string()]);
}

#[test]
fn normalizer_rejects_hosts_that_trim_to_empty() {
    assert!(normalize_visit_url("https://./path").is_none());
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
    // The title makes it a docs page; it says nothing about what the site is.
    assert_eq!(docs.taxonomy.page_category, PageCategory::DocsPage);
    assert_eq!(docs.taxonomy.domain_category, DomainCategory::Unknown);
    assert_eq!(docs.taxonomy.source, TaxonomyDecisionSource::LexiconRule);

    // Latin words match whole: "rapid" holds "api" and "explanation" holds "plan"; neither counts.
    let words = analyze_visit(
        VisitAnalysisInput {
            url: "https://mystery.example/post",
            title: Some("Rapid explanation"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(words.taxonomy.page_category, PageCategory::Unknown);

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

// The built-in site rules. The ways they go wrong, each checked below:
// - a host with a rule of its own is classified by its parent's (Google Docs counted as search);
// - matching climbs past the registrable domain, so a rule could cover a whole public suffix;
// - a parent matches without a label boundary (`notgoogle.com` taken for Google);
// - a rule can never fire: an exact rule for a site, or a parent of it, whose pages path rules
//   tell apart (exact rules are tried first);
// - a domain is listed twice, so which category wins depends on the order of the lists;
// - a listed domain no visit can produce (uppercase, `www.`, a scheme or path, a public suffix);
// - a site that path rules tell apart has pages no rule matches, so most of its visits are unknown;
// - the rules change but the version a build records does not, so archives keep the old labels;
// - a word in one page's title labels the whole site ("tokio vs smol" on a blog made it shopping).

fn classify(url: &str) -> TaxonomyClassification {
    analyze_visit(
        VisitAnalysisInput {
            url,
            title: None,
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    )
    .taxonomy
}

#[test]
fn a_host_with_its_own_rule_beats_its_parent() {
    assert_eq!(
        classify("https://docs.google.com/document/d/x/edit").domain_category,
        DomainCategory::Work
    );
    assert_eq!(classify("https://mail.google.com/mail/u/0/").domain_category, DomainCategory::Work);
    assert_eq!(classify("https://gemini.google.com/app").domain_category, DomainCategory::Ai);
    let search = classify("https://www.google.com/search?q=sqlite");
    assert_eq!(search.domain_category, DomainCategory::Search);
    assert_eq!(search.page_category, PageCategory::SearchResults);
    // A host without a rule of its own takes its parent's.
    assert_eq!(
        classify("https://en.wikipedia.org/wiki/Search_engine").domain_category,
        DomainCategory::Education
    );
    assert_eq!(classify("https://notgoogle.com/").domain_category, DomainCategory::Unknown);
}

#[test]
fn host_walk_stops_at_the_domain_the_owner_registered() {
    let walk = |host, registrable| host_and_parents(host, registrable).collect::<Vec<_>>();
    assert_eq!(
        walk("a.b.example.co.uk", "example.co.uk"),
        ["a.b.example.co.uk", "b.example.co.uk", "example.co.uk"]
    );
    // A site under a private suffix reaches its owner's domain, never the public suffix above.
    assert_eq!(
        walk("tokio.readthedocs.io", "tokio.readthedocs.io"),
        ["tokio.readthedocs.io", "readthedocs.io"]
    );
    assert_eq!(
        walk("rust-lang.github.io", "rust-lang.github.io"),
        ["rust-lang.github.io", "github.io"]
    );
    assert_eq!(walk("127.0.0.1", "127.0.0.1"), ["127.0.0.1"]);
    assert_eq!(walk("example.com", "example.com"), ["example.com"]);
    assert_eq!(walk("localhost", "localhost"), ["localhost"]);
    // A registrable domain the host is not under: only the host itself.
    assert_eq!(walk("a.example.com", "other.com"), ["a.example.com"]);
    assert_eq!(walk("a.example.com", ""), ["a.example.com"]);
}

#[test]
fn every_site_rule_can_fire_and_is_listed_once() {
    let mut seen = std::collections::HashMap::new();
    for pack in super::rules::RULE_PACKS {
        for rule in pack.exact_domains {
            let domain = rule.domain;
            assert_eq!(domain, domain.trim().to_ascii_lowercase(), "{domain} is not normalized");
            assert!(
                !domain.starts_with("www.") && !domain.contains("://") && !domain.contains('/'),
                "{domain} is not a host"
            );
            let registrable = registrable_domain_for_host(domain);
            assert!(
                domain == registrable || domain.ends_with(&format!(".{registrable}")),
                "{domain} is above its registrable domain {registrable}"
            );
            // A single-label host (`localhost`) has no parents; it matches only itself.
            if domain.contains('.') {
                let probe = format!("probe.{domain}");
                assert!(
                    host_and_parents(&probe, &registrable_domain_for_host(&probe))
                        .any(|h| h == domain),
                    "a host under {domain} never reaches its rule"
                );
            }
            if let Some(other) = seen.insert(domain, pack.id) {
                panic!("{domain} is listed in both {other} and {}", pack.id);
            }
        }
    }
    for pack in super::rules::RULE_PACKS {
        for rule in pack.host_path_rules {
            for host in rule.host_suffixes {
                for exact in host_and_parents(host, &registrable_domain_for_host(host)) {
                    assert!(
                        !seen.contains_key(exact),
                        "the exact rule for {exact} hides the path rule {} for {host}",
                        rule.rule.id
                    );
                }
            }
        }
    }
}

#[test]
fn sites_told_apart_by_path_still_have_a_category_on_every_page() {
    let issue = classify("https://github.com/example/repo/issues/42");
    assert_eq!(issue.page_category, PageCategory::Issue);
    let notifications = classify("https://github.com/notifications");
    assert_eq!(notifications.domain_category, DomainCategory::Developer);
    assert_eq!(notifications.page_category, PageCategory::Unknown);
    assert_eq!(classify("https://github.com/").domain_category, DomainCategory::Developer);

    assert_eq!(
        classify("https://www.youtube.com/watch?v=abc").page_category,
        PageCategory::VideoPage
    );
    let home = classify("https://www.youtube.com/");
    assert_eq!(home.domain_category, DomainCategory::Video);
    assert_eq!(home.page_category, PageCategory::Unknown);

    assert_eq!(classify("https://www.amazon.com/").domain_category, DomainCategory::Shopping);
    assert_eq!(classify("https://www.reddit.com/").domain_category, DomainCategory::Community);
    assert_eq!(classify("https://www.nytimes.com/").domain_category, DomainCategory::News);
}

#[test]
fn taxonomy_version_names_every_pack() {
    let version = taxonomy_version();
    assert!(version.starts_with(TAXONOMY_VERSION), "{version}");
    for pack in super::rules::RULE_PACKS {
        assert!(version.contains(&format!("{}:{}", pack.id, pack.version)), "{version}");
    }
}

#[test]
fn a_page_title_does_not_label_the_site() {
    let post = analyze_visit(
        VisitAnalysisInput {
            url: "https://without.boats/blog/async-runtimes/",
            title: Some("tokio vs smol: choosing an async runtime"),
            query: None,
            has_canonical_search_term: false,
            external_referrer_url: None,
            from_visit: None,
        },
        &[],
    );
    assert_eq!(post.taxonomy.domain_category, DomainCategory::Unknown);
}
