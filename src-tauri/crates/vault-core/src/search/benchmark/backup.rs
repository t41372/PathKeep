//! Responsibilities: measure real backup/indexer contention and interactive visibility.
//! Not responsible for: browser discovery, simulated writes or latency extrapolation.
//! Failure modes: baselines retain a backlog; fixtures grow without bounds; work outside
//! the backup is mislabeled throughput; a successful note write is not searchable.
use super::{fixture, measure};
use crate::{BrowserProfile, SetNotesRequest, SetStarRequest, StarEntityKind, Vault};
use browser_history_fixtures::{ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow};
use serde_json::json;
use std::time::Instant;
const PAGES: i64 = 10_000;

pub(super) fn run(vault: &Vault) {
    for (run, enabled) in [false, true, true, false].into_iter().enumerate() {
        let profile = source(vault, run as i64);
        vault.search().cycle().unwrap();
        let pause = (!enabled).then(|| vault.search().pause());
        let before = vault.search().state();
        let started = Instant::now();
        crate::archive::run_backup_profiles(
            vault,
            false,
            &vault_store::CancelToken::new(),
            vec![profile],
            |_| {},
        )
        .unwrap();
        let seconds = started.elapsed().as_secs_f64();
        let after = vault.search().state();
        println!(
            "{}",
            json!({"case":"backup","run":run,"indexerEnabled":enabled,"visits":PAGES*2,"pagesTouched":PAGES,"seconds":seconds,"visitsPerSecond":(PAGES*2) as f64/seconds,"outboxRowsAppliedDuringBackup":after.applied_seq-before.applied_seq,"documentsIndexedDuringBackup":after.documents_processed-before.documents_processed,"documentsPerSecond":(after.documents_processed-before.documents_processed) as f64/seconds,"peakRssMiB":measure::peak(),"steadyRssMiB":measure::rss(),"uptime":measure::uptime()})
        );
        drop(pause);
        vault.search().cycle().unwrap();
        assert!(measure::disk(&vault.paths().app_root) < 40_000_000_000);
    }
    let profile = source(vault, 4);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let writer = vault.clone();
    let interactive = std::thread::spawn(move || {
        rx.recv().unwrap();
        let start = Instant::now();
        let url = "https://site1.example/articles/1";
        crate::set_star(
            &writer,
            SetStarRequest {
                entity_kind: StarEntityKind::Url,
                entity_key: url.into(),
                source_profile: None,
            },
        )
        .unwrap();
        crate::set_notes(
            &writer,
            SetNotesRequest {
                url: url.into(),
                notes: "interactionvisibilitysentinel".into(),
                source_profile: None,
            },
        )
        .unwrap();
        let commit_ms = start.elapsed().as_secs_f64() * 1000.;
        writer.search().cycle().unwrap();
        let cycle_ms = start.elapsed().as_secs_f64() * 1000.;
        let request = super::SearchRequest {
            query: "interactionvisibilitysentinel".into(),
            starred: true,
            ..Default::default()
        };
        let result = super::SearchSession::start(&writer, request).unwrap().finish_page().unwrap();
        assert_eq!(result.count.value, 1);
        println!(
            "{}",
            json!({"case":"interactive-during-backup","writeCommitMs":commit_ms,"oneCycleVisibleMs":cycle_ms,"queryVerifiedMs":start.elapsed().as_secs_f64()*1000.,"uptime":measure::uptime()})
        );
    });
    let mut notified = false;
    crate::archive::run_backup_profiles(
        vault,
        false,
        &vault_store::CancelToken::new(),
        vec![profile],
        |event| {
            if !notified && event.imported_records.is_some_and(|n| n >= 10_000) {
                notified = true;
                tx.send(()).unwrap();
            }
        },
    )
    .unwrap();
    assert!(notified);
    interactive.join().unwrap();
    vault.search().cycle().unwrap();
}
fn source(vault: &Vault, run: i64) -> BrowserProfile {
    let root = vault.paths().app_root.join(format!("source-{run}"));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("History");
    let mut data = ChromiumHistoryFixture::new();
    for offset in 1..=PAGES {
        let id = run * PAGES + offset;
        let time = fixture::BASE + 200_000_000_000 + id * 2000;
        data = data.add_url(ChromiumUrlRow {
            id,
            url: format!("https://site{}.example/articles/{id}", id % 20000),
            title: Some("archive backup calibration".into()),
            visit_count: 2,
            typed_count: 0,
            last_visit_unix_ms: time + 1000,
            hidden: false,
        });
        for step in 0..2 {
            data = data.add_visit(ChromiumVisitRow {
                id: offset * 2 + step,
                url_id: id,
                visit_time_unix_ms: time + step * 1000,
                from_visit: None,
                transition: Some(1),
                visit_duration_micros: None,
                is_known_to_sync: false,
                visited_link_id: None,
                external_referrer_url: None,
                app_id: None,
            });
        }
    }
    data.write(&path).unwrap();
    BrowserProfile {
        profile_id: format!("chrome:search-bench-{run}"),
        profile_name: format!("search-bench-{run}"),
        browser_family: "chromium".into(),
        browser_name: "Chrome".into(),
        profile_path: root.display().to_string(),
        history_path: Some(path.display().to_string()),
        history_exists: true,
        history_readable: true,
        history_file_name: "History".into(),
        user_name: None,
        favicons_path: None,
        access_issue: None,
        browser_version: None,
        history_bytes: 0,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: Default::default(),
    }
}
