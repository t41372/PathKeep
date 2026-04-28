//! Session and trail Core Intelligence read models.
//!
//! ## Responsibilities
//! - Serve session list/detail and trail list/detail queries for route-level
//!   Core Intelligence and Explorer-linked surfaces.
//! - Keep session/trail row decoding and session-local member loading out of
//!   the parent intelligence module.
//! - Reuse one consistent trail-summary decoder across refind and query-family
//!   detail reads.
//!
//! ## Not responsible for
//! - Rebuilding sessions or trails.
//! - Search-query, query-family, or domain-deep-dive reads.
//! - Export payload formatting or explainability surfaces.
//!
//! ## Dependencies
//! - Parent-module date-range helpers and shared archive connection setup.
//! - `sessions`, `search_trails`, `search_trail_members`, and
//!   `visit_derived_facts` in the intelligence plane plus archive joins.
//!
//! ## Performance notes
//! - List reads stay bounded by the requested page size.
//! - Detail reads only load rows linked to one `session_id` or `trail_id`;
//!   they do not scan unrelated sessions/trails in the same date range.

use super::{date_range_bounds, ensure_core_intelligence_schema};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        AppConfig, PagedDateRangeRequest, SearchTrailQueryRequest, SessionDetail,
        SessionListResult, SessionSummary, SessionVisit, TrailDetail, TrailListResult, TrailMember,
        TrailSummary,
    },
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

/// Returns one page of deterministic sessions for the requested scope and date
/// range.
pub fn get_sessions(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM sessions
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
         FROM sessions
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
         ORDER BY first_visit_ms DESC, session_id DESC
         LIMIT ?4 OFFSET ?5",
    )?;
    let sessions = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::<_, i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SessionListResult { sessions, total, page: request.page, page_size: request.page_size })
}

/// Loads one session plus the visits and trails that belong to it so the
/// frontend can inspect the deterministic grouping.
pub fn get_session_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    session_id: &str,
) -> Result<SessionDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let session = connection
        .query_row(
            "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
             FROM sessions WHERE session_id = ?1",
            [session_id],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::<_, i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )
        .optional()?
        .with_context(|| format!("session {session_id} was not found"))?;
    let visits = load_session_visits(&connection, session_id)?;
    let trails = load_session_trails(&connection, session_id)?;
    Ok(SessionDetail { session, visits, trails })
}

/// Returns one page of search trails for the requested scope and engine filter.
pub fn get_search_trails(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4",
        params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4
         ORDER BY first_visit_ms DESC, trail_id DESC
         LIMIT ?5 OFFSET ?6",
    )?;
    let trails = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.engine.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            trail_summary_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TrailListResult { trails, total, page: request.page, page_size: request.page_size })
}

/// Loads one deterministic trail plus all ordered member visits that belong to
/// it.
pub fn get_trail_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    trail_id: &str,
) -> Result<TrailDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let trail = connection
        .query_row(
            "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                    landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
             FROM search_trails
             WHERE trail_id = ?1",
            [trail_id],
            trail_summary_from_row,
        )
        .optional()?
        .with_context(|| format!("trail {trail_id} was not found"))?;
    let mut statement = connection.prepare(
        "SELECT search_trail_members.trail_id, search_trail_members.visit_id, search_trail_members.ordinal,
                search_trail_members.role, urls.url, visit_derived_facts.canonical_url, urls.title,
                visit_derived_facts.registrable_domain, visits.visit_time_ms, visit_derived_facts.search_query
         FROM search_trail_members
         JOIN archive.visits AS visits ON visits.id = search_trail_members.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         LEFT JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         WHERE search_trail_members.trail_id = ?1
         ORDER BY search_trail_members.ordinal ASC",
    )?;
    let members = statement
        .query_map([trail_id], |row| {
            Ok(TrailMember {
                trail_id: row.get(0)?,
                visit_id: row.get(1)?,
                ordinal: row.get(2)?,
                role: row.get(3)?,
                url: row.get(4)?,
                canonical_url: row.get(5)?,
                title: row.get(6)?,
                registrable_domain: row.get(7)?,
                visit_time_ms: row.get(8)?,
                search_query: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TrailDetail { trail, members })
}

/// Loads all ordered visits that the given deterministic session references.
fn load_session_visits(connection: &Connection, session_id: &str) -> Result<Vec<SessionVisit>> {
    let mut statement = connection.prepare(
        "SELECT visits.id, urls.url, urls.title, visit_derived_facts.registrable_domain, visits.visit_time_ms,
                visit_derived_facts.is_search_event, visit_derived_facts.search_query,
                visit_derived_facts.search_engine, visit_derived_facts.trail_id, visits.transition_type
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.session_id = ?1
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map([session_id], |row| {
            Ok(SessionVisit {
                visit_id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                registrable_domain: row.get(3)?,
                visit_time_ms: row.get(4)?,
                is_search_event: row.get::<_, i64>(5)? != 0,
                search_query: row.get(6)?,
                search_engine: row.get(7)?,
                trail_id: row.get(8)?,
                transition_type: row.get::<_, Option<i64>>(9)?.map(|value| value.to_string()),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Loads all persisted trails that belong to one session.
fn load_session_trails(connection: &Connection, session_id: &str) -> Result<Vec<TrailSummary>> {
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE session_id = ?1
         ORDER BY first_visit_ms ASC, trail_id ASC",
    )?;
    statement
        .query_map([session_id], trail_summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Decodes one `search_trails` row into the shared summary type that multiple
/// higher-level read models reuse.
pub(super) fn trail_summary_from_row(row: &Row<'_>) -> rusqlite::Result<TrailSummary> {
    Ok(TrailSummary {
        trail_id: row.get(0)?,
        session_id: row.get(1)?,
        initial_query: row.get(2)?,
        search_engine: row.get(3)?,
        reformulation_count: row.get(4)?,
        visit_count: row.get(5)?,
        landing_url: row.get(6)?,
        landing_domain: row.get(7)?,
        first_visit_ms: row.get(8)?,
        last_visit_ms: row.get(9)?,
        max_depth: row.get(10)?,
        queries: serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::{get_search_trails, get_session_detail, get_sessions, get_trail_detail};
    use crate::{
        archive::{open_archive_connection, open_intelligence_connection},
        config::{ProjectPaths, project_paths_with_root},
        models::{
            AppConfig, ArchiveMode, DateRange, PagedDateRangeRequest, SearchTrailQueryRequest,
        },
    };
    use rusqlite::{Connection, params};

    fn range() -> DateRange {
        DateRange { start: "1970-01-01".to_string(), end: "2100-01-01".to_string() }
    }

    fn config() -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        }
    }

    fn seed_archive(paths: &ProjectPaths, config: &AppConfig) {
        let archive = open_archive_connection(paths, config, None).expect("archive connection");
        archive
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("run");
        archive
            .execute(
                "INSERT INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                 VALUES (1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'profile-a', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("profile");

        let urls = [
            (1_i64, "https://google.com/search?q=sqlite+wal", "Search"),
            (2_i64, "https://sqlite.org/wal.html", "SQLite WAL"),
            (3_i64, "https://postgresql.org/docs/wal.html", "Postgres WAL"),
        ];
        for (id, url, title) in urls {
            archive
                .execute(
                    "INSERT INTO urls
                     (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso,
                      last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id,
                      source_url_id, hidden, payload_hash, recorded_at)
                     VALUES (?1, ?2, ?3, 1, 0, ?4, '2027-01-15T08:00:00Z',
                             ?4, '2027-01-15T08:00:00Z', 1, 1, ?1, 0, ?5,
                             '2026-04-14T00:00:00Z')",
                    params![
                        id,
                        url,
                        title,
                        1_800_000_000_000_i64 + id * 1_000,
                        format!("hash-{id}")
                    ],
                )
                .expect("url");
            archive
                .execute(
                    "INSERT INTO visits
                     (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type,
                      visit_duration_ms, source_profile_id, created_by_run_id, from_visit,
                      is_known_to_sync, event_fingerprint, payload_hash, recorded_at)
                     VALUES (?1, ?1, ?2, ?3, '2027-01-15T08:00:00Z', ?4, 0,
                             1, 1, NULL, 0, ?5, ?6, '2026-04-14T00:00:00Z')",
                    params![
                        id,
                        format!("source-{id}"),
                        1_800_000_000_000_i64 + id * 1_000,
                        id,
                        format!("fingerprint-{id}"),
                        format!("visit-hash-{id}")
                    ],
                )
                .expect("visit");
        }
    }

    fn seed_intelligence(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO sessions
                 (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count,
                  domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES ('session-1', 'profile-a', 1_800_000_001_000, 1_800_000_003_000,
                         3, 1, 3, 1, 'SQLite WAL research', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("session");
        connection
            .execute(
                "INSERT INTO sessions
                 (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count,
                  domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES ('session-old', 'profile-b', 500, 600, 1, 0, 1, 0, NULL, '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("old session");
        connection
            .execute(
                "INSERT INTO search_trails
                 (trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count,
                  visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth,
                  queries_json, computed_at)
                 VALUES ('trail-1', 'profile-a', 'session-1', 'sqlite wal', 'google', 1, 3,
                         'https://sqlite.org/wal.html', 'sqlite.org',
                         1_800_000_001_000, 1_800_000_003_000, 2,
                         '[\"sqlite wal\",\"sqlite checkpoint\"]', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("trail");

        let facts = [
            (
                1_i64,
                "https://google.com/search?q=sqlite+wal",
                "google.com",
                "search",
                1_i64,
                Some("sqlite wal"),
                Some("google"),
                Some("trail-1"),
            ),
            (
                2_i64,
                "https://sqlite.org/wal.html",
                "sqlite.org",
                "docs_page",
                0_i64,
                None,
                None,
                Some("trail-1"),
            ),
            (
                3_i64,
                "https://postgresql.org/docs/wal.html",
                "postgresql.org",
                "docs_page",
                0_i64,
                None,
                None,
                Some("trail-1"),
            ),
        ];
        for (visit_id, canonical_url, domain, page_category, is_search, query, engine, trail_id) in
            facts
        {
            connection
                .execute(
                    "INSERT INTO visit_derived_facts
                     (visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url,
                      domain_category, page_category, search_engine, search_query, is_search_event,
                      evidence_tier, taxonomy_source, computed_at)
                     VALUES (?1, 'profile-a', 'session-1', ?2, ?3, ?4, 'reference',
                             ?5, ?6, ?7, ?8, 'tier-a', 'test', '2026-04-14T00:00:00Z')",
                    params![
                        visit_id,
                        trail_id,
                        domain,
                        canonical_url,
                        page_category,
                        engine,
                        query,
                        is_search
                    ],
                )
                .expect("derived fact");
            connection
                .execute(
                    "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
                     VALUES ('trail-1', 'profile-a', ?1, ?1, ?2)",
                    params![visit_id, if is_search == 1 { "search" } else { "result" }],
                )
                .expect("trail member");
        }
    }

    fn seed_project() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
        let root = tempfile::tempdir().expect("temp project root");
        let paths = project_paths_with_root(root.path());
        let config = config();
        seed_archive(&paths, &config);
        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence connection");
        seed_intelligence(&intelligence);
        (root, paths, config)
    }

    #[test]
    fn session_and_trail_public_reads_return_bounded_details() {
        let (_root, paths, config) = seed_project();
        let sessions = get_sessions(
            &paths,
            &config,
            None,
            &PagedDateRangeRequest {
                date_range: range(),
                profile_id: Some("profile-a".to_string()),
                page: 0,
                page_size: 10,
            },
        )
        .expect("sessions");
        assert_eq!(sessions.total, 1);
        assert_eq!(sessions.sessions[0].session_id, "session-1");
        assert!(sessions.sessions[0].is_deep_dive);

        let empty_page = get_sessions(
            &paths,
            &config,
            None,
            &PagedDateRangeRequest {
                date_range: range(),
                profile_id: Some("profile-a".to_string()),
                page: 1,
                page_size: 0,
            },
        )
        .expect("sessions with saturated page size");
        assert!(empty_page.sessions.is_empty());
        assert_eq!(empty_page.page_size, 0);

        let detail =
            get_session_detail(&paths, &config, None, "session-1").expect("session detail");
        assert_eq!(detail.visits.len(), 3);
        assert_eq!(detail.visits[0].transition_type.as_deref(), Some("1"));
        assert_eq!(detail.visits[0].search_query.as_deref(), Some("sqlite wal"));
        assert_eq!(detail.trails[0].queries, vec!["sqlite wal", "sqlite checkpoint"]);
        assert!(get_session_detail(&paths, &config, None, "missing").is_err());

        let trails = get_search_trails(
            &paths,
            &config,
            None,
            &SearchTrailQueryRequest {
                date_range: range(),
                profile_id: Some("profile-a".to_string()),
                engine: Some("google".to_string()),
                page: 0,
                page_size: 10,
            },
        )
        .expect("trails");
        assert_eq!(trails.total, 1);
        assert_eq!(trails.trails[0].landing_domain.as_deref(), Some("sqlite.org"));

        let no_engine_match = get_search_trails(
            &paths,
            &config,
            None,
            &SearchTrailQueryRequest {
                date_range: range(),
                profile_id: Some("profile-a".to_string()),
                engine: Some("duckduckgo".to_string()),
                page: 0,
                page_size: 10,
            },
        )
        .expect("filtered trails");
        assert!(no_engine_match.trails.is_empty());

        let trail = get_trail_detail(&paths, &config, None, "trail-1").expect("trail detail");
        assert_eq!(trail.members.len(), 3);
        assert_eq!(trail.members[0].role, "search");
        assert_eq!(trail.members[1].registrable_domain.as_deref(), Some("sqlite.org"));
        assert!(get_trail_detail(&paths, &config, None, "missing").is_err());
    }
}
