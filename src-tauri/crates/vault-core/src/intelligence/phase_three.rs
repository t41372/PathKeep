//! Phase 3 Core Intelligence read models.
//!
//! This module owns the shipping backend queries for:
//! - breadth/concentration analysis
//! - habitual visit detection
//! - interrupted-habit reminders
//! - common path flows
//! - browser-reported interaction metrics

use super::{
    date_range_bounds, ensure_core_intelligence_schema, local_date_key, rfc3339_from_millis,
};
use crate::{
    archive::{open_intelligence_connection, open_source_evidence_connection},
    config::ProjectPaths,
    intelligence::site_dictionary::display_name_for_domain,
    models::{
        AppConfig, BreadthIndex, HabitPattern, InterruptedHabit, ObservedInteraction, PathFlow,
        PathFlowRequest, ProfileScopedRequest, ScopedDateRangeRequest,
    },
};
use anyhow::Result;
use chrono::{Local, NaiveDate};
use rusqlite::{Connection, params, params_from_iter, types::Value as SqlValue};
use std::collections::{BTreeSet, HashMap};

pub fn get_breadth_index(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<BreadthIndex> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT registrable_domain, SUM(visit_count) AS total_visits
         FROM domain_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY registrable_domain
         ORDER BY total_visits DESC, registrable_domain ASC",
    )?;
    let rows = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end,
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

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
        breadth_score: ((1.0 - hhi).clamp(0.0, 1.0) * 100.0),
        concentration_domain_count,
    })
}

pub fn get_habit_patterns(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<HabitPattern>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.registrable_domain, visits.visit_time_ms
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         ORDER BY visit_derived_facts.registrable_domain ASC, visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut by_domain = HashMap::<String, DomainSeries>::new();
    for (domain, visit_time_ms) in rows {
        let entry = by_domain.entry(domain).or_default();
        let day = NaiveDate::parse_from_str(&local_date_key(visit_time_ms), "%Y-%m-%d")?;
        entry.days.insert(day);
        entry.last_visited_ms = entry.last_visited_ms.max(visit_time_ms);
    }

    let mut habits = by_domain
        .into_iter()
        .filter_map(|(domain, series)| {
            classify_habit_pattern(&domain, &series).map(|pattern| {
                (pattern.last_visited_at.clone(), pattern.registrable_domain.clone(), pattern)
            })
        })
        .collect::<Vec<_>>();

    habits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    Ok(habits.into_iter().map(|(_, _, pattern)| pattern).collect())
}

pub fn get_interrupted_habits(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ProfileScopedRequest,
) -> Result<Vec<InterruptedHabit>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
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
            let last_visited_ms = row.get::<_, i64>(5)?;
            let days_since_last_visit =
                ((now_ms - last_visited_ms).max(0) as f32 / 86_400_000.0).floor() as i64;
            let mean_interval_days = row.get::<_, f32>(2)?;
            Ok(InterruptedHabit {
                habit: HabitPattern {
                    registrable_domain: row.get(0)?,
                    display_name: display_name_for_domain(&row.get::<_, String>(0)?),
                    habit_type: row.get(1)?,
                    mean_interval_days,
                    cv: row.get(3)?,
                    visit_count: row.get(4)?,
                    last_visited_at: rfc3339_from_millis(last_visited_ms),
                    is_interrupted: true,
                },
                days_since_last_visit,
                interruption_threshold_days: mean_interval_days * 2.0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(habits)
}

pub fn get_path_flows(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PathFlowRequest,
) -> Result<Vec<PathFlow>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let step_count = request.step_count.clamp(2, 3) as usize;
    let limit = request.limit.unwrap_or(20).max(1) as usize;
    let mut statement = connection.prepare(
        "SELECT visit_derived_facts.session_id, visit_derived_facts.registrable_domain, visits.visit_time_ms
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.session_id IS NOT NULL
           AND (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         ORDER BY visit_derived_facts.session_id ASC, visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut flows = HashMap::<(String, usize), FlowAggregate>::new();
    let mut current_session = String::new();
    let mut current_sequence = Vec::<(String, i64)>::new();
    for (session_id, domain, visit_time_ms) in rows {
        if current_session != session_id {
            finalize_path_sequence(&current_sequence, step_count, &mut flows);
            current_session = session_id;
            current_sequence.clear();
        }
        if current_sequence.last().is_none_or(|(last_domain, _)| *last_domain != domain) {
            current_sequence.push((domain, visit_time_ms));
        } else if let Some((_, last_seen_ms)) = current_sequence.last_mut() {
            *last_seen_ms = visit_time_ms;
        }
    }
    finalize_path_sequence(&current_sequence, step_count, &mut flows);

    let mut results = flows
        .into_iter()
        .map(|((flow_pattern, step_count), aggregate)| PathFlow {
            flow_pattern,
            step_count: step_count as i64,
            occurrence_count: aggregate.occurrence_count,
            last_seen_at: rfc3339_from_millis(aggregate.last_seen_ms),
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .occurrence_count
            .cmp(&left.occurrence_count)
            .then_with(|| right.last_seen_at.cmp(&left.last_seen_at))
            .then_with(|| left.flow_pattern.cmp(&right.flow_pattern))
    });
    results.truncate(limit);
    Ok(results)
}

pub fn get_observed_interactions(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<ObservedInteraction>> {
    let archive = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&archive)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let visits = load_observed_interaction_visits(
        &archive,
        request.profile_id.as_deref(),
        start_ms,
        end_ms,
    )?;
    if visits.is_empty() {
        return Ok(Vec::new());
    }

    let source_evidence = open_source_evidence_connection(paths, config, key)?;
    let evidence = load_engagement_evidence(&source_evidence, &visits)?;
    let mut interactions = visits
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
        .collect::<Vec<_>>();
    interactions.sort_by(|left, right| right.visit_id.cmp(&left.visit_id));
    Ok(interactions)
}

#[derive(Debug, Default)]
struct DomainSeries {
    days: BTreeSet<NaiveDate>,
    last_visited_ms: i64,
}

#[derive(Debug, Default)]
struct FlowAggregate {
    occurrence_count: i64,
    last_seen_ms: i64,
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

#[derive(Debug, Default, Clone)]
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

fn classify_habit_pattern(domain: &str, series: &DomainSeries) -> Option<HabitPattern> {
    if series.days.len() < 5 {
        return None;
    }
    let first_day = *series.days.first()?;
    let last_day = *series.days.last()?;
    if (last_day - first_day).num_days() < 14 {
        return None;
    }
    let ordered_days = series.days.iter().copied().collect::<Vec<_>>();
    let intervals = ordered_days
        .windows(2)
        .map(|window| (window[1] - window[0]).num_days() as f32)
        .collect::<Vec<_>>();
    if intervals.is_empty() {
        return None;
    }

    let mean_interval_days = intervals.iter().sum::<f32>() / intervals.len() as f32;
    let variance = intervals.iter().map(|value| (*value - mean_interval_days).powi(2)).sum::<f32>()
        / intervals.len() as f32;
    let cv = if mean_interval_days == 0.0 { 0.0 } else { variance.sqrt() / mean_interval_days };
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
        ((Local::now().timestamp_millis() - series.last_visited_ms).max(0) as f32 / 86_400_000.0)
            .max(0.0);

    Some(HabitPattern {
        registrable_domain: domain.to_string(),
        display_name: display_name_for_domain(domain),
        habit_type: habit_type.to_string(),
        mean_interval_days,
        cv,
        visit_count: series.days.len() as i64,
        last_visited_at: rfc3339_from_millis(series.last_visited_ms),
        is_interrupted: days_since_last_visit > mean_interval_days * 2.0,
    })
}

fn finalize_path_sequence(
    sequence: &[(String, i64)],
    step_count: usize,
    flows: &mut HashMap<(String, usize), FlowAggregate>,
) {
    if sequence.len() < step_count {
        return;
    }
    for window in sequence.windows(step_count) {
        if window.len() < 2 {
            continue;
        }
        let flow_pattern =
            window.iter().map(|(domain, _)| domain.as_str()).collect::<Vec<_>>().join(" → ");
        let last_seen_ms =
            window.iter().map(|(_, visit_time_ms)| *visit_time_ms).max().unwrap_or(0);
        let aggregate = flows.entry((flow_pattern, step_count)).or_default();
        aggregate.occurrence_count += 1;
        aggregate.last_seen_ms = aggregate.last_seen_ms.max(last_seen_ms);
    }
}

fn load_observed_interaction_visits(
    archive: &Connection,
    profile_id: Option<&str>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ObservedVisit>> {
    let mut statement = archive.prepare(
        "SELECT visits.id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS TEXT),
                urls.url,
                urls.title,
                COALESCE(source_profiles.browser_family, source_profiles.browser_kind)
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND visits.source_visit_id IS NOT NULL
           AND (?1 IS NULL OR source_profiles.profile_key = ?1)
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         ORDER BY visits.visit_time_ms DESC, visits.id DESC",
    )?;
    let visits = statement
        .query_map(params![profile_id, start_ms, end_ms], |row| {
            Ok(ObservedVisit {
                visit_id: row.get(0)?,
                source_profile_id: row.get(1)?,
                source_visit_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                browser_family: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(visits)
}

fn load_engagement_evidence(
    source_evidence: &Connection,
    visits: &[ObservedVisit],
) -> Result<HashMap<(i64, String), ObservedEvidence>> {
    let mut by_profile = HashMap::<i64, Vec<String>>::new();
    for visit in visits {
        by_profile.entry(visit.source_profile_id).or_default().push(visit.source_visit_id.clone());
    }

    let mut evidence = HashMap::<(i64, String), ObservedEvidence>::new();
    for (source_profile_id, visit_ids) in by_profile {
        for chunk in visit_ids.chunks(400) {
            let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(", ");
            let sql = format!(
                "SELECT source_profile_id, source_visit_id, metric_key, metric_value_int, metric_value_real
                 FROM visit_engagement_evidence
                 WHERE source_profile_id = ?
                   AND source_visit_id IN ({placeholders})"
            );
            let mut bindings = Vec::<SqlValue>::with_capacity(chunk.len() + 1);
            bindings.push(SqlValue::from(source_profile_id));
            bindings.extend(chunk.iter().cloned().map(SqlValue::from));
            let mut statement = source_evidence.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(bindings.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<f64>>(4)?,
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
    source_evidence: &Connection,
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
            let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(", ");
            let sql = format!(
                "SELECT source_profile_id, source_visit_id, context_key, value_json
                 FROM visit_context_evidence
                 WHERE source_profile_id = ?
                   AND source_visit_id IN ({placeholders})
                   AND context_key IN ('context.load_successful', 'context.page_end_reason')"
            );
            let mut bindings = Vec::<SqlValue>::with_capacity(chunk.len() + 1);
            bindings.push(SqlValue::from(source_profile_id));
            bindings.extend(chunk.iter().cloned().map(SqlValue::from));
            let mut statement = source_evidence.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(bindings.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            for row in rows {
                let (source_profile_id, source_visit_id, context_key, value_json) = row?;
                let Some(source_visit_id) = source_visit_id else {
                    continue;
                };
                has_rows = true;
                let entry = evidence.entry((source_profile_id, source_visit_id)).or_default();
                match context_key.as_str() {
                    "context.load_successful" => {
                        entry.load_successful = serde_json::from_str::<bool>(&value_json).ok();
                    }
                    "context.page_end_reason" => {
                        entry.page_end_reason = serde_json::from_str::<Option<String>>(&value_json)
                            .ok()
                            .flatten()
                            .or_else(|| serde_json::from_str::<String>(&value_json).ok());
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(has_rows.then_some(evidence))
}

#[cfg(test)]
mod tests {
    use super::{DomainSeries, classify_habit_pattern, finalize_path_sequence};
    use std::collections::{BTreeSet, HashMap};

    #[test]
    fn classify_habit_pattern_requires_five_visits_and_two_weeks() {
        let mut days = BTreeSet::new();
        for day in [1, 3, 5, 7, 9] {
            days.insert(chrono::NaiveDate::from_ymd_opt(2026, 4, day).expect("date"));
        }
        let series = DomainSeries { days, last_visited_ms: 0 };
        assert!(classify_habit_pattern("example.com", &series).is_none());
    }

    #[test]
    fn classify_habit_pattern_detects_weekly_habit() {
        let mut days = BTreeSet::new();
        for day in [1, 8, 15, 22, 29] {
            days.insert(chrono::NaiveDate::from_ymd_opt(2026, 4, day).expect("date"));
        }
        let series =
            DomainSeries { days, last_visited_ms: chrono::Local::now().timestamp_millis() };
        let habit = classify_habit_pattern("example.com", &series).expect("habit");
        assert_eq!(habit.habit_type, "weekly_habit");
        assert!(!habit.is_interrupted);
    }

    #[test]
    fn finalize_path_sequence_counts_windows() {
        let sequence = vec![
            ("google.com".to_string(), 10),
            ("github.com".to_string(), 20),
            ("sqlite.org".to_string(), 30),
        ];
        let mut flows = HashMap::new();
        finalize_path_sequence(&sequence, 2, &mut flows);
        finalize_path_sequence(&sequence, 3, &mut flows);
        assert_eq!(flows.len(), 3);
        assert!(flows.contains_key(&("google.com → github.com".to_string(), 2)));
        assert!(flows.contains_key(&("github.com → sqlite.org".to_string(), 2)));
        assert!(flows.contains_key(&("google.com → github.com → sqlite.org".to_string(), 3)));
    }
}
