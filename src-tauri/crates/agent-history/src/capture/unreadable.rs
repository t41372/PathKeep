//! Responsibilities: publish only successfully captured generation gaps using retained metadata.
//! Not responsible for: inventing an identity for a new undecodable source.
use super::BackupOptions;
use crate::{Archive, Artifact, Result, SourceRoot};

pub(super) fn publish(
    archive: &Archive,
    _source: &SourceRoot,
    _artifact: &Artifact,
    source_id: &str,
    revision: i64,
    options: &BackupOptions,
) -> Result<bool> {
    super::check_cancel(options)?;
    archive.publish_generation_gap(source_id, revision, "agent-history-unreadable/1")
}
