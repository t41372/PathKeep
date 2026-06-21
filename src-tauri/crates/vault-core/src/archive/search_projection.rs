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
use std::collections::HashMap;
use std::time::Duration as StdDuration;

// v3 (W-ENRICH-1): adds the `enrichment_text` column to `search_documents` + the term FTS mirror so
// a content-fetch enrichment's CAPPED summary + key metadata (GitHub topics/desc, video channel) are
// keyword-searchable. A version mismatch drops + rebuilds the projection (the projection is fully
// rebuildable derived state). NOTE: only the capped summary + metadata ride here — NOT the full
// body/caption (06 §4: multi-KB blobs would explode the index size + wreck BM25 at the 14.4M tail).
const SEARCH_PROJECTION_SCHEMA_VERSION: i64 = 3;

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
  enrichment_text TEXT NOT NULL DEFAULT '',
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
  enrichment_text,
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

    // The seed runs during archive open (no enrichment available yet); enrichment text is mirrored in
    // by a later `rebuild_search_projection` once content has been fetched. Seed with an empty map.
    rebuild_search_projection_from_archive(archive, paths)
}

pub(crate) fn rebuild_search_projection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<()> {
    ensure_search_projection_bootstrapped(paths)?;
    let archive = open_archive_connection(paths, config, key)?;
    // Load the per-URL enrichment text (capped summary + key metadata, 06 §4) from the intelligence
    // DB so the rebuild mirrors it into the FTS `enrichment_text` column. The full body never rides.
    let enrichment_text = load_enrichment_text_by_url_id(paths, config, key).unwrap_or_default();
    rebuild_search_projection_from_archive_with_enrichment(&archive, paths, &enrichment_text)
}

/// Refreshes the FTS `enrichment_text` for ONE visit's URL right after a content fetch (W-ENRICH-1 CORR-1).
///
/// A `rebuild_search_projection` is the only thing that previously mirrored enrichment into FTS, so a
/// fetch-now / working-set fetch left the URL un-findable by keyword until the next FULL rebuild. This
/// closes that gap WITHOUT a full rebuild: it (1) resolves the visit's `url_id` via the intelligence
/// connection's attached `archive` schema (mirroring `latest_visit_for_canonical`), (2) recomputes the
/// CAPPED `enrichment_text` from the just-stored best row for that URL, and (3) UPDATEs the existing
/// `search_documents` row + re-mirrors the term FTS (delete-then-insert, the same pattern
/// `refresh_search_document` uses so the contentless FTS stays consistent). Best-effort + idempotent:
/// when the URL has no `search_documents` row yet (the projection has not been seeded) it is a no-op —
/// the eventual rebuild/seed will pick the enrichment up. `connection` is the INTELLIGENCE connection
/// (it owns `visit_content_enrichments` + the attached archive); `paths` opens the SEARCH database.
///
/// Siblings of one page share ONE canonical URL ⇒ ONE `url_id`, so a single refresh keyed off any one
/// of the fanned-out visits covers them all (CORR-1 ↔ CORR-3).
pub(crate) fn refresh_enrichment_text_for_history(
    paths: &ProjectPaths,
    connection: &Connection,
    history_id: i64,
) -> Result<()> {
    // (1) Resolve the visit's url_id via the attached archive (same join shape as the working-set
    // resolver). A reverted/absent visit yields None → nothing to refresh.
    let url_id: Option<i64> = connection
        .query_row(
            "SELECT visits.url_id
             FROM archive.visits AS visits
             WHERE visits.id = ?1
               AND visits.reverted_at IS NULL",
            params![history_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(url_id) = url_id else {
        return Ok(());
    };

    // (2) Recompute the capped enrichment_text from the best stored row for THIS url (newest success).
    let enrichment_text: String = connection
        .query_row(
            "SELECT vce.enrichment_summary, vce.extraction_json
             FROM visit_content_enrichments AS vce
             JOIN archive.visits AS visits ON visits.id = vce.history_id
             WHERE visits.url_id = ?1
               AND vce.content_source IN ('github-repo', 'generic-readable')
               AND vce.fetch_status = 'success'
             ORDER BY vce.fetched_at DESC
             LIMIT 1",
            params![url_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?
        .map(|(summary, extraction)| {
            enrichment_text_for_index(summary.as_deref(), extraction.as_deref())
        })
        .unwrap_or_default();

    // (3) UPDATE the existing search_documents row + re-mirror the term FTS (delete-then-insert).
    update_enrichment_text_in_projection(paths, url_id, &enrichment_text)
}

/// Applies a recomputed `enrichment_text` to one `search_documents` row + its term FTS mirror.
///
/// Reads the stored doc, and if present re-mirrors the term FTS via the contentless delete-then-insert
/// pair (so the FTS row's `enrichment_text` matches the doc) and UPDATEs the doc column. A missing doc
/// is a no-op (the projection has not been seeded for this URL yet). The trigram FTS is untouched (it
/// mirrors `compact_text`, which carries no enrichment text). Runs in one transaction.
fn update_enrichment_text_in_projection(
    paths: &ProjectPaths,
    url_id: i64,
    enrichment_text: &str,
) -> Result<()> {
    let mut search = open_search_connection(paths)?;
    ensure_search_schema(&search)?;
    let transaction = search.transaction()?;
    let stored = transaction
        .query_row(
            "SELECT
               url,
               title,
               search_terms,
               normalized_url,
               normalized_title,
               normalized_search_terms,
               compact_text,
               cjk_grams,
               enrichment_text
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
                    enrichment_text: row.get(8)?,
                })
            },
        )
        .optional()?;
    let Some(stored) = stored else {
        // No projection row for this URL yet → nothing to refresh (a later rebuild/seed mirrors it).
        return Ok(());
    };
    if stored.enrichment_text == enrichment_text {
        return Ok(()); // Already current — avoid a redundant FTS rewrite.
    }

    // Delete the old term-FTS row (contentless FTS5 needs the OLD column values to delete cleanly).
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
           cjk_grams,
           enrichment_text
         )
         VALUES('delete', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            url_id,
            &stored.url,
            &stored.title,
            &stored.search_terms,
            &stored.lexical.normalized_url,
            &stored.lexical.normalized_title,
            &stored.lexical.normalized_search_terms,
            &stored.lexical.cjk_grams,
            &stored.enrichment_text,
        ],
    )?;
    transaction.execute(
        "UPDATE search_documents
         SET enrichment_text = ?1, updated_at = datetime('now')
         WHERE url_id = ?2",
        params![enrichment_text, url_id],
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
           cjk_grams,
           enrichment_text
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            url_id,
            &stored.url,
            &stored.title,
            &stored.search_terms,
            &stored.lexical.normalized_url,
            &stored.lexical.normalized_title,
            &stored.lexical.normalized_search_terms,
            &stored.lexical.cjk_grams,
            enrichment_text,
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Loads `url_id → enrichment_text` from the intelligence DB (best enrichment per URL, 06 §4).
///
/// The search projection lives in the archive plane, but `visit_content_enrichments` lives in the
/// intelligence plane (which attaches the canonical archive as `archive`), so this opens the
/// intelligence connection and resolves the BEST enrichment per URL there — keyed by `url_id` so the
/// rebuild can join it to `search_documents`. "Enrichment text" is the CAPPED summary plus the GitHub
/// topics/desc metadata from `extraction_json` — NEVER the full body blob (06 §4: that would explode
/// the index + wreck BM25). A best-effort failure (the intelligence DB not yet present) returns an
/// empty map so the rebuild still produces a valid projection without enrichment text.
fn load_enrichment_text_by_url_id(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<HashMap<i64, String>> {
    let connection = crate::archive::open_intelligence_connection(paths, config, key)?;
    let mut statement = connection.prepare(
        "SELECT urls.id,
                vce.enrichment_summary,
                vce.extraction_json,
                vce.fetch_status,
                vce.content_source
         FROM visit_content_enrichments AS vce
         JOIN archive.visits AS visits ON visits.id = vce.history_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE vce.content_source IN ('github-repo', 'generic-readable')
           AND vce.fetch_status = 'success'
         ORDER BY urls.id ASC, vce.fetched_at DESC",
    )?;
    let mut rows = statement.query([])?;
    let mut by_url: HashMap<i64, String> = HashMap::new();
    while let Some(row) = rows.next()? {
        let url_id: i64 = row.get(0)?;
        // First-seen (newest) enrichment per URL wins; later (older) rows are skipped.
        if by_url.contains_key(&url_id) {
            continue;
        }
        let summary: Option<String> = row.get(1)?;
        let extraction_json: Option<String> = row.get(2)?;
        let text = enrichment_text_for_index(summary.as_deref(), extraction_json.as_deref());
        if !text.trim().is_empty() {
            by_url.insert(url_id, text);
        }
    }
    Ok(by_url)
}

/// Builds the FTS `enrichment_text` value from a row's capped summary + key metadata (06 §4).
///
/// PURE → unit-tested. Concatenates the capped summary with the searchable structured metadata fields
/// (GitHub `topics` + `description` + `fullName`/`language`) — NOT the full body. Keeping it to the
/// summary + a handful of keyword-bearing metadata fields is what keeps the FTS index bounded at the
/// 14.4M tail while still making "find that repo about wasm" hit on the topics.
fn enrichment_text_for_index(summary: Option<&str>, extraction_json: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(summary) = summary.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(summary.to_string());
    }
    if let Some(extraction) =
        extraction_json.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
    {
        for key in ["fullName", "description", "language"] {
            if let Some(value) = extraction.get(key).and_then(|value| value.as_str()) {
                let value = value.trim();
                if !value.is_empty() {
                    parts.push(value.to_string());
                }
            }
        }
        if let Some(topics) = extraction.get("topics").and_then(|value| value.as_array()) {
            let joined =
                topics.iter().filter_map(|value| value.as_str()).collect::<Vec<_>>().join(" ");
            if !joined.trim().is_empty() {
                parts.push(joined);
            }
        }
    }
    // De-duplicate identical parts (the summary often equals the description) so the index isn't fed
    // the same token twice; preserve order.
    let mut seen = std::collections::HashSet::new();
    parts.retain(|part| seen.insert(part.clone()));
    parts.join(" \u{2022} ")
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
    rebuild_search_projection_from_archive_with_enrichment(archive, paths, &HashMap::new())
}

fn rebuild_search_projection_from_archive_with_enrichment(
    archive: &Connection,
    paths: &ProjectPaths,
    enrichment_text: &HashMap<i64, String>,
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
           enrichment_text,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))",
    )?;
    while let Some(row) = rows.next()? {
        let url_id = row.get::<_, i64>(0)?;
        let url = row.get::<_, String>(1)?;
        let title = row.get::<_, String>(2)?;
        let search_terms = row.get::<_, String>(3)?;
        let lexical = analyze_document(&url, &title, &search_terms);
        let enrichment = enrichment_text.get(&url_id).map(String::as_str).unwrap_or("");
        insert.execute(params![
            url_id,
            url,
            title,
            search_terms,
            lexical.normalized_url,
            lexical.normalized_title,
            lexical.normalized_search_terms,
            lexical.compact_text,
            lexical.cjk_grams,
            enrichment,
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
    let lexical = analyze_document(url, title, search_terms);

    // Preserve any existing enrichment_text across an import-batch refresh: import refresh re-derives
    // the lexical fields from the canonical archive but carries NO new enrichment (content fetch is a
    // separate, later pass), so a previously-mirrored enrichment summary must survive the refresh
    // rather than being wiped to ''.
    let mut enrichment_text = String::new();
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
               cjk_grams,
               enrichment_text
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
                    enrichment_text: row.get(8)?,
                })
            },
        )
        .optional()?
    {
        enrichment_text = old_document.enrichment_text.clone();
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
               cjk_grams,
               enrichment_text
             )
             VALUES('delete', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                url_id,
                &old_document.url,
                &old_document.title,
                &old_document.search_terms,
                &old_document.lexical.normalized_url,
                &old_document.lexical.normalized_title,
                &old_document.lexical.normalized_search_terms,
                &old_document.lexical.cjk_grams,
                &old_document.enrichment_text,
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
           enrichment_text,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
         ON CONFLICT(url_id) DO UPDATE SET
           url = excluded.url,
           title = excluded.title,
           search_terms = excluded.search_terms,
           normalized_url = excluded.normalized_url,
           normalized_title = excluded.normalized_title,
           normalized_search_terms = excluded.normalized_search_terms,
           compact_text = excluded.compact_text,
           cjk_grams = excluded.cjk_grams,
           enrichment_text = excluded.enrichment_text,
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
            &enrichment_text,
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
           cjk_grams,
           enrichment_text
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            url_id,
            url,
            title,
            search_terms,
            &lexical.normalized_url,
            &lexical.normalized_title,
            &lexical.normalized_search_terms,
            &lexical.cjk_grams,
            &enrichment_text,
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
    enrichment_text: String,
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
        archive::open_intelligence_connection,
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
    fn fuzzy_trigram_candidate_probe_is_limited_before_rust_rerank() {
        let connection = Connection::open_in_memory().expect("open memory db");
        connection
            .execute(
                "CREATE VIRTUAL TABLE fuzzy_probe
                 USING fts5(compact_text, tokenize='trigram')",
                [],
            )
            .expect("fuzzy fts table");
        for index in 0..500 {
            connection
                .execute(
                    "INSERT INTO fuzzy_probe(compact_text)
                     VALUES (?1)",
                    params![format!("githubcandidate{index}")],
                )
                .expect("insert fuzzy probe row");
        }

        let query = "(\"git\" OR \"ith\" OR \"thu\" OR \"hub\")";
        let unbounded_hits: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM fuzzy_probe
                 WHERE fuzzy_probe MATCH ?1",
                [query],
                |row| row.get(0),
            )
            .expect("unbounded fuzzy hits");
        let bounded_hits: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM (
                   SELECT rowid
                   FROM fuzzy_probe
                   WHERE fuzzy_probe MATCH ?1
                   ORDER BY bm25(fuzzy_probe, 1.0) ASC
                   LIMIT 200
                 )",
                [query],
                |row| row.get(0),
            )
            .expect("bounded fuzzy hits");

        assert_eq!(unbounded_hits, 500);
        assert_eq!(bounded_hits, 200);
    }

    #[test]
    fn enrichment_text_for_index_concats_summary_and_key_metadata_only() {
        // GitHub: summary (description) + topics + fullName + language, de-duplicated, body excluded.
        let extraction = r#"{
            "fullName": "rust-lang/rust",
            "description": "A safe systems language",
            "language": "Rust",
            "topics": ["compiler", "wasm"],
            "hasReadme": true
        }"#;
        let text = enrichment_text_for_index(Some("A safe systems language"), Some(extraction));
        assert!(text.contains("A safe systems language"));
        assert!(text.contains("rust-lang/rust"));
        assert!(text.contains("compiler wasm"));
        assert!(text.contains("Rust"));
        // The summary == description, so it appears ONCE (de-duplicated), not twice.
        assert_eq!(text.matches("A safe systems language").count(), 1);
        // Empty inputs yield an empty index value.
        assert_eq!(enrichment_text_for_index(None, None), "");
        assert_eq!(enrichment_text_for_index(Some("   "), Some("not json")), "");
        // A summary with no metadata is still indexed.
        assert_eq!(enrichment_text_for_index(Some("just a summary"), None), "just a summary");
    }

    #[test]
    fn search_schema_version_is_v3_with_enrichment_text() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_search_schema(&connection).expect("schema");
        // The version is persisted as v3.
        let version: String = connection
            .query_row(
                "SELECT value FROM search_projection_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, "3");
        // The `enrichment_text` column exists on both the doc table and the term FTS mirror.
        let doc_has_enrichment: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('search_documents') WHERE name = 'enrichment_text'",
                [],
                |row| row.get(0),
            )
            .expect("doc column");
        assert_eq!(doc_has_enrichment, 1);
        // Insert a doc with enrichment text and confirm it is FTS-searchable.
        connection
            .execute(
                "INSERT INTO search_documents (
                   url_id, url, title, search_terms, normalized_url, normalized_title,
                   normalized_search_terms, compact_text, cjk_grams, enrichment_text, updated_at
                 )
                 VALUES (1, 'https://github.com/o/r', 'o/r', '', 'https://github.com/o/r', 'o/r',
                         '', 'github', '', 'wasmtime sandbox runtime', datetime('now'))",
                [],
            )
            .expect("insert doc");
        connection
            .execute("INSERT INTO history_search_terms(history_search_terms) VALUES('rebuild')", [])
            .expect("rebuild fts");
        let hits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM history_search_terms WHERE history_search_terms MATCH 'wasmtime'",
                [],
                |row| row.get(0),
            )
            .expect("fts match");
        assert_eq!(hits, 1, "enrichment_text must be keyword-searchable via the term FTS mirror");
    }

    #[test]
    fn search_schema_v2_to_v3_auto_rebuilds_and_drops_legacy_table() {
        // Stand up a v2-shaped projection (no enrichment_text), then ensure_search_schema must drop +
        // rebuild it to v3 (the projection is rebuildable derived state, so a version bump is safe).
        let connection = Connection::open_in_memory().expect("memory db");
        connection
            .execute_batch(
                "CREATE TABLE search_projection_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE search_documents (
                   url_id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL,
                   search_terms TEXT NOT NULL, normalized_url TEXT NOT NULL,
                   normalized_title TEXT NOT NULL, normalized_search_terms TEXT NOT NULL,
                   compact_text TEXT NOT NULL, cjk_grams TEXT NOT NULL, updated_at TEXT NOT NULL
                 );
                 INSERT INTO search_projection_meta (key, value) VALUES ('schema_version', '2');
                 INSERT INTO search_documents VALUES (1, 'u', 't', '', 'u', 't', '', 'c', '', 'now');",
            )
            .expect("seed v2 projection");

        ensure_search_schema(&connection).expect("upgrade to v3");

        // The version is now v3 and the legacy v2 rows were dropped (a rebuild reseeds from archive).
        let version: String = connection
            .query_row(
                "SELECT value FROM search_projection_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, "3");
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM search_documents", [], |row| row.get(0))
            .expect("count");
        assert_eq!(remaining, 0, "v2 rows must be dropped by the v2→v3 reset");
        // The v3 enrichment_text column is present after the upgrade.
        let has_enrichment: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('search_documents') WHERE name = 'enrichment_text'",
                [],
                |row| row.get(0),
            )
            .expect("column");
        assert_eq!(has_enrichment, 1);
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

    /// Seeds a plaintext archive with one url + visit, returns (paths, config, intelligence conn).
    fn seeded_plaintext_archive() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        archive
            .execute(
                "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'test', '2026-01-01', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("run");
        archive
            .execute(
                "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                 VALUES (1, 'chrome', 'x', 'p', '/p', '2026-01-01', 1, 'chrome:Default', '2026-01-01')",
                [],
            )
            .expect("profile");
        archive
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                 VALUES (1, 'https://github.com/o/r', 'o/r', 1, 0, 1, '', 1, '', 1, 1)",
                [],
            )
            .expect("url");
        archive
            .execute(
                "INSERT INTO visits (id, url_id, source_profile_id, visit_time_ms, visit_time_iso, created_by_run_id)
                 VALUES (10, 1, 1, 1, '2026-01-01T00:00:00Z', 1)",
                [],
            )
            .expect("visit");
        drop(archive);
        (dir, paths, config)
    }

    /// Stores one successful github enrichment for `history_id` on the intelligence connection.
    fn store_github_enrichment(connection: &Connection, history_id: i64, summary: &str) {
        crate::enrichment::ensure_visit_content_enrichment_schema(connection).expect("schema");
        connection
            .execute(
                "INSERT OR REPLACE INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
                  pipeline_version, extractor_version, enrichment_summary)
                 VALUES (?1, 'github-repo', 'success', '2026-06-21T00:00:00Z', '[]', ?2, 'v1', 1, ?3)",
                params![
                    history_id,
                    format!(
                        r#"{{"fullName":"o/r","description":"{summary}","topics":["wasm"],"language":"Rust"}}"#
                    ),
                    summary,
                ],
            )
            .expect("insert enrichment");
    }

    #[test]
    fn rebuild_search_projection_mirrors_stored_enrichment_text() {
        // Covers `load_enrichment_text_by_url_id` (the LEFT-JOIN-per-URL loop + the newest-wins skip)
        // and the rebuild path that mirrors enrichment_text into the FTS term index.
        let (_dir, paths, config) = seeded_plaintext_archive();
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        // Two rows for the SAME url_id (newest wins; the older is skipped → covers the skip branch).
        store_github_enrichment(&connection, 10, "Wasmtime sandbox runtime");
        connection
            .execute(
                "INSERT OR REPLACE INTO visit_content_enrichments
                 (history_id, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
                  pipeline_version, extractor_version, enrichment_summary)
                 VALUES (11, 'github-repo', 'success', '2026-06-20T00:00:00Z', '[]', '{}', 'v1', 1, 'older')",
                [],
            )
            .expect("older row");
        // Add a second visit (11) of the same url so both map to url_id 1.
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        archive
            .execute(
                "INSERT INTO visits (id, url_id, source_profile_id, visit_time_ms, visit_time_iso, created_by_run_id)
                 VALUES (11, 1, 1, 1, '2026-01-01T00:00:00Z', 1)",
                [],
            )
            .expect("visit 11");
        drop(archive);

        rebuild_search_projection(&paths, &config, None).expect("rebuild");

        let search = open_search_connection(&paths).expect("search");
        let hits: i64 = search
            .query_row(
                "SELECT COUNT(*) FROM history_search_terms WHERE history_search_terms MATCH 'wasmtime'",
                [],
                |row| row.get(0),
            )
            .expect("match");
        assert_eq!(hits, 1, "the stored enrichment summary must be mirrored into the FTS index");
    }

    #[test]
    fn refresh_enrichment_text_is_a_noop_without_a_projection_row() {
        // Covers `refresh_enrichment_text_for_history` resolving a url_id but finding NO search_documents
        // row (the projection has not been seeded) → a clean no-op.
        let (_dir, paths, config) = seeded_plaintext_archive();
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        store_github_enrichment(&connection, 10, "Wasmtime");
        // No search_documents row exists for url_id 1 yet → no-op (must not error).
        refresh_enrichment_text_for_history(&paths, &connection, 10).expect("noop refresh");

        // An unknown/absent history_id resolves no url_id → also a no-op.
        refresh_enrichment_text_for_history(&paths, &connection, 999).expect("noop unknown");
    }

    #[test]
    fn refresh_enrichment_text_updates_then_skips_when_already_current() {
        // Covers the UPDATE path AND the "already current → skip" early return.
        let (_dir, paths, config) = seeded_plaintext_archive();
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        store_github_enrichment(&connection, 10, "Wasmtime sandbox runtime");
        // Seed a projection row for url_id 1 (empty enrichment_text).
        {
            let mut search = open_search_connection(&paths).expect("search");
            ensure_search_schema(&search).expect("schema");
            let txn = search.transaction().expect("txn");
            txn.execute(
                "INSERT INTO search_documents (
                   url_id, url, title, search_terms, normalized_url, normalized_title,
                   normalized_search_terms, compact_text, cjk_grams, enrichment_text, updated_at
                 )
                 VALUES (1, 'https://github.com/o/r', 'o/r', '', 'https://github.com/o/r', 'o/r',
                         '', 'githubor', '', '', datetime('now'))",
                [],
            )
            .expect("seed doc");
            txn.execute(
                "INSERT INTO history_search_terms(rowid, url, title, search_terms, normalized_url, normalized_title, normalized_search_terms, cjk_grams, enrichment_text)
                 VALUES (1, 'https://github.com/o/r', 'o/r', '', 'https://github.com/o/r', 'o/r', '', '', '')",
                [],
            )
            .expect("seed fts");
            txn.commit().expect("commit");
        }
        // First refresh UPDATEs the row.
        refresh_enrichment_text_for_history(&paths, &connection, 10).expect("refresh");
        let search = open_search_connection(&paths).expect("search");
        let stored: String = search
            .query_row("SELECT enrichment_text FROM search_documents WHERE url_id = 1", [], |row| {
                row.get(0)
            })
            .expect("text");
        assert!(stored.contains("Wasmtime sandbox runtime"));
        let hits: i64 = search
            .query_row(
                "SELECT COUNT(*) FROM history_search_terms WHERE history_search_terms MATCH 'wasmtime'",
                [],
                |row| row.get(0),
            )
            .expect("match");
        assert_eq!(hits, 1);
        drop(search);

        // A second refresh with the SAME enrichment is a no-op (covers the already-current early return).
        refresh_enrichment_text_for_history(&paths, &connection, 10).expect("idempotent refresh");
    }
}
