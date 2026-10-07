//! Responsibilities: retain the timeline grouping regression during search extraction.
//! Not responsible for: search ranking or database ownership.
//! Failure mode: a search grouping flag accidentally groups ordinary visits.
use super::*;
#[test]
fn browsing_without_text_still_lists_every_visit() {
    for key in [None, Some([76; 32])] {
        let vault = export::regression_tests::seeded_archive(2, key);
        let response =
            list_history(&vault, HistoryQuery { group_by_url: Some(true), ..Default::default() })
                .unwrap();
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.total_visits, None);
        assert!(response.items.iter().all(|row| row.visit_count.is_none()));
    }
}
