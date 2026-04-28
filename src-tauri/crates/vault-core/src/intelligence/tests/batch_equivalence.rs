//! Batch equivalence regressions for Core Intelligence rebuild helpers.
//!
//! ## Responsibilities
//! - Compare batched builders against their in-memory reference builders.
//! - Verify fallback-full rebuilds match clean full rebuilds across batch
//!   boundaries.
//! - Keep query-family, structural aggregate, and source-effectiveness behavior
//!   stable while the implementation stays streaming-friendly.
//!
//! ## Not responsible for
//! - Route-facing overview or explanation DTO coverage.
//! - Remote backup or AI/enrichment behavior.
//!
//! ## Dependencies
//! - `fixtures` seeds paired fallback and clean archives.
//! - Structural helper modules expose test-only builders through `pub(super)`.
//!
//! ## Performance notes
//! These tests intentionally cross configured fallback batch sizes. They compare
//! persisted rows instead of holding large expected structures in memory.

use super::super::{
    DAILY_ROLLUP_FALLBACK_BATCH_SIZE, ProfileSourceWatermark, QueryFamilyRecord, RefindPageRecord,
    TrailRecord, VISIT_DERIVE_FALLBACK_BATCH_SIZE, VisitRecord,
    intelligence_daily_rollups::load_profile_derived_visits,
    intelligence_rebuild::{run_core_intelligence, run_core_intelligence_job_type_with_progress},
    intelligence_structural_aggregates::{
        build_habit_patterns, build_path_flows, build_refind_pages, build_reopened_investigations,
        build_source_effectiveness, build_structural_profile_aggregates_from_batches,
    },
    intelligence_structural_build::{
        build_query_families, build_query_families_from_batches, load_profile_search_events,
    },
    intelligence_structural_stage::{
        build_source_effectiveness_from_database, execute_structural_stage,
        expand_structural_rebuild_start, load_profile_dirty_date_keys,
        load_profile_first_visible_visit_ms, load_profile_trails, load_structural_delta_summary,
    },
    site_dictionary::normalize_query,
};
use super::fixtures::{
    append_fixture_visit, append_many_fixture_visits, load_daily_rollup_rows,
    load_visit_derived_fact_rows, seed_core_intelligence_fixture,
};
use crate::{
    archive::{open_archive_connection, open_intelligence_connection},
    config::project_paths_with_root,
    models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
};

const DAY_MS: i64 = 86_400_000;

fn aggregate_visit(visit_id: i64, domain: &str, day: i64, session_id: Option<&str>) -> VisitRecord {
    let visit_time_ms = 1711929600000 + (day * DAY_MS) + visit_id;
    VisitRecord {
        visit_id,
        profile_id: "chrome:Default".to_string(),
        source_profile_id: 1,
        source_visit_id: visit_id,
        source_url_id: visit_id + 10_000,
        url: format!("https://{domain}/page-{visit_id}"),
        title: Some(format!("{domain} page {visit_id}")),
        visit_time_ms,
        from_visit: Some(visit_id - 1),
        transition_type: Some(1),
        external_referrer_url: None,
        canonical_url: format!("https://{domain}/page-{visit_id}"),
        registrable_domain: domain.to_string(),
        domain_category: "reference".to_string(),
        page_category: "article".to_string(),
        search_engine: None,
        search_query: None,
        is_new_domain: false,
        is_search_event: false,
        evidence_tier: "deterministic".to_string(),
        taxonomy_source: "rules".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        display_name: Some(domain.to_string()),
        session_id: session_id.map(str::to_string),
        trail_id: None,
    }
}

/// Regression coverage for batched query family builder matches in memory builder.
#[test]
fn batched_query_family_builder_matches_in_memory_builder() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    for visit_id in 4..=24 {
        let timestamp = 1712145600000 + ((visit_id - 4) * 60_000);
        let query = if visit_id % 2 == 0 {
            format!("sqlite wal checkpoint {}", visit_id % 3)
        } else {
            format!("tauri ipc bridge {}", visit_id % 4)
        };
        append_fixture_visit(
            &archive,
            visit_id,
            &format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
            &format!("Search {query}"),
            timestamp,
            None,
            Some(&query),
        );
    }
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let all_events =
        load_profile_search_events(&intelligence, "chrome:Default").expect("search events");
    let in_memory = build_query_families(&all_events);
    let batched = build_query_families_from_batches(&intelligence, "chrome:Default")
        .expect("batched query families");

    let to_summary = |families: Vec<QueryFamilyRecord>| {
        let mut summary = families
            .into_iter()
            .map(|family| {
                (
                    normalize_query(&family.anchor_query),
                    family.member_count,
                    family.search_engine,
                    family.queries.len(),
                )
            })
            .collect::<Vec<_>>();
        summary.sort();
        summary
    };
    assert_eq!(to_summary(batched), to_summary(in_memory));
}

/// Regression coverage for batched structural aggregates match in memory builders.
#[test]
fn batched_structural_aggregates_match_in_memory_builders() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    append_fixture_visit(
        &archive,
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712102400000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        5,
        "https://alpha-docs.dev/guide",
        "Guide",
        1712145600000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        6,
        "https://beta-community.dev/thread",
        "Thread",
        1712145660000,
        Some(5),
        None,
    );
    append_fixture_visit(
        &archive,
        7,
        "https://gamma-news.dev/article",
        "Article",
        1712145720000,
        Some(6),
        None,
    );
    append_fixture_visit(
        &archive,
        8,
        "https://delta-shop.dev/item",
        "Item",
        1712145780000,
        Some(7),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let visits = load_profile_derived_visits(&intelligence, "chrome:Default", None, None)
        .expect("derived visits");
    let mut in_memory_refind = build_refind_pages(&visits)
        .into_iter()
        .map(|page| (page.canonical_url, page.refind_score, page.cross_day_count))
        .collect::<Vec<_>>();
    let mut in_memory_flows = build_path_flows(&visits)
        .into_iter()
        .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
        .collect::<Vec<_>>();
    let mut in_memory_habits = build_habit_patterns(&visits)
        .into_iter()
        .map(|habit| (habit.registrable_domain, habit.habit_type, habit.visit_count))
        .collect::<Vec<_>>();

    let (batched_refind, batched_flows, batched_habits) =
        build_structural_profile_aggregates_from_batches(&intelligence, "chrome:Default")
            .expect("batched aggregates");
    let mut batched_refind = batched_refind
        .into_iter()
        .map(|page| (page.canonical_url, page.refind_score, page.cross_day_count))
        .collect::<Vec<_>>();
    let mut batched_flows = batched_flows
        .into_iter()
        .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
        .collect::<Vec<_>>();
    let mut batched_habits = batched_habits
        .into_iter()
        .map(|habit| (habit.registrable_domain, habit.habit_type, habit.visit_count))
        .collect::<Vec<_>>();

    in_memory_refind.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    in_memory_flows.sort();
    in_memory_habits.sort();
    batched_refind.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    batched_flows.sort();
    batched_habits.sort();

    assert_eq!(batched_refind, in_memory_refind);
    assert_eq!(batched_flows, in_memory_flows);
    assert_eq!(batched_habits, in_memory_habits);
}

/// Regression coverage for batched source effectiveness matches in memory builder.
#[test]
fn batched_source_effectiveness_matches_in_memory_builder() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    append_fixture_visit(
        &archive,
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712102400000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        5,
        "https://alpha-docs.dev/guide",
        "Guide",
        1712145600000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        6,
        "https://beta-community.dev/thread",
        "Thread",
        1712145660000,
        Some(5),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let trails = load_profile_trails(&intelligence, "chrome:Default").expect("trails");
    let (refind_pages, _, _) =
        build_structural_profile_aggregates_from_batches(&intelligence, "chrome:Default")
            .expect("aggregates");
    let mut in_memory = build_source_effectiveness(&trails, &refind_pages)
        .into_iter()
        .map(|record| {
            (
                record.registrable_domain,
                record.source_role,
                record.trail_count,
                record.stable_landing_count,
                record.effectiveness_score,
                record.first_seen_ms,
                record.last_seen_ms,
            )
        })
        .collect::<Vec<_>>();
    let mut batched =
        build_source_effectiveness_from_database(&intelligence, "chrome:Default", &refind_pages)
            .expect("batched source effectiveness")
            .into_iter()
            .map(|record| {
                (
                    record.registrable_domain,
                    record.source_role,
                    record.trail_count,
                    record.stable_landing_count,
                    record.effectiveness_score,
                    record.first_seen_ms,
                    record.last_seen_ms,
                )
            })
            .collect::<Vec<_>>();
    in_memory.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.total_cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
            .then_with(|| left.6.cmp(&right.6))
    });
    batched.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.total_cmp(&right.4))
            .then_with(|| left.5.cmp(&right.5))
            .then_with(|| left.6.cmp(&right.6))
    });
    assert_eq!(batched, in_memory);
}

/// Direct aggregate-builder coverage for role selection, session flushes, and habit classes.
#[test]
fn structural_aggregate_builders_cover_roles_flows_reopened_and_habits() {
    let trails = vec![
        TrailRecord {
            trail_id: "trail-landing-1".to_string(),
            profile_id: "chrome:Default".to_string(),
            session_id: "session-a".to_string(),
            initial_query: "landing docs".to_string(),
            search_engine: "Google".to_string(),
            reformulation_count: 0,
            visit_count: 3,
            landing_url: Some("https://landing.example/guide".to_string()),
            landing_domain: Some("landing.example".to_string()),
            first_visit_ms: 1711929600000,
            last_visit_ms: 1711929660000,
            max_depth: 2,
            queries: vec!["landing docs".to_string()],
            members: Vec::new(),
        },
        TrailRecord {
            trail_id: "trail-landing-2".to_string(),
            profile_id: "chrome:Default".to_string(),
            session_id: "session-b".to_string(),
            initial_query: "landing reference".to_string(),
            search_engine: "Google".to_string(),
            reformulation_count: 1,
            visit_count: 2,
            landing_url: Some("https://landing.example/reference".to_string()),
            landing_domain: Some("landing.example".to_string()),
            first_visit_ms: 1712016000000,
            last_visit_ms: 1712016060000,
            max_depth: 1,
            queries: vec!["landing reference".to_string()],
            members: Vec::new(),
        },
        TrailRecord {
            trail_id: "trail-reference".to_string(),
            profile_id: "chrome:Default".to_string(),
            session_id: "session-c".to_string(),
            initial_query: "reference docs".to_string(),
            search_engine: "Google".to_string(),
            reformulation_count: 0,
            visit_count: 1,
            landing_url: Some("https://reference.example/page".to_string()),
            landing_domain: Some("reference.example".to_string()),
            first_visit_ms: 1712102400000,
            last_visit_ms: 1712102460000,
            max_depth: 1,
            queries: vec!["reference docs".to_string()],
            members: Vec::new(),
        },
    ];
    let refind_pages = vec![
        RefindPageRecord {
            profile_id: "chrome:Default".to_string(),
            canonical_url: "https://reference.example/page".to_string(),
            url: "https://reference.example/page".to_string(),
            title: Some("Reference Page".to_string()),
            registrable_domain: "reference.example".to_string(),
            cross_day_count: 3,
            trail_count: 1,
            search_arrival_count: 1,
            typed_revisit_count: 2,
            refind_score: 10.0,
            evidence_json: "{}".to_string(),
            first_seen_ms: 1711929600000,
            last_seen_ms: 1712102400000,
        },
        RefindPageRecord {
            profile_id: "chrome:Default".to_string(),
            canonical_url: "https://landing.example/guide".to_string(),
            url: "https://landing.example/guide".to_string(),
            title: None,
            registrable_domain: "landing.example".to_string(),
            cross_day_count: 1,
            trail_count: 1,
            search_arrival_count: 1,
            typed_revisit_count: 0,
            refind_score: 3.0,
            evidence_json: "{}".to_string(),
            first_seen_ms: 1711929600000,
            last_seen_ms: 1711929600000,
        },
    ];

    let source_roles = build_source_effectiveness(&trails, &refind_pages)
        .into_iter()
        .map(|record| (record.registrable_domain, record.source_role))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(source_roles.get("landing.example").map(String::as_str), Some("landing"));
    assert_eq!(source_roles.get("reference.example").map(String::as_str), Some("reference"));

    let investigations = build_reopened_investigations(
        &[QueryFamilyRecord {
            family_id: "family-repeated".to_string(),
            profile_id: "chrome:Default".to_string(),
            anchor_query: "rust coverage".to_string(),
            member_count: 2,
            search_engine: "Google".to_string(),
            first_seen_ms: 1711929600000,
            last_seen_ms: 1712016000000,
            queries: vec!["rust coverage".to_string(), "rust coverage tips".to_string()],
        }],
        &refind_pages,
    );
    assert!(investigations.iter().any(|item| item.anchor_type == "query_family"));
    assert!(investigations.iter().any(|item| {
        item.anchor_type == "reference_page" && item.anchor_label == "Reference Page"
    }));

    let mut flow_visits = vec![
        aggregate_visit(1, "alpha.example", 0, Some("session-flow")),
        aggregate_visit(2, "alpha.example", 0, Some("session-flow")),
        aggregate_visit(3, "beta.example", 0, Some("session-flow")),
        aggregate_visit(4, "gamma.example", 0, Some("session-flow")),
        aggregate_visit(5, "delta.example", 0, Some("session-flow")),
        aggregate_visit(6, "omega.example", 1, Some("session-next")),
        aggregate_visit(7, "sigma.example", 1, Some("session-next")),
    ];
    let flow_patterns = build_path_flows(&flow_visits)
        .into_iter()
        .map(|flow| (flow.flow_pattern, flow.step_count, flow.occurrence_count))
        .collect::<Vec<_>>();
    assert!(flow_patterns.iter().any(|(pattern, steps, _)| {
        pattern.contains("alpha.example")
            && pattern.contains("delta.example")
            && !pattern.contains("omega.example")
            && *steps == 4
    }));
    assert!(flow_patterns.iter().any(|(pattern, steps, _)| {
        pattern.contains("omega.example") && pattern.contains("sigma.example") && *steps == 2
    }));

    for day in 0..15 {
        flow_visits.push(aggregate_visit(100 + day, "daily.example", day, Some("daily")));
    }
    for (index, day) in [0, 7, 14, 21, 28].into_iter().enumerate() {
        flow_visits.push(aggregate_visit(
            200 + index as i64,
            "weekly.example",
            day,
            Some("weekly"),
        ));
    }
    for (index, day) in [0, 14, 28, 42, 56].into_iter().enumerate() {
        flow_visits.push(aggregate_visit(
            300 + index as i64,
            "periodic.example",
            day,
            Some("periodic"),
        ));
    }
    for (index, day) in [0, 1, 20, 21, 50].into_iter().enumerate() {
        flow_visits.push(aggregate_visit(400 + index as i64, "noisy.example", day, Some("noisy")));
    }
    let habits = build_habit_patterns(&flow_visits)
        .into_iter()
        .map(|habit| (habit.registrable_domain, (habit.habit_type, habit.visit_count)))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        habits.get("daily.example").map(|(habit, count)| (habit.as_str(), *count)),
        Some(("daily_habit", 15))
    );
    assert_eq!(
        habits.get("weekly.example").map(|(habit, count)| (habit.as_str(), *count)),
        Some(("weekly_habit", 5))
    );
    assert_eq!(
        habits.get("periodic.example").map(|(habit, count)| (habit.as_str(), *count)),
        Some(("periodic_reference", 5))
    );
    assert!(!habits.contains_key("noisy.example"));
}

/// Regression coverage for structural stage helper boundaries.
#[test]
fn structural_stage_helpers_cover_noop_delta_and_dirty_windows() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    append_fixture_visit(
        &archive,
        4,
        "https://docs.example.com/sqlite/wal-checkpoint",
        "SQLite WAL Checkpoint",
        1712102400000,
        None,
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");
    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");

    let dirty_keys = load_profile_dirty_date_keys(&intelligence, "chrome:Default", None, None)
        .expect("all dirty dates");
    assert!(!dirty_keys.is_empty());
    let filtered_keys =
        load_profile_dirty_date_keys(&intelligence, "chrome:Default", Some(1712102400000), Some(2))
            .expect("filtered dirty dates");
    assert!(!filtered_keys.is_empty());

    let delta = load_structural_delta_summary(&intelligence, "chrome:Default", 2)
        .expect("structural delta");
    assert!(delta.delta_count >= 1);
    assert!(delta.dirty_from_visit_ms.is_some());
    assert!(!delta.dirty_date_keys.is_empty());

    let first_visible = load_profile_first_visible_visit_ms(&intelligence, "chrome:Default")
        .expect("first visible visit")
        .expect("first visible visit exists");
    let expanded =
        expand_structural_rebuild_start(&intelligence, "chrome:Default", first_visible + 60_000)
            .expect("expanded structural start");
    assert!(expanded <= first_visible + 60_000);

    let empty_report = execute_structural_stage(
        &intelligence,
        "empty-profile",
        &ProfileSourceWatermark::default(),
        false,
        99,
        "2026-04-14T00:00:00Z",
    )
    .expect("empty structural stage");
    assert_eq!(empty_report.execution_mode.as_deref(), Some("noop"));
    assert_eq!(empty_report.affected_profiles, vec!["empty-profile"]);
    assert!(empty_report.notes[0].contains("No visible visits remained"));
}

/// Regression coverage for visit derive fallback matches clean full rebuild across batches.
#[test]
fn visit_derive_fallback_matches_clean_full_rebuild_across_batches() {
    let fallback_root = tempfile::tempdir().expect("fallback tempdir");
    let clean_root = tempfile::tempdir().expect("clean tempdir");
    let fallback_paths = project_paths_with_root(fallback_root.path());
    let clean_paths = project_paths_with_root(clean_root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let reverted_visit_id = 1200_i64;

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    seed_core_intelligence_fixture(&fallback_archive);
    append_many_fixture_visits(
        &fallback_archive,
        4,
        VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    drop(fallback_archive);

    run_core_intelligence(
        &fallback_paths,
        &config,
        None,
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("fallback full rebuild");

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    fallback_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert fallback visit");
    drop(fallback_archive);

    let fallback_report = run_core_intelligence_job_type_with_progress(
        &fallback_paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive fallback");
    assert_eq!(fallback_report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(fallback_report.visit_derived_facts > VISIT_DERIVE_FALLBACK_BATCH_SIZE);

    let clean_archive =
        open_archive_connection(&clean_paths, &config, None).expect("clean archive");
    seed_core_intelligence_fixture(&clean_archive);
    append_many_fixture_visits(
        &clean_archive,
        4,
        VISIT_DERIVE_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    clean_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert clean visit");
    drop(clean_archive);

    run_core_intelligence(&clean_paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("clean full rebuild");

    let fallback_intelligence = open_intelligence_connection(&fallback_paths, &config, None)
        .expect("fallback intelligence");
    let clean_intelligence =
        open_intelligence_connection(&clean_paths, &config, None).expect("clean intelligence");
    assert_eq!(
        load_visit_derived_fact_rows(&fallback_intelligence),
        load_visit_derived_fact_rows(&clean_intelligence)
    );
}

/// Regression coverage for daily rollup fallback matches clean full rebuild across batches.
#[test]
fn daily_rollup_fallback_matches_clean_full_rebuild_across_batches() {
    let fallback_root = tempfile::tempdir().expect("fallback tempdir");
    let clean_root = tempfile::tempdir().expect("clean tempdir");
    let fallback_paths = project_paths_with_root(fallback_root.path());
    let clean_paths = project_paths_with_root(clean_root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let reverted_visit_id = 1200_i64;

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    seed_core_intelligence_fixture(&fallback_archive);
    append_many_fixture_visits(
        &fallback_archive,
        4,
        DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    drop(fallback_archive);

    run_core_intelligence(
        &fallback_paths,
        &config,
        None,
        &CoreIntelligenceRebuildRequest::default(),
    )
    .expect("fallback full rebuild");

    let fallback_archive =
        open_archive_connection(&fallback_paths, &config, None).expect("fallback archive");
    fallback_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert fallback visit");
    drop(fallback_archive);

    run_core_intelligence_job_type_with_progress(
        &fallback_paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive fallback");
    let fallback_report = run_core_intelligence_job_type_with_progress(
        &fallback_paths,
        &config,
        None,
        "daily-rollup",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("daily rollup fallback");
    assert_eq!(fallback_report.execution_mode.as_deref(), Some("fallback-full"));
    assert!(fallback_report.processed_visits > DAILY_ROLLUP_FALLBACK_BATCH_SIZE);

    let clean_archive =
        open_archive_connection(&clean_paths, &config, None).expect("clean archive");
    seed_core_intelligence_fixture(&clean_archive);
    append_many_fixture_visits(
        &clean_archive,
        4,
        DAILY_ROLLUP_FALLBACK_BATCH_SIZE + 17,
        1712145600000,
    );
    clean_archive
        .execute(
            "UPDATE visits SET reverted_at = '2026-04-16T00:00:00Z' WHERE id = ?1",
            [reverted_visit_id],
        )
        .expect("revert clean visit");
    drop(clean_archive);

    run_core_intelligence(&clean_paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("clean full rebuild");

    let fallback_intelligence = open_intelligence_connection(&fallback_paths, &config, None)
        .expect("fallback intelligence");
    let clean_intelligence =
        open_intelligence_connection(&clean_paths, &config, None).expect("clean intelligence");
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "domain_daily_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "domain_daily_rollups")
    );
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "category_daily_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "category_daily_rollups")
    );
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "engine_daily_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "engine_daily_rollups")
    );
    assert_eq!(
        load_daily_rollup_rows(&fallback_intelligence, "daily_summary_rollups"),
        load_daily_rollup_rows(&clean_intelligence, "daily_summary_rollups")
    );
}
