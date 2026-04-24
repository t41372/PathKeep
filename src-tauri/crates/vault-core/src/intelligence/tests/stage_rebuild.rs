//! Stage rebuild, clear-state, and external-output regressions.
//!
//! ## Responsibilities
//! - Protect derived-state clearing counts and runtime cleanup.
//! - Verify stage timing aggregation and rebuild result metadata.
//! - Cover incremental visit-derived and daily-rollup stage behavior.
//! - Keep explanation and trusted-output payload surfaces attached to rebuilt
//!   Core Intelligence tables.
//!
//! ## Not responsible for
//! - Batch equivalence against clean full rebuilds.
//! - Structural tail reassignment behavior.
//!
//! ## Dependencies
//! - `fixtures` seeds canonical archive rows and appends focused visit deltas.
//! - Runtime schema helpers expose queue rows needed by clear-state coverage.
//!
//! ## Performance notes
//! Tests assert dirty-date and dirty-visit scoping so incremental stages do not
//! regress into unnecessary full-table work for ordinary updates.

use super::super::{
    StageRunResult, explain_entity, get_intelligence_embed_cards, get_intelligence_public_snapshot,
    get_intelligence_widget_snapshot,
    intelligence_rebuild::{
        merge_stage_run_result, run_core_intelligence, run_core_intelligence_job_type_with_progress,
    },
    intelligence_schema::{
        clear_derived_intelligence_state, count_core_intelligence_job_triggers,
        count_core_intelligence_jobs, sum_table_row_counts, table_row_count,
    },
    intelligence_shared::local_date_key,
};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture};
use crate::{
    archive::{open_archive_connection, open_intelligence_connection},
    config::project_paths_with_root,
    intelligence_catalog::RebuildMode,
    intelligence_runtime::{FULL_REBUILD_JOB_TYPE, ensure_intelligence_runtime_schema},
    models::{
        AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, CoreIntelligenceStageTimings,
        DateRange, EntityExplanationRequest, IntelligenceEmbedCardsRequest, ScopedDateRangeRequest,
    },
    utils::now_rfc3339,
};
use rusqlite::params;
use std::collections::BTreeMap;

/// Regression coverage for clear derived intelligence state reports canonical group counts.
#[test]
fn clear_derived_intelligence_state_reports_canonical_group_counts() {
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

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("runtime");
    ensure_intelligence_runtime_schema(&intelligence).expect("runtime schema");
    let now = now_rfc3339();
    intelligence
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, NULL, NULL, 'running', 50, 1, 'clear-test:full', '{}', '{}',
                     ?2, ?2, ?2, ?2)",
            params![FULL_REBUILD_JOB_TYPE, now],
        )
        .expect("insert runtime job");
    let job_id = intelligence.last_insert_rowid();
    intelligence
        .execute(
            "INSERT INTO intelligence_job_triggers (job_id, run_id, reason, requested_at)
             VALUES (?1, NULL, 'clear test', ?2)",
            params![job_id, now_rfc3339()],
        )
        .expect("insert runtime trigger");

    let expected_visit_derived =
        table_row_count(&intelligence, "visit_derived_facts").expect("visit facts");
    let expected_daily_rollups = sum_table_row_counts(
        &intelligence,
        &[
            "domain_daily_rollups",
            "category_daily_rollups",
            "engine_daily_rollups",
            "daily_summary_rollups",
        ],
    )
    .expect("daily rollups");
    let expected_structural = sum_table_row_counts(
        &intelligence,
        &[
            "sessions",
            "search_trails",
            "search_trail_members",
            "search_events",
            "search_event_terms",
            "query_families",
            "refind_pages",
            "source_effectiveness",
            "habit_patterns",
            "reopened_investigations",
            "path_flows",
        ],
    )
    .expect("structural");
    let expected_runtime = table_row_count(&intelligence, "deterministic_module_runtime")
        .expect("module runtime")
        + table_row_count(&intelligence, "core_intelligence_stage_checkpoints")
            .expect("stage checkpoints")
        + count_core_intelligence_jobs(&intelligence).expect("runtime jobs")
        + count_core_intelligence_job_triggers(&intelligence).expect("runtime triggers");
    drop(intelligence);

    let report =
        clear_derived_intelligence_state(&paths, &config, None).expect("clear derived state");
    assert_eq!(report.cleared_visit_derived_fact_rows, expected_visit_derived);
    assert_eq!(report.cleared_daily_rollup_rows, expected_daily_rollups);
    assert_eq!(report.cleared_structural_rows, expected_structural);
    assert_eq!(report.cleared_runtime_rows, expected_runtime);

    let intelligence =
        open_intelligence_connection(&paths, &config, None).expect("runtime after clear");
    assert_eq!(table_row_count(&intelligence, "visit_derived_facts").expect("visit facts"), 0);
    assert_eq!(
        table_row_count(&intelligence, "daily_summary_rollups").expect("daily summaries"),
        0
    );
    assert_eq!(table_row_count(&intelligence, "search_trails").expect("search trails"), 0);
    assert_eq!(
        table_row_count(&intelligence, "core_intelligence_stage_checkpoints")
            .expect("stage checkpoints"),
        0
    );
    assert_eq!(
        table_row_count(&intelligence, "deterministic_module_runtime").expect("module runtime"),
        0
    );
    assert_eq!(count_core_intelligence_jobs(&intelligence).expect("runtime jobs"), 0);
    assert_eq!(count_core_intelligence_job_triggers(&intelligence).expect("runtime triggers"), 0);
}

/// Regression coverage for merge stage run result sums stage timings across profiles.
#[test]
fn merge_stage_run_result_sums_stage_timings_across_profiles() {
    let mut aggregate = StageRunResult {
        stage_timings_ms: Some(CoreIntelligenceStageTimings {
            visit_derive_ms: 10,
            daily_rollup_ms: 20,
            structural_rebuild_ms: 30,
            total_ms: 60,
        }),
        ..StageRunResult::default()
    };
    let next = StageRunResult {
        stage_timings_ms: Some(CoreIntelligenceStageTimings {
            visit_derive_ms: 1,
            daily_rollup_ms: 2,
            structural_rebuild_ms: 3,
            total_ms: 6,
        }),
        ..StageRunResult::default()
    };

    merge_stage_run_result(&mut aggregate, next, RebuildMode::FullRebuild);

    let timings = aggregate.stage_timings_ms.expect("stage timings");
    assert_eq!(timings.visit_derive_ms, 11);
    assert_eq!(timings.daily_rollup_ms, 22);
    assert_eq!(timings.structural_rebuild_ms, 33);
    assert_eq!(timings.total_ms, 66);
}

/// Regression coverage for explain entity and provider snapshots build from core intelligence tables.
#[test]
fn explain_entity_and_provider_snapshots_build_from_core_intelligence_tables() {
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

    let rebuild =
        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("run core intelligence");
    assert!(rebuild.processed_visits >= 3);
    let stage_timings = rebuild.stage_timings_ms.as_ref().expect("full rebuild stage timings");
    assert!(stage_timings.total_ms >= stage_timings.visit_derive_ms);
    assert!(stage_timings.total_ms >= stage_timings.daily_rollup_ms);
    assert!(stage_timings.total_ms >= stage_timings.structural_rebuild_ms);

    let session_explanation = explain_entity(
        &paths,
        &config,
        None,
        &EntityExplanationRequest {
            entity_type: "session".to_string(),
            entity_id: "session:chrome:Default:1".to_string(),
        },
    )
    .expect("session explanation");
    assert_eq!(session_explanation.entity_type, "session");
    assert!(!session_explanation.participating_visit_ids.is_empty());

    let refind_explanation = explain_entity(
        &paths,
        &config,
        None,
        &EntityExplanationRequest {
            entity_type: "refind_page".to_string(),
            entity_id: "https://github.com/example/repo/issues/42".to_string(),
        },
    )
    .expect("refind explanation");
    assert_eq!(refind_explanation.entity_type, "refind_page");
    assert!(refind_explanation.factors.iter().any(|factor| factor.label == "cross_day_count"));

    let embed_cards = get_intelligence_embed_cards(
        &paths,
        &config,
        None,
        &IntelligenceEmbedCardsRequest {
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(4),
        },
    )
    .expect("embed cards");
    assert!(!embed_cards.is_empty());
    assert!(embed_cards.iter().any(|card| card.card_type == "digest"));

    let public_snapshot = get_intelligence_public_snapshot(
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
    .expect("public snapshot");
    assert!(!public_snapshot.top_domains.is_empty());
    assert!(public_snapshot.notes.iter().any(|note| note.contains("omit visit-level identifiers")));
    let public_snapshot_json =
        serde_json::to_string(&public_snapshot).expect("serialize public snapshot");
    assert!(!public_snapshot_json.contains("https://"));
    assert!(!public_snapshot_json.contains("visitId"));

    let widget_snapshot = get_intelligence_widget_snapshot(
        &paths,
        &config,
        None,
        &IntelligenceEmbedCardsRequest {
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(8),
        },
    )
    .expect("widget snapshot");
    assert!(widget_snapshot.highlights.len() <= 4);
    assert!(widget_snapshot.notes.iter().any(|note| note.contains("internal_only")));

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let migration_count: i64 = intelligence
        .query_row("SELECT COUNT(*) FROM intelligence_schema_migrations", [], |row| row.get(0))
        .expect("migration count");
    assert_eq!(migration_count, 6);
}

/// Regression coverage for visit derive stage processes only new visible visits.
#[test]
fn visit_derive_stage_processes_only_new_visible_visits() {
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

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    append_fixture_visit(
        &archive,
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712102400000,
        None,
        None,
    );
    drop(archive);

    let report = run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
    assert_eq!(report.dirty_visit_count, Some(1));
    assert_eq!(report.visit_derived_facts, 1);

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let checkpoint = intelligence
        .query_row(
            "SELECT last_processed_visit_id
             FROM core_intelligence_stage_checkpoints
             WHERE profile_id = 'chrome:Default' AND stage = 'visit-derive'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("stage checkpoint");
    assert_eq!(checkpoint, 4);
}

/// Regression coverage for daily rollup stage recomputes only dirty days.
#[test]
fn daily_rollup_stage_recomputes_only_dirty_days() {
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

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    append_fixture_visit(
        &archive,
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712059200000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        5,
        "https://example.com/deep-dive",
        "Deep Dive",
        1712145600000,
        None,
        None,
    );
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    let report = run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("daily rollup stage");
    let expected_dirty_dates = vec![local_date_key(1712059200000), local_date_key(1712145600000)];
    let expected_totals = [
        1711929600000_i64,
        1711929660000_i64,
        1712016000000_i64,
        1712059200000_i64,
        1712145600000_i64,
    ]
    .into_iter()
    .fold(BTreeMap::<String, i64>::new(), |mut acc, timestamp| {
        *acc.entry(local_date_key(timestamp)).or_default() += 1;
        acc
    });
    assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
    assert_eq!(report.dirty_date_keys.as_deref(), Some(expected_dirty_dates.as_slice()));

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let first_dirty_total: i64 = intelligence
        .query_row(
            "SELECT total_visits
             FROM daily_summary_rollups
             WHERE profile_id = 'chrome:Default' AND date_key = ?1",
            [expected_dirty_dates[0].as_str()],
            |row| row.get(0),
        )
        .expect("first dirty rollup");
    let second_dirty_total: i64 = intelligence
        .query_row(
            "SELECT total_visits
             FROM daily_summary_rollups
             WHERE profile_id = 'chrome:Default' AND date_key = ?1",
            [expected_dirty_dates[1].as_str()],
            |row| row.get(0),
        )
        .expect("second dirty rollup");
    let summary_row_count: i64 = intelligence
        .query_row(
            "SELECT COUNT(*)
             FROM daily_summary_rollups
             WHERE profile_id = 'chrome:Default'",
            [],
            |row| row.get(0),
        )
        .expect("summary row count");
    assert_eq!(first_dirty_total, *expected_totals.get(&expected_dirty_dates[0]).unwrap_or(&0));
    assert_eq!(second_dirty_total, *expected_totals.get(&expected_dirty_dates[1]).unwrap_or(&0));
    assert_eq!(summary_row_count, expected_totals.len() as i64);
}

/// Regression coverage for daily rollup fallback collapses conflicting categories into one domain row.
#[test]
fn daily_rollup_fallback_collapses_conflicting_categories_into_one_domain_row() {
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
        "https://github.com/example/repo/pulls/7",
        "Pull Request 7",
        1711929720000,
        Some(2),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    intelligence
        .execute(
            "UPDATE visit_derived_facts
             SET domain_category = CASE
                 WHEN visit_id = 2 THEN 'docs'
                 WHEN visit_id = 4 THEN 'developer'
                 ELSE domain_category
             END
             WHERE visit_id IN (2, 4)",
            [],
        )
        .expect("inject conflicting categories");
    intelligence
        .execute(
            "DELETE FROM core_intelligence_stage_checkpoints
             WHERE profile_id = 'chrome:Default' AND stage = 'daily-rollup'",
            [],
        )
        .expect("clear daily rollup checkpoint");
    drop(intelligence);

    let report = run_core_intelligence_job_type_with_progress(
        &paths,
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
    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));

    let intelligence =
        open_intelligence_connection(&paths, &config, None).expect("intelligence reopen");
    let row_count: i64 = intelligence
        .query_row(
            "SELECT COUNT(*)
             FROM domain_daily_rollups
             WHERE profile_id = 'chrome:Default'
               AND date_key = ?1
               AND registrable_domain = 'github.com'",
            [local_date_key(1711929600000)],
            |row| row.get(0),
        )
        .expect("domain row count");
    let category: String = intelligence
        .query_row(
            "SELECT domain_category
             FROM domain_daily_rollups
             WHERE profile_id = 'chrome:Default'
               AND date_key = ?1
               AND registrable_domain = 'github.com'",
            [local_date_key(1711929600000)],
            |row| row.get(0),
        )
        .expect("selected domain category");
    assert_eq!(row_count, 1);
    assert_eq!(category, "developer");
}

/// Regression coverage for visit derive stage falls back full after visibility regression.
#[test]
fn visit_derive_stage_falls_back_full_after_visibility_regression() {
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

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    archive
        .execute("UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = 2", [])
        .expect("revert visit");
    drop(archive);

    let report = run_core_intelligence_job_type_with_progress(
        &paths,
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
    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(
        report
            .fallback_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("visibility regressed"))
    );

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let row_count: i64 = intelligence
        .query_row("SELECT COUNT(*) FROM visit_derived_facts", [], |row| row.get(0))
        .expect("row count");
    assert_eq!(row_count, 2);
}
