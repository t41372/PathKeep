//! Responsibilities: interpret retained bytes through bounded parser and write batches.
//! Not responsible for: discovering sources, reading attachments or publishing search.
use super::{BackupOptions, check_cancel};
use crate::{
    Archive, Artifact, Diagnostic, Error, NativeEvent, ParseContext, ParseReport, Result,
    SourceRoot,
    adapters::{adapter, common},
    archive::{ProjectionAttempt, RetainedInput, projection::ReaderStage},
};
use std::{
    cell::{Cell, RefCell},
    io::{self, BufReader, Read, Write},
    rc::Rc,
};

const BATCH_RECORDS: usize = 128;
const BATCH_BYTES: usize = 4 * 1024 * 1024;

pub(super) enum Preparation {
    Ready(Box<Prepared>),
    Writing,
    Unreadable,
}

pub(super) struct Prepared {
    pub report: ParseReport,
    pub attempt: Option<(ProjectionAttempt, ReaderStage)>,
}

pub(super) fn prepare(
    archive: &Archive,
    source: &SourceRoot,
    artifact: &Artifact,
    source_id: &str,
    revision: i64,
    options: &BackupOptions,
    issues: &mut Vec<Diagnostic>,
    force: bool,
) -> Result<Preparation> {
    check_cancel(options)?;
    if adapter(source.harness).needs_first_line_end() {
        match super::readiness::first_record(archive, artifact, revision, options)? {
            super::readiness::Readiness::Whole => {}
            super::readiness::Readiness::Writing => {
                failure(issues, artifact, "first_record_in_progress");
                return Ok(Preparation::Writing);
            }
            super::readiness::Readiness::Unreadable => {
                failure(issues, artifact, "source_decompression_failed");
                return Ok(Preparation::Unreadable);
            }
        }
    }
    // Cached Pi and DeepSeek reports from parser 4 lack the native format number.
    let parser = match source.harness {
        crate::Harness::Pi => 6,
        crate::Harness::Deepseek => 5,
        _ => 4,
    };
    let parser_version = format!(
        "agent-history-parser/{parser}:record={}:decoded={}",
        options.max_record_bytes, options.max_decoded_bytes,
    );
    let mut attempt = archive.prepare_projection(source_id, revision, &parser_version, force)?;
    if attempt.is_none() {
        if let Some(report) = archive.load_versioned_parse_report(revision, &parser_version)? {
            return Ok(Preparation::Ready(Box::new(Prepared { report, attempt: None })));
        }
        // Publication may have survived a crash before its interpretation report
        // was saved. Rebuild under a fresh generation without exposing staged rows.
        attempt = archive.prepare_projection(source_id, revision, &parser_version, true)?;
    }
    let read_error = Rc::new(RefCell::new(None));
    let raw = RetainedInput { reader: archive.raw_reader(revision)?, error: read_error.clone() };
    let decoded = common::decoded_reader(Box::new(raw), &artifact.relative_path);
    let decoded = match decoded {
        Ok(decoded) => decoded,
        Err(_) => {
            if let Some(error) = read_error.borrow_mut().take() {
                return Err(error.into());
            }
            failure(issues, artifact, "source_decompression_failed");
            return Ok(Preparation::Unreadable);
        }
    };
    let limited = Rc::new(Cell::new(false));
    let reader = DecodeBudget {
        reader: decoded,
        remaining: options.max_decoded_bytes,
        cancel: options.cancel.clone(),
        limited: limited.clone(),
    };
    let mut input = BufReader::with_capacity(64 * 1024, reader);
    let context = ParseContext {
        relative_path: artifact.relative_path.clone(),
        project_hint: artifact.project_hint.clone(),
        max_record_bytes: options.max_record_bytes,
    };
    let mut batch = EventBatch::new(archive, attempt.as_ref());
    let mut parsed = adapter(source.harness).parse(&mut input, &context, &mut |event| {
        check_cancel(options)?;
        batch.push(event)
    });
    if parsed.is_ok()
        && let Err(error) = io::copy(&mut input, &mut io::sink())
    {
        parsed = Err(error.into());
    }
    if let Some(error) = batch.destination_error.take() {
        return Err(error);
    }
    if let Some(error) = read_error.borrow_mut().take() {
        return Err(error.into());
    }
    check_cancel(options)?;
    let mut report = match parsed {
        Ok(report) => report,
        Err(error) => {
            if matches!(error, Error::Store(_) | Error::Integrity(_)) {
                return Err(error);
            }
            let code =
                if limited.get() { "decoded_byte_limit" } else { "source_interpretation_failed" };
            failure(issues, artifact, code);
            return Ok(Preparation::Unreadable);
        }
    };
    if let Err(error) = batch.flush() {
        return Err(batch.destination_error.take().unwrap_or(error));
    }
    if report.session.is_none() {
        report.session = Some(crate::SessionMetadata::default());
        report.complete = false;
        failure(&mut report.diagnostics, artifact, "session_metadata_unavailable");
    }
    let staged = if let Some(attempt) = attempt {
        archive.save_projection_gaps(&attempt, &report)?;
        archive.save_versioned_parse_report(revision, &parser_version, &report)?;
        let job = crate::read_model::JobControl {
            cancel: options.cancel.clone(),
            max_decoded_bytes: options.max_decoded_bytes,
            progress: None,
        };
        let reader = if force {
            // Explicit reparse must report a failure to rebuild the requested source.
            archive.stage_reader(&attempt, &context, &job)?;
            ReaderStage::Ready
        } else {
            archive.stage_capture_reader(&attempt, &context, &job)?
        };
        Some((attempt, reader))
    } else {
        None
    };
    Ok(Preparation::Ready(Box::new(Prepared { report, attempt: staged })))
}

fn failure(issues: &mut Vec<Diagnostic>, artifact: &Artifact, code: &str) {
    common::push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: Some(artifact.relative_path.clone()),
            ordinal: None,
            detail: None,
        },
    );
}

struct EventBatch<'a> {
    archive: &'a Archive,
    attempt: Option<&'a ProjectionAttempt>,
    events: Vec<NativeEvent>,
    bytes: usize,
    destination_error: Option<Error>,
}

impl<'a> EventBatch<'a> {
    fn new(archive: &'a Archive, attempt: Option<&'a ProjectionAttempt>) -> Self {
        Self {
            archive,
            attempt,
            events: Vec::with_capacity(BATCH_RECORDS),
            bytes: 0,
            destination_error: None,
        }
    }

    fn push(&mut self, event: NativeEvent) -> Result<()> {
        if self.attempt.is_none() {
            return Ok(());
        }
        let mut size = JsonSize(0);
        serde_json::to_writer(&mut size, &event.payload)?;
        let bytes = size.0.saturating_add(event.text.len());
        if !self.events.is_empty()
            && (self.events.len() >= BATCH_RECORDS
                || self.bytes.saturating_add(bytes) > BATCH_BYTES)
        {
            self.flush()?;
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.events.push(event);
        if self.bytes >= BATCH_BYTES {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.events.is_empty() {
            return Ok(());
        }
        let events = std::mem::replace(&mut self.events, Vec::with_capacity(BATCH_RECORDS));
        self.bytes = 0;
        let Some(attempt) = self.attempt else { return Ok(()) };
        if let Err(error) = self.archive.append_projection_events(attempt, events) {
            self.destination_error = Some(error);
            return Err(Error::Integrity("projection destination write failed".into()));
        }
        Ok(())
    }
}

struct JsonSize(usize);
impl Write for JsonSize {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(input.len());
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct DecodeBudget<R> {
    reader: R,
    remaining: u64,
    cancel: vault_store::CancelToken,
    limited: Rc<Cell<bool>>,
}
impl<R: Read> Read for DecodeBudget<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.cancel.check().is_err() {
            // Interrupted asks io::copy/read_to_end to retry. Cancellation is
            // permanent for this operation; the caller maps it to Cancelled.
            return Err(io::Error::other("capture cancelled"));
        }
        if self.remaining == 0 {
            let mut probe = [0u8; 1];
            if self.reader.read(&mut probe)? == 0 {
                return Ok(0);
            }
            self.limited.set(true);
            return Err(io::Error::other("decoded source exceeds the configured byte budget"));
        }
        let length = output.len().min(self.remaining.min(usize::MAX as u64) as usize);
        let size = self.reader.read(&mut output[..length])?;
        self.remaining -= size as u64;
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Failure mode: std::io stream consumers retry Interrupted forever. A
    // permanently cancelled source must terminate draining, not ask for retry.
    #[test]
    fn permanent_cancellation_is_terminal_for_standard_stream_consumers() {
        let cancel = vault_store::CancelToken::new();
        cancel.cancel();
        let mut reader = DecodeBudget {
            reader: io::Cursor::new(b"unread native bytes"),
            remaining: 1024,
            cancel,
            limited: Rc::new(Cell::new(false)),
        };
        let error = reader.read(&mut [0u8; 1]).unwrap_err();
        assert_ne!(error.kind(), io::ErrorKind::Interrupted);
        assert!(io::copy(&mut reader, &mut io::sink()).is_err());
    }
}
