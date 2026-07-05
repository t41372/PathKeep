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
/// PURE → unit-tested + mutation-hardened.
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
/// ONLY; the embed loop's WORK-dedup keys on the FULL `content_hash` (see `indexing::select_embed_targets`,
/// MEDIUM-4), so two distinct pages whose hashes collide on this u64 are EACH still embedded — the
/// second is NOT silently dropped onto the first's vector. At the `.pkvec` layer such a collision
/// stores both records under one u64 (resolved by `read_all` last-writer-wins); the SQLite
/// `ai_embeddings` rows carry the full `content_hash` for exact identity. PURE → unit-tested. A hex
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_hash_is_visit_independent_and_collapses_url_variants() {
        // Two visits of the SAME page (different tracking params + host casing) share one hash.
        let a = build_dedup_content_hash(
            "https://Mail.Google.com/inbox?utm_source=newsletter",
            Some("Inbox"),
            None,
        );
        let b = build_dedup_content_hash("https://mail.google.com/inbox", Some("Inbox"), None);
        assert_eq!(a, b, "tracking params + host casing must collapse to one dedup identity");
    }

    #[test]
    fn dedup_hash_changes_with_title_and_summary() {
        let base = build_dedup_content_hash("https://example.com/p", Some("Old"), None);
        let new_title = build_dedup_content_hash("https://example.com/p", Some("New"), None);
        assert_ne!(base, new_title, "a title change re-hashes (and re-embeds) the page");

        // The RESERVED enrichment slot: filling it changes the hash for THAT url only (06 §3).
        let enriched =
            build_dedup_content_hash("https://example.com/p", Some("Old"), Some("a summary"));
        assert_ne!(base, enriched, "filling enrichment_summary re-hashes only the enriched URL");
    }

    #[test]
    fn dedup_hash_treats_none_and_empty_uniformly_and_trims() {
        let none = build_dedup_content_hash("https://example.com/p", None, None);
        let empty = build_dedup_content_hash("https://example.com/p", Some(""), Some(""));
        assert_eq!(none, empty, "None and empty contribute the same empty segment");
        // Whitespace-only title is trimmed to empty.
        let spaces = build_dedup_content_hash("https://example.com/p", Some("   "), None);
        assert_eq!(none, spaces);
    }

    #[test]
    fn dedup_hash_falls_back_on_unparseable_url_without_panicking() {
        let a = build_dedup_content_hash("not a url", Some("T"), None);
        let b = build_dedup_content_hash("  not a url  ", Some("T"), None);
        // The raw string is trimmed, so the same garbage URL dedups against itself.
        assert_eq!(a, b);
    }

    #[test]
    fn content_key_reads_first_eight_bytes_big_endian() {
        // A known 16-hex prefix maps to the matching u64.
        assert_eq!(content_key_from_hash("0102030405060708ffff"), 0x0102_0304_0506_0708);
        // Different hashes → (almost surely) different keys.
        let key_a = content_key_from_hash(&build_dedup_content_hash("https://a.com/", None, None));
        let key_b = content_key_from_hash(&build_dedup_content_hash("https://b.com/", None, None));
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn content_key_truncation_collides_for_distinct_hashes_sharing_a_u64_prefix() {
        // MEDIUM-4: the u64 key is a 16-hex-char TRUNCATION, so two DISTINCT full hashes that share
        // their first 16 hex chars collide on the u64. This is the root cause the embed loop must
        // tolerate by keying its work-dedup on the FULL hash (see indexing::select_embed_targets) —
        // documented + asserted here so the truncation behavior is pinned.
        let hash_a = "0102030405060708aaaaaaaaaaaaaaaa";
        let hash_b = "0102030405060708bbbbbbbbbbbbbbbb";
        assert_ne!(hash_a, hash_b, "the full hashes are distinct");
        assert_eq!(
            content_key_from_hash(hash_a),
            content_key_from_hash(hash_b),
            "distinct hashes sharing a 16-hex prefix truncate to the same u64 (the collision)",
        );
    }

    #[test]
    fn content_key_handles_uppercase_and_short_input() {
        // Uppercase hex is accepted (defensive; sha256_hex emits lowercase but be robust).
        assert_eq!(content_key_from_hash("0A0B0C0D0E0F0102"), 0x0A0B_0C0D_0E0F_0102);
        // A short / non-hex input yields a deterministic 0 rather than panicking.
        assert_eq!(content_key_from_hash(""), 0);
        assert_eq!(content_key_from_hash("xyz"), 0);
    }
}
