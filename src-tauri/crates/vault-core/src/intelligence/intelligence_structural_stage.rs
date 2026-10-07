//! Responsibilities: preserve structural reference queries and fallback classification tests.
//! Not responsible for: production generation writes or schema installation.
//! Failure modes: dirty ranges omit a linked session/trail, source scores drift,
//! or a regressed watermark is treated as an append.
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

/// Streams persisted trails in `first_visit_ms` order so source-effectiveness
/// rebuilds can stay bounded.
fn load_profile_trail_batch(
    connection: &Session<'_>,
    profile_id: &str,
    after: Option<&TrailBatchCursor>,
    limit: usize,
) -> Result<Vec<TrailRecord>> {
    let mut statement = connection.prepare(
        "SELECT trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count,
                visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE profile_id = ?1
           AND (
             ?2 IS NULL
             OR first_visit_ms > ?2
             OR (first_visit_ms = ?2 AND trail_id > ?3)
           )
         ORDER BY first_visit_ms ASC, trail_id ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                after.map(|cursor| cursor.first_visit_ms),
                after.map(|cursor| cursor.trail_id.as_str()),
                limit.max(1) as i64
            ],
            |row| {
                let queries_json: String = row.get(12)?;
                Ok(TrailRecord {
                    trail_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    session_id: row.get(2)?,
                    initial_query: row.get(3)?,
                    search_engine: row.get(4)?,
                    reformulation_count: row.get(5)?,
                    visit_count: row.get(6)?,
                    landing_url: row.get(7)?,
                    landing_domain: row.get(8)?,
                    first_visit_ms: row.get(9)?,
                    last_visit_ms: row.get(10)?,
                    max_depth: row.get(11)?,
                    queries: serde_json::from_str(&queries_json).unwrap_or_default(),
                    members: Vec::new(),
                })
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Test helper that reloads all persisted trails for one profile.
#[cfg(test)]
pub(super) fn load_profile_trails(
    connection: &Session<'_>,
    profile_id: &str,
) -> Result<Vec<TrailRecord>> {
    let mut cursor = None::<TrailBatchCursor>;
    let mut trails = Vec::new();
    loop {
        let batch = load_profile_trail_batch(
            connection,
            profile_id,
            cursor.as_ref(),
            super::STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        cursor = batch.last().map(|trail| TrailBatchCursor {
            first_visit_ms: trail.first_visit_ms,
            trail_id: trail.trail_id.clone(),
        });
        trails.extend(batch);
    }
    Ok(trails)
}

/// Rebuilds source-effectiveness rows by streaming already-persisted trails
/// instead of reloading full visit history.
pub(super) fn build_source_effectiveness_from_database(
    connection: &Session<'_>,
    profile_id: &str,
    refind_pages: &[super::RefindPageRecord],
) -> Result<Vec<super::SourceEffectivenessRecord>> {
    let mut landing_counts = std::collections::HashMap::<String, i64>::new();
    let mut trail_counts = std::collections::HashMap::<String, i64>::new();
    let mut first_seen = std::collections::HashMap::<String, i64>::new();
    let mut last_seen = std::collections::HashMap::<String, i64>::new();
    let mut cursor = None::<TrailBatchCursor>;

    loop {
        let batch = load_profile_trail_batch(
            connection,
            profile_id,
            cursor.as_ref(),
            super::STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for trail in &batch {
            if let Some(domain) = &trail.landing_domain {
                *landing_counts.entry(domain.clone()).or_default() += 1;
                *trail_counts.entry(domain.clone()).or_default() += 1;
                first_seen
                    .entry(domain.clone())
                    .and_modify(|value| *value = (*value).min(trail.first_visit_ms))
                    .or_insert(trail.first_visit_ms);
                last_seen
                    .entry(domain.clone())
                    .and_modify(|value| *value = (*value).max(trail.last_visit_ms))
                    .or_insert(trail.last_visit_ms);
            }
        }
        cursor = batch.last().map(|trail| TrailBatchCursor {
            first_visit_ms: trail.first_visit_ms,
            trail_id: trail.trail_id.clone(),
        });
    }

    let reference_counts = refind_pages.iter().fold(
        std::collections::HashMap::<String, i64>::new(),
        |mut acc, page| {
            *acc.entry(page.registrable_domain.clone()).or_default() += 1;
            acc
        },
    );
    let domains = landing_counts
        .keys()
        .chain(reference_counts.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();

    Ok(domains
        .into_iter()
        .map(|domain| {
            let stable_landing_count = *landing_counts.get(&domain).unwrap_or(&0);
            let reference_count = *reference_counts.get(&domain).unwrap_or(&0);
            let trail_count = *trail_counts.get(&domain).unwrap_or(&0);
            let source_role = if reference_count >= stable_landing_count && reference_count > 0 {
                "reference"
            } else {
                "landing"
            };
            let effectiveness_score = (stable_landing_count as f32 * 2.0)
                + (reference_count as f32 * 1.5)
                + (trail_count as f32 * 0.5);
            let evidence_json = serde_json::json!({
                "stableLandingCount": stable_landing_count,
                "referenceCount": reference_count,
                "trailCount": trail_count
            })
            .to_string();
            super::SourceEffectivenessRecord {
                profile_id: profile_id.to_string(),
                registrable_domain: domain.clone(),
                source_role: source_role.to_string(),
                trail_count,
                stable_landing_count,
                effectiveness_score,
                evidence_json,
                first_seen_ms: *first_seen.get(&domain).unwrap_or(&0),
                last_seen_ms: *last_seen.get(&domain).unwrap_or(&0),
            }
        })
        .collect())
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
