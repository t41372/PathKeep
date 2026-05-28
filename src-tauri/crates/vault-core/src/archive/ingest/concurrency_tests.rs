//! Concurrent ingest contract tests.
//!
//! ## Responsibilities
//! - Pin same-profile archive writer serialization at the SQLite transaction
//!   boundary.
//! - Verify watermark reads happen after a contending writer has committed.
//!
//! ## Not responsible for
//! - Exercising browser discovery, parser chunking, or UI worker dispatch.
//! - Replacing queue-level or end-to-end Tauri command tests.
//!
//! ## Dependencies
//! - Uses the real archive connection setup so the busy timeout, WAL mode, and
//!   schema bootstrap match production.
//! - Uses ingest-local watermark/profile helpers to target the race described
//!   in `import-dedup-audit.md`.
//!
//! ## Performance notes
//! - The test uses tiny temp archives and a bounded timeout; it does not create
//!   large source fixtures.

use super::{
    parser::{Watermark, load_watermark, save_watermark},
    writes::upsert_source_profile,
};
use crate::{
    archive::open_archive_connection,
    config::{ProjectPaths, project_paths_with_root},
    models::{AppConfig, BrowserProfile, BrowserRetentionBoundary},
};
use std::{sync::mpsc, thread, time::Duration};
use tempfile::tempdir;

fn test_config() -> AppConfig {
    AppConfig { initialized: true, ..AppConfig::default() }
}

fn test_paths(root: &std::path::Path) -> ProjectPaths {
    project_paths_with_root(root)
}

fn profile() -> BrowserProfile {
    BrowserProfile {
        profile_id: "chrome:Default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Chrome".to_string(),
        user_name: Some("tim@example.com".to_string()),
        profile_path: "/tmp/chrome-default".to_string(),
        history_path: Some("/tmp/chrome-default/History".to_string()),
        favicons_path: Some("/tmp/chrome-default/Favicons".to_string()),
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("146.0.0.0".to_string()),
        history_file_name: "History".to_string(),
        history_bytes: 128,
        favicons_bytes: 64,
        supporting_bytes: 0,
        retention_boundary: BrowserRetentionBoundary::default(),
    }
}

#[test]
fn same_profile_writer_waits_for_committed_watermark() {
    let dir = tempdir().expect("tempdir");
    let paths = test_paths(dir.path());
    let config = test_config();
    let profile = profile();
    let mut first_archive = open_archive_connection(&paths, &config, None).expect("first archive");
    let first_transaction = first_archive.transaction().expect("first transaction");

    upsert_source_profile(&first_transaction, &profile).expect("first source profile");
    save_watermark(
        &first_transaction,
        &profile.profile_id,
        &Watermark {
            last_visit_id: 41,
            last_url_last_visit_time: 99,
            updated_at: "2026-05-26T00:00:00Z".to_string(),
            ..Watermark::default()
        },
    )
    .expect("first watermark");

    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (loaded_tx, loaded_rx) = mpsc::channel();
    let second_paths = paths.clone();
    let second_config = config.clone();
    let second_profile = profile.clone();
    let second_writer = thread::spawn(move || {
        let mut archive =
            open_archive_connection(&second_paths, &second_config, None).expect("second archive");
        let transaction = archive.transaction().expect("second transaction");
        attempt_tx.send(()).expect("attempt signal");
        upsert_source_profile(&transaction, &second_profile).expect("second source profile");
        let loaded = load_watermark(&transaction, &second_profile.profile_id)
            .expect("second watermark after lock");
        loaded_tx.send(loaded.last_visit_id).expect("loaded watermark signal");
        transaction.commit().expect("second commit");
    });

    attempt_rx.recv_timeout(Duration::from_secs(2)).expect("second writer attempted");
    assert!(
        loaded_rx.recv_timeout(Duration::from_millis(150)).is_err(),
        "the second same-profile writer must not read the watermark before the first writer commits"
    );

    first_transaction.commit().expect("first commit");
    assert_eq!(
        loaded_rx.recv_timeout(Duration::from_secs(2)).expect("second loaded committed watermark"),
        41,
        "the second writer must observe the committed cursor from the first writer"
    );
    second_writer.join().expect("second writer thread");
}
