//! Responsibilities: durable per-profile generations, cursors and publication.
//! Not responsible for: classification, aggregate math or provider work.
//! Failure modes: a crash loses a committed cursor, a build clears the live view,
//! profiles overwrite each other, publication leaves stale row counters, or a later stage loses the original dirty count.

use super::{CoreIntelligenceProgress, ProfileSourceWatermark, VisibleVisitBatchCursor};
use crate::Vault;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use vault_store::{Db, OptionalRow, Priority, Session, params_owned as params};

pub(super) const TABLES: &[(&str, &str)] = &[
    ("visit_derived_facts", "facts"),
    ("domain_daily_rollups", "daily"),
    ("category_daily_rollups", "daily"),
    ("engine_daily_rollups", "daily"),
    ("daily_summary_rollups", "daily"),
    ("sessions", "navigation"),
    ("search_trails", "navigation"),
    ("search_trail_members", "navigation"),
    ("search_events", "navigation"),
    ("search_event_terms", "navigation"),
    ("query_families", "aggregates"),
    ("query_family_queries", "aggregates"),
    ("refind_pages", "aggregates"),
    ("source_effectiveness", "aggregates"),
    ("habit_patterns", "aggregates"),
    ("reopened_investigations", "aggregates"),
    ("path_flows", "aggregates"),
    ("compare_sets", "aggregates"),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct BuildState {
    pub id: i64,
    pub profile_id: String,
    pub source_profile_id: i64,
    pub source: ProfileSourceWatermark,
    pub archive_revision: i64,
    pub rule_version: String,
    pub computed_at: String,
    pub full: bool,
    pub fallback: Option<String>,
    pub facts_generation: i64,
    pub daily_generation: i64,
    pub navigation_generation: i64,
    pub aggregate_generation: i64,
    pub cursor: Option<VisibleVisitBatchCursor>,
    pub facts_done: bool,
    pub daily_done: bool,
    pub daily_cursor: Option<VisibleVisitBatchCursor>,
    pub navigation_done: bool,
    pub aggregates_done: bool,
    pub refind_done: bool,
    pub refind_after: Option<String>,
    pub processed: usize,
    pub dirty_visit_count: usize,
    pub dirty_from_ms: Option<i64>,
    pub dirty_dates: Vec<String>,
    pub stream_json: Option<String>,
    pub structural_cursor: Option<VisibleVisitBatchCursor>,
    pub counts: std::collections::BTreeMap<String, usize>,
}

pub(super) fn load(s: &Session<'_>, profile: &str) -> Result<Option<BuildState>> {
    let json=s.query_one("SELECT state_json FROM intelligence_builds WHERE profile_id=?1 ORDER BY id DESC LIMIT 1",[profile],|r|Ok(r.get::<String>(0)?)).optional()?;
    json.map(|v| serde_json::from_str(&v).map_err(Into::into)).transpose()
}

pub(super) fn save(s: &Session<'_>, state: &BuildState) -> Result<()> {
    s.execute(
        "UPDATE intelligence_builds SET state_json=?2,updated_at=?3 WHERE id=?1",
        params![state.id, serde_json::to_string(state)?, crate::utils::now_rfc3339()],
    )?;
    Ok(())
}

pub(super) fn allocate(s: &Session<'_>, profile: &str) -> Result<i64> {
    Ok(s.query_one("INSERT INTO intelligence_builds(profile_id,state_json,updated_at) VALUES(?1,'{}',?2) RETURNING id",params![profile,crate::utils::now_rfc3339()],|r|Ok(r.get(0)?))?)
}

pub(super) fn publish(s: &Session<'_>, state: &BuildState, groups: &[&str]) -> Result<()> {
    s.execute("INSERT INTO intelligence_generations(profile_id) VALUES(?1) ON CONFLICT(profile_id) DO NOTHING",[state.profile_id.as_str()])?;
    for group in groups {
        let generation = match *group {
            "facts" => state.facts_generation,
            "daily" => state.daily_generation,
            "navigation" => state.navigation_generation,
            "aggregates" => state.aggregate_generation,
            _ => unreachable!(),
        };
        for (table, _) in TABLES.iter().filter(|(_, owner)| owner == group) {
            let count = state.counts.get(*table).copied().unwrap_or(0) as i64;
            s.execute("INSERT INTO intelligence_counts(generation,profile_id,table_name,value) VALUES(?1,?2,?3,?4) ON CONFLICT(generation,profile_id,table_name) DO UPDATE SET value=excluded.value",params![generation,state.profile_id,*table,count])?;
        }
        s.execute(
            &format!("UPDATE intelligence_generations SET {group}=?2 WHERE profile_id=?1"),
            params![state.profile_id, generation],
        )?;
    }
    save(s, state)
}

pub(super) fn progress(
    state: &BuildState,
    phase: &str,
    processed: usize,
    total: usize,
) -> CoreIntelligenceProgress {
    CoreIntelligenceProgress {
        phase: phase.into(),
        detail: format!("{} · {}", state.profile_id, phase),
        processed_items: Some(processed),
        total_items: Some(total),
        progress_percent: Some(if total == 0 {
            100.0
        } else {
            (processed as f32 / total as f32 * 100.0).min(100.0)
        }),
    }
}

/// Old generations are disposable after publication, but every delete yields the writer.
pub(super) fn discard_generation(
    vault: &Vault,
    generation: i64,
    profile: &str,
    tables: &[&str],
) -> Result<()> {
    for table in tables {
        let sql = format!(
            "DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE generation=?1 AND profile_id=?2 LIMIT 1024)"
        );
        loop {
            let sql = sql.clone();
            let profile = profile.to_owned();
            let n = vault.write(Db::Intelligence, Priority::Batch, move |s| {
                Ok(s.execute(&sql, params![generation, profile])?)
            })?;
            if n == 0 {
                break;
            }
        }
    }
    Ok(())
}

/// Reclaims superseded rows in bounded writes after every publication.
pub(super) fn collect_old_generations(vault: &Vault, state: &BuildState) -> Result<()> {
    let profile = state.profile_id.clone();
    let newest = state.id;
    let current=vault.read(Db::Intelligence,move |s| Ok(s.query_one("SELECT facts,daily,navigation,aggregates FROM intelligence_generations WHERE profile_id=?1",[profile],|r|Ok([r.get::<i64>(0)?,r.get(1)?,r.get(2)?,r.get(3)?]))?))?;
    let pending = [
        state.facts_generation,
        state.daily_generation,
        state.navigation_generation,
        state.aggregate_generation,
    ];
    let mut after = 0;
    loop {
        let profile = state.profile_id.clone();
        let generations=vault.read(Db::Intelligence,move |s| Ok(s.query("SELECT id FROM intelligence_builds WHERE profile_id=?1 AND id>?2 AND id<?3 ORDER BY id LIMIT 256",params![profile,after,newest],|r|Ok(r.get::<i64>(0)?))?))?;
        if generations.is_empty() {
            break;
        }
        after = *generations.last().unwrap();
        for generation in generations {
            let mut tables = Vec::new();
            for (name, group) in TABLES {
                let i = match *group {
                    "facts" => 0,
                    "daily" => 1,
                    "navigation" => 2,
                    "aggregates" => 3,
                    _ => unreachable!(),
                };
                if current[i] != generation && pending[i] != generation {
                    tables.push(format!("{name}_rows"));
                }
            }
            for (name, i) in [
                ("visit_assignments", 2),
                ("session_domains", 2),
                ("intelligence_seen_domains", 0),
                ("intelligence_day_counts", 1),
            ] {
                if current[i] != generation && pending[i] != generation {
                    tables.push(name.to_owned());
                }
            }
            discard_generation(
                vault,
                generation,
                &state.profile_id,
                &tables.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
            if !current.contains(&generation) && !pending.contains(&generation) {
                let profile = state.profile_id.clone();
                vault.write(Db::Intelligence, Priority::Batch, move |s| {
                    s.execute(
                        "DELETE FROM intelligence_counts WHERE generation=?1 AND profile_id=?2",
                        params![generation, profile],
                    )?;
                    s.execute("DELETE FROM intelligence_builds WHERE id=?1", [generation])?;
                    Ok(())
                })?;
            }
        }
    }
    Ok(())
}
