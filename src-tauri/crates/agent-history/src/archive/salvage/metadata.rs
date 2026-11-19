//! Read optional facts in bounded pages. A bad source row is local to that
//! record. The destination callback's errors always abort the private build.
use super::*;
use vault_store::{Row, Value};

#[derive(Default, Debug)]
pub struct Diagnostics {
    pub skipped_records: u64,
    pub unreadable_sections: u64,
}

pub(crate) fn optional<T>(read: impl FnOnce() -> Result<T>) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(read)).ok()?.ok()
}

pub(crate) fn columns(s: &Session<'_>, table: &str, required: &[&str]) -> bool {
    optional(|| s.table_columns(table))
        .is_some_and(|cols| required.iter().all(|name| cols.iter().any(|c| c == name)))
}

/// The selected fields and table are defined by the archive, never by a user.
pub(crate) fn scan<T>(
    s: &Session<'_>,
    table: &str,
    fields: &str,
    diagnostics: &mut Diagnostics,
    decode: impl Fn(&Row) -> Result<T>,
    mut consume: impl FnMut(Vec<T>) -> Result<()>,
) -> Result<bool> {
    if !columns(s, table, &fields.split(',').collect::<Vec<_>>()) {
        diagnostics.unreadable_sections += 1;
        return Ok(false);
    }
    let query =
        format!("SELECT rowid,{fields} FROM {table} WHERE rowid>?1 ORDER BY rowid LIMIT 64");
    let single = format!("SELECT rowid,{fields} FROM {table} WHERE rowid=?1");
    let mut after = 0;
    let mut complete = true;
    loop {
        let page = optional(|| {
            s.query(&query, [after], |r| Ok((r.get::<i64>(0)?, optional(|| decode(r)))))
        });
        let page = match page {
            Some(rows) => rows,
            None => {
                let Some(ids) = optional(|| {
                    s.query(
                        &format!(
                            "SELECT rowid FROM {table} WHERE rowid>?1 ORDER BY rowid LIMIT 64"
                        ),
                        [after],
                        |r| Ok(r.get::<i64>(0)?),
                    )
                }) else {
                    diagnostics.unreadable_sections += 1;
                    return Ok(false);
                };
                ids.into_iter()
                    .map(|id| (id, optional(|| s.query_one(&single, [id], &decode))))
                    .collect()
            }
        };
        let Some((last, _)) = page.last() else { break };
        after = *last;
        let mut batch = Vec::new();
        for (_, item) in page {
            if let Some(item) = item {
                batch.push(item)
            } else {
                diagnostics.skipped_records += 1;
                complete = false
            }
        }
        consume(batch)?;
    }
    Ok(complete)
}

fn valid_parser(version: &str, json: &str) -> bool {
    let limits = version.split_once(":record=").map(|(_, v)| v);
    let valid = limits.is_none_or(|v| {
        v.split_once(":decoded=").is_some_and(|(r, d)| {
            r.parse::<usize>().is_ok_and(|n| (1..=64 * 1024 * 1024).contains(&n))
                && d.parse::<u64>().is_ok_and(|n| n > 0)
        })
    });
    valid
        && !version.is_empty()
        && version.len() <= 256
        && serde_json::from_str::<crate::model::ParseReport>(json).is_ok()
}

pub(super) fn projection_facts(
    archive: &Archive,
    s: &Session<'_>,
    cancel: &CancelToken,
    diag: &mut Diagnostics,
) -> Result<()> {
    for (table, fields, predicate) in [
        (
            "ah_parse_reports",
            "revision_id,parser_version,report_json",
            "EXISTS(SELECT 1 FROM ah_revisions WHERE id=?1 AND state='complete')",
        ),
        (
            "ah_source_conversations",
            "source_id,conversation_id,is_child,revision_id",
            "EXISTS(SELECT 1 FROM ah_revisions WHERE id=?4 AND source_id=?1 AND state='complete')",
        ),
        (
            "ah_reference_bindings",
            "id,parent_revision,reference_key,target_revision,observed_at",
            "EXISTS(SELECT 1 FROM ah_revisions WHERE id=?2 AND state='complete') AND EXISTS(SELECT 1 FROM ah_revisions WHERE id=?4 AND state='complete')",
        ),
    ] {
        let params =
            (1..=fields.split(',').count()).map(|n| format!("?{n}")).collect::<Vec<_>>().join(",");
        let insert =
            format!("INSERT OR REPLACE INTO {table}({fields}) SELECT {params} WHERE {predicate}");
        let mut invalid = 0;
        scan(
            s,
            table,
            fields,
            diag,
            |r| (1..r.column_count()).map(|n| Ok(r.get_value(n)?)).collect::<Result<Vec<_>>>(),
            |batch| {
                cancel.check()?;
                let mut valid = Vec::new();
                for row in batch {
                    let accept = match (table, row.as_slice()) {
                        (
                            "ah_parse_reports",
                            [Value::Integer(_), Value::Text(v), Value::Text(j)],
                        ) => valid_parser(v, j),
                        (
                            "ah_source_conversations",
                            [
                                Value::Text(_),
                                Value::Text(id),
                                Value::Integer(0 | 1),
                                Value::Integer(_),
                            ],
                        ) => !id.is_empty(),
                        (
                            "ah_reference_bindings",
                            [
                                Value::Integer(_),
                                Value::Integer(_),
                                Value::Text(_),
                                Value::Integer(_),
                                Value::Integer(_),
                            ],
                        ) => true,
                        _ => false,
                    };
                    if accept { valid.push(row) } else { invalid += 1 }
                }
                let sql = insert.clone();
                invalid += archive.store.write(EVIDENCE, Priority::Batch, move |tx| {
                    let mut unavailable = 0;
                    for row in valid {
                        if tx.execute(&sql, row)? == 0 {
                            unavailable += 1;
                        }
                    }
                    Ok(unavailable)
                })?;
                Ok(())
            },
        )?;
        diag.skipped_records += invalid;
    }
    Ok(())
}

/// A missing or undecodable field leaves the independently readable mirror field
/// intact. A decoded SQL NULL is the canonical instruction to clear a title or trash mark.
#[derive(Clone)]
struct Mark {
    id: String,
    star: Option<i64>,
    away: Option<i64>,
    trash: Option<Option<i64>>,
    title: Option<Option<String>>,
}
impl Mark {
    fn apply(self, tx: &Session<'_>) -> Result<()> {
        let has_trash = self.trash.is_some();
        let has_title = self.title.is_some();
        tx.execute("INSERT INTO ah_user_state VALUES(?1,coalesce(?2,0),coalesce(?3,0),?4,?5)
            ON CONFLICT(conversation_id) DO UPDATE SET starred=coalesce(?2,starred),put_away=coalesce(?3,put_away),
            trashed_at=CASE WHEN ?6=1 THEN excluded.trashed_at ELSE trashed_at END,
            title=CASE WHEN ?7=1 THEN excluded.title ELSE title END",
            params![self.id,self.star,self.away,self.trash.flatten(),self.title.flatten(),i64::from(has_trash),i64::from(has_title)])?;
        Ok(())
    }
    fn complete(&self) -> bool {
        self.star.is_some() && self.away.is_some() && self.trash.is_some() && self.title.is_some()
    }
}
impl Archive {
    /// Merge only marks for identities actually published by the salvaged model.
    /// A readable canonical absence clears the mirrored candidate for that ID.
    pub fn recover_salvaged_marks(
        &self,
        config: &Config,
        trust_mirror: bool,
        cancel: &CancelToken,
        diag: &mut Diagnostics,
    ) -> Result<()> {
        for role in [Db::AgentHistoryReadModel, EVIDENCE] {
            if role == Db::AgentHistoryReadModel && !trust_mirror {
                continue;
            }
            let mut entered = false;
            let result = Store::inspect_existing(config, role, |s| {
                entered = true;
                let names = optional(|| s.table_columns("ah_user_state")).unwrap_or_default();
                let has = |name: &str| names.iter().any(|n| n == name);
                if !has("conversation_id") {
                    diag.unreadable_sections += 1;
                    return Ok(());
                }
                let fields = ["conversation_id", "starred", "put_away", "trashed_at", "title"]
                    .map(|f| if has(f) { f } else { "NULL" })
                    .join(",");
                let mark = |r: &Row| -> Result<Mark> {
                    let flag = |i| r.get::<i64>(i).ok().filter(|n| matches!(n, 0 | 1));
                    Ok(Mark {
                        id: r.get(0)?,
                        star: flag(1),
                        away: flag(2),
                        trash: if has("trashed_at") { r.get::<Option<i64>>(3).ok() } else { None },
                        title: if has("title") { r.get::<Option<String>>(4).ok() } else { None },
                    })
                };
                let mut after = String::new();
                loop {
                    cancel.check()?;
                    let cursor = after.clone();
                    let ids=self.store.read(Db::AgentHistoryReadModel,move|s|s.query("SELECT DISTINCT conversation_id FROM ah_sessions WHERE conversation_current=1 AND conversation_id>?1 ORDER BY conversation_id LIMIT 128",[cursor],|r|Ok(r.get::<String>(0)?)))?;
                    let Some(last) = ids.last() else { break };
                    after = last.clone();

                    let placeholders = vec!["?"; ids.len()].join(",");
                    let mut clear = vec![];
                    let mut rows = vec![];
                    if let Some(batch) = optional(|| {
                        s.query(&format!("SELECT {fields} FROM ah_user_state WHERE conversation_id IN ({placeholders})"),vault_store::params_from_iter(ids.clone()),mark)
                    }) {
                        rows = batch;
                        clear = ids;
                    } else {
                        for id in ids {
                            match optional(|| {
                                s.query_one(&format!("SELECT {fields} FROM ah_user_state WHERE conversation_id=?1"),[id.as_str()],mark).optional()
                            }) {
                                Some(value) => {
                                    clear.push(id);
                                    rows.extend(value);
                                }
                                None => diag.skipped_records += 1,
                            }
                        }
                    }
                    let present =
                        rows.iter().map(|m| &m.id).collect::<std::collections::HashSet<_>>();
                    clear.retain(|id| !present.contains(id));
                    diag.skipped_records += rows.iter().filter(|m| !m.complete()).count() as u64;
                    for dest in [EVIDENCE, Db::AgentHistoryReadModel] {
                        let clear = clear.clone();
                        let rows = rows.clone();
                        self.store.write(dest, Priority::Batch, move |tx| {
                            if role == EVIDENCE {
                                for id in clear {
                                    tx.execute(
                                        "DELETE FROM ah_user_state WHERE conversation_id=?1",
                                        [id],
                                    )?;
                                }
                            }
                            for mark in rows {
                                mark.apply(tx)?;
                            }
                            Ok(())
                        })?;
                    }
                }
                Ok(())
            });
            if entered {
                result?
            } else if result.is_err() {
                diag.unreadable_sections += 1
            }
        }
        Ok(())
    }
}
