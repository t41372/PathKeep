//! Responsibilities: stream verified raw bytes and explicit, readable projections to a writer.
//! Not responsible for: destination paths, file publication, source migration or harness resume.
//! A caller writing a file must publish its staged output only after this module returns success.

use std::{fmt, io::Write, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    Error, Result,
    archive::Archive,
    model::NativeEvent,
    query::{EventQuery, SessionView},
};

const EXPORT_PAGE_SIZE: usize = 64;
const FORMAT_VERSION: u32 = 1;

/// Normalized formats are for inspection and interchange, never native harness resumption.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Jsonl,
    Markdown,
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Jsonl => "jsonl",
            Self::Markdown => "markdown",
        })
    }
}

impl FromStr for ExportFormat {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "jsonl" => Ok(Self::Jsonl),
            "markdown" => Ok(Self::Markdown),
            _ => Err(Error::InvalidInput("unsupported export format".into())),
        }
    }
}

/// Records exactly what reached the caller's writer, without claiming native restorability.
/// `complete` means the captured bytes for raw exports and parsed coverage for normalized ones.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExportReport {
    pub format: String,
    /// `native` preserves source bytes; other values identify documented logical snapshots.
    pub source_capture_format: String,
    pub session_id: Option<String>,
    #[serde(with = "crate::query::opaque_id")]
    pub revision_id: i64,
    /// A normalized interpretation can change without changing the retained raw revision.
    #[serde(with = "crate::query::opaque_id::option")]
    pub projection_id: Option<i64>,
    pub event_count: u64,
    pub byte_len: u64,
    pub complete: bool,
    pub native_resume_ready: bool,
}

#[derive(Serialize)]
struct Header<'a> {
    r#type: &'static str,
    format_version: u32,
    normalized: bool,
    native_resume_ready: bool,
    complete: bool,
    source_capture_format: &'a str,
    session: &'a SessionView,
}

impl Archive {
    /// Produces an inspectable projection without hiding unknown native payloads or branches.
    /// Paging checks the projection identity throughout; a concurrent replacement or reparse
    /// fails instead of returning a successful document assembled from two interpretations.
    pub fn write_normalized(
        &self,
        session_id: &str,
        format: ExportFormat,
        output: &mut dyn Write,
    ) -> Result<ExportReport> {
        let session = self.session(session_id)?;
        let revision = self.revision(session.revision_id)?;
        let mut output = CountedWriter::new(output);
        let header = Header {
            r#type: "pathkeep_agent_history_export",
            format_version: FORMAT_VERSION,
            normalized: true,
            native_resume_ready: false,
            complete: session.complete,
            source_capture_format: &revision.capture_format,
            session: &session,
        };
        match format {
            ExportFormat::Jsonl => write_json_line(&mut output, &header)?,
            ExportFormat::Markdown => {
                output.write_all(b"# PathKeep\n\n")?;
                write_json_block(&mut output, &header)?;
            }
        }

        let mut after = None;
        let mut event_count = 0;
        loop {
            self.ensure_export_projection(session_id, session.projection_id)?;
            let page = self.events(&EventQuery {
                session_id: session_id.to_owned(),
                limit: EXPORT_PAGE_SIZE,
                after,
            })?;
            self.ensure_export_projection(session_id, session.projection_id)?;
            if page.items.is_empty() && page.next_cursor.is_some() {
                return Err(Error::Integrity("event page did not advance".into()));
            }
            for event in &page.items {
                match format {
                    ExportFormat::Jsonl => write_json_line(&mut output, event)?,
                    ExportFormat::Markdown => write_markdown_event(&mut output, event)?,
                }
                event_count += 1;
            }
            after = page.next_cursor;
            if after.is_none() {
                break;
            }
        }
        self.ensure_export_projection(session_id, session.projection_id)?;
        output.flush()?;
        Ok(ExportReport {
            format: format.to_string(),
            source_capture_format: revision.capture_format,
            session_id: Some(session.id),
            revision_id: session.revision_id,
            projection_id: Some(session.projection_id),
            event_count,
            byte_len: output.bytes,
            complete: session.complete,
            native_resume_ready: false,
        })
    }

    /// Reproduces retained evidence bytes, including compression and incomplete native tails.
    /// A logical SQLite snapshot contains captured rows, not the original main/WAL file bytes;
    /// the report identifies that format. Chunk and complete digests are verified on success.
    pub fn write_raw(&self, revision: i64, output: &mut dyn Write) -> Result<ExportReport> {
        let info = self.revision(revision)?;
        let mut input = self.raw_reader(revision)?;
        let mut output = CountedWriter::new(output);
        let bytes = std::io::copy(&mut input, &mut output)?;
        if bytes != info.byte_len {
            return Err(Error::Integrity("raw export length does not match its revision".into()));
        }
        output.flush()?;
        Ok(ExportReport {
            format: "raw".into(),
            source_capture_format: info.capture_format,
            session_id: None,
            revision_id: revision,
            projection_id: None,
            event_count: 0,
            byte_len: bytes,
            complete: true,
            native_resume_ready: false,
        })
    }

    fn ensure_export_projection(&self, session_id: &str, projection_id: i64) -> Result<()> {
        if self.current_projection_id(session_id)? != Some(projection_id) {
            return Err(Error::StaleCursor);
        }
        Ok(())
    }
}

fn write_json_line(output: &mut dyn Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *output, value).map_err(serialization_error)?;
    output.write_all(b"\n")?;
    Ok(())
}

fn write_json_block(output: &mut dyn Write, value: &impl Serialize) -> Result<()> {
    output.write_all(b"```json\n")?;
    // JSON escapes newlines within strings. Its pretty-printed lines therefore cannot
    // start with a Markdown closing fence, even when a native string contains one.
    serde_json::to_writer_pretty(&mut *output, value).map_err(serialization_error)?;
    output.write_all(b"\n```\n\n")?;
    Ok(())
}

fn serialization_error(error: serde_json::Error) -> Error {
    match error.io_error_kind() {
        Some(kind) => Error::Io(std::io::Error::new(kind, error)),
        None => Error::Json(error),
    }
}

fn write_markdown_event(output: &mut dyn Write, event: &NativeEvent) -> Result<()> {
    writeln!(output, "## {}\n", event.ordinal)?;
    if !event.text.is_empty() {
        let fence_len = longest_backtick_run(&event.text).saturating_add(1).max(3);
        write_fence(output, fence_len)?;
        output.write_all(b"text\n")?;
        output.write_all(event.text.as_bytes())?;
        // Always separate the closing fence, even when the native text is unterminated.
        output.write_all(b"\n")?;
        write_fence(output, fence_len)?;
        output.write_all(b"\n\n")?;
    }
    // The full event keeps parent edges, unknown fields and non-text evidence available
    // alongside its readable text; Markdown is explicitly a projection, not raw bytes.
    write_json_block(output, event)
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for byte in text.bytes() {
        if byte == b'`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn write_fence(output: &mut dyn Write, mut length: usize) -> std::io::Result<()> {
    const BACKTICKS: [u8; 64] = [b'`'; 64];
    while length > 0 {
        let count = length.min(BACKTICKS.len());
        output.write_all(&BACKTICKS[..count])?;
        length -= count;
    }
    Ok(())
}

struct CountedWriter<'a> {
    inner: &'a mut dyn Write,
    bytes: u64,
}

impl<'a> CountedWriter<'a> {
    fn new(inner: &'a mut dyn Write) -> Self {
        Self { inner, bytes: 0 }
    }
}

impl Write for CountedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.bytes += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
