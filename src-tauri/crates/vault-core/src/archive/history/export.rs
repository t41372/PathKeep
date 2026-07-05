//! History export artifact rendering.
//!
//! ## Responsibilities
//! - Walk the full visible history result set for an export request.
//! - Render JSONL, HTML, Markdown, and plain-text artifacts.
//! - Write export artifacts under the configured exports directory.
//!
//! ## Not responsible for
//! - Choosing which history rows are visible.
//! - Running SQL or lexical recall directly.
//! - Streaming very large exports; this module preserves the current paged
//!   collection contract until a dedicated export-performance slice changes it.
//!
//! ## Dependencies
//! - The public history facade for cursor-based page walking.
//! - Archive export models and the configured project exports directory.
//!
//! ## Performance notes
//! - Export walks history in 1000-row pages so the query layer remains bounded,
//!   but the final artifact is still materialized as one string as before.

use super::list_history;
use crate::{
    config::ProjectPaths,
    models::{AppConfig, ExportFormat, ExportRequest, ExportResult, HistoryQueryResponse},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use std::fs;

/// Lets the archive export command reuse the exact history visibility contract
/// as Explorer while producing a durable local artifact.
pub fn export_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    let results = collect_history_for_export(paths, config, key, request.query)?;
    fs::create_dir_all(&paths.exports_dir)?;
    let format = request.format;
    let extension = match format {
        ExportFormat::Html => "html",
        ExportFormat::Markdown => "md",
        ExportFormat::Text => "txt",
        ExportFormat::Jsonl => "jsonl",
    };
    let file_name = format!("export-{}.{}", now_rfc3339().replace(':', "-"), extension);
    let target_path = paths.exports_dir.join(file_name);
    let content = render_export_content(&results, &format)?;
    fs::write(&target_path, content)
        .with_context(|| format!("writing {}", target_path.display()))?;
    Ok(ExportResult { format, path: target_path.display().to_string(), count: results.items.len() })
}

/// Re-queries history until all visible matches are collected for export.
///
/// Note: in regex recall mode the underlying `list_history` path scans a bounded
/// window (`REGEX_SCAN_CAP`, 50k rows) rather than the whole visits table, so a
/// regex export with more than ~50k matches emits only the matches inside that
/// window. This is the same bound the Browse/Search surfaces honour and is
/// strictly safer than the former unbounded scan (which would OOM on a large
/// archive before producing any output); narrow the date/profile/domain filter
/// to export a regex result set larger than the window.
fn collect_history_for_export(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: crate::models::HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let mut export_query = query;
    // Export should always walk the full visible result set, not stay pinned to
    // whichever UI page happened to be open when the user clicked export.
    export_query.page = None;
    export_query.cursor = None;
    export_query.limit = Some(1_000);

    let mut items = Vec::new();

    let total = loop {
        let page = list_history(paths, config, key, export_query.clone())?;
        let total = page.total;
        let next_cursor = page.next_cursor.clone();
        items.extend(page.items);

        let Some(next_cursor) = next_cursor else {
            break total;
        };

        export_query.cursor = Some(next_cursor);
    };

    Ok(HistoryQueryResponse {
        total,
        page: 1,
        page_size: items.len(),
        page_count: 1,
        has_previous: false,
        has_next: false,
        items,
        next_cursor: None,
    })
}

/// Renders one export artifact in the requested output format.
fn render_export_content(results: &HistoryQueryResponse, format: &ExportFormat) -> Result<String> {
    Ok(match format {
        ExportFormat::Html => render_html_export(results),
        ExportFormat::Markdown => render_markdown_export(results),
        ExportFormat::Text => render_text_export(results),
        ExportFormat::Jsonl => results
            .items
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n"),
    })
}

/// Renders export output as a minimal HTML artifact.
///
/// Page titles and URLs originate from imported browser history and are fully
/// attacker-controlled (a visited page chooses its own `<title>`), so every
/// interpolated value is HTML-escaped and the link target is scheme-filtered.
/// Without this a crafted title such as `<img src=x onerror=…>` would execute
/// when the user opens the exported report in a browser.
fn render_html_export(results: &HistoryQueryResponse) -> String {
    let body = results
        .items
        .iter()
        .map(|item| {
            let title = crate::utils::escape_html(item.title.as_deref().unwrap_or(&item.url));
            let visited_at = crate::utils::escape_html(&item.visited_at);
            let link_text = crate::utils::escape_html(&item.url);
            let href = safe_href(&item.url);
            format!(
                "<article><h2>{title}</h2><p><a href=\"{href}\">{link_text}</a></p><p>{visited_at}</p></article>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<html><body>{body}</body></html>")
}

/// Returns an escaped, scheme-filtered `href` for the export anchor. Only
/// `http(s)`/`mailto` targets are linkable; anything else (e.g. `javascript:`)
/// collapses to `#` so the report can never carry an executable link.
fn safe_href(url: &str) -> String {
    let scheme = url.trim_start().to_ascii_lowercase();
    if scheme.starts_with("http://")
        || scheme.starts_with("https://")
        || scheme.starts_with("mailto:")
    {
        crate::utils::escape_html(url)
    } else {
        "#".to_string()
    }
}

/// Renders export output as Markdown.
fn render_markdown_export(results: &HistoryQueryResponse) -> String {
    results
        .items
        .iter()
        .map(|item| {
            format!(
                "- [{}]({}) — {}",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.url,
                item.visited_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders export output as plain text.
fn render_text_export(results: &HistoryQueryResponse) -> String {
    results
        .items
        .iter()
        .map(|item| {
            format!(
                "{}\n{}\n{}\n",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.url,
                item.visited_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HistoryEntry;

    fn entry(url: &str, title: Option<&str>, visited_at: &str) -> HistoryEntry {
        HistoryEntry {
            id: 1,
            profile_id: "profile".to_string(),
            url: url.to_string(),
            title: title.map(str::to_string),
            domain: "example.com".to_string(),
            favicon: None,
            visited_at: visited_at.to_string(),
            visit_time: 0,
            duration_ms: None,
            transition: None,
            source_visit_id: 0,
            app_id: None,
            enrichment_excerpt: None,
        }
    }

    #[test]
    fn html_export_escapes_untrusted_fields_and_neutralizes_dangerous_hrefs() {
        let response = HistoryQueryResponse {
            items: vec![
                entry(
                    "https://example.com/?a=1&b=2",
                    Some("<img src=x onerror=\"alert('xss')\">"),
                    "2024-01-01T00:00:00+00:00",
                ),
                entry("javascript:alert(1)", Some("evil"), "<b>when</b>"),
            ],
            total: 2,
            ..HistoryQueryResponse::default()
        };
        let html = render_html_export(&response);

        // Title markup is escaped, never emitted as live tags.
        assert!(html.contains("&lt;img src=x onerror=&quot;alert(&#39;xss&#39;)&quot;&gt;"));
        assert!(!html.contains("<img src=x"));
        // The ampersand in the URL is escaped in href and link text.
        assert!(html.contains("https://example.com/?a=1&amp;b=2"));
        // A javascript: URL is neutralized to '#'; no executable href survives.
        assert!(html.contains("href=\"#\""));
        assert!(!html.contains("href=\"javascript:"));
        // The visited_at field is escaped too.
        assert!(html.contains("&lt;b&gt;when&lt;/b&gt;"));
    }
}
