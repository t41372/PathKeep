//! Responsibilities: moving and deleting PathKeep's own files so that another program holding one
//! for a moment does not stop the work. Windows refuses to rename or delete a file another program
//! has open without delete sharing, and an antivirus scan, the search indexer or a sync client opens
//! a file for a moment right after it is written or moved, which is when PathKeep moves or deletes
//! it next. Every PathKeep crate that moves or deletes files in the data folder does it through
//! here, so they all wait the same way.
//! Not responsible for: durability (vault-core's `durable_io` flushes and syncs around these
//! calls), deciding what to move or delete, or a program that keeps a file open: the call fails
//! after the wait, as it would have at once.
//!
//! What waits and what does not:
//! - A move or a delete the work needs in order to finish waits: putting a written file in place,
//!   clearing or resetting an index, removing a temporary file that would be in the next step's way,
//!   every step of a restore, an import or "Delete all data".
//! - A cleanup that tries again on its next pass calls `std::fs` and does not wait: tantivy's
//!   garbage collection, which keeps a file it could not delete, and the semantic index's trash. A
//!   reader keeps those files open for as long as it reads, and a wait there would hold up the
//!   commit or the collection beside it on every pass.
//!
//! How this could fail, and what guards it:
//! - A wait that never ends: the budget is fixed ([`HELD_RETRIES`]) and the last try's error is
//!   returned. The tests below check it on every system with a stand-in for "held".
//! - A failure that has nothing to do with another program, waited on for nothing: only Windows's
//!   access-denied, sharing and lock violations are tried again, and only on Windows.
//! - A temporary file lost between tries: a failed `persist` hands the file back, which is kept for
//!   the next try and deleted by its guard when every try fails.
//! - A wrapper that does not wait: on Windows a test holds a file without delete sharing and checks
//!   that each waits, and that one held throughout fails within seconds.

use std::{
    fs::{self, File},
    io,
    path::Path,
    time::Duration,
};
use tempfile::NamedTempFile;

/// How many more times a move or a delete is tried while another program holds the file.
///
/// SQLite retries the same errors on the same budget (`winRetryIoerr` in its `os_win.c`): ten more
/// tries, each 25 ms longer apart, about 1.4 s in all. A program that holds the file longer still
/// makes the call fail.
const HELD_RETRIES: u64 = 10;

/// `std::fs::rename`, waiting for another program to let go of either file.
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    retry_while_held(|| fs::rename(from, to))
}

/// `std::fs::remove_file`, waiting for another program to let go of the file.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    retry_while_held(|| fs::remove_file(path))
}

/// `std::fs::remove_dir`, waiting for another program to let go of the folder.
pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    retry_while_held(|| fs::remove_dir(path))
}

/// `std::fs::remove_dir_all`, waiting for another program to let go of what is in the folder. A
/// try that stops part of the way leaves the rest for the next, which picks up where it stopped.
pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    retry_while_held(|| fs::remove_dir_all(path))
}

/// `NamedTempFile::persist`, waiting for another program to let go of the temporary file or of the
/// one it replaces. When every try fails the temporary file is deleted.
pub fn persist(temp: NamedTempFile, path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    persist_with(temp, |temp| temp.persist(path))
}

/// `NamedTempFile::persist_noclobber`, waiting the same way. A file already at `path` is an error
/// at once (`AlreadyExists`), never waited on.
pub fn persist_noclobber(temp: NamedTempFile, path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    persist_with(temp, |temp| temp.persist_noclobber(path))
}

fn persist_with(
    temp: NamedTempFile,
    mut persist: impl FnMut(NamedTempFile) -> Result<File, tempfile::PersistError>,
) -> io::Result<File> {
    let mut temp = Some(temp);
    retry_while_held(|| {
        let file = temp.take().expect("a failed persist hands the temporary file back");
        persist(file).map_err(|error| {
            temp = Some(error.file);
            error.error
        })
    })
}

/// Runs `op`, and again while it fails only because another program holds the file, within
/// [`HELD_RETRIES`], each wait 25 ms longer. For a step the functions above do not cover: one
/// that decides between a file and a folder, or opens a file to flush it. Elsewhere than on
/// Windows nothing holds a file against a rename or a delete, and `op` runs once.
pub fn retry_while_held<T>(op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    retry(op, held_by_another_program, |attempt| {
        std::thread::sleep(Duration::from_millis(25 * attempt))
    })
}

fn retry<T>(
    mut op: impl FnMut() -> io::Result<T>,
    held: impl Fn(&io::Error) -> bool,
    mut wait: impl FnMut(u64),
) -> io::Result<T> {
    for attempt in 1..=HELD_RETRIES {
        match op() {
            Err(error) if held(&error) => wait(attempt),
            done => return done,
        }
    }
    op()
}

/// `ERROR_ACCESS_DENIED`, `ERROR_SHARING_VIOLATION` and `ERROR_LOCK_VIOLATION`: what Windows
/// returns for a file another program has open. The first is also what a file nobody may change
/// gives, which then fails after the wait.
fn held_by_another_program(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The retry itself, on every system, with a stand-in for "another program holds the file":
    /// tried again within the budget, each wait longer; given up after it with the last error; any
    /// other failure, and a success, returned at once. Only Windows's codes count as held.
    #[test]
    fn a_move_or_delete_is_tried_again_only_while_the_file_is_held() {
        let held = || io::Error::from_raw_os_error(32);
        let is_held = |error: &io::Error| error.raw_os_error() == Some(32);
        let (mut calls, mut waits) = (0, Vec::new());
        let result = retry(
            || {
                calls += 1;
                if calls <= 3 { Err(held()) } else { Ok(calls) }
            },
            is_held,
            |attempt| waits.push(attempt),
        );
        assert_eq!(result.expect("let go"), 4);
        assert_eq!(waits, [1, 2, 3]);

        let mut calls = 0;
        let error = retry(
            || -> io::Result<()> {
                calls += 1;
                Err(held())
            },
            is_held,
            |_| {},
        )
        .expect_err("held throughout");
        assert_eq!(error.raw_os_error(), Some(32));
        assert_eq!(calls, HELD_RETRIES + 1);

        let mut calls = 0;
        let error = retry(
            || -> io::Result<()> {
                calls += 1;
                Err(io::Error::from(io::ErrorKind::AlreadyExists))
            },
            is_held,
            |_| panic!("another failure is not waited on"),
        )
        .expect_err("not held");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(calls, 1);

        assert_eq!(held_by_another_program(&held()), cfg!(windows));
    }

    /// A temporary file whose every try fails is deleted, and one that is put in place is the file
    /// that was written. Nothing is held here, so this runs the same on every system.
    #[test]
    fn a_temporary_file_is_put_in_place_or_deleted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut temp = NamedTempFile::new_in(dir.path()).expect("temp");
        io::Write::write_all(&mut temp, b"written").expect("write");
        let placed = dir.path().join("placed");
        persist(temp, &placed).expect("put in place");
        assert_eq!(fs::read(&placed).expect("read"), b"written");

        let temp = NamedTempFile::new_in(dir.path()).expect("temp");
        let temp_path = temp.path().to_path_buf();
        let error = persist_noclobber(temp, &placed).expect_err("a file is there");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(!temp_path.exists(), "the temporary file is not left behind");
        assert_eq!(fs::read(&placed).expect("read"), b"written");
    }

    /// Windows refuses to move, replace or delete a file another program has open without delete
    /// sharing, as an antivirus scan holds a file it reads right after it was written. Each call
    /// waits for the program to let go; one that never does makes it fail, not hang.
    #[cfg(windows)]
    #[test]
    fn a_file_another_program_holds_for_a_moment_is_still_moved_replaced_and_deleted() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        let dir = tempfile::tempdir().expect("tempdir");
        let open = |path: &Path| {
            fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(path).expect("held")
        };
        let hold = |path: &Path| {
            let file = open(path);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                drop(file);
            })
        };
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"a").expect("seed");
        let held = hold(&a);
        rename(&a, &b).expect("moved once let go");
        held.join().expect("let go");

        let held = hold(&b);
        let mut temp = NamedTempFile::new_in(dir.path()).expect("temp");
        io::Write::write_all(&mut temp, b"replaced").expect("write");
        persist(temp, &b).expect("replaced once let go");
        held.join().expect("let go");
        assert_eq!(fs::read(&b).expect("read"), b"replaced");

        let held = hold(&b);
        remove_file(&b).expect("deleted once let go");
        held.join().expect("let go");

        let folder = dir.path().join("folder");
        fs::create_dir(&folder).expect("folder");
        fs::write(folder.join("inside"), b"inside").expect("seed");
        let held = hold(&folder.join("inside"));
        remove_dir_all(&folder).expect("emptied and deleted once let go");
        held.join().expect("let go");
        assert!(!a.exists() && !b.exists() && !folder.exists());

        let c = dir.path().join("c");
        fs::write(&c, b"c").expect("seed");
        let kept = open(&c);
        let started = std::time::Instant::now();
        remove_file(&c).expect_err("held throughout");
        assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
        drop(kept);
        assert!(c.exists());
    }
}
