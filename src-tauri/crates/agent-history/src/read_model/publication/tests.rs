//! Storage faults are introduced in the disposable R role. Assertions use public reads.
use super::*;
use crate::{Archive, Harness, SourceRoot};
use std::fs;
use vault_store::{Priority, params};

fn large_frame() -> (tempfile::TempDir, Archive, ReaderPublication) {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let root = temp.path().join("codex");
    fs::create_dir_all(root.join("sessions")).unwrap();
    let header = br#"{"type":"session_meta","payload":{"id":"large"}}"#;
    let mut bytes = header.to_vec();
    bytes.push(b'\n');
    for _ in 0..4 {
        bytes.resize(bytes.len() + 8 * 1024 * 1024, b' ');
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(br#"{"type":"response_item","payload":{"type":"message","role":"user","id":"tail","content":"exact tail"}}"#);
    let file = root.join("sessions/rollout.jsonl.zst");
    fs::write(&file, zstd::stream::encode_all(bytes.as_slice(), 1).unwrap()).unwrap();
    archive.backup(&[SourceRoot { harness: Harness::Codex, root }], &Default::default()).unwrap();
    let session = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    let head = archive.reader_header(&session.id).unwrap();
    (temp, archive, head)
}
#[test]
fn long_frame_cache_is_selective_and_missing_or_stale_text_reads_evidence() {
    let (_temp, a, h) = large_frame();
    assert_eq!(h.physical_records, 6);
    let member = a.reader_sources(&h, None, 1).unwrap().remove(0);
    assert!(member.rendered);
    let small = JobControl { max_decoded_bytes: 16, ..Default::default() };
    assert_eq!(a.reader_window(&h, None, 5, 1, &small).unwrap()[0].parts[0].text, "exact tail");
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_version SET renderer=0", ())?;
            Ok(())
        })
        .unwrap();
    assert_eq!(a.reader_window(&h, None, 5, 1, &small).unwrap_err().code(), "limit_exceeded");
    assert_eq!(
        a.reader_window(&h, None, 5, 1, &Default::default()).unwrap()[0].parts[0].text,
        "exact tail"
    );
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_version SET renderer=?1", [RENDERER])?;
            tx.execute(
                "UPDATE ah_reader_page SET binding=zeroblob(32) WHERE family=?1",
                [RENDERED],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(a.reader_window(&h, None, 5, 1, &small).unwrap_err().code(), "limit_exceeded");
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("DELETE FROM ah_reader_page WHERE family=?1", [RENDERED])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        a.reader_window(&h, None, 5, 1, &Default::default()).unwrap()[0].parts[0].text,
        "exact tail"
    );
    // A normal backup of unchanged bytes must publish the new renderer, even
    // when the old native-event projection already has a complete parse report.
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_version SET renderer=0", ())?;
            Ok(())
        })
        .unwrap();
    a.backup(
        &[SourceRoot { harness: Harness::Codex, root: member.root.into() }],
        &Default::default(),
    )
    .unwrap();
    let updated = a.reader_header(&h.conversation_id).unwrap();
    assert_ne!(updated.publication, h.publication);
    assert_eq!(
        a.reader_window(&updated, None, 5, 1, &small).unwrap()[0].parts[0].text,
        "exact tail"
    );
}

#[test]
fn replacement_collects_private_pages_and_keeps_shared_rendered_blobs() {
    let (_temp, a, h) = large_frame();
    let old = h.publication.0.clone();
    // A decodable stale payload under the original hash must fall back, and an
    // offline rebuild must repair the shared blob even though that hash already exists.
    a.store
        .write(DB, Priority::Batch, |tx| {
            let (hash, data) = tx.query_one(
                "SELECT b.hash,b.data FROM ah_reader_blob b JOIN ah_reader_page p ON b.hash=p.hash
             WHERE p.family=?1 AND p.first_rank=0 AND p.part=0",
                [RENDERED],
                |r| Ok((r.get::<Vec<u8>>(0)?, r.get::<Vec<u8>>(1)?)),
            )?;
            let mut decoded = zstd::stream::decode_all(data.as_slice()).unwrap();
            let pos = decoded.windows(10).position(|w| w == b"exact tail").unwrap();
            decoded[pos..pos + 10].copy_from_slice(b"wrong tail");
            let changed = zstd::stream::encode_all(decoded.as_slice(), 3).unwrap();
            tx.execute("UPDATE ah_reader_blob SET data=?2 WHERE hash=?1", params![hash, changed])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        a.reader_window(&h, None, 5, 1, &Default::default()).unwrap()[0].parts[0].text,
        "exact tail"
    );
    damage_lookup_binding(&a);
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    while a.prune_reader_pages(2).unwrap() != 0 {}
    a.store
        .read(DB, move |tx| {
            assert_eq!(
                tx.query_one(
                    "SELECT count(*) FROM ah_reader_member WHERE publication=?1",
                    [old],
                    |r| Ok(r.get::<i64>(0)?)
                )?,
                0
            );
            assert_eq!(
                tx.query_one(
                    "SELECT count(*) FROM ah_reader_version WHERE state!='published'",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                )?,
                0
            );
            assert_eq!(
                tx.query_one(
                    "SELECT count(*) FROM ah_reader_blob b WHERE NOT EXISTS(SELECT 1 FROM
                 ah_reader_page p WHERE p.hash=b.hash)",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                )?,
                0
            );
            Ok(())
        })
        .unwrap();
    let new = a.reader_header(&h.conversation_id).unwrap();
    assert_ne!(new.publication, h.publication);
    assert_eq!(
        a.reader_window(
            &new,
            None,
            5,
            1,
            &JobControl { max_decoded_bytes: 1, ..Default::default() }
        )
        .unwrap()[0]
            .native_id
            .as_deref(),
        Some("tail")
    );
}

#[test]
fn equal_revision_handles_in_a_foreign_catalog_never_authorize_a_header() {
    let (_temp, a, h) = large_frame();
    a.store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_revisions SET digest=?1", params!["0".repeat(64)])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(a.reader_header(&h.conversation_id).unwrap_err().code(), "stale_cursor");
}

#[test]
fn only_a_frame_strictly_above_the_limit_selects_rendering() {
    use crate::{Artifact, ArtifactKind, SessionMetadata};
    use sha2::{Digest, Sha256};
    for extra in [0, 1] {
        let temp = tempfile::tempdir().unwrap();
        let a = Archive::create_unencrypted(temp.path()).unwrap();
        let root = SourceRoot { harness: Harness::Codex, root: "/test".into() };
        let path = "sessions/frames.jsonl.zstd";
        let artifact = Artifact {
            path: root.root.join(path),
            relative_path: path.into(),
            current: true,
            kind: ArtifactKind::Transcript,
            project_hint: None,
            generation: None,
        };
        let mut native = zstd::stream::encode_all(b"{}\n".as_slice(), 1).unwrap();
        let mut frame = vec![b' '; REPLAY_LIMIT as usize + extra];
        for k in (1024 * 1024 - 1..frame.len()).step_by(1024 * 1024) {
            frame[k] = b'\n'
        }
        let first_native = native.len() as u64;
        native.extend(zstd::stream::encode_all(frame.as_slice(), 1).unwrap());
        let second_native = native.len() as u64;
        native.extend(zstd::stream::encode_all(b"\n{}\n".as_slice(), 1).unwrap());
        let id = a.begin_revision("frames", &root, &artifact).unwrap();
        a.append_chunk(id, 0, native.clone()).unwrap();
        a.finish_revision(id, &hex::encode(Sha256::digest(&native)), native.len() as u64).unwrap();
        let attempt =
            a.prepare_projection("frames", id, "frame-boundaries", true).unwrap().unwrap();
        a.publish_projection_attempt(
            &attempt,
            &SessionMetadata { native_id: "frames".into(), ..Default::default() },
            true,
        )
        .unwrap();
        let h =
            a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
        let member = a.reader_sources(&h, None, 1).unwrap().pop().unwrap();
        assert_eq!(member.max_frame_bytes, REPLAY_LIMIT + extra as u64);
        assert_eq!(member.rendered, extra != 0);
        let stored = a
            .store
            .read(DB, |tx| {
                tx.query_one(
                    "SELECT count(*) FROM ah_reader_page WHERE family=?1",
                    [RENDERED],
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap();
        assert_eq!(stored > 0, extra != 0, "persisted rendered-page policy");
        let owner = a
            .store
            .read(DB, |tx| {
                tx.query_one(
                    "SELECT token FROM ah_reader_version WHERE state='published'",
                    (),
                    |r| Ok(r.get::<String>(0)?),
                )
            })
            .unwrap();
        let (_, frames) =
            pages::load::<TransportFrame>(&a, &owner, FRAMES, 1, &Default::default()).unwrap();
        let decoded_end = 3 + frame.len() as u64;
        assert_eq!(
            frames
                .iter()
                .map(|f| (f.start.native.0, f.start.decoded.0, f.native_end.0, f.decoded_end.0))
                .collect::<Vec<_>>(),
            [
                (0, 0, first_native, 3),
                (first_native, 3, second_native, decoded_end),
                (second_native, decoded_end, native.len() as u64, decoded_end + 4)
            ]
        );
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[1].decoded_end.0 - frames[1].start.decoded.0, member.max_frame_bytes);
    }
}

// Models a newer dependency ticket arriving after private member/line batches and
// immediately before the CAS. Reopening also exercises recovery of that exact cut.
thread_local! {static SUPERSEDE:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
pub(super) fn before_head_switch(a: &Archive) -> crate::Result<()> {
    if SUPERSEDE.with(|flag| flag.replace(false)) {
        a.store.write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_dirty SET ticket=lower(hex(randomblob(16)))", ())?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn stale_membership_ticket_leaves_old_head_and_recovers_dirty_work() {
    let (temp, a, old) = large_frame();
    damage_lookup_binding(&a);
    SUPERSEDE.with(|flag| flag.set(true));
    assert!(a.rebuild_reader_catalog(&Default::default()).is_err());
    assert_eq!(a.reader_header(&old.conversation_id).unwrap().publication, old.publication);
    assert_eq!(
        a.reader_window(&old, None, 5, 1, &Default::default()).unwrap()[0].parts[0].text,
        "exact tail"
    );
    drop(a);
    let a = Archive::open(&temp.path().join("archive"), b"").unwrap();
    let current = a.reader_header(&old.conversation_id).unwrap();
    assert_ne!(current.publication, old.publication);
    assert_eq!(
        a.reader_window(&current, None, 5, 1, &Default::default()).unwrap()[0].parts[0].text,
        "exact tail"
    );
}

#[test]
fn development_catalog_without_reader_tables_data_rebuilds_offline() {
    let (t, a, h) = large_frame();
    let original_digest = h.input_digest.clone();
    fs::remove_dir_all(t.path().join("codex")).unwrap();
    a.store
        .write(DB, Priority::Batch, |tx| {
            let tables = tx.query(
                "SELECT name FROM sqlite_schema WHERE type='table' AND name LIKE 'ah_reader_%'",
                (),
                |r| Ok(r.get::<String>(0)?),
            )?;
            for table in tables {
                tx.execute(&format!("DELETE FROM {table}"), ())?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(a.reader_header(&h.conversation_id).unwrap_err().code(), "reader_rebuild_required");
    assert_eq!(a.rebuild_reader_catalog(&Default::default()).unwrap(), 1);
    let new = a.reader_header(&h.conversation_id).unwrap();
    assert_eq!(new.input_digest, original_digest);
    assert_eq!(
        a.reader_window(
            &new,
            None,
            5,
            1,
            &JobControl { max_decoded_bytes: 1, ..Default::default() }
        )
        .unwrap()[0]
            .parts[0]
            .text,
        "exact tail"
    );
}

#[test]
fn obsolete_physical_counts_require_a_rebuilt_publication() {
    let (_temp, a, old) = large_frame();
    a.store
        .write(DB, Priority::Batch, |tx| tx.execute("UPDATE ah_reader_version SET codec=1", ()))
        .unwrap();
    assert!(matches!(a.reader_header(&old.conversation_id), Err(Error::ReaderRebuildRequired)));
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    let current = a.reader_header(&old.conversation_id).unwrap();
    assert_ne!(current.publication, old.publication);
    assert_eq!(
        a.reader_window(&current, None, 5, 1, &Default::default()).unwrap()[0].parts[0].text,
        "exact tail"
    );
}

#[test]
fn lookup_hole_is_not_end_of_postings() {
    let (_t, a, h) = large_frame();
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("DELETE FROM ah_reader_page WHERE family=?1", [LOOKUP])
        })
        .unwrap();
    assert!(matches!(
        a.reader_lookup(&h, &h.current_source, "large", None, 1, &Default::default()),
        Err(Error::ReaderRebuildRequired)
    ));
}
#[test]
fn redundant_ticket_preserves_publication() {
    let (_t, a, h) = large_frame();
    let source = h.current_source.clone();
    a.store.write(DB, Priority::Batch, move |tx| refresh(tx, &source, None)).unwrap();
    let sequence = || {
        a.store.read(DB,|tx|tx.query_one("SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name='ah_reader_retired'),0)",(),|r|Ok(r.get::<i64>(0)?))).unwrap()
    };
    let before = sequence();
    a.flush_reader_heads().unwrap();
    assert_eq!(sequence(), before, "a redundant ticket staged a private candidate");
    assert_eq!(a.reader_header(&h.conversation_id).unwrap().publication, h.publication);
}
#[test]
fn open_drains_more_than_one_retired_publication() {
    let (t, a, _) = large_frame();
    a.store
        .write(DB, Priority::Batch, |tx| {
            for i in 0..10 {
                tx.execute(
                    "INSERT INTO ah_reader_retired(publication) VALUES(?1)",
                    [format!("obsolete-{i}")],
                )?;
            }
            Ok(())
        })
        .unwrap();
    drop(a);
    let a = Archive::open(&t.path().join("archive"), b"").unwrap();
    assert_eq!(
        a.store
            .read(DB, |tx| tx
                .query_one("SELECT count(*) FROM ah_reader_retired", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        0
    );
}
#[test]
fn admitted_reader_keeps_superseded_members_until_release() {
    let (_t, a, h) = large_frame();
    let guard = a.jobs.pin_publication().unwrap();
    damage_lookup_binding(&a);
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    while a.prune_reader_pages(256).unwrap() > 0 {}
    let count = || {
        let token = h.publication.0.clone();
        a.store
            .read(DB, move |tx| {
                tx.query_one(
                    "SELECT count(*) FROM ah_reader_member WHERE publication=?1",
                    [token],
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap()
    };
    assert_eq!(count(), 1);
    drop(guard);
    while a.prune_reader_pages(256).unwrap() > 0 {}
    assert_eq!(count(), 0);
}
#[test]
fn ready_version_publishes_while_scan_slot_is_busy() {
    let (_t, a, h) = large_frame();
    let rev = a.latest_revision(&h.current_source).unwrap().unwrap();
    let attempt = a.prepare_projection(&h.current_source, rev.id, "ready", true).unwrap().unwrap();
    a.stage_reader(&attempt, &Default::default(), &Default::default()).unwrap();
    let busy = a.jobs.enter_scan(&Default::default()).unwrap();
    std::thread::scope(|scope| {
        let (s, r) = std::sync::mpsc::channel();
        scope.spawn(move || {
            s.send(a.publish_projection_attempt(
                &attempt,
                &crate::SessionMetadata { native_id: "large".into(), ..Default::default() },
                true,
            ))
            .unwrap()
        });
        let result = r.recv_timeout(std::time::Duration::from_secs(1));
        drop(busy);
        result.unwrap().unwrap();
    });
}
#[test]
fn missing_published_version_remains_an_expected_member() {
    let (temp, a, h) = large_frame();
    let copy = temp.path().join("copy");
    fs::create_dir_all(copy.join("sessions")).unwrap();
    fs::copy(
        temp.path().join("codex/sessions/rollout.jsonl.zst"),
        copy.join("sessions/rollout.jsonl.zst"),
    )
    .unwrap();
    a.backup(&[SourceRoot { harness: Harness::Codex, root: copy }], &Default::default()).unwrap();
    let h = a.reader_header(&h.conversation_id).unwrap();
    let others = a.reader_sources(&h, None, 10).unwrap();
    let other = others.iter().find(|m| m.source_id != h.current_source).unwrap().source_id.clone();
    a.store
        .write(DB, Priority::Batch, move |tx| {
            tx.execute(
                "UPDATE ah_reader_version SET state='retired' WHERE source=?1",
                [other.as_str()],
            )?;
            refresh(tx, &other, None)
        })
        .unwrap();
    a.flush_reader_heads().unwrap();
    let head = a.reader_header(&h.conversation_id).unwrap();
    assert_eq!(a.reader_sources(&head, None, 10).unwrap().len(), 2);
    assert!(!head.complete);
    assert_eq!(head.coverage_reason.as_deref(), Some("awaiting_rebuild"));
}
#[test]
fn expiry_uses_source_index() {
    let (_t, a, h) = large_frame();
    a.store.capture_query_plans(DB, true).unwrap();
    let src = h.current_source;
    a.store.read(DB,move|tx|tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_version WHERE source=?1 AND state<>'retired')",[src],|r|Ok(r.get::<i64>(0)?))).unwrap();
    let plans = a.store.take_query_plans(DB).unwrap();
    assert!(
        plans.iter().any(|p| p.steps.iter().any(|s| s.contains("ah_reader_version_source"))),
        "{plans:?}"
    );
}

#[test]
fn healthy_offline_rebuild_keeps_the_same_serving_head() {
    let (_t, a, h) = large_frame();
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    assert_eq!(a.reader_header(&h.conversation_id).unwrap().publication, h.publication);
}
#[test]
fn missing_rank_block_requires_rebuild() {
    let (_t, a, h) = large_frame();
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("DELETE FROM ah_reader_page WHERE family=?1", [RANK])
        })
        .unwrap();
    assert!(matches!(
        a.reader_visible_window(&h, None, 0, 10, &Default::default()),
        Err(Error::ReaderRebuildRequired)
    ));
}

fn damage_lookup_binding(a: &Archive) {
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute(
                "UPDATE ah_reader_page SET binding=zeroblob(32) WHERE family=?1",
                [DIRECTORY],
            )
        })
        .unwrap();
}

#[test]
fn cached_leaf_seeks_by_rank_and_rejects_redirected_run() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    fs::create_dir_all(root.join("sessions")).unwrap();
    let mut data = String::from("{\"type\":\"session\",\"id\":\"p\",\"version\":3}\n");
    for n in 1..=80 {
        let parent = if n <= 2 { serde_json::Value::Null } else { format!("m{}", n - 2).into() };
        data += &format!(
            "{}\n",
            serde_json::json!({"type":"message","id":format!("m{n}"),"parentId":parent,"message":{"role":"user","content":"text"}})
        );
    }
    fs::write(root.join("sessions/p.jsonl"), data).unwrap();
    a.backup(&[SourceRoot { harness: Harness::Pi, root }], &Default::default()).unwrap();
    let h = a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
    let read = || a.reader_leaf_window(&h, &h.current_source, 79, 1, 1, &Default::default());
    assert_eq!(read().unwrap()[0].ordinal.0, 1);
    a.store.capture_query_plans(DB, true).unwrap();
    assert_eq!(read().unwrap()[0].ordinal.0, 1);
    assert!(
        !a.store
            .take_query_plans(DB)
            .unwrap()
            .iter()
            .any(|p| p.sql.contains("SELECT lookup_pages"))
    );
    a.store
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_walk_run SET first_ordinal=2 WHERE first_ordinal=1", ())
        })
        .unwrap();
    assert!(matches!(read(), Err(Error::ReaderRebuildRequired)));
}

#[test]
fn lost_pi_selection_cannot_become_a_physical_line() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    fs::create_dir_all(root.join("sessions")).unwrap();
    fs::write(root.join("sessions/p.jsonl"),"{\"type\":\"session\",\"id\":\"p\",\"version\":3}\n{\"type\":\"message\",\"id\":\"a\",\"parentId\":null,\"message\":{\"role\":\"user\",\"content\":\"root\"}}\n").unwrap();
    a.backup(&[SourceRoot { harness: Harness::Pi, root }], &Default::default()).unwrap();
    let h = a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
    a.store.write(DB, Priority::Batch, |tx| tx.execute("DELETE FROM ah_reader_leaf", ())).unwrap();
    assert!(matches!(
        a.reader_visible_window(&h, None, 0, 1, &Default::default()),
        Err(Error::ReaderRebuildRequired)
    ));
}

#[test]
fn navigation_uses_callers_cancel_while_both_slots_are_held() {
    use std::{sync::mpsc, time::Duration};
    let (_t, a, h) = large_frame();
    for lookup in [true, false] {
        let one = a.jobs.enter_window(&Default::default()).unwrap();
        let two = a.jobs.enter_window(&Default::default()).unwrap();
        let job = JobControl::default();
        let token = job.cancel.clone();
        let (started, ready) = mpsc::channel();
        let (done, answer) = mpsc::channel();
        std::thread::scope(|scope| {
            let (a, h) = (&a, &h);
            scope.spawn(move || {
                started.send(()).unwrap();
                let result = if lookup {
                    a.reader_lookup(h, &h.current_source, "tail", None, 1, &job).map(drop)
                } else {
                    a.reader_leaves(h, &h.current_source, 0, 1, &job).map(drop)
                };
                done.send(result).unwrap();
            });
            ready.recv().unwrap();
            token.cancel();
            let result = answer.recv_timeout(Duration::from_secs(1));
            drop((one, two));
            assert!(matches!(result, Ok(Err(Error::Cancelled))));
        });
    }
}
#[test]
fn immutable_lookup_error_rechecks_the_admitted_head() {
    let (_t, a, h) = large_frame();
    damage_lookup_binding(&a);
    let a = Arc::new(a);
    let publisher = a.clone();
    let once = std::sync::atomic::AtomicBool::new(false);
    let job = JobControl {
        progress: Some(Arc::new(move |_| {
            if !once.swap(true, std::sync::atomic::Ordering::SeqCst) {
                publisher.rebuild_reader_catalog(&Default::default()).unwrap();
            }
        })),
        ..Default::default()
    };
    assert!(matches!(a.reader_window(&h, Some("missing"), 0, 1, &job), Err(Error::StaleCursor)));
}

#[test]
fn owner_idle_collects_after_last_publication_pin() {
    use std::time::{Duration, Instant};
    let (_t, a, h) = large_frame();
    let a = Arc::new(a);
    a.enable_idle_reader_collection();
    let lease = a.jobs.enter(None).unwrap();
    let pin = a.jobs.pin_publication().unwrap();
    damage_lookup_binding(&a);
    a.rebuild_reader_catalog(&Default::default()).unwrap();
    let retained = || {
        let old = h.publication.0.clone();
        a.store
            .read(DB, move |tx| {
                tx.query_one(
                    "SELECT EXISTS(SELECT 1 FROM ah_reader_member WHERE publication=?1)",
                    [old],
                    |r| Ok(r.get::<i64>(0)? != 0),
                )
            })
            .unwrap()
    };
    assert!(retained());
    drop(pin);
    drop(lease);
    let deadline = Instant::now() + Duration::from_secs(3);
    while retained() {
        assert!(Instant::now() < deadline, "idle owner left the retired member");
        std::thread::sleep(Duration::from_millis(5));
    }
    a.stop_jobs();
}

#[test]
fn shared_codex_predecessor_is_resolved_once_per_restage() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    let ids = [
        "11111111-1111-4111-8111-111111111111",
        "22222222-2222-4222-8222-222222222222",
        "33333333-3333-4333-8333-333333333333",
    ];
    let mut last_size = 0;
    for (i, id) in ids.iter().enumerate() {
        let mut meta = serde_json::json!({"type":"session_meta","payload":{"id":ids[0],"cwd":"/p","timestamp":format!("2026-11-10T1{i}:00:00Z")}});
        let name = if i == 0 {
            id.to_string()
        } else {
            meta["payload"]["history_base"] = serde_json::json!({"thread_id":ids[i-1],"end_ordinal_exclusive":i,"end_byte_offset":last_size});
            format!("{}_{id}", ids[0])
        };
        let raw = format!("{meta}\n");
        last_size = raw.len();
        fs::write(sessions.join(format!("rollout-2026-11-10T1{i}-00-00-{name}.jsonl")), raw)
            .unwrap();
    }
    a.backup(&[SourceRoot { harness: Harness::Codex, root }], &Default::default()).unwrap();
    let h = a.reader_header(&a.list_sessions(&Default::default()).unwrap().items[0].id).unwrap();
    assert_eq!(a.reader_sources(&h, None, 8).unwrap().len(), 3);
    assert_eq!(h.physical_records, 3);
    let source = h.current_source.clone();
    a.store.write(DB, Priority::Batch, move |tx| refresh(tx, &source, None)).unwrap();
    a.store.capture_query_plans(DB, true).unwrap();
    a.flush_reader_heads().unwrap();
    let plans = a.store.take_query_plans(DB).unwrap();
    assert_eq!(plans.iter().filter(|p| p.sql.contains("p.rollout_native_id=?2")).count(), 2);
    let segments = plans
        .iter()
        .filter(|p| p.sql.contains("SELECT v.token,v.records,s.lineage_base_native_id"))
        .count();
    assert_eq!(segments, 3, "each source version is loaded once");
}

#[test]
fn upgrade_recovers_the_parent_from_the_elected_child_descriptor() {
    use vault_store::{Config, MigrationRegistry, Store};
    let t = tempfile::tempdir().unwrap();
    let open = |old| {
        let mut cfg = Config::new(t.path());
        cfg.databases = vec![DB, Db::Scratch];
        let mut migrations = crate::archive::schema_steps(DB);
        if old {
            // Version 62 stored the parent in the member descriptor and ah_reader_edge.
            // Migration 63 added the parent column to ah_reader_head.
            migrations.truncate(62);
        }
        cfg.migrations = MigrationRegistry::default();
        cfg.migrations.register(DB, migrations);
        Store::open(cfg).unwrap()
    };
    let old = open(true);
    old.write(DB, Priority::Batch, |tx| {
        tx.execute(
            "INSERT INTO ah_reader_head VALUES('child','publication','source','digest')",
            (),
        )?;
        tx.execute(
            "INSERT INTO ah_reader_member VALUES('publication','source','v',?1,1,1,0,NULL,1)",
            [r#"{"parent_id":"local","relation":{"parent_native_id":"native"}}"#],
        )?;
        tx.execute("INSERT INTO ah_reader_edge VALUES('obsolete-parent-publication','child')", ())?;
        Ok(())
    })
    .unwrap();
    drop(old);
    let upgraded = open(false);
    let relationship = upgraded
        .read(DB, |tx| {
            tx.query_one(
                "SELECT parent,parent_status FROM ah_reader_head WHERE conversation='child'",
                (),
                |r| Ok((r.get::<Option<String>>(0)?, r.get::<String>(1)?)),
            )
        })
        .unwrap();
    assert_eq!(relationship, (Some("local".into()), "resolved".into()));
}

#[path = "round_two_tests.rs"]
pub(super) mod round_two;

#[path = "isolation_tests.rs"]
mod isolation;
