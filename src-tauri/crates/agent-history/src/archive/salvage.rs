//! Recovers independently verifiable revisions from a private, read-only evidence copy.
//! A bad ledger or revision cannot authorize publishing unchecked bytes. Destination failures abort.
use super::{
    Archive,
    reader::{VerifiedSlice, inspect_slices},
    revisions::map_revision,
    sources::read_source,
};
use vault_store::{
    CancelToken, Config, Db, Error, OptionalRow, Priority, Result, Session, Store, params,
};
mod metadata;
mod writer;
pub use metadata::Diagnostics;
use writer::Batch;
pub mod loss;
const EVIDENCE: Db = Db::AgentHistoryEvidence;

/// Only damaged source contents can be skipped. Never classify an output operation this way.
pub fn unreadable_input(error: &Error) -> bool {
    match error {
        Error::Open(f) => unreadable_input(&f.cause),
        Error::Operation(cause)
            if matches!(cause.downcast_ref::<crate::Error>(), Some(crate::Error::Integrity(_))) =>
        {
            true
        }
        e if e.invalid_stored_value() => true,
        Error::Corrupt { .. }
        | Error::IntegrityCheck { .. }
        | Error::WrongKey { .. }
        | Error::RetainedBytes { .. }
        | Error::MissingStore(_)
        | Error::JobPanicked { .. } => true,
        Error::Io(e) => matches!(
            e.kind(),
            std::io::ErrorKind::InvalidData
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::NotFound
        ),
        _ => false,
    }
}

fn readable<T>(work: impl FnOnce() -> Result<T>) -> Result<Option<T>> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(Ok(value)) => Ok(Some(value)),
        Ok(Err(e)) if unreadable_input(&e) => Ok(None),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(None), // Turso can assert while stepping a malformed page.
    }
}

fn has_object(s: &Session<'_>, name: &str) -> Result<bool> {
    Ok(readable(|| {
        s.query_one("SELECT count(*) FROM sqlite_schema WHERE name=?1", [name], |r| {
            Ok(r.get::<i64>(0)? != 0)
        })
    })?
    .unwrap_or(false))
}

impl Archive {
    pub fn salvage_evidence(
        &self,
        config: &Config,
        cancel: &CancelToken,
        diag: &mut Diagnostics,
    ) -> Result<()> {
        Store::inspect_existing(config, EVIDENCE, |s| {
            if !has_object(s, "ah_revisions")? {
                return Ok(());
            }
            let has_descriptor = has_object(s, "ah_sources")?;
            let has_slices = has_object(s, "ah_revision_chunks")? && has_object(s, "ah_chunks")?;
            let mut batch = Batch::default();
            let has_index = has_object(s, "ah_revisions_state")?;
            // The state index covers the IDs even when a table leaf is unreadable.
            // If the index is damaged, range probes of the table still reach its other leaves.
            let mut after = 0;
            loop {
                cancel.check()?;
                if !has_index {
                    self.salvage_table_ranges(
                        s,
                        after,
                        cancel,
                        (has_descriptor, has_slices),
                        diag,
                        &mut batch,
                    )?;
                    break;
                }
                let page = readable(|| {
                    s.query("SELECT id FROM ah_revisions INDEXED BY ah_revisions_state WHERE state='complete' AND id>?1 ORDER BY id LIMIT 128",
                    [after],|r|Ok(r.get::<i64>(0)?))
                })?;
                let Some(ids) = page else {
                    self.salvage_table_ranges(
                        s,
                        after,
                        cancel,
                        (has_descriptor, has_slices),
                        diag,
                        &mut batch,
                    )?;
                    break;
                };
                let Some(last) = ids.last().copied() else { break };
                for id in ids {
                    self.salvage_revision(
                        s,
                        id,
                        cancel,
                        (has_descriptor, has_slices),
                        diag,
                        &mut batch,
                    )?;
                }
                after = last;
            }
            // Keep the published canonical head when it survived, parser budgets
            // for large records and attachment bindings between recovered revisions.
            batch.flush(self)?;
            metadata::projection_facts(self, s, cancel, diag)?;
            Ok(())
        })
    }

    fn salvage_table_ranges(
        &self,
        s: &Session<'_>,
        mut after: i64,
        cancel: &CancelToken,
        schema: (bool, bool),
        diag: &mut Diagnostics,
        batch: &mut Batch,
    ) -> Result<()> {
        let (has_descriptor, has_slices) = schema;
        // An independent covering index supplies the upper bound when the last table leaf is damaged.
        let mut last = None;
        for index in [
            "NOT INDEXED",
            "INDEXED BY ah_revisions_digest",
            "INDEXED BY ah_revisions_source_state",
        ] {
            if let Some(name) = index.strip_prefix("INDEXED BY ")
                && !has_object(s, name)?
            {
                continue;
            }
            if let Some(Some(id)) = readable(|| {
                s.query_one(&format!("SELECT max(id) FROM ah_revisions {index}"), (), |r| {
                    Ok(r.get::<Option<i64>>(0)?)
                })
            })? {
                last = Some(id);
                break;
            }
        }
        let Some(last) = last else { return Ok(()) };
        while after < last {
            cancel.check()?;
            let end = last.min(after.saturating_add(256));
            let page = readable(|| {
                s.query(
                    "SELECT id FROM ah_revisions NOT INDEXED WHERE id>?1 AND id<=?2 ORDER BY id",
                    params![after, end],
                    |r| Ok(r.get::<i64>(0)?),
                )
            })?;
            if let Some(ids) = page {
                for id in ids {
                    self.salvage_revision(
                        s,
                        id,
                        cancel,
                        (has_descriptor, has_slices),
                        diag,
                        batch,
                    )?;
                }
            } else {
                for id in after + 1..=end {
                    self.salvage_revision(
                        s,
                        id,
                        cancel,
                        (has_descriptor, has_slices),
                        diag,
                        batch,
                    )?;
                }
            }
            after = end;
        }
        Ok(())
    }

    fn salvage_revision(
        &self,
        s: &Session<'_>,
        id: i64,
        cancel: &CancelToken,
        schema: (bool, bool),
        diag: &mut Diagnostics,
        batch: &mut Batch,
    ) -> Result<()> {
        let (has_descriptor, has_slices) = schema;
        cancel.check()?;
        let Some(Some(info)) = readable(|| {
            s.query_one("SELECT id,source_id,digest,byte_len,captured_at,capture_format,bytes_retained FROM ah_revisions WHERE id=?1 AND state='complete' AND bytes_retained=1",
            [id],map_revision).optional()
        })?
        else {
            return Ok(());
        };
        if info.byte_len > 0 && !has_slices {
            return Ok(());
        }

        let mut descriptor = if has_descriptor {
            metadata::optional(|| read_source(s, &info.source_id)).flatten()
        } else {
            None
        };
        if descriptor.is_none() {
            diag.skipped_records += 1;
        }
        if let Some(descriptor) = descriptor.as_mut()
            && descriptor.relative_path.ends_with("/audit.jsonl")
            && metadata::optional(|| super::sources::retired_audit(s, descriptor)) == Some(true)
        {
            descriptor.kind = "metadata".into();
        }
        let original_head = descriptor.as_ref().and_then(|d| d.latest_revision_id);
        batch.begin(self, &info, descriptor)?;
        let mut ordinal = 0;
        let verified = readable(|| {
            inspect_slices(s, &info, |slice| {
                cancel.check()?;
                batch.slice(self, id, ordinal, slice).map_err(|e| Error::Operation(Box::new(e)))?;
                ordinal += 1;
                Ok(())
            })
        })?;
        if verified.is_none() {
            batch.flush(self)?;
            self.abandon_revision(id).map_err(|e| Error::Operation(Box::new(e)))?;
            while self
                .prune_abandoned_capture_batch(256)
                .map_err(|e| Error::Operation(Box::new(e)))?
                > 0
            {
                cancel.check()?;
            }
            return Ok(());
        }
        batch.finish(self, info, original_head, ordinal)
    }
}
