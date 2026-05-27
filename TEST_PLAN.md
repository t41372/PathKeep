# PathKeep Behavior Safety Net Test Plan

Date: 2026-05-26 MST

Scope: repository-wide behavior safety-net audit for the active
`WORK-V03-PAPER-REDESIGN-A` block. The goal is mutation-resistant behavior
coverage, not numeric coverage theater.

## Rules For This Pass

- One module at a time. After each module: run the targeted tests, run that
  module's coverage command, record coverage movement and suspected bugs, then
  stop for checkpoint.
- Product bugs are not silently fixed in this pass. If a correct behavioral
  expectation fails, keep the failing test evidence and report the bug for
  confirmation before changing production code.
- Every added test must assert behavior that would fail under plausible
  mutations such as boundary flips, inverted guards, dropped error handling,
  stale cache reuse, or changed return values.
- Coverage is a floor. Branches with no meaningful assertion remain gaps even
  if line coverage is high.

## Reproducible Baseline Commands

```sh
bun run coverage:js
bun run coverage:rust
```

Actual baseline captured on 2026-05-26:

```text
bun run coverage:js
Test Files  276 passed (276)
Tests       2023 passed (2023)
All files   statements 99 | branches 98 | functions 99.51 | lines 99.5

bun run coverage:rust
Rust coverage verified at 100% for 35246 instrumented source lines and
1634 source functions.
```

Important gate drift:

- `docs/plan/program/quality-matrix.md` says `bun run coverage:js` requires
  100% statements / branches / functions / lines.
- `vitest.config.ts` currently enforces only statements 99 / branches 98 /
  functions 99 / lines 99.
- The JS baseline therefore passes the current config while failing the
  documented shipping contract. Treat this as a safety-net gap, not as done.

Rust note: `coverage:rust` uses the project verifier's semantic source-line
and function rules. It does not provide Rust branch coverage; branch confidence
must come from focused tests plus later `cargo mutants`.

## Module Coverage And Risk Map

Risk levels:

- P0: data loss, privacy/security, archive mutation, large-archive performance,
  desktop command contract.
- P1: user-visible workflow state, route navigation, migration/import/export,
  AI/provider-gated behavior.
- P2: presentational components or fully pinned deterministic helpers.

### Frontend JS/TS Runtime Modules

| Module                                      | Files |  Lines | Branches |  Funcs | Missing branches | Risk | Primary gap                                                                          |
| ------------------------------------------- | ----: | -----: | -------: | -----: | ---------------: | ---- | ------------------------------------------------------------------------------------ |
| `src/pages/explorer`                        |    31 |  97.81 |    93.43 |  97.95 |               95 | P0   | Browse/search route state, infinite pages, annotations, paper view branches.         |
| `src/components/explorer-paper`             |    35 |  99.00 |    94.53 |  98.73 |               40 | P1   | Paper contact sheet/detail/search result render branches and empty/error states.     |
| `src/app`                                   |    15 |  99.87 |    97.50 | 100.00 |               13 | P1   | Router/shell fallback and runtime state branches.                                    |
| `src/components/shell`                      |     7 |  99.50 |    94.50 | 100.00 |               12 | P1   | Navigation history, search palette, status/topbar fallbacks.                         |
| `src/pages/dashboard`                       |    10 | 100.00 |    94.48 | 100.00 |               10 | P1   | Dashboard fallback/dashboard paper branches have line coverage but weak branch pins. |
| `src/lib/explorer-preferences.ts`           |     1 |  94.59 |    83.33 | 100.00 |                4 | P1   | Persistent explorer view-mode parse and localStorage failure paths.                  |
| `src/lib/paper-preferences.ts`              |     1 | 100.00 |    80.00 | 100.00 |                4 | P1   | Appearance preference parse/localStorage fallbacks.                                  |
| `src/components/intelligence`               |    19 | 100.00 |    99.07 | 100.00 |                5 | P1   | Browsing rhythm optional/empty branches.                                             |
| `src/components/sidebar`                    |     3 | 100.00 |    97.32 | 100.00 |                3 | P1   | Background status variant branches.                                                  |
| `src/pages/assistant`                       |     6 |  99.30 |    98.00 |  98.08 |                3 | P1   | Provider-gated assistant page fallback and action branches.                          |
| `src/pages/import`                          |     8 |  99.76 |    99.31 |  99.09 |                3 | P0   | Import workflow edge branches and failure UX.                                        |
| `src/components/primitives`                 |     9 | 100.00 |    98.41 | 100.00 |                2 | P2   | Background progress display branches.                                                |
| `src/components/heatmap`                    |     2 | 100.00 |    95.56 | 100.00 |                2 | P2   | Calendar/heatmap presentational boundary branches.                                   |
| `src/pages/audit`                           |     6 |  99.67 |    99.71 |  98.85 |                1 | P1   | Audit page fallback function branch.                                                 |
| `src/pages/intelligence`                    |    34 | 100.00 |    99.89 | 100.00 |                1 | P1   | Paper intelligence panel optional branch.                                            |
| `src/pages/settings`                        |    32 | 100.00 |    99.89 | 100.00 |                1 | P1   | Data migration section branch.                                                       |
| `src/lib/backend-preview-support.ts`        |     1 |  96.15 |    98.39 | 100.00 |                1 | P1   | Browser-preview support failure branch.                                              |
| `src/lib/backend-preview-shell-commands.ts` |     1 | 100.00 |    98.91 | 100.00 |                1 | P1   | Shell command fixture branch.                                                        |
| `src/lib/backend.ts`                        |     1 |  99.02 |   100.00 |  98.72 |                0 | P1   | Legacy/browser-preview function residual. Do not expand surface.                     |
| `src/main.tsx`                              |     1 | 100.00 |   100.00 | 100.00 |                0 | P0   | Desktop entry is covered and mutation-gated by desktop-contract slice.               |
| `src/lib/backend-client`                    |    15 | 100.00 |   100.00 | 100.00 |                0 | P0   | IPC client surface covered; keep mutation focus on contracts.                        |
| `src/lib/ipc`                               |     4 | 100.00 |   100.00 | 100.00 |                0 | P0   | Desktop bridge covered and mutation-gated by desktop-contract slice.                 |
| `src/lib/core-intelligence`                 |    10 | 100.00 |   100.00 | 100.00 |                0 | P1   | Deterministic API/client covered; later mutation still needed.                       |
| `src/lib/i18n`                              |    29 | 100.00 |   100.00 | 100.00 |                0 | P1   | Catalog parity covered by separate i18n gate.                                        |
| `src/components/review`                     |    10 | 100.00 |   100.00 | 100.00 |                0 | P1   | PME review primitives covered.                                                       |
| `src/pages/schedule`                        |     4 | 100.00 |   100.00 | 100.00 |                0 | P0   | UI covered; Rust scheduler remains the higher-risk owner.                            |
| `src/pages/security`                        |     4 | 100.00 |   100.00 | 100.00 |                0 | P0   | UI covered; Rust app-lock/keyring remain higher-risk owners.                         |
| Other single-purpose UI/helper modules      |    18 | 100.00 |   100.00 | 100.00 |                0 | P2   | Keep as regression surface; deprioritize until P0/P1 gaps close.                     |

Top JS branch residual files:

| File                                                          | Branches | Missing branches | Why it matters                                       |
| ------------------------------------------------------------- | -------: | ---------------: | ---------------------------------------------------- |
| `src/pages/explorer/index.tsx`                                |    90.35 |               22 | Main Browse/Search route orchestrator.               |
| `src/pages/explorer/hooks/use-explorer-infinite-pages.ts`     |    77.66 |               21 | Large-archive infinite scroll and prefetch behavior. |
| `src/components/explorer-paper/paper-contact-sheet.tsx`       |    87.27 |               14 | Browse rendering, virtualization entry point.        |
| `src/app/shell.tsx`                                           |    83.82 |               11 | Global shell runtime/fallback behavior.              |
| `src/pages/explorer/paper-view.tsx`                           |    86.57 |                9 | Paper Browse/Search composition.                     |
| `src/pages/dashboard/index.tsx`                               |    72.41 |                8 | Dashboard fallback branches.                         |
| `src/components/explorer-paper/paper-day-insights-helpers.ts` |    89.74 |                8 | Day aggregate presentation parity.                   |
| `src/components/explorer-paper/paper-detail-panel.tsx`        |    93.33 |                7 | Notes/tags/detail interaction states.                |

### Rust Modules

The current Rust verifier reports 100% semantic source-line and function
coverage across the full `src-tauri/**/src/*.rs` workspace. Remaining Rust
gaps are therefore behavioral, branch, concurrency, I/O, and mutation gaps.

| Module                                                      | Risk | Primary gap to audit next                                                                        |
| ----------------------------------------------------------- | ---- | ------------------------------------------------------------------------------------------------ |
| `vault-core/archive`                                        | P0   | Ingest/write rollback, dedup edge cases, history query semantics, og-image fetch I/O failures.   |
| `vault-core/takeout`                                        | P0   | Malformed/partial payloads, zip read failures, source evidence limits.                           |
| `browser-history-parser/{chromium,firefox,safari,takeout}`  | P0   | Real-browser schema drift, missing optional tables, malformed DBs, timestamp boundaries.         |
| `browser-history-fixtures`                                  | P0   | Sidecar table fixture gaps already tracked in BACKLOG.                                           |
| `vault-core/migration`                                      | P0   | Partial writes, manifest tamper, wrong key, forward schema migration, rollback recoverability.   |
| `vault-core/app_lock` and `vault-platform/keyring`          | P0   | Auth-disabled states, host keyring failures, session/key recovery boundaries.                    |
| `vault-platform/scheduler`                                  | P0   | Host command failures, permission/mismatch branches, cross-platform semantics.                   |
| `pathkeep-desktop/dev_ipc_bridge` and `worker_bridge`       | P0   | Desktop command truth, payload validation, poisoned state, CORS/localhost boundary.              |
| `vault-worker/archive_flows`                                | P0   | Background job cancellation, queue concurrency, og-image worker throttling/retry behavior.       |
| `vault-core/intelligence*` and `vault-worker/intelligence*` | P1   | Optional AI/provider gating, stale sidecars, queue replay/cancel, deterministic fallback parity. |
| `vault-core/annotations`                                    | P1   | URL notes/tags limits, search dimensions, FTS behavior.                                          |
| `vault-core/visit_taxonomy`                                 | P1   | Regional taxonomy, URL normalization, CJK/script-aware tokenization.                             |

## Ordered Work Queue

1. `[x]` `src/pages/explorer/hooks/use-explorer-infinite-pages.ts`
   - Reason: P0 large-archive performance and correctness owner; 21 missing JS
     branches; recent BROWSE-VIRT changes make this a mutation-priority module.
   - Behavior focus: no duplicate in-flight fetches, no prefetch past pageCount,
     cache-token reset, silent background prefetch failure, load guards.
2. `[x]` `src/pages/explorer/index.tsx`
   - Main route orchestrator; many branches likely need integration-level route
     tests instead of shallow render assertions.
3. `[ ]` `src/components/explorer-paper/paper-contact-sheet.tsx`
   - Virtualization/render-state branches; must assert mounted/recycled/session
     behavior rather than DOM snapshots alone.
4. `[ ]` `src/components/explorer-paper/paper-detail-panel.tsx`
   - Notes/tags persistence UX; assert disabled/error/loading states and
     mutation-prone handler behavior.
5. `[ ]` `src/lib/explorer-preferences.ts` and `src/lib/paper-preferences.ts`
   - Persistent local preference parse/fallback failure paths.
6. `[ ]` `src/app/shell.tsx` and `src/components/shell/*`
   - Route history and global shell state branches.
7. `[ ]` `src/pages/dashboard/index.tsx`
   - Branch coverage is low despite line coverage; assert fallback semantics.
8. `[ ]` Rust import/fixture sidecar backlog blocks
   - Follow existing BACKLOG order: sidecar fixture extension, minor integrity
     pins, parser ordering, concurrency.
9. `[ ]` Rust migration/security/scheduler fault-injection sweep
   - Add tests for partial I/O, permission errors, command failures, and
     concurrent state transitions before full mutation.

## Bug / Drift Register

| ID         | Status | Evidence                                                                                                                                                                                                                                                                         | Action                                                                                                                                       |
| ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| QA-GAP-001 | open   | JS coverage command passes at 99/98/99/99 while quality matrix says 100/100/100/100.                                                                                                                                                                                             | Close residual branches, then raise `vitest.config.ts` thresholds back to 100.                                                               |
| QA-GAP-002 | open   | Rust coverage verifier has no branch metric.                                                                                                                                                                                                                                     | Use focused boundary/error tests plus `cargo mutants`; do not claim Rust branch coverage from llvm-cov alone.                                |
| QA-GAP-003 | open   | `src/pages/explorer/index.tsx` still builds a paginated `PaperExplorerView` prop, but the current render grammar routes every `infiniteDisabled` condition to Search, grouped views, invalid-regex callout, or locked/uninitialized states before `PaperExplorerView` can mount. | Confirm whether the legacy paginated time-view branch should be deleted or restored as an intentional mode; do not add a fake coverage seam. |

## Checkpoint Log

### Baseline

- JS: `bun run coverage:js` passed, 2023 tests, global 99 statements / 98
  branches / 99.51 functions / 99.5 lines.
- Rust: `bun run coverage:rust` passed, verifier 100% across 35246 semantic
  source lines and 1634 source functions.
- No product bugs confirmed yet; only gate drift recorded.

### Module 1: `src/pages/explorer/hooks/use-explorer-infinite-pages.ts`

Added 5 focused behavior tests:

- cache-token refresh clears accumulated page 2..N state even when the query
  signature is unchanged.
- a second `loadMore()` while page 2 is in-flight does not issue a duplicate
  foreground request.
- unconditional background prefetch does not request `pageCount + 1`.
- background prefetch rejection stays silent when the foreground page succeeds.
- downward `+2` prefetch does not request beyond the reported `pageCount`.

Commands:

```sh
bunx vitest run src/pages/explorer/hooks/use-explorer-infinite-pages.test.tsx
bunx vitest run src/pages/explorer/hooks/use-explorer-infinite-pages.test.tsx --coverage --coverage.include=src/pages/explorer/hooks/use-explorer-infinite-pages.ts --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Test Files  1 passed (1)
Tests       15 passed (15)

use-explorer-infinite-pages.ts targeted coverage:
statements 92.02 | branches 79.78 | functions 95.65 | lines 99.01
uncovered line: 345
```

Movement from repository baseline for this file:

- statements: 91.30 -> 92.02
- branches: 77.66 -> 79.78
- functions: 95.65 -> 95.65
- lines: 99.01 -> 99.01

Suspected bugs: none. The uncovered residual is mostly the hard
`MAX_ACCUMULATED_PAGES` cap path, which is awkward to hit without either
exporting a test seam or driving hundreds of `loadMore()` transitions. Keep it
queued for a dedicated cap-boundary test rather than adding an artificial
coverage-only assertion.

### Module 2: `src/pages/explorer/index.tsx`

Added 4 route-shell behavior tests and tightened the paper-surface mocks:

- Paper filter strip apply trims filled values, deletes blank values, clears
  stale `page`, removes individual chips through `updateParam`, and delegates
  clear-all to the URL-state hook.
- Legacy `config.ogImage.fetchEnabled = false` is folded into `fetchMode:
'off'` before the route calls `useExplorerOgImages`.
- Search-result URLs such as Google SERPs suppress misleading og:image
  hydration even when the row/cache advertises an image.
- Detail panel "All of domain" resets the URL to a domain-only Browse state,
  dropping search/date/profile/pagination context.

Commands:

```sh
bunx vitest run src/pages/explorer/index.test.tsx
bunx vitest run src/pages/explorer/index.test.tsx --coverage --coverage.include=src/pages/explorer/index.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
bun run typecheck
```

Actual output:

```text
Test Files  1 passed (1)
Tests       17 passed (17)

index.tsx targeted coverage from this test file:
statements 86.54 | branches 79.82 | functions 65.3 | lines 86.25
uncovered lines include 606, 659-698, 867-933
```

Suspected product bugs: none confirmed. Drift recorded as `QA-GAP-003`: the
paginated `PaperExplorerView` prop branch appears unreachable under the current
route grammar, so it should be resolved as product-code cleanup or restored as
an intentional mode rather than covered with artificial tests.
