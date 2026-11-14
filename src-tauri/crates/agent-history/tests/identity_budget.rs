//! Responsibilities: keep conversation identity independent of a projection's record budget.
//! Not responsible for: recovering unreadable source bytes or changing native field rules.
//! Failure mode: skipping a large header gives a temporary source key and orphans user state.
#[allow(dead_code)]
mod support;
use agent_history::archive::ConversationMark;
use agent_history::{Archive, Harness, SourceRoot, capture::BackupOptions};
use support::native::{codex_meta, codex_rollout, codex_user};

#[test]
fn a_larger_parser_budget_does_not_rename_a_known_native_conversation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    let native = "33333333-3333-4333-8333-333333333333";
    let mut header = codex_meta(native, "/synthetic/project");
    header["payload"]["base_instructions"] = serde_json::json!({"text":"x".repeat(4096)});
    codex_rollout(
        &root,
        "2026-11-10T12-00-00",
        native,
        &[header, codex_user("budgetidentityneedle")],
    );
    let archive = Archive::create_unencrypted(&temp.path().join("archive")).unwrap();
    archive
        .backup(
            &[SourceRoot { harness: Harness::Codex, root }],
            &BackupOptions { max_record_bytes: 512, ..Default::default() },
        )
        .unwrap();
    let before = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    archive
        .mark_conversation(
            &before.id,
            &ConversationMark { starred: Some(true), ..Default::default() },
            true,
        )
        .unwrap();
    archive.reparse_source(&before.sources[0].source_id, &BackupOptions::default()).unwrap();
    let after = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    assert_eq!(before.id, after.id);
    assert_eq!(after.native_id, native);
    assert!(after.starred);
}
