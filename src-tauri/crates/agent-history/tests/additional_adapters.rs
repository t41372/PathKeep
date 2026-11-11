//! Responsibilities: specify Continue and classic Cline/Roo disk-format failures
//! before implementing their readers. Not responsible for native restore or UI.
//! Failures: stale indexes hide files; whole arrays exhaust memory; oversized or
//! malformed entries hide following messages; metadata order changes identity;
//! tool/compaction fields disappear; legacy reads delete files; credentials or
//! symlinks escape discovery; source failures swallow destination failures.

use std::{
    cell::Cell,
    fs,
    io::{self, BufReader, Cursor, Read},
    rc::Rc,
};

use agent_history::{
    Error,
    adapters::{Adapter, ClineAdapter, ContinueAdapter, RooCodeAdapter},
    model::{ArtifactKind, Harness, NativeEvent, ParseContext, ParseReport, SourceRoot},
};
use serde_json::json;

fn parse(
    adapter: &dyn Adapter,
    bytes: &[u8],
    path: &str,
    limit: usize,
) -> (ParseReport, Vec<NativeEvent>) {
    let context =
        ParseContext { relative_path: path.into(), project_hint: None, max_record_bytes: limit };
    let mut events = Vec::new();
    let report = adapter
        .parse(&mut BufReader::with_capacity(3, Cursor::new(bytes)), &context, &mut |event| {
            events.push(event);
            Ok(())
        })
        .unwrap();
    (report, events)
}

#[test]
fn continue_metadata_can_follow_history_and_original_history_items_survive() {
    let first = json!({"message":{"role":"user","content":[{"type":"text","text":"Review the plan"},{"type":"imageUrl","imageUrl":{"url":"data:image/png;base64,cHJpdmF0ZQ=="}}]},"contextItems":[{"content":"project context","id":{"providerTitle":"file","itemId":"/project/source.rs"}}]});
    let second = json!({"message":{"role":"assistant","content":"A revised plan","toolCalls":[{"id":"call-a","function":{"name":"read_file","arguments":"{\"path\":\"/project/source.rs\"}"}}]},"futureField":{"retain":true}});
    let bytes = format!(
        r#"{{"history":[{first},{second}],"workspaceDirectory":"/project/renamed","sessionId":"native-id","chatModelTitle":"local model","title":"Review","mode":"agent"}}"#
    );
    let (report, events) =
        parse(&ContinueAdapter, bytes.as_bytes(), "sessions/native-id.json", 4096);
    assert!(report.complete, "{:?}", report.diagnostics);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].payload, first);
    assert_eq!(events[1].payload, second);
    assert_eq!(events[0].text, "Review the plan");
    assert_eq!(events[1].role.as_deref(), Some("assistant"));
    let session = report.session.unwrap();
    assert_eq!(session.native_id, "native-id");
    assert_eq!(session.project_path.as_deref(), Some("/project/renamed"));
    assert_eq!(session.model.as_deref(), Some("local model"));
    assert_eq!(session.title.as_deref(), Some("Review"));
    assert!(session.created_at.is_none());
    assert_eq!(session.origin, "unknown");
    assert!(report.references.is_empty()); // Context/tool paths are not attachment authority.
}

#[test]
fn task_arrays_preserve_compaction_reasoning_tool_results_and_inline_images() {
    let rows = json!([
        {"role":"user","content":[{"type":"text","text":"Inspect"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"cHJpdmF0ZQ=="}}],"ts":1794355200000_i64,"id":"q"},
        {"role":"assistant","content":[{"type":"tool_use","id":"call","name":"read_file","input":{"path":"/private/not-an-attachment"}}],"reasoning_content":"Compare alternatives","id":"a","condenseParent":"summary"},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"call","content":"File contents"}],"ts":1794355202000_i64},
        {"role":"assistant","content":"Compacted context","isSummary":true,"condenseId":"summary","truncationParent":"boundary","future":{"keep":true}}
    ]);
    for adapter in [&ClineAdapter as &dyn Adapter, &RooCodeAdapter] {
        let (report, events) = parse(
            adapter,
            rows.to_string().as_bytes(),
            "tasks/1794355200000/api_conversation_history.json",
            4096,
        );
        assert!(report.complete, "{:?}", report.diagnostics);
        assert_eq!(
            events.iter().map(|event| &event.payload).collect::<Vec<_>>(),
            rows.as_array().unwrap().iter().collect::<Vec<_>>()
        );
        assert_eq!(events[0].native_id.as_deref(), Some("q"));
        assert_eq!(events[2].text, "File contents");
        assert!(events[1].text.contains("Compare alternatives"));
        assert!(report.references.is_empty());
        let session = report.session.unwrap();
        assert_eq!(session.native_id, "1794355200000");
        assert!(session.project_path.is_none());
        assert_eq!(session.created_at, Some(1794355200000));
        assert_eq!(session.updated_at, Some(1794355202000));
        assert_eq!(session.origin, "unknown");
    }
}

#[test]
fn oversized_array_element_is_drained_and_does_not_renumber_later_entries() {
    let long = json!({"role":"assistant","content":"x".repeat(100_000)});
    let bytes = format!(
        r#"[{{"role":"user","content":"before"}},{long},{{"role":"assistant","content":"after"}}]"#
    );
    let (report, events) =
        parse(&ClineAdapter, bytes.as_bytes(), "task/api_conversation_history.json", 128);
    assert!(!report.complete);
    assert_eq!(events.iter().map(|event| event.ordinal).collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(events[1].text, "after");
    assert!(
        report.diagnostics.iter().any(|d| d.code == "record_too_large" && d.ordinal == Some(1))
    );
}

#[test]
fn malformed_element_nonobject_and_unfinished_tail_have_explicit_coverage() {
    let bytes = br#"[{"role":"user","content":"before"},{"role":bad},17,{"role":"assistant","content":"after"},{"secret":"unfinished"#;
    let (report, events) =
        parse(&RooCodeAdapter, bytes, "task/api_conversation_history.json", 1024);
    assert!(!report.complete);
    assert_eq!(events.iter().map(|event| event.ordinal).collect::<Vec<_>>(), vec![0, 3]);
    assert!(report.diagnostics.iter().any(|d| d.code == "malformed_record"));
    assert!(report.diagnostics.iter().any(|d| d.code == "non_object_record"));
    assert!(report.diagnostics.iter().any(|d| d.code == "incomplete_document"));
    assert!(!serde_json::to_string(&report.diagnostics).unwrap().contains("secret"));
}

#[test]
fn invalid_envelopes_missing_identity_and_duplicate_history_are_not_complete() {
    for bytes in [
        br#"{"history":[]}"#.as_slice(),
        br#"{"sessionId":"s","history":[],"history":[]}"#.as_slice(),
        br#"{"sessionId":"s","history":[]} trailing"#.as_slice(),
        br#"{"sessionId":"s","history":{},"title":"bad"}"#.as_slice(),
        br#"{"sessionId":"s","history":[{"message":{"role":"user","content":"ok"}}],}"#.as_slice(),
    ] {
        let (report, _) = parse(&ContinueAdapter, bytes, "sessions/s.json", 1024);
        assert!(!report.complete, "accepted invalid envelope");
        assert!(!report.diagnostics.is_empty());
    }
    let (report, _) = parse(&ClineAdapter, b"[]", "api_conversation_history.json", 1024);
    assert!(report.session.is_none());
    assert!(!report.complete);
}

#[test]
fn large_unknown_metadata_does_not_hide_late_identity() {
    let bytes = format!(r#"{{"future":"{}","history":[],"sessionId":"s"}}"#, "z".repeat(10_000));
    let (report, _) = parse(&ContinueAdapter, bytes.as_bytes(), "sessions/s.json", 128);
    assert!(!report.complete);
    assert_eq!(report.session.unwrap().native_id, "s");
    assert!(report.diagnostics.iter().any(|d| d.code == "record_too_large"));
}

#[test]
fn schema_drift_in_known_metadata_is_reported_without_hiding_messages() {
    let bytes = br#"{"sessionId":"s","title":{"future":"structured"},"history":[{"message":{"role":"user","content":"still readable"}}]}"#;
    let (report, events) = parse(&ContinueAdapter, bytes, "sessions/s.json", 1024);
    assert!(!report.complete);
    assert_eq!(events[0].text, "still readable");
    assert!(report.session.unwrap().title.is_none());
    assert!(report.diagnostics.iter().any(|d| d.code == "unexpected_metadata_type"));
}

#[test]
fn escaped_delimiters_unicode_bom_and_crlf_do_not_split_records() {
    let payload = json!({"role":"user","content":"\"quoted\\\" [{,}] 中文\nnext"});
    let bytes = format!("\u{feff}[\r\n{payload}\r\n]\r\n");
    let (report, events) =
        parse(&ClineAdapter, bytes.as_bytes(), "task/api_conversation_history.json", 1024);
    assert!(report.complete, "{:?}", report.diagnostics);
    assert_eq!(events[0].payload, payload);
}

struct IncrementalReader {
    bytes: Cursor<Vec<u8>>,
    emitted: Rc<Cell<usize>>,
}

impl Read for IncrementalReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.bytes.position() > 200 && self.emitted.get() == 0 {
            return Err(io::Error::other("reader consumed the corpus before emitting"));
        }
        let count = output.len().min(32);
        self.bytes.read(&mut output[..count])
    }
}

#[test]
fn emits_before_reading_the_whole_array_and_propagates_sink_failures() {
    let row = r#"{"role":"user","content":"hello"}"#;
    let bytes = format!("[{}]", std::iter::repeat_n(row, 10_000).collect::<Vec<_>>().join(","));
    let emitted = Rc::new(Cell::new(0));
    let mut input = BufReader::with_capacity(
        32,
        IncrementalReader { bytes: Cursor::new(bytes.into_bytes()), emitted: emitted.clone() },
    );
    let context = ParseContext {
        relative_path: "task/api_conversation_history.json".into(),
        max_record_bytes: 128,
        ..Default::default()
    };
    let report = ClineAdapter
        .parse(&mut input, &context, &mut |_| {
            emitted.set(emitted.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(report.complete);
    assert_eq!(emitted.get(), 10_000);
    let error = ContinueAdapter
        .parse(
            &mut Cursor::new(
                br#"{"sessionId":"s","history":[{"message":{"role":"user","content":"hi"}}]}"#,
            ),
            &context,
            &mut |_| Err(Error::Integrity("destination failed".into())),
        )
        .unwrap_err();
    assert!(matches!(error, Error::Integrity(message) if message == "destination failed"));
}

#[test]
fn discovery_enumerates_sessions_without_relying_on_stale_index() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sessions")).unwrap();
    fs::write(root.path().join("sessions/sessions.json"), "[]").unwrap();
    fs::write(
        root.path().join("sessions/unindexed.json"),
        r#"{"sessionId":"unindexed","history":[]}"#,
    )
    .unwrap();
    fs::write(root.path().join("config.json"), r#"{"apiKey":"private"}"#).unwrap();
    fs::write(root.path().join("sessions/auth.json"), r#"{"token":"private"}"#).unwrap();
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    ContinueAdapter
        .discover(
            &SourceRoot { harness: Harness::Continue, root: root.path().to_owned() },
            &mut |artifact| {
                artifacts.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert_eq!(artifacts.len(), 2);
    assert!(artifacts.iter().any(
        |a| a.relative_path == "sessions/unindexed.json" && a.kind == ArtifactKind::Transcript
    ));
    assert!(
        artifacts.iter().any(
            |a| a.relative_path == "sessions/sessions.json" && a.kind == ArtifactKind::Metadata
        )
    );
    assert!(artifacts.iter().all(|a| a.project_hint.is_none()));
}

#[test]
fn task_discovery_keeps_both_native_views_without_double_normalizing_or_reading_settings() {
    let root = tempfile::tempdir().unwrap();
    let task = root.path().join("tasks/native-task");
    fs::create_dir_all(&task).unwrap();
    for name in [
        "api_conversation_history.json",
        "ui_messages.json",
        "context_history.json",
        "task_metadata.json",
        "history_item.json",
        "claude_messages.json",
        "settings.json",
        "secrets.json",
    ] {
        fs::write(task.join(name), "[]").unwrap();
    }
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    RooCodeAdapter
        .discover(
            &SourceRoot { harness: Harness::RooCode, root: root.path().to_owned() },
            &mut |artifact| {
                artifacts.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert_eq!(artifacts.len(), 6);
    assert_eq!(artifacts.iter().filter(|a| a.kind == ArtifactKind::Transcript).count(), 1);
    assert!(
        artifacts.iter().any(|a| a.relative_path.ends_with("claude_messages.json")
            && a.kind == ArtifactKind::Metadata)
    );
    assert_eq!(fs::read_to_string(task.join("claude_messages.json")).unwrap(), "[]");
    fs::remove_file(task.join("api_conversation_history.json")).unwrap();
    let mut fallback = Vec::new();
    RooCodeAdapter
        .discover(
            &SourceRoot { harness: Harness::RooCode, root: root.path().to_owned() },
            &mut |artifact| {
                fallback.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert!(
        fallback.iter().any(|a| a.relative_path.ends_with("claude_messages.json")
            && a.kind == ArtifactKind::Transcript)
    );
}

#[cfg(unix)]
#[test]
fn discovery_never_follows_task_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("tasks")).unwrap();
    fs::write(outside.path().join("api_conversation_history.json"), "[]").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("tasks/linked")).unwrap();
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    ClineAdapter
        .discover(
            &SourceRoot { harness: Harness::Cline, root: root.path().to_owned() },
            &mut |artifact| {
                artifacts.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert!(artifacts.is_empty());
    assert!(issues.iter().any(|d| d.code == "symlink_skipped"));
}
