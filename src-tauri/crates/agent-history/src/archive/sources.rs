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

    /// Discovery emits the main transcripts before the audit. Only a saved main
    /// in this task can take over publication from the audit fallback.
    pub(crate) fn cowork_audit_role(
        &self,
        root: &SourceRoot,
        artifact: &Artifact,
        run: i64,
    ) -> Result<Option<ArtifactKind>> {
        if root.harness != crate::Harness::ClaudeCode || artifact.kind != ArtifactKind::Metadata {
            return Ok(None);
        }
        let root = root
            .root
            .to_str()
            .ok_or_else(|| Error::InvalidInput("source root must be UTF-8".into()))?;
        let Some(body) = cowork_body(root, &artifact.relative_path) else { return Ok(None) };
        let (root, body) = (root.to_owned(), body.to_owned());
        let main = self
            .store
            .read(Db::AgentHistoryEvidence, move |tx| saved_main(tx, &root, &body, run))?;
        Ok(Some(if main { ArtifactKind::Metadata } else { ArtifactKind::Transcript }))
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

/// Discovery accepts the tool root or local-agent-mode-sessions itself as a grant.
fn cowork_body<'a>(root: &str, path: &'a str) -> Option<&'a str> {
    let body = path.strip_suffix("/audit.jsonl")?;
    let task = if let Some(task) = body.strip_prefix("local-agent-mode-sessions/") {
        task
    } else if root.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next()
        == Some("local-agent-mode-sessions")
    {
        body
    } else {
        return None;
    };
    let (_, task) = task.split_once('/')?;
    let (_, session) = task.split_once('/')?;
    (session.starts_with("local_") && !session.contains('/')).then_some(body)
}

/// Older archives kept the highest observed kind. Reconstruct the role at the
/// last successful audit capture. A later failed run cannot change that decision.
pub(crate) fn retired_audit(
    tx: &vault_store::Session<'_>,
    source: &SourceInfo,
) -> vault_store::Result<bool> {
    if source.harness != "claude-code" {
        return Ok(false);
    }
    let Some(body) = cowork_body(&source.root, &source.relative_path) else { return Ok(false) };
    let run = tx
        .query_one(
            "SELECT o.run_id FROM ah_source_observations o
         JOIN ah_run_files f ON f.run_id=o.run_id AND f.source_id=o.source_id
         WHERE o.source_id=?1 AND o.seen=1 AND f.revision_id IS NOT NULL
         ORDER BY o.run_id DESC LIMIT 1",
            [source.id.as_str()],
            |r| Ok(r.get::<i64>(0)?),
        )
        .optional()?;
    match run {
        Some(run) => saved_main(tx, &source.root, body, run),
        None => Ok(false),
    }
}

fn saved_main(
    tx: &vault_store::Session<'_>,
    root: &str,
    body: &str,
    run: i64,
) -> vault_store::Result<bool> {
    let prefix = format!("{body}/.claude/projects/");
    let mut retired = false;
    tx.for_each(
        "SELECT s.relative_path FROM ah_sources s
        WHERE s.harness='claude-code' AND s.root=?1 AND s.relative_path>=?2 AND s.relative_path<?3
        AND s.kind='transcript' AND EXISTS(SELECT 1 FROM ah_source_observations o
        WHERE o.source_id=s.id AND o.seen=1 AND o.run_id=?4)
        AND EXISTS(SELECT 1 FROM ah_source_observations prior
            JOIN ah_run_files f ON f.run_id=prior.run_id AND f.source_id=prior.source_id
            JOIN ah_revisions r ON r.id=f.revision_id AND r.source_id=s.id AND r.state='complete'
            WHERE prior.source_id=s.id AND prior.run_id<=?4)",
        params![root, prefix.clone(), format!("{prefix}\u{10ffff}"), run],
        |r| {
            let path = r.get::<String>(0)?;
            if let Some((_, name)) = path.strip_prefix(&prefix).and_then(|p| p.split_once('/')) {
                retired |= !name.contains('/')
                    && !name.starts_with("agent-")
                    && name.ends_with(".jsonl")
                    && crate::adapters::adapter(crate::Harness::ClaudeCode)
                        .publishes_transcript(&path);
            }
            Ok(())
        },
    )?;
    Ok(retired)
}
