//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Persistent all-time `/intelligence` overview snapshot cache.
//!
//! ## Responsibilities
//! - Decide whether an all-time overview request can be served from a persisted
//!   snapshot instead of recomputing every heavy aggregate on each route open.
//! - Derive a content fingerprint from the archive watermark, taxonomy / rule
//!   versions, and the rebuild checkpoint ledger so a snapshot is reused only
//!   while nothing that could change the result has changed.
//! - Persist and reload the serialized primary / secondary overview payloads.
//!
//! ## Not responsible for
//! - Building the overview payloads (owned by `intelligence_overview`).
//! - Deciding when to warm the snapshot (the shell preloads it on idle).
//! - Any other scope: only all time and the recent ranges the page offers, over every profile
//!   (`RECENT_RANGE_DAYS`), are materialized; custom ranges and one profile's recent ranges are
//!   computed each time.
//!
//! ## Dependencies
//! - `incremental` for the per-profile source watermark and profile list.
//! - `visit_taxonomy` for the taxonomy version dimension.
//!
//! ## Performance notes
//! - The fingerprint runs on every all-time open *before* the cache lookup, so
//!   it must stay O(small) even at 14.4M visits. It reads the per-profile
//!   watermark from the small `core_intelligence_stage_checkpoints` ledger and
//!   detects un-rebuilt archive deltas with O(1) `MAX(visits.id)` / `runs`
//!   probes, instead of scanning the visible-visit index (and randomly probing
//!   `urls`) the way `load_profile_source_watermark` does. See
//!   `incremental::load_archive_delta_signature` / `list_fingerprint_profiles`
//!   for the EXPLAIN-backed rationale. The snapshot turns "recompute per open"
//!   into "recompute per data change".

use super::incremental::{
    ProfileSourceWatermark, list_fingerprint_profiles, load_archive_delta_signature,
    load_checkpoint_source_watermark, load_site_dictionary_signature,
};
use crate::models::{DateRange, ScopedDateRangeRequest};
use crate::utils::now_rfc3339;
use crate::visit_taxonomy::taxonomy_version;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Serialize;
use serde::de::DeserializeOwned;
use vault_store::{OptionalRow, Session, params_owned as params};

/// Sentinel start date the frontend emits for the `All time` preset. Kept in
/// sync with `ALL_TIME_DATE_RANGE_START` in `src/lib/core-intelligence/hooks.ts`;
/// the two constants form the all-time transport contract.
pub(crate) const ALL_TIME_SCOPE_START: &str = "1900-01-01";

/// Selects which staged overview band a snapshot row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverviewBand {
    Primary,
    Secondary,
}

impl OverviewBand {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }
}

/// Returns true when a request targets the all-time scope. Detected by the shared sentinel start
/// date so the transport stays a plain `DateRange`.
pub(super) fn is_all_time_range(date_range: &DateRange) -> bool {
    date_range.start == ALL_TIME_SCOPE_START
}

/// The recent ranges the page offers (`presetDays` in `src/features/insights/range.ts`), each
/// ending today. Their overviews over every profile are kept like all time's, so the range toggle
/// answers at once whatever the archive's size.
pub(super) const RECENT_RANGE_DAYS: [i64; 4] = [365, 90, 30, 7];

/// The kept snapshot a request reads: all time (every profile, or one), or a recent range ending
/// today over every profile. `None` for any other range, which is computed each time.
pub(super) fn snapshot_key(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<Option<String>> {
    if is_all_time_range(&request.date_range) {
        return Ok(Some(all_time_scope_key(request.profile_id.as_deref())));
    }
    if request.profile_id.is_some() {
        return Ok(None);
    }
    let today = super::InsightsZone::for_reading(connection)?.today_key();
    Ok(recent_range_days(&request.date_range, &today).map(|days| format!("recent:{days}")))
}

/// How many days `range` covers when it is one of the recent ranges ending `today`.
fn recent_range_days(range: &DateRange, today: &str) -> Option<i64> {
    let day = |key: &str| NaiveDate::parse_from_str(key, "%Y-%m-%d").ok();
    let days = (day(&range.end)? - day(&range.start)?).num_days() + 1;
    (range.end == today && RECENT_RANGE_DAYS.contains(&days)).then_some(days)
}

/// The recent ranges ending `today`, over every profile: the requests a build warms.
pub(super) fn recent_range_requests(today: &str) -> Vec<ScopedDateRangeRequest> {
    let Ok(end) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else { return Vec::new() };
    RECENT_RANGE_DAYS
        .iter()
        .map(|days| ScopedDateRangeRequest {
            date_range: DateRange {
                start: (end - chrono::Duration::days(days - 1)).format("%Y-%m-%d").to_string(),
                end: today.to_owned(),
            },
            profile_id: None,
        })
        .collect()
}

/// Builds the snapshot scope key, distinguishing archive-wide from
/// profile-scoped all-time views so they never reuse each other's payload.
pub(super) fn all_time_scope_key(profile_id: Option<&str>) -> String {
    match profile_id {
        Some(profile_id) => format!("all-time:profile:{profile_id}"),
        None => "all-time:archive-wide".to_string(),
    }
}

/// Resolves one overview band for a request with a kept snapshot (`snapshot_key`), returning it
/// when its fingerprint still matches the current archive state and otherwise recomputing and
/// returning fresh data; any other request is computed.
///
/// `build` is only invoked on a cache miss or fingerprint change, so steady-state
/// opens never pay the all-time aggregation cost. A serialization or persistence
/// failure degrades to returning freshly built data rather than erroring.
pub(super) fn resolve_overview_snapshot<T, F>(
    connection: &Session<'_>,
    band: OverviewBand,
    request: &ScopedDateRangeRequest,
    build: F,
) -> Result<T>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Result<T>,
{
    let Some(scope_key) = snapshot_key(connection, request)? else { return build() };
    let fingerprint = request_fingerprint(connection, request)?;
    if let Some(existing) = load_overview_snapshot(connection, &scope_key, band)?
        && existing.fingerprint == fingerprint
        && let Ok(payload) = serde_json::from_str::<T>(&existing.payload_json)
    {
        return Ok(payload);
    }
    let payload = build()?;
    Ok(payload)
}

/// The fingerprint a request with a kept snapshot is served under. All time always ends today, so
/// its end date only says which day it was asked and is left out; the sections that depend on
/// today are recomputed when a snapshot is served (`intelligence_overview::refresh_for_today`).
/// A recent range's days move every day, so they are in its fingerprint: a snapshot from
/// yesterday is not served today.
pub(super) fn request_fingerprint(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<String> {
    let content = compute_overview_fingerprint(connection, request.profile_id.as_deref())?;
    Ok(if is_all_time_range(&request.date_range) {
        content
    } else {
        format!("{content}|days={}..{}", request.date_range.start, request.date_range.end)
    })
}

/// Computes the content fingerprint for one all-time scope. The snapshot is
/// reused only while this value is stable, so it must fold in every input that
/// can change an aggregate: the archive watermark (raw visits / search terms),
/// the taxonomy / site-dictionary / search-engine-rule versions, the rebuild
/// checkpoint ledger (which moves on every derived rebuild or clear), an O(1)
/// archive-delta probe (ingest/reverts not yet rebuilt), and the UTC offset,
/// since a visit's local day depends on it. The leading `v` changes with what a
/// snapshot holds (v4: each section's evidence count; v5: a page topic's site, for its icon;
/// v6: searches merged across profiles and engines into one row per wording), so older ones are
/// rebuilt.
///
/// Today's date is deliberately not in it. It used to be, for "On this day";
/// then the first open of every day recomputed all of history (seconds at
/// 387k visits, far more at 14.4M), and nothing saved the result until the next
/// rebuild. The parts that depend on today are recomputed when served instead.
pub(super) fn compute_overview_fingerprint(
    connection: &Session<'_>,
    profile_id: Option<&str>,
) -> Result<String> {
    let profiles = list_fingerprint_profiles(connection, profile_id)?;
    let mut aggregate = ProfileSourceWatermark::default();
    for profile in &profiles {
        let watermark = load_checkpoint_source_watermark(connection, profile)?;
        aggregate.visible_visit_count += watermark.visible_visit_count;
        aggregate.visible_search_term_count += watermark.visible_search_term_count;
        aggregate.max_visit_id = aggregate.max_visit_id.max(watermark.max_visit_id);
        aggregate.max_url_last_visit_ms =
            aggregate.max_url_last_visit_ms.max(watermark.max_url_last_visit_ms);
    }
    let archive_delta_signature = load_archive_delta_signature(connection)?;
    let checkpoint_signature = load_checkpoint_signature(connection, profile_id)?;
    let site_dictionary_signature = load_site_dictionary_signature(connection)?;
    let search_rule_signature = load_search_rule_signature(connection)?;
    Ok(format!(
        "v6|profiles={}|visits={}|maxVisit={}|maxUrlMs={}|terms={}|tax={}|{}|{}|{}|{}|tz={}",
        profiles.len(),
        aggregate.visible_visit_count,
        aggregate.max_visit_id,
        aggregate.max_url_last_visit_ms,
        aggregate.visible_search_term_count,
        taxonomy_version(),
        site_dictionary_signature,
        search_rule_signature,
        checkpoint_signature,
        archive_delta_signature,
        super::InsightsZone::for_reading(connection)?.name(),
    ))
}

/// One persisted snapshot row.
pub(super) struct OverviewSnapshotRow {
    pub fingerprint: String,
    pub payload_json: String,
}

/// Loads a snapshot row when one exists for the scope and band.
pub(super) fn load_overview_snapshot(
    connection: &Session<'_>,
    scope_key: &str,
    band: OverviewBand,
) -> Result<Option<OverviewSnapshotRow>> {
    let row = connection
        .query_one(
            "SELECT fingerprint, payload_json
             FROM intelligence_overview_snapshots
             WHERE scope_key = ?1 AND band = ?2",
            params![scope_key, band.as_str()],
            |row| Ok(OverviewSnapshotRow { fingerprint: row.get(0)?, payload_json: row.get(1)? }),
        )
        .optional()?;
    Ok(row)
}

/// Inserts or replaces the snapshot row for one scope and band.
pub(super) fn save_overview_snapshot(
    connection: &Session<'_>,
    scope_key: &str,
    band: OverviewBand,
    fingerprint: &str,
    payload_json: &str,
) -> Result<()> {
    connection.execute(
        "INSERT INTO intelligence_overview_snapshots
           (scope_key, band, fingerprint, payload_json, computed_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(scope_key, band) DO UPDATE SET
           fingerprint = excluded.fingerprint,
           payload_json = excluded.payload_json,
           computed_at = excluded.computed_at",
        params![scope_key, band.as_str(), fingerprint, payload_json, now_rfc3339()],
    )?;
    Ok(())
}

/// Drops every persisted snapshot. Used by full derived-state clears and archive
/// repair, where the derived tables change without moving the archive watermark.
pub(super) fn clear_overview_snapshots(connection: &Session<'_>) -> Result<usize> {
    let cleared = connection.execute("DELETE FROM intelligence_overview_snapshots", ())?;
    Ok(cleared as usize)
}

/// Summarizes the rebuild checkpoint ledger so any derived rebuild or clear
/// changes the fingerprint even when the canonical archive watermark is stable.
fn load_checkpoint_signature(connection: &Session<'_>, profile_id: Option<&str>) -> Result<String> {
    let (count, max_run_id, max_updated): (i64, i64, String) = if let Some(profile_id) = profile_id
    {
        connection.query_one(
            "SELECT COUNT(*), COALESCE(MAX(last_run_id), 0), COALESCE(MAX(updated_at), 'none')
             FROM core_intelligence_stage_checkpoints
             WHERE profile_id = ?1",
            [profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?
    } else {
        connection.query_one(
            "SELECT COUNT(*), COALESCE(MAX(last_run_id), 0), COALESCE(MAX(updated_at), 'none')
             FROM core_intelligence_stage_checkpoints",
            (),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?
    };
    Ok(format!("ckpt:{count}:{max_run_id}:{max_updated}"))
}

/// Summarizes mutable search-engine rules so editing them (which reshapes query
/// families and search trails) invalidates the snapshot even though no canonical
/// visit changed.
pub(crate) fn load_search_rule_signature(connection: &Session<'_>) -> Result<String> {
    connection
        .query_one(
            "SELECT COUNT(*), COALESCE(SUM(enabled), 0), COALESCE(MAX(updated_at), 'none')
             FROM search_engine_rules",
            (),
            |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?, row.get::<String>(2)?)),
        )
        .map(|(count, enabled, updated)| format!("srules:{count}:{enabled}:{updated}"))
        .context("loading search engine rule signature")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::native::TestDatabase;
    use vault_store::{Db, Priority};
    fn open_schema_connection() -> TestDatabase {
        let vault = crate::Vault::open_temp().unwrap();
        TestDatabase::new(&vault, Db::Intelligence)
    }
    fn open_fingerprint_connection() -> TestDatabase {
        open_schema_connection()
    }
    fn fingerprint(db: &TestDatabase, profile: Option<&str>) -> Result<String> {
        let profile = profile.map(str::to_owned);
        db.read(move |s| compute_overview_fingerprint(s, profile.as_deref()))
    }
    fn save(
        db: &TestDatabase,
        scope: &str,
        band: OverviewBand,
        fp: &str,
        json: &str,
    ) -> Result<()> {
        let (scope, fp, json) = (scope.to_owned(), fp.to_owned(), json.to_owned());
        db.write(move |s| save_overview_snapshot(s, &scope, band, &fp, &json))
    }
    fn resolve(
        db: &TestDatabase,
        request: &ScopedDateRangeRequest,
        value: i64,
        calls: &mut u32,
    ) -> i64 {
        let request = request.clone();
        let (result, built) = db
            .read(move |s| {
                let mut built = false;
                let result = resolve_overview_snapshot(s, OverviewBand::Primary, &request, || {
                    built = true;
                    Ok(value)
                })?;
                Ok((result, built))
            })
            .unwrap();
        *calls += u32::from(built);
        result
    }
    fn warm(db: &TestDatabase, request: &ScopedDateRangeRequest, value: i64) {
        let request_copy = request.clone();
        let fp = db.read(move |s| request_fingerprint(s, &request_copy)).unwrap();
        save(
            db,
            &all_time_scope_key(request.profile_id.as_deref()),
            OverviewBand::Primary,
            &fp,
            &value.to_string(),
        )
        .unwrap();
    }
    fn append_visit(db: &TestDatabase) {
        db.vault.write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')",())?;
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://example.com/','https://example.com/','example.com')",())?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,1,'',1,'')",())?;
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(1,1,1,0,1,'')",())?;Ok(())
        }).unwrap();
    }
    fn append_run(db: &TestDatabase, id: i64, kind: &str) {
        let kind = kind.to_owned();
        db.vault.write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO runs(id,run_type,trigger,started_at,timezone,status,profile_scope_json,warnings_json,stats_json,due_only) VALUES(?1,?2,'test','','UTC','success','[]','[]','{}',0)",params![id,kind])?;Ok(())
        }).unwrap();
    }

    // Recent ranges: what goes wrong if the scope rules do —
    // - a range the page does not offer is kept (every build would compute it for nothing);
    // - yesterday's "last 30 days" is served today (its days moved);
    // - one profile's recent range is served from the snapshot of every profile.
    #[test]
    fn recent_ranges_are_kept_for_today_over_every_profile() {
        let db = open_schema_connection();
        let today =
            db.read(|s| Ok(super::super::InsightsZone::for_reading(s)?.today_key())).unwrap();
        let requests = recent_range_requests(&today);
        assert_eq!(requests.len(), RECENT_RANGE_DAYS.len());
        let day = |key: &str| NaiveDate::parse_from_str(key, "%Y-%m-%d").unwrap();
        let shifted = |request: &ScopedDateRangeRequest, by: i64| ScopedDateRangeRequest {
            date_range: DateRange {
                start: (day(&request.date_range.start) + chrono::Duration::days(by))
                    .format("%Y-%m-%d")
                    .to_string(),
                end: (day(&request.date_range.end) + chrono::Duration::days(by))
                    .format("%Y-%m-%d")
                    .to_string(),
            },
            profile_id: request.profile_id.clone(),
        };
        let key =
            |request: ScopedDateRangeRequest| db.read(move |s| snapshot_key(s, &request)).unwrap();
        let print = |request: ScopedDateRangeRequest| {
            db.read(move |s| request_fingerprint(s, &request)).unwrap()
        };
        for (request, days) in requests.iter().zip(RECENT_RANGE_DAYS) {
            assert_eq!(key(request.clone()), Some(format!("recent:{days}")));
            assert_eq!(key(shifted(request, -1)), None, "{days} days ending yesterday");
            let one_profile = ScopedDateRangeRequest {
                profile_id: Some("chrome:Default".into()),
                ..request.clone()
            };
            assert_eq!(key(one_profile), None);
            assert_ne!(print(request.clone()), print(shifted(request, -1)));
        }
        let thirty_one = ScopedDateRangeRequest {
            date_range: DateRange {
                start: (day(&today) - chrono::Duration::days(30)).format("%Y-%m-%d").to_string(),
                end: today.clone(),
            },
            profile_id: None,
        };
        assert_eq!(key(thirty_one), None, "31 days is not a range the page offers");
    }

    /// Inserting a raw archive visit (and the backup `runs` row that always
    /// accompanies ingest) must change the fingerprint *and* force the builder
    /// to run again, even though the checkpoint ledger has not been rebuilt yet.
    /// This guards the cache's core promise: do not serve the stale all-time
    /// snapshot after a backup adds data.
    #[test]
    fn resolve_rebuilds_when_archive_gains_un_ingested_visits() {
        let connection = open_fingerprint_connection();
        let request = ScopedDateRangeRequest {
            date_range: DateRange {
                start: ALL_TIME_SCOPE_START.to_string(),
                end: "2026-06-20".to_string(),
            },
            profile_id: None,
        };

        let before = fingerprint(&connection, None).expect("before fingerprint");

        let mut build_calls = 0u32;
        let first = resolve(&connection, &request, 1, &mut build_calls);
        assert_eq!(first, 1);
        assert_eq!(build_calls, 1);
        warm(&connection, &request, 1);

        // Simulate a backup: a new visit row plus the run row ingest records.
        append_visit(&connection);
        append_run(&connection, 1, "backup");

        let after = fingerprint(&connection, None).expect("after fingerprint");
        assert_ne!(before, after, "an un-ingested archive delta must change the fingerprint");

        let second = resolve(&connection, &request, 2, &mut build_calls);
        assert_eq!(second, 2, "the snapshot must be rebuilt, not served stale");
        assert_eq!(build_calls, 2);
    }

    /// A revert that touches only existing visits leaves `MAX(visits.id)`
    /// unchanged, so the fingerprint must still move via the `runs` ledger.
    #[test]
    fn fingerprint_reacts_to_revert_runs_without_new_visits() {
        let connection = open_fingerprint_connection();
        append_visit(&connection);
        let before = fingerprint(&connection, None).expect("before fingerprint");
        // A repair/revert run adds no visit row but always records a run.
        append_run(&connection, 7, "repair");
        let after = fingerprint(&connection, None).expect("after fingerprint");
        assert_ne!(before, after);
    }

    /// A snapshot outlives the day it was built on: the fingerprint leaves the
    /// date out (the parts that depend on today are recomputed when served), and
    /// two all-time requests asked on different days share it.
    #[test]
    fn fingerprint_outlives_the_day_but_not_the_timezone() {
        let connection = open_fingerprint_connection();
        let fingerprint = fingerprint(&connection, None).expect("fingerprint");
        assert!(!fingerprint.contains("day="), "{fingerprint} must not hold today's date");
        // Before any build the zone is the computer's.
        let zone = super::super::InsightsZone::system();
        assert!(fingerprint.ends_with(&format!("tz={}", zone.name())), "{fingerprint}");
        let asked_on = |end: &str| {
            let request = ScopedDateRangeRequest {
                date_range: DateRange { start: ALL_TIME_SCOPE_START.into(), end: end.into() },
                profile_id: None,
            };
            connection.read(move |s| request_fingerprint(s, &request)).expect("fingerprint")
        };
        assert_eq!(asked_on("2026-10-07"), asked_on("2026-10-08"));
    }
}
