//! Responsibilities: protect source ownership and immutable projection publication.
//! Not responsible for: parser semantics or query presentation.
//! Failure modes, written before implementation: one source publishes another's
//! evidence; a published generation is later mutated; duplicate event ordinals
//! partially commit a batch; a superseded parser publishes over a newer attempt.
//! Reparse failures added before generation implementation: same-byte reparsing
//! changes visible rows before publication; old tokens write into newer attempts;
//! parser upgrades are skipped; cursors cross projection versions; metadata is unbounded.
use agent_history::{
    archive::Archive,
    model::{Artifact, ArtifactKind, Harness, NativeEvent, SessionMetadata, SourceRoot},
};
use sha2::{Digest, Sha256};
use vault_crypto::KdfParams;

fn archive() -> (tempfile::TempDir, Archive) {
    let temp = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &temp.path().join("archive"),
        b"test password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    (temp, archive)
}

fn revision(archive: &Archive, source_id: &str, content: &[u8]) -> i64 {
    let source = SourceRoot { harness: Harness::Pi, root: "/synthetic/pi".into() };
    let artifact = Artifact {
        path: format!("/synthetic/pi/{source_id}.jsonl").into(),
        relative_path: format!("{source_id}.jsonl"),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let revision = archive.begin_revision(source_id, &source, &artifact).unwrap();
    archive.append_chunk(revision, 0, content.to_vec()).unwrap();
    archive
        .finish_revision(revision, &hex::encode(Sha256::digest(content)), content.len() as u64)
        .unwrap()
        .revision_id
}

fn metadata() -> SessionMetadata {
    SessionMetadata {
        native_id: "native-session".into(),
        project_path: None,
        title: None,
        created_at: None,
        updated_at: None,
        origin: "unknown".into(),
        harness_version: None,
        parent_session_id: None,
        model: None,
    }
}

fn event(ordinal: u64) -> NativeEvent {
    NativeEvent {
        ordinal,
        native_id: None,
        parent_id: None,
        kind: "message".into(),
        role: Some("user".into()),
        timestamp: None,
        text: "preserve exactly".into(),
        payload: serde_json::json!({"record": ordinal, "future": [true, null]}),
    }
}

#[test]
fn a_source_cannot_project_another_sources_revision() {
    let (_temp, archive) = archive();
    let left = revision(&archive, "left", b"left evidence");
    revision(&archive, "right", b"right evidence");
    assert!(archive.begin_projection("right", left).is_err());
    assert!(archive.publish_projection("right", left, &metadata(), true).is_err());
    assert_eq!(archive.current_projection("right").unwrap(), None);
}

#[test]
fn parser_identity_fits_the_same_bounded_contract_as_saved_reports() {
    let (_temp, archive) = archive();
    let raw = revision(&archive, "chat", b"native bytes");
    assert!(archive.prepare_projection("chat", raw, &"v".repeat(257), false).is_err());
    assert!(archive.prepare_projection("chat", raw, &"v".repeat(256), false).unwrap().is_some());
}

#[test]
fn a_published_projection_cannot_be_appended_or_published_twice() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"source evidence");
    archive.begin_projection("chat", revision).unwrap();
    archive.append_events("chat", revision, vec![event(0)]).unwrap();
    archive.publish_projection("chat", revision, &metadata(), true).unwrap();
    let generation = archive.projection_generation().unwrap();
    assert!(archive.append_events("chat", revision, vec![event(1)]).is_err());
    assert!(archive.publish_projection("chat", revision, &metadata(), true).is_err());
    assert_eq!(archive.projection_generation().unwrap(), generation);
}

#[test]
fn duplicate_ordinals_roll_back_the_entire_batch() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"source evidence");
    archive.begin_projection("chat", revision).unwrap();
    assert!(archive.append_events("chat", revision, vec![event(0), event(0)]).is_err());
    // If the first record in the failed batch leaked, its unique ordinal prevents
    // this correct retry. The real transaction must discard both attempted rows.
    archive.append_events("chat", revision, vec![event(0), event(1)]).unwrap();
    archive.publish_projection("chat", revision, &metadata(), true).unwrap();
    assert_eq!(archive.current_projection("chat").unwrap(), Some(revision));
}

#[test]
fn superseded_projection_work_cannot_publish() {
    let (_temp, archive) = archive();
    let old = revision(&archive, "chat", b"old evidence");
    let newer = revision(&archive, "chat", b"newer evidence");
    archive.begin_projection("chat", old).unwrap();
    archive.append_events("chat", old, vec![event(0)]).unwrap();
    archive.begin_projection("chat", newer).unwrap();
    assert!(archive.publish_projection("chat", old, &metadata(), true).is_err());
    archive.append_events("chat", newer, vec![event(0)]).unwrap();
    archive.publish_projection("chat", newer, &metadata(), true).unwrap();
    assert_eq!(archive.current_projection("chat").unwrap(), Some(newer));
}

#[test]
fn bounded_cleanup_preserves_current_projection_and_all_raw_revisions() {
    let (_temp, archive) = archive();
    let old = revision(&archive, "chat", b"old evidence");
    archive.begin_projection("chat", old).unwrap();
    archive.append_events("chat", old, vec![event(0)]).unwrap();
    archive.publish_projection("chat", old, &metadata(), true).unwrap();
    let newer = revision(&archive, "chat", b"newer evidence");
    archive.begin_projection("chat", newer).unwrap();
    archive.append_events("chat", newer, vec![event(0)]).unwrap();
    archive.publish_projection("chat", newer, &metadata(), true).unwrap();
    let mut reclaimed = 0;
    loop {
        let progress = archive.prune_obsolete_projection_batch(1).unwrap();
        if progress == 0 {
            break;
        }
        reclaimed += progress;
    }
    assert!(reclaimed > 0);
    assert_eq!(archive.current_projection("chat").unwrap(), Some(newer));
    assert!(archive.raw_reader(old).is_ok());
    assert!(archive.raw_reader(newer).is_ok());
}

#[test]
fn a_failed_newer_parse_keeps_prior_results_but_marks_their_coverage_incomplete() {
    let (_temp, archive) = archive();
    let old = revision(&archive, "chat", b"old evidence");
    archive.begin_projection("chat", old).unwrap();
    archive.append_events("chat", old, vec![event(0)]).unwrap();
    archive.publish_projection("chat", old, &metadata(), true).unwrap();
    archive.synchronize_search().unwrap();
    let query = agent_history::search::SearchQuery {
        text: "preserve".into(),
        harness: None,
        project_path: None,
        limit: 10,
    };
    assert!(archive.search(&query).unwrap().complete);
    let newer = revision(&archive, "chat", b"uninterpretable newer evidence");
    archive.set_coverage("chat", false).unwrap();
    let partial = archive.search(&query).unwrap();
    assert!(!partial.complete);
    assert_eq!(partial.total, 1);
    assert_eq!(archive.current_projection("chat").unwrap(), Some(old));
    assert!(archive.set_coverage("chat", true).is_err());
    assert!(!archive.search(&query).unwrap().complete);
    archive.begin_projection("chat", newer).unwrap();
    archive.append_events("chat", newer, vec![event(0)]).unwrap();
    archive.publish_projection("chat", newer, &metadata(), true).unwrap();
    archive.synchronize_search().unwrap();
    assert!(archive.search(&query).unwrap().complete);
}

fn publish_attempt(
    archive: &Archive,
    revision: i64,
    parser: &str,
    text: &str,
) -> agent_history::archive::ProjectionAttempt {
    let attempt = archive.prepare_projection("chat", revision, parser, false).unwrap().unwrap();
    let mut events = vec![event(0), event(1)];
    for event in &mut events {
        event.text = text.into();
    }
    archive.append_projection_events(&attempt, events).unwrap();
    archive.publish_projection_attempt(&attempt, &metadata(), true).unwrap();
    attempt
}

#[test]
fn same_byte_reparse_keeps_old_rows_visible_and_invalidates_old_event_cursor_on_publish() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"unchanged source bytes");
    let original = publish_attempt(&archive, revision, "fixture-v1", "original interpretation");
    let mut page_query = agent_history::query::EventQuery::new("chat");
    page_query.limit = 1;
    let page = archive.events(&page_query).unwrap();
    let cursor = page.next_cursor.clone().expect("two events need continuation");
    assert_eq!(page.items[0].text, "original interpretation");
    let replacement =
        archive.prepare_projection("chat", revision, "fixture-v1", true).unwrap().unwrap();
    assert_ne!(replacement.id, original.id);
    let mut improved = event(0);
    improved.text = "corrected interpretation".into();
    archive.append_projection_events(&replacement, vec![improved]).unwrap();
    assert_eq!(archive.current_projection_id("chat").unwrap(), Some(original.id));
    assert_eq!(archive.events(&page_query).unwrap().items[0].text, "original interpretation");
    archive.publish_projection_attempt(&replacement, &metadata(), true).unwrap();
    assert_eq!(archive.current_projection("chat").unwrap(), Some(revision));
    assert_eq!(archive.current_projection_id("chat").unwrap(), Some(replacement.id));
    assert_eq!(archive.events(&page_query).unwrap().items[0].text, "corrected interpretation");
    page_query.after = Some(cursor);
    assert!(matches!(archive.events(&page_query), Err(agent_history::Error::StaleCursor)));
}

#[test]
fn a_same_revision_attempt_cannot_write_or_publish_after_a_newer_attempt_starts() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"one raw revision");
    let old = archive.prepare_projection("chat", revision, "fixture-v1", false).unwrap().unwrap();
    let newer = archive.prepare_projection("chat", revision, "fixture-v2", false).unwrap().unwrap();
    assert!(archive.append_projection_events(&old, vec![event(0)]).is_err());
    assert!(archive.publish_projection_attempt(&old, &metadata(), true).is_err());
    archive.append_projection_events(&newer, vec![event(0)]).unwrap();
    archive.publish_projection_attempt(&newer, &metadata(), true).unwrap();
    assert_eq!(archive.current_projection_id("chat").unwrap(), Some(newer.id));
}

#[test]
fn parser_identity_changes_reparse_while_an_identical_complete_version_is_idempotent() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"same source bytes");
    let first = publish_attempt(&archive, revision, "fixture-v1", "v1");
    assert!(archive.prepare_projection("chat", revision, "fixture-v1", false).unwrap().is_none());
    let upgraded =
        archive.prepare_projection("chat", revision, "fixture-v2", false).unwrap().unwrap();
    assert_ne!(first.id, upgraded.id);
    archive.append_projection_events(&upgraded, vec![event(0)]).unwrap();
    archive.publish_projection_attempt(&upgraded, &metadata(), false).unwrap();
    assert!(archive.prepare_projection("chat", revision, "fixture-v2", false).unwrap().is_none());
    assert!(archive.prepare_projection("chat", revision, "fixture-v2", true).unwrap().is_some());
}

#[test]
fn retiring_a_generation_removes_search_and_invalidates_staging_without_removing_raw_bytes() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"raw bytes must remain");
    publish_attempt(&archive, revision, "fixture-v1", "retirementneedle");
    archive.synchronize_search().unwrap();
    let query = agent_history::search::SearchQuery {
        text: "retirementneedle".into(),
        harness: None,
        project_path: None,
        limit: 10,
    };
    assert_eq!(archive.search(&query).unwrap().total, 2);
    let staging =
        archive.prepare_projection("chat", revision, "fixture-v2", true).unwrap().unwrap();
    archive.append_projection_events(&staging, vec![event(0)]).unwrap();
    assert!(archive.retire_projection("chat").unwrap());
    assert_eq!(archive.current_projection_id("chat").unwrap(), None);
    assert!(archive.append_projection_events(&staging, vec![event(1)]).is_err());
    assert!(archive.publish_projection_attempt(&staging, &metadata(), true).is_err());
    assert!(matches!(archive.search(&query), Err(agent_history::Error::IndexBehind)));
    archive.synchronize_search().unwrap();
    assert_eq!(archive.search(&query).unwrap().total, 0);
    while archive.prune_obsolete_projection_batch(1).unwrap() > 0 {}
    assert!(archive.raw_reader(revision).is_ok());
    assert!(!archive.retire_projection("chat").unwrap());
}

#[test]
fn oversized_metadata_is_rejected_before_replacing_the_current_projection() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"unchanged source bytes");
    let original = publish_attempt(&archive, revision, "fixture-v1", "original");
    let replacement =
        archive.prepare_projection("chat", revision, "fixture-v2", false).unwrap().unwrap();
    archive.append_projection_events(&replacement, vec![event(0)]).unwrap();
    let mut oversized = metadata();
    oversized.title = Some("x".repeat(64 * 1024 * 1024));
    assert!(archive.publish_projection_attempt(&replacement, &oversized, true).is_err());
    assert_eq!(archive.current_projection_id("chat").unwrap(), Some(original.id));
    archive.publish_projection_attempt(&replacement, &metadata(), true).unwrap();
    assert_eq!(archive.current_projection_id("chat").unwrap(), Some(replacement.id));
}

#[test]
fn an_attempt_cannot_publish_after_a_newer_raw_revision_becomes_the_source_head() {
    let (_temp, archive) = archive();
    let old = revision(&archive, "chat", b"old source bytes");
    let attempt = archive.prepare_projection("chat", old, "fixture-v1", false).unwrap().unwrap();
    archive.append_projection_events(&attempt, vec![event(0)]).unwrap();
    let newer = revision(&archive, "chat", b"newer source bytes");
    assert!(archive.publish_projection_attempt(&attempt, &metadata(), true).is_err());
    assert_eq!(archive.current_projection_id("chat").unwrap(), None);
    let latest = archive.prepare_projection("chat", newer, "fixture-v1", false).unwrap().unwrap();
    archive.append_projection_events(&latest, vec![event(0)]).unwrap();
    archive.publish_projection_attempt(&latest, &metadata(), true).unwrap();
    assert_eq!(archive.current_projection_id("chat").unwrap(), Some(latest.id));
}

#[test]
fn parser_and_reference_coverage_publish_atomically_and_can_be_repaired() {
    let (_temp, archive) = archive();
    let revision = revision(&archive, "chat", b"native source with missing attachment");
    let attempt =
        archive.prepare_projection("chat", revision, "fixture-v1", false).unwrap().unwrap();
    archive.append_projection_events(&attempt, vec![event(0)]).unwrap();
    archive.publish_projection_attempt_with_coverage(&attempt, &metadata(), true, false).unwrap();
    archive.synchronize_search().unwrap();
    let query = agent_history::search::SearchQuery {
        text: "preserve".into(),
        harness: None,
        project_path: None,
        limit: 10,
    };
    assert!(!archive.search(&query).unwrap().complete);
    let generation = archive.projection_generation().unwrap();
    archive.update_projection_coverage("chat", attempt.id, true, true).unwrap();
    assert!(archive.search(&query).unwrap().complete);
    assert_eq!(archive.projection_generation().unwrap(), generation);
}

#[test]
fn coverage_repair_rejects_superseded_projections_and_newer_raw_heads() {
    let (_temp, archive) = archive();
    let old_raw = revision(&archive, "chat", b"same native bytes");
    let old = publish_attempt(&archive, old_raw, "fixture-v1", "old view");
    let next = archive.prepare_projection("chat", old_raw, "fixture-v2", false).unwrap().unwrap();
    archive.append_projection_events(&next, vec![event(0)]).unwrap();
    archive.publish_projection_attempt_with_coverage(&next, &metadata(), true, false).unwrap();
    assert!(archive.update_projection_coverage("chat", old.id, true, true).is_err());
    archive.synchronize_search().unwrap();
    let query = agent_history::search::SearchQuery {
        text: "preserve".into(),
        harness: None,
        project_path: None,
        limit: 10,
    };
    assert!(!archive.search(&query).unwrap().complete);
    revision(&archive, "chat", b"new raw bytes without interpretation");
    assert!(archive.update_projection_coverage("chat", next.id, true, true).is_err());
    assert!(!archive.search(&query).unwrap().complete);
}

#[test]
fn combined_coverage_cannot_claim_complete_when_parser_coverage_is_incomplete() {
    let (_temp, archive) = archive();
    let raw = revision(&archive, "chat", b"native bytes");
    let attempt = archive.prepare_projection("chat", raw, "fixture-v1", false).unwrap().unwrap();
    archive.append_projection_events(&attempt, vec![event(0)]).unwrap();
    assert!(
        archive
            .publish_projection_attempt_with_coverage(&attempt, &metadata(), false, true)
            .is_err()
    );
    assert_eq!(archive.current_projection_id("chat").unwrap(), None);
    archive.publish_projection_attempt_with_coverage(&attempt, &metadata(), false, false).unwrap();
    assert!(archive.set_coverage("chat", true).is_err());
    assert!(archive.update_projection_coverage("chat", attempt.id, false, true).is_err());
}
