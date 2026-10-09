//! Responsibilities: preserve structural reference queries and fallback classification tests.
//! Not responsible for: production generation writes or schema installation.
//! Failure modes: dirty ranges omit a linked session/trail or a regressed watermark is
//! treated as an append.
use super::incremental::watermark_regressed;
use super::*;
use anyhow::Result;
use vault_store::{OptionalRow, Session, params_owned as params};

fn structural_fallback_reason(
    force_full: bool,
    checkpoint: Option<&StageCheckpoint>,
    watermark: &ProfileSourceWatermark,
    current_version: &str,
) -> Option<(&'static str, String)> {
    if force_full {
        return Some((
            crate::models::REBUILD_FALLBACK_STRUCTURAL_MANUAL_FULL_REBUILD,
            "Manual full rebuild requested for structural entities.".to_string(),
        ));
    }
    match checkpoint {
        None => Some((
            crate::models::REBUILD_FALLBACK_STRUCTURAL_NO_CHECKPOINT,
            "No structural checkpoint was recorded for this profile yet.".to_string(),
        )),
        Some(checkpoint) if checkpoint.stage_version != current_version => Some((
            crate::models::REBUILD_FALLBACK_STRUCTURAL_RULES_CHANGED,
            "Structural rebuild logic changed since the last successful rebuild.".to_string(),
        )),
        Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => Some((
            crate::models::REBUILD_FALLBACK_STRUCTURAL_VISIBILITY_REGRESSED,
            "Archive visibility regressed or source counters moved backwards for structural entities."
                .to_string(),
        )),
        _ => None,
    }
}

/// Loads distinct dirty day keys for the structural stage from the current
/// derived visit facts.
pub(super) fn load_profile_dirty_date_keys(
    connection: &Session<'_>,
    profile_id: &str,
    start_ms: Option<i64>,
    last_processed_visit_id: Option<i64>,
) -> Result<Vec<String>> {
    connection
        .prepare(
            "SELECT DISTINCT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime')
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
               AND (?2 IS NULL OR archive.visits.visit_time_ms >= ?2)
               AND (?3 IS NULL OR visit_derived_facts.visit_id > ?3)
             ORDER BY 1 ASC",
        )?
        .query_map(params![profile_id, start_ms, last_processed_visit_id], |row| {
            Ok(row.get::< String>(0)?)
        })?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Summarizes the current structural delta so the stage can decide whether the
/// incremental replay window is still trustworthy.
pub(super) fn load_structural_delta_summary(
    connection: &Session<'_>,
    profile_id: &str,
    last_processed_visit_id: i64,
) -> Result<StructuralDeltaSummary> {
    let (delta_count, dirty_from_visit_ms) = connection.query_one(
        "SELECT COUNT(*), MIN(archive.visits.visit_time_ms)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND visit_derived_facts.visit_id > ?2",
        params![profile_id, last_processed_visit_id],
        |row| Ok((row.get::<i64>(0)?.max(0) as usize, row.get::<Option<i64>>(1)?)),
    )?;
    Ok(StructuralDeltaSummary {
        delta_count,
        dirty_from_visit_ms,
        dirty_date_keys: load_profile_dirty_date_keys(
            connection,
            profile_id,
            None,
            Some(last_processed_visit_id),
        )?,
    })
}

/// Reads the earliest currently visible visit for one profile so fallback
/// checkpoints can keep a truthful dirty-start watermark.
pub(super) fn load_profile_first_visible_visit_ms(
    connection: &Session<'_>,
    profile_id: &str,
) -> Result<Option<i64>> {
    connection
        .query_one(
            "SELECT MIN(archive.visits.visit_time_ms)
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL",
            [profile_id],
            |row| Ok(row.get::<Option<i64>>(0)?),
        )
        .optional()
        .map(|value| value.flatten())
        .map_err(Into::into)
}

/// Expands the dirty structural replay window backward to the first overlapping
/// session or trail so replayed boundaries stay deterministic.
pub(super) fn expand_structural_rebuild_start(
    connection: &Session<'_>,
    profile_id: &str,
    dirty_from_visit_ms: i64,
) -> Result<i64> {
    let session_start = connection
        .query_one(
            "SELECT MIN(first_visit_ms)
             FROM sessions
             WHERE profile_id = ?1
               AND last_visit_ms >= ?2",
            params![profile_id, dirty_from_visit_ms - super::SESSION_GAP_MS],
            |row| Ok(row.get::<Option<i64>>(0)?),
        )
        .optional()?
        .flatten();
    let trail_start = connection
        .query_one(
            "SELECT MIN(first_visit_ms)
             FROM search_trails
             WHERE profile_id = ?1
               AND last_visit_ms >= ?2",
            params![profile_id, dirty_from_visit_ms - super::TRAIL_GAP_MS],
            |row| Ok(row.get::<Option<i64>>(0)?),
        )
        .optional()?
        .flatten();
    Ok(session_start.into_iter().chain(trail_start).min().unwrap_or(dirty_from_visit_ms))
}

#[cfg(test)]
mod tests {
    use super::{ProfileSourceWatermark, StageCheckpoint, structural_fallback_reason};

    #[test]
    fn structural_fallback_reason_reports_regressed_watermarks() {
        let checkpoint = StageCheckpoint {
            stage_version: "structural-rebuild-v2".to_string(),
            source_watermark: ProfileSourceWatermark {
                visible_visit_count: 10,
                max_visit_id: 10,
                max_url_last_visit_ms: 100,
                visible_search_term_count: 4,
            },
            ..StageCheckpoint::default()
        };
        let regressed = ProfileSourceWatermark {
            visible_visit_count: 9,
            max_visit_id: 10,
            max_url_last_visit_ms: 100,
            visible_search_term_count: 4,
        };

        let (code, reason) = structural_fallback_reason(
            false,
            Some(&checkpoint),
            &regressed,
            "structural-rebuild-v2",
        )
        .expect("regression reason");

        assert!(reason.contains("regressed"));
        assert_eq!(code, crate::models::REBUILD_FALLBACK_STRUCTURAL_VISIBILITY_REGRESSED);
        assert!(
            structural_fallback_reason(
                false,
                Some(&checkpoint),
                &checkpoint.source_watermark,
                "structural-rebuild-v2",
            )
            .is_none()
        );
    }
}
