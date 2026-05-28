//! Minor column-level ingest contract tests.
//!
//! ## Responsibilities
//! - Pin visit counts, dangling visit references, duration values, Safari context evidence, and Firefox visit type passthrough.
//! - Keep narrow data-integrity assertions close to the import harness.
//!
//! ## Not responsible for
//! - Broad dedup scenario coverage owned by the main scenario modules.
//! - Product-code fixes for behavior these tests pin.
//!
//! ## Dependencies
//! - Uses shared edge-case fixture helpers from the parent module.
//! - Uses Chromium, Safari, and Firefox synthetic fixtures.
//!
//! ## Performance notes
//! - Each test imports a tiny focused fixture; no large history corpus is generated.

use super::*;

// ======================================================================
// E10-E14 — Minor data-integrity contract pins
// ======================================================================

#[test]
fn e10_chromium_visit_counts_round_trip_for_zero_and_nonzero_urls() {
    let env = ScenarioEnv::new();
    let visited_ms = 1_777_680_000_000_i64;
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/typed-never-visited".to_string(),
            title: Some("Typed Never Visited".to_string()),
            visit_count: 0,
            typed_count: 1,
            last_visit_unix_ms: 0,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/frequently-visited".to_string(),
            title: Some("Frequently Visited".to_string()),
            visit_count: 7,
            typed_count: 2,
            last_visit_unix_ms: visited_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(20, 2, visited_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:VisitCounts", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    assert_eq!(summary.new_urls, 2);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let counts = archive
        .prepare(
            "SELECT source_url_id, visit_count, typed_count FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:VisitCounts'
             ORDER BY source_url_id",
        )
        .expect("prepare count query")
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        })
        .expect("query count rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect count rows");
    assert_eq!(counts, vec![(1, 0, 1), (2, 7, 2)]);
}

#[test]
fn e11_chromium_dangling_from_visit_is_preserved_verbatim() {
    let env = ScenarioEnv::new();
    let visit_ms = 1_777_680_000_000_i64;
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/dangling-referrer".to_string(),
            title: Some("Dangling Referrer".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_ms,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            from_visit: Some(999),
            ..chromium_visit_row(11, 1, visit_ms)
        });

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Dangling", "Google Chrome"),
    );
    run_one_ingest(&env, 1, &snapshot, false);

    let archive = env.open_archive();
    let from_visit: Option<i64> = archive
        .query_row(
            "SELECT from_visit FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Dangling'
               AND visits.source_visit_id = '11'",
            [],
            |row| row.get(0),
        )
        .expect("query dangling from_visit");
    assert_eq!(from_visit, Some(999), "dangling parent visit ids should not be rewritten");
}

#[test]
fn e12_chromium_visit_duration_value_is_preserved_verbatim() {
    let env = ScenarioEnv::new();
    let visit_ms = 1_777_680_000_000_i64;
    let duration_value = 6_543_210_i64;
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/duration".to_string(),
            title: Some("Duration".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_ms,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            visit_duration_micros: Some(duration_value),
            ..chromium_visit_row(12, 1, visit_ms)
        });

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Duration", "Google Chrome"),
    );
    run_one_ingest(&env, 1, &snapshot, false);

    let archive = env.open_archive();
    let stored_duration: Option<i64> = archive
        .query_row(
            "SELECT visit_duration_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Duration'
               AND visits.source_visit_id = '12'",
            [],
            |row| row.get(0),
        )
        .expect("query visit duration");
    assert_eq!(stored_duration, Some(duration_value));
}

#[test]
fn e13_safari_synthesized_context_evidence_persists_boolean_value() {
    let env = ScenarioEnv::new();
    let visit_ms = 1_777_680_000_000_i64;
    let fixture = SafariHistoryFixture::new()
        .add_item(SafariHistoryItemRow {
            id: 7,
            url: "https://example.com/safari-synthesized".to_string(),
        })
        .add_visit(SafariHistoryVisitRow {
            id: 70,
            history_item: 7,
            title: Some("Safari Synthesized".to_string()),
            visit_time_unix_ms: visit_ms,
            load_successful: Some(true),
            http_non_get: Some(false),
            synthesized: Some(true),
            redirect_source: None,
            redirect_destination: None,
            origin: None,
            generation: None,
            attributes: None,
            score: None,
        });
    let snapshot = safari_snapshot(&fixture, "safari:Synthesized");
    let summary = run_one_ingest_with_persisted_source_evidence(&env, 1, &snapshot);

    assert_eq!(summary.new_visits, 1);
    let evidence = open_source_evidence_connection(&env.paths, &env.config, None)
        .expect("open source evidence");
    let row: (Option<String>, Option<String>, String, String) = evidence
        .query_row(
            "SELECT source_visit_id, source_url_id, value_json, source_field
             FROM visit_context_evidence
             WHERE context_key = 'safari.synthesized'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("query safari synthesized context evidence");
    assert_eq!(
        row,
        (
            Some("70".to_string()),
            Some("7".to_string()),
            "true".to_string(),
            "history_visits.synthesized".to_string()
        )
    );
}

#[test]
fn e14_firefox_visit_type_enum_lands_as_transition_type_without_normalization() {
    let env = ScenarioEnv::new();
    let first_ms = 1_777_680_000_000_i64;
    let second_ms = 1_777_680_010_000_i64;
    let fixture = FirefoxPlacesFixture::new()
        .add_place(FirefoxPlaceRow {
            id: 1,
            url: "https://example.com/firefox-link".to_string(),
            title: Some("Firefox Link".to_string()),
            visit_count: 1,
            hidden: false,
            last_visit_unix_ms: first_ms,
        })
        .add_place(FirefoxPlaceRow {
            id: 2,
            url: "https://example.com/firefox-typed".to_string(),
            title: Some("Firefox Typed".to_string()),
            visit_count: 1,
            hidden: false,
            last_visit_unix_ms: second_ms,
        })
        .add_visit(FirefoxVisitRow {
            id: 101,
            place_id: 1,
            visit_time_unix_ms: first_ms,
            from_visit: None,
            visit_type: Some(4),
        })
        .add_visit(FirefoxVisitRow {
            id: 102,
            place_id: 2,
            visit_time_unix_ms: second_ms,
            from_visit: Some(101),
            visit_type: Some(7),
        });
    let snapshot = firefox_snapshot(&fixture, "firefox:VisitType");
    run_one_ingest(&env, 1, &snapshot, false);

    let archive = env.open_archive();
    let transition_types = archive
        .prepare(
            "SELECT source_visit_id, transition_type FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'firefox:VisitType'
             ORDER BY source_visit_id",
        )
        .expect("prepare firefox visit_type query")
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)))
        .expect("query firefox visit_type rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect firefox visit_type rows");
    assert_eq!(transition_types, vec![("101".to_string(), Some(4)), ("102".to_string(), Some(7))]);
}
