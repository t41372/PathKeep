//! Integrity failures: interrupted exports must not leave a visible partial file;
//! existing or concurrently-created outputs must survive; source/archive paths
//! and symlink aliases must not become export destinations. These tests precede
//! the implementation and exercise real filesystem operations.
//! A removed source also remains reserved under the directory's actual case and
//! Unicode name rules; spelling comparisons alone must not release that boundary.

use super::write_export;
use agent_history::Error;
use std::{cell::Cell, fs, io, path::PathBuf};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    archive: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let archive = root.join("archive");
        let source = root.join("source");
        fs::create_dir(&archive).unwrap();
        fs::create_dir(&source).unwrap();
        Self { _temp: temp, root, archive, source }
    }

    fn sources(&self) -> Vec<PathBuf> {
        vec![self.source.clone()]
    }

    fn entries(&self) -> Vec<PathBuf> {
        let mut entries: Vec<_> =
            fs::read_dir(&self.root).unwrap().map(|entry| entry.unwrap().path()).collect();
        entries.sort();
        entries
    }
}

#[test]
fn publishes_only_a_finished_export_and_preserves_its_return_value() {
    let fixture = Fixture::new();
    let target = fixture.root.join("history.jsonl");
    let count = write_export(&target, &fixture.archive, &fixture.sources(), |output| {
        output.write_all(b"one\n")?;
        assert!(!target.exists(), "final path appears only after successful encoding");
        output.write_all(b"two\n")?;
        Ok(2_u64)
    })
    .unwrap();
    assert_eq!(count, 2);
    assert_eq!(fs::read(target).unwrap(), b"one\ntwo\n");
}

#[test]
fn writer_failure_removes_the_temporary_file_and_keeps_destination_absent() {
    let fixture = Fixture::new();
    let before = fixture.entries();
    let target = fixture.root.join("history.raw");
    let result = write_export(&target, &fixture.archive, &fixture.sources(), |output| {
        output.write_all(b"private partial transcript")?;
        Err::<(), _>(Error::Io(io::Error::other("injected encoder failure")))
    });
    assert!(matches!(result, Err(Error::Io(_))));
    assert!(!target.exists());
    assert_eq!(fixture.entries(), before);
}

#[test]
fn an_existing_output_is_never_opened_for_writing() {
    let fixture = Fixture::new();
    let target = fixture.root.join("history.raw");
    fs::write(&target, b"previous export").unwrap();
    let called = Cell::new(false);
    let result = write_export(&target, &fixture.archive, &fixture.sources(), |_| {
        called.set(true);
        Ok(())
    });
    assert!(matches!(result, Err(Error::AlreadyExists(_))));
    assert!(!called.get());
    assert_eq!(fs::read(target).unwrap(), b"previous export");
}

#[test]
fn an_output_created_during_encoding_is_not_replaced() {
    let fixture = Fixture::new();
    let target = fixture.root.join("history.raw");
    let result = write_export(&target, &fixture.archive, &fixture.sources(), |output| {
        output.write_all(b"this attempted export")?;
        fs::write(&target, b"another process won")?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::AlreadyExists(_))));
    assert_eq!(fs::read(&target).unwrap(), b"another process won");
    assert_eq!(fixture.entries().len(), 3, "failed temporary export is removed");
}

#[test]
fn archive_and_source_subtrees_are_not_export_destinations() {
    let fixture = Fixture::new();
    for root in [&fixture.archive, &fixture.source] {
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();
        let called = Cell::new(false);
        let result = write_export(
            &nested.join("new-transcript.jsonl"),
            &fixture.archive,
            &fixture.sources(),
            |_| {
                called.set(true);
                Ok(())
            },
        );
        assert!(matches!(result, Err(Error::InvalidInput(_))));
        assert!(!called.get());
        assert_eq!(fs::read_dir(nested).unwrap().count(), 0);
    }
}

#[test]
fn missing_output_parents_are_not_created() {
    let fixture = Fixture::new();
    let parent = fixture.root.join("missing");
    let called = Cell::new(false);
    let result =
        write_export(&parent.join("history.jsonl"), &fixture.archive, &fixture.sources(), |_| {
            called.set(true);
            Ok(())
        });
    assert!(matches!(result, Err(Error::Io(_))));
    assert!(!called.get());
    assert!(!parent.exists());
}

#[test]
fn a_missing_former_source_root_still_cannot_be_replaced_by_an_export() {
    let fixture = Fixture::new();
    fs::remove_dir(&fixture.source).unwrap();
    let result = write_export(&fixture.source, &fixture.archive, &fixture.sources(), |_| Ok(()));
    assert!(matches!(result, Err(Error::InvalidInput(_))));
    assert!(!fixture.source.exists());
}

#[test]
fn a_removed_source_cannot_be_replaced_through_a_case_alias() {
    let fixture = Fixture::new();
    let source = fixture.root.join("HistoryRoot");
    let target = fixture.root.join("historyroot");
    fs::create_dir(&source).unwrap();
    let directory_folds_case = target.try_exists().unwrap();
    fs::remove_dir(&source).unwrap();
    if !directory_folds_case {
        return; // This case applies only when the real directory aliases these names.
    }
    let before = fixture.entries();
    let called = Cell::new(false);
    let result = write_export(&target, &fixture.archive, &[source], |_| {
        called.set(true);
        Ok(())
    });
    assert!(matches!(result, Err(Error::InvalidInput(_))));
    assert!(!called.get());
    assert!(!target.exists());
    assert_eq!(fixture.entries(), before, "name probes leave no files behind");
}

#[test]
fn case_sensitive_directories_keep_distinct_case_names_available() {
    let fixture = Fixture::new();
    let source = fixture.root.join("HistoryRoot");
    let target = fixture.root.join("historyroot");
    fs::create_dir(&source).unwrap();
    let directory_folds_case = target.try_exists().unwrap();
    fs::remove_dir(&source).unwrap();
    if directory_folds_case {
        return;
    }
    write_export(&target, &fixture.archive, &[source], |output| {
        output.write_all(b"different filesystem name")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(target).unwrap(), b"different filesystem name");
}

#[test]
fn a_removed_source_remains_reserved_under_unicode_normalization_aliases() {
    let fixture = Fixture::new();
    let source = fixture.root.join("Caf\u{e9}Root");
    let target = fixture.root.join("Cafe\u{301}Root");
    fs::create_dir(&source).unwrap();
    let directory_normalizes_names = target.try_exists().unwrap();
    fs::remove_dir(&source).unwrap();
    if !directory_normalizes_names {
        return;
    }
    let before = fixture.entries();
    let result = write_export(&target, &fixture.archive, &[source], |_| Ok(()));
    assert!(matches!(result, Err(Error::InvalidInput(_))));
    assert!(!target.exists());
    assert_eq!(fixture.entries(), before);
}

#[cfg(unix)]
#[test]
fn symlink_parent_aliases_cannot_bypass_source_containment() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let alias = fixture.root.join("source-alias");
    symlink(&fixture.source, &alias).unwrap();
    let result = write_export(
        &alias.join("new-transcript.jsonl"),
        &fixture.archive,
        &fixture.sources(),
        |_| Ok(()),
    );
    assert!(matches!(result, Err(Error::InvalidInput(_))));
    assert_eq!(fs::read_dir(&fixture.source).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn source_roots_resolve_parent_components_after_following_a_symlink() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let nested = fixture.source.join("nested");
    fs::create_dir(&nested).unwrap();
    let alias = fixture.root.join("nested-alias");
    symlink(&nested, &alias).unwrap();
    let source_with_parent = alias.join("..");
    let result = write_export(
        &fixture.source.join("history.raw"),
        &fixture.archive,
        &[source_with_parent],
        |_| Ok(()),
    );
    assert!(matches!(result, Err(Error::InvalidInput(_))));
}

#[cfg(unix)]
#[test]
fn dangling_destination_symlinks_are_not_followed_or_replaced() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let target = fixture.root.join("history.raw");
    let missing = fixture.root.join("missing-source.raw");
    symlink(&missing, &target).unwrap();
    let result = write_export(&target, &fixture.archive, &fixture.sources(), |_| Ok(()));
    assert!(matches!(result, Err(Error::AlreadyExists(_))));
    assert!(fs::symlink_metadata(&target).unwrap().file_type().is_symlink());
    assert!(!missing.exists());
}

#[cfg(unix)]
#[test]
fn exported_transcripts_are_owner_read_write_only() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let target = fixture.root.join("history.raw");
    write_export(&target, &fixture.archive, &fixture.sources(), |output| {
        output.write_all(b"private transcript")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::metadata(target).unwrap().permissions().mode() & 0o777, 0o600);
}
