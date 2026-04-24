//! Data contracts shared by site-dictionary classification and persistence.
//!
//! ## Responsibilities
//! - Define the in-memory visit classification row consumed by rebuild code.
//! - Define user override and search-rule configuration shapes used across
//!   sibling owner modules.
//! - Keep stringly database target kinds behind one parser/formatter boundary.
//!
//! ## Not responsible for
//! - Reading or writing SQLite tables.
//! - Matching URLs against search rules.
//! - Translating taxonomy enum values into persisted category keys.
//!
//! ## Dependencies
//! - `visit_taxonomy::TaxonomyOverride` for user-controlled taxonomy patches.
//!
//! ## Performance notes
//! - These structs are cloned per loaded batch, so fields remain compact owned
//!   strings instead of carrying connection- or row-lifetime references.

use crate::visit_taxonomy::TaxonomyOverride;

/// Normalized visit facts that higher-level Core Intelligence stages can
/// consume without repeating URL parsing, search extraction, or taxonomy
/// lookups.
#[derive(Debug, Clone, Default)]
pub(crate) struct SiteDictionaryEntry {
    pub canonical_url: String,
    pub registrable_domain: String,
    pub domain_category: String,
    pub page_category: String,
    pub search_engine: Option<String>,
    pub search_query: Option<String>,
    pub evidence_tier: String,
    pub taxonomy_source: String,
    pub taxonomy_pack: Option<String>,
    pub taxonomy_version: Option<String>,
    pub display_name: Option<String>,
    #[allow(dead_code)]
    pub is_noisy: bool,
}

/// Scope used by a user override. Keeping this as an enum prevents persisted
/// target strings from leaking throughout the classifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SiteDictionaryOverrideTargetKind {
    ExactDomain,
    Host,
    UrlPrefix,
}

/// Provides the string contract for the SQLite table while keeping invalid
/// values rejected at load time.
impl SiteDictionaryOverrideTargetKind {
    /// Converts a trusted target kind into the persisted key used by
    /// `site_dictionary_overrides`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::ExactDomain => "exact-domain",
            Self::Host => "host",
            Self::UrlPrefix => "url-prefix",
        }
    }

    /// Parses one persisted target key. Unknown values return `None` so the
    /// loader can fail loudly instead of silently ignoring an unsafe override.
    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value {
            "exact-domain" => Some(Self::ExactDomain),
            "host" => Some(Self::Host),
            "url-prefix" => Some(Self::UrlPrefix),
            _ => None,
        }
    }
}

/// User override merged into visit classification. An override may patch
/// taxonomy, display label, search engine identity, or noisy-site status.
#[derive(Debug, Clone)]
pub(crate) struct SiteDictionaryOverride {
    pub target_kind: SiteDictionaryOverrideTargetKind,
    pub target_value: String,
    pub taxonomy_override: Option<TaxonomyOverride>,
    pub display_name: Option<String>,
    pub search_engine: Option<String>,
    pub is_noisy: bool,
    #[allow(dead_code)]
    pub note: Option<String>,
}

/// Keeps URL matching and taxonomy extraction close to the override type so the
/// classifier does not need to know the storage representation.
impl SiteDictionaryOverride {
    /// Checks whether this override applies to the already-normalized URL
    /// context for one visit.
    pub(super) fn matches(&self, url: &str, registrable_domain: &str, host: &str) -> bool {
        match self.target_kind {
            SiteDictionaryOverrideTargetKind::ExactDomain => {
                registrable_domain == self.target_value
            }
            SiteDictionaryOverrideTargetKind::Host => host == self.target_value,
            SiteDictionaryOverrideTargetKind::UrlPrefix => url.starts_with(&self.target_value),
        }
    }

    /// Returns the taxonomy patch for `visit_taxonomy::analyze_visit`. Cloning
    /// is acceptable because overrides are small and loaded once per batch.
    pub(super) fn taxonomy_override(&self) -> Option<TaxonomyOverride> {
        let taxonomy_override = self.taxonomy_override.as_ref()?;
        Some(taxonomy_override.clone())
    }
}

/// Settings-side payload used by tests and future rule editors when persisting
/// one site-dictionary override.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub(crate) struct SiteDictionaryOverrideUpsert {
    pub target_kind: SiteDictionaryOverrideTargetKind,
    pub target_value: String,
    pub domain_category: Option<String>,
    pub page_category: Option<String>,
    pub interaction_kind: Option<String>,
    pub display_name: Option<String>,
    pub search_engine: Option<String>,
    pub is_noisy: bool,
    pub note: Option<String>,
}

/// Internal merged representation of built-in and custom search-engine rules.
/// Field access stays within the owner module so external callers cannot depend
/// on storage details.
#[derive(Debug, Clone)]
pub(crate) struct SearchEngineRuleConfig {
    pub(super) rule_id: String,
    pub(super) engine_id: String,
    pub(super) display_name: String,
    pub(super) host_pattern: String,
    pub(super) path_prefix: Option<String>,
    pub(super) query_param_key: String,
    pub(super) enabled: bool,
    pub(super) note: Option<String>,
    pub(super) example_url: Option<String>,
    pub(super) built_in: bool,
}

/// Result of a URL query-param match against one enabled search-engine rule.
/// Keeping it separate from the persisted rule avoids mutating rule config
/// while normalizing a single visit.
#[derive(Debug, Clone)]
pub(super) struct SearchQueryMatch {
    pub(super) engine_id: String,
    pub(super) display_name: String,
    pub(super) query: String,
}
