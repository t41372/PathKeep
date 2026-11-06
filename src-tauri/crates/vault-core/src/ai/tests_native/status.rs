//! Responsibilities: native AI status regression coverage.
//! Not responsible for: search or network transport.
//! Failure modes: omitted records, stale state, non-durable progress, and incorrect provider scope.
use super::*;

#[test]
fn ai_index_status_flags_metadata_rows_with_an_empty_vector_store() {
    // F3 (0-byte honesty): SQLite `ai_embeddings` rows exist (a build counted rows) but NO vectors
    // landed on the `.pkvec` plane. The status must NOT report "ready, N indexed" — it surfaces the
    // honest `IndexVectorsMissing` warning, a degraded state, and the real (zero) vector count.
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ghost", Some("Ghost"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ghost");
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    VectorStore::create_stamped(vault.paths(), &fp).unwrap();
    migrate_fixture(&vault, &provider);
    // Deliberately do NOT seed vector planes — the dishonest "indexed N with an empty sidecar" case.

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(!status.ready, "metadata rows without vectors are NOT ready");
    assert_eq!(status.state, "degraded");
    assert_eq!(status.indexed_items, 1, "the SQLite metadata count is still reported honestly");
    assert_eq!(status.semantic_vector_count, 0, "the REAL vector count is zero");
    assert_eq!(status.warning_code, Some(AiIndexWarning::IndexVectorsMissing));
}
