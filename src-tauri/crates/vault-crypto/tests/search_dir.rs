//! `EncryptedDirectory` against Tantivy's directory contract and the file format's failure
//! modes (`src/search_dir/format.rs`, `file.rs`, `writer.rs`, `cache.rs`), without an index.

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::time::Duration;

use common::{ChildProcess, Rng, is_child};
use tantivy::HasLen;
use tantivy::directory::error::{DeleteError, LockError, OpenReadError, OpenWriteError};
use tantivy::directory::{Directory, Lock, TerminatingWrite, WatchCallback};
use vault_crypto::{DataKey, EncryptedDirectory, IndexFileError};
use zeroize::Zeroizing;

fn key(byte: u8) -> DataKey {
    DataKey::from_bytes(Zeroizing::new([byte; 32]))
}

fn directory(dir: &Path) -> EncryptedDirectory {
    EncryptedDirectory::open(dir, &key(1), 1024 * 1024).unwrap()
}

fn write_file(directory: &EncryptedDirectory, name: &str, data: &[u8]) {
    let mut writer = directory.open_write(Path::new(name)).unwrap();
    // Uneven pieces, so block boundaries fall inside writes.
    for piece in data.chunks(7919) {
        writer.write_all(piece).unwrap();
    }
    writer.terminate().unwrap();
}

fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng::new(seed);
    (0..len).map(|_| rng.next_u64() as u8).collect()
}

fn index_file_error(error: &(dyn std::error::Error + 'static)) -> IndexFileError {
    IndexFileError::find_in(error)
        .cloned()
        .unwrap_or_else(|| panic!("not an index file error: {error:?}"))
}

/// `tests/fixtures/search-index-v1.idx` was built by `make_search_index_vector.py` (next to it)
/// with Python's `hmac` and `cryptography`, from the format description alone: data key
/// 11 11 .. 11, salt 80 .. 9f, two blocks (64 KiB, then 3 bytes) of `i % 251`, under the name
/// `vector.idx`. If the key derivation, key id, nonce, associated data or layout change, every
/// existing index stops opening and is rebuilt; this test is what notices first.
#[test]
fn reads_a_file_built_by_an_independent_implementation() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/search-index-v1.idx");
    std::fs::copy(&fixture, dir.path().join("vector.idx")).unwrap();
    let directory = EncryptedDirectory::open(dir.path(), &key(0x11), 1024 * 1024).unwrap();
    let expected: Vec<u8> = (0..64 * 1024 + 3).map(|i| (i % 251) as u8).collect();
    assert_eq!(
        directory.open_read(Path::new("vector.idx")).unwrap().read_bytes().unwrap().as_slice(),
        &expected[..]
    );
    assert_eq!(directory.atomic_read(Path::new("vector.idx")).unwrap(), expected);
    // The same bytes under another name, or with another key, do not open.
    std::fs::copy(&fixture, dir.path().join("other.idx")).unwrap();
    assert!(directory.open_read(Path::new("other.idx")).is_err());
    let wrong = EncryptedDirectory::open(dir.path(), &key(0x12), 1024).unwrap();
    let error = wrong.open_read(Path::new("vector.idx")).unwrap_err();
    assert!(matches!(index_file_error(&error), IndexFileError::WrongKey { .. }));
}

#[test]
fn files_of_awkward_sizes_read_back_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let block = 64 * 1024;
    let sizes = [0, 1, 100, block - 1, block, block + 1, 2 * block, 3 * block + 5, 300_001];
    let mut rng = Rng::new(42);
    for (i, &size) in sizes.iter().enumerate() {
        let name = format!("file-{i}.bin");
        let data = pseudo_random_bytes(size, i as u64 + 1);
        write_file(&directory, &name, &data);
        let handle = directory.open_read(Path::new(&name)).unwrap();
        assert_eq!(handle.len(), size);
        assert_eq!(handle.read_bytes().unwrap().as_slice(), &data[..], "size {size}");
        for _ in 0..200 {
            if size == 0 {
                break;
            }
            let start = rng.below(size as u64) as usize;
            let end = start + rng.below((size - start) as u64 + 1) as usize;
            assert_eq!(
                handle.slice(start..end).read_bytes().unwrap().as_slice(),
                &data[start..end]
            );
        }
        // Every boundary between blocks, read across.
        for boundary in (block..size).step_by(block) {
            let range = boundary - 3..(boundary + 3).min(size);
            assert_eq!(handle.slice(range.clone()).read_bytes().unwrap().as_slice(), &data[range]);
        }
    }
}

#[test]
fn nothing_written_is_readable_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let text = b"PKNEEDLE readable words https://pksentinel.example/page ".repeat(5000);
    write_file(&directory, "segment.idx", &text);
    directory.atomic_write(Path::new("meta.json"), br#"{"segments":["PKNEEDLE"]}"#).unwrap();
    let found = common::find_plaintext(dir.path(), &[b"PKNEEDLE", b"pksentinel", b"segments"]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn tantivy_directory_contract() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let path = Path::new("some_path_for_test");

    // Reading a missing file says so.
    assert!(matches!(directory.open_read(path), Err(OpenReadError::FileDoesNotExist(_))));
    assert!(matches!(directory.atomic_read(path), Err(OpenReadError::FileDoesNotExist(_))));
    assert!(!directory.exists(path).unwrap());

    // open_write creates the file at once, and refuses to overwrite.
    let mut writer = directory.open_write(path).unwrap();
    assert!(directory.exists(path).unwrap());
    assert!(matches!(directory.open_write(path), Err(OpenWriteError::FileAlreadyExists(_))));
    writer.write_all(&[4, 3, 7, 3, 5]).unwrap();
    writer.terminate().unwrap();
    assert_eq!(
        directory.open_read(path).unwrap().read_bytes().unwrap().as_slice(),
        &[4, 3, 7, 3, 5]
    );

    // Delete, then it is gone; deleting again says it does not exist.
    let open_before_delete = directory.open_read(path).unwrap();
    directory.delete(path).unwrap();
    assert!(!directory.exists(path).unwrap());
    assert!(matches!(directory.delete(path), Err(DeleteError::FileDoesNotExist(_))));
    assert!(directory.open_read(path).is_err());
    // A handle opened before the delete still reads (Unix keeps the inode).
    #[cfg(unix)]
    assert_eq!(open_before_delete.read_bytes().unwrap().as_slice(), &[4, 3, 7, 3, 5]);

    // atomic_write replaces, atomic_read returns the latest version.
    directory.atomic_write(path, b"first").unwrap();
    assert_eq!(directory.atomic_read(path).unwrap(), b"first");
    directory.atomic_write(path, b"second, longer").unwrap();
    assert_eq!(directory.atomic_read(path).unwrap(), b"second, longer");
    directory.atomic_write(path, b"").unwrap();
    assert_eq!(directory.atomic_read(path).unwrap(), b"");
    // No temporary file is left next to it.
    let names: Vec<_> =
        std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, [path.as_os_str()]);
}

#[test]
fn unfinished_files_are_reported_as_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let block = 64 * 1024;
    // Header only; a partial first block; one full block sealed and one in memory (the file
    // ends exactly at a block boundary, the case a length check alone cannot catch).
    for (name, len) in [("header-only", 0), ("partial", 1000), ("boundary", 2 * block)] {
        let mut writer = directory.open_write(Path::new(name)).unwrap();
        writer.write_all(&vec![9u8; len]).unwrap();
        writer.flush().unwrap();
        drop(writer);
        let error = directory.open_read(Path::new(name)).unwrap_err();
        assert!(
            matches!(index_file_error(&error), IndexFileError::Incomplete { .. }),
            "{name}: {error}"
        );
    }
    // Files whose writer crashed before the whole header reached disk.
    for (name, bytes) in [("empty", &b""[..]), ("magic-only", b"PKSI"), ("partial-magic", b"PK")] {
        std::fs::write(dir.path().join(name), bytes).unwrap();
        let error = directory.open_read(Path::new(name)).unwrap_err();
        assert!(
            matches!(index_file_error(&error), IndexFileError::Incomplete { .. }),
            "{name}: {error}"
        );
    }
}

#[test]
fn files_from_another_key_or_format_are_named_as_such() {
    let dir = tempfile::tempdir().unwrap();
    let first = directory(dir.path());
    write_file(&first, "segment.idx", b"data");
    first.atomic_write(Path::new("meta.json"), b"{}").unwrap();

    let other = EncryptedDirectory::open(dir.path(), &key(2), 1024).unwrap();
    let error = other.open_read(Path::new("segment.idx")).unwrap_err();
    assert!(matches!(index_file_error(&error), IndexFileError::WrongKey { .. }), "{error}");
    let error = other.atomic_read(Path::new("meta.json")).unwrap_err();
    assert!(matches!(index_file_error(&error), IndexFileError::WrongKey { .. }), "{error}");
    assert!(error.to_string().contains("different key"), "{error}");

    std::fs::write(dir.path().join("plain.json"), b"{\"segments\": []}").unwrap();
    let error = first.atomic_read(Path::new("plain.json")).unwrap_err();
    assert!(matches!(index_file_error(&error), IndexFileError::NotEncrypted { .. }), "{error}");

    let mut future = std::fs::read(dir.path().join("segment.idx")).unwrap();
    future[4] = 2;
    std::fs::write(dir.path().join("future.idx"), future).unwrap();
    let error = first.open_read(Path::new("future.idx")).unwrap_err();
    assert!(matches!(
        index_file_error(&error),
        IndexFileError::UnsupportedVersion { version: 2, .. }
    ));
}

/// Moving a whole file to another name, appending a block, and cutting blocks off the end are
/// caught without help from Tantivy's own reads.
#[test]
fn renamed_extended_and_shortened_files_fail_to_open() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let block = 64 * 1024;
    let data = pseudo_random_bytes(3 * block + 10, 7);
    write_file(&directory, "a.idx", &data);
    write_file(&directory, "b.idx", &data);
    let sealed = std::fs::read(dir.path().join("a.idx")).unwrap();
    // Both files' blocks are in the cache, as they would be in a running app: a cached block
    // must not stand in for one that no longer verifies.
    for name in ["a.idx", "b.idx"] {
        assert_eq!(
            directory.open_read(Path::new(name)).unwrap().read_bytes().unwrap().as_slice(),
            &data[..]
        );
    }
    let sealed_block = block + 16;
    let header = 53;

    let cases: Vec<(&str, Vec<u8>)> = vec![
        // a.idx's bytes under the name b.idx.
        ("renamed", sealed.clone()),
        // The last two blocks cut off: what remains is two whole, authentic blocks.
        ("cut at a block boundary", sealed[..header + 2 * sealed_block].to_vec()),
        // Cut inside the last block.
        ("cut inside a block", sealed[..sealed.len() - 5].to_vec()),
        // A copy of block 0 appended after the final block.
        (
            "block appended",
            [sealed.clone(), sealed[header..header + sealed_block].to_vec()].concat(),
        ),
    ];
    for (what, bytes) in cases {
        std::fs::write(dir.path().join("b.idx"), &bytes).unwrap();
        let error = directory.open_read(Path::new("b.idx")).unwrap_err();
        let error = index_file_error(&error);
        assert!(
            matches!(error, IndexFileError::Tampered { .. } | IndexFileError::Incomplete { .. }),
            "{what}: {error}"
        );
    }
}

#[test]
fn watchers_hear_about_meta_json_only_while_subscribed() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let calls = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = channel();
    let counter = calls.clone();
    let handle = directory
        .watch(WatchCallback::new(move || {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = sender.send(n);
        }))
        .unwrap();
    let timeout = Duration::from_secs(2);
    directory.atomic_write(Path::new("meta.json"), b"one").unwrap();
    assert_eq!(receiver.recv_timeout(timeout), Ok(1));
    directory.atomic_write(Path::new(".managed.json"), b"[]").unwrap();
    directory.atomic_write(Path::new("meta.json"), b"two").unwrap();
    assert_eq!(receiver.recv_timeout(timeout), Ok(2));
    drop(handle);
    directory.atomic_write(Path::new("meta.json"), b"three").unwrap();
    assert!(receiver.recv_timeout(Duration::from_millis(300)).is_err());
}

fn lock(name: &str, blocking: bool) -> Lock {
    Lock { filepath: PathBuf::from(name), is_blocking: blocking }
}

#[test]
fn locks_exclude_each_other_and_blocking_locks_wait() {
    let dir = tempfile::tempdir().unwrap();
    let directory = directory(dir.path());
    let a = directory.acquire_lock(&lock("a.lock", false)).unwrap();
    let _b = directory.acquire_lock(&lock("b.lock", false)).unwrap();
    assert!(matches!(directory.acquire_lock(&lock("a.lock", false)), Err(LockError::LockBusy)));

    let (sender, receiver) = channel();
    let waiting = directory.clone();
    let waiter = std::thread::spawn(move || {
        let lock = waiting.acquire_lock(&lock("a.lock", true)).unwrap();
        sender.send(()).unwrap();
        drop(lock);
    });
    assert!(
        receiver.recv_timeout(Duration::from_millis(300)).is_err(),
        "blocking lock did not wait"
    );
    drop(a);
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("blocking lock was not granted after release");
    waiter.join().unwrap();
    directory.acquire_lock(&lock("a.lock", false)).unwrap();
}

/// Tantivy's default lock is a file deleted on drop, which a crash never deletes, so every
/// later writer would fail. Here the OS releases the lock when the holder dies.
#[test]
fn a_killed_holder_releases_the_writer_lock() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = ChildProcess::spawn(
        "child_hold_writer_lock",
        &[("INDEX_DIR", dir.path().to_str().unwrap())],
    );
    child.wait_for_line("locked", Duration::from_secs(60));
    let directory = directory(dir.path());
    let writer_lock = lock(".tantivy-writer.lock", false);
    assert!(matches!(directory.acquire_lock(&writer_lock), Err(LockError::LockBusy)));
    child.kill();
    directory.acquire_lock(&writer_lock).expect("lock still held after the holder was killed");
}

#[test]
#[ignore = "runs only as a child of a_killed_holder_releases_the_writer_lock"]
fn child_hold_writer_lock() {
    if !is_child() {
        return;
    }
    let dir = PathBuf::from(std::env::var("INDEX_DIR").unwrap());
    let directory = directory(&dir);
    let _lock = directory.acquire_lock(&lock(".tantivy-writer.lock", false)).unwrap();
    println!("locked");
    loop {
        std::thread::park();
    }
}

#[test]
fn the_cache_stays_within_its_limit_and_clears() {
    let dir = tempfile::tempdir().unwrap();
    let limit = 256 * 1024;
    let directory = EncryptedDirectory::open(dir.path(), &key(1), limit).unwrap();
    let data = pseudo_random_bytes(2 * 1024 * 1024, 3);
    write_file(&directory, "big.bin", &data);
    let handle = directory.open_read(Path::new("big.bin")).unwrap();
    for _ in 0..3 {
        for start in (0..data.len()).step_by(10_000) {
            let end = (start + 20_000).min(data.len());
            assert_eq!(
                handle.slice(start..end).read_bytes().unwrap().as_slice(),
                &data[start..end]
            );
            assert!(directory.cache_stats().bytes <= limit, "{:?}", directory.cache_stats());
        }
    }
    let stats = directory.cache_stats();
    assert_eq!(stats.entries, 4);
    assert!(stats.hits > 0 && stats.misses > 0, "{stats:?}");
    directory.clear_cache();
    assert_eq!(directory.cache_stats().bytes, 0);
    assert_eq!(directory.cache_stats().entries, 0);
    // Still readable after clearing; blocks come back from disk.
    assert_eq!(handle.read_bytes().unwrap().as_slice(), &data[..]);
}

#[test]
fn concurrent_readers_get_the_same_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let directory = EncryptedDirectory::open(dir.path(), &key(1), 128 * 1024).unwrap();
    let data = Arc::new(pseudo_random_bytes(1024 * 1024 + 77, 9));
    write_file(&directory, "shared.bin", &data);
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let directory = directory.clone();
            let data = data.clone();
            std::thread::spawn(move || {
                let handle = directory.open_read(Path::new("shared.bin")).unwrap();
                let mut rng = Rng::new(t + 1);
                for _ in 0..500 {
                    let start = rng.below(data.len() as u64) as usize;
                    let end = (start + rng.below(200_000) as usize).min(data.len());
                    assert_eq!(
                        handle.slice(start..end).read_bytes().unwrap().as_slice(),
                        &data[start..end]
                    );
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}
