//! Shared fixtures for Core Intelligence regression modules.
//!
//! ## Responsibilities
//! - Seed a tiny but realistic archive graph with profiles, URLs, visits, and
//!   search terms.
//! - Append deterministic visit chains used by incremental and batch tests.
//! - Normalize result rows for equality checks across rebuild modes.
//!
//! ## Not responsible for
//! - Testing behavior directly; assertions belong in sibling test modules.
//! - Creating large benchmark archives.
//!
//! ## Dependencies
//! - `rusqlite` writes directly to temporary archive databases.
//! - Core Intelligence schema tests call `has_index` for migration assertions.
//!
//! ## Performance notes
//! Helpers insert only the rows needed by each regression. Batch-boundary tests
//! scale by count but keep payload columns small to avoid unnecessary memory or
//! disk pressure during normal quality gates.

use rusqlite::{Connection, OptionalExtension};

/// Checks whether a schema migration created a named index.
pub(super) fn has_index(connection: &Connection, index_name: &str) -> bool {
    connection
        .query_row(
            "SELECT 1
             FROM sqlite_master
             WHERE type = 'index'
               AND name = ?1
             LIMIT 1",
            [index_name],
            |_| Ok(()),
        )
        .optional()
        .expect("index lookup")
        .is_some()
}

/// Appends one canonical URL, visit, and optional search term row.
pub(super) fn append_fixture_visit(
    connection: &Connection,
    visit_id: i64,
    url: &str,
    title: &str,
    visit_time_ms: i64,
    from_visit: Option<i64>,
    normalized_search_term: Option<&str>,
) {
    let url_id = visit_id + 10;
    let visit_time_iso =
        chrono::DateTime::from_timestamp_millis(visit_time_ms).expect("timestamp millis");
    connection
        .execute(
            "INSERT INTO urls (
                id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
             ) VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, 1, 1, ?6, 0, ?7, '2026-04-14T00:00:00Z')",
            rusqlite::params![
                url_id,
                url,
                title,
                visit_time_ms,
                visit_time_iso.to_rfc3339(),
                url_id + 100,
                format!("hash-{visit_id}")
            ],
        )
        .expect("insert url");
    connection
        .execute(
            "INSERT INTO visits (
                id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, 1, 1, ?6, 0, ?7, ?8, '2026-04-14T00:00:00Z')",
            rusqlite::params![
                visit_id,
                url_id,
                visit_id.to_string(),
                visit_time_ms,
                visit_time_iso.to_rfc3339(),
                from_visit,
                format!("fingerprint-{visit_id}"),
                format!("visit-hash-{visit_id}")
            ],
        )
        .expect("insert visit");
    if let Some(normalized_search_term) = normalized_search_term {
        connection
            .execute(
                "INSERT INTO search_terms (
                    id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                 ) VALUES (?1, ?2, ?3, ?3, 1, 1, 'chrome:Default')",
                rusqlite::params![visit_id + 1000, url_id, normalized_search_term],
            )
            .expect("insert search term");
    }
}

/// Appends a deterministic mixed search/content sequence across a batch boundary.
pub(super) fn append_many_fixture_visits(
    connection: &Connection,
    start_visit_id: i64,
    count: usize,
    start_time_ms: i64,
) {
    for offset in 0..count {
        let visit_id = start_visit_id + offset as i64;
        let query = format!("incremental topic {}", offset % 9);
        let (url, title, from_visit, normalized_search_term) = match offset % 3 {
            0 => (
                format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
                format!("Search {query}"),
                None,
                Some(query),
            ),
            1 => (
                format!("https://docs.incremental-{}.dev/guide/{}", offset % 7, offset),
                format!("Guide {offset}"),
                Some(visit_id - 1),
                None,
            ),
            _ => (
                format!("https://reference.incremental-{}.dev/page/{}", offset % 5, offset),
                format!("Reference {offset}"),
                Some(visit_id - 1),
                None,
            ),
        };
        append_fixture_visit(
            connection,
            visit_id,
            &url,
            &title,
            start_time_ms + (offset as i64 * 60_000),
            from_visit,
            normalized_search_term.as_deref(),
        );
    }
}

/// Appends a linked visit chain that should stay in one structural trail.
pub(super) fn append_fixture_chain_visits(
    connection: &Connection,
    start_visit_id: i64,
    count: usize,
    start_time_ms: i64,
    from_visit_seed: i64,
) {
    let mut previous_visit_id = from_visit_seed;
    for offset in 0..count {
        let visit_id = start_visit_id + offset as i64;
        append_fixture_visit(
            connection,
            visit_id,
            &format!("https://github.com/example/repo/pulls/{visit_id}"),
            &format!("Pull Request {visit_id}"),
            start_time_ms + (offset as i64 * 60_000),
            Some(previous_visit_id),
            None,
        );
        previous_visit_id = visit_id;
    }
}

/// Loads derived fact rows in stable order for rebuild-equivalence checks.
pub(super) fn load_visit_derived_fact_rows(
    connection: &Connection,
) -> Vec<(i64, String, String, i64)> {
    connection
        .prepare(
            "SELECT visit_id, registrable_domain, canonical_url, is_new_domain
             FROM visit_derived_facts
             ORDER BY visit_id ASC",
        )
        .expect("prepare visit-derived facts")
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("query visit-derived facts")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect visit-derived facts")
}

/// Loads rollup rows as normalized strings so whole-table equality is easy.
pub(super) fn load_daily_rollup_rows(connection: &Connection, table: &str) -> Vec<String> {
    let sql = format!("SELECT * FROM {table} ORDER BY 1, 2, 3");
    let mut statement = connection.prepare(&sql).expect("prepare rollup rows");
    statement
        .query_map([], |row| {
            let mut values = Vec::with_capacity(row.as_ref().column_count());
            for index in 0..row.as_ref().column_count() {
                let value = row.get_ref(index)?;
                let normalized = match value {
                    rusqlite::types::ValueRef::Null => "NULL".to_string(),
                    rusqlite::types::ValueRef::Integer(inner) => inner.to_string(),
                    rusqlite::types::ValueRef::Real(inner) => format!("{inner:.6}"),
                    rusqlite::types::ValueRef::Text(inner) => {
                        String::from_utf8_lossy(inner).into_owned()
                    }
                    rusqlite::types::ValueRef::Blob(inner) => format!("blob:{}", inner.len()),
                };
                values.push(normalized);
            }
            Ok(values.join("|"))
        })
        .expect("query rollup rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect rollup rows")
}

/// Seeds the minimal archive graph used by most Core Intelligence regressions.
pub(super) fn seed_core_intelligence_fixture(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO runs (
                id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
             ) VALUES (
                1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0
             )",
            [],
        )
        .expect("run");
    connection
        .execute(
            "INSERT INTO source_profiles (
                id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
             ) VALUES (
                1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z'
             )",
            [],
        )
        .expect("profile");
    connection
        .execute(
            "INSERT INTO urls (
                id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
             ) VALUES
             (1, 'https://www.google.com/search?q=sqlite+wal', 'sqlite wal - Google Search', 1, 0, 1, '1970-01-01T00:00:00Z', 1, '1970-01-01T00:00:00Z', 1, 1, 11, 0, 'hash-1', '2026-04-14T00:00:00Z'),
             (2, 'https://github.com/example/repo/issues/42', 'Issue 42', 2, 1, 2, '1970-01-01T00:00:02Z', 86400002, '1970-01-02T00:00:00Z', 1, 1, 12, 0, 'hash-2', '2026-04-14T00:00:00Z')",
            [],
        )
        .expect("urls");
    connection
        .execute(
            "INSERT INTO visits (
                id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
             ) VALUES
             (1, 1, '1', 1711929600000, '2024-04-01T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-1', 'visit-hash-1', '2026-04-14T00:00:00Z'),
             (2, 2, '2', 1711929660000, '2024-04-01T00:01:00Z', 1, 0, 1, 1, 1, 0, 'fingerprint-2', 'visit-hash-2', '2026-04-14T00:00:00Z'),
             (3, 2, '3', 1712016000000, '2024-04-02T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-3', 'visit-hash-3', '2026-04-14T00:00:00Z')",
            [],
        )
        .expect("visits");
    connection
        .execute(
            "INSERT INTO search_terms (
                id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
             ) VALUES (
                1, 1, 'sqlite wal', 'sqlite wal', 1, 1, 'chrome:Default'
             )",
            [],
        )
        .expect("search term");
}

/// Adds URL-like query noise plus one valid repository-search query.
pub(super) fn seed_search_keyword_noise_fixture(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO urls (
                id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
             ) VALUES
             (3, 'https://www.google.com/search?q=https%3A%2F%2Fasu.edu', 'asu.edu - Google Search', 1, 0, 1711929720000, '2024-04-01T00:02:00Z', 1711929720000, '2024-04-01T00:02:00Z', 1, 1, 13, 0, 'hash-3', '2026-04-14T00:00:00Z'),
             (4, 'https://asu.edu/', 'Arizona State University', 1, 0, 1711929780000, '2024-04-01T00:03:00Z', 1711929780000, '2024-04-01T00:03:00Z', 1, 1, 14, 0, 'hash-4', '2026-04-14T00:00:00Z'),
             (5, 'https://github.com/search?q=pathkeep+sqlite', 'Repository search results', 1, 0, 1711929840000, '2024-04-01T00:04:00Z', 1711929840000, '2024-04-01T00:04:00Z', 1, 1, 15, 0, 'hash-5', '2026-04-14T00:00:00Z'),
             (6, 'https://github.com/example/pathkeep', 'PathKeep repo', 1, 0, 1711929900000, '2024-04-01T00:05:00Z', 1711929900000, '2024-04-01T00:05:00Z', 1, 1, 16, 0, 'hash-6', '2026-04-14T00:00:00Z')",
            [],
        )
        .expect("extra urls");
    connection
        .execute(
            "INSERT INTO visits (
                id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
             ) VALUES
             (4, 3, '4', 1711929720000, '2024-04-01T00:02:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-4', 'visit-hash-4', '2026-04-14T00:00:00Z'),
             (5, 4, '5', 1711929780000, '2024-04-01T00:03:00Z', 1, 0, 1, 1, 4, 0, 'fingerprint-5', 'visit-hash-5', '2026-04-14T00:00:00Z'),
             (6, 5, '6', 1711929840000, '2024-04-01T00:04:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-6', 'visit-hash-6', '2026-04-14T00:00:00Z'),
             (7, 6, '7', 1711929900000, '2024-04-01T00:05:00Z', 1, 0, 1, 1, 6, 0, 'fingerprint-7', 'visit-hash-7', '2026-04-14T00:00:00Z')",
            [],
        )
        .expect("extra visits");
}
