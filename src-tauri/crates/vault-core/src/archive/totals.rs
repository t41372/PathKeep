//! Responsibilities: maintain archive totals alongside fact writes and load cached counts.
//! Not responsible for: legacy SQLite dashboards or calculating totals by scanning facts.
//! Failure modes: rollback leaves counts ahead, resume double-counts, visibility changes
//! leave totalVisits stale. Tests: ingest concurrency and batch_search_tests.
use vault_store::{Result, Session, params};

pub(crate) fn add(s: &Session<'_>, field: &str, amount: i64) -> Result<()> {
    s.execute("UPDATE archive_totals SET value=value+?2 WHERE name=?1", params![field, amount])?;
    Ok(())
}
pub(crate) fn load_cached_archive_totals(
    s: &Session<'_>,
) -> Result<std::collections::BTreeMap<String, i64>> {
    Ok(s.query("SELECT name,value FROM archive_totals", (), |r| Ok((r.get(0)?, r.get(1)?)))?
        .into_iter()
        .collect())
}
