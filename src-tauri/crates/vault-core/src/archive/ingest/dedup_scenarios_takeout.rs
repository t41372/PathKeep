//! Responsibilities: verify parser-to-archive dedup contracts on real Turso.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: duplicate visits, changed URL metadata, cursor loss, timestamp
//! truncation, lost optional fields or source evidence. Scenarios below pin each.

//! Takeout-family dedup scenarios (T1, T2, T2b, T3, T5).
//!
//! Covers the Google Takeout BrowserHistory JSON import path and its
//! interaction with local-Chrome backups. Each scenario pins a specific
//! dedup contract documented in the audit:
//!
//! - **T1** — Takeout baseline import (happy path).
//! - **T2** — File-rename re-import deduplicates via fingerprint partial index.
//! - **T2b** — Fingerprint divergence (title drift) exposes B3.
//! - **T3** — Takeout × local Chrome: both kept, counted once, in either order.
//! - **T5** — `time_usec` unit contract (B6 pinning).
//! - **T8** — My Activity × Takeout × Chrome: the best source counts, groups stay flat.
//! - **T9** — Undo and restore move which copy counts.
//! - **T10** — Insights count a visit from two sources once.

use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, TakeoutBrowserHistoryFixture,
    TakeoutBrowserRecord,
};
use tempfile::tempdir;

// ======================================================================
// Shared helpers (per satellite-module pattern — each #[cfg(test)] module
// owns its own ScenarioEnv)
// ======================================================================

struct ScenarioEnv {
    vault: crate::Vault,
}

impl ScenarioEnv {
    fn new(key: Option<[u8; 32]>) -> Self {
        let vault = crate::Vault::open_temp_with_key(key).expect("temporary vault");
        let paths = vault.paths().clone();
        crate::config::ensure_paths(&paths).expect("ensure paths");
        Self { vault }
    }

    fn open_archive(&self) -> crate::test_support::store::Probe {
        crate::test_support::store::Probe::new(&self.vault)
    }
}

fn seed_run(archive: &crate::test_support::store::Probe, run_id: i64) {
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
    seed_run(&env.open_archive(), run_id);
    process_profile_snapshot(
        &env.vault,
        run_id,
        snapshot,
        false,
        use_watermark,
        &vault_store::CancelToken::new(),
        None,
    )
    .expect("process profile snapshot")
}

fn count_urls_for_profile(env: &ScenarioEnv, profile_key: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT COUNT(*) FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1",
            [profile_key],
            |row| Ok(row.get(0)?),
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
            |row| Ok(row.get(0)?),
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
        &env.vault,
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
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let source_root = tempdir().expect("takeout source root");
        let payload_path = source_root.path().join("Chrome/BrowserHistory.json");

        TakeoutBrowserHistoryFixture::new()
            .add_record(takeout_record(
                "https://example.com/page-one",
                "Page One",
                1_777_680_000_000,
            ))
            .add_record(takeout_record(
                "https://example.com/page-two",
                "Page Two",
                1_777_809_600_000,
            ))
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

        let inspection =
            crate::takeout::import_takeout(&env.vault, &request).expect("import takeout");

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
                |row| Ok(row.get(0)?),
            )
            .expect("takeout app_id count");
        assert_eq!(takeout_visit_count, 3);
    }
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
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

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
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

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
}

// ======================================================================
// T3: Takeout x local Chrome same-period overlap — B4 contract
// ======================================================================

/// What each source holds and counts of the same visit (`archive::same_visit`).
#[derive(Debug, PartialEq, Eq)]
struct SameVisits {
    /// `(profile, rows, counted)` per source, by profile key.
    sources: Vec<(String, i64, i64)>,
    /// `archive_totals` 'visits'.
    total: i64,
    /// Hidden copies whose stand-in is itself hidden, or that point at nothing: groups are flat.
    dangling: i64,
}

fn same_visits(env: &ScenarioEnv) -> SameVisits {
    let archive = env.open_archive();
    let sources = archive
        .query(
            "SELECT p.profile_key, COUNT(v.id), COALESCE(SUM(CASE WHEN v.reverted_at IS NULL THEN 1 ELSE 0 END),0)
             FROM source_profiles p JOIN visits v ON v.source_profile_id=p.id
             GROUP BY p.profile_key ORDER BY p.profile_key",
            (),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("count sources");
    let total =
        archive
            .query_row("SELECT value FROM archive_totals WHERE name='visits'", (), |row| {
                Ok(row.get(0)?)
            })
            .expect("total");
    let dangling = archive
        .query_row(
            "SELECT COUNT(*) FROM visits c LEFT JOIN visits s ON s.id=c.same_as
             WHERE c.same_as IS NOT NULL AND (s.id IS NULL OR s.reverted_at IS NOT NULL)",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("dangling");
    let counted = archive
        .query(
            "SELECT p.id, COALESCE(SUM(CASE WHEN v.reverted_at IS NULL THEN 1 ELSE 0 END),0) FROM source_profiles p
             LEFT JOIN visits v ON v.source_profile_id=p.id GROUP BY p.id",
            (),
            |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
        )
        .expect("counted per source");
    for (profile, counted) in counted {
        let cached = archive
            .query(
                "SELECT value FROM archive_totals WHERE name=?1",
                [format!("sourceVisits:{profile}")],
                |row| Ok(row.get::<i64>(0)?),
            )
            .expect("source total")
            .pop()
            .unwrap_or(0);
        assert_eq!(cached, counted, "the cached count of source {profile} is what it counts");
    }
    SameVisits { sources, total, dangling }
}

fn sources(rows: &[(&str, i64, i64)], total: i64) -> SameVisits {
    SameVisits {
        sources: rows.iter().map(|(key, all, counted)| (key.to_string(), *all, *counted)).collect(),
        total,
        dangling: 0,
    }
}

fn three_day_chrome() -> ProfileSnapshot {
    let (day_one, day_two, day_three) =
        (1_777_680_000_000_i64, 1_777_809_600_000, 1_777_872_930_000);
    let mut fixture = ChromiumHistoryFixture::new();
    for (id, name, at) in [(1, "one", day_one), (2, "two", day_two), (3, "three", day_three)] {
        fixture = fixture
            .add_url(ChromiumUrlRow {
                id,
                url: format!("https://example.com/shared-{name}"),
                title: Some(format!("Shared {name}")),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: at,
                hidden: false,
            })
            .add_visit(visit_row(id + 9, id, at));
    }
    snapshot_for_fixture(&fixture, chromium_profile("chrome:Default", "Google Chrome"))
}

/// The same three visits as `three_day_chrome`, in Takeout's copy of Chrome's history.
fn three_day_takeout() -> Vec<TakeoutBrowserRecord> {
    [("one", 1_777_680_000_000_i64), ("two", 1_777_809_600_000), ("three", 1_777_872_930_000)]
        .into_iter()
        .map(|(name, at)| {
            takeout_record(
                &format!("https://example.com/shared-{name}"),
                &format!("Shared {name}"),
                at,
            )
        })
        .collect()
}

/// The same three visits in My Activity's JSON (behind Google's redirect, with its prefix).
fn write_three_day_activity(root: &std::path::Path) {
    let folder = root.join("My Activity").join("Chrome");
    std::fs::create_dir_all(&folder).expect("activity folder");
    let records = [("one", 1_777_680_000_000_i64), ("two", 1_777_809_600_000), ("three", 1_777_872_930_000)]
        .into_iter()
        .map(|(name, at)| {
            serde_json::json!({
                "title": format!("Visited Shared {name}"),
                "titleUrl": format!("https://www.google.com/url?q=https://example.com/shared-{name}&usg=x"),
                "time": chrono::DateTime::from_timestamp_millis(at).unwrap().to_rfc3339(),
                "products": ["Chrome"],
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(folder.join("MyActivity.json"), serde_json::to_string(&records).unwrap())
        .expect("write activity");
}

fn import_folder(env: &ScenarioEnv, root: &std::path::Path) -> crate::models::TakeoutInspection {
    crate::takeout::import_takeout(
        &env.vault,
        &crate::models::TakeoutRequest { source_path: root.display().to_string(), dry_run: false },
    )
    .expect("import takeout")
}

fn batch_of(inspection: &crate::models::TakeoutInspection) -> i64 {
    inspection.import_batch.as_ref().expect("an import batch").id
}

/// T3 — Takeout's copy of a visit and the local Chrome's own are the same visit: both rows are
/// kept, the Chrome one counts (`same-visit-across-sources.md`), whichever came first. The
/// import says how many it already had from another source; a backup that takes over Takeout's
/// copies names Takeout, whose Insights then change.
#[test]
fn t3_takeout_and_local_chrome_keep_both_and_count_once() {
    for key in [None, Some([83; 32])] {
        // Chrome first: Takeout's copies are written hidden behind Chrome's.
        let env = ScenarioEnv::new(key);
        run_one_ingest(&env, 900, &three_day_chrome(), false);
        import_takeout_fixture(&env, &three_day_takeout(), "after-chrome");
        assert_eq!(
            same_visits(&env),
            sources(&[("chrome:Default", 3, 3), ("takeout::browser-history", 3, 0)], 3)
        );

        // Takeout first: Chrome's backup takes over, and says whose copies it took over.
        let env = ScenarioEnv::new(key);
        import_takeout_fixture(&env, &three_day_takeout(), "before-chrome");
        assert_eq!(same_visits(&env), sources(&[("takeout::browser-history", 3, 3)], 3));
        let summary = run_one_ingest(&env, 900, &three_day_chrome(), false);
        assert_eq!(summary.new_visits, 3);
        assert_eq!(summary.replaced_profiles, vec!["takeout::browser-history".to_string()]);
        assert_eq!(
            same_visits(&env),
            sources(&[("chrome:Default", 3, 3), ("takeout::browser-history", 3, 0)], 3)
        );
    }
}

/// T8 — My Activity, Takeout's Chrome history and the browser hold one visit: each better
/// source takes over as it arrives, and every copy points at the one that counts.
#[test]
fn t8_my_activity_takeout_and_chrome_count_one_visit_once() {
    let env = ScenarioEnv::new(None);
    let activity = tempdir().expect("activity root");
    write_three_day_activity(activity.path());
    let imported = import_folder(&env, activity.path());
    assert_eq!((imported.imported_items, imported.same_visit_items), (3, 0));

    import_takeout_fixture(&env, &three_day_takeout(), "history");
    assert_eq!(
        same_visits(&env),
        sources(&[("takeout::browser-history", 3, 3), ("takeout::my-activity", 3, 0)], 3)
    );

    run_one_ingest(&env, 900, &three_day_chrome(), false);
    assert_eq!(
        same_visits(&env),
        sources(
            &[
                ("chrome:Default", 3, 3),
                ("takeout::browser-history", 3, 0),
                ("takeout::my-activity", 3, 0),
            ],
            3
        )
    );

    // My Activity again: written, all already counted elsewhere.
    let again = tempdir().expect("activity again");
    write_three_day_activity(again.path());
    let imported = import_folder(&env, again.path());
    assert_eq!(
        (imported.imported_items, imported.same_visit_items),
        (0, 0),
        "same source: duplicates"
    );
}

/// T9 — Undoing the import whose copy counts lets the best other copy count; restoring it links
/// it again. Undoing an import whose copies were hidden changes nothing that counts.
#[test]
fn t9_undo_and_restore_move_which_copy_counts() {
    let env = ScenarioEnv::new(None);
    let history = tempdir().expect("history root");
    let payload = history.path().join("Chrome/BrowserHistory.json");
    let mut fixture = TakeoutBrowserHistoryFixture::new();
    for record in three_day_takeout() {
        fixture = fixture.add_record(record);
    }
    fixture.write(&payload).expect("write history");
    let history_batch = batch_of(&import_folder(&env, history.path()));
    let activity = tempdir().expect("activity root");
    write_three_day_activity(activity.path());
    let activity_import = import_folder(&env, activity.path());
    assert_eq!((activity_import.imported_items, activity_import.same_visit_items), (0, 3));
    let activity_batch = batch_of(&activity_import);

    crate::takeout::revert_import_batch(&env.vault, history_batch).expect("undo history");
    assert_eq!(
        same_visits(&env),
        sources(&[("takeout::browser-history", 3, 0), ("takeout::my-activity", 3, 3)], 3)
    );

    crate::takeout::restore_import_batch(&env.vault, history_batch).expect("restore history");
    assert_eq!(
        same_visits(&env),
        sources(&[("takeout::browser-history", 3, 3), ("takeout::my-activity", 3, 0)], 3)
    );

    crate::takeout::revert_import_batch(&env.vault, activity_batch).expect("undo activity");
    crate::takeout::restore_import_batch(&env.vault, activity_batch).expect("restore activity");
    assert_eq!(
        same_visits(&env),
        sources(&[("takeout::browser-history", 3, 3), ("takeout::my-activity", 3, 0)], 3)
    );
}

/// Insights' visits per day over every source, from the published rollups.
fn insights_days(env: &ScenarioEnv) -> Vec<(String, i64)> {
    crate::intelligence::run_core_intelligence(
        &env.vault,
        &crate::models::CoreIntelligenceRebuildRequest::default(),
    )
    .expect("build Insights");
    crate::test_support::store::Probe::for_db(&env.vault, vault_store::Db::Intelligence)
        .query(
            "SELECT date_key, SUM(total_visits) FROM daily_summary_rollups
             GROUP BY date_key ORDER BY date_key",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("days")
}

/// T10 — Insights count a visit once: Takeout's copies were counted until the browser's backup
/// took over, and the next build gives the days the browser alone gives (the Takeout profile,
/// with fewer counted visits, is built again).
#[test]
fn t10_insights_count_a_visit_from_two_sources_once() {
    let both = ScenarioEnv::new(None);
    import_takeout_fixture(&both, &three_day_takeout(), "takeout first");
    assert_eq!(insights_days(&both).iter().map(|(_, n)| n).sum::<i64>(), 3);
    run_one_ingest(&both, 900, &three_day_chrome(), false);

    let alone = ScenarioEnv::new(None);
    run_one_ingest(&alone, 900, &three_day_chrome(), false);
    let want = insights_days(&alone);
    assert_eq!(want.iter().map(|(_, n)| n).sum::<i64>(), 3);
    assert_eq!(insights_days(&both), want);
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
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
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
            &env.vault,
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
                (),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query takeout visit time");

        assert_eq!(
            visit_time_ms, visit_one,
            "Takeout time_usec must round-trip as Unix milliseconds"
        );
        assert!(
            visit_time_iso.starts_with("2026-05-02"),
            "Takeout ISO must reflect 2026-05-02, got {visit_time_iso}"
        );
    }
}

// ======================================================================
// T6: Takeout URL upsert B1 protection — older-snapshot re-import must not regress
// ======================================================================

/// T6 — Audit bug B1 was originally identified and fixed in
/// `archive/ingest/writes.rs::upsert_url`, but the
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
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
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
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
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
}
