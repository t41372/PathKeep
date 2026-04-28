//! Daily-rollup stage execution and bounded rollup builders.
//!
//! ## Responsibilities
//! - Run the daily-rollup rebuild stage in incremental or fallback-full mode.
//! - Load visit-derived facts for dirty day windows and build deterministic
//!   daily rollups from them.
//! - Replace only the affected rollup rows when an incremental daily window is
//!   sufficient.
//!
//! ## Not responsible for
//! - Applying the site dictionary or persisting visit-derived facts.
//! - Building sessions, trails, refind pages, or other structural aggregates.
//! - Route-level KPI or trend read-model shaping.
//!
//! ## Dependencies
//! - Parent-module `VisitRecord`, `DailyRollupBundle`, and checkpoint types.
//! - `visit_derived_facts` plus the derived daily-rollup tables.
//! - Shared local-day helpers from `intelligence_shared`.
//!
//! ## Performance notes
//! - Incremental mode only rebuilds the day keys touched by the dirty suffix.
//! - Fallback mode uses grouped SQL over persisted visit-derived facts instead
//!   of reloading the raw archive into memory.

use super::incremental::{
    ProfileSourceWatermark, StageCheckpoint, StageExecutionMode, load_stage_checkpoint,
    save_stage_checkpoint, stage_name, stage_version, watermark_regressed,
};
use super::{
    DailyRollupBundle, DerivedVisitBatchCursor, RebuildMode, StageRunResult, VisitRecord,
    build_daily_rollups, clear_core_tables_for_job_kind, display_name_for_domain, local_date_key,
    unique_date_keys,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, LocalResult, NaiveDate, TimeZone};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Default)]
struct DailyRollupFallbackSummary {
    processed_visits: usize,
    rollups: DailyRollupBundle,
    dirty_date_keys: Vec<String>,
    dirty_from_visit_ms: Option<i64>,
}

/// Runs the daily-rollup stage for one profile and only replaces the dirty
/// date buckets when the visit-derived suffix still matches the watermark.
pub(super) fn execute_daily_rollup_stage(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    force_full: bool,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let current_version = stage_version(connection, RebuildMode::DailyRollup)?;
    let checkpoint = load_stage_checkpoint(connection, profile_id, RebuildMode::DailyRollup)?;
    if watermark.visible_visit_count == 0 {
        clear_core_tables_for_job_kind(connection, Some(profile_id), RebuildMode::DailyRollup)?;
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::DailyRollup).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: 0,
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!(
                "No visible visits remained for {profile_id}; cleared daily rollups."
            )],
            ..StageRunResult::default()
        });
    }

    let mut fallback_reason = if force_full {
        Some("Manual full rebuild requested for daily rollups.".to_string())
    } else {
        None
    };
    if !force_full {
        match checkpoint.as_ref() {
            None => fallback_reason =
                Some("No daily-rollup checkpoint was recorded for this profile yet.".to_string()),
            Some(checkpoint) if checkpoint.stage_version != current_version => {
                fallback_reason =
                    Some("Daily rollup logic changed since the last successful rebuild.".to_string())
            }
            Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => {
                fallback_reason = Some(
                    "Archive visibility regressed or source counters moved backwards for daily rollups."
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    if fallback_reason.is_none()
        && checkpoint.as_ref().is_some_and(|checkpoint| checkpoint.source_watermark == *watermark)
    {
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::DailyRollup).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: checkpoint
                    .as_ref()
                    .map(|value| value.last_processed_visit_id)
                    .unwrap_or_default(),
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!("Daily rollups for {profile_id} were already up to date.")],
            ..StageRunResult::default()
        });
    }

    let (_visits, rollups, execution_mode, dirty_visit_count, dirty_date_keys, dirty_from_visit_ms) =
        if let Some(_reason) = fallback_reason.clone() {
            let fallback_rollups = build_daily_rollups_for_profile_in_batches(
                connection,
                profile_id,
                super::DAILY_ROLLUP_FALLBACK_BATCH_SIZE,
            )?;
            (
                Vec::new(),
                fallback_rollups.rollups,
                StageExecutionMode::FallbackFull,
                fallback_rollups.processed_visits,
                fallback_rollups.dirty_date_keys,
                fallback_rollups.dirty_from_visit_ms,
            )
        } else {
            let last_processed_visit_id =
                checkpoint.as_ref().map(|value| value.last_processed_visit_id).unwrap_or_default();
            let delta_visits = load_profile_derived_visits(
                connection,
                profile_id,
                None,
                Some(last_processed_visit_id),
            )?;
            let expected_delta = (watermark.visible_visit_count
                - checkpoint
                    .as_ref()
                    .map(|value| value.source_watermark.visible_visit_count)
                    .unwrap_or_default())
            .max(0) as usize;
            if delta_visits.is_empty() || delta_visits.len() != expected_delta {
                fallback_reason = Some(
                    "Daily rollup delta rows no longer matched the current archive watermark."
                        .to_string(),
                );
                let fallback_rollups = build_daily_rollups_for_profile_in_batches(
                    connection,
                    profile_id,
                    super::DAILY_ROLLUP_FALLBACK_BATCH_SIZE,
                )?;
                (
                    Vec::new(),
                    fallback_rollups.rollups,
                    StageExecutionMode::FallbackFull,
                    fallback_rollups.processed_visits,
                    fallback_rollups.dirty_date_keys,
                    fallback_rollups.dirty_from_visit_ms,
                )
            } else {
                let dirty_date_keys = unique_date_keys(&delta_visits);
                let visits = load_profile_derived_visits_for_date_keys(
                    connection,
                    profile_id,
                    &dirty_date_keys,
                )?;
                let dirty_from_visit_ms = visits.first().map(|visit| visit.visit_time_ms);
                let rollups = build_daily_rollups(&visits);
                (
                    visits,
                    rollups,
                    StageExecutionMode::Incremental,
                    expected_delta,
                    dirty_date_keys,
                    dirty_from_visit_ms,
                )
            }
        };

    replace_daily_rollups(
        connection,
        profile_id,
        if execution_mode == StageExecutionMode::FallbackFull {
            None
        } else {
            Some(&dirty_date_keys)
        },
        &rollups,
    )?;
    save_stage_checkpoint(
        connection,
        &StageCheckpoint {
            profile_id: profile_id.to_string(),
            stage: stage_name(RebuildMode::DailyRollup).to_string(),
            stage_version: current_version,
            source_watermark: watermark.clone(),
            last_processed_visit_id: watermark.max_visit_id,
            dirty_from_visit_ms,
            dirty_date_key: dirty_date_keys.first().cloned(),
            last_run_id: Some(run_id),
            fallback_reason: fallback_reason.clone(),
            updated_at: computed_at.to_string(),
        },
    )?;
    Ok(StageRunResult {
        processed_visits: dirty_visit_count,
        execution_mode: Some(execution_mode.as_str().to_string()),
        affected_profiles: vec![profile_id.to_string()],
        dirty_visit_count: Some(dirty_visit_count),
        dirty_date_keys,
        fallback_reason: fallback_reason.clone(),
        notes: vec![if matches!(execution_mode, StageExecutionMode::Incremental) {
            format!("Refreshed dirty daily rollups for {profile_id}.")
        } else {
            format!("Rebuilt all daily rollups for {profile_id}.")
        }],
        ..StageRunResult::default()
    })
}

/// Loads persisted visit-derived rows for one profile, preserving deterministic
/// fields needed by later daily/structural rebuild stages.
pub(super) fn load_profile_derived_visits(
    connection: &Connection,
    profile_id: &str,
    start_ms: Option<i64>,
    last_processed_visit_id: Option<i64>,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                visit_derived_facts.profile_id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category,
                visit_derived_facts.page_category,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier,
                visit_derived_facts.taxonomy_source,
                visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version,
                visit_derived_facts.session_id,
                visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visits.reverted_at IS NULL
           AND (?2 IS NULL OR visits.visit_time_ms >= ?2)
           AND (?3 IS NULL OR visit_derived_facts.visit_id > ?3)
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map(params![profile_id, start_ms, last_processed_visit_id], |row| {
            Ok(VisitRecord {
                visit_id: row.get(0)?,
                profile_id: row.get(1)?,
                source_profile_id: row.get(2)?,
                source_visit_id: row.get(3)?,
                source_url_id: row.get(4)?,
                url: row.get(5)?,
                title: row.get(6)?,
                visit_time_ms: row.get(7)?,
                from_visit: row.get(8)?,
                transition_type: row.get(9)?,
                external_referrer_url: row.get(10)?,
                canonical_url: row.get(11)?,
                registrable_domain: row.get(12)?,
                domain_category: row.get(13)?,
                page_category: row.get(14)?,
                search_engine: row.get(15)?,
                search_query: row.get(16)?,
                is_new_domain: row.get::<_, i64>(17)? != 0,
                is_search_event: row.get::<_, i64>(18)? != 0,
                evidence_tier: row.get(19)?,
                taxonomy_source: row.get(20)?,
                taxonomy_pack: row.get(21)?,
                taxonomy_version: row.get(22)?,
                display_name: display_name_for_domain(&row.get::<_, String>(12)?),
                session_id: row.get(23)?,
                trail_id: row.get(24)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Streams visit-derived rows in deterministic order so aggregate builders can
/// stay bounded on large profiles.
pub(super) fn load_profile_derived_visit_batch(
    connection: &Connection,
    profile_id: &str,
    after: Option<DerivedVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                visit_derived_facts.profile_id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category,
                visit_derived_facts.page_category,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier,
                visit_derived_facts.taxonomy_source,
                visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version,
                visit_derived_facts.session_id,
                visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visits.reverted_at IS NULL
           AND (
             ?2 IS NULL
             OR visits.visit_time_ms > ?2
             OR (visits.visit_time_ms = ?2 AND visit_derived_facts.visit_id > ?3)
           )
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                after.map(|cursor| cursor.visit_time_ms),
                after.map(|cursor| cursor.visit_id),
                limit.max(1) as i64,
            ],
            |row| {
                Ok(VisitRecord {
                    visit_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    source_profile_id: row.get(2)?,
                    source_visit_id: row.get(3)?,
                    source_url_id: row.get(4)?,
                    url: row.get(5)?,
                    title: row.get(6)?,
                    visit_time_ms: row.get(7)?,
                    from_visit: row.get(8)?,
                    transition_type: row.get(9)?,
                    external_referrer_url: row.get(10)?,
                    canonical_url: row.get(11)?,
                    registrable_domain: row.get(12)?,
                    domain_category: row.get(13)?,
                    page_category: row.get(14)?,
                    search_engine: row.get(15)?,
                    search_query: row.get(16)?,
                    is_new_domain: row.get::<_, i64>(17)? != 0,
                    is_search_event: row.get::<_, i64>(18)? != 0,
                    evidence_tier: row.get(19)?,
                    taxonomy_source: row.get(20)?,
                    taxonomy_pack: row.get(21)?,
                    taxonomy_version: row.get(22)?,
                    display_name: display_name_for_domain(&row.get::<_, String>(12)?),
                    session_id: row.get(23)?,
                    trail_id: row.get(24)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Guards the unique `(date_key, profile_id, domain)` invariant before the
/// caller replaces daily rollup rows.
pub(super) fn ensure_unique_domain_rollup_rows(
    domain_rows: &[(String, String, String, String, i64, i64, i64, i64)],
) -> Result<()> {
    let mut seen = HashSet::<(String, String, String)>::new();
    for (date_key, profile_id, registrable_domain, _, _, _, _, _) in domain_rows {
        let key = (date_key.clone(), profile_id.clone(), registrable_domain.clone());
        if !seen.insert(key.clone()) {
            anyhow::bail!(
                "duplicate domain daily rollup row prepared for {} / {} / {}",
                key.0,
                key.1,
                key.2
            );
        }
    }
    Ok(())
}

fn load_profile_derived_visits_for_date_keys(
    connection: &Connection,
    profile_id: &str,
    date_keys: &[String],
) -> Result<Vec<VisitRecord>> {
    if date_keys.is_empty() {
        return Ok(Vec::new());
    }
    let start_ms = local_day_start_ms(
        date_keys.iter().min().context("missing minimum dirty date key for daily rollup")?,
    )?;
    let end_ms = local_day_end_exclusive_ms(
        date_keys.iter().max().context("missing maximum dirty date key for daily rollup")?,
    )?;
    Ok(load_profile_derived_visits(connection, profile_id, Some(start_ms), None)?
        .into_iter()
        .filter(|visit| {
            let date_key = local_date_key(visit.visit_time_ms);
            date_keys.iter().any(|candidate| candidate == &date_key) && visit.visit_time_ms < end_ms
        })
        .collect())
}

fn build_daily_rollups_for_profile_in_batches(
    connection: &Connection,
    profile_id: &str,
    _batch_size: usize,
) -> Result<DailyRollupFallbackSummary> {
    let (processed_visits, dirty_from_visit_ms) = connection.query_row(
        "SELECT COUNT(*), MIN(archive.visits.visit_time_ms)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL",
        [profile_id],
        |row| Ok((row.get::<_, i64>(0)?.max(0) as usize, row.get::<_, Option<i64>>(1)?)),
    )?;
    if processed_visits == 0 {
        return Ok(DailyRollupFallbackSummary::default());
    }

    let mut domain_statement = connection.prepare(
        "WITH category_counts AS (
             SELECT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime') AS date_key,
                    visit_derived_facts.profile_id AS profile_id,
                    visit_derived_facts.registrable_domain AS registrable_domain,
                    visit_derived_facts.domain_category AS domain_category,
                    COUNT(*) AS category_visits
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
             GROUP BY date_key, profile_id, registrable_domain, domain_category
         ),
         ranked_categories AS (
             SELECT date_key,
                    profile_id,
                    registrable_domain,
                    domain_category,
                    ROW_NUMBER() OVER (
                        PARTITION BY date_key, profile_id, registrable_domain
                        ORDER BY category_visits DESC,
                                 CASE WHEN domain_category = 'unknown' THEN 1 ELSE 0 END ASC,
                                 domain_category ASC
                    ) AS category_rank
             FROM category_counts
         ),
         domain_totals AS (
             SELECT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime') AS date_key,
                    visit_derived_facts.profile_id AS profile_id,
                    visit_derived_facts.registrable_domain AS registrable_domain,
                    COUNT(*) AS visit_count,
                    SUM(visit_derived_facts.is_search_event) AS search_count,
                    SUM(visit_derived_facts.is_new_domain) AS new_domain_visits,
                    COUNT(DISTINCT visit_derived_facts.canonical_url) AS unique_urls
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
             GROUP BY date_key, profile_id, registrable_domain
         )
         SELECT domain_totals.date_key,
                domain_totals.profile_id,
                domain_totals.registrable_domain,
                COALESCE(ranked_categories.domain_category, 'unknown') AS domain_category,
                domain_totals.visit_count,
                domain_totals.search_count,
                domain_totals.new_domain_visits,
                domain_totals.unique_urls
         FROM domain_totals
         LEFT JOIN ranked_categories
           ON ranked_categories.date_key = domain_totals.date_key
          AND ranked_categories.profile_id = domain_totals.profile_id
          AND ranked_categories.registrable_domain = domain_totals.registrable_domain
          AND ranked_categories.category_rank = 1
         ORDER BY domain_totals.date_key ASC, domain_totals.registrable_domain ASC",
    )?;
    let domain_rows = domain_statement
        .query_map([profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut engine_statement = connection.prepare(
        "SELECT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime') AS date_key,
                visit_derived_facts.profile_id,
                visit_derived_facts.search_engine,
                COUNT(*)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND visit_derived_facts.search_engine IS NOT NULL
         GROUP BY date_key, visit_derived_facts.profile_id, visit_derived_facts.search_engine
         ORDER BY date_key ASC, visit_derived_facts.search_engine ASC",
    )?;
    let engine_rows = engine_statement
        .query_map([profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut category_map = BTreeMap::<(String, String, String), (i64, i64)>::new();
    let mut summary_map = BTreeMap::<(String, String), (i64, i64, i64, i64, f32)>::new();
    let mut dirty_date_keys = BTreeSet::<String>::new();

    for (
        date_key,
        profile_id,
        _,
        domain_category,
        visit_count,
        search_count,
        new_domain_visits,
        _,
    ) in &domain_rows
    {
        dirty_date_keys.insert(date_key.clone());

        let category_entry = category_map
            .entry((date_key.clone(), profile_id.clone(), domain_category.clone()))
            .or_insert((0, 0));
        category_entry.0 += *visit_count;
        category_entry.1 += 1;

        let summary_entry =
            summary_map.entry((date_key.clone(), profile_id.clone())).or_insert((0, 0, 0, 0, 0.0));
        summary_entry.0 += *visit_count;
        summary_entry.1 += *search_count;
        summary_entry.2 += i64::from(*new_domain_visits > 0);
        summary_entry.3 += 1;
        summary_entry.4 += (*visit_count * *visit_count) as f32;
    }

    let category_rows = category_map
        .into_iter()
        .map(|((date_key, profile_id, domain_category), (visit_count, unique_domains))| {
            (date_key, profile_id, domain_category, visit_count, unique_domains)
        })
        .collect::<Vec<_>>();
    let summary_rows = summary_map
        .into_iter()
        .map(
            |(
                (date_key, profile_id),
                (total_visits, total_searches, new_domains, unique_domains, sumsq_domain_visits),
            )| {
                let hhi_score = sumsq_domain_visits / (total_visits * total_visits) as f32;
                let discovery_rate = new_domains as f32 / total_visits as f32;
                (
                    date_key,
                    profile_id,
                    total_visits,
                    total_searches,
                    new_domains,
                    unique_domains,
                    hhi_score,
                    discovery_rate,
                )
            },
        )
        .collect::<Vec<_>>();

    Ok(DailyRollupFallbackSummary {
        processed_visits,
        rollups: DailyRollupBundle { domain_rows, category_rows, engine_rows, summary_rows },
        dirty_date_keys: dirty_date_keys.into_iter().collect(),
        dirty_from_visit_ms,
    })
}

fn replace_daily_rollups(
    connection: &Connection,
    profile_id: &str,
    dirty_date_keys: Option<&[String]>,
    rollups: &DailyRollupBundle,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    if let Some(date_keys) = dirty_date_keys {
        for date_key in date_keys {
            tx.execute(
                "DELETE FROM domain_daily_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
            tx.execute(
                "DELETE FROM category_daily_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
            tx.execute(
                "DELETE FROM engine_daily_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
            tx.execute(
                "DELETE FROM daily_summary_rollups WHERE profile_id = ?1 AND date_key = ?2",
                params![profile_id, date_key],
            )?;
        }
    } else {
        tx.execute("DELETE FROM domain_daily_rollups WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM category_daily_rollups WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM engine_daily_rollups WHERE profile_id = ?1", [profile_id])?;
        tx.execute("DELETE FROM daily_summary_rollups WHERE profile_id = ?1", [profile_id])?;
    }

    ensure_unique_domain_rollup_rows(&rollups.domain_rows)?;
    for row in &rollups.domain_rows {
        tx.execute(
            "INSERT INTO domain_daily_rollups
             (date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7],
        )?;
    }
    for row in &rollups.category_rows {
        tx.execute(
            "INSERT INTO category_daily_rollups
             (date_key, profile_id, domain_category, visit_count, unique_domains)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![row.0, row.1, row.2, row.3, row.4],
        )?;
    }
    for row in &rollups.engine_rows {
        tx.execute(
            "INSERT INTO engine_daily_rollups
             (date_key, profile_id, search_engine, search_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![row.0, row.1, row.2, row.3],
        )?;
    }
    for row in &rollups.summary_rows {
        tx.execute(
            "INSERT INTO daily_summary_rollups
             (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn local_day_start_ms(date_key: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .with_context(|| format!("parsing local date key '{date_key}'"))?;
    let start = date.and_hms_opt(0, 0, 0).context("building local day start")?;
    local_result_timestamp_millis(
        Local.from_local_datetime(&start),
        format!("Local timezone could not represent day start for {date_key}."),
    )
}

fn local_day_end_exclusive_ms(date_key: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .with_context(|| format!("parsing local date key '{date_key}'"))?;
    let next = date.succ_opt().context("computing next local day for dirty rollup range")?;
    let start = next.and_hms_opt(0, 0, 0).context("building local day end")?;
    local_result_timestamp_millis(
        Local.from_local_datetime(&start),
        format!("Local timezone could not represent next day start for {date_key}."),
    )
}

fn local_result_timestamp_millis(
    result: LocalResult<DateTime<Local>>,
    none_message: String,
) -> Result<i64> {
    match result {
        LocalResult::Single(value) => Ok(value.timestamp_millis()),
        LocalResult::Ambiguous(first, _) => Ok(first.timestamp_millis()),
        LocalResult::None => Err(anyhow::anyhow!(none_message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_rows_and_empty_dirty_dates_are_guarded() {
        let duplicate_rows = vec![
            (
                "2026-04-25".to_string(),
                "chrome:Default".to_string(),
                "example.com".to_string(),
                "docs".to_string(),
                1,
                0,
                1,
                1,
            ),
            (
                "2026-04-25".to_string(),
                "chrome:Default".to_string(),
                "example.com".to_string(),
                "docs".to_string(),
                2,
                0,
                0,
                2,
            ),
        ];
        let duplicate_error =
            ensure_unique_domain_rollup_rows(&duplicate_rows).expect_err("duplicate row");
        assert!(duplicate_error.to_string().contains("duplicate domain daily rollup row"));

        let connection = Connection::open_in_memory().expect("in-memory sqlite");
        let visits = load_profile_derived_visits_for_date_keys(&connection, "chrome:Default", &[])
            .expect("empty dirty date list");
        assert!(visits.is_empty());
    }

    #[test]
    fn local_result_timestamp_millis_covers_timezone_edges() {
        let now = Local::now();
        assert_eq!(
            local_result_timestamp_millis(LocalResult::Single(now), "none".to_string())
                .expect("single local result"),
            now.timestamp_millis()
        );
        assert_eq!(
            local_result_timestamp_millis(LocalResult::Ambiguous(now, now), "none".to_string())
                .expect("ambiguous local result"),
            now.timestamp_millis()
        );
        let error = local_result_timestamp_millis(LocalResult::None, "missing".to_string())
            .expect_err("missing local result");
        assert!(error.to_string().contains("missing"));

        let connection = Connection::open_in_memory().expect("memory");
        connection
            .execute_batch(
                "ATTACH DATABASE ':memory:' AS archive;
                 CREATE TABLE visit_derived_facts (
                   visit_id INTEGER NOT NULL,
                   profile_id TEXT NOT NULL,
                   registrable_domain TEXT,
                   domain_category TEXT,
                   page_category TEXT,
                   is_search INTEGER NOT NULL DEFAULT 0,
                   search_engine TEXT
                 );
                 CREATE TABLE archive.visits (
                   id INTEGER PRIMARY KEY,
                   visit_time_ms INTEGER NOT NULL,
                   reverted_at TEXT
                 );",
            )
            .expect("empty fallback schema");
        let summary = build_daily_rollups_for_profile_in_batches(&connection, "chrome:Default", 10)
            .expect("empty fallback summary");
        assert_eq!(summary.processed_visits, 0);
    }
}
