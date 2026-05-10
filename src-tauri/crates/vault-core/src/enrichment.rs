//! Shared readable-content enrichment helpers.
//!
//! Core Intelligence and optional AI both depend on `visit_content_enrichments`
//! as a rebuildable evidence plane. Keeping the shared schema, enrichment
//! execution path, and lookup helpers here prevents the legacy `insights`
//! module from remaining the canonical owner of readable-text evidence after
//! the hard cutover.

#[allow(dead_code)]
#[path = "enrichment_site_adapters.rs"]
mod site_adapters;

use crate::{
    config::ProjectPaths,
    intelligence_blobs::{load_readable_text_blob, store_readable_text_blob},
    intelligence_runtime::{
        ClaimedEnrichmentJob, claim_enrichment_job_by_id, intelligence_job_stop_requested,
        mark_intelligence_job_failed, mark_intelligence_job_succeeded,
        mark_running_intelligence_job_cancelled,
    },
    models::{READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID},
    utils::{now_rfc3339, url_domain},
};
use anyhow::Result;
use rusqlite::{Connection, Row, params};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use std::collections::HashMap;

use self::site_adapters::{SiteAdapterResult, adapt_site_content};

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
#[allow(dead_code)]
const ENRICH_TEXT_LIMIT: usize = 12_000;
#[allow(dead_code)]
const SNIPPET_LIMIT: usize = 3;
const ENRICHMENT_PIPELINE_VERSION: &str = "insights-v2";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StoredEnrichment {
    pub fetch_status: String,
    pub fetched_at: String,
    pub readable_title: Option<String>,
    pub readable_text: Option<String>,
    pub snippet_json: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EnrichmentResult {
    pub status: String,
    pub final_url: Option<String>,
    pub language: Option<String>,
    pub readable_title: Option<String>,
    pub readable_text: Option<String>,
    pub snippets: Vec<String>,
    pub extraction: Value,
}

pub(crate) fn ensure_visit_content_enrichment_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL)?;
    Ok(())
}

/// Claims and executes one persisted enrichment job from the intelligence queue.
pub fn execute_enrichment_job_by_id(
    paths: &ProjectPaths,
    connection: &Connection,
    job_id: i64,
) -> Result<bool> {
    let Some(job) = claim_enrichment_job_by_id(connection, job_id)? else {
        return Ok(false);
    };
    if let Some(done) = finish_if_enrichment_cancelled(connection, job.id)? {
        return Ok(done);
    }

    let enrichment = match job.plugin_id.as_str() {
        TITLE_NORMALIZATION_PLUGIN_ID => {
            title_normalization_enrichment(&job.payload.url, job.payload.title.as_deref())
        }
        READABLE_CONTENT_PLUGIN_ID => EnrichmentResult {
            status: "deferred".to_string(),
            final_url: Some(job.payload.url.clone()),
            extraction: json!({
                "reason": "Readable content fetching is tracked for PathKeep v0.3.0 and remains disabled in PathKeep v0.2.0."
            }),
            ..EnrichmentResult::default()
        },
        _ => {
            fail_unknown_enrichment_plugin(connection, job.id, &job.plugin_id)?;
            return Ok(true);
        }
    };
    finish_claimed_enrichment_job(paths, connection, &job, &enrichment)
}

fn finish_claimed_enrichment_job(
    paths: &ProjectPaths,
    connection: &Connection,
    job: &ClaimedEnrichmentJob,
    enrichment: &EnrichmentResult,
) -> Result<bool> {
    if cancel_running_enrichment_job_if_requested(connection, job.id)? {
        return Ok(true);
    }
    store_enrichment(paths, connection, job.payload.history_id, &job.plugin_id, enrichment)?;
    let artifact = json!({
        "status": enrichment.status,
        "snippetCount": enrichment.snippets.len(),
        "textLength": enrichment
            .readable_text
            .as_ref()
            .map(|value| value.len())
            .unwrap_or(0),
        "attempt": job.attempt,
    });
    if enrichment_is_terminal_failure(enrichment) {
        if !mark_intelligence_job_failed(
            connection,
            job.id,
            &enrichment_failure_message(enrichment),
        )? {
            let _ =
                mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        }
    } else if !mark_intelligence_job_succeeded(connection, job.id, &artifact)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
    }

    Ok(true)
}

fn cancel_running_enrichment_job_if_requested(
    connection: &Connection,
    job_id: i64,
) -> Result<bool> {
    if intelligence_job_stop_requested(connection, job_id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job_id, "cancelled from UI");
        return Ok(true);
    }
    Ok(false)
}

fn fail_unknown_enrichment_plugin(
    connection: &Connection,
    job_id: i64,
    plugin_id: &str,
) -> Result<()> {
    if !mark_intelligence_job_failed(
        connection,
        job_id,
        &format!("Unknown enrichment plugin {plugin_id}"),
    )? {
        let _ = mark_running_intelligence_job_cancelled(connection, job_id, "cancelled from UI");
    }
    Ok(())
}

fn finish_if_enrichment_cancelled(connection: &Connection, job_id: i64) -> Result<Option<bool>> {
    if cancel_running_enrichment_job_if_requested(connection, job_id)? {
        return Ok(Some(true));
    }
    Ok(None)
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

pub(crate) fn title_normalization_enrichment(url: &str, title: Option<&str>) -> EnrichmentResult {
    let readable_title = title
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_title_from_url(url));
    let status = if readable_title.is_some() { "success" } else { "empty" };
    let snippets = readable_title.clone().into_iter().collect::<Vec<_>>();
    EnrichmentResult {
        status: status.to_string(),
        final_url: Some(url.to_string()),
        language: None,
        readable_title,
        readable_text: None,
        snippets,
        extraction: json!({
            "strategy": if title.is_some() { "browser-title" } else { "url-fallback" },
        }),
    }
}

pub(crate) fn enrichment_is_terminal_failure(enrichment: &EnrichmentResult) -> bool {
    matches!(enrichment.status.as_str(), "fetch-error" | "decode-error" | "unsupported-content")
}

pub(crate) fn enrichment_failure_message(enrichment: &EnrichmentResult) -> String {
    match enrichment.status.as_str() {
        "unsupported-content" => {
            let content_type = enrichment
                .extraction
                .get("contentType")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("non-HTML response");
            format!("Skipped non-readable content ({content_type}).")
        }
        "fetch-error" => enrichment
            .extraction
            .get("error")
            .and_then(Value::as_str)
            .map(|error| format!("Could not fetch the page again. {error}"))
            .unwrap_or_else(|| "Could not fetch the page again.".to_string()),
        "decode-error" => {
            let content_type = enrichment
                .extraction
                .get("contentType")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let error = enrichment
                .extraction
                .get("error")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            match (content_type, error) {
                (Some(content_type), Some(error)) => {
                    format!("Could not decode the response body ({content_type}). {error}")
                }
                (Some(content_type), None) => {
                    format!("Could not decode the response body ({content_type}).")
                }
                (None, Some(error)) => format!("Could not decode the response body. {error}"),
                (None, None) => "Could not decode the response body.".to_string(),
            }
        }
        _ => enrichment
            .extraction
            .get("error")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("Enrichment failed with status {}", enrichment.status)),
    }
}

pub(crate) fn store_enrichment(
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    content_source: &str,
    enrichment: &EnrichmentResult,
) -> Result<()> {
    let stored_blob = store_readable_text_blob(paths, enrichment.readable_text.as_deref())?;
    connection.execute(
        "INSERT OR REPLACE INTO visit_content_enrichments
         (history_id, content_source, fetch_status, fetched_at, final_url, language, readable_title,
          readable_text_blob_path, readable_text_bytes, text_hash, snippet_json, extraction_json, pipeline_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            history_id,
            content_source,
            enrichment.status,
            now_rfc3339(),
            enrichment.final_url,
            enrichment.language,
            enrichment.readable_title,
            stored_blob.as_ref().map(|blob| blob.relative_path.as_str()),
            stored_blob.as_ref().map(|blob| blob.byte_len as i64).unwrap_or(0),
            stored_blob.as_ref().map(|blob| blob.content_hash.as_str()),
            serde_json::to_string(&enrichment.snippets)?,
            serde_json::to_string(&enrichment.extraction)?,
            ENRICHMENT_PIPELINE_VERSION,
        ],
    )?;
    Ok(())
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
    let mut enrichments =
        load_best_enrichment_map_by_history_ids(paths, connection, &[history_id])?;
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

fn normalized_title_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let last_segment = parsed
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .map(percent_decode)?;
    let candidate = normalize_whitespace(&last_segment.replace(['-', '_'], " "));
    if candidate.is_empty() { None } else { Some(candidate) }
}

#[allow(dead_code)]
fn build_enrichment_result_from_html(
    url: &str,
    final_url: Option<String>,
    content_type: String,
    text: &str,
) -> EnrichmentResult {
    let document = Html::parse_document(text);
    let title_selector = Selector::parse("title").expect("selector");
    let html_selector = Selector::parse("html").expect("selector");
    let body_selector = Selector::parse("main, article, body").expect("selector");
    let block_selector =
        Selector::parse("p, li, h1, h2, h3, h4, h5, h6, pre, code").expect("selector");

    let readable_title = document
        .select(&title_selector)
        .next()
        .map(|node| normalize_whitespace(&node.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty());
    let language = document
        .select(&html_selector)
        .next()
        .and_then(|node| node.value().attr("lang"))
        .map(ToString::to_string);

    let mut blocks = Vec::new();
    if let Some(root) = document.select(&body_selector).next() {
        for node in root.select(&block_selector) {
            let value = normalize_whitespace(&node.text().collect::<Vec<_>>().join(" "));
            if !value.is_empty() {
                blocks.push(value);
            }
        }
        if blocks.is_empty() {
            let fallback = normalize_whitespace(&root.text().collect::<Vec<_>>().join(" "));
            if !fallback.is_empty() {
                blocks.push(fallback);
            }
        }
    }

    let snippets = blocks.iter().take(SNIPPET_LIMIT).cloned().collect::<Vec<_>>();
    let readable_text = truncate_text(&blocks.join("\n\n"), ENRICH_TEXT_LIMIT);
    let generic_result = EnrichmentResult {
        status: if readable_text.is_empty() { "empty".to_string() } else { "success".to_string() },
        final_url,
        language,
        readable_title,
        readable_text: (!readable_text.is_empty()).then_some(readable_text),
        snippets: snippets.clone(),
        extraction: json!({
            "contentType": content_type,
            "snippetCount": snippets.len(),
            "textLength": snippets.iter().map(|value| value.len()).sum::<usize>(),
        }),
    };

    if let Some(adapter) = adapt_site_content(url, &document) {
        return merge_site_adapter_result(generic_result, &content_type, adapter);
    }

    generic_result
}

#[allow(dead_code)]
fn merge_site_adapter_result(
    generic_result: EnrichmentResult,
    content_type: &str,
    adapter: SiteAdapterResult,
) -> EnrichmentResult {
    let adapter_id = adapter.adapter_id;
    let metadata = adapter.metadata;
    let readable_title = adapter.readable_title.or_else(|| generic_result.readable_title.clone());
    let readable_text = adapter
        .readable_text
        .map(|value| truncate_text(&value, ENRICH_TEXT_LIMIT))
        .or_else(|| generic_result.readable_text.clone());
    let snippets = if adapter.snippets.is_empty() {
        generic_result.snippets.clone()
    } else {
        adapter.snippets.into_iter().take(SNIPPET_LIMIT).collect()
    };

    EnrichmentResult {
        status: if readable_text.as_deref().is_some_and(|value| !value.is_empty()) {
            "success".to_string()
        } else {
            generic_result.status
        },
        final_url: generic_result.final_url,
        language: generic_result.language,
        readable_title,
        readable_text,
        snippets: snippets.clone(),
        extraction: json!({
            "contentType": content_type,
            "snippetCount": snippets.len(),
            "textLength": snippets.iter().map(|value| value.len()).sum::<usize>(),
            "siteAdapter": {
                "id": adapter_id,
                "metadata": metadata,
            },
        }),
    }
}

fn percent_decode(input: &str) -> String {
    let mut output = String::new();
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => output.push(' '),
            b'%' if index + 2 < bytes.len() => {
                let hi = (bytes[index + 1] as char).to_digit(16);
                let lo = (bytes[index + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    output.push(char::from_u32(hi * 16 + lo).unwrap_or('%'));
                    index += 2;
                } else {
                    output.push('%');
                }
            }
            value => output.push(value as char),
        }
        index += 1;
    }
    output
}

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
        } else {
            output.push(ch);
            last_was_space = false;
        }
    }
    output.trim().to_string()
}

#[allow(dead_code)]
fn truncate_text(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }
    input.chars().take(limit).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{
        EnrichmentResult, SiteAdapterResult, StoredEnrichment, build_embedding_content_from_parts,
        build_enrichment_result_from_html, enrichment_failure_message,
        enrichment_is_terminal_failure, ensure_visit_content_enrichment_schema,
        execute_enrichment_job_by_id, fail_unknown_enrichment_plugin,
        finish_claimed_enrichment_job, finish_if_enrichment_cancelled,
        load_best_enrichment_map_by_history_ids, merge_site_adapter_result,
        preferred_embedding_content, store_enrichment, title_normalization_enrichment,
    };
    use crate::{
        config::{ensure_paths, project_paths_with_root},
        intelligence_blobs::store_readable_text_blob,
        intelligence_runtime::{
            ClaimedEnrichmentJob, ENRICHMENT_JOB_TYPE, EnrichmentJobPayload,
            built_in_enrichment_plugin, enqueue_enrichment_job, ensure_intelligence_runtime_schema,
        },
        models::{READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID},
        utils::now_rfc3339,
    };
    use rusqlite::{Connection, Row, params};
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

    #[test]
    fn title_normalization_failure_messages_and_storage_cover_enrichment_edges() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("ensure paths");
        let connection = Connection::open_in_memory().expect("sqlite");
        ensure_visit_content_enrichment_schema(&connection).expect("schema");

        let fallback = title_normalization_enrichment(
            "https://example.com/docs/pathkeep%20archive+replay",
            None,
        );
        assert_eq!(fallback.status, "success");
        assert_eq!(fallback.readable_title.as_deref(), Some("pathkeep archive replay"));
        assert_eq!(fallback.extraction["strategy"], "url-fallback");

        let invalid_escape =
            title_normalization_enrichment("https://example.com/docs/bad%zztitle", None);
        assert_eq!(invalid_escape.readable_title.as_deref(), Some("bad%zztitle"));

        let empty = title_normalization_enrichment("notaurl", Some(" \n\t "));
        assert_eq!(empty.status, "empty");
        assert!(empty.snippets.is_empty());

        let unsupported = EnrichmentResult {
            status: "unsupported-content".to_string(),
            extraction: serde_json::json!({ "contentType": "image/png" }),
            ..EnrichmentResult::default()
        };
        assert!(enrichment_is_terminal_failure(&unsupported));
        assert!(enrichment_failure_message(&unsupported).contains("image/png"));

        let fetch_error = EnrichmentResult {
            status: "fetch-error".to_string(),
            extraction: serde_json::json!({ "error": "connection refused" }),
            ..EnrichmentResult::default()
        };
        assert!(enrichment_failure_message(&fetch_error).contains("connection refused"));

        let decode_with_context = EnrichmentResult {
            status: "decode-error".to_string(),
            extraction: serde_json::json!({
                "contentType": "application/gzip",
                "error": "invalid gzip header"
            }),
            ..EnrichmentResult::default()
        };
        assert!(enrichment_failure_message(&decode_with_context).contains("application/gzip"));
        let decode_without_context = EnrichmentResult {
            status: "decode-error".to_string(),
            extraction: serde_json::json!({}),
            ..EnrichmentResult::default()
        };
        assert_eq!(
            enrichment_failure_message(&decode_without_context),
            "Could not decode the response body."
        );
        let decode_content_only = EnrichmentResult {
            status: "decode-error".to_string(),
            extraction: serde_json::json!({ "contentType": "text/html" }),
            ..EnrichmentResult::default()
        };
        assert_eq!(
            enrichment_failure_message(&decode_content_only),
            "Could not decode the response body (text/html)."
        );
        let decode_error_only = EnrichmentResult {
            status: "decode-error".to_string(),
            extraction: serde_json::json!({ "error": "bad bytes" }),
            ..EnrichmentResult::default()
        };
        assert_eq!(
            enrichment_failure_message(&decode_error_only),
            "Could not decode the response body. bad bytes"
        );
        let unknown = EnrichmentResult {
            status: "empty".to_string(),
            extraction: serde_json::json!({ "error": "no readable text" }),
            ..EnrichmentResult::default()
        };
        assert_eq!(enrichment_failure_message(&unknown), "no readable text");
        let unknown_without_error = EnrichmentResult {
            status: "empty".to_string(),
            extraction: serde_json::json!({}),
            ..EnrichmentResult::default()
        };
        assert_eq!(
            enrichment_failure_message(&unknown_without_error),
            "Enrichment failed with status empty"
        );

        store_enrichment(
            &paths,
            &connection,
            55,
            "capture",
            &EnrichmentResult {
                status: "success".to_string(),
                final_url: Some("https://example.com/docs".to_string()),
                language: Some("en".to_string()),
                readable_title: Some("Readable Docs".to_string()),
                readable_text: Some("Long readable body".to_string()),
                snippets: vec!["Long readable body".to_string()],
                extraction: serde_json::json!({ "strategy": "test" }),
            },
        )
        .expect("store enrichment");

        let content = preferred_embedding_content(
            &paths,
            &connection,
            55,
            "chrome:Default",
            "https://example.com/docs",
            Some("Browser Docs"),
            "2026-04-17T00:00:00Z",
        )
        .expect("preferred content");
        assert!(content.contains("Readable title: Readable Docs"));
        assert!(content.contains("Readable text:\nLong readable body"));
    }

    #[test]
    fn html_enrichment_result_covers_fallback_adapter_and_truncation_paths() {
        let fallback = build_enrichment_result_from_html(
            "https://example.com/fallback",
            Some("https://example.com/fallback".to_string()),
            "text/html".to_string(),
            "<html><body> Body fallback only </body></html>",
        );
        assert_eq!(fallback.status, "success");
        assert_eq!(fallback.snippets, vec!["Body fallback only".to_string()]);

        let long_text = "x".repeat(12_010);
        let truncated = build_enrichment_result_from_html(
            "https://example.com/long",
            Some("https://example.com/long".to_string()),
            "text/html".to_string(),
            &format!("<html><body><main><p>{long_text}</p></main></body></html>"),
        );
        assert_eq!(truncated.readable_text.as_deref().map(str::len), Some(12_000));

        let youtube = build_enrichment_result_from_html(
            "https://www.youtube.com/watch?v=abc123",
            Some("https://www.youtube.com/watch?v=abc123".to_string()),
            "text/html".to_string(),
            r#"
            <html lang="en">
              <head>
                <title>Generic title</title>
                <script type="application/ld+json">
                {
                  "@context": "https://schema.org",
                  "@type": "VideoObject",
                  "name": "PathKeep demo",
                  "description": "A walkthrough of local history backup.",
                  "author": { "name": "PathKeep" },
                  "duration": "PT1H2M3S",
                  "uploadDate": "2026-04-17"
                }
                </script>
              </head>
              <body><main><p>Generic body text.</p></main></body>
            </html>
            "#,
        );
        assert_eq!(youtube.status, "success");
        assert_eq!(youtube.readable_title.as_deref(), Some("PathKeep demo"));
        assert!(
            youtube
                .readable_text
                .as_deref()
                .is_some_and(|text| text.contains("Video: PathKeep demo"))
        );
        assert_eq!(youtube.extraction["siteAdapter"]["id"], "youtube-video");
        assert_eq!(youtube.extraction["siteAdapter"]["metadata"]["videoId"], "abc123");
    }

    #[test]
    fn site_adapter_merge_falls_back_to_generic_snippets_and_status() {
        let merged = merge_site_adapter_result(
            EnrichmentResult {
                status: "empty".to_string(),
                final_url: Some("https://video.example/watch".to_string()),
                language: Some("en".to_string()),
                readable_title: Some("Generic title".to_string()),
                readable_text: None,
                snippets: vec!["Generic snippet".to_string()],
                extraction: serde_json::json!({}),
            },
            "text/html",
            SiteAdapterResult {
                adapter_id: "fixture-video",
                readable_title: None,
                readable_text: None,
                snippets: Vec::new(),
                metadata: serde_json::json!({ "fixture": true }),
            },
        );

        assert_eq!(merged.status, "empty");
        assert_eq!(merged.readable_title.as_deref(), Some("Generic title"));
        assert_eq!(merged.snippets, vec!["Generic snippet".to_string()]);
        assert_eq!(merged.extraction["siteAdapter"]["id"], "fixture-video");
    }

    #[test]
    fn execute_enrichment_job_by_id_runs_outside_legacy_insights_module() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("ensure paths");
        let connection = Connection::open(&paths.intelligence_database_path).expect("sqlite");
        ensure_visit_content_enrichment_schema(&connection).expect("schema");
        ensure_intelligence_runtime_schema(&connection).expect("runtime schema");

        let plugin =
            built_in_enrichment_plugin(TITLE_NORMALIZATION_PLUGIN_ID).expect("title plugin");
        enqueue_enrichment_job(
            &connection,
            41,
            plugin,
            &EnrichmentJobPayload {
                history_id: 7,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/docs/pathkeep-archive-replay".to_string(),
                title: None,
            },
        )
        .expect("enqueue enrichment job");
        let job_id = connection
            .query_row(
                "SELECT id FROM intelligence_jobs WHERE dedupe_key = 'title-normalization:7'",
                [],
                |row: &Row<'_>| row.get::<_, i64>(0),
            )
            .expect("job id");

        assert!(execute_enrichment_job_by_id(&paths, &connection, job_id).expect("execute job"));

        let job_state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [job_id],
                |row: &Row<'_>| row.get::<_, String>(0),
            )
            .expect("job state");
        assert_eq!(job_state, "succeeded");

        let stored_title = connection
            .query_row(
                "SELECT readable_title
                 FROM visit_content_enrichments
                 WHERE history_id = 7 AND content_source = ?1",
                [TITLE_NORMALIZATION_PLUGIN_ID],
                |row: &Row<'_>| row.get::<_, Option<String>>(0),
            )
            .expect("stored title");
        assert_eq!(stored_title.as_deref(), Some("pathkeep archive replay"));
    }

    #[test]
    fn execute_enrichment_job_by_id_covers_cancellation_and_failure_edges() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("ensure paths");
        let connection = Connection::open(&paths.intelligence_database_path).expect("sqlite");
        ensure_visit_content_enrichment_schema(&connection).expect("schema");
        ensure_intelligence_runtime_schema(&connection).expect("runtime schema");

        assert!(
            !execute_enrichment_job_by_id(&paths, &connection, 404)
                .expect("missing enrichment job is a no-op")
        );

        let title_plugin =
            built_in_enrichment_plugin(TITLE_NORMALIZATION_PLUGIN_ID).expect("title plugin");
        let readable_plugin =
            built_in_enrichment_plugin(READABLE_CONTENT_PLUGIN_ID).expect("readable plugin");
        let enqueue = |history_id: i64, plugin_id: &str, title: Option<&str>| {
            let plugin = if plugin_id == TITLE_NORMALIZATION_PLUGIN_ID {
                title_plugin
            } else {
                readable_plugin
            };
            enqueue_enrichment_job(
                &connection,
                41,
                plugin,
                &EnrichmentJobPayload {
                    history_id,
                    profile_id: "chrome:Default".to_string(),
                    url: if plugin_id == READABLE_CONTENT_PLUGIN_ID {
                        "not a url".to_string()
                    } else {
                        format!("https://example.com/docs/{history_id}")
                    },
                    title: title.map(ToString::to_string),
                },
            )
            .expect("enqueue enrichment job");
            connection
                .query_row(
                    "SELECT id FROM intelligence_jobs WHERE dedupe_key = ?1",
                    [format!("{plugin_id}:{history_id}")],
                    |row: &Row<'_>| row.get::<_, i64>(0),
                )
                .expect("job id")
        };

        let pre_cancelled_id = enqueue(101, TITLE_NORMALIZATION_PLUGIN_ID, Some("Pre cancel"));
        connection
            .execute(
                &format!(
                    "CREATE TRIGGER enrichment_pre_cancel
                     AFTER UPDATE OF state ON intelligence_jobs
                     WHEN NEW.id = {pre_cancelled_id} AND NEW.state = 'running'
                     BEGIN
                       UPDATE intelligence_jobs SET stop_requested = 1 WHERE id = NEW.id;
                     END"
                ),
                [],
            )
            .expect("create targeted pre-cancel trigger");
        assert!(
            execute_enrichment_job_by_id(&paths, &connection, pre_cancelled_id)
                .expect("pre-cancelled job handled")
        );
        connection
            .execute("DROP TRIGGER enrichment_pre_cancel", [])
            .expect("drop pre-cancel trigger");
        let pre_cancelled_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [pre_cancelled_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("pre-cancelled state");
        assert_eq!(pre_cancelled_state, "cancelled");

        let unknown_id = {
            let now = now_rfc3339();
            let payload = serde_json::to_string(&EnrichmentJobPayload {
                history_id: 102,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/unknown".to_string(),
                title: Some("Unknown Plugin".to_string()),
            })
            .expect("payload json");
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
                     (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key,
                      payload_json, artifact_json, created_at, scheduled_at, updated_at)
                     VALUES (?1, 'unknown-enrichment-plugin', 41, 'queued', 10, 0,
                      'unknown-enrichment-plugin:102', ?2, '{}', ?3, ?3, ?3)",
                    params![ENRICHMENT_JOB_TYPE, payload, now],
                )
                .expect("insert unknown plugin job");
            connection.last_insert_rowid()
        };
        assert!(
            execute_enrichment_job_by_id(&paths, &connection, unknown_id)
                .expect("unknown plugin handled")
        );
        let unknown_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [unknown_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("unknown state");
        assert_eq!(unknown_state, "failed");

        let post_success_cancelled_id =
            enqueue(103, TITLE_NORMALIZATION_PLUGIN_ID, Some("Post success cancel"));
        connection
            .execute(
                &format!(
                    "CREATE TRIGGER enrichment_post_success_cancel
                     AFTER INSERT ON visit_content_enrichments
                     WHEN NEW.history_id = 103
                     BEGIN
                       UPDATE intelligence_jobs SET stop_requested = 1
                       WHERE id = {post_success_cancelled_id};
                     END"
                ),
                [],
            )
            .expect("create success cancel trigger");
        assert!(
            execute_enrichment_job_by_id(&paths, &connection, post_success_cancelled_id)
                .expect("post-success cancelled job handled")
        );
        connection
            .execute("DROP TRIGGER enrichment_post_success_cancel", [])
            .expect("drop success cancel trigger");
        let post_success_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [post_success_cancelled_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("post success state");
        assert_eq!(post_success_state, "cancelled");

        let terminal_failure_id = enqueue(104, READABLE_CONTENT_PLUGIN_ID, None);
        assert!(
            execute_enrichment_job_by_id(&paths, &connection, terminal_failure_id)
                .expect("deferred readable-content job handled")
        );
        let terminal_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [terminal_failure_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("deferred readable-content state");
        assert_eq!(terminal_state, "succeeded");

        let terminal_cancelled_id = enqueue(105, READABLE_CONTENT_PLUGIN_ID, None);
        connection
            .execute(
                &format!(
                    "CREATE TRIGGER enrichment_terminal_cancel
                     AFTER INSERT ON visit_content_enrichments
                     WHEN NEW.history_id = 105
                     BEGIN
                       UPDATE intelligence_jobs SET stop_requested = 1
                       WHERE id = {terminal_cancelled_id};
                     END"
                ),
                [],
            )
            .expect("create terminal cancel trigger");
        assert!(
            execute_enrichment_job_by_id(&paths, &connection, terminal_cancelled_id)
                .expect("terminal cancelled job handled")
        );
        connection
            .execute("DROP TRIGGER enrichment_terminal_cancel", [])
            .expect("drop terminal cancel trigger");
        let terminal_cancelled_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [terminal_cancelled_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("terminal cancelled state");
        assert_eq!(terminal_cancelled_state, "cancelled");

        let terminal_failure_cancelled_id = enqueue(107, READABLE_CONTENT_PLUGIN_ID, None);
        connection
            .execute(
                "UPDATE intelligence_jobs SET state = 'running', attempt = 1 WHERE id = ?1",
                [terminal_failure_cancelled_id],
            )
            .expect("mark terminal failure job running");
        connection
            .execute(
                &format!(
                    "CREATE TRIGGER enrichment_terminal_failure_cancel
                     AFTER INSERT ON visit_content_enrichments
                     WHEN NEW.history_id = 107
                     BEGIN
                       UPDATE intelligence_jobs SET stop_requested = 1
                       WHERE id = {terminal_failure_cancelled_id};
                     END"
                ),
                [],
            )
            .expect("create terminal failure cancel trigger");
        let terminal_failure_job = ClaimedEnrichmentJob {
            id: terminal_failure_cancelled_id,
            plugin_id: READABLE_CONTENT_PLUGIN_ID.to_string(),
            attempt: 1,
            payload: EnrichmentJobPayload {
                history_id: 107,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/fail".to_string(),
                title: None,
            },
        };
        assert!(
            finish_claimed_enrichment_job(
                &paths,
                &connection,
                &terminal_failure_job,
                &EnrichmentResult {
                    status: "fetch-error".to_string(),
                    extraction: serde_json::json!({ "error": "offline" }),
                    ..EnrichmentResult::default()
                },
            )
            .expect("terminal failure cancelled job handled")
        );
        connection
            .execute("DROP TRIGGER enrichment_terminal_failure_cancel", [])
            .expect("drop terminal failure cancel trigger");
        let terminal_failure_cancelled_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [terminal_failure_cancelled_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("terminal failure cancelled state");
        assert_eq!(terminal_failure_cancelled_state, "cancelled");

        let post_plugin_stopped_id = {
            let now = now_rfc3339();
            let payload = EnrichmentJobPayload {
                history_id: 108,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/post-plugin-stopped".to_string(),
                title: Some("Post plugin stopped".to_string()),
            };
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
	                     (job_type, plugin_id, run_id, state, priority, attempt, stop_requested,
	                      dedupe_key, payload_json, artifact_json, created_at, scheduled_at, updated_at)
	                     VALUES (?1, ?2, 41, 'running', 10, 1, 1,
	                      'title-normalization:108', ?3, '{}', ?4, ?4, ?4)",
                    params![
                        ENRICHMENT_JOB_TYPE,
                        TITLE_NORMALIZATION_PLUGIN_ID,
                        serde_json::to_string(&payload).expect("payload json"),
                        now
                    ],
                )
                .expect("insert post-plugin stopped job");
            (
                connection.last_insert_rowid(),
                ClaimedEnrichmentJob {
                    id: connection.last_insert_rowid(),
                    plugin_id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
                    attempt: 1,
                    payload,
                },
            )
        };
        assert!(
            finish_claimed_enrichment_job(
                &paths,
                &connection,
                &post_plugin_stopped_id.1,
                &EnrichmentResult::default(),
            )
            .expect("finish stopped post-plugin job")
        );
        let post_plugin_stopped_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [post_plugin_stopped_id.0],
                |row: &Row<'_>| row.get(0),
            )
            .expect("post-plugin stopped state");
        assert_eq!(post_plugin_stopped_state, "cancelled");

        let stopped_running_id = {
            let now = now_rfc3339();
            let payload = serde_json::to_string(&EnrichmentJobPayload {
                history_id: 109,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/stopped".to_string(),
                title: Some("Stopped".to_string()),
            })
            .expect("payload json");
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
	                     (job_type, plugin_id, run_id, state, priority, attempt, stop_requested,
	                      dedupe_key, payload_json, artifact_json, created_at, scheduled_at, updated_at)
	                     VALUES (?1, ?2, 41, 'running', 10, 1, 1,
	                      'title-normalization:109', ?3, '{}', ?4, ?4, ?4)",
                    params![ENRICHMENT_JOB_TYPE, TITLE_NORMALIZATION_PLUGIN_ID, payload, now],
                )
                .expect("insert stopped running job");
            connection.last_insert_rowid()
        };
        assert_eq!(
            finish_if_enrichment_cancelled(&connection, stopped_running_id)
                .expect("finish stopped running job"),
            Some(true)
        );

        let unknown_stopped_id = {
            let now = now_rfc3339();
            let payload = serde_json::to_string(&EnrichmentJobPayload {
                history_id: 110,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/unknown-stopped".to_string(),
                title: Some("Unknown stopped".to_string()),
            })
            .expect("payload json");
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
	                     (job_type, plugin_id, run_id, state, priority, attempt, stop_requested,
	                      dedupe_key, payload_json, artifact_json, created_at, scheduled_at, updated_at)
	                     VALUES (?1, 'unknown-enrichment-plugin', 41, 'running', 10, 1, 1,
	                      'unknown-enrichment-plugin:110', ?2, '{}', ?3, ?3, ?3)",
                    params![ENRICHMENT_JOB_TYPE, payload, now],
                )
                .expect("insert stopped unknown plugin job");
            connection.last_insert_rowid()
        };
        fail_unknown_enrichment_plugin(
            &connection,
            unknown_stopped_id,
            "unknown-enrichment-plugin",
        )
        .expect("cancel stopped unknown plugin job");
        let unknown_stopped_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [unknown_stopped_id],
                |row: &Row<'_>| row.get(0),
            )
            .expect("unknown stopped state");
        assert_eq!(unknown_stopped_state, "cancelled");
    }
}
