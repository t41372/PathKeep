//! Content-hash dedup keys — the biggest, near-free embedding lever (W-AI-4c, 05 §1).
//!
//! ## Responsibilities
//! - derive the VISIT-INDEPENDENT `content_hash` from `canonical_url + title + enrichment_summary`
//!   (05 §1) so every visit of one page shares ONE hash — 5000 gmail visits → 1 embedding.
//! - reserve the `enrichment_summary` slot NOW (empty/None until W-ENRICH-1 fills it) so filling it
//!   later re-hashes only the enriched URLs, not the whole corpus (06 §3 / §8 dependency).
//! - derive the stable `content_key: u64` from the hash, the fixed-width key the `.pkvec` vector
//!   store + visit→content map use (keeping the flat, mmap-friendly record layout).
//!
//! ## Not responsible for
//! - the per-VISIT embedding content funnel (`enrichment::build_embedding_content_from_parts`): that
//!   builds the TEXT fed to the model and still carries profile/visited-at for the embedding payload.
//!   This module builds the DEDUP IDENTITY, which must NOT include visit-specific parts or every
//!   visit would be "unique" and dedup would collapse to nothing.
//! - persistence (the vector store / map own that) or canonicalization (reuses `visit_taxonomy`).
//!
//! ## Why a separate hash from the embedding-content hash
//! W-AI-4a hashed `build_embedding_content_from_parts` (profile + visited_at + url + title + …) and
//! keyed the store by `history_id`. That makes EVERY visit unique (different visited_at), defeating
//! dedup. The dedup identity here is deliberately the visit-INDEPENDENT page identity: canonical URL
//! (tracking-param + host-casing variants collapsed) + title + the reserved enrichment summary. Two
//! visits of the same page produce the same `content_hash` → the same `content_key` → one vector.

use crate::utils::sha256_hex;
use crate::visit_taxonomy::normalize_visit_url;

/// Builds the VISIT-INDEPENDENT dedup content hash for one page (05 §1).
///
/// `content_hash = sha256(canonical_url + "\n" + title + "\n" + enrichment_summary)`. The URL is
/// canonicalized through the SAME [`normalize_visit_url`] stars/annotations/refind use, so tracking
/// params + host casing collapse onto one identity (an unparseable URL falls back to the raw string
/// trimmed, so a malformed row still dedups against itself rather than panicking). `title` is
/// trimmed; `None`/empty title and `None`/empty summary both contribute an empty segment.
///
/// `enrichment_summary` is the RESERVED slot (06 §3): it is `None` for now (W-ENRICH-1 will pass the
/// capped summary). Each field unconditionally emits its own `\n`-separated segment, so the hash is
/// stable + collision-resistant across the three parts the same way the embedding fingerprint is.
/// Pure (no I/O).
pub fn build_dedup_content_hash(
    url: &str,
    title: Option<&str>,
    enrichment_summary: Option<&str>,
) -> String {
    let canonical = normalize_visit_url(url)
        .map(|normalized| normalized.canonical_url)
        .unwrap_or_else(|| url.trim().to_string());
    let title = title.map(str::trim).unwrap_or("");
    let summary = enrichment_summary.map(str::trim).unwrap_or("");
    // Each segment is `name=value` on its own line so an injected delimiter in one value can only
    // add lines, never impersonate another field — the same collision-resistance invariant the
    // embedding fingerprint relies on.
    let payload = format!("dedup/v1\ncanonical={canonical}\ntitle={title}\nenrichment={summary}");
    sha256_hex(payload.as_bytes())
}

/// Derives the fixed-width `content_key: u64` from a dedup content hash.
///
/// The `.pkvec` store + visit→content map key by a u64 (the flat fixed-stride layout), so we take
/// the first 8 bytes of the SHA-256 hex digest as a big-endian u64. SHA-256 is uniform, so the first
/// 64 bits are a well-distributed, stable key; a 64-bit space over an at-most-14.4M-unique corpus has
/// a small collision probability (≈ n²/2^65 ≈ 2.8e-6 at 14.4M). The u64 is the STORAGE-boundary key
/// ONLY; the embed loop's WORK-dedup keys on the FULL `content_hash` (see `indexing::select_embed_targets`),
/// so two distinct pages whose hashes collide on this u64 are EACH still embedded — the
/// second is NOT silently dropped onto the first's vector. At the `.pkvec` layer such a collision
/// stores both records under one u64 (resolved by `read_all` last-writer-wins); the SQLite
/// `ai_embeddings` rows carry the full `content_hash` for exact identity. Pure (no I/O). A hex
/// string shorter than 16 chars (only possible from a non-SHA input) yields 0, a deterministic fallback.
pub fn content_key_from_hash(content_hash: &str) -> u64 {
    let mut key: u64 = 0;
    for byte in content_hash.bytes().take(16) {
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => 0,
        };
        key = (key << 4) | nibble as u64;
    }
    key
}
