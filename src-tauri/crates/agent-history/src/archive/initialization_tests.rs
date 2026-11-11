//! Responsibilities: reproduce stale initialization preflight with real encrypted state.
//! Not responsible for: password KDF correctness or storage-engine encryption.
//! Failure mode, specified before the fix: a second initializer passes the empty
//! directory check, waits while another initializer commits, then replaces its key.
use super::*;
use sha2::{Digest, Sha256};

#[test]
fn a_stale_empty_directory_check_cannot_replace_a_completed_archive_key() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    fs::create_dir(&root).unwrap();
    // Initializer B has made the observation the old implementation trusted.
    check_empty_destination(&root).unwrap();
    let archive = Archive::create_with_params(
        &root,
        b"first owner password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    let source = crate::model::SourceRoot {
        harness: crate::model::Harness::Pi,
        root: "/synthetic/pi".into(),
    };
    let artifact = crate::model::Artifact {
        path: "/synthetic/pi/chat.jsonl".into(),
        relative_path: "chat.jsonl".into(),
        kind: crate::model::ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let revision = archive.begin_revision("chat", &source, &artifact).unwrap();
    archive.append_chunk(revision, 0, b"first owner evidence".to_vec()).unwrap();
    let revision = archive
        .finish_revision(revision, &hex::encode(Sha256::digest(b"first owner evidence")), 20)
        .unwrap()
        .revision_id;
    drop(archive);
    let envelope = fs::read(root.join(ENVELOPE_NAME)).unwrap();
    // B resumes at the real post-preflight reservation boundary, after A released
    // the lock. The locked recheck, rather than lock contention, must reject it.
    assert!(reserve_destination(&root).is_err());
    assert_eq!(fs::read(root.join(ENVELOPE_NAME)).unwrap(), envelope);
    let reopened = Archive::open(&root, b"first owner password").unwrap();
    let mut retained = Vec::new();
    reopened.raw_reader(revision).unwrap().read_to_end(&mut retained).unwrap();
    assert_eq!(retained, b"first owner evidence");
}

#[test]
fn envelope_installation_never_clobbers_even_if_called_twice() {
    let temp = tempfile::tempdir().unwrap();
    let (first, _) =
        Envelope::create(b"first", KdfParams { memory_kib: 32, iterations: 1, lanes: 1 }).unwrap();
    let (second, _) =
        Envelope::create(b"second", KdfParams { memory_kib: 32, iterations: 1, lanes: 1 }).unwrap();
    install_new_envelope(temp.path(), &first).unwrap();
    assert!(install_new_envelope(temp.path(), &second).is_err());
    assert_eq!(fs::read(temp.path().join(ENVELOPE_NAME)).unwrap(), first.to_bytes());
}
