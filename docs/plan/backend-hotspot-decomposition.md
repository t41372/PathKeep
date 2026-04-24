# Backend Hotspot Decomposition

> User-directed parallel backend track opened on 2026-04-21 while `WORK-M13-B` continues as a separate frontend/reuse stream.

## Why this track exists

- The Rust workspace is no longer suffering from the older `insights.rs` / `vault-worker/src/lib.rs` mega-file shape, but the current backend still has several oversized hotspots that mix orchestration, schema/bootstrap, read models, and long-running job control.
- The most acute risk is no longer "no architecture"; it is concentrated complexity in a few high-churn files that sit on large-data paths.
- This track turns the 2026-04-21 architecture review into an execution plan with explicit sequencing and invariants.

## Current hotspot snapshot

| File                                                  | Current line count | Primary risk                                                             |
| ----------------------------------------------------- | -----------------: | ------------------------------------------------------------------------ |
| `src-tauri/crates/vault-core/src/intelligence/mod.rs` |               2583 | Export surface, core record types, and regression suite still co-located |
| `src-tauri/crates/vault-core/src/remote.rs`           |               1165 | Bundle build, upload, verify, and endpoint helpers still mixed           |
| `src-tauri/crates/vault-worker/src/intelligence.rs`   |                124 | Thin façade after queue/runtime and read-surface splits                  |

## Sequencing

### Slice 1 — Takeout import boundary

- Split `takeout.rs` into focused modules for inspect/quarantine, import execution, and batch review / revert / restore.
- Keep Tauri command names, worker entrypoints, serde payloads, audit artifact format, and import-batch behavior unchanged.
- Use this slice to establish the doc-comment standard for newly split backend modules.
- 2026-04-21 landed: `src-tauri/crates/vault-core/src/takeout.rs` is now `takeout/{mod,inspect,import_flow,batches,tests}.rs`, and the targeted Takeout Rust regression suite passed after the split.
- 2026-04-21 follow-up landed: import execution no longer goes through the inspection preview helper. `takeout/import_flow.rs` now delegates payload parsing/writes to `takeout/payload_import.rs`, so import avoids allocating a second visit-sized preview vector and source-evidence plans take ownership of the parsed history instead of cloning it.
- 2026-04-21 review follow-up landed: batch review reads/audit repair now live in `takeout/batch_review.rs`, while `takeout/batches.rs` keeps the write-side revert/restore path. The active Takeout boundary no longer has any file above the 600-line hard stop.
- 2026-04-21 execute-path follow-up landed: non-dry-run `import_takeout` no longer runs a full `inspect_takeout` pass before importing. The execute path now scans files once, accumulates batch metadata while writing canonical rows, and hydrates its final review payload from `preview_import_batch`, removing the old double-parse / double-read behavior from the hottest Takeout import path.

### Slice 2 — Parser and ingest streaming boundary

- Revisit the parser-to-ingest contract so large imports no longer require one giant in-memory batch shape by default.
- Separate parser-side collection helpers from archive-side write orchestration.
- Preserve canonical schema semantics and profile watermark behavior.
- 2026-04-21 landed: `src-tauri/crates/vault-core/src/archive/mod.rs` now delegates canonical ingest work to `archive/ingest/{mod,parser,writes}.rs`, shrinking the parent file to `1299` lines while preserving backup, checkpoint, and snapshot-restore behavior. `archive::maintenance` now calls an explicit ingest preview helper instead of reaching into watermark internals.
- 2026-04-21 follow-up landed: `archive/mod.rs` no longer owns backup orchestration or manifest/snapshot support helpers. Those responsibilities now live in `archive/{backup,run_support,artifacts}.rs`, shrinking the parent module to `406` lines without changing backup, restore, or takeout-facing contracts.
- 2026-04-22 follow-up landed: backup ingest and Takeout import no longer retain a second full `ParsedHistory` just to persist cold source evidence after canonical commits. `archive::source_evidence` now consumes a narrower payload (`typed_evidence + native_entities`), which drops the hottest duplicate retention path while keeping parser APIs and source-evidence schema stable.
- 2026-04-22 streaming follow-up landed: the Chromium live-backup path no longer waits for one full parser batch before canonical writes begin. `browser-history-parser::chromium::stream_history` now emits URL/visit/download/search-term/favicon batches into `archive::ingest`, so the primary backup path can start canonical writes while parsing is still in progress.
- 2026-04-22 cross-family streaming follow-up landed: Firefox and Safari now expose the same streamed parser contract, and `archive::ingest` routes all live local-backup browser families through one streamed canonical-ingest path. The remaining full-batch parser path is now concentrated in Takeout and restore-preview flows rather than ordinary browser backup runs.
- 2026-04-22 bounded-memory follow-up landed: deferred cold source-evidence payloads now spill to temporary files under `staging/source-evidence-spool/` once they exceed the in-memory threshold, so backup/import flows no longer need to hold every post-commit native payload in RAM until the cold archive write begins. `snapshot_restore` preview also switched from parser materialization to direct checkpoint row counts, and checkpoint replay now resolves the owning profile id from the checkpoint directory when one backup run covered multiple profiles.
- 2026-04-22 Takeout streaming follow-up landed: `browser-history-parser::takeout` is now split into focused submodules and exposes payload-level streaming. `vault-core::takeout::payload_import` now consumes that streamed contract, so Takeout import can write canonical URL/visit rows while a BrowserHistory payload is still being parsed instead of waiting for one giant payload report first.
- 2026-04-22 inspection follow-up landed: `vault-core::takeout::inspect_takeout` also switched to the payload-level streamed Takeout contract. Dry-run preview now caps rows directly from streamed visits and explicitly disables source-evidence accumulation, so inspection no longer materializes a full payload report or keeps native evidence blobs in memory just to show preview rows.
- 2026-04-22 source-evidence follow-up landed: Takeout payload streaming now has an explicit source-evidence chunk sink. `vault-core::takeout::payload_import` routes those chunks through `archive::source_evidence_builder`, which spills oversized native evidence into `staging/source-evidence-spool/` before a single payload can accumulate one giant in-memory evidence batch.
- 2026-04-22 Chrome-first truth follow-up landed: `browser-history-parser::takeout` no longer treats any path containing `browser` / `history` as implicitly importable. The boundary now uses locale-aware path dispatch for dedicated Chrome history payloads (`BrowserHistory.json`, `History.json`, `Verlauf.json`), keeps typed-url / session companions as source-evidence-only, and marks Chrome-related `My Activity` files as review-needed instead of silently ingesting them. `vault-core::takeout::{inspect,import_flow}` now surface additive file classification (`will-import / known-but-ignored / needs-review / parse-error`), detected locale, and preview time range so the frontend can explain why a real Google Takeout does or does not import.
- 2026-04-22 real-payload follow-up landed: the Chrome-first boundary now also matches the real dedicated Takeout payload envelope instead of repo-local synthetic fixtures. `browser-history-parser::takeout::browser_history` accepts the observed top-level `Browser History` array together with `time_usec`, and the regression fixtures in both parser/vault-core now place those payloads under realistic `Chrome/BrowserHistory.json` paths so future refactors cannot go green against a fake payload shape again.

### Slice 3 — Intelligence runtime queue boundary

- Split `intelligence_runtime.rs` into queue writes, recovery, runtime snapshot reads, and module/plugin runtime ownership.
- Preserve the `IntelligenceRuntimeSnapshot` contract and job ids / job-type semantics.
- 2026-04-22 landed: `src-tauri/crates/vault-core/src/intelligence_runtime.rs` is now `intelligence_runtime/{mod,enqueue,claims,job_control,recovery,snapshot,tests_queue,tests_runtime}.rs`. Queue writes, compare-and-set claims, lease recovery, runtime snapshot reads, and runtime regression coverage now have distinct owners while preserving `load_intelligence_runtime`, retry/cancel controls, and worker claim semantics.

### Slice 4 — Core archive ingest boundary

- Split `archive/mod.rs` into backup execution, canonical ingest helpers, checkpoint / manifest helpers, and retention/recoverability helpers.
- Preserve canonical archive behavior, run-ledger semantics, and existing `archive::*` public surface.
- 2026-04-21 follow-up landed: backup execution plus manifest/checkpoint helpers are now out of `archive/mod.rs`, so the remaining archive-side work is no longer file-size triage.
- Remaining risk after the 2026-04-22 Takeout source-evidence cut: the Takeout import boundary has now been pushed onto bounded canonical-write, bounded preview, and bounded cold-evidence paths. The next slice should therefore shift to `intelligence_runtime.rs` queue/recovery/runtime-snapshot ownership or another measured backend hotspot, rather than revisiting already-streamed archive/takeout import paths.

### Slice 5 — Core Intelligence domain boundary

- Split `intelligence/mod.rs` by schema/bootstrap, rebuild stages, read models, and explanation / host-artifact surfaces.
- Do not reopen route, payload, or frontend grammar decisions already accepted in M6-M13.
- 2026-04-22 landed: the first `intelligence/mod.rs` cut extracted the route-facing overview/read-model layer into `intelligence_{overview,summary,domain,outputs}.rs`. `/intelligence` staged overview composition, digest/stable-source/search-effectiveness reads, domain/discovery/on-this-day surfaces, and export payload builders now have distinct owners while preserving the existing public query surface and runtime-loading contract.
- 2026-04-22 follow-up landed: the next cut extracted `intelligence_{refind,explain,explain_helpers}.rs`, so refind detail payloads, `explain_entity`, and explanation-only helper loaders now have dedicated owners too. The remaining `intelligence/mod.rs` hotspot is now concentrated around schema/bootstrap plus rebuild-stage ownership rather than route-facing explanation code.
- 2026-04-22 schema/rebuild follow-up landed: the next cut extracted `intelligence_{schema,schema_sql,rebuild}.rs`, so schema bootstrap, derived-state clear, public rebuild entrypoints, legacy scoped fallback, and runtime-ready module updates no longer live inside `intelligence/mod.rs`. The parent module dropped to `7703` lines, and the remaining hotspot was concentrated around structural rebuild internals plus query/read-model helper clusters rather than top-level orchestration.
- 2026-04-22 structural follow-up landed: the next cut extracted `intelligence_structural_{state,build,aggregates,persist,stream,stage}.rs`, giving structural rebuild state machines, streamed replay, write-side replacements, and stage orchestration distinct owners. `intelligence/mod.rs` is now down to `5561` lines, and every newly created structural module is back under the repo's `600`-line hard stop.
- 2026-04-22 read-model follow-up landed: the next cut extracted `intelligence_{sessions,navigation,search_metrics,search_queries}.rs` and moved domain-only helpers into `intelligence_domain.rs`. Session/trail detail reads, navigation-path reconstruction, search engine/rule/concept surfaces, and recent-search/query-family reads now have distinct owners, which brings `intelligence/mod.rs` down to `4508` lines without changing Tauri/worker/query contracts.
- 2026-04-22 helper-cluster follow-up landed: the next cut extracted `intelligence_{shared,visit_records,visit_derive,daily_rollup_state,daily_rollups,core_persist}.rs`. Shared date/query heuristics, visit-derived replay, daily-rollup replay, and scoped full-rebuild persistence now have dedicated owners, which brings `intelligence/mod.rs` down to `2583` lines and leaves the parent focused on exported surface, core record types, cursors, constants, and the still-embedded regression suite.

### Slice 6 — AI and worker follow-through

- Split `ai.rs`, `ai_queue.rs`, and `vault-worker/src/intelligence.rs` after the archive/intelligence core boundaries are clearer.
- Treat `remote.rs`, `models/core_intelligence.rs`, and dev-only bridge surfaces as follow-up cleanup, not first blockers.
- As of 2026-04-22, this is now the main remaining backend hotspot after the `intelligence/mod.rs` structural split; the next active block should treat worker passthrough clustering and `ai.rs` mixed ownership as first-order cleanup, not optional polish.
- 2026-04-22 worker follow-through landed: `vault-worker/src/intelligence.rs` is now a `789`-line façade with `intelligence/{ai_queue,runtime}.rs` owning queue/search/assistant orchestration and deterministic runtime control. The remaining worker-side hotspot is no longer queue execution itself; it is the still-mixed read-surface passthrough layer that sits above those dedicated owners.
- 2026-04-22 AI follow-through landed: `vault-core/src/ai.rs` is now a `199`-line façade with `ai/{control,provider,indexing,ledger,search,read_model}.rs` owning provider probes, semantic index builds, run-ledger bookkeeping, semantic search, and assistant tool orchestration. The remaining backend hotspot map no longer treats `ai.rs` as a first-order mega-file; the next real owners to revisit are `intelligence/mod.rs` and the still-mixed DTO / remote helper surfaces.
- 2026-04-22 worker read-surface follow-through landed: `vault-worker/src/intelligence.rs` is now down to `124` lines after moving route-level and section-level passthroughs into `intelligence/{route_queries,section_queries}.rs`. The worker-side hotspot is no longer a mixed mega-file; further worker cleanup is now optional follow-up rather than the next stop-ship backend risk.
- With the `intelligence/mod.rs` helper-cluster cut now landed, `WORK-BE-B` is effectively complete. The next backend block should shift to the remaining oversized support files (`models/core_intelligence.rs`, `remote.rs`, `site_dictionary.rs`) plus the still-embedded `intelligence/mod.rs` regression suite, rather than revisiting worker or AI boundaries that now already have dedicated owners.

### Slice 7 — Visit taxonomy and support-file cleanup

- Split remaining oversized support files without reopening Tauri command, worker, serde, or Core Intelligence route contracts.
- Treat the old `deterministic` module first because site-dictionary classification consumes its taxonomy, URL normalization, query extraction, and tokenization helpers.
- 2026-04-23 visit-taxonomy slice landed: the old `deterministic` module is now `src-tauri/crates/vault-core/src/visit_taxonomy/{mod,types,url,text,rules,classification,tests}.rs`. The public `crate::visit_taxonomy::*` façade stayed stable, the taxonomy version / rule-pack semantics stayed unchanged, and the largest new owner is `rules.rs` at `535` lines.
- 2026-04-23 site-dictionary slice landed: `src-tauri/crates/vault-core/src/intelligence/site_dictionary.rs` is now `site_dictionary/{mod,types,overrides,search_rules,classification,tests}.rs`. Search-engine rule persistence, user overrides, and per-visit classification now have separate owners, and the largest new owner is `search_rules.rs` at `436` lines.
- 2026-04-23 Core Intelligence DTO slice landed: `src-tauri/crates/vault-core/src/models/core_intelligence.rs` is now `models/core_intelligence/{mod,shared,requests,reads,analytics,overview,exports,tests}.rs`. Request payloads, section envelopes, read rows, advanced analytics, overview batches, and trusted-output bundles now have separate owners, and the largest new owner is `analytics.rs` at `376` lines.
- Next support-file order: `remote.rs`, then the still-embedded `intelligence/mod.rs` regression suite / support-type split.

## Non-negotiable invariants

- Preserve existing Tauri command names, worker CLI commands, top-level serde payloads, and frontend-facing transport semantics unless a slice explicitly funds a breaking change.
- Keep long-running archive/import/intelligence work off the Tauri UI thread.
- Do not let derived-state cleanup or runtime recovery rewrite canonical archive facts.
- Every newly created or fully rewritten module must ship with file-level responsibility docs and declaration-level doc comments.
- Each slice must be independently reviewable and commit-ready; do not batch unrelated backend cleanup together.

## Validation strategy

- During each slice: run the most relevant targeted Rust tests first so refactors fail fast near the touched boundary.
- Before closing a backend slice that changes code: `bun run check && bun run build`.
- If a slice exposes an existing large-data or off-main-thread risk that cannot be fixed inside that slice, record the exact follow-up in `BACKLOG.md` instead of hand-waving it away.
