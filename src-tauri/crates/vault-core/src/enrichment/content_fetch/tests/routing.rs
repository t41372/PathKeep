//! Responsibilities: the consent gates checked before any request, and the check of where a
//! redirect finally landed.
//! Not responsible for: other test groups or production code.
//! Failure modes: a request without consent, or a private page's body kept after a redirect.
use super::*;

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
fn guard_final_url_blocks_a_private_redirect_landing() {
    // Defence-in-depth: a body whose post-redirect final_url is private becomes a `blocked`
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
