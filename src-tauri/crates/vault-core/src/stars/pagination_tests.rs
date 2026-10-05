//! Responsibilities: prove every star and its canonical annotations remain reachable.
//! Not responsible for: the Starred view or speculative sort modes.
//! Failure modes: limits truncate the collection; ties duplicate/skip rows; sorting
//! happens after limiting; notes/tags on URL variants disappear; metadata uses N+1 reads.
use super::*;
#[test]
fn every_sort_pages_past_five_thousand_tied_stars_exactly_once() {
    let vault = Vault::open_temp().unwrap();
    vault
        .write(Db::Archive, Priority::Batch, |s| {
            for id in 0..5_003 {
                s.execute(
                    "INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url',?1,'same')",
                    [format!("https://star.test/{id:05}")],
                )?;
            }
            Ok(())
        })
        .unwrap();
    crate::set_notes(
        &vault,
        crate::SetNotesRequest {
            url: "https://star.test/05002?utm_source=notes".into(),
            notes: "Beyond old limits".into(),
            source_profile: None,
        },
    )
    .unwrap();
    for sort in [StarSort::RecentlyStarred, StarSort::MostRevisited] {
        let mut cursor = None;
        let mut seen = HashSet::new();
        loop {
            let page = list_stars(&vault, None, sort, Some(317), cursor).unwrap();
            for item in page.items {
                if item.entity_key == "https://star.test/05002" {
                    assert_eq!(item.notes, "Beyond old limits");
                }
                assert!(seen.insert(item.entity_key));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 5_003);
    }
}
#[test]
fn star_page_contains_annotations_from_another_url_variant() {
    let vault = Vault::open_temp().unwrap();
    set_star(
        &vault,
        SetStarRequest {
            entity_kind: StarEntityKind::Url,
            entity_key: "https://EXAMPLE.com:443/?utm_source=star".into(),
            source_profile: None,
        },
    )
    .unwrap();
    crate::set_notes(
        &vault,
        crate::SetNotesRequest {
            url: "https://example.com?utm_source=notes".into(),
            notes: "Variant note".into(),
            source_profile: None,
        },
    )
    .unwrap();
    crate::replace_tags(
        &vault,
        crate::ReplaceTagsRequest {
            url: "https://EXAMPLE.com:443/".into(),
            tags: vec!["variant".into()],
            source_profile: None,
        },
    )
    .unwrap();
    let page = list_stars(&vault, None, StarSort::RecentlyStarred, None, None).unwrap();
    assert_eq!(page.items[0].notes, "Variant note");
    assert_eq!(page.items[0].tags, vec!["variant"]);
}

#[test]
fn most_revisited_sorts_the_complete_star_set_before_limiting() {
    let vault = Vault::open_temp().unwrap();
    for (url, at, count) in
        [("https://popular.test/", "2020-01-01", 99), ("https://recent.test/", "2026-01-01", 1)]
    {
        super::tests::seed_url_row(&vault, url, url, count);
        super::tests::insert_star_row(&vault, StarEntityKind::Url, url, at);
    }
    let page = list_stars(&vault, None, StarSort::MostRevisited, Some(1), None).unwrap();
    assert_eq!(page.items[0].entity_key, "https://popular.test/");
    let last =
        list_stars(&vault, None, StarSort::MostRevisited, Some(1), page.next_cursor).unwrap();
    assert_eq!(last.items[0].entity_key, "https://recent.test/");
    assert!(last.next_cursor.is_none());
}
