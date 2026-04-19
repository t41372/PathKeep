# Core Intelligence Handoff

> **Date:** 2026-04-19
> **Audience:** frontend implementer, next backend implementer  
> **Status:** current handoff after desktop truth repair follow-up

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
- 2026-04-17 frontend finish-line follow-up: `/intelligence` now includes a compact runtime digest that matches Jobs / sidebar queue grammar, Dashboard CTAs and repo-wide browser-preview/product-flow tests now point at `/intelligence`, and the remaining external-output surface has moved into a manual Settings review/copy-export panel instead of pretending full host integrations already exist
- 2026-04-18 evidence follow-up: `/intelligence` and `/intelligence/domain/:domain` now also ship a shared evidence / freshness drawer backed by a typed section envelope. Each section can expose generated-at, active scope / window, owning modules, source tables, enrichment participation, and stale / disabled / degraded reason without growing a second mutation control tower.
- 2026-04-18 host follow-up: Settings external outputs now also ship the first reusable trusted local host, `browser-snippet-v1`. The app can preview, build, and verify a fixed `index.html` + `bundle.json` artifact under `app_root/integrations/core-intelligence/browser-snippet-v1/`, and that host reuses the same embed/widget/public payload-provider results as the manual baseline.
- 2026-04-18 app truth-gate follow-up: the repo also absorbed a post-closeout repair pass for real-app blockers — section-envelope camel/snake drift, `daily-rollup` duplicate domain-day rows, encrypted onboarding without keychain persistence, and several product-truth issues (mixed copy, default React Router error page, explorer title redaction, dashboard/sidebar queue drift). Those fixes are in source and covered by targeted regressions, so a fresh agent should treat them as already landed rather than reopen them as TODOs.
- `bun run check` and `bun run build` were green at handoff time
- 2026-04-18 desktop truth repair follow-up: source also landed the front-end shipped-truth repairs for archive-wide callout copy, `category_community`, external-output CTA wording, Explorer visible URL redaction, domain deep-dive decoded page paths, and `/intelligence` runtime digest dependency narrowing (`load_intelligence_runtime` only; no new Tauri commands, no schema change)
- 2026-04-19 calendar heatmap follow-up: `Browsing Rhythm` no longer uses the old weekday × hour main chart. The main card now renders a real-date calendar heatmap backed by `getDiscoveryTrend(..., 'day')`, and the selected-day detail alone uses `getBrowsingRhythm(singleDayRange)` for the hourly strip. The same pass also moved `Search Activity` + `Activity Mix` into a shared half-width row and enforced capped-scroll bodies across Intelligence cards.
- 2026-04-18 locked-archive/bootstrap follow-up: source now also degrades locked encrypted `app_snapshot` reads into a usable shell snapshot, makes the Security unlock flow validate candidate keys before full shell refresh, stops sidebar background polling until the archive is unlocked, and shows compact `version · short-sha[+]` build labels in shell/onboarding/lock chrome

What is **not** done, plus the latest backend truth:

- legacy `vault-core::insights` has been deleted from the repo; readable-content helpers and queued enrichment ownership now live under `enrichment` / `intelligence`, and new code must not reintroduce snapshot-era contracts
- only the first trusted local host is delivered so far: `browser-snippet-v1` now exists as a reusable local artifact, but OS widget install, localhost host/API, public API, and other alternate hosts are still not delivered
- 2026-04-18 contract follow-up note: app snapshot / worker runtime readiness now treat `intelligenceStatus` / `IntelligenceStatus` as the only accepted naming; the repo no longer ships `insightStatus` / `InsightStatus` aliases. `src/lib/core-intelligence/{types,api}.ts` still include typed payload-provider wrappers for embed / widget / public snapshot commands, but this does **not** mean deeper host integration is done.
- 2026-04-17 backend finish-line closeout: append-only `visit-derive` / `daily-rollup` / `structural-rebuild` now persist per-profile `core_intelligence_stage_checkpoints`, structural stage profile aggregates batch-scan search events / derived visits, `visit-derive` / `daily-rollup` full-fallback paths are chunked, and the benchmark harness supports `--persist-app-root`, `--app-root`, `--session-key`, and `--skip-baseline-rebuild` for replayable synthetic plus existing-archive scenarios
- 2026-04-17 signoff note: corrected artifacts now exist at `artifacts/benchmarks/2026-04-17-intelligence-signoff/{full-2k-smoke-signoff,full-1m-60y-signoff,full-10m-60y-signoff,expired-lease-recovery-10m-signoff,real-replay-signoff}.json`. `stageTimingsMs` now sum across all profiles, the durable `10m-signoff` root completed a rebuild-only replay at about `2,078,480 ms` baseline rebuild / `1,250 ms` query surfaces / `1.44 GiB` peak RSS, and the disposable encrypted app-root replay completed at about `373 ms` query surfaces / `44.1 MiB` peak RSS with the stored command shape redacting `--session-key` as `<redacted>`
- 2026-04-18 long-horizon signoff note: `artifacts/benchmarks/2026-04-18-intelligence-long-horizon-signoff/{full-14_4m-60y-signoff,expired-lease-recovery-14_4m-signoff}.json` now close the current-host `14.4M / 60y` envelope. Full replay measured about `4,758,160 ms` baseline rebuild / `8,969 ms` query surfaces / `1.74 GiB` peak RSS; expired-lease replay measured about `2,013 ms` query surfaces / `598.6 MiB` peak RSS while recovering the queued lease and leaving the cancelled lease untouched.
- host verification caveat: on this machine, Computer Use can still end up showing stale frontend behavior even when `devUrl` is already serving the repaired modules. In practice that meant raw `intelligence.archiveWideBadge` / `archiveWideBody`, the old external-output CTA copy, and the old `/intelligence` queue digest behavior were still visible in the desktop app while `http://127.0.0.1:1420/src/pages/intelligence/{index,sections,domain-deep-dive,copy}.tsx` already exposed the repaired source. Treat that as current-host WebView / stale bundle cache drift, not as current source truth. If someone needs fresh desktop screenshots, rebuild / relaunch and invalidate the stale desktop bundle before reopening a frontend TODO.
- latest restart caveat: even after a fresh `bun run desktop:dev` relaunch, this host still rendered the old generic locked-archive dashboard shell and omitted the new short-SHA build label from visible chrome, while the worker logs already reflected encrypted-archive warnings from the new build. Treat that the same way: host-specific stale WebView / bundle cache drift until proven otherwise.
- `PG-RD-AI-011`, `WORK-CI-B`, and `WORK-CI-C` are now closed. If we later want second-machine benchmark parity, that should be an explicitly re-scoped follow-up rather than a queued continuation of this cleanup block.
- after this follow-up, the original deterministic Core Intelligence P1–P4 scope should be treated as complete. The only original-scope work still genuinely open is external host integration beyond `browser-snippet-v1` (OS widget install, localhost host/API, public API, alternate hosts).

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
- [`src/components/intelligence/section-meta.tsx`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/components/intelligence/section-meta.tsx)

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
- `preview_intelligence_local_host`
- `build_intelligence_local_host`

For `/intelligence` and `/intelligence/domain/:domain`, the route-facing commands above now return a section envelope with `data + meta`; only runtime snapshot commands, external-output payload providers, and local-host preview/build commands stay on their existing non-envelope shapes.

### Important Frontend Caveat

The command surface above is implemented, but the frontend should still assume:

- PathKeep now ships one reusable trusted local host, `browser-snippet-v1`, but the broader P4 host family is still unavailable: no OS widget install, localhost host/API, public API, or alternate local hosts yet
- `observed interactions` is capability-gated and may legitimately return an empty list on archives without supported source evidence
- if future work re-opens `embed/widget/public snapshot`, treat the current Settings panel plus `browser-snippet-v1` local artifact as the existing baseline; do not reopen them as if no host integration exists yet

### Frontend Testing Note

App shell tests and browser-preview shell e2e now expect `/intelligence` instead of `/insights`.

Relevant file:

- [`src/app/index.test.tsx`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/app/index.test.tsx)
- [`tests/e2e/shell.spec.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/tests/e2e/shell.spec.ts)

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
- typed `preview_intelligence_local_host` / `build_intelligence_local_host` commands that materialize the `browser-snippet-v1` bundle
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

There are **two kinds** of future backend work.

#### 1. New backlog items, not `WORK-CI-C`

The next backend owner is **not** starting from P1/P2 anymore. `WORK-CI-C` is closed, so any continuation should be framed as a fresh block such as:

- any remaining P4 host/service integrations beyond the current payload-provider commands plus `browser-snippet-v1`
- future performance / operational refinement that is explicitly reopened by docs first, rather than inferred from historical finish-line notes

Second-machine benchmark parity is **not** part of the current residual scope. Only reopen it if a future request explicitly re-scopes validation beyond the current-host signoff.

#### 2. Do not mistake archive notes for living scope

This handoff assumes the repo truth is whatever is actually committed plus the source docs in this folder. Do not invent extra “probably-finished” intelligence scope beyond what the committed tree and updated docs can prove.

### Do Not Accidentally Regress These

- Do not reintroduce `load_insights()` as the accepted deterministic read path
- Do not reintroduce `/insights` as the accepted route name
- Do not remove `visit_content_enrichments` unless you also replace the readable-text evidence path for AI/enrichment
- Do not confuse “typed payload-provider wrappers exist” with “P4 host/service integrations are done”; CI-H still owns the actual consumer / embed / widget surface

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
