# QA Gate Handoff - 2026-04-27

This document is the reset point for the strict checker restoration work. It is
written for a blank-context agent taking over `WORK-QA-GATE-A`.

> 2026-04-27 update: the gate policy changed after current-host timing
> measurement and explicit user approval. Treat `docs/plan/STATUS.md`,
> `docs/plan/program/quality-matrix.md`, `TESTING.md`, `AGENTS.md`, and
> `package.json` as newer than the original strict-mutation text below.
> `bun run check` remains the authoritative per-commit gate, but it now contains
> 100% JS/Rust coverage plus lightweight desktop-contract JS mutation. Full
> frontend Stryker and whole-workspace Rust cargo-mutants moved to
> `check:deep` / scheduled/manual mutation workflows because they are
> multi-hour-scale on this checkout and Rust mutation currently has a
> copy-sandbox fixture-path failure.

## Current State

- Active block: `docs/plan/STATUS.md` first unchecked block is still
  `WORK-QA-GATE-A - Restore Strict Checker Gates`.
- Gate state: `bun run check` is not green yet. Do not close the block, do not
  append it to `CHANGELOG.md`, and do not commit as finished.
- Commit state: no atomic commits have been made for the current large QA-gate
  worktree. The repo rule remains: commit only after the real checker is green.
- Authoritative acceptance remains:
  - `bun run check`
  - `bun run verify`
- This handoff file itself adds one more untracked path. Immediately before this
  file was created, `git status --short` showed 343 changed paths: 238 tracked
  changed/deleted paths plus 105 untracked paths.

## What I Was Doing

I was restoring the strict QA gate after coverage and mutation had been
temporarily relaxed during earlier refactors. The intended final contract is:

- `bun run check` is the full local and CI gate.
- `check:base` is only a triage helper.
- JS coverage and JS mutation cover all active `src/**/*.{ts,tsx}` runtime
  source at 100%.
- Rust coverage covers full `src-tauri/**/src/*.rs` source at 100%.
- Rust mutation uses whole-workspace `cargo mutants`; surviving mutants fail
  unless narrowly proven equivalent or inapplicable.
- Browser build, browser e2e, desktop-bridge truth e2e, JS mutation, and Rust
  mutation all belong inside the checker.

The last active implementation focus was frontend mutation hardening. A focused
Stryker run over the app cluster had many surviving mutants; I was tightening
router and app-shell assertions before moving into components and pages.

## Script And Config Wiring Already Done

Current `package.json` script wiring:

```text
check: bun run check:base && bun run check:coverage && bun run build && bun run test:e2e && bun run test:e2e:desktop-bridge:truth && bun run check:mutation
check:base: bun run check:js && bun run check:desktop-contract && bun run check:rust && bun run check:supply-chain && bun run check:platform
check:coverage: bun run coverage:js && bun run coverage:rust
coverage:js: vitest run --coverage
coverage:rust: bun run coverage:rust:raw && node scripts/verify-rust-coverage.mjs coverage/rust.lcov.info full
check:mutation: bun run mutation
mutation:js: stryker run
mutation:rust: bun run mutation:rust:full
verify: bun run check && bun run desktop:build:debug
```

Other known wiring changes in the worktree:

- `.github/workflows/ci.yml` runs the same effective gate as local
  `bun run check`.
- `.github/workflows/mutation.yml` runs `bun run mutation:rust` for Rust
  mutation instead of the old focused contract.
- `stryker.config.json` mutates active frontend runtime source under `src/` and
  writes JSON to `reports/mutation/js/mutation.json`.
- `vitest.quality.config.ts` is deleted; full active-source Vitest coverage is
  the mainline role.
- `scripts/verify-rust-coverage.mjs` was updated for full Rust coverage
  accounting.
- Docs were updated to describe strict checker truth: `AGENTS.md`, `README.md`,
  `RELEASE.md`, `TESTING.md`, `docs/standards.md`, `docs/plan/README.md`, and
  `docs/plan/program/quality-matrix.md`.

Do not undo this wiring. The remaining work is to make the strict gate pass, not
to relax the gate.

## Verification Record

Known recent successful checks from this work block:

- `bun run typecheck` passed.
- Targeted Vitest batch passed:

```bash
bun run test:unit -- \
  src/app/index-tests/router-structure.test.tsx \
  src/lib/browser-icons.test.tsx \
  src/lib/intelligence-presentation.test.ts \
  src/lib/intelligence-runtime.test.ts \
  src/lib/runtime-diagnostics.test.ts \
  src/lib/wait-for-next-paint.test.ts \
  src/lib/onboarding-estimates.test.ts \
  src/lib/storage-analytics.test.ts
```

The targeted batch result was 8 files passed / 34 tests passed.

Earlier in this QA-gate block:

- `coverage:rust` was driven to 100% for the full Rust source set. Recorded
  verifier summary: `Rust coverage verified at 100% for 28616 instrumented
source lines and 1291 source functions.`
- `coverage:js` was previously reported green at 100% after the full active
  frontend coverage expansion, but it has not been rerun in this handoff
  moment. Rerun it before closeout.
- `test:e2e` and `test:e2e:desktop-bridge:truth` were repaired and reported
  green earlier in this block, but they also need final reruns through
  `bun run check`.

Do not treat any of the above as final acceptance. The final acceptance is only
fresh `bun run check` and `bun run verify`.

## Current Mutation Evidence

Current persistent reports:

- `reports/mutation/js/mutation.json` is absent. A full `bun run mutation:js`
  was interrupted before a current JSON report was produced.
- `reports/mutation/js/index.html` exists but is old and should not be used as
  authoritative evidence.
- `reports/mutation/focus/mutation.json` currently exists and, as of this
  handoff, reports only:

```text
src/app/router.tsx 143 surviving mutants
```

Treat `reports/mutation/focus/mutation.json` as a focused triage report, not the
main gate.

The current temporary focused config is `stryker.focus.config.json`. It is
untracked and targets the app cluster:

- `mutate`: `src/app/**/*.{ts,tsx}` excluding tests and test helpers.
- `testFiles`: app tests under `src/app/**/*.test.{ts,tsx}`,
  `src/app/index-tests/**/*.test.{ts,tsx}`, and
  `src/app/shell-data-tests/**/*.test.{ts,tsx}`.
- JSON reporter: `reports/mutation/focus/mutation.json`.
- thresholds: `high/low/break = 100`.

This file is useful for triage. Remove it before the final commit unless the
team deliberately decides to keep it as a named helper and documents that.

Leftover Stryker sandboxes exist under:

```text
reports/mutation/.stryker-tmp/
reports/mutation/.stryker-focus-tmp/
```

They are generated artifacts. It is safe to clean them before reruns if needed,
but do not treat their contents as source truth.

## Last Mutation Work Completed

A temporary lib-focused Stryker config was used and then deleted. The focused run
over this lib cluster reached 100%:

- `src/lib/browser-icons.tsx`
- `src/lib/intelligence-presentation.ts`
- `src/lib/intelligence-runtime.ts`
- `src/lib/runtime-diagnostics.ts`
- `src/lib/wait-for-next-paint.ts`

Recorded focused score:

```text
All files 100.00
294 killed
1 timeout
0 survived
114 errors
```

Related source/test hardening included:

- `src/lib/browser-icons.test.tsx`
- `src/lib/build-info.test.ts`
- `src/lib/intelligence-runtime.test.ts`
- `src/lib/intelligence-presentation.test.ts`
- `src/lib/onboarding-estimates.test.ts`
- `src/lib/storage-analytics.test.ts`
- `src/lib/runtime-diagnostics.test.ts`
- `src/lib/wait-for-next-paint.test.ts`
- `src/lib/runtime-diagnostics.ts`
- `src/lib/wait-for-next-paint.ts`
- `src/lib/intelligence-presentation.ts`

## Current App Mutation Work

A prior app-cluster focused run had this shape:

- 13 source files.
- 958 mutants.
- 17 test files.
- Dry run: 81 tests passed.
- Final score: 46.37%.
- 281 killed.
- 325 survived.
- 352 errors.

Largest surviving files from that app-cluster run:

```text
src/app/router.tsx 143
src/app/shell-data.tsx 62
src/app/shell-data-actions.ts 41
src/app/shell-data-helpers.ts 31
src/app/shell-runtime-status.ts 19
src/app/shell-route-error-boundary.tsx 11
src/app/route-guards.tsx 9
src/app/shell.tsx 5
src/app/onboarding-shell.tsx 3
src/app/route-hydrate-fallback.tsx 1
```

After that run, `src/app/index-tests/router-structure.test.tsx` was
substantially strengthened:

- It imports `appScreens` and `type AppScreen`.
- It defines exact expected app-shell and onboarding screen descriptors.
- It asserts exact `appScreens`.
- It asserts exact `sidebarSections`.
- It asserts exact `appRoutes` descriptors, including path/index/lazy,
  errorBoundary, handle IDs, and children.
- It adds a `routeDescriptors` helper.
- The targeted router structure test passes.

A router-only Stryker rerun was then started, but the work was interrupted for
this handoff. Do not claim router mutation is fixed until the command below is
rerun and the JSON report confirms it.

## Next Exact Commands

Start with the app router focused mutation rerun:

```bash
bunx stryker run stryker.focus.config.json \
  --mutate src/app/router.tsx \
  --testFiles src/app/index-tests/router-structure.test.tsx \
  --reporters clear-text,json,progress
```

Then parse survivors:

```bash
node -e 'const fs=require("fs");const report=JSON.parse(fs.readFileSync("reports/mutation/focus/mutation.json","utf8"));for(const [file,data] of Object.entries(report.files)){const survivors=data.mutants.filter(m=>m.status==="Survived");if(survivors.length){console.log("\n"+file+" "+survivors.length);const by={};for(const m of survivors){by[m.mutatorName]=(by[m.mutatorName]||0)+1}console.log(Object.entries(by).map(([k,v])=>`${k}:${v}`).join(", "));for(const m of survivors.slice(0,80)){console.log(`${m.location.start.line+1}:${m.location.start.column+1} ${m.mutatorName} -> ${JSON.stringify(m.replacement).slice(0,160)}`)}if(survivors.length>80) console.log(`... ${survivors.length-80} more`)}}'
```

Continue frontend mutation in this order:

1. `src/app/router.tsx`
2. `src/app/shell-data-helpers.ts`
3. `src/app/shell-route-error-boundary.tsx`
4. `src/app/route-guards.tsx`
5. `src/app/route-hydrate-fallback.tsx`
6. `src/app/onboarding-shell.tsx`
7. `src/app/shell.tsx`
8. `src/app/shell-runtime-status.ts`
9. `src/app/shell-data.tsx`
10. `src/app/shell-data-actions.ts`

After the app focused cluster reaches 100%, move to focused component and page
clusters. Only then rerun the full JS mutation gate:

```bash
bun run mutation:js
```

The last interrupted full JS mutation run found 269 source files and 21,849
mutants; dry run passed 963 tests. It was interrupted around 42% / 9,213 tested
with 1,091 survived and 2 timed out. Because it is expensive, do not rerun full
JS mutation blindly after every small edit.

After JS mutation is green, run Rust mutation:

```bash
bun run mutation:rust
```

Then final acceptance:

```bash
bun run check
bun run verify
```

## Worktree Summary

The current diff is large but not random. It falls into these reviewable
categories:

1. Checker and CI wiring:
   - `.github/workflows/ci.yml`
   - `.github/workflows/mutation.yml`
   - `package.json`
   - `stryker.config.json`
   - `vitest.config.ts`
   - deleted `vitest.quality.config.ts`

2. QA docs:
   - `AGENTS.md`
   - `README.md`
   - `RELEASE.md`
   - `TESTING.md`
   - `docs/standards.md`
   - `docs/plan/README.md`
   - `docs/plan/STATUS.md`
   - `docs/plan/program/quality-matrix.md`

3. Rust coverage and mutation hardening:
   - Many `src-tauri/**` source and test files.
   - `scripts/verify-rust-coverage.mjs`.
   - New untracked `src-tauri/src/test_support.rs`.
   - Rust coverage has been pushed to 100%; mutation still needs final sweep.

4. Frontend coverage expansion:
   - Many new test files under `src/app`, `src/components`, `src/lib`, and
     `src/pages`.
   - Many existing tests were strengthened from broad smoke assertions into
     specific behavioral contracts.

5. Product fixes required by tests and mutation:
   - `src/pages/import/index.tsx`: optional `importBatch` handling was hardened.
   - `src/lib/runtime-diagnostics.ts`: rejection reasons are trimmed and
     whitespace-only strings fall back instead of leaking empty reasons.
   - `src/lib/wait-for-next-paint.ts`: idempotent resolve handling was
     simplified and tested.
   - `src/lib/intelligence-presentation.ts` and related tests were tightened.

6. Latest app/router mutation work:
   - `src/app/index-tests/router-structure.test.tsx` now asserts exact app
     screen, sidebar, and route descriptors to kill router object-literal and
     label/href/icon mutants.

## Do Not Redo

- Do not restore `vitest.quality.config.ts` as the main coverage surface.
- Do not narrow `coverage:js` or `mutation:js` back to a small quality slice.
- Do not treat desktop-contract helpers as signed-off full quality gates.
- Do not mark `WORK-QA-GATE-A` complete from docs, targeted tests, or focused
  mutation reports.
- Do not commit `stryker.focus.config.json` or generated Stryker sandboxes
  unless you intentionally convert the focus config into a documented helper.
- Do not lower thresholds. They must remain 100%.
- Do not broad-exclude active runtime source to make the gate pass.

## Commit Plan Once Green

Do not commit until the real gate is green. Once `bun run check` and
`bun run verify` pass, split reviewable conventional commits roughly like this:

1. `chore(checker): restore strict local and ci gates`
2. `test(frontend): harden active-source coverage and mutation contracts`
3. `test(backend): harden rust coverage and mutation contracts`
4. `docs(qa): sync strict checker status and quality matrix`

If the actual final diff boundaries differ, preserve the same principle:
checker wiring, frontend test/product hardening, backend test/product hardening,
and docs truth should be reviewable independently.

## Closeout Requirements

Only after fresh `bun run check` and `bun run verify` both pass:

1. Mark `WORK-QA-GATE-A` complete in `docs/plan/STATUS.md`.
2. Append the completed block to `docs/plan/CHANGELOG.md`.
3. Sync `docs/plan/BACKLOG.md` and
   `docs/plan/program/quality-matrix.md` if the final gate behavior changed.
4. Remove or intentionally document transient focus configs and mutation
   artifacts.
5. Make atomic conventional commits.
