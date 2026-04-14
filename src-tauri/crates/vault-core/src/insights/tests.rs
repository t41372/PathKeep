//! Regression tests for deterministic insights and enrichment orchestration.
use super::*;
use crate::{
    archive::{ensure_archive_initialized, open_archive_connection},
    config::project_paths_with_root,
    models::{
        AiProviderConfig, AiProviderPurpose, AiRequestFormat, AiSettings, ArchiveMode,
        InsightQueryGroupSummary, InsightReferencePageSummary,
    },
};
use tempfile::tempdir;

fn test_paths() -> ProjectPaths {
    let dir = tempdir().expect("tempdir");
    project_paths_with_root(dir.path())
}

fn test_config() -> AppConfig {
    let mut config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ai: AiSettings::default(),
        ..AppConfig::default()
    };
    config.ai.embedding_provider_id = Some("embed".to_string());
    config.ai.embedding_providers = vec![AiProviderConfig {
        id: "embed".to_string(),
        name: "Embed".to_string(),
        purpose: AiProviderPurpose::Embedding,
        request_format: AiRequestFormat::OpenAi,
        enabled: true,
        default_model: "text-embedding-3-large".to_string(),
        dimensions: Some(8),
        ..AiProviderConfig::default()
    }];
    config
}

fn seed_visits(connection: &Connection) {
    let visit_one = (Utc::now() - Duration::days(10)).to_rfc3339();
    let visit_two = (Utc::now() - Duration::days(10) + Duration::minutes(12)).to_rfc3339();
    let visit_three = (Utc::now() - Duration::days(8)).to_rfc3339();
    let visit_four = (Utc::now() - Duration::days(8) + Duration::minutes(28)).to_rfc3339();
    let visit_five = (Utc::now() - Duration::days(2)).to_rfc3339();
    let visit_six = (Utc::now() - Duration::days(365)).to_rfc3339();
    connection
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert test run");
    connection
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (1, 'chrome', 'test', 'Default', '/tmp/chrome-default', ?1, 1, 'chrome:Default', ?1)",
            [now_rfc3339()],
        )
        .expect("insert test profile");
    connection
        .execute(
            "INSERT INTO urls
             (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
             VALUES
             (1, 'https://example.com/docs/archive', 'Archive docs', 1, 0, ?1, ?6, ?1, ?6, 1, 1, 1, 0, 'a', ?6),
             (2, 'https://github.com/example/repo/issues/1', 'Issue one', 1, 0, ?2, ?7, ?2, ?7, 1, 1, 2, 0, 'b', ?7),
             (3, 'https://www.google.com/search?q=archive+tool+compare', 'Google Search', 1, 0, ?3, ?8, ?3, ?8, 1, 1, 3, 0, 'c', ?8),
             (4, 'https://www.google.com/search?q=archive+tool+compare+github', 'Google Search Refined', 1, 0, ?4, ?9, ?4, ?9, 1, 1, 4, 0, 'd', ?9),
             (5, 'https://example.com/pricing', 'Pricing', 1, 0, ?5, ?10, ?5, ?10, 1, 1, 5, 0, 'e', ?10),
             (6, 'https://example.com/on-this-day', 'On this day', 1, 0, ?11, ?12, ?11, ?12, 1, 1, 6, 0, 'f', ?12)",
            params![
                DateTime::parse_from_rfc3339(&visit_one).expect("visit one time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_two).expect("visit two time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_three).expect("visit three time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_four).expect("visit four time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_five).expect("visit five time").timestamp_millis(),
                visit_one,
                visit_two,
                visit_three,
                visit_four,
                visit_five,
                DateTime::parse_from_rfc3339(&visit_six).expect("visit six time").timestamp_millis(),
                visit_six,
            ],
        )
        .expect("insert urls");
    connection
        .execute(
            "INSERT INTO visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, external_referrer_url, event_fingerprint, payload_hash, recorded_at)
             VALUES
             (1, 1, '1', ?1, ?6, 805306368, 24000, 1, 1, NULL, 1, 'https://google.com', 'a', 'a', ?6),
             (2, 2, '2', ?2, ?7, 805306368, 12000, 1, 1, 1, 1, NULL, 'b', 'b', ?7),
             (3, 3, '3', ?3, ?8, 805306368, 6000, 1, 1, NULL, 1, NULL, 'c', 'c', ?8),
             (4, 4, '4', ?4, ?9, 805306368, 8000, 1, 1, NULL, 1, NULL, 'd', 'd', ?9),
             (5, 5, '5', ?5, ?10, 805306368, 5000, 1, 1, NULL, 1, NULL, 'e', 'e', ?10),
             (6, 6, '6', ?11, ?12, 805306368, 5000, 1, 1, NULL, 1, NULL, 'f', 'f', ?12)",
            params![
                DateTime::parse_from_rfc3339(&visit_one).expect("visit one time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_two).expect("visit two time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_three).expect("visit three time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_four).expect("visit four time").timestamp_millis(),
                DateTime::parse_from_rfc3339(&visit_five).expect("visit five time").timestamp_millis(),
                visit_one,
                visit_two,
                visit_three,
                visit_four,
                visit_five,
                DateTime::parse_from_rfc3339(&visit_six).expect("visit six time").timestamp_millis(),
                visit_six,
            ],
        )
        .expect("insert visits");
    connection
        .execute(
            "INSERT INTO search_terms (
               url_id,
               term,
               normalized_term,
               source_profile_id,
               created_by_run_id,
               profile_id,
               keyword_id,
               recorded_at
             )
             VALUES
             (
               3,
               'archive tool compare',
               'archive tool compare',
               (SELECT id FROM source_profiles WHERE profile_key = 'chrome:Default'),
               0,
               'chrome:Default',
               1,
               ?1
             ),
             (
               4,
               'archive tool compare github',
               'archive tool compare github',
               (SELECT id FROM source_profiles WHERE profile_key = 'chrome:Default'),
               0,
               'chrome:Default',
               2,
               ?2
             )",
            params![visit_three, visit_four],
        )
        .expect("insert search term");
}

fn insert_test_visit(
    connection: &Connection,
    history_id: i64,
    url: &str,
    title: &str,
    visited_at_iso: &str,
    from_visit: Option<i64>,
    external_referrer_url: Option<&str>,
) {
    let visit_time_ms =
        DateTime::parse_from_rfc3339(visited_at_iso).expect("visit time").timestamp_millis();
    connection
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert test run");
    connection
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (1, 'chrome', 'test', 'Default', '/tmp/chrome-default', ?1, 1, 'chrome:Default', ?1)",
            [now_rfc3339()],
        )
        .expect("insert test profile");
    connection
        .execute(
            "INSERT INTO urls
             (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, 1, 1, ?1, 0, ?6, ?5)",
            params![history_id, url, title, visit_time_ms, visited_at_iso, format!("payload-{history_id}")],
        )
        .expect("insert test url");
    connection
        .execute(
            "INSERT INTO visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, external_referrer_url, event_fingerprint, payload_hash, recorded_at)
             VALUES (?1, ?1, ?2, ?3, ?4, 805306368, 1000, 1, 1, ?5, 1, ?6, ?7, ?8, ?4)",
            params![
                history_id,
                history_id.to_string(),
                visit_time_ms,
                visited_at_iso,
                from_visit,
                external_referrer_url,
                format!("fp-{history_id}"),
                format!("payload-{history_id}"),
            ],
        )
        .expect("insert test visit");
}

#[test]
fn insight_schema_and_snapshot_roundtrip() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    seed_visits(&archive);

    let report = run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("run insights");
    assert!(report.processed_visits >= 4);

    let snapshot = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load insights");
    assert!(!snapshot.cards.is_empty());
    assert!(!snapshot.threads.is_empty());
    assert!(!snapshot.canonical.on_this_day.is_empty());
    assert!(!snapshot.canonical.top_domains.is_empty());
    assert!(!snapshot.query_ladders.is_empty());
    assert!(snapshot.template_summaries.iter().any(|summary| summary.kind == "periodic-summary"));
    assert!(
        snapshot.template_summaries.iter().any(|summary| summary.kind == "contrastive-summary")
    );
    assert!(snapshot.query_ladders[0].steps.len() > 1);
    assert!(snapshot.workflow_map.chromium_enhanced);
}

#[test]
fn scoped_persistence_keeps_30_day_and_365_day_snapshots_isolated() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("schema");
    ensure_insight_schema(&connection).expect("insight schema");
    seed_visits(&connection);

    let older_visit_one = (Utc::now() - Duration::days(90)).to_rfc3339();
    let older_visit_two = (Utc::now() - Duration::days(60)).to_rfc3339();
    insert_test_visit(
        &connection,
        100,
        "https://example.com/docs/archive",
        "Archive docs",
        &older_visit_one,
        None,
        None,
    );
    insert_test_visit(
        &connection,
        101,
        "https://example.com/docs/archive",
        "Archive docs",
        &older_visit_two,
        Some(100),
        None,
    );

    run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("run 30 day insights");

    let before = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load 30 day snapshot before 365 day rebuild");
    assert!(
        before.reference_pages.iter().all(|page| page.url != "https://example.com/docs/archive"),
        "30 day snapshot should not include the older reference page yet"
    );

    run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(365),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("run 365 day insights");

    let after = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load 30 day snapshot after 365 day rebuild");
    assert!(
        after.reference_pages.iter().all(|page| page.url != "https://example.com/docs/archive"),
        "30 day snapshot must stay isolated from the 365 day rebuild"
    );

    let year_snapshot = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(365),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load 365 day snapshot");
    assert!(
        year_snapshot
            .reference_pages
            .iter()
            .any(|page| page.url == "https://example.com/docs/archive"),
        "365 day snapshot should keep the older reference page"
    );
}

#[test]
fn load_insights_can_use_persisted_snapshot_payloads_without_feature_rows() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("schema");
    ensure_insight_schema(&connection).expect("insight schema");
    seed_visits(&connection);

    run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("run insights");

    connection.execute("DELETE FROM visit_insight_features", []).expect("delete feature rows");

    let snapshot = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load persisted snapshot");
    assert!(!snapshot.query_ladders.is_empty());
    assert!(!snapshot.template_summaries.is_empty());
    assert!(snapshot.workflow_map.chromium_enhanced);
    assert!(!snapshot.profile_facets.is_empty());
    assert!(!snapshot.canonical.top_domains.is_empty());
}

#[test]
fn failed_full_rebuild_keeps_previous_snapshot_visible() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("schema");
    ensure_insight_schema(&connection).expect("insight schema");
    seed_visits(&connection);

    run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("seed snapshot");
    let before = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load snapshot before failure");

    let error = run_insights_with_progress(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: true,
            limit: None,
        },
        |progress| {
            if progress.phase_step >= 7 {
                return Err(anyhow::anyhow!("stop before commit"));
            }
            Ok(())
        },
    )
    .expect_err("full rebuild should fail before commit");
    assert!(error.to_string().contains("stop before commit"));

    let after = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load snapshot after failed rebuild");
    assert_eq!(after.cards.len(), before.cards.len());
    assert_eq!(after.query_groups.len(), before.query_groups.len());
    assert_eq!(after.threads.len(), before.threads.len());
    assert_eq!(after.canonical.window_visit_count, before.canonical.window_visit_count);
}

#[test]
fn on_this_day_excludes_current_year_visits() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("core schema");
    let current_year_visit = Local::now().to_rfc3339();
    let prior_year_visit = (Local::now() - Duration::days(365)).to_rfc3339();
    insert_test_visit(
        &connection,
        1,
        "https://example.com/today",
        "Today",
        &current_year_visit,
        None,
        None,
    );
    insert_test_visit(
        &connection,
        2,
        "https://example.com/last-year",
        "Last year",
        &prior_year_visit,
        None,
        None,
    );
    drop(connection);

    let intelligence =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");

    let items =
        load_on_this_day(&intelligence, Some("chrome:Default"), 8).expect("load on this day");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].url, "https://example.com/last-year");
}

#[test]
fn explicit_analysis_limit_can_still_sample_recent_visits() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("schema");
    ensure_insight_schema(&connection).expect("insight schema");
    seed_visits(&connection);

    let report = run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: Some(2),
        },
    )
    .expect("run limited insights");
    assert_eq!(report.processed_visits, 2);
}

#[test]
fn feature_scoring_penalizes_repeated_token_sets_without_quadratic_scan() {
    fn score_visit(history_id: i64, url: &str, keywords: &[&str]) -> VisitRecord {
        VisitRecord {
            history_id,
            profile_id: "chrome:Default".to_string(),
            source_visit_id: history_id,
            source_url_id: history_id,
            url: url.to_string(),
            title: Some(url.to_string()),
            visited_at: now_rfc3339(),
            visit_time: history_id,
            from_visit: None,
            transition: None,
            duration_ms: None,
            external_referrer_url: None,
            app_id: None,
            query_term: None,
            has_canonical_search_term: false,
            readable_title: None,
            readable_text: None,
            snippets: Vec::new(),
            source_role: "reference".to_string(),
            page_type: "docs-page".to_string(),
            domain_category: DomainCategory::Docs,
            page_category_v2: PageCategory::DocsPage,
            interaction_kind: InteractionKind::Learn,
            evidence_tier: EvidenceTier::TierB,
            taxonomy_source: "test".to_string(),
            taxonomy_pack: None,
            taxonomy_version: None,
            taxonomy_reason: None,
            registrable_domain: "example.com".to_string(),
            keywords: keywords.iter().map(|value| value.to_string()).collect(),
            entities: Vec::new(),
            novelty_score: 0.0,
            importance_score: 0.0,
            explore_score: 0.0,
            burst_id: None,
            query_group_id: None,
            topic_id: None,
            thread_id: None,
        }
    }

    let mut visits = vec![
        score_visit(1, "https://example.com/docs/rust", &["rust", "sqlite"]),
        score_visit(2, "https://example.com/docs/rust-2", &["rust", "sqlite"]),
        score_visit(3, "https://example.com/docs/safari", &["safari", "automation"]),
    ];

    compute_feature_scores(&mut visits);

    assert!(visits[0].novelty_score > visits[1].novelty_score);
    assert!(visits[2].novelty_score > visits[1].novelty_score);
    assert_eq!(visits[1].novelty_score, 0.0);
}

#[test]
fn feature_scoring_emits_progress_at_start_and_finish() {
    fn score_visit(history_id: i64, keyword: &str) -> VisitRecord {
        VisitRecord {
            history_id,
            profile_id: "chrome:Default".to_string(),
            source_visit_id: history_id,
            source_url_id: history_id,
            url: format!("https://example.com/{keyword}/{history_id}"),
            title: Some(format!("Visit {history_id}")),
            visited_at: now_rfc3339(),
            visit_time: history_id,
            from_visit: None,
            transition: None,
            duration_ms: None,
            external_referrer_url: None,
            app_id: None,
            query_term: None,
            has_canonical_search_term: false,
            readable_title: None,
            readable_text: None,
            snippets: Vec::new(),
            source_role: "reference".to_string(),
            page_type: "docs-page".to_string(),
            domain_category: DomainCategory::Docs,
            page_category_v2: PageCategory::DocsPage,
            interaction_kind: InteractionKind::Learn,
            evidence_tier: EvidenceTier::TierB,
            taxonomy_source: "test".to_string(),
            taxonomy_pack: None,
            taxonomy_version: None,
            taxonomy_reason: None,
            registrable_domain: "example.com".to_string(),
            keywords: vec![keyword.to_string()],
            entities: Vec::new(),
            novelty_score: 0.0,
            importance_score: 0.0,
            explore_score: 0.0,
            burst_id: None,
            query_group_id: None,
            topic_id: None,
            thread_id: None,
        }
    }

    let mut visits = vec![score_visit(1, "alpha"), score_visit(2, "beta"), score_visit(3, "gamma")];
    let mut progress = Vec::new();

    compute_feature_scores_with_progress(&mut visits, |processed, total| {
        progress.push((processed, total));
        Ok(())
    })
    .expect("score visits with progress");

    assert_eq!(progress.first().copied(), Some((0, 3)));
    assert_eq!(progress.last().copied(), Some((3, 3)));
}

#[test]
fn ensure_insight_schema_requires_fresh_init_shape_for_visit_features() {
    let connection = Connection::open_in_memory().expect("db");
    connection
        .execute_batch(
            r#"
CREATE TABLE visit_insight_features (
  history_id INTEGER PRIMARY KEY,
  profile_id TEXT NOT NULL,
  topic_id TEXT,
  thread_id TEXT,
  page_type TEXT NOT NULL,
  source_role TEXT NOT NULL,
  query_term TEXT,
  query_stage TEXT,
  novelty_score REAL NOT NULL,
  importance_score REAL NOT NULL,
  explore_score REAL NOT NULL,
  keywords_json TEXT NOT NULL,
  entities_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  pipeline_version TEXT NOT NULL
);
"#,
        )
        .expect("legacy insight feature table");

    let error = ensure_insight_schema(&connection).expect_err("legacy schema should fail");
    assert!(error.to_string().contains("burst_id"));
}

#[test]
fn source_effectiveness_counts_distinct_reference_pages_per_domain() {
    let base_visit = VisitRecord {
        history_id: 1,
        profile_id: "chrome:Default".to_string(),
        source_visit_id: 1,
        source_url_id: 1,
        url: "https://docs.example.com/a".to_string(),
        title: Some("A".to_string()),
        visited_at: now_rfc3339(),
        visit_time: 1,
        from_visit: None,
        transition: None,
        duration_ms: None,
        external_referrer_url: None,
        app_id: None,
        query_term: Some("sqlite wal".to_string()),
        has_canonical_search_term: true,
        readable_title: None,
        readable_text: None,
        snippets: Vec::new(),
        source_role: "docs".to_string(),
        page_type: "docs-page".to_string(),
        domain_category: DomainCategory::Docs,
        page_category_v2: PageCategory::DocsPage,
        interaction_kind: InteractionKind::Learn,
        evidence_tier: EvidenceTier::TierA,
        taxonomy_source: "test".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        taxonomy_reason: None,
        registrable_domain: "example.com".to_string(),
        keywords: vec!["sqlite".to_string()],
        entities: Vec::new(),
        novelty_score: 0.0,
        importance_score: 0.0,
        explore_score: 0.0,
        burst_id: None,
        query_group_id: Some("group-a".to_string()),
        topic_id: None,
        thread_id: Some("thread-a".to_string()),
    };
    let visits = vec![
        base_visit.clone(),
        VisitRecord {
            history_id: 2,
            source_visit_id: 2,
            source_url_id: 2,
            visit_time: 2,
            ..base_visit
        },
    ];
    let reference_pages = vec![
        InsightReferencePageSummary {
            reference_page_id: "page-a".to_string(),
            profile_id: Some("chrome:Default".to_string()),
            url: "https://docs.example.com/a".to_string(),
            title: Some("A".to_string()),
            domain: "example.com".to_string(),
            first_seen_at: now_rfc3339(),
            last_seen_at: now_rfc3339(),
            revisit_count: 2,
            cross_day_revisits: 1,
            query_group_count: 1,
            thread_count: 1,
            score: 4.0,
            evidence_tier: "tier-a".to_string(),
            evidence: Vec::new(),
        },
        InsightReferencePageSummary {
            reference_page_id: "page-b".to_string(),
            profile_id: Some("chrome:Default".to_string()),
            url: "https://docs.example.com/b".to_string(),
            title: Some("B".to_string()),
            domain: "example.com".to_string(),
            first_seen_at: now_rfc3339(),
            last_seen_at: now_rfc3339(),
            revisit_count: 1,
            cross_day_revisits: 1,
            query_group_count: 1,
            thread_count: 1,
            score: 3.0,
            evidence_tier: "tier-a".to_string(),
            evidence: Vec::new(),
        },
    ];

    let rows = build_source_effectiveness(&visits, "chrome:Default", &reference_pages);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reference_page_count, 2);
}

#[test]
fn template_summaries_skip_single_step_query_groups() {
    let query_groups = vec![InsightQueryGroupSummary {
        query_group_id: "group-a".to_string(),
        profile_id: "chrome:Default".to_string(),
        thread_id: None,
        title: "SQLite WAL".to_string(),
        root_query: "sqlite wal".to_string(),
        latest_query: "sqlite wal".to_string(),
        first_seen_at: now_rfc3339(),
        last_seen_at: now_rfc3339(),
        visit_count: 1,
        burst_count: 1,
        step_count: 1,
        confidence: 0.6,
        evidence_tier: "tier-a".to_string(),
        chromium_enhanced: true,
        steps: vec!["sqlite wal".to_string()],
        stages: vec!["broad".to_string()],
        evidence: Vec::new(),
    }];

    let summaries = build_template_summaries(
        &[],
        &query_groups,
        &[],
        &[],
        &[],
        Some("chrome:Default"),
        30,
        None,
    );
    assert!(summaries.iter().all(|summary| summary.kind != "query-groups"));
}

#[test]
fn readable_content_plugin_can_be_disabled_and_cleared() {
    let paths = test_paths();
    let mut config = test_config();
    if let Some(plugin) =
        config.enrichment.plugins.iter_mut().find(|plugin| plugin.id == READABLE_CONTENT_PLUGIN_ID)
    {
        plugin.enabled = false;
    }
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("schema");
    ensure_insight_schema(&connection).expect("insight schema");
    seed_visits(&connection);

    let report = run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: Some(20),
        },
    )
    .expect("run insights with plugin disabled");
    assert!(report.enriched_visits > 0);
    let readable_content_rows = connection
        .query_row(
            "SELECT COUNT(*) FROM visit_content_enrichments WHERE content_source = ?1",
            [READABLE_CONTENT_PLUGIN_ID],
            |row: &Row<'_>| row.get::<_, i64>(0),
        )
        .expect("readable content row count");
    assert_eq!(readable_content_rows, 0);
    assert!(report.notes.iter().any(|note| note.contains("successful plugin jobs")));

    let cleared =
        clear_derived_intelligence_state(&paths, &config, None).expect("clear derived state");
    assert!(cleared.cleared_card_rows > 0);
    let snapshot = load_insights(
        &paths,
        &config,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("load cleared snapshot");
    assert!(snapshot.cards.is_empty());
    assert!(snapshot.threads.is_empty());
    assert!(!snapshot.canonical.on_this_day.is_empty());
}

#[test]
fn enrichment_failure_message_turns_known_fetch_states_into_honest_copy() {
    assert_eq!(
        enrichment_failure_message(&EnrichmentResult {
            status: "unsupported-content".to_string(),
            extraction: json!({ "contentType": "application/pdf" }),
            ..EnrichmentResult::default()
        }),
        "Skipped non-readable content (application/pdf)."
    );
    assert_eq!(
        enrichment_failure_message(&EnrichmentResult {
            status: "fetch-error".to_string(),
            extraction: json!({ "error": "error following redirect" }),
            ..EnrichmentResult::default()
        }),
        "Could not fetch the page again. error following redirect"
    );
    assert_eq!(
        enrichment_failure_message(&EnrichmentResult {
            status: "decode-error".to_string(),
            extraction: json!({
                "contentType": "text/html; charset=UTF-8",
                "error": "invalid utf-8"
            }),
            ..EnrichmentResult::default()
        }),
        "Could not decode the response body (text/html; charset=UTF-8). invalid utf-8"
    );
}

#[test]
fn run_insights_leaves_network_enrichment_jobs_queued_for_later_review() {
    let paths = test_paths();
    let config = test_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&connection).expect("schema");
    ensure_insight_schema(&connection).expect("insight schema");
    ensure_intelligence_runtime_schema(&connection).expect("runtime schema");
    seed_visits(&connection);

    let report = run_insights(
        &paths,
        &config,
        None,
        None,
        &RunInsightsRequest {
            profile_id: Some("chrome:Default".to_string()),
            window_days: Some(30),
            full_rebuild: false,
            limit: None,
        },
    )
    .expect("run insights");

    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    let (queued, running, succeeded) = connection
        .query_row(
            "SELECT
               SUM(CASE WHEN state = 'queued' THEN 1 ELSE 0 END),
               SUM(CASE WHEN state = 'running' THEN 1 ELSE 0 END),
               SUM(CASE WHEN state = 'succeeded' THEN 1 ELSE 0 END)
             FROM intelligence_jobs
             WHERE job_type = ?1 AND plugin_id = ?2",
            params![ENRICHMENT_JOB_TYPE, READABLE_CONTENT_PLUGIN_ID],
            |row: &Row<'_>| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                ))
            },
        )
        .expect("readable content job counts");

    assert!(queued > 0);
    assert_eq!(running, 0);
    assert_eq!(succeeded, 0);
    assert!(report.enriched_visits > 0);
    assert!(report.notes.iter().any(|note| note.contains("Deferred")));
}

#[test]
fn preferred_embedding_content_prefers_enriched_text() {
    let paths = test_paths();
    let connection = Connection::open_in_memory().expect("db");
    ensure_insight_schema(&connection).expect("schema");
    store_enrichment(
        &paths,
        &connection,
        7,
        "refetch",
        &EnrichmentResult {
            status: "success".to_string(),
            final_url: Some("https://example.com/final".to_string()),
            language: Some("en".to_string()),
            readable_title: Some("Readable".to_string()),
            readable_text: Some("Readable text body".to_string()),
            snippets: vec!["Readable text body".to_string()],
            extraction: json!({}),
        },
    )
    .expect("store enrichment");
    let content = preferred_embedding_content(
        &paths,
        &connection,
        7,
        "chrome:Default",
        "https://example.com",
        Some("Original"),
        "2026-04-03T00:00:00Z",
    )
    .expect("embedding content");
    assert!(content.contains("Readable text body"));
    assert!(content.contains("Readable title"));
}

#[test]
fn run_insights_marks_runs_failed_when_job_payload_is_invalid() {
    let paths = test_paths();
    let mut config = test_config();
    if let Some(plugin) =
        config.enrichment.plugins.iter_mut().find(|plugin| plugin.id == READABLE_CONTENT_PLUGIN_ID)
    {
        plugin.enabled = false;
    }
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    seed_visits(&archive);
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    ensure_insight_schema(&connection).expect("insight schema");
    ensure_intelligence_runtime_schema(&connection).expect("runtime schema");

    let now = now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, updated_at)
             VALUES (?1, ?2, NULL, 'queued', 0, 0, 'broken-job', '{', '{}', ?3, ?3, ?3)",
            params![ENRICHMENT_JOB_TYPE, TITLE_NORMALIZATION_PLUGIN_ID, now],
        )
        .expect("insert malformed job");

    let error = run_insights(&paths, &config, None, None, &RunInsightsRequest::default())
        .expect_err("invalid payload should fail the run");
    assert!(error.to_string().contains("parsing enrichment payload"));

    let (status, warning) = connection
        .query_row(
            "SELECT status, warning
             FROM insight_runs
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row: &Row<'_>| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .expect("load failed run");
    assert_eq!(status, "failed");
    assert!(warning.expect("warning").contains("Insight refresh stopped before completion"));
}

#[test]
fn run_insights_recovers_interrupted_runs_and_requeues_stuck_jobs() {
    let paths = test_paths();
    let mut config = test_config();
    if let Some(plugin) =
        config.enrichment.plugins.iter_mut().find(|plugin| plugin.id == READABLE_CONTENT_PLUGIN_ID)
    {
        plugin.enabled = false;
    }
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    seed_visits(&archive);
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    ensure_insight_schema(&connection).expect("insight schema");
    ensure_intelligence_runtime_schema(&connection).expect("runtime schema");

    let now = now_rfc3339();
    connection
        .execute(
            "INSERT INTO insight_runs (id, started_at, status, mode, profile_scope, window_days, notes_json)
             VALUES (41, ?1, 'running', 'manual', 'all', 30, '[]')",
            [now.clone()],
        )
        .expect("insert interrupted run");
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, ?2, 41, 'running', 10, 1, 'title-normalization:1', ?3, '{}', ?4, ?4, ?4, ?4)",
            params![
                ENRICHMENT_JOB_TYPE,
                TITLE_NORMALIZATION_PLUGIN_ID,
                serde_json::to_string(&EnrichmentJobPayload {
                    history_id: 1,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/docs/archive".to_string(),
                    title: Some("Archive docs".to_string()),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert interrupted job");

    let report = run_insights(&paths, &config, None, None, &RunInsightsRequest::default())
        .expect("run insights after recovery");
    assert!(report.notes.iter().any(|note| note.contains("Recovered 1 interrupted insight run")));

    let previous_status = connection
        .query_row("SELECT status FROM insight_runs WHERE id = 41", [], |row: &Row<'_>| {
            row.get::<_, String>(0)
        })
        .expect("previous run status");
    assert_eq!(previous_status, "failed");

    let job_state = connection
        .query_row(
            "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'title-normalization:1'",
            [],
            |row: &Row<'_>| row.get::<_, String>(0),
        )
        .expect("job state");
    assert_eq!(job_state, "succeeded");
}

#[test]
fn query_stage_heuristics_cover_compare_and_site_restrict() {
    assert_eq!(classify_query_stage(Some("best archive tool vs obsidian"), None), "compare");
    assert_eq!(classify_query_stage(Some("site:github.com archive tool"), None), "site-restrict");
}
