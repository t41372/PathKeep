//! Responsibilities: maintain archive totals alongside fact writes and load cached counts.
//! Not responsible for: legacy SQLite dashboards or calculating totals by scanning facts.
//! Failure modes: rollback leaves counts ahead, resume double-counts, visibility changes
//! leave totalVisits or per-profile totals stale, or zero-row/duplicate batches increment counts. Tests: ingest concurrency and batch_search_tests.
use vault_store::{Result, Session, params};

pub(crate) fn add(s: &Session<'_>, field: &str, amount: i64) -> Result<()> {
    s.execute("UPDATE archive_totals SET value=value+?2 WHERE name=?1", params![field, amount])?;
    Ok(())
}
/// Global and source visit counters share one statement and the facts' transaction.
pub(crate) fn add_visits(s: &Session<'_>, profile: i64, amount: i64) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    s.execute("INSERT INTO archive_totals(name,value) VALUES('visits',?2),(?1,?2) ON CONFLICT(name) DO UPDATE SET value=archive_totals.value+excluded.value",params![format!("sourceVisits:{profile}"),amount])?;
    Ok(())
}

/// Keeps the ingest statement budget and its production counter maintenance identical.
pub(crate) fn add_batch(
    s: &Session<'_>,
    profile: i64,
    urls: usize,
    visits: usize,
    downloads: usize,
) -> Result<()> {
    add(s, "urls", urls as i64)?;
    add_visits(s, profile, visits as i64)?;
    add(s, "downloads", downloads as i64)
}
pub(crate) fn load_cached_archive_totals(
    s: &Session<'_>,
) -> Result<std::collections::BTreeMap<String, i64>> {
    Ok(s.query("SELECT name,value FROM archive_totals WHERE name IN ('sourceProfiles','urls','visits','downloads','manifests','snapshots')", (), |r| Ok((r.get(0)?, r.get(1)?)))?
        .into_iter()
        .collect())
}
