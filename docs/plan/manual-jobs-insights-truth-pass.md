# Jobs / Insights Manual Truth Pass

> Scope: background-work Jobs route + Insights route after the 2026-04-11 UI/UX reset.  
> Built from: `docs/design/screens-and-nav.md`, `docs/features/intelligence.md`, `docs/plan/e2e-workflow-tests.md`, and the user-visible UI surface.

## Goal

Verify every user story, visible affordance, and implied action on the Jobs / Insights surfaces using real desktop data when available, and record any environment blocker honestly when the live desktop bridge cannot be reached.

## Environment

- Host: macOS
- App data root: `~/Library/Application Support/com.yi-ting.pathkeep`
- Real archive DB: `~/Library/Application Support/com.yi-ting.pathkeep/archive/history-vault.sqlite`
- Preferred truth path:
  1. real desktop app or browser-desktop-bridge with live Rust command facade
  2. if blocked, browser preview + shipped UI regression tests + direct archive/runtime inspection

## Preflight

1. Open PathKeep and confirm the archive is initialized and unlocked.
2. Confirm the footer / shell background-work strip is visible.
3. Confirm at least one recent backup exists.
4. Confirm `readable-content-refetch` and deterministic modules are enabled in Settings.
5. Capture the current runtime queue summary before touching Jobs / Insights.

## Route 1 — Jobs

### Entry

1. Open sidebar → `Jobs`.
2. Verify the page answers three questions immediately:
   - what is running now
   - what is merely queued / deferred
   - what needs review

### Header and queue grammar

1. Read the top status callout.
2. Click `Refresh`.
   Expected: queue data reloads without leaving the page.
3. Click `Pause background work`, then `Resume background work`.
   Expected: queue status changes truthfully; queued jobs stay persisted.
4. Click `Open Settings`.
   Expected: Settings opens to the review surface without losing context.

### Queue overview

1. Verify the overview hero shows:
   - running count
   - queued count
   - failed count
   - readable pages saved
2. Verify the explanatory copy distinguishes:
   - deterministic rebuild first
   - network-backed content fetch later
   - failed items vs deferred backlog
3. Verify the small callouts show one live focus item and one review item when those states exist.

### Queue families

1. In `AI queue`, verify queued / running / failed / worker count and last activity.
2. In `Derived-data queue`, verify queued / running / failed / last activity.
3. In `Page content fetch`, verify:
   - network boundary is explicit
   - stored row count is visible
   - queued / running / failed counts are visible
   - last error is phrased in human language, not only raw status
4. In `Title normalization`, verify local-only boundary and local-first copy.
5. In `Deterministic modules`, verify ready vs attention counts and last build timestamp.
6. In `Recovery`, verify restart / recovery notes appear when present and empty copy is honest when absent.

### Runtime health and recent activity

1. Review `Enrichment plugins`.
   Expected: each plugin shows boundary, description, queue counts, last completed, and last error.
2. Review `Deterministic modules`.
   Expected: each module shows status, explanation, derived tables, and stale reason if any.
3. Review `Recent AI jobs`.
   Expected: job state, summary, timestamps, and retry / cancel controls match the job state.
4. Review `Recent derived-data jobs`.
   Expected:
   - running rebuilds show percent, label, and progress detail
   - failed fetches show readable failure copy
   - queued jobs can be cancelled
   - failed jobs can be retried

### Cross-route actions

1. Retry one failed runtime job.
2. Cancel one queued runtime job.
3. Retry one failed AI job.
4. Cancel one queued AI job.
5. Return to Jobs and confirm the visible state updates.

## Route 2 — Insights

### Entry

1. Open sidebar → `Insights`.
2. Verify the page opens with:
   - analysis snapshot first
   - runtime digest second
   - full queue review delegated to `Jobs`

### Header and runtime digest

1. Verify AI status callout copy matches the current AI-enabled / disabled truth.
2. If a shared profile scope is active, verify the scoped-view callout appears.
3. Click `Refresh`.
   Expected: a deterministic rebuild is queued and the page points back to Jobs for full progress review.
4. Verify the runtime digest:
   - shows queue summary
   - exposes one lead job if a notable running / failed / queued item exists
   - provides `Retry` / `Cancel` only when valid
   - deep-links to `Jobs`

### Analysis snapshot

1. Verify the hero shows:
   - time range
   - highlight count
   - topic count
   - coverage
2. Verify archive-wide vs profile-scoped honesty copy is visible.

### Spotlight section

1. Review `On This Day`.
   Expected: only previous years appear; clicking an item deep-links to Explorer.
2. Review `Site analytics`.
   Expected: ranked domains, bars, and counts are readable; clicking a row deep-links to Explorer.
3. Review `Storage analytics`.
   Expected:
   - tracked bytes
   - reclaimable bytes
   - dominant slice
   - latest growth signal deep-links to Audit
4. Review `Summary`.
   Expected:
   - template summaries or fallback summary paragraphs
   - notes remain visible
   - explain action works when a generated summary exists

### Research signals section

1. Review `Query groups`.
   Expected: title, steps, confidence, evidence tier, explain action, Explorer deep-link.
2. Review `Topic timeline`.
   Expected: labels, bars, counts, and deep-link counts remain readable.
3. Review `Query evolution`.
   Expected: ladder steps, stage labels, and profile scope remain visible.

### Evidence and health section

1. Review `Reference pages`.
   Expected: explain + Explorer links work and evidence counts are visible.
2. Review `Source effectiveness`.
   Expected: source role, group / reference / landing counts are visible.
3. Review `Deterministic modules`.
   Expected: status, derived tables, and last built details are visible.

### Explainability

1. Trigger `Why this?` from:
   - one highlight card
   - one query group or summary if available
2. Verify the explainability panel:
   - names the selected item
   - shows explanation text
   - shows citations with timestamps
   - deep-links citations back to Explorer

### Cross-route actions

1. Click `Browse history`.
2. Click `Ask assistant`.
3. Click the latest growth signal → Audit.
4. Click `Runtime queue` / `Jobs`.
5. Return to Insights and verify state remains coherent.

## Off-happy-path probes

1. Shared profile scope enabled:
   - verify scope callout and archive-wide metric honesty
2. AI disabled:
   - verify the page still shows deterministic surfaces
3. Queue failed but insights still load:
   - verify runtime digest warns without taking over the whole page
4. Zero-data / cleared derived state:
   - verify honest empty states, not fake full cards

## Execution Log — 2026-04-11

### Completed

- `bun run test:unit:product-flows` — passed
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core enrichment_failure_message_turns_known_fetch_states_into_honest_copy` — passed
- `bun run build` — passed
- Direct archive inspection against real app data:
  - config: initialized, plaintext archive, `chrome:Default` selected
  - latest backup runs: ids `1`–`4` succeeded on `2026-04-12 UTC`
  - `visit_content_enrichments`: `27687 success`, `83 empty`, `16 unsupported-content`, `14 fetch-error`
  - `intelligence_jobs`: `27769 succeeded`, `23486 queued`, `1 running`, `30 failed`
  - latest `insight_run`:
    - `26139` visits processed
    - `26120` enrichments completed
    - `370` query groups
    - `228` topics
    - `234` threads
    - `12` reference pages
    - `10` source-effectiveness rows
    - `4` cards
  - conclusion: the real archive does **not** support the claim “all web content fetches fail”; the dominant truth is “most succeed or are deferred, a small set fails on PDFs / redirects / unsupported responses”.

### Blocked in this environment

- `PATHKEEP_DEV_SERVER_PORT=15420 PATHKEEP_DEV_IPC_PORT=43118 bun run desktop:dev:bridge`
  - Vite UI started
  - browser surface at `http://127.0.0.1:15420/` still reported `app_lock_status` could not reach the local desktop bridge
  - localhost health probes to ports `43117` / `43118` were unreachable from this sandboxed run
- `bun run check`
  - JS / desktop-contract / clippy / workspace Rust unit tests passed
  - blocked at host-native `vault-platform` tests:
    - `macos_scheduler_apply_bootstrap_status_and_cleanup_work`
    - `macos_keychain_roundtrip_uses_a_unique_service_namespace`
  - failure reason in this environment: `launchctl bootstrap` and Keychain roundtrip are not permitted from the current host sandbox

## Remaining live-desktop pass

Run the full route checklist again on an unsandboxed host session where:

- the browser-desktop-bridge localhost IPC is reachable, or
- a real Tauri window can be driven end-to-end

That final pass should attach screenshots for:

1. Jobs overview + plugin/module health
2. Jobs recent activity with retry / cancel
3. Insights hero + runtime digest
4. Spotlight section
5. Research signals section
6. Evidence / health section
7. Explainability panel

## Execution Log — 2026-04-12

### Runtime and data truth

- `bun run desktop:dev:bridge` succeeded on the local host.
- Confirmed `curl http://127.0.0.1:43117/health` returned `{"runtime":"browser-desktop-bridge", ...}`.
- Confirmed the browser surface at `http://127.0.0.1:1420` switched to `browser-desktop-bridge`, not preview fixture mode.
- Verified live data came from the real local archive:
  - Jobs showed real queue movement and live content-fetch counts.
  - Insights showed real archive-wide highlights, query groups, reference pages, source effectiveness, and storage growth.

### Manual Jobs route pass

- Opened `#/jobs` and verified:
  - status callout loaded real queue state
  - queue overview showed running / queued / failed / readable pages saved
  - content-fetch backlog copy clearly distinguished deferred backlog from failure
  - deterministic modules and recovery sections rendered against live runtime data
- Found and fixed two UI honesty bugs:
  - the hero previously said “Nothing needs manual review right now” while failed jobs existed
  - plugin/runtime surfaces still leaked raw fetch-status wording in a few places
- Re-tested after the patch:
  - review callout now stays consistent with failed backlog
  - plugin error copy now uses human language on both the hero and runtime-health surfaces

### Manual Insights route pass

- Opened `#/insights` and verified:
  - analysis snapshot loads before lower-level evidence sections
  - runtime digest stays compact and points back to Jobs
  - `On This Day` stayed honest with an empty previous-years state
  - `Top sites`, `Storage`, `Summary`, `Query groups`, `Reference pages`, `Source effectiveness`, and deterministic modules rendered from live data
- Found and fixed two readability issues:
  - extremely small coverage rounded down to `0%`
  - long tracking-style URLs dominated query-group and card titles
- Re-tested after the patch:
  - coverage now shows `<1%` when the run is tiny but non-zero
  - long tracking URLs are compacted into readable host/path labels

### Real interactions exercised

- Jobs:
  - opened `Refresh`
  - attempted `Cancel` on a live content-fetch job
  - observed a real backend race: the job had already moved to `succeeded`, so `cancel_intelligence_job` returned `500`
  - patched the frontend to treat “cannot be cancelled / retried” state drift as a refreshable race instead of surfacing a stale hard failure
  - re-tested and confirmed the page refreshed back to coherent queue state without getting stuck on a broken error surface
- Insights:
  - triggered `Why this?`
  - verified the explainability panel showed explanation text plus citations with timestamps
  - clicked a deep link from Insights to Explorer
  - verified Explorer opened with the expected scoped filter (`domain=linux.do`)

### Artifacts

- Saved real-surface screenshots:
  - `artifacts/jobs-page-real-data.png`
  - `artifacts/insights-page-real-data.png`

### Automated regressions run during the pass

- `bunx vitest run src/lib/intelligence-presentation.test.ts src/pages/intelligence-surfaces.test.tsx` — passed
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core enrichment_failure_message_turns_known_fetch_states_into_honest_copy` — passed
- `bun run build` — passed

### Desktop bridge truth gate follow-up

- `bun run test:e2e:desktop-bridge:truth` initially failed on this host because the test uses a fresh temporary `CARGO_TARGET_DIR`, which forced a full desktop rebuild and exceeded the test's own `120s` bridge-readiness poll.
- Fixed the test timeout to match the real cold-start cost of a clean desktop-bridge build.
- Re-run is required after that timeout adjustment to record the new steady-state result.
