# Search integration validation

The owner path is `Vault → search session → vault-search → canonical Turso hydration`.
Tests use real temporary Vaults, including encrypted stores. Transport fixtures write
canonical facts and use the owner indexer; they do not provide canned search results.

## Coverage audit

STATUS names the audit's 13 locations without enumerating all of them. The search
call sites resolve as follows; output page sizes remain separate from coverage.

| Former restriction                                           | Outcome                                                                                                                                   |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Grouped keyword 25,000-page ranking window                   | Removed; complete global BM25 ranking and cursor enumeration.                                                                             |
| Ungrouped keyword window                                     | Removed; all search is canonical page search; timeline reads remain area 2.                                                               |
| Large-match newest-first fallback                            | Removed; relevance never changes because a query is common.                                                                               |
| Regex newest 50,000 visits / stopping at an empty chunk      | Complete canonical scan; empty chunks continue; counts settle only at exhaustion.                                                         |
| Fuzzy 200 trigram candidates                                 | Complete fuzzy predicate and cursors; when edits can destroy all shared trigrams, the scan also visits their complement.                  |
| Hybrid lexical candidate pool derived from requested limit   | Complete lexical rank input before fusion.                                                                                                |
| Starred lexical over-fetch ×8 / 1,000                        | Canonical eligibility precedes final selection; no finite over-fetch pool.                                                                |
| Semantic Hamming ×8 / ×64 / 2,000 candidate pool             | Every stored vector is scored; canonical facets cannot hide a lower-ranked eligible page.                                                 |
| AI date-order 1,000-row window                               | Chronological engine cursors continue to exhaustion.                                                                                      |
| Blank AI query's finite newest pool                          | All visible pages are pageable with canonical chronological order.                                                                        |
| Code-mode `fetch_visits` newest 1,000 window                 | Requested IDs are hydrated directly through indexed canonical visits. `query_history` cursors enumerate every page.                       |
| Palette first-six stop / no continuation through scan chunks | The six displayed results come from a complete rank walk; the streaming surface supplies provisional steps until it finishes.             |
| MCP and export inheriting truncated retrieval                | MCP and legacy assistant tools carry next cursors and coverage; search export waits for its starting archive prefix and walks every page. |

Annotation API limits are owned by area 1: annotation inventory/search already
returns `{items, nextCursor}` over Turso, including annotations created before a
page has a visit. They are not visit-eligible page searches; routing these orphan
annotations through the page index would incorrectly hide them.

The 50-row model message and 1,000-row sandbox response limits bound individual
messages, with continuation. Sandbox memory/fuel/time/host-call limits bound code
execution, not the search universe. Enrichment excerpts bound displayed text, not
indexed candidates. AI embedding-generation/working-set limits belong to area 4;
search scores every vector that exists. Timeline/site/read-model limits belong to
area 2. Retention, migration bundles and cleanup belong to area 5/6. Star list
pagination and canonical write coverage are owned by area 1.

## Failure modes and contract tests

| Modules                                                             | Failure modes covered                                                                                                                                                                                                     |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `search/documents`, `indexer`, `generations`                        | Acknowledgement before commit; duplicate replay; incompatible/missing/corrupt index; build gaps; cancelled rebuild replacing a healthy reader; leaked generations; close retaining writers or keys.                       |
| `search/query`, existing lexical normalizer, engine analyzer        | Lost numeric/Unicode prefixes; phrase boundaries; exclusion/OR/field drift; CJK/accent/OpenCC drift; long tokens discarded by the engine; nonadjacent trigrams accepted as substrings.                                    |
| `search/eligibility`, `visits`, `archive/search_documents`          | Date and profile satisfied by different visits; revoked text from a surviving shared page; stale session counts; accidental archive scans; missing notes/tags/enrichment; lost title variants.                            |
| `search/session`, `stream`                                          | Rank/count blocking each other; provisional cursors; repeated/skipped pages; changed filters accepted with old cursors; failed sinks retaining jobs; close racing start; retained caller handles keeping snapshots alive. |
| `search/regex`, `regex_prefilter`                                   | Empty chunks stop scanning; page copies repeat; unrelated profile title hydrates; optional regex fragment causes false negatives; stale prefilter hides newly changed data.                                               |
| `ai/search_pages`, `search_ranking`, `search_store`, vector scoring | Page-size-derived recall pools; facet post-filter loss; partial RRF; sampled counts; numeric offsets; stale SQLCipher metadata; deep IDs unreachable in code mode.                                                        |
| History/export adapters                                             | Search opens legacy storage; numbered pages diverge from cursors; partial initial index creates a complete-looking export; cancellation leaves export artifacts.                                                          |
| Worker, MCP, Tauri/dev bridge                                       | Different engines/shapes; missing nonempty hydration; dropped cursors; slow SSE consumer loses final page; disconnect/close retains work; MCP loses audit or consent checks.                                              |

The source headers contain the module-local failure lists and test locations.
`search/tests` compares generated query forms, same-visit eligibility, paging and
progressive completion in both storage modes. `projection_tests` checks revocation
before outbox apply and during a live session. `recovery_tests` kills a child after
prepare/commit handoff, reopens, corrupts real metadata, and pauses a healthy rebuild
while the old generation serves. `fuzzy_tests` compares every stored document with the existing edit predicate, including words with no surviving shared trigram, exact compound/CJK branches and star facets. `coverage_tests` walks 25,007 pages / 50,014 visits
for lexical and fuzzy search, finds an old regex/palette record, and exports all
matches. AI tests cover 1,107 lexical pages and 1,207 vectors with an eligible page
below every former top candidate. Code-mode executes real Javy/Wasmtime paging and
old-ID hydration. Worker, MCP and SSE tests enumerate 1,107 canonical pages.

## Test ports and deletions

The grouped, regex, query/normalization, annotation/enrichment, AI/RRF, code-mode,
worker, MCP and search-export suites now exercise the native owner. Timeline and
favicon assertions remain on their existing read path. Legacy generated SQL input
is copied into canonical test facts only in fixture builders; assertions call the
production search engine. Outbox durability assertions use the durable sequence
and published index, since acknowledged queue rows are now deleted.

Behavioral replacements include full-archive paging instead of window-edge
termination, opaque cursors instead of numeric offsets, exact global counts instead
of window counts, canonical plan assertions instead of FTS temporary-table plans,
progressive owner rebuilds instead of projection rebuild callbacks, and page results
instead of duplicate visit rows. No query form or visibility test was dropped to
make a failing implementation pass. Model-facing summaries and code-mode replies
retain incomplete index coverage; no tool prompt instructs users to work around a
retrieval window.

Only FTS-engine-specific assertions were removed:

- `bundled_sqlcipher_supports_required_fts_features`
- `fuzzy_trigram_candidate_probe_is_limited_before_rust_rerank` (replaced by full fuzzy coverage)
- `search_schema_version_is_v4_with_notes_and_tags_text`
- `search_schema_v3_to_v4_auto_rebuilds_and_adds_notes_tags`
- `encrypted_archive_attaches_plaintext_search_projection`
- FTS `MATCH`/virtual-table query-plan assertions in `legacy_queries` and grouped search.
- v0.2.0 upgrade assertions requiring FTS schema v4, FTS reprojection progress, and deletion of stale FTS document 999. Canonical upgrade preservation assertions remain.

Projection behavior tests were ported: enrichment mirror/no-op/update, notes/tag
mirror/clear, and initial/intermediate/final rebuild progress. The old window tests
now assert complete coverage, stable paging, relevant winners, exact totals and
same-visit filters through `ported_grouped`, `projection_tests`, `tests` and
`coverage_tests`; the required truncation behavior itself is intentionally gone.

## Implementation decisions

- Use durable generation pointers so healthy rebuilds and old reader lifetimes remain independent.
- Publish increasing initial-build prefixes and expose coverage separately from exact query counts.
- Validate current canonical visibility, including text lost by partial revocation; restart cached membership/counts on canonical changes.
- Keep normalization in vault-core and version its projection in engine commits.
- Preserve global advanced constraints and field-substring behavior; phrase positions never come from a gram set, and quoted aliases remain literal.
- Pin the committed visit prefix for paging, revalidate revocation, and keep a delivered-page bitset so backup/revocation cannot skip or repeat a page.
- Use an explicit last-ID lookup and indexed page/visit batches; positive tag postings preselect rare tag queries.
- Retain complete vector scoring and RRF rank inputs; page size controls only final output.
- Keep serialized compatibility fields (`windowed`, historical note/upgrade codes) readable; native search never emits truncation.
- Preserve existing timeline/read implementations in the shared dispatch files.
- Keep the historical migration number as a no-op; search never opens or attaches the old FTS database.
- Record tokenizer limitations and local reproductions in [search.md](../search.md); use tested local analyzers and make no vault-crypto changes.

## Required gates

| Command                                                                   | Result                                                                                                                     |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `cd src-tauri && cargo fmt --all --check`                                 | Passed                                                                                                                     |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings`    | Passed                                                                                                                     |
| `cargo test -p vault-core -p vault-worker -p vault-search -p vault-store` | Passed: vault-core 1,867 unit tests; vault-worker 149 unit tests; engine/store integration and documentation suites passed |
| `bun run test:desktop-bridge:rust`                                        | Passed: 23 bridge tests                                                                                                    |
| `cargo build --workspace`                                                 | Passed                                                                                                                     |

All Cargo commands used the dedicated target directory and offline dependency
resolution. Corpus/recovery tests ran with four test threads. The ignored full-scale
benchmark ran separately; its complete measurements and cleanup assertion are in
[the benchmark report](2026-10-vault-core-search-benchmark.md).

## Validation scope

The requested Rust gates use the dedicated `shared-target-area3` directory and
Cargo offline mode. Runtime tests use a local guard that denies outside resolution
and connections while permitting loopback fixtures and Unix sockets.

The broader `bun run check` also passed JavaScript formatting, lint, i18n,
type-checking, frontend build and desktop unit tests. It stopped in
`owner_process::cold_client_backup_single_instance_handoff_and_mcp_use_only_owner`:
macOS LaunchServices rejected the generated `PathKeep Test.app` with
`kLSNoExecutableErr` (-10827), and its owner endpoint never became ready.
The owner-launch implementation/test was not changed to suppress this failure.
Consequently this run did not reach the frontend E2E suite. Frontend channel wiring
and its obsolete window assertions remain with the frontend owner.

[Encrypted owner benchmark](2026-10-vault-core-search-benchmark.md) records all
five query cases, exact counts, backup/indexing contention, interactive visibility,
rebuild and RSS. Its generated root was removed. Hardware inventory was unavailable;
cold means fresh process, with no claim that the OS cache was purged.

## Changed files

A = added, M = modified, D = removed.

- M `docs/architecture/data-model.md`
- A `docs/architecture/research/2026-10-search-integration-validation.md`
- A `docs/architecture/research/2026-10-vault-core-search-benchmark.jsonl`
- A `docs/architecture/research/2026-10-vault-core-search-benchmark.md`
- A `docs/architecture/search.md`
- M `docs/architecture/store-schema.md`
- M `docs/architecture/turso-migration-design.md`
- M `docs/features/intelligence.md`
- M `docs/features/recall.md`
- M `docs/plan/CHANGELOG.md`
- M `docs/plan/STATUS.md`
- M `src-tauri/Cargo.lock`
- M `src-tauri/Cargo.toml`
- M `src-tauri/crates/vault-core/Cargo.toml`
- M `src-tauri/crates/vault-core/src/ai.rs`
- M `src-tauri/crates/vault-core/src/ai/agent_tools.rs`
- A `src-tauri/crates/vault-core/src/ai/assistant.rs`
- M `src-tauri/crates/vault-core/src/ai/code_mode.rs`
- M `src-tauri/crates/vault-core/src/ai/code_mode/tests.rs`
- M `src-tauri/crates/vault-core/src/ai/reverse_visit_map.rs`
- M `src-tauri/crates/vault-core/src/ai/search.rs`
- A `src-tauri/crates/vault-core/src/ai/search_pages.rs`
- A `src-tauri/crates/vault-core/src/ai/search_ranking.rs`
- A `src-tauri/crates/vault-core/src/ai/search_store.rs`
- M `src-tauri/crates/vault-core/src/ai/tests.rs`
- M `src-tauri/crates/vault-core/src/ai/vector_index.rs`
- M `src-tauri/crates/vault-core/src/annotations/search_tests.rs`
- M `src-tauri/crates/vault-core/src/archive/backup.rs`
- M `src-tauri/crates/vault-core/src/archive/history.rs`
- M `src-tauri/crates/vault-core/src/archive/history/export.rs`
- M `src-tauri/crates/vault-core/src/archive/history/export/progress.rs`
- M `src-tauri/crates/vault-core/src/archive/history/export/regression_tests.rs`
- D `src-tauri/crates/vault-core/src/archive/history/grouped.rs`
- D `src-tauri/crates/vault-core/src/archive/history/grouped/tests.rs`
- D `src-tauri/crates/vault-core/src/archive/history/grouped/window.rs`
- M `src-tauri/crates/vault-core/src/archive/history/pagination.rs`
- D `src-tauri/crates/vault-core/src/archive/history/regex_scan.rs`
- D `src-tauri/crates/vault-core/src/archive/history/regex_scan/tests.rs`
- A `src-tauri/crates/vault-core/src/archive/history/timeline_grouping_test.rs`
- M `src-tauri/crates/vault-core/src/archive/ingest/concurrency_tests.rs`
- M `src-tauri/crates/vault-core/src/archive/mod.rs`
- M `src-tauri/crates/vault-core/src/archive/schema.rs`
- M `src-tauri/crates/vault-core/src/archive/search_documents.rs`
- M `src-tauri/crates/vault-core/src/archive/search_documents/tests.rs`
- M `src-tauri/crates/vault-core/src/archive/search_lexical.rs`
- D `src-tauri/crates/vault-core/src/archive/search_projection.rs`
- M `src-tauri/crates/vault-core/src/archive/search_query.rs`
- M `src-tauri/crates/vault-core/src/archive/tests.rs`
- M `src-tauri/crates/vault-core/src/archive/tests/legacy_queries.rs`
- A `src-tauri/crates/vault-core/src/archive/tests/native_search.rs`
- M `src-tauri/crates/vault-core/src/archive/upgrade_v020_tests.rs`
- M `src-tauri/crates/vault-core/src/archive/wipe/tests.rs`
- M `src-tauri/crates/vault-core/src/lib.rs`
- M `src-tauri/crates/vault-core/src/migrations/003_history_search_fts.sql`
- M `src-tauri/crates/vault-core/src/models/archive.rs`
- M `src-tauri/crates/vault-core/src/models/intelligence.rs`
- A `src-tauri/crates/vault-core/src/search/benchmark.rs`
- A `src-tauri/crates/vault-core/src/search/benchmark/backup.rs`
- A `src-tauri/crates/vault-core/src/search/benchmark/fixture.rs`
- A `src-tauri/crates/vault-core/src/search/benchmark/measure.rs`
- A `src-tauri/crates/vault-core/src/search/coverage_tests.rs`
- A `src-tauri/crates/vault-core/src/search/documents.rs`
- A `src-tauri/crates/vault-core/src/search/eligibility.rs`
- A `src-tauri/crates/vault-core/src/search/fixture.rs`
- A `src-tauri/crates/vault-core/src/search/fuzzy_tests.rs`
- A `src-tauri/crates/vault-core/src/search/generations.rs`
- A `src-tauri/crates/vault-core/src/search/history.rs`
- A `src-tauri/crates/vault-core/src/search/indexer.rs`
- A `src-tauri/crates/vault-core/src/search/mod.rs`
- A `src-tauri/crates/vault-core/src/search/ported_grouped.rs`
- A `src-tauri/crates/vault-core/src/search/ported_queries.rs`
- A `src-tauri/crates/vault-core/src/search/ported_regex.rs`
- A `src-tauri/crates/vault-core/src/search/projection_tests.rs`
- A `src-tauri/crates/vault-core/src/search/query.rs`
- A `src-tauri/crates/vault-core/src/search/recovery_tests.rs`
- A `src-tauri/crates/vault-core/src/search/regex.rs`
- A `src-tauri/crates/vault-core/src/search/regex_prefilter.rs`
- A `src-tauri/crates/vault-core/src/search/session.rs`
- A `src-tauri/crates/vault-core/src/search/stream.rs`
- A `src-tauri/crates/vault-core/src/search/tests.rs`
- A `src-tauri/crates/vault-core/src/search/types.rs`
- A `src-tauri/crates/vault-core/src/search/visits.rs`
- M `src-tauri/crates/vault-core/src/storage_migration.rs`
- M `src-tauri/crates/vault-core/src/takeout/batch_search_tests.rs`
- M `src-tauri/crates/vault-core/src/test_support.rs`
- M `src-tauri/crates/vault-core/src/test_support/legacy_fixtures.rs`
- A `src-tauri/crates/vault-core/src/test_support/search.rs`
- M `src-tauri/crates/vault-core/src/vault.rs`
- M `src-tauri/crates/vault-core/src/vault/live.rs`
- M `src-tauri/crates/vault-core/tests/s2_vector_index_bench.rs`
- M `src-tauri/crates/vault-core/tests/turso_write_path.rs`
- M `src-tauri/crates/vault-search/Cargo.toml`
- M `src-tauri/crates/vault-search/README.md`
- M `src-tauri/crates/vault-search/src/analyzer.rs`
- M `src-tauri/crates/vault-search/src/collector.rs`
- M `src-tauri/crates/vault-search/src/filter.rs`
- M `src-tauri/crates/vault-search/src/index.rs`
- M `src-tauri/crates/vault-search/src/lib.rs`
- M `src-tauri/crates/vault-search/src/model.rs`
- M `src-tauri/crates/vault-search/src/progress.rs`
- M `src-tauri/crates/vault-search/src/query.rs`
- M `src-tauri/crates/vault-search/src/schema.rs`
- M `src-tauri/crates/vault-search/tests/ast.rs`
- A `src-tauri/crates/vault-search/tests/canonical.rs`
- M `src-tauri/crates/vault-search/tests/contracts.rs`
- M `src-tauri/crates/vault-search/tests/recovery.rs`
- M `src-tauri/crates/vault-worker/src/lib.rs`
- M `src-tauri/crates/vault-worker/src/mcp.rs`
- A `src-tauri/crates/vault-worker/src/mcp_search_tests.rs`
- M `src-tauri/crates/vault-worker/src/owner_requests/mod.rs`
- A `src-tauri/crates/vault-worker/src/search.rs`
- A `src-tauri/crates/vault-worker/src/search_tests.rs`
- M `src-tauri/crates/vault-worker/src/session.rs`
- M `src-tauri/crates/vault-worker/src/tests.rs`
- M `src-tauri/src/commands/mod.rs`
- A `src-tauri/src/commands/search.rs`
- M `src-tauri/src/dev_ipc_bridge.rs`
- M `src-tauri/src/dev_ipc_bridge/dispatch.rs`
- M `src-tauri/src/dev_ipc_bridge/router.rs`
- A `src-tauri/src/dev_ipc_bridge/search.rs`
- A `src-tauri/src/dev_ipc_bridge/search_tests.rs`
- M `src-tauri/src/lib.rs`
- M `src-tauri/src/worker_bridge/mod.rs`
- A `src-tauri/src/worker_bridge/search.rs`

## Archive recency index and page refresh

Combining the archive read schema with search exposed a Turso 0.8.1 planner
regression. `idx_urls_last_visit_ms(last_visit_ms DESC, id DESC)` satisfies the
title projection's ordering, so the planner scans that index and filters
`page_id` afterward. Refreshing one page then reads unrelated URLs; repeating
this for every inserted page makes fixture construction grow quadratically.

Minimal reproduction: open `Vault::open_temp()` with the consolidated schema
(no data is needed to reproduce the plan) and execute this through an Archive
read Session, binding page ID 1:

```sql
EXPLAIN QUERY PLAN
SELECT COALESCE(group_concat(title, ' | '), '') FROM (
  SELECT u.title FROM urls u WHERE u.page_id = ?1
  AND EXISTS (
    SELECT 1 FROM visits v WHERE v.url_id = u.id AND v.reverted_at IS NULL
  )
  ORDER BY u.last_visit_ms DESC, u.id DESC
);
```

Observed plan: `SCAN urls AS u USING INDEX idx_urls_last_visit_ms`, followed by
an indexed visit lookup. The local fix adds `INDEXED BY idx_urls_page_id` to the
URL source. It seeks only the changed page's profile observations before sorting
their titles. The recency index remains available to archive readers; no schema
step or dependency change is needed. The production-query regression
`archive::search_documents::tests::affected_page_refresh_seeks_its_urls_visits_and_search_terms`
checks the page and visible-visit indexes in plaintext and encrypted stores.
