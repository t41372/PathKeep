# Turso archive reads — area 2

The native archive readers share the session's `Vault`. Search, including grouped
keyword/regex queries and search exports, remains a separate area: the worker dispatches
those requests to the existing search function. No grouped search SQL is changed here.
Frontend and browser E2E files are outside this work.

## Read and pagination decisions

- Public readers receive `&Vault`; SQL helpers receive `&Session`. Native reads use the
  read pool, without schema initialization or a writer-queue request.
- Timeline, day aggregation, import review and image-candidate walks split a cursor into
  an equality seek within its timestamp and a strict remaining range. A complete
  tie-breaker makes every row reachable. Timeline requests with numbered pages beyond
  the first must supply a cursor.
- A lookahead row determines continuation independently from totals. Limits bound pages,
  not the reachable set. First/last/latest/best-image lookups remain scalar selections;
  day top cards summarize the entire day, rather than limit the visits being aggregated.
- Archive totals and per-source totals use `archive_totals`. The per-source keys are
  `sourceVisits:<profile_id>`. Global and source visit deltas share one upsert statement
  per ingest batch; the previous ingest statement budget is unchanged. Revert/restore
  accounts for the actual profiles of changed rows, including mixed-profile batches.
  Exact-page totals use existing page/profile summaries, with each profile counted once
  even if it has several URL observations. Weekly/date/domain aggregates still inspect
  their indexed ranges.
- Day insights retain one read page plus aggregate maps, not all visits. Exports retain
  one 1,000-row page and a 256 KiB output buffer, bound the walk by its initial maximum
  visit ID, and ignore the screen's current page/cursor. Each page sees committed state;
  visibility changes can be observed between pages.
- Source statistics initially summed every page/profile summary. The encrypted fixture
  measured 1,414.953 ms cold and 326.475 ms warm. Per-source counters remove that work
  from the render path. They are checked by Doctor against canonical visits.
- Archive v2 installs read indexes; v3 initializes source counters. These are forward
  migrations for existing native vaults, not a reset of their data. Upgrade tests compare
  every existing fixture field in plaintext and encrypted files.
- Existing SQLite initialization, whole-file maintenance and intelligence algorithms stay
  explicitly isolated with their owning areas. Browser-file schema inspection remains
  on read-only rusqlite. The per-call source-evidence opener is removed from the native
  read boundary; excluded callers use its explicitly named legacy counterpart.

## Turso 0.8.1 findings and reproductions

### A tuple cursor can still reread the prefix

This production-shaped query uses the time index but does not necessarily seek to its
cursor in Turso 0.8.1:

```sql
SELECT id, visit_time_ms FROM visits INDEXED BY idx_visits_visible_time_id
WHERE reverted_at IS NULL
  AND visit_time_ms >= ?1 AND visit_time_ms <= ?2
  AND (visit_time_ms, id) < (?3, ?4)
ORDER BY visit_time_ms DESC, id DESC LIMIT ?5;
```

With the date upper bound at `i64::MAX`, `EXPLAIN` emitted `SeekLE` from that bound; the
tuple comparison remained a filter. An export's later pages therefore reread the prefix.
Checking only that `EXPLAIN QUERY PLAN` names an index misses this failure.

The replacement first seeks `visit_time_ms = cursor_time AND id < cursor_id`, then, if
needed, reads `visit_time_ms < cursor_time`. Ascending walks use the reverse comparisons.
Do not add a weaker `id <= export_upper_id` to a descending equality-tie query when the
cursor already dominates it: that duplicate bound also selected the wrong starting seek.
The export watermark remains enforced by the range query and by the dominant tie bound.

`timeline_tests::cursor_seek_uses_the_id_after_an_equal_timestamp` checks the production
bytecode for `SeekLT`; the result tests walk ties across page boundaries in both
orders. Plan tests reject both SQLite's `TEMP B-TREE` wording and Turso's `SORTER` wording.

### Partial-index predicates need to be explicit

The host favicon index includes:

```sql
WHERE page_host IS NOT NULL
  AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
```

In `LOAD_FAVICON_SAME_PROFILE_HOST_SQL` or `LOAD_FAVICON_CROSS_PROFILE_HOST_SQL`, remove
`AND page_host IS NOT NULL` and run `EXPLAIN QUERY PLAN` with profile 1, host
`example.com`, page `https://example.com/a` and time 5000. The forced partial index then
returns `no query solution` on 0.8.1. Restoring the explicit predicate gives an index seek.
The equality alone does not establish the predicate for this query shape.

The domain-star branch similarly scanned `pages` when its partial registrable-domain
predicate was implicit. It now uses an explicit non-null predicate and a correlated
scalar sum. This also removes grouping/sorting over all matching URL observations;
only the star result set is sorted. Tests require both page indexes and URL index seeks.
The import-review index likewise carries an explicit non-null batch predicate.

These are local query changes; no dependency is patched and no upstream report is sent.

### Integrity and cancellation

The cached 0.8.1 source and the repository's
[probes](2026-10-turso-probes-part-b.md) confirm `integrity_check` and `quick_check`.
Store exposes only fixed `schema_version()` and `integrity_check()` methods inside a
reader job; arbitrary pragmas and migration-table access remain outside business SQL.

There is no `foreign_key_check` pragma, so Doctor checks canonical references through
explicit parent-key lookups. There is no public running-statement interrupt/progress
handler. Export cancellation is cooperative between read pages, rows and 64 KiB write
chunks; an executing statement and full integrity check cannot be interrupted. The old
SQLite opcode-interruption test is deleted because that engine capability does not exist.

The reported [MVCC/PASSIVE integrity timeout](2026-10-turso-probes-part-c.md) belongs to
that probe's live MVCC sequence; its exact internal cause was not established. This Store
uses the accepted single-owner WAL design, not MVCC.

Turso buffers `UPDATE … RETURNING` before yielding results. Its ephemeral B-tree uses
`temp_store=FILE` and the configured IO layer, so each run creates a temporary file. Since
2026-10-11 undo and restore read the visits that change and then update them, without
`RETURNING` (`ipc-performance.md` §14).

## Failure modes and tests

All native database scenarios use real temporary Store files, including keyed files.
Pure parsing/calendar/formatting tests do not need a database. Existing excluded-area
algorithms retain their own legacy fixtures.

| Modules                                        | Required failure modes / verification                                                                                                                                                                                                                                        |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `history`, `timeline`, `pagination`            | Timestamp ties skip/repeat visits; filters change counts; OFFSET/prefix scans; incorrect has-next; a reader sees or waits for an uncommitted batch. Keyset, production-plan, bytecode and real multi-batch backup tests cover them.                                          |
| `day_insights`                                 | Incomplete days, profile leakage, reverted visits, wrong hour/session boundaries, DST gaps, lost title backfill, a dense day crossing read pages. All existing calendar/aggregation scenarios are retained; 4,101 tied visits span pages.                                    |
| `source_stats`, `totals`                       | Empty sources disappear; reverted visits counted; stale counters; duplicates/resume count twice; mixed-profile undo updates the wrong source. Real import/duplicate/revert/restore and read-during-backup cases verify transaction coupling.                                 |
| `url_detail`                                   | Wrong 12-week boundaries, hidden visits/browsers, unknown URL errors, duplicate observations multiply counts, full fact scans. Existing weekly/empty cases and production plans are retained and extended.                                                                   |
| `favicons`                                     | Future icons, wrong profile precedence, unrelated registrable hosts, duplicate replies, missing profiles, shared blob loss, candidate scans. Native fallback and blob fixtures replace the old mixed legacy scenario.                                                        |
| `og_images/reads`                              | Exact-page/negative-cache mistakes, HTTP/cached prefetch candidates, tied candidates lost at a limit. Existing cache scenarios run in both modes and new cursors reach every candidate.                                                                                      |
| `read_models`, `run_support`, `snapshot_reads` | Recounting/stale totals, missing running runs, compatibility seed leakage, audit field loss, truncated run lists, explicit zero replaced by requested scope, passwordless unlock reading SQLite. Native fixtures and serialized bridge requests cover these.                 |
| `artifacts`, browser schema boundary           | Lost source/snapshot identity or hashes, missing paths, browser staging mutated. Existing artifact/checkpoint/parser tests remain; only browser-owned files use rusqlite here.                                                                                               |
| `batch_review`                                 | Older/unfinished batches hidden, tied preview rows lost, hidden visits shown, visible-item counts scan facts. Complete batch/review walks and indexed query plans cover them.                                                                                                |
| `doctor`, `doctor_facts`                       | Schema/integrity omitted, orphan references, missing/modified artifacts, bad outbox state or counters, repair unhides visits, failed work marked successful, derived-only repair loses its run. Native repair cases and retained legacy intelligence cases cover them.       |
| `export`, `progress`                           | Screen cursor truncates export, ties disappear, buffers grow, cancellation leaks files or another job's flag, setup/read errors never become terminal, format bytes change. Native streaming/format/cancellation cases and the scale benchmark cover them.                   |
| `stars/list`                                   | Domain branch scans pages, grouping sorts the corpus, unknown future kinds leak, totals/titles or cursor order change. Existing star tests plus full production-plan assertions cover them.                                                                                  |
| `store_schema`, Store inspection               | Existing v1/v2 data loses fields, migrations replay or accept future versions, encrypted inspection fails. Full catalog/CRUD/reopen/upgrade tests and Store's `health` integration test cover them.                                                                          |
| command/worker/dev IPC entries                 | Requests use a different store or require a SQLCipher password, new cursors are unreachable, progress polling blocks. The native bridge test imports 41 tied visits, unlocks through the keychain, walks them and their review pages, reads summaries and exports every row. |

The original source-stat and URL-detail tests are ported to Session/Vault. The original
13 day/calendar cases are retained in `day_insights_tests.rs`, with plan and dense-page
cases added. Export formatting, output comparison, setup/query failure, cancellation,
large-field chunking and bounded-buffer cases are retained on the native store. The
browser-source staging and excluded search tests keep their engine-specific fixtures.

The monolithic archive-reader tests below are replaced by focused native tests, rather
than discarded without coverage:

| Former scenarios                                                                                                                                                                                             | Native or retained coverage                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dashboard_read_models_cover_uninitialized_storage_and_cached_totals_edges`, `dashboard_snapshot_reports_archive_coverage_bounds_for_imported_visits`, `load_recent_runs_excludes_compat_seed_baseline_row`  | `read_model_tests`: empty metadata/files, cached counters, visible date bounds, seed exclusion and 35 tied run rows. Legacy malformed/incomplete run-JSON fallback remains in `legacy_totals::tests` for maintenance. |
| `doctor_detects_missing_snapshot_artifacts`, `doctor_labels_every_check_with_a_stable_kebab_code`, `doctor_detects_manifest_parent_and_hash_damage`, `doctor_detects_import_batches_without_audit_artifacts` | `doctor_tests`: every snapshot is checked, missing import audits, manifest parent/hash damage and stable codes.                                                                                                       |
| `doctor_repair_noops_on_healthy_archive`, `doctor_repair_restores_missing_import_artifacts_visibility_and_derived_state`, `doctor_repair_restores_visibility_when_import_audits_are_intact`                  | Native no-op, artifact, visibility, counter/outbox and derived-only audit tests; legacy stale-row algorithms retain their existing tests.                                                                             |
| `doctor_repair_tolerates_missing_optional_intelligence_tables`                                                                                                                                               | Retained as `doctor_derived::tests::legacy_repairs_tolerate_missing_optional_intelligence_tables`, including the same dropped-table combinations.                                                                     |
| `doctor_repair_records_rebuilt_import_artifacts_when_git_is_enabled`, `doctor_repair_records_failed_run_when_audit_artifact_rewrite_fails`                                                                   | Native optional-Git outcome/warning and failed-run provenance tests.                                                                                                                                                  |
| Favicon assertions in `legacy_queries`                                                                                                                                                                       | Native exact/profile/host/time/blob tests, including separate `github.io` and `example.co.uk` hosts; legacy search/export assertions remain with area 3.                                                              |
| `sqlite_work_can_be_interrupted_before_it_yields_a_row`                                                                                                                                                      | Deleted: SQLite progress-handler engine behavior. Cooperative native export cancellation is tested instead; no fake interrupt API is provided.                                                                        |

## Request/response changes

The full contracts are in [data-model.md](../data-model.md#turso-archive-read-contract).

- `query_history`: same fields/cursor encoding; numbered timeline pages beyond page 1
  require a cursor, and `hasNext` comes from lookahead. Limits remain page sizes.
- New `load_recent_runs({cursor?, limit?}) -> {items, nextCursor}`; run ID cursor is a number.
- New `load_import_batches({cursor?, limit?}) -> {items, nextCursor}`; cursor is an opaque string.
  Unfinished operations sort first, then ID descending.
- `preview_import_batch` accepts optional `cursor` and `limit` (default 24), and adds
  `previewNextCursor` to its existing detail response.
- `AppSnapshot` adds `recentRunsNextCursor`, `recentImportBatchesNextCursor`; its archive
  directory path names the same native file as `archiveStatus.databasePath`.
- `DashboardSnapshot` adds `recentRunsNextCursor`.
- `HealthRepairReport` adds numeric `repairedTotalCounters`, `repairedBatchTotals`,
  `refreshedSearchPages`, `queuedSearchUpdates`. Existing fields remain, and only actual
  work produces action notes. Doctor repair is no longer refused by the migration gate.
- Core OG candidate APIs now take an optional cursor and return `{items, nextCursor}`;
  background worker response shapes and their explicit per-operation work budgets stay unchanged.
- Native core/worker functions take the retained Vault. Search dispatch and optional
  legacy intelligence diagnostics retain their existing password-dependent call. These
  Rust signature changes do not add credentials to desktop request payloads.

Doctor label copy for the frontend owner (diagnostic `detail` and repair `notes` remain
raw technical facts, as in the existing health-report contract):

| Code                        | en                          | zh-CN          | zh-TW          |
| --------------------------- | --------------------------- | -------------- | -------------- |
| `archive-integrity`         | Archive integrity           | 归档完整性     | 封存完整性     |
| `source-evidence-integrity` | Source evidence integrity   | 来源证据完整性 | 來源證據完整性 |
| `intelligence-integrity`    | Intelligence data integrity | 洞察数据完整性 | 洞察資料完整性 |
| `agent-integrity`           | Conversation data integrity | 对话数据完整性 | 對話資料完整性 |
| `canonical-references`      | Canonical references        | 归档引用       | 封存參照       |
| `cached-totals`             | Cached totals               | 缓存统计       | 快取統計       |
| `search-outbox`             | Search outbox               | 搜索更新队列   | 搜尋更新佇列   |

## Benchmark reproduction

With `CARGO_TARGET_DIR` set to this checkout's shared build directory, run from `src-tauri`:

```sh
CARGO_NET_OFFLINE=true cargo test -p vault-worker --release --test archive_scale_bench \
  turso_reads::encrypted_archive_reads_at_scale -- --ignored --nocapture --test-threads=1
```

The fixture has 14,400,000 visible visits, 2,880,000 stored exact pages, one profile and
20,000 domains. A page has 100,005 visits. The generator's hot-page redistribution leaves
2,860,000 pages with visits; URL-only anchors remain stored. Every measured operation gets
its own new child process and then a warm repetition. Cold does not purge the OS file
cache. Database open/migration and fixture construction are outside the timed calls.
The full export asserts its exact row count and records file bytes and `getrusage` peak RSS.
Warm RSS is the child process's cumulative high-water mark. `uptime` records load averages.
The harness checks the 40 GB bound and deletes generated data, including exported files.

The sandbox denied CPU/RAM inventory and `time -l`'s `kern.clockrate` query. Measurements
therefore use `Instant` and the process's own `getrusage`, without querying the denied
system fields. No timings are extrapolated to a different machine.

## Measured results

All operations below passed on the encrypted 14,400,000-visit / 2,880,000-page fixture.
Each cold operation used its own new process. These are observed wall-clock samples,
not target-machine projections. [Raw benchmark log](2026-10-turso-archive-reads-results.txt).

| Operation                        |       Cold |       Warm |
| -------------------------------- | ---------: | ---------: |
| First timeline page (150 visits) |   1.998 ms |   0.316 ms |
| Next timeline page (150 visits)  |   0.792 ms |   0.425 ms |
| One day's insights               |  26.477 ms |   9.020 ms |
| URL detail (100,005 visits)      |   1.885 ms |   0.490 ms |
| Overview                         |   1.176 ms |   0.603 ms |
| Source stats                     |   0.286 ms |   0.677 ms |
| Full JSONL history export        |   33.869 s |   19.771 s |
| Export process peak RSS          | 38.641 MiB | 38.766 MiB |

Load averages (1/5/15 minutes) were **5.01 / 7.71 / 6.53** for the reads and cold
export, and **4.74 / 7.36 / 6.45** for the warm export. The archive fixture occupied
5,357,121,542 bytes; each complete export was 4,643,256,843 bytes. Fixture plus export
peaked at 10,000,378,385 bytes, below 40 GB. Both export files and the complete synthetic
fixture were deleted; absence was verified. No 4-core/8-GB hardware certification is
claimed from a host whose CPU/RAM inventory the sandbox would not expose.

## Validation

The requested gates passed with the shared `CARGO_TARGET_DIR` and offline Cargo:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test -p vault-core -p vault-worker -p vault-store`
- `bun run test:desktop-bridge:rust` (20 serialized bridge cases)
- `cargo build --workspace`

The full Rust suite used `RUST_TEST_THREADS=4` and an isolated temporary directory.
External-model opt-in flags were off; no outside services were contacted. Gate logs and
a hash manifest are in `artifacts/turso-archive-reads/`. Search, intelligence, whole-file
maintenance, frontend integration and the browser E2E suite are separate areas.

## Changed files

The following repository files make up the change. Legacy-only entries are
mechanical call-site renames or retained maintenance/intelligence test boundaries.

```text
artifacts/turso-archive-reads/sha256.json
docs/architecture/data-model.md
docs/architecture/research/2026-10-turso-archive-reads-results.txt
docs/architecture/research/2026-10-turso-archive-reads.md
docs/architecture/store-schema.md
docs/architecture/turso-migration-design.md
docs/features/archive.md
docs/plan/CHANGELOG.md
src-tauri/Cargo.lock
src-tauri/crates/vault-core/examples/field-promotion-dry-run.rs
src-tauri/crates/vault-core/src/ai.rs
src-tauri/crates/vault-core/src/ai/agent_harness.rs
src-tauri/crates/vault-core/src/ai/code_mode.rs
src-tauri/crates/vault-core/src/ai/search.rs
src-tauri/crates/vault-core/src/archive/artifacts.rs
src-tauri/crates/vault-core/src/archive/at_rest.rs
src-tauri/crates/vault-core/src/archive/browser_schema.rs
src-tauri/crates/vault-core/src/archive/doctor.rs
src-tauri/crates/vault-core/src/archive/doctor_derived.rs
src-tauri/crates/vault-core/src/archive/doctor_facts.rs
src-tauri/crates/vault-core/src/archive/doctor_tests.rs
src-tauri/crates/vault-core/src/archive/history.rs
src-tauri/crates/vault-core/src/archive/history/day_insights.rs
src-tauri/crates/vault-core/src/archive/history/day_insights_tests.rs
src-tauri/crates/vault-core/src/archive/history/export.rs
src-tauri/crates/vault-core/src/archive/history/export/progress.rs
src-tauri/crates/vault-core/src/archive/history/export/regression_tests.rs
src-tauri/crates/vault-core/src/archive/history/favicon_tests.rs
src-tauri/crates/vault-core/src/archive/history/favicons.rs
src-tauri/crates/vault-core/src/archive/history/grouped/tests.rs
src-tauri/crates/vault-core/src/archive/history/og_images/reads.rs
src-tauri/crates/vault-core/src/archive/history/og_images/tests.rs
src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs
src-tauri/crates/vault-core/src/archive/history/pagination.rs
src-tauri/crates/vault-core/src/archive/history/regex_scan/tests.rs
src-tauri/crates/vault-core/src/archive/history/search_export.rs
src-tauri/crates/vault-core/src/archive/history/timeline.rs
src-tauri/crates/vault-core/src/archive/history/timeline_tests.rs
src-tauri/crates/vault-core/src/archive/ingest/concurrency_tests.rs
src-tauri/crates/vault-core/src/archive/ingest/consumer.rs
src-tauri/crates/vault-core/src/archive/legacy_source_evidence.rs
src-tauri/crates/vault-core/src/archive/legacy_status.rs
src-tauri/crates/vault-core/src/archive/legacy_totals.rs
src-tauri/crates/vault-core/src/archive/maintenance.rs
src-tauri/crates/vault-core/src/archive/maintenance_runs.rs
src-tauri/crates/vault-core/src/archive/mod.rs
src-tauri/crates/vault-core/src/archive/read_model_tests.rs
src-tauri/crates/vault-core/src/archive/read_models.rs
src-tauri/crates/vault-core/src/archive/run_support.rs
src-tauri/crates/vault-core/src/archive/source_evidence_open.rs
src-tauri/crates/vault-core/src/archive/source_stats.rs
src-tauri/crates/vault-core/src/archive/source_stats_tests.rs
src-tauri/crates/vault-core/src/archive/tests.rs
src-tauri/crates/vault-core/src/archive/tests/legacy_queries.rs
src-tauri/crates/vault-core/src/archive/totals.rs
src-tauri/crates/vault-core/src/archive/upgrade_v020_tests.rs
src-tauri/crates/vault-core/src/archive/url_detail.rs
src-tauri/crates/vault-core/src/archive/url_detail_tests.rs
src-tauri/crates/vault-core/src/archive/wipe.rs
src-tauri/crates/vault-core/src/archive/wipe/tests.rs
src-tauri/crates/vault-core/src/intelligence/phase_three.rs
src-tauri/crates/vault-core/src/lib.rs
src-tauri/crates/vault-core/src/migration.rs
src-tauri/crates/vault-core/src/migration/fault_tests.rs
src-tauri/crates/vault-core/src/models/app.rs
src-tauri/crates/vault-core/src/models/archive.rs
src-tauri/crates/vault-core/src/models/audit.rs
src-tauri/crates/vault-core/src/models/import.rs
src-tauri/crates/vault-core/src/snapshot_reads.rs
src-tauri/crates/vault-core/src/stars/list.rs
src-tauri/crates/vault-core/src/store_schema/archive.rs
src-tauri/crates/vault-core/src/store_schema/mod.rs
src-tauri/crates/vault-core/src/store_schema/tests/archive.rs
src-tauri/crates/vault-core/src/store_schema/tests/mod.rs
src-tauri/crates/vault-core/src/takeout/batch_review.rs
src-tauri/crates/vault-core/src/takeout/batch_review_tests.rs
src-tauri/crates/vault-core/src/takeout/batch_search_tests.rs
src-tauri/crates/vault-core/src/takeout/browser_history/consumer_tests.rs
src-tauri/crates/vault-core/src/takeout/browser_history/staging.rs
src-tauri/crates/vault-core/src/takeout/mod.rs
src-tauri/crates/vault-core/src/takeout/tests.rs
src-tauri/crates/vault-core/src/takeout/tests/browser_history.rs
src-tauri/crates/vault-core/src/takeout/visibility.rs
src-tauri/crates/vault-core/src/vault/tests.rs
src-tauri/crates/vault-core/tests/lmstudio_agent_e2e.rs
src-tauri/crates/vault-core/tests/turso_write_path.rs
src-tauri/crates/vault-store/README.md
src-tauri/crates/vault-store/src/session.rs
src-tauri/crates/vault-store/tests/health.rs
src-tauri/crates/vault-worker/Cargo.toml
src-tauri/crates/vault-worker/src/app.rs
src-tauri/crates/vault-worker/src/archive_flows.rs
src-tauri/crates/vault-worker/src/intelligence/chat.rs
src-tauri/crates/vault-worker/src/lib.rs
src-tauri/crates/vault-worker/src/mcp.rs
src-tauri/crates/vault-worker/src/owner_requests/mod.rs
src-tauri/crates/vault-worker/src/schedule.rs
src-tauri/crates/vault-worker/src/security.rs
src-tauri/crates/vault-worker/src/tests.rs
src-tauri/crates/vault-worker/tests/archive_scale_bench.rs
src-tauri/crates/vault-worker/tests/archive_scale_bench/turso_reads.rs
src-tauri/src/commands/app.rs
src-tauri/src/commands/archive.rs
src-tauri/src/commands/import.rs
src-tauri/src/commands/schedule.rs
src-tauri/src/commands/security.rs
src-tauri/src/desktop_integration/mod.rs
src-tauri/src/desktop_integration/status.rs
src-tauri/src/dev_ipc_bridge/dispatch.rs
src-tauri/src/dev_ipc_bridge/dispatch/native_reads_tests.rs
src-tauri/src/dev_ipc_bridge/dispatch/tests.rs
src-tauri/src/dev_ipc_bridge/payloads.rs
src-tauri/src/lib.rs
src-tauri/src/worker_bridge/app.rs
src-tauri/src/worker_bridge/archive.rs
src-tauri/src/worker_bridge/import.rs
src-tauri/src/worker_bridge/mod.rs
src-tauri/src/worker_bridge/schedule.rs
src-tauri/src/worker_bridge/security.rs
```
