//! Shared readable-content enrichment helpers.
//!
//! Core Intelligence and optional AI both depend on `visit_content_enrichments`
//! as a rebuildable evidence plane. Keeping the shared schema, enrichment
//! execution path, and lookup helpers here prevents the legacy `insights`
//! module from remaining the canonical owner of readable-text evidence after
//! the hard cutover.

#[path = "insights/site_adapters.rs"]
mod site_adapters;

use crate::{
    config::ProjectPaths,
    intelligence_blobs::{load_readable_text_blob, store_readable_text_blob},
    intelligence_runtime::{
        claim_enrichment_job_by_id, intelligence_job_stop_requested, mark_intelligence_job_failed,
        mark_intelligence_job_succeeded, mark_running_intelligence_job_cancelled,
    },
    models::{READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID},
    utils::{now_rfc3339, url_domain},
};
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use rusqlite::{Connection, Row, params};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use std::collections::HashMap;

use self::site_adapters::adapt_site_content;

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
const ENRICH_TEXT_LIMIT: usize = 12_000;
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
    if intelligence_job_stop_requested(connection, job.id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        return Ok(true);
    }

    let enrichment = match job.plugin_id.as_str() {
        TITLE_NORMALIZATION_PLUGIN_ID => {
            title_normalization_enrichment(&job.payload.url, job.payload.title.as_deref())
        }
        READABLE_CONTENT_PLUGIN_ID => {
            let client = build_refetch_client()?;
            refetch_visit_content(&client, &job.payload.url)
        }
        _ => {
            if !mark_intelligence_job_failed(
                connection,
                job.id,
                &format!("Unknown enrichment plugin {}", job.plugin_id),
            )? {
                let _ = mark_running_intelligence_job_cancelled(
                    connection,
                    job.id,
                    "cancelled from UI",
                );
            }
            return Ok(true);
        }
    };
    if intelligence_job_stop_requested(connection, job.id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        return Ok(true);
    }

    store_enrichment(paths, connection, job.payload.history_id, &job.plugin_id, &enrichment)?;
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
    if enrichment_is_terminal_failure(&enrichment) {
        if !mark_intelligence_job_failed(
            connection,
            job.id,
            &enrichment_failure_message(&enrichment),
        )? {
            let _ =
                mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
        }
    } else if !mark_intelligence_job_succeeded(connection, job.id, &artifact)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job.id, "cancelled from UI");
    }

    Ok(true)
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

fn build_refetch_client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(4))
        .user_agent("PathKeep Enrichment/0.1")
        .build()
        .context("building content refetch client")
}

fn refetch_visit_content(client: &Client, url: &str) -> EnrichmentResult {
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(error) => {
            return EnrichmentResult {
                status: "fetch-error".to_string(),
                extraction: json!({ "error": error.to_string() }),
                ..EnrichmentResult::default()
            };
        }
    };
    let final_url = Some(response.url().to_string());
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let text = match response.text() {
        Ok(text) => text,
        Err(error) => {
            return EnrichmentResult {
                status: "decode-error".to_string(),
                final_url,
                extraction: json!({ "error": error.to_string(), "contentType": content_type }),
                ..EnrichmentResult::default()
            };
        }
    };
    if !content_type.is_empty() && !content_type.contains("html") {
        return EnrichmentResult {
            status: "unsupported-content".to_string(),
            final_url,
            extraction: json!({ "contentType": content_type }),
            ..EnrichmentResult::default()
        };
    }
    let document = Html::parse_document(&text);
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
        let readable_title =
            adapter.readable_title.or_else(|| generic_result.readable_title.clone());
        let readable_text = adapter
            .readable_text
            .map(|value| truncate_text(&value, ENRICH_TEXT_LIMIT))
            .or_else(|| generic_result.readable_text.clone());
        let snippets = if adapter.snippets.is_empty() {
            generic_result.snippets.clone()
        } else {
            adapter.snippets.into_iter().take(SNIPPET_LIMIT).collect()
        };

        return EnrichmentResult {
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
                    "id": adapter.adapter_id,
                    "metadata": adapter.metadata,
                },
            }),
        };
    }

    generic_result
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

fn truncate_text(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }
    input.chars().take(limit).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{
        StoredEnrichment, build_embedding_content_from_parts,
        ensure_visit_content_enrichment_schema, execute_enrichment_job_by_id,
        load_best_enrichment_map_by_history_ids,
    };
    use crate::{
        config::{ensure_paths, project_paths_with_root},
        intelligence_blobs::store_readable_text_blob,
        intelligence_runtime::{
            EnrichmentJobPayload, built_in_enrichment_plugin, enqueue_enrichment_job,
            ensure_intelligence_runtime_schema,
        },
        models::TITLE_NORMALIZATION_PLUGIN_ID,
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
}
