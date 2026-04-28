//! Structural incremental rebuild regressions.
//!
//! ## Responsibilities
//! - Protect incremental session/trail assignment updates.
//! - Verify structural tail streaming across batch boundaries.
//! - Preserve older structural rows before a dirty range.
//! - Cover path-flow explanation routing for four-step flows.
//!
//! ## Not responsible for
//! - In-memory versus batched aggregate equivalence.
//! - Overview or schema migration coverage.
//!
//! ## Dependencies
//! - `fixtures` appends deterministic chains that exercise tail streaming.
//! - Core Intelligence rebuild entrypoints run the same stage contract used by
//!   worker jobs.
//!
//! ## Performance notes
//! Boundary tests use the production batch-size constant to catch regressions in
//! streaming behavior without loading an unbounded visit vector.

use super::super::{
    STRUCTURAL_TAIL_STREAM_BATCH_SIZE, explain_entity, get_path_flows,
    intelligence_rebuild::{run_core_intelligence, run_core_intelligence_job_type_with_progress},
};
use super::fixtures::{
    append_fixture_chain_visits, append_fixture_visit, seed_core_intelligence_fixture,
};
use crate::{
    archive::{open_archive_connection, open_intelligence_connection},
    config::project_paths_with_root,
    models::{
        AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest, DateRange,
        EntityExplanationRequest, PathFlowRequest,
    },
};
use rusqlite::params;

/// Regression coverage for structural stage updates tail assignments incrementally.
#[test]
fn structural_stage_updates_tail_assignments_incrementally() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    append_fixture_visit(
        &archive,
        4,
        "https://github.com/example/repo/pulls/7",
        "Pull Request 7",
        1711929900000,
        Some(2),
        None,
    );
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    let report = run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural stage");
    assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
    assert_eq!(report.dirty_visit_count, Some(1));

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let assignments = intelligence
        .prepare(
            "SELECT visit_id, session_id, trail_id
             FROM visit_derived_facts
             WHERE visit_id IN (1, 2, 4)
             ORDER BY visit_id ASC",
        )
        .expect("prepare assignments")
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .expect("query assignments")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect assignments");
    assert_eq!(assignments[0].1.as_deref(), Some("session:chrome:Default:1"));
    assert_eq!(assignments[1].1.as_deref(), Some("session:chrome:Default:1"));
    assert_eq!(assignments[2].1.as_deref(), Some("session:chrome:Default:1"));
    assert_eq!(assignments[0].2.as_deref(), Some("trail:chrome:Default:1"));
    assert_eq!(assignments[1].2.as_deref(), Some("trail:chrome:Default:1"));
    assert_eq!(assignments[2].2.as_deref(), Some("trail:chrome:Default:1"));
}

/// Regression coverage for structural stream keeps assignments stable across batch boundaries.
#[test]
fn structural_stream_keeps_assignments_stable_across_batch_boundaries() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    append_fixture_chain_visits(
        &archive,
        4,
        STRUCTURAL_TAIL_STREAM_BATCH_SIZE + 5,
        1711929720000,
        2,
    );
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    let report = run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural stage");
    assert_eq!(report.execution_mode.as_deref(), Some("incremental"));
    assert!(report.processed_visits > STRUCTURAL_TAIL_STREAM_BATCH_SIZE);

    let boundary_visit_id = 4 + STRUCTURAL_TAIL_STREAM_BATCH_SIZE as i64 - 1;
    let trailing_visit_id = 4 + STRUCTURAL_TAIL_STREAM_BATCH_SIZE as i64 + 4;
    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let assignments = intelligence
        .prepare(
            "SELECT visit_id, session_id, trail_id
             FROM visit_derived_facts
             WHERE visit_id IN (1, 2, 4, ?1, ?2)
             ORDER BY visit_id ASC",
        )
        .expect("prepare assignments")
        .query_map(params![boundary_visit_id, trailing_visit_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .expect("query assignments")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect assignments");
    assert_eq!(assignments.len(), 5);
    for (_, session_id, trail_id) in assignments {
        assert_eq!(session_id.as_deref(), Some("session:chrome:Default:1"));
        assert_eq!(trail_id.as_deref(), Some("trail:chrome:Default:1"));
    }
}

/// Regression coverage for structural stream reclassifying the seed search event
/// once a landing domain is known.
#[test]
fn structural_stream_reclassifies_incremental_search_event_after_landing_domain() {
    let root = tempfile::tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    seed_core_intelligence_fixture(&archive);
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    append_fixture_visit(
        &archive,
        40,
        "https://www.google.com/search?q=pathkeep+sqlite",
        "pathkeep sqlite - Google Search",
        1711929720000,
        None,
        Some("pathkeep sqlite"),
    );
    append_fixture_visit(
        &archive,
        41,
        "https://github.com/example/pathkeep",
        "PathKeep repository",
        1711929780000,
        Some(40),
        None,
    );
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    let report = run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural stage");
    assert_eq!(report.execution_mode.as_deref(), Some("incremental"));

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let (query_kind, trail_id): (String, Option<String>) = intelligence
        .query_row(
            "SELECT query_kind, trail_id FROM search_events WHERE visit_id = 40",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("search event");
    assert_eq!(query_kind, "keyword");
    assert_eq!(trail_id.as_deref(), Some("trail:chrome:Default:40"));
    let term_count: i64 = intelligence
        .query_row("SELECT COUNT(*) FROM search_event_terms WHERE visit_id = 40", [], |row| {
            row.get(0)
        })
        .expect("term count");
    assert_eq!(term_count, 2);
}

/// Regression coverage for structural range delete preserves unaffected rows before start ms.
#[test]
fn structural_range_delete_preserves_unaffected_rows_before_start_ms() {
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
        40,
        "https://www.google.com/search?q=earlier+investigation",
        "Earlier Search",
        1711843200000,
        None,
        Some("earlier investigation"),
    );
    append_fixture_visit(
        &archive,
        41,
        "https://docs.example.com/older-trail",
        "Older Trail",
        1711843260000,
        Some(40),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let archive = open_archive_connection(&paths, &config, None).expect("archive reopen");
    append_fixture_chain_visits(&archive, 4, 12, 1711929720000, 2);
    drop(archive);

    run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "visit-derive",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("visit derive stage");
    run_core_intelligence_job_type_with_progress(
        &paths,
        &config,
        None,
        "structural-rebuild",
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        |_progress| Ok(()),
    )
    .expect("structural stage");

    let intelligence = open_intelligence_connection(&paths, &config, None).expect("intelligence");
    let preserved_session = intelligence
        .query_row("SELECT session_id FROM visit_derived_facts WHERE visit_id = 40", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .expect("preserved session");
    let preserved_trail = intelligence
        .query_row("SELECT trail_id FROM visit_derived_facts WHERE visit_id = 41", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .expect("preserved trail");
    let trail_rows: i64 = intelligence
        .query_row(
            "SELECT COUNT(*) FROM search_trails WHERE trail_id = 'trail:chrome:Default:40'",
            [],
            |row| row.get(0),
        )
        .expect("trail row count");
    let event_rows: i64 = intelligence
        .query_row("SELECT COUNT(*) FROM search_events WHERE visit_id = 40", [], |row| row.get(0))
        .expect("event row count");
    assert_eq!(preserved_session.as_deref(), Some("session:chrome:Default:40"));
    assert_eq!(preserved_trail.as_deref(), Some("trail:chrome:Default:40"));
    assert_eq!(trail_rows, 1);
    assert_eq!(event_rows, 1);
}

/// Regression coverage for path flows support four step queries and explanations.
#[test]
fn path_flows_support_four_step_queries_and_explanations() {
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
        "https://alpha-docs.dev/guide",
        "Guide",
        1712145600000,
        None,
        None,
    );
    append_fixture_visit(
        &archive,
        5,
        "https://beta-community.dev/thread",
        "Thread",
        1712145660000,
        Some(4),
        None,
    );
    append_fixture_visit(
        &archive,
        6,
        "https://gamma-news.dev/article",
        "Article",
        1712145720000,
        Some(5),
        None,
    );
    append_fixture_visit(
        &archive,
        7,
        "https://delta-shop.dev/item",
        "Item",
        1712145780000,
        Some(6),
        None,
    );
    drop(archive);

    run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
        .expect("full rebuild");

    let flows = get_path_flows(
        &paths,
        &config,
        None,
        &PathFlowRequest {
            date_range: DateRange {
                start: "2024-03-30".to_string(),
                end: "2024-04-10".to_string(),
            },
            profile_id: Some("chrome:Default".to_string()),
            step_count: 4,
            limit: Some(10),
        },
    )
    .expect("path flows");
    let flow = flows.iter().find(|entry| entry.step_count == 4).expect("four-step flow");
    let explanation = explain_entity(
        &paths,
        &config,
        None,
        &EntityExplanationRequest {
            entity_type: "path_flow".to_string(),
            entity_id: format!("chrome:Default::4::{}", flow.flow_pattern),
        },
    )
    .expect("path flow explanation");
    assert!(
        explanation
            .factors
            .iter()
            .any(|factor| factor.label == "step_count" && factor.raw_value == 4.0)
    );
}
