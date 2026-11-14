//! Responsibilities: validate native byte/ordinal boundaries once against immutable evidence.
//! Not responsible for: choosing an ambiguous ancestor or doing hidden work during reads.
use crate::{
    Archive, Result,
    adapters::{common, identity::SourceRelation},
    archive::ProjectionAttempt,
};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader};
use vault_store::{Db, Priority, params};

impl Archive {
    pub(super) fn prepare_lineage_boundaries(
        &self,
        attempt: &ProjectionAttempt,
        relation: &SourceRelation,
        rollout: Option<&str>,
    ) -> Result<()> {
        let provenance = self.source(&attempt.source_id)?;
        if let Some(base) = &relation.base_native_id {
            let base = base.clone();
            let harness = provenance.harness.clone();
            let sources = self.store.read(Db::Archive, move |tx| {
                tx.query(
                    "
                SELECT revision_id, relative_path, relation_json FROM ah_sessions
                WHERE harness=?1 AND rollout_native_id=?2",
                    params![harness, base],
                    |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
                )
            })?;
            for (revision, path, own) in sources {
                let own: SourceRelation = serde_json::from_str(&own)?;
                self.record_boundary(revision, &path, relation, own.base_end_ordinal.unwrap_or(0))?;
            }
        }
        if let Some(rollout) = rollout {
            let rollout = rollout.to_owned();
            let harness = provenance.harness;
            let pending = self.store.read(Db::Archive, move |tx| {
                tx.query(
                    "
                SELECT relation_json FROM ah_sessions WHERE harness=?1 AND
                lineage_base_native_id=?2",
                    params![harness, rollout],
                    |r| Ok(r.get::<String>(0)?),
                )
            })?;
            let relation_inherited = relation.base_end_ordinal.unwrap_or(0);
            for relation in pending {
                self.record_boundary(
                    attempt.revision_id,
                    &provenance.relative_path,
                    &serde_json::from_str(&relation)?,
                    relation_inherited,
                )?;
            }
        }
        Ok(())
    }

    fn record_boundary(
        &self,
        revision: i64,
        path: &str,
        relation: &SourceRelation,
        inherited: u64,
    ) -> Result<()> {
        let (Some(end), Some(length)) = (relation.base_end_ordinal, relation.base_end_byte) else {
            return Ok(());
        };
        let (Ok(end), Ok(length)) = (i64::try_from(end), i64::try_from(length)) else {
            return Ok(());
        };
        let present = self.store.read(Db::Archive, move |tx| {
            tx.query_one(
                "
            SELECT EXISTS(SELECT 1 FROM ah_lineage_validation WHERE revision_id=?1 AND
            end_ordinal=?2 AND byte_len=?3)",
                params![revision, end, length],
                |r| Ok(r.get::<i64>(0)? != 0),
            )
        })?;
        if present {
            return Ok(());
        }
        let own = (end as u64).checked_sub(inherited);
        let digest = match own {
            Some(own) => self.check_boundary(revision, path, own, length as u64)?,
            None => None,
        };
        self.store.write(Db::Archive, Priority::Batch, move |tx| {
            tx.execute(
                "INSERT OR IGNORE INTO ah_lineage_validation
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    revision,
                    end,
                    length,
                    i64::from(digest.is_some()),
                    digest,
                    own.map(|n| n as i64)
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn check_boundary(
        &self,
        revision: i64,
        path: &str,
        end: u64,
        length: u64,
    ) -> Result<Option<String>> {
        let mut input =
            BufReader::new(common::decoded_reader(Box::new(self.raw_reader(revision)?), path)?);
        let mut digest = Sha256::new();
        let mut ordinal = 0;
        let mut bytes = 0;
        while ordinal < end {
            let available = input.fill_buf()?;
            if available.is_empty() {
                return Ok(None);
            }
            let n = available.iter().position(|b| *b == b'\n').map_or(available.len(), |i| i + 1);
            if available[n - 1] == b'\n' {
                ordinal += 1;
            }
            digest.update(&available[..n]);
            bytes += n as u64;
            input.consume(n);
            if bytes > length {
                return Ok(None);
            }
        }
        Ok((bytes == length).then(|| hex::encode(digest.finalize())))
    }
}
