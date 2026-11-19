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
        .write(DB, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_reader_version SET digest=?1", params!["0".repeat(64)])?;
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
        native.extend(zstd::stream::encode_all(frame.as_slice(), 1).unwrap());
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
