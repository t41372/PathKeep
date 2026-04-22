# Backend Hotspot Decomposition

> User-directed parallel backend track opened on 2026-04-21 while `WORK-M13-B` continues as a separate frontend/reuse stream.

## Why this track exists

- The Rust workspace is no longer suffering from the older `insights.rs` / `vault-worker/src/lib.rs` mega-file shape, but the current backend still has several oversized hotspots that mix orchestration, schema/bootstrap, read models, and long-running job control.
- The most acute risk is no longer "no architecture"; it is concentrated complexity in a few high-churn files that sit on large-data paths.
- This track turns the 2026-04-21 architecture review into an execution plan with explicit sequencing and invariants.

## Current hotspot snapshot

| File                                                  | Current line count | Primary risk                                                                 |
| ----------------------------------------------------- | -----------------: | ---------------------------------------------------------------------------- |
| `src-tauri/crates/vault-core/src/intelligence/mod.rs` |               7703 | Structural rebuild internals and many query/read-model helpers still mixed   |
| `src-tauri/crates/vault-core/src/ai.rs`               |               2116 | Provider validation, indexing, semantic search, assistant, ledger mixed      |
| `src-tauri/crates/vault-worker/src/intelligence.rs`   |               1636 | AI queue execution, deterministic queue execution, query pass-through mixed  |
| `src-tauri/crates/vault-core/src/deterministic.rs`    |               1528 | URL normalization, query extraction, tokenization, taxonomy rules co-located |

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
- 2026-04-22 schema/rebuild follow-up landed: the next cut extracted `intelligence_{schema,schema_sql,rebuild}.rs`, so schema bootstrap, derived-state clear, public rebuild entrypoints, legacy scoped fallback, and runtime-ready module updates no longer live inside `intelligence/mod.rs`. The parent module is now down to `7703` lines, and the remaining hotspot is concentrated around structural rebuild internals plus query/read-model helper clusters rather than top-level orchestration.

### Slice 6 — AI and worker follow-through

- Split `ai.rs`, `ai_queue.rs`, and `vault-worker/src/intelligence.rs` after the archive/intelligence core boundaries are clearer.
- Treat `remote.rs`, `models/core_intelligence.rs`, and dev-only bridge surfaces as follow-up cleanup, not first blockers.

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
