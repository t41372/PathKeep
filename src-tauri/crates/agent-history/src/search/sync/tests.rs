//! Regression written before the fix for the measured 100k-event indexing stall.
//! Turso sorted the remaining conversation for every joined ordinal page. The
//! real encrypted engine must use a bound projection/index seek without a sorter;
//! checking returned rows alone cannot catch this scale-dependent failure.

use crate::{Archive, Artifact, ArtifactKind, Harness, NativeEvent, SessionMetadata, SourceRoot};
use sha2::{Digest, Sha256};
use vault_crypto::KdfParams;
use vault_store::Db;

#[test]
fn event_index_pages_seek_the_published_projection_without_a_sorter() {
    let temporary = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &temporary.path().join("archive"),
        b"synthetic-plan-probe",
        KdfParams { memory_kib: 8, iterations: 1, lanes: 1 },
    )
    .unwrap();
    let source = SourceRoot { harness: Harness::Pi, root: temporary.path().join("pi") };
    let artifact = Artifact {
        path: source.root.join("session.jsonl"),
        relative_path: "session.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let raw = b"{\"type\":\"message\",\"text\":\"plan fixture\"}\n".repeat(512);
    let revision = archive.begin_revision("source-a", &source, &artifact).unwrap();
    archive.append_chunk(revision, 0, raw.clone()).unwrap();
    let revision = archive
        .finish_revision(revision, &hex::encode(Sha256::digest(&raw)), raw.len() as u64)
        .unwrap()
        .revision_id;
    let attempt_revision =
        archive.prepare_projection("source-a", revision, "fixture-v1", false).unwrap().unwrap();
    for start in (0..512).step_by(128) {
        let events = (start..start + 128)
            .map(|ordinal| NativeEvent {
                references: Vec::new(),
                ordinal,
                native_id: None,
                parent_id: None,
                kind: "message".into(),
                role: Some("user".into()),
                timestamp: None,
                text: "plan fixture".into(),
                payload: serde_json::json!({"type":"message","text":"plan fixture"}),
            })
            .collect();
        archive.append_projection_events(&attempt_revision, events).unwrap();
    }
    archive
        .publish_projection_attempt(
            &attempt_revision,
            &SessionMetadata { native_id: "session-a".into(), ..Default::default() },
            true,
        )
        .unwrap();
    archive.store.capture_query_plans(Db::Archive, true).unwrap();
    let source = archive.index_source("source-a").unwrap().unwrap();
    let page = archive.index_event_page(&source, 127).unwrap();
    assert_eq!(page.len(), 128);
    assert_eq!(page[0].ordinal, 128);
    assert_eq!(page[127].ordinal, 255);
    let plans = archive.store.take_query_plans(Db::Archive).unwrap();
    let ephemeral = archive.store.take_ephemeral_tables(Db::Archive).unwrap();
    eprintln!("event indexing plans: {plans:#?}\nephemeral tables: {ephemeral:#?}");
    assert!(ephemeral.is_empty(), "an ordinal page must not materialize temporary tables");
    let events = plans
        .iter()
        .find(|plan| plan.sql.contains("FROM ah_events") || plan.sql.contains("JOIN ah_events"))
        .unwrap();
    assert!(
        events.steps.iter().all(|step| {
            let upper = step.to_ascii_uppercase();
            !upper.contains("SORT") && !upper.contains("TEMP B-TREE")
        }),
        "indexing must not sort the rest of the conversation on every page: {events:#?}"
    );
    assert!(
        events.steps.iter().any(|step| {
            step.contains("ah_events_session_projection")
                && step.contains("session_id=?")
                && step.contains("projection_id=?")
                && step.contains("ordinal>?")
        }),
        "the event page must seek its complete projection/ordinal key: {events:#?}"
    );
}
