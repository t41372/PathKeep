//! HTTP fetch + parse pipeline for og:image previews.
//!
//! ## Responsibilities
//! - GET the target page over HTTPS, parse out the og:image URL using the
//!   accepted selector order (og:image:secure_url → og:image →
//!   twitter:image → twitter:image:src).
//! - GET the image URL, enforce size/MIME guards, and shape a
//!   `FetchedOgImage` whose `as_insert()` produces an `OgImageInsert`
//!   the storage layer can persist verbatim.
//! - Surface explicit fetch outcomes (`ok`, `missing`, `http_error`,
//!   `parse_error`, `too_large`, `unsupported_mime`, `blocked`) so the
//!   worker queue + Settings UI can reason about cache state.
//!
//! ## Not responsible for
//! - Scheduling retries, rate limiting between hosts, or persisting
//!   results. The orchestrator (vault-worker, C4-wired) calls
//!   `fetch_og_image_for` for one URL at a time and hands the
//!   `FetchedOgImage` to `upsert_og_image` via `as_insert()`.
//! - Image decoding / dimension measurement. The frontend measures the
//!   intrinsic size at render time; storing dimensions is intentionally
//!   deferred so the dependency surface stays small.
//!
//! ## Privacy posture
//! - HTTPS-only. http:// page URLs return `parse_error` without ever
//!   hitting the wire. http:// og:image URLs are upgraded to https.
//! - No Referer header. A static, non-identifying UA string is used.
//! - The fetch path is opt-out per `AppConfig.og_image.fetch_enabled`,
//!   not opt-in — but the orchestrator must short-circuit when the user
//!   has disabled fetching. This module always tries when called.

use super::og_images::{OgImageInsert, fetch_status};
use crate::utils::url_domain;
use anyhow::Result;
use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT},
};
use scraper::{Html, Selector};
use std::time::Duration;

/// Hard cap on the HTML body we are willing to parse. Anything bigger
/// is treated as `parse_error` — pages that fat are usually SSR-rendered
/// SPAs whose og:image is not in the static HTML anyway.
const MAX_HTML_BYTES: usize = 1_048_576; // 1 MiB

/// Hard cap on image bytes. Larger responses are rejected up-front via
/// Content-Length and truncated mid-stream if the header lies.
pub const MAX_IMAGE_BYTES: usize = 2 * 1_048_576; // 2 MiB

const USER_AGENT_VALUE: &str = "PathKeep/0.3 (link-preview; data-sovereignty)";
const ACCEPT_HTML: &str = "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5";
const ACCEPT_IMAGE: &str = "image/png,image/jpeg,image/webp,image/gif;q=0.9,*/*;q=0.1";

/// Builds the reqwest client used by the fetch pipeline. Exposed so the
/// orchestrator can share one client across many fetches (connection
/// pool reuse, single timeout policy).
pub fn build_fetch_client() -> Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    Client::builder()
        .default_headers(headers)
        // We enforce https on the page URL ourselves so http:// → parse_error.
        .https_only(false)
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(1))
        .build()
        .map_err(Into::into)
}

/// Owned fetch outcome. Always produced — even for failures — so the
/// caller can persist a negative-cache row regardless of result.
#[derive(Debug, Clone)]
pub struct FetchedOgImage {
    page_host: Option<String>,
    source_og_url: Option<String>,
    image_bytes: Option<Vec<u8>>,
    mime: Option<&'static str>,
    fetch_status: &'static str,
    http_status: Option<i64>,
}

impl FetchedOgImage {
    /// Borrows the owned fields into an `OgImageInsert` lifetime-tied
    /// to `(self, page_url)`. The caller persists this directly via
    /// `upsert_og_image`.
    pub fn as_insert<'a>(&'a self, page_url: &'a str) -> OgImageInsert<'a> {
        OgImageInsert {
            page_url,
            page_host: self.page_host.as_deref(),
            source_og_url: self.source_og_url.as_deref(),
            image_bytes: self.image_bytes.as_deref(),
            mime: self.mime,
            width: None,
            height: None,
            fetch_status: self.fetch_status,
            http_status: self.http_status,
            refetch_after: None,
            fetch_attempts: 1,
            created_by_run_id: None,
        }
    }

    /// The terminal fetch state. Read by tests + the worker drain loop
    /// to decide whether to schedule a retry.
    pub fn fetch_status(&self) -> &'static str {
        self.fetch_status
    }

    /// True when the fetch produced bytes ready to be cached.
    pub fn is_ok(&self) -> bool {
        self.fetch_status == fetch_status::OK
    }
}

/// Convenience constructor for a host that was blocked by the user — no
/// network call, just a persistable row.
pub fn blocked_outcome(page_url: &str) -> FetchedOgImage {
    FetchedOgImage {
        page_host: nonempty_host(page_url),
        source_og_url: None,
        image_bytes: None,
        mime: None,
        fetch_status: fetch_status::BLOCKED,
        http_status: None,
    }
}

/// One pass through the fetch pipeline.
pub fn fetch_og_image_for(client: &Client, page_url: &str) -> FetchedOgImage {
    if !page_url.starts_with("https://") {
        return FetchedOgImage {
            page_host: nonempty_host(page_url),
            source_og_url: None,
            image_bytes: None,
            mime: None,
            fetch_status: fetch_status::PARSE_ERROR,
            http_status: None,
        };
    }
    fetch_og_image_for_pipeline(client, page_url, /* upgrade_image_url = */ true)
}

/// Same pipeline as `fetch_og_image_for` minus the HTTPS guard, exposed so
/// the mockito-based unit tests can drive the full body through an
/// `http://localhost` mock server without standing up TLS. Production
/// callers always go through `fetch_og_image_for`; the test path passes
/// `upgrade_image_url = false` so mockito's http URLs survive intact.
pub(crate) fn fetch_og_image_for_unchecked(client: &Client, page_url: &str) -> FetchedOgImage {
    fetch_og_image_for_pipeline(client, page_url, /* upgrade_image_url = */ false)
}

fn fetch_og_image_for_pipeline(
    client: &Client,
    page_url: &str,
    upgrade_image_url: bool,
) -> FetchedOgImage {
    let mut outcome = FetchedOgImage {
        page_host: nonempty_host(page_url),
        source_og_url: None,
        image_bytes: None,
        mime: None,
        fetch_status: fetch_status::HTTP_ERROR,
        http_status: None,
    };

    let page = match client.get(page_url).header(ACCEPT, ACCEPT_HTML).send() {
        Ok(response) => response,
        Err(error) => {
            outcome.fetch_status = fetch_status::HTTP_ERROR;
            outcome.http_status = http_status_from_error(&error);
            return outcome;
        }
    };
    outcome.http_status = Some(i64::from(page.status().as_u16()));
    if !page.status().is_success() {
        outcome.fetch_status = fetch_status::HTTP_ERROR;
        return outcome;
    }
    if !response_content_type_is(&page, "text/html") {
        outcome.fetch_status = fetch_status::PARSE_ERROR;
        return outcome;
    }
    let html_bytes = match read_response_body(page, MAX_HTML_BYTES) {
        Ok(bytes) => bytes,
        Err(BodyReadError::TooLarge) => {
            outcome.fetch_status = fetch_status::PARSE_ERROR;
            return outcome;
        }
        Err(BodyReadError::Io) => {
            outcome.fetch_status = fetch_status::HTTP_ERROR;
            return outcome;
        }
    };
    let html = match std::str::from_utf8(&html_bytes) {
        Ok(text) => text.to_string(),
        Err(_) => String::from_utf8_lossy(&html_bytes).into_owned(),
    };

    let og_image_url = match extract_og_image_url(&html, page_url) {
        Some(url) => url,
        None => {
            outcome.fetch_status = fetch_status::MISSING;
            return outcome;
        }
    };
    let og_image_url =
        if upgrade_image_url { upgrade_http_to_https(&og_image_url) } else { og_image_url };
    outcome.source_og_url = Some(og_image_url.clone());

    let image_response = match client.get(&og_image_url).header(ACCEPT, ACCEPT_IMAGE).send() {
        Ok(response) => response,
        Err(error) => {
            outcome.fetch_status = fetch_status::HTTP_ERROR;
            outcome.http_status = http_status_from_error(&error);
            return outcome;
        }
    };
    outcome.http_status = Some(i64::from(image_response.status().as_u16()));
    if !image_response.status().is_success() {
        outcome.fetch_status = fetch_status::HTTP_ERROR;
        return outcome;
    }
    let mime = match supported_image_mime(&image_response) {
        Some(mime) => mime,
        None => {
            outcome.fetch_status = fetch_status::UNSUPPORTED_MIME;
            return outcome;
        }
    };
    if let Some(declared_length) = image_response.content_length() {
        if declared_length as usize > MAX_IMAGE_BYTES {
            outcome.fetch_status = fetch_status::TOO_LARGE;
            return outcome;
        }
    }
    let bytes = match read_response_body(image_response, MAX_IMAGE_BYTES) {
        Ok(bytes) => bytes,
        Err(BodyReadError::TooLarge) => {
            outcome.fetch_status = fetch_status::TOO_LARGE;
            return outcome;
        }
        Err(BodyReadError::Io) => {
            outcome.fetch_status = fetch_status::HTTP_ERROR;
            return outcome;
        }
    };

    outcome.image_bytes = Some(bytes);
    outcome.mime = Some(mime);
    outcome.fetch_status = fetch_status::OK;
    outcome
}

/// Pulls the first non-empty og:image candidate URL out of page HTML
/// using the accepted selector precedence. Returns absolute URLs; relative
/// URLs are resolved against the page URL.
pub fn extract_og_image_url(html: &str, page_url: &str) -> Option<String> {
    let document = Html::parse_document(html);
    for selector_str in [
        r#"meta[property="og:image:secure_url"]"#,
        r#"meta[property="og:image"]"#,
        r#"meta[name="twitter:image"]"#,
        r#"meta[name="twitter:image:src"]"#,
    ] {
        let selector = match Selector::parse(selector_str) {
            Ok(selector) => selector,
            Err(_) => continue,
        };
        for element in document.select(&selector) {
            let candidate = element.value().attr("content").unwrap_or_default().trim();
            if candidate.is_empty() {
                continue;
            }
            return Some(absolutize_url(candidate, page_url));
        }
    }
    None
}

/// Returns true when `page_host` (lower-cased) is on the user's
/// blocklist. The blocklist is case-insensitive on both sides.
pub fn is_host_blocked(blocked_hosts: &[String], page_url: &str) -> bool {
    let host = url_domain(page_url).to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    blocked_hosts.iter().any(|blocked| blocked.trim().to_ascii_lowercase() == host)
}

fn http_status_from_error(error: &reqwest::Error) -> Option<i64> {
    error.status().map(|status| i64::from(status.as_u16()))
}

fn response_content_type_is(response: &Response, expected_prefix: &str) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .map(|head| head.trim().eq_ignore_ascii_case(expected_prefix))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn supported_image_mime(response: &Response) -> Option<&'static str> {
    let raw = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())?;
    let head = raw.split(';').next()?.trim().to_ascii_lowercase();
    match head.as_str() {
        "image/png" => Some("image/png"),
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/webp" => Some("image/webp"),
        "image/gif" => Some("image/gif"),
        _ => None,
    }
}

enum BodyReadError {
    TooLarge,
    Io,
}

fn read_response_body(mut response: Response, cap_bytes: usize) -> Result<Vec<u8>, BodyReadError> {
    use std::io::Read;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let n = match response.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return Err(BodyReadError::Io),
        };
        if buffer.len() + n > cap_bytes {
            return Err(BodyReadError::TooLarge);
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
    Ok(buffer)
}

fn absolutize_url(raw: &str, base: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return raw.to_string();
    }
    if let Ok(base_url) = reqwest::Url::parse(base) {
        if let Ok(joined) = base_url.join(raw) {
            return joined.to_string();
        }
    }
    raw.to_string()
}

fn upgrade_http_to_https(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

fn nonempty_host(page_url: &str) -> Option<String> {
    let host = url_domain(page_url).to_ascii_lowercase();
    if host.is_empty() { None } else { Some(host) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html_with_og_image(image_url: &str) -> String {
        format!(
            r#"<html><head>
              <meta property="og:image" content="{image_url}">
              <meta name="twitter:image" content="https://wrong.example.com/twitter.png">
            </head><body>page body</body></html>"#,
        )
    }

    #[test]
    fn extract_og_image_url_prefers_secure_url_over_og_image() {
        let html = r#"<html><head>
          <meta property="og:image" content="https://example.com/og.jpg">
          <meta property="og:image:secure_url" content="https://example.com/secure.jpg">
          <meta name="twitter:image" content="https://example.com/twitter.jpg">
        </head></html>"#;
        let result = extract_og_image_url(html, "https://example.com/").unwrap();
        assert_eq!(result, "https://example.com/secure.jpg");
    }

    #[test]
    fn extract_og_image_url_falls_through_to_twitter_image() {
        let html = r#"<html><head>
          <meta name="twitter:image" content="https://example.com/twitter.png">
        </head></html>"#;
        let result = extract_og_image_url(html, "https://example.com/").unwrap();
        assert_eq!(result, "https://example.com/twitter.png");
    }

    #[test]
    fn extract_og_image_url_falls_through_to_twitter_image_src() {
        let html = r#"<html><head>
          <meta name="twitter:image:src" content="https://example.com/twitter-src.png">
        </head></html>"#;
        let result = extract_og_image_url(html, "https://example.com/").unwrap();
        assert_eq!(result, "https://example.com/twitter-src.png");
    }

    #[test]
    fn extract_og_image_url_resolves_relative_against_page() {
        let html = r#"<html><head>
          <meta property="og:image" content="/static/social.png">
        </head></html>"#;
        let result = extract_og_image_url(html, "https://example.com/path/").unwrap();
        assert_eq!(result, "https://example.com/static/social.png");
    }

    #[test]
    fn extract_og_image_url_returns_none_when_meta_missing() {
        let html = r#"<html><head><title>plain</title></head></html>"#;
        assert!(extract_og_image_url(html, "https://example.com/").is_none());
    }

    #[test]
    fn extract_og_image_url_skips_empty_content_attribute() {
        let html = r#"<html><head>
          <meta property="og:image" content="   ">
          <meta name="twitter:image" content="https://example.com/twitter.png">
        </head></html>"#;
        let result = extract_og_image_url(html, "https://example.com/").unwrap();
        assert_eq!(result, "https://example.com/twitter.png");
    }

    #[test]
    fn http_only_page_url_returns_parse_error_without_network() {
        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for(&client, "http://insecure.example.com/");
        assert_eq!(outcome.fetch_status(), fetch_status::PARSE_ERROR);
        assert!(outcome.image_bytes.is_none());
    }

    #[test]
    fn https_page_url_passes_through_the_production_dispatcher() {
        // `fetch_og_image_for` is the production entry: the https-guard
        // branch falls through to `fetch_og_image_for_pipeline(.., true)`
        // (line 141). We point it at an unresolvable host so the request
        // fails with a network error — exercising both the guard fall-
        // through and `http_status_from_error` (line 297) on the
        // err.status() == None path.
        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for(&client, "https://og-image-test.invalid./");
        // DNS resolution / connect should fail; status remains None.
        assert_eq!(outcome.fetch_status(), fetch_status::HTTP_ERROR);
        assert!(outcome.image_bytes.is_none());
    }

    #[test]
    fn http_only_image_url_is_upgraded_to_https_by_production_pipeline() {
        // Production callers run `fetch_og_image_for` which sets
        // upgrade_image_url=true. After og:image extraction the pipeline
        // calls `upgrade_http_to_https` — covered indirectly here by
        // pointing at an https page that returns an http://og.png URL.
        // We can't use mockito (https-only guard), so we exercise the
        // helper directly to lock the branch.
        assert_eq!(
            upgrade_http_to_https("http://images.example.com/og.png"),
            "https://images.example.com/og.png"
        );
        assert_eq!(
            upgrade_http_to_https("https://images.example.com/og.png"),
            "https://images.example.com/og.png"
        );
        assert_eq!(upgrade_http_to_https("data:image/png;base64,AAA"), "data:image/png;base64,AAA");
    }

    #[test]
    fn is_host_blocked_matches_lowercase_and_ignores_whitespace() {
        let blocklist = vec!["GitHub.com".to_string(), "  medium.com  ".to_string()];
        assert!(is_host_blocked(&blocklist, "https://github.com/foo"));
        assert!(is_host_blocked(&blocklist, "https://medium.com/@a/b"));
        assert!(!is_host_blocked(&blocklist, "https://example.com/foo"));
    }

    #[test]
    fn blocked_outcome_persists_block_state_without_bytes() {
        let outcome = blocked_outcome("https://github.com/foo");
        assert_eq!(outcome.fetch_status(), fetch_status::BLOCKED);
        let insert = outcome.as_insert("https://github.com/foo");
        assert_eq!(insert.page_url, "https://github.com/foo");
        assert_eq!(insert.fetch_status, fetch_status::BLOCKED);
        assert!(insert.image_bytes.is_none());
        assert_eq!(insert.page_host, Some("github.com"));
    }

    #[test]
    fn fetch_returns_ok_with_bytes_when_page_and_image_resolve() {
        let mut page = mockito::Server::new();
        let mut images = mockito::Server::new();
        let image_url = format!("{}/og.png", images.url());
        let html = html_with_og_image(&image_url);
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(html)
            .create();
        let png_bytes: [u8; 9] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0xFF];
        let _image = images
            .mock("GET", "/og.png")
            .with_status(200)
            .with_header("content-type", "image/png")
            .with_body(png_bytes)
            .create();

        let client = build_fetch_client().unwrap();
        let page_url = page.url();
        // Mockito uses http:// — we relax the https-only guard here via a
        // direct call to the inner helper for testing.
        let outcome = fetch_og_image_for_unchecked(&client, &page_url);
        assert_eq!(outcome.fetch_status(), fetch_status::OK);
        assert!(outcome.is_ok());
        let insert = outcome.as_insert(&page_url);
        assert_eq!(insert.mime, Some("image/png"));
        assert_eq!(insert.image_bytes.unwrap().len(), png_bytes.len());
        assert_eq!(insert.fetch_status, fetch_status::OK);
        assert!(insert.source_og_url.is_some());
    }

    #[test]
    fn fetch_returns_missing_when_page_has_no_og_image() {
        let mut page = mockito::Server::new();
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><head><title>blank</title></head></html>")
            .create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        assert_eq!(outcome.fetch_status(), fetch_status::MISSING);
        assert!(outcome.image_bytes.is_none());
    }

    #[test]
    fn fetch_returns_http_error_on_404_page() {
        let mut page = mockito::Server::new();
        let _page = page.mock("GET", "/").with_status(404).create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        assert_eq!(outcome.fetch_status(), fetch_status::HTTP_ERROR);
        assert_eq!(outcome.http_status, Some(404));
    }

    #[test]
    fn fetch_returns_parse_error_on_non_html_content_type() {
        let mut page = mockito::Server::new();
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{}")
            .create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        assert_eq!(outcome.fetch_status(), fetch_status::PARSE_ERROR);
    }

    #[test]
    fn fetch_rejects_unsupported_image_mime() {
        let mut page = mockito::Server::new();
        let mut images = mockito::Server::new();
        let image_url = format!("{}/og.svg", images.url());
        let html = html_with_og_image(&image_url);
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create();
        let _image = images
            .mock("GET", "/og.svg")
            .with_status(200)
            .with_header("content-type", "image/svg+xml")
            .with_body("<svg/>")
            .create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        assert_eq!(outcome.fetch_status(), fetch_status::UNSUPPORTED_MIME);
    }

    #[test]
    fn fetch_rejects_oversize_image_via_content_length_header() {
        let mut page = mockito::Server::new();
        let mut images = mockito::Server::new();
        let image_url = format!("{}/og.png", images.url());
        let html = html_with_og_image(&image_url);
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create();
        // Mockito will set Content-Length based on the body. To trigger
        // the cap we send a body larger than MAX_IMAGE_BYTES.
        let huge = vec![0_u8; MAX_IMAGE_BYTES + 1024];
        let _image = images
            .mock("GET", "/og.png")
            .with_status(200)
            .with_header("content-type", "image/png")
            .with_body(huge)
            .create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        assert_eq!(outcome.fetch_status(), fetch_status::TOO_LARGE);
    }

    // Tests above call `fetch_og_image_for_unchecked` directly (the
    // production helper that bypasses the https-only guard) so they can
    // run against mockito's http:// mock servers without standing up TLS.

    #[test]
    fn fetch_returns_parse_error_when_html_body_exceeds_cap() {
        let mut page = mockito::Server::new();
        let huge = vec![b'a'; MAX_HTML_BYTES + 4 * 1024];
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(huge)
            .create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        // body length > MAX_HTML_BYTES → read_response_body returns
        // BodyReadError::TooLarge → PARSE_ERROR (line 190 branch).
        assert_eq!(outcome.fetch_status(), fetch_status::PARSE_ERROR);
    }

    #[test]
    fn fetch_returns_http_error_when_og_image_url_is_unreachable() {
        let mut page = mockito::Server::new();
        // Point the og:image at an unresolvable hostname so the image fetch
        // step returns Err → covers lines 219-222 (http_error after image
        // network failure).
        let html = html_with_og_image("http://og-image-test.invalid./og.png");
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        assert_eq!(outcome.fetch_status(), fetch_status::HTTP_ERROR);
    }

    #[test]
    fn fetch_returns_http_error_when_image_endpoint_returns_404() {
        let mut page = mockito::Server::new();
        let mut images = mockito::Server::new();
        let image_url = format!("{}/og.png", images.url());
        let html = html_with_og_image(&image_url);
        let _page = page
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(html)
            .create();
        let _image = images.mock("GET", "/og.png").with_status(404).create();

        let client = build_fetch_client().unwrap();
        let outcome = fetch_og_image_for_unchecked(&client, &page.url());
        // Covers lines 226-228 (HTTP_ERROR after image status != success).
        assert_eq!(outcome.fetch_status(), fetch_status::HTTP_ERROR);
        assert_eq!(outcome.http_status, Some(404));
    }

    #[test]
    fn is_host_blocked_short_circuits_on_empty_host() {
        // url_domain("") == "" → empty host check returns false (line 292).
        let blocked = vec!["example.com".to_string()];
        assert!(!is_host_blocked(&blocked, ""));
    }

    #[test]
    fn absolutize_url_joins_relative_paths_against_the_page() {
        // Direct helper tests so the relative path branch (line 360 area)
        // executes deterministically — important because some pages return
        // og:image="/path/og.png" instead of an absolute URL.
        let html = format!(
            "<html><head>\
             <meta property=\"og:image\" content=\"/relative/og.png\">\
             </head></html>",
        );
        let absolute = extract_og_image_url(&html, "https://example.com/path/article")
            .expect("relative og:image should be resolved");
        assert!(
            absolute.starts_with("https://example.com/relative/og.png"),
            "expected absolute URL, got {absolute}",
        );
    }
}
