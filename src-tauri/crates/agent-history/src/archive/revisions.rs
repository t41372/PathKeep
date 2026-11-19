//! Responsibilities: retain immutable raw revisions and atomically publish validated captures.
//! Not responsible for: reading live sources, parsing their contents or presenting revisions.
use super::{Archive, RawReader};
use crate::{
    Error, Result,
    model::{Artifact, SourceRoot},
};
use serde::Serialize;
use std::io;
use vault_store::{Db, OptionalRow, Priority, params};

/// A complete retained revision. IDs are serialized as strings for JavaScript safety.
#[derive(Clone, Debug, Serialize)]
pub struct RevisionInfo {
    #[serde(serialize_with = "serialize_id")]
    pub id: i64,
    pub source_id: String,
    pub digest: String,
    pub byte_len: u64,
    pub captured_at: i64,
    pub capture_format: String,
    pub bytes_retained: bool,
}

fn serialize_id<S: serde::Serializer>(
    value: &i64,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

fn serialize_optional_id<S: serde::Serializer>(
    value: &Option<i64>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&value.to_string()),
        None => serializer.serialize_none(),
    }
}

/// Source identity is independent of native session IDs, which can repeat across roots.
#[derive(Clone, Debug, Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub harness: String,
    pub root: String,
    pub relative_path: String,
    pub kind: String,
    pub retention: String,
    pub last_seen_at: Option<i64>,
    pub missing_since: Option<i64>,
    #[serde(serialize_with = "serialize_optional_id")]
    pub latest_revision_id: Option<i64>,
    pub observed_at: i64,
}

/// A duplicate capture can reuse bytes while still changing a source's observed head.
#[derive(Clone, Debug, Serialize)]
pub struct RevisionOutcome {
    #[serde(serialize_with = "serialize_id")]
    pub revision_id: i64,
    pub unchanged: bool,
}

impl Archive {
    /// Reserves an unpublished revision. Only `finish_revision` may make it readable.
    pub fn begin_revision(
        &self,
        source_id: &str,
        source: &SourceRoot,
        artifact: &Artifact,
    ) -> Result<i64> {
        self.begin_revision_with_format(source_id, source, artifact, "native")
    }

    /// Records whether evidence is exact native bytes or a documented logical snapshot.
    pub fn begin_revision_with_format(
        &self,
        source_id: &str,
        source: &SourceRoot,
        artifact: &Artifact,
        capture_format: &str,
    ) -> Result<i64> {
        if source_id.is_empty()
            || source_id.len() > 256
            || !["native", "pathkeep-sqlite-evidence-v2"].contains(&capture_format)
        {
            return Err(Error::InvalidInput("invalid source identity or capture format".into()));
        }
        self.register_source(source_id, source, artifact)?;
        let id = source_id.to_owned();
        let format = capture_format.to_owned();
        let codec = if capture_format == "native"
            && (artifact.relative_path.ends_with(".zst")
                || artifact.relative_path.ends_with(".zstd"))
        {
            "native"
        } else {
            "zstd"
        };
        let now = chrono::Utc::now().timestamp_millis();
        Ok(self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            tx.insert(
                "
                INSERT INTO ah_revisions(source_id, captured_at, state, capture_format, codec)
                VALUES(?1, ?2, 'staged' , ?3, ?4)",
                params![id, now, format, codec],
            )?
            .ok_or_else(|| integrity("revision insertion did not produce an identity"))
        })?)
    }

    /// Freezes and verifies the complete manifest before publishing it. A caller-supplied
    /// digest is checked against stored bytes, not trusted as proof of a complete write.
    pub fn finish_revision(
        &self,
        revision: i64,
        digest: &str,
        byte_len: u64,
    ) -> Result<RevisionOutcome> {
        if byte_len > i64::MAX as u64
            || digest.len() != 64
            || !digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::InvalidInput("invalid revision size or SHA-256 digest".into()));
        }
        let digest = digest.to_ascii_lowercase();
        let expected = digest.clone();
        let info = self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let values = tx.query_one(
                "
                SELECT source_id, staged_byte_len, captured_at, capture_format, state,
                next_ordinal FROM ah_revisions WHERE id=?1",
                [revision],
                |row| {
                    Ok((
                        row.get::<String>(0)?,
                        row.get::<i64>(1)?,
                        row.get::<i64>(2)?,
                        row.get::<String>(3)?,
                        row.get::<String>(4)?,
                        row.get::<i64>(5)?,
                    ))
                },
            )?;
            if values.4 != "staged" || values.1 != byte_len as i64 {
                return Err(integrity("revision size or state does not match its staged bytes"));
            }
            tx.execute("UPDATE ah_revisions SET state='sealing' WHERE id=?1", [revision])?;
            Ok((
                RevisionInfo {
                    id: revision,
                    source_id: values.0,
                    digest: expected,
                    byte_len,
                    captured_at: values.2,
                    capture_format: values.3,
                    bytes_retained: true,
                },
                values.5,
            ))
        })?;
        let mut reader = RawReader::new(self, info.0.clone());
        io::copy(&mut reader, &mut io::sink())?;
        let source_id = info.0.source_id;
        let format = info.0.capture_format;
        let source = self.source(&source_id)?;
        let previous = self.latest_revision(&source_id)?;
        let unchanged = previous
            .as_ref()
            .is_some_and(|previous| previous.digest == digest && previous.capture_format == format);
        let prefix = if source.retention == "appended" {
            match &previous {
                Some(old) if old.byte_len < byte_len => {
                    super::retention::is_prefix(self, old, revision, byte_len)?
                }
                _ => false,
            }
        } else {
            false
        };
        let _publication = self
            .publication_gate
            .lock()
            .map_err(|_| Error::Integrity("archive publication lock was poisoned".into()))?;
        if self.latest_revision(&source_id)?.as_ref().map(|r| r.id)
            != previous.as_ref().map(|r| r.id)
        {
            return Err(Error::SourceChanged(
                "source head changed while its prefix was being verified".into(),
            ));
        }
        if source.kind == "transcript" && !unchanged {
            // Hot coverage commits first. A crash before the cold head changes is
            // conservative; the reverse order could falsely claim complete search
            // for bytes no parser has seen. Projection publication shares this lock.
            self.set_coverage(&source_id, false)?;
        }
        let retention = source.retention;
        let retires_slices = prefix || retention == "state";
        let now = chrono::Utc::now().timestamp_millis();
        let outcome = self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let state: String =
                tx.query_one("SELECT state FROM ah_revisions WHERE id=?1", [revision], |row| {
                    Ok(row.get(0)?)
                })?;
            if state != "sealing" {
                return Err(integrity("revision changed while its manifest was being verified"));
            }
            let previous: Option<i64> = tx.query_one(
                "
                SELECT latest_revision_id FROM ah_sources WHERE id=?1",
                [source_id.as_str()],
                |row| Ok(row.get(0)?),
            )?;
            let existing: Option<i64> = tx
                .query_one(
                    "
                SELECT id FROM ah_revisions WHERE source_id=?1 AND digest=?2 AND capture_format=?3
                AND state='complete' AND bytes_retained=1 ORDER BY id LIMIT 1",
                    params![source_id.clone(), digest.clone(), format],
                    |row| Ok(row.get(0)?),
                )
                .optional()?;
            let selected = existing.unwrap_or(revision);
            if existing.is_some() {
                tx.execute("UPDATE ah_revisions SET state='duplicate' WHERE id=?1", [revision])?;
            } else {
                tx.execute(
                    "
                    UPDATE ah_revisions SET state='complete', digest=?1, byte_len=?2 WHERE id=?3",
                    params![digest, byte_len as i64, revision],
                )?;
            }
            tx.execute(
                "UPDATE ah_sources SET latest_revision_id=?1, observed_at=?2 WHERE id=?3",
                params![selected, now, source_id.clone()],
            )?;
            if prefix
                && let Some(previous) = previous
                && previous != selected
            {
                super::retention::share_prefix(tx, previous, selected)?;
            }
            if retention == "state" {
                super::retention::retain_latest_state(tx, &source_id, selected)?;
            }
            Ok(RevisionOutcome { revision_id: selected, unchanged: previous == Some(selected) })
        })?;
        drop(_publication);
        if retires_slices {
            // Limit garbage during large imports. Backup checks the durable queue again at
            // completion so a collection failure cannot undo or misreport this publication.
            let _ = self.reclaim_slices();
        }
        Ok(outcome)
    }

    /// Discards only an unfinished attempt; successful evidence cannot be erased by this call.
    pub fn abandon_revision(&self, revision: i64) -> Result<()> {
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            tx.execute(
                "
                UPDATE ah_revisions SET state='abandoned' WHERE id=?1 AND state IN ('staged',
                'sealing' )",
                [revision],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// Retrieves a complete revision without exposing unfinished staging state.
    pub fn revision(&self, revision: i64) -> Result<RevisionInfo> {
        self.store
            .read(Db::AgentHistoryEvidence, move |tx| {
                tx.query_one(
                    "SELECT id, source_id, digest, byte_len, captured_at, capture_format,
                bytes_retained FROM ah_revisions WHERE id=?1 AND state='complete'",
                    [revision],
                    map_revision,
                )
                .optional()
            })?
            .ok_or_else(|| Error::NotFound("complete revision".into()))
    }

    /// Returns the last observed content, including a deliberate return to an older revision.
    pub fn latest_revision(&self, source_id: &str) -> Result<Option<RevisionInfo>> {
        let source_id = source_id.to_owned();
        Ok(self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one(
                "
                SELECT r.id, r.source_id, r.digest, r.byte_len, r.captured_at, r.capture_format,
                r.bytes_retained FROM ah_sources s JOIN ah_revisions r ON
                r.id=s.latest_revision_id WHERE s.id=?1 AND r.state='complete'",
                [source_id],
                map_revision,
            )
            .optional()
        })?)
    }

    pub(super) fn recover_unfinished_revisions(&self) -> Result<()> {
        loop {
            let changed = self.store.write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
                tx.execute(
                    "UPDATE ah_revisions SET state='abandoned' WHERE id IN (SELECT id FROM
                    ah_revisions WHERE state IN ('staged', 'sealing' ) LIMIT 128)",
                    (),
                )
            })?;
            if changed == 0 {
                return Ok(());
            }
        }
    }
}

fn map_revision(row: &vault_store::Row) -> vault_store::Result<RevisionInfo> {
    let length: i64 = row.get(3)?;
    if length < 0 {
        return Err(integrity("revision byte length is negative"));
    }
    Ok(RevisionInfo {
        id: row.get(0)?,
        source_id: row.get(1)?,
        digest: row.get(2)?,
        byte_len: length as u64,
        captured_at: row.get(4)?,
        capture_format: row.get(5)?,
        bytes_retained: row.get::<i64>(6)? != 0,
    })
}

pub(super) fn integrity(message: &str) -> vault_store::Error {
    vault_store::Error::Operation(Box::new(Error::Integrity(message.into())))
}
