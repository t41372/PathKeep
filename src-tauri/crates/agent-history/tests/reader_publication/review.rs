//! Exercises reader publication decisions through the same backup and navigation calls as the app.
use super::*;

#[test]
fn local_parent_replaces_the_cross_root_parent() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let local = t.path().join("local");
    let remote = t.path().join("remote");
    native::claude_main(
        &remote,
        "parent",
        &[native::claude_user("parent", "r", None, "/r", "remote")],
    );
    capture(&a, Harness::ClaudeCode, &remote);
    let remote_id = head(&a).conversation_id;
    native::claude_subagent(&local, "parent", "agent-a.jsonl", "a", "/l", "child");
    capture(&a, Harness::ClaudeCode, &local);
    let child =
        a.reader_children(&a.reader_header(&remote_id).unwrap(), None, 10).unwrap().pop().unwrap();
    native::claude_main(
        &local,
        "parent",
        &[native::claude_user("parent", "l", None, "/l", "local")],
    );
    capture(&a, Harness::ClaudeCode, &local);
    let h = a.reader_header(&child).unwrap();
    let parent = a.reader_sources(&h, None, 1).unwrap().remove(0).parent_id.unwrap();
    assert_eq!(h.parent_status, ParentStatus::Resolved);
    assert_eq!(h.parent_id.as_deref(), Some(parent.as_str()));
    assert_ne!(parent, remote_id);
    assert_eq!(a.reader_children(&a.reader_header(&parent).unwrap(), None, 10).unwrap(), [child]);
    assert!(a.reader_children(&a.reader_header(&remote_id).unwrap(), None, 10).unwrap().is_empty());
}
fn pi(
    rows: &[serde_json::Value],
    prefix: &[u8],
) -> (tempfile::TempDir, Archive, ReaderPublication) {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    write(&root.join("sessions/p.jsonl"), &[prefix, &fixtures::jsonl(rows)].concat());
    capture(&a, Harness::Pi, &root);
    let h = head(&a);
    (t, a, h)
}
#[test]
fn unresolved_selection_keeps_only_the_proven_suffix() {
    let (_t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"p"}),
            fixtures::pi_message("u", None, "user", "prompt"),
            fixtures::pi_message("a", Some("u"), "assistant", "left"),
            fixtures::pi_message("b", Some("u"), "assistant", "right"),
            fixtures::pi_message("lost", Some("missing"), "user", "suffix"),
        ],
        b"",
    );
    let rows = a.reader_visible_window(&h, None, 0, 200, &Default::default()).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
        ["unresolved_parent", "message"]
    );
    assert_eq!(texts(rows), ["suffix"]);
    assert_eq!(h.coverage_reason.as_deref(), Some("unresolved_parent"));
    assert!(!h.complete);
    assert_eq!(a.reader_window(&h, None, 0, 200, &Default::default()).unwrap().len(), 5);
}
#[test]
fn header_after_blank_is_root_and_never_a_leaf() {
    let (_t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"p"}),
            fixtures::pi_message("u", None, "user", "hello"),
        ],
        b"\n",
    );
    let member = a.reader_sources(&h, None, 1).unwrap().remove(0);
    assert_eq!(member.format_version, Some(3));
    assert_eq!(
        a.reader_leaves(&h, &h.current_source, 0, 10, &Default::default()).unwrap().ordinals,
        [2]
    );
    assert_eq!(
        a.reader_leaf_window(&h, &h.current_source, 2, 0, 10, &Default::default())
            .unwrap()
            .iter()
            .map(|r| r.ordinal.0)
            .collect::<Vec<_>>(),
        [1, 2]
    );
}
#[test]
fn cancellation_after_raw_commit_invalidates_retained_text() {
    use agent_history::capture::BackupOptions;
    let (t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"p"}),
            fixtures::pi_message("u", None, "user", "old"),
        ],
        b"",
    );
    let root = t.path().join("pi");
    use std::io::Write;
    writeln!(
        fs::OpenOptions::new().append(true).open(root.join("sessions/p.jsonl")).unwrap(),
        "{}",
        fixtures::pi_message("v", Some("u"), "assistant", "new")
    )
    .unwrap();
    let mut options = BackupOptions::default();
    let cancel = options.cancel.clone();
    options.progress = Some(std::sync::Arc::new(move |p| {
        if p.phase == "raw_committed" {
            cancel.cancel()
        }
    }));
    assert!(a.backup(&[SourceRoot { harness: Harness::Pi, root }], &options).is_err());
    let current = a.reader_header(&h.conversation_id).unwrap();
    assert!(!current.complete);
    assert!(!a.reader_sources(&current, None, 1).unwrap()[0].raw_head_current);
    assert_eq!(
        texts(a.reader_visible_window(&current, None, 0, 10, &Default::default()).unwrap()),
        ["old"]
    );
}
#[test]
fn claude_boundary_survives_without_content() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    native::claude_main(
        &root,
        "p",
        &[
            native::claude_user("p", "u", None, "/p", "before"),
            json!({"type":"system","subtype":"compact_boundary","uuid":"c","compactMetadata":{"preTokens":721}}),
        ],
    );
    capture(&a, Harness::ClaudeCode, &root);
    let h = head(&a);
    let rows = a.reader_visible_window(&h, None, 0, 20, &Default::default()).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(serde_json::to_value(&rows[1]).unwrap()["boundary"]["tokens_before"], 721);
}
#[test]
fn filtered_window_has_one_decoded_work_budget() {
    let mut rows = vec![json!({"type":"session","id":"p","version":3})];
    for i in 1..20 {
        rows.push(if i % 2 == 0 {
            fixtures::pi_message(&i.to_string(), None, "user", "some text")
        } else {
            json!({"type":"custom","id":i.to_string()})
        });
        rows[i]["parentId"] = rows[i - 1]["id"].clone();
    }
    let budget = rows.iter().map(|r| r.to_string().len() + 1).max().unwrap() as u64;
    let (_t, a, h) = pi(&rows, b"");
    assert!(matches!(
        a.reader_visible_window(
            &h,
            None,
            0,
            10,
            &JobControl { max_decoded_bytes: budget, ..Default::default() }
        ),
        Err(Error::LimitExceeded(_))
    ));
}

#[test]
fn codex_outer_and_event_compaction_are_typed() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex");
    let id = "11111111-1111-4111-8111-111111111111";
    native::codex_rollout(
        &root,
        "2026-11-10T12-00-00",
        id,
        &[
            native::codex_meta(id, "/p"),
            json!({"type":"compacted","payload":{}}),
            json!({"type":"event_msg","payload":{"type":"context_compacted"}}),
        ],
    );
    capture(&a, Harness::Codex, &root);
    let rows = a.reader_visible_window(&head(&a), None, 0, 10, &Default::default()).unwrap();
    let boundaries: Vec<_> =
        rows.iter().filter_map(|r| r.boundary.as_ref().map(|b| (r.ordinal.0, &b.kind))).collect();
    assert_eq!(boundaries, [(1, &BoundaryKind::Compaction), (2, &BoundaryKind::Compaction)]);
}

#[test]
fn adjacent_duplicate_parent_does_not_prove_ancestry() {
    let (_t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"p"}),
            fixtures::pi_message("dup", None, "assistant", "first copy"),
            fixtures::pi_message("dup", None, "assistant", "adjacent copy"),
            fixtures::pi_message("last", Some("dup"), "user", "proven suffix"),
        ],
        b"",
    );
    assert_eq!(h.coverage_reason.as_deref(), Some("unresolved_parent"));
    assert_eq!(
        texts(a.reader_visible_window(&h, None, 0, 20, &Default::default()).unwrap()),
        ["proven suffix"]
    );
}
#[test]
fn missing_modern_parent_field_is_not_a_root_declaration() {
    let mut row = fixtures::pi_message("orphan", None, "user", "surviving message");
    row.as_object_mut().unwrap().remove("parentId");
    let (_t, a, h) = pi(&[json!({"type":"session","version":3,"id":"p"}), row], b"");
    let rows = a.reader_visible_window(&h, None, 0, 8, &Default::default()).unwrap();
    assert_eq!(rows[0].kind, "unresolved_parent");
}

#[test]
fn admitted_backup_flushes_a_pending_ticket_on_early_cancel() {
    let (t, a, old) = pi(
        &[
            json!({"type":"session","id":"p","version":3}),
            fixtures::pi_message("m", None, "user", "saved"),
        ],
        b"",
    );
    a.set_coverage(&old.current_source, false).unwrap();
    let opts = agent_history::capture::BackupOptions::default();
    opts.cancel.cancel();
    let result = a.backup(&[SourceRoot { harness: Harness::Pi, root: t.path().join("pi") }], &opts);
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(!a.reader_header(&old.conversation_id).unwrap().complete);
}

#[test]
fn later_pi_id_cannot_steal_an_earlier_edge() {
    let (_t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"session"}),
            fixtures::pi_message("p", None, "user", "original"),
            fixtures::pi_message("a", Some("p"), "assistant", "reply"),
            fixtures::pi_message("p", None, "user", "alternative"),
            fixtures::pi_message("z", Some("a"), "assistant", "last"),
        ],
        b"",
    );
    assert_eq!(
        texts(a.reader_visible_window(&h, None, 0, 10, &Default::default()).unwrap()),
        ["original", "reply", "last"]
    );
    let mut tips =
        a.reader_leaves(&h, &h.current_source, 0, 10, &Default::default()).unwrap().ordinals;
    tips.sort_unstable();
    assert_eq!(tips, [3, 4]);
}
#[test]
fn idless_pi_tail_has_a_marker_and_proven_content() {
    let mut tail = fixtures::pi_message("missing", Some("a"), "assistant", "answer");
    tail.as_object_mut().unwrap().remove("id");
    let (_t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"session"}),
            fixtures::pi_message("a", None, "user", "prompt"),
            tail,
        ],
        b"",
    );
    let rows = a.reader_visible_window(&h, None, 0, 8, &Default::default()).unwrap();
    assert_eq!(rows.first().map(|r| r.kind.as_str()), Some("unresolved_parent"));
    assert_eq!(texts(rows), ["prompt", "answer"]);
    assert!(!h.complete);
    let (_t, a, h) = pi(&[json!({"type":"session","version":3,"id":"empty"})], b"");
    assert!(a.reader_visible_window(&h, None, 0, 8, &Default::default()).unwrap().is_empty());
}

#[test]
fn pi_cycle_has_a_marked_provable_suffix() {
    let (_t, a, h) = pi(
        &[
            json!({"type":"session","version":3,"id":"session"}),
            fixtures::pi_message("a", Some("b"), "user", "first"),
            fixtures::pi_message("b", Some("a"), "assistant", "second"),
        ],
        b"",
    );
    let rows = a.reader_visible_window(&h, None, 0, 8, &Default::default()).unwrap();
    assert_eq!(rows[0].kind, "unresolved_parent");
    assert_eq!(texts(rows), ["first", "second"]);
    assert!(!h.complete);
}
