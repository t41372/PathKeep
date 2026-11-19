//! Responsibilities: preserve physical source identity while discovery improves its role.
//! Not responsible for: path discovery, source reads or deciding which adapters are enabled.
use super::{Archive, SourceInfo};
use crate::{
    Error, Result,
    model::{Artifact, ArtifactKind, SourceRoot},
};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    /// Registers discovery facts even when the source bytes are unchanged. References
    /// can introduce a file as metadata before its adapter recognizes a transcript.
    /// Role promotion never changes the source key or downgrades an established role.
    pub fn register_source(
        &self,
        source_id: &str,
        source: &SourceRoot,
        artifact: &Artifact,
    ) -> Result<()> {
        if source_id.is_empty() || source_id.len() > 256 {
            return Err(Error::InvalidInput("source identity must be 1..=256 bytes".into()));
        }
        let root = source
            .root
            .to_str()
            .ok_or_else(|| Error::InvalidInput("source root must be UTF-8".into()))?
            .to_owned();
        let desired = SourceInfo {
            id: source_id.into(),
            harness: source.harness.to_string(),
            root,
            relative_path: artifact.relative_path.clone(),
            kind: kind_name(artifact.kind).into(),
            retention: crate::adapters::adapter(source.harness).retention(artifact).into(),
            last_seen_at: None,
            missing_since: None,
            latest_revision_id: None,
            observed_at: chrono::Utc::now().timestamp_millis(),
        };
        let _publication = self
            .publication_gate
            .lock()
            .map_err(|_| Error::Integrity("archive publication lock was poisoned".into()))?;
        let id = source_id.to_owned();
        let known = self.store.read(Db::AgentHistoryEvidence, move |tx| read_source(tx, &id))?;
        if let Some(known) = &known {
            // Salvage can retain verified bytes whose descriptor page was lost.
            // Only the original physical source identity may fill that blank.
            let missing =
                known.root.is_empty() && known.harness.is_empty() && known.relative_path.is_empty();
            let matches = if missing {
                crate::capture::source_identity(source, artifact)? == source_id
            } else {
                known.harness == desired.harness
                    && known.root == desired.root
                    && known.relative_path == desired.relative_path
            };
            if !matches {
                return Err(Error::Integrity(
                    "source identity was reused for a different source".into(),
                ));
            }
            if desired.kind == "transcript"
                && known.kind != "transcript"
                && known.latest_revision_id.is_some()
            {
                // Coverage becomes conservative before this raw-only source gains
                // a searchable role, including the unchanged-byte capture path.
                self.set_coverage(source_id, false)?;
            }
        }
        let selected_retention = match &known {
            Some(known) if known.latest_revision_id.is_some() && known.retention != "state" => {
                known.retention.clone()
            }
            _ => desired.retention,
        };
        let selected_kind = match &known {
            Some(known) if kind_rank(&known.kind) >= kind_rank(&desired.kind) => known.kind.clone(),
            _ => desired.kind,
        };
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            tx.execute(
                "INSERT INTO ah_sources(id, harness, root, relative_path, kind, observed_at,
                    retention) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET
                    kind=excluded.kind, retention=excluded.retention, harness=excluded.harness,
                    root=excluded.root,relative_path=excluded.relative_path",
                params![
                    desired.id,
                    desired.harness,
                    desired.root,
                    desired.relative_path,
                    selected_kind,
                    desired.observed_at,
                    selected_retention
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub(crate) fn retired_fallback(&self, source: &SourceInfo) -> Result<bool> {
        if !source.relative_path.ends_with("/audit.jsonl") {
            return Ok(false);
        }
        let source = source.clone();
        Ok(self.store.read(Db::AgentHistoryEvidence, move |tx| retired_audit(tx, &source))?)
    }

    /// Loads provenance for exact source association; native session IDs are not used here.
    pub fn source(&self, source_id: &str) -> Result<SourceInfo> {
        let id = source_id.to_owned();
        self.store
            .read(Db::AgentHistoryEvidence, move |tx| read_source(tx, &id))?
            .ok_or_else(|| Error::NotFound("source".into()))
    }

    /// Allows export policy to reject destinations inside source roots.
    pub fn source_roots(&self) -> Result<Vec<std::path::PathBuf>> {
        Ok(self.store.read(Db::AgentHistoryEvidence, |tx| {
            tx.query("SELECT DISTINCT root FROM ah_sources WHERE root<>'' ORDER BY root", (), |r| {
                Ok(std::path::PathBuf::from(r.get::<String>(0)?))
            })
        })?)
    }
}

pub(super) fn read_source(
    tx: &vault_store::Session<'_>,
    id: &str,
) -> vault_store::Result<Option<SourceInfo>> {
    tx.query_one(
        "SELECT id, harness, root, relative_path, kind, latest_revision_id, observed_at,
            retention, last_seen_at, missing_since FROM ah_sources WHERE id=?1",
        [id],
        |row| {
            Ok(SourceInfo {
                id: row.get(0)?,
                harness: row.get(1)?,
                root: row.get(2)?,
                relative_path: row.get(3)?,
                kind: row.get(4)?,
                latest_revision_id: row.get(5)?,
                observed_at: row.get(6)?,
                retention: row.get(7)?,
                last_seen_at: row.get(8)?,
                missing_since: row.get(9)?,
            })
        },
    )
    .optional()
}

fn kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Attachment => "attachment",
        ArtifactKind::Metadata => "metadata",
        ArtifactKind::Sqlite => "sqlite",
        ArtifactKind::Transcript => "transcript",
    }
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "attachment" => 0,
        "metadata" => 1,
        "sqlite" => 2,
        "transcript" => 3,
        _ => 0,
    }
}

/// Older evidence kept the highest kind ever observed. For a Cowork fallback
/// use the last retained sighting and the primary transcripts seen alongside it.
/// New captures record the effective metadata role at retirement.
pub(crate) fn retired_audit(
    tx: &vault_store::Session<'_>,
    source: &SourceInfo,
) -> vault_store::Result<bool> {
    if source.harness != "claude-code" {
        return Ok(false);
    }
    let Some(body) = source.relative_path.strip_suffix("/audit.jsonl") else { return Ok(false) };
    if !body.rsplit('/').next().is_some_and(|name| name.starts_with("local_")) {
        return Ok(false);
    }
    let prefix = format!("{body}/.claude/projects/");
    let mut retired = false;
    tx.for_each("SELECT s.relative_path FROM ah_sources s
        WHERE s.harness='claude-code' AND s.root=?1 AND s.relative_path>=?2 AND s.relative_path<?3
        AND s.kind='transcript' AND EXISTS(SELECT 1 FROM ah_source_observations o
        WHERE o.source_id=s.id AND o.seen=1 AND o.run_id=(SELECT max(run_id) FROM ah_source_observations WHERE source_id=?4 AND seen=1))",
        params![source.root.clone(),prefix.clone(),format!("{prefix}\u{10ffff}"),source.id.clone()],|r|{
            let path=r.get::<String>(0)?;
            if let Some((_,name))=path.strip_prefix(&prefix).and_then(|p|p.split_once('/')){
                retired |= !name.contains('/') && !name.starts_with("agent-") && name.ends_with(".jsonl") && crate::adapters::adapter(crate::Harness::ClaudeCode).publishes_transcript(&path);
            }
            Ok(())
        })?;
    Ok(retired)
}
