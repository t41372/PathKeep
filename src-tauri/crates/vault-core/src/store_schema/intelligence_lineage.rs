//! Responsibilities: install v2 provenance identities, then finish a bounded startup backfill.
//! Not responsible for: vector files, serving selection or application reads during upgrades.
//! Setup and its version marker commit together. Each batch moves rows and commits the resume
//! state with them; readers start only after every row has moved.
use vault_store::{BackfillProgress, Migration, Session};

const BATCH_ROWS: i64 = 50_000;

/// Old index names stay on the renamed v1 table until the move has emptied it.
pub(super) fn migration() -> Migration {
    let observer = crate::vault::upgrade::observer();
    Migration::rust(2, apply).with_backfill(backfill).on_backfill_progress(move |progress| {
        if let Some(observer) = &observer {
            observer(crate::vault::upgrade::event(progress));
        }
    })
}

pub(super) fn apply(s: &Session<'_>) -> vault_store::Result<()> {
    for statement in [
        "ALTER TABLE ai_embeddings RENAME TO ai_embeddings_v1",
        "ALTER TABLE ai_embedding_totals RENAME TO ai_embedding_totals_v1",
        "CREATE TABLE ai_embeddings (
            id INTEGER PRIMARY KEY AUTOINCREMENT, history_id INTEGER NOT NULL,
            profile_id TEXT NOT NULL, url TEXT NOT NULL, title TEXT, domain TEXT NOT NULL,
            visited_at TEXT NOT NULL, content_hash TEXT NOT NULL, content_bytes INTEGER NOT NULL,
            metadata_bytes INTEGER NOT NULL DEFAULT 0, provider_id TEXT NOT NULL, model TEXT NOT NULL,
            indexed_at TEXT NOT NULL, lineage TEXT NOT NULL,
            UNIQUE(history_id,provider_id,model,lineage,content_hash))",
        "CREATE TABLE ai_embedding_totals (
            provider_id TEXT NOT NULL, model TEXT NOT NULL, item_count INTEGER NOT NULL,
            content_bytes INTEGER NOT NULL, metadata_bytes INTEGER NOT NULL, last_indexed_at TEXT,
            lineage TEXT NOT NULL, PRIMARY KEY(provider_id,model,lineage))",
        "CREATE INDEX idx_ai_embeddings_lineage_id ON ai_embeddings(provider_id,model,lineage,id)",
        "CREATE INDEX idx_ai_embeddings_lineage_history ON ai_embeddings(history_id,provider_id,model,lineage)",
        "CREATE TABLE ai_embedding_upgrade (
            id INTEGER PRIMARY KEY CHECK(id=1),phase TEXT NOT NULL,cursor INTEGER NOT NULL,
            processed INTEGER NOT NULL,total INTEGER NOT NULL)",
        "INSERT INTO ai_embedding_upgrade VALUES(1,'initialize',0,0,0)",
    ] {
        s.execute(statement, ())?;
    }
    Ok(())
}

/// One fixed-size transaction; the registry repeats it before starting any read pool.
pub(super) fn backfill(s: &Session<'_>) -> vault_store::Result<Option<BackfillProgress>> {
    let (phase, cursor, processed, total) = s.query_one(
        "SELECT phase,cursor,processed,total FROM ai_embedding_upgrade WHERE id=1",
        (),
        |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?, r.get::<i64>(3)?)),
    )?;
    let mut next_cursor = cursor;
    let (next_phase, changed, total) = match phase.as_str() {
        "done" => return Ok(None),
        "initialize" => {
            let count=s.query_one("SELECT (SELECT COUNT(*) FROM ai_embeddings_v1)+(SELECT COUNT(*) FROM ai_embedding_totals_v1)",(),|r| r.get::<i64>(0).map_err(Into::into))?;
            if count == 0 {
                s.execute("DROP TABLE ai_embeddings_v1", ())?;
                s.execute("DROP TABLE ai_embedding_totals_v1", ())?;
                ("done", 0, 0)
            } else {
                ("embeddings", 0, count)
            }
        }
        // Each batch moves its rows: the copy and the delete commit together, so the old table's
        // freed pages hold the next batch and the file never carries both copies at once.
        "embeddings" => {
            let changed = move_batch(
                s,
                "SELECT MAX(id) FROM (SELECT id FROM ai_embeddings_v1 ORDER BY id LIMIT ?1)",
                "INSERT INTO ai_embeddings SELECT id,history_id,profile_id,url,title,domain,visited_at,content_hash,content_bytes,metadata_bytes,provider_id,model,indexed_at,'legacy' FROM ai_embeddings_v1 WHERE id<=?1 ORDER BY id",
                "DELETE FROM ai_embeddings_v1 WHERE id<=?1",
                &mut next_cursor,
            )?;
            (if changed == 0 { "totals" } else { "embeddings" }, changed, total)
        }
        "totals" => {
            let changed = move_batch(
                s,
                "SELECT MAX(rowid) FROM (SELECT rowid FROM ai_embedding_totals_v1 ORDER BY rowid LIMIT ?1)",
                "INSERT INTO ai_embedding_totals(rowid,provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at,lineage) SELECT rowid,provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at,'legacy' FROM ai_embedding_totals_v1 WHERE rowid<=?1 ORDER BY rowid",
                "DELETE FROM ai_embedding_totals_v1 WHERE rowid<=?1",
                &mut next_cursor,
            )?;
            (if changed == 0 { "finish" } else { "totals" }, changed, total)
        }
        "finish" => {
            // Both old tables and their old indexes are empty: this commit stays small too.
            s.execute("DROP TABLE ai_embeddings_v1", ())?;
            s.execute("DROP TABLE ai_embedding_totals_v1", ())?;
            ("done", 0, total)
        }
        _ => {
            return Err(vault_store::Error::InvalidConfig(
                "unknown embedding upgrade phase".into(),
            ));
        }
    };
    let processed = processed + changed;
    s.execute(
        "UPDATE ai_embedding_upgrade SET phase=?1,cursor=?2,processed=?3,total=?4 WHERE id=1",
        vault_store::params![next_phase, next_cursor, processed, total],
    )?;
    Ok(Some(BackfillProgress {
        processed: processed as u64,
        total: total as u64,
        table: Some(
            if phase.ends_with("totals") { "ai_embedding_totals" } else { "ai_embeddings" }.into(),
        ),
        complete: next_phase == "done",
    }))
}

/// Moves the next batch by id range; the range bound is the batch's own last id, so no step
/// scans the growing new table.
fn move_batch(
    s: &Session<'_>,
    bound: &str,
    copy: &str,
    delete: &str,
    cursor: &mut i64,
) -> vault_store::Result<i64> {
    let Some(last) =
        s.query_one(bound, [BATCH_ROWS], |r| r.get::<Option<i64>>(0).map_err(Into::into))?
    else {
        *cursor = 0;
        return Ok(0);
    };
    let changed = s.execute(copy, [last])? as i64;
    s.execute(delete, [last])?;
    *cursor = last;
    Ok(changed)
}
