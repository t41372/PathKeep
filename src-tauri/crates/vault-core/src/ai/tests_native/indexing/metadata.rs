//! Responsibilities: metadata behavior regressions.
//! Not responsible for: other test groups or production code.
//! Failure modes: wrong state, missing records and replay inconsistency.
use super::*;

#[test]
fn collect_visits_to_index_skips_already_indexed_rows_and_cleanup_removes_stale_rows() {
    let (_paths, _config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);

    // History id 1 is "already indexed" iff its stored hash equals the VISIT-INDEPENDENT dedup hash
    // (canonical URL + title + reserved enrichment summary), so it is skipped on the next collect.
    let first_dedup_hash = build_dedup_content_hash("https://example.com/docs", Some("Docs"), None);
    seed_embedding(&connection, 1, &provider, &first_dedup_hash);
    seed_embedding(&connection, 999, &provider, "orphan-hash");

    let candidates = collect_visits_to_index(&vault, &provider, Some(10)).expect("collect");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].history_id, 2);

    let removed = cleanup_stale_embeddings(&vault, &provider, None).expect("cleanup");
    assert_eq!(removed, 1);
}

#[test]
fn cleanup_stale_embeddings_returns_zero_when_nothing_is_removed() {
    let (_paths, _config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let removed = cleanup_stale_embeddings(&vault, &embedding_provider(), None).expect("cleanup");
    assert_eq!(removed, 0);
}

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
    let before = collect_visit_chunk(&vault, &provider, 0, i64::MAX, 10).expect("before");
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

    let after = collect_visit_chunk(&vault, &provider, 0, i64::MAX, 10).expect("after");
    let visit1 = after.iter().find(|visit| visit.history_id == 1).expect("visit 1");
    let visit2 = after.iter().find(|visit| visit.history_id == 2).expect("visit 2");
    assert!(visit1.needs_embedding, "the enriched URL must re-embed (its dedup hash changed)");
    assert!(
        !visit2.needs_embedding,
        "the un-enriched URL must NOT re-embed (bounded blast radius)"
    );
    // The enriched visit's embedded TEXT now carries the summary (the 06 §4 funnel).
    assert!(visit1.content.contains("Summary: A repo about wasm"));
}

#[test]
fn upsert_embedding_reports_prior_existence() {
    let (_paths, _config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let candidates = collect_visit_chunk(&vault, &provider, 0, i64::MAX, 10).expect("collect");
    let visit = &candidates[0];

    // First write: no prior row.
    assert!(!put_embedding(&connection, &provider, visit, "2026-04-14T00:00:00Z").expect("first"));
    // Second write of the same (history_id, provider, model): prior row existed.
    assert!(put_embedding(&connection, &provider, visit, "2026-04-14T00:00:01Z").expect("second"));
    // Still exactly one row (the prior one was replaced, not duplicated).
    assert_eq!(
        embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("count"),
        1
    );
}

#[test]
fn index_clear_only_and_sqlite_mirror_helpers_cover_stale_rows() {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let candidates = collect_visits_to_index(&vault, &provider, Some(1)).expect("collect visit");
    assert_eq!(candidates.len(), 1);
    put_embedding(&connection, &provider, &candidates[0], "2026-04-14T00:00:00Z")
        .expect("upsert embedding mirror");
    assert_eq!(
        embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("provider count"),
        1
    );
    assert!(
        collect_stale_history_ids(&vault, &provider, None).expect("fresh stale ids").is_empty()
    );

    archive
        .execute("UPDATE visits SET reverted_at = '2026-04-14T00:00:00Z' WHERE id = 1", ())
        .expect("revert indexed visit");
    archive
        .execute(
            "UPDATE archive_totals SET value=0 WHERE name='visits' OR name LIKE 'sourceVisits:%'",
            (),
        )
        .unwrap();
    assert_eq!(collect_stale_history_ids(&vault, &provider, None).expect("stale ids"), vec![1]);
    clear_provider_embeddings(&vault, &provider, None).expect("clear provider mirror");
    assert_eq!(
        embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("provider count after clear"),
        0
    );

    seed_embedding(&connection, 1, &provider, "old-hash");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest { clear_only: true, ..AiIndexRequest::default() },
        ))
        .expect("clear semantic index");
    assert_eq!(report.indexed_items, 0);
    assert!(report.notes[0].contains("Cleared the semantic index"));
    let cleared = TestDatabase::new(&vault, Db::Intelligence);
    assert_eq!(
        embedding_count(&cleared, &provider.config.id, &provider.config.default_model)
            .expect("cleared provider count"),
        0
    );
}
