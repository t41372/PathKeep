//! Responsibilities: complete archive row fixtures for maintenance round trips.
//! Not responsible for: producing migrations or exercising production query code.
//! Failure modes: a table or column the round trip should carry has no fixture row.
use super::Fixture;

pub(super) const ROWS: &[Fixture] = &[
    Fixture {
        table: "runs",
        columns: "id, run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, stats_json, warnings_json, error_message, due_only",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "source_profiles",
        columns: "id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, user_name, updated_at, browser_family, browser_product",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "pages",
        columns: "id, url, registrable_domain, canonical_url",
        values: "1, 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "favicon_blobs",
        columns: "blob_hash, image_data, recorded_at",
        values: "'sample', X'0102', 'sample'",
    },
    Fixture {
        table: "og_image_blobs",
        columns: "blob_hash, image_data, mime, byte_size, width, height, recorded_at",
        values: "'sample', X'0102', 'sample', 1, 1, 1, 'sample'",
    },
    Fixture {
        table: "import_batches",
        columns: "id, source_kind, source_path, profile_id, created_at, imported_at, reverted_at, status, summary_json, audit_path, git_commit",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "urls",
        columns: "id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at, page_id",
        values: "1, 'sample', 'sample', 1, 1, 1, 'sample', 1, 'sample', 1, 1, 1, 1, 'sample', 'sample', 1",
    },
    Fixture {
        table: "visits",
        columns: "id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, reverted_at, reverted_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at, import_batch_id",
        values: "1, 1, 'sample', 1, 'sample', 1, 1, 1, 1, 'sample', 1, 1, 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "downloads",
        columns: "id, source_download_id, guid, current_path, target_path, start_time_ms, start_time_iso, total_bytes, received_bytes, state, mime_type, original_mime_type, source_profile_id, created_by_run_id, reverted_at, reverted_by_run_id, payload_hash, recorded_at",
        values: "1, 'sample', 'sample', 'sample', 'sample', 1, 'sample', 1, 1, 1, 'sample', 'sample', 1, 1, 'sample', 1, 'sample', 'sample'",
    },
    Fixture {
        table: "search_terms",
        columns: "id, url_id, term, normalized_term, source_profile_id, created_by_run_id, reverted_at, reverted_by_run_id, profile_id, keyword_id, recorded_at",
        values: "1, 1, 'sample', 'sample', 1, 1, 'sample', 1, 'sample', 1, 'sample'",
    },
    Fixture {
        table: "favicons",
        columns: "id, page_url, icon_url, icon_type, width, height, last_updated_ms, last_updated_iso, image_data, source_profile_id, created_by_run_id, payload_hash, recorded_at, image_blob_hash, page_host, page_registrable_domain",
        values: "1, 'sample', 'sample', 1, 1, 1, 1, 'sample', X'0102', 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "manifests",
        columns: "id, run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path",
        values: "1, 1, 1, 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "snapshots",
        columns: "id, run_id, file_path, file_size, checksum, reason, created_at",
        values: "1, 1, 'sample', 1, 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "profile_watermarks",
        columns: "profile_id, last_visit_id, last_url_last_visit_time, last_download_id, last_favicon_last_updated, last_checkpoint_at, last_schema_hash, updated_at, last_source_batch_id",
        values: "'sample', 1, 1, 1, 1, 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "url_annotations",
        columns: "url, notes, created_at, updated_at, source_profile, canonical_url",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "url_tags",
        columns: "url, tag, created_at, source_profile, canonical_url",
        values: "'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "og_images",
        columns: "id, page_url, page_host, source_og_url, image_blob_hash, fetch_status, http_status, fetched_at, last_shown_at, refetch_after, fetch_attempts, created_by_run_id",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 1, 1",
    },
    Fixture {
        table: "star",
        columns: "entity_kind, entity_key, starred_at, source_profile",
        values: "'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "search_documents",
        columns: "page_id, url, title, search_terms, normalized_url, normalized_title, normalized_search_terms, cjk_grams, enrichment_text, notes_text, tags_text, compact_text, domain, newest_visit_ms, updated_at",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample'",
    },
    Fixture {
        table: "search_document_profiles",
        columns: "page_id, source_profile_id, visible_visit_count, newest_visit_ms",
        values: "1, 1, 1, 1",
    },
    Fixture { table: "search_outbox", columns: "seq, page_id, change", values: "1, 1, 'upsert'" },
];
