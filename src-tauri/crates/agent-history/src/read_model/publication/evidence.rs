//! Batches evidence identity and presence checks. No transcript bytes or live paths are read.
use super::*;
use crate::Archive;
use std::collections::HashMap;
use std::io::BufReader;
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
    pub(super) digest: String,
    pub(super) bytes: u64,
    pub(super) retained: bool,
    pub observation: SourceObservation,
    pub launch_call: Option<String>,
}
impl Facts {
    pub(super) fn verify_identity(&self, source: &str, digest: &str) -> Result<()> {
        if self.source != source || self.digest != digest {
            return Err(Error::StaleCursor);
        }
        Ok(())
    }
    pub fn check(&self, source: &str, digest: &str) -> Result<()> {
        self.verify_identity(source, digest)?;
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
                 r.id,r.source_id,r.digest,r.bytes_retained,r.captured_at,r.capture_format,s.kind,s.last_seen_at,s.missing_since,r.byte_len
                 FROM ah_revisions r JOIN ah_sources s ON s.id=r.source_id WHERE
                 r.state='complete' AND r.id IN ({placeholders})"
                    ),
                    ids.into_iter().map(Value::Integer).collect::<Vec<_>>(),
                    |r| {
                        Ok((
                            r.get::<i64>(0)?,
                            Facts {
                                source: r.get(1)?, launch_call: None,
                                digest:r.get(2)?, bytes:r.get::<i64>(9)? as u64,
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
        let members = self.store.read(DB, move |tx| {
            tx.query(
                "SELECT revision_id,harness,source_root,relative_path FROM ah_sessions
                 WHERE conversation_id=?1 AND id>?2 ORDER BY id LIMIT 8",
                params![id, after],
                |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        r.get::<String>(1)?,
                        r.get::<String>(2)?,
                        r.get::<String>(3)?,
                    ))
                },
            )
        })?;
        let mut facts = self
            .facts(members.iter().map(|m| m.0).collect())
            .map_err(|e| self.recovery_error(Db::AgentHistoryEvidence, e))?;
        for (revision, harness, root, path) in members {
            if harness == "claude-code"
                && path.contains("/subagents/")
                && let Some(stem) = path.strip_suffix(".jsonl")
                && let Some(member) = facts.get_mut(&revision)
            {
                member.launch_call = self.saved_launch_call(root, format!("{stem}.meta.json"))?;
            }
        }
        Ok(facts)
    }

    fn saved_launch_call(&self, root: String, path: String) -> Result<Option<String>> {
        use vault_store::OptionalRow;
        let revision = self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one(
                "SELECT r.id FROM ah_sources s JOIN ah_revisions r ON r.id=s.latest_revision_id
                 WHERE s.harness='claude-code' AND s.root=?1 AND s.relative_path=?2
                 AND r.bytes_retained=1",
                params![root, path],
                |r| Ok(r.get::<i64>(0)?),
            )
            .optional()
        })?;
        let Some(revision) = revision else { return Ok(None) };
        #[derive(Deserialize)]
        struct Metadata {
            #[serde(rename = "toolUseId")]
            call: String,
        }
        let reader = BufReader::new(self.raw_reader(revision)?);
        match serde_json::from_reader::<_, Metadata>(reader) {
            Ok(meta) => Ok(Some(meta.call)),
            Err(error) if error.is_io() => Err(error.into()),
            Err(_) => Ok(None),
        }
    }
}
