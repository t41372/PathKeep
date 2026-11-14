//! Responsibilities: retain a reachable empty view for a captured unreadable generation.
//! Not responsible for: guessing native IDs from paths or superseding uncaptured files.
use super::Archive;
use crate::{Result, SessionMetadata, adapters::adapter};
use vault_store::{Db, OptionalRow, params};

impl Archive {
    pub(crate) fn publish_generation_gap(
        &self,
        source_id: &str,
        revision: i64,
        parser: &str,
    ) -> Result<bool> {
        let source = self.source(source_id)?;
        let native = adapter(source.harness.parse()?);
        if !native.current_source_only() {
            return Ok(false);
        }
        let Some(group) = native.generation_group(&source.relative_path) else {
            return Ok(false);
        };
        let root = source.root.clone();
        let harness = source.harness;
        let previous = self.store.read(Db::Archive, move |tx| {
            tx.query_one(
                "SELECT native_id, project_path, title, created_at, updated_at, origin,
                    harness_version, parent_session_id, model, forked_from_native_id, generation,
                    EXISTS(SELECT 1 FROM ah_events e WHERE e.session_id=s.id AND
                    e.projection_id=s.projection_id) FROM ah_sessions s WHERE harness=?1 AND
                    source_root=?2 AND generation_group=?3 ORDER BY generation DESC LIMIT 1",
                params![harness, root, group],
                |row| {
                    Ok((
                        SessionMetadata {
                            native_id: row.get(0)?,
                            project_path: row.get(1)?,
                            title: row.get(2)?,
                            created_at: row.get(3)?,
                            updated_at: row.get(4)?,
                            origin: row.get(5)?,
                            harness_version: row.get(6)?,
                            parent_session_id: row.get(7)?,
                            model: row.get(8)?,
                            forked_from_session_id: row.get(9)?,
                        },
                        row.get::<i64>(10)? as u64,
                        row.get::<i64>(11)? != 0,
                    ))
                },
            )
            .optional()
        })?;
        let Some((metadata, previous_generation, had_records)) = previous else {
            return Ok(false);
        };
        let key = native.conversation_key(
            &metadata,
            source_id,
            &source.relative_path,
            "",
            &serde_json::Value::Null,
        );
        if key.generation < previous_generation
            || (key.generation == previous_generation && had_records)
        {
            return Ok(true);
        }
        if let Some(attempt) = self.prepare_projection(source_id, revision, parser, false)? {
            // The producer could not read this generation. Metadata comes only
            // from the adapter-proven prior group; no older events are copied.
            self.publish_projection_attempt_with_coverage(&attempt, &metadata, false, false)?;
        }
        Ok(true)
    }
}
