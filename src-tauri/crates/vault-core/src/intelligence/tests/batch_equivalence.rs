//! Batch equivalence regressions for Core Intelligence rebuild helpers.
//!
//! ## Responsibilities
//! - Verify fallback-full rebuilds match clean full rebuilds across batch
//!   boundaries.
//!
//! ## Not responsible for
//! - Route-facing overview or explanation DTO coverage.
//! - Remote backup or AI/enrichment behavior.
//!
//! ## Dependencies
//! - `fixtures` seeds paired fallback and clean archives.
//!
//! ## Performance notes
//! These tests intentionally cross configured fallback batch sizes. They compare
//! persisted rows instead of holding large expected structures in memory.

use super::fixtures::sync_archive_totals;
use crate::test_support::native::TestDatabase;

use super::super::{
    DAILY_ROLLUP_FALLBACK_BATCH_SIZE, VISIT_DERIVE_FALLBACK_BATCH_SIZE,
    intelligence_rebuild::{run_core_intelligence, run_core_intelligence_job_type_with_progress},
};
use super::fixtures::{
    append_many_fixture_visits, load_daily_rollup_rows, load_visit_derived_fact_rows,
    seed_core_intelligence_fixture,
};
use crate::{
    config::project_paths_with_root,
    models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
};

/// Regression coverage for visit derive fallback matches clean full rebuild across batches.
#[test]
fn visit_derive_fallback_matches_clean_full_rebuild_across_batches() {
    let fallback_root = tempfile::tempdir().expect("fallback tempdir");
    let clean_root = tempfile::tempdir().expect("clean tempdir");
    let _fallback_paths = project_paths_with_root(fallback_root.path());
    let _clean_paths = project_paths_with_root(clean_root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let fallback_vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let clean_vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _fallback_paths = fallback_vault.paths().clone();
    let _clean_paths = clean_vault.paths().clone();
    let reverted_visit_id = 1200_i64;

    let fallback_archive = TestDatabase::new(&fallback_vault, vault_store::Db::Archive);
    fallback_archive
        .write(move |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    fallback_archive
        .write(move |s| {
            append_many_fixture_visits(s, 4, VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17, 1712145600000);
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&fallback_archive);
    drop(fallback_archive);

    run_core_intelligence(
        &fallback_vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("fallback full rebuild");

    let fallback_archive = TestDatabase::new(&fallback_vault, vault_store::Db::Archive);
    fallback_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert fallback visit");
    sync_archive_totals(&fallback_archive);
    drop(fallback_archive);

    let fallback_report = run_core_intelligence_job_type_with_progress(
        &fallback_vault.with_config(config.clone()),
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

    let clean_archive = TestDatabase::new(&clean_vault, vault_store::Db::Archive);
    clean_archive
        .write(move |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    clean_archive
        .write(move |s| {
            append_many_fixture_visits(s, 4, VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17, 1712145600000);
            Ok(())
        })
        .unwrap();
    clean_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert clean visit");
    sync_archive_totals(&clean_archive);
    drop(clean_archive);

    run_core_intelligence(
        &clean_vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("clean full rebuild");

    let fallback_intelligence = TestDatabase::new(&fallback_vault, vault_store::Db::Intelligence);
    let clean_intelligence = TestDatabase::new(&clean_vault, vault_store::Db::Intelligence);
    assert_eq!(
        fallback_intelligence.read(move |s| Ok(load_visit_derived_fact_rows(s))).unwrap(),
        clean_intelligence.read(move |s| Ok(load_visit_derived_fact_rows(s))).unwrap()
    );
}

/// Regression coverage for daily rollup fallback matches clean full rebuild across batches.
#[test]
fn daily_rollup_fallback_matches_clean_full_rebuild_across_batches() {
    let fallback_root = tempfile::tempdir().expect("fallback tempdir");
    let clean_root = tempfile::tempdir().expect("clean tempdir");
    let _fallback_paths = project_paths_with_root(fallback_root.path());
    let _clean_paths = project_paths_with_root(clean_root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let fallback_vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let clean_vault = crate::Vault::open_temp().unwrap().with_config(config.clone());
    let _fallback_paths = fallback_vault.paths().clone();
    let _clean_paths = clean_vault.paths().clone();
    let reverted_visit_id = 1200_i64;

    let fallback_archive = TestDatabase::new(&fallback_vault, vault_store::Db::Archive);
    fallback_archive
        .write(move |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    fallback_archive
        .write(move |s| {
            append_many_fixture_visits(s, 4, DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17, 1712145600000);
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&fallback_archive);
    drop(fallback_archive);

    run_core_intelligence(
        &fallback_vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("fallback full rebuild");

    let fallback_archive = TestDatabase::new(&fallback_vault, vault_store::Db::Archive);
    fallback_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert fallback visit");
    sync_archive_totals(&fallback_archive);
    drop(fallback_archive);

    run_core_intelligence_job_type_with_progress(
        &fallback_vault.with_config(config.clone()),
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive fallback");
    let fallback_report = run_core_intelligence_job_type_with_progress(
        &fallback_vault.with_config(config.clone()),
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

    let clean_archive = TestDatabase::new(&clean_vault, vault_store::Db::Archive);
    clean_archive
        .write(move |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    clean_archive
        .write(move |s| {
            append_many_fixture_visits(s, 4, DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17, 1712145600000);
            Ok(())
        })
        .unwrap();
    clean_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert clean visit");
    sync_archive_totals(&clean_archive);
    drop(clean_archive);

    run_core_intelligence(
        &clean_vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("clean full rebuild");

    let fallback_intelligence = TestDatabase::new(&fallback_vault, vault_store::Db::Intelligence);
    let clean_intelligence = TestDatabase::new(&clean_vault, vault_store::Db::Intelligence);
    assert_eq!(
        fallback_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "domain_daily_rollups")))
            .unwrap(),
        clean_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "domain_daily_rollups")))
            .unwrap()
    );
    assert_eq!(
        fallback_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "category_daily_rollups")))
            .unwrap(),
        clean_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "category_daily_rollups")))
            .unwrap()
    );
    assert_eq!(
        fallback_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "engine_daily_rollups")))
            .unwrap(),
        clean_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "engine_daily_rollups")))
            .unwrap()
    );
    assert_eq!(
        fallback_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "daily_summary_rollups")))
            .unwrap(),
        clean_intelligence
            .read(move |s| Ok(load_daily_rollup_rows(s, "daily_summary_rollups")))
            .unwrap()
    );
}
