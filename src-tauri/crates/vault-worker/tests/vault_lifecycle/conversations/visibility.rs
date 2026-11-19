//! Exercise device visibility across the real owner and archive installations.
//! A readable conversation must never stand in for a grant to read a tool.
use super::*;
use serde_json::{Value, from_value, json, to_value};
use vault_worker::{
    VaultSession,
    data_kinds::{self as kinds, backups},
};

fn seed(vault: &Vault, base: &std::path::Path) {
    fixtures::pi(base, "/synthetic", "saved", "visibilityneedle");
    vault
        .agent_history()
        .unwrap()
        .backup(
            &[SourceRoot { harness: agent_history::Harness::Pi, root: base.to_owned() }],
            &BackupOptions { read_git: false, ..Default::default() },
        )
        .unwrap();
    fs::remove_dir_all(base).unwrap();
}

fn choose(session: &VaultSession, state: &str) -> anyhow::Result<Value> {
    let request = from_value(
        json!({"kind":"agent-history","state":state,"expectedPolicyToken":kinds::settings()?.policy_token}),
    )?;
    Ok(to_value(kinds::set_state(session, request, |_| {})?)?)
}
fn assert_view(session: &VaultSession, hidden: bool) {
    let snapshot = to_value(kinds::list(session).unwrap()).unwrap();
    let row = &snapshot["kinds"][1];
    assert_eq!(row["state"], if hidden { "off" } else { "paused" });
    assert_eq!(row["hasBackupSource"], false);
    let policy = kinds::settings().unwrap().policy;
    assert!(policy.enabled_tools.is_empty() && policy.selection_ref.is_none());
    let before = || {
        session
            .vault()
            .unwrap()
            .store()
            .read(vault_store::Db::AgentHistoryEvidence, |s| {
                s.query_one("SELECT COUNT(*) FROM ah_backup_runs", (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap()
    };
    let runs = before();
    let read = backups::overview(session, from_value(json!({"purpose":"space"})).unwrap());
    if hidden {
        assert_eq!(read.unwrap_err().to_string(), "kind-off");
    } else {
        assert_eq!(read.unwrap()["roots"], "1");
        assert!(
            !session
                .vault()
                .unwrap()
                .agent_history()
                .unwrap()
                .search(&query())
                .unwrap()
                .items
                .is_empty()
        );
    }
    assert!(backups::start(session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{})).is_err());
    assert_eq!(choose(session, "on").unwrap_err().to_string(), "enable-preview-required");
    // Exercise the due-only path used by the existing wake. Step 3 owns
    // multikind all-now; the current dispatcher refuses that scope.
    vault_worker::run_backup_now(&session.vault().unwrap(), None, true).unwrap();
    assert!(
        from_value::<backups::Start>(
            json!({"scope":{"type":"all"},"requestId":uuid::Uuid::new_v4().to_string()})
        )
        .is_err()
    );
    assert_eq!(before(), runs);
}

#[test]
fn saved_conversations_on_first_open_and_restore_follow_device_choice() {
    for hidden in [false, true] {
        let f = Fixture::new(false);
        f.create_browser();
        if hidden {
            choose(&f.session, "off").unwrap();
        }
        seed(&f.vault(), &f.sandbox.path().join("native-pi"));
        let snapshot = f.root.join("raw-snapshots/maintenance/with-conversations");
        vault_core::maintenance::snapshot(&f.vault(), &snapshot).unwrap();
        f.session.close_vault().unwrap();
        fs::remove_dir_all(f.root.join("derived/agent-history-search")).unwrap();
        f.reopen(false);
        assert_view(&f.session, hidden);
        vault_worker::run_full_archive_restore(
            &f.session,
            None,
            &vault_core::SnapshotRestoreRequest { snapshot_path: snapshot.display().to_string() },
        )
        .unwrap();
        assert_view(&f.session, hidden);
    }
}

#[test]
fn move_preview_and_install_follow_destination_visibility_without_granting_roots() {
    let f = Fixture::new(false);
    f.create_browser();
    seed(&f.vault(), &f.sandbox.path().join("native-pi"));
    let bundle = f.root.join("exports/visible.pathkeep");
    vault_worker::export_app_data(&f.vault(), bundle.clone(), EXPORT_PASSWORD).unwrap();
    for populated in [false, true] {
        for hidden in [false, true] {
            let dest = f.sandbox.path().join(format!("move-{populated}-{hidden}"));
            fs::create_dir(&dest).unwrap();
            unsafe {
                std::env::set_var("CHB_PROJECT_ROOT", &dest);
            }
            let session = VaultSession::default();
            if populated || hidden {
                vault_worker::initialize_archive_database_with_progress(
                    &session,
                    &vault_core::AppConfig {
                        initialized: true,
                        archive_mode: ArchiveMode::Plaintext,
                        ..Default::default()
                    },
                    None,
                    |_| {},
                )
                .unwrap();
                if populated {
                    seed(&session.vault().unwrap(), &f.sandbox.path().join("other-pi"));
                }
                if hidden {
                    choose(&session, "off").unwrap();
                }
            }
            let paths = vault_core::project_paths_with_root(&dest);
            let preview =
                to_value(vault_worker::preview_import(&paths, bundle.clone()).unwrap()).unwrap();
            assert_eq!(
                preview["agentConversations"]["stateIfPresent"],
                if hidden { "off" } else { "paused" }
            );
            assert_eq!(preview["agentConversations"]["carried"], true);
            let current = session.current_vault();
            let lease = session.begin_maintenance().unwrap();
            let (_, installed) = vault_worker::apply_import(
                &paths,
                current.as_ref(),
                session.owner(),
                bundle.clone(),
                vault_core::ApplyImportOptions {
                    confirm_overwrite: true,
                    source_archive_key: Some(EXPORT_PASSWORD.into()),
                },
            )
            .unwrap();
            session
                .install_archive(
                    installed,
                    Some(EXPORT_PASSWORD.into()),
                    vault_worker::KeychainAfterInstall::AsSaved,
                )
                .unwrap();
            drop(lease);
            assert_view(&session, hidden);
            if hidden {
                choose(&session, "paused").unwrap();
                assert_view(&session, false);
            }
            session.close_vault().unwrap();
        }
    }
}

#[test]
fn retry_agent_store_during_a_browsing_backup_keeps_its_producer() {
    use std::{sync::mpsc, time::Duration};
    let f = Fixture::new(false);
    f.create_browser();
    f.session.close_vault().unwrap();
    let path = f.root.join("archive/agent-history.db");
    let held = path.with_extension("saved");
    fs::rename(&path, &held).unwrap();
    let vault = f.reopen(false);
    let (arrived, at_backup) = mpsc::channel();
    let (release, go) = mpsc::channel();
    let running = vault.clone();
    let worker = std::thread::spawn(move || {
        let mut first = true;
        vault_worker::run_backup_now_with_progress(&running, None, false, |_| {
            if first {
                first = false;
                arrived.send(()).unwrap();
                go.recv_timeout(Duration::from_secs(10)).unwrap();
            }
        })
    });
    at_backup.recv_timeout(Duration::from_secs(10)).unwrap();
    let retry = || {
        to_value(kinds::retry_agent_store(&f.session,|_|{}).unwrap()).unwrap()["kinds"][1]["availability"].clone()
    };
    assert_eq!(retry(), "unavailable");
    fs::rename(held, path).unwrap();
    assert_eq!(retry(), "available");
    assert_eq!(retry(), "available");
    assert!(Arc::ptr_eq(vault.store(), f.vault().store()));
    release.send(()).unwrap();
    assert_eq!(worker.join().unwrap().unwrap().run.unwrap().new_visits, 1);
}

/// A previously configured destination can keep a selection reference that the
/// installed evidence no longer contains. That refuses capture, not archive reads.
#[test]
fn missing_destination_descriptor_does_not_hide_retained_records() {
    let f = Fixture::new(false);
    seed(&f.vault(), &f.sandbox.path().join("recorded"));
    let paths = vault_core::project_paths().unwrap();
    let mut config = vault_core::load_config(&paths).unwrap();
    config.capture_policy.agent_history = vault_core::KindState::Paused;
    config.capture_policy.enabled_tools.insert(vault_core::AgentTool::Pi);
    config.capture_policy.selection_ref = Some(uuid::Uuid::new_v4().to_string());
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let row = to_value(kinds::list(&f.session).unwrap()).unwrap()["kinds"][1].clone();
    assert_eq!(row["state"], "paused");
    assert_eq!(row["hasBackupSource"], false);
    let overview =
        backups::overview(&f.session, from_value(json!({"purpose":"space"})).unwrap()).unwrap();
    assert_eq!(overview["roots"], "1");
    assert!(overview["perTool"].as_array().unwrap().iter().all(|t| t["enabled"] == false));
    assert!(backups::start(&f.session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{})).is_err());
}

#[test]
fn incomplete_initial_index_recovers_but_established_damage_needs_repair() {
    let f = Fixture::new(false);
    seed(&f.vault(), &f.sandbox.path().join("native-pi"));
    let index = f.root.join("derived/agent-history-search");
    for pending in [false, true] {
        let vault = f.vault();
        if pending {
            vault.store().write(vault_store::Db::AgentHistoryReadModel,vault_store::Priority::Interactive,|s|{
                s.execute("INSERT OR REPLACE INTO ah_meta(key,value) VALUES('initial_search_pending','1')",())?;Ok(())
            }).unwrap();
        }
        f.session.close_vault().unwrap();
        if index.exists() {
            fs::remove_dir_all(&index).unwrap();
        }
        fs::create_dir(&index).unwrap();
        let reopened = f.reopen(false);
        let history = reopened.agent_history().unwrap();
        history.prepare_visible_search().unwrap();
        let found = history.search(&query());
        if pending {
            assert!(!found.unwrap().items.is_empty());
        } else {
            assert!(matches!(found, Err(agent_history::Error::IndexBehind)));
        }
    }
}

#[test]
fn intact_live_folder_opened_by_a_second_installation_has_no_capture_consent() {
    for hidden in [true, false] {
        let f = Fixture::new(false);
        f.create_browser();
        if hidden {
            seed(&f.vault(), &f.sandbox.path().join("pi"));
            choose(&f.session, "off").unwrap();
        } else {
            let path = f.sandbox.path().join("installation-a-pi");
            fixtures::pi(&path, "/synthetic", "saved", "visibilityneedle");
            let path = path.canonicalize().unwrap();
            let request =
                from_value(json!({"purpose":"enable","pickedFolder":{"tool":"pi","path":path}}))
                    .unwrap();
            let mut frames = kinds::preview::start(&f.session, request).unwrap();
            let report = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    while let Some(frame) = frames.next_frame().await {
                        if frame["type"] == "finished" {
                            return frame["payload"].clone();
                        }
                    }
                    panic!("preview ended without its inventory")
                });
            let (notify, finished) = std::sync::mpsc::channel();
            let enable = json!({
                "requestId":uuid::Uuid::new_v4().to_string(),"expectedPolicyToken":kinds::settings().unwrap().policy_token,
                "previewId":report["previewId"],"selectedRootIds":[report["rows"][0]["rootId"]],"readGit":false
            });
            backups::enable(
                &f.session,
                from_value(enable).unwrap(),
                Arc::new(move |name, _| {
                    if name == "pathkeep://kind-backup-finished" {
                        let _ = notify.send(());
                    }
                }),
            )
            .unwrap();
            finished.recv_timeout(std::time::Duration::from_secs(20)).unwrap();
            let list = to_value(kinds::list(&f.session).unwrap()).unwrap();
            assert_eq!(list["kinds"][1]["state"], "on");
            assert_eq!(list["kinds"][1]["hasBackupSource"], true);
        }
        f.session.close_vault().unwrap();
        unsafe {
            std::env::set_var(
                "PATHKEEP_PLATFORM_TEST_SANDBOX_DIR",
                f.sandbox.path().join("installation-b"),
            );
        }
        f.reopen(false);
        assert_view(&f.session, false);
    }
}

#[test]
fn paused_browsing_runs_only_when_requested_for_that_kind() {
    let f = Fixture::new(false);
    f.create_browser();
    let vault = f.vault();
    let paths = vault.paths();
    let mut config = vault_core::load_config(paths).unwrap();
    config.capture_policy.agent_history = vault_core::KindState::On;
    config.capture_policy.browsing = vault_core::KindState::Paused;
    vault_core::save_config(&vault_core::ControlGate::acquire(paths), &config).unwrap();
    let run = |kind_specific| {
        vault_worker::run_browsing_backup(&vault, None, false, kind_specific, |_| {}).unwrap()
    };
    assert_eq!(run(false).reason_code.as_deref(), Some("kind-paused"));
    assert_eq!(run(true).run.unwrap().new_visits, 1);
    config.capture_policy.browsing = vault_core::KindState::Off;
    vault_core::save_config(&vault_core::ControlGate::acquire(paths), &config).unwrap();
    assert_eq!(run(true).reason_code.as_deref(), Some("kind-off"));
}
