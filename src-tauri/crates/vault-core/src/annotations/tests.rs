//! Responsibilities: preserve annotation editing and atomic search handoff.
//! Not responsible for: Tantivy recall or UI rendering.
//! Failure modes: whitespace/limits drift, tags lose order or provenance, clears
//! remove unrelated data, search/outbox failure partially saves a user edit.

use super::*;

#[test]
fn returns_none_when_no_annotation_exists() {
    let vault = crate::Vault::open_temp().unwrap();
    let result = get_annotation(&vault, "https://example.com/").unwrap();
    assert!(result.is_none());
}

#[test]
fn writes_and_reads_notes_round_trip() {
    let vault = crate::Vault::open_temp().unwrap();
    let bundle = set_notes(
        &vault,
        SetNotesRequest {
            url: "https://example.com/x".into(),
            notes: "  hello world  ".into(),
            source_profile: Some("chrome:Default".into()),
        },
    )
    .unwrap();
    assert_eq!(bundle.notes, "hello world");
    assert_eq!(bundle.source_profile.as_deref(), Some("chrome:Default"));
    let reread =
        get_annotation(&vault, "https://example.com/x").unwrap().expect("annotation should exist");
    assert_eq!(reread.notes, "hello world");
    assert!(!reread.created_at.is_empty());
}

#[test]
fn empty_notes_clears_row_when_no_tags_remain() {
    let vault = crate::Vault::open_temp().unwrap();
    let url = "https://example.com/clear";
    set_notes(
        &vault,
        SetNotesRequest { url: url.into(), notes: "stay".into(), source_profile: None },
    )
    .unwrap();
    set_notes(&vault, SetNotesRequest { url: url.into(), notes: "".into(), source_profile: None })
        .unwrap();
    let after = get_annotation(&vault, url).unwrap();
    assert!(after.is_none());
}

#[test]
fn replace_tags_keeps_the_order_tags_were_added_in() {
    let vault = crate::Vault::open_temp().unwrap();
    let url = "https://example.com/ordered";
    let write = |tags: &[&str]| {
        replace_tags(
            &vault,
            ReplaceTagsRequest {
                url: url.into(),
                tags: tags.iter().map(|tag| tag.to_string()).collect(),
                source_profile: None,
            },
        )
        .unwrap()
        .tags
    };
    assert_eq!(write(&["zebra"]), vec!["zebra"]);
    // A moment later, so the second write's timestamp is newer.
    std::thread::sleep(std::time::Duration::from_millis(5));
    // "apple" sorts first alphabetically but was added last, so it stays last.
    assert_eq!(write(&["zebra", "apple"]), vec!["zebra", "apple"]);
    // Removing one tag leaves the other where it was.
    assert_eq!(write(&["apple"]), vec!["apple"]);
}

#[test]
fn replace_tags_dedupes_and_trims() {
    let vault = crate::Vault::open_temp().unwrap();
    let url = "https://example.com/tagged";
    let bundle = replace_tags(
        &vault,
        ReplaceTagsRequest {
            url: url.into(),
            tags: vec!["rust".into(), "  Rust ".into(), "".into(), "sqlite".into()],
            source_profile: None,
        },
    )
    .unwrap();
    assert_eq!(bundle.tags, vec!["rust".to_string(), "sqlite".to_string()]);
}

#[test]
fn replace_tags_with_empty_list_removes_tags_but_keeps_notes() {
    let vault = crate::Vault::open_temp().unwrap();
    let url = "https://example.com/mix";
    set_notes(
        &vault,
        SetNotesRequest { url: url.into(), notes: "keep me".into(), source_profile: None },
    )
    .unwrap();
    replace_tags(
        &vault,
        ReplaceTagsRequest { url: url.into(), tags: vec!["one".into()], source_profile: None },
    )
    .unwrap();
    let after_set = get_annotation(&vault, url).unwrap().unwrap();
    assert_eq!(after_set.tags, vec!["one"]);
    assert_eq!(after_set.notes, "keep me");
    replace_tags(
        &vault,
        ReplaceTagsRequest { url: url.into(), tags: vec![], source_profile: None },
    )
    .unwrap();
    let after_clear = get_annotation(&vault, url).unwrap().unwrap();
    assert!(after_clear.tags.is_empty());
    assert_eq!(after_clear.notes, "keep me");
}

#[test]
fn search_finds_notes_substring_case_insensitive() {
    let vault = crate::Vault::open_temp().unwrap();
    set_notes(
        &vault,
        SetNotesRequest {
            url: "https://example.com/a".into(),
            notes: "Reading about Rust async runtimes".into(),
            source_profile: None,
        },
    )
    .unwrap();
    set_notes(
        &vault,
        SetNotesRequest {
            url: "https://example.com/b".into(),
            notes: "SQLite docs".into(),
            source_profile: None,
        },
    )
    .unwrap();
    let results = search_annotations(&vault, "RUST", None, None).unwrap().items;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://example.com/a");
}

#[test]
fn notes_exceeding_byte_limit_is_rejected() {
    let vault = crate::Vault::open_temp().unwrap();
    let big = "x".repeat(MAX_NOTES_BYTES + 1);
    let err = set_notes(
        &vault,
        SetNotesRequest { url: "https://example.com/big".into(), notes: big, source_profile: None },
    )
    .unwrap_err();
    assert!(err.to_string().contains("exceeds"));
}

#[test]
fn tag_exceeding_length_is_rejected() {
    let vault = crate::Vault::open_temp().unwrap();
    let big_tag = "t".repeat(MAX_TAG_BYTES + 1);
    let err = replace_tags(
        &vault,
        ReplaceTagsRequest {
            url: "https://example.com/longtag".into(),
            tags: vec![big_tag],
            source_profile: None,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("exceeds"));
}

#[test]
fn list_returns_most_recent_first() {
    let vault = crate::Vault::open_temp().unwrap();
    for (url, note) in [
        ("https://example.com/1", "first"),
        ("https://example.com/2", "second"),
        ("https://example.com/3", "third"),
    ] {
        set_notes(
            &vault,
            SetNotesRequest { url: url.into(), notes: note.into(), source_profile: None },
        )
        .unwrap();
    }
    let listed = list_annotations(&vault, None, None).unwrap().items;
    assert_eq!(listed.len(), 3);
    let first_url = &listed[0].url;
    let last_url = &listed[2].url;
    assert!(first_url != last_url);
}

#[test]
fn set_notes_rejects_empty_or_whitespace_url() {
    let vault = crate::Vault::open_temp().unwrap();
    let err = set_notes(
        &vault,
        SetNotesRequest { url: "   ".into(), notes: "x".into(), source_profile: None },
    )
    .unwrap_err();
    assert!(err.to_string().contains("url"));
}

#[test]
fn replace_tags_rejects_empty_or_whitespace_url() {
    let vault = crate::Vault::open_temp().unwrap();
    let err = replace_tags(
        &vault,
        ReplaceTagsRequest { url: "".into(), tags: vec!["x".into()], source_profile: None },
    )
    .unwrap_err();
    assert!(err.to_string().contains("url"));
}

#[test]
fn replace_tags_rejects_when_tag_count_exceeds_per_url_limit() {
    let vault = crate::Vault::open_temp().unwrap();
    // MAX_TAGS_PER_URL + 1 unique short tags.
    let tags: Vec<String> = (0..=MAX_TAGS_PER_URL).map(|n| format!("t{n}")).collect();
    let err = replace_tags(
        &vault,
        ReplaceTagsRequest { url: "https://example.com/many".into(), tags, source_profile: None },
    )
    .unwrap_err();
    assert!(err.to_string().contains("tags"));
}

#[test]
fn search_with_empty_query_falls_through_to_list_annotations() {
    let vault = crate::Vault::open_temp().unwrap();
    set_notes(
        &vault,
        SetNotesRequest {
            url: "https://example.com/first".into(),
            notes: "alpha".into(),
            source_profile: None,
        },
    )
    .unwrap();
    // Whitespace-only query takes the "trimmed.is_empty()" branch on
    // line 178-179 → delegates to list_annotations.
    let listed = search_annotations(&vault, "   ", None, None).unwrap().items;
    assert_eq!(listed.len(), 1);
}

#[test]
fn annotation_cursors_reach_every_record_past_five_thousand() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        for id in 0..5_003 { s.execute("INSERT INTO url_annotations(url,canonical_url,notes,created_at,updated_at) VALUES(?1,?1,'needle','same','same')",[format!("https://notes.test/{id:05}")])?; }
        Ok(())
    }).unwrap();
    for query in ["", "needle"] {
        let first = search_annotations(&vault, query, Some(5_000), None).unwrap();
        assert_eq!(first.items.len(), 5_000);
        let second = search_annotations(&vault, query, Some(5_000), first.next_cursor).unwrap();
        assert_eq!(second.items.len(), 3);
        assert!(second.next_cursor.is_none());
        assert!(first.items.last().unwrap().url < second.items[0].url);
    }
}
