//! Responsibilities: withhold identities until a JSONL producer finishes its first record.
//! Not responsible for: changing retained bytes or constraining low-level publication callers.
use super::BackupOptions;
use crate::archive::RetainedInput;
use crate::{Archive, Artifact, Error, Result, adapters::common};
use std::{
    cell::RefCell,
    io::{BufRead, BufReader},
    rc::Rc,
};

pub(super) enum Readiness {
    Whole,
    Writing,
    Unreadable,
}

pub(super) fn first_record(
    archive: &Archive,
    artifact: &Artifact,
    revision: i64,
    options: &BackupOptions,
) -> Result<Readiness> {
    let raw_error = Rc::new(RefCell::new(None));
    let raw = RetainedInput { reader: archive.raw_reader(revision)?, error: raw_error.clone() };
    let decoded = match common::decoded_reader(Box::new(raw), &artifact.relative_path) {
        Ok(reader) => reader,
        Err(_) => {
            if let Some(error) = raw_error.borrow_mut().take() {
                return Err(error.into());
            }
            return Ok(Readiness::Unreadable);
        }
    };
    let mut reader = BufReader::new(decoded);
    let mut bytes = 0;
    loop {
        super::check_cancel(options)?;
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(error) => {
                if let Some(error) = raw_error.borrow_mut().take() {
                    return Err(error.into());
                }
                if error.kind() == std::io::ErrorKind::UnexpectedEof {
                    return Ok(Readiness::Writing);
                }
                // Raw corruption must still abort rather than become a parser limitation.
                if !archive.revision(revision)?.bytes_retained {
                    return Err(Error::NotFound("revision bytes expired by retention".into()));
                }
                return Ok(Readiness::Unreadable);
            }
        };
        if available.is_empty() {
            return Ok(Readiness::Writing);
        }
        if available.contains(&b'\n') {
            return Ok(Readiness::Whole);
        }
        bytes += available.len() as u64;
        if bytes > options.max_decoded_bytes {
            return Ok(Readiness::Unreadable);
        }
        let length = available.len();
        reader.consume(length);
    }
}
