use reqwest::Url;

const SEARCH_QUERY_KEYS: &[&str] =
    &["q", "query", "query_text", "search_query", "p", "wd", "word", "text", "keyword", "k"];
const MULTI_LABEL_PUBLIC_SUFFIXES: &[&str] = &[
    "co.jp", "co.kr", "co.uk", "com.au", "com.cn", "com.hk", "com.sg", "com.tr", "com.tw",
    "net.cn", "org.cn",
];
const LATIN_STOP_WORDS: &[&str] = &[
    "the", "and", "for", "that", "with", "from", "into", "this", "your", "what", "how", "why",
    "when", "where", "about", "http", "https", "www", "com", "org", "net", "html",
];
const TAXONOMY_VERSION: &str = "m5-taxonomy-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedVisitUrl {
    pub canonical_url: String,
    pub host: String,
    pub registrable_domain: String,
    pub subdomain: Option<String>,
    pub path: String,
    pub preserved_query: Vec<(String, String)>,
    pub dropped_tracking_params: Vec<String>,
    pub search_query: Option<String>,
    pub is_search_results: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvidenceTier {
    TierA,
    TierB,
    #[default]
    TierC,
}

impl EvidenceTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TierA => "tier-a",
            Self::TierB => "tier-b",
            Self::TierC => "tier-c",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VisitEvidenceAssessment {
    pub tier: EvidenceTier,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DomainCategory {
    Ai,
    Community,
    Developer,
    Docs,
    Education,
    Entertainment,
    Finance,
    News,
    Search,
    Shopping,
    Social,
    Travel,
    Video,
    Work,
    #[default]
    Unknown,
}

impl DomainCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ai => "ai",
            Self::Community => "community",
            Self::Developer => "developer",
            Self::Docs => "docs",
            Self::Education => "education",
            Self::Entertainment => "entertainment",
            Self::Finance => "finance",
            Self::News => "news",
            Self::Search => "search",
            Self::Shopping => "shopping",
            Self::Social => "social",
            Self::Travel => "travel",
            Self::Video => "video",
            Self::Work => "work",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageCategory {
    ArticlePage,
    CategoryPage,
    Dashboard,
    DocsPage,
    ForumThread,
    Home,
    Issue,
    ProductPage,
    Profile,
    PullRequest,
    Repo,
    SearchResults,
    VideoPage,
    #[default]
    Unknown,
}

impl PageCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArticlePage => "article-page",
            Self::CategoryPage => "category-page",
            Self::Dashboard => "dashboard",
            Self::DocsPage => "docs-page",
            Self::ForumThread => "forum-thread",
            Self::Home => "home",
            Self::Issue => "issue",
            Self::ProductPage => "product-page",
            Self::Profile => "profile",
            Self::PullRequest => "pull-request",
            Self::Repo => "repo",
            Self::SearchResults => "search-results",
            Self::VideoPage => "video-page",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractionKind {
    Compare,
    Discover,
    Discuss,
    Learn,
    Manage,
    Resolve,
    Transact,
    Watch,
    #[default]
    Unknown,
}

impl InteractionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compare => "compare",
            Self::Discover => "discover",
            Self::Discuss => "discuss",
            Self::Learn => "learn",
            Self::Manage => "manage",
            Self::Resolve => "resolve",
            Self::Transact => "transact",
            Self::Watch => "watch",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaxonomyDecisionSource {
    UserOverride,
    ExactDomainRule,
    HostPathRule,
    LexiconRule,
    OptionalModelFallback,
    #[default]
    Unknown,
}

impl TaxonomyDecisionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserOverride => "user-override",
            Self::ExactDomainRule => "exact-domain-rule",
            Self::HostPathRule => "host-path-query-rule",
            Self::LexiconRule => "title-query-lexicon-rule",
            Self::OptionalModelFallback => "optional-model-fallback",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaxonomyClassification {
    pub domain_category: DomainCategory,
    pub page_category: PageCategory,
    pub interaction_kind: InteractionKind,
    pub source: TaxonomyDecisionSource,
    pub confidence: f32,
    pub rule_pack: Option<String>,
    pub rule_id: Option<String>,
    pub version: String,
    pub reasons: Vec<String>,
}

impl Default for TaxonomyClassification {
    fn default() -> Self {
        Self {
            domain_category: DomainCategory::Unknown,
            page_category: PageCategory::Unknown,
            interaction_kind: InteractionKind::Unknown,
            source: TaxonomyDecisionSource::Unknown,
            confidence: 0.0,
            rule_pack: None,
            rule_id: None,
            version: TAXONOMY_VERSION.to_string(),
            reasons: vec!["unknown-fallback".to_string()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaxonomyOverrideTarget {
    ExactDomain,
    Host,
    UrlPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyOverride {
    pub target: TaxonomyOverrideTarget,
    pub value: String,
    pub domain_category: DomainCategory,
    pub page_category: PageCategory,
    pub interaction_kind: InteractionKind,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisitAnalysisInput<'a> {
    pub url: &'a str,
    pub title: Option<&'a str>,
    pub query: Option<&'a str>,
    pub has_canonical_search_term: bool,
    pub external_referrer_url: Option<&'a str>,
    pub from_visit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeterministicVisitAnalysis {
    pub normalized_url: Option<NormalizedVisitUrl>,
    pub evidence: VisitEvidenceAssessment,
    pub taxonomy: TaxonomyClassification,
}

#[derive(Debug, Clone, Copy)]
struct TaxonomyRule {
    id: &'static str,
    domain_category: DomainCategory,
    page_category: PageCategory,
    interaction_kind: InteractionKind,
}

#[derive(Debug, Clone, Copy)]
struct ExactDomainRule {
    domain: &'static str,
    rule: TaxonomyRule,
}

#[derive(Debug, Clone, Copy)]
struct HostPathRule {
    rule: TaxonomyRule,
    host_suffixes: &'static [&'static str],
    path_prefixes: &'static [&'static str],
    path_contains: &'static [&'static str],
    query_keys: &'static [&'static str],
    query_value_contains: &'static [&'static str],
    path_segment_count_at_least: usize,
}

#[derive(Debug, Clone, Copy)]
struct LexiconRule {
    rule: TaxonomyRule,
    tokens: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
struct TaxonomyRulePack {
    id: &'static str,
    version: &'static str,
    exact_domains: &'static [ExactDomainRule],
    host_path_rules: &'static [HostPathRule],
    lexicon_rules: &'static [LexiconRule],
}

const GLOBAL_CORE_EXACT_RULES: &[ExactDomainRule] = &[
    ExactDomainRule {
        domain: "baidu.com",
        rule: TaxonomyRule {
            id: "baidu-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "bing.com",
        rule: TaxonomyRule {
            id: "bing-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "brave.com",
        rule: TaxonomyRule {
            id: "brave-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "duckduckgo.com",
        rule: TaxonomyRule {
            id: "duckduckgo-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "google.com",
        rule: TaxonomyRule {
            id: "google-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "sogou.com",
        rule: TaxonomyRule {
            id: "sogou-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "so.com",
        rule: TaxonomyRule {
            id: "so-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "yahoo.com",
        rule: TaxonomyRule {
            id: "yahoo-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
    ExactDomainRule {
        domain: "yandex.ru",
        rule: TaxonomyRule {
            id: "yandex-search",
            domain_category: DomainCategory::Search,
            page_category: PageCategory::SearchResults,
            interaction_kind: InteractionKind::Discover,
        },
    },
];

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
    HostPathRule {
        rule: TaxonomyRule {
            id: "developer-docs",
            domain_category: DomainCategory::Docs,
            page_category: PageCategory::DocsPage,
            interaction_kind: InteractionKind::Learn,
        },
        host_suffixes: &["developer.mozilla.org", "devdocs.io"],
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
            id: "ai-chat",
            domain_category: DomainCategory::Ai,
            page_category: PageCategory::Dashboard,
            interaction_kind: InteractionKind::Manage,
        },
        host_suffixes: &["chat.openai.com", "claude.ai", "gemini.google.com", "chat.deepseek.com"],
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
            domain_category: DomainCategory::Docs,
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
            domain_category: DomainCategory::Shopping,
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
            domain_category: DomainCategory::Community,
            page_category: PageCategory::ForumThread,
            interaction_kind: InteractionKind::Discuss,
        },
        tokens: &["forum", "discussion", "thread", "问答", "問答", "讨论", "討論"],
    },
    LexiconRule {
        rule: TaxonomyRule {
            id: "article-lexicon",
            domain_category: DomainCategory::News,
            page_category: PageCategory::ArticlePage,
            interaction_kind: InteractionKind::Learn,
        },
        tokens: &["article", "blog", "newsletter", "news", "analysis"],
    },
];

const CN_CORE_EXACT_RULES: &[ExactDomainRule] = &[
    ExactDomainRule {
        domain: "bilibili.com",
        rule: TaxonomyRule {
            id: "bilibili-video",
            domain_category: DomainCategory::Video,
            page_category: PageCategory::VideoPage,
            interaction_kind: InteractionKind::Watch,
        },
    },
    ExactDomainRule {
        domain: "zhihu.com",
        rule: TaxonomyRule {
            id: "zhihu-community",
            domain_category: DomainCategory::Community,
            page_category: PageCategory::ForumThread,
            interaction_kind: InteractionKind::Discuss,
        },
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
];

const CN_CORE_LEXICON_RULES: &[LexiconRule] = &[LexiconRule {
    rule: TaxonomyRule {
        id: "cn-docs-lexicon",
        domain_category: DomainCategory::Docs,
        page_category: PageCategory::DocsPage,
        interaction_kind: InteractionKind::Learn,
    },
    tokens: &["文档", "文檔", "教程", "指南", "开发", "開發"],
}];

const US_CORE_EXACT_RULES: &[ExactDomainRule] = &[
    ExactDomainRule {
        domain: "linkedin.com",
        rule: TaxonomyRule {
            id: "linkedin-social",
            domain_category: DomainCategory::Social,
            page_category: PageCategory::Profile,
            interaction_kind: InteractionKind::Manage,
        },
    },
    ExactDomainRule {
        domain: "x.com",
        rule: TaxonomyRule {
            id: "x-social",
            domain_category: DomainCategory::Social,
            page_category: PageCategory::Profile,
            interaction_kind: InteractionKind::Discuss,
        },
    },
];

const US_CORE_HOST_PATH_RULES: &[HostPathRule] = &[
    HostPathRule {
        rule: TaxonomyRule {
            id: "amazon-product",
            domain_category: DomainCategory::Shopping,
            page_category: PageCategory::ProductPage,
            interaction_kind: InteractionKind::Compare,
        },
        host_suffixes: &["amazon.com"],
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
        host_suffixes: &["amazon.com"],
        path_prefixes: &["/cart", "/checkout"],
        path_contains: &["/cart", "/checkout"],
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
];

const US_CORE_LEXICON_RULES: &[LexiconRule] = &[LexiconRule {
    rule: TaxonomyRule {
        id: "us-shopping-lexicon",
        domain_category: DomainCategory::Shopping,
        page_category: PageCategory::CategoryPage,
        interaction_kind: InteractionKind::Compare,
    },
    tokens: &["pricing", "plan", "plans", "checkout", "coupon", "deal"],
}];

const EMPTY_EXACT_RULES: &[ExactDomainRule] = &[];
const EMPTY_HOST_PATH_RULES: &[HostPathRule] = &[];
const EMPTY_LEXICON_RULES: &[LexiconRule] = &[];

const RULE_PACKS: &[TaxonomyRulePack] = &[
    TaxonomyRulePack {
        id: "global-core",
        version: "2026-04-10",
        exact_domains: GLOBAL_CORE_EXACT_RULES,
        host_path_rules: GLOBAL_CORE_HOST_PATH_RULES,
        lexicon_rules: GLOBAL_CORE_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "cn-core",
        version: "2026-04-10",
        exact_domains: CN_CORE_EXACT_RULES,
        host_path_rules: CN_CORE_HOST_PATH_RULES,
        lexicon_rules: CN_CORE_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "us-core",
        version: "2026-04-10",
        exact_domains: US_CORE_EXACT_RULES,
        host_path_rules: US_CORE_HOST_PATH_RULES,
        lexicon_rules: US_CORE_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "tw-core",
        version: "2026-04-10",
        exact_domains: EMPTY_EXACT_RULES,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "jp-core",
        version: "2026-04-10",
        exact_domains: EMPTY_EXACT_RULES,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "kr-core",
        version: "2026-04-10",
        exact_domains: EMPTY_EXACT_RULES,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "eu-core",
        version: "2026-04-10",
        exact_domains: EMPTY_EXACT_RULES,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "ru-core",
        version: "2026-04-10",
        exact_domains: EMPTY_EXACT_RULES,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
    TaxonomyRulePack {
        id: "international-common",
        version: "2026-04-10",
        exact_domains: EMPTY_EXACT_RULES,
        host_path_rules: EMPTY_HOST_PATH_RULES,
        lexicon_rules: EMPTY_LEXICON_RULES,
    },
];

pub fn normalize_visit_url(raw_url: &str) -> Option<NormalizedVisitUrl> {
    let mut parsed = Url::parse(raw_url).ok()?;
    let host = parsed.host_str()?.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    let mut preserved_query = Vec::new();
    let mut dropped_tracking_params = Vec::new();
    let mut search_query = None;
    for (key, value) in parsed.query_pairs() {
        let key = key.to_string();
        let value = normalize_whitespace(&value.replace('+', " "));
        if is_tracking_param(&key) {
            dropped_tracking_params.push(key);
            continue;
        }
        if search_query.is_none()
            && is_search_engine_host(&host)
            && SEARCH_QUERY_KEYS.iter().any(|candidate| candidate.eq_ignore_ascii_case(&key))
            && !value.is_empty()
        {
            search_query = Some(value.clone());
        }
        preserved_query.push((key, value));
    }

    parsed.set_query(None);
    if !preserved_query.is_empty() {
        let mut serializer = parsed.query_pairs_mut();
        for (key, value) in &preserved_query {
            serializer.append_pair(key, value);
        }
    }

    let registrable_domain = registrable_domain_for_host(&host);
    let subdomain = subdomain_for_host_and_domain(&host, &registrable_domain);
    Some(NormalizedVisitUrl {
        canonical_url: parsed.to_string(),
        host: host.clone(),
        registrable_domain,
        subdomain,
        path: parsed.path().to_string(),
        preserved_query,
        dropped_tracking_params,
        search_query: search_query.clone(),
        is_search_results: search_query.is_some() && is_search_engine_host(&host),
    })
}

pub fn analyze_visit(
    input: VisitAnalysisInput<'_>,
    overrides: &[TaxonomyOverride],
) -> DeterministicVisitAnalysis {
    let normalized_url = normalize_visit_url(input.url);
    let query = input
        .query
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_url.as_ref().and_then(|value| value.search_query.clone()));
    let evidence = assess_visit_evidence(
        normalized_url.as_ref(),
        input.title,
        input.has_canonical_search_term,
        input.external_referrer_url,
        input.from_visit,
    );
    let taxonomy = classify_visit_taxonomy(
        normalized_url.as_ref(),
        input.url,
        input.title,
        query.as_deref(),
        overrides,
    );
    DeterministicVisitAnalysis { normalized_url, evidence, taxonomy }
}

pub fn tokenize_text(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut cjk_run = String::new();

    for ch in input.chars() {
        if is_cjk_like(ch) {
            flush_word(&mut word, &mut tokens);
            cjk_run.push(ch);
            continue;
        }
        if !cjk_run.is_empty() {
            flush_cjk_run(&mut cjk_run, &mut tokens);
        }
        if ch.is_alphanumeric() {
            word.extend(ch.to_lowercase());
        } else {
            flush_word(&mut word, &mut tokens);
        }
    }

    flush_word(&mut word, &mut tokens);
    flush_cjk_run(&mut cjk_run, &mut tokens);
    tokens
}

pub fn extract_search_query_from_url(url: &str) -> Option<String> {
    normalize_visit_url(url).and_then(|value| value.search_query)
}

pub fn registrable_domain_for_url(url: &str) -> Option<String> {
    normalize_visit_url(url).map(|value| value.registrable_domain)
}

pub fn registrable_domain_for_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    let segments = host.split('.').collect::<Vec<_>>();
    if segments.len() <= 2 {
        return host;
    }

    let suffix = format!("{}.{}", segments[segments.len() - 2], segments[segments.len() - 1]);
    if MULTI_LABEL_PUBLIC_SUFFIXES.iter().any(|candidate| candidate.eq_ignore_ascii_case(&suffix))
        && segments.len() >= 3
    {
        return segments[segments.len() - 3..].join(".");
    }

    segments[segments.len() - 2..].join(".")
}

fn assess_visit_evidence(
    normalized_url: Option<&NormalizedVisitUrl>,
    title: Option<&str>,
    has_canonical_search_term: bool,
    external_referrer_url: Option<&str>,
    from_visit: Option<i64>,
) -> VisitEvidenceAssessment {
    let mut tier_a_reasons = Vec::new();
    if has_canonical_search_term {
        tier_a_reasons.push("canonical-search-term".to_string());
    }
    if normalized_url.is_some_and(|value| value.is_search_results && value.search_query.is_some()) {
        tier_a_reasons.push("search-result-url".to_string());
    }
    if external_referrer_url.is_some() || from_visit.is_some() {
        tier_a_reasons.push("navigation-anchor".to_string());
    }
    if !tier_a_reasons.is_empty() {
        return VisitEvidenceAssessment { tier: EvidenceTier::TierA, reasons: tier_a_reasons };
    }

    let mut tier_b_reasons = Vec::new();
    if normalized_url.is_some_and(|value| value.path != "/") {
        tier_b_reasons.push("normalized-path".to_string());
    }
    if normalized_url.is_some_and(|value| !value.preserved_query.is_empty()) {
        tier_b_reasons.push("semantic-query-params".to_string());
    }
    if title.is_some_and(|value| !tokenize_text(value).is_empty()) {
        tier_b_reasons.push("title-tokens".to_string());
    }
    if !tier_b_reasons.is_empty() {
        return VisitEvidenceAssessment { tier: EvidenceTier::TierB, reasons: tier_b_reasons };
    }

    VisitEvidenceAssessment {
        tier: EvidenceTier::TierC,
        reasons: vec!["time-adjacency-only".to_string()],
    }
}

fn classify_visit_taxonomy(
    normalized_url: Option<&NormalizedVisitUrl>,
    raw_url: &str,
    title: Option<&str>,
    query: Option<&str>,
    overrides: &[TaxonomyOverride],
) -> TaxonomyClassification {
    if let Some(classification) = match_taxonomy_override(normalized_url, raw_url, overrides) {
        return classification;
    }

    let fallback_domain =
        normalized_url.map(|value| value.registrable_domain.as_str()).unwrap_or_default();
    let title_and_query = normalize_whitespace(
        &[
            title.unwrap_or_default(),
            query.unwrap_or_default(),
            normalized_url.and_then(|value| value.search_query.as_deref()).unwrap_or_default(),
        ]
        .join(" "),
    )
    .to_lowercase();

    for pack in RULE_PACKS {
        for rule in pack.exact_domains {
            if fallback_domain == rule.domain {
                return classification_from_rule(
                    pack,
                    rule.rule,
                    TaxonomyDecisionSource::ExactDomainRule,
                    0.95,
                    vec![format!("exact-domain={}", rule.domain)],
                );
            }
        }
    }

    if let Some(normalized_url) = normalized_url {
        for pack in RULE_PACKS {
            for rule in pack.host_path_rules {
                if matches_host_path_rule(normalized_url, rule) {
                    return classification_from_rule(
                        pack,
                        rule.rule,
                        TaxonomyDecisionSource::HostPathRule,
                        0.86,
                        vec![
                            format!("host={}", normalized_url.host),
                            format!("path={}", normalized_url.path),
                        ],
                    );
                }
            }
        }
    }

    for pack in RULE_PACKS {
        for rule in pack.lexicon_rules {
            if rule.tokens.iter().any(|token| title_and_query.contains(token)) {
                return classification_from_rule(
                    pack,
                    rule.rule,
                    TaxonomyDecisionSource::LexiconRule,
                    0.62,
                    vec![format!("lexicon={}", rule.tokens.join("|"))],
                );
            }
        }
    }

    TaxonomyClassification {
        reasons: if fallback_domain.is_empty() {
            vec!["unknown-fallback".to_string(), "invalid-or-empty-domain".to_string()]
        } else {
            vec![format!("unknown-fallback:{}", fallback_domain)]
        },
        ..TaxonomyClassification::default()
    }
}

fn classification_from_rule(
    pack: &TaxonomyRulePack,
    rule: TaxonomyRule,
    source: TaxonomyDecisionSource,
    confidence: f32,
    mut reasons: Vec<String>,
) -> TaxonomyClassification {
    reasons.push(format!("rule-pack={}", pack.id));
    reasons.push(format!("rule-id={}", rule.id));
    TaxonomyClassification {
        domain_category: rule.domain_category,
        page_category: rule.page_category,
        interaction_kind: rule.interaction_kind,
        source,
        confidence,
        rule_pack: Some(pack.id.to_string()),
        rule_id: Some(rule.id.to_string()),
        version: format!("{}:{}", TAXONOMY_VERSION, pack.version),
        reasons,
    }
}

fn match_taxonomy_override(
    normalized_url: Option<&NormalizedVisitUrl>,
    raw_url: &str,
    overrides: &[TaxonomyOverride],
) -> Option<TaxonomyClassification> {
    for override_rule in overrides {
        let matches = match override_rule.target {
            TaxonomyOverrideTarget::ExactDomain => {
                normalized_url.is_some_and(|value| value.registrable_domain == override_rule.value)
            }
            TaxonomyOverrideTarget::Host => {
                normalized_url.is_some_and(|value| value.host == override_rule.value)
            }
            TaxonomyOverrideTarget::UrlPrefix => raw_url.starts_with(&override_rule.value),
        };
        if matches {
            let mut reasons = vec![format!("override-target={:?}", override_rule.target)];
            if let Some(note) = &override_rule.note {
                reasons.push(format!("override-note={note}"));
            }
            return Some(TaxonomyClassification {
                domain_category: override_rule.domain_category,
                page_category: override_rule.page_category,
                interaction_kind: override_rule.interaction_kind,
                source: TaxonomyDecisionSource::UserOverride,
                confidence: 1.0,
                rule_pack: Some("user-override".to_string()),
                rule_id: Some(override_rule.value.clone()),
                version: TAXONOMY_VERSION.to_string(),
                reasons,
            });
        }
    }
    None
}

fn matches_host_path_rule(normalized_url: &NormalizedVisitUrl, rule: &HostPathRule) -> bool {
    if !rule.host_suffixes.is_empty()
        && !rule
            .host_suffixes
            .iter()
            .any(|suffix| host_matches_suffix(&normalized_url.host, suffix))
    {
        return false;
    }

    let path = normalized_url.path.to_ascii_lowercase();
    if !rule.path_prefixes.is_empty()
        && !rule.path_prefixes.iter().any(|prefix| path.starts_with(prefix))
    {
        return false;
    }
    if !rule.path_contains.is_empty()
        && !rule.path_contains.iter().any(|needle| path.contains(needle))
    {
        return false;
    }
    if rule.path_segment_count_at_least > 0
        && path_segments(&path) < rule.path_segment_count_at_least
    {
        return false;
    }
    if !rule.query_keys.is_empty()
        && !normalized_url.preserved_query.iter().any(|(key, _)| {
            rule.query_keys.iter().any(|candidate| candidate.eq_ignore_ascii_case(key))
        })
    {
        return false;
    }
    if !rule.query_value_contains.is_empty()
        && !normalized_url.preserved_query.iter().any(|(_, value)| {
            let value = value.to_ascii_lowercase();
            rule.query_value_contains.iter().any(|candidate| value.contains(candidate))
        })
    {
        return false;
    }
    true
}

fn path_segments(path: &str) -> usize {
    path.split('/').filter(|segment| !segment.is_empty()).count()
}

fn host_matches_suffix(host: &str, suffix: &str) -> bool {
    host == suffix || host.ends_with(&format!(".{suffix}"))
}

fn is_search_engine_host(host: &str) -> bool {
    let domain = registrable_domain_for_host(host);
    matches!(
        domain.as_str(),
        "baidu.com"
            | "bing.com"
            | "brave.com"
            | "duckduckgo.com"
            | "google.com"
            | "sogou.com"
            | "so.com"
            | "yahoo.com"
            | "yandex.ru"
    ) || host.starts_with("www.google.")
        || host == "search.brave.com"
}

fn is_tracking_param(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_")
        || matches!(
            key.as_str(),
            "fbclid"
                | "gclid"
                | "igshid"
                | "mc_cid"
                | "mc_eid"
                | "mkt_tok"
                | "ref"
                | "ref_src"
                | "si"
                | "spm"
                | "source"
                | "sourceid"
        )
}

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut saw_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !saw_space {
                output.push(' ');
                saw_space = true;
            }
        } else {
            output.push(ch);
            saw_space = false;
        }
    }
    output.trim().to_string()
}

fn subdomain_for_host_and_domain(host: &str, registrable_domain: &str) -> Option<String> {
    if host == registrable_domain {
        return None;
    }
    let suffix = format!(".{registrable_domain}");
    host.strip_suffix(&suffix).map(|value| value.to_string()).filter(|value| !value.is_empty())
}

fn flush_word(word: &mut String, tokens: &mut Vec<String>) {
    if word.is_empty() {
        return;
    }
    if word.len() > 1 && !LATIN_STOP_WORDS.contains(&word.as_str()) {
        tokens.push(word.clone());
    }
    word.clear();
}

fn flush_cjk_run(run: &mut String, tokens: &mut Vec<String>) {
    if run.is_empty() {
        return;
    }
    let chars = run.chars().collect::<Vec<_>>();
    if chars.len() == 1 {
        tokens.push(chars[0].to_string());
    } else {
        tokens.push(chars.iter().collect());
        for window in chars.windows(2) {
            tokens.push(window.iter().collect());
        }
    }
    run.clear();
}

fn is_cjk_like(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x30ff // Hiragana + Katakana
            | 0x3400..=0x4dbf // CJK Extension A
            | 0x4e00..=0x9fff // CJK Unified Ideographs
            | 0xac00..=0xd7af // Hangul syllables
            | 0xf900..=0xfaff // CJK compatibility ideographs
    )
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            normalized.canonical_url,
            "https://github.com/example/repo/issues/42?tab=comments"
        );
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
}
