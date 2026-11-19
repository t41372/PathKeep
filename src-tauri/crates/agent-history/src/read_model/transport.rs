//! Decodes bounded buffers and records native frame starts at consumed input bytes.
//! Record delimiters and archive manifest addressing belong to the layers above and below.
use super::TransportFrame;
use super::{DecodedOffset, FrameStart, JobControl, NativeOffset, ReadPhase, Transport};
use crate::{Error, Result};
use std::{
    cell::Cell,
    io::{self, BufRead, Read},
};
use zstd::stream::raw::{DParameter, Decoder, Operation};

pub(super) const BUFFER: usize = 64 * 1024;

/// Each successful read comes from exactly one native frame. This lets a framer
/// save the restart at the first byte of a record even if its end is frames away.
pub(super) struct Decoded<'a> {
    input: Box<dyn BufRead + 'a>,
    decoder: Option<Decoder<'static>>,
    native: u64,
    pub(super) max_frame_bytes: u64,
    pub(super) on_frame: Option<Box<dyn FnMut(TransportFrame) -> Result<()> + 'a>>,
    decoded: u64,
    pub(super) prior_work: u64,
    boundary: bool,
    restart: &'a Cell<Option<FrameStart>>,
    job: &'a JobControl,
    phase: ReadPhase,
}
impl<'a> Decoded<'a> {
    pub(super) fn work(&self) -> u64 {
        self.prior_work + self.decoded
    }

    pub(super) fn new(
        input: Box<dyn BufRead + 'a>,
        transport: Transport,
        restart: &'a Cell<Option<FrameStart>>,
        job: &'a JobControl,
        phase: ReadPhase,
    ) -> Result<Self> {
        let decoder = if transport == Transport::Zstd {
            let mut decoder = Decoder::new()?;
            decoder.set_parameter(DParameter::WindowLogMax(26))?;
            Some(decoder)
        } else {
            None
        };
        Ok(Self {
            input,
            decoder,
            max_frame_bytes: 0,
            on_frame: None,
            native: 0,
            decoded: 0,
            prior_work: 0,
            boundary: true,
            restart,
            job,
            phase,
        })
    }
}
impl Read for Decoded<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let size = out.len().min(BUFFER);
        if size == 0 {
            return Ok(0);
        }
        loop {
            self.job.cancel.check().map_err(|_| io::Error::other(Error::Cancelled))?;
            let input = self.input.fill_buf()?;
            let n = if let Some(decoder) = &mut self.decoder {
                if self.boundary {
                    if input.is_empty() {
                        if self.native == 0 {
                            return Err(io::Error::other(Error::NativeTransport(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "missing native zstd frame",
                            ))));
                        }
                        return Ok(0);
                    }
                    self.restart.set(Some(FrameStart {
                        native: NativeOffset(self.native),
                        decoded: DecodedOffset(self.decoded),
                    }));
                }
                let status = decoder
                    .run_on_buffers(input, &mut out[..size])
                    .map_err(|e| io::Error::other(Error::NativeTransport(e)))?;
                if status.bytes_read == 0 && status.bytes_written == 0 && status.remaining != 0 {
                    return Err(io::Error::other(Error::NativeTransport(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "truncated native zstd frame",
                    ))));
                }
                self.native += status.bytes_read as u64;
                self.input.consume(status.bytes_read);
                self.boundary = status.remaining == 0;
                if self.boundary
                    && let Some(report) = &mut self.on_frame
                {
                    let frame = TransportFrame {
                        start: self.restart.get().expect("decoder has started a frame"),
                        native_end: NativeOffset(self.native),
                        decoded_end: DecodedOffset(self.decoded + status.bytes_written as u64),
                    };
                    report(frame).map_err(io::Error::other)?;
                }
                if status.bytes_written == 0 {
                    continue;
                }
                status.bytes_written
            } else {
                let n = input.len().min(size);
                out[..n].copy_from_slice(&input[..n]);
                self.input.consume(n);
                if n == 0 {
                    return Ok(0);
                }
                n
            };
            self.decoded += n as u64;
            if let Some(frame) = self.restart.get() {
                self.max_frame_bytes = self.max_frame_bytes.max(self.decoded - frame.decoded.0);
            }
            self.job.checkpoint(self.phase, self.work()).map_err(io::Error::other)?;
            return Ok(n);
        }
    }
}

pub(super) fn read_error(error: io::Error) -> Error {
    match error.downcast::<Error>() {
        Ok(cause) => cause,
        Err(io) => Error::Io(io),
    }
}
