//! Responsibilities: count only unkept references on the inherited line and retain their reasons.
//! Not responsible for: reading live attachments during offline reparse.
//! Failure modes: excluded base tails inflate counts; reparse loses the capture-time reason.
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::native::*;
use support::{Case, args, string, write};

#[test]
fn inherited_attachment_counts_exclude_the_abandoned_tail_and_keep_reparse_reasons() {
    let mut case = Case::unencrypted("attachment-prefix-reasons");
    let base = support::harness::canonical_root(&case);
    let root = base.join("codex");
    let first_path = base.join("desktop/first-private.png");
    let last_path = base.join("desktop/last-private.png");
    write(&first_path, b"outside roots");
    write(&last_path, b"outside roots");
    let thread = "33333333-3333-4333-8333-333333333333";
    let attachment = |path: &std::path::Path| {
        json!({"timestamp":TIME,"type":"event_msg",
        "payload":{"type":"user_message","message":"image","local_images":[path]}})
    };
    let rows = vec![
        codex_meta(thread, "/synthetic/project"),
        attachment(&first_path),
        codex_reply(thread, "answer", "reply"),
        attachment(&last_path),
    ];
    let first = codex_rollout(&root, "2026-11-10T12-00-00", thread, &rows);
    let mut header = codex_meta(thread, "/synthetic/project");
    header["payload"]["timestamp"] = json!("2026-11-10T13:00:00Z");
    header["payload"]["history_base"] = json!({"thread_id":thread,
        "end_ordinal_exclusive":3,"end_byte_offset":support::fixtures::jsonl(&rows[..3]).len()});
    codex_rollout(
        &root,
        "2026-11-10T13-00-00",
        &format!("{thread}_44444444-4444-4444-8444-444444444444"),
        &[header, codex_user_at("2026-11-10T13:01:00Z", "continue")],
    );
    case.fixture(json!({"seed":"attachment-prefix-reasons-v1","private_data":false}));
    case.init();
    case.call("backup", args(&["--source", &format!("codex={}", root.display()), "--execute"]));
    let conversation = case.conversations().remove(0);
    case.equal(
        "only the inherited attachment counts",
        conversation["attachments_not_kept"].clone(),
        json!(1),
    );
    case.equal("all line records parse", conversation["complete"].clone(), json!(true));
    let sources = case.source_index();
    let source = sources
        .values()
        .find(|source| root.join(string(source, "relative_path")) == first)
        .unwrap();
    let id = string(source, "id").to_owned();
    std::fs::remove_dir_all(&root).unwrap();
    let report = case.call("reparse", args(&["--source-id", &id, "--execute"]));
    let missing = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|issue| issue["code"] == "attachment_not_retained")
        .collect::<Vec<_>>();
    case.equal(
        "both source references keep their original class",
        json!(missing.iter().map(|issue| issue["detail"].clone()).collect::<Vec<_>>()),
        json!(["outside_roots", "outside_roots"]),
    );
    case.truth(
        "targets do not leak through diagnostics",
        !report.to_string().contains("private.png"),
    );
    let after = case.conversations().remove(0);
    case.equal(
        "offline reparse keeps the bounded count",
        after["attachments_not_kept"].clone(),
        json!(1),
    );
}

#[test]
fn repeated_references_count_as_occurrences_while_capture_stays_deduplicated() {
    let mut case = Case::unencrypted("repeated-attachment-occurrences");
    let base = support::harness::canonical_root(&case);
    let root = base.join("claude");
    let outside = base.join("desktop/private.png");
    write(&outside, b"outside the approved root");
    let session = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    claude_main(
        &root,
        session,
        &[
            claude_user(session, "u", None, "/synthetic/project", "images"),
            claude_attachment(session, "a1", &outside),
            claude_attachment(session, "a2", &outside),
        ],
    );
    case.fixture(json!({"seed":"repeated-attachment-occurrences-v1","private_data":false}));
    case.init();
    let report = case.call(
        "backup",
        args(&["--source", &format!("claude-code={}", root.display()), "--execute"]),
    );
    case.equal(
        "one capture outcome is reported",
        json!(
            report["issues"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|issue| issue["detail"] == "outside_roots")
                .count()
        ),
        json!(1),
    );
    let conversation = case.conversations().remove(0);
    case.equal(
        "each source occurrence is counted",
        conversation["attachments_not_kept"].clone(),
        json!(2),
    );
    case.equal("its own records are complete", conversation["complete"].clone(), json!(true));
}

// The binding still identifies the captured SQLite snapshot after retention expires its bytes.
#[test]
fn offline_reparse_classifies_an_expired_attachment_as_missing() {
    use agent_history::{Archive, Harness, SourceRoot};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("codex");
    std::fs::create_dir(&root).unwrap();
    let state = root.join("state_5.sqlite");
    let db = rusqlite::Connection::open(&state).unwrap();
    db.execute("CREATE TABLE sample(value INTEGER)", []).unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let sources = [SourceRoot { harness: Harness::Codex, root: root.clone() }];
    let backup = || archive.backup(&sources, &Default::default()).unwrap();
    backup();
    let target = archive.list_sources(&Default::default()).unwrap().items.remove(0);
    assert_eq!(target.retention, "state");
    let revision = target.latest_revision_id.unwrap();
    let thread = "11111111-1111-4111-8111-111111111111";
    let attachment = json!({"type":"event_msg","payload":{"type":"user_message",
        "local_images":[state],"message":"snapshot"}});
    let transcript = codex_rollout(
        &root,
        "2026-11-10T12-00-00",
        thread,
        &[codex_meta(thread, "/synthetic/project"), attachment],
    );
    backup();
    let item = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    assert_eq!(item.attachments_not_kept, 0);
    std::fs::remove_file(transcript).unwrap();
    db.execute("INSERT INTO sample VALUES(2)", []).unwrap();
    drop(db);
    backup();
    assert!(!archive.revision(revision).unwrap().bytes_retained);
    std::fs::remove_dir_all(root).unwrap();
    let report = archive.reparse_source(&item.source_id, &Default::default()).unwrap();
    let issue = report.issues.iter().find(|issue| issue.code == "attachment_not_retained").unwrap();
    assert_eq!(issue.detail.as_deref(), Some("missing"));
    assert!(issue.path.is_none());
    let after = archive.session(&item.id).unwrap();
    assert_eq!(after.attachments_not_kept, 1);
    assert!(after.complete);
}
