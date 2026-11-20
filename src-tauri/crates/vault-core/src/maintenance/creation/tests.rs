//! Responsibilities: prove that an archive of the released layout comes up to this build's layout
//! whichever way it arrives: opened in place (the promotion, cut by a crash at each of its points),
//! restored from a snapshot PathKeep 0.5 took, or imported from a file PathKeep 0.5 exported; and
//! that a stamped archive missing a store is refused rather than refilled empty.
//! Not responsible for: what the agent-history stores hold (the agent-history crate) or the
//! install journal (`install_tests`).
//! Failure modes: a crash leaves a stamped archive whose next open creates an empty store where
//! data was; a record deletes or writes over a store, an older release's since the crash or another
//! archive's it was carried into; a store that cannot be looked at is taken for a missing one; a
//! record names a file that is not a store, or a folder it did not make; the record or its folder
//! outlives a finished promotion; a later build's compatible stamp is lowered by a save, a mode
//! change or a restore; a 0.5 snapshot or file installs without the new stores, or keeps the
//! conversations of the archive it replaced.
use super::*;
use crate::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use std::{collections::BTreeMap, fs};
use vault_store::{Db, Priority};

/// The stores PathKeep 0.5 opens.
const RELEASED: [Db; 5] =
    [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent, Db::Scratch];
/// The stores this build adds, as files.
pub(in crate::maintenance) const ADDED: [&str; 2] =
    ["archive/agent-history.db", "derived/agent-history-read-model.db"];

pub(in crate::maintenance) fn released_config() -> AppConfig {
    AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Plaintext,
        ..Default::default()
    }
}

/// A folder as PathKeep 0.5 leaves it: an initialized control without a stamp, its five stores,
/// and a marker in the browsing archive.
pub(crate) fn released_archive() -> (tempfile::TempDir, ProjectPaths) {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    crate::save_config(&crate::ControlGate::acquire(&paths), &released_config()).unwrap();
    let store = released_store(&paths);
    write_marker(&store, Db::Archive, "released-archive");
    store.close().unwrap();
    (root, paths)
}

/// The store as PathKeep 0.5 opens it: its five stores and none of this build's recovery.
fn released_store(paths: &ProjectPaths) -> vault_store::Store {
    let mut config = vault_store::Config::new(&paths.app_root);
    config.databases = RELEASED.to_vec();
    config.migrations = crate::store_schema::migration_registry();
    vault_store::Store::open(config).unwrap()
}

pub(in crate::maintenance) fn write_marker(store: &vault_store::Store, db: Db, body: &'static str) {
    store
        .write(db, Priority::Interactive, move |s| {
            s.execute("CREATE TABLE IF NOT EXISTS creation_marker(body TEXT NOT NULL)", ())?;
            s.execute("DELETE FROM creation_marker", ())?;
            s.execute("INSERT INTO creation_marker VALUES(?1)", [body])?;
            Ok(())
        })
        .unwrap();
}

/// The marker a store holds; `None` for a store nothing was written to.
pub(in crate::maintenance) fn marker(vault: &Vault, db: Db) -> Option<String> {
    vault
        .store()
        .read(db, |s| {
            let exists = s.query_one(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='creation_marker'",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            if exists == 0 {
                return Ok(None);
            }
            s.query_one("SELECT body FROM creation_marker", (), |r| Ok(Some(r.get::<String>(0)?)))
        })
        .unwrap()
}

fn layout(paths: &ProjectPaths) -> ArchiveLayout {
    crate::admission::root_layout(&paths.app_root).unwrap()
}

pub(in crate::maintenance) fn open(paths: &ProjectPaths) -> anyhow::Result<Vault> {
    Ok(Vault::open(paths.clone(), released_config(), None)?)
}

/// Opens the archive with a crash armed at `point`.
fn crash_at(paths: &ProjectPaths, point: &str) {
    let fault = crate::fault_inject::FaultGuard::panic_at(point);
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open(paths)));
    drop(fault);
    assert!(crashed.is_err(), "the open was cut at {point}");
}

fn remove_store(paths: &ProjectPaths, relative: &str) {
    let main = paths.app_root.join(relative);
    for suffix in vault_layout::CompanionFamily::Native.suffixes(CompanionUse::Install) {
        let file = PathBuf::from(format!("{}{suffix}", main.display()));
        match fs::remove_file(&file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => panic!("{error}"),
            _ => {}
        }
    }
}

/// The archive's control with this build's stamp, as a bring-up writes it: a record that names
/// anything wrong is refused for that, not for its control.
fn stamped_control(paths: &ProjectPaths) -> String {
    let config = crate::load_config(paths).unwrap();
    crate::config::encode_config(&config, Some(&crate::admission::written_compatibility())).unwrap()
}

pub(in crate::maintenance) fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    found
}

/// The folders stores were made in that are still under `staging/`.
fn made_folders(paths: &ProjectPaths) -> Vec<PathBuf> {
    match fs::read_dir(&paths.staging_dir) {
        Ok(entries) => entries
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with("creating-"))
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn every_cut_of_the_promotion_is_finished_by_the_next_open() {
    // The folder goes while the record still names it, before the record: none is ever left
    // that no record names.
    for (point, recorded, stamped, moved, folders) in [
        ("promotion-made", false, false, false, 1),
        ("promotion-recorded", true, false, false, 1),
        ("promotion-stamped", true, true, false, 1),
        ("promotion-moved", true, true, true, 0),
    ] {
        let (_root, paths) = released_archive();
        crash_at(&paths, point);
        assert_eq!(read(&paths).unwrap().is_some(), recorded, "{point}: the record");
        assert_eq!(layout(&paths) == ArchiveLayout::AgentHistoryV2, stamped, "{point}");
        for added in ADDED {
            assert_eq!(paths.app_root.join(added).is_file(), moved, "{point}: {added}");
        }
        assert_eq!(made_folders(&paths).len(), folders, "{point}: the folder they were made in");

        let vault = open(&paths).unwrap();
        assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2, "{point}");
        assert!(read(&paths).unwrap().is_none(), "{point}: the record is removed");
        assert_eq!(marker(&vault, Db::Archive).as_deref(), Some("released-archive"), "{point}");
        for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            assert_eq!(marker(&vault, db), None, "{point}: {db:?} starts empty");
            write_marker(vault.store(), db, "usable");
            assert_eq!(marker(&vault, db).as_deref(), Some("usable"), "{point}: {db:?}");
        }
        vault.close().unwrap();
        drop(vault);

        assert!(made_folders(&paths).is_empty(), "{point}: no folder is left");
        // Brought up once: the next open rewrites nothing and records nothing.
        let control = fs::read(&paths.config_path).unwrap();
        drop(open(&paths).unwrap());
        assert_eq!(fs::read(&paths.config_path).unwrap(), control, "{point}");
        assert!(read(&paths).unwrap().is_none(), "{point}");
    }
}

// The folder the stores were made in is gone after the stamp (cleaned by hand, or not copied with
// the rest of the folder). The record still says they are being made: the next open makes them
// again rather than refusing the archive or creating them in place.
#[test]
fn stores_the_folder_lost_after_the_stamp_are_made_again() {
    let (_root, paths) = released_archive();
    crash_at(&paths, "promotion-stamped");
    for folder in made_folders(&paths) {
        fs::remove_dir_all(folder).unwrap();
    }
    let vault = open(&paths).unwrap();
    assert_eq!(marker(&vault, Db::Archive).as_deref(), Some("released-archive"));
    for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        write_marker(vault.store(), db, "usable");
        assert_eq!(marker(&vault, db).as_deref(), Some("usable"), "{db:?}");
    }
    assert!(read(&paths).unwrap().is_none());
    assert!(made_folders(&paths).is_empty());
}

// A store's log left in its place without the store (a file put there by hand, or left by another
// tool): a new store beside it would be read with another file's log. Nothing is moved in, and the
// archive is refused rather than opened that way.
#[test]
fn a_log_without_its_store_keeps_the_new_store_out() {
    let (_root, paths) = released_archive();
    crash_at(&paths, "promotion-stamped");
    let log = paths.app_root.join("archive/agent-history.db-wal");
    fs::write(&log, b"another store's log").unwrap();
    let before = files(&paths.app_root);
    let Err(error) = open(&paths) else { panic!("opened") };
    assert!(format!("{error:#}").contains("archive-store-missing"), "{error:#}");
    assert_eq!(files(&paths.app_root), before, "nothing was moved in");
}

// A store that cannot be looked at (its folder lost its permissions) is not one that is missing.
// Taken for one, it was recorded as being made, and the next open after the stamp made it again
// over the conversations in it. The bring-up stops instead, before it records or stamps anything.
#[cfg(unix)]
#[test]
fn a_store_that_cannot_be_looked_at_is_not_taken_for_a_missing_one() {
    use std::os::unix::fs::PermissionsExt;
    let (_root, paths) = released_archive();
    let store = released_store(&paths);
    write_marker(&store, Db::Agent, "an ask conversation");
    store.close().unwrap();
    drop(store);
    let derived = paths.app_root.join("derived");
    fs::set_permissions(&derived, fs::Permissions::from_mode(0o000)).unwrap();
    let refused = open(&paths);
    fs::set_permissions(&derived, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(refused.is_err(), "the open stops while the folder cannot be read");
    assert!(read(&paths).unwrap().is_none(), "nothing was recorded");
    assert_eq!(layout(&paths), ArchiveLayout::BrowsingV1, "nothing was stamped");

    let vault = open(&paths).unwrap();
    assert_eq!(marker(&vault, Db::Agent).as_deref(), Some("an ask conversation"));
    assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2);
}

// A new archive: setup wrote its control, and the first open makes every store. Before the stamp,
// PathKeep 0.5 can open the folder, and it creates the stores it lacks, which the record also
// names. Deleting what the record names would delete what 0.5 saved since.
#[test]
fn a_crash_before_the_stamp_never_deletes_what_an_older_release_saved_since() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    crate::config::ensure_paths(&paths).unwrap();
    crate::save_config(&crate::ControlGate::acquire(&paths), &released_config()).unwrap();
    crash_at(&paths, "promotion-recorded");
    let record = read(&paths).unwrap().expect("the crash left a record");
    assert!(record.creating.iter().any(|made| *made.relative == *Path::new("derived/agent.db")));
    assert_eq!(layout(&paths), ArchiveLayout::BrowsingV1);

    // PathKeep 0.5 does not know the record and opens the folder.
    let store = released_store(&paths);
    write_marker(&store, Db::Agent, "saved-by-0.5");
    store.close().unwrap();
    drop(store);

    let vault = open(&paths).unwrap();
    assert_eq!(marker(&vault, Db::Agent).as_deref(), Some("saved-by-0.5"));
    assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2);
    assert!(read(&paths).unwrap().is_none());
}

// An archive of PathKeep 0.5 that lost one of its own stores, its Ask conversations say. The
// bring-up makes only the stores this layout adds: making the lost one empty and stamping the
// archive would pass the loss off as part of the upgrade. The open is refused, and nothing is
// made or stamped.
#[test]
fn a_released_archive_missing_one_of_its_stores_is_refused_not_refilled() {
    for lost in ["derived/agent.db", "derived/intelligence.db", "archive/source-evidence.db"] {
        let (_root, paths) = released_archive();
        remove_store(&paths, lost);
        let before = files(&paths.app_root);
        let Err(error) = open(&paths) else { panic!("{lost}: opened") };
        assert!(format!("{error:#}").contains("archive-store-missing"), "{lost}: {error:#}");
        assert_eq!(layout(&paths), ArchiveLayout::BrowsingV1, "{lost}");
        assert!(!paths.app_root.join(lost).exists(), "{lost}");
        let after = files(&paths.app_root);
        assert!(
            after.keys().all(|path| before.contains_key(path)),
            "{lost}: {:?}",
            after.keys().filter(|path| !before.contains_key(*path)).collect::<Vec<_>>()
        );
    }
}

// A crash before the stamp, then PathKeep 0.5 changes the archive's password or turns encryption
// on or off (it can: there is no stamp). A store made before the crash, with the key the archive
// had, would not open with the archive any more. So nothing made before the stamp is ever moved
// in: the next open makes the stores again, for the archive as it is then. The bytes written below
// stand for a store made under the old key.
#[test]
fn nothing_made_before_the_stamp_enters_the_archive() {
    let (_root, paths) = released_archive();
    crash_at(&paths, "promotion-recorded");
    let record = read(&paths).unwrap().expect("the crash left a record");
    let made = paths.app_root.join(&record.made_in).join(ADDED[0]);
    assert!(made.is_file(), "the store was made before the record");
    fs::write(&made, b"made under the key the archive had").unwrap();

    let vault = open(&paths).unwrap();
    assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2);
    assert_eq!(marker(&vault, Db::Archive).as_deref(), Some("released-archive"));
    assert_ne!(
        fs::read(paths.app_root.join(ADDED[0])).unwrap(),
        b"made under the key the archive had"
    );
    assert!(made_folders(&paths).is_empty());
    assert!(read(&paths).unwrap().is_none());
}

// A crash during the bring-up, then its record carried into another archive's folder (the two
// folders merged, or the record restored over the other one), with or without the folder it names.
// That archive is stamped and keeps conversations in the stores the record names. Nothing is moved
// over them, nothing of them is deleted, and the record goes.
#[test]
fn a_creation_record_carried_into_another_archive_changes_none_of_its_stores() {
    for with_folder in [false, true] {
        let (_first_root, first) = released_archive();
        crash_at(&first, "promotion-stamped");
        let (_second_root, second) = released_archive();
        {
            let vault = open(&second).unwrap();
            write_marker(vault.store(), Db::AgentHistoryEvidence, "the other archive's");
            write_marker(vault.store(), Db::AgentHistoryReadModel, "the other archive's");
            vault.close().unwrap();
        }
        fs::copy(record_path(&first), record_path(&second)).unwrap();
        if with_folder {
            let made = read(&first).unwrap().unwrap().made_in;
            for file in files(&first.app_root.join(&made)).into_keys() {
                let to = second
                    .app_root
                    .join(&made)
                    .join(file.strip_prefix(first.app_root.join(&made)).unwrap());
                fs::create_dir_all(to.parent().unwrap()).unwrap();
                fs::copy(&file, &to).unwrap();
            }
        }

        let vault = open(&second).unwrap();
        for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            assert_eq!(marker(&vault, db).as_deref(), Some("the other archive's"), "{db:?}");
        }
        assert!(read(&second).unwrap().is_none(), "the record is gone");
        assert!(made_folders(&second).is_empty(), "with its folder");
        vault.close().unwrap();
        // The folder it came from is still brought up from it.
        let vault = open(&first).unwrap();
        assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2);
        assert!(read(&first).unwrap().is_none());
    }
}

#[test]
fn a_stamped_archive_missing_a_browsing_store_is_refused_and_left_as_it_is() {
    for missing in ["derived/agent.db", "archive/history-vault.db"] {
        let (_root, paths) = released_archive();
        drop(open(&paths).unwrap());
        remove_store(&paths, missing);
        let before = files(&paths.app_root);
        let Err(error) = open(&paths) else { panic!("opened") };
        assert!(format!("{error:#}").contains("archive-store-missing"), "{missing}: {error:#}");
        assert_eq!(files(&paths.app_root), before, "{missing}: nothing was created");
    }
}

#[test]
fn a_record_naming_anything_but_a_store_is_refused_and_deletes_nothing() {
    for named in [
        "config.json",
        "archive/keys.envelope",
        "archive/history-vault.db-wal",
        "archive/../config.json",
        "../elsewhere.db",
    ] {
        let (_root, paths) = released_archive();
        drop(open(&paths).unwrap());
        let record = serde_json::json!({
            "identity": "pathkeep-create", "version": 1,
            "creating": [{"relative": named, "holds": "file:0"}],
            "made_in": "staging/creating-x", "control": stamped_control(&paths),
        });
        fs::write(record_path(&paths), serde_json::to_vec(&record).unwrap()).unwrap();
        let before = files(&paths.app_root);
        let Err(error) = open(&paths) else { panic!("opened") };
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{named}: {error:#}");
        assert_eq!(files(&paths.app_root), before, "{named}: nothing was deleted");
    }
}

// A record naming a folder this build does not make stores in is refused, and nothing in that
// folder, or anywhere, is moved or deleted.
#[test]
fn a_record_naming_another_folder_is_refused_and_moves_nothing() {
    for folder in [
        "staging",
        "archive",
        "staging/maintenance-x",
        "staging/creating-x/inner",
        "../creating-x",
        "creating-x",
    ] {
        let (_root, paths) = released_archive();
        drop(open(&paths).unwrap());
        let record = serde_json::json!({
            "identity": "pathkeep-create", "version": 1,
            "creating": [{"relative": "archive/agent-history.db", "holds": "file:0"}],
            "made_in": folder, "control": stamped_control(&paths),
        });
        fs::write(record_path(&paths), serde_json::to_vec(&record).unwrap()).unwrap();
        let before = files(&paths.app_root);
        let Err(error) = open(&paths) else { panic!("opened") };
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{folder}: {error:#}");
        assert_eq!(files(&paths.app_root), before, "{folder}: nothing changed");
    }
}

#[test]
fn a_snapshot_pathkeep_0_5_took_restores_browsing_and_keeps_the_conversations() {
    let (_root, paths) = released_archive();
    let snapshots = tempfile::tempdir().unwrap();
    let taken = snapshots.path().join("taken-by-0.5");
    {
        // The snapshot as 0.5 takes it: its five stores and its control, without a stamp.
        let released = Vault::open_staged(paths.clone(), released_config(), None).unwrap();
        assert_eq!(released.layout(), ArchiveLayout::BrowsingV1);
        crate::maintenance::snapshot::snapshot(&released, &taken).unwrap();
        released.close().unwrap();
    }
    assert_eq!(crate::admission::root_layout(&taken).unwrap(), ArchiveLayout::BrowsingV1);
    for added in ADDED {
        assert!(!taken.join(added).exists());
    }

    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "changed-since");
    write_marker(live.store(), Db::AgentHistoryEvidence, "saved-since");
    write_marker(live.store(), Db::AgentHistoryReadModel, "read-since");
    let installed =
        crate::maintenance::snapshot::restore_snapshot(&live, &taken, None, Default::default())
            .unwrap();
    let restored = &installed.vault;
    assert_eq!(restored.layout(), ArchiveLayout::AgentHistoryV2);
    assert_eq!(layout(&paths), ArchiveLayout::AgentHistoryV2);
    assert_eq!(marker(restored, Db::Archive).as_deref(), Some("released-archive"));
    // A restore does not roll agent conversations back: the copy has none, and they stay.
    assert_eq!(marker(restored, Db::AgentHistoryEvidence).as_deref(), Some("saved-since"));
    assert_eq!(marker(restored, Db::AgentHistoryReadModel).as_deref(), Some("read-since"));
    for added in ADDED {
        assert!(!installed.previous_files.join(added).exists(), "{added} stayed where it was");
    }
}

#[test]
fn a_file_pathkeep_0_5_exported_keeps_local_conversations() {
    let (_source_root, source) = released_archive();
    let files_dir = tempfile::tempdir().unwrap();
    let bundle = files_dir.path().join("from-0.5.pathkeep");
    {
        // The file as 0.5 writes it: format 2, its five stores.
        let released = Vault::open_staged(source.clone(), released_config(), None).unwrap();
        crate::migration::native::export(
            &released,
            &bundle,
            "export password",
            Default::default(),
            |_| {},
        )
        .unwrap();
        released.close().unwrap();
    }
    assert_eq!(crate::migration::read_and_validate_manifest(&bundle).unwrap().format_version, 2);

    // Into an empty folder, and over an archive of this build that holds conversations.
    for populated in [false, true] {
        let target_root = tempfile::tempdir().unwrap();
        let target = project_paths_with_root(target_root.path());
        let current = populated.then(|| {
            crate::config::ensure_paths(&target).unwrap();
            crate::save_config(&crate::ControlGate::acquire(&target), &released_config()).unwrap();
            let vault = open(&target).unwrap();
            write_marker(vault.store(), Db::AgentHistoryEvidence, "the-replaced-archive");
            vault
        });
        let (_, installed) = crate::migration::native::import(
            &target,
            current.as_ref(),
            None,
            &bundle,
            &crate::ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some("export password".into()),
            },
            Default::default(),
        )
        .unwrap();
        let vault = &installed.vault;
        assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2, "populated: {populated}");
        assert_eq!(marker(vault, Db::Archive).as_deref(), Some("released-archive"));
        assert_eq!(
            marker(vault, Db::AgentHistoryEvidence).as_deref(),
            populated.then_some("the-replaced-archive")
        );
        assert_eq!(marker(vault, Db::AgentHistoryReadModel), None, "populated: {populated}");
        assert!(read(&target).unwrap().is_none());
    }
}

// A later PathKeep can raise the revision and still let this build read and write the archive
// (its floors stay at 1). This build then writes the archive back with that stamp, byte for byte:
// writing its own would let a build between the two open an archive it should not.
#[test]
fn a_later_builds_compatible_stamp_survives_saves_mode_changes_and_restores() {
    let (_root, paths) = released_archive();
    let snapshots = tempfile::tempdir().unwrap();
    let ours_snapshot = snapshots.path().join("stamped-by-this-build");
    {
        let vault = open(&paths).unwrap();
        crate::maintenance::snapshot::snapshot(&vault, &ours_snapshot).unwrap();
        vault.close().unwrap();
    }
    let ours = r#"{"revision":1,"minReader":1,"minWriter":1}"#;
    let later = r#"{"revision":2,"minReader":1,"minWriter":1,"addedBy":"a later build"}"#;
    let text = fs::read_to_string(&paths.config_path).unwrap();
    assert_eq!(text.matches(ours).count(), 1, "{text}");
    fs::write(&paths.config_path, text.replace(ours, later)).unwrap();
    let kept = |what: &str| {
        let text = fs::read_to_string(&paths.config_path).unwrap();
        assert!(text.contains(later), "{what}: {text}");
    };

    let vault = open(&paths).unwrap();
    kept("an open");
    let mut changed = vault.config().clone();
    changed.preferred_language = crate::LanguagePreference::ZhTw;
    crate::save_config(&crate::ControlGate::acquire(&paths), &changed).unwrap();
    kept("a settings save");
    let switched = crate::maintenance::switch_mode(
        &vault,
        None,
        ArchiveMode::Encrypted,
        Some("new password"),
        Default::default(),
        |_| {},
    )
    .unwrap();
    kept("a mode change");
    let restored = crate::maintenance::snapshot::restore_snapshot(
        &switched.vault,
        &ours_snapshot,
        None,
        Default::default(),
    )
    .unwrap();
    kept("a restore of a snapshot with this build's stamp");
    assert_eq!(marker(&restored.vault, Db::Archive).as_deref(), Some("released-archive"));
}

// A power cut during the move of a made store into its place, on exFAT, can leave it under both
// names. Removing the folder it was made in would free data the store in place still uses: the
// made name goes into the quarantine instead. A hard link stands in for the two names.
#[cfg(unix)]
#[test]
fn a_store_under_two_names_after_a_cut_move_keeps_both() {
    use std::os::unix::fs::MetadataExt;
    let (_root, paths) = released_archive();
    crash_at(&paths, "promotion-stamped");
    let record = read(&paths).unwrap().expect("the crash left a record");
    let made = paths.app_root.join(&record.made_in).join(ADDED[0]);
    let place = paths.app_root.join(ADDED[0]);
    fs::hard_link(&made, &place).unwrap();

    let vault = open(&paths).unwrap();
    assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2);
    vault.close().unwrap();
    assert!(made_folders(&paths).is_empty());
    assert_eq!(fs::metadata(&place).unwrap().nlink(), 2, "the other name was deleted");
    // Only the store's other name: nothing else of the folder was moved in, whatever it holds
    // (its own lock file has the same bytes as the archive's).
    let kept = paths.quarantine_dir.join(record.made_in.file_name().unwrap());
    let kept: Vec<_> =
        files(&kept).into_keys().map(|path| path.strip_prefix(&kept).unwrap().to_owned()).collect();
    assert_eq!(kept, [Path::new("0").join(ADDED[0])]);
}

// On a file system without a journal a power cut during the rename that writes the stamp over the
// control can leave no control at all. The record keeps the control it writes: the next start
// writes it and finishes the bring-up, instead of refusing an archive that only lost its control.
#[test]
fn a_bring_up_whose_stamp_a_power_cut_took_is_finished() {
    for point in ["promotion-recorded", "promotion-stamped"] {
        let (_root, paths) = released_archive();
        crash_at(&paths, point);
        fs::remove_file(&paths.config_path).unwrap();
        assert!(
            matches!(
                crate::admission::inspect(&paths.app_root),
                Ok(crate::admission::RootState::Interrupted(_))
            ),
            "{point}: {:?}",
            crate::admission::inspect(&paths.app_root)
        );
        crate::maintenance::install::recover_before_open(&paths)
            .unwrap_or_else(|error| panic!("{point}: {error:#}"));
        let vault = open(&paths).unwrap_or_else(|error| panic!("{point}: {error:#}"));
        assert_eq!(vault.layout(), ArchiveLayout::AgentHistoryV2, "{point}");
        assert!(crate::load_config(&paths).unwrap().initialized, "{point}: the archive's own");
        assert_eq!(marker(&vault, Db::Archive).as_deref(), Some("released-archive"), "{point}");
        assert!(read(&paths).unwrap().is_none(), "{point}");
    }
}

// A copy of the folder cut short after the stamp leaves a made store shorter than it was made.
// Moved in, it would be the store the archive opens, and the record that could make it again
// would be gone. It is not moved in: the next open makes it again.
#[test]
fn a_made_store_a_copy_cut_short_is_made_again() {
    let (_root, paths) = released_archive();
    crash_at(&paths, "promotion-stamped");
    let [folder] = <[PathBuf; 1]>::try_from(made_folders(&paths)).unwrap();
    let made = folder.join(ADDED[0]);
    let bytes = fs::read(&made).unwrap();
    fs::write(&made, &bytes[..bytes.len() / 2]).unwrap();

    let vault = open(&paths).unwrap_or_else(|error| panic!("{error:#}"));
    write_marker(vault.store(), Db::AgentHistoryEvidence, "usable");
    assert_eq!(marker(&vault, Db::AgentHistoryEvidence).as_deref(), Some("usable"));
    assert!(read(&paths).unwrap().is_none());
    assert!(made_folders(&paths).is_empty());
}

// The stores the first folder lost are made again in a second folder. That folder is recorded
// before anything leaves it, and no folder is left behind once the record goes: an unrecorded one
// may hold a second name of a store in place (a cut move on exFAT), and whatever removes it later
// (the next bring-up, Free up space) would free data the archive uses.
#[test]
fn every_folder_stores_leave_is_recorded_and_none_outlives_the_record() {
    let cuts = [("promotion-store-moved", 1), ("promotion-record-removed", 1)];
    for (point, hit) in cuts {
        let (_root, paths) = released_archive();
        crash_at(&paths, "promotion-stamped");
        for folder in made_folders(&paths) {
            fs::remove_dir_all(folder).unwrap();
        }
        let fault = crate::fault_inject::FaultGuard::panic_at_hit(point, hit);
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open(&paths)));
        drop(fault);
        assert!(crashed.is_err(), "{point}: not reached");
        if let Some(record) = read(&paths).unwrap() {
            for folder in made_folders(&paths) {
                let name = folder.strip_prefix(&paths.app_root).unwrap();
                assert_eq!(&*record.made_in, name, "{point}: an unrecorded folder");
            }
        } else {
            assert!(made_folders(&paths).is_empty(), "{point}: a folder outlived its record");
        }
        drop(open(&paths).unwrap_or_else(|error| panic!("{point}: {error:#}")));
        assert!(made_folders(&paths).is_empty(), "{point}");
        assert!(read(&paths).unwrap().is_none(), "{point}");
    }
}

// The control a record writes back must be one this build stamps: a record holding anything else
// (cut short, edited, or not this build's) is not acted on, and nothing is written in the folder.
#[test]
fn a_record_whose_control_is_not_this_builds_stamp_writes_nothing() {
    let unstamped = crate::config::encode_config(&released_config(), None).unwrap();
    for control in ["not json", unstamped.as_str()] {
        let (_root, paths) = released_archive();
        crash_at(&paths, "promotion-stamped");
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(record_path(&paths)).unwrap()).unwrap();
        record["control"] = control.into();
        fs::write(record_path(&paths), serde_json::to_vec(&record).unwrap()).unwrap();
        fs::remove_file(&paths.config_path).unwrap();
        let before = files(&paths.app_root);

        assert!(
            !matches!(
                crate::admission::inspect(&paths.app_root),
                Ok(crate::admission::RootState::Interrupted(_))
            ),
            "{control}"
        );
        let error = crate::maintenance::install::recover_before_open(&paths).expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{control}: {error:#}");
        assert_eq!(files(&paths.app_root), before, "{control}: nothing was written");
    }
}

// Without a control, the folder the record names, still holding every store it names as made, is
// what says the record is this folder's. A record carried into another data folder that lost its
// control writes no control there and changes nothing.
#[test]
fn a_record_carried_into_a_folder_without_a_control_writes_nothing_there() {
    let (_first_root, first) = released_archive();
    crash_at(&first, "promotion-stamped");
    let (_root, paths) = released_archive();
    drop(open(&paths).unwrap());
    fs::remove_file(&paths.config_path).unwrap();
    fs::copy(record_path(&first), record_path(&paths)).unwrap();
    let before = files(&paths.app_root);

    assert!(!matches!(
        crate::admission::inspect(&paths.app_root),
        Ok(crate::admission::RootState::Interrupted(_))
    ));
    let error = crate::maintenance::install::recover_before_open(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-missing"), "{error:#}");
    assert_eq!(files(&paths.app_root), before);
    assert!(!paths.config_path.exists());
}
