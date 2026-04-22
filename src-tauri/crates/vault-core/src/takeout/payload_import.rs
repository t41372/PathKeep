//! Takeout payload import helpers.
//!
//! ## Responsibilities
//! - Parse one recognized Takeout payload for archive import without allocating
//!   inspection-only preview rows.
//! - Write canonical URL/visit rows for that payload and stage the matching
//!   source-evidence plan.
//! - Persist deferred source-evidence plans and synthetic Takeout profile
//!   provenance after canonical archive commits.
//!
//! ## Not responsible for
//! - Driving the outer import run loop or emitting shell progress events.
//! - Building inspection previews or quarantining unsupported files.
//! - Updating import-batch review read models after import completes.
//!
//! ## Dependencies
//! - Canonical archive and source-evidence helpers from `crate::archive`.
//! - Inspection-side parse helpers from `super::inspect`.
//!
//! ## Performance notes
//! - Import intentionally bypasses preview-row allocation and consumes parser
//!   reports directly so large payloads do not carry an extra visit-sized
//!   allocation just for import execution.
//! - Source-evidence plans take ownership of the parsed history instead of
//!   cloning it, keeping per-file memory bounded to one parser report.

use super::{inspect, *};

/// Imports one recognized payload into canonical archive rows plus source-evidence plans.
pub(super) fn import_supported_payload(
    archive: &Transaction<'_>,
    run_id: i64,
    batch_id: i64,
    source_profile_id: i64,
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<ImportedPayload> {
    let report = inspect::parse_payload_report(source_path, kind, bytes)?;
    let mut stats =
        ImportStats { skipped_items: report.skipped_missing_visit_time, ..ImportStats::default() };
    let mut url_id_map = std::collections::BTreeMap::new();

    for url in &report.history.urls {
        let payload_hash = sha256_hex(
            serde_json::to_string(url)
                .context("serializing Takeout URL for payload hash")?
                .as_bytes(),
        );
        let url_id = archive.query_row(
            "INSERT INTO urls (
               url,
               title,
               visit_count,
               typed_count,
               first_visit_ms,
               first_visit_iso,
               last_visit_ms,
               last_visit_iso,
               source_profile_id,
               created_by_run_id,
               source_url_id,
               hidden,
               payload_hash,
               recorded_at
             )
             VALUES (?1, ?2, 1, 0, ?3, ?4, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
               url = excluded.url,
               title = excluded.title,
               hidden = excluded.hidden,
               last_visit_ms = CASE
                 WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_ms
                 ELSE urls.last_visit_ms
               END,
               last_visit_iso = CASE
                 WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_iso
                 ELSE urls.last_visit_iso
               END,
               payload_hash = excluded.payload_hash,
               recorded_at = excluded.recorded_at
             RETURNING id",
            params![
                url.url,
                url.title,
                url.last_visit_ms,
                url.last_visit_iso,
                source_profile_id,
                run_id,
                url.source_url_id.to_string(),
                i64::from(url.hidden),
                payload_hash,
                now_rfc3339(),
            ],
            |row| row.get::<_, i64>(0),
        )?;
        url_id_map.insert(url.source_url_id, url_id);
    }

    for visit in &report.history.visits {
        let Some(&url_id) = url_id_map.get(&visit.source_url_id) else {
            continue;
        };
        let payload_hash = sha256_hex(
            serde_json::to_string(visit)
                .context("serializing Takeout visit for payload hash")?
                .as_bytes(),
        );
        let inserted = archive.execute(
            "INSERT OR IGNORE INTO visits (
               url_id,
               source_visit_id,
               visit_time_ms,
               visit_time_iso,
               transition_type,
               visit_duration_ms,
               source_profile_id,
               created_by_run_id,
               from_visit,
               is_known_to_sync,
               visited_link_id,
               external_referrer_url,
               app_id,
               event_fingerprint,
               payload_hash,
               recorded_at,
               import_batch_id
             )
             VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, ?6, NULL, 0, NULL, ?7, 'takeout', ?8, ?9, ?10, ?11)",
            params![
                url_id,
                visit.source_visit_id.to_string(),
                visit.visit_time_ms,
                visit.visit_time_iso,
                source_profile_id,
                run_id,
                source_path,
                visit_event_fingerprint(
                    "takeout",
                    &visit.url,
                    visit.visit_time_ms,
                    visit.title.as_deref(),
                    None,
                    Some("takeout"),
                ),
                payload_hash,
                now_rfc3339(),
                batch_id,
            ],
        )?;

        if inserted > 0 {
            stats.imported_items += 1;
        } else {
            stats.duplicate_items += 1;
        }
    }

    Ok(ImportedPayload {
        stats,
        record_count: report.record_count,
        recognized_file: TakeoutFileReport {
            path: source_path.to_string(),
            kind: kind.to_string(),
            status: if report.skipped_missing_visit_time > 0 {
                "previewed-with-skips".to_string()
            } else {
                "previewed".to_string()
            },
            records: report.record_count,
        },
        source_evidence_plan: build_takeout_source_evidence_plan(
            source_profile_id,
            run_id,
            source_path,
            report,
        )?,
    })
}

/// Builds the cold source-evidence plan corresponding to one imported payload.
fn build_takeout_source_evidence_plan(
    source_profile_id: i64,
    run_id: i64,
    source_path: &str,
    report: TakeoutPayloadReport,
) -> Result<TakeoutSourceEvidencePlan> {
    let mut history = report.history;
    let observation_json = serde_json::to_string(&history.schema_observation)?;
    Ok(TakeoutSourceEvidencePlan {
        source_batch: SourceBatchInput {
            source_profile_id,
            run_id: Some(run_id),
            source_kind: "takeout".to_string(),
            browser_version: None,
            schema_version_text: Some(report.kind.clone()),
            schema_version_int: None,
            schema_fingerprint: sha256_hex(observation_json.as_bytes()),
            capability_snapshot: history.capability_snapshot.clone(),
            coverage_stats_json: coverage_stats_json(&history),
            artifact_refs_json: Some(
                json!({
                    "sourcePath": source_path,
                    "payloadKind": report.kind,
                })
                .to_string(),
            ),
            notes_json: Some(serde_json::to_string(&history.warnings)?),
        },
        schema_observation: history.schema_observation.clone(),
        source_evidence_payload: take_source_evidence_payload(&mut history),
    })
}

/// Persists all deferred source-evidence writes after canonical import success.
pub(super) fn persist_takeout_source_evidence_plans(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    profile_id: &str,
    plans: &[TakeoutSourceEvidencePlan],
) -> Result<()> {
    if plans.is_empty() {
        return Ok(());
    }

    let archive = open_archive_connection(paths, config, key)?;
    let mut source_evidence = open_source_evidence_connection(paths, config, key)?;
    let transaction = source_evidence.transaction()?;
    let mut last_source_batch_id = None;

    for plan in plans {
        let source_batch_id = upsert_source_batch(&transaction, &plan.source_batch)?;
        record_schema_observation(
            &transaction,
            source_batch_id,
            "takeout-payload",
            &plan.schema_observation,
        )?;
        persist_source_evidence(
            &transaction,
            source_batch_id,
            plan.source_batch.source_profile_id,
            &plan.source_evidence_payload,
        )?;
        last_source_batch_id = Some(source_batch_id);
    }

    transaction.commit()?;
    if let Some(source_batch_id) = last_source_batch_id {
        touch_takeout_source_batch_watermark(&archive, profile_id, source_batch_id)?;
    }
    Ok(())
}

/// Advances the Takeout watermark so later provenance reads can find the newest source batch.
fn touch_takeout_source_batch_watermark(
    archive: &Connection,
    profile_id: &str,
    source_batch_id: i64,
) -> Result<()> {
    archive.execute(
        "INSERT INTO profile_watermarks (
           profile_id,
           last_visit_id,
           last_url_last_visit_time,
           last_download_id,
           last_favicon_last_updated,
           last_checkpoint_at,
           last_schema_hash,
           last_source_batch_id,
           updated_at
         )
         VALUES (?1, 0, 0, 0, 0, NULL, NULL, ?2, ?3)
         ON CONFLICT(profile_id) DO UPDATE SET
           last_source_batch_id = excluded.last_source_batch_id,
           updated_at = excluded.updated_at",
        params![profile_id, source_batch_id, now_rfc3339()],
    )?;
    Ok(())
}

/// Ensures the synthetic Takeout profile exists before import writes start.
pub(super) fn upsert_takeout_profile(
    archive: &Transaction<'_>,
    profile_id: &str,
    source: &Path,
) -> Result<i64> {
    archive.execute(
        "INSERT INTO source_profiles (
           browser_kind,
           browser_family,
           browser_product,
           browser_version,
           profile_name,
           profile_path,
           discovered_at,
           enabled,
           profile_key,
           user_name,
           updated_at
         )
         VALUES ('takeout', 'takeout', 'takeout', 'takeout', ?1, ?2, ?3, 1, ?4, NULL, ?3)
         ON CONFLICT(profile_key) DO UPDATE SET
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           browser_family = excluded.browser_family,
           browser_product = excluded.browser_product,
           browser_version = excluded.browser_version,
           updated_at = excluded.updated_at,
           enabled = 1",
        params![
            "Imported browser history".to_string(),
            source.display().to_string(),
            now_rfc3339(),
            profile_id,
        ],
    )?;
    archive
        .query_row("SELECT id FROM source_profiles WHERE profile_key = ?1", [profile_id], |row| {
            row.get(0)
        })
        .map_err(Into::into)
}
