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
        merge_stage_run_result, run_core_intelligence,
        run_core_intelligence_job_type_with_progress, run_core_intelligence_with_progress,
    },
    intelligence_schema::{
        clear_derived_intelligence_state, count_core_intelligence_job_triggers,
        count_core_intelligence_jobs, preview_derived_intelligence_clear, sum_table_row_counts,
        table_row_count,
    },
    intelligence_shared::local_date_key,
};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture};
use crate::test_support::native::TestDatabase;
use crate::{
    config::project_paths_with_root,
    intelligence_catalog::RebuildMode,
    intelligence_runtime::FULL_REBUILD_JOB_TYPE,
    models::{
        AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, CoreIntelligenceStageTimings,
        DateRange, EntityExplanationRequest, IntelligenceEmbedCardsRequest, ScopedDateRangeRequest,
    },
    utils::now_rfc3339,
};
use chrono::{Datelike, TimeZone};
use std::collections::BTreeMap;
use vault_store::params_owned as params;

/// Regression coverage for clear derived intelligence state reports canonical group counts.
#[test]
fn clear_derived_intelligence_state_reports_canonical_group_counts() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
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
    let job_id = intelligence
        .query_one("SELECT MAX(id) FROM intelligence_jobs", (), |r| Ok(r.get::<i64>(0)?))
        .unwrap();
    intelligence
        .execute(
            "INSERT INTO intelligence_job_triggers (job_id, run_id, reason, requested_at)
             VALUES (?1, NULL, 'clear test', ?2)",
            params![job_id, now_rfc3339()],
        )
        .expect("insert runtime trigger");

    let expected_visit_derived = intelligence
        .read(move |session| table_row_count(session, "visit_derived_facts"))
        .expect("visit facts");
    let expected_daily_rollups = intelligence
        .read(move |session| {
            sum_table_row_counts(
                session,
                &[
                    "domain_daily_rollups",
                    "category_daily_rollups",
                    "engine_daily_rollups",
                    "daily_summary_rollups",
                ],
            )
        })
        .expect("daily rollups");
    let expected_structural = intelligence
        .read(move |session| {
            sum_table_row_counts(
                session,
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
        })
        .expect("structural");
    let expected_runtime = intelligence
        .read(move |session| table_row_count(session, "deterministic_module_runtime"))
        .expect("module runtime")
        + intelligence
            .read(move |session| table_row_count(session, "core_intelligence_stage_checkpoints"))
            .expect("stage checkpoints")
        + intelligence.read(count_core_intelligence_jobs).expect("runtime jobs")
        + intelligence.read(count_core_intelligence_job_triggers).expect("runtime triggers");
    drop(intelligence);

    // The preview reports exactly what the clear will remove, and removes nothing itself.
    let preview = preview_derived_intelligence_clear(&vault.with_config(config.clone()))
        .expect("preview derived clear");
    assert!(expected_runtime > 0, "the seeded job gives the preview something to count");
    assert_eq!(preview.cleared_visit_derived_fact_rows, expected_visit_derived);
    assert_eq!(preview.cleared_daily_rollup_rows, expected_daily_rollups);
    assert_eq!(preview.cleared_structural_rows, expected_structural);
    assert_eq!(preview.cleared_runtime_rows, expected_runtime);
    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    assert_eq!(
        intelligence
            .read(move |session| table_row_count(session, "visit_derived_facts"))
            .expect("visit facts"),
        expected_visit_derived
    );
    assert_eq!(intelligence.read(count_core_intelligence_jobs).expect("runtime jobs"), 1);
    drop(intelligence);

    let report = clear_derived_intelligence_state(&vault.with_config(config.clone()))
        .expect("clear derived state");
    assert_eq!(report.cleared_visit_derived_fact_rows, expected_visit_derived);
    assert_eq!(report.cleared_daily_rollup_rows, expected_daily_rollups);
    assert_eq!(report.cleared_structural_rows, expected_structural);
    assert_eq!(report.cleared_runtime_rows, expected_runtime);

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    assert_eq!(
        intelligence
            .read(move |session| table_row_count(session, "visit_derived_facts"))
            .expect("visit facts"),
        0
    );
    assert_eq!(
        intelligence
            .read(move |session| table_row_count(session, "daily_summary_rollups"))
            .expect("daily summaries"),
        0
    );
    assert_eq!(
        intelligence
            .read(move |session| table_row_count(session, "search_trails"))
            .expect("search trails"),
        0
    );
    assert_eq!(
        intelligence
            .read(move |session| table_row_count(session, "core_intelligence_stage_checkpoints"))
            .expect("stage checkpoints"),
        0
    );
    assert_eq!(
        intelligence
            .read(move |session| table_row_count(session, "deterministic_module_runtime"))
            .expect("module runtime"),
        0
    );
    assert_eq!(intelligence.read(count_core_intelligence_jobs).expect("runtime jobs"), 0);
    assert_eq!(
        intelligence.read(count_core_intelligence_job_triggers).expect("runtime triggers"),
        0
    );
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

/// Regression coverage for rebuild progress and empty-profile no-op handling.
#[test]
fn rebuild_with_progress_reports_noop_when_scope_has_no_profiles() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();

    let mut progress = Vec::new();
    let report = run_core_intelligence_with_progress(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
        |event| {
            progress.push(event);
            Ok(())
        },
    )
    .expect("empty scoped rebuild");

    assert!(progress.is_empty());
    assert_eq!(report.execution_mode.as_deref(), Some("noop"));
    assert_eq!(report.affected_profiles.as_deref(), Some([].as_slice()));
    assert_eq!(report.processed_visits, 0);
    assert!(report.notes.iter().any(|note| note.contains("No visible visits")));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let ready_modules: i64 = intelligence
        .query_one(
            "SELECT COUNT(*) FROM deterministic_module_runtime WHERE status = 'ready'",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("ready modules");
    assert!(ready_modules > 0);
}

/// Regression coverage for scoped debug rebuilds preserving the legacy fallback contract.
#[test]
fn scoped_rebuild_uses_batches_and_progress_callbacks() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let mut phases = Vec::new();
    let report = run_core_intelligence_with_progress(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(3),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |event| {
            phases.push((
                event.phase,
                event.processed_items,
                event.total_items,
                event.progress_percent,
            ));
            Ok(())
        },
    )
    .expect("scoped debug rebuild");

    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));
    assert_eq!(report.processed_visits, 3);
    assert_eq!(report.visit_derived_facts, 3);
    assert!(report.sessions > 0);
    assert!(report.search_trails > 0);
    assert_eq!(
        report.affected_profiles.as_deref(),
        Some(["chrome:Default".to_string()].as_slice())
    );
    assert_eq!(phases[0].0, "visit-derived-facts");
    assert_eq!(phases[0].1, Some(0));
    assert_eq!(phases[0].2, Some(3));
    assert_eq!(phases[0].3, Some(0.0));
    assert!(phases.iter().any(|p| p.0 == "daily-rollups"));
    assert!(phases.iter().any(|p| p.0 == "sessions"));
    assert!(phases.iter().any(|p| p.0 == "refind-pages"));
    assert!(phases.iter().all(|p| p.3.is_none_or(|value| (0.0..=100.0).contains(&value))));
}

/// Regression coverage for explain entity and provider snapshots build from core intelligence tables.
#[test]
fn explain_entity_and_provider_snapshots_build_from_core_intelligence_tables() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let rebuild = run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("run core intelligence");
    assert!(rebuild.processed_visits >= 3);
    let stage_timings = rebuild.stage_timings_ms.as_ref().expect("full rebuild stage timings");
    assert!(stage_timings.total_ms >= stage_timings.visit_derive_ms);
    assert!(stage_timings.total_ms >= stage_timings.daily_rollup_ms);
    assert!(stage_timings.total_ms >= stage_timings.structural_rebuild_ms);

    let session_explanation = explain_entity(
        &vault.with_config(config.clone()),
        &EntityExplanationRequest {
            cursor: None,
            page_size: None,
            entity_type: "session".to_string(),
            entity_id: "session:chrome:Default:1".to_string(),
        },
    )
    .expect("session explanation");
    assert_eq!(session_explanation.entity_type, "session");
    assert!(!session_explanation.participating_visit_ids.is_empty());

    let unsupported_error = explain_entity(
        &vault.with_config(config.clone()),
        &EntityExplanationRequest {
            cursor: None,
            page_size: None,
            entity_type: "unsupported".to_string(),
            entity_id: "anything".to_string(),
        },
    )
    .expect_err("unsupported explanation type");
    assert!(unsupported_error.to_string().contains("does not support entity type"));

    let refind_explanation = explain_entity(
        &vault.with_config(config.clone()),
        &EntityExplanationRequest {
            cursor: None,
            page_size: None,
            entity_type: "refind_page".to_string(),
            entity_id: "https://github.com/example/repo/issues/42".to_string(),
        },
    )
    .expect("refind explanation");
    assert_eq!(refind_explanation.entity_type, "refind_page");
    assert!(refind_explanation.factors.iter().any(|factor| factor.label == "cross_day_count"));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    intelligence
        .execute(
            "UPDATE visit_derived_facts_rows
             SET page_category = 'article'
             WHERE visit_id IN (1, 2)",
            (),
        )
        .expect("scope compare set facts");
    intelligence
        .execute(
            "UPDATE visit_assignments SET trail_id='trail-public-compare' WHERE visit_id IN (1,2)",
            (),
        )
        .unwrap();
    for (visit_id, ordinal) in [(1_i64, 1_i64), (2_i64, 2_i64)] {
        intelligence
            .execute(
                "INSERT OR REPLACE INTO search_trail_members_rows
                 (generation,trail_id, profile_id, visit_id, ordinal, role)
                 VALUES ((SELECT navigation FROM intelligence_generations WHERE profile_id='chrome:Default'),'trail-public-compare', 'chrome:Default', ?1, ?2, 'landing')",
                params![visit_id, ordinal],
            )
            .expect("compare set trail member");
    }
    intelligence
        .execute(
            "INSERT OR REPLACE INTO source_effectiveness_rows (
                generation,profile_id, registrable_domain, source_role, trail_count, stable_landing_count,
                effectiveness_score, evidence_json, first_seen_ms, last_seen_ms, computed_at
             ) VALUES (
                (SELECT aggregates FROM intelligence_generations WHERE profile_id='chrome:Default'),'chrome:Default', 'docs.rust-lang.org', 'landing', 4, 3, 0.875, '[]',
                1711929600000, 1712016000000, '2026-04-14T00:00:00Z'
             )",
            (),
        )
        .expect("stable source row");
    let today = chrono::Local::now();
    let anniversary = chrono::Local
        .with_ymd_and_hms(today.year() - 1, today.month(), today.day(), 12, 0, 0)
        .single()
        .expect("anniversary timestamp");
    let anniversary_ms = anniversary.timestamp_millis();
    let date_key = anniversary.format("%Y-%m-%d").to_string();
    intelligence.execute(
        "INSERT INTO daily_summary_rollups_rows(generation,date_key,profile_id,total_visits,total_searches,new_domains,unique_domains,hhi_score,discovery_rate)
         VALUES((SELECT daily FROM intelligence_generations WHERE profile_id='chrome:Default'),?1,'chrome:Default',1,0,1,1,1.0,1.0)",
        params![date_key],
    ).expect("anniversary summary");
    intelligence.execute(
        "INSERT INTO domain_daily_rollups_rows(generation,date_key,profile_id,registrable_domain,domain_category,visit_count,search_count,new_domain_visits,unique_urls)
         VALUES((SELECT daily FROM intelligence_generations WHERE profile_id='chrome:Default'),?1,'chrome:Default','anniversary.example','reference',1,0,1,1)",
        params![date_key],
    ).expect("anniversary domain");
    intelligence.execute(
        "INSERT INTO sessions_rows(generation,session_id,profile_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,auto_title,computed_at)
         VALUES((SELECT navigation FROM intelligence_generations WHERE profile_id='chrome:Default'),'session:anniversary','chrome:Default',?1,?1,1,0,1,1,'Anniversary review','2026-04-14T00:00:00Z')",
        params![anniversary_ms],
    ).expect("anniversary session");
    drop(intelligence);

    let compare_explanation = explain_entity(
        &vault.with_config(config.clone()),
        &EntityExplanationRequest {
            cursor: None,
            page_size: None,
            entity_type: "compare_set".to_string(),
            entity_id: "compare:trail-public-compare:article".to_string(),
        },
    )
    .expect("compare set explanation");
    assert_eq!(compare_explanation.entity_type, "compare_set");
    assert_eq!(compare_explanation.participating_visit_ids, vec![1, 2]);

    let embed_cards = get_intelligence_embed_cards(
        &vault.with_config(config.clone()),
        &IntelligenceEmbedCardsRequest {
            cursor: None,
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(6),
        },
    )
    .expect("embed cards");
    assert!(!embed_cards.is_empty());
    assert!(embed_cards.iter().any(|card| card.card_type == "digest"));
    assert!(embed_cards.iter().any(|card| card.card_type == "top_site"));
    assert!(embed_cards.iter().any(|card| card.card_type == "refind_page"));
    assert!(embed_cards.iter().any(|card| card.card_type == "stable_source"));
    assert!(embed_cards.iter().any(|card| card.card_type == "on_this_day"));

    let public_snapshot = get_intelligence_public_snapshot(
        &vault.with_config(config.clone()),
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
        &vault.with_config(config.clone()),
        &IntelligenceEmbedCardsRequest {
            cursor: None,
            date_range: DateRange {
                start: "2024-04-01".to_string(),
                end: "2024-04-30".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            limit: Some(8),
        },
    )
    .expect("widget snapshot");
    assert_eq!(widget_snapshot.highlights.len(), embed_cards.len());
    assert!(widget_snapshot.notes.iter().any(|note| note.contains("internal_only")));
}

/// Regression coverage for visit derive stage processes only new visible visits.
#[test]
fn visit_derive_stage_processes_only_new_visible_visits() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");
    let visit_noop_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("noop visit derive stage");
    assert_eq!(visit_noop_report.execution_mode.as_deref(), Some("noop"));
    assert!(visit_noop_report.notes.iter().any(|note| note.contains("already up to date")));
    let noop_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("noop daily rollup stage");
    assert_eq!(noop_report.execution_mode.as_deref(), Some("noop"));
    assert!(noop_report.notes.iter().any(|note| note.contains("already up to date")));

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(move |s| {
            append_fixture_visit(
                s,
                4,
                "https://docs.example.com/sqlite/wal-checkpoint",
                "SQLite WAL Checkpoint",
                1712102400000,
                None,
                None,
            );
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
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

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let checkpoint = intelligence
        .query_one(
            "SELECT last_processed_visit_id
             FROM core_intelligence_stage_checkpoints
             WHERE profile_id = 'chrome:Default' AND stage = 'visit-derive'",
            (),
            |row| Ok(row.get::<i64>(0)?),
        )
        .expect("stage checkpoint");
    assert_eq!(checkpoint, 4);
}

/// Regression coverage for visit-derived checkpoint drift and empty-profile clearing.
#[test]
fn visit_derive_stage_handles_version_drift_and_empty_profile_clearing() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");
    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    intelligence
        .execute(
            "UPDATE intelligence_builds SET state_json=json_set(state_json,'$.rule_version','old') WHERE profile_id='chrome:Default'",
            (),
        )
        .expect("downgrade visit checkpoint version");
    drop(intelligence);

    let drift_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive version drift");
    assert_eq!(drift_report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(
        drift_report
            .fallback_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("rules changed"))
    );

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .execute("UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z'", ())
        .expect("revert all visits");
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let empty_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive empty profile");
    assert_eq!(empty_report.execution_mode.as_deref(), Some("noop"));
    assert!(empty_report.notes.iter().any(|note| note.contains("No visible visits")));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let derived_rows: i64 = intelligence
        .query_one("SELECT COUNT(*) FROM visit_derived_facts", (), |row| Ok(row.get(0)?))
        .expect("derived rows");
    assert_eq!(derived_rows, 0);
}

/// Regression coverage for structural checkpoint drift and empty-profile clearing.
#[test]
fn structural_stage_handles_noop_version_drift_and_empty_profile_clearing() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");
    let noop_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("noop structural stage");
    assert_eq!(noop_report.execution_mode.as_deref(), Some("noop"));
    assert!(noop_report.notes.iter().any(|note| note.contains("already up to date")));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    intelligence
        .execute(
            "UPDATE intelligence_builds SET state_json=json_set(state_json,'$.rule_version','old') WHERE profile_id='chrome:Default'",
            (),
        )
        .expect("downgrade structural checkpoint version");
    drop(intelligence);

    let drift_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural version drift");
    assert_eq!(drift_report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(
        drift_report
            .fallback_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("logic changed"))
    );

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .execute("UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z'", ())
        .expect("revert all visits");
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let empty_report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural empty profile");
    assert_eq!(empty_report.execution_mode.as_deref(), Some("noop"));
    assert!(empty_report.notes.iter().any(|note| note.contains("No visible visits")));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let structural_rows: i64 = intelligence
        .query_one("SELECT COUNT(*) FROM search_trails", (), |row| Ok(row.get(0)?))
        .expect("structural rows");
    assert_eq!(structural_rows, 0);
}

/// Regression coverage for daily rollup stage recomputes only dirty days.
#[test]
fn daily_rollup_stage_recomputes_only_dirty_days() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(move |s| {
            append_fixture_visit(
                s,
                4,
                "https://docs.example.com/sqlite/wal-checkpoint",
                "SQLite WAL Checkpoint",
                1712059200000,
                None,
                None,
            );
            Ok(())
        })
        .unwrap();
    archive
        .write(move |s| {
            append_fixture_visit(
                s,
                5,
                "https://example.com/deep-dive",
                "Deep Dive",
                1712145600000,
                None,
                None,
            );
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    let report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
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

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let first_dirty_total: i64 = intelligence
        .query_one(
            "SELECT total_visits
             FROM daily_summary_rollups
             WHERE profile_id = 'chrome:Default' AND date_key = ?1",
            [expected_dirty_dates[0].as_str()],
            |row| Ok(row.get(0)?),
        )
        .expect("first dirty rollup");
    let second_dirty_total: i64 = intelligence
        .query_one(
            "SELECT total_visits
             FROM daily_summary_rollups
             WHERE profile_id = 'chrome:Default' AND date_key = ?1",
            [expected_dirty_dates[1].as_str()],
            |row| Ok(row.get(0)?),
        )
        .expect("second dirty rollup");
    let summary_row_count: i64 = intelligence
        .query_one(
            "SELECT COUNT(*)
             FROM daily_summary_rollups
             WHERE profile_id = 'chrome:Default'",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("summary row count");
    assert_eq!(first_dirty_total, *expected_totals.get(&expected_dirty_dates[0]).unwrap_or(&0));
    assert_eq!(second_dirty_total, *expected_totals.get(&expected_dirty_dates[1]).unwrap_or(&0));
    assert_eq!(summary_row_count, expected_totals.len() as i64);
}

/// Regression coverage for daily rollup stage clears stale rows when a profile loses visible visits.
#[test]
fn daily_rollup_stage_clears_when_profile_has_no_visible_visits() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE source_profile_id = 1",
            (),
        )
        .expect("revert all visits");
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("daily rollup no visible visits");
    assert_eq!(report.execution_mode.as_deref(), Some("noop"));
    assert!(report.notes.iter().any(|note| note.contains("No visible visits remained")));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let rollup_rows = intelligence
        .read(move |session| {
            sum_table_row_counts(
                session,
                &[
                    "domain_daily_rollups",
                    "category_daily_rollups",
                    "engine_daily_rollups",
                    "daily_summary_rollups",
                ],
            )
        })
        .expect("rollup row count");
    assert_eq!(rollup_rows, 0);
}

/// A facts checkpoint and its committed rows stay aligned after an interrupted batch.
#[test]
fn daily_rollup_resumes_an_interrupted_fact_batch_without_missing_delta_rows() {
    let vault = crate::Vault::open_temp().unwrap();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    super::fixtures::sync_archive_totals(&archive);
    run_core_intelligence(&vault, &Default::default()).unwrap();
    archive
        .write(|s| {
            append_fixture_visit(
                s,
                4,
                "https://docs.example.com/four",
                "Four",
                1712059200000,
                None,
                None,
            );
            append_fixture_visit(
                s,
                5,
                "https://docs.example.com/five",
                "Five",
                1712059260000,
                None,
                None,
            );
            Ok(())
        })
        .unwrap();
    let interrupted = run_core_intelligence_job_type_with_progress(
        &vault,
        "visit-derive",
        &CoreIntelligenceRebuildRequest { limit: Some(1), ..Default::default() },
        |p| {
            if p.phase == "visit-derived-facts" && p.processed_items == Some(4) {
                anyhow::bail!("interrupt after a committed delta batch");
            }
            Ok(())
        },
    );
    assert!(interrupted.is_err());
    run_core_intelligence_job_type_with_progress(
        &vault,
        "daily-rollup",
        &Default::default(),
        |_| Ok(()),
    )
    .unwrap();
    vault
        .read(vault_store::Db::Intelligence, |s| {
            assert_eq!(
                s.query_one("SELECT COUNT(DISTINCT visit_id) FROM visit_derived_facts", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })?,
                5
            );
            assert_eq!(
                s.query_one("SELECT SUM(total_visits) FROM daily_summary_rollups", (), |r| Ok(
                    r.get::<i64>(0)?
                ))?,
                5
            );
            Ok(())
        })
        .unwrap();
}

/// Regression coverage for daily rollup checkpoint version drift.
#[test]
fn daily_rollup_stage_falls_back_after_version_drift() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    intelligence
        .execute(
            "UPDATE intelligence_builds SET state_json=json_set(state_json,'$.rule_version','old') WHERE profile_id='chrome:Default'",
            (),
        )
        .expect("downgrade daily checkpoint version");
    drop(intelligence);

    let report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("daily rollup version drift");
    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(
        report
            .fallback_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("Daily rollup logic changed"))
    );
}

/// Regression coverage for daily rollup fallback collapses conflicting categories into one domain row.
#[test]
fn daily_rollup_fallback_collapses_conflicting_categories_into_one_domain_row() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive
        .write(move |s| {
            append_fixture_visit(
                s,
                4,
                "https://github.com/example/repo/pulls/7",
                "Pull Request 7",
                1711929720000,
                Some(2),
                None,
            );
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "visit-derive",
        &CoreIntelligenceRebuildRequest::default(),
        |_| Ok(()),
    )
    .expect("full rebuild");

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    intelligence
        .execute(
            "UPDATE visit_derived_facts_rows
             SET domain_category = CASE
                 WHEN visit_id = 2 THEN 'docs'
                 WHEN visit_id = 4 THEN 'developer'
                 ELSE domain_category
             END
             WHERE visit_id IN (2, 4)",
            (),
        )
        .expect("inject conflicting categories");
    drop(intelligence);

    let report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("daily rollup fallback");
    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let row_count: i64 = intelligence
        .query_one(
            "SELECT COUNT(*)
             FROM domain_daily_rollups
             WHERE profile_id = 'chrome:Default'
               AND date_key = ?1
               AND registrable_domain = 'github.com'",
            [local_date_key(1711929600000)],
            |row| Ok(row.get(0)?),
        )
        .expect("domain row count");
    let category: String = intelligence
        .query_one(
            "SELECT domain_category
             FROM domain_daily_rollups
             WHERE profile_id = 'chrome:Default'
               AND date_key = ?1
               AND registrable_domain = 'github.com'",
            [local_date_key(1711929600000)],
            |row| Ok(row.get(0)?),
        )
        .expect("selected domain category");
    assert_eq!(row_count, 1);
    assert_eq!(category, "developer");
}

/// Regression coverage for visit derive stage falls back full after visibility regression.
#[test]
fn visit_derive_stage_falls_back_full_after_visibility_regression() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .execute("UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = 2", ())
        .expect("revert visit");
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    let report = run_core_intelligence_job_type_with_progress(
        &vault.with_config(config.clone()),
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

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let row_count: i64 = intelligence
        .query_one("SELECT COUNT(*) FROM visit_derived_facts", (), |row| Ok(row.get(0)?))
        .expect("row count");
    assert_eq!(row_count, 2);
}

/// Covers the `force_full` (manual full rebuild) branch of all three stage executors: running each
/// stage job with `full_rebuild: true` takes the "Manual full rebuild requested" fallback path rather
/// than the incremental delta path.
#[test]
fn stage_jobs_take_the_manual_full_rebuild_branch_when_forced() {
    let root = tempfile::tempdir().expect("tempdir");
    let _paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _paths = vault.paths().clone();
    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    archive.execute("UPDATE archive_totals SET value=(SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL) WHERE name IN ('visits','sourceVisits:1')",()).unwrap();
    drop(archive);

    // Establish a baseline so the stages have a watermark/checkpoint to ignore when forced.
    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("baseline full rebuild");

    // Each stage, forced full, must run in "full" mode (not noop/incremental) — the force_full arm.
    for job_type in ["visit-derive", "daily-rollup", "structural-rebuild"] {
        let report = run_core_intelligence_job_type_with_progress(
            &vault.with_config(config.clone()),
            job_type,
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                full_rebuild: true,
                ..CoreIntelligenceRebuildRequest::default()
            },
            |_progress| Ok(()),
        )
        .unwrap_or_else(|error| panic!("forced {job_type} stage: {error:#}"));
        // A forced rebuild takes the manual-full fallback path (the `force_full` arm), reported as
        // "fallback-full" rather than the incremental/noop modes.
        assert_eq!(
            report.execution_mode.as_deref(),
            Some("fallback-full"),
            "a forced {job_type} stage must run via the manual full-rebuild fallback"
        );
    }
}
