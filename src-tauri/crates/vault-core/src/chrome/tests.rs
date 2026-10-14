//! Regression tests for browser discovery and staging helpers.
use super::*;
use crate::config::{ProjectPaths, project_paths_with_root};
use tempfile::tempdir;

fn sample_paths(root: &Path) -> ProjectPaths {
    project_paths_with_root(root)
}

#[test]
fn no_two_browsers_share_a_default_profile_root() {
    // On a real machine each browser must look in its own folder, or one profile would be archived
    // under several browser names. Compared case-insensitively for macOS and Windows file systems.
    type PerPlatform = (Vec<&'static str>, Vec<&'static str>, Vec<&'static str>);
    let roots: Vec<(&str, PerPlatform)> = CHROMIUM_BROWSERS
        .iter()
        .map(|definition| (definition.key, chromium_relative_paths(definition.key)))
        .chain(
            FIREFOX_BROWSERS
                .iter()
                .map(|definition| (definition.key, firefox_relative_paths(definition.key))),
        )
        .collect();
    for platform in 0..3 {
        let mut seen = std::collections::BTreeMap::new();
        for (key, per_platform) in &roots {
            let paths = match platform {
                0 => &per_platform.0,
                1 => &per_platform.1,
                _ => &per_platform.2,
            };
            for path in paths {
                if let Some(other) = seen.insert(path.to_ascii_lowercase(), *key) {
                    panic!("{key} and {other} share the profile root {path}");
                }
            }
        }
    }
}

#[test]
fn fallback_chromium_profiles_support_direct_history_layouts() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("History"), b"history").expect("write history");
    fs::write(dir.path().join("Favicons"), b"favicons").expect("write favicons");

    let definition = CHROMIUM_BROWSERS
        .iter()
        .copied()
        .find(|definition| definition.key == "opera")
        .expect("opera definition");
    let profiles =
        fallback_chromium_profiles(definition, dir.path(), Some("118.0")).expect("profiles");

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile_id, "opera:default");
    assert_eq!(profiles[0].browser_name, "Opera");
    assert!(profiles[0].history_exists);
    assert_eq!(profiles[0].browser_version.as_deref(), Some("118.0"));
}

/// Reopens a cleanly written SQLite database, starts an uncommitted write, and
/// flushes its dirty pages to disk so a populated rollback journal lands beside
/// it, then copies that hot `(database, journal)` pair into `dest_dir` under
/// `base_name`.
///
/// The result reproduces a browser caught mid-write whose writer then vanished —
/// the exact state that fails a read-only open with `SQLITE_READONLY_ROLLBACK`,
/// which is what broke backups when the staging fallback handed it to the parser.
fn capture_hot_journal_pair(clean_db: &Path, dest_dir: &Path, base_name: &str, dirty_sql: &str) {
    use rusqlite::Connection;

    let source_journal = PathBuf::from(format!("{}-journal", clean_db.display()));
    let writer = Connection::open(clean_db).expect("reopen clean database");
    writer
        .execute_batch(&format!("BEGIN IMMEDIATE;\n{dirty_sql}"))
        .expect("start uncommitted write");
    writer.cache_flush().expect("flush dirty pages into the rollback journal");
    assert!(source_journal.exists(), "an uncommitted flush must leave a rollback journal");

    fs::copy(clean_db, dest_dir.join(base_name)).expect("copy database into staging source");
    fs::copy(&source_journal, dest_dir.join(format!("{base_name}-journal")))
        .expect("copy hot journal into staging source");

    writer.execute_batch("ROLLBACK").ok();
}

#[test]
fn stage_profile_snapshot_recovers_a_hot_journal_for_the_chromium_parser() {
    use browser_history_fixtures::chromium::{
        ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow,
    };
    use browser_history_parser::{
        chromium,
        types::{ChromiumReadCursor, HistoryDatabaseSet},
    };

    let visit_time = 1_777_000_000_000_i64;

    // A real-format Chrome History database, written cleanly...
    let clean = tempdir().expect("clean db dir");
    let clean_db = clean.path().join("History");
    ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/a".to_string(),
            title: Some("Committed Title".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_time,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 10,
            url_id: 1,
            visit_time_unix_ms: visit_time,
            from_visit: None,
            transition: Some(0),
            visit_duration_micros: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        })
        .write(&clean_db)
        .expect("write chrome fixture");

    // ...then captured mid-write with a hot rollback journal in the profile dir.
    let profile_dir = tempdir().expect("profile dir");
    capture_hot_journal_pair(
        &clean_db,
        profile_dir.path(),
        "History",
        "UPDATE urls SET title = 'in-flight' WHERE id = 1;",
    );

    let staging_root = tempdir().expect("staging root");
    let paths = sample_paths(staging_root.path());
    fs::create_dir_all(&paths.staging_dir).expect("create staging dir");

    let profile = BrowserProfile {
        profile_id: "chrome:Default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Google Chrome".to_string(),
        user_name: None,
        profile_path: profile_dir.path().display().to_string(),
        history_path: Some(profile_dir.path().join("History").display().to_string()),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: None,
        history_file_name: "History".to_string(),
        history_bytes: 0,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: retention_boundary_for_browser("chromium"),
    };

    let staged = stage_profile_snapshot(&paths, &profile).expect("stage hot-journal profile");

    // The degraded staging path was recorded as a run warning, not swallowed.
    assert_eq!(staged.warnings.len(), 1, "the recovered-copy fallback must be reported");
    assert!(staged.warnings[0].message.contains("chrome:Default"), "the warning names the profile");
    assert_eq!(staged.warnings[0].code, "staging-fallback-recovered-copy");
    assert_eq!(staged.warnings[0].profile_id.as_deref(), Some("chrome:Default"));
    assert!(
        staged.warnings[0].diagnostic.is_some(),
        "the snapshot failure reason rides in `diagnostic`"
    );

    // The production read path opens the recovered copy and parses the last
    // committed state — no SQLITE_READONLY_ROLLBACK, in-flight write rolled back.
    let parsed = chromium::parse_history(
        &HistoryDatabaseSet {
            history_path: staged.snapshot.history_path.clone(),
            favicons_path: None,
        },
        ChromiumReadCursor::default(),
    )
    .expect("parse recovered chrome copy");
    assert_eq!(parsed.urls.len(), 1, "the committed url survives recovery");
    assert_eq!(parsed.visits.len(), 1, "the committed visit survives recovery");
}

#[test]
fn staging_backs_up_a_live_wal_source_to_a_complete_journal_free_copy() {
    use rusqlite::{Connection, OpenFlags};

    // A WAL database with committed rows still living in the -wal (autocheckpoint
    // off, writer kept open), captured the way a live Firefox `places.sqlite`
    // would be. Whichever path staging takes — the online snapshot reads WAL
    // frames directly, the fallback recovers them — the result must be complete.
    let live = tempdir().expect("live");
    let live_db = live.path().join("History");
    let writer = Connection::open(&live_db).expect("open wal db");
    writer
        .execute_batch(
            "PRAGMA journal_mode=WAL;\n\
             PRAGMA wal_autocheckpoint=0;\n\
             CREATE TABLE t (id INTEGER PRIMARY KEY, payload TEXT);\n\
             INSERT INTO t (id, payload) VALUES (1, 'a'), (2, 'b'), (3, 'c');",
        )
        .expect("seed wal rows");
    assert!(live.path().join("History-wal").exists(), "writer must leave a WAL");

    let source = tempdir().expect("source");
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", live_db.display()));
        if from.exists() {
            fs::copy(&from, source.path().join(format!("History{suffix}")))
                .expect("copy wal member");
        }
    }
    drop(writer);

    let dest = tempdir().expect("dest");
    let staged =
        copy_database_with_sidecars(source.path(), "History", dest.path()).expect("stage wal copy");

    // The copy the parser receives is self-contained: every committed row is
    // present and a read-only open needs no WAL sidecar.
    assert!(!dest.path().join("History-wal").exists(), "staged copy carries no live WAL");
    let reader = Connection::open_with_flags(
        &staged.path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("read-only open of staged wal copy");
    let count: i64 =
        reader.query_row("SELECT count(*) FROM t", [], |row| row.get(0)).expect("count rows");
    assert_eq!(count, 3, "all committed WAL rows survive staging");
}
