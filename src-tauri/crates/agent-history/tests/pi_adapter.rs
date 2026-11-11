//! Pi adapter acceptance scenarios written before its implementation.
//!
//! Responsibilities: prove lossless event delivery, branch evidence, legacy
//! identity, bounded failures, and discovery against real on-disk layouts.
//! Not responsible for archive publication or native Pi resume behavior.

use std::io::Cursor;

use agent_history::adapters::{Adapter, PiAdapter};
use agent_history::model::{ArtifactKind, Harness, NativeEvent, ParseContext, SourceRoot};
use serde_json::{Value, json};

fn parse(rows: &[Value]) -> (agent_history::model::ParseReport, Vec<NativeEvent>) {
    let bytes = rows.iter().map(|row| format!("{row}\n")).collect::<String>();
    let context = ParseContext {
        relative_path: "sessions/--work-example--/session.jsonl".into(),
        project_hint: None,
        max_record_bytes: 1024 * 1024,
    };
    let mut events = Vec::new();
    let report = PiAdapter
        .parse(&mut Cursor::new(bytes), &context, &mut |event| {
            events.push(event);
            Ok(())
        })
        .unwrap();
    (report, events)
}

fn header(version: u64) -> Value {
    json!({
        "type": "session", "version": version, "id": "pi-project-review",
        "timestamp": "2026-11-11T00:00:00.000Z", "cwd": "/work/example"
    })
}

#[test]
fn preserves_compacted_branches_edits_and_extension_payloads() {
    // A linear message-only export would lose branch B, the original question,
    // context replacement, and extension state. Every physical object survives.
    let rows = vec![
        header(3),
        json!({"type":"message","id":"root","parentId":null,"timestamp":"2026-11-11T00:00:01Z",
            "message":{"role":"user","content":"Review this design","timestamp":1794355201000_i64}}),
        json!({"type":"message","id":"branch-a","parentId":"root","timestamp":"2026-11-11T00:00:02Z",
            "message":{"role":"assistant","content":[{"type":"thinking","thinking":"Compare alternatives"},{"type":"text","text":"Alternative A"}],
                "api":"anthropic-messages","provider":"anthropic","model":"model-a","stopReason":"stop","timestamp":1794355202000_i64,
                "usage":{"input":12,"output":5,"cacheRead":0,"cacheWrite":0,"totalTokens":17,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}}}),
        json!({"type":"branch_summary","id":"branch-b","parentId":"root","timestamp":"2026-11-11T00:00:03Z","fromId":"branch-a","summary":"Alternative A was explored"}),
        json!({"type":"context_edit","id":"edit","parentId":"branch-b","timestamp":"2026-11-11T00:00:04Z","targetId":"root","replacement":{"content":"Review the revised design"}}),
        json!({"type":"compaction","id":"compact","parentId":"edit","timestamp":"2026-11-11T00:00:05Z","firstKeptEntryId":"branch-b","tokensBefore":200,"summary":"Decision history"}),
        json!({"type":"custom","id":"state","parentId":"compact","timestamp":"2026-11-11T00:00:06Z","customType":"example.extension","data":{"opaque":{"meaning":"retain verbatim"}}}),
        json!({"type":"session_info","id":"title","parentId":"state","timestamp":"2026-11-11T00:00:07Z","name":"Design review"}),
        json!({"type":"future_plugin_entry","id":"unknown","parentId":"title","timestamp":"2026-11-11T00:00:08Z","future":{"field":true}}),
    ];
    let (report, events) = parse(&rows);
    assert!(report.complete);
    assert_eq!(report.event_count, rows.len() as u64);
    assert_eq!(
        events.iter().map(|event| &event.payload).collect::<Vec<_>>(),
        rows.iter().collect::<Vec<_>>()
    );
    assert_eq!(events[3].parent_id.as_deref(), Some("root"));
    assert_eq!(events[4].kind, "context_edit");
    assert_eq!(events[4].text, "Review the revised design");
    assert_eq!(events[1].text, "Review this design");
    let session = report.session.unwrap();
    assert_eq!(session.native_id, "pi-project-review");
    assert_eq!(session.project_path.as_deref(), Some("/work/example"));
    assert_eq!(session.title.as_deref(), Some("Design review"));
    assert_eq!(session.origin, "unknown");
    assert_eq!(session.model.as_deref(), Some("model-a"));
}

#[test]
fn context_edits_and_labels_are_searchable_without_rewriting_history() {
    // Context edits change a later model projection, not the recorded original.
    // Labels are user-authored bookmarks; clearing either contributes no text.
    let rows = vec![
        header(3),
        json!({"type":"message","id":"original","parentId":null,"timestamp":"2026-11-11T00:00:01Z",
            "message":{"role":"user","content":"Original request","timestamp":1794355201000_i64}}),
        json!({"type":"context_edit","id":"edit-text","parentId":"original","timestamp":"2026-11-11T00:00:02Z",
            "targetId":"original","replacement":{"content":"Revised request"}}),
        json!({"type":"context_edit","id":"edit-blocks","parentId":"edit-text","timestamp":"2026-11-11T00:00:03Z",
            "targetId":"original","replacement":{"content":[{"type":"text","text":"Revision with screenshot"},
                {"type":"image","mimeType":"image/png","data":"c2Vuc2l0aXZlLWJpbmFyeQ=="}]}}),
        json!({"type":"context_edit","id":"edit-clear","parentId":"edit-blocks","timestamp":"2026-11-11T00:00:04Z",
            "targetId":"original","replacement":null}),
        json!({"type":"label","id":"bookmark","parentId":"edit-clear","timestamp":"2026-11-11T00:00:05Z",
            "targetId":"original","label":"Decision to revisit"}),
        json!({"type":"label","id":"bookmark-clear","parentId":"bookmark","timestamp":"2026-11-11T00:00:06Z",
            "targetId":"original"}),
    ];
    let (report, events) = parse(&rows);
    assert!(report.complete);
    assert_eq!(events[1].text, "Original request");
    assert_eq!(events[2].text, "Revised request");
    assert_eq!(events[3].text, "Revision with screenshot");
    assert!(events[4].text.is_empty());
    assert_eq!(events[5].text, "Decision to revisit");
    assert!(events[6].text.is_empty());
    assert_eq!(
        events.iter().map(|event| &event.payload).collect::<Vec<_>>(),
        rows.iter().collect::<Vec<_>>()
    );
}

#[test]
fn keeps_inline_images_and_only_declares_known_external_references() {
    let mut parent_header = header(3);
    parent_header["parentSession"] = json!("/archive/parent.jsonl");
    let rows = vec![
        parent_header,
        json!({"type":"message","id":"image","parentId":null,"timestamp":"2026-11-11T00:00:01Z",
            "message":{"role":"user","content":[{"type":"text","text":"Inspect this screenshot"},{"type":"image","mimeType":"image/png","data":"c2Vuc2l0aXZlLWJpbmFyeQ=="}],"timestamp":1794355201000_i64}}),
        json!({"type":"message","id":"tool","parentId":"image","timestamp":"2026-11-11T00:00:02Z",
            "message":{"role":"toolResult","toolCallId":"call-1","toolName":"bash","content":[{"type":"text","text":"Output truncated"}],"details":{"fullOutputPath":"/tmp/pi-full-output.log"},"isError":false,"timestamp":1794355202000_i64}}),
        json!({"type":"message","id":"shell","parentId":"tool","timestamp":"2026-11-11T00:00:03Z",
            "message":{"role":"bashExecution","command":"printf sample","output":"sample","truncated":true,"fullOutputPath":"/tmp/pi-shell-output.log","exitCode":0,"cancelled":false,"timestamp":1794355203000_i64}}),
        json!({"type":"custom","id":"untrusted","parentId":"shell","timestamp":"2026-11-11T00:00:04Z","customType":"untrusted","data":{"path":"/private/credentials","fullOutputPath":"/private/also-not-an-artifact"}}),
    ];
    let (report, events) = parse(&rows);
    assert!(report.complete);
    assert_eq!(events[1].text, "Inspect this screenshot");
    assert_eq!(events[1].payload["message"]["content"][1]["data"], "c2Vuc2l0aXZlLWJpbmFyeQ==");
    let references =
        report.references.iter().map(|r| (r.kind.as_str(), r.target.as_str())).collect::<Vec<_>>();
    assert!(references.contains(&("lineage", "/archive/parent.jsonl")));
    assert!(references.contains(&("tool-output", "/tmp/pi-full-output.log")));
    assert!(references.contains(&("tool-output", "/tmp/pi-shell-output.log")));
    assert_eq!(references.len(), 3);
    assert!(report.session.unwrap().parent_session_id.is_none());
}

#[test]
fn derives_repeatable_legacy_identity_without_modifying_payload() {
    let mut old_header = header(1);
    old_header.as_object_mut().unwrap().remove("version");
    let rows = vec![
        old_header,
        json!({"type":"message","timestamp":"2026-11-11T00:00:01Z","message":{"role":"user","content":"Legacy question","timestamp":1794355201000_i64}}),
        json!({"type":"message","timestamp":"2026-11-11T00:00:02Z","message":{"role":"hookMessage","content":"Legacy hook","timestamp":1794355202000_i64}}),
    ];
    let (report, first) = parse(&rows);
    let (_, second) = parse(&rows);
    assert!(report.complete);
    assert!(first[1].native_id.is_some());
    assert_eq!(first[1].native_id, second[1].native_id);
    assert_eq!(first[2].parent_id, first[1].native_id);
    assert_eq!(first[2].role.as_deref(), Some("custom"));
    assert_eq!(first[2].payload["message"]["role"], "hookMessage");
    assert!(report.diagnostics.iter().any(|d| d.code == "pi-legacy-derived-identity"));
}

#[test]
fn future_format_is_preserved_but_does_not_claim_complete_normalization() {
    let rows = vec![
        header(99),
        json!({"type":"future-event","id":"new","parentId":null,"future":"opaque"}),
    ];
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].payload, rows[1]);
    assert!(report.diagnostics.iter().any(|d| d.code == "pi-unsupported-version"));
}

#[test]
fn latest_session_info_can_clear_an_earlier_title() {
    // Native getSessionName treats an empty or absent latest name as a clear.
    let rows = vec![
        header(3),
        json!({"type":"session_info","id":"named","parentId":null,"timestamp":"2026-11-11T00:00:01Z","name":"Old title"}),
        json!({"type":"session_info","id":"cleared","parentId":"named","timestamp":"2026-11-11T00:00:02Z","name":"   "}),
    ];
    let (report, _) = parse(&rows);
    assert!(report.complete);
    assert!(report.session.unwrap().title.is_none());
}

#[test]
fn tool_names_and_arguments_are_searchable_without_indexing_image_bytes() {
    let rows = vec![
        header(3),
        json!({"type":"message","id":"call","parentId":null,"timestamp":"2026-11-11T00:00:01Z",
            "message":{"role":"assistant","content":[{"type":"text","text":"Inspecting"},{"type":"toolCall","id":"call-1","name":"read","arguments":{"path":"src/important.rs"}}],
                "api":"anthropic-messages","provider":"anthropic","model":"model-a","stopReason":"toolUse","timestamp":1794355201000_i64,
                "usage":{"input":3,"output":8,"cacheRead":0,"cacheWrite":0,"totalTokens":11,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}}}),
        json!({"type":"message","id":"result","parentId":"call","timestamp":"2026-11-11T00:00:02Z",
            "message":{"role":"toolResult","toolCallId":"call-1","toolName":"read","content":[{"type":"text","text":"File contents"}],"isError":false,"timestamp":1794355202000_i64}}),
    ];
    let (report, events) = parse(&rows);
    assert!(report.complete);
    assert!(events[1].text.contains("Inspecting"));
    assert!(events[1].text.contains("read"));
    assert!(events[1].text.contains("src/important.rs"));
    assert_eq!(events[2].text, "File contents");
}

#[test]
fn invalid_header_keeps_raw_objects_without_inventing_a_session() {
    let rows = vec![
        json!({"type":"message","id":"orphan","parentId":null,"message":{"role":"user","content":"No header"}}),
    ];
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert!(report.session.is_none());
    assert_eq!(events.len(), 1);
    assert!(report.diagnostics.iter().any(|d| d.code == "pi-missing-header"));
}

#[test]
fn discovery_reads_standard_layout_without_treating_settings_as_history() {
    let source = tempfile::tempdir().unwrap();
    let project = source.path().join("agent/sessions/--work-example--");
    std::fs::create_dir_all(&project).unwrap();
    let native = format!("{}\n", header(3));
    let transcript = project.join("session.jsonl");
    std::fs::write(&transcript, &native).unwrap();
    std::fs::write(source.path().join("agent/auth.json"), "{}").unwrap();
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    PiAdapter
        .discover(
            &SourceRoot { harness: Harness::Pi, root: source.path().to_path_buf() },
            &mut |a| {
                artifacts.push(a);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert!(issues.is_empty());
    assert_eq!(artifacts.len(), 1);
    assert!(matches!(artifacts[0].kind, ArtifactKind::Transcript));
    assert_eq!(artifacts[0].relative_path, "agent/sessions/--work-example--/session.jsonl");
    assert!(artifacts[0].project_hint.is_none());
    assert_eq!(std::fs::read_to_string(transcript).unwrap(), native);
}

#[test]
fn oversized_native_reference_remains_raw_without_unbounded_retention() {
    let target = format!("/tmp/{}", "x".repeat(70 * 1024));
    let rows = vec![
        header(3),
        json!({"type":"message","id":"shell","parentId":null,"timestamp":"2026-11-11T00:00:01Z",
            "message":{"role":"bashExecution","command":"example","output":"truncated","fullOutputPath":target,"truncated":true,"timestamp":1794355201000_i64}}),
    ];
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert!(report.references.is_empty());
    assert_eq!(events[1].payload, rows[1]);
    assert!(report.diagnostics.iter().any(|d| d.code == "pi-reference-limit"));
}

#[test]
fn repeated_reference_at_the_budget_boundary_does_not_reduce_coverage() {
    let mut rows = vec![header(3)];
    for ordinal in 0..4097 {
        rows.push(json!({"type":"message","id":format!("entry-{ordinal}"),"parentId":null,
            "timestamp":"2026-11-11T00:00:01Z","message":{"role":"bashExecution","command":"example",
                "output":"truncated","fullOutputPath":format!("/tmp/output-{}",ordinal.min(4095)),"truncated":true}}));
    }
    let (report, _) = parse(&rows);
    assert!(report.complete);
    assert_eq!(report.references.len(), 4096);
}

#[cfg(unix)]
#[test]
fn discovery_rejects_a_symlink_in_the_standard_root_chain() {
    let source = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(outside.path().join("sessions")).unwrap();
    std::fs::write(outside.path().join("sessions/secret.jsonl"), format!("{}\n", header(3)))
        .unwrap();
    std::os::unix::fs::symlink(outside.path(), source.path().join("agent")).unwrap();
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    PiAdapter
        .discover(
            &SourceRoot { harness: Harness::Pi, root: source.path().to_path_buf() },
            &mut |a| {
                artifacts.push(a);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert!(artifacts.is_empty());
    assert!(issues.iter().any(|d| d.code == "symlink_skipped"));
}
