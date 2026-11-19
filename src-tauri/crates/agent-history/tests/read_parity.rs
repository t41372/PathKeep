//! Compare retained-record reads with the public projection and normalized export.
//! Fixture bytes are owned by the existing CLI cases. No CLI process runs here.
#[allow(dead_code)]
#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/native.rs"]
mod native;
use agent_history::{Archive, Harness, SourceRoot, export::ExportFormat, query::EventQuery};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn seven_harnesses_agree_with_published_events_and_both_normalized_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = fixtures::Fixture::four_harnesses(tmp.path());
    // Keep shell input and output independently visible while normalized exports keep their legacy text.
    use std::io::Write;
    let shell = json!({"type":"message","id":"bash-parity","parentId":"00000002","message":{"role":"bashExecution","command":"pwd","output":"/work"}});
    writeln!(fs::OpenOptions::new().append(true).open(&fixture.files[2]).unwrap(), "{shell}")
        .unwrap();
    let mut roots: Vec<_> = fixture
        .roots
        .iter()
        .map(|(tool, path)| SourceRoot { harness: tool.parse().unwrap(), root: path.clone() })
        .collect();
    for harness in [Harness::Continue, Harness::Cline, Harness::RooCode] {
        let root = tmp.path().join(harness.as_str());
        let messages = json!([
            {"role":"user","content":[{"type":"text","text":"Inspect"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"cHJpdmF0ZQ=="}}],"ts":1794355200000_i64,"id":"q"},
            {"role":"assistant","content":[{"type":"tool_use","id":"call","name":"read_file","input":{"path":"/private/not-an-attachment"}}],"reasoning_content":"Compare alternatives","id":"a","condenseParent":"summary"},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"call","content":"File contents"}],"ts":1794355202000_i64},
            {"role":"assistant","content":"Compacted context","isSummary":true,"condenseId":"summary","truncationParent":"boundary","future":{"keep":true}}
        ]);
        if harness == Harness::Continue {
            native::continue_session(
                &root,
                "fixture",
                &[
                    native::continue_entry("user", "test 中文"),
                    native::continue_entry("assistant", "done"),
                ],
            );
        } else {
            write(
                &root.join("tasks/123/api_conversation_history.json"),
                messages.to_string().as_bytes(),
            );
        }
        roots.push(SourceRoot { harness, root });
    }
    let archive = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
    archive.backup(&roots, &Default::default()).unwrap();
    for root in &roots {
        fs::remove_dir_all(&root.root).unwrap();
    }
    let sessions = archive.list_sessions(&Default::default()).unwrap().items;
    assert_eq!(sessions.len(), 7);
    for session in sessions {
        let mut outline = Vec::new();
        archive
            .scan_revision(
                session.revision_id,
                &Default::default(),
                &Default::default(),
                &mut |record, context, value| {
                    if value.is_some() {
                        outline.push((record, context.clone()));
                    }
                    Ok(())
                },
            )
            .unwrap();
        let projected = archive
            .events(&EventQuery { session_id: session.id.clone(), limit: 500, after: None })
            .unwrap()
            .items;
        let mut exported = Vec::new();
        archive.write_normalized(&session.id, ExportFormat::Jsonl, &mut exported).unwrap();
        let export: Vec<Value> = String::from_utf8(exported)
            .unwrap()
            .lines()
            .skip(1)
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut events = Vec::new();
        let mut shell_ordinal = None;
        let revision = archive.revision(session.revision_id).unwrap();
        archive
            .read_window(&revision, &outline, 64 * 1024, &Default::default(), &mut |row| {
                let anchor =
                    row.address(agent_history::read_model::ConversationId(session.id.clone()));
                assert_eq!(anchor.source_id.0, session.source_id);
                assert_eq!(anchor.revision_digest, revision.digest);
                let interpreted = row.interpretation.unwrap();
                let event = interpreted.event;
                if event.native_id.as_deref() == Some("bash-parity") {
                    check_shell_parts(&interpreted.parts);
                    assert_eq!(event.text, "pwd\n/work");
                    shell_ordinal = Some(event.ordinal);
                }
                assert_eq!(anchor.native_id, event.native_id);
                events.push(
                    serde_json::to_value(agent_history::query::EventView {
                        source_id: session.source_id.clone(),
                        event,
                    })
                    .unwrap(),
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(
            events,
            serde_json::to_value(projected).unwrap().as_array().unwrap().clone(),
            "{}",
            session.harness
        );
        assert_eq!(events, export, "{}", session.harness);
        if matches!(session.harness, Harness::Cline | Harness::RooCode) {
            let head = archive.reader_header(&session.id).unwrap();
            let rows =
                archive.reader_visible_window(&head, None, 0, 20, &Default::default()).unwrap();
            let summary = rows
                .iter()
                .find(|r| r.parts.iter().any(|p| p.text == "Compacted context"))
                .unwrap();
            assert!(
                matches!(&summary.boundary,Some(b) if b.kind==agent_history::read_model::BoundaryKind::Compaction)
            );
        }
        if session.harness == Harness::Pi {
            let header = archive.reader_header(&session.id).unwrap();
            let displayed = archive
                .reader_window(&header, None, shell_ordinal.unwrap(), 1, &Default::default())
                .unwrap();
            check_shell_parts(&displayed[0].parts);
        }
        let mut markdown = Vec::new();
        archive.write_normalized(&session.id, ExportFormat::Markdown, &mut markdown).unwrap();
        let markdown = String::from_utf8(markdown).unwrap();
        let blocks: Vec<Value> = markdown
            .split("```json\n")
            .skip(2)
            .map(|tail| serde_json::from_str(tail.split_once("\n```").unwrap().0).unwrap())
            .collect();
        for event in &mut events {
            event.as_object_mut().unwrap().remove("source_id");
        }
        assert_eq!(blocks, events, "{}", session.harness);
    }
}

fn check_shell_parts(parts: &[agent_history::read_model::RecordPart]) {
    use agent_history::read_model::PartKind;
    assert_eq!(
        parts.iter().map(|p| (p.kind, p.text.as_str())).collect::<Vec<_>>(),
        [(PartKind::CallInput, "pwd"), (PartKind::CallResult, "/work")]
    );
}
