//! Identifies physical records before normalizing their contents. No outline is retained here.
use super::{
    transport::{self, BUFFER, Decoded},
    *,
};
use crate::{
    Diagnostic, Harness, ParseContext,
    adapters::{common, session_json},
};
use serde_json::Value;
use std::{
    cell::Cell,
    io::{BufRead, BufReader},
};

/// Callers may retain the locator and context, or consume the parsed object immediately.
pub type RecordVisitor<'a> =
    dyn FnMut(RecordLocator, &RecordContext, Option<&Value>) -> Result<()> + 'a;

impl RecordCoverage {
    pub(crate) fn from_issue(code: &str) -> Self {
        match code {
            "record_too_large" => Self::Oversized,
            "invalid_utf8" => Self::InvalidUtf8,
            "non_object_record" => Self::NonObject,
            "incomplete_tail" | "incomplete_document" => Self::Incomplete,
            _ => Self::Malformed,
        }
    }
}

/// Streams one immutable native transport. Row-local failures are in each locator;
/// document-wide failures are returned as bounded diagnostics.
pub fn scan_records(
    input: &mut dyn BufRead,
    harness: Harness,
    transport: Transport,
    context: &ParseContext,
    job: &JobControl,
    visit: &mut RecordVisitor<'_>,
) -> Result<ScanReport> {
    scan_with_frames(input, harness, transport, context, job, visit, &mut |_| Ok(()))
}
pub(super) fn scan_with_frames(
    input: &mut dyn BufRead,
    harness: Harness,
    transport: Transport,
    context: &ParseContext,
    job: &JobControl,
    visit: &mut RecordVisitor<'_>,
    frame: &mut dyn FnMut(TransportFrame) -> Result<()>,
) -> Result<ScanReport> {
    if !(1..=64 * 1024 * 1024).contains(&context.max_record_bytes) {
        return Err(Error::InvalidInput("invalid record byte budget".into()));
    }
    job.checkpoint(ReadPhase::Framing, 0)?;
    let restart = Cell::new(None);
    let mut decoded = Decoded::new(Box::new(input), transport, &restart, job, ReadPhase::Framing)?;
    decoded.on_frame = Some(Box::new(frame));
    let mut stream = BufReader::with_capacity(BUFFER, decoded);
    let mut interpretation = RecordContext::new(harness);
    let mut first_object = true;
    let mut physical_records = 0;
    let mut objects = 0;
    let mut emit = |record: RecordLocator, value: Option<Value>| {
        job.cancel.check().map_err(|_| Error::Cancelled)?;
        physical_records += 1;
        objects += u64::from(value.is_some());
        let ordinal = record.ordinal;
        if let Some(value) = &value {
            interpretation.header(value, ordinal, first_object);
            first_object = false;
        }
        visit(record, &interpretation, value.as_ref())?;
        if let Some(value) = value {
            interpretation.advance(&value, ordinal)
        }
        Ok(())
    };
    let issues = match harness {
        Harness::Continue | Harness::Cline | Harness::RooCode => {
            session_json::scan_document(&mut stream, context, harness, &restart, &mut emit)
        }
        _ => scan_lines(&mut stream, &restart, context, &mut emit),
    }
    .map_err(|error| match error {
        Error::Io(io) => transport::read_error(io),
        other => other,
    })?;
    // A malformed envelope may finish framing before the transport ends. Drain
    // the bounded decoder so truncated native frames and evidence failures still surface.
    std::io::copy(&mut stream, &mut std::io::sink()).map_err(transport::read_error)?;
    Ok(ScanReport {
        decoded_bytes: stream.get_ref().work(),
        max_frame_bytes: stream.get_ref().max_frame_bytes,
        physical_records,
        objects,
        coverage: if issues.is_empty() {
            Coverage::Complete
        } else {
            Coverage::Partial { reasons: issues }
        },
    })
}

fn scan_lines(
    input: &mut dyn BufRead,
    restart: &Cell<Option<FrameStart>>,
    context: &ParseContext,
    emit: &mut dyn FnMut(RecordLocator, Option<Value>) -> Result<()>,
) -> Result<Vec<Diagnostic>> {
    let mut issues = Vec::new();
    let mut bytes = Vec::new();
    let mut start = 0;
    let mut ordinal = 0;
    while !input.fill_buf()?.is_empty() {
        let frame = restart.get();
        let Some(line) = common::read_line(input, &mut bytes, context.max_record_bytes)? else {
            break;
        };
        let (value, coverage) = match line.object(&bytes, ordinal) {
            Ok(Some(value)) => (Some(value), RecordCoverage::Object),
            Ok(None) => (None, RecordCoverage::Blank),
            Err(code) => {
                common::record_issue(&mut issues, context, ordinal, code);
                (None, RecordCoverage::from_issue(code))
            }
        };
        emit(
            RecordLocator {
                owner: None,
                ordinal: PhysicalOrdinal(ordinal),
                start: DecodedOffset(start),
                length: line.length,
                frame,
                coverage,
            },
            value,
        )?;
        ordinal += 1;
        start += line.length;
    }
    Ok(issues)
}
