//! Time-boundary and nullable-column ingest contract tests.
//!
//! ## Responsibilities
//! - Pin epoch, year-2038, far-future, and negative timestamp behavior.
//! - Pin NULL title projection into the archive.
//!
//! ## Not responsible for
//! - Locale/timezone aggregation behavior outside raw ingest storage.
//! - Unicode normalization and hidden-flag contracts covered by sibling modules.
//!
//! ## Dependencies
//! - Uses shared edge-case fixture helpers from the parent module.
//! - Uses synthetic Chromium History fixtures only.
//!
//! ## Performance notes
//! - Each test imports one tiny fixture into an isolated temp archive.

use super::*;

// ======================================================================
// E1-E4 — Time boundary edge cases
// ======================================================================

/// E1 — Epoch timestamp boundary: visit_time_ms = 0 (1970-01-01T00:00:00Z).
/// A zero timestamp is legal in the archive schema and must round-trip
/// without error. This pins the lower bound of the time domain.
#[test]
fn e1_epoch_timestamp_imports_without_error() {
    let env = ScenarioEnv::new();
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/epoch".to_string(),
            title: Some("Epoch Boundary".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: 0,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, 0));
    let snapshot =
        snapshot_for_chromium_fixture(&fixture, chromium_profile("chrome:Epoch", "Google Chrome"));
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    // Verify the timestamp is stored as 0.
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Epoch'",
            [],
            |row| row.get(0),
        )
        .expect("query epoch visit time");
    assert_eq!(visit_time, 0, "epoch timestamp must round-trip as 0");
}

/// E2 — Year-2038 boundary (2038-01-19T03:14:07Z = 2_147_483_647_000 ms).
/// PathKeep uses i64 for timestamps, so the 32-bit overflow must be
/// transparent. This pins the contract.
#[test]
fn e2_year_2038_boundary_imports_without_error() {
    let env = ScenarioEnv::new();
    let y2038_ms = 2_147_483_647_000_i64; // 2038-01-19T03:14:07Z
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/y2038".to_string(),
            title: Some("Year 2038".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: y2038_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, y2038_ms));
    let snapshot =
        snapshot_for_chromium_fixture(&fixture, chromium_profile("chrome:Y2038", "Google Chrome"));
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Y2038'",
            [],
            |row| row.get(0),
        )
        .expect("query y2038 visit time");
    assert_eq!(visit_time, y2038_ms, "year-2038 timestamp must round-trip correctly");
}

/// E3 — Far-future timestamp (year 3000 ≈ 32_503_680_000_000 ms).
/// Clock skew or data corruption can produce far-future timestamps.
/// The archive must accept them without error.
#[test]
fn e3_far_future_timestamp_imports_without_error() {
    let env = ScenarioEnv::new();
    let far_future_ms = 32_503_680_000_000_i64; // ~3000-01-01
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/future".to_string(),
            title: Some("Far Future".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: far_future_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, far_future_ms));
    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:FarFuture", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:FarFuture'",
            [],
            |row| row.get(0),
        )
        .expect("query far-future visit time");
    assert_eq!(visit_time, far_future_ms, "far-future timestamp must round-trip correctly");
}

/// E4 — Negative timestamp (before Unix epoch, e.g. 1969-12-31).
///
/// All browser parsers (Chromium, Firefox, Safari) clamp visit times to
/// `max(0)` when converting from browser-native format back to Unix ms.
/// A negative source timestamp therefore survives the fixture writer
/// (Chromium maps it to a valid Chrome-epoch microsecond value) but the
/// parser clamps the result to 0 on read-back. The archive must accept
/// the row without error; the stored `visit_time_ms` will be 0.
#[test]
fn e4_negative_timestamp_clamped_to_zero_without_error() {
    let env = ScenarioEnv::new();
    // -86_400_000 ms = 1969-12-31T00:00:00Z (one day before epoch).
    // The Chromium fixture writer converts this to a valid Chrome-epoch
    // microsecond (11_558_073_600_000_000), but the production parser's
    // `chrome_time_to_unix_ms` applies `.max(0)`, so it becomes 0.
    let negative_ms = -86_400_000_i64;
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/pre-epoch".to_string(),
            title: Some("Pre-Epoch".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: negative_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, negative_ms));
    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:PreEpoch", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:PreEpoch'",
            [],
            |row| row.get(0),
        )
        .expect("query pre-epoch visit time");
    assert_eq!(visit_time, 0, "negative timestamp must be clamped to 0 by parser's max(0)");
}

// ======================================================================
// E7 — NULL title handling
// ======================================================================

/// E7 — Real Chrome `History` databases routinely have URLs with NULL
/// `title` columns (the user navigated to a URL but the page never
/// finished loading, or it was a binary download). The PathKeep parser
/// must tolerate this and produce a canonical URL row with `title =
/// NULL` rather than failing or storing an empty string. This pins the
/// contract that nullable source columns project as NULL in the archive.
#[test]
fn e7_null_title_imports_with_null_archive_title() {
    let env = ScenarioEnv::new();
    let day_one_ms = 1_777_680_000_000_i64;

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/no-title".to_string(),
            title: None,
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/with-title".to_string(),
            title: Some("Has Title".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, day_one_ms))
        .add_visit(chromium_visit_row(2, 2, day_one_ms));
    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:NullTitle", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 2);
    assert_eq!(summary.new_visits, 2);

    let archive = env.open_archive();
    let no_title: Option<String> = archive
        .query_row(
            "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:NullTitle'
               AND urls.source_url_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("query null-title url");
    assert!(
        no_title.is_none(),
        "NULL source title must project as NULL in archive, not empty string"
    );

    let with_title: Option<String> = archive
        .query_row(
            "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:NullTitle'
               AND urls.source_url_id = 2",
            [],
            |row| row.get(0),
        )
        .expect("query with-title url");
    assert_eq!(with_title.as_deref(), Some("Has Title"));
}
