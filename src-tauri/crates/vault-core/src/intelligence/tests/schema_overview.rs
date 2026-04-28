//! Schema, overview, query, and route-facing Core Intelligence regressions.
//!
//! ## Responsibilities
//! - Protect date grouping, KPI math, query classification, and schema migration
//!   invariants.
//! - Verify primary/secondary overview connection reuse and runtime snapshot
//!   reuse.
//! - Cover day/domain/query surfaces that front-end routes depend on.
//!
//! ## Not responsible for
//! - Exercising rebuild fallback equivalence or structural tail streaming.
//! - Testing remote backup or site-dictionary rule persistence directly.
//!
//! ## Dependencies
//! - `fixtures` supplies canonical archive rows and keyword-noise rows.
//! - Core Intelligence read APIs provide route-facing DTO coverage.
//!
//! ## Performance notes
//! Overview tests assert single intelligence connection / runtime snapshot reuse
//! so route reads do not regress into repeated database opens on large archives.

use super::super::{
    SearchQueryKind, VisitRecord, explain_refind, get_browsing_rhythm, get_day_insights,
    get_discovery_trend, get_domain_deep_dive, get_hub_pages, get_intelligence_primary_overview,
    get_intelligence_secondary_overview, get_navigation_path, get_on_this_day,
    get_query_family_detail, get_refind_page_detail, get_refind_pages, get_search_queries,
    get_top_search_concepts, get_top_sites,
    intelligence_domain::{build_domain_flows, path_from_url},
    intelligence_rebuild::run_core_intelligence,
    intelligence_schema::{
        count_core_intelligence_job_triggers, ensure_core_intelligence_schema,
        sum_table_row_counts, table_row_count,
    },
    intelligence_shared::{
        build_kpi, classify_search_query_kind, collapse_date_key, jaccard, local_date_key,
        local_datetime_from_millis, previous_date_range, query_token_set, tokenize_query_terms,
    },
    intelligence_structural_build::load_profile_search_events,
    site_dictionary::normalize_query,
};
use super::fixtures::{
    append_fixture_visit, has_index, seed_core_intelligence_fixture,
    seed_search_keyword_noise_fixture,
};
use crate::{
    archive::{
        open_archive_connection, open_intelligence_connection,
        open_intelligence_connection_call_count, open_intelligence_connection_call_sites,
        reset_open_intelligence_connection_call_count,
    },
    config::project_paths_with_root,
    intelligence_runtime::{
        load_intelligence_runtime_from_connection_call_count,
        reset_load_intelligence_runtime_from_connection_call_count,
    },
    models::{
        AppConfig, ArchiveMode, CategoryFilteredDateRangeRequest, CoreIntelligenceRebuildRequest,
        DateRange, DayInsightsRequest, DomainDeepDiveRequest, ExplainRefindRequest,
        GranularityDateRangeRequest, QueryFamilyDetailRequest, RefindPageDetailRequest,
        RefindPagesRequest, ScopedDateRangeRequest, SearchQueryListRequest,
        TopSearchConceptsRequest, TopSitesRequest,
    },
};
use chrono::{Datelike, Local, TimeZone};
use rusqlite::{Connection, params};
use std::collections::HashSet;

fn domain_visit_record(
    visit_id: i64,
    registrable_domain: &str,
    session_id: Option<&str>,
) -> VisitRecord {
    VisitRecord {
        visit_id,
        profile_id: "chrome:Default".to_string(),
        source_profile_id: 1,
        source_visit_id: visit_id,
        source_url_id: visit_id + 100,
        url: format!("https://{registrable_domain}/page-{visit_id}"),
        title: Some(format!("Page {visit_id}")),
        visit_time_ms: 1711929600000 + visit_id,
        from_visit: Some(visit_id - 1),
        transition_type: Some(1),
        external_referrer_url: None,
        canonical_url: format!("https://{registrable_domain}/page-{visit_id}"),
        registrable_domain: registrable_domain.to_string(),
        domain_category: "reference".to_string(),
        page_category: "article".to_string(),
        search_engine: None,
        search_query: None,
        is_new_domain: false,
        is_search_event: false,
        evidence_tier: "deterministic".to_string(),
        taxonomy_source: "rules".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        display_name: Some(registrable_domain.to_string()),
        session_id: session_id.map(str::to_string),
        trail_id: None,
    }
}

fn has_table(connection: &Connection, table_name: &str) -> bool {
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM sqlite_master
             WHERE type = 'table'
               AND name = ?1",
            [table_name],
            |row| row.get::<_, i64>(0),
        )
        .expect("table lookup")
        > 0
}

/// Regression coverage for collapse date key supports week and month.
#[test]
fn collapse_date_key_supports_week_and_month() {
    assert_eq!(collapse_date_key("2026-04-14", "month"), "2026-04");
    assert!(collapse_date_key("2026-04-14", "week").starts_with("2026-W"));
    assert_eq!(collapse_date_key("not-a-date", "week"), "not-a-date");
}

/// Regression coverage for build kpi reports flat when values match.
#[test]
fn build_kpi_reports_flat_when_values_match() {
    let metric = build_kpi(10, 10);
    assert_eq!(metric.trend, "flat");
    assert_eq!(metric.change_percent, Some(0.0));
    let growing = build_kpi(15, 10);
    assert_eq!(growing.trend, "up");
    assert_eq!(growing.change_percent, Some(50.0));
    let shrinking = build_kpi(5, 10);
    assert_eq!(shrinking.trend, "down");
    assert_eq!(shrinking.change_percent, Some(-50.0));
    let new_metric = build_kpi(5, 0);
    assert_eq!(new_metric.trend, "up");
    assert_eq!(new_metric.change_percent, None);
}

#[test]
fn shared_query_and_date_helpers_cover_boundary_inputs() {
    assert_eq!(
        tokenize_query_terms("https://www.example.com 路 径 sqlite"),
        vec!["example", "路", "径", "sqlite"]
    );
    let tokens = query_token_set("PathKeep sqlite sqlite");
    assert!(tokens.contains("pathkeep"));
    assert!(tokens.contains("sqlite"));
    assert_eq!(jaccard(&HashSet::new(), &tokens), 0.0);
    assert!((jaccard(&tokens, &tokens) - 1.0).abs() < f32::EPSILON);

    let previous = previous_date_range(&DateRange {
        start: "2026-04-10".to_string(),
        end: "2026-04-12".to_string(),
    })
    .expect("previous date range");
    assert_eq!(previous.start, "2026-04-07");
    assert_eq!(previous.end, "2026-04-09");
    let fallback_now = local_datetime_from_millis(i64::MAX);
    assert!(fallback_now.timestamp_millis() > 0);
}

/// Regression coverage for normalize query trims and lowercases.
#[test]
fn normalize_query_trims_and_lowercases() {
    assert_eq!(normalize_query("  WAL   Checkpoint "), "wal checkpoint");
}

/// Regression coverage for ensure core intelligence schema records versioned migrations.
#[test]
fn ensure_core_intelligence_schema_records_versioned_migrations() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    connection
        .execute("CREATE TABLE insight_cards (id INTEGER PRIMARY KEY)", [])
        .expect("legacy insight table");
    ensure_core_intelligence_schema(&connection).expect("ensure intelligence schema");
    ensure_core_intelligence_schema(&connection).expect("ensure intelligence schema twice");

    let migration_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM intelligence_schema_migrations", [], |row| row.get(0))
        .expect("migration count");
    assert_eq!(migration_count, 6);
    assert!(has_index(&connection, "idx_vdf_profile_visit_id"));
    assert!(has_index(&connection, "idx_search_trails_profile_time_trail"));
    assert!(has_index(&connection, "idx_search_events_profile_visit"));
    assert!(has_index(&connection, "idx_search_events_profile_kind"));
    assert!(!has_table(&connection, "insight_cards"));
}

/// Regression coverage for legacy search event tables receiving query-kind metadata.
#[test]
fn ensure_core_intelligence_schema_migrates_legacy_search_query_kind() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");
    connection
        .execute_batch(
            "
            CREATE TABLE search_trails (
              trail_id TEXT PRIMARY KEY,
              profile_id TEXT NOT NULL,
              session_id TEXT,
              initial_query TEXT NOT NULL,
              search_engine TEXT NOT NULL,
              reformulation_count INTEGER NOT NULL DEFAULT 0,
              visit_count INTEGER NOT NULL,
              landing_url TEXT,
              landing_domain TEXT,
              first_visit_ms INTEGER NOT NULL,
              last_visit_ms INTEGER NOT NULL,
              max_depth INTEGER NOT NULL DEFAULT 0,
              queries_json TEXT NOT NULL,
              computed_at TEXT NOT NULL
            );
            CREATE TABLE search_events (
              visit_id INTEGER PRIMARY KEY,
              profile_id TEXT NOT NULL,
              search_engine TEXT NOT NULL,
              raw_query TEXT NOT NULL,
              normalized_query TEXT NOT NULL,
              trail_id TEXT,
              computed_at TEXT NOT NULL
            );
            ",
        )
        .expect("legacy schema");
    connection
        .execute(
            "INSERT INTO search_trails
             (trail_id, profile_id, initial_query, search_engine, visit_count, landing_domain,
              first_visit_ms, last_visit_ms, queries_json, computed_at)
             VALUES ('trail-keyword', 'chrome:Default', 'pathkeep sqlite', 'google', 1,
                     'github.com', 1, 1, '[]', 'now')",
            [],
        )
        .expect("keyword trail");
    connection
        .execute(
            "INSERT INTO search_trails
             (trail_id, profile_id, initial_query, search_engine, visit_count, landing_domain,
              first_visit_ms, last_visit_ms, queries_json, computed_at)
             VALUES ('trail-nav', 'chrome:Default', 'asu.edu', 'google', 1,
                     'asu.edu', 1, 1, '[]', 'now')",
            [],
        )
        .expect("nav trail");
    for (visit_id, raw_query, trail_id) in
        [(1_i64, "pathkeep sqlite", "trail-keyword"), (2_i64, "asu.edu", "trail-nav")]
    {
        connection
            .execute(
                "INSERT INTO search_events
                 (visit_id, profile_id, search_engine, raw_query, normalized_query, trail_id, computed_at)
                 VALUES (?1, 'chrome:Default', 'google', ?2, ?2, ?3, 'now')",
                params![visit_id, raw_query, trail_id],
            )
            .expect("legacy event");
    }

    ensure_core_intelligence_schema(&connection).expect("migrate legacy schema");

    assert!(has_index(&connection, "idx_search_events_profile_kind"));
    let query_kinds = connection
        .prepare("SELECT visit_id, query_kind FROM search_events ORDER BY visit_id")
        .expect("query kind statement")
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .expect("query kinds")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("query kind rows");
    assert_eq!(query_kinds, vec![(1, "keyword".to_string()), (2, "navigational".to_string())]);
}

/// Regression coverage for schema row-count helpers on fresh or partial stores.
#[test]
fn schema_count_helpers_tolerate_missing_tables() {
    let connection = Connection::open_in_memory().expect("in memory sqlite");

    assert_eq!(table_row_count(&connection, "missing_table").expect("missing count"), 0);
    assert_eq!(
        sum_table_row_counts(&connection, &["missing_a", "missing_b"]).expect("missing sum"),
        0
    );
    assert_eq!(count_core_intelligence_job_triggers(&connection).expect("missing triggers"), 0);
}

/// Regression coverage for classifies url like search queries as navigational noise.
#[test]
fn classifies_url_like_search_queries_as_navigational_noise() {
    assert_eq!(
        classify_search_query_kind("https://asu.edu", "https://asu.edu", Some("asu.edu")),
        SearchQueryKind::Navigational
    );
    assert_eq!(
        classify_search_query_kind("asu.edu", "asu.edu", Some("asu.edu")),
        SearchQueryKind::Navigational
    );
    assert_eq!(
        classify_search_query_kind("docs.asu.edu", "docs.asu.edu", Some("asu.edu")),
        SearchQueryKind::Navigational
    );
    assert_eq!(
        classify_search_query_kind("asu.edu", "asu.edu", Some("docs.asu.edu")),
        SearchQueryKind::Navigational
    );
    assert_eq!(
        classify_search_query_kind("docs.asu.edu", "docs.asu.edu", Some("docs.asu.edu.cn")),
        SearchQueryKind::Keyword
    );
    assert_eq!(
        classify_search_query_kind("pathkeep sqlite", "pathkeep sqlite", Some("github.com")),
        SearchQueryKind::Keyword
    );
    assert_eq!(
        classify_search_query_kind("", "", Some("github.com")),
        SearchQueryKind::Navigational
    );
}

/// Regression coverage for primary overview reuses single connection and runtime snapshot.
#[test]
fn primary_overview_reuses_single_connection_and_runtime_snapshot() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    reset_open_intelligence_connection_call_count();
    reset_load_intelligence_runtime_from_connection_call_count();

    let overview = get_intelligence_primary_overview(
        &paths,
        &config,
        None,
        &ScopedDateRangeRequest {
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("primary overview");

    assert_eq!(
        open_intelligence_connection_call_count(),
        1,
        "{:?}",
        open_intelligence_connection_call_sites()
    );
    assert_eq!(load_intelligence_runtime_from_connection_call_count(), 1);
    assert_eq!(overview.timings.len(), 11);
    assert_eq!(overview.digest_summary.meta.section_id, "digest-summary");
    assert_eq!(overview.search_engine_ranking.meta.section_id, "search-activity");
    assert!(overview.total_duration_ms >= overview.timings[0].duration_ms);
}

/// Empty archives should keep the primary overview flat instead of implying work exists.
#[test]
fn primary_overview_marks_empty_digest_as_flat() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    open_archive_connection(&paths, &config, None).expect("archive");

    let overview = get_intelligence_primary_overview(
        &paths,
        &config,
        None,
        &ScopedDateRangeRequest {
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("empty primary overview");

    assert_eq!(overview.digest_summary.data.total_visits.value, 0);
    assert_eq!(overview.digest_summary.data.total_visits.trend, "flat");
}

/// Regression coverage for secondary overview reuses single connection and runtime snapshot.
#[test]
fn secondary_overview_reuses_single_connection_and_runtime_snapshot() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    reset_open_intelligence_connection_call_count();
    reset_load_intelligence_runtime_from_connection_call_count();

    let overview = get_intelligence_secondary_overview(
        &paths,
        &config,
        None,
        &ScopedDateRangeRequest {
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("secondary overview");

    assert_eq!(
        open_intelligence_connection_call_count(),
        1,
        "{:?}",
        open_intelligence_connection_call_sites()
    );
    assert_eq!(load_intelligence_runtime_from_connection_call_count(), 1);
    assert_eq!(overview.timings.len(), 10);
    assert_eq!(overview.stable_sources.meta.section_id, "stable-sources");
    assert_eq!(overview.path_flows.meta.section_id, "path-flows");
    assert!(overview.total_duration_ms >= overview.timings[0].duration_ms);
}

/// Regression coverage for discovery trend reports available years and respects profile scope.
#[test]
fn discovery_trend_reports_available_years_and_respects_profile_scope() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
    ensure_core_intelligence_schema(&intelligence).expect("ensure intelligence schema");

    for (date_key, profile_id, total_visits, new_domains) in [
        ("2024-04-18", "chrome:Default", 3_i64, 1_i64),
        ("2025-04-18", "chrome:Default", 8_i64, 2_i64),
        ("2026-04-18", "firefox:Default", 5_i64, 1_i64),
    ] {
        intelligence
            .execute(
                "INSERT INTO daily_summary_rollups
                 (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
                 VALUES (?1, ?2, ?3, 0, ?4, ?4, 0.0, 0.0)",
                params![date_key, profile_id, total_visits, new_domains],
            )
            .expect("insert daily summary rollup");
    }
    drop(intelligence);

    let archive_wide = get_discovery_trend(
        &paths,
        &config,
        None,
        &GranularityDateRangeRequest {
            date_range: DateRange {
                start: "2025-01-01".to_string(),
                end: "2025-12-31".to_string(),
            },
            profile_id: None,
            granularity: "day".to_string(),
        },
    )
    .expect("archive-wide discovery trend");
    assert_eq!(archive_wide.available_years, vec![2026, 2025, 2024]);
    assert_eq!(archive_wide.points.len(), 1);
    assert_eq!(archive_wide.points[0].date_key, "2025-04-18");

    let scoped = get_discovery_trend(
        &paths,
        &config,
        None,
        &GranularityDateRangeRequest {
            date_range: DateRange {
                start: "2025-01-01".to_string(),
                end: "2025-12-31".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            granularity: "day".to_string(),
        },
    )
    .expect("scoped discovery trend");
    assert_eq!(scoped.available_years, vec![2025, 2024]);
    assert_eq!(scoped.points.len(), 1);
    assert_eq!(scoped.points[0].date_key, "2025-04-18");
}

/// Regression coverage for day insights compose exact day entities and drilldown metadata.
#[test]
fn day_insights_compose_exact_day_entities_and_drilldown_metadata() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");

    let first_day = local_date_key(1711929600000);
    let insights = get_day_insights(
        &paths,
        &config,
        None,
        &DayInsightsRequest {
            date: first_day.clone(),
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("day insights");

    assert_eq!(insights.date, first_day);
    assert_eq!(insights.digest_summary.total_visits.value, 2);
    assert_eq!(insights.drilldown.explorer_date_range.start, insights.date);
    assert_eq!(insights.drilldown.explorer_date_range.end, insights.date);
    assert_eq!(insights.hourly_activity.len(), 24);
    assert!(insights.top_sites.iter().any(|site| site.registrable_domain == "github.com"));
    assert!(!insights.query_families.families.is_empty());
    assert!(!insights.refind_pages.is_empty());
}

/// Regression coverage for domain deep dive keeps day scoped trend consistent with exact day insights.
#[test]
fn domain_deep_dive_keeps_day_scoped_trend_consistent_with_exact_day_insights() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");

    let first_day = local_date_key(1711929600000);
    let day = get_day_insights(
        &paths,
        &config,
        None,
        &DayInsightsRequest {
            date: first_day.clone(),
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("day insights");
    let domain = get_domain_deep_dive(
        &paths,
        &config,
        None,
        &DomainDeepDiveRequest {
            registrable_domain: "github.com".to_string(),
            date_range: DateRange { start: first_day.clone(), end: first_day.clone() },
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("domain deep dive");

    assert_eq!(domain.registrable_domain, "github.com");
    assert_eq!(domain.total_visits, 1);
    assert_eq!(domain.active_days, 1);
    assert_eq!(domain.visit_trend.len(), 1);
    assert_eq!(domain.visit_trend[0].date_key, first_day);
    assert!(day.top_sites.iter().any(|site| site.registrable_domain == domain.registrable_domain));
}

/// Regression coverage for domain helpers and calendar/rhythm surfaces that need
/// richer route fixtures than the tiny baseline archive.
#[test]
fn domain_calendar_rhythm_and_flow_helpers_cover_multi_visit_edges() {
    assert_eq!(path_from_url("https://example.com/a/b?c=1"), "/a/b?c=1");
    assert_eq!(path_from_url("not-a-url"), "/");

    let (referrers, exits) = build_domain_flows(&[
        domain_visit_record(10, "search.example", Some("session-a")),
        domain_visit_record(11, "github.com", Some("session-a")),
        domain_visit_record(12, "docs.rs", Some("session-a")),
        domain_visit_record(13, "calendar.example", Some("session-b")),
        domain_visit_record(14, "github.com", Some("session-b")),
    ]);
    assert_eq!(
        referrers.iter().map(|stat| stat.domain.as_str()).collect::<Vec<_>>(),
        vec!["calendar.example", "github.com", "search.example",]
    );
    assert_eq!(
        exits.iter().map(|stat| stat.domain.as_str()).collect::<Vec<_>>(),
        vec!["github.com", "docs.rs"]
    );

    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    append_fixture_visit(
        &archive,
        91,
        "https://github.com/example/repo/issues/43",
        "Issue 43",
        1711929720000,
        Some(2),
        None,
    );
    append_fixture_visit(
        &archive,
        92,
        "https://github.com/example/repo/pulls/44",
        "Pull Request 44",
        1711929780000,
        Some(91),
        None,
    );
    let today = Local::now();
    let anniversary_ms = Local
        .with_ymd_and_hms(today.year() - 1, today.month(), today.day(), 12, 0, 0)
        .single()
        .expect("anniversary local timestamp")
        .timestamp_millis();
    append_fixture_visit(
        &archive,
        93,
        "https://anniversary.example/path",
        "Anniversary",
        anniversary_ms,
        None,
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");

    let date_range =
        DateRange { start: local_date_key(1711929600000), end: "2024-04-30".to_string() };
    let domain = get_domain_deep_dive(
        &paths,
        &config,
        None,
        &DomainDeepDiveRequest {
            registrable_domain: "github.com".to_string(),
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("github domain deep dive");
    assert_eq!(domain.total_visits, 4);
    assert!(domain.top_pages.iter().any(|page| page.path == "/example/repo/pulls/44"));
    assert_eq!(domain.arrival_breakdown.link, 3);
    assert_eq!(domain.visit_trend.iter().map(|point| point.visit_count).sum::<i64>(), 4);

    let rhythm = get_browsing_rhythm(
        &paths,
        &config,
        None,
        &CategoryFilteredDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
            category: None,
        },
    )
    .expect("browsing rhythm");
    assert!(rhythm.max_count > 0);
    assert!(rhythm.cells.iter().any(|cell| cell.visit_count == rhythm.max_count));

    let on_this_day =
        get_on_this_day(&paths, &config, None, Some("chrome:Default")).expect("on this day");
    assert!(on_this_day.iter().any(|entry| {
        entry.year == today.year() - 1
            && entry.top_domains.iter().any(|domain| domain == "anniversary.example")
    }));

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
    intelligence
        .execute(
            "INSERT OR REPLACE INTO daily_summary_rollups
             (date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate)
             VALUES ('2024-04-03', 'chrome:Default', 0, 0, 0, 0, 0.0, 0.0)",
            [],
        )
        .expect("insert zero total daily rollup");
    drop(intelligence);
    let discovery = get_discovery_trend(
        &paths,
        &config,
        None,
        &GranularityDateRangeRequest {
            date_range: DateRange {
                start: "2024-04-03".to_string(),
                end: "2024-04-03".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            granularity: "day".to_string(),
        },
    )
    .expect("zero-total discovery trend");
    assert_eq!(discovery.points[0].total_visits, 0);
    assert_eq!(discovery.points[0].discovery_rate, 0.0);
}

/// Regression coverage for search queries reuse family and trail identity.
#[test]
fn search_queries_reuse_family_and_trail_identity() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    append_fixture_visit(
        &archive,
        10,
        "https://www.google.com/search?q=rust+ownership",
        "rust ownership - Google Search",
        1711929720000,
        None,
        Some("rust ownership"),
    );
    append_fixture_visit(
        &archive,
        11,
        "https://www.google.com/search?q=sqlite+wal+checkpoint",
        "sqlite wal checkpoint - Google Search",
        1711929780000,
        None,
        Some("sqlite wal checkpoint"),
    );
    append_fixture_visit(
        &archive,
        12,
        "https://www.google.com/search?q=sqlite+wal",
        "sqlite wal - Google Search",
        1711929840000,
        None,
        Some("sqlite wal"),
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");
    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    intelligence
        .execute(
            "INSERT OR REPLACE INTO query_families (
                family_id, profile_id, anchor_query, member_count, search_engine,
                first_seen_ms, last_seen_ms, queries_json, computed_at
             ) VALUES
             (
                'family:small-sqlite', 'chrome:Default', 'sqlite wal', 1, 'google',
                1711929600000, 1711929840000, ?1, '2026-04-14T00:00:00Z'
             ),
             (
                'family:large-sqlite', 'chrome:Default', 'sqlite wal tuning', 7, 'google',
                1711929600000, 1711929900000, ?1, '2026-04-14T00:00:00Z'
             )",
            params![serde_json::json!(["sqlite wal"]).to_string()],
        )
        .expect("conflicting query families");
    drop(intelligence);

    let queries = get_search_queries(
        &paths,
        &config,
        None,
        &SearchQueryListRequest {
            date_range: DateRange {
                start: local_date_key(1711929600000),
                end: local_date_key(1711929600000),
            },
            profile_id: Some("chrome:Default".to_string()),
            browser_kind: Some("chrome".to_string()),
            engine: Some("google".to_string()),
            domain: None,
            query: Some("sqlite".to_string()),
            sort: Some("family-frequency".to_string()),
            page: 0,
            page_size: 10,
        },
    )
    .expect("search queries");

    assert!(!queries.rows.is_empty());
    assert!(queries.rows.iter().any(|row| row.family_id.is_some() && row.trail_id.is_some()));
    let top_row = &queries.rows[0];
    assert!(top_row.family_count >= top_row.exact_repeat_count);
    assert_eq!(top_row.family_id.as_deref(), Some("family:large-sqlite"));
    assert_eq!(top_row.display_name.as_deref(), Some("Google"));

    let detail = get_query_family_detail(
        &paths,
        &config,
        None,
        &QueryFamilyDetailRequest {
            family_id: top_row.family_id.clone().expect("family id"),
            profile_id: Some("chrome:Default".to_string()),
            date_range: DateRange {
                start: local_date_key(1711929600000),
                end: local_date_key(1711929600000),
            },
        },
    )
    .expect("query family detail");
    assert_eq!(detail.family.family_id, "family:large-sqlite");
    assert!(detail.related_trails.iter().any(|trail| {
        trail.queries.iter().any(|query| normalize_query(query).contains("sqlite"))
            || normalize_query(&trail.initial_query).contains("sqlite")
    }));

    for sort in ["exact-frequency", "alphabetical"] {
        let sorted = get_search_queries(
            &paths,
            &config,
            None,
            &SearchQueryListRequest {
                date_range: DateRange {
                    start: local_date_key(1711929600000),
                    end: local_date_key(1711929600000),
                },
                profile_id: Some("chrome:Default".to_string()),
                browser_kind: Some("chrome".to_string()),
                engine: Some("google".to_string()),
                domain: None,
                query: Some("sqlite".to_string()),
                sort: Some(sort.to_string()),
                page: 0,
                page_size: 2,
            },
        )
        .expect("alternate search sort");
        assert!(!sorted.rows.is_empty(), "{sort}");
    }

    let empty_queries = get_search_queries(
        &paths,
        &config,
        None,
        &SearchQueryListRequest {
            date_range: DateRange {
                start: "2030-01-01".to_string(),
                end: "2030-01-02".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            browser_kind: Some("chrome".to_string()),
            engine: Some("google".to_string()),
            domain: None,
            query: Some("sqlite".to_string()),
            sort: Some("family-frequency".to_string()),
            page: 0,
            page_size: 10,
        },
    )
    .expect("empty search queries");
    assert!(empty_queries.rows.is_empty());
}

/// Regression coverage for keyword surfaces filter navigational noise and support domain reads.
#[test]
fn keyword_surfaces_filter_navigational_noise_and_support_domain_reads() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    seed_search_keyword_noise_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
    let search_events =
        load_profile_search_events(&intelligence, "chrome:Default").expect("search events");
    let navigation_event = search_events
        .iter()
        .find(|event| event.raw_query == "https://asu.edu")
        .expect("navigational event");
    assert_eq!(navigation_event.query_kind, SearchQueryKind::Navigational);
    drop(intelligence);
    let fixture_day = local_date_key(1711929600000);

    let google_queries = get_search_queries(
        &paths,
        &config,
        None,
        &SearchQueryListRequest {
            date_range: DateRange { start: fixture_day.clone(), end: "2024-04-30".to_string() },
            profile_id: Some("chrome:Default".to_string()),
            browser_kind: Some("chrome".to_string()),
            engine: None,
            domain: Some("google.com".to_string()),
            query: None,
            sort: Some("newest".to_string()),
            page: 0,
            page_size: 20,
        },
    )
    .expect("google queries");
    assert_eq!(google_queries.total, 1);
    assert_eq!(google_queries.rows[0].raw_query, "sqlite wal");

    let github_queries = get_search_queries(
        &paths,
        &config,
        None,
        &SearchQueryListRequest {
            date_range: DateRange { start: fixture_day.clone(), end: "2024-04-30".to_string() },
            profile_id: Some("chrome:Default".to_string()),
            browser_kind: Some("chrome".to_string()),
            engine: None,
            domain: Some("github.com".to_string()),
            query: None,
            sort: Some("newest".to_string()),
            page: 0,
            page_size: 20,
        },
    )
    .expect("github queries");
    assert_eq!(github_queries.total, 1);
    assert_eq!(github_queries.rows[0].raw_query, "pathkeep sqlite");

    let concepts = get_top_search_concepts(
        &paths,
        &config,
        None,
        &TopSearchConceptsRequest {
            date_range: DateRange { start: fixture_day, end: "2024-04-30".to_string() },
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(20),
        },
    )
    .expect("top concepts");
    let terms = concepts.into_iter().map(|concept| concept.term).collect::<HashSet<_>>();
    assert!(terms.contains("sqlite"));
    assert!(terms.contains("pathkeep"));
    assert!(!terms.contains("https"));
    assert!(!terms.contains("asu"));
    assert!(!terms.contains("edu"));
}

/// Regression coverage for refind, top-site, and navigation read models sharing archive truth.
#[test]
fn refind_navigation_and_hub_reads_reuse_persisted_structural_context() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");

    let date_range = DateRange { start: "2024-04-01".to_string(), end: "2024-04-30".to_string() };
    let top_by_visits = get_top_sites(
        &paths,
        &config,
        None,
        &TopSitesRequest {
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
            sort_by: None,
            limit: Some(10),
        },
    )
    .expect("top sites default sort");
    assert_eq!(top_by_visits[0].registrable_domain, "github.com");

    let top_by_days = get_top_sites(
        &paths,
        &config,
        None,
        &TopSitesRequest {
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
            sort_by: Some("unique_days".to_string()),
            limit: Some(10),
        },
    )
    .expect("top sites days sort");
    assert!(top_by_days[0].unique_days >= top_by_visits[0].unique_days);

    let top_by_average = get_top_sites(
        &paths,
        &config,
        None,
        &TopSitesRequest {
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
            sort_by: Some("average_daily_visits".to_string()),
            limit: Some(10),
        },
    )
    .expect("top sites average sort");
    assert!(top_by_average[0].average_daily_visits >= 1.0);

    let refind_pages = get_refind_pages(
        &paths,
        &config,
        None,
        &RefindPagesRequest {
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(10),
        },
    )
    .expect("refind pages");
    let refind_page = refind_pages
        .iter()
        .find(|page| page.registrable_domain == "github.com")
        .expect("github refind page");
    assert!(refind_page.cross_day_count >= 2);

    let explanation = explain_refind(
        &paths,
        &config,
        None,
        &ExplainRefindRequest { canonical_url: refind_page.canonical_url.clone() },
    )
    .expect("refind explanation");
    assert!(!explanation.factors.is_empty());
    assert!(explanation.visit_ids.len() >= 2);

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
    for (ordinal, visit_id) in explanation.visit_ids.iter().enumerate() {
        intelligence
            .execute(
                "UPDATE visit_derived_facts
                 SET trail_id = 'trail-refind-detail'
                 WHERE visit_id = ?1",
                [visit_id],
            )
            .expect("link refind evidence to trail");
        intelligence
            .execute(
                "INSERT OR REPLACE INTO search_trail_members
                 (trail_id, profile_id, visit_id, ordinal, role)
                 VALUES ('trail-refind-detail', 'chrome:Default', ?1, ?2, 'landing')",
                params![visit_id, ordinal as i64],
            )
            .expect("link refind evidence as trail member");
    }
    intelligence
        .execute(
            "INSERT OR REPLACE INTO search_trails (
                trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count,
                visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth,
                queries_json, computed_at
             ) VALUES (
                'trail-refind-detail', 'chrome:Default', 'session-refind-detail', 'sqlite wal',
                'google', 1, 2, ?1, 'github.com', 1711929600000, 1712016000000, 1,
                '[\"sqlite wal\"]', '2026-04-14T00:00:00Z'
             )",
            [refind_page.url.as_str()],
        )
        .expect("seed related trail");
    drop(intelligence);

    let detail = get_refind_page_detail(
        &paths,
        &config,
        None,
        &RefindPageDetailRequest {
            canonical_url: refind_page.canonical_url.clone(),
            date_range: date_range.clone(),
            profile_id: Some("chrome:Default".to_string()),
        },
    )
    .expect("refind detail");
    assert_eq!(detail.page.canonical_url, refind_page.canonical_url);
    assert_eq!(detail.explanation.visit_ids, explanation.visit_ids);
    assert!(!detail.recent_days.is_empty());
    assert!(detail.recent_days.iter().all(|day| day.len() == 10 && day.contains('-')));
    assert!(!detail.related_trails.is_empty());

    let path = get_navigation_path(&paths, &config, None, 2).expect("navigation path");
    assert_eq!(path.target_visit_id, 2);
    assert_eq!(path.steps.iter().map(|step| step.visit_id).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(path.steps[0].depth, 0);
    assert_eq!(path.steps[1].depth, 1);

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    archive.execute("UPDATE visits SET from_visit = 999 WHERE id = 2", []).expect("missing parent");
    drop(archive);
    let missing_parent_path =
        get_navigation_path(&paths, &config, None, 2).expect("missing parent path");
    assert_eq!(
        missing_parent_path.steps.iter().map(|step| step.visit_id).collect::<Vec<_>>(),
        vec![2]
    );

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    archive.execute("UPDATE visits SET from_visit = 2 WHERE id = 1", []).expect("cycle parent");
    archive.execute("UPDATE visits SET from_visit = 1 WHERE id = 2", []).expect("cycle target");
    drop(archive);
    let cyclic_path = get_navigation_path(&paths, &config, None, 2).expect("cyclic path");
    assert_eq!(cyclic_path.steps.iter().map(|step| step.visit_id).collect::<Vec<_>>(), vec![1, 2]);

    let hub_pages = get_hub_pages(
        &paths,
        &config,
        None,
        &TopSitesRequest {
            date_range,
            profile_id: Some("chrome:Default".to_string()),
            sort_by: None,
            limit: Some(5),
        },
    )
    .expect("hub pages");
    assert!(hub_pages.iter().any(|page| page.registrable_domain == "github.com"));
}
