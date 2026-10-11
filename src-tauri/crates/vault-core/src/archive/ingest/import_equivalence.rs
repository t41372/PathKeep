//! Responsibilities: pin what a large Takeout import leaves in the archive, so the way it writes
//! can change without changing what it writes.
//! Not responsible for: how fast it writes (`archive_scale_bench/turso_same_visit.rs`), or the
//! small same-visit cases (`dedup_scenarios_takeout`, `same_visit_tests`).
//!
//! Failure modes, written down before the bulk-write work (`WORK-IMPORT-BULK-WRITES`):
//! - a URL seen in several chunks keeps an older title, or loses its counts or first/last time;
//! - a page's search document misses its title, a profile, or its newest visible visit;
//! - Takeout's copy of a browser visit, or My Activity's copy of a Chrome history record in the
//!   same export, counts as a new visit instead of hiding behind the better copy;
//! - the cached totals drift from the visible visits;
//! - an import interrupted after its first chunk and run again ends differently from one that
//!   ran through;
//! - undoing the import and restoring it ends differently from before the undo.
//!
//! The import spans three 10,000-row chunks. The archive is compared as a canonical dump: rows by
//! their natural keys (URL, profile key, source ids), without row ids, run ids or the times they
//! were written. Each table's digest is printed (`IMPORT_EQUIVALENCE`), so two versions of the
//! write path can be compared on the same input.

use super::*;
use browser_history_fixtures::{ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

/// Chrome history records in the export: three chunks of `consumer::BATCH_ROWS`.
const HISTORY_RECORDS: i64 = 21_000;
/// Browser visits, each a copy candidate.
const BROWSER_URLS: i64 = 40;
const BROWSER_VISITS_PER_URL: i64 = 5;
const START_MS: i64 = 1_777_680_000_000;

/// The browser's visits: `BROWSER_URLS` pages, a visit an hour apart.
fn browser_visits() -> Vec<(String, i64)> {
    (0..BROWSER_URLS * BROWSER_VISITS_PER_URL)
        .map(|n| {
            (format!("https://shared.example/page-{}", n % BROWSER_URLS), START_MS + n * 3_600_000)
        })
        .collect()
}

fn browser_snapshot() -> ProfileSnapshot {
    let mut fixture = ChromiumHistoryFixture::new();
    let visits = browser_visits();
    for url in 0..BROWSER_URLS {
        let last =
            visits.iter().filter(|(_, at)| (*at - START_MS) / 3_600_000 % BROWSER_URLS == url);
        fixture = fixture.add_url(ChromiumUrlRow {
            id: url + 1,
            url: format!("https://shared.example/page-{url}"),
            title: Some(format!("Shared page {url}")),
            visit_count: BROWSER_VISITS_PER_URL,
            typed_count: 0,
            last_visit_unix_ms: last.map(|(_, at)| *at).max().unwrap_or(START_MS),
            hidden: false,
        });
    }
    for (n, (_, at)) in visits.iter().enumerate() {
        let n = n as i64;
        fixture = fixture.add_visit(ChromiumVisitRow {
            id: n + 1,
            url_id: n % BROWSER_URLS + 1,
            visit_time_unix_ms: *at,
            from_visit: Some(0),
            transition: Some(805306368),
            visit_duration_micros: Some(5_000_000),
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        });
    }
    let temp_dir = tempdir().expect("snapshot dir");
    let history_path = temp_dir.path().join("History");
    fixture.write(&history_path).expect("write chromium fixture");
    let mut profile = crate::models::BrowserProfile {
        profile_id: "chrome:Default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Google Chrome".to_string(),
        user_name: None,
        profile_path: "/synthetic/chrome:Default".to_string(),
        history_path: Some("/synthetic/chrome:Default/History".to_string()),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("146.0.0.0".to_string()),
        history_file_name: "History".to_string(),
        history_bytes: 0,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    };
    profile.history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
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

/// One Chrome history record: `(url, title, at)`.
type Record = (String, String, i64);

/// The export's Chrome history, in a scrambled order so every chunk holds old and new records:
/// - one page visited throughout, its title changing as it goes, so the newest title is in
///   whichever chunk;
/// - 2,000 pages visited about ten times each, a search results page among every hundred;
/// - copies of the browser's visits at the same millisecond, which hide behind them;
/// - two records of one page in the same microsecond, with different titles: two visits.
fn history_records() -> Vec<Record> {
    let mut records = Vec::new();
    let copies = browser_visits().into_iter().step_by(4);
    for (url, at) in copies {
        let page = url.rsplit('-').next().unwrap_or_default().to_string();
        records.push((url, format!("Shared page {page}"), at));
    }
    let base = START_MS + 400 * 86_400_000;
    for n in 0..300_i64 {
        records.push((
            "https://hot.example/".into(),
            format!("Hot, version {n}"),
            base + n * 3_600_000,
        ));
    }
    let mut n = 0_i64;
    while (records.len() as i64) < HISTORY_RECORDS - 2 {
        let page = n % 2_000;
        let url = if page % 100 == 7 {
            format!("https://www.google.com/search?q=topic+{page}")
        } else {
            format!("https://site-{}.example/page-{page}", page % 37)
        };
        records.push((url, format!("Page {page}"), base + n * 61_000));
        n += 1;
    }
    let twin = base + 999_999;
    records.push(("https://twin.example/".into(), "Twin, first".into(), twin));
    records.push(("https://twin.example/".into(), "Twin, second".into(), twin));
    // A fixed scramble (a multiplicative step coprime with the length) instead of a sort.
    let len = records.len();
    (0..len).map(|i| records[(i * 7_919) % len].clone()).collect()
}

/// My Activity in the same export: copies of Chrome history records, which hide behind them,
/// and records only it has. The copies skip pages with a query, which Google's redirect would
/// carry unencoded.
fn activity_records(history: &[Record]) -> Vec<Record> {
    let mut records: Vec<Record> = history
        .iter()
        .filter(|(url, ..)| !url.contains('?'))
        .step_by(97)
        .take(200)
        .cloned()
        .collect();
    assert_eq!(records.len(), 200);
    let base = START_MS + 900 * 86_400_000;
    records.extend((0..100_i64).map(|n| {
        (format!("https://activity.example/{n}"), format!("Activity {n}"), base + n * 60_000)
    }));
    records
}

fn write_export(root: &std::path::Path) {
    let history = history_records();
    let json = history
        .iter()
        .map(|(url, title, at)| {
            serde_json::json!({"url": url, "title": title, "time_usec": at * 1_000, "page_transition": "LINK"})
        })
        .collect::<Vec<_>>();
    std::fs::create_dir_all(root.join("Chrome")).expect("history folder");
    std::fs::write(
        root.join("Chrome/BrowserHistory.json"),
        serde_json::json!({ "Browser History": json }).to_string(),
    )
    .expect("write history");
    let activity = activity_records(&history)
        .into_iter()
        .map(|(url, title, at)| {
            serde_json::json!({
                "title": format!("Visited {title}"),
                "titleUrl": format!("https://www.google.com/url?q={url}&usg=x"),
                "time": chrono::DateTime::from_timestamp_millis(at).expect("time").to_rfc3339(),
                "products": ["Chrome"],
            })
        })
        .collect::<Vec<_>>();
    let folder = root.join("My Activity").join("Chrome");
    std::fs::create_dir_all(&folder).expect("activity folder");
    std::fs::write(folder.join("MyActivity.json"), serde_json::to_string(&activity).unwrap())
        .expect("write activity");
}

fn vault_with_browser() -> crate::Vault {
    let vault = crate::Vault::open_temp_with_key(Some([61; 32])).expect("vault");
    crate::config::ensure_paths(vault.paths()).expect("paths");
    let probe = crate::test_support::store::Probe::new(&vault);
    probe
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (900, 'backup', 'manual', '2026-05-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
            (),
        )
        .expect("seed run");
    process_profile_snapshot(
        &vault,
        900,
        &browser_snapshot(),
        false,
        false,
        &vault_store::CancelToken::new(),
        None,
    )
    .expect("browser backup");
    vault
}

fn request(root: &std::path::Path) -> crate::models::TakeoutRequest {
    crate::models::TakeoutRequest { source_path: root.display().to_string(), dry_run: false }
}

/// Each table's rows by natural keys, sorted. Takeout visits name the export they came from, a
/// temporary folder here, so it reads `<export>`.
fn dump(vault: &crate::Vault, export: &std::path::Path) -> Vec<(&'static str, Vec<String>)> {
    let probe = crate::test_support::store::Probe::new(vault);
    let export = export.display().to_string();
    let rows = |sql: &str| {
        let mut rows = probe
            .query(sql, (), |row| {
                (0..row.column_count())
                    .map(|i| Ok(format!("{:?}", row.get_value(i)?)))
                    .collect::<vault_store::Result<Vec<_>>>()
                    .map(|values| values.join("|"))
            })
            .expect("dump query")
            .into_iter()
            .map(|row| row.replace(&export, "<export>"))
            .collect::<Vec<_>>();
        rows.sort();
        rows
    };
    vec![
        ("pages", rows("SELECT url, canonical_url, registrable_domain FROM pages")),
        (
            "urls",
            rows(
                "SELECT p.profile_key, u.source_url_id, u.url, u.title, u.visit_count, u.typed_count,
                        u.first_visit_ms, u.first_visit_iso, u.last_visit_ms, u.last_visit_iso, u.hidden,
                        u.payload_hash, g.url
                 FROM urls u JOIN source_profiles p ON p.id=u.source_profile_id
                 JOIN pages g ON g.id=u.page_id",
            ),
        ),
        (
            "visits",
            rows(
                "SELECT p.profile_key, v.source_visit_id, u.url, v.visit_time_ms, v.visit_time_iso,
                        v.transition_type, v.visit_duration_ms, v.from_visit, v.is_known_to_sync,
                        v.visited_link_id, v.external_referrer_url, v.app_id, v.event_fingerprint,
                        v.payload_hash, v.reverted_at IS NULL, v.import_batch_id IS NULL,
                        sp.profile_key, s.source_visit_id
                 FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id
                 JOIN urls u ON u.id=v.url_id
                 LEFT JOIN visits s ON s.id=v.same_as
                 LEFT JOIN source_profiles sp ON sp.id=s.source_profile_id",
            ),
        ),
        (
            "search_terms",
            rows(
                "SELECT p.profile_key, u.url, t.term, t.normalized_term, t.profile_id, t.keyword_id,
                        t.reverted_at IS NULL
                 FROM search_terms t JOIN source_profiles p ON p.id=t.source_profile_id
                 JOIN urls u ON u.id=t.url_id",
            ),
        ),
        (
            "search_documents",
            rows(
                "SELECT g.url, d.url, d.title, d.search_terms, d.normalized_url, d.normalized_title,
                        d.normalized_search_terms, d.cjk_grams, d.enrichment_text, d.notes_text,
                        d.tags_text, d.compact_text, d.domain, d.newest_visit_ms
                 FROM search_documents d JOIN pages g ON g.id=d.page_id",
            ),
        ),
        (
            "search_document_profiles",
            rows(
                "SELECT g.url, p.profile_key, m.visible_visit_count, m.newest_visit_ms
                 FROM search_document_profiles m JOIN pages g ON g.id=m.page_id
                 JOIN source_profiles p ON p.id=m.source_profile_id",
            ),
        ),
        (
            "archive_totals",
            rows(
                "SELECT COALESCE('sourceVisits:' || p.profile_key, t.name), t.value
                 FROM archive_totals t LEFT JOIN source_profiles p ON t.name = 'sourceVisits:' || p.id",
            ),
        ),
    ]
}

fn digests(dump: &[(&'static str, Vec<String>)]) {
    for (table, rows) in dump {
        let mut hash = Sha256::new();
        for row in rows {
            hash.update(row.as_bytes());
            hash.update(b"\n");
        }
        println!("IMPORT_EQUIVALENCE {table} rows={} sha256={:x}", rows.len(), hash.finalize());
    }
}

fn visible(vault: &crate::Vault, profile: &str) -> (i64, i64) {
    crate::test_support::store::Probe::new(vault)
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(v.reverted_at IS NULL), 0) FROM visits v
             JOIN source_profiles p ON p.id=v.source_profile_id WHERE p.profile_key=?1",
            [profile],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count visits")
}

/// The import's own counts, and the claims each failure mode above rests on.
fn check_claims(vault: &crate::Vault) {
    let probe = crate::test_support::store::Probe::new(vault);
    let copies = (BROWSER_URLS * BROWSER_VISITS_PER_URL + 3) / 4;
    assert_eq!(visible(vault, "chrome:Default"), (200, 200), "the browser's visits all count");
    assert_eq!(
        visible(vault, "takeout::browser-history"),
        (HISTORY_RECORDS, HISTORY_RECORDS - copies),
        "Takeout's copies of browser visits hide behind them"
    );
    assert_eq!(
        visible(vault, "takeout::my-activity"),
        (300, 100),
        "My Activity's copies hide behind Chrome history's"
    );
    let (total, counted): (i64, i64) = probe
        .query_row(
            "SELECT (SELECT value FROM archive_totals WHERE name='visits'),
                    (SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL)",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("totals");
    assert_eq!(total, counted, "the cached total is the visible visits");
    let hot: (String, i64, String) = probe
        .query_row(
            "SELECT u.title, u.last_visit_ms, d.title FROM urls u
             JOIN source_profiles p ON p.id=u.source_profile_id
             JOIN search_documents d ON d.page_id=u.page_id
             WHERE u.url='https://hot.example/' AND p.profile_key='takeout::browser-history'",
            (),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the hot page");
    assert_eq!(hot.0, "Hot, version 299", "the newest title wins, whatever chunk it is in");
    assert_eq!(hot.1, START_MS + 400 * 86_400_000 + 299 * 3_600_000);
    assert!(hot.2.contains("Hot, version 299"), "the search document has the newest title");
    let (pages, documents, members): (i64, i64, i64) = probe
        .query_row(
            "SELECT (SELECT COUNT(DISTINCT u.page_id) FROM urls u JOIN visits v ON v.url_id=u.id
                     WHERE v.reverted_at IS NULL),
                    (SELECT COUNT(*) FROM search_documents WHERE newest_visit_ms IS NOT NULL
                     AND title <> ''),
                    (SELECT COUNT(*) FROM (SELECT DISTINCT u.page_id, v.source_profile_id
                     FROM urls u JOIN visits v ON v.url_id=u.id WHERE v.reverted_at IS NULL))",
            (),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("documents");
    assert_eq!(documents, pages, "every page with a visible visit has a titled, dated document");
    let listed: i64 = probe
        .query_row("SELECT COUNT(*) FROM search_document_profiles", (), |row| Ok(row.get(0)?))
        .expect("memberships");
    assert_eq!(listed, members, "one membership per page and profile with visible visits");
}

#[test]
fn a_large_import_ends_the_same_however_it_is_written() {
    let export = tempdir().expect("export");
    write_export(export.path());

    let vault = vault_with_browser();
    let imported = crate::takeout::import_takeout(&vault, &request(export.path())).expect("import");
    assert_eq!(imported.same_visit_items, 50 + 200, "copies already in the archive");
    check_claims(&vault);
    let through = dump(&vault, export.path());
    digests(&through);

    // Undone and restored, the archive is as it was.
    let batch = imported.import_batch.expect("a batch").id;
    crate::takeout::revert_import_batch(&vault, batch).expect("undo");
    assert_eq!(visible(&vault, "takeout::browser-history").1, 0);
    crate::takeout::restore_import_batch(&vault, batch).expect("restore");
    assert!(dump(&vault, export.path()) == through, "undo then restore ends as before the undo");

    // Stopped after its first chunk and run again, the import ends as one that ran through.
    let vault = vault_with_browser();
    let cancel = vault_store::CancelToken::new();
    let stopped = crate::takeout::import_takeout_with_control(
        &vault,
        &request(export.path()),
        &cancel,
        |event| {
            if event.imported_records.is_some_and(|n| n >= 10_000) {
                cancel.cancel();
            }
        },
    );
    assert!(stopped.is_err(), "the import stopped");
    crate::takeout::import_takeout(&vault, &request(export.path())).expect("run again");
    check_claims(&vault);
    assert!(
        dump(&vault, export.path()) == through,
        "an interrupted import, run again, ends the same"
    );
}
