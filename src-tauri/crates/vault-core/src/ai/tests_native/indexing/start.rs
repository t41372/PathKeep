//! Responsibilities: what a build reads before its first embedding request.
//! Not responsible for: the embed loop, resuming or rebuilding (their own groups).
//! Failure modes (2026-11-13: a build over an index of a few hundred thousand visits never
//! reached its first request. The statement that counts what is left to index walked the whole index once per
//! visit, and it was one statement, so it could not be stopped and held up every other read):
//! - a statement that reads visits against the index walks the index, or the visits, whole;
//! - the count is wrong: a visit taken back, or a row of another model, changes it;
//! - a stop asked for during the count is not seen until the count ends.
use super::*;
use crate::ai::indexing::{COUNT_PAGE_IDS, COUNT_STOPPED};
use fixtures::{SeedVisit, seed_visits};
use std::sync::atomic::{AtomicUsize, Ordering};

fn pages(ids: impl IntoIterator<Item = i64>) -> Vec<SeedVisit> {
    ids.into_iter()
        .map(|id| {
            SeedVisit::chrome(
                id,
                "chrome:Default",
                &format!("https://example.com/page/{id}"),
                Some("Page"),
                id,
            )
        })
        .collect()
}

#[test]
fn preparation_uses_short_keyset_reads_without_per_visit_lineage_scans() {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone().with_config(config);
    let provider = embedding_provider();
    seed_visits(&vault, pages(1..=40));
    runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).expect("first");
    seed_visits(&vault, pages(41..=43));

    vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
    vault.store().capture_query_plans(Db::Archive, true).unwrap();
    let report = runtime
        .block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default()))
        .expect("second build");
    vault.store().capture_query_plans(Db::Intelligence, false).unwrap();
    vault.store().capture_query_plans(Db::Archive, false).unwrap();
    assert_eq!(report.indexed_items, 3);

    let joins: Vec<_> = vault
        .store()
        .take_query_plans(Db::Intelligence)
        .unwrap()
        .into_iter()
        .chain(vault.store().take_query_plans(Db::Archive).unwrap())
        .filter(|plan| {
            (plan.sql.contains("archive.visits") && plan.sql.contains("ai_embeddings"))
                || plan.sql.starts_with("SELECT id FROM visits WHERE id>=")
        })
        .collect();
    assert!(
        joins.iter().any(|plan| plan.sql.starts_with("SELECT id FROM visits WHERE id>=")),
        "the build must measure through bounded primary-key reads: {joins:#?}"
    );
    for plan in &joins {
        for step in &plan.steps {
            // Every row of one index shares these three values, so a search held by them alone
            // reads all of it. (Cleanup also bounds `id`, and is fine.)
            assert!(
                !step.ends_with("(provider_id=? AND model=? AND lineage=?)"),
                "walks the whole index for each visit: {plan:#?}"
            );
            assert!(
                !step.starts_with("SCAN visits"),
                "reads every visit in one statement: {plan:#?}"
            );
        }
    }
}

#[test]
fn checked_visit_counts_include_unchanged_other_model_and_hidden_rows() {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone().with_config(config.clone());
    let archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visits(&vault, pages(1..=6));
    let (indexed, first) =
        run_backfill_chunk(&runtime, &vault, &config, &provider, &AiIndexRequest::default(), 0);
    assert_eq!((indexed.len(), first.scan.unwrap().total), (6, 6));

    // Left to index: 3 lost its row, 7 has a row of another model only, 8 is new.
    // Not left: 9 was taken back.
    seed_visits(&vault, pages(7..=9));
    connection.execute("DELETE FROM ai_embeddings WHERE history_id=3", ()).unwrap();
    let mut other = embedding_provider();
    other.config.default_model = "another-embedding-model".to_string();
    let lineage = source_paths(&vault, &provider)
        .vectors_dir
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let row = IndexedVisit {
        history_id: 7,
        profile_id: "chrome:Default".into(),
        url: "https://example.com/page/7".into(),
        title: Some("Page".into()),
        domain: "example.com".into(),
        visited_at: "2026-04-04T00:00:00Z".into(),
        content: "content".into(),
        content_hash: "other-model".into(),
        content_key: 0,
        needs_embedding: true,
    };
    let other_model = other.clone();
    connection
        .write(move |s| {
            crate::ai::indexing::upsert_embedding(
                s,
                &other_model,
                &row,
                "2026-04-04T00:00:00Z",
                &lineage,
            )
        })
        .unwrap();
    archive
        .execute(
            "UPDATE visits SET reverted_at='2026-04-05T00:00:00Z', reverted_by_run_id=1 WHERE id=9",
            (),
        )
        .unwrap();

    let (_, second) =
        run_backfill_chunk(&runtime, &vault, &config, &provider, &AiIndexRequest::default(), 0);
    let scan = second.scan.unwrap();
    assert_eq!((scan.examined, scan.total), (9, 9));
}

/// Lets the count read its first page, then asks the build to stop.
struct StopAfterTheFirstPage(AtomicUsize);
impl AiRunControl for StopAfterTheFirstPage {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        if detail == COUNT_STOPPED && self.0.fetch_add(1, Ordering::SeqCst) == 1 {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }
}

#[test]
fn a_stop_asked_for_during_the_count_ends_the_build_before_the_count_does() {
    let runtime = Runtime::new().expect("runtime");
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone().with_config(config);
    let provider = embedding_provider();
    // Three actual pages of ids, independent of gaps in their numeric values.
    seed_visits(&vault, pages(1..=3 * COUNT_PAGE_IDS));
    let control = Arc::new(StopAfterTheFirstPage(AtomicUsize::new(0)));
    let stop: Arc<dyn AiRunControl> = control.clone();
    let error = runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &AiIndexRequest::default(),
            Some(stop),
            0,
            0,
            None,
        ))
        .expect_err("the build went on after it was asked to stop");
    assert!(error.downcast_ref::<AiRunCancelled>().is_some(), "{error:#}");
    assert_eq!(control.0.load(Ordering::SeqCst), 2, "the count read on after the stop");
    assert_eq!(
        embedding_count(&connection, &provider.config.id, &provider.config.default_model).unwrap(),
        0
    );
}
