# Turso migration review ledger

Review findings for the move to Turso (`WORK-TURSO-STORAGE`), collected while the areas were built. They are fixed in one pass after every area is done, not area by area; the user asked for this on 2026-10-05.

Raw reports (Codex results, review texts, briefs) are kept outside the repo in `.git/pathkeep-wip/reports/` on the development machine.

The findings here are unverified. In the final pass, verify each one as BDFL:

- is it reachable?
- is it the root cause?
- does it fit the threat model?
- is it worth the complexity?

Then do one fix pass.

## Area 1 (write path)

### Corrections I already verified (the round-3 brief over-built these)

These are in `brief-area1-fixes3b.txt` (raw reports):

- **M2:** write the envelope before `open_owned`. This replaces the pending, promote and recover scheme, if round 3 built it.
- **Drop "a typed password during the startup open is dropped".** It can't happen; remove any retry logic added for it.
- **Drop the migration of old keychain entries.** There are no released users.
- **Owner endpoint:** don't add a bind-failure dialog. Binding at startup fails during setup, as HEAD did.
- **Statement benchmark:** stays an ignored measurement, with no per-row budget assertion.

### Open questions for the product

- **Background mode never exits.** A `--background` launch (scheduled backup, CLI, MCP) stays resident in the tray, even when the tray preference is off.
  - Proposal: when the window was never opened and the tray preference is off, exit after all work is done (backup, import, indexing, requests) and a short idle period.
  - I told the user and am waiting for an objection.

### The canonical-URL rule changed silently (round 3)

- Canonical normalisation now decodes percent-escapes of unreserved characters in the path, so `%7E` and `~` become the same page.
- RFC 3986 treats these as equivalent, so the rule itself is probably right. What to check:
  - Star and annotation keys stored under the old rule may no longer match their page.
  - History grouping changes.
  - Every place that computes the canonical key must use the same function.
- The SQLCipher converter (area 5) must recompute every stored canonical key (stars, annotations, enrichment) with the current rule. Put this into the area 5 brief.
- Write the rule down in data-model.md as a decision, not as a side effect.

### Reviewer round-3 minors not put into any brief

- Firefox resume (M1c) is in the round-3 brief; check that it's done.

## My frontend to-do (not review findings)

- **Search while the index is building** (the user agreed on 2026-10-05: "記得 UI 上要讓用戶知道這一點"). After an upgrade or an index reset, search covers only what is indexed so far. While that lasts:
  - every search result view (History search, the command palette, AI search evidence) shows clearly that the index is still being built, with its progress;
  - it must not look like the full answer. No "no results" without that notice, and counts are marked as partial;
  - the notice goes away once the index catches up.
  - Applies in en / zh-CN / zh-TW. E2E: hold the indexer mid-build and assert the notice and the partial count.
- Streamed search presentation (provisional results, progress, counts arriving).
- Run states, partial-import states, revert and restore triggers.

## Area 2 (read paths), Codex's own report

- **Schema migrations again.** Area 2 added "forward v2/v3 migrations", although round 3 had folded the unreleased schema into v1. In the final pass, fold them into v1 again. Then say plainly in AGENTS.md or the store-schema doc that unreleased schemas stay at v1.
- **legacy\_\* modules** (legacy_source_evidence, legacy_status, legacy_totals) hold the old SQLCipher helpers. Delete them in area 6.
- **Changed command shapes (frontend is mine):**
  - Timeline `page > 1` requires a cursor.
  - `load_recent_runs` and `load_import_batches` return `{items, nextCursor}`; unfinished batches come first.
  - `preview_import_batch` takes a cursor and limit and returns `previewNextCursor`.
  - The snapshot carries run and import cursors.
  - The dashboard has `recentRunsNextCursor`.
  - The repair report has new counts.
- **Benchmark** (14.4M encrypted): first timeline page 2.0 ms cold, day insights 26 ms cold, URL detail with 100k visits 1.9 ms, full export 34 s cold at 39 MiB peak.
- Not run on real 4-core / 8 GB hardware.
- Turso has no `foreign_key_check` and no way to interrupt a running statement. Codex replaced them with explicit relationship checks and cooperative cancellation.

### Codex review of area 2 (unverified; full text in review-area2-codex.md)

- [P2] `vault-worker/src/app.rs:180-181`: commands that return a snapshot, such as saving or clearing an AI provider key or changing content-fetch settings, go through a wrapper that builds a locked, empty snapshot. The frontend (`use-ai-providers.ts`) then overwrites the valid snapshot cache with it. Codex's proposed fix: pass the session's Vault through.
- [P2] `doctor.rs:49-53`: the new Doctor check codes (`canonical-references`, `cached-totals`, `search-outbox` and the four integrity codes) aren't in `HealthRow`'s `knownChecks` or the locale catalogs, so zh-CN and zh-TW show English. This is frontend and mine: add the labels in all three locales.

## Area 3 (search), Codex's own report (full text in the raw reports)

- **Performance to investigate (root cause, not symptoms):**
  - The final first page for a common term takes 8.3 s, and a profile filter 3.8 s.
  - The vault-search crate alone did a common word's first page in 40 ms warm, so the cost is in the vault-core layer (eligibility, visibility validation, hydration, or "pinned visit boundaries") rather than in Tantivy.
  - The first provisional results arrive in 11–93 ms, so the user does see results quickly, but 8 s to final is far off the crate's numbers. Profile it before deciding anything.
- **Indexer delay:** a star or note becomes searchable 1.75 s after commit. Is the delay in the poll interval?
- **Full rebuild:** 35.6 s at 404 MiB. Good: an upgraded archive's index is ready quickly.
- **Commands (frontend is mine):**
  - `start_search({request, onEvent}) -> {sessionId, indexer}`;
  - `next_search_page({sessionId, cursor})`;
  - `cancel_search`;
  - `search_indexer_state`.
  - Events: `indexer`, `step`, `error`, `cancelled`. A step carries page, items, provisional, progress, exactness, cursor, indexer state, snapshotSeq and `coverageComplete`.
  - Dev IPC adds SSE at `/searches/{id}/events`.
  - Contract: docs/architecture/search.md.
- "Retaining historical wire fields without emitting truncation": check for dead fields (`windowed` and the like) and remove them.
- Check the new docs for process narration: `research/2026-10-search-integration-validation.md`.
- `bun run check` failed in Codex's sandbox at owner_process with kLSNoExecutableErr; that's a sandbox issue, so run it natively.

## Area 4 (intelligence, AI queue, agent, enrichment), Codex's own report (raw reports)

- **Full intelligence rebuild at 14.4M is not finished after 1,877 s.** It was stopped at the 15 GB data budget during refind generation; peak RSS was 151 MiB.
  - Find the root cause: is refind quadratic, is scratch I/O the bottleneck, or is it batch-size churn?
  - Measure on Modal, where disk is not a constraint.
- Incremental update and cold/warm read numbers were never measured.
- The changed request/response shapes are in `docs/architecture/research/2026-10-turso-area4.md`: list/detail cursors, job-history endpoints, enrichment provenance and agent output fragments. The frontend is mine.
- 17 SQLite-engine tests deleted and 23 renamed. Check that the deletions are justified.

## Found while moving checks to Modal (Linux, 16 cores)

- `ai::code_mode::tests::the_async_bridge_is_safe_from_inside_a_runtime_worker` fails in the full suite with a `Time` trap but passes alone (32 s each in a debug build).
  - Cause: the wall-clock deadline is hit under parallel load.
  - The test depends on wall time, and compiling the debug wasm/JS engine is slow. Fix the test, or give cranelift/wasmtime an opt-level in the dev profile; don't just raise the deadline.
  - Present on redesign/v0.4 too.

## Benchmarks measured under gVisor

- Until 2026-10-06 ~13:30 local time, offload Sandboxes ran under gVisor, which makes SQLite system calls several times slower. That covers area 3's Linux test timings and any area 4 or area 5 benchmark run before the switch to `runtime = "vm"`.
- Re-measure those benchmarks on vm before quoting them, and check each report for which runtime it ran on.

## Area 5+

(appended as blocks finish)
