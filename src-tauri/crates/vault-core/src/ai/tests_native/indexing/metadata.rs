//! Responsibilities: metadata behavior regressions.
//! Not responsible for: other test groups or production code.
//! Failure modes: wrong state, missing records and replay inconsistency.
use super::*;

#[test]
fn enrichment_summary_re_embed_blast_radius_is_bounded_to_the_enriched_url() {
    // W-ENRICH-1 / 06 §3: filling a URL's enrichment_summary re-hashes (and thus marks for re-embed)
    // ONLY that URL, never the rest of the corpus. We seed two visits, store both as "already embedded"
    // under their NO-summary dedup hash, then enrich ONLY visit 1 — and assert only visit 1 flips to
    // `needs_embedding` while visit 2 stays unchanged. This proves `enrichment_summary_for` resolves the
    // stored summary into the dedup hash with a bounded blast radius.
    let (paths, _config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://github.com/o/r", Some("o/r"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);

    // Both visits are "already embedded" under their summary-less dedup hash.
    seed_embedding(
        &connection,
        1,
        &provider,
        &build_dedup_content_hash("https://github.com/o/r", Some("o/r"), None),
    );
    seed_embedding(
        &connection,
        2,
        &provider,
        &build_dedup_content_hash("https://example.com/blog", Some("Blog"), None),
    );

    // Nothing changed yet → both are skipped (needs_embedding = false).
    let before = collect_visit_chunk(&vault, &provider, 0, i64::MAX, 10, "legacy").expect("before");
    assert!(
        before.iter().all(|visit| !visit.needs_embedding),
        "no enrichment → nothing to re-embed"
    );

    // Enrich ONLY visit 1 with a capped summary (the content-fetch job would do this).
    connection
        .write(move |session| {
            crate::enrichment::store_enrichment_with_cache(
                &paths,
                session,
                "https://github.com/o/r",
                "github-repo",
                &crate::enrichment::EnrichmentResult {
                    status: "success".to_string(),
                    readable_title: Some("o/r".to_string()),
                    enrichment_summary: Some("A repo about wasm".to_string()),
                    extractor_version: Some(1),
                    ..crate::enrichment::EnrichmentResult::default()
                },
                None,
            )
        })
        .expect("store enrichment for visit 1");

    let after = collect_visit_chunk(&vault, &provider, 0, i64::MAX, 10, "legacy").expect("after");
    let visit1 = after.iter().find(|visit| visit.history_id == 1).expect("visit 1");
    let visit2 = after.iter().find(|visit| visit.history_id == 2).expect("visit 2");
    assert!(visit1.needs_embedding, "the enriched URL must re-embed (its dedup hash changed)");
    assert!(
        !visit2.needs_embedding,
        "the un-enriched URL must NOT re-embed (bounded blast radius)"
    );
    // The enriched visit's embedded TEXT now carries the summary (the 06 §4 funnel).
    assert!(visit1.content.lines().any(|line| line == "A repo about wasm"));
}
