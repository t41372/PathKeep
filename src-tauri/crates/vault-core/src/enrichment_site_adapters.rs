//! Site-specific enrichment adapters used by readable-content enrichment.

use crate::utils::url_domain;
use scraper::{Html, Selector};
use serde_json::{Value, json};

const SNIPPET_LIMIT: usize = 3;

#[derive(Debug, Clone, Default)]
pub(crate) struct SiteAdapterResult {
    pub adapter_id: &'static str,
    pub readable_title: Option<String>,
    pub readable_text: Option<String>,
    pub snippets: Vec<String>,
    pub metadata: Value,
}

/// Applies any known site-specific readability adapter to fetched HTML content.
pub(crate) fn adapt_site_content(url: &str, document: &Html) -> Option<SiteAdapterResult> {
    let domain = url_domain(url);
    if domain.contains("youtube.com") || domain.contains("youtu.be") {
        adapt_video_site(document, url, "youtube-video")
    } else if domain.contains("vimeo.com") {
        adapt_video_site(document, url, "vimeo-video")
    } else {
        None
    }
}

fn adapt_video_site(
    document: &Html,
    url: &str,
    adapter_id: &'static str,
) -> Option<SiteAdapterResult> {
    let metadata = extract_video_metadata(document);
    let title = metadata
        .get("title")
        .and_then(Value::as_str)
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty());
    let description = metadata
        .get("description")
        .and_then(Value::as_str)
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty());
    let channel = metadata
        .get("channel")
        .and_then(Value::as_str)
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty());
    let duration = metadata
        .get("duration")
        .and_then(Value::as_str)
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty());
    let published_at = metadata
        .get("publishedAt")
        .and_then(Value::as_str)
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty());

    if title.is_none() && description.is_none() {
        return None;
    }

    let mut lines = Vec::new();
    if let Some(title) = title.as_ref() {
        lines.push(format!("Video: {title}"));
    }

    let mut context = Vec::new();
    if let Some(channel) = channel.as_ref() {
        context.push(format!("Channel: {channel}"));
    }
    if let Some(duration) = duration.as_ref() {
        context.push(format!("Duration: {duration}"));
    }
    if let Some(published_at) = published_at.as_ref() {
        context.push(format!("Published: {published_at}"));
    }
    if !context.is_empty() {
        lines.push(context.join(" | "));
    }

    if let Some(description) = description.as_ref() {
        lines.push(description.clone());
    }

    let snippets = lines
        .iter()
        .map(normalize_whitespace)
        .filter(|line| !line.is_empty())
        .take(SNIPPET_LIMIT)
        .collect::<Vec<_>>();

    Some(SiteAdapterResult {
        adapter_id,
        readable_title: title.clone(),
        readable_text: Some(lines.join("\n")),
        snippets: if snippets.is_empty() { title.iter().cloned().collect() } else { snippets },
        metadata: json!({
            "adapterId": adapter_id,
            "videoId": extract_video_id(url, adapter_id),
            "title": title,
            "description": description,
            "channel": channel,
            "duration": duration,
            "publishedAt": published_at,
        }),
    })
}

fn extract_video_metadata(document: &Html) -> Value {
    if let Some(json_ld) = extract_video_json_ld(document) {
        return json!({
            "title": string_field(&json_ld, "name"),
            "description": string_field(&json_ld, "description"),
            "channel": nested_string_field(&json_ld, &["author", "name"])
                .or_else(|| nested_string_field(&json_ld, &["publisher", "name"])),
            "duration": nested_string_field(&json_ld, &["duration"]).map(format_duration),
            "publishedAt": string_field(&json_ld, "uploadDate"),
        });
    }

    json!({
        "title": first_meta_content(document, &[
            "meta[property='og:title']",
            "meta[name='twitter:title']",
            "meta[itemprop='name']",
            "meta[name='title']",
        ]),
        "description": first_meta_content(document, &[
            "meta[property='og:description']",
            "meta[name='twitter:description']",
            "meta[name='description']",
        ]),
        "channel": first_meta_content(document, &[
            "meta[itemprop='author']",
            "meta[name='author']",
        ]),
        "duration": first_meta_content(document, &[
            "meta[itemprop='duration']",
        ]).map(format_duration),
        "publishedAt": first_meta_content(document, &[
            "meta[itemprop='uploadDate']",
            "meta[property='video:release_date']",
        ]),
    })
}

fn extract_video_json_ld(document: &Html) -> Option<Value> {
    let selector = Selector::parse("script[type='application/ld+json']").ok()?;
    for node in document.select(&selector) {
        let raw = node.text().collect::<Vec<_>>().join(" ");
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed)
            && let Some(video) = find_video_object(&value)
        {
            return Some(video.clone());
        }
    }
    None
}

fn find_video_object(value: &Value) -> Option<&Value> {
    match value {
        Value::Array(items) => items.iter().find_map(find_video_object),
        Value::Object(map) => {
            if type_contains_video(map.get("@type")) {
                return Some(value);
            }

            for key in ["@graph", "mainEntity", "itemListElement"] {
                if let Some(found) = map.get(key).and_then(find_video_object) {
                    return Some(found);
                }
            }

            None
        }
        _ => None,
    }
}

fn type_contains_video(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(value)) => value.eq_ignore_ascii_case("VideoObject"),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.eq_ignore_ascii_case("VideoObject")),
        _ => false,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(normalize_whitespace)
}

fn nested_string_field(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(normalize_whitespace)
}

fn first_meta_content(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|selector| {
        Selector::parse(selector).ok().and_then(|selector| {
            document
                .select(&selector)
                .next()
                .and_then(|node| node.value().attr("content"))
                .map(normalize_whitespace)
                .filter(|value| !value.is_empty())
        })
    })
}

fn extract_video_id(url: &str, adapter_id: &str) -> Option<String> {
    if adapter_id == "youtube-video" {
        if let Some(value) = query_param(url, "v") {
            return Some(value);
        }
        return url
            .split("youtu.be/")
            .nth(1)
            .and_then(|value| value.split(['?', '&', '/']).next())
            .map(ToString::to_string);
    }

    if adapter_id == "vimeo-video" {
        return url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|value| value.chars().all(|ch| ch.is_ascii_digit()))
            .map(ToString::to_string);
    }

    None
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    for pair in query.split('&') {
        let (param_key, param_value) = pair.split_once('=')?;
        if param_key == key {
            let value = normalize_whitespace(param_value);
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn format_duration(value: String) -> String {
    let trimmed = value.trim().to_ascii_uppercase();
    if !trimmed.starts_with('P') {
        return value;
    }

    let mut hours = None;
    let mut minutes = None;
    let mut seconds = None;
    let mut current = String::new();
    for character in trimmed.chars() {
        match character {
            '0'..='9' => current.push(character),
            'H' => {
                hours = current.parse::<u64>().ok();
                current.clear();
            }
            'M' => {
                minutes = current.parse::<u64>().ok();
                current.clear();
            }
            'S' => {
                seconds = current.parse::<u64>().ok();
                current.clear();
            }
            _ => current.clear(),
        }
    }

    let mut parts = Vec::new();
    if let Some(hours) = hours {
        parts.push(format!("{hours}h"));
    }
    if let Some(minutes) = minutes {
        parts.push(format!("{minutes}m"));
    }
    if let Some(seconds) = seconds {
        parts.push(format!("{seconds}s"));
    }

    if parts.is_empty() { value } else { parts.join(" ") }
}

fn normalize_whitespace(value: impl AsRef<str>) -> String {
    value.as_ref().split_whitespace().collect::<Vec<_>>().join(" ")
}
