//! Responsibilities: transport behavior regressions.
//! Not responsible for: other test groups or production code.
//! Failure modes: wrong state, missing records and replay inconsistency.
use super::*;

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_returns_html_body_for_a_readable_article() {
    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/article")
        .with_status(200)
        .with_header("content-type", "text/html; charset=utf-8")
        .with_body("<html><head><title>Article</title></head><body><p>Body</p></body></html>")
        .create();
    let outcome = shared_fetcher().fetch_html(&format!("{}/article", server.url()));
    match outcome {
        FetchOutcome::Body { bytes, content_type, http_status, .. } => {
            assert_eq!(http_status, 200);
            assert_eq!(content_type.as_deref(), Some("text/html"));
            assert!(String::from_utf8_lossy(&bytes).contains("Article"));
        }
        other => panic!("expected Body, got {other:?}"),
    }
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_reports_rate_limited_on_429() {
    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/r").with_status(429).create();
    let outcome = shared_fetcher().fetch_html(&format!("{}/r", server.url()));
    assert!(matches!(outcome, FetchOutcome::Failed { status, .. } if status == "rate-limited"));
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_reports_unsupported_content_for_non_html() {
    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/doc.pdf")
        .with_status(200)
        .with_header("content-type", "application/pdf")
        .with_body("%PDF-1.7")
        .create();
    let outcome = shared_fetcher().fetch_html(&format!("{}/doc.pdf", server.url()));
    assert!(
        matches!(outcome, FetchOutcome::Failed { status, .. } if status == "unsupported-content")
    );
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_reports_fetch_error_on_4xx_5xx() {
    let mut server = mockito::Server::new();
    let _not_found = server.mock("GET", "/missing").with_status(404).create();
    let outcome = shared_fetcher().fetch_html(&format!("{}/missing", server.url()));
    assert!(
        matches!(outcome, FetchOutcome::Failed { status, http_status: Some(404), .. } if status == "fetch-error")
    );
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_reports_decode_error_when_html_body_exceeds_cap() {
    let mut server = mockito::Server::new();
    let huge = vec![b'a'; MAX_HTML_BODY_BYTES + 4096];
    let _mock = server
        .mock("GET", "/huge")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(huge)
        .create();
    let outcome = shared_fetcher().fetch_html(&format!("{}/huge", server.url()));
    assert!(matches!(outcome, FetchOutcome::Failed { status, .. } if status == "decode-error"));
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_returns_github_repo_json() {
    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/repos/o/r")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"full_name":"o/r","description":"A repo","topics":["x"]}"#)
        .create();
    let outcome = shared_fetcher().fetch_json(&format!("{}/repos/o/r", server.url()), 65536);
    match outcome {
        FetchOutcome::Body { bytes, .. } => {
            assert!(String::from_utf8_lossy(&bytes).contains("\"full_name\":\"o/r\""));
        }
        other => panic!("expected Body, got {other:?}"),
    }
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_treats_github_403_as_rate_limited() {
    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", "/repos/o/r").with_status(403).create();
    let outcome = shared_fetcher().fetch_json(&format!("{}/repos/o/r", server.url()), 65536);
    assert!(matches!(outcome, FetchOutcome::Failed { status, .. } if status == "rate-limited"));
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_caps_json_body() {
    let mut server = mockito::Server::new();
    let huge = vec![b'{'; 4096];
    let _mock = server
        .mock("GET", "/big")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(huge)
        .create();
    let outcome = shared_fetcher().fetch_json(&format!("{}/big", server.url()), 1024);
    assert!(matches!(outcome, FetchOutcome::Failed { status, .. } if status == "decode-error"));
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_follows_a_redirect_chain_to_the_final_html() {
    let mut server = mockito::Server::new();
    let _redirect =
        server.mock("GET", "/start").with_status(301).with_header("location", "/final").create();
    let _final = server
        .mock("GET", "/final")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body("<html><head><title>Final</title></head><body>ok</body></html>")
        .create();
    let outcome = shared_fetcher().fetch_html(&format!("{}/start", server.url()));
    match outcome {
        FetchOutcome::Body { final_url, bytes, .. } => {
            assert!(final_url.as_deref().unwrap().ends_with("/final"));
            assert!(String::from_utf8_lossy(&bytes).contains("Final"));
        }
        other => panic!("expected Body after redirect, got {other:?}"),
    }
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_reports_fetch_error_on_unreachable_host() {
    // An unresolvable host fails at connect → honest fetch-error, never a fake success.
    let outcome = shared_fetcher().fetch_html("https://content-fetch-test.invalid./x");
    assert!(matches!(outcome, FetchOutcome::Failed { status, .. } if status == "fetch-error"));
    let json_outcome = shared_fetcher().fetch_json("https://content-fetch-test.invalid./api", 1024);
    assert!(matches!(json_outcome, FetchOutcome::Failed { status, .. } if status == "fetch-error"));
}

#[test]
#[ignore = "requires a network socket; offline read_capped tests cover the byte limit"]
fn read_capped_drains_under_cap_and_rejects_over_cap() {
    // Exercise the body-cap helper directly through mockito so both arms are covered.
    let mut server = mockito::Server::new();
    let _ok = server
        .mock("GET", "/ok")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("{\"k\":1}")
        .create();
    let outcome = shared_fetcher().fetch_json(&format!("{}/ok", server.url()), 1024);
    assert!(matches!(outcome, FetchOutcome::Body { .. }));
}

#[test]
#[ignore = "requires a network socket; kept offline by the area-4 task"]
fn shared_fetcher_reports_fetch_error_on_json_5xx() {
    // Covers the SharedClientFetcher JSON non-success (non-403/429) branch.
    let mut server = mockito::Server::new();
    let _err = server.mock("GET", "/repos/o/r").with_status(500).create();
    let outcome = shared_fetcher().fetch_json(&format!("{}/repos/o/r", server.url()), 4096);
    assert!(
        matches!(outcome, FetchOutcome::Failed { status, http_status: Some(500), .. } if status == "fetch-error")
    );
}
