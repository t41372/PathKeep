//! Responsibilities: define chat transcripts and durable agent journals.
//! Not responsible for: model calls, conversation behavior or canonical history.
//! Failure modes: missing trace columns/indexes, orphan messages/steps/citations,
//! or deleting a conversation leaves its journal behind.
//! Tests: newer_versions_are_refused; the maintenance round trips seed every table.
//! Drops: agent_schema_migrations (version/name/checksum/applied_at), superseded
//! by vault-store's migration ledger. No transcript or journal columns are dropped.

use vault_store::Migration;

pub(super) fn migration() -> Migration {
    super::v1(STATEMENTS)
}

const STATEMENTS: &[&str] = &[
    r"CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  provider_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  message_count INTEGER NOT NULL DEFAULT 0
)",
    r"CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  reasoning TEXT,
  tool_calls_json TEXT,
  status TEXT,
  created_at TEXT NOT NULL
)",
    r"CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
  message_id TEXT,
  provider_id TEXT,
  embedding_provider_id TEXT,
  status TEXT NOT NULL,
  iterations INTEGER NOT NULL DEFAULT 0,
  prompt_tokens INTEGER NOT NULL DEFAULT 0,
  completion_tokens INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
)",
    r"CREATE TABLE agent_steps (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL,
  turn INTEGER NOT NULL,
  kind TEXT NOT NULL,
  tool_name TEXT,
  tool_call_id TEXT,
  payload TEXT NOT NULL,
  created_at TEXT NOT NULL
)",
    r"CREATE TABLE agent_citations (
  run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  history_id INTEGER NOT NULL,
  canonical_url TEXT NOT NULL,
  url TEXT NOT NULL,
  title TEXT,
  visited_at TEXT,
  score REAL,
  profile_id TEXT,
  PRIMARY KEY(run_id, history_id)
)",
    "CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC, id DESC)",
    "CREATE UNIQUE INDEX idx_messages_conversation_seq ON messages(conversation_id, seq)",
    "CREATE INDEX idx_agent_runs_conversation ON agent_runs(conversation_id)",
    "CREATE INDEX idx_agent_runs_message ON agent_runs(conversation_id, message_id, updated_at DESC, id DESC)",
    "CREATE INDEX idx_agent_steps_run_seq ON agent_steps(run_id, seq)",
    "CREATE UNIQUE INDEX idx_agent_steps_tool_call ON agent_steps(run_id, kind, tool_call_id) WHERE tool_call_id IS NOT NULL",
];
