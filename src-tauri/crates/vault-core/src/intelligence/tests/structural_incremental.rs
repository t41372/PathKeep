//! Structural incremental rebuild regressions.
//!
//! ## Responsibilities
//! - Protect incremental session/trail assignment updates when older visits arrive late.
//! - Verify structural tail streaming across batch boundaries.
//!
//! ## Not responsible for
//! - Overview or schema migration coverage.
//!
//! ## Dependencies
//! - `fixtures` appends deterministic chains that exercise tail streaming.
//! - Core Intelligence rebuild entrypoints run the same stage contract used by
//!   worker jobs.
//!
//! ## Performance notes
//! The boundary test crosses `STRUCTURAL_TAIL_STREAM_BATCH_SIZE` to catch regressions in
//! streaming behavior without loading an unbounded visit vector.

use super::fixtures::sync_archive_totals;
use crate::test_support::native::TestDatabase;

use super::super::{
    STRUCTURAL_TAIL_STREAM_BATCH_SIZE,
    intelligence_rebuild::{run_core_intelligence, run_core_intelligence_job_type_with_progress},
};
use super::fixtures::{
    append_fixture_chain_visits, append_fixture_visit, seed_core_intelligence_fixture,
};
use crate::{
    config::project_paths_with_root,
    models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
};
use vault_store::params_owned as params;

/// Regression coverage for structural stage updates tail assignments incrementally.
#[test]
fn late_visits_rebuild_tail_assignments_without_duplicates() {
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
        .write(move |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&archive);
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
                "https://github.com/example/repo/pulls/7",
                "Pull Request 7",
                1711929900000,
                Some(2),
                None,
            );
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&archive);
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
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural stage");
    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));
    assert_eq!(report.dirty_visit_count, Some(1));

    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let assignments = intelligence
        .prepare(
            "SELECT visit_id, session_id, trail_id
             FROM visit_derived_facts
             WHERE visit_id IN (1, 2, 4)
             ORDER BY visit_id ASC",
        )
        .expect("prepare assignments")
        .query_map((), |row| {
            Ok((row.get::<i64>(0)?, row.get::<Option<String>>(1)?, row.get::<Option<String>>(2)?))
        })
        .expect("query assignments")
        .collect::<vault_store::Result<Vec<_>>>()
        .expect("collect assignments");
    assert_eq!(assignments[0].1.as_deref(), Some("session:chrome:Default:1"));
    assert_eq!(assignments[1].1.as_deref(), Some("session:chrome:Default:1"));
    assert_eq!(assignments[2].1.as_deref(), Some("session:chrome:Default:1"));
    assert_eq!(assignments[0].2.as_deref(), Some("trail:chrome:Default:1"));
    assert_eq!(assignments[1].2.as_deref(), Some("trail:chrome:Default:1"));
    assert_eq!(assignments[2].2.as_deref(), Some("trail:chrome:Default:1"));
}

/// Regression coverage for structural stream keeps assignments stable across batch boundaries.
#[test]
fn late_visit_chains_keep_assignments_stable_across_batch_boundaries() {
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
        .write(move |s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&archive);
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let archive = TestDatabase::new(&vault, vault_store::Db::Archive);
    archive
        .write(move |s| {
            append_fixture_chain_visits(
                s,
                4,
                STRUCTURAL_TAIL_STREAM_BATCH_SIZE + 5,
                1711929720000,
                2,
            );
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&archive);
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
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural stage");
    assert_eq!(report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(report.processed_visits > STRUCTURAL_TAIL_STREAM_BATCH_SIZE);

    let boundary_visit_id = 4 + STRUCTURAL_TAIL_STREAM_BATCH_SIZE as i64 - 1;
    let trailing_visit_id = 4 + STRUCTURAL_TAIL_STREAM_BATCH_SIZE as i64 + 4;
    let intelligence = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let assignments = intelligence
        .prepare(
            "SELECT visit_id, session_id, trail_id
             FROM visit_derived_facts
             WHERE visit_id IN (1, 2, 4, ?1, ?2)
             ORDER BY visit_id ASC",
        )
        .expect("prepare assignments")
        .query_map(params![boundary_visit_id, trailing_visit_id], |row| {
            Ok((row.get::<i64>(0)?, row.get::<Option<String>>(1)?, row.get::<Option<String>>(2)?))
        })
        .expect("query assignments")
        .collect::<vault_store::Result<Vec<_>>>()
        .expect("collect assignments");
    assert_eq!(assignments.len(), 5);
    for (_, session_id, trail_id) in assignments {
        assert_eq!(session_id.as_deref(), Some("session:chrome:Default:1"));
        assert_eq!(trail_id.as_deref(), Some("trail:chrome:Default:1"));
    }
}
