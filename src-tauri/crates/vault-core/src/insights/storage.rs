//! SQLite persistence helpers for deterministic insights.

use super::{INSIGHT_PIPELINE_VERSION, VisitRecord, evidence_from_visit};
use crate::models::{
    InsightQueryGroupSummary, InsightReferencePageSummary, InsightSourceEffectivenessSummary,
    InsightTopicSummary,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::{Connection, Row, params};

use super::grouping::{BurstRecord, QueryGroupRecord, ThreadRecord};

pub(super) fn persist_bursts(connection: &Connection, bursts: &[BurstRecord]) -> Result<()> {
    connection.execute("DELETE FROM insight_bursts", [])?;
    for burst in bursts {
        connection.execute(
            "INSERT INTO insight_bursts
             (burst_id, profile_id, first_seen_at, last_seen_at, visit_count, confidence, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                burst.burst_id,
                burst.profile_id,
                burst.first_seen_at,
                burst.last_seen_at,
                burst.visit_indexes.len() as i64,
                burst.confidence,
                serde_json::to_string(&burst.visit_indexes)?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn persist_query_groups(
    connection: &Connection,
    query_groups: &[QueryGroupRecord],
    visits: &[VisitRecord],
) -> Result<()> {
    connection.execute("DELETE FROM insight_query_groups", [])?;
    connection.execute("DELETE FROM insight_query_group_members", [])?;
    for group in query_groups {
        connection.execute(
            "INSERT INTO insight_query_groups
             (query_group_id, profile_id, thread_id, title, root_query, latest_query, first_seen_at,
              last_seen_at, visit_count, burst_count, step_count, confidence, evidence_tier,
              chromium_enhanced, steps_json, stages_json, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                group.query_group_id,
                group.profile_id,
                group.thread_id,
                group.title,
                group.root_query,
                group.latest_query,
                group.first_seen_at,
                group.last_seen_at,
                group.visit_indexes.len() as i64,
                group.burst_ids.len() as i64,
                group.steps.len() as i64,
                group.confidence,
                group.evidence_tier,
                group.chromium_enhanced as i64,
                serde_json::to_string(&group.steps)?,
                serde_json::to_string(&group.stages)?,
                serde_json::to_string(
                    &group
                        .visit_indexes
                        .iter()
                        .rev()
                        .take(4)
                        .map(|index| evidence_from_visit(&visits[*index], Some(group.title.clone())))
                        .collect::<Vec<_>>(),
                )?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
        for (ordinal, index) in group.visit_indexes.iter().enumerate() {
            connection.execute(
                "INSERT INTO insight_query_group_members
                 (query_group_id, history_id, ordinal, visited_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    group.query_group_id,
                    visits[*index].history_id,
                    ordinal as i64,
                    visits[*index].visited_at,
                ],
            )?;
        }
    }
    Ok(())
}

pub(super) fn persist_topic_summaries(
    connection: &Connection,
    profile_scope: &str,
    window_days: u32,
    topics: &[InsightTopicSummary],
) -> Result<()> {
    connection.execute(
        "DELETE FROM insight_topics WHERE profile_scope = ?1 AND window_days = ?2",
        params![profile_scope, window_days as i64],
    )?;
    for topic in topics {
        connection.execute(
            "INSERT OR REPLACE INTO insight_topics
             (topic_id, profile_scope, window_days, label, first_seen_at, last_seen_at, visit_count,
              revisit_count, trend_slope, burst_score, evidence_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                topic.topic_id,
                profile_scope,
                window_days as i64,
                topic.label,
                topic.first_seen_at,
                topic.last_seen_at,
                topic.visit_count as i64,
                topic.revisit_count as i64,
                topic.trend_slope,
                topic.burst_score,
                serde_json::to_string(&topic.evidence)?,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn persist_threads(
    connection: &Connection,
    threads: &[ThreadRecord],
    visits: &[VisitRecord],
) -> Result<()> {
    connection.execute("DELETE FROM insight_threads", [])?;
    connection.execute("DELETE FROM insight_thread_members", [])?;
    for thread in threads {
        connection.execute(
            "INSERT INTO insight_threads
             (thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
              query_group_count, reopen_count, open_loop_score, confidence, evidence_tier,
              dominant_topic_id, chromium_enhanced, evidence_json, summary_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, ?16)",
            params![
                thread.thread_id,
                thread.profile_id,
                thread.title,
                thread.status,
                thread.first_seen_at,
                thread.last_seen_at,
                thread.visit_indexes.len() as i64,
                thread.query_group_ids.len() as i64,
                thread.reopen_count as i64,
                thread.open_loop_score,
                thread.confidence,
                thread.evidence_tier,
                thread.chromium_enhanced as i64,
                serde_json::to_string(&thread.evidence)?,
                serde_json::to_string(&serde_json::json!({
                    "queryGroupCount": thread.query_group_ids.len(),
                    "reopenCount": thread.reopen_count,
                    "openLoopScore": thread.open_loop_score,
                }))?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
        for (ordinal, visit_index) in thread.visit_indexes.iter().enumerate() {
            connection.execute(
                "INSERT INTO insight_thread_members (thread_id, history_id, ordinal, visited_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    thread.thread_id,
                    visits[*visit_index].history_id,
                    ordinal as i64,
                    visits[*visit_index].visited_at,
                ],
            )?;
        }
    }
    Ok(())
}

pub(super) fn persist_reference_pages(
    connection: &Connection,
    profile_scope: &str,
    reference_pages: &[InsightReferencePageSummary],
) -> Result<()> {
    connection
        .execute("DELETE FROM insight_reference_pages WHERE profile_scope = ?1", [profile_scope])?;
    for page in reference_pages {
        connection.execute(
            "INSERT INTO insight_reference_pages
             (reference_page_id, profile_scope, url, title, domain, first_seen_at, last_seen_at,
              revisit_count, cross_day_revisits, query_group_count, thread_count, score,
              evidence_tier, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                page.reference_page_id,
                profile_scope,
                page.url,
                page.title,
                page.domain,
                page.first_seen_at,
                page.last_seen_at,
                page.revisit_count as i64,
                page.cross_day_revisits as i64,
                page.query_group_count as i64,
                page.thread_count as i64,
                page.score,
                page.evidence_tier,
                serde_json::to_string(&page.evidence)?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn persist_source_effectiveness(
    connection: &Connection,
    profile_scope: &str,
    source_effectiveness: &[InsightSourceEffectivenessSummary],
) -> Result<()> {
    connection.execute(
        "DELETE FROM insight_source_effectiveness WHERE profile_scope = ?1",
        [profile_scope],
    )?;
    for row in source_effectiveness {
        connection.execute(
            "INSERT INTO insight_source_effectiveness
             (source_id, profile_scope, domain, source_role, query_group_count, thread_count,
              stable_landing_count, reference_page_count, reopen_support_count,
              effectiveness_score, evidence_tier, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                row.source_id,
                profile_scope,
                row.domain,
                row.source_role,
                row.query_group_count as i64,
                row.thread_count as i64,
                row.stable_landing_count as i64,
                row.reference_page_count as i64,
                row.reopen_support_count as i64,
                row.effectiveness_score,
                row.evidence_tier,
                serde_json::to_string(&row.evidence)?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn load_query_groups(
    connection: &Connection,
    profile_id: Option<&str>,
    window_days: u32,
) -> Result<Vec<InsightQueryGroupSummary>> {
    let start = (Utc::now() - Duration::days(window_days as i64)).to_rfc3339();
    let sql = if profile_id.is_some() {
        "SELECT query_group_id, profile_id, thread_id, title, root_query, latest_query,
                first_seen_at, last_seen_at, visit_count, burst_count, step_count, confidence,
                evidence_tier, chromium_enhanced, steps_json, stages_json, evidence_json
         FROM insight_query_groups
         WHERE profile_id = ?1 AND last_seen_at >= ?2
         ORDER BY last_seen_at DESC"
    } else {
        "SELECT query_group_id, profile_id, thread_id, title, root_query, latest_query,
                first_seen_at, last_seen_at, visit_count, burst_count, step_count, confidence,
                evidence_tier, chromium_enhanced, steps_json, stages_json, evidence_json
         FROM insight_query_groups
         WHERE last_seen_at >= ?1
         ORDER BY last_seen_at DESC"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = if let Some(profile_id) = profile_id {
        statement.query_map(params![profile_id, start], query_group_summary_from_row)?
    } else {
        statement.query_map(params![start], query_group_summary_from_row)?
    };
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn load_reference_pages(
    connection: &Connection,
    profile_scope: &str,
) -> Result<Vec<InsightReferencePageSummary>> {
    let mut statement = connection.prepare(
        "SELECT reference_page_id, url, title, domain, first_seen_at, last_seen_at, revisit_count,
                cross_day_revisits, query_group_count, thread_count, score, evidence_tier, evidence_json
         FROM insight_reference_pages
         WHERE profile_scope = ?1
         ORDER BY score DESC, last_seen_at DESC",
    )?;
    let rows = statement.query_map([profile_scope], |row| {
        Ok(InsightReferencePageSummary {
            reference_page_id: row.get(0)?,
            profile_id: (profile_scope != "all").then(|| profile_scope.to_string()),
            url: row.get(1)?,
            title: row.get(2)?,
            domain: row.get(3)?,
            first_seen_at: row.get(4)?,
            last_seen_at: row.get(5)?,
            revisit_count: row.get::<_, i64>(6)?.max(0) as usize,
            cross_day_revisits: row.get::<_, i64>(7)?.max(0) as usize,
            query_group_count: row.get::<_, i64>(8)?.max(0) as usize,
            thread_count: row.get::<_, i64>(9)?.max(0) as usize,
            score: row.get(10)?,
            evidence_tier: row.get(11)?,
            evidence: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn load_source_effectiveness(
    connection: &Connection,
    profile_scope: &str,
) -> Result<Vec<InsightSourceEffectivenessSummary>> {
    let mut statement = connection.prepare(
        "SELECT source_id, domain, source_role, query_group_count, thread_count, stable_landing_count,
                reference_page_count, reopen_support_count, effectiveness_score, evidence_tier,
                evidence_json
         FROM insight_source_effectiveness
         WHERE profile_scope = ?1
         ORDER BY effectiveness_score DESC, domain ASC",
    )?;
    let rows = statement.query_map([profile_scope], |row| {
        Ok(InsightSourceEffectivenessSummary {
            source_id: row.get(0)?,
            profile_id: (profile_scope != "all").then(|| profile_scope.to_string()),
            domain: row.get(1)?,
            source_role: row.get(2)?,
            query_group_count: row.get::<_, i64>(3)?.max(0) as usize,
            thread_count: row.get::<_, i64>(4)?.max(0) as usize,
            stable_landing_count: row.get::<_, i64>(5)?.max(0) as usize,
            reference_page_count: row.get::<_, i64>(6)?.max(0) as usize,
            reopen_support_count: row.get::<_, i64>(7)?.max(0) as usize,
            effectiveness_score: row.get(8)?,
            evidence_tier: row.get(9)?,
            evidence: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn load_thread_query_groups(
    connection: &Connection,
    thread_id: &str,
) -> Result<Vec<InsightQueryGroupSummary>> {
    let mut statement = connection.prepare(
        "SELECT query_group_id, profile_id, thread_id, title, root_query, latest_query,
                first_seen_at, last_seen_at, visit_count, burst_count, step_count, confidence,
                evidence_tier, chromium_enhanced, steps_json, stages_json, evidence_json
         FROM insight_query_groups
         WHERE thread_id = ?1
         ORDER BY first_seen_at ASC",
    )?;
    let rows = statement.query_map([thread_id], query_group_summary_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn query_group_summary_from_row(row: &Row<'_>) -> rusqlite::Result<InsightQueryGroupSummary> {
    Ok(InsightQueryGroupSummary {
        query_group_id: row.get(0)?,
        profile_id: row.get(1)?,
        thread_id: row.get(2)?,
        title: row.get(3)?,
        root_query: row.get(4)?,
        latest_query: row.get(5)?,
        first_seen_at: row.get(6)?,
        last_seen_at: row.get(7)?,
        visit_count: row.get::<_, i64>(8)?.max(0) as usize,
        burst_count: row.get::<_, i64>(9)?.max(0) as usize,
        step_count: row.get::<_, i64>(10)?.max(0) as usize,
        confidence: row.get(11)?,
        evidence_tier: row.get(12)?,
        chromium_enhanced: row.get::<_, i64>(13)? != 0,
        steps: serde_json::from_str(&row.get::<_, String>(14)?).unwrap_or_default(),
        stages: serde_json::from_str(&row.get::<_, String>(15)?).unwrap_or_default(),
        evidence: serde_json::from_str(&row.get::<_, String>(16)?).unwrap_or_default(),
    })
}
