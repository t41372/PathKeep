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
        .map(|line| normalize_whitespace(line))
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

fn normalize_whitespace(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
        } else {
            output.push(ch);
            last_was_space = false;
        }
    }
    output.trim().to_string()
}

fn format_duration(input: String) -> String {
    if !input.starts_with('P') {
        return input;
    }

    let mut hours = 0u32;
    let mut minutes = 0u32;
    let mut seconds = 0u32;
    let mut buffer = String::new();

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            buffer.push(ch);
            continue;
        }

        let value = buffer.parse::<u32>().unwrap_or_default();
        match ch {
            'H' => hours = value,
            'M' => minutes = value,
            'S' => seconds = value,
            _ => {}
        }
        buffer.clear();
    }

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 {
        parts.push(format!("{seconds}s"));
    }

    if parts.is_empty() { input } else { parts.join(" ") }
}

#[cfg(test)]
mod tests {
    use super::adapt_site_content;
    use scraper::Html;

    #[test]
    fn extracts_youtube_video_metadata_from_json_ld() {
        let html = Html::parse_document(
            r#"
            <html>
              <head>
                <script type="application/ld+json">
                  {
                    "@context": "https://schema.org",
                    "@type": "VideoObject",
                    "name": "PathKeep walkthrough",
                    "description": "A detailed desktop tour.",
                    "duration": "PT12M31S",
                    "uploadDate": "2026-04-01",
                    "author": { "@type": "Person", "name": "Tim" }
                  }
                </script>
              </head>
            </html>
            "#,
        );

        let result = adapt_site_content("https://www.youtube.com/watch?v=abc123", &html)
            .expect("youtube adapter result");
        assert_eq!(result.adapter_id, "youtube-video");
        assert_eq!(result.readable_title.as_deref(), Some("PathKeep walkthrough"));
        assert!(
            result
                .readable_text
                .as_deref()
                .is_some_and(|value| value.contains("Channel: Tim | Duration: 12m 31s"))
        );
        assert_eq!(result.metadata["videoId"].as_str(), Some("abc123"));
    }

    #[test]
    fn falls_back_to_meta_tags_for_vimeo_pages() {
        let html = Html::parse_document(
            r#"
            <html>
              <head>
                <meta property="og:title" content="Archive replay" />
                <meta property="og:description" content="Comparing backup flows." />
                <meta itemprop="author" content="PathKeep Studio" />
              </head>
            </html>
            "#,
        );

        let result =
            adapt_site_content("https://vimeo.com/123456789", &html).expect("vimeo adapter result");
        assert_eq!(result.adapter_id, "vimeo-video");
        assert_eq!(result.readable_title.as_deref(), Some("Archive replay"));
        assert!(
            result
                .readable_text
                .as_deref()
                .is_some_and(|value| value.contains("Comparing backup flows."))
        );
        assert_eq!(result.metadata["videoId"].as_str(), Some("123456789"));
    }
}
