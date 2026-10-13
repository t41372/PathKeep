//! Responsibilities: preserve exact-page image caching, dedup and eviction.
//! Not responsible for: network fetching.
//! Failure modes: bytes duplicate, negative cache forgotten, LRU recency lost,
//! shared blobs evict too early, row/blob writes partially commit.
use super::*;

#[test]
fn prefetch_and_eviction_walk_indexes_without_sorting_the_archive() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.store().read(Db::Archive,|s| {
        for (query,index) in [
            (super::reads::prefetch_sql(false,false).replace("?1","10").replace("?2","9223372036854775807").replace("?3","9223372036854775807"),"idx_urls_last_visit_ms"),
            ("SELECT id,image_blob_hash FROM og_images INDEXED BY idx_og_images_eviction ORDER BY fetched_at,id LIMIT 1000".into(),"idx_og_images_eviction"),
            ("SELECT id,image_blob_hash FROM og_images INDEXED BY idx_og_images_lru ORDER BY last_shown_at IS NULL DESC,last_shown_at,fetched_at,id LIMIT 1000".into(),"idx_og_images_lru"),
        ] {
            let plan=s.query(&format!("EXPLAIN QUERY PLAN {query}"),(),|r|Ok(r.get::<String>(3)?))?.join("\n");
            assert!(plan.contains(index),"{plan}");
            assert!(!plan.to_ascii_uppercase().contains("TEMP B-TREE"),"{plan}");
            assert!(!plan.to_ascii_uppercase().contains("SORT"),"{plan}");
        }
        Ok(())
    }).unwrap();
}
