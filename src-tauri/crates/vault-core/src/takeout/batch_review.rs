//! Responsibilities: read bounded batch previews and repair their audit artifacts.
//! Not responsible for: parsing or changing canonical visit visibility.
//! Failure modes: hidden visits in previews, missing batch provenance or audit files.
//! Tests: takeout::tests. import_batches_from remains the unported SQLite snapshot reader.

use super::*;

/// Loads recent import batches, newest first.
pub fn load_import_batches(vault: &crate::Vault) -> Result<Vec<ImportBatchOverview>> {
    vault.read(vault_store::Db::Archive, |s| {
        let ids = s.query("SELECT id FROM import_batches WHERE status IN ('reverting','restoring','interrupted') OR id IN (SELECT id FROM import_batches ORDER BY id DESC LIMIT 16) ORDER BY id DESC", (), |r| {
            Ok(r.get::<i64>(0)?)
        })?;
        ids.into_iter()
            .map(|id| Ok(load_import_batch_record(s, id)?.context("batch missing")?.overview))
            .collect()
    })
}

/// Reads the 16 newest import batches from an open archive.
///
/// No `create_schema` here: `open_archive_connection` already bootstraps the schema once per
/// process, and running it again took a `BEGIN IMMEDIATE` write lock on every app snapshot, which
/// waited out the 5 s busy timeout and failed the snapshot whenever a backup was writing.
pub(crate) fn import_batches_from(connection: &Connection) -> Result<Vec<ImportBatchOverview>> {
    const LOAD_IMPORT_BATCHES_SQL: &str = "SELECT id, source_kind, source_path, profile_id, created_at, imported_at, reverted_at, status, summary_json, audit_path, git_commit, (SELECT COUNT(*) FROM visits WHERE import_batch_id = import_batches.id AND reverted_at IS NULL) AS visible_items FROM import_batches ORDER BY id DESC LIMIT 16";
    let mut statement = connection.prepare(LOAD_IMPORT_BATCHES_SQL)?;
    let rows = statement.query_map([], |row: &Row<'_>| {
        let summary_json: String = row.get(8)?;
        let visible_items: i64 = row.get(11)?;
        Ok(import_batch_overview_from_summary(ImportBatchOverviewRow {
            id: row.get(0)?,
            source_kind: row.get(1)?,
            source_path: row.get(2)?,
            profile_id: row.get(3)?,
            created_at: row.get(4)?,
            imported_at: row.get(5)?,
            reverted_at: row.get(6)?,
            status: row.get(7)?,
            summary_json,
            visible_items: visible_items.max(0) as usize,
            audit_path: row.get(9)?,
            git_commit: row.get(10)?,
        }))
    })?;

    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Loads one import batch detail and recreates its audit artifact if it is missing.
pub fn preview_import_batch(vault: &crate::Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    let detail =
        vault.read(vault_store::Db::Archive, move |s| load_import_batch_detail(s, batch_id))?;
    if matches!(detail.batch.status.as_str(), "running" | "reverting" | "restoring")
        || detail.batch.audit_path.as_deref().is_some_and(|p| Path::new(p).exists())
    {
        return Ok(detail);
    }
    super::batches::ensure_import_batch_audit_artifact(vault, batch_id, None)?;
    vault.read(vault_store::Db::Archive, move |s| load_import_batch_detail(s, batch_id))
}

/// Loads the full review detail for one import batch.
pub(super) fn load_import_batch_detail(
    connection: &vault_store::Session<'_>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let batch = load_import_batch_record(connection, batch_id)?
        .with_context(|| format!("import batch {batch_id} was not found"))?;

    const PREVIEW_IMPORT_BATCH_SQL: &str = r#"
        SELECT urls.url, urls.title, visits.visit_time_iso, visits.source_visit_id
        FROM visits
        JOIN urls ON urls.id = visits.url_id
        WHERE visits.import_batch_id = ?1
          AND visits.reverted_at IS NULL
        ORDER BY visits.visit_time_ms DESC, visits.id DESC
        LIMIT ?2
    "#;
    let preview_entries = connection.query(
        PREVIEW_IMPORT_BATCH_SQL,
        vault_store::params![batch_id, PREVIEW_LIMIT as i64],
        |row| {
            Ok(TakeoutPreviewEntry {
                source_path: batch.overview.source_path.clone(),
                url: row.get(0)?,
                title: row.get(1)?,
                visited_at: row.get(2)?,
                source_visit_id: row
                    .get::<Option<String>>(3)?
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or_default(),
                status: "imported".to_string(),
            })
        },
    )?;

    Ok(ImportBatchDetail {
        batch: batch.overview,
        preview_entries,
        recognized_files: batch.recognized_files,
        quarantined_files: batch.quarantined_files,
        notes: batch.notes,
        note_details: batch.note_details,
        detected_locale: batch.detected_locale,
        preview_range_start: batch.preview_range_start,
        preview_range_end: batch.preview_range_end,
    })
}

/// Loads the persisted batch row and reconstructs its review metadata.
pub(super) fn load_import_batch_record(
    connection: &vault_store::Session<'_>,
    batch_id: i64,
) -> Result<Option<ImportBatchRecord>> {
    let row = connection
        .query(
            "SELECT
                id,
                source_kind,
                source_path,
                profile_id,
                created_at,
                imported_at,
                reverted_at,
                status,
                summary_json,
                audit_path,
                git_commit,
                visible_items
             FROM import_batches
             WHERE id = ?1",
            [batch_id],
            |row| {
                Ok((
                    row.get::<i64>(0)?,
                    row.get::<String>(1)?,
                    row.get::<String>(2)?,
                    row.get::<String>(3)?,
                    row.get::<String>(4)?,
                    row.get::<Option<String>>(5)?,
                    row.get::<Option<String>>(6)?,
                    row.get::<String>(7)?,
                    row.get::<String>(8)?,
                    row.get::<Option<String>>(9)?,
                    row.get::<Option<String>>(10)?,
                    row.get::<i64>(11)?,
                ))
            },
        )?
        .pop();

    let Some((
        id,
        source_kind,
        source_path,
        profile_id,
        created_at,
        imported_at,
        reverted_at,
        status,
        summary_json,
        audit_path,
        git_commit,
        visible_items,
    )) = row
    else {
        return Ok(None);
    };

    let summary: Value = serde_json::from_str(&summary_json).unwrap_or_else(|_| json!({}));
    let overview = import_batch_overview_from_summary(ImportBatchOverviewRow {
        id,
        source_kind,
        source_path,
        profile_id,
        created_at,
        imported_at,
        reverted_at,
        status,
        summary_json,
        visible_items: visible_items.max(0) as usize,
        audit_path,
        git_commit,
    });
    Ok(Some(ImportBatchRecord {
        overview,
        recognized_files: serde_json::from_value(
            summary.get("recognizedFiles").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
        quarantined_files: serde_json::from_value(
            summary.get("quarantinedFiles").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
        notes: serde_json::from_value(summary.get("notes").cloned().unwrap_or_else(|| json!([])))
            .unwrap_or_default(),
        // Batches written before note code-ification only stored `notes`, so an
        // absent/unreadable `noteDetails` degrades to the string channel rather
        // than failing the whole batch read.
        note_details: serde_json::from_value(
            summary.get("noteDetails").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
        detected_locale: summary
            .get("detectedLocale")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        preview_range_start: summary
            .get("previewRangeStart")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        preview_range_end: summary
            .get("previewRangeEnd")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }))
}

/// Rebuilds the lightweight overview row consumed by recent-import surfaces.
fn import_batch_overview_from_summary(row: ImportBatchOverviewRow) -> ImportBatchOverview {
    let summary: Value = serde_json::from_str(&row.summary_json).unwrap_or_else(|_| json!({}));
    ImportBatchOverview {
        id: row.id,
        source_kind: row.source_kind,
        source_path: row.source_path,
        profile_id: row.profile_id,
        created_at: row.created_at,
        imported_at: row.imported_at,
        reverted_at: row.reverted_at,
        status: row.status,
        candidate_items: summary_count(&summary, "candidateItems"),
        imported_items: summary_count(&summary, "importedItems"),
        duplicate_items: summary_count(&summary, "duplicateItems"),
        visible_items: row.visible_items,
        audit_path: row.audit_path,
        git_commit: row.git_commit,
    }
}

/// Extracts one integer summary field from the stored summary JSON.
fn summary_count(summary: &Value, key: &str) -> usize {
    summary.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}
