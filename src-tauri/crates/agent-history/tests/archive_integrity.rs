//! Responsibilities: exercise preservation/publication failures using real encrypted files.
//! Not responsible for: native harness parsing or CLI presentation.
//!
//! Written before archive implementation. Failure modes: unfinished captures become current;
//! bad lengths/hashes publish; a repeated capture consumes another revision; staging a newer
//! projection hides the old one; wrong credentials mutate the archive; evidence leaks to disk;
//! a process killed after committed chunks loses the previous usable revision.
use agent_history::{
    archive::Archive,
    model::{Artifact, ArtifactKind, Harness, NativeEvent, SessionMetadata, SourceRoot},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path, process::Command};
use vault_crypto::KdfParams;

const PASSWORD: &[u8] = b"archive integrity fixture password";
const SOURCE: &str = "integrity-source";

fn create(path: &Path) -> Archive {
    Archive::create_with_params(
        path,
        PASSWORD,
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap()
}

fn source() -> (SourceRoot, Artifact) {
    (
        SourceRoot { harness: Harness::Pi, root: "/synthetic/agent".into() },
        Artifact {
            path: "/synthetic/agent/sessions/chat.jsonl".into(),
            relative_path: "sessions/chat.jsonl".into(),
            kind: ArtifactKind::Transcript,
            project_hint: None,
            generation: None,
            current: true,
        },
    )
}

fn stage(archive: &Archive, bytes: &[u8]) -> i64 {
    let (source, artifact) = source();
    let id = archive.begin_revision(SOURCE, &source, &artifact).unwrap();
    for (ordinal, chunk) in bytes.chunks(256 * 1024).enumerate() {
        archive.append_chunk(id, ordinal as u64, chunk.to_vec()).unwrap();
    }
    id
}

fn retain(archive: &Archive, bytes: &[u8]) -> i64 {
    let id = stage(archive, bytes);
    let digest = hex::encode(Sha256::digest(bytes));
    archive.finish_revision(id, &digest, bytes.len() as u64).unwrap().revision_id
}

fn read(archive: &Archive, revision: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive.raw_reader(revision).unwrap().read_to_end(&mut bytes).unwrap();
    bytes
}

#[test]
fn unfinished_or_misdescribed_bytes_never_publish() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let previous = retain(&archive, b"complete old bytes");
    let staged = stage(&archive, b"new partial bytes");
    assert!(archive.raw_reader(staged).is_err());
    let digest = hex::encode(Sha256::digest(b"new partial bytes"));
    assert!(archive.finish_revision(staged, &digest, 1).is_err());
    assert!(archive.finish_revision(staged, &"0".repeat(64), 17).is_err());
    assert_eq!(read(&archive, previous), b"complete old bytes");
    assert!(archive.raw_reader(staged).is_err());
}

#[test]
fn source_keys_are_bounded_before_they_can_escape_into_page_cursors() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let (source, artifact) = source();
    let invalid = "x".repeat(257);
    assert!(archive.begin_revision(&invalid, &source, &artifact).is_err());
    assert!(archive.latest_revision(&invalid).unwrap().is_none());
}

#[test]
fn source_inventory_serializes_revision_handles_as_opaque_strings() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let (source, artifact) = source();
    archive.register_source(SOURCE, &source, &artifact).unwrap();
    let empty = serde_json::to_value(archive.source(SOURCE).unwrap()).unwrap();
    assert!(empty["latest_revision_id"].is_null());
    let raw = retain(&archive, b"recoverable raw evidence");
    let captured = serde_json::to_value(archive.source(SOURCE).unwrap()).unwrap();
    assert_eq!(captured["latest_revision_id"], raw.to_string());
}

#[test]
fn discovery_can_promote_reference_metadata_to_a_transcript_without_changing_source_identity() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let (source, mut artifact) = source();
    artifact.kind = ArtifactKind::Metadata;
    let raw = archive.begin_revision(SOURCE, &source, &artifact).unwrap();
    archive.append_chunk(raw, 0, b"native transcript bytes".to_vec()).unwrap();
    let raw = archive
        .finish_revision(raw, &hex::encode(Sha256::digest(b"native transcript bytes")), 23)
        .unwrap()
        .revision_id;
    assert_eq!(archive.source(SOURCE).unwrap().kind, "metadata");
    artifact.kind = ArtifactKind::Transcript;
    archive.register_source(SOURCE, &source, &artifact).unwrap();
    assert_eq!(archive.source(SOURCE).unwrap().kind, "transcript");
    assert_eq!(archive.latest_revision(SOURCE).unwrap().unwrap().id, raw);
    artifact.kind = ArtifactKind::Metadata;
    archive.register_source(SOURCE, &source, &artifact).unwrap();
    assert_eq!(archive.source(SOURCE).unwrap().kind, "transcript");
    assert!(archive.begin_revision(SOURCE, &source, &artifact).is_ok());
    artifact.relative_path = "different.jsonl".into();
    assert!(archive.register_source(SOURCE, &source, &artifact).is_err());
    assert_eq!(archive.source(SOURCE).unwrap().relative_path, "sessions/chat.jsonl");
}

#[test]
fn rewrites_and_truncation_keep_each_exact_version_and_repeats_reuse_it() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let old = retain(&archive, b"alpha");
    let changed = retain(&archive, b"omega");
    let truncated = retain(&archive, b"om");
    let staged = stage(&archive, b"omega");
    let repeated =
        archive.finish_revision(staged, &hex::encode(Sha256::digest(b"omega")), 5).unwrap();
    // Reverting to older retained bytes is a changed source observation, even
    // though the evidence object itself can be reused.
    assert!(!repeated.unchanged);
    assert_eq!(repeated.revision_id, changed);
    assert_eq!(archive.latest_revision(SOURCE).unwrap().unwrap().id, changed);
    let same = stage(&archive, b"omega");
    assert!(
        archive.finish_revision(same, &hex::encode(Sha256::digest(b"omega")), 5).unwrap().unchanged
    );
    assert_eq!(read(&archive, old), b"alpha");
    assert_eq!(read(&archive, changed), b"omega");
    assert_eq!(read(&archive, truncated), b"om");
}

#[test]
fn reclaiming_interrupted_attempts_keeps_chunks_used_by_completed_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let old = retain(&archive, b"shared evidence chunk");
    let unfinished = stage(&archive, b"shared evidence chunk");
    archive.abandon_revision(unfinished).unwrap();
    while archive.prune_abandoned_capture_batch(1).unwrap() > 0 {}
    assert_eq!(read(&archive, old), b"shared evidence chunk");
    assert!(archive.raw_reader(unfinished).is_err());
    let retry = retain(&archive, b"replacement evidence");
    assert_eq!(read(&archive, retry), b"replacement evidence");
}

#[test]
fn chunk_sequence_is_append_only_and_large_reads_cross_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let (source, artifact) = source();
    let revision = archive.begin_revision(SOURCE, &source, &artifact).unwrap();
    assert!(archive.append_chunk(revision, 1, vec![1]).is_err());
    let chunk = vec![0x35; 256 * 1024];
    let mut expected = Sha256::new();
    for ordinal in 0..9 {
        archive.append_chunk(revision, ordinal, chunk.clone()).unwrap();
        expected.update(&chunk);
    }
    assert!(archive.append_chunk(revision, 0, vec![2]).is_err());
    let outcome = archive
        .finish_revision(revision, &hex::encode(expected.finalize()), 9 * 256 * 1024)
        .unwrap();
    let mut reader = archive.raw_reader(outcome.revision_id).unwrap();
    let mut buffer = [0u8; 7919];
    let mut count = 0;
    loop {
        let received = reader.read(&mut buffer).unwrap();
        if received == 0 {
            break;
        }
        assert!(buffer[..received].iter().all(|byte| *byte == 0x35));
        count += received;
    }
    assert_eq!(count, 9 * 256 * 1024);
}

fn metadata() -> SessionMetadata {
    SessionMetadata {
        native_id: "native-chat".into(),
        project_path: Some("/synthetic/project".into()),
        title: Some("Preservation fixture".into()),
        created_at: Some(1_700_000_000_000),
        updated_at: Some(1_700_000_000_100),
        origin: "cli".into(),
        harness_version: None,
        parent_session_id: None,
        model: None,
    }
}

fn event(text: &str) -> NativeEvent {
    NativeEvent {
        ordinal: 0,
        native_id: Some("native-event".into()),
        parent_id: None,
        kind: "message".into(),
        role: Some("user".into()),
        timestamp: Some(1_700_000_000_000),
        text: text.into(),
        payload: serde_json::json!({"type":"message","future_field":{"keep":true}}),
    }
}

#[test]
fn new_projection_is_visible_only_after_publication_and_advances_search_generation_once() {
    let temp = tempfile::tempdir().unwrap();
    let archive = create(&temp.path().join("archive"));
    let old = retain(&archive, b"old source");
    assert!(archive.begin_projection(SOURCE, old).unwrap());
    archive.append_events(SOURCE, old, vec![event("old event")]).unwrap();
    archive.publish_projection(SOURCE, old, &metadata(), true).unwrap();
    let generation = archive.projection_generation().unwrap();
    let new = retain(&archive, b"new source");
    assert!(archive.begin_projection(SOURCE, new).unwrap());
    archive.append_events(SOURCE, new, vec![event("new event")]).unwrap();
    assert_eq!(archive.current_projection(SOURCE).unwrap(), Some(old));
    assert_eq!(archive.projection_generation().unwrap(), generation);
    archive.publish_projection(SOURCE, new, &metadata(), false).unwrap();
    assert_eq!(archive.current_projection(SOURCE).unwrap(), Some(new));
    assert!(archive.projection_generation().unwrap() > generation);
    assert!(!archive.begin_projection(SOURCE, new).unwrap());
    assert_eq!(read(&archive, old), b"old source");
}

#[test]
fn publishing_new_raw_bytes_invalidates_old_search_coverage_before_any_parser_runs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let archive = create(&root);
    let original = retain(&archive, b"original native evidence");
    archive.begin_projection(SOURCE, original).unwrap();
    archive.append_events(SOURCE, original, vec![event("retained searchable text")]).unwrap();
    archive.publish_projection(SOURCE, original, &metadata(), true).unwrap();
    archive.synchronize_search().unwrap();
    let query = agent_history::search::SearchQuery {
        text: "searchable".into(),
        harness: None,
        project_path: None,
        limit: 10,
    };
    assert!(archive.search(&query).unwrap().complete);
    // Use only the public low-level capture APIs, then end the owner before any
    // high-level backup code can adjust coverage or start the new projection.
    retain(&archive, b"newer native evidence waiting for parser");
    drop(archive);
    let reopened = Archive::open(&root, PASSWORD).unwrap();
    let previous = reopened.search(&query).unwrap();
    assert!(!previous.complete);
    assert_eq!(previous.total, 1);
    assert_eq!(reopened.current_projection(SOURCE).unwrap(), Some(original));
}

fn disk_snapshot(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut snapshot = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            snapshot.extend(disk_snapshot(&path));
        } else {
            snapshot.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    snapshot
}

#[test]
fn wrong_password_cannot_modify_or_expose_encrypted_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let archive = create(&root);
    let sentinel = b"SECRET-AGENT-HISTORY-SENTINEL-57f6ce92";
    let revision = retain(&archive, sentinel);
    drop(archive);
    let before = disk_snapshot(&root);
    assert!(before.values().all(|bytes| !bytes.windows(sentinel.len()).any(|s| s == sentinel)));
    assert!(Archive::open(&root, b"incorrect password").is_err());
    assert_eq!(disk_snapshot(&root), before);
    let reopened = Archive::open(&root, PASSWORD).unwrap();
    assert_eq!(read(&reopened, revision), sentinel);
}

#[test]
fn initialization_refuses_an_occupied_directory_without_touching_it() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("user-data"), b"keep this").unwrap();
    let before = disk_snapshot(temp.path());
    assert!(Archive::create(temp.path(), PASSWORD).is_err());
    assert_eq!(disk_snapshot(temp.path()), before);
}

#[test]
fn a_missing_canonical_database_is_not_replaced_with_an_empty_archive() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let archive = create(&root);
    retain(&archive, b"previous canonical evidence");
    drop(archive);
    fs::rename(root.join("archive/evidence.db"), root.join("retained-evidence.db")).unwrap();
    let before = disk_snapshot(&root);
    assert!(Archive::open(&root, PASSWORD).is_err());
    assert_eq!(disk_snapshot(&root), before);
}

#[test]
fn crash_child_after_chunks_before_publication() {
    let Some(root) = std::env::var_os("PATHKEEP_AGENT_HISTORY_CRASH_ROOT") else {
        return;
    };
    let archive = Archive::open(Path::new(&root), PASSWORD).unwrap();
    stage(&archive, b"unpublished crash bytes");
    // Exit without dropping the archive: committed chunks survive but no usable
    // revision may point at an incomplete capture after the owner restarts.
    std::process::exit(91);
}

#[test]
fn process_exit_during_capture_preserves_previous_published_revision() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("archive");
    let archive = create(&root);
    let previous = retain(&archive, b"previous durable bytes");
    drop(archive);
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child_after_chunks_before_publication", "--nocapture"])
        .env("PATHKEEP_AGENT_HISTORY_CRASH_ROOT", &root)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(91));
    let reopened = Archive::open(&root, PASSWORD).unwrap();
    assert_eq!(read(&reopened, previous), b"previous durable bytes");
    let retry = retain(&reopened, b"complete retry bytes");
    assert_eq!(read(&reopened, retry), b"complete retry bytes");
}
