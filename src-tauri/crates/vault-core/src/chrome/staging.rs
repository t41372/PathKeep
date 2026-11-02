//! Browser staging-copy helpers.
//!
//! The parser crate never touches live browser files directly. This module is
//! the bridge that copies History/Favicons databases and their sidecars into a
//! worker-owned staging directory before `vault-core` or the parser reads them.

use super::*;
use crate::archive::source_error::SourceError;
use rusqlite::{Connection, MAIN_DB, OpenFlags};
use std::{
    io::{ErrorKind, Read, Write},
    time::{Duration, SystemTime},
};

/// How long staging operations wait on a lock held by a live browser before
/// giving up. Long enough to outlast a browser's brief write transactions,
/// short enough to keep a backup from stalling on a wedged writer.
const STAGING_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// How many times a raw copy is taken when the browser wrote to the file while it was copied,
/// and the pause before the next. Chrome commits about every ten seconds and a copy takes far
/// less, so the second nearly always fits between two commits.
const RAW_COPY_ATTEMPTS: u32 = 5;
const RAW_COPY_PAUSE: Duration = Duration::from_millis(200);

/// The 16-byte magic every SQLite database file begins with.
const SQLITE_FILE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// Returns the on-disk size for one file, treating missing files as zero.
fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|metadata| metadata.len()).unwrap_or_default()
}

/// Totals the bytes consumed by SQLite sidecars next to one base file.
fn sidecar_bytes(path: &Path) -> u64 {
    ["-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| file_size(&PathBuf::from(format!("{}{}", path.display(), suffix))))
        .sum()
}

/// Computes the storage footprint summary used in the browser profile read model.
pub(super) fn profile_storage_bytes(
    history_path: &Path,
    favicons_path: Option<&Path>,
) -> (u64, u64, u64) {
    let history_bytes = file_size(history_path);
    let favicons_bytes = favicons_path.map(file_size).unwrap_or_default();
    let supporting_bytes =
        sidecar_bytes(history_path) + favicons_path.map(sidecar_bytes).unwrap_or_default();
    (history_bytes, favicons_bytes, supporting_bytes)
}

/// Copies a selected browser profile into staging and fingerprints the copied sources.
pub fn stage_profile_snapshot(
    paths: &ProjectPaths,
    profile: &BrowserProfile,
) -> Result<StagedProfile> {
    // Before the copy starts: nothing it holds is older than this.
    let captured_at_ms = chrono::Utc::now().timestamp_millis();
    let temp_prefix = format!(
        "{}-{}",
        filesystem_safe_path_segment(&profile.profile_id),
        now_rfc3339().replace(':', "-")
    );
    let temp_dir = tempfile::Builder::new()
        .prefix(&temp_prefix)
        .tempdir_in(&paths.staging_dir)
        .with_context(|| format!("creating temp dir in {}", paths.staging_dir.display()))?;
    let source_dir = PathBuf::from(&profile.profile_path);

    let mut warnings = Vec::new();
    let staged_history =
        copy_database_with_sidecars(&source_dir, &profile.history_file_name, temp_dir.path())
            .with_context(|| staging_access_hint(profile))?;
    if let Some(reason) = &staged_history.fallback_reason {
        warnings.push(staging_fallback_warning(profile, &profile.history_file_name, reason));
    }
    let history_path = staged_history.path;

    let staged_favicons = if profile.favicons_path.is_some() {
        match copy_database_with_sidecars(&source_dir, "Favicons", temp_dir.path()) {
            Ok(staged) => Some(staged),
            Err(error) if error.downcast_ref::<SourceError>().is_some() => {
                warnings.push(
                    crate::models::BackupWarning::new(
                        "source-favicons-unavailable",
                        "source-favicons-unavailable",
                    )
                    .with_profile_id(profile.profile_id.clone())
                    .with_diagnostic(format!("{error:#}")),
                );
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    if let Some(reason) =
        staged_favicons.as_ref().and_then(|staged| staged.fallback_reason.as_ref())
    {
        warnings.push(staging_fallback_warning(profile, "Favicons", reason));
    }
    let favicons_path = staged_favicons.map(|staged| staged.path);

    let mut source_hashes = Vec::new();
    for path in [Some(history_path.clone()), favicons_path.clone()].into_iter().flatten() {
        source_hashes.push(FileFingerprint {
            sha256: file_sha256_hex(&path)?,
            path: path.display().to_string(),
        });
    }

    Ok(StagedProfile {
        snapshot: ProfileSnapshot {
            profile: profile.clone(),
            temp_dir,
            history_path,
            favicons_path,
            source_hashes,
            captured_at_ms,
        },
        warnings,
    })
}

/// Builds the non-fatal note recorded when a profile's database had to be staged
/// through the raw-copy + recovery fallback instead of an online snapshot.
///
/// Coded so the Audit surface can localize the degradation; the snapshot
/// failure reason is a true error chain, so it rides in `diagnostic` (with the
/// affected source label) instead of being pattern-matched out of the prose.
fn staging_fallback_warning(
    profile: &BrowserProfile,
    source_label: &str,
    fallback: &Fallback,
) -> crate::models::BackupWarning {
    let (code, why) = if fallback.in_use {
        ("staging-fallback-recovered-copy", "the browser held the database")
    } else {
        ("staging-fallback-repaired-copy", "the database needed repair")
    };
    let reason = &fallback.reason;
    crate::models::BackupWarning::new(
        code,
        format!(
            "{} ({source_label}): {why}, so the backup used a recovered file copy instead of an online snapshot — {reason}",
            profile.profile_id
        ),
    )
    .with_profile_id(profile.profile_id.clone())
    .with_diagnostic(format!("{source_label}: {reason}"))
}

/// A staged database file plus how it got there.
pub(crate) struct StagedDatabase {
    /// The staged copy the parser will read.
    pub path: PathBuf,
    /// Set when the preferred online snapshot failed and we used the slower
    /// raw-copy + recovery fallback, with why, so the backup run can record why
    /// staging was degraded; `None` on the clean path.
    pub fallback_reason: Option<Fallback>,
}

/// Why a database was staged from a raw copy.
pub(crate) struct Fallback {
    /// The browser held the database locked: Chrome and desktop Firefox do while they run, so
    /// this is how their files are read while they are open.
    pub in_use: bool,
    /// The online snapshot's error chain.
    pub reason: String,
}

/// Copies one SQLite database plus its known sidecars into the staging directory.
pub(super) fn copy_database_with_sidecars(
    source_dir: &Path,
    base_name: &str,
    destination_dir: &Path,
) -> Result<StagedDatabase> {
    stage_sqlite_file(&source_dir.join(base_name), &destination_dir.join(base_name))
}

/// Copies one SQLite database a browser may be writing to `destination`, as a file the read-only
/// parser can open: an online snapshot when the source allows one, else a raw copy with its
/// sidecars, recovered. Backups and Browser Direct imports both stage this way; the imports had
/// their own copy, which lacked the recovery and failed on a hot journal.
pub(crate) fn stage_sqlite_file(source: &Path, destination: &Path) -> Result<StagedDatabase> {
    // Preferred path: an online backup yields a single, transactionally
    // consistent, journal-free file even while the browser keeps writing. Its
    // failure is not terminal — we fall back to the raw copy below — but we keep
    // the reason so the run can surface why staging was degraded.
    let fallback = match snapshot_sqlite_database(source, destination) {
        Ok(()) => {
            return Ok(StagedDatabase { path: destination.to_path_buf(), fallback_reason: None });
        }
        Err(error) if error.downcast_ref::<SourceError>().is_some() => {
            Fallback { in_use: held_by_another_process(&error), reason: format!("{error:#}") }
        }
        Err(error) => return Err(error),
    };

    // Fallback: a plain file copy. This can capture the live browser's hot
    // rollback journal or WAL, so we copy those sidecars too — they hold the
    // pages needed to reach a consistent state. Without them the main file is a
    // torn copy. A copy the browser wrote during is taken again.
    copy_while_still(source, || copy_with_sidecars(source, destination))?;

    // The read-only parser cannot replay a hot journal or WAL itself
    // (SQLITE_READONLY_ROLLBACK / _RECOVERY). Fold them into the main file now,
    // on our private throwaway copy where writing is safe — never the live
    // browser file. Non-SQLite sources (the usual reason the online backup
    // failed) have nothing to recover; let the parser report their real problem.
    if is_sqlite_database(destination)? {
        recover_staged_database(destination).map_err(|error| {
            if let Some(sqlite) = error.downcast_ref::<rusqlite::Error>()
                && matches!(sqlite, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code, rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase)) {
                return error.context(SourceError::Failed("source-read-failed"));
            }
            error
        })?;
    }

    Ok(StagedDatabase { path: destination.to_path_buf(), fallback_reason: Some(fallback) })
}

/// Whether the online snapshot failed because another process held the database (BUSY or
/// LOCKED), rather than on the file itself (a hot journal, damage, not a database at all).
fn held_by_another_process(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<rusqlite::Error>().is_some_and(|error| {
            matches!(error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
        }) || cause.downcast_ref::<SnapshotBusy>().is_some()
    })
}

/// The online snapshot could not go on: another process holds the database.
#[derive(Debug, thiserror::Error)]
#[error("browser snapshot is busy")]
struct SnapshotBusy;

/// Runs `copy` until nothing wrote to `source` while it ran. The browser keeps writing while a
/// raw copy is taken (Chrome commits about every ten seconds, Firefox appends to its WAL), and
/// a copy that overlapped a write can hold pages from before and after it: a database that never
/// existed, which still opens and parses, and nothing in the file says so. A browser that writes
/// through every try fails this read; the next backup tries again.
fn copy_while_still(source: &Path, mut copy: impl FnMut() -> Result<()>) -> Result<()> {
    for attempt in 1..=RAW_COPY_ATTEMPTS {
        let before = write_marks(source);
        copy()?;
        if write_marks(source) == before {
            return Ok(());
        }
        if attempt < RAW_COPY_ATTEMPTS {
            std::thread::sleep(RAW_COPY_PAUSE);
        }
    }
    Err(anyhow::anyhow!(
        "the browser wrote to {} during each of {RAW_COPY_ATTEMPTS} copies",
        source.display()
    )
    .context(SourceError::Failed("source-read-failed")))
}

/// Size, modification time and creation time of the main file and of the sidecars that hold its
/// pages. On macOS (APFS) and Linux every write sets the modification time another process reads
/// at once, kept to the nanosecond or near it, and a file put in another's place has its own
/// creation time; so equal before and after a copy, nothing wrote to them or replaced them in
/// between. Windows does not promise this: NTFS may not update a file's last-write time until the
/// handles that wrote to it are closed (`WriteFile`), and a running browser keeps its handle
/// open, so a write that keeps the size can leave every mark as it was. There the check catches
/// writes that change a size, not all of them (`WORK-WINDOWS-RAW-COPY-CHECK`). A filesystem that
/// keeps times to the second (FAT, exFAT) could hide a write of the same size within one; browser
/// profiles do not live on those. `-shm` is left out: SQLite rebuilds it from the WAL when the
/// copy is first opened, and readers write to it too.
fn write_marks(source: &Path) -> [Option<WriteMark>; 3] {
    ["", "-wal", "-journal"].map(|suffix| {
        fs::metadata(with_suffix(source, suffix))
            .ok()
            .map(|metadata| (metadata.len(), metadata.modified().ok(), metadata.created().ok()))
    })
}

type WriteMark = (u64, Option<SystemTime>, Option<SystemTime>);

/// Copies the main file and whichever of its sidecars exist. One an earlier try copied that is
/// gone now is removed from the copy, which would otherwise replay it.
fn copy_with_sidecars(source: &Path, destination: &Path) -> Result<()> {
    copy_with_context(source, destination)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let target = with_suffix(destination, suffix);
        match fs::remove_file(&target) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                return Err(error)
                    .with_context(|| format!("removing staged sidecar {}", target.display()));
            }
            _ => {}
        }
        // Gone between the browser's writes is not an error: the marks above see the change.
        match fs::File::open(with_suffix(source, suffix)) {
            Ok(file) => copy_open_file(file, &with_suffix(source, suffix), &target)?,
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(open_error(error, &with_suffix(source, suffix))),
        }
    }
    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.display()))
}

fn snapshot_sqlite_database(source: &Path, destination: &Path) -> Result<()> {
    let source_connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| {
        anyhow::Error::new(error).context(SourceError::Unavailable("source-unreadable"))
    })
    .with_context(|| format!("opening source database {}", source.display()))?;
    // The browser is still writing while we copy. Wait out its short write locks
    // instead of dropping into the slower, torn-copy-prone raw fallback on the
    // first `SQLITE_BUSY`.
    source_connection
        .busy_timeout(STAGING_BUSY_TIMEOUT)
        .context("configuring snapshot busy timeout")?;
    // Opening the destination is machinery work; its failures never become source outcomes.
    let mut destination_connection = Connection::open(destination)
        .with_context(|| format!("opening staging destination {}", destination.display()))?;
    let backup = rusqlite::backup::Backup::new_with_names(
        &source_connection,
        MAIN_DB,
        &mut destination_connection,
        MAIN_DB,
    )
    .map_err(classify_source_sqlite_error)?;
    // All pages in one step, under one read transaction: between smaller steps SQLite lets go of
    // the source, and every write the browser makes starts the copy again from the first page.
    // A WAL reader does not hold up the browser's writes.
    match backup.step(-1).map_err(classify_source_sqlite_error)? {
        rusqlite::backup::StepResult::Done => Ok(()),
        rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => {
            Err(anyhow::Error::new(SnapshotBusy).context(SourceError::Failed("source-read-failed")))
        }
        _ => Err(anyhow::anyhow!("unexpected browser snapshot step")),
    }
}

/// Folds a captured hot rollback journal or live WAL back into the main database
/// file so the read-only parser can open the staged copy.
///
/// Opening read-write is safe here and only here: `path` is always a throwaway
/// file inside the worker-owned staging directory, never a live browser file.
/// SQLite replays a hot rollback journal on the first access from a read-write
/// connection; switching to a DELETE journal additionally checkpoints any WAL
/// into the main file, leaving one self-contained database with no sidecar the
/// read-only parser would have to replay.
pub(super) fn recover_staged_database(path: &Path) -> Result<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening staged database for recovery {}", path.display()))?;
    connection
        .busy_timeout(STAGING_BUSY_TIMEOUT)
        .context("configuring staged recovery busy timeout")?;
    connection
        .query_row("PRAGMA journal_mode=DELETE", [], |row| row.get::<_, String>(0))
        .with_context(|| {
            format!("normalizing journal mode for staged database {}", path.display())
        })?;
    // Touching the schema forces the hot-journal rollback to run and confirms the
    // recovered file is actually readable before the parser depends on it.
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .with_context(|| format!("verifying recovered staged database {}", path.display()))?;
    Ok(())
}

/// Cheap check for the SQLite file header, used to skip recovery on the
/// non-database files that send the staging copy down the raw-copy fallback.
fn is_sqlite_database(path: &Path) -> Result<bool> {
    let mut header = [0u8; 16];
    let mut file = fs::File::open(path)
        .with_context(|| format!("opening staged header {}", path.display()))?;
    match file.read_exact(&mut header) {
        Ok(()) => Ok(&header == SQLITE_FILE_HEADER),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("reading staged header {}", path.display()))
        }
    }
}

/// Copies one file while preserving the source/destination context in errors.
fn copy_with_context(source: &Path, destination: &Path) -> Result<()> {
    let input = fs::File::open(source).map_err(|error| open_error(error, source))?;
    copy_open_file(input, source, destination)
}

/// A source that cannot be opened is `unavailable`, with why (`SourceError::access`).
fn open_error(error: std::io::Error, source: &Path) -> anyhow::Error {
    let marker = SourceError::access(&error);
    anyhow::Error::new(error)
        .context(marker)
        .context(format!("opening source {}", source.display()))
}

fn copy_open_file(mut input: fs::File, source: &Path, destination: &Path) -> Result<()> {
    let mut output = fs::File::create(destination)
        .with_context(|| format!("creating staging destination {}", destination.display()))?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|error| {
            // Opened but not read to the end is `failed`. A directory opens on Unix and fails
            // here (Safari's History.db is one when its profile is not the expected shape): that
            // is no history file, `unavailable` as if it had not opened.
            let marker = if error.kind() == ErrorKind::IsADirectory {
                SourceError::Unavailable("source-unreadable")
            } else {
                SourceError::Failed("source-read-failed")
            };
            anyhow::Error::new(error)
                .context(marker)
                .context(format!("reading source {}", source.display()))
        })?;
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .with_context(|| format!("writing staging destination {}", destination.display()))?;
    }
    Ok(())
}

/// SQLite reports destination IO/full errors through the same API as source failures.
/// Only unambiguous source conditions are recoverable; everything else retains its chain.
fn classify_source_sqlite_error(error: rusqlite::Error) -> anyhow::Error {
    let source = matches!(&error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code,
        rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked |
        rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase)
        || matches!(code.extended_code, rusqlite::ffi::SQLITE_READONLY_ROLLBACK | rusqlite::ffi::SQLITE_READONLY_RECOVERY));
    let error = anyhow::Error::new(error);
    if source { error.context(SourceError::Failed("source-read-failed")) } else { error }
}

fn staging_access_hint(profile: &BrowserProfile) -> String {
    if profile.browser_family == "safari" {
        return "Safari History.db is not readable yet. Grant Full Disk Access to PathKeep or the running development process, then run the backup again.".to_string();
    }

    format!("staging {} history database from {}", profile.browser_name, profile.profile_path)
}

#[cfg(test)]
mod error_origin_tests {
    use super::*;
    // A missing source is unavailable; an unwritable destination is machinery failure even
    // when snapshot fallback is used. Optional favicon callers must distinguish these too.
    #[test]
    fn staging_classifies_source_but_never_destination_errors() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("History");
        let missing = stage_sqlite_file(&source, &dir.path().join("copy")).err().unwrap();
        assert!(missing.downcast_ref::<crate::archive::source_error::SourceError>().is_some());
        Connection::open(&source).unwrap().execute_batch("CREATE TABLE urls(id INTEGER)").unwrap();
        let failed = stage_sqlite_file(&source, &dir.path().join("absent/copy")).err().unwrap();
        assert!(failed.downcast_ref::<crate::archive::source_error::SourceError>().is_none());
    }
}

#[cfg(test)]
mod raw_copy_tests {
    //! A raw copy of a database a running browser holds locked. Ways it can fail:
    //! - the browser commits while the main file is copied, and the copy holds pages from
    //!   before and after the commit: a database that never existed, which opens and parses;
    //! - the browser keeps writing through every try, and a copy is used anyway;
    //! - a journal or WAL an earlier try copied, gone by the next, is replayed into that copy.
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }

    #[test]
    fn a_copy_the_browser_wrote_during_is_taken_again() {
        let dir = tempfile::tempdir().unwrap();
        let (source, copy) = (dir.path().join("History"), dir.path().join("copy"));
        write(&source, b"before");
        let mut tries = 0;
        copy_while_still(&source, || {
            tries += 1;
            copy_with_sidecars(&source, &copy)?;
            if tries == 1 {
                write(&source, b" commit");
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(tries, 2);
        assert_eq!(fs::read(&copy).unwrap(), b"before commit");
    }

    #[test]
    fn a_browser_that_writes_through_every_copy_fails_the_read() {
        let dir = tempfile::tempdir().unwrap();
        let (source, copy) = (dir.path().join("History"), dir.path().join("copy"));
        write(&source, b"start");
        let error = copy_while_still(&source, || {
            copy_with_sidecars(&source, &copy)?;
            write(&source, b"+");
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<SourceError>(),
            Some(SourceError::Failed("source-read-failed"))
        ));
    }

    #[test]
    fn a_journal_gone_by_the_next_try_is_not_kept_in_the_copy() {
        let dir = tempfile::tempdir().unwrap();
        let (source, copy) = (dir.path().join("History"), dir.path().join("copy"));
        write(&source, b"pages");
        let journal = |path: &Path| PathBuf::from(format!("{}-journal", path.display()));
        write(&journal(&source), b"hot");
        copy_with_sidecars(&source, &copy).unwrap();
        assert!(journal(&copy).exists());
        fs::remove_file(journal(&source)).unwrap();
        copy_with_sidecars(&source, &copy).unwrap();
        assert!(!journal(&copy).exists());
    }
}
