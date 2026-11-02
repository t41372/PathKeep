//! Responsibilities: find the archived visit a captured browser row is, or say it is new; keep,
//! for one capture, which archived visit each of its rows became.
//! Not responsible for: writing visits (`writes`) or choosing which rows a backup reads (`mod`).
//! Failure modes: a reused row id joins a new visit to an old one; a redirect's two rows with one
//! time become two visits; a Firefox Sync copy cut to the millisecond becomes a second visit; a
//! version 1 visit is saved again once its browser row is renumbered; two visits less than a
//! millisecond apart become one; a visit hidden behind another source's copy is not found and is
//! added again on every backup.
//!
//! A captured row is the visit with its page's address and its browser time, kept without loss
//! (`visits.source_visit_time`). Among this profile's visits at the same millisecond and address:
//! 1. the one with the same browser time is that visit; when this capture already wrote or found
//!    it, the row is another of the visit's rows (Chrome writes a redirect's hops with one time);
//! 2. for Firefox, when one of the two times is a whole millisecond, the other row it has not been
//!    matched to: Sync gives a device's own visits back after a reset with new ids and times cut
//!    to the millisecond;
//! 3. a visit archived by version 1, which kept no browser time and is not yet matched: it takes
//!    the capture's time from then on;
//! 4. otherwise the row is a new visit.
//!
//! A row that matches nothing is always written; nothing is left out because it looks familiar.
use std::collections::{HashMap, HashSet};
use vault_store::{Result, Session, params};

/// An archived visit at a captured row's millisecond, with what an update compares.
#[derive(Debug, Clone)]
pub(super) struct Archived {
    pub(super) id: i64,
    pub(super) url: String,
    pub(super) key: Option<String>,
    pub(super) payload_hash: Option<String>,
    pub(super) parent_visit_id: Option<i64>,
    pub(super) import_batch_id: Option<i64>,
}

/// What a captured row is in the archive.
pub(super) enum Found {
    /// Rule 1 or 2: an archived visit an earlier capture wrote or matched.
    Same(Archived),
    /// Rule 1 again: another row, in this capture, of a visit this capture already wrote.
    Collapsed(Archived),
    /// Rule 3: a version 1 visit, to be given the row's time.
    Legacy(Archived),
    /// Rule 4.
    New,
}

/// One capture's rows and the archived visits they became. Its size is the capture's: a backup
/// that reads the visits after its last one holds those, a full read holds every visit in the
/// browser file (about 50 MB for two million).
#[derive(Default)]
pub(super) struct Capture {
    /// Browser row id to archive id, for the parents of later rows.
    pub(super) visits: HashMap<i64, i64>,
    /// Archive ids this capture wrote or matched: rules 2 and 3 never take one of these twice.
    claimed: HashSet<i64>,
    /// Visits whose opener the read has not reached yet: (archive id, the opener's browser row
    /// id). Sync writes older visits after newer ones, so many are found later in the read.
    pub(super) pending_openers: Vec<(i64, i64)>,
}

impl Capture {
    pub(super) fn record(&mut self, native_id: i64, archive_id: i64) {
        self.visits.insert(native_id, archive_id);
        self.claimed.insert(archive_id);
    }
}

/// The visits of `profile` at `time_ms`, through the profile/time index, which also holds the
/// visits hidden behind another source's copy. Usually one or two rows.
pub(super) fn archived_at(s: &Session<'_>, profile: i64, time_ms: i64) -> Result<Vec<Archived>> {
    s.query(
        "SELECT v.id,u.url,v.source_visit_time,v.payload_hash,v.parent_visit_id,v.import_batch_id
         FROM visits v INDEXED BY idx_visits_profile_time
         JOIN urls u ON u.id=v.url_id
         WHERE v.source_profile_id=?1 AND v.visit_time_ms=?2
         ORDER BY v.id",
        params![profile, time_ms],
        |r| {
            Ok(Archived {
                id: r.get(0)?,
                url: r.get(1)?,
                key: r.get(2)?,
                payload_hash: r.get(3)?,
                parent_visit_id: r.get(4)?,
                import_batch_id: r.get(5)?,
            })
        },
    )
}

/// What an import has seen so far of a file it previews, so the preview's "already in your
/// archive" is what the import then skips: archived visits it matched (a version 1 visit is
/// matched once, as the import stamps it once), and the identities the import gives a visit as
/// it goes, hashed: a new visit's, and a version 1 visit's once stamped. A later row with one of
/// those is another row of that visit, wherever it is in the file. About 8 MB for a million.
#[derive(Default)]
pub(crate) struct Preview {
    capture: Capture,
    written: HashSet<u64>,
}

impl Preview {
    /// Whether the import would not write the row as a new visit.
    pub(crate) fn already_archived(
        &mut self,
        s: &Session<'_>,
        profile: i64,
        family: &str,
        visit: &browser_history_parser::ParsedVisit,
        key: &str,
    ) -> Result<bool> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (&visit.url, key).hash(&mut hasher);
        let identity = hasher.finish();
        // The archive the preview reads does not change: a version 1 visit the import stamps
        // with this time still has none here, so `find` would not see the next row of it.
        if self.written.contains(&identity) {
            return Ok(true);
        }
        match find(s, profile, family, &visit.url, visit.visit_time_ms, key, &self.capture)? {
            Found::Same(archived) | Found::Collapsed(archived) => {
                self.capture.record(visit.source_visit_id, archived.id);
                Ok(true)
            }
            Found::Legacy(archived) => {
                self.capture.record(visit.source_visit_id, archived.id);
                self.written.insert(identity);
                Ok(true)
            }
            Found::New => {
                self.written.insert(identity);
                Ok(false)
            }
        }
    }
}

/// The archived visit a browser's record of a visit is, for an opener a read did not reach: the
/// same address and time, or a version 1 visit at that millisecond on that address.
pub(super) fn archived(
    s: &Session<'_>,
    profile: i64,
    mark: &browser_history_parser::native_visits::VisitMark,
) -> Result<Option<i64>> {
    let at = archived_at(s, profile, mark.time_ms)?;
    let same_address = || at.iter().filter(|a| a.url == mark.url);
    Ok(same_address()
        .find(|a| a.key.as_deref() == Some(mark.key.as_str()))
        .or_else(|| same_address().find(|a| a.key.is_none()))
        .map(|a| a.id))
}

/// Applies the rules above to one captured row with browser time `key`.
pub(super) fn find(
    s: &Session<'_>,
    profile: i64,
    family: &str,
    url: &str,
    time_ms: i64,
    key: &str,
    capture: &Capture,
) -> Result<Found> {
    let candidates =
        archived_at(s, profile, time_ms)?.into_iter().filter(|a| a.url == url).collect::<Vec<_>>();
    if let Some(same) = candidates.iter().find(|a| a.key.as_deref() == Some(key)) {
        return Ok(if capture.claimed.contains(&same.id) {
            Found::Collapsed(same.clone())
        } else {
            Found::Same(same.clone())
        });
    }
    let open = |a: &&Archived| !capture.claimed.contains(&a.id);
    if family == "firefox"
        && let Some(copy) = candidates.iter().filter(open).find(|a| {
            a.key
                .as_deref()
                .is_some_and(|archived| whole_millisecond(archived) || whole_millisecond(key))
        })
    {
        return Ok(Found::Same(copy.clone()));
    }
    if let Some(legacy) = candidates.iter().filter(open).find(|a| a.key.is_none()) {
        return Ok(Found::Legacy(legacy.clone()));
    }
    Ok(Found::New)
}

/// A Firefox time whose microseconds are a whole millisecond: what Sync writes.
fn whole_millisecond(key: &str) -> bool {
    key.strip_prefix("firefox-us:")
        .and_then(|micros| micros.parse::<i64>().ok())
        .is_some_and(|micros| micros.rem_euclid(1_000) == 0)
}

#[cfg(test)]
mod tests {
    use super::whole_millisecond;

    #[test]
    fn only_a_firefox_time_on_a_millisecond_reads_as_a_sync_copy() {
        assert!(whole_millisecond("firefox-us:1700000000123000"));
        assert!(!whole_millisecond("firefox-us:1700000000123456"));
        assert!(!whole_millisecond("chromium-us:13300000000123000"));
    }
}
