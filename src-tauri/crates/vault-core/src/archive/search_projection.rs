//! Search projection storage for lexical recall.
//!
//! The canonical archive keeps source-of-truth visits and URLs. Keyword recall
//! lives in a separate SQLite plane so FTS and projection rebuilds do not
//! bloat the hot archive file.

use super::search_lexical::{LexicalDocument, analyze_document};
use crate::{
    archive::open_archive_connection,
    config::{ProjectPaths, ensure_paths},
    models::AppConfig,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::time::Duration as StdDuration;

const SEARCH_PROJECTION_SCHEMA_VERSION: i64 = 2;

const SEARCH_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS search_projection_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS search_documents (
  url_id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT NOT NULL,
  search_terms TEXT NOT NULL,
  normalized_url TEXT NOT NULL,
  normalized_title TEXT NOT NULL,
  normalized_search_terms TEXT NOT NULL,
  compact_text TEXT NOT NULL,
  cjk_grams TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS history_search_terms USING fts5(
  url,
  title,
  search_terms,
  normalized_url,
  normalized_title,
  normalized_search_terms,
  cjk_grams,
  content='search_documents',
  content_rowid='url_id',
  tokenize = 'unicode61 remove_diacritics 2',
  prefix = '2 3 4'
);
CREATE VIRTUAL TABLE IF NOT EXISTS history_search_trigram USING fts5(
  compact_text,
  content='search_documents',
  content_rowid='url_id',
  tokenize = 'trigram'
);
"#;

const RESET_SEARCH_SCHEMA_SQL: &str = r#"
DROP TABLE IF EXISTS history_search;
DROP TABLE IF EXISTS history_search_terms;
DROP TABLE IF EXISTS history_search_trigram;
DROP TABLE IF EXISTS search_documents;
DROP TABLE IF EXISTS search_projection_meta;
"#;

pub(crate) fn ensure_search_projection_bootstrapped(paths: &ProjectPaths) -> Result<()> {
    ensure_paths(paths)?;
    let connection = open_search_connection(paths)?;
    ensure_search_schema(&connection)?;
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
    ensure_search_schema(&search)?;
    let transaction = search.transaction()?;
    transaction.execute("DELETE FROM search_documents", [])?;
    transaction.execute(
        "INSERT INTO history_search_terms(history_search_terms) VALUES('delete-all')",
        [],
    )?;
    transaction.execute(
        "INSERT INTO history_search_trigram(history_search_trigram) VALUES('delete-all')",
        [],
    )?;

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
        "INSERT INTO search_documents (
           url_id,
           url,
           title,
           search_terms,
           normalized_url,
           normalized_title,
           normalized_search_terms,
           compact_text,
           cjk_grams,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
    )?;
    while let Some(row) = rows.next()? {
        let url = row.get::<_, String>(1)?;
        let title = row.get::<_, String>(2)?;
        let search_terms = row.get::<_, String>(3)?;
        let lexical = analyze_document(&url, &title, &search_terms)?;
        insert.execute(params![
            row.get::<_, i64>(0)?,
            url,
            title,
            search_terms,
            lexical.normalized_url,
            lexical.normalized_title,
            lexical.normalized_search_terms,
            lexical.compact_text,
            lexical.cjk_grams,
        ])?;
    }
    drop(insert);
    transaction
        .execute("INSERT INTO history_search_terms(history_search_terms) VALUES('rebuild')", [])?;
    transaction.execute(
        "INSERT INTO history_search_trigram(history_search_trigram) VALUES('rebuild')",
        [],
    )?;
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
    let lexical = analyze_document(url, title, search_terms)?;

    if let Some(old_document) = transaction
        .query_row(
            "SELECT
               url,
               title,
               search_terms,
               normalized_url,
               normalized_title,
               normalized_search_terms,
               compact_text,
               cjk_grams
             FROM search_documents
             WHERE url_id = ?1",
            params![url_id],
            |row| {
                Ok(StoredSearchDocument {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    search_terms: row.get(2)?,
                    lexical: LexicalDocument {
                        normalized_url: row.get(3)?,
                        normalized_title: row.get(4)?,
                        normalized_search_terms: row.get(5)?,
                        compact_text: row.get(6)?,
                        cjk_grams: row.get(7)?,
                    },
                })
            },
        )
        .optional()?
    {
        transaction.execute(
            "INSERT INTO history_search_terms(
               history_search_terms,
               rowid,
               url,
               title,
               search_terms,
               normalized_url,
               normalized_title,
               normalized_search_terms,
               cjk_grams
             )
             VALUES('delete', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                url_id,
                &old_document.url,
                &old_document.title,
                &old_document.search_terms,
                &old_document.lexical.normalized_url,
                &old_document.lexical.normalized_title,
                &old_document.lexical.normalized_search_terms,
                &old_document.lexical.cjk_grams,
            ],
        )?;
        transaction.execute(
            "INSERT INTO history_search_trigram(history_search_trigram, rowid, compact_text)
             VALUES('delete', ?1, ?2)",
            params![url_id, &old_document.lexical.compact_text],
        )?;
    }

    transaction.execute(
        "INSERT INTO search_documents (
           url_id,
           url,
           title,
           search_terms,
           normalized_url,
           normalized_title,
           normalized_search_terms,
           compact_text,
           cjk_grams,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
         ON CONFLICT(url_id) DO UPDATE SET
           url = excluded.url,
           title = excluded.title,
           search_terms = excluded.search_terms,
           normalized_url = excluded.normalized_url,
           normalized_title = excluded.normalized_title,
           normalized_search_terms = excluded.normalized_search_terms,
           compact_text = excluded.compact_text,
           cjk_grams = excluded.cjk_grams,
           updated_at = excluded.updated_at",
        params![
            url_id,
            url,
            title,
            search_terms,
            &lexical.normalized_url,
            &lexical.normalized_title,
            &lexical.normalized_search_terms,
            &lexical.compact_text,
            &lexical.cjk_grams,
        ],
    )?;
    transaction.execute(
        "INSERT INTO history_search_terms(
           rowid,
           url,
           title,
           search_terms,
           normalized_url,
           normalized_title,
           normalized_search_terms,
           cjk_grams
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            url_id,
            url,
            title,
            search_terms,
            &lexical.normalized_url,
            &lexical.normalized_title,
            &lexical.normalized_search_terms,
            &lexical.cjk_grams,
        ],
    )?;
    transaction.execute(
        "INSERT INTO history_search_trigram(rowid, compact_text)
         VALUES(?1, ?2)",
        params![url_id, &lexical.compact_text],
    )?;
    Ok(())
}

struct StoredSearchDocument {
    url: String,
    title: String,
    search_terms: String,
    lexical: LexicalDocument,
}

fn ensure_search_schema(connection: &Connection) -> Result<()> {
    let current_version = search_schema_version(connection)?;
    if current_version != Some(SEARCH_PROJECTION_SCHEMA_VERSION) {
        connection.execute_batch(RESET_SEARCH_SCHEMA_SQL)?;
    }
    connection.execute_batch(SEARCH_SCHEMA_SQL)?;
    connection.execute(
        "INSERT INTO search_projection_meta (key, value)
         VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SEARCH_PROJECTION_SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

fn search_schema_version(connection: &Connection) -> Result<Option<i64>> {
    let has_meta = connection
        .query_row(
            "SELECT 1
             FROM sqlite_master
             WHERE type = 'table'
               AND name = 'search_projection_meta'
             LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_meta {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT value
             FROM search_projection_meta
             WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|value| value.parse::<i64>().context("parsing search projection schema version"))
        .transpose()
}

fn open_search_connection(paths: &ProjectPaths) -> Result<Connection> {
    let connection = Connection::open(&paths.search_database_path)
        .with_context(|| format!("opening {}", paths.search_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::project_paths_with_root,
        models::{AppConfig, ArchiveMode},
    };
    use rusqlite::named_params;
    use tempfile::tempdir;

    #[test]
    fn bundled_sqlcipher_supports_required_fts_features() {
        let connection = Connection::open_in_memory().expect("open memory db");
        let sqlite_version: String = connection
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .expect("sqlite version");
        let cipher_version: String = connection
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .expect("sqlcipher version");
        let compile_options = connection
            .prepare("PRAGMA compile_options")
            .expect("compile options statement")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("compile options")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect compile options");

        assert!(!sqlite_version.is_empty());
        assert!(cipher_version.contains("4."));
        assert!(compile_options.iter().any(|option| option == "ENABLE_FTS5"));

        connection
            .execute(
                "CREATE VIRTUAL TABLE unicode_probe
                 USING fts5(value, tokenize='unicode61 remove_diacritics 2')",
                [],
            )
            .expect("unicode fts table");
        connection
            .execute("INSERT INTO unicode_probe(value) VALUES ('cafe GitHub actions')", [])
            .expect("unicode fts row");
        let unicode_hits: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM unicode_probe
                 WHERE unicode_probe MATCH 'cafe AND github*'",
                [],
                |row| row.get(0),
            )
            .expect("unicode hits");
        assert_eq!(unicode_hits, 1);

        connection
            .execute(
                "CREATE VIRTUAL TABLE trigram_probe
                 USING fts5(value, tokenize='trigram')",
                [],
            )
            .expect("trigram fts table");
        connection
            .execute("INSERT INTO trigram_probe(value) VALUES ('abcdef'), ('中文搜尋')", [])
            .expect("trigram rows");
        let trigram_hits: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM trigram_probe
                 WHERE trigram_probe MATCH 'bcd'",
                [],
                |row| row.get(0),
            )
            .expect("trigram hits");
        let cjk_hits: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM trigram_probe
                 WHERE trigram_probe MATCH '中文搜'",
                [],
                |row| row.get(0),
            )
            .expect("cjk trigram hits");
        let bm25_score: f64 = connection
            .query_row(
                "SELECT bm25(trigram_probe)
                 FROM trigram_probe
                 WHERE trigram_probe MATCH 'abc'
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("bm25 score");
        assert_eq!(trigram_hits, 1);
        assert_eq!(cjk_hits, 1);
        assert!(bm25_score.is_finite());
    }

    #[test]
    fn encrypted_archive_attaches_plaintext_search_projection() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..AppConfig::default() };
        let archive =
            open_archive_connection(&paths, &config, Some("test-key")).expect("archive open");
        let search_schema_version: String = archive
            .query_row(
                "SELECT value
                 FROM search.search_projection_meta
                 WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("search schema version");
        assert_eq!(search_schema_version, SEARCH_PROJECTION_SCHEMA_VERSION.to_string());

        archive
            .execute(
                "INSERT INTO search.search_documents (
                   url_id,
                   url,
                   title,
                   search_terms,
                   normalized_url,
                   normalized_title,
                   normalized_search_terms,
                   compact_text,
                   cjk_grams,
                   updated_at
                 )
                 VALUES (1, 'https://github.com', 'GitHub 設定', '', 'https://github.com', 'github 设置', '', 'github设置', '设置', datetime('now'))",
                [],
            )
            .expect("insert attached search document");
        archive
            .execute(
                "INSERT INTO search.history_search_terms(
                   rowid,
                   url,
                   title,
                   search_terms,
                   normalized_url,
                   normalized_title,
                   normalized_search_terms,
                   cjk_grams
                 )
                 VALUES(1, 'https://github.com', 'GitHub 設定', '', 'https://github.com', 'github 设置', '', '设置')",
                [],
            )
            .expect("insert attached terms row");
        archive
            .execute(
                "INSERT INTO search.history_search_trigram(rowid, compact_text)
                 VALUES(1, 'github设置')",
                [],
            )
            .expect("insert attached trigram row");
        let terms_hits: i64 = archive
            .query_row(
                "SELECT COUNT(*)
                 FROM search.history_search_terms
                 WHERE history_search_terms MATCH :query",
                named_params! { ":query": "\"设置\"" },
                |row| row.get(0),
            )
            .expect("terms match");
        let trigram_hits: i64 = archive
            .query_row(
                "SELECT COUNT(*)
                 FROM search.history_search_trigram
                 WHERE history_search_trigram MATCH :query",
                named_params! { ":query": "\"github\"" },
                |row| row.get(0),
            )
            .expect("trigram match");
        assert_eq!(terms_hits, 1);
        assert_eq!(trigram_hits, 1);
    }
}
