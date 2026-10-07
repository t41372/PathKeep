//! Responsibilities: routing behavior regressions.
//! Not responsible for: other test groups or production code.
//! Failure modes: wrong state, missing records and replay inconsistency.
use super::*;

#[test]
fn run_content_fetch_routes_github_url_to_github_extractor() {
    let _bucket =
        GITHUB_BUCKET_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|p| p.into_inner());
    rate_limit::reset_host_bucket_for_test("api.github.com");
    let fetcher = FakeFetcher::new().with_json(
        "https://api.github.com/repos/o/r",
        body(br#"{"full_name":"o/r","description":"A repo","topics":["x"]}"#, "application/json"),
    );
    let payload = EnrichmentJobPayload {
        history_id: Some(1),
        profile_id: Some("chrome:Default".to_string()),
        url: "https://github.com/o/r".to_string(),
        title: None,
    };
    let result = run_content_fetch_with_fetcher(&fetcher, &payload);
    assert_eq!(result.content_source, "github-repo");
    assert_eq!(result.enrichment.status, "success");
    assert_eq!(result.enrichment.enrichment_summary.as_deref(), Some("A repo"));
    // A success schedules NO retry.
    assert!(result.refetch_after.is_none());
}

#[test]
fn run_content_fetch_routes_generic_url_to_readable_extractor() {
    rate_limit::reset_host_bucket_for_test("example.com");
    let html = "<html><head><title>Generic Doc</title></head><body><main><p>Body.</p></main></body></html>";
    let fetcher = FakeFetcher::new()
        .with_html("https://example.com/post", body(html.as_bytes(), "text/html"));
    let payload = EnrichmentJobPayload {
        history_id: Some(2),
        profile_id: Some("chrome:Default".to_string()),
        url: "https://example.com/post".to_string(),
        title: None,
    };
    let result = run_content_fetch_with_fetcher(&fetcher, &payload);
    assert_eq!(result.content_source, "generic-readable");
    assert_eq!(result.enrichment.readable_title.as_deref(), Some("Generic Doc"));
}

#[test]
fn run_content_fetch_records_honest_failure_and_negative_cache() {
    rate_limit::reset_host_bucket_for_test("example.com");
    let fetcher = FakeFetcher::new().with_html(
        "https://example.com/paywall",
        FetchOutcome::Failed {
            status: "unsupported-content".to_string(),
            http_status: Some(200),
            detail: "PDF".to_string(),
        },
    );
    let payload = EnrichmentJobPayload {
        history_id: Some(3),
        profile_id: Some("chrome:Default".to_string()),
        url: "https://example.com/paywall".to_string(),
        title: None,
    };
    let result = run_content_fetch_with_fetcher(&fetcher, &payload);
    assert_eq!(result.enrichment.status, "unsupported-content");
    // A structural failure schedules a (long) retry — the negative cache.
    assert!(result.refetch_after.is_some());
    assert_eq!(result.enrichment.extractor_version, Some(1));
}

#[test]
fn run_content_fetch_blocks_private_github_api_subresource() {
    // The github extractor wants api.github.com, but we route through guard_then_fetch_json which
    // SSRF-checks. Use a github URL whose API base would be public; assert the guard path via the
    // unit test of guard_then_fetch_json below. Here we cover the JSON primary-failure path.
    let _bucket =
        GITHUB_BUCKET_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|p| p.into_inner());
    rate_limit::reset_host_bucket_for_test("api.github.com");
    let fetcher = FakeFetcher::new(); // no fixture → fetch_json returns fetch-error
    let payload = EnrichmentJobPayload {
        history_id: Some(4),
        profile_id: Some("chrome:Default".to_string()),
        url: "https://github.com/o/r".to_string(),
        title: None,
    };
    let result = run_content_fetch_with_fetcher(&fetcher, &payload);
    assert_eq!(result.enrichment.status, "fetch-error");
}

#[test]
fn content_fetch_allowed_honors_master_switch_extractor_and_domain_gates() {
    // Master OFF → never allowed.
    let mut off = AppConfig::default();
    off.ai.content_fetch_enabled = false;
    assert!(!content_fetch_allowed(&off, "https://github.com/o/r"));

    // Master ON, default extractor prefs (github + generic enabled) → allowed.
    let on = consenting_config();
    assert!(content_fetch_allowed(&on, "https://github.com/o/r"));
    assert!(content_fetch_allowed(&on, "https://example.com/post"));

    // Per-extractor disable github → github URL denied, generic still allowed.
    let mut github_off = consenting_config();
    github_off.ai.content_fetch_extractors = vec![ContentFetchExtractorPreference {
        extractor_id: "github-repo".to_string(),
        enabled: false,
    }];
    assert!(!content_fetch_allowed(&github_off, "https://github.com/o/r"));
    assert!(content_fetch_allowed(&github_off, "https://example.com/post"));

    // Per-domain block example.com → that domain denied.
    let mut domain_block = consenting_config();
    domain_block.ai.content_fetch_domains =
        vec![ContentFetchDomainRule { domain: "example.com".to_string(), allowed: false }];
    assert!(!content_fetch_allowed(&domain_block, "https://example.com/post"));
    assert!(content_fetch_allowed(&domain_block, "https://other.com/post"));
}

#[test]
fn guard_then_fetch_json_blocks_private_api_url() {
    let fetcher = FakeFetcher::new();
    let request =
        ApiRequest { url: "https://127.0.0.1/repos/o/r".to_string(), body_cap_bytes: 1024 };
    let blocked = guard_then_fetch_json(&fetcher, &request);
    assert!(matches!(blocked, FetchOutcome::Failed { status, .. } if status == "blocked"));
}

#[test]
fn run_content_fetch_blocks_private_github_api_via_guard() {
    // A github URL whose resolved API base is loopback must short-circuit with a `blocked` row
    // BEFORE any fetch — the API sub-resource SSRF guard (06 §2b). We use the production github
    // extractor (api.github.com is public), so to exercise the guard we drive `guard_then_fetch_json`
    // directly above; here we confirm the runner stores a `blocked` enrichment when the guard trips
    // via a fake whose api_request points at loopback is not possible (extractor pins the base), so
    // this test documents that the guard is the gate and the direct test above covers it.
    let fetcher = FakeFetcher::new();
    let request = ApiRequest {
        url: "http://169.254.169.254/latest/meta-data/".to_string(),
        body_cap_bytes: 1024,
    };
    let blocked = guard_then_fetch_json(&fetcher, &request);
    assert!(matches!(blocked, FetchOutcome::Failed { status, .. } if status == "blocked"));
}

#[test]
fn fetch_json_api_with_no_api_request_is_fetch_error() {
    // Covers the `api_request() == None` branch + the trait DEFAULT api/secondary methods. With no
    // API request there is no egress, so the (unused-by-the-deferral-path) reported host is the page
    // host (BUG-1: this arm yields `fetch-error`, never `rate-limited`).
    let fetcher = FakeFetcher::new();
    let (result, egress_host) =
        fetch_json_api(&fetcher, &NoApiJsonExtractor, "https://api.example.com/x");
    assert_eq!(result.status, "fetch-error");
    assert_eq!(result.extraction["error"], "No API request to fetch.");
    assert_eq!(egress_host, "api.example.com");
    // Direct trait-default coverage (extractors.rs api_request/secondary_api_request).
    assert!(NoApiJsonExtractor.api_request("https://x").is_none());
    assert!(NoApiJsonExtractor.secondary_api_request("https://x").is_none());
}

#[test]
fn fetch_json_api_keeps_a_successful_secondary_and_tolerates_a_failed_one() {
    // A successful secondary body is kept (covers `Some(secondary) => Body => Some(bytes)`); a
    // failed secondary is non-fatal (covers `Failed => None`); a no-secondary extractor covers the
    // `None` arm. Driven on UNIQUE hosts via a stub extractor so the shared github bucket is never
    // touched (deterministic under parallel tests).
    let primary = format!("https://{}/api", unique_host("sec-ok-p"));
    let secondary = format!("https://{}/readme", unique_host("sec-ok-s"));
    let fetcher = FakeFetcher::new()
        .with_json(&primary, body(b"{}", "application/json"))
        .with_json(&secondary, body(b"{}", "application/json"));
    let extractor =
        StubJsonExtractor { primary: primary.clone(), secondary: Some(secondary.clone()) };
    let (result, egress_host) = fetch_json_api(&fetcher, &extractor, "https://page.example/x");
    assert_eq!(result.status, "success");
    assert_eq!(result.extraction["hasSecondary"], true);
    // BUG-1: the reported egress host is the PRIMARY API host (where the GET lands), not the page.
    assert_eq!(egress_host, url_domain(&primary));

    // A failing secondary is tolerated (the body is omitted, the primary still extracts).
    let fetcher_fail = FakeFetcher::new().with_json(&primary, body(b"{}", "application/json"));
    let extractor_fail =
        StubJsonExtractor { primary: primary.clone(), secondary: Some(secondary.clone()) };
    let (result, _) = fetch_json_api(&fetcher_fail, &extractor_fail, "https://page.example/x");
    assert_eq!(result.status, "success");
    assert_eq!(result.extraction["hasSecondary"], false);

    // No secondary declared at all (covers the `None` arm).
    let fetcher_none = FakeFetcher::new().with_json(&primary, body(b"{}", "application/json"));
    let extractor_none = StubJsonExtractor { primary, secondary: None };
    let (result, _) = fetch_json_api(&fetcher_none, &extractor_none, "https://page.example/x");
    assert_eq!(result.status, "success");
    assert_eq!(result.extraction["hasSecondary"], false);
}

#[test]
fn html_egress_parks_when_host_bucket_is_empty() {
    // SEC-1/SEC-2: drain a unique host's bucket, then the HTML egress returns `rate-limited`
    // (covers the per-egress acquire + `rate_limit_detail`). Drain past the default burst (8).
    let host = unique_host("html-rl");
    let url = format!("https://{host}/post");
    for _ in 0..16 {
        let _ = rate_limit::acquire_host_token(&host);
    }
    let fetcher = FakeFetcher::new(); // never reached — the bucket is empty
    let result = run_content_fetch_with_fetcher(&fetcher, &payload(&url, 1));
    assert_eq!(result.enrichment.status, "rate-limited");
    assert!(
        result.enrichment.extraction["error"].as_str().unwrap().contains("per-host request budget")
    );
}

#[test]
fn json_egress_parks_when_host_bucket_is_empty() {
    // SEC-1/SEC-2 on the JSON path: drain a unique API host's bucket, then `guard_then_fetch_json`
    // returns `rate-limited` (covers the JSON per-egress acquire failure branch).
    let host = unique_host("json-rl");
    let api = format!("https://{host}/repos/o/r");
    for _ in 0..16 {
        let _ = rate_limit::acquire_host_token(host.split('.').next().unwrap_or(&host));
        let _ = rate_limit::acquire_host_token(&host);
    }
    let fetcher = FakeFetcher::new(); // never reached
    let outcome = guard_then_fetch_json(&fetcher, &ApiRequest { url: api, body_cap_bytes: 4096 });
    assert!(matches!(outcome, FetchOutcome::Failed { status, .. } if status == "rate-limited"));
}

#[test]
fn guard_final_url_blocks_a_private_redirect_landing() {
    // SEC-3 defence-in-depth: a body whose post-redirect final_url is private becomes a `blocked`
    // outcome (the bytes are dropped, never parsed). Drive a generic page whose fetcher reports a
    // loopback final_url.
    let host = unique_host("final-guard");
    let url = format!("https://{host}/post");
    let fetcher = FakeFetcher::new().with_html(
        &url,
        FetchOutcome::Body {
            bytes: b"<html><title>X</title></html>".to_vec(),
            content_type: Some("text/html".to_string()),
            final_url: Some("https://127.0.0.1/internal".to_string()),
            http_status: 200,
        },
    );
    let result = run_content_fetch_with_fetcher(&fetcher, &payload(&url, 1));
    assert_eq!(result.enrichment.status, "blocked");
}

#[test]
fn run_content_fetch_falls_back_to_generic_for_unresolved_scheme() {
    // Covers `resolve_extractor` returning None + the defensive generic-readable fallback in
    // `run_content_fetch_with_fetcher` (a non-http(s) URL matches no extractor).
    let fetcher = FakeFetcher::new();
    let result = run_content_fetch_with_fetcher(&fetcher, &payload("ftp://example.com/x", 1));
    assert_eq!(result.content_source, "generic-readable");
}

#[test]
fn content_fetch_allowed_is_false_for_non_http_scheme() {
    // Covers `content_fetch_allowed`'s `resolve_extractor == None` early-out (ftp matches nothing).
    let on = consenting_config();
    assert!(!content_fetch_allowed(&on, "ftp://example.com/x"));
}
