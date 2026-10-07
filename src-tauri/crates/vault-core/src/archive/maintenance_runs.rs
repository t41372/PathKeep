//! Failure modes: a ledger points at an unwritten artifact, provenance is lost, or maintenance counts include hidden facts. Tests: maintenance and restore suites.
//! Responsibilities: preserve SQLCipher maintenance ledger operations until area 5.
//! Not responsible for: native read paths.
use super::artifacts::snapshot_artifact_bytes_and_checksum;
use super::run_support::write_manifest_artifact;
use super::*;
/// Parent-manifest pointer used to keep the manifest chain append-only.
#[derive(Debug, Clone)]
pub(crate) struct ManifestRow {
    pub(crate) id: i64,
}

/// Persists one structured manifest payload and links it into the manifest chain.
///
/// Maintenance flows use this when they need manifest semantics identical to a
/// backup run but already have the final JSON payload assembled.
pub(crate) fn persist_structured_manifest(
    connection: &Connection,
    paths: &ProjectPaths,
    run_id: i64,
    finished_at: &str,
    payload: &Value,
) -> Result<(String, PathBuf)> {
    let manifest_json = serde_json::to_string_pretty(payload)?;
    let manifest_hash = crate::utils::sha256_hex(manifest_json.as_bytes());
    let row_counts = archive_row_counts(connection)?;
    let parent_manifest = latest_manifest_row(connection)?;
    let manifest_path =
        write_manifest_artifact(paths, run_id, finished_at, &manifest_hash, &manifest_json)?;
    persist_manifest_row(
        connection,
        run_id,
        parent_manifest.as_ref(),
        &manifest_hash,
        &manifest_path,
        finished_at,
        &row_counts,
    )?;
    Ok((manifest_hash, manifest_path))
}

/// Loads the most recent manifest row so the next run can chain to it.
pub(crate) fn latest_manifest_row(connection: &Connection) -> Result<Option<ManifestRow>> {
    connection
        .query_row(
            "SELECT id
             FROM manifests
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| Ok(ManifestRow { id: row.get(0)? }),
        )
        .optional()
        .map_err(Into::into)
}

/// Records one manifest row after its artifact has been written successfully.
///
/// Keeping the file write ahead of the SQL row avoids ledger entries that point
/// at audit artifacts which were never created.
pub(crate) fn persist_manifest_row(
    connection: &Connection,
    run_id: i64,
    parent: Option<&ManifestRow>,
    content_hash: &str,
    file_path: &Path,
    created_at: &str,
    row_counts: &Value,
) -> Result<()> {
    connection.execute(
        "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            run_id,
            parent.map(|manifest| manifest.id),
            content_hash,
            serde_json::to_string(row_counts)?,
            created_at,
            file_path.display().to_string(),
        ],
    )?;
    Ok(())
}
/// Counts the row families surfaced in manifest metadata.
///
/// These counts deliberately exclude reverted visits/downloads because the
/// manifest is meant to describe the user-visible archive state.
pub(crate) fn archive_row_counts(connection: &Connection) -> Result<Value> {
    let urls: i64 = connection.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0))?;
    let visits: i64 = connection.query_row(
        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let downloads: i64 = connection.query_row(
        "SELECT COUNT(*) FROM downloads WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let manifests: i64 =
        connection.query_row("SELECT COUNT(*) FROM manifests", [], |row| row.get(0))?;
    let snapshots: i64 =
        connection.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
    Ok(json!({
        "urls": urls,
        "visits": visits,
        "downloads": downloads,
        "manifests": manifests,
        "snapshots": snapshots,
    }))
}

/// Merges run-local counters with the current visible archive totals.
///
/// This helper avoids a second ad-hoc shape for stats JSON, which keeps Audit,
/// Dashboard, and takeout batch summaries aligned.
pub(crate) fn stats_with_archive_totals(connection: &Connection, stats: Value) -> Result<Value> {
    let totals = count_visible_archive_totals(connection)?;
    let mut object = match stats {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    object.insert("totalProfiles".to_string(), json!(totals.total_profiles));
    object.insert("totalUrls".to_string(), json!(totals.total_urls));
    object.insert("totalVisits".to_string(), json!(totals.total_visits));
    object.insert("totalDownloads".to_string(), json!(totals.total_downloads));
    object.insert("earliestVisitAt".to_string(), json!(totals.earliest_visit_at));
    object.insert("latestVisitAt".to_string(), json!(totals.latest_visit_at));
    Ok(Value::Object(object))
}

/// Counts visible archive totals (plus coverage bounds) without including
/// reverted history rows.
///
/// Read models use this as a fallback when cached totals are missing or
/// stale. The bounds query is paid here once per backup-class run so the
/// dashboard never has to scan every visit on render.
pub(crate) fn count_visible_archive_totals(
    connection: &Connection,
) -> Result<ArchiveVisibleTotals> {
    let total_profiles: i64 = connection.query_row(
        "SELECT COUNT(*) FROM source_profiles WHERE enabled = 1",
        [],
        |row| row.get(0),
    )?;
    let total_urls: i64 =
        connection.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0))?;
    let total_visits: i64 = connection.query_row(
        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let total_downloads: i64 = connection.query_row(
        "SELECT COUNT(*) FROM downloads WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let bounds: (Option<String>, Option<String>) = connection
        .query_row(
            "SELECT MIN(visit_time_iso), MAX(visit_time_iso)
             FROM visits
             WHERE reverted_at IS NULL",
            [],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?
        .unwrap_or_default();

    Ok(ArchiveVisibleTotals {
        total_profiles: total_profiles.max(0) as usize,
        total_urls: total_urls.max(0) as usize,
        total_visits: total_visits.max(0) as usize,
        total_downloads: total_downloads.max(0) as usize,
        earliest_visit_at: bounds.0,
        latest_visit_at: bounds.1,
    })
}

/// Records that a later run reused or consumed an existing snapshot artifact.
///
/// Restore and rekey flows use this instead of duplicating files when they only
/// need an audit reference to an existing artifact.
pub(crate) fn record_snapshot_reference(
    connection: &Connection,
    run_id: i64,
    path: &Path,
    reason: &str,
    created_at: &str,
) -> Result<()> {
    let (file_size, checksum) = snapshot_artifact_bytes_and_checksum(path)?;
    connection.execute(
        "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            run_id,
            path.display().to_string(),
            file_size as i64,
            checksum,
            reason,
            created_at,
        ],
    )?;
    Ok(())
}
