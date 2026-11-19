//! Responsibilities: bound parser writes and atomically publish an attempt token.
//! Not responsible for: allocating attempt identities, parsing or retiring raw data.
use super::coverage::validate_coverage;
use super::{
    ProjectionAttempt, current_projection_id, publish_outbox, require_work, retire, set_coverage,
};
use crate::{
    Error, Result,
    archive::Archive,
    model::{NativeEvent, SessionMetadata},
};
use std::io::{self, Write};
use vault_store::{Db, OptionalRow, Priority, params};

const MAX_BATCH_EVENTS: usize = 128;
const MAX_BATCH_BYTES: usize = 8 * 1024 * 1024;
const MAX_SINGLE_EVENT_BYTES: usize = 128 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 64 * 1024 * 1024;

impl Archive {
    /// Commits one bounded parser batch to exactly the named staging generation.
    /// Duplicate ordinals roll back the whole batch; stale attempts cannot write.
    pub fn append_projection_events(
        &self,
        attempt: &ProjectionAttempt,
        events: Vec<NativeEvent>,
    ) -> Result<()> {
        if events.len() > MAX_BATCH_EVENTS {
            return Err(Error::InvalidInput("projection batch exceeds 128 records".into()));
        }
        let limit = if events.len() == 1 { MAX_SINGLE_EVENT_BYTES } else { MAX_BATCH_BYTES };
        let mut bytes = 0usize;
        let mut encoded = Vec::with_capacity(events.len());
        for event in events {
            let ordinal = i64::try_from(event.ordinal)
                .map_err(|_| Error::InvalidInput("event ordinal exceeds supported range".into()))?;
            let scalar_bytes = [
                Some(event.text.as_str()),
                Some(event.kind.as_str()),
                event.native_id.as_deref(),
                event.parent_id.as_deref(),
                event.role.as_deref(),
            ]
            .into_iter()
            .flatten()
            .fold(0usize, |total, value| total.saturating_add(value.len()));
            bytes = bytes.saturating_add(scalar_bytes);
            if bytes > limit {
                return Err(batch_limit());
            }
            let payload = encode_payload(&event.payload, limit - bytes)?;
            bytes += payload.len();
            encoded.push((ordinal, event, payload));
        }
        let attempt = attempt.clone();
        self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            require_work(tx, &attempt)?;
            let mut insert = tx.prepare(
                "INSERT INTO ah_events(session_id, revision_id, projection_id, ordinal,
                    native_id, parent_id, kind, role, timestamp, text, payload) VALUES(?1, ?2, ?3,
                    ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for (ordinal, event, payload) in encoded {
                let mut references = std::collections::BTreeMap::new();
                for reference in &event.references {
                    if reference.kind == "lineage" {
                        continue;
                    }
                    if let Ok(key) = super::super::references::reference_key(reference) {
                        *references.entry(key).or_insert(0_i64) += 1;
                    }
                }
                for (key, count) in references {
                    tx.execute(
                        "INSERT INTO ah_event_references VALUES(?1,?2,?3,?4)",
                        params![attempt.id, ordinal, key, count],
                    )?;
                }
                insert.execute(params![
                    attempt.source_id.clone(),
                    attempt.revision_id,
                    attempt.id,
                    ordinal,
                    event.native_id,
                    event.parent_id,
                    event.kind,
                    event.role,
                    event.timestamp,
                    event.text,
                    payload,
                ])?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Makes one fully staged interpretation visible and records search work in
    /// the same transaction. Its raw revision may equal the previous one's.
    pub fn publish_projection_attempt(
        &self,
        attempt: &ProjectionAttempt,
        metadata: &SessionMetadata,
        complete: bool,
    ) -> Result<()> {
        self.publish_projection_attempt_with_coverage(attempt, metadata, complete, complete)
    }

    /// Publishes parsing and resolved-reference coverage together, so a source
    /// with missing attachments is never briefly advertised as complete.
    pub fn publish_projection_attempt_with_coverage(
        &self,
        attempt: &ProjectionAttempt,
        metadata: &SessionMetadata,
        parser_complete: bool,
        combined_complete: bool,
    ) -> Result<()> {
        validate_coverage(parser_complete, combined_complete)?;
        validate_metadata(metadata)?;
        self.stage_reader(attempt, &Default::default(), &Default::default())?;
        let counter = self.build_attachment_counter(attempt.id, attempt.revision_id)?;
        let snapshot = self.projection_generation()?;
        self.require_revision_source(&attempt.source_id, attempt.revision_id)?;
        let source = self.source(&attempt.source_id)?;
        if source.latest_revision_id != Some(attempt.revision_id) {
            return Err(Error::SourceChanged(
                "raw revision was superseded before projection publication".into(),
            ));
        }
        let key = self.conversation_key(&attempt.source_id, attempt.revision_id, metadata)?;
        let conversation_id = key.id(source.harness.parse()?, &attempt.source_id);
        let native = crate::adapters::adapter(source.harness.parse()?);
        let rollout = native.physical_source_id(&source.relative_path);
        let source_time = native.source_time(&source.relative_path);
        let current_only = native.current_source_only();
        let generation_group = native.generation_group(&source.relative_path);
        self.prepare_lineage_boundaries(attempt, &key.relation, rollout.as_deref())?;
        let base_native = key.relation.base_native_id.clone();
        let relation = serde_json::to_string(&key.relation)?;
        let selection =
            super::super::selection::choose(self, &conversation_id, attempt, &key, metadata)?;
        let prior_source = attempt.source_id.clone();
        let prior = self
            .store
            .read(Db::AgentHistoryReadModel, move |tx| {
                tx.query(
                    "SELECT conversation_id FROM ah_sessions WHERE id=?1",
                    [prior_source],
                    |r| Ok(r.get::<String>(0)?),
                )
            })?
            .pop();
        let prior_selection = match &prior {
            Some(id) if id != &conversation_id => {
                super::super::selection::remaining(self, id, &attempt.source_id)?
            }
            _ => None,
        };
        let fork = key.relation.forked_from_native_id.clone();
        let fork_path = key.relation.fork_path.clone();
        let parent_path = key.relation.parent_path;
        let mut metadata = metadata.clone();
        metadata.native_id = key.native_id;
        metadata.parent_session_id = key.relation.parent_native_id.or(metadata.parent_session_id);
        let title_source = if metadata.title.is_some() { "tool" } else { "first-message" };
        let attempt = attempt.clone();
        let _publication = self
            .publication_gate
            .lock()
            .map_err(|_| Error::Integrity("publication gate was poisoned".into()))?;
        if self.projection_generation()? != snapshot
            || self.source(&attempt.source_id)?.latest_revision_id != Some(attempt.revision_id)
        {
            return Err(Error::SourceChanged(
                "publication changed while source relations were being validated".into(),
            ));
        }
        self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            require_work(tx, &attempt)?;
            crate::read_model::publication::install(tx, &attempt)?;
            if let Some(previous) = current_projection_id(tx, &attempt.source_id)? {
                retire(tx, &attempt.source_id, previous)?;
            }
            let previous_counter = tx
                .query_one(
                    "SELECT attachment_generation FROM ah_sessions WHERE id=?1",
                    [attempt.source_id.as_str()],
                    |row| Ok(row.get::<i64>(0)?),
                )
                .optional()?
                .unwrap_or(0);
            tx.execute(
                "INSERT INTO ah_sessions(id, source_id, native_id, harness, project_path,
                    title, created_at, updated_at, origin, harness_version, parent_session_id,
                    model, revision_id, projection_id, parser_version, complete, conversation_id,
                    origin_mark, source_root, relative_path, relation_json, generation,
                    title_source, parent_path, rollout_native_id, lineage_base_native_id,
                    forked_from_native_id, fork_path, current_only, source_time, generation_group,
                    first_record_digest,attachment_generation,attachments_not_kept)
                 VALUES(?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                    ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26,
                    ?27, ?28, ?29, ?30, ?31, ?32, ?33) ON CONFLICT(id) DO UPDATE SET
                    native_id=excluded.native_id, harness=excluded.harness,
                    project_path=excluded.project_path, title=excluded.title,
                    created_at=excluded.created_at, updated_at=excluded.updated_at,
                    origin=excluded.origin, harness_version=excluded.harness_version,
                    parent_session_id=excluded.parent_session_id, model=excluded.model,
                    revision_id=excluded.revision_id, projection_id=excluded.projection_id,
                    parser_version=excluded.parser_version, complete=excluded.complete,
                    conversation_id=excluded.conversation_id, origin_mark=excluded.origin_mark,
                    source_root=excluded.source_root, relative_path=excluded.relative_path,
                    relation_json=excluded.relation_json, generation=excluded.generation,
                    title_source=excluded.title_source, parent_path=excluded.parent_path,
                    rollout_native_id=excluded.rollout_native_id,
                    lineage_base_native_id=excluded.lineage_base_native_id,
                    forked_from_native_id=excluded.forked_from_native_id,
                    fork_path=excluded.fork_path, current_only=excluded.current_only,
                    source_time=excluded.source_time, generation_group=excluded.generation_group,
                    first_record_digest=excluded.first_record_digest,
                 raw_head_current=1,attachment_generation=excluded.attachment_generation,
                 attachments_not_kept=excluded.attachments_not_kept",
                params![
                    attempt.source_id.clone(),
                    metadata.native_id,
                    source.harness,
                    metadata.project_path,
                    metadata.title,
                    metadata.created_at,
                    metadata.updated_at,
                    metadata.origin,
                    metadata.harness_version,
                    metadata.parent_session_id,
                    metadata.model,
                    attempt.revision_id,
                    attempt.id,
                    attempt.parser_version,
                    i64::from(parser_complete),
                    conversation_id.clone(),
                    key.origin_mark,
                    source.root,
                    source.relative_path,
                    relation,
                    key.generation as i64,
                    title_source,
                    parent_path,
                    rollout,
                    base_native,
                    fork,
                    fork_path,
                    i64::from(current_only),
                    source_time,
                    generation_group,
                    key.first_record_digest,
                    counter.id,
                    counter.total as i64,
                ],
            )?;
            counter.publish(tx, previous_counter)?;
            selection.publish(tx, &conversation_id, Some(&attempt.source_id))?;
            if let (Some(selection), Some(id)) = (prior_selection, prior.as_ref()) {
                selection.publish(tx, id, None)?;
            }
            publish_outbox(tx, &attempt.source_id, attempt.revision_id, attempt.id)?;
            set_coverage(tx, &attempt.source_id, combined_complete)?;
            crate::read_model::publication::refresh(tx, &attempt.source_id, prior.as_deref())?;
            tx.execute("DELETE FROM ah_projection_work WHERE session_id=?1", [attempt.source_id])?;
            Ok(())
        })?;
        self.flush_reader_heads()
    }
}

fn validate_metadata(metadata: &SessionMetadata) -> Result<()> {
    let bytes = [
        Some(metadata.native_id.as_str()),
        metadata.project_path.as_deref(),
        metadata.title.as_deref(),
        Some(metadata.origin.as_str()),
        metadata.harness_version.as_deref(),
        metadata.parent_session_id.as_deref(),
        metadata.forked_from_session_id.as_deref(),
        metadata.model.as_deref(),
    ]
    .into_iter()
    .flatten()
    .fold(0usize, |total, value| total.saturating_add(value.len()));
    if bytes > MAX_METADATA_BYTES {
        return Err(Error::LimitExceeded("session metadata exceeds 64 MiB".into()));
    }
    Ok(())
}

fn encode_payload(payload: &serde_json::Value, limit: usize) -> Result<String> {
    let mut output = BoundedPayload { bytes: Vec::new(), limit, exceeded: false };
    if let Err(error) = serde_json::to_writer(&mut output, payload) {
        return Err(if output.exceeded { batch_limit() } else { Error::Json(error) });
    }
    String::from_utf8(output.bytes)
        .map_err(|_| Error::Integrity("serialized payload is not UTF-8".into()))
}

struct BoundedPayload {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl Write for BoundedPayload {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("projection payload limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn batch_limit() -> Error {
    Error::LimitExceeded("projection batch exceeds its byte budget".into())
}
