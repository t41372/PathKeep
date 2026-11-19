//! Responsibilities: attach source membership from the same hot snapshot as its summary.
//! Not responsible for: guessing foreign IDs or building unbounded SQL parameter lists.
use super::{ConversationSource, SessionView, line};
use crate::{Archive, Result, adapters::identity::SourceRelation};
use std::collections::HashMap;
use vault_store::{Db, Session, Value};

struct Member {
    source: String,
    current: bool,
    copy: Option<String>,
    relation: SourceRelation,
}

pub(super) fn attach(tx: &Session<'_>, items: &mut [SessionView]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    // The public metadata page is capped at 500 conversations.
    let ids = items.iter().map(|item| Value::Text(item.id.clone())).collect::<Vec<_>>();
    let placeholders = vec!["?"; ids.len()].join(",");
    let rows = tx.query(
        &format!(
            "SELECT conversation_id,id,conversation_current,copy_of_source_id,relation_json
             FROM ah_sessions WHERE conversation_id IN ({placeholders}) ORDER BY id"
        ),
        ids,
        |row| {
            Ok((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<i64>(2)? != 0,
                row.get::<Option<String>>(3)?,
                row.get::<String>(4)?,
            ))
        },
    )?;
    let mut groups: HashMap<String, Vec<Member>> = HashMap::new();
    for (conversation, source, current, copy, relation) in rows {
        groups.entry(conversation).or_default().push(Member {
            source,
            current,
            copy,
            relation: serde_json::from_str(&relation)?,
        });
    }
    for item in items {
        let status = line::describe(tx, &item.id)?.expect("published conversation");
        item.complete = status.complete;
        item.attachments_not_kept = status.attachments;
        for member in groups.remove(&item.id).unwrap_or_default() {
            let base = line::base(tx, &member.source, &member.relation)?;
            item.sources.push(ConversationSource {
                source_id: member.source,
                relation: if member.copy.is_some() {
                    "copy"
                } else if base.is_some() {
                    "continues"
                } else {
                    "whole"
                }
                .into(),
                current: member.current,
                base_source_id: base.map(|predecessor| predecessor.source),
                base_end_ordinal: member.relation.base_end_ordinal,
                of_source_id: member.copy,
            });
        }
    }
    Ok(())
}

impl Archive {
    pub(super) fn source_presence(&self, items: &mut [SessionView]) -> Result<()> {
        let ids = items
            .iter()
            .flat_map(|item| &item.sources)
            .map(|source| source.source_id.clone())
            .collect::<Vec<_>>();
        let mut missing = HashMap::new();
        for batch in ids.chunks(128) {
            let ids = batch.iter().cloned().map(Value::Text).collect::<Vec<_>>();
            let placeholders = vec!["?"; ids.len()].join(",");
            let rows = self.store.read(Db::AgentHistoryEvidence, move |tx| {
                tx.query(
                    &format!(
                        "SELECT id,missing_since FROM ah_sources WHERE id IN ({placeholders})"
                    ),
                    ids,
                    |row| Ok((row.get::<String>(0)?, row.get::<Option<i64>>(1)?.is_some())),
                )
            })?;
            missing.extend(rows);
        }
        for item in items {
            item.only_in_archive = item
                .sources
                .iter()
                .all(|source| missing.get(&source.source_id).copied().unwrap_or(false));
        }
        Ok(())
    }
}
