//! Responsibilities: say exactly what a file, a folder or a symlink holds, so that the install
//! journal takes out of its place only a file that holds what the install put there. What it holds,
//! not where it lives or what the file system calls it: a copy says the same, and so do a rename, a
//! remount and a restore from a backup, with or without the files' times; one changed byte does not.
//!
//! Not responsible for: what a record does with a file (the install journal decides by where its
//! files are, and only checks with this before it takes one out of its place), or reading a
//! database.
//!
//! Failure modes: two different contents taken for the same (every byte goes through SHA-256, and
//! so does every byte of every name, in fields that carry their length: no);
//! a faithful copy taken for another (times, owners, modes and file numbers are not part of it, and
//! neither are the files a file browser drops into a folder, nor empty folders a copy tool skips);
//! a large archive read too slowly (once, in chunks hashed in parallel).
//!
//! Tests: `maintenance::install_tests`.

use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs,
    io::{self, Read, Seek},
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A file is hashed in chunks of this size, in parallel, and the chunks' hashes are hashed with its
/// length. Tests use small chunks, so small files have several.
#[cfg(not(test))]
const CHUNK: u64 = 64 << 20;
#[cfg(test)]
const CHUNK: u64 = 4 << 10;

/// What the file, folder or symlink at `path` holds, without following a symlink. `None` when
/// nothing is there.
///
/// - A file: `file:` and the hash of its length and of its chunks' hashes.
/// - A folder: `tree:` and the hash of an entry for every file and symlink in it: the names on its
///   path and what it holds, each a field that carries its length, the entries sorted. A name may
///   hold a tab or a newline, so only lengths keep one entry from reading like two. Folders
///   themselves are not entries: an empty one holds nothing. Finder's `.DS_Store` and the like are
///   skipped (`vault_layout::is_os_metadata`).
/// - A symlink: `link:` and the hash of its target.
pub(crate) fn of(path: &Path) -> io::Result<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some(if metadata.file_type().is_symlink() {
        link(path)?
    } else if metadata.is_dir() {
        tree(path)?
    } else {
        file(path, metadata.len())?
    }))
}

fn link(path: &Path) -> io::Result<String> {
    let target = fs::read_link(path)?;
    Ok(format!("link:{}", hex(&Sha256::digest(exact(target.as_os_str())))))
}

fn tree(root: &Path) -> io::Result<String> {
    let mut entries = Vec::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1).follow_links(false) {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_name().to_str().is_some_and(vault_layout::is_os_metadata) {
            continue;
        }
        let what = if entry.file_type().is_symlink() {
            link(entry.path())?
        } else if entry.file_type().is_dir() {
            continue;
        } else {
            file(entry.path(), entry.metadata().map_err(io::Error::other)?.len())?
        };
        let relative = entry.path().strip_prefix(root).map_err(io::Error::other)?;
        let mut names = Vec::new();
        for name in relative {
            field(&mut names, &exact(name));
        }
        let mut one = Vec::new();
        field(&mut one, &names);
        field(&mut one, what.as_bytes());
        entries.push(one);
    }
    // Sorted, so the order a walk happens to find them in does not count.
    entries.sort();
    let mut whole = Sha256::new();
    for one in entries {
        whole.update(one);
    }
    Ok(format!("tree:{}", hex(&whole.finalize())))
}

/// `bytes` behind their length, so that fields one after another read only one way, whatever bytes
/// they hold.
fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Every byte of a name or a symlink's target: its UTF-8 where it is Unicode, which every system
/// writes the same way, and otherwise the system's own bytes behind a different first byte.
/// `display()` would turn every byte that is not UTF-8 into the same `�`, so two names would read
/// as one.
fn exact(name: &OsStr) -> Vec<u8> {
    if let Some(text) = name.to_str() {
        return [&[0u8][..], text.as_bytes()].concat();
    }
    let mut bytes = vec![1u8];
    #[cfg(unix)]
    bytes.extend_from_slice(std::os::unix::ffi::OsStrExt::as_bytes(name));
    #[cfg(windows)]
    bytes.extend(std::os::windows::ffi::OsStrExt::encode_wide(name).flat_map(u16::to_le_bytes));
    bytes
}

fn file(path: &Path, len: u64) -> io::Result<String> {
    let chunks = usize::try_from(len.div_ceil(CHUNK).max(1)).map_err(io::Error::other)?;
    let hashes = in_parallel(chunks, |index| {
        let start = index as u64 * CHUNK;
        chunk(path, start, CHUNK.min(len - start))
    })?;
    let mut whole = Sha256::new();
    whole.update(len.to_le_bytes());
    for hash in hashes {
        whole.update(hash);
    }
    Ok(format!("file:{}", hex(&whole.finalize())))
}

/// The hash of `want` bytes from `start`. A file shorter than it was a moment ago is an error, not a
/// shorter content.
fn chunk(path: &Path, start: u64, want: u64) -> io::Result<[u8; 32]> {
    let mut file = fs::File::open(path)?;
    file.seek(io::SeekFrom::Start(start))?;
    let mut hasher = Sha256::new();
    let read = io::copy(&mut file.take(want), &mut hasher)?;
    if read != want {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("{} changed while it was read", path.display()),
        ));
    }
    Ok(hasher.finalize().into())
}

/// `each(0..count)`, on as many threads as the machine has cores, in order.
fn in_parallel<T: Send>(
    count: usize,
    each: impl Fn(usize) -> io::Result<T> + Sync,
) -> io::Result<Vec<T>> {
    if count == 1 {
        return Ok(vec![each(0)?]);
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(count);
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, io::Result<T>)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= count {
                            break mine;
                        }
                        mine.push((index, each(index)));
                    }
                })
            })
            .collect();
        workers.into_iter().flat_map(|worker| worker.join().expect("a hashing thread")).collect()
    });
    done.sort_by_key(|(index, _)| *index);
    done.into_iter().map(|(_, result)| result).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::of;

    // A file of several chunks: a copy gives the same answer, and one changed byte in any chunk, or
    // one byte less, gives another.
    #[test]
    fn one_changed_byte_anywhere_changes_it_and_a_copy_does_not() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("big");
        let mut bytes = vec![7u8; (super::CHUNK * 2 + 5) as usize];
        std::fs::write(&path, &bytes).unwrap();
        let first = of(&path).unwrap().unwrap();
        let copy = folder.path().join("copy");
        std::fs::copy(&path, &copy).unwrap();
        assert_eq!(of(&copy).unwrap().unwrap(), first);
        for at in [0, super::CHUNK as usize, bytes.len() - 1] {
            bytes[at] ^= 1;
            std::fs::write(&path, &bytes).unwrap();
            assert_ne!(of(&path).unwrap().unwrap(), first, "byte {at}");
            bytes[at] ^= 1;
        }
        std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
        assert_ne!(of(&path).unwrap().unwrap(), first, "one byte shorter");
    }

    // A folder is what its files hold: not its times, not an empty folder, not a `.DS_Store`.
    #[test]
    fn a_folder_is_what_its_files_hold() {
        let folder = tempfile::tempdir().unwrap();
        let index = folder.path().join("index");
        std::fs::create_dir_all(index.join("segments")).unwrap();
        std::fs::write(index.join("segments/a"), b"a").unwrap();
        std::fs::write(index.join("meta"), b"meta").unwrap();
        let first = of(&index).unwrap().unwrap();
        std::fs::create_dir(index.join("empty")).unwrap();
        std::fs::write(index.join(".DS_Store"), b"finder").unwrap();
        std::fs::write(index.join("segments/._a"), b"finder").unwrap();
        crate::utils::set_folder_modified(&index, std::time::SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(of(&index).unwrap().unwrap(), first);
        std::fs::write(index.join("segments/b"), b"b").unwrap();
        assert_ne!(of(&index).unwrap().unwrap(), first, "another file");
        std::fs::remove_file(index.join("segments/b")).unwrap();
        std::fs::rename(index.join("meta"), index.join("segments/meta")).unwrap();
        assert_ne!(of(&index).unwrap().unwrap(), first, "a file moved");
        assert_eq!(of(&folder.path().join("nothing")).unwrap(), None);
    }

    // A name may hold any byte but `/` and NUL, and a symlink's target any byte but NUL. Folders
    // whose files have other names, and links with other targets, never read the same, however the
    // bytes line up: a name that reads like the next entry, a name that runs on into the next one, a
    // target that is not UTF-8.
    #[cfg(unix)]
    #[test]
    fn names_and_targets_count_byte_for_byte() {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        let folder = tempfile::tempdir().unwrap();
        let make = |name: &str| {
            let path = folder.path().join(name);
            std::fs::create_dir(&path).unwrap();
            path
        };
        let two = make("two");
        std::fs::write(two.join("a"), b"x").unwrap();
        std::fs::write(two.join("b"), b"y").unwrap();
        let (x, y) = (of(&two.join("a")).unwrap().unwrap(), of(&two.join("b")).unwrap().unwrap());

        let reads_like_two = make("reads like two");
        std::fs::write(reads_like_two.join(format!("a\t{x}\nb")), b"y").unwrap();
        assert_ne!(of(&reads_like_two).unwrap(), of(&two).unwrap(), "a name like two entries");
        let runs_on = make("runs on");
        std::fs::create_dir(runs_on.join(format!("a{x}"))).unwrap();
        std::fs::write(runs_on.join(format!("a{x}/b")), b"y").unwrap();
        assert_ne!(of(&runs_on).unwrap(), of(&two).unwrap(), "a name that runs on");

        let linked = make("linked");
        symlink("t", linked.join("a")).unwrap();
        std::fs::write(linked.join("b"), b"y").unwrap();
        let one_link = make("one link");
        symlink(format!("t\nb\t{y}"), one_link.join("a")).unwrap();
        assert_ne!(of(&one_link).unwrap(), of(&linked).unwrap(), "a target like two entries");
        for (name, target) in [("ff", b"t\xff"), ("fe", b"t\xfe")] {
            symlink(std::ffi::OsStr::from_bytes(target), folder.path().join(name)).unwrap();
        }
        let [ff, fe] = ["ff", "fe"].map(|name| of(&folder.path().join(name)).unwrap());
        assert_ne!(ff, fe, "targets that are not UTF-8");
    }
}
