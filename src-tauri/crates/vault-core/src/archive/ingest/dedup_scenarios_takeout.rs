//! Takeout-family dedup scenarios (T1, T2, T2b, T3, T5).
//!
//! Covers the Google Takeout BrowserHistory JSON import path and its
//! interaction with local-Chrome backups. Each scenario pins a specific
//! dedup contract documented in the audit:
//!
//! - **T1** — Takeout baseline import (happy path).
//! - **T2** — File-rename re-import deduplicates via fingerprint partial index.
//! - **T2b** — Fingerprint divergence (title drift) exposes B3.
//! - **T3** — Takeout × local Chrome same-period overlap (B4 contract).
//! - **T5** — `time_usec` unit contract (B6 pinning).

use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, TakeoutBrowserHistoryFixture,
    TakeoutBrowserRecord,
};
use rusqlite::Connection;
use tempfile::{TempDir, tempdir};

// ======================================================================
// Shared helpers (per satellite-module pattern — each #[cfg(test)] module
// owns its own ScenarioEnv)
// ======================================================================

fn test_config() -> AppConfig {
    AppConfig { initialized: true, ..AppConfig::default() }
}

fn test_paths(root: &Path) -> ProjectPaths {
    crate::config::project_paths_with_root(root)
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

fn seed_run(archive: &Transaction<'_>, run_id: i64) {
    archive
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (?1, 'backup', 'manual', '2026-05-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
            [run_id],
        )
        .expect("seed run");
}

fn run_one_ingest(
    env: &ScenarioEnv,
    run_id: i64,
    snapshot: &ProfileSnapshot,
    use_watermark: bool,
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
        use_watermark,
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

fn count_urls_for_profile(env: &ScenarioEnv, profile_key: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT COUNT(*) FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1",
            [profile_key],
            |row| row.get(0),
        )
        .expect("count urls for profile")
}

fn count_visits_for_profile(env: &ScenarioEnv, profile_key: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT COUNT(*) FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = ?1",
            [profile_key],
            |row| row.get(0),
        )
        .expect("count visits for profile")
}

// ======================================================================
// Chromium helpers (needed by T3 which imports Chrome + Takeout)
// ======================================================================

fn chromium_profile(profile_id: &str, browser_name: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: browser_name.to_string(),
        user_name: Some("synthetic-user".to_string()),
        profile_path: format!("/synthetic/{profile_id}"),
        history_path: Some(format!("/synthetic/{profile_id}/History")),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("146.0.0.0".to_string()),
        history_file_name: "History".to_string(),
        history_bytes: 128,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    }
}

fn visit_row(id: i64, url_id: i64, visit_time_unix_ms: i64) -> ChromiumVisitRow {
    ChromiumVisitRow {
        id,
        url_id,
        visit_time_unix_ms,
        from_visit: Some(0),
        transition: Some(805306368),
        visit_duration_micros: Some(5_000_000),
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: None,
    }
}

fn snapshot_for_fixture(
    fixture: &ChromiumHistoryFixture,
    profile: crate::models::BrowserProfile,
) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("snapshot tempdir");
    let history_path = temp_dir.path().join("History");
    fixture.write(&history_path).expect("write chromium fixture");
    let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    let mut profile = profile;
    profile.history_bytes = history_bytes;
    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "History".to_string(),
            sha256: "synthetic-fixture-hash".to_string(),
        }],
    }
}

// ======================================================================
// Takeout helpers
// ======================================================================

fn takeout_record(url: &str, title: &str, visit_time_unix_ms: i64) -> TakeoutBrowserRecord {
    TakeoutBrowserRecord {
        url: url.to_string(),
        title: Some(title.to_string()),
        visit_time_unix_ms,
        page_transition: Some("LINK".to_string()),
        client_id: None,
        favicon_url: None,
        ptoken: None,
    }
}

fn import_takeout_fixture(env: &ScenarioEnv, records: &[TakeoutBrowserRecord], label: &str) {
    let root = tempdir().unwrap_or_else(|_| panic!("{label} takeout root"));
    let payload = root.path().join("Chrome/BrowserHistory.json");
    let mut fixture = TakeoutBrowserHistoryFixture::new();
    for record in records {
        fixture = fixture.add_record(record.clone());
    }
    fixture.write(&payload).expect("write takeout fixture");
    crate::takeout::import_takeout(
        &env.paths,
        &env.config,
        None,
        &crate::models::TakeoutRequest {
            source_path: root.path().display().to_string(),
            dry_run: false,
        },
    )
    .unwrap_or_else(|err| panic!("{label} import_takeout failed: {err}"));
    drop(root);
}

// ======================================================================
// T1: Takeout baseline import
// ======================================================================

/// T1 — A Takeout BrowserHistory JSON gets imported via the public
/// `import_takeout` flow. Asserts row counts under the synthetic profile
/// the Takeout flow upserts (`takeout::browser-history`) and that visit
/// `app_id` lands as `"takeout"`.
#[test]
fn t1_takeout_baseline_import() {
    let env = ScenarioEnv::new();
    let source_root = tempdir().expect("takeout source root");
    let payload_path = source_root.path().join("Chrome/BrowserHistory.json");

    TakeoutBrowserHistoryFixture::new()
        .add_record(takeout_record("https://example.com/page-one", "Page One", 1_777_680_000_000))
        .add_record(takeout_record("https://example.com/page-two", "Page Two", 1_777_809_600_000))
        .add_record(takeout_record(
            "https://example.org/page-three",
            "Page Three",
            1_777_872_930_000,
        ))
        .write(&payload_path)
        .expect("write takeout fixture");

    let request = crate::models::TakeoutRequest {
        source_path: source_root.path().display().to_string(),
        dry_run: false,
    };

    let inspection = crate::takeout::import_takeout(&env.paths, &env.config, None, &request)
        .expect("import takeout");

    assert!(!inspection.dry_run);
    assert_eq!(inspection.imported_items + inspection.duplicate_items, 3);

    let profile_key = "takeout::browser-history";
    assert_eq!(count_urls_for_profile(&env, profile_key), 3);
    assert_eq!(count_visits_for_profile(&env, profile_key), 3);

    // Takeout-sourced visits must carry app_id="takeout"; this is the same
    // hardcoded marker that contributes to B4's fingerprint mismatch.
    let archive = env.open_archive();
    let takeout_visit_count: i64 = archive
        .query_row(
            "SELECT COUNT(*) FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND visits.app_id = 'takeout'",
            [profile_key],
            |row| row.get(0),
        )
        .expect("takeout app_id count");
    assert_eq!(takeout_visit_count, 3);
}

// ======================================================================
// T2: Takeout file rename re-import — refines B3 framing
// ======================================================================

/// T2 — Re-importing the same Takeout records from a different on-disk
/// path. The audit's first cut of **B3** ("path-bound source_visit_id
/// causes a full duplicate set on every re-import") turned out to overstate
/// the practical risk: while it is true that the path change does produce
/// completely different `source_visit_id` values for every record, the
/// `(source_profile_id, event_fingerprint)` partial unique index catches
/// the duplicates because the fingerprint inputs (url, visit_time_ms,
/// title, transition=None, app_id="takeout") are identical across the two
/// imports.
///
/// This scenario pins the **actual current behavior**: rename-only
/// re-import of unchanged Takeout records is correctly de-duplicated by
/// the fingerprint partial index, ending at 3 visit rows. The B3 design
/// concern (poor robustness — the path-bound id provides zero useful
/// signal, so the system relies on the fingerprint as a single layer)
/// stays documented in the audit; [`t2b_takeout_rename_with_title_change_demonstrates_b3_when_fingerprint_diverges`]
/// covers the case where the fingerprint can't save B3 anymore.
#[test]
fn t2_takeout_rename_file_reimport_dedups_via_fingerprint_partial_index() {
    let env = ScenarioEnv::new();

    let records: Vec<TakeoutBrowserRecord> = (0..3)
        .map(|index| {
            let visit_time = 1_777_680_000_000 + (index as i64 * 86_400_000);
            takeout_record(
                &format!("https://example.com/article-{index}"),
                &format!("Article {index}"),
                visit_time,
            )
        })
        .collect();

    import_takeout_fixture(&env, &records, "first");
    let profile_key = "takeout::browser-history";
    assert_eq!(count_visits_for_profile(&env, profile_key), 3);

    import_takeout_fixture(&env, &records, "second");

    // The fingerprint partial index catches the duplicates even though
    // every source_visit_id differs from the first pass.
    assert_eq!(
        count_visits_for_profile(&env, profile_key),
        3,
        "fingerprint partial index dedups the renamed-source re-import"
    );
}

// ======================================================================
// T2b: Fingerprint divergence exposes B3
// ======================================================================

/// T2b — When the fingerprint cannot rescue B3, the path-bound
/// `source_visit_id` produces a real duplicate set. Two re-imports of the
/// "same" record but with even one fingerprint input changed (title
/// here) defeat the fingerprint partial index, leaving the broken
/// path-bound primary key as the only defense. The result is the full
/// duplicate set the audit warned about.
///
/// This is a `should_panic` failing test today: the assertion below is
/// what the system should provide after B3 is fixed (e.g. by deriving
/// `source_visit_id` from `(url, visit_time_micros)` so the primary key
/// is stable across re-imports regardless of path or fingerprint input
/// drift). Today the count grows to 6 and the assertion fires.
#[test]
fn t2b_takeout_rename_with_title_change_demonstrates_b3_when_fingerprint_diverges() {
    let env = ScenarioEnv::new();

    let first_records: Vec<TakeoutBrowserRecord> = (0..3)
        .map(|index| {
            let visit_time = 1_777_680_000_000 + (index as i64 * 86_400_000);
            takeout_record(
                &format!("https://example.com/article-{index}"),
                &format!("Original title {index}"),
                visit_time,
            )
        })
        .collect();
    import_takeout_fixture(&env, &first_records, "first");

    // Real-world equivalent: user re-exports Takeout months later; Google
    // captured an updated page title in the meantime. Same URL, same
    // visit time, different title → fingerprint differs.
    let second_records: Vec<TakeoutBrowserRecord> = first_records
        .iter()
        .map(|record| {
            let mut next = record.clone();
            next.title = Some(format!(
                "Updated title for {}",
                record.url.rsplit('/').next().unwrap_or("page")
            ));
            next
        })
        .collect();
    import_takeout_fixture(&env, &second_records, "second");

    let profile_key = "takeout::browser-history";
    let visit_count = count_visits_for_profile(&env, profile_key);

    // Expected post-fix: 3 visits (treated as the same logical event with
    // an updated title). Today: 6 (because both source_visit_id and
    // event_fingerprint differ across the two imports).
    assert_eq!(
        visit_count, 3,
        "B3 fix required: rename + title drift duplicates rows (got {visit_count})"
    );
}

// ======================================================================
// T3: Takeout x local Chrome same-period overlap — B4 contract
// ======================================================================

/// T3 — Same-period overlap between a local Chrome profile and the
/// Takeout JSON of the same Chrome installation. The audit's **B4**
/// observation: even when records describe literally the same browsing
/// event, the fingerprint inputs differ between the two source paths
/// (local Chrome has a real `transition` and the browser's real
/// `app_id`; Takeout hardcodes `app_id = "takeout"` and `transition =
/// None`), so even a hypothetical cross-source-profile fingerprint
/// dedup would not match. This contract scenario pins the current
/// storage truth — 3 + 3 = 6 visits across two profiles — and
/// documents the input divergence so any future "merge across sources"
/// proposal must address the fingerprint normalization gap first.
#[test]
fn t3_takeout_and_local_chrome_same_period_b4_contract() {
    let env = ScenarioEnv::new();
    let day_one = 1_777_680_000_000_i64;
    let day_two = 1_777_809_600_000_i64;
    let day_three = 1_777_872_930_000_i64;

    let chrome_fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/shared-one".to_string(),
            title: Some("Shared One".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/shared-two".to_string(),
            title: Some("Shared Two".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_two,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 3,
            url: "https://example.com/shared-three".to_string(),
            title: Some("Shared Three".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_three,
            hidden: false,
        })
        .add_visit(visit_row(10, 1, day_one))
        .add_visit(visit_row(11, 2, day_two))
        .add_visit(visit_row(12, 3, day_three));
    let chrome_snapshot =
        snapshot_for_fixture(&chrome_fixture, chromium_profile("chrome:Default", "Google Chrome"));
    run_one_ingest(&env, 1, &chrome_snapshot, false);

    let takeout_source = tempdir().expect("takeout source root");
    let takeout_payload = takeout_source.path().join("Chrome/BrowserHistory.json");
    TakeoutBrowserHistoryFixture::new()
        .add_record(takeout_record("https://example.com/shared-one", "Shared One", day_one))
        .add_record(takeout_record("https://example.com/shared-two", "Shared Two", day_two))
        .add_record(takeout_record("https://example.com/shared-three", "Shared Three", day_three))
        .write(&takeout_payload)
        .expect("write takeout fixture");
    crate::takeout::import_takeout(
        &env.paths,
        &env.config,
        None,
        &crate::models::TakeoutRequest {
            source_path: takeout_source.path().display().to_string(),
            dry_run: false,
        },
    )
    .expect("import takeout");

    // Each source kept independent rows under its own source_profile.
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 3);
    assert_eq!(count_visits_for_profile(&env, "takeout::browser-history"), 3);
    assert_eq!(count_archive_rows(&env, "visits"), 6);

    // Fingerprint divergence: a future cross-source dedup design has to
    // normalize app_id (and likely also project transition to None) before
    // any pair of these visits could share a fingerprint.
    let archive = env.open_archive();
    let chrome_app_ids: Vec<Option<String>> = archive
        .prepare(
            "SELECT app_id FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Default'",
        )
        .expect("prepare chrome")
        .query_map([], |row| row.get(0))
        .expect("query chrome")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect chrome");
    let takeout_app_ids: Vec<Option<String>> = archive
        .prepare(
            "SELECT app_id FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'takeout::browser-history'",
        )
        .expect("prepare takeout")
        .query_map([], |row| row.get(0))
        .expect("query takeout")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect takeout");
    assert!(chrome_app_ids.iter().all(|app_id| app_id.is_none()));
    assert!(takeout_app_ids.iter().all(|app_id| app_id.as_deref() == Some("takeout")));
}

// ======================================================================
// T5: Takeout time_usec unit contract — B6 pinning
// ======================================================================

/// T5 — Pins the current interpretation of Takeout's `time_usec` field
/// as **Unix-epoch microseconds**. The audit raised **B6** because the
/// helper `micros_to_unix_ms` (parser side) name asserts Unix
/// microseconds but Google's Takeout dumps historically used Chrome
/// epoch microseconds (since 1601). The harness writer emits Unix
/// microseconds; the parser reads Unix microseconds; this test pins
/// that contract end-to-end. If anyone later flips the parser to assume
/// Chrome epoch, T5 fails immediately. If a future real-world Takeout
/// sample disagrees with this interpretation, the writer + this test
/// must be updated together — the audit B6 note documents the open
/// question.
#[test]
fn t5_takeout_time_usec_pinned_as_unix_microseconds_b6_contract() {
    let env = ScenarioEnv::new();
    let source_root = tempdir().expect("takeout source root");
    let payload_path = source_root.path().join("Chrome/BrowserHistory.json");

    // 2026-05-02T00:00:00Z = 1_777_680_000_000 Unix ms = 1_777_680_000_000_000 Unix μs.
    // If the parser treated this as Chrome μs the resulting Unix ms would
    // be (1_777_680_000_000_000 - 11_644_473_600_000_000) / 1000, which
    // produces a negative or wildly different timestamp the assertion
    // below catches.
    let visit_one = 1_777_680_000_000_i64;

    TakeoutBrowserHistoryFixture::new()
        .add_record(takeout_record("https://example.com/time-pin", "Time Pin", visit_one))
        .write(&payload_path)
        .expect("write takeout fixture");

    crate::takeout::import_takeout(
        &env.paths,
        &env.config,
        None,
        &crate::models::TakeoutRequest {
            source_path: source_root.path().display().to_string(),
            dry_run: false,
        },
    )
    .expect("import takeout");

    let archive = env.open_archive();
    let (visit_time_ms, visit_time_iso): (i64, String) = archive
        .query_row(
            "SELECT visits.visit_time_ms, visits.visit_time_iso FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'takeout::browser-history'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query takeout visit time");

    assert_eq!(visit_time_ms, visit_one, "Takeout time_usec must round-trip as Unix milliseconds");
    assert!(
        visit_time_iso.starts_with("2026-05-02"),
        "Takeout ISO must reflect 2026-05-02, got {visit_time_iso}"
    );
}

// ======================================================================
// T6: Takeout URL upsert B1 protection — older-snapshot re-import must not regress
// ======================================================================

/// T6 — Audit bug B1 was originally identified and fixed in
/// `archive/ingest/writes.rs::upsert_url` (commit 6884c10d) but the
/// Takeout import path in `takeout/payload_import.rs` was left with
/// unconditional `excluded.*` overwrites and a hardcoded
/// `visit_count = 1` literal in the INSERT VALUES with no UPDATE clause
/// for visit_count or typed_count at all. A re-import of an older
/// Takeout snapshot would silently overwrite title / hidden with stale
/// values, and a fresh Takeout export with new visits to the same URL
/// would never bump visit_count.
///
/// This scenario pins the B1 fix applied to `payload_import.rs`:
///
/// 1. **Older snapshot re-import** must not regress `title` / `hidden`
///    (strictly older `last_visit_ms` → preserve newer values).
/// 2. **MAX(visit_count)** must use the larger of stored vs incoming so
///    a later Takeout export reflecting new visits actually bumps the
///    archive's visit_count.
/// 3. **Tied `last_visit_ms`** must NOT trigger an overwrite (matches the
///    `>` vs `>=` tie-break tightened in writes.rs).
#[test]
fn t6_takeout_payload_import_url_upsert_protects_against_older_snapshot_regression() {
    let env = ScenarioEnv::new();
    let earlier_ms = 1_777_680_000_000_i64; // 2026-05-02T00:00:00Z
    let later_ms = 1_777_809_600_000_i64; // 2026-05-03T12:00:00Z

    // Pass 1: import the LATER snapshot first. Two records to the same
    // URL with the meaningful title; visit_count merges to 2 in the
    // parser via merge_url_state.
    let later_records: Vec<TakeoutBrowserRecord> = vec![
        TakeoutBrowserRecord {
            url: "https://example.com/news".to_string(),
            title: Some("Meaningful Title".to_string()),
            visit_time_unix_ms: later_ms - 1_000,
            page_transition: Some("LINK".to_string()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        },
        TakeoutBrowserRecord {
            url: "https://example.com/news".to_string(),
            title: Some("Meaningful Title".to_string()),
            visit_time_unix_ms: later_ms,
            page_transition: Some("LINK".to_string()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        },
    ];
    import_takeout_fixture(&env, &later_records, "later");

    let profile_key = "takeout::browser-history";
    let archive = env.open_archive();
    let read_url_state = || -> (String, Option<String>, i64, i64) {
        let conn = env.open_archive();
        conn.query_row(
            "SELECT url, title, visit_count, hidden FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1
               AND urls.url = 'https://example.com/news'",
            [profile_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("query url state")
    };
    drop(archive);

    let (url1, title1, count1, hidden1) = read_url_state();
    assert_eq!(url1, "https://example.com/news");
    assert_eq!(title1.as_deref(), Some("Meaningful Title"));
    assert_eq!(count1, 2, "later snapshot's visit_count of 2 must land");
    assert_eq!(hidden1, 0);

    // Pass 2: re-import the OLDER snapshot. Single record at earlier_ms
    // with a NULL title and (implicitly) hidden=false. The parser will
    // produce visit_count=1.
    let older_records: Vec<TakeoutBrowserRecord> = vec![TakeoutBrowserRecord {
        url: "https://example.com/news".to_string(),
        title: None,
        visit_time_unix_ms: earlier_ms,
        page_transition: Some("LINK".to_string()),
        client_id: None,
        favicon_url: None,
        ptoken: None,
    }];
    import_takeout_fixture(&env, &older_records, "older");

    let (url2, title2, count2, hidden2) = read_url_state();
    assert_eq!(url2, "https://example.com/news");
    assert_eq!(
        title2.as_deref(),
        Some("Meaningful Title"),
        "B1 fix for Takeout: older snapshot must NOT overwrite captured title with NULL"
    );
    assert_eq!(
        count2, 2,
        "B1 fix for Takeout: MAX(visit_count) must preserve the higher value (2 > 1)"
    );
    assert_eq!(hidden2, 0, "B1 fix for Takeout: hidden must not flip from older snapshot");
}

// ======================================================================
// T7: Same-URL same-microsecond Takeout records must NOT collapse silently
// ======================================================================

/// T7 — When Google's Takeout export emits multiple records for the same
/// URL within the same microsecond (Chrome sync replay, redirect within
/// 1 µs, multiple devices syncing the same event), they must produce
/// distinct `source_visit_id` values so the
/// `(source_profile_id, source_visit_id)` UNIQUE index doesn't silently
/// drop later records via INSERT OR IGNORE.
///
/// Before the ordinal-tiebreaker fix, `source_visit_id` was derived from
/// `stable_key_i64("{url}:{visit_time_micros}")` alone — identical for
/// every record at the same URL+microsecond. The first record landed;
/// the rest were silently dropped because both UNIQUE indexes (source
/// id + event_fingerprint, since transition=None and app_id="takeout"
/// are constant) fired on every subsequent INSERT OR IGNORE.
///
/// The fix adds `ordinal` (per-record position in the source file) as a
/// tiebreaker. Within a single file, ordinals are unique; across renames
/// of the same file the same record keeps the same ordinal (Google's
/// JSON export is deterministic), so per-record-stability and dedup
/// across path renames both hold.
#[test]
fn t7_takeout_same_url_same_microsecond_records_land_as_distinct_visits() {
    let env = ScenarioEnv::new();
    // Same URL, same visit_time_unix_ms. Two genuinely distinct events
    // (different titles to make the input non-degenerate; in practice
    // they could differ only in transition or page_transition).
    let visit_time_ms = 1_777_680_000_000_i64;

    let records: Vec<TakeoutBrowserRecord> = vec![
        TakeoutBrowserRecord {
            url: "https://example.com/sync-collision".to_string(),
            title: Some("First Event".to_string()),
            visit_time_unix_ms: visit_time_ms,
            page_transition: Some("LINK".to_string()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        },
        TakeoutBrowserRecord {
            url: "https://example.com/sync-collision".to_string(),
            title: Some("Second Event Same Microsecond".to_string()),
            visit_time_unix_ms: visit_time_ms,
            page_transition: Some("LINK".to_string()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        },
    ];
    import_takeout_fixture(&env, &records, "same-microsecond");

    let visits = count_visits_for_profile(&env, "takeout::browser-history");
    assert_eq!(
        visits, 2,
        "Two Takeout records at the same URL+microsecond must produce two distinct visit rows (ordinal tiebreaker), not silently collapse to 1"
    );

    // Cross-path stability check: re-importing the SAME file content
    // (same records in same order) must still dedup — the second pass
    // produces the same ordinals and therefore the same
    // source_visit_ids, so INSERT OR IGNORE catches the dupes.
    import_takeout_fixture(&env, &records, "same-microsecond-reimport");
    let visits_after_reimport = count_visits_for_profile(&env, "takeout::browser-history");
    assert_eq!(
        visits_after_reimport, 2,
        "Re-importing the same file (same records, same ordinals) must dedup, not double the visit count"
    );
}
