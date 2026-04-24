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
    SearchQueryKind, get_day_insights, get_discovery_trend, get_domain_deep_dive,
    get_intelligence_primary_overview, get_intelligence_secondary_overview, get_search_queries,
    get_top_search_concepts,
    intelligence_rebuild::run_core_intelligence,
    intelligence_schema::ensure_core_intelligence_schema,
    intelligence_shared::{
        build_kpi, classify_search_query_kind, collapse_date_key, local_date_key,
    },
    intelligence_structural_build::load_profile_search_events,
    site_dictionary::normalize_query,
};
use super::fixtures::{
    has_index, seed_core_intelligence_fixture, seed_search_keyword_noise_fixture,
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
        AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, DayInsightsRequest,
        DomainDeepDiveRequest, GranularityDateRangeRequest, ScopedDateRangeRequest,
        SearchQueryListRequest, TopSearchConceptsRequest,
    },
};
use rusqlite::{Connection, params};
use std::collections::HashSet;

/// Regression coverage for collapse date key supports week and month.
#[test]
fn collapse_date_key_supports_week_and_month() {
    assert_eq!(collapse_date_key("2026-04-14", "month"), "2026-04");
    assert!(collapse_date_key("2026-04-14", "week").starts_with("2026-W"));
}

/// Regression coverage for build kpi reports flat when values match.
#[test]
fn build_kpi_reports_flat_when_values_match() {
    let metric = build_kpi(10, 10);
    assert_eq!(metric.trend, "flat");
    assert_eq!(metric.change_percent, Some(0.0));
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
        classify_search_query_kind("pathkeep sqlite", "pathkeep sqlite", Some("github.com")),
        SearchQueryKind::Keyword
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
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("rebuild intelligence");

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
    assert_eq!(top_row.display_name.as_deref(), Some("Google"));
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
