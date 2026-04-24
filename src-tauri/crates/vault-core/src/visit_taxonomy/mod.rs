//! Rule-based visit taxonomy and URL analysis.
//!
//! ## Responsibilities
//! - Expose the stable visit-taxonomy API consumed by Core Intelligence and
//!   site-dictionary rebuild code.
//! - Keep URL normalization, text tokenization, taxonomy rule packs, and visit
//!   classification in focused owner modules.
//! - Preserve the non-LLM taxonomy contract used by derived-state rebuilds.
//!
//! ## Not responsible for
//! - Persisting Core Intelligence rows or running rebuild stages.
//! - Loading user-defined site dictionary overrides from SQLite.
//! - Shaping frontend or Tauri transport payloads.
//!
//! ## Dependencies
//! - `reqwest::Url` and `publicsuffix` for bounded URL/domain parsing.
//! - Internal taxonomy rule packs frozen by the Core Intelligence contract.
//!
//! ## Performance notes
//! - All analysis is per-visit and allocation-bounded; callers remain
//!   responsible for streaming or batching large archives.

mod classification;
mod rules;
#[cfg(test)]
mod tests;
mod text;
mod types;
mod url;

pub use self::classification::analyze_visit;
pub use self::text::tokenize_text;
pub use self::types::{
    DomainCategory, EvidenceTier, InteractionKind, NormalizedVisitUrl, PageCategory,
    TaxonomyClassification, TaxonomyDecisionSource, TaxonomyOverride, TaxonomyOverrideTarget,
    VisitAnalysisInput, VisitEvidenceAssessment, VisitTaxonomyAnalysis,
};
pub use self::url::{
    extract_search_query_from_url, normalize_visit_url, registrable_domain_for_host,
    registrable_domain_for_url,
};

pub(crate) use self::types::taxonomy_version;
