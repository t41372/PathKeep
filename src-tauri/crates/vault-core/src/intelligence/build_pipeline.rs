//! Responsibilities: choose resumable profile work and publish complete derived generations.
//! Not responsible for: provider calls, frontend transport or connection ownership.
//! Failure modes: lost cursors, duplicate replay, clearing a live generation, late
//! visits treated as an append, or a build monopolizing the writer.

use super::build_state::{self, BuildState};
use super::*;
use crate::Vault;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use vault_store::{Db, OptionalRow, Priority, Session, params_owned as params};

#[derive(Clone)]
pub(super) struct Source {
    pub profile_id: String,
    pub source_id: i64,
    pub watermark: ProfileSourceWatermark,
    pub revision: i64,
}

pub(super) fn sources(vault: &Vault, profile: Option<&str>) -> Result<Vec<Source>> {
    let profile = profile.map(str::to_owned);
    vault.read(Db::Archive,move |s| {
        let max_id=s.query_one("SELECT COALESCE(MAX(id),0) FROM visits",(),|r|Ok(r.get::<i64>(0)?))?;
        let revision=s.query_one("SELECT COALESCE(MAX(id),0) FROM runs WHERE run_type IN ('backup','import','rollback','restore','snapshot_restore')",(),|r|Ok(r.get::<i64>(0)?))?;
        let profiles=s.query("SELECT id,profile_key FROM source_profiles WHERE ?1 IS NULL OR profile_key=?1 ORDER BY id",params![profile],|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?)))?;
        let mut result=Vec::new();
        for (id,profile_id) in profiles {
            let count=s.query_one("SELECT value FROM archive_totals WHERE name=?1",[format!("sourceVisits:{id}")],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
            let latest=s.query_one("SELECT last_visit_ms FROM urls WHERE source_profile_id=?1 ORDER BY last_visit_ms DESC LIMIT 1",[id],|r|Ok(r.get::<i64>(0)?)).optional()?.unwrap_or(0);
            let terms=s.query_one("SELECT COUNT(*) FROM search_terms WHERE source_profile_id=?1 AND reverted_at IS NULL",[id],|r|Ok(r.get::<i64>(0)?))?;
            result.push(Source {profile_id,source_id:id,revision,watermark:ProfileSourceWatermark {
                visible_visit_count:count,max_visit_id:max_id,max_url_last_visit_ms:latest,visible_search_term_count:terms}});
        }
        Ok(result)
    })
}

/// The rules a build classifies and groups visits by. A profile last built with others is built
/// again from scratch.
fn rule_version(s: &Session<'_>) -> Result<String> {
    Ok(format!(
        "{}|{}|{}",
        stage_version(s, RebuildMode::VisitDerive)?,
        stage_version(s, RebuildMode::DailyRollup)?,
        stage_version(s, RebuildMode::StructuralRebuild)?
    ))
}

/// Whether the Insights of some profile were last built with rules other than this version's (a
/// new site list, a changed matcher): they show the old labels until the next build.
pub(crate) fn built_with_other_rules(s: &Session<'_>) -> Result<bool> {
    let current = rule_version(s)?;
    let profiles = s.query("SELECT profile_id FROM intelligence_generations", params![], |r| {
        Ok(r.get::<String>(0)?)
    })?;
    for profile in profiles {
        if build_state::load(s, &profile)?.is_some_and(|state| state.rule_version != current) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn prepare(vault: &Vault, source: &Source, force_full: bool) -> Result<BuildState> {
    let profile = source.profile_id.clone();
    let (previous, rule_version) = vault
        .read(Db::Intelligence, move |s| Ok((build_state::load(s, &profile)?, rule_version(s)?)))?;
    let pending = previous.as_ref().is_some_and(|old| {
        !(old.facts_done && old.daily_done && old.navigation_done && old.aggregates_done)
    });
    if let Some(old) = previous.as_ref()
        && pending
        && old.rule_version == rule_version
        && old.source == source.watermark
        && old.archive_revision == source.revision
    {
        return Ok(old.clone());
    }
    let rules_changed = previous.as_ref().is_some_and(|old| old.rule_version != rule_version);
    let mut full = force_full || previous.is_none() || rules_changed || pending;
    let mut fallback = if force_full {
        Some("manual-full-rebuild")
    } else if previous.is_none() {
        Some("no-checkpoint")
    } else if rules_changed {
        Some("rules-changed")
    } else if pending {
        Some(
            if super::incremental::watermark_regressed(
                &source.watermark,
                &previous.as_ref().unwrap().source,
            ) {
                "visibility-regressed"
            } else {
                "delta-mismatch"
            },
        )
    } else {
        None
    };
    let mut dirty = None;
    let mut appended = 0;
    if let Some(old) = previous.as_ref() {
        if old.source == source.watermark && old.archive_revision == source.revision && !full {
            return Ok(old.clone());
        }
        if !full {
            let source_id = source.source_id;
            let after = old.source.max_visit_id;
            let upper = source.watermark.max_visit_id;
            let (count, min_time) = vault.read(Db::Archive, move |s| {
                Ok(s.query_one(
                    "SELECT COUNT(*),MIN(visit_time_ms) FROM visits NOT INDEXED
                 WHERE id>?1 AND id<=?2 AND source_profile_id=?3 AND reverted_at IS NULL",
                    params![after, upper, source_id],
                    |r| Ok((r.get::<i64>(0)?, r.get::<Option<i64>>(1)?)),
                )?)
            })?;
            full = count != source.watermark.visible_visit_count - old.source.visible_visit_count
                || min_time
                    .zip(old.cursor.map(|c| c.visit_time_ms))
                    .is_some_and(|(new, old)| new < old)
                || (count == 0
                    && (old.archive_revision != source.revision || old.source != source.watermark));
            if full {
                fallback = Some(
                    if super::incremental::watermark_regressed(&source.watermark, &old.source) {
                        "visibility-regressed"
                    } else {
                        "delta-mismatch"
                    },
                );
            }
            dirty = min_time;
            appended = count;
        }
    }
    let dirty_visit_count = previous.as_ref().map_or(source.watermark.visible_visit_count, |old| {
        if force_full
            || rules_changed
            || pending
            || source.watermark.visible_visit_count < old.source.visible_visit_count
        {
            source.watermark.visible_visit_count
        } else {
            source.watermark.visible_visit_count - old.source.visible_visit_count
        }
    }) as usize;
    // An append small enough for one write updates the published aggregates in place; this is
    // what it needs from before. A larger one builds them into a new generation.
    let append = match (previous.as_ref(), dirty) {
        (Some(old), Some(first_new_ms))
            if !full && appended <= super::build_append::IN_PLACE_LIMIT =>
        {
            Some(super::build_append::base(vault, old, first_new_ms)?)
        }
        _ => None,
    };
    let source = source.clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        let id = build_state::allocate(s, &source.profile_id)?;
        let old = previous.as_ref();
        let reuse = |field: fn(&BuildState) -> i64| if full { id } else { field(old.unwrap()) };
        let state = BuildState {
            id,
            profile_id: source.profile_id,
            source_profile_id: source.source_id,
            source: source.watermark,
            archive_revision: source.revision,
            rule_version,
            computed_at: crate::utils::now_rfc3339(),
            full,
            fallback: fallback.map(str::to_owned),
            facts_generation: reuse(|s| s.facts_generation),
            daily_generation: reuse(|s| s.daily_generation),
            navigation_generation: reuse(|s| s.navigation_generation),
            aggregate_generation: if append.is_some() {
                reuse(|s| s.aggregate_generation)
            } else {
                id
            },
            cursor: if full { None } else { old.and_then(|s| s.cursor) },
            facts_done: false,
            daily_done: false,
            daily_cursor: None,
            navigation_done: false,
            aggregates_done: false,
            refind_done: false,
            refind_after: None,
            processed: if full { 0 } else { old.unwrap().processed },
            dirty_from_ms: dirty,
            dirty_visit_count,
            dirty_dates: Vec::new(),
            stream_json: if full { None } else { old.and_then(|s| s.stream_json.clone()) },
            structural_cursor: if full { None } else { old.and_then(|s| s.structural_cursor) },
            counts: if full { BTreeMap::new() } else { old.unwrap().counts.clone() },
            append,
        };
        let mut state = state;
        if state.append.is_none() {
            for (table, group) in build_state::TABLES {
                if *group == "aggregates" {
                    state.counts.remove(*table);
                }
            }
        }
        build_state::save(s, &state)?;
        Ok(state)
    })
}

pub(super) fn derive<F>(
    vault: &Vault,
    state: &mut BuildState,
    batch_size: usize,
    on_progress: &mut F,
) -> Result<()>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if state.facts_done {
        return Ok(());
    }
    let (overrides, rules) = vault.read(Db::Intelligence, |s| {
        Ok((load_site_dictionary_overrides(s)?, load_enabled_search_engine_rules(s)?))
    })?;
    let mut seen = HashSet::new();
    let mut after = String::new();
    loop {
        let generation = state.facts_generation;
        let profile = state.profile_id.clone();
        let cursor = after.clone();
        let page=vault.read(Db::Intelligence,move |s|Ok(s.query("SELECT domain FROM intelligence_seen_domains WHERE generation=?1 AND profile_id=?2 AND domain>?3 ORDER BY domain LIMIT 1024",params![generation,profile,cursor],|r|Ok(r.get::<String>(0)?))?))?;
        if page.is_empty() {
            break;
        }
        after = page.last().unwrap().clone();
        seen.extend(page);
    }
    let mut dates: BTreeSet<String> = state.dirty_dates.iter().cloned().collect();
    on_progress(build_state::progress(
        state,
        "visit-derived-facts",
        state.processed,
        state.source.visible_visit_count as usize,
    ))?;
    loop {
        let mut batch = super::build_rows::source_page(vault, state, state.cursor, batch_size)?;
        if batch.is_empty() {
            break;
        }
        for visit in &mut batch {
            let query = visit.search_query.clone();
            let dictionary = classify_visit(
                &visit.url,
                visit.title.as_deref(),
                query.as_deref(),
                query.is_some(),
                visit.external_referrer_url.as_deref(),
                visit.from_visit,
                &overrides,
                &rules,
            );
            super::intelligence_visit_records::apply_site_dictionary(visit, query, dictionary);
            visit.is_new_domain = seen.insert(visit.registrable_domain.clone());
            dates.insert(local_date_key(visit.visit_time_ms));
            state.dirty_from_ms = Some(
                state.dirty_from_ms.map_or(visit.visit_time_ms, |v| v.min(visit.visit_time_ms)),
            );
        }
        let last = batch.last().unwrap();
        state.cursor = Some(VisibleVisitBatchCursor {
            visit_time_ms: last.visit_time_ms,
            visit_id: last.visit_id,
        });
        state.processed += batch.len();
        state.dirty_dates = dates.iter().cloned().collect();
        let saved = state.clone();
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            super::build_rows::persist_facts(s, &saved, &batch)?;
            build_state::save(s, &saved)
        })?;
        on_progress(build_state::progress(
            state,
            "visit-derived-facts",
            state.processed,
            state.source.visible_visit_count as usize,
        ))?;
    }
    state.facts_done = true;
    state.counts.insert("visit_derived_facts".into(), state.processed);
    checkpoint(vault, state, RebuildMode::VisitDerive)
}

pub(super) fn checkpoint(vault: &Vault, state: &BuildState, stage: RebuildMode) -> Result<()> {
    let state = state.clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        save_stage_checkpoint(
            s,
            &StageCheckpoint {
                profile_id: state.profile_id.clone(),
                stage: stage_name(stage).into(),
                stage_version: stage_version(s, stage)?,
                source_watermark: state.source.clone(),
                last_processed_visit_id: state.source.max_visit_id,
                dirty_from_visit_ms: state.dirty_from_ms,
                dirty_date_key: state.dirty_dates.first().cloned(),
                last_run_id: Some(state.id),
                fallback_reason: state.full.then(|| "Full generation rebuild.".into()),
                updated_at: state.computed_at.clone(),
            },
        )?;
        build_state::save(s, &state)
    })
}

pub(super) fn publish(vault: &Vault, state: &BuildState, groups: &[&str]) -> Result<()> {
    let state = state.clone();
    let groups = groups.iter().map(|g| g.to_string()).collect::<Vec<_>>();
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        build_state::publish(s, &state, &groups.iter().map(String::as_str).collect::<Vec<_>>())
    })
}
