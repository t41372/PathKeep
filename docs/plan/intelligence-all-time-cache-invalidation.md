# Intelligence All-Time Scope, Preload, And Cache Invalidation

> Status: design note for `WORK-INTEL-SCOPE-A` follow-up work.  
> Date: 2026-04-28  
> Scope: `/intelligence` overview time scopes, staged loading, cache freshness, and large-range performance.

> **2026-06-20 implementation note — all-time default + persistent snapshot.**
> The deeper follow-up's first slice has shipped:
>
> - **Default scope is now `All time`.** `route-state.ts` returns the `all` preset when no `range` is present (`DEFAULT_RANGE_PRESET`). `Month` is no longer the first-load default.
> - **Backend persistent snapshot.** `derived/history-intelligence.sqlite` migration v7 adds `intelligence_overview_snapshots(scope_key, band, fingerprint, payload_json, computed_at)`. `get_intelligence_primary_overview` / `get_intelligence_secondary_overview` detect the all-time scope (sentinel start `1900-01-01`) and, via `intelligence_overview_snapshot::resolve_overview_snapshot`, return the stored payload when the fingerprint matches, otherwise recompute → persist → return. Bounded scopes are unchanged.
> - **Fingerprint = the "did the data change?" key.** It combines the aggregated archive watermark (visit/search-term counts + max ids across profiles), the taxonomy / site-dictionary / search-engine-rule version signatures, and the `core_intelligence_stage_checkpoints` ledger (`MAX(last_run_id)`, `MAX(updated_at)`). So it moves on any import/backup/revert/restore, rule edit, or derived rebuild/clear — exactly the invalidation sources listed below — without wall-clock guessing.
> - **Warm-before-open.** `ShellDataProvider` idle-prewarms the archive-wide all-time overview on app start and whenever `refreshKey` bumps (runtime jobs drain to idle = archive changed), via `preloadAllTimeIntelligenceOverview`. Because the backend snapshot makes an unchanged fingerprint a cheap check, re-warming is cheap; the heavy recompute runs at most once per data change.
> - **Explicit invalidation.** `clear_derived_intelligence_state` and the archive doctor's `invalidate_insight_state` drop the snapshot table (the doctor path rewrites derived tables without moving the canonical watermark, so it must clear explicitly).
> - **Still open:** per-profile-scope preload (only archive-wide is prewarmed today), section-level stale-while-revalidate chrome, bounded eviction of the persisted snapshot, and the all-time aggregation profiling artifact.

## Problem

`/intelligence` currently works through concrete `DateRange` payloads. That keeps the frontend/backend command contract simple, but the cost of each section grows with the selected range. A month of current-host history is already visibly delayed; an all-time view can span years of browser records.

The immediate slice adds an `All time` preset without changing the Tauri payload shape. The route emits `?range=all`, maps it to a broad concrete window (`1900-01-01` through today), keeps `Month` as first-load default, and trims the visible `Browsing Rhythm` calendar to the occupied date span so the UI does not render a century of empty cells.

The deeper follow-up should optimize three separate things:

1. first paint latency for large scopes
2. revisit latency for already-viewed scopes
3. invalidation correctness after archive mutations

## Current Slice

- The time selector now includes `Day / Week / Month / Quarter / Year / All time / Custom`.
- `?range=all` is a route preset, not a custom `start/end` link.
- Backend command payloads remain `DateRange` based in this slice.
- Secondary cards no longer wait on one all-or-nothing `secondaryReady` gate if their cache is already warm.
- The secondary overview batch remains the warm path; the UI does not fan out many foreground section IPC calls on cold load.

## Scope Keys

Cache keys should explicitly include every user-visible axis that can change a result:

```text
intelligence:v1:{preset}:{start}:{end}:{profile-scope}:{focus?}:{section}:{params-hash}
```

Important notes:

- `preset=all` should stay distinct from `preset=custom&start=1900-01-01...` because the user intent and eviction priority differ.
- `profile-scope` must distinguish archive-wide from explicit `profileId` and inherited shell profile scope.
- Section-specific parameters, such as top-sites sort or path-flow limits, belong in `params-hash`.
- A future backend archive watermark should join this key so stale cache can be rejected without recomputing every dirty-range intersection in the frontend.

## Stale-While-Revalidate

The preferred route behavior is:

1. render warm cached sections immediately
2. mark section metadata as stale if the archive watermark moved
3. revalidate in the background
4. replace each section independently as fresh data returns

This is different from one page-level `ready` flag. The page should tolerate mixed freshness: a cached `Top Sites` card can be visible while `Path Flows` is still skeletonized, and the evidence/freshness chrome should tell the truth for each section.

## Preload Policy

Preloading needs bounded intent:

- `Month` remains the default first-load scope.
- Do not eagerly preload `All time` on app start or route entry; it is user-triggered.
- After a month scope settles, it is reasonable to idle-prewarm adjacent `Week / Quarter / Year` metadata if CPU and queue state are idle.
- For `All time`, preload only after explicit selection, and prefer section ordering: digest/top sites/search activity/browsing rhythm first, then secondary overview.
- Hidden tab data should stay idle-prefetched only after visible content is stable.

## Invalidation Sources

Any operation that changes canonical visits, source profiles, or derived intelligence can make a cached section wrong:

- Browser Direct import
- Google Takeout import
- manual backup ingest
- scheduled backup ingest
- import batch revert
- import batch restore
- archive repair / doctor workflows that rewrite canonical rows
- derived-state clear or rebuild
- search-engine rule changes that affect query families and search trails
- profile selection changes that alter which profiles are archived

The backend already tracks rebuild/runtime state. The follow-up should expose enough invalidation metadata for the frontend to know which scopes are stale without guessing from wall-clock time.

## Dirty Date Ranges

Import and backup flows should produce dirty date intervals per affected profile:

```text
profileId: chrome:Default
dirtyDateRange: 2026-01-03..2026-04-28
operationId: import-batch-42
archiveWatermark: 1087
```

For small ranges, intersect section cache windows with dirty windows and revalidate only affected keys. For `All time`, any dirty range in the archive can stale the all-time scope, but sections should still refresh independently rather than blocking the whole page.

Open question for backend follow-up: whether dirty ranges should be persisted as a compact ledger table or derived from import batch/source profile records plus intelligence runtime checkpoints.

## All-Time Aggregation Risk

All-time is not just a longer month:

- trend and rhythm sections can cover very sparse history across many years
- top domains may be dominated by ancient behavior unless freshness metadata is obvious
- refind/habit/path-flow sections can change meaning when older archives are restored
- current broad `DateRange` mapping is transport-compatible but not semantically ideal

Longer term, the backend should expose archive min/max local dates and may add all-time-aware aggregate endpoints. That should be done only after profiling shows which sections are actually bottlenecked.

## Eviction

Cache eviction should prefer user-perceived value:

- Keep the current route scope and last few route scopes.
- Keep month/week/year before all-time if memory pressure is high.
- Evict section payloads independently; do not force a whole overview cache eviction when only one large section is heavy.
- Cap all-time secondary payloads more aggressively than current-month primary payloads.

## Storage Engine Boundary

Do not jump to DuckDB, a second analytical store, or another storage swap based on route latency alone. First profile:

- backend query plan and indexes
- SQLite connection reuse / attach overhead
- JSON serialization cost
- IPC transfer size
- frontend render and layout cost
- cache hit rate after route revisit

Only if the current SQLite/read path remains the bottleneck after those measurements should a storage-engine change become a formal architecture decision.

## Follow-Up Acceptance

A deeper implementation slice should deliver:

- section-level stale/fresh state in the UI
- backend or shared-runtime archive watermark exposed to cache keys
- dirty date/profile invalidation after import, backup, revert, and restore
- bounded all-time cache eviction
- profiling artifact for at least month/year/all-time scopes before proposing storage changes
