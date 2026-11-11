//! Responsibilities: exercise format boundaries before adapter implementation.
//! Not responsible for: harness-specific interpretation or archive persistence.
//! Failure modes: corrupt records disappear, ordinals shift, long lines allocate
//! without bounds, decoder stops at the first frame, discovery escapes a source,
//! or an archive writer's failure becomes a parser warning. Raw preservation is
//! verified through the CLI separately; no test/build was run while writing this.

use std::{
    fs,
    io::{self, BufReader, Cursor, Read},
};

use agent_history::{
    Error,
    adapters::common::{
        decoded_reader, push_diagnostic, push_reference, records, text, timestamp, walk_files,
    },
    model::{Diagnostic, ParseContext, Reference},
};
use serde_json::{Value, json};

fn context(limit: usize) -> ParseContext {
    ParseContext {
        relative_path: "session.jsonl".into(),
        project_hint: None,
        max_record_bytes: limit,
    }
}

#[test]
fn broken_and_oversized_records_do_not_hide_later_evidence_or_shift_ordinals() {
    let input = b"{\"type\":\"first\"}\n\nprivate-secret broken\n{\"type\":\"a record larger than the budget\"}\n{\"type\":\"last\"}";
    let mut emitted = Vec::new();
    let mut issues = Vec::new();
    let stats = records(
        &mut Cursor::new(input),
        &context(32),
        &mut |value, ordinal| {
            emitted.push((ordinal, value));
            Ok(())
        },
        &mut issues,
    )
    .unwrap();
    assert_eq!(emitted, vec![(0, json!({"type":"first"})), (4, json!({"type":"last"}))]);
    assert_eq!(stats.physical_records, 5);
    assert_eq!(stats.emitted_records, 2);
    assert!(!stats.complete);
    assert!(issues.iter().any(|i| i.code == "malformed_record" && i.ordinal == Some(2)));
    assert!(issues.iter().any(|i| i.code == "record_too_large" && i.ordinal == Some(3)));
    assert!(!serde_json::to_string(&issues).unwrap().contains("private-secret"));
}

#[test]
fn encoding_boundaries_and_partial_tail_are_distinguishable() {
    let input =
        b"\xef\xbb\xbf{\"type\":\"first\"}\r\n{\"type\":\"last\"}\r\n{\"secret\":\"unfinished";
    let mut ordinals = Vec::new();
    let mut issues = Vec::new();
    let stats = records(
        &mut BufReader::with_capacity(2, Cursor::new(input)),
        &context(128),
        &mut |_, ordinal| {
            ordinals.push(ordinal);
            Ok(())
        },
        &mut issues,
    )
    .unwrap();
    assert_eq!(ordinals, vec![0, 1]);
    assert_eq!(stats.physical_records, 3);
    assert!(!stats.complete);
    assert!(issues.iter().any(|i| i.code == "incomplete_tail" && i.ordinal == Some(2)));
    assert!(!serde_json::to_string(&issues).unwrap().contains("unfinished"));
}

#[test]
fn exact_record_limit_and_valid_unterminated_record_are_accepted() {
    let mut issues = Vec::new();
    let mut emitted = Vec::new();
    let stats = records(
        &mut Cursor::new(b"{\"a\":1}\n{\"a\":2}"),
        &context(7),
        &mut |value, ordinal| {
            emitted.push((ordinal, value));
            Ok(())
        },
        &mut issues,
    )
    .unwrap();
    assert_eq!(emitted.len(), 2);
    assert_eq!(stats.physical_records, 2);
    assert!(stats.complete);
    assert!(stats.unterminated_tail);
    assert!(issues.is_empty());
}

#[test]
fn invalid_utf8_non_objects_and_midstream_bom_do_not_become_valid_events() {
    let mut issues = Vec::new();
    let mut emitted = 0;
    let stats = records(
        &mut Cursor::new(b"\xff\n[1,2]\n\xef\xbb\xbf{}\n{}\n"),
        &context(64),
        &mut |_, _| {
            emitted += 1;
            Ok(())
        },
        &mut issues,
    )
    .unwrap();
    assert_eq!(emitted, 1);
    assert_eq!(stats.physical_records, 4);
    assert!(!stats.complete);
    assert!(issues.iter().any(|i| i.code == "invalid_utf8"));
    assert!(issues.iter().any(|i| i.code == "non_object_record"));
}

#[test]
fn diagnostics_are_bounded_without_reporting_complete_coverage() {
    let input = "invalid\n".repeat(10_000);
    let mut issues = Vec::new();
    let stats =
        records(&mut Cursor::new(input), &context(32), &mut |_, _| Ok(()), &mut issues).unwrap();
    assert_eq!(stats.physical_records, 10_000);
    assert!(!stats.complete);
    assert!(issues.len() <= 129);
    assert!(issues.iter().any(|i| i.code == "diagnostics_omitted"));
}

#[test]
fn merging_capped_diagnostics_preserves_the_total_number_of_failures() {
    let mut combined = Vec::new();
    for _ in 0..2 {
        let mut batch = Vec::new();
        records(
            &mut Cursor::new("invalid\n".repeat(10_000)),
            &context(32),
            &mut |_, _| Ok(()),
            &mut batch,
        )
        .unwrap();
        for issue in batch {
            push_diagnostic(&mut combined, issue);
        }
    }
    let retained = combined.iter().filter(|issue| issue.code != "diagnostics_omitted").count();
    let summaries: Vec<_> =
        combined.iter().filter(|issue| issue.code == "diagnostics_omitted").collect();
    assert_eq!(summaries.len(), 1);
    let omitted: usize = summaries[0].detail.as_ref().unwrap().parse().unwrap();
    assert_eq!(retained + omitted, 20_000);
    assert!(combined.len() <= 129);
}

#[test]
fn reference_discovery_reports_its_budget_instead_of_growing_without_bounds() {
    let mut references = Vec::new();
    let mut issues = Vec::new();
    let mut rejected = false;
    for ordinal in 0..10_000 {
        rejected |= !push_reference(
            &mut references,
            Reference {
                kind: "attachment".into(),
                target: format!("attachments/{ordinal}"),
                end_byte_offset: None,
                end_ordinal_exclusive: None,
            },
            &mut issues,
        );
    }
    assert!(rejected);
    assert!(references.len() <= 4096);
    assert!(issues.iter().any(|issue| issue.code == "reference_limit"));
    assert!(issues.len() <= 129);
}

#[test]
fn reference_targets_have_individual_and_aggregate_byte_budgets() {
    let reference = |size| Reference {
        kind: "attachment".into(),
        target: "x".repeat(size),
        end_byte_offset: None,
        end_ordinal_exclusive: None,
    };
    let mut references = Vec::new();
    let mut issues = Vec::new();
    assert!(!push_reference(&mut references, reference(64 * 1024 + 1), &mut issues));
    assert!(references.is_empty());
    for _ in 0..100 {
        push_reference(&mut references, reference(64 * 1024), &mut issues);
    }
    assert!(references.iter().map(|r| r.target.len()).sum::<usize>() <= 4 * 1024 * 1024);
    assert!(issues.iter().any(|i| i.code == "reference_target_limit"));
    assert!(issues.iter().any(|i| i.code == "reference_bytes_limit"));
}

#[test]
fn destination_errors_are_never_downgraded_to_source_warnings() {
    let mut issues = Vec::new();
    let result = records(
        &mut Cursor::new(b"{}\n{}\n"),
        &context(32),
        &mut |_, _| Err(Error::Integrity("destination failed".into())),
        &mut issues,
    );
    assert!(matches!(result, Err(Error::Integrity(message)) if message == "destination failed"));
    assert!(issues.is_empty());
}

struct BrokenReader;
impl Read for BrokenReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "source unreadable"))
    }
}

#[test]
fn read_failures_are_not_reported_as_empty_sources() {
    let mut issues = Vec::new();
    let result =
        records(&mut BufReader::new(BrokenReader), &context(32), &mut |_, _| Ok(()), &mut issues);
    assert!(
        matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied)
    );
}

#[test]
fn concatenated_zstandard_frames_are_read_and_truncation_is_an_error() {
    let mut compressed = zstd::stream::encode_all(Cursor::new(b"{\"a\":1}\n"), 1).unwrap();
    compressed.extend(zstd::stream::encode_all(Cursor::new(b"{\"a\":2}\n"), 1).unwrap());
    for name in ["session.jsonl.zst", "session.v4.jsonl.zstd"] {
        let mut reader = decoded_reader(Box::new(Cursor::new(compressed.clone())), name).unwrap();
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();
        assert_eq!(output, "{\"a\":1}\n{\"a\":2}\n");
    }
    compressed.pop();
    let mut reader =
        decoded_reader(Box::new(Cursor::new(compressed)), "session.jsonl.zst").unwrap();
    assert!(reader.read_to_end(&mut Vec::new()).is_err());
}

#[test]
fn text_extraction_indexes_content_without_serializing_metadata_or_image_bytes() {
    let value = json!([
        {"type":"text","text":"hello"},
        {"type":"image","source":{"type":"base64","data":"private-image"}},
        {"type":"tool_result","content":[{"type":"text","text":"result"}]},
        {"type":"thinking","thinking":"reasoning"},
        {"type":"unknown","text":"metadata-text"}
    ]);
    assert_eq!(text(&value), "hello\nresult\nreasoning");
    assert_eq!(text(&json!({"metadata":{"text":"not-content"}})), "");
    assert_eq!(timestamp(&json!("2026-01-02T03:04:05.678Z")), Some(1767323045678));
    assert_eq!(timestamp(&json!(1767323045678_i64)), Some(1767323045678));
    assert_eq!(timestamp(&Value::Null), None);
}

#[test]
fn native_payload_preserves_integers_and_decimals_beyond_machine_precision() {
    use agent_history::adapters::{Adapter, PiAdapter};

    // Start from native bytes, not json! or f64: a rounded fixture could make
    // precision loss look like successful payload preservation. This regression
    // was written before enabling serde_json's arbitrary_precision feature.
    let bytes = concat!(
        "{\"type\":\"session\",\"version\":3,\"id\":\"precision-session\",",
        "\"timestamp\":\"2026-11-11T00:00:00Z\",\"cwd\":\"/work/example\"}\n",
        "{\"type\":\"future_record\",\"id\":\"precise-fields\",\"parentId\":null,",
        "\"timestamp\":\"2026-11-11T00:00:01Z\",\"future\":{",
        "\"counter\":1844674407370955161618446744073709551616,",
        "\"negative\":-1844674407370955161618446744073709551616,",
        "\"decimal\":0.123456789012345678901234567890,",
        "\"negative_zero\":-0}}\n"
    );
    let mut events = Vec::new();
    PiAdapter
        .parse(&mut Cursor::new(bytes), &context(4096), &mut |event| {
            events.push(event);
            Ok(())
        })
        .unwrap();
    let native = &events
        .iter()
        .find(|event| event.native_id.as_deref() == Some("precise-fields"))
        .expect("unknown event must remain available")
        .payload["future"];
    for (field, expected) in [
        ("counter", "1844674407370955161618446744073709551616"),
        ("negative", "-1844674407370955161618446744073709551616"),
        ("decimal", "0.123456789012345678901234567890"),
        ("negative_zero", "-0"),
    ] {
        assert!(native[field].is_number(), "{field} must remain a JSON number");
        assert_eq!(serde_json::to_string(&native[field]).unwrap(), expected);
    }
}

#[test]
fn missing_source_is_reported_and_regular_files_are_streamed() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("nested")).unwrap();
    fs::write(directory.path().join("nested/session.jsonl"), b"{}\n").unwrap();
    let mut paths = Vec::new();
    let mut issues: Vec<Diagnostic> = Vec::new();
    walk_files(
        directory.path(),
        &mut |path| {
            paths.push(path);
            Ok(())
        },
        &mut issues,
    )
    .unwrap();
    assert_eq!(paths.len(), 1);
    assert!(issues.is_empty());
    walk_files(&directory.path().join("missing"), &mut |_| Ok(()), &mut issues).unwrap();
    assert!(issues.iter().any(|issue| issue.code == "source_unavailable"));
}

#[cfg(unix)]
#[test]
fn symlinks_never_expand_discovery_authority_even_when_dangling() {
    use std::os::unix::fs::symlink;
    let source = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.jsonl"), b"{}\n").unwrap();
    symlink(outside.path(), source.path().join("escape")).unwrap();
    symlink("absent", source.path().join("dangling.jsonl")).unwrap();
    let mut found = Vec::new();
    let mut issues = Vec::new();
    walk_files(
        source.path(),
        &mut |path| {
            found.push(path);
            Ok(())
        },
        &mut issues,
    )
    .unwrap();
    assert!(found.is_empty());
    assert_eq!(issues.iter().filter(|issue| issue.code == "symlink_skipped").count(), 2);
}
