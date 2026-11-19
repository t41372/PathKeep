//! Responsibilities: maintain archive totals alongside fact writes and load cached counts.
//! Not responsible for: legacy SQLite dashboards or calculating totals by scanning facts.
//! Failure modes: rollback leaves counts ahead, resume double-counts, visibility changes
//! leave totalVisits or per-profile totals stale, or zero-row/duplicate batches increment counts. Tests: ingest concurrency and batch_search_tests.
use vault_store::{Result, Session, params};

pub(crate) fn add(s: &Session<'_>, field: &str, amount: i64) -> Result<()> {
    s.execute("UPDATE archive_totals SET value=value+?2 WHERE name=?1", params![field, amount])?;
    Ok(())
}
// The reader encodes positive tokens as -2 - token. Leave the last encodable
// token out of the draw: it replaces the candidate if it equals the previous
// token. This gives each alternative equal probability without another DB read.
const LAST_TOKEN: i64 = i64::MAX - 1;

fn membership_candidate() -> Result<i64> {
    loop {
        let bits = getrandom::u64().map_err(|e| vault_store::Error::Operation(Box::new(e)))?;
        let candidate = (bits >> 1) as i64;
        if (1..LAST_TOKEN).contains(&candidate) {
            return Ok(candidate);
        }
    }
}

/// Record a visible membership change in the facts' transaction. An import can swap
/// two IDs of the same profile with a zero count delta; it still needs a fresh stamp.
pub(crate) fn add_visits(s: &Session<'_>, profile: i64, amount: i64) -> Result<()> {
    s.execute(
        "INSERT INTO archive_totals(name,value)
         VALUES('visits',?2),(?1,?2),('visitMembershipToken',?3)
         ON CONFLICT(name) DO UPDATE SET value=CASE
             WHEN excluded.name <> 'visitMembershipToken' THEN archive_totals.value+excluded.value
             WHEN archive_totals.value = excluded.value THEN ?4
             ELSE excluded.value END",
        params![format!("sourceVisits:{profile}"), amount, membership_candidate()?, LAST_TOKEN],
    )?;
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
    if visits > 0 {
        add_visits(s, profile, visits as i64)?;
    }
    add(s, "downloads", downloads as i64)
}
pub(crate) fn load_cached_archive_totals(
    s: &Session<'_>,
) -> Result<std::collections::BTreeMap<String, i64>> {
    Ok(s.query("SELECT name,value FROM archive_totals WHERE name IN ('sourceProfiles','urls','visits','downloads','manifests','snapshots')", (), |r| Ok((r.get(0)?, r.get(1)?)))?
        .into_iter()
        .collect())
}
