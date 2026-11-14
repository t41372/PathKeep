//! Responsibilities: build cumulative reference counts and atomically publish their pointer.
//! Not responsible for: deciding reference authority or changing event publication identities.
use super::{Archive, revisions::integrity};
use crate::Result;
use vault_store::{Db, OptionalRow, Priority, Session, params};

pub(super) struct AttachmentCounter {
    pub id: i64,
    pub total: u64,
}

impl Archive {
    pub(super) fn build_attachment_counter(
        &self,
        projection: i64,
        revision: i64,
    ) -> Result<AttachmentCounter> {
        let present = self.store.read(Db::Archive, move |tx| {
            tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM ah_event_references WHERE projection_id=?1)",
                [projection],
                |row| Ok(row.get::<i64>(0)? != 0),
            )
        })?;
        if !present {
            return Ok(AttachmentCounter { id: 0, total: 0 });
        }
        let id = self.store.write(Db::Archive, Priority::Batch, |tx| {
            tx.insert("INSERT INTO ah_attachment_generations(state) VALUES('staged')", ())?
                .ok_or_else(|| integrity("reference counter has no generation"))
        })?;
        let mut after = -1;
        let mut total = 0_u64;
        loop {
            let rows = self.store.read(Db::Archive, move |tx| {
                tx.query(
                    "SELECT e.ordinal,sum(e.occurrences) FROM ah_event_references e
                     LEFT JOIN ah_reference_outcomes o
                       ON o.parent_revision=?1 AND o.reference_key=e.reference_key
                     WHERE e.projection_id=?2 AND e.ordinal>?3
                       AND (o.reference_key IS NULL OR o.reason IS NOT NULL)
                     GROUP BY e.ordinal ORDER BY e.ordinal LIMIT 128",
                    params![revision, projection, after],
                    |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)? as u64)),
                )
            })?;
            if rows.is_empty() {
                break;
            }
            let mut counts = Vec::with_capacity(rows.len());
            for (ordinal, count) in rows {
                total += count;
                counts.push((ordinal, total));
                after = ordinal;
            }
            self.store.write(Db::Archive, Priority::Batch, move |tx| {
                for (ordinal, count) in counts {
                    tx.execute(
                        "INSERT INTO ah_attachment_counts VALUES(?1,?2,?3)",
                        params![id, ordinal, count as i64],
                    )?;
                }
                Ok(())
            })?;
        }
        Ok(AttachmentCounter { id, total })
    }

    pub(crate) fn refresh_attachment_counter(&self, source: &str, projection: i64) -> Result<()> {
        let revision = self.current_projection(source)?.expect("published source");
        let counter = self.build_attachment_counter(projection, revision)?;
        let source = source.to_owned();
        self.store.write(Db::Archive, Priority::Batch, move |tx| {
            let previous = tx.query_one(
                "SELECT attachment_generation FROM ah_sessions WHERE id=?1 AND projection_id=?2",
                params![source.clone(), projection],
                |row| Ok(row.get::<i64>(0)?),
            )?;
            tx.execute(
                "UPDATE ah_sessions SET attachment_generation=?1,attachments_not_kept=?2
                 WHERE id=?3 AND projection_id=?4",
                params![counter.id, counter.total as i64, source, projection],
            )?;
            counter.publish(tx, previous)
        })?;
        Ok(())
    }

    pub(crate) fn prune_attachment_counts(&self) -> Result<u64> {
        Ok(self.store.write(Db::Archive, Priority::Batch, |tx| {
            let id = tx
                .query_one(
                    "SELECT id FROM ah_attachment_generations WHERE state='retired' LIMIT 1",
                    (),
                    |row| Ok(row.get::<i64>(0)?),
                )
                .optional()?;
            let Some(id) = id else {
                return Ok(0);
            };
            let removed = tx.execute(
                "DELETE FROM ah_attachment_counts WHERE generation=?1 AND ordinal IN (
                   SELECT ordinal FROM ah_attachment_counts WHERE generation=?1 LIMIT 256)",
                [id],
            )?;
            if removed < 256 {
                tx.execute("DELETE FROM ah_attachment_generations WHERE id=?1", [id])?;
                return Ok(removed + 1);
            }
            Ok(removed)
        })?)
    }

    pub(super) fn recover_attachment_counts(&self) -> Result<()> {
        self.store.write(Db::Archive, Priority::Batch, |tx| {
            tx.execute(
                "UPDATE ah_attachment_generations SET state='retired' WHERE state='staged'",
                (),
            )?;
            Ok(())
        })?;
        Ok(())
    }
}

impl AttachmentCounter {
    pub(super) fn publish(self, tx: &Session<'_>, previous: i64) -> vault_store::Result<()> {
        if previous != 0 {
            tx.execute(
                "UPDATE ah_attachment_generations SET state='retired' WHERE id=?1",
                [previous],
            )?;
        }
        if self.id != 0 {
            tx.execute(
                "UPDATE ah_attachment_generations SET state='ready' WHERE id=?1",
                [self.id],
            )?;
        }
        Ok(())
    }
}
