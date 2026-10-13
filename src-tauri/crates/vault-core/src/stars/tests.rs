//! Star lookup regressions and the seed helpers the star tests share.
//!
//! Failure modes: a star lookup that scans every page or URL instead of seeking the
//! canonical-URL and domain indexes.
//!
//! Each test boots a fresh plaintext canonical archive in a temp dir (the migration
//! pipeline runs on first open).

use super::*;
use crate::test_support::store::Probe;
use crate::visit_taxonomy::registrable_domain_for_url;
use std::sync::atomic::{AtomicU32, Ordering};
use vault_store::{Db, Priority, params};

/// Monotonic sequence for `urls.source_url_id` + `last_visit_ms` so successive
/// `seed_url_row` calls never collide on the
/// `(source_profile_id, source_url_id)` UNIQUE index and arrive in a
/// deterministic most-recent-first order (the later-seeded row is "fresher").
static SEED_SEQ: AtomicU32 = AtomicU32::new(1);

/// Seeds a `urls` row so the list/enrichment paths have a visit count + title
/// to read back. Requires a parent run + source profile (FKs). Each call gets a
/// fresh `source_url_id` + strictly-larger `last_visit_ms` so multiple rows for
/// one canonical page (raw-variant fan-out) coexist and order deterministically.
pub(super) fn seed_url_row(vault: &crate::Vault, url: &str, title: &str, count: i64) {
    let (url, title) = (url.to_owned(), title.to_owned());
    let seq = SEED_SEQ.fetch_add(1, Ordering::SeqCst) as i64;
    vault.store().write(Db::Archive,Priority::Batch,move |s| {
        parents(s)?;
        let page=page(s,&url)?;
        s.execute("INSERT INTO urls(url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,page_id) VALUES(?1,?2,?3,0,1,'then',?4,'then',1,1,?4,?5)",params![url,title,count,seq,page])?;Ok(())
    }).unwrap();
}
fn parents(s: &Session<'_>) -> vault_store::Result<()> {
    s.execute("INSERT OR IGNORE INTO runs(id,run_type,trigger,started_at,status) VALUES(1,'backup','manual','then','success')",())?;
    s.execute("INSERT OR IGNORE INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at) VALUES(1,'chrome','Default','test','then')",())?;
    Ok(())
}
fn page(s: &Session<'_>, url: &str) -> vault_store::Result<i64> {
    s.query_one("INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?3) ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",params![url,registrable_domain_for_url(url),crate::pages::canonical_key(url)],|r|Ok(r.get(0)?))
}

/// Inserts a `star` row directly with a controlled `starred_at`, bypassing
/// `set_star`'s `now_rfc3339()` clock. Lets ordering tests pin DESC behaviour
/// without sleeping for the timestamp to advance.
pub(super) fn insert_star_row(vault: &crate::Vault, kind: StarEntityKind, key: &str, at: &str) {
    Probe::new(vault)
        .execute(
            "INSERT INTO star(entity_kind,entity_key,starred_at) VALUES(?1,?2,?3)",
            params![kind.as_str(), key, at],
        )
        .unwrap();
}

#[test]
fn canonical_url_and_domain_lookups_use_production_sql() {
    let vault = crate::Vault::open_temp().unwrap();
    seed_url_row(&vault, "https://Example.com:443/?utm_source=x", "Page", 2);
    vault
        .read(Db::Archive, |s| {
            assert_eq!(url_ids_for_canonical(s, "https://example.com/")?.len(), 1);
            let plan = s.query(
                &format!("EXPLAIN QUERY PLAN {}", crate::pages::URL_IDS_SQL),
                ["https://example.com/"],
                |r| Ok(r.get::<String>(3)?),
            )?;
            assert!(
                plan.iter().any(|p| p.contains("idx_pages_canonical_url") && p.contains("SEARCH")),
                "{plan:?}"
            );
            assert_eq!(url_ids_for_domain(s, "example.com")?.len(), 1);
            let plan = s.query(
                &format!("EXPLAIN QUERY PLAN {DOMAIN_URL_IDS_SQL}"),
                ["example.com"],
                |r| Ok(r.get::<String>(3)?),
            )?;
            assert!(
                plan.iter()
                    .any(|p| p.contains("idx_pages_registrable_domain") && p.contains("SEARCH")),
                "{plan:?}"
            );
            Ok(())
        })
        .unwrap();
}
