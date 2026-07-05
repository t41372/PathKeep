//! `generic-readable` extractor — the terminal fallback (W-ENRICH-1, doc 06 §7).
//!
//! ## Responsibilities
//! - Match ANY http(s) URL (it is the first-match-wins registry's last entry, so every page that no
//!   specific extractor claims lands here).
//! - Turn already-fetched HTML bytes into an [`EnrichmentResult`] via the deterministic readability
//!   extractor [`crate::enrichment::build_enrichment_result_from_html`] (NO LLM in the MVP).
//!
//! ## Not responsible for
//! - Fetching (the runner hands it bytes), or summarising with an LLM (P2, opt-in only).

use super::super::build_enrichment_result_from_html;
use super::{ExtractContext, ExtractKind, Extractor};
use crate::enrichment::EnrichmentResult;

/// Deterministic readability fallback extractor.
pub(crate) struct GenericReadableExtractor;

/// Extractor id + stored `content_source`.
pub(crate) const GENERIC_READABLE_EXTRACTOR_ID: &str = "generic-readable";
/// Schema version; a bump refetches only generic-readable rows (06 §3).
pub(crate) const GENERIC_READABLE_EXTRACTOR_VERSION: u32 = 1;

impl Extractor for GenericReadableExtractor {
    fn id(&self) -> &'static str {
        GENERIC_READABLE_EXTRACTOR_ID
    }

    fn version(&self) -> u32 {
        GENERIC_READABLE_EXTRACTOR_VERSION
    }

    fn matches(&self, url: &str) -> bool {
        // Terminal fallback: match any https/http page. The runner already enforces https-only +
        // SSRF before fetching, so matching http here is harmless (it never reaches the wire over
        // http in production); keeping it permissive means the registry always resolves to SOMETHING.
        url.starts_with("https://") || url.starts_with("http://")
    }

    fn fetch_kind(&self) -> ExtractKind {
        ExtractKind::Html
    }

    fn extract(&self, ctx: &ExtractContext) -> EnrichmentResult {
        // The runner already verified the content-type is HTML before fetching the body; decode
        // leniently (lossy) so a page with a stray invalid byte still yields its readable title/text
        // rather than failing the whole extraction.
        let text = match std::str::from_utf8(&ctx.primary_body) {
            Ok(text) => text.to_string(),
            Err(_) => String::from_utf8_lossy(&ctx.primary_body).into_owned(),
        };
        let content_type = ctx.content_type.clone().unwrap_or_else(|| "text/html".to_string());
        let mut result =
            build_enrichment_result_from_html(&ctx.url, ctx.final_url.clone(), content_type, &text);
        result.extractor_version = Some(GENERIC_READABLE_EXTRACTOR_VERSION as i64);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(url: &str, html: &str) -> ExtractContext {
        ExtractContext {
            url: url.to_string(),
            final_url: Some(url.to_string()),
            primary_body: html.as_bytes().to_vec(),
            content_type: Some("text/html".to_string()),
            secondary_body: None,
        }
    }

    #[test]
    fn matches_any_http_or_https_url() {
        let extractor = GenericReadableExtractor;
        assert!(extractor.matches("https://example.com/x"));
        assert!(extractor.matches("http://example.com/x"));
        assert!(!extractor.matches("ftp://example.com/x"));
    }

    #[test]
    fn extract_produces_summary_and_stamps_version() {
        let extractor = GenericReadableExtractor;
        let html = "<html><head><title>A clean title</title></head>\
                    <body><main><p>First readable paragraph with content.</p></main></body></html>";
        let result = extractor.extract(&ctx("https://example.com/post", html));
        assert_eq!(result.status, "success");
        assert_eq!(result.readable_title.as_deref(), Some("A clean title"));
        assert_eq!(result.enrichment_summary.as_deref(), Some("A clean title"));
        assert_eq!(result.extractor_version, Some(GENERIC_READABLE_EXTRACTOR_VERSION as i64));
    }

    #[test]
    fn extract_recovers_from_invalid_utf8_via_lossy_decode() {
        let extractor = GenericReadableExtractor;
        let mut body = b"<html><head><title>Lossy</title></head><body><p>".to_vec();
        body.extend_from_slice(&[0xFF, 0xFE]);
        body.extend_from_slice(b" body</p></body></html>");
        let result = extractor.extract(&ExtractContext {
            url: "https://example.com/lossy".to_string(),
            final_url: None,
            primary_body: body,
            content_type: None,
            secondary_body: None,
        });
        assert_eq!(result.readable_title.as_deref(), Some("Lossy"));
        assert_eq!(result.extractor_version, Some(1));
    }
}
