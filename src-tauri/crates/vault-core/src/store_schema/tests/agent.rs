//! Responsibilities: pin the agent catalog and complete row fixtures.
//! Not responsible for: producing migrations or exercising production query code.
//! Failure modes: missing columns/indexes, lossy rows and weakened foreign keys.
use super::{Fixture, ForeignKey};

pub(super) const TABLES: &[&str] = &[
    "__pk_migrations",
    "agent_citations",
    "agent_runs",
    "agent_steps",
    "conversations",
    "messages",
];
pub(super) const INDEXES: &[&str] = &[
    "idx_agent_runs_conversation",
    "idx_agent_runs_message",
    "idx_agent_steps_run_seq",
    "idx_agent_steps_tool_call",
    "idx_conversations_updated",
    "idx_messages_conversation_seq",
    "sqlite_autoindex_agent_citations_1",
    "sqlite_autoindex_agent_runs_1",
    "sqlite_autoindex_agent_steps_1",
    "sqlite_autoindex_conversations_1",
    "sqlite_autoindex_messages_1",
];
pub(super) const ROWS: &[Fixture] = &[
    Fixture {
        table: "conversations",
        columns: "id, title, provider_id, created_at, updated_at, message_count",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 1",
        updated: "title = 'changed'",
    },
    Fixture {
        table: "messages",
        columns: "id, conversation_id, seq, role, content, reasoning, tool_calls_json, status, created_at",
        values: "'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
        updated: "seq = 2",
    },
    Fixture {
        table: "agent_runs",
        columns: "id, conversation_id, message_id, provider_id, embedding_provider_id, status, iterations, prompt_tokens, completion_tokens, error, created_at, updated_at",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 1, 'sample', 'sample', 'sample'",
        updated: "message_id = 'changed'",
    },
    Fixture {
        table: "agent_steps",
        columns: "id, run_id, seq, turn, kind, tool_name, tool_call_id, payload, created_at",
        values: "'sample', 'sample', 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample'",
        updated: "seq = 2",
    },
    Fixture {
        table: "agent_citations",
        columns: "run_id, history_id, canonical_url, url, title, visited_at, score",
        values: "'sample', 1, 'sample', 'sample', 'sample', 'sample', 1.5",
        updated: "canonical_url = 'changed'",
    },
];
pub(super) const FOREIGN_KEYS: &[ForeignKey] = &[
    ForeignKey { table: "messages", column: "conversation_id", missing: "'missing'" },
    ForeignKey { table: "agent_runs", column: "conversation_id", missing: "'missing'" },
    ForeignKey { table: "agent_steps", column: "run_id", missing: "'missing'" },
    ForeignKey { table: "agent_citations", column: "run_id", missing: "'missing'" },
];
