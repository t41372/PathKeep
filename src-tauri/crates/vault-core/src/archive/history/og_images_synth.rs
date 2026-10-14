//! Per-host og:image synthesis for video sites.
//!
//! ## Responsibilities
//! - Recognise URLs from video hosts whose social-card image is reliably
//!   addressable without parsing the page HTML, and return the
//!   ready-to-fetch image URL.
//! - For hosts whose cover image requires a side-channel JSON API call
//!   (Bilibili), expose a function that performs that call and returns
//!   the image URL.
//!
//! ## Why this module exists
//! - The generic `og:image` scraper relies on the page returning a
//!   server-rendered `<meta property="og:image">`. YouTube ships such a
//!   tag but most modern Chrome UAs see CDN edge variants that strip it
//!   for cache-key reasons; the Browse view consistently came back empty
//!   on `youtube.com/watch?v=…` pages.
//! - Bilibili gates the page on regional UA / cookie checks and
//!   intermittently rewrites the meta tag into JS, so the scraper misses
//!   the cover image in roughly half of real-archive URLs.
//! - For both hosts the canonical cover image is recoverable from the
//!   URL (YouTube) or via a public unauthenticated API (Bilibili). Using
//!   that path lets us cache the right bytes against the right page URL
//!   without falling back to a generic-looking icon-only card.
//!
//! ## Privacy posture
//! - YouTube path: no extra network — we only synthesize a URL and let
//!   the existing image fetch pipeline download it. No referrer leak,
//!   no cookies, no fingerprinting.
//! - Bilibili path: one extra HTTP GET to `api.bilibili.com` per cache
//!   miss, made through the same reqwest client (same UA, no referrer,
//!   no cookies). We do not retain the JSON body — only the `data.pic`
//!   string is extracted before the response is dropped.

use reqwest::blocking::Client;

use crate::utils::url_domain;

/// Synthesizes an og:image URL that the fetch pipeline can download
/// directly, without parsing the page HTML.
///
/// Returns `None` when the URL is not on a host we have a deterministic
/// template for. Callers fall through to the generic scraper path.
pub fn synthesize_image_url_from_url(page_url: &str) -> Option<String> {
    if let Some(id) = youtube_video_id(page_url) {
        // `maxresdefault.jpg` is the highest fidelity. The fetch pipeline
        // already handles 404 / unsupported-mime cleanly; callers that
        // observe `http_error` for a synthesized YouTube URL can retry
        // through the page-html path on the next refetch cycle.
        return Some(format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg"));
    }
    None
}

/// Resolves the cover-image URL for hosts that require a side-channel
/// API call (currently Bilibili). Returns `None` when the URL is not on
/// such a host, or when the API call fails or returns an unparseable body.
pub fn resolve_image_url_via_api(client: &Client, page_url: &str) -> Option<String> {
    let bilibili_id = bilibili_video_id(page_url)?;
    let api_url = match &bilibili_id {
        BilibiliId::Bv(bv) => format!("{BILIBILI_API_BASE}/x/web-interface/view?bvid={bv}"),
        BilibiliId::Av(av) => format!("{BILIBILI_API_BASE}/x/web-interface/view?aid={av}"),
    };
    let response = client.get(&api_url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    // The view API returns about 5–10 KB. Anything past 64 KiB is a misbehaving or hostile
    // endpoint, and reading stops there instead of after the whole body is in memory.
    let body = super::og_images_fetch::read_capped_response(response, 64 * 1024)?;
    extract_bilibili_pic_field(&body)
}

const BILIBILI_API_BASE: &str = "https://api.bilibili.com";

/// Pulls the `data.pic` string out of a Bilibili view-API JSON body.
/// Returns `None` when the body isn't JSON, the `data` object is
/// missing, the `pic` field is absent, or the value is not a non-empty
/// string.
fn extract_bilibili_pic_field(body: &[u8]) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_slice(body).ok()?;
    let pic = parsed.get("data")?.get("pic")?.as_str()?;
    let trimmed = pic.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

/// Extracts the YouTube video id from any common watch / shorts / share URL.
///
/// Supported shapes:
/// - `https://www.youtube.com/watch?v=ID&...`
/// - `https://m.youtube.com/watch?v=ID`
/// - `https://music.youtube.com/watch?v=ID`
/// - `https://youtube.com/shorts/ID`
/// - `https://youtu.be/ID`
fn youtube_video_id(page_url: &str) -> Option<String> {
    let url = reqwest::Url::parse(page_url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    // Compare against the canonical host form (strip leading `www.`).
    let host_norm = host.strip_prefix("www.").unwrap_or(&host);
    if host_norm == "youtu.be" {
        let id = url.path().trim_matches('/');
        return sanitise_youtube_id(id);
    }
    let is_youtube_host = matches!(
        host_norm,
        "youtube.com" | "m.youtube.com" | "music.youtube.com" | "gaming.youtube.com"
    );
    if !is_youtube_host {
        return None;
    }
    if url.path() == "/watch" {
        for (key, value) in url.query_pairs() {
            if key == "v" {
                return sanitise_youtube_id(&value);
            }
        }
        return None;
    }
    // /shorts/{id} or /embed/{id}
    let segments: Vec<&str> = url.path().split('/').filter(|seg| !seg.is_empty()).collect();
    if segments.len() >= 2 && matches!(segments[0], "shorts" | "embed" | "live") {
        return sanitise_youtube_id(segments[1]);
    }
    None
}

fn sanitise_youtube_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    // YouTube video ids are 11 chars from the [A-Za-z0-9_-] alphabet.
    // Anything else is either a playlist slug, an empty path segment, or a
    // pasted essay; rejecting them here keeps the synthesised image URL
    // valid by construction.
    if trimmed.len() != 11 {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some(trimmed.to_string())
}

#[derive(Debug, PartialEq, Eq)]
enum BilibiliId {
    Bv(String),
    Av(String),
}

/// Extracts the Bilibili video id (BV-form or aid-form) from any common
/// watch URL.
///
/// TODO: short-link hosts (`b23.tv`) and the live / space / search
/// subdomains are not handled — they redirect on visit to the
/// canonical `bilibili.com/video/{id}` URL, so the prefetch pass picks
/// them up on the next backup once the redirect resolves. If users
/// report blank cards on freshly-pasted `b23.tv` URLs, add a HEAD
/// redirect resolver here before calling the view API.
fn bilibili_video_id(page_url: &str) -> Option<BilibiliId> {
    let url = reqwest::Url::parse(page_url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host_norm = host.strip_prefix("www.").unwrap_or(&host);
    if !matches!(host_norm, "bilibili.com" | "m.bilibili.com") {
        return None;
    }
    let segments: Vec<&str> = url.path().split('/').filter(|seg| !seg.is_empty()).collect();
    // /video/{idLike}/...
    if segments.len() < 2 || segments[0] != "video" {
        return None;
    }
    let id_like = segments[1];
    if let Some(bv) = parse_bilibili_bv(id_like) {
        return Some(BilibiliId::Bv(bv));
    }
    if let Some(av) = parse_bilibili_av(id_like) {
        return Some(BilibiliId::Av(av));
    }
    None
}

fn parse_bilibili_bv(raw: &str) -> Option<String> {
    if raw.len() != 12 {
        return None;
    }
    if !raw.starts_with("BV") {
        return None;
    }
    let body = &raw[2..];
    if !body.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(raw.to_string())
}

fn parse_bilibili_av(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let digits = lower.strip_prefix("av")?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(digits.to_string())
}

/// True when the URL is on a host that always returns `None` from the
/// generic HTML-meta scraper today and should bypass it in favour of one
/// of the synth paths above. Exposed so the fetch pipeline can short
/// circuit cleanly without re-running the per-host classifiers.
pub fn host_requires_synthesis(page_url: &str) -> bool {
    let host = url_domain(page_url).to_ascii_lowercase();
    let host_norm = host.strip_prefix("www.").unwrap_or(&host);
    matches!(
        host_norm,
        "youtube.com"
            | "m.youtube.com"
            | "music.youtube.com"
            | "gaming.youtube.com"
            | "youtu.be"
            | "bilibili.com"
            | "m.bilibili.com"
    )
}
