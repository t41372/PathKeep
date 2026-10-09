//! Responsibilities: persist transcripts and replayable agent journals through Vault.
//! Not responsible for: opening databases, migrations, provider calls or IPC.
//! Failure modes: incomplete transcript replacement, lost timestamps or citations,
//! duplicate replay, missing conversation parents, skipped cursor ties or old chats.
//! Tests: agent_store::tests and tests/agent_store_paging.rs on real Turso stores.

mod journal;
#[cfg(test)]
mod tests;
mod transcript;
mod types;

pub use journal::*;
pub use transcript::*;
pub use types::*;

use crate::{Vault, models::*, utils::now_rfc3339};
use anyhow::Result;
use vault_store::{Db, Priority, Session, params};

const MAX_AGENT_PAGE_SIZE: u32 = 200;
const DEFAULT_AGENT_PAGE_SIZE: u32 = 50;
const AUTO_TITLE_MAX_CHARS: usize = 80;
/// A conversation with no title yet: empty, so the interface shows "Untitled" in the user's
/// language. Earlier versions stored the English placeholder below, which reads as empty too.
const UNTITLED_CONVERSATION_TITLE: &str = "";
const LEGACY_UNTITLED_CONVERSATION_TITLE: &str = "Untitled conversation";

fn page_size(limit: Option<u32>) -> i64 {
    i64::from(limit.filter(|n| *n > 0).unwrap_or(DEFAULT_AGENT_PAGE_SIZE).min(MAX_AGENT_PAGE_SIZE))
}

fn summary(row: &vault_store::Row) -> vault_store::Result<AgentConversationSummary> {
    Ok(AgentConversationSummary {
        id: row.get(0)?,
        title: Some(row.get::<String>(1)?)
            .filter(|title| title != LEGACY_UNTITLED_CONVERSATION_TITLE)
            .unwrap_or_default(),
        provider_id: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        message_count: row.get::<i64>(5)? as usize,
    })
}

fn load_summary(s: &Session<'_>, id: &str) -> Result<Option<AgentConversationSummary>> {
    Ok(s.query(
        "SELECT id, title, provider_id, created_at, updated_at, message_count
         FROM conversations WHERE id=?1",
        [id],
        summary,
    )?
    .pop())
}

fn derive_title_from_messages(messages: &[AgentMessage]) -> String {
    let Some(message) = messages.iter().find(|m| m.role == "user" && !m.content.trim().is_empty())
    else {
        return UNTITLED_CONVERSATION_TITLE.into();
    };
    let collapsed = message.content.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= AUTO_TITLE_MAX_CHARS {
        return collapsed;
    }
    let title: String = collapsed.chars().take(AUTO_TITLE_MAX_CHARS).collect();
    format!("{}…", title.trim_end())
}
