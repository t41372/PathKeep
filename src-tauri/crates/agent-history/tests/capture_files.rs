//! Responsibilities: specify source integrity and path authority before capture code.
//! Not responsible for archive publication, native parsing or SQLite snapshots.
//! Failure modes: same-length rewrites evade mtime checks; source replacement or
//! truncation publishes mixed bytes; symlinks/FIFOs escape or stall capture;
//! cancellation or destination failure is mislabeled as source corruption.

use std::{
    fs::{self, FileTimes, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use agent_history::{
    Error,
    capture::files::{copy_verified, ensure_source_path, inspect},
};
use sha2::{Digest, Sha256};

fn source() -> (tempfile::TempDir, PathBuf, Vec<u8>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.jsonl");
    let bytes: Vec<_> = (0..200_000).map(|index| (index % 251) as u8).collect();
    fs::write(&path, &bytes).unwrap();
    (directory, path, bytes)
}

#[test]
fn verified_copy_preserves_every_byte_without_changing_the_source() {
    let (directory, path, bytes) = source();
    let before = fs::metadata(&path).unwrap();
    let snapshot = inspect(&path, directory.path(), &|| false).unwrap();
    assert_eq!(snapshot.byte_len, bytes.len() as u64);
    assert_eq!(snapshot.digest, format!("{:x}", Sha256::digest(&bytes)));
    let mut output = Vec::new();
    copy_verified(&path, directory.path(), &snapshot, &mut output, &|| false).unwrap();
    assert_eq!(output, bytes);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before.modified().unwrap());
}

#[test]
fn same_length_rewrite_with_restored_mtime_is_not_unchanged() {
    let (directory, path, bytes) = source();
    let snapshot = inspect(&path, directory.path(), &|| false).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let mut changed = bytes;
    changed[90000] ^= 0x7f;
    fs::write(&path, &changed).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    let result = copy_verified(&path, directory.path(), &snapshot, &mut Vec::new(), &|| false);
    assert!(matches!(result, Err(Error::SourceChanged(_))));
}

struct MutatingWriter<F> {
    action: Option<F>,
    output: Vec<u8>,
}

impl<F: FnOnce()> Write for MutatingWriter<F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(bytes);
        if let Some(action) = self.action.take() {
            action();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn changed_during_copy(change: impl FnOnce(&Path)) {
    let (directory, path, _) = source();
    let snapshot = inspect(&path, directory.path(), &|| false).unwrap();
    let mut writer = MutatingWriter { action: Some(|| change(&path)), output: Vec::new() };
    let result = copy_verified(&path, directory.path(), &snapshot, &mut writer, &|| false);
    assert!(matches!(result, Err(Error::SourceChanged(_))));
}

#[test]
fn append_during_copy_cannot_publish_the_old_length_as_current() {
    changed_during_copy(|path| {
        OpenOptions::new().append(true).open(path).unwrap().write_all(b"new event\n").unwrap();
    });
}

#[test]
fn truncation_during_copy_is_detected_before_publication() {
    changed_during_copy(|path| {
        OpenOptions::new().write(true).open(path).unwrap().set_len(12).unwrap();
    });
}

#[test]
fn same_size_mutation_during_copy_does_not_produce_a_valid_mixed_snapshot() {
    changed_during_copy(|path| {
        let metadata = fs::metadata(path).unwrap();
        fs::write(path, vec![b'x'; metadata.len() as usize]).unwrap();
        OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
    });
}

#[cfg(unix)]
#[test]
fn named_file_replacement_is_detected_even_when_the_old_handle_stays_readable() {
    changed_during_copy(|path| {
        let replacement = path.with_extension("replacement");
        fs::write(&replacement, fs::read(path).unwrap()).unwrap();
        fs::rename(&replacement, path).unwrap();
    });
}

#[test]
fn source_removed_after_inspection_is_reported_as_changed() {
    let (directory, path, _) = source();
    let snapshot = inspect(&path, directory.path(), &|| false).unwrap();
    fs::remove_file(&path).unwrap();
    let result = copy_verified(&path, directory.path(), &snapshot, &mut Vec::new(), &|| false);
    assert!(matches!(result, Err(Error::SourceChanged(_))));
}

#[test]
fn cancellation_and_destination_errors_keep_their_identity() {
    let (directory, path, _) = source();
    assert!(matches!(inspect(&path, directory.path(), &|| true), Err(Error::Cancelled)));
    let snapshot = inspect(&path, directory.path(), &|| false).unwrap();
    assert!(matches!(
        copy_verified(&path, directory.path(), &snapshot, &mut Vec::new(), &|| true),
        Err(Error::Cancelled)
    ));
    struct Full;
    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::StorageFull, "destination is full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let result = copy_verified(&path, directory.path(), &snapshot, &mut Full, &|| false);
    assert!(matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::StorageFull));
}

#[test]
fn containment_rejects_parent_traversal_and_same_prefix_siblings() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("approved");
    let sibling = directory.path().join("approved-other");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&sibling).unwrap();
    fs::write(sibling.join("secret"), b"private").unwrap();
    assert!(ensure_source_path(&sibling.join("secret"), &root).is_err());
    assert!(ensure_source_path(&root.join("../approved-other/secret"), &root).is_err());
}

#[test]
fn selected_file_authorizes_only_itself() {
    let (directory, path, _) = source();
    assert_eq!(ensure_source_path(&path, &path).unwrap(), path.canonicalize().unwrap());
    assert!(ensure_source_path(directory.path(), &path).is_err());
    let sibling = directory.path().join("unselected.jsonl");
    fs::write(&sibling, b"private").unwrap();
    assert!(ensure_source_path(&sibling, &path).is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_at_root_leaf_or_descendant_directory_are_rejected() {
    use std::os::unix::fs::symlink;
    let source = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"private").unwrap();
    symlink(outside.path(), source.path().join("escape")).unwrap();
    symlink(outside.path().join("secret"), source.path().join("leaf")).unwrap();
    symlink("missing", source.path().join("dangling")).unwrap();
    assert!(ensure_source_path(&source.path().join("escape/secret"), source.path()).is_err());
    assert!(ensure_source_path(&source.path().join("leaf"), source.path()).is_err());
    assert!(ensure_source_path(&source.path().join("dangling"), source.path()).is_err());
    assert!(
        ensure_source_path(&source.path().join("escape/secret"), &source.path().join("escape"))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn ambient_ancestor_aliases_are_resolved_before_establishing_authority() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let real = directory.path().join("real");
    fs::create_dir_all(real.join("approved")).unwrap();
    fs::write(real.join("approved/session"), b"evidence").unwrap();
    let alias = directory.path().join("ambient-alias");
    symlink(&real, &alias).unwrap();
    let root = alias.join("approved");
    assert_eq!(
        ensure_source_path(&root.join("session"), &root).unwrap(),
        real.join("approved/session").canonicalize().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn fifo_is_rejected_without_opening_it_for_a_blocking_read() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pipe.jsonl");
    let name = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: name is a valid NUL-terminated path owned for the duration of mkfifo.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(matches!(inspect(&path, directory.path(), &|| false), Err(Error::InvalidInput(_))));
}
