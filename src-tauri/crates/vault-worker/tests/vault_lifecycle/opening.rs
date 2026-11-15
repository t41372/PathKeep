//! Responsibilities: record first-open and backup files from the current owner.
//! Not responsible for: proving these were written by the released v0.5.0 executable.

use super::support::*;

#[test]
fn first_open_creates_the_current_seven_databases() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        let actual =
            observe(&fixture.root, Some(&fixture.vault()), &format!("first-open-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected(&format!("first-open-{encrypted}")));
    }
}

#[test]
fn backup_keeps_a_browser_checkpoint_and_all_distinct_store_markers() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let actual = observe(&fixture.root, Some(&fixture.vault()), &format!("backup-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected(&format!("backup-{encrypted}")));
    }
}
