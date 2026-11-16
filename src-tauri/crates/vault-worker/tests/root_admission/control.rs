//! Responsibilities: distinguish absent control from damaged or unsupported control on real roots.
//! Not responsible for: publishing a stamp or treating a successful parse as owner admission.
use super::support::{self, Fixture, error};
use std::{fs, sync::Arc};
use vault_owner_ipc::{Client, Request};
// The lifecycle tests keep this file byte for byte as it was handed over; so does this one.
#[allow(dead_code)]
#[rustfmt::skip]
#[path = "../vault_lifecycle/support/expected_root.rs"]
mod expected_root;

#[test]
fn malformed_control_is_recovery_required_through_the_real_owner_without_changes() {
    let mut failures = Vec::new();
    let mut cases: Vec<(String, String, &str)> = [
        ("zero-floor.json", "archive-compatibility-invalid"),
        ("negative-floor.json", "archive-compatibility-invalid"),
        ("reader-after-writer.json", "archive-compatibility-invalid"),
        ("writer-after-revision.json", "archive-compatibility-invalid"),
        ("wrong-type.json", "archive-compatibility-invalid"),
        ("unknown-envelope.json", "archive-config-format-too-new"),
        ("invalid-json.json", "archive-control-invalid"),
        // Revision 2 with floors 1/1 is admitted at level 1. With no stores in it, the stamped
        // archive is missing what its layout requires: damage, not a first run.
        ("compatible-higher.json", "archive-store-missing"),
    ]
    .into_iter()
    .map(|(file, code)| (file.into(), support::fixture(file), code))
    .collect();
    let invalid: serde_json::Value =
        serde_json::from_str(&support::fixture("invalid-stamps.json")).unwrap();
    cases.extend(invalid.as_array().unwrap().iter().enumerate().map(|(index, stamp)| {
        (
            format!("invalid-stamps[{index}]"),
            format!(r#"{{"initialized":true,"archiveMode":{stamp}}}"#),
            "archive-compatibility-invalid",
        )
    }));
    for (file, control, code) in cases {
        let f = Fixture::new(false);
        f.session.reserve_owner(Arc::new(vault_store::OwnerLock::acquire(&f.root).unwrap()));
        let session = f.session.clone();
        let server = vault_owner_ipc::Server::start(&f.root, move |request, progress| {
            vault_worker::owner_requests::dispatch(request, progress, session.get_key(), || {
                session.background_vault().map_err(anyhow::Error::msg)
            })
        })
        .unwrap();
        f.write("config.json", control);
        let before = f.observe();
        let outside = f.outside();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = error(runtime.block_on(
            Client::new(&f.root).request(Request::new("doctor", serde_json::json!({})), |_| {}),
        ));
        let after = f.observe();
        if !result.contains(code) || before != after || outside != f.outside() {
            failures.push(format!(
                "{file}: expected {code}, got {result}; before={before:?}, after={after:?}"
            ));
        }
        drop(server);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn real_owner_distinguishes_missing_control_with_stores_from_uninitialized() {
    let mut failures = Vec::new();
    for (has_store, expected) in
        [(false, "archive-uninitialized"), (true, "archive-control-missing")]
    {
        let f = Fixture::new(false);
        f.session.reserve_owner(Arc::new(vault_store::OwnerLock::acquire(&f.root).unwrap()));
        let session = f.session.clone();
        let server = vault_owner_ipc::Server::start(&f.root, move |request, progress| {
            vault_worker::owner_requests::dispatch(request, progress, session.get_key(), || {
                session.background_vault().map_err(anyhow::Error::msg)
            })
        })
        .unwrap();
        if has_store {
            f.write("archive/source-evidence.db", "untouched existing data");
        }
        let before = f.observe();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_pathkeep-worker"))
            .arg("doctor")
            .output()
            .unwrap();
        let detail = String::from_utf8_lossy(&output.stderr);
        let after = f.observe();
        if output.status.code() != Some(1) || !detail.contains(expected) || before != after {
            failures.push(format!("has_store={has_store}: expected={expected}, status={:?}, error={detail}; before={before:?}; after={after:?}", output.status.code()));
        }
        drop(server);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn unreadable_control_is_recovery_required_without_effects() {
    let f = Fixture::new(false);
    // A directory in place of the file produces a portable read failure, even as root.
    fs::create_dir(f.root.join("config.json")).unwrap();
    let before = f.observe();
    let result = error(f.session.vault());
    let after = f.observe();
    assert!(
        result.contains("archive-control-unreadable") && before == after,
        "result={result}; before={before:?}; after={after:?}"
    );
}

#[test]
fn no_control_with_any_store_or_pending_record_is_never_first_run() {
    let mut failures = Vec::new();
    // These are literal independent presence cases, not a projection of production inventory.
    for path in [
        "archive/history-vault.db",
        "archive/source-evidence.db",
        "derived/agent.db",
        "derived/scratch.db",
        "archive/keys.envelope",
        "archive/.pk-import-journal.json",
        ".pk-wipe-in-progress.json",
    ] {
        let f = Fixture::new(false);
        f.write(path, "presence is enough; inspection must not open a database");
        let before = f.observe();
        let result = error(vault_core::load_config(&f.paths()));
        let after = f.observe();
        if !result.contains("archive-control-missing") || before != after {
            failures.push(format!("{path}: result={result}; before={before:?}; after={after:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn intact_stores_without_control_are_not_initialized_over() {
    let mut f = Fixture::new(true);
    f.session.close_vault().unwrap();
    f.vault.take().unwrap().close().unwrap();
    fs::remove_file(f.root.join("config.json")).unwrap();
    let before = f.observe();
    let outside = f.outside();
    let result = error(vault_worker::initialize_archive_database_with_progress(
        &f.session,
        &f.config,
        None,
        |_| {},
    ));
    let after = f.observe();
    assert!(
        result.contains("archive-control-missing") && before == after && outside == f.outside(),
        "result={result}; before={before:?}; after={after:?}"
    );
}

// "Delete all data" removes `config.json` first, so a wipe a crash cut short always looks like
// this: the marker, no control, and whatever was not yet deleted. The user confirmed it.
#[test]
fn a_wipe_cut_short_after_removing_the_control_is_finished() {
    let f = Fixture::new(false);
    f.write(".pk-wipe-in-progress.json", r#"{"providerIds":[],"schedule":null}"#);
    f.write("archive/source-evidence.db", "not deleted yet");
    f.write("derived/agent.db", "not deleted yet");
    assert!(vault_worker::finish_interrupted_data_wipe().unwrap());
    assert!(!f.root.join("archive/source-evidence.db").exists());
    assert!(!f.root.join("derived/agent.db").exists());
    assert!(!f.root.join(".pk-wipe-in-progress.json").exists());
    assert!(!vault_core::load_config(&f.paths()).unwrap().initialized);
}

// The wipe writes its marker whole or not at all. One that does not decode was not left by a
// wipe, and without a control nothing else says the user asked for a deletion.
#[test]
fn an_unreadable_wipe_marker_without_a_control_deletes_nothing() {
    let f = Fixture::new(false);
    f.write(".pk-wipe-in-progress.json", "{\"providerIds\":");
    f.write("archive/source-evidence.db", "must survive");
    let before = f.observe();
    let outside = f.outside();
    let result = error(vault_worker::finish_interrupted_data_wipe());
    assert!(
        result.contains("archive-control-missing")
            && before == f.observe()
            && outside == f.outside(),
        "{result}"
    );
}

// A restore, import or encryption change moves `config.json` aside before the new one lands. A
// crash in between leaves the journal and no control; the journal puts the old one back.
#[test]
fn an_install_cut_short_while_the_control_was_moved_aside_is_rolled_back() {
    let f = Fixture::new(false);
    let root = std::env::current_dir().unwrap().join(&f.root);
    f.write("maintenance.bak-fixture/config.json", support::LEGACY);
    f.write("staging/maintenance-fixture/archive/history-vault.db", "never installed");
    f.write(
        "archive/.pk-import-journal.json",
        serde_json::to_vec(&serde_json::json!({
            "stage": root.join("staging/maintenance-fixture"),
            "backup": root.join("maintenance.bak-fixture"),
            "entries": [
                {"relative": "config.json", "previous": true, "staged": true},
                {"relative": "archive/history-vault.db", "previous": false, "staged": true}
            ],
            "keep_previous": false, "committed": false
        }))
        .unwrap(),
    );
    assert!(f.session.recover_pending_archive(&f.paths()).unwrap());
    assert_eq!(fs::read_to_string(f.root.join("config.json")).unwrap(), support::LEGACY);
    assert!(!f.root.join("archive/.pk-import-journal.json").exists());
    assert!(!f.root.join("maintenance.bak-fixture").exists());
    assert!(vault_core::load_config(&f.paths()).unwrap().initialized);
}

// Whatever a finished wipe leaves behind has to read as a first run, or the user who deleted
// everything can never start again. That includes files that are not PathKeep's and an export
// the user saved into the data folder, which the wipe does not remove.
#[test]
fn after_delete_all_data_the_root_is_a_first_run_again() {
    let mut f = Fixture::new(true);
    vault_core::run_backup(f.vault(), None).unwrap();
    f.write(".DS_Store", "finder");
    f.write("my-export.pkexport", "an export saved into the data folder");
    f.write("schedule/logs/worker.stderr.log", "written by launchd");
    // With automatic backup installed, removing it during the wipe writes a record of the
    // removal after `audit/` is already gone.
    let plan = vault_worker::preview_schedule_plan(Some("macos"), None, None).unwrap();
    vault_worker::apply_schedule_plan(&plan).unwrap();
    f.session.close_vault().unwrap();
    f.vault.take().unwrap().close().unwrap();
    let report = vault_worker::wipe_all_data("DELETE", || Ok(())).unwrap();
    assert!(report.schedule_removed, "{report:?}");
    let left: Vec<_> = f.observe().entries.into_keys().collect();
    assert!(left.iter().any(|path| path.starts_with("audit/scheduler/")), "{left:?}");
    // launchd keeps writing here for a schedule the wipe could not remove.
    f.write("schedule/logs/worker.stderr.log", "archive has not been initialized");
    assert!(f.root.join("my-export.pkexport").exists());
    assert!(!vault_core::load_config(&f.paths()).unwrap().initialized);
    vault_worker::initialize_archive_database_with_progress(&f.session, &f.config, None, |_| {})
        .unwrap();
    assert!(vault_core::load_config(&f.paths()).unwrap().initialized);
}

// A damaged or lost `config.json` is what a restore repairs: the snapshot carries its own. The
// app refuses to open such a folder, so the recovery screen depends on these two calls working.
#[test]
fn a_restore_repairs_an_archive_whose_control_is_damaged_or_lost() {
    for damage in ["damaged", "lost"] {
        let mut f = Fixture::new(true);
        let source = f.root.join("raw-snapshots/maintenance/before-the-damage");
        vault_core::maintenance::snapshot(f.vault(), &source).unwrap();
        f.session.close_vault().unwrap();
        f.vault.take().unwrap().close().unwrap();
        let expected = if damage == "damaged" {
            f.write("config.json", "{ not json");
            "archive-control-invalid"
        } else {
            fs::remove_file(f.root.join("config.json")).unwrap();
            "archive-control-missing"
        };
        assert!(error(vault_core::load_config(&f.paths())).contains(expected), "{damage}");
        let listed = vault_worker::list_recovery_snapshots().unwrap();
        assert_eq!(listed.len(), 1, "{damage}: {listed:?}");
        vault_worker::run_full_archive_restore(
            &f.session,
            None,
            &vault_core::SnapshotRestoreRequest { snapshot_path: listed[0].path.clone() },
        )
        .unwrap_or_else(|error| panic!("{damage}: {error:#}"));
        assert!(vault_core::load_config(&f.paths()).unwrap().initialized, "{damage}");
        assert!(f.root.join("archive/history-vault.db").is_file(), "{damage}");
    }
}

// The same for a move-import at the level that replaces the archive. The worker's import
// command reads the target's settings first, as it always has, so it is not a way in here.
#[test]
fn an_import_replaces_an_archive_whose_control_is_damaged_or_lost() {
    for damage in ["damaged", "lost"] {
        let mut f = Fixture::new(true);
        let bundle = f.bundle_source();
        f.session.close_vault().unwrap();
        f.vault.take().unwrap().close().unwrap();
        if damage == "damaged" {
            f.write("config.json", "{ not json");
        } else {
            fs::remove_file(f.root.join("config.json")).unwrap();
        }
        let (_, installed) = vault_core::migration::native::import(
            &f.paths(),
            None,
            None,
            &bundle,
            &vault_core::ApplyImportOptions {
                source_archive_key: Some(support::PASSWORD.into()),
                confirm_overwrite: true,
            },
            Default::default(),
        )
        .unwrap_or_else(|error| panic!("{damage}: {error:#}"));
        installed.vault.close().unwrap();
        assert!(vault_core::load_config(&f.paths()).unwrap().initialized, "{damage}");
    }
}

// An import over a folder whose confirmed wipe was cut short settles that wipe first, as
// initialization does: nothing acts on the archive it then installs at the next start. Cut
// short before it had moved everything out, the wipe is undone, and the import replaces the
// archive as it would any other.
#[test]
fn an_import_settles_a_pending_wipe_before_it_installs() {
    let mut f = Fixture::new(true);
    let bundle = f.bundle_source();
    f.session.close_vault().unwrap();
    f.vault.take().unwrap().close().unwrap();
    let confirmed = fs::read_to_string(f.root.join("config.json")).unwrap();
    // What the wipe records before its first move: everything the user confirmed deleting, as
    // the preview listed it, but the control, which goes last. Then its first move. A record
    // joins names with `/` on every system, as the wipe writes it (`RecordedPath`).
    let mut targets: Vec<String> = vault_core::preview_data_wipe(&f.paths())
        .unwrap()
        .items
        .iter()
        .map(|item| {
            let relative = std::path::Path::new(&item.path).strip_prefix(&f.root).unwrap();
            relative.iter().map(|name| name.to_str().unwrap()).collect::<Vec<_>>().join("/")
        })
        .filter(|relative| relative != "config.json")
        .collect();
    let exports = f.root.join("exports");
    if exports.exists() {
        fs::create_dir_all(f.root.join("wipe-trash-fixture")).unwrap();
        fs::rename(&exports, f.root.join("wipe-trash-fixture/exports")).unwrap();
    } else {
        targets.push("exports".into());
    }
    f.write(
        ".pk-wipe-v2.json",
        serde_json::json!({
            "identity": "pathkeep-wipe", "version": 1, "control": confirmed, "emptied": "{}",
            "trash": "wipe-trash-fixture", "targets": targets, "committed": false,
            "providerIds": [], "schedule": null,
        })
        .to_string(),
    );
    f.write("wipe-trash-fixture/exports/moved.csv", "moved by the wipe");
    let (_, installed) = vault_worker::apply_import(
        &f.paths(),
        None,
        None,
        bundle,
        vault_core::ApplyImportOptions {
            source_archive_key: Some(support::PASSWORD.into()),
            confirm_overwrite: true,
        },
    )
    .unwrap();
    installed.vault.close().unwrap();
    assert!(!f.root.join(".pk-wipe-v2.json").exists());
    assert!(!f.root.join("wipe-trash-fixture").exists(), "what the wipe moved is back");
    assert_eq!(fs::read_to_string(f.root.join("exports/moved.csv")).unwrap(), "moved by the wipe");
    assert!(vault_core::load_config(&f.paths()).unwrap().initialized);
    assert!(!vault_worker::finish_interrupted_data_wipe().unwrap(), "nothing left to settle");
    assert!(f.root.join("archive/history-vault.db").is_file());
}

// Files PathKeep has no name for at the top of the data folder do not make it an archive.
#[test]
fn files_that_are_not_pathkeeps_do_not_block_a_first_run() {
    let f = Fixture::new(false);
    f.write(".DS_Store", "finder");
    f.write("unknown-store.db", "another program's file");
    f.write("notes/readme.txt", "another program's folder");
    let before = f.observe();
    assert!(!vault_core::load_config(&f.paths()).unwrap().initialized);
    assert_eq!(before, f.observe());
    vault_worker::initialize_archive_database_with_progress(&f.session, &f.config, None, |_| {})
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.root.join("unknown-store.db")).unwrap(),
        "another program's file"
    );
}

#[test]
fn empty_scaffolding_and_wipe_surviving_runtime_files_do_not_imply_an_archive() {
    let f = Fixture::new(false);
    for folder in [
        "archive",
        "derived",
        "staging",
        "quarantine",
        "raw-snapshots",
        "sidecars",
        "exports",
        "audit",
        "schedule",
        "models",
        "logs",
        "ipc",
    ] {
        fs::create_dir_all(f.root.join(folder)).unwrap();
    }
    for (path, bytes) in [
        ("archive/.pk-owner.lock", "previous pid"),
        ("archive/.pk-archive-write.lock", ""),
        ("logs/rust.log", "previous run log"),
    ] {
        assert!(expected_root::classify(path).unwrap().survives_wipe);
        f.write(path, bytes);
    }
    let before = f.observe();
    let config = vault_core::load_config(&f.paths()).unwrap();
    let after = f.observe();
    assert!(!config.initialized);
    assert_eq!(before, after);
}

#[test]
fn a_genuinely_empty_root_is_read_only_until_explicit_initialization() {
    let f = Fixture::new(false);
    let before = f.observe();
    let config = vault_core::load_config(&f.paths()).unwrap();
    assert!(!config.initialized);
    assert_eq!(before, f.observe());
    vault_worker::initialize_archive_database_with_progress(&f.session, &f.config, None, |_| {})
        .unwrap();
    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root.join("config.json")).unwrap()).unwrap();
    // A new archive is this build's format from the start: stamped, every store of the layout.
    assert_eq!(persisted["archiveMode"]["mode"], "Plaintext");
    assert_eq!(
        persisted["archiveMode"]["compatibility"],
        serde_json::json!({"revision": 1, "minReader": 1, "minWriter": 1})
    );
    for store in [
        "archive/history-vault.db",
        "archive/source-evidence.db",
        "archive/agent-history.db",
        "derived/agent-history-read-model.db",
    ] {
        assert!(f.root.join(store).is_file(), "{store}");
    }
    assert!(!f.root.join("archive/.pk-install-v2.json").exists(), "the creation record is gone");
}

#[test]
fn onboarding_snapshot_of_an_empty_root_does_not_create_diagnostics_scaffolding() {
    let f = Fixture::new(false);
    let before = f.observe();
    let snapshot = vault_worker::app_snapshot(None).unwrap();
    let after = f.observe();
    assert!(!snapshot.config.initialized);
    assert_eq!(before, after);
}
