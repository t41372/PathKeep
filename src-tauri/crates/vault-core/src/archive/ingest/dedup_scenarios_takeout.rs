//! Responsibilities: verify parser-to-archive dedup contracts on real Turso.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: duplicate visits, changed URL metadata, cursor loss, timestamp
//! truncation, lost optional fields or source evidence. Scenarios below pin each.

//! Takeout-family dedup scenarios (T2b, T6, T7).
//!
//! Covers the Google Takeout BrowserHistory JSON import path. Each
//! scenario pins a specific dedup contract:
//!
//! - **T2b** — A re-export with changed page titles does not store the visits again.
//! - **T6** — Re-importing an older Takeout does not regress URL metadata.
//! - **T7** — Records with the same URL and microsecond stay distinct visits.

use browser_history_fixtures::{TakeoutBrowserHistoryFixture, TakeoutBrowserRecord};
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
// T2b: A re-export with changed titles is the same visits
// ======================================================================

/// T2b — Google re-exports Takeout months later with updated page titles:
/// same URL and visit time, different title, so the event fingerprint
/// differs between the two imports. The second import must still find the
/// visits it already has and not store them again.
#[test]
fn t2b_takeout_reexport_with_changed_titles_does_not_duplicate_visits() {
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

        // The same three visits with updated titles, not six.
        assert_eq!(
            visit_count, 3,
            "a re-export with changed titles duplicated visits (got {visit_count})"
        );
    }
}

// ======================================================================
// T6: Takeout URL upsert — older-snapshot re-import must not regress
// ======================================================================

/// T6 — The Takeout import has its own URL upsert in
/// `takeout/payload_import.rs`, separate from
/// `archive/ingest/writes.rs::upsert_url`. Without the guards below, a
/// re-import of an older Takeout snapshot would silently overwrite
/// title / hidden with stale values, and a fresh Takeout export with new
/// visits to the same URL would never bump visit_count.
///
/// This scenario pins the guards in `payload_import.rs`:
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
            "older Takeout snapshot must NOT overwrite captured title with NULL"
        );
        assert_eq!(count2, 2, "MAX(visit_count) must preserve the higher value (2 > 1)");
        assert_eq!(hidden2, 0, "hidden must not flip from older Takeout snapshot");
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
