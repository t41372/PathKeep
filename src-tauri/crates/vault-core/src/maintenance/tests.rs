//! Responsibilities: acceptance of native maintenance on real project roots.
//! Not responsible for: browser parsing or UI flows.
//! Failure modes: rewrap changes data/key; wrong passwords mutate the envelope;
//! copying loses a database; cancel replaces the source; recovery mixes generations.
use super::*;
use crate::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use vault_store::{Db, Priority};

pub(crate) fn encrypted() -> (tempfile::TempDir, Vault, vault_crypto::DataKey) {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    crate::save_config(&crate::ControlGate::acquire(&paths), &config).unwrap();
    let (vault, key) = Vault::open_password(paths, config, "old password", None).unwrap();
    (root, vault, key)
}

#[test]
fn mode_switch_round_trips_all_databases_and_cancel_keeps_original() {
    let vault = Vault::open_temp().unwrap();
    crate::test_support::search::seed_pages(&vault, 2);
    crate::config::ensure_paths(vault.paths()).unwrap();
    crate::save_config(&crate::ControlGate::acquire(vault.paths()), vault.config()).unwrap();
    for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
        vault
            .store()
            .write(db, Priority::Batch, |s| {
                s.execute(
                    "CREATE TABLE maintenance_sentinel(id INTEGER PRIMARY KEY,body TEXT)",
                    (),
                )?;
                s.execute(
                    "INSERT INTO maintenance_sentinel VALUES(1,'retain every database')",
                    (),
                )?;
                Ok(())
            })
            .unwrap();
    }
    let cancel = vault_store::CancelToken::new();
    let stop = cancel.clone();
    assert!(
        switch_mode(&vault, None, ArchiveMode::Encrypted, Some("new"), cancel, move |progress| {
            if progress.phase == "copying" && progress.processed > 0 {
                stop.cancel();
            }
        },)
        .is_err()
    );
    assert!(vault.store().is_open());
    let encrypted =
        switch_mode(&vault, None, ArchiveMode::Encrypted, Some("new"), Default::default(), |_| {})
            .unwrap();
    assert_search(&encrypted.vault);
    crate::archive::check_config_disk_consistency(encrypted.vault.paths()).unwrap();
    assert!(encrypted.previous_files.join("snapshot.json").is_file());
    assert!(
        Vault::unlock_data_key(&project_paths_with_root(&encrypted.previous_files), "new").is_ok()
    );
    let plain = switch_mode(
        &encrypted.vault,
        Some("new"),
        ArchiveMode::Plaintext,
        None,
        Default::default(),
        |_| {},
    )
    .unwrap();
    assert_search(&plain.vault);
    crate::archive::check_config_disk_consistency(plain.vault.paths()).unwrap();
    assert!(plain.previous_files.join("snapshot.json").is_file());
    for db in [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent] {
        let body = plain
            .vault
            .store()
            .read(db, |s| {
                s.query_one("SELECT body FROM maintenance_sentinel WHERE id=1", (), |r| {
                    Ok(r.get::<String>(0)?)
                })
            })
            .unwrap();
        assert_eq!(body, "retain every database");
    }
    assert!(!plain.vault.paths().app_root.join("archive/keys.envelope").exists());
}

fn assert_search(vault: &Vault) {
    vault.search().cycle().unwrap();
    let mut query = crate::search::SearchSession::start(
        vault,
        crate::search::SearchRequest { query: "ancienttransport".into(), ..Default::default() },
    )
    .unwrap();
    assert_eq!(query.finish_page().unwrap().items[0].page_id, 1);
}

#[test]
fn snapshot_restores_evidence_and_agent_alongside_the_archive() {
    let (_root, vault, _key) = encrypted();
    vault.store().write(Db::Archive,Priority::Batch,|s|{
        s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url','https://example.com/','before')",())?;Ok(())
    }).unwrap();
    let path = vault.paths().raw_snapshots_dir.join("maintenance/test");
    snapshot(&vault, &path).unwrap();
    vault
        .store()
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute("DELETE FROM star", ())?;
            Ok(())
        })
        .unwrap();
    let restored =
        restore_snapshot(&vault, &path, Some("old password"), Default::default()).unwrap();
    crate::archive::check_config_disk_consistency(restored.vault.paths()).unwrap();
    assert_eq!(
        restored
            .vault
            .store()
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM star", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        1
    );
}

#[test]
fn startup_rolls_back_a_mode_switch_interrupted_after_the_first_install() {
    let (_root, vault, _key) = encrypted();
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    let original = std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap();
    let fault = crate::fault_inject::FaultGuard::panic_at("native-install-installed");
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        switch_mode(
            &vault,
            Some("old password"),
            ArchiveMode::Plaintext,
            None,
            Default::default(),
            |_| {},
        )
        .unwrap();
    }));
    drop(fault);
    assert!(crashed.is_err());
    assert!(crate::maintenance::install::pending(&paths));
    crate::recover_archive_on_launch(&paths, &config).unwrap();
    assert!(!crate::maintenance::install::pending(&paths));
    assert_eq!(std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap(), original);
    crate::archive::check_config_disk_consistency(&paths).unwrap();
    let (opened, _) = Vault::open_password(paths, config, "old password", None).unwrap();
    opened.close().unwrap();
}

#[test]
fn startup_rolls_back_interrupted_bundle_import_and_snapshot_restore() {
    for bundle in [false, true] {
        let (_root, vault, _key) = encrypted();
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        let snapshot_path = paths.raw_snapshots_dir.join("maintenance/restore-test");
        let bundle_path = paths.app_root.join("exports/pathkeep/format-3/portable.pathkeep");
        snapshot(&vault, &snapshot_path).unwrap();
        if bundle {
            crate::migration::native::export(
                &vault,
                &bundle_path,
                "export password",
                Default::default(),
                |_| {},
            )
            .unwrap();
        }
        vault.store().write(Db::Archive,Priority::Interactive,|s|{
            s.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url','https://example.com/','after')",())?;Ok(())
        }).unwrap();
        let fault = crate::fault_inject::FaultGuard::panic_at("native-install-installed");
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if bundle {
                crate::migration::native::import(
                    &paths,
                    Some(&vault),
                    None,
                    &bundle_path,
                    &crate::ApplyImportOptions {
                        confirm_overwrite: true,
                        source_archive_key: Some("export password".into()),
                    },
                    Default::default(),
                )
                .unwrap();
            } else {
                restore_snapshot(&vault, &snapshot_path, Some("old password"), Default::default())
                    .unwrap();
            }
        }));
        drop(fault);
        assert!(crash.is_err());
        let (recovered, _) = Vault::open_password(paths, config, "old password", None).unwrap();
        assert_eq!(
            recovered
                .store()
                .read(Db::Archive, |s| s
                    .query_one("SELECT count(*) FROM star", (), |r| Ok(r.get::<i64>(0)?)))
                .unwrap(),
            1
        );
    }
}

#[test]
fn incomplete_snapshot_manifest_cannot_create_empty_replacement_databases() {
    let vault = Vault::open_temp().unwrap();
    let root = tempfile::tempdir().unwrap();
    snapshot(&vault, root.path()).unwrap();
    let manifest = root.path().join("snapshot.json");
    let mut proof: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    proof.retain(|entry| entry["path"] != "archive/source-evidence.db");
    fs::write(manifest, serde_json::to_vec(&proof).unwrap()).unwrap();
    fs::remove_file(root.path().join("archive/source-evidence.db")).unwrap();
    assert!(restore_snapshot(&vault, root.path(), None, Default::default()).is_err());
    assert!(vault.store().is_open());
}

fn plaintext_archive() -> (tempfile::TempDir, crate::ProjectPaths, Vault) {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Plaintext,
        ..Default::default()
    };
    crate::save_config(&crate::ControlGate::acquire(&paths), &config).unwrap();
    let vault = Vault::open(paths.clone(), config, None).unwrap();
    (root, paths, vault)
}

fn in_chinese(config: &mut AppConfig) {
    config.preferred_language = crate::LanguagePreference::ZhTw;
}

fn is_chinese(config: &AppConfig) -> bool {
    matches!(config.preferred_language, crate::LanguagePreference::ZhTw)
}

// The repair compares the files with the control on disk, and changes only the mode on it: the
// settings the archive was opened with are older than a save made since.
#[test]
fn the_at_rest_repair_keeps_a_setting_saved_after_the_archive_opened() {
    let (_root, paths, vault) = plaintext_archive();
    {
        let gate = crate::ControlGate::acquire(&paths);
        let mut later = crate::load_config(&paths).unwrap();
        in_chinese(&mut later);
        later.archive_mode = ArchiveMode::Encrypted; // stale: the files are plaintext
        crate::save_config(&gate, &later).unwrap();
    }
    let report = reconcile(&vault).unwrap();
    assert!(report.repaired);
    let on_disk = crate::load_config(&paths).unwrap();
    assert_eq!(on_disk.archive_mode, ArchiveMode::Plaintext);
    assert!(is_chinese(&on_disk), "the setting saved after the open is kept");
}

// The copy takes minutes on a real archive and settings stay usable meanwhile: the replacement's
// control is the live one at install time with only the mode changed.
#[test]
fn a_setting_saved_while_a_mode_change_copies_is_kept() {
    let (_root, paths, vault) = plaintext_archive();
    let (paused, copying) = std::sync::mpsc::channel();
    let (resume, resumed) = std::sync::mpsc::channel::<()>();
    let mut first = true;
    let switching = std::thread::spawn(move || {
        let progress = move |_: Progress| {
            if std::mem::take(&mut first) {
                paused.send(()).unwrap();
                resumed.recv().unwrap();
            }
        };
        switch_mode(&vault, None, ArchiveMode::Encrypted, Some("new"), Default::default(), progress)
            .map(|installed| installed.vault.close())
    });
    copying.recv_timeout(std::time::Duration::from_secs(60)).expect("the copy reports progress");
    {
        let gate = crate::ControlGate::acquire(&paths);
        let mut current = crate::load_config(&paths).unwrap();
        in_chinese(&mut current);
        crate::save_config(&gate, &current).unwrap();
    }
    resume.send(()).unwrap();
    switching.join().unwrap().unwrap().unwrap();
    let on_disk = crate::load_config(&paths).unwrap();
    assert_eq!(on_disk.archive_mode, ArchiveMode::Encrypted);
    assert!(is_chinese(&on_disk), "the setting saved during the copy is kept");
}

#[test]
fn device_policy_survives_rewrap_switch_and_restore_only_on_the_live_archive() {
    // A safety copy must not acquire the ability to inspect any of these tools.
    let (_root, vault, _key) = encrypted();
    let mut config = vault.config().clone();
    config.dismissed_agent_hints.insert(crate::AgentHint::Intro);
    config.capture_policy.explicitly_off = true;
    config.capture_policy.empty_tools_chosen = true;
    config.capture_policy.read_git = false;
    config.capture_policy.selection_ref = Some("device-grant".into());
    config.capture_policy.pending_first_save = Some("authorized-first-save".into());
    crate::save_config(&crate::ControlGate::acquire(vault.paths()), &config).unwrap();
    let vault = vault.with_config(config.clone());
    let snap = vault.paths().raw_snapshots_dir.join("maintenance/policy");
    snapshot(&vault, &snap).unwrap();
    let copy_policy = |path: &std::path::Path| {
        crate::load_config(&crate::project_paths_with_root(path)).unwrap().capture_policy
    };
    let hints = |path: &std::path::Path| {
        crate::load_config(&crate::project_paths_with_root(path)).unwrap().dismissed_agent_hints
    };
    assert!(hints(&snap).is_empty());
    assert_eq!(copy_policy(&snap), Default::default());
    rewrap_password(&vault, "old password", "new password").unwrap();
    assert_eq!(copy_policy(&vault.paths().app_root), config.capture_policy);
    let restored =
        restore_snapshot(&vault, &snap, Some("old password"), Default::default()).unwrap();
    assert_eq!(restored.vault.config().capture_policy, config.capture_policy);
    assert!(hints(&restored.previous_files).is_empty());
    assert_eq!(hints(&restored.vault.paths().app_root), config.dismissed_agent_hints);
    assert_eq!(copy_policy(&restored.previous_files), Default::default());
    let plain = switch_mode(
        &restored.vault,
        Some("new password"),
        ArchiveMode::Plaintext,
        None,
        Default::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(plain.vault.config().capture_policy, config.capture_policy);
    assert!(hints(&plain.previous_files).is_empty());
    assert_eq!(hints(&plain.vault.paths().app_root), config.dismissed_agent_hints);
    assert_eq!(copy_policy(&plain.previous_files), Default::default());
}
