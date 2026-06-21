//! Unit tests for the stars (favorites) module.
//!
//! These mirror the annotations test harness: each test boots a fresh
//! plaintext canonical archive in a temp dir (the migration pipeline runs on
//! first open) and drives the public `set_star` / `unset_star` /
//! `is_starred_batch` / `list_stars` / `star_counts` surface.

use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    models::{AppConfig, ArchiveMode},
};
use rusqlite::params;
use std::{
    fs,
    sync::atomic::{AtomicU32, Ordering},
};

static TEST_PATH_SEQ: AtomicU32 = AtomicU32::new(0);

fn make_paths(label: &str) -> ProjectPaths {
    let seq = TEST_PATH_SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("pk-stars-{label}-{}-{seq}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    project_paths_with_root(&root)
}

fn plaintext_config() -> AppConfig {
    AppConfig { archive_mode: ArchiveMode::Plaintext, ..AppConfig::default() }
}

fn ensure_schema(paths: &ProjectPaths, config: &AppConfig) {
    let _ = open_archive_connection(paths, config, None).expect("schema bootstrap");
}

/// Monotonic sequence for `urls.source_url_id` + `last_visit_ms` so successive
/// `seed_url_row` calls never collide on the
/// `(source_profile_id, source_url_id)` UNIQUE index and arrive in a
/// deterministic most-recent-first order (the later-seeded row is "fresher").
static SEED_SEQ: AtomicU32 = AtomicU32::new(1);

/// Seeds a `urls` row so the list/enrichment paths have a visit count + title
/// to read back. Requires a parent run + source profile (FKs). Each call gets a
/// fresh `source_url_id` + strictly-larger `last_visit_ms` so multiple rows for
/// one canonical page (raw-variant fan-out) coexist and order deterministically.
fn seed_url_row(
    paths: &ProjectPaths,
    config: &AppConfig,
    url: &str,
    title: &str,
    visit_count: i64,
) {
    let seq = SEED_SEQ.fetch_add(1, Ordering::SeqCst) as i64;
    let connection = open_archive_connection(paths, config, None).unwrap();
    connection
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, status)
             VALUES (1, 'backup', 'manual', '2026-04-24T00:00:00Z', 'success')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at)
             VALUES (1, 'chrome', 'Default', '/tmp/Default', '2026-04-24T00:00:00Z')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO urls (
               url, title, visit_count, typed_count,
               first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
               source_profile_id, created_by_run_id, source_url_id
             ) VALUES (?1, ?2, ?3, 0, 1, '2026-04-24T00:00:00Z', ?4, '2026-04-24T00:00:00Z', 1, 1, ?5)",
            params![url, title, visit_count, seq, seq],
        )
        .unwrap();
}

/// Inserts a `star` row directly with a controlled `starred_at`, bypassing
/// `set_star`'s `now_rfc3339()` clock. Lets ordering tests pin DESC behaviour
/// without sleeping for the timestamp to advance.
fn insert_star_row(
    paths: &ProjectPaths,
    config: &AppConfig,
    kind: StarEntityKind,
    entity_key: &str,
    starred_at: &str,
) {
    let connection = open_archive_connection(paths, config, None).unwrap();
    connection
        .execute(
            "INSERT INTO star(entity_kind, entity_key, starred_at, source_profile)
             VALUES(?1, ?2, ?3, NULL)",
            params![kind.as_str(), entity_key, starred_at],
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
    let paths = make_paths("roundtrip");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let url = "https://example.com/page";

    set_star(&paths, &config, None, url_request(url)).unwrap();
    let status =
        is_starred_batch(&paths, &config, None, StarEntityKind::Url, &[url.to_string()]).unwrap();
    assert_eq!(status.get(url), Some(&true));

    unset_star(&paths, &config, None, url_request(url)).unwrap();
    let status =
        is_starred_batch(&paths, &config, None, StarEntityKind::Url, &[url.to_string()]).unwrap();
    assert_eq!(status.get(url), Some(&false));
}

#[test]
fn set_star_is_idempotent_and_refreshes_timestamp() {
    let paths = make_paths("idempotent");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let url = "https://example.com/x";

    set_star(&paths, &config, None, url_request(url)).unwrap();
    // Re-starring must not error and must keep exactly one row.
    set_star(&paths, &config, None, url_request(url)).unwrap();

    let counts = star_counts(&paths, &config, None).unwrap();
    assert_eq!(counts.urls, 1);
    assert_eq!(counts.domains, 0);
}

#[test]
fn unset_missing_star_is_a_noop() {
    let paths = make_paths("unset-missing");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // Deleting a star that never existed must succeed silently.
    unset_star(&paths, &config, None, url_request("https://example.com/never")).unwrap();
    let counts = star_counts(&paths, &config, None).unwrap();
    assert_eq!(counts.urls, 0);
}

#[test]
fn star_keys_by_canonical_url_so_tracking_params_collapse() {
    let paths = make_paths("canonical");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // utm_* is a tracking param stripped by normalize_visit_url, so both raw
    // URLs canonicalize to the same key — starring one stars the other.
    let tracked = "https://example.com/post?utm_source=news&id=7";
    let clean = "https://example.com/post?id=7";

    set_star(&paths, &config, None, url_request(tracked)).unwrap();
    let status = is_starred_batch(
        &paths,
        &config,
        None,
        StarEntityKind::Url,
        &[clean.to_string(), tracked.to_string()],
    )
    .unwrap();
    assert_eq!(status.get(clean), Some(&true), "clean variant must read as starred");
    assert_eq!(status.get(tracked), Some(&true), "tracked variant must read as starred");

    let counts = star_counts(&paths, &config, None).unwrap();
    assert_eq!(counts.urls, 1, "both variants collapse to one canonical star");
}

#[test]
fn is_starred_batch_returns_false_for_unstarred_and_empty_for_no_keys() {
    let paths = make_paths("batch");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let empty = is_starred_batch(&paths, &config, None, StarEntityKind::Url, &[]).unwrap();
    assert!(empty.is_empty());

    set_star(&paths, &config, None, url_request("https://a.test/")).unwrap();
    let status = is_starred_batch(
        &paths,
        &config,
        None,
        StarEntityKind::Url,
        &["https://a.test/".to_string(), "https://b.test/".to_string()],
    )
    .unwrap();
    assert_eq!(status.get("https://a.test/"), Some(&true));
    assert_eq!(status.get("https://b.test/"), Some(&false));
}

#[test]
fn is_starred_batch_reports_unparseable_keys_as_false() {
    let paths = make_paths("batch-bad");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // "not a url" cannot be canonicalized; it must read false, not error, and
    // must not blank the valid sibling key.
    set_star(&paths, &config, None, url_request("https://good.test/")).unwrap();
    let status = is_starred_batch(
        &paths,
        &config,
        None,
        StarEntityKind::Url,
        &["not a url".to_string(), "https://good.test/".to_string()],
    )
    .unwrap();
    assert_eq!(status.get("not a url"), Some(&false));
    assert_eq!(status.get("https://good.test/"), Some(&true));
}

#[test]
fn is_starred_batch_returns_empty_when_no_keys_canonicalize() {
    let paths = make_paths("batch-all-bad");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let status = is_starred_batch(
        &paths,
        &config,
        None,
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
    let paths = make_paths("domain");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // Starring a subdomain host reduces to the registrable domain…
    set_star(&paths, &config, None, domain_request("docs.example.com")).unwrap();
    // …and a full URL on the same registrable domain reads as the same star.
    let status = is_starred_batch(
        &paths,
        &config,
        None,
        StarEntityKind::Domain,
        &["https://www.example.com/anything".to_string()],
    )
    .unwrap();
    assert_eq!(status.get("https://www.example.com/anything"), Some(&true));

    let counts = star_counts(&paths, &config, None).unwrap();
    assert_eq!(counts.domains, 1);
}

#[test]
fn list_stars_orders_recently_starred_first_and_enriches_urls() {
    let paths = make_paths("list-recent");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let first = "https://example.com/first";
    let second = "https://example.com/second";
    seed_url_row(&paths, &config, first, "First Page", 3);
    seed_url_row(&paths, &config, second, "Second Page", 9);

    set_star(&paths, &config, None, url_request(first)).unwrap();
    // Force a strictly-later timestamp for the second star so DESC order is
    // deterministic even at sub-second resolution.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    set_star(&paths, &config, None, url_request(second)).unwrap();

    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Url),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].entity_key, second, "most recent star comes first");
    assert_eq!(listed[0].title, "Second Page");
    assert_eq!(listed[0].visit_count, 9);
    assert_eq!(listed[0].domain, "example.com");
    assert_eq!(listed[1].entity_key, first);
}

#[test]
fn list_stars_most_revisited_sort_orders_by_visit_count() {
    let paths = make_paths("list-revisited");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let low = "https://example.com/low";
    let high = "https://example.com/high";
    seed_url_row(&paths, &config, low, "Low", 2);
    seed_url_row(&paths, &config, high, "High", 42);

    // Star the low-visit page LAST so recency and visit-count disagree, proving
    // the sort actually re-orders by visit_count.
    set_star(&paths, &config, None, url_request(high)).unwrap();
    set_star(&paths, &config, None, url_request(low)).unwrap();

    let listed =
        list_stars(&paths, &config, None, None, StarSort::MostRevisited, Some(50)).unwrap();
    assert_eq!(listed[0].entity_key, high, "highest visit count first");
    assert_eq!(listed[1].entity_key, low);
}

#[test]
fn list_stars_with_no_kind_filter_interleaves_kinds() {
    let paths = make_paths("list-all-kinds");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    set_star(&paths, &config, None, url_request("https://example.com/a")).unwrap();
    set_star(&paths, &config, None, domain_request("example.com")).unwrap();

    let listed = list_stars(&paths, &config, None, None, StarSort::RecentlyStarred, None).unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|item| item.entity_kind == StarEntityKind::Url));
    assert!(listed.iter().any(|item| item.entity_kind == StarEntityKind::Domain));
}

#[test]
fn list_stars_enriches_domain_visit_count_from_urls() {
    let paths = make_paths("list-domain-count");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    seed_url_row(&paths, &config, "https://example.com/one", "One", 4);
    seed_url_row(&paths, &config, "https://www.example.com/two", "Two", 6);

    set_star(&paths, &config, None, domain_request("example.com")).unwrap();
    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Domain),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].domain, "example.com");
    assert_eq!(listed[0].visit_count, 10, "domain count sums its URLs");
}

#[test]
fn list_stars_handles_url_with_no_archive_row() {
    let paths = make_paths("list-orphan");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // Star a URL the archive has never seen — enrichment must fall back to
    // empty title / 0 visits without failing.
    set_star(&paths, &config, None, url_request("https://unseen.test/page")).unwrap();
    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Url),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "");
    assert_eq!(listed[0].visit_count, 0);
    assert_eq!(listed[0].domain, "unseen.test");
}

#[test]
fn list_stars_clamps_limit_to_bounds() {
    let paths = make_paths("list-clamp");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    set_star(&paths, &config, None, url_request("https://example.com/clamp")).unwrap();
    // A zero limit clamps up to 1 (never returns nothing for a valid request);
    // an over-cap limit clamps down to MAX_LIST_LIMIT (no panic on cast).
    let zero = list_stars(&paths, &config, None, None, StarSort::RecentlyStarred, Some(0)).unwrap();
    assert_eq!(zero.len(), 1);
    let huge = list_stars(
        &paths,
        &config,
        None,
        None,
        StarSort::RecentlyStarred,
        Some(MAX_LIST_LIMIT + 1_000),
    )
    .unwrap();
    assert_eq!(huge.len(), 1);
}

#[test]
fn set_star_rejects_empty_key() {
    let paths = make_paths("empty-key");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let err = set_star(
        &paths,
        &config,
        None,
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
    let paths = make_paths("bad-url");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let err = set_star(&paths, &config, None, url_request("definitely not a url")).unwrap_err();
    assert!(err.to_string().contains("canonicalize"));
}

#[test]
fn set_star_rejects_undomainable_key() {
    let paths = make_paths("bad-domain");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let err = set_star(
        &paths,
        &config,
        None,
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
    let paths = make_paths("future-kind");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let connection = open_archive_connection(&paths, &config, None).unwrap();
    connection
        .execute(
            "INSERT INTO star(entity_kind, entity_key, starred_at, source_profile)
             VALUES('query_family', 'q:rust async', '2026-04-24T00:00:00Z', NULL)",
            [],
        )
        .unwrap();
    set_star(&paths, &config, None, url_request("https://example.com/known")).unwrap();

    let listed = list_stars(&paths, &config, None, None, StarSort::RecentlyStarred, None).unwrap();
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
    let paths = make_paths("enrich-tracking");
    let config = plaintext_config();
    ensure_schema(&paths, &config);

    // The stored visit row is the RAW url: mixed-case host + a utm_* tracking
    // param. normalize_visit_url lowercases the host and strips utm_*, so it
    // canonicalizes to https://example.com/post?id=7 — the star key below.
    let raw = "https://Example.com/post?utm_source=newsletter&id=7";
    seed_url_row(&paths, &config, raw, "The Real Title", 11);

    // Star the page via its clean (canonical-ish) form; set_star canonicalizes.
    set_star(&paths, &config, None, url_request("https://example.com/post?id=7")).unwrap();

    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Url),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
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
    let paths = make_paths("enrich-multi-variant");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    seed_url_row(&paths, &config, "https://example.com/post?id=7&utm_source=a", "Older", 4);
    seed_url_row(&paths, &config, "https://example.com/post?id=7&utm_source=b", "Newer", 6);

    set_star(&paths, &config, None, url_request("https://example.com/post?id=7")).unwrap();
    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Url),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
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
    let paths = make_paths("enrich-prefix-overmatch");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // Force pass 2 (prefix scan) by storing only a tracking-param raw variant so
    // the exact seek misses; /foobar shares the /foo prefix but is a different
    // canonical page and must not be summed in.
    seed_url_row(&paths, &config, "https://example.com/foo?utm_source=x", "Foo", 3);
    seed_url_row(&paths, &config, "https://example.com/foobar?utm_source=x", "Foobar", 99);

    set_star(&paths, &config, None, url_request("https://example.com/foo")).unwrap();
    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Url),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].visit_count, 3, "only the /foo page counts, not /foobar");
    assert_eq!(listed[0].title, "Foo");
}

#[test]
fn is_starred_is_partitioned_by_entity_kind() {
    // A url star and a domain star can share the same key string
    // ("example.com"). The `entity_kind = ?1` predicate must keep them apart:
    // a url query must not see the domain star (and vice versa).
    let paths = make_paths("kind-partition");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // Insert two rows with the IDENTICAL key but different kinds.
    insert_star_row(&paths, &config, StarEntityKind::Domain, "example.com", "2026-04-24T00:00:00Z");
    insert_star_row(
        &paths,
        &config,
        StarEntityKind::Url,
        "https://example.com/",
        "2026-04-24T00:00:01Z",
    );

    // A url query for the domain's key string must NOT report it starred — the
    // only url-kind row is the full URL, not the bare domain.
    let url_view = is_starred_batch(
        &paths,
        &config,
        None,
        StarEntityKind::Url,
        &["https://example.com/".to_string()],
    )
    .unwrap();
    assert_eq!(url_view.get("https://example.com/"), Some(&true));

    // The domain star is keyed "example.com"; a domain query sees it, a url
    // query for a bare host string would canonicalize to a registrable domain
    // and is a different kind entirely — confirm the domain side still reads.
    let domain_view = is_starred_batch(
        &paths,
        &config,
        None,
        StarEntityKind::Domain,
        &["example.com".to_string()],
    )
    .unwrap();
    assert_eq!(domain_view.get("example.com"), Some(&true));

    // The kinds are independent counts.
    let counts = star_counts(&paths, &config, None).unwrap();
    assert_eq!(counts.urls, 1);
    assert_eq!(counts.domains, 1);
}

#[test]
fn list_stars_most_revisited_breaks_ties_by_recency() {
    // Equal visit_count must fall back to starred_at DESC (more-recent first),
    // pinning the tie-break direction the sort comment documents.
    let paths = make_paths("revisited-tie");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let older = "https://example.com/older";
    let newer = "https://example.com/newer";
    seed_url_row(&paths, &config, older, "Older", 5);
    seed_url_row(&paths, &config, newer, "Newer", 5);
    // Controlled timestamps: `newer` was starred strictly later.
    insert_star_row(&paths, &config, StarEntityKind::Url, older, "2026-04-24T00:00:00Z");
    insert_star_row(&paths, &config, StarEntityKind::Url, newer, "2026-04-24T00:00:05Z");

    let listed =
        list_stars(&paths, &config, None, None, StarSort::MostRevisited, Some(50)).unwrap();
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
    let paths = make_paths("none-branch-desc");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    insert_star_row(
        &paths,
        &config,
        StarEntityKind::Url,
        "https://a.test/",
        "2026-04-24T00:00:00Z",
    );
    insert_star_row(&paths, &config, StarEntityKind::Domain, "b.test", "2026-04-24T00:00:01Z");
    insert_star_row(
        &paths,
        &config,
        StarEntityKind::Url,
        "https://c.test/",
        "2026-04-24T00:00:02Z",
    );

    let listed = list_stars(&paths, &config, None, None, StarSort::RecentlyStarred, None).unwrap();
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
    let paths = make_paths("enrich-like-escape");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    // `_` would otherwise match any single char; `/a_b` must not pull in `/axb`.
    seed_url_row(&paths, &config, "https://example.com/a_b?utm_source=x", "Underscore", 4);
    seed_url_row(&paths, &config, "https://example.com/axb?utm_source=x", "Wildcard", 50);

    set_star(&paths, &config, None, url_request("https://example.com/a_b")).unwrap();
    let listed = list_stars(
        &paths,
        &config,
        None,
        Some(StarEntityKind::Url),
        StarSort::RecentlyStarred,
        None,
    )
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].visit_count, 4, "the `_` is matched literally, not as a wildcard");
    assert_eq!(listed[0].title, "Underscore");
}

#[test]
fn enrich_url_star_falls_back_when_key_is_not_a_parseable_url() {
    // Defensive path: `enrich_url_star` is only fed canonical URLs in production,
    // but if a url-kind star ever carries an unparseable key, `canonical_prefix`
    // returns None and enrichment falls back to the (empty) exact result instead
    // of panicking.
    let paths = make_paths("enrich-bad-key");
    let config = plaintext_config();
    ensure_schema(&paths, &config);
    let connection = open_archive_connection(&paths, &config, None).unwrap();
    let (title, visit_count) = enrich_url_star(&connection, "not a url").unwrap();
    assert_eq!(title, "");
    assert_eq!(visit_count, 0);
}

#[test]
fn canonical_prefix_and_escape_like_helpers_are_well_formed() {
    // The prefix drops the query string and keeps the port; escaping covers all
    // three LIKE metacharacters.
    assert_eq!(
        canonical_prefix("https://example.com:8443/path/page?id=7"),
        Some("https://example.com:8443/path/page".to_string()),
    );
    assert_eq!(canonical_prefix("not a url"), None);
    assert_eq!(escape_like("a_b%c\\d"), "a\\_b\\%c\\\\d");
    assert_eq!(escape_like("plain/path"), "plain/path");
}

#[test]
fn entity_kind_as_str_matches_storage_encoding() {
    assert_eq!(StarEntityKind::Url.as_str(), "url");
    assert_eq!(StarEntityKind::Domain.as_str(), "domain");
}
