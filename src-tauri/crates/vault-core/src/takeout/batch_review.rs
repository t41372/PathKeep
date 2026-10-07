//! Responsibilities: read bounded batch previews and repair their audit artifacts.
//! Not responsible for: parsing or changing canonical visit visibility.
//! Failure modes: hidden visits in previews, missing batch provenance or audit files.
//! Tests: takeout::tests and batch_review_tests in plaintext and encrypted stores.
//! Failure modes: fixed caps hide old imports or tied preview rows; active imports vanish.

use super::*;

/// A complete keyset page of imports, unfinished operations first.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchPage {
    pub items: Vec<ImportBatchOverview>,
    pub next_cursor: Option<String>,
}

pub fn load_import_batches(
    vault: &crate::Vault,
    cursor: Option<String>,
    limit: usize,
) -> Result<ImportBatchPage> {
    vault.read(vault_store::Db::Archive,move |s| {
        let (active,id)=match cursor.as_deref() {
            None=>(2,i64::MAX),
            Some(raw)=>{let (active,id)=raw.split_once('|').context("invalid-cursor: import batch")?;(active.parse::<i64>()?,id.parse::<i64>()?)},
        };
        let limit=limit.max(1);
        let sql="SELECT id,source_kind,source_path,profile_id,created_at,imported_at,reverted_at,status,summary_json,audit_path,git_commit,visible_items FROM import_batches WHERE (CASE WHEN status IN ('reverting','restoring','interrupted') THEN 1 ELSE 0 END,id)<(?1,?2) ORDER BY CASE WHEN status IN ('reverting','restoring','interrupted') THEN 1 ELSE 0 END DESC,id DESC LIMIT ?3";
        let mut items=s.query(sql,vault_store::params![active,id,limit as i64+1],|r| {
            Ok(import_batch_overview_from_summary(ImportBatchOverviewRow {
                id:r.get(0)?,source_kind:r.get(1)?,source_path:r.get(2)?,profile_id:r.get(3)?,created_at:r.get(4)?,imported_at:r.get(5)?,reverted_at:r.get(6)?,status:r.get(7)?,summary_json:r.get(8)?,audit_path:r.get(9)?,git_commit:r.get(10)?,visible_items:r.get::<i64>(11)? as usize,
            }))
        })?;
        let more=items.len()>limit;items.truncate(limit);
        let next_cursor=more.then(||items.last().map(|b|format!("{}|{}",i64::from(matches!(b.status.as_str(),"reverting"|"restoring"|"interrupted")),b.id))).flatten();
        Ok(ImportBatchPage{items,next_cursor})
    })
}

/// Loads one import batch detail and recreates its audit artifact if it is missing.
pub fn preview_import_batch(vault: &crate::Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    preview_import_batch_page(vault, batch_id, None, PREVIEW_LIMIT)
}

/// Makes every visible visit in an import reachable from its review panel.
pub fn preview_import_batch_page(
    vault: &crate::Vault,
    batch_id: i64,
    cursor: Option<String>,
    limit: usize,
) -> Result<ImportBatchDetail> {
    let read_cursor = cursor.clone();
    let detail = vault.read(vault_store::Db::Archive, move |s| {
        load_import_batch_detail_page(s, batch_id, read_cursor.as_deref(), limit)
    })?;
    if matches!(detail.batch.status.as_str(), "running" | "reverting" | "restoring")
        || detail.batch.audit_path.as_deref().is_some_and(|p| Path::new(p).exists())
    {
        return Ok(detail);
    }
    super::batches::ensure_import_batch_audit_artifact(vault, batch_id, None)?;
    vault.read(vault_store::Db::Archive, move |s| {
        load_import_batch_detail_page(s, batch_id, cursor.as_deref(), limit)
    })
}

/// Loads the full review detail for one import batch.
pub(super) fn load_import_batch_detail(
    connection: &vault_store::Session<'_>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    load_import_batch_detail_page(connection, batch_id, None, PREVIEW_LIMIT)
}

fn load_import_batch_detail_page(
    connection: &vault_store::Session<'_>,
    batch_id: i64,
    cursor: Option<&str>,
    limit: usize,
) -> Result<ImportBatchDetail> {
    let (time, id) = match cursor {
        None => (i64::MAX, i64::MAX),
        Some(raw) => {
            let (time, id) = raw.split_once('|').context("invalid-cursor: batch visits")?;
            (time.parse::<i64>()?, id.parse::<i64>()?)
        }
    };
    let limit = limit.clamp(1, 1000);
    let batch = load_import_batch_record(connection, batch_id)?
        .with_context(|| format!("import batch {batch_id} was not found"))?;

    let map = |row: &vault_store::Row| -> vault_store::Result<_> {
        Ok((
            TakeoutPreviewEntry {
                source_path: batch.overview.source_path.clone(),
                url: row.get(0)?,
                title: row.get(1)?,
                visited_at: row.get(2)?,
                source_visit_id: row
                    .get::<Option<String>>(3)?
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_default(),
                status: "imported".into(),
            },
            row.get::<i64>(4)?,
            row.get::<i64>(5)?,
        ))
    };
    let mut rows = if cursor.is_some() {
        connection.query(
            &batch_visits_sql(true, true),
            vault_store::params![batch_id, time, id, limit as i64 + 1],
            map,
        )?
    } else {
        Vec::new()
    };
    if rows.len() <= limit {
        rows.extend(connection.query(
            &batch_visits_sql(false, cursor.is_some()),
            vault_store::params![batch_id, time, id, (limit + 1 - rows.len()) as i64],
            map,
        )?);
    }

    let more = rows.len() > limit;
    rows.truncate(limit);
    let preview_next_cursor =
        more.then(|| rows.last().map(|(_, time, id)| format!("{time}|{id}"))).flatten();
    let preview_entries = rows.into_iter().map(|(entry, _, _)| entry).collect();
    Ok(ImportBatchDetail {
        batch: batch.overview,
        preview_entries,
        preview_next_cursor,
        recognized_files: batch.recognized_files,
        quarantined_files: batch.quarantined_files,
        notes: batch.notes,
        note_details: batch.note_details,
        detected_locale: batch.detected_locale,
        preview_range_start: batch.preview_range_start,
        preview_range_end: batch.preview_range_end,
    })
}

fn batch_visits_sql(ties: bool, after: bool) -> String {
    let bounds = if ties {
        "visits.visit_time_ms=?2 AND visits.id<?3"
    } else if after {
        "visits.visit_time_ms<?2"
    } else {
        "visits.visit_time_ms<=?2"
    };
    format!("SELECT urls.url,urls.title,visits.visit_time_iso,visits.source_visit_id,visits.visit_time_ms,visits.id
        FROM visits INDEXED BY idx_visits_visible_batch_time_id CROSS JOIN urls ON urls.id=visits.url_id
        WHERE visits.import_batch_id IS NOT NULL AND visits.import_batch_id=?1 AND visits.reverted_at IS NULL AND {bounds}
        ORDER BY visits.visit_time_ms DESC,visits.id DESC LIMIT ?4")
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

#[cfg(test)]
#[path = "batch_review_tests.rs"]
mod tests;
