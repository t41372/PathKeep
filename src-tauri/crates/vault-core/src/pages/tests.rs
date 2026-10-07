//! Responsibilities: verify canonical equality across URL spellings on real Turso.
//! Not responsible for: network fetching or legacy-reader queries.
//! Failure modes: default ports, empty paths, escapes and IDNs miss their shared key;
//! lookups scan unrelated pages or use an untested SQL copy.
use super::*;
use vault_store::{Db, Priority, params};
#[test]
fn canonical_variants_use_the_production_indexed_lookup() {
    let vault = crate::Vault::open_temp().unwrap();
    let pairs = [
        ("https://Example.com:443", "https://example.com/"),
        ("https://example.com/%7Euser?utm_source=x", "https://example.com/~user"),
        ("https://bücher.example/", "https://xn--bcher-kva.example/"),
    ];
    vault
        .store()
        .write(Db::Archive, Priority::Batch, move |s| {
            for (raw, canonical_input) in pairs {
                s.execute(
                    "INSERT INTO pages(url,canonical_url) VALUES(?1,?2)",
                    params![raw, canonical_key(raw)],
                )?;
                let key = canonical_key(canonical_input);
                assert_eq!(ids_for_key(s, &key)?.len(), 1, "{raw}");
                let plan = s.query(&format!("EXPLAIN QUERY PLAN {PAGE_IDS_SQL}"), [key], |r| {
                    Ok(r.get::<String>(3)?)
                })?;
                assert!(
                    plan.iter().any(
                        |row| row.contains("idx_pages_canonical_url") && row.contains("SEARCH")
                    ),
                    "{plan:?}"
                );
            }
            Ok(())
        })
        .unwrap();
}
