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

use super::site_days::{SiteDayCache, SiteDayScope, VisitDays};
use super::{date_range_bounds, rfc3339_from_millis};
use crate::{
    intelligence::site_dictionary::display_name_for_domain,
    models::{
        BreadthIndex, HabitPattern, InterruptedHabit, ObservedInteraction, ProfileScopedRequest,
        ScopedDateRangeRequest,
    },
};
use anyhow::Result;
use chrono::{Local, NaiveDate};
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
    let (_, end_ms) = date_range_bounds(&request.date_range)?;
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
                    .map(local_midnight_ms)
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

/// Local midnight at the start of `day`, in Unix milliseconds.
fn local_midnight_ms(day: NaiveDate) -> i64 {
    day.and_hms_opt(0, 0, 0)
        .and_then(|naive| naive.and_local_timezone(Local).earliest())
        .map(|local| local.timestamp_millis())
        .unwrap_or_default()
}

pub fn get_interrupted_habits(
    vault: &crate::Vault,
    request: &ProfileScopedRequest,
) -> Result<Vec<InterruptedHabit>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_interrupted_habits_with_connection(connection, request)
    })
}

pub(crate) fn get_interrupted_habits_with_connection(
    connection: &Session<'_>,
    request: &ProfileScopedRequest,
) -> Result<Vec<InterruptedHabit>> {
    let mut statement = connection.prepare(
        "SELECT registrable_domain, habit_type, mean_interval_days, cv, visit_count, last_visited_ms
         FROM habit_patterns
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND is_interrupted = 1
         ORDER BY last_visited_ms DESC, registrable_domain ASC",
    )?;
    let now_ms = Local::now().timestamp_millis();
    let habits = statement
        .query_map([request.profile_id.as_deref()], |row| {
            let last_visited_ms = row.get::<i64>(5)?;
            let days_since_last_visit =
                ((now_ms - last_visited_ms).max(0) as f32 / 86_400_000.0).floor() as i64;
            let mean_interval_days = row.get::<f64>(2)? as f32;
            Ok(InterruptedHabit {
                habit: HabitPattern {
                    registrable_domain: row.get(0)?,
                    display_name: display_name_for_domain(&row.get::<String>(0)?),
                    habit_type: row.get(1)?,
                    mean_interval_days,
                    cv: row.get::<f64>(3)? as f32,
                    visit_count: row.get(4)?,
                    last_visited_at: rfc3339_from_millis(last_visited_ms),
                    is_interrupted: true,
                },
                days_since_last_visit,
                interruption_threshold_days: mean_interval_days * 2.0,
            })
        })?
        .collect::<vault_store::Result<Vec<_>>>()?;
    Ok(habits)
}

pub use super::path_flows::get_path_flows;
pub(crate) use super::path_flows::get_path_flows_with_connection;
#[cfg(test)]
use super::path_flows::{build_path_flow_steps, count_segment_paths};

pub fn get_observed_interactions(
    vault: &crate::Vault,
    request: &crate::models::ScopedPageRequest,
) -> Result<crate::models::IntelligencePage<ObservedInteraction>> {
    use vault_store::Db;
    let available=vault.read(Db::SourceEvidence,|s|Ok(s.query_one("SELECT EXISTS(SELECT 1 FROM visit_engagement_evidence LIMIT 1) OR EXISTS(SELECT 1 FROM visit_context_evidence LIMIT 1)",(),|r|Ok(r.get::<i64>(0)?!=0))?))?;
    if !available {
        return Ok(crate::models::IntelligencePage::default());
    }
    let (start, end) = date_range_bounds(&request.date_range)?;
    let profile = request.profile_id.clone();
    let limit = request.limit.unwrap_or(100).clamp(1, 512) as usize;
    let after = super::paging::cursor::<(i64, i64)>(request.cursor.as_deref())?;
    let rows = vault.read(Db::Intelligence, move |s| {
        load_observed_interaction_visits(s, profile.as_deref(), start, end, after, limit + 1)
    })?;
    let page = super::paging::finish(rows, limit)?;
    vault.read(Db::SourceEvidence, move |s| {
        let evidence = load_engagement_evidence(s, &page.items)?;
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
            let observed =
                evidence.get(&(visit.source_profile_id, visit.source_visit_id.clone()))?;
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
        ((Local::now().timestamp_millis() - last_visited_ms).max(0) as f32 / 86_400_000.0).max(0.0);

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
    Ok(s.query(&format!("SELECT v.id,v.source_profile_id,v.source_visit_id,u.url,u.title,COALESCE(p.browser_family,p.browser_kind),v.visit_time_ms
        FROM archive.visits v INDEXED BY idx_visits_visible_time_id
        CROSS JOIN archive.urls u ON u.id=v.url_id
        CROSS JOIN archive.source_profiles p ON p.id=v.source_profile_id
        WHERE v.reverted_at IS NULL AND v.source_visit_id IS NOT NULL
          AND (?1 IS NULL OR p.profile_key=?1) AND v.visit_time_ms>=?2 AND {upper}
        ORDER BY v.visit_time_ms DESC,v.id DESC LIMIT ?6"),
        params![profile,start,end,after.map(|c|c.0),after.map(|c|c.1),limit as i64],|r| {
            let id=r.get::<i64>(0)?;
            Ok((ObservedVisit {visit_id:id,source_profile_id:r.get(1)?,source_visit_id:r.get(2)?,url:r.get(3)?,title:r.get(4)?,browser_family:r.get(5)?},(r.get::<i64>(6)?,id)))
        })?)
}

fn load_engagement_evidence(
    source_evidence: &Session<'_>,
    visits: &[ObservedVisit],
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
                 ON e.source_profile_id=?1 AND e.source_visit_id=requested.source_visit_id"
            );
            let mut bindings = Vec::<SqlValue>::with_capacity(chunk.len() + 1);
            bindings.push(SqlValue::from(source_profile_id));
            bindings.extend(chunk.iter().cloned().map(SqlValue::from));
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

    if let Some(context_evidence) = load_context_evidence(source_evidence, visits)? {
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
                 WHERE e.context_key IN ('context.load_successful','context.page_end_reason')"
            );
            let mut bindings = Vec::<SqlValue>::with_capacity(chunk.len() + 1);
            bindings.push(SqlValue::from(source_profile_id));
            bindings.extend(chunk.iter().cloned().map(SqlValue::from));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::intelligence_structural_state::PathSegment;
    use crate::test_support::native::TestDatabase;
    use vault_store::{Db, Priority};

    use crate::models::{DateRange, PathFlowRequest, ProfileScopedRequest, ScopedDateRangeRequest};
    use chrono::NaiveDate;
    use std::collections::HashMap;

    use vault_store::params_owned as params;

    fn visit_days(days: impl IntoIterator<Item = NaiveDate>) -> VisitDays {
        let mut visited = VisitDays::default();
        for day in days {
            visited.add(chrono::Datelike::num_days_from_ce(&day));
        }
        visited
    }

    fn april(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, day).expect("date")
    }

    #[test]
    fn classify_habit_pattern_requires_five_visits_and_two_weeks() {
        let days = visit_days([1, 3, 5, 7, 9].map(april));
        assert!(classify_habit_pattern("example.com", &days, 0).is_none());
    }

    #[test]
    fn classify_habit_pattern_detects_weekly_habit() {
        let now = chrono::Local::now().timestamp_millis();
        let weekly = visit_days([1, 8, 15, 22, 29].map(april));
        let habit = classify_habit_pattern("example.com", &weekly, now).expect("habit");
        assert_eq!(habit.habit_type, "weekly_habit");
        assert!(!habit.is_interrupted);

        let daily = classify_habit_pattern("daily.example", &visit_days((1..=15).map(april)), now)
            .expect("daily habit");
        assert_eq!(daily.habit_type, "daily_habit");

        let first = NaiveDate::from_ymd_opt(2026, 1, 1).expect("date");
        let periodic =
            visit_days([0, 15, 30, 45, 60].map(|days| first + chrono::Duration::days(days)));
        let periodic =
            classify_habit_pattern("periodic.example", &periodic, now).expect("periodic habit");
        assert_eq!(periodic.habit_type, "periodic_reference");
    }

    /// Running totals give the gaps' mean and spread the list of days gives, a day added twice
    /// (two browsers on the same day) counts once, and no rounding makes the spread negative.
    #[test]
    fn visit_days_match_the_days_they_stand_for() {
        let days = [1, 2, 2, 5, 6, 13, 27, 28].map(april);
        let visited = visit_days(days);
        let mut distinct = days.to_vec();
        distinct.dedup();
        let gaps = distinct.windows(2).map(|w| (w[1] - w[0]).num_days() as f64).collect::<Vec<_>>();
        let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
        let variance = gaps.iter().map(|gap| (gap - mean).powi(2)).sum::<f64>() / gaps.len() as f64;
        assert_eq!(visited.count, 7);
        let (running_mean, running_variance) = visited.gaps().unwrap();
        assert!((running_mean - mean).abs() < 1e-9);
        assert!((running_variance - variance).abs() < 1e-9);
        let steady = visit_days([1, 8, 15, 22, 29].map(april));
        assert_eq!(steady.gaps(), Some((7.0, 0.0)));
    }

    /// Coming back to a site makes a round trip, not a path: "google → github → google" is
    /// left out, and so is any longer window that revisits a site.
    #[test]
    fn round_trips_are_not_paths() {
        let at = |site: &str| (site.to_string(), 0);
        let sequence = vec![at("google.com"), at("github.com"), at("google.com"), at("docs.rs")];
        let mut flows = HashMap::new();
        count_segment_paths(&sequence, 0, 3, &mut flows);
        let patterns: Vec<_> = flows.keys().map(|(pattern, _)| pattern.as_str()).collect();
        assert_eq!(patterns, ["github.com → google.com → docs.rs"]);
        let mut four = HashMap::new();
        count_segment_paths(&sequence, 0, 4, &mut four);
        assert!(four.is_empty());
    }

    #[test]
    fn a_session_counts_every_window_once() {
        let sequence = vec![
            ("google.com".to_string(), 10),
            ("github.com".to_string(), 20),
            ("sqlite.org".to_string(), 30),
        ];
        let mut flows = HashMap::new();
        count_segment_paths(&sequence, 0, 2, &mut flows);
        count_segment_paths(&sequence, 0, 3, &mut flows);
        assert_eq!(flows.len(), 3);
        assert!(flows.contains_key(&("google.com → github.com".to_string(), 2)));
        assert!(flows.contains_key(&("github.com → sqlite.org".to_string(), 2)));
        assert!(flows.contains_key(&("google.com → github.com → sqlite.org".to_string(), 3)));

        let steps = build_path_flow_steps("valid.example → bad/path → .hidden → localhost");
        assert_eq!(steps[0].registrable_domain.as_deref(), Some("valid.example"));
        assert!(steps[1].registrable_domain.is_none());
        assert!(steps[2].registrable_domain.is_none());
        assert!(steps[3].registrable_domain.is_none());
    }

    #[test]
    fn phase_three_read_models_cover_breadth_habits_interruptions_and_flows() {
        let connection = phase_three_connection();
        seed_rollups(&connection);
        seed_visits_and_facts(&connection);
        seed_interrupted_habits(&connection);

        let scoped = ScopedDateRangeRequest { date_range: april_range(), profile_id: None };
        let breadth = {
            let scoped = scoped.clone();
            connection.read(move |session| {
                get_breadth_index_with_connection(session, &SiteDayCache::default(), &scoped)
            })
        }
        .expect("breadth index");
        assert!(breadth.hhi > 0.0);
        assert_eq!(breadth.concentration_domain_count, 1);

        let empty = {
            connection.read(move |session| {
                get_breadth_index_with_connection(
                    session,
                    &SiteDayCache::default(),
                    &ScopedDateRangeRequest {
                        date_range: DateRange {
                            start: "2026-05-01".to_string(),
                            end: "2026-05-02".to_string(),
                        },
                        profile_id: None,
                    },
                )
            })
        }
        .expect("empty breadth");
        assert_eq!(empty.breadth_score, 0.0);

        let habits = {
            let scoped = scoped.clone();
            connection.read(move |session| {
                get_habit_patterns_with_connection(session, &SiteDayCache::default(), &scoped)
            })
        }
        .expect("habits");
        let weekly = habits
            .iter()
            .find(|habit| habit.registrable_domain == "weekly.example")
            .expect("weekly habit");
        assert_eq!(weekly.habit_type, "weekly_habit");
        assert_eq!(weekly.visit_count, 5, "active days, from the rollups");
        assert_eq!(weekly.last_visited_at, rfc3339_from_millis(day_ms(29)));
        assert!(!habits.iter().any(|habit| habit.registrable_domain == "docs.example"));

        let interrupted = {
            connection.read(move |session| {
                get_interrupted_habits_with_connection(
                    session,
                    &ProfileScopedRequest { profile_id: Some("chrome:Default".to_string()) },
                )
            })
        }
        .expect("interrupted habits");
        assert_eq!(interrupted.len(), 1);
        assert!(interrupted[0].days_since_last_visit >= 0);

        let flows = {
            connection.read(move |session| {
                get_path_flows_with_connection(
                    session,
                    &PathFlowRequest {
                        cursor: None,
                        date_range: april_range(),
                        profile_id: None,
                        step_count: 2,
                        limit: Some(5),
                    },
                )
            })
        }
        .expect("path flows");
        assert!(flows.iter().any(|flow| flow.flow_pattern == "daily.example → weekly.example"));

        let scoped_flows = {
            connection.read(move |session| {
                get_path_flows_with_connection(
                    session,
                    &PathFlowRequest {
                        cursor: None,
                        date_range: april_range(),
                        profile_id: Some("chrome:Default".to_string()),
                        step_count: 8,
                        limit: Some(1),
                    },
                )
            })
        }
        .expect("scoped path flows");
        assert!(scoped_flows.len() <= 1);
        assert!(scoped_flows.iter().all(|flow| flow.step_count == 4));

        let empty_observed = get_observed_interactions(
            &connection.vault,
            &crate::models::ScopedPageRequest {
                date_range: DateRange { start: "2026-05-01".into(), end: "2026-05-02".into() },
                profile_id: None,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(empty_observed.is_empty());
    }

    #[test]
    fn observed_interaction_evidence_merges_metrics_and_context_fallbacks() {
        let vault = crate::Vault::open_temp().unwrap();
        let source_evidence = TestDatabase::new(&vault, Db::SourceEvidence);
        let visits = vec![
            ObservedVisit {
                visit_id: 1,
                source_profile_id: 7,
                source_visit_id: "a".to_string(),
                url: "https://example.com/a".to_string(),
                title: Some("A".to_string()),
                browser_family: "chrome".to_string(),
            },
            ObservedVisit {
                visit_id: 2,
                source_profile_id: 7,
                source_visit_id: "b".to_string(),
                url: "https://example.com/b".to_string(),
                title: None,
                browser_family: "chrome".to_string(),
            },
        ];

        for (visit_id, metric_key, value_int, value_real) in [
            ("a", "engagement.total_foreground_duration_ms", Some(10), None),
            ("a", "engagement.scrolling_time_ms", None, Some(20.4)),
            ("a", "engagement.scrolling_distance_px", Some(30), None),
            ("a", "engagement.key_presses", Some(4), None),
            ("a", "engagement.typing_time_ms", Some(50), None),
            ("a", "engagement.load_successful", Some(1), None),
            ("a", "engagement.unknown", Some(999), None),
            ("b", "engagement.visit_duration_ms", None, Some(7.6)),
        ] {
            source_evidence
                .execute(
                    "INSERT INTO visit_engagement_evidence(source_profile_id,source_visit_id,metric_key,metric_value_int,metric_value_real,source_batch_id,source_field,recorded_at) VALUES (7,?1,?2,?3,?4,0,'fixture','')",
                    params![visit_id, metric_key, value_int, value_real],
                )
                .expect("metric");
        }
        for (visit_id, context_key, value_json) in [
            (Some("a"), "context.load_successful", "false"),
            (Some("a"), "context.page_end_reason", "\"closed\""),
            (Some("a"), "context.unknown", "true"),
            (Some("b"), "context.load_successful", "true"),
            (Some("b"), "context.page_end_reason", "null"),
            (None, "context.page_end_reason", "\"ignored\""),
        ] {
            source_evidence
                .execute(
                    "INSERT INTO visit_context_evidence(source_profile_id,source_visit_id,context_key,value_json,source_batch_id,source_field,recorded_at) VALUES (7,?1,?2,?3,0,'fixture','')",
                    params![visit_id, context_key, value_json],
                )
                .expect("context");
        }

        let evidence =
            { source_evidence.read(move |session| load_engagement_evidence(session, &visits)) }
                .expect("evidence");
        let first = evidence.get(&(7, "a".to_string())).expect("first visit evidence");
        assert_eq!(first.foreground_duration_ms, Some(10));
        assert_eq!(first.scrolling_time_ms, Some(20));
        assert_eq!(first.scrolling_distance, Some(30));
        assert_eq!(first.key_presses, Some(4));
        assert_eq!(first.typing_time_ms, Some(50));
        assert_eq!(first.load_successful, Some(true));
        assert_eq!(first.page_end_reason.as_deref(), Some("closed"));
        assert!(first.has_any_signal());

        let second = evidence.get(&(7, "b".to_string())).expect("second visit evidence");
        assert_eq!(second.foreground_duration_ms, Some(8));
        assert_eq!(second.load_successful, Some(true));
        assert!(second.page_end_reason.is_none());

        let no_context =
            { source_evidence.read(move |session| load_context_evidence(session, &[])) }
                .expect("empty context evidence");
        assert!(no_context.is_none());

        assert!(observed_interactions_from_visits(Vec::new(), &HashMap::new()).is_empty());
        let skipped = observed_interactions_from_visits(
            vec![ObservedVisit {
                visit_id: 3,
                source_profile_id: 7,
                source_visit_id: "c".to_string(),
                url: "https://example.com/c".to_string(),
                title: None,
                browser_family: "chrome".to_string(),
            }],
            &HashMap::from([((7, "c".to_string()), ObservedEvidence::default())]),
        );
        assert!(skipped.is_empty());

        for evidence in [
            ObservedEvidence::default(),
            ObservedEvidence { foreground_duration_ms: Some(1), ..ObservedEvidence::default() },
            ObservedEvidence { scrolling_time_ms: Some(1), ..ObservedEvidence::default() },
            ObservedEvidence { scrolling_distance: Some(1), ..ObservedEvidence::default() },
            ObservedEvidence { key_presses: Some(1), ..ObservedEvidence::default() },
            ObservedEvidence { typing_time_ms: Some(1), ..ObservedEvidence::default() },
            ObservedEvidence { load_successful: Some(false), ..ObservedEvidence::default() },
            ObservedEvidence {
                page_end_reason: Some("closed".to_string()),
                ..ObservedEvidence::default()
            },
        ] {
            let expected = evidence != ObservedEvidence::default();
            assert_eq!(evidence.has_any_signal(), expected);
        }
    }

    fn phase_three_connection() -> TestDatabase {
        let vault = crate::Vault::open_temp().unwrap();
        vault.write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')",())?;Ok(())
        }).unwrap();
        let db = TestDatabase::new(&vault, Db::Intelligence);
        db.execute("INSERT INTO intelligence_generations(profile_id) VALUES('chrome:Default'),('safari:Default')",()).unwrap();
        db
    }

    fn april_range() -> DateRange {
        DateRange { start: "2026-04-01".to_string(), end: "2026-04-30".to_string() }
    }

    fn day_ms(day: u32) -> i64 {
        NaiveDate::from_ymd_opt(2026, 4, day)
            .expect("date")
            .and_hms_opt(12, 0, 0)
            .expect("time")
            .and_utc()
            .timestamp_millis()
    }

    fn seed_rollups(connection: &TestDatabase) {
        for (domain, visits) in [("daily.example", 12), ("weekly.example", 6), ("docs.example", 3)]
        {
            connection
                .execute(
                    "INSERT INTO domain_daily_rollups_rows(profile_id,date_key,registrable_domain,visit_count,domain_category,search_count,new_domain_visits,unique_urls) VALUES ('chrome:Default','2026-04-15',?1,?2,'unknown',0,0,1)",
                    params![domain, visits],
                )
                .expect("rollup");
        }
    }

    fn seed_visits_and_facts(connection: &TestDatabase) {
        let mut visit_id = 1_i64;
        for day in 1..=15 {
            insert_visit_fact(connection, visit_id, "daily.example", "session-a", day);
            visit_id += 1;
        }
        for day in [1, 8, 15, 22, 29] {
            insert_visit_fact(connection, visit_id, "weekly.example", "session-a", day);
            visit_id += 1;
        }
        insert_visit_fact(connection, visit_id, "docs.example", "session-b", 20);
        insert_visit_fact(connection, visit_id + 1, "daily.example", "session-b", 21);
        insert_session_paths(connection);
    }

    /// Each session's path segments, cut the way the structural build cuts them.
    fn insert_session_paths(connection: &TestDatabase) {
        let visits = connection
            .read(|s| {
                Ok(s.query(
                    "SELECT a.session_id,f.registrable_domain,v.visit_time_ms
                     FROM visit_assignments a
                     JOIN visit_derived_facts_rows f ON f.visit_id=a.visit_id
                     JOIN archive.visits v ON v.id=a.visit_id
                     ORDER BY a.session_id,v.visit_time_ms,v.id",
                    (),
                    |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)),
                )?)
            })
            .unwrap();
        let mut segments = Vec::new();
        let mut open: Option<(String, PathSegment)> = None;
        for (session, domain, time) in visits {
            if open.as_ref().is_none_or(|(current, _)| *current != session) {
                segments.extend(open.take());
                open = Some((session.clone(), PathSegment::default()));
            }
            let (_, path) = open.as_mut().unwrap();
            if let Some(closed) = path.push(&domain, time) {
                segments.push((session, closed));
            }
        }
        segments.extend(open);
        for (session, path) in segments {
            connection
                .execute(
                    "INSERT INTO session_paths(generation,profile_id,session_id,segment,date_key,prefix_len,sites_json) VALUES(0,'chrome:Default',?1,?2,?3,?4,?5)",
                    params![
                        session,
                        path.segment,
                        path.date_key.clone(),
                        path.prefix_len as i64,
                        serde_json::to_string(&path.sites).unwrap()
                    ],
                )
                .unwrap();
        }
    }

    fn insert_visit_fact(
        connection: &TestDatabase,
        visit_id: i64,
        domain: &str,
        session_id: &str,
        day: u32,
    ) {
        let archive = TestDatabase::new(&connection.vault, Db::Archive);
        let url = format!("https://{domain}/{visit_id}");
        let time = day_ms(day);
        archive
            .execute(
                "INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3)",
                params![visit_id, url, domain],
            )
            .unwrap();
        archive.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,?3,'',?3,'')",params![visit_id,url,time]).unwrap();
        archive.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,?2,'')",params![visit_id,time]).unwrap();
        connection.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,registrable_domain,canonical_url,computed_at) VALUES(?1,'chrome:Default',?2,?3,'')",params![visit_id,domain,url]).unwrap();
        connection.execute("INSERT INTO visit_assignments(visit_id,profile_id,session_id) VALUES(?1,'chrome:Default',?2)",params![visit_id,session_id]).unwrap();
        connection.execute("INSERT INTO domain_daily_rollups_rows(profile_id,date_key,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls,last_visit_ms) VALUES ('chrome:Default',?1,?2,'unknown',1,0,0,1,?3) ON CONFLICT(generation,date_key,profile_id,registrable_domain) DO UPDATE SET visit_count=visit_count+1,last_visit_ms=MAX(last_visit_ms,excluded.last_visit_ms)",params![format!("2026-04-{day:02}"),domain,time]).unwrap();
    }

    fn seed_interrupted_habits(connection: &TestDatabase) {
        connection
            .execute(
                "INSERT INTO habit_patterns_rows(profile_id,registrable_domain,habit_type,mean_interval_days,cv,visit_count,last_visited_ms,is_interrupted,computed_at) VALUES ('chrome:Default','weekly.example','weekly_habit',7.0,0.1,5,?1,1,'')",
                params![day_ms(1)],
            )
            .expect("interrupted habit");
        connection
            .execute(
                "INSERT INTO habit_patterns_rows(profile_id,registrable_domain,habit_type,mean_interval_days,cv,visit_count,last_visited_ms,is_interrupted,computed_at) VALUES ('safari:Default','other.example','weekly_habit',7.0,0.1,5,?1,1,'')",
                params![day_ms(1)],
            )
            .expect("scoped out habit");
    }
}
