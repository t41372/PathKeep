//! Responsibilities: leave ambiguous physical rollout prefixes unresolved across roots.
//! Not responsible for: splitting the conversation key or preferring a capture order.
//! Failure mode: a matching thread ID and equal boundary lengths select a stranger's prefix.
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::native::*;
use support::{Case, args, string};

#[test]
fn divergent_copies_of_a_physical_rollout_do_not_supply_an_arbitrary_prefix() {
    let mut case = Case::unencrypted("ambiguous-native-prefix");
    let base = support::harness::canonical_root(&case);
    let roots = [base.join("a"), base.join("b"), base.join("child")];
    let thread = "33333333-3333-4333-8333-333333333333";
    let spec = |turns| RolloutSpec {
        thread,
        rollout: None,
        name_hour: 12,
        clock_hour: 12,
        cwd: "/synthetic/project",
        base: None,
        meta: json!({}),
        turns,
    };
    let first = codex_thread_rollout(&roots[0], spec(&["originone"]));
    let other = codex_thread_rollout(&roots[1], spec(&["origintwo"]));
    assert_eq!(
        support::fixtures::jsonl(&first.rows).len(),
        support::fixtures::jsonl(&other.rows).len()
    );
    let child = codex_thread_rollout(
        &roots[2],
        RolloutSpec {
            thread,
            rollout: Some("44444444-4444-4444-8444-444444444444"),
            name_hour: 13,
            clock_hour: 13,
            cwd: "/synthetic/project",
            base: Some((&first, 3)),
            meta: json!({}),
            turns: &["child body"],
        },
    );
    case.fixture(json!({"seed":"ambiguous-native-prefix-v1","private_data":false}));
    case.init();
    let mut arguments = Vec::new();
    for root in &roots {
        arguments.extend(args(&["--source", &format!("codex={}", root.display())]));
    }
    arguments.push("--execute".into());
    case.call("backup", arguments);
    let conversation = case.conversations().remove(0);
    let current = support::harness::current_source(&conversation);
    case.equal(
        "ambiguous evidence remains unresolved",
        current["relation"].clone(),
        json!("whole"),
    );
    let events = case.all("events", args(&["--session", string(&conversation, "id")]));
    case.equal("only its own records are read", json!(events.len()), json!(child.rows.len()));
}

// A cycle in two distinct threads used to abort backup before it could index a healthy third.
#[test]
fn native_cycle_keeps_each_conversation_readable() {
    use agent_history::{Archive, Harness, SearchQuery, SourceRoot, query::EventQuery};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    let first = "11111111-1111-4111-8111-111111111111";
    let second = "22222222-2222-4222-8222-222222222222";
    let healthy = "33333333-3333-4333-8333-333333333333";
    for (id, parent, text) in [
        (first, Some(second), "firstcycle"),
        (second, Some(first), "secondcycle"),
        (healthy, None, "healthyneedle"),
    ] {
        let mut meta = codex_meta(id, "/synthetic/project");
        if let Some(parent) = parent {
            meta["payload"]["history_base"] = json!({
                "thread_id":parent, "end_ordinal_exclusive":0, "end_byte_offset":0
            });
        }
        codex_rollout(&root, "2026-11-10T12-00-00", id, &[meta, codex_user(text)]);
    }
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let report = archive
        .backup(&[SourceRoot { harness: Harness::Codex, root }], &Default::default())
        .unwrap();
    assert_eq!(report.status, "partial");
    for id in [first, second] {
        assert!(report.issues.iter().any(|issue| issue.code == "lineage_unresolved"
            && issue.detail.as_deref() == Some("cycle")
            && issue.path.as_deref().is_some_and(|p| p.contains(id))));
    }
    let conversations = archive.list_sessions(&Default::default()).unwrap().items;
    assert_eq!(conversations.len(), 3);
    for item in conversations {
        assert_eq!(item.complete, item.native_id == healthy);
        let events = archive.events(&EventQuery::new(item.id)).unwrap();
        assert_eq!(events.items.len(), 2);
        assert!(events.items.iter().all(|event| event.source_id == item.source_id));
    }
    let search = archive
        .search(&SearchQuery {
            text: "healthyneedle".into(),
            harness: None,
            project_path: None,
            limit: 10,
        })
        .unwrap();
    assert_eq!(search.total, 1);
    assert!(!search.complete);
    let verified = archive.verify().unwrap();
    assert_eq!(verified.revisions, 3);
    assert!(!verified.normalization_complete);
}

// A producer can publish the readable header of a rollout with a corrupt second zstd frame.
// Interpreting a child's boundary inside that frame used to abort unrelated captures.
#[test]
fn undecodable_predecessor_boundary_keeps_the_child_readable() {
    use agent_history::{Archive, Harness, SessionMetadata, SourceRoot};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    let base = "11111111-1111-4111-8111-111111111111";
    let child = "22222222-2222-4222-8222-222222222222";
    let first = codex_meta(base, "/synthetic/project");
    let mut bytes = zstd_frame(std::slice::from_ref(&first));
    bytes.extend_from_slice(b"unreadable second frame");
    let path = root.join(format!("sessions/rollout-2026-11-10T12-00-00-{base}.jsonl.zst"));
    support::write(&path, &bytes);
    let sources = [SourceRoot { harness: Harness::Codex, root: root.clone() }];
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    archive.backup(&sources, &Default::default()).unwrap();
    let raw = archive.list_sources(&Default::default()).unwrap().items.remove(0);
    let attempt = archive
        .prepare_projection(&raw.id, raw.latest_revision_id.unwrap(), "header-only", true)
        .unwrap()
        .unwrap();
    archive
        .publish_projection_attempt(
            &attempt,
            &SessionMetadata { native_id: base.into(), ..Default::default() },
            false,
        )
        .unwrap();
    std::fs::remove_file(path).unwrap();
    let mut meta = codex_meta(child, "/synthetic/project");
    meta["payload"]["history_base"] = json!({"thread_id":base,
        "end_ordinal_exclusive":2,"end_byte_offset":support::fixtures::jsonl(&[first]).len()+100});
    codex_rollout(&root, "2026-11-10T13-00-00", child, &[meta, codex_user("survivor")]);
    let report = archive.backup(&sources, &Default::default()).unwrap();
    assert!(report.issues.iter().any(|issue| issue.code == "lineage_unresolved"
        && issue.path.as_deref().is_some_and(|p| p.contains(child))));
    let child = archive
        .list_sessions(&Default::default())
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.native_id == child)
        .unwrap();
    assert!(!child.complete);
    assert_eq!(
        archive.events(&agent_history::query::EventQuery::new(child.id)).unwrap().items.len(),
        2
    );
    let mut exported = Vec::new();
    archive.write_raw(raw.latest_revision_id.unwrap(), &mut exported).unwrap();
    assert_eq!(exported, bytes);
}
