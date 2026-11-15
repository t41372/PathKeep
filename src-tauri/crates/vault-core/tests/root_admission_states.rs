//! Responsibilities: pin what a data root is taken for, for each combination of control file,
//! pending record and contents, and that deciding it changes nothing.
//! Not responsible for: what each entry point then does (`vault-worker/tests/root_admission`).
//!
//! Every row is written out. The expectations are not derived from the layout catalog, so a
//! catalog change that turns a first run into "recovery required", or the reverse, fails here.
use std::{fs, path::Path};
use vault_core::admission::{Interrupted, RootState, inspect, require_control};
#[path = "../../../../tests/root_admission/support/tree.rs"]
mod tree;

const LEGACY: &str = include_str!("../../../../tests/root_admission/fixtures/legacy.json");
const FUTURE: &str = include_str!("../../../../tests/root_admission/fixtures/reader-floor.json");

#[derive(Clone, Copy, Debug)]
enum Control {
    Absent,
    Legacy,
    Future,
    Garbage,
    /// Valid JSON whose settings no PathKeep wrote.
    BadSettings,
    /// A directory where the file should be: a read failure that is not "not found".
    Unreadable,
}

#[derive(Clone, Copy, Debug)]
enum Record {
    None,
    Wipe,
    /// A wipe marker that does not decode. The wipe writes it whole or not at all.
    WipeGarbage,
    Install,
    /// The journal of a PathKeep 0.3 import, which has no `stage`.
    OldImport,
    /// The journal of a PathKeep 0.3 restore.
    OldRestore,
    /// An install journal that is not JSON.
    InstallGarbage,
    /// The records this build writes. The control outlives both, so without one they are damage.
    WipeV2,
    InstallV2,
}

#[derive(Clone, Copy, Debug)]
enum Contents {
    Empty,
    /// Everything that may lie in a root with no archive in it.
    Inert,
    Store,
    /// A file under `archive/` this build has no name for.
    UnknownInArchive,
    PreviousArchive,
    AppLock,
    Assistant,
    /// An archive still in PathKeep 0.3's format.
    OldStore,
    Snapshot,
    Quarantined,
    SavedText,
    ConversionStage,
    /// The staging folder of an operation that never finished.
    AbandonedStage,
}

#[derive(Debug, PartialEq)]
enum Is {
    Admitted,
    Uninitialized,
    InterruptedWipe,
    InterruptedInstall,
    InterruptedOldFormat,
    Refused(&'static str),
}

fn write(root: &Path, relative: &str, bytes: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn build(root: &Path, control: Control, record: Record, contents: Contents) {
    fs::create_dir_all(root).unwrap();
    match control {
        Control::Absent => {}
        Control::Legacy => write(root, "config.json", LEGACY),
        Control::Future => write(root, "config.json", FUTURE),
        Control::Garbage => write(root, "config.json", "{\"archiveMode\":"),
        Control::BadSettings => write(root, "config.json", r#"{"initialized":"yes"}"#),
        Control::Unreadable => fs::create_dir(root.join("config.json")).unwrap(),
    }
    match record {
        Record::None => {}
        Record::Wipe => {
            write(root, ".pk-wipe-in-progress.json", r#"{"providerIds":[],"schedule":null}"#)
        }
        Record::WipeGarbage => write(root, ".pk-wipe-in-progress.json", "{\"providerIds\":"),
        Record::Install => write(
            root,
            "archive/.pk-import-journal.json",
            r#"{"stage":"staging/maintenance-x","backup":"maintenance.bak-x","entries":[],"keep_previous":false,"committed":false}"#,
        ),
        Record::OldImport => {
            write(root, "archive/.pk-import-journal.json", r#"{"phase":"swapping"}"#)
        }
        Record::OldRestore => write(root, "archive/.pk-restore-journal.json", "{}"),
        Record::InstallGarbage => write(root, "archive/.pk-import-journal.json", "not json"),
        Record::WipeV2 => write(
            root,
            ".pk-wipe-v2.json",
            r#"{"identity":"pathkeep-wipe","version":1,"providerIds":[],"schedule":null}"#,
        ),
        Record::InstallV2 => write(
            root,
            "archive/.pk-install-v2.json",
            r#"{"identity":"pathkeep-install","version":1,"stage":"staging/maintenance-x","backup":"maintenance.bak-x","entries":[],"keep_previous":false,"committed":false}"#,
        ),
    }
    match contents {
        Contents::Empty => {}
        Contents::Inert => {
            // Left by a finished wipe.
            write(root, "archive/.pk-owner.lock", "4242");
            write(root, "archive/.pk-archive-write.lock", "");
            write(root, "logs/rust.log", "a previous run");
            write(root, "logs/rust.log.1", "rotated");
            write(root, "logs/owner-startup.log", "a previous start");
            write(root, "diagnostics/crash-reports/rust-panic-latest.json", "{}");
            write(root, "ipc/owner.sock", "");
            // Written by launchd for a schedule that outlived the archive.
            write(root, "schedule/logs/worker.stderr.log", "archive has not been initialized");
            write(root, "schedule/attempts/attempt-1.json", "{}");
            // The record the scheduler writes when "delete all data" removes the schedule.
            write(root, "audit/scheduler/remove-2026-11-14.json", "{}");
            // A journal whose durable write a crash cut short, during a first import.
            write(root, "archive/.pk-durable-9f", "{");
            write(root, "staging/maintenance-1a2b/archive/history-vault.db", "half an import");
            // Re-downloadable or regenerated, and nothing an archive is recovered from.
            write(root, "models/static-embedding/model.bin", "weights");
            write(root, "integrations/pathkeep-mcp.json", "{}");
            write(root, "exports/history.csv", "an export");
            // An interrupted atomic write, before any control existed.
            write(root, ".pk-durable-1a2b", "{");
            // Not PathKeep's, or the user's own to keep.
            write(root, ".DS_Store", "finder");
            // The file manager writes these into whichever folder a person opens.
            write(root, "archive/.DS_Store", "finder");
            write(root, "derived/._agent.db", "an AppleDouble companion");
            write(root, "sidecars/Thumbs.db", "explorer");
            write(root, "raw-snapshots/desktop.ini", "explorer");
            write(root, "my-export.pkexport", "an export saved into the data folder");
            write(root, "notes/readme.txt", "another program's folder");
            for folder in [
                "derived/vectors",
                "audit/manifests",
                "exports",
                "raw-snapshots",
                "staging",
                "quarantine",
                "sidecars/intelligence-blobs",
                "sidecars/semantic-index",
                "models",
            ] {
                fs::create_dir_all(root.join(folder)).unwrap();
            }
        }
        Contents::Store => write(root, "archive/history-vault.db", "an archive"),
        Contents::UnknownInArchive => write(root, "archive/agent-history.db", "a newer store"),
        Contents::PreviousArchive => write(
            root,
            "maintenance.bak-x/archive/history-vault.db",
            "the archive before a restore",
        ),
        Contents::AppLock => write(root, "app-lock-passcode.json", "{}"),
        Contents::Assistant => write(root, "derived/agent.db-wal", "ask conversations"),
        Contents::OldStore => write(root, "archive/history-vault.sqlite", "an old archive"),
        Contents::Snapshot => {
            write(root, "raw-snapshots/maintenance/2026-11-14/archive/history-vault.db", "a copy")
        }
        Contents::Quarantined => write(root, "quarantine/history-vault.db", "set aside"),
        Contents::SavedText => write(root, "sidecars/intelligence-blobs/ab/cd.txt", "page text"),
        Contents::ConversionStage => {
            write(root, "staging/sqlcipher-conversion/conversion.json", "{}")
        }
        Contents::AbandonedStage => {
            write(root, "staging/maintenance-1a2b/archive/history-vault.db", "half an import");
            write(root, "staging/maintenance-1a2b/sidecars/intelligence-blobs/a/b.txt", "text");
            write(root, "archive/.pk-owner.lock", "4242");
        }
    }
}

fn observed(root: &Path) -> Is {
    match inspect(root) {
        Ok(RootState::Admitted(_)) => Is::Admitted,
        Ok(RootState::Uninitialized) => Is::Uninitialized,
        Ok(RootState::Interrupted(Interrupted::Wipe)) => Is::InterruptedWipe,
        Ok(RootState::Interrupted(Interrupted::Install)) => Is::InterruptedInstall,
        Ok(RootState::Interrupted(Interrupted::OldFormat)) => Is::InterruptedOldFormat,
        Err(refusal) => Is::Refused(refusal.reason.code()),
    }
}

#[test]
fn every_combination_is_classified_as_written_and_nothing_is_touched() {
    use Contents as C;
    use Control::*;
    use Record as R;
    const MISSING: Is = Is::Refused("archive-control-missing");
    let rows = [
        // No control: only what is in the root can say whether an archive was here.
        (Absent, R::None, C::Empty, Is::Uninitialized),
        (Absent, R::None, C::Inert, Is::Uninitialized),
        (Absent, R::None, C::Store, MISSING),
        (Absent, R::None, C::UnknownInArchive, MISSING),
        (Absent, R::None, C::PreviousArchive, MISSING),
        (Absent, R::None, C::AppLock, MISSING),
        (Absent, R::None, C::Assistant, MISSING),
        (Absent, R::None, C::Snapshot, MISSING),
        (Absent, R::None, C::Quarantined, MISSING),
        (Absent, R::None, C::SavedText, MISSING),
        (Absent, R::None, C::ConversionStage, MISSING),
        // An import into a new data folder that a crash cut short. The next open deletes it.
        (Absent, R::None, C::AbandonedStage, Is::Uninitialized),
        // A wipe removes the control first; an install moves it aside. Both are finished.
        (Absent, R::Wipe, C::Empty, Is::InterruptedWipe),
        (Absent, R::Wipe, C::Inert, Is::InterruptedWipe),
        (Absent, R::Wipe, C::Store, Is::InterruptedWipe),
        (Absent, R::Wipe, C::UnknownInArchive, Is::InterruptedWipe),
        (Absent, R::Install, C::Empty, Is::InterruptedInstall),
        (Absent, R::Install, C::Store, Is::InterruptedInstall),
        (Absent, R::Install, C::PreviousArchive, Is::InterruptedInstall),
        // PathKeep 0.3's own journals; the conversion's launch recovery finishes these.
        (Absent, R::OldImport, C::Empty, Is::InterruptedOldFormat),
        (Absent, R::OldImport, C::OldStore, Is::InterruptedOldFormat),
        (Absent, R::OldRestore, C::OldStore, Is::InterruptedOldFormat),
        // Next to a native archive those journals are stale and explain nothing.
        (Absent, R::OldImport, C::Store, MISSING),
        (Absent, R::OldRestore, C::Store, MISSING),
        (Absent, R::None, C::OldStore, MISSING),
        // A record nothing can vouch for is not acted on.
        (Absent, R::WipeGarbage, C::Empty, MISSING),
        (Absent, R::WipeGarbage, C::Store, MISSING),
        (Absent, R::InstallGarbage, C::Empty, MISSING),
        (Absent, R::InstallGarbage, C::Store, MISSING),
        // This build's own records never outlive the control.
        (Absent, R::WipeV2, C::Empty, MISSING),
        (Absent, R::WipeV2, C::Store, MISSING),
        (Absent, R::InstallV2, C::Empty, MISSING),
        (Absent, R::InstallV2, C::Store, MISSING),
        (Legacy, R::WipeV2, C::Store, Is::Admitted),
        (Legacy, R::InstallV2, C::Store, Is::Admitted),
        // A control this build may use: records are the start sequence's business, not a class.
        (Legacy, R::None, C::Empty, Is::Admitted),
        (Legacy, R::None, C::Store, Is::Admitted),
        (Legacy, R::None, C::UnknownInArchive, Is::Admitted),
        (Legacy, R::Wipe, C::Store, Is::Admitted),
        (Legacy, R::WipeGarbage, C::Store, Is::Admitted),
        (Legacy, R::Install, C::Store, Is::Admitted),
        // A control this build may not use wins over everything else in the root.
        (Future, R::None, C::Empty, Is::Refused("archive-reader-too-old")),
        (Future, R::None, C::Store, Is::Refused("archive-reader-too-old")),
        (Future, R::Wipe, C::Store, Is::Refused("archive-reader-too-old")),
        (Future, R::Install, C::Store, Is::Refused("archive-reader-too-old")),
        (Garbage, R::None, C::Empty, Is::Refused("archive-control-invalid")),
        (Garbage, R::None, C::Store, Is::Refused("archive-control-invalid")),
        (Garbage, R::Wipe, C::Store, Is::Refused("archive-control-invalid")),
        (BadSettings, R::None, C::Empty, Is::Refused("archive-control-invalid")),
        (BadSettings, R::None, C::Store, Is::Refused("archive-control-invalid")),
        (BadSettings, R::Install, C::Store, Is::Refused("archive-control-invalid")),
        (Unreadable, R::None, C::Empty, Is::Refused("archive-control-unreadable")),
        (Unreadable, R::None, C::Store, Is::Refused("archive-control-unreadable")),
        (Unreadable, R::Wipe, C::Store, Is::Refused("archive-control-unreadable")),
    ];
    let mut failures = Vec::new();
    for (control, record, contents, expected) in rows {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        build(&root, control, record, contents);
        let before = tree::census(&root, vault_core::utils::sha256_hex);
        let actual = observed(&root);
        // A reader of the config gets the control, nothing, or an error. An interrupted wipe or
        // install reads as nothing, as in every release: the recoveries are built on that.
        let reader = match require_control(&root) {
            Ok(Some(_)) => Is::Admitted,
            Ok(None) => Is::Uninitialized,
            Err(refusal) => Is::Refused(refusal.reason.code()),
        };
        let expected_reader = match &expected {
            Is::InterruptedWipe | Is::InterruptedInstall | Is::InterruptedOldFormat => {
                Is::Uninitialized
            }
            Is::Admitted => Is::Admitted,
            Is::Uninitialized => Is::Uninitialized,
            Is::Refused(code) => Is::Refused(code),
        };
        let after = tree::census(&root, vault_core::utils::sha256_hex);
        if actual != expected || reader != expected_reader || before != after {
            failures.push(format!(
                "{control:?} + {record:?} + {contents:?}: expected {expected:?}, inspect said {actual:?}, a reader got {reader:?}, changed={}",
                before != after
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_root_that_does_not_exist_is_a_first_run_and_is_not_created() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("never-created");
    assert_eq!(observed(&root), Is::Uninitialized);
    assert!(!root.exists());
    let config = vault_core::load_config(&vault_core::project_paths_with_root(&root)).unwrap();
    assert!(!config.initialized);
    assert!(!root.exists());
}

#[test]
fn a_setting_this_build_cannot_decode_is_a_damaged_control_not_a_default() {
    for control in [
        r#"{"initialized":true,"archiveMode":"Banana"}"#,
        r#"{"initialized":"yes"}"#,
        r#"{"initialized":true,"archiveMode":null}"#,
        r#"{"initialized":true,"archiveMode":{"formatVersion":2,"mode":"Banana","compatibility":{"revision":1,"minReader":1,"minWriter":1}}}"#,
        r#"{"initialized":true,"archiveMode":{"formatVersion":1,"mode":"Plaintext","compatibility":{"revision":1,"minReader":1,"minWriter":1}}}"#,
        r#"{"initialized":true,"archiveMode":{"formatVersion":2,"mode":"Plaintext","extra":1,"compatibility":{"revision":1,"minReader":1,"minWriter":1}}}"#,
    ] {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("config.json"), control).unwrap();
        let error = vault_core::load_config(&vault_core::project_paths_with_root(temp.path()))
            .map(|config| format!("loaded, initialized={}", config.initialized))
            .unwrap_err();
        let message = format!("{error:#}");
        // The stamped rows are refused for their floors before the mode is looked at: level 0
        // admits no stamp. Either way nothing is defaulted.
        assert!(
            message.starts_with("archive-control-invalid")
                || message.starts_with("archive-reader-too-old"),
            "{control}: {message}"
        );
    }
}

#[test]
fn an_array_where_an_object_belongs_is_not_read_as_one() {
    // serde fills a struct from an array by position. None of these is something PathKeep wrote.
    for (control, code) in [
        (r#"[true]"#, "archive-control-invalid"),
        (
            r#"{"initialized":true,"archiveMode":{"formatVersion":2,"mode":"Plaintext","compatibility":[1,1,1]}}"#,
            "archive-compatibility-invalid",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("config.json"), control).unwrap();
        assert_eq!(observed(temp.path()), Is::Refused(code), "{control}");
        assert!(vault_core::config::decode_document(control.as_bytes()).is_err(), "{control}");
    }
}

// The lifecycle tests keep, for every operation, a listing of what the data folder held
// afterwards. They were written down from disk before this rule existed and by someone else, so
// they judge it from outside: without its `config.json`, the folder a wipe leaves is a first
// run, and every folder that holds an archive is one whose control is missing.
#[test]
fn the_folders_the_lifecycle_tests_recorded_are_classified_as_what_they_are() {
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vault-worker/tests/vault_lifecycle/expected");
    let mut seen = 0;
    for entry in fs::read_dir(expected).unwrap() {
        let path = entry.unwrap().path();
        let listing: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let temp = tempfile::tempdir().unwrap();
        for file in listing["files"].as_array().unwrap() {
            // `<maintenance.bak-1>` stands for a name that differs from run to run.
            let relative = file.as_str().unwrap().replace(['<', '>'], "");
            if relative != "config.json" {
                write(temp.path(), &relative, "");
            }
        }
        let wiped = path.file_name().unwrap() == "wiped.json";
        assert_eq!(
            observed(temp.path()),
            if wiped { Is::Uninitialized } else { Is::Refused("archive-control-missing") },
            "{}",
            path.display()
        );
        seen += 1;
    }
    assert!(seen >= 20, "the recorded listings were not found");
}
