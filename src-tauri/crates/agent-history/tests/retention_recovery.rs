//! Responsibilities: exercise retention failures beyond the CLI contract's small-file cases.
//! Not responsible for: adapter discovery or command presentation.
//! Failure modes written before implementation: state A→B→A reuses pruned manifests;
//! a multi-slice prefix→rewrite→append loses old bytes; a reader opened before prefix
//! retargeting sees a missing tail or reads beyond its own revision boundary.
use agent_history::{Archive, Artifact, ArtifactKind, Harness, SourceRoot};
use sha2::{Digest, Sha256};
use std::io::Read;
use vault_crypto::KdfParams;

fn create(path: &std::path::Path) -> Archive {
    Archive::create_with_params(
        path,
        b"synthetic retention password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap()
}

fn retain(archive: &Archive, kind: ArtifactKind, bytes: &[u8]) -> i64 {
    retain_slices(archive, kind, bytes, agent_history::archive::SLICE_BYTES)
}

fn retain_slices(archive: &Archive, kind: ArtifactKind, bytes: &[u8], slice_bytes: usize) -> i64 {
    let source = SourceRoot { harness: Harness::Codex, root: "/synthetic/codex".into() };
    let artifact = Artifact {
        path: "/synthetic/codex/state_5.sqlite".into(),
        relative_path: "state_5.sqlite".into(),
        kind,
        project_hint: None,
        generation: None,
        current: true,
    };
    let id = archive.begin_revision("retention-source", &source, &artifact).unwrap();
    for (ordinal, chunk) in bytes.chunks(slice_bytes).enumerate() {
        archive.append_chunk(id, ordinal as u64, chunk.to_vec()).unwrap();
    }
    archive
        .finish_revision(id, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id
}

fn read(archive: &Archive, id: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive.raw_reader(id).unwrap().read_to_end(&mut bytes).unwrap();
    bytes
}

#[test]
fn a_state_returning_to_pruned_content_restores_its_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(temp.path());
    let first = retain(&archive, ArtifactKind::Sqlite, b"state A");
    retain(&archive, ArtifactKind::Sqlite, b"state B");
    assert!(!archive.revision(first).unwrap().bytes_retained);
    let restored = retain(&archive, ArtifactKind::Sqlite, b"state A");
    assert_eq!(read(&archive, restored), b"state A");
    assert_eq!(
        archive
            .revisions("retention-source", 100, None)
            .unwrap()
            .items
            .iter()
            .filter(|r| r.bytes_retained)
            .count(),
        1
    );
}

#[test]
fn prefix_retargeting_keeps_open_readers_and_rewritten_branches_byte_exact() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(temp.path());
    let mut first = vec![b'x'; agent_history::archive::SLICE_BYTES + 37];
    let old = retain_slices(&archive, ArtifactKind::Transcript, &first, 1024 * 1024);
    let mut reader = archive.raw_reader(old).unwrap();
    let mut received = vec![0; 12345];
    reader.read_exact(&mut received).unwrap();
    let mut appended = first.clone();
    appended.extend(vec![b'y'; agent_history::archive::SLICE_BYTES + 29]);
    let next = retain(&archive, ArtifactKind::Transcript, &appended);
    reader.read_to_end(&mut received).unwrap();
    assert_eq!(received, first);
    first[19] = b'z';
    let rewrite = retain(&archive, ArtifactKind::Transcript, &first);
    first.extend(b"more after rewriting");
    let final_revision = retain(&archive, ArtifactKind::Transcript, &first);
    assert_eq!(read(&archive, old), appended[..agent_history::archive::SLICE_BYTES + 37]);
    assert_eq!(read(&archive, next), appended);
    assert_eq!(read(&archive, rewrite), first[..first.len() - 20]);
    assert_eq!(read(&archive, final_revision), first);
}

// Failure mode: retention removes a state manifest between reader slices and
// misreports the intentional expiry as archive corruption.
#[test]
fn a_state_reader_reports_expiry_instead_of_missing_slice_damage() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(temp.path());
    let first = vec![b'a'; agent_history::archive::SLICE_BYTES + 37];
    let revision = retain(&archive, ArtifactKind::Sqlite, &first);
    let mut reader = archive.raw_reader(revision).unwrap();
    reader.read_exact(&mut [0; 1]).unwrap();
    retain(&archive, ArtifactKind::Sqlite, b"new state");
    let error = reader.read_to_end(&mut Vec::new()).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains("expired by retention"));
    assert!(!error.to_string().contains("chunk is missing"));
}

// Failure mode: raw export wraps the reader's intentional expiry as a generic I/O error.
#[test]
fn raw_export_keeps_the_existing_expired_revision_error() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(temp.path());
    let bytes = vec![b'a'; agent_history::archive::SLICE_BYTES + 37];
    let revision = retain(&archive, ArtifactKind::Sqlite, &bytes);
    struct Expire<'a> {
        archive: &'a Archive,
        done: bool,
    }
    impl std::io::Write for Expire<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.done {
                self.done = true;
                retain(self.archive, ArtifactKind::Sqlite, b"replacement state");
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let error =
        archive.write_raw(revision, &mut Expire { archive: &archive, done: false }).unwrap_err();
    assert_eq!(error.code(), "not_found");
    assert!(error.to_string().contains("expired by retention"));
}

#[test]
fn expired_record_rejects_even_an_empty_final_continuation() {
    use agent_history::read_model::*;
    let tmp = tempfile::tempdir().unwrap();
    let archive = create(tmp.path());
    let id = retain(&archive, ArtifactKind::Sqlite, b"{}\n");
    let revision = archive.revision(id).unwrap();
    let mut locator = None;
    scan_records(
        &mut b"{}\n".as_slice(),
        Harness::Codex,
        Transport::Plain,
        &Default::default(),
        &Default::default(),
        &mut |row, _, _| {
            locator = Some(row);
            Ok(())
        },
    )
    .unwrap();
    let row = locator.unwrap();
    assert!(
        archive
            .record_bytes(&revision, &row, row.length, 16, &Default::default())
            .unwrap()
            .bytes
            .is_empty()
    );
    retain(&archive, ArtifactKind::Sqlite, b"replacement");
    for offset in [0, row.length] {
        let error =
            archive.record_bytes(&revision, &row, offset, 16, &Default::default()).unwrap_err();
        assert_eq!(error.code(), "not_found");
    }
}

#[test]
fn replacing_retained_state_removes_its_reader_publication() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(temp.path());
    let revision = retain(
        &archive,
        ArtifactKind::Metadata,
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"state\"}}\n",
    );
    let attempt =
        archive.prepare_projection("retention-source", revision, "state", true).unwrap().unwrap();
    archive
        .publish_projection_attempt(
            &attempt,
            &agent_history::SessionMetadata { native_id: "state".into(), ..Default::default() },
            true,
        )
        .unwrap();
    let id = archive.list_sessions(&Default::default()).unwrap().items.remove(0).id;
    let h = archive.reader_header(&id).unwrap();
    assert_eq!(archive.reader_window(&h, None, 0, 1, &Default::default()).unwrap().len(), 1);
    retain(&archive, ArtifactKind::Metadata, b"replacement");
    assert!(matches!(archive.reader_header(&id), Err(agent_history::Error::ConversationNotFound)));
    while archive.prune_reader_pages(2).unwrap() > 0 {}
}
