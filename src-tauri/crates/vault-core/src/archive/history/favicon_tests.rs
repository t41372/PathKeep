//! Responsibilities: verify favicon precedence and indexed hydration.
//! Not responsible for: browser parsing or image fetching.
//! Failure modes: icons from future visits or unrelated hosts, duplicate replies,
//! missing-profile errors, reading all candidates before selecting one.
use super::*;
use vault_store::Db;

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
