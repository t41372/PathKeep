//! Failure modes: late arrivals are mistaken for appends or recipe changes reuse stale facts.
//! Responsibilities: compare source watermarks and derivation recipes.
//! Not responsible for: archive ingestion or stage execution.
//! Incremental Core Intelligence checkpoint helpers.
//!
//! These helpers keep the Core Intelligence rebuild stages honest about whether a
//! queued job can stay incremental or must fall back to a scoped full refresh.

use crate::{
    intelligence_catalog::RebuildMode,
    visit_taxonomy::{taxonomy_version, url_rules_version},
};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
#[cfg(test)]
use vault_store::OptionalRow;
use vault_store::{Session, params_owned as params};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct ProfileSourceWatermark {
    pub visible_visit_count: i64,
    pub max_visit_id: i64,
    pub max_url_last_visit_ms: i64,
    pub visible_search_term_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct StageCheckpoint {
    pub profile_id: String,
    pub stage: String,
    pub stage_version: String,
    pub source_watermark: ProfileSourceWatermark,
    pub last_processed_visit_id: i64,
    pub dirty_from_visit_ms: Option<i64>,
    pub dirty_date_key: Option<String>,
    pub last_run_id: Option<i64>,
    pub fallback_reason: Option<String>,
    pub updated_at: String,
}

pub(super) fn stage_name(stage: RebuildMode) -> &'static str {
    match stage {
        RebuildMode::VisitDerive => "visit-derive",
        RebuildMode::DailyRollup => "daily-rollup",
        RebuildMode::StructuralRebuild => "structural-rebuild",
        RebuildMode::FullRebuild => "full-rebuild",
    }
}

pub(super) fn stage_version(connection: &Session<'_>, stage: RebuildMode) -> Result<String> {
    match stage {
        // v3: categories left out; they have their own version (`category_version`).
        RebuildMode::VisitDerive => Ok(format!(
            "visit-derived-facts-v3:{}:{}:{}",
            url_rules_version(),
            override_digest(connection, "search", "search_engine", "search_engine IS NOT NULL")?,
            super::intelligence_overview_snapshot::load_search_rule_signature(connection)?
        )),
        RebuildMode::DailyRollup => Ok("daily-rollups-v2".to_string()),
        // v3: Threads count the days a search came back on; compare sets need sibling pages.
        // v4: a full English stop-word list for rewordings, families and search words.
        RebuildMode::StructuralRebuild => Ok("structural-rebuild-v4".to_string()),
        RebuildMode::FullRebuild => Ok("full-rebuild-v2".to_string()),
    }
}

#[cfg(test)]
pub(super) fn load_stage_checkpoint(
    connection: &Session<'_>,
    profile_id: &str,
    stage: RebuildMode,
) -> Result<Option<StageCheckpoint>> {
    connection
        .query_one(
            "SELECT profile_id,
                    stage,
                    stage_version,
                    visible_visit_count,
                    max_visit_id,
                    max_url_last_visit_ms,
                    visible_search_term_count,
                    last_processed_visit_id,
                    dirty_from_visit_ms,
                    dirty_date_key,
                    last_run_id,
                    fallback_reason,
                    updated_at
             FROM core_intelligence_stage_checkpoints
             WHERE profile_id = ?1 AND stage = ?2",
            params![profile_id, stage_name(stage)],
            |row| {
                Ok(StageCheckpoint {
                    profile_id: row.get(0)?,
                    stage: row.get(1)?,
                    stage_version: row.get(2)?,
                    source_watermark: ProfileSourceWatermark {
                        visible_visit_count: row.get(3)?,
                        max_visit_id: row.get(4)?,
                        max_url_last_visit_ms: row.get(5)?,
                        visible_search_term_count: row.get(6)?,
                    },
                    last_processed_visit_id: row.get(7)?,
                    dirty_from_visit_ms: row.get(8)?,
                    dirty_date_key: row.get(9)?,
                    last_run_id: row.get(10)?,
                    fallback_reason: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

pub(super) fn save_stage_checkpoint(
    connection: &Session<'_>,
    checkpoint: &StageCheckpoint,
) -> Result<()> {
    connection.execute(
        "INSERT INTO core_intelligence_stage_checkpoints (
             profile_id,
             stage,
             stage_version,
             visible_visit_count,
             max_visit_id,
             max_url_last_visit_ms,
             visible_search_term_count,
             last_processed_visit_id,
             dirty_from_visit_ms,
             dirty_date_key,
             last_run_id,
             fallback_reason,
             updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(profile_id, stage) DO UPDATE SET
             stage_version = excluded.stage_version,
             visible_visit_count = excluded.visible_visit_count,
             max_visit_id = excluded.max_visit_id,
             max_url_last_visit_ms = excluded.max_url_last_visit_ms,
             visible_search_term_count = excluded.visible_search_term_count,
             last_processed_visit_id = excluded.last_processed_visit_id,
             dirty_from_visit_ms = excluded.dirty_from_visit_ms,
             dirty_date_key = excluded.dirty_date_key,
             last_run_id = excluded.last_run_id,
             fallback_reason = excluded.fallback_reason,
             updated_at = excluded.updated_at",
        params![
            checkpoint.profile_id,
            checkpoint.stage,
            checkpoint.stage_version,
            checkpoint.source_watermark.visible_visit_count,
            checkpoint.source_watermark.max_visit_id,
            checkpoint.source_watermark.max_url_last_visit_ms,
            checkpoint.source_watermark.visible_search_term_count,
            checkpoint.last_processed_visit_id,
            checkpoint.dirty_from_visit_ms,
            checkpoint.dirty_date_key,
            checkpoint.last_run_id,
            checkpoint.fallback_reason,
            checkpoint.updated_at,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn delete_stage_checkpoints(
    connection: &Session<'_>,
    profile_id: Option<&str>,
) -> Result<()> {
    if let Some(profile_id) = profile_id {
        connection.execute(
            "DELETE FROM core_intelligence_stage_checkpoints WHERE profile_id = ?1",
            [profile_id],
        )?;
    } else {
        connection.execute("DELETE FROM core_intelligence_stage_checkpoints", ())?;
    }
    Ok(())
}

/// Enumerates the profiles the all-time overview fingerprint must fold in,
/// using only small indexed tables.
///
/// Why a separate list from [`list_core_intelligence_profiles`]: the rebuild
/// path must discover every profile present in the *raw* archive (a
/// `SELECT DISTINCT` over `archive.visits`), but the fingerprint runs on every
/// all-time open and cannot afford to scan the multi-million-row visit index.
/// `source_profiles` holds one row per ingested browser profile and the
/// checkpoint ledger holds one row per (profile, rebuilt stage); their union is
/// a superset of `visit_derived_facts.profile_id` (every derived row descends
/// from a visit whose `source_profile_id` exists in `source_profiles`), so this
/// never under-counts a profile that could contribute data.
pub(super) fn list_fingerprint_profiles(
    connection: &Session<'_>,
    requested_profile_id: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(profile_id) = requested_profile_id {
        return Ok(vec![profile_id.to_string()]);
    }
    let mut profiles = BTreeSet::<String>::new();
    let mut collect = |sql: &str| -> Result<()> {
        let mut statement = connection.prepare(sql)?;
        let rows = statement
            .query_map((), |row| Ok(row.get::<String>(0)?))?
            .collect::<vault_store::Result<Vec<_>>>()?;
        profiles.extend(rows);
        Ok(())
    };
    collect("SELECT profile_key FROM archive.source_profiles")?;
    collect("SELECT DISTINCT profile_id FROM core_intelligence_stage_checkpoints")?;
    Ok(profiles.into_iter().collect())
}

/// Reads one profile's derived-state watermark straight from the checkpoint
/// ledger instead of re-scanning the raw archive.
///
/// The ledger captured these counts when the profile was last rebuilt, so this
/// is O(1) on a PK-indexed small table. It carries the same four dimensions as
/// [`load_profile_source_watermark`] but is the *fingerprint* source: pairing it
/// with [`load_archive_delta_signature`] (which catches ingest/reverts that have
/// not been rebuilt yet) reproduces the staleness sensitivity of the old live
/// scan without its cost. A profile with no checkpoint row yet contributes a
/// zero watermark, which the archive-delta signature still moves on first data.
pub(super) fn load_checkpoint_source_watermark(
    connection: &Session<'_>,
    profile_id: &str,
) -> Result<ProfileSourceWatermark> {
    // One profile owns up to one row per rebuild stage, all sharing the same
    // source watermark at steady state; MAX collapses them to that watermark
    // without multiply-counting across stages.
    let watermark = connection.query_one(
        "SELECT COALESCE(MAX(visible_visit_count), 0),
                COALESCE(MAX(max_visit_id), 0),
                COALESCE(MAX(max_url_last_visit_ms), 0),
                COALESCE(MAX(visible_search_term_count), 0)
         FROM core_intelligence_stage_checkpoints
         WHERE profile_id = ?1",
        [profile_id],
        |row| {
            Ok(ProfileSourceWatermark {
                visible_visit_count: row.get(0)?,
                max_visit_id: row.get(1)?,
                max_url_last_visit_ms: row.get(2)?,
                visible_search_term_count: row.get(3)?,
            })
        },
    )?;
    Ok(watermark)
}

/// Summarizes raw-archive mutations with constant-time probes so the fingerprint changes the
/// moment a backup ingests rows or a repair reverts them, even before the derived rebuild that
/// would move the checkpoint ledger has run.
///
/// `sqlite_sequence` holds the last id given to a visit and to a run (both tables are
/// AUTOINCREMENT): the first jumps on any ingested visit, the second on every backup, ingest and
/// revert. `archive_totals` has the visible visit count, written in the same transaction as each
/// of those, which moves when a revert or restore leaves both ids alone. `MAX(id)` and
/// `COUNT(*)` would read every row in Turso, which has no shortcut for them: for visits, the
/// whole archive on every all-time open and every Insights card read.
pub(super) fn load_archive_delta_signature(connection: &Session<'_>) -> Result<String> {
    let (last_visit_id, last_run_id): (i64, i64) = connection.query_one(
        "SELECT COALESCE(MAX(CASE WHEN name='visits' THEN seq END), 0),
                COALESCE(MAX(CASE WHEN name='runs' THEN seq END), 0)
         FROM archive.sqlite_sequence",
        (),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let visible: i64 = connection.query_one(
        "SELECT COALESCE((SELECT value FROM archive.archive_totals WHERE name='visits'), 0)",
        (),
        |row| Ok(row.get(0)?),
    )?;
    Ok(format!("arch:{last_visit_id}:{last_run_id}:{visible}"))
}

pub(super) fn watermark_regressed(
    current: &ProfileSourceWatermark,
    previous: &ProfileSourceWatermark,
) -> bool {
    current.visible_visit_count < previous.visible_visit_count
        || current.max_visit_id < previous.max_visit_id
        || current.max_url_last_visit_ms < previous.max_url_last_visit_ms
        || current.visible_search_term_count < previous.visible_search_term_count
}

/// The rules that put a visit's page and site into categories: the matcher, every rule pack and
/// the categories the user set for sites. When only these change, a build sorts the archive's
/// pages again instead of starting over (`build_reclassify`).
pub(super) fn category_version(connection: &Session<'_>) -> Result<String> {
    Ok(format!(
        "{}:{}",
        taxonomy_version(),
        override_digest(
            connection,
            "categories",
            "COALESCE(domain_category,'')||'|'||COALESCE(page_category,'')||'|'||COALESCE(interaction_kind,'')",
            "domain_category IS NOT NULL OR page_category IS NOT NULL OR interaction_kind IS NOT NULL",
        )?
    ))
}

/// A digest of what the user's site overrides set in one respect (`value`, for the rows
/// `filter` keeps), so a change there, and only there, changes it.
fn override_digest(
    connection: &Session<'_>,
    label: &str,
    value: &str,
    filter: &str,
) -> Result<String> {
    let rows = connection.query(
        &format!(
            "SELECT target_kind,target_value,{value} FROM site_dictionary_overrides
             WHERE {filter} ORDER BY target_kind,target_value"
        ),
        (),
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
    )?;
    let mut digest = Sha256::new();
    for (kind, target, value) in &rows {
        digest.update(format!("{kind}\u{1f}{target}\u{1f}{value}\u{1e}"));
    }
    Ok(format!("{label}:{}:{:x}", rows.len(), digest.finalize()))
}

pub(super) fn load_site_dictionary_signature(connection: &Session<'_>) -> Result<String> {
    connection
        .query_one(
            "SELECT COUNT(*), COALESCE(MAX(updated_at), 'none') FROM site_dictionary_overrides",
            (),
            |row| Ok((row.get::<i64>(0)?.max(0), row.get::<String>(1)?)),
        )
        .map(|(count, updated_at)| format!("overrides:{count}:{updated_at}"))
        .context("loading site dictionary signature")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_checkpoint_helpers_cover_names_versions_and_scoped_deletes() {
        let vault = crate::Vault::open_temp().unwrap();
        vault
            .write(
                vault_store::Db::Intelligence,
                vault_store::Priority::Interactive,
                |connection| {
                    assert_eq!(stage_name(RebuildMode::StructuralRebuild), "structural-rebuild");
                    assert_eq!(stage_name(RebuildMode::FullRebuild), "full-rebuild");
                    assert!(
                        stage_version(connection, RebuildMode::VisitDerive)
                            .expect("visit version")
                            .starts_with("visit-derived-facts-v3:")
                    );
                    assert_eq!(
                        stage_version(connection, RebuildMode::StructuralRebuild)
                            .expect("structural version"),
                        "structural-rebuild-v4"
                    );
                    assert_eq!(
                        stage_version(connection, RebuildMode::FullRebuild).expect("full version"),
                        "full-rebuild-v2"
                    );

                    save_stage_checkpoint(
                        connection,
                        &StageCheckpoint {
                            profile_id: "profile-a".to_string(),
                            stage: stage_name(RebuildMode::DailyRollup).to_string(),
                            stage_version: "daily-rollups-v2".to_string(),
                            source_watermark: ProfileSourceWatermark {
                                visible_visit_count: 1,
                                max_visit_id: 1,
                                max_url_last_visit_ms: 1,
                                visible_search_term_count: 1,
                            },
                            last_processed_visit_id: 1,
                            updated_at: "2026-04-26T00:00:00Z".to_string(),
                            ..StageCheckpoint::default()
                        },
                    )
                    .expect("checkpoint a");
                    save_stage_checkpoint(
                        connection,
                        &StageCheckpoint {
                            profile_id: "profile-b".to_string(),
                            stage: stage_name(RebuildMode::DailyRollup).to_string(),
                            stage_version: "daily-rollups-v2".to_string(),
                            source_watermark: ProfileSourceWatermark {
                                visible_visit_count: 2,
                                max_visit_id: 2,
                                max_url_last_visit_ms: 2,
                                visible_search_term_count: 2,
                            },
                            last_processed_visit_id: 2,
                            updated_at: "2026-04-26T00:00:01Z".to_string(),
                            ..StageCheckpoint::default()
                        },
                    )
                    .expect("checkpoint b");

                    delete_stage_checkpoints(connection, Some("profile-a"))
                        .expect("delete profile a");
                    assert!(
                        load_stage_checkpoint(connection, "profile-a", RebuildMode::DailyRollup)
                            .expect("load profile a")
                            .is_none()
                    );
                    assert!(
                        load_stage_checkpoint(connection, "profile-b", RebuildMode::DailyRollup)
                            .expect("load profile b")
                            .is_some()
                    );
                    delete_stage_checkpoints(connection, None).expect("delete all");
                    assert!(
                        load_stage_checkpoint(connection, "profile-b", RebuildMode::DailyRollup)
                            .expect("load profile b after delete")
                            .is_none()
                    );
                    Ok(())
                },
            )
            .unwrap();
    }
}
