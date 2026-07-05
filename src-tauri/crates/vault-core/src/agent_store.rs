//! Agent sidecar store: chat-history persistence over `derived/agent.sqlite`.
//!
//! ## Responsibilities
//! - Own the agent-plane SQLite database (`derived/agent.sqlite`) and its INDEPENDENT,
//!   extensible migration framework (`agent_schema_migrations` table + ordered migrations +
//!   ensure-on-open + checksum guard), mirroring the intelligence plane's own migration system.
//! - Provide pure-SQLite CRUD for chat conversations and messages: upsert a conversation,
//!   atomically replace its message transcript, list (bounded, newest-first), load one, delete,
//!   rename, and auto-title from the first user message.
//!
//! ## Not responsible for
//! - Canonical archive facts (this plane is rebuildable/discardable derived trace).
//! - Driving the LLM stream, tool execution, or any model call.
//! - Tauri command naming / IPC payload design (the desktop layer owns that).
//! - Agent run/step journaling — W-AI-7 will ADD those tables here via a new migration; this
//!   module ships only the chat tables now, but the framework is built to extend.
//!
//! ## Dependencies
//! - `rusqlite` with the archive plane's PRAGMA conventions (WAL, foreign_keys, busy_timeout,
//!   per-operation `Connection`). No SQLCipher key: the agent plane is plaintext like the other
//!   derived sidecars (`02-architecture-decisions.md` §A).
//! - `config::ProjectPaths` for `agent_database_path` (added in W-AI-0).
//!
//! ## Performance notes
//! - Built for the 14.4M baseline by being bounded and indexed: list reads cap rows and never
//!   load message bodies; transcript reads are indexed on `(conversation_id, seq)`; a save
//!   replaces messages in a single transaction. There is no full-table scan on any hot path.

use crate::{
    config::{ProjectPaths, ensure_paths},
    models::{
        AgentCitation, AgentConversationDetail, AgentConversationSummary, AgentMessage, AgentUsage,
        SaveAgentConversationRequest,
    },
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration as StdDuration;

/// Hard ceiling on conversations returned by one list read, regardless of the requested limit.
///
/// Keeps the explorer's first paint bounded even if a caller passes a huge limit; the explorer
/// shows a recency-ordered list, not the full archive of every chat ever held.
const MAX_CONVERSATION_LIST_LIMIT: u32 = 200;

/// Default conversation list cap when the caller does not specify one.
const DEFAULT_CONVERSATION_LIST_LIMIT: u32 = 50;

/// Maximum characters kept when deriving an auto-title from the first user message.
const AUTO_TITLE_MAX_CHARS: usize = 80;

/// Fallback title used when a conversation has no user message to derive one from.
const UNTITLED_CONVERSATION_TITLE: &str = "Untitled conversation";

/// Bootstrap DDL for the agent-plane migration ledger.
///
/// `checksum` pins a hash of the SQL that defined each migration so a divergent build is caught
/// (the guard in [`run_agent_migrations`]) rather than silently running mismatched schema against
/// old data.
const AGENT_SCHEMA_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS agent_schema_migrations (
  version     INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  checksum    TEXT NOT NULL,
  applied_at  TEXT NOT NULL
);
"#;

/// Baseline chat-history schema (migration v1).
///
/// `conversations.message_count` is a denormalized cache kept in sync by [`save_conversation`] so
/// the explorer list never counts message rows per conversation. `messages.seq` is the stable
/// ordinal within a conversation; the `(conversation_id, seq)` index drives ordered transcript
/// reads without a sort scan. `ON DELETE CASCADE` lets deleting a conversation drop its messages
/// in one statement (foreign_keys is enabled per connection).
const AGENT_CHAT_BASELINE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS conversations (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  provider_id    TEXT,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL,
  message_count  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_conversations_updated
  ON conversations(updated_at DESC);

CREATE TABLE IF NOT EXISTS messages (
  id               TEXT PRIMARY KEY,
  conversation_id  TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  seq              INTEGER NOT NULL,
  role             TEXT NOT NULL,
  content          TEXT NOT NULL,
  reasoning        TEXT,
  tool_calls_json  TEXT,
  status           TEXT,
  created_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_conversation_seq
  ON messages(conversation_id, seq);
"#;

/// Defines one ordered agent-plane schema migration.
///
/// `apply` runs the structural change against the in-flight transaction; `sql` is the canonical SQL
/// text whose SHA-256 (via [`migration_checksum`]) is recorded in the ledger so an accidental
/// in-place edit of an applied migration is detected. W-AI-7 adds `agent_runs`/`agent_steps` by
/// appending a new [`AgentMigrationSpec`] with the next version.
#[derive(Clone, Copy)]
struct AgentMigrationSpec {
    version: i64,
    name: &'static str,
    /// The exact SQL that defines this migration; the ledger checksum is the hash of this string,
    /// so editing the SQL after the migration is applied trips the drift guard.
    sql: &'static str,
    apply: fn(&Transaction<'_>, &'static str) -> Result<()>,
}

impl AgentMigrationSpec {
    /// Computes the stable checksum recorded in the ledger from the migration's actual SQL text.
    fn checksum(&self) -> String {
        migration_checksum(self.version, self.name, self.sql)
    }
}

/// Derives the ledger checksum for one migration from its version, name, and exact SQL text.
///
/// Hashing the real SQL (not a hand-typed label) means any edit to an applied migration's
/// definition changes the recorded fingerprint, so the drift guard in [`run_agent_migrations`]
/// catches it instead of silently running mismatched schema against existing data.
fn migration_checksum(version: i64, name: &str, sql: &str) -> String {
    sha256_hex(format!("v{version}:{name}:{sql}").as_bytes())
}

/// Agent run/step/citation trace schema (migration v2, W-AI-7).
///
/// A PARALLEL journal keyed by `run_id` — NOT part of the chat transcript that [`save_conversation`]
/// wholesale-replaces — so durable agent traces coexist with the v1 chat tables untouched.
/// `agent_runs` is the run header (status + budget accounting); `agent_steps` is the append-only,
/// monotonically-sequenced journal that makes resume = replay (the harness journals every model/tool
/// output BEFORE observing it, 02 §F); `agent_citations` pins evidence rows so a citation survives
/// any later context compaction. Indices keep run lookup and ordered replay off any full scan; every
/// child cascades on run delete. `agent_steps.tool_call_id` is the per-tool idempotency key
/// component (with `run_id`) so a replayed/duplicate tool result is detectable.
const AGENT_RUN_TRACE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS agent_runs (
  id                    TEXT PRIMARY KEY,
  conversation_id       TEXT REFERENCES conversations(id) ON DELETE CASCADE,
  message_id            TEXT,
  provider_id           TEXT,
  embedding_provider_id TEXT,
  status                TEXT NOT NULL,
  iterations            INTEGER NOT NULL DEFAULT 0,
  prompt_tokens         INTEGER NOT NULL DEFAULT 0,
  completion_tokens     INTEGER NOT NULL DEFAULT 0,
  error                 TEXT,
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_runs_conversation
  ON agent_runs(conversation_id);

CREATE TABLE IF NOT EXISTS agent_steps (
  id            TEXT PRIMARY KEY,
  run_id        TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  seq           INTEGER NOT NULL,
  turn          INTEGER NOT NULL,
  kind          TEXT NOT NULL,
  tool_name     TEXT,
  tool_call_id  TEXT,
  payload       TEXT NOT NULL,
  created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_steps_run_seq
  ON agent_steps(run_id, seq);

CREATE TABLE IF NOT EXISTS agent_citations (
  run_id        TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  history_id    INTEGER NOT NULL,
  canonical_url TEXT NOT NULL,
  url           TEXT NOT NULL,
  title         TEXT,
  visited_at    TEXT,
  score         REAL,
  PRIMARY KEY(run_id, history_id)
);
"#;

/// The ordered agent-plane migrations. Append-only: never edit or reorder an applied entry; add a
/// new spec with the next version (this is how W-AI-7 extends the agent plane).
const AGENT_MIGRATIONS: &[AgentMigrationSpec] = &[
    AgentMigrationSpec {
        version: 1,
        name: "chat-history-baseline",
        sql: AGENT_CHAT_BASELINE_SQL,
        apply: apply_chat_history_baseline_migration,
    },
    AgentMigrationSpec {
        version: 2,
        name: "agent-run-trace",
        sql: AGENT_RUN_TRACE_SQL,
        apply: apply_run_trace_migration,
    },
];

/// Installs the baseline chat-history schema on a freshly attached agent database.
fn apply_chat_history_baseline_migration(tx: &Transaction<'_>, sql: &'static str) -> Result<()> {
    tx.execute_batch(sql)?;
    Ok(())
}

/// Installs the agent run/step/citation trace tables (migration v2).
fn apply_run_trace_migration(tx: &Transaction<'_>, sql: &'static str) -> Result<()> {
    tx.execute_batch(sql)?;
    Ok(())
}

/// Opens the agent sidecar connection with the shared SQLite plane conventions.
///
/// Plaintext (no cipher key): the agent plane is a rebuildable derived sidecar, never canonical
/// truth. Uses WAL + `foreign_keys` (so cascade deletes work) + a busy timeout, matching the
/// intelligence/archive planes; schema is ensured on every open so the caller can treat the
/// database as ready immediately.
pub fn open_agent_connection(paths: &ProjectPaths) -> Result<Connection> {
    ensure_paths(paths)?;
    let mut connection = Connection::open(&paths.agent_database_path)
        .with_context(|| format!("opening {}", paths.agent_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    ensure_agent_schema(&mut connection)?;
    Ok(connection)
}

/// Loads the set of already-applied agent migration versions, bootstrapping the ledger first.
fn load_applied_agent_migrations(connection: &Connection) -> Result<BTreeSet<i64>> {
    connection.execute_batch(AGENT_SCHEMA_MIGRATIONS_SQL)?;
    let mut statement =
        connection.prepare("SELECT version FROM agent_schema_migrations ORDER BY version ASC")?;
    statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .map_err(Into::into)
}

/// Applies every pending agent-plane migration in version order, guarding recorded checksums.
///
/// For an already-applied version it verifies the recorded checksum still matches the current
/// spec; a mismatch means an applied migration's definition was edited in place (forbidden) and
/// is surfaced as an error rather than running drifted schema against existing data.
///
/// Each pending migration's `apply` plus its `agent_schema_migrations` INSERT run inside ONE
/// transaction (SQLite DDL is transactional), so a crash mid-migration can never leave a partial
/// structural change without its ledger row — important because W-AI-7 appends non-idempotent
/// migrations that must not half-apply.
fn run_agent_migrations(connection: &mut Connection) -> Result<()> {
    apply_agent_migrations(connection, AGENT_MIGRATIONS)
}

/// Core migration runner over an explicit migration list, so tests can drive the real atomic
/// apply/record path with an injected (e.g. deliberately failing) spec without duplicating logic.
fn apply_agent_migrations(
    connection: &mut Connection,
    migrations: &[AgentMigrationSpec],
) -> Result<()> {
    let applied = load_applied_agent_migrations(connection)?;
    for migration in migrations {
        let expected_checksum = migration.checksum();
        if applied.contains(&migration.version) {
            let recorded: Option<String> = connection
                .query_row(
                    "SELECT checksum FROM agent_schema_migrations WHERE version = ?1",
                    params![migration.version],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(recorded) = recorded {
                anyhow::ensure!(
                    recorded == expected_checksum,
                    "agent migration {} checksum mismatch: recorded `{}` but expected `{}`; an \
                     applied migration's definition was changed in place",
                    migration.version,
                    recorded,
                    expected_checksum
                );
            }
            continue;
        }
        let tx = connection.transaction()?;
        (migration.apply)(&tx, migration.sql)?;
        tx.execute(
            "INSERT INTO agent_schema_migrations (version, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, expected_checksum, now_rfc3339()],
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Ensures the agent plane is migrated before any read/write touches it. Idempotent.
pub fn ensure_agent_schema(connection: &mut Connection) -> Result<()> {
    run_agent_migrations(connection)
}

/// Derives a human title from the first non-empty user message, bounded in length.
///
/// Returns the fallback title when no user message has content. Truncation is on a `char`
/// boundary (never mid-codepoint) and collapses interior whitespace so a multi-line prompt yields
/// a single tidy line; trailing whitespace from truncation is trimmed.
fn derive_title_from_messages(messages: &[AgentMessage]) -> String {
    let first_user = messages
        .iter()
        .find(|message| message.role == "user" && !message.content.trim().is_empty());
    let Some(message) = first_user else {
        return UNTITLED_CONVERSATION_TITLE.to_string();
    };
    let collapsed = message.content.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= AUTO_TITLE_MAX_CHARS {
        return collapsed;
    }
    let truncated: String = collapsed.chars().take(AUTO_TITLE_MAX_CHARS).collect();
    format!("{}…", truncated.trim_end())
}

/// Resolves the title to persist: an explicit non-blank title wins, else an auto-derived one.
fn resolve_title(explicit: Option<&str>, messages: &[AgentMessage]) -> String {
    match explicit.map(str::trim) {
        Some(title) if !title.is_empty() => title.to_string(),
        _ => derive_title_from_messages(messages),
    }
}

/// Upserts a conversation and atomically replaces its full message transcript.
///
/// Persist-on-finalize contract: the caller sends the whole message list per save, so this
/// deletes the prior messages and re-inserts the supplied ones in one transaction (the
/// denormalized `message_count` is refreshed to match). The conversation `created_at` is preserved
/// on update so a re-saved conversation keeps its original birth time while `updated_at` advances;
/// the explorer orders by `updated_at`.
///
/// Per-message `created_at` is also preserved across re-saves: each message id's original stamp is
/// carried forward (looked up before the wholesale replace), and only a genuinely new message id is
/// stamped with `now`. Re-saving the same transcript therefore never restamps history — a chat is
/// append-mostly, so existing turns keep their real creation time. Returns the persisted summary so
/// the caller can refresh its list without a second round trip.
pub fn save_conversation(
    paths: &ProjectPaths,
    request: &SaveAgentConversationRequest,
) -> Result<AgentConversationSummary> {
    anyhow::ensure!(!request.id.trim().is_empty(), "a conversation id is required");
    let mut connection = open_agent_connection(paths)?;
    let now = now_rfc3339();
    let title = resolve_title(request.title.as_deref(), &request.messages);
    let message_count = request.messages.len();

    let tx = connection.transaction()?;
    // Preserve the original creation time across re-saves; first save uses `now`.
    let existing_created_at: Option<String> = tx
        .query_row(
            "SELECT created_at FROM conversations WHERE id = ?1",
            params![request.id],
            |row| row.get(0),
        )
        .optional()?;
    let created_at = existing_created_at.unwrap_or_else(|| now.clone());

    // Snapshot each prior message's original created_at so the wholesale replace below can carry it
    // forward instead of restamping every turn to `now` (which would make per-message timestamps
    // meaningless after the first re-save).
    let mut prior_message_created_at: HashMap<String, String> = HashMap::new();
    {
        let mut prior =
            tx.prepare("SELECT id, created_at FROM messages WHERE conversation_id = ?1")?;
        let rows = prior.query_map(params![request.id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, created_at) = row?;
            prior_message_created_at.insert(id, created_at);
        }
    }

    tx.execute(
        "INSERT INTO conversations (id, title, provider_id, created_at, updated_at, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           title = excluded.title,
           provider_id = excluded.provider_id,
           updated_at = excluded.updated_at,
           message_count = excluded.message_count",
        params![request.id, title, request.provider_id, created_at, now, message_count as i64],
    )?;

    // Replace the transcript wholesale: simplest correct contract for persist-on-finalize. Each
    // message keeps its original created_at (carried from the prior snapshot) so a re-save preserves
    // per-turn timestamps; only a never-seen message id is stamped `now`.
    tx.execute("DELETE FROM messages WHERE conversation_id = ?1", params![request.id])?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO messages
               (id, conversation_id, seq, role, content, reasoning, tool_calls_json, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for (seq, message) in request.messages.iter().enumerate() {
            let message_created_at =
                prior_message_created_at.get(&message.id).map_or(now.as_str(), String::as_str);
            insert.execute(params![
                message.id,
                request.id,
                seq as i64,
                message.role,
                message.content,
                message.reasoning,
                message.tool_calls_json,
                message.status,
                message_created_at,
            ])?;
        }
    }
    tx.commit()?;

    Ok(AgentConversationSummary {
        id: request.id.clone(),
        title,
        provider_id: request.provider_id.clone(),
        created_at,
        updated_at: now,
        message_count,
    })
}

/// Lists conversations newest-first, bounded by the requested (clamped) limit.
///
/// Returns summaries only — no message bodies — so the explorer's first paint stays light even on
/// a large history. A limit of zero or `None` falls back to the default cap; any limit above the
/// hard ceiling is clamped. Reads run against the `updated_at` index.
pub fn list_conversations(
    paths: &ProjectPaths,
    limit: Option<u32>,
) -> Result<Vec<AgentConversationSummary>> {
    let effective_limit = match limit {
        Some(0) | None => DEFAULT_CONVERSATION_LIST_LIMIT,
        Some(value) => value.min(MAX_CONVERSATION_LIST_LIMIT),
    };
    let connection = open_agent_connection(paths)?;
    let mut statement = connection.prepare(
        "SELECT id, title, provider_id, created_at, updated_at, message_count
         FROM conversations
         ORDER BY updated_at DESC, id DESC
         LIMIT ?1",
    )?;
    let conversations = statement
        .query_map(params![effective_limit], |row| {
            Ok(AgentConversationSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                provider_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                message_count: row.get::<_, i64>(5)?.max(0) as usize,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(conversations)
}

/// Loads one conversation plus its FULL message transcript in `seq` order, with each assistant
/// turn's durable agent trace (citations + token usage) RECONSTRUCTED from the run journal.
///
/// Returns `None` when the id is unknown. The transcript is read ascending from the
/// `(conversation_id, seq)` index and is intentionally NOT capped: a single chat is bounded by real
/// usage, not the 14.4M archive baseline, so loading every message is the only correct behavior.
///
/// Capping the load was a data-loss trap: the front end rehydrates the loaded view and re-saves it
/// on the next turn, which (with a wholesale replace) would have permanently dropped every message
/// beyond the cap. Loading all messages keeps re-save lossless.
///
/// W-AI-7 WU-7 — durable trace on reopen: reasoning + tool calls + status already round-trip on the
/// message row, but the run's CITATIONS and USAGE live in the parallel `agent_runs`/`agent_citations`
/// journal (keyed by run_id; `agent_runs.message_id` links a run to its assistant message). So this
/// also reads, scoped to THIS conversation, each message's latest run usage and its pinned citations,
/// and stitches them onto the assistant turns — so a reopened conversation shows the SAME evidence
/// rows + star keys + token footer the live turn did. Both reads are bounded to this conversation's
/// runs/citations (a message has one run; a chat has few turns) via the `idx_agent_runs_conversation`
/// index — never a corpus scan or an N+1 across the 14.4M baseline.
pub fn load_conversation(
    paths: &ProjectPaths,
    conversation_id: &str,
) -> Result<Option<AgentConversationDetail>> {
    let connection = open_agent_connection(paths)?;
    let summary: Option<AgentConversationSummary> = connection
        .query_row(
            "SELECT id, title, provider_id, created_at, updated_at, message_count
             FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| {
                Ok(AgentConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    message_count: row.get::<_, i64>(5)?.max(0) as usize,
                })
            },
        )
        .optional()?;
    let Some(summary) = summary else {
        return Ok(None);
    };

    // Read the entire transcript in ascending seq order for replay; no tail-cap (see fn doc).
    let mut statement = connection.prepare(
        "SELECT id, role, content, reasoning, tool_calls_json, status
         FROM messages
         WHERE conversation_id = ?1
         ORDER BY seq ASC",
    )?;
    let mut messages = statement
        .query_map(params![conversation_id], |row| {
            Ok(AgentMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                reasoning: row.get(3)?,
                tool_calls_json: row.get(4)?,
                status: row.get(5)?,
                citations: Vec::new(),
                usage: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Reconstruct each message's run trace (usage + citations) from the parallel journal and stitch
    // it onto the matching assistant turn (drops the borrow of `connection` held by `statement`).
    drop(statement);
    let runs = load_conversation_message_runs(&connection, conversation_id)?;
    if !runs.is_empty() {
        let mut citations =
            load_conversation_message_citations(&connection, conversation_id, &runs)?;
        for message in &mut messages {
            if let Some(run) = runs.get(&message.id) {
                if run.prompt_tokens > 0 || run.completion_tokens > 0 {
                    message.usage = Some(AgentUsage {
                        prompt_tokens: run.prompt_tokens.max(0) as u64,
                        completion_tokens: run.completion_tokens.max(0) as u64,
                    });
                }
                if let Some(rows) = citations.remove(&run.run_id) {
                    message.citations = rows;
                }
            }
        }
    }

    Ok(Some(AgentConversationDetail { summary, messages }))
}

/// The single most-recent run that answered one assistant message, with its token tally.
struct MessageRun {
    run_id: String,
    prompt_tokens: i64,
    completion_tokens: i64,
}

/// Resolves, for each `message_id` in this conversation, the most-recent run that answered it.
///
/// A message normally has exactly one run, but a re-begin or a re-asked turn could leave more than
/// one `agent_runs` row pointing at the same `message_id`; the latest (`updated_at` then `id`) is the
/// honest trace to show on reopen, so this keeps that one. Scoped to the conversation via the
/// `idx_agent_runs_conversation` index — bounded by the chat's turn count, never the corpus.
fn load_conversation_message_runs(
    connection: &Connection,
    conversation_id: &str,
) -> Result<HashMap<String, MessageRun>> {
    let mut statement = connection.prepare(
        "SELECT message_id, id, prompt_tokens, completion_tokens, updated_at
         FROM agent_runs
         WHERE conversation_id = ?1 AND message_id IS NOT NULL
         ORDER BY message_id ASC, updated_at ASC, id ASC",
    )?;
    let mut runs: HashMap<String, MessageRun> = HashMap::new();
    let rows = statement.query_map(params![conversation_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    // Ascending order means the LAST row written per message_id wins (the most recent run).
    for row in rows {
        let (message_id, run_id, prompt_tokens, completion_tokens) = row?;
        runs.insert(message_id, MessageRun { run_id, prompt_tokens, completion_tokens });
    }
    Ok(runs)
}

/// Loads the pinned citations for this conversation's runs, grouped by `run_id`, ordered by
/// `history_id`.
///
/// One bounded read over THIS conversation's citations (joined through its runs via the
/// `idx_agent_runs_conversation` index), so the caller stitches each run's evidence onto its message
/// without a per-message round trip (no N+1). Only the runs the caller resolved as the latest per
/// message (`runs`) are surfaced; a citation for a superseded run of the same conversation is dropped
/// client-side so a reopened turn shows exactly the run it now reflects.
fn load_conversation_message_citations(
    connection: &Connection,
    conversation_id: &str,
    runs: &HashMap<String, MessageRun>,
) -> Result<HashMap<String, Vec<AgentCitation>>> {
    let wanted: BTreeSet<&str> = runs.values().map(|run| run.run_id.as_str()).collect();
    let mut statement = connection.prepare(
        "SELECT r.id, c.history_id, c.canonical_url, c.url, c.title, c.visited_at, c.score
         FROM agent_citations c
         JOIN agent_runs r ON c.run_id = r.id
         WHERE r.conversation_id = ?1 AND r.message_id IS NOT NULL
         ORDER BY r.id ASC, c.history_id ASC",
    )?;
    let mut grouped: HashMap<String, Vec<AgentCitation>> = HashMap::new();
    let rows = statement.query_map(params![conversation_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            AgentCitation {
                history_id: row.get(1)?,
                profile_id: String::new(),
                canonical_url: Some(row.get::<_, String>(2)?),
                url: row.get(3)?,
                title: row.get(4)?,
                visited_at: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                score: row.get(6)?,
            },
        ))
    })?;
    for row in rows {
        let (run_id, citation) = row?;
        if wanted.contains(run_id.as_str()) {
            grouped.entry(run_id).or_default().push(citation);
        }
    }
    Ok(grouped)
}

/// Deletes one conversation and (via cascade) its messages. Returns whether a row existed.
pub fn delete_conversation(paths: &ProjectPaths, conversation_id: &str) -> Result<bool> {
    let connection = open_agent_connection(paths)?;
    let affected =
        connection.execute("DELETE FROM conversations WHERE id = ?1", params![conversation_id])?;
    Ok(affected > 0)
}

/// Renames one conversation, bumping `updated_at`. Returns the refreshed summary, or `None` when
/// the id is unknown. A blank title is rejected so the explorer never shows an empty row.
pub fn rename_conversation(
    paths: &ProjectPaths,
    conversation_id: &str,
    title: &str,
) -> Result<Option<AgentConversationSummary>> {
    let trimmed = title.trim();
    anyhow::ensure!(!trimmed.is_empty(), "a conversation title cannot be blank");
    let connection = open_agent_connection(paths)?;
    let now = now_rfc3339();
    let affected = connection.execute(
        "UPDATE conversations SET title = ?2, updated_at = ?3 WHERE id = ?1",
        params![conversation_id, trimmed, now],
    )?;
    if affected == 0 {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT id, title, provider_id, created_at, updated_at, message_count
             FROM conversations WHERE id = ?1",
            params![conversation_id],
            |row| {
                Ok(AgentConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    message_count: row.get::<_, i64>(5)?.max(0) as usize,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Agent run trace journal (W-AI-7, migration v2).
//
// A durable, observable, append-only journal that is INDEPENDENT of the chat transcript: the
// harness journals every model/tool output here BEFORE emitting it, so a crash mid-run leaves an
// interrupted partial trace and a resume replays the journal instead of re-calling the model
// (02 §F). Keyed by run_id; never touched by `save_conversation`'s wholesale replace.
// ---------------------------------------------------------------------------

/// Terminal/in-flight status of one agent run, persisted as a stable lowercase tag.
///
/// `Running` is the in-flight state set at [`begin_agent_run`]; the harness moves it to exactly one
/// terminal state at the end. A crash leaves it `Running` forever, which is the honest "interrupted"
/// signal trace replay shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunStatus {
    /// The run is in flight (or was interrupted by a crash and never finalized).
    Running,
    /// The run finished and produced a final answer.
    Completed,
    /// The run was cooperatively cancelled.
    Cancelled,
    /// The run ended in a terminal error.
    Failed,
}

impl AgentRunStatus {
    /// Returns the stable lowercase tag stored in `agent_runs.status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Parses a stored status tag back into the enum (unknown tags read as `Running`/interrupted).
    fn from_tag(tag: &str) -> Self {
        match tag {
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            _ => Self::Running,
        }
    }
}

/// Parameters that open one agent run header.
///
/// A struct (not a long positional arg list) so the harness call site stays readable and a new
/// optional field is additive. `conversation_id`/`message_id` link the run to the chat turn it
/// answers; both providers are recorded for observability + replay context.
#[derive(Debug, Clone, Default)]
pub struct BeginAgentRun {
    pub id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
}

/// One journaled step in an agent run, appended BEFORE the harness observes/emits it.
///
/// `seq` is the monotonic 0-based ordinal within the run (the replay order); `turn` is the model
/// turn the step belongs to. `kind` is a stable tag (`assistant-turn` / `tool-result` / ...).
/// `payload` is opaque JSON owned by the caller, so the journal schema does not couple to the
/// evolving tool/turn shapes. `tool_call_id` (+ `run_id`) is the idempotency key for tool-result
/// steps so a duplicate/replayed result is detectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStepRecord {
    pub id: String,
    pub seq: i64,
    pub turn: i64,
    pub kind: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub payload: String,
}

/// A new step to append; `seq` is assigned by [`append_agent_step`] (the next ordinal for the run).
#[derive(Debug, Clone)]
pub struct AppendAgentStep {
    pub run_id: String,
    pub turn: i64,
    pub kind: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub payload: String,
}

/// One pinned citation row for a run (evidence that must survive context compaction).
///
/// `canonical_url` is the W-STAR star key, so WU-6 can star a cited page directly from a trace.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentCitationRecord {
    pub history_id: i64,
    pub canonical_url: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: Option<String>,
    pub score: Option<f32>,
}

/// A fully loaded run trace for replay/inspection: the header plus its ordered steps + citations.
#[derive(Debug, Clone)]
pub struct AgentRunTrace {
    pub id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub status: AgentRunStatus,
    pub iterations: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub error: Option<String>,
    pub steps: Vec<AgentStepRecord>,
    pub citations: Vec<AgentCitationRecord>,
}

/// Opens one agent run header in the `Running` state. Idempotent on the run id (re-begin updates).
///
/// The header is the durable anchor every step/citation cascades from. `updated_at` advances on
/// every later write so the explorer can order runs by recency. Re-begin (same id) is treated as a
/// fresh open of that run (status reset to `Running`, accounting zeroed) so a deterministic test or
/// a deliberate restart starts from a clean header without orphaning the prior child rows (callers
/// that need a clean slate delete first via the conversation cascade).
///
/// Self-heals the parent FK: a tools-enabled run is opened BEFORE the front end persists its
/// conversation (the FE saves lazily on finalize, after the turn), so the first run of a brand-new
/// conversation would otherwise INSERT against a missing `conversations` row and trip
/// `FOREIGN KEY constraint failed`, killing the whole run before any model call. When
/// `conversation_id` is `Some`, this idempotently upserts a STUB parent conversation (placeholder
/// title, `created_at`/`updated_at = now`, `message_count = 0`) so the FK is satisfied; the FE's
/// later [`save_conversation`] overwrites title/transcript/`updated_at` and preserves the row's
/// `created_at`, so the stub is invisible once the real save lands. The stub upsert and the
/// `agent_runs` insert run in ONE transaction so a run header is never opened without its parent.
pub fn begin_agent_run(paths: &ProjectPaths, run: &BeginAgentRun) -> Result<()> {
    anyhow::ensure!(!run.id.trim().is_empty(), "an agent run id is required");
    let mut connection = open_agent_connection(paths)?;
    let now = now_rfc3339();
    let tx = connection.transaction()?;

    // Satisfy the agent_runs → conversations FK for a not-yet-persisted conversation: insert a stub
    // parent only when the row is absent (ON CONFLICT DO NOTHING), so an already-saved conversation
    // keeps its real title/created_at and a later wholesale save still overwrites the stub.
    if let Some(conversation_id) = run.conversation_id.as_deref() {
        tx.execute(
            "INSERT INTO conversations (id, title, provider_id, created_at, updated_at, message_count)
             VALUES (?1, ?2, ?3, ?4, ?4, 0)
             ON CONFLICT(id) DO NOTHING",
            params![
                conversation_id,
                UNTITLED_CONVERSATION_TITLE,
                run.provider_id,
                now,
            ],
        )?;
    }

    tx.execute(
        "INSERT INTO agent_runs
           (id, conversation_id, message_id, provider_id, embedding_provider_id, status,
            iterations, prompt_tokens, completion_tokens, error, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, NULL, ?7, ?7)
         ON CONFLICT(id) DO UPDATE SET
           conversation_id = excluded.conversation_id,
           message_id = excluded.message_id,
           provider_id = excluded.provider_id,
           embedding_provider_id = excluded.embedding_provider_id,
           status = excluded.status,
           iterations = 0,
           prompt_tokens = 0,
           completion_tokens = 0,
           error = NULL,
           updated_at = excluded.updated_at",
        params![
            run.id,
            run.conversation_id,
            run.message_id,
            run.provider_id,
            run.embedding_provider_id,
            AgentRunStatus::Running.as_str(),
            now,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// Appends one journal step, assigning the next monotonic `seq` for the run. Returns the `seq`.
///
/// JOURNAL-BEFORE-OBSERVE (02 §F): the harness calls this for a model/tool output and only emits
/// the corresponding stream chunk AFTER it commits, so a crash can never have shown the user (or
/// resumed past) a step that is not durable. The `(run_id, seq)` assignment + insert run in ONE
/// transaction so two concurrent appends cannot collide on a seq. A tool-result step whose
/// `(run_id, tool_call_id)` already exists is a duplicate (idempotency key) and is skipped — the
/// existing seq is returned — so a retried/replayed tool result never double-journals.
pub fn append_agent_step(paths: &ProjectPaths, step: &AppendAgentStep) -> Result<i64> {
    anyhow::ensure!(!step.run_id.trim().is_empty(), "an agent run id is required");
    let mut connection = open_agent_connection(paths)?;
    let now = now_rfc3339();
    let tx = connection.transaction()?;

    // Idempotency: a tool result with the same (run_id, tool_call_id) is a duplicate. Return the
    // already-journaled seq instead of inserting a second row.
    if let Some(tool_call_id) = step.tool_call_id.as_deref() {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT seq FROM agent_steps
                 WHERE run_id = ?1 AND kind = ?2 AND tool_call_id = ?3",
                params![step.run_id, step.kind, tool_call_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(seq) = existing {
            tx.commit()?;
            return Ok(seq);
        }
    }

    let next_seq: i64 = tx.query_row(
        "SELECT COALESCE(MAX(seq) + 1, 0) FROM agent_steps WHERE run_id = ?1",
        params![step.run_id],
        |row| row.get(0),
    )?;
    let step_id = format!("{}:{next_seq}", step.run_id);
    tx.execute(
        "INSERT INTO agent_steps
           (id, run_id, seq, turn, kind, tool_name, tool_call_id, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            step_id,
            step.run_id,
            next_seq,
            step.turn,
            step.kind,
            step.tool_name,
            step.tool_call_id,
            step.payload,
            now,
        ],
    )?;
    tx.execute("UPDATE agent_runs SET updated_at = ?2 WHERE id = ?1", params![step.run_id, now])?;
    tx.commit()?;
    Ok(next_seq)
}

/// Records the terminal status + budget accounting for one run.
///
/// Called exactly once at the end of a run. `iterations`/token counts are the final budget tallies
/// the harness accumulated; `error` is `Some` only for the `Failed` path. Advances `updated_at`.
pub fn finalize_agent_run(
    paths: &ProjectPaths,
    run_id: &str,
    status: AgentRunStatus,
    iterations: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    error: Option<&str>,
) -> Result<()> {
    let connection = open_agent_connection(paths)?;
    let now = now_rfc3339();
    connection.execute(
        "UPDATE agent_runs
         SET status = ?2, iterations = ?3, prompt_tokens = ?4, completion_tokens = ?5,
             error = ?6, updated_at = ?7
         WHERE id = ?1",
        params![run_id, status.as_str(), iterations, prompt_tokens, completion_tokens, error, now],
    )?;
    Ok(())
}

/// Pins the evidence citations for one run, replacing any prior set for that run.
///
/// Idempotent per run: a re-record (e.g. a resumed run re-emitting its citations) replaces the
/// prior rows rather than accumulating duplicates. `canonical_url` is the W-STAR key so a later
/// star action can address the cited page directly. Runs in one transaction so the set is never
/// observed half-replaced.
pub fn record_agent_citations(
    paths: &ProjectPaths,
    run_id: &str,
    citations: &[AgentCitationRecord],
) -> Result<()> {
    let mut connection = open_agent_connection(paths)?;
    let tx = connection.transaction()?;
    tx.execute("DELETE FROM agent_citations WHERE run_id = ?1", params![run_id])?;
    {
        let mut insert = tx.prepare(
            "INSERT OR REPLACE INTO agent_citations
               (run_id, history_id, canonical_url, url, title, visited_at, score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for citation in citations {
            insert.execute(params![
                run_id,
                citation.history_id,
                citation.canonical_url,
                citation.url,
                citation.title,
                citation.visited_at,
                citation.score,
            ])?;
        }
    }
    tx.execute(
        "UPDATE agent_runs SET updated_at = ?2 WHERE id = ?1",
        params![run_id, now_rfc3339()],
    )?;
    tx.commit()?;
    Ok(())
}

/// Loads one run's full trace (header + ordered steps + citations) for replay/inspection.
///
/// One `agent_runs` header row as read by [`load_agent_run`]:
/// `(conversation_id, message_id, provider_id, embedding_provider_id, status, iterations,
/// prompt_tokens, completion_tokens, error)`.
type AgentRunHeaderRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    i64,
    i64,
    i64,
    Option<String>,
);

/// Steps are read ascending by the `(run_id, seq)` index (the replay order); citations by
/// `history_id`. Returns `None` for an unknown run id. This is the read side of resume = replay:
/// the worker rebuilds the run's observable timeline from the journal, NEVER by re-calling the
/// model.
pub fn load_agent_run(paths: &ProjectPaths, run_id: &str) -> Result<Option<AgentRunTrace>> {
    let connection = open_agent_connection(paths)?;
    let header: Option<AgentRunHeaderRow> = connection
        .query_row(
            "SELECT conversation_id, message_id, provider_id, embedding_provider_id, status,
                    iterations, prompt_tokens, completion_tokens, error
             FROM agent_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        conversation_id,
        message_id,
        provider_id,
        embedding_provider_id,
        status,
        iterations,
        prompt_tokens,
        completion_tokens,
        error,
    )) = header
    else {
        return Ok(None);
    };

    let mut step_stmt = connection.prepare(
        "SELECT id, seq, turn, kind, tool_name, tool_call_id, payload
         FROM agent_steps WHERE run_id = ?1 ORDER BY seq ASC",
    )?;
    let steps = step_stmt
        .query_map(params![run_id], |row| {
            Ok(AgentStepRecord {
                id: row.get(0)?,
                seq: row.get(1)?,
                turn: row.get(2)?,
                kind: row.get(3)?,
                tool_name: row.get(4)?,
                tool_call_id: row.get(5)?,
                payload: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut citation_stmt = connection.prepare(
        "SELECT history_id, canonical_url, url, title, visited_at, score
         FROM agent_citations WHERE run_id = ?1 ORDER BY history_id ASC",
    )?;
    let citations = citation_stmt
        .query_map(params![run_id], |row| {
            Ok(AgentCitationRecord {
                history_id: row.get(0)?,
                canonical_url: row.get(1)?,
                url: row.get(2)?,
                title: row.get(3)?,
                visited_at: row.get(4)?,
                score: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(AgentRunTrace {
        id: run_id.to_string(),
        conversation_id,
        message_id,
        provider_id,
        embedding_provider_id,
        status: AgentRunStatus::from_tag(&status),
        iterations,
        prompt_tokens,
        completion_tokens,
        error,
        steps,
        citations,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;

    fn paths_for_test() -> (tempfile::TempDir, ProjectPaths) {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        (dir, paths)
    }

    fn user_message(id: &str, content: &str) -> AgentMessage {
        AgentMessage {
            id: id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            reasoning: None,
            tool_calls_json: None,
            status: None,
            ..Default::default()
        }
    }

    fn assistant_message(id: &str, content: &str) -> AgentMessage {
        AgentMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
            content: content.to_string(),
            reasoning: Some("thinking".to_string()),
            tool_calls_json: Some(
                "[{\"id\":\"t1\",\"name\":\"search_bm25\",\"arguments\":\"{}\"}]".to_string(),
            ),
            status: Some("done".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn agent_schema_migrations_records_versioned_migrations() {
        let mut connection = Connection::open_in_memory().expect("in memory sqlite");
        connection.pragma_update(None, "foreign_keys", true).expect("foreign keys");
        ensure_agent_schema(&mut connection).expect("ensure agent schema");
        ensure_agent_schema(&mut connection).expect("ensure agent schema twice");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM agent_schema_migrations", [], |row| row.get(0))
            .expect("migration count");
        assert_eq!(migration_count as usize, AGENT_MIGRATIONS.len());

        let recorded_checksum: String = connection
            .query_row(
                "SELECT checksum FROM agent_schema_migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .expect("recorded checksum");
        assert_eq!(recorded_checksum, AGENT_MIGRATIONS[0].checksum());

        let has_conversations: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='conversations'",
                [],
                |row| row.get(0),
            )
            .expect("conversations table");
        let has_messages: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |row| row.get(0),
            )
            .expect("messages table");
        let has_index: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_messages_conversation_seq'",
                [],
                |row| row.get(0),
            )
            .expect("seq index");
        assert_eq!(has_conversations, 1);
        assert_eq!(has_messages, 1);
        assert_eq!(has_index, 1);
    }

    #[test]
    fn migration_guard_rejects_recorded_checksum_drift() {
        let mut connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_agent_schema(&mut connection).expect("ensure agent schema");
        // Simulate an in-place edit of an applied migration by corrupting its recorded checksum.
        connection
            .execute(
                "UPDATE agent_schema_migrations SET checksum = 'drifted' WHERE version = 1",
                [],
            )
            .expect("corrupt checksum");
        let error = ensure_agent_schema(&mut connection).expect_err("drift must fail");
        assert!(error.to_string().contains("checksum mismatch"), "unexpected error: {error}");
    }

    #[test]
    fn migration_checksum_is_derived_from_sql_and_changes_when_sql_changes() {
        // The recorded checksum must be the hash of the migration's actual SQL, not a hand-typed
        // label — so editing the SQL changes the fingerprint and trips the drift guard.
        let spec = &AGENT_MIGRATIONS[0];
        assert_eq!(
            spec.checksum(),
            migration_checksum(spec.version, spec.name, spec.sql),
            "checksum must be the hash of (version, name, sql)"
        );
        // It is a SHA-256 hex digest, not the old literal label.
        assert_eq!(spec.checksum().len(), 64);
        assert!(spec.checksum().chars().all(|c| c.is_ascii_hexdigit()));

        // Any edit to the migration SQL yields a different checksum (the guard's whole point).
        let edited = migration_checksum(
            spec.version,
            spec.name,
            "CREATE TABLE conversations (id TEXT PRIMARY KEY); -- edited",
        );
        assert_ne!(spec.checksum(), edited, "an SQL edit must change the checksum");
    }

    #[test]
    fn migration_apply_and_ledger_insert_are_one_transaction() {
        // Drives the REAL `apply_agent_migrations` runner with an injected failing spec: if a
        // migration's `apply` fails, its ledger row must NOT be recorded (atomicity), so a retry
        // re-runs the whole migration rather than skipping a half-applied one.
        const FAILING_MIGRATIONS: &[AgentMigrationSpec] = &[AgentMigrationSpec {
            version: 99,
            name: "intentionally-broken",
            sql: "THIS IS NOT VALID SQL;",
            apply: apply_chat_history_baseline_migration,
        }];

        let mut connection = Connection::open_in_memory().expect("in memory sqlite");
        ensure_agent_schema(&mut connection).expect("baseline schema");

        // Re-running the already-applied baseline through the real runner exercises the
        // already-applied / checksum-verify branch without re-inserting a row.
        apply_agent_migrations(&mut connection, AGENT_MIGRATIONS).expect("idempotent re-run");

        let error = apply_agent_migrations(&mut connection, FAILING_MIGRATIONS)
            .expect_err("broken migration fails");
        assert!(!error.to_string().is_empty());

        // The failed migration left NO ledger row: the transaction rolled back as a unit.
        let recorded: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM agent_schema_migrations WHERE version = 99",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(recorded, 0, "a failed migration must not record a ledger row");
    }

    #[test]
    fn open_agent_connection_bootstraps_database_file() {
        let (_dir, paths) = paths_for_test();
        let connection = open_agent_connection(&paths).expect("open agent connection");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .expect("query conversations");
        assert_eq!(count, 0);
        assert!(paths.agent_database_path.exists());
    }

    #[test]
    fn save_then_load_roundtrips_messages_in_order() {
        let (_dir, paths) = paths_for_test();
        let request = SaveAgentConversationRequest {
            id: "conv-1".to_string(),
            title: None,
            provider_id: Some("llm-local".to_string()),
            messages: vec![
                user_message("m1", "when did I read about tauri?"),
                assistant_message("m2", "You read about Tauri in April."),
            ],
        };
        let summary = save_conversation(&paths, &request).expect("save");
        assert_eq!(summary.message_count, 2);
        // Auto-title is derived from the first user message.
        assert_eq!(summary.title, "when did I read about tauri?");
        assert_eq!(summary.provider_id.as_deref(), Some("llm-local"));

        let detail = load_conversation(&paths, "conv-1").expect("load").expect("present");
        assert_eq!(detail.summary.id, "conv-1");
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.messages[0].id, "m1");
        assert_eq!(detail.messages[0].role, "user");
        assert_eq!(detail.messages[1].id, "m2");
        assert_eq!(detail.messages[1].reasoning.as_deref(), Some("thinking"));
        assert_eq!(detail.messages[1].status.as_deref(), Some("done"));
        assert!(detail.messages[1].tool_calls_json.as_deref().unwrap().contains("search_bm25"));
    }

    #[test]
    fn explicit_title_overrides_auto_title_and_save_replaces_transcript() {
        let (_dir, paths) = paths_for_test();
        let first = SaveAgentConversationRequest {
            id: "conv-2".to_string(),
            title: Some("  Custom title  ".to_string()),
            provider_id: None,
            messages: vec![user_message("m1", "first"), assistant_message("m2", "answer one")],
        };
        let saved = save_conversation(&paths, &first).expect("first save");
        assert_eq!(saved.title, "Custom title");
        let created_at = saved.created_at.clone();

        // Re-save with a longer transcript: it replaces messages and preserves created_at.
        let second = SaveAgentConversationRequest {
            id: "conv-2".to_string(),
            title: Some("Custom title".to_string()),
            provider_id: None,
            messages: vec![
                user_message("m1", "first"),
                assistant_message("m2", "answer one"),
                user_message("m3", "second"),
                assistant_message("m4", "answer two"),
            ],
        };
        let resaved = save_conversation(&paths, &second).expect("second save");
        assert_eq!(resaved.message_count, 4);
        assert_eq!(resaved.created_at, created_at);

        let detail = load_conversation(&paths, "conv-2").expect("load").expect("present");
        assert_eq!(detail.messages.len(), 4);
        assert_eq!(detail.messages[3].id, "m4");
        // No stale duplicate rows from the first save.
        let connection = open_agent_connection(&paths).expect("connection");
        let total: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = 'conv-2'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(total, 4);
    }

    #[test]
    fn list_orders_newest_first_and_clamps_limit() {
        let (_dir, paths) = paths_for_test();
        for (index, id) in ["conv-a", "conv-b", "conv-c"].into_iter().enumerate() {
            // Save with strictly increasing updated_at by nudging the message content; the store
            // stamps now_rfc3339 per save and the loop runs fast, so also assert via re-save order.
            let request = SaveAgentConversationRequest {
                id: id.to_string(),
                title: Some(format!("conversation {index}")),
                provider_id: None,
                messages: vec![user_message(&format!("m{index}"), "hello")],
            };
            save_conversation(&paths, &request).expect("save");
            // Ensure a distinct updated_at ordering: re-save conv-c last so it sorts first.
        }
        // Touch conv-a last so it becomes newest.
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-a".to_string(),
                title: Some("conversation 0".to_string()),
                provider_id: None,
                messages: vec![user_message("m0b", "again")],
            },
        )
        .expect("touch conv-a");

        let all = list_conversations(&paths, None).expect("list default");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, "conv-a", "most recently updated sorts first");

        // A zero/None limit uses the default; a tiny limit caps the page.
        let one = list_conversations(&paths, Some(1)).expect("list capped");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].id, "conv-a");

        let zero = list_conversations(&paths, Some(0)).expect("zero falls back to default");
        assert_eq!(zero.len(), 3);

        // An oversized limit is clamped but still returns all existing rows.
        let huge = list_conversations(&paths, Some(10_000)).expect("oversized limit");
        assert_eq!(huge.len(), 3);
    }

    #[test]
    fn load_unknown_conversation_returns_none() {
        let (_dir, paths) = paths_for_test();
        assert!(load_conversation(&paths, "missing").expect("load missing").is_none());
    }

    #[test]
    fn delete_removes_conversation_and_cascades_messages() {
        let (_dir, paths) = paths_for_test();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-del".to_string(),
                title: None,
                provider_id: None,
                messages: vec![user_message("m1", "to be deleted"), assistant_message("m2", "ok")],
            },
        )
        .expect("save");

        assert!(delete_conversation(&paths, "conv-del").expect("delete present"));
        assert!(!delete_conversation(&paths, "conv-del").expect("delete absent again"));
        assert!(load_conversation(&paths, "conv-del").expect("load").is_none());

        let connection = open_agent_connection(&paths).expect("connection");
        let orphan_messages: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = 'conv-del'",
                [],
                |row| row.get(0),
            )
            .expect("orphan count");
        assert_eq!(orphan_messages, 0, "cascade must remove messages");
    }

    #[test]
    fn rename_updates_title_and_rejects_blank() {
        let (_dir, paths) = paths_for_test();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-rn".to_string(),
                title: Some("old".to_string()),
                provider_id: None,
                messages: vec![user_message("m1", "hi")],
            },
        )
        .expect("save");

        let renamed = rename_conversation(&paths, "conv-rn", "  brand new  ")
            .expect("rename")
            .expect("present");
        assert_eq!(renamed.title, "brand new");

        let blank = rename_conversation(&paths, "conv-rn", "   ").expect_err("blank rejected");
        assert!(blank.to_string().contains("cannot be blank"));

        assert!(rename_conversation(&paths, "missing", "x").expect("rename missing").is_none());
    }

    #[test]
    fn resave_with_more_messages_never_truncates_history() {
        // Regression for the data-loss trap: load returned a capped tail, the FE rehydrated that
        // view, and the next save wholesale-replaced the transcript — silently dropping everything
        // beyond the cap. A single chat must survive a re-save with every message intact, even when
        // it is far larger than the old 2_000 tail cap.
        let (_dir, paths) = paths_for_test();
        let large: Vec<AgentMessage> =
            (0..2_500).map(|i| user_message(&format!("m{i}"), &format!("turn {i}"))).collect();
        let first = SaveAgentConversationRequest {
            id: "conv-big".to_string(),
            title: Some("big chat".to_string()),
            provider_id: None,
            messages: large.clone(),
        };
        let saved = save_conversation(&paths, &first).expect("first save");
        assert_eq!(saved.message_count, 2_500);

        // Load returns the FULL transcript in order (no tail-cap), so a rehydrate-then-resave keeps
        // every message: simulate the FE path by loading, appending one, and re-saving.
        let loaded = load_conversation(&paths, "conv-big").expect("load").expect("present");
        assert_eq!(loaded.messages.len(), 2_500, "load must return all messages, not a tail");
        assert_eq!(loaded.messages.first().expect("first").id, "m0");
        assert_eq!(loaded.messages.last().expect("last").id, "m2499");

        let mut rehydrated = loaded.messages.clone();
        rehydrated.push(user_message("m2500", "one more"));
        let resaved = save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-big".to_string(),
                title: Some("big chat".to_string()),
                provider_id: None,
                messages: rehydrated,
            },
        )
        .expect("resave");
        assert_eq!(resaved.message_count, 2_501);

        // Re-load and assert ALL 2_501 survive, in order, with no loss.
        let after = load_conversation(&paths, "conv-big").expect("load").expect("present");
        assert_eq!(after.messages.len(), 2_501, "re-save must not truncate history");
        for (index, message) in after.messages.iter().enumerate() {
            assert_eq!(message.id, format!("m{index}"), "message {index} out of order or lost");
        }

        // The persisted seq is monotonic and complete (mutation guard: assert the stored column,
        // not the in-memory order).
        let connection = open_agent_connection(&paths).expect("connection");
        let mut statement = connection
            .prepare("SELECT seq FROM messages WHERE conversation_id = 'conv-big' ORDER BY seq ASC")
            .expect("prepare seq");
        let seqs: Vec<i64> = statement
            .query_map([], |row| row.get(0))
            .expect("query seq")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect seq");
        assert_eq!(seqs, (0..2_501).collect::<Vec<i64>>(), "persisted seq must be 0..N monotonic");
    }

    #[test]
    fn resave_preserves_per_message_created_at_and_stamps_only_new_messages() {
        // Per-message created_at must survive a re-save: restamping every message to `now` made the
        // timestamps meaningless (and is a W-AI-7 trap). Only a genuinely new message id is stamped.
        let (_dir, paths) = paths_for_test();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-ts".to_string(),
                title: None,
                provider_id: None,
                messages: vec![user_message("m1", "first"), assistant_message("m2", "answer")],
            },
        )
        .expect("first save");

        let read_created_at = |id: &str| -> Option<String> {
            let connection = open_agent_connection(&paths).expect("connection");
            connection
                .query_row("SELECT created_at FROM messages WHERE id = ?1", params![id], |row| {
                    row.get(0)
                })
                .optional()
                .expect("query created_at")
        };
        let m1_created = read_created_at("m1").expect("m1 created");
        let m2_created = read_created_at("m2").expect("m2 created");

        // Re-save with the same two messages plus a new one. The original two keep their stamps;
        // the new one is stamped at save time.
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-ts".to_string(),
                title: None,
                provider_id: None,
                messages: vec![
                    user_message("m1", "first"),
                    assistant_message("m2", "answer"),
                    user_message("m3", "second"),
                ],
            },
        )
        .expect("resave");

        assert_eq!(read_created_at("m1").as_deref(), Some(m1_created.as_str()), "m1 restamped");
        assert_eq!(read_created_at("m2").as_deref(), Some(m2_created.as_str()), "m2 restamped");
        assert!(read_created_at("m3").is_some(), "new message m3 must be stamped");
    }

    #[test]
    fn resave_holds_conversation_created_at_and_advances_updated_at_in_db() {
        // Mutation guard: assert the PERSISTED conversation row (created_at held, updated_at moved),
        // not just the Rust-built return summary.
        let (_dir, paths) = paths_for_test();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-row".to_string(),
                title: Some("row".to_string()),
                provider_id: None,
                messages: vec![user_message("m1", "hi")],
            },
        )
        .expect("first save");

        let read_row = || -> (String, String, i64) {
            let connection = open_agent_connection(&paths).expect("connection");
            connection
                .query_row(
                    "SELECT created_at, updated_at, message_count FROM conversations WHERE id = 'conv-row'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("query row")
        };
        let (created_first, updated_first, _) = read_row();

        // Force a strictly later wall-clock stamp so the updated_at comparison is meaningful.
        std::thread::sleep(StdDuration::from_millis(5));
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-row".to_string(),
                title: Some("row".to_string()),
                provider_id: None,
                messages: vec![user_message("m1", "hi"), assistant_message("m2", "yo")],
            },
        )
        .expect("resave");

        let (created_second, updated_second, count_second) = read_row();
        assert_eq!(
            created_second, created_first,
            "persisted created_at must be held across resave"
        );
        assert!(updated_second > updated_first, "persisted updated_at must advance on resave");
        assert_eq!(count_second, 2, "persisted message_count must reflect the resaved transcript");
    }

    #[test]
    fn list_cap_holds_at_two_hundred_even_for_oversized_limits() {
        // Mutation guard: insert more than the hard ceiling, then assert the PERSISTED list read
        // honors both caps — the default (when None) and the hard ceiling (when an oversized
        // explicit limit is passed). 225 rows exist on disk.
        let (_dir, paths) = paths_for_test();
        let connection = open_agent_connection(&paths).expect("connection");
        let now = now_rfc3339();
        let total = MAX_CONVERSATION_LIST_LIMIT as usize + 25;
        for index in 0..total {
            connection
                .execute(
                    "INSERT INTO conversations (id, title, provider_id, created_at, updated_at, message_count)
                     VALUES (?1, ?2, NULL, ?3, ?4, 0)",
                    params![format!("c{index:04}"), format!("conversation {index}"), now, format!("{now}-{index:04}")],
                )
                .expect("insert conversation");
        }
        drop(connection);

        // None falls back to the default page size, not the hard ceiling.
        let default_page = list_conversations(&paths, None).expect("default page");
        assert_eq!(
            default_page.len(),
            DEFAULT_CONVERSATION_LIST_LIMIT as usize,
            "None must use the default page size"
        );
        // An oversized explicit limit clamps to the hard ceiling: exactly 200, never all 225.
        let huge = list_conversations(&paths, Some(10_000)).expect("oversized limit");
        assert_eq!(
            huge.len(),
            MAX_CONVERSATION_LIST_LIMIT as usize,
            "Some(10_000) must clamp to the hard ceiling of 200"
        );
        assert!(huge.len() < total, "the clamp must drop rows beyond the ceiling");
    }

    #[test]
    fn save_requires_non_empty_id() {
        let (_dir, paths) = paths_for_test();
        let error = save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "  ".to_string(),
                title: None,
                provider_id: None,
                messages: vec![],
            },
        )
        .expect_err("blank id rejected");
        assert!(error.to_string().contains("conversation id is required"));
    }

    #[test]
    fn auto_title_falls_back_and_truncates() {
        // No user message → fallback title.
        assert_eq!(
            derive_title_from_messages(&[assistant_message("m1", "answer only")]),
            UNTITLED_CONVERSATION_TITLE
        );
        // Blank user content is skipped in favor of the fallback.
        assert_eq!(
            derive_title_from_messages(&[user_message("m1", "   ")]),
            UNTITLED_CONVERSATION_TITLE
        );
        // Whitespace is collapsed.
        assert_eq!(
            derive_title_from_messages(&[user_message("m1", "  multi   line\nprompt  ")]),
            "multi line prompt"
        );
        // Long content is truncated on a char boundary with an ellipsis.
        let long = "word ".repeat(40);
        let title = derive_title_from_messages(&[user_message("m1", &long)]);
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= AUTO_TITLE_MAX_CHARS + 1);
    }

    // -----------------------------------------------------------------------
    // W-AI-7 migration v2: agent run trace journal.
    // -----------------------------------------------------------------------

    #[test]
    fn migration_v2_creates_run_trace_tables_and_indices() {
        let mut connection = Connection::open_in_memory().expect("in memory sqlite");
        connection.pragma_update(None, "foreign_keys", true).expect("foreign keys");
        ensure_agent_schema(&mut connection).expect("ensure agent schema");

        // Both migrations applied and recorded.
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM agent_schema_migrations", [], |row| row.get(0))
            .expect("migration count");
        assert_eq!(migration_count as usize, AGENT_MIGRATIONS.len());
        assert_eq!(AGENT_MIGRATIONS.len(), 2, "v1 + v2");

        for table in ["agent_runs", "agent_steps", "agent_citations"] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| panic!("query {table}"));
            assert_eq!(exists, 1, "{table} must exist");
        }
        for index in ["idx_agent_runs_conversation", "idx_agent_steps_run_seq"] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    params![index],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| panic!("query {index}"));
            assert_eq!(exists, 1, "{index} must exist");
        }
    }

    #[test]
    fn forward_migration_from_v1_only_applies_v2_atomically() {
        // A database that already has v1 (chat-history) but not v2 must apply ONLY v2 on next open,
        // and its ledger row appears as a unit (no partial structural change).
        let mut connection = Connection::open_in_memory().expect("in memory sqlite");
        apply_agent_migrations(&mut connection, &AGENT_MIGRATIONS[..1]).expect("apply v1 only");
        let before: i64 = connection
            .query_row("SELECT COUNT(*) FROM agent_schema_migrations", [], |row| row.get(0))
            .expect("count before");
        assert_eq!(before, 1);

        apply_agent_migrations(&mut connection, AGENT_MIGRATIONS).expect("apply through v2");
        let v2: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM agent_schema_migrations WHERE version = 2",
                [],
                |row| row.get(0),
            )
            .expect("v2 row");
        assert_eq!(v2, 1, "v2 recorded exactly once");
    }

    fn run_trace_paths() -> (tempfile::TempDir, ProjectPaths) {
        paths_for_test()
    }

    fn citation(history_id: i64, canonical: &str) -> AgentCitationRecord {
        AgentCitationRecord {
            history_id,
            canonical_url: canonical.to_string(),
            url: format!("{canonical}?utm=x"),
            title: Some(format!("page {history_id}")),
            visited_at: Some("2026-06-21T00:00:00Z".to_string()),
            score: Some(0.5),
        }
    }

    #[test]
    fn run_trace_round_trips_header_steps_and_citations_in_order() {
        let (_dir, paths) = run_trace_paths();
        // The run links to a real conversation so the cascade FK is exercised.
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-run".to_string(),
                title: Some("run".to_string()),
                provider_id: None,
                messages: vec![user_message("m1", "what did i read?")],
            },
        )
        .expect("save conversation");

        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-1".to_string(),
                conversation_id: Some("conv-run".to_string()),
                message_id: Some("m1".to_string()),
                provider_id: Some("llm-local".to_string()),
                embedding_provider_id: Some("embed-local".to_string()),
            },
        )
        .expect("begin run");

        // Journal-before-observe: the harness appends each step; seq is assigned monotonically.
        let s0 = append_agent_step(
            &paths,
            &AppendAgentStep {
                run_id: "run-1".to_string(),
                turn: 1,
                kind: "assistant-turn".to_string(),
                tool_name: None,
                tool_call_id: None,
                payload: r#"{"text":"thinking"}"#.to_string(),
            },
        )
        .expect("append step 0");
        let s1 = append_agent_step(
            &paths,
            &AppendAgentStep {
                run_id: "run-1".to_string(),
                turn: 1,
                kind: "tool-result".to_string(),
                tool_name: Some("search_history".to_string()),
                tool_call_id: Some("call-1".to_string()),
                payload: r#"{"rows":3}"#.to_string(),
            },
        )
        .expect("append step 1");
        assert_eq!((s0, s1), (0, 1), "seq is 0-based monotonic");

        record_agent_citations(
            &paths,
            "run-1",
            &[citation(20, "https://b.example/"), citation(10, "https://a.example/")],
        )
        .expect("record citations");
        finalize_agent_run(&paths, "run-1", AgentRunStatus::Completed, 2, 42, 17, None)
            .expect("finalize");

        let trace = load_agent_run(&paths, "run-1").expect("load").expect("present");
        assert_eq!(trace.status, AgentRunStatus::Completed);
        assert_eq!(trace.iterations, 2);
        assert_eq!((trace.prompt_tokens, trace.completion_tokens), (42, 17));
        assert_eq!(trace.error, None);
        assert_eq!(trace.message_id.as_deref(), Some("m1"));
        // Steps replay in seq order.
        assert_eq!(trace.steps.len(), 2);
        assert_eq!(trace.steps[0].seq, 0);
        assert_eq!(trace.steps[0].kind, "assistant-turn");
        assert_eq!(trace.steps[1].tool_call_id.as_deref(), Some("call-1"));
        // Citations sorted by history_id, canonical_url carried for the W-STAR key.
        assert_eq!(trace.citations.len(), 2);
        assert_eq!(trace.citations[0].history_id, 10);
        assert_eq!(trace.citations[0].canonical_url, "https://a.example/");
    }

    #[test]
    fn duplicate_tool_result_step_is_idempotent_on_call_id() {
        let (_dir, paths) = run_trace_paths();
        begin_agent_run(
            &paths,
            &BeginAgentRun { id: "run-idem".to_string(), ..Default::default() },
        )
        .expect("begin");
        let first = append_agent_step(
            &paths,
            &AppendAgentStep {
                run_id: "run-idem".to_string(),
                turn: 1,
                kind: "tool-result".to_string(),
                tool_name: Some("search_bm25".to_string()),
                tool_call_id: Some("call-7".to_string()),
                payload: r#"{"rows":1}"#.to_string(),
            },
        )
        .expect("first append");
        // Re-journaling the SAME (run_id, tool_call_id) returns the existing seq, no second row.
        let again = append_agent_step(
            &paths,
            &AppendAgentStep {
                run_id: "run-idem".to_string(),
                turn: 1,
                kind: "tool-result".to_string(),
                tool_name: Some("search_bm25".to_string()),
                tool_call_id: Some("call-7".to_string()),
                payload: r#"{"rows":1}"#.to_string(),
            },
        )
        .expect("dup append");
        assert_eq!(first, again, "duplicate returns the same seq");

        let trace = load_agent_run(&paths, "run-idem").expect("load").expect("present");
        assert_eq!(trace.steps.len(), 1, "idempotency key prevents a second row");
    }

    #[test]
    fn finalize_failed_run_records_error_and_load_missing_is_none() {
        let (_dir, paths) = run_trace_paths();
        begin_agent_run(
            &paths,
            &BeginAgentRun { id: "run-fail".to_string(), ..Default::default() },
        )
        .expect("begin");
        finalize_agent_run(&paths, "run-fail", AgentRunStatus::Failed, 1, 5, 0, Some("boom"))
            .expect("finalize failed");
        let trace = load_agent_run(&paths, "run-fail").expect("load").expect("present");
        assert_eq!(trace.status, AgentRunStatus::Failed);
        assert_eq!(trace.error.as_deref(), Some("boom"));
        assert!(load_agent_run(&paths, "missing-run").expect("load missing").is_none());
    }

    #[test]
    fn begin_agent_run_rejects_blank_id_and_reopen_resets_accounting() {
        let (_dir, paths) = run_trace_paths();
        let blank =
            begin_agent_run(&paths, &BeginAgentRun { id: "  ".to_string(), ..Default::default() })
                .expect_err("blank id");
        assert!(blank.to_string().contains("agent run id is required"));

        begin_agent_run(
            &paths,
            &BeginAgentRun { id: "run-reopen".to_string(), ..Default::default() },
        )
        .expect("begin");
        finalize_agent_run(&paths, "run-reopen", AgentRunStatus::Completed, 3, 9, 9, None)
            .expect("finalize");
        // Re-begin resets status to running and zeroes accounting (clean restart of the same id).
        begin_agent_run(
            &paths,
            &BeginAgentRun { id: "run-reopen".to_string(), ..Default::default() },
        )
        .expect("re-begin");
        let trace = load_agent_run(&paths, "run-reopen").expect("load").expect("present");
        assert_eq!(trace.status, AgentRunStatus::Running);
        assert_eq!((trace.iterations, trace.prompt_tokens, trace.completion_tokens), (0, 0, 0));
    }

    #[test]
    fn record_agent_citations_replaces_prior_set() {
        let (_dir, paths) = run_trace_paths();
        begin_agent_run(
            &paths,
            &BeginAgentRun { id: "run-cite".to_string(), ..Default::default() },
        )
        .expect("begin");
        record_agent_citations(&paths, "run-cite", &[citation(1, "https://one.example/")])
            .expect("first citations");
        record_agent_citations(
            &paths,
            "run-cite",
            &[citation(2, "https://two.example/"), citation(3, "https://three.example/")],
        )
        .expect("replace citations");
        let trace = load_agent_run(&paths, "run-cite").expect("load").expect("present");
        assert_eq!(trace.citations.len(), 2, "prior set replaced, not appended");
        assert_eq!(trace.citations[0].history_id, 2);
    }

    #[test]
    fn deleting_conversation_cascades_run_trace() {
        // The run trace is keyed off the conversation; deleting the conversation must cascade the
        // run, its steps, and citations (the trace is rebuildable derived state).
        let (_dir, paths) = run_trace_paths();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-casc".to_string(),
                title: None,
                provider_id: None,
                messages: vec![user_message("m1", "hi")],
            },
        )
        .expect("save");
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-casc".to_string(),
                conversation_id: Some("conv-casc".to_string()),
                ..Default::default()
            },
        )
        .expect("begin");
        append_agent_step(
            &paths,
            &AppendAgentStep {
                run_id: "run-casc".to_string(),
                turn: 1,
                kind: "assistant-turn".to_string(),
                tool_name: None,
                tool_call_id: None,
                payload: "{}".to_string(),
            },
        )
        .expect("step");
        record_agent_citations(&paths, "run-casc", &[citation(1, "https://x.example/")])
            .expect("citations");

        assert!(delete_conversation(&paths, "conv-casc").expect("delete"));
        assert!(load_agent_run(&paths, "run-casc").expect("load after cascade").is_none());
        let connection = open_agent_connection(&paths).expect("connection");
        let orphan_steps: i64 = connection
            .query_row("SELECT COUNT(*) FROM agent_steps WHERE run_id = 'run-casc'", [], |row| {
                row.get(0)
            })
            .expect("orphan steps");
        let orphan_citations: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM agent_citations WHERE run_id = 'run-casc'",
                [],
                |row| row.get(0),
            )
            .expect("orphan citations");
        assert_eq!((orphan_steps, orphan_citations), (0, 0), "cascade removes children");
    }

    #[test]
    fn begin_agent_run_self_heals_missing_conversation_parent() {
        // Regression: the FE persists a conversation lazily ON FINALIZE (after the turn), so the
        // FIRST tools-enabled turn of a NEW conversation opens a run whose `conversation_id` has no
        // `conversations` row yet. With the live FK (+ foreign_keys=ON) the agent_runs INSERT would
        // trip `FOREIGN KEY constraint failed` and kill the run before any model call. begin_agent_run
        // must self-heal by upserting a stub parent so the run opens cleanly.
        let (_dir, paths) = run_trace_paths();
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-orphan".to_string(),
                conversation_id: Some("conv-not-yet-saved".to_string()),
                message_id: Some("m1".to_string()),
                provider_id: Some("llm-local".to_string()),
                ..Default::default()
            },
        )
        .expect("begin must succeed even though the conversation row does not exist yet");

        // The run header is persisted and round-trips, linked to the (now stubbed) conversation.
        let trace = load_agent_run(&paths, "run-orphan").expect("load").expect("present");
        assert_eq!(trace.conversation_id.as_deref(), Some("conv-not-yet-saved"));
        assert_eq!(trace.status, AgentRunStatus::Running);

        // A stub parent conversations row now exists (placeholder title) so the FK is satisfied.
        let connection = open_agent_connection(&paths).expect("connection");
        let (title, message_count): (String, i64) = connection
            .query_row(
                "SELECT title, message_count FROM conversations WHERE id = 'conv-not-yet-saved'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("stub conversation row exists");
        assert_eq!(title, UNTITLED_CONVERSATION_TITLE, "stub carries a placeholder title");
        assert_eq!(message_count, 0, "stub carries no messages");
    }

    #[test]
    fn save_conversation_overwrites_a_begin_agent_run_stub() {
        // The stub begin_agent_run plants must be a true placeholder: a later save_conversation for
        // the same id lands the real title + transcript (wholesale replace) and the run still links.
        let (_dir, paths) = run_trace_paths();
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-stub".to_string(),
                conversation_id: Some("conv-lazy".to_string()),
                message_id: Some("m1".to_string()),
                ..Default::default()
            },
        )
        .expect("begin plants a stub conversation");

        // Capture the stub's created_at so we can prove the real save preserves the birth time.
        let stub_created_at: String = {
            let connection = open_agent_connection(&paths).expect("connection");
            connection
                .query_row(
                    "SELECT created_at FROM conversations WHERE id = 'conv-lazy'",
                    [],
                    |row| row.get(0),
                )
                .expect("stub created_at")
        };

        // The FE finalizes the turn: the real save overwrites the stub with the genuine transcript.
        std::thread::sleep(StdDuration::from_millis(5));
        let saved = save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-lazy".to_string(),
                title: None,
                provider_id: Some("llm-local".to_string()),
                messages: vec![
                    user_message("m1", "what did i read about tauri?"),
                    assistant_message("m2", "You read about Tauri in April."),
                ],
            },
        )
        .expect("save overwrites the stub");
        assert_eq!(saved.title, "what did i read about tauri?", "real title replaces the stub");
        assert_eq!(saved.message_count, 2, "real transcript replaces the empty stub");
        assert_eq!(saved.created_at, stub_created_at, "created_at preserved across the overwrite");

        // The transcript landed and the run still links to the (now real) conversation.
        let detail = load_conversation(&paths, "conv-lazy").expect("load").expect("present");
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.messages[0].id, "m1");
        let trace = load_agent_run(&paths, "run-stub").expect("load").expect("present");
        assert_eq!(trace.conversation_id.as_deref(), Some("conv-lazy"));
    }

    #[test]
    fn begin_agent_run_does_not_clobber_an_existing_saved_conversation() {
        // When the conversation already exists (e.g. a follow-up turn), the stub upsert must be a
        // no-op: ON CONFLICT DO NOTHING keeps the real title, created_at, and message_count intact.
        let (_dir, paths) = run_trace_paths();
        let saved = save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-existing".to_string(),
                title: Some("Real title".to_string()),
                provider_id: None,
                messages: vec![user_message("m1", "hi"), assistant_message("m2", "yo")],
            },
        )
        .expect("save real conversation first");

        std::thread::sleep(StdDuration::from_millis(5));
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-followup".to_string(),
                conversation_id: Some("conv-existing".to_string()),
                ..Default::default()
            },
        )
        .expect("begin against an existing conversation");

        let connection = open_agent_connection(&paths).expect("connection");
        let (title, created_at, message_count): (String, String, i64) = connection
            .query_row(
                "SELECT title, created_at, message_count FROM conversations WHERE id = 'conv-existing'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("existing conversation row");
        assert_eq!(title, "Real title", "begin must not overwrite the real title with a stub");
        assert_eq!(created_at, saved.created_at, "begin must not restamp created_at");
        assert_eq!(message_count, 2, "begin must not zero the real message_count");
    }

    #[test]
    fn agent_run_status_tags_round_trip() {
        for status in [
            AgentRunStatus::Running,
            AgentRunStatus::Completed,
            AgentRunStatus::Cancelled,
            AgentRunStatus::Failed,
        ] {
            assert_eq!(AgentRunStatus::from_tag(status.as_str()), status);
        }
        // An unknown tag reads as the honest "interrupted" Running state.
        assert_eq!(AgentRunStatus::from_tag("nonsense"), AgentRunStatus::Running);
    }

    // -----------------------------------------------------------------------
    // W-AI-7 WU-7: citations + usage reconstructed on reopen (load_conversation).
    // -----------------------------------------------------------------------

    #[test]
    fn load_conversation_reconstructs_citations_and_usage_for_the_answering_turn() {
        // The durable trace must survive a reopen: a run linked to an assistant message (by
        // message_id) pins citations + tallies tokens, and a later load_conversation stitches BOTH
        // onto that assistant turn (and only that one) — so a reopened conversation renders the same
        // evidence rows + star keys + token footer the live turn streamed.
        let (_dir, paths) = run_trace_paths();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-trace".to_string(),
                title: None,
                provider_id: Some("llm-local".to_string()),
                messages: vec![
                    user_message("u1", "what did i read about tauri?"),
                    assistant_message("a1", "You read the Tauri guide in April."),
                ],
            },
        )
        .expect("save conversation");

        // The agent run that ANSWERED message a1 (the FE passes the assistant message id as the run's
        // message_id), pins two citations, and finalizes with a token tally.
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-trace".to_string(),
                conversation_id: Some("conv-trace".to_string()),
                message_id: Some("a1".to_string()),
                provider_id: Some("llm-local".to_string()),
                embedding_provider_id: None,
            },
        )
        .expect("begin run");
        record_agent_citations(
            &paths,
            "run-trace",
            &[citation(20, "https://tauri.app/guide/"), citation(10, "https://tauri.app/")],
        )
        .expect("record citations");
        finalize_agent_run(&paths, "run-trace", AgentRunStatus::Completed, 2, 128, 64, None)
            .expect("finalize");

        let detail = load_conversation(&paths, "conv-trace").expect("load").expect("present");
        assert_eq!(detail.messages.len(), 2);
        // The user turn carries NO reconstructed trace.
        let user = &detail.messages[0];
        assert_eq!(user.id, "u1");
        assert!(user.citations.is_empty(), "user turn must not carry citations");
        assert!(user.usage.is_none(), "user turn must not carry usage");
        // The assistant turn carries the run's usage + citations (ordered by history_id, with the
        // canonical_url star key preserved).
        let assistant = &detail.messages[1];
        assert_eq!(assistant.id, "a1");
        assert_eq!(
            assistant.usage,
            Some(AgentUsage { prompt_tokens: 128, completion_tokens: 64 }),
            "assistant turn must carry the run's reconstructed usage"
        );
        assert_eq!(assistant.citations.len(), 2, "both pinned citations are reconstructed");
        assert_eq!(assistant.citations[0].history_id, 10);
        assert_eq!(
            assistant.citations[0].canonical_url.as_deref(),
            Some("https://tauri.app/"),
            "the W-STAR star key survives the reopen"
        );
        assert_eq!(assistant.citations[1].history_id, 20);
        // profile_id is not journaled with a citation; it reconstructs empty (every consumer keys off
        // canonical_url/url, never profile_id).
        assert!(assistant.citations[0].profile_id.is_empty());
    }

    #[test]
    fn load_conversation_without_a_run_leaves_turns_trace_free() {
        // A plain conversation (no agent run journaled) must reopen with no citations + no usage on
        // any turn — the reconstruction is purely additive and never fabricates a trace.
        let (_dir, paths) = run_trace_paths();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-plain".to_string(),
                title: None,
                provider_id: None,
                messages: vec![user_message("u1", "hi"), assistant_message("a1", "hello")],
            },
        )
        .expect("save");

        let detail = load_conversation(&paths, "conv-plain").expect("load").expect("present");
        for message in &detail.messages {
            assert!(message.citations.is_empty(), "no run → no citations");
            assert!(message.usage.is_none(), "no run → no usage");
        }
    }

    #[test]
    fn load_conversation_uses_the_latest_run_per_message_and_zero_tokens_yield_no_usage() {
        // Two runs answer the SAME assistant message (e.g. a re-asked turn reusing the id): the LATEST
        // run's trace wins on reopen, and a run that tallied zero tokens surfaces no usage footer (the
        // honest "no accounting" state), while its citations still reconstruct.
        let (_dir, paths) = run_trace_paths();
        save_conversation(
            &paths,
            &SaveAgentConversationRequest {
                id: "conv-latest".to_string(),
                title: None,
                provider_id: None,
                messages: vec![user_message("u1", "ask"), assistant_message("a1", "answer")],
            },
        )
        .expect("save");

        // The earlier run pins one citation + a token tally.
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-old".to_string(),
                conversation_id: Some("conv-latest".to_string()),
                message_id: Some("a1".to_string()),
                ..Default::default()
            },
        )
        .expect("begin old");
        record_agent_citations(&paths, "run-old", &[citation(1, "https://old.example/")])
            .expect("old citations");
        finalize_agent_run(&paths, "run-old", AgentRunStatus::Completed, 1, 50, 20, None)
            .expect("finalize old");

        // A later run for the same message tallies ZERO tokens and pins a different citation. Force a
        // strictly later updated_at so the "latest run wins" tie-break is meaningful.
        std::thread::sleep(StdDuration::from_millis(5));
        begin_agent_run(
            &paths,
            &BeginAgentRun {
                id: "run-new".to_string(),
                conversation_id: Some("conv-latest".to_string()),
                message_id: Some("a1".to_string()),
                ..Default::default()
            },
        )
        .expect("begin new");
        record_agent_citations(&paths, "run-new", &[citation(2, "https://new.example/")])
            .expect("new citations");
        finalize_agent_run(&paths, "run-new", AgentRunStatus::Completed, 1, 0, 0, None)
            .expect("finalize new");

        let detail = load_conversation(&paths, "conv-latest").expect("load").expect("present");
        let assistant = &detail.messages[1];
        assert!(assistant.usage.is_none(), "a zero-token run surfaces no usage footer");
        assert_eq!(assistant.citations.len(), 1, "only the latest run's citation is shown");
        assert_eq!(assistant.citations[0].history_id, 2, "latest run's evidence wins on reopen");
    }
}
