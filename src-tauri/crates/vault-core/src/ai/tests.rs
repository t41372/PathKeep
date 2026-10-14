//! Regression tests for optional AI and semantic retrieval helpers.
use super::indexing::{
    IndexedVisit, clear_provider_embeddings, collect_visit_chunk, select_embed_targets,
    validate_embedding_batch_for_keys,
};
use super::search::{search_history_internal, semantic_matches};
use super::*;
use crate::models::{AiSettings, ArchiveMode};
use native::fixtures::{
    SeedVisit, prepared_archive, prepared_archive_with_key, seed_embedding,
    seed_successful_index_ledger, seed_visit, seed_visits,
};
use tokio::runtime::Runtime;

fn base_config() -> AppConfig {
    let mut llm_config = llm_provider().config;
    llm_config.api_key_saved = true;
    let mut embedding_config = embedding_provider().config;
    embedding_config.api_key_saved = true;
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        ai: AiSettings {
            enabled: true,
            assistant_enabled: true,
            semantic_index_enabled: true,
            llm_provider_id: Some("llm".to_string()),
            embedding_provider_id: Some("embed".to_string()),
            llm_providers: vec![llm_config],
            embedding_providers: vec![embedding_config],
            ..AiSettings::default()
        },
        ..AppConfig::default()
    }
}

fn embedding_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "embed".to_string(),
            name: "Embedding provider".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "text-embedding-3-small".to_string(),
            dimensions: Some(3),
            ..AiProviderConfig::default()
        },
        api_key: Some("secret".into()),
    }
}

/// The built-in in-app STATIC embedding provider runtime (F1) — routes the embed loop to the real
/// in-memory static engine (`AnyEmbeddingProvider::Static`) via the `static:in-app` sentinel.
fn static_embedding_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: crate::models::BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID.to_string(),
            name: "In-app static (local)".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            base_url: Some(STATIC_INAPP_BASE_URL.to_string()),
            default_model: DEFAULT_STATIC_MODEL_REPO.to_string(),
            dimensions: None,
            ..AiProviderConfig::default()
        },
        api_key: None,
    }
}

fn llm_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "llm".to_string(),
            name: "LLM provider".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "gpt-4.1-mini".to_string(),
            ..AiProviderConfig::default()
        },
        api_key: Some("secret".into()),
    }
}

/// Seeds the derived vector planes (W-AI-5) for a provider from `(history_id, content_key, vector)`.
///
/// Writes a fingerprint-stamped `.pkvec` store (one vector per content_key, last-writer-wins), a
/// `.pkmap` fanning each history_id to its content_key, then projects the binary + int8 planes — the
/// exact on-disk state `build_ai_index` leaves, so semantic search resolves real hits in tests.
fn seed_vector_planes(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    rows: &[(i64, u64, Vec<f32>)],
) {
    let dim = rows.first().map(|(_, _, vector)| vector.len()).unwrap_or(1);
    let fingerprint = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        dim,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(paths, &fingerprint).expect("stamp store");
    // Dedup to one vector per content_key for the store (its read_all is a set), keep full fan-out for
    // the map.
    let mut seen = std::collections::HashSet::new();
    let mut store_records = Vec::new();
    let mut map_records = Vec::new();
    for (history_id, content_key, vector) in rows {
        if seen.insert(*content_key) {
            store_records.push((*content_key, vector.clone()));
        }
        map_records.push((*history_id, *content_key));
    }
    store.append_vectors(&store_records).expect("append vectors");
    let map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    map.ensure_created(paths).expect("create map");
    map.append(&map_records).expect("append map");
    super::vector_planes::build_planes_from_store(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .expect("build planes");
    // Project the keyed reverse/forward sidecars from the `.pkmap`, stamped with the same store
    // fingerprint hash — the exact on-disk state `build_ai_index` now leaves, so the search hydration +
    // `is:starred` paths exercise the KEYED binary-search lookups (the steady-state path), not just the
    // `.pkmap`-scan fallback. (`seed_vector_planes_without_reverse_sidecars` omits this to cover the
    // older-index fallback.)
    super::reverse_visit_map::ReverseVisitMap::for_provider(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .build_from_visit_map(paths, &map, &fingerprint.hash())
    .expect("build reverse sidecars");
}

/// Seeds the vector planes + `.pkmap` but NOT the keyed sidecars (the pre-sidecar / torn-sidecar
/// on-disk state), so the search read path exercises the authoritative `.pkmap`-scan FALLBACK.
fn seed_vector_planes_without_reverse_sidecars(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    rows: &[(i64, u64, Vec<f32>)],
) {
    seed_vector_planes(paths, provider, rows);
    // Drop the just-built sidecars so the read path falls back to the `.pkmap` scan (an older index
    // built before the sidecar existed has the planes + map but no `.pkrev`/`.pkfwd`).
    super::reverse_visit_map::ReverseVisitMap::for_provider(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .delete()
    .expect("drop reverse sidecars");
}

/// Asserts the id list is a SET (the visit→content map / `.pkvec` read_all no-dup contract).
fn assert_unique(ids: &[i64]) {
    let unique: std::collections::HashSet<i64> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "ids must be unique (a set): {ids:?}");
}

#[test]
fn validate_embedding_batch_pairs_keys_and_guards_shape() {
    // Happy path: content_keys paired with vectors, effective dim = first vector length.
    let (dim, records) =
        validate_embedding_batch_for_keys(&[10, 20], &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]])
            .expect("valid batch");
    assert_eq!(dim, 3);
    assert_eq!(records, vec![(10u64, vec![0.1, 0.2, 0.3]), (20u64, vec![0.4, 0.5, 0.6])]);

    // Count mismatch.
    let count_error =
        validate_embedding_batch_for_keys(&[1, 2], &[vec![0.1]]).expect_err("count mismatch");
    assert!(count_error.to_string().contains("1 vector(s) for 2 input(s)"));

    // Empty vector.
    let empty_error = validate_embedding_batch_for_keys(&[1], &[vec![]]).expect_err("empty vector");
    assert!(empty_error.to_string().contains("empty vector"));

    // Ragged batch (second vector a different dim).
    let ragged_error = validate_embedding_batch_for_keys(&[1, 2], &[vec![0.1, 0.2], vec![0.3]])
        .expect_err("ragged batch");
    assert!(ragged_error.to_string().contains("ragged batch"));

    // F3 (0-byte honesty): an all-zero batch (right dim, no signal) is rejected, not counted.
    let zero_error = validate_embedding_batch_for_keys(&[1, 2], &[vec![0.0, 0.0], vec![0.0, 0.0]])
        .expect_err("all-zero batch");
    assert!(zero_error.to_string().contains("only zero vectors"));
    // A batch with at least one real vector still passes (a single degenerate row is tolerated).
    let (_, mixed) = validate_embedding_batch_for_keys(&[1, 2], &[vec![0.0, 0.0], vec![0.0, 1.0]])
        .expect("mixed");
    assert_eq!(mixed.len(), 2);
}

/// Builds a synthetic `IndexedVisit` for the pure dedup-selection tests (no archive needed).
fn indexed_visit(history_id: i64, content_hash: &str) -> IndexedVisit {
    IndexedVisit {
        history_id,
        profile_id: "chrome:Default".to_string(),
        url: format!("https://example.com/{history_id}"),
        title: Some("Page".to_string()),
        domain: "example.com".to_string(),
        visited_at: "2026-04-04T00:00:00Z".to_string(),
        content: format!("content-{history_id}"),
        content_key: super::dedup::content_key_from_hash(content_hash),
        content_hash: content_hash.to_string(),
        needs_embedding: true,
    }
}

#[test]
fn select_embed_targets_keeps_distinct_pages_that_collide_on_truncated_u64() {
    // Two DISTINCT pages whose content_hashes truncate to the SAME u64 content_key must BOTH
    // be embedded — the work-dedup keys on the FULL hash, never the truncated u64. Construct two hashes
    // that share the first 16 hex chars (so `content_key_from_hash` collides) but differ after.
    let hash_a = "0102030405060708aaaaaaaaaaaaaaaa";
    let hash_b = "0102030405060708bbbbbbbbbbbbbbbb";
    let a = indexed_visit(1, hash_a);
    let b = indexed_visit(2, hash_b);
    // Precondition: the two distinct hashes DO collide on the u64 key (the bug's trigger).
    assert_eq!(a.content_key, b.content_key, "test setup must produce a real u64 collision");
    assert_ne!(a.content_hash, b.content_hash);

    let changed: Vec<&IndexedVisit> = vec![&a, &b];
    let targets = select_embed_targets(
        &changed,
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );
    // BOTH pages are selected for embedding (the second is NOT collapsed onto the first's vector).
    assert_eq!(targets.len(), 2, "a u64 collision must NOT drop the second distinct page's embed");
    let target_hashes: Vec<&str> = targets.iter().map(|v| v.content_hash.as_str()).collect();
    assert!(target_hashes.contains(&hash_a));
    assert!(target_hashes.contains(&hash_b));

    // The SAME page seen twice (same full hash) IS deduped to one embed (the intended dedup).
    let a_again = indexed_visit(3, hash_a);
    let same_page: Vec<&IndexedVisit> = vec![&a, &a_again];
    let one = select_embed_targets(
        &same_page,
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );
    assert_eq!(one.len(), 1, "two visits of one page (same full hash) → one embed");

    // A page whose FULL hash is already persisted this run is skipped (no re-embed).
    let persisted_hashes = std::collections::HashSet::from([hash_a.to_string()]);
    let skip = select_embed_targets(&changed, &persisted_hashes, &std::collections::HashSet::new());
    assert_eq!(skip.len(), 1, "the already-embedded full hash is skipped, the other still embeds");
    assert_eq!(skip[0].content_hash, hash_b);

    // The u64 storage-boundary backstop still skips a key already on the `.pkvec` plane (resume).
    let persisted_keys = std::collections::HashSet::from([a.content_key]);
    let backstop = select_embed_targets(&[&a], &std::collections::HashSet::new(), &persisted_keys);
    assert!(backstop.is_empty(), "a u64 key already on disk is the resume backstop skip");
}

#[test]
fn semantic_matches_falls_back_to_pkmap_scan_when_reverse_sidecars_absent() {
    // Read-path guard: an OLDER index (planes + `.pkmap` present, but no `.pkrev`/`.pkfwd`) must
    // still resolve the SAME hits via the authoritative `.pkmap` scan fallback — no wrong/empty result.
    // This exercises the `reverse_sidecars_usable == false` branch + `VisitContentMap` scan path.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    let other_vector: Vec<f32> = docs_vector.iter().rev().map(|value| -value).collect();
    // Seed the planes + `.pkmap` but DROP the keyed sidecars (the pre-sidecar on-disk shape).
    seed_vector_planes_without_reverse_sidecars(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x2222, other_vector)],
    );
    seed_embedding(&connection, 1, &embedding, "hash-1");
    seed_successful_index_ledger(&connection, &embedding, 1);

    let report = runtime
        .block_on(semantic_matches(
            &vault,
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
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
            None,
        ))
        .expect("semantic matches via fallback");
    assert!(!report.hits.is_empty(), "the `.pkmap`-scan fallback must still return the real hit");
    assert_eq!(report.hits[0].history_id, 1, "fallback resolves the same most-recent visit");
    assert!(report.hits[0].url.contains("/docs"));
}

#[test]
fn semantic_matches_rejects_same_dim_fingerprint_drift_with_an_honest_note() {
    // D1: the planes were built at the SAME dim as the live query (3) but under a DIFFERENT fingerprint
    // (Mean pooling), while the live External engine reports Unknown pooling. The bytes line up but the
    // geometry differs, so scores would still be meaningless. `planes_are_stale` must catch the drift
    // and the guard must degrade to lexical-only with a rebuild note.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    // Stamp the source store (and thus the planes) with a Mean-pooled fingerprint — same dim, different
    // hash than the live External (Unknown-pooled) descriptor the guard rebuilds.
    let drifted = EmbeddingFingerprint::new(
        embedding.config.id.clone(),
        embedding.config.default_model.clone(),
        docs_vector.len(),
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Mean,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &drifted).expect("stamp drifted store");
    store.append_vectors(&[(0x1111, docs_vector.clone())]).expect("append");
    let map = VisitContentMap::for_provider(
        &paths,
        &embedding.config.id,
        &embedding.config.default_model,
    );
    map.ensure_created(&paths).expect("create map");
    map.append(&[(1, 0x1111)]).expect("append map");
    super::vector_planes::build_planes_from_store(
        &paths,
        &embedding.config.id,
        &embedding.config.default_model,
    )
    .expect("build planes");

    let report = runtime
        .block_on(semantic_matches(
            &vault,
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
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
            None,
        ))
        .expect("semantic matches under fingerprint drift");
    assert!(report.hits.is_empty(), "a fingerprint drift must yield NO semantic hits");
    assert!(
        report.notes.iter().any(|note| matches!(note, AiSearchNote::ConfigDriftFingerprint)),
        "the fingerprint-drift note CODE must surface: {:?}",
        report.notes
    );
}

#[test]
fn lexical_ai_cursors_reach_beyond_every_former_pool_and_date_window() {
    let (paths, config, connection) = prepared_archive_with_key(Some([94; 32]));
    let vault = connection.vault.clone();
    seed_visits(
        &vault,
        (1..=1107)
            .map(|id| {
                SeedVisit::chrome(
                    id,
                    "chrome:Default",
                    &format!("https://all.test/{id}"),
                    Some("completepool"),
                    13_300_000_000_000_000 + id * 1000,
                )
            })
            .collect(),
    );
    let runtime = Runtime::new().unwrap();
    for sort in [None, Some("oldest".to_owned()), Some("newest".to_owned())] {
        let mut request = AiSearchRequest {
            query: "completepool".into(),
            limit: Some(73),
            sort,
            ..Default::default()
        };
        let mut ids = Vec::new();
        loop {
            let response = runtime
                .block_on(search_history_internal(&vault, &paths, &config, None, None, &request))
                .unwrap();
            assert_eq!(response.total, 1107);
            ids.extend(response.items.iter().map(|item| item.history_id));
            if response.next_cursor.is_none() {
                break;
            }
            request.cursor = response.next_cursor;
        }
        ids.sort_unstable();
        assert_eq!(ids, (1..=1107).collect::<Vec<_>>());
    }
}

#[test]
fn semantic_facets_find_eligible_pages_beyond_every_former_recall_pool() {
    let (paths, config, connection) = prepared_archive_with_key(Some([108; 32]));
    let vault = connection.vault.clone();
    let provider = embedding_provider();
    let runtime = Runtime::new().unwrap();
    let vector =
        runtime.block_on(embed_query(&provider, "candidatephrase", EmbeddingRole::Query)).unwrap();
    let mut visits = Vec::new();
    let mut vectors = Vec::new();
    for id in 1..=1207 {
        visits.push(SeedVisit::chrome(
            id,
            if id == 1207 { "firefox:target" } else { "chrome:Default" },
            &format!("https://scope.test/page/{id}"),
            Some("Unrelated"),
            13_300_000_000_000_000 + id * 1000,
        ));
        vectors.push((
            id,
            id as u64,
            if id == 1207 { vector.iter().map(|value| -value).collect() } else { vector.clone() },
        ));
    }
    seed_visits(&vault, visits);
    seed_vector_planes(&paths, &provider, &vectors);
    let response = runtime
        .block_on(search_history_internal(
            &vault,
            &paths,
            &config,
            None,
            Some(&provider),
            &AiSearchRequest {
                query: "candidatephrase".into(),
                profile_id: Some("firefox:target".into()),
                limit: Some(1),
                ..Default::default()
            },
        ))
        .unwrap();
    assert_eq!(response.total, 1);
    assert_eq!(response.items[0].history_id, 1207);
}

#[path = "tests_native/mod.rs"]
pub(in crate::ai) mod native;
