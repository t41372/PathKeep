//! Chromium sidecar ingest scenarios (T6-T9).
//!
//! These tests drive the real `process_profile_snapshot` pipeline against
//! synthetic Chromium `History` + `Favicons` databases produced by the
//! `browser-history-fixtures` crate.
//!
//! ## Responsibilities
//! - Prove downloads, keyword search terms, favicons, and icon mappings move
//!   from generated Chromium fixtures into the canonical archive tables.
//! - Pin the column-level behavior named in `WORK-IMPORT-FIXTURE-SIDECARS-A`
//!   without reading any real browser data.
//!
//! ## Not responsible for
//! - Parser unit coverage; `browser-history-fixtures/tests/chromium_roundtrip.rs`
//!   self-validates the fixture writer against the real parser.
//! - Fixing product bugs exposed by these scenarios.

use super::*;
use browser_history_fixtures::{
    ChromiumDownloadRow, ChromiumFaviconRow, ChromiumHistoryFixture, ChromiumIconMappingRow,
    ChromiumKeywordSearchTermRow, ChromiumUrlRow, ChromiumVisitRow,
};
use rusqlite::Connection;
use tempfile::{TempDir, tempdir};

fn test_config() -> AppConfig {
    AppConfig { initialized: true, ..AppConfig::default() }
}

fn test_paths(root: &Path) -> ProjectPaths {
    crate::config::project_paths_with_root(root)
}

fn chromium_profile(profile_id: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Google Chrome".to_string(),
        user_name: Some("synthetic-user".to_string()),
        profile_path: format!("/synthetic/{profile_id}"),
        history_path: Some(format!("/synthetic/{profile_id}/History")),
        favicons_path: Some(format!("/synthetic/{profile_id}/Favicons")),
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("146.0.0.0".to_string()),
        history_file_name: "History".to_string(),
        history_bytes: 128,
        favicons_bytes: 64,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    }
}

fn seed_run(archive: &Transaction<'_>, run_id: i64) {
    archive
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (?1, 'backup', 'manual', '2026-05-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
            [run_id],
        )
        .expect("seed run");
}

struct ScenarioEnv {
    _root: TempDir,
    paths: ProjectPaths,
    config: AppConfig,
}

impl ScenarioEnv {
    fn new() -> Self {
        let root = tempdir().expect("scenario root tempdir");
        let paths = test_paths(root.path());
        let config = test_config();
        crate::config::ensure_paths(&paths).expect("ensure paths");
        Self { _root: root, paths, config }
    }

    fn open_archive(&self) -> Connection {
        open_archive_connection(&self.paths, &self.config, None).expect("open archive")
    }
}

fn snapshot_for_fixture(fixture: &ChromiumHistoryFixture) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("snapshot tempdir");
    let history_path = temp_dir.path().join("History");
    let favicons_path = temp_dir.path().join("Favicons");
    fixture.write(&history_path).expect("write History fixture");
    fixture.write_favicons(&favicons_path).expect("write Favicons fixture");

    let mut profile = chromium_profile("chrome:Default");
    profile.history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    profile.favicons_bytes = std::fs::metadata(&favicons_path).map(|meta| meta.len()).unwrap_or(0);

    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: Some(favicons_path),
        source_hashes: vec![
            FileFingerprint {
                path: "History".to_string(),
                sha256: "synthetic-history-hash".to_string(),
            },
            FileFingerprint {
                path: "Favicons".to_string(),
                sha256: "synthetic-favicons-hash".to_string(),
            },
        ],
    }
}

fn run_one_ingest(
    env: &ScenarioEnv,
    run_id: i64,
    snapshot: &ProfileSnapshot,
) -> BackupProfileSummary {
    let mut archive = env.open_archive();
    let transaction = archive.transaction().expect("scenario transaction");
    seed_run(&transaction, run_id);
    let mut snapshot_artifacts = Vec::new();
    let mut source_evidence_plans = Vec::new();
    let summary = process_profile_snapshot(
        &transaction,
        run_id,
        &env.paths,
        &env.config,
        snapshot,
        &mut snapshot_artifacts,
        &mut source_evidence_plans,
        false,
        false,
    )
    .expect("process profile snapshot");
    transaction.commit().expect("commit scenario transaction");
    summary
}

fn count_archive_rows(env: &ScenarioEnv, table: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .expect("count rows")
}

fn base_fixture() -> ChromiumHistoryFixture {
    let visit_ms = 1_777_680_000_000_i64;
    ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/article".to_string(),
            title: Some("Article".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_ms,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 10,
            url_id: 1,
            visit_time_unix_ms: visit_ms,
            from_visit: None,
            transition: Some(805_306_368),
            visit_duration_micros: Some(5_000_000),
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        })
}

fn png_bytes(seed: u8) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.push(seed);
    bytes
}

#[test]
fn t6_chromium_downloads_round_trip_to_archive_downloads_table() {
    let env = ScenarioEnv::new();
    let download_ms = 1_777_680_010_000_i64;
    let fixture = base_fixture().add_download(ChromiumDownloadRow {
        id: 77,
        guid: Some("download-guid-77".to_string()),
        current_path: Some("/tmp/pathkeep.partial".to_string()),
        target_path: Some("/tmp/pathkeep.zip".to_string()),
        start_time_unix_ms: Some(download_ms),
        received_bytes: Some(128),
        total_bytes: Some(256),
        state: Some(1),
        mime_type: Some("application/zip".to_string()),
        original_mime_type: Some("application/octet-stream".to_string()),
    });
    let snapshot = snapshot_for_fixture(&fixture);

    let summary = run_one_ingest(&env, 1, &snapshot);

    assert_eq!(summary.new_downloads, 1, "summary should report the new download row");
    let archive = env.open_archive();
    let row = archive
        .query_row(
            "SELECT source_download_id, guid, current_path, target_path, start_time_ms,
                    received_bytes, total_bytes, state, mime_type, original_mime_type
             FROM downloads",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .expect("download row");

    assert_eq!(row.0, "77");
    assert_eq!(row.1.as_deref(), Some("download-guid-77"));
    assert_eq!(row.2.as_deref(), Some("/tmp/pathkeep.partial"));
    assert_eq!(row.3.as_deref(), Some("/tmp/pathkeep.zip"));
    assert_eq!(row.4, Some(download_ms));
    assert_eq!(row.5, Some(128));
    assert_eq!(row.6, Some(256));
    assert_eq!(row.7, Some(1));
    assert_eq!(row.8.as_deref(), Some("application/zip"));
    assert_eq!(row.9.as_deref(), Some("application/octet-stream"));
}

#[test]
fn t7_chromium_keyword_search_terms_land_with_term_text_preserved() {
    let env = ScenarioEnv::new();
    let fixture = base_fixture().add_search_term(ChromiumKeywordSearchTermRow {
        keyword_id: 3,
        url_id: 1,
        term: "PathKeep fixtures".to_string(),
        normalized_term: "pathkeep fixtures".to_string(),
    });
    let snapshot = snapshot_for_fixture(&fixture);

    run_one_ingest(&env, 1, &snapshot);

    let archive = env.open_archive();
    let row = archive
        .query_row(
            "SELECT term, normalized_term, profile_id, keyword_id FROM search_terms",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .expect("search term row");
    assert_eq!(
        row,
        (
            "PathKeep fixtures".to_string(),
            "pathkeep fixtures".to_string(),
            "chrome:Default".to_string(),
            3
        )
    );
}

#[test]
fn t8_chromium_favicons_link_to_canonical_url_rows_with_blob_dedup() {
    let env = ScenarioEnv::new();
    let image = png_bytes(42);
    let fixture = base_fixture()
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://synthetic.test/second".to_string(),
            title: Some("Second".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: 1_777_680_001_000,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 11,
            url_id: 2,
            visit_time_unix_ms: 1_777_680_001_000,
            from_visit: None,
            transition: Some(805_306_368),
            visit_duration_micros: Some(1_000),
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        })
        .add_favicon(ChromiumFaviconRow {
            id: 20,
            icon_url: "https://example.com/favicon.ico".to_string(),
            icon_type: Some(1),
            width: 32,
            height: 32,
            last_updated_unix_ms: 1_777_680_020_000,
            image_data: Some(image.clone()),
        })
        .add_favicon(ChromiumFaviconRow {
            id: 21,
            icon_url: "https://synthetic.test/favicon.ico".to_string(),
            icon_type: Some(1),
            width: 16,
            height: 16,
            last_updated_unix_ms: 1_777_680_021_000,
            image_data: Some(image.clone()),
        })
        .add_icon_mapping(ChromiumIconMappingRow {
            page_url: "https://example.com/article".to_string(),
            icon_id: 20,
        })
        .add_icon_mapping(ChromiumIconMappingRow {
            page_url: "https://synthetic.test/second".to_string(),
            icon_id: 21,
        });
    let snapshot = snapshot_for_fixture(&fixture);

    run_one_ingest(&env, 1, &snapshot);

    let archive = env.open_archive();
    let linked_count: i64 = archive
        .query_row(
            "SELECT COUNT(*)
             FROM favicons
             JOIN urls ON urls.url = favicons.page_url
              AND urls.source_profile_id = favicons.source_profile_id",
            [],
            |row| row.get(0),
        )
        .expect("linked favicon count");
    let blob_count = count_archive_rows(&env, "favicon_blobs");
    let distinct_hash_count: i64 = archive
        .query_row("SELECT COUNT(DISTINCT image_blob_hash) FROM favicons", [], |row| row.get(0))
        .expect("distinct favicon hashes");
    let blob_bytes: Vec<u8> = archive
        .query_row("SELECT image_data FROM favicon_blobs", [], |row| row.get(0))
        .expect("favicon blob bytes");

    assert_eq!(count_archive_rows(&env, "favicons"), 2);
    assert_eq!(linked_count, 2, "favicon page URLs should match canonical URL rows");
    assert_eq!(blob_count, 1, "identical favicon payload bytes should deduplicate");
    assert_eq!(distinct_hash_count, 1);
    assert_eq!(blob_bytes, image);
}

#[test]
fn t9_chromium_icon_mapping_resolves_url_to_favicon() {
    let env = ScenarioEnv::new();
    let fixture = base_fixture()
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/settings".to_string(),
            title: Some("Settings".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: 1_777_680_002_000,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 11,
            url_id: 2,
            visit_time_unix_ms: 1_777_680_002_000,
            from_visit: None,
            transition: Some(805_306_368),
            visit_duration_micros: Some(1_000),
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        })
        .add_favicon(ChromiumFaviconRow {
            id: 30,
            icon_url: "https://cdn.synthetic.test/shared.ico".to_string(),
            icon_type: Some(1),
            width: 48,
            height: 48,
            last_updated_unix_ms: 1_777_680_030_000,
            image_data: Some(png_bytes(7)),
        })
        .add_icon_mapping(ChromiumIconMappingRow {
            page_url: "https://example.com/article".to_string(),
            icon_id: 30,
        })
        .add_icon_mapping(ChromiumIconMappingRow {
            page_url: "https://example.com/settings".to_string(),
            icon_id: 30,
        });
    let snapshot = snapshot_for_fixture(&fixture);

    run_one_ingest(&env, 1, &snapshot);

    let archive = env.open_archive();
    let mut statement = archive
        .prepare(
            "SELECT page_url
             FROM favicons
             WHERE icon_url = 'https://cdn.synthetic.test/shared.ico'
             ORDER BY page_url ASC",
        )
        .expect("prepare favicon mapping query");
    let page_urls = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query favicon mappings")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect mapped page urls");

    assert_eq!(
        page_urls,
        vec!["https://example.com/article".to_string(), "https://example.com/settings".to_string()]
    );
}
