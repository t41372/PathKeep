//! Responsibilities: verify retained bytes independently of normalization and search.
//! Not responsible for: mocking the database engine or changing source records.
//! Failure modes specified first: raw-only evidence is skipped, incomplete attempts
//! are mistaken for complete revisions, and a stale search index passes as current.
use agent_history::{Archive, Artifact, ArtifactKind, Harness, SessionMetadata, SourceRoot};
use sha2::{Digest, Sha256};
use vault_crypto::KdfParams;

#[test]
fn verification_checks_complete_raw_revisions_without_requiring_parseable_messages() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &temp.path().join("archive"),
        b"synthetic password",
        KdfParams { memory_kib: 8192, iterations: 1, lanes: 1 },
    )
    .unwrap();
    let source = SourceRoot { harness: Harness::Pi, root: temp.path().join("native") };
    let artifact = Artifact {
        path: source.root.join("damaged.jsonl"),
        relative_path: "damaged.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let bytes = b"not valid JSON, but retained native evidence\0\xff";
    let revision = archive.begin_revision("source", &source, &artifact).unwrap();
    archive.append_chunk(revision, 0, bytes.to_vec()).unwrap();
    archive
        .finish_revision(revision, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap();
    let interrupted = archive.begin_revision("source", &source, &artifact).unwrap();
    archive.append_chunk(interrupted, 0, b"unfinished".to_vec()).unwrap();
    let report = archive.verify().unwrap();
    assert_eq!(report.revisions, 1);
    assert_eq!(report.bytes, bytes.len() as u64);
    assert_eq!(report.status, "complete");

    // No session was parseable: generation zero must not hide the coverage failure.
    archive.set_coverage("source", false).unwrap();
    assert!(!archive.verify().unwrap().normalization_complete);

    // An unavailable index cannot erase the independently known normalization gap.
    let attempt_revision =
        archive.prepare_projection("source", revision, "fixture-v1", false).unwrap().unwrap();
    archive
        .publish_projection_attempt(
            &attempt_revision,
            &SessionMetadata { native_id: "native-source".into(), ..SessionMetadata::default() },
            false,
        )
        .unwrap();
    let report = archive.verify().unwrap();
    assert!(!report.search_current);
    assert!(!report.normalization_complete);
}
