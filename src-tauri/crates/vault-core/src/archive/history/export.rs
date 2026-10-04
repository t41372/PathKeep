//! History export artifact rendering.
//!
//! ## Responsibilities
//! - Walk the full visible history result set for an export request.
//! - Render JSONL, HTML, Markdown, and plain-text artifacts.
//! - Write export artifacts under the configured exports directory, atomically.
//!
//! ## Not responsible for
//! - Choosing which history rows are visible.
//! - Defining history filters or lexical recall semantics.
//!
//! ## Dependencies
//! - Shared history SQL, hydration and recall dispatch.
//! - `durable_io` for the temp-file-then-rename write.
//! - Archive export models and the configured project exports directory.
//!
//! ## Performance notes
//! - Ordinary exports stream one statement; search exports hold one bounded recall page.
//! - One archive connection retains its fixed SQLite cache for the entire walk.
//! - A 256 KiB write buffer never grows with archive size; see ipc-performance.md §9.

mod progress;
#[cfg(test)]
mod regression_tests;

use super::{history_entry_from_row, list_history_on_connection, prepare_advanced_search_filters};
use crate::{
    archive::{
        list_history_bounds, list_history_sql, open_archive_connection,
        read_models::load_cached_archive_totals, search_query::ParsedHistorySearchQuery,
    },
    config::ProjectPaths,
    durable_io::{atomic_durable_write_with, sweep_temps_older_than},
    models::{AppConfig, ExportFormat, ExportRequest, ExportResult, HistoryEntry, HistoryQuery},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use progress::ExportJob;
pub use progress::{ExportCancelled, cancel_export, get_export_progress, record_export_failure};
use rusqlite::{Connection, named_params};
use std::{
    fs,
    io::{BufWriter, Write},
    sync::Arc,
    time::Duration,
};

const EXPORT_PAGE_SIZE: u32 = 1_000;
const EXPORT_BUFFER_BYTES: usize = 256 * 1024;
const PROGRESS_ROW_CHUNK: usize = 128;
const WRITE_CHECK_BYTES: u64 = 64 * 1024;
const ABANDONED_EXPORT_AGE: Duration = Duration::from_secs(60 * 60);

/// Produces the same history artifact with one archive connection and bounded buffers.
pub fn export_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    let job = ExportJob::start(request.export_id.as_deref())?;
    let result = write_export(paths, config, key, request, job.as_ref()).map_err(|error| {
        // Includes early setup failures as well as SQLite's generic SQLITE_INTERRUPT.
        if job.as_ref().is_some_and(|job| job.is_cancelled()) {
            ExportCancelled.into()
        } else {
            error
        }
    });
    if let Some(job) = job {
        job.complete(&result);
    }
    result
}

fn write_export(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
    job: Option<&Arc<ExportJob>>,
) -> Result<ExportResult> {
    check_cancel(job)?;
    fs::create_dir_all(&paths.exports_dir)?;
    sweep_temps_older_than(&paths.exports_dir, ABANDONED_EXPORT_AGE)?;
    let connection = open_archive_connection(paths, config, key)?;
    // Search ranking/grouping may need temporary B-trees: spill rather than grow with history.
    connection.pragma_update(None, "temp_store", "FILE")?;
    if let Some(job) = job {
        job.total(
            // Legacy runs may have NULL or malformed stats. An advisory estimate must not
            // prevent an export; the actual row walk still reports archive/read failures.
            load_cached_archive_totals(&connection)
                .ok()
                .flatten()
                .map(|totals| totals.total_visits as u64),
        );
        let job = Arc::clone(job);
        // Also interrupt expensive filtering/sorting before SQLite yields its first row.
        connection.progress_handler(1_000, Some(move || job.is_cancelled()))?;
    }
    let format = request.format;
    let extension = match format {
        ExportFormat::Html => "html",
        ExportFormat::Markdown => "md",
        ExportFormat::Text => "txt",
        ExportFormat::Jsonl => "jsonl",
    };
    // A unique suffix prevents concurrent exports (even without ids) overwriting each other.
    let mut artifact_id = [0u8; 16];
    getrandom::fill(&mut artifact_id).context("export artifact id")?;
    let artifact_id = hex::encode(artifact_id);
    let file_name = format!("export-{}-{artifact_id}.{extension}", now_rfc3339().replace(':', "-"));
    let target_path = paths.exports_dir.join(file_name);
    let mut count = 0;
    let mut rows_written = 0;
    let result = atomic_durable_write_with(&target_path, |writer| {
        let mut buffer = BufWriter::with_capacity(EXPORT_BUFFER_BYTES, writer);
        let mut counted = CountedWriter { writer: &mut buffer, bytes: 0, checked_at: 0, job };
        let render_result: Result<()> = (|| {
            let mut export = ExportWriter::new(&mut counted, &format)?;
            walk_history_for_export(&connection, request.query, job, |item| {
                check_cancel(job)?;
                export.item(item)?;
                rows_written = export.count as u64;
                if export.count % PROGRESS_ROW_CHUNK == 0 {
                    if let Some(job) = job {
                        job.update(export.count as u64, export.writer.bytes);
                    }
                }
                Ok(())
            })?;
            count = export.finish()?;
            Ok(())
        })();
        if let Some(job) = job {
            job.update(rows_written, counted.bytes);
        }
        render_result?;
        if let Some(job) = job {
            job.finishing()?;
        }
        buffer.flush()?;
        Ok(())
    })
    .with_context(|| format!("writing {}", target_path.display()));
    result?;
    Ok(ExportResult { format, path: target_path.display().to_string(), count })
}

fn check_cancel(job: Option<&Arc<ExportJob>>) -> Result<()> {
    if let Some(job) = job {
        job.check()?;
    }
    Ok(())
}

/// Counts rendered bytes without materializing a formatted row; checks unusually large rows
/// every 64 KiB as well as the row checks in the walker.
struct CountedWriter<'a> {
    writer: &'a mut dyn Write,
    bytes: u64,
    checked_at: u64,
    job: Option<&'a Arc<ExportJob>>,
}

impl Write for CountedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes - self.checked_at >= WRITE_CHECK_BYTES {
            self.checked_at = self.bytes;
            if let Some(job) = self.job {
                job.bytes(self.bytes);
            }
            if self.job.is_some_and(|job| job.is_cancelled()) {
                return Err(std::io::Error::other(ExportCancelled));
            }
        }
        // Split an unusually large field too: one write may otherwise exceed the chunk bound.
        let written = self.writer.write(&bytes[..bytes.len().min(WRITE_CHECK_BYTES as usize)])?;
        self.bytes += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Ordinary browse exports stream one statement in existing time/id order. Search retains the
/// shared cursor reader on this same connection, including its bounded regex/fuzzy semantics.
fn walk_history_for_export(
    connection: &Connection,
    query: HistoryQuery,
    job: Option<&Arc<ExportJob>>,
    mut visit: impl FnMut(&HistoryEntry) -> Result<()>,
) -> Result<()> {
    if query.q.as_ref().is_none_or(|q| q.trim().is_empty()) {
        prepare_advanced_search_filters(connection, &ParsedHistorySearchQuery::default())?;
        let sort = query.sort.as_deref().unwrap_or("newest");
        let (start, end, time, id) =
            list_history_bounds(sort, query.start_time_ms, query.end_time_ms, None);
        // LIMIT -1 with OFFSET 0 streams every row from the start of the keyset walk.
        let mut statement = connection.prepare(list_history_sql(sort))?;
        let mut rows = statement.query(named_params! {
            ":profileId": query.profile_id, ":browserKind": query.browser_kind,
            ":domainPattern": query.domain.filter(|value| !value.trim().is_empty()).map(|value| format!("%{value}%")),
            ":startTimeMs": start, ":endTimeMs": end, ":cursorVisitTime": time,
            ":cursorId": id, ":pageLimit": -1i64, ":pageOffset": 0i64,
        })?;
        while let Some(row) = rows.next()? {
            check_cancel(job)?;
            visit(&history_entry_from_row(row)?)?;
        }
        return Ok(());
    }
    let mut query = query;
    query.page = None;
    query.cursor = None;
    query.limit = Some(EXPORT_PAGE_SIZE);
    query.include_total = Some(false);
    loop {
        check_cancel(job)?;
        let page = list_history_on_connection(connection, query.clone())?;
        for item in &page.items {
            check_cancel(job)?;
            visit(item)?;
        }
        let Some(cursor) = page.next_cursor else {
            return Ok(());
        };
        query.cursor = Some(cursor);
    }
}

/// Writes one export format row by row: an opening, the rows separated by newlines, a closing.
/// The output is byte-for-byte what joining all rendered rows used to produce.
struct ExportWriter<'a, W: Write + ?Sized> {
    writer: &'a mut W,
    format: &'a ExportFormat,
    count: usize,
}

impl<'a, W: Write> ExportWriter<'a, W> {
    fn new(writer: &'a mut W, format: &'a ExportFormat) -> Result<Self> {
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
