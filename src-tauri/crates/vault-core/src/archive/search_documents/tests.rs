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
fn shared_page_tracks_visible_profiles_and_preserves_enrichment() {
    for key in [None, Some([45; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault);
        vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                set_enrichment(s, 1, "An enriched description")?;
                s.execute("UPDATE visits SET reverted_at='now' WHERE id=2", ())?;
                s.execute("UPDATE urls SET title='Revised title' WHERE id=1", ())?;
                refresh(s, 1)
            })
            .unwrap();
        let result = vault.store().read(Db::Archive, |s| {
            let document = s.query_one("SELECT title,newest_visit_ms,enrichment_text FROM search_documents WHERE page_id=1", (), |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<String>(2)?)))?;
            let members = s.query("SELECT source_profile_id,visible_visit_count,newest_visit_ms FROM search_document_profiles", (), |r| Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))?;
            Ok((document, members))
        }).unwrap();
        assert_eq!(result.0, ("Revised title".into(), 10, "An enriched description".into()));
        assert_eq!(result.1, vec![(1, 1, 10)]);
    }
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
            s.query_one("SELECT notes_text,tags_text,(SELECT COUNT(*) FROM search_outbox) FROM search_documents WHERE page_id=1", (), |r| Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<i64>(2)?)))
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
fn absent_enrichment_document_does_not_publish_a_phantom_change() {
    for key in [None, Some([69; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        super::set_enrichment_text(&vault, 999, "missing document").unwrap();
        assert_eq!(
            vault
                .store()
                .read(Db::Archive, |s| s
                    .query_one("SELECT COUNT(*) FROM search_outbox", (), |r| Ok(r.get::<i64>(0)?)))
                .unwrap(),
            0
        );
    }
}
