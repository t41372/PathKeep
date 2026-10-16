//! Responsibilities: select visit-independent page text and readable URL words for embedding.
//! Not responsible for: model prompts, dedup identity, fetching content or persistence.

use super::{cap_embedding_text, normalize_whitespace, percent_decode};

/// Keeps the title separate so models with a document template can place it correctly.
pub(crate) struct EmbeddingDocument {
    pub title: Option<String>,
    pub body: String,
}

/// A shared page vector must describe the page regardless of which visit first indexed it.
pub(crate) fn build_embedding_document(
    url: &str,
    browser_title: Option<&str>,
    readable_title: Option<&str>,
    summary: Option<&str>,
    readable_body: Option<&str>,
) -> EmbeddingDocument {
    let title = browser_title
        .filter(|value| !value.trim().is_empty())
        .or(readable_title)
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty());
    let mut body = page_location(url);
    if let Some(readable_title) = readable_title.filter(|value| !value.trim().is_empty())
        && !same_text(Some(readable_title), title.as_deref())
    {
        append_line(&mut body, readable_title.trim());
    }
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty())
        && !same_text(Some(summary), title.as_deref())
        && !same_text(Some(summary), readable_title)
    {
        append_line(&mut body, summary.trim());
    }
    if let Some(text) = readable_body.filter(|value| !value.trim().is_empty()) {
        append_line(&mut body, &cap_embedding_text(text.trim()));
    }
    EmbeddingDocument { title, body }
}

fn append_line(body: &mut String, line: &str) {
    if !body.is_empty() {
        body.push('\n');
    }
    body.push_str(line);
}

fn same_text(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left
            .split_whitespace()
            .flat_map(|word| word.chars().flat_map(char::to_lowercase).chain([' ']))
            .eq(right
                .split_whitespace()
                .flat_map(|word| word.chars().flat_map(char::to_lowercase).chain([' ']))),
        _ => false,
    }
}

fn page_location(url: &str) -> String {
    let Ok(url) = reqwest::Url::parse(url) else { return String::new() };
    let host = url.host_str().unwrap_or_default();
    let mut line = host.strip_prefix("www.").unwrap_or(host).to_ascii_lowercase();
    let path = if url.path().contains('%') {
        std::borrow::Cow::Owned(percent_decode(url.path()))
    } else {
        std::borrow::Cow::Borrowed(url.path())
    };
    let mut segments = path.split('/').filter(|segment| !segment.is_empty()).peekable();
    let mut count = 0;
    while let Some(segment) = segments.next() {
        let segment = if segments.peek().is_none() {
            match segment.rsplit_once('.') {
                Some((stem, extension))
                    if ["html", "htm", "shtml", "php", "asp", "aspx", "jsp"]
                        .iter()
                        .any(|known| extension.eq_ignore_ascii_case(known)) =>
                {
                    stem
                }
                _ => segment,
            }
        } else {
            segment
        };
        // Reject UUIDs before splitting on hyphens, which would leave short hex fragments.
        if is_uuid(segment) {
            continue;
        }
        for word in segment
            .split(|ch: char| matches!(ch, '-' | '_' | '.' | '+' | '~' | ',') || ch.is_whitespace())
        {
            if !word.chars().any(char::is_alphabetic)
                || (word.len() >= 8 && word.bytes().all(|byte| byte.is_ascii_hexdigit()))
            {
                continue;
            }
            if count == 0 && !line.is_empty() {
                line.push_str(" · ");
            } else if count != 0 {
                line.push(' ');
            }
            line.push_str(word);
            count += 1;
            if count == 16 {
                return line;
            }
        }
    }
    line
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) { byte == b'-' } else { byte.is_ascii_hexdigit() }
        })
}

#[cfg(test)]
#[path = "embedding_document_tests.rs"]
mod tests;
