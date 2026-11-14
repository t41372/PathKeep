//! Responsibilities: keep a published conversation reachable when its current copy is retired.
//! Not responsible for: testing native generation decoding or changing contract assertions.
//! Failure mode: retirement deletes the current row and leaves every surviving copy non-current.
#[allow(dead_code)]
mod support;
use agent_history::{Archive, Harness, SourceRoot, archive::ConversationMark};
use support::native::{claude_main, claude_user};

#[test]
fn retiring_the_current_source_selects_a_remaining_copy_and_preserves_state() {
    let temp = tempfile::tempdir().unwrap();
    let roots = [temp.path().join("first"), temp.path().join("second")];
    let native = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    for root in &roots {
        claude_main(root, native, &[claude_user(native, "u", None, "/synthetic/project", "work")]);
    }
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    let sources = roots
        .into_iter()
        .map(|root| SourceRoot { harness: Harness::ClaudeCode, root })
        .collect::<Vec<_>>();
    archive.backup(&sources, &Default::default()).unwrap();
    let before = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    archive
        .mark_conversation(
            &before.id,
            &ConversationMark { starred: Some(true), ..Default::default() },
            true,
        )
        .unwrap();
    let current = &before.sources.iter().find(|source| source.current).unwrap().source_id;
    assert!(archive.retire_projection(current).unwrap());
    let after = archive.session(&before.id).unwrap();
    assert!(after.starred);
    assert_eq!(after.sources.len(), 1);
    assert!(after.sources[0].current);
    assert!(
        !archive
            .events(&agent_history::query::EventQuery::new(&before.id))
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        archive
            .mark_conversation(
                &before.id,
                &ConversationMark { put_away: Some(true), ..Default::default() },
                true
            )
            .unwrap()
            .put_away
    );
}
