//! Shared setup for ingest tests that drive the archive writers directly.
//!
//! ## Responsibilities
//! - Build a browser profile and a seeded run, source profile and source batch to write under.
//!
//! ## Not responsible for
//! - Browser-family scenario matrices owned by the dedup scenario modules.

use super::writes::upsert_source_profile;
use super::*;
use browser_history_parser::CapabilitySnapshot;
use vault_store::{Db, Priority, Session};

pub(super) fn profile(family: &str, profile_id: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: family.to_string(),
        browser_name: match family {
            "firefox" => "Firefox".to_string(),
            "safari" => "Safari".to_string(),
            _ => "Chrome".to_string(),
        },
        user_name: Some("tim@example.com".to_string()),
        profile_path: format!("/tmp/{profile_id}"),
        history_path: Some(format!("/tmp/{profile_id}/History")),
        favicons_path: Some(format!("/tmp/{profile_id}/Favicons")),
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("146.0.0.0".to_string()),
        history_file_name: "History".to_string(),
        history_bytes: 128,
        favicons_bytes: 64,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    }
}

fn seed_run(archive: &Session<'_>, run_id: i64) {
    archive
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (?1, 'backup', 'manual', '2026-04-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
                [run_id],
            )
            .expect("seed run");
}

pub(super) fn seed_context(
    vault: &crate::Vault,
    run: i64,
    profile: &crate::models::BrowserProfile,
) -> (WriteContext, i64) {
    let owned = profile.clone();
    let id = vault
        .write(Db::Archive, Priority::Batch, move |s| {
            seed_run(s, run);
            Ok(upsert_source_profile(s, &owned)?)
        })
        .unwrap();
    let input = SourceBatchInput {
        source_profile_id: id,
        run_id: Some(run),
        source_kind: "local_db".into(),
        browser_version: None,
        schema_version_text: None,
        schema_version_int: None,
        schema_fingerprint: "test".into(),
        capability_snapshot: CapabilitySnapshot::default(),
        coverage_stats_json: "{}".into(),
        artifact_refs_json: None,
        notes_json: None,
    };
    let batch = vault
        .write(Db::SourceEvidence, Priority::Batch, move |s| upsert_source_batch(s, &input))
        .unwrap();
    (
        WriteContext {
            run_id: run,
            source_profile_id: id,
            profile_id: profile.profile_id.clone(),
            browser_family: profile.browser_family.clone(),
            import: None,
        },
        batch,
    )
}
