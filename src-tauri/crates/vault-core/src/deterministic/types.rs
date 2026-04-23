//! Public deterministic analysis types.
//!
//! ## Responsibilities
//! - Define the stable structs and enums exported by `crate::deterministic`.
//! - Keep serialized category identifiers next to their canonical enum values.
//! - Preserve the taxonomy version contract used by persisted derived rows.
//!
//! ## Not responsible for
//! - URL parsing, tokenization, or rule matching.
//! - Loading user overrides or writing derived-state rows.
//! - Frontend DTO shaping beyond stable string identifiers.
//!
//! ## Dependencies
//! - Standard library strings and slices only.
//!
//! ## Performance notes
//! - These are compact value types; large-archive memory behavior is governed
//!   by the caller's batching strategy, not by this type module.

pub(super) const TAXONOMY_VERSION: &str = "m5-taxonomy-v1";

/// Returns the current deterministic taxonomy baseline version.
pub(crate) const fn taxonomy_version() -> &'static str {
    TAXONOMY_VERSION
}

/// URL normalized for deterministic analysis and taxonomy matching.
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

/// Strength of deterministic evidence supporting a derived interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvidenceTier {
    TierA,
    TierB,
    #[default]
    TierC,
}

/// Provides the persisted string identifiers for evidence tiers.
impl EvidenceTier {
    /// Returns the serialized tier identifier used in read models.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TierA => "tier-a",
            Self::TierB => "tier-b",
            Self::TierC => "tier-c",
        }
    }
}

/// Explanation of why one visit received a particular evidence tier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VisitEvidenceAssessment {
    pub tier: EvidenceTier,
    pub reasons: Vec<String>,
}

/// High-level domain taxonomy used by deterministic insights.
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

/// Provides the persisted string identifiers for domain categories.
impl DomainCategory {
    /// Returns the serialized domain-category identifier used in read models.
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

/// High-level page taxonomy used by deterministic insights.
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

/// Provides the persisted string identifiers for page categories.
impl PageCategory {
    /// Returns the serialized page-category identifier used in read models.
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

/// High-level interaction taxonomy used by deterministic insights.
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

/// Provides the persisted string identifiers for interaction kinds.
impl InteractionKind {
    /// Returns the serialized interaction-kind identifier used in read models.
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

/// Source of the final deterministic taxonomy decision.
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

/// Provides the persisted string identifiers for taxonomy decision sources.
impl TaxonomyDecisionSource {
    /// Returns the serialized taxonomy-source identifier used in read models.
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

/// Final deterministic taxonomy classification for one visit.
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

/// Provides the explicit unknown fallback used when no deterministic rule applies.
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

/// Target type for one deterministic taxonomy override rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaxonomyOverrideTarget {
    ExactDomain,
    Host,
    UrlPrefix,
}

/// User-provided taxonomy override used ahead of built-in rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyOverride {
    pub target: TaxonomyOverrideTarget,
    pub value: String,
    pub domain_category: DomainCategory,
    pub page_category: PageCategory,
    pub interaction_kind: InteractionKind,
    pub note: Option<String>,
}

/// Raw visit fields needed for deterministic analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisitAnalysisInput<'a> {
    pub url: &'a str,
    pub title: Option<&'a str>,
    pub query: Option<&'a str>,
    pub has_canonical_search_term: bool,
    pub external_referrer_url: Option<&'a str>,
    pub from_visit: Option<i64>,
}

/// Full deterministic analysis output for one visit.
#[derive(Debug, Clone, PartialEq)]
pub struct DeterministicVisitAnalysis {
    pub normalized_url: Option<NormalizedVisitUrl>,
    pub evidence: VisitEvidenceAssessment,
    pub taxonomy: TaxonomyClassification,
}
