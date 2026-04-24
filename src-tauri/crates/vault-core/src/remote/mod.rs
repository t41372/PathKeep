//! Remote-backup bundle orchestration.
//!
//! Remote backup is a portable copy of the local archive state, not a second
//! source of truth. The public surface stays deliberately small so Tauri and
//! worker callers only need to preview, upload, or verify a bundle.
//!
//! ## Responsibilities
//! - Build remote-backup zip bundles from the local archive and source-evidence
//!   stores.
//! - Preserve the `pathkeep.remote-backup.v1` manifest and checksum contract.
//! - Shape explicit upload commands and curl-backed upload execution.
//! - Verify required entries, manifest integrity, checksums, and SQLite restore
//!   readiness for downloaded bundles.
//!
//! ## Not responsible for
//! - Scheduling remote uploads after local backups.
//! - Replacing local archive initialization, restore planning, or key management.
//! - Abstracting over every remote storage provider; S3-compatible URLs are the
//!   supported contract.
//!
//! ## Dependencies
//! - `archive` exports consistent SQLite snapshots for bundle payloads.
//! - `config` supplies local paths and persists upload state.
//! - `models::remote` defines the frontend-facing DTOs.
//! - `zip` writes and reads the portable bundle container.
//!
//! ## Performance notes
//! Bundle payloads can include multi-GB SQLite files on large archives. File
//! entries are streamed through the zip writer and SHA-256 hasher in bounded
//! chunks so remote backup does not require loading whole databases into memory.

mod bundle;
mod manifest;
mod transfer;
mod verify;

pub use transfer::{preview_remote_backup, run_remote_backup};
pub use verify::verify_remote_backup;

#[cfg(test)]
mod tests;
