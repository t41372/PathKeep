//! Content-fetch job runner — the SINGLE egress chokepoint for site enrichment (W-ENRICH-1, doc 06).
//!
//! ## Responsibilities
//! - Own the `content-fetch` job: claim a queued row → consent + SSRF + per-host rate-limit gates →
//!   fetch via the ONE shared `build_fetch_client()` → route to the matched [`Extractor`] → store the
//!   result with the negative-cache `refetch_after` cadence.
//! - Perform ALL network egress so extractors never touch a socket (06 §1/§2): every page URL AND
//!   every API sub-resource is `net_guard`-checked, https-only, redirect/size/MIME-capped, and
//!   rate-limited before the GET.
//! - Enqueue content-fetch jobs (keyed by UNIQUE URL so 5000 gmail visits fetch once, 06 §3) and gate
//!   re-fetches by the negative cache + extractor version.
//!
//! ## Not responsible for
//! - The dedup content_hash or index consumption, the working-set ranking ([`crate::ai::working_set`] is the prioritization
//!   hook the enqueue caller feeds in), or extractor parsing (the extractors own that).
//!
//! Failure modes: enrichment misses its canonical document, an absent document publishes
//! a phantom event, a failed outbox write is reported as a completed fetch job. Tests:
//! finish_fans_enrichment_out_to_sibling_visits_and_publishes_outbox and search_documents tests.
//!
//! ## Privacy posture (06 §2 — these GATE acceptance)
//! - Egress ONLY through `build_fetch_client` (desktop Chrome UA, no Referer/cookies/fingerprint).
//! - `content_fetch_enabled` master switch (hard-default-OFF) + per-extractor + per-domain gates.
//! - Honest failure status (login/paywall/PDF/non-HTML/429 → real status, never a fake success) +
//!   negative-cache cadence so a failing host is never retry-stormed.
//!
//! Real network calls live behind `cfg(not(any(test, coverage)))`; the test build drives the pipeline
//! through `*_with_fetcher` seams + a [`Fetcher`] trait so mockito fixtures cover every branch without
//! the coverage gate flagging the (deliberately uncovered) production socket call.

use super::extractors::{ApiRequest, ExtractContext, ExtractKind, Extractor, resolve_extractor};
use super::rate_limit::{acquire_host_token, next_token_eta_secs};
use super::{
    EnrichmentResult, default_enrichment_refetch_after_for_status, store_enrichment_with_cache,
};
#[cfg(not(test))]
use crate::archive::net_guard::url_target_is_blocked;

// Unit fixtures never resolve DNS. Literal-address checks still exercise the real classifier.
#[cfg(test)]
fn url_target_is_blocked(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return true;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    let Some(host) = url.host_str() else {
        return true;
    };
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    match literal.parse::<std::net::IpAddr>() {
        Ok(ip) => crate::archive::net_guard::ip_is_non_public(ip),
        Err(_) => host == "localhost" || host.ends_with(".localhost"),
    }
}
use crate::intelligence_runtime::{
    ClaimedEnrichmentJob, EnrichmentJobPayload, claim_content_fetch_job_by_id,
    intelligence_job_stop_requested, mark_intelligence_job_failed,
    mark_running_intelligence_job_cancelled, requeue_content_fetch_job_after,
};
use crate::models::{AppConfig, content_extractor_enabled, content_fetch_domain_blocked};
use crate::utils::url_domain;
use anyhow::Result;
use serde_json::json;
use vault_store::{Db, Priority, Session, params_owned as params};

/// Hard cap on the HTML body the runner will read (matches og:image: 1 MiB, 06 §2b).
///
/// The redirect-hop budget (8) lives in `build_fetch_client`; the runner does not re-configure it.
pub(crate) const MAX_HTML_BODY_BYTES: usize = 1_048_576;

/// The terminal outcome of a fetched resource the runner hands to extractor routing.
///
/// Carries the bytes + content-type so the runner can MIME-guard before parsing and so the extractor
/// receives an honest `ExtractContext`. Failure shapes carry the status string the negative cache +
/// stored row use; the runner never fabricates a success.
#[derive(Debug, Clone)]
pub(crate) enum FetchOutcome {
    /// A body was fetched: bytes + lower-cased content-type head + post-redirect final URL.
    Body {
        bytes: Vec<u8>,
        content_type: Option<String>,
        final_url: Option<String>,
        http_status: i64,
    },
    /// The fetch failed honestly: a status token (`fetch-error` | `rate-limited` | `unsupported-content`
    /// | `decode-error` | `blocked`) + optional http status + a user-facing detail.
    Failed { status: String, http_status: Option<i64>, detail: String },
}

/// Abstracts the actual HTTP GET so the test build can inject a deterministic fetcher.
///
/// Production wires [`SharedClientFetcher`] (the real `build_fetch_client` egress, behind the coverage
/// cfg) whose redirect policy re-applies the SSRF guard on EVERY hop (SEC-3), so a public page that
/// 30x-redirects to a private host is stopped mid-chain. The runner additionally guards the initial
/// URL before calling AND re-checks the post-redirect `final_url` after the body returns
/// (defence-in-depth, see [`reject_nonpublic_final_url`]) — but the trait contract itself is just
/// "GET this already-guarded URL and report the final URL you landed on". Tests inject a fake or a
/// mockito-backed fetcher so every routing/guard/negative-cache branch is covered without the coverage
/// gate flagging the production socket call.
pub(crate) trait Fetcher {
    /// Distinguishes offline fixtures from transports covered by the debug egress switch.
    fn networked(&self) -> bool {
        true
    }
    /// GETs `url` as HTML (Accept: text/html, body capped at `MAX_HTML_BODY_BYTES`).
    fn fetch_html(&self, url: &str) -> FetchOutcome;
    /// GETs `url` as a JSON API resource (Accept: application/json, body capped at `cap`).
    fn fetch_json(&self, url: &str, cap: usize) -> FetchOutcome;
}

/// Result of one content-fetch attempt: the enrichment to store + the negative-cache timestamp.
#[derive(Debug, Clone)]
pub(crate) struct ContentFetchResult {
    pub content_source: String,
    pub enrichment: EnrichmentResult,
    pub refetch_after: Option<String>,
    /// The host whose rate-limit bucket this attempt CHARGED — the page host for an HTML extractor,
    /// the API host for a `JsonApi` extractor (SEC-1: that is where the GETs actually land). When the
    /// attempt is throttled (`status == "rate-limited"`) the deferral ETA (SEC-2) is read from THIS
    /// host's bucket, not the page host, so a GitHub job sleeps the ~60s `api.github.com` needs rather
    /// than the always-full `github.com` bucket's 0s. Always set to the real targeted host (no
    /// fallback), so the deferral site reads it directly.
    pub egress_host: String,
}

/// What one `execute_content_fetch_job_*` call did, so the drain knows whether MORE work is due now.
///
/// `Ran` (a row was stored / the job was terminally cancelled) means the drain should loop for the
/// next due job. `Deferred` means the job was REQUEUED for a future `scheduled_at` (rate-limit
/// back-pressure, SEC-2) — there is nothing more due *right now*, so the drain reports idle and the
/// worker lane sleeps until the deferred row's schedule. `NotClaimable` means the id was not a queued
/// content-fetch row (idle). Both `Deferred` and `NotClaimable` map to "drain returned false".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentFetchJobOutcome {
    /// A row was stored or the job was terminally cancelled — loop for the next due job.
    Ran,
    /// The job was requeued for a later drain (rate-limited) — nothing more is due now.
    Deferred,
    /// The id was not a claimable queued content-fetch row.
    NotClaimable,
    /// No canonical page has reached the archive yet; a later candidate pass can requeue it.
    AwaitingArchive,
}

/// Runs one content-fetch attempt for a payload through an injected [`Fetcher`] (PURE of real egress).
///
/// This is the testable core: resolve the extractor (first-match-wins; generic-readable is the
/// terminal fallback, so an extractor ALWAYS resolves) → fetch its declared resources through the
/// fetcher (which guards + rate-limits) → route bytes to `extract` → derive the negative-cache
/// `refetch_after` from the terminal status. The caller persists the result.
pub(crate) fn run_content_fetch_with_fetcher(
    fetcher: &dyn Fetcher,
    payload: &EnrichmentJobPayload,
) -> ContentFetchResult {
    // `resolve_extractor` cannot return `None` (generic-readable matches every http(s) URL); fall back
    // defensively to generic-readable rather than panicking if the registry ever drops its fallback.
    let extractor = resolve_extractor(&payload.url)
        .unwrap_or_else(|| Box::new(super::extractors::GenericReadableExtractor));
    // Avoid DNS and rate-limit deferrals as well as the actual HTML/API requests.
    if fetcher.networked() && crate::test_support::network_is_blocked() {
        return ContentFetchResult {
            content_source: extractor.id().to_string(),
            enrichment: failure_enrichment(
                extractor.as_ref(),
                &payload.url,
                "blocked",
                None,
                "The page URL is not https or resolves to a non-public address.",
            ),
            refetch_after: None,
            egress_host: url_domain(&payload.url),
        };
    }
    let (enrichment, egress_host) = fetch_and_extract(fetcher, extractor.as_ref(), &payload.url);
    let refetch_after = default_enrichment_refetch_after_for_status(&enrichment.status);
    ContentFetchResult {
        content_source: extractor.id().to_string(),
        enrichment,
        refetch_after,
        egress_host,
    }
}

/// Fetches the extractor's declared resources and routes them to `extract`, or returns a failure row.
///
/// Also returns the EGRESS host this attempt charged (the page host for HTML, the API host for a
/// `JsonApi` resource, SEC-1) so a throttled attempt's deferral ETA reads the bucket that was actually
/// exhausted (BUG-1 / SEC-2). The host is always the real targeted host — no fallback at the read site.
fn fetch_and_extract(
    fetcher: &dyn Fetcher,
    extractor: &dyn Extractor,
    url: &str,
) -> (EnrichmentResult, String) {
    match extractor.fetch_kind() {
        ExtractKind::Html => {
            // Per-egress rate limit keyed by the ACTUAL host being hit (SEC-1): the HTML page host. An
            // empty bucket parks the job (a `rate-limited` result the runner defers, SEC-2) rather than
            // exceeding the host's budget.
            let egress_host = url_domain(url);
            if !acquire_host_token(&egress_host) {
                return (
                    failure_enrichment(extractor, url, "rate-limited", None, &rate_limit_detail()),
                    egress_host,
                );
            }
            let enrichment = match guard_final_url(fetcher.fetch_html(url)) {
                FetchOutcome::Body { bytes, content_type, final_url, .. } => {
                    let ctx = ExtractContext {
                        url: url.to_string(),
                        final_url,
                        primary_body: bytes,
                        content_type,
                        secondary_body: None,
                    };
                    extractor.extract(&ctx)
                }
                FetchOutcome::Failed { status, http_status, detail } => {
                    failure_enrichment(extractor, url, &status, http_status, &detail)
                }
            };
            (enrichment, egress_host)
        }
        ExtractKind::JsonApi => fetch_json_api(fetcher, extractor, url),
    }
}

/// Detail string for a parked (host-bucket-empty) egress (SEC-1/SEC-2).
fn rate_limit_detail() -> String {
    "The per-host request budget is exhausted; the fetch was deferred.".to_string()
}

/// Defence-in-depth (SEC-3): converts a body landing on a non-public final host into a `blocked` row.
///
/// The shared client re-guards every redirect HOP, but this is a cheap post-fetch backstop: if the
/// post-redirect `final_url` resolves to a private/loopback/metadata address, drop the body and report
/// `blocked` (no parse, no store of internal content) rather than handing internal bytes to an
/// extractor. Coverable via a fake fetcher that reports a private `final_url`.
fn guard_final_url(outcome: FetchOutcome) -> FetchOutcome {
    if let FetchOutcome::Body { final_url: Some(final_url), .. } = &outcome {
        if url_target_is_blocked(final_url) {
            return FetchOutcome::Failed {
                status: "blocked".to_string(),
                http_status: None,
                detail: format!(
                    "The fetch landed on {final_url}, which resolves to a non-public address."
                ),
            };
        }
    }
    outcome
}

/// Fetches the JSON primary (+ optional secondary) for a [`ExtractKind::JsonApi`] extractor.
///
/// Returns the enrichment + the EGRESS host charged (the PRIMARY API host, SEC-1) so a throttled
/// attempt's deferral ETA reads the API bucket — `api.github.com` for GitHub, not the page host
/// `github.com` (BUG-1). When the extractor declares no API request there is no egress, so the page
/// host is reported as the (unused) targeted host: that arm yields `fetch-error`, never `rate-limited`,
/// so the deferral site never reads it.
fn fetch_json_api(
    fetcher: &dyn Fetcher,
    extractor: &dyn Extractor,
    url: &str,
) -> (EnrichmentResult, String) {
    let Some(primary) = extractor.api_request(url) else {
        return (
            failure_enrichment(extractor, url, "fetch-error", None, "No API request to fetch."),
            url_domain(url),
        );
    };
    // The bucket the primary GET charges is the API host (SEC-1) — the deferral ETA must read it.
    let egress_host = url_domain(&primary.url);
    let (bytes, content_type, final_url, _http) = match guard_then_fetch_json(fetcher, &primary) {
        FetchOutcome::Body { bytes, content_type, final_url, http_status } => {
            (bytes, content_type, final_url, http_status)
        }
        FetchOutcome::Failed { status, http_status, detail } => {
            return (
                failure_enrichment(extractor, url, &status, http_status, &detail),
                egress_host,
            );
        }
    };

    // Optional follow-up resource (e.g. the GitHub README). A failed secondary is NOT fatal — the
    // primary metadata is still worth storing — so we just omit the secondary body on failure.
    let secondary_body = match extractor.secondary_api_request(url) {
        Some(secondary) => match guard_then_fetch_json(fetcher, &secondary) {
            FetchOutcome::Body { bytes, .. } => Some(bytes),
            FetchOutcome::Failed { .. } => None,
        },
        None => None,
    };

    let ctx = ExtractContext {
        url: url.to_string(),
        final_url,
        primary_body: bytes,
        content_type,
        secondary_body,
    };
    (extractor.extract(&ctx), egress_host)
}

/// SSRF-guards an API sub-resource URL before fetching it (06 §2b: EVERY sub-resource is guarded).
///
/// Returns a `blocked` [`FetchOutcome::Failed`] when the API URL targets a non-public address so the
/// caller short-circuits with an honest `blocked` row (no network, no retry). Otherwise fetches via
/// the injected fetcher. Returning `FetchOutcome` (not a large `Result<_, EnrichmentResult>`) keeps
/// the failure path uniform with the body-read failures the caller already handles.
fn guard_then_fetch_json(fetcher: &dyn Fetcher, request: &ApiRequest) -> FetchOutcome {
    if url_target_is_blocked(&request.url) {
        return FetchOutcome::Failed {
            status: "blocked".to_string(),
            http_status: None,
            detail: format!(
                "The API sub-resource {} resolves to a non-public address and was blocked.",
                request.url
            ),
        };
    }
    // Per-egress rate limit keyed by the API HOST (SEC-1): GitHub's 60/hr cap lives on `api.github.com`
    // — where the GETs actually land — not the page host (`github.com`, which receives 0 requests).
    // Acquiring HERE charges the bucket of the host that is really hit.
    if !acquire_host_token(&url_domain(&request.url)) {
        return FetchOutcome::Failed {
            status: "rate-limited".to_string(),
            http_status: None,
            detail: rate_limit_detail(),
        };
    }
    guard_final_url(fetcher.fetch_json(&request.url, request.body_cap_bytes))
}

/// Builds a failure [`EnrichmentResult`] carrying the extractor version + honest status (06 §2c).
fn failure_enrichment(
    extractor: &dyn Extractor,
    url: &str,
    status: &str,
    http_status: Option<i64>,
    detail: &str,
) -> EnrichmentResult {
    let mut extraction = json!({
        "extractor": extractor.id(),
        "error": detail,
    });
    if let Some(http_status) = http_status {
        extraction["httpStatus"] = json!(http_status);
    }
    EnrichmentResult {
        status: status.to_string(),
        final_url: Some(url.to_string()),
        extraction,
        extractor_version: Some(extractor.version() as i64),
        ..EnrichmentResult::default()
    }
}

/// Whether `config` permits a content-fetch of `url` (master switch + per-extractor + per-domain).
///
/// PURE → unit-tested. The master `content_fetch_enabled` is the hard gate (06 §2a, default OFF): when
/// off, this is always `false` and the job runner is a no-op. With it on, the matched extractor's
/// per-extractor toggle must be enabled AND the URL's domain must not be on the per-domain blocklist.
/// Centralized so the enqueue path and the runner share ONE consent decision.
pub(crate) fn content_fetch_allowed(config: &AppConfig, url: &str) -> bool {
    if !config.ai.content_fetch_enabled {
        return false;
    }
    let Some(extractor) = resolve_extractor(url) else {
        return false;
    };
    if !content_extractor_enabled(config, extractor.id()) {
        return false;
    }
    !content_fetch_domain_blocked(config, &url_domain(url))
}

/// Executes one claimed content-fetch job end-to-end (claim → gates → fetch → store → cache).
///
/// The production entry the worker drain calls. Resolves consent against `config`, claims the queued
/// row, runs the fetch through the production [`SharedClientFetcher`], stores the result with its
/// negative-cache `refetch_after`, and marks the job succeeded/failed. A consent-denied job is parked
/// (left non-failed); a rate-limited job is REQUEUED for a later drain (SEC-2) rather than terminally
/// cancelled. Returns the [`ContentFetchJobOutcome`] so the drain knows whether more work is due now.
#[cfg(not(any(test, coverage)))]
pub fn execute_content_fetch_job_by_id(vault: &crate::Vault, job_id: i64) -> Result<bool> {
    let fetcher = SharedClientFetcher::new()?;
    let outcome = execute_content_fetch_job_with_fetcher(vault, job_id, &fetcher)?;
    Ok(matches!(outcome, ContentFetchJobOutcome::Ran | ContentFetchJobOutcome::AwaitingArchive))
}

/// Test/coverage stub for the production entry: the worker still links, but real egress is excluded.
///
/// The real `execute_content_fetch_job_by_id` performs the (deliberately uncovered) socket fetch, so it
/// is cfg-gated out of the coverage build. It stays faithful to the production CONTRACT — Ran → `true`,
/// Deferred/NotClaimable → `false` — by driving the real [`execute_content_fetch_job_with_fetcher`]
/// core through a deterministic offline [`OfflineStubFetcher`] (no socket). That keeps the worker
/// drain's run/defer signal honest under the coverage build (BUG-3): a job whose egress bucket is
/// exhausted defers → `false`, so the lane idles + sleeps rather than busy-looping the parked row.
#[cfg(any(test, coverage))]
pub fn execute_content_fetch_job_by_id(vault: &crate::Vault, job_id: i64) -> Result<bool> {
    let outcome = execute_content_fetch_job_with_fetcher(vault, job_id, &OfflineStubFetcher)?;
    Ok(matches!(outcome, ContentFetchJobOutcome::Ran | ContentFetchJobOutcome::AwaitingArchive))
}

/// An offline fetcher for the coverage build: returns a small benign body for any URL (no socket), so a
/// claimed, un-throttled job runs to a stored row (`Ran`) while a job whose host bucket is drained still
/// defers (`rate-limited`) BEFORE this is ever called — keeping the stub `execute_content_fetch_job_by_id`
/// faithful to the production run/defer contract without real egress.
#[cfg(any(test, coverage))]
struct OfflineStubFetcher;

#[cfg(any(test, coverage))]
impl Fetcher for OfflineStubFetcher {
    fn networked(&self) -> bool {
        false
    }
    fn fetch_html(&self, _url: &str) -> FetchOutcome {
        FetchOutcome::Body {
            bytes: b"<html><head><title>Stub</title></head><body><main><p>Stub.</p></main></body></html>".to_vec(),
            content_type: Some("text/html".to_string()),
            final_url: None,
            http_status: 200,
        }
    }
    fn fetch_json(&self, _url: &str, _cap: usize) -> FetchOutcome {
        FetchOutcome::Body {
            bytes: br#"{"full_name":"o/r","description":"Stub repo","topics":["x"]}"#.to_vec(),
            content_type: Some("application/json".to_string()),
            final_url: None,
            http_status: 200,
        }
    }
}

/// Executes one claimed content-fetch job through an injected fetcher (the testable production core).
///
/// Shared by the production entry (real fetcher) and the tests (fake/mockito fetcher), so every gate
/// — consent, claim, SSRF, rate-limit, store, cache, cancel — is covered without the real socket call.
/// Gate order follows doc 06 §2b: claim → stop → consent → https + SSRF page guard → fetch (which
/// acquires the per-egress rate-limit token at the REAL host) → store. The SSRF/https page guard runs
/// BEFORE any rate-limit acquire so a blocked/non-https job never burns a token (SEC-4).
pub(crate) fn execute_content_fetch_job_with_fetcher(
    vault: &crate::Vault,
    job_id: i64,
    fetcher: &dyn Fetcher,
) -> Result<ContentFetchJobOutcome> {
    let Some(job) = vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        crate::intelligence_runtime::claim_result(claim_content_fetch_job_by_id(s, job_id))
    })??
    else {
        return Ok(ContentFetchJobOutcome::NotClaimable);
    };

    let outcome = (|| -> Result<ContentFetchJobOutcome> {
        let artifact = vault.read(Db::Intelligence, move |s| {
            Ok(s.query_one(
                "SELECT artifact_json FROM intelligence_jobs WHERE id=?1",
                [job_id],
                |r| Ok(r.get::<String>(0)?),
            )?)
        })?;
        let artifact: serde_json::Value = serde_json::from_str(&artifact)?;
        if artifact["contentStored"].as_bool() == Some(true) {
            let source =
                artifact["contentSource"].as_str().expect("stored handoff records its source");
            publish_stored_content(vault, &job, source)?;
            return Ok(ContentFetchJobOutcome::Ran);
        }
        // Consent gate (06 §2a): the master switch may have been turned OFF after the job was queued, or
        // the per-domain block added. A denied job is parked (left non-failed) so re-enabling consent
        // simply lets the next drain pick it up.
        if !content_fetch_allowed(vault.config(), &job.payload.url) {
            let _ = mark_running_intelligence_job_cancelled(
                vault,
                job.id,
                "content fetch is disabled for this URL",
            );
            return Ok(ContentFetchJobOutcome::Ran);
        }

        if super::search_text::pages(vault, &job.payload.url)?.is_empty() {
            vault.write(Db::Intelligence, Priority::Interactive, move |s| {
                await_archive(s, job.id)
            })?;
            return Ok(ContentFetchJobOutcome::AwaitingArchive);
        }

        // Page-URL SSRF guard + https-only (06 §2b), BEFORE any rate-limit acquire (SEC-4): a blocked /
        // non-https job must not burn a token, and the stored row must carry the RESOLVED extractor's
        // `content_source` + `extractor_version` so `content_fetch_job_due` matches it and the working set
        // stops re-enqueuing it (a "blocked-content" source would never match → infinite re-enqueue).
        if (fetcher.networked() && crate::test_support::network_is_blocked())
            || !job.payload.url.starts_with("https://")
            || url_target_is_blocked(&job.payload.url)
        {
            let extractor = resolve_extractor(&job.payload.url)
                .unwrap_or_else(|| Box::new(super::extractors::GenericReadableExtractor));
            let blocked = EnrichmentResult {
                status: "blocked".to_string(),
                final_url: Some(job.payload.url.clone()),
                extraction: json!({
                    "extractor": extractor.id(),
                    "error": "The page URL is not https or resolves to a non-public address.",
                }),
                extractor_version: Some(extractor.version() as i64),
                ..EnrichmentResult::default()
            };
            finish_content_fetch_job(vault, &job, extractor.id(), &blocked, None)?;
            return Ok(ContentFetchJobOutcome::Ran);
        }

        let result = run_content_fetch_with_fetcher(fetcher, &job.payload);

        // A throttled egress is BACK-PRESSURE, not a failure (SEC-2): requeue the job for the host's
        // token-refill ETA instead of cancelling it (which the queued-only drain would never re-pick). The
        // deferred row is invisible to `next_queued_content_fetch_job` until its `scheduled_at` passes, so
        // the drain reports idle and the worker lane sleeps until then.
        if result.enrichment.status == "rate-limited" {
            // A UI cancel (`cancel_intelligence_job`, no job-type filter) can land on this STILL-`running`
            // row during the throttled fetch. The requeue below clears `stop_requested` and re-queues, so
            // it would silently swallow that cancel — honour it FIRST (BUG-2): if a stop was requested,
            // cancel the running job and report `Ran` rather than re-running it on the next drain.
            if intelligence_job_stop_requested(vault, job.id)? {
                let _ = mark_running_intelligence_job_cancelled(vault, job.id, "cancelled from UI");
                return Ok(ContentFetchJobOutcome::Ran);
            }
            // Read the deferral ETA from the bucket the attempt ACTUALLY charged (SEC-1) — the API host for
            // a JsonApi extractor (`api.github.com`), NOT the page host (`github.com`, whose separate bucket
            // is always full → 0s ETA → a ~1Hz drain thrash for the whole refill window). `egress_host` is
            // always the real targeted host, so no fallback is needed here (BUG-1).
            let eta_secs = next_token_eta_secs(&result.egress_host).max(1);
            let scheduled_at = (chrono::Utc::now() + chrono::Duration::seconds(eta_secs as i64))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            vault.write(Db::Intelligence, Priority::Interactive, move |s| {
                requeue_content_fetch_job_after(s, job.id, &scheduled_at)
            })?;
            return Ok(ContentFetchJobOutcome::Deferred);
        }

        finish_content_fetch_job(
            vault,
            &job,
            &result.content_source,
            &result.enrichment,
            result.refetch_after.as_deref(),
        )?;
        Ok(ContentFetchJobOutcome::Ran)
    })();
    if let Err(error) = &outcome {
        mark_intelligence_job_failed(vault, job_id, &format!("{error:#}"))?;
    }
    outcome
}

/// Persists one content-fetch outcome + marks the job terminal (succeeded for any stored row).
///
/// A content fetch that returns an honest failure status (`fetch-error`, `blocked`, …) STILL stores a
/// row (the negative-cache marker) and marks the JOB succeeded — the job did its work; the failure is
/// data, not a job error (mirrors og:image's negative-cache rows). This keeps the queue from retry-
/// storming a paywalled host. A cancel requested mid-run still wins.
///
/// Store-time fan-out (CORR-3, 06 §3): the fetch is keyed by URL, so the SAME enrichment is written for
/// EVERY non-reverted visit sharing the page's canonical URL — not just `job.payload.history_id`. That
/// gives the page ONE visit-independent dedup identity (else a sibling visit hashes WITHOUT the summary
/// and the page double-embeds, violating 05 §1). Then the FTS `enrichment_text` projection is refreshed
/// for that URL so a fetched page is immediately findable WITHOUT a full rebuild (CORR-1).
fn finish_content_fetch_job(
    vault: &crate::Vault,
    job: &ClaimedEnrichmentJob,
    content_source: &str,
    enrichment: &EnrichmentResult,
    refetch_after: Option<&str>,
) -> Result<bool> {
    let paths = vault.paths().clone();
    let key = crate::pages::canonical_key(&job.payload.url);
    let source = content_source.to_owned();
    let result = enrichment.clone();
    let refetch = refetch_after.map(str::to_owned);
    let id = job.id;
    let stored=vault.write(Db::Intelligence,Priority::Batch,move |s| {
        if crate::intelligence_runtime::intelligence_job_stop_requested_in_session(s,id)? {
            crate::intelligence_runtime::mark_running_intelligence_job_cancelled_in_session(s,id,"cancelled from UI")?;
            return Ok(false);
        }
        store_enrichment_with_cache(&paths,s,&key,&source,&result,refetch.as_deref())?;
        s.execute("UPDATE intelligence_jobs SET artifact_json=?2,updated_at=?3 WHERE id=?1",params![id,json!({"contentStored":true,"contentSource":source,"status":result.status,"hasSummary":result.enrichment_summary.is_some()}).to_string(),crate::utils::now_rfc3339()])?;
        Ok(true)
    })?;
    if stored {
        publish_stored_content(vault, job, content_source)?;
    }
    Ok(true)
}

/// Replays a committed cache/outbox handoff without fetching the same page again after a crash.
fn publish_stored_content(
    vault: &crate::Vault,
    job: &ClaimedEnrichmentJob,
    source: &str,
) -> Result<()> {
    let pages = super::search_text::pages(vault, &job.payload.url)?;
    if pages.is_empty() {
        let id = job.id;
        vault.write(Db::Intelligence, Priority::Interactive, move |s| await_archive(s, id))?;
        return Ok(());
    }
    let key = crate::pages::canonical_key(&job.payload.url);
    let source = source.to_owned();
    let text=vault.read(Db::Intelligence,move |s| {
        let (summary,metadata)=s.query_one("SELECT enrichment_summary,extraction_json FROM page_content_enrichments WHERE canonical_url=?1 AND content_source=?2",params![key,source],|r|Ok((r.get::<Option<String>>(0)?,r.get::<String>(1)?)))?;
        Ok(crate::archive::enrichment_text_for_index(summary.as_deref(),Some(&metadata)))
    })?;
    super::search_text::publish(vault, pages, text)?;
    let id = job.id;
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        let artifact =
            s.query_one("SELECT artifact_json FROM intelligence_jobs WHERE id=?1", [id], |r| {
                Ok(r.get::<String>(0)?)
            })?;
        let artifact = serde_json::from_str(&artifact)?;
        if !crate::intelligence_runtime::mark_intelligence_job_succeeded_in_session(
            s, id, &artifact,
        )? {
            crate::intelligence_runtime::mark_running_intelligence_job_cancelled_in_session(
                s,
                id,
                "cancelled from UI",
            )?;
        }
        Ok(())
    })
}

fn await_archive(connection: &Session<'_>, job: i64) -> Result<()> {
    let artifact = connection.query_one(
        "SELECT artifact_json FROM intelligence_jobs WHERE id=?1",
        [job],
        |r| Ok(r.get::<String>(0)?),
    )?;
    let mut artifact: serde_json::Value = serde_json::from_str(&artifact)?;
    artifact["code"] = json!("archive-page-missing");
    connection.execute("UPDATE intelligence_jobs SET state='awaiting_archive',finished_at=?2,updated_at=?2,lease_owner=NULL,lease_expires_at=NULL,last_error=NULL,artifact_json=?3 WHERE id=?1 AND state='running'",params![job,crate::utils::now_rfc3339(),artifact.to_string()])?;
    Ok(())
}

/// Production fetcher backed by the shared `build_fetch_client()` egress (06 §2b).
///
/// Performs the real socket fetch through the ONE shared client (desktop Chrome UA, no Referer/cookies,
/// redirect/timeout limited). The HTML body is content-type + size guarded; JSON is size guarded. The
/// `fetch_html`/`fetch_json` methods are exercised by mockito acceptance tests (mirroring og:image's
/// pattern) so the real socket path is covered. Only the `execute_content_fetch_job_by_id` ENTRY that
/// targets real hosts is cfg-gated out of coverage (it cannot be redirected to mockito).
pub(crate) struct SharedClientFetcher {
    client: crate::og_images_fetch::FetchClient,
}

impl SharedClientFetcher {
    pub(crate) fn new() -> Result<Self> {
        // The shared client checks every redirect hop like the first URL, so a public page that
        // redirects to a private host is stopped there (SEC-3).
        Ok(Self { client: crate::og_images_fetch::build_fetch_client()? })
    }
}

impl Fetcher for SharedClientFetcher {
    fn fetch_html(&self, url: &str) -> FetchOutcome {
        if crate::test_support::network_is_blocked() {
            return FetchOutcome::Failed {
                status: "blocked".to_string(),
                http_status: None,
                detail: "The page URL is not https or resolves to a non-public address."
                    .to_string(),
            };
        }
        use reqwest::header::{ACCEPT, CONTENT_TYPE};
        let response = match self.client.get(url).header(ACCEPT, "text/html").send() {
            Ok(response) => response,
            Err(error) => {
                return FetchOutcome::Failed {
                    status: "fetch-error".to_string(),
                    http_status: error.status().map(|status| i64::from(status.as_u16())),
                    detail: format!("Could not fetch the page. {error}"),
                };
            }
        };
        let http_status = i64::from(response.status().as_u16());
        if response.status().as_u16() == 429 {
            return FetchOutcome::Failed {
                status: "rate-limited".to_string(),
                http_status: Some(http_status),
                detail: "The host rate-limited the request (HTTP 429).".to_string(),
            };
        }
        if !response.status().is_success() {
            return FetchOutcome::Failed {
                status: "fetch-error".to_string(),
                http_status: Some(http_status),
                detail: format!("The host returned HTTP {http_status}."),
            };
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or("").trim().to_ascii_lowercase());
        if content_type.as_deref() != Some("text/html") {
            return FetchOutcome::Failed {
                status: "unsupported-content".to_string(),
                http_status: Some(http_status),
                detail: format!(
                    "The page returned non-HTML content ({}).",
                    content_type.as_deref().unwrap_or("unknown")
                ),
            };
        }
        let final_url = Some(response.url().to_string());
        match read_capped(response, MAX_HTML_BODY_BYTES) {
            Ok(bytes) => FetchOutcome::Body { bytes, content_type, final_url, http_status },
            Err(detail) => FetchOutcome::Failed {
                status: "decode-error".to_string(),
                http_status: Some(http_status),
                detail,
            },
        }
    }

    fn fetch_json(&self, url: &str, cap: usize) -> FetchOutcome {
        if crate::test_support::network_is_blocked() {
            return FetchOutcome::Failed {
                status: "blocked".to_string(),
                http_status: None,
                detail: "The page URL is not https or resolves to a non-public address."
                    .to_string(),
            };
        }
        use reqwest::header::ACCEPT;
        let response = match self.client.get(url).header(ACCEPT, "application/json").send() {
            Ok(response) => response,
            Err(error) => {
                return FetchOutcome::Failed {
                    status: "fetch-error".to_string(),
                    http_status: error.status().map(|status| i64::from(status.as_u16())),
                    detail: format!("Could not fetch the API resource. {error}"),
                };
            }
        };
        let http_status = i64::from(response.status().as_u16());
        if response.status().as_u16() == 429 || response.status().as_u16() == 403 {
            // GitHub signals an exhausted unauthenticated quota with 403 (rate-limit) or 429.
            return FetchOutcome::Failed {
                status: "rate-limited".to_string(),
                http_status: Some(http_status),
                detail: format!("The API rate-limited the request (HTTP {http_status})."),
            };
        }
        if !response.status().is_success() {
            return FetchOutcome::Failed {
                status: "fetch-error".to_string(),
                http_status: Some(http_status),
                detail: format!("The API returned HTTP {http_status}."),
            };
        }
        let final_url = Some(response.url().to_string());
        match read_capped(response, cap) {
            Ok(bytes) => FetchOutcome::Body {
                bytes,
                content_type: Some("application/json".to_string()),
                final_url,
                http_status,
            },
            Err(detail) => FetchOutcome::Failed {
                status: "decode-error".to_string(),
                http_status: Some(http_status),
                detail,
            },
        }
    }
}

/// Reads a body stream, capping at `cap` bytes (over-cap → size error, broken stream → read error).
///
/// Generic over [`std::io::Read`] (a `reqwest::blocking::Response` IS a `Read`) so the read-error arm —
/// a connection broken mid-body, which mockito cannot deterministically produce — is covered with a
/// fake failing reader rather than being hidden behind a cfg stub.
fn read_capped<R: std::io::Read>(
    mut reader: R,
    cap: usize,
) -> std::result::Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                if buffer.len() + read > cap {
                    return Err("The response body exceeded the size cap.".to_string());
                }
                buffer.extend_from_slice(&chunk[..read]);
            }
            Err(error) => return Err(format!("The response body could not be read. {error}")),
        }
    }
    Ok(buffer)
}

#[cfg(test)]
pub(super) mod tests;
