//! Responsibilities: complete intelligence row fixtures for maintenance round trips.
//! Not responsible for: producing migrations or exercising production query code.
//! Failure modes: a table or column the round trip should carry has no fixture row.
use super::Fixture;

pub(super) const ROWS: &[Fixture] = &[
    Fixture {
        table: "session_domains",
        columns: "generation,profile_id,session_id,domain,visit_count",
        values: "1,'sample','sample','sample',1",
    },
    Fixture {
        table: "session_paths",
        columns: "generation,profile_id,session_id,segment,date_key,prefix_len,sites_json",
        values: "1,'sample','sample',0,'2026-01-01',0,'[]'",
    },
    Fixture {
        table: "site_flows_rows",
        columns: "generation,profile_id,date_key,registrable_domain,kind,other,visit_count",
        values: "1,'sample','2026-01-01','sample','link','sample',1",
    },
    Fixture {
        table: "intelligence_generations",
        columns: "profile_id,facts,daily,navigation,aggregates",
        values: "'sample',1,1,1,1",
    },
    Fixture {
        table: "intelligence_builds",
        columns: "id,profile_id,state_json,updated_at",
        values: "1,'sample','{}','sample'",
    },
    Fixture {
        table: "intelligence_counts",
        columns: "generation,profile_id,table_name,value",
        values: "1,'sample','sessions',1",
    },
    Fixture {
        table: "intelligence_day_counts",
        columns: "generation,profile_id,date_key,domains,categories,engines,summaries",
        values: "1,'sample','sample',1,1,1,1",
    },
    Fixture {
        table: "intelligence_seen_domains",
        columns: "generation,profile_id,domain",
        values: "1,'sample','sample'",
    },
    Fixture {
        table: "visit_assignments",
        columns: "generation,visit_id,profile_id,session_id,trail_id, ordinal",
        values: "1,1,'sample','sample','sample', 1",
    },
    Fixture {
        table: "ai_embedding_totals",
        columns: "provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at,lineage",
        values: "'sample','sample',1,1,1,'sample','legacy'",
    },
    Fixture {
        table: "visit_derived_facts_rows",
        columns: "generation, visit_id, profile_id, registrable_domain, canonical_url, domain_category, page_category, search_engine, search_query, is_new_domain, is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version, computed_at",
        values: "1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "domain_daily_rollups_rows",
        columns: "generation, date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls",
        values: "1, 'sample', 'sample', 'sample', 'sample', 1, 1, 1, 1",
    },
    Fixture {
        table: "category_daily_rollups_rows",
        columns: "generation, date_key, profile_id, domain_category, visit_count, unique_domains",
        values: "1, 'sample', 'sample', 'sample', 1, 1",
    },
    Fixture {
        table: "engine_daily_rollups_rows",
        columns: "generation, date_key, profile_id, search_engine, search_count",
        values: "1, 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "daily_summary_rollups_rows",
        columns: "generation, date_key, profile_id, total_visits, total_searches, new_domains, unique_domains, hhi_score, discovery_rate",
        values: "1, 'sample', 'sample', 1, 1, 1, 1, 1.5, 1.5",
    },
    Fixture {
        table: "sessions_rows",
        columns: "generation, session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at",
        values: "1, 'sample', 'sample', 1, 1, 1, 1, 1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "search_trails_rows",
        columns: "generation, trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count, visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json, computed_at",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 'sample', 'sample', 1, 1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "search_trail_members_rows",
        columns: "generation, trail_id, profile_id, visit_id, ordinal, role",
        values: "1, 'sample', 'sample', 1, 1, 'sample'",
    },
    Fixture {
        table: "search_events_rows",
        columns: "generation, visit_id, profile_id, search_engine, raw_query, normalized_query, query_kind, trail_id, computed_at, visit_time_ms",
        values: "1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "search_event_terms_rows",
        columns: "generation, visit_id, profile_id, term",
        values: "1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "query_family_queries_rows",
        columns: "generation, family_id, profile_id, normalized_query",
        values: "1, 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "query_families_rows",
        columns: "generation, family_id, profile_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json, computed_at",
        values: "1, 'sample', 'sample', 'sample', 1, 'sample', 1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "refind_pages_rows",
        columns: "generation, profile_id, canonical_url, url, title, registrable_domain, cross_day_count, trail_count, search_arrival_count, typed_revisit_count, refind_score, evidence_json, first_seen_ms, last_seen_ms, computed_at, page_key",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 1, 1, 1.5, 'sample', 1, 1, 'sample', 1",
    },
    Fixture {
        table: "refind_day_pages_rows",
        columns: "generation, profile_id, date_key, page_keys",
        values: "1, 'sample', '2026-10-11', X'0100000000000000'",
    },
    Fixture {
        table: "reopened_investigations_rows",
        columns: "generation, investigation_id, profile_id, anchor_type, anchor_id, anchor_label, occurrence_count, distinct_days, first_seen_ms, last_seen_ms, evidence_json, computed_at",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "path_flows_rows",
        columns: "generation, profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms",
        values: "1, 'sample', 'sample', 1, 1, 1",
    },
    Fixture {
        table: "compare_sets_rows",
        columns: "generation, compare_set_id, profile_id, trail_id, search_query, page_category, first_visit_ms, last_visit_ms, pages_json, computed_at",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "page_content_enrichments",
        columns: "id, canonical_url, content_source, fetch_status, fetched_at, final_url, language, readable_title, readable_text_blob_path, readable_text_bytes, text_hash, snippet_json, extraction_json, pipeline_version, extractor_version, enrichment_summary, refetch_after, http_status",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 1",
    },
    Fixture {
        table: "site_dictionary_overrides",
        columns: "id, target_kind, target_value, domain_category, page_category, interaction_kind, display_name, search_engine, is_noisy, note, created_at, updated_at",
        values: "1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "search_engine_rules",
        columns: "rule_id, engine_id, display_name, host_pattern, path_prefix, query_param_key, enabled, note, example_url, created_at, updated_at",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "core_intelligence_stage_checkpoints",
        columns: "profile_id, stage, stage_version, visible_visit_count, max_visit_id, max_url_last_visit_ms, visible_search_term_count, last_processed_visit_id, dirty_from_visit_ms, dirty_date_key, last_run_id, fallback_reason, updated_at",
        values: "'sample', 'sample', 'sample', 1, 1, 1, 1, 1, 1, 'sample', 1, 'sample', 'sample'",
    },
    Fixture {
        table: "intelligence_overview_snapshots",
        columns: "scope_key, band, fingerprint, payload_json, computed_at",
        values: "'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "intelligence_jobs",
        columns: "id, job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json, artifact_json, created_at, scheduled_at, started_at, finished_at, heartbeat_at, lease_owner, lease_expires_at, updated_at, last_error, cancellation_reason, stop_requested, canonical_url, trigger_cursor, list_order",
        values: "1, 'sample', 'sample', 1, 'sample', 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 1, 1",
    },
    Fixture {
        table: "intelligence_job_triggers",
        columns: "id, job_id, run_id, reason, requested_at",
        values: "1, 1, 1, 'sample', 'sample'",
    },
    Fixture {
        table: "deterministic_module_runtime",
        columns: "module_id, version, status, depends_on_json, derived_tables_json, last_run_id, last_built_at, last_invalidated_at, stale_reason, stale_reason_code, notes_json, updated_at, invalidation_revision",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1",
    },
    Fixture {
        table: "ai_embeddings",
        columns: "id, history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at, metadata_bytes, lineage",
        values: "1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample', 'sample', 1, 'legacy'",
    },
    Fixture {
        table: "ai_assistant_runs",
        columns: "id, run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at",
        values: "1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "ai_index_ledger",
        columns: "provider_id, model, sidecar_table, index_version, state, source_watermark, last_run_id, build_started_at, build_finished_at, last_indexed_at, last_cleared_at, last_failure_at, failure_reason",
        values: "'sample', 'sample', 'sample', 'sample', 'sample', 1, 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample'",
    },
    Fixture {
        table: "ai_jobs",
        columns: "id, job_type, state, priority, attempt, max_attempts, payload_json, run_id, summary, available_at, created_at, updated_at, started_at, finished_at, heartbeat_at, lease_owner, lease_expires_at, stop_requested, error_code, error_message",
        values: "1, 'sample', 'sample', 1, 1, 1, 'sample', 1, 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 'sample', 1, 'sample', 'sample'",
    },
];
