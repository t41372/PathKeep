//! Responsibilities: read breadth, habits and observed interaction evidence.
//! Not responsible for: rebuild stages or source capture.
//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Phase 3 Core Intelligence read models.
//!
//! This module owns the shipping backend queries for:
//! - breadth/concentration analysis
//! - habitual visit detection
//! - interrupted-habit reminders
//! - common path flows
//! - browser-reported interaction metrics

use super::rfc3339_from_millis;
use super::site_days::{SiteDayCache, SiteDayScope, VisitDays};
use crate::{
    intelligence::site_dictionary::display_name_for_domain,
    models::{
        BreadthIndex, DateRange, HabitPattern, InterruptedHabit, ObservedInteraction,
        ProfileScopedRequest, ScopedDateRangeRequest,
    },
};
use anyhow::Result;
use chrono::{Datelike, NaiveDate, Utc};
use std::collections::HashMap;
use vault_store::{Session, Value as SqlValue, params_from_iter, params_owned as params};

pub fn get_breadth_index(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<BreadthIndex> {
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        get_breadth_index_with_connection(connection, &cache, &request)
    })
}

/// Where on a focused-to-wide scale the sites holding half the visits put someone: 1 site is 0,
/// 64 or more is 100, on a log scale so 2, 4, 8, 16 and 32 sites are evenly spaced. 1 - HHI, the
/// score before, sits near 100 for nearly everyone, so it said "wide" next to "half your browsing
/// is on 4 sites".
fn breadth_score(sites_holding_half: i64) -> f32 {
    if sites_holding_half <= 1 {
        return 0.0;
    }
    ((sites_holding_half as f32).ln() / 64_f32.ln()).clamp(0.0, 1.0) * 100.0
}

/// How spread out the range's visits are over sites, from the range's shared site-day read.
pub(crate) fn get_breadth_index_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    request: &ScopedDateRangeRequest,
) -> Result<BreadthIndex> {
    let days = cache
        .get(connection, &SiteDayScope::of(&request.date_range, request.profile_id.as_deref()))?;
    let mut rows =
        days.sites.iter().map(|site| (site.domain.as_str(), site.visits)).collect::<Vec<_>>();
    rows.sort_by(|(a_site, a), (b_site, b)| b.cmp(a).then_with(|| a_site.cmp(b_site)));

    let total_visits = rows.iter().map(|(_, visits)| *visits).sum::<i64>();
    if total_visits <= 0 {
        return Ok(BreadthIndex::default());
    }

    let total_visits_f32 = total_visits as f32;
    let hhi = rows
        .iter()
        .map(|(_, visits)| {
            let share = *visits as f32 / total_visits_f32;
            share * share
        })
        .sum::<f32>();

    let mut running = 0_i64;
    let concentration_domain_count = rows
        .iter()
        .position(|(_, visits)| {
            running += *visits;
            running * 2 >= total_visits
        })
        .map(|index| index as i64 + 1)
        .unwrap_or_default();

    Ok(BreadthIndex {
        hhi,
        breadth_score: breadth_score(concentration_domain_count),
        concentration_domain_count,
    })
}

pub fn get_habit_patterns(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<HabitPattern>> {
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        get_habit_patterns_with_connection(connection, &cache, &request)
    })
}

/// Habits only need the local days each site was visited on and its last visit, which the
/// range's shared site-day read keeps per site as running totals (`site_days::VisitDays`), so
/// this reads no visits and holds no list of days.
pub(crate) fn get_habit_patterns_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<HabitPattern>> {
    let zone = super::InsightsZone::for_reading(connection)?;
    let (_, end_ms) = zone.range_bounds(&request.date_range)?;
    let days = cache
        .get(connection, &SiteDayScope::of(&request.date_range, request.profile_id.as_deref()))?;
    let mut habits = days
        .sites
        .iter()
        .filter_map(|site| {
            // Rows built before the rollup kept the time: the end of the last active day.
            let last_visited_ms = if site.last_visit_ms == 0 {
                let last_day_end = site
                    .days
                    .last_date()
                    .and_then(|day| day.succ_opt())
                    .map(|next| day_start_ms(zone, next))
                    .unwrap_or(end_ms);
                last_day_end.min(end_ms) - 1
            } else {
                site.last_visit_ms
            };
            classify_habit_pattern(&site.domain, &site.days, last_visited_ms)
        })
        .map(|pattern| {
            (pattern.last_visited_at.clone(), pattern.registrable_domain.clone(), pattern)
        })
        .collect::<Vec<_>>();

    habits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    Ok(habits.into_iter().map(|(_, _, pattern)| pattern).collect())
}

/// One site's habit in one profile over all time, from its site-day rollups: what the habit
/// explanation describes.
pub(super) fn site_habit(
    connection: &Session<'_>,
    profile: &str,
    domain: &str,
) -> Result<Option<HabitPattern>> {
    let rows = connection.query(
        "SELECT r.date_key,r.last_visit_ms FROM domain_daily_rollups_rows r
           INDEXED BY idx_domain_daily_site
         WHERE r.registrable_domain=?1 AND r.profile_id=?2
           AND r.generation IN (SELECT daily FROM intelligence_generations)
         ORDER BY r.date_key",
        params![domain, profile],
        |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
    )?;
    let mut days = VisitDays::default();
    let mut last_visited_ms = 0;
    for (date_key, last_visit_ms) in rows {
        days.add(NaiveDate::parse_from_str(&date_key, "%Y-%m-%d")?.num_days_from_ce());
        last_visited_ms = last_visited_ms.max(last_visit_ms);
    }
    if last_visited_ms == 0 {
        // Rows built before the rollup kept the time: the end of the last active day.
        let zone = super::InsightsZone::for_reading(connection)?;
        last_visited_ms = days
            .last_date()
            .and_then(|day| day.succ_opt())
            .map_or(0, |next| day_start_ms(zone, next) - 1);
    }
    Ok(classify_habit_pattern(domain, &days, last_visited_ms))
}

/// The start of `day` in Insights' zone, in Unix milliseconds.
fn day_start_ms(zone: super::InsightsZone, day: NaiveDate) -> i64 {
    zone.start_of(day).map(|start| start.timestamp_millis()).unwrap_or_default()
}

pub fn get_interrupted_habits(
    vault: &crate::Vault,
    request: &ProfileScopedRequest,
) -> Result<Vec<InterruptedHabit>> {
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        get_interrupted_habits_with_connection(connection, &cache, &request)
    })
}

/// The habits of all time that went quiet. Worked out when read, like the habits themselves,
/// because a habit stops while no backup runs.
pub(crate) fn get_interrupted_habits_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    request: &ProfileScopedRequest,
) -> Result<Vec<InterruptedHabit>> {
    let all_time = ScopedDateRangeRequest {
        date_range: DateRange {
            start: super::intelligence_overview_snapshot::ALL_TIME_SCOPE_START.to_owned(),
            end: super::InsightsZone::for_reading(connection)?.today_key(),
        },
        profile_id: request.profile_id.clone(),
    };
    let habits = get_habit_patterns_with_connection(connection, cache, &all_time)?;
    Ok(interrupted_among(&habits, Utc::now().timestamp_millis()))
}

/// The habits among `habits` that went quiet by `now_ms`: no visit for twice their usual gap.
pub(crate) fn interrupted_among(habits: &[HabitPattern], now_ms: i64) -> Vec<InterruptedHabit> {
    habits
        .iter()
        .filter_map(|habit| {
            let last = chrono::DateTime::parse_from_rfc3339(&habit.last_visited_at).ok()?;
            let days_since = (now_ms - last.timestamp_millis()).max(0) as f32 / 86_400_000.0;
            let threshold = habit.mean_interval_days * 2.0;
            (days_since > threshold).then(|| InterruptedHabit {
                habit: HabitPattern { is_interrupted: true, ..habit.clone() },
                days_since_last_visit: days_since.floor() as i64,
                interruption_threshold_days: threshold,
            })
        })
        .collect()
}

pub use super::path_flows::get_path_flows;
pub(crate) use super::path_flows::get_path_flows_with_connection;

pub fn get_observed_interactions(
    vault: &crate::Vault,
    request: &crate::models::ScopedPageRequest,
) -> Result<crate::models::IntelligencePage<ObservedInteraction>> {
    use vault_store::Db;
    let available=vault.read(Db::SourceEvidence,|s|Ok(s.query_one("SELECT EXISTS(SELECT 1 FROM visit_engagement_evidence LIMIT 1) OR EXISTS(SELECT 1 FROM visit_context_evidence LIMIT 1)",(),|r|Ok(r.get::<i64>(0)?!=0))?))?;
    if !available {
        return Ok(crate::models::IntelligencePage::default());
    }
    let zone = vault.read(Db::Intelligence, super::InsightsZone::for_reading)?;
    let (start, end) = zone.range_bounds(&request.date_range)?;
    let profile = request.profile_id.clone();
    let limit = request.limit.unwrap_or(100).clamp(1, 512) as usize;
    let after = super::paging::cursor::<(i64, i64)>(request.cursor.as_deref())?;
    let (rows, last_legacy_run) = vault.read(Db::Intelligence, move |s| {
        let rows =
            load_observed_interaction_visits(s, profile.as_deref(), start, end, after, limit + 1)?;
        let last_legacy_run = s.query_one(
            "SELECT last_legacy_run FROM archive.identity_upgrade WHERE id=1",
            (),
            |r| Ok(r.get::<i64>(0)?),
        )?;
        Ok((rows, last_legacy_run))
    })?;
    let page = super::paging::finish(rows, limit)?;
    vault.read(Db::SourceEvidence, move |s| {
        // Only visits archived before browser times were kept are looked up by browser row id:
        // that id was unique among them. A later visit's duration is on its own row.
        let legacy = page.items.iter().filter(|visit| !visit.keyed).cloned().collect::<Vec<_>>();
        let evidence = load_engagement_evidence(s, &legacy, last_legacy_run)?;
        Ok(crate::models::IntelligencePage {
            items: observed_interactions_from_visits(page.items, &evidence),
            next_cursor: page.next_cursor,
        })
    })
}

fn observed_interactions_from_visits(
    visits: Vec<ObservedVisit>,
    evidence: &HashMap<(i64, String), ObservedEvidence>,
) -> Vec<ObservedInteraction> {
    if visits.is_empty() {
        return Vec::new();
    }
    visits
        .into_iter()
        .filter_map(|visit| {
            // The browser's duration, kept current by every backup that reads the visit again.
            let own = ObservedEvidence {
                foreground_duration_ms: visit.duration_ms,
                ..ObservedEvidence::default()
            };
            let observed = if visit.keyed {
                &own
            } else {
                evidence.get(&(visit.source_profile_id, visit.source_visit_id.clone()))?
            };
            if !observed.has_any_signal() {
                return None;
            }
            Some(ObservedInteraction {
                visit_id: visit.visit_id,
                url: visit.url,
                title: visit.title,
                browser_family: visit.browser_family,
                foreground_duration_ms: observed.foreground_duration_ms,
                scrolling_time_ms: observed.scrolling_time_ms,
                scrolling_distance: observed.scrolling_distance,
                key_presses: observed.key_presses,
                typing_time_ms: observed.typing_time_ms,
                load_successful: observed.load_successful,
                page_end_reason: observed.page_end_reason.clone(),
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct ObservedVisit {
    visit_id: i64,
    source_profile_id: i64,
    source_visit_id: String,
    url: String,
    title: Option<String>,
    browser_family: String,
    duration_ms: Option<i64>,
    /// Archived with its browser time: its browser row id may be another visit's too.
    keyed: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ObservedEvidence {
    foreground_duration_ms: Option<i64>,
    scrolling_time_ms: Option<i64>,
    scrolling_distance: Option<i64>,
    key_presses: Option<i64>,
    typing_time_ms: Option<i64>,
    load_successful: Option<bool>,
    page_end_reason: Option<String>,
}

impl ObservedEvidence {
    fn has_any_signal(&self) -> bool {
        self.foreground_duration_ms.is_some()
            || self.scrolling_time_ms.is_some()
            || self.scrolling_distance.is_some()
            || self.key_presses.is_some()
            || self.typing_time_ms.is_some()
            || self.load_successful.is_some()
            || self.page_end_reason.is_some()
    }
}

/// A site is a habit when it was visited on at least five days over at least two weeks, at
/// steady gaps: about daily, about weekly, or every so often.
fn classify_habit_pattern(
    domain: &str,
    days: &VisitDays,
    last_visited_ms: i64,
) -> Option<HabitPattern> {
    if days.count < 5 || days.last - days.first < 14 {
        return None;
    }
    let (mean, variance) = days.gaps()?;
    let mean_interval_days = mean as f32;
    let cv = if mean == 0.0 { 0.0 } else { (variance.sqrt() / mean) as f32 };
    let habit_type = if mean_interval_days < 2.0 && cv < 0.5 {
        Some("daily_habit")
    } else if (5.0..=10.0).contains(&mean_interval_days) && cv < 0.6 {
        Some("weekly_habit")
    } else if mean_interval_days > 10.0 && cv < 0.8 {
        Some("periodic_reference")
    } else {
        None
    }?;

    let days_since_last_visit =
        ((Utc::now().timestamp_millis() - last_visited_ms).max(0) as f32 / 86_400_000.0).max(0.0);

    Some(HabitPattern {
        registrable_domain: domain.to_string(),
        display_name: display_name_for_domain(domain),
        habit_type: habit_type.to_string(),
        mean_interval_days,
        cv,
        visit_count: days.count,
        last_visited_at: rfc3339_from_millis(last_visited_ms),
        is_interrupted: days_since_last_visit > mean_interval_days * 2.0,
    })
}

fn load_observed_interaction_visits(
    s: &Session<'_>,
    profile: Option<&str>,
    start: i64,
    end: i64,
    after: Option<(i64, i64)>,
    limit: usize,
) -> Result<Vec<(ObservedVisit, (i64, i64))>> {
    // A later page seeks the time index to the cursor, which then replaces the window's end (a
    // cursor at or past the end excludes nothing). Turso keeps scanning from the window's end when
    // the cursor is written as `t<? OR (t=? AND id<?)`, or sits behind `?4 IS NULL OR`.
    let after = after.filter(|(time, _)| *time < end);
    let upper = if after.is_some() {
        "v.visit_time_ms<=?4 AND (v.visit_time_ms<?4 OR v.id<?5)"
    } else {
        "v.visit_time_ms<?3"
    };
    Ok(s.query(&format!("SELECT v.id,v.source_profile_id,v.source_visit_id,u.url,u.title,COALESCE(p.browser_family,p.browser_kind),v.visit_time_ms,
               v.visit_duration_ms,v.source_visit_time IS NOT NULL
        FROM archive.visits v INDEXED BY idx_visits_visible_time_id
        CROSS JOIN archive.urls u ON u.id=v.url_id
        CROSS JOIN archive.source_profiles p ON p.id=v.source_profile_id
        WHERE v.reverted_at IS NULL AND v.source_visit_id IS NOT NULL
          AND (?1 IS NULL OR p.profile_key=?1) AND v.visit_time_ms>=?2 AND {upper}
        ORDER BY v.visit_time_ms DESC,v.id DESC LIMIT ?6"),
        params![profile,start,end,after.map(|c|c.0),after.map(|c|c.1),limit as i64],|r| {
            let id=r.get::<i64>(0)?;
            Ok((ObservedVisit {visit_id:id,source_profile_id:r.get(1)?,source_visit_id:r.get(2)?,url:r.get(3)?,title:r.get(4)?,browser_family:r.get(5)?,
                duration_ms:r.get(7)?,keyed:r.get::<i64>(8)?!=0},(r.get::<i64>(6)?,id)))
        })?)
}

/// A visit's engagement and how its page ended. A value read again later is a row of its own when
/// it changed (Chrome writes a visit's duration when its tab closes), so rows are read in the order
/// they were written and the newest one wins.
/// Only evidence version 1's runs captured: a later visit the browser gave the same row id
/// writes its evidence under that id too. Evidence whose batch is not on record counts as
/// version 1's.
fn load_engagement_evidence(
    source_evidence: &Session<'_>,
    visits: &[ObservedVisit],
    last_legacy_run: i64,
) -> Result<HashMap<(i64, String), ObservedEvidence>> {
    let mut by_profile = HashMap::<i64, Vec<String>>::new();
    for visit in visits {
        by_profile.entry(visit.source_profile_id).or_default().push(visit.source_visit_id.clone());
    }

    let mut evidence = HashMap::<(i64, String), ObservedEvidence>::new();
    for (source_profile_id, visit_ids) in by_profile {
        for chunk in visit_ids.chunks(400) {
            let placeholders =
                (2..chunk.len() + 2).map(|n| format!("(?{n})")).collect::<Vec<_>>().join(",");
            let sql = format!(
                "WITH requested(source_visit_id) AS (VALUES {placeholders})
                 SELECT e.source_profile_id,e.source_visit_id,e.metric_key,e.metric_value_int,e.metric_value_real
                 FROM requested CROSS JOIN visit_engagement_evidence e INDEXED BY idx_visit_engagement_profile_visit
                 ON e.source_profile_id=?1 AND e.source_visit_id=requested.source_visit_id
                 LEFT JOIN source_batches b ON b.id=e.source_batch_id
                 WHERE b.run_id IS NULL OR b.run_id<=?{legacy}
                 ORDER BY e.id",
                legacy = chunk.len() + 2
            );
            let mut bindings = Vec::<SqlValue>::with_capacity(chunk.len() + 2);
            bindings.push(SqlValue::from(source_profile_id));
            bindings.extend(chunk.iter().cloned().map(SqlValue::from));
            bindings.push(SqlValue::from(last_legacy_run));
            let mut statement = source_evidence.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(bindings.iter().cloned()), |row| {
                Ok((
                    row.get::<i64>(0)?,
                    row.get::<String>(1)?,
                    row.get::<String>(2)?,
                    row.get::<Option<i64>>(3)?,
                    row.get::<Option<f64>>(4)?,
                ))
            })?;

            for row in rows {
                let (
                    source_profile_id,
                    source_visit_id,
                    metric_key,
                    metric_value_int,
                    metric_value_real,
                ) = row?;
                let entry = evidence.entry((source_profile_id, source_visit_id)).or_default();
                match metric_key.as_str() {
                    "engagement.total_foreground_duration_ms"
                    | "engagement.total_view_time_ms"
                    | "engagement.visit_duration_ms" => {
                        entry.foreground_duration_ms = metric_value_int
                            .or_else(|| metric_value_real.map(|value| value.round() as i64));
                    }
                    "engagement.scrolling_time_ms" => {
                        entry.scrolling_time_ms = metric_value_int
                            .or_else(|| metric_value_real.map(|value| value.round() as i64));
                    }
                    "engagement.scrolling_distance" | "engagement.scrolling_distance_px" => {
                        entry.scrolling_distance = metric_value_int
                            .or_else(|| metric_value_real.map(|value| value.round() as i64));
                    }
                    "engagement.key_presses" => {
                        entry.key_presses = metric_value_int
                            .or_else(|| metric_value_real.map(|value| value.round() as i64));
                    }
                    "engagement.typing_time_ms" => {
                        entry.typing_time_ms = metric_value_int
                            .or_else(|| metric_value_real.map(|value| value.round() as i64));
                    }
                    "engagement.load_successful" => {
                        entry.load_successful = metric_value_int.map(|value| value != 0);
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(context_evidence) = load_context_evidence(source_evidence, visits, last_legacy_run)?
    {
        for ((source_profile_id, source_visit_id), context) in context_evidence {
            let entry = evidence.entry((source_profile_id, source_visit_id)).or_default();
            if entry.load_successful.is_none() {
                entry.load_successful = context.load_successful;
            }
            if entry.page_end_reason.is_none() {
                entry.page_end_reason = context.page_end_reason;
            }
        }
    }

    Ok(evidence)
}

fn load_context_evidence(
    source_evidence: &Session<'_>,
    visits: &[ObservedVisit],
    last_legacy_run: i64,
) -> Result<Option<HashMap<(i64, String), ObservedEvidence>>> {
    let mut by_profile = HashMap::<i64, Vec<String>>::new();
    for visit in visits {
        by_profile.entry(visit.source_profile_id).or_default().push(visit.source_visit_id.clone());
    }

    let mut evidence = HashMap::<(i64, String), ObservedEvidence>::new();
    let mut has_rows = false;
    for (source_profile_id, visit_ids) in by_profile {
        for chunk in visit_ids.chunks(400) {
            let placeholders =
                (2..chunk.len() + 2).map(|n| format!("(?{n})")).collect::<Vec<_>>().join(",");
            let sql = format!(
                "WITH requested(source_visit_id) AS (VALUES {placeholders})
                 SELECT e.source_profile_id,e.source_visit_id,e.context_key,e.value_json
                 FROM requested CROSS JOIN visit_context_evidence e INDEXED BY idx_visit_context_profile_visit
                 ON e.source_profile_id=?1 AND e.source_visit_id=requested.source_visit_id
                 LEFT JOIN source_batches b ON b.id=e.source_batch_id
                 WHERE e.context_key IN ('context.load_successful','context.page_end_reason')
                   AND (b.run_id IS NULL OR b.run_id<=?{legacy})
                 ORDER BY e.id",
                legacy = chunk.len() + 2
            );
            let mut bindings = Vec::<SqlValue>::with_capacity(chunk.len() + 2);
            bindings.push(SqlValue::from(source_profile_id));
            bindings.extend(chunk.iter().cloned().map(SqlValue::from));
            bindings.push(SqlValue::from(last_legacy_run));
            let mut statement = source_evidence.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(bindings.iter().cloned()), |row| {
                Ok((
                    row.get::<i64>(0)?,
                    row.get::<String>(1)?,
                    row.get::<String>(2)?,
                    row.get::<String>(3)?,
                ))
            })?;
            for row in rows {
                let (source_profile_id, source_visit_id, context_key, value_json) = row?;
                has_rows = true;
                let entry = evidence.entry((source_profile_id, source_visit_id)).or_default();
                if context_key == "context.load_successful" {
                    entry.load_successful = serde_json::from_str::<bool>(&value_json).ok();
                } else {
                    entry.page_end_reason = serde_json::from_str::<Option<String>>(&value_json)
                        .ok()
                        .flatten()
                        .or_else(|| serde_json::from_str::<String>(&value_json).ok());
                }
            }
        }
    }

    Ok(has_rows.then_some(evidence))
}
