//! Recovers independently verifiable revisions from a private, read-only evidence copy.
//! A bad ledger or revision cannot authorize publishing unchecked bytes. Destination failures abort.
use super::{Archive, reader::inspect_bytes, revisions::map_revision, sources::read_source};
use vault_store::{
    CancelToken, Config, Db, Error, OptionalRow, Priority, Result, Session, Store, params,
};
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
    pub fn salvage_evidence(&self, config: &Config, cancel: &CancelToken) -> Result<()> {
        // Load the readable mirror first. Readable canonical marks below take precedence.
        let mirror = Store::inspect_existing(config, Db::AgentHistoryReadModel, |s| {
            self.salvage_marks(s, cancel)
        });
        if let Err(e) = mirror
            && !unreadable_input(&e)
        {
            return Err(e);
        }
        Store::inspect_existing(config, EVIDENCE, |s| {
            if !has_object(s, "ah_revisions")? {
                return Ok(());
            }
            let has_descriptor = has_object(s, "ah_sources")?;
            let has_slices = has_object(s, "ah_revision_chunks")? && has_object(s, "ah_chunks")?;
            let has_index = has_object(s, "ah_revisions_state")?;
            // The state index covers the IDs even when a table leaf is unreadable.
            // If the index is damaged, range probes of the table still reach its other leaves.
            let mut after = 0;
            loop {
                cancel.check()?;
                if !has_index {
                    self.salvage_table_ranges(s, after, cancel, has_descriptor, has_slices)?;
                    break;
                }
                let page = readable(|| {
                    s.query("SELECT id FROM ah_revisions INDEXED BY ah_revisions_state WHERE state='complete' AND id>?1 ORDER BY id LIMIT 128",
                    [after],|r|Ok(r.get::<i64>(0)?))
                })?;
                let Some(ids) = page else {
                    self.salvage_table_ranges(s, after, cancel, has_descriptor, has_slices)?;
                    break;
                };
                let Some(last) = ids.last().copied() else { break };
                for id in ids {
                    self.salvage_revision(s, id, cancel, has_descriptor, has_slices)?;
                }
                after = last;
            }
            // Keep the published canonical head when it survived, parser budgets
            // for large records and attachment bindings between recovered revisions.
            self.salvage_projection_facts(s, cancel)?;
            self.salvage_marks(s, cancel)?;
            Ok(())
        })
    }

    fn salvage_marks(&self, s: &Session<'_>, cancel: &CancelToken) -> Result<()> {
        if !has_object(s, "ah_user_state")? {
            return Ok(());
        }
        let mut after = String::new();
        loop {
            cancel.check()?;
            let Some(ids) = readable(|| {
                s.query("SELECT conversation_id FROM ah_user_state WHERE conversation_id>?1 ORDER BY conversation_id LIMIT 128",[after.as_str()],|r|Ok(r.get::<String>(0)?))
            })?
            else {
                break;
            };
            let Some(last) = ids.last() else { break };
            after = last.clone();
            for id in ids {
                let row = readable(|| {
                    s.query_one("SELECT starred,put_away,trashed_at,title FROM ah_user_state WHERE conversation_id=?1",[id.as_str()],|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<Option<i64>>(2)?,r.get::<Option<String>>(3)?)))
                })?;
                if let Some((star, away, trash, title)) = row {
                    self.store.write(EVIDENCE, Priority::Batch, move |s| {
                        s.execute(
                            "INSERT OR REPLACE INTO ah_user_state VALUES(?1,?2,?3,?4,?5)",
                            params![id, star, away, trash, title],
                        )?;
                        Ok(())
                    })?;
                }
            }
        }
        Ok(())
    }

    fn salvage_projection_facts(&self, source: &Session<'_>, cancel: &CancelToken) -> Result<()> {
        // Only fixed native tables and predicates enter these statements. The
        // destination predicate excludes facts whose referenced revisions were lost.
        for (table, columns, predicate) in [
            (
                "ah_parse_reports",
                "revision_id,parser_version,report_json",
                "EXISTS(SELECT 1 FROM ah_revisions WHERE id=?1)",
            ),
            (
                "ah_source_conversations",
                "source_id,conversation_id,is_child,revision_id",
                "EXISTS(SELECT 1 FROM ah_revisions WHERE id=?4 AND source_id=?1)",
            ),
            (
                "ah_reference_bindings",
                "id,parent_revision,reference_key,target_revision,observed_at",
                "EXISTS(SELECT 1 FROM ah_revisions WHERE id=?2) AND EXISTS(SELECT 1 FROM ah_revisions WHERE id=?4)",
            ),
        ] {
            if !readable(|| {
                source.query_one("SELECT count(*) FROM sqlite_schema WHERE name=?1", [table], |r| {
                    Ok(r.get::<i64>(0)? != 0)
                })
            })?
            .unwrap_or(false)
            {
                continue;
            }
            let select = format!("SELECT {columns} FROM {table} WHERE rowid=?1");
            let placeholders = (1..=columns.split(',').count())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let insert = format!(
                "INSERT OR REPLACE INTO {table}({columns}) SELECT {placeholders} WHERE {predicate}"
            );
            let mut after = 0;
            loop {
                cancel.check()?;
                let Some(ids) = readable(|| {
                    source.query(
                        &format!(
                            "SELECT rowid FROM {table} WHERE rowid>?1 ORDER BY rowid LIMIT 128"
                        ),
                        [after],
                        |r| Ok(r.get::<i64>(0)?),
                    )
                })?
                else {
                    break;
                };
                let Some(last) = ids.last().copied() else { break };
                after = last;
                for id in ids {
                    let Some(values) = readable(|| {
                        source.query_one(&select, [id], |r| {
                            (0..r.column_count())
                                .map(|i| Ok(r.get_value(i)?))
                                .collect::<Result<Vec<_>>>()
                        })
                    })?
                    else {
                        continue;
                    };
                    if table == "ah_parse_reports"
                        && !matches!(&values[2],vault_store::Value::Text(json) if serde_json::from_str::<serde_json::Value>(json).is_ok())
                    {
                        continue;
                    }
                    let insert = insert.clone();
                    self.store.write(EVIDENCE, Priority::Batch, move |tx| {
                        tx.execute(&insert, values)?;
                        Ok(())
                    })?;
                }
            }
        }
        Ok(())
    }

    fn salvage_table_ranges(
        &self,
        s: &Session<'_>,
        mut after: i64,
        cancel: &CancelToken,
        has_descriptor: bool,
        has_slices: bool,
    ) -> Result<()> {
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
                    self.salvage_revision(s, id, cancel, has_descriptor, has_slices)?;
                }
            } else {
                for id in after + 1..=end {
                    self.salvage_revision(s, id, cancel, has_descriptor, has_slices)?;
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
        has_descriptor: bool,
        has_slices: bool,
    ) -> Result<()> {
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
        if readable(|| inspect_bytes(s, &info, |_| cancel.check()))?.is_none() {
            return Ok(());
        }
        let descriptor = if has_descriptor {
            readable(|| read_source(s, &info.source_id))?.flatten()
        } else {
            None
        };
        let source = info.source_id.clone();
        let date = info.captured_at;
        let format = info.capture_format.clone();
        let original_head = descriptor.as_ref().and_then(|d| d.latest_revision_id);
        self.store.write(EVIDENCE,Priority::Batch,move|tx|{
            if let Some(d)=descriptor {
                tx.execute("INSERT OR IGNORE INTO ah_sources(id,harness,root,relative_path,kind,retention,observed_at,last_seen_at,missing_since) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![source.clone(),d.harness,d.root,d.relative_path,d.kind,d.retention,d.observed_at,d.last_seen_at,d.missing_since])?;
            } else {
                // Preserve verified raw bytes by source ID. Missing provenance cannot grant
                // filesystem access or claim a transcript that a parser can publish.
                tx.execute("INSERT OR IGNORE INTO ah_sources(id,harness,root,relative_path,kind,retention,observed_at) VALUES(?1,'','','','metadata','state',?2)",params![source.clone(),date])?;
            }
            tx.execute("INSERT INTO ah_revisions(id,source_id,captured_at,state,capture_format,codec) VALUES(?1,?2,?3,'staged',?4,'zstd')",params![id,source,date,format])?;
            Ok(())
        })?;
        let mut ordinal = 0;
        inspect_bytes(s, &info, |bytes| {
            cancel.check()?;
            self.append_chunk(id, ordinal, bytes).map_err(|e| Error::Operation(Box::new(e)))?;
            ordinal += 1;
            Ok(())
        })?;
        self.store.write(EVIDENCE,Priority::Batch,move|tx|{
            tx.execute("UPDATE ah_revisions SET state='complete',digest=?1,byte_len=?2 WHERE id=?3",params![info.digest,info.byte_len as i64,id])?;
            tx.execute("UPDATE ah_sources SET latest_revision_id=?1 WHERE id=?2 AND (latest_revision_id IS NULL OR latest_revision_id IS NOT ?3)",params![id,info.source_id,original_head])?;
            Ok(())
        })?;
        Ok(())
    }
}
