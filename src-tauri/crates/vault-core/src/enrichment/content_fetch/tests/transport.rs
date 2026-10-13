//! Responsibilities: the production fetcher's per-hop redirect guard.
//! Not responsible for: other test groups or production code.
//! Failure modes: a redirect followed to a private address.
use super::*;

#[test]
fn the_production_fetcher_does_not_follow_a_redirect_to_a_private_address() {
    // The real hop check: the mock server is on loopback, so its redirect points at a private
    // address, and the page behind it must never be requested.
    let mut server = mockito::Server::new();
    let _start =
        server.mock("GET", "/start").with_status(302).with_header("location", "/internal").create();
    let internal = server
        .mock("GET", "/internal")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body("<html><body>internal</body></html>")
        .expect(0)
        .create();
    let outcome =
        SharedClientFetcher::new().expect("fetcher").fetch_html(&format!("{}/start", server.url()));
    assert!(!matches!(outcome, FetchOutcome::Body { .. }), "{outcome:?}");
    internal.assert();
}
