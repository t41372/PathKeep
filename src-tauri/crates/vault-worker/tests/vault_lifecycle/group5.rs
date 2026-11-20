//! Exercise archive protection at the owner boundary using released files and independent
//! filesystems. The desktop recovery view consumes the same services.
use super::support::*;
use std::{fs, path::Path, process::Command};
use vault_core::{Vault, maintenance::rollback};
use vault_store::{Db, Priority};

#[test]
fn released_encrypted_archive_detaches_and_rejoins_after_a_password_change() {
    let f = Fixture::new(false);
    let root = f.sandbox.path().join("released");
    fs::create_dir(&root).unwrap();
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/v0.5-archive/data.tar.gz");
    assert!(
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let paths = vault_core::project_paths_with_root(&root);
    let config = vault_core::load_config(&paths).unwrap();
    let password = "harbour almanac 2026";
    vault_core::unlock_app_session(
        &paths,
        &config,
        &vault_core::UnlockAppSessionRequest {
            passcode: Some("4826".into()),
            use_biometric: false,
        },
    )
    .unwrap();
    let (live, _) = Vault::open_password(paths.clone(), config, password, None).unwrap();
    let visits = |v: &Vault| {
        v.store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap()
    };
    let before = visits(&live);
    assert!(before >= 1952);
    let pi = f.sandbox.path().join("pi");
    super::conversations::corpus::fixtures::pi(
        &pi,
        "/pi-project",
        "handoff-session",
        "saved while upgraded",
    );
    live.agent_history()
        .unwrap()
        .backup(
            &[agent_history::SourceRoot { root: pi, harness: agent_history::Harness::Pi }],
            &Default::default(),
        )
        .unwrap();
    live.store()
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute(
                "INSERT INTO archive_meta(name,value) VALUES('after-upgrade','retained')",
                (),
            )?;
            Ok(())
        })
        .unwrap();
    let copy = vault_core::maintenance::manual_snapshot(&live, false).unwrap();
    // Pre-release builds allowed a move file directly in the data root, with any extension.
    let exported = f.sandbox.path().join("portable.pathkeep");
    vault_worker::export_app_data(&f.vault(), exported.clone(), EXPORT_PASSWORD).unwrap();
    let previous_location = root.join("my earlier transfer");
    fs::rename(exported, &previous_location).unwrap();
    let preview = serde_json::to_value(rollback::preview(&paths).unwrap()).unwrap();
    assert_eq!(preview["targetVersion"], "0.5.0");
    assert!(preview["conversationBytes"].as_u64().unwrap() > 0);
    let package = rollback::detach(&paths, Some(&live), None, None).unwrap();
    assert_eq!(
        vault_core::admission::root_layout(&root).unwrap(),
        vault_layout::ArchiveLayout::BrowsingV1
    );
    assert!(!Path::new(&copy.path).exists());
    assert!(
        package.join("copies").join(Path::new(&copy.path).strip_prefix(&root).unwrap()).is_dir()
    );
    assert!(!previous_location.exists());
    assert!(package.join("copies/my earlier transfer").is_file());
    let saved =
        agent_history::Archive::open(&package.join("agent-history"), password.as_bytes()).unwrap();
    assert_eq!(
        saved.list_sessions(&Default::default()).unwrap().items[0].native_id,
        "handoff-session"
    );
    drop(saved);
    let refused = Vault::open_password(
        paths.clone(),
        vault_core::load_config(&paths).unwrap(),
        password,
        None,
    )
    .unwrap_err();
    assert!(refused.to_string().contains("archive-handoff-choice-required"), "{refused}");
    // Rewrap only the legacy live envelope, exactly as a 0.5 password change does.
    let envelope = paths.artifact_path(vault_layout::ArtifactId::Envelope);
    let previous =
        vault_crypto::Envelope::parse(&vault_crypto::read_envelope_file(&envelope).unwrap())
            .unwrap();
    let next = previous
        .rewrap(password.as_bytes(), b"new browsing password", vault_crypto::KdfParams::RECOMMENDED)
        .unwrap();
    vault_crypto::write_envelope_file(&envelope, &next).unwrap();
    rollback::resolve(&paths, None, true, Some("new browsing password"), Some(password)).unwrap();
    let (joined, _) = Vault::open_password(
        paths.clone(),
        vault_core::load_config(&paths).unwrap(),
        "new browsing password",
        None,
    )
    .unwrap();
    assert_eq!(visits(&joined), before);
    assert_eq!(
        joined.agent_history().unwrap().list_sessions(&Default::default()).unwrap().items[0]
            .native_id,
        "handoff-session"
    );
    assert_eq!(
        joined
            .store()
            .read(Db::Archive, |s| s.query_one(
                "SELECT value FROM archive_meta WHERE name='after-upgrade'",
                (),
                |r| Ok(r.get::<String>(0)?)
            ))
            .unwrap(),
        "retained"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linked_snapshots_refuse_before_replacement_and_allow_the_same_volume() {
    use std::os::unix::fs::{MetadataExt, symlink};
    let f = Fixture::new(false);
    f.seed();
    let v = f.vault();
    let paths = v.paths();
    let raw = &paths.raw_snapshots_dir;
    let here = f.sandbox.path().join("same-volume");
    fs::rename(raw, &here).unwrap();
    symlink(&here, raw).unwrap();
    let copy = vault_core::maintenance::manual_snapshot(&v, false).unwrap();
    let other = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_ne!(fs::metadata(other.path()).unwrap().dev(), fs::metadata(&here).unwrap().dev());
    fs::remove_file(raw).unwrap();
    symlink(other.path(), raw).unwrap();
    let before = fs::read(&paths.config_path).unwrap();
    let err = vault_core::maintenance::restore_snapshot(
        &v,
        Path::new(&copy.path),
        None,
        Default::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("snapshot-folder-cross-device"));
    assert_eq!(before, fs::read(&paths.config_path).unwrap());
    assert!(other.path().read_dir().unwrap().next().is_none());
    assert_eq!(search_titles(&v), ["pksearchelm"]);
}

#[test]
fn a_wipe_waits_for_an_admitted_browser_backup() {
    let f = Fixture::new(false);
    f.create_browser();
    let vault = f.vault();
    let (entered, ready) = std::sync::mpsc::channel();
    let (resume, paused) = std::sync::mpsc::channel();
    let (wiped, done) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let run = scope.spawn(|| {
            let mut first = true;
            vault_worker::run_backup_now_with_progress(&vault, None, false, move |_| {
                if first {
                    first = false;
                    entered.send(()).unwrap();
                    paused.recv().unwrap();
                }
            })
        });
        ready.recv_timeout(std::time::Duration::from_secs(15)).unwrap();
        let session = f.session.clone();
        let deletion = scope.spawn(move || {
            vault_worker::wipe_all_data("DELETE", || session.close_vault()).unwrap();
            wiped.send(()).unwrap();
        });
        assert!(done.recv_timeout(std::time::Duration::from_millis(150)).is_err());
        resume.send(()).unwrap();
        assert_eq!(run.join().unwrap().unwrap().run.unwrap().new_visits, 1);
        deletion.join().unwrap();
        assert!(!vault.paths().native_archive_path().exists());
    });
}

#[test]
fn a_legacy_move_file_keeps_current_conversations_and_identifies_the_old_version() {
    let f = Fixture::new(true);
    let local = f.vault();
    let tools = f.sandbox.path().join("pi");
    super::conversations::corpus::fixtures::pi(
        &tools,
        "/work",
        "local-before-move",
        "sole surviving copy",
    );
    local
        .agent_history()
        .unwrap()
        .backup(
            &[agent_history::SourceRoot {
                root: tools.clone(),
                harness: agent_history::Harness::Pi,
            }],
            &Default::default(),
        )
        .unwrap();
    fs::remove_dir_all(tools).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/v0.5-archive/move-file.pathkeep");
    let preview = vault_worker::preview_import(local.paths(), fixture.clone()).unwrap();
    assert!(!preview.version_known);
    let (_, installed) = vault_worker::apply_import(
        local.paths(),
        Some(&local),
        None,
        fixture,
        vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some("a second almanac for the move".into()),
        },
    )
    .unwrap();
    assert_eq!(
        installed.vault.agent_history().unwrap().list_sessions(&Default::default()).unwrap().items
            [0]
        .native_id,
        "local-before-move"
    );
    assert!(
        installed
            .vault
            .store()
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap()
            >= 1952
    );
}

#[test]
fn export_cleanup_keeps_equal_sibling_bytes_even_under_another_filename() {
    let f = Fixture::new(false);
    let exports = f.vault().paths().exports_dir.clone();
    fs::create_dir_all(&exports).unwrap();
    for (name, bytes) in [
        ("history.txt", "original"),
        (".pk-durable-twin", "original"),
        (".pk-durable-orphan", "leftover"),
    ] {
        fs::write(exports.join(name), bytes).unwrap();
    }
    assert_eq!(
        vault_core::durable_io::sweep_export_temps(&exports, std::time::Duration::ZERO).unwrap(),
        1
    );
    assert!(exports.join(".pk-durable-twin").is_file());
    assert_eq!(fs::read_to_string(exports.join("history.txt")).unwrap(), "original");
}

#[test]
fn a_lost_control_can_be_rebuilt_without_touching_the_browser_archive() {
    let f = Fixture::new(true);
    f.seed();
    let paths = f.vault().paths().clone();
    f.session.close_vault().unwrap();
    fs::remove_file(&paths.config_path).unwrap();
    let hash = vault_core::file_sha256_hex(&paths.native_archive_path()).unwrap();
    let repair = |password| {
        vault_core::maintenance::control_recovery::rebuild_settings(
            &paths,
            f.session.owner(),
            password,
        )
    };
    assert!(repair(None).unwrap_err().to_string().contains("archive-current-password-required"));
    assert!(repair(Some("wrong")).is_err());
    assert!(!paths.config_path.exists());
    repair(Some(PASSWORD)).unwrap();
    assert_eq!(hash, vault_core::file_sha256_hex(&paths.native_archive_path()).unwrap());
    let v = f.reopen(true);
    assert_eq!(search_titles(&v), ["pksearchelm"]);
}

/// Run under xvfb and a private D-Bus session with the unpacked released Linux executable.
/// The normal gate also exercises the checked-in encrypted release fixture above.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the released PathKeep v0.5.0 desktop executable"]
fn released_binary_can_back_up_into_the_handoff() {
    use std::time::{Duration, Instant};
    let bin = std::env::var("PATHKEEP_TEST_V05_DESKTOP").expect("released desktop path");
    let f = Fixture::new(false);
    f.session.close_vault().unwrap();
    fs::remove_dir_all(&f.root).unwrap();
    fs::create_dir(&f.root).unwrap();
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/v0.5-archive/data.tar.gz");
    assert!(
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&f.root)
            .status()
            .unwrap()
            .success()
    );
    let paths = vault_core::project_paths_with_root(&f.root);
    let config = vault_core::load_config(&paths).unwrap();
    let password = "harbour almanac 2026";
    vault_core::unlock_app_session(
        &paths,
        &config,
        &vault_core::UnlockAppSessionRequest {
            passcode: Some("4826".into()),
            use_biometric: false,
        },
    )
    .unwrap();
    let (v, _) = Vault::open_password(paths.clone(), config, password, None).unwrap();
    let v = vault_core::maintenance::switch_mode(
        &v,
        Some(password),
        vault_core::ArchiveMode::Plaintext,
        None,
        Default::default(),
        |_| {},
    )
    .unwrap()
    .vault;
    // The CLI can now open this plaintext test root without an OS keyring. Turn off the
    // fixture's display lock through its authenticated config before starting the old UI.
    let mut config = v.config().clone();
    config.app_lock.enabled = false;
    config.selected_profile_ids = vec!["chrome:Default".into()];
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let v = v.with_config(config);
    f.create_browser();
    assert_eq!(vault_core::run_backup(&v, None).unwrap().run.unwrap().new_visits, 1);
    rollback::detach(&paths, Some(&v), None, None).unwrap();
    let source = rusqlite::Connection::open(f.browser()).unwrap();
    source.execute_batch("INSERT INTO urls VALUES(2,'https://after-handoff.test/','Old release backup',1,1,13412823400000000,0);
        INSERT INTO visits VALUES(2,2,13412823400000000,0,1,1000000,0,NULL,NULL,NULL)").unwrap();
    drop(source);
    let short_root = f.root.canonicalize().unwrap();
    let mut child =
        Command::new(bin).env("CHB_PROJECT_ROOT", &short_root).arg("--background").spawn().unwrap();
    let client = vault_owner_ipc::Client::new(&short_root);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let started = Instant::now();
    let output = rt.block_on(async {
        let ping = || vault_owner_ipc::Request::new("ping", serde_json::json!({}));
        while let Err(reason) = client.request(ping(), |_| {}).await {
            if started.elapsed() > Duration::from_secs(30) || child.try_wait().unwrap().is_some() {
                return Err(format!(
                    "released owner did not start: {reason}; {}",
                    fs::read_to_string(f.root.join("logs/rust.log")).unwrap_or_default()
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        client
            .request(
                vault_owner_ipc::Request::new("backup", serde_json::json!({"dueOnly":false})),
                |_| {},
            )
            .await
            .map_err(|e| e.to_string())
    });
    // The command has committed before it replies. Wait for the background owner's normal exit.
    let deadline = Instant::now() + Duration::from_secs(155);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline && output.is_ok() {
        std::thread::sleep(Duration::from_millis(200));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
    }
    let output = output.unwrap();
    assert_eq!(output["run"]["newVisits"], 1, "{output}");
    rollback::resolve(&paths, None, false, None, None).unwrap();
    let v = Vault::open(paths.clone(), vault_core::load_config(&paths).unwrap(), None).unwrap();
    assert!(
        v.store()
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap()
            >= 1954
    );
}

#[test]
fn storage_accounts_for_local_files_and_only_frees_the_selected_retained_folder() {
    let f = Fixture::new(false);
    let vault = f.vault();
    let before = vault_core::load_dashboard_snapshot(&vault).unwrap().storage;
    for (rel, bytes) in [
        ("models/model.bin", 11),
        ("audit/other/entry", 12),
        ("schedule/rule", 13),
        ("integrations/rule", 14),
        ("wipe-trash-held/displaced/one", 15),
        ("maintenance.bak-old/config.json", 16),
    ] {
        let path = f.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![1; bytes]).unwrap();
    }
    let after = vault_core::load_dashboard_snapshot(&vault).unwrap().storage;
    assert_eq!(after.model_bytes - before.model_bytes, 11);
    assert_eq!(after.other_audit_bytes - before.other_audit_bytes, 12);
    assert_eq!(after.schedule_bytes - before.schedule_bytes, 13);
    assert_eq!(after.integration_bytes - before.integration_bytes, 14);
    assert_eq!(after.retained_copy_bytes - before.retained_copy_bytes, 31);
    let preview = vault_worker::preview_retention_plan().unwrap();
    let trash = preview.copies.iter().find(|c| c.source_op == "wipeTrash").unwrap();
    assert_eq!(trash.bytes, 15);
    assert!(!trash.restorable);
    let plan = vault_core::RetentionPruneRequest {
        copies: vec![trash.path.clone()],
        ..Default::default()
    };
    vault_worker::run_retention_plan(&vault, &plan).unwrap();
    assert!(!f.root.join("wipe-trash-held").exists());
    assert!(f.root.join("maintenance.bak-old/config.json").is_file());
    let failure = vault_worker::export_app_data(
        &vault,
        f.root.join("left-after-wipe.pathkeep"),
        EXPORT_PASSWORD,
    )
    .unwrap_err();
    assert!(failure.to_string().contains("export-target-is-managed"));
    assert!(!f.root.join("left-after-wipe.pathkeep").exists());
    let old = f.root.join("my old transfer");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/v0.5-archive/move-file.pathkeep"),
        &old,
    )
    .unwrap();
    let personal = f.root.join("unrelated.pathkeep");
    fs::write(&personal, b"personal file").unwrap();
    let wipe = vault_worker::preview_data_wipe().unwrap();
    assert!(wipe.items.iter().any(|item| Path::new(&item.path) == old));
    assert!(!wipe.items.iter().any(|item| Path::new(&item.path) == personal));
    f.session.wipe("DELETE").unwrap();
    assert!(!old.exists() && personal.is_file());
}

#[test]
fn restore_can_explicitly_take_the_copy_encryption_when_the_live_envelope_is_unusable() {
    let f = Fixture::new(true);
    f.seed();
    let vault = f.vault();
    let copy = vault_core::maintenance::manual_snapshot(&vault, false).unwrap();
    let paths = vault.paths().clone();
    f.session.close_vault().unwrap();
    fs::write(paths.artifact_path(vault_layout::ArtifactId::Envelope), b"unusable envelope")
        .unwrap();
    let request = vault_core::SnapshotRestoreRequest { snapshot_path: copy.path };
    let error =
        vault_worker::run_full_archive_restore(&f.session, Some(PASSWORD), &request).unwrap_err();
    assert!(error.to_string().contains("archive-current-password-required"), "{error:#}");
    let result = vault_worker::restore_with_current_password(
        &f.session,
        Some(PASSWORD),
        None,
        false,
        true,
        &request,
    )
    .unwrap();
    assert_eq!(
        fs::read(Path::new(&result.quarantine_dir).join("archive/keys.envelope")).unwrap(),
        b"unusable envelope"
    );
    let restored = f.session.vault().unwrap();
    assert_eq!(search_titles(&restored), ["pksearchelm"]);
    assert!(Vault::unlock_data_key(&paths, PASSWORD).is_ok());
}
