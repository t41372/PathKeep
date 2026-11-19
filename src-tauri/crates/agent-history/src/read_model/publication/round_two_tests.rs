//! Faults in one disposable reader publication are exercised through public reads and repair.
use super::*;

fn fixture(lines: &str) -> (tempfile::TempDir, Archive, ReaderPublication) {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    let project = root.join("projects/p");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("a.jsonl"), lines).unwrap();
    a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &Default::default()).unwrap();
    let h = a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
    (t, a, h)
}
const MESSAGE: &str =
    "{\"type\":\"user\",\"uuid\":\"a\",\"message\":{\"role\":\"user\",\"content\":\"saved\"}}\n";

#[test]
fn lost_version_and_line_are_rebuilt() {
    for table in ["ah_reader_version", "ah_reader_line"] {
        let (_t, a, h) = fixture(MESSAGE);
        a.store
            .write(DB, Priority::Batch, move |tx| tx.execute(&format!("DELETE FROM {table}"), ()))
            .unwrap();
        a.rebuild_reader_catalog(&Default::default()).unwrap();
        let repaired = a.reader_header(&h.conversation_id).unwrap();
        assert_eq!(
            a.reader_window(&repaired, None, 0, 1, &Default::default()).unwrap()[0].parts[0].text,
            "saved",
            "{table}"
        );
    }
}
#[test]
fn broken_rank_parks_only_its_conversation_and_repair_unparks() {
    let (t, a, h) = fixture(MESSAGE);
    let root = t.path().join("claude");
    fs::write(root.join("projects/p/b.jsonl"), MESSAGE.replace("saved", "other")).unwrap();
    a.backup(
        &[SourceRoot { harness: Harness::ClaudeCode, root: root.clone() }],
        &Default::default(),
    )
    .unwrap();
    let other = a
        .list_sessions(&Default::default())
        .unwrap()
        .items
        .into_iter()
        .find(|i| i.id != h.conversation_id)
        .unwrap()
        .id;
    let source = h.current_source.clone();
    a.store.write(DB,Priority::Batch,move|tx|{
        tx.execute("DELETE FROM ah_reader_page WHERE family=?1 AND owner IN (SELECT token FROM ah_reader_version WHERE source=?2)",params![RANK,source])?;
        Ok(())
    }).unwrap();
    a.set_coverage(&h.current_source, false).unwrap();
    let opts = crate::capture::BackupOptions::default();
    opts.cancel.cancel();
    let error = a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &opts).unwrap_err();
    assert_eq!(error.code(), "reader_rebuild_required", "flush fault was masked");
    drop(a);
    let a = Archive::open(&t.path().join("archive"), b"").unwrap();
    assert!(matches!(a.reader_header(&h.conversation_id), Err(Error::ReaderRebuildRequired)));
    let good = a.reader_header(&other).unwrap();
    assert_eq!(
        a.reader_window(&good, None, 0, 1, &Default::default()).unwrap()[0].parts[0].text,
        "other"
    );
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    assert_eq!(
        a.reader_visible_window(
            &a.reader_header(&h.conversation_id).unwrap(),
            None,
            0,
            1,
            &Default::default()
        )
        .unwrap()[0]
            .parts[0]
            .text,
        "saved"
    );
}
#[test]
fn coverage_ticket_waits_for_publication_snapshot() {
    use std::{sync::mpsc, time::Duration};
    let (_t, a, h) = fixture(MESSAGE);
    let gate = a.publication_gate.lock().unwrap();
    std::thread::scope(|scope| {
        let (done, rx) = mpsc::channel();
        let (ready, started) = mpsc::channel();
        let a = &a;
        scope.spawn(move || {
            ready.send(()).unwrap();
            a.set_coverage(&h.current_source, false).unwrap();
            done.send(()).unwrap();
        });
        started.recv().unwrap();
        let slipped = rx.recv_timeout(Duration::from_millis(100)).is_ok();
        drop(gate);
        if !slipped {
            rx.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        assert!(!slipped, "coverage invalidated inside publication enumeration");
    });
}
#[test]
fn shortened_posting_extent_cannot_hide_final_id() {
    let ids = (0..257).map(|i| format!("id-{i}")).collect::<Vec<_>>();
    let lines =
        ids.iter().map(|id| MESSAGE.replace("\"a\"", &format!("\"{id}\""))).collect::<String>();
    let (_t, a, h) = fixture(&lines);
    let id = ids.iter().max_by_key(|id| build::key(id)).unwrap();
    assert_eq!(
        a.reader_lookup(&h, &h.current_source, id, None, 8, &Default::default()).unwrap().len(),
        1
    );
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_version SET postings=256", ())
        })
        .unwrap();
    assert!(matches!(
        a.reader_lookup(&h, &h.current_source, id, None, 8, &Default::default()),
        Err(Error::ReaderRebuildRequired)
    ));
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    let h = a.reader_header(&h.conversation_id).unwrap();
    assert_eq!(
        a.reader_lookup(&h, &h.current_source, id, None, 8, &Default::default()).unwrap().len(),
        1
    );
}
#[test]
fn hidden_rank_gap_has_bounded_page_reads() {
    let (_t, a, h) = fixture(&format!("{MESSAGE}{}{MESSAGE}", "{}\n".repeat(32768)));
    a.store.capture_query_plans(DB, true).unwrap();
    let rows = a.reader_visible_window(&h, None, 0, 2, &Default::default()).unwrap();
    assert_eq!(rows.iter().map(|r| r.ordinal.0).collect::<Vec<_>>(), [0, 32769]);
    let loads = a
        .store
        .take_query_plans(DB)
        .unwrap()
        .iter()
        .filter(|p| p.sql.contains("SELECT first_rank FROM ah_reader_page"))
        .count();
    assert!(loads < 32, "two rows fetched {loads} pages");
}
thread_local! {pub(in crate::read_model::publication) static CHECK_GATE:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
#[test]
fn immutable_comparison_releases_publication_gate() {
    let (_t, a, _h) = fixture(MESSAGE);
    CHECK_GATE.set(true);
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    assert!(!CHECK_GATE.get());
}

#[test]
fn altered_serving_rows_are_local_and_repairable() {
    for sql in [
        "UPDATE ah_reader_member SET records=0",
        "UPDATE ah_reader_head SET parent='nonexistent'",
        "UPDATE ah_reader_line SET records=0",
        "UPDATE ah_reader_member SET descriptor='bad json'",
        "DELETE FROM ah_reader_line",
    ] {
        let (_t, a, h) = fixture(MESSAGE);
        a.store.write(DB, Priority::Batch, move |tx| tx.execute(sql, ())).unwrap();
        let read = || {
            let current = a.reader_header(&h.conversation_id)?;
            let rows = a.reader_window(&current, None, 0, 8, &Default::default())?;
            Ok::<_, Error>((current.physical_records, current.parent_id, rows.len()))
        };
        assert!(matches!(read(), Err(Error::ReaderRebuildRequired) | Ok((1, None, 1))), "{sql}");
        a.rebuild_reader_catalog(&Default::default()).unwrap();
        assert_eq!(read().unwrap(), (1, None, 1), "{sql}");
    }
}

#[test]
fn open_finishes_healthy_tickets_and_keeps_a_bad_one_repairable() {
    let (t, a, h) = fixture(MESSAGE);
    let root = t.path().join("claude");
    fs::write(root.join("projects/p/healthy.jsonl"), MESSAGE.replace("saved", "healthy")).unwrap();
    a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &Default::default()).unwrap();
    let healthy = a
        .list_sessions(&Default::default())
        .unwrap()
        .items
        .into_iter()
        .find(|s| s.id != h.conversation_id)
        .unwrap();
    let good = a.reader_header(&healthy.id).unwrap();
    a.set_coverage(&good.current_source, false).unwrap();
    a.set_coverage(&h.current_source, false).unwrap();
    let source = h.current_source;
    a.store
        .write(DB, Priority::Batch, move |tx| {
            tx.execute(
                "DELETE FROM ah_reader_page WHERE family=?1 AND owner IN
        (SELECT token FROM ah_reader_version WHERE source=?2)",
                params![RANK, source],
            )
        })
        .unwrap();
    drop(a);
    let a = Archive::open(&t.path().join("archive"), b"").unwrap();
    assert!(!a.reader_header(&healthy.id).unwrap().complete);
    assert!(matches!(a.reader_header(&h.conversation_id), Err(Error::ReaderRebuildRequired)));
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    let restored = a.reader_header(&h.conversation_id).unwrap();
    assert!(!restored.complete);
    assert_eq!(
        a.reader_visible_window(&restored, None, 0, 1, &Default::default()).unwrap()[0].parts[0]
            .text,
        "saved"
    );
}

#[test]
fn lost_child_head_does_not_silently_hide_the_relationship() {
    let (t, a, parent) = fixture(&MESSAGE.replace("\"uuid\"", "\"sessionId\":\"a\",\"uuid\""));
    let root = t.path().join("claude");
    let dir = root.join("projects/p/a/subagents");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("agent-child.jsonl"),concat!(r#"{"type":"user","uuid":"c","sessionId":"a","agentId":"child","isSidechain":true,"message":{"role":"user","content":"child"}}"#, "\n")).unwrap();
    a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &Default::default()).unwrap();
    let h = a.reader_header(&parent.conversation_id).unwrap();
    let children = a.reader_children(&h, None, 5).unwrap();
    assert_eq!(children.len(), 1);
    let child = children[0].clone();
    a.store
        .write(DB, Priority::Batch, move |tx| {
            tx.execute("DELETE FROM ah_reader_head WHERE conversation=?1", [child])
        })
        .unwrap();
    assert!(matches!(a.reader_children(&h, None, 5), Err(Error::ReaderRebuildRequired)));
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    let h = a.reader_header(&parent.conversation_id).unwrap();
    assert_eq!(a.reader_children(&h, None, 5).unwrap(), children);
}

#[test]
fn damaged_version_metadata_cannot_block_its_replacement() {
    for sql in [
        "UPDATE ah_reader_version SET repair_generation=9223372036854775807",
        "UPDATE ah_reader_version SET attempt=attempt+5",
        "UPDATE ah_reader_version SET repair_generation=x'00ff'",
    ] {
        let (_t, a, h) = fixture(MESSAGE);
        a.store.write(DB, Priority::Batch, move |tx| tx.execute(sql, ())).unwrap();
        assert!(matches!(a.reader_header(&h.conversation_id), Err(Error::ReaderRebuildRequired)));
        a.rebuild_reader_catalog(&Default::default()).unwrap();
        let h = a.reader_header(&h.conversation_id).unwrap();
        assert_eq!(
            a.reader_visible_window(&h, None, 0, 1, &Default::default()).unwrap()[0].parts[0].text,
            "saved"
        );
    }
}

#[test]
fn owner_revocation_interrupts_exit_flush_then_resume_labels_coverage() {
    use std::{sync::mpsc, time::Duration};
    let (t, a, h) = fixture(MESSAGE);
    let a = Arc::new(a);
    a.set_coverage(&h.current_source, false).unwrap();
    let gate = a.publication_gate.lock().unwrap();
    std::thread::scope(|scope| {
        let backup = a.clone();
        let root = t.path().join("claude");
        let (start, started) = mpsc::channel();
        let (end, ended) = mpsc::channel();
        scope.spawn(move || {
            let opts = crate::capture::BackupOptions::default();
            opts.cancel.cancel();
            start.send(()).unwrap();
            end.send(backup.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &opts))
                .unwrap();
        });
        started.recv().unwrap();
        assert!(ended.recv_timeout(Duration::from_millis(100)).is_err());
        let owner = a.clone();
        let (paused, ready) = mpsc::channel();
        scope.spawn(move || paused.send(owner.pause_jobs()).unwrap());
        let guard = ready.recv_timeout(Duration::from_secs(1));
        drop(gate);
        assert!(guard.is_ok(), "owner could not revoke the exit finalizer");
        assert!(matches!(ended.recv_timeout(Duration::from_secs(2)), Ok(Err(Error::Cancelled))));
        drop(guard);
    });
    let current = a.reader_header(&h.conversation_id).unwrap();
    assert!(!current.complete);
    assert_eq!(
        a.reader_window(&current, None, 0, 1, &Default::default()).unwrap()[0].parts[0].text,
        "saved"
    );
}

#[test]
fn reader_damage_advice_preserves_real_evidence_integrity_errors() {
    let (_t, a, h) = fixture(MESSAGE);
    a.store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_chunks SET data=x'00'", ())
        })
        .unwrap();
    assert!(matches!(
        a.reader_window(&h, None, 0, 1, &Default::default()),
        Err(Error::Integrity(_))
    ));
}

#[test]
fn a_missing_dependency_row_cannot_hide_a_late_base() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let base = "11111111-1111-4111-8111-111111111111";
    let child = "22222222-2222-4222-8222-222222222222";
    let header = |id| serde_json::json!({"type":"session_meta","payload":{"id":id,"cwd":"/p"}});
    let base_row = format!("{}\n", header(base));
    let mut continuation = header(child);
    continuation["payload"]["history_base"] = serde_json::json!({"thread_id":base,
        "end_ordinal_exclusive":1,"end_byte_offset":base_row.len()});
    let capture = |id, bytes| {
        let root = t.path().join(id);
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).unwrap();
        fs::write(sessions.join(format!("rollout-2026-11-10T12-00-00-{id}.jsonl")), bytes).unwrap();
        a.backup(&[SourceRoot { harness: Harness::Codex, root }], &Default::default()).unwrap();
    };
    capture(child, format!("{continuation}\n"));
    let old = a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
    assert!(!old.complete);
    let owner = old.publication.0.clone();
    let removed = a
        .store
        .write(DB, Priority::Batch, move |tx| {
            tx.execute(
                "DELETE FROM ah_reader_dependency WHERE publication=?1 AND target=?2",
                params![owner, catalog::rollout_key("codex", base)],
            )
        })
        .unwrap();
    assert_eq!(removed, 1);
    capture(base, base_row);
    let current = a.reader_header(&old.conversation_id).unwrap();
    assert!(current.complete, "base arrived but its dependent still reports a gap");
    assert_eq!(current.physical_records, 2);
}
