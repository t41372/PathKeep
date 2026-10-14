//! Responsibilities: keep every note and tag reachable through the annotation cursor.
//! Not responsible for: Tantivy recall or UI rendering.
//! Failure modes: a page limit or tied timestamps hide records past the first page.

use super::*;

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
