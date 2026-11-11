//! Search integrity scenarios written before the search implementation.
//!
//! These use the real encrypted archive and Tantivy index. CLI E2E covers the user
//! flow; these isolate data-loss windows that a normal CLI run cannot pause in:
//! canonical publication ahead of the index, replacement generations, reopening
//! an index, missing derived files, and text beyond common indexing size caps.
//! No test substitutes the database, index, or encryption implementation.

use agent_history::{
    Archive, Artifact, ArtifactKind, Error, Harness, NativeEvent, SearchQuery, SessionMetadata,
    SourceRoot,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use tempfile::TempDir;
use vault_crypto::KdfParams;

const PASSWORD: &[u8] = b"search-fixture-password";

fn archive(root: &Path) -> Archive {
    Archive::create_with_params(
        root,
        PASSWORD,
        KdfParams { memory_kib: 8, iterations: 1, lanes: 1 },
    )
    .unwrap()
}

fn publish(
    archive: &Archive,
    id: &str,
    harness: Harness,
    project: &str,
    text: &str,
    complete: bool,
) {
    publish_with_empty_prefix(archive, id, harness, project, text, complete, 0);
}

fn publish_with_empty_prefix(
    archive: &Archive,
    id: &str,
    harness: Harness,
    project: &str,
    text: &str,
    complete: bool,
    empty_prefix: usize,
) {
    let source = SourceRoot { harness, root: Path::new("/synthetic/agent").into() };
    let filename = format!("{}.jsonl", hex::encode(Sha256::digest(id.as_bytes())));
    let artifact = Artifact {
        path: source.root.join(&filename),
        relative_path: filename,
        kind: ArtifactKind::Transcript,
        project_hint: Some(project.into()),
        generation: None,
        current: true,
    };
    let payload = serde_json::json!({"type":"message","role":"user","text":text});
    let mut bytes = b"{\"type\":\"metadata\"}\n".repeat(empty_prefix);
    bytes.extend(serde_json::to_vec(&payload).unwrap());
    let revision = archive.begin_revision(id, &source, &artifact).unwrap();
    for (ordinal, chunk) in bytes.chunks(256 * 1024).enumerate() {
        archive.append_chunk(revision, ordinal as u64, chunk.to_vec()).unwrap();
    }
    let digest = hex::encode(Sha256::digest(&bytes));
    let revision = archive.finish_revision(revision, &digest, bytes.len() as u64).unwrap();
    archive.begin_projection(id, revision.revision_id).unwrap();
    for start in (0..empty_prefix).step_by(128) {
        let events = (start..(start + 128).min(empty_prefix))
            .map(|ordinal| NativeEvent {
                ordinal: ordinal as u64,
                native_id: None,
                parent_id: None,
                kind: "metadata".into(),
                role: None,
                timestamp: None,
                text: String::new(),
                payload: serde_json::json!({"type":"metadata"}),
            })
            .collect();
        archive.append_events(id, revision.revision_id, events).unwrap();
    }
    archive
        .append_events(
            id,
            revision.revision_id,
            vec![NativeEvent {
                ordinal: empty_prefix as u64,
                native_id: Some("same-native-event-id".into()),
                parent_id: None,
                kind: "message".into(),
                role: Some("user".into()),
                timestamp: Some(1_700_000_000_000),
                text: text.into(),
                payload,
            }],
        )
        .unwrap();
    archive
        .publish_projection(
            id,
            revision.revision_id,
            &SessionMetadata {
                native_id: "same-native-session-id".into(),
                project_path: Some(project.into()),
                title: None,
                created_at: Some(1_700_000_000_000),
                updated_at: Some(1_700_000_000_000),
                origin: "unknown".into(),
                harness_version: None,
                parent_session_id: None,
                model: None,
            },
            complete,
        )
        .unwrap();
}

fn query(text: &str) -> SearchQuery {
    SearchQuery { text: text.into(), harness: None, project_path: None, limit: 20 }
}

#[test]
fn publication_ahead_of_search_is_explicit_and_replacement_removes_old_text() {
    let temporary = TempDir::new().unwrap();
    let archive = archive(&temporary.path().join("archive"));
    publish(&archive, "codex-a", Harness::Codex, "/work/a", "obsoleteword", true);
    assert!(matches!(archive.search(&query("obsoleteword")), Err(Error::IndexBehind)));

    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("obsoleteword")).unwrap().total, 1);
    publish(&archive, "codex-a", Harness::Codex, "/work/a", "replacementword", true);
    assert!(matches!(archive.search(&query("obsoleteword")), Err(Error::IndexBehind)));

    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("obsoleteword")).unwrap().total, 0);
    let replacement = archive.search(&query("replacementword")).unwrap();
    assert_eq!(replacement.total, 1);
    assert_eq!(replacement.items[0].session_id, "codex-a");
    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("replacementword")).unwrap().total, 1);
}

#[test]
fn native_ids_do_not_merge_harnesses_and_unicode_filters_are_exact() {
    let temporary = TempDir::new().unwrap();
    let archive = archive(&temporary.path().join("archive"));
    publish(&archive, "codex-a", Harness::Codex, "/work/shared", "整理聊天記錄 searchable", true);
    publish(&archive, "pi-a", Harness::Pi, "/work/shared", "整理聊天記錄 searchable", true);
    publish(&archive, "pi-b", Harness::Pi, "/work/other", "整理聊天記錄 searchable", true);
    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("聊天")).unwrap().total, 3);

    let mut filtered = query("searchable");
    filtered.harness = Some(Harness::Pi);
    filtered.project_path = Some("/work/shared".into());
    let page = archive.search(&filtered).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].session_id, "pi-a");
}

#[test]
fn long_utf8_project_paths_remain_filterable_without_losing_matches() {
    let temporary = TempDir::new().unwrap();
    let archive = archive(&temporary.path().join("archive"));
    // Below Windows' 32,767 UTF-16 long-path limit, but above Tantivy's 65,530-byte
    // term limit. Metadata is portable even when this test host is not Windows.
    let project = format!("\\\\?\\C:\\{}", format!("{}\\", "界".repeat(100)).repeat(220));
    publish(&archive, "codex-a", Harness::Codex, &project, "pathneedle", true);
    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("pathneedle")).unwrap().total, 1);
    let mut filter = query("pathneedle");
    filter.project_path = Some(project.clone());
    assert_eq!(archive.search(&filter).unwrap().total, 1);
    filter.project_path = Some(format!("{project}other"));
    assert_eq!(archive.search(&filter).unwrap().total, 0);
}

#[test]
fn native_records_without_search_text_still_have_bounded_cancellation_points() {
    let temporary = TempDir::new().unwrap();
    let archive = archive(&temporary.path().join("archive"));
    publish_with_empty_prefix(
        &archive,
        "codex-a",
        Harness::Codex,
        "/work/a",
        "tailneedle",
        true,
        10_000,
    );
    let checks = std::cell::Cell::new(0);
    let result = archive.synchronize_search_with_cancel(&|| {
        checks.set(checks.get() + 1);
        checks.get() == 8
    });
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(matches!(archive.search(&query("tailneedle")), Err(Error::IndexBehind)));
    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("tailneedle")).unwrap().total, 1);
}

#[test]
fn reopening_preserves_coverage_and_missing_index_requires_explicit_rebuild() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("archive");
    let archive = archive(&root);
    publish(&archive, "pi-a", Harness::Pi, "/work/a", "knownword", false);
    archive.synchronize_search().unwrap();
    let result = archive.search(&query("knownword")).unwrap();
    assert_eq!(result.total, 1);
    assert!(!result.complete, "partial normalization must not claim full searchable coverage");
    drop(archive);

    let reopened = Archive::open(&root, PASSWORD).unwrap();
    assert_eq!(reopened.search(&query("knownword")).unwrap().total, 1);
    std::fs::remove_dir_all(root.join("derived/history-search")).unwrap();
    assert!(matches!(reopened.search(&query("knownword")), Err(Error::IndexBehind)));
    assert!(!root.join("derived/history-search").exists(), "search must stay read-only");
    reopened.synchronize_search().unwrap();
    assert_eq!(reopened.search(&query("knownword")).unwrap().total, 1);
}

#[test]
fn a_large_message_keeps_terms_after_eight_mebibytes_and_index_stays_encrypted() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("archive");
    let archive = archive(&root);
    let mut text = "padding ".repeat(1024 * 1024 + 1);
    text.push_str("NEVER_PLAINTEXT_HISTORY_SENTINEL tailneedle");
    publish(&archive, "deepseek-a", Harness::Deepseek, "/work/a", &text, true);
    archive.synchronize_search().unwrap();
    let result = archive.search(&query("tailneedle")).unwrap();
    assert_eq!(result.total, 1, "the entire message is indexed, not an 8 MiB prefix");
    assert_eq!(result.items[0].session_id, "deepseek-a");
    assert!(result.items[0].text.contains("tailneedle"));

    for entry in std::fs::read_dir(root.join("derived/history-search")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let bytes = std::fs::read(path).unwrap();
            assert!(
                !bytes
                    .windows(b"NEVER_PLAINTEXT_HISTORY_SENTINEL".len())
                    .any(|part| { part == b"NEVER_PLAINTEXT_HISTORY_SENTINEL" }),
                "search artifacts must not retain plaintext message content"
            );
        }
    }
}

#[test]
fn cancelling_a_replay_never_acknowledges_unindexed_canonical_changes() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("archive");
    let archive = archive(&root);
    publish(&archive, "pi-a", Harness::Pi, "/work/a", "originalword", true);
    archive.synchronize_search().unwrap();
    let replacement = format!("{}replacementword", " ".repeat(5 * 1024 * 1024));
    publish(&archive, "pi-a", Harness::Pi, "/work/a", &replacement, true);
    let checks = std::cell::Cell::new(0);
    let cancelled = archive.synchronize_search_with_cancel(&|| {
        checks.set(checks.get() + 1);
        checks.get() >= 4
    });
    assert!(matches!(cancelled, Err(Error::Cancelled)));
    drop(archive);
    let archive = Archive::open(&root, PASSWORD).unwrap();
    assert!(matches!(archive.search(&query("replacementword")), Err(Error::IndexBehind)));
    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query("replacementword")).unwrap().total, 1);
    assert_eq!(archive.search(&query("originalword")).unwrap().total, 0);
}

#[test]
fn explicit_rebuild_repairs_a_corrupt_segment_even_when_commit_metadata_survives() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("archive");
    let archive = archive(&root);
    publish(&archive, "pi-a", Harness::Pi, "/work/a", "repairneedle", true);
    archive.synchronize_search().unwrap();
    let postings = std::fs::read_dir(root.join("derived/history-search"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "idx"))
        .expect("the real Tantivy index has a postings segment");
    std::fs::write(postings, b"damaged ciphertext").unwrap();
    assert!(matches!(archive.search(&query("repairneedle")), Err(Error::IndexBehind)));

    let repair = archive.rebuild_search().unwrap();
    assert!(repair.rebuilt);
    assert_eq!(archive.search(&query("repairneedle")).unwrap().total, 1);
}

#[cfg(unix)]
#[test]
fn rebuilding_rejects_a_search_parent_redirected_outside_the_archive() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("archive");
    let archive = archive(&root);
    publish(&archive, "pi-a", Harness::Pi, "/work/a", "knownword", true);
    let external = temporary.path().join("unrelated");
    std::fs::create_dir_all(external.join("history-search")).unwrap();
    let sentinel = external.join("history-search/user-file");
    std::fs::write(&sentinel, b"must survive").unwrap();
    std::fs::rename(root.join("derived"), root.join("original-derived")).unwrap();
    std::os::unix::fs::symlink(&external, root.join("derived")).unwrap();

    assert!(matches!(archive.synchronize_search(), Err(Error::InvalidInput(_))));
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"must survive");
}
