//! Serialization regression coverage for Core Intelligence DTOs.
//!
//! ## Responsibilities
//! - Guard enum tagging and field casing after DTO owner splits.
//! - Keep representative request aliases covered.
//! - Exercise external-output entity reference serde without touching runtime
//!   builders.
//!
//! ## Not responsible for
//! - Testing Core Intelligence SQL or rebuild behavior.
//! - Exhaustively serializing every DTO field.
//! - Validating frontend TypeScript bindings.
//!
//! ## Dependencies
//! - `serde_json` for explicit wire-shape assertions.
//!
//! ## Performance notes
//! - DTO serde tests are tiny and should remain cheap enough to run in every
//!   Rust gate.

use super::{
    CoreIntelligenceSectionWindow, DateRange, DomainDeepDiveRequest, InsightEntityReference,
};

#[test]
fn section_window_serializes_with_camel_case_variant_fields() {
    let window = CoreIntelligenceSectionWindow::DateRange {
        date_range: DateRange { start: "2026-04-01".to_string(), end: "2026-04-18".to_string() },
    };
    let serialized = serde_json::to_value(&window).expect("serialize date range window");
    assert_eq!(
        serialized,
        serde_json::json!({
            "kind": "date-range",
            "dateRange": {
                "start": "2026-04-01",
                "end": "2026-04-18"
            }
        })
    );
}

#[test]
fn calendar_day_history_window_serializes_with_camel_case_reference_date() {
    let window = CoreIntelligenceSectionWindow::CalendarDayHistory {
        reference_date: "2026-04-18".to_string(),
    };
    let serialized = serde_json::to_value(&window).expect("serialize calendar day history window");
    assert_eq!(
        serialized,
        serde_json::json!({
            "kind": "calendar-day-history",
            "referenceDate": "2026-04-18"
        })
    );
}

#[test]
fn domain_deep_dive_request_accepts_legacy_domain_alias() {
    let request: DomainDeepDiveRequest = serde_json::from_value(serde_json::json!({
        "domain": "example.com",
        "dateRange": {
            "start": "2026-04-01",
            "end": "2026-04-18"
        },
        "profileId": "chrome-default"
    }))
    .expect("deserialize legacy domain alias");

    assert_eq!(request.registrable_domain, "example.com");
    assert_eq!(request.profile_id.as_deref(), Some("chrome-default"));
}

#[test]
fn entity_reference_serializes_with_existing_camel_case_tags() {
    let reference = InsightEntityReference::RefindPage {
        canonical_url: "https://example.com/docs".to_string(),
    };
    let serialized = serde_json::to_value(&reference).expect("serialize entity reference");
    assert_eq!(
        serialized,
        serde_json::json!({
            "kind": "refindPage",
            "canonical_url": "https://example.com/docs"
        })
    );
}
