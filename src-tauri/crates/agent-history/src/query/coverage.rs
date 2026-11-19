//! Responsibilities: derive chain coverage consistently for reader, search and verification.
//! Not responsible for: counting unavailable files as unread conversation records.
use crate::{Archive, Diagnostic, Result};
use vault_store::Db;

impl Archive {
    pub(crate) fn normalization_complete(&self) -> Result<bool> {
        self.store.read(Db::AgentHistoryReadModel, |tx| {
            let failed = tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM ah_coverage c
                 WHERE c.complete=0 AND c.affects_conversations=1
                 AND NOT EXISTS(SELECT 1 FROM ah_sessions s WHERE s.id=c.source_id))",
                (),
                |row| Ok(row.get::<i64>(0)? != 0),
            )?;
            if failed {
                return Ok(Ok(false));
            }
            let standalone = tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM ah_sessions s
                 WHERE conversation_current=1 AND lineage_base_native_id IS NULL
                   AND (s.complete=0 OR s.raw_head_current=0 OR coalesce(
                     (SELECT complete FROM ah_coverage WHERE source_id=s.id),1)=0))",
                (),
                |row| Ok(row.get::<i64>(0)? != 0),
            )?;
            if standalone {
                return Ok(Ok(false));
            }
            let mut after = String::new();
            loop {
                let ids = tx.query(
                    "SELECT conversation_id FROM ah_sessions WHERE conversation_current=1
                     AND lineage_base_native_id IS NOT NULL AND conversation_id>?1
                     ORDER BY conversation_id LIMIT 128",
                    [after.as_str()],
                    |row| Ok(row.get::<String>(0)?),
                )?;
                if ids.is_empty() {
                    return Ok(Ok(true));
                }
                for id in ids {
                    let status = super::line::describe(tx, &id)
                        .map_err(super::operation)?
                        .expect("published conversation");
                    if !status.complete {
                        return Ok(Ok(false));
                    }
                    after = id;
                }
            }
        })?
    }

    pub(crate) fn append_lineage_issues(&self, issues: &mut Vec<Diagnostic>) -> Result<()> {
        let unresolved = self.store.read(Db::AgentHistoryReadModel, |tx| {
            let mut after = String::new();
            let mut issues = Vec::new();
            loop {
                let rows = tx.query(
                    "SELECT conversation_id,relative_path FROM ah_sessions
                     WHERE conversation_current=1 AND lineage_base_native_id IS NOT NULL
                     AND conversation_id>?1 ORDER BY conversation_id LIMIT 128",
                    [after.as_str()],
                    |row| Ok((row.get::<String>(0)?, row.get::<String>(1)?)),
                )?;
                if rows.is_empty() {
                    return Ok(issues);
                }
                for (id, path) in rows {
                    let status = super::line::describe(tx, &id)
                        .map_err(super::operation)?
                        .expect("published conversation");
                    if let Some(issue) = status.issue {
                        crate::adapters::common::push_diagnostic(
                            &mut issues,
                            Diagnostic {
                                code: "lineage_unresolved".into(),
                                path: Some(path),
                                ordinal: None,
                                detail: match issue {
                                    super::line::LineageIssue::Cycle => Some("cycle".into()),
                                    super::line::LineageIssue::Unresolved => None,
                                },
                            },
                        );
                    }
                    after = id;
                }
            }
        })?;
        for diagnostic in unresolved {
            crate::adapters::common::push_diagnostic(issues, diagnostic);
        }
        Ok(())
    }
}
