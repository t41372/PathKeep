//! Batches evidence identity and presence checks. No transcript bytes or live paths are read.
use super::*;
use crate::Archive;
use std::collections::HashMap;
use vault_store::{Value, params};
/// Evidence observation included in the membership snapshot, even if the live path has since vanished.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceObservation {
    pub captured_at: i64,
    pub capture_format: String,
    pub kind: String,
    pub last_seen_at: Option<i64>,
    pub missing_since: Option<i64>,
}
pub(super) struct Facts {
    source: String,
    digest: String,
    retained: bool,
    pub observation: SourceObservation,
}
impl Facts {
    pub fn check(&self, source: &str, digest: &str) -> Result<()> {
        if self.source != source || self.digest != digest {
            return Err(Error::StaleCursor);
        }
        if !self.retained {
            return Err(Error::NotFound("revision bytes expired by retention".into()));
        }
        Ok(())
    }
}
impl Archive {
    pub(super) fn facts(&self, ids: Vec<i64>) -> Result<HashMap<i64, Facts>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        Ok(self
            .store
            .read(Db::AgentHistoryEvidence, move |tx| {
                tx.query(
                    &format!(
                        "SELECT
                 r.id,r.source_id,r.digest,r.bytes_retained,r.captured_at,r.capture_format,s.kind,s.last_seen_at,s.missing_since
                 FROM ah_revisions r JOIN ah_sources s ON s.id=r.source_id WHERE
                 r.state='complete' AND r.id IN ({placeholders})"
                    ),
                    ids.into_iter().map(Value::Integer).collect::<Vec<_>>(),
                    |r| {
                        Ok((
                            r.get::<i64>(0)?,
                            Facts {
                                source: r.get(1)?,
                                digest: r.get(2)?,
                                retained: r.get::<i64>(3)? != 0,
                                observation: SourceObservation { captured_at: r.get(4)?, capture_format: r.get(5)?, kind: r.get(6)?, last_seen_at: r.get(7)?, missing_since: r.get(8)? },
                            },
                        ))
                    },
                )
            })?
            .into_iter()
            .collect())
    }

    pub(super) fn batch_facts(&self, id: &str, after: &str) -> Result<HashMap<i64, Facts>> {
        let id = id.to_string();
        let after = after.to_string();
        let ids = self.store.read(DB, move |tx| {
            tx.query(
                "SELECT v.revision FROM ah_sessions s JOIN ah_reader_version v ON
                 v.attempt=s.projection_id AND v.state='published' WHERE s.conversation_id=?1 AND
                 s.id>?2 ORDER BY s.id LIMIT 8",
                params![id, after],
                |r| Ok(r.get::<i64>(0)?),
            )
        })?;
        self.facts(ids).map_err(|e| crate::archive::recovery_error(Db::AgentHistoryEvidence, e))
    }
}
