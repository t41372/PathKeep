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
use std::collections::BTreeMap;
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

/// Counts canonical visit rows on an open connection. Reopening the archive from disk
/// and calling this is how the crash-window tests prove a backup left either the
/// pre-backup state or the fully-applied state — never a torn half-write.
fn canonical_visit_count(connection: &Connection) -> i64 {
    connection.query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0)).expect("count visits")
}

/// Asserts the archive passes `PRAGMA integrity_check` (structurally consistent on disk).
fn assert_archive_integrity_ok(connection: &Connection) {
    let status: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity_check");
    assert_eq!(status, "ok", "the archive must be structurally consistent after a crash");
}

fn seed_chrome_fixture(root: &Path) -> PathBuf {
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
fn regex_scan_cap_truncates_window_yet_matches_full_scan_when_cap_exceeds_rows() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    // seed_lexical_archive creates source_profile id 1 plus baseline rows; the
    // FK on visits requires that profile before insert_lexical_history_row works.
    seed_lexical_archive(&paths, &config);

    // Five contiguous, newest visits carrying a token only these rows match, so
    // the newest-sorted scan window maps one-to-one onto matches: row count in
    // the window == match count, with no interleaved non-matches to muddy the
    // boundary assertion.
    let marker_visit_times = [900_001, 900_002, 900_003, 900_004, 900_005];
    for (offset, visit_time) in marker_visit_times.iter().enumerate() {
        let id = 100 + offset as i64;
        insert_lexical_history_row(
            &paths,
            &config,
            id,
            &format!("https://capmarker.test/row-{id}"),
            "Cap marker row",
            *visit_time,
            &format!("2026-05-02T00:00:0{offset}+00:00"),
        );
    }

    let connection = open_archive_connection(&paths, &config, None).expect("open archive");

    // A scan_cap smaller than the match count must stop inside the window: only
    // the two newest marker rows are scanned, so the three older matches at
    // 900_001..=900_003 are never examined. This is the whole reason the capped
    // inner fn exists, and it fails (total == 5) if the cap is removed.
    let truncated = super::history::list_history_with_regex_capped_for_test(
        &connection,
        10,
        "newest",
        r"capmarker\.test",
        2,
    )
    .expect("capped regex scan");
    assert_eq!(
        truncated.total, 2,
        "scan_cap=2 must bound the window to the two newest matches, not all five"
    );
    assert_eq!(truncated.items.len(), 2, "only the scanned window is returned");
    let returned_urls: Vec<&str> = truncated.items.iter().map(|entry| entry.url.as_str()).collect();
    assert_eq!(
        returned_urls,
        vec!["https://capmarker.test/row-104", "https://capmarker.test/row-103"],
        "the bounded scan must surface the two newest rows in newest order, nothing deeper"
    );

    // A scan_cap larger than the total visible rows recovers the old full-scan
    // semantics: every marker row is examined and the reported total is exact.
    let full = super::history::list_history_with_regex_capped_for_test(
        &connection,
        10,
        "newest",
        r"capmarker\.test",
        50_000,
    )
    .expect("uncapped regex scan");
    assert_eq!(
        full.total,
        marker_visit_times.len(),
        "a cap above the row count must scan every match (full-scan parity)"
    );
    assert_eq!(full.items.len(), marker_visit_times.len());
    assert!(
        full.items.iter().all(|entry| entry.url.starts_with("https://capmarker.test/row-")),
        "the full scan must return exactly the seeded marker rows"
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
    crate::annotations::replace_tags(
        &paths,
        &config,
        None,
        crate::ReplaceTagsRequest {
            url: "https://example.test/page-with-rust-tag".to_string(),
            tags: vec!["Rust".to_string()],
            source_profile: None,
        },
    )
    .expect("replace tags");
    crate::annotations::set_notes(
        &paths,
        &config,
        None,
        crate::SetNotesRequest {
            url: "https://example.test/page-with-design-note".to_string(),
            notes: "Initial design doc for the cache layer".to_string(),
            source_profile: None,
        },
    )
    .expect("set notes");

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
    let mut progress_events = Vec::new();
    let report = run_backup_with_progress(&paths, &config, None, false, |event| {
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

    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
    assert!(!recent_runs.is_empty());
    assert!(recent_runs.iter().any(|run| run.run_type == "backup" && run.status == "success"));

    let history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().all(|entry| entry.favicon.is_none()));

    let loaded_favicons = load_history_favicons(
        &paths,
        &config,
        None,
        history
            .items
            .iter()
            .map(|entry| HistoryFaviconLookupEntry {
                profile_id: entry.profile_id.clone(),
                url: entry.url.clone(),
                visit_time: entry.visit_time,
            })
            .collect(),
    )
    .expect("load history favicons");
    assert_eq!(loaded_favicons.len(), 2);
    let empty_favicons =
        load_history_favicons(&paths, &config, None, Vec::new()).expect("empty favicon lookup");
    assert!(empty_favicons.is_empty());
    let duplicate_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![
            HistoryFaviconLookupEntry {
                profile_id: history.items[0].profile_id.clone(),
                url: history.items[0].url.clone(),
                visit_time: history.items[0].visit_time,
            },
            HistoryFaviconLookupEntry {
                profile_id: history.items[0].profile_id.clone(),
                url: history.items[0].url.clone(),
                visit_time: history.items[0].visit_time,
            },
        ],
    )
    .expect("duplicate favicon lookup");
    assert_eq!(duplicate_favicon_lookup.len(), 1);
    let missing_profile_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Missing".to_string(),
            url: history.items[0].url.clone(),
            visit_time: history.items[0].visit_time,
        }],
    )
    .expect("missing profile favicon lookup");
    assert!(missing_profile_favicon_lookup[0].favicon.is_none());
    let second_visit_favicon = loaded_favicons
        .iter()
        .find(|entry| entry.visit_time == history.items[0].visit_time)
        .expect("second visit favicon result");
    assert!(
        second_visit_favicon
            .favicon
            .as_ref()
            .is_some_and(|favicon| favicon.data_url.starts_with("data:image/")),
        "expected the visit at the icon observation time to load the exact page icon"
    );
    let first_visit_favicon = loaded_favicons
        .iter()
        .find(|entry| entry.visit_time == history.items[1].visit_time)
        .expect("first visit favicon result");
    assert!(
        first_visit_favicon.favicon.is_none(),
        "favicon lookup must not use an exact page icon first observed after the visit"
    );

    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    let favicon_blob_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM favicon_blobs", [], |row| row.get(0))
        .expect("favicon blob count");
    let favicon_reference_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM favicons WHERE image_blob_hash IS NOT NULL", [], |row| {
            row.get(0)
        })
        .expect("favicon reference count");
    assert_eq!(favicon_blob_count, 1);
    assert_eq!(favicon_reference_count, 2);

    let search_term_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("deep recall".to_string()), ..HistoryQuery::default() },
    )
    .expect("list search term history");
    assert_eq!(search_term_history.total, 2);
    let search_term_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("search term first page");
    assert_eq!(search_term_first_page.items.len(), 1);
    assert!(search_term_first_page.next_cursor.is_some());
    let search_term_second_cursor_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            cursor: search_term_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("search term second cursor page");
    assert_eq!(search_term_second_cursor_page.items.len(), 1);
    assert!(search_term_second_cursor_page.has_previous);
    let search_term_explicit_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("search term explicit second page");
    assert_eq!(search_term_explicit_second_page.page, 2);
    assert_eq!(search_term_explicit_second_page.items.len(), 1);

    let url_fragment_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("example.com/archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list url fragment history");
    assert_eq!(url_fragment_history.total, 2);

    let punctuation_only_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("!!!".to_string()), ..HistoryQuery::default() },
    )
    .expect("list punctuation-only history");
    assert_eq!(punctuation_only_history.total, 0);

    let regex_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect("regex history");
    assert_eq!(regex_history.total, 2);
    let regex_oldest_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("regex oldest first page");
    assert!(regex_oldest_first_page.next_cursor.is_some());
    let regex_oldest_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            cursor: regex_oldest_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("regex oldest second page");
    assert_eq!(regex_oldest_second_page.items.len(), 1);
    assert!(regex_oldest_second_page.has_previous);
    let regex_newest_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("regex newest first page");
    let regex_newest_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            cursor: regex_newest_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("regex newest second page");
    assert_eq!(regex_newest_second_page.items.len(), 1);
    let regex_explicit_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("regex explicit second page");
    assert_eq!(regex_explicit_second_page.page, 2);
    assert_eq!(regex_explicit_second_page.items.len(), 1);

    let invalid_regex = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive(".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect_err("invalid regex");
    assert!(
        format!("{invalid_regex:#}").contains("invalid regex pattern"),
        "unexpected error: {invalid_regex:#}"
    );
    let unsupported_regex = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("^((?!pathkeep).)*$".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect_err("unsupported regex");
    assert!(
        format!("{unsupported_regex:#}").contains("invalid regex pattern"),
        "Rust regex should reject look-around patterns before searching"
    );

    let first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("first history page");
    assert_eq!(first_page.total, 2);
    assert_eq!(first_page.items.len(), 1);
    assert!(first_page.next_cursor.is_some());

    let second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            limit: Some(1),
            cursor: first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("second history page");
    assert_eq!(second_page.total, 2);
    assert_eq!(second_page.items.len(), 1);
    assert!(second_page.next_cursor.is_none());

    let explicit_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(1), page: Some(2), ..HistoryQuery::default() },
    )
    .expect("explicit second history page");
    assert_eq!(explicit_second_page.total, 2);
    assert_eq!(explicit_second_page.page, 2);
    assert_eq!(explicit_second_page.page_count, 2);
    assert_eq!(explicit_second_page.items.len(), 1);
    assert!(explicit_second_page.has_previous);
    assert!(!explicit_second_page.has_next);
    assert!(explicit_second_page.next_cursor.is_none());
    let empty_domain_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { domain: Some("missing.invalid".to_string()), ..HistoryQuery::default() },
    )
    .expect("empty domain history");
    assert_eq!(empty_domain_history.total, 0);
    assert_eq!(empty_domain_history.page_count, 1);

    let mut connection = open_archive_connection(&paths, &config, None).expect("open archive");
    let run_id = connection
        .query_row("SELECT id FROM runs ORDER BY id LIMIT 1", [], |row| row.get::<_, i64>(0))
        .expect("load run id");
    connection
        .execute(
            "INSERT INTO favicons (
               page_url,
               icon_url,
               icon_type,
               width,
               height,
               last_updated_ms,
               last_updated_iso,
               image_data,
               source_profile_id,
               created_by_run_id,
               page_host,
               page_registrable_domain
             )
             VALUES (?1, ?2, 1, 16, 16, ?3, ?4, ?5, 1, ?6, ?7, ?8)",
            params![
                "https://docs.example.co.uk/favicon-source",
                "https://docs.example.co.uk/favicon.ico",
                chrono::DateTime::parse_from_rfc3339("2026-04-05T10:30:00+00:00")
                    .expect("registrable same-profile favicon time")
                    .timestamp_millis(),
                "2026-04-05T10:30:00+00:00",
                vec![0x89_u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x02],
                run_id,
                "docs.example.co.uk",
                "example.co.uk",
            ],
        )
        .expect("insert same-profile registrable favicon");
    let same_profile_registrable_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://blog.example.co.uk/article".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T11:00:00+00:00")
                .expect("same-profile registrable visit time")
                .timestamp_millis(),
        }],
    )
    .expect("same-profile registrable favicon lookup");
    // The registrable-domain fallback used to fire here; it was removed
    // because it leaked icons across unrelated sites that share a public
    // suffix (e.g. `*.github.io`). The exact page URL and same FQDN are
    // the only safe sources, so a different host on the same registrable
    // domain must now resolve to `None`.
    assert!(
        same_profile_registrable_lookup[0].favicon.is_none(),
        "registrable-domain fallback is disabled — different hosts on the same registrable domain must not borrow each other's icons"
    );
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
               browser_family,
               browser_product
             )
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, 1, ?6, NULL, ?5, ?7, ?8)",
            params![
                2_i64,
                "takeout",
                "Imported",
                "/tmp/imported-profile",
                now_rfc3339(),
                "takeout::browser-history",
                "chromium",
                "Takeout",
            ],
        )
        .expect("insert imported source profile");
    connection
        .execute(
            "INSERT INTO favicons (
               page_url,
               icon_url,
               icon_type,
               width,
               height,
               last_updated_ms,
               last_updated_iso,
               image_data,
               source_profile_id,
               created_by_run_id,
               page_host,
               page_registrable_domain
             )
             VALUES (?1, ?2, 1, 16, 16, ?3, ?4, ?5, 2, ?6, ?7, ?8)",
            params![
                "https://shared.example.org/favicon-source",
                "https://shared.example.org/favicon.ico",
                chrono::DateTime::parse_from_rfc3339("2026-04-05T10:40:00+00:00")
                    .expect("cross-profile host favicon time")
                    .timestamp_millis(),
                "2026-04-05T10:40:00+00:00",
                vec![0x89_u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x04],
                run_id,
                "shared.example.org",
                "example.org",
            ],
        )
        .expect("insert cross-profile host favicon");
    let cross_profile_host_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://shared.example.org/article".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T11:20:00+00:00")
                .expect("cross-profile host visit time")
                .timestamp_millis(),
        }],
    )
    .expect("cross-profile host favicon lookup");
    assert!(
        cross_profile_host_lookup[0].favicon.is_some(),
        "expected host fallback to cross profiles when the current profile has no matching host icon"
    );
    connection
        .execute(
            "INSERT INTO favicons (
               page_url,
               icon_url,
               icon_type,
               width,
               height,
               last_updated_ms,
               last_updated_iso,
               image_data,
               source_profile_id,
               created_by_run_id,
               page_host,
               page_registrable_domain
             )
             VALUES (?1, ?2, 1, 16, 16, ?3, ?4, ?5, 2, ?6, ?7, ?8)",
            params![
                "https://learn.example.net/favicon-source",
                "https://learn.example.net/favicon.ico",
                chrono::DateTime::parse_from_rfc3339("2026-04-05T10:45:00+00:00")
                    .expect("cross-profile registrable favicon time")
                    .timestamp_millis(),
                "2026-04-05T10:45:00+00:00",
                vec![0x89_u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x03],
                run_id,
                "learn.example.net",
                "example.net",
            ],
        )
        .expect("insert cross-profile registrable favicon");
    let cross_profile_registrable_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://www.example.net/article".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T11:30:00+00:00")
                .expect("cross-profile registrable visit time")
                .timestamp_millis(),
        }],
    )
    .expect("cross-profile registrable favicon lookup");
    // Same rationale as the same-profile case above: cross-host borrowing
    // at the registrable-domain level was removed to stop icon bleed.
    assert!(
        cross_profile_registrable_lookup[0].favicon.is_none(),
        "registrable-domain fallback is disabled across profiles too"
    );
    connection
        .execute("UPDATE visits SET source_profile_id = 2 WHERE id = 2", [])
        .expect("reassign visit profile");

    let cross_profile_favicon = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { page: Some(1), limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("cross-profile favicon history page");
    assert!(cross_profile_favicon.items[0].favicon.is_none());
    let cross_profile_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: cross_profile_favicon.items[0].profile_id.clone(),
            url: cross_profile_favicon.items[0].url.clone(),
            visit_time: cross_profile_favicon.items[0].visit_time,
        }],
    )
    .expect("cross-profile favicon lookup");
    assert!(
        cross_profile_favicon_lookup[0].favicon.is_some(),
        "expected favicon lookup to fall back across source profiles for the same page URL"
    );

    let same_host_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/missing-page".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T12:00:00+00:00")
                .expect("same-host visit time")
                .timestamp_millis(),
        }],
    )
    .expect("same-host favicon lookup");
    assert!(
        same_host_favicon_lookup[0].favicon.is_some(),
        "expected same-host fallback to reuse a historical icon without requiring exact page_url"
    );

    let future_host_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/before-icon".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T09:00:00+00:00")
                .expect("future-host visit time")
                .timestamp_millis(),
        }],
    )
    .expect("future-host favicon lookup");
    assert!(
        future_host_favicon_lookup[0].favicon.is_none(),
        "domain fallback must not use an icon first observed after the visit"
    );

    let paged_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery {
                q: Some("archive".to_string()),
                limit: Some(1),
                page: Some(2),
                ..HistoryQuery::default()
            },
            format: ExportFormat::Jsonl,
        },
    )
    .expect("export all visible history even when current query is paged");
    assert_eq!(paged_export.count, 2);
    let html_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Html,
        },
    )
    .expect("export history html");
    assert_eq!(html_export.count, 2);
    let html_content = fs::read_to_string(&html_export.path).expect("read html export");
    assert!(html_content.contains("<article>"));
    assert!(html_content.contains("Archive docs"));
    let markdown_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Markdown,
        },
    )
    .expect("export history markdown");
    let markdown_content = fs::read_to_string(&markdown_export.path).expect("read markdown export");
    assert!(markdown_content.contains("- [Archive docs](https://example.com/archive)"));
    let text_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Text,
        },
    )
    .expect("export history text");
    let text_content = fs::read_to_string(&text_export.path).expect("read text export");
    assert!(text_content.contains("Archive docs\nhttps://example.com/archive\n"));

    let bulk_url_id = 9_001_i64;
    let bulk_base_ms = chrono::DateTime::parse_from_rfc3339("2026-04-06T00:00:00+00:00")
        .expect("bulk base time")
        .timestamp_millis();
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
               created_by_run_id
             )
             VALUES (?1, ?2, ?3, 1001, 0, ?4, ?5, ?6, ?7, 1, ?8)",
            params![
                bulk_url_id,
                "https://bulk.example/export",
                "Bulk export cursor fixture",
                bulk_base_ms,
                "2026-04-06T00:00:00+00:00",
                bulk_base_ms + 1_000,
                "2026-04-06T00:00:01+00:00",
                run_id,
            ],
        )
        .expect("insert bulk export url");
    let bulk_insert = connection.transaction().expect("bulk transaction");
    for index in 0..=1_000_i64 {
        let visit_time_ms = bulk_base_ms + index;
        let visit_time_iso = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(visit_time_ms)
            .expect("bulk visit time")
            .to_rfc3339();
        bulk_insert
            .execute(
                "INSERT INTO visits (
                   url_id,
                   source_visit_id,
                   visit_time_ms,
                   visit_time_iso,
                   transition_type,
                   visit_duration_ms,
                   source_profile_id,
                   created_by_run_id
                 )
                 VALUES (?1, ?2, ?3, ?4, 805306368, 1000, 1, ?5)",
                params![
                    bulk_url_id,
                    format!("bulk-export-{index}"),
                    visit_time_ms,
                    visit_time_iso,
                    run_id,
                ],
            )
            .expect("insert bulk export visit");
    }
    bulk_insert.commit().expect("commit bulk export rows");
    let bulk_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery {
                domain: Some("bulk.example".to_string()),
                ..HistoryQuery::default()
            },
            format: ExportFormat::Jsonl,
        },
    )
    .expect("export multi-page history");
    assert_eq!(bulk_export.count, 1001);

    let report_again = run_backup(&paths, &config, None, false).expect("rerun backup");
    assert_eq!(report_again.run.as_ref().expect("run").new_visits, 0);

    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    let mut statement = connection
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT visits.id
             FROM visits
             JOIN urls ON urls.id = visits.url_id
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             JOIN search.history_search_terms ON history_search_terms.rowid = urls.id
             WHERE visits.reverted_at IS NULL
               AND history_search_terms MATCH ?1",
        )
        .expect("prepare query plan");
    let plan = statement
        .query_map(["\"deep\"*"], |row| row.get::<_, String>(3))
        .expect("query plan rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect query plan");
    assert!(
        plan.iter().any(|detail| detail.contains("VIRTUAL TABLE INDEX")),
        "unexpected query plan: {plan:?}"
    );
    assert!(
        plan.iter().any(|detail| detail.contains("idx_visits_visible_url_time")),
        "fts history query is not using the visible visit lookup index: {plan:?}"
    );
    assert!(
        !plan.iter().any(|detail| detail == "SCAN visits"),
        "fts history query still scans the whole visits table: {plan:?}"
    );

    let visit_time = chrono::DateTime::parse_from_rfc3339("2026-04-05T12:00:00+00:00")
        .expect("query plan visit time")
        .timestamp_millis();
    fn assert_favicon_plan_uses<P: rusqlite::Params>(
        connection: &Connection,
        sql: &str,
        expected_index: &str,
        params: P,
    ) {
        let mut favicon_statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("prepare favicon query plan");
        let favicon_plan = favicon_statement
            .query_map(params, |row| row.get::<_, String>(3))
            .expect("query favicon plan rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect favicon query plan");
        assert!(
            favicon_plan.iter().any(|detail| detail.contains(expected_index)),
            "favicon query is not using {expected_index}: {favicon_plan:?}"
        );
        assert!(
            !favicon_plan.iter().any(|detail| detail.contains("SCAN favicons")),
            "favicon lookup still scans the whole table: {favicon_plan:?}"
        );
    }
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_SAME_PROFILE_PAGE_SQL,
        "idx_favicons_recall_lookup",
        params![1_i64, "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL,
        "idx_favicons_page_lookup",
        params![1_i64, "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_SAME_PROFILE_HOST_SQL,
        "idx_favicons_host_profile_lookup",
        params![1_i64, "example.com", "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_CROSS_PROFILE_HOST_SQL,
        "idx_favicons_host_lookup",
        params![1_i64, "example.com", "https://example.com/archive", visit_time],
    );
    // Registrable-domain fallback queries were removed; their dormant
    // indexes still exist in the schema but are no longer reached by the
    // lookup pipeline. See favicons.rs for rationale (icon-bleed guard).

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

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
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let uninitialized = run_backup(&paths, &AppConfig::default(), None, false)
        .expect_err("uninitialized archive should fail");
    assert!(uninitialized.to_string().contains("archive has not been initialized"));

    let initialized = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &initialized, None).expect("init archive");
    let no_selection = run_backup(&paths, &initialized, None, false)
        .expect_err("empty selected profiles should fail");
    assert!(no_selection.to_string().contains("select at least one readable browser profile"));

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    let recent = chrono::Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [recent],
        )
        .expect("insert recent successful backup");

    let skipped = run_backup(&paths, &initialized, None, true).expect("due-only backup skip");
    assert!(skipped.due_skipped);
    assert!(skipped.reason.as_deref().is_some_and(|reason| reason.contains("minutes old")));
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

    let error = run_backup(&paths, &config, None, false)
        .expect_err("unreadable selected profile should fail");

    assert!(error.to_string().contains("selected profiles are not readable"));

    // A backup tool must never fail silently: the failure is RECORDED as a `failed` run AND carries
    // its reason, so it shows in the run history (it used to bail before any run row was written).
    let recent = load_recent_runs(&paths, &config, None).expect("recent runs");
    let failed = recent
        .iter()
        .find(|run| run.status == "failed")
        .expect("the failed backup attempt must be recorded as a run");
    assert_eq!(failed.run_type, "backup");
    assert!(
        failed.error_message.as_deref().is_some_and(|reason| reason.contains("not readable")),
        "the failed run must carry its reason, got {:?}",
        failed.error_message
    );

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

    let source_warning =
        super::backup::source_evidence_rebuild_warning(anyhow::anyhow!("source offline"));
    let search_warning =
        super::backup::keyword_recall_rebuild_warning(anyhow::anyhow!("search offline"));
    assert!(source_warning.contains("source-evidence archive"));
    assert!(search_warning.contains("keyword-recall projection"));
}

#[test]
fn backup_surfaces_a_degraded_staging_fallback_as_a_run_warning() {
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

    let report = run_backup(&paths, &config, None, false);
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

    let error = run_backup(&paths, &config, None, false).expect_err("staging should fail");
    let status = Connection::open(&paths.archive_database_path)
        .expect("archive")
        .query_row("SELECT status FROM runs ORDER BY id DESC LIMIT 1", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("run status");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());

    assert!(format!("{error:#}").contains("staging profile chrome:Default"));
    assert_eq!(status, "failed");
}

#[test]
fn backup_skips_unreadable_selected_profile_when_another_profile_is_readable() {
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
    let report = run_backup(&paths, &config, None, false).expect("run backup with skipped profile");

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
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
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
    let report = run_backup(&paths, &config, None, false).expect("run multi-browser backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 2);
    assert_eq!(report.profiles.len(), 2);
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("firefox:")));
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("safari:")));
    assert!(report.warnings.iter().any(|warning| warning.contains("Firefox baseline ingest")));
    assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().any(|entry| entry.profile_id.starts_with("firefox:")));
    assert!(history.items.iter().any(|entry| entry.profile_id.starts_with("safari:")));

    let rerun = run_backup(&paths, &config, None, false).expect("rerun multi-browser backup");
    assert_eq!(rerun.run.as_ref().expect("rerun").new_visits, 0);
    assert_eq!(rerun.run.as_ref().expect("rerun").new_urls, 0);

    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn safari_backup_baseline_ingests_history_without_firefox_or_chrome_dependency() {
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
    let report = run_backup(&paths, &config, None, false).expect("run safari backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "safari:default");
    assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 1);
    assert_eq!(history.items[0].profile_id, "safari:default");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn backup_keeps_chrome_successful_when_selected_safari_is_unreadable() {
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
    let report = run_backup(&paths, &config, None, false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().all(|entry| entry.profile_id.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[cfg(unix)]
#[test]
fn backup_keeps_readable_profiles_when_safari_staging_loses_access() {
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
    let report = run_backup(&paths, &config, None, false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().all(|entry| entry.profile_id.starts_with("chrome:")));

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
    assert!(report.checks.iter().any(|check| check.name == "Snapshot artifacts" && !check.ok));
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
        check.name == "Manifest chain"
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
        check.name == "Manifest chain"
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
        check.name == "Import audit artifacts"
            && !check.ok
            && check.detail.contains("does not have an audit artifact")
    }));
}

#[test]
fn doctor_repair_noops_on_healthy_archive() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let repair = repair_health_issues(&paths, &config, None).expect("repair health");

    assert!(repair.run_id.is_none());
    assert_eq!(repair.repaired_import_audits, 0);
    assert_eq!(repair.repaired_visibility_rows, 0);
    assert_eq!(repair.cleared_derived_rows, 0);
    assert!(repair.notes.iter().any(|note| note.contains("found no actionable damage")));
}

#[test]
fn doctor_repair_restores_missing_import_artifacts_visibility_and_derived_state() {
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
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
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
    assert!(report.checks.iter().any(|check| check.name == "Import audit artifacts" && !check.ok));
    assert!(
        report.checks.iter().any(|check| check.name == "Broken visibility references" && !check.ok)
    );
    assert!(report.checks.iter().any(|check| check.name == "Derived state freshness" && !check.ok));

    let repair = repair_health_issues(&paths, &config, None).expect("repair health");
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

        let repair = repair_health_issues(&paths, &config, None).expect("repair health");

        assert!(repair.run_id.is_none());
        assert_eq!(repair.cleared_derived_rows, 0);
    }
}

#[test]
fn doctor_repair_restores_visibility_when_import_audits_are_intact() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "UPDATE visits SET reverted_at = ?1, reverted_by_run_id = NULL WHERE import_batch_id = ?2",
            params![now_rfc3339(), batch.id],
        )
        .expect("break visibility");

    let repair = repair_health_issues(&paths, &config, None).expect("repair health");

    assert_eq!(repair.repaired_import_audits, 0);
    assert_eq!(repair.repaired_visibility_rows, 1);
    assert!(repair.notes.iter().any(|note| note.contains("Re-linked 1 reverted visit rows")));
}

#[test]
fn doctor_repair_records_rebuilt_import_artifacts_when_git_is_enabled() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let import_config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &import_config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &paths,
        &import_config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    fs::remove_file(batch.audit_path.expect("audit path")).expect("remove import audit artifact");
    let repair_config = AppConfig { git_enabled: true, ..import_config };

    let repair = repair_health_issues(&paths, &repair_config, None).expect("repair health");

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

    let error = repair_health_issues(&paths, &config, None)
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
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch_id = inspection.import_batch.expect("batch").id;

    let dashboard_after_import =
        load_dashboard_snapshot(&paths, &config, None).expect("dashboard after import");
    assert_eq!(dashboard_after_import.total_visits, 1);
    assert_eq!(dashboard_after_import.total_urls, 1);
    let visible_after_import = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
    )
    .expect("query after import");
    assert_eq!(visible_after_import.total, 1);

    let after_import_stats: Value = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .query_row(
            "SELECT stats_json
             FROM runs
             WHERE run_type = 'import'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .map(|value| serde_json::from_str(&value).expect("parse import stats"))
        .expect("load import stats");
    assert_eq!(after_import_stats["totalVisits"], 1);

    crate::takeout::revert_import_batch(&paths, &config, None, batch_id)
        .expect("revert import batch");
    let dashboard_after_revert =
        load_dashboard_snapshot(&paths, &config, None).expect("dashboard after revert");
    assert_eq!(dashboard_after_revert.total_visits, 0);
    assert_eq!(dashboard_after_revert.total_urls, 1);
    let hidden_after_revert = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
    )
    .expect("query after revert");
    assert_eq!(hidden_after_revert.total, 0);

    let after_revert_stats: Value = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .query_row(
            "SELECT stats_json
             FROM runs
             WHERE run_type = 'rollback'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .map(|value| serde_json::from_str(&value).expect("parse revert stats"))
        .expect("load revert stats");
    assert_eq!(after_revert_stats["totalVisits"], 0);

    crate::takeout::restore_import_batch(&paths, &config, None, batch_id)
        .expect("restore import batch");
    let dashboard_after_restore =
        load_dashboard_snapshot(&paths, &config, None).expect("dashboard after restore");
    assert_eq!(dashboard_after_restore.total_visits, 1);
    assert_eq!(dashboard_after_restore.total_urls, 1);
    let visible_after_restore = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
    )
    .expect("query after restore");
    assert_eq!(visible_after_restore.total, 1);
}

#[test]
fn snapshot_restore_preview_and_run_record_the_saved_checkpoint() {
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
    let backup = run_backup(&paths, &config, None, false).expect("run backup");
    let snapshot_path: String = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("latest snapshot path");

    let preview = preview_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .expect("preview snapshot restore");
    assert!(preview.execute_supported);
    assert_eq!(preview.snapshot_kind, "raw-source-checkpoint");
    assert_eq!(preview.estimated_visits, 2);
    assert_eq!(preview.estimated_urls, 1);

    let restored = run_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .expect("run snapshot restore");
    let restore_run = restored.run.expect("restore run");
    assert_eq!(restore_run.run_type, "snapshot_restore");
    assert_eq!(restore_run.status, "success");
    assert!(backup.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));

    let detail =
        load_audit_run_detail(&paths, &config, None, restore_run.id).expect("restore detail");
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.reason.as_deref() == Some("restored-source-checkpoint"))
    );

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn snapshot_restore_records_failed_run_when_replay_cannot_persist() {
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
    run_backup(&paths, &config, None, false).expect("run backup");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let snapshot_path: String = connection
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("latest snapshot path");
    connection.execute("DROP TABLE visits", []).expect("damage archive schema");
    drop(connection);

    let restore_error =
        run_snapshot_restore(&paths, &config, None, &SnapshotRestoreRequest { snapshot_path })
            .expect_err("damaged archive should fail snapshot restore");
    let restore_error_chain = format!("{restore_error:#}");
    assert!(restore_error_chain.contains("visits"), "{restore_error_chain}");

    let failed_status: String = Connection::open(&paths.archive_database_path)
        .expect("reopen archive")
        .query_row(
            "SELECT status
             FROM runs
             WHERE run_type = 'snapshot_restore'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("failed restore run");
    assert_eq!(failed_status, "failed");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn source_evidence_spools_large_deferred_payloads_and_cleans_up_tempfiles() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let spool_dir = paths.staging_dir.join("source-evidence-spool");
    let large_blob = "x".repeat(300_000);

    {
        let deferred = defer_source_evidence_payload(
            &paths,
            "large-source-evidence",
            SourceEvidencePayload {
                typed_evidence: TypedEvidenceBatch {
                    context: vec![ContextEvidence {
                        source_visit_id: Some(1),
                        source_url_id: Some(1),
                        context_key: "context.takeout.large".to_string(),
                        value_json: serde_json::to_string(&large_blob).expect("serialize context"),
                        source_field: "payload".to_string(),
                    }],
                    ..TypedEvidenceBatch::default()
                },
                native_entities: vec![NativeEntity {
                    entity_kind: "takeout-browser-history".to_string(),
                    native_primary_key: "row-1".to_string(),
                    parent_native_primary_key: None,
                    payload_json: serde_json::json!({ "blob": large_blob }).to_string(),
                    metadata: BTreeMap::new(),
                }],
            },
        )
        .expect("defer source evidence");

        assert!(deferred.is_spooled());
        assert_eq!(fs::read_dir(&spool_dir).expect("read spool dir").count(), 1);
    }

    assert_eq!(fs::read_dir(&spool_dir).expect("read cleaned spool dir").count(), 0);
}

#[test]
fn snapshot_restore_preview_sizes_firefox_and_safari_checkpoints() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
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
    run_backup(&paths, &config, None, false).expect("run multi-browser backup");

    let snapshot_paths = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .prepare("SELECT file_path FROM snapshots ORDER BY id")
        .expect("prepare snapshot query")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query snapshots")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect snapshot paths");
    let previews = snapshot_paths
        .iter()
        .map(|snapshot_path| {
            preview_snapshot_restore(
                &paths,
                &config,
                None,
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
    run_backup(&paths, &config, None, false).expect("run backup");
    fs::create_dir_all(&paths.exports_dir).expect("create exports dir");
    fs::write(paths.exports_dir.join("export.jsonl"), "[]").expect("write export fixture");

    let preview = preview_retention(&paths, &config, None).expect("preview retention");
    assert!(preview.buckets.iter().any(|bucket| bucket.id == "snapshots" && bucket.bytes > 0));
    assert!(preview.buckets.iter().any(|bucket| bucket.id == "exports" && bucket.bytes > 0));

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

    let preview = preview_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect("manual safety snapshot preview");
    assert_eq!(preview.snapshot_kind, "archive-safety-snapshot");
    assert!(!preview.execute_supported);
    assert_eq!(preview.source_profile_id.as_deref(), Some("profile-a"));
    assert!(preview.warnings[0].contains("manual recovery"));
    let run_error = run_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect_err("manual safety copies are not automatically restored");
    assert!(run_error.to_string().contains("automatic restore"));

    let uninitialized = AppConfig { initialized: false, ..AppConfig::default() };
    let restore_uninitialized = run_snapshot_restore(
        &paths,
        &uninitialized,
        None,
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

#[test]
fn backup_crash_before_canonical_commit_rolls_back_to_pre_backup_state() {
    // Crash window: the process dies AFTER every selected profile's rows are staged into
    // the open write transaction but BEFORE it commits. SQLite must roll the transaction
    // back, so the on-disk archive stays at its pre-backup (empty) state — never a torn
    // half-write. The injected fault stands in for the kill/power-loss at that instant.
    let _env = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os(TEST_CHROME_USER_DATA_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let _fault =
        crate::fault_inject::FaultGuard::error_at_must_fire("backup.before_canonical_commit");
    let error = run_backup(&paths, &config, None, false)
        .expect_err("a crash before the canonical commit must abort the backup");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("backup.before_canonical_commit"),
        "the INJECTED fault must propagate, got: {rendered}"
    );

    // Reopen from disk (a fresh connection = the post-crash view): the staged rows rolled
    // back, the archive opens cleanly, and integrity_check passes.
    let connection = open_archive_connection(&paths, &config, None).expect("reopen archive");
    assert_eq!(
        canonical_visit_count(&connection),
        0,
        "the uncommitted visits must roll back to the pre-backup state"
    );
    assert_archive_integrity_ok(&connection);
    // A backup must never fail silently: the aborted run is recorded as `failed`.
    let failed_backups: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE run_type = 'backup' AND status = 'failed'",
            [],
            |row| row.get(0),
        )
        .expect("count failed backup runs");
    assert_eq!(failed_backups, 1, "the aborted backup must record a visible failed run");

    restore_test_env_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, original_override.as_deref());
}

#[test]
fn backup_crash_after_canonical_commit_leaves_a_fully_applied_consistent_archive() {
    // Crash window: the process dies AFTER the canonical history-vault transaction has
    // committed but BEFORE the source-evidence, manifest, and finalize follow-ups run. The
    // on-disk archive must be fully consistent at the newly-committed state — the canonical
    // facts are durable and never torn; only the recoverable follow-ups are deferred.
    let _env = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os(TEST_CHROME_USER_DATA_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let _fault =
        crate::fault_inject::FaultGuard::error_at_must_fire("backup.after_canonical_commit");
    let error = run_backup(&paths, &config, None, false)
        .expect_err("a crash after the canonical commit must abort the backup");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("backup.after_canonical_commit"),
        "the INJECTED fault must propagate, got: {rendered}"
    );

    // Reopen from disk: the canonical visits are present (fully applied) and consistent.
    let connection = open_archive_connection(&paths, &config, None).expect("reopen archive");
    assert_eq!(
        canonical_visit_count(&connection),
        2,
        "the committed canonical visits must survive the crash, fully applied"
    );
    assert_archive_integrity_ok(&connection);

    restore_test_env_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, original_override.as_deref());
}

#[cfg(unix)]
#[test]
fn scheduled_backup_defers_when_another_process_holds_the_write_lock() {
    // CRITICAL-1 / CRITICAL-5: a due (`due_only`) scheduled backup must DEFER — never race —
    // when another OS process already holds the archive write lock (e.g. a foreground rekey
    // mid-swap). We simulate the foreign process with a RAW second `flock` so THIS process's
    // `try_acquire` is genuinely refused (an in-process guard would instead be handed back a
    // reentrant guard and could not reproduce the defer). The run must step aside with a
    // visible reason and write NOTHING — no archive open, no run row, no rows.
    let _env = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os(TEST_CHROME_USER_DATA_OVERRIDE_ENV);
    unsafe {
        std::env::set_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    // Hold the lock as if from another OS process (raw fd, NOT the reentrant manager).
    let foreign = super::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

    let report = run_backup(&paths, &config, None, true)
        .expect("a contended scheduled backup defers cleanly; it must not error");
    assert!(report.due_skipped, "the scheduled backup must defer while the lock is held");
    assert!(
        report.reason.as_deref().is_some_and(|reason| reason.contains("Another archive operation")),
        "the deferral reason must be visible, got: {:?}",
        report.reason
    );
    assert!(report.run.is_none(), "a deferred backup must not produce a run");

    drop(foreign);

    // The deferral happened BEFORE opening the archive: nothing was written.
    let connection = open_archive_connection(&paths, &config, None).expect("reopen archive");
    let backup_runs: i64 = connection
        .query_row("SELECT COUNT(*) FROM runs WHERE run_type = 'backup'", [], |row| row.get(0))
        .expect("count backup runs");
    assert_eq!(backup_runs, 0, "a deferred scheduled backup must not write any run");
    assert_eq!(canonical_visit_count(&connection), 0, "a deferred backup must write nothing");

    restore_test_env_var(TEST_CHROME_USER_DATA_OVERRIDE_ENV, original_override.as_deref());
}

#[test]
fn run_support_failed_runs_and_due_windows_stay_truthful() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, due_after_hours: 72.0, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let started_at = now_rfc3339();
    connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, 'UTC', 'running', '[\"profile-a\"]', '[]', '{}', 0)",
            params![started_at],
        )
        .expect("insert running run");
    let run_id = connection.last_insert_rowid();

    finalize_failed_run(
        &connection,
        run_id,
        &[
            BackupProfileSummary {
                profile_id: "profile-a".to_string(),
                new_visits: 2,
                new_urls: 1,
                new_downloads: 0,
                checkpoint_created: true,
                notes: vec!["partial".to_string()],
            },
            BackupProfileSummary {
                profile_id: "profile-b".to_string(),
                new_visits: 3,
                new_urls: 2,
                new_downloads: 1,
                checkpoint_created: false,
                notes: Vec::new(),
            },
        ],
        &["warning".to_string()],
        &anyhow::anyhow!("fixture failure"),
    )
    .expect("finalize failed run");

    let (status, stats_json, warnings_json, error_message): (String, String, String, String) =
        connection
            .query_row(
                "SELECT status, stats_json, warnings_json, error_message FROM runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("failed run row");
    let stats: serde_json::Value = serde_json::from_str(&stats_json).expect("stats json");
    let warnings: Vec<String> = serde_json::from_str(&warnings_json).expect("warnings json");
    assert_eq!(status, "failed");
    assert_eq!(stats["profilesProcessed"], 2);
    assert_eq!(stats["newVisits"], 5);
    assert_eq!(stats["newUrls"], 3);
    assert_eq!(stats["newDownloads"], 1);
    assert_eq!(warnings, vec!["warning"]);
    assert!(error_message.contains("fixture failure"));

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
            "UPDATE runs
             SET status = 'success', finished_at = ?1, error_message = NULL
             WHERE id = ?2",
            params![recent.to_rfc3339(), run_id],
        )
        .expect("mark successful backup");
    let due_reason = super::run_support::backup_due_skip_reason(&connection, &config)
        .expect("due skip query")
        .expect("recent success should skip");
    assert!(due_reason.contains("minutes old"));
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
