//! Responsibilities: pin readable URL words and visit-independent document parts.
//! Not responsible for: model prompts, fetching or vector storage.
//! Failure modes: numeric, hex and UUID ids survive; escaped CJK breaks; empty paths,
//! trailing slashes or extensions add noise; query/fragment leaks; IDN changes; long paths
//! exceed the token limit; escaped whitespace breaks the location line; blank or repeated titles and summaries dilute the document.
//! A title, readable title, summary or path word of any length goes out whole; a cut lands inside a
//! character; short fields push the page past its budget; cutting changes which fields repeat.
use super::*;

#[test]
fn path_words_keep_page_meaning_without_identifiers() {
    for (url, expected) in [
        ("https://www.strava.com/activities/1001", "strava.com · activities"),
        (
            "https://EXAMPLE.com/a/123/abcdef12/550e8400-e29b-41d4-a716-446655440000/end",
            "example.com · a end",
        ),
        ("https://example.com/%E4%B8%AD%E6%96%87/%E6%90%9C%E5%B0%8B", "example.com · 中文 搜尋"),
        ("https://example.com", "example.com"),
        ("https://example.com/", "example.com"),
        ("https://example.com/word/", "example.com · word"),
        ("https://example.com/one%20two%0Athree", "example.com · one two three"),
        ("https://example.com/550e8400-e29b-41d4-a716-446655440000.html", "example.com"),
        (
            "https://example.com/a-b_c.d+e~f,g/page.HTML?q=secret#hidden",
            "example.com · a b c d e f g page",
        ),
        ("https://xn--bcher-kva.example/read", "xn--bcher-kva.example · read"),
        ("not a URL?secret#hidden", ""),
    ] {
        assert_eq!(page_location(url), expected, "{url}");
    }
    for extension in ["html", "htm", "shtml", "php", "asp", "aspx", "jsp"] {
        assert_eq!(
            page_location(&format!("https://example.com/page.{extension}")),
            "example.com · page"
        );
    }
    assert_eq!(
        page_location("https://example.com/html/page.html/next"),
        "example.com · html page html next"
    );
    let long = format!("https://example.com/{}", "word/".repeat(100));
    assert_eq!(page_location(&long), format!("example.com · {}", vec!["word"; 16].join(" ")));
}

#[test]
fn document_parts_prefer_browser_title_and_avoid_repeated_labels() {
    let document = build_embedding_document(
        "https://www.example.com/read/123",
        Some("  Browser title  "),
        Some("Readable title"),
        Some(" readable   TITLE "),
        Some(" Body "),
    );
    assert_eq!(document.title.as_deref(), Some("Browser title"));
    assert_eq!(document.body, "example.com · read\nReadable title\nBody");
    let document = build_embedding_document(
        "https://example.com",
        Some(" \n "),
        Some(" Readable   title "),
        Some("READABLE title"),
        None,
    );
    assert_eq!(document.title.as_deref(), Some("Readable title"));
    assert_eq!(document.body, "example.com");
    let document = build_embedding_document(
        "https://example.com",
        Some("Title"),
        Some(" title "),
        Some("Summary"),
        Some(&"中".repeat(3000)),
    );
    // The page has one budget: the body gets what the title, site and summary leave of it.
    let room = crate::enrichment::EMBEDDING_TEXT_CHAR_BUDGET
        - "Title".len()
        - "example.com\nSummary".len();
    assert_eq!(document.body, format!("example.com\nSummary\n{}", "中".repeat(room)));
    assert!(
        build_embedding_document("https://example.com", None, None, None, None).title.is_none()
    );
}

#[test]
fn page_text_stays_within_its_budget() {
    let budget = crate::enrichment::EMBEDDING_TEXT_CHAR_BUDGET;
    let huge = "標題".repeat(100_000);
    let long_word = format!("https://example.com/{}", "a".repeat(10_000));
    let document = build_embedding_document(
        &long_word,
        Some(&huge),
        Some(&format!("{huge} readable")),
        Some(&"摘要".repeat(50_000)),
        Some(&"正文".repeat(50_000)),
    );
    let title = document.title.as_deref().expect("title");
    assert_eq!(title.chars().count(), SHORT_FIELD_CHAR_CAP);
    assert!(title.chars().all(|c| c == '標' || c == '題'), "cut on a character boundary");
    let lines: Vec<&str> = document.body.lines().collect();
    assert_eq!(lines.len(), 4, "location, readable title, summary, body");
    for line in &lines[..3] {
        assert!(line.chars().count() <= SHORT_FIELD_CHAR_CAP, "{}", line.chars().count());
    }
    let total = title.chars().count() + document.body.chars().count();
    assert!(total <= budget + lines.len(), "{total} characters for a {budget} budget");
    // The full readable title differs from the title even though their first 256 characters match.
    assert!(lines[1].starts_with('標'));
    // Short fields can use the whole budget; then no body is sent rather than a cut one past it.
    let crowded = build_embedding_document(
        "https://example.com/",
        Some(&"t".repeat(300)),
        Some(&"r".repeat(300)),
        Some(&"s".repeat(300)),
        Some("body"),
    );
    assert!(crowded.body.ends_with("body"));
    assert!(crowded.title.unwrap().chars().count() + crowded.body.chars().count() <= budget + 4);
}
