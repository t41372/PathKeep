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
fn render_html_export(results: &HistoryQueryResponse) -> String {
    let body = results
        .items
        .iter()
        .map(|item| {
            format!(
                "<article><h2>{}</h2><p><a href=\"{url}\">{url}</a></p><p>{}</p></article>",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.visited_at,
                url = item.url,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<html><body>{body}</body></html>")
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
