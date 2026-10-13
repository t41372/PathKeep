//! Responsibilities: preserve projection behavior while replacing FTS storage.
//! Not responsible for: SQLCipher schema versions or FTS tokenization internals.
//! Failure modes: another profile's title disappears; a revoked title leaks while the
//! outbox is pending; enrichment/notes/tags disappear during rebuild or clear.
use super::{
    SearchRequest, SearchSession,
    fixture::{Archive, CHROME, FIREFOX},
};
use vault_store::{Db, Priority};

#[test]
fn every_visible_title_is_searchable_and_revoked_text_never_leaks_from_a_shared_page() {
    for key in [None, Some([102; 32])] {
        let archive = Archive::with_key(key);
        archive
            .url(1, CHROME, "https://shared.test/page", "privatephrase", &[10])
            .url(2, FIREFOX, "https://shared.test/page", "survivor", &[20])
            .index();
        assert_eq!(archive.words("privatephrase").total, 1);
        let paused = archive.vault.search().pause();
        archive
            .vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                s.execute("UPDATE visits SET reverted_at='now' WHERE url_id=1", ())?;
                crate::archive::search_documents::refresh(s, 1)
            })
            .unwrap();
        assert_eq!(
            archive
                .vault
                .search()
                .index()
                .unwrap()
                .search(
                    vault_search::Ast::Prefix { token: "privatephrase".into(), field: None },
                    Default::default()
                )
                .unwrap()
                .count()
                .unwrap(),
            1
        );
        for start in [None, Some(0)] {
            let result = SearchSession::start(
                &archive.vault,
                SearchRequest {
                    query: "privatephrase".into(),
                    start_time_ms: start,
                    ..Default::default()
                },
            )
            .unwrap()
            .finish_page()
            .unwrap();
            assert_eq!(result.count.value, 0);
            assert!(result.items.is_empty());
            assert!(!result.coverage_complete);
        }
        drop(paused);
        archive.index();
        assert_eq!(archive.words("survivor").total, 1);
    }
}
#[test]
fn rebuild_and_clearing_preserve_the_enrichment_and_annotation_contract() {
    let archive = Archive::with_key(Some([103; 32]));
    archive.url(1, CHROME, "https://fields.test/page", "ordinary", &[1]).index();
    crate::archive::set_enrichment_text(&archive.vault, 1, "enrichedsentinel").unwrap();
    let after_enrichment = crate::test_support::search_sequence(&archive.vault);
    crate::archive::set_enrichment_text(&archive.vault, 1, "enrichedsentinel").unwrap();
    assert_eq!(
        crate::test_support::search_sequence(&archive.vault),
        after_enrichment,
        "unchanged enrichment must not enqueue another document replacement"
    );
    crate::set_notes(
        &archive.vault,
        crate::SetNotesRequest {
            url: "https://fields.test/page".into(),
            notes: "設計系統 notephrase".into(),
            source_profile: None,
        },
    )
    .unwrap();
    archive.tag("https://fields.test/page", "tagphrase");
    archive.index();
    for query in ["enrichedsentinel", "设计系统", "notephrase", "tagphrase"] {
        assert_eq!(archive.words(query).total, 1);
    }
    archive.vault.search().rebuild().unwrap();
    for query in ["enrichedsentinel", "设计系统", "notephrase", "tagphrase"] {
        assert_eq!(archive.words(query).total, 1);
    }
    crate::set_notes(
        &archive.vault,
        crate::SetNotesRequest {
            url: "https://fields.test/page".into(),
            notes: String::new(),
            source_profile: None,
        },
    )
    .unwrap();
    crate::replace_tags(
        &archive.vault,
        crate::ReplaceTagsRequest {
            url: "https://fields.test/page".into(),
            tags: vec![],
            source_profile: None,
        },
    )
    .unwrap();
    archive.index();
    assert_eq!(archive.words("notephrase").total, 0);
    assert_eq!(archive.words("tagphrase").total, 0);
    assert_eq!(archive.words("enrichedsentinel").total, 1);
}

#[test]
fn a_live_session_drops_revoked_text_when_hydrating() {
    let archive = Archive::with_key(Some([105; 32]));
    archive
        .url(1, CHROME, "https://shared.test/page", "privatephrase", &[10])
        .url(2, FIREFOX, "https://shared.test/page", "survivor", &[20])
        .index();
    let mut session = SearchSession::start(
        &archive.vault,
        SearchRequest { query: "privatephrase".into(), ..Default::default() },
    )
    .unwrap();
    assert_eq!(session.finish_page().unwrap().count.value, 1);
    let _paused = archive.vault.search().pause();
    archive
        .vault
        .store()
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute("UPDATE visits SET reverted_at='now' WHERE url_id=1", ())?;
            crate::archive::search_documents::refresh(s, 1)
        })
        .unwrap();
    // The page keeps a visible visit, but no longer has the revoked text.
    let after = session.finish_page().unwrap();
    assert!(after.items.is_empty());
    // Counts belong to the session's snapshot; a new search counts the revoke.
    drop(_paused);
    let mut fresh = SearchSession::start(
        &archive.vault,
        SearchRequest { query: "privatephrase".into(), ..Default::default() },
    )
    .unwrap();
    let fresh = fresh.finish_page().unwrap();
    assert!(fresh.items.is_empty());
    assert_eq!((fresh.count.value, fresh.count.exact), (0, true));
}
