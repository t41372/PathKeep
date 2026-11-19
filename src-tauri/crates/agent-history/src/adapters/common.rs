//! Responsibilities: bounded transport and content helpers shared by adapters.
//! Not responsible for: native session semantics, capture authorization or raw retention.
//! Failure modes are specified first in `tests/adapter_transport.rs`.

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{Diagnostic, Error, ParseContext, Reference, Result};

pub(crate) const DIAGNOSTIC_SAMPLES: usize = 128;
pub(crate) const REFERENCE_LIMIT: usize = 4096;
pub(crate) const REFERENCE_TARGET_BYTES: usize = 64 * 1024;
pub(crate) const REFERENCE_TOTAL_BYTES: usize = 4 * 1024 * 1024;

/// Counts physical input lines separately from successfully interpreted objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordStats {
    pub physical_records: u64,
    pub emitted_records: u64,
    pub complete: bool,
    pub unterminated_tail: bool,
}

/// Streams JSONL without letting a single corrupt/oversized line hide later records.
/// The byte budget includes BOM and CR bytes but excludes LF. Blank lines count
/// toward physical ordinals. Callback and read errors propagate unchanged.
pub fn records(
    input: &mut dyn BufRead,
    context: &ParseContext,
    emit: &mut dyn FnMut(Value, u64) -> Result<()>,
    issues: &mut Vec<Diagnostic>,
) -> Result<RecordStats> {
    if context.max_record_bytes == 0 {
        return Err(Error::InvalidInput("record byte limit must be positive".into()));
    }
    let mut stats = RecordStats {
        physical_records: 0,
        emitted_records: 0,
        complete: true,
        unterminated_tail: false,
    };
    let mut buffer = Vec::new();
    while let Some(line) = read_line(input, &mut buffer, context.max_record_bytes)? {
        let ordinal = stats.physical_records;
        stats.physical_records += 1;
        stats.unterminated_tail = !line.terminated;
        if line.oversized {
            record_issue(issues, context, ordinal, "record_too_large");
            stats.complete = false;
            continue;
        }
        let mut bytes = buffer.as_slice();
        if ordinal == 0 && bytes.starts_with(b"\xef\xbb\xbf") {
            bytes = &bytes[3..];
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if std::str::from_utf8(bytes).is_err() {
            record_issue(issues, context, ordinal, "invalid_utf8");
            stats.complete = false;
            continue;
        }
        match serde_json::from_slice::<Value>(bytes) {
            Ok(value) if value.is_object() => {
                emit(value, ordinal)?;
                stats.emitted_records += 1;
            }
            Ok(_) => {
                record_issue(issues, context, ordinal, "non_object_record");
                stats.complete = false;
            }
            Err(error) => {
                let code = if !line.terminated && error.is_eof() {
                    "incomplete_tail"
                } else {
                    "malformed_record"
                };
                record_issue(issues, context, ordinal, code);
                stats.complete = false;
            }
        }
    }
    Ok(stats)
}

struct Line {
    oversized: bool,
    terminated: bool,
}

fn read_line(
    input: &mut dyn BufRead,
    buffer: &mut Vec<u8>,
    limit: usize,
) -> std::io::Result<Option<Line>> {
    buffer.clear();
    let mut seen = false;
    let mut oversized = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(seen.then_some(Line { oversized, terminated: false }));
        }
        seen = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        let retain = content_len.min(limit.saturating_sub(buffer.len()));
        buffer.extend_from_slice(&available[..retain]);
        oversized |= retain < content_len;
        let consume = content_len + usize::from(newline.is_some());
        input.consume(consume);
        if newline.is_some() {
            return Ok(Some(Line { oversized, terminated: true }));
        }
    }
}

fn record_issue(issues: &mut Vec<Diagnostic>, context: &ParseContext, ordinal: u64, code: &str) {
    push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: Some(context.relative_path.clone()),
            ordinal: Some(ordinal),
            detail: None,
        },
    );
}

/// Keeps representative failures bounded while recording how many were omitted.
pub fn push_diagnostic(issues: &mut Vec<Diagnostic>, issue: Diagnostic) {
    if issue.omission() {
        let omitted = issue.detail.as_deref().and_then(|n| n.parse::<u64>().ok()).unwrap_or(1);
        add_omitted(issues, &issue.code, omitted);
        return;
    }
    let summary = issues.iter().position(Diagnostic::omission);
    let samples = summary.unwrap_or(issues.len());
    if samples < DIAGNOSTIC_SAMPLES {
        if let Some(index) = summary {
            issues.insert(index, issue);
        } else {
            issues.push(issue);
        }
    } else {
        add_omitted(
            issues,
            if issue.save_warning() { "capture_warnings_omitted" } else { "diagnostics_omitted" },
            1,
        );
    }
}

fn add_omitted(issues: &mut Vec<Diagnostic>, code: &str, omitted: u64) {
    if let Some(summary) = issues.iter_mut().find(|issue| issue.code == code) {
        let previous = summary.detail.as_deref().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
        summary.detail = Some(previous.saturating_add(omitted).to_string());
    } else {
        issues.push(Diagnostic {
            code: code.into(),
            path: None,
            ordinal: None,
            detail: Some(omitted.to_string()),
        });
    }
}

/// Caps deferred attachment/lineage work; false means interpretation is partial.
/// Native pointers remain in retained raw records even when resolution is bounded.
pub fn push_reference(
    references: &mut Vec<Reference>,
    reference: Reference,
    issues: &mut Vec<Diagnostic>,
) -> bool {
    let limit = if references.len() >= REFERENCE_LIMIT {
        Some(("reference_limit", REFERENCE_LIMIT))
    } else if reference.target.len() > REFERENCE_TARGET_BYTES {
        Some(("reference_target_limit", REFERENCE_TARGET_BYTES))
    } else if references.iter().map(|reference| reference.target.len()).sum::<usize>()
        > REFERENCE_TOTAL_BYTES.saturating_sub(reference.target.len())
    {
        Some(("reference_bytes_limit", REFERENCE_TOTAL_BYTES))
    } else {
        None
    };
    if let Some((code, limit)) = limit {
        push_diagnostic(
            issues,
            Diagnostic {
                code: code.into(),
                path: None,
                ordinal: None,
                detail: Some(limit.to_string()),
            },
        );
        false
    } else {
        references.push(reference);
        true
    }
}

/// Streams every Zstandard frame; frame-window memory is capped at 64 MiB.
/// Callers still own cancellation and total decoded-byte policy. Decode failures
/// affect normalization, never the independently captured compressed evidence.
pub fn decoded_reader<'a>(
    input: Box<dyn Read + 'a>,
    relative_path: &str,
) -> Result<Box<dyn BufRead + 'a>> {
    if relative_path.ends_with(".zst") || relative_path.ends_with(".zstd") {
        let mut decoder = zstd::stream::read::Decoder::new(input)?;
        decoder.window_log_max(26)?;
        Ok(Box::new(BufReader::with_capacity(64 * 1024, decoder)))
    } else {
        Ok(Box::new(BufReader::with_capacity(64 * 1024, input)))
    }
}

/// Converts documented native millisecond integers or RFC3339 timestamps only.
/// Raw payloads preserve any greater source precision and unrecognized formats.
pub fn timestamp(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value.as_str().and_then(|text| {
            chrono::DateTime::parse_from_rfc3339(text).ok().map(|time| time.timestamp_millis())
        })
    })
}

/// Extracts known textual content, avoiding image bytes and arbitrary metadata.
/// Pass a message/content subtree; each adapter decides which native fields have
/// conversation meaning. Unknown blocks remain in the event's native payload.
pub fn text(value: &Value) -> String {
    let mut output = String::new();
    append_text(value, &mut output);
    output
}

fn append_text(value: &Value, output: &mut String) {
    match value {
        Value::String(text) => append_piece(output, text),
        Value::Array(values) => {
            for value in values {
                append_text(value, output);
            }
        }
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str);
            let content = match kind {
                Some(
                    "text" | "Text" | "input_text" | "output_text" | "reasoning_text"
                    | "summary_text",
                ) => object.get("text"),
                Some("thinking" | "reasoning") => {
                    object.get("thinking").or_else(|| object.get("text"))
                }
                Some("tool_result" | "tool-result") => object.get("content"),
                None => object.get("content"),
                _ => None,
            };
            if let Some(content) = content {
                append_text(content, output);
            }
        }
        _ => {}
    }
}

fn append_piece(output: &mut String, value: &str) {
    if value.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(value);
}

/// Streams regular files without following root or descendant symlinks.
/// This inventory check is not authorization to open the path later: capture
/// must revalidate containment, file type and identity against filesystem races.
pub fn walk_files(
    root: &Path,
    emit: &mut dyn FnMut(PathBuf) -> Result<()>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    walk_files_cancellable(&crate::capture::Discovery::default(), root, emit, issues)
}

/// Inspect the entry name before opening a directory. Git administrative
/// data is read only by the separately authorized repository observer.
pub(super) fn walk_files_cancellable(
    cancel: &crate::capture::Discovery,
    root: &Path,
    emit: &mut dyn FnMut(PathBuf) -> Result<()>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    let check = || cancel.check();
    check()?;
    let inspect = |path: &Path,
                   ty: cap_primitives::fs::FileType,
                   emit: &mut dyn FnMut(PathBuf) -> Result<()>,
                   issues: &mut Vec<Diagnostic>|
     -> Result<Option<crate::capture::discovery::Entries>> {
        if ty.is_symlink() {
            path_issue(issues, path, "symlink_skipped", None);
        } else if ty.is_dir() {
            match cancel.read_dir(path) {
                Ok(entries) => return Ok(Some(entries)),
                Err(e) => {
                    path_issue(issues, path, "source_unavailable", Some(format!("{:?}", e.kind())))
                }
            }
        } else if ty.is_file() {
            if path.to_str().is_some() {
                emit(path.to_owned())?;
            } else {
                path_issue(issues, path, "non_utf8_path", None);
            }
        } else {
            path_issue(issues, path, "non_regular_file", None);
        }
        Ok(None)
    };
    let mut stack = Vec::new();
    match cancel.metadata(root) {
        Ok(meta) => {
            if let Some(entries) = inspect(root, meta.file_type(), emit, issues)? {
                stack.push(entries);
            }
        }
        Err(e) => path_issue(issues, root, "source_unavailable", Some(format!("{:?}", e.kind()))),
    }
    while let Some(entries) = stack.last_mut() {
        check()?;
        let Some(entry) = entries.next() else {
            stack.pop();
            continue;
        };
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                path_issue(issues, root, "source_unavailable", Some(format!("{:?}", e.kind())));
                continue;
            }
        };
        if entry.file_name().eq_ignore_ascii_case(".git") {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(ty) if ty.is_dir() && stack.len() >= 64 => {
                path_issue(issues, &path, "source_depth_limit", None)
            }
            Ok(ty) => {
                if let Some(children) = inspect(&path, ty, emit, issues)? {
                    stack.push(children);
                }
            }
            Err(e) => {
                path_issue(issues, &path, "source_unavailable", Some(format!("{:?}", e.kind())))
            }
        }
    }
    Ok(())
}

fn path_issue(issues: &mut Vec<Diagnostic>, path: &Path, code: &str, detail: Option<String>) {
    push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: Some(path.to_string_lossy().into_owned()),
            ordinal: None,
            detail,
        },
    );
}
