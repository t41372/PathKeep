//! Responsibilities: resume behavior regressions.
//! Not responsible for: other test groups or production code.
//! Failure modes: wrong state, missing records and replay inconsistency.
use super::*;

#[test]
fn incremental_revisit_build_maps_without_embedding_and_reports_it_honestly() {
    // F3 honesty note: a NEW visit of an ALREADY-embedded page is MAPPED but produces NO new vector,
    // so the build reports "Mapped N row(s) to existing vectors" rather than implying a fresh embed.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/p", Some("Page"), 1);
    let first = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("first build");
    assert_eq!(first.indexed_items, 1);
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 1);

    // A NEW visit (history_id 2) of the SAME page (same url_id → same content_key).
    seed_extra_visit(&connection, 2, 1, 2);
    drop(connection);
    let second = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("second build");
    assert_eq!(second.indexed_items, 1, "the new visit is processed (mapped)");
    assert_eq!(
        store.count().expect("count after revisit"),
        1,
        "no new vector — page already embedded"
    );
    assert!(second.notes[0].contains("Mapped"), "honest mapped note: {:?}", second.notes);
}

#[test]
fn build_with_ledger_persists_last_durable_checkpoint_on_failure() {
    // F2.1: when a chunk fails, the LAST-DURABLE cursor is persisted via the ledger BEFORE the error
    // propagates, so a reclaim resumes from real progress (here the origin, since the first chunk
    // itself fails). Without the failure-path checkpoint the ledger would never see this position.
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/x", Some("Page"), 1);
    drop(connection);
    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    let error = runtime
        .block_on(build_ai_index_with_control(
            &vault.with_config(config.clone()),
            &zero_dim_embedding_provider(),
            &AiIndexRequest::default(),
            None,
            0,
            0,
            Some(dyn_ledger),
        ))
        .expect_err("zero-dim build fails");
    assert!(error.to_string().contains("empty vector"));
    let progresses = ledger.snapshot();
    assert_eq!(
        progresses.last().copied(),
        // The watermark is still the origin (the first chunk itself failed), but the denominators are
        // captured at the build's TRUE start: `scan_target` = max candidate history id (1) and
        // `embed_target` = candidate page COUNT (1, the single seeded visit) — proving both determinate
        // denominators are recorded even when the build fails before any chunk completes.
        Some(IndexBackfillProgress {
            next_history_id: 0,
            embedded_so_far: 0,
            scan_target: 1,
            embed_target: 1,
        }),
        "the failure path persisted the last-durable checkpoint (origin + captured denominators)"
    );
}

#[test]
fn build_ai_index_resumes_from_cursor_and_records_progress() {
    // The resumable backfill: starting from a cursor only embeds rows at or beyond it, and the
    // ledger receives the advancing watermark.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    // Resume from history_id 3 → only rows 3,4,5 are embedded.
    let report = runtime
        .block_on(build_ai_index_with_control(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
            None,
            3,
            0,
            Some(dyn_ledger),
        ))
        .expect("resumed build");
    assert_eq!(report.indexed_items, 3);

    // Visits 3,4,5 were mapped (the per-visit no-miss contract), and 3 distinct pages → 3 vectors.
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let mut mapped: Vec<i64> = map.mapped_history_ids().expect("mapped").into_iter().collect();
    mapped.sort_unstable();
    assert_eq!(mapped, vec![3, 4, 5]);
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 3, "3 distinct pages → 3 deduped vectors");

    // The ledger saw a monotone, advancing watermark ending past the last id with the full count.
    let progresses = ledger.snapshot();
    assert!(!progresses.is_empty());
    let last = progresses.last().expect("final progress");
    assert_eq!(last.embedded_so_far, 3);
    assert!(last.next_history_id >= 6);
    for window in progresses.windows(2) {
        assert!(window[1].next_history_id >= window[0].next_history_id);
        assert!(window[1].embedded_so_far >= window[0].embedded_so_far);
    }
}

#[test]
fn build_ai_index_resume_carries_cumulative_embedded_so_far() {
    // HIGH regression: the persisted `embedded_so_far` (and the embedded/embed_target progress bar)
    // must be CUMULATIVE across a reclaim. A worker that died after embedding some pages resumes with a
    // non-zero `start_embedded_so_far`; the count must continue from there, not restart at this
    // session's partial count — which would collapse the bar to ~0% on the very recovery path the
    // orphan-recovery fix enables.
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    // Simulate a reclaim: resume from history_id 3 (rows 3,4,5 remain) with 2 pages already embedded in
    // the prior session.
    runtime
        .block_on(build_ai_index_with_control(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
            None,
            3,
            2,
            Some(dyn_ledger),
        ))
        .expect("resumed build");

    // 2 (carried from the prior session) + 3 (embedded this session) = 5 cumulative — NOT 3.
    let last = ledger.snapshot().last().copied().expect("final progress");
    assert_eq!(
        last.embedded_so_far, 5,
        "embedded_so_far must carry the resume seed, not reset to 3"
    );
}

#[test]
fn full_rebuild_resume_keeps_rows_below_cursor_and_completes_id_set() {
    // CRITICAL-1: a crashed full_rebuild that resumes with the persisted cursor (and the ORIGINAL
    // full_rebuild request still true) must NOT re-run the destructive wipe — rows already embedded
    // below the cursor MUST survive, and the final store must hold the complete id set exactly once.
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    // Chunk 1 of a FULL REBUILD: embed ids 1,2 (limit caps the embed count), cursor advances to 3.
    let full_rebuild =
        AiIndexRequest { full_rebuild: true, limit: Some(2), ..AiIndexRequest::default() };
    let (ids_after_chunk1, progress) =
        run_backfill_chunk(&runtime, &vault, &config, &provider, &full_rebuild, 0);
    assert_eq!(ids_after_chunk1, vec![1, 2], "chunk 1 embedded ids 1,2");
    assert_eq!(progress.next_history_id, 3, "cursor advances to last + 1");

    // Simulate a worker restart that re-claims the job with the ORIGINAL full_rebuild request still
    // set, resuming from the persisted cursor. The wipe MUST be skipped (start_history_id > 0).
    let resume = AiIndexRequest { full_rebuild: true, limit: None, ..AiIndexRequest::default() };
    let (final_ids, _) =
        run_backfill_chunk(&runtime, &vault, &config, &provider, &resume, progress.next_history_id);

    // Rows below the cursor SURVIVED and the full id set is present exactly once.
    assert_eq!(final_ids, vec![1, 2, 3, 4, 5], "rows below the cursor must not be wiped on resume");
    assert_unique(&final_ids);
}

#[test]
fn resume_after_dropped_cursor_does_not_duplicate_vectors() {
    // CRITICAL-2: a crash AFTER a chunk's vectors are appended but BEFORE its SQLite hash rows /
    // cursor are written leaves the vectors on disk while `needs_embedding` is true again. A resume
    // must re-embed those rows WITHOUT appending a second copy. We reproduce the window by deleting
    // the boundary chunk's SQLite metadata, then resuming from before it.
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }

    // Chunk 1: embed ids 1,2 → vectors on disk, SQLite rows present.
    let chunk = AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() };
    let (ids, _) = run_backfill_chunk(&runtime, &vault, &config, &provider, &chunk, 0);
    assert_eq!(ids, vec![1, 2]);

    // Simulate the crash window: the vectors for 1,2 are on disk but their SQLite hash rows were
    // never written (so they look un-embedded to the next pass).
    clear_provider_embeddings(&vault, &provider, None).expect("drop metadata");
    drop(connection);

    // Resume re-scanning from id 1 (cursor lost back to the start of the boundary chunk): rows 1,2
    // are re-embedded (needs_embedding true) but their vectors must NOT be appended a second time.
    let resume = AiIndexRequest::default();
    let (final_ids, _) = run_backfill_chunk(&runtime, &vault, &config, &provider, &resume, 1);

    assert_unique(&final_ids);
    let mut sorted = final_ids.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5], "exact id set, no dup, no miss");
}

#[test]
fn multi_chunk_resume_yields_exact_contiguous_id_set() {
    // HIGH-7: a real multi-chunk run resumed from each persisted cursor must end with EXACTLY the
    // ids [1..=N] — no duplicate, no miss — and each cursor advance is exactly last + 1.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    const N: i64 = 5;
    for id in 1..=N {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    // Walk the backfill two rows at a time, always resuming from the last reported watermark.
    let mut start = 0_i64;
    let mut guard = 0;
    loop {
        let chunk = AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() };
        let (ids, progress) =
            run_backfill_chunk(&runtime, &vault, &config, &provider, &chunk, start);
        assert_unique(&ids);
        if progress.next_history_id == 0 {
            // Nothing was embedded this pass (no candidates at/after `start`): the run is complete.
            break;
        }
        // The cursor advances to exactly last embedded id + 1.
        assert!(progress.next_history_id > start);
        if ids.len() as i64 >= N {
            assert_eq!(progress.next_history_id, N + 1, "final cursor is last id + 1");
            break;
        }
        start = progress.next_history_id;
        guard += 1;
        assert!(guard < 10, "multi-chunk resume should converge");
    }

    // The visit→content map holds EXACTLY the visit id set [1..=5] — no dup, no miss — after the
    // multi-chunk resume (the per-visit contract the store's content_key keying moved here).
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let mut final_ids: Vec<i64> = map.mapped_history_ids().expect("mapped").into_iter().collect();
    final_ids.sort_unstable();
    assert_eq!(final_ids, vec![1, 2, 3, 4, 5], "exact contiguous id set after multi-chunk resume");
    assert_unique(&final_ids);
    // And 5 distinct pages → 5 deduped vectors, also no-dup.
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 5);
}

#[test]
fn incremental_build_does_not_re_embed_a_new_visit_of_an_already_embedded_page() {
    // MEDIUM-5: a PLAIN incremental job starts at cursor 0 with empty dedup sets. Without loading the
    // persisted keys/map when a store already exists, a NEW visit of an already-embedded page would
    // re-embed + append a DUPLICATE `.pkvec` record (bloat that `read_all` last-writer-wins masks).
    // Assert the RAW record count (store.count()), not read_all().len(), so the duplicate is visible.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    // First build embeds ONE page (one canonical URL, one visit).
    seed_visit(&connection, 1, "chrome:Default", "https://mail.google.com/inbox", Some("Inbox"), 1);
    drop(connection);
    runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("first build");
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 1, "one page → one vector after first build");

    // A NEW visit (history_id 2) of the SAME page (tracking-param variant → same canonical URL/hash).
    let connection = TestDatabase::new(&vault, Db::Intelligence);
    seed_visit(
        &connection,
        2,
        "chrome:Default",
        "https://mail.google.com/inbox?utm_source=x",
        Some("Inbox"),
        2,
    );
    drop(connection);

    // A PLAIN incremental build (cursor 0, no full_rebuild). The new visit must be MAPPED but the page
    // must NOT be re-embedded — the RAW record count stays 1 (no duplicate `.pkvec` record appended).
    runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("incremental build");
    assert_eq!(
        store.count().expect("raw count after incremental"),
        1,
        "a new visit of an already-embedded page must NOT append a duplicate vector record",
    );

    // The new visit IS mapped to the same content_key (the per-visit no-miss contract holds).
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let all = map.read_all().expect("map");
    let mut mapped: Vec<i64> = all.keys().copied().collect();
    mapped.sort_unstable();
    assert_eq!(mapped, vec![1, 2], "both visits are mapped");
    assert_eq!(all[&1], all[&2], "the new visit maps to the SAME content_key (one page)");
}
