//! Responsibilities: resume behavior regressions.
//! Not responsible for: other test groups or production code.
//! Failure modes: wrong state, missing records and replay inconsistency.
use super::*;

#[test]
fn resume_after_dropped_cursor_does_not_duplicate_vectors() {
    // A crash AFTER a chunk's vectors are appended but BEFORE its SQLite hash rows /
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
    let source = source_paths(&vault, &provider);
    let lineage = source.vectors_dir.file_name().unwrap().to_str().unwrap();
    crate::ai::indexing::store_rows::remove_lineage_rows(
        &vault,
        &provider.config.id,
        &provider.config.default_model,
        lineage,
    )
    .expect("drop metadata");
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
