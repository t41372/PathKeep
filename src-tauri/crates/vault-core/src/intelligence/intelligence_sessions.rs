//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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

use super::date_range_bounds;
use crate::models::{
    PagedDateRangeRequest, SearchTrailQueryRequest, SessionListResult, SessionSummary,
    TrailListResult, TrailSummary,
};
use anyhow::Result;
use vault_store::{Row, params_owned as params};

/// Returns one page of deterministic sessions for the requested scope and date
/// range.
pub fn get_sessions(
    vault: &crate::Vault,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence,move |connection| {
let request=&request;

    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    // Exact interval overlap includes sessions spanning any number of days.
    let total: i64 = connection.query_one(
        "SELECT COUNT(*)
         FROM sessions
         WHERE profile_id IN (
                 SELECT profile_key FROM archive.source_profiles
                 WHERE profile_key IS NOT NULL AND (?1 IS NULL OR profile_key = ?1))
           AND first_visit_ms < ?3
           AND last_visit_ms >= ?2",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| Ok(row.get(0)?),
    )?;
    let limit=request.page_size.clamp(1,1000) as usize;
    // A cursor at or past the window's end excludes nothing; any other cursor replaces the end.
    let cursor=super::paging::cursor::<(i64,String)>(request.cursor.as_deref())?.filter(|c|c.0<end_ms);
    let upper=if cursor.is_some() {"r.first_visit_ms<=?4 AND (r.first_visit_ms<?4 OR r.session_id<?5)"} else {"r.first_visit_ms<?3"};
    // Through the `sessions` view Turso reads the whole generation and sorts it on every page.
    // The rows table, walked through idx_sessions_recent, is read in list order and seeks to the
    // cursor; it needs the bounded cursor form above, with no `?4 IS NULL OR` escape.
    let mut statement = connection.prepare(&format!(
        "SELECT r.session_id, r.first_visit_ms, r.last_visit_ms, r.visit_count, r.search_count, r.domain_count, r.is_deep_dive, r.auto_title
         FROM sessions_rows r INDEXED BY idx_sessions_recent
         WHERE r.generation IN (SELECT navigation FROM intelligence_generations)
           AND r.profile_id IN (
                 SELECT profile_key FROM archive.source_profiles
                 WHERE profile_key IS NOT NULL AND (?1 IS NULL OR profile_key = ?1))
           AND r.last_visit_ms >= ?2
           AND {upper}
         ORDER BY r.first_visit_ms DESC, r.session_id DESC LIMIT ?6"
    ))?;
    let sessions = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                cursor.as_ref().map(|c|c.0),cursor.as_ref().map(|c|c.1.as_str()),
                (limit+1) as i64
            ],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::< i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let page=super::paging::finish(sessions.into_iter().map(|row| {let key=(row.first_visit_ms,row.session_id.clone());(row,key)}).collect(),limit)?;
    Ok(SessionListResult {sessions:page.items,total,next_cursor:page.next_cursor,page_size:limit as u32})

})
}

pub use super::session_details::{get_session_detail, get_trail_detail};

/// Returns one page of search trails for the requested scope and engine filter.
pub fn get_search_trails(
    vault: &crate::Vault,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence,move |connection| {
let request=&request;

    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_one(
        "SELECT COUNT(*)
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4",
        params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
        |row| Ok(row.get(0)?),
    )?;
    let limit=request.page_size.clamp(1,1000) as usize;
    // Same shape as `get_sessions`: the rows table in list order, seeking to the cursor.
    let cursor=super::paging::cursor::<(i64,String)>(request.cursor.as_deref())?.filter(|c|c.0<end_ms);
    let upper=if cursor.is_some() {"r.first_visit_ms<=?5 AND (r.first_visit_ms<?5 OR r.trail_id<?6)"} else {"r.first_visit_ms<?4"};
    let mut statement = connection.prepare(&format!(
        "SELECT r.trail_id, r.session_id, r.initial_query, r.search_engine, r.reformulation_count, r.visit_count,
                r.landing_url, r.landing_domain, r.first_visit_ms, r.last_visit_ms, r.max_depth, r.queries_json
         FROM search_trails_rows r INDEXED BY idx_search_trails_recent
         WHERE r.generation IN (SELECT navigation FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)
           AND (?2 IS NULL OR r.search_engine = ?2)
           AND r.last_visit_ms >= ?3
           AND {upper}
         ORDER BY r.first_visit_ms DESC, r.trail_id DESC LIMIT ?7"
    ))?;
    let trails = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.engine.as_deref(),
                start_ms,
                end_ms,
                cursor.as_ref().map(|c|c.0),cursor.as_ref().map(|c|c.1.as_str()),
                (limit+1) as i64
            ],
            trail_summary_from_row,
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let page=super::paging::finish(trails.into_iter().map(|row| {let key=(row.first_visit_ms,row.trail_id.clone());(row,key)}).collect(),limit)?;
    Ok(TrailListResult {trails:page.items,total,next_cursor:page.next_cursor,page_size:limit as u32})

})
}

/// Decodes one `search_trails` row into the shared summary type that multiple
/// higher-level read models reuse.
pub(super) fn trail_summary_from_row(row: &Row) -> vault_store::Result<TrailSummary> {
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
        queries: serde_json::from_str(&row.get::<String>(11)?).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::{get_search_trails, get_session_detail, get_sessions, get_trail_detail};
    use crate::models::{
        AppConfig, ArchiveMode, DateRange, PagedDateRangeRequest, SearchTrailQueryRequest,
    };
    use vault_store::{Session, params_owned as params};

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

    fn seed_archive(archive: &Session<'_>) {
        archive
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                (),
            )
            .expect("run");
        archive
            .execute(
                "INSERT INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                 VALUES (1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'profile-a', '2026-04-14T00:00:00Z')",
                (),
            )
            .expect("profile");

        let urls = [
            (1_i64, "https://google.com/search?q=sqlite+wal", "Search"),
            (2_i64, "https://sqlite.org/wal.html", "SQLite WAL"),
            (3_i64, "https://postgresql.org/docs/wal.html", "Postgres WAL"),
        ];
        for (id, url, title) in urls {
            archive.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?3,?4)",params![id,url,crate::pages::canonical_key(url),crate::visit_taxonomy::registrable_domain_for_url(url)]).unwrap();
            archive
                .execute(
                    "INSERT INTO urls
                     (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso,
                      last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id,
                      source_url_id, hidden, payload_hash, recorded_at, page_id)
                     VALUES (?1, ?2, ?3, 1, 0, ?4, '2027-01-15T08:00:00Z',
                             ?4, '2027-01-15T08:00:00Z', 1, 1, ?1, 0, ?5,
                             '2026-04-14T00:00:00Z', ?1)",
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

    fn seed_intelligence(connection: &Session<'_>) {
        for profile in ["profile-a", "profile-b"] {
            connection
                .execute("INSERT INTO intelligence_generations(profile_id) VALUES(?1)", [profile])
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO sessions_rows
                 (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count,
                  domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES ('session-1', 'profile-a', 1800000001000, 1800000003000,
                         3, 1, 3, 1, 'SQLite WAL research', '2026-04-14T00:00:00Z')",
                (),
            )
            .expect("session");
        connection
            .execute(
                "INSERT INTO sessions_rows
                 (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count,
                  domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES ('session-old', 'profile-b', 500, 600, 1, 0, 1, 0, NULL, '2026-04-14T00:00:00Z')",
                (),
            )
            .expect("old session");
        connection
            .execute(
                "INSERT INTO search_trails_rows
                 (trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count,
                  visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth,
                  queries_json, computed_at)
                 VALUES ('trail-1', 'profile-a', 'session-1', 'sqlite wal', 'google', 1, 3,
                         'https://sqlite.org/wal.html', 'sqlite.org',
                         1800000001000, 1800000003000, 2,
                         '[\"sqlite wal\",\"sqlite checkpoint\"]', '2026-04-14T00:00:00Z')",
                (),
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
            connection.execute("INSERT INTO visit_assignments(visit_id,profile_id,session_id,trail_id) VALUES(?1,'profile-a','session-1',?2)",params![visit_id,trail_id]).unwrap();
            connection
                .execute(
                    "INSERT INTO visit_derived_facts_rows
                     (visit_id, profile_id, registrable_domain, canonical_url,
                      domain_category, page_category, search_engine, search_query, is_search_event,
                      evidence_tier, taxonomy_source, computed_at)
                     VALUES (?1, 'profile-a', ?2, ?3, 'reference',
                             ?4, ?5, ?6, ?7, 'tier-a', 'test', '2026-04-14T00:00:00Z')",
                    params![
                        visit_id,
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
                    "INSERT INTO search_trail_members_rows (trail_id, profile_id, visit_id, ordinal, role)
                     VALUES ('trail-1', 'profile-a', ?1, ?1, ?2)",
                    params![visit_id, if is_search == 1 { "search" } else { "result" }],
                )
                .expect("trail member");
        }
    }

    fn seed_project(key: Option<[u8; 32]>) -> crate::Vault {
        let vault = crate::Vault::open_temp_with_key(key).unwrap().with_config(config());
        vault
            .write(vault_store::Db::Archive, vault_store::Priority::Batch, |s| {
                seed_archive(s);
                Ok(())
            })
            .unwrap();
        vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Batch, |s| {
                seed_intelligence(s);
                Ok(())
            })
            .unwrap();
        vault
    }

    #[test]
    fn session_and_trail_public_reads_return_bounded_details() {
        for key in [None, Some([30; 32])] {
            let vault = seed_project(key);
            let sessions = get_sessions(
                &vault,
                &PagedDateRangeRequest {
                    cursor: None,
                    date_range: range(),
                    profile_id: Some("profile-a".to_string()),

                    page_size: 10,
                },
            )
            .expect("sessions");
            assert_eq!(sessions.total, 1);
            assert_eq!(sessions.sessions[0].session_id, "session-1");
            assert!(sessions.sessions[0].is_deep_dive);

            let empty_page = get_sessions(
                &vault,
                &PagedDateRangeRequest {
                    cursor: None,
                    date_range: range(),
                    profile_id: Some("profile-a".to_string()),

                    page_size: 0,
                },
            )
            .expect("sessions with saturated page size");
            assert_eq!(empty_page.sessions.len(), 1);
            assert_eq!(empty_page.page_size, 1);

            let detail = get_session_detail(
                &vault,
                &crate::models::SessionDetailRequest {
                    session_id: "session-1".into(),
                    ..Default::default()
                },
            )
            .expect("session detail");
            assert_eq!(detail.visits.len(), 3);
            assert_eq!(detail.visits[0].transition_type.as_deref(), Some("1"));
            assert_eq!(detail.visits[0].search_query.as_deref(), Some("sqlite wal"));
            assert_eq!(detail.trails[0].queries, vec!["sqlite wal", "sqlite checkpoint"]);
            assert!(
                get_session_detail(
                    &vault,
                    &crate::models::SessionDetailRequest {
                        session_id: "missing".into(),
                        ..Default::default()
                    }
                )
                .is_err()
            );

            let trails = get_search_trails(
                &vault,
                &SearchTrailQueryRequest {
                    cursor: None,
                    date_range: range(),
                    profile_id: Some("profile-a".to_string()),
                    engine: Some("google".to_string()),

                    page_size: 10,
                },
            )
            .expect("trails");
            assert_eq!(trails.total, 1);
            assert_eq!(trails.trails[0].landing_domain.as_deref(), Some("sqlite.org"));

            let no_engine_match = get_search_trails(
                &vault,
                &SearchTrailQueryRequest {
                    cursor: None,
                    date_range: range(),
                    profile_id: Some("profile-a".to_string()),
                    engine: Some("duckduckgo".to_string()),

                    page_size: 10,
                },
            )
            .expect("filtered trails");
            assert!(no_engine_match.trails.is_empty());

            let trail = get_trail_detail(
                &vault,
                &crate::models::TrailDetailRequest {
                    trail_id: "trail-1".into(),
                    ..Default::default()
                },
            )
            .expect("trail detail");
            assert_eq!(trail.members.len(), 3);
            assert_eq!(trail.members[0].role, "search");
            assert_eq!(trail.members[1].registrable_domain.as_deref(), Some("sqlite.org"));
            assert!(
                get_trail_detail(
                    &vault,
                    &crate::models::TrailDetailRequest {
                        trail_id: "missing".into(),
                        ..Default::default()
                    }
                )
                .is_err()
            );
        }
    }
}
