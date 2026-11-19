//! Public reader contracts for version changes, inherited source segments and alternate leaves.
//! These fixtures use backup and offline reparse. Accounting belongs to the turn builder.
#[allow(dead_code)]
#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/native.rs"]
mod native;
use agent_history::{Archive, Error, Harness, SourceRoot, read_model::*};
use serde_json::json;
use std::{fs, path::Path};
fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn capture(a: &Archive, h: Harness, path: &Path) {
    a.backup(&[SourceRoot { harness: h, root: path.into() }], &Default::default()).unwrap();
}
fn head(a: &Archive) -> ReaderPublication {
    let id = a.list_sessions(&Default::default()).unwrap().items.remove(0).id;
    a.reader_header(&id).unwrap()
}
fn texts(rows: Vec<DisplayRow>) -> Vec<String> {
    rows.into_iter().flat_map(|r| r.parts.into_iter().map(|p| p.text)).collect()
}

#[test]
fn b_pi_alternate_leaf_and_stale_head() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    let path = root.join("sessions/p.jsonl");
    let rows = [
        json!({"type":"session","id":"p","version":3}),
        fixtures::pi_message("u", None, "user", "prompt"),
        fixtures::pi_message("a", Some("u"), "assistant", "earlier answer"),
        fixtures::pi_message("b", Some("u"), "assistant", "later answer"),
        json!({"type":"compaction","id":"c","parentId":"b","firstKeptEntryId":"b","tokensBefore":42,"summary":"shorter"}),
        json!({"type":"context_edit","id":"e","parentId":"c"}),
    ];
    write(&path, &fixtures::jsonl(&rows));
    capture(&a, Harness::Pi, &root);
    let h = head(&a);
    assert_eq!(h.physical_records, 6);
    let compaction =
        a.reader_outline(&h, None, 4, 1, &Default::default()).unwrap().pop().unwrap().1;
    assert_eq!((compaction.first_kept.as_deref(), compaction.tokens_before), (Some("b"), Some(42)));
    let choices = a.reader_leaves(&h, &h.current_source, 0, 20, &Default::default()).unwrap();
    let mut tips = choices.ordinals;
    tips.sort_unstable();
    assert_eq!(tips, [2, 5]);
    assert_eq!(choices.latest_recorded, Some(5));
    assert!(
        !texts(a.reader_visible_window(&h, None, 0, 20, &Default::default()).unwrap())
            .contains(&"earlier answer".into())
    );
    let found = a.reader_lookup(&h, &h.current_source, "a", None, 8, &Default::default()).unwrap();
    assert_eq!(found, [2]);
    let alt = a.reader_leaf_window(&h, &h.current_source, 2, 0, 200, &Default::default()).unwrap();
    assert_eq!(texts(alt), ["prompt", "earlier answer"]);
    let other =
        a.reader_leaf_window(&h, &h.current_source, 5, 0, 200, &Default::default()).unwrap();
    assert!(!texts(other).contains(&"earlier answer".into()));
    fs::remove_dir_all(root).unwrap();
    a.reparse_source(&h.current_source, &Default::default()).unwrap();
    assert_ne!(head(&a).publication, h.publication);
    assert!(matches!(
        a.reader_window(&h, None, 0, 2, &Default::default()),
        Err(Error::StaleCursor)
    ));
}

#[test]
fn c_late_base_publishes_clipped_line_and_keeps_abandoned_branch() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex");
    let thread = native::codex_reverted_thread(
        &root,
        "11111111-1111-4111-8111-111111111111",
        "22222222-2222-4222-8222-222222222222",
        "/base",
        "/continued",
    );
    let base = fs::read(&thread.base).unwrap();
    fs::remove_file(&thread.base).unwrap();
    capture(&a, Harness::Codex, &root);
    let partial = head(&a);
    assert!(!partial.complete);
    assert_eq!(partial.physical_records, 3);
    write(&thread.base, &base);
    capture(&a, Harness::Codex, &root);
    let h = head(&a);
    assert!(h.complete);
    assert_eq!(h.physical_records, 6);
    let bytes = a
        .reader_outline(&h, None, 0, 6, &Default::default())
        .unwrap()
        .iter()
        .map(|(_, entry)| entry.locator.length)
        .sum::<u64>();
    assert!(matches!(
        a.reader_window(
            &h,
            None,
            0,
            6,
            &JobControl { max_decoded_bytes: bytes - 1, ..Default::default() }
        ),
        Err(Error::LimitExceeded(_))
    ));
    let members = a.reader_sources(&h, None, 10).unwrap();
    assert_eq!(members.len(), 2);
    let base = members.iter().find(|s| s.source_id != h.current_source).unwrap();
    let continued = members.iter().find(|m| m.source_id == h.current_source).unwrap();
    let cut = continued.resolved_base.as_ref().unwrap();
    assert_eq!((continued.own_records, continued.line_records, base.line_records), (3, 6, 5));
    assert_eq!((&cut.source_id, cut.own_end), (&base.source_id, 3));
    use sha2::Digest;
    assert_eq!(
        cut.prefix_digest,
        hex::encode(sha2::Sha256::digest(fixtures::jsonl(&thread.base_rows[..3])))
    );
    let current = texts(a.reader_window(&h, None, 0, 200, &Default::default()).unwrap());
    assert!(current.iter().any(|s| s.contains("first answer")));
    assert!(!current.iter().any(|s| s.contains("reverted answer")));
    let earlier =
        texts(a.reader_window(&h, Some(&base.source_id), 0, 200, &Default::default()).unwrap());
    assert!(earlier.iter().any(|s| s.contains("reverted answer")));
}

#[test]
fn d_unreadable_higher_generation_has_no_inherited_rendering() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("deepseek");
    let folder = root.join("sessions/project/d");
    write(
        &folder.join("session.jsonl"),
        &fixtures::jsonl(&[
            json!({"type":"session","id":"d","version":4,"createdAt":0,"isSeeded":false,"delegationDepth":0}),
        ]),
    );
    capture(&a, Harness::Deepseek, &root);
    let first = head(&a);
    write(&folder.join("session.v5.jsonl.zstd"), b"broken");
    capture(&a, Harness::Deepseek, &root);
    let h = a.reader_header(&first.conversation_id).unwrap();
    assert_ne!(h.publication, first.publication);
    assert!(!h.complete);
    assert!(a.reader_sources(&h, None, 1).unwrap()[0].raw_head_current);
    assert_eq!(h.physical_records, 0);
    assert!(a.reader_window(&h, None, 0, 2, &Default::default()).unwrap().is_empty());
}

#[test]
fn i_abandoned_staging_survives_reopen_and_stale_builder_cannot_switch() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("archive");
    let a = Archive::create_unencrypted(&dir).unwrap();
    let root = t.path().join("pi");
    fixtures::pi(&root, "/project", "p", "original");
    capture(&a, Harness::Pi, &root);
    let h = head(&a);
    let info = a.latest_revision(&h.current_source).unwrap().unwrap();
    let old = a.prepare_projection(&h.current_source, info.id, "older", true).unwrap().unwrap();
    let exhausted = JobControl { max_decoded_bytes: 1, ..Default::default() };
    assert!(matches!(
        a.stage_reader(&old, &Default::default(), &exhausted),
        Err(Error::LimitExceeded(_))
    ));
    assert_eq!(head(&a).publication, h.publication);
    a.stage_reader(&old, &Default::default(), &Default::default()).unwrap();
    let new = a.prepare_projection(&h.current_source, info.id, "newer", true).unwrap().unwrap();
    assert!(a.publish_projection_attempt(&old, &Default::default(), true).is_err());
    a.stage_reader(&new, &Default::default(), &Default::default()).unwrap();
    drop(a);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_with_private_pages"])
        .env("AH_TEST_READER_CRASH_DIR", &dir)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(59));
    let crashed = agent_history::archive::ProjectionAttempt {
        id: fs::read_to_string(t.path().join("attempt")).unwrap().parse().unwrap(),
        source_id: h.current_source.clone(),
        revision_id: info.id,
        parser_version: "crash".into(),
    };
    let a = Archive::open(&dir, b"").unwrap();
    assert_eq!(head(&a).publication, h.publication);
    assert_eq!(a.reader_window(&h, None, 0, 200, &Default::default()).unwrap().len(), 4);
    assert!(a.publish_projection_attempt(&new, &Default::default(), true).is_err());
    assert!(a.publish_projection_attempt(&crashed, &Default::default(), true).is_err());
}

#[test]
fn frame_extent_counts_decoded_bytes_across_record_boundaries() {
    let plain = b"{}\n".repeat(40);
    let a = zstd::stream::encode_all(&plain[..91], 1).unwrap();
    let b = zstd::stream::encode_all(&plain[91..], 1).unwrap();
    let native = [a, b].concat();
    let report = scan_records(
        &mut native.as_slice(),
        Harness::Codex,
        Transport::Zstd,
        &Default::default(),
        &Default::default(),
        &mut |_, _, _| Ok(()),
    )
    .unwrap();
    assert_eq!(report.max_frame_bytes, 91);
}

#[test]
fn scan_reports_verified_work_inside_the_first_storage_slice() {
    use std::sync::{Arc, Mutex};
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    fixtures::pi(&root, "/p", "p", &"x".repeat(700_000));
    capture(&a, Harness::Pi, &root);
    let h = head(&a);
    let id = a.latest_revision(&h.current_source).unwrap().unwrap().id;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let job = JobControl {
        progress: Some(Arc::new(move |p| {
            if p.phase == ReadPhase::VerifyingSlice {
                sink.lock().unwrap().push(p.done)
            }
        })),
        ..Default::default()
    };
    a.scan_revision(id, &Default::default(), &job, &mut |_, _, _| Ok(())).unwrap();
    assert!(
        seen.lock().unwrap().iter().any(|n| (1..=65536).contains(n)),
        "no verified block before the complete slice"
    );
}

#[test]
fn prefix_gaps_and_attachments_follow_the_exclusive_cut() {
    for inside in [false, true] {
        let t = tempfile::tempdir().unwrap();
        let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
        let root = t.path().join("codex");
        let thread = "33333333-3333-4333-8333-333333333333";
        let pointer = json!({"type":"event_msg","payload":{"type":"user_message","message":"image","local_images":["/outside/synthetic.png"]}});
        let mut prefix = fixtures::jsonl(&[native::codex_meta(thread, "/p"), pointer.clone()]);
        prefix.extend(if inside { b"oops\n".as_slice() } else { b"{}\n" });
        let mut bytes = prefix.clone();
        bytes.extend(fixtures::jsonl(&[pointer]));
        bytes.extend(b"oops\n");
        let path = native::codex_rollout(&root, "2026-11-10T12-00-00", thread, &[]);
        write(&path, &bytes);
        let mut meta = native::codex_meta(thread, "/p");
        meta["payload"]["history_base"] =
            json!({"thread_id":thread,"end_ordinal_exclusive":3,"end_byte_offset":prefix.len()});
        native::codex_rollout(
            &root,
            "2026-11-10T13-00-00",
            &format!("{thread}_44444444-4444-4444-8444-444444444444"),
            &[meta, native::codex_user_at("2026-11-10T14:00:00Z", "replacement")],
        );
        capture(&a, Harness::Codex, &root);
        let h = head(&a);
        assert_eq!((h.complete, h.physical_records, h.attachments_not_kept), (!inside, 5, 1));
    }
}

#[test]
fn cycle_stops_at_visited_source_in_the_published_line() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex");
    let a_id = "11111111-1111-4111-8111-111111111111";
    let b_id = "22222222-2222-4222-8222-222222222222";
    for (id, parent) in [(a_id, b_id), (b_id, a_id)] {
        let mut meta = native::codex_meta(id, "/p");
        meta["payload"]["history_base"] =
            json!({"thread_id":parent,"end_ordinal_exclusive":0,"end_byte_offset":0});
        native::codex_rollout(&root, "2026-11-10T12-00-00", id, &[meta, native::codex_user(id)]);
    }
    capture(&a, Harness::Codex, &root);
    for s in a.list_sessions(&Default::default()).unwrap().items {
        let h = a.reader_header(&s.id).unwrap();
        assert!(!h.complete);
        assert_eq!(h.physical_records, 2);
        assert!(
            a.reader_window(&h, None, 0, 10, &Default::default())
                .unwrap()
                .iter()
                .all(|r| r.source_id == h.current_source)
        );
    }
}

#[test]
fn offline_page_rebuild_preserves_native_projection_and_evidence() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("archive");
    let a = Archive::create_unencrypted(&dir).unwrap();
    let root = t.path().join("pi");
    fixtures::pi(&root, "/p", "p", "no live root");
    capture(&a, Harness::Pi, &root);
    let h = head(&a);
    let revision = a.latest_revision(&h.current_source).unwrap().unwrap();
    let parser = a.current_projection_id(&h.current_source).unwrap();
    let before = texts(a.reader_window(&h, None, 0, 200, &Default::default()).unwrap());
    fs::remove_dir_all(root).unwrap();
    assert_eq!(a.rebuild_reader_catalog(&Default::default()).unwrap(), 1);
    let new = head(&a);
    assert_eq!(h.publication, new.publication);
    assert_eq!(h.input_digest, new.input_digest);
    assert_eq!(a.current_projection_id(&h.current_source).unwrap(), parser);
    assert_eq!(a.latest_revision(&h.current_source).unwrap().unwrap().digest, revision.digest);
    assert_eq!(texts(a.reader_window(&new, None, 0, 200, &Default::default()).unwrap()), before);
}

#[test]
fn crash_with_private_pages() {
    let Some(dir) = std::env::var_os("AH_TEST_READER_CRASH_DIR") else { return };
    let a = Archive::open(Path::new(&dir), b"").unwrap();
    let h = head(&a);
    let info = a.latest_revision(&h.current_source).unwrap().unwrap();
    let attempt = a.prepare_projection(&h.current_source, info.id, "crash", true).unwrap().unwrap();
    a.stage_reader(&attempt, &Default::default(), &Default::default()).unwrap();
    fs::write(Path::new(&dir).parent().unwrap().join("attempt"), attempt.id.to_string()).unwrap();
    std::process::exit(59)
}

#[test]
fn sorted_lookup_pages_preserve_repeated_ids_and_raw_provenance() {
    use agent_history::{Artifact, ArtifactKind};
    use sha2::{Digest, Sha256};
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(tmp.path()).unwrap();
    let root = SourceRoot { harness: Harness::Pi, root: "/fixture".into() };
    let artifact = Artifact {
        kind: ArtifactKind::Transcript,
        path: "/fixture/p.jsonl".into(),
        relative_path: "p.jsonl".into(),
        project_hint: None,
        generation: None,
        current: true,
    };
    let mut bytes = b"{\"type\":\"session\",\"version\":3,\"id\":\"p\"}\n".to_vec();
    for ordinal in 1..=4300 {
        let id = if ordinal % 2 == 0 { "repeated".to_string() } else { format!("u-{ordinal}") };
        bytes.extend(
            format!("{{\"type\":\"custom\",\"id\":\"{id}\",\"parentId\":null}}\n").as_bytes(),
        );
    }
    let revision = archive.begin_revision("lookup", &root, &artifact).unwrap();
    archive.append_chunk(revision, 0, bytes.clone()).unwrap();
    archive
        .finish_revision(revision, &hex::encode(Sha256::digest(&bytes)), bytes.len() as u64)
        .unwrap();
    let attempt =
        archive.prepare_projection("lookup", revision, "lookup/1", true).unwrap().unwrap();
    archive
        .publish_projection_attempt(
            &attempt,
            &agent_history::SessionMetadata { native_id: "p".into(), ..Default::default() },
            true,
        )
        .unwrap();
    let h = head(&archive);
    assert_eq!(
        archive
            .reader_lookup(&h, "lookup", "repeated", Some(2048), 4, &Default::default())
            .unwrap(),
        [2050, 2052, 2054, 2056]
    );
    assert_eq!(
        archive.reader_lookup(&h, "lookup", "u-4299", None, 1, &Default::default()).unwrap(),
        [4299]
    );
    let outline = archive.reader_outline(&h, None, 4299, 2, &Default::default()).unwrap();
    assert_eq!(outline[0].1.id.as_deref(), Some("u-4299"));
    assert_eq!(outline[1].1.id.as_deref(), Some("repeated"));
    let exact =
        archive.reader_record_bytes(&h, "lookup", 4299, 0, 128, &Default::default()).unwrap();
    assert_eq!(exact.bytes, b"{\"type\":\"custom\",\"id\":\"u-4299\",\"parentId\":null}\n");
}

#[test]
fn late_parent_and_missing_source_update_membership() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    let child_path =
        native::claude_subagent(&root, "parent", "agent-one.jsonl", "one", "/p", "child");
    capture(&a, Harness::ClaudeCode, &root);
    let child = head(&a);
    native::claude_main(&root, "parent", &[native::claude_user("parent", "u", None, "/p", "main")]);
    capture(&a, Harness::ClaudeCode, &root);
    let h = a.reader_header(&child.conversation_id).unwrap();
    let parent = a.reader_sources(&h, None, 1).unwrap().remove(0).parent_id.unwrap();
    assert_eq!(
        a.reader_children(&a.reader_header(&parent).unwrap(), None, 2).unwrap(),
        std::slice::from_ref(&h.conversation_id)
    );
    fs::remove_file(child_path).unwrap();
    capture(&a, Harness::ClaudeCode, &root);
    let missing = a.reader_header(&child.conversation_id).unwrap();
    assert_ne!(missing.publication, h.publication);
    assert!(a.reader_sources(&missing, None, 1).unwrap()[0].observation.missing_since.is_some());
}

#[test]
fn visible_ranks_skip_non_display_records_before_the_requested_window() {
    let temp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let root = temp.path().join("pi");
    let mut rows = vec![json!({"id":"p","type":"session","version":3})];
    for n in 1..=550 {
        if n % 50 == 0 {
            rows.push(fixtures::pi_message(
                &format!("m-{n}"),
                None,
                "user",
                &format!("display {n}"),
            ))
        } else {
            rows.push(json!({"type":"custom","id":format!("c-{n}"),"parentId":null}))
        }
    }
    for n in 2..rows.len() {
        rows[n]["parentId"] = rows[n - 1]["id"].clone();
    }
    write(&root.join("sessions/p.jsonl"), &fixtures::jsonl(&rows));
    capture(&a, Harness::Pi, &root);
    let h = head(&a);
    assert_eq!((h.visible_records, h.physical_records), (11, 551));
    let page = a.reader_visible_window(&h, None, 8, 2, &Default::default()).unwrap();
    assert_eq!(texts(page), ["display 450", "display 500"]);
}

#[test]
fn base_from_another_root_refreshes_a_dependent_without_recapturing_it() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let older = t.path().join("base");
    let newer = t.path().join("continued");
    let base = "11111111-1111-4111-8111-111111111111";
    let child = "22222222-2222-4222-8222-222222222222";
    let prefix = [native::codex_meta(base, "/p"), native::codex_reply(base, "anchor", "inherited")];
    let mut meta = native::codex_meta(child, "/p");
    meta["payload"]["history_base"] = json!({"thread_id":base,"end_ordinal_exclusive":2,"end_byte_offset":fixtures::jsonl(&prefix).len()});
    native::codex_rollout(
        &newer,
        "2026-11-10T13-00-00",
        child,
        &[meta, native::codex_user("child")],
    );
    capture(&a, Harness::Codex, &newer);
    let old = head(&a);
    assert!(!old.complete);
    fs::remove_dir_all(newer).unwrap();
    let mut with_tail = prefix.to_vec();
    with_tail.push(native::codex_user("discarded"));
    native::codex_rollout(&older, "2026-11-10T12-00-00", base, &with_tail);
    capture(&a, Harness::Codex, &older);
    let h = a.reader_header(&old.conversation_id).unwrap();
    assert!(h.complete);
    assert_eq!(h.physical_records, 4);
    let row = a.reader_window(&h, None, 1, 1, &Default::default()).unwrap().remove(0);
    assert_eq!(row.parts[0].text, "inherited");
    assert_ne!(row.source_id, h.current_source);
    assert_eq!(
        a.reader_record_bytes(&h, &row.source_id, 1, 0, 4096, &Default::default()).unwrap().bytes,
        fixtures::jsonl(&prefix[1..])
    );
    assert!(a.reader_record_bytes(&h, &row.source_id, 2, 0, 4096, &Default::default()).is_err());
}

#[path = "reader_publication/review.rs"]
mod review;
