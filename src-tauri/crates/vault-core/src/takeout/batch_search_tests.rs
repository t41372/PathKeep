//! Search equivalence regressions for import-batch visibility changes.
//!
//! Responsibilities: exercise real imports, visibility mutations, both FTS planes,
//! chunk boundaries and transaction rollback against a full-rebuild oracle.
//! Not responsible for parser coverage or desktop command contracts.

use super::*;
use crate::{
    archive::{check_config_disk_consistency, ensure_archive_initialized, list_history},
    config::{project_paths_with_root, save_config},
    fault_inject::FaultGuard,
    models::{ArchiveMode, HistoryQuery},
};
use tempfile::TempDir;

struct Fixture {
    _directory: TempDir,
    paths: ProjectPaths,
    config: AppConfig,
    batch_id: i64,
}

impl Fixture {
    fn new(url_count: usize) -> Self {
        let directory = tempfile::tempdir().expect("fixture directory");
        let paths = project_paths_with_root(directory.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            ..AppConfig::default()
        };
        save_config(&paths, &config).expect("persist config");
        ensure_archive_initialized(&paths, &config, None).expect("initialize archive");
        let mut fixture = Self { _directory: directory, paths, config, batch_id: 0 };
        fixture.batch_id = fixture.import("batch", 0, url_count, 0);
        fixture
    }

    fn import(&self, name: &str, first_url: usize, count: usize, time_offset: i64) -> i64 {
        let source = self.paths.app_root.join(name);
        fs::create_dir_all(&source).expect("source directory");
        let records = (first_url..first_url + count)
            .map(|index| {
                let visited_at = chrono::DateTime::from_timestamp_millis(
                    1_775_000_000_000 + time_offset + index as i64 * 1_000,
                )
                .expect("visit timestamp")
                .to_rfc3339();
                json!({
                    "url": format!("https://batch.example/page/{index}"),
                    "title": format!("Batchneedle page {index}"),
                    "visitedAt": visited_at,
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(source.join("history.jsonl"), records).expect("import fixture");
        import_takeout(
            &self.paths,
            &self.config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("real import")
        .import_batch
        .expect("batch overview")
        .id
    }

    fn search(&self, term: &str, grouped: bool) -> Value {
        serde_json::to_value(
            list_history(
                &self.paths,
                &self.config,
                None,
                HistoryQuery {
                    q: Some(term.to_owned()),
                    limit: Some(100),
                    include_total: Some(true),
                    group_by_url: Some(grouped),
                    ..HistoryQuery::default()
                },
            )
            .expect("keyword search"),
        )
        .expect("search response")
    }

    fn assert_matches_rebuild(&self) {
        let terms = [
            "batchneedle",
            "newneedle",
            "page",
            "annotationword",
            "設計系統",
            "tagword",
            "enrichedword",
        ];
        let before = terms
            .iter()
            .flat_map(|term| [self.search(term, false), self.search(term, true)])
            .collect::<Vec<_>>();
        let documents = self.documents();
        crate::archive::rebuild_search_projection(&self.paths, &self.config, None)
            .expect("full-rebuild oracle");
        let rebuilt = terms
            .iter()
            .flat_map(|term| [self.search(term, false), self.search(term, true)])
            .collect::<Vec<_>>();
        assert_eq!(before, rebuilt, "incremental recall must equal full rebuild");
        assert_eq!(documents, self.documents(), "all mirrored fields must equal full rebuild");
        check_config_disk_consistency(&self.paths).expect("config matches canonical files");
    }

    fn documents(&self) -> Vec<Vec<Value>> {
        // Exclude only the refresh timestamp; compare every column mirrored into either FTS plane.
        let search = Connection::open(&self.paths.search_database_path).expect("search database");
        search
            .prepare(
                "SELECT url_id, url, title, search_terms, normalized_url, normalized_title,
                 normalized_search_terms, compact_text, cjk_grams, enrichment_text,
                 notes_text, tags_text FROM search_documents ORDER BY url_id",
            )
            .expect("documents")
            .query_map([], |row| {
                let mut fields = vec![Value::from(row.get::<_, i64>(0)?)];
                for index in 1..12 {
                    fields.push(Value::from(row.get::<_, String>(index)?));
                }
                Ok(fields)
            })
            .expect("read documents")
            .collect::<rusqlite::Result<_>>()
            .expect("collect documents")
    }

    fn add_search_metadata(&self) {
        let archive = open_archive_connection(&self.paths, &self.config, None).expect("archive");
        archive.execute(
            "INSERT INTO url_annotations(url, notes, created_at, updated_at)
             VALUES ('https://batch.example/page/0', 'annotationword 設計系統', '2026-01-01', '2026-01-01')",
            [],
        ).expect("annotation");
        archive
            .execute(
                "INSERT INTO url_tags(url, tag, created_at)
             VALUES ('https://batch.example/page/0', 'tagword', '2026-01-01')",
                [],
            )
            .expect("tag");
        let history_id: i64 = archive
            .query_row(
                "SELECT id FROM visits WHERE import_batch_id = ?1 ORDER BY id LIMIT 1",
                [self.batch_id],
                |row| row.get(0),
            )
            .expect("enriched visit");
        let intelligence =
            crate::archive::open_intelligence_connection(&self.paths, &self.config, None)
                .expect("intelligence");
        intelligence
            .execute(
                "INSERT INTO visit_content_enrichments
             (history_id, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
              pipeline_version, extractor_version, enrichment_summary)
             VALUES (?1, 'github-repo', 'success', '2026-06-21T00:00:00Z', '[]', '{}',
                     'v1', 1, 'enrichedword')",
                [history_id],
            )
            .expect("enrichment");
        crate::archive::rebuild_search_projection(&self.paths, &self.config, None)
            .expect("seed metadata mirrors");
    }
}

#[test]
fn revert_keeps_shared_urls_and_only_remaining_visits_searchable() {
    let fixture = Fixture::new(2);
    fixture.import("other-batch", 0, 1, 86_400_000);
    fixture.import("untouched-batch", 9, 1, 86_400_000);
    let archive = open_archive_connection(&fixture.paths, &fixture.config, None).expect("archive");
    archive.execute(
        "INSERT INTO runs(run_type, trigger, started_at, status) VALUES ('backup', 'manual', '2026-01-01', 'success')",
        [],
    ).expect("regular backup run");
    let run_id = archive.last_insert_rowid();
    archive
        .execute(
            "INSERT INTO visits(url_id, source_visit_id, visit_time_ms, visit_time_iso,
                            source_profile_id, created_by_run_id)
         SELECT url_id, 'regular-backup', visit_time_ms - 1, visit_time_iso, source_profile_id, ?1
         FROM visits WHERE import_batch_id = ?2 ORDER BY id LIMIT 1",
            params![run_id, fixture.batch_id],
        )
        .expect("regular backup visit on shared URL");
    fixture.add_search_metadata();
    let search = Connection::open(&fixture.paths.search_database_path).expect("search");
    search
        .execute(
            "UPDATE search_documents SET updated_at = 'untouched'
         WHERE url = 'https://batch.example/page/9'",
            [],
        )
        .expect("unaffected document witness");

    revert_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id).expect("revert");
    let result = fixture.search("batchneedle", true);
    assert_eq!(result["total"], 2);
    assert_eq!(result["totalVisits"], 3);
    let shared = result["items"]
        .as_array()
        .expect("results")
        .iter()
        .find(|item| item["url"] == "https://batch.example/page/0")
        .expect("shared URL remains");
    assert_eq!(shared["visitCount"], 2);
    let untouched: String = search
        .query_row(
            "SELECT updated_at FROM search_documents WHERE url = 'https://batch.example/page/9'",
            [],
            |row| row.get(0),
        )
        .expect("unaffected timestamp");
    assert_eq!(untouched, "untouched", "revert must not rebuild unrelated documents");
    fixture.assert_matches_rebuild();
}

#[test]
fn restore_makes_hidden_visits_and_metadata_searchable_again() {
    let fixture = Fixture::new(3);
    fixture.add_search_metadata();
    revert_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id).expect("revert");
    assert_eq!(fixture.search("batchneedle", false)["total"], 0);
    fixture.assert_matches_rebuild();
    restore_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id).expect("restore");
    assert_eq!(fixture.search("batchneedle", false)["total"], 3);
    for term in ["annotationword", "設計系統", "tagword", "enrichedword"] {
        assert_eq!(fixture.search(term, false)["total"], 1, "restored {term}");
    }
    fixture.assert_matches_rebuild();
}

#[test]
fn visibility_refresh_crosses_chunk_boundaries_without_skipping_urls() {
    let fixture = Fixture::new(2_003);
    let archive = open_archive_connection(&fixture.paths, &fixture.config, None).expect("archive");
    for offset in 1..=4 {
        archive
            .execute(
                "INSERT INTO visits(url_id, visit_time_ms, visit_time_iso, source_profile_id,
                                created_by_run_id, import_batch_id)
             SELECT url_id, visit_time_ms + ?1, visit_time_iso, source_profile_id,
                    created_by_run_id, import_batch_id
             FROM visits WHERE import_batch_id = ?2 ORDER BY id LIMIT 1",
                params![offset, fixture.batch_id],
            )
            .expect("same URL in a later chunk");
    }
    fixture.import("shared-across-boundary", 999, 3, 86_400_000);
    revert_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id).expect("revert");
    assert_eq!(fixture.search("batchneedle", false)["total"], 3);
    fixture.assert_matches_rebuild();
    restore_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id).expect("restore");
    assert_eq!(fixture.search("batchneedle", false)["total"], 2_010);
    fixture.assert_matches_rebuild();
}

#[test]
fn interrupted_refresh_rolls_back_all_chunks_and_reports_the_checkpoint() {
    for checkpoint in ["search.import_batch.after_chunk", "search.import_batch.before_commit"] {
        let fixture = Fixture::new(1_003);
        // Change canonical titles so a partially committed first chunk would be observable.
        let before = fixture.documents();
        let archive =
            open_archive_connection(&fixture.paths, &fixture.config, None).expect("archive");
        archive.execute("UPDATE urls SET title = 'Newneedle'", []).expect("new canonical titles");
        let fault = FaultGuard::error_at_must_fire(checkpoint);
        let error = crate::archive::refresh_search_projection_for_import_batch(
            &fixture.paths,
            &fixture.config,
            None,
            fixture.batch_id,
        )
        .expect_err("abort projection transaction");
        assert!(format!("{error:#}").contains(checkpoint), "injected error must propagate");
        drop(fault);
        assert_eq!(fixture.documents(), before, "every chunk must roll back");
        check_config_disk_consistency(&fixture.paths).expect("canonical disk consistency");
        crate::archive::refresh_search_projection_for_import_batch(
            &fixture.paths,
            &fixture.config,
            None,
            fixture.batch_id,
        )
        .expect("repeat refresh after reopening");
        assert_eq!(fixture.search("newneedle", false)["total"], 1_003);
        fixture.assert_matches_rebuild();
    }
}

#[test]
fn visibility_changes_report_refresh_failure_after_committing_canonical_state() {
    for checkpoint in ["search.import_batch.after_chunk", "search.import_batch.before_commit"] {
        let fixture = Fixture::new(1_003);
        for (restore, expected_status, expected_total, warning_code) in [
            (false, "reverted", 0, "batch-revert-projection-rebuild-needed"),
            (true, "imported", 1_003, "batch-restore-projection-rebuild-needed"),
        ] {
            let fault = FaultGuard::error_at_must_fire(checkpoint);
            let detail = if restore {
                restore_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id)
            } else {
                revert_import_batch(&fixture.paths, &fixture.config, None, fixture.batch_id)
            }
            .expect("canonical visibility change succeeds with a projection warning");
            drop(fault);
            assert_eq!(detail.batch.status, expected_status);
            let warning = detail
                .note_details
                .iter()
                .find(|note| note.code == warning_code)
                .expect("refresh failure must be reported");
            assert!(warning.diagnostic.as_deref().expect("diagnostic").contains(checkpoint));
            assert_eq!(fixture.search("batchneedle", false)["total"], expected_total);
            fixture.assert_matches_rebuild();
        }
    }
}
