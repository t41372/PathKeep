//! Responsibilities: prove page documents, memberships and outbox are one fact.
//! Not responsible for: Tantivy consumption or full archive rebuilds.
//! Failure modes: cross-profile duplication, hidden visits affect membership,
//! title changes leave stale text, clearing annotations leaves searchable text,
//! failed handoff commits only part of a change, enrichment is lost on refresh.

use super::*;
use crate::{ReplaceTagsRequest, SetNotesRequest, Vault};
use vault_store::{Db, Priority};

fn seed(vault: &Vault) {
    vault.store().write(Db::Archive, Priority::Batch, |s| {
        for id in [1, 2] {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at) VALUES(?1,'chrome','test','test','now')", [id])?;
        }
        s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(1,'https://example.com/shared','https://example.com/shared')", ())?;
        for (id, profile, time) in [(1, 1, 10), (2, 2, 20)] {
            s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) VALUES(?1,1,'https://example.com/shared',?2,?3,'then',?3,'then',?4,0)", params![id, format!("Title {id}"), time, profile])?;
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,?2,0,?3,'then')", params![id, profile, time])?;
        }
        refresh(s, 1)
    }).unwrap();
}

#[test]
fn notes_and_tags_commit_with_the_document_and_outbox_or_not_at_all() {
    for key in [None, Some([46; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault);
        let url = "https://example.com/shared";
        let notes = |body: &str| {
            crate::set_notes(
                &vault,
                SetNotesRequest { url: url.into(), notes: body.into(), source_profile: None },
            )
        };
        let tags = |tag: &str| {
            crate::replace_tags(
                &vault,
                ReplaceTagsRequest {
                    url: url.into(),
                    tags: vec![tag.into()],
                    source_profile: None,
                },
            )
        };
        notes("Remember this").unwrap();
        tags("Research").unwrap();
        let result = vault.store().read(Db::Archive, |s| {
            s.query_one("SELECT notes_text,tags_text,(SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='search_outbox'),0)) FROM search_documents WHERE page_id=1", (), |r| Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<i64>(2)?)))
        }).unwrap();
        assert_eq!(result, ("Remember this".into(), "Research".into(), 3));
        vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                s.execute("DROP TABLE search_outbox", ())?;
                Ok(())
            })
            .unwrap();
        assert!(notes("Must roll back").is_err());
        assert!(tags("Must roll back").is_err());
        let annotation = crate::get_annotation(&vault, url).unwrap().unwrap();
        assert_eq!(annotation.notes, "Remember this");
        assert_eq!(annotation.tags, vec!["Research"]);
    }
}

#[test]
fn affected_page_refresh_seeks_its_urls_visits_and_search_terms() {
    for key in [None, Some([47; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        for (sql, index) in [
            (TITLE_SELECT, "idx_visits_visible_url_time"),
            (MEMBERSHIP_SELECT, "idx_visits_visible_url_time"),
            (TERMS_SELECT, "idx_search_terms_profile_url_term"),
        ] {
            let plan = vault
                .store()
                .read(Db::Archive, move |s| {
                    s.query(&format!("EXPLAIN QUERY PLAN {sql}"), [1], |r| Ok(r.get::<String>(3)?))
                })
                .unwrap()
                .join("\n");
            assert!(plan.contains("idx_urls_page_id"), "{plan}");
            assert!(plan.contains(index), "{plan}");
            assert!(!plan.contains("SCAN v") && !plan.contains("SCAN t"), "{plan}");
        }
    }
}
