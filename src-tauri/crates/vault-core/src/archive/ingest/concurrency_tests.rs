//! Responsibilities: prove committed batch recovery and interactive write scheduling.
//! Not responsible for: scheduler IPC or browser-owned SQLite locking.
//! Failure modes: batch-size-dependent results, cancelled/crashed run loses its
//! committed prefix, resume duplicates visits, stars wait until the backup ends,
//! contending writes read an uncommitted watermark; early URL cursors skip search terms;
//! resumed visits are rescanned; downloads or favicons disappear after a crash.

use super::*;
use crate::{SetStarRequest, StarEntityKind, Vault};
use browser_history_fixtures::{
    ChromiumDownloadRow, ChromiumFaviconRow, ChromiumHistoryFixture, ChromiumIconMappingRow,
    ChromiumKeywordSearchTermRow, ChromiumUrlRow, ChromiumVisitRow,
};
use vault_store::{Db, Priority};

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
    vault.store().read(Db::Archive, move |s| {
        for table in ["urls", "visits", "search_terms", "downloads", "favicons", "search_documents"] {
            let actual = s.query_one(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(r.get::<i64>(0)?))?;
            assert_eq!(actual, if table=="visits" {rows} else {rows.min(503)}, "{table}");
        }
        assert_eq!(s.query_one("SELECT COUNT(*) FROM pages p WHERE NOT EXISTS(SELECT 1 FROM search_outbox o WHERE o.page_id=p.id)",(),|r| Ok(r.get::<i64>(0)?))?,0);
        Ok(())
    }).unwrap();
}

fn count(vault: &Vault) -> i64 {
    vault
        .store()
        .read(Db::Archive, |s| s.query_one("SELECT COUNT(*) FROM visits", (), |r| Ok(r.get(0)?)))
        .unwrap()
}

#[test]
fn cancelled_backup_resumes_its_committed_prefix_without_duplicates() {
    for key in [None, Some([51; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = varied_snapshot(20_003);
        let cancel = CancelToken::new();
        let result = super::super::backup::run_backup_profiles(
            &vault,
            false,
            &cancel,
            vec![source.profile.clone()],
            |event| {
                if event.imported_records.is_some_and(|n| n >= 10_000) {
                    cancel.cancel();
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(count(&vault), 10_000);
        let (state,cursor)=vault.store().read(Db::Archive,|s| {
            s.query_one("SELECT status,(SELECT last_visit_id FROM profile_watermarks WHERE profile_id='chrome:batch-test') FROM runs WHERE id>0 ORDER BY id DESC LIMIT 1",(),|r| Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))
        }).unwrap();
        assert_eq!((state, cursor), ("interrupted".into(), 10_000));
        super::super::backup::run_backup_profiles(
            &vault,
            false,
            &CancelToken::new(),
            vec![source.profile],
            |_| {},
        )
        .unwrap();
        assert_eq!(count(&vault), 20_003);
        assert_all_source_facts(&vault, 20_003);
        let processed = vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one(
                    "SELECT stats_json FROM runs WHERE run_type='backup' ORDER BY id DESC LIMIT 1",
                    (),
                    |r| Ok(r.get::<String>(0)?),
                )
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&processed).unwrap()["processedRecords"],
            10_003,
            "resume must read only visits after the committed watermark"
        );
    }
}

#[test]
fn star_commits_between_backup_batches() {
    for key in [None, Some([52; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = snapshot(20_003);
        let (at_batch, ready) = std::sync::mpsc::channel();
        let (continue_backup, wait_backup) = std::sync::mpsc::channel();
        let backup_vault = vault.clone();
        let backup = std::thread::spawn(move || {
            let mut paused = false;
            super::super::backup::run_backup_profiles(
                &backup_vault,
                false,
                &CancelToken::new(),
                vec![source.profile.clone()],
                |event| {
                    if !paused && event.imported_records == Some(10_000) {
                        paused = true;
                        at_batch.send(()).unwrap();
                        wait_backup.recv().unwrap();
                    }
                },
            )
            .unwrap();
        });
        ready.recv().unwrap();
        assert!(
            super::super::backup::run_backup_profiles(
                &vault,
                false,
                &CancelToken::new(),
                vec![],
                |_| {}
            )
            .unwrap_err()
            .to_string()
            .contains("backup-already-running")
        );
        let (running, entered) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let store = vault.store().clone();
        let holder = std::thread::spawn(move || {
            store
                .write(Db::Archive, Priority::Batch, move |_| {
                    running.send(()).unwrap();
                    blocked.recv().unwrap();
                    Ok(())
                })
                .unwrap()
        });
        entered.recv().unwrap();
        continue_backup.send(()).unwrap();
        wait_pending(&vault, false, 1);
        let star_vault = vault.clone();
        let star = std::thread::spawn(move || {
            crate::set_star(
                &star_vault,
                SetStarRequest {
                    entity_kind: StarEntityKind::Url,
                    entity_key: "https://example.com/batches".into(),
                    source_profile: None,
                },
            )
            .unwrap()
        });
        wait_pending(&vault, true, 1);
        let store = vault.store().clone();
        let observation = std::thread::spawn(move || {
            store
                .write(Db::Archive, Priority::Interactive, |s| {
                    let starred =
                        s.query_one("SELECT COUNT(*) FROM star", (), |r| Ok(r.get::<i64>(0)?))?;
                    let visits =
                        s.query_one("SELECT COUNT(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?;
                    Ok((starred, visits))
                })
                .unwrap()
        });
        wait_pending(&vault, true, 2);
        release.send(()).unwrap();
        assert_eq!(
            observation.join().unwrap(),
            (1, 10_000),
            "interactive requests must overtake the queued backup batch; FIFO would see 20,000 visits"
        );
        star.join().unwrap();
        holder.join().unwrap();
        backup.join().unwrap();
        assert_eq!(count(&vault), 20_003);
    }
}

fn wait_pending(vault: &Vault, interactive: bool, expected: usize) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let metrics = vault.store().metrics(Db::Archive);
        let pending = if interactive { metrics.interactive_pending } else { metrics.batch_pending };
        if pending >= expected {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "producer did not queue its request");
        std::thread::yield_now();
    }
}

#[test]
fn same_profile_writer_waits_for_committed_watermark() {
    let vault = Vault::open_temp().unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let store = vault.store().clone();
    let first = std::thread::spawn(move || {
        store
            .write(Db::Archive, Priority::Batch, move |s| {
                save_watermark(
                    s,
                    "test",
                    &Watermark { last_visit_id: 41, ..Watermark::default() },
                )?;
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
            .unwrap()
    });
    entered_rx.recv().unwrap();
    let store = vault.store().clone();
    let second = std::thread::spawn(move || {
        store
            .write(Db::Archive, Priority::Batch, |s| Ok(load_watermark(s, "test")?.last_visit_id))
            .unwrap()
    });
    wait_pending(&vault, false, 1);
    assert!(!second.is_finished(), "writer must wait for the first transaction to commit");
    release_tx.send(()).unwrap();
    first.join().unwrap();
    assert_eq!(second.join().unwrap(), 41);
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
fn batched_backup_equals_one_transaction_of_parsed_facts() {
    for key in [None, Some([92; 32])] {
        let batched = Vault::open_temp_with_key(key).unwrap();
        let single = Vault::open_temp_with_key(key).unwrap();
        let source = varied_snapshot(20_003);
        let parsed = chromium::parse_history(
            &HistoryDatabaseSet {
                history_path: source.history_path.clone(),
                favicons_path: source.favicons_path.clone(),
            },
            ChromiumReadCursor::default(),
        )
        .unwrap();
        let profile = source.profile.clone();
        let evidence = SourceEvidencePayload {
            typed_evidence: parsed.typed_evidence.clone(),
            native_entities: parsed.native_entities.clone(),
        };
        single
            .store()
            .write(Db::Archive, Priority::Batch, move |s| {
                let profile_id = writes::upsert_source_profile(s, &profile)?;
                let mut urls = std::collections::HashMap::new();
                let mut bounds = std::collections::HashMap::new();
                let mut pages = std::collections::BTreeSet::new();
                let mut cursor = Watermark::default();
                for url in &parsed.urls {
                    let (id, page, _) = writes::upsert_url(
                        s,
                        0,
                        profile_id,
                        url,
                        &sha256_hex(&serde_json::to_vec(url).unwrap()),
                    )?;
                    urls.insert(url.source_url_id, id);
                    pages.insert(page);
                    cursor.last_url_last_visit_time = cursor
                        .last_url_last_visit_time
                        .max(super::selection::url_marker("chromium", url));
                }
                for visit in &parsed.visits {
                    let id = urls[&visit.source_url_id];
                    writes::insert_visit(
                        s,
                        0,
                        profile_id,
                        id,
                        visit,
                        &sha256_hex(&serde_json::to_vec(visit).unwrap()),
                    )?;
                    writes::track_url_visit_bounds(&mut bounds, id, visit);
                    cursor.last_visit_id = cursor.last_visit_id.max(visit.source_visit_id);
                }
                for (id, bounds) in bounds {
                    writes::sync_url_bounds(s, id, &bounds)?;
                }
                for download in &parsed.downloads {
                    writes::insert_download(
                        s,
                        0,
                        profile_id,
                        download,
                        &sha256_hex(&serde_json::to_vec(download).unwrap()),
                    )?;
                    cursor.last_download_id =
                        cursor.last_download_id.max(download.source_download_id);
                }
                for term in &parsed.search_terms {
                    cursor.last_search_url_time =
                        cursor.last_search_url_time.max(term.url_last_visit_time);
                    writes::insert_search_term(
                        s,
                        0,
                        profile_id,
                        &profile.profile_id,
                        urls[&term.url_id],
                        term,
                    )?;
                }
                for icon in &parsed.favicons {
                    writes::insert_favicon(
                        s,
                        0,
                        profile_id,
                        icon,
                        &sha256_hex(&serde_json::to_vec(icon).unwrap()),
                    )?;
                    cursor.last_favicon_last_updated = cursor
                        .last_favicon_last_updated
                        .max(super::selection::favicon_marker("chromium", icon));
                }
                for page in pages {
                    crate::archive::search_documents::refresh(s, page)?;
                }
                save_watermark(s, &profile.profile_id, &cursor)?;
                Ok(())
            })
            .unwrap();
        single
            .write(Db::SourceEvidence, Priority::Batch, move |s| {
                let id = upsert_source_batch(
                    s,
                    &SourceBatchInput {
                        source_profile_id: 1,
                        run_id: Some(0),
                        source_kind: "local_db".into(),
                        browser_version: None,
                        schema_version_text: None,
                        schema_version_int: None,
                        schema_fingerprint: "fixture".into(),
                        capability_snapshot: Default::default(),
                        coverage_stats_json: "{}".into(),
                        artifact_refs_json: None,
                        notes_json: None,
                    },
                )?;
                crate::archive::source_evidence::persist_source_evidence(s, id, 1, &evidence)
            })
            .unwrap();
        super::super::backup::run_backup_profiles(
            &batched,
            false,
            &CancelToken::new(),
            vec![source.profile],
            |_| {},
        )
        .unwrap();
        for (db, sql) in [
            (
                Db::Archive,
                "SELECT url,title,visit_count,typed_count,first_visit_ms,last_visit_ms,source_url_id FROM urls ORDER BY source_url_id",
            ),
            (
                Db::Archive,
                "SELECT source_visit_id,url_id,visit_time_ms,transition_type,event_fingerprint FROM visits ORDER BY source_visit_id",
            ),
            (Db::Archive, "SELECT url_id,term,normalized_term FROM search_terms ORDER BY url_id"),
            (
                Db::Archive,
                "SELECT source_download_id,target_path,start_time_ms FROM downloads ORDER BY source_download_id",
            ),
            (Db::Archive, "SELECT page_url,icon_url,payload_hash FROM favicons ORDER BY page_url"),
            (
                Db::Archive,
                "SELECT last_visit_id,last_url_last_visit_time,last_download_id,last_favicon_last_updated,last_search_url_time,url_scan_json FROM profile_watermarks",
            ),
            (
                Db::Archive,
                "SELECT page_id,url,title,search_terms,newest_visit_ms FROM search_documents ORDER BY page_id",
            ),
            (
                Db::Archive,
                "SELECT page_id,source_profile_id,visible_visit_count,newest_visit_ms FROM search_document_profiles ORDER BY page_id",
            ),
            (
                Db::SourceEvidence,
                "SELECT source_url_id,evidence_key,evidence_value FROM visit_search_evidence ORDER BY source_url_id,evidence_key,evidence_value",
            ),
            (
                Db::SourceEvidence,
                "SELECT entity_kind,native_primary_key,payload_json FROM native_entities ORDER BY entity_kind,native_primary_key,payload_json",
            ),
        ] {
            assert_eq!(rows(&batched, db, sql), rows(&single, db, sql), "{sql}");
        }
        assert_all_source_facts(&batched, 20_003);
    }
}

fn rows(vault: &Vault, db: Db, sql: &'static str) -> Vec<Vec<vault_store::Value>> {
    vault
        .store()
        .read(db, move |s| {
            s.query(sql, (), |r| (0..r.column_count()).map(|i| Ok(r.get_value(i)?)).collect())
        })
        .unwrap()
}

#[test]
fn failed_backup_keeps_completed_profile_counts_and_warnings() {
    for key in [None, Some([76; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = varied_snapshot(1);
        let bad = source.temp_dir.path().join("broken");
        std::fs::create_dir(&bad).unwrap();
        let mut failed = source.profile.clone();
        failed.profile_id = "chrome:broken".into();
        failed.profile_path = bad.display().to_string();
        std::fs::create_dir(bad.join("History")).unwrap();
        failed.history_path = Some(bad.join("History").display().to_string());
        let error = super::super::backup::run_backup_profiles(
            &vault,
            false,
            &CancelToken::new(),
            vec![source.profile.clone(), failed],
            |_| {},
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("staging profile chrome:broken"));
        vault
            .store()
            .read(Db::Archive, |s| {
                let (status, stats, warnings) = s.query_one(
                    "SELECT status,stats_json,warnings_json FROM runs WHERE run_type='backup'",
                    (),
                    |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
                )?;
                let stats: serde_json::Value = serde_json::from_str(&stats).unwrap();
                assert_eq!(status, "failed");
                assert_eq!(stats["profilesProcessed"], 1);
                assert_eq!(stats["newVisits"], 1);
                assert_eq!(stats["newDownloads"], 1);
                assert_eq!(stats["profiles"][0]["newVisits"], 1);
                assert!(warnings.contains("profile-search-terms-captured"));
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn backup_overview_agrees_with_the_persisted_run_metadata() {
    let vault = Vault::open_temp().unwrap();
    let source = varied_snapshot(1);
    let mut config = vault.config().clone();
    config.selected_profile_ids = vec![source.profile.profile_id.clone(), "chrome:missing".into()];
    let vault = vault.with_config(config);
    let report = super::super::backup::run_backup_profiles(
        &vault,
        false,
        &CancelToken::new(),
        vec![source.profile.clone()],
        |_| {},
    )
    .unwrap();
    for run in [
        report.run.unwrap(),
        super::super::backup::run_backup_profiles(
            &vault,
            true,
            &CancelToken::new(),
            vec![source.profile],
            |_| {},
        )
        .unwrap()
        .run
        .unwrap(),
    ] {
        let id = run.id;
        let row=vault.store().read(Db::Archive,move |s|s.query_one("SELECT started_at,finished_at,status,trigger,profile_scope_json FROM runs WHERE id=?1",[id],|r|Ok((r.get::<String>(0)?,r.get::<Option<String>>(1)?,r.get::<String>(2)?,r.get::<String>(3)?,r.get::<String>(4)?)))).unwrap();
        assert_eq!(run.started_at, row.0);
        assert_eq!(run.finished_at, row.1);
        assert_eq!(run.status, row.2);
        assert_eq!(run.trigger, row.3);
        assert_eq!(run.profile_scope, serde_json::from_str::<Vec<String>>(&row.4).unwrap());
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
            assert_eq!(stats["processedRecords"], 10_003);
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
