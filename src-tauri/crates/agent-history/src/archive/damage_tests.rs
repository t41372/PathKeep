//! Exercises public capture and mark operations against isolated storage faults.
//! The store handle is used only to introduce or inspect damage in a real archive.
use super::{Archive, ConversationMark};
use crate::{Harness, SourceRoot};
use serde_json::json;
use std::path::Path;
use vault_store::{Db, Priority};

const THREAD: &str = "11111111-1111-4111-8111-111111111111";

fn rollout(root: &Path, message: Option<&str>) -> SourceRoot {
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let path = sessions.join(format!("rollout-2026-11-10T12-00-00-{THREAD}.jsonl"));
    let header = json!({"type":"session_meta","payload":{"id":THREAD,"cwd":"/synthetic",
        "timestamp":"2026-11-10T12:00:00Z"}});
    let mut body = format!("{header}\n");
    if let Some(message) = message {
        let row = json!({"type":"response_item","payload":{"type":"message","role":"user",
            "content":[{"type":"input_text","text":message}]}});
        body.push_str(&format!("{row}\n"));
    }
    std::fs::write(path, body).unwrap();
    SourceRoot { harness: Harness::Codex, root: root.into() }
}

fn capture(archive: &Archive, root: &SourceRoot) -> crate::Result<crate::capture::BackupReport> {
    archive.backup(std::slice::from_ref(root), &Default::default())
}

// Losing the only manifest row on the old head used to panic during prefix comparison.
#[test]
fn damaged_source_missing_slice_is_an_integrity_error() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let root = rollout(&temp.path().join("native"), None);
    capture(&archive, &root).unwrap();
    let old = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            assert_eq!(
                tx.execute(
                    "DELETE FROM ah_revision_chunks WHERE revision_id=?1",
                    [old.revision_id]
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
    rollout(&root.root, Some("an append"));
    let error = capture(&archive, &root).unwrap_err();
    assert_eq!(error.code(), "integrity_error");
    assert!(error.to_string().contains(&old.source_id));
    assert_eq!(archive.latest_revision(&old.source_id).unwrap().unwrap().id, old.revision_id);
}

// Copy selection used to panic looking up a published member whose raw revision is absent.
#[test]
fn damaged_source_missing_candidate_revision_is_an_integrity_error() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let root = rollout(&temp.path().join("first"), Some("first"));
    capture(&archive, &root).unwrap();
    let old = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    archive
        .store
        .write(Db::AgentHistoryReadModel, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_sessions SET revision_id=-1", ())?;
            Ok(())
        })
        .unwrap();
    let copy = rollout(&temp.path().join("copy"), Some("second"));
    let error = capture(&archive, &copy).unwrap_err();
    assert_eq!(error.code(), "integrity_error");
    assert!(error.to_string().contains(&old.source_id));
}

// A constraint on the mirror alone used to turn a committed edit into a failed mark.
#[test]
fn committed_mark_survives_a_failed_mirror_write() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let root = rollout(&temp.path().join("first"), Some("first title"));
    capture(&archive, &root).unwrap();
    let item = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    let native_title = item.title.clone();
    archive
        .mark_conversation(
            &item.id,
            &ConversationMark { title: Some(Some("override".into())), ..Default::default() },
            true,
        )
        .unwrap();
    archive
        .store
        .write(Db::AgentHistoryReadModel, Priority::Batch, |tx| {
            tx.execute(
                "INSERT INTO ah_user_state(conversation_id,starred) VALUES('fixture',1)",
                (),
            )?;
            tx.execute("CREATE UNIQUE INDEX fixture_star ON ah_user_state(starred)", ())?;
            Ok(())
        })
        .unwrap();
    let result = archive.mark_conversation(
        &item.id,
        &ConversationMark {
            starred: Some(true),
            trash: Some(true),
            put_away: Some(true),
            title: Some(None),
        },
        true,
    );
    let committed = archive
        .store
        .read(Db::AgentHistoryEvidence, |tx| {
            tx.query_one("SELECT starred FROM ah_user_state", (), |row| Ok(row.get::<i64>(0)?))
        })
        .unwrap();
    assert_eq!(committed, 1);
    let marked = result.unwrap();
    assert!(marked.starred && marked.trashed && marked.put_away);
    assert_eq!(marked.title, native_title);
    assert_eq!(marked.title_source, item.title_source);
    assert!(archive.list_sessions(&Default::default()).is_err());
    archive
        .store
        .write(Db::AgentHistoryReadModel, Priority::Batch, |tx| {
            tx.execute("DROP INDEX fixture_star", ())?;
            Ok(())
        })
        .unwrap();
    assert!(archive.list_sessions(&Default::default()).unwrap().items.is_empty());
    let recovered = archive.session(&item.id).unwrap();
    assert!(recovered.starred && recovered.put_away);
    assert_eq!(recovered.trashed_at, marked.trashed_at);
    assert_eq!(recovered.title, marked.title);
}

// A foreign key holds the obsolete slice through the first collection attempt.
#[test]
fn committed_backup_survives_a_failed_slice_reclamation() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let root = rollout(&temp.path().join("first"), None);
    capture(&archive, &root).unwrap();
    let old = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute("CREATE TABLE fixture_hold(hash TEXT REFERENCES ah_chunks(hash))", ())?;
            tx.execute("INSERT INTO fixture_hold SELECT hash FROM ah_chunks", ())?;
            Ok(())
        })
        .unwrap();
    rollout(&root.root, Some("appended"));
    let result = capture(&archive, &root);
    let head = archive.latest_revision(&old.source_id).unwrap().unwrap();
    assert_ne!(head.id, old.revision_id);
    let saved = result.unwrap();
    assert!(saved.issues.iter().any(|issue| issue.code == "slice_reclamation_deferred"));
    assert_eq!(archive.backup_run(saved.run_id).unwrap().status, "complete");
    assert_eq!(archive.session(&old.id).unwrap().revision_id, head.id);
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute("DELETE FROM fixture_hold", ())?;
            Ok(())
        })
        .unwrap();
    assert_eq!(capture(&archive, &root).unwrap().status, "complete");
    assert_eq!(archive.verify().unwrap().slices.count, 1);
}

// A retained slice failure comes from storage even when its bytes feed a native decoder.
#[test]
fn damaged_slice_is_distinct_from_invalid_native_transport() {
    use crate::read_model::*;
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
    let root = rollout(&tmp.path().join("native"), None);
    capture(&archive, &root).unwrap();
    let session = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Batch, |tx| {
            tx.execute("UPDATE ah_chunks SET data=x'00'", ())?;
            Ok(())
        })
        .unwrap();
    let error = archive
        .scan_revision(
            session.revision_id,
            &Default::default(),
            &JobControl::default(),
            &mut |_, _, _| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code(), "integrity_error");
}

#[test]
fn invalid_revision_and_manifest_coordinates_are_integrity_failures() {
    let mut codes = Vec::new();
    for sql in
        ["UPDATE ah_revisions SET byte_len=-1", "UPDATE ah_revision_chunks SET byte_offset=-1"]
    {
        let tmp = tempfile::tempdir().unwrap();
        let archive = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
        let root = rollout(&tmp.path().join("native"), None);
        capture(&archive, &root).unwrap();
        let id = archive.list_sessions(&Default::default()).unwrap().items[0].revision_id;
        archive
            .store
            .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| tx.execute(sql, ()))
            .unwrap();
        let err = archive
            .scan_revision(id, &Default::default(), &Default::default(), &mut |_, _, _| Ok(()))
            .unwrap_err();
        codes.push(err.code());
    }
    assert_eq!(codes, ["integrity_error", "integrity_error"]);
}

#[test]
fn array_and_filtered_windows_fetch_each_slice_once() {
    use crate::{Artifact, ArtifactKind};
    use sha2::{Digest, Sha256};
    for (harness, bytes) in [
        (Harness::Cline, format!("[{}]", vec!["{}"; 200].join(","))),
        (Harness::Pi, "{}\nnull\n".repeat(200)),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let a = Archive::create_unencrypted(tmp.path()).unwrap();
        let root = SourceRoot { harness, root: "/fixture".into() };
        let artifact = Artifact {
            path: "/fixture/test.json".into(),
            relative_path: "test.json".into(),
            kind: ArtifactKind::Transcript,
            project_hint: None,
            generation: None,
            current: true,
        };
        let id = a.begin_revision("source", &root, &artifact).unwrap();
        a.append_chunk(id, 0, bytes.as_bytes().to_vec()).unwrap();
        a.finish_revision(id, &hex::encode(Sha256::digest(&bytes)), bytes.len() as u64).unwrap();
        let mut records = Vec::new();
        a.scan_revision(id, &Default::default(), &Default::default(), &mut |r, c, v| {
            if v.is_some() {
                records.push((r, c.clone()));
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(records.len(), 200);
        a.store.capture_query_plans(Db::AgentHistoryEvidence, true).unwrap();
        let mut rows = 0;
        a.read_window(&a.revision(id).unwrap(), &records, 32, &Default::default(), &mut |r| {
            assert_eq!(r.raw.bytes.strip_suffix(b"\n").unwrap_or(&r.raw.bytes), b"{}");
            rows += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(rows, 200);
        let plans = a.store.take_query_plans(Db::AgentHistoryEvidence).unwrap();
        assert_eq!(
            plans.iter().filter(|p| p.sql.contains("SELECT data,codec FROM ah_chunks")).count(),
            1,
            "{harness}"
        );
    }
}

#[test]
fn malformed_revision_digest_and_retention_flag_fail_before_reading() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
    let root = rollout(&tmp.path().join("native"), None);
    capture(&archive, &root).unwrap();
    let id = archive.list_sessions(&Default::default()).unwrap().items[0].revision_id;
    let expected = archive.revision(id).unwrap().digest;
    for (digest, retained) in [("z".repeat(64), 1), (expected, 2)] {
        archive
            .store
            .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
                tx.execute(
                    "UPDATE ah_revisions SET digest=?1,bytes_retained=?2 WHERE id=?3",
                    vault_store::params![digest, retained as i64, id],
                )
            })
            .unwrap();
        assert_eq!(archive.revision(id).err().map(|e| e.code()), Some("integrity_error"));
    }
}

// Both manifests lose the slice that distinguished their contents. Their surviving
// slices have the same offset and hash, so comparison never opens a raw reader.
#[test]
fn matching_tail_slices_cannot_prove_equality_across_a_manifest_gap() {
    use crate::{Artifact, ArtifactKind};
    use sha2::{Digest, Sha256};

    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let source = SourceRoot { harness: Harness::Codex, root: temp.path().join("codex") };
    let artifact = Artifact {
        path: source.root.join("draft.txt"),
        relative_path: "draft.txt".into(),
        kind: ArtifactKind::Attachment,
        project_hint: None,
        generation: None,
        current: true,
    };
    let retain = |first: &[u8]| {
        let tail = b"common final record\n";
        let id = archive.begin_revision("history", &source, &artifact).unwrap();
        archive.append_chunk(id, 0, first.to_vec()).unwrap();
        archive.append_chunk(id, 1, tail.to_vec()).unwrap();
        let digest = Sha256::new().chain_update(first).chain_update(tail).finalize();
        archive
            .finish_revision(id, &hex::encode(digest), (first.len() + tail.len()) as u64)
            .unwrap()
            .revision_id
    };
    let first = retain(b"draft A\n");
    let second = retain(b"draft B\n");
    assert_eq!(archive.revisions_prefix(first, second).unwrap(), (false, false));
    archive
        .store
        .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let removed = tx.execute(
                "DELETE FROM ah_revision_chunks WHERE revision_id IN (?1,?2) AND ordinal=0",
                vault_store::params![first, second],
            )?;
            assert_eq!(removed, 2);
            Ok(())
        })
        .unwrap();
    let error = archive.revisions_prefix(first, second).unwrap_err();
    assert!(matches!(error, crate::Error::Integrity(_)));
}
