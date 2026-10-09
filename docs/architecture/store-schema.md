# Turso store schema

`vault_core::store_schema::migration_registry()` supplies a consolidated v1
migration for Archive, SourceEvidence, Intelligence and Agent, with an empty list
for Scratch. Every statement runs through `vault_store::Session`; each database's
v1 and migration record commit in one transaction. No SQLite migration replay, triggers, TEMP tables, FTS virtual tables, window
functions or WITHOUT ROWID tables are used. Intelligence has sixteen generation views.

This is the schema contract for the Turso port in [ADR-012](decisions/012-turso-storage-engine.md)
and [the migration design](turso-migration-design.md). Archive writers and readers use Vault
and real Store transactions. Search uses the native archive and the owner's vault-search
index. Intelligence, AI queues/indexing, enrichment, agent storage and whole-file maintenance
(snapshots, bundles, rekey, mode switch, restore) use the native files; SQLCipher is read
only by the converter. The desktop session owns the shared Vault for ported commands.

The baseline is the final effect of archive SQL migrations 001–016 and runtime
DDL in archive schema/search/source evidence, intelligence schema and its runtime
helpers, AI schema/queue, and agent store. The v1 files contain consolidated
definitions; they do not include or execute those legacy migration files.

## Files and lifecycle

| Database        | File                         | Version | Application tables |
| --------------- | ---------------------------- | ------- | ------------------ |
| Archive         | `archive/history-vault.db`   | 1       | 22                 |
| Source evidence | `archive/source-evidence.db` | 1       | 7                  |
| Intelligence    | `derived/intelligence.db`    | 1       | 36                 |
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
owned by search code, not SQL. The title includes every visible profile title,
with phrase boundaries between copies. Compact/CJK text also includes approved enrichment.

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
index commit before pruning outbox rows. The archive writers append the outbox; the Vault owner indexer consumes and acknowledges it independently. There are no implicit
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
- Index `idx_urls_last_visit_ms`: `urls(last_visit_ms DESC, id DESC)`.
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
- Index `idx_visits_visible_batch_time_id`: `visits(import_batch_id, visit_time_ms DESC, id DESC) WHERE reverted_at IS NULL AND import_batch_id IS NOT NULL`, for complete review pagination.
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
- Index `idx_manifests_run_id`: `manifests(run_id, id DESC)`.
- Unique index `idx_manifests_content_hash`: `manifests(content_hash)`.

**`snapshots`** — Saved snapshot paths, sizes, checksums and reasons. Writer: `archive/run_support; archive/maintenance`.

- Primary key: `id`.
- Foreign key: `run_id` → `runs.id`; delete no action.
- Index `idx_snapshots_run_id`: `snapshots(run_id, id)`.

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

**`search_documents`** — One searchable document per page, including bounded enrichment, notes and tags. Writer: `archive/search_documents and archive mutation writers (Turso)`.

- Primary key: `page_id`.
- Foreign key: `page_id` → `pages.id`; delete cascade.
- Index `idx_search_documents_newest`: `search_documents(newest_visit_ms DESC, page_id DESC)`.
- Index `idx_search_documents_domain_time`: `search_documents(domain, newest_visit_ms DESC, page_id DESC)`.

**`search_document_profiles`** — Visible visit count and newest time for each page/profile pair. Writer: `Archive mutation writers (Turso)`.

- Primary key: `page_id, source_profile_id`.
- Foreign key: `source_profile_id` → `source_profiles.id`; delete no action.
- Foreign key: `page_id` → `search_documents.page_id`; delete cascade.
- Index `idx_search_document_profiles_profile`: `search_document_profiles(source_profile_id, newest_visit_ms DESC, page_id)`.

**`search_outbox`** — Ordered upsert/delete notifications committed with page document changes. Writer: `Archive mutation writers; owner indexer acknowledges committed rows with a Batch write`.

- Primary key: `seq`.
- Index `idx_search_outbox_page_seq`: `search_outbox(page_id, seq)`, for latest-state consistency checks without repeated outbox scans.

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

Consolidated v1 has 36 application tables and sixteen logical read views. Public operations
use `Vault`; SQL runs through `Session`. Rebuild writers live in `intelligence/build_pipeline`,
`build_rows`, `build_stages`, `build_refind` and `intelligence_structural_stream`. The logical
views below select published generations. Their physical names append `_rows`, and their
primary keys prepend `generation` to the logical key shown here.

| Logical view              | Logical key                                | Meaning                                                                                                                                           |
| ------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `visit_derived_facts`     | `visit_id`                                 | Domain, canonical URL, taxonomy and search classification; session/trail IDs come from `visit_assignments`.                                       |
| `domain_daily_rollups`    | `date_key, profile_id, registrable_domain` | Visit/search/discovery counts and distinct URLs.                                                                                                  |
| `category_daily_rollups`  | `date_key, profile_id, domain_category`    | Visits and distinct domains.                                                                                                                      |
| `engine_daily_rollups`    | `date_key, profile_id, search_engine`      | Daily searches.                                                                                                                                   |
| `daily_summary_rollups`   | `date_key, profile_id`                     | Totals, concentration and discovery.                                                                                                              |
| `sessions`                | `session_id`                               | Session span, counts, deep-dive state and title.                                                                                                  |
| `search_trails`           | `trail_id`                                 | Search/landing chain, depth, span and queries.                                                                                                    |
| `search_trail_members`    | `trail_id, visit_id`                       | Ordered members and roles.                                                                                                                        |
| `search_events`           | `visit_id`                                 | Search text, normalized query, kind, trail and visit time.                                                                                        |
| `search_event_terms`      | `visit_id, term`                           | Keyword concept terms.                                                                                                                            |
| `query_families`          | `family_id`                                | Related queries and repetition counts.                                                                                                            |
| `refind_pages`            | `profile_id, canonical_url`                | Reuse signals, score and bounded persisted evidence sample. Public evidence is paged from facts.                                                  |
| `reopened_investigations` | `investigation_id`                         | Recurring query/page anchors. Reference evidence resolves through the refind row.                                                                 |
| `path_flows`              | `profile_id, flow_pattern, step_count`     | Repeated all-time navigation windows, counted from `session_paths` the way a date range counts them.                                              |
| `compare_sets`            | `compare_set_id`                           | Pages of one kind compared within a search trail, found while the aggregate build streams visits; read newest first by `idx_compare_sets_recent`. |

Generation and checkpoint tables:

- `intelligence_generations(profile_id, facts, daily, navigation, aggregates)` selects the
  published generation of each group; `profile_id` is the primary key.
- `intelligence_builds(id, profile_id, state_json, updated_at)` stores captured source/rule
  state, cursors, dirty days, stage completion flags and the structural stream checkpoint.
- `intelligence_counts(generation, profile_id, table_name, value)` provides cached counts.
- `intelligence_day_counts(generation, profile_id, date_key, domains, categories, engines,
summaries)` records committed day totals for idempotent replay.
- `intelligence_seen_domains(generation, profile_id, domain)` preserves first-domain state.
- `visit_assignments(generation, visit_id, profile_id, session_id, trail_id, ordinal)` has
  primary key `(generation, visit_id)`. Session reads seek `(generation, session_id, ordinal,
visit_id)`; visit/generation and trail indexes support point hydration.
- `session_domains(generation, profile_id, session_id, domain, visit_count)` has primary key
  `(generation, session_id, domain)` and restores open-session counts without visit replay.
- `session_paths(generation, profile_id, session_id, segment, date_key, prefix_len, sites_json)`
  has primary key `(generation, session_id, segment)`: a session's sites in order with each one's
  last visit time, cut at local midnight and every 256 sites, each segment starting with the
  previous segment's last three sites. Bounded common paths read it through
  `idx_session_paths_range (date_key, generation, profile_id)`.

Full builds retain published rows until replacement stages commit. Append builds extend the
fact/daily/navigation groups in place. An append of up to 100,000 visits
(`build_append::IN_PLACE_LIMIT`) also updates the published aggregates in place: it works out,
reading only, each page a new visit opened (its refind row from all its visits), what each flow
gains (`session_paths` of the days it wrote, after it, less before it), the compare sets of the
trails walked again from the start of the trail open at the last build, and the query families
the new keyword searches join in visit order (all of them again, writing only those that
differ, if the search that opened that open trail changed kind); then it writes all of it in
one transaction with its done mark, so readers see the old aggregates or the new ones. A larger
append, or stored rows the update does not expect, builds the aggregates into a new generation.
A build records two rule versions: the URL rules (normalization, site, search; with the daily
and structural versions, `rule_version`) and the category rules (matcher, packs, the
categories a user set for sites; `category_version`). A change of the first builds anew. A
change of only the second sorts the visits already built again in place (`build_reclassify`):
each of the profile's URLs is classified once (a category depends on the URL, its title and its
search, not on a visit), the facts of the visits whose categories moved are rewritten, the daily
rollups of their days are made again (a day's category rows are removed first) and the compare
sets of their trails are replaced with the in-place aggregate write.
Checkpoints and their writes commit in one batch; superseded generations are reclaimed in
bounded writes. No TEMP tables are used.

**`page_content_enrichments`** has primary key `id` and unique `(canonical_url, content_source)`.
It stores status/time, final URL, language, title, blob path/size/hash, snippets, extraction JSON,
pipeline/extractor versions, summary, retry time and HTTP status. Canonical/source and
status/time lookups are indexed. Every visit of a canonical page shares this cache.

**`site_dictionary_overrides`** has primary key `id` and unique `(target_kind, target_value)`.
**`search_engine_rules`** has primary key `rule_id` and a host/enabled lookup index. They hold
user edits; built-in rules stay in code.

**`core_intelligence_stage_checkpoints`** has primary key `(profile_id, stage)` and retains
stage versions, source watermarks, dirty ranges and fallback reasons.
**`intelligence_overview_snapshots`** has primary key `(scope_key, band)`, fingerprint and
serialized payload. Producers warm it after publication; render reads do not write it.

**`intelligence_jobs`** has primary key `id`, unique `dedupe_key`, state/priority/attempt,
payload/artifact JSON, scheduling and lease timestamps, errors and cancellation state.
`canonical_url` indexes parked enrichment work. `trigger_cursor` captures requests covered by
a claim. A newer trigger leaves the job queued after completion. Artifact `contentStored`
records the cache-to-Archive publication handoff so recovery never refetches committed content.

**`intelligence_job_triggers`** has primary key `id`, `job_id` with delete cascade, run/reason/time,
and index `(job_id, id)`. **`deterministic_module_runtime`** has primary key `module_id`, module
version/dependencies, build/invalidation state and notes; `invalidation_revision` prevents
older completions from clearing newer stale state.

**`ai_embeddings`** stores compact per-visit metadata, never vectors or full text. It has
primary key `id`, unique `(history_id, provider_id, model, content_hash)`, history/provider/model
indexes and a provider/model/ID cleanup index. `metadata_bytes` and `content_bytes` support
exact delta accounting. Writer: `ai/indexing/store_rows`.

**`ai_embedding_totals`** is keyed by `(provider_id, model)` and stores `item_count`,
`content_bytes`, `metadata_bytes`, `last_indexed_at`. Replacements, rollback, cleanup and repair
update metadata and totals in the same transaction.

**`ai_assistant_runs`** retains question/answer, citations and provider trace under primary key
`id`. Its search producer remains area 3's responsibility. **`ai_index_ledger`**, keyed by
`(provider_id, model)`, stores index/run/watermark outcomes; writer: `ai/native_ledger`.
**`ai_jobs`** has primary key `id` and durable payload, retry, lease, cancellation and outcome
fields. Its state/availability/priority/ID index serves claims; ID cursors expose complete history.

Render indexes cover search kind/time and normalized query/time, refind score and canonical
identity, session overlap/order, trail member order, domain/date rollups and queue traversal.
Exact physical DDL is `src-tauri/crates/vault-core/src/store_schema/intelligence.rs`.

Paged lists keep one index in exactly their list order, with `generation` last:
`idx_sessions_recent`, `idx_search_trails_recent`, `idx_search_trails_friction`,
`idx_query_families_recent`, `idx_query_families_members`,
`idx_reopened_investigations_recent`, `idx_refind_pages_score` and, for one profile's all-time
flows, `idx_path_flows_profile_rank`. These reads use the `_rows` table with `INDEXED BY` and
check `generation IN (SELECT … FROM intelligence_generations)` per row, because through the
views Turso 0.8.1 reads the whole generation by primary key and sorts it on every page.

Keyset cursors are written in the bounded form, `a<=?X AND (a<?X OR b<?Y)` (nested for more
keys, `>=`/`>` for ascending keys), and the cursor clause is only in the statement text after
the first page. Turso 0.8.1 does not seek on `a<?X OR (a=?X AND b<?Y)`, nor on any cursor clause
behind `?X IS NULL OR`; both re-walk the index from its first row on every page. Both texts bind
the same values, so the cursor values come before the `LIMIT` value: Turso rejects a bound
index above the highest one in the statement, while lower unused indexes are fine. Plan tests in
`vault-core/tests/intelligence_keyset_plans.rs` read page two of each list and check the plan.

### Agent — `derived/agent.db`

**`conversations`** — Conversation title, provider, timestamps and message count. Writer: `agent_store`.

- Primary key: `id`.
- Index `idx_conversations_updated`: `conversations(updated_at DESC, id DESC)`; complete cursor order for tied timestamps.

**`messages`** — Ordered transcript messages, reasoning and tool call payloads. Writer: `agent_store`.

- Primary key: `id`.
- Foreign key: `conversation_id` → `conversations.id`; delete cascade.
- Unique index `idx_messages_conversation_seq`: `messages(conversation_id, seq)`; persisted transcript order and page cursor.

**`agent_runs`** — Durable agent execution status, providers and token accounting. Writer: `agent_store`.

- Primary key: `id`.
- Foreign key: `conversation_id` → `conversations.id`; delete cascade.
- Index `idx_agent_runs_conversation`: `agent_runs(conversation_id)`.
- Index `idx_agent_runs_message`: `agent_runs(conversation_id, message_id, updated_at DESC, id DESC)`; latest answering run for a message page.

**`agent_steps`** — Ordered model/tool journal used to resume a run. Writer: `agent_store`.

- Primary key: `id`.
- Foreign key: `run_id` → `agent_runs.id`; delete cascade.
- Index `idx_agent_steps_run_seq`: `agent_steps(run_id, seq)`.
- Unique partial index `idx_agent_steps_tool_call`: `agent_steps(run_id, kind, tool_call_id) WHERE tool_call_id IS NOT NULL`; replayed tool results keep their original sequence.

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

## Archive read indexes and source totals

The consolidated v1 includes the batch-review, outbox-page, manifest-run and
snapshot-run indexes, and the URL recency index `(last_visit_ms DESC, id DESC)`.
No read path runs DDL. Doctor expects v1 for every durable database. The catalog,
reopen and consolidated-version tests cover the schema in plaintext and with a key.

`archive_totals` keys named `sourceVisits:<source_profile_id>` track visible visits.
Ingest applies one source delta per committed
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

Area-4 render indexes include visit/generation point lookups, canonical-page visit traversal,
session overlap/order, trail member order, search kind/time and normalized query/time, refind
score/canonical traversal, parked jobs and recent-job tie breakers. SourceEvidence v1 adds
`(source_profile_id, source_visit_id)` indexes for engagement and context hydration. Batched
lookups drive these indexes with VALUES rows rather than relying on Turso's secondary-index
IN planning. Exact DDL is in `src-tauri/crates/vault-core/src/store_schema/intelligence.rs`
and `source_evidence.rs`; the reported logical keys above omit the physical generation prefix.

The v1 area-4 index set omits redundant profile/session, profile/trail, profile/search and
profile/visit indexes on generation facts/assignments. Membership reads use the
`(generation, session_id, ordinal, visit_id)` index; trail assignment lookup is partial on
`trail_id IS NOT NULL`. These choices avoid writing duplicate per-visit entries while
preserving the tested render and resume plans.
