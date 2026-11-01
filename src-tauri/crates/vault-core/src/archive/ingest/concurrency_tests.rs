//! Responsibilities: prove committed batch recovery and interactive write scheduling.
//! Not responsible for: scheduler IPC or browser-owned SQLite locking.
//! Failure modes: batch-size-dependent results, cancelled/crashed run loses its
//! committed prefix, resume duplicates visits, stars wait until the backup ends,
//! contending writes read an uncommitted watermark; timeline reads wait on an open backup batch; early URL cursors skip search terms;
//! resumed visits are rescanned; downloads or favicons disappear after a crash.

use super::*;
use crate::Vault;
use browser_history_fixtures::{
    ChromiumDownloadRow, ChromiumFaviconRow, ChromiumHistoryFixture, ChromiumIconMappingRow,
    ChromiumKeywordSearchTermRow, ChromiumUrlRow, ChromiumVisitRow,
};
use vault_store::Db;

fn snapshot(rows: i64) -> ProfileSnapshot {
    let temp_dir = tempfile::tempdir().unwrap();
    let history_path = temp_dir.path().join("History");
    let mut fixture = ChromiumHistoryFixture::new().add_url(ChromiumUrlRow {
        id: 1,
        url: "https://example.com/batches".into(),
        title: Some("Batch proof".into()),
        visit_count: rows,
        typed_count: 0,
        last_visit_unix_ms: 1_800_000_000_000 + rows,
        hidden: false,
    });
    for id in 1..=rows {
        fixture = fixture.add_visit(ChromiumVisitRow {
            id,
            url_id: 1,
            visit_time_unix_ms: 1_800_000_000_000 + id,
            from_visit: None,
            transition: Some(1),
            visit_duration_micros: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        });
    }
    fixture.write(&history_path).unwrap();
    let profile = crate::models::BrowserProfile {
        profile_id: "chrome:batch-test".into(),
        profile_name: "batch-test".into(),
        browser_family: "chromium".into(),
        browser_name: "Chrome".into(),
        profile_path: temp_dir.path().display().to_string(),
        history_path: Some(history_path.display().to_string()),
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
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    };
    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: Vec::new(),
    }
}

/// Safety copies must capture a committed backup prefix, including its evidence
/// and cursor, while allowing the remaining batches to continue afterwards.
#[test]
fn safety_snapshot_during_backup_restores_a_consistent_committed_prefix() {
    for encrypted in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(root.path());
        crate::config::ensure_paths(&paths).unwrap();
        let config = crate::AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: if encrypted {
                crate::ArchiveMode::Encrypted
            } else {
                crate::ArchiveMode::Plaintext
            },
            ..Default::default()
        };
        crate::save_config(&paths, &config).unwrap();
        let vault = if encrypted {
            Vault::open_password(paths, config, "snapshot password", None).unwrap().0
        } else {
            Vault::open(paths, config, None).unwrap()
        };
        let source = snapshot(20_003);
        let (sent, ready) = std::sync::mpsc::channel();
        let (resume, wait) = std::sync::mpsc::channel();
        let writer = vault.clone();
        let backup = std::thread::spawn(move || {
            let mut paused = false;
            super::super::backup::run_backup_profiles(
                &writer,
                false,
                &CancelToken::new(),
                vec![source.profile],
                |p| {
                    if !paused && p.imported_records == Some(10_000) {
                        paused = true;
                        sent.send(()).unwrap();
                        wait.recv().unwrap();
                    }
                },
            )
            .unwrap();
        });
        ready.recv().unwrap();
        let saved = vault.paths().raw_snapshots_dir.join("maintenance/backup-prefix");
        crate::maintenance::snapshot(&vault, &saved).unwrap();
        resume.send(()).unwrap();
        backup.join().unwrap();
        let restored = crate::maintenance::restore_snapshot(
            &vault,
            &saved,
            encrypted.then_some("snapshot password"),
            Default::default(),
        )
        .unwrap();
        let counts=restored.vault.store().read(Db::Archive,|s|s.query_one("SELECT (SELECT count(*) FROM visits),(SELECT value FROM archive_totals WHERE name='visits'),(SELECT last_visit_id FROM profile_watermarks LIMIT 1)",(),|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))).unwrap();
        assert_eq!(counts, (10_000, 10_000, 10_000));
    }
}

fn varied_snapshot(rows: i64) -> ProfileSnapshot {
    varied_snapshot_with_urls(rows, rows.min(503))
}

fn varied_snapshot_with_urls(rows: i64, urls: i64) -> ProfileSnapshot {
    let mut source = snapshot(0);
    let mut fixture = ChromiumHistoryFixture::new();
    for id in 1..=urls {
        let url = format!("https://example.com/page/{id}");
        fixture = fixture
            .add_url(ChromiumUrlRow {
                id,
                url: url.clone(),
                title: Some(format!("Page {id}")),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: 1_800_000_000_000 + id,
                hidden: false,
            })
            .add_search_term(ChromiumKeywordSearchTermRow {
                keyword_id: 1,
                url_id: id,
                term: format!("term {id}"),
                normalized_term: format!("term {id}"),
            })
            .add_download(ChromiumDownloadRow {
                id,
                guid: Some(format!("download-{id}")),
                current_path: None,
                target_path: Some(format!("/downloads/{id}")),
                start_time_unix_ms: Some(1_800_000_000_000 + id),
                received_bytes: Some(12),
                total_bytes: Some(12),
                state: Some(1),
                mime_type: Some("text/plain".into()),
                original_mime_type: None,
            })
            .add_favicon(ChromiumFaviconRow {
                id,
                icon_url: format!("https://example.com/icon/{id}"),
                icon_type: Some(1),
                width: 16,
                height: 16,
                last_updated_unix_ms: 1_800_000_000_000 + id,
                image_data: Some(vec![id as u8; 16]),
            })
            .add_icon_mapping(ChromiumIconMappingRow { page_url: url, icon_id: id });
    }
    for id in 1..=rows {
        fixture = fixture.add_visit(ChromiumVisitRow {
            id,
            url_id: (id - 1) % urls + 1,
            visit_time_unix_ms: 1_800_000_000_000 + id,
            from_visit: None,
            transition: Some(1),
            visit_duration_micros: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        });
    }
    fixture.write(&source.history_path).unwrap();
    let icons = source.temp_dir.path().join("Favicons");
    fixture.write_favicons(&icons).unwrap();
    source.profile.favicons_path = Some(icons.display().to_string());
    source.favicons_path = Some(icons);
    source
}

fn assert_all_source_facts(vault: &Vault, rows: i64) {
    vault
        .store()
        .read(Db::Archive, move |s| {
            for table in
                ["urls", "visits", "search_terms", "downloads", "favicons", "search_documents"]
            {
                let actual = s.query_one(&format!("SELECT COUNT(*) FROM {table}"), (), |r| {
                    Ok(r.get::<i64>(0)?)
                })?;
                assert_eq!(actual, if table == "visits" { rows } else { rows.min(503) }, "{table}");
            }
            Ok(())
        })
        .unwrap();
    crate::test_support::assert_search_handoff(vault);
}

fn count(vault: &Vault) -> i64 {
    vault
        .store()
        .read(Db::Archive, |s| s.query_one("SELECT COUNT(*) FROM visits", (), |r| Ok(r.get(0)?)))
        .unwrap()
}

#[test]
fn backup_crash_child() {
    let Some(root) = std::env::var_os("PATHKEEP_BATCH_CRASH_ROOT") else { return };
    let profile: crate::BrowserProfile =
        serde_json::from_str(&std::env::var("PATHKEEP_BATCH_CRASH_PROFILE").unwrap()).unwrap();
    let key = std::env::var_os("PATHKEEP_BATCH_CRASH_ENCRYPTED").map(|_| [91; 32]);
    let config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    let vault = Vault::open(crate::project_paths_with_root(Path::new(&root)), config, key).unwrap();
    super::super::backup::run_backup_profiles(
        &vault,
        false,
        &CancelToken::new(),
        vec![profile],
        |event| {
            if event.imported_records.is_some_and(|count| count >= 10_000) {
                std::process::exit(73);
            }
        },
    )
    .unwrap();
    panic!("the child must terminate at a committed batch boundary");
}

#[test]
fn process_exit_preserves_committed_batches_and_resume_watermark() {
    for key in [None, Some([91; 32])] {
        let root = tempdir().unwrap();
        let source = varied_snapshot(10_001);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "archive::ingest::concurrency_tests::backup_crash_child",
                "--nocapture",
            ])
            .env("PATHKEEP_BATCH_CRASH_ROOT", root.path())
            .env("PATHKEEP_BATCH_CRASH_PROFILE", serde_json::to_string(&source.profile).unwrap());
        if key.is_some() {
            child.env("PATHKEEP_BATCH_CRASH_ENCRYPTED", "1");
        }
        let output = child.output().unwrap();
        assert_eq!(output.status.code(), Some(73), "{}", String::from_utf8_lossy(&output.stderr));
        let vault = Vault::open(
            crate::project_paths_with_root(root.path()),
            AppConfig { initialized: true, git_enabled: false, ..Default::default() },
            key,
        )
        .unwrap();
        assert_eq!(count(&vault), 10_000);
        let state = vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT status FROM runs WHERE run_type='backup'", (), |r| {
                    Ok(r.get::<String>(0)?)
                })
            })
            .unwrap();
        assert_eq!(state, "interrupted");
        super::super::backup::run_backup_profiles(
            &vault,
            false,
            &CancelToken::new(),
            vec![source.profile],
            |_| {},
        )
        .unwrap();
        assert_all_source_facts(&vault, 10_001);
    }
}

#[test]
fn resume_does_not_reupsert_committed_url_batches_or_skip_terms() {
    let vault = Vault::open_temp().unwrap();
    let source = varied_snapshot_with_urls(20_003, 20_003);
    let cancel = CancelToken::new();
    assert!(
        super::super::backup::run_backup_profiles(
            &vault,
            false,
            &cancel,
            vec![source.profile.clone()],
            |event| {
                if event.imported_records.is_some_and(|n| n >= 10_000) {
                    cancel.cancel();
                }
            }
        )
        .is_err()
    );
    let cursors = vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one(
                "SELECT last_url_last_visit_time,last_search_url_time FROM profile_watermarks",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
            )
        })
        .unwrap();
    assert!(cursors.0 > 0);
    assert_eq!(cursors.1, 0, "terms have not been read yet");
    super::super::backup::run_backup_profiles(
        &vault,
        false,
        &CancelToken::new(),
        vec![source.profile],
        |_| {},
    )
    .unwrap();
    vault
        .store()
        .read(Db::Archive, |s| {
            let stats: serde_json::Value = serde_json::from_str(&s.query_one(
                "SELECT stats_json FROM runs ORDER BY id DESC LIMIT 1",
                (),
                |r| Ok(r.get::<String>(0)?),
            )?)
            .unwrap();
            assert_eq!(
                stats["processedUrls"], 0,
                "resume must not read/upsert any committed URL batch"
            );
            // The visits are read from the first again: the openers the stopped read was
            // waiting for were only in memory. The 10,000 it saved are matched, not added.
            assert_eq!(stats["processedRecords"], 20_003);
            assert_eq!(stats["newVisits"], 10_003);
            for table in ["urls", "visits", "search_terms", "downloads", "favicons"] {
                assert_eq!(
                    s.query_one(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(
                        r.get::<i64>(0)?
                    ))?,
                    20_003,
                    "{table}"
                );
            }
            Ok(())
        })
        .unwrap();
}
