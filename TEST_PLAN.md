# PathKeep Behavior Safety Net Test Plan

Date: 2026-05-26 MST

Scope: repository-wide behavior safety-net audit for the active
`WORK-V03-PAPER-REDESIGN-A` block. The goal is mutation-resistant behavior
coverage, not numeric coverage theater.

## Rules For This Pass

- One module at a time. After each module: run the targeted tests, run that
  module's coverage command, record coverage movement and suspected bugs, then
  make a checkpoint commit before continuing.
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
3. `[x]` `src/components/explorer-paper/paper-contact-sheet.tsx`
   - Virtualization/render-state branches; must assert mounted/recycled/session
     behavior rather than DOM snapshots alone.
4. `[x]` `src/components/explorer-paper/paper-detail-panel.tsx`
   - Notes/tags persistence UX; assert disabled/error/loading states and
     mutation-prone handler behavior.
5. `[x]` `src/lib/explorer-preferences.ts` and `src/lib/paper-preferences.ts`
   - Persistent local preference parse/fallback failure paths.
6. `[x]` `src/app/shell.tsx` and `src/components/shell/*`
   - Route history and global shell state branches.
7. `[x]` `src/pages/dashboard/index.tsx`
   - Branch coverage is low despite line coverage; assert fallback semantics.
8. `[x]` Rust import/fixture sidecar backlog blocks
   - Follow existing BACKLOG order: sidecar fixture extension, minor integrity
     pins, parser ordering, concurrency.
9. `[x]` Rust migration/security/scheduler fault-injection sweep
   - Add tests for partial I/O, permission errors, command failures, and
     concurrent state transitions before full mutation.
   - `[x]` 9A migration import fail-fast refusal paths.
   - `[x]` 9B security/keyring refusal and recovery paths.
   - `[x]` 9C scheduler host-command failure paths.
10. `[x]` JS coverage residual restoration
    - Restored `coverage:js` from the temporary 99/98/99/99 floor back to the
      documented 100/100/100/100 gate without excluding active runtime code.
    - `[x]` 10A Search/Browse chip and result residuals.
    - `[x]` 10B Explorer route and paper-view residuals.
    - `[x]` 10C Dashboard/app shell residuals.
    - `[x]` 10D Hook/helper branch residuals.
    - `[x]` 10E Explorer runtime hooks and paper helper residuals.
    - `[x]` 10F Explorer Paper component residuals.
    - `[x]` 10G Shared component, preview facade, and route residuals.
11. `[x]` Rust parser mutation hardening
    - Used focused `cargo-mutants` evidence to compensate for the unavailable
      stable Rust branch metric without claiming branch coverage that the tool
      does not report.
    - Hardened `browser-history-parser` Chromium/Firefox/Safari/Takeout
      streaming, schema observation, evidence chunk, source sniffing, native
      key, warning, and merge-report contracts until the parser mutation slice
      reached 0 missed mutants.

## Bug / Drift Register

| ID         | Status | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Action                                                                                                                                                                                                                            |
| ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| QA-GAP-001 | closed | JS coverage command previously passed at 99/98/99/99 while quality matrix says 100/100/100/100. Module 10G restored `vitest.config.ts` thresholds to 100/100/100/100 after `bun run coverage:js` reported All files 100/100/100/100 and lcov had no uncovered JS lines, branches, or functions.                                                                                                                                                                                                                                             | Resolved in Module 10G; keep 100/100/100/100 as the per-commit JS runtime gate.                                                                                                                                                   |
| QA-GAP-002 | closed | Rust coverage verifier has no branch metric, and `cargo llvm-cov --branch --manifest-path src-tauri/Cargo.toml --workspace --all-features --lcov --output-path coverage/rust-branch.lcov.info` fails on the pinned stable Rust 1.94.1 toolchain because `-Z coverage-options=branch` is nightly-only. Before hardening, `bun run mutation:rust:parser` reported 431 mutants with 78 missed, 298 caught, and 55 unviable. After focused parser tests, the same parser slice reported 431 mutants with 376 caught, 55 unviable, and 0 missed. | Resolved by mutation-resistant parser behavior tests. Do not claim Rust branch coverage from llvm-cov; keep Rust branch confidence tied to focused boundary/error tests plus `cargo mutants` evidence under the stable toolchain. |
| QA-GAP-003 | closed | `src/pages/explorer/index.tsx` still built a paginated `PaperExplorerView` prop, but the current render grammar routes every `infiniteDisabled` condition to Search, grouped views, invalid-regex callout, or locked/uninitialized states before `PaperExplorerView` can mount.                                                                                                                                                                                                                                                             | Resolved in Module 10B by deleting the route-only paginated prop, the redundant results render guard, and the unreachable infinite-scroll fallback. Component-level pagination remains covered in `paper-view.test.tsx`.          |
| QA-GAP-004 | closed | `src/components/explorer-paper/paper-contact-sheet.tsx` retained defensive guards that targeted behavior could not naturally hit: toolbar ref missing after mount and `canLoadMore` false after the sentinel exists.                                                                                                                                                                                                                                                                                                                        | Resolved in Module 10F by simplifying the redundant guards and keeping contact-sheet behavior pinned through component and virtualization assertions.                                                                             |
| QA-GAP-005 | closed | `src/components/explorer-paper/paper-detail-panel.tsx` retained unreachable defensive branches: layout-flush with `pendingFlushRef` but no active timer, and `LookFurtherRow` non-interactive rendering even though the parent filters out rows without handlers.                                                                                                                                                                                                                                                                           | Resolved in Module 10F by simplifying the redundant branches and keeping visible detail-panel behavior pinned through focused component assertions.                                                                               |
| QA-GAP-006 | closed | Shell residual branches were defensive or redundant under current public grammar: `shellStorage()` without `window`, `handleSearchQuery()` receiving a blank query after `PKSearchPalette` already suppresses blank searches, `archiveHealthy ?? false` after a boolean expression, `domainAbbreviation().split('.')[0] ?? cleaned`, and duplicate route-history location keys inside an effect keyed by `location.key`.                                                                                                                    | Resolved in Module 10C by deleting redundant shell/palette fallbacks, pinning palette whitespace suppression, and covering the route-history duplicate-key branch through a StrictMode render.                                    |

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

### Module 3: `src/components/explorer-paper/paper-contact-sheet.tsx`

Added a focused behavior test file with 10 assertions over the contact-sheet
branches most likely to survive mutation:

- Cards-mode frame numbers continue across day boundaries, so virtualization
  extraction cannot reset the filmstrip counter per day.
- Sticky day headers use the measured toolbar height both with and without
  `ResizeObserver`, including when filter chips wrap the toolbar.
- Invalid visit timestamps render `--:--`; malformed/unrepresentable day keys
  render raw labels instead of crashing.
- Null page titles fall back to sanitized URL text in both cards and list mode.
- Target clear is safe when the caller omits `onClearTarget`.
- Infinite-scroll cap guidance, non-intersecting sentinel, and load-error alert
  states render with the intended copy and side effects.

Commands:

```sh
bunx vitest run src/components/explorer-paper/paper-contact-sheet.behavior.test.tsx
bunx vitest run src/components/explorer-paper/paper-contact-sheet.test.tsx src/components/explorer-paper/paper-contact-sheet.behavior.test.tsx src/components/explorer-paper/paper-contact-sheet.virt.test.tsx src/components/explorer-paper/paper-contact-sheet.spike.test.tsx --coverage --coverage.include=src/components/explorer-paper/paper-contact-sheet.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Behavior test file:
Test Files  1 passed (1)
Tests       10 passed (10)

All paper-contact-sheet focused files:
Test Files  4 passed (4)
Tests       45 passed (45)

paper-contact-sheet.tsx targeted coverage:
statements 98.14 | branches 98.18 | functions 100 | lines 100
uncovered lines: 279, 677

Full checkpoint gate:
bun run check
JS coverage: statements 99.27 | branches 98.24 | functions 99.66 | lines 99.69
Rust coverage: 100% for 35246 instrumented source lines and 1634 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed. Drift recorded as `QA-GAP-004`: the
remaining guards are defensive branches that current render grammar does not
make observable through user behavior.

### Module 4: `src/components/explorer-paper/paper-detail-panel.tsx`

Added 5 behavior assertions around the detail panel's persistence and navigation
surface:

- External notes refreshes update the textarea when no local edit is pending.
- Pending local notes survive a stale backend refresh and still debounce-save the
  local draft.
- Look-further rows are suppressed when route handlers are not wired, avoiding
  phantom navigation labels.
- Look-further rows remain clickable when count hints are omitted.
- Favicon and og:image media use default test ids when the caller does not pass
  a panel `testId`.

Commands:

```sh
bunx vitest run src/components/explorer-paper/paper-detail-panel.test.tsx --coverage --coverage.include=src/components/explorer-paper/paper-detail-panel.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Test Files  1 passed (1)
Tests       28 passed (28)

paper-detail-panel.tsx targeted coverage:
statements 100 | branches 96.19 | functions 100 | lines 100
uncovered lines: 228, 757-765

Full checkpoint gate:
bun run check
JS coverage: statements 99.27 | branches 98.27 | functions 99.66 | lines 99.69
Rust coverage: 100% for 35246 instrumented source lines and 1634 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed. Drift recorded as `QA-GAP-005`: the
remaining branches are defensive paths that the current public render grammar
does not expose through user behavior.

### Module 5: `src/lib/explorer-preferences.ts` and `src/lib/paper-preferences.ts`

Added 9 behavior assertions over persistent local preference fallbacks:

- Explorer view mode and clock format return defaults and no-op safely when
  `window` is unavailable.
- Explorer read helpers return defaults when localStorage read access throws.
- Re-persisting the current clock format skips storage writes and does not emit
  a redundant live-update event.
- Paper preferences return defaults and no-op safely when `window` is
  unavailable.
- Unrecognized paper preference values normalize back to the shipped appearance.
- `applyPaperPreferences` returns the supplied candidate without touching
  globals when both `window` and `document` are unavailable.
- `applyPaperPreferences` still updates document attributes and dispatches the
  live-update event when localStorage persistence fails.

Commands:

```sh
bunx vitest run src/lib/explorer-preferences.test.ts src/lib/paper-preferences.test.ts --coverage --coverage.include=src/lib/explorer-preferences.ts --coverage.include=src/lib/paper-preferences.ts --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Test Files  2 passed (2)
Tests       37 passed (37)

explorer-preferences.ts targeted coverage:
statements 100 | branches 100 | functions 100 | lines 100

paper-preferences.ts targeted coverage:
statements 100 | branches 100 | functions 100 | lines 100

Full checkpoint gate:
bun run check
JS coverage: statements 99.34 | branches 98.35 | functions 99.66 | lines 99.71
Rust coverage: 100% for 35246 instrumented source lines and 1634 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed.

### Module 11: Rust parser mutation hardening

Added or strengthened parser behavior tests around the Rust branch/mutation gap
that cannot be measured by stable `cargo llvm-cov --branch`:

- Chromium and Firefox incremental URL queries now prove that a single zero
  cursor does not accidentally take the first-import fast path.
- Chromium/Firefox/Safari streaming tests now assert exact batch boundaries,
  retained vs streamed evidence movement, and capability populated/total row
  counts.
- Shared schema observation tests pin required/missing table rows, optional
  table status, row counts, primary-key ordinals, and `NOT NULL` detection.
- Source-evidence chunk tests pin the empty contract across search,
  navigation, engagement, context, and native entity families.
- Takeout source tests now pin direct localized history recognition, whitespace
  normalization, non-Chrome JSON no-sniff behavior, sniff byte limits, and the
  `"Browser History"` + `"time_usec"` conjunction.
- Takeout payload tests now pin native primary key fallback/source fields,
  missing-time warning absence/presence, callback-abort display text,
  canonical-only adapter passthrough, merge-report count/capability/evidence
  aggregation, and default optional `HistoryBatchConsumer` no-op methods.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser
bun run mutation:rust:quality
bun run coverage:rust
bun run check
```

Actual output:

```text
Focused parser tests:
browser-history-parser: 56 passed; 0 failed

Rust branch metric probe:
cargo llvm-cov --branch ... failed on pinned stable Rust 1.94.1 because
`-Z coverage-options=branch` is nightly-only.

Rust mutation quality:
bun run mutation:rust:quality
browser-history-parser: 431 mutants tested in 11m; 376 caught; 55 unviable; 0 missed
vault-core ai helper filter: Found 0 mutants to test

Rust coverage:
Rust coverage verified at 100% for 35643 instrumented source lines and
1654 source functions.

Full checkpoint gate:
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2162 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
Rust workspace tests: browser-history-parser 56 passed; vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
JS coverage: All files statements 100%, branches 100%, functions 100%, lines 100%
Rust coverage: verified at 100% for 35643 instrumented source lines and 1654 source functions
supply-chain/platform/release checks: passed
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. Drift closed as `QA-GAP-002`; Rust
branch coverage is still not claimed from llvm-cov under the stable toolchain,
and branch confidence for this slice is now backed by focused boundary/error
tests plus zero missed parser mutants.

### Module 8A: `WORK-IMPORT-FIXTURE-SIDECARS-A`

Added 6 focused Rust behavior tests:

- Chromium fixture self-validation proves generated downloads, keyword search
  terms, favicon bitmap bytes, and icon mappings round-trip through the
  production parser.
- Chromium `Favicons` fixture writer overwrites an existing invalid file with a
  queryable companion schema and does not retain duplicate rows on rewrite.
- T6 asserts Chromium `downloads` rows land in archive `downloads` with source
  id, paths, byte counts, state, MIME fields, and Unix-ms start time preserved.
- T7 asserts `keyword_search_terms` rows land in archive `search_terms` linked
  to the canonical URL with term text, normalized term, profile id, and keyword
  id preserved.
- T8 asserts favicon page URLs match canonical URL rows and identical synthetic
  PNG payload bytes deduplicate into one `favicon_blobs` row.
- T9 asserts multiple `icon_mapping` rows for one icon create separate page URL
  favicon rows while preserving the shared icon URL.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-fixtures write_favicons_overwrites_existing_file_with_companion_schema
cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-fixtures chromium --tests
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core chromium_sidecars --lib
```

Actual output:

```text
browser-history-fixtures:
write_favicons_overwrites_existing_file_with_companion_schema: 1 passed
chromium_roundtrip: 4 passed

vault-core:
Test result: ok. 4 passed; 0 failed; 651 filtered out.
```

Full checkpoint gate:

```text
bun run check
JS unit: Test Files 277 passed (277), Tests 2076 passed (2076)
JS coverage: statements 99.37 | branches 98.59 | functions 99.66 | lines 99.72
Rust coverage: 100% for 35596 instrumented source lines and 1643 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed.

### Module 8B: `WORK-IMPORT-TEST-MINOR-A`

Added 5 focused Rust behavior tests:

- E10 asserts Chromium URL `visit_count` and `typed_count` values round-trip for
  both zero-count typed URLs and nonzero visited URLs.
- E11 asserts a dangling Chromium `from_visit` reference is preserved verbatim
  instead of being rewritten to NULL or 0.
- E12 asserts Chromium `visits.visit_duration` lands unchanged in the current
  archive `visits.visit_duration_ms` column.
- E13 asserts Safari `history_visits.synthesized` persists to the cold
  source-evidence DB as `safari.synthesized` with source ids and `source_field`
  intact.
- E14 asserts Firefox `moz_historyvisits.visit_type` values land in
  `visits.transition_type` without Chromium normalization.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core e1 --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```

Actual output:

```text
e1 filter: 6 passed; 0 failed; 654 filtered out (existing E1 + new E10-E14)
vault-core lib: 660 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full checkpoint gate:

```text
bun run check
JS unit: Test Files 277 passed (277), Tests 2076 passed (2076)
JS coverage: statements 99.38 | branches 98.61 | functions 99.66 | lines 99.72
Rust coverage: 100% for 35805 instrumented source lines and 1648 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed.

### Module 8C: `WORK-IMPORT-TEST-PARSER-ORDERING-A`

Added 1 focused Rust behavior test:

- `chunk_consumer_skips_visits_when_url_batch_has_not_populated_the_map` asserts
  the current `ArchiveChunkConsumer::visits` contract when a parser emits a
  visit before its URL batch: the visit is skipped silently, no canonical visit
  row is inserted, skipped progress increments, imported/duplicate progress
  stays at zero, and `last_visit_id` is not advanced.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core chunk_consumer_skips_visits_when_url_batch_has_not_populated_the_map --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```

Actual output:

```text
parser-ordering targeted: 1 passed; 0 failed; 660 filtered out
vault-core lib: 661 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full checkpoint gate:

```text
bun run check
JS unit: Test Files 277 passed (277), Tests 2076 passed (2076)
JS coverage: statements 99.37 | branches 98.59 | functions 99.66 | lines 99.72
Rust coverage: 100% for 35835 instrumented source lines and 1649 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Gate note: the first full `bun run check` attempt hit a non-reproducible
existing `settings-shell-b` navigation timing failure under `coverage:js`.
The targeted test passed without and with coverage, standalone `coverage:js`
passed, and the subsequent full `bun run check` passed.

Suspected product bugs: none confirmed.

### Module 8D: `WORK-IMPORT-TEST-CONCURRENCY-A`

Added 1 focused Rust behavior test:

- `same_profile_writer_waits_for_committed_watermark` asserts same-profile
  concurrent archive writers are serialized at the SQLite transaction boundary:
  the second writer cannot read `profile_watermarks` while the first writer's
  transaction is uncommitted, and it observes the committed cursor after the
  first writer commits.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core same_profile_writer_waits_for_committed_watermark --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```

Actual output:

```text
concurrency targeted: 1 passed; 0 failed; 661 filtered out
vault-core lib: 662 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full checkpoint gate:

```text
bun run check
JS unit: 277 files passed; 2076 tests passed
JS desktop contract: 5 files passed; 26 tests passed; coverage 100% statements/branches/functions/lines
JS coverage: statements 99.38%; branches 98.61%; functions 99.66%; lines 99.72%
Rust workspace tests: vault-core 662 passed in base; vault-core 663 passed under coverage cfg
Rust coverage: 100% for 35835 instrumented source lines and 1649 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 score; 64 mutants; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. The audit found no separate app-level
ingest queue; the current same-profile guarantee is SQLite writer-lock
serialization after `upsert_source_profile`, documented in
`import-dedup-audit.md` §4.1.

### Module 8E: `WORK-MAINT-IMPORT-EDGE-CASES-SPLIT-A`

Maintained the import edge-case scenario safety net while reducing the oversized
owner file:

- `dedup_scenarios_edge_cases.rs` now holds only shared fixture/env helpers and
  child module declarations.
- Test bodies moved into focused owners:
  `chromium_contracts`, `empty_and_resilience`, `time_and_nullable`,
  `unicode_and_flags`, and `minor_data_integrity`.
- All 19 existing edge-case test names and assertions were preserved.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive::ingest::dedup_scenarios_edge_cases --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```

Actual output:

```text
edge-case targeted: 19 passed; 0 failed; 643 filtered out
vault-core lib: 662 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full checkpoint gate:

```text
bun run check
JS unit: 277 files passed; 2076 tests passed
JS desktop contract: 5 files passed; 26 tests passed; coverage 100% statements/branches/functions/lines
JS coverage: statements 99.37%; branches 98.59%; functions 99.66%; lines 99.72%
Rust workspace tests: vault-core 662 passed in base; vault-core 663 passed under coverage cfg
Rust coverage: 100% for 35835 instrumented source lines and 1660 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 score; 64 mutants; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. This was a maintainability-only split;
audit §4 / §6 links now point at the focused owner modules.

### Module 8F: `WORK-MAINT-IMPORT-INGEST-FACADE-SPLIT-A`

Maintained the ingest orchestrator safety net while reducing the oversized
facade:

- `ingest/mod.rs` now contains the production facade only: profile selection,
  stream dispatch, chunk consumer, watermark advancement, and source-evidence
  plan persistence.
- The embedded low-level regression suite moved to `ingest/core_tests.rs`.
- A responsibility map in `import-dedup-audit.md` records why production
  `ArchiveChunkConsumer` and source-evidence persistence stayed together for
  now.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core archive::ingest::core_tests --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```

Actual output:

```text
core_tests targeted: 7 passed; 0 failed; 655 filtered out
vault-core lib: 662 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full checkpoint gate:

```text
bun run check
unit: 277 files passed; 2076 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.38%, branches 98.61%, functions 99.66%, lines 99.72%
Rust workspace tests: vault-core 662 passed; vault-platform 46 passed; vault-worker 70 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. This was a maintainability-only split;
test assertions and product ingest semantics were preserved.

### Module 9A: `vault-core::migration` import refusal fault paths

Added two behavior assertions for whole-app import refusal ordering:

- Wrong encrypted source key must fail with the typed invalid-key prefix while
  preserving the existing target archive, derived marker, and all non-`.bak`
  paths.
- Payload hash mismatch must fail before preservation/install renames, leaving
  the existing target archive and derived marker untouched and creating no
  `.bak-*` sidecars.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core migration::fault_tests --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core --lib
```

Actual output:

```text
migration::fault_tests: 2 passed; 0 failed; 662 filtered out
vault-core lib: 664 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full checkpoint gate:

```text
bun run check
unit: 277 files passed; 2076 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.38%, branches 98.61%, functions 99.66%, lines 99.72%
Rust workspace tests: vault-core 664 passed; vault-platform 46 passed; vault-worker 70 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. These tests pin the current intended
fail-fast behavior before the live project tree is renamed or overwritten.

### Module 9B: `vault-core::app_lock` and `vault-platform::keyring`

Added two behavior assertions for security refusal and recovery paths:

- Malformed App Lock session state fails closed for status and unlock, malformed
  secret hydration fails, and clearing the passcode removes the corrupted state
  and secret files so the user can recover.
- Provider API keys do not satisfy database-key saved status, and clearing a
  provider key does not remove the database key.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core malformed_app_lock_files_fail_closed_until_passcode_is_cleared --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform provider_key_does_not_satisfy_database_key_status_or_clear_database_secret --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-core app_lock --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform keyring --lib
```

Actual output:

```text
app_lock targeted: 1 passed; 0 failed; 664 filtered out
keyring targeted: 1 passed; 0 failed; 46 filtered out
app_lock module: 5 passed; 0 failed; 660 filtered out
keyring module: 4 passed; 0 failed; 43 filtered out
```

Full checkpoint gate:

```text
bun run check
unit: 277 files passed; 2076 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.38%, branches 98.61%, functions 99.66%, lines 99.72%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. These tests pin fail-closed behavior for
corrupted lock files while keeping the documented recovery path available, and
they prevent provider API key storage from being mistaken for database key
storage.

### Module 9C: `vault-platform::scheduler`

Strengthened one host-command failure behavior:

- macOS `launchctl bootstrap` failure must leave `apply_schedule` non-applied,
  preserve the generated plist, write an audit artifact with the bootstrap
  status, return an error verification check, and make the next
  `schedule_status` report `permission-warning` instead of `installed`.

Commands:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform macos_apply_schedule_reports_bootstrap_failures_without_erroring --lib
cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform scheduler --lib
```

Actual output:

```text
macOS bootstrap-failure targeted: 1 passed; 0 failed; 46 filtered out
scheduler module: 35 passed; 0 failed; 12 filtered out
```

Full checkpoint gate:

```text
bun run check
unit: 277 files passed; 2076 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.38%, branches 98.61%, functions 99.66%, lines 99.72%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. The scheduler audit found existing
Windows/macOS host-command failure coverage for access denied, missing tasks,
mismatches, loaded-without-file states, and manual Linux paths; this checkpoint
tightens the macOS failed-bootstrap recovery/status contract.

### Module 10A: JS Search/Browse chip and result residuals

Added focused frontend behavior assertions and removed two dead defensive guards:

- Advanced search help now has direct coverage for its default test id plus the
  keyboard focus/blur popover path.
- Search-filter helpers now pin escaped quotes inside quoted operators,
  smart-quoted operands, one-character operands, and quoted-empty operands that
  must not surface as chips.
- Search result rows now assert unrelated keys do not select the row, missing
  transition labels stay hidden, and title fallback proceeds from title to URL
  to domain.
- Paper search panel now has a child-contract test proving stale remove ids are
  ignored and focus scheduling tolerates a child that does not attach the input
  ref.
- Deleted the unreachable `highlightQuery` regex `try/catch` and the
  unreachable `appendFilterOperator` unchanged-query guard. Query tokens are
  escaped before regex construction, and the panel only appends valid hard-coded
  `tag` / `note` operators.

Commands:

```sh
bunx vitest run src/components/explorer-paper/paper-advanced-search-help.test.tsx src/components/explorer-paper/paper-search-hero.test.tsx src/pages/explorer/paper-search-filters.test.ts src/components/explorer-paper/paper-search-result.test.tsx src/pages/explorer/paper-search-panel.test.tsx src/pages/explorer/paper-search-panel.child-contract.test.tsx --coverage --coverage.include=src/components/explorer-paper/paper-advanced-search-help.tsx --coverage.include=src/pages/explorer/paper-search-filters.ts --coverage.include=src/components/explorer-paper/paper-search-result.tsx --coverage.include=src/pages/explorer/paper-search-panel.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Test Files 6 passed (6)
Tests 60 passed (60)
Targeted coverage for paper-advanced-search-help.tsx, paper-search-filters.ts,
paper-search-result.tsx, and paper-search-panel.tsx:
statements 100 | branches 100 | functions 100 | lines 100
```

Full checkpoint gate:

```text
bun run check
unit: 279 files passed; 2085 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.48%, branches 98.75%, functions 99.72%, lines 99.77%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Gate note: the first full `bun run check` attempt in the command sandbox failed
only in mockito-backed Rust tests because local test servers could not bind
sockets (`Operation not permitted`). Re-running the same command with normal
local permissions passed.

Suspected product bugs: none confirmed.

### Module 10B: JS Explorer route and paper-view residuals

Added 10 focused frontend behavior assertions and removed route/helper branches
that the current public grammar cannot exercise:

- `PaperExplorerView` now pins calendar day selection, non-Escape key handling,
  invalid active-date labels, omitted `todayIso` current-day fallback, malformed
  clock-format events, locale-format fallback/no-crash behavior, and decorated
  pagination / infinite-scroll descriptors.
- `PaperDetailPanelMount` now asserts "All of domain" forwards the selected
  domain and closes the panel.
- `ExplorerPage` now asserts enabled og:image settings forward their configured
  fetch mode and desktop command transport selects the desktop annotation store.
- Deleted the route-only paginated `PaperExplorerView` prop, the redundant
  `|| results` render guard, and the unreachable `infiniteDisabled ? undefined`
  branch at the Browse route mount point. The route grammar already sends every
  `infiniteDisabled` surface to Search, grouped views, invalid-regex callouts,
  or locked/uninitialized states before Browse mounts.
- Simplified Paper search day-group sorting and Paper view density override
  helpers to remove equality / `Map.has` branches that were unreachable under
  the helper contracts.

Commands:

```sh
bunx vitest run src/pages/explorer/index.test.tsx src/pages/explorer/paper-view.test.tsx src/pages/explorer/paper-detail-panel-mount.test.tsx src/pages/explorer/paper-view-helpers.test.ts src/pages/explorer/paper-search-helpers.test.ts
bunx tsc -b --pretty false
bun run coverage:js
```

Actual output:

```text
Targeted tests:
Test Files 5 passed (5)
Tests 94 passed (94)

coverage:js:
Test Files 279 passed (279)
Tests 2095 passed (2095)
All files statements 99.6 | branches 99 | functions 99.84 | lines 99.88

Module 10B files now at 100/100/100/100:
src/pages/explorer/index.tsx
src/pages/explorer/paper-detail-panel-mount.tsx
src/pages/explorer/paper-search-helpers.ts
src/pages/explorer/paper-view-helpers.ts
src/pages/explorer/paper-view.tsx
```

Full checkpoint gate:

```text
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2095 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.61%, branches 99.02%, functions 99.84%, lines 99.88%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed.

### Module 10C: JS Dashboard/app shell residuals

Added 6 focused frontend behavior assertions and removed public-grammar
duplicate branches:

- Exported and tested the route registry lookup guard so unknown route handles
  fail loudly instead of silently picking the wrong shell screen.
- Pinned epigraph storage values that omit the date/index separator so daily
  rotation overwrites malformed local state.
- Pinned `PKSearchPalette` whitespace-only queries so the palette stays on the
  empty branch and does not call the backend search path.
- Pinned `useRouteHistoryNav` under `StrictMode` so duplicate effect mounts do
  not move the shell history stack twice for the same location key.
- Pinned Dashboard On This Day target-date formatting for non-English resolved
  locales.
- Pinned Dashboard archive-access fallback cleanup so a queued non-probe clear
  cannot update state after unmount.
- Deleted redundant `shellStorage()` SSR fallback, the shell-level blank search
  guard that `PKSearchPalette` already makes unreachable, the redundant
  `archiveHealthy ?? false` fallback, and the unreachable
  `domainAbbreviation().split('.')[0] ?? cleaned` fallback.

Commands:

```sh
bunx vitest run src/app/index-tests/router-structure.test.tsx src/app/shell-helpers.test.ts src/app/shell.test.tsx src/components/shell/pk-search-palette.test.tsx src/components/shell/use-route-history-nav.test.tsx src/pages/dashboard/on-this-day-card.test.tsx src/pages/dashboard/route-fallback.test.tsx
bunx tsc -b --pretty false
bunx vitest run src/app/index-tests/router-structure.test.tsx src/app/shell-helpers.test.ts src/app/shell.test.tsx src/components/shell/pk-search-palette.test.tsx src/components/shell/use-route-history-nav.test.tsx src/pages/dashboard/on-this-day-card.test.tsx src/pages/dashboard/route-fallback.test.tsx --coverage --coverage.include=src/app/router.tsx --coverage.include=src/app/shell-helpers.ts --coverage.include=src/app/shell.tsx --coverage.include=src/components/shell/pk-search-palette.tsx --coverage.include=src/components/shell/use-route-history-nav.ts --coverage.include=src/pages/dashboard/on-this-day-card.tsx --coverage.include=src/pages/dashboard/route-fallback-access.ts --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
bun run coverage:js
```

Actual output:

```text
Targeted tests:
Test Files 7 passed (7)
Tests 109 passed (109)

Targeted 10C coverage:
All files 100/100/100/100
src/app/router.tsx 100/100/100/100
src/app/shell-helpers.ts 100/100/100/100
src/app/shell.tsx 100/100/100/100
src/components/shell/pk-search-palette.tsx 100/100/100/100
src/components/shell/use-route-history-nav.ts 100/100/100/100
src/pages/dashboard/on-this-day-card.tsx 100/100/100/100
src/pages/dashboard/route-fallback-access.ts 100/100/100/100

coverage:js:
Test Files 279 passed (279)
Tests 2101 passed (2101)
All files statements 99.63 | branches 99.09 | functions 99.84 | lines 99.89

Full coverage groups now at 100/100/100/100:
src/app
src/components/shell
src/pages/dashboard
```

Full checkpoint gate:

```text
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2101 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.64%, branches 99.11%, functions 99.84%, lines 99.89%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed.

### Module 10D: JS Explorer hook/helper residuals

Added 15 focused hook/helper behavior assertions and removed duplicate private
guards that the public hook grammar could not exercise:

- `url-state-derivations` now pins keyword grouped views, positive page parsing,
  empty query/cursor history payloads, null query signatures, semantic/session
  active-filter chips, bare recent-search labels, and incomplete date shortcut
  ranges.
- `useDesktopAnnotations` now treats missing backend `tags` as an empty list
  during hydration instead of leaking `undefined` into the UI state.
- `useBrowseDayInsightsCache` now pins stale success/failure replies after a
  refresh-key rotation and removed a private duplicate request guard already
  made unreachable by `resolve()`'s cache-entry check.
- `useExplorerArchiveDensity` now pins stale success/failure replies, cancelled
  not-ready reset microtasks, available-year-only bounds, and no-finite-year
  responses. The removed sequence ref duplicated effect cleanup cancellation.
- `useExplorerData` now pins cached adjacent prefetch reuse, stale multi-page
  prefetch cancellation, cancellation before the transition paint, and empty
  semantic recall results without inventing a selected row.

Commands:

```sh
bunx vitest run src/pages/explorer/url-state-derivations.test.ts src/pages/explorer/use-desktop-annotations.test.tsx src/pages/explorer/hooks/use-browse-day-insights-cache.test.tsx src/pages/explorer/hooks/use-explorer-archive-density.test.tsx src/pages/explorer/hooks/use-explorer-data.test.tsx
bunx vitest run src/pages/explorer/url-state-derivations.test.ts src/pages/explorer/use-desktop-annotations.test.tsx src/pages/explorer/hooks/use-browse-day-insights-cache.test.tsx src/pages/explorer/hooks/use-explorer-archive-density.test.tsx src/pages/explorer/hooks/use-explorer-data.test.tsx --coverage --coverage.include=src/pages/explorer/url-state-derivations.ts --coverage.include=src/pages/explorer/use-desktop-annotations.ts --coverage.include=src/pages/explorer/hooks/use-browse-day-insights-cache.ts --coverage.include=src/pages/explorer/hooks/use-explorer-archive-density.ts --coverage.include=src/pages/explorer/hooks/use-explorer-data.ts --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Targeted tests:
Test Files 5 passed (5)
Tests 58 passed (58)

Targeted 10D coverage:
All files 100/100/100/100
src/pages/explorer/url-state-derivations.ts 100/100/100/100
src/pages/explorer/use-desktop-annotations.ts 100/100/100/100
src/pages/explorer/hooks/use-browse-day-insights-cache.ts 100/100/100/100
src/pages/explorer/hooks/use-explorer-archive-density.ts 100/100/100/100
src/pages/explorer/hooks/use-explorer-data.ts 100/100/100/100
```

Full checkpoint gate:

```text
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2114 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.69%, branches 99.21%, functions 99.84%, lines 99.91%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed.

### Module 10E: JS Explorer runtime hook and paper helper residuals

Added 24 focused behavior assertions and removed redundant branches inside
Explorer runtime hooks/helpers:

- `useExplorerInfinitePages` now pins disabled/no-head dormancy, same-key buffer
  retention, cancelled rejected foreground pages, directional prefetch failure,
  directional prefetch reuse, and the hard-cap/page-count decision through a
  pure derivation helper. The hook now treats `pageCount` as the numbered-page
  contract instead of mixing in cursor-style `hasNext`.
- `useExplorerOgImages` now pins empty result windows, cache-token reset of the
  pending mark-shown batch, and unmount timer cleanup. Redundant enqueued-url
  tracking was removed because the visible URL dedupe, inflight set, and cache
  map already prevent duplicate local reads/refetch enqueues.
- `useScrollDirection` now pins stable same-direction samples, RAF dedupe, and
  pending-frame cleanup.
- `useViewportMount` now pins no-ref setup, empty observer callbacks,
  zero-height recycle, and re-entry measurement behavior.
- `groupEntriesByDay`/format helpers now pin out-of-range date fallback and
  hour12 formatting. Private empty-session and manual date-comparator branches
  were simplified under the public grouping contract.
- `ExplorerDetailPanel` now pins loading, empty, missing metadata, null-domain,
  and null-profile detail-rail branches.

Commands:

```sh
bunx vitest run src/pages/explorer/hooks/use-explorer-infinite-pages.test.tsx src/pages/explorer/hooks/use-explorer-og-images.test.tsx src/pages/explorer/hooks/use-scroll-direction.test.tsx src/pages/explorer/hooks/use-viewport-mount.test.tsx src/pages/explorer/paper/group-entries.test.ts src/pages/explorer/panels/detail-panel.test.tsx
bunx vitest run src/pages/explorer/hooks/use-explorer-infinite-pages.test.tsx src/pages/explorer/hooks/use-explorer-og-images.test.tsx src/pages/explorer/hooks/use-scroll-direction.test.tsx src/pages/explorer/hooks/use-viewport-mount.test.tsx src/pages/explorer/paper/group-entries.test.ts src/pages/explorer/panels/detail-panel.test.tsx --coverage --coverage.include=src/pages/explorer/hooks/use-explorer-infinite-pages.ts --coverage.include=src/pages/explorer/hooks/use-explorer-og-images.ts --coverage.include=src/pages/explorer/hooks/use-scroll-direction.ts --coverage.include=src/pages/explorer/hooks/use-viewport-mount.ts --coverage.include=src/pages/explorer/paper/group-entries.ts --coverage.include=src/pages/explorer/panels/detail-panel.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Targeted tests:
Test Files 6 passed (6)
Tests 81 passed (81)

Targeted 10E coverage:
All files 100/100/100/100
src/pages/explorer/hooks/use-explorer-infinite-pages.ts 100/100/100/100
src/pages/explorer/hooks/use-explorer-og-images.ts 100/100/100/100
src/pages/explorer/hooks/use-scroll-direction.ts 100/100/100/100
src/pages/explorer/hooks/use-viewport-mount.ts 100/100/100/100
src/pages/explorer/paper/group-entries.ts 100/100/100/100
src/pages/explorer/panels/detail-panel.tsx 100/100/100/100
```

Full checkpoint gate:

```text
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2138 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.87%, branches 99.56%, functions 99.87%, lines 99.92%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed.

### Module 10F: JS Explorer Paper component residuals

Added 11 focused behavior assertions and removed redundant private branches in
Explorer Paper presentation/runtime components:

- `PaperCalendarPopover` now pins low- and medium-density hover spark opacity
  instead of relying on the year picker to incidentally walk density tiers.
- `aggregateDayInsights` now pins empty URL handling, empty domain+URL fallback,
  invalid timestamp handling, exact-hour durations, and the `Intl.NumberFormat`
  unit-format fallback.
- `PaperDayInsights` now pins a details disclosure with no peak-hour row, host
  time-formatting failures, and long revisited-URL truncation.
- `PaperListRow` now pins og:image as the list icon fallback when favicon bytes
  are absent.
- Redundant branches were simplified where the public render grammar already
  guarantees the ref or handler: assistant auto-scroll, contact-sheet toolbar
  measurement, infinite-scroll footer `canLoadMore`, detail-panel pending flush
  and look-further rows, filter-strip outside-click container, and top-domain
  max-visit lookup.

Commands:

```sh
bunx vitest run src/components/explorer-paper/paper-assistant-view.test.tsx src/components/explorer-paper/paper-calendar-popover.test.tsx src/components/explorer-paper/paper-contact-sheet.test.tsx src/components/explorer-paper/paper-contact-sheet.behavior.test.tsx src/components/explorer-paper/paper-contact-sheet.virt.test.tsx src/components/explorer-paper/paper-day-insights-helpers.test.ts src/components/explorer-paper/paper-day-insights.test.tsx src/components/explorer-paper/paper-detail-panel.test.tsx src/components/explorer-paper/paper-filter-strip.test.tsx src/components/explorer-paper/paper-browse-primitives.test.tsx
bunx vitest run src/components/explorer-paper/paper-assistant-view.test.tsx src/components/explorer-paper/paper-calendar-popover.test.tsx src/components/explorer-paper/paper-contact-sheet.test.tsx src/components/explorer-paper/paper-contact-sheet.behavior.test.tsx src/components/explorer-paper/paper-contact-sheet.virt.test.tsx src/components/explorer-paper/paper-day-insights-helpers.test.ts src/components/explorer-paper/paper-day-insights.test.tsx src/components/explorer-paper/paper-detail-panel.test.tsx src/components/explorer-paper/paper-filter-strip.test.tsx src/components/explorer-paper/paper-browse-primitives.test.tsx --coverage --coverage.include=src/components/explorer-paper/paper-assistant-view.tsx --coverage.include=src/components/explorer-paper/paper-calendar-popover.tsx --coverage.include=src/components/explorer-paper/paper-contact-sheet.tsx --coverage.include=src/components/explorer-paper/paper-day-insights-helpers.ts --coverage.include=src/components/explorer-paper/paper-day-insights.tsx --coverage.include=src/components/explorer-paper/paper-detail-panel.tsx --coverage.include=src/components/explorer-paper/paper-filter-strip.tsx --coverage.include=src/components/explorer-paper/paper-list-row.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Targeted tests:
Test Files 10 passed (10)
Tests 173 passed (173)

Targeted 10F coverage:
All files 100/100/100/100
src/components/explorer-paper/paper-assistant-view.tsx 100/100/100/100
src/components/explorer-paper/paper-calendar-popover.tsx 100/100/100/100
src/components/explorer-paper/paper-contact-sheet.tsx 100/100/100/100
src/components/explorer-paper/paper-day-insights-helpers.ts 100/100/100/100
src/components/explorer-paper/paper-day-insights.tsx 100/100/100/100
src/components/explorer-paper/paper-detail-panel.tsx 100/100/100/100
src/components/explorer-paper/paper-filter-strip.tsx 100/100/100/100
src/components/explorer-paper/paper-list-row.tsx 100/100/100/100
```

Full checkpoint gate:

```text
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2149 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 99.94%, branches 99.76%, functions 99.87%, lines 99.95%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed.

### Module 10G: JS shared component, preview facade, and route residuals

Added or extended 14 focused behavior assertions and removed two redundant
private branches in the final JS coverage residual set:

- `YearHeatmap` click behavior now depends on a single observable clickable
  predicate, while covered tests still pin count-bearing date selection.
- Browsing-rhythm calendar/card tests now pin disabled year navigation,
  missing reset shortcuts, ready-state summaries, zero-visit cells, and
  selection-prompt behavior.
- Background progress/status tests now pin empty labels, log-only detail rows,
  AI queue fallback totals, and warning precedence.
- Browser-preview facade tests now pin og:image default hydration, dashboard
  earliest/latest ordering stability, and the legacy unsupported
  `prefetch_og_images` rejection instead of expanding `src/lib/backend.ts`.
- Paper assistant/audit/import route tests now pin paper-layout send behavior,
  providerless fallback attribution, manifest-chain selection, and browser/file
  method normalization.
- Intelligence and settings residuals now pin title fallback rendering and the
  data-migration apply path without retaining an unreachable phase guard.

Commands:

```sh
bunx vitest run src/components/heatmap/year-heatmap.test.tsx src/components/intelligence/browsing-rhythm-calendar.test.tsx src/components/intelligence/browsing-rhythm-card.test.tsx src/components/primitives/background-progress.test.tsx src/components/sidebar/background-status.test.tsx src/lib/backend-preview-shell-commands.test.ts src/lib/backend-preview-showcase.test.ts src/lib/backend-tests/preview-workflows.test.ts src/pages/intelligence-surfaces/assistant-and-shell.test.tsx src/pages/audit/index.test.tsx src/pages/import/index.test.tsx src/pages/intelligence/paper-intelligence-panel.test.tsx src/pages/settings/data-migration-section.test.tsx
bunx vitest run src/lib/backend-preview-showcase.test.ts src/pages/import/index.test.tsx
bun run coverage:js
```

Actual output:

```text
Focused 10G sweep:
Test Files 13 passed (13)
Tests 103 passed (103)

Final residual recheck:
Test Files 2 passed (2)
Tests 17 passed (17)

JS coverage:
Test Files 279 passed (279)
Tests 2162 passed (2162)
All files statements 100%, branches 100%, functions 100%, lines 100%
No uncovered JS lines, branches, or functions in lcov.info.
```

Full checkpoint gate:

```text
bun run check
format/lint/i18n/typecheck: passed
unit: 279 files passed; 2162 tests passed
desktop contract: 5 files passed; 26 tests passed; coverage 100/100/100/100
JS coverage: All files statements 100%, branches 100%, functions 100%, lines 100%
Rust workspace tests: vault-core 665 passed; vault-platform 47 passed; vault-worker 70 passed
Rust coverage cfg tests: vault-core 666 passed; vault-platform 49 passed; vault-worker 80 passed
Rust coverage: verified at 100% for 35459 instrumented source lines and 1652 source functions
supply-chain/platform/release checks: passed
build: passed
browser E2E: 4 passed
desktop bridge E2E: 3 passed
desktop contract mutation: 64 mutants; 100.00 score; 0 survived; 0 timed out
```

Suspected product bugs: none confirmed. Drift closed as `QA-GAP-001`; the JS
runtime coverage floor is now the documented 100/100/100/100 gate.

### Module 6: `src/app/shell.tsx` and `src/components/shell/*`

Added 15 behavior assertions across the global shell and shell chrome:

- Background busy payloads render the non-blocking progress strip and do not
  show the blocking overlay.
- Malformed preference-change events leave the current theme chrome unchanged.
- Dashboard totals, last-archive telemetry, and source color fallback order are
  visible in the status bar.
- Backend palette errors and missing `items` payloads render the no-results
  state instead of silently passing.
- URL-only, titleless, URL-less, and no-visit-date palette hits map to visible,
  selectable results and route to Explorer as expected.
- Escape actually closes the palette; Manage Sources actually routes to
  Settings; stale palette searches do not leak late success or failure results.
- Status bar epigraph/profile-label fallbacks, topbar navigator fallback, route
  replace navigation, blank-UA shortcut labels, and `isContentEditable` shortcut
  suppression are pinned with observable assertions.

Commands:

```sh
bunx vitest run src/app/shell.test.tsx src/components/shell/pk-search-palette.test.tsx src/components/shell/pk-status-bar.test.tsx src/components/shell/pk-topbar.test.tsx src/components/shell/use-route-history-nav.test.tsx --coverage --coverage.include=src/app/shell.tsx --coverage.include=src/components/shell/pk-search-palette.tsx --coverage.include=src/components/shell/pk-status-bar.tsx --coverage.include=src/components/shell/pk-topbar.tsx --coverage.include=src/components/shell/use-route-history-nav.ts --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Test Files  5 passed (5)
Tests       68 passed (68)

Targeted shell coverage:
All files: statements 99.2 | branches 97.79 | functions 100 | lines 100
shell.tsx: statements 98.8 | branches 95.58 | functions 100 | lines 100
pk-search-palette.tsx: statements 100 | branches 97.14 | functions 100 | lines 100
pk-status-bar.tsx: statements 100 | branches 100 | functions 100 | lines 100
pk-topbar.tsx: statements 100 | branches 100 | functions 100 | lines 100
use-route-history-nav.ts: statements 98.83 | branches 98.27 | functions 100 | lines 100
```

Full checkpoint gate:

```text
bun run check
JS unit: Test Files 277 passed (277), Tests 2070 passed (2070)
JS coverage: statements 99.36 | branches 98.51 | functions 99.66 | lines 99.72
Rust coverage: 100% for 35246 instrumented source lines and 1634 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed. Drift recorded as `QA-GAP-006` for the
remaining defensive/redundant branches that are not naturally reachable through
the current public shell grammar.

### Module 7: `src/pages/dashboard/index.tsx`

Added 6 focused behavior tests:

- archive span uses the current day when `latestVisitAt` is missing, instead of
  rendering the missing-span placeholder.
- read-model derived archive state renders zero-size fallback, encrypted mode,
  source count, database-path fallback, and missing manifest hash.
- `getOnThisDay()` null data renders the empty state instead of preserving stale
  entries.
- stale successful and failed On This Day responses are discarded after the
  route becomes uninitialized.
- On This Day entries/header actions, year heatmap actions, and active-thread
  actions navigate to their route-level destinations.
- morning greeting branch is covered alongside the existing afternoon/evening
  branches.

Commands:

```sh
bunx vitest run src/pages/dashboard/index.test.tsx --coverage --coverage.include=src/pages/dashboard/index.tsx --coverage.thresholds.lines=0 --coverage.thresholds.branches=0 --coverage.thresholds.functions=0 --coverage.thresholds.statements=0
```

Actual output:

```text
Test Files  1 passed (1)
Tests       17 passed (17)

Targeted dashboard coverage:
All files: statements 100 | branches 100 | functions 100 | lines 100
index.tsx: statements 100 | branches 100 | functions 100 | lines 100
```

Full checkpoint gate:

```text
bun run check
JS unit: Test Files 277 passed (277), Tests 2076 passed (2076)
JS coverage: statements 99.38 | branches 98.61 | functions 99.66 | lines 99.72
Rust coverage: 100% for 35246 instrumented source lines and 1634 source functions
Browser E2E: 4 passed
Desktop bridge E2E: 3 passed
Desktop contract mutation: 100.00 mutation score, 64 mutants, 0 survived
```

Suspected product bugs: none confirmed.
