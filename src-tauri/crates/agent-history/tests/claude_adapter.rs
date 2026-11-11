//! Responsibilities: exercise Claude's real on-disk shapes and streaming parser contract.
//! Not responsible for archive durability, UI projections, or launching Claude.

use std::{fs, io::Cursor, path::Path};

use agent_history::{
    adapters::{Adapter, claude::ClaudeAdapter},
    model::{Artifact, ArtifactKind, Diagnostic, Harness, NativeEvent, ParseContext, SourceRoot},
};
use serde_json::{Value, json};

fn context(path: &str) -> ParseContext {
    ParseContext { relative_path: path.into(), project_hint: None, max_record_bytes: 1024 * 1024 }
}

fn parse(events: &[Value], path: &str) -> (agent_history::model::ParseReport, Vec<NativeEvent>) {
    let bytes = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let mut output = Vec::new();
    let report = ClaudeAdapter
        .parse(&mut Cursor::new(bytes), &context(path), &mut |event| {
            output.push(event);
            Ok(())
        })
        .unwrap();
    (report, output)
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn discover(root: &Path) -> (Vec<Artifact>, Vec<Diagnostic>) {
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    ClaudeAdapter
        .discover(
            &SourceRoot { harness: Harness::ClaudeCode, root: root.into() },
            &mut |artifact| {
                artifacts.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    (artifacts, issues)
}

#[test]
fn parallel_response_blocks_branches_and_unknown_records_all_survive() {
    let rows = vec![
        json!({"type":"user","uuid":"u","parentUuid":null,"sessionId":"s","cwd":"/work/專案","entrypoint":"cli","timestamp":"2026-11-01T10:00:00.123Z","version":"2.1.322","message":{"role":"user","content":"Investigate"}}),
        json!({"type":"assistant","uuid":"a1","parentUuid":"u","sessionId":"s","message":{"id":"msg_same","role":"assistant","model":"model-x","content":[{"type":"tool_use","id":"call-1","name":"Read","input":{"file_path":"/work/a"}}],"usage":{"input_tokens":20,"output_tokens":5}}}),
        json!({"type":"assistant","uuid":"a2","parentUuid":"a1","sessionId":"s","message":{"id":"msg_same","role":"assistant","content":[{"type":"tool_use","id":"call-2","name":"Read","input":{"file_path":"/work/b"}}],"usage":{"input_tokens":20,"output_tokens":5}}}),
        json!({"type":"user","uuid":"r1","parentUuid":"a1","sessionId":"s","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call-1","content":[{"type":"text","text":"first result"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"AA=="}}]}]}}),
        json!({"type":"user","uuid":"r2","parentUuid":"a2","sessionId":"s","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call-2","content":"second result"}]}}),
        json!({"type":"system","subtype":"compact_boundary","uuid":"c","parentUuid":null,"logicalParentUuid":"r1","compactMetadata":{"trigger":"auto"}}),
        json!({"type":"future-event","newField":{"opaque":true}}),
    ];
    let (report, events) = parse(&rows, "projects/custom/s.jsonl");
    assert!(report.complete);
    assert_eq!(report.event_count, 7);
    assert_eq!(events.len(), rows.len());
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.ordinal, index as u64);
        assert_eq!(event.payload, rows[index]);
    }
    assert_eq!(events[2].native_id.as_deref(), Some("a2"));
    assert_eq!(events[4].parent_id.as_deref(), Some("a2"));
    assert!(events[3].text.contains("first result"));
    let session = report.session.unwrap();
    assert_eq!(session.native_id, "s");
    assert_eq!(session.project_path.as_deref(), Some("/work/專案"));
    assert_eq!(session.origin, "cli");
    assert_eq!(session.model.as_deref(), Some("model-x"));
}

#[test]
fn metadata_events_are_not_assumed_to_have_uuid_or_message() {
    let (report, events) = parse(
        &[
            json!({"type":"session-owner","sessionId":"s","accountUuid":"opaque"}),
            json!({"type":"ai-title","sessionId":"s","aiTitle":"Initial"}),
            json!({"type":"custom-title","sessionId":"s","customTitle":"Chosen title"}),
            json!({"type":"ai-title","sessionId":"s","aiTitle":"Later generated"}),
            json!({"type":"api-request-blob","sessionId":"s","hash":"blob","message":{"content":"request detail"}}),
        ],
        "projects/not-a-reversible-path/s.jsonl",
    );
    assert_eq!(events.len(), 5);
    let session = report.session.unwrap();
    assert_eq!(session.title.as_deref(), Some("Chosen title"));
    assert_eq!(session.project_path, None);
    assert_eq!(session.origin, "unknown");
    assert!(events.iter().all(|event| event.native_id.is_none()));
}

#[test]
fn cowork_audit_uses_native_snake_case_id_and_audit_timestamp() {
    let (report, events) = parse(
        &[
            json!({"type":"system","subtype":"init","session_id":"cli-native","cwd":"/sessions/work","claude_code_version":"2.1.260","model":"model-y","_audit_timestamp":"2026-09-01T12:00:00Z"}),
            json!({"type":"assistant","uuid":"a","session_id":"cli-native","message":{"role":"assistant","content":[{"type":"text","text":"A result"}]},"_audit_hmac":"opaque","_audit_timestamp":"2026-09-01T12:01:00Z"}),
        ],
        "local-agent-mode-sessions/account/org/local_desktop/audit.jsonl",
    );
    let session = report.session.unwrap();
    assert_eq!(session.native_id, "cli-native");
    assert_eq!(session.origin, "cowork");
    assert_eq!(session.harness_version.as_deref(), Some("2.1.260"));
    assert!(events[1].timestamp.unwrap() > events[0].timestamp.unwrap());
}

#[test]
fn source_origin_beats_desktop_presence_and_mixed_origins_are_not_invented() {
    let (report, _) = parse(
        &[
            json!({"type":"user","sessionId":"s","entrypoint":"cli","message":{"content":"one"}}),
            json!({"type":"assistant","sessionId":"s","entrypoint":"claude-desktop","message":{"content":"two"}}),
        ],
        "projects/desktop-project/s.jsonl",
    );
    assert_eq!(report.session.unwrap().origin, "unknown");
    assert!(report.diagnostics.iter().any(|issue| issue.code == "claude-mixed-origins"));
}

#[test]
fn subagent_keeps_native_id_and_parent_session_relationship() {
    let (report, events) = parse(
        &[
            json!({"type":"user","uuid":"u","sessionId":"parent","agentId":"child","isSidechain":true,"message":{"content":"Subtask"}}),
        ],
        "projects/p/parent/subagents/workflows/run/agent-child.jsonl",
    );
    assert_eq!(events[0].payload["agentId"], "child");
    let session = report.session.unwrap();
    assert_eq!(session.native_id, "parent");
    assert_eq!(session.parent_session_id.as_deref(), Some("parent"));
    assert!(
        report
            .references
            .iter()
            .any(|reference| reference.kind == "lineage" && reference.target == "parent")
    );
}

#[test]
fn malformed_record_does_not_hide_later_events_or_fabricate_success() {
    let mut events = Vec::new();
    let report = ClaudeAdapter.parse(
        &mut Cursor::new(b"{\"type\":\"user\",\"sessionId\":\"s\"}\n{broken}\n{\"type\":\"future\"}\n{\"partial\":"),
        &context("projects/p/s.jsonl"),
        &mut |event| { events.push(event); Ok(()) },
    ).unwrap();
    assert!(!report.complete);
    assert_eq!(events.iter().map(|event| event.ordinal).collect::<Vec<_>>(), vec![0, 2]);
    assert!(!report.diagnostics.is_empty());
}

#[test]
fn structured_attachment_paths_are_references_and_never_opened_by_the_parser() {
    let (report, events) = parse(
        &[
            json!({"type":"attachment","sessionId":"s","attachment":{"type":"file","filename":"/unavailable/private/document.pdf","content":"already captured excerpt"}}),
            json!({"type":"attachment","sessionId":"s","attachment":{"type":"task_status","outputFilePath":"../../outside/result.txt","status":"completed"}}),
        ],
        "projects/p/s.jsonl",
    );
    assert!(report.complete);
    assert_eq!(events.len(), 2);
    assert!(report.references.iter().any(|reference| reference.kind == "attachment"
        && reference.target == "/unavailable/private/document.pdf"));
    assert!(report.references.iter().any(|reference| reference.kind == "tool-output"
        && reference.target == "../../outside/result.txt"));
}

#[test]
fn config_discovery_keeps_revisions_subagents_and_owned_artifacts() {
    let root = tempfile::tempdir().unwrap();
    let expected = [
        "projects/custom/s.jsonl",
        "projects/custom/s.orphaned-123-old.jsonl",
        "projects/custom/s.jsonl.superseded-123",
        "projects/custom/s/subagents/workflows/run/agent-a.jsonl",
        "projects/custom/s/subagents/workflows/run/agent-a.meta.json",
        "projects/custom/s/tool-results/result.jsonl",
        "projects/custom/sessions-index.json",
        "projects/custom/future-index.v2",
        "file-history/s/snapshot",
        "uploads/s/image.png",
        "image-cache/s/image.png",
        "paste-cache/paste.txt",
        "tasks/shared-list/1.json",
        "history.jsonl",
    ];
    for path in expected {
        write(root.path(), path, b"{}");
    }
    write(root.path(), ".credentials.json", b"secret");
    write(root.path(), "settings.json", b"secret");
    write(root.path(), "projects/custom/memory/private.md", b"not a transcript");
    let (artifacts, issues) = discover(root.path());
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(artifacts.len(), expected.len());
    for path in expected {
        assert!(artifacts.iter().any(|artifact| artifact.relative_path == path), "{path}");
    }
    let output = artifacts
        .iter()
        .find(|artifact| artifact.relative_path.ends_with("tool-results/result.jsonl"))
        .unwrap();
    assert!(matches!(output.kind, ArtifactKind::Attachment));
    assert!(artifacts.iter().all(|artifact| artifact.project_hint.is_none()));
}

#[test]
fn desktop_and_cowork_capture_metadata_and_index_each_conversation_once() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "claude-code-sessions/account/org/local_code.json",
        b"{\"cliSessionId\":\"s\"}",
    );
    let cowork = "local-agent-mode-sessions/account/org/local_cowork";
    write(root.path(), &format!("{cowork}.json"), b"{}");
    write(root.path(), &format!("{cowork}/audit.jsonl"), b"{}");
    write(root.path(), &format!("{cowork}/.audit-key"), b"opaque");
    write(root.path(), &format!("{cowork}/.claude/projects/p/s.jsonl"), b"{}");
    write(root.path(), &format!("{cowork}/uploads/image.png"), b"image");
    write(root.path(), &format!("{cowork}/outputs/result.txt"), b"result");
    write(root.path(), &format!("{cowork}/.claude/.claude.json"), b"account config");
    write(root.path(), "IndexedDB/cache.ldb", b"not complete cloud chat");
    let (artifacts, _) = discover(root.path());
    assert_eq!(artifacts.len(), 7);
    assert_eq!(
        artifacts
            .iter()
            .filter(|artifact| matches!(artifact.kind, ArtifactKind::Transcript))
            .count(),
        1
    );
    assert!(artifacts.iter().any(|artifact| artifact.relative_path.ends_with("audit.jsonl")
        && matches!(artifact.kind, ArtifactKind::Metadata)));
    assert!(!artifacts.iter().any(|artifact| artifact.relative_path.contains("IndexedDB")
        || artifact.relative_path.ends_with(".claude.json")));
}

#[test]
fn orphan_cowork_body_remains_discoverable_and_audit_can_be_fallback() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "local-agent-mode-sessions/account/org/local_orphan/audit.jsonl", b"{}");
    let (artifacts, _) = discover(root.path());
    assert_eq!(artifacts.len(), 1);
    assert!(matches!(artifacts[0].kind, ArtifactKind::Transcript));
}

#[test]
fn cowork_audit_fallback_survives_when_only_subagents_or_old_main_revisions_remain() {
    for remaining in
        ["s/subagents/agent-child.jsonl", "agent-child.jsonl", "s.jsonl.superseded-123"]
    {
        let root = tempfile::tempdir().unwrap();
        let body = "local-agent-mode-sessions/account/org/local_session";
        write(root.path(), &format!("{body}/audit.jsonl"), b"{}");
        write(root.path(), &format!("{body}/.claude/projects/p/{remaining}"), b"{}");
        let (artifacts, _) = discover(root.path());
        assert!(
            artifacts.iter().any(|artifact| artifact.relative_path.ends_with("audit.jsonl")
                && matches!(artifact.kind, ArtifactKind::Transcript)),
            "{remaining}"
        );
    }
}

#[test]
fn project_names_do_not_imply_history_origin_or_subagent_relationships() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "projects/.orphaned-work/s.jsonl", b"{}");
    let (artifacts, _) = discover(root.path());
    assert!(artifacts[0].current);
    for path in ["projects/subagents/s.jsonl", "projects/local-agent-mode-sessions/s.jsonl"] {
        let (report, _) = parse(&[json!({"type":"user","sessionId":"s"})], path);
        let session = report.session.unwrap();
        assert_eq!(session.parent_session_id, None, "{path}");
        assert_eq!(session.origin, "unknown", "{path}");
    }
}

#[test]
fn a_projects_directory_can_be_selected_without_its_original_home() {
    let root = tempfile::tempdir().unwrap();
    let projects = root.path().join("projects");
    write(&projects, "custom/s.jsonl", b"{}");
    let (artifacts, _) = discover(&projects);
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].relative_path, "custom/s.jsonl");
}

#[cfg(unix)]
#[test]
fn discovery_reports_symlinks_without_following_external_content() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "secret.jsonl", b"private");
    fs::create_dir_all(root.path().join("projects/p")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.jsonl"),
        root.path().join("projects/p/s.jsonl"),
    )
    .unwrap();
    let (artifacts, issues) = discover(root.path());
    assert!(artifacts.is_empty());
    assert!(!issues.is_empty());
}

#[cfg(unix)]
#[test]
fn cowork_does_not_follow_a_symlinked_parent_of_its_projects_tree() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "projects/p/s.jsonl", b"private");
    let body = root.path().join("local-agent-mode-sessions/account/org/local_session");
    fs::create_dir_all(&body).unwrap();
    std::os::unix::fs::symlink(outside.path(), body.join(".claude")).unwrap();
    let (artifacts, issues) = discover(root.path());
    assert!(artifacts.is_empty());
    assert!(issues.iter().any(|issue| issue.code == "source-symlink-skipped"));
}
