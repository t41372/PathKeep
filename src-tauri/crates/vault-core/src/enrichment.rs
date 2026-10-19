//! Responsibilities: persist enrichment outcomes and provide text for native indexing.
//! Not responsible for: network fetches or canonical archive ownership.
//! Failure modes: cancellation loses a race, failures look successful, or sibling visits diverge.
//! Tests: content-fetch consent and private-address gates, page embedding parts, and HTML text
//! extraction that excludes executable and decorative subtrees.
//!
//! Core Intelligence and optional AI both depend on `visit_content_enrichments`
//! as a rebuildable evidence plane. Keeping the shared schema, enrichment
//! execution path, and lookup helpers here prevents the legacy `insights`
//! module from remaining the canonical owner of readable-text evidence after
//! the hard cutover.

mod embedding_document;
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
pub(crate) mod search_text;

use crate::{
    config::ProjectPaths,
    intelligence_blobs::{load_readable_text_blob, store_readable_text_blob},
    intelligence_runtime::{ClaimedEnrichmentJob, claim_enrichment_job_by_id},
    models::{READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID},
    utils::now_rfc3339,
};
use anyhow::Result;
use scraper::{Html, Selector};
use serde_json::{Value, json};
use std::collections::HashMap;
use vault_store::{Db, Priority, Session, params_owned as params};

pub(crate) use self::embedding_document::{EmbeddingDocument, build_embedding_document};
use self::site_adapters::{SiteAdapterResult, adapt_site_content};

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

/// Claims and executes one persisted enrichment job from the intelligence queue.
pub fn execute_enrichment_job_by_id(vault: &crate::Vault, job_id: i64) -> Result<bool> {
    let Some(job) = vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        crate::intelligence_runtime::claim_result(claim_enrichment_job_by_id(s, job_id))
    })??
    else {
        return Ok(false);
    };
    if let Some(done) = vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        finish_if_enrichment_cancelled(s, job_id)
    })? {
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
            vault.write(Db::Intelligence, Priority::Interactive, move |s| {
                fail_unknown_enrichment_plugin(s, job.id, &job.plugin_id)
            })?;
            return Ok(true);
        }
    };
    let paths = vault.paths().clone();
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        finish_claimed_enrichment_job(&paths, s, &job, &enrichment)
    })
}

fn finish_claimed_enrichment_job(
    paths: &ProjectPaths,
    connection: &Session<'_>,
    job: &ClaimedEnrichmentJob,
    enrichment: &EnrichmentResult,
) -> Result<bool> {
    if cancel_running_enrichment_job_if_requested(connection, job.id)? {
        return Ok(true);
    }
    store_enrichment(
        paths,
        connection,
        &crate::pages::canonical_key(&job.payload.url),
        &job.plugin_id,
        enrichment,
    )?;
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
        if !crate::intelligence_runtime::mark_intelligence_job_failed_in_session(
            connection,
            job.id,
            &enrichment_failure_message(enrichment),
        )? {
            let _ = crate::intelligence_runtime::mark_running_intelligence_job_cancelled_in_session(
                connection,
                job.id,
                "cancelled from UI",
            );
        }
    } else if !crate::intelligence_runtime::mark_intelligence_job_succeeded_in_session(
        connection, job.id, &artifact,
    )? {
        let _ = crate::intelligence_runtime::mark_running_intelligence_job_cancelled_in_session(
            connection,
            job.id,
            "cancelled from UI",
        );
    }

    Ok(true)
}

fn cancel_running_enrichment_job_if_requested(
    connection: &Session<'_>,
    job_id: i64,
) -> Result<bool> {
    if crate::intelligence_runtime::intelligence_job_stop_requested_in_session(connection, job_id)?
    {
        let _ = crate::intelligence_runtime::mark_running_intelligence_job_cancelled_in_session(
            connection,
            job_id,
            "cancelled from UI",
        );
        return Ok(true);
    }
    Ok(false)
}

fn fail_unknown_enrichment_plugin(
    connection: &Session<'_>,
    job_id: i64,
    plugin_id: &str,
) -> Result<()> {
    if !crate::intelligence_runtime::mark_intelligence_job_failed_in_session(
        connection,
        job_id,
        &format!("Unknown enrichment plugin {plugin_id}"),
    )? {
        let _ = crate::intelligence_runtime::mark_running_intelligence_job_cancelled_in_session(
            connection,
            job_id,
            "cancelled from UI",
        );
    }
    Ok(())
}

fn finish_if_enrichment_cancelled(connection: &Session<'_>, job_id: i64) -> Result<Option<bool>> {
    if cancel_running_enrichment_job_if_requested(connection, job_id)? {
        return Ok(Some(true));
    }
    Ok(None)
}

/// The order that picks a page's best enrichment: a successful fetch, then the
/// most specific source, then the newest. Shared by every reader that needs one.
pub(crate) const BEST_ENRICHMENT_ORDER: &str =
    "CASE fetch_status WHEN 'success' THEN 0 WHEN 'empty' THEN 1 ELSE 2 END,
               CASE content_source
                 WHEN 'capture' THEN 0
                 WHEN 'github-repo' THEN 1
                 WHEN 'generic-readable' THEN 2
                 WHEN 'readable-content-refetch' THEN 3
                 WHEN 'title-normalization' THEN 4
                 ELSE 5
               END,
               fetched_at DESC";

pub(crate) fn load_best_enrichment_map_by_history_ids(
    paths: &ProjectPaths,
    connection: &vault_store::Session<'_>,
    history_ids: &[i64],
) -> Result<HashMap<i64, StoredEnrichment>> {
    if history_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::new();
    let mut bodies = HashMap::<String, Option<String>>::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT v.id, fetch_status, fetched_at, readable_title, readable_text_blob_path,
                    snippet_json, enrichment_summary, extractor_version
             FROM archive.visits v CROSS JOIN archive.urls u ON u.id=v.url_id
             CROSS JOIN archive.pages p ON p.id=u.page_id
             JOIN page_content_enrichments e ON e.canonical_url=p.canonical_url
             WHERE v.id IN ({placeholders})
             ORDER BY v.id ASC, {BEST_ENRICHMENT_ORDER}"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = chunk.iter().copied();
        let rows = statement.query_map(vault_store::params_from_iter(params), |row| {
            Ok((
                row.get::<i64>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
                row.get::<Option<String>>(3)?,
                row.get::<Option<String>>(4)?,
                row.get::<String>(5)?,
                row.get::<Option<String>>(6)?,
                row.get::<Option<i64>>(7)?,
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
            let readable_text = if let Some(path) = blob_path.as_deref() {
                if !bodies.contains_key(path) {
                    bodies.insert(
                        path.to_owned(),
                        load_readable_text_blob(paths, Some(path))?
                            .map(|text| cap_embedding_text(&text)),
                    );
                }
                bodies[path].clone()
            } else {
                None
            };
            map.entry(history_id).or_insert(StoredEnrichment {
                fetch_status,
                fetched_at,
                readable_title,
                readable_text,
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
/// Characters of text sent to an embedding model for one page, all fields together.
pub(crate) const EMBEDDING_TEXT_CHAR_BUDGET: usize =
    EMBEDDING_TEXT_TOKEN_CAP * EMBEDDING_CHARS_PER_TOKEN;

/// Caps free text to roughly [`EMBEDDING_TEXT_TOKEN_CAP`] tokens on a char boundary (05 §8).
///
/// PURE → unit-tested. A char budget (`token cap × chars-per-token`) is a deliberately simple, model-
/// agnostic proxy for the token window: it never feeds a transcript longer than the window into the
/// model. Truncation counts CHARACTERS (CJK-safe) and trims trailing partial whitespace. Text already
/// under budget is returned unchanged (the common case for titles + short summaries).
pub(crate) fn cap_embedding_text(text: &str) -> String {
    let char_budget = EMBEDDING_TEXT_CHAR_BUDGET;
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
    connection: &Session<'_>,
    canonical_url: &str,
    content_source: &str,
    enrichment: &EnrichmentResult,
) -> Result<()> {
    store_enrichment_with_cache(paths, connection, canonical_url, content_source, enrichment, None)
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
    connection: &Session<'_>,
    canonical_url: &str,
    content_source: &str,
    enrichment: &EnrichmentResult,
    refetch_after: Option<&str>,
) -> Result<()> {
    let stored_blob = store_readable_text_blob(paths, enrichment.readable_text.as_deref())?;
    let http_status = enrichment.extraction.get("httpStatus").and_then(Value::as_i64);
    connection.execute(
        "INSERT OR REPLACE INTO page_content_enrichments
         (canonical_url, content_source, fetch_status, fetched_at, final_url, language, readable_title,
          readable_text_blob_path, readable_text_bytes, text_hash, snippet_json, extraction_json,
          pipeline_version, extractor_version, enrichment_summary, refetch_after, http_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            canonical_url,
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
            if node
                .ancestors()
                .filter_map(scraper::ElementRef::wrap)
                .any(|ancestor| excluded_text_element(ancestor.value().name()))
            {
                continue;
            }
            let value = readable_element_text(node);
            if !value.is_empty() {
                blocks.push(value);
            }
        }
        if blocks.is_empty() {
            let fallback = readable_element_text(root);
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

fn excluded_text_element(name: &str) -> bool {
    matches!(name, "script" | "style" | "noscript" | "template" | "svg")
}

fn readable_element_text(node: scraper::ElementRef<'_>) -> String {
    fn collect(node: scraper::ElementRef<'_>, text: &mut String) {
        if excluded_text_element(node.value().name()) {
            return;
        }
        for child in node.children() {
            if let Some(value) = child.value().as_text() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(value);
            } else if let Some(element) = scraper::ElementRef::wrap(child) {
                collect(element, text);
            }
        }
    }
    let mut text = String::new();
    collect(node, &mut text);
    normalize_whitespace(&text)
}

#[cfg(test)]
#[path = "enrichment/html_text_tests.rs"]
mod html_text_tests;
