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

// A common prefix can grow down either branch, regardless of its old representative.
#[test]
fn growing_copy_follows_its_other_branch() {
    let temp = tempfile::tempdir().unwrap();
    let thread = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let first = claude_user(thread, "u", None, "/synthetic/project", "common prefix");
    let mut branches = Vec::new();
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    for branch in ["left", "right"] {
        let root = SourceRoot { harness: Harness::ClaudeCode, root: temp.path().join(branch) };
        let next = claude_user(thread, branch, Some("u"), "/synthetic/project", branch);
        let tail = claude_user(thread, "last", Some(branch), "/synthetic/project", "end");
        claude_main(&root.root, thread, &[first.clone(), next.clone(), tail]);
        archive.backup(&[root], &Default::default()).unwrap();
        let id = archive
            .list_sources(&Default::default())
            .unwrap()
            .items
            .into_iter()
            .find(|s| std::path::Path::new(&s.root).ends_with(branch))
            .unwrap()
            .id;
        branches.push((id, next));
    }
    let copy = SourceRoot { harness: Harness::ClaudeCode, root: temp.path().join("copy") };
    claude_main(&copy.root, thread, std::slice::from_ref(&first));
    archive.backup(std::slice::from_ref(&copy), &Default::default()).unwrap();
    let before = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    assert_eq!(before.sources.len(), 3);
    let member = before.sources.iter().find(|s| s.relation == "copy").unwrap();
    let previous = member.of_source_id.as_ref().unwrap();
    let (other, next) = branches.into_iter().find(|(id, _)| id != previous).unwrap();
    claude_main(&copy.root, thread, &[first, next]);
    archive.backup(&[copy], &Default::default()).unwrap();
    let after = archive.session(&before.id).unwrap();
    let grown = after.sources.iter().find(|s| s.source_id == member.source_id).unwrap();
    assert_eq!(grown.of_source_id.as_ref(), Some(&other));
    assert_eq!(grown.relation, "copy");
    assert!(!grown.current);
}
