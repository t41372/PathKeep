//! Responsibilities: restore a pre-Turso archive-only snapshot through the converter.
//! Not responsible for: source checkpoint replay, source evidence reconstruction or UI.
//! Failure modes: wrong password/corruption replaces the live archive, source snapshot
//! is changed, interruption publishes an unverified conversion. Tests: tests/snapshot.rs.
use anyhow::Result;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};
use vault_core::{
    ArchiveMode, ProjectPaths, Vault,
    maintenance::{InstalledArchive, Progress},
};
use vault_store::{CancelToken, OwnerLock};

/// Old full snapshots contain only the archive. Conversion supplies empty evidence
/// and agent databases, and native restore retains the replaced generation.
pub fn restore_snapshot(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    owner: Option<Arc<OwnerLock>>,
    source: &Path,
    password: Option<&str>,
    cancel: CancelToken,
    mut progress: impl FnMut(Progress),
) -> Result<InstalledArchive> {
    cancel.check()?;
    let input = crate::source::open(source, password)?;
    let integrity: String = input.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    anyhow::ensure!(integrity == "ok", "snapshot integrity: {integrity}");
    drop(input);
    fs::create_dir_all(&paths.staging_dir)?;
    let stage =
        tempfile::Builder::new().prefix("maintenance-legacy-").tempdir_in(&paths.staging_dir)?;
    let staged_paths = vault_core::project_paths_with_root(stage.path());
    vault_core::config::ensure_paths(&staged_paths)?;
    let mut config = vault_core::load_config(paths).unwrap_or_default();
    config.initialized = true;
    config.archive_mode = if crate::source::encrypted(source)? {
        ArchiveMode::Encrypted
    } else {
        ArchiveMode::Plaintext
    };
    vault_core::save_config(&staged_paths, &config)?;
    let mut input = fs::File::open(source)?;
    let total = input.metadata()?.len();
    let mut output = fs::File::create(&staged_paths.archive_database_path)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut processed = 0;
    loop {
        cancel.check()?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        processed += count as u64;
        progress(Progress {
            phase: "copying".into(),
            database: Some("Archive".into()),
            table: None,
            processed,
            total,
        });
    }
    output.sync_all()?;
    drop(output);
    let converted = crate::convert(&staged_paths, &config, password, cancel.clone(), progress)?;
    let snapshot = stage.path().join("converted-snapshot");
    vault_core::maintenance::snapshot(&converted.vault, &snapshot)?;
    converted.vault.close()?;
    vault_core::maintenance::restore_snapshot_to(paths, current, owner, &snapshot, password, cancel)
}
