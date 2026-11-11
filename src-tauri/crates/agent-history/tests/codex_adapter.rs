//! Contract scenarios written before the Codex adapter implementation.
//! Responsibilities: native file discovery and loss-aware record interpretation.
//! Not responsible for archive encryption, zstd transport, or native resume.

use agent_history::adapters::{
    ArtifactKind, Harness, NativeEvent, ParseContext, ParseReport, SourceRoot, adapter,
};
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use tempfile::TempDir;

fn parse(input: &str) -> (ParseReport, Vec<NativeEvent>) {
    let mut events = Vec::new();
    let report = adapter(Harness::Codex)
        .parse(
            &mut Cursor::new(input.as_bytes()),
            &ParseContext {
                relative_path: "sessions/2026/11/10/rollout-example.jsonl".into(),
                project_hint: None,
                max_record_bytes: 1024 * 1024,
            },
            &mut |event| {
                events.push(event);
                Ok(())
            },
        )
        .expect("parse source records");
    (report, events)
}

fn lines(records: &[Value]) -> String {
    let mut output = records.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    output.push('\n');
    output
}

fn metadata() -> Value {
    json!({
        "timestamp": "2026-11-10T12:00:00Z",
        "type": "session_meta",
        "payload": {
            "id": "11111111-1111-4111-8111-111111111111",
            "timestamp": "2026-11-10T12:00:00Z",
            "cwd": "/projects/observatory",
            "originator": "codex_cli_rs",
            "source": "cli",
            "cli_version": "0.167.1"
        }
    })
}

#[test]
fn first_metadata_owns_the_thread_and_all_native_records_survive() {
    let (report, events) = parse(include_str!("fixtures/codex/paginated.jsonl"));
    assert!(report.complete);
    assert_eq!(report.event_count, 9);
    let session = report.session.expect("canonical metadata");
    assert_eq!(session.native_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(session.project_path.as_deref(), Some("/projects/observatory"));
    assert_eq!(session.origin, "desktop");
    assert_eq!(session.harness_version.as_deref(), Some("0.167.1"));
    assert_eq!(session.model.as_deref(), Some("test-model"));
    assert_eq!(session.updated_at, Some(1794312006000));
    assert_eq!(events.len(), 9);
    assert_eq!(events[2].role.as_deref(), Some("user"));
    assert_eq!(events[2].text, "Locate the violet comet");
    assert_eq!(events[3].kind, "event_msg.item_completed.UserMessage");
    assert_eq!(events[6].role.as_deref(), Some("assistant"));
    assert_eq!(events[6].text, "Look toward the northern sky");
    assert_eq!(events[5].role.as_deref(), Some("tool"));
    assert_eq!(events[5].parent_id.as_deref(), Some("call-1"));
    assert_eq!(events[7].payload["payload"]["opaque"]["version"], 99);
    assert_eq!(events[8].payload["payload"]["id"], "22222222-2222-4222-8222-222222222222");
    assert_eq!(
        events.iter().map(|event| event.ordinal).collect::<Vec<_>>(),
        (0..9).collect::<Vec<_>>()
    );
}

#[test]
fn referenced_history_is_preserved_without_claiming_a_complete_linear_thread() {
    let mut meta = metadata();
    meta["payload"]["history_mode"] = json!("paginated");
    meta["payload"]["forked_from_id"] = json!("logical-parent");
    meta["payload"]["history_base"] = json!({
        "thread_id": "immutable-rollout-parent",
        "end_ordinal_exclusive": 48,
        "end_byte_offset": 8192
    });
    meta["payload"]["subagent_history_start_ordinal"] = json!(52);
    let (report, events) = parse(&lines(&[meta.clone()]));
    assert!(!report.complete);
    assert_eq!(report.session.unwrap().parent_session_id.as_deref(), Some("logical-parent"));
    assert!(report.references.iter().any(|reference| {
        reference.kind == "lineage"
            && reference.target == "immutable-rollout-parent"
            && reference.end_byte_offset == Some(8192)
            && reference.end_ordinal_exclusive == Some(48)
    }));
    assert!(report.diagnostics.iter().any(|issue| issue.code == "codex-lineage-unresolved"));
    assert_eq!(events[0].payload, meta);
}

#[test]
fn explicit_media_references_are_reported_without_treating_tool_paths_as_assets() {
    let records = [
        metadata(),
        json!({"type":"event_msg","payload":{"type":"user_message","message":"inspect image","local_images":["/media/photo.png"]}}),
        json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","id":"input-1","content":[{"type":"local_audio","path":"/media/voice.wav"}]}}}),
        json!({"type":"response_item","payload":{"type":"function_call","name":"read_file","arguments":"{\"path\":\"/private/not-an-attachment\"}"}}),
    ];
    let (report, _) = parse(&lines(&records));
    assert!(report.complete);
    let targets =
        report.references.iter().map(|reference| reference.target.as_str()).collect::<Vec<_>>();
    assert!(targets.contains(&"/media/photo.png"));
    assert!(targets.contains(&"/media/voice.wav"));
    assert!(!targets.contains(&"/private/not-an-attachment"));
}

#[test]
fn legacy_generated_images_keep_their_explicit_local_artifact_reference() {
    let image = json!({"type":"event_msg","payload":{
        "type":"image_generation_end", "call_id":"image-call-1", "status":"completed", "result":"",
        "saved_path":"/projects/observatory/generated_images/drawing.png"
    }});
    let (report, events) = parse(&lines(&[metadata(), image.clone()]));
    assert!(report.references.iter().any(|reference| {
        reference.kind == "attachment"
            && reference.target == "/projects/observatory/generated_images/drawing.png"
    }));
    assert_eq!(events[1].payload, image);
}

#[test]
fn missing_identity_and_future_history_mode_remain_explicit() {
    let (report, events) = parse("{\"type\":\"future_record\",\"payload\":{\"preserve\":true}}\n");
    assert!(report.session.is_none());
    assert!(!report.complete);
    assert_eq!(events.len(), 1);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "codex-session-metadata-missing"));

    let mut future = metadata();
    future["payload"]["history_mode"] = json!("future-mode");
    let (report, events) = parse(&lines(&[future.clone()]));
    assert!(!report.complete);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "codex-history-mode-unsupported"));
    assert_eq!(events[0].payload, future);
}

#[test]
fn ide_source_without_desktop_evidence_stays_unknown_and_subagents_keep_parentage() {
    let mut meta = metadata();
    meta["payload"]["originator"] = json!("unidentified-client");
    meta["payload"]["source"] = json!("vscode");
    let (report, _) = parse(&lines(&[meta.clone()]));
    assert_eq!(report.session.unwrap().origin, "unknown");
    meta["payload"]["source"] =
        json!({"subagent":{"thread_spawn":{"parent_thread_id":"parent-thread","depth":1}}});
    let (report, _) = parse(&lines(&[meta]));
    assert_eq!(report.session.unwrap().parent_session_id.as_deref(), Some("parent-thread"));
}

#[test]
fn remote_only_media_is_a_reported_source_limit_and_inline_bytes_remain_native() {
    let remote = json!({"type":"response_item","payload":{"type":"message","role":"user","content":[
        {"type":"input_image","file_id":"file-remote-only"},
        {"type":"input_image","image_url":"data:image/png;base64,aW1hZ2U="}
    ]}});
    let (report, events) = parse(&lines(&[metadata(), remote.clone()]));
    assert!(!report.complete);
    assert!(
        report.diagnostics.iter().any(|issue| issue.code == "codex-remote-attachment-unavailable")
    );
    assert_eq!(events[1].payload, remote);
    assert!(report.references.iter().all(|reference| !reference.target.starts_with("data:")));
}

#[test]
fn remote_media_in_tool_outputs_and_legacy_audio_is_not_silently_complete() {
    for record in [
        json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"tool-1","output":[{"type":"input_audio","file_id":"audio-provider-only"}]}}),
        json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"FunctionCallOutput","id":"tool-2","name":"image_tool","output":[{"type":"input_image","file_id":"image-provider-only"}]}}}),
        json!({"type":"event_msg","payload":{"type":"user_message","message":"listen","audio_references":[{"file_id":"legacy-audio-only"}]}}),
        json!({"type":"event_msg","payload":{"type":"user_message","message":"look","images":["https://example.invalid/image.png"]}}),
    ] {
        let (report, events) = parse(&lines(&[metadata(), record.clone()]));
        assert!(!report.complete);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|issue| issue.code == "codex-remote-attachment-unavailable")
        );
        assert_eq!(events[1].payload, record);
    }
}

#[test]
fn known_legacy_reasoning_and_mcp_results_remain_searchable() {
    for (record, expected_role, expected_text) in [
        (
            json!({"type":"event_msg","payload":{"type":"agent_reasoning","text":"violet analysis"}}),
            "assistant",
            "violet analysis",
        ),
        (
            json!({"type":"event_msg","payload":{"type":"agent_reasoning_raw_content","text":"amber reasoning"}}),
            "assistant",
            "amber reasoning",
        ),
        (
            json!({"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"mcp-1","result":{"Ok":{"content":[{"type":"text","text":"indigo telescope"}]}}}}),
            "tool",
            "indigo telescope",
        ),
        (
            json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"McpToolCall","id":"mcp-2","result":{"content":[{"type":"text","text":"copper observatory"}]}}}}),
            "tool",
            "copper observatory",
        ),
    ] {
        let (_, events) = parse(&lines(&[metadata(), record]));
        assert_eq!(events[1].role.as_deref(), Some(expected_role));
        assert_eq!(events[1].text, expected_text);
    }
}

#[test]
fn reference_work_is_bounded_by_target_bytes_as_well_as_count() {
    let too_long = format!("/media/{}", "x".repeat(64 * 1024));
    let record = json!({"type":"event_msg","payload":{"type":"user_message","message":"image","local_images":[too_long]}});
    let (report, events) = parse(&lines(&[metadata(), record]));
    assert!(!report.complete);
    assert!(report.references.is_empty());
    assert_eq!(events.len(), 2);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "codex-reference-budget"));

    let mut records = vec![metadata()];
    for index in 0..80 {
        let target = format!("/media/{index}/{}", "x".repeat(60 * 1024));
        records.push(json!({"type":"event_msg","payload":{"type":"user_message","message":"image","local_images":[target]}}));
    }
    let (report, events) = parse(&lines(&records));
    assert!(!report.complete);
    assert_eq!(events.len(), 81);
    assert!(
        report.references.iter().map(|reference| reference.target.len()).sum::<usize>()
            <= 4 * 1024 * 1024
    );
}

#[cfg(unix)]
#[test]
fn literal_backslash_in_a_native_filename_is_not_rewritten_as_a_separator() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("sessions")).unwrap();
    fs::write(directory.path().join("sessions/rollout\\name.jsonl"), b"{}\n").unwrap();
    let mut captured = Vec::new();
    adapter(Harness::Codex)
        .discover(
            &SourceRoot { harness: Harness::Codex, root: directory.path().into() },
            &mut |artifact| {
                captured.push(artifact);
                Ok(())
            },
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].relative_path, "sessions/rollout\\name.jsonl");
}

#[test]
fn discovery_captures_compressed_archives_metadata_and_assets_without_credentials() {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    let files = [
        "sessions/2026/11/10/rollout-first.jsonl",
        "sessions/2026/11/10/rollout-second.jsonl.zst",
        "archived_sessions/rollout-archived.jsonl.zst",
        "attachments/thread-1/diagram.png",
        "generated_images/thread-1/render.gif",
        "state_5.sqlite",
        "thread_history_1.sqlite",
        "state_5.sqlite-wal",
        "history.jsonl",
        "session_index.jsonl",
        "auth.json",
        "config.toml",
        "logs_2.sqlite",
        "unrelated/notes.jsonl",
    ];
    for relative in files {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"synthetic source bytes").unwrap();
    }
    let mut captured = Vec::new();
    let mut issues = Vec::new();
    adapter(Harness::Codex)
        .discover(
            &SourceRoot { harness: Harness::Codex, root: root.into() },
            &mut |artifact| {
                captured.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert!(issues.is_empty());
    assert_eq!(captured.len(), 9);
    assert_eq!(
        captured.iter().filter(|artifact| artifact.kind == ArtifactKind::Transcript).count(),
        3
    );
    assert_eq!(captured.iter().filter(|artifact| artifact.kind == ArtifactKind::Sqlite).count(), 2);
    assert_eq!(
        captured.iter().filter(|artifact| artifact.kind == ArtifactKind::Attachment).count(),
        2
    );
    assert!(captured.iter().all(|artifact| artifact.current));
    assert!(captured.iter().all(|artifact| !artifact.relative_path.contains("auth")));
    assert!(captured.iter().all(|artifact| !artifact.relative_path.ends_with("-wal")));
}
