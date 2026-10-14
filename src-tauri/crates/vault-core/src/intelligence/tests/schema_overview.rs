//! All-time overview snapshot regressions.
//!
//! ## Responsibilities
//! - Check that the all-time primary and secondary overviews are served from their persisted
//!   snapshots, and that a bounded range is computed fresh.
//!
//! ## Not responsible for
//! - Exercising rebuild fallback equivalence or structural tail streaming.
//! - When a snapshot goes stale; `intelligence_overview_snapshot` tests own that.
//!
//! ## Performance notes
//! Reading all time from the snapshot takes about 1.5 s on a 14.4M-visit archive, against over a
//! minute when it is computed again on every open.

use super::super::{
    get_intelligence_primary_overview, get_intelligence_secondary_overview,
    intelligence_rebuild::run_core_intelligence,
};
use super::fixtures::seed_core_intelligence_fixture;
use crate::test_support::native::TestDatabase;
use crate::{
    config::project_paths_with_root,
    models::{
        AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange, ScopedDateRangeRequest,
    },
};
use vault_store::params_owned as params;

/// All-time overview requests must persist a snapshot and then be served from it
/// verbatim until the archive fingerprint changes.
#[test]
fn all_time_primary_overview_serves_persisted_snapshot() {
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
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let all_time = ScopedDateRangeRequest {
        date_range: DateRange {
            start: "1900-01-01".to_string(),
            end: chrono::Local::now().format("%Y-%m-%d").to_string(),
        },
        profile_id: None,
    };

    let first = get_intelligence_primary_overview(&vault.with_config(config.clone()), &all_time)
        .expect("first all-time");

    // The archive-wide all-time scope must now have exactly one persisted snapshot.
    let connection = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let stored: i64 = connection
        .query_one(
            "SELECT COUNT(*) FROM intelligence_overview_snapshots
             WHERE scope_key = 'all-time:archive-wide' AND band = 'primary'",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("snapshot count");
    assert_eq!(stored, 1);

    // Doctor the stored payload while keeping the fingerprint, then confirm the
    // next request is served verbatim from the snapshot rather than recomputed.
    let doctored = {
        let mut payload = first.clone();
        payload.total_duration_ms = 4242;
        serde_json::to_string(&payload).expect("serialize doctored payload")
    };
    connection
        .execute(
            "UPDATE intelligence_overview_snapshots SET payload_json = ?1
             WHERE scope_key = 'all-time:archive-wide' AND band = 'primary'",
            params![doctored],
        )
        .expect("doctor payload");
    drop(connection);

    let second = get_intelligence_primary_overview(&vault.with_config(config.clone()), &all_time)
        .expect("second all-time");
    assert_eq!(
        second.total_duration_ms, 4242,
        "all-time overview must be served from the persisted snapshot"
    );

    // A bounded scope must bypass the snapshot and recompute fresh.
    let scoped = ScopedDateRangeRequest {
        date_range: DateRange { start: "2024-04-01".to_string(), end: "2024-04-30".to_string() },
        profile_id: None,
    };
    let scoped_overview =
        get_intelligence_primary_overview(&vault.with_config(config.clone()), &scoped)
            .expect("scoped overview");
    assert_ne!(scoped_overview.total_duration_ms, 4242);
}

/// The all-time SECONDARY overview persists and is then served from its own
/// snapshot band, independently of the primary band.
#[test]
fn all_time_secondary_overview_serves_persisted_snapshot() {
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
    drop(archive);

    run_core_intelligence(
        &vault.with_config(config.clone()),
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("full rebuild");

    let all_time = ScopedDateRangeRequest {
        date_range: DateRange {
            start: "1900-01-01".to_string(),
            end: chrono::Local::now().format("%Y-%m-%d").to_string(),
        },
        profile_id: None,
    };

    let first = get_intelligence_secondary_overview(&vault.with_config(config.clone()), &all_time)
        .expect("first all-time secondary");

    let connection = TestDatabase::new(&vault, vault_store::Db::Intelligence);
    let stored: i64 = connection
        .query_one(
            "SELECT COUNT(*) FROM intelligence_overview_snapshots
             WHERE scope_key = 'all-time:archive-wide' AND band = 'secondary'",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("snapshot count");
    assert_eq!(stored, 1);

    let doctored = {
        let mut payload = first.clone();
        payload.total_duration_ms = 7373;
        serde_json::to_string(&payload).expect("serialize doctored payload")
    };
    connection
        .execute(
            "UPDATE intelligence_overview_snapshots SET payload_json = ?1
             WHERE scope_key = 'all-time:archive-wide' AND band = 'secondary'",
            params![doctored],
        )
        .expect("doctor payload");
    drop(connection);

    let second = get_intelligence_secondary_overview(&vault.with_config(config.clone()), &all_time)
        .expect("second all-time secondary");
    assert_eq!(
        second.total_duration_ms, 7373,
        "all-time secondary overview must be served from the persisted snapshot"
    );

    let scoped = ScopedDateRangeRequest {
        date_range: DateRange { start: "2024-04-01".to_string(), end: "2024-04-30".to_string() },
        profile_id: None,
    };
    let scoped_overview =
        get_intelligence_secondary_overview(&vault.with_config(config.clone()), &scoped)
            .expect("scoped secondary overview");
    assert_ne!(scoped_overview.total_duration_ms, 7373);
}
