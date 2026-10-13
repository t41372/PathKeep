//! Failure modes: missed ties, records beyond page size lost, writes block reads,
//! cancellation publishes partial files, memory grows with the result,
//! search progress mislabels the whole archives visit total as matching pages.
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
//! - Exports read short keyset pages through the reader pool with a captured upper visit ID.
//! - Only one page and the fixed write buffer remain live between reads.
//! - A 256 KiB write buffer never grows with archive size; see ipc-performance.md §9.

mod progress;

use crate::{
    Vault,
    config::ProjectPaths,
    durable_io::{atomic_durable_write_with, sweep_temps_older_than},
    models::{
        ExportFormat, ExportRequest, ExportResult, HistoryEntry, HistoryQuery, HistoryQueryResponse,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
pub(super) use progress::ExportJob;
pub use progress::{ExportCancelled, cancel_export, get_export_progress, record_export_failure};
use std::{
    fs,
    io::{BufWriter, Write},
    sync::Arc,
    time::Duration,
};
use vault_store::Db;

const EXPORT_PAGE_SIZE: u32 = 1_000;
const EXPORT_BUFFER_BYTES: usize = 256 * 1024;
const PROGRESS_ROW_CHUNK: usize = 128;
const WRITE_CHECK_BYTES: u64 = 64 * 1024;
const ABANDONED_EXPORT_AGE: Duration = Duration::from_secs(60 * 60);

/// Streams complete history through short read transactions and bounded buffers.
pub fn export_history(vault: &Vault, request: ExportRequest) -> Result<ExportResult> {
    finish_export(request, |request, job| {
        let (total, upper) = export_bounds(vault)?;
        write_export(vault.paths(), request, job, total, |query| {
            vault.read(Db::Archive, move |s| super::timeline::timeline_page(s, &query, upper))
        })
    })
}

fn export_bounds(vault: &Vault) -> Result<(Option<u64>, i64)> {
    vault.read(Db::Archive, |s| {
        let total = s
            .query("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
                Ok(r.get::<i64>(0)?)
            })?
            .pop()
            .map(|n| n as u64);
        let upper = s
            .query("SELECT id FROM visits ORDER BY id DESC LIMIT 1", (), |r| Ok(r.get::<i64>(0)?))?
            .pop()
            .unwrap_or(0);
        Ok((total, upper))
    })
}

pub(super) fn finish_export(
    request: ExportRequest,
    run: impl FnOnce(ExportRequest, Option<&Arc<ExportJob>>) -> Result<ExportResult>,
) -> Result<ExportResult> {
    let job = ExportJob::start(request.export_id.as_deref())?;
    let result = run(request, job.as_ref()).map_err(|error| {
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

pub(super) fn write_export(
    paths: &ProjectPaths,
    request: ExportRequest,
    job: Option<&Arc<ExportJob>>,
    total: Option<u64>,
    mut fetch: impl FnMut(HistoryQuery) -> Result<HistoryQueryResponse>,
) -> Result<ExportResult> {
    check_cancel(job)?;
    fs::create_dir_all(&paths.exports_dir)?;
    sweep_temps_older_than(&paths.exports_dir, ABANDONED_EXPORT_AGE)?;
    let search = request.query.q.as_ref().is_some_and(|q| !q.trim().is_empty());
    if let Some(job) = job {
        job.total(if search { None } else { total });
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
            let mut visit = |item: &HistoryEntry| -> Result<()> {
                if let Some(delay) = crate::test_support::export_row_delay() {
                    std::thread::sleep(delay);
                }
                check_cancel(job)?;
                export.item(item)?;
                rows_written = export.count as u64;
                if export.count % PROGRESS_ROW_CHUNK == 0 {
                    if let Some(job) = job {
                        job.update(export.count as u64, export.writer.bytes);
                    }
                }
                Ok(())
            };
            if search {
                crate::search::walk_history_for_export(
                    paths,
                    request.query,
                    job.map_or_else(vault_store::CancelToken::new, |job| job.token()),
                    &mut visit,
                )?;
            } else {
                walk_pages(&mut fetch, request.query, job, &mut visit)?;
            }
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

fn walk_pages(
    fetch: &mut impl FnMut(HistoryQuery) -> Result<HistoryQueryResponse>,
    mut query: HistoryQuery,
    job: Option<&Arc<ExportJob>>,
    mut visit: impl FnMut(&HistoryEntry) -> Result<()>,
) -> Result<()> {
    query.page = None;
    query.cursor = None;
    query.limit = Some(EXPORT_PAGE_SIZE);
    query.include_total = Some(false);
    loop {
        check_cancel(job)?;
        let page = fetch(query.clone())?;
        for item in &page.items {
            check_cancel(job)?;
            visit(item)?;
        }
        let Some(cursor) = page.next_cursor else { return Ok(()) };
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

const HTML_HEAD: &[u8] = b"<!doctype html><html><head><meta charset=\"utf-8\"></head><body>";

impl<'a, W: Write> ExportWriter<'a, W> {
    fn new(writer: &'a mut W, format: &'a ExportFormat) -> Result<Self> {
        if matches!(format, ExportFormat::Html) {
            // Without a declared charset a browser may read the UTF-8 titles as Latin-1.
            writer.write_all(HTML_HEAD)?;
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
}
