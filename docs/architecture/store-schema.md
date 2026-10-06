# Turso store schema

`vault_core::store_schema::migration_registry()` supplies a consolidated v1
migration for Archive, SourceEvidence, Intelligence and Agent, followed by Archive v2 read indexes and v3 source totals, with an empty list
for Scratch. Every statement runs through `vault_store::Session`; each database's
v1 and migration record commit in one transaction. No SQLite migration replay,
views, triggers, TEMP tables, FTS virtual tables, window functions or WITHOUT ROWID
tables are used.

This is the schema contract for the Turso port in [ADR-012](decisions/012-turso-storage-engine.md)
and [the migration design](turso-migration-design.md). Archive mutation writers now use Vault and real Store transactions. Native archive readers use the same Store. Search, intelligence and whole-file maintenance retain their existing SQLite files until their separate ports. The desktop session owns the shared Vault for ported commands.

The baseline is the final effect of archive SQL migrations 001–016 and runtime
DDL in archive schema/search/source evidence, intelligence schema and its runtime
helpers, AI schema/queue, and agent store. The v1 files contain consolidated
definitions; they do not include or execute those legacy migration files.

## Files and lifecycle

| Database        | File                         | Version | Application tables |
| --------------- | ---------------------------- | ------- | ------------------ |
| Archive         | `archive/history-vault.db`   | 3       | 22                 |
| Source evidence | `archive/source-evidence.db` | 1       | 7                  |
| Intelligence    | `derived/intelligence.db`    | 1       | 28                 |
| Agent           | `derived/agent.db`           | 1       | 5                  |
| Scratch         | `derived/scratch.db`         | 0       | None               |

The owner derives a distinct 32-byte HKDF sub-key for each file from the data key,
using vault-crypto purpose labels, and supplies those optional sub-keys to `vault-store`.
No database stores a key, password or key envelope. The owner uses `vault-crypto`
for the external envelope. Store resets scratch only after successful preflight;
operation-specific staging tables are not part of the durable schema.

`__pk_migrations(version INTEGER PRIMARY KEY)` is private to vault-store in the
four durable databases. It records applied versions, prevents replay and refuses
newer versions before migrations or scratch cleanup. `sqlite_sequence(name, seq)`
is engine-owned AUTOINCREMENT accounting in the three databases with AUTOINCREMENT tables (Archive, SourceEvidence and
Intelligence); Agent uses text IDs. Turso 0.8.1 also creates one
`__turso_internal_seq___turso_internal_autoincrement_<table>` table per
AUTOINCREMENT table. All of these engine objects appear in the test catalog.
These tables are not application write surfaces. Non-integer and composite primary keys and
inline UNIQUE constraints also have engine-generated `sqlite_autoindex_*`
indexes; the tests name these explicitly as well as the indexes below.

## Page identity and search

`pages.id` is a non-reused integer identity. `pages.url` is unique by the exact URL
string, across profiles. `registrable_domain` is computed by the existing public
suffix logic and is nullable for unclassifiable URLs. Its partial index serves
domain favorites without scanning all URL observations.

`urls.page_id` is required and references `pages`. Each row still represents a
profile's browser URL row, with its source URL ID, title, visit/typed counts,
first/last observed times, hidden flag, payload hash and run provenance. These
facts must not be arbitrarily taken from one profile to represent every profile.
`urls.url` retains the observed string for import/export; writers must assign it
to the matching page. The redundant derived `urls.registrable_domain` moves to
`pages`. `idx_urls_page_id` finds a page's observations without a corpus scan.

Notes and tags remain keyed by exact URL, so one annotation serves every profile.
Stars preserve `(entity_kind, entity_key)`: normalized URL stars can cover tracking
variants, and domain stars cover many pages. Replacing those keys with one exact
page ID would change existing behavior. Pages and profile observations remain
anchors when visits are reverted; visibility belongs to visits.

`search_documents.page_id` is both its primary key and a cascading page FK. The ten
weighted text fields are `url`, `title`, `search_terms`, `normalized_url`,
`normalized_title`, `normalized_search_terms`, `cjk_grams`, `enrichment_text`,
`notes_text` and `tags_text`. `compact_text` is also retained for gram generation.
`domain` holds the search host filter value produced by the URL-domain helper;
it is distinct from the registrable domain used for domain stars. `newest_visit_ms`
is the maximum visible visit time across profiles, or NULL with no visible visits.
`updated_at` records projection freshness. Text normalization and weighting remain
owned by search code, not SQL.

`search_document_profiles` has primary key `(page_id, source_profile_id)`, a positive
`visible_visit_count` and non-null `newest_visit_ms`. A join table avoids a fixed
profile limit and gives a reversible membership count. For each affected pair,
writers count only non-reverted visits and compute their maximum time using
`idx_urls_page_id` and `idx_visits_visible_url_time`. When the count reaches zero,
the membership row is deleted; the document maximum is recomputed from remaining
memberships. All affected document, membership and outbox writes belong in the
same archive transaction, including import revert/restore and annotation updates.
These summaries do not prove combined date/profile eligibility: search still
checks that both filters match the same canonical visible visit.

`search_outbox(seq INTEGER PRIMARY KEY AUTOINCREMENT, page_id, change)` records
`upsert` or `delete`. AUTOINCREMENT prevents acknowledged sequence numbers from
being reused after pruning, including across reopen. There is deliberately no
page FK: a deletion tombstone must outlive the page. Page IDs also never reuse a
committed identity. The Tantivy consumer records its acknowledged sequence in the
index commit before pruning outbox rows. The archive writers append the outbox; its Tantivy consumer is not part of the archive write-path port. There are no implicit
triggers: the caller must use one Store write transaction.

## Table reference

Primary keys and UNIQUE constraints are retained from the final SQLite/runtime
schema except for the explicitly listed changes. Integer primary keys alias rowid.
Times ending in `_ms` are Unix epoch milliseconds; ISO/audit timestamps remain
text. Optional source fields stay nullable. Cross-database IDs remain logical
references; SQL cannot enforce a foreign key into another database. No new FK is
invented between independently rebuilt intelligence tables.

### Archive — `archive/history-vault.db`

**`runs`** — Unified backup, import, revert, repair and maintenance audit ledger. Writer: `archive/run_support; takeout; archive/maintenance`.

- Primary key: `id`.
- Index `idx_runs_started_at`: `runs(started_at DESC)`.

**`source_profiles`** — Stable browser profile identity and discovery metadata. Writer: `archive/ingest; takeout`.

- Primary key: `id`.
- Unique index `idx_source_profiles_profile_key`: `source_profiles(profile_key)`.

**`pages`** — Exact URL identity shared by every profile; registrable domain is a page fact. Writer: `Archive ingest/import writers (Turso)`.

- Primary key: `id`.
- Index `idx_pages_registrable_domain`: `pages(registrable_domain) WHERE registrable_domain IS NOT NULL`.
- Unique: `url` (automatic index).
- Index `idx_pages_canonical_url`: `pages(canonical_url)`, computed by the shared vault-core URL normalizer.

**`urls`** — Browser profile observations: source URL ID, title, counts, times, hidden flag and provenance. Writer: `archive/ingest/writes; takeout`.

- Primary key: `id`.
- Foreign key: `page_id` → `pages.id`; delete no action.
- Foreign key: `created_by_run_id` → `runs.id`; delete no action.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Index `idx_urls_page_id`: `urls(page_id)`.
- Index `idx_urls_url`: `urls(url)`.
- Index `idx_urls_last_visit_ms`: `urls(last_visit_ms DESC, id DESC)` (v2).
- Unique index `idx_urls_profile_source_url_id`: `urls(source_profile_id, source_url_id)`.
- Index `idx_urls_profile_last_visit`: `urls(source_profile_id, last_visit_ms DESC)`.

**`visits`** — Visits with source identity, navigation metadata and rollback visibility. Writer: `archive/ingest/writes; takeout/batches`.

- Primary key: `id`.
- Foreign key: `reverted_by_run_id` → `runs.id`; delete no action.
- Foreign key: `created_by_run_id` → `runs.id`; delete no action.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Foreign key: `url_id` → `urls.id`; delete no action.
- Index `idx_visits_visible_time_id`: `visits(visit_time_ms, id) WHERE reverted_at IS NULL`.
- Index `idx_visits_visible_profile_time_id`: `visits(source_profile_id, visit_time_ms ASC, id ASC) WHERE reverted_at IS NULL`.
- Index `idx_visits_visible_url_time`: `visits( url_id, visit_time_ms DESC, id DESC ) WHERE reverted_at IS NULL`.
- Index `idx_visits_visible_batch_time_id`: `visits(import_batch_id, visit_time_ms DESC, id DESC) WHERE reverted_at IS NULL AND import_batch_id IS NOT NULL` (v2), for complete review pagination.
- Index `idx_visits_import_batch_id`: `visits(import_batch_id)`.
- Index `idx_visits_batch_url`: `visits(import_batch_id, url_id)`; keyset walks stay within the changed batch.
- Resumable visibility uses `import_batches.previous_status`, `visibility_cursor`, and `visibility_run_id`; `reverting` / `restoring` resume from the last committed URL chunk. The schema also includes `idx_og_images_eviction` and `idx_og_images_lru` so eviction follows an index rather than sorting each page.
- Unique index `idx_visits_profile_event_fingerprint`: `visits(source_profile_id, event_fingerprint) WHERE event_fingerprint IS NOT NULL AND event_fingerprint != ''`.
- Unique index `idx_visits_profile_source_visit_id`: `visits(source_profile_id, source_visit_id)`.
- Index `idx_visits_profile_time`: `visits(source_profile_id, visit_time_ms DESC)`.

**`downloads`** — Observed downloads, paths, progress and rollback visibility. Writer: `archive/ingest/writes`.

- Primary key: `id`.
- Foreign key: `reverted_by_run_id` → `runs.id`; delete no action.
- Foreign key: `created_by_run_id` → `runs.id`; delete no action.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Unique index `idx_downloads_profile_source_download_id`: `downloads(source_profile_id, source_download_id)`.
- Index `idx_downloads_profile_start`: `downloads(source_profile_id, start_time_ms DESC)`.

**`search_terms`** — Source search terms and rollback visibility. Writer: `archive/ingest/writes; takeout`.

- Primary key: `id`.
- Foreign key: `reverted_by_run_id` → `runs.id`; delete no action.
- Foreign key: `created_by_run_id` → `runs.id`; delete no action.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Foreign key: `url_id` → `urls.id`; delete no action.
- Unique index `idx_search_terms_profile_url_term`: `search_terms(source_profile_id, url_id, normalized_term)`.
- Index `idx_search_terms_profile_term`: `search_terms(source_profile_id, normalized_term)`.

**`favicons`** — Profile-specific icon observations and page/host/domain lookup facts. Writer: `archive/ingest/writes`.

- Primary key: `id`.
- Foreign key: `image_blob_hash` → `favicon_blobs.blob_hash`; delete no action.
- Foreign key: `created_by_run_id` → `runs.id`; delete no action.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Index `idx_favicons_registrable_lookup`: `favicons( page_registrable_domain, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_registrable_domain IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)`.
- Index `idx_favicons_registrable_profile_lookup`: `favicons( source_profile_id, page_registrable_domain, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_registrable_domain IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)`.
- Index `idx_favicons_host_lookup`: `favicons( page_host, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_host IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)`.
- Index `idx_favicons_host_profile_lookup`: `favicons( source_profile_id, page_host, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_host IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)`.
- Index `idx_favicons_blob_hash`: `favicons(image_blob_hash) WHERE image_blob_hash IS NOT NULL`.
- Index `idx_favicons_page_lookup`: `favicons( page_url, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE image_blob_hash IS NOT NULL OR image_data IS NOT NULL`.
- Index `idx_favicons_recall_lookup`: `favicons( source_profile_id, page_url, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE image_blob_hash IS NOT NULL OR image_data IS NOT NULL`.
- Unique index `idx_favicons_profile_page_icon_payload`: `favicons(source_profile_id, page_url, icon_url, payload_hash) WHERE payload_hash IS NOT NULL`.

**`manifests`** — Content hashes, row counts and parent links for audit manifests. Writer: `archive/run_support`.

- Primary key: `id`.
- Foreign key: `parent_manifest_id` → `manifests.id`; delete no action.
- Foreign key: `run_id` → `runs.id`; delete no action.
- Index `idx_manifests_run_id`: `manifests(run_id, id DESC)` (v2).
- Unique index `idx_manifests_content_hash`: `manifests(content_hash)`.

**`snapshots`** — Saved snapshot paths, sizes, checksums and reasons. Writer: `archive/run_support; archive/maintenance`.

- Primary key: `id`.
- Foreign key: `run_id` → `runs.id`; delete no action.
- Index `idx_snapshots_run_id`: `snapshots(run_id, id)` (v2).

**`profile_watermarks`** — Per-profile incremental cursors and last successful evidence batch. Writer: `archive/ingest/parser; takeout/payload_import`.

- Primary key: `profile_id`.

**`favicon_blobs`** — Content-addressed icon bytes shared by favicon observations. Writer: `archive/ingest/writes`.

- Primary key: `blob_hash`.

**`url_annotations`** — User notes shared across profiles, keyed by the exact URL. Writer: `annotations`.

- Primary key: `url`.
- Index `idx_url_annotations_updated_at`: `url_annotations(updated_at DESC)`.
- Index `idx_url_annotations_canonical`: `url_annotations(canonical_url, updated_at DESC, url)`.

**`url_tags`** — User tags shared across profiles, one row per exact URL/tag. Writer: `annotations`.

- Primary key: `url, tag`.
- Index `idx_url_tags_tag`: `url_tags(tag, url)`.
- Index `idx_url_tags_canonical`: `url_tags(canonical_url, created_at, tag)`.

**`og_image_blobs`** — Content-addressed preview image bytes and dimensions. Writer: `archive/history/og_images`.

- Primary key: `blob_hash`.

**`og_images`** — Exact-page preview fetch result, retry state and display recency. Writer: `archive/history/og_images`.

- Primary key: `id`.
- Foreign key: `created_by_run_id` → `runs.id`; delete no action.
- Foreign key: `image_blob_hash` → `og_image_blobs.blob_hash`; delete no action.
- Index `idx_og_images_last_shown`: `og_images(last_shown_at) WHERE last_shown_at IS NOT NULL`.
- Index `idx_og_images_refetch`: `og_images(refetch_after) WHERE refetch_after IS NOT NULL`.
- Index `idx_og_images_blob_hash`: `og_images(image_blob_hash) WHERE image_blob_hash IS NOT NULL`.
- Unique index `idx_og_images_page_url`: `og_images(page_url)`.

**`star`** — User favorites keyed by normalized URL or registrable domain. Writer: `stars`.

- Primary key: `entity_kind, entity_key`.
- Index `idx_star_kind_starred_at`: `star(entity_kind, starred_at DESC)`.

**`import_batches`** — Import provenance, status, undo state and local audit artifact references. Writer: `takeout/batches; takeout/payload_import; archive/doctor`.

- Primary key: `id`.
- No secondary indexes.

**`search_documents`** — One searchable document per page, including bounded enrichment, notes and tags. Writer: `archive/search_projection and archive mutation writers (Turso)`.

- Primary key: `page_id`.
- Foreign key: `page_id` → `pages.id`; delete cascade.
- Index `idx_search_documents_newest`: `search_documents(newest_visit_ms DESC, page_id DESC)`.
- Index `idx_search_documents_domain_time`: `search_documents(domain, newest_visit_ms DESC, page_id DESC)`.

**`search_document_profiles`** — Visible visit count and newest time for each page/profile pair. Writer: `Archive mutation writers (Turso)`.

- Primary key: `page_id, source_profile_id`.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Foreign key: `page_id` → `search_documents.page_id`; delete cascade.
- Index `idx_search_document_profiles_profile`: `search_document_profiles(source_profile_id, newest_visit_ms DESC, page_id)`.

**`search_outbox`** — Ordered upsert/delete notifications committed with page document changes. Writer: `Archive mutation writers; Tantivy consumer acknowledges rows (consumer pending)`.

- Primary key: `seq`.
- Index `idx_search_outbox_page_seq`: `search_outbox(page_id, seq)` (v2), for latest-state consistency checks without repeated outbox scans.

### Source Evidence — `archive/source-evidence.db`

**`source_batches`** — Parser version, source schema, capabilities, coverage and artifact provenance for one batch. Writer: `archive/source_evidence`.

- Primary key: `id`.
- Index `idx_source_batches_run`: `source_batches(run_id)`.
- Index `idx_source_batches_profile_created`: `source_batches(source_profile_id, created_at DESC)`.

**`schema_observations`** — Raw schema observations for a source database within a batch. Writer: `archive/source_evidence`.

- No declared primary key; ordinary rowid preserves repeated observations.
- No secondary indexes.

**`visit_search_evidence`** — Browser-native search evidence and normalized values. Writer: `archive/source_evidence`.

- Primary key: `id`.
- Index `idx_visit_search_evidence_batch`: `visit_search_evidence(source_batch_id, source_profile_id)`.

**`visit_navigation_evidence`** — Browser-native navigation edges and referrers. Writer: `archive/source_evidence`.

- Primary key: `id`.
- Index `idx_visit_navigation_evidence_batch`: `visit_navigation_evidence(source_batch_id, source_profile_id)`.

**`visit_engagement_evidence`** — Browser-native integer/real engagement metrics. Writer: `archive/source_evidence`.

- Primary key: `id`.
- Index `idx_visit_engagement_evidence_batch`: `visit_engagement_evidence(source_batch_id, source_profile_id)`.

**`visit_context_evidence`** — Browser-native visit or URL context JSON. Writer: `archive/source_evidence`.

- Primary key: `id`.
- Index `idx_visit_context_evidence_batch`: `visit_context_evidence(source_batch_id, source_profile_id)`.

**`native_entities`** — Non-visit native entities with payload, metadata and parent identity. Writer: `archive/source_evidence`.

- Primary key: `id`.
- Index `idx_native_entities_batch_kind`: `native_entities(source_batch_id, entity_kind)`.

### Intelligence — `derived/intelligence.db`

**`visit_derived_facts`** — Per-visit domain, taxonomy, search, session and trail explanation facts. Writer: `intelligence/intelligence_visit_records; intelligence/intelligence_core_persist`.

- Primary key: `visit_id`.
- Index `idx_vdf_profile_visit_id`: `visit_derived_facts(profile_id, visit_id)`.
- Index `idx_vdf_profile_search`: `visit_derived_facts(profile_id, is_search_event, search_engine)`.
- Index `idx_vdf_profile_domain`: `visit_derived_facts(profile_id, registrable_domain)`.
- Index `idx_vdf_profile_trail`: `visit_derived_facts(profile_id, trail_id)`.
- Index `idx_vdf_profile_session`: `visit_derived_facts(profile_id, session_id)`.

**`domain_daily_rollups`** — Daily visit/search/discovery counts per profile/domain. Writer: `intelligence/intelligence_daily_rollups`.

- Primary key: `date_key, profile_id, registrable_domain`.

**`category_daily_rollups`** — Daily visits and distinct domains per profile/category. Writer: `intelligence/intelligence_daily_rollups`.

- Primary key: `date_key, profile_id, domain_category`.

**`engine_daily_rollups`** — Daily search counts per profile/engine. Writer: `intelligence/intelligence_daily_rollups`.

- Primary key: `date_key, profile_id, search_engine`.

**`daily_summary_rollups`** — Daily totals and concentration/discovery scores per profile. Writer: `intelligence/intelligence_daily_rollups`.

- Primary key: `date_key, profile_id`.

**`sessions`** — Derived browsing sessions and titles. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_stream`.

- Primary key: `session_id`.
- Index `idx_sessions_profile_time`: `sessions(profile_id, first_visit_ms DESC)`.

**`search_trails`** — Search-to-landing trails, queries, depths and timing. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_stream`.

- Primary key: `trail_id`.
- Index `idx_search_trails_profile_time_trail`: `search_trails(profile_id, first_visit_ms ASC, trail_id ASC)`.
- Index `idx_search_trails_profile_time`: `search_trails(profile_id, first_visit_ms DESC)`.

**`search_trail_members`** — Ordered visits and roles in a trail. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_stream`.

- Primary key: `trail_id, visit_id`.
- Index `idx_search_trail_members_profile_visit`: `search_trail_members(profile_id, visit_id)`.

**`search_events`** — Search query, kind, engine and visit time for range reads. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_stream`.

- Primary key: `visit_id`.
- Index `idx_search_events_profile_query`: `search_events(profile_id,normalized_query)`.
- Index `idx_search_events_kind_time`: `search_events(query_kind,visit_time_ms)`.
- Index `idx_search_events_profile_kind`: `search_events(profile_id,query_kind)`.
- Index `idx_search_events_profile_visit`: `search_events(profile_id, visit_id)`.
- Index `idx_search_events_profile_engine`: `search_events(profile_id, search_engine)`.

**`search_event_terms`** — Individual normalized query terms for a search event. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_stream`.

- Primary key: `visit_id, term`.
- Index `idx_search_event_terms_profile_term`: `search_event_terms(profile_id, term)`.

**`query_families`** — Related search queries and recurrence times. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_persist`.

- Primary key: `family_id`.
- Index `idx_query_families_profile_time`: `query_families(profile_id, last_seen_ms DESC)`.

**`refind_pages`** — Repeated page discovery signals and scores per profile. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_persist`.

- Primary key: `profile_id, canonical_url`.
- Index `idx_refind_pages_profile_score`: `refind_pages(profile_id, refind_score DESC, last_seen_ms DESC)`.

**`source_effectiveness`** — Per-domain trail outcomes and stable landing scores. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_persist`.

- Primary key: `profile_id, registrable_domain`.
- Index `idx_source_effectiveness_profile_score`: `source_effectiveness(profile_id, effectiveness_score DESC)`.

**`habit_patterns`** — Per-domain recurrence intervals and interruption state. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_persist`.

- Primary key: `profile_id, registrable_domain`.

**`reopened_investigations`** — Repeated investigations across distinct days. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_persist`.

- Primary key: `investigation_id`.
- Index `idx_reopened_investigations_profile_time`: `reopened_investigations(profile_id, last_seen_ms DESC)`.

**`path_flows`** — Repeated navigation patterns per profile. Writer: `intelligence/intelligence_core_persist; intelligence/intelligence_structural_persist`.

- Primary key: `profile_id, flow_pattern, step_count`.

**`visit_content_enrichments`** — Fetched metadata, bounded summary, blob pointer and retry/extractor state. Writer: `enrichment; enrichment/content_fetch`.

- Primary key: `id`.
- Index `idx_visit_content_enrichments_status`: `visit_content_enrichments(fetch_status, fetched_at)`.
- Index `idx_visit_content_enrichments_history_id`: `visit_content_enrichments(history_id)`.
- Unique: `history_id, content_source` (automatic index).

**`site_dictionary_overrides`** — User-authored taxonomy and classification overrides. Writer: `intelligence/site_dictionary/overrides`.

- Primary key: `id`.
- Index `idx_site_dictionary_overrides_target`: `site_dictionary_overrides(target_kind, target_value)`.
- Unique: `target_kind, target_value` (automatic index).

**`search_engine_rules`** — User-authored search engine detection rules; built-ins remain code. Writer: `intelligence/site_dictionary/search_rules`.

- Primary key: `rule_id`.
- Index `idx_search_engine_rules_host`: `search_engine_rules(host_pattern, enabled)`.

**`core_intelligence_stage_checkpoints`** — Incremental stage cursors, dirty ranges and fallback reasons. Writer: `intelligence/incremental`.

- Primary key: `profile_id, stage`.

**`intelligence_overview_snapshots`** — Persisted overview payload and fingerprint per scope/band. Writer: `intelligence/intelligence_overview_snapshot`.

- Primary key: `scope_key, band`.

**`intelligence_jobs`** — Durable derived-work queue, deduplication, leases and cancellation. Writer: `intelligence_runtime`.

- Primary key: `id`.
- Index `idx_intelligence_jobs_plugin_state`: `intelligence_jobs(plugin_id, state, updated_at DESC)`.
- Index `idx_intelligence_jobs_state`: `intelligence_jobs(job_type, state, updated_at DESC)`.
- Unique: `dedupe_key` (automatic index).

**`intelligence_job_triggers`** — Requests that caused a queue job, with run/reason/time. Writer: `intelligence_runtime/enqueue`.

- Primary key: `id`.
- Foreign key: `job_id` → `intelligence_jobs.id`; delete cascade.
- Index `idx_intelligence_job_triggers_job_id`: `intelligence_job_triggers(job_id, requested_at DESC)`.

**`deterministic_module_runtime`** — Module version, dependencies, build/invalidation state and notes. Writer: `intelligence_runtime/enqueue; intelligence/intelligence_rebuild`.

- Primary key: `module_id`.

**`ai_embeddings`** — Compact embedding metadata; vector bytes live in sidecars. Writer: `ai/indexing/store_rows; ai/ledger`.

- Primary key: `id`.
- Index `idx_ai_embeddings_history_id`: `ai_embeddings(history_id)`.
- Index `idx_ai_embeddings_provider_model`: `ai_embeddings(provider_id, model)`.
- Unique: `history_id, provider_id, model, content_hash` (automatic index).

**`ai_assistant_runs`** — Assistant question/answer records with citations and provider trace. Writer: `ai/search`.

- Primary key: `id`.
- No secondary indexes.

**`ai_index_ledger`** — Per-provider/model semantic index state and rebuild accounting. Writer: `ai/ledger`.

- Primary key: `provider_id, model`.

**`ai_jobs`** — Optional AI queue with retries, leases and stop requests. Writer: `ai_queue`.

- Primary key: `id`.
- Index `idx_ai_jobs_state_available`: `ai_jobs(state, available_at, priority DESC, id ASC)`.

### Agent — `derived/agent.db`

**`conversations`** — Conversation title, provider, timestamps and message count. Writer: `agent_store`.

- Primary key: `id`.
- Index `idx_conversations_updated`: `conversations(updated_at DESC)`.

**`messages`** — Ordered transcript messages, reasoning and tool call payloads. Writer: `agent_store`.

- Primary key: `id`.
- Foreign key: `conversation_id` → `conversations.id`; delete cascade.
- Index `idx_messages_conversation_seq`: `messages(conversation_id, seq)`.

**`agent_runs`** — Durable agent execution status, providers and token accounting. Writer: `agent_store`.

- Primary key: `id`.
- Foreign key: `conversation_id` → `conversations.id`; delete cascade.
- Index `idx_agent_runs_conversation`: `agent_runs(conversation_id)`.

**`agent_steps`** — Ordered model/tool journal used to resume a run. Writer: `agent_store`.

- Primary key: `id`.
- Foreign key: `run_id` → `agent_runs.id`; delete cascade.
- Index `idx_agent_steps_run_seq`: `agent_steps(run_id, seq)`.

**`agent_citations`** — Evidence pinned to a run independently of transcript replacement. Writer: `agent_store`.

- Primary key: `run_id, history_id`.
- Foreign key: `run_id` → `agent_runs.id`; delete cascade.

## Intentional omissions and renamed columns

- Archive `settings` and its `key`, `value`, `updated_at` columns: no production
  consumer exists. Keys belong outside SQL, in the envelope.
- Archive `schema_migrations` (`version`, `applied_at`, `checksum`, `backup_path`),
  intelligence `intelligence_schema_migrations` (`version`, `name`, `applied_at`),
  and agent `agent_schema_migrations` (`version`, `name`, `checksum`, `applied_at`):
  these describe legacy migration steps. Store owns `__pk_migrations` instead.
- `urls.registrable_domain` moves to `pages.registrable_domain`; its old
  `idx_urls_registrable_domain` is replaced by `idx_pages_registrable_domain`.
- Old search `search_documents.url_id` becomes `page_id`. All text fields and
  `updated_at` survive; documents now aggregate across profiles.
- `search_projection_meta` (`key`, `value`): old FTS schema/seed flags are obsolete;
  Store versions SQL and Tantivy versions its own index.
- `history_search`, `history_search_terms`, `history_search_trigram`, and their
  FTS shadow tables/indexes: full-text indexing belongs to Tantivy.
- Already-retired intelligence tables `insight_bursts`, `insight_query_groups`,
  `insight_query_group_members`, `insight_topics`, `insight_threads`,
  `insight_thread_members`, `insight_reference_pages`, `insight_source_effectiveness`,
  `insight_cards`, `insight_snapshot_payloads`, `insight_runs`, `visit_insight_features`:
  the current runtime already removes them and no live storage reader needs them.
- Source evidence, agent transcript/journal and live intelligence data columns:
  none dropped. This includes fields only written for preservation/export.

The archive retains the `runs.id = 0` system provenance row used by existing
writers. It is data, not another migration. New run states remain text, so the
batched writer model can record `running`, `completed` and `interrupted` without
losing older audit statuses.

## Archive v2 read indexes

Version 1 remains the consolidated table baseline. Version 2 creates the batch-review,
outbox-page, manifest-run and snapshot-run indexes and replaces the URL recency index
with `(last_visit_ms DESC, id DESC)`. Store applies this transaction once at owner open;
existing native v1 vaults keep all facts and encrypted files. A plaintext/encrypted
upgrade test compares every persisted fixture field before and after the index step.
No read path runs DDL. Doctor expects Archive v3 and v1 for the other durable databases.

Version 3 initializes `archive_totals` keys named `sourceVisits:<source_profile_id>`
with indexed counts of visible visits. Ingest applies one source delta per committed
batch. Revert/restore consumes the changed rows' profile IDs through `RETURNING` and
updates only those profiles in the same transaction. Duplicate imports and resumed
visibility chunks do not double-count. Source statistics and profile/browser timeline
totals perform counter lookups instead of walking every page/profile summary.
Doctor compares these counters with canonical visits and repairs drift. Global counter
readers select their six named keys, so per-source keys do not leak into manifest counts.

Turso 0.8.1 requires explicit `IS NOT NULL` predicates for the partial host/domain/batch
indexes in these query shapes. Timeline/review/media cursors use separate equality-tie
and strict-range seeks rather than relying on tuple comparisons. Production-plan tests
reject full fact/page scans and unwanted `SORTER` operations; details and minimal
reproductions are in [the archive-read report](research/2026-10-turso-archive-reads.md).

## Verification

The `store_schema` tests use temporary real Store files in plaintext and with a
32-byte key. They hold independent expected table/index lists and full-column row
fixtures, written before the migration implementation. They cover CRUD, foreign
keys and cascades, natural-key uniqueness, shared pages, revert/restore membership,
document/outbox commit and rollback, tombstones and sequence pruning, reopen and
future-version refusal for every database. Scratch is empty after reopen.

Run offline from `src-tauri`:

```sh
CARGO_NET_OFFLINE=true cargo fmt --all --check
CARGO_NET_OFFLINE=true cargo clippy -p vault-core --all-targets -- -D warnings
CARGO_NET_OFFLINE=true cargo test -p vault-core store_schema
CARGO_NET_OFFLINE=true cargo test -p vault-core
CARGO_NET_OFFLINE=true cargo build --workspace
```

Archive runtime metadata (part of version 1):

- `profile_watermarks.last_search_url_time` is independent from the URL cursor.
  `url_scan_json` records the last committed source rowid and the original time filter,
  visit floor/ceiling and full/incremental selection for every browser family.
- `archive_totals(name, value)` caches enabled source profiles, URLs, visible visits,
  visible downloads, manifest rows and snapshot rows. It starts at zero; fact
  transactions apply deltas.
- `import_batches.visible_items` changes with visit visibility; `previous_status`
  is non-null with the `imported` default. New imports explicitly start it as `running`
  and update it to their terminal import state.
- `pages.canonical_url`, `url_annotations.canonical_url` and `url_tags.canonical_url`
  use the shared URL normalizer and have indexes for variant equality lookups.
