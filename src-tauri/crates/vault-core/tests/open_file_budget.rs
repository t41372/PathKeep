//! Responsibilities: an open archive's open files stay bounded however long it runs.
//! Not responsible for: raising the process limit (`vault_platform::raise_open_file_limit`).
//! Failure modes: every index commit or search session keeps files open, so the
//! count grows with the archive's age until "Too many open files"; databases or
//! index readers are reopened per request and never closed.
//!
//! One test per binary: the count covers the whole process.

use browser_history_fixtures::{TakeoutBrowserHistoryFixture, TakeoutBrowserRecord};
use vault_core::search::{SearchRequest, SearchSession};
use vault_core::{TakeoutRequest, Vault};

fn open_files() -> usize {
    let dir = if cfg!(target_os = "linux") { "/proc/self/fd" } else { "/dev/fd" };
    std::fs::read_dir(dir).unwrap().count()
}

fn import(vault: &Vault, round: i64) {
    let source = tempfile::tempdir().unwrap();
    let mut fixture = TakeoutBrowserHistoryFixture::new();
    for item in 0..100 {
        fixture = fixture.add_record(TakeoutBrowserRecord {
            url: format!("https://budget{round}.test/{item}"),
            title: Some(format!("Budget page {item} of round {round}")),
            visit_time_unix_ms: 1_800_000_000_000 + round * 1_000_000 + item,
            page_transition: Some("LINK".into()),
            client_id: None,
            favicon_url: None,
            ptoken: None,
        });
    }
    fixture.write(&source.path().join("Chrome/BrowserHistory.json")).unwrap();
    let request =
        TakeoutRequest { source_path: source.path().display().to_string(), dry_run: false };
    assert_eq!(vault_core::import_takeout(vault, &request).unwrap().imported_items, 100);
    vault.search().cycle().unwrap();
}

fn search(vault: &Vault, query: &str, regex: bool) -> SearchSession {
    let request = SearchRequest { query: query.into(), regex, ..Default::default() };
    let mut session = SearchSession::start(vault, request).unwrap();
    assert!(!session.finish_page().unwrap().items.is_empty());
    session
}

#[test]
fn open_files_stay_bounded_as_imports_and_searches_accumulate() {
    let vault = Vault::open_temp_with_key(Some([77; 32])).unwrap();
    let mut counts = Vec::new();
    for round in 0..64 {
        import(&vault, round);
        drop(search(&vault, "budget", false));
        drop(search(&vault, "budget.*round", true));
        if round % 8 == 7 {
            counts.push(open_files());
        }
    }
    println!("open files every 8 rounds: {counts:?}");
    // Index segments merge, so more commits mean no more open files. One segment
    // is six files, each open once per reader generation; allow a few in flight.
    let (first, most) = (counts[0], *counts.iter().max().unwrap());
    assert!(most <= first + 24, "open files grew from {first} to {most} as imports accumulated");
    // Several searches open at once each pin an index snapshot; closing them releases it.
    let settled = open_files();
    let sessions = ["budget", "page", "round", "of"].map(|query| search(&vault, query, false));
    let with_sessions = open_files();
    drop(sessions);
    println!("settled {settled}, with four open searches {with_sessions}, after {}", open_files());
    assert!(with_sessions <= settled + 24, "each open search opened its own files");
    assert!(open_files() <= settled, "closed searches kept files open");
}
