//! Reads adjacent physical spans through authenticated archive slices. The caller
//! selects revision members and supplies their locator/context pairs in source order.
use super::{
    transport::{BUFFER, Decoded, read_error},
    *,
};
use crate::{
    Archive, Harness, ParseContext,
    archive::{RawReader, RevisionInfo},
};
use std::{
    cell::Cell,
    io::{self, BufReader, Read},
};

const MAX_PAGE: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 200;

impl Archive {
    /// Scans captured native bytes without granting access to a live source root.
    pub fn scan_revision(
        &self,
        revision: i64,
        context: &ParseContext,
        job: &JobControl,
        visit: &mut RecordVisitor<'_>,
    ) -> Result<ScanReport> {
        let _worker = self.jobs.enter_scan(&job.cancel)?;
        self.scan_revision_admitted(revision, context, job, visit)
    }
    pub(super) fn scan_revision_admitted(
        &self,
        revision: i64,
        context: &ParseContext,
        job: &JobControl,
        visit: &mut RecordVisitor<'_>,
    ) -> Result<ScanReport> {
        self.scan_revision_with_frames(revision, context, job, visit, &mut |_| Ok(()))
    }
    pub(super) fn scan_revision_with_frames(
        &self,
        revision: i64,
        context: &ParseContext,
        job: &JobControl,
        visit: &mut RecordVisitor<'_>,
        frame: &mut dyn FnMut(TransportFrame) -> Result<()>,
    ) -> Result<ScanReport> {
        job.checkpoint(ReadPhase::Framing, 0)?;
        let info = self.revision(revision)?;
        let source = self.source(&info.source_id)?;
        let harness: Harness = source.harness.parse()?;
        let transport = Transport::from_path(&source.relative_path);
        let owner = Arc::new(LocatorOwner::new(&info, transport == Transport::Zstd));
        let mut input = BufReader::with_capacity(
            BUFFER,
            EvidenceReader(self.raw_reader(revision)?.with_scan_control(job)),
        );
        let mut context = context.clone();
        context.relative_path.clone_from(&source.relative_path);
        framing::scan_with_frames(
            &mut input,
            harness,
            transport,
            &context,
            job,
            &mut |mut record, ctx, value| {
                record.owner = Some(owner.clone());
                visit(record, ctx, value)
            },
            frame,
        )
    }

    /// Returns a byte-exact continuation page, including invalid UTF-8 or line delimiters.
    pub fn record_bytes(
        &self,
        revision: &RevisionInfo,
        record: &RecordLocator,
        offset: u64,
        limit: usize,
        job: &JobControl,
    ) -> Result<BytePage> {
        let _worker = self.jobs.enter_window(&job.cancel)?;
        self.record_bytes_admitted(revision, record, offset, limit, job)
    }
    pub(super) fn record_bytes_admitted(
        &self,
        revision: &RevisionInfo,
        record: &RecordLocator,
        offset: u64,
        limit: usize,
        job: &JobControl,
    ) -> Result<BytePage> {
        job.checkpoint(ReadPhase::Reading, 0)?;
        page_limit(limit)?;
        let remaining = record.length.checked_sub(offset).ok_or_else(invalid_span)?;
        let count = remaining.min(limit as u64);
        let start = record.start.0.checked_add(offset).ok_or_else(invalid_span)?;
        let end = start.checked_add(count).ok_or_else(invalid_span)?;
        let info = self.checked_revision(revision)?;
        validate(record, &info)?;
        if count == 0 {
            return Ok(BytePage { bytes: Vec::new(), next: None });
        }
        let mut raw = RawReader::new(self, info.clone());
        let restart = Cell::new(None);
        let (mut input, position) =
            Self::open_range(&mut raw, &info, record.frame, start, end, &restart, job)?;
        skip(&mut input, start - position)?;
        let mut bytes = vec![0; count as usize];
        input.read_exact(&mut bytes).map_err(range_error)?;
        Ok(BytePage { bytes, next: (count < remaining).then_some(offset + count) })
    }

    /// Streams at most 200 ordered rows. Adjacent rows share a slice cache and a
    /// native decoder. A read reflects the supplied revision at its start. If retention
    /// expires it during delivery, authenticated buffered rows can still complete the read.
    /// Otherwise the read reports expiry. Rows delivered to the visitor are provisional:
    /// discard them on error. A successor's bytes never complete the window.
    pub fn read_window(
        &self,
        revision: &RevisionInfo,
        records: &[(RecordLocator, RecordContext)],
        preview_bytes: usize,
        job: &JobControl,
        visit: &mut dyn FnMut(ReaderRow) -> Result<()>,
    ) -> Result<()> {
        let _worker = self.jobs.enter_window(&job.cancel)?;
        self.read_window_admitted(revision, records, preview_bytes, job, visit).map(|_| ())
    }
    pub(super) fn read_window_admitted(
        &self,
        revision: &RevisionInfo,
        records: &[(RecordLocator, RecordContext)],
        preview_bytes: usize,
        job: &JobControl,
        visit: &mut dyn FnMut(ReaderRow) -> Result<()>,
    ) -> Result<u64> {
        job.checkpoint(ReadPhase::Reading, 0)?;
        page_limit(preview_bytes)?;
        if records.len() > MAX_ROWS {
            return Err(Error::InvalidInput("window exceeds 200 rows".into()));
        }
        let info = self.checked_revision(revision)?;
        if records.is_empty() {
            return Ok(0);
        }
        let first = &records[0].0;
        let mut end = first.start.0;
        for (record, _) in records {
            if record.start.0 < end || record.frame.is_some() != first.frame.is_some() {
                return Err(invalid_span());
            }
            end = validate(record, &info)?;
        }
        let mut raw = RawReader::new(self, info.clone());
        let mut remaining = records;
        let mut work = 0;
        while let Some(((first, _), rest)) = remaining.split_first() {
            let mut last = first;
            let mut rows = 1;
            for (next, _) in rest {
                let preview_end = last.start.0 + read_length(last, preview_bytes);
                if next.start.0 != preview_end
                    && next.frame.is_none_or(|frame| frame.decoded.0 >= preview_end)
                {
                    break;
                }
                last = next;
                rows += 1;
            }
            let end = last.start.0 + read_length(last, preview_bytes);
            let restart = Cell::new(None);
            let (mut input, position) =
                Self::open_range(&mut raw, &info, first.frame, first.start.0, end, &restart, job)?;
            input.prior_work = work;
            Self::visit_run(&mut input, position, &remaining[..rows], preview_bytes, &info, visit)?;
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            work = input.work();
            remaining = &remaining[rows..];
        }
        Ok(work)
    }

    fn visit_run(
        input: &mut Decoded<'_>,
        mut position: u64,
        records: &[(RecordLocator, RecordContext)],
        preview_bytes: usize,
        revision: &RevisionInfo,
        visit: &mut dyn FnMut(ReaderRow) -> Result<()>,
    ) -> Result<()> {
        for (record, context) in records {
            skip(input, record.start.0 - position)?;
            let count = read_length(record, preview_bytes);
            let mut bytes = vec![0; count as usize];
            input.read_exact(&mut bytes).map_err(range_error)?;
            position = record.start.0 + count;
            let clipped = record.length > preview_bytes as u64;
            let next = clipped.then_some(preview_bytes as u64);
            let mut interpretation = if record.coverage == RecordCoverage::Object {
                let raw = if record.start.0 == 0 {
                    bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes)
                } else {
                    &bytes
                };
                Some(context.interpret(record.ordinal, serde_json::from_slice(raw)?)?)
            } else {
                None
            };
            let preview = if clipped {
                bytes.truncate(preview_bytes);
                bytes.shrink_to_fit();
                interpretation.take().map(|event| {
                    DisplayRow::from_event(&revision.source_id, record, Some(event), preview_bytes)
                })
            } else {
                None
            };
            visit(ReaderRow {
                preview,
                source_id: SourceId(revision.source_id.clone()),
                revision_digest: revision.digest.clone(),
                record: record.clone(),
                raw: BytePage { bytes, next },
                interpretation,
            })?;
        }
        Ok(())
    }

    fn checked_revision(&self, expected: &RevisionInfo) -> Result<RevisionInfo> {
        let info = self.revision(expected.id)?;
        if info.digest != expected.digest || info.source_id != expected.source_id {
            return Err(Error::StaleCursor);
        }
        if !info.bytes_retained {
            return Err(Error::NotFound("revision bytes expired by retention".into()));
        }
        Ok(info)
    }

    fn open_range<'a>(
        raw: &'a mut RawReader,
        info: &RevisionInfo,
        frame: Option<FrameStart>,
        start: u64,
        end: u64,
        restart: &'a Cell<Option<FrameStart>>,
        job: &'a JobControl,
    ) -> Result<(Decoded<'a>, u64)> {
        let (native, length, position, transport) = match frame {
            Some(frame) => (
                frame.native.0,
                info.byte_len.checked_sub(frame.native.0).ok_or_else(invalid_span)?,
                frame.decoded.0,
                Transport::Zstd,
            ),
            None => (start, end - start, start, Transport::Plain),
        };
        let raw = raw.range(native, length);
        let decoded = Decoded::new(
            Box::new(BufReader::with_capacity(BUFFER, EvidenceReader(raw))),
            transport,
            restart,
            job,
            ReadPhase::Reading,
        )?;
        Ok((decoded, position))
    }
}

/// Mark failures at the evidence boundary. An InvalidData returned by a consumer
/// or the native decoder must never be reported as damage to an archive slice.
struct EvidenceReader<R>(R);
impl<R: Read> Read for EvidenceReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes).map_err(|e| io::Error::other(classify_evidence(read_error(e))))
    }
}

fn classify_evidence(error: Error) -> Error {
    match error {
        Error::Io(io) if crate::archive::expired_read(&io) => {
            Error::NotFound("revision bytes expired by retention".into())
        }
        Error::Io(io) if io.kind() == io::ErrorKind::InvalidData => {
            Error::Integrity(io.to_string())
        }
        Error::Io(io) if io.get_ref().is_some_and(|e| e.is::<vault_store::Error>()) => {
            io.downcast::<vault_store::Error>().expect("checked error type").into()
        }
        other => other,
    }
}

fn validate(record: &RecordLocator, info: &RevisionInfo) -> Result<u64> {
    if !record.owner.as_ref().is_some_and(|o| {
        o.revision == info.id
            && o.source == info.source_id
            && o.digest == info.digest
            && o.compressed == record.frame.is_some()
    }) {
        return Err(Error::StaleCursor);
    }
    // Framing admits at most 64 MiB of content plus the line terminator.
    if record.coverage == RecordCoverage::Object && record.length > (64 << 20) + 2 {
        return Err(invalid_span());
    }
    let end = record.start.0.checked_add(record.length).ok_or_else(invalid_span)?;
    match record.frame {
        Some(frame) if frame.decoded.0 > record.start.0 || frame.native.0 >= info.byte_len => {
            Err(invalid_span())
        }
        None if end > info.byte_len => Err(invalid_span()),
        _ => Ok(end),
    }
}

fn read_length(record: &RecordLocator, preview_bytes: usize) -> u64 {
    match record.coverage {
        RecordCoverage::Object => record.length,
        _ => record.length.min(preview_bytes as u64),
    }
}

fn page_limit(limit: usize) -> Result<()> {
    if (1..=MAX_PAGE).contains(&limit) {
        Ok(())
    } else {
        Err(Error::InvalidInput("invalid preview byte budget".into()))
    }
}
fn invalid_span() -> Error {
    Error::InvalidInput("record span outside revision".into())
}
fn range_error(io: io::Error) -> Error {
    if io.kind() == io::ErrorKind::UnexpectedEof { invalid_span() } else { read_error(io) }
}

fn skip(input: &mut dyn Read, count: u64) -> Result<()> {
    if count != 0
        && io::copy(&mut input.take(count), &mut io::sink()).map_err(range_error)? != count
    {
        return Err(invalid_span());
    }
    Ok(())
}
