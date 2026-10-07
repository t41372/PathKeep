//! Responsibilities: native AI working set regression coverage.
//! Not responsible for: search or network transport.
//! Failure modes: omitted records, stale state, non-durable progress, and incorrect provider scope.
use super::*;

#[test]
fn select_working_set_ranks_starred_first_then_other_signals() {
    // W-AI-4c heavy-working-set selector (05 §4/§8): the shared hook ranks unique-content candidates
    // by starred (top) ∪ recent ∪ annotated ∪ frequency, bounded + indexed off the archive.
    let (_paths, _config, _intelligence) = prepared_archive();
    let vault = _intelligence.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let archive = TestDatabase::new(&vault, Db::Archive);

    // Seed four distinct pages with different signal profiles.
    let now_ms = 1_000_000_000_000_i64;
    let recent_ms = now_ms - 1; // inside any window
    let old_ms = now_ms - 5 * 365 * 86_400_000; // ~5 years ago, outside an 18-month window
    let seed_url = |id: i64, url: &str, visit_count: i64, last_visit_ms: i64| {
        archive
            .execute(
                "INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?3,?4)",
                params![
                    id,
                    url,
                    crate::pages::canonical_key(url),
                    crate::visit_taxonomy::registrable_domain_for_url(url)
                ],
            )
            .unwrap();
        archive.execute("INSERT INTO urls(id,page_id,url,title,visit_count,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,?3,?4,1,1,?5,'',?5,'')",params![id,url,format!("Title {id}"),visit_count,last_visit_ms]).unwrap();
        archive.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,updated_at,newest_visit_ms) VALUES(?1,?2,'','','','','','','','',?3)",params![id,url,last_visit_ms]).unwrap();
        if visit_count > 0 {
            archive.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms) VALUES(?1,1,?2,?3)",params![id,visit_count,last_visit_ms]).unwrap();
        }
    };
    // A run + profile so the FK constraints hold.
    archive
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', '2026-01-01', 'UTC', 'success', '[]', '[]', '{}', 0)",
            (),
        )
        .expect("run");
    archive
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (1, 'chrome', 'x', 'p', '/p', '2026-01-01', 1, 'chrome:Default', '2026-01-01')",
            (),
        )
        .expect("profile");

    seed_url(1, "https://starred.com/page", 1, old_ms); // STARRED (top weight) but old
    seed_url(2, "https://recent.com/page", 1, recent_ms); // recent only
    seed_url(3, "https://frequent.com/page", 9000, old_ms); // very frequent but old
    seed_url(4, "https://noted.com/page", 1, old_ms); // annotated only
    seed_url(5, "https://cold.com/page", 0, old_ms); // NO active signal → excluded
    // A SECOND raw variant of the recent page, with a HIGHER visit count and old last-visit. It
    // collapses onto the recent page's canonical URL, so the frequency gather must lift that
    // candidate's visit_count to this higher value (the max-across-variants branch).
    seed_url(6, "https://recent.com/page?utm_source=ad", 42, old_ms);

    // Star page 1 (canonical key), annotate page 4 (note), TAG page 2 (the tags gather), and add an
    // unknown star kind that must be ignored (a future query_family star, not errored).
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            (),
        )
        .expect("star");
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('query_family', 'some-future-key', '2026-01-01')",
            (),
        )
        .expect("future star kind");
    archive
        .execute(
            "INSERT INTO url_annotations (url,canonical_url,notes,created_at,updated_at) VALUES('https://noted.com/page','https://noted.com/page','keep this','2026-01-01','2026-01-01')",
            (),
        )
        .expect("note");
    archive
        .execute(
            "INSERT INTO url_tags(url,canonical_url,tag,created_at) VALUES('https://recent.com/page','https://recent.com/page','work','2026-01-01')",
            (),
        )
        .expect("tag");

    let config = crate::WorkingSetConfig::default();
    let mut candidates =
        crate::select_working_set(&vault, &config, now_ms, 10, None).expect("select").items;

    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    // The cold page (no signal) is excluded.
    assert!(!candidates.iter().any(|c| c.canonical_url.contains("cold.com")));
    // Starred ranks first (top weight beats frequency/recency/annotation alone).
    assert!(candidates[0].canonical_url.contains("starred.com"));
    assert!(candidates[0].signals.starred);
    // Four active pages are present (the second recent.com variant collapsed onto the first).
    assert_eq!(candidates.len(), 4);
    // The frequent page carries its visit count signal; the annotated page is flagged annotated.
    let frequent = candidates.iter().find(|c| c.canonical_url.contains("frequent.com")).unwrap();
    assert_eq!(frequent.signals.visit_count, 9000);
    let noted = candidates.iter().find(|c| c.canonical_url.contains("noted.com")).unwrap();
    assert!(noted.signals.annotated);
    // The recent page is flagged recent AND annotated (via the tag) AND carries the MAX visit count
    // across its two raw variants (the higher 42, not the original 1) — the max-across-variants path.
    let recent = candidates.iter().find(|c| c.canonical_url.contains("recent.com")).unwrap();
    assert!(recent.signals.recent);
    assert!(recent.signals.annotated, "the tag marks the recent page annotated");
    assert_eq!(recent.signals.visit_count, 43, "visible visits sum across canonical variants");

    // A zero limit returns nothing.
    assert_eq!(
        crate::select_working_set(&vault, &config, now_ms, 0, None)
            .expect("minimum page")
            .items
            .len(),
        1
    );
}

#[test]
fn select_working_set_marks_domain_starred_pages() {
    // A DOMAIN star marks every page on that registrable domain (the user starred the source).
    let (_paths, _config, _intelligence) = prepared_archive();
    let vault = _intelligence.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let archive = TestDatabase::new(&vault, Db::Archive);
    archive
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only) VALUES (1, 'backup', 't', '2026', 'UTC', 'success', '[]', '[]', '{}', 0)",
            (),
        )
        .expect("run");
    archive
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at) VALUES (1, 'chrome', 'x', 'p', '/p', '2026', 1, 'chrome:Default', '2026')",
            (),
        )
        .expect("profile");
    archive.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://github.com/a/repo','https://github.com/a/repo','github.com')",()).unwrap();
    archive
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, page_id) VALUES (1, 'https://github.com/a/repo', 'Repo', 3, 0, 1, '', 1, '', 1, 1, 1)",
            (),
        )
        .expect("url");
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('domain', 'github.com', '2026')",
            (),
        )
        .expect("domain star");

    let candidates = crate::select_working_set(
        &vault,
        &crate::WorkingSetConfig::default(),
        1_000_000_000_000,
        10,
        None,
    )
    .expect("select")
    .items;
    let github = candidates.iter().find(|c| c.canonical_url.contains("github.com")).unwrap();
    assert!(github.signals.starred, "a page on a starred domain inherits the starred signal");
}

#[test]
fn estimate_reembed_full_counts_unique_pages_and_sizes_cost() {
    // W-AI-9-D: a Full re-embed estimate counts unique pages (`COUNT(*) FROM urls`) and applies the
    // S1 throughput constants. Three seeded pages → 3 / 1.25 / 60 CPU minutes, GPU = /10.
    let (_paths, _config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/a", Some("A"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/b", Some("B"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/c", Some("C"), 3);
    drop(connection);

    let _archive = TestDatabase::new(&vault, Db::Archive);
    let estimate = crate::estimate_reembed(
        &vault,
        &crate::WorkingSetConfig::default(),
        1_000_000_000_000,
        crate::ReembedScope::Full,
    )
    .expect("estimate full");
    assert_eq!(estimate.scope, crate::ReembedScope::Full);
    assert_eq!(estimate.page_count, 3);
    assert!((estimate.est_minutes_cpu - 3.0 / 1.25 / 60.0).abs() < 1e-9);
    assert!((estimate.est_minutes_gpu - estimate.est_minutes_cpu / 10.0).abs() < 1e-9);
    // Honest: in this CPU-only coverage build the GPU path is unavailable.
    assert_eq!(estimate.gpu_available, cfg!(feature = "metal"));
}

#[test]
fn estimate_reembed_working_set_counts_only_the_bounded_set() {
    // The WorkingSet estimate counts only the bounded working set: here one starred page is a member,
    // a cold page (no signal) is not — so the estimate's page_count is 1, not the whole archive.
    let (_paths, _config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let archive = TestDatabase::new(&vault, Db::Archive);
    seed_visit(&connection, 1, "chrome:Default", "https://starred.com/page", Some("Star"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://cold.com/page", Some("Cold"), 2);
    // `seed_visit` gives every page visit_count=1 (a frequency signal). Zero the cold page's count so
    // it has NO active signal and is genuinely outside the working set (the starred page is the lone
    // member). An old last_visit keeps it out of the recency window too.
    archive
        .execute("DELETE FROM search_document_profiles WHERE page_id=2", ())
        .expect("zero cold page");
    drop(connection);

    let archive = TestDatabase::new(&vault, Db::Archive);
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            (),
        )
        .expect("star");

    let estimate = crate::estimate_reembed(
        &vault,
        &crate::WorkingSetConfig::default(),
        1_000_000_000_000,
        crate::ReembedScope::WorkingSet,
    )
    .expect("estimate working set");
    assert_eq!(estimate.scope, crate::ReembedScope::WorkingSet);
    assert_eq!(estimate.page_count, 1, "only the starred page is a working-set member");
}

#[test]
fn build_ai_index_working_set_scope_embeds_only_working_set_pages() {
    // W-AI-9-D: a WorkingSet re-embed touches ONLY the bounded heavy-tier set. The starred page is
    // embedded; the cold (no-signal) page is skipped even though it is a fresh candidate.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://starred.com/page", Some("Star"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://cold.com/page", Some("Cold"), 2);
    // Zero the cold page's signals (see the estimate test) so it is genuinely outside the working set.
    archive
        .execute("DELETE FROM search_document_profiles WHERE page_id=2", ())
        .expect("zero cold page");
    // Star page 1 so it (and only it) is in the working set.
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            (),
        )
        .expect("star");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &vault.with_config(config.clone()),
            &provider,
            &AiIndexRequest { scope: crate::ReembedScope::WorkingSet, ..AiIndexRequest::default() },
        ))
        .expect("working-set build");
    // Only the starred page embedded; the cold page is counted as skipped (out of scope).
    assert_eq!(report.indexed_items, 1);
    assert!(report.skipped_items >= 1, "the cold page is skipped (out of working set)");
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 1, "one vector — only the working-set page");
}
