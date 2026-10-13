//! Responsibilities: complete source_evidence row fixtures for maintenance round trips.
//! Not responsible for: producing migrations or exercising production query code.
//! Failure modes: a table or column the round trip should carry has no fixture row.
use super::Fixture;

pub(super) const ROWS: &[Fixture] = &[
    Fixture {
        table: "source_batches",
        columns: "id, source_profile_id, run_id, source_kind, browser_version, schema_version_text, schema_version_int, schema_fingerprint, parser_version, capability_snapshot_json, coverage_stats_json, artifact_refs_json, notes_json, created_at",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "schema_observations",
        columns: "source_batch_id, database_label, observation_json, recorded_at",
        values: "1, 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "visit_search_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, source_url_id, evidence_key, evidence_value, normalized_value, source_field, recorded_at, content_key",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', X'0102'",
    },
    Fixture {
        table: "visit_navigation_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, edge_kind, target_visit_id, target_url, transition, source_field, recorded_at, content_key",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', X'0102'",
    },
    Fixture {
        table: "visit_engagement_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, metric_key, metric_value_int, metric_value_real, source_field, recorded_at, content_key",
        values: "1, 1, 1, 'sample', 'sample', 1, 1.5, 'sample', 'sample', X'0102'",
    },
    Fixture {
        table: "visit_context_evidence",
        columns: "id, source_batch_id, source_profile_id, source_visit_id, source_url_id, context_key, value_json, source_field, recorded_at, content_key",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', X'0102'",
    },
    Fixture {
        table: "native_entities",
        columns: "id, source_batch_id, source_profile_id, entity_kind, native_primary_key, parent_native_primary_key, payload_json, metadata_json, recorded_at, content_key",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', X'0102'",
    },
];
