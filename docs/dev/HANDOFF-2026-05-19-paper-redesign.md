# HANDOFF — Paper Redesign + og:image Work (2026-05-19)

> Hand-off document for an AI coding agent picking up this branch on a
> different machine. Read this end-to-end before touching the code. It
> covers: original ask, decisions made, current state per commit, all
> untracked-but-needed resources, the full punch list of remaining work
> with task-level detail, and how to verify everything is green.

---

## 0. TL;DR

- **Branch:** `feat/v0.3-redesign-2` (47 commits ahead of `main`).
- **Last commit:** `c9a61895 fix(review): address Codex review — data
  correctness + i18n + dedup math`.
- **Tree state:** working tree has ~37 uncommitted v0.2-legacy
  prettier reformats + 2 untracked files (`src/components/status-bar/`,
  `src/styles/app/statusbar.css`). These are leftovers from earlier
  sessions and **are NOT needed** on the dev machine — paper shell is
  the active surface; the legacy `status-bar/` directory is never
  mounted. See §6 for the exact stash recommendation.
- **Test status:** JS 1,580/1,580 pass (252 files), Rust vault-core
  483/483 pass, `bun run typecheck` + `bun run format` clean.
  Full `bun run check` gate not yet run end-to-end after Codex fixes —
  do that first thing on the dev machine (see §7).
- **Pending high-impact work:** flip dashboard heatmap from "empty state"
  to real data when backend daily-rollups land, finish Settings
  blocklist/eviction-picker UI, plus the four follow-up items in
  `docs/features/og-images.md` §6.

---

## 1. Original Ask + How It Evolved

### 1.1 Session origin (per prior summaries)

The user opened this branch to migrate the v0.2 brutalist frontend to
a "Paper + Archival" aesthetic. The accepted design package lives in
`docs/design/handoff/paper-redesign/` (HTML + 11 JSX prototypes +
`pk-tokens.css`). The design is **source of truth**; we are not
authoring it.

Core acceptance criteria laid out in the original work block:

1. Match the design pixel-honestly across all routes in light + dark.
2. Wire every route to **real backend data** (no `v0.3-coming` disabled
   UI).
3. Notes + Tags must persist via canonical archive (backend
   annotations table — not localStorage).
4. Three-language i18n parity (`en` / `zh-CN` / `zh-TW`) at every
   commit; `html[lang]` follows runtime locale.
5. Bundled Newsreader + JetBrains Mono Latin subsets default; system
   fonts opt-out; CJK always falls back to system.
6. 100 % JS / Rust coverage + mutation gates not relaxed.
7. Override prior Accepted design docs (design-tokens / screens-and-nav
   / ux-principles / brutalist radius memory / typography memory) —
   already authorized.

### 1.2 What the user added in **this** session (2026-05-19)

| Turn | Ask | Outcome |
|---|---|---|
| Resume the v0.2→v0.3 sweep | "繼續工作" | Foundation tasks closed; finished list-view extracts + tests. |
| "版本號下面要和舊版本一樣放編譯的 commit sha" + "我好像沒看到 redesign 過的 browse 和 search 頁面？為什麼？" | Show commit SHA under sidebar version; explain why Browse/Search look v0.2 | Commit `57105186` — wired `formatBuildRevisionLabel` into `PKSidebar`; flipped `?layout=paper` from opt-in to default with `?layout=legacy` as escape hatch; legacy tests updated to pass `layout=legacy`. |
| "browse 歷史紀錄的頁面，列表模式時用網頁的 icon，卡片模式時用網頁的og:image。考慮一下怎麼高效的儲存。不要用 host 級別的緩存..." | Card mode shows og:image; per-URL keyed; content-hash dedup; not host-level | Six-commit block `WORK-V03-OG-IMAGE-A` (C1–C6). Plan saved at `.claude/plans/indexed-giggling-ullman.md`. Policy: opt-out (default on), user-pickable eviction (default Off), exclude from backup. |
| Codex external review pasted in (P1×3, P2×6 findings) | Confirm + fix | Commit `c9a61895` — all P1 and 5/6 P2 addressed; remaining P2 (status-bar source picker filter no-op) documented but not wired (deferred — wider UX/backend question). |
| "我現在想把你移動到開發機上，你的記憶可能要清除..." | Produce this handoff doc and transfer needed resources to `ssh ubuntu@100.84.91.69` → `/home/ubuntu/coding/PathKeep` | (current task) |

### 1.3 Policy decisions confirmed by the user this session

1. **Paper layout is the default**, with `?layout=legacy` as a short-lived
   escape hatch. Legacy chrome (`ExplorerTimelineBar` + `ExplorerQueryFiltersPanel`)
   stays for v0.2 tests; intelligence-surfaces + lock-and-explorer-shell
   suites all pass `layout=legacy` now.
2. **Commit SHA under version line** in sidebar (3-line PathKeep / v0.X.Y /
   abc1234 lockup; revisits if the layout grows new affordances).
3. **og:image fetch posture = opt-out, default on.** Reasoning:
   - GET is to a URL the user already visited; no incremental info-leak.
   - Default off would silently degrade card UX in a way the user can't
     discover. Toggle lives in Settings → Link previews.
4. **og:image eviction = user-pickable, default Off.** Modes:
   `Off` / `TimeTtl { max_age_days }` / `SizeCap { max_bytes }` /
   `Lru { max_bytes }`. Settings always shows current row · blob · byte
   stats.
5. **og:image cache is derived; excluded from backup export.** Restored
   archives lazy-rebuild from empty tables.
6. **No host-fallback on og:image reads.** GitHub & Medium-class hosts
   serve per-page social cards; reading a sibling-page entry as fallback
   would systematically return wrong images. Pure exact-URL match
   (guard test `lookup_returns_none_for_unknown_page_with_known_host`).
7. **Dashboard fake-data ban (Codex P1):** Heatmap + Active Threads cards
   now render honest empty states pointing at `/intelligence` until the
   backend exposes per-day rollups / query-family signals.
8. **Detail-panel `Open` argument is the URL, not row id** (Codex P1).
9. **`?date=` URL param now drives the query** by synthesizing
   `start = end = date` (Codex P1).

Anything not listed here was either pre-decided (the v0.3 work block)
or follows from one of these rules.

---

## 2. Repository State at Handoff

### 2.1 Branch + remote

```
Branch:   feat/v0.3-redesign-2
Ahead of: main  by 47 commits
Tracking: origin/feat/v0.3-redesign-2  (24 commits ahead at last check,
          all 47 are local-only relative to remote unless pushed)
```

**Action on dev machine:** `git fetch origin` then `git status -sb` to
see the local-vs-remote delta. If `origin/feat/v0.3-redesign-2` is
behind local, you'll want to `git push -u origin feat/v0.3-redesign-2`
after verifying. Don't force-push.

### 2.2 Last 12 commits (newest first)

```
c9a61895 fix(review): address Codex review — data correctness + i18n + dedup math
bbdede3c docs(og-images): feature spec + data-model + STATUS + decision record
59a6f3ca feat(settings): link-previews subsection — toggle + stats + cleanup
13075fab feat(browse): card-mode og:image render with favicon fallback
dde09daa feat(commands): tauri og:image command surface + frontend hook
1902efcf feat(archive): og:image fetch pipeline + AppConfig settings
c8b0ef63 feat(archive): migration 012 og_images schema + vault-core module
48d67299 feat(browse): show favicon in paper list mode
57105186 feat(shell): paper Browse/Search by default + commit SHA back in sidebar
cb330f2b test(paper): extract PaperDetailPanelMount + cover remaining paper helpers
dc333eed refactor(paper): extract route paper panels into testable sibling modules
5d10a0b4 chore(redesign): prettier + lint fixes across the new paper files
```

Each commit message body documents Why / What / How and includes
verification (test counts). Read them in reverse order to reconstruct
the build-up.

### 2.3 Working-tree state (uncommitted)

35 modified files + 2 untracked. Breakdown:

| Bucket | Files | Verdict |
|---|---|---|
| Prettier reformats of shadcn primitives | `src/components/ui/{badge,button,command,dialog,dropdown-menu,input,label,popover,radio-group,scroll-area,select,separator,sheet,skeleton,slider,switch,tabs,textarea,tooltip}.tsx` | Stale — double-quote → single-quote reformats from an earlier session that was never committed. **Stash and forget.** |
| Legacy v0.2 shell tweaks | `src/components/sidebar/index.tsx` + test (PATHKEEP→PathKeep brand), `src/styles/app/{buttons,shell-frame,sidebar,topbar}.css`, `src/styles/app.css`, `src/styles/tokens.css` | Legacy v0.2 surface — the paper shell (`PKSidebar`, `PKStatusBar`, etc.) replaces these. **Stash.** |
| Misc loose tweaks | `docs/design/design-tokens.md`, `src/components/heatmap/year-heatmap.tsx`, `src/lib/i18n/catalog/navigation.ts`, `src/pages/dashboard/{archive-card,on-this-day-card}.tsx`, `src/pages/intelligence-surfaces/explorer-grouped-views.test.tsx`, `src/pages/settings/appearance-section.tsx` + test | Small drive-by edits never finalized. Inspect line-by-line on dev machine; most are no-ops worth dropping. **Stash, decide later.** |
| Untracked v0.2 status bar | `src/components/status-bar/` (epigraph.ts, epigraph.test.ts, index.tsx, index.test.tsx), `src/styles/app/statusbar.css` | v0.2 status-bar component that was never deleted when paper shell replaced it. Not mounted by any active route. **Don't carry, don't commit.** Either `git clean -df` later or leave as-is. |

**Recommendation:** On the dev machine, after the working tree from
git is restored, **do nothing** with these — they're leftovers from
the Mac machine's broader work history. The active branch is fully
self-contained in the 47 commits. If you want them physically on the
dev box for reference, scp them as a sibling tarball (see §6).

### 2.4 Test + build verification snapshot

Captured immediately after commit `c9a61895`:

| Gate | Result |
|---|---|
| `bun run typecheck` | clean |
| `bun run format` | clean (no changes pending) |
| `bun run test:unit` | 252 files / **1,580 tests passing** |
| `cargo test -p vault-core --lib` | **483 passed** |
| `cargo build --workspace` | clean |
| `bun run check` (FULL GATE) | **NOT YET RUN end-to-end after Codex fixes** — run on the dev machine first. |
| `bun run check:deep` (mutation sweep) | not yet run since the last storage change |
| `bun run test:e2e` | not run this session |

---

## 3. WORK-V03-OG-IMAGE-A Detail

Full plan lives at `.claude/plans/indexed-giggling-ullman.md` (file is
in `~/.claude/plans/`, NOT in the repo — transfer separately, see §6).
Six atomic commits:

### C1 — `48d67299` list-mode favicon
- `paper-list-row.tsx` + `paper-contact-sheet.tsx` (pass-through)
- 16 × 16 `<img>` slot with domain-swatch fallback (`data-testid={id}-favicon` / `-swatch`)
- 3 new primitive tests

### C2 — `c8b0ef63` migration 012 + vault-core og_images module
- `src-tauri/crates/vault-core/src/migrations/012_og_images.sql`
- `og_images` (per-page-URL) + `og_image_blobs` (sha256_hex content-addressed)
- 13 vault-core tests including negative `no host fallback` guard
- Four eviction modes: Off / TimeTtl / SizeCap / Lru

### C3 — `1902efcf` fetch pipeline + AppConfig
- `og_images_fetch.rs` — reqwest blocking + scraper, HTTPS-only, 2 MiB cap, 12s timeout
- 15 mockito tests
- `AppConfig.og_image` (fetch_enabled default true, blocked_hosts, cleanup default Off)

### C4 — `dde09daa` Tauri commands + frontend hook
- 6 commands: `load_history_og_images`, `mark_og_images_shown`, `trigger_og_image_refetch`, `get_og_image_storage_stats`, `clear_og_image_cache`, `run_og_image_cleanup`
- `use-explorer-og-images.ts` hook with 1s debounced `mark_shown`
- 7 hook tests

### C5 — `13075fab` + `59a6f3ca` card render + Settings
- `paper-contact-frame.tsx` precedence: og:image > favicon > swatch + scrim
- `link-previews-section.tsx` Settings subsection: toggle + stats + cleanup buttons
- 4 settings tests + 2 contact-frame tests

### C6 — `bbdede3c` docs sweep
- `docs/features/og-images.md` (NEW, full feature spec)
- `docs/architecture/data-model.md` — og_images paragraph
- `docs/plan/STATUS.md` + `CHANGELOG.md` + `research-and-decisions.md` `PG-RD-ARCH-011`
- `docs/design/screens-and-nav.md` — Browse paper paragraph

### Followups from Codex review (commit `c9a61895`)

| Codex finding | Status | Notes |
|---|---|---|
| **P1** dashboard fake data | Fixed | Empty states; heroMessage rewritten in 3 langs |
| **P1** `?date=` not driving query | Fixed | `url-state-derivations.ts` synthesizes start=end=date |
| **P1** Detail panel Open passes id | Fixed | Now passes URL; test rewritten |
| **P2** og:image size-cap byte accounting on shared blobs | Fixed | Refcount map; new regression test |
| **P2** annotations hydration race | Fixed | `locallyMutatedRef` + `lastError` surface |
| **P2** raw i18n strings | Fixed | `paper-view.tsx` + `shell.tsx` route through catalog |
| **P2** status-bar bytes-as-pages | Fixed | `pages` made optional; never populated until real per-profile count exists |
| **P2** `replace_tags` not atomic | Fixed | Wrapped in transaction; row-decode errors no longer swallowed |
| **P2** Search "See in context" stays in search | Fixed | Drops `q`/`mode`/`regex`/`page` on jump |
| **P2** Status-bar source picker filter no-op | **Deferred** | Toggle stores local state but doesn't filter queries. Documented in type comment; requires UX + backend wiring decision. |

---

## 4. Complete Remaining-Work Punch List

Ordered by impact. **READ THE WHY before doing the WHAT.**

### 4.1 P0 — Before next push

1. **Run `bun run check` end-to-end.** Codex review fixes haven't been
   gated through the full per-commit gate (100% JS+Rust coverage, e2e,
   desktop-bridge truth, desktop-contract mutation). If anything fails,
   fix it before pushing — the gate is the source of truth, not the
   focused suites I ran.
2. **Run `cargo test --workspace`.** I only ran `cargo test -p vault-core`.
   `vault-worker` and `pathkeep-desktop` may have integration-level
   tests that the new og:image surface touches.
3. **Coverage delta.** `bun run coverage:js` was at ~97.28 % lines before
   the Codex sweep; the dashboard fake-data removal deleted a few code
   paths. Re-run to confirm still ≥ existing baseline; bring up
   newly-uncovered branches if needed.

### 4.2 P1 — Active design / product gaps

1. **Source-picker filter wiring (Codex P2 deferred).** Currently the
   status-bar source dropdown writes `sourceFilter` to shell-local
   state but no route reads it. Decide: (a) drop the dropdown until
   the filter actually works, or (b) thread `profileId` from
   `useProfileScope` into the picker and have it call `setActiveProfile`.
   Files: `src/app/shell.tsx`, `src/components/shell/pk-status-bar.tsx`,
   `src/lib/profile-scope-context.tsx`.

2. **Section-panel paper restyle inside Settings sections.** The route
   wraps each section with `?layout=paper` chrome, but the *insides*
   of `general` / `ai` / `applock` / `profiles` / `derived` / `remote` /
   `platform` sections still use the v0.2 form primitives. Designer
   prototype: `docs/design/handoff/paper-redesign/project/pk-settings.jsx`.

3. **Paper restyle remaining sibling routes:** `/schedule`, `/security`,
   `/maintenance`, `/jobs`, `/integrations`, `/onboarding`, `/lock`.
   `?layout=paper` is the default everywhere, but these routes still
   render v0.2 chrome. Each is a self-contained sweep matching the
   pattern set by `/intelligence`, `/import`, `/audit`.

4. **Drop `?layout=legacy`** once the v0.2 intelligence-surfaces and
   lock-and-explorer-shell tests are migrated or retired. The current
   escape hatch carries ~6 tests that simulate v0.2 chrome behaviour.
   Migration plan: rewrite those tests against the paper surface,
   delete the legacy code paths in `explorer/index.tsx` (the
   `ExplorerTimelineBar` + `ExplorerQueryFiltersPanel` blocks).

5. **Dashboard real data:** wire `getBrowsingRhythm` or a new daily-
   rollup API into `YearHeatmap`. Currently the card shows an empty
   state because the backend only exposes dow×hour aggregates. Either:
   - Add `get_daily_rollups(range)` to vault-worker → core-intelligence
     and use it from the dashboard, OR
   - Reshape the heatmap UI to dow×hour (rhythm) instead of year-grid.

6. **Active Threads real data:** wire `getPathFlows` / `getQueryFamilies`
   from core-intelligence so the empty state can become a real list.
   Card already routes to `/intelligence`; the embed needs the same
   data shape the Intelligence route consumes.

### 4.3 P2 — og:image follow-ups (from `docs/features/og-images.md` §6)

1. **Settings UI completion:** blocklist textarea, eviction-mode
   picker (Off / Time / Size / LRU segmented control), per-mode
   numeric input. Backend + AppConfig already support it; only UI
   missing. Pattern: mirror `appearance-section.tsx`'s
   `SegmentedControl` + `Toggle` components.

2. **Worker parallelism + per-host rate limit:** `refetch_og_images`
   currently serial-fetches. Add token bucket (≥ 500 ms between same-host
   requests) and a 2-worker pool. Touch `archive_flows.rs::refetch_og_images`
   only — vault-core fetch function stays single-call.

3. **Daily schedule.rs tick:** add `run_og_image_cleanup` to the daily
   maintenance tick so user-configured eviction actually fires
   automatically. File: `src-tauri/crates/vault-core/src/schedule.rs`.

4. **Negative-cache TTL auto-refetch:** worker scans rows where
   `refetch_after < now()` and re-tries once. Currently only manual
   "Run cleanup now" prompts retries.

5. **Image dimension probe:** when willing to add `image` crate
   (features = "png,jpeg" only — vcpkg rule still applies), populate
   `width`/`height` in `og_image_blobs`. Currently NULL.

### 4.4 P3 — Docs + memory hygiene

1. **Memory updates:** `~/.claude/projects/.../memory/`
   - `feedback_brutalist_radius.md` — already says "superseded";
     leave as-is.
   - `feedback_typography_policy.md` — current, leave.
   - `project_v0_3_redesign.md` — needs an update to note that
     paper is now the default, and og:image work is shipped.

2. **Design doc sweeps still outstanding:** AGENTS.md called for
   `design-tokens.md`, `ux-principles.md`, `ui-review-guardrails.md`,
   `typography-and-font-fallback.md` to be **rewritten** for the v0.3
   redesign. They've been **touched** but not authoritatively rewritten.
   Owner discretion on scope; the visual contract in
   `docs/design/handoff/paper-redesign/` remains source-of-truth.

3. **STATUS.md** — verify `WORK-V03-PAPER-REDESIGN-A` close-out
   criteria. The block still has `[ ]` open; not every acceptance
   criterion (e.g. light + dark screenshots in release artifacts)
   is met yet. Don't close it until the criteria match reality.

---

## 5. Architectural Constants Worth Repeating

These are derived from AGENTS.md but are easy to forget when picking
up cold; pinning here for the dev-machine session:

1. **Performance target:** 4-core / 3 GHz / 8 GB RAM machine streaming
   14.4 M visits (60-year archive). Every algorithm should be sanity-
   checked at this scale. No full-table scans, no monolithic JOINs.
2. **Native deps red line:** no Homebrew/apt/winget global packages.
   New native libs go via `vcpkg.json` + project-scoped install.
   `build.rs` is forbidden from compiling C/C++ source. Pure-Rust
   crates only for image decode etc.
3. **i18n contract:** every user-visible string at commit time in
   en / zh-CN / zh-TW. `bun run check:i18n` enforces parity.
4. **Trust & Transparency:** PME (Preview → Manual → Execute) for
   anything destructive. No black-box mutations.
5. **Data sovereignty:** no cloud transmission by default. og:image
   fetch is the closest we get; it's an outbound GET to a URL the
   user already visited, opt-out and gated.
6. **`bun run check` is the per-commit authority.** `check:base` is a
   triage tool, never a replacement.
7. **Big files (>1,000 lines)** require a two-stage refactor: audit
   pass (no code changes), then execute pass. Several explorer files
   are approaching this — be careful.

---

## 6. Untracked Resources Needed on Dev Machine

The commits carry everything in the repo. These additional pieces are
outside git:

### 6.1 Plan files at `~/.claude/plans/`

The plans directory holds three files; **only one is current**:

| File | Status | Action |
|---|---|---|
| `indexed-giggling-ullman.md` | **CURRENT** — WORK-V03-OG-IMAGE-A plan | **MUST be transferred** to dev machine `~/.claude/plans/` |
| `fetch-this-design-file-purrfect-bumblebee.md` | Historical (initial design-fetch plan) | Optional — useful for context only |
| `staged-squishing-teapot.md` | Historical (earlier paper redesign plan) | Optional |

### 6.2 Auto-memory at `~/.claude/projects/-Users-tim-LocalData-coding-2026-Lab-8-chrome-history-backup/memory/`

| File | Status | Action |
|---|---|---|
| `MEMORY.md` | Active index | Transfer (rename project dir to match dev machine path) |
| `feedback_brutalist_radius.md` | Active (superseded note) | Transfer |
| `feedback_typography_policy.md` | Active | Transfer |
| `project_v0_3_redesign.md` | Active (slightly stale — see §4.4) | Transfer |

**Critical:** the directory name embeds the project absolute path. On the
dev machine the path is `/home/ubuntu/coding/PathKeep`, so the destination
dir is `~/.claude/projects/-home-ubuntu-coding-PathKeep/memory/`. You
must mkdir that and place the files there, OR rely on Claude to
recreate the dir from scratch using the same MEMORY.md content (it
will re-index from the index file).

### 6.3 Repo `.claude/` (already in git)

`launch.json` + `settings.local.json` are tracked in the repo (they came
across in commit history). No action needed.

### 6.4 The 35 uncommitted files + 2 untracked dirs

**Do not transfer.** They're v0.2-legacy debris that the paper shell
already replaced. On the dev machine you'll have a clean tree.

### 6.5 Codex review prompt (paste-in)

The original Codex review prompt + findings was passed inline; the
fixed-state record is in the body of commit `c9a61895`. If Codex
needs to re-review on the dev machine, point it at
`main...HEAD` and feed it this handoff doc as context.

---

## 7. Dev-Machine Bootstrap Sequence

```bash
ssh ubuntu@100.84.91.69
cd /home/ubuntu/coding/PathKeep

# 1. Pull the branch.
git fetch origin
git checkout feat/v0.3-redesign-2
git pull --ff-only origin feat/v0.3-redesign-2

# Confirm last commit:
git log -1 --format="%H %s"
# Expect: c9a61895 fix(review): address Codex review — data correctness + i18n + dedup math

# 2. Install deps (assuming bun + cargo + tauri prerequisites are already on the box).
bun install
cd src-tauri && cargo fetch && cd ..
bun run native-deps:doctor   # confirms vcpkg setup
```

If `c9a61895` isn't on `origin/feat/v0.3-redesign-2`, the local Mac
hasn't pushed yet. From the Mac, run `git push -u origin
feat/v0.3-redesign-2` then re-run the pull on the dev box. **Don't
force-push.**

```bash
# 3. Place auto-memory.
mkdir -p ~/.claude/projects/-home-ubuntu-coding-PathKeep/memory
# scp the four memory files in from the Mac → this directory
# scp the indexed-giggling-ullman.md → ~/.claude/plans/

# 4. Authoritative gate.
bun run check                # MUST PASS before any new work
cargo test --workspace       # confirms Rust workspace-wide pass

# 5. Continue work.
# Read this handoff doc, then docs/plan/STATUS.md, then the relevant
# work-item file (e.g. .claude/plans/indexed-giggling-ullman.md).
# Pick from §4 of this doc.
```

---

## 8. File-Level Quick Reference

Where each major piece of the WORK-V03-OG-IMAGE-A surface lives:

| Layer | Path |
|---|---|
| Migration SQL | `src-tauri/crates/vault-core/src/migrations/012_og_images.sql` |
| Schema registration | `src-tauri/crates/vault-core/src/archive/schema.rs:82` (MIGRATIONS array) |
| Core storage API | `src-tauri/crates/vault-core/src/archive/history/og_images.rs` |
| Fetch pipeline | `src-tauri/crates/vault-core/src/archive/history/og_images_fetch.rs` |
| AppConfig field | `src-tauri/crates/vault-core/src/models/app.rs::OgImageSettings` |
| Worker entry points | `src-tauri/crates/vault-worker/src/archive_flows.rs::load_history_og_images / refetch_og_images / …` |
| Tauri commands | `src-tauri/src/commands/archive.rs` (6 commands) |
| Worker bridge | `src-tauri/src/worker_bridge/archive.rs` (6 `*_impl`) |
| Invoke handler | `src-tauri/src/lib.rs:118` |
| Dev IPC mirror | `src-tauri/src/dev_ipc_bridge/{dispatch.rs,payloads.rs}` |
| Frontend types | `src/lib/types/archive.ts::HistoryOgImage*` |
| Backend client | `src/lib/backend-client/explorer.ts` + `src/lib/backend-client/index.ts` |
| Backend facade | `src/lib/backend.ts` |
| Hydration hook | `src/pages/explorer/hooks/use-explorer-og-images.ts` |
| Lookup key | `src/pages/explorer/helpers.ts::historyOgImageLookupKey` |
| Card render | `src/components/explorer-paper/paper-contact-frame.tsx` |
| Settings UI | `src/pages/settings/link-previews-section.tsx` |
| i18n keys | `src/lib/i18n/catalog/settings-core-and-platform.ts` (`linkPreviews*`) |
| Feature spec | `docs/features/og-images.md` |
| Data-model paragraph | `docs/architecture/data-model.md` (§ Storage planes) |
| Decision record | `docs/plan/program/research-and-decisions.md::PG-RD-ARCH-011` |
| Plan file | `~/.claude/plans/indexed-giggling-ullman.md` |

---

## 9. Open Questions That May Re-Surface

1. **OG image dimension probing:** deferred for dependency-surface
   reasons (would need `image` crate). If the Settings stats panel
   ever wants "average preview dimensions", revisit.
2. **OG image fetch parallelism:** currently serial. If users
   complain about hydration speed, see §4.3.2.
3. **Heatmap real data path:** see §4.2.5. The choice between
   year-grid-with-daily-rollups vs dow×hour-rhythm-card is a design
   question, not yet asked.
4. **Source picker filter:** see §4.2.1. Either it does something or
   it goes away.
5. **`?layout=legacy` lifetime:** see §4.2.4. Roughly six tests'
   worth of work to retire; not urgent.

---

## 10. Quick Sanity Checklist Before First Commit on Dev Machine

```
[ ] `git log -1 --format="%H %s"` matches c9a61895
[ ] `bun run check` clean
[ ] `cargo test --workspace` clean
[ ] `~/.claude/plans/indexed-giggling-ullman.md` present
[ ] `~/.claude/projects/-home-ubuntu-coding-PathKeep/memory/MEMORY.md` present
[ ] Pick a task from §4, read the related code BEFORE editing
[ ] Open `docs/plan/STATUS.md` to confirm current focus
[ ] If touching paper components, read `docs/design/handoff/paper-redesign/`
```

---

_Living document — update if you keep going. Don't trust commit
messages alone; the *why* lives here._
