//! Browser-specific retention-boundary defaults.
//!
//! PathKeep preserves honesty about what the browser may already evict locally.
//! This module centralizes those browser-family heuristics so discovery and the
//! UI can explain retention expectations consistently.

use crate::models::BrowserRetentionBoundary;

const SAFARI_LOCAL_HISTORY_DAYS: u32 = 365;

/// Returns the browser-managed local-retention boundary for one browser family.
pub(crate) fn retention_boundary_for_browser(browser_family: &str) -> BrowserRetentionBoundary {
    match browser_family {
        "safari" => BrowserRetentionBoundary {
            kind: "macos-safari".to_string(),
            local_days: Some(SAFARI_LOCAL_HISTORY_DAYS),
        },
        _ => BrowserRetentionBoundary { kind: "browser-managed".to_string(), local_days: None },
    }
}
