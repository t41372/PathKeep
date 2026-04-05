# Browser History Insights V1 Plan

## Goal

Ship the first real insights layer for the local browser history archive so the app can do more than storage and retrieval.

V1 is intentionally:

- Chromium-first for advanced signals
- evidence-first instead of LLM-first
- embedding-compatible with the existing dense-vector pipeline
- local-friendly and incremental

The target user-facing outcomes are:

- topic timelines with rising/cooling signals
- task/thread detection with reopen and open-loop scoring
- revisit-based importance and resurfacing
- query reformulation ladders
- source-role workflow maps
- faceted workstyle/profile cards

## Full Implementation Plan

### 1. Data model and schema

Add derived-analysis tables on top of the existing archive:

- `visit_content_enrichments`
  - store readable-text enrichment results
  - preserve provenance via `content_source`
  - allow future `capture` rows to override `refetch`
- `visit_insight_features`
  - store normalized per-visit features used by analytics and UI
- `insight_topics`
  - store aggregate topic summaries by profile scope and window
- `insight_threads` and `insight_thread_members`
  - persist reconstructed research threads and ordered evidence visits
- `insight_cards`
  - precompute high-signal cards for the UI
- `insight_runs`
  - track run status, coverage, counts, warnings, and notes

### 2. Content acquisition strategy

Use a hybrid content strategy:

- V1 ships refetch-based readable-text enrichment
- future browse-time capture can be inserted later as `content_source = capture`
- embedding input should prefer enriched readable content and fall back to metadata-only strings

### 3. Deterministic analytics pipeline

Use a local-friendly pipeline instead of heavy always-on LLM analysis:

1. Load visits for a profile/window
2. Hydrate Chromium search terms and chain signals
3. Refetch eligible pages in the background
4. Extract readable text/snippets
5. Load dense vectors from the existing embedding store
6. Compute per-visit features
7. Build sessions
8. Assign topics
9. Merge sessions into threads
10. Compute cards, ladders, workflow maps, and profile facets
11. Persist aggregates

### 4. Algorithms and heuristics

The V1 backbone should stay deterministic:

- Sessions
  - prefer `from_visit` chains
  - otherwise same-profile visits within 30 minutes
- Threads
  - merge semantically similar adjacent sessions up to a 14-day gap
- Reopen detection
  - count reactivations after 24+ hour gaps
- Open-loop scoring
  - combine revisit frequency, spaced returns, compare signals, and unresolved patterns
- Topic assignment
  - nearest-centroid clustering over existing dense vectors
  - lexical fallback when vectors are absent
- Query ladders
  - broad, narrowing, compare, site-restrict, error-driven
- Source roles
  - search, docs, repo, forum, video, shopping, notes, social, news, general
- Profile facets
  - aggregate metrics only; no freeform personality inference

### 5. API and worker surface

Expose the insight layer through the same Tauri/worker path as the rest of the app:

- `run_insights_now`
- `load_insights`
- `load_thread_detail`
- `explain_insight`

Also add `insight_status` to the main app snapshot so the shell can render status without eagerly loading a full snapshot.

### 6. Frontend experience

Keep the existing analysis area but make it insights-first:

- overview cards and run status
- topics and trend bars
- active threads and evidence drill-down
- query ladders and workflow map
- profile facets and “why this?” explanations
- provider management lower in the page

### 7. Testing and verification

Cover:

- schema initialization and migration
- readable-text precedence rules
- Chromium visit/query/referrer ingestion
- session and thread construction
- topic/card generation on synthetic histories
- worker and Tauri command surface
- TypeScript/backend facade and browser-preview fixtures
- frontend rendering and empty/degraded states

## Current Progress

Status legend:

- `Done`: implemented and verified
- `Partial`: implemented in a basic form, but still intentionally conservative
- `Deferred`: intentionally out of V1 scope

### Backend and pipeline

- `Done` Added insight schema and derived tables in `vault-core`
- `Done` Added `InsightStatus`, snapshots, cards, topics, threads, ladders, workflow map, facets, and explanation types
- `Done` Added `insights.rs` pipeline with:
  - visit loading
  - Chromium term hydration
  - refetch-based readable-text enrichment
  - keyword/entity extraction
  - feature scoring
  - session construction
  - topic clustering
  - thread construction
  - card generation
  - workflow/facet generation
- `Done` Hooked archive initialization so insight tables are created automatically
- `Done` Updated embedding input selection so enriched readable content is preferred over metadata-only strings
- `Done` Wired backup flow to refresh insights after backup

### Worker and app bridge

- `Done` Added worker functions for running/loading/explaining insights
- `Done` Added Tauri command handlers for the new insight APIs
- `Done` Added `insight_status` to the app snapshot path

### Frontend integration

- `Done` The current repo already includes frontend insight types, backend facade bindings, context wiring, and an insights page structure
- `Partial` This commit mainly finalizes and stabilizes the Rust/backend side that those frontend surfaces depend on

### Verification

- `Done` `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets`
- `Done` `bun run typecheck`
- `Done` `bun run test:unit`

## Known V1 Limits

- `Partial` Explanations are still deterministic/evidence-based rather than full provider-backed LLM narrative generation
- `Partial` Topic/thread labels are heuristic rather than LLM-renamed
- `Partial` Refetch enrichment is best-effort and does not yet use browse-time capture
- `Deferred` Personality-style portraiting
- `Deferred` Sensitive inference
- `Deferred` missing-link ontology reasoning
- `Deferred` sparse or multi-vector retrieval
- `Deferred` graph-heavy curiosity map UI

## Recommended Next Steps

### Near-term

- add provider-backed optional topic/thread naming
- add richer contrastive summaries for 7d/30d windows
- surface insight run controls and status more prominently in the UI
- add more synthetic tests around topic drift and reopen scoring

### Medium-term

- add `capture` content-source ingestion
- add incremental rerun optimization based on new visits only
- support lighter non-Chromium insight subsets
- add export/share format for insight summaries

### Long-term

- provider-backed weekly/monthly narrative digests
- richer temporal compare views
- curiosity graph and bridge-concept exploration
- hybrid retrieval upgrades once sparse/multi-vector storage is worth adding
