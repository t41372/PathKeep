//! Failure modes: missing URL parents, duplicate counters, incorrect raw cursors,
//! missing checkpoint provenance and interrupted batches. Tests exercise real Store jobs.
//! Core ingest orchestration regression tests.
//!
//! ## Responsibilities
//! - Pin helper and orchestration behavior owned by `ingest::mod`.
//! - Keep large regression scaffolding out of the production facade file.
//!
//! ## Not responsible for
//! - Broad browser-family scenario matrices owned by sibling scenario modules.
//! - Changing ingest semantics while moving test ownership.
//!
//! ## Dependencies
//! - Uses private parent-module helpers through `use super::*`.
//! - Uses synthetic temp archives and parser DTO fixtures only.
//!
//! ## Performance notes
//! - Tests use tiny temp archives and do not generate large history corpora.

use super::*;
use super::{
    selection::{favicon_marker, url_marker},
    writes::{track_url_visit_bounds, upsert_source_profile},
};
use crate::test_support::store::Probe;
use browser_history_parser::{
    CapabilitySnapshot, ContextEvidence, EngagementEvidence, NativeEntity, NavigationEvidence,
    ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit, SchemaObservation, SearchEvidence,
    TypedEvidenceBatch,
};
use browser_history_parser::{HistoryBatchConsumer, ParsedDownload};
use rusqlite::Connection;
use std::collections::HashMap;
use tempfile::tempdir;
use vault_store::{Db, Priority, Session};

pub(super) fn profile(family: &str, profile_id: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: family.to_string(),
        browser_name: match family {
            "firefox" => "Firefox".to_string(),
            "safari" => "Safari".to_string(),
            _ => "Chrome".to_string(),
        },
        user_name: Some("tim@example.com".to_string()),
        profile_path: format!("/tmp/{profile_id}"),
        history_path: Some(format!("/tmp/{profile_id}/History")),
        favicons_path: Some(format!("/tmp/{profile_id}/Favicons")),
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

fn parsed_url(source_url_id: i64, last_visit_ms: i64) -> ParsedUrl {
    ParsedUrl {
        source_url_id,
        url: "https://example.com/archive".to_string(),
        title: Some("Archive docs".to_string()),
        visit_count: 2,
        typed_count: 1,
        last_visit_ms,
        last_visit_iso: "2026-04-05T11:00:00+00:00".to_string(),
        hidden: false,
        source_last_visit_marker: None,
    }
}

fn parsed_visit(source_visit_id: i64, source_url_id: i64, visit_time_ms: i64) -> ParsedVisit {
    ParsedVisit {
        source_visit_id,
        source_url_id,
        url: "https://example.com/archive".to_string(),
        title: Some("Archive docs".to_string()),
        visit_time_ms,
        visit_time_iso: format!("2026-04-05T11:00:0{source_visit_id}+00:00"),
        from_visit: (source_visit_id > 1).then_some(source_visit_id - 1),
        transition: Some(805306368),
        visit_duration_ms: Some(1_000),
        is_known_to_sync: true,
        visited_link_id: Some(source_visit_id + 100),
        external_referrer_url: Some("https://referrer.example".to_string()),
        app_id: None,
    }
}

fn parsed_download(source_download_id: i64) -> ParsedDownload {
    ParsedDownload {
        source_download_id,
        guid: Some(format!("guid-{source_download_id}")),
        current_path: Some("/tmp/archive.pdf".to_string()),
        target_path: Some("/tmp/archive.pdf".to_string()),
        start_time_ms: Some(2_000),
        start_time_iso: Some("2026-04-05T11:00:00+00:00".to_string()),
        received_bytes: Some(10),
        total_bytes: Some(10),
        state: Some(1),
        mime_type: Some("application/pdf".to_string()),
        original_mime_type: Some("application/pdf".to_string()),
    }
}

fn parsed_favicon(last_updated_ms: i64) -> ParsedFavicon {
    ParsedFavicon {
        page_url: "https://example.com/archive".to_string(),
        icon_url: "https://example.com/favicon.ico".to_string(),
        icon_type: Some(1),
        width: 16,
        height: 16,
        last_updated_ms,
        last_updated_iso: "2026-04-05T11:00:00+00:00".to_string(),
        image_data: Some(vec![0x89, 0x50, 0x4e, 0x47]),
    }
}

fn snapshot_with_history(
    temp_dir: tempfile::TempDir,
    profile: crate::models::BrowserProfile,
) -> ProfileSnapshot {
    let history_path = temp_dir.path().join("History");
    Connection::open(&history_path)
        .expect("history db")
        .execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY);
             CREATE TABLE visits (id INTEGER PRIMARY KEY);",
        )
        .expect("minimal history schema");
    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "History".to_string(),
            sha256: "hash-history".to_string(),
        }],
    }
}

fn seed_run(archive: &Session<'_>, run_id: i64) {
    archive
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (?1, 'backup', 'manual', '2026-04-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
                [run_id],
            )
            .expect("seed run");
}

#[test]
fn chunk_consumer_persists_batches_and_reports_duplicate_progress() {
    let vault = crate::Vault::open_temp().unwrap();
    let archive = Probe::new(&vault);
    let profile = profile("chromium", "chrome:Default");
    let (context, source_batch) = seed_context(&vault, 42, &profile);
    let cancel = CancelToken::new();
    let mut progress_events = Vec::new();

    {
        let mut consumer = ArchiveChunkConsumer::new(
            &vault,
            context,
            Watermark::default(),
            &cancel,
            Some(Box::new(|progress| progress_events.push(progress))),
            source_batch,
        );
        consumer.urls(vec![parsed_url(1, 2_000)]).expect("urls");
        consumer
            .visits(vec![
                parsed_visit(90, 999, 500),
                parsed_visit(1, 1, 1_000),
                parsed_visit(1, 1, 1_000),
                parsed_visit(2, 1, 2_000),
            ])
            .expect("visits");
        consumer.downloads(vec![parsed_download(9), parsed_download(9)]).expect("downloads");
        consumer
            .search_terms(vec![
                ParsedSearchTerm {
                    url_last_visit_time: 0,
                    keyword_id: 1,
                    url_id: 404,
                    term: "ignored".to_string(),
                    normalized_term: "ignored".to_string(),
                },
                ParsedSearchTerm {
                    url_last_visit_time: 0,
                    keyword_id: 2,
                    url_id: 1,
                    term: "deep recall".to_string(),
                    normalized_term: "deep recall".to_string(),
                },
            ])
            .expect("search terms");
        consumer.favicons(vec![parsed_favicon(2_000)]).expect("favicons");
        let progress = consumer.finish().expect("finish");
        assert_eq!(progress.new_urls, 1);
        assert_eq!(progress.new_visits, 2);
        assert_eq!(progress.new_downloads, 1);
        assert_eq!(progress.skipped_visits, 1);
        assert_eq!(progress.inserted_search_terms, 1);
        assert_eq!(progress.last_visit_id, 2);
        assert!(progress.last_url_marker.is_some_and(|marker| marker > 2_000));
        assert!(progress.last_download_id.is_some_and(|id| id == 9));
        assert!(progress.last_favicon_marker.is_some_and(|marker| marker > 2_000));
    }

    assert_eq!(progress_events.len(), 1);
    assert_eq!(progress_events[0].processed_records, 4);
    assert_eq!(progress_events[0].imported_records, 2);
    assert_eq!(progress_events[0].duplicate_records, 1);
    assert_eq!(progress_events[0].skipped_records, 1);

    let url_bounds: (i64, i64) = archive
        .query_row(
            "SELECT first_visit_ms, last_visit_ms FROM urls WHERE source_url_id = 1",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("url bounds");
    assert_eq!(url_bounds, (1_000, 2_000));
    let visit_count: i64 = archive
        .query_row("SELECT COUNT(*) FROM visits", (), |row| Ok(row.get(0)?))
        .expect("visit count");
    let download_count: i64 = archive
        .query_row("SELECT COUNT(*) FROM downloads", (), |row| Ok(row.get(0)?))
        .expect("download count");
    let search_count: i64 = archive
        .query_row("SELECT COUNT(*) FROM search_terms", (), |row| Ok(row.get(0)?))
        .expect("search count");
    let favicon_count: i64 = archive
        .query_row("SELECT COUNT(*) FROM favicons", (), |row| Ok(row.get(0)?))
        .expect("favicon count");
    assert_eq!(visit_count, 2);
    assert_eq!(download_count, 1);
    assert_eq!(search_count, 1);
    assert_eq!(favicon_count, 1);
}

/// Pins `import-dedup-audit.md` section 4's Visit-to-URL ordering dependency:
/// parser streams must emit URL batches before visits. Current ingest
/// behavior is intentionally silent skip, not fail-fast, when a visit
/// arrives before its source URL has populated `url_id_map`.
#[test]
fn chunk_consumer_skips_visits_when_url_batch_has_not_populated_the_map() {
    let vault = crate::Vault::open_temp().unwrap();
    let archive = Probe::new(&vault);
    let profile = profile("chromium", "chrome:VisitBeforeUrl");
    let (context, source_batch) = seed_context(&vault, 43, &profile);
    let cancel = CancelToken::new();
    let mut progress_events = Vec::new();

    {
        let mut consumer = ArchiveChunkConsumer::new(
            &vault,
            context,
            Watermark::default(),
            &cancel,
            Some(Box::new(|progress| progress_events.push(progress))),
            source_batch,
        );
        consumer.visits(vec![parsed_visit(1, 99, 1_000)]).expect("visits");
        let progress = consumer.finish().expect("finish");
        assert_eq!(progress.new_visits, 0);
        assert_eq!(progress.skipped_visits, 1);
        assert_eq!(
            progress.last_visit_id, 0,
            "skipped visits must not advance the visit watermark marker"
        );
    }

    assert_eq!(progress_events.len(), 1);
    assert_eq!(progress_events[0].processed_records, 1);
    assert_eq!(progress_events[0].imported_records, 0);
    assert_eq!(progress_events[0].duplicate_records, 0);
    assert_eq!(progress_events[0].skipped_records, 1);
    let visit_count: i64 = archive
        .query_row("SELECT COUNT(*) FROM visits", (), |row| Ok(row.get(0)?))
        .expect("visit count");
    assert_eq!(visit_count, 0);
}

#[test]
fn skipped_profile_and_marker_helpers_cover_family_edges() {
    let readable = profile("chromium", "chrome:Default");
    let mut missing_chrome = profile("chromium", "chrome:Missing");
    missing_chrome.history_exists = false;
    missing_chrome.history_file_name = "History".to_string();
    missing_chrome.profile_path = "/profiles/missing".to_string();
    let mut missing_safari = profile("safari", "safari:Default");
    missing_safari.history_exists = false;
    let mut unreadable_edge = profile("chromium", "edge:Default");
    unreadable_edge.browser_name = "Microsoft Edge".to_string();
    unreadable_edge.history_readable = false;
    unreadable_edge.profile_path = "/profiles/edge/Default".to_string();

    let discovered = vec![readable.clone(), missing_chrome, missing_safari, unreadable_edge];
    let selected = vec![
        "chrome:Default".to_string(),
        "chrome:Missing".to_string(),
        "safari:Default".to_string(),
        "edge:Default".to_string(),
        "chrome:Gone".to_string(),
    ];
    let supported = select_supported_profiles(&discovered, &selected);
    assert_eq!(
        supported.iter().map(|profile| profile.profile_id.as_str()).collect::<Vec<_>>(),
        vec!["chrome:Default"]
    );

    let warnings = collect_skipped_profiles(&discovered, &selected);
    assert!(warnings.iter().any(|warning| warning.message.contains("Full Disk Access")));
    assert!(warnings.iter().any(|warning| warning.message.contains("History is missing")));
    assert!(warnings.iter().any(|warning| warning.message.contains("edge:Default")));
    assert!(warnings.iter().any(|warning| warning.message.contains("missing or unreadable")));
    assert!(warnings.iter().any(|warning| warning.message.contains("chrome:Gone")));
    // The code + typed-param channel: Safari FDA keeps its existing frozen
    // code; the other skips carry the new enumerable codes with profile ids.
    assert!(warnings.iter().any(|warning| {
        warning.code == "safari-full-disk-access-skip"
            && warning.profile_id.as_deref() == Some("safari:Default")
    }));
    assert!(warnings.iter().any(|warning| {
        warning.code == "profile-history-unreadable-skip"
            && warning.profile_id.as_deref() == Some("edge:Default")
            && warning
                .diagnostic
                .as_deref()
                .is_some_and(|diagnostic| diagnostic.contains("/profiles/edge/Default"))
    }));
    assert!(warnings.iter().any(|warning| {
        warning.code == "profile-not-detected-skip"
            && warning.profile_id.as_deref() == Some("chrome:Gone")
    }));

    let firefox = profile("firefox", "firefox:Default");
    assert_eq!(url_marker(&firefox.browser_family, &parsed_url(7, 123)), 123);
    assert_eq!(favicon_marker(&firefox.browser_family, &parsed_favicon(456)), 456);

    // Chromium watermark precision: when the parser exposes the raw
    // chrome_micros value via `source_last_visit_marker`, the marker MUST
    // return it verbatim. The legacy path round-tripped through
    // `last_visit_ms` (`/1000` then `*1000`) which truncated up to 999 µs
    // per URL and caused every incremental ingest's `WHERE last_visit_time
    // >= ?1` predicate to re-match the same URL row.
    let chromium = profile("chromium", "chrome:Default");
    let mut precise = parsed_url(11, 1_355_526_400_500); // ms-precision parsed value
    let raw_chrome_micros = 13_000_000_000_500_123_i64; // arbitrary sub-ms chrome_time
    precise.source_last_visit_marker = Some(raw_chrome_micros);
    assert_eq!(url_marker(&chromium.browser_family, &precise), raw_chrome_micros);

    // Legacy chromium path (no native marker exposed): falls back to the
    // truncating round-trip so existing serialised ParsedUrl payloads keep
    // their pre-fix behaviour. Use a deterministic mid-range value to make
    // the conversion easy to read.
    let mut legacy = parsed_url(13, 1_500);
    legacy.source_last_visit_marker = None;
    let recovered = url_marker(&chromium.browser_family, &legacy);
    // `unix_micros_to_chrome_time(1_500 * 1_000) = 1_500_000 + OFFSET`.
    let expected_legacy = 1_500_i64 * 1_000 + 11_644_473_600_000_000_i64;
    assert_eq!(recovered, expected_legacy);

    let snapshot = snapshot_with_history(tempdir().expect("tempdir"), readable);
    let source_hashes = snapshot_source_hashes(&snapshot);
    assert_eq!(source_hashes.get("History").map(String::as_str), Some("hash-history"));
}

#[test]
fn ingest_stream_helpers_cover_favicon_toggle_and_parse_error_mapping() {
    let readable = profile("chromium", "chrome:Default");
    let snapshot = snapshot_with_history(tempdir().expect("snapshot"), readable);

    let with_favicons = chromium_history_database_set(&snapshot, true);
    assert_eq!(with_favicons.history_path, snapshot.history_path);
    assert_eq!(with_favicons.favicons_path, snapshot.favicons_path);

    let without_favicons = chromium_history_database_set(&snapshot, false);
    assert!(without_favicons.favicons_path.is_none());

    let error = stream_history_error_to_anyhow(StreamHistoryError::<anyhow::Error>::Parse(
        browser_history_parser::ParseError::UnsupportedProvider { provider: "fixture" },
    ));
    assert!(format!("{error:#}").contains("fixture"));
}

#[test]
fn streamed_profile_summary_records_checkpoint_and_evidence_plan() {
    let vault = crate::Vault::open_temp().unwrap();
    let (context, _) = seed_context(&vault, 42, &profile("chromium", "chrome:Default"));
    let dir = tempdir().unwrap();
    let history = dir.path().join("History");
    use browser_history_fixtures::{
        ChromiumHistoryFixture, ChromiumKeywordSearchTermRow, ChromiumUrlRow, ChromiumVisitRow,
    };
    let favicons = dir.path().join("Favicons");
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 7,
            url: "https://checkpoint.test".into(),
            title: Some("checkpoint".into()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: 1_800_000_000_003,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 11,
            url_id: 7,
            visit_time_unix_ms: 1_800_000_000_003,
            from_visit: None,
            transition: Some(1),
            visit_duration_micros: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        })
        .add_search_term(ChromiumKeywordSearchTermRow {
            keyword_id: 1,
            url_id: 7,
            term: "checkpoint term".into(),
            normalized_term: "checkpoint term".into(),
        })
        .add_download(browser_history_fixtures::ChromiumDownloadRow {
            id: 21,
            guid: Some("download-21".into()),
            current_path: None,
            target_path: Some("/tmp/file".into()),
            start_time_unix_ms: Some(1_800_000_000_004),
            received_bytes: Some(1),
            total_bytes: Some(1),
            state: Some(1),
            mime_type: None,
            original_mime_type: None,
        })
        .add_favicon(browser_history_fixtures::ChromiumFaviconRow {
            id: 31,
            icon_url: "https://checkpoint.test/favicon.ico".into(),
            icon_type: Some(1),
            width: 16,
            height: 16,
            last_updated_unix_ms: 1_800_000_000_005,
            image_data: Some(vec![1, 2, 3]),
        })
        .add_icon_mapping(browser_history_fixtures::ChromiumIconMappingRow {
            page_url: "https://checkpoint.test".into(),
            icon_id: 31,
        });
    fixture.write(&history).unwrap();
    fixture.write_favicons(&favicons).unwrap();
    let snapshot = ProfileSnapshot {
        profile: profile("chromium", "chrome:Default"),
        temp_dir: dir,
        history_path: history,
        favicons_path: Some(favicons),
        source_hashes: vec![FileFingerprint {
            path: "History".into(),
            sha256: "hash-history".into(),
        }],
    };
    let summary = process_profile_snapshot(
        &vault,
        context.run_id,
        &snapshot,
        true,
        true,
        &CancelToken::new(),
        None,
    )
    .unwrap();
    assert!(summary.checkpoint_created);
    let saved = vault.store().read(Db::Archive, |s| load_watermark(s, "chrome:Default")).unwrap();
    assert_eq!(saved.last_visit_id, 11);
    assert_eq!(saved.last_download_id, 21);
    assert_eq!(
        saved.last_favicon_last_updated,
        crate::utils::unix_micros_to_chrome_time(1_800_000_000_005_000)
    );
    assert_eq!(
        saved.last_url_last_visit_time,
        crate::utils::unix_micros_to_chrome_time(1_800_000_000_003_000)
    );
    assert!(saved.last_schema_hash.is_some());
    assert_eq!(summary.new_visits, 1);
    assert!(
        summary
            .note_details
            .iter()
            .any(|warning| warning.code == "profile-search-terms-captured"
                && warning.count == Some(1))
    );
    assert!(saved.last_checkpoint_at.is_some());
    assert!(saved.last_source_batch_id.is_some());
    let path: String = Probe::new(&vault)
        .query_row("SELECT file_path FROM snapshots", (), |r| Ok(r.get(0)?))
        .unwrap();
    assert!(Path::new(&path).join("History").exists());
    let evidence = Probe::for_db(&vault, Db::SourceEvidence);
    let artifacts: String = evidence
        .query_row(
            "SELECT artifact_refs_json FROM source_batches ORDER BY id DESC LIMIT 1",
            (),
            |r| Ok(r.get(0)?),
        )
        .unwrap();
    assert!(artifacts.contains("hash-history"));
}

#[test]
fn ingest_low_level_helpers_cover_empty_product_and_unsupported_family_edges() {
    let vault = crate::Vault::open_temp().unwrap();
    let mut profile = profile("chromium", "chrome:Default");
    profile.browser_name = " ".into();
    let (context, _) = seed_context(&vault, 12, &profile);
    let product: String = Probe::new(&vault)
        .query_row(
            "SELECT browser_product FROM source_profiles WHERE id=?1",
            [context.source_profile_id],
            |r| Ok(r.get(0)?),
        )
        .unwrap();
    assert_eq!(product, "chrome");
    let mut bounds = HashMap::new();
    track_url_visit_bounds(&mut bounds, 1, &parsed_visit(1, 1, 3000));
    track_url_visit_bounds(&mut bounds, 1, &parsed_visit(2, 1, 1000));
    assert_eq!((bounds[&1].first_visit_ms, bounds[&1].last_visit_ms), (1000, 3000));
    profile.browser_family = "opera".into();
    let snapshot = snapshot_with_history(tempdir().unwrap(), profile);
    assert!(
        process_profile_snapshot(&vault, 12, &snapshot, false, false, &CancelToken::new(), None)
            .unwrap_err()
            .to_string()
            .contains("not supported")
    );
}

pub(super) fn seed_context(
    vault: &crate::Vault,
    run: i64,
    profile: &crate::models::BrowserProfile,
) -> (WriteContext, i64) {
    let owned = profile.clone();
    let id = vault
        .write(Db::Archive, Priority::Batch, move |s| {
            seed_run(s, run);
            Ok(upsert_source_profile(s, &owned)?)
        })
        .unwrap();
    let input = SourceBatchInput {
        source_profile_id: id,
        run_id: Some(run),
        source_kind: "local_db".into(),
        browser_version: None,
        schema_version_text: None,
        schema_version_int: None,
        schema_fingerprint: "test".into(),
        capability_snapshot: CapabilitySnapshot::default(),
        coverage_stats_json: "{}".into(),
        artifact_refs_json: None,
        notes_json: None,
    };
    let batch = vault
        .write(Db::SourceEvidence, Priority::Batch, move |s| upsert_source_batch(s, &input))
        .unwrap();
    (
        WriteContext {
            run_id: run,
            source_profile_id: id,
            profile_id: profile.profile_id.clone(),
            browser_family: profile.browser_family.clone(),
            import: None,
        },
        batch,
    )
}

#[test]
fn source_evidence_commits_before_the_matching_watermark() {
    for key in [None, Some([78; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let (context, batch) = seed_context(&vault, 77, &profile("chromium", "chrome:Default"));
        vault
            .write(Db::SourceEvidence, Priority::Batch, move |s| {
                record_schema_observation(s, batch, "test", &SchemaObservation::default())
            })
            .unwrap();
        let payload = SourceEvidencePayload {
            typed_evidence: TypedEvidenceBatch {
                search: vec![SearchEvidence {
                    source_visit_id: Some(1),
                    source_url_id: Some(1),
                    evidence_key: "query".to_string(),
                    evidence_value: "pathkeep".to_string(),
                    normalized_value: Some("pathkeep".to_string()),
                    source_field: "keyword_search_terms.term".to_string(),
                }],
                navigation: vec![NavigationEvidence {
                    source_visit_id: 1,
                    edge_kind: "from_visit".to_string(),
                    target_visit_id: Some(2),
                    target_url: Some("https://example.com/next".to_string()),
                    transition: Some(805306368),
                    source_field: "visits.from_visit".to_string(),
                }],
                engagement: vec![EngagementEvidence {
                    source_visit_id: 1,
                    metric_key: "duration_ms".to_string(),
                    metric_value_int: Some(1200),
                    metric_value_real: None,
                    source_field: "visits.visit_duration".to_string(),
                }],
                context: vec![ContextEvidence {
                    source_visit_id: Some(1),
                    source_url_id: Some(1),
                    context_key: "app_id".to_string(),
                    value_json: "\"browser\"".to_string(),
                    source_field: "visits.app_id".to_string(),
                }],
            },
            native_entities: vec![NativeEntity {
                entity_kind: "visit".to_string(),
                native_primary_key: "1".to_string(),
                parent_native_primary_key: None,
                payload_json: "{}".to_string(),
                metadata: BTreeMap::new(),
            }],
        };
        let cancel = CancelToken::new();
        let mut consumer =
            ArchiveChunkConsumer::new(&vault, context, Watermark::default(), &cancel, None, batch);
        consumer.urls(vec![parsed_url(1, 1000)]).unwrap();
        consumer.visits(vec![parsed_visit(1, 1, 1000)]).unwrap();
        consumer
            .source_evidence(browser_history_parser::SourceEvidenceChunk {
                typed_evidence: payload.typed_evidence,
                native_entities: payload.native_entities,
            })
            .unwrap();
        consumer.finish().unwrap();
        let evidence = Probe::for_db(&vault, Db::SourceEvidence);
        for table in [
            "source_batches",
            "schema_observations",
            "visit_search_evidence",
            "visit_navigation_evidence",
            "visit_engagement_evidence",
            "visit_context_evidence",
            "native_entities",
        ] {
            let count: i64 = evidence
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(r.get(0)?))
                .unwrap();
            assert_eq!(count, 1, "{table}");
        }
        let saved =
            vault.store().read(Db::Archive, |s| load_watermark(s, "chrome:Default")).unwrap();
        assert_eq!(saved.last_source_batch_id, Some(batch));
        assert_eq!(saved.last_visit_id, 1);
    }
}
