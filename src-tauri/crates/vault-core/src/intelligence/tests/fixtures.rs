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

use vault_store::{Session, params_owned as params};

/// Appends one canonical URL, visit, and optional search term row.
pub(super) fn append_fixture_visit(
    connection: &Session<'_>,
    visit_id: i64,
    url: &str,
    title: &str,
    visit_time_ms: i64,
    from_visit: Option<i64>,
    normalized_search_term: Option<&str>,
) {
    let url_id = visit_id + 10;
    let page_id=connection.query_one("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?3,?4) ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",params![url_id,url,crate::pages::canonical_key(url),crate::visit_taxonomy::registrable_domain_for_url(url)],|r|Ok(r.get::<i64>(0)?)).unwrap();
    let visit_time_iso =
        chrono::DateTime::from_timestamp_millis(visit_time_ms).expect("timestamp millis");
    connection
        .execute(
            "INSERT INTO urls (
                id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at,
                page_id
             ) VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, 1, 1, ?6, 0, ?7, '2026-04-14T00:00:00Z', ?8)",
            params![
                url_id,
                url,
                title,
                visit_time_ms,
                visit_time_iso.to_rfc3339(),
                url_id + 100,
                format!("hash-{visit_id}"),
                page_id
            ],
        )
        .expect("insert url");
    connection
        .execute(
            "INSERT INTO visits (
                id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at, parent_visit_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, 1, 1, ?6, 0, ?7, ?8, '2026-04-14T00:00:00Z', (SELECT id FROM visits WHERE id=?6 AND source_profile_id=1))",
            params![
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
    connection.execute("UPDATE archive_totals SET value=value+1 WHERE name IN ('visits','urls','sourceVisits:1')",()).unwrap();
    if let Some(normalized_search_term) = normalized_search_term {
        connection
            .execute(
                "INSERT INTO search_terms (
                    id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                 ) VALUES (?1, ?2, ?3, ?3, 1, 1, 'chrome:Default')",
                params![visit_id + 1000, url_id, normalized_search_term],
            )
            .expect("insert search term");
    }
}

/// Appends a deterministic mixed search/content sequence across a batch boundary.
pub(super) fn append_many_fixture_visits(
    connection: &Session<'_>,
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
    connection: &Session<'_>,
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
    connection: &Session<'_>,
) -> Vec<(i64, String, String, i64)> {
    connection
        .prepare(
            "SELECT visit_id, registrable_domain, canonical_url, is_new_domain
             FROM visit_derived_facts
             ORDER BY visit_id ASC",
        )
        .expect("prepare visit-derived facts")
        .query_map((), |row| {
            Ok((
                row.get::<i64>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
                row.get::<i64>(3)?,
            ))
        })
        .expect("query visit-derived facts")
        .collect::<vault_store::Result<Vec<_>>>()
        .expect("collect visit-derived facts")
}

/// Loads rollup rows as normalized strings so whole-table equality is easy.
pub(super) fn load_daily_rollup_rows(connection: &Session<'_>, table: &str) -> Vec<String> {
    let columns = match table {
        "domain_daily_rollups" => 8,
        "category_daily_rollups" => 5,
        "engine_daily_rollups" => 4,
        "daily_summary_rollups" => 8,
        _ => panic!("rollup fixture table"),
    };
    connection
        .query(&format!("SELECT * FROM {table} ORDER BY 1,2,3"), (), |row| {
            let mut values = Vec::new();
            for index in 0..columns {
                values.push(match row.get_value(index)? {
                    vault_store::Value::Null => "NULL".to_owned(),
                    vault_store::Value::Integer(v) => v.to_string(),
                    vault_store::Value::Real(v) => format!("{v:.6}"),
                    vault_store::Value::Text(v) => v,
                    vault_store::Value::Blob(v) => format!("blob:{}", v.len()),
                });
            }
            Ok(values.join("|"))
        })
        .unwrap()
}

/// Seeds the minimal archive graph used by most Core Intelligence regressions.
pub(super) fn seed_core_intelligence_fixture(connection: &Session<'_>) {
    for (id, url, domain) in [
        (1, "https://www.google.com/search?q=sqlite+wal", "google.com"),
        (2, "https://github.com/example/repo/issues/42", "github.com"),
    ] {
        connection
            .execute(
                "INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?3,?4)",
                params![id, url, crate::pages::canonical_key(url), domain],
            )
            .unwrap();
    }
    connection
        .execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',3)", ())
        .unwrap();
    connection.execute("UPDATE archive_totals SET value=CASE name WHEN 'visits' THEN 3 WHEN 'urls' THEN 2 ELSE value END",()).unwrap();
    connection
        .execute(
            "INSERT INTO runs (
                id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
             ) VALUES (
                1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0
             )",
            (),
        )
        .expect("run");
    connection
        .execute(
            "INSERT INTO source_profiles (
                id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
             ) VALUES (
                1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z'
             )",
            (),
        )
        .expect("profile");
    connection
        .execute(
            "INSERT INTO urls (
                id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at,
                page_id
             ) VALUES
             (1, 'https://www.google.com/search?q=sqlite+wal', 'sqlite wal - Google Search', 1, 0, 1, '1970-01-01T00:00:00Z', 1, '1970-01-01T00:00:00Z', 1, 1, 11, 0, 'hash-1', '2026-04-14T00:00:00Z', 1),
             (2, 'https://github.com/example/repo/issues/42', 'Issue 42', 2, 1, 2, '1970-01-01T00:00:02Z', 86400002, '1970-01-02T00:00:00Z', 1, 1, 12, 0, 'hash-2', '2026-04-14T00:00:00Z', 2)",
            (),
        )
        .expect("urls");
    connection
        .execute(
            "INSERT INTO visits (
                id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at, parent_visit_id
             ) VALUES
             (1, 1, '1', 1711929600000, '2024-04-01T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-1', 'visit-hash-1', '2026-04-14T00:00:00Z', NULL),
             (2, 2, '2', 1711929660000, '2024-04-01T00:01:00Z', 1, 0, 1, 1, 1, 0, 'fingerprint-2', 'visit-hash-2', '2026-04-14T00:00:00Z', 1),
             (3, 2, '3', 1712016000000, '2024-04-02T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-3', 'visit-hash-3', '2026-04-14T00:00:00Z', NULL)",
            (),
        )
        .expect("visits");
    connection
        .execute(
            "INSERT INTO search_terms (
                id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
             ) VALUES (
                1, 1, 'sqlite wal', 'sqlite wal', 1, 1, 'chrome:Default'
             )",
            (),
        )
        .expect("search term");
}

/// Direct fixture edits keep the same canonical counters maintained by archive writers.
pub(super) fn sync_archive_totals(db: &crate::test_support::native::TestDatabase) {
    db.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name='visits'",()).unwrap();
    db.execute(
        "UPDATE archive_totals SET value=(SELECT COUNT(*) FROM pages) WHERE name='urls'",
        (),
    )
    .unwrap();
    db.execute("INSERT INTO archive_totals(name,value) SELECT 'sourceVisits:'||sp.id,COUNT(v.id) FROM source_profiles sp LEFT JOIN visits v ON v.source_profile_id=sp.id AND v.reverted_at IS NULL GROUP BY sp.id ON CONFLICT(name) DO UPDATE SET value=excluded.value",()).unwrap();
}
