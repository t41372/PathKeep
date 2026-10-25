//! Responsibilities: exercise search through real Vault owners in both storage modes.
//! Not responsible for: mocking either database or index.
//! Failure modes: a query form returns other pages than the corpus holds; revoked hits leak
//! before the index catches up; cross-visit filters invent eligibility; regex paging stops at
//! an empty chunk; paging repeats or skips while a backup or an undo writes.

use super::*;
use crate::Vault;
use vault_store::{Db, Priority, params};

pub(super) fn seed(vault: &Vault, count: i64) {
    vault.store().write(Db::Archive, Priority::Batch, move |s| {
        for (id, name) in [(1,"chrome"),(2,"firefox")] {
            s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES(?1,?2,?2,'test','test','now')",params![id,name])?;
        }
        for id in 1..=count {
            let url=format!("https://example.com/page/{id}");
            let title=match id%5 {0=>"common café 設定 release notes 4217",1=>"common 42fish excluded",2=>"common design release later notes",3=>"common 上海設計系統",_=>"common rare"};
            s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(?1,?2,?2)",params![id,url.as_str()])?;
            for profile in 1..=2 {
                let row=id*2+profile;
                let time=if profile==1 {id} else {10_000+id};
                s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) VALUES(?1,?2,?3,?4,?5,'then',?5,'then',?6,0)",params![row,id,url.as_str(),title,time,profile])?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,?2,0,?3,'then')",params![row,profile,time])?;
            }
            crate::archive::search_documents::refresh(s,id)?;
        }
        Ok(())
    }).unwrap();
    vault.search().cycle().unwrap();
}

#[test]
fn query_forms_and_pages_match_the_generated_corpus() {
    for key in [None, Some([73; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 105);
        for (query, residues) in [
            ("42*", vec![0, 1]),
            ("cafe", vec![0]),
            ("设置", vec![0]),
            ("設計系統", vec![3]),
            ("\"release notes\"", vec![0]),
            ("common -excluded", vec![0, 2, 3, 4]),
            ("intitle:rare", vec![4]),
            ("rare OR cafe", vec![0, 4]),
        ] {
            let request = SearchRequest { query: query.into(), page_size: 7, ..Default::default() };
            let mut session = SearchSession::start(&vault, request).unwrap();
            let mut ids = Vec::new();
            loop {
                let step = session.finish_page().unwrap();
                assert!(step.count.exact);
                ids.extend(step.items.iter().map(|h| h.page_id));
                let Some(cursor) = step.cursor else { break };
                session.next_page(&cursor).unwrap();
            }
            let mut expected = (1..=105)
                .filter(|id| {
                    residues.contains(&(id % 5))
                        || (query == "42*" && id.to_string().starts_with("42"))
                })
                .collect::<Vec<_>>();
            ids.sort_unstable();
            expected.sort_unstable();
            assert_eq!(ids, expected, "{query}");
        }
    }
}

#[test]
fn canonical_filters_use_the_same_visit_and_revokes_do_not_wait_for_indexing() {
    for key in [None, Some([74; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 30);
        let request = SearchRequest {
            query: "common".into(),
            profile_id: Some("chrome".into()),
            start_time_ms: Some(10_001),
            ..Default::default()
        };
        assert!(
            SearchSession::start(&vault, request).unwrap().finish_page().unwrap().items.is_empty()
        );
        let request = SearchRequest {
            query: "common".into(),
            profile_id: Some("chrome".into()),
            start_time_ms: Some(8),
            end_time_ms: Some(12),
            ..Default::default()
        };
        let mut session = SearchSession::start(&vault, request).unwrap();
        let first = session.finish_page().unwrap();
        assert_eq!(first.count.value, 5);
        // Intentionally leave the index and outbox untouched: canonical validation is mandatory.
        vault.store().write(Db::Archive,Priority::Interactive,|s| {
            s.execute("UPDATE visits SET reverted_at='now' WHERE url_id IN (SELECT id FROM urls WHERE page_id=10)",())?;
            Ok(())
        }).unwrap();
        let next = session.finish_page().unwrap();
        assert!(next.items.iter().all(|h| h.page_id != 10));
    }
}

#[test]
fn regex_steps_scan_past_empty_chunks_and_page_without_duplicates() {
    for key in [None, Some([76; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        seed(&vault, 2200);
        let mut session = SearchSession::start(
            &vault,
            SearchRequest {
                query: "release.*notes".into(),
                regex: true,
                page_size: 73,
                ..Default::default()
            },
        )
        .unwrap();
        let mut ids = Vec::new();
        loop {
            let step = session.finish_page().unwrap();
            assert_eq!(step.count.value, 880);
            ids.extend(step.items.iter().map(|h| h.page_id));
            let Some(cursor) = step.cursor else { break };
            session.next_page(&cursor).unwrap();
        }
        ids.sort_unstable();
        assert_eq!(ids, (1..=2200).filter(|id| [0, 2].contains(&(id % 5))).collect::<Vec<_>>());
    }
}

#[test]
fn paging_keeps_its_visit_snapshot_when_a_backup_adds_a_new_visit_to_an_old_match() {
    let archive = super::fixture::Archive::with_key(Some([109; 32]));
    for id in 1..=6 {
        archive.url(
            id,
            super::fixture::CHROME,
            &format!("https://stable.test/{id}"),
            "sameword",
            &[id],
        );
    }
    archive.index();
    let mut session = SearchSession::start(
        &archive.vault,
        SearchRequest { query: "sameword".into(), page_size: 2, ..Default::default() },
    )
    .unwrap();
    let first = session.finish_page().unwrap();
    let mut ids = first.items.iter().map(|hit| hit.page_id).collect::<Vec<_>>();
    assert_eq!(ids, vec![6, 5]);
    archive.vault.store().write(Db::Archive,Priority::Batch,|s| {
        s.execute("INSERT INTO visits(url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(1,1,0,100,'later')",())?;
        crate::archive::search_documents::refresh(s,1)
    }).unwrap();
    archive.index();
    let mut cursor = first.cursor;
    while let Some(token) = cursor {
        session.next_page(&token).unwrap();
        let page = session.finish_page().unwrap();
        ids.extend(page.items.into_iter().map(|hit| hit.page_id));
        cursor = page.cursor;
    }
    assert_eq!(ids, vec![6, 5, 4, 3, 2, 1]);
}

#[test]
fn paging_does_not_repeat_a_shared_page_when_its_newest_visit_is_revoked() {
    // Page 6 comes first by its newest visit, and by its visit count. Once that visit is
    // revoked it belongs below the cursor either way, and must not come back.
    for (regex, sort) in [false, true].into_iter().flat_map(|regex| {
        [None, Some("visits".to_owned())].into_iter().map(move |sort| (regex, sort))
    }) {
        let archive = super::fixture::Archive::with_key(Some([110; 32]));
        for id in 1..=5 {
            archive.url(
                id,
                super::fixture::CHROME,
                &format!("https://stable.test/{id}"),
                "sameword",
                &[id * 1000],
            );
        }
        archive.url(6, super::fixture::CHROME, "https://stable.test/6", "sameword", &[4500, 6000]);
        archive.index();
        let mut session = SearchSession::start(
            &archive.vault,
            SearchRequest {
                query: "sameword".into(),
                regex,
                sort: sort.clone(),
                page_size: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let first = session.finish_page().unwrap();
        let mut ids = first.items.iter().map(|hit| hit.page_id).collect::<Vec<_>>();
        assert_eq!(ids, vec![6, 5], "{regex} {sort:?}");
        archive
            .vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute(
                    "UPDATE visits SET reverted_at='now' WHERE url_id=6 AND visit_time_ms=6000",
                    (),
                )?;
                crate::archive::search_documents::refresh(s, 6)
            })
            .unwrap();
        let mut cursor = first.cursor;
        while let Some(token) = cursor {
            session.next_page(&token).unwrap();
            let page = session.finish_page().unwrap();
            ids.extend(page.items.into_iter().map(|hit| hit.page_id));
            cursor = page.cursor;
        }
        assert_eq!(ids, vec![6, 5, 4, 3, 2, 1], "{regex} {sort:?}");
    }
}
