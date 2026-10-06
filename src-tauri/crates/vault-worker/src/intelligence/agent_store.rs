//! Responsibilities: enforce the existing App Lock gate and route chat operations through Vault.
//! Not responsible for: database ownership, schema, model calls or transcript algorithms.
//! Failure modes: a locked request reaches storage, a worker opens a second owner,
//! or conversation cursors are lost at the transport boundary.

use crate::context::load_unlocked_config;
use anyhow::Result;
use vault_core::{
    AgentConversationDetail, AgentConversationListResponse, AgentConversationSummary,
    DeleteAgentConversationResult, ListAgentConversationsRequest, RenameAgentConversationRequest,
    SaveAgentConversationRequest,
};

/// Reads a transcript page from the current unlocked owner.
pub fn list_ai_messages(
    vault: &vault_core::Vault,
    request: &vault_core::ListAgentMessagesRequest,
) -> Result<vault_core::AgentMessagePage> {
    load_unlocked_config(vault.paths())?;
    vault_core::list_conversation_messages(
        vault,
        &request.conversation_id,
        request.limit,
        request.cursor,
    )
}

/// Persists (upsert) one conversation and atomically replaces its message transcript.
pub fn save_ai_conversation(
    vault: &vault_core::Vault,
    request: &SaveAgentConversationRequest,
) -> Result<AgentConversationSummary> {
    let paths = vault.paths();
    // App Lock gate: refuse to touch the agent plane while the session is locked (M-1).
    load_unlocked_config(paths)?;
    vault_core::save_conversation(vault, request)
}

/// Lists the next page of conversation summaries, newest first.
pub fn list_ai_conversations(
    vault: &vault_core::Vault,
    request: &ListAgentConversationsRequest,
) -> Result<AgentConversationListResponse> {
    let paths = vault.paths();
    // App Lock gate: a locked session must not be able to enumerate chat transcripts (M-1).
    load_unlocked_config(paths)?;
    vault_core::list_conversations(vault, request.limit, request.cursor.as_deref())
}

/// Loads one complete transcript for callers that save the entire conversation.
pub fn load_ai_conversation(
    vault: &vault_core::Vault,
    conversation_id: &str,
) -> Result<Option<AgentConversationDetail>> {
    let paths = vault.paths();
    // App Lock gate: a locked session must not be able to read a transcript body (M-1).
    load_unlocked_config(paths)?;
    vault_core::load_conversation(vault, conversation_id)
}

/// Deletes one conversation (cascading its messages); reports whether a row existed.
pub fn delete_ai_conversation(
    vault: &vault_core::Vault,
    conversation_id: &str,
) -> Result<DeleteAgentConversationResult> {
    let paths = vault.paths();
    // App Lock gate: a locked session must not be able to delete a transcript (M-1).
    load_unlocked_config(paths)?;
    let deleted = vault_core::delete_conversation(vault, conversation_id)?;
    Ok(DeleteAgentConversationResult { deleted })
}

/// Renames one conversation, returning the refreshed summary or `None` when unknown.
pub fn rename_ai_conversation(
    vault: &vault_core::Vault,
    request: &RenameAgentConversationRequest,
) -> Result<Option<AgentConversationSummary>> {
    let paths = vault.paths();
    // App Lock gate: a locked session must not be able to mutate a transcript (M-1).
    load_unlocked_config(paths)?;
    vault_core::rename_conversation(vault, &request.id, &request.title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{PROJECT_ROOT_OVERRIDE_ENV, lock_env, restore_env_var};
    use vault_core::AgentMessage;

    /// Drives all five worker entry points against an isolated project root so the thin delegation
    /// (paths resolution + DTO shaping) is covered end to end without a desktop harness.
    ///
    /// Uses the crate-wide env lock so this never races other tests that mutate `CHB_PROJECT_ROOT`.
    #[test]
    fn worker_chat_history_roundtrip_covers_all_entry_points() {
        let vault = vault_core::Vault::open_temp().unwrap();
        let dir = vault.paths().app_root.clone();
        let _guard = lock_env();
        let original = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.as_path());
        }

        let save = save_ai_conversation(
            &vault,
            &SaveAgentConversationRequest {
                id: "conv-worker".to_string(),
                title: None,
                provider_id: Some("llm-local".to_string()),
                messages: vec![
                    AgentMessage {
                        id: "m1".to_string(),
                        role: "user".to_string(),
                        content: "worker question".to_string(),
                        reasoning: None,
                        tool_calls_json: None,
                        status: None,
                        ..Default::default()
                    },
                    AgentMessage {
                        id: "m2".to_string(),
                        role: "assistant".to_string(),
                        content: "worker answer".to_string(),
                        reasoning: Some("r".to_string()),
                        tool_calls_json: None,
                        status: Some("done".to_string()),
                        ..Default::default()
                    },
                ],
            },
        )
        .expect("save");
        assert_eq!(save.title, "worker question");
        assert_eq!(save.message_count, 2);

        let listed = list_ai_conversations(
            &vault,
            &ListAgentConversationsRequest { limit: Some(10), cursor: None },
        )
        .expect("list");
        assert_eq!(listed.conversations.len(), 1);
        assert_eq!(listed.conversations[0].id, "conv-worker");

        let loaded = load_ai_conversation(&vault, "conv-worker").expect("load").expect("present");
        assert_eq!(loaded.messages.len(), 2);

        let renamed = rename_ai_conversation(
            &vault,
            &RenameAgentConversationRequest {
                id: "conv-worker".to_string(),
                title: "renamed".to_string(),
            },
        )
        .expect("rename")
        .expect("present");
        assert_eq!(renamed.title, "renamed");

        let deleted = delete_ai_conversation(&vault, "conv-worker").expect("delete");
        assert!(deleted.deleted);
        assert!(load_ai_conversation(&vault, "conv-worker").expect("load missing").is_none());

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original.as_deref());
    }

    /// While the App Lock session is LOCKED, the agent plane must stay sealed: list/load
    /// (and the mutating ops) refuse with the honest "currently locked" message rather than serving
    /// or touching a transcript (M-1). The agent plane has no SQLCipher key barrier, so this lock
    /// gate is the only thing standing between a locked machine and the chat history.
    #[test]
    fn chat_history_crud_refuses_when_app_session_is_locked() {
        let vault = vault_core::Vault::open_temp().unwrap();
        let dir = vault.paths().app_root.clone();
        let _guard = lock_env();
        let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let keyring_root = dir.as_path().join("test-keyring");
        let original_keyring = std::env::var_os(crate::tests::TEST_KEYRING_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.as_path());
            std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        // Seed one conversation while still UNLOCKED, then enable + lock the session.
        save_ai_conversation(
            &vault,
            &SaveAgentConversationRequest {
                id: "conv-locked".to_string(),
                title: Some("secret".to_string()),
                provider_id: None,
                messages: vec![AgentMessage {
                    id: "m1".to_string(),
                    role: "user".to_string(),
                    content: "sensitive question".to_string(),
                    ..Default::default()
                }],
            },
        )
        .expect("seed conversation while unlocked");

        crate::security::configure_app_lock_passcode(&vault_core::SetAppLockPasscodeRequest {
            passcode: "2468".to_string(),
            recovery_hint: None,
        })
        .expect("configure passcode");
        let mut config = crate::tests::initialized_config();
        config.app_lock.enabled = true;
        crate::app::save_user_config(&config, None).expect("enable app lock");
        let locked = crate::security::lock_app_ui_session(Some("manual")).expect("lock session");
        assert!(locked.locked, "session must be locked for this assertion");

        // Read commands refuse while locked (the `desktop-command-surface.md` contract: AI data read
        // commands MUST refuse when locked).
        let list_error = list_ai_conversations(
            &vault,
            &ListAgentConversationsRequest { limit: Some(10), cursor: None },
        )
        .expect_err("list must refuse while locked");
        assert!(list_error.to_string().contains("currently locked"), "{list_error}");

        let load_error =
            load_ai_conversation(&vault, "conv-locked").expect_err("load must refuse while locked");
        assert!(load_error.to_string().contains("currently locked"), "{load_error}");

        // The mutating ops are sealed too, so a locked machine cannot delete/rename a transcript.
        assert!(
            delete_ai_conversation(&vault, "conv-locked").is_err(),
            "delete must refuse while locked"
        );
        assert!(
            rename_ai_conversation(
                &vault,
                &RenameAgentConversationRequest {
                    id: "conv-locked".to_string(),
                    title: "renamed".to_string(),
                }
            )
            .is_err(),
            "rename must refuse while locked"
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
        restore_env_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }
}
