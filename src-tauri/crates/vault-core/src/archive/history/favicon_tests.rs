//! Responsibilities: verify favicon precedence and indexed hydration.
//! Not responsible for: browser parsing or image fetching.
//! Failure modes: icons from future visits or unrelated hosts, duplicate replies,
//! missing-profile errors, reading all candidates before selecting one.
use super::*;
use vault_store::{Db, Priority};

#[test]
fn exact_then_same_host_respects_time_profiles_and_request_identity() {
    for key in [None, Some([77; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            for (profile,url,host,time,marker) in [
                (1,"https://example.com/a","example.com",5000,1),
                (2,"https://example.com/a","example.com",3000,2),
                (1,"https://example.com/b","example.com",1000,3),
                (2,"https://example.com/c","example.com",500,4),
                (1,"https://other.example.com/x","other.example.com",100,5),
            ] {
                s.execute("INSERT INTO favicons(source_profile_id,page_url,page_host,icon_url,last_updated_ms,image_data,created_by_run_id) VALUES(?1,?2,?3,?2,?4,?5,0)",params![profile,url,host,time,vec![137,80,78,71,13,10,26,10,marker]])?;
            }
            Ok(())
        }).unwrap();
        let entry = |time| HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".into(),
            url: "https://example.com/a".into(),
            visit_time: time,
        };
        let rows = load_history_favicons(
            &vault,
            vec![entry(5000), entry(4000), entry(2000), entry(700), entry(200), entry(200)],
        )
        .unwrap();
        assert_eq!(rows.len(), 5);
        for (row, marker) in rows[..4].iter().zip(1..=4) {
            assert_eq!(
                row.favicon.as_ref().unwrap().data_url,
                image_data_to_data_url(&[137, 80, 78, 71, 13, 10, 26, 10, marker]).unwrap()
            );
        }
        assert!(rows[4].favicon.is_none());
        let mut unknown = entry(5000);
        unknown.profile_id = "missing".into();
        assert!(load_history_favicons(&vault, vec![unknown]).unwrap()[0].favicon.is_none());
        assert!(load_history_favicons(&vault, vec![]).unwrap().is_empty());
    }
}

#[test]
fn lookup_queries_seek_indexes_before_loading_blobs() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .store()
        .read(Db::Archive, |s| {
            for sql in [
                LOAD_FAVICON_SAME_PROFILE_PAGE_SQL,
                LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL,
                LOAD_FAVICON_SAME_PROFILE_HOST_SQL,
                LOAD_FAVICON_CROSS_PROFILE_HOST_SQL,
            ] {
                let args = if sql.contains("?4") {
                    params![1, "example.com", "https://example.com/a", 5000]
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    params![1, "https://example.com/a", 5000]
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?
                };
                let plan = s
                    .query(&format!("EXPLAIN QUERY PLAN {sql}"), args, |r| Ok(r.get::<String>(3)?))?
                    .join("\n");
                assert!(plan.contains("SEARCH favicons"), "{plan}");
            }
            let args = params!["example.com", "https://example.com/", "http://example.com/"]
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            let plan = s
                .query(&format!("EXPLAIN QUERY PLAN {LOAD_SITE_ICON_SQL}"), args, |r| {
                    Ok(r.get::<String>(3)?)
                })?
                .join("\n");
            assert!(plan.contains("SEARCH favicons"), "{plan}");
            Ok(())
        })
        .unwrap();
}

#[test]
fn hydration_reads_shared_blobs_and_refuses_other_registrable_hosts() {
    for key in [None, Some([77; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO favicon_blobs(blob_hash,image_data,recorded_at) VALUES('shared',?1,'now')",[vec![137,80,78,71,13,10,26,10,9]])?;
            s.execute("INSERT INTO favicons(source_profile_id,page_url,page_host,page_registrable_domain,icon_url,last_updated_ms,image_blob_hash,created_by_run_id) VALUES(1,'https://tim.github.io/a','tim.github.io','github.io','/icon',1000,'shared',0),(2,'https://docs.example.co.uk/a','docs.example.co.uk','example.co.uk','/icon',1000,'shared',0)",())?;
            Ok(())
        }).unwrap();
        let entries =
            ["https://tim.github.io/a", "https://nick.github.io/a", "https://blog.example.co.uk/a"]
                .map(|url| HistoryFaviconLookupEntry {
                    profile_id: "chrome:Default".into(),
                    url: url.into(),
                    visit_time: 2000,
                });
        let rows = load_history_favicons(&vault, entries.into()).unwrap();
        assert!(rows[0].favicon.is_some());
        assert!(rows[1].favicon.is_none() && rows[2].favicon.is_none());
    }
}
