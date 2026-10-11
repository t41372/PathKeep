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
//! - Any scope other than all-time; only all-time is materialized.
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

/// Returns true when a request targets the all-time scope, the only scope the
/// snapshot cache materializes. Detected by the shared sentinel start date so
/// the transport stays a plain `DateRange`.
pub(super) fn is_all_time_range(date_range: &DateRange) -> bool {
    date_range.start == ALL_TIME_SCOPE_START
}

/// Builds the snapshot scope key, distinguishing archive-wide from
/// profile-scoped all-time views so they never reuse each other's payload.
pub(super) fn all_time_scope_key(profile_id: Option<&str>) -> String {
    match profile_id {
        Some(profile_id) => format!("all-time:profile:{profile_id}"),
        None => "all-time:archive-wide".to_string(),
    }
}

/// Resolves one overview band for an all-time request, returning a persisted
/// snapshot when its fingerprint still matches the current archive state and
/// otherwise recomputing, persisting, and returning fresh data.
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
    let scope_key = all_time_scope_key(request.profile_id.as_deref());
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

/// The fingerprint an all-time request is served under. All time always ends
/// today, so its end date only says which day it was asked and is left out; the
/// sections that depend on today are recomputed when a snapshot is served
/// (`intelligence_overview::refresh_for_today`).
pub(super) fn request_fingerprint(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<String> {
    compute_overview_fingerprint(connection, request.profile_id.as_deref())
}

/// Computes the content fingerprint for one all-time scope. The snapshot is
/// reused only while this value is stable, so it must fold in every input that
/// can change an aggregate: the archive watermark (raw visits / search terms),
/// the taxonomy / site-dictionary / search-engine-rule versions, the rebuild
/// checkpoint ledger (which moves on every derived rebuild or clear), an O(1)
/// archive-delta probe (ingest/reverts not yet rebuilt), and the UTC offset,
/// since a visit's local day depends on it.
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
        "v3|profiles={}|visits={}|maxVisit={}|maxUrlMs={}|terms={}|tax={}|{}|{}|{}|{}|tz={}",
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
pub(super) fn load_search_rule_signature(connection: &Session<'_>) -> Result<String> {
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
    fn load(
        db: &TestDatabase,
        scope: &str,
        band: OverviewBand,
    ) -> Result<Option<OverviewSnapshotRow>> {
        let scope = scope.to_owned();
        db.read(move |s| load_overview_snapshot(s, &scope, band))
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

    /// Inserts one stage-checkpoint row carrying the given source watermark, so
    /// fingerprint tests can move a single watermark dimension at a time.
    fn insert_checkpoint_with_watermark(
        connection: &TestDatabase,
        profile_id: &str,
        stage: &str,
        visible_visit_count: i64,
        max_visit_id: i64,
        max_url_last_visit_ms: i64,
        visible_search_term_count: i64,
    ) {
        connection
            .execute(
                "INSERT INTO core_intelligence_stage_checkpoints
                   (profile_id, stage, stage_version,
                    visible_visit_count, max_visit_id, max_url_last_visit_ms,
                    visible_search_term_count, updated_at, last_run_id)
                 VALUES (?1, ?2, 'v', ?3, ?4, ?5, ?6, '2026-06-20T00:00:00Z', 1)",
                params![
                    profile_id,
                    stage,
                    visible_visit_count,
                    max_visit_id,
                    max_url_last_visit_ms,
                    visible_search_term_count,
                ],
            )
            .expect("insert checkpoint watermark");
    }

    #[test]
    fn detects_all_time_range_by_sentinel_start() {
        assert!(is_all_time_range(&DateRange {
            start: ALL_TIME_SCOPE_START.to_string(),
            end: "2026-06-20".to_string(),
        }));
        assert!(!is_all_time_range(&DateRange {
            start: "2026-05-01".to_string(),
            end: "2026-06-20".to_string(),
        }));
    }

    #[test]
    fn scope_key_separates_archive_wide_from_profile() {
        assert_eq!(all_time_scope_key(None), "all-time:archive-wide");
        assert_eq!(all_time_scope_key(Some("chrome:Default")), "all-time:profile:chrome:Default");
    }

    #[test]
    fn snapshot_round_trips_and_overwrites() {
        let connection = open_schema_connection();
        save(&connection, "all-time:archive-wide", OverviewBand::Primary, "fp1", "{\"a\":1}")
            .expect("save");
        let row = load(&connection, "all-time:archive-wide", OverviewBand::Primary)
            .expect("load")
            .expect("row present");
        assert_eq!(row.fingerprint, "fp1");
        assert_eq!(row.payload_json, "{\"a\":1}");

        save(&connection, "all-time:archive-wide", OverviewBand::Primary, "fp2", "{\"a\":2}")
            .expect("overwrite");
        let row = load(&connection, "all-time:archive-wide", OverviewBand::Primary)
            .expect("load")
            .expect("row present");
        assert_eq!(row.fingerprint, "fp2");
        assert_eq!(row.payload_json, "{\"a\":2}");

        // The secondary band is stored independently of the primary band.
        assert!(
            load(&connection, "all-time:archive-wide", OverviewBand::Secondary)
                .expect("load secondary")
                .is_none()
        );
    }

    #[test]
    fn clear_removes_all_snapshots() {
        let connection = open_schema_connection();
        save(&connection, "all-time:archive-wide", OverviewBand::Primary, "fp", "{}")
            .expect("save");
        assert_eq!(connection.write(clear_overview_snapshots).expect("clear"), 1);
        assert!(
            load(&connection, "all-time:archive-wide", OverviewBand::Primary)
                .expect("load")
                .is_none()
        );
    }

    #[test]
    fn resolve_serves_snapshot_until_fingerprint_changes() {
        let connection = open_fingerprint_connection();
        let request = ScopedDateRangeRequest {
            date_range: DateRange {
                start: ALL_TIME_SCOPE_START.to_string(),
                end: "2026-06-20".to_string(),
            },
            profile_id: None,
        };

        let mut build_calls = 0u32;
        let first = resolve(&connection, &request, 11, &mut build_calls);
        assert_eq!(first, 11);
        assert_eq!(build_calls, 1);
        warm(&connection, &request, 11);

        // A second resolve with unchanged inputs must serve the cached value
        // without invoking the builder again.
        let cached = resolve(&connection, &request, 22, &mut build_calls);
        assert_eq!(cached, 11);
        assert_eq!(build_calls, 1);

        // Moving the checkpoint ledger changes the fingerprint and forces a
        // rebuild on the next resolve.
        connection
            .execute(
                "INSERT INTO core_intelligence_stage_checkpoints
                   (profile_id, stage, stage_version, updated_at, last_run_id)
                 VALUES ('chrome:Default', 'visit-derive', 'v', '2026-06-20T00:00:00Z', 7)",
                (),
            )
            .expect("insert checkpoint");
        let rebuilt = resolve(&connection, &request, 33, &mut build_calls);
        assert_eq!(rebuilt, 33);
        assert_eq!(build_calls, 2);
    }

    #[test]
    fn fingerprint_changes_with_checkpoint_ledger() {
        let connection = open_fingerprint_connection();
        let before = fingerprint(&connection, None).expect("fingerprint");
        connection
            .execute(
                "INSERT INTO core_intelligence_stage_checkpoints
                   (profile_id, stage, stage_version, updated_at, last_run_id)
                 VALUES ('chrome:Default', 'visit-derive', 'v', '2026-06-20T00:00:00Z', 1)",
                (),
            )
            .expect("insert checkpoint");
        let after = fingerprint(&connection, None).expect("fingerprint");
        assert_ne!(before, after);
    }

    /// Each watermark dimension folded into the fingerprint must independently
    /// change it, so a snapshot is never reused after a backup moves any one of
    /// them. This is the behavioral guard the fingerprint's watermark fields
    /// previously lacked: hardcoding any field to a constant collapses two
    /// distinct watermarks to one fingerprint and fails here.
    #[test]
    fn fingerprint_reacts_to_each_watermark_dimension() {
        let connection = open_fingerprint_connection();
        // Seed one checkpoint row with a fixed run id / timestamp and zero
        // watermarks. Mutating a single watermark *column* in place then isolates
        // that fingerprint dimension: the checkpoint count, last_run_id, and
        // updated_at (which load_checkpoint_signature folds in) all stay fixed,
        // so any observed fingerprint change must come from the watermark field
        // under test.
        insert_checkpoint_with_watermark(&connection, "chrome:Default", "visit-derive", 0, 0, 0, 0);
        let mut previous = fingerprint(&connection, None).expect("baseline fingerprint");
        let mut seen = std::collections::BTreeSet::from([previous.clone()]);

        for (index, column) in [
            "visible_visit_count",
            "max_visit_id",
            "max_url_last_visit_ms",
            "visible_search_term_count",
        ]
        .iter()
        .enumerate()
        {
            connection
                .execute(
                    &format!(
                        "UPDATE core_intelligence_stage_checkpoints
                         SET {column} = {column} + 11
                         WHERE profile_id = 'chrome:Default' AND stage = 'visit-derive'"
                    ),
                    (),
                )
                .expect("bump one watermark column");
            let fingerprint = fingerprint(&connection, None).expect("fingerprint");
            assert_ne!(
                previous, fingerprint,
                "moving the {column} watermark must change the fingerprint (step {index})"
            );
            assert!(seen.insert(fingerprint.clone()), "fingerprints must stay distinct");
            previous = fingerprint;
        }
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

    /// The profile-scoped fingerprint folds in only that profile's checkpoint
    /// watermark, so a sibling profile's rebuild must not invalidate it while a
    /// change to the scoped profile must.
    #[test]
    fn profile_scoped_fingerprint_isolates_its_own_profile() {
        let connection = open_fingerprint_connection();
        insert_checkpoint_with_watermark(&connection, "chrome:Default", "visit-derive", 1, 1, 1, 1);
        let scoped = fingerprint(&connection, Some("chrome:Default")).expect("scoped");

        // A different profile's checkpoint must not move the scoped fingerprint.
        insert_checkpoint_with_watermark(
            &connection,
            "firefox:Default",
            "visit-derive",
            9,
            9,
            9,
            9,
        );
        let after_sibling =
            fingerprint(&connection, Some("chrome:Default")).expect("after sibling");
        assert_eq!(scoped, after_sibling);

        // But a change to the scoped profile must.
        connection
            .execute(
                "UPDATE core_intelligence_stage_checkpoints SET visible_visit_count = 42
                 WHERE profile_id = 'chrome:Default'",
                (),
            )
            .expect("bump scoped profile");
        let after_self = fingerprint(&connection, Some("chrome:Default")).expect("after self");
        assert_ne!(scoped, after_self);
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

    /// Editing the site dictionary or the search-engine rules reshapes derived
    /// aggregates without moving any canonical watermark, so each must change
    /// the fingerprint on its own.
    #[test]
    fn fingerprint_reacts_to_site_dictionary_and_search_rule_edits() {
        let connection = open_fingerprint_connection();
        let baseline = fingerprint(&connection, None).expect("baseline");

        // load_site_dictionary_signature lazily creates the overrides table; a
        // row insert must then change the fingerprint.
        let _ = connection.read(load_site_dictionary_signature).unwrap();
        connection
            .execute(
                "INSERT INTO site_dictionary_overrides
                   (target_kind, target_value, created_at, updated_at)
                 VALUES ('domain', 'example.com', 'now', 'now')",
                (),
            )
            .expect("insert override");
        let after_override = fingerprint(&connection, None).expect("after override");
        assert_ne!(baseline, after_override, "a site-dictionary edit must change the fingerprint");

        connection.execute("INSERT INTO search_engine_rules(rule_id,engine_id,display_name,host_pattern,query_param_key,enabled,created_at,updated_at) VALUES('rule','engine','Engine','search.example.com','q',1,'','2026-06-20T00:00:00Z')",()).unwrap();
        let after_rule = fingerprint(&connection, None).expect("after rule");
        assert_ne!(
            after_override, after_rule,
            "a search-engine-rule edit must change the fingerprint"
        );
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
