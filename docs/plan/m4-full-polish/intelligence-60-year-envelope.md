# Intelligence 60-Year Support Envelope

> `WORK-QC-D` closeout note. This document records what PathKeep could honestly claim about optional intelligence as of 2026-04-09, and just as importantly, what it still could not claim.
>
> **2026-04-13 reset note:** 這份文件保留的是 pre-reset truth，方便回看當時哪裡卡住。新的 long-horizon target 不再是「把同一個 hot SQLite 繼續修到能撐住」，而是由 `WORK-QC-R` 直接把 storage plane 重構成 canonical/search/intelligence/sidecars 四層結構。

---

## Scope

- Baseline under review:
  - roughly 60 years of accumulated history
  - optional insights and AI surfaces enabled
  - 8 GB RAM
  - 4 CPU cores at roughly 3 GHz
- Important caveat: "`60 years`" is not a data volume by itself. The real pressure comes from visit count, enrichment coverage, embedding count, derived-state byte size, and whole-shell refresh behavior.

---

## Evidence Collected

- Repo quality / release gates remain the closeout acceptance path:
  - `bun run check`
  - `bun run build`
  - `bun run verify`
- 2026-04-09 current rerunnable evidence on this workspace:
  - `bun run build` passes and no longer emits the earlier single-main-chunk warning; shell routes are now code-split
  - `bun run check:js` passes for the living JS surface
  - `bun run perf:artifact:shell` regenerates a checked-in artifact bundle at `artifacts/perf/2026-04-09-large-archive-shell-scaling/`
  - that bundle currently records `580261` approx base-shell bytes, `629465` approx first-route bytes for the heaviest route (`settings`), and a synthetic Explorer FTS query plan that still uses `VIRTUAL TABLE INDEX`
- 2026-04-11 recovery evidence on the primary dev archive:
  - real archive sample on this machine is `64,781` visits across roughly three months of normal use, with about `10,425,513` total URL+title characters (`8,109,160` URL chars + `2,316,353` title chars)
  - observed URL-length distribution already includes extreme outliers (`158,100` chars max), so any "average row size" estimate must keep pathological URLs in mind
  - deterministic rebuild previously looked stuck for minutes because two bugs stacked:
    - interrupted runtime recovery re-queued the same running deterministic job on every runtime load, making progress appear to reset
    - `compute_feature_scores` contained an O(n²) token-set comparison hot path
  - after fixing one-time recovery, deterministic-priority ordering, and replacing the O(n²) feature-scoring path with a linear seen-token-count approach, the same real archive completed a full deterministic rebuild in about 25 seconds on the local dev machine (`started_at = 2026-04-12T03:44:35.638254+00:00`, `finished_at = 2026-04-12T03:45:00.336066+00:00` in `intelligence_jobs`)
  - Jobs/runtime UI now surfaces phase + heartbeat + coarse progress for long-running deterministic rebuilds instead of only a generic running state
- 2026-04-12 replayable deterministic benchmark evidence:
  - `src-tauri/crates/vault-core/examples/intelligence-benchmark.rs` now provides a rerunnable synthetic corpus harness
  - checked artifacts now include:
    - `artifacts/benchmarks/2026-04-12-intelligence-rewrite/100k.json` (`100k` visits, 2-year horizon): `runInsightsMs = 40104`, `loadInsightsMs = 591`
    - `artifacts/benchmarks/2026-04-12-intelligence-rewrite/100k-60y.json` (`100k` visits, 60-year horizon): `runInsightsMs = 1032`, `loadInsightsMs = 91`
    - `artifacts/benchmarks/2026-04-12-intelligence-rewrite/1m-60y.json` (`1,000,000` visits, 60-year horizon): `runInsightsMs = 13052`, `loadInsightsMs = 921`
  - these artifacts specifically exercise deterministic rebuild + snapshot read on synthetic corpora; they still do **not** close queue recovery RSS profiling or 10M-scale signoff
- Pre-reset signoff blockers:
  - `bun run verify` is still the acceptance target for `WORK-M4-J`, and it now reruns green on this machine
  - the remaining blocker is evidence quality, not CI: the checked-in `artifacts/perf/2026-04-09-large-archive-shell-scaling/` bundle is still synthetic and does not replace a true large-profile replay with webview trace plus Rust sampling
- Code / architecture fixes landed during this closeout:
  - semantic retrieval now queries the LanceDB sidecar first and only falls back to the SQLite compatibility mirror with explicit notes
  - SQLite semantic mirror no longer stores JSON vectors; `ensure_ai_schema` migrates legacy `embedding_json` rows to `embedding_blob` and request-path semantic retrieval stays sidecar-first / lexical-only fallback
  - index state now reports `stale` when archive visibility / import watermark or readable-content enrichment freshness diverges from the last semantic build
  - embedding rebuild now batches requests, retries, and tolerates partial failure instead of doing only one-row-at-a-time indexing
  - Settings now shows indexed rows, LanceDB sidecar bytes, SQLite mirror bytes, and estimated embedding token volume
  - Settings now shows MCP / skill consent, capability, scope-boundary, audit-trace copy, plus preview artifact contents instead of only opaque file paths
  - MCP searches now write dedicated `mcp_query` run-ledger entries, and import restore no longer masquerades as `rollback`
  - selected provider/model readiness is now model-scoped, so switching embedding models no longer reuses readiness from a different model
  - deterministic rebuild progress now writes heartbeat / phase / coarse percent into persisted runtime job artifacts, so the app can distinguish "slow but advancing" from "possibly stalled"
  - deterministic feature scoring no longer compares every visit against every prior token set; current shipped path is linear in visit count for that stage
  - deterministic pipeline no longer does per-visit enrichment N+1 lookups or whole-table search-term / feature / embedding scans for the current window; those joins are now bounded to the current visit set, and thread merge now maintains incremental token/domain/anchor accumulators instead of rebuilding full unions on every comparison
- Missing evidence that still matters:
  - there is now a checked-in perf artifact bundle, but it is only a shell-scaling / synthetic SQLite query-plan bundle with placeholder trace/sample files, not a real large-profile replay with actual webview trace + Rust CPU sample
  - there is still no synthetic long-horizon benchmark that exercises semantic search, assistant retrieval, insights rebuild, and shell responsiveness under a defined 60-year corpus shape
  - there is still no benchmark artifact for the more realistic "incremental 60-year archive grown over time, plus occasional 20-year one-shot import" shape the product actually needs to tolerate
  - exploratory whole-workspace JS mutation still shows concentrated survivors in `src/app/shell-data.tsx` and `src/lib/backend.ts`; that debt does not fail the current gate, but it is still evidence that shell-state / preview-state complexity needs another hardening pass before we advertise long-horizon smoothness

---

## Current Shipped Boundary

- Semantic search:
  - supported as optional derived state
  - truthful states now include `disabled`, `blocked`, `empty`, `queued`, `paused`, `rebuilding`, `failed`, `stale`, `ready`, and `degraded`
  - `stale` means "manual rebuild required", not "results are silently fresh"
- Assistant:
  - remains evidence-first and queue-backed
  - queued requests keep their enqueue-time provider snapshot
  - running cancel is now cooperative: the queue records a stop request, the worker exits at the next phase / chunk boundary, and terminal success can no longer overwrite cancelled / failed state
- MCP / IDE integration:
  - localhost-only and explicit opt-in
  - App Lock still gates archive access
  - read-only lexical recall is allowed even without an embedding provider, but the UI must say semantic recall is unavailable
  - every external query leaves an audit trail
- Enrichment / derived state:
  - shipped built-in plugins are `title-normalization` and `readable-content-refetch`
  - rebuild / clear remains explicit and traceable
  - derived-state storage impact is visible, but only for the currently shipped v1 slice

---

## Deferred Or Not Proven

- Not shipped:
  - revisit / resurfacing features such as forgotten pages, returning topics, and session-pattern recovery
  - plugin sandboxing for untrusted or third-party enrichment execution
  - per-plugin queue family with independent retry / cancel / concurrency controls
- Only partially closed:
  - invalidation is honest and visible, but v1 does not auto-enqueue a rebuild every time visibility or enrichment changes
  - the pre-reset storage plane still leaves FTS, intelligence runtime, and semantic compatibility state too close to the hot archive
  - the deterministic pipeline is now materially more honest and faster on a 64k real archive plus synthetic 100k / 1M corpora, but it still materializes the active analysis window in memory and has not yet been reworked into a chunked / resumable / low-RAM 10M-scale pipeline
- Not honestly signed off:
  - a blanket claim that "all features remain smooth with 60 years of data and all AI enabled on an 8 GB / 4-core machine"
  - any promise that current Insights rebuild, semantic retrieval, and whole-shell refresh have already been profiled against a representative long-horizon archive

---

## Privacy And Data Sovereignty Review

- Still true:
  - PathKeep does not upload archive facts to a PathKeep-operated cloud service
  - AI providers remain explicit user configuration, not hidden background dependencies
  - MCP is localhost-only and lock-aware
  - semantic / enrichment / insight state remains rebuildable derived data, not canonical history facts
- Current caveats that must stay explicit:
  - `readable-content-refetch` fetches remote pages again by design, so its network activity is optional and user-visible
  - plaintext remote backup bundles remain supported only with strong warnings, not silent acceptance
  - integration preview artifacts are preview/manual-copy material; PathKeep does not auto-install MCP or skill files behind the user's back

---

## Verdict

- No: PathKeep should not yet be documented as having completed every intelligence requirement in the design docs.
- No: PathKeep should not yet claim a fully verified "60-year, all AI on, smooth on 8 GB / 4-core" baseline.
- Yes: the current M3 / M4 intelligence slice is now materially more honest and more reviewable. Semantic staleness, cost visibility, MCP consent / scope / audit copy, and run-type truth have all been pulled into the shipped contract.
- The next promotion from "truthful partial support" to "large-archive signed-off support" requires the storage-plane reset from `WORK-QC-R`, a real large-profile replay bundle, and a replayable corpus definition, not just another synthetic shell artifact or docs-only closeout.
