//! Combines original evidence in a private stage before publishing a move import.
//! Rebuilding the read model, archive replacement and password handling belong to the owner.

use vault_store::{CancelToken, Result, Session, params};

pub(super) const SCHEMA: &[&str] = &[
    "CREATE TABLE ah_move_origin(origin TEXT PRIMARY KEY)",
    "INSERT INTO ah_move_origin VALUES(lower(hex(randomblob(16))))",
    "CREATE TABLE ah_move_imports(id TEXT PRIMARY KEY, imported_at TEXT NOT NULL, previous_copy TEXT)",
    "CREATE TABLE ah_import_conversations(import_id TEXT NOT NULL,conversation_id TEXT NOT NULL,
        eligible INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(import_id,conversation_id))",
    "CREATE INDEX ah_import_eligible ON ah_import_conversations(conversation_id,eligible)",
];

/// Registers the incoming conversations before any source membership is changed.
pub fn record_new(s: &Session<'_>, id: &str, date: &str, fresh: bool) -> Result<()> {
    let select = if fresh {
        "SELECT DISTINCT ?1,conversation_id FROM ah_source_conversations"
    } else {
        "SELECT DISTINCT ?1,i.conversation_id FROM incoming.ah_source_conversations i
         WHERE NOT EXISTS(SELECT 1 FROM main.ah_source_conversations h WHERE h.conversation_id=i.conversation_id)"
    };
    let inserted = s.execute(
        &format!("INSERT INTO ah_import_conversations(import_id,conversation_id) {select}"),
        [id],
    )?;
    if inserted > 0 {
        s.execute("INSERT INTO ah_move_imports(id,imported_at) VALUES(?1,?2)", params![id, date])?;
    }
    Ok(())
}

/// Imports one evidence file attached to a private destination. Source identities are qualified
/// by the exporting archive so a state file at the same path on two computers retains both heads.
pub fn merge(
    s: &Session<'_>,
    origin: &str,
    id: &str,
    date: &str,
    cancel: &CancelToken,
) -> Result<()> {
    record_new(s, id, date, false)?;
    for stmt in include_str!("move_mapping.sql").split(';').map(str::trim).filter(|s| !s.is_empty())
    {
        cancel.check()?;
        if stmt.contains("?1") {
            s.execute(stmt, [origin])?;
        } else {
            s.execute(stmt, ())?;
        }
    }
    // The chunk body is the largest original by orders of magnitude. Commit a bounded
    // set at a time. Digest-addressed rows already in the local store are reused.
    let mut cursor = 0i64;
    loop {
        cancel.check()?;
        let last = s.query_one("SELECT max(rowid) FROM (SELECT rowid FROM incoming.ah_chunks WHERE rowid>?1 ORDER BY rowid LIMIT 4)", [cursor], |r| Ok(r.get::<Option<i64>>(0)?))?;
        let Some(last) = last else { break };
        s.execute("INSERT OR IGNORE INTO main.ah_chunks SELECT hash,data,codec FROM incoming.ah_chunks WHERE rowid>?1 AND rowid<=?2",params![cursor,last])?;
        cursor = last;
    }
    for stmt in
        include_str!("move_evidence.sql").split(';').map(str::trim).filter(|s| !s.is_empty())
    {
        cancel.check()?;
        s.execute(stmt, ())?;
    }
    Ok(())
}

impl super::Archive {
    /// Replay the marks committed by undo before it reports completion to the reader.
    pub fn refresh_import_marks(&self) -> crate::Result<()> {
        self.synchronize_user_state()
    }
}

/// Copy exactly the eligible conversations and their retained revision/attachment closure.
/// The live evidence connection only reads its main schema. Every write targets the private pair.
pub fn copy_eligible(s: &Session<'_>, id: &str) -> Result<()> {
    let statements = |script: &str| -> Result<()> {
        for sql in script.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if sql.contains("?1") {
                s.execute(sql, [id])?;
            } else {
                s.execute(sql, ())?;
            }
        }
        Ok(())
    };
    statements(include_str!("copy_import.sql"))?;
    let mut after = 0i64;
    loop {
        let last=s.query_one("SELECT max(slot) FROM (SELECT rowid AS slot FROM incoming.copy_chunks WHERE rowid>?1 ORDER BY rowid LIMIT 4)",[after],|r|Ok(r.get::<Option<i64>>(0)?))?;
        let Some(last) = last else { break };
        s.execute("INSERT INTO incoming.ah_chunks SELECT c.* FROM incoming.copy_chunks w JOIN main.ah_chunks c ON c.hash=w.hash WHERE w.rowid>?1 AND w.rowid<=?2",params![after,last])?;
        after = last;
    }
    statements(include_str!("copy_import_metadata.sql"))
}
