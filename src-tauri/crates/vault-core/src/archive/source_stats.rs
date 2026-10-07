//! Responsibilities: source visit counts and date spans from one committed snapshot.
//! Not responsible for: browser discovery, ingestion or persistent caches.
//! Failure modes: reverted rows counted, empty profiles hidden, stale file-stamp
//! caches, full fact scans. Tests: source_stats_tests, plaintext and encrypted.

use crate::{Vault, chrome::browser_display_name, models::SourceStats};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use vault_store::{Db, Session};

const PROFILE_COUNT_SQL: &str =
    "SELECT COALESCE((SELECT value FROM archive_totals WHERE name='sourceVisits:'||?1),0)";
const PROFILE_FIRST_SQL: &str = "SELECT visit_time_ms FROM visits INDEXED BY idx_visits_visible_profile_time_id WHERE source_profile_id=?1 AND reverted_at IS NULL ORDER BY visit_time_ms,id LIMIT 1";
const PROFILE_LAST_SQL: &str = "SELECT visit_time_ms FROM visits INDEXED BY idx_visits_visible_profile_time_id WHERE source_profile_id=?1 AND reverted_at IS NULL ORDER BY visit_time_ms DESC,id DESC LIMIT 1";

/// Includes empty profiles so discovery and archive statistics can share identities.
pub fn load_source_stats(vault: &Vault) -> Result<Vec<SourceStats>> {
    vault.read(Db::Archive, source_stats_for_session)
}

pub(crate) fn source_stats_for_session(s: &Session<'_>) -> Result<Vec<SourceStats>> {
    let profiles = s.query(
        "SELECT id, COALESCE(NULLIF(profile_key,''),browser_kind || ':' || profile_name), browser_kind,profile_name FROM source_profiles ORDER BY browser_kind,profile_name",
        (), |r| Ok((r.get::<i64>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<String>(3)?)),
    )?;
    profiles
        .into_iter()
        .map(|(id, profile_id, browser_kind, profile_name)| {
            Ok(SourceStats {
                profile_id,
                browser_name: browser_display_name(&browser_kind),
                profile_name,
                visit_count: s.query_one(PROFILE_COUNT_SQL, [id], |r| Ok(r.get(0)?))?,
                first_visit_at: s
                    .query(PROFILE_FIRST_SQL, [id], |r| Ok(r.get::<i64>(0)?))?
                    .pop()
                    .and_then(rfc3339_from_ms),
                last_visit_at: s
                    .query(PROFILE_LAST_SQL, [id], |r| Ok(r.get::<i64>(0)?))?
                    .pop()
                    .and_then(rfc3339_from_ms),
            })
        })
        .collect()
}

pub(crate) fn rfc3339_from_ms(ms: i64) -> Option<String> {
    Utc.timestamp_millis_opt(ms).single().map(|value| value.to_rfc3339())
}

#[cfg(test)]
#[path = "source_stats_tests.rs"]
pub(super) mod tests;
