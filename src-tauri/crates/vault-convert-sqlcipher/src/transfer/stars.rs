//! Responsibilities: verify normalized favorites, coalescing newly equivalent URL keys.
//! Not responsible for: archive ingestion or retaining source SQLCipher files.
//! Failure modes: canonical collisions drop the newest star time or source profile, require an
//! archive-sized map, rescan earlier keys, or bypass count/content verification.
//! Tests: conversion.rs and the indexed proof-page plan below.
use super::{hash_row, normalize, text};
use crate::source::{self, Row};
use anyhow::Result;
use sha2::{Digest, Sha256};
use vault_core::Vault;
use vault_store::{Db, Priority, Session, Value};

const AFTER_KEY: &str = "entity_kind=?1 AND entity_key>?2";

pub(super) fn verify(
    vault: &Vault,
    source: &rusqlite::Connection,
    fields: &[String],
    cancel: &vault_store::CancelToken,
) -> Result<()> {
    vault.store().write(Db::Scratch,Priority::Batch,|s|{
        s.execute("DROP TABLE IF EXISTS conversion_star_proof",())?;
        s.execute("CREATE TABLE conversion_star_proof(entity_kind TEXT,entity_key TEXT,starred_at TEXT,source_profile TEXT,PRIMARY KEY(entity_kind,entity_key))",())?;Ok(())
    })?;
    let mut cursor = None;
    loop {
        cancel.check()?;
        let mut rows = source::page(source, "star", fields, cursor)?;
        if rows.is_empty() {
            break;
        }
        cursor = rows.last().map(|r| r.id);
        for row in &mut rows {
            normalize("star", fields, row)?;
        }
        let fields = fields.to_vec();
        vault.store().write(Db::Scratch, Priority::Batch, move |s| {
            for row in rows {
                upsert(s, "conversion_star_proof", &fields, &row)?;
            }
            Ok(())
        })?;
    }
    let expected = star_digest(vault, Db::Scratch, "conversion_star_proof", cancel)?;
    let actual = star_digest(vault, Db::Archive, "star", cancel)?;
    anyhow::ensure!(expected == actual, "conversion-digest-mismatch: Archive.star");
    vault.store().write(Db::Scratch, Priority::Batch, |s| {
        s.execute("DROP TABLE conversion_star_proof", ())?;
        Ok(())
    })?;
    Ok(())
}

/// Canonical collisions retain the newest timestamp and its available source
/// profile. Replayed older pages cannot replace a newer non-null profile.
pub(super) fn upsert(
    s: &Session<'_>,
    table: &str,
    fields: &[String],
    row: &Row,
) -> vault_store::Result<()> {
    s.execute(
        &format!(
            "INSERT INTO {table}(entity_kind,entity_key,starred_at,source_profile)
            VALUES(?1,?2,?3,?4) ON CONFLICT(entity_kind,entity_key) DO UPDATE SET
            source_profile=CASE WHEN excluded.starred_at>={table}.starred_at
                THEN COALESCE(excluded.source_profile,{table}.source_profile)
                ELSE COALESCE({table}.source_profile,excluded.source_profile) END,
            starred_at=MAX({table}.starred_at,excluded.starred_at)"
        ),
        vault_store::params![
            text(fields, &row.values, "entity_kind").unwrap(),
            text(fields, &row.values, "entity_key").unwrap(),
            text(fields, &row.values, "starred_at").unwrap(),
            text(fields, &row.values, "source_profile"),
        ],
    )?;
    Ok(())
}

fn star_digest(
    vault: &Vault,
    db: Db,
    table: &'static str,
    cancel: &vault_store::CancelToken,
) -> Result<(u64, Vec<u8>)> {
    let mut cursor: Option<(String, String)> = None;
    let mut count = 0;
    let mut hash = Sha256::new();
    loop {
        cancel.check()?;
        let after = cursor.clone();
        let rows = vault.store().read(db, move |s| {
            let read = |r: &vault_store::Row| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<Option<String>>(3)?,
                ))
            };
            match after {
                None => s.query(&page_sql(table, "1"), (), read),
                Some((kind, key)) => {
                    let rows = s.query(&page_sql(table, AFTER_KEY), [kind.clone(), key], read)?;
                    if rows.is_empty() {
                        s.query(&page_sql(table, "entity_kind>?1"), [kind], read)
                    } else {
                        Ok(rows)
                    }
                }
            }
        })?;
        if rows.is_empty() {
            break;
        }
        for (kind, key, at, profile) in rows {
            cursor = Some((kind.clone(), key.clone()));
            hash_row(
                &mut hash,
                &Row {
                    id: 0,
                    values: vec![
                        Value::Text(kind),
                        Value::Text(key),
                        Value::Text(at),
                        profile.map(Value::Text).unwrap_or(Value::Null),
                    ],
                },
            );
            count += 1;
        }
    }
    Ok((count, hash.finalize().to_vec()))
}

fn page_sql(table: &str, predicate: &str) -> String {
    format!(
        "SELECT entity_kind,entity_key,starred_at,source_profile FROM {table} WHERE {predicate} ORDER BY entity_kind,entity_key LIMIT 1024"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_pages_seek_both_parts_of_the_favorite_key() {
        let vault = Vault::open_temp().unwrap();
        vault
            .store()
            .read(Db::Archive, |s| {
                let sql = format!("EXPLAIN QUERY PLAN {}", page_sql("star", AFTER_KEY));
                let plan =
                    s.query(&sql, ["url", "https://example.com/"], |r| Ok(r.get::<String>(3)?))?;
                assert!(
                    plan.iter().any(|line| line.contains("SEARCH") && line.contains("entity_key>")),
                    "{plan:?}"
                );
                Ok(())
            })
            .unwrap();
    }
}
