//! Responsibilities: retain proven native-reference resolutions for offline reparsing.
//! Not responsible for: trusting a native path, following symlinks or capturing target files.
use super::{Archive, RevisionInfo};
use crate::{Error, Result, model::Reference};
use sha2::{Digest, Sha256};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    /// Records a resolution only after both artifacts were retained successfully.
    /// Changed attachment targets append observations; prior evidence is never replaced.
    pub fn bind_reference(
        &self,
        parent_revision: i64,
        reference: &Reference,
        target_revision: i64,
    ) -> Result<()> {
        self.revision(parent_revision)?;
        self.revision(target_revision)?;
        let key = reference_key(reference)?;
        let now = chrono::Utc::now().timestamp_millis();
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let previous = tx
                .query_one(
                    "SELECT target_revision FROM ah_reference_bindings WHERE parent_revision=?1 AND
                    reference_key=?2 ORDER BY id DESC LIMIT 1",
                    params![parent_revision, key.clone()],
                    |row| Ok(row.get::<i64>(0)?),
                )
                .optional()?;
            if previous != Some(target_revision) {
                tx.execute(
                    "INSERT INTO ah_reference_bindings(parent_revision, reference_key,
                        target_revision, observed_at) VALUES(?1, ?2, ?3, ?4)",
                    params![parent_revision, key, target_revision, now],
                )?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Resolves from retained provenance without guessing a live filesystem alias.
    /// A missing binding is a coverage gap; a missing bound revision is an error.
    pub fn bound_reference(
        &self,
        parent_revision: i64,
        reference: &Reference,
    ) -> Result<Option<RevisionInfo>> {
        self.revision(parent_revision)?;
        let key = reference_key(reference)?;
        let target = self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one(
                "SELECT target_revision FROM ah_reference_bindings WHERE parent_revision=?1 AND
                    reference_key=?2 ORDER BY id DESC LIMIT 1",
                params![parent_revision, key],
                |row| Ok(row.get::<i64>(0)?),
            )
            .optional()
        })?;
        target.map(|revision| self.revision(revision)).transpose()
    }
}

pub(crate) fn reference_key(reference: &Reference) -> Result<String> {
    if reference.kind.len() > 256 || reference.target.len() > 64 * 1024 {
        return Err(Error::LimitExceeded(
            "native reference exceeds its 256-byte kind or 64 KiB target limit".into(),
        ));
    }
    // Serde preserves this struct's field order. Include byte/ordinal bounds so
    // two inherited prefixes of one rollout never share an accidental binding.
    // The input limits also bound JSON's worst-case escaping to less than 400 KiB.
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(reference)?)))
}
