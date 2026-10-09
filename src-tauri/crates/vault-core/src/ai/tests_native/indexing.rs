//! Responsibilities: native AI indexing regression coverage.
//! Not responsible for: search or network transport.
//! Failure modes: omitted records, stale state, non-durable progress, and incorrect provider scope.
use super::*;

#[test]
fn build_ai_index_returns_without_network_when_no_candidates_exist() {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    drop(connection);
    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding_provider(),
            &AiIndexRequest { full_rebuild: true, limit: Some(5), ..AiIndexRequest::default() },
        ))
        .expect("empty build");
    assert_eq!(report.indexed_items, 0);
    assert!(report.notes.iter().any(|note| note.contains("No new or changed history rows")));
}

#[test]
fn build_ai_index_with_control_stops_at_batch_boundaries() {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    let control: Arc<dyn AiRunControl> = Arc::new(CountdownControl::new(1));

    let error = runtime
        .block_on(build_ai_index_with_control(
            &vault.with_config(config.clone()),
            &embedding_provider(),
            &AiIndexRequest::default(),
            Some(control),
            0,
            0,
            None,
        ))
        .expect_err("controlled build should cancel");
    assert!(error.to_string().contains("cancelled"));
}

#[test]
fn build_ai_index_embeds_rows_into_vector_plane() {
    // The embed loop (W-AI-4a) now writes real vectors to `derived/vectors/` instead of bailing.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/news", Some("News"), 3);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("embed build");
    assert_eq!(report.indexed_items, 3);
    assert_eq!(report.updated_items, 0);
    assert_eq!(report.skipped_items, 0);
    assert!(report.notes[0].contains("vector plane"));

    // Vectors landed on the dedicated plane, keyed by content_key (deduped), fingerprint-stamped.
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert!(store.exists());
    // Three distinct pages → three deduped vectors (one per content_key).
    assert_eq!(store.count().expect("count"), 3);
    let records = store.read_all().expect("read all");
    let header = store.read_header().expect("header").expect("present");
    assert_eq!(header.dim, records[0].1.len());

    // The visit→content map fans each visit to its content_key (the dedup join), all three visits.
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let mut mapped: Vec<i64> = map.mapped_history_ids().expect("mapped").into_iter().collect();
    mapped.sort_unstable();
    assert_eq!(mapped, vec![1, 2, 3]);
    // Each visit's content_key resolves to a vector actually on the store (no orphan map entry).
    let store_keys = store.existing_ids().expect("store keys");
    for content_key in map.referenced_content_keys().expect("referenced") {
        assert!(store_keys.contains(&content_key), "every mapped content_key has a vector");
    }

    // SQLite metadata rows mirror the embedded count; vectors themselves are NOT in SQLite.
    let connection = TestDatabase::new(&vault, Db::Intelligence);
    assert_eq!(
        embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("count"),
        3
    );

    // A second build with unchanged content re-scans but skips every row (no re-embed).
    let second = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("idempotent rebuild");
    assert_eq!(second.indexed_items, 0);
    assert_eq!(second.skipped_items, 3);
    assert_eq!(store.count().expect("count after rebuild"), 3);
}

#[test]
fn empty_vector_provider_fails_the_build_without_counting_or_writing_a_store() {
    // HEADLINE (F3): a provider/path that yields empty (zero-dim) vectors must surface a real failure
    // and NEVER report success with an empty store — `indexed_items` stays 0 and no `.pkvec` is left.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = zero_dim_embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/x", Some("Page"), 1);
    drop(connection);

    let error = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect_err("a zero-dim provider must fail the build, not report success");
    assert!(
        error.to_string().contains("empty vector"),
        "the failure names the empty-vector cause: {error}"
    );

    // No vector store was created (nothing was honestly indexed).
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert!(!store.exists(), "no .pkvec store is left behind by a failed empty-vector build");

    // The status read model reports zero vectors (and is NOT ready) for this provider.
    let mut status_config = config.clone();
    status_config.ai.embedding_provider_id = Some(provider.config.id.clone());
    status_config.ai.embedding_providers = vec![provider.config.clone()];
    let status = ai_index_status(&vault.with_config(status_config.clone())).expect("status");
    assert_eq!(status.semantic_vector_count, 0);
    assert!(!status.ready);
}

#[test]
fn build_ai_index_full_scope_wipes_and_rebuilds() {
    // W-AI-9-D: `ReembedScope::Full` carries full-rebuild semantics — it wipes the plane then embeds
    // every unique page, exactly like the legacy `full_rebuild` flag.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/a", Some("A"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/b", Some("B"), 2);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest { scope: crate::ReembedScope::Full, ..AiIndexRequest::default() },
        ))
        .expect("full-scope build");
    assert_eq!(report.indexed_items, 2);
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 2);
}

#[test]
fn build_ai_index_dedupes_repeated_page_into_one_vector_and_maps_every_visit() {
    // W-AI-4c content-hash dedup (05 §1, "the biggest near-free lever"): many visits of the SAME
    // page (same canonical URL + title, tracking-param variants collapsed) must produce ONE vector
    // yet map EVERY visit to it — "5000 gmail visits → 1 embedding" in miniature.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    // Five visits of the SAME page (different tracking params + host casing → same canonical URL),
    // plus one DIFFERENT page → 2 unique contents across 6 visits.
    seed_visit(&connection, 1, "chrome:Default", "https://mail.google.com/inbox", Some("Inbox"), 1);
    seed_visit(
        &connection,
        2,
        "chrome:Default",
        "https://mail.google.com/inbox?utm_source=a",
        Some("Inbox"),
        2,
    );
    seed_visit(&connection, 3, "chrome:Default", "https://Mail.Google.com/inbox", Some("Inbox"), 3);
    seed_visit(
        &connection,
        4,
        "chrome:Default",
        "https://mail.google.com/inbox?ref=x",
        Some("Inbox"),
        4,
    );
    seed_visit(
        &connection,
        5,
        "chrome:Default",
        "https://mail.google.com/inbox?fbclid=z",
        Some("Inbox"),
        5,
    );
    seed_visit(&connection, 6, "chrome:Default", "https://example.com/other", Some("Other"), 6);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("dedup build");
    // All 6 visits are processed (metadata rows), but only 2 UNIQUE pages are embedded.
    assert_eq!(report.indexed_items, 6);

    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 2, "6 visits of 2 pages → 2 deduped vectors");

    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let all = map.read_all().expect("map");
    // Every visit is mapped.
    let mut mapped_ids: Vec<i64> = all.keys().copied().collect();
    mapped_ids.sort_unstable();
    assert_eq!(mapped_ids, vec![1, 2, 3, 4, 5, 6]);
    // The five gmail visits all share ONE content_key; the sixth has its own.
    let gmail_key = all[&1];
    for id in 2..=5 {
        assert_eq!(all[&id], gmail_key, "every gmail visit maps to the same content_key");
    }
    assert_ne!(all[&6], gmail_key, "the different page has its own content_key");
    // Exactly 2 distinct content_keys referenced, both with a vector on the store.
    let referenced = map.referenced_content_keys().expect("referenced");
    assert_eq!(referenced.len(), 2);
    let store_keys = store.existing_ids().expect("store keys");
    assert_eq!(referenced, store_keys, "every referenced content_key has a vector, no orphans");
}

#[test]
fn build_index_re_embeds_changed_rows_and_counts_updates() {
    // Row 1 already has a (stale) metadata row → re-embedded as an update; row 2 is new.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    seed_embedding(&connection, 1, &embedding, "stale-hash");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest::default(),
        ))
        .expect("embed build");
    assert_eq!(report.indexed_items, 2);
    assert_eq!(report.updated_items, 1);

    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 2);
}

#[test]
fn build_index_full_rebuild_resets_the_vector_plane() {
    // A full rebuild deletes any prior store and re-stamps from the freshly observed dim.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);

    runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest::default(),
        ))
        .expect("initial build");
    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 1);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest { full_rebuild: true, ..AiIndexRequest::default() },
        ))
        .expect("full rebuild");
    assert_eq!(report.indexed_items, 1);
    // Still exactly one record (rebuilt, not duplicated) — the prior store was reset.
    assert_eq!(store.count().expect("count after rebuild"), 1);
}

#[test]
fn build_index_uses_limit_as_page_size_and_reaches_every_candidate() {
    // A page size below the candidate count must not become a total coverage limit.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let embedding = embedding_provider();
    for id in 1..=4 {
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

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() },
        ))
        .expect("limited build");
    assert_eq!(report.indexed_items, 4);
    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 4);
}

#[test]
fn build_index_incremental_rejects_a_stale_store() {
    // An incremental build over a store stamped with a DIFFERENT fingerprint must refuse to append
    // dimension-incompatible vectors — the clear seam W-AI-5 wires re-embed migration into.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);

    // Stamp a store at the SAME (provider, model) — so it shares the store file — but under a
    // different DIM, so the live config is stale against it.
    let stale_fp = EmbeddingFingerprint::new(
        embedding.config.id.clone(),
        embedding.config.default_model.clone(),
        99,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    VectorStore::create_stamped(&paths, &stale_fp).expect("stamp stale store");

    let error = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest::default(),
        ))
        .expect_err("stale store should block an incremental append");
    assert!(error.to_string().contains("different embedding configuration"));
}

#[test]
fn build_index_incremental_appends_to_a_matching_store() {
    // A second (incremental) build with a NEW row appends to the prior matching store rather than
    // recreating it — exercising the "reuse existing store" lazy-resolution branch.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);

    runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest::default(),
        ))
        .expect("first build");
    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 1);
    let header_before = store.read_header().expect("header").expect("present");

    // Add a new visit (via the intelligence connection's attached `archive` schema), then run an
    // incremental build (no full_rebuild) so it appends to the existing matching store.
    let connection = TestDatabase::new(&vault, Db::Intelligence);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &embedding,
            &AiIndexRequest::default(),
        ))
        .expect("incremental build");
    assert_eq!(report.indexed_items, 1);
    assert_eq!(store.count().expect("count after incremental"), 2);
    // Same store (same fingerprint header), appended in place.
    assert_eq!(store.read_header().expect("header2").expect("present"), header_before);
}

/// Embeds rows [start, …] capped by `limit` and returns the resulting MAPPED VISIT id set + the last
/// reported watermark, so the resume tests can drive deterministic chunks regardless of the
/// build-time `EMBEDDING_BACKFILL_CHUNK`.
///
/// W-AI-4c: the vector store is now keyed by content_key (deduped), so the per-VISIT no-dup/no-miss
/// guarantee lives on the visit→content map. These resume tests assert that visit-level contract via
/// the map's mapped history ids (sorted ascending), preserving the original W-AI-4a intent.
fn run_backfill_chunk(
    runtime: &Runtime,
    vault: &crate::Vault,
    config: &AppConfig,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    start_history_id: i64,
) -> (Vec<i64>, IndexBackfillProgress) {
    let paths = vault.paths();
    let ledger = Arc::new(PauseAfterBatch {
        progress: std::sync::Mutex::new(None),
        pause: request.limit.is_some(),
    });
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    let result = runtime.block_on(build_ai_index_with_control(
        &vault.with_config(config.clone()),
        provider,
        request,
        None,
        start_history_id,
        0,
        Some(dyn_ledger),
    ));
    if request.limit.is_some() {
        assert!(result.as_ref().unwrap_err().downcast_ref::<AiRunCancelled>().is_some());
    } else {
        result.expect("backfill chunk");
    }
    let map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    let mut ids: Vec<i64> = map.mapped_history_ids().expect("mapped ids").into_iter().collect();
    ids.sort_unstable();
    let last = ledger.progress.lock().unwrap().unwrap_or_default();
    (ids, last)
}

#[test]
fn static_tier_index_writes_real_vectors_and_semantic_search_finds_the_seeded_page() {
    let _vault = crate::Vault::open_temp().unwrap();
    // HEADLINE (F1 + F4 + F3): build the index with the BUILT-IN STATIC tier over a few seeded rows
    // through the REAL in-memory static engine → assert `.pkvec` is NON-ZERO and the vector count
    // matches `indexed_items` → semantic search resolves the seeded page. This exercises the real
    // static embed (tokenize → matrix lookup → mean-pool → L2-norm), not a digest stub.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = static_embedding_provider();
    // Each page carries a DISTINCTIVE vocab word in its title so the bag-of-words static vectors are
    // separable: only the "kestrel" page contains "kestrel".
    seed_visit(
        &connection,
        1,
        "chrome:Default",
        "https://example.com/a",
        Some("kestrel falcon"),
        1,
    );
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/b", Some("banana zebra"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/c", Some("quantum otter"), 3);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest::default(),
        ))
        .expect("static build");
    assert_eq!(report.indexed_items, 3);
    assert!(
        report.notes[0].contains("page vector"),
        "honest note reports vectors: {:?}",
        report.notes
    );

    // The `.pkvec` plane is NON-ZERO and its vector count matches the indexed rows (F3 honesty).
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert!(store.exists());
    let vector_count = store.count().expect("count");
    assert_eq!(vector_count, 3, "one vector per distinct page");
    assert_eq!(vector_count, report.indexed_items as u64, "vector count == indexed_items");
    assert!(
        store.path().metadata().expect("meta").len() > 0,
        "the .pkvec store holds non-zero bytes"
    );
    // The stored vectors are the static engine's real dim (not the external stub dim).
    let header = store.read_header().expect("header").expect("present");
    assert!(header.dim > 1, "static engine produced a real multi-dim vector (dim {})", header.dim);

    // Semantic search through the SAME static engine resolves the page carrying the query word.
    let response = runtime
        .block_on(semantic_search_history(
            &vault,
            &paths,
            &config,
            None,
            Some(&provider),
            &AiSearchRequest {
                query: "kestrel".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
                browser_kind: None,
            },
        ))
        .expect("semantic search");
    assert!(
        response.items.iter().any(|item| item.url == "https://example.com/a"),
        "semantic search over the real static index returns the seeded 'kestrel' page; got {:?}",
        response.items.iter().map(|item| &item.url).collect::<Vec<_>>()
    );
}

struct PauseAfterBatch {
    progress: std::sync::Mutex<Option<IndexBackfillProgress>>,
    pause: bool,
}
impl IndexBackfillLedger for PauseAfterBatch {
    fn record(&self, value: IndexBackfillProgress) -> Result<()> {
        *self.progress.lock().unwrap() = Some(value);
        if self.pause {
            Err(AiRunCancelled::new("test interruption after a durable batch").into())
        } else {
            Ok(())
        }
    }
}

mod metadata;

mod resume;
