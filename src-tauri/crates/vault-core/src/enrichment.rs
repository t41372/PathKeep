//! Shared readable-content enrichment helpers.
//!
//! Core Intelligence and optional AI both depend on `visit_content_enrichments`
//! as a rebuildable evidence plane. Keeping the shared schema and lookup
//! helpers here prevents the legacy `insights` module from remaining the
//! canonical owner of readable-text evidence after the hard cutover.

use crate::{config::ProjectPaths, intelligence_blobs::load_readable_text_blob, utils::url_domain};
use anyhow::Result;
use rusqlite::{Connection, Row};
use std::collections::HashMap;

pub(crate) const VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS visit_content_enrichments (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  history_id          INTEGER NOT NULL,
  content_source      TEXT NOT NULL,
  fetch_status        TEXT NOT NULL,
  fetched_at          TEXT NOT NULL,
  final_url           TEXT,
  language            TEXT,
  readable_title      TEXT,
  readable_text_blob_path TEXT,
  readable_text_bytes INTEGER NOT NULL DEFAULT 0,
  text_hash           TEXT,
  snippet_json        TEXT NOT NULL,
  extraction_json     TEXT NOT NULL,
  pipeline_version    TEXT NOT NULL,
  UNIQUE(history_id, content_source)
);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_history_id
  ON visit_content_enrichments(history_id);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_status
  ON visit_content_enrichments(fetch_status, fetched_at);
"#;

const SQLITE_BATCH_SIZE: usize = 400;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StoredEnrichment {
    pub fetch_status: String,
    pub fetched_at: String,
    pub readable_title: Option<String>,
    pub readable_text: Option<String>,
    pub snippet_json: String,
}

pub(crate) fn ensure_visit_content_enrichment_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL)?;
    Ok(())
}

pub(crate) fn load_best_enrichment_map_by_history_ids(
    paths: &ProjectPaths,
    connection: &Connection,
    history_ids: &[i64],
) -> Result<HashMap<i64, StoredEnrichment>> {
    if history_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT history_id, fetch_status, fetched_at, readable_title, readable_text_blob_path, snippet_json
             FROM visit_content_enrichments
             WHERE history_id IN ({placeholders})
             ORDER BY
               history_id ASC,
               CASE fetch_status WHEN 'success' THEN 0 WHEN 'empty' THEN 1 ELSE 2 END,
               CASE content_source
                 WHEN 'capture' THEN 0
                 WHEN 'readable-content-refetch' THEN 1
                 WHEN 'title-normalization' THEN 2
                 ELSE 3
               END,
               fetched_at DESC"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = chunk.iter().map(|history_id| history_id as &dyn rusqlite::ToSql);
        let rows = statement.query_map(rusqlite::params_from_iter(params), |row: &Row<'_>| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        for row in rows {
            let (history_id, fetch_status, fetched_at, readable_title, blob_path, snippet_json) =
                row?;
            map.entry(history_id).or_insert(StoredEnrichment {
                fetch_status,
                fetched_at,
                readable_title,
                readable_text: load_readable_text_blob(paths, blob_path.as_deref())?,
                snippet_json,
            });
        }
    }

    Ok(map)
}

/// Builds the canonical text blob fed into semantic indexing from visit/enrichment parts.
pub(crate) fn build_embedding_content_from_parts(
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
    readable_title: Option<&str>,
    readable_text: Option<&str>,
) -> String {
    let title = title.unwrap_or("(untitled)");
    let mut content = format!(
        "Profile: {profile_id}\nVisited at: {visited_at}\nURL: {url}\nDomain: {}\nTitle: {title}",
        url_domain(url)
    );
    if let Some(readable_title) = readable_title.filter(|value| !value.trim().is_empty()) {
        content.push_str(&format!("\nReadable title: {}", readable_title.trim()));
    }
    if let Some(readable_text) = readable_text.filter(|value| !value.trim().is_empty()) {
        content.push_str("\nReadable text:\n");
        content.push_str(readable_text.trim());
    }
    content
}

#[allow(dead_code)]
/// Chooses the strongest available text payload for semantic indexing or assistant context.
pub(crate) fn preferred_embedding_content(
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
) -> Result<String> {
    let mut enrichments = load_best_enrichment_map_by_history_ids(paths, connection, &[history_id])?;
    let enrichment = enrichments.remove(&history_id);
    Ok(build_embedding_content_from_parts(
        profile_id,
        url,
        title,
        visited_at,
        enrichment.as_ref().and_then(|value| value.readable_title.as_deref()),
        enrichment.as_ref().and_then(|value| value.readable_text.as_deref()),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        StoredEnrichment, build_embedding_content_from_parts,
        ensure_visit_content_enrichment_schema, load_best_enrichment_map_by_history_ids,
    };
    use crate::{config::project_paths_with_root, intelligence_blobs::store_readable_text_blob};
    use rusqlite::{Connection, params};
    use tempfile::tempdir;

    #[test]
    fn build_embedding_content_includes_only_non_empty_enrichment_fields() {
        let enriched = build_embedding_content_from_parts(
            "chrome:Default",
            "https://example.com/docs/intro",
            Some("Intro"),
            "2026-04-17T00:00:00Z",
            Some("Readable Intro"),
            Some("First paragraph"),
        );
        assert!(enriched.contains("Readable title: Readable Intro"));
        assert!(enriched.contains("Readable text:\nFirst paragraph"));

        let without_enrichment = build_embedding_content_from_parts(
            "chrome:Default",
            "https://example.com/docs/intro",
            None,
            "2026-04-17T00:00:00Z",
            Some("   "),
            Some("  "),
        );
        assert!(without_enrichment.contains("Title: (untitled)"));
        assert!(!without_enrichment.contains("Readable title:"));
        assert!(!without_enrichment.contains("Readable text:"));
    }

    #[test]
    fn load_best_enrichment_map_prefers_success_then_source_priority() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let connection = Connection::open_in_memory().expect("sqlite");
        ensure_visit_content_enrichment_schema(&connection).expect("schema");

        let capture_blob =
            store_readable_text_blob(&paths, Some("capture body")).expect("capture blob");
        let title_blob = store_readable_text_blob(&paths, Some("title body")).expect("title blob");
        connection
            .execute(
                "INSERT INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, readable_title,
                  readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
                  extraction_json, pipeline_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '{}', 'test-v1')",
                params![
                    1_i64,
                    "title-normalization",
                    "success",
                    "2026-04-17T10:00:00Z",
                    "Title Fallback",
                    title_blob.as_ref().map(|blob| blob.relative_path.as_str()),
                    title_blob.as_ref().map(|blob| blob.byte_len as i64).unwrap_or(0),
                    title_blob.as_ref().map(|blob| blob.content_hash.as_str()),
                    "[\"title snippet\"]",
                ],
            )
            .expect("title row");
        connection
            .execute(
                "INSERT INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, readable_title,
                  readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
                  extraction_json, pipeline_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '{}', 'test-v1')",
                params![
                    1_i64,
                    "capture",
                    "success",
                    "2026-04-16T09:00:00Z",
                    "Captured Title",
                    capture_blob.as_ref().map(|blob| blob.relative_path.as_str()),
                    capture_blob.as_ref().map(|blob| blob.byte_len as i64).unwrap_or(0),
                    capture_blob.as_ref().map(|blob| blob.content_hash.as_str()),
                    "[\"capture snippet\"]",
                ],
            )
            .expect("capture row");
        connection
            .execute(
                "INSERT INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, readable_title,
                  readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
                  extraction_json, pipeline_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, NULL, ?6, '{}', 'test-v1')",
                params![
                    2_i64,
                    "readable-content-refetch",
                    "fetch-error",
                    "2026-04-17T12:00:00Z",
                    Option::<String>::None,
                    "[\"failed snippet\"]",
                ],
            )
            .expect("failed row");
        connection
            .execute(
                "INSERT INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, readable_title,
                  readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
                  extraction_json, pipeline_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, NULL, ?6, '{}', 'test-v1')",
                params![
                    2_i64,
                    "title-normalization",
                    "empty",
                    "2026-04-17T13:00:00Z",
                    Option::<String>::None,
                    "[\"empty snippet\"]",
                ],
            )
            .expect("empty row");

        let loaded =
            load_best_enrichment_map_by_history_ids(&paths, &connection, &[1, 2]).expect("load");
        assert_eq!(
            loaded.get(&1),
            Some(&StoredEnrichment {
                fetch_status: "success".to_string(),
                fetched_at: "2026-04-16T09:00:00Z".to_string(),
                readable_title: Some("Captured Title".to_string()),
                readable_text: Some("capture body".to_string()),
                snippet_json: "[\"capture snippet\"]".to_string(),
            })
        );
        assert_eq!(
            loaded.get(&2),
            Some(&StoredEnrichment {
                fetch_status: "empty".to_string(),
                fetched_at: "2026-04-17T13:00:00Z".to_string(),
                readable_title: None,
                readable_text: None,
                snippet_json: "[\"empty snippet\"]".to_string(),
            })
        );
    }

    #[test]
    fn load_best_enrichment_map_skips_empty_requests() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let connection = Connection::open_in_memory().expect("sqlite");
        ensure_visit_content_enrichment_schema(&connection).expect("schema");

        let loaded =
            load_best_enrichment_map_by_history_ids(&paths, &connection, &[]).expect("load");
        assert!(loaded.is_empty());
    }
}
