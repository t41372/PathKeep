//! Responsibilities: select visit-independent page text and readable URL words for embedding.
//! Not responsible for: model prompts, dedup identity, fetching content or persistence.

use super::{EMBEDDING_TEXT_CHAR_BUDGET, normalize_whitespace, percent_decode};

/// Longest title, readable title, summary or address line sent to the model, in characters. Real
/// ones are far shorter, but a page can put a whole article in its `<title>`, and one such text
/// can fail every request of its batch at a service with a small context window.
const SHORT_FIELD_CHAR_CAP: usize = 256;

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
    let mut body = cap_chars(&page_location(url), SHORT_FIELD_CHAR_CAP).to_owned();
    // Repeats are judged on the whole texts; only what is sent is cut.
    if let Some(readable_title) = readable_title.filter(|value| !value.trim().is_empty())
        && !same_text(Some(readable_title), title.as_deref())
    {
        append_line(&mut body, cap_chars(readable_title.trim(), SHORT_FIELD_CHAR_CAP));
    }
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty())
        && !same_text(Some(summary), title.as_deref())
        && !same_text(Some(summary), readable_title)
    {
        append_line(&mut body, cap_chars(summary.trim(), SHORT_FIELD_CHAR_CAP));
    }
    let title = title.map(|title| cap_chars(&title, SHORT_FIELD_CHAR_CAP).to_owned());
    // The body gets what the short fields leave of the page's budget.
    let used = title.as_deref().map_or(0, |title| title.chars().count()) + body.chars().count();
    let room = EMBEDDING_TEXT_CHAR_BUDGET.saturating_sub(used);
    if let Some(text) = readable_body.filter(|value| !value.trim().is_empty())
        && room > 0
    {
        append_line(&mut body, cap_chars(text.trim(), room));
    }
    EmbeddingDocument { title, body }
}

/// The first `limit` characters of `text`, cut on a character boundary, without trailing space.
fn cap_chars(text: &str, limit: usize) -> &str {
    match text.char_indices().nth(limit) {
        Some((end, _)) => text[..end].trim_end(),
        None => text,
    }
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
