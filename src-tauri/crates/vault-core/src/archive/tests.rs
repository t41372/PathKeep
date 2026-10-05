//! Regression tests for the canonical archive domain.
use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    models::{
        ArchiveMode, BrowserProfile, ExportFormat, ExportRequest, HistoryFaviconLookupEntry,
        RetentionPruneRequest, SnapshotRestoreRequest, TakeoutRequest,
    },
    utils::{restore_test_env_var, test_env_lock},
};
use browser_history_parser::{ContextEvidence, NativeEntity, TypedEvidenceBatch};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashSet};
use tempfile::tempdir;

const TEST_CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

fn sample_paths(root: &Path) -> ProjectPaths {
    project_paths_with_root(root)
}

/// Inserts a uniquely-tagged run row so a rekey test can prove the canonical
/// rows survived (the export copies it forward, so it is present in BOTH the
/// original and the rekeyed file). `runs` is a root table, so this needs no FK
/// scaffolding.
fn seed_rekey_marker(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) {
    let connection =
        open_archive_connection(paths, config, key).expect("open archive to seed rekey marker");
    connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, status)
             VALUES ('backup', 'rekey-marker', '2026-06-30T00:00:00Z', 'success')",
            [],
        )
        .expect("seed rekey marker run");
}

/// Counts the seeded marker rows, opening the archive in the given mode/key.
fn rekey_marker_count(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) -> i64 {
    let connection =
        open_archive_connection(paths, config, key).expect("open archive to count rekey marker");
    connection
        .query_row("SELECT COUNT(*) FROM runs WHERE trigger = 'rekey-marker'", [], |row| row.get(0))
        .expect("count rekey marker rows")
}

/// Counts the verified before-rekey safety snapshots on disk.
fn rekey_snapshot_count(paths: &ProjectPaths) -> usize {
    let rekey_dir = paths.raw_snapshots_dir.join("rekey");
    fs::read_dir(&rekey_dir)
        .map(|entries| {
            entries.filter_map(|entry| entry.ok()).filter(|entry| entry.path().is_file()).count()
        })
        .unwrap_or(0)
}

pub(super) fn seed_chrome_fixture(root: &Path) -> PathBuf {
    let chrome_root = root.join("chrome-user-data");
    let profile_dir = chrome_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
    fs::write(
        chrome_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tim@example.com"}}}}"#,
    )
    .expect("write local state");

    let history = Connection::open(profile_dir.join("History")).expect("open history");
    history
        .execute_batch(
            "CREATE TABLE urls (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER NOT NULL,
               typed_count INTEGER NOT NULL,
               last_visit_time INTEGER NOT NULL,
               hidden INTEGER NOT NULL
             );
             CREATE TABLE visits (
               id INTEGER PRIMARY KEY,
               url INTEGER NOT NULL,
               visit_time INTEGER NOT NULL,
               from_visit INTEGER,
               transition INTEGER,
               visit_duration INTEGER,
               is_known_to_sync INTEGER,
               visited_link_id INTEGER,
               external_referrer_url TEXT,
               app_id TEXT
             );
             CREATE TABLE downloads (
               id INTEGER PRIMARY KEY,
               guid TEXT,
               current_path TEXT,
               target_path TEXT,
               start_time INTEGER,
               received_bytes INTEGER,
               total_bytes INTEGER,
               state INTEGER,
               mime_type TEXT,
               original_mime_type TEXT
             );
             CREATE TABLE keyword_search_terms (
               keyword_id INTEGER,
               url_id INTEGER,
               term TEXT,
               normalized_term TEXT
             );",
        )
        .expect("create history tables");
    let first_visit = crate::utils::iso_to_chrome_time_micros("2026-04-05T10:00:00+00:00")
        .expect("first visit time");
    let second_visit = crate::utils::iso_to_chrome_time_micros("2026-04-05T11:00:00+00:00")
        .expect("second visit time");
    history
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
             VALUES (1, 'https://example.com/archive', 'Archive docs', 2, 0, ?1, 0)",
            [second_visit],
        )
        .expect("insert url");
    history
        .execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
             VALUES
             (1, 1, ?1, NULL, 805306368, 24000, 1, NULL, 'https://google.com', NULL),
             (2, 1, ?2, 1, 805306368, 12000, 1, NULL, NULL, NULL)",
            params![first_visit, second_visit],
        )
        .expect("insert visits");
    history
        .execute(
            "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
             VALUES (9, 'guid-9', '/tmp/archive.pdf', '/tmp/archive.pdf', ?1, 10, 10, 1, 'application/pdf', 'application/pdf')",
            [second_visit],
        )
        .expect("insert download");
    history
        .execute(
            "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
             VALUES (1, 1, 'deep recall token', 'deep recall token')",
            [],
        )
        .expect("insert search term");

    let favicons = Connection::open(profile_dir.join("Favicons")).expect("open favicons");
    favicons
        .execute_batch(
            "CREATE TABLE favicons (id INTEGER PRIMARY KEY, url TEXT NOT NULL, icon_type INTEGER);
             CREATE TABLE icon_mapping (page_url TEXT NOT NULL, icon_id INTEGER NOT NULL);
             CREATE TABLE favicon_bitmaps (icon_id INTEGER NOT NULL, width INTEGER, height INTEGER, last_updated INTEGER, image_data BLOB);",
        )
        .expect("create favicons tables");
    favicons
        .execute(
            "INSERT INTO favicons (id, url, icon_type) VALUES (1, 'https://example.com/favicon.ico', 1)",
            [],
        )
        .expect("insert favicon");
    favicons
        .execute(
            "INSERT INTO favicons (id, url, icon_type) VALUES (2, 'https://example.com/app-icon.ico', 1)",
            [],
        )
        .expect("insert duplicate favicon");
    favicons
        .execute(
            "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/archive', 1)",
            [],
        )
        .expect("insert icon mapping");
    favicons
        .execute(
            "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/archive', 2)",
            [],
        )
        .expect("insert duplicate icon mapping");
    favicons
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (1, 16, 16, ?1, X'89504E470D0A1A0A01')",
            [second_visit],
        )
        .expect("insert favicon bitmap");
    favicons
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (2, 16, 16, ?1, X'89504E470D0A1A0A01')",
            [second_visit],
        )
        .expect("insert duplicate favicon bitmap");

    chrome_root
}

fn seed_missing_chrome_history_fixture(root: &Path) -> PathBuf {
    let chrome_root = root.join("chrome-missing-history");
    let profile_dir = chrome_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create missing chrome profile dir");
    fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
    fs::write(
        chrome_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tim@example.com"}}}}"#,
    )
    .expect("write local state");
    chrome_root
}

fn seed_lexical_archive(paths: &ProjectPaths, config: &AppConfig) {
    let connection = open_archive_connection(paths, config, None).expect("open archive");
    connection
        .execute(
            "INSERT INTO source_profiles (
               id,
               browser_kind,
               browser_version,
               profile_name,
               profile_path,
               discovered_at,
               enabled,
               profile_key,
               user_name,
               updated_at,
               browser_product
             )
             VALUES (1, 'chrome', '146.0.0.0', 'Default', '/tmp/profile', '2026-05-01T00:00:00+00:00', 1, 'chrome:Default', NULL, '2026-05-01T00:00:00+00:00', 'Google Chrome')",
            [],
        )
        .expect("insert source profile");

    let rows = [
        (
            1,
            "https://example.test/preferences",
            "瀏覽器設定中心",
            1_000,
            "2026-05-01T00:00:01+00:00",
        ),
        (
            2,
            "https://example.test/simplified",
            "浏览器设定说明",
            2_000,
            "2026-05-01T00:00:02+00:00",
        ),
        (
            3,
            "https://example.test/github-actions",
            "GitHub Actions manual",
            3_000,
            "2026-05-01T00:00:03+00:00",
        ),
        (4, "https://github.com/releases", "Release notes", 4_000, "2026-05-01T00:00:04+00:00"),
        (
            5,
            "https://example.test/git-hub-guide",
            "Git Hub spacing guide",
            5_000,
            "2026-05-01T00:00:05+00:00",
        ),
        (
            6,
            "https://www.youtube.com/watch?v=recall",
            "YouTube watch queue",
            6_000,
            "2026-05-01T00:00:06+00:00",
        ),
        (
            7,
            "https://example.test/reviews/pull-request",
            "Pull Request review checklist",
            7_000,
            "2026-05-01T00:00:07+00:00",
        ),
    ];

    for (id, url, title, visit_time, iso) in rows {
        connection
            .execute(
                "INSERT INTO urls (
                   id,
                   url,
                   title,
                   visit_count,
                   typed_count,
                   first_visit_ms,
                   first_visit_iso,
                   last_visit_ms,
                   last_visit_iso,
                   source_profile_id,
                   created_by_run_id,
                   source_url_id,
                   hidden,
                   recorded_at
                 )
                 VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, 1, 0, ?1, 0, ?5)",
                params![id, url, title, visit_time, iso],
            )
            .expect("insert url");
        connection
            .execute(
                "INSERT INTO visits (
                   id,
                   url_id,
                   source_visit_id,
                   visit_time_ms,
                   visit_time_iso,
                   transition_type,
                   visit_duration_ms,
                   source_profile_id,
                   created_by_run_id,
                   app_id,
                   from_visit,
                   is_known_to_sync,
                   recorded_at
                 )
                 VALUES (?1, ?1, ?1, ?2, ?3, 805306368, 1000, 1, 0, NULL, NULL, 1, ?3)",
                params![id, visit_time, iso],
            )
            .expect("insert visit");
    }

    rebuild_search_projection(paths, config, None).expect("rebuild search projection");
}

fn insert_lexical_history_row(
    paths: &ProjectPaths,
    config: &AppConfig,
    id: i64,
    url: &str,
    title: &str,
    visit_time: i64,
    iso: &str,
) {
    let connection = open_archive_connection(paths, config, None).expect("open archive");
    connection
        .execute(
            "INSERT INTO urls (
               id,
               url,
               title,
               visit_count,
               typed_count,
               first_visit_ms,
               first_visit_iso,
               last_visit_ms,
               last_visit_iso,
               source_profile_id,
               created_by_run_id,
               source_url_id,
               hidden,
               recorded_at
             )
             VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, 1, 0, ?1, 0, ?5)",
            params![id, url, title, visit_time, iso],
        )
        .expect("insert url");
    connection
        .execute(
            "INSERT INTO visits (
               id,
               url_id,
               source_visit_id,
               visit_time_ms,
               visit_time_iso,
               transition_type,
               visit_duration_ms,
               source_profile_id,
               created_by_run_id,
               app_id,
               from_visit,
               is_known_to_sync,
               recorded_at
             )
             VALUES (?1, ?1, ?1, ?2, ?3, 805306368, 1000, 1, 0, NULL, NULL, 1, ?3)",
            params![id, visit_time, iso],
        )
        .expect("insert visit");
    rebuild_search_projection(paths, config, None).expect("rebuild search projection");
}

/// Stores one successful GitHub-repo enrichment for `history_id` (intelligence plane) and re-mirrors
/// it into the search projection so a subsequent `list_history` lexical search can surface the excerpt.
///
/// `summary` doubles as the GitHub `description` (the indexer de-dupes the two) and `topics` carries an
/// extra searchable keyword, mirroring how `enrichment_text_for_index` builds the projected text.
fn store_lexical_enrichment(
    paths: &ProjectPaths,
    config: &AppConfig,
    history_id: i64,
    summary: &str,
) {
    let intelligence =
        open_intelligence_connection(paths, config, None).expect("open intelligence");
    crate::enrichment::ensure_visit_content_enrichment_schema(&intelligence)
        .expect("enrichment schema");
    intelligence
        .execute(
            "INSERT OR REPLACE INTO visit_content_enrichments
             (history_id, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
              pipeline_version, extractor_version, enrichment_summary)
             VALUES (?1, 'github-repo', 'success', '2026-06-21T00:00:00Z', '[]', ?2, 'v1', 1, ?3)",
            params![
                history_id,
                format!(
                    r#"{{"fullName":"o/r","description":"{summary}","topics":["enrichkw"],"language":"Rust"}}"#
                ),
                summary,
            ],
        )
        .expect("insert enrichment");
    drop(intelligence);
    // Mirror the just-stored enrichment into the projection's enrichment_text + its FTS column.
    rebuild_search_projection(paths, config, None).expect("rebuild search projection");
}

#[test]
fn lexical_search_surfaces_capped_enrichment_excerpt_only_for_enriched_results() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);
    // Enrich ONE of the seeded rows (url/visit id 3 = the GitHub Actions page) with a summary the
    // search query will hit, and a `topics` keyword ("enrichkw") that only lives in enrichment_text.
    store_lexical_enrichment(&paths, &config, 3, "Reusable workflow runner for CI pipelines");

    // A keyword that matches the enriched page surfaces a capped excerpt on THAT result.
    let enriched = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("github".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("enriched query");
    let actions_entry = enriched
        .items
        .iter()
        .find(|entry| entry.title.as_deref() == Some("GitHub Actions manual"))
        .expect("the enriched GitHub Actions row is recalled");
    // The excerpt mirrors the projected enrichment_text: the summary plus the searchable metadata
    // (topics/desc/language) the index carries — NOT just the bare summary.
    let actions_excerpt =
        actions_entry.enrichment_excerpt.as_deref().expect("the enriched row carries an excerpt");
    assert!(
        actions_excerpt.contains("Reusable workflow runner for CI pipelines"),
        "an enriched search result must surface its enrichment summary: {actions_excerpt}"
    );
    assert!(
        actions_excerpt.contains("enrichkw"),
        "the excerpt carries the searchable enrichment metadata too: {actions_excerpt}"
    );
    // A different recalled result with NO stored enrichment leaves the excerpt None.
    let plain_entry = enriched
        .items
        .iter()
        .find(|entry| entry.title.as_deref() == Some("Git Hub spacing guide"))
        .expect("the non-enriched Git Hub row is recalled");
    assert_eq!(
        plain_entry.enrichment_excerpt, None,
        "a result without stored enrichment must leave the excerpt None"
    );

    // The enrichment text itself is keyword-searchable, and that match also carries the excerpt.
    let by_enrichment_keyword = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("enrichkw".to_string()),
            limit: Some(10),
            ..HistoryQuery::default()
        },
    )
    .expect("enrichment-keyword query");
    assert_eq!(
        by_enrichment_keyword.total, 1,
        "a token that lives only in enrichment_text must recall exactly the enriched page"
    );
    assert!(
        by_enrichment_keyword.items[0]
            .enrichment_excerpt
            .as_deref()
            .is_some_and(|excerpt| excerpt.contains("Reusable workflow runner for CI pipelines")),
        "the enrichment-keyword match also carries the excerpt"
    );
}

#[test]
fn lexical_search_caps_long_enrichment_excerpt_on_a_cjk_char_boundary() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);
    // A long CJK summary (> the 180-char cap) on the GitHub Actions row: the excerpt must be capped on
    // a CHAR boundary (never panicking by splitting a multi-byte codepoint) and gain a trailing "…".
    let long_summary = "工作流程".repeat(80); // 320 CJK chars, well over the cap.
    store_lexical_enrichment(&paths, &config, 3, &long_summary);

    let response = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("github".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("capped query");
    let excerpt = response
        .items
        .iter()
        .find(|entry| entry.title.as_deref() == Some("GitHub Actions manual"))
        .and_then(|entry| entry.enrichment_excerpt.clone())
        .expect("the enriched row carries a capped excerpt");
    // 180 capped chars + the trailing ellipsis = 181 chars; the long input was truncated, not panicked.
    assert_eq!(excerpt.chars().count(), 181, "the excerpt is capped on a char boundary");
    assert!(excerpt.ends_with('…'), "a truncated excerpt gains a trailing ellipsis");
    assert!(excerpt.starts_with("工作流程"), "CJK codepoints survive the char-boundary cap");
}

#[test]
fn plain_browse_history_query_leaves_enrichment_excerpt_none() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);
    // Enrichment exists for a row, but a non-search browse query must NOT attach any excerpt: the
    // excerpt is a lexical-search-only affordance, so browse rows stay None regardless.
    store_lexical_enrichment(&paths, &config, 3, "Reusable workflow runner for CI pipelines");

    let browse = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(50), ..HistoryQuery::default() },
    )
    .expect("plain browse query");
    assert!(browse.total >= 7, "the browse path returns the full seeded set");
    assert!(
        browse.items.iter().all(|entry| entry.enrichment_excerpt.is_none()),
        "browse rows must never carry an enrichment excerpt"
    );
}

#[test]
fn uncounted_history_pages_walk_the_same_rows_as_counted_pages() {
    // History's infinite list sends `includeTotal: false` so a page never pays an archive-wide
    // COUNT. Walking with the cursor must still reach every row exactly once and stop cleanly.
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);

    let walk = |q: Option<&str>| {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = list_history(
                &paths,
                &config,
                None,
                HistoryQuery {
                    q: q.map(str::to_string),
                    limit: Some(3),
                    cursor: cursor.clone(),
                    include_total: Some(false),
                    ..HistoryQuery::default()
                },
            )
            .expect("uncounted page");
            assert!(!page.total_exact, "an uncounted page must say its total is not exact");
            assert_eq!(page.total, page.items.len(), "the total is only this page's size");
            assert_eq!(page.has_previous, cursor.is_some());
            seen.extend(page.items.iter().map(|entry| entry.url.clone()));
            if !page.has_next {
                assert!(page.next_cursor.is_none());
                break seen;
            }
            cursor = Some(page.next_cursor.expect("a page with more rows carries a cursor"));
        }
    };

    let counted = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(50), ..HistoryQuery::default() },
    )
    .expect("counted browse");
    assert!(counted.total_exact);
    let browse = walk(None);
    assert_eq!(browse, counted.items.iter().map(|entry| entry.url.clone()).collect::<Vec<_>>());

    let keyword = walk(Some("example"));
    let counted_keyword = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("example".to_string()), limit: Some(50), ..HistoryQuery::default() },
    )
    .expect("counted keyword");
    assert_eq!(keyword.len(), counted_keyword.total);
    let mut unique = keyword.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), keyword.len(), "no row may appear on two pages");
}

#[test]
fn lexical_recall_matches_cjk_script_folding_and_compact_substrings() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);

    let simplified_query = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("设定".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("simplified query");
    assert_eq!(simplified_query.total, 2);
    assert!(
        simplified_query.items.iter().any(|entry| entry.title.as_deref() == Some("浏览器设定说明"))
    );
    assert!(
        simplified_query.items.iter().any(|entry| entry.title.as_deref() == Some("瀏覽器設定中心")),
        "simplified query should recall the traditional indexed title"
    );

    let traditional_query = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("設定".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("traditional query");
    assert_eq!(traditional_query.total, 2);
    assert!(
        traditional_query
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("瀏覽器設定中心"))
    );
    assert!(
        traditional_query
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("浏览器设定说明")),
        "traditional query should recall the simplified indexed title"
    );

    let cjk_substring = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("器设".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("cjk substring query");
    assert_eq!(cjk_substring.total, 2);

    let compact_latin = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("github".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("compact latin query");
    assert!(
        compact_latin
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("Git Hub spacing guide")),
        "compact trigram recall should match Git Hub when the query is github"
    );

    let full_width_latin = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("ＧｉｔＨｕｂ".to_string()),
            limit: Some(10),
            ..HistoryQuery::default()
        },
    )
    .expect("full-width latin query");
    assert!(
        full_width_latin
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("GitHub Actions manual")),
        "NFKC recall should fold full-width GitHub into the indexed latin form"
    );
}

#[test]
fn lexical_recall_expands_aliases_and_uses_bounded_fuzzy_fallback() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);

    let github_alias = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("gh".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("github alias query");
    assert!(
        github_alias
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("GitHub Actions manual")),
        "gh should expand to github before FTS recall"
    );

    let youtube_alias = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("yt".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("youtube alias query");
    assert!(
        youtube_alias
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("YouTube watch queue")),
        "yt should expand to youtube before FTS recall"
    );

    let pull_request_alias = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("pr".to_string()), limit: Some(10), ..HistoryQuery::default() },
    )
    .expect("pull request alias query");
    assert!(
        pull_request_alias
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("Pull Request review checklist")),
        "pr should expand to pull request before FTS recall"
    );

    let fuzzy_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("gihub".to_string()), limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("fuzzy typo query");
    assert_eq!(fuzzy_first_page.total, 3);
    assert_eq!(fuzzy_first_page.items[0].title.as_deref(), Some("GitHub Actions manual"));
    assert!(fuzzy_first_page.next_cursor.as_deref().is_some_and(|cursor| cursor.starts_with("r|")));

    let fuzzy_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("gihub".to_string()),
            limit: Some(1),
            cursor: fuzzy_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy typo second page");
    assert_eq!(fuzzy_second_page.items.len(), 1);
    assert!(fuzzy_second_page.has_previous);
    let fuzzy_second_page_by_number = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("gihub".to_string()),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy typo explicit second page");
    assert_eq!(fuzzy_second_page_by_number.items.len(), 1);
    assert_eq!(fuzzy_second_page_by_number.page, 2);

    let fuzzy_newest = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("newest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy newest query");
    assert_eq!(fuzzy_newest.items[0].title.as_deref(), Some("Git Hub spacing guide"));
    let fuzzy_newest_second = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("newest".to_string()),
            limit: Some(1),
            cursor: fuzzy_newest.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy newest second page");
    assert_eq!(fuzzy_newest_second.items[0].title.as_deref(), Some("Release notes"));

    let fuzzy_oldest = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy oldest query");
    assert_eq!(fuzzy_oldest.items[0].title.as_deref(), Some("GitHub Actions manual"));
    let fuzzy_oldest_second = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("gihub".to_string()),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            cursor: fuzzy_oldest.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("fuzzy oldest second page");
    assert_eq!(fuzzy_oldest_second.items[0].title.as_deref(), Some("Release notes"));
}

#[test]
fn lexical_recall_defaults_to_relevance_and_accepts_time_sort_override() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);

    let relevance = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("github".to_string()), limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("relevance query");
    assert_eq!(relevance.items[0].title.as_deref(), Some("GitHub Actions manual"));
    assert!(relevance.next_cursor.as_deref().is_some_and(|cursor| cursor.starts_with("r|")));

    let explicit_relevance = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("github".to_string()),
            sort: Some("relevance".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("explicit relevance query");
    assert_eq!(explicit_relevance.items[0].title.as_deref(), Some("GitHub Actions manual"));

    let second_relevance_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("github".to_string()),
            limit: Some(1),
            cursor: relevance.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("second relevance page");
    assert_eq!(second_relevance_page.items.len(), 1);
    assert!(second_relevance_page.has_previous);

    let legacy_cursor_relevance_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("github".to_string()),
            sort: Some("relevance".to_string()),
            limit: Some(1),
            cursor: Some("4000|4".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("legacy chronological cursor with relevance query");
    assert_eq!(
        legacy_cursor_relevance_page.items[0].title.as_deref(),
        Some("GitHub Actions manual")
    );

    let newest = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("github".to_string()),
            sort: Some("newest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("newest query");
    assert_eq!(newest.items[0].title.as_deref(), Some("Git Hub spacing guide"));
    assert!(newest.next_cursor.as_deref().is_some_and(|cursor| !cursor.starts_with("r|")));
}

#[test]
fn history_keyword_query_supports_google_like_local_operators() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);
    insert_lexical_history_row(
        &paths,
        &config,
        8,
        "https://github.com/pathkeep/pathkeep/issues",
        "PathKeep issue tracker",
        8_000,
        "2026-05-01T00:00:08+00:00",
    );
    insert_lexical_history_row(
        &paths,
        &config,
        9,
        "https://github.com/pathkeep/spec.pdf",
        "PathKeep PDF spec",
        9_000,
        "2026-05-01T00:00:09+00:00",
    );

    let site_without_keyword = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("site:github.com -pathkeep".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("site exclusion query");
    assert_eq!(site_without_keyword.total, 1);
    assert_eq!(site_without_keyword.items[0].title.as_deref(), Some("Release notes"));

    let domain_without_keyword = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("-pathkeep".to_string()),
            domain: Some("github.com".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("domain exclusion query");
    assert_eq!(domain_without_keyword.total, 1);
    assert_eq!(domain_without_keyword.items[0].title.as_deref(), Some("Release notes"));

    let exact_phrase = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("\"release notes\"".to_string()), ..HistoryQuery::default() },
    )
    .expect("exact phrase query");
    assert_eq!(exact_phrase.total, 1);
    assert_eq!(exact_phrase.items[0].title.as_deref(), Some("Release notes"));

    let intersected_date_filters = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("site:github.com after:1970-01-01 before:1970-01-01".to_string()),
            start_time_ms: Some(3_500),
            end_time_ms: Some(4_500),
            ..HistoryQuery::default()
        },
    )
    .expect("intersected date filters");
    assert_eq!(intersected_date_filters.total, 1);
    assert_eq!(intersected_date_filters.items[0].title.as_deref(), Some("Release notes"));

    let ui_time_only_filters = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            start_time_ms: Some(3_500),
            end_time_ms: Some(4_500),
            ..HistoryQuery::default()
        },
    )
    .expect("ui time-only filters");
    assert_eq!(ui_time_only_filters.total, 1);
    assert_eq!(ui_time_only_filters.items[0].title.as_deref(), Some("Release notes"));

    let title_operator = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("intitle:manual".to_string()), ..HistoryQuery::default() },
    )
    .expect("title operator query");
    assert_eq!(title_operator.total, 1);
    assert_eq!(title_operator.items[0].title.as_deref(), Some("GitHub Actions manual"));

    let url_operator = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("inurl:pull-request".to_string()), ..HistoryQuery::default() },
    )
    .expect("url operator query");
    assert_eq!(url_operator.total, 1);
    assert_eq!(url_operator.items[0].title.as_deref(), Some("Pull Request review checklist"));

    let filetype_operator = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("site:github.com filetype:pdf".to_string()),
            ..HistoryQuery::default()
        },
    )
    .expect("filetype operator query");
    assert_eq!(filetype_operator.total, 1);
    assert_eq!(filetype_operator.items[0].title.as_deref(), Some("PathKeep PDF spec"));

    let any_of_these_words = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("manual OR youtube".to_string()), ..HistoryQuery::default() },
    )
    .expect("or query");
    assert_eq!(any_of_these_words.total, 2);
    assert!(
        any_of_these_words
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("GitHub Actions manual"))
    );
    assert!(
        any_of_these_words
            .items
            .iter()
            .any(|entry| entry.title.as_deref() == Some("YouTube watch queue"))
    );
}

#[test]
fn history_keyword_query_supports_tag_and_note_operators_against_annotations() {
    // feedback-2026-05-25 §3.3 A — annotations existed but the search
    // surface only saw title / URL. This pins the new `tag:` /
    // `note:` operators end-to-end: parser → SQL → archive results.
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    seed_lexical_archive(&paths, &config);

    // Two extra URLs that only differ by their annotations — neither
    // mentions "rust" or "design" in title or URL, so any match must
    // come from the new tag/note filters.
    insert_lexical_history_row(
        &paths,
        &config,
        20,
        "https://example.test/page-with-rust-tag",
        "Generic page A",
        20_000,
        "2026-05-01T00:00:20+00:00",
    );
    insert_lexical_history_row(
        &paths,
        &config,
        21,
        "https://example.test/page-with-design-note",
        "Generic page B",
        21_000,
        "2026-05-01T00:00:21+00:00",
    );
    insert_lexical_history_row(
        &paths,
        &config,
        22,
        "https://example.test/page-with-no-annotations",
        "Generic page C",
        22_000,
        "2026-05-01T00:00:22+00:00",
    );

    // Seed annotations via the public surface so we exercise the same
    // schema the detail panel writes to.
    // Independent annotation facts exercise the unported query parser, not the Turso writer.
    let db = open_archive_connection(&paths, &config, None).unwrap();
    db.execute("INSERT INTO url_tags(url,tag,created_at) VALUES('https://example.test/page-with-rust-tag','Rust','2026-05-01')",[]).unwrap();
    db.execute("INSERT INTO url_annotations(url,notes,created_at,updated_at) VALUES('https://example.test/page-with-design-note','Initial design doc for the cache layer','2026-05-01','2026-05-01')",[]).unwrap();
    rebuild_search_projection(&paths, &config, None).unwrap();

    // `tag:rust` — only the page tagged Rust matches (and the case is
    // folded so the user can type either `tag:Rust` or `tag:rust`).
    let tag_hit = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("tag:rust".to_string()), ..HistoryQuery::default() },
    )
    .expect("tag query");
    assert_eq!(tag_hit.total, 1);
    assert_eq!(tag_hit.items[0].url, "https://example.test/page-with-rust-tag",);

    // `note:design` — substring match on `url_annotations.notes`,
    // case-insensitive.
    let note_hit = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("note:\"design doc\"".to_string()), ..HistoryQuery::default() },
    )
    .expect("note query");
    assert_eq!(note_hit.total, 1);
    assert_eq!(note_hit.items[0].url, "https://example.test/page-with-design-note",);

    // `-tag:rust` — exclude any URL tagged Rust. Both Generic page B
    // and Generic page C survive (along with every other seed row).
    let tag_negated = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("-tag:rust".to_string()), ..HistoryQuery::default() },
    )
    .expect("negated tag query");
    assert!(
        !tag_negated
            .items
            .iter()
            .any(|entry| entry.url == "https://example.test/page-with-rust-tag"),
        "expected -tag:rust to exclude the Rust-tagged row",
    );
    // And the un-tagged Generic page C is still in the result set,
    // pinning that the exclusion is a soft filter (not "only show
    // rows that have any tag").
    assert!(
        tag_negated
            .items
            .iter()
            .any(|entry| { entry.url == "https://example.test/page-with-no-annotations" }),
        "expected -tag:rust to keep un-tagged rows",
    );
}

fn seed_firefox_fixture(root: &Path) -> PathBuf {
    let firefox_root = root.join("firefox");
    let profiles_dir = firefox_root.join("Profiles");
    let profile_dir = profiles_dir.join("abcd.default-release");
    fs::create_dir_all(&profile_dir).expect("create firefox profile dir");
    fs::write(
        firefox_root.join("profiles.ini"),
        "[Profile0]\nName=Work Firefox\nPath=abcd.default-release\nIsRelative=1\n",
    )
    .expect("write firefox profiles.ini");

    let history = Connection::open(profile_dir.join("places.sqlite")).expect("open firefox db");
    history
        .execute_batch(
            "CREATE TABLE moz_places (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER,
               hidden INTEGER,
               last_visit_date INTEGER
             );
             CREATE TABLE moz_historyvisits (
               id INTEGER PRIMARY KEY,
               place_id INTEGER NOT NULL,
               visit_date INTEGER NOT NULL,
               from_visit INTEGER,
               visit_type INTEGER
             );",
        )
        .expect("create firefox tables");
    history
        .execute(
            "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
             VALUES (1, 'https://example.com/firefox', 'Firefox docs', 1, 0, 1744146000000000)",
            [],
        )
        .expect("insert firefox place");
    history
        .execute(
            "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
             VALUES (1, 1, 1744146000000000, NULL, 1)",
            [],
        )
        .expect("insert firefox visit");

    profiles_dir
}

fn seed_safari_fixture(root: &Path) -> PathBuf {
    let safari_root = root.join("Safari");
    fs::create_dir_all(&safari_root).expect("create safari root");
    let history = Connection::open(safari_root.join("History.db")).expect("open safari db");
    history
        .execute_batch(
            "CREATE TABLE history_items (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL
             );
             CREATE TABLE history_visits (
               id INTEGER PRIMARY KEY,
               history_item INTEGER NOT NULL,
               title TEXT,
               visit_time REAL NOT NULL
             );",
        )
        .expect("create safari tables");
    history
        .execute("INSERT INTO history_items (id, url) VALUES (1, 'https://example.com/safari')", [])
        .expect("insert safari item");
    history
        .execute(
            "INSERT INTO history_visits (id, history_item, title, visit_time)
             VALUES (1, 1, 'Safari docs', 765838800.0)",
            [],
        )
        .expect("insert safari visit");
    safari_root
}

fn seed_takeout_fixture(root: &Path) -> PathBuf {
    let source_dir = root.join("takeout-source");
    fs::create_dir_all(&source_dir).expect("create takeout dir");
    fs::write(
        source_dir.join("entries.jsonl"),
        r#"{"url":"https://example.com/import","title":"Imported","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
    )
    .expect("write takeout fixture");
    source_dir
}

#[test]
fn canonical_backup_pipeline_writes_runs_manifests_snapshots_and_queries() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    let mut progress_events = Vec::new();
    let report = run_backup_with_progress(&vault.with_config(config.clone()), false, |event| {
        progress_events.push(event);
    })
    .expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
    assert_eq!(report.run.as_ref().expect("run").new_downloads, 1);
    assert!(report.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));
    assert!(report.profiles[0].checkpoint_created);
    assert!(progress_events.iter().any(|event| event.phase == "prepare"));
    assert!(progress_events.iter().any(|event| event.phase == "stage-profile"));
    assert!(progress_events.iter().any(|event| event.phase == "ingest-profile"));
    assert!(progress_events.iter().any(|event| {
        event.phase == "ingest-profile"
            && event.processed_records == Some(2)
            && event.imported_records == Some(2)
            && event.progress_percent.is_none()
            && event.log_events[0].processed_records == Some(2)
    }));
    assert!(progress_events.iter().any(|event| event.phase == "finalize"));

    vault.store().read(vault_store::Db::Archive,|db| {
        for (sql,expected) in [
            ("SELECT COUNT(*) FROM visits",2), ("SELECT COUNT(*) FROM urls",1),
            ("SELECT COUNT(*) FROM downloads",1),("SELECT COUNT(*) FROM search_terms",1),
            ("SELECT COUNT(*) FROM favicon_blobs",1),("SELECT COUNT(*) FROM favicons WHERE image_blob_hash IS NOT NULL",2),
            ("SELECT COUNT(*) FROM search_documents WHERE title='Archive docs' AND search_terms='deep recall token'",1),
            ("SELECT visible_visit_count FROM search_document_profiles",2),
            ("SELECT COUNT(*) FROM pages p WHERE NOT EXISTS(SELECT 1 FROM search_outbox o WHERE o.page_id=p.id)",0),
        ] {assert_eq!(db.query_one(sql,(),|r|Ok(r.get::<i64>(0)?))?,expected,"{sql}");}
        assert_eq!(db.query_one("SELECT status FROM runs WHERE run_type='backup'",(),|r|Ok(r.get::<String>(0)?))?,"success");
        Ok(())
    }).unwrap();
    let report_again = run_backup(&vault.with_config(config), false).unwrap();
    assert_eq!(report_again.run.unwrap().new_visits, 0);
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[path = "tests/legacy_queries.rs"]
pub(super) mod legacy_queries;

#[test]
fn dashboard_read_models_cover_uninitialized_storage_and_cached_totals_edges() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let uninitialized = AppConfig::default();

    let recent_runs =
        load_recent_runs(&paths, &uninitialized, None).expect("uninitialized recent runs");
    assert!(recent_runs.is_empty());

    let snapshot =
        load_dashboard_snapshot(&paths, &uninitialized, None).expect("uninitialized dashboard");
    assert!(snapshot.next_action.as_deref().is_some_and(|copy| copy.contains("Initialize")));
    assert_eq!(snapshot.storage.archive_database_bytes, 0);
    assert_eq!(directory_size(&dir.path().join("missing-dir")), 0);

    let file_instead_of_directory = dir.path().join("not-a-directory");
    fs::write(&file_instead_of_directory, "plain file").expect("write file");
    assert_eq!(directory_size(&file_instead_of_directory), 0);

    let nested = dir.path().join("nested");
    fs::create_dir_all(nested.join("child")).expect("create nested");
    fs::write(nested.join("root.bin"), "1234").expect("write root");
    fs::write(nested.join("child").join("leaf.bin"), "123456").expect("write child");
    assert_eq!(directory_size(&nested), 10);

    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    connection.execute("DELETE FROM runs", []).expect("clear bootstrap run rows");
    drop(connection);
    let initialized_empty =
        load_dashboard_snapshot(&paths, &config, None).expect("initialized dashboard");
    assert!(
        initialized_empty.next_action.as_deref().is_some_and(|copy| copy.contains("manual backup"))
    );

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    for (id, stats_json) in [
        (100_i64, "{malformed"),
        (99_i64, r#"{"totalProfiles":1}"#),
        (98_i64, r#"{"totalProfiles":2,"totalUrls":3,"totalVisits":5,"totalDownloads":7}"#),
    ] {
        connection
            .execute(
                "INSERT INTO runs
                 (id, run_type, trigger, started_at, timezone, status, profile_scope_json,
                  warnings_json, stats_json, due_only)
                 VALUES (?1, 'backup', 'manual', ?2, 'UTC', 'success', '[]', '[]', ?3, 0)",
                params![id, now_rfc3339(), stats_json],
            )
            .expect("insert cached stats run");
    }

    let totals = read_models::load_cached_archive_totals(&connection)
        .expect("cached totals")
        .expect("valid cached totals");
    assert_eq!(totals.total_profiles, 2);
    assert_eq!(totals.total_urls, 3);
    assert_eq!(totals.total_visits, 5);
    assert_eq!(totals.total_downloads, 7);
}

#[test]
fn dashboard_snapshot_reports_archive_coverage_bounds_for_imported_visits() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    // Empty archive → both bounds are None and the FE renders an em-dash.
    let empty = load_dashboard_snapshot(&paths, &config, None).expect("empty snapshot");
    assert_eq!(empty.earliest_visit_at, None);
    assert_eq!(empty.latest_visit_at, None);

    // Seed two real-ish visits a year apart; mark one as reverted to prove
    // we use the same visible filter the totals query uses (rolled-back
    // visits must not widen the span).
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    connection
        .execute_batch(
            r#"
            INSERT INTO runs (id, run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
              VALUES (1, 'backup', 'manual', '2026-04-14T00:00:00Z', '2026-04-14T00:00:01Z', 'UTC', 'success', '[]', '[]', '{}', 0);
            INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
              VALUES (1, 'chrome', 'Default', '/tmp', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z');
            INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, recorded_at)
              VALUES (1, 'https://example.com', 'Example', 1, 0, 0, '2025-04-22T00:00:00Z', 0, '2025-04-22T00:00:00Z', 1, 1, '2026-04-14T00:00:00Z');
            INSERT INTO visits (id, url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id)
              VALUES (1, 1, 1745280000000, '2025-04-22T00:00:00Z', 1, 1);
            INSERT INTO visits (id, url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id)
              VALUES (2, 1, 1776816000000, '2026-04-22T00:00:00Z', 1, 1);
            INSERT INTO visits (id, url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id, reverted_at)
              VALUES (3, 1, 2208988800000, '2040-01-01T00:00:00Z', 1, 1, '2026-04-23T00:00:00Z');
            "#,
        )
        .expect("seed archive");
    drop(connection);

    let populated = load_dashboard_snapshot(&paths, &config, None).expect("populated snapshot");
    assert_eq!(populated.earliest_visit_at.as_deref(), Some("2025-04-22T00:00:00Z"));
    assert_eq!(populated.latest_visit_at.as_deref(), Some("2026-04-22T00:00:00Z"));
}

#[test]
fn load_recent_runs_excludes_compat_seed_baseline_row() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    // The 002 migration seeds id=0/run_type='system'/trigger='compat'. It
    // must exist on disk (legacy foreign keys depend on it) but never appear
    // in the dashboard or audit ledger.
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let seed_exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE id = 0 AND run_type = 'system' AND trigger = 'compat'",
            [],
            |row| row.get(0),
        )
        .expect("count seed");
    assert_eq!(seed_exists, 1, "002 migration should seed the compat baseline");
    drop(connection);

    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
    assert!(
        recent_runs.iter().all(|run| run.id != 0),
        "compat seed must be filtered from the recent-runs read model"
    );

    // A real backup row (even id=1) must still appear in the ledger so the
    // user can see their actual runs.
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    connection
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'manual', '2026-04-24T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
            [],
        )
        .expect("insert real run");
    drop(connection);

    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs after backup");
    assert!(recent_runs.iter().any(|run| run.id == 1));
    assert!(recent_runs.iter().all(|run| run.id != 0));
}

#[test]
fn backup_guards_initialization_selection_and_due_skip_before_profile_work() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    let uninitialized = run_backup(&vault.with_config(config.clone()), false)
        .expect_err("uninitialized archive should fail");
    assert!(uninitialized.to_string().contains("archive has not been initialized"));

    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let no_selection = run_backup(&vault.with_config(config.clone()), false)
        .expect_err("empty selected profiles should fail");
    assert!(
        format!("{no_selection:#}").contains("select at least one readable browser profile"),
        "{no_selection:#}"
    );

    vault.store().write(vault_store::Db::Archive,vault_store::Priority::Interactive,|s| {
        s.execute("INSERT INTO runs(run_type,trigger,started_at,finished_at,status) VALUES('backup','manual',?1,?1,'success')",[chrono::Utc::now().to_rfc3339()])?;Ok(())
    }).unwrap();
    let skipped =
        run_backup(&vault.with_config(config.clone()), true).expect("due-only backup skip");
    assert!(skipped.due_skipped);
    assert_eq!(skipped.reason_code.as_deref(), Some("not-due"));
    assert!(skipped.reason.as_deref().is_some_and(|reason| reason.contains("minutes old")));
    let skipped_run = skipped.run.expect("due skip must have unified runs evidence");
    assert_eq!(skipped_run.status, "skipped");
    assert_eq!(skipped_run.trigger, "schedule");
    let connection = crate::test_support::store::Probe::new(&vault);
    let persisted: (String, String) = connection
        .query_row(
            "SELECT status, json_extract(stats_json, '$.reasonCode') FROM runs WHERE id = ?1",
            [skipped_run.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("persisted skip row");
    assert_eq!(persisted, ("skipped".to_string(), "not-due".to_string()));
}

#[test]
fn retention_helpers_fall_back_to_filesystem_counts_when_archive_is_unreadable() {
    let root = tempdir().expect("tempdir");
    let paths = sample_paths(root.path());
    let nested = paths.raw_snapshots_dir.join("nested");
    fs::create_dir_all(&nested).expect("snapshot nested dir");
    fs::write(nested.join("snapshot.sqlite"), b"snapshot").expect("snapshot file");
    fs::write(paths.raw_snapshots_dir.join("root.sqlite"), b"snapshot").expect("root snapshot");

    let missing = root.path().join("missing-retention-root");
    assert_eq!(count_path_entries(&missing), 0);
    assert_eq!(remove_directory_contents(&missing).expect("missing directory"), (0, 0));
    assert_eq!(remove_path(&missing).expect("missing path"), (0, 0));

    let file_instead_of_dir = root.path().join("not-a-directory");
    fs::write(&file_instead_of_dir, b"file").expect("file path");
    assert_eq!(count_path_entries(&file_instead_of_dir), 0);

    let mut unreadable_paths = paths.clone();
    unreadable_paths.archive_database_path = root.path().join("archive-directory");
    fs::create_dir_all(&unreadable_paths.archive_database_path).expect("archive dir");
    let bucket = retention_snapshot_bucket(
        &unreadable_paths,
        &AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        },
        None,
    )
    .expect("retention bucket");

    assert_eq!(bucket.id, "snapshots");
    assert_eq!(bucket.item_count, 3);

    let uninitialized_bucket = retention_snapshot_bucket(
        &paths,
        &AppConfig {
            initialized: false,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        },
        None,
    )
    .expect("uninitialized retention bucket");
    assert_eq!(uninitialized_bucket.item_count, 3);
}

#[test]
fn remove_path_except_skips_a_nonexistent_path() {
    // The `!path.exists()` defensive arm of the D3 last-good-keep recursion: a path that does not
    // exist removes nothing and counts nothing.
    assert_eq!(
        remove_path_except(Path::new("/nonexistent/pathkeep-x"), None).expect("missing path"),
        (0, 0),
    );
}

#[test]
fn remove_directory_contents_except_skips_a_missing_root() {
    // The `!root.exists()` guard of the D3 last-good-keep helper: a missing root removes nothing.
    assert_eq!(
        remove_directory_contents_except(Path::new("/nonexistent/pathkeep-root"), None)
            .expect("missing root"),
        (0, 0),
    );
}

#[test]
fn stats_with_archive_totals_replaces_non_object_inputs_with_totals() {
    let connection = Connection::open_in_memory().expect("sqlite");
    create_schema(&connection).expect("schema");

    let stats =
        stats_with_archive_totals(&connection, serde_json::json!("not-an-object")).expect("stats");

    assert_eq!(stats["totalProfiles"], 0);
    assert_eq!(stats["totalUrls"], 0);
    assert_eq!(stats["totalVisits"], 0);
    assert_eq!(stats["totalDownloads"], 0);
}

#[test]
fn backup_rejects_selected_profiles_that_are_not_readable() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = dir.path().join("empty-chrome-root");
    fs::create_dir_all(&chrome_root).expect("empty chrome root");
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Missing".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let error = run_backup(&vault.with_config(config.clone()), false)
        .expect_err("unreadable selected profile should fail");

    assert!(error.to_string().contains("selected profiles are not readable"));

    // A backup tool must never fail silently: the failure is RECORDED as a `failed` run AND carries
    // its reason, so it shows in the run history (it used to bail before any run row was written).
    vault.store().read(vault_store::Db::Archive,|s| {
        let (kind,status,reason)=s.query_one("SELECT run_type,status,error_message FROM runs WHERE id>0 ORDER BY id DESC LIMIT 1",(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?)))?;
        assert_eq!((kind.as_str(),status.as_str()),("backup","failed"));
        assert!(reason.contains("not readable"));Ok(())
    }).unwrap();

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn classify_browser_access_error_tags_permission_denial_as_full_disk_access() {
    use std::io::{Error, ErrorKind};

    // A macOS TCC denial (io PermissionDenied) is rewritten into actionable Full Disk Access guidance.
    let denied =
        anyhow::Error::new(Error::new(ErrorKind::PermissionDenied, "Operation not permitted"))
            .context("reading /Users/me/Library/Application Support/Google/Chrome");
    let classified = super::backup::classify_browser_access_error(denied);
    assert!(
        format!("{classified:#}").contains("Full Disk Access"),
        "permission-denied must be tagged with Full Disk Access guidance, got: {classified:#}"
    );

    // A stringified permission error (no downcastable io::Error) is caught by the message fallback.
    let stringified = anyhow::anyhow!("reading the profile: Operation not permitted (os error 1)");
    assert!(
        format!("{:#}", super::backup::classify_browser_access_error(stringified))
            .contains("Full Disk Access")
    );

    // An unrelated failure is left exactly as-is (no spurious Full Disk Access advice).
    let unrelated = super::backup::classify_browser_access_error(anyhow::anyhow!("disk full"));
    assert_eq!(format!("{unrelated:#}"), "disk full");
}

#[test]
fn backup_progress_and_warning_helpers_preserve_failure_contracts() {
    let profile = BrowserProfile {
        profile_id: "chrome:Default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Google Chrome".to_string(),
        user_name: None,
        profile_path: "/tmp/chrome/Default".to_string(),
        history_path: Some("/tmp/chrome/Default/History".to_string()),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: None,
        history_file_name: "History".to_string(),
        history_bytes: 0,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: Default::default(),
    };
    let mut last_processed_records = 0;
    let mut events = Vec::new();
    super::backup::emit_backup_ingest_progress_if_changed(
        &mut |event| events.push(event),
        &mut last_processed_records,
        0,
        1,
        &profile,
        super::ingest::ArchiveIngestProgress {
            processed_records: 0,
            imported_records: 0,
            duplicate_records: 0,
            skipped_records: 0,
        },
    );
    assert!(events.is_empty());

    super::backup::emit_backup_ingest_progress_if_changed(
        &mut |event| events.push(event),
        &mut last_processed_records,
        0,
        1,
        &profile,
        super::ingest::ArchiveIngestProgress {
            processed_records: 2,
            imported_records: 1,
            duplicate_records: 1,
            skipped_records: 0,
        },
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].processed_records, Some(2));
    assert_eq!(events[0].source_label.as_deref(), Some("Google Chrome / Default"));
    assert_eq!(events[0].log_events[0].code, "backup.ingest-profile.records");
    assert_eq!(events[0].log_events[0].imported_records, Some(1));
}

#[test]
fn backup_surfaces_a_degraded_staging_fallback_as_a_run_warning() {
    let vault = crate::Vault::open_temp().unwrap();
    // A History captured mid-write (a hot rollback journal) cannot take the online
    // snapshot, so staging falls back to the raw-copy + recover path. The recovered
    // copy still parses (the uncommitted write rolls back), so the backup COMPLETES
    // — but the degraded path must surface as a visible run warning, not be
    // swallowed (the backup-level degraded-staging emit).
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let history = chrome_root.join("Default").join("History");

    // Freeze a hot rollback journal next to the fixture History (mirrors the
    // chrome staging test's `capture_hot_journal_pair`): flush an uncommitted
    // header write into a journal on a scratch copy, then snapshot the (db,
    // journal) pair over the fixture.
    {
        let scratch = dir.path().join("History.hot");
        fs::copy(&history, &scratch).expect("copy clean history");
        let writer = Connection::open(&scratch).expect("reopen history");
        writer
            .execute_batch("BEGIN IMMEDIATE;\nUPDATE urls SET title = 'in-flight' WHERE id = 1;")
            .expect("start uncommitted write");
        writer.cache_flush().expect("flush dirty pages into the rollback journal");
        let scratch_journal = PathBuf::from(format!("{}-journal", scratch.display()));
        assert!(scratch_journal.exists(), "an uncommitted flush must leave a rollback journal");
        fs::copy(&scratch, &history).expect("freeze hot db over fixture");
        fs::copy(&scratch_journal, PathBuf::from(format!("{}-journal", history.display())))
            .expect("freeze hot journal over fixture");
        writer.execute_batch("ROLLBACK").ok();
    }

    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let report = run_backup(&vault.with_config(config.clone()), false);
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
    let report = report.expect("backup completes after recovering the hot-journal history");

    assert!(
        report.warnings.iter().any(|warning| warning.contains("recovered file copy")),
        "a recovered hot-journal staging must be reported as a run warning: {:?}",
        report.warnings
    );
}

#[test]
fn backup_marks_run_failed_when_readable_profile_cannot_be_staged() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = dir.path().join("broken-chrome-root");
    let profile_dir = chrome_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
    fs::write(
        chrome_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default"}}}}"#,
    )
    .expect("write local state");
    fs::create_dir(profile_dir.join("History")).expect("create bad history directory");
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let error =
        run_backup(&vault.with_config(config.clone()), false).expect_err("staging should fail");
    let status = crate::test_support::store::Probe::new(&vault)
        .query_row("SELECT status FROM runs ORDER BY id DESC LIMIT 1", (), |row| {
            Ok(row.get::<String>(0)?)
        })
        .expect("run status");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());

    assert!(format!("{error:#}").contains("staging profile chrome:Default"));
    assert_eq!(status, "failed");
}

#[test]
fn backup_skips_unreadable_selected_profile_when_another_profile_is_readable() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let chrome_root = seed_missing_chrome_history_fixture(dir.path());
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec![
            "firefox:abcd.default-release".to_string(),
            "chrome:Default".to_string(),
        ],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&vault.with_config(config.clone()), false)
        .expect("run backup with skipped profile");

    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());

    assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "firefox:abcd.default-release");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("chrome:Default") && warning.contains("unreadable"))
    );
}

#[test]
fn multi_browser_backup_ingests_firefox_and_safari_history() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let empty_chrome_root = dir.path().join("empty-chrome");
    fs::create_dir_all(&empty_chrome_root).expect("empty chrome root");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &empty_chrome_root);
        std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec![
            "firefox:abcd.default-release".to_string(),
            "safari:default".to_string(),
        ],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report =
        run_backup(&vault.with_config(config.clone()), false).expect("run multi-browser backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 2);
    assert_eq!(report.profiles.len(), 2);
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("firefox:")));
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("safari:")));
    assert!(report.warnings.iter().any(|warning| warning.contains("Firefox baseline ingest")));
    assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().any(|entry| entry.starts_with("firefox:")));
    assert!(history.iter().any(|entry| entry.starts_with("safari:")));

    let rerun =
        run_backup(&vault.with_config(config.clone()), false).expect("rerun multi-browser backup");
    assert_eq!(rerun.run.as_ref().expect("rerun").new_visits, 0);
    assert_eq!(rerun.run.as_ref().expect("rerun").new_urls, 0);

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn safari_backup_baseline_ingests_history_without_firefox_or_chrome_dependency() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let safari_root = seed_safari_fixture(dir.path());
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", dir.path().join("missing-chrome"));
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["safari:default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&vault.with_config(config.clone()), false).expect("run safari backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "safari:default");
    assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], "safari:default");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn backup_keeps_chrome_successful_when_selected_safari_is_unreadable() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let safari_root = dir.path().join("Safari");
    fs::create_dir_all(&safari_root).expect("create safari root");
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string(), "safari:default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|entry| entry.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[cfg(unix)]
#[test]
fn backup_keeps_readable_profiles_when_safari_staging_loses_access() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let safari_root = dir.path().join("Safari");
    fs::create_dir_all(&safari_root).expect("create safari root");
    let safari_history = safari_root.join("History.db");
    fs::create_dir(&safari_history).expect("create unreadable staging source");
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string(), "safari:default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|entry| entry.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn doctor_detects_missing_snapshot_artifacts() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_paths(&paths).expect("ensure paths");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
             VALUES (0, ?1, 0, 'missing', 'test', ?2)",
            params![dir.path().join("missing").display().to_string(), now_rfc3339()],
        )
        .expect("insert missing snapshot");

    let report = doctor(&paths, &config, None).expect("doctor");
    assert!(report.checks.iter().any(|check| {
        check.code == "snapshot-artifacts" && check.name == "Snapshot artifacts" && !check.ok
    }));
}

#[test]
fn doctor_labels_every_check_with_a_stable_kebab_code() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let report = doctor(&paths, &config, None).expect("doctor");
    assert!(!report.checks.is_empty());
    for check in &report.checks {
        assert!(!check.code.is_empty(), "check {} is missing a code", check.name);
        assert!(
            check.code.chars().all(|character| character.is_ascii_lowercase() || character == '-'),
            "check code {} is not kebab-case",
            check.code
        );
    }
    let codes = report.checks.iter().map(|check| check.code.as_str()).collect::<Vec<_>>();
    for expected in [
        "config",
        "browser-sources",
        "archive-db",
        "archive-unlock",
        "schema-version",
        "manifest-chain",
        "snapshot-artifacts",
        "import-audit-artifacts",
        "broken-visibility-references",
        "derived-state-freshness",
    ] {
        assert!(codes.contains(&expected), "doctor report is missing check code {expected}");
    }
}

#[test]
fn doctor_detects_manifest_parent_and_hash_damage() {
    let parent_dir = tempdir().expect("tempdir");
    let parent_paths = sample_paths(parent_dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&parent_paths, &config, None).expect("init parent archive");
    let parent_connection =
        Connection::open(&parent_paths.archive_database_path).expect("open parent archive");
    create_schema(&parent_connection).expect("parent schema");
    parent_connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert parent run");
    let run_id = parent_connection.last_insert_rowid();
    parent_connection
        .execute(
            "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
             VALUES (?1, NULL, 'first-hash', '{}', ?2, NULL)",
            params![run_id, now_rfc3339()],
        )
        .expect("insert first manifest");
    parent_connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys for damaged fixture");
    parent_connection
        .execute(
            "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
             VALUES (?1, 9999, 'second-hash', '{}', ?2, NULL)",
            params![run_id, now_rfc3339()],
        )
        .expect("insert broken parent manifest");

    let parent_report = doctor(&parent_paths, &config, None).expect("doctor parent");
    assert!(parent_report.checks.iter().any(|check| {
        check.code == "manifest-chain"
            && check.name == "Manifest chain"
            && !check.ok
            && check.detail.contains("does not point to the previous manifest")
    }));

    let hash_dir = tempdir().expect("tempdir");
    let hash_paths = sample_paths(hash_dir.path());
    ensure_archive_initialized(&hash_paths, &config, None).expect("init hash archive");
    let hash_connection =
        Connection::open(&hash_paths.archive_database_path).expect("open hash archive");
    create_schema(&hash_connection).expect("hash schema");
    hash_connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert hash run");
    let run_id = hash_connection.last_insert_rowid();
    let manifest_path = hash_dir.path().join("manifest.json");
    fs::write(&manifest_path, r#"{"ok":true}"#).expect("write manifest artifact");
    hash_connection
        .execute(
            "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
             VALUES (?1, NULL, 'not-the-real-hash', '{}', ?2, ?3)",
            params![run_id, now_rfc3339(), manifest_path.display().to_string()],
        )
        .expect("insert hash mismatch manifest");

    let hash_report = doctor(&hash_paths, &config, None).expect("doctor hash");
    assert!(hash_report.checks.iter().any(|check| {
        check.code == "manifest-chain"
            && check.name == "Manifest chain"
            && !check.ok
            && check.detail.contains("manifest hash mismatch")
    }));
}

#[test]
fn doctor_detects_import_batches_without_audit_artifacts() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, imported_at, status, summary_json, audit_path)
             VALUES ('takeout', '/tmp/takeout', 'takeout::browser-history', ?1, ?1, 'imported', '{}', NULL)",
            [now_rfc3339()],
        )
        .expect("insert import batch without audit path");

    let report = doctor(&paths, &config, None).expect("doctor");
    assert!(report.checks.iter().any(|check| {
        check.code == "import-audit-artifacts"
            && check.name == "Import audit artifacts"
            && !check.ok
            && check.detail.contains("does not have an audit artifact")
    }));
}

#[test]
fn doctor_repair_noops_on_healthy_archive() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let repair = repair_health_issues(&vault, &paths, &config, None).expect("repair health");

    assert!(repair.run_id.is_none());
    assert_eq!(repair.repaired_import_audits, 0);
    assert_eq!(repair.repaired_visibility_rows, 0);
    assert_eq!(repair.cleared_derived_rows, 0);
    assert!(repair.notes.iter().any(|note| note.contains("found no actionable damage")));
}

#[test]
fn doctor_repair_restores_missing_import_artifacts_visibility_and_derived_state() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    let original_chrome = std::env::var_os(TEST_CHROME_USER_DATA_OVERRIDE_ENV);
    let chrome_root = seed_chrome_fixture(dir.path());
    unsafe {
        std::env::set_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
    }
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &vault,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    seed_legacy_import_fixture(&paths, &config, batch.audit_path.as_deref());
    let audit_path = batch.audit_path.expect("audit path");
    fs::remove_file(&audit_path).expect("remove import audit artifact");

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "UPDATE visits SET reverted_at = ?1, reverted_by_run_id = NULL WHERE import_batch_id = ?2",
            params![now_rfc3339(), batch.id],
        )
        .expect("break visibility");
    let intelligence =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    intelligence
        .execute(
            "INSERT INTO ai_embeddings
             (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at)
             VALUES (999, 'takeout::browser-history', 'https://example.com/import', 'Imported', 'example.com', ?1, 'hash', 8, 'provider', 'model', ?1)",
            rusqlite::params![now_rfc3339()],
        )
        .expect("insert stale ai embedding");
    intelligence
        .execute(
            "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
             VALUES ('trail-1', 'takeout::browser-history', 999, 0, 'result')",
            [],
        )
        .expect("insert stale trail member");
    intelligence
        .execute(
            "INSERT INTO visit_derived_facts
             (visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url, domain_category, page_category, search_engine, search_query, is_new_domain, is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version, computed_at)
             VALUES (999, 'takeout::browser-history', 'session-1', 'trail-1', 'example.com', 'https://example.com/import', 'reference', 'article', NULL, NULL, 0, 0, 'tier-c', 'builtin', 'core-intelligence', 'test', ?1)",
            [now_rfc3339()],
        )
        .expect("insert stale visit-derived facts");

    let report = doctor(&paths, &config, None).expect("doctor before repair");
    assert!(report.checks.iter().any(|check| {
        check.code == "import-audit-artifacts"
            && check.name == "Import audit artifacts"
            && !check.ok
    }));
    assert!(report.checks.iter().any(|check| {
        check.code == "broken-visibility-references"
            && check.name == "Broken visibility references"
            && !check.ok
    }));
    assert!(report.checks.iter().any(|check| {
        check.code == "derived-state-freshness"
            && check.name == "Derived state freshness"
            && !check.ok
    }));

    let repair = repair_health_issues(&vault, &paths, &config, None).expect("repair health");
    assert!(repair.run_id.is_some());
    assert_eq!(repair.repaired_import_audits, 1);
    assert_eq!(repair.repaired_visibility_rows, 1);
    assert!(repair.cleared_derived_rows >= 2);

    let repaired_report = doctor(&paths, &config, None).expect("doctor after repair");
    assert!(repaired_report.checks.iter().all(|check| check.ok));
    restore_test_env_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, original_chrome.as_deref());
}

#[test]
fn doctor_repair_tolerates_missing_optional_intelligence_tables() {
    let vault = crate::Vault::open_temp().unwrap();
    for dropped_tables in [
        vec!["ai_embeddings", "search_trail_members"],
        vec!["visit_derived_facts"],
        vec!["search_trail_members", "visit_derived_facts"],
    ] {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("open intelligence");
        for table in dropped_tables {
            intelligence
                .execute(&format!("DROP TABLE IF EXISTS {table}"), [])
                .expect("drop optional intelligence table");
        }
        drop(intelligence);

        let repair = repair_health_issues(&vault, &paths, &config, None).expect("repair health");

        assert!(repair.run_id.is_none());
        assert_eq!(repair.cleared_derived_rows, 0);
    }
}

#[test]
fn doctor_repair_restores_visibility_when_import_audits_are_intact() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &vault,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    seed_legacy_import_fixture(&paths, &config, batch.audit_path.as_deref());
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "UPDATE visits SET reverted_at = ?1, reverted_by_run_id = NULL WHERE import_batch_id = ?2",
            params![now_rfc3339(), batch.id],
        )
        .expect("break visibility");

    let repair = repair_health_issues(&vault, &paths, &config, None).expect("repair health");

    assert_eq!(repair.repaired_import_audits, 0);
    assert_eq!(repair.repaired_visibility_rows, 1);
    assert!(repair.notes.iter().any(|note| note.contains("Re-linked 1 reverted visit rows")));
}

#[test]
fn doctor_repair_records_rebuilt_import_artifacts_when_git_is_enabled() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let import_config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &import_config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &vault,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    seed_legacy_import_fixture(&paths, &import_config, batch.audit_path.as_deref());
    fs::remove_file(batch.audit_path.expect("audit path")).expect("remove import audit artifact");
    let repair_config = AppConfig { git_enabled: true, ..import_config };

    let repair = repair_health_issues(&vault, &paths, &repair_config, None).expect("repair health");

    assert_eq!(repair.repaired_import_audits, 1);
    assert!(
        repair
            .notes
            .iter()
            .any(|note| note.contains("Recorded repaired import artifacts in audit commit")
                || note.contains("optional Git history step was skipped"))
    );
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let git_commit: Option<String> = connection
        .query_row("SELECT git_commit FROM import_batches WHERE id = ?1", [batch.id], |row| {
            row.get(0)
        })
        .expect("git commit");
    if let Some(git_commit) = git_commit {
        assert!(!git_commit.is_empty());
    }
}

#[test]
fn doctor_repair_records_failed_run_when_audit_artifact_rewrite_fails() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, imported_at, status, summary_json, audit_path)
             VALUES ('takeout', '/tmp/takeout', 'takeout::browser-history', ?1, ?1, 'imported', '{}', NULL)",
            [now_rfc3339()],
        )
        .expect("insert import batch without audit path");
    fs::create_dir_all(&paths.audit_repo_path).expect("audit repo dir");
    fs::write(paths.audit_repo_path.join("imports"), "not a directory")
        .expect("block audit imports path");

    let error = repair_health_issues(&vault, &paths, &config, None)
        .expect_err("blocked audit repo should fail repair");

    assert!(!error.to_string().is_empty());
    let failed_runs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE run_type = 'doctor' AND status = 'failed'",
            [],
            |row| row.get(0),
        )
        .expect("failed doctor run count");
    assert_eq!(failed_runs, 1);
}

#[test]
fn dashboard_snapshot_tracks_cached_totals_across_import_visibility_changes() {
    let vault = crate::Vault::open_temp().unwrap();
    let source = seed_takeout_fixture(&vault.paths().app_root);
    let imported = crate::import_takeout(
        &vault,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .unwrap();
    let batch = imported.import_batch.unwrap().id;
    for (step, expected) in [("import", 1), ("rollback", 0), ("restore", 1)] {
        if step == "rollback" {
            crate::revert_import_batch(&vault, batch).unwrap();
        }
        if step == "restore" {
            crate::restore_import_batch(&vault, batch).unwrap();
        }
        vault.store().read(vault_store::Db::Archive,move |s| {
            let stats=s.query_one("SELECT stats_json FROM runs WHERE run_type=?1 ORDER BY id DESC LIMIT 1",[step],|r|Ok(r.get::<String>(0)?))?;
            let stats:Value=serde_json::from_str(&stats).unwrap();
            assert_eq!(stats["totalVisits"],expected);assert_eq!(stats["totalUrls"],1);
            assert_eq!(s.query_one("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",(),|r|Ok(r.get::<i64>(0)?))?,expected);
            assert_eq!(s.query_one("SELECT COALESCE(SUM(visible_visit_count),0) FROM search_document_profiles",(),|r|Ok(r.get::<i64>(0)?))?,expected);
            assert_eq!(s.query_one("SELECT COUNT(*) FROM search_documents WHERE title LIKE '%Imported%' AND newest_visit_ms IS NOT NULL",(),|r|Ok(r.get::<i64>(0)?))?,expected);
            assert!(s.query_one("SELECT COUNT(*) FROM search_outbox",(),|r|Ok(r.get::<i64>(0)?))?>0);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn snapshot_restore_preview_and_run_record_the_saved_checkpoint() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let backup = run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    let snapshot_path: String = crate::test_support::store::Probe::new(&vault)
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("latest snapshot path");

    let recorded = vault
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query_one("SELECT created_at FROM snapshots ORDER BY id LIMIT 1", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    let unrecorded = dir.path().join("unrecorded");
    fs::create_dir_all(&unrecorded).unwrap();
    assert!(
        run_snapshot_restore(
            &vault,
            &SnapshotRestoreRequest { snapshot_path: unrecorded.display().to_string() }
        )
        .unwrap_err()
        .to_string()
        .contains("recorded ledger")
    );
    let restored = run_snapshot_restore(
        &vault,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .expect("run snapshot restore");
    let restore_run = restored.run.expect("restore run");
    assert_eq!(restore_run.run_type, "snapshot_restore");
    assert_eq!(restore_run.status, "success");
    assert!(backup.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));

    let row = vault
        .store()
        .read(vault_store::Db::Archive, move |s| {
            s.query_one(
                "SELECT reason,created_at FROM snapshots WHERE run_id=?1",
                [restore_run.id],
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
            )
        })
        .unwrap();
    assert_eq!(row, ("restored-source-checkpoint".into(), recorded));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(restored.manifest_path.unwrap()).unwrap())
            .unwrap();
    assert!(!manifest["sourceHashes"]["chrome:Default"].as_object().unwrap().is_empty());

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn snapshot_restore_records_failed_run_when_replay_cannot_persist() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    let connection = crate::test_support::store::Probe::new(&vault);
    let snapshot_path: String = connection
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("latest snapshot path");
    let manifests = &vault.paths().manifests_dir;
    let saved_manifests = manifests.with_file_name("saved-manifests");
    fs::rename(manifests, &saved_manifests).unwrap();
    fs::write(manifests, b"prevent manifest persistence").unwrap();
    let error = run_snapshot_restore(
        &vault,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .unwrap_err();
    assert!(!error.to_string().is_empty());
    let failed_stats: String = connection.query_row("SELECT stats_json FROM runs WHERE run_type='snapshot_restore' ORDER BY id DESC LIMIT 1", (), |r| Ok(r.get(0)?)).unwrap();
    let failed_stats: Value = serde_json::from_str(&failed_stats).unwrap();
    assert_eq!(failed_stats["profilesProcessed"], 1);
    assert_eq!(failed_stats["profiles"].as_array().unwrap().len(), 1);
    let visible: i64 = connection
        .query_row("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |r| Ok(r.get(0)?))
        .unwrap();
    assert_eq!(
        failed_stats["processedRecords"], visible,
        "checkpoint replay retains its processed counts when the manifest fails"
    );
    assert_eq!(failed_stats["newVisits"], 0);
    fs::remove_file(manifests).unwrap();
    fs::rename(saved_manifests, manifests).unwrap();
    // One visit batch commits before a corrupt later source row aborts replay.
    let source = Connection::open(Path::new(&snapshot_path).join("History")).unwrap();
    source.execute_batch("WITH RECURSIVE seq(id) AS (SELECT 100 UNION ALL SELECT id+1 FROM seq WHERE id<10100)
        INSERT INTO visits(id,url,visit_time,transition) SELECT id,1,13390000000000000+id,0 FROM seq;
        UPDATE visits SET visit_time='bad timestamp' WHERE id=10100;").unwrap();
    drop(source);
    assert!(
        run_snapshot_restore(
            &vault,
            &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() }
        )
        .is_err()
    );
    let stats:String=connection.query_row("SELECT stats_json FROM runs WHERE run_type='snapshot_restore' ORDER BY id DESC LIMIT 1",(),|r|Ok(r.get(0)?)).unwrap();
    let stats: Value = serde_json::from_str(&stats).unwrap();
    assert_eq!(stats["processedRecords"], 10000);
    assert_eq!(stats["profilesProcessed"], 1);
    assert_eq!(stats["profiles"][0]["newVisits"], stats["newVisits"]);
    assert!(stats["newVisits"].as_u64().unwrap() > 0);
    vault
        .store()
        .write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
            s.execute("DROP TABLE visits", ())?;
            Ok(())
        })
        .expect("damage archive schema");
    drop(connection);

    let restore_error = run_snapshot_restore(&vault, &SnapshotRestoreRequest { snapshot_path })
        .expect_err("damaged archive should fail snapshot restore");
    let restore_error_chain = format!("{restore_error:#}");
    assert!(restore_error_chain.contains("visits"), "{restore_error_chain}");

    let failed_status: String = crate::test_support::store::Probe::new(&vault)
        .query_row(
            "SELECT status
             FROM runs
             WHERE run_type = 'snapshot_restore'
             ORDER BY id DESC
             LIMIT 1",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("failed restore run");
    assert_eq!(failed_status, "failed");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn source_evidence_keeps_large_payloads_without_plaintext_spool_files() {
    let vault = crate::Vault::open_temp_with_key(Some([81; 32])).unwrap();
    let large = "x".repeat(300_000);
    let payload = SourceEvidencePayload {
        typed_evidence: TypedEvidenceBatch {
            context: vec![ContextEvidence {
                source_visit_id: Some(1),
                source_url_id: Some(1),
                context_key: "large".into(),
                value_json: large.clone(),
                source_field: "payload".into(),
            }],
            ..Default::default()
        },
        native_entities: vec![NativeEntity {
            entity_kind: "large".into(),
            native_primary_key: "1".into(),
            parent_native_primary_key: None,
            payload_json: large.clone(),
            metadata: BTreeMap::new(),
        }],
    };
    let mut writer = super::source_evidence::EvidenceWriter::new(&vault, 1, 1);
    writer.push(payload).unwrap();
    let restored: String =
        crate::test_support::store::Probe::for_db(&vault, vault_store::Db::SourceEvidence)
            .query_row("SELECT payload_json FROM native_entities", (), |r| Ok(r.get(0)?))
            .unwrap();
    assert_eq!(restored, large);
    assert!(!vault.paths().staging_dir.join("source-evidence-spool").exists());
}

#[test]
fn snapshot_restore_preview_sizes_firefox_and_safari_checkpoints() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let empty_chrome_root = dir.path().join("empty-chrome");
    fs::create_dir_all(&empty_chrome_root).expect("empty chrome root");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &empty_chrome_root);
        std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec![
            "firefox:abcd.default-release".to_string(),
            "safari:default".to_string(),
        ],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    run_backup(&vault.with_config(config.clone()), false).expect("run multi-browser backup");

    let snapshot_paths = vault
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query("SELECT file_path FROM snapshots ORDER BY id", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    let previews = snapshot_paths
        .iter()
        .map(|snapshot_path| {
            preview_snapshot_restore(
                &vault,
                &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
            )
        })
        .collect::<Result<Vec<_>>>()
        .expect("preview snapshots");

    assert!(previews.iter().any(|preview| {
        preview
            .source_browser_name
            .as_deref()
            .is_some_and(|browser_name| browser_name.eq_ignore_ascii_case("firefox"))
            && preview.estimated_visits == 1
            && preview.estimated_urls == 1
    }));
    assert!(previews.iter().any(|preview| {
        preview
            .source_browser_name
            .as_deref()
            .is_some_and(|browser_name| browser_name.eq_ignore_ascii_case("safari"))
            && preview.estimated_visits == 1
            && preview.estimated_urls == 1
    }));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn retention_preview_and_prune_clear_local_artifacts_and_record_a_run() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let checkpoint = paths.raw_snapshots_dir.join("retention-fixture");
    fs::create_dir_all(&checkpoint).unwrap();
    fs::write(checkpoint.join("History"), b"fixture").unwrap();
    let db = open_archive_connection(&paths, &config, None).unwrap();
    db.execute("INSERT INTO snapshots(run_id,file_path,file_size,created_at,reason) VALUES(0,?1,7,'2026-04-05','fixture')",[checkpoint.display().to_string()]).unwrap();
    fs::create_dir_all(&paths.exports_dir).expect("create exports dir");
    fs::write(paths.exports_dir.join("export.jsonl"), "[]").expect("write export fixture");

    let preview = preview_retention(&paths, &config, None).expect("preview retention");
    assert!(preview.buckets.iter().any(|bucket| bucket.id == "snapshots" && bucket.bytes > 0));
    assert!(preview.buckets.iter().any(|bucket| bucket.id == "exports" && bucket.bytes > 0));
    // The code channel is what Settings localizes against, so it must stay
    // index-aligned with the diagnostic prose.
    assert_eq!(preview.warnings.len(), preview.warning_codes.len());
    assert_eq!(
        preview.warning_codes,
        vec![
            crate::models::RETENTION_WARNING_SNAPSHOT_PRUNE_REMOVES_CHECKPOINTS.to_string(),
            crate::models::RETENTION_WARNING_EXPORT_PRUNE_LOCAL_ONLY.to_string(),
        ]
    );

    let result = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest { bucket_ids: vec!["snapshots".to_string(), "exports".to_string()] },
    )
    .expect("run retention prune");
    assert!(result.run_id.is_some());
    assert!(result.deleted_bytes > 0);
    assert_eq!(directory_size(&paths.raw_snapshots_dir), 0);
    assert_eq!(directory_size(&paths.exports_dir), 0);

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let snapshot_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
        .expect("snapshot count");
    assert_eq!(snapshot_count, 0);
    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
    assert!(recent_runs.iter().any(|run| run.run_type == "retention_prune"));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn maintenance_guards_manual_snapshots_and_retention_edge_cases() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let safety_snapshot = paths.raw_snapshots_dir.join("manual-safety.sqlite");
    fs::create_dir_all(&paths.raw_snapshots_dir).expect("create snapshot dir");
    fs::write(&safety_snapshot, "manual safety copy").expect("write safety snapshot");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    connection
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (90, 'backup', 'manual', ?1, 'UTC', 'success', '[\"profile-a\"]', '[]', '{}', 0)",
            params![now_rfc3339()],
        )
        .expect("insert run");
    connection
        .execute(
            "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
             VALUES (90, ?1, 18, 'manual', 'safety-copy', ?2)",
            params![safety_snapshot.display().to_string(), now_rfc3339()],
        )
        .expect("insert safety snapshot");

    let stored_path = safety_snapshot.display().to_string();
    vault.store().write(vault_store::Db::Archive,vault_store::Priority::Interactive,move |s| {
        s.execute("INSERT INTO runs(id,run_type,trigger,started_at,status,profile_scope_json) VALUES(90,'backup','manual','2026-04-05','success','[\"profile-a\"]')",())?;
        s.execute("INSERT INTO snapshots(run_id,file_path,file_size,checksum,reason,created_at) VALUES(90,?1,18,'manual','safety-copy','2026-04-05')",[stored_path])?;Ok(())
    }).unwrap();
    let preview = preview_snapshot_restore(
        &vault,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect("manual safety snapshot preview");
    assert_eq!(preview.snapshot_kind, "archive-safety-snapshot");
    // D1: the full-archive safety snapshot is now one-click restorable (quarantine + install +
    // reconcile), so the preview advertises execute support and describes the restore.
    assert!(preview.execute_supported);
    assert_eq!(preview.source_profile_id.as_deref(), Some("profile-a"));
    assert!(preview.warnings[0].contains("One-click restore"));
    let run_error = run_snapshot_restore(
        &vault,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect_err("manual safety copies are not automatically restored");
    assert!(run_error.to_string().contains("automatic restore"));

    let uninitialized = AppConfig { initialized: false, ..AppConfig::default() };
    let restore_uninitialized = run_snapshot_restore(
        &vault.with_config(uninitialized.clone()),
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect_err("uninitialized archive cannot restore snapshots");
    assert!(restore_uninitialized.to_string().contains("archive has not been initialized"));

    let prune_error = run_retention_prune(
        &paths,
        &uninitialized,
        None,
        &RetentionPruneRequest { bucket_ids: vec!["exports".to_string()] },
    )
    .expect_err("uninitialized archive cannot prune");
    assert!(prune_error.to_string().contains("initialize the archive"));

    let empty = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest { bucket_ids: Vec::new() },
    )
    .expect("empty prune request");
    assert!(empty.run_id.is_none());
    assert!(empty.warnings[0].contains("Choose at least one"));

    let unknown = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest { bucket_ids: vec!["not-a-bucket".to_string()] },
    )
    .expect("unknown bucket prune request");
    assert!(unknown.run_id.is_none());
    assert!(unknown.warnings[0].contains("No matching"));

    fs::create_dir_all(&paths.exports_dir).expect("exports dir");
    fs::create_dir_all(&paths.staging_dir).expect("staging dir");
    fs::create_dir_all(&paths.quarantine_dir).expect("quarantine dir");
    fs::write(paths.exports_dir.join("export.json"), "{}").expect("export file");
    fs::write(paths.staging_dir.join("stage.tmp"), "stage").expect("staging file");
    fs::write(paths.quarantine_dir.join("bad.txt"), "bad").expect("quarantine file");
    let all = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest {
            bucket_ids: vec![
                "snapshots".to_string(),
                "exports".to_string(),
                "staging".to_string(),
                "quarantine".to_string(),
            ],
        },
    )
    .expect("all retention buckets");
    assert!(all.run_id.is_some());
    assert!(all.deleted_files >= 4);
    assert_eq!(directory_size(&paths.exports_dir), 0);
    assert_eq!(directory_size(&paths.staging_dir), 0);
    assert_eq!(directory_size(&paths.quarantine_dir), 0);
}

#[test]
fn rekey_archive_keeps_a_safety_snapshot() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    fs::write(paths.archive_database_path.with_extension("rekey.sqlite"), "stale rekey temp")
        .expect("write stale rekey temp");
    fs::write(paths.archive_database_path.with_extension("backup.sqlite"), "stale rekey backup")
        .expect("write stale rekey backup");

    let status =
        rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("vault-passphrase"))
            .expect("rekey archive");

    let rekey_dir = paths.raw_snapshots_dir.join("rekey");
    let snapshots = fs::read_dir(&rekey_dir)
        .expect("read rekey snapshot dir")
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();

    assert!(status.encrypted);
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].path().is_file());
    assert!(!paths.archive_database_path.with_extension("rekey.sqlite").exists());
    assert!(!paths.archive_database_path.with_extension("backup.sqlite").exists());

    let encrypted_config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..config.clone() };
    let recent_runs = load_recent_runs(&paths, &encrypted_config, Some("vault-passphrase"))
        .expect("recent runs after rekey");
    let rekey_run =
        recent_runs.iter().find(|run| run.run_type == "rekey").expect("rekey run in ledger");
    let detail =
        load_audit_run_detail(&paths, &encrypted_config, Some("vault-passphrase"), rekey_run.id)
            .expect("rekey audit detail");
    assert!(detail.manifest_path.is_some());
    assert!(
        detail.artifacts.iter().any(|artifact| artifact.reason.as_deref() == Some("before-rekey"))
    );

    let plaintext_status = rekey_archive(
        &paths,
        &encrypted_config,
        Some("vault-passphrase"),
        ArchiveMode::Plaintext,
        None,
    )
    .expect("rekey back to plaintext");
    assert!(!plaintext_status.encrypted);
}

#[test]
fn rekey_archive_reports_missing_database_and_missing_new_key() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };

    let missing_database = rekey_archive(&paths, &config, None, ArchiveMode::Plaintext, None)
        .expect_err("missing archive database");
    assert!(missing_database.to_string().contains("archive database does not exist"));

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let missing_key = rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, None)
        .expect_err("missing new encryption key");
    assert!(missing_key.to_string().contains("new encryption key is required"));
}

#[test]
fn rekey_archive_records_failed_run_when_config_save_fails_after_swap() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    fs::remove_file(&paths.config_path).expect("remove config file");
    fs::create_dir(&paths.config_path).expect("replace config path with directory");

    let error = rekey_archive(&paths, &config, None, ArchiveMode::Plaintext, None)
        .expect_err("config save failure should abort rekey closeout");
    assert!(error.to_string().contains("writing"));

    let connection =
        Connection::open(&paths.archive_database_path).expect("open archive after swap");
    let (status, error_message): (String, Option<String>) = connection
        .query_row(
            "SELECT status, error_message
             FROM runs
             WHERE run_type = 'rekey'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("failed rekey run");
    assert_eq!(status, "failed");
    assert!(error_message.as_deref().is_some_and(|message| message.contains("writing")));
}

// --- Crash-window regression suite (Phase B/E) ---------------------------------------------------
//
// The named, legible home for the archive-mutation crash-window tests: each destructive op (rekey,
// backup; import lives in `migration::fault_tests`, restore + launch recovery in `maintenance`) has a
// kill-at-EXACT-checkpoint regression that (a) FAILS on the un-hardened code (no crash seam, config
// written before the swap) and (b) asserts the INJECTED fault propagated via `format!("{err:#}")`,
// never a bare `is_err()`. Phase E adds the config<->disk invariant post-condition (see
// `config_disk_invariant_holds_after_a_successful_backup` here, plus the enumerated torture + the
// `check_config_disk_consistency` checker in `archive::at_rest`) so a crash can never ship a config
// that lags the installed files.

#[test]
fn rekey_crash_after_export_before_swap_leaves_the_original_archive_recoverable() {
    // Window (4): a crash AFTER the new-keyed export but BEFORE the swap must be a
    // full no-op — the ORIGINAL canonical archive + config are untouched and the
    // verified backstop already exists. This would FAIL on the old flow, which had
    // no crash seam at all and swapped before writing config.
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    seed_rekey_marker(&paths, &config, None);

    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("rekey.after_export_before_swap");
    let error = rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("new-pass"))
        .expect_err("a crash before the swap must abort the rekey");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("rekey.after_export_before_swap"),
        "the INJECTED fault must propagate, got: {rendered}"
    );

    // Config never changed and the ORIGINAL archive still opens with the ORIGINAL key
    // and holds the seeded rows.
    assert!(matches!(
        crate::config::load_config(&paths).expect("load config").archive_mode,
        ArchiveMode::Plaintext
    ));
    assert_eq!(rekey_marker_count(&paths, &config, None), 1, "the original rows must survive");
    assert_eq!(rekey_snapshot_count(&paths), 1, "the verified backstop must exist");
}

#[test]
fn rekey_crash_after_swap_before_config_keeps_backstop_and_new_file_openable() {
    // Window (6) — THE incident window: a crash AFTER the durable swap but BEFORE
    // config is written. Config still reads the OLD mode (it is written LAST, so this
    // is the self-healable lag, not a brick), the on-disk file is the converted one
    // and opens with the NEW key, and the backstop snapshot is still present. The old
    // flow deleted its rollback copy before config and had no durability barrier here.
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    seed_rekey_marker(&paths, &config, None);

    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("rekey.after_swap_before_config");
    let error = rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("new-pass"))
        .expect_err("a crash in the incident window must abort the rekey");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("rekey.after_swap_before_config"),
        "the INJECTED fault must propagate, got: {rendered}"
    );

    // Config still reflects the OLD mode (config is written LAST).
    assert!(matches!(
        crate::config::load_config(&paths).expect("load config").archive_mode,
        ArchiveMode::Plaintext
    ));

    // The on-disk archive was durably converted and opens with the NEW key, rows
    // intact (forward-recoverable once config heals), and the backstop remains.
    let encrypted = AppConfig { archive_mode: ArchiveMode::Encrypted, ..config.clone() };
    assert_eq!(
        rekey_marker_count(&paths, &encrypted, Some("new-pass")),
        1,
        "the rekeyed file must open with the new key and preserve rows"
    );
    assert_eq!(rekey_snapshot_count(&paths), 1, "the backstop snapshot must remain after the swap");
}

#[test]
fn rekey_crash_after_config_has_consistent_new_mode_and_openable_archive() {
    // Window (9): a crash AFTER config is durably written (before the closeout)
    // leaves config AND the on-disk file both at the NEW mode — a consistent,
    // openable state.
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    seed_rekey_marker(&paths, &config, None);

    let _guard = crate::fault_inject::FaultGuard::error_at_must_fire("rekey.after_config");
    let error = rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("new-pass"))
        .expect_err("a crash after the config write must abort closeout");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("rekey.after_config"),
        "the INJECTED fault must propagate, got: {rendered}"
    );

    // Config matches the NEW mode and the archive opens with the new key.
    assert!(matches!(
        crate::config::load_config(&paths).expect("load config").archive_mode,
        ArchiveMode::Encrypted
    ));
    let encrypted = AppConfig { archive_mode: ArchiveMode::Encrypted, ..config.clone() };
    assert_eq!(rekey_marker_count(&paths, &encrypted, Some("new-pass")), 1);
    assert_eq!(rekey_snapshot_count(&paths), 1);
}

#[cfg(unix)]
#[test]
fn run_retention_prune_blocks_on_a_foreign_write_lock() {
    // Lock-completion block: retention-prune is a destructive op that MUST hold the
    // cross-process write lock for its duration. While another OS process holds it, the
    // prune cannot proceed. We simulate the foreign holder with a raw second `flock`, run
    // the prune on a worker, prove it is BLOCKED (it cannot finish), then release the lock
    // and prove it completes. FAILS on pre-change code, which took no lock here and so ran
    // to completion despite the foreign holder.
    use std::sync::mpsc;
    use std::time::Duration;

    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let foreign = super::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

    let worker_paths = paths.clone();
    let worker_config = config.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = run_retention_prune(
            &worker_paths,
            &worker_config,
            None,
            &RetentionPruneRequest { bucket_ids: vec!["exports".to_string()] },
        );
        done_tx.send(result.is_ok()).ok();
    });

    assert!(
        done_rx.recv_timeout(Duration::from_millis(500)).is_err(),
        "retention prune ran without holding the cross-process write lock",
    );

    drop(foreign);
    assert!(
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("retention prune must finish once the foreign lock is released"),
        "retention prune must succeed after the lock frees",
    );
    worker.join().expect("worker thread");
}

#[cfg(unix)]
#[test]
fn reconcile_archive_encryption_blocks_on_a_foreign_write_lock() {
    // Lock-completion block: the unlock-path at-rest reconcile is a destructive rewrite
    // that MUST hold the cross-process write lock so a scheduled backup can never race it
    // on source-evidence. We prove it is BLOCKED behind a foreign holder, then completes
    // once the lock frees. FAILS on pre-change code, which opened + reconciled without the
    // lock and so finished despite the foreign holder (with no interrupted-import marker
    // present, pre-change recovery was a lock-free no-op too).
    use std::sync::mpsc;
    use std::time::Duration;

    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let foreign = super::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

    let worker_paths = paths.clone();
    let worker_config = config.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = reconcile_archive_encryption(&worker_paths, &worker_config, None);
        done_tx.send(result.is_ok()).ok();
    });

    assert!(
        done_rx.recv_timeout(Duration::from_millis(500)).is_err(),
        "at-rest reconcile ran without holding the cross-process write lock",
    );

    drop(foreign);
    assert!(
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("at-rest reconcile must finish once the foreign lock is released"),
        "at-rest reconcile must succeed after the lock frees",
    );
    worker.join().expect("worker thread");
}

#[test]
fn run_support_failed_runs_and_due_windows_stay_truthful() {
    let vault = crate::Vault::open_temp().unwrap();
    let config = AppConfig { initialized: true, due_after_hours: 72.0, ..Default::default() };
    let run_id =
        super::write_runs::start(&vault, "backup", "manual", &["profile-a".into()]).unwrap();
    vault
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, move |s| {
            super::write_runs::record_progress(
                s,
                run_id,
                &[("profilesProcessed", 2), ("newVisits", 5), ("newUrls", 3), ("newDownloads", 1)],
            )?;
            s.execute(
                "UPDATE runs SET warnings_json=?2 WHERE id=?1",
                vault_store::params![
                    run_id,
                    serde_json::to_string(&[crate::models::BackupWarning::opaque("warning")])?
                ],
            )?;
            Ok(())
        })
        .unwrap();
    super::write_runs::stopped(
        &vault,
        run_id,
        &anyhow::anyhow!("fixture failure"),
        json!({}),
        None,
    )
    .unwrap();
    let connection = crate::test_support::store::Probe::new(&vault);
    let (status, stats_json, warnings_json, error_message): (String, String, String, String) =
        connection
            .query_row(
                "SELECT status, stats_json, warnings_json, error_message FROM runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("failed run row");
    let stats: serde_json::Value = serde_json::from_str(&stats_json).expect("stats json");
    // Backup runs now persist coded warnings, so the ledger stores objects and
    // the read path is what flattens them back to the message channel.
    let warnings = super::read_models::decode_run_warnings(Some(&warnings_json));
    assert_eq!(status, "failed");
    assert_eq!(stats["profilesProcessed"], 2);
    assert_eq!(stats["newVisits"], 5);
    assert_eq!(stats["newUrls"], 3);
    assert_eq!(stats["newDownloads"], 1);
    assert_eq!(crate::models::backup_warning_messages(&warnings), vec!["warning"]);
    assert_eq!(warnings[0].code, "");
    assert!(error_message.contains("fixture failure"));

    // Ledger rows written before warning code-ification stored bare strings, and
    // non-backup run types still do. Both shapes must stay readable or Audit
    // would silently drop warnings that are already on disk.
    assert!(super::read_models::decode_run_warnings(None).is_empty());
    assert!(super::read_models::decode_run_warnings(Some("not json")).is_empty());
    assert!(super::read_models::decode_run_warnings(Some("{}")).is_empty());
    let mixed = super::read_models::decode_run_warnings(Some(
        r#"["legacy string", {"code":"og-cleanup-failed","message":"cleanup failed"}, 7]"#,
    ));
    assert_eq!(mixed.len(), 2);
    assert_eq!(mixed[0].code, "");
    assert_eq!(mixed[0].message, "legacy string");
    assert_eq!(mixed[1].code, "og-cleanup-failed");
    assert_eq!(mixed[1].message, "cleanup failed");

    let now = chrono::Utc::now();
    let recent = now - chrono::Duration::hours(2);
    let old = now - chrono::Duration::hours(96);
    let recent_reason = super::run_support::backup_due_skip_reason_at(recent, &config, now)
        .expect("recent backup should skip");
    assert!(recent_reason.contains("120 minutes old"));
    assert!(super::run_support::backup_due_skip_reason_at(old, &config, now).is_none());

    let minute_config =
        AppConfig { initialized: true, due_after_hours: 1.5, ..AppConfig::default() };
    let recent_minute_backup = now - chrono::Duration::minutes(89);
    let minute_reason =
        super::run_support::backup_due_skip_reason_at(recent_minute_backup, &minute_config, now)
            .expect("backup younger than 90 minutes should skip");
    assert!(minute_reason.contains("89 minutes old"));

    let invalid_config =
        AppConfig { initialized: true, due_after_hours: 0.0, ..AppConfig::default() };
    let sub_minute_backup = now - chrono::Duration::seconds(30);
    assert!(
        super::run_support::backup_due_skip_reason_at(sub_minute_backup, &invalid_config, now)
            .is_some()
    );

    connection
        .execute(
            "UPDATE runs SET status='success',finished_at=?2,error_message=NULL WHERE id=?1",
            vault_store::params![run_id, recent.to_rfc3339()],
        )
        .unwrap();
    assert!(run_backup(&vault.with_config(config), true).unwrap().due_skipped);
}

#[test]
fn visit_event_fingerprint_is_stable() {
    let fingerprint = visit_event_fingerprint(
        "chromium-history",
        "https://example.com",
        1,
        Some("Title"),
        Some(805306368),
        None,
    );
    assert_eq!(fingerprint, "da53df0772e36b09afd187a0454da559fe451c828a40353f4e5c7514d17ecc59");
}

/// Independent SQLite inputs for the unported doctor's repair contracts.
/// These rows never come from a Turso query or validate a Turso writer.
fn seed_legacy_import_fixture(paths: &ProjectPaths, config: &AppConfig, audit_path: Option<&str>) {
    let db = open_archive_connection(paths, config, None).unwrap();
    db.execute_batch("INSERT INTO runs(id,run_type,trigger,started_at,status) VALUES(1,'import','manual','2026-04-01','success');
      INSERT INTO source_profiles(id,browser_kind,browser_family,browser_product,profile_name,profile_path,discovered_at,enabled,profile_key,updated_at)
      VALUES(1,'takeout','takeout','Takeout','Takeout','/fixture','2026-04-01',1,'takeout::browser-history','2026-04-01');
      INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json) VALUES(1,'takeout','/fixture','takeout::browser-history','2026-04-01','imported','{}');
      INSERT INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id)
      VALUES(1,'https://example.com/import','Imported',1,0,1,'2026-04-01T10:00:00+00:00',1,'2026-04-01T10:00:00+00:00',1,1);
      INSERT INTO visits(id,source_profile_id,url_id,visit_time_ms,visit_time_iso,created_by_run_id,import_batch_id)
      VALUES(1,1,1,1,'2026-04-01T10:00:00+00:00',1,1);").unwrap();
    db.execute("UPDATE import_batches SET audit_path=?1 WHERE id=1", [audit_path]).unwrap();
    rebuild_search_projection(paths, config, None).unwrap();
}
