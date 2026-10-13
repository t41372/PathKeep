//! Regression coverage for visit-taxonomy analysis contracts.
//!
//! ## Responsibilities
//! - Lock what a site is (the public suffix list, private section included).
//! - Lint the built-in site lists: every rule can fire and no domain is listed twice.
//! - Keep the core-site and regional rule fixtures the rule packs must classify.
//! - Guard that the user's site category changes only the site category.
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

use super::url::host_and_parents;
use super::*;

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

// A site's category set by the user (Insights' site page). The ways it goes wrong: it replaces
// the page kinds the rules give the site's pages (a GitHub issue no longer counted as an issue,
// which compare sets and the page card read), it misses the site's other hosts, it overrides the
// parts of the site the lists sort on their own (Gmail and Google Docs called news with Google
// search), or a site no rule knows stays unclassified.
#[test]
fn a_site_category_from_the_user_changes_only_the_site_category() {
    let site = |value: &str, domain_category| TaxonomyOverride {
        target: TaxonomyOverrideTarget::ExactDomain,
        value: value.to_string(),
        domain_category,
        page_category: None,
        interaction_kind: None,
        note: None,
    };
    let classify = |url: &str, title: Option<&str>, overrides: &[TaxonomyOverride]| {
        analyze_visit(
            VisitAnalysisInput {
                url,
                title,
                query: None,
                has_canonical_search_term: false,
                external_referrer_url: None,
                from_visit: None,
            },
            overrides,
        )
        .taxonomy
    };
    let issue_url = "https://github.com/rust-lang/rust/issues/1";
    let github = [site("github.com", DomainCategory::Work)];
    let by_rules = classify(issue_url, Some("Issue"), &[]);
    let by_user = classify(issue_url, Some("Issue"), &github);
    assert_eq!(by_user.domain_category, DomainCategory::Work);
    assert_eq!(by_rules.page_category, PageCategory::Issue);
    assert_eq!(by_user.page_category, by_rules.page_category, "the page kind stays the rules'");
    assert_eq!(by_user.interaction_kind, by_rules.interaction_kind);
    assert_eq!(by_user.source, TaxonomyDecisionSource::UserOverride);
    // The site's other hosts follow it, unless the lists name them on their own.
    let gist = classify("https://gist.github.com/someone/1", None, &github);
    assert_eq!(gist.domain_category, DomainCategory::Work);
    let google = [site("google.com", DomainCategory::News)];
    for url in [
        "https://www.google.com/search?q=sqlite",
        "https://google.com/",
        "https://foo.research.google.com/a",
    ] {
        let visit = classify(url, None, &google);
        assert_eq!(visit.domain_category, DomainCategory::News, "{url}");
        assert_eq!(visit.source, TaxonomyDecisionSource::UserOverride, "{url}");
    }
    for url in [
        "https://mail.google.com/mail/u/0/",
        "https://colab.research.google.com/drive/1",
        "https://x.mail.google.com/",
    ] {
        let by_rules = classify(url, None, &[]);
        assert_ne!(by_rules.domain_category, DomainCategory::Unknown, "{url}");
        assert_eq!(classify(url, None, &google), by_rules, "{url} keeps the lists' category");
    }
    // A host a path rule is for is a part of its own too.
    let hn = "https://news.ycombinator.com/item?id=1";
    let ycombinator = [site("ycombinator.com", DomainCategory::News)];
    assert_eq!(classify(hn, None, &ycombinator), classify(hn, None, &[]));
    assert_eq!(
        classify("https://www.ycombinator.com/", None, &ycombinator).domain_category,
        DomainCategory::News
    );
    // A host the user names is theirs, list or not.
    let gmail = TaxonomyOverride {
        target: TaxonomyOverrideTarget::Host,
        ..site("mail.google.com", DomainCategory::Shopping)
    };
    let mail = classify("https://mail.google.com/mail/u/0/", None, &[google[0].clone(), gmail]);
    assert_eq!(mail.domain_category, DomainCategory::Shopping);
    // What the site's page lists: its parts on the lists, not other sites under it.
    assert_eq!(
        listed_parts("ycombinator.com"),
        [("news.ycombinator.com", DomainCategory::Community)]
    );
    let parts = listed_parts("google.com");
    assert!(parts.contains(&("mail.google.com", DomainCategory::Work)));
    assert!(parts.contains(&("colab.research.google.com", DomainCategory::Developer)));
    assert!(parts.iter().all(|(name, _)| name.ends_with(".google.com")));
    assert!(parts.is_sorted_by_key(|(name, _)| *name));
    assert!(listed_parts("github.io").is_empty(), "each site on github.io is a site of its own");
    assert!(listed_parts("e2e.example").is_empty());
    // A site no rule knows.
    let notes = [site("e2e.example", DomainCategory::News)];
    let page = classify("https://notes.e2e.example/a", Some("A page"), &notes);
    assert_eq!(page.domain_category, DomainCategory::News);
    assert_eq!(page.source, TaxonomyDecisionSource::UserOverride);
}

// The built-in site rules. The ways they go wrong, each checked below:
// - matching climbs past the registrable domain, so a rule could cover a whole public suffix;
// - a rule can never fire: no host under it reaches it, or an exact rule for a site, or a parent
//   of it, whose pages path rules tell apart (exact rules are tried first);
// - a domain is listed twice, so which category wins depends on the order of the lists;
// - a listed domain no visit can produce (uppercase, `www.`, a scheme or path, a public suffix).

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
