//! Bounded reclamation of replaced conversation snapshots and unreferenced source pages.
//! Canonical evidence and a currently reachable line can never be collected here.
use super::*;
use crate::Archive;
use vault_store::{OptionalRow, Priority, params};

impl Archive {
    /// Gives maintenance a bounded share of the writer. Repeat until it returns zero
    /// to drain abandoned snapshots and release blobs without surviving page references.
    pub fn prune_reader_pages(&self, limit: usize) -> Result<u64> {
        if !(1..=256).contains(&limit) {
            return Err(Error::InvalidInput("reader cleanup limit must be 1..=256".into()));
        }
        Ok(self.store.write(DB, Priority::Batch, move |tx| {
            if let Some(token) = tx.query_one("SELECT publication FROM ah_reader_retired LIMIT 1", (), |r| Ok(r.get::<String>(0)?)).optional()? {
                let mut changed = 0;
                for table in ["ah_reader_member", "ah_reader_line", "ah_reader_dependency", "ah_reader_edge"] {
                    changed += tx.execute(
                        &format!(
                            "DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE
                 publication=?1 LIMIT ?2)"
                        ),
                        params![token.clone(), limit as i64],
                    )?;
                }
                if changed == 0 {
                    changed += tx.execute("DELETE FROM ah_reader_retired WHERE publication=?1", [token])?;
                }
                return Ok(changed);
            }
            let token = tx
                .query_one(
                    "SELECT token FROM ah_reader_version v WHERE state='retired' AND NOT
                 EXISTS(SELECT 1 FROM ah_reader_member WHERE version=v.token) AND NOT
                 EXISTS(SELECT 1 FROM ah_reader_line WHERE version=v.token) LIMIT 1",
                    (),
                    |r| Ok(r.get::<String>(0)?),
                )
                .optional()?;
            let Some(token) = token else { return Ok(0) };
            let mut changed = 0;
            let pages = tx.query("SELECT rowid,hash FROM ah_reader_page WHERE owner=?1 LIMIT ?2", params![token.clone(), limit as i64], |r| Ok((r.get::<i64>(0)?, r.get::<Vec<u8>>(1)?)))?;
            for (row, hash) in pages {
                changed += tx.execute("DELETE FROM ah_reader_page WHERE rowid=?1", [row])?;
                tx.execute(
                    "DELETE FROM ah_reader_blob WHERE hash=?1 AND NOT EXISTS(SELECT 1 FROM
                 ah_reader_page WHERE hash=?1)",
                    params![hash],
                )?;
            }
            for table in ["ah_reader_sort", "ah_reader_lookup", "ah_reader_rank","ah_reader_leaf","ah_reader_leaf_line"] {
                changed += tx.execute(&format!("DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE owner=?1 LIMIT ?2)"), params![token.clone(), limit as i64])?;
            }
            if changed == 0 {
                changed += tx.execute("DELETE FROM ah_reader_version WHERE token=?1", [token])?;
            }
            Ok(changed)
        })?)
    }
}
