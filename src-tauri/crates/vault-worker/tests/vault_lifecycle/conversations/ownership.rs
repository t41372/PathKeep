//! Pins attachment failures, recovery isolation and shutdown ordering on real stores.
//! Corpus preservation through file replacement belongs to the lifecycle scenarios.

use super::*;

#[test]
fn attach_requires_both_roles() {
    use vault_store::{Config, Db, Store};
    for role in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::new(temp.path());
        config.databases = vec![role, Db::Scratch];
        config.migrations.register(role, agent_history::schema_steps(role));
        let store = Arc::new(Store::open(config).unwrap());
        let resources = agent_history::AgentHistoryResources {
            search_path: temp.path().join("index"),
            search_key: None,
        };
        assert!(matches!(
            Archive::from_store(store.clone(), resources),
            Err(agent_history::Error::InvalidArchive(_))
        ));
        assert!(store.is_open());
        assert!(!temp.path().join("index").exists());
    }
}

#[test]
fn failed_attach_leaves_browsing_available_and_can_retry() {
    let fixture = Fixture::new(false);
    fixture.session.close_vault().unwrap();
    let file =
        fixture.root.join(vault_layout::database_path(vault_store::Db::AgentHistoryEvidence));
    let saved = fixture.sandbox.path().join("healthy");
    fs::copy(&file, &saved).unwrap();
    rusqlite::Connection::open(&file)
        .unwrap()
        .execute("ALTER TABLE ah_backup_runs RENAME TO saved_runs", ())
        .unwrap();
    let vault = fixture.reopen(false);
    assert!(vault.agent_history().is_err());
    fixture.create_browser();
    assert_eq!(fixture.backup().run.unwrap().new_visits, 1);
    fs::rename(saved, file).unwrap();
    vault.retry_conversations().unwrap();
    vault.conversation_availability().unwrap();
    assert!(vault.agent_history().is_ok());
}

#[test]
fn close_and_maintenance_drain_capture_and_sync_before_store_shutdown() {
    use std::{
        sync::{Mutex, mpsc},
        time::{Duration, Instant},
    };
    for (capture, maintenance) in [(true, false), (false, false), (true, true), (false, true)] {
        let fixture = Fixture::new(false);
        let vault = fixture.vault();
        let archive = vault.agent_history().unwrap();
        let logs = fixtures::Fixture::four_harnesses(fixture.sandbox.path());
        let roots =
            vec![SourceRoot { harness: "pi".parse().unwrap(), root: logs.roots[2].1.clone() }];
        let (entered, ready) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        let wait = Mutex::new(Some(wait));
        let block_once = move || {
            if let Some(wait) = wait.lock().unwrap().take() {
                entered.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(15)).unwrap();
            }
        };
        std::thread::scope(|scope| {
            let handle = archive.clone();
            let job = scope.spawn(move || {
                if capture {
                    handle
                        .backup(
                            &roots,
                            &BackupOptions {
                                progress: Some(Arc::new(move |_| block_once())),
                                ..Default::default()
                            },
                        )
                        .map(|_| ())
                } else {
                    handle
                        .synchronize_search_with_cancel(&|| {
                            block_once();
                            false
                        })
                        .map(|_| ())
                }
            });
            ready.recv_timeout(Duration::from_secs(15)).unwrap();
            let (finished, done) = mpsc::channel();
            let owner = &vault;
            let session = &fixture.session;
            let closing = scope.spawn(move || {
                if maintenance {
                    let _lease = session.begin_maintenance().unwrap();
                } else {
                    owner.close().unwrap();
                }
                finished.send(()).unwrap();
            });
            // Observe actual revocation before checking order. The callback still owns the job.
            let deadline = Instant::now() + Duration::from_secs(10);
            while !matches!(
                archive.search(&query()),
                Err(agent_history::Error::Store(vault_store::Error::Closed)
                    | agent_history::Error::Cancelled)
            ) {
                assert!(Instant::now() < deadline, "close did not revoke admission");
                std::thread::yield_now();
            }
            let blocked = matches!(
                done.recv_timeout(Duration::from_millis(250)),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            let still_open = vault.store().is_open();
            release.send(()).unwrap();
            let result = job.join().unwrap();
            closing.join().unwrap();
            assert!(blocked, "close returned during the callback");
            assert!(still_open, "store closed inside a running conversation callback");
            assert!(matches!(result, Err(agent_history::Error::Cancelled)), "{result:?}");
        });
        if maintenance {
            assert!(Arc::ptr_eq(&archive, &vault.agent_history().unwrap()));
            archive.synchronize_search().unwrap();
        }
    }
}

#[test]
fn supplied_resources_match_store_mode_and_select_index_location() {
    use agent_history::AgentHistoryResources;
    for encrypted in [true, false] {
        let fixture = Fixture::new(encrypted);
        let vault = fixture.vault();
        let key = if encrypted {
            Vault::unlock_data_key(vault.paths(), PASSWORD).unwrap()
        } else {
            vault_crypto::DataKey::from_bytes(zeroize::Zeroizing::new([13; 32]))
        };
        let search_path = fixture.sandbox.path().join("authorized-index");
        let resources = |sealed: bool| AgentHistoryResources {
            search_path: search_path.clone(),
            search_key: sealed
                .then(|| key.derive(vault_crypto::KeyPurpose::AgentHistorySearchIndex)),
        };
        assert!(
            Archive::from_store(vault.store().clone(), resources(!encrypted)).is_err(),
            "inconsistent index mode"
        );
        let attached = Archive::from_store(vault.store().clone(), resources(encrypted)).unwrap();
        attached.synchronize_search().unwrap();
        assert!(search_path.join("meta.json").is_file());
        drop(attached);
        assert!(vault.store().is_open());
        assert!(!fixture.root.join("derived/agent-history-search").exists());
    }
}
