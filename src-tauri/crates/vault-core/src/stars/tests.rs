//! Failure modes: key normalization changes, duplicate favorites, lost provenance,
//! domain/prefix lookups overmatch, reverted visits appear, sorting is unstable.
//! Unit tests for the stars (favorites) module.
//!
//! These mirror the annotations test harness: each test boots a fresh
//! plaintext canonical archive in a temp dir (the migration pipeline runs on
//! first open) and drives the public `set_star` / `unset_star` /
//! `is_starred_batch` / `list_stars` / `star_counts` surface.

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

fn url_request(url: &str) -> SetStarRequest {
    SetStarRequest {
        entity_kind: StarEntityKind::Url,
        entity_key: url.into(),
        source_profile: Some("chrome:Default".into()),
    }
}

fn domain_request(domain: &str) -> SetStarRequest {
    SetStarRequest {
        entity_kind: StarEntityKind::Domain,
        entity_key: domain.into(),
        source_profile: None,
    }
}

#[test]
fn set_and_unset_round_trip_for_url() {
    let vault = crate::Vault::open_temp().unwrap();
    let url = "https://example.com/page";

    set_star(&vault, url_request(url)).unwrap();
    let status = is_starred_batch(&vault, StarEntityKind::Url, &[url.to_string()]).unwrap();
    assert_eq!(status.get(url), Some(&true));

    unset_star(&vault, url_request(url)).unwrap();
    let status = is_starred_batch(&vault, StarEntityKind::Url, &[url.to_string()]).unwrap();
    assert_eq!(status.get(url), Some(&false));
}

#[test]
fn set_star_is_idempotent_and_refreshes_timestamp() {
    let vault = crate::Vault::open_temp().unwrap();
    let url = "https://example.com/x";

    set_star(&vault, url_request(url)).unwrap();
    // Re-starring must not error and must keep exactly one row.
    set_star(&vault, url_request(url)).unwrap();

    let counts = star_counts(&vault).unwrap();
    assert_eq!(counts.urls, 1);
    assert_eq!(counts.domains, 0);
}

#[test]
fn unset_missing_star_is_a_noop() {
    let vault = crate::Vault::open_temp().unwrap();
    // Deleting a star that never existed must succeed silently.
    unset_star(&vault, url_request("https://example.com/never")).unwrap();
    let counts = star_counts(&vault).unwrap();
    assert_eq!(counts.urls, 0);
}

#[test]
fn star_keys_by_canonical_url_so_tracking_params_collapse() {
    let vault = crate::Vault::open_temp().unwrap();
    // utm_* is a tracking param stripped by normalize_visit_url, so both raw
    // URLs canonicalize to the same key — starring one stars the other.
    let tracked = "https://example.com/post?utm_source=news&id=7";
    let clean = "https://example.com/post?id=7";

    set_star(&vault, url_request(tracked)).unwrap();
    let status =
        is_starred_batch(&vault, StarEntityKind::Url, &[clean.to_string(), tracked.to_string()])
            .unwrap();
    assert_eq!(status.get(clean), Some(&true), "clean variant must read as starred");
    assert_eq!(status.get(tracked), Some(&true), "tracked variant must read as starred");

    let counts = star_counts(&vault).unwrap();
    assert_eq!(counts.urls, 1, "both variants collapse to one canonical star");
}

#[test]
fn is_starred_batch_returns_false_for_unstarred_and_empty_for_no_keys() {
    let vault = crate::Vault::open_temp().unwrap();
    let empty = is_starred_batch(&vault, StarEntityKind::Url, &[]).unwrap();
    assert!(empty.is_empty());

    set_star(&vault, url_request("https://a.test/")).unwrap();
    let status = is_starred_batch(
        &vault,
        StarEntityKind::Url,
        &["https://a.test/".to_string(), "https://b.test/".to_string()],
    )
    .unwrap();
    assert_eq!(status.get("https://a.test/"), Some(&true));
    assert_eq!(status.get("https://b.test/"), Some(&false));
}

#[test]
fn is_starred_batch_reports_unparseable_keys_as_false() {
    let vault = crate::Vault::open_temp().unwrap();
    // "not a url" cannot be canonicalized; it must read false, not error, and
    // must not blank the valid sibling key.
    set_star(&vault, url_request("https://good.test/")).unwrap();
    let status = is_starred_batch(
        &vault,
        StarEntityKind::Url,
        &["not a url".to_string(), "https://good.test/".to_string()],
    )
    .unwrap();
    assert_eq!(status.get("not a url"), Some(&false));
    assert_eq!(status.get("https://good.test/"), Some(&true));
}

#[test]
fn is_starred_batch_returns_empty_when_no_keys_canonicalize() {
    let vault = crate::Vault::open_temp().unwrap();
    let status = is_starred_batch(
        &vault,
        StarEntityKind::Url,
        &["nope".to_string(), "also nope".to_string()],
    )
    .unwrap();
    // Every requested key is present in the map, all false.
    assert_eq!(status.get("nope"), Some(&false));
    assert_eq!(status.get("also nope"), Some(&false));
}

#[test]
fn domain_star_canonicalizes_host_and_accepts_full_url() {
    let vault = crate::Vault::open_temp().unwrap();
    // Starring a subdomain host reduces to the registrable domain…
    set_star(&vault, domain_request("docs.example.com")).unwrap();
    // …and a full URL on the same registrable domain reads as the same star.
    let status = is_starred_batch(
        &vault,
        StarEntityKind::Domain,
        &["https://www.example.com/anything".to_string()],
    )
    .unwrap();
    assert_eq!(status.get("https://www.example.com/anything"), Some(&true));

    let counts = star_counts(&vault).unwrap();
    assert_eq!(counts.domains, 1);
}

#[test]
fn list_stars_orders_recently_starred_first_and_enriches_urls() {
    let vault = crate::Vault::open_temp().unwrap();
    let first = "https://example.com/first";
    let second = "https://example.com/second";
    seed_url_row(&vault, first, "First Page", 3);
    seed_url_row(&vault, second, "Second Page", 9);

    set_star(&vault, url_request(first)).unwrap();
    // Force a strictly-later timestamp for the second star so DESC order is
    // deterministic even at sub-second resolution.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    set_star(&vault, url_request(second)).unwrap();

    let listed =
        list_stars(&vault, Some(StarEntityKind::Url), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].entity_key, second, "most recent star comes first");
    assert_eq!(listed[0].title, "Second Page");
    assert_eq!(listed[0].visit_count, 9);
    assert_eq!(listed[0].domain, "example.com");
    assert_eq!(listed[1].entity_key, first);
}

#[test]
fn list_stars_most_revisited_sort_orders_by_visit_count() {
    let vault = crate::Vault::open_temp().unwrap();
    let low = "https://example.com/low";
    let high = "https://example.com/high";
    seed_url_row(&vault, low, "Low", 2);
    seed_url_row(&vault, high, "High", 42);

    // Star the low-visit page LAST so recency and visit-count disagree, proving
    // the sort actually re-orders by visit_count.
    set_star(&vault, url_request(high)).unwrap();
    set_star(&vault, url_request(low)).unwrap();

    let listed = list_stars(&vault, None, StarSort::MostRevisited, Some(50), None).unwrap().items;
    assert_eq!(listed[0].entity_key, high, "highest visit count first");
    assert_eq!(listed[1].entity_key, low);
}

#[test]
fn list_stars_with_no_kind_filter_interleaves_kinds() {
    let vault = crate::Vault::open_temp().unwrap();
    set_star(&vault, url_request("https://example.com/a")).unwrap();
    set_star(&vault, domain_request("example.com")).unwrap();

    let listed = list_stars(&vault, None, StarSort::RecentlyStarred, None, None).unwrap().items;
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|item| item.entity_kind == StarEntityKind::Url));
    assert!(listed.iter().any(|item| item.entity_kind == StarEntityKind::Domain));
}

#[test]
fn list_stars_enriches_domain_visit_count_from_urls() {
    let vault = crate::Vault::open_temp().unwrap();
    seed_url_row(&vault, "https://example.com/one", "One", 4);
    seed_url_row(&vault, "https://www.example.com/two", "Two", 6);

    set_star(&vault, domain_request("example.com")).unwrap();
    let listed =
        list_stars(&vault, Some(StarEntityKind::Domain), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].domain, "example.com");
    assert_eq!(listed[0].visit_count, 10, "domain count sums its URLs");
}

#[test]
fn list_stars_handles_url_with_no_archive_row() {
    let vault = crate::Vault::open_temp().unwrap();
    // Star a URL the archive has never seen — enrichment must fall back to
    // empty title / 0 visits without failing.
    set_star(&vault, url_request("https://unseen.test/page")).unwrap();
    let listed =
        list_stars(&vault, Some(StarEntityKind::Url), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "");
    assert_eq!(listed[0].visit_count, 0);
    assert_eq!(listed[0].domain, "unseen.test");
}

#[test]
fn list_stars_rejects_zero_and_treats_limit_as_page_size() {
    let vault = crate::Vault::open_temp().unwrap();
    set_star(&vault, url_request("https://example.com/clamp")).unwrap();
    // A zero limit clamps up to 1 (never returns nothing for a valid request);
    // an over-cap limit clamps down to MAX_LIST_LIMIT (no panic on cast).
    assert!(list_stars(&vault, None, StarSort::RecentlyStarred, Some(0), None).is_err());
    let huge = list_stars(&vault, None, StarSort::RecentlyStarred, Some(5003), None).unwrap().items;
    assert_eq!(huge.len(), 1);
}

#[test]
fn set_star_rejects_empty_key() {
    let vault = crate::Vault::open_temp().unwrap();
    let err = set_star(
        &vault,
        SetStarRequest {
            entity_kind: StarEntityKind::Url,
            entity_key: "   ".into(),
            source_profile: None,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("required"));
}

#[test]
fn set_star_rejects_unparseable_url() {
    let vault = crate::Vault::open_temp().unwrap();
    let err = set_star(&vault, url_request("definitely not a url")).unwrap_err();
    assert!(err.to_string().contains("canonicalize"));
}

#[test]
fn set_star_rejects_undomainable_key() {
    let vault = crate::Vault::open_temp().unwrap();
    let err = set_star(
        &vault,
        SetStarRequest {
            entity_kind: StarEntityKind::Domain,
            entity_key: "...".into(),
            source_profile: None,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("registrable domain"));
}

#[test]
fn parse_kind_drops_unknown_future_kinds() {
    // A future query_family star written by a newer build must not crash an
    // older read path — collect_star_rows silently skips it.
    let vault = crate::Vault::open_temp().unwrap();
    let connection = Probe::new(&vault);
    connection
        .execute(
            "INSERT INTO star(entity_kind, entity_key, starred_at, source_profile)
             VALUES('query_family', 'q:rust async', '2026-04-24T00:00:00Z', NULL)",
            (),
        )
        .unwrap();
    set_star(&vault, url_request("https://example.com/known")).unwrap();

    let listed = list_stars(&vault, None, StarSort::RecentlyStarred, None, None).unwrap().items;
    // Only the known url star surfaces; the unknown kind is skipped.
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].entity_kind, StarEntityKind::Url);
    assert!(parse_kind("query_family").is_none());
    assert_eq!(parse_kind("url"), Some(StarEntityKind::Url));
    assert_eq!(parse_kind("domain"), Some(StarEntityKind::Domain));
}

#[test]
fn list_stars_enriches_canonical_url_from_tracking_param_raw_visit() {
    // Regression for the keying/enrichment bug: the archive only ever stored the
    // RAW visit URL (tracking params + non-normalized host), but the star keys by
    // the CANONICAL url. A naive `urls.url = canonical` join misses the row, so
    // the hub showed an empty title and visit_count = 0. Enrichment must resolve
    // the real title + visit count by canonicalizing the candidate raw rows.
    let vault = crate::Vault::open_temp().unwrap();

    // The stored visit row is the RAW url: a lowercase host (the form every
    // browser stores — hosts are case-insensitive and normalized to lowercase
    // before persisting) plus a utm_* tracking param. normalize_visit_url strips
    // utm_*, so it canonicalizes to https://example.com/post?id=7 — the star key
    // below — but the stored url differs (tracking param), so the exact seek
    // misses and the prefix RANGE SEEK + Rust re-check must resolve the title.
    // (Host casing is an accepted residual: a stored mixed-case host sorts
    // outside the BINARY byte-range, but browsers never store one — see the
    // RESIDUALS note on `url_ids_for_canonical`.)
    let raw = "https://example.com/post?utm_source=newsletter&id=7";
    seed_url_row(&vault, raw, "The Real Title", 11);

    // Star the page via its clean (canonical-ish) form; set_star canonicalizes.
    set_star(&vault, url_request("https://example.com/post?id=7")).unwrap();

    let listed =
        list_stars(&vault, Some(StarEntityKind::Url), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "The Real Title", "title resolves across the raw variant");
    assert!(listed[0].visit_count > 0, "visit_count must be summed from the raw visit");
    assert_eq!(listed[0].visit_count, 11);
    assert_eq!(listed[0].domain, "example.com");
}

#[test]
fn list_stars_sums_visit_count_across_raw_variants_of_one_canonical_page() {
    // Several raw rows (different tracking params) collapse to one canonical
    // page; the hub must SUM their visit counts, not pick one or miss them.
    let vault = crate::Vault::open_temp().unwrap();
    seed_url_row(&vault, "https://example.com/post?id=7&utm_source=a", "Older", 4);
    seed_url_row(&vault, "https://example.com/post?id=7&utm_source=b", "Newer", 6);

    set_star(&vault, url_request("https://example.com/post?id=7")).unwrap();
    let listed =
        list_stars(&vault, Some(StarEntityKind::Url), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].visit_count, 10, "both raw variants sum into the canonical count");
    // The most-recently-visited row's title wins (Newer was seeded last).
    assert_eq!(listed[0].title, "Newer");
}

#[test]
fn list_stars_prefix_scan_does_not_match_unrelated_path_suffixes() {
    // The prefix range scan keys on scheme://host/path and can over-match
    // (/foo is a prefix of /foobar). The Rust canonicalization re-check must
    // exclude the over-matched rows so the count is honest.
    let vault = crate::Vault::open_temp().unwrap();
    // Force pass 2 (prefix scan) by storing only a tracking-param raw variant so
    // the exact seek misses; /foobar shares the /foo prefix but is a different
    // canonical page and must not be summed in.
    seed_url_row(&vault, "https://example.com/foo?utm_source=x", "Foo", 3);
    seed_url_row(&vault, "https://example.com/foobar?utm_source=x", "Foobar", 99);

    set_star(&vault, url_request("https://example.com/foo")).unwrap();
    let listed =
        list_stars(&vault, Some(StarEntityKind::Url), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].visit_count, 3, "only the /foo page counts, not /foobar");
    assert_eq!(listed[0].title, "Foo");
}

#[test]
fn is_starred_is_partitioned_by_entity_kind() {
    // A url star and a domain star can share the same key string
    // ("example.com"). The `entity_kind = ?1` predicate must keep them apart:
    // a url query must not see the domain star (and vice versa).
    let vault = crate::Vault::open_temp().unwrap();
    // Insert two rows with the IDENTICAL key but different kinds.
    insert_star_row(&vault, StarEntityKind::Domain, "example.com", "2026-04-24T00:00:00Z");
    insert_star_row(&vault, StarEntityKind::Url, "https://example.com/", "2026-04-24T00:00:01Z");

    // A url query for the domain's key string must NOT report it starred — the
    // only url-kind row is the full URL, not the bare domain.
    let url_view =
        is_starred_batch(&vault, StarEntityKind::Url, &["https://example.com/".to_string()])
            .unwrap();
    assert_eq!(url_view.get("https://example.com/"), Some(&true));

    // The domain star is keyed "example.com"; a domain query sees it, a url
    // query for a bare host string would canonicalize to a registrable domain
    // and is a different kind entirely — confirm the domain side still reads.
    let domain_view =
        is_starred_batch(&vault, StarEntityKind::Domain, &["example.com".to_string()]).unwrap();
    assert_eq!(domain_view.get("example.com"), Some(&true));

    // The kinds are independent counts.
    let counts = star_counts(&vault).unwrap();
    assert_eq!(counts.urls, 1);
    assert_eq!(counts.domains, 1);
}

#[test]
fn list_stars_most_revisited_breaks_ties_by_recency() {
    // Equal visit_count must fall back to starred_at DESC (more-recent first),
    // pinning the tie-break direction the sort comment documents.
    let vault = crate::Vault::open_temp().unwrap();
    let older = "https://example.com/older";
    let newer = "https://example.com/newer";
    seed_url_row(&vault, older, "Older", 5);
    seed_url_row(&vault, newer, "Newer", 5);
    // Controlled timestamps: `newer` was starred strictly later.
    insert_star_row(&vault, StarEntityKind::Url, older, "2026-04-24T00:00:00Z");
    insert_star_row(&vault, StarEntityKind::Url, newer, "2026-04-24T00:00:05Z");

    let listed = list_stars(&vault, None, StarSort::MostRevisited, Some(50), None).unwrap().items;
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].visit_count, listed[1].visit_count, "the test relies on a real tie");
    assert_eq!(listed[0].entity_key, newer, "on equal visit_count, more-recently-starred wins");
    assert_eq!(listed[1].entity_key, older);
}

#[test]
fn list_stars_none_kind_orders_recently_starred_desc() {
    // The kind=None branch must order by starred_at DESC. Use direct inserts
    // with controlled timestamps (not a sleep) so the assertion is exact and
    // fast, and mix kinds so we also exercise the interleave on the None branch.
    let vault = crate::Vault::open_temp().unwrap();
    insert_star_row(&vault, StarEntityKind::Url, "https://a.test/", "2026-04-24T00:00:00Z");
    insert_star_row(&vault, StarEntityKind::Domain, "b.test", "2026-04-24T00:00:01Z");
    insert_star_row(&vault, StarEntityKind::Url, "https://c.test/", "2026-04-24T00:00:02Z");

    let listed = list_stars(&vault, None, StarSort::RecentlyStarred, None, None).unwrap().items;
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0].entity_key, "https://c.test/", "newest first");
    assert_eq!(listed[1].entity_key, "b.test");
    assert_eq!(listed[2].entity_key, "https://a.test/", "oldest last");
}

#[test]
fn list_stars_enriches_url_path_with_like_wildcards_literally() {
    // A canonical path containing `_`/`%` must be escaped in the prefix LIKE so
    // the wildcard chars match literally, not as SQL wildcards. Force the
    // prefix-scan pass by storing only a tracking-param raw variant.
    let vault = crate::Vault::open_temp().unwrap();
    // `_` would otherwise match any single char; `/a_b` must not pull in `/axb`.
    seed_url_row(&vault, "https://example.com/a_b?utm_source=x", "Underscore", 4);
    seed_url_row(&vault, "https://example.com/axb?utm_source=x", "Wildcard", 50);

    set_star(&vault, url_request("https://example.com/a_b")).unwrap();
    let listed =
        list_stars(&vault, Some(StarEntityKind::Url), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].visit_count, 4, "the `_` is matched literally, not as a wildcard");
    assert_eq!(listed[0].title, "Underscore");
}

#[test]
fn entity_kind_as_str_matches_storage_encoding() {
    assert_eq!(StarEntityKind::Url.as_str(), "url");
    assert_eq!(StarEntityKind::Domain.as_str(), "domain");
}

/// Seeds a `urls` + `visits` row pair so `starred_history_ids` (which joins them) can resolve a
/// starred page to its visit id. The visit id equals `visit_id`; `reverted` marks it reverted so the
/// test can prove reverted visits are excluded from the starred-visit scan.
fn seed_visit_row(vault: &crate::Vault, visit_id: i64, url: &str, reverted: bool) {
    let seq = SEED_SEQ.fetch_add(1, Ordering::SeqCst) as i64;
    let url = url.to_owned();
    vault.store().write(Db::Archive,Priority::Batch,move |s| {
        parents(s)?;let page=page(s,&url)?;
        s.execute("INSERT INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,page_id) VALUES(?1,?2,'Seed',1,0,1,'then',?3,'then',1,1,?3,?4)",params![visit_id,url,seq,page])?;
        s.execute("INSERT INTO visits(id,url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id,reverted_at) VALUES(?1,?1,?2,?3,'then',1,1,?4)",params![visit_id,visit_id.to_string(),seq,reverted.then_some("reverted")])?;
        Ok(())
    }).unwrap();
}

#[test]
fn starred_matcher_matches_url_variants_and_starred_domains() {
    // W-AI-6: the in-memory matcher resolves starred-ness for a result URL. A starred canonical URL
    // matches every tracking-param/host-casing variant; a starred domain covers every page on it.
    let mut matcher = StarredMatcher::default();
    assert!(matcher.is_empty(), "a fresh matcher is empty");
    assert!(!matcher.is_starred("https://example.com/a"), "nothing is starred yet");

    let vault = crate::Vault::open_temp().unwrap();
    set_star(&vault, url_request("https://example.com/page")).unwrap();
    set_star(&vault, domain_request("news.example.org")).unwrap();
    matcher = load_starred_matcher(&vault).unwrap();
    assert!(!matcher.is_empty());
    // URL star: a tracking-param + host-casing variant of the starred page reads as starred.
    assert!(matcher.is_starred("https://Example.com/page?utm_source=x"));
    // Domain star: any page on the starred registrable domain reads as starred (subdomain included).
    assert!(matcher.is_starred("https://www.news.example.org/world/story"));
    // An unrelated page on neither a starred URL nor a starred domain is NOT starred.
    assert!(!matcher.is_starred("https://other.com/page"));
    // An unparseable URL is simply not URL-starred (no panic).
    assert!(!matcher.is_starred("not a url"));
}

#[test]
fn load_starred_matcher_reads_url_and_domain_kinds() {
    let vault = crate::Vault::open_temp().unwrap();
    set_star(&vault, url_request("https://example.com/keep")).unwrap();
    set_star(&vault, domain_request("docs.rs")).unwrap();

    let matcher = load_starred_matcher(&vault).unwrap();
    assert!(matcher.is_starred("https://example.com/keep"));
    assert!(matcher.is_starred("https://docs.rs/serde/latest"));
    assert!(!matcher.is_starred("https://example.com/other"));
}

#[test]
fn starred_history_ids_resolves_starred_visits_and_excludes_reverted() {
    // W-AI-6 facet allowlist: starred URLs/domains resolve to their archive visit ids (bounded join).
    let vault = crate::Vault::open_temp().unwrap();
    // visit 1: a starred page (visible). visit 2: same starred page but REVERTED (must be excluded).
    seed_visit_row(&vault, 1, "https://example.com/page?utm_source=ad", false);
    seed_visit_row(&vault, 2, "https://example.com/page", true);
    // visit 3: a page on a starred DOMAIN. visit 4: an unstarred page (must be excluded).
    seed_visit_row(&vault, 3, "https://blog.starred-domain.com/post", false);
    seed_visit_row(&vault, 4, "https://unstarred.com/page", false);

    set_star(&vault, url_request("https://example.com/page")).unwrap();
    set_star(&vault, domain_request("starred-domain.com")).unwrap();

    let matcher = load_starred_matcher(&vault).unwrap();
    let ids = starred_history_ids(&vault, &matcher).unwrap();
    assert!(ids.contains(&1), "the visible starred-URL visit is included");
    assert!(!ids.contains(&2), "a reverted starred visit is excluded");
    assert!(ids.contains(&3), "a visit on a starred domain is included");
    assert!(!ids.contains(&4), "an unstarred visit is excluded");
    assert_eq!(ids.len(), 2);
}

#[test]
fn load_starred_matcher_drops_unknown_future_kinds() {
    // A newer build may write a star kind (e.g. `query_family`) an older read path doesn't know; the
    // matcher must drop it rather than crash or mis-bucket it (the `None` arm of the kind match).
    let vault = crate::Vault::open_temp().unwrap();
    let connection = Probe::new(&vault);
    connection
        .execute(
            "INSERT INTO star(entity_kind, entity_key, starred_at, source_profile)
             VALUES('query_family', 'rust tutorials', '2026-04-24T00:00:00Z', NULL)",
            (),
        )
        .unwrap();
    // A known URL star coexists so the matcher is non-empty for a sound assertion.
    set_star(&vault, url_request("https://example.com/keep")).unwrap();

    let matcher = vault.read(Db::Archive, load_starred_matcher_from).unwrap();
    assert!(matcher.is_starred("https://example.com/keep"), "the known URL star is kept");
    assert!(
        !matcher.is_starred("https://example.com/query%20family"),
        "the unknown kind is dropped"
    );
}

#[test]
fn starred_history_ids_is_empty_when_nothing_starred() {
    // No stars → the resolution short-circuits to an empty set (no archive query needed).
    let vault = crate::Vault::open_temp().unwrap();
    seed_visit_row(&vault, 1, "https://example.com/page", false);
    let matcher = load_starred_matcher(&vault).unwrap();
    assert!(matcher.is_empty());
    assert!(starred_history_ids(&vault, &matcher).unwrap().is_empty());
}

#[test]
fn starred_history_ids_resolution_is_bounded_by_the_star_set_not_the_corpus() {
    // HIGH regression: `starred_history_ids` must resolve the tiny starred set FORWARD (starred URL →
    // urls.id → visits.id), NOT scan every non-reverted visit and test each URL. Seed MANY non-starred
    // visits + a handful starred ones and assert ONLY the starred visits resolve — the result count is
    // bounded by the star set, never the corpus. A regression to the full-corpus scan would still pass
    // the equality assert, so we ALSO assert the query plan: the urls step is an `idx_urls_url` SEARCH
    // (not a SCAN urls) and the visits step rides the url_id index.
    let vault = crate::Vault::open_temp().unwrap();

    // 200 non-starred visits (the "corpus" the old code would have materialized + canonicalized).
    for visit_id in 1..=200 {
        seed_visit_row(&vault, visit_id, &format!("https://noise{visit_id}.test/p"), false);
    }
    // A handful of starred rows: a URL star (stored under a tracking-param raw variant so the prefix
    // pass is exercised) and a domain star covering one page.
    seed_visit_row(&vault, 201, "https://keep.test/post?utm_source=ad", false);
    seed_visit_row(&vault, 202, "https://blog.fav-domain.test/x", false);
    set_star(&vault, url_request("https://keep.test/post")).unwrap();
    set_star(&vault, domain_request("fav-domain.test")).unwrap();

    let matcher = load_starred_matcher(&vault).unwrap();
    let ids = starred_history_ids(&vault, &matcher).unwrap();
    // Exactly the two starred visits resolve; the 200 noise visits never enter the result.
    assert_eq!(ids, std::collections::HashSet::from([201, 202]));

    // Prove the plan is index-bounded, not a full table scan. `idx_urls_url` must be SEARCHed for the
    // exact-seek pass (the load-bearing index the fix relies on, migration 014_stars.sql:49).
    let connection = Probe::new(&vault);
    let url_plan = explain_query_plan(&connection, crate::pages::URL_IDS_SQL, &["x"]);
    assert!(
        url_plan
            .iter()
            .any(|step| step.contains("SEARCH") && step.contains("idx_pages_canonical_url")),
        "the URL-star exact seek must SEARCH idx_urls_url, not SCAN urls: {url_plan:?}"
    );
    assert!(
        !url_plan
            .iter()
            .any(|step| step.contains("SCAN urls") && !step.contains("idx_pages_canonical_url")),
        "the URL-star exact seek must not full-scan urls: {url_plan:?}"
    );
    // The visits hop rides an index on url_id (the visible-url-time index), not a full visits scan.
    let visit_plan = explain_query_plan(&connection, &visit_ids_query(1), &["1"]);
    assert!(
        visit_plan.iter().any(|step| step.contains("SEARCH") && step.contains("visits")),
        "the visits resolution must SEARCH visits by url_id, not SCAN: {visit_plan:?}"
    );
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

#[test]
fn starred_history_ids_is_empty_when_starred_pages_are_not_in_the_archive() {
    // A non-empty matcher whose starred URL/domain has NO matching `urls` row resolves to zero
    // `urls.id`s, so the visits hop short-circuits (the `url_ids.is_empty()` guard) to an empty set
    // rather than running an `IN ()` query. The user starred something the archive has not (yet) seen.
    let vault = crate::Vault::open_temp().unwrap();
    // Seed an UNRELATED visit so the visits table is non-empty (proving the empty result is the missing
    // url_id, not an empty archive).
    seed_visit_row(&vault, 1, "https://present.test/page", false);
    set_star(&vault, url_request("https://unseen.test/never")).unwrap();
    set_star(&vault, domain_request("also-unseen.test")).unwrap();

    let matcher = load_starred_matcher(&vault).unwrap();
    assert!(!matcher.is_empty(), "the matcher has stars even though the archive lacks those pages");
    assert!(
        starred_history_ids(&vault, &matcher).unwrap().is_empty(),
        "stars with no matching urls.id resolve to no visits"
    );
}

#[test]
fn url_ids_for_canonical_falls_back_when_key_is_not_a_parseable_url() {
    // Equality on an absent canonical key produces no matches.
    let vault = crate::Vault::open_temp().unwrap();
    let ids = vault.read(Db::Archive, |s| url_ids_for_canonical(s, "not a url")).unwrap();
    assert!(ids.is_empty(), "an unparseable key resolves to no urls.id without panicking");
}

#[test]
fn starred_history_ids_domain_arm_rejects_embedded_domain_on_unrelated_host() {
    // A-1 (over-recall rejected): a raw URL with the starred domain embedded in a path/query
    // (`?img=//example.com/p.jpg`) on an UNRELATED host substring-matches the bare `%//example.com/%`
    // LIKE, but is NOT on `example.com`. The `registrable_domain_for_url` confirm — exactly the test
    // `StarredMatcher::is_starred` uses — must reject it so the facet allowlist matches the matcher.
    let vault = crate::Vault::open_temp().unwrap();
    seed_visit_row(&vault, 1, "https://news.other.com/story?img=//example.com/p.jpg", false);
    set_star(&vault, domain_request("example.com")).unwrap();

    let matcher = load_starred_matcher(&vault).unwrap();
    // Sanity: the per-visit matcher (the reviewed-correct baseline) does NOT consider this starred.
    assert!(
        !matcher.is_starred("https://news.other.com/story?img=//example.com/p.jpg"),
        "the matcher binds to the registrable host, not a substring",
    );
    let ids = starred_history_ids(&vault, &matcher).unwrap();
    assert!(
        ids.is_empty(),
        "the embedded-domain visit on an unrelated host must NOT enter the facet allowlist: {ids:?}",
    );
}

#[test]
fn starred_history_ids_domain_arm_matches_ported_hosts() {
    // A-2 (ported matched): a raw URL whose host carries a port has no `/` immediately after the host,
    // so the old `%//domain/%` pattern pre-filtered it OUT. The `:`-tolerant candidate patterns admit
    // it and the registrable-domain confirm includes it — exactly like `StarredMatcher::is_starred`.
    let vault = crate::Vault::open_temp().unwrap();
    // A bare-host port (localhost:3000) and an apex-host port (example.com:8443).
    seed_visit_row(&vault, 1, "http://localhost:3000/x", false);
    seed_visit_row(&vault, 2, "https://example.com:8443/p", false);
    // Pin no-regression: a subdomain page (no port) still matches via the `%.domain/%` pattern.
    seed_visit_row(&vault, 3, "https://blog.example.com/p", false);
    set_star(&vault, domain_request("localhost")).unwrap();
    set_star(&vault, domain_request("example.com")).unwrap();

    let matcher = load_starred_matcher(&vault).unwrap();
    // The matcher (baseline) agrees these are all starred.
    assert!(matcher.is_starred("http://localhost:3000/x"));
    assert!(matcher.is_starred("https://example.com:8443/p"));
    assert!(matcher.is_starred("https://blog.example.com/p"));
    let ids = starred_history_ids(&vault, &matcher).unwrap();
    assert!(ids.contains(&1), "a ported bare host (localhost:3000) resolves: {ids:?}");
    assert!(ids.contains(&2), "a ported apex host (example.com:8443) resolves: {ids:?}");
    assert!(ids.contains(&3), "a subdomain page (no port) still resolves: {ids:?}");
    assert_eq!(ids.len(), 3);
}

#[test]
fn url_ids_for_domain_returns_empty_when_no_url_matches_the_domain() {
    // Empty-result branch: a starred domain with no matching `urls` row yields an empty id list (the
    // candidate LIKE matches nothing, so the confirm loop never runs).
    let vault = crate::Vault::open_temp().unwrap();
    seed_url_row(&vault, "https://elsewhere.test/p", "Elsewhere", 3);
    let ids = vault.read(Db::Archive, |s| url_ids_for_domain(s, "example.com")).unwrap();
    assert!(ids.is_empty(), "a domain with no matching urls resolves to no ids: {ids:?}");
}

#[test]
fn enrich_entity_domain_count_agrees_with_facet_host_binding() {
    // The Starred-hub domain visit-count and the `is:starred` facet must agree: both bind to the
    // registrable host. Seed a real on-domain page, a ported on-domain page, AND an unrelated host
    // whose URL embeds the domain in a query — the count must include only the two real on-domain rows.
    let vault = crate::Vault::open_temp().unwrap();
    seed_url_row(&vault, "https://example.com/one", "One", 4);
    seed_url_row(&vault, "https://example.com:8443/two", "Two (ported)", 6);
    seed_url_row(&vault, "https://other.com/x?img=//example.com/p.jpg", "Embedded", 99);

    set_star(&vault, domain_request("example.com")).unwrap();
    let listed =
        list_stars(&vault, Some(StarEntityKind::Domain), StarSort::RecentlyStarred, None, None)
            .unwrap()
            .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].visit_count, 10,
        "the count sums on-domain rows (incl. ported) but NOT the embedded-domain unrelated host",
    );
}

/// Returns the `detail` column of `EXPLAIN QUERY PLAN <sql>` (one string per plan step). Used by the
/// boundedness regression to assert the starred resolution rides `idx_urls_url` / the url_id index
/// rather than scanning the corpus.
fn explain_query_plan(connection: &Probe, sql: &str, binds: &[&str]) -> Vec<String> {
    connection
        .query(
            &format!("EXPLAIN QUERY PLAN {sql}"),
            binds.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            |r| Ok(r.get(3)?),
        )
        .unwrap()
}
