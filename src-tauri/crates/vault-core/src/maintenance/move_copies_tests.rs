//! Responsibilities: prove that safety copies stay on the computer that made them through a move
//! (the user's decision, 2026-11-15): a move file leaves them out, an import keeps this computer's
//! where they are and listed, the browser-file copies in `raw-snapshots/` still travel and are
//! replaced, and the copies a file of PathKeep 0.5 or earlier brings are added beside this
//! computer's, each once. Also when the import is cut short, and when it is finished after a crash.
//! Not responsible for: the archive an import replaces becoming a copy (`safety_copy_tests`), the
//! journal's own recovery rules (`install_tests`), listing old-format copies (the converter's
//! `list_legacy_snapshots`).
//! Failure modes: a move file carries every copy, each the size of the archive; an import moves
//! this computer's copies into the "before import" copy, where the restore list does not show them
//! and freeing that copy deletes them; a copy the file brings replaces a different one of the same
//! name, or is taken for it and dropped; the same copy listed twice, or added again by a second
//! import of the same file; an old-format copy separated from the file SQLite keeps beside it,
//! taken for another because SQLite rebuilt its index, taken for the same one although its log
//! differs, or put beside a log left under the name it gets; a browser-file copy kept as if it were
//! a safety copy, or left out of the move; an import cut short leaves the file's copies behind or
//! has moved this computer's.
use super::creation::tests::{marker, open, released_archive, released_config, write_marker};
use super::snapshot::{list_snapshots, snapshot};
use crate::{ApplyImportOptions, ProjectPaths, Vault};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use vault_store::Db;

const PASSWORD: &str = "export password";
const ARTIFACTS: &str = "archive/portable-artifacts.pkf";

/// An archive with a marker naming `who`, and in `raw-snapshots/` one of each kind of thing a
/// computer keeps there, all named after `who`: a safety copy as PathKeep makes one, an old-format
/// copy PathKeep 0.3 left (with the file SQLite keeps beside it), and a browser's history file
/// copied by a backup.
fn computer(who: &'static str) -> (tempfile::TempDir, ProjectPaths, Vault) {
    let (root, paths) = released_archive();
    let vault = open(&paths).unwrap();
    write_marker(vault.store(), Db::Archive, who);
    native_copy(&vault, &format!("rekey-{who}"));
    old_format_copy(&paths, who, who);
    browser_copy(&paths, "chrome%3ADefault", who);
    (root, paths, vault)
}

fn native_copy(vault: &Vault, name: &str) -> PathBuf {
    let copy = vault.paths().raw_snapshots_dir.join("maintenance").join(name);
    snapshot(vault, &copy).unwrap();
    copy
}

fn old_format_copy(paths: &ProjectPaths, name: &str, body: &str) -> PathBuf {
    let bucket = paths.raw_snapshots_dir.join("periodic");
    fs::create_dir_all(&bucket).unwrap();
    let copy = bucket.join(format!("{name}.sqlite"));
    fs::write(&copy, format!("SQLite format 3\0{body}")).unwrap();
    fs::write(bucket.join(format!("{name}.sqlite-wal")), format!("wal of {body}")).unwrap();
    copy
}

fn browser_copy(paths: &ProjectPaths, profile: &str, run: &str) -> PathBuf {
    let dir = paths.raw_snapshots_dir.join(profile).join(run);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("History"), format!("{profile} history at {run}")).unwrap();
    dir
}

fn identity(path: &Path) -> Option<String> {
    crate::content_identity::of(path).unwrap()
}

fn export(vault: &Vault, files: &Path) -> PathBuf {
    let bundle = files.join("moved.pathkeep");
    crate::migration::native::export(vault, &bundle, PASSWORD, Default::default(), |_| {}).unwrap();
    bundle
}

fn import(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    bundle: &Path,
) -> crate::maintenance::InstalledArchive {
    let options =
        ApplyImportOptions { confirm_overwrite: true, source_archive_key: Some(PASSWORD.into()) };
    crate::migration::native::import(paths, current, None, bundle, &options, Default::default())
        .unwrap()
        .1
}

/// The names of the native copies the restore list offers, sorted.
fn listed(paths: &ProjectPaths) -> Vec<String> {
    let mut names: Vec<String> = list_snapshots(paths)
        .unwrap()
        .into_iter()
        .map(|copy| Path::new(&copy.path).file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The one copy the import made of the archive it replaced.
fn before_import(paths: &ProjectPaths) -> PathBuf {
    let copies: Vec<PathBuf> = list_snapshots(paths)
        .unwrap()
        .into_iter()
        .filter(|copy| copy.source_op == "import")
        .map(|copy| PathBuf::from(copy.path))
        .collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    copies.into_iter().next().unwrap()
}

/// Nothing in the copy an import made is a safety copy: no folder of native copies, no old-format
/// copy.
fn assert_holds_no_copy(copy: &Path) {
    let raw = copy.join("raw-snapshots");
    assert!(!raw.join("maintenance").exists(), "{} holds safety copies", copy.display());
    assert!(!raw.join("periodic").exists(), "{} holds old-format copies", copy.display());
}

fn bundle_key(bundle: &Path) -> vault_crypto::DataKey {
    let mut zip = zip::ZipArchive::new(fs::File::open(bundle).unwrap()).unwrap();
    let mut entry = zip.by_name("archive/keys.envelope").unwrap();
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).unwrap();
    crate::vault::password::unwrap(&bytes, PASSWORD).unwrap()
}

/// The names of the files the move file carries in its artifact entry.
fn carried(bundle: &Path) -> Vec<String> {
    let scratch = tempfile::tempdir().unwrap();
    let sealed = scratch.path().join(ARTIFACTS);
    fs::create_dir_all(sealed.parent().unwrap()).unwrap();
    {
        let mut zip = zip::ZipArchive::new(fs::File::open(bundle).unwrap()).unwrap();
        let mut entry = zip.by_name(ARTIFACTS).unwrap();
        std::io::copy(&mut entry, &mut fs::File::create(&sealed).unwrap()).unwrap();
    }
    vault_crypto::read_artifact(scratch.path(), ARTIFACTS, &bundle_key(bundle), |input| {
        let mut tar = tar::Archive::new(input);
        let mut names = Vec::new();
        for entry in tar.entries()? {
            let entry = entry?;
            if entry.header().entry_type().is_file() {
                names.push(entry.path()?.to_string_lossy().into_owned());
            }
        }
        Ok(names)
    })
    .unwrap()
}

/// `bundle` as PathKeep 0.5 and earlier wrote it: the artifact entry holds every file under
/// `audit/`, `raw-snapshots/` and `sidecars/` of `source`, safety copies included.
fn as_pathkeep_0_5_packs_it(bundle: &Path, source: &ProjectPaths) -> PathBuf {
    let key = bundle_key(bundle);
    let scratch = tempfile::tempdir().unwrap();
    fs::create_dir_all(scratch.path().join(ARTIFACTS).parent().unwrap()).unwrap();
    vault_crypto::write_artifact(scratch.path(), ARTIFACTS, &key, |output| {
        let mut tar = tar::Builder::new(output);
        let origin = serde_json::to_vec(
            &serde_json::json!({ "sourceRoot": source.app_root.display().to_string() }),
        )?;
        let mut header = tar::Header::new_gnu();
        header.set_size(origin.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        tar.append_data(&mut header, "pathkeep-origin.json", origin.as_slice())?;
        for root in ["audit", "raw-snapshots", "sidecars"] {
            let dir = source.app_root.join(root);
            if !dir.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(&dir) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let name = entry
                        .path()
                        .strip_prefix(&source.app_root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    tar.append_path_with_name(entry.path(), name)?;
                }
            }
        }
        tar.finish()
    })
    .unwrap();
    let artifacts = fs::read(scratch.path().join(ARTIFACTS)).unwrap();
    let mut original = zip::ZipArchive::new(fs::File::open(bundle).unwrap()).unwrap();
    let mut manifest: serde_json::Value = {
        let mut bytes = Vec::new();
        original.by_name("pathkeep-export-manifest.json").unwrap().read_to_end(&mut bytes).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    };
    for file in manifest["files"].as_array_mut().unwrap() {
        if file["path"] == ARTIFACTS {
            file["sha256"] = crate::utils::sha256_hex(&artifacts).into();
            file["sizeBytes"] = artifacts.len().into();
        }
    }
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let older = bundle.with_file_name("as-0.5-wrote-it.pathkeep");
    let mut output = zip::ZipWriter::new(fs::File::create(&older).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    for index in 0..original.len() {
        let mut entry = original.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        output.start_file(name.as_str(), options).unwrap();
        match name.as_str() {
            ARTIFACTS => output.write_all(&artifacts).unwrap(),
            "pathkeep-export-manifest.json" => output.write_all(&manifest).unwrap(),
            "pathkeep-export-manifest.sha256" => {
                output.write_all(crate::utils::sha256_hex(&manifest).as_bytes()).unwrap()
            }
            _ => {
                std::io::copy(&mut entry, &mut output).unwrap();
            }
        }
    }
    output.finish().unwrap();
    older
}

/// The names in `raw-snapshots/<home>/`, without the copies the imports made of the archives they
/// replaced, sorted.
fn held(paths: &ProjectPaths, home: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(paths.raw_snapshots_dir.join(home))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with("before-"))
        .collect();
    names.sort();
    names
}

/// Every file in `dir` with what it holds, sorted by name.
fn contents(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name().to_string_lossy().into_owned(), fs::read(entry.path()).unwrap())
        })
        .collect();
    files.sort();
    files
}

pub(super) fn copy_tree(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let target = to.join(entry.path().strip_prefix(from).unwrap());
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).unwrap();
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn a_move_file_leaves_the_safety_copies_on_the_computer_that_made_them() {
    let (_root, _paths, vault) = computer("other");
    let files = tempfile::tempdir().unwrap();
    let bundle = export(&vault, files.path());
    vault.close().unwrap();
    let carried = carried(&bundle);
    assert!(
        carried.iter().any(|name| name == "raw-snapshots/chrome%3ADefault/other/History"),
        "the browser's history file travels: {carried:?}"
    );
    let copies: Vec<_> = carried
        .iter()
        .filter(|name| {
            name.starts_with("raw-snapshots/maintenance/")
                || name.starts_with("raw-snapshots/periodic/")
        })
        .collect();
    assert!(copies.is_empty(), "safety copies in the move file: {copies:?}");
}

#[test]
fn an_import_keeps_this_computers_safety_copies_where_they_are() {
    let files = tempfile::tempdir().unwrap();
    let (_source_root, _source, vault) = computer("other");
    let bundle = export(&vault, files.path());
    vault.close().unwrap();

    let (_root, paths, live) = computer("here");
    browser_copy(&paths, "firefox%3Aabc", "here-only");
    let raw = paths.raw_snapshots_dir.clone();
    let kept: Vec<(PathBuf, Option<String>)> = [
        raw.join("maintenance/rekey-here"),
        raw.join("periodic/here.sqlite"),
        raw.join("periodic/here.sqlite-wal"),
    ]
    .into_iter()
    .map(|path| {
        let held = identity(&path);
        assert!(held.is_some(), "{}", path.display());
        (path, held)
    })
    .collect();
    let imported = import(&paths, Some(&live), &bundle);
    assert_eq!(marker(&imported.vault, Db::Archive).as_deref(), Some("other"));
    imported.vault.close().unwrap();

    for (path, held) in kept {
        assert_eq!(identity(&path), held, "{} moved or changed", path.display());
    }
    let listed = listed(&paths);
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed.contains(&"rekey-here".to_owned()), "{listed:?}");
    // The browser-file copies are the file's; this computer's went with the archive they belong to.
    assert!(raw.join("chrome%3ADefault/other/History").is_file());
    assert!(!raw.join("chrome%3ADefault/here").exists());
    assert!(!raw.join("firefox%3Aabc").exists());
    let replaced = before_import(&paths);
    assert!(replaced.join("raw-snapshots/chrome%3ADefault/here/History").is_file());
    assert!(replaced.join("raw-snapshots/firefox%3Aabc/here-only/History").is_file());
    assert_holds_no_copy(&replaced);
}

#[test]
fn the_copies_a_file_of_pathkeep_0_5_brings_are_added_once() {
    let files = tempfile::tempdir().unwrap();
    let (_source_root, source, vault) = computer("other");
    let only_there = native_copy(&vault, "rekey-only-there");
    let bundle = export(&vault, files.path());
    vault.close().unwrap();
    let older = as_pathkeep_0_5_packs_it(&bundle, &source);
    assert!(carried(&older).iter().any(|name| name.starts_with("raw-snapshots/maintenance/")));

    let (_root, paths, live) = computer("here");
    let raw = paths.raw_snapshots_dir.clone();
    // A copy this computer already has: the file was exported from an archive this one came from.
    copy_tree(
        &source.raw_snapshots_dir.join("maintenance/rekey-other"),
        &raw.join("maintenance/rekey-other"),
    );
    // A different old-format copy under the name of the one the file brings.
    old_format_copy(&paths, "other", "a different copy");
    let imported = import(&paths, Some(&live), &older);
    assert_eq!(marker(&imported.vault, Db::Archive).as_deref(), Some("other"));
    imported.vault.close().unwrap();

    let listed = listed(&paths);
    assert_eq!(listed.iter().filter(|name| *name == "rekey-other").count(), 1, "{listed:?}");
    assert!(listed.contains(&"rekey-here".to_owned()), "{listed:?}");
    assert!(listed.contains(&"rekey-only-there".to_owned()), "{listed:?}");
    assert_eq!(listed.len(), 4, "here, other, only there, and the replaced archive: {listed:?}");
    assert_eq!(identity(&raw.join("maintenance/rekey-only-there")), identity(&only_there));

    // The old-format copy of the same name: this computer's stays as it was, the file's is added
    // beside it under another name, with the file SQLite keeps beside it.
    let bucket = raw.join("periodic");
    assert_eq!(
        fs::read(bucket.join("other.sqlite")).unwrap(),
        b"SQLite format 3\0a different copy"
    );
    assert_eq!(fs::read(bucket.join("here.sqlite")).unwrap(), b"SQLite format 3\0here");
    let added: Vec<PathBuf> = fs::read_dir(&bucket)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sqlite"))
        .filter(|path| fs::read(path).unwrap() == b"SQLite format 3\0other")
        .collect();
    assert_eq!(added.len(), 1, "{added:?}");
    let mut wal = added[0].clone().into_os_string();
    wal.push("-wal");
    assert_eq!(fs::read(&wal).unwrap(), b"wal of other");
    assert_eq!(
        fs::read_dir(&bucket).unwrap().count(),
        6,
        "here, mine of the same name, the file's"
    );
    assert_holds_no_copy(&before_import(&paths));
}

#[test]
fn an_import_cut_short_or_finished_after_a_crash_leaves_every_copy_where_it_belongs() {
    let files = tempfile::tempdir().unwrap();
    let (_source_root, source, vault) = computer("other");
    native_copy(&vault, "rekey-only-there");
    let bundle = export(&vault, files.path());
    vault.close().unwrap();
    let older = as_pathkeep_0_5_packs_it(&bundle, &source);
    let crash = |paths: &ProjectPaths, live: Vault, point: &str, hit: u32| {
        let fault = crate::fault_inject::FaultGuard::panic_at_hit(point, hit);
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            import(paths, Some(&live), &older)
        }));
        drop(fault);
        assert!(crashed.is_err(), "the import went past {point} {hit}");
    };

    // How many files the import puts in place, from its record where it first stops.
    let (_probe_root, probe, live) = computer("here");
    crash(&probe, live, "native-install-preserved", 1);
    let journal = vault_layout::spec(vault_layout::ArtifactId::InstallJournalV2).path.relative();
    let journal: serde_json::Value =
        serde_json::from_slice(&fs::read(probe.app_root.join(journal)).unwrap()).unwrap();
    let entries = journal["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry["relative"] == "raw-snapshots/maintenance/rekey-only-there"),
        "the copy only the file has is put in place on its own: {journal}"
    );
    let staged = entries.iter().filter(|entry| entry["staged"] == true).count() as u32;

    // Stopped after the last file went in, before the import was published: all of it goes back.
    // Stopped once it was published: the recovery finishes it.
    for (point, hit) in [("native-install-installed", staged), ("native-install-published", 1)] {
        let (_root, paths, live) = computer("here");
        let raw = paths.raw_snapshots_dir.clone();
        let before = identity(&raw);
        crash(&paths, live, point, hit);
        crate::recover_archive_on_launch(&paths, &released_config()).unwrap();
        if point == "native-install-installed" {
            let reopened = open(&paths).unwrap();
            assert_eq!(marker(&reopened, Db::Archive).as_deref(), Some("here"));
            assert_eq!(identity(&raw), before, "raw-snapshots is not as it was");
            reopened.close().unwrap();
        } else {
            // The imported archive, under the file's password.
            let config = crate::load_config(&paths).unwrap();
            let (reopened, _) =
                Vault::open_password(paths.clone(), config, PASSWORD, None).unwrap();
            assert_eq!(marker(&reopened, Db::Archive).as_deref(), Some("other"));
            reopened.close().unwrap();
            let listed = listed(&paths);
            assert!(listed.contains(&"rekey-here".to_owned()), "{listed:?}");
            assert!(listed.contains(&"rekey-only-there".to_owned()), "{listed:?}");
            assert_holds_no_copy(&before_import(&paths));
        }
    }
}

// An earlier import of the same file added its copies under the names it found free
// (`rekey-other-2`, `other-2.sqlite`), a different copy here having their own. Imported again, the
// file brings each under its own name again, where that different copy still is; the one under the
// name the earlier import gave it is the same copy, and nothing is added twice.
#[test]
fn the_same_file_imported_again_adds_none_of_its_copies_twice() {
    let files = tempfile::tempdir().unwrap();
    let (_source_root, source, vault) = computer("other");
    let bundle = export(&vault, files.path());
    vault.close().unwrap();
    let older = as_pathkeep_0_5_packs_it(&bundle, &source);

    let (_root, paths, live) = computer("here");
    // Different copies under the names of the file's.
    native_copy(&live, "rekey-other");
    old_format_copy(&paths, "other", "a different copy");
    let first = import(&paths, Some(&live), &older);
    let raw = paths.raw_snapshots_dir.clone();
    assert_eq!(
        identity(&raw.join("maintenance/rekey-other-2")),
        identity(&source.raw_snapshots_dir.join("maintenance/rekey-other"))
    );
    assert_eq!(fs::read(raw.join("periodic/other-2.sqlite")).unwrap(), b"SQLite format 3\0other");
    let copies = (held(&paths, "maintenance"), held(&paths, "periodic"));

    let second = import(&paths, Some(&first.vault), &older);
    assert_eq!(marker(&second.vault, Db::Archive).as_deref(), Some("other"));
    second.vault.close().unwrap();
    assert_eq!((held(&paths, "maintenance"), held(&paths, "periodic")), copies);
}

// An old-format copy is a database: its file and the files SQLite keeps beside it. The same copy
// holds the same database: the same file, and the same log or none, an empty log being none. The
// `-shm` file is an index SQLite builds again from the log when it opens the database, and opening
// it, read-only too, writes one: it says nothing about which copy this is. A name is free only when
// no file of its database is there; a log left beside it would be read into the copy put there.
#[test]
fn an_old_format_copy_is_the_same_one_when_its_database_is() {
    let write = |dir: &Path, name: &str, body: &[u8]| fs::write(dir.join(name), body).unwrap();
    let files = tempfile::tempdir().unwrap();
    let (_source_root, source, vault) = computer("other");
    let there = source.raw_snapshots_dir.join("periodic");
    write(&there, "probed.sqlite", b"SQLite format 3\0probed");
    write(&there, "indexed.sqlite", b"SQLite format 3\0indexed");
    write(&there, "indexed.sqlite-wal", b"wal of indexed");
    write(&there, "indexed.sqlite-shm", b"index there");
    write(&there, "logged.sqlite", b"SQLite format 3\0logged");
    write(&there, "orphaned.sqlite", b"SQLite format 3\0orphaned");
    let bundle = export(&vault, files.path());
    vault.close().unwrap();
    let older = as_pathkeep_0_5_packs_it(&bundle, &source);

    let (_root, paths, live) = computer("here");
    let here = paths.raw_snapshots_dir.join("periodic");
    // The same database, opened here since: an empty log and an index.
    write(&here, "probed.sqlite", b"SQLite format 3\0probed");
    write(&here, "probed.sqlite-wal", b"");
    write(&here, "probed.sqlite-shm", b"index here");
    // The same database with another index.
    write(&here, "indexed.sqlite", b"SQLite format 3\0indexed");
    write(&here, "indexed.sqlite-wal", b"wal of indexed");
    write(&here, "indexed.sqlite-shm", b"index here");
    // The same file with a log of changes the file's copy does not have: another database.
    write(&here, "logged.sqlite", b"SQLite format 3\0logged");
    write(&here, "logged.sqlite-wal", b"changes made here");
    // Another copy under the same name, and a log left where the next free name would be.
    write(&here, "orphaned.sqlite", b"SQLite format 3\0mine");
    write(&here, "orphaned-2.sqlite-wal", b"left behind");
    let mut expected = contents(&here);
    let imported = import(&paths, Some(&live), &older);
    imported.vault.close().unwrap();

    expected.extend([
        ("logged-2.sqlite".to_owned(), b"SQLite format 3\0logged".to_vec()),
        ("orphaned-3.sqlite".to_owned(), b"SQLite format 3\0orphaned".to_vec()),
        ("other.sqlite".to_owned(), b"SQLite format 3\0other".to_vec()),
        ("other.sqlite-wal".to_owned(), b"wal of other".to_vec()),
    ]);
    expected.sort();
    assert_eq!(contents(&here), expected);
}
