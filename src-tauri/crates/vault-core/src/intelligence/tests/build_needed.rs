//! When opening an archive queues an Insights build by itself (`insights_build_needed`).
//!
//! Failure modes: Insights built with rules this version changed keep the old labels until the
//! next backup; Insights built with the current rules are rebuilt on every open; a second build
//! is queued while one runs.

use super::super::intelligence_rebuild::run_core_intelligence;
use super::fixtures::{seed_core_intelligence_fixture, sync_archive_totals};
use crate::intelligence_runtime::{FULL_REBUILD_JOB_TYPE, insights_build_needed};
use crate::models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest};
use crate::test_support::native::TestDatabase;
use crate::utils::now_rfc3339;
use std::sync::atomic::{AtomicUsize, Ordering};
use vault_store::{Db, params_owned as params};

fn archive_with_visits() -> crate::Vault {
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let vault = crate::Vault::open_temp().unwrap().with_config(config);
    let archive = TestDatabase::new(&vault, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    sync_archive_totals(&archive);
    vault
}

/// Adds an Insights job in `state`, newer than every job before it.
fn add_job(vault: &crate::Vault, state: &str) {
    static JOBS: AtomicUsize = AtomicUsize::new(0);
    let key = format!("build-needed:{}", JOBS.fetch_add(1, Ordering::Relaxed));
    TestDatabase::new(vault, Db::Intelligence)
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, NULL, NULL, ?2, 50, 1, ?3, '{}', '{}', ?4, ?4, ?4, ?4)",
            params![FULL_REBUILD_JOB_TYPE, state, key, now_rfc3339()],
        )
        .unwrap();
}

fn built(vault: &crate::Vault) {
    run_core_intelligence(vault, &CoreIntelligenceRebuildRequest::default()).expect("build");
    add_job(vault, "succeeded");
}

/// The profiles' last builds as if an older PathKeep, with other rules, had made them.
fn built_by_older_rules(vault: &crate::Vault) {
    let db = TestDatabase::new(vault, Db::Intelligence);
    let states = db
        .read(|s| {
            Ok(s.query("SELECT id,state_json FROM intelligence_builds", params![], |r| {
                Ok((r.get::<i64>(0)?, r.get::<String>(1)?))
            })?)
        })
        .unwrap();
    assert!(!states.is_empty());
    for (id, json) in states {
        let mut state: serde_json::Value = serde_json::from_str(&json).unwrap();
        state["rule_version"] = "visit-derived-facts-v2:m5-taxonomy-v1:older".into();
        db.execute(
            "UPDATE intelligence_builds SET state_json=?2 WHERE id=?1",
            params![id, state.to_string()],
        )
        .unwrap();
    }
}

#[test]
fn insights_built_with_older_rules_are_built_again() {
    let vault = archive_with_visits();
    built(&vault);
    assert_eq!(insights_build_needed(&vault).unwrap(), None, "current rules");

    built_by_older_rules(&vault);
    assert!(insights_build_needed(&vault).unwrap().is_some());

    add_job(&vault, "running");
    assert_eq!(insights_build_needed(&vault).unwrap(), None, "a build is already running");
}
