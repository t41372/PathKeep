//! Embedding-content rendering and the enrichment → dedup-hash seam.
//!
//! ## Responsibilities
//! - render the stable plain-text representation hashed/embedded for one history row
//! - resolve the capped enrichment summary that feeds the visit-independent dedup hash
//!
//! ## Not responsible for
//! - the dedup hashing itself (`super::super::dedup` owns the hash + u64 key derivation)
//! - candidate collection or storage-row persistence (sibling submodules own those)

use super::super::*;

/// Builds the stable plain-text representation used to hash and embed one history row.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::ai) fn build_embedding_content(
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
) -> String {
    let title = title.unwrap_or("(untitled)");
    format!(
        "Profile: {profile_id}\nVisited at: {visited_at}\nURL: {url}\nDomain: {domain}\nTitle: {title}",
        domain = url_domain(url)
    )
}

/// Resolves the capped `enrichment_summary` for one visit's dedup hash (W-ENRICH-1, 06 §3).
///
/// Returns the stored capped summary from the best `visit_content_enrichments` row, so a freshly
/// enriched URL re-hashes (and thus re-embeds) ONLY itself — the bounded blast radius the dedup hash
/// was designed for (06 §3 / 05 §1). A visit with no enrichment (or a failure row that stored no
/// summary) contributes `None`, the same empty dedup segment as before W-ENRICH-1, so the corpus that
/// was never enriched keeps its existing content_hash and is NOT re-embedded. This is the SINGLE seam
/// between the enrichment plane and the embedding dedup identity — centralizing it here is why
/// W-ENRICH-1 only had to fill this one function (the reserved slot the dedup hash already carried).
///
/// Whitespace-only / empty summaries are treated as absent so they never perturb the hash; the stored
/// value is already capped + normalized by the extractor (`build_enrichment_summary`), so this is a
/// thin pass-through that keeps the dedup hash stable across re-reads of the same row.
pub(in crate::ai) fn enrichment_summary_for(
    enrichment: Option<&crate::enrichment::StoredEnrichment>,
) -> Option<&str> {
    enrichment
        .and_then(|value| value.enrichment_summary.as_deref())
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
}
