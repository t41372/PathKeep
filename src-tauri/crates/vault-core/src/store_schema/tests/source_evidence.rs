//! Responsibilities: pin the source_evidence catalog and complete row fixtures.
//! Not responsible for: producing migrations or exercising production query code.
//! Failure modes: missing columns/indexes, lossy rows and weakened foreign keys.
use super::{Fixture, ForeignKey};

pub(super) const TABLES: &[&str] = &[
    "__pk_migrations",
    "native_entities",
    "schema_observations",
    "source_batches",
    "sqlite_sequence",
    "visit_context_evidence",
    "visit_engagement_evidence",
    "visit_navigation_evidence",
    "visit_search_evidence",
    "__turso_internal_seq___turso_internal_autoincrement_native_entities",
    "__turso_internal_seq___turso_internal_autoincrement_source_batches",
    "__turso_internal_seq___turso_internal_autoincrement_visit_context_evidence",
    "__turso_internal_seq___turso_internal_autoincrement_visit_engagement_evidence",
    "__turso_internal_seq___turso_internal_autoincrement_visit_navigation_evidence",
    "__turso_internal_seq___turso_internal_autoincrement_visit_search_evidence",
];
pub(super) const INDEXES: &[&str] = &[
    "idx_native_entities_batch_kind",
    "idx_source_batches_profile_created",
    "idx_source_batches_run",
    "idx_visit_context_evidence_batch",
    "idx_visit_engagement_evidence_batch",
    "idx_visit_navigation_evidence_batch",
    "idx_visit_search_evidence_batch",
];
pub(super) const ROWS: &[Fixture] = &[
    Fixture {
        table: "source_batches",
        columns: "id, source_profile_id, run_id, source_kind, browser_version, schema_version_text, schema_version_int, schema_fingerprint, parser_version, capability_snapshot_json, coverage_stats_json, artifact_refs_json, notes_json, created_at",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
        updated: "source_profile_id = 2",
    },
    Fixture {
        table: "schema_observations",
        columns: "source_batch_id, database_label, observation_json, recorded_at",
        values: "1, 'sample', 'sample', 'sample'",
        updated: "source_batch_id = 2",
    },
    Fixture {
        table: "visit_search_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, source_url_id, evidence_key, evidence_value, normalized_value, source_field, recorded_at",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
        updated: "source_batch_id = 2",
    },
    Fixture {
        table: "visit_navigation_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, edge_kind, target_visit_id, target_url, transition, source_field, recorded_at",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample'",
        updated: "source_batch_id = 2",
    },
    Fixture {
        table: "visit_engagement_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, metric_key, metric_value_int, metric_value_real, source_field, recorded_at",
        values: "1, 1, 1, 'sample', 'sample', 1, 1.5, 'sample', 'sample'",
        updated: "source_batch_id = 2",
    },
    Fixture {
        table: "visit_context_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, source_url_id, context_key, value_json, source_field, recorded_at",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
        updated: "source_batch_id = 2",
    },
    Fixture {
        table: "native_entities",
        columns: "id, source_batch_id, source_profile_id, entity_kind, native_primary_key, parent_native_primary_key, payload_json, metadata_json, recorded_at",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
        updated: "source_batch_id = 2",
    },
];
pub(super) const FOREIGN_KEYS: &[ForeignKey] = &[];
