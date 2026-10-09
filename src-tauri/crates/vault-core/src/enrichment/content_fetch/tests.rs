//! Responsibilities: retain the parent module's behavior regressions on native storage.
//! Not responsible for: product implementation or external network execution.
//! Failure modes: incorrect boundaries, lost records, replay drift and invalid state transitions.
use super::*;
use crate::test_support::native::TestDatabase;
use vault_store::{Db, Priority};

use crate::enrichment::rate_limit;
use crate::intelligence_runtime::{claim_content_fetch_job_by_id, enqueue_content_fetch_job};
use crate::models::{AppConfig, ContentFetchDomainRule, ContentFetchExtractorPreference};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Process-global lock serializing tests that touch the SHARED `api.github.com` bucket.
///
/// That host's bucket is process-global, so a test that DRAINS it (the BUG-1 deferral test) would
/// otherwise race with the full-bucket github routing tests running in parallel. Each github-bucket
/// test holds `GITHUB_BUCKET_LOCK` for its duration (acquired INLINE, never via a helper returning a
/// `'static`-annotated guard — the coverage verifier's test-block masker mishandles a `'` lifetime
/// token as a char literal) so they never overlap; tests on unique hosts need no lock.
static GITHUB_BUCKET_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// A scripted fetcher: maps a URL to a canned outcome so every routing branch is deterministic.
struct FakeFetcher {
    html: HashMap<String, FetchOutcome>,
    json: Mutex<HashMap<String, FetchOutcome>>,
}

impl FakeFetcher {
    fn new() -> Self {
        Self { html: HashMap::new(), json: Mutex::new(HashMap::new()) }
    }
    fn with_html(mut self, url: &str, outcome: FetchOutcome) -> Self {
        self.html.insert(url.to_string(), outcome);
        self
    }
    fn with_json(self, url: &str, outcome: FetchOutcome) -> Self {
        self.json.lock().unwrap().insert(url.to_string(), outcome);
        self
    }
}

impl Fetcher for FakeFetcher {
    fn networked(&self) -> bool {
        false
    }
    fn fetch_html(&self, url: &str) -> FetchOutcome {
        self.html.get(url).cloned().unwrap_or(FetchOutcome::Failed {
            status: "fetch-error".to_string(),
            http_status: None,
            detail: "no fixture".to_string(),
        })
    }
    fn fetch_json(&self, url: &str, _cap: usize) -> FetchOutcome {
        self.json.lock().unwrap().get(url).cloned().unwrap_or(FetchOutcome::Failed {
            status: "fetch-error".to_string(),
            http_status: None,
            detail: "no fixture".to_string(),
        })
    }
}

fn consenting_config() -> AppConfig {
    let mut config = AppConfig { initialized: true, ..AppConfig::default() };
    config.ai.content_fetch_enabled = true;
    config
}

fn body(bytes: &[u8], content_type: &str) -> FetchOutcome {
    FetchOutcome::Body {
        bytes: bytes.to_vec(),
        content_type: Some(content_type.to_string()),
        final_url: None,
        http_status: 200,
    }
}

// ── SharedClientFetcher real-socket acceptance (mockito, mirrors og:image) ───────────────────

/// The production fetcher, except that redirect hops may go to the loopback mock server; the real
/// hop check is tested by `the_production_fetcher_does_not_follow_a_redirect_to_a_private_address`.
fn shared_fetcher() -> SharedClientFetcher {
    SharedClientFetcher {
        client: crate::og_images_fetch::build_guarded_fetch_client(|_| false).expect("client"),
    }
}

#[test]
fn network_switch_blocks_content_fetch_without_connecting() {
    if !cfg!(debug_assertions) {
        return;
    }
    if !crate::test_support::network_test_child() {
        crate::test_support::run_network_test(
            "enrichment::content_fetch::tests::network_switch_blocks_content_fetch_without_connecting",
            Some("1"),
        );
        return;
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    listener.set_nonblocking(true).expect("nonblocking listener");
    let url = format!("http://{}/article", listener.local_addr().unwrap());
    let fetcher = shared_fetcher();
    for outcome in [fetcher.fetch_html(&url), fetcher.fetch_json(&url, 1024)] {
        match outcome {
            FetchOutcome::Failed { status, http_status, .. } => {
                assert_eq!(status, "blocked");
                assert!(http_status.is_none());
            }
            other => panic!("expected blocked, got {other:?}"),
        }
    }
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);

    // Even an exhausted API budget must produce a terminal blocked result,
    // rather than deferring a job that will never be allowed to fetch.
    for _ in 0..100 {
        let _ = rate_limit::acquire_host_token("api.github.com");
    }
    let result = run_content_fetch_with_fetcher(&fetcher, &payload("https://github.com/o/r", 1));
    assert_eq!(result.content_source, "github-repo");
    assert_eq!(result.enrichment.status, "blocked");
    assert!(result.refetch_after.is_none());

    let vault = crate::Vault::open_temp().unwrap();
    let _paths = vault.paths().clone();
    let connection = TestDatabase::new(&vault, Db::Intelligence);
    let job_id =
        enqueue_native(&connection, &payload("https://github.com/o/r", 1)).expect("enqueue");
    assert_eq!(
        execute_content_fetch_job_with_fetcher(
            &{
                seed_page(&vault, "https://github.com/o/r");
                vault.with_config(consenting_config().clone())
            },
            job_id,
            &fetcher
        )
        .expect("execute"),
        ContentFetchJobOutcome::Ran,
    );
    let state: String = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            Ok(row.get(0)?)
        })
        .expect("job state");
    assert_eq!(state, "succeeded");
    let (status, refetch): (String, Option<String>) = connection.query_row(
        "SELECT fetch_status, refetch_after FROM page_content_enrichments WHERE canonical_url = 'https://github.com/o/r' AND content_source = 'github-repo'", (),
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).expect("stored blocked result");
    assert_eq!(status, "blocked");
    assert!(refetch.is_none());
}

// ── W-ENRICH-1 fix coverage (SEC-1/2/3/4, CORR-1/3, COV-1) ───────────────────────────────────

/// A test-only `JsonApi` extractor that uses the DEFAULT `api_request`/`secondary_api_request`
/// (both `None`) so the "no API request to fetch" branch + the trait defaults are covered.
struct NoApiJsonExtractor;
impl Extractor for NoApiJsonExtractor {
    fn id(&self) -> &'static str {
        "no-api-json"
    }
    fn version(&self) -> u32 {
        1
    }
    fn matches(&self, _url: &str) -> bool {
        true
    }
    fn fetch_kind(&self) -> ExtractKind {
        ExtractKind::JsonApi
    }
    fn extract(&self, _ctx: &ExtractContext) -> EnrichmentResult {
        EnrichmentResult { status: "success".to_string(), ..EnrichmentResult::default() }
    }
}

/// A test-only `JsonApi` extractor with a configurable primary + optional secondary on any host, so
/// the secondary-present/absent + per-egress-host paths are exercised WITHOUT the shared
/// `api.github.com` bucket (which parallel github tests would race on).
struct StubJsonExtractor {
    primary: String,
    secondary: Option<String>,
}
impl Extractor for StubJsonExtractor {
    fn id(&self) -> &'static str {
        "stub-json"
    }
    fn version(&self) -> u32 {
        1
    }
    fn matches(&self, _url: &str) -> bool {
        true
    }
    fn fetch_kind(&self) -> ExtractKind {
        ExtractKind::JsonApi
    }
    fn api_request(&self, _url: &str) -> Option<ApiRequest> {
        Some(ApiRequest { url: self.primary.clone(), body_cap_bytes: 4096 })
    }
    fn secondary_api_request(&self, _url: &str) -> Option<ApiRequest> {
        self.secondary.as_ref().map(|url| ApiRequest { url: url.clone(), body_cap_bytes: 4096 })
    }
    fn extract(&self, ctx: &ExtractContext) -> EnrichmentResult {
        EnrichmentResult {
            status: "success".to_string(),
            enrichment_summary: Some("stub".to_string()),
            extraction: json!({ "hasSecondary": ctx.secondary_body.is_some() }),
            extractor_version: Some(1),
            ..EnrichmentResult::default()
        }
    }
}

/// A unique per-test host so the process-global rate-limit registry never leaks between tests.
fn unique_host(tag: &str) -> String {
    format!("wenrich-{tag}-{}.example", std::process::id())
}

fn payload(url: &str, history_id: i64) -> EnrichmentJobPayload {
    EnrichmentJobPayload {
        history_id: Some(history_id),
        profile_id: Some("chrome:Default".to_string()),
        url: url.to_string(),
        title: None,
    }
}

#[test]
fn read_capped_handles_under_cap_over_cap_and_a_broken_stream() {
    use std::io::{Error, ErrorKind, Read};

    // A reader that returns some bytes, then errors — stands in for a connection broken mid-body
    // (which mockito cannot deterministically produce). Covers the read-error arm of `read_capped`.
    struct FlakyReader {
        served: bool,
    }
    impl Read for FlakyReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.served {
                return Err(Error::new(ErrorKind::ConnectionReset, "stream broke"));
            }
            self.served = true;
            buf[..3].copy_from_slice(b"abc");
            Ok(3)
        }
    }
    let broken = read_capped(FlakyReader { served: false }, 1024);
    assert!(broken.is_err(), "a broken stream must surface a read error");
    assert!(broken.unwrap_err().contains("could not be read"));

    // Under-cap reads succeed; over-cap is rejected (both arms via in-memory slices).
    assert_eq!(read_capped(&b"hello"[..], 1024).expect("under cap"), b"hello".to_vec());
    assert!(read_capped(&b"too-long-body"[..], 4).is_err(), "over-cap must be rejected");
}

/// A `Fetcher` that, when invoked, sets `stop_requested = 1` on a job row (modelling a UI cancel
/// that lands DURING the fetch) and then returns a `rate-limited` outcome. Used to drive the BUG-2
/// concurrent-stop branch deterministically single-threaded: the side-effect runs after the claim
/// (which cleared the flag) and before the rate-limited requeue reads it.
struct StopRequestingFetcher {
    vault: crate::Vault,
    job_id: i64,
}
impl StopRequestingFetcher {
    fn request_stop(&self) {
        let connection = TestDatabase::new(&self.vault, Db::Intelligence);
        connection
            .execute("UPDATE intelligence_jobs SET stop_requested = 1 WHERE id = ?1", [self.job_id])
            .expect("set stop");
    }
    fn rate_limited() -> FetchOutcome {
        FetchOutcome::Failed {
            status: "rate-limited".to_string(),
            http_status: Some(429),
            detail: "the host rate-limited the request".to_string(),
        }
    }
}
impl Fetcher for StopRequestingFetcher {
    fn networked(&self) -> bool {
        false
    }
    fn fetch_html(&self, _url: &str) -> FetchOutcome {
        self.request_stop();
        Self::rate_limited()
    }
    fn fetch_json(&self, _url: &str, _cap: usize) -> FetchOutcome {
        self.request_stop();
        Self::rate_limited()
    }
}

fn seed_page(vault: &crate::Vault, url: &str) {
    let url = url.to_owned();
    vault.write(Db::Archive,Priority::Batch,move |s| {
        let id=s.query_one("INSERT INTO pages(url,canonical_url) VALUES(?1,?2) ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",params![url,crate::pages::canonical_key(&url)],|r|Ok(r.get::<i64>(0)?))?;
        crate::archive::search_documents::text_changed(s,id)?;Ok(())
    }).unwrap();
}
fn enqueue_native(db: &TestDatabase, payload: &EnrichmentJobPayload) -> anyhow::Result<i64> {
    let payload = payload.clone();
    db.write(move |s| enqueue_content_fetch_job(s, &payload))
}
fn claim_native(db: &TestDatabase, id: i64) -> anyhow::Result<Option<ClaimedEnrichmentJob>> {
    db.write(move |s| claim_content_fetch_job_by_id(s, id))
}

fn seed_run_profile(archive: &TestDatabase) {
    archive
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', '2026-01-01', 'UTC', 'success', '[]', '[]', '{}', 0)",
            (),
        )
        .expect("run");
    archive
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (1, 'chrome', 'x', 'p', '/p', '2026-01-01', 1, 'chrome:Default', '2026-01-01')",
            (),
        )
        .expect("profile");
}

mod transport;

mod storage;

mod routing;
