//! Responsibilities: inspect and page read-only SQLCipher input files.
//! Not responsible for: native writes, normalization or swap decisions.
//! Failure modes: wrong password, missing optional database, whole-table allocation,
//! source rowids lost, accidental write to a legacy file. Tests: conversion acceptance.
use anyhow::Result;
use rusqlite::{Connection, OpenFlags, types::Value as LegacyValue};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use vault_core::ProjectPaths;
use vault_store::{Db, Value};

pub(crate) const PAGE: i64 = 4096;

pub(crate) fn files(paths: &ProjectPaths) -> [(Db, PathBuf); 4] {
    [
        (Db::Archive, paths.app_root.join("archive/history-vault.sqlite")),
        (Db::SourceEvidence, paths.app_root.join("archive/source-evidence.sqlite")),
        (Db::Intelligence, paths.derived_dir.join("history-intelligence.sqlite")),
        (Db::Agent, paths.derived_dir.join("agent.sqlite")),
    ]
}

pub(crate) fn open(path: &Path, password: Option<&str>) -> Result<Connection> {
    let encrypted = encrypted(path)?;
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if encrypted {
        db.pragma_update(
            None,
            "key",
            password.ok_or_else(|| {
                anyhow::anyhow!("archive-password-required: enter the archive password")
            })?,
        )?;
    }
    db.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get::<_, i64>(0)).map_err(
        |e| match e {
            rusqlite::Error::SqliteFailure(ref code, _)
                if code.code == rusqlite::ErrorCode::NotADatabase =>
            {
                anyhow::Error::new(vault_core::ArchivePasswordWrong)
            }
            other => other.into(),
        },
    )?;
    Ok(db)
}

pub(crate) fn encrypted(path: &Path) -> Result<bool> {
    let mut header = [0; 16];
    fs::File::open(path)?.read_exact(&mut header)?;
    Ok(&header != b"SQLite format 3\0")
}

pub(crate) fn tables(db: &Connection) -> Result<Vec<String>> {
    Ok(db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '%fts%' AND name NOT LIKE 'history_search%' AND name!='schema_migrations' AND name!='settings' ORDER BY rowid")?
        .query_map([],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?)
}
pub(crate) fn columns(db: &Connection, table: &str) -> Result<Vec<String>> {
    Ok(db
        .prepare(&format!("PRAGMA table_info({})", quote(table)))?
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<_>>()?)
}
pub(crate) fn count(db: &Connection, table: &str) -> Result<u64> {
    Ok(db
        .query_row(&format!("SELECT count(*) FROM {}", quote(table)), [], |r| r.get::<_, i64>(0))?
        as u64)
}

#[derive(Clone)]
pub(crate) struct Row {
    pub id: i64,
    pub values: Vec<Value>,
}

pub(crate) fn page(
    db: &Connection,
    table: &str,
    fields: &[String],
    after: Option<i64>,
) -> Result<Vec<Row>> {
    let filter = after.map(|id| format!("WHERE rowid>{id}")).unwrap_or_default();
    let mut statement = db.prepare(&format!(
        "SELECT rowid,{} FROM {} {filter} ORDER BY rowid LIMIT {PAGE}",
        fields.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
        quote(table)
    ))?;
    Ok(statement
        .query_map([], |row| {
            Ok(Row {
                id: row.get(0)?,
                values: (0..fields.len())
                    .map(|i| row.get::<_, LegacyValue>(i + 1).map(value))
                    .collect::<rusqlite::Result<_>>()?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}
fn value(value: LegacyValue) -> Value {
    match value {
        LegacyValue::Null => Value::Null,
        LegacyValue::Integer(v) => Value::Integer(v),
        LegacyValue::Real(v) => Value::Real(v),
        LegacyValue::Text(v) => Value::Text(v),
        LegacyValue::Blob(v) => Value::Blob(v),
    }
}
pub(crate) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
