//! Query integrity scenarios, written before the query implementation.
//!
//! Failures that matter: timestamp ties or NULLs skip conversations; a cursor is
//! reused with a different filter; publication while paging silently duplicates
//! or loses records; a partial projection becomes visible; equal native IDs in
//! different sources merge; old raw revisions disappear from the archive.
//! These tests use the real encrypted store and its public capture API. The CLI
//! E2E suite separately proves discovery, native parsing and user entry points.

use agent_history::{
    Archive, Error,
    model::{Artifact, ArtifactKind, Harness, NativeEvent, SessionMetadata, SourceRoot},
    query::{EventQuery, SessionQuery, SessionSort},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use vault_crypto::KdfParams;

fn archive() -> (TempDir, Archive) {
    let directory = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &directory.path().join("archive"),
        b"query-test-password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    (directory, archive)
}

fn raw_revision(archive: &Archive, id: &str, harness: Harness, bytes: &[u8]) -> i64 {
    let source = SourceRoot { harness, root: format!("/fixture/{id}").into() };
    let artifact = Artifact {
        path: source.root.join("session.jsonl"),
        relative_path: "session.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let revision = archive.begin_revision(id, &source, &artifact).unwrap();
    for (ordinal, bytes) in bytes.chunks(256 * 1024).enumerate() {
        archive.append_chunk(revision, ordinal as u64, bytes.to_vec()).unwrap();
    }
    archive
        .finish_revision(revision, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id
}

fn event(ordinal: u64, text: &str) -> NativeEvent {
    NativeEvent {
        ordinal,
        native_id: Some(format!("native-{ordinal}")),
        parent_id: ordinal.checked_sub(1).map(|value| format!("native-{value}")),
        kind: "message".into(),
        role: Some("user".into()),
        timestamp: Some(1_700_000_000_000 + ordinal as i64),
        text: text.into(),
        payload: json!({"type":"message", "text":text, "future_field":{"keep":true}}),
    }
}

fn publish(
    archive: &Archive,
    id: &str,
    harness: Harness,
    created: Option<i64>,
    updated: Option<i64>,
    texts: &[&str],
) -> i64 {
    let bytes = serde_json::to_vec(texts).unwrap();
    let revision = raw_revision(archive, id, harness, &bytes);
    assert!(archive.begin_projection(id, revision).unwrap());
    archive
        .append_events(
            id,
            revision,
            texts.iter().enumerate().map(|(index, text)| event(index as u64, text)).collect(),
        )
        .unwrap();
    archive
        .publish_projection(
            id,
            revision,
            &SessionMetadata {
                native_id: "same-native-id".into(),
                project_path: Some("/projects/shared".into()),
                title: Some(format!("Title {id}")),
                created_at: created,
                updated_at: updated,
                origin: "unknown".into(),
                harness_version: None,
                parent_session_id: None,
                model: None,
            },
            true,
        )
        .unwrap();
    revision
}

#[test]
fn keyset_pages_keep_timestamp_ties_nulls_and_distinct_source_identities() {
    let (_directory, archive) = archive();
    publish(&archive, "a", Harness::Codex, Some(30), Some(50), &["codex a"]);
    publish(&archive, "b", Harness::Pi, Some(20), Some(50), &["pi b"]);
    publish(&archive, "c", Harness::Codex, Some(10), None, &["codex c"]);
    publish(&archive, "d", Harness::Pi, None, None, &["pi d"]);

    let mut query = SessionQuery { limit: 1, ..SessionQuery::default() };
    let mut identities = Vec::new();
    loop {
        let page = archive.list_sessions(&query).unwrap();
        identities.extend(page.items.iter().map(|item| item.id.clone()));
        assert!(page.items.iter().all(|item| item.native_id == "same-native-id"));
        query.after = page.next_cursor;
        if query.after.is_none() {
            break;
        }
    }
    assert_eq!(identities, ["b", "a", "d", "c"]);

    let page = archive
        .list_sessions(&SessionQuery {
            harness: Some(Harness::Codex),
            project_path: Some("/projects/shared".into()),
            sort: SessionSort::Created,
            ..SessionQuery::default()
        })
        .unwrap();
    assert_eq!(page.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), ["a", "c"]);
    assert!(
        archive
            .list_sessions(&SessionQuery {
                project_path: Some("/projects/shared-child".into()),
                ..SessionQuery::default()
            })
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn changed_scope_or_publication_invalidates_a_session_cursor() {
    let (_directory, archive) = archive();
    publish(&archive, "a", Harness::Codex, Some(1), Some(1), &["a"]);
    publish(&archive, "b", Harness::Pi, Some(2), Some(2), &["b"]);
    let query = SessionQuery { limit: 1, ..SessionQuery::default() };
    let cursor = archive.list_sessions(&query).unwrap().next_cursor.unwrap();
    let wrong_scope = SessionQuery {
        harness: Some(Harness::Codex),
        after: Some(cursor.clone()),
        ..query.clone()
    };
    assert!(matches!(archive.list_sessions(&wrong_scope), Err(Error::InvalidInput(_))));
    publish(&archive, "c", Harness::Codex, Some(3), Some(3), &["c"]);
    let stale = SessionQuery { after: Some(cursor), ..query };
    assert!(matches!(archive.list_sessions(&stale), Err(Error::StaleCursor)));
}

#[test]
fn events_never_mix_published_and_staged_revisions() {
    let (_directory, archive) = archive();
    let first = publish(
        &archive,
        "source",
        Harness::ClaudeCode,
        Some(1),
        Some(2),
        &["old one", "old two", "old three"],
    );
    let query = EventQuery { session_id: "source".into(), limit: 1, after: None };
    let page = archive.events(&query).unwrap();
    assert_eq!(page.items[0].text, "old one");
    let cursor = page.next_cursor.unwrap();

    let staged = raw_revision(&archive, "source", Harness::ClaudeCode, b"replacement bytes");
    archive.begin_projection("source", staged).unwrap();
    archive.append_events("source", staged, vec![event(0, "replacement")]).unwrap();
    let page =
        archive.events(&EventQuery { after: Some(cursor.clone()), ..query.clone() }).unwrap();
    assert_eq!(page.items[0].text, "old two");

    let session = archive.session("source").unwrap();
    archive
        .publish_projection(
            "source",
            staged,
            &SessionMetadata {
                native_id: session.native_id,
                project_path: session.project_path,
                title: session.title,
                created_at: Some(1),
                updated_at: Some(3),
                origin: "unknown".into(),
                harness_version: None,
                parent_session_id: None,
                model: None,
            },
            false,
        )
        .unwrap();
    assert!(matches!(
        archive.events(&EventQuery { after: Some(cursor), ..query.clone() }),
        Err(Error::StaleCursor)
    ));
    let fresh = archive.events(&query).unwrap();
    assert_eq!(fresh.items[0].text, "replacement");
    assert_eq!(fresh.items[0].payload["future_field"]["keep"], true);
    assert!(!archive.session("source").unwrap().complete);
    let revisions = archive.revisions("source", 10, None).unwrap();
    assert_eq!(
        revisions.items.iter().map(|revision| revision.id).collect::<Vec<_>>(),
        [staged, first]
    );
}

#[test]
fn revision_pages_exclude_unfinished_captures_and_are_bound_to_their_source() {
    let (_directory, archive) = archive();
    let first = raw_revision(&archive, "source", Harness::Pi, b"first");
    let second = raw_revision(&archive, "source", Harness::Pi, b"second");
    let source = SourceRoot { harness: Harness::Pi, root: "/fixture/source".into() };
    archive
        .begin_revision(
            "source",
            &source,
            &Artifact {
                path: source.root.join("session.jsonl"),
                relative_path: "session.jsonl".into(),
                kind: ArtifactKind::Transcript,
                project_hint: None,
                generation: None,
                current: true,
            },
        )
        .unwrap();
    raw_revision(&archive, "other", Harness::Pi, b"unrelated");
    let page = archive.revisions("source", 1, None).unwrap();
    assert_eq!(page.items[0].id, second);
    let cursor = page.next_cursor.unwrap();
    assert!(matches!(
        archive.revisions("other", 1, Some(cursor.clone())),
        Err(Error::InvalidInput(_))
    ));
    let page = archive.revisions("source", 1, Some(cursor)).unwrap();
    assert_eq!(page.items[0].id, first);
    assert!(page.next_cursor.is_none());
    assert!(matches!(
        archive.list_sessions(&SessionQuery { limit: 0, ..Default::default() }),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(archive.session("missing"), Err(Error::NotFound(_))));
}

#[test]
fn large_native_events_page_by_bytes_without_dropping_a_record() {
    let (_directory, archive) = archive();
    let text = "x".repeat(3 * 1024 * 1024);
    publish(&archive, "large", Harness::Pi, Some(1), Some(2), &[&text, "tail"]);
    let query = EventQuery { session_id: "large".into(), limit: 500, after: None };
    let first = archive.events(&query).unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].text.len(), text.len());
    let second = archive.events(&EventQuery { after: first.next_cursor, ..query }).unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].text, "tail");
    assert!(second.next_cursor.is_none());
}

#[test]
fn failed_new_normalization_does_not_claim_the_old_projection_covers_latest_bytes() {
    let (_directory, archive) = archive();
    let original = publish(&archive, "source", Harness::Codex, Some(1), Some(2), &["retained"]);
    raw_revision(&archive, "source", Harness::Codex, b"new unsupported native format");
    archive.set_coverage("source", false).unwrap();
    let session = archive.session("source").unwrap();
    assert_eq!(session.revision_id, original);
    assert!(!session.complete);
    let page = archive.list_sessions(&SessionQuery::default()).unwrap();
    assert!(!page.items[0].complete);
    assert_eq!(archive.events(&EventQuery::new("source")).unwrap().items[0].text, "retained");
}

#[test]
fn metadata_pages_bound_bytes_without_truncating_or_skipping_native_titles() {
    let (_directory, archive) = archive();
    let title = "t".repeat(3 * 1024 * 1024);
    for (id, updated) in [("a", Some(2)), ("b", Some(1)), ("c", None)] {
        let revision = raw_revision(&archive, id, Harness::Pi, id.as_bytes());
        archive.begin_projection(id, revision).unwrap();
        archive.append_events(id, revision, vec![event(0, "small event")]).unwrap();
        archive
            .publish_projection(
                id,
                revision,
                &SessionMetadata {
                    native_id: id.into(),
                    project_path: None,
                    title: Some(title.clone()),
                    created_at: Some(1),
                    updated_at: updated,
                    origin: "cli".into(),
                    harness_version: None,
                    parent_session_id: None,
                    model: None,
                },
                true,
            )
            .unwrap();
    }
    let mut query = SessionQuery { limit: 500, ..Default::default() };
    let mut ids = Vec::new();
    loop {
        let page = archive.list_sessions(&query).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].title.as_deref(), Some(title.as_str()));
        ids.push(page.items[0].id.clone());
        query.after = page.next_cursor;
        if query.after.is_none() {
            break;
        }
    }
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn reparsing_identical_bytes_publishes_a_distinct_projection_and_expires_event_cursors() {
    let (_directory, archive) = archive();
    let revision = publish(
        &archive,
        "source",
        Harness::Codex,
        Some(1),
        Some(2),
        &["previous interpretation", "old second event"],
    );
    let original = archive.session("source").unwrap();
    let query = EventQuery { session_id: "source".into(), limit: 1, after: None };
    let first = archive.events(&query).unwrap();
    let cursor = first.next_cursor.unwrap();
    let replacement =
        archive.prepare_projection("source", revision, "parser-v2", true).unwrap().unwrap();
    archive
        .append_projection_events(&replacement, vec![event(0, "corrected interpretation")])
        .unwrap();

    assert_eq!(archive.session("source").unwrap().projection_id, original.projection_id);
    let old_page =
        archive.events(&EventQuery { after: Some(cursor.clone()), ..query.clone() }).unwrap();
    assert_eq!(old_page.items[0].text, "old second event");
    archive
        .publish_projection_attempt(
            &replacement,
            &SessionMetadata {
                native_id: original.native_id,
                project_path: original.project_path,
                title: original.title,
                created_at: original.created_at,
                updated_at: original.updated_at,
                origin: original.origin,
                harness_version: original.harness_version,
                parent_session_id: original.parent_session_id,
                model: original.model,
            },
            true,
        )
        .unwrap();

    let current = archive.session("source").unwrap();
    assert_eq!(current.revision_id, revision);
    assert_ne!(current.projection_id, original.projection_id);
    assert_eq!(current.parser_version, "parser-v2");
    assert!(matches!(
        archive.events(&EventQuery { after: Some(cursor), ..query.clone() }),
        Err(Error::StaleCursor)
    ));
    assert_eq!(archive.events(&query).unwrap().items[0].text, "corrected interpretation");
    assert_eq!(archive.revisions("source", 10, None).unwrap().items.len(), 1);
    let encoded = serde_json::to_value(current).unwrap();
    assert!(encoded["revision_id"].is_string());
    assert!(encoded["projection_id"].is_string());
}
