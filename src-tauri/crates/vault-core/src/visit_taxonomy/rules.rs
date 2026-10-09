//! Built-in visit-taxonomy rule packs.
//!
//! ## Responsibilities
//! - Own the static host/path and lexicon rule packs, and the registry of packs with the site
//!   lists in `sites`.
//! - Keep regional taxonomy packs separate from visit classification logic.
//! - Preserve the accepted pack ids and versions used in explanation metadata.
//!
//! ## Not responsible for
//! - URL normalization or text tokenization.
//! - Applying user overrides ahead of built-in rules.
//! - Persisting taxonomy decisions.
//!
//! ## Dependencies
//! - Public visit-taxonomy enums from `types`.
//!
//! ## Performance notes
//! - Rule packs are static slices. Matching cost is bounded by the number of
//!   built-in rules and does not grow with archive size.

use super::sites;
use super::types::{DomainCategory, InteractionKind, PageCategory, TAXONOMY_VERSION};
use std::sync::LazyLock;

/// Compact category tuple shared by all built-in rule variants.
#[derive(Debug, Clone, Copy)]
pub(super) struct TaxonomyRule {
    pub(super) id: &'static str,
    pub(super) domain_category: DomainCategory,
    pub(super) page_category: PageCategory,
    pub(super) interaction_kind: InteractionKind,
}

/// Exact registrable-domain match inside a rule pack.
#[derive(Debug, Clone, Copy)]
pub(super) struct ExactDomainRule {
    pub(super) domain: &'static str,
    pub(super) rule: TaxonomyRule,
}

/// Host/path/query match inside a rule pack.
#[derive(Debug, Clone, Copy)]
pub(super) struct HostPathRule {
    pub(super) rule: TaxonomyRule,
    pub(super) host_suffixes: &'static [&'static str],
    pub(super) path_prefixes: &'static [&'static str],
    pub(super) path_contains: &'static [&'static str],
    pub(super) query_keys: &'static [&'static str],
    pub(super) query_value_contains: &'static [&'static str],
    pub(super) path_segment_count_at_least: usize,
}

/// Text-token match inside a rule pack. A title or query says what kind of page a visit is, not
/// what the site is (a Rust blog post titled "tokio vs smol" is not shopping), so these rules
/// leave the site's category unknown and set only the page's kind.
#[derive(Debug, Clone, Copy)]
pub(super) struct LexiconRule {
    pub(super) rule: TaxonomyRule,
    pub(super) tokens: &'static [&'static str],
}

/// Versioned built-in taxonomy rule pack.
#[derive(Debug, Clone, Copy)]
pub(super) struct TaxonomyRulePack {
    pub(super) id: &'static str,
    pub(super) version: &'static str,
    pub(super) exact_domains: &'static [ExactDomainRule],
    pub(super) host_path_rules: &'static [HostPathRule],
    pub(super) lexicon_rules: &'static [LexiconRule],
}

/// A site, or one host of it, all of whose pages are in `domain_category`. What kind of page each
/// is stays unknown: one rule covers every page of the host.
pub(super) const fn site(domain: &'static str, domain_category: DomainCategory) -> ExactDomainRule {
    rule(domain, domain_category, PageCategory::Unknown, InteractionKind::Unknown)
}

/// A documentation site: every page of it is a docs page.
pub(super) const fn docs(domain: &'static str) -> ExactDomainRule {
    rule(domain, DomainCategory::Docs, PageCategory::DocsPage, InteractionKind::Learn)
}

/// A search engine: its pages are result pages.
pub(super) const fn search_engine(domain: &'static str) -> ExactDomainRule {
    rule(domain, DomainCategory::Search, PageCategory::SearchResults, InteractionKind::Discover)
}

/// A site whose pages are all one kind of page as well. Its domain names the rule.
pub(super) const fn rule(
    domain: &'static str,
    domain_category: DomainCategory,
    page_category: PageCategory,
    interaction_kind: InteractionKind,
) -> ExactDomainRule {
    ExactDomainRule {
        domain,
        rule: TaxonomyRule { id: domain, domain_category, page_category, interaction_kind },
    }
}

/// The taxonomy a build classifies visits with: the matcher's version and every pack's. A build
/// records it, so a changed pack makes the next build classify every visit again, and opening the
/// archive queues that build.
pub(crate) fn taxonomy_version() -> &'static str {
    static VERSION: LazyLock<String> = LazyLock::new(|| {
        RULE_PACKS.iter().fold(TAXONOMY_VERSION.to_string(), |version, pack| {
            format!("{version}|{}:{}", pack.id, pack.version)
        })
    });
    &VERSION
}

const GLOBAL_CORE_HOST_PATH_RULES: &[HostPathRule] = &[
    HostPathRule {
        rule: TaxonomyRule {
            id: "github-issue",
            domain_category: DomainCategory::Developer,
            page_category: PageCategory::Issue,
            interaction_kind: InteractionKind::Resolve,
        },
        host_suffixes: &["github.com", "gitlab.com"],
        path_prefixes: &[],
        path_contains: &["/issues/"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "github-pull-request",
            domain_category: DomainCategory::Developer,
            page_category: PageCategory::PullRequest,
            interaction_kind: InteractionKind::Resolve,
        },
        host_suffixes: &["github.com", "gitlab.com"],
        path_prefixes: &[],
        path_contains: &["/pull/", "/merge_requests/"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "github-repo",
            domain_category: DomainCategory::Developer,
            page_category: PageCategory::Repo,
            interaction_kind: InteractionKind::Resolve,
        },
        host_suffixes: &["github.com", "gitlab.com"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 2,
    },
    // Every other page of these sites.
    HostPathRule {
        rule: TaxonomyRule {
            id: "github-site",
            domain_category: DomainCategory::Developer,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: &["github.com", "gitlab.com"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "docs-path",
            domain_category: DomainCategory::Docs,
            page_category: PageCategory::DocsPage,
            interaction_kind: InteractionKind::Learn,
        },
        host_suffixes: &[],
        path_prefixes: &["/docs", "/doc", "/reference", "/api", "/guide", "/guides"],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "community-thread",
            domain_category: DomainCategory::Community,
            page_category: PageCategory::ForumThread,
            interaction_kind: InteractionKind::Discuss,
        },
        host_suffixes: &["reddit.com", "stackoverflow.com", "news.ycombinator.com"],
        path_prefixes: &[],
        path_contains: &["/questions/", "/comments/", "/forum", "/discuss"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "community-site",
            domain_category: DomainCategory::Community,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: &["reddit.com", "stackoverflow.com", "news.ycombinator.com"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "video-watch",
            domain_category: DomainCategory::Video,
            page_category: PageCategory::VideoPage,
            interaction_kind: InteractionKind::Watch,
        },
        host_suffixes: &["youtube.com", "youtu.be", "vimeo.com"],
        path_prefixes: &["/watch"],
        path_contains: &[],
        query_keys: &["v"],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "video-site",
            domain_category: DomainCategory::Video,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: &["youtube.com", "youtu.be", "vimeo.com"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
];

const GLOBAL_CORE_LEXICON_RULES: &[LexiconRule] = &[
    LexiconRule {
        rule: TaxonomyRule {
            id: "docs-lexicon",
            domain_category: DomainCategory::Unknown,
            page_category: PageCategory::DocsPage,
            interaction_kind: InteractionKind::Learn,
        },
        tokens: &[
            "docs",
            "documentation",
            "reference",
            "guide",
            "guides",
            "api",
            "文档",
            "文檔",
            "教程",
            "教學",
        ],
    },
    LexiconRule {
        rule: TaxonomyRule {
            id: "compare-lexicon",
            domain_category: DomainCategory::Unknown,
            page_category: PageCategory::CategoryPage,
            interaction_kind: InteractionKind::Compare,
        },
        tokens: &[
            "compare",
            "comparison",
            "versus",
            "vs",
            "pricing",
            "plans",
            "价格",
            "價格",
            "比较",
            "比較",
            "对比",
            "對比",
        ],
    },
    LexiconRule {
        rule: TaxonomyRule {
            id: "forum-lexicon",
            domain_category: DomainCategory::Unknown,
            page_category: PageCategory::ForumThread,
            interaction_kind: InteractionKind::Discuss,
        },
        tokens: &["forum", "discussion", "thread", "问答", "問答", "讨论", "討論"],
    },
    LexiconRule {
        rule: TaxonomyRule {
            id: "article-lexicon",
            domain_category: DomainCategory::Unknown,
            page_category: PageCategory::ArticlePage,
            interaction_kind: InteractionKind::Learn,
        },
        tokens: &["article", "blog", "newsletter", "news", "analysis"],
    },
];

const CN_CORE_HOST_PATH_RULES: &[HostPathRule] = &[
    HostPathRule {
        rule: TaxonomyRule {
            id: "taobao-product",
            domain_category: DomainCategory::Shopping,
            page_category: PageCategory::ProductPage,
            interaction_kind: InteractionKind::Compare,
        },
        host_suffixes: &["taobao.com", "tmall.com", "jd.com"],
        path_prefixes: &["/item", "/product", "/dp"],
        path_contains: &["/item", "/product"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "cn-shop-site",
            domain_category: DomainCategory::Shopping,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: &["taobao.com", "tmall.com", "jd.com"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "juejin-article",
            domain_category: DomainCategory::Developer,
            page_category: PageCategory::ArticlePage,
            interaction_kind: InteractionKind::Learn,
        },
        host_suffixes: &["juejin.cn", "csdn.net"],
        path_prefixes: &["/post", "/article"],
        path_contains: &["/article/"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "cn-developer-site",
            domain_category: DomainCategory::Developer,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: &["juejin.cn", "csdn.net"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
];

const CN_CORE_LEXICON_RULES: &[LexiconRule] = &[LexiconRule {
    rule: TaxonomyRule {
        id: "cn-docs-lexicon",
        domain_category: DomainCategory::Unknown,
        page_category: PageCategory::DocsPage,
        interaction_kind: InteractionKind::Learn,
    },
    tokens: &["文档", "文檔", "教程", "指南", "开发", "開發"],
}];

/// Amazon's shops: product and cart pages are found by the same paths on each.
const AMAZON: &[&str] = &[
    "amazon.com",
    "amazon.ca",
    "amazon.cn",
    "amazon.co.jp",
    "amazon.co.uk",
    "amazon.com.au",
    "amazon.de",
    "amazon.es",
    "amazon.fr",
    "amazon.in",
    "amazon.it",
];

const US_CORE_HOST_PATH_RULES: &[HostPathRule] = &[
    HostPathRule {
        rule: TaxonomyRule {
            id: "amazon-product",
            domain_category: DomainCategory::Shopping,
            page_category: PageCategory::ProductPage,
            interaction_kind: InteractionKind::Compare,
        },
        host_suffixes: AMAZON,
        path_prefixes: &["/dp", "/gp/product"],
        path_contains: &["/dp/", "/gp/product/"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "amazon-cart",
            domain_category: DomainCategory::Shopping,
            page_category: PageCategory::Dashboard,
            interaction_kind: InteractionKind::Transact,
        },
        host_suffixes: AMAZON,
        path_prefixes: &["/cart", "/checkout"],
        path_contains: &["/cart", "/checkout"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "amazon-site",
            domain_category: DomainCategory::Shopping,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: AMAZON,
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "us-news-article",
            domain_category: DomainCategory::News,
            page_category: PageCategory::ArticlePage,
            interaction_kind: InteractionKind::Learn,
        },
        host_suffixes: &["nytimes.com", "wsj.com", "theverge.com", "cnn.com"],
        path_prefixes: &[],
        path_contains: &["/article/", "/202", "/news/"],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
    HostPathRule {
        rule: TaxonomyRule {
            id: "us-news-site",
            domain_category: DomainCategory::News,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
        },
        host_suffixes: &["nytimes.com", "wsj.com", "theverge.com", "cnn.com"],
        path_prefixes: &[],
        path_contains: &[],
        query_keys: &[],
        query_value_contains: &[],
        path_segment_count_at_least: 0,
    },
];

const US_CORE_LEXICON_RULES: &[LexiconRule] = &[LexiconRule {
    rule: TaxonomyRule {
        id: "us-shopping-lexicon",
        domain_category: DomainCategory::Unknown,
        page_category: PageCategory::CategoryPage,
        interaction_kind: InteractionKind::Compare,
    },
    tokens: &["pricing", "plan", "plans", "checkout", "coupon", "deal"],
}];

const EMPTY_HOST_PATH_RULES: &[HostPathRule] = &[];
const EMPTY_LEXICON_RULES: &[LexiconRule] = &[];

pub(super) const RULE_PACKS: &[TaxonomyRulePack] = &[
    TaxonomyRulePack {
        id: "global-core",
        version: "2026-10-08",
        exact_domains: sites::GLOBAL_CORE,
        host_path_rules: GLOBAL_CORE_HOST_PATH_RULES,
        lexicon_rules: GLOBAL_CORE_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "cn-core",
        version: "2026-10-08",
        exact_domains: sites::CN_CORE,
        host_path_rules: CN_CORE_HOST_PATH_RULES,
        lexicon_rules: CN_CORE_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "us-core",
        version: "2026-10-08",
        exact_domains: sites::US_CORE,
        host_path_rules: US_CORE_HOST_PATH_RULES,
        lexicon_rules: US_CORE_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "tw-core",
        version: "2026-10-08",
        exact_domains: sites::TW_CORE,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "jp-core",
        version: "2026-10-08",
        exact_domains: sites::JP_CORE,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "kr-core",
        version: "2026-10-08",
        exact_domains: sites::KR_CORE,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "eu-core",
        version: "2026-10-08",
        exact_domains: sites::EU_CORE,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "ru-core",
        version: "2026-10-08",
        exact_domains: sites::RU_CORE,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "international-common",
        version: "2026-10-08",
        exact_domains: sites::INTERNATIONAL_COMMON,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
];
