//! Responsibilities: retain annotation search-text coverage on canonical Turso documents.
//! Not responsible for: index consumption, Tantivy ranking or legacy FTS query semantics.
//! Failure modes: note/tag text omitted, CJK grams stale, edits retain old words,
//! one edit publishes duplicate outbox rows. Original note/tag regressions run here.
use super::*;
const SEEDED_URL: &str = "https://github.com/o/r";
fn search_fixture(vault: &crate::Vault) {
    vault
        .store()
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,?1,?1)", [SEEDED_URL])?;
            crate::archive::search_documents::text_changed(s, 1)?;
            s.execute("DELETE FROM search_outbox", ())?;
            Ok(())
        })
        .unwrap();
}
fn document_contains(vault: &crate::Vault, text: &str) -> bool {
    let text = text.to_owned();
    vault
        .store()
        .read(Db::Archive, move |s| {
            let (notes, tags, compact) = s.query_one(
                "SELECT notes_text,tags_text,compact_text FROM search_documents WHERE page_id=1",
                (),
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
            )?;
            assert!(
                s.query_one("SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='search_outbox'),0)", (), |r| Ok(
                    r.get::<i64>(0)?
                ))? > 0
            );
            Ok((notes.contains(&text) || tags.contains(&text)) && compact.contains(&text))
        })
        .unwrap()
}

#[test]
fn note_is_findable_by_plain_keyword_search_in_search_document_and_outbox() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    set_notes(
        &vault,
        SetNotesRequest {
            url: SEEDED_URL.into(),
            notes: "wasmtime sandbox".into(),
            source_profile: None,
        },
    )
    .expect("set_notes");
    assert!(
        document_contains(&vault, "wasmtime"),
        "a saved note must be findable by a plain keyword search"
    );
}
#[test]
fn tag_is_findable_by_plain_keyword_search_in_search_document_and_outbox() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    replace_tags(
        &vault,
        ReplaceTagsRequest {
            url: SEEDED_URL.into(),
            tags: vec!["kubernetes".into()],
            source_profile: None,
        },
    )
    .expect("replace_tags");
    assert!(
        document_contains(&vault, "kubernetes"),
        "a saved tag must be findable by a plain keyword search"
    );
}
#[test]
fn edited_note_is_findable_and_old_word_drops_via_refresh_hook() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    set_notes(
        &vault,
        SetNotesRequest {
            url: SEEDED_URL.into(),
            notes: "wasmtime sandbox".into(),
            source_profile: None,
        },
    )
    .expect("set first note");
    assert!(document_contains(&vault, "wasmtime"));

    set_notes(
        &vault,
        SetNotesRequest {
            url: SEEDED_URL.into(),
            notes: "freshword runtime".into(),
            source_profile: None,
        },
    )
    .expect("edit note");
    assert!(document_contains(&vault, "freshword"), "the edited note's new word must be findable");
    assert!(
        !document_contains(&vault, "wasmtime"),
        "the replaced note's old word must no longer match"
    );
}
#[test]
fn note_is_published_once_per_page() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    let before = crate::test_support::search_sequence(&vault);
    set_notes(
        &vault,
        SetNotesRequest {
            url: SEEDED_URL.into(),
            notes: "wasmtime sandbox".into(),
            source_profile: None,
        },
    )
    .expect("set_notes");
    assert!(document_contains(&vault, "wasmtime"));
    assert_eq!(crate::test_support::search_sequence(&vault), before + 1);
}

#[test]
fn cjk_note_is_findable_by_plain_keyword_search_in_search_document_and_outbox() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    set_notes(
        &vault,
        SetNotesRequest {
            url: SEEDED_URL.into(),
            notes: "meta 的设计系统".into(),
            source_profile: None,
        },
    )
    .expect("set_notes");
    assert!(
        document_contains(&vault, "设计系统"),
        "a CJK note must be findable by a plain CJK substring search"
    );
}
#[test]
fn cjk_tag_is_findable_by_plain_keyword_search_in_search_document_and_outbox() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    replace_tags(
        &vault,
        ReplaceTagsRequest {
            url: SEEDED_URL.into(),
            tags: vec!["参考资料".into()],
            source_profile: None,
        },
    )
    .expect("replace_tags");
    assert!(
        document_contains(&vault, "参考资料"),
        "a CJK tag must be findable by a plain CJK keyword search"
    );
}
#[test]
fn edited_cjk_note_is_findable_and_old_term_drops_via_refresh_hook() {
    let vault = crate::Vault::open_temp().unwrap();
    search_fixture(&vault);
    set_notes(
        &vault,
        SetNotesRequest { url: SEEDED_URL.into(), notes: "图书馆".into(), source_profile: None },
    )
    .expect("set first CJK note");
    assert!(document_contains(&vault, "图书馆"));

    set_notes(
        &vault,
        SetNotesRequest {
            url: SEEDED_URL.into(), notes: "天气预报".into(), source_profile: None
        },
    )
    .expect("edit CJK note");
    assert!(
        document_contains(&vault, "天气预报"),
        "the edited CJK note's new term must be findable"
    );
    assert!(
        !document_contains(&vault, "图书馆"),
        "the replaced CJK note's old term must no longer match"
    );
}

#[test]
fn search_treats_percent_underscore_and_backslash_as_literal_text() {
    let vault = crate::Vault::open_temp().unwrap();
    for (url, notes) in [("https://a.test", "100% _done\\"), ("https://b.test", "1000 undone")] {
        crate::set_notes(
            &vault,
            crate::SetNotesRequest { url: url.into(), notes: notes.into(), source_profile: None },
        )
        .unwrap();
    }
    for query in ["%", "_", "\\"] {
        let page = crate::search_annotations(&vault, query, None, None).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].url, "https://a.test");
    }
}
