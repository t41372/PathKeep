# Intelligence 60-Year Support Envelope

> `WORK-QC-D` closeout note. This document records what PathKeep can honestly claim about optional intelligence as of 2026-04-09, and just as importantly, what it still cannot claim.

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
- 2026-04-09 release-style sweep status:
  - `bun run verify` passed end-to-end, including `coverage:js`, `coverage:rust`, `mutation:js`, `mutation:rust`, `test:e2e`, and `desktop:build:debug`
  - the current Vite production build still emits a single main-chunk warning (`dist/assets/index-*.js` at roughly 702 kB minified), so checker parity is back but whole-shell payload size is not yet signed off as "large-archive smooth"
- Focused intelligence regressions were re-run while closing this work:
  - `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core ai:: -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker mcp_ -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core takeout:: -- --nocapture`
  - `bunx vitest run src/lib/backend.test.ts src/lib/intelligence.test.ts src/lib/trust-review.test.ts src/app/index.test.tsx src/pages/intelligence-surfaces.test.tsx`
- Code / architecture fixes landed during this closeout:
  - semantic retrieval now queries the LanceDB sidecar first and only falls back to the SQLite compatibility mirror with explicit notes
  - index state now reports `stale` when archive visibility / import watermark or readable-content enrichment freshness diverges from the last semantic build
  - embedding rebuild now batches requests, retries, and tolerates partial failure instead of doing only one-row-at-a-time indexing
  - Settings now shows indexed rows, LanceDB sidecar bytes, SQLite mirror bytes, and estimated embedding token volume
  - Settings now shows MCP / skill consent, capability, scope-boundary, audit-trace copy, plus preview artifact contents instead of only opaque file paths
  - MCP searches now write dedicated `mcp_query` run-ledger entries, and import restore no longer masquerades as `rollback`
  - selected provider/model readiness is now model-scoped, so switching embedding models no longer reuses readiness from a different model
- Missing evidence that still matters:
  - there is still no checked-in `artifacts/perf/<date>-large-archive-...>/` bundle for a real large-profile replay
  - there is still no synthetic long-horizon benchmark that exercises semantic search, assistant retrieval, insights rebuild, and shell responsiveness under a defined 60-year corpus shape
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
  - mid-flight cancel is still not supported and must remain explicit in UI copy
- MCP / IDE integration:
  - localhost-only and explicit opt-in
  - App Lock still gates archive access
  - read-only lexical recall is allowed even without an embedding provider, but the UI must say semantic recall is unavailable
  - every external query leaves an audit trail
- Enrichment / derived state:
  - `readable-content-refetch` is the only shipped plugin
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
  - the SQLite embedding mirror still exists as a compatibility fallback and storage cost, so the sidecar boundary is not yet the only runtime path
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
- The next promotion from "truthful partial support" to "large-archive signed-off support" requires a real perf artifact bundle plus a replayable corpus definition, not just another docs-only closeout.
