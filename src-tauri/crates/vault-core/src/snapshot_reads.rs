//! The archive and intelligence reads behind the desktop app snapshot, from one connection each.
//!
//! ## Responsibilities
//! - Open the archive once and the intelligence database once per snapshot, and serve the archive
//!   status, recent runs, recent import batches, semantic-index status and derived-intelligence
//!   status from those two connections.
//!
//! ## Not responsible for
//! - Browser discovery, App Lock and keychain state, or diagnostics; `vault-worker` adds those.
//! - Turning a failed AI or intelligence read into a warning; the worker decides how to degrade.
//!
//! ## Performance notes
//! - The shell asks for a snapshot on start and after most settings changes. Each archive open
//!   derives the SQLCipher key for an encrypted archive (about 40 ms on a fast machine) and the
//!   intelligence open derives it again to attach the archive. The snapshot used to open the
//!   archive three times and the intelligence database twice; now it pays two derivations.

use crate::{
    ai::ai_index_status_from,
    archive::{
        archive_status_from_open, open_archive_connection, open_intelligence_connection,
        recent_runs_from,
    },
    config::{ProjectPaths, ensure_paths},
    intelligence::intelligence_status_from,
    models::{
        AiIndexStatus, AppConfig, ArchiveStatus, BackupRunOverview, ImportBatchOverview,
        IntelligenceStatus,
    },
    takeout::import_batches_from,
};
use anyhow::{Context, Result};

/// What the app snapshot reads from PathKeep's own databases.
#[derive(Debug)]
pub struct SnapshotReads {
    pub archive_status: ArchiveStatus,
    /// Empty unless the archive opened.
    pub recent_runs: Vec<BackupRunOverview>,
    /// Empty unless the archive opened.
    pub recent_import_batches: Vec<ImportBatchOverview>,
    pub ai_status: Result<AiIndexStatus>,
    pub intelligence_status: Result<IntelligenceStatus>,
}

/// Loads every database-backed part of the app snapshot.
///
/// A locked or unreadable archive is not an error here: it shows up as `archive_status.warning`
/// and as errors in the two status fields. A run or import ledger that cannot be decoded is.
pub fn load_snapshot_reads(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<SnapshotReads> {
    ensure_paths(paths)?;
    let initialized = config.initialized && paths.archive_database_path.exists();
    let archive = initialized.then(|| open_archive_connection(paths, config, key));
    let archive_status = archive_status_from_open(paths, config, archive.as_ref())?;
    let (recent_runs, recent_import_batches) = match archive.as_ref() {
        Some(Ok(connection)) => (
            recent_runs_from(connection)
                .context("loading the recent run ledger for the app snapshot")?,
            import_batches_from(connection)
                .context("loading the recent import ledger for the app snapshot")?,
        ),
        _ => (Vec::new(), Vec::new()),
    };
    drop(archive);

    let (ai_status, intelligence_status) = if initialized {
        match open_intelligence_connection(paths, config, key) {
            Ok(connection) => (
                ai_index_status_from(paths, config, &connection),
                intelligence_status_from(paths, &connection),
            ),
            // Both reads fail the same way; each caller-facing status carries the reason.
            Err(error) => {
                let reason = format!("{error:#}");
                (Err(anyhow::anyhow!(reason.clone())), Err(anyhow::anyhow!(reason)))
            }
        }
    } else {
        // Nothing has been derived before onboarding, and opening the intelligence database
        // would create it (and attach an archive that does not exist yet).
        (crate::ai_index_status(paths, config, key), Ok(IntelligenceStatus::default()))
    };

    Ok(SnapshotReads {
        archive_status,
        recent_runs,
        recent_import_batches,
        ai_status,
        intelligence_status,
    })
}
