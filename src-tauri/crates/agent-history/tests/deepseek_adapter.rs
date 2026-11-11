//! Responsibilities: pin native DeepSeek preservation and discovery failure contracts.
//! Not responsible for: archive durability, compression, or executing the harness.
//!
//! Written before the adapter. Failure inventory: generation ordering versus lexical
//! ordering; mixed encodings; malformed/oversized/torn records; future/legacy schema;
//! required unknown events; duplicate/gapped sequence numbers; unknown-origin claims;
//! lost injected context, reasoning, tool payloads or references; and attachment reads
//! accidentally authorized by opaque JSON. Native lifecycle follows upstream
//! fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159, session-format-v3-to-v4/tests/relationships.spec.ts.

use agent_history::adapters::{
    Adapter, ArtifactKind, DeepseekAdapter, Harness, NativeEvent, ParseContext, ParseReport,
    SourceRoot,
};
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;

fn header(version: u64) -> Value {
    json!({"type":"session","version":version,"id":"native-relations",
        "createdAt":1,"cwd":"/work/project","delegationDepth":0,"isSeeded":false})
}

fn lifecycle() -> Vec<Value> {
    let mut events = vec![
        json!({"type":"turn/start","data":{"turn":1}}),
        json!({"type":"step/start","data":{"turn":1,"step":1}}),
        json!({"type":"user/message","data":{"id":"input","role":"user",
            "source":{"kind":"user"},"content":[{"type":"text","text":"Read the report"}]},
            "surfaceOp":"append"}),
        json!({"type":"request/header","data":{"reason":"initial",
            "header":{"config":{"provider":"mock","model":"mock"}}}}),
        json!({"type":"assistant/message","data":{"turn":1,"step":1,"stream":[],
            "message":{"id":"assistant","role":"assistant",
                "source":{"kind":"model","provider":"mock","model":"mock"},
                "content":[{"type":"tool-call","id":"call","name":"read","arguments":"{}"}]}},
            "surfaceOp":"append"}),
        json!({"type":"tool/call","data":{"turn":1,"step":1,"callId":"call","name":"read","arguments":"{}"}}),
        json!({"type":"tool/result","data":{"turn":1,"step":1,"message":{
            "id":"result","role":"tool","source":{"kind":"tool","callId":"call"},
            "toolCallId":"call","content":[{"type":"text","text":"Report complete"}]}},
            "surfaceOp":"append"}),
        json!({"type":"step/end","data":{"turn":1,"step":1}}),
        json!({"type":"turn/end","data":{"turn":1,"reason":{"kind":"completed"}}}),
    ];
    for (seq, event) in events.iter_mut().enumerate() {
        event["seq"] = json!(seq);
        event["time"] = json!(seq + 1);
    }
    events
}

fn parse(rows: &[Value]) -> (ParseReport, Vec<NativeEvent>) {
    let bytes = rows.iter().map(|row| format!("{row}\n")).collect::<String>();
    parse_bytes(bytes.as_bytes(), 1024 * 1024)
}

fn parse_bytes(bytes: &[u8], max_record_bytes: usize) -> (ParseReport, Vec<NativeEvent>) {
    let mut events = Vec::new();
    let report = DeepseekAdapter
        .parse(
            &mut Cursor::new(bytes),
            &ParseContext {
                relative_path: "sessions/--work-project--/native-relations/session.v4.jsonl".into(),
                project_hint: Some("wrong-lossy-slug".into()),
                max_record_bytes,
            },
            &mut |event| {
                events.push(event);
                Ok(())
            },
        )
        .unwrap();
    (report, events)
}

#[test]
fn preserves_complete_native_lifecycle_and_never_infers_launch_origin() {
    let mut rows = vec![header(4)];
    rows.extend(lifecycle());
    let (report, events) = parse(&rows);
    assert!(report.complete, "{:?}", report.diagnostics);
    let session = report.session.unwrap();
    assert_eq!(session.native_id, "native-relations");
    assert_eq!(session.project_path.as_deref(), Some("/work/project"));
    assert_eq!(session.origin, "unknown");
    assert_eq!(session.model.as_deref(), Some("mock"));
    assert_eq!(events.len(), rows.len());
    assert_eq!(events[3].native_id.as_deref(), Some("input"));
    assert_eq!(events[7].role.as_deref(), Some("tool"));
    assert_eq!(events[7].text, "Report complete");
    for (ordinal, event) in events.iter().enumerate() {
        assert_eq!(event.ordinal, ordinal as u64);
        assert_eq!(event.payload, rows[ordinal]);
    }
}

#[test]
fn only_declared_content_authorizes_attachment_references() {
    let image_hash = "a".repeat(64);
    let file_hash = "b".repeat(64);
    let image = json!({"type":"image","attachment":{"attachmentId":format!("sha256:{image_hash}"),
        "mediaType":"image/png","bytes":10,"width":1,"height":1}});
    let file = json!({"type":"file","attachment":{"attachmentId":format!("sha256:{file_hash}"),
        "bytes":20,"name":"report.txt"}});
    let rows = vec![
        header(4),
        json!({"type":"user/message","seq":0,"time":2,"surfaceOp":"append","data":{
            "id":"input","role":"user","source":{"kind":"runtime-context"},
            "content":[{"type":"text","text":"Injected context"}, image, file]}}),
        json!({"type":"custom/opaque","seq":1,"time":3,"ignorable":true,
            "data":{"content":[{"type":"image","attachment":{"attachmentId":format!("sha256:{}", "c".repeat(64))}}],
                "path":"/private/credentials"}}),
    ];
    let (report, events) = parse(&rows);
    assert!(report.complete);
    assert_eq!(report.references.len(), 2);
    assert_eq!(report.references[0].target, format!("attachments/v1/objects/aa/{image_hash}"));
    assert_eq!(report.references[1].target, format!("attachments/v1/file-objects/bb/{file_hash}"));
    assert_eq!(events[1].text, "Injected context");
    assert_eq!(events[1].payload["data"]["source"]["kind"], "runtime-context");
    assert!(events[2].text.is_empty());
}

#[test]
fn unknown_required_events_remain_raw_and_report_limited_interpretation() {
    let rows = vec![
        header(4),
        json!({"type":"custom/required","seq":0,"time":2,"data":{"secret":"never diagnostics"}}),
    ];
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert_eq!(events[1].payload, rows[1]);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "deepseek-unknown-required-event"));
    assert!(!format!("{:?}", report.diagnostics).contains("never diagnostics"));
}

#[test]
fn future_and_legacy_versions_are_retained_without_claiming_full_projection() {
    for version in [0, 1, 2, 3, 5] {
        let rows = vec![
            header(version),
            json!({"type":"user/message","seq":0,"time":2,
            "data":{"id":"input","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"Retained"}]},"surfaceOp":"append"}),
        ];
        let (report, events) = parse(&rows);
        assert!(!report.complete, "version {version}");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].payload, rows[1]);
        assert!(report.diagnostics.iter().any(|issue| issue.code == "deepseek-format-limited"));
        if version > 4 {
            assert!(events[1].text.is_empty());
            assert!(events[1].role.is_none());
        }
    }
}

#[test]
fn sequence_gaps_and_invalid_headers_do_not_discard_raw_records() {
    let rows = vec![header(4), json!({"type":"turn/start","seq":2,"time":2,"data":{"turn":1}})];
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert_eq!(events.len(), 2);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "deepseek-sequence-invalid"));
    let (report, events) = parse(&[json!({"type":"session","version":4}), rows[1].clone()]);
    assert!(report.session.is_none());
    assert!(!report.complete);
    assert_eq!(events.len(), 2);
}

#[test]
fn torn_malformed_and_oversized_records_are_explicit_partial_coverage() {
    // Native writer commits newline-terminated records; valid JSON without LF
    // is still an uncommitted physical suffix in DeepSeek's own reader.
    assert!(!parse_bytes(header(4).to_string().as_bytes(), 1024).0.complete);
    let mut bytes = format!("{}\n", header(4)).into_bytes();
    bytes.extend_from_slice(b"{\"type\":\"user/message\"");
    assert!(!parse_bytes(&bytes, 1024).0.complete);
    let bytes = format!("{}\nnot-json\n", header(4));
    assert!(!parse_bytes(bytes.as_bytes(), 1024).0.complete);
    let rows = [
        header(4),
        json!({"type":"custom/huge","seq":0,"time":2,"ignorable":true,"data":{"text":"x".repeat(2048)}}),
    ];
    let bytes = rows.iter().map(|row| format!("{row}\n")).collect::<String>();
    assert!(!parse_bytes(bytes.as_bytes(), 512).0.complete);
}

#[test]
fn preserves_fork_cut_and_latest_title_without_mistaking_parent_for_launch_origin() {
    let mut source = header(4);
    source["parentSession"] = json!("parent-session");
    source["isSeeded"] = json!(true);
    let mut rows = vec![source];
    rows.extend(lifecycle());
    rows.push(json!({"type":"session/end-seed","seq":9,"time":10,"data":{"inherited":true}}));
    rows.push(json!({"type":"session/title","seq":10,"time":11,
        "data":{"title":"A retained fork","messageSeqs":[],"source":{"kind":"user"}}}));
    let (report, events) = parse(&rows);
    assert!(report.complete);
    let session = report.session.unwrap();
    assert_eq!(session.parent_session_id.as_deref(), Some("parent-session"));
    assert_eq!(session.origin, "unknown");
    assert_eq!(session.title.as_deref(), Some("A retained fork"));
    assert_eq!(events[10].payload["data"]["inherited"], true);
    assert!(
        report
            .references
            .iter()
            .any(|reference| reference.kind == "lineage" && reference.target == "parent-session")
    );
}

#[test]
fn discovery_keeps_generations_but_only_selects_the_highest_canonical_number() {
    let temp = tempfile::tempdir().unwrap();
    let session = temp.path().join("sessions/--work--/id");
    fs::create_dir_all(&session).unwrap();
    for filename in [
        "session.jsonl",
        "session.v2.jsonl",
        "session.v10.jsonl",
        "session.v01.jsonl",
        "session.V11.jsonl",
        "session.lock",
    ] {
        fs::write(session.join(filename), b"raw").unwrap();
    }
    let attachment = temp.path().join(format!("attachments/v1/objects/aa/{}", "a".repeat(64)));
    fs::create_dir_all(attachment.parent().unwrap()).unwrap();
    fs::write(&attachment, b"image").unwrap();
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    DeepseekAdapter
        .discover(
            &SourceRoot { harness: Harness::Deepseek, root: temp.path().to_owned() },
            &mut |artifact| {
                artifacts.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    let transcripts: Vec<_> =
        artifacts.iter().filter(|artifact| artifact.kind == ArtifactKind::Transcript).collect();
    assert_eq!(transcripts.len(), 3);
    assert_eq!(transcripts.iter().filter(|artifact| artifact.current).count(), 1);
    assert_eq!(transcripts.iter().find(|artifact| artifact.current).unwrap().generation, Some(10));
    assert!(artifacts.iter().any(|artifact| artifact.path == attachment));
}

#[test]
fn ambiguous_same_generation_encodings_are_preserved_without_selecting_a_winner() {
    let temp = tempfile::tempdir().unwrap();
    let session = temp.path().join("--work--/id");
    fs::create_dir_all(&session).unwrap();
    fs::write(session.join("session.v4.jsonl"), b"raw").unwrap();
    fs::write(session.join("session.v4.jsonl.zstd"), b"compressed").unwrap();
    let mut artifacts = Vec::new();
    let mut issues = Vec::new();
    DeepseekAdapter
        .discover(
            &SourceRoot { harness: Harness::Deepseek, root: temp.path().to_owned() },
            &mut |artifact| {
                artifacts.push(artifact);
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert_eq!(artifacts.len(), 2);
    assert!(artifacts.iter().all(|artifact| !artifact.current));
    assert!(issues.iter().any(|issue| issue.code == "deepseek-ambiguous-generation"));
}

#[test]
fn known_tool_names_arguments_and_legacy_results_remain_searchable() {
    let mut rows = vec![header(4)];
    rows.extend(lifecycle());
    let (_, events) = parse(&rows);
    assert!(events[5].text.contains("read"));
    assert!(events[5].text.contains("{}"));
    assert!(events[6].text.contains("read"));
    assert!(events[6].text.contains("{}"));
    rows[0]["version"] = json!(3);
    rows[7]["data"]["message"] = json!({"id":"result","role":"user",
        "source":{"kind":"tool","callId":"call"},"content":[{"type":"tool-result",
        "toolCallId":"call","isError":false,"content":[{"type":"text","text":"Legacy output"}]}]});
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert_eq!(events[7].role.as_deref(), Some("tool"));
    assert_eq!(events[7].text, "Legacy output");
}

#[test]
fn known_malformed_messages_and_timestamps_cannot_claim_complete_projection() {
    let mut cases = Vec::new();
    let valid = lifecycle()[2].clone();
    let mut missing = valid.clone();
    missing["data"].as_object_mut().unwrap().remove("content");
    cases.push(missing);
    let mut invalid_content = valid.clone();
    invalid_content["data"]["content"] = json!("not native blocks");
    cases.push(invalid_content);
    let mut invalid_role = valid.clone();
    invalid_role["data"]["role"] = json!("assistant");
    cases.push(invalid_role);
    let mut invalid_time = valid;
    invalid_time["time"] = json!("2026-11-11T00:00:00Z");
    cases.push(invalid_time);
    cases.push(json!({"type":"assistant/message","seq":0,"time":2,"data":{"turn":1,"step":1}}));
    for mut candidate in cases {
        candidate["seq"] = json!(0);
        let (report, events) = parse(&[header(4), candidate.clone()]);
        assert!(!report.complete, "{candidate}");
        assert_eq!(events[1].payload, candidate);
        assert!(report.diagnostics.iter().any(|issue| issue.code == "deepseek-record-invalid"));
    }
}

#[test]
fn rendered_spill_notice_reports_unresolved_output_without_authorizing_path_reads() {
    let mut rows = vec![header(4)];
    rows.extend(lifecycle());
    rows[7]["data"]["message"]["content"] = json!([{"type":"text","text":
        "Preview\n\n(Omitted 120 bytes. Full formatted result stored at: /private/secret.txt. Use read with offset/limit, or grep this path to search within it.)"}]);
    let (report, events) = parse(&rows);
    assert!(!report.complete);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "deepseek-spill-coverage-unknown"));
    assert!(report.references.is_empty());
    assert!(events[7].text.contains("/private/secret.txt"));
    let dispatch = json!({"type":"tool/ptc-dispatch","seq":0,"time":2,"data":{
        "name":"read","arguments":{},"content":rows[7]["data"]["message"]["content"]}});
    let (report, _) = parse(&[header(4), dispatch]);
    assert!(!report.complete);
    assert!(report.diagnostics.iter().any(|issue| issue.code == "deepseek-spill-coverage-unknown"));
    assert!(report.references.is_empty());
}

#[cfg(unix)]
#[test]
fn discovery_rejects_source_and_attachment_ancestor_symlinks() {
    use std::os::unix::fs::symlink;
    for escaped_component in ["root", "attachments", "v1"] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let external = temp.path().join("external");
        fs::create_dir_all(external.join("sessions/project/id")).unwrap();
        fs::write(external.join("sessions/project/id/session.v4.jsonl"), b"outside").unwrap();
        fs::create_dir_all(external.join("v1/objects/aa")).unwrap();
        fs::create_dir_all(external.join("objects/aa")).unwrap();
        fs::write(external.join(format!("v1/objects/aa/{}", "a".repeat(64))), b"outside").unwrap();
        fs::write(external.join(format!("objects/aa/{}", "a".repeat(64))), b"outside").unwrap();
        match escaped_component {
            "root" => symlink(&external, &source).unwrap(),
            "attachments" => {
                fs::create_dir_all(source.join("sessions")).unwrap();
                symlink(&external, source.join("attachments")).unwrap();
            }
            _ => {
                fs::create_dir_all(source.join("sessions")).unwrap();
                fs::create_dir_all(source.join("attachments")).unwrap();
                symlink(&external, source.join("attachments/v1")).unwrap();
            }
        }
        let mut artifacts = Vec::new();
        let mut issues = Vec::new();
        DeepseekAdapter
            .discover(
                &SourceRoot { harness: Harness::Deepseek, root: source },
                &mut |artifact| {
                    artifacts.push(artifact);
                    Ok(())
                },
                &mut issues,
            )
            .unwrap();
        assert!(artifacts.is_empty(), "escaped {escaped_component}");
        assert!(!issues.is_empty(), "unreported {escaped_component}");
    }
}

#[test]
fn discovery_streams_every_generation_from_a_large_flat_directory() {
    let temp = tempfile::tempdir().unwrap();
    for generation in 1..=512 {
        fs::write(temp.path().join(format!("session.v{generation}.jsonl")), b"raw").unwrap();
    }
    let mut emitted = 0;
    let mut selected = Vec::new();
    let mut issues = Vec::new();
    DeepseekAdapter
        .discover(
            &SourceRoot { harness: Harness::Deepseek, root: temp.path().to_owned() },
            &mut |artifact| {
                emitted += 1;
                if artifact.current {
                    selected.push(artifact.generation);
                }
                Ok(())
            },
            &mut issues,
        )
        .unwrap();
    assert_eq!(emitted, 512);
    assert_eq!(selected, vec![Some(512)]);
    assert!(issues.is_empty());
}
