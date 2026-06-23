//! Worker-side chat-history persistence (W-AI-3).
//!
//! ## Responsibilities
//! - Resolve the project paths and delegate to `vault_core::agent_store` for save / list / load /
//!   delete / rename of persisted assistant conversations.
//! - Keep the desktop façade independent of `vault-core` function names, returning typed DTOs.
//! - Enforce the App Lock session boundary on every CRUD op: the agent plane is plaintext (no
//!   SQLCipher key barrier), so without a lock gate anyone at the machine could list/read/delete a
//!   user's chat transcripts while PathKeep is locked. Every op funnels through
//!   `load_unlocked_config`, which refuses when the session is locked (M-1).
//!
//! ## Not responsible for
//! - Opening or migrating the agent database (vault-core owns the schema + per-op connection).
//! - Driving the LLM stream or executing tools.
//! - The `ai.enabled` consent gate: chat-history CRUD is an App-Lock-gated data path, independent of
//!   AI consent — a user may read or delete existing transcripts with AI turned off, as long as the
//!   session is unlocked. Only the firing sites that EGRESS or COMPUTE (chat send, re-embed, MCP)
//!   carry the `ensure_ai_capability_enabled` consent gate.
//!
//! ## Performance notes
//! - Pure SQLite work; the desktop layer hops these onto a blocking thread so the chat stream is
//!   never janked by a finalize-time save. The added lock gate is a cheap config read on the same
//!   blocking hop, so it never touches the UI thread.

use crate::context::load_unlocked_config;
use anyhow::Result;
use vault_core::{
    AgentConversationDetail, AgentConversationListResponse, AgentConversationSummary,
    DeleteAgentConversationResult, ListAgentConversationsRequest, RenameAgentConversationRequest,
    SaveAgentConversationRequest,
};

/// Persists (upsert) one conversation and atomically replaces its message transcript.
pub fn save_ai_conversation(
    request: &SaveAgentConversationRequest,
) -> Result<AgentConversationSummary> {
    let paths = vault_core::project_paths()?;
    // App Lock gate: refuse to touch the plaintext agent plane while the session is locked (M-1).
    load_unlocked_config(&paths)?;
    vault_core::save_conversation(&paths, request)
}

/// Lists conversations newest-first, bounded by the (clamped) requested limit.
pub fn list_ai_conversations(
    request: &ListAgentConversationsRequest,
) -> Result<AgentConversationListResponse> {
    let paths = vault_core::project_paths()?;
    // App Lock gate: a locked session must not be able to enumerate chat transcripts (M-1).
    load_unlocked_config(&paths)?;
    let conversations = vault_core::list_conversations(&paths, request.limit)?;
    Ok(AgentConversationListResponse { conversations })
}

/// Loads one conversation plus its bounded message transcript, or `None` when unknown.
pub fn load_ai_conversation(conversation_id: &str) -> Result<Option<AgentConversationDetail>> {
    let paths = vault_core::project_paths()?;
    // App Lock gate: a locked session must not be able to read a transcript body (M-1).
    load_unlocked_config(&paths)?;
    vault_core::load_conversation(&paths, conversation_id)
}

/// Deletes one conversation (cascading its messages); reports whether a row existed.
pub fn delete_ai_conversation(conversation_id: &str) -> Result<DeleteAgentConversationResult> {
    let paths = vault_core::project_paths()?;
    // App Lock gate: a locked session must not be able to delete a transcript (M-1).
    load_unlocked_config(&paths)?;
    let deleted = vault_core::delete_conversation(&paths, conversation_id)?;
    Ok(DeleteAgentConversationResult { deleted })
}

/// Renames one conversation, returning the refreshed summary or `None` when unknown.
pub fn rename_ai_conversation(
    request: &RenameAgentConversationRequest,
) -> Result<Option<AgentConversationSummary>> {
    let paths = vault_core::project_paths()?;
    // App Lock gate: a locked session must not be able to mutate a transcript (M-1).
    load_unlocked_config(&paths)?;
    vault_core::rename_conversation(&paths, &request.id, &request.title)
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
        let dir = tempfile::tempdir().expect("tempdir");
        let _guard = lock_env();
        let original = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }

        let save = save_ai_conversation(&SaveAgentConversationRequest {
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
        })
        .expect("save");
        assert_eq!(save.title, "worker question");
        assert_eq!(save.message_count, 2);

        let listed = list_ai_conversations(&ListAgentConversationsRequest { limit: Some(10) })
            .expect("list");
        assert_eq!(listed.conversations.len(), 1);
        assert_eq!(listed.conversations[0].id, "conv-worker");

        let loaded = load_ai_conversation("conv-worker").expect("load").expect("present");
        assert_eq!(loaded.messages.len(), 2);

        let renamed = rename_ai_conversation(&RenameAgentConversationRequest {
            id: "conv-worker".to_string(),
            title: "renamed".to_string(),
        })
        .expect("rename")
        .expect("present");
        assert_eq!(renamed.title, "renamed");

        let deleted = delete_ai_conversation("conv-worker").expect("delete");
        assert!(deleted.deleted);
        assert!(load_ai_conversation("conv-worker").expect("load missing").is_none());

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original.as_deref());
    }

    /// While the App Lock session is LOCKED, the plaintext agent plane must stay sealed: list/load
    /// (and the mutating ops) refuse with the honest "currently locked" message rather than serving
    /// or touching a transcript (M-1). The agent plane has no SQLCipher key barrier, so this lock
    /// gate is the only thing standing between a locked machine and the chat history.
    #[test]
    fn chat_history_crud_refuses_when_app_session_is_locked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _guard = lock_env();
        let original_root = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        let keyring_root = dir.path().join("test-keyring");
        let original_keyring = std::env::var_os(crate::tests::TEST_KEYRING_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        // Seed one conversation while still UNLOCKED, then enable + lock the session.
        save_ai_conversation(&SaveAgentConversationRequest {
            id: "conv-locked".to_string(),
            title: Some("secret".to_string()),
            provider_id: None,
            messages: vec![AgentMessage {
                id: "m1".to_string(),
                role: "user".to_string(),
                content: "sensitive question".to_string(),
                ..Default::default()
            }],
        })
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
        let list_error = list_ai_conversations(&ListAgentConversationsRequest { limit: Some(10) })
            .expect_err("list must refuse while locked");
        assert!(list_error.to_string().contains("currently locked"), "{list_error}");

        let load_error =
            load_ai_conversation("conv-locked").expect_err("load must refuse while locked");
        assert!(load_error.to_string().contains("currently locked"), "{load_error}");

        // The mutating ops are sealed too, so a locked machine cannot delete/rename a transcript.
        assert!(delete_ai_conversation("conv-locked").is_err(), "delete must refuse while locked");
        assert!(
            rename_ai_conversation(&RenameAgentConversationRequest {
                id: "conv-locked".to_string(),
                title: "renamed".to_string(),
            })
            .is_err(),
            "rename must refuse while locked"
        );

        restore_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_root.as_deref());
        restore_env_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, original_keyring.as_deref());
    }
}
