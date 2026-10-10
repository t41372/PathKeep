//! Responsibilities: the same visit recorded by two sources — a Google Takeout import and a
//! browser, or two imports — kept in both and counted once
//! (`docs/architecture/same-visit-across-sources.md`). One copy stands in for the visit; the
//! others point at it (`same_as`) and are hidden the way an undone import is (`reverted_at`), so
//! every read, total, search document and Insights build counts the stand-in only.
//! Not responsible for: which visits a source has (ingest), undoing an import (`takeout::
//! visibility`, which calls `promote` and `link` here), or showing the copies (History details).
//! Failure modes: two browsers' own visits merged (only Takeout copies are linked); a copy that
//! outranks the stand-in left hidden (the browser's record must stand in); a group with two
//! stand-ins or a copy pointing at a hidden copy (groups stay flat); a lookup per visit when the
//! archive has nothing to link with (each chunk asks first which sources could hold a copy); a
//! statement run once per copy that reads every visit.
//! Tests: `same_visit_tests.rs` (plans), T3 and T8–T10 in `ingest/dedup_scenarios_takeout.rs`.

use crate::utils::now_rfc3339;
use vault_store::{Result, Session, params};

/// Copies further apart than this are separate visits. Measured on real data, the copies of
/// one visit are 0 ms apart; the same page opened again is minutes later.
pub(crate) const SAME_VISIT_MS: i64 = 1_000;

/// Which copy stands in for a visit, best first: the browser's own record, then Takeout's copy
/// of Chrome's history, then My Activity's.
pub(crate) fn rank(profile_key: &str) -> u8 {
    if profile_key == "takeout::my-activity" {
        2
    } else if profile_key.starts_with("takeout::") {
        1
    } else {
        0
    }
}

/// The sources a chunk of one source's visits can share visits with.
#[derive(Debug, Clone, Default)]
pub(crate) struct Candidates {
    own_rank: u8,
    /// `source_profiles.id` → rank.
    ranks: Vec<(i64, u8)>,
    /// The ids as an SQL list.
    list: String,
}

impl Candidates {
    pub(crate) fn is_empty(&self) -> bool {
        self.ranks.is_empty()
    }

    fn rank_of(&self, profile: i64) -> u8 {
        self.ranks.iter().find(|(id, _)| *id == profile).map_or(0, |(_, rank)| *rank)
    }
}

/// The sources whose counted visits between `from_ms` and `to_ms` a visit of `profile` can be
/// the same as: any other source for a Takeout import, only Takeout imports for a browser (two
/// browsers record their own visits). Empty when none has a visit in that time, so the chunk
/// looks nothing up.
pub(crate) fn candidates(
    s: &Session<'_>,
    profile: i64,
    from_ms: i64,
    to_ms: i64,
) -> Result<Candidates> {
    // Archives from v0.2 can hold a profile without a key: a browser's, as far as ranks go.
    let profiles =
        s.query("SELECT id, COALESCE(profile_key, '') FROM source_profiles", (), |r| {
            Ok((r.get::<i64>(0)?, r.get::<String>(1)?))
        })?;
    let own_rank = profiles.iter().find(|(id, _)| *id == profile).map_or(0, |(_, key)| rank(key));
    let mut ranks = Vec::new();
    for (id, key) in profiles {
        let other_rank = rank(&key);
        if id == profile || (own_rank == 0 && other_rank == 0) {
            continue;
        }
        let overlaps = s.query_one(
            OVERLAPS_SQL,
            params![id, from_ms - SAME_VISIT_MS, to_ms + SAME_VISIT_MS],
            |r| Ok(r.get::<i64>(0)? != 0),
        )?;
        if overlaps {
            ranks.push((id, other_rank));
        }
    }
    let list = ranks.iter().map(|(id, _)| id.to_string()).collect::<Vec<_>>().join(",");
    Ok(Candidates { own_rank, ranks, list })
}

const OVERLAPS_SQL: &str =
    "SELECT EXISTS(SELECT 1 FROM visits INDEXED BY idx_visits_visible_profile_time_id
    WHERE source_profile_id=?1 AND reverted_at IS NULL AND visit_time_ms BETWEEN ?2 AND ?3)";

/// The counted copy of a visit, found in one of the candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StandIn {
    pub visit: i64,
    pub profile: i64,
    pub page: i64,
    rank: u8,
}

impl StandIn {
    /// Whether a new copy from the candidates' own source takes the stand-in's place.
    pub(crate) fn outranked_by(&self, candidates: &Candidates) -> bool {
        candidates.own_rank < self.rank
    }
}

/// The counted copy of the visit to `url` at `at_ms` among `candidates`, the nearest in time.
pub(crate) fn stand_in(
    s: &Session<'_>,
    candidates: &Candidates,
    url: &str,
    at_ms: i64,
) -> Result<Option<StandIn>> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let found = s
        .query(
            &stand_in_sql(&candidates.list),
            params![url, at_ms - SAME_VISIT_MS, at_ms + SAME_VISIT_MS, at_ms],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?)),
        )?
        .pop();
    Ok(found.map(|(visit, profile, page)| StandIn {
        visit,
        profile,
        page,
        rank: candidates.rank_of(profile),
    }))
}

/// `profiles`: the candidates' ids as an SQL list.
fn stand_in_sql(profiles: &str) -> String {
    format!(
        "SELECT v.id, v.source_profile_id, u.page_id FROM urls u INDEXED BY idx_urls_url
         JOIN visits v INDEXED BY idx_visits_visible_url_time ON v.url_id=u.id
         WHERE u.url=?1 AND u.source_profile_id IN ({profiles}) AND v.reverted_at IS NULL
           AND v.visit_time_ms BETWEEN ?2 AND ?3
         ORDER BY abs(v.visit_time_ms-?4), v.id LIMIT 1"
    )
}

/// Hides `visit` behind `stand_in`; copies that pointed at `visit` follow it.
/// `visit` was counted: its import's visible count goes down with it.
pub(crate) fn hide_behind(s: &Session<'_>, run: i64, visit: i64, stand_in: i64) -> Result<()> {
    s.execute(HIDE_FROM_BATCH_SQL, [visit])?;
    s.execute(HIDE_SQL, params![now_rfc3339(), run, stand_in, visit])?;
    s.execute(FOLLOW_SQL, params![stand_in, visit])?;
    Ok(())
}

const HIDE_FROM_BATCH_SQL: &str = "UPDATE import_batches SET visible_items=visible_items-1
    WHERE id=(SELECT import_batch_id FROM visits WHERE id=?1)";
const HIDE_SQL: &str =
    "UPDATE visits SET reverted_at=?1, reverted_by_run_id=?2, same_as=?3 WHERE id=?4";

/// Points the copies of a visit at its new stand-in (`?1`) instead of the old one (`?2`).
/// `same_as IS NOT NULL` lets the partial index answer; without it every visit is read, once
/// per copy hidden.
const FOLLOW_SQL: &str = "UPDATE visits SET same_as=?1 WHERE same_as IS NOT NULL AND same_as=?2";

/// A copy that now stands in for its visit, after the stand-in was undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Promoted {
    pub visit: i64,
    pub profile: i64,
}

/// When the stand-in `visit` is undone (its import reverted), the best of its copies that are
/// not undone themselves (in `batch`, or in another reverted import) stands in instead.
pub(crate) fn promote(s: &Session<'_>, visit: i64, batch: i64) -> Result<Option<Promoted>> {
    let copies = s.query(COPIES_SQL, params![visit, batch], |r| {
        Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<String>(2)?))
    })?;
    let Some(&(best, profile, _)) = copies.iter().min_by_key(|(id, _, key)| (rank(key), *id))
    else {
        return Ok(None);
    };
    s.execute(PROMOTE_SQL, [best])?;
    s.execute(FOLLOW_SQL, params![best, visit])?;
    s.execute(SHOW_IN_BATCH_SQL, [best])?;
    Ok(Some(Promoted { visit: best, profile }))
}

const COPIES_SQL: &str = "SELECT v.id, v.source_profile_id, COALESCE(p.profile_key, '')
    FROM visits v INDEXED BY idx_visits_same_as
    JOIN source_profiles p ON p.id=v.source_profile_id
    LEFT JOIN import_batches b ON b.id=v.import_batch_id
    WHERE v.same_as IS NOT NULL AND v.same_as=?1 AND (v.import_batch_id IS NULL
      OR (v.import_batch_id!=?2 AND b.status NOT IN ('reverted','reverting')))";
const PROMOTE_SQL: &str =
    "UPDATE visits SET reverted_at=NULL, reverted_by_run_id=NULL, same_as=NULL WHERE id=?1";
const SHOW_IN_BATCH_SQL: &str = "UPDATE import_batches SET visible_items=visible_items+1
    WHERE id=(SELECT import_batch_id FROM visits WHERE id=?1)";

/// What linking one counted visit changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Linked {
    /// No other source has it.
    Alone,
    /// Another source's copy stands in; this one is now hidden behind it.
    Hidden { stand_in: StandIn },
    /// This one outranks the copy that stood in, which is now hidden behind it.
    Replaced { stand_in: StandIn },
}

/// Links an existing counted visit (a restored import's) with the visit's other copies.
pub(crate) fn link(
    s: &Session<'_>,
    run: i64,
    visit: i64,
    profile: i64,
    url: &str,
    at_ms: i64,
) -> Result<Linked> {
    let candidates = candidates(s, profile, at_ms, at_ms)?;
    let Some(stand_in) = self::stand_in(s, &candidates, url, at_ms)? else {
        return Ok(Linked::Alone);
    };
    if stand_in.outranked_by(&candidates) {
        hide_behind(s, run, stand_in.visit, visit)?;
        Ok(Linked::Replaced { stand_in })
    } else {
        hide_behind(s, run, visit, stand_in.visit)?;
        Ok(Linked::Hidden { stand_in })
    }
}

/// The profile keys of `profiles` (`source_profiles.id`), for reports.
pub(crate) fn profile_keys(s: &Session<'_>, profiles: &[i64]) -> Result<Vec<String>> {
    let mut keys = Vec::with_capacity(profiles.len());
    for id in profiles {
        keys.extend(s.query(
            "SELECT profile_key FROM source_profiles WHERE id=?1 AND profile_key IS NOT NULL",
            [*id],
            |r| Ok(r.get::<String>(0)?),
        )?);
    }
    Ok(keys)
}

/// Links one source's counted visits — `(visit, url, at_ms)` — with their other copies, as a
/// write would have (an archive written before linking existed). Candidates are found once for
/// the rows' span.
pub(crate) fn link_existing(
    s: &Session<'_>,
    run: i64,
    profile: i64,
    rows: &[(i64, String, i64)],
) -> Result<()> {
    let span = rows.iter().map(|(_, _, at)| *at);
    let (Some(from), Some(to)) = (span.clone().min(), span.max()) else { return Ok(()) };
    let candidates = candidates(s, profile, from, to)?;
    for (visit, url, at_ms) in rows {
        let Some(stand_in) = self::stand_in(s, &candidates, url, *at_ms)? else { continue };
        if stand_in.outranked_by(&candidates) {
            hide_behind(s, run, stand_in.visit, *visit)?;
        } else {
            hide_behind(s, run, *visit, stand_in.visit)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "same_visit_tests.rs"]
mod tests;
