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

// W-ENRICH-1 site-content enrichment plane (doc 06): the extractor framework, the per-host rate
// limiter, and the content-fetch job runner (the single egress chokepoint). `enrichment.rs` owns the
// `enrichment/` directory, so these resolve to `enrichment/{extractors,rate_limit,content_fetch}.rs`.
pub(crate) mod content_fetch;
pub mod content_fetch_api;
pub(crate) mod extractors;
pub(crate) mod rate_limit;

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
  -- W-ENRICH-1 (migration 015 / intelligence-plane v8): the extractor schema
  -- version (bump → bounded refetch of just this source's rows) and the capped
  -- inline summary that feeds BOTH the dedup content_hash (05 §1) and the FTS5
  -- `enrichment_text` mirror without reading the (possibly large) blob.
  extractor_version   INTEGER,
  enrichment_summary  TEXT,
  -- W-ENRICH-1 negative-cache cadence: the ISO timestamp before which a failed
  -- fetch must not be retried (mirrors og_images' refetch_after) and the http
  -- status of the last fetch, kept honest for the detail panel + diagnostics.
  refetch_after       TEXT,
  http_status         INTEGER,
  UNIQUE(history_id, content_source)
);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_history_id
  ON visit_content_enrichments(history_id);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_status
  ON visit_content_enrichments(fetch_status, fetched_at);
"#;

/// Adds the W-ENRICH-1 columns to a pre-existing `visit_content_enrichments` table.
///
/// Run by the intelligence-plane migration ledger (`intelligence_schema.rs` v8) so an archive that
/// already created the table at the baseline shape gains `extractor_version`/`enrichment_summary`/
/// `refetch_after`/`http_status` without dropping the rebuildable enrichment rows. Each column is
/// nullable so the ALTER never needs a default backfill; a fresh DB gets them straight from
/// [`VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL`] above and this ALTER is a no-op (guarded by the caller's
/// column probe). Centralized here so the column list has ONE source of truth next to the table.
pub(crate) fn add_visit_content_enrichment_w_enrich_columns(connection: &Connection) -> Result<()> {
    for column in [
        "ALTER TABLE visit_content_enrichments ADD COLUMN extractor_version INTEGER",
        "ALTER TABLE visit_content_enrichments ADD COLUMN enrichment_summary TEXT",
        "ALTER TABLE visit_content_enrichments ADD COLUMN refetch_after TEXT",
        "ALTER TABLE visit_content_enrichments ADD COLUMN http_status INTEGER",
    ] {
        if let Err(error) = connection.execute(column, []) {
            // A second run (or a fresh DB that already has the column from the baseline schema)
            // surfaces "duplicate column name"; that is the idempotent no-op, not a failure.
            let message = error.to_string();
            if !message.contains("duplicate column name") {
                return Err(error.into());
            }
        }
    }
    Ok(())
}

/// Maximum length (in characters) of the inline `enrichment_summary` (06 §3, ~280 chars).
///
/// The summary is the canonical short field that participates in the dedup `content_hash` and the
/// FTS5 `enrichment_text` mirror, so it must stay small (multi-KB summaries would bloat the search
/// index + wreck BM25 at the 14.4M tail, 06 §4). The full body stays in the content-addressed blob.
pub(crate) const ENRICHMENT_SUMMARY_CHAR_CAP: usize = 280;

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
    /// The capped inline summary (06 §3): the canonical short field fed into the dedup content_hash
    /// and the FTS5 mirror. `None` for rows written before W-ENRICH-1 or for failure rows.
    pub enrichment_summary: Option<String>,
    /// The extractor schema version this row was produced under (`None` for the offline title plugin),
    /// so a bumped extractor refetches only its own rows (06 §3).
    pub extractor_version: Option<i64>,
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
    /// The capped inline summary (06 §3). Built by extractors (`build_enrichment_summary`), it is the
    /// ONLY enrichment field that flows into the dedup content_hash + FTS5; the readable_text blob is
    /// excluded from both to keep the index bounded.
    pub enrichment_summary: Option<String>,
    /// Extractor schema version for refetch gating (`None` for the offline title plugin).
    pub extractor_version: Option<i64>,
}

/// Caps a candidate summary to [`ENRICHMENT_SUMMARY_CHAR_CAP`] characters on a UTF-8 boundary.
///
/// PURE → unit-tested. Whitespace is normalized first (so a multi-line description collapses to one
/// clean line) and an empty result yields `None` (an empty summary must contribute the SAME empty
/// dedup segment as no summary, mirroring `build_dedup_content_hash`). Truncation counts CHARACTERS
/// (CJK-safe) and trims any trailing partial whitespace so the stored summary never ends mid-space.
pub(crate) fn build_enrichment_summary(candidate: Option<&str>) -> Option<String> {
    let normalized = normalize_whitespace(candidate?);
    if normalized.is_empty() {
        return None;
    }
    if normalized.chars().count() <= ENRICHMENT_SUMMARY_CHAR_CAP {
        return Some(normalized);
    }
    let capped: String = normalized
        .chars()
        .take(ENRICHMENT_SUMMARY_CHAR_CAP)
        .collect::<String>()
        .trim_end()
        .to_string();
    if capped.is_empty() { None } else { Some(capped) }
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
            "SELECT history_id, fetch_status, fetched_at, readable_title, readable_text_blob_path,
                    snippet_json, enrichment_summary, extractor_version
             FROM visit_content_enrichments
             WHERE history_id IN ({placeholders})
             ORDER BY
               history_id ASC,
               CASE fetch_status WHEN 'success' THEN 0 WHEN 'empty' THEN 1 ELSE 2 END,
               CASE content_source
                 WHEN 'capture' THEN 0
                 WHEN 'github-repo' THEN 1
                 WHEN 'generic-readable' THEN 2
                 WHEN 'readable-content-refetch' THEN 3
                 WHEN 'title-normalization' THEN 4
                 ELSE 5
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
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        })?;
        for row in rows {
            let (
                history_id,
                fetch_status,
                fetched_at,
                readable_title,
                blob_path,
                snippet_json,
                enrichment_summary,
                extractor_version,
            ) = row?;
            map.entry(history_id).or_insert(StoredEnrichment {
                fetch_status,
                fetched_at,
                readable_title,
                readable_text: load_readable_text_blob(paths, blob_path.as_deref())?,
                snippet_json,
                enrichment_summary,
                extractor_version,
            });
        }
    }

    Ok(map)
}

/// Approximate token budget for the embedding payload's free text (doc 05 §8, ~512 tokens).
///
/// The embedding models PathKeep targets use a ~512-token context window; feeding a 1-hour transcript
/// into it is wasteful + meaningless (the tail is silently dropped by the model, and the front is what
/// matters). We cap the LONG `readable_text` to roughly this many tokens BEFORE handing it to the
/// model. Short, structured sources (a GitHub repo header, a video title+channel, the capped summary)
/// stay verbatim because they are already well under the window.
pub(crate) const EMBEDDING_TEXT_TOKEN_CAP: usize = 512;
/// Heuristic characters-per-token for the soft token cap (English ~4, CJK ~1.5; 4 is a safe average
/// that errs toward keeping MORE text for CJK, where each char is its own token).
const EMBEDDING_CHARS_PER_TOKEN: usize = 4;

/// Builds the canonical text blob fed into semantic indexing from visit/enrichment parts (05 §8).
///
/// The structured header (profile/visited-at/url/domain/title) plus, when present, the readable title,
/// the capped enrichment SUMMARY (06 §4: short structured sources verbatim), and the readable text
/// (capped to ~512 tokens, 05 §8 — a long transcript is truncated, never fed whole into a 512-token
/// window). The summary is added BEFORE the body so the most-signal-dense field survives any later
/// model-side truncation. The summary participates in the dedup hash separately; here it enriches the
/// embedded TEXT so a structured source (GitHub topics, video channel) is searchable semantically.
pub(crate) fn build_embedding_content_from_parts(
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
    readable_title: Option<&str>,
    readable_summary: Option<&str>,
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
    if let Some(readable_summary) = readable_summary.filter(|value| !value.trim().is_empty()) {
        // The summary is already capped (~280 chars) by the extractor, so it is added verbatim — it is
        // the highest-signal short field for a structured source.
        content.push_str(&format!("\nSummary: {}", readable_summary.trim()));
    }
    if let Some(readable_text) = readable_text.filter(|value| !value.trim().is_empty()) {
        content.push_str("\nReadable text:\n");
        content.push_str(&cap_embedding_text(readable_text.trim()));
    }
    content
}

/// Caps free text to roughly [`EMBEDDING_TEXT_TOKEN_CAP`] tokens on a char boundary (05 §8).
///
/// PURE → unit-tested. A char budget (`token cap × chars-per-token`) is a deliberately simple, model-
/// agnostic proxy for the token window: it never feeds a transcript longer than the window into the
/// model. Truncation counts CHARACTERS (CJK-safe) and trims trailing partial whitespace. Text already
/// under budget is returned unchanged (the common case for titles + short summaries).
pub(crate) fn cap_embedding_text(text: &str) -> String {
    let char_budget = EMBEDDING_TEXT_TOKEN_CAP.saturating_mul(EMBEDDING_CHARS_PER_TOKEN);
    if text.chars().count() <= char_budget {
        return text.to_string();
    }
    text.chars().take(char_budget).collect::<String>().trim_end().to_string()
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
        // The offline title plugin contributes no NETWORK-fetched summary: keeping its
        // `enrichment_summary` empty means it never re-hashes the dedup identity (06 §3 — only the
        // content-fetch extractors fill the summary slot). `extractor_version` stays `None` too.
        enrichment_summary: None,
        extractor_version: None,
    }
}

pub(crate) fn enrichment_is_terminal_failure(enrichment: &EnrichmentResult) -> bool {
    matches!(enrichment.status.as_str(), "fetch-error" | "decode-error" | "unsupported-content")
}

/// Returns the ISO timestamp before which a content-fetch row must not be retried (06 §2c).
///
/// PURE → unit-tested. Mirrors og:image's `default_refetch_after_for_status`: the cadence reflects how
/// likely a retry is to succeed. `success`/`empty` (the page fetched fine, the extractor just found
/// nothing) and `blocked` (an SSRF / user veto) schedule NO retry. Transient `fetch-error` cools down
/// fast; structural failures (`decode-error`, `unsupported-content` like a PDF/login wall) cool down
/// slowly so the worker never retry-storms a paywalled host. An unknown status returns `None` (stays
/// dormant) rather than hammering the network — matching the conservative og:image fallback.
pub(crate) fn default_enrichment_refetch_after_for_status(status: &str) -> Option<String> {
    let days: i64 = match status {
        "success" | "empty" | "blocked" => return None,
        "fetch-error" => 2,
        "rate-limited" => 1,
        "decode-error" => 7,
        "unsupported-content" => 30,
        _ => return None,
    };
    let when = chrono::Utc::now() + chrono::Duration::days(days);
    Some(when.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
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
    store_enrichment_with_cache(paths, connection, history_id, content_source, enrichment, None)
}

/// Stores one enrichment row, optionally stamping the negative-cache `refetch_after` cadence.
///
/// `refetch_after` is the ISO timestamp before which a failed fetch must not be retried (06 §2c, the
/// same negative-cache discipline og:image uses). The offline title plugin passes `None` (it never
/// hits the network, so it has no retry cadence); the content-fetch job runner derives it from the
/// terminal status via [`default_enrichment_refetch_after_for_status`]. `http_status` rides the
/// enrichment's `extraction["httpStatus"]` when the extractor recorded one.
pub(crate) fn store_enrichment_with_cache(
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
    content_source: &str,
    enrichment: &EnrichmentResult,
    refetch_after: Option<&str>,
) -> Result<()> {
    let stored_blob = store_readable_text_blob(paths, enrichment.readable_text.as_deref())?;
    let http_status = enrichment.extraction.get("httpStatus").and_then(Value::as_i64);
    connection.execute(
        "INSERT OR REPLACE INTO visit_content_enrichments
         (history_id, content_source, fetch_status, fetched_at, final_url, language, readable_title,
          readable_text_blob_path, readable_text_bytes, text_hash, snippet_json, extraction_json,
          pipeline_version, extractor_version, enrichment_summary, refetch_after, http_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
            enrichment.extractor_version,
            enrichment.enrichment_summary,
            refetch_after,
            http_status,
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
        enrichment.as_ref().and_then(|value| value.enrichment_summary.as_deref()),
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

/// Deterministic readability extraction over fetched HTML (no LLM, W-ENRICH-1 generic-readable).
///
/// Pulls the document title + a body-block readable text, applies the JSON-LD site adapter when the
/// host matches (YouTube/Vimeo VideoObject), and derives the capped `enrichment_summary` (06 §3) from
/// the first readable snippet. Pure (the job runner hands it already-fetched bytes; this NEVER touches
/// the network — 06 §1) so it is fully unit-tested. `extractor_version` is stamped by the calling
/// extractor, not here, so this stays a content-shaping helper.
pub(crate) fn build_enrichment_result_from_html(
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
    // The capped summary (06 §3) prefers the readable title (a clean, short label) and falls back to
    // the first readable block, so the dedup hash + FTS5 mirror get a concise canonical short field
    // instead of the multi-KB body. The body itself stays in the content-addressed blob.
    let summary_candidate = readable_title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| blocks.first().cloned());
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
        enrichment_summary: build_enrichment_summary(summary_candidate.as_deref()),
        extractor_version: None,
    };

    if let Some(adapter) = adapt_site_content(url, &document) {
        return merge_site_adapter_result(generic_result, &content_type, adapter);
    }

    generic_result
}

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
    // The video adapter's first snippet ("Video: {title}") is the cleanest short label; fall back to
    // the generic summary the body produced so the capped field is never empty when text exists.
    let summary_candidate = snippets
        .first()
        .cloned()
        .or_else(|| generic_result.enrichment_summary.clone())
        .or_else(|| readable_title.clone());

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
        enrichment_summary: build_enrichment_summary(summary_candidate.as_deref()),
        extractor_version: None,
    }
}

fn percent_decode(input: &str) -> String {
    // Decode into a byte buffer and interpret the result as UTF-8 at the end.
    // A percent escape encodes a single *byte*, not a Unicode scalar, so a
    // multi-byte UTF-8 sequence like `%E4%B8%AD` (中) must be reassembled from
    // its bytes — the previous `char::from_u32(byte)` produced one Latin-1
    // codepoint per byte (mojibake `ä¸­`), which is wrong for this product's
    // CJK-heavy history.
    let bytes = input.as_bytes();
    let mut decoded: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                let hi = (bytes[index + 1] as char).to_digit(16);
                let lo = (bytes[index + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    decoded.push((hi * 16 + lo) as u8);
                    index += 2;
                } else {
                    decoded.push(b'%');
                }
            }
            value => decoded.push(value),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
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

pub(crate) fn truncate_text(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }
    input.chars().take(limit).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{
        EMBEDDING_TEXT_TOKEN_CAP, ENRICHMENT_SUMMARY_CHAR_CAP, EnrichmentResult, SiteAdapterResult,
        StoredEnrichment, build_embedding_content_from_parts, build_enrichment_result_from_html,
        build_enrichment_summary, cap_embedding_text, default_enrichment_refetch_after_for_status,
        enrichment_failure_message, enrichment_is_terminal_failure,
        ensure_visit_content_enrichment_schema, execute_enrichment_job_by_id,
        fail_unknown_enrichment_plugin, finish_claimed_enrichment_job,
        finish_if_enrichment_cancelled, load_best_enrichment_map_by_history_ids,
        merge_site_adapter_result, preferred_embedding_content, store_enrichment,
        title_normalization_enrichment,
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
    fn percent_encoded_cjk_url_slug_decodes_to_utf8_not_mojibake() {
        // Regression: `%E4%B8%AD%E6%96%87` is the UTF-8 encoding of "中文". The
        // decoder must reassemble the raw bytes into one string, not emit one
        // Latin-1 codepoint per byte (which produced mojibake `ä¸­æ–‡`).
        let enrichment =
            title_normalization_enrichment("https://example.com/articles/%E4%B8%AD%E6%96%87", None);
        assert_eq!(enrichment.readable_title.as_deref(), Some("中文"));
    }

    #[test]
    fn build_embedding_content_includes_only_non_empty_enrichment_fields() {
        let enriched = build_embedding_content_from_parts(
            "chrome:Default",
            "https://example.com/docs/intro",
            Some("Intro"),
            "2026-04-17T00:00:00Z",
            Some("Readable Intro"),
            Some("A capped summary"),
            Some("First paragraph"),
        );
        assert!(enriched.contains("Readable title: Readable Intro"));
        assert!(enriched.contains("Summary: A capped summary"));
        assert!(enriched.contains("Readable text:\nFirst paragraph"));

        let without_enrichment = build_embedding_content_from_parts(
            "chrome:Default",
            "https://example.com/docs/intro",
            None,
            "2026-04-17T00:00:00Z",
            Some("   "),
            Some("  "),
            Some("  "),
        );
        assert!(without_enrichment.contains("Title: (untitled)"));
        assert!(!without_enrichment.contains("Readable title:"));
        assert!(!without_enrichment.contains("Summary:"));
        assert!(!without_enrichment.contains("Readable text:"));
    }

    #[test]
    fn cap_embedding_text_truncates_long_text_to_the_token_budget() {
        // Short text is unchanged.
        assert_eq!(cap_embedding_text("a short body"), "a short body");
        // A body longer than the ~512-token char budget is truncated; never fed whole into the window.
        let long = "x ".repeat(5_000); // ~10k chars, well over 512 * 4 = 2048
        let capped = cap_embedding_text(&long);
        assert!(capped.chars().count() <= EMBEDDING_TEXT_TOKEN_CAP * 4);
        assert!(capped.chars().count() < long.chars().count());
    }

    #[test]
    fn build_enrichment_summary_caps_and_normalizes() {
        // Whitespace is collapsed; an empty result is None.
        assert_eq!(build_enrichment_summary(Some("  a   b  ")).as_deref(), Some("a b"));
        assert_eq!(build_enrichment_summary(Some("   ")), None);
        assert_eq!(build_enrichment_summary(None), None);
        // A long summary is capped to ~280 chars.
        let long = "word ".repeat(200); // 1000 chars
        let capped = build_enrichment_summary(Some(&long)).expect("capped");
        assert!(capped.chars().count() <= ENRICHMENT_SUMMARY_CHAR_CAP);
    }

    #[test]
    fn default_enrichment_refetch_after_for_status_matches_negative_cache_cadence() {
        // Success / empty / blocked never schedule a retry.
        assert!(default_enrichment_refetch_after_for_status("success").is_none());
        assert!(default_enrichment_refetch_after_for_status("empty").is_none());
        assert!(default_enrichment_refetch_after_for_status("blocked").is_none());
        // Transient + structural failures schedule a (future) retry timestamp.
        for status in ["fetch-error", "rate-limited", "decode-error", "unsupported-content"] {
            let when = default_enrichment_refetch_after_for_status(status);
            assert!(when.is_some(), "{status} must schedule a retry");
            assert!(when.unwrap().ends_with('Z'));
        }
        // Unknown status stays dormant (no retry storm).
        assert!(default_enrichment_refetch_after_for_status("mystery").is_none());
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
                enrichment_summary: None,
                extractor_version: None,
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
                enrichment_summary: None,
                extractor_version: None,
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
                ..EnrichmentResult::default()
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
                ..EnrichmentResult::default()
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
