//! Responsibilities: dispatch native timeline reads and Vault-owned search.
//! Not responsible for: storage ownership, indexing or UI transport.
//! Failure modes: search reaches timeline reads, legacy SQLCipher opens, missed cursors.
//! Tests: timeline_tests and search/ported_* suites.
//!
//! This module owns the Explorer-facing read surface over canonical visits. It
//! keeps the recall modes explicit:
//!
//! - regular SQL filtering
//! - all search dispatches to the Vault-owned search sessions
//!
//! Search page grouping and streaming live in `crate::search`.
//!
//! The accepted product contract is that only visible facts participate in
//! recall/export. Hidden/reverted rows stay out of these surfaces, and regex is
//! a slower manual path instead of pretending to be the default fast query
//! engine.

use super::*;

mod export;
mod search_export;
pub use search_export::export_search_history;
mod favicons;
// `net_guard` is the SSRF guard reused by W-ENRICH-1's content-fetch egress (06 §2b: every page URL
// AND every API sub-resource is checked). Promoted to `pub(crate)` so the enrichment plane can reach
// `url_target_is_blocked` through the same chokepoint og:image fetching uses.
pub(crate) mod net_guard;
pub mod og_images;
pub mod og_images_fetch;
pub mod og_images_synth;
mod pagination;
mod timeline;

pub use self::export::{
    ExportCancelled, cancel_export, export_history, get_export_progress, record_export_failure,
};
pub use self::favicons::{load_history_favicons, load_site_favicons};
// og_images functions are re-exported via the `og_images` module path so the
// worker and Tauri command crates can address them as
// `vault_core::archive::history::og_images::*`. They land in C3/C4.
#[allow(unused_imports)]
pub use self::og_images::{
    OgImageInsert, clear_cache as clear_og_image_cache, load_og_images, mark_og_images_shown,
    run_cleanup as run_og_image_cleanup, storage_stats as og_image_storage_stats, upsert_og_image,
};
/// Dispatches all search forms to the owner while retaining the native timeline reader.
pub fn list_history(vault: &crate::Vault, query: HistoryQuery) -> Result<HistoryQueryResponse> {
    if query.q.as_ref().is_some_and(|q| !q.trim().is_empty()) {
        return crate::search::history(vault, query);
    }
    timeline::list_history(vault, query)
}

/// Resolves the existing owner for callers that still carry an archive root.
pub fn search_history(
    paths: &ProjectPaths,
    _config: &AppConfig,
    _key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    crate::search::history_for_root(paths, query)
}

/// Maximum characters surfaced in a lexical-search enrichment excerpt (06 §6).
///
/// A single-line teaser, not the full summary: the FE clamps to two lines and highlights query terms,
/// so a tight char cap keeps the search response payload bounded at the 14.4M tail (one capped string
/// per RESULT, not per visit) without an extra scan. 180 sits inside the prompt's ~160-200 guidance.
const ENRICHMENT_EXCERPT_MAX_CHARS: usize = 180;

/// Caps one enrichment summary into a search-result excerpt on a CHAR boundary (CJK-safe).
///
/// PURE → unit-tested. Empty/whitespace text yields `None` (the FE suppresses a blank affordance).
/// Text already within budget is returned verbatim; longer text is sliced by CHARACTER count — never
/// by byte index, so a multi-byte CJK or emoji codepoint can never be split — and gets a trailing
/// ellipsis. Mirrors the char-count cap pattern used by `enrichment::truncate_text` /
/// `agent_store` auto-titles rather than inventing a new truncation idiom.
pub(crate) fn cap_enrichment_excerpt(enrichment_text: &str) -> Option<String> {
    let trimmed = enrichment_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= ENRICHMENT_EXCERPT_MAX_CHARS {
        return Some(trimmed.to_string());
    }
    let capped: String = trimmed.chars().take(ENRICHMENT_EXCERPT_MAX_CHARS).collect();
    Some(format!("{}…", capped.trim_end()))
}
