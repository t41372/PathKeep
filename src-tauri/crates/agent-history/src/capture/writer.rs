//! Responsibilities: turn a validated source stream into bounded encrypted chunks.
//! Not responsible for: native parsing, source selection, search or error presentation.
use super::{BackupOptions, check_cancel, files};
use crate::{Archive, Artifact, ArtifactKind, Diagnostic, Error, Result, SourceRoot};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

use crate::archive::SLICE_BYTES as CHUNK_BYTES;
const SOURCE_ATTEMPTS: usize = 3;

/// Source failure preserves earlier versions; destination failure aborts the run.
pub(super) enum CaptureFailure {
    Source(Error),
    Destination(Error),
}

pub(super) struct Captured {
    pub revision_id: i64,
    pub byte_len: u64,
    pub unchanged: bool,
    pub complete: bool,
    pub diagnostics: Vec<Diagnostic>,
}

pub(super) fn capture(
    archive: &Archive,
    source_id: &str,
    source: &SourceRoot,
    artifact: &Artifact,
    options: &BackupOptions,
) -> std::result::Result<Captured, CaptureFailure> {
    use CaptureFailure::{Destination, Source};
    check_cancel(options).map_err(Source)?;
    archive.register_source(source_id, source, artifact).map_err(Destination)?;
    if artifact.kind == ArtifactKind::Sqlite {
        return capture_database(archive, source_id, source, artifact, options);
    }
    let cancelled = || options.cancel.check().is_err();
    for attempt in 0..SOURCE_ATTEMPTS {
        let inspected = match files::inspect(&artifact.path, &source.root, &cancelled) {
            Ok(value) => value,
            Err(Error::SourceChanged(_)) if attempt + 1 < SOURCE_ATTEMPTS => continue,
            Err(error) => return Err(Source(error)),
        };
        if let Some(previous) = archive.latest_revision(source_id).map_err(Destination)?
            && previous.digest == inspected.digest
            && previous.byte_len == inspected.byte_len
            && previous.capture_format == "native"
        {
            return Ok(Captured {
                revision_id: previous.id,
                byte_len: previous.byte_len,
                unchanged: true,
                complete: true,
                diagnostics: Vec::new(),
            });
        }
        archive.set_coverage(source_id, false).map_err(Destination)?;
        let revision = archive.begin_revision(source_id, source, artifact).map_err(Destination)?;
        let mut writer = ChunkWriter::new(archive, revision);
        let copied =
            files::copy_verified(&artifact.path, &source.root, &inspected, &mut writer, &cancelled);
        if let Some(error) = writer.destination_error.take() {
            return Err(Destination(error));
        }
        if let Err(error) = copied {
            archive.abandon_revision(revision).map_err(Destination)?;
            if matches!(error, Error::SourceChanged(_)) && attempt + 1 < SOURCE_ATTEMPTS {
                continue;
            }
            return Err(Source(error));
        }
        check_cancel(options).map_err(Source)?;
        let (digest, byte_len) = writer.finish().map_err(Destination)?;
        let outcome = archive.finish_revision(revision, &digest, byte_len).map_err(Destination)?;
        return Ok(Captured {
            revision_id: outcome.revision_id,
            byte_len,
            unchanged: outcome.unchanged,
            complete: true,
            diagnostics: Vec::new(),
        });
    }
    Err(Source(Error::SourceChanged("source did not stabilize".into())))
}

fn capture_database(
    archive: &Archive,
    source_id: &str,
    source: &SourceRoot,
    artifact: &Artifact,
    options: &BackupOptions,
) -> std::result::Result<Captured, CaptureFailure> {
    use CaptureFailure::{Destination, Source};
    let path = files::ensure_source_path(&artifact.path, &source.root).map_err(Source)?;
    archive.set_coverage(source_id, false).map_err(Destination)?;
    let revision = archive
        .begin_revision_with_format(
            source_id,
            source,
            artifact,
            crate::sqlite::SQLITE_EVIDENCE_FORMAT,
        )
        .map_err(Destination)?;
    let mut writer = ChunkWriter::new(archive, revision);
    let result =
        crate::sqlite::capture_sqlite(&path, &mut writer, &|| options.cancel.check().is_err());
    if let Some(error) = writer.destination_error.take() {
        return Err(Destination(error));
    }
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            archive.abandon_revision(revision).map_err(Destination)?;
            return Err(Source(error));
        }
    };
    check_cancel(options).map_err(Source)?;
    let (digest, byte_len) = writer.finish().map_err(Destination)?;
    let outcome = archive.finish_revision(revision, &digest, byte_len).map_err(Destination)?;
    Ok(Captured {
        revision_id: outcome.revision_id,
        byte_len,
        unchanged: outcome.unchanged,
        complete: report.complete,
        diagnostics: report.diagnostics,
    })
}

struct ChunkWriter<'a> {
    archive: &'a Archive,
    revision: i64,
    ordinal: u64,
    bytes: u64,
    digest: Sha256,
    pending: Vec<u8>,
    destination_error: Option<Error>,
}

impl<'a> ChunkWriter<'a> {
    fn new(archive: &'a Archive, revision: i64) -> Self {
        Self {
            archive,
            revision,
            ordinal: 0,
            bytes: 0,
            digest: Sha256::new(),
            pending: Vec::with_capacity(CHUNK_BYTES),
            destination_error: None,
        }
    }

    fn persist(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::replace(&mut self.pending, Vec::with_capacity(CHUNK_BYTES));
        if let Err(error) = self.archive.append_chunk(self.revision, self.ordinal, bytes) {
            self.destination_error = Some(error);
            return Err(io::Error::other("archive chunk persistence failed"));
        }
        self.ordinal += 1;
        Ok(())
    }

    fn finish(mut self) -> Result<(String, u64)> {
        let flushed = self.flush();
        if let Some(error) = self.destination_error.take() {
            return Err(error);
        }
        flushed?;
        Ok((hex::encode(self.digest.finalize()), self.bytes))
    }
}

impl Write for ChunkWriter<'_> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.destination_error.is_some() {
            return Err(io::Error::other("archive chunk persistence previously failed"));
        }
        let mut remaining = input;
        while !remaining.is_empty() {
            let take = remaining.len().min(CHUNK_BYTES - self.pending.len());
            let bytes = &remaining[..take];
            self.pending.extend_from_slice(bytes);
            self.digest.update(bytes);
            self.bytes = self.bytes.saturating_add(take as u64);
            remaining = &remaining[take..];
            if self.pending.len() == CHUNK_BYTES {
                self.persist()?;
            }
        }
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.persist()
    }
}
