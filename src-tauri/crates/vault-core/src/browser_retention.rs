use crate::models::BrowserRetentionBoundary;

const SAFARI_LOCAL_HISTORY_DAYS: u32 = 365;

pub(crate) fn retention_boundary_for_browser(browser_family: &str) -> BrowserRetentionBoundary {
    match browser_family {
        "safari" => BrowserRetentionBoundary {
            kind: "macos-safari".to_string(),
            local_days: Some(SAFARI_LOCAL_HISTORY_DAYS),
        },
        _ => BrowserRetentionBoundary { kind: "browser-managed".to_string(), local_days: None },
    }
}

#[cfg(test)]
mod tests {
    use super::retention_boundary_for_browser;

    #[test]
    fn marks_safari_as_macos_year_scale_retention() {
        let boundary = retention_boundary_for_browser("safari");
        assert_eq!(boundary.kind, "macos-safari");
        assert_eq!(boundary.local_days, Some(365));
    }

    #[test]
    fn defaults_other_families_to_browser_managed_retention() {
        for family in ["chromium", "firefox", "unknown"] {
            let boundary = retention_boundary_for_browser(family);
            assert_eq!(boundary.kind, "browser-managed");
            assert_eq!(boundary.local_days, None);
        }
    }
}
