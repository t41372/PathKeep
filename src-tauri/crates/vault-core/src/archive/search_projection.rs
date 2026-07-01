//! Search projection storage for lexical recall.
//!
//! The canonical archive keeps source-of-truth visits and URLs. Keyword recall
//! lives in a separate SQLite plane so FTS and projection rebuilds do not
//! bloat the hot archive file.

use super::search_lexical::{LexicalDocument, analyze_document};
use crate::{
    archive::open_archive_connection,
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveUpgradePhase, ArchiveUpgradeProgress},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::time::Duration as StdDuration;

/// Rows reprojected between search-reprojection progress ticks. Coalesces the
/// per-document loop into a handful of events across the 14.4M tail so the
/// upgrade screen updates smoothly without flooding the event channel.
const SEARCH_REPROJECTION_PROGRESS_STRIDE: u64 = 2_048;

// v4 (notes/tags recall): adds `notes_text` + `tags_text` columns to `search_documents` + the term FTS
// mirror so per-URL NOTES (`url_annotations.notes`) and TAGS (`url_tags.tag`, space-joined) are
// keyword-searchable through the SAME lexical path the UI uses — a note like "meta 的设计系统" now
// surfaces on a plain "设计系统" search, not only via the `note:`/`tag:` post-filters. CRUCIALLY, notes/
// tags also fold into the field-agnostic `compact_text` (trigram plane) + `cjk_grams` (term-FTS gram
// plane) via `analyze_document`, because PathKeep's CJK recall is entirely gram/trigram-based — the
// unicode61 `notes_text`/`tags_text` columns alone would only serve Latin. Notes/tags are canonical-
// archive content (NOT intelligence-DB derived), so BOTH rebuild variants populate them and
// `set_notes`/`replace_tags` re-derive ONE url's full document immediately (no full rebuild needed).
//
// v3 (W-ENRICH-1): adds the `enrichment_text` column to `search_documents` + the term FTS mirror so
// a content-fetch enrichment's CAPPED summary + key metadata (GitHub topics/desc, video channel) are
// keyword-searchable. A version mismatch drops + rebuilds the projection (the projection is fully
// rebuildable derived state). NOTE: only the capped summary + metadata ride here — NOT the full
// body/caption (06 §4: multi-KB blobs would explode the index size + wreck BM25 at the 14.4M tail).
const SEARCH_PROJECTION_SCHEMA_VERSION: i64 = 4;

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
  notes_text TEXT NOT NULL DEFAULT '',
  tags_text TEXT NOT NULL DEFAULT '',
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
  notes_text,
  tags_text,
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

/// Seeds the search projection from the canonical archive when it is empty,
/// reprojecting with per-batch progress so the first-run upgrade's reprojection
/// is observable instead of an opaque multi-minute stall.
///
/// Guards: skip when the projection already holds documents, or when there are
/// no canonical URLs. A plain [`open_archive_connection`](crate::archive::open_archive_connection)
/// passes a no-op reporter, so the no-callback path is byte-for-byte the
/// original seed (the seed runs during archive open with no enrichment yet;
/// enrichment text is mirrored in by a later `rebuild_search_projection` once
/// content has been fetched, so it seeds with an empty map). Reuses the
/// already-computed `urls` count as the progress total, so no extra COUNT is
/// paid beyond the guard. Progress is OBSERVATION ONLY — the reprojection SQL
/// and per-document derivation are unchanged.
pub(crate) fn seed_search_projection_with_progress<F>(
    archive: &Connection,
    paths: &ProjectPaths,
    report: &mut F,
) -> Result<()>
where
    F: FnMut(ArchiveUpgradeProgress),
{
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

    let total = canonical_urls.max(0) as u64;
    rebuild_search_projection_core(
        archive,
        paths,
        &HashMap::new(),
        total,
        SEARCH_REPROJECTION_PROGRESS_STRIDE,
        &mut |processed, total| {
            report(ArchiveUpgradeProgress::phase(
                ArchiveUpgradePhase::SearchReprojection,
                processed.min(total),
                total,
            ));
        },
    )
}

/// Whether opening the archive will trigger a search-projection reprojection
/// (a schema drift that drops + rebuilds, or a not-yet-seeded projection).
///
/// Used by the cheap upgrade pre-check. Reads only the projection meta version
/// and an existence probe — never a full COUNT of the document tail. A missing
/// search DB reports `true` WITHOUT opening (which would create the file).
pub(crate) fn search_reprojection_pending(paths: &ProjectPaths) -> Result<bool> {
    if !paths.search_database_path.exists() {
        return Ok(true);
    }
    let connection = open_search_connection(paths)?;
    if search_schema_version(&connection)? != Some(SEARCH_PROJECTION_SCHEMA_VERSION) {
        // A version drift resets + rebuilds the whole projection on next open.
        return Ok(true);
    }
    // Version matches: a rebuild still runs only if the projection holds no
    // documents yet (a fresh seed). Existence probe, not a COUNT of the tail.
    let has_document = connection
        .query_row("SELECT 1 FROM search_documents LIMIT 1", [], |_| Ok(()))
        .optional()?
        .is_some();
    Ok(!has_document)
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
    let Some(stored) = read_stored_search_document(&transaction, url_id)? else {
        // No projection row for this URL yet → nothing to refresh (a later rebuild/seed mirrors it).
        return Ok(());
    };
    if stored.enrichment_text == enrichment_text {
        return Ok(()); // Already current — avoid a redundant FTS rewrite.
    }

    // Delete the old term-FTS row (contentless FTS5 needs the OLD column values to delete cleanly).
    delete_term_fts_row(&transaction, url_id, &stored)?;
    transaction.execute(
        "UPDATE search_documents
         SET enrichment_text = ?1, updated_at = datetime('now')
         WHERE url_id = ?2",
        params![enrichment_text, url_id],
    )?;
    // Re-mirror with the new enrichment_text; notes/tags are unchanged here so they ride through.
    insert_term_fts_row(
        &transaction,
        url_id,
        &stored.url,
        &stored.title,
        &stored.search_terms,
        &stored.lexical,
        enrichment_text,
        &stored.notes_text,
        &stored.tags_text,
    )?;
    transaction.commit()?;
    Ok(())
}

/// Refreshes the term FTS `notes_text`+`tags_text` for ONE canonical URL right after a note/tag edit.
///
/// Mirror of `refresh_enrichment_text_for_history`, but for user-authored annotations: a `set_notes` /
/// `replace_tags` write lands in the canonical archive (`url_annotations` / `url_tags`), yet the lexical
/// recall plane only re-derived notes/tags during a FULL rebuild — so an edited note was un-findable by
/// plain keyword until the next rebuild. This closes that gap without a rebuild: it recomputes the
/// CURRENT notes/tags for `url` from the archive `connection` (PK / `(url,tag)`-indexed lookups, never a
/// scan), then UPDATEs every `search_documents` row sharing that canonical URL + re-mirrors the term
/// FTS (delete-then-insert). `url` is NOT unique in `urls` (the same address can ride under several
/// profiles), so it fans out to every matching `url_id`. Best-effort + idempotent: a URL with no
/// `search_documents` row yet is a no-op (the eventual seed/rebuild picks it up); identical text skips
/// the rewrite. `connection` is the ARCHIVE connection (it owns `urls`/`url_annotations`/`url_tags`);
/// `paths` opens the SEARCH database, exactly like the enrichment refresh.
pub(crate) fn refresh_notes_tags_text_for_url(
    paths: &ProjectPaths,
    connection: &Connection,
    url: &str,
) -> Result<()> {
    // Recompute the CURRENT notes + space-joined tags for this canonical URL (default '' when absent).
    let notes_text: String = connection
        .query_row("SELECT notes FROM url_annotations WHERE url = ?1", params![url], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .unwrap_or_default();
    let tags_text: String = connection
        .query_row(
            "SELECT GROUP_CONCAT(tag, ' ') FROM url_tags WHERE url = ?1",
            params![url],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .unwrap_or_default();

    // Fan out to every url_id carrying this address (notes/tags travel with the URL, across profiles).
    let mut statement = connection.prepare("SELECT id FROM urls WHERE url = ?1")?;
    let url_ids: Vec<i64> = statement
        .query_map(params![url], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for url_id in url_ids {
        update_notes_tags_text_in_projection(paths, url_id, &notes_text, &tags_text)?;
    }
    Ok(())
}

/// Applies a recomputed `notes_text`+`tags_text` to one `search_documents` row + its term FTS mirror.
///
/// Sibling of `update_enrichment_text_in_projection`: reads the stored doc, and if present re-mirrors
/// the term FTS via the contentless delete-then-insert pair (so the FTS row's notes/tags match the doc)
/// and UPDATEs the doc columns. A missing doc is a no-op (the projection has not been seeded for this
/// URL yet); identical text is skipped to avoid a redundant FTS rewrite. The trigram FTS is untouched
/// (it mirrors `compact_text`, which carries no notes/tags). Runs in one transaction.
fn update_notes_tags_text_in_projection(
    paths: &ProjectPaths,
    url_id: i64,
    notes_text: &str,
    tags_text: &str,
) -> Result<()> {
    let mut search = open_search_connection(paths)?;
    ensure_search_schema(&search)?;
    let transaction = search.transaction()?;
    let Some(stored) = read_stored_search_document(&transaction, url_id)? else {
        // No projection row for this URL yet → nothing to refresh (a later rebuild/seed mirrors it).
        return Ok(());
    };
    if stored.notes_text == notes_text && stored.tags_text == tags_text {
        return Ok(()); // Already current — avoid a redundant FTS rewrite.
    }

    // Re-derive the FULL lexical document with the fresh notes/tags so compact_text + cjk_grams (the
    // gram/trigram CJK recall plane) carry the note/tag text immediately — not only the unicode61
    // notes_text/tags_text columns. Without this, an edited CJK note would stay un-findable until the
    // next full rebuild. url/title/search_terms are unchanged, so their normalized fields are identical.
    let lexical =
        analyze_document(&stored.url, &stored.title, &stored.search_terms, notes_text, tags_text);

    // Re-mirror BOTH FTS planes: delete by the OLD stored values, insert the freshly derived document.
    delete_term_fts_row(&transaction, url_id, &stored)?;
    delete_trigram_fts_row(&transaction, url_id, &stored.lexical.compact_text)?;
    transaction.execute(
        "UPDATE search_documents
         SET compact_text = ?1,
             cjk_grams = ?2,
             notes_text = ?3,
             tags_text = ?4,
             updated_at = datetime('now')
         WHERE url_id = ?5",
        params![&lexical.compact_text, &lexical.cjk_grams, notes_text, tags_text, url_id],
    )?;
    insert_term_fts_row(
        &transaction,
        url_id,
        &stored.url,
        &stored.title,
        &stored.search_terms,
        &lexical,
        &stored.enrichment_text,
        notes_text,
        tags_text,
    )?;
    insert_trigram_fts_row(&transaction, url_id, &lexical.compact_text)?;
    transaction.commit()?;
    Ok(())
}

/// Reads the stored projection doc for one URL (every FTS-mirrored column), or None if absent.
///
/// Shared by all delete-then-insert paths: a contentless FTS5 'delete' needs the OLD column values to
/// remove the stored tokens cleanly, so each refresh first reads the doc it is about to rewrite.
fn read_stored_search_document(
    connection: &Connection,
    url_id: i64,
) -> Result<Option<StoredSearchDocument>> {
    Ok(connection
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
               enrichment_text,
               notes_text,
               tags_text
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
                    notes_text: row.get(9)?,
                    tags_text: row.get(10)?,
                })
            },
        )
        .optional()?)
}

/// Removes the OLD term-FTS row for `url_id` using the stored column values (contentless FTS5 delete).
fn delete_term_fts_row(
    transaction: &rusqlite::Transaction<'_>,
    url_id: i64,
    stored: &StoredSearchDocument,
) -> Result<()> {
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
           enrichment_text,
           notes_text,
           tags_text
         )
         VALUES('delete', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            &stored.notes_text,
            &stored.tags_text,
        ],
    )?;
    Ok(())
}

/// Inserts a term-FTS row mirroring the given column values (the new state after an UPDATE).
#[allow(clippy::too_many_arguments)]
fn insert_term_fts_row(
    transaction: &rusqlite::Transaction<'_>,
    url_id: i64,
    url: &str,
    title: &str,
    search_terms: &str,
    lexical: &LexicalDocument,
    enrichment_text: &str,
    notes_text: &str,
    tags_text: &str,
) -> Result<()> {
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
           enrichment_text,
           notes_text,
           tags_text
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            url_id,
            url,
            title,
            search_terms,
            &lexical.normalized_url,
            &lexical.normalized_title,
            &lexical.normalized_search_terms,
            &lexical.cjk_grams,
            enrichment_text,
            notes_text,
            tags_text,
        ],
    )?;
    Ok(())
}

/// Removes the OLD trigram-FTS row for `url_id` using the stored `compact_text` (contentless delete).
fn delete_trigram_fts_row(
    transaction: &rusqlite::Transaction<'_>,
    url_id: i64,
    compact_text: &str,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO history_search_trigram(history_search_trigram, rowid, compact_text)
         VALUES('delete', ?1, ?2)",
        params![url_id, compact_text],
    )?;
    Ok(())
}

/// Inserts a trigram-FTS row mirroring the given `compact_text` (the new state after an UPDATE).
fn insert_trigram_fts_row(
    transaction: &rusqlite::Transaction<'_>,
    url_id: i64,
    compact_text: &str,
) -> Result<()> {
    transaction.execute(
        "INSERT INTO history_search_trigram(rowid, compact_text)
         VALUES(?1, ?2)",
        params![url_id, compact_text],
    )?;
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

/// Loads `url → notes` from the canonical archive for the rebuild's `notes_text` mirror.
///
/// `url_annotations` is keyed by the URL string (notes travel with the address, across profiles), so
/// the map is keyed the same way and the rebuild looks each `urls.url` up directly. The table is tiny
/// versus the URL count (only annotated pages have a row), so this is a bounded read, not a scan of the
/// 14.4M tail. Empty notes are skipped so the map only carries searchable text.
fn load_notes_text_by_url(archive: &Connection) -> Result<HashMap<String, String>> {
    let mut statement = archive.prepare("SELECT url, notes FROM url_annotations")?;
    let mut rows = statement.query([])?;
    let mut by_url: HashMap<String, String> = HashMap::new();
    while let Some(row) = rows.next()? {
        let url: String = row.get(0)?;
        let notes: String = row.get(1)?;
        if !notes.trim().is_empty() {
            by_url.insert(url, notes);
        }
    }
    Ok(by_url)
}

/// Loads `url → space-joined tags` from the canonical archive for the rebuild's `tags_text` mirror.
///
/// `url_tags` stores one row per (url, tag); `GROUP_CONCAT(tag, ' ')` joins a URL's tags into one
/// searchable string. Keyed by the URL string for the same reason as notes. `(url, tag)` is the table's
/// primary key, so the GROUP BY rides an index — a bounded read over only the tagged pages.
fn load_tags_text_by_url(archive: &Connection) -> Result<HashMap<String, String>> {
    let mut statement =
        archive.prepare("SELECT url, GROUP_CONCAT(tag, ' ') FROM url_tags GROUP BY url")?;
    let mut rows = statement.query([])?;
    let mut by_url: HashMap<String, String> = HashMap::new();
    while let Some(row) = rows.next()? {
        let url: String = row.get(0)?;
        let tags: Option<String> = row.get(1)?;
        if let Some(tags) = tags.filter(|value| !value.trim().is_empty()) {
            by_url.insert(url, tags);
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

fn rebuild_search_projection_from_archive_with_enrichment(
    archive: &Connection,
    paths: &ProjectPaths,
    enrichment_text: &HashMap<i64, String>,
) -> Result<()> {
    // The plain rebuild carries no progress: `total = 0` and a no-op reporter mean
    // no extra COUNT and no ticks, so this stays byte-for-byte the original path.
    rebuild_search_projection_core(
        archive,
        paths,
        enrichment_text,
        0,
        SEARCH_REPROJECTION_PROGRESS_STRIDE,
        &mut |_, _| {},
    )
}

/// Progress-aware core of the full search-projection rebuild.
///
/// Identical DELETE-then-reinsert reprojection as before; `report(processed,
/// total)` fires an initial `0/total`, one tick every `stride` documents, and a
/// final `processed/total`. The no-progress callers pass `total = 0` and a no-op
/// reporter, so they incur no extra work; only
/// [`seed_search_projection_with_progress`] passes a real total (the
/// already-counted `urls` total) and reporter. `stride` is a parameter (rather
/// than the constant directly) so a unit test can drive the mid-loop tick with a
/// small corpus — the production callers always pass
/// [`SEARCH_REPROJECTION_PROGRESS_STRIDE`].
fn rebuild_search_projection_core<R>(
    archive: &Connection,
    paths: &ProjectPaths,
    enrichment_text: &HashMap<i64, String>,
    total: u64,
    stride: u64,
    report: &mut R,
) -> Result<()>
where
    R: FnMut(u64, u64),
{
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
    report(0, total);

    // Notes/tags are canonical-archive content (NOT intelligence-derived), so load them straight from
    // the SAME archive connection the rebuild reads from — keyed by url STRING (their natural key; the
    // same address can ride under several `url_id`s, and every one gets the note). Both rebuild variants
    // run through here, so the seed/no-enrichment path mirrors notes/tags too.
    let notes_by_url = load_notes_text_by_url(archive)?;
    let tags_by_url = load_tags_text_by_url(archive)?;

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
           notes_text,
           tags_text,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))",
    )?;
    let mut processed: u64 = 0;
    while let Some(row) = rows.next()? {
        let url_id = row.get::<_, i64>(0)?;
        let url = row.get::<_, String>(1)?;
        let title = row.get::<_, String>(2)?;
        let search_terms = row.get::<_, String>(3)?;
        let enrichment = enrichment_text.get(&url_id).map(String::as_str).unwrap_or("");
        let notes = notes_by_url.get(&url).map(String::as_str).unwrap_or("");
        let tags = tags_by_url.get(&url).map(String::as_str).unwrap_or("");
        // Fold notes/tags into the lexical derivation so compact_text + cjk_grams carry their grams
        // (CJK recall plane), not only the unicode61 notes_text/tags_text columns.
        let lexical = analyze_document(&url, &title, &search_terms, notes, tags);
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
            notes,
            tags,
        ])?;
        processed += 1;
        if processed % stride == 0 {
            report(processed, total);
        }
    }
    drop(insert);
    transaction
        .execute("INSERT INTO history_search_terms(history_search_terms) VALUES('rebuild')", [])?;
    transaction.execute(
        "INSERT INTO history_search_trigram(history_search_trigram) VALUES('rebuild')",
        [],
    )?;
    transaction.commit()?;
    report(processed, total);
    Ok(())
}

fn refresh_search_document(
    transaction: &rusqlite::Transaction<'_>,
    url_id: i64,
    url: &str,
    title: &str,
    search_terms: &str,
) -> Result<()> {
    // Preserve any existing enrichment_text / notes_text / tags_text across an import-batch refresh:
    // import refresh re-derives the lexical fields from the canonical archive but carries NO new
    // enrichment (a separate content-fetch pass) and does NOT touch annotations, so a previously
    // mirrored summary / note / tag set must survive the refresh rather than being wiped to ''. The
    // old doc is read FIRST so the preserved notes/tags can fold back into the re-derived lexical
    // document (compact_text + cjk_grams), keeping CJK note/tag grams intact across the refresh.
    let mut enrichment_text = String::new();
    let mut notes_text = String::new();
    let mut tags_text = String::new();
    if let Some(old_document) = read_stored_search_document(transaction, url_id)? {
        enrichment_text = old_document.enrichment_text.clone();
        notes_text = old_document.notes_text.clone();
        tags_text = old_document.tags_text.clone();
        delete_term_fts_row(transaction, url_id, &old_document)?;
        delete_trigram_fts_row(transaction, url_id, &old_document.lexical.compact_text)?;
    }

    let lexical = analyze_document(url, title, search_terms, &notes_text, &tags_text);

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
           notes_text,
           tags_text,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
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
           notes_text = excluded.notes_text,
           tags_text = excluded.tags_text,
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
            &notes_text,
            &tags_text,
        ],
    )?;
    insert_term_fts_row(
        transaction,
        url_id,
        url,
        title,
        search_terms,
        &lexical,
        &enrichment_text,
        &notes_text,
        &tags_text,
    )?;
    insert_trigram_fts_row(transaction, url_id, &lexical.compact_text)?;
    Ok(())
}

struct StoredSearchDocument {
    url: String,
    title: String,
    search_terms: String,
    lexical: LexicalDocument,
    enrichment_text: String,
    notes_text: String,
    tags_text: String,
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
    fn search_schema_version_is_v4_with_notes_and_tags_text() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_search_schema(&connection).expect("schema");
        // The version is persisted as v4.
        let version: String = connection
            .query_row(
                "SELECT value FROM search_projection_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, "4");
        // enrichment_text (v3) plus the new notes_text + tags_text (v4) columns all exist on the doc.
        for column in ["enrichment_text", "notes_text", "tags_text"] {
            let present: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('search_documents') WHERE name = ?1",
                    params![column],
                    |row| row.get(0),
                )
                .expect("doc column");
            assert_eq!(present, 1, "search_documents must carry the {column} column");
        }
        // Insert a doc carrying enrichment / notes / tags text and confirm EACH is FTS-searchable
        // through the term FTS mirror (the same MATCH path the lexical query uses).
        connection
            .execute(
                "INSERT INTO search_documents (
                   url_id, url, title, search_terms, normalized_url, normalized_title,
                   normalized_search_terms, compact_text, cjk_grams, enrichment_text,
                   notes_text, tags_text, updated_at
                 )
                 VALUES (1, 'https://github.com/o/r', 'o/r', '', 'https://github.com/o/r', 'o/r',
                         '', 'github', '', 'wasmtime sandbox runtime',
                         'meta design system notes', 'reference design', datetime('now'))",
                [],
            )
            .expect("insert doc");
        connection
            .execute("INSERT INTO history_search_terms(history_search_terms) VALUES('rebuild')", [])
            .expect("rebuild fts");
        for word in ["wasmtime", "notes", "reference"] {
            let hits: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM history_search_terms WHERE history_search_terms MATCH ?1",
                    params![word],
                    |row| row.get(0),
                )
                .expect("fts match");
            assert_eq!(hits, 1, "`{word}` must be keyword-searchable via the term FTS mirror");
        }
    }

    #[test]
    fn search_schema_v3_to_v4_auto_rebuilds_and_adds_notes_tags() {
        // Stand up a v3-shaped projection (enrichment_text but NO notes_text/tags_text), then
        // ensure_search_schema must drop + rebuild it to v4 (the projection is rebuildable derived
        // state, so a version bump is safe).
        let connection = Connection::open_in_memory().expect("memory db");
        connection
            .execute_batch(
                "CREATE TABLE search_projection_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE search_documents (
                   url_id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL,
                   search_terms TEXT NOT NULL, normalized_url TEXT NOT NULL,
                   normalized_title TEXT NOT NULL, normalized_search_terms TEXT NOT NULL,
                   compact_text TEXT NOT NULL, cjk_grams TEXT NOT NULL,
                   enrichment_text TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL
                 );
                 INSERT INTO search_projection_meta (key, value) VALUES ('schema_version', '3');
                 INSERT INTO search_documents
                   (url_id, url, title, search_terms, normalized_url, normalized_title,
                    normalized_search_terms, compact_text, cjk_grams, enrichment_text, updated_at)
                 VALUES (1, 'u', 't', '', 'u', 't', '', 'c', '', '', 'now');",
            )
            .expect("seed v3 projection");

        ensure_search_schema(&connection).expect("upgrade to v4");

        // The version is now v4 and the legacy v3 rows were dropped (a rebuild reseeds from archive).
        let version: String = connection
            .query_row(
                "SELECT value FROM search_projection_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, "4");
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM search_documents", [], |row| row.get(0))
            .expect("count");
        assert_eq!(remaining, 0, "v3 rows must be dropped by the v3→v4 reset");
        // The v4 notes_text + tags_text columns are present after the upgrade.
        for column in ["notes_text", "tags_text"] {
            let present: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('search_documents') WHERE name = ?1",
                    params![column],
                    |row| row.get(0),
                )
                .expect("column");
            assert_eq!(present, 1, "the {column} column must exist after the v3→v4 upgrade");
        }
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

    use crate::{
        annotations::{replace_tags, set_notes},
        archive::list_history,
        models::{HistoryQuery, ReplaceTagsRequest, SetNotesRequest},
    };

    /// The canonical url the `seeded_plaintext_archive` helper installs (url_id 1).
    const SEEDED_URL: &str = "https://github.com/o/r";

    /// Runs the REAL lexical query path the UI uses and returns whether the seeded url is in the page.
    fn plain_keyword_finds_seeded_url(
        paths: &ProjectPaths,
        config: &AppConfig,
        query: &str,
    ) -> bool {
        let response = list_history(
            paths,
            config,
            None,
            HistoryQuery { q: Some(query.to_string()), limit: Some(50), ..HistoryQuery::default() },
        )
        .expect("list_history");
        response.items.iter().any(|item| item.url == SEEDED_URL)
    }

    #[test]
    fn note_is_findable_by_plain_keyword_search_end_to_end() {
        // The headline fix: a saved NOTE must surface on a PLAIN keyword search (no `note:` operator),
        // through the same `list_history` lexical path the Explorer UI uses. "wasmtime" appears ONLY in
        // the note — not in the url/title/search_terms — so this query can only hit via the new
        // notes_text FTS column. On the old v3 schema (no notes_text) the term FTS never indexed the
        // note, so this assertion would FAIL there; that is exactly what proves the fix is exercised.
        let (_dir, paths, config) = seeded_plaintext_archive();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "wasmtime sandbox".into(),
                source_profile: None,
            },
        )
        .expect("set_notes");
        assert!(
            plain_keyword_finds_seeded_url(&paths, &config, "wasmtime"),
            "a saved note must be findable by a plain keyword search"
        );
    }

    #[test]
    fn tag_is_findable_by_plain_keyword_search_end_to_end() {
        // Same contract for a TAG: a plain keyword search for the tag word must surface the page. The
        // tag word appears ONLY in url_tags, so it can only hit via the new tags_text FTS column.
        let (_dir, paths, config) = seeded_plaintext_archive();
        replace_tags(
            &paths,
            &config,
            None,
            ReplaceTagsRequest {
                url: SEEDED_URL.into(),
                tags: vec!["kubernetes".into()],
                source_profile: None,
            },
        )
        .expect("replace_tags");
        assert!(
            plain_keyword_finds_seeded_url(&paths, &config, "kubernetes"),
            "a saved tag must be findable by a plain keyword search"
        );
    }

    #[test]
    fn edited_note_is_findable_and_old_word_drops_via_refresh_hook() {
        // The immediate-refresh hook (no full rebuild) must keep the FTS in lock-step with edits: the
        // new word becomes findable AND the replaced word stops matching.
        let (_dir, paths, config) = seeded_plaintext_archive();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "wasmtime sandbox".into(),
                source_profile: None,
            },
        )
        .expect("set first note");
        assert!(plain_keyword_finds_seeded_url(&paths, &config, "wasmtime"));

        // Edit the note: the new word lands, the old word is removed from the term FTS mirror.
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "freshword runtime".into(),
                source_profile: None,
            },
        )
        .expect("edit note");
        assert!(
            plain_keyword_finds_seeded_url(&paths, &config, "freshword"),
            "the edited note's new word must be findable"
        );
        assert!(
            !plain_keyword_finds_seeded_url(&paths, &config, "wasmtime"),
            "the replaced note's old word must no longer match"
        );
    }

    #[test]
    fn rebuild_mirrors_notes_and_tags_into_the_term_fts() {
        // Covers the rebuild loaders (`load_notes_text_by_url` + `load_tags_text_by_url`, incl. the
        // GROUP_CONCAT tag join) and the rebuild INSERT populating notes_text/tags_text. Notes/tags are
        // written DIRECTLY to the canonical tables (NOT via set_notes) so this exercises the REBUILD
        // path specifically, then a full rebuild must mirror both into the term FTS.
        let (_dir, paths, config) = seeded_plaintext_archive();
        {
            let archive = open_archive_connection(&paths, &config, None).expect("archive");
            archive
                .execute(
                    "INSERT INTO url_annotations(url, notes, created_at, updated_at)
                     VALUES (?1, 'rustlang compiler internals', '2026-01-01', '2026-01-01')",
                    params![SEEDED_URL],
                )
                .expect("note");
            for tag in ["wasm", "toolchain"] {
                archive
                    .execute(
                        "INSERT INTO url_tags(url, tag, created_at) VALUES (?1, ?2, '2026-01-01')",
                        params![SEEDED_URL, tag],
                    )
                    .expect("tag");
            }
        }

        rebuild_search_projection(&paths, &config, None).expect("rebuild");

        let search = open_search_connection(&paths).expect("search");
        for word in ["rustlang", "wasm", "toolchain"] {
            let hits: i64 = search
                .query_row(
                    "SELECT COUNT(*) FROM history_search_terms WHERE history_search_terms MATCH ?1",
                    params![word],
                    |row| row.get(0),
                )
                .expect("match");
            assert_eq!(
                hits, 1,
                "`{word}` (note/tag) must be mirrored into the term FTS by a rebuild"
            );
        }
    }

    #[test]
    fn refresh_notes_tags_skips_when_current_and_no_ops_without_a_doc_or_url() {
        // Covers the three guard branches of the notes/tags refresh: (1) already-current → skip,
        // (2) a resolvable url_id with NO projection doc → no-op, (3) an unknown url → empty fan-out.
        let (_dir, paths, config) = seeded_plaintext_archive();
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        // The seed rebuilt the projection with empty notes/tags. Refreshing with NO annotation yet
        // recomputes empty notes/tags == the stored empty values → the already-current early return.
        refresh_notes_tags_text_for_url(&paths, &archive, SEEDED_URL)
            .expect("already-current skip");

        // Drop the projection doc, then write a note: the refresh resolves url_id 1 but finds no doc →
        // a clean no-op (a later seed/rebuild will pick the note up).
        {
            let mut search = open_search_connection(&paths).expect("search");
            ensure_search_schema(&search).expect("schema");
            let txn = search.transaction().expect("txn");
            txn.execute("DELETE FROM search_documents", []).expect("clear doc");
            txn.execute(
                "INSERT INTO history_search_terms(history_search_terms) VALUES('delete-all')",
                [],
            )
            .expect("clear fts");
            txn.commit().expect("commit");
        }
        archive
            .execute(
                "INSERT INTO url_annotations(url, notes, created_at, updated_at)
                 VALUES (?1, 'orphan note', '2026-01-01', '2026-01-01')",
                params![SEEDED_URL],
            )
            .expect("note");
        refresh_notes_tags_text_for_url(&paths, &archive, SEEDED_URL).expect("no-doc no-op");

        // An unknown url resolves zero url_ids → the fan-out loop body never runs (and the notes/tags
        // recompute hits the absent-row default '' branches).
        refresh_notes_tags_text_for_url(&paths, &archive, "https://nonexistent.example/")
            .expect("unknown-url no-op");
    }

    #[test]
    fn note_is_mirrored_into_the_projection_term_fts_after_set_notes() {
        // Projection-level mirror assertion (sibling of `rebuild_search_projection_mirrors_stored_
        // enrichment_text`): after a set_notes write, a direct `history_search_terms MATCH '<note word>'`
        // against the search database returns exactly one row. Like the enrichment_text precedent, the
        // notes_text column rides the `unicode61` term FTS, so this asserts a word-token match.
        let (_dir, paths, config) = seeded_plaintext_archive();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "wasmtime sandbox".into(),
                source_profile: None,
            },
        )
        .expect("set_notes");
        let search = open_search_connection(&paths).expect("search");
        let hits: i64 = search
            .query_row(
                "SELECT COUNT(*) FROM history_search_terms WHERE history_search_terms MATCH 'wasmtime'",
                [],
                |row| row.get(0),
            )
            .expect("match");
        assert_eq!(hits, 1, "the saved note must be mirrored into the term FTS index");
    }

    #[test]
    fn cjk_note_is_findable_by_plain_keyword_search_end_to_end() {
        // The actual user's failing case: a CJK note searched by a CJK SUBSTRING. PathKeep's CJK recall
        // is gram/trigram-based, so this only works because notes now fold into compact_text + cjk_grams
        // (not just the unicode61 notes_text). "设计系统" appears ONLY in the note (url=…/o/r, title=o/r).
        // On the pre-CJK-extension state (notes only in the unicode61 term FTS) this returns nothing —
        // a contiguous-CJK note tokenizes as ONE unicode61 token, so a "设计系统" substring never hits.
        let (_dir, paths, config) = seeded_plaintext_archive();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "meta 的设计系统".into(),
                source_profile: None,
            },
        )
        .expect("set_notes");
        assert!(
            plain_keyword_finds_seeded_url(&paths, &config, "设计系统"),
            "a CJK note must be findable by a plain CJK substring search"
        );
    }

    #[test]
    fn cjk_tag_is_findable_by_plain_keyword_search_end_to_end() {
        // Same CJK contract for a TAG: a plain CJK keyword search must surface the page via the gram/
        // trigram plane (the tag word appears ONLY in url_tags).
        let (_dir, paths, config) = seeded_plaintext_archive();
        replace_tags(
            &paths,
            &config,
            None,
            ReplaceTagsRequest {
                url: SEEDED_URL.into(),
                tags: vec!["参考资料".into()],
                source_profile: None,
            },
        )
        .expect("replace_tags");
        assert!(
            plain_keyword_finds_seeded_url(&paths, &config, "参考资料"),
            "a CJK tag must be findable by a plain CJK keyword search"
        );
    }

    #[test]
    fn edited_cjk_note_is_findable_and_old_term_drops_via_refresh_hook() {
        // CJK edit through the refresh hook only (no full rebuild): the new CJK term becomes findable AND
        // the replaced CJK term stops matching (its old grams + trigram compact row are removed). CJK has
        // no Latin fuzzy fallback, so an absent term genuinely returns zero rows.
        let (_dir, paths, config) = seeded_plaintext_archive();
        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "图书馆".into(),
                source_profile: None,
            },
        )
        .expect("set first CJK note");
        assert!(plain_keyword_finds_seeded_url(&paths, &config, "图书馆"));

        set_notes(
            &paths,
            &config,
            None,
            SetNotesRequest {
                url: SEEDED_URL.into(),
                notes: "天气预报".into(),
                source_profile: None,
            },
        )
        .expect("edit CJK note");
        assert!(
            plain_keyword_finds_seeded_url(&paths, &config, "天气预报"),
            "the edited CJK note's new term must be findable"
        );
        assert!(
            !plain_keyword_finds_seeded_url(&paths, &config, "图书馆"),
            "the replaced CJK note's old term must no longer match"
        );
    }

    #[test]
    fn rebuild_core_emits_initial_mid_loop_and_final_progress_ticks() {
        // The reprojection reports an initial `0/total`, one tick every `stride`
        // documents, and a final `total/total` — proving the FE gets REAL moving
        // progress across the tail, not a single 0→100 jump. Driven with a tiny
        // stride so a 3-doc corpus crosses the mid-loop boundary once.
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
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'test', '2026-01-01', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("run");
        archive
            .execute(
                "INSERT INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                 VALUES (1, 'chrome', 'x', 'p', '/p', '2026-01-01', 1, 'chrome:Default', '2026-01-01')",
                [],
            )
            .expect("profile");
        for id in 1..=3 {
            archive
                .execute(
                    "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                     VALUES (?1, ?2, 't', 1, 0, 1, '', 1, '', 1, 1)",
                    params![id, format!("https://s{id}.example.com/")],
                )
                .expect("url");
        }

        let mut events: Vec<(u64, u64)> = Vec::new();
        // stride = 2 over 3 docs => initial 0/3, one mid-loop tick at 2/3, final 3/3.
        rebuild_search_projection_core(
            &archive,
            &paths,
            &HashMap::new(),
            3,
            2,
            &mut |processed, total| events.push((processed, total)),
        )
        .expect("rebuild with progress");
        assert_eq!(
            events,
            vec![(0, 3), (2, 3), (3, 3)],
            "an initial tick, a per-stride mid-loop tick, and a terminal tick"
        );
    }
}
