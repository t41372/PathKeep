//! Responsibilities: frame one bounded JSON value at a time inside native arrays
//! and session envelopes. Serde validates each frame; no document-sized Value is
//! built. Not responsible for harness semantics or preservation of original bytes.

use std::io::BufRead;

use serde_json::Value;

use crate::{Diagnostic, Error, ParseContext, ParseReport, Result};

use super::super::common::push_diagnostic;

pub(super) struct Document<'a> {
    input: &'a mut dyn BufRead,
    context: &'a ParseContext,
    report: ParseReport,
}

impl<'a> Document<'a> {
    pub(super) fn new(input: &'a mut dyn BufRead, context: &'a ParseContext) -> Result<Self> {
        if context.max_record_bytes == 0 {
            return Err(Error::InvalidInput("record byte limit must be positive".into()));
        }
        let mut document = Self { input, context, report: ParseReport::default() };
        if document.peek()? == Some(0xef) {
            for expected in [0xef, 0xbb, 0xbf] {
                if document.next()? != Some(expected) {
                    document.issue("invalid_utf8", None);
                    break;
                }
            }
        }
        Ok(document)
    }

    pub(super) fn object(
        &mut self,
        field: &mut dyn FnMut(&str, &mut Self) -> Result<()>,
    ) -> Result<()> {
        if !self.expect(b'{')? || self.close_empty(b'}')? {
            return Ok(());
        }
        loop {
            let Some(Value::String(key)) = self.value(None)? else {
                self.issue("malformed_document", None);
                return Ok(());
            };
            if !self.expect(b':')? {
                return Ok(());
            }
            field(&key, self)?;
            if !self.separator(b'}')? {
                return Ok(());
            }
        }
    }

    pub(super) fn array(&mut self, emit: &mut dyn FnMut(Value, u64) -> Result<()>) -> Result<()> {
        if !self.expect(b'[')? || self.close_empty(b']')? {
            return Ok(());
        }
        let mut ordinal = 0;
        loop {
            if let Some(value) = self.value(Some(ordinal))? {
                if value.is_object() {
                    emit(value, ordinal)?;
                    self.report.event_count += 1;
                } else {
                    self.issue("non_object_record", Some(ordinal));
                }
            }
            ordinal += 1;
            if !self.separator(b']')? {
                return Ok(());
            }
        }
    }

    pub(super) fn value(&mut self, ordinal: Option<u64>) -> Result<Option<Value>> {
        self.whitespace()?;
        let Some(first) = self.peek()? else {
            self.issue("incomplete_document", ordinal);
            return Ok(None);
        };
        if matches!(first, b',' | b']' | b'}' | b':') {
            self.issue("malformed_record", ordinal);
            return Ok(None);
        }
        let mut frame = Frame::new(first);
        let mut bytes = Vec::new();
        let mut oversized = false;
        loop {
            let available = self.input.fill_buf()?;
            if available.is_empty() {
                if !frame.scalar || bytes.is_empty() {
                    self.issue("incomplete_document", ordinal);
                    return Ok(None);
                }
                break;
            }
            let mut consumed = 0;
            let mut finished = false;
            for &byte in available {
                if frame.scalar && (is_space(byte) || matches!(byte, b',' | b']' | b'}')) {
                    finished = true;
                    break;
                }
                consumed += 1;
                if frame.accept(byte) {
                    finished = true;
                    break;
                }
            }
            let retained = consumed.min(self.context.max_record_bytes.saturating_sub(bytes.len()));
            bytes.extend_from_slice(&available[..retained]);
            oversized |= retained < consumed;
            self.input.consume(consumed);
            if finished {
                break;
            }
        }
        if oversized {
            self.issue("record_too_large", ordinal);
            return Ok(None);
        }
        match serde_json::from_slice(&bytes) {
            Ok(value) => Ok(Some(value)),
            Err(_) => {
                let code = if std::str::from_utf8(&bytes).is_err() {
                    "invalid_utf8"
                } else {
                    "malformed_record"
                };
                self.issue(code, ordinal);
                Ok(None)
            }
        }
    }

    pub(super) fn issue(&mut self, code: &str, ordinal: Option<u64>) {
        self.report.complete = false;
        push_diagnostic(
            &mut self.report.diagnostics,
            Diagnostic {
                code: code.into(),
                path: Some(self.context.relative_path.clone()),
                ordinal,
                detail: None,
            },
        );
    }

    pub(super) fn finish(mut self) -> Result<ParseReport> {
        self.whitespace()?;
        if self.peek()?.is_some() {
            self.issue("malformed_document", None);
        }
        Ok(self.report)
    }

    fn expect(&mut self, expected: u8) -> Result<bool> {
        self.whitespace()?;
        let actual = self.next()?;
        if actual == Some(expected) {
            return Ok(true);
        }
        self.issue(
            if actual.is_none() { "incomplete_document" } else { "unexpected_json_shape" },
            None,
        );
        Ok(false)
    }

    fn close_empty(&mut self, closing: u8) -> Result<bool> {
        self.whitespace()?;
        if self.peek()? == Some(closing) {
            self.next()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// True means another member is required. A trailing comma is never accepted.
    fn separator(&mut self, closing: u8) -> Result<bool> {
        self.whitespace()?;
        match self.next()? {
            Some(byte) if byte == closing => Ok(false),
            Some(b',') => {
                self.whitespace()?;
                if self.peek()? == Some(closing) {
                    self.next()?;
                    self.issue("malformed_document", None);
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            value => {
                self.issue(
                    if value.is_none() { "incomplete_document" } else { "malformed_document" },
                    None,
                );
                Ok(false)
            }
        }
    }

    fn whitespace(&mut self) -> std::io::Result<()> {
        loop {
            let bytes = self.input.fill_buf()?;
            let count = bytes.iter().take_while(|byte| is_space(**byte)).count();
            if count == 0 {
                return Ok(());
            }
            self.input.consume(count);
        }
    }

    fn peek(&mut self) -> std::io::Result<Option<u8>> {
        Ok(self.input.fill_buf()?.first().copied())
    }

    fn next(&mut self) -> std::io::Result<Option<u8>> {
        let byte = self.peek()?;
        if byte.is_some() {
            self.input.consume(1);
        }
        Ok(byte)
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\n' | b'\r' | b'\t')
}

/// Only finds frame boundaries. JSON grammar, Unicode and nesting limits are
/// validated by serde after framing; oversized frames remain raw-only evidence.
struct Frame {
    scalar: bool,
    string: bool,
    escaped: bool,
    depth: u64,
}

impl Frame {
    fn new(first: u8) -> Self {
        Self {
            scalar: !matches!(first, b'{' | b'[' | b'"'),
            string: false,
            escaped: false,
            depth: 0,
        }
    }

    fn accept(&mut self, byte: u8) -> bool {
        if self.scalar {
            return false;
        }
        if self.string {
            if self.escaped {
                self.escaped = false;
            } else if byte == b'\\' {
                self.escaped = true;
            } else if byte == b'"' {
                self.string = false;
                return self.depth == 0;
            }
        } else {
            match byte {
                b'"' => self.string = true,
                b'{' | b'[' => self.depth = self.depth.saturating_add(1),
                b'}' | b']' => {
                    self.depth = self.depth.saturating_sub(1);
                    return self.depth == 0;
                }
                _ => {}
            }
        }
        false
    }
}
