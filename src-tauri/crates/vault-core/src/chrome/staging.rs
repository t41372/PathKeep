//! Browser staging-copy helpers.
//!
//! The parser crate never touches live browser files directly. This module is
//! the bridge that copies History/Favicons databases and their sidecars into a
//! worker-owned staging directory before `vault-core` or the parser reads them.

use super::*;
use rusqlite::{Connection, MAIN_DB, OpenFlags};

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
) -> Result<ProfileSnapshot> {
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
    let history_path =
        copy_database_with_sidecars(&source_dir, &profile.history_file_name, temp_dir.path())
            .with_context(|| staging_access_hint(profile))?;
    let favicons_path = profile
        .favicons_path
        .as_ref()
        .and_then(|_| copy_database_with_sidecars(&source_dir, "Favicons", temp_dir.path()).ok());

    let mut source_hashes = Vec::new();
    for path in [Some(history_path.clone()), favicons_path.clone()].into_iter().flatten() {
        source_hashes.push(FileFingerprint {
            sha256: file_sha256_hex(&path)?,
            path: path.display().to_string(),
        });
    }

    Ok(ProfileSnapshot {
        profile: profile.clone(),
        temp_dir,
        history_path,
        favicons_path,
        source_hashes,
    })
}

/// Copies one SQLite database plus its known sidecars into the staging directory.
pub(super) fn copy_database_with_sidecars(
    source_dir: &Path,
    base_name: &str,
    destination_dir: &Path,
) -> Result<PathBuf> {
    let source = source_dir.join(base_name);
    let destination = destination_dir.join(base_name);
    if snapshot_sqlite_database(&source, &destination).is_ok() {
        return Ok(destination);
    }

    copy_with_context(&source, &destination)?;

    for suffix in ["-wal", "-shm", "-journal"] {
        let source_sidecar = source_dir.join(format!("{base_name}{suffix}"));
        if source_sidecar.exists() {
            let target_sidecar = destination_dir.join(format!("{base_name}{suffix}"));
            copy_with_context(&source_sidecar, &target_sidecar)?;
        }
    }

    Ok(destination)
}

fn snapshot_sqlite_database(source: &Path, destination: &Path) -> Result<()> {
    let source_connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening source database {}", source.display()))?;
    source_connection.backup(MAIN_DB, destination, None).with_context(|| {
        format!(
            "creating online SQLite snapshot from {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

/// Copies one file while preserving the source/destination context in errors.
fn copy_with_context(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination)
        .with_context(|| format!("copying {} to {}", source.display(), destination.display()))?;
    Ok(())
}

fn staging_access_hint(profile: &BrowserProfile) -> String {
    if profile.browser_family == "safari" {
        return "Safari History.db is not readable yet. Grant Full Disk Access to PathKeep or the running development process, then run the backup again.".to_string();
    }

    format!("staging {} history database from {}", profile.browser_name, profile.profile_path)
}
