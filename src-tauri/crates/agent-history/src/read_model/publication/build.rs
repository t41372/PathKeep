//! Builds private source pages from retained evidence. Visibility is owned by the head-switch transaction.
use super::{
    pages::{self, Buffer},
    *,
};
use crate::{
    Archive, ParseContext,
    archive::{ProjectionAttempt, projection::require_work},
};
use sha2::{Digest, Sha256};
use vault_store::{OptionalRow, Priority, Session, params};

pub(super) fn require_staging(tx: &Session<'_>, owner: &str) -> vault_store::Result<()> {
    tx.query_one(
        "SELECT 1 FROM ah_reader_version v WHERE v.token=?1 AND ((v.state='staging' AND
                 EXISTS(SELECT 1 FROM ah_projection_work w WHERE w.session_id=v.source AND
                 w.projection_id=v.attempt)) OR (v.state='repairing' AND EXISTS(SELECT 1 FROM
                 ah_sessions s WHERE s.id=v.source AND s.projection_id=v.attempt)))",
        [owner],
        |_| Ok(()),
    )
}

pub(super) fn key(id: &str) -> String {
    hex::encode(Sha256::digest(id.as_bytes()))
}

impl Entry {
    pub(super) fn interpret(
        locator: RecordLocator,
        ctx: &RecordContext,
        value: Option<&serde_json::Value>,
        previous: &mut Option<String>,
        run_start: &mut u64,
    ) -> Result<Self> {
        let event = value.map(|v| ctx.interpret(locator.ordinal, v.clone())).transpose()?;
        let boundary = event.as_ref().and_then(|r| r.boundary.clone());
        let visible = boundary.is_some() || event.as_ref().is_some_and(|r| !r.parts.is_empty());
        let (id, parent, kind, role, timestamp) = event
            .map(|r| {
                (
                    r.event.native_id,
                    r.event.parent_id,
                    r.event.kind,
                    r.event.role,
                    r.event.timestamp,
                )
            })
            .unwrap_or_default();
        let first_kept = value.and_then(|v| v["firstKeptEntryId"].as_str()).map(str::to_owned);
        let tokens_before = boundary.as_ref().and_then(|b| b.tokens_before);
        if parent.is_none() || &parent != previous {
            *run_start = locator.ordinal.0
        }
        let declared_root = matches!(ctx, RecordContext::Pi { version: Some(1), .. })
            || value.is_some_and(|v| v.get("parentId").is_some_and(serde_json::Value::is_null));
        previous.clone_from(&id);
        Ok(Self {
            visible,
            boundary,
            locator,
            context: ctx.clone(),
            id,
            parent,
            run_start: *run_start,
            declared_root,
            kind,
            role,
            timestamp,
            first_kept,
            tokens_before,
        })
    }
}
impl Archive {
    /// A local reader failure leaves a rebuildable member. The parser has already
    /// saved its report, and capture can still publish the native session and events.
    pub(crate) fn stage_capture_reader(
        &self,
        attempt: &ProjectionAttempt,
        context: &ParseContext,
        job: &JobControl,
    ) -> Result<crate::archive::projection::ReaderStage> {
        use crate::archive::projection::ReaderStage;
        match self.stage_reader(attempt, context, job) {
            Ok(()) => Ok(ReaderStage::Ready),
            Err(error) if can_defer_reader_work(&error) => {
                eprintln!(
                    "Conversation reader build deferred for source {}: {error}",
                    attempt.source_id
                );
                Ok(ReaderStage::AwaitingRebuild)
            }
            Err(error) => Err(error),
        }
    }
    /// Builds a private source version. A replacement parser or a newer raw head can still veto publication.
    pub fn stage_reader(
        &self,
        attempt: &ProjectionAttempt,
        context: &ParseContext,
        job: &JobControl,
    ) -> Result<()> {
        let _lease = self.jobs.track(&job.cancel)?;
        let revision = self.revision(attempt.revision_id)?;
        let source = self.source(&attempt.source_id)?;
        if revision.source_id != source.id || source.latest_revision_id != Some(revision.id) {
            return Err(Error::StaleCursor);
        }
        let record_limit = context.max_record_bytes as i64;
        let decoded_limit = job.max_decoded_bytes.min(i64::MAX as u64) as i64;
        let probe = attempt.clone();
        let digest = revision.digest.clone();
        let ready=self.store.read(DB,move|tx|{
            require_work(tx,&probe)?;
            tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_version WHERE attempt=?1 AND source=?2 AND revision=?3 AND digest=?4 AND state='ready')",
                params![probe.id,probe.source_id,probe.revision_id,digest],|r|Ok(r.get::<i64>(0)?!=0))
        })?;
        if ready {
            return Ok(());
        }
        let _guard = self.jobs.enter_scan(&job.cancel)?;
        let parser = attempt.clone();
        let token = self.store.write(DB, Priority::Batch, move |tx| {
            require_work(tx, &parser)?;
            let prior = tx
                .query_one(
                    "SELECT state FROM ah_reader_version WHERE attempt=?1 AND source=?2 AND revision=?3 AND digest=?4 AND state!='retired'",
                    params![parser.id,parser.source_id.clone(),revision.id,revision.digest.clone()],
                    |r| Ok(r.get::<String>(0)?),
                )
                .optional()?;
            if prior.as_deref() == Some("ready") {
                return Ok(None);
            }
            if prior.is_some() {
                return Err(crate::query::operation(Error::SourceChanged(
                    "reader build already in progress".into(),
                )));
            }
            let token: String =
                tx.query_one("SELECT lower(hex(randomblob(16)))", (), |r| Ok(r.get(0)?))?;
            tx.execute(
                "INSERT INTO ah_reader_version(token,attempt,source,revision,digest,byte_len,codec,renderer,records,selective,state,record_limit,decoded_limit,max_frame,format_version)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0,0,'staging',?9,?10,0,NULL)",
                params![
                    token.clone(),
                    parser.id,
                    parser.source_id,
                    revision.id,
                    revision.digest,
                    revision.byte_len as i64,
                    CODEC,
                    RENDERER,
                    record_limit,
                    decoded_limit
                ],
            )?;
            Ok(Some(token))
        })?;
        let Some(token) = token else { return Ok(()) };
        let (count, max_frame, format_version) = match self
            .build_reader_pages(&token, attempt, context, job)
        {
            Ok(built) => built,
            Err(error) => {
                self.store.write(DB, Priority::Batch, move |tx| {
                    tx.execute("UPDATE ah_reader_version SET state='retired' WHERE token=?1 AND state='staging'", [token])?;
                    Ok(())
                })?;
                return Err(error);
            }
        };
        self.store.write(DB, Priority::Batch, move |tx| {
            require_staging(tx, &token)?;
            tx.execute(
                "UPDATE ah_reader_version SET
                 records=?2,selective=?3,state='ready',max_frame=?4,format_version=?5 WHERE
                 token=?1",
                params![
                    token.clone(),
                    count as i64,
                    i64::from(max_frame > REPLAY_LIMIT),
                    max_frame as i64,
                    format_version
                ],
            )?;
            manifest::sign_version(tx, &token)
        })?;
        Ok(())
    }
    fn build_reader_pages(
        &self,
        token: &str,
        attempt: &ProjectionAttempt,
        context: &ParseContext,
        job: &JobControl,
    ) -> Result<(u64, u64, Option<String>)> {
        let mut outline = Buffer::new(OUTLINE, pages::META_BYTES);
        let mut visibility = visible::Builder::default();
        let mut lookup = super::lookup::SortedRuns::default();
        let unreadable = attempt.parser_version.starts_with("agent-history-unreadable/");
        let mut count = 0;
        let mut max_frame = 0;
        let mut format_version = None;
        let mut pi = false;
        let mut last_entry = None;
        let mut last_visible_orphan = None;
        let mut header = None;
        let mut first_object = true;
        let mut previous = None;
        let mut run_start = 0;
        if !unreadable {
            let mut frames = Buffer::new(FRAMES, pages::META_BYTES);
            #[cfg(test)]
            let scan_start = std::time::Instant::now();
            let result = self.scan_revision_with_frames(
                attempt.revision_id,
                context,
                job,
                &mut |locator, ctx, value| {
                    if first_object && value.is_some() {
                        if matches!(ctx, RecordContext::Pi { version: Some(_), .. })
                            && value.is_some_and(|v| v["type"] == "session")
                        {
                            header = Some(locator.ordinal.0);
                        }
                        first_object = false;
                        format_version = match ctx {
                            RecordContext::Pi { version, .. }
                            | RecordContext::Deepseek { version } => version.map(|v| v.to_string()),
                            _ => None,
                        };
                    }
                    pi = matches!(ctx, RecordContext::Pi { .. });
                    let entry =
                        Entry::interpret(locator, ctx, value, &mut previous, &mut run_start)?;
                    let n = entry.locator.ordinal.0;
                    if pi && entry.id.is_none() && entry.visible {
                        last_visible_orphan = Some(n);
                    }
                    if pi && value.is_some() && Some(n) != header {
                        last_entry = Some(n);
                    }
                    if let Some(id) = &entry.id {
                        lookup.push(self, token, id, n * 2 + 1)?
                    }
                    if pi && let Some(parent) = &entry.parent {
                        lookup.push(self, token, parent, n * 2)?
                    }
                    visibility.add(self, token, &entry)?;
                    outline.push(self, token, &entry)
                },
                &mut |frame| {
                    max_frame = max_frame.max(frame.decoded_end.0 - frame.start.decoded.0);
                    frames.push(self, token, &frame)
                },
            );
            frames.flush(self, token)?;
            match result {
                Ok(report) => {
                    #[cfg(test)]
                    super::measure::measured_scan(&report, scan_start.elapsed());
                    count = report.physical_records;
                    max_frame = report.max_frame_bytes;
                }
                Err(Error::NativeTransport(_)) => {
                    // A damaged transport suffix can leave a proven native prefix.
                    // A resource limit aborts the attempt and cannot publish a shorter head.
                    count = outline.len();
                }
                Err(error) => return Err(error),
            }
        }
        let (blocks, visible, nonempty) = visibility.finish(self, token)?;
        outline.flush(self, token)?;

        let owner = token.to_owned();
        self.store.write(DB, Priority::Batch, move |tx| {
            tx.execute(
                "UPDATE ah_reader_version SET rank_pages=?1,visible_total=?2,nonempty_blocks=?4,records=?5 WHERE token=?3",
                params![blocks,visible,owner,nonempty,count as i64],
            )
        })?;
        lookup.finish(self, token, job, pi, header, (last_entry, last_visible_orphan))?;
        if max_frame > REPLAY_LIMIT {
            let mut rendered = Buffer::new(RENDERED, pages::TEXT_BYTES);
            let scan = self.scan_revision_admitted(
                attempt.revision_id,
                context,
                job,
                &mut |loc, ctx, value| {
                    let row = DisplayRow::interpret(&attempt.source_id, &loc, ctx, value.cloned())?;
                    rendered.push(self, token, &row)
                },
            );
            match scan {
                Ok(_) => {}
                Err(Error::NativeTransport(_)) if rendered.len() == count => {}
                Err(error) => return Err(error),
            }
            rendered.flush(self, token)?;
        }
        let owner = token.to_owned();
        self.store.write(DB,Priority::Batch,move|tx|tx.execute("UPDATE ah_reader_version SET pieces=(SELECT count(*) FROM ah_reader_page WHERE owner=?1) WHERE token=?1",[owner]))?;
        Ok((count, max_frame, format_version))
    }

    /// Regenerates pages for the existing interpretation, including an older retained
    /// generation. No live root, backup run, mark queue or search writer is involved.
    pub fn rebuild_reader_catalog(&self, job: &JobControl) -> Result<u64> {
        let _guard = self.jobs.enter_scan(&job.cancel)?;
        let mut conversation = String::new();
        let mut count = 0;
        loop {
            let after = conversation.clone();
            let next = self.store.read(DB, move |tx| {
                tx.query_one(
                    "SELECT conversation_id FROM ah_sessions WHERE conversation_id>?1
                     ORDER BY conversation_id LIMIT 1",
                    [after],
                    |row| Ok(row.get::<String>(0)?),
                )
                .optional()
            })?;
            let Some(id) = next else { break };
            conversation = id;
            count += self.rebuild_conversation_versions(&conversation, job)?;
            let _guard =
                self.publication_gate.lock().map_err(|_| pages::invalid("publication lock"))?;
            // A damaged dependency in another group stays parked until its turn.
            self.recover_reader_heads(&job.cancel)?;
        }
        {
            let _guard =
                self.publication_gate.lock().map_err(|_| pages::invalid("publication lock"))?;
            self.store.write(DB, Priority::Batch, |tx| {
                let inserted = tx.execute(
                    "INSERT OR IGNORE INTO ah_reader_dirty
                SELECT p.conversation,p.ticket FROM ah_reader_parked p
                UNION ALL SELECT h.conversation,lower(hex(randomblob(16))) FROM ah_reader_head h
                WHERE NOT EXISTS(SELECT 1 FROM ah_sessions s WHERE s.conversation_id=h.conversation)
                UNION ALL SELECT w.conversation,lower(hex(randomblob(16))) FROM ah_reader_serving w
                WHERE NOT EXISTS(SELECT 1 FROM ah_sessions s WHERE s.conversation_id=w.conversation)",
                    (),
                )?;
                if inserted > 0 { tx.report_reader_repair(|| Ok(true)); }
                Ok(())
            })?;
            self.recover_reader_heads(&job.cancel)?;
            if self.store.read(DB, |tx| {
                tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_parked)", (), |r| {
                    Ok(r.get::<i64>(0)? != 0)
                })
            })? {
                return Err(Error::ReaderRebuildRequired);
            }
        }
        // A repair can replace a summary without changing a native projection. Publish its
        // queued title and card before collecting the superseded presentation.
        self.synchronize_search_with_cancel(&|| job.cancel.check().is_err())?;
        self.collect_reader_garbage(&job.cancel)?;
        Ok(count)
    }

    pub(super) fn rebuild_conversation_versions(
        &self,
        conversation: &str,
        job: &JobControl,
    ) -> Result<u64> {
        let mut count = 0;
        let mut after = String::new();
        loop {
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            let (id, key) = (conversation.to_owned(), after.clone());
            let batch = self.store.read(DB, move |tx| {
                tx.query(
                    "SELECT id,projection_id,revision_id,parser_version FROM ah_sessions
                         WHERE conversation_id=?1 AND id>?2 ORDER BY id LIMIT 128",
                    params![id, key],
                    |row| {
                        Ok(ProjectionAttempt {
                            source_id: row.get(0)?,
                            id: row.get(1)?,
                            revision_id: row.get(2)?,
                            parser_version: row.get(3)?,
                        })
                    },
                )
            })?;
            if batch.is_empty() {
                break;
            }
            for attempt in batch {
                after.clone_from(&attempt.source_id);
                if let Err(error) = self.repair_reader_version(&attempt, job) {
                    // Only the active scan can own a repairing version of this parser attempt.
                    self.store.write(DB,Priority::Batch,move|tx|tx.execute(
                            "UPDATE ah_reader_version SET state='retired' WHERE attempt=?1 AND state='repairing'",
                            [attempt.id]))?;
                    self.jobs.request_collection();
                    return Err(error);
                }
                count += 1;
            }
        }
        Ok(count)
    }

    fn repair_reader_version(&self, attempt: &ProjectionAttempt, job: &JobControl) -> Result<()> {
        let info = self.revision(attempt.revision_id)?;
        if info.source_id != attempt.source_id || !info.bytes_retained {
            return Err(Error::StaleCursor);
        }
        let expected_digest = info.digest.clone();
        let expected_head = self.source(&attempt.source_id)?.latest_revision_id;
        let mut context = ParseContext::default();
        let mut job = job.clone();
        for option in attempt.parser_version.split(':') {
            if let Some(size) = option.strip_prefix("record=") {
                context.max_record_bytes = size.parse().map_err(pages::invalid)?;
            }
            if let Some(size) = option.strip_prefix("decoded=") {
                job.max_decoded_bytes =
                    job.max_decoded_bytes.min(size.parse().map_err(pages::invalid)?);
            }
        }
        let record_limit = context.max_record_bytes as i64;
        let decoded_limit = job.max_decoded_bytes.min(i64::MAX as u64) as i64;
        let snapshot = attempt.clone();
        let token = self.store.write(DB, Priority::Batch, move |tx| {
            tx.execute("UPDATE ah_reader_version SET state='retired' WHERE attempt=?1 AND state='repairing'", [snapshot.id])?;
            let token: String = tx.query_one("SELECT lower(hex(randomblob(16)))", (), |r| Ok(r.get(0)?))?;
            tx.execute(
                "INSERT INTO ah_reader_version(token,attempt,source,revision,digest,byte_len,codec,renderer,records,selective,state,record_limit,decoded_limit,max_frame,format_version)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0,0,'repairing',?9,?10,0,NULL)",
                params![token.clone(), snapshot.id, snapshot.source_id, info.id, info.digest, info.byte_len as i64, CODEC, RENDERER, record_limit, decoded_limit],
            )?;
            require_staging(tx, &token)?;
            Ok(token)
        })?;
        let (records, max_frame, format_version) =
            self.build_reader_pages(&token, attempt, &context, &job)?;
        let attempt_id = attempt.id;
        let staged = token.clone();
        let prior_source = attempt.source_id.clone();
        let previous=self.store.write(DB,Priority::Batch,move|tx|{
            require_staging(tx,&staged)?;
            tx.execute(
                "UPDATE ah_reader_version SET records=?2,selective=?3,max_frame=?4,format_version=?5 WHERE token=?1",
                params![staged,records as i64,i64::from(max_frame>REPLAY_LIMIT),max_frame as i64,format_version]
            )?;
            serving_version(tx,&prior_source,attempt_id)
        })?;
        let healthy = if let Some((old, _, 1)) = &previous {
            match self.same_reader_pages(old, &token, &job) {
                Ok(equal) => equal,
                Err(error) if local_reader_damage(&error) => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };
        let _lock = self.publication_gate.lock().map_err(|_| pages::invalid("publication lock"))?;
        let info = self.revision(attempt.revision_id)?;
        if !info.bytes_retained
            || info.digest != expected_digest
            || self.source(&attempt.source_id)?.latest_revision_id != expected_head
        {
            return Err(Error::StaleCursor);
        }
        let source = attempt.source_id.clone();
        self.store.write(DB,Priority::Batch,move|tx|{
            require_staging(tx,&token)?;
            if serving_version(tx,&source,attempt_id)?!=previous{return Err(crate::query::operation(Error::StaleCursor))}
            // A damaged attempt/source column can strand an extra published row. Retire only
            // versions no live legacy projection owns; unrelated valid sources keep their owner.
            tx.execute("UPDATE ah_reader_version SET state='retired' WHERE state='published' AND
                (attempt=?1 OR source=?2) AND NOT EXISTS(SELECT 1 FROM ah_sessions s
                WHERE s.projection_id=ah_reader_version.attempt AND s.id=ah_reader_version.source)",
                params![attempt_id,source.as_str()])?;
            if healthy{
                tx.execute("UPDATE ah_reader_version SET state='retired' WHERE token=?1",[token])?;
            }else{
                tx.execute("UPDATE ah_reader_version SET state='retired' WHERE source=?1 AND attempt=?2 AND state='published'",params![source.as_str(),attempt_id])?;
                let generation=previous.map_or(0,|(_,old,_)|old.checked_add(1).filter(|n|*n>0).unwrap_or(1));
                tx.execute("UPDATE ah_reader_version SET state='published',repair_generation=?2 WHERE token=?1",params![token.clone(),generation])?;
                manifest::sign_version(tx,&token)?;
            }
            refresh(tx,&source,None)
        })?;
        Ok(())
    }
}

/// Record the owner and multiplicity together. Ambiguous or malformed old metadata
/// needs replacement; only an unchanged unique serving version can be reused.
fn serving_version(
    tx: &Session<'_>,
    source: &str,
    attempt: i64,
) -> vault_store::Result<Option<(String, i64, i64)>> {
    tx.query_one("SELECT min(CAST(token AS TEXT)),max(CAST(repair_generation AS INTEGER)),count(*)
        FROM ah_reader_version WHERE attempt=?1 AND source=?2 AND state='published' HAVING count(*)>0",
        params![attempt,source],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()
}
