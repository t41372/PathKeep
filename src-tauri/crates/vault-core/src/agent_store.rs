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
        AgentConversationDetail, AgentConversationSummary, AgentMessage, SaveAgentConversationRequest,
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

/// The ordered agent-plane migrations. Append-only: never edit or reorder an applied entry; add a
/// new spec with the next version (this is how W-AI-7 extends the agent plane).
const AGENT_MIGRATIONS: &[AgentMigrationSpec] = &[AgentMigrationSpec {
    version: 1,
    name: "chat-history-baseline",
    sql: AGENT_CHAT_BASELINE_SQL,
    apply: apply_chat_history_baseline_migration,
}];

/// Installs the baseline chat-history schema on a freshly attached agent database.
fn apply_chat_history_baseline_migration(tx: &Transaction<'_>, sql: &'static str) -> Result<()> {
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
    let mut statement = connection
        .prepare("SELECT version FROM agent_schema_migrations ORDER BY version ASC")?;
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
        let mut prior = tx.prepare(
            "SELECT id, created_at FROM messages WHERE conversation_id = ?1",
        )?;
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
        params![
            request.id,
            title,
            request.provider_id,
            created_at,
            now,
            message_count as i64
        ],
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

/// Loads one conversation plus its FULL message transcript in `seq` order.
///
/// Returns `None` when the id is unknown. The transcript is read ascending from the
/// `(conversation_id, seq)` index and is intentionally NOT capped: a single chat is bounded by real
/// usage, not the 14.4M archive baseline, so loading every message is the only correct behavior.
///
/// Capping the load was a data-loss trap: the front end rehydrates the loaded view and re-saves it
/// on the next turn, which (with a wholesale replace) would have permanently dropped every message
/// beyond the cap. Loading all messages keeps re-save lossless.
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
    let messages = statement
        .query_map(params![conversation_id], |row| {
            Ok(AgentMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                reasoning: row.get(3)?,
                tool_calls_json: row.get(4)?,
                status: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(AgentConversationDetail { summary, messages }))
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
        }
    }

    fn assistant_message(id: &str, content: &str) -> AgentMessage {
        AgentMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
            content: content.to_string(),
            reasoning: Some("thinking".to_string()),
            tool_calls_json: Some("[{\"id\":\"t1\",\"name\":\"search_bm25\",\"arguments\":\"{}\"}]".to_string()),
            status: Some("done".to_string()),
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
                .query_row(
                    "SELECT created_at FROM messages WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
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
        assert_eq!(created_second, created_first, "persisted created_at must be held across resave");
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
}
