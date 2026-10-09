# Unit test audit, 2026-10

Asked by the user on 2026-10-08: early on, agents wrote many unit tests and the project chased 100%
mutation coverage; the product still broke whenever it was touched, and every change now carries those
tests. Are they pinning subtle or wrong behavior, and which have no reason to exist?

Four read-only subagents went through every Rust and frontend unit test (about 2,400) and sorted each
into:

- **A**: pins wrong behavior. The asserted outcome contradicts the docs or the product's intent and
  would fail a correct fix.
- **B**: pins an incidental detail: strings nobody sees, private helper outputs, counts, internal order.
  Breaks on a harmless refactor.
- **C**: a mutation or coverage leftover: restates the code, probes a constant, or is one of many
  near-duplicates.
- **D**: keep. Pure logic with a clear spec, data integrity, crash recovery, security boundaries, a
  real past bug, or something an E2E cannot reach.

| Report                                                                | A            | B   | C   | D   | Tests |
| --------------------------------------------------------------------- | ------------ | --- | --- | --- | ----- |
| [vault-core: AI, AI queue, agent store](vault-core-ai.md)             | 6            | 65  | 318 | 247 | 636   |
| [vault-core: archive, import, og:image](vault-core-archive.md)        | 7            | 46  | 160 | 340 | 553   |
| [vault-core: Insights, runtime, the rest](vault-core-intelligence.md) | 5 (+4 maybe) | 66  | 161 | 355 | 591   |
| [Other crates and the frontend](other-crates.md)                      | 4            | 58  | 186 | 396 | 644   |

The reports are the subagents' work, with file and line references as of 2026-10-08. Each item is
checked against the code before anything is changed. The AI and Insights reports overlap on a few
integration test files in `vault-core/tests/`.

## What the audit found besides tests

These matter more than any test, and come first.

**Security (P0)**

1. The Ask screen opens links the model wrote through `open_external_url`, which accepts `file://`
   (`src/features/ask/markdown.tsx`, `vault-platform` `launcher.rs`).
2. The og:image fetcher, on by default, follows up to 8 redirects with no SSRF check per hop, while
   `ai-security-posture.md` says every hop of both fetchers is checked.
3. The streaming 64 KiB cap on the Bilibili API body (`cafac470`) was lost in a merge; the body is read
   whole again. An ignored test passed either way.
4. Under `cfg(test)` the content fetcher's redirect guard always allows, so no test sees the real guard.
5. Testing an embedding provider's connection sends an empty `Bearer` header.
6. 35 network tests (og:image, SSRF guard, content-fetch transport) are `#[ignore]`d and never run,
   although they only need a local mock server.

**Wrong in the product (P1)**, from the A items and from reading the code:

- Settings → AI → Rebuild estimates with the removed candle engine's speed: about 22 hours for 100k
  pages that take minutes.
- The Ask agent's date filter takes the day in UTC, while its results are bucketed by local day.
- Raw English, or a raw i18n key, reaches the screen: Insights' "Limited" reason, the Windows schedule
  message, an interrupted chat's "Untitled conversation".
- The Threads card shows the number of wordings of a search as days.
- Chrome's visit duration (microseconds) is stored as milliseconds, so durations are 1000× too long.
  Changing it needs a trade-off note.
- Firefox's visit type is stored as if it were Chrome's transition, so Firefox links count as typed and
  redirects as searches.
- Any `.jsonl` file in a Takeout is imported as Chrome history (a test fixture format in the product).
- History's semantic search filters date and browser in the browser after each 50-row page, though the
  backend takes dates.
- Warnings after a backup reach neither the run record nor any screen.
- Clearing the index queue turns failed jobs into cancelled ones; a torn last record blocks every
  incremental index build; a corrupt sidecar header stops semantic search instead of falling back.
- Smaller: a reopened chat loses each citation's profile; the connection test says OpenAI-compatible
  providers can't call tools while sending them tools; a script with no `return` yields `null`; compare
  sets compare only the first path segment; a 429 is retried after about a second; YouTube ids are lost
  when any query pair has no `=`; title words match as substrings ("api" in "rapid"); the Clear Insights
  preview misses four tables; HTML export has no charset; an entity reference serializes in snake_case.
- Needs a decision: legacy recovery deletes an unreadable rekey or import marker (moderate confidence);
  a visit whose parent URL is missing is skipped and the cursor moves past it (weak).

## Plan

1. **Security.** Each with an E2E where the product reaches it (a loopback page that redirects to a
   private address; a model answer with a `file://` link), and the ignored network tests running in
   the gate.
2. **P1 bugs**, each proved by an E2E scenario where one exists or can be built; the A tests that pin
   them go in the same change.
3. **Delete B and C**, area by area, with the code that only they keep alive. The reports list it: the
   `cfg(coverage)` twins (the coverage gate is gone), test-only stubs inside production code, dead
   commands (`get_browse_day_insights`, the legacy queued assistant, `narrative.rs`, and others),
   duplicate test-only builders in Insights. Code that is reachable from no screen but is described as
   shipped in an accepted doc (the candle embedding tier) is a user decision first. The user
   decided to remove the candle tier (2026-10-09); the review is `candle-removal.md`.
4. **Tests that never run.** The `devtools-bridge` tests and the Vitest files run in the gate or go;
   clock-seeded torture tests get a fixed seed.
5. **E2E scenarios the reports name** where a deleted test was the only guard: Takeout and Safari
   import, the Ask flow against a fake OpenAI-compatible server, the connection test, building the
   semantic index with a kill and relaunch, stars and note search, og:image fetching, post-backup
   warnings, search ranking.
6. **Rules.** `quality-matrix.md` and AGENTS.md say when a unit test is written at all; stale docs the
   reports list are corrected.

Every step runs the full gate; a change that deletes tests says in its commit which E2E covers the
behavior, or that nothing user-visible depended on it.
