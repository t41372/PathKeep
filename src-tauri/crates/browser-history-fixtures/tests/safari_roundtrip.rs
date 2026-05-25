//! Self-validation for the Safari `History.db` fixture writer.
//!
//! Covers both the minimal and current macOS Safari schema variants. The
//! current variant exercises the parser's optional-column probing path
//! (`load_successful`, `synthesized`, `redirect_*`, `score`).

use browser_history_fixtures::{
    SafariHistoryFixture, SafariHistoryItemRow, SafariHistoryVisitRow, SafariSchemaVariant,
    safari_time_to_unix_ms, unix_ms_to_safari_time,
};
use browser_history_parser::safari;
use tempfile::TempDir;

#[test]
fn safari_minimal_fixture_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("History.db");

    let visit_one_ms = 1_777_680_000_000;
    let visit_two_ms = 1_777_809_600_000;

    SafariHistoryFixture::new()
        .with_variant(SafariSchemaVariant::Minimal)
        .add_item(SafariHistoryItemRow { id: 5, url: "https://example.com/safari".to_string() })
        .add_visit(SafariHistoryVisitRow {
            id: 9,
            history_item: 5,
            title: Some("Safari Example One".to_string()),
            visit_time_unix_ms: visit_one_ms,
            load_successful: None,
            http_non_get: None,
            synthesized: None,
            redirect_source: None,
            redirect_destination: None,
            origin: None,
            generation: None,
            attributes: None,
            score: None,
        })
        .add_visit(SafariHistoryVisitRow {
            id: 10,
            history_item: 5,
            title: Some("Safari Example Two".to_string()),
            visit_time_unix_ms: visit_two_ms,
            load_successful: None,
            http_non_get: None,
            synthesized: None,
            redirect_source: None,
            redirect_destination: None,
            origin: None,
            generation: None,
            attributes: None,
            score: None,
        })
        .write(&history_path)
        .expect("write minimal safari fixture");

    let parsed = safari::parse_history(&history_path, 0, 0).expect("parse minimal safari fixture");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 2);

    let url = &parsed.urls[0];
    assert_eq!(url.url, "https://example.com/safari");
    assert_eq!(url.visit_count, 2);
    assert_eq!(url.last_visit_ms, visit_two_ms);

    let visit_nine =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 9).expect("visit 9");
    assert_eq!(visit_nine.visit_time_ms, visit_one_ms);
    assert_eq!(visit_nine.title.as_deref(), Some("Safari Example One"));
    assert_eq!(visit_nine.app_id.as_deref(), Some("safari"));
}

#[test]
fn safari_current_fixture_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("History.db");

    let visit_one_ms = 1_777_680_000_000;

    SafariHistoryFixture::new()
        .with_variant(SafariSchemaVariant::Current)
        .add_item(SafariHistoryItemRow {
            id: 5,
            url: "https://example.com/safari-current".to_string(),
        })
        .add_visit(SafariHistoryVisitRow {
            id: 9,
            history_item: 5,
            title: Some("Safari Current Schema".to_string()),
            visit_time_unix_ms: visit_one_ms,
            load_successful: Some(true),
            http_non_get: Some(false),
            synthesized: Some(false),
            redirect_source: None,
            redirect_destination: Some(10),
            origin: Some(1),
            generation: Some(2),
            attributes: Some(4),
            score: Some(0.75),
        })
        .write(&history_path)
        .expect("write current safari fixture");

    let parsed = safari::parse_history(&history_path, 0, 0).expect("parse current safari fixture");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.urls[0].url, "https://example.com/safari-current");
    assert_eq!(parsed.visits[0].visit_time_ms, visit_one_ms);
    assert_eq!(parsed.visits[0].title.as_deref(), Some("Safari Current Schema"));
    assert_eq!(parsed.visits[0].source_url_id, 5);
    assert_eq!(parsed.visits[0].source_visit_id, 9);
    assert_eq!(parsed.visits[0].app_id.as_deref(), Some("safari"));

    // Safari parser hardcodes these fields for visits — confirm the contract.
    assert_eq!(parsed.visits[0].from_visit, None);
    assert_eq!(parsed.visits[0].transition, None);
    assert_eq!(parsed.visits[0].visit_duration_ms, None);
    assert!(!parsed.visits[0].is_known_to_sync);
    assert_eq!(parsed.visits[0].visited_link_id, None);
    assert_eq!(parsed.visits[0].external_referrer_url, None);

    // Safari URL row: typed_count is hardcoded to 0, hidden to false.
    assert_eq!(parsed.urls[0].typed_count, 0);
    assert!(!parsed.urls[0].hidden);
    assert_eq!(parsed.urls[0].visit_count, 1);
    assert_eq!(parsed.urls[0].last_visit_ms, visit_one_ms);

    // --- Extra columns surface through typed_evidence, not ParsedVisit ---

    // load_successful=true → ContextEvidence with value "true"
    assert!(
        parsed.typed_evidence.context.iter().any(|ctx| {
            ctx.context_key == "safari.load_successful"
                && ctx.value_json == "true"
                && ctx.source_visit_id == Some(9)
        }),
        "load_successful=true should produce context evidence"
    );

    // http_non_get=false → ContextEvidence with value "false"
    assert!(
        parsed.typed_evidence.context.iter().any(|ctx| {
            ctx.context_key == "safari.http_non_get"
                && ctx.value_json == "false"
                && ctx.source_visit_id == Some(9)
        }),
        "http_non_get=false should produce context evidence"
    );

    // synthesized=false → ContextEvidence with value "false"
    assert!(
        parsed.typed_evidence.context.iter().any(|ctx| {
            ctx.context_key == "safari.synthesized"
                && ctx.value_json == "false"
                && ctx.source_visit_id == Some(9)
        }),
        "synthesized=false should produce context evidence"
    );

    // redirect_destination=10 → NavigationEvidence with edge_kind
    // "safari.redirect_destination" and target_visit_id=10
    assert!(
        parsed.typed_evidence.navigation.iter().any(|nav| {
            nav.edge_kind == "safari.redirect_destination"
                && nav.target_visit_id == Some(10)
                && nav.source_visit_id == 9
        }),
        "redirect_destination=10 should produce navigation evidence"
    );

    // redirect_source=None → no NavigationEvidence for redirect_source
    // (the parser only emits evidence when the value is Some)
    assert!(
        !parsed
            .typed_evidence
            .navigation
            .iter()
            .any(|nav| { nav.edge_kind == "safari.redirect_source" && nav.source_visit_id == 9 }),
        "redirect_source=None should not produce navigation evidence"
    );

    // origin=1 → ContextEvidence with value "1"
    assert!(
        parsed.typed_evidence.context.iter().any(|ctx| {
            ctx.context_key == "safari.origin"
                && ctx.value_json == "1"
                && ctx.source_visit_id == Some(9)
        }),
        "origin=1 should produce context evidence"
    );

    // generation=2 → ContextEvidence with value "2"
    assert!(
        parsed.typed_evidence.context.iter().any(|ctx| {
            ctx.context_key == "safari.generation"
                && ctx.value_json == "2"
                && ctx.source_visit_id == Some(9)
        }),
        "generation=2 should produce context evidence"
    );

    // attributes=4 → ContextEvidence with value "4"
    assert!(
        parsed.typed_evidence.context.iter().any(|ctx| {
            ctx.context_key == "safari.attributes"
                && ctx.value_json == "4"
                && ctx.source_visit_id == Some(9)
        }),
        "attributes=4 should produce context evidence"
    );

    // score=0.75 → EngagementEvidence with metric_key "safari.score"
    assert!(
        parsed.typed_evidence.engagement.iter().any(|eng| {
            eng.metric_key == "safari.score"
                && eng.metric_value_real == Some(0.75)
                && eng.source_visit_id == 9
        }),
        "score=0.75 should produce engagement evidence"
    );
}

#[test]
fn safari_visit_before_cocoa_epoch_is_clamped_to_zero() {
    // safari_time_to_unix_ms applies `.max(0)` to the final Unix-ms result.
    // A CFAbsoluteTime far enough before the Cocoa epoch (2001-01-01) that
    // the computed Unix ms is negative gets clamped to 0. This is lossy —
    // the original timestamp is not recoverable.
    //
    // The parser's URL watermark also uses Cocoa time, so a full integration
    // test can't reach this path (the URL is filtered out before the time
    // conversion runs). We test the conversion function directly.

    // -979_000_000.0 seconds from 2001-01-01 ≈ 1969-12-25.
    // Without clamping: (-979_000_000 + 978_307_200) * 1000 = -692_800_000 ms.
    let pre_unix = safari_time_to_unix_ms(-979_000_000.0);
    assert_eq!(pre_unix, 0, "pre-Unix-epoch Cocoa time must clamp to 0");

    // Just barely before 1970: offset is 978_307_200, so -978_307_201 gives
    // (−978_307_201 + 978_307_200) × 1000 = −1000 → clamped to 0.
    let barely_pre = safari_time_to_unix_ms(-978_307_201.0);
    assert_eq!(barely_pre, 0, "barely-pre-Unix-epoch must also clamp");

    // Exactly at Unix epoch: (−978_307_200 + 978_307_200) × 1000 = 0.
    let at_unix = safari_time_to_unix_ms(-978_307_200.0);
    assert_eq!(at_unix, 0, "Cocoa time mapping to Unix epoch is 0");

    // Just after 1970: positive result, no clamping.
    let post_unix = safari_time_to_unix_ms(-978_307_199.0);
    assert_eq!(post_unix, 1000, "one second after Unix epoch = 1000 ms");
}

#[test]
fn safari_time_helpers_match_production_offset() {
    let unix_ms = 1_777_809_600_000;
    let safari = unix_ms_to_safari_time(unix_ms);
    let back = safari_time_to_unix_ms(safari);
    assert_eq!(back, unix_ms);

    // Unix epoch zero maps to a negative CFAbsoluteTime since the Cocoa
    // epoch is in 2001. Production helpers clamp negatives back to zero on
    // the inverse path, so the pinning here is one-way.
    let cocoa_epoch_unix_ms = 978_307_200_000;
    assert!((unix_ms_to_safari_time(cocoa_epoch_unix_ms)).abs() < 0.001);
}
