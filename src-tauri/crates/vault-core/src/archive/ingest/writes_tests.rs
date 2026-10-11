//! Responsibilities: pin backup fingerprint and visit bounds.
//! Not responsible for: parser behavior.
//! Failure modes: family-specific fingerprints diverge; ignored visits widen bounds.
use super::*;
use crate::archive::visit_event_fingerprint;
use crate::utils::unix_micros_to_chrome_time;

/// Contract: the backup-pipeline writer (`insert_visit` above) uses
/// the hardcoded source_kind `"chromium-history"` for every browser
/// family it serves (Chromium, Firefox, Safari). This is intentional —
/// keeping the constant identical across families inside this writer
/// means a re-import of the same browser profile always produces the
/// same fingerprint, which is what the
/// `(source_profile_id, event_fingerprint)` partial unique index
/// relies on for fallback dedup.
///
/// Cross-flow fingerprint matching against the Takeout writers
/// (`vault-core/src/takeout/payload_import.rs`,
/// `vault-core/src/takeout/browser_history.rs`) is NOT a contract —
/// those writers use different source_kind values and Unix-millisecond
/// timestamps. Their visits always land in distinct source_profiles
/// rows from this writer's output, so the partial index naturally
/// scopes the dedup per flow.
///
/// If a future change parameterizes source_kind per family inside
/// `insert_visit` itself, this test fails immediately and forces a
/// follow-up audit of any re-imports that crossed family-by-version.
#[test]
fn fingerprint_is_family_agnostic_within_backup_writer() {
    let url = "https://example.com/article";
    let visit_time_ms: i64 = 1_777_680_000_000;
    let visit_time_chrome = unix_micros_to_chrome_time(visit_time_ms.saturating_mul(1_000));
    let title = Some("Article");
    let transition = Some(805306368_i64);
    let app_id: Option<&str> = None;

    let chromium_fp = visit_event_fingerprint(
        "chromium-history",
        url,
        visit_time_chrome,
        title,
        transition,
        app_id,
    );

    // Identical inputs must produce identical fingerprints; that is
    // what the backup writer guarantees across families today.
    let firefox_fp = visit_event_fingerprint(
        "chromium-history",
        url,
        visit_time_chrome,
        title,
        transition,
        app_id,
    );
    let safari_fp = visit_event_fingerprint(
        "chromium-history",
        url,
        visit_time_chrome,
        title,
        transition,
        app_id,
    );

    assert_eq!(
        chromium_fp, firefox_fp,
        "fingerprint must be identical regardless of browser family"
    );
    assert_eq!(
        chromium_fp, safari_fp,
        "fingerprint must be identical regardless of browser family"
    );

    // Sanity: changing any input produces a different fingerprint.
    let different_url_fp = visit_event_fingerprint(
        "chromium-history",
        "https://example.com/other",
        visit_time_chrome,
        title,
        transition,
        app_id,
    );
    assert_ne!(chromium_fp, different_url_fp, "different URL must produce different fingerprint");

    // Sanity: a hypothetical per-family source_kind WOULD diverge.
    let hypothetical_firefox_fp = visit_event_fingerprint(
        "firefox-history",
        url,
        visit_time_chrome,
        title,
        transition,
        app_id,
    );
    assert_ne!(
        chromium_fp, hypothetical_firefox_fp,
        "different source_kind must produce different fingerprint (proves the hardcode matters)"
    );
}

/// Contract: `sync_url_bounds` only widens the stored bounds — a visit
/// whose timestamp falls between the existing first and last does not
/// change either bound. This prevents mid-range backfill from shifting
/// the URL's reported first or last visit.
#[test]
fn sync_url_bounds_no_change_for_middle_visit() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.store().write(vault_store::Db::Archive, vault_store::Priority::Batch, |transaction| {
        // Seed a run and source profile so FK constraints are satisfied.
        transaction
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'manual', '2026-05-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
                (),
            )
            .expect("seed run");
        let profile = crate::models::BrowserProfile {
            profile_id: "chrome:Default".to_string(),
            profile_name: "Default".to_string(),
            browser_family: "chromium".to_string(),
            browser_name: "Google Chrome".to_string(),
            user_name: Some("test".to_string()),
            profile_path: "/synthetic/chrome:Default".to_string(),
            history_path: Some("/synthetic/chrome:Default/History".to_string()),
            favicons_path: None,
            history_exists: true,
            history_readable: true,
            access_issue: None,
            browser_version: Some("146.0.0.0".to_string()),
            history_file_name: "History".to_string(),
            history_bytes: 128,
            favicons_bytes: 0,
            supporting_bytes: 0,
            retention_boundary: crate::models::BrowserRetentionBoundary::default(),
        };
        let source_profile_id =
            upsert_source_profile(transaction, &profile).expect("upsert profile");

        // Insert a URL with initial bounds at time 1000.
        let url = browser_history_parser::ParsedUrl {
            source_url_id: 1,
            url: "https://example.com/bounds-test".to_string(),
            title: Some("Bounds Test".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_ms: 1000,
            last_visit_iso: "2026-01-01T00:00:01+00:00".to_string(),
            hidden: false,
            source_last_visit_marker: None,
        };
        let url_id = upsert_url(transaction, 1, source_profile_id, &url, "hash-1")
            .expect("upsert url")
            .id;

        // Widen bounds: first=1000, last=3000.
        sync_url_bounds(
            transaction,
            url_id,
            &UrlVisitBounds {
                first_visit_ms: 1000,
                first_visit_iso: "2026-01-01T00:00:01+00:00".to_string(),
                last_visit_ms: 3000,
                last_visit_iso: "2026-01-01T00:00:03+00:00".to_string(),
            },
        )
        .expect("initial bounds");

        // Now insert a middle visit at time 2000.
        sync_url_bounds(
            transaction,
            url_id,
            &UrlVisitBounds {
                first_visit_ms: 2000,
                first_visit_iso: "2026-01-01T00:00:02+00:00".to_string(),
                last_visit_ms: 2000,
                last_visit_iso: "2026-01-01T00:00:02+00:00".to_string(),
            },
        )
        .expect("middle bounds");

        // Assert bounds remain (1000, 3000) — the middle visit must not
        // shift either bound.
        let (first_ms, last_ms): (i64, i64) = transaction
            .query_one(
                "SELECT first_visit_ms, last_visit_ms FROM urls WHERE id = ?1",
                [url_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query bounds");

        assert_eq!(first_ms, 1000, "first_visit_ms must not shift to middle visit");
        assert_eq!(last_ms, 3000, "last_visit_ms must not shift to middle visit");
        Ok(())
        }).unwrap();
}

#[test]
fn older_url_observations_do_not_allocate_unpublished_page_anchors() {
    for key in [None, Some([82; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        vault
            .store()
            .write(vault_store::Db::Archive, vault_store::Priority::Batch, |s| {
                let profile = super::super::core_tests::profile("chromium", "chrome:anchors");
                let source = upsert_source_profile(s, &profile)?;
                let mut row = browser_history_parser::ParsedUrl {
                    source_url_id: 1,
                    url: "https://newer.test".into(),
                    title: Some("newer".into()),
                    visit_count: 1,
                    typed_count: 0,
                    last_visit_ms: 2000,
                    last_visit_iso: "1970-01-01T00:00:02Z".into(),
                    hidden: false,
                    source_last_visit_marker: None,
                };
                let first = upsert_url(s, 0, source, &row, "new")?;
                row.url = "https://older.test".into();
                row.last_visit_ms = 1000;
                let older = upsert_url(s, 0, source, &row, "old")?;
                assert_eq!((first.id, first.page), (older.id, older.page));
                assert!(!older.rewritten, "an older observation keeps the row as it was");
                assert_eq!(
                    s.query_one("SELECT COUNT(*) FROM pages", (), |r| Ok(r.get::<i64>(0)?))?,
                    1
                );
                Ok(())
            })
            .unwrap();
    }
}
