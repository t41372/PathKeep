//! Incremental stage regressions.
//!
//! ## Responsibilities
//! - Check that an append derives only the new visible visits.
//! - Check that the daily-rollup stage recomputes only the dirty days, and that it resumes an
//!   interrupted fact batch without missing a delta row.
//!
//! ## Not responsible for
//! - Batch equivalence against clean full rebuilds.
//! - Structural tail reassignment behavior.
//!
//! ## Dependencies
//! - `fixtures` seeds canonical archive rows and appends focused visit deltas.
//!
//! ## Performance notes
//! Tests assert dirty-date and dirty-visit scoping so incremental stages do not
//! regress into unnecessary full-table work for ordinary updates.

use super::super::{
    intelligence_rebuild::{run_core_intelligence, run_core_intelligence_job_type_with_progress},
    intelligence_shared::local_date_key,
};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture};
use crate::test_support::native::TestDatabase;
use crate::{
    config::project_paths_with_root,
    models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
};
use std::collections::BTreeMap;

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
