//! Responsibilities: prove that an archive of the released layout comes up to this build's layout
//! whichever way it arrives: opened in place (the promotion, cut by a crash at each of its points),
//! restored from a snapshot PathKeep 0.5 took, or imported from a file PathKeep 0.5 exported; and
//! that a stamped archive missing a store is refused rather than refilled empty.
//! Not responsible for: what the agent-history stores hold (the agent-history crate) or the
//! install journal (`install_tests`).
//! Failure modes: a crash leaves a stamped archive whose next open creates an empty store where
//! data was; a record deletes a store an older release created after the crash; a record names a
//! file that is not a store; the record outlives a finished promotion; a later build's compatible
//! stamp is lowered by a save, a mode change or a restore; a 0.5 snapshot or file installs
//! without the new stores, or keeps the conversations of the archive it replaced.
use super::*;
use crate::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use std::collections::BTreeMap;
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
pub(in crate::maintenance) fn released_archive() -> (tempfile::TempDir, ProjectPaths) {
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

#[test]
fn every_cut_of_the_promotion_is_finished_by_the_next_open() {
    for (point, stamped, created) in [
        ("promotion-recorded", false, false),
        ("promotion-stamped", true, false),
        ("promotion-created", true, true),
    ] {
        let (_root, paths) = released_archive();
        crash_at(&paths, point);
        assert!(read(&paths).unwrap().is_some(), "{point}: the record is there");
        assert_eq!(layout(&paths) == ArchiveLayout::AgentHistoryV2, stamped, "{point}");
        for added in ADDED {
            assert_eq!(paths.app_root.join(added).is_file(), created, "{point}: {added}");
        }
        if created {
            // As if the crash had stopped the store halfway through writing the file.
            fs::write(paths.app_root.join(ADDED[0]), b"SQLite format 3\0cut short").unwrap();
        }

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

        // Brought up once: the next open rewrites nothing and records nothing.
        let control = fs::read(&paths.config_path).unwrap();
        drop(open(&paths).unwrap());
        assert_eq!(fs::read(&paths.config_path).unwrap(), control, "{point}");
        assert!(read(&paths).unwrap().is_none(), "{point}");
    }
}

// Before the stamp, PathKeep 0.5 can open the folder, and it creates a store it lacks. A record
// from a crash before the stamp may name that store; deleting what it names would delete what 0.5
// saved since.
#[test]
fn a_crash_before_the_stamp_never_deletes_what_an_older_release_saved_since() {
    let (_root, paths) = released_archive();
    remove_store(&paths, "derived/agent.db");
    crash_at(&paths, "promotion-recorded");
    let record = read(&paths).unwrap().expect("the crash left a record");
    assert!(record.creating.contains(&PathBuf::from("derived/agent.db")));
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

#[test]
fn a_stamped_archive_missing_a_store_is_refused_and_left_as_it_is() {
    for missing in ["archive/agent-history.db", "archive/history-vault.db"] {
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
        let record =
            serde_json::json!({"identity": "pathkeep-create", "version": 1, "creating": [named]});
        fs::write(record_path(&paths), serde_json::to_vec(&record).unwrap()).unwrap();
        let before = files(&paths.app_root);
        let Err(error) = open(&paths) else { panic!("opened") };
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{named}: {error:#}");
        assert_eq!(files(&paths.app_root), before, "{named}: nothing was deleted");
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
fn a_file_pathkeep_0_5_exported_imports_without_conversations() {
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
        assert_eq!(marker(vault, Db::AgentHistoryEvidence), None, "populated: {populated}");
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
