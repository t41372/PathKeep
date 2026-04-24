//! Batch equivalence regressions for Core Intelligence rebuild helpers.
//!
//! ## Responsibilities
//! - Compare batched builders against their in-memory reference builders.
//! - Verify fallback-full rebuilds match clean full rebuilds across batch
//!   boundaries.
//! - Keep query-family, structural aggregate, and source-effectiveness behavior
//!   stable while the implementation stays streaming-friendly.
//!
//! ## Not responsible for
//! - Route-facing overview or explanation DTO coverage.
//! - Remote backup or AI/enrichment behavior.
//!
//! ## Dependencies
//! - `fixtures` seeds paired fallback and clean archives.
//! - Structural helper modules expose test-only builders through `pub(super)`.
//!
//! ## Performance notes
//! These tests intentionally cross configured fallback batch sizes. They compare
//! persisted rows instead of holding large expected structures in memory.

use super::super::{
    DAILY_ROLLUP_FALLBACK_BATCH_SIZE, QueryFamilyRecord, VISIT_DERIVE_FALLBACK_BATCH_SIZE,
    intelligence_daily_rollups::load_profile_derived_visits,
    intelligence_rebuild::{run_core_intelligence, run_core_intelligence_job_type_with_progress},
    intelligence_structural_aggregates::{
        build_habit_patterns, build_path_flows, build_refind_pages, build_source_effectiveness,
        build_structural_profile_aggregates_from_batches,
    },
    intelligence_structural_build::{
        build_query_families, build_query_families_from_batches, load_profile_search_events,
    },
    intelligence_structural_stage::{
        build_source_effectiveness_from_database, load_profile_trails,
    },
    site_dictionary::normalize_query,
};
use super::fixtures::{
    append_fixture_visit, append_many_fixture_visits, load_daily_rollup_rows,
    load_visit_derived_fact_rows, seed_core_intelligence_fixture,
};
use crate::{
    archive::{open_archive_connection, open_intelligence_connection},
    config::project_paths_with_root,
    models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
};

/// Regression coverage for batched query family builder matches in memory builder.
#[test]
fn batched_query_family_builder_matches_in_memory_builder() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    for visit_id in 4..=24 {
        let timestamp = 1712145600000 + ((visit_id - 4) * 60_000);
        let query = if visit_id % 2 == 0 {
            format!("sqlite wal checkpoint {}", visit_id % 3)
        } else {
            format!("tauri ipc bridge {}", visit_id % 4)
        };
        append_fixture_visit(
            &archive,
            visit_id,
            &format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
            &format!("Search {query}"),
            timestamp,
            None,
            Some(&query),
        );
    }
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let all_events =
        load_profile_search_events(&intelligence, "chrome:Default").expect("search events");
    let in_memory = build_query_families(&all_events);
    let batched = build_query_families_from_batches(&intelligence, "chrome:Default")
        .expect("batched query families");

    let to_summary = |families: Vec<QueryFamilyRecord>| {
        let mut summary = families
            .into_iter()
            .map(|family| {
                (
                    normalize_query(&family.anchor_query),
                    family.member_count,
                    family.search_engine,
                    family.queries.len(),
                )
            })
            .collect::<Vec<_>>();
        summary.sort();
        summary
    };
    assert_eq!(to_summary(batched), to_summary(in_memory));
}

/// Regression coverage for batched structural aggregates match in memory builders.
#[test]
fn batched_structural_aggregates_match_in_memory_builders() {
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
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712102400000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        5,
        "https://alpha-docs.dev/guide",
        "Guide",
        1712145600000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        6,
        "https://beta-community.dev/thread",
        "Thread",
        1712145660000,
        Some(5),
        None,
    );
    append_fixture_visit(
        &archive,
        7,
        "https://gamma-news.dev/article",
        "Article",
        1712145720000,
        Some(6),
        None,
    );
    append_fixture_visit(
        &archive,
        8,
        "https://delta-shop.dev/item",
        "Item",
        1712145780000,
        Some(7),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let visits = load_profile_derived_visits(&intelligence, "chrome:Default", None, None)
        .expect("derived visits");
    let mut in_memory_refind = build_refind_pages(&visits)
        .into_iter()
        .map(|page| (page.canonical_url, page.refind_score, page.cross_day_count))
        .collect::<Vec<_>>();
    let mut in_memory_flows = build_path_flows(&visits)
        .into_iter()
        .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
        .collect::<Vec<_>>();
    let mut in_memory_habits = build_habit_patterns(&visits)
        .into_iter()
        .map(|habit| (habit.registrable_domain, habit.habit_type, habit.visit_count))
        .collect::<Vec<_>>();

    let (batched_refind, batched_flows, batched_habits) =
        build_structural_profile_aggregates_from_batches(&intelligence, "chrome:Default")
            .expect("batched aggregates");
    let mut batched_refind = batched_refind
        .into_iter()
        .map(|page| (page.canonical_url, page.refind_score, page.cross_day_count))
        .collect::<Vec<_>>();
    let mut batched_flows = batched_flows
        .into_iter()
        .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
        .collect::<Vec<_>>();
    let mut batched_habits = batched_habits
        .into_iter()
        .map(|habit| (habit.registrable_domain, habit.habit_type, habit.visit_count))
        .collect::<Vec<_>>();

    in_memory_refind.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    in_memory_flows.sort();
    in_memory_habits.sort();
    batched_refind.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    batched_flows.sort();
    batched_habits.sort();

    assert_eq!(batched_refind, in_memory_refind);
    assert_eq!(batched_flows, in_memory_flows);
    assert_eq!(batched_habits, in_memory_habits);
}

/// Regression coverage for batched source effectiveness matches in memory builder.
#[test]
fn batched_source_effectiveness_matches_in_memory_builder() {
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
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712102400000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        5,
        "https://alpha-docs.dev/guide",
        "Guide",
        1712145600000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        6,
        "https://beta-community.dev/thread",
        "Thread",
        1712145660000,
        Some(5),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let trails = load_profile_trails(&intelligence, "chrome:Default").expect("trails");
    let (refind_pages, _, _) =
        build_structural_profile_aggregates_from_batches(&intelligence, "chrome:Default")
            .expect("aggregates");
    let mut in_memory = build_source_effectiveness(&trails, &refind_pages)
        .into_iter()
        .map(|record| {
            (
                record.registrable_domain,
                record.source_role,
                record.trail_count,
                record.stable_landing_count,
                record.effectiveness_score,
                record.first_seen_ms,
                record.last_seen_ms,
            )
        })
        .collect::<Vec<_>>();
    let mut batched =
        build_source_effectiveness_from_database(&intelligence, "chrome:Default", &refind_pages)
            .expect("batched source effectiveness")
            .into_iter()
            .map(|record| {
                (
                    record.registrable_domain,
                    record.source_role,
                    record.trail_count,
                    record.stable_landing_count,
                    record.effectiveness_score,
                    record.first_seen_ms,
                    record.last_seen_ms,
                )
            })
            .collect::<Vec<_>>();
    in_memory.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.total_cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
            .then_with(|| left.6.cmp(&right.6))
    });
    batched.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.total_cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
            .then_with(|| left.6.cmp(&right.6))
    });
    assert_eq!(batched, in_memory);
}

/// Regression coverage for visit derive fallback matches clean full rebuild across batches.
#[test]
fn visit_derive_fallback_matches_clean_full_rebuild_across_batches() {
    let fallback_root = tempfile::tempdir().expect("fallback tempdir");
    let clean_root = tempfile::tempdir().expect("clean tempdir");
    let fallback_paths = project_paths_with_root(fallback_root.path());
    let clean_paths = project_paths_with_root(clean_root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let reverted_visit_id = 1200_i64;

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    seed_core_intelligence_fixture(&fallback_archive);
    append_many_fixture_visits(
        &fallback_archive,
        4,
        VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    drop(fallback_archive);

    run_core_intelligence(
        &fallback_paths,
        &config,
        None,
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("fallback full rebuild");

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    fallback_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert fallback visit");
    drop(fallback_archive);

    let fallback_report = run_core_intelligence_job_type_with_progress(
        &fallback_paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive fallback");
    assert_eq!(fallback_report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(fallback_report.visit_derived_facts > VISIT_DERIVE_FALLBACK_BATCH_SIZE);

    let clean_archive =
        open_archive_connection(&clean_paths, &config, None).expect("clean archive");
    seed_core_intelligence_fixture(&clean_archive);
    append_many_fixture_visits(
        &clean_archive,
        4,
        VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    clean_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert clean visit");
    drop(clean_archive);

    run_core_intelligence(&clean_paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("clean full rebuild");

    let fallback_intelligence = open_intelligence_connection(&fallback_paths, &config, None)
        .expect("fallback intelligence");
    let clean_intelligence =
        open_intelligence_connection(&clean_paths, &config, None).expect("clean intelligence");
    assert_eq!(
        load_visit_derived_fact_rows(&fallback_intelligence),
        load_visit_derived_fact_rows(&clean_intelligence)
    );
}

/// Regression coverage for daily rollup fallback matches clean full rebuild across batches.
#[test]
fn daily_rollup_fallback_matches_clean_full_rebuild_across_batches() {
    let fallback_root = tempfile::tempdir().expect("fallback tempdir");
    let clean_root = tempfile::tempdir().expect("clean tempdir");
    let fallback_paths = project_paths_with_root(fallback_root.path());
    let clean_paths = project_paths_with_root(clean_root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let reverted_visit_id = 1200_i64;

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    seed_core_intelligence_fixture(&fallback_archive);
    append_many_fixture_visits(
        &fallback_archive,
        4,
        DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    drop(fallback_archive);

    run_core_intelligence(
        &fallback_paths,
        &config,
        None,
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("fallback full rebuild");

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    fallback_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert fallback visit");
    drop(fallback_archive);

    run_core_intelligence_job_type_with_progress(
        &fallback_paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive fallback");
    let fallback_report = run_core_intelligence_job_type_with_progress(
        &fallback_paths,
        &config,
        None,
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("daily rollup fallback");
    assert_eq!(fallback_report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(fallback_report.processed_visits > DAILY_ROLLUP_FALLBACK_BATCH_SIZE);

    let clean_archive =
        open_archive_connection(&clean_paths, &config, None).expect("clean archive");
    seed_core_intelligence_fixture(&clean_archive);
    append_many_fixture_visits(
        &clean_archive,
        4,
        DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    clean_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert clean visit");
    drop(clean_archive);

    run_core_intelligence(&clean_paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("clean full rebuild");

    let fallback_intelligence = open_intelligence_connection(&fallback_paths, &config, None)
        .expect("fallback intelligence");
    let clean_intelligence =
        open_intelligence_connection(&clean_paths, &config, None).expect("clean intelligence");
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "domain_daily_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "domain_daily_rollups")
    );
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "category_daily_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "category_daily_rollups")
    );
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "engine_daily_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "engine_daily_rollups")
    );
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "daily_summary_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "daily_summary_rollups")
    );
}
