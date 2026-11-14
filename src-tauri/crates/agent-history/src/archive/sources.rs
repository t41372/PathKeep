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
        let known = self.store.read(Db::SourceEvidence, move |tx| read_source(tx, &id))?;
        if let Some(known) = &known {
            if known.harness != desired.harness
                || known.root != desired.root
                || known.relative_path != desired.relative_path
            {
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
        self.store.write(Db::SourceEvidence, Priority::Batch, move |tx| {
            tx.execute(
                "INSERT INTO ah_sources(id, harness, root, relative_path, kind, observed_at,
                    retention) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET
                    kind=excluded.kind, retention=excluded.retention",
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

    /// Loads provenance for exact source association; native session IDs are not used here.
    pub fn source(&self, source_id: &str) -> Result<SourceInfo> {
        let id = source_id.to_owned();
        self.store
            .read(Db::SourceEvidence, move |tx| read_source(tx, &id))?
            .ok_or_else(|| Error::NotFound("source".into()))
    }

    /// Allows export policy to reject destinations inside source roots.
    pub fn source_roots(&self) -> Result<Vec<std::path::PathBuf>> {
        Ok(self.store.read(Db::SourceEvidence, |tx| {
            tx.query("SELECT DISTINCT root FROM ah_sources ORDER BY root", (), |r| {
                Ok(std::path::PathBuf::from(r.get::<String>(0)?))
            })
        })?)
    }
}

fn read_source(tx: &vault_store::Session<'_>, id: &str) -> vault_store::Result<Option<SourceInfo>> {
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
