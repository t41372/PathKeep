//! Responsibilities: share verified prefixes and retire superseded state bytes atomically.
//! Not responsible for: selecting sources or treating parser coverage as raw integrity.
use super::{Archive, RevisionInfo};
use crate::Result;
use vault_store::{Db, Priority, Session, params};

/// Release measurements select one bounded slice size for capture, export and verification.
pub const SLICE_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn is_prefix(
    archive: &Archive,
    old: &RevisionInfo,
    new: i64,
    length: u64,
) -> Result<bool> {
    let info = RevisionInfo {
        id: new,
        source_id: old.source_id.clone(),
        digest: String::new(),
        byte_len: length,
        captured_at: 0,
        capture_format: "native".into(),
        bytes_retained: true,
    };
    Ok(archive.revision_prefix(old, &info)?.0)
}

fn retire_manifest(tx: &Session<'_>, revision: i64) -> vault_store::Result<()> {
    tx.execute(
        "
        INSERT OR IGNORE INTO ah_chunk_garbage(hash) SELECT chunk_hash FROM ah_revision_chunks
        WHERE revision_id=?1",
        [revision],
    )?;
    tx.execute("DELETE FROM ah_revision_chunks WHERE revision_id=?1", [revision])?;
    Ok(())
}

pub(super) fn share_prefix(
    tx: &Session<'_>,
    previous: i64,
    selected: i64,
) -> vault_store::Result<()> {
    // Flatten every incoming edge in the publication transaction. Readers look up
    // this direct manifest each time they load a slice, including already-open readers.
    let manifest: Option<i64> = tx.query_one(
        "SELECT manifest_revision_id FROM ah_revisions WHERE id=?1",
        [selected],
        |r| Ok(r.get(0)?),
    )?;
    let target = manifest.unwrap_or(selected);
    tx.execute(
        "UPDATE ah_revisions SET manifest_revision_id=?1 WHERE manifest_revision_id=?2 OR id=?2",
        params![target, previous],
    )?;
    retire_manifest(tx, previous)
}

pub(super) fn retain_latest_state(
    tx: &Session<'_>,
    source: &str,
    selected: i64,
) -> vault_store::Result<()> {
    tx.execute(
        "
        INSERT OR IGNORE INTO ah_chunk_garbage(hash) SELECT m.chunk_hash FROM ah_revision_chunks m
        JOIN ah_revisions r ON r.id=m.revision_id WHERE r.source_id=?1 AND r.id<>?2 AND
        r.state='complete'",
        params![source, selected],
    )?;
    tx.execute(
        "
        DELETE FROM ah_revision_chunks WHERE revision_id IN (SELECT id FROM ah_revisions WHERE
        source_id=?1 AND id<>?2 AND state='complete')",
        params![source, selected],
    )?;
    tx.execute(
        "UPDATE ah_revisions SET bytes_retained=0 WHERE source_id=?1 AND id<>?2 AND
        state='complete'",
        params![source, selected],
    )?;
    Ok(())
}

impl Archive {
    pub(super) fn reclaim_slices(&self) -> Result<()> {
        loop {
            let count = self.store.write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
                let hashes = tx.query(
                    "SELECT hash FROM ah_chunk_garbage ORDER BY hash LIMIT 128",
                    (),
                    |r| Ok(r.get::<String>(0)?),
                )?;
                for hash in &hashes {
                    tx.execute(
                        "DELETE FROM ah_chunks WHERE hash=?1 AND NOT EXISTS(SELECT 1 FROM
                        ah_revision_chunks WHERE chunk_hash=?1)",
                        [hash.as_str()],
                    )?;
                    tx.execute("DELETE FROM ah_chunk_garbage WHERE hash=?1", [hash.as_str()])?;
                }
                Ok(hashes.len())
            })?;
            if count == 0 {
                return Ok(());
            }
        }
    }
}
