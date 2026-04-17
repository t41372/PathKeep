# Core Intelligence Handoff

> **Date:** 2026-04-17  
> **Audience:** frontend implementer, next backend implementer  
> **Status:** current handoff after the 2026-04-17 progress audit

---

## What This File Is For

This is the quickest way for a new agent to resume Core Intelligence work without replaying chat history.

Use this together with:

1. [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)  
   Accepted product/design baseline
2. [`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)  
   Current planning-side completion matrix and remaining work
3. [`docs/plan/CHANGELOG.md`](CHANGELOG.md)  
   Search for `WORK-QC-T`
4. [`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)  
   Transition truth, especially what is still legacy

---

## Executive Summary

The hard cutover from legacy deterministic `insights` to **Core Intelligence** is done.

That means:

- the main deterministic backend contract no longer relies on `run_insights_now`, `load_insights`, `load_thread_detail`, or `explain_insight`
- `derived/history-intelligence.sqlite` now boots Core Intelligence tables and drops legacy `insight_*` tables during bootstrap
- worker orchestration, Tauri commands, desktop bridge, runtime read model, and backup/import follow-up rebuild flow now point at Core Intelligence
- the backend ships Phase 1 / Phase 2 query APIs **and** the planned deterministic Phase 3 / Phase 4 query APIs
- the frontend already ships more than the original P1/P2 delegation assumed: `/intelligence`, `/intelligence/domain/:domain`, Explorer session/trail grouping, navigation tracer, Jobs / Settings runtime controls, and most deterministic overview/detail sections already exist in-repo
- `bun run check` and `bun run build` were green at handoff time

What is **not** done:

- legacy `vault-core::insights` code still exists in the repo for supporting enrichment-related paths and helper reuse, but it is now crate-internal rather than part of the accepted public backend contract
- external snippet / embed / widget host integrations from Phase 4 are still not delivered; the backend only ships data-provider payloads now
- large-archive / low-RAM / queue-recovery signoff remains open under `PG-RD-AI-011`
- 2026-04-17 follow-up: append-only `visit-derive` / `daily-rollup` / `structural-rebuild` now persist per-profile `core_intelligence_stage_checkpoints` and emit `executionMode` / `dirtyVisitCount` / `dirtyDateKeys` / `fallbackReason` runtime metadata; structural stage profile aggregates now batch-scan search events / derived visits instead of materializing whole-profile aggregate inputs; `visit-derive` / `daily-rollup` full-fallback paths are also chunked now instead of materializing a whole-profile `Vec`; benchmark artifacts record corpus stats, peak RSS, low-RAM fallback timings, and expired-lease recovery evidence. This is still a finish-line follow-up, not final 10M / low-RAM signoff
- benchmark harness note: `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs` now supports disposable existing-archive replay via `--app-root` / `--session-key`, and `artifacts/benchmarks/2026-04-17-intelligence-finish-line/README.md` is the source of truth for the current synthetic/manual replay contract. On this host, the real app root is encrypted and no benchmark session key was available, so real-replay evidence is still missing
- 2026-04-17 signoff attempt note: queued enrichment execution now lives in `enrichment.rs` instead of `insights.rs`, and the benchmark harness also supports `--persist-app-root` for reusable synthetic corpora. Large-host `10M / 60y` attempts first exposed `daily-rollup` full fallback, then structural `build_sessions`; the repo now contains SQLite-side daily-rollup aggregation, prepared `visit_derived_facts` persistence, no extra structural `tail_visits.clone()`, and one-pass session aggregate building. Those fixes materially improved the large-host shape, but release `10M` persisted-root attempts still did not emit a final JSON artifact after >30 minutes on this machine, and real-replay remains blocked by encrypted-archive key access. See `artifacts/benchmarks/2026-04-17-intelligence-signoff/README.md`.
- 2026-04-17 post-streaming note: structural tail rebuild no longer materializes the whole tail into memory; it now streams batches, range-clears affected memberships, and rebuilds `source_effectiveness` from batched trail reads. The harness also supports `--skip-baseline-rebuild` for existing `--app-root` follow-up scenarios and refuses that flag unless the replay target already has a Core Intelligence read model. `full-1m-60y-post-streaming.json` now exists with about `383s` full rebuild / `403ms` query surfaces / `118 MiB` peak RSS. Release `10M / 60y` still failed to emit a final JSON artifact after >`31m`, but the process RSS stayed around `440 MiB`, so the active blocker has moved from structural memory blow-up to wall-clock completion plus missing real-archive session-key replay evidence.

---

## For Frontend

### What You Can Treat As Stable Now

The accepted route/product direction is now:

- `/intelligence` replaces `/insights`
- `Core Intelligence` is the deterministic product surface

The frontend IPC draft lives here:

- [`src/lib/core-intelligence/types.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/types.ts)
- [`src/lib/core-intelligence/api.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/api.ts)
- [`src/lib/core-intelligence/hooks.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/hooks.ts)
- [`src/pages/intelligence/index.tsx`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/pages/intelligence/index.tsx)

The backend command surface that is implemented and safe to wire now is:

- `run_core_intelligence_now`
- `queue_core_intelligence_rebuild`
- `load_intelligence_runtime`
- `retry_intelligence_job`
- `cancel_intelligence_job`
- `get_sessions`
- `get_session_detail`
- `get_search_trails`
- `get_trail_detail`
- `get_navigation_path`
- `get_hub_pages`
- `get_search_engine_ranking`
- `get_top_search_concepts`
- `get_query_families`
- `get_top_sites`
- `get_domain_trend`
- `get_refind_pages`
- `explain_refind`
- `explain_entity`
- `get_activity_mix`
- `get_activity_mix_trend`
- `get_digest_summary`
- `get_stable_sources`
- `get_search_effectiveness`
- `get_friction_signals`
- `get_reopened_investigations`
- `get_domain_deep_dive`
- `get_browsing_rhythm`
- `get_discovery_trend`
- `get_on_this_day`
- `get_breadth_index`
- `get_habit_patterns`
- `get_interrupted_habits`
- `get_path_flows`
- `get_observed_interactions`
- `get_compare_sets`
- `get_multi_browser_diff`
- `get_intelligence_embed_cards`
- `get_intelligence_widget_snapshot`
- `get_intelligence_public_snapshot`

### Important Frontend Caveat

The command surface above is implemented, but the frontend should still assume:

- P4 external snippet/embed hosts are not available yet; only backend payload providers are shipping
- `observed interactions` is capability-gated and may legitimately return an empty list on archives without supported source evidence
- a few tests / copy / route references in the repo still say `Insights` or `/insights`; treat that as cleanup work, not as the accepted product contract

### Frontend Testing Note

App shell tests were already updated to expect `/intelligence` instead of `/insights`, but repo-wide cleanup is not fully finished yet.

Relevant file:

- [`src/app/index.test.tsx`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/app/index.test.tsx)

---

## For The Next Backend Implementer

### What Was Added

New backend ownership root:

- [`src-tauri/crates/vault-core/src/intelligence/mod.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/intelligence/mod.rs)
- [`src-tauri/crates/vault-core/src/intelligence_catalog.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/intelligence_catalog.rs)
- [`src-tauri/crates/vault-core/src/intelligence/site_dictionary.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/intelligence/site_dictionary.rs)
- [`src-tauri/crates/vault-core/src/models/core_intelligence.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/models/core_intelligence.rs)

Key supporting rewires:

- [`src-tauri/crates/vault-core/src/intelligence_runtime.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/intelligence_runtime.rs)
- [`src-tauri/crates/vault-core/src/archive/intelligence_projection.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/archive/intelligence_projection.rs)
- [`src-tauri/crates/vault-core/src/archive/doctor.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core/src/archive/doctor.rs)
- [`src-tauri/crates/vault-worker/src/intelligence.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-worker/src/intelligence.rs)
- [`src-tauri/src/commands/intelligence.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/src/commands/intelligence.rs)
- [`src-tauri/src/worker_bridge/intelligence.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/src/worker_bridge/intelligence.rs)
- [`src-tauri/src/dev_ipc_bridge.rs`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/src/dev_ipc_bridge.rs)

### Current Schema Truth

Core Intelligence bootstrap now creates these tables:

- `visit_content_enrichments`
- `visit_derived_facts`
- `domain_daily_rollups`
- `category_daily_rollups`
- `engine_daily_rollups`
- `daily_summary_rollups`
- `sessions`
- `search_trails`
- `search_trail_members`
- `search_events`
- `search_event_terms`
- `query_families`
- `refind_pages`
- `source_effectiveness`
- `habit_patterns`
- `reopened_investigations`
- `path_flows`

Legacy `insight_*` tables are dropped during Core Intelligence bootstrap.

### What Is Actually Done

The backend already delivers:

- trait-backed Core Intelligence module registry with `RebuildMode`, settings-schema descriptors, module dependency resolution, and explainability ownership
- site dictionary / URL normalization wrapper
- persisted site-dictionary overrides + versioned intelligence-side schema bootstrap
- visit-derived facts materialization
- daily rollups
- sessions
- search events
- query families
- search trails
- navigation path
- hub pages
- refind pages
- source effectiveness
- activity mix
- digest summary
- stable sources
- search effectiveness
- friction signals
- reopened investigations
- domain deep dive
- browsing rhythm
- discovery trend
- on this day
- breadth index
- habit patterns
- interrupted habits
- path flows
- observed interactions
- compare sets
- multi-browser diff
- generic `explain_entity`
- read-only embed / widget / public snapshot payload providers
- staged queue semantics for `visit-derive`, `daily-rollup`, `structural-rebuild`, and `full-rebuild`
- profile-scoped auto-enqueue for backup/import follow-up rebuilds
- per-profile stage checkpoints plus append-only incremental execution for `visit-derive`, `daily-rollup`, and `structural-rebuild`
- `path_flows` end-to-end 4-step support
- replayable incremental benchmark scenarios under `artifacts/benchmarks/2026-04-17-intelligence-incremental-foundation/`
- replayable finish-line benchmark scenarios under `artifacts/benchmarks/2026-04-17-intelligence-finish-line/`, including low-RAM fallback and expired-lease recovery
- replayable signoff artifacts under `artifacts/benchmarks/2026-04-17-intelligence-signoff/`, including post-streaming `2k` smoke and `1M / 60y` full rebuild evidence
- structural stage aggregate passes that batch-scan `search_events` / `visit_derived_facts` for query-family / refind / habit / path-flow rebuild inputs instead of loading extra whole-profile aggregate Vecs
- chunked `visit-derive` / `daily-rollup` full-fallback paths that no longer materialize an entire profile before persisting fallback output
- structural tail rebuild streaming that keeps session/trail assignment stable across batch boundaries and avoids giant in-memory tail buffers
- queued enrichment execution owned by `enrichment.rs` instead of legacy `insights.rs`
- persisted synthetic benchmark roots via `--persist-app-root`, so large-host scenarios can reuse one seeded app root

### What Is Still Left

There are **three kinds** of remaining backend work.

#### 1. Finish the remaining real backend scope

The next backend owner is **not** starting from P1/P2 anymore. The current remaining scope is:

- `PG-RD-AI-011` large-archive / low-RAM / queue-recovery signoff
- larger-host queue-recovery RSS proof plus `10M / 14.4M` synthetic evidence
- real-archive replay evidence from a disposable app-root copy with a valid session key
- any remaining P4 host/service integrations beyond the new payload-provider commands
- latest blocker detail: the first obvious `10M` bottleneck is no longer daily-rollup fallback or the old structural RSS cliff; the remaining heavy path is wall-clock structural/full-run completion plus the missing session-key-backed real replay

#### 2. Finish the cutover cleanup

The reset is working, but there is still technical cleanup to do:

- remove or reduce remaining legacy `vault-core::insights` implementation code
- keep only the parts still genuinely needed for enrichment/readable-content support
- continue moving shared readable-content helpers toward `enrichment` instead of leaving them in `insights`
- decide whether the staged queue should become more incremental than the current “stage-specific tables, mostly profile-scoped recompute” implementation

#### 3. Do not mistake local WIP for completed scope

There are currently uncommitted intelligence changes in the worktree. Read them if they help, but do **not** mark them as done or update docs as if they were shipped until they land with tests and source-doc updates.

### Do Not Accidentally Regress These

- Do not reintroduce `load_insights()` as the accepted deterministic read path
- Do not reintroduce `/insights` as the accepted route name
- Do not remove `visit_content_enrichments` unless you also replace the readable-text evidence path for AI/enrichment
- Do not treat `src/lib/core-intelligence/types.ts` as fully implemented; the backend now ships `explain_entity` plus data-provider payload commands, but P4 host/service integrations are still draft-only

### Validation Commands

These were green at handoff time:

```bash
bun run check
bun run build
```

If you change backend Core Intelligence again, these are the minimum honest gates:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
bun run check
bun run build
```

---

## Existing Docs: Are They Enough On Their Own?

For a frontend implementer: **yes, if they read the progress doc too**.  
If they read:

- [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)
- [`docs/plan/core-intelligence-progress.md`](core-intelligence-progress.md)
- [`src/lib/core-intelligence/types.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/types.ts)
- [`src/lib/core-intelligence/api.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/api.ts)

they can move pretty far.

For a new backend implementer: **yes, if they read the progress doc and this handoff together**.  
Without them, they are likely to miss:

- that repo reality has moved beyond the original “backend P1/P2 only” assumption
- that most frontend deterministic surfaces already exist, so remaining work is finish-line cleanup rather than blank-page implementation
- that `visit_content_enrichments` was intentionally preserved
- that queue/job-type granularity and large-archive signoff are still unfinished
- that local worktree diffs are currently WIP and not accepted truth

So if you want a brain-empty agent to continue backend or frontend work, send them to `core-intelligence-progress.md` first, then this file.

---

## One-Line Resume Prompt For The Next Backend Agent

If you want to resume with a fresh agent, this prompt is good enough:

> Read `docs/plan/core-intelligence-progress.md` and `docs/plan/core-intelligence-handoff.md`, then continue the remaining Core Intelligence work for the requested side (frontend or backend), focusing on finish-line cleanup, large-archive/runtime truth, host-output gaps, and legacy-insights cleanup without reintroducing `load_insights` as the product contract.
