//! Responsibilities: backfill integrity and explicit serving-pair retention regressions.
//! Not responsible for: search or network transport.
//! Failure modes: omitted records, stale state, non-durable progress, and incorrect provider scope.
use super::*;

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
        start_history_id,
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
    start_history_id: i64,
}
impl IndexBackfillLedger for PauseAfterBatch {
    fn record(&self, value: IndexBackfillProgress) -> Result<()> {
        *self.progress.lock().unwrap() = Some(value);
        if self.pause && value.next_history_id > self.start_history_id {
            Err(AiRunCancelled::new("test interruption after a durable batch").into())
        } else {
            Ok(())
        }
    }
}

mod metadata;

mod resume;

/// Changing a service's model leaves the old model's index behind. Ways cleaning it up could go
/// wrong: the index still in use deleted with it; the old rows deleted but its files kept (or the
/// other way round); a ready ledger survives deleted files; cleanup repeats on every build.
#[test]
fn cleanup_preserves_the_serving_pair_until_replacement_then_removes_its_ledger() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    drop(connection);
    let old = embedding_provider();
    let mut new = embedding_provider();
    new.config.default_model = "another-embedding-model".to_string();
    for provider in [&old, &new] {
        runtime
            .block_on(build_ai_index(
                &vault.with_config(config.clone()),
                provider,
                &AiIndexRequest::default(),
            ))
            .expect("build");
    }
    let files = |provider: &AiProviderRuntime| {
        let (id, model) = (&provider.config.id, &provider.config.default_model);
        (
            VectorStore::for_provider(&paths, id, model).exists(),
            VisitContentMap::for_provider(&paths, id, model).exists(),
        )
    };
    assert_eq!(files(&old), (true, true));

    let retained = [
        (new.config.id.clone(), new.config.default_model.clone()),
        (old.config.id.clone(), old.config.default_model.clone()),
    ];
    assert_eq!(runtime.block_on(forget_unconfigured_indexes(&vault, &retained, None)).unwrap(), 0);
    assert_eq!(files(&old), (true, true), "serving files survive replacement work");
    let configured = [(new.config.id.clone(), new.config.default_model.clone())];
    let forgotten =
        runtime.block_on(forget_unconfigured_indexes(&vault, &configured, None)).expect("forget");
    assert_eq!(forgotten, 1);
    assert_eq!(files(&old), (false, false));
    assert_eq!(files(&new), (true, true));
    let connection = TestDatabase::new(&vault, Db::Intelligence);
    let rows = |provider: &AiProviderRuntime| {
        embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("count")
    };
    assert_eq!((rows(&old), rows(&new)), (0, 2));
    let totals: Vec<String> = connection
        .vault
        .read(Db::Intelligence, |s| {
            Ok(s.query("SELECT model FROM ai_embedding_totals", (), |r| Ok(r.get::<String>(0)?))?)
        })
        .expect("totals");
    assert_eq!(totals, [new.config.default_model.clone()]);
    let ledger_models = vault
        .read(Db::Intelligence, |s| {
            Ok(s.query("SELECT model FROM ai_index_ledger", (), |r| Ok(r.get::<String>(0)?))?)
        })
        .unwrap();
    assert_eq!(
        ledger_models,
        [new.config.default_model.clone()],
        "deleted files cannot remain a serving candidate"
    );
    drop(connection);

    let again = runtime
        .block_on(forget_unconfigured_indexes(&vault, &configured, None))
        .expect("forget again");
    assert_eq!(again, 0, "nothing left to forget");
}
