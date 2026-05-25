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
    resolve_image_url_via_api_with_base(client, page_url, BILIBILI_API_BASE)
}

const BILIBILI_API_BASE: &str = "https://api.bilibili.com";

/// Same as `resolve_image_url_via_api` but with a caller-supplied API
/// origin so tests can point the resolver at a mockito server without
/// monkeypatching `api.bilibili.com`. Production code never calls this
/// directly; the public wrapper above pins the production base URL.
pub(crate) fn resolve_image_url_via_api_with_base(
    client: &Client,
    page_url: &str,
    api_base: &str,
) -> Option<String> {
    let bilibili_id = bilibili_video_id(page_url)?;
    let trimmed_base = api_base.trim_end_matches('/');
    let api_url = match &bilibili_id {
        BilibiliId::Bv(bv) => format!("{trimmed_base}/x/web-interface/view?bvid={bv}"),
        BilibiliId::Av(av) => format!("{trimmed_base}/x/web-interface/view?aid={av}"),
    };
    let response = client.get(&api_url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    // The view API typically returns ~5–10 KB. Cap the body before the
    // JSON parse so a misbehaving endpoint can't blow memory.
    let body = response.bytes().ok()?;
    if body.len() > 64 * 1024 {
        return None;
    }
    extract_bilibili_pic_field(&body)
}

/// Pulls the `data.pic` string out of a Bilibili view-API JSON body.
/// Returns `None` when the body isn't JSON, the `data` object is
/// missing, the `pic` field is absent, or the value is not a non-empty
/// string. Exposed so unit tests can pin the parser without a live API.
pub fn extract_bilibili_pic_field(body: &[u8]) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_watch_url_resolves_to_max_res_thumbnail() {
        assert_eq!(
            synthesize_image_url_from_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_short_url_resolves_to_max_res_thumbnail() {
        assert_eq!(
            synthesize_image_url_from_url("https://youtu.be/dQw4w9WgXcQ"),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_shorts_url_resolves_to_max_res_thumbnail() {
        assert_eq!(
            synthesize_image_url_from_url("https://www.youtube.com/shorts/dQw4w9WgXcQ"),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_embed_url_resolves_to_max_res_thumbnail() {
        assert_eq!(
            synthesize_image_url_from_url("https://www.youtube.com/embed/dQw4w9WgXcQ"),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_music_url_resolves_to_max_res_thumbnail() {
        assert_eq!(
            synthesize_image_url_from_url("https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RD1"),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_live_url_resolves_to_max_res_thumbnail() {
        assert_eq!(
            synthesize_image_url_from_url("https://www.youtube.com/live/dQw4w9WgXcQ"),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_id_must_be_eleven_characters_from_canonical_alphabet() {
        // Wrong length.
        assert_eq!(synthesize_image_url_from_url("https://www.youtube.com/watch?v=tooShort"), None,);
        // Forbidden character (`.`) in the id segment.
        assert_eq!(
            synthesize_image_url_from_url("https://www.youtube.com/watch?v=dQw4w9WgX.Q"),
            None,
        );
    }

    #[test]
    fn youtube_watch_url_without_v_param_falls_through() {
        assert_eq!(synthesize_image_url_from_url("https://www.youtube.com/watch?list=PLfoo"), None,);
    }

    #[test]
    fn unrelated_url_returns_none() {
        assert_eq!(synthesize_image_url_from_url("https://example.com/article"), None,);
    }

    #[test]
    fn malformed_url_returns_none() {
        assert_eq!(synthesize_image_url_from_url("not a url"), None);
    }

    #[test]
    fn bilibili_bv_id_extracts_correctly() {
        assert_eq!(
            bilibili_video_id("https://www.bilibili.com/video/BV1xx411c7m1"),
            Some(BilibiliId::Bv("BV1xx411c7m1".into())),
        );
    }

    #[test]
    fn bilibili_av_id_extracts_correctly() {
        assert_eq!(
            bilibili_video_id("https://www.bilibili.com/video/av170001"),
            Some(BilibiliId::Av("170001".into())),
        );
    }

    #[test]
    fn bilibili_id_extractor_rejects_unknown_shape() {
        assert!(bilibili_video_id("https://www.bilibili.com/video/notReal").is_none());
        assert!(bilibili_video_id("https://www.bilibili.com/").is_none());
        assert!(bilibili_video_id("https://example.com/video/BV1xx411c7m1").is_none());
        assert!(parse_bilibili_bv("BV1xx411c7m!").is_none());
        assert!(parse_bilibili_av("av").is_none());
        assert!(parse_bilibili_av("foo123").is_none());
    }

    #[test]
    fn extract_bilibili_pic_handles_valid_response() {
        let body = br#"{"code":0,"data":{"pic":"https://i0.hdslb.com/cover/foo.jpg","other":1}}"#;
        assert_eq!(
            extract_bilibili_pic_field(body),
            Some("https://i0.hdslb.com/cover/foo.jpg".into()),
        );
    }

    #[test]
    fn extract_bilibili_pic_rejects_missing_or_blank_fields() {
        assert_eq!(extract_bilibili_pic_field(br#"{"code":-1,"data":{}}"#), None,);
        assert_eq!(extract_bilibili_pic_field(br#"{"code":0,"data":{"pic":"  "}}"#), None,);
        assert_eq!(extract_bilibili_pic_field(br#"{"code":0,"data":{"pic":42}}"#), None,);
        assert_eq!(extract_bilibili_pic_field(b"not json"), None,);
        assert_eq!(extract_bilibili_pic_field(br#"{"code":0}"#), None,);
    }

    #[test]
    fn host_requires_synthesis_recognises_video_hosts() {
        assert!(host_requires_synthesis("https://www.youtube.com/watch?v=abc"));
        assert!(host_requires_synthesis("https://youtu.be/abc"));
        assert!(host_requires_synthesis("https://music.youtube.com/watch?v=abc"));
        assert!(host_requires_synthesis("https://www.bilibili.com/video/BV1xx411c7m1"));
        assert!(!host_requires_synthesis("https://example.com/article"));
    }

    #[test]
    fn resolve_image_url_via_api_returns_none_for_non_bilibili_hosts() {
        let client = reqwest::blocking::Client::new();
        assert!(resolve_image_url_via_api(&client, "https://example.com/").is_none());
    }

    #[test]
    fn resolve_image_url_via_api_success_path_returns_pic_url() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", mockito::Matcher::Regex("/x/web-interface/view.*".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(br#"{"data":{"pic":"https://i0.hdslb.com/cover/foo.jpg"}}"#)
            .create();
        let client = test_client();
        let result = resolve_image_url_via_api_with_base(
            &client,
            "https://www.bilibili.com/video/BV1xx411c7m1",
            &server.url(),
        );
        assert_eq!(result.as_deref(), Some("https://i0.hdslb.com/cover/foo.jpg"));
    }

    #[test]
    fn resolve_image_url_via_api_returns_none_on_http_error() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", mockito::Matcher::Regex("/x/web-interface/view.*".into()))
            .with_status(500)
            .with_body(br#"{"error":true}"#)
            .create();
        let client = test_client();
        let result = resolve_image_url_via_api_with_base(
            &client,
            "https://www.bilibili.com/video/BV1xx411c7m1",
            &server.url(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn resolve_image_url_via_api_returns_none_when_body_exceeds_cap() {
        let mut server = mockito::Server::new();
        let big = vec![b'x'; 100 * 1024];
        let _mock = server
            .mock("GET", mockito::Matcher::Regex("/x/web-interface/view.*".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(big)
            .create();
        let client = test_client();
        let result = resolve_image_url_via_api_with_base(
            &client,
            "https://www.bilibili.com/video/BV1xx411c7m1",
            &server.url(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn resolve_image_url_via_api_with_av_id_uses_aid_query_param() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", mockito::Matcher::Regex("aid=170001".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(br#"{"data":{"pic":"https://i0.hdslb.com/cover/av.jpg"}}"#)
            .create();
        let client = test_client();
        let result = resolve_image_url_via_api_with_base(
            &client,
            "https://www.bilibili.com/video/av170001",
            &server.url(),
        );
        assert_eq!(result.as_deref(), Some("https://i0.hdslb.com/cover/av.jpg"));
    }

    #[test]
    fn resolve_image_url_via_api_strips_trailing_slash_from_base() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", mockito::Matcher::Regex("/x/web-interface/view.*".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(br#"{"data":{"pic":"https://i0.hdslb.com/cover/x.jpg"}}"#)
            .create();
        let client = test_client();
        let base_with_slash = format!("{}/", server.url());
        let result = resolve_image_url_via_api_with_base(
            &client,
            "https://www.bilibili.com/video/BV1xx411c7m1",
            &base_with_slash,
        );
        assert_eq!(result.as_deref(), Some("https://i0.hdslb.com/cover/x.jpg"));
    }

    #[test]
    fn resolve_image_url_via_api_returns_none_on_transport_error() {
        // Point the client at an unroutable address so reqwest fails
        // before producing a response. Covers the `response.send` failure
        // branch in the production pipeline.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(80))
            .build()
            .expect("test client");
        let result = resolve_image_url_via_api_with_base(
            &client,
            "https://www.bilibili.com/video/BV1xx411c7m1",
            "http://127.0.0.1:1",
        );
        assert!(result.is_none());
    }

    fn test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("test client")
    }

    // ── Additional edge-case coverage ────────────────────────────────

    #[test]
    fn youtube_watch_url_with_extra_query_params_still_resolves() {
        assert_eq!(
            synthesize_image_url_from_url(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42&feature=youtu.be",
            ),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg".into()),
        );
    }

    #[test]
    fn youtube_watch_url_v_param_first_wins_over_later_v_params() {
        // Some redirects produce two `v=` params; URL crate iterates in
        // order so the first one we see is canonical. Pin that — if a
        // future refactor switches to a HashMap-based extractor, the
        // order assumption breaks.
        let id = synthesize_image_url_from_url(
            "https://www.youtube.com/watch?v=aaaaaaaaaaa&v=bbbbbbbbbbb",
        );
        assert_eq!(id, Some("https://i.ytimg.com/vi/aaaaaaaaaaa/maxresdefault.jpg".into()),);
    }

    #[test]
    fn youtube_id_with_underscore_or_dash_is_accepted() {
        for id in ["AaB_cDeFgHi", "AaB-cDeFgHi", "_aBcDeFgHij"] {
            let url = format!("https://www.youtube.com/watch?v={id}");
            assert_eq!(
                synthesize_image_url_from_url(&url),
                Some(format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg")),
                "id {id} should be valid",
            );
        }
    }

    #[test]
    fn youtube_id_with_space_or_plus_is_rejected() {
        // YouTube ids never contain spaces or `+`; rejecting these
        // prevents a corrupt query like `?v=abc def` from synthesising
        // a broken image URL.
        for id in ["abc def0123", "abc+def0123"] {
            let url = format!("https://www.youtube.com/watch?v={id}");
            assert_eq!(synthesize_image_url_from_url(&url), None, "id {id} must be rejected",);
        }
    }

    #[test]
    fn youtube_url_with_uppercase_scheme_still_parses() {
        // reqwest::Url normalises the scheme to lower-case during parse;
        // the synth call should not care about input casing.
        assert!(
            synthesize_image_url_from_url("HTTPS://www.youtube.com/watch?v=dQw4w9WgXcQ").is_some(),
        );
    }

    #[test]
    fn youtube_url_without_host_returns_none() {
        // `https://watch?v=abc` parses but has no host_str()
        // → host extractor returns None.
        assert_eq!(synthesize_image_url_from_url("https:///watch?v=abc"), None);
    }

    #[test]
    fn youtube_shorts_with_trailing_slash_or_query_is_handled() {
        assert!(
            synthesize_image_url_from_url("https://www.youtube.com/shorts/dQw4w9WgXcQ/",).is_some(),
        );
        assert!(
            synthesize_image_url_from_url(
                "https://www.youtube.com/shorts/dQw4w9WgXcQ?si=tracking",
            )
            .is_some(),
        );
    }

    #[test]
    fn youtube_user_channel_paths_are_not_videos() {
        // Channel / user / playlist URLs are not videos — synth must
        // refuse so the worker does not POST a fake thumbnail URL.
        for url in [
            "https://www.youtube.com/channel/UC1234567890123456789012",
            "https://www.youtube.com/@somecreator",
            "https://www.youtube.com/playlist?list=PLfoo",
        ] {
            assert_eq!(synthesize_image_url_from_url(url), None, "URL {url} should not synthesize",);
        }
    }

    #[test]
    fn bilibili_bv_id_with_uppercase_letters_in_url_works() {
        // The path segment is case-sensitive for BV ids; reqwest::Url
        // does NOT lowercase the path. Confirm both upper-and-lower
        // case BV alphanumeric ids are accepted.
        assert_eq!(
            bilibili_video_id("https://www.bilibili.com/video/BV1Xx411c7m1"),
            Some(BilibiliId::Bv("BV1Xx411c7m1".into())),
        );
    }

    #[test]
    fn bilibili_av_id_with_trailing_slash_and_query_is_handled() {
        assert_eq!(
            bilibili_video_id("https://www.bilibili.com/video/av170001/"),
            Some(BilibiliId::Av("170001".into())),
        );
        assert_eq!(
            bilibili_video_id("https://www.bilibili.com/video/av170001?p=2"),
            Some(BilibiliId::Av("170001".into())),
        );
    }

    #[test]
    fn bilibili_subdomains_other_than_m_or_www_are_not_recognised_yet() {
        // live.bilibili.com / space.bilibili.com / search.bilibili.com
        // are NOT video pages — the synth must skip them so the worker
        // falls back to the generic scraper (which may legitimately
        // find an og:image for a streamer's live banner).
        for url in [
            "https://live.bilibili.com/12345",
            "https://space.bilibili.com/12345",
            "https://search.bilibili.com/?keyword=test",
        ] {
            assert!(
                bilibili_video_id(url).is_none(),
                "URL {url} should not match the video synth path",
            );
        }
    }

    #[test]
    fn bilibili_short_link_host_not_recognised() {
        // `b23.tv/{shortid}` redirects to canonical video URL on visit;
        // until we have a HEAD redirect resolver, the synth must NOT
        // pretend the short id is a BV id.
        assert_eq!(bilibili_video_id("https://b23.tv/abcd1234"), None);
    }

    #[test]
    fn extract_bilibili_pic_field_handles_nested_data_object() {
        // The actual API also returns `code`, `message`, `ttl` siblings.
        // The extractor only cares about `data.pic` and ignores other
        // payload keys.
        let body = br#"{
          "code": 0,
          "message": "0",
          "ttl": 1,
          "data": {
            "bvid": "BV1xx411c7m1",
            "pic": "https://i0.hdslb.com/cover/foo.jpg",
            "title": "test"
          }
        }"#;
        assert_eq!(
            extract_bilibili_pic_field(body),
            Some("https://i0.hdslb.com/cover/foo.jpg".into()),
        );
    }

    #[test]
    fn extract_bilibili_pic_field_strips_surrounding_whitespace() {
        let body = br#"{"data":{"pic":"  https://i0.hdslb.com/cover/x.jpg  "}}"#;
        assert_eq!(
            extract_bilibili_pic_field(body),
            Some("https://i0.hdslb.com/cover/x.jpg".into()),
        );
    }

    #[test]
    fn extract_bilibili_pic_field_rejects_arrays_and_nulls() {
        assert_eq!(extract_bilibili_pic_field(br#"{"data":{"pic":null}}"#), None,);
        assert_eq!(extract_bilibili_pic_field(br#"{"data":{"pic":[]}}"#), None,);
        // data itself missing
        assert_eq!(extract_bilibili_pic_field(br#"{"code":0,"message":"ok"}"#), None,);
    }

    #[test]
    fn host_requires_synthesis_is_case_insensitive() {
        assert!(host_requires_synthesis("HTTPS://WWW.YOUTUBE.COM/watch?v=abc"));
        assert!(host_requires_synthesis("https://M.bilibili.com/video/BV1xx411c7m1",));
    }

    #[test]
    fn host_requires_synthesis_rejects_subdomain_lookalikes() {
        // `youtubex.com` / `notyoutube.com` must not be mistaken for
        // YouTube hosts even though they contain the substring.
        assert!(!host_requires_synthesis("https://youtubex.com/watch?v=abc"));
        assert!(!host_requires_synthesis("https://notyoutube.com/"));
        assert!(!host_requires_synthesis("https://bilibili.com.attacker/"));
    }
}
