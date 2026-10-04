//! History export artifact rendering.
//!
//! ## Responsibilities
//! - Walk the full visible history result set for an export request.
//! - Render JSONL, HTML, Markdown, and plain-text artifacts.
//! - Write export artifacts under the configured exports directory, atomically.
//!
//! ## Not responsible for
//! - Choosing which history rows are visible.
//! - Running SQL or lexical recall directly.
//!
//! ## Dependencies
//! - The public history facade for cursor-based page walking.
//! - `durable_io` for the temp-file-then-rename write.
//! - Archive export models and the configured project exports directory.
//!
//! ## Performance notes
//! - Rows are written as each 1,000-row page arrives, so memory holds one page and the write
//!   buffer, whatever the archive size. Holding every row first needed several GB at 14.4M
//!   visits; numbers in `docs/architecture/ipc-performance.md`.
//! - Each page is a cursor query over the visible-time index, so the walk is linear in the
//!   number of exported rows.

use super::list_history;
use crate::{
    config::ProjectPaths,
    durable_io::{atomic_durable_write_with, sweep_temps_older_than},
    models::{AppConfig, ExportFormat, ExportRequest, ExportResult, HistoryEntry, HistoryQuery},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use std::{fs, io::Write, time::Duration};

/// Rows fetched per history page while exporting (the most `list_history` returns).
const EXPORT_PAGE_SIZE: u32 = 1_000;

/// An export temp untouched this long belongs to an export that was killed, not one in progress.
const ABANDONED_EXPORT_AGE: Duration = Duration::from_secs(60 * 60);

/// Lets the archive export command reuse the exact history visibility contract
/// as Explorer while producing a durable local artifact.
pub fn export_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    fs::create_dir_all(&paths.exports_dir)?;
    // A killed export leaves its temp behind, and at full archive size that is gigabytes.
    sweep_temps_older_than(&paths.exports_dir, ABANDONED_EXPORT_AGE)?;
    let format = request.format;
    let extension = match format {
        ExportFormat::Html => "html",
        ExportFormat::Markdown => "md",
        ExportFormat::Text => "txt",
        ExportFormat::Jsonl => "jsonl",
    };
    let file_name = format!("export-{}.{}", now_rfc3339().replace(':', "-"), extension);
    let target_path = paths.exports_dir.join(file_name);
    let mut count = 0;
    atomic_durable_write_with(&target_path, |writer| {
        let mut export = ExportWriter::new(writer, &format)?;
        walk_history_for_export(paths, config, key, request.query, |item| export.item(item))?;
        count = export.finish()?;
        Ok(())
    })
    .with_context(|| format!("writing {}", target_path.display()))?;
    Ok(ExportResult { format, path: target_path.display().to_string(), count })
}

/// Pages through every visible match for `query` and hands each row to `visit`, one page in
/// memory at a time.
///
/// Note: in regex recall mode the underlying `list_history` path scans a bounded
/// window (`REGEX_SCAN_CAP`, 50k rows) rather than the whole visits table, so a
/// regex export with more than ~50k matches emits only the matches inside that
/// window. This is the same bound the Browse/Search surfaces honour and is
/// strictly safer than the former unbounded scan (which would OOM on a large
/// archive before producing any output); narrow the date/profile/domain filter
/// to export a regex result set larger than the window.
fn walk_history_for_export(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
    mut visit: impl FnMut(&HistoryEntry) -> Result<()>,
) -> Result<()> {
    let mut export_query = query;
    // Export should always walk the full visible result set, not stay pinned to
    // whichever UI page happened to be open when the user clicked export.
    export_query.page = None;
    export_query.cursor = None;
    export_query.limit = Some(EXPORT_PAGE_SIZE);
    export_query.include_total = Some(false);

    loop {
        let page = list_history(paths, config, key, export_query.clone())?;
        for item in &page.items {
            visit(item)?;
        }
        let Some(next_cursor) = page.next_cursor else {
            return Ok(());
        };
        export_query.cursor = Some(next_cursor);
    }
}

/// Writes one export format row by row: an opening, the rows separated by newlines, a closing.
/// The output is byte-for-byte what joining all rendered rows used to produce.
struct ExportWriter<'a> {
    writer: &'a mut dyn Write,
    format: &'a ExportFormat,
    count: usize,
}

impl<'a> ExportWriter<'a> {
    fn new(writer: &'a mut dyn Write, format: &'a ExportFormat) -> Result<Self> {
        if matches!(format, ExportFormat::Html) {
            writer.write_all(b"<html><body>")?;
        }
        Ok(Self { writer, format, count: 0 })
    }

    fn item(&mut self, item: &HistoryEntry) -> Result<()> {
        if self.count > 0 {
            self.writer.write_all(b"\n")?;
        }
        render_item(self.writer, self.format, item)?;
        self.count += 1;
        Ok(())
    }

    /// Writes the closing and returns how many rows were written.
    fn finish(self) -> Result<usize> {
        if matches!(self.format, ExportFormat::Html) {
            self.writer.write_all(b"</body></html>")?;
        }
        Ok(self.count)
    }
}

/// Renders one row in the requested format, without the separator between rows.
fn render_item(writer: &mut dyn Write, format: &ExportFormat, item: &HistoryEntry) -> Result<()> {
    match format {
        ExportFormat::Html => write_html_item(writer, item)?,
        ExportFormat::Markdown => write!(
            writer,
            "- [{}]({}) — {}",
            item.title.as_deref().unwrap_or(&item.url),
            item.url,
            item.visited_at
        )?,
        ExportFormat::Text => write!(
            writer,
            "{}\n{}\n{}\n",
            item.title.as_deref().unwrap_or(&item.url),
            item.url,
            item.visited_at
        )?,
        ExportFormat::Jsonl => serde_json::to_writer(&mut *writer, item)?,
    }
    Ok(())
}

/// Renders one HTML `<article>`.
///
/// Page titles and URLs originate from imported browser history and are fully
/// attacker-controlled (a visited page chooses its own `<title>`), so every
/// interpolated value is HTML-escaped and the link target is scheme-filtered.
/// Without this a crafted title such as `<img src=x onerror=…>` would execute
/// when the user opens the exported report in a browser.
fn write_html_item(writer: &mut dyn Write, item: &HistoryEntry) -> std::io::Result<()> {
    let title = crate::utils::escape_html(item.title.as_deref().unwrap_or(&item.url));
    let visited_at = crate::utils::escape_html(&item.visited_at);
    let link_text = crate::utils::escape_html(&item.url);
    let href = safe_href(&item.url);
    write!(
        writer,
        "<article><h2>{title}</h2><p><a href=\"{href}\">{link_text}</a></p><p>{visited_at}</p></article>"
    )
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

#[cfg(test)]
mod tests {
    use super::*;

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
            visit_count: None,
        }
    }

    fn render(format: ExportFormat, items: &[HistoryEntry]) -> String {
        let mut bytes = Vec::new();
        let mut export = ExportWriter::new(&mut bytes, &format).expect("open");
        for item in items {
            export.item(item).expect("row");
        }
        assert_eq!(export.finish().expect("close"), items.len());
        String::from_utf8(bytes).expect("utf-8")
    }

    #[test]
    fn html_export_escapes_untrusted_fields_and_neutralizes_dangerous_hrefs() {
        let html = render(
            ExportFormat::Html,
            &[
                entry(
                    "https://example.com/?a=1&b=2",
                    Some("<img src=x onerror=\"alert('xss')\">"),
                    "2024-01-01T00:00:00+00:00",
                ),
                entry("javascript:alert(1)", Some("evil"), "<b>when</b>"),
            ],
        );

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

    /// Streaming must not change the files: same opening and closing, rows separated by one
    /// newline, no trailing separator, untitled rows fall back to the URL.
    #[test]
    fn every_format_writes_the_same_bytes_as_the_joined_export() {
        let rows = [
            entry("https://a.example/", Some("A"), "2024-01-01T00:00:00+00:00"),
            entry("https://b.example/", None, "2024-01-02T00:00:00+00:00"),
        ];
        assert_eq!(
            render(ExportFormat::Markdown, &rows),
            "- [A](https://a.example/) — 2024-01-01T00:00:00+00:00\n\
             - [https://b.example/](https://b.example/) — 2024-01-02T00:00:00+00:00"
        );
        assert_eq!(
            render(ExportFormat::Text, &rows),
            "A\nhttps://a.example/\n2024-01-01T00:00:00+00:00\n\n\
             https://b.example/\nhttps://b.example/\n2024-01-02T00:00:00+00:00\n"
        );
        let jsonl = render(ExportFormat::Jsonl, &rows);
        assert_eq!(jsonl.lines().count(), 2);
        assert!(!jsonl.ends_with('\n'));
        assert_eq!(
            serde_json::from_str::<HistoryEntry>(jsonl.lines().nth(1).expect("row"))
                .expect("json")
                .url,
            "https://b.example/"
        );
        let html = render(ExportFormat::Html, &rows);
        assert!(html.starts_with("<html><body><article><h2>A</h2>"));
        assert!(html.ends_with("</article></body></html>"));
        assert_eq!(html.matches("</article>\n<article>").count(), 1);
        assert_eq!(render(ExportFormat::Html, &[]), "<html><body></body></html>");
        assert_eq!(render(ExportFormat::Jsonl, &[]), "");
    }
}
