//! `github-repo` extractor — public REST repo metadata (W-ENRICH-1, doc 06 §1 MVP).
//!
//! ## Responsibilities
//! - Match `github.com/{owner}/{repo}` URLs (and only those — issues/PRs/blobs/users are NOT repos).
//! - Declare the public unauthenticated REST resources the runner fetches: `api.github.com/repos/
//!   {owner}/{repo}` (description + topics + stars) and, as a follow-up, `/repos/{owner}/{repo}/readme`
//!   (the raw README, capped + summarised). NO auth — the unauth limit is 60 req/hr/IP, enforced by
//!   the runner's per-host token bucket (06 §1: "the hardest constraint").
//! - Map the JSON into an [`EnrichmentResult`]: readable_title = `owner/repo`, summary = description,
//!   metadata/extraction_json = topics + stars + language.
//!
//! ## Not responsible for
//! - Fetching (the runner does ALL egress through the shared client, SSRF-guarded + rate-limited), or
//!   user-PAT auth (5000/hr is a future W-ENRICH; the runner's rate-limiter is left pluggable for it).
//!
//! ## Why a side-channel API rather than scraping the repo HTML
//! GitHub's repo page is a heavy SPA whose description/topics live behind hydration; the public REST
//! endpoint returns the same fields as small, stable JSON. og:image already proves an unauthenticated
//! side-channel API is viable within this privacy posture (06 §0, Bilibili precedent).

use super::super::{EnrichmentResult, build_enrichment_summary, truncate_text};
use super::{ApiRequest, ExtractContext, ExtractKind, Extractor};
use serde_json::{Value, json};

/// Extractor id + stored `content_source`.
pub(crate) const GITHUB_REPO_EXTRACTOR_ID: &str = "github-repo";
/// Schema version; a bump refetches only github-repo rows (06 §3).
pub(crate) const GITHUB_REPO_EXTRACTOR_VERSION: u32 = 1;
/// Production GitHub REST API origin. Pinned here; tests inject a mockito base via the runner.
pub(crate) const GITHUB_API_BASE: &str = "https://api.github.com";
/// Body cap for the repo JSON (the `/repos` payload is a few KB; 256 KiB is generous head-room).
const GITHUB_REPO_BODY_CAP: usize = 256 * 1024;
/// Body cap for the README payload (capped harder; only the lead is summarised).
const GITHUB_README_BODY_CAP: usize = 512 * 1024;
/// Char cap on the README text we retain in the readable blob (the full README stays re-fetchable).
const GITHUB_README_CHAR_CAP: usize = 8_000;

/// GitHub public-repo metadata extractor.
pub(crate) struct GithubRepoExtractor;

impl Extractor for GithubRepoExtractor {
    fn id(&self) -> &'static str {
        GITHUB_REPO_EXTRACTOR_ID
    }

    fn version(&self) -> u32 {
        GITHUB_REPO_EXTRACTOR_VERSION
    }

    fn matches(&self, url: &str) -> bool {
        parse_owner_repo(url).is_some()
    }

    fn fetch_kind(&self) -> ExtractKind {
        ExtractKind::JsonApi
    }

    fn api_request(&self, url: &str) -> Option<ApiRequest> {
        api_request_with_base(url, GITHUB_API_BASE)
    }

    fn secondary_api_request(&self, url: &str) -> Option<ApiRequest> {
        readme_request_with_base(url, GITHUB_API_BASE)
    }

    fn extract(&self, ctx: &ExtractContext) -> EnrichmentResult {
        build_github_enrichment(ctx)
    }
}

/// Parses `(owner, repo)` from a GitHub repo URL, or `None` when the URL is not a repo root.
///
/// Accepts `https://github.com/{owner}/{repo}` (with optional trailing slash / `.git` / `#fragment` /
/// `?query`). REJECTS non-repo paths: a bare user (`/owner`), reserved roots (`/about`, `/features`,
/// `/settings`, `/marketplace`, …), and deeper sub-paths (`/owner/repo/issues`, `/blob/...`) so the
/// extractor only claims the canonical repo landing page (a sub-path is still readable via the generic
/// fallback). PURE → unit-tested + mutation-hardened.
pub(crate) fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host_norm = host.strip_prefix("www.").unwrap_or(&host);
    if host_norm != "github.com" {
        return None;
    }
    let segments: Vec<&str> =
        parsed.path().split('/').filter(|segment| !segment.is_empty()).collect();
    // A repo root is EXACTLY two path segments: owner/repo. One segment is a user, three+ is a
    // sub-page (issues/blob/...). Anything else falls through to the generic extractor.
    if segments.len() != 2 {
        return None;
    }
    let owner = segments[0];
    let repo = segments[1].trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    // GitHub reserves a set of top-level paths that are NOT user accounts; a two-segment path under
    // one of these (e.g. `/marketplace/actions`) is not a repo. Guarding the common reserved roots
    // keeps the extractor from claiming product pages as repos.
    const RESERVED_OWNERS: &[&str] = &[
        "about",
        "features",
        "settings",
        "marketplace",
        "sponsors",
        "topics",
        "collections",
        "trending",
        "explore",
        "notifications",
        "pulls",
        "issues",
        "new",
        "login",
        "join",
        "apps",
        "orgs",
    ];
    if RESERVED_OWNERS.contains(&owner.to_ascii_lowercase().as_str()) {
        return None;
    }
    // Owner/repo characters: GitHub allows [A-Za-z0-9._-]; rejecting anything else keeps a malformed
    // path (encoded slashes, spaces) from synthesising a bad API URL.
    let valid = |segment: &str| {
        segment.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    };
    if !valid(owner) || !valid(repo) {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// Builds the primary `/repos/{owner}/{repo}` API request against a base origin.
///
/// Split from the trait method so tests can point at a mockito base without monkeypatching
/// `api.github.com` (mirrors og_images_synth's Bilibili `_with_base` pattern).
pub(crate) fn api_request_with_base(url: &str, api_base: &str) -> Option<ApiRequest> {
    let (owner, repo) = parse_owner_repo(url)?;
    let trimmed = api_base.trim_end_matches('/');
    Some(ApiRequest {
        url: format!("{trimmed}/repos/{owner}/{repo}"),
        body_cap_bytes: GITHUB_REPO_BODY_CAP,
    })
}

/// Builds the follow-up `/repos/{owner}/{repo}/readme` API request against a base origin.
pub(crate) fn readme_request_with_base(url: &str, api_base: &str) -> Option<ApiRequest> {
    let (owner, repo) = parse_owner_repo(url)?;
    let trimmed = api_base.trim_end_matches('/');
    Some(ApiRequest {
        url: format!("{trimmed}/repos/{owner}/{repo}/readme"),
        body_cap_bytes: GITHUB_README_BODY_CAP,
    })
}

/// Maps the fetched repo JSON (+ optional README) into an [`EnrichmentResult`].
///
/// PURE → unit-tested over fixture JSON. Reads `full_name`/`description`/`topics`/`stargazers_count`/
/// `language` from the primary body and the base64-encoded `content` from the README body. A repo with
/// no description AND no README is `empty` (honest: nothing readable found), not a fake success.
fn build_github_enrichment(ctx: &ExtractContext) -> EnrichmentResult {
    let repo: Value = match serde_json::from_slice(&ctx.primary_body) {
        Ok(value) => value,
        Err(error) => {
            return EnrichmentResult {
                status: "decode-error".to_string(),
                final_url: ctx.final_url.clone().or_else(|| Some(ctx.url.clone())),
                extraction: json!({
                    "contentType": "application/json",
                    "error": format!("GitHub repo JSON was unparseable: {error}"),
                }),
                extractor_version: Some(GITHUB_REPO_EXTRACTOR_VERSION as i64),
                ..EnrichmentResult::default()
            };
        }
    };

    let full_name = string_field(&repo, "full_name")
        .or_else(|| parse_owner_repo(&ctx.url).map(|(owner, name)| format!("{owner}/{name}")));
    let description = string_field(&repo, "description");
    let language = string_field(&repo, "language");
    let stars = repo.get("stargazers_count").and_then(Value::as_i64);
    let topics = repo
        .get("topics")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();

    let readme = ctx.secondary_body.as_deref().and_then(decode_github_readme);

    // The readable text is the structured header (repo name + description + topics + stars) followed
    // by the capped README lead, so the embedding funnel + detail panel get a rich-but-bounded body.
    let mut body_lines: Vec<String> = Vec::new();
    if let Some(full_name) = full_name.as_ref() {
        body_lines.push(format!("Repository: {full_name}"));
    }
    if let Some(description) = description.as_ref() {
        body_lines.push(description.clone());
    }
    if !topics.is_empty() {
        body_lines.push(format!("Topics: {}", topics.join(", ")));
    }
    if let Some(language) = language.as_ref() {
        body_lines.push(format!("Language: {language}"));
    }
    if let Some(stars) = stars {
        body_lines.push(format!("Stars: {stars}"));
    }
    if let Some(readme) = readme.as_ref() {
        body_lines.push(String::new());
        body_lines.push(truncate_text(readme.trim(), GITHUB_README_CHAR_CAP));
    }
    let readable_text = body_lines.join("\n").trim().to_string();

    // The capped summary (06 §3) is the description (the cleanest one-line label); fall back to the
    // README lead, then the repo name, so the dedup hash + FTS5 mirror always get a concise field.
    let summary_candidate = description
        .clone()
        .or_else(|| readme.as_deref().map(|value| value.to_string()))
        .or_else(|| full_name.clone());
    let enrichment_summary = build_enrichment_summary(summary_candidate.as_deref());

    let has_content = description.is_some() || !topics.is_empty() || readme.is_some();
    let status = if has_content { "success" } else { "empty" };

    EnrichmentResult {
        status: status.to_string(),
        final_url: ctx.final_url.clone().or_else(|| Some(ctx.url.clone())),
        language: language.clone(),
        readable_title: full_name.clone(),
        readable_text: (!readable_text.is_empty()).then_some(readable_text),
        snippets: enrichment_summary.clone().into_iter().collect(),
        extraction: json!({
            "contentType": "application/json",
            "extractor": GITHUB_REPO_EXTRACTOR_ID,
            "fullName": full_name,
            "description": description,
            "topics": topics,
            "stars": stars,
            "language": language,
            "hasReadme": readme.is_some(),
        }),
        enrichment_summary,
        extractor_version: Some(GITHUB_REPO_EXTRACTOR_VERSION as i64),
    }
}

/// Decodes the GitHub `/readme` payload's base64 `content` into UTF-8 text.
///
/// The `/readme` endpoint returns `{ "content": "<base64>", "encoding": "base64", ... }`. We accept
/// only base64 (the documented encoding); whitespace inside the base64 (GitHub line-wraps it) is
/// stripped before decoding. Returns `None` when the body is not JSON, the encoding is unexpected, the
/// base64 is invalid, or the decoded bytes are empty. PURE → unit-tested.
fn decode_github_readme(body: &[u8]) -> Option<String> {
    let parsed: Value = serde_json::from_slice(body).ok()?;
    let encoding = parsed.get("encoding").and_then(Value::as_str).unwrap_or("base64");
    if !encoding.eq_ignore_ascii_case("base64") {
        return None;
    }
    let raw = parsed.get("content").and_then(Value::as_str)?;
    let compact: String = raw.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.is_empty() {
        return None;
    }
    let bytes = base64_decode(&compact)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let trimmed = text.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// Minimal standard-base64 decoder (no new dependency).
///
/// GitHub's README content is standard base64 (`+`/`/` alphabet) with `=` padding. A tiny decoder
/// keeps the supply-chain surface unchanged (no `base64` crate pull-in for one call site). Returns
/// `None` on any invalid character so a malformed body is skipped rather than panicking. PURE.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = input.bytes().filter(|byte| *byte != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut chunk: u32 = 0;
    let mut bits = 0_u32;
    for byte in bytes {
        let six = value(byte)? as u32;
        chunk = (chunk << 6) | six;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((chunk >> bits) as u8);
        }
    }
    Some(out)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}
