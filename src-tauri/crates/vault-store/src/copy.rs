//! Responsibilities: copy ordinary tables (and recreate their indexes and views)
//! through keyed ATTACH and verify logical content.
//! Not responsible for: choosing databases, swapping live files or rebuilding search.
//! Failure modes: lost rowids/high-water marks, omitted tables or views, changed typed values,
//! unbounded transactions, ignored cancel, plaintext destination, failed detach.
//! Tests: tests/maintenance.rs. No TEMP tables or VACUUM are used.
use crate::{CancelToken, Error, Result, engine::Engine};
use sha2::{Digest, Sha256};
use std::path::Path;

const PAGE_ROWS: i64 = 4096;

/// Logical proof is independent of page layout, cipher and database file size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDigest {
    pub table: String,
    pub rows: u64,
    pub sha256: String,
}

/// Emitted after each copy/verification page and before each index build.
#[derive(Debug, Clone)]
pub struct CopyProgress {
    pub table: String,
    pub copied: u64,
    pub total: u64,
    pub phase: CopyPhase,
}

/// Index creation is indeterminate; row copying and verification have exact counters.
#[derive(Debug, Clone, Copy)]
pub enum CopyPhase {
    Copying,
    Indexes,
    Verifying,
}

struct Table {
    name: String,
    sql: String,
}

pub(crate) fn database(
    engine: &Engine,
    destination: &Path,
    key: Option<&[u8; 32]>,
    cancel: &CancelToken,
    mut progress: impl FnMut(CopyProgress),
) -> Result<Vec<TableDigest>> {
    cancel.check()?;
    std::fs::create_dir_all(destination.parent().unwrap())?;
    let uri = crate::engine::scratch_uri(destination, key);
    engine.rt.block_on(engine.conn.execute("ATTACH ?1 AS pk_copy", [uri.as_str()]))?;
    execute(engine, "PRAGMA foreign_keys=OFF")?;
    let copied = crate::session::catch_job(|| copy_attached(engine, cancel, &mut progress));
    // An error must not leave a partial transaction or attachment on the writer.
    if !engine.conn.is_autocommit()? {
        engine.rt.block_on(engine.conn.execute("ROLLBACK", ()))?;
    }
    let detached = engine.rt.block_on(engine.conn.execute("DETACH pk_copy", ()));
    execute(engine, "PRAGMA foreign_keys=ON")?;
    let proof = copied?;
    detached?;
    Ok(proof)
}

fn copy_attached(
    engine: &Engine,
    cancel: &CancelToken,
    progress: &mut impl FnMut(CopyProgress),
) -> Result<Vec<TableDigest>> {
    let s = engine.session(false);
    let tables = s.query(
        "SELECT name,sql FROM sqlite_schema WHERE type='table' AND sql IS NOT NULL ORDER BY rowid",
        (),
        |r| Ok(Table { name: r.get(0)?, sql: r.get(1)? }),
    )?;
    let tables: Vec<_> = tables
        .into_iter()
        .filter(|t| !t.name.starts_with("sqlite_") && !t.name.starts_with("__turso_internal_"))
        .collect();
    let mut counts = std::collections::HashMap::new();
    for table in &tables {
        let body = table
            .sql
            .find('(')
            .ok_or_else(|| Error::InvalidConfig("copy requires ordinary rowid tables".into()))?;
        execute(
            engine,
            &format!("CREATE TABLE pk_copy.{} {}", quote(&table.name), &table.sql[body..]),
        )?;
    }
    for table in &tables {
        let name = quote(&table.name);
        let total = scalar(engine, &format!("SELECT count(*) FROM main.{name}"))? as u64;
        counts.insert(table.name.clone(), total);
        let column_names = columns(engine, &name)?;
        let mut cursor = None;
        let mut copied = 0;
        loop {
            cancel.check()?;
            let filter = cursor.map(|id| format!("WHERE rowid>{id}")).unwrap_or_default();
            let ids = engine.rt.block_on(async {
                let mut rows = engine.conn.query(
                    format!("SELECT rowid FROM main.{name} {filter} ORDER BY rowid LIMIT {PAGE_ROWS}"), (),
                ).await?;
                let mut ids = Vec::new();
                while let Some(row) = rows.next().await? {
                    ids.push(row.get::<i64>(0)?);
                }
                Ok::<_, Error>(ids)
            })?;
            let Some(last) = ids.last().copied() else { break };
            let first = ids[0];
            execute(engine, "BEGIN")?;
            execute(
                engine,
                &format!(
                    "INSERT INTO pk_copy.{name}(rowid,{}) SELECT rowid,{} FROM main.{name} WHERE rowid>={first} AND rowid<={last} ORDER BY rowid",
                    column_names, column_names
                ),
            )?;
            execute(engine, "COMMIT")?;
            checkpoint(engine, "pk_copy")?;
            copied += ids.len() as u64;
            cursor = Some(last);
            progress(CopyProgress {
                table: table.name.clone(),
                copied,
                total,
                phase: CopyPhase::Copying,
            });
        }
    }
    let has_sequence =
        s.query_one("SELECT count(*) FROM sqlite_schema WHERE name='sqlite_sequence'", (), |r| {
            Ok(r.get::<i64>(0)?)
        })? != 0;
    if has_sequence {
        counts.insert(
            "sqlite_sequence".into(),
            scalar(engine, "SELECT count(*) FROM main.sqlite_sequence")? as u64,
        );
        execute(engine, "BEGIN")?;
        execute(engine, "DELETE FROM pk_copy.sqlite_sequence")?;
        execute(
            engine,
            "INSERT INTO pk_copy.sqlite_sequence(rowid,name,seq) SELECT rowid,name,seq FROM main.sqlite_sequence ORDER BY rowid",
        )?;
        execute(engine, "COMMIT")?;
    }
    let indexes=s.query("SELECT name,tbl_name,sql FROM sqlite_schema WHERE type='index' AND sql IS NOT NULL ORDER BY rowid",(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?)))?;
    for (name, table, sql) in indexes {
        cancel.check()?;
        if name.starts_with("__turso_internal_") {
            continue;
        }
        progress(CopyProgress {
            table: name.clone(),
            copied: 0,
            total: 0,
            phase: CopyPhase::Indexes,
        });
        let body = sql
            .find('(')
            .ok_or_else(|| Error::InvalidConfig("copy requires ordinary indexes".into()))?;
        let unique =
            if sql.to_ascii_uppercase().starts_with("CREATE UNIQUE") { "UNIQUE " } else { "" };
        execute(
            engine,
            &format!(
                "CREATE {unique}INDEX pk_copy.{} ON {} {}",
                quote(&name),
                quote(&table),
                &sql[body..]
            ),
        )?;
        checkpoint(engine, "pk_copy")?;
    }
    // Views hold no rows; recreate each from its stored `CREATE VIEW name AS …`.
    let views = s.query(
        "SELECT name,sql FROM sqlite_schema WHERE type='view' AND sql IS NOT NULL ORDER BY rowid",
        (),
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
    )?;
    for (name, sql) in views {
        cancel.check()?;
        let body = sql
            .to_ascii_uppercase()
            .find(" AS ")
            .ok_or_else(|| Error::InvalidConfig("copy requires ordinary views".into()))?;
        execute(engine, &format!("CREATE VIEW pk_copy.{} {}", quote(&name), &sql[body + 1..]))?;
    }
    let mut proof = Vec::new();
    let names =
        tables.iter().map(|t| t.name.as_str()).chain(has_sequence.then_some("sqlite_sequence"));
    for table in names {
        let total = counts[table];
        let source = digest(engine, "main", table, cancel, &mut |copied| {
            progress(CopyProgress {
                table: table.into(),
                copied,
                total: total * 2,
                phase: CopyPhase::Verifying,
            })
        })?;
        let destination = digest(engine, "pk_copy", table, cancel, &mut |copied| {
            progress(CopyProgress {
                table: table.into(),
                copied: total + copied,
                total: total * 2,
                phase: CopyPhase::Verifying,
            })
        })?;
        if source != destination {
            return Err(Error::InvalidConfig(format!("copy-digest-mismatch: {table}")));
        }
        proof.push(source);
    }
    cancel.check()?;
    checkpoint(engine, "pk_copy")?;
    Ok(proof)
}

fn columns(engine: &Engine, table: &str) -> Result<String> {
    engine.rt.block_on(async {
        let rows = engine.conn.query(format!("SELECT * FROM main.{table} LIMIT 0"), ()).await?;
        Ok(rows.column_names().iter().map(|s| quote(s)).collect::<Vec<_>>().join(","))
    })
}

fn digest(
    engine: &Engine,
    schema: &str,
    table: &str,
    cancel: &CancelToken,
    progress: &mut impl FnMut(u64),
) -> Result<TableDigest> {
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut cursor = None;
    loop {
        cancel.check()?;
        let filter = cursor.map(|id| format!("WHERE rowid>{id}")).unwrap_or_default();
        let mut page_rows = 0;
        engine.rt.block_on(async {
            let mut rows = engine
                .conn
                .query(
                    format!(
                        "SELECT rowid,* FROM {schema}.{} {filter} ORDER BY rowid LIMIT {PAGE_ROWS}",
                        quote(table)
                    ),
                    (),
                )
                .await?;
            while let Some(row) = rows.next().await? {
                cursor = Some(row.get::<i64>(0)?);
                for i in 0..row.column_count() {
                    hash_value(&mut hash, row.get_value(i)?);
                }
                count += 1;
                page_rows += 1;
            }
            Ok::<_, Error>(())
        })?;
        progress(count);
        if page_rows < PAGE_ROWS {
            break;
        }
    }
    Ok(TableDigest { table: table.into(), rows: count, sha256: format!("{:x}", hash.finalize()) })
}

fn hash_value(hash: &mut Sha256, value: turso::Value) {
    match value {
        turso::Value::Null => hash.update([0]),
        turso::Value::Integer(v) => {
            hash.update([1]);
            hash.update(v.to_le_bytes());
        }
        turso::Value::Real(v) => {
            hash.update([2]);
            hash.update(v.to_bits().to_le_bytes());
        }
        turso::Value::Text(v) => {
            hash.update([3]);
            hash.update((v.len() as u64).to_le_bytes());
            hash.update(v.as_bytes());
        }
        turso::Value::Blob(v) => {
            hash.update([4]);
            hash.update((v.len() as u64).to_le_bytes());
            hash.update(v);
        }
    }
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn execute(engine: &Engine, sql: &str) -> Result<()> {
    engine.rt.block_on(engine.conn.execute(sql, ()))?;
    Ok(())
}

pub(crate) fn checkpoint(engine: &Engine, schema: &str) -> Result<()> {
    engine.rt.block_on(async {
        let mut rows =
            engine.conn.query(format!("PRAGMA {schema}.wal_checkpoint(TRUNCATE)"), ()).await?;
        let row = rows.next().await?.ok_or(turso::Error::QueryReturnedNoRows)?;
        let busy = row.get::<i64>(0)?;
        while rows.next().await?.is_some() {}
        if busy != 0 {
            return Err(Error::Busy {
                source: turso::Error::Busy("maintenance checkpoint".into()),
            });
        }
        Ok(())
    })
}

fn scalar(engine: &Engine, sql: &str) -> Result<i64> {
    engine.rt.block_on(async {
        let mut rows = engine.conn.query(sql, ()).await?;
        let row = rows.next().await?.ok_or(turso::Error::QueryReturnedNoRows)?;
        Ok(row.get(0)?)
    })
}
