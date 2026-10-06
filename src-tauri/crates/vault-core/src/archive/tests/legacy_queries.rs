//! Responsibilities: keep the unported SQLite history, favicon and export contracts covered.
//! Not responsible for: proving Turso backup writes, schema migration or copying between stores.
//! Failure modes: query pagination drops rows, fallback favicons bleed between sites,
//! exports truncate pages, lookup plans scan visits. These use independent SQL facts.
use super::*;

#[test]
fn legacy_history_favicons_exports_and_plans_use_native_fixtures() {
    let dir = tempdir().unwrap();
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, git_enabled: false, ..Default::default() };
    seed_reader_archive(&paths, &config);
    let recent_runs = legacy_recent_runs(&paths, &config, None);
    assert!(!recent_runs.is_empty());
    assert!(recent_runs.iter().any(|run| run.run_type == "backup" && run.status == "success"));

    let history = search_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().all(|entry| entry.favicon.is_none()));

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

    let search_term_history = search_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("deep recall".to_string()), ..HistoryQuery::default() },
    )
    .expect("list search term history");
    assert_eq!(search_term_history.total, 2);
    let search_term_first_page = search_history(
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
    let search_term_second_cursor_page = search_history(
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
    let search_term_explicit_second_page = search_history(
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

    let url_fragment_history = search_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("example.com/archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list url fragment history");
    assert_eq!(url_fragment_history.total, 2);

    let punctuation_only_history = search_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("!!!".to_string()), ..HistoryQuery::default() },
    )
    .expect("list punctuation-only history");
    assert_eq!(punctuation_only_history.total, 0);

    let regex_history = search_history(
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
    let regex_oldest_first_page = search_history(
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
    let regex_oldest_second_page = search_history(
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
    let regex_newest_first_page = search_history(
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
    let regex_newest_second_page = search_history(
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
    let regex_explicit_second_page = search_history(
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

    let invalid_regex = search_history(
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
    let unsupported_regex = search_history(
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

    let first_page = search_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("first history page");
    assert_eq!(first_page.total, 2);
    assert_eq!(first_page.items.len(), 1);
    assert!(first_page.next_cursor.is_some());

    let second_page = search_history(
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

    let explicit_second_page = search_history(
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
    let empty_domain_history = search_history(
        &paths,
        &config,
        None,
        HistoryQuery { domain: Some("missing.invalid".to_string()), ..HistoryQuery::default() },
    )
    .expect("empty domain history");
    assert_eq!(empty_domain_history.total, 0);
    assert_eq!(empty_domain_history.page_count, 1);

    let mut connection = open_archive_connection(&paths, &config, None).unwrap();
    let run_id = 1;
    let paged_export = export_search_history(
        &paths,
        &config,
        None,
        ExportRequest {
            export_id: None,
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
    let html_export = export_search_history(
        &paths,
        &config,
        None,
        ExportRequest {
            export_id: None,
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Html,
        },
    )
    .expect("export history html");
    assert_eq!(html_export.count, 2);
    let html_content = fs::read_to_string(&html_export.path).expect("read html export");
    assert!(html_content.contains("<article>"));
    assert!(html_content.contains("Archive docs"));
    let markdown_export = export_search_history(
        &paths,
        &config,
        None,
        ExportRequest {
            export_id: None,
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Markdown,
        },
    )
    .expect("export history markdown");
    let markdown_content = fs::read_to_string(&markdown_export.path).expect("read markdown export");
    assert!(markdown_content.contains("- [Archive docs](https://example.com/archive)"));
    let text_export = export_search_history(
        &paths,
        &config,
        None,
        ExportRequest {
            export_id: None,
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
    let bulk_export = export_search_history(
        &paths,
        &config,
        None,
        ExportRequest {
            export_id: None,
            query: HistoryQuery {
                domain: Some("bulk.example".to_string()),
                ..HistoryQuery::default()
            },
            format: ExportFormat::Jsonl,
        },
    )
    .expect("export multi-page history");
    assert_eq!(bulk_export.count, 1001);
    // Rows are streamed page by page: the 1,001 rows cross a page boundary, and each must be
    // written exactly once.
    let bulk_lines = fs::read_to_string(&bulk_export.path).expect("read bulk export");
    let bulk_ids = bulk_lines
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("json line")["id"].clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(bulk_ids.len(), 1001);
    assert_eq!(bulk_ids.iter().map(|id| id.to_string()).collect::<HashSet<_>>().len(), 1001);

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
}

pub(in crate::archive) fn seed_reader_archive(paths: &ProjectPaths, config: &AppConfig) {
    let db = open_archive_connection(paths, config, None).unwrap();
    db.execute_batch("INSERT INTO runs(id,run_type,trigger,started_at,finished_at,status,stats_json) VALUES(1,'backup','manual','2026-04-05','2026-04-05','success','{}');
        INSERT INTO source_profiles(id,browser_kind,browser_family,browser_product,profile_name,profile_path,discovered_at,enabled,profile_key,updated_at)
        VALUES(1,'chrome','chromium','Google Chrome','Default','/fixture','2026-04-05',1,'chrome:Default','2026-04-05');
        INSERT INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,registrable_domain)
        VALUES(1,'https://example.com/archive','Archive docs',2,0,1775383200000,'2026-04-05T10:00:00+00:00',1775386800000,'2026-04-05T11:00:00+00:00',1,1,1,'example.com');
        INSERT INTO visits(id,source_profile_id,url_id,visit_time_ms,visit_time_iso,transition_type,created_by_run_id,source_visit_id)
        VALUES(1,1,1,1775383200000,'2026-04-05T10:00:00+00:00',805306368,1,'1'),(2,1,1,1775386800000,'2026-04-05T11:00:00+00:00',805306368,1,'2');
        INSERT INTO search_terms(url_id,term,normalized_term,source_profile_id,created_by_run_id) VALUES(1,'deep recall token','deep recall token',1,1);
        INSERT INTO favicon_blobs(blob_hash,image_data,recorded_at) VALUES('fixture-icon',X'89504E470D0A1A0A01','2026-04-05');
        INSERT INTO favicons(page_url,icon_url,width,height,last_updated_ms,last_updated_iso,image_blob_hash,page_host,page_registrable_domain,source_profile_id,created_by_run_id)
        VALUES('https://example.com/archive','https://example.com/favicon.ico',16,16,1775386800000,'2026-04-05T11:00:00+00:00','fixture-icon','example.com','example.com',1,1),
        ('https://example.com/archive','https://example.com/app-icon.ico',16,16,1775386800000,'2026-04-05T11:00:00+00:00','fixture-icon','example.com','example.com',1,1);").unwrap();
    rebuild_search_projection(paths, config, None).unwrap();
}
