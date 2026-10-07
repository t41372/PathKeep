//! Responsibilities: merge legacy canonical collisions and verify normalized tables.
//! Not responsible for: intelligence execution, source mutation or live installation.
//! Failure modes: lost newest content/counters/sets, dangling job triggers, replayed
//! counters doubled, unbounded corpus maps, or unverified merged rows published.
//! Tests: tests/collisions.rs on real plaintext and encrypted archives.
mod record;
use crate::{
    source::{self, Row},
    transfer,
};
use anyhow::Result;
use record::Record;
use sha2::{Digest, Sha256};
use vault_core::{Vault, maintenance::Progress};
use vault_store::{CancelToken, Db, Priority, params};

// Refind pages used to collide here too; they are derived data now and the
// first Insights rebuild after conversion recomputes them.
const TABLES: [&str; 2] = ["intelligence_jobs", "intelligence_job_triggers"];

/// These natural keys can coalesce; ordinary row-for-row copying would lose data.
pub(crate) fn handles(table: &str) -> bool {
    TABLES.contains(&table)
}

/// Rebuild expected aggregates from immutable source rows on each attempt. Only
/// bounded pages and one colliding logical record are resident; Scratch holds the
/// corpus and job-ID aliases. Replaying an attempt cannot add its counters twice.
pub(crate) fn convert(
    vault: &Vault,
    source: &rusqlite::Connection,
    cancel: &CancelToken,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    let present = source::tables(source)?;
    vault.store().write(Db::Scratch,Priority::Batch,|s| {
        s.execute("DROP TABLE IF EXISTS conversion_merged",())?;
        s.execute("DROP TABLE IF EXISTS conversion_job_alias",())?;
        s.execute("CREATE TABLE conversion_merged(kind TEXT,key TEXT,row_id INTEGER,payload TEXT,PRIMARY KEY(kind,key))",())?;
        s.execute("CREATE INDEX conversion_merged_rows ON conversion_merged(kind,row_id)",())?;
        s.execute("CREATE TABLE conversion_job_alias(source_id INTEGER PRIMARY KEY,canonical_key TEXT)",())?;
        Ok(())
    })?;
    let mut fields = std::collections::BTreeMap::new();
    for table in TABLES {
        if !present.iter().any(|name| name == table) {
            continue;
        }
        let columns = transfer::fields(vault, Db::Intelligence, source, table)?;
        merge_table(vault, source, table, &columns, cancel, progress)?;
        fields.insert(table, columns);
    }
    // Children must be cleared first. This is an isolated staged Vault, and all
    // expected rows have already been rebuilt before replacing these tables.
    for table in ["intelligence_job_triggers", "intelligence_jobs"] {
        if !fields.contains_key(table) {
            continue;
        }
        loop {
            cancel.check()?;
            let removed = vault.store().write(Db::Intelligence,Priority::Batch,move |s| {
                s.execute(&format!("DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} ORDER BY rowid LIMIT 4096)"),())
            })?;
            if removed == 0 {
                break;
            }
        }
    }
    for table in TABLES {
        let Some(columns) = fields.get(table) else {
            continue;
        };
        let mut cursor = None;
        let mut processed = 0;
        loop {
            cancel.check()?;
            let rows = expected_page(vault, table, columns, cursor)?;
            if rows.is_empty() {
                break;
            }
            cursor = rows.last().map(|r| r.id);
            processed += rows.len() as u64;
            transfer::write(vault, Db::Intelligence, table, columns, rows)?;
            report(progress, "converting", table, processed, 0);
        }
        verify(vault, table, columns, cancel, progress)?;
    }
    vault.store().write(Db::Scratch, Priority::Batch, |s| {
        s.execute("DROP TABLE conversion_job_alias", ())?;
        s.execute("DROP TABLE conversion_merged", ())?;
        Ok(())
    })?;
    Ok(())
}

fn merge_table(
    vault: &Vault,
    source: &rusqlite::Connection,
    table: &'static str,
    fields: &[String],
    cancel: &CancelToken,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    let total = source::count(source, table)?;
    let mut cursor = None;
    let mut processed = 0;
    loop {
        cancel.check()?;
        let mut rows = source::page(source, table, fields, cursor)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|r| r.id);
        processed += rows.len() as u64;
        for row in &mut rows {
            transfer::normalize(table, fields, row)?;
        }
        let records = rows.into_iter().map(|row| Record::from_row(fields, row)).collect::<Vec<_>>();
        vault.store().write(Db::Scratch, Priority::Batch, move |s| {
            merge_records(s, table, records)
                .map_err(|error| vault_store::Error::Operation(error.into_boxed_dyn_error()))
        })?;
        report(progress, "merging", table, processed, total);
    }
    Ok(())
}

fn merge_records(s: &vault_store::Session<'_>, table: &str, records: Vec<Record>) -> Result<()> {
    for mut record in records {
        let key = record.key(table);
        if table == "intelligence_jobs" {
            s.execute(
                "INSERT INTO conversion_job_alias(source_id,canonical_key) VALUES(?1,?2)",
                params![record.id, key.as_str()],
            )?;
        } else if table == "intelligence_job_triggers" {
            let parent = s.query_one(
                        "SELECT m.row_id FROM conversion_job_alias a JOIN conversion_merged m ON m.kind='intelligence_jobs' AND m.key=a.canonical_key WHERE a.source_id=?1",
                        [record.job_id()], |r|Ok(r.get::<i64>(0)?),
                    )?;
            record.set_job_id(parent);
        }
        let previous = s
            .query(
                "SELECT payload FROM conversion_merged WHERE kind=?1 AND key=?2",
                [table, key.as_str()],
                |r| Ok(r.get::<String>(0)?),
            )?
            .pop();
        if let Some(previous) = previous {
            record = serde_json::from_str::<Record>(&previous)?.merge(record)?;
        }
        s.execute(
                    "INSERT INTO conversion_merged(kind,key,row_id,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(kind,key) DO UPDATE SET row_id=excluded.row_id,payload=excluded.payload",
                    params![table,key,record.id,serde_json::to_string(&record)?],
                )?;
    }
    Ok(())
}

fn expected_page(
    vault: &Vault,
    table: &'static str,
    fields: &[String],
    cursor: Option<i64>,
) -> Result<Vec<Row>> {
    let payloads = vault.store().read(Db::Scratch,move |s| {
        let filter = cursor.map(|id|format!("AND row_id>{id}")).unwrap_or_default();
        s.query(&format!("SELECT payload FROM conversion_merged WHERE kind=?1 {filter} ORDER BY row_id LIMIT 4096"),[table],|r|Ok(r.get::<String>(0)?))
    })?;
    payloads
        .into_iter()
        .map(|text| Ok(serde_json::from_str::<Record>(&text)?.row(fields)))
        .collect()
}

fn verify(
    vault: &Vault,
    table: &'static str,
    fields: &[String],
    cancel: &CancelToken,
    progress: &mut impl FnMut(Progress),
) -> Result<()> {
    let mut expected = Sha256::new();
    let mut cursor = None;
    let mut expected_count = 0;
    loop {
        cancel.check()?;
        let rows = expected_page(vault, table, fields, cursor)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|r| r.id);
        for row in rows {
            transfer::hash_row(&mut expected, &row);
            expected_count += 1;
        }
        report(progress, "verifying", table, expected_count, 0);
    }
    let mut actual = Sha256::new();
    let mut cursor = None;
    let mut actual_count = 0;
    loop {
        cancel.check()?;
        let rows = transfer::native_page(vault, Db::Intelligence, table, fields, cursor)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|r| r.id);
        for row in rows {
            transfer::hash_row(&mut actual, &row);
            actual_count += 1;
        }
        report(progress, "verifying", table, expected_count + actual_count, expected_count * 2);
    }
    anyhow::ensure!(
        actual_count == expected_count && actual.finalize() == expected.finalize(),
        "conversion-digest-mismatch: Intelligence.{table}"
    );
    Ok(())
}
fn report(
    progress: &mut impl FnMut(Progress),
    phase: &str,
    table: &str,
    processed: u64,
    total: u64,
) {
    progress(Progress {
        phase: phase.into(),
        database: Some("Intelligence".into()),
        table: Some(table.into()),
        processed,
        total,
    });
}
