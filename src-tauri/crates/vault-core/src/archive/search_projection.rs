//! Search projection storage for lexical recall.
//!
//! The canonical archive keeps source-of-truth visits and URLs. Keyword recall
//! lives in a separate SQLite plane so FTS and projection rebuilds do not
//! bloat the hot archive file.

use crate::{
    archive::open_archive_connection,
    config::{ProjectPaths, ensure_paths},
    models::AppConfig,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::time::Duration as StdDuration;

const SEARCH_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS search_documents (
  url_id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT NOT NULL,
  search_terms TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS history_search USING fts5(
  url,
  title,
  search_terms,
  content='search_documents',
  content_rowid='url_id',
  tokenize = 'unicode61 remove_diacritics 2'
);
"#;

pub(crate) fn ensure_search_projection_bootstrapped(paths: &ProjectPaths) -> Result<()> {
    ensure_paths(paths)?;
    let connection = open_search_connection(paths)?;
    connection.execute_batch(SEARCH_SCHEMA_SQL)?;
    Ok(())
}

pub(crate) fn attach_search_database(archive: &Connection, paths: &ProjectPaths) -> Result<()> {
    ensure_search_projection_bootstrapped(paths)?;
    let target = paths.search_database_path.display().to_string().replace('\'', "''");
    archive.execute_batch(&format!("ATTACH DATABASE '{target}' AS search KEY '';"))?;
    Ok(())
}

pub(crate) fn seed_search_projection_if_missing(
    archive: &Connection,
    paths: &ProjectPaths,
) -> Result<()> {
    let projected_documents: i64 = archive
        .query_row("SELECT COUNT(*) FROM search.search_documents", [], |row| row.get(0))
        .unwrap_or_default();
    if projected_documents > 0 {
        return Ok(());
    }

    let canonical_urls: i64 =
        archive.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0))?;
    if canonical_urls == 0 {
        return Ok(());
    }

    rebuild_search_projection_from_archive(archive, paths)
}

pub(crate) fn rebuild_search_projection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<()> {
    ensure_search_projection_bootstrapped(paths)?;
    let archive = open_archive_connection(paths, config, key)?;
    rebuild_search_projection_from_archive(&archive, paths)
}

pub(crate) fn refresh_search_projection_for_import_batch(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    import_batch_id: i64,
) -> Result<()> {
    ensure_search_projection_bootstrapped(paths)?;
    let archive = open_archive_connection(paths, config, key)?;
    let mut search = open_search_connection(paths)?;
    let transaction = search.transaction()?;
    let mut statement = archive.prepare(
        "SELECT DISTINCT
           urls.id,
           urls.url,
           COALESCE(urls.title, ''),
           COALESCE(
             (
               SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
               FROM search_terms
               WHERE search_terms.url_id = urls.id
                 AND search_terms.source_profile_id = urls.source_profile_id
                 AND search_terms.reverted_at IS NULL
             ),
             ''
           )
         FROM visits
         JOIN urls ON urls.id = visits.url_id
         WHERE visits.import_batch_id = ?1",
    )?;
    let mut rows = statement.query(params![import_batch_id])?;
    while let Some(row) = rows.next()? {
        refresh_search_document(
            &transaction,
            row.get::<_, i64>(0)?,
            &row.get::<_, String>(1)?,
            &row.get::<_, String>(2)?,
            &row.get::<_, String>(3)?,
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn rebuild_search_projection_from_archive(
    archive: &Connection,
    paths: &ProjectPaths,
) -> Result<()> {
    let mut search = open_search_connection(paths)?;
    let transaction = search.transaction()?;
    transaction.execute("DELETE FROM search_documents", [])?;
    transaction.execute("INSERT INTO history_search(history_search) VALUES('delete-all')", [])?;

    let mut statement = archive.prepare(
        "SELECT
       urls.id,
       urls.url,
       COALESCE(urls.title, ''),
       COALESCE(
         (
           SELECT REPLACE(GROUP_CONCAT(DISTINCT search_terms.normalized_term), ',', ' ')
           FROM search_terms
           WHERE search_terms.url_id = urls.id
             AND search_terms.source_profile_id = urls.source_profile_id
             AND search_terms.reverted_at IS NULL
         ),
         ''
       )
     FROM urls",
    )?;
    let mut rows = statement.query([])?;
    let mut insert = transaction.prepare(
        "INSERT INTO search_documents (url_id, url, title, search_terms, updated_at)
     VALUES (?1, ?2, ?3, ?4, datetime('now'))",
    )?;
    while let Some(row) = rows.next()? {
        insert.execute(params![
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ])?;
    }
    drop(insert);
    transaction.execute("INSERT INTO history_search(history_search) VALUES('rebuild')", [])?;
    transaction.commit()?;
    Ok(())
}

fn refresh_search_document(
    transaction: &rusqlite::Transaction<'_>,
    url_id: i64,
    url: &str,
    title: &str,
    search_terms: &str,
) -> Result<()> {
    if let Some((old_url, old_title, old_search_terms)) = transaction
        .query_row(
            "SELECT url, title, search_terms
             FROM search_documents
             WHERE url_id = ?1",
            params![url_id],
            |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            },
        )
        .optional()?
    {
        transaction.execute(
            "INSERT INTO history_search(history_search, rowid, url, title, search_terms)
             VALUES('delete', ?1, ?2, ?3, ?4)",
            params![url_id, old_url, old_title, old_search_terms],
        )?;
    }

    transaction.execute(
        "INSERT INTO search_documents (url_id, url, title, search_terms, updated_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))
         ON CONFLICT(url_id) DO UPDATE SET
           url = excluded.url,
           title = excluded.title,
           search_terms = excluded.search_terms,
           updated_at = excluded.updated_at",
        params![url_id, url, title, search_terms],
    )?;
    transaction.execute(
        "INSERT INTO history_search(rowid, url, title, search_terms)
         VALUES(?1, ?2, ?3, ?4)",
        params![url_id, url, title, search_terms],
    )?;
    Ok(())
}

fn open_search_connection(paths: &ProjectPaths) -> Result<Connection> {
    let connection = Connection::open(&paths.search_database_path)
        .with_context(|| format!("opening {}", paths.search_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    Ok(connection)
}
