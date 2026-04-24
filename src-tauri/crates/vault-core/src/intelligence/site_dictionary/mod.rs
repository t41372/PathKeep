//! Shared site-dictionary owner for Core Intelligence visit classification.
//!
//! ## Responsibilities
//! - Expose the stable site-dictionary façade consumed by rebuild stages and
//!   Settings search-rule commands.
//! - Keep URL/domain classification, user overrides, and search-engine rules
//!   in separate owner modules.
//! - Preserve one merged view of built-in and user-defined search behavior so
//!   rebuilds and Settings never drift.
//!
//! ## Not responsible for
//! - Defining the base taxonomy rule packs; that belongs to `visit_taxonomy`.
//! - Running Core Intelligence rebuild stages or route-level read models.
//! - Owning the frontend Settings grammar for search-rule editing.
//!
//! ## Dependencies
//! - `visit_taxonomy` for URL normalization and taxonomy evidence.
//! - The derived intelligence SQLite connection for user overrides and custom
//!   search-engine rules.
//! - `models::SearchEngineRule*` for Settings-facing payloads.
//!
//! ## Performance notes
//! - Classification is intentionally per-visit and allocation-light; callers
//!   cache override and rule vectors once per rebuild batch instead of querying
//!   SQLite per row.
//! - Rule/override schema loaders use indexed target columns because the data
//!   is small but read on every large Core Intelligence rebuild.

mod classification;
mod overrides;
mod search_rules;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use classification::{
    classify_visit, display_name_for_domain, display_name_for_search_engine, normalize_query,
};
#[cfg(test)]
pub(crate) use overrides::upsert_site_dictionary_override;
pub(crate) use overrides::{
    ensure_site_dictionary_override_schema, load_site_dictionary_overrides,
};
pub(crate) use search_rules::{
    delete_search_engine_rule, display_name_for_search_engine_with_map,
    ensure_search_engine_rule_schema, list_search_engine_rules, load_enabled_search_engine_rules,
    load_search_engine_display_names, upsert_search_engine_rule,
};
pub(crate) use types::SiteDictionaryEntry;
#[cfg(test)]
pub(crate) use types::{SiteDictionaryOverrideTargetKind, SiteDictionaryOverrideUpsert};
