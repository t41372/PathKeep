//! Incremental Core Intelligence checkpoint helpers.
//!
//! These helpers keep the Core Intelligence rebuild stages honest about whether a
//! queued job can stay incremental or must fall back to a scoped full refresh.

use crate::{intelligence_catalog::RebuildMode, visit_taxonomy::taxonomy_version};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

pub(super) const CORE_INTELLIGENCE_STAGE_CHECKPOINTS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS core_intelligence_stage_checkpoints (
  profile_id                TEXT NOT NULL,
  stage                     TEXT NOT NULL,
  stage_version             TEXT NOT NULL,
  visible_visit_count       INTEGER NOT NULL DEFAULT 0,
  max_visit_id              INTEGER NOT NULL DEFAULT 0,
  max_url_last_visit_ms     INTEGER NOT NULL DEFAULT 0,
  visible_search_term_count INTEGER NOT NULL DEFAULT 0,
  last_processed_visit_id   INTEGER NOT NULL DEFAULT 0,
  dirty_from_visit_ms       INTEGER,
  dirty_date_key            TEXT,
  last_run_id               INTEGER,
  fallback_reason           TEXT,
  updated_at                TEXT NOT NULL,
  PRIMARY KEY(profile_id, stage)
);
"#;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ProfileSourceWatermark {
    pub visible_visit_count: i64,
    pub max_visit_id: i64,
    pub max_url_last_visit_ms: i64,
    pub visible_search_term_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StageExecutionMode {
    Incremental,
    FallbackFull,
    Noop,
}

impl StageExecutionMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::FallbackFull => "fallback-full",
            Self::Noop => "noop",
        }
    }
}

pub(super) fn ensure_core_intelligence_stage_checkpoint_schema(
    connection: &Connection,
) -> Result<()> {
    connection.execute_batch(CORE_INTELLIGENCE_STAGE_CHECKPOINTS_SQL)?;
    Ok(())
}

pub(super) fn stage_name(stage: RebuildMode) -> &'static str {
    match stage {
        RebuildMode::VisitDerive => "visit-derive",
        RebuildMode::DailyRollup => "daily-rollup",
        RebuildMode::StructuralRebuild => "structural-rebuild",
        RebuildMode::FullRebuild => "full-rebuild",
    }
}

pub(super) fn stage_version(connection: &Connection, stage: RebuildMode) -> Result<String> {
    match stage {
        RebuildMode::VisitDerive => Ok(format!(
            "visit-derived-facts-v2:{}:{}",
            taxonomy_version(),
            load_site_dictionary_signature(connection)?
        )),
        RebuildMode::DailyRollup => Ok("daily-rollups-v2".to_string()),
        RebuildMode::StructuralRebuild => Ok("structural-rebuild-v2".to_string()),
        RebuildMode::FullRebuild => Ok("full-rebuild-v2".to_string()),
    }
}

pub(super) fn load_stage_checkpoint(
    connection: &Connection,
    profile_id: &str,
    stage: RebuildMode,
) -> Result<Option<StageCheckpoint>> {
    ensure_core_intelligence_stage_checkpoint_schema(connection)?;
    connection
        .query_row(
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
    connection: &Connection,
    checkpoint: &StageCheckpoint,
) -> Result<()> {
    ensure_core_intelligence_stage_checkpoint_schema(connection)?;
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

pub(super) fn delete_stage_checkpoints(
    connection: &Connection,
    profile_id: Option<&str>,
) -> Result<()> {
    ensure_core_intelligence_stage_checkpoint_schema(connection)?;
    if let Some(profile_id) = profile_id {
        connection.execute(
            "DELETE FROM core_intelligence_stage_checkpoints WHERE profile_id = ?1",
            [profile_id],
        )?;
    } else {
        connection.execute("DELETE FROM core_intelligence_stage_checkpoints", [])?;
    }
    Ok(())
}

pub(super) fn list_core_intelligence_profiles(
    connection: &Connection,
    requested_profile_id: Option<&str>,
) -> Result<Vec<String>> {
    ensure_core_intelligence_stage_checkpoint_schema(connection)?;
    if let Some(profile_id) = requested_profile_id {
        return Ok(vec![profile_id.to_string()]);
    }
    let mut profiles = BTreeSet::<String>::new();
    let mut collect = |sql: &str| -> Result<()> {
        let mut statement = connection.prepare(sql)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        profiles.extend(rows);
        Ok(())
    };
    collect(
        "SELECT DISTINCT source_profiles.profile_key
         FROM archive.visits
         JOIN archive.source_profiles ON archive.source_profiles.id = archive.visits.source_profile_id
         WHERE archive.visits.reverted_at IS NULL",
    )?;
    collect("SELECT DISTINCT profile_id FROM visit_derived_facts")?;
    collect("SELECT DISTINCT profile_id FROM core_intelligence_stage_checkpoints")?;
    Ok(profiles.into_iter().collect())
}

pub(super) fn load_profile_source_watermark(
    connection: &Connection,
    profile_id: &str,
) -> Result<ProfileSourceWatermark> {
    let (visible_visit_count, max_visit_id, max_url_last_visit_ms) = connection.query_row(
        "SELECT COUNT(*),
                COALESCE(MAX(archive.visits.id), 0),
                COALESCE(MAX(archive.urls.last_visit_ms), 0)
         FROM archive.visits
         JOIN archive.urls ON archive.urls.id = archive.visits.url_id
         JOIN archive.source_profiles
           ON archive.source_profiles.id = archive.visits.source_profile_id
         WHERE archive.visits.reverted_at IS NULL
           AND archive.source_profiles.profile_key = ?1",
        [profile_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?.max(0),
                row.get::<_, i64>(1)?.max(0),
                row.get::<_, i64>(2)?.max(0),
            ))
        },
    )?;
    let visible_search_term_count = connection
        .query_row(
            "SELECT COUNT(*)
             FROM archive.search_terms
             WHERE reverted_at IS NULL AND profile_id = ?1",
            [profile_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        .max(0);
    Ok(ProfileSourceWatermark {
        visible_visit_count,
        max_visit_id,
        max_url_last_visit_ms,
        visible_search_term_count,
    })
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

fn load_site_dictionary_signature(connection: &Connection) -> Result<String> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS site_dictionary_overrides (
           id                INTEGER PRIMARY KEY AUTOINCREMENT,
           target_kind       TEXT NOT NULL,
           target_value      TEXT NOT NULL,
           domain_category   TEXT,
           page_category     TEXT,
           interaction_kind  TEXT,
           display_name      TEXT,
           search_engine     TEXT,
           is_noisy          INTEGER NOT NULL DEFAULT 0,
           note              TEXT,
           created_at        TEXT NOT NULL,
           updated_at        TEXT NOT NULL,
           UNIQUE(target_kind, target_value)
         )",
    )?;
    connection
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(updated_at), 'none') FROM site_dictionary_overrides",
            [],
            |row| Ok((row.get::<_, i64>(0)?.max(0), row.get::<_, String>(1)?)),
        )
        .map(|(count, updated_at)| format!("overrides:{count}:{updated_at}"))
        .context("loading site dictionary signature")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_checkpoint_helpers_cover_names_versions_and_scoped_deletes() {
        let connection = Connection::open_in_memory().expect("memory");
        ensure_core_intelligence_stage_checkpoint_schema(&connection).expect("schema");

        assert_eq!(stage_name(RebuildMode::StructuralRebuild), "structural-rebuild");
        assert_eq!(stage_name(RebuildMode::FullRebuild), "full-rebuild");
        assert!(
            stage_version(&connection, RebuildMode::VisitDerive)
                .expect("visit version")
                .starts_with("visit-derived-facts-v2:")
        );
        assert_eq!(
            stage_version(&connection, RebuildMode::StructuralRebuild).expect("structural version"),
            "structural-rebuild-v2"
        );
        assert_eq!(
            stage_version(&connection, RebuildMode::FullRebuild).expect("full version"),
            "full-rebuild-v2"
        );

        save_stage_checkpoint(
            &connection,
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
            &connection,
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

        delete_stage_checkpoints(&connection, Some("profile-a")).expect("delete profile a");
        assert!(
            load_stage_checkpoint(&connection, "profile-a", RebuildMode::DailyRollup)
                .expect("load profile a")
                .is_none()
        );
        assert!(
            load_stage_checkpoint(&connection, "profile-b", RebuildMode::DailyRollup)
                .expect("load profile b")
                .is_some()
        );
        delete_stage_checkpoints(&connection, None).expect("delete all");
        assert!(
            load_stage_checkpoint(&connection, "profile-b", RebuildMode::DailyRollup)
                .expect("load profile b after delete")
                .is_none()
        );
    }
}
