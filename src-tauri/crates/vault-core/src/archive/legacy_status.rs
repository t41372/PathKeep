//! Failure modes: a locked archive appears readable, startup failures are hidden, or initialization loses its progress signal. Tests: maintenance and launch-recovery suites.
//! Responsibilities: retain initialization and SQLCipher maintenance status until area 5/6.
//! Not responsible for: native archive reads.
use super::*;
use crate::models::ArchiveUpgradeProgress;
/// Initializes the archive schema and returns the resulting status snapshot.
pub fn ensure_archive_initialized(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_archive_initialized_with_progress(paths, config, key, |_| {})
}

/// Initializes the archive schema, streaming first-run upgrade progress as the
/// heavy phases advance, and returns the resulting status snapshot.
///
/// This is the observable twin of [`ensure_archive_initialized`], which
/// delegates here with a no-op callback so the no-callback path is byte-for-byte
/// the original behavior. The heavy work (index-build migrations, the `015`
/// registrable-domain backfill, and the v2→v4 search reprojection) all happens
/// inside the single [`open_archive_connection_reporting`] call — the SAME
/// bootstrap the plain open runs, now threaded so a large first-launch upgrade
/// can drive a calm "Upgrading your archive…" screen instead of an opaque
/// multi-minute busy overlay. A terminal [`ArchiveUpgradeProgress::finished`] is
/// emitted once the archive is fully open.
///
/// Progress is OBSERVATION ONLY: no migration SQL, backfill algorithm, or
/// reprojection logic changes, so the backfill stays keyset-paged + idempotent
/// and a user who quits mid-upgrade still resumes cleanly on the next launch.
pub fn ensure_archive_initialized_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    mut report: F,
) -> Result<ArchiveStatus>
where
    F: FnMut(ArchiveUpgradeProgress),
{
    ensure_paths(paths)?;
    let mut next_config = config.clone();
    next_config.initialized = true;
    save_config(paths, &next_config)?;
    let _connection = open_archive_connection_reporting(paths, &next_config, key, &mut report)?;
    report(ArchiveUpgradeProgress::finished());
    legacy_archive_status(paths, &next_config, key)
}

/// Reports whether the canonical archive exists and is currently readable.
pub fn legacy_archive_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ArchiveStatus> {
    if config.archive_mode == ArchiveMode::Encrypted && key.is_none() {
        let native = crate::vault::live::store(&paths.app_root);
        let last_successful_backup_at = if let Some(store) = &native {
            store.read(vault_store::Db::Archive, |s| Ok(s.query("SELECT finished_at FROM runs WHERE run_type='backup' AND status='success' ORDER BY id DESC LIMIT 1", (), |r| Ok(r.get::<String>(0)?))?.pop()))?
        } else {
            None
        };
        return Ok(ArchiveStatus {
            initialized: config.initialized
                && (native.is_some()
                    || paths.archive_database_path.exists()
                    || paths.app_root.join("archive/history-vault.db").exists()),
            encrypted: true,
            unlocked: native.is_some(),
            database_path: if native.is_some() {
                paths.app_root.join("archive/history-vault.db")
            } else {
                paths.archive_database_path.clone()
            }
            .display()
            .to_string(),
            last_successful_backup_at,
            warning: None,
        });
    }
    ensure_paths(paths)?;
    let opened = (config.initialized && paths.archive_database_path.exists())
        .then(|| open_archive_connection(paths, config, key));
    legacy_archive_status_from_open(paths, config, opened.as_ref())
}

/// Builds the archive status from an open the caller already attempted (`None` when the archive
/// is not initialized), so the app snapshot can reuse that connection for its other reads.
pub(crate) fn legacy_archive_status_from_open(
    paths: &ProjectPaths,
    config: &AppConfig,
    opened: Option<&Result<Connection>>,
) -> Result<ArchiveStatus> {
    let mut status = ArchiveStatus {
        initialized: config.initialized && paths.archive_database_path.exists(),
        encrypted: matches!(config.archive_mode, ArchiveMode::Encrypted),
        unlocked: false,
        database_path: paths.archive_database_path.display().to_string(),
        last_successful_backup_at: None,
        warning: None,
    };

    if !status.initialized {
        return Ok(status);
    }

    match opened.context("an initialized archive needs an open attempt")? {
        Ok(connection) => {
            status.unlocked = true;
            status.last_successful_backup_at = connection
                .query_row(
                    "SELECT finished_at
                     FROM runs
                     WHERE run_type = 'backup'
                       AND status = 'success'
                     ORDER BY id DESC
                     LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
        }
        Err(error) => status.warning = Some(error.to_string()),
    }

    Ok(status)
}
