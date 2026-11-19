//! Compare the original inventories and their revision heads with visible recovered
//! conversations. All joins live in the trial's scratch database and expire with it.
use super::{Diagnostics, metadata};
use vault_store::{CancelToken, Config, Db, Priority, Result, Store, params};

#[derive(Default)]
pub struct Counts {
    pub whole: Option<u64>,
    pub partial: Option<u64>,
    pub only_in_copy: Option<u64>,
}

pub fn compare(
    config: &Config,
    target: &Store,
    trust_model: bool,
    cancel: &CancelToken,
    diag: &mut Diagnostics,
) -> Result<Counts> {
    target.write(Db::Scratch,Priority::Batch,|s|{
        for sql in [
            "CREATE TABLE repair_original(id TEXT,src TEXT,rev INTEGER,visible INTEGER,PRIMARY KEY(id,src,rev,visible))",
            "CREATE INDEX repair_original_source ON repair_original(src, visible, id)",
            "CREATE TABLE repair_head(src TEXT PRIMARY KEY,rev INTEGER,kind TEXT)",
            "CREATE TABLE repair_view(id TEXT,src TEXT,rev INTEGER,PRIMARY KEY(id,src,rev))",
            "CREATE TABLE repair_raw(src TEXT,rev INTEGER,PRIMARY KEY(src,rev))",
        ]{s.execute(sql,())?;}
        Ok(())
    })?;
    let mut evidence_complete = false;
    let mut model_complete = false;
    let mut heads_complete = false;
    for db in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        if db == Db::AgentHistoryReadModel && !trust_model {
            continue;
        }
        let mut entered = false;
        let read = Store::inspect_existing(config, db, |s| {
            entered = true;
            let model = db == Db::AgentHistoryReadModel;
            let (table, fields) = if model {
                ("ah_sessions", "conversation_id,source_id,revision_id,conversation_current")
            } else {
                ("ah_source_conversations", "conversation_id,source_id,revision_id")
            };
            let complete = metadata::scan(
                s,
                table,
                fields,
                diag,
                |r| {
                    Ok((
                        r.get::<String>(1)?,
                        r.get::<String>(2)?,
                        r.get::<i64>(3)?,
                        !model || r.get::<i64>(4)? != 0,
                    ))
                },
                |rows| {
                    cancel.check()?;
                    target.write(Db::Scratch, Priority::Batch, move |tx| {
                        for (id, src, rev, include) in rows {
                            if include {
                                tx.execute(
                                    "INSERT OR IGNORE INTO repair_original VALUES(?1,?2,?3,?4)",
                                    params![id, src, rev, i64::from(model)],
                                )?;
                            }
                        }
                        Ok(())
                    })
                },
            )?;
            if model {
                model_complete = complete
            } else {
                evidence_complete = complete;
                heads_complete = metadata::scan(
                    s,
                    "ah_sources",
                    "id,latest_revision_id,kind",
                    diag,
                    |r| Ok((r.get::<String>(1)?, r.get::<Option<i64>>(2)?, r.get::<String>(3)?)),
                    |rows| {
                        cancel.check()?;
                        target.write(Db::Scratch, Priority::Batch, move |tx| {
                            for (src, rev, kind) in rows {
                                tx.execute(
                                    "INSERT INTO repair_head VALUES(?1,?2,?3)",
                                    params![src, rev, kind],
                                )?;
                            }
                            Ok(())
                        })
                    },
                )?;
            }
            Ok(())
        });
        if entered {
            read?
        } else if read.is_err() {
            diag.unreadable_sections += 1
        }
    }

    for (db, select, insert) in [
        (
            Db::AgentHistoryReadModel,
            "SELECT rowid,conversation_id,source_id,revision_id FROM ah_sessions WHERE conversation_current=1 AND rowid>?1 ORDER BY rowid LIMIT 256",
            "INSERT OR IGNORE INTO repair_view VALUES(?1,?2,?3)",
        ),
        (
            Db::AgentHistoryEvidence,
            "SELECT id,source_id,id FROM ah_revisions WHERE state='complete' AND bytes_retained=1 AND id>?1 ORDER BY id LIMIT 256",
            "INSERT INTO repair_raw VALUES(?1,?2)",
        ),
    ] {
        let mut after = 0;
        loop {
            cancel.check()?;
            let rows = target.read(db, move |s| {
                s.query(select, [after], |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        (1..r.column_count())
                            .map(|n| Ok(r.get_value(n)?))
                            .collect::<Result<Vec<_>>>()?,
                    ))
                })
            })?;
            let Some((last, _)) = rows.last() else { break };
            after = *last;
            target.write(Db::Scratch, Priority::Batch, move |s| {
                for (_, values) in rows {
                    s.execute(insert, values)?;
                }
                Ok(())
            })?;
        }
    }
    // Physical identity proves the fast path. A different revision or a missing
    // redundant source only nominates a conversation for a content comparison.
    target.write(Db::Scratch, Priority::Batch, |s| {
        s.execute("CREATE TABLE repair_partial(id TEXT PRIMARY KEY)", ())?;
        s.execute(
            "INSERT OR IGNORE INTO repair_partial SELECT o.id FROM repair_original o
             WHERE EXISTS (SELECT 1 FROM repair_view v WHERE v.id = o.id)
             AND (
                 NOT EXISTS (SELECT 1 FROM repair_raw r WHERE r.src=o.src AND r.rev=o.rev)
                 OR EXISTS (SELECT 1 FROM repair_head h WHERE h.src=o.src AND h.rev IS NOT NULL
                     AND NOT EXISTS (SELECT 1 FROM repair_raw r WHERE r.src=h.src AND r.rev=h.rev))
                 OR (o.visible=1 AND NOT EXISTS
                     (SELECT 1 FROM repair_view v WHERE v.id=o.id AND v.src=o.src AND v.rev=o.rev))
             )",
            (),
        )?;
        Ok(())
    })?;
    let comparable = compare_changed_lines(config, target, trust_model, cancel, diag)?;
    target.read(Db::Scratch, move |s| {
        let number = |sql| s.query_one(sql, (), |row| Ok(row.get::<i64>(0)? as u64));
        let returned = number("SELECT count(DISTINCT id) FROM repair_view")?;
        let partial = number("SELECT count(*) FROM repair_partial")?;
        let missing = number(
            "SELECT count(DISTINCT o.id) FROM repair_original o
            WHERE NOT EXISTS (SELECT 1 FROM repair_view v WHERE v.id=o.id)",
        )?;
        let known = number("SELECT EXISTS (SELECT 1 FROM repair_original)")? != 0;
        let unmapped = number(
            "SELECT EXISTS (SELECT 1 FROM repair_view v
            WHERE NOT EXISTS (SELECT 1 FROM repair_original o WHERE o.id=v.id))",
        )? != 0;
        let heads = number("SELECT EXISTS (SELECT 1 FROM repair_head WHERE rev IS NOT NULL)")? != 0;
        let conflict = number(
            "SELECT EXISTS (SELECT 1 FROM repair_original a
            JOIN repair_original b ON a.src=b.src AND a.id<>b.id AND a.visible<>b.visible)",
        )? != 0;
        // If even the identity of a missing transcript head is unknown, an
        // older empty view cannot establish how many conversations it represented.
        let unidentified = number(
            "SELECT EXISTS (SELECT 1 FROM repair_head h
            WHERE h.kind='transcript' AND h.rev IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM repair_raw r WHERE r.src=h.src AND r.rev=h.rev)
            AND NOT EXISTS (SELECT 1 FROM repair_original o WHERE o.src=h.src))",
        )? != 0;
        let inventory = !unidentified
            && !conflict
            && !unmapped
            && ((evidence_complete && known)
                || ((evidence_complete || model_complete) && heads_complete && !heads));
        let exact_heads = returned == 0 || (inventory && heads_complete && comparable);
        Ok(Counts {
            whole: exact_heads.then_some(returned - partial),
            partial: exact_heads.then_some(partial),
            only_in_copy: inventory.then_some(missing),
        })
    })
}

/// Only a source whose latest raw head was published can use that publication
/// as proof of its contents. An unreadable comparison leaves the count unknown.
fn compare_changed_lines(
    config: &Config,
    target: &Store,
    trust_model: bool,
    cancel: &CancelToken,
    diag: &mut Diagnostics,
) -> Result<bool> {
    let any =
        target.read(Db::Scratch, |s| {
            s.query_one("SELECT EXISTS(SELECT 1 FROM repair_partial)", (), |r| {
                Ok(r.get::<i64>(0)? != 0)
            })
        })?;
    if !any || !trust_model {
        return Ok(!any);
    }
    let mut known = true;
    let mut entered = false;
    let result = Store::inspect_existing(config, Db::AgentHistoryReadModel, |old| {
        entered = true;
        let mut after = String::new();
        loop {
            cancel.check()?;
            let ids = target.read(Db::Scratch, move |s| {
                s.query(
                    "SELECT p.id FROM repair_partial p WHERE p.id>?1 AND NOT EXISTS (
                    SELECT 1 FROM repair_original o JOIN repair_head h ON h.src=o.src
                    WHERE o.id=p.id AND h.rev<>o.rev AND NOT EXISTS (
                        SELECT 1 FROM repair_raw r WHERE r.src=h.src AND r.rev=h.rev))
                 ORDER BY p.id LIMIT 64",
                    [after],
                    |r| Ok(r.get::<String>(0)?),
                )
            })?;
            let Some(last) = ids.last() else { break };
            after = last.clone();
            let mut whole = Vec::new();
            for id in ids {
                cancel.check()?;
                let original =
                    metadata::optional(|| crate::query::content_digest(old, &id, cancel)).flatten();
                cancel.check()?;
                let Some(original) = original else {
                    known = false;
                    diag.unreadable_sections += 1;
                    continue;
                };
                let conversation = id.clone();
                let token = cancel.clone();
                let recovered = target.read(Db::AgentHistoryReadModel, move |s| {
                    crate::query::content_digest(s, &conversation, &token)
                })?;
                if recovered == Some(original) {
                    whole.push(id);
                }
            }
            target.write(Db::Scratch, Priority::Batch, move |s| {
                for id in whole {
                    s.execute("DELETE FROM repair_partial WHERE id=?1", [id])?;
                }
                Ok(())
            })?;
        }
        Ok(())
    });
    if entered {
        result?;
    } else {
        diag.unreadable_sections += 1;
    }
    Ok(entered && known)
}
