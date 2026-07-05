//! Site-content extractor framework (W-ENRICH-1, doc 06 §1).
//!
//! ## Responsibilities
//! - Define the [`Extractor`] trait: how a site adapter is matched (`matches`), what kind of bytes it
//!   needs (`fetch_kind`), and how already-fetched bytes become an [`EnrichmentResult`] (`extract`).
//! - Own the first-match-wins [`registry`], with `generic-readable` as the terminal fallback so every
//!   page resolves to exactly one extractor.
//! - Carry the [`ExtractKind`] (Html | JsonApi | None) so the job runner knows what to fetch and which
//!   `Accept` header / body cap to apply, and [`ExtractContext`] (the already-fetched bytes + URLs).
//!
//! ## Not responsible for
//! - ANY network egress (doc 06 §1/§2: privacy posture is enforced in ONE place). Extractors NEVER
//!   open a socket — the job runner ([`super::content_fetch`]) does ALL fetching through the single
//!   shared `build_fetch_client()`, applies the SSRF guard + rate-limit + body cap, and hands the
//!   extractor already-fetched bytes. This makes the egress chokepoint un-bypassable.
//! - Persisting the result, the dedup hash, or the FTS5 mirror — the job runner + indexing own those.
//!
//! ## Why a trait + registry rather than the `enrichment_site_adapters` match
//! The og:image-era `adapt_site_content` routed a PARSED `scraper::Html` by domain. That skeleton is
//! promoted here into a first-class plug-in surface so (a) each source declares its OWN
//! `version()` (a bump triggers a bounded refetch of just its rows, 06 §3), (b) the JSON-API sources
//! (GitHub) and HTML sources (generic-readable) share ONE dispatch + ONE egress path, and (c) adding
//! P2 extractors (YouTube/Bilibili metadata) is a registry append, not a new fetch path.

pub(crate) mod generic_readable;
pub(crate) mod github_repo;

use super::EnrichmentResult;

pub(crate) use self::generic_readable::GenericReadableExtractor;
pub(crate) use self::github_repo::GithubRepoExtractor;

/// What kind of resource one extractor needs the job runner to fetch.
///
/// The runner uses this to pick the request shape (HTML `Accept` + 1 MiB cap vs. JSON `Accept` + a
/// smaller cap) and to decide whether a sub-resource (the GitHub README) is fetched. A purely
/// URL-derived extractor (no body fetch) is a P2 concern and will add a `UrlSynth` variant then; the
/// MVP only needs `Html` (generic-readable) and `JsonApi` (github-repo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtractKind {
    /// Fetch the page as HTML (text/html, 1 MiB cap) and hand the bytes to `extract`.
    Html,
    /// Fetch a JSON API resource the extractor names via [`Extractor::api_request`].
    JsonApi,
}

/// One JSON-API sub-request an extractor wants the runner to perform on its behalf.
///
/// The extractor NEVER fetches; it returns this descriptor and the runner performs the GET through the
/// shared client AFTER the URL passes the SSRF guard (doc 06 §2b: every API sub-resource URL is
/// guarded, not just the page URL). `body_cap_bytes` lets a chatty API cap its body below the HTML cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiRequest {
    /// Absolute https URL of the API resource (guarded by the runner before the GET).
    pub url: String,
    /// Hard cap on the JSON body the runner will read for this request.
    pub body_cap_bytes: usize,
}

/// The already-fetched inputs handed to [`Extractor::extract`].
///
/// `primary_body` is the bytes the runner fetched for the extractor's [`ExtractKind`] (HTML or the
/// first JSON resource). `final_url` is the post-redirect URL (the runner followed redirects through
/// the shared client). `secondary_body` carries an optional follow-up resource (the GitHub README)
/// the runner fetched when the extractor's `extract` is a two-step source — kept simple (one optional
/// extra) for the MVP; a richer multi-resource shape is a P2 concern.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExtractContext {
    /// The visited page URL (already SSRF-guarded + https-only by the runner).
    pub url: String,
    /// The post-redirect final URL, when the runner observed redirects.
    pub final_url: Option<String>,
    /// The primary fetched body (HTML or the first JSON resource).
    pub primary_body: Vec<u8>,
    /// The declared content-type header of `primary_body`, lower-cased head only.
    pub content_type: Option<String>,
    /// An optional secondary fetched body (e.g. the GitHub README), when the runner fetched one.
    pub secondary_body: Option<Vec<u8>>,
}

/// A site-content extractor: matches a URL, declares what to fetch, and shapes the result.
///
/// Implementors are PURE w.r.t. the network — `matches`, `fetch_kind`, `api_request`, and `extract`
/// must not perform I/O. The runner orchestrates all egress. `content_source()` is the stored
/// `visit_content_enrichments.content_source` (the extractor id), and `version()` is stamped into the
/// row's `extractor_version` so a bump refetches only this source's rows (06 §3).
pub(crate) trait Extractor: Send + Sync {
    /// Stable id, also the stored `content_source` ("github-repo" | "generic-readable").
    fn id(&self) -> &'static str;

    /// Schema version; a bump triggers a bounded refetch of only THIS source's rows (06 §3).
    fn version(&self) -> u32;

    /// True when this extractor handles `url`. First-match-wins in the registry.
    fn matches(&self, url: &str) -> bool;

    /// What the runner must fetch for this extractor.
    fn fetch_kind(&self) -> ExtractKind;

    /// For [`ExtractKind::JsonApi`] extractors: the primary API resource to fetch.
    ///
    /// Returns `None` for HTML extractors. The runner SSRF-guards the returned URL before the GET.
    fn api_request(&self, _url: &str) -> Option<ApiRequest> {
        None
    }

    /// For two-step JSON extractors: an optional follow-up resource fetched after the primary.
    ///
    /// The runner fetches it (guarded) into [`ExtractContext::secondary_body`] when `Some`. Returns
    /// `None` for single-resource extractors. The GitHub extractor uses this for the README.
    fn secondary_api_request(&self, _url: &str) -> Option<ApiRequest> {
        None
    }

    /// Shapes the already-fetched [`ExtractContext`] into an [`EnrichmentResult`].
    ///
    /// Must stamp `extractor_version` = `self.version()` and fill `enrichment_summary`. NEVER fetches.
    fn extract(&self, ctx: &ExtractContext) -> EnrichmentResult;
}

/// Returns the first-match-wins extractor registry (06 §1).
///
/// Order matters: specific extractors come first, `generic-readable` is the terminal fallback so EVERY
/// page resolves to exactly one extractor. New sources (P2 YouTube/Bilibili) insert BEFORE the
/// fallback. Built fresh per call (the set is tiny + the trait objects are zero-state), so callers do
/// not share mutable state.
pub(crate) fn registry() -> Vec<Box<dyn Extractor>> {
    vec![Box::new(GithubRepoExtractor), Box::new(GenericReadableExtractor)]
}

/// Resolves the first extractor whose `matches(url)` is true (first-match-wins).
///
/// Always returns `Some` in practice because `generic-readable` matches every http(s) URL, but the
/// signature is honest (`Option`) so a future registry without a terminal fallback can't panic.
pub(crate) fn resolve_extractor(url: &str) -> Option<Box<dyn Extractor>> {
    registry().into_iter().find(|extractor| extractor.matches(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_github_before_generic_fallback() {
        let github = resolve_extractor("https://github.com/rust-lang/rust").expect("github match");
        assert_eq!(github.id(), "github-repo");

        let generic =
            resolve_extractor("https://example.com/some/article").expect("generic fallback");
        assert_eq!(generic.id(), "generic-readable");
    }

    #[test]
    fn generic_readable_is_the_terminal_fallback_for_any_https_page() {
        // Even an odd path resolves to SOMETHING — the fallback guarantees one extractor per page.
        let resolved = resolve_extractor("https://news.example.org/2026/story").expect("resolved");
        assert_eq!(resolved.id(), "generic-readable");
    }

    #[test]
    fn registry_is_first_match_wins_with_fallback_last() {
        let registry = registry();
        assert_eq!(registry.first().map(|extractor| extractor.id()), Some("github-repo"));
        assert_eq!(registry.last().map(|extractor| extractor.id()), Some("generic-readable"));
    }
}
