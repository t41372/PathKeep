//! Worker-side chat-history persistence (W-AI-3).
//!
//! ## Responsibilities
//! - Resolve the project paths and delegate to `vault_core::agent_store` for save / list / load /
//!   delete / rename of persisted assistant conversations.
//! - Keep the desktop façade independent of `vault-core` function names, returning typed DTOs.
//!
//! ## Not responsible for
//! - Opening or migrating the agent database (vault-core owns the schema + per-op connection).
//! - Driving the LLM stream or executing tools.
//!
//! ## Performance notes
//! - Pure SQLite work; the desktop layer hops these onto a blocking thread so the chat stream is
//!   never janked by a finalize-time save.

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
    vault_core::save_conversation(&paths, request)
}

/// Lists conversations newest-first, bounded by the (clamped) requested limit.
pub fn list_ai_conversations(
    request: &ListAgentConversationsRequest,
) -> Result<AgentConversationListResponse> {
    let paths = vault_core::project_paths()?;
    let conversations = vault_core::list_conversations(&paths, request.limit)?;
    Ok(AgentConversationListResponse { conversations })
}

/// Loads one conversation plus its bounded message transcript, or `None` when unknown.
pub fn load_ai_conversation(conversation_id: &str) -> Result<Option<AgentConversationDetail>> {
    let paths = vault_core::project_paths()?;
    vault_core::load_conversation(&paths, conversation_id)
}

/// Deletes one conversation (cascading its messages); reports whether a row existed.
pub fn delete_ai_conversation(conversation_id: &str) -> Result<DeleteAgentConversationResult> {
    let paths = vault_core::project_paths()?;
    let deleted = vault_core::delete_conversation(&paths, conversation_id)?;
    Ok(DeleteAgentConversationResult { deleted })
}

/// Renames one conversation, returning the refreshed summary or `None` when unknown.
pub fn rename_ai_conversation(
    request: &RenameAgentConversationRequest,
) -> Result<Option<AgentConversationSummary>> {
    let paths = vault_core::project_paths()?;
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
                },
                AgentMessage {
                    id: "m2".to_string(),
                    role: "assistant".to_string(),
                    content: "worker answer".to_string(),
                    reasoning: Some("r".to_string()),
                    tool_calls_json: None,
                    status: Some("done".to_string()),
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
}
