//! Responsibilities: find the archived visit a captured browser row is, or say it is new; keep,
//! for one capture, which archived visit each of its rows became.
//! Not responsible for: writing visits (`writes`) or choosing which rows a backup reads (`mod`).
//! Failure modes: a reused row id joins a new visit to an old one; a redirect's two rows with one
//! time become two visits; a Firefox Sync copy cut to the millisecond is counted as a second
//! visit, or merged into the one it copies and lost; a version 1 visit is saved again once its
//! browser row is renumbered, or two of them at one millisecond swap their times; two visits less
//! than a millisecond apart become one; a visit hidden behind another source's copy is not found
//! and is added again on every backup.
//!
//! A captured row is the visit with its page's address and its browser time, kept without loss
//! (`visits.source_visit_time`), and for Firefox its kind (transition): Firefox tells two visits
//! at one address and time apart by it, as Sync does. Among this profile's visits at the same
//! millisecond and address:
//! 1. the one with the same browser time (and Firefox kind) is that visit; when this capture
//!    already wrote or found it, the row is another of the visit's rows (Chrome writes a
//!    redirect's hops with one time and two kinds, which are one visit);
//! 2. for Firefox, when one of the two times is a whole millisecond, the other row of the same
//!    kind (transition) it has not been matched to is probably the same visit: Sync gives a
//!    device's own visits back after a reset with new ids, times cut to the millisecond and their
//!    kind kept, and itself tells a returned visit by its millisecond and kind. Probably, not
//!    surely, so the row is still written as its own visit, hidden behind that one and counted
//!    once (`same_as`), the way another source's copy is: both stay, nothing is merged;
//! 3. a visit archived by version 1, which kept no browser time and is not yet matched: it takes
//!    the capture's time from then on. Of several, the one of the row's kind, then the one read
//!    from its row id;
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
    pub(super) transition: Option<i64>,
    /// The browser row id it was read from, as text (a version 1 visit's only link to its row;
    /// Takeout's are synthetic).
    pub(super) source_visit_id: Option<String>,
    /// The visit it is a hidden copy of (`same-visit-across-sources.md`), when it is one.
    pub(super) same_as: Option<i64>,
    /// Whether the visit that counts for it does: not an undone import's.
    pub(super) counts: bool,
}

impl Archived {
    /// The visit that counts for it: itself, or the one it is a hidden copy of. Openers and the
    /// capture point there, so a link never ends at a visit no read shows.
    pub(super) fn counted(&self) -> i64 {
        self.same_as.unwrap_or(self.id)
    }
}

/// Whether `archived` is the visit a row with browser time `key` and kind `kind` records (rule 1):
/// the same time, and for Firefox the same kind. The one test of it, for the capture, the import
/// preview and the openers a read did not reach.
fn same_event(family: &str, archived: &Archived, key: &str, kind: Option<i64>) -> bool {
    archived.key.as_deref() == Some(key) && (family != "firefox" || archived.transition == kind)
}

/// What a captured row is in the archive.
pub(super) enum Found {
    /// Rule 1: an archived visit an earlier capture wrote or matched.
    Same(Archived),
    /// Rule 2: a Firefox Sync copy of this archived visit. It is written as its own visit, with
    /// its own time, hidden behind this one and counted once.
    SyncCopy(Archived),
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
    /// A row that is `copy`, a hidden copy of `counted`: the copy is taken, and the row's
    /// children open `counted`, the visit reads show. `counted` is not taken: its own row may
    /// come later in the read and is that visit, not another row of it.
    pub(super) fn record_copy(&mut self, native_id: i64, copy: i64, counted: i64) {
        self.claimed.insert(copy);
        self.visits.insert(native_id, counted);
    }

    /// Takes an archived visit without a row for it: a hidden copy the read matched another
    /// copy to, which no other row of this read may match again.
    pub(super) fn claim(&mut self, archive_id: i64) {
        self.claimed.insert(archive_id);
    }

    pub(super) fn record(&mut self, native_id: i64, archive_id: i64) {
        self.visits.insert(native_id, archive_id);
        self.claimed.insert(archive_id);
    }
}

/// The visits of `profile` at `time_ms`, through the profile/time index, which also holds the
/// visits hidden behind another source's copy. Usually one or two rows.
pub(super) fn archived_at(s: &Session<'_>, profile: i64, time_ms: i64) -> Result<Vec<Archived>> {
    s.query(
        "SELECT v.id,u.url,v.source_visit_time,v.payload_hash,v.parent_visit_id,v.import_batch_id,
                v.transition_type,v.source_visit_id,v.same_as,
                (SELECT g.reverted_at IS NULL FROM visits g WHERE g.id=COALESCE(v.same_as,v.id))
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
                transition: r.get(6)?,
                source_visit_id: r.get(7)?,
                same_as: r.get(8)?,
                counts: r.get(9)?,
            })
        },
    )
}

/// What an import has seen so far of a file it previews, so the preview's "already in your
/// archive" is what the import then skips: archived visits it matched (a version 1 visit is
/// matched once, as the import stamps it once), and the identities the import gives a visit as
/// it goes, hashed: a new visit's, and a version 1 visit's once stamped. A later row with one of
/// those is another row of that visit, wherever it is in the file. About 8 MB for a million.
/// Hashed because the address and time of every new row would take gigabytes at 14.4 million;
/// a collision (about one in 180,000 such previews) counts one new visit as already there, and
/// the import itself compares exact identities.
#[derive(Default)]
pub(crate) struct Preview {
    capture: Capture,
    written: HashSet<u64>,
}

/// What the import would do with a row: what its report then counts it as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Previewed {
    /// A visit the archive has: a duplicate.
    Archived,
    /// Written as a visit that counts.
    New,
    /// Written, hidden behind the visit it is a Firefox Sync copy of and counted with it: the
    /// report's "same visit".
    Copy,
}

impl Preview {
    /// What the import would do with the row. `profile` is `None` while the archive has never
    /// seen the file's profile: nothing is in the archive, but the file's own rows of one visit
    /// are still one visit.
    pub(crate) fn classify(
        &mut self,
        s: &Session<'_>,
        profile: Option<i64>,
        family: &str,
        visit: &browser_history_parser::ParsedVisit,
        key: &str,
    ) -> Result<Previewed> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (&visit.url, key, (family == "firefox").then_some(visit.transition)).hash(&mut hasher);
        let identity = hasher.finish();
        // The archive the preview reads does not change: a version 1 visit the import stamps
        // with this time, or a visit it writes, is not in it, so `find` would not see the next
        // row of it.
        if self.written.contains(&identity) {
            return Ok(Previewed::Archived);
        }
        let found = match profile {
            Some(profile) => find(s, profile, family, visit, key, &self.capture)?,
            None => Found::New,
        };
        Ok(match found {
            Found::Same(archived) | Found::Collapsed(archived) => {
                self.capture.record(visit.source_visit_id, archived.id);
                Previewed::Archived
            }
            Found::SyncCopy(archived) => {
                self.capture.record_copy(visit.source_visit_id, archived.id, archived.counted());
                self.written.insert(identity);
                Previewed::Copy
            }
            Found::Legacy(archived) => {
                self.capture.record(visit.source_visit_id, archived.id);
                self.written.insert(identity);
                Previewed::Archived
            }
            Found::New => {
                self.written.insert(identity);
                Previewed::New
            }
        })
    }
}

/// The archived visit a browser's record of a visit is, for an opener a read did not reach: the
/// same address and time (and Firefox kind), or a version 1 visit at that millisecond on that
/// address.
pub(super) fn archived(
    s: &Session<'_>,
    profile: i64,
    family: &str,
    mark: &browser_history_parser::native_visits::VisitMark,
) -> Result<Option<i64>> {
    let at = archived_at(s, profile, mark.time_ms)?;
    let same_address = || at.iter().filter(|a| a.url == mark.url);
    Ok(same_address()
        .find(|a| same_event(family, a, &mark.key, mark.kind))
        .or_else(|| same_address().find(|a| a.key.is_none()))
        .map(Archived::counted))
}

/// Applies the rules above to one captured row with browser time `key`.
pub(super) fn find(
    s: &Session<'_>,
    profile: i64,
    family: &str,
    visit: &browser_history_parser::ParsedVisit,
    key: &str,
    capture: &Capture,
) -> Result<Found> {
    let candidates = archived_at(s, profile, visit.visit_time_ms)?
        .into_iter()
        .filter(|a| a.url == visit.url)
        .collect::<Vec<_>>();
    if let Some(same) = candidates.iter().find(|a| same_event(family, a, key, visit.transition)) {
        return Ok(if capture.claimed.contains(&same.id) {
            Found::Collapsed(same.clone())
        } else {
            Found::Same(same.clone())
        });
    }
    let open = |a: &&Archived| !capture.claimed.contains(&a.id);
    // Only beside a visit that counts: a copy hidden behind an undone import's visit would not
    // be seen while its own import is.
    if family == "firefox"
        && let Some(copy) = candidates.iter().filter(open).filter(|a| a.counts).find(|a| {
            a.transition == visit.transition
                && a.key.as_deref().is_some_and(|archived| sync_pair(archived, key))
        })
    {
        return Ok(Found::SyncCopy(copy.clone()));
    }
    // Version 1 kept visits at one address and millisecond apart by their transition and title,
    // so several may wait for a time. The row's own: the one with its transition (a renumbered
    // file keeps it, not the row id), then the one read from its row id; the first only when
    // nothing tells them apart, where any is as good.
    if let Some(legacy) =
        candidates.iter().filter(open).filter(|a| a.key.is_none()).min_by_key(|a| {
            let row = a.source_visit_id.as_deref().and_then(|id| id.parse::<i64>().ok());
            (a.transition != visit.transition, row != Some(visit.source_visit_id))
        })
    {
        return Ok(Found::Legacy(legacy.clone()));
    }
    Ok(Found::New)
}

/// Whether two browser times at one millisecond are a Firefox Sync copy and the visit it copies
/// (rule 2's test, for visits of one profile, address and kind).
pub(crate) fn sync_pair(own: &str, other: &str) -> bool {
    let firefox = |key: &str| key.starts_with("firefox-us:");
    own != other
        && firefox(own)
        && firefox(other)
        && (whole_millisecond(own) || whole_millisecond(other))
}

/// A Firefox time whose microseconds are a whole millisecond: what Sync writes.
fn whole_millisecond(key: &str) -> bool {
    key.strip_prefix("firefox-us:")
        .and_then(|micros| micros.parse::<i64>().ok())
        .is_some_and(|micros| micros.rem_euclid(1_000) == 0)
}

#[cfg(test)]
mod tests {
    use super::{sync_pair, whole_millisecond};

    #[test]
    fn a_sync_copy_and_its_visit_both_have_firefox_times() {
        assert!(sync_pair("firefox-us:1700000000123000", "firefox-us:1700000000123456"));
        assert!(!sync_pair("firefox-us:1700000000123000", "chromium-us:13350000000123456"));
        assert!(!sync_pair("chromium-us:13350000000123456", "firefox-us:1700000000123000"));
        assert!(!sync_pair("firefox-us:1700000000123000", "firefox-us:1700000000123000"));
    }

    #[test]
    fn only_a_firefox_time_on_a_millisecond_reads_as_a_sync_copy() {
        assert!(whole_millisecond("firefox-us:1700000000123000"));
        assert!(!whole_millisecond("firefox-us:1700000000123456"));
        assert!(!whole_millisecond("chromium-us:13300000000123000"));
    }
}
