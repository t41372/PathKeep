# Core Intelligence Handoff

> **Date:** 2026-04-15  
> **Audience:** frontend implementer, next backend implementer  
> **Status:** current handoff after the post-`WORK-QC-T` backend/frontend follow-up

---

## What This File Is For

This is the quickest way for a new agent to resume Core Intelligence work without replaying chat history.

Use this together with:

1. [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)  
   Accepted product/design baseline
2. [`docs/plan/CHANGELOG.md`](CHANGELOG.md)  
   Search for `WORK-QC-T`
3. [`docs/features/intelligence-current-state.md`](../features/intelligence-current-state.md)  
   Transition truth, especially what is still legacy

---

## Executive Summary

The backend hard cutover from legacy deterministic `insights` to **Core Intelligence** is done, and the deterministic shipping surface now covers **Phase 1 + Phase 2 + the planned Phase 3 / Phase 4 backend query APIs**.

That means:

- the main deterministic backend contract no longer relies on `run_insights_now`, `load_insights`, `load_thread_detail`, or `explain_insight`
- `derived/history-intelligence.sqlite` now boots Core Intelligence tables and drops legacy `insight_*` tables during bootstrap
- worker orchestration, Tauri commands, desktop bridge, runtime read model, and backup/import follow-up rebuild flow now point at Core Intelligence
- `bun run check` and `bun run build` both pass after the cutover

What is **not** done:

- legacy `vault-core::insights` code still exists in the repo for supporting enrichment-related paths and helper reuse, but it is now crate-internal rather than part of the accepted public backend contract
- external snippet / embed / widget host integrations from Phase 4 are still not delivered; the backend only ships data-provider payloads now

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

### Frontend Testing Note

App shell tests were already updated to expect `/intelligence` instead of `/insights`.

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

### What Is Still Left

There are **two kinds** of remaining backend work.

#### 1. Finish Core Intelligence roadmap work

The next backend owner is expected to finish the rest of **all backend Core Intelligence**, which means Phase 3 / Phase 4 still remain.

These are still pending as real shipping backend scope:

- any remaining P4 host/service integrations beyond the new payload-provider commands

#### 2. Finish the cutover cleanup

The reset is working, but there is still technical cleanup to do:

- remove or reduce remaining legacy `vault-core::insights` implementation code
- keep only the parts still genuinely needed for enrichment/readable-content support
- decide whether the staged queue should become more incremental than the current “stage-specific tables, mostly profile-scoped recompute” implementation

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

For a frontend implementer: **almost**.  
If they read:

- [`docs/features/core-intelligence-ultimate-design.md`](../features/core-intelligence-ultimate-design.md)
- [`src/lib/core-intelligence/types.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/types.ts)
- [`src/lib/core-intelligence/api.ts`](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src/lib/core-intelligence/api.ts)

they can move pretty far.

For a new backend implementer: **not quite**.  
Without this handoff, they are likely to miss:

- that `WORK-QC-T` only covered Phase 1 + Phase 2 backend
- that some P3/P4-looking frontend types already exist but are not backed by Rust yet
- that `visit_content_enrichments` was intentionally preserved
- that queue/job-type granularity is still unfinished

So if you want a brain-empty agent to continue backend work, send them to this file first.

---

## One-Line Resume Prompt For The Next Backend Agent

If you want to resume with a fresh agent, this prompt is good enough:

> Read `docs/plan/core-intelligence-handoff.md` and continue finishing all remaining Core Intelligence backend work, especially Phase 3/4 APIs and the remaining legacy-insights/queue cleanup, without reintroducing `load_insights` as the product contract.
