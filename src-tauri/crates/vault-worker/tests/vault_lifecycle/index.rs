//! Responsibilities: pin automatic search rebuild through a real owner reopen.
//! Not responsible for: adding a rebuild command or treating compressed index bytes as plaintext.

use super::support::*;

#[test]
fn reopening_without_the_index_rebuilds_the_saved_search_marker() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let before =
            observe(&fixture.root, Some(&fixture.vault()), &format!("index-before-{encrypted}"));
        assert_eq!(before["unknown"], serde_json::json!([]));
        assert_eq!(before, expected(&format!("backup-{encrypted}")));
        assert_eq!(search_titles(&fixture.vault()), ["pksearchelm"]);
        fixture.session.close_vault().unwrap();
        std::fs::remove_dir_all(fixture.root.join("derived/search-index")).unwrap();
        let rebuilt = fixture.reopen(encrypted);
        let after = observe(&fixture.root, Some(&rebuilt), &format!("index-after-{encrypted}"));
        assert_eq!(after["unknown"], serde_json::json!([]));
        assert_eq!(after, expected(&format!("backup-{encrypted}")));
        assert_eq!(after, before);
        assert_eq!(search_titles(&rebuilt), ["pksearchelm"]);
    }
}
