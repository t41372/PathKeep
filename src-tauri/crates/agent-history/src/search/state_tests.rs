//! Responsibilities: force a user-state change between real search filter and hit reads.
//! Not responsible for: mocking SQL, Tantivy or native content interpretation.
//! Failure mode: a search mixes the old hidden set with new hit flags after a mark.
use crate::archive::ConversationMark;
use crate::{Archive, Artifact, ArtifactKind, Harness, NativeEvent, SessionMetadata, SourceRoot};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, sync::Arc};

thread_local! {
    static AFTER_FILTER: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

pub(super) fn after_filter() {
    let callback = AFTER_FILTER.with(|slot| slot.borrow_mut().take());
    if let Some(callback) = callback {
        callback();
    }
}

#[test]
fn search_cannot_mix_old_trash_filter_with_new_hit_state() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Arc::new(Archive::create_unencrypted(temp.path()).unwrap());
    let source = SourceRoot { harness: Harness::Codex, root: "/synthetic/codex".into() };
    let artifact = Artifact {
        path: "/synthetic/codex/chat.jsonl".into(),
        relative_path: "chat.jsonl".into(),
        kind: ArtifactKind::Transcript,
        project_hint: None,
        generation: None,
        current: true,
    };
    let bytes = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"state-race\"}}\n";
    let raw = archive.begin_revision("source", &source, &artifact).unwrap();
    archive.append_chunk(raw, 0, bytes.to_vec()).unwrap();
    let raw = archive
        .finish_revision(raw, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id;
    let attempt = archive.prepare_projection("source", raw, "synthetic", false).unwrap().unwrap();
    archive
        .append_projection_events(
            &attempt,
            vec![NativeEvent {
                references: Vec::new(),
                ordinal: 0,
                native_id: None,
                parent_id: None,
                kind: "message".into(),
                role: Some("user".into()),
                timestamp: None,
                text: "statevisibilityneedle".into(),
                payload: serde_json::json!({}),
            }],
        )
        .unwrap();
    archive
        .publish_projection_attempt(
            &attempt,
            &SessionMetadata { native_id: "state-race".into(), ..Default::default() },
            true,
        )
        .unwrap();
    archive.synchronize_search().unwrap();
    let id = archive.list_sessions(&Default::default()).unwrap().items.remove(0).id;
    let writer = archive.clone();
    AFTER_FILTER.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            writer
                .mark_conversation(
                    &id,
                    &ConversationMark {
                        trash: Some(true),
                        put_away: Some(true),
                        ..Default::default()
                    },
                    true,
                )
                .unwrap();
        }))
    });
    let page = archive
        .search(&crate::SearchQuery {
            text: "statevisibilityneedle".into(),
            harness: None,
            project_path: None,
            limit: 10,
        })
        .unwrap();
    if page.items.is_empty() {
        assert_eq!(page.total, 0);
    } else {
        assert_eq!(page.total, 1);
        assert!(!page.items[0].put_away, "only the complete old state may still include the hit");
    }
}
