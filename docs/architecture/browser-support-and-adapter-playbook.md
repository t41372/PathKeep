# Browser Support And Adapter Playbook

> This document is the source of truth for PathKeep's browser-support taxonomy. It separates code coverage from public promise so README/UI only claim what maintainers have recently validated.

## Current Support Taxonomy

### Validated Now

- `Google Chrome`
  - Discovery: shipping
  - Parser / ingest: shipping via the Chromium family pipeline
  - Public promise: allowed in README, onboarding, and release docs
  - Validation evidence required: local backup / recall pass on the current dev host
- `Microsoft Edge` / `Microsoft Edge Dev`
  - Discovery: shipping on macOS / Windows / Linux through the Chromium-family profile matrix
  - Parser / ingest: shipping via the Chromium family pipeline for backup and Browser Direct local `History` import
  - Public promise: allowed in README, onboarding, and release docs
  - Validation evidence required: Edge Browser Direct preview / import / re-import / revert / restore, backup readability handling, and source-profile metadata checks that preserve `Microsoft Edge` / `Microsoft Edge Dev` instead of collapsing to generic Chrome
- `Firefox`
  - Discovery: shipping on macOS / Windows / Linux through the Firefox-family profile matrix
  - Parser / ingest: shipping as a history-only baseline through `places.sqlite` for backup and Browser Direct import
  - Public promise: allowed in README, onboarding, and release docs with the history-only caveat
  - Validation evidence required: Firefox `places.sqlite` Browser Direct preview / import / re-import / revert / restore, backup readability handling, source evidence, and schema mismatch / quick-check failure coverage
  - Explicitly out of scope: Firefox downloads, favicons, keyword-search sidecars, and richer `moz_*` evidence promotion until separate follow-up validation lands
- `ChatGPT Atlas` on macOS
  - Discovery: shipping for `~/Library/Application Support/com.openai.atlas/browser-data/host/<profile>`
  - Parser / ingest: shipping via the Chromium family pipeline for browser history data only
  - Public promise: allowed only for the validated macOS Browser Direct / backup layout
  - Validation evidence required: local Atlas `History` Browser Direct preview / import / re-import / revert / restore, plus source-profile and source-evidence checks
  - Explicitly out of scope: Atlas workspace data, chats, tabs, bookmarks, suggestions, and unvalidated Windows / Linux locations
- `Perplexity Comet` on macOS
  - Discovery: shipping for `~/Library/Application Support/Comet/<profile>`
  - Parser / ingest: shipping via the Chromium family pipeline for browser history data only
  - Public promise: allowed only for the validated macOS Browser Direct / backup layout
  - Validation evidence required: local Comet `History` Browser Direct preview / import / re-import / revert / restore, plus source-profile and source-evidence checks
  - Explicitly out of scope: Comet AI memory, Perplexity account/workspace data, chats, tabs, bookmarks, suggestions, and unvalidated Windows / Linux locations
- `Safari` on macOS
  - Discovery: shipping
  - Parser / ingest: shipping as history-only baseline for backup and Browser Direct local `History.db` import
  - Public promise: allowed only with the explicit Full Disk Access caveat
  - Validation evidence required: visible-but-unreadable state when access is missing, plus a successful backup and Browser Direct preview/import/re-import/revert/restore once access is granted

### Implemented, Not Yet Publicly Promised

- Chromium-family adapters currently wired into discovery and ingest:
  - `Chromium`
  - `Brave`
  - `Vivaldi`
  - `Arc`
  - `Opera`
  - `Opera GX`
- Firefox-family adapters currently wired into discovery and ingest:
  - `LibreWolf`
  - `Floorp`
  - `Waterfox`

These adapters may appear in discovery, icons, logs, or archive data, but they must not be promoted into README/UI promise copy until they pass the promotion gate below.

### Adapter Candidates

- Any browser that is not yet wired into discovery, parser selection, and canonical archive ingest
- Examples worth separate research: Pale Moon, qutebrowser, mobile exports, and custom-path onboarding for unsupported-but-compatible SQLite sources

## Promotion Gate

A browser is not allowed into README, onboarding, release docs, or other public-support copy until all of the following are true:

1. Discovery is implemented and documented.
2. Parser coverage is implemented, either by reusing an existing family parser or by adding a new parser module.
3. Canonical archive ingest is wired end to end.
4. Capability snapshot / source-batch provenance is wired end to end.
5. User-visible caveat / degraded-state copy exists in `en`, `zh-CN`, and `zh-TW`.
6. Icons, names, and route-level UI touchpoints are aligned.
7. Parser, discovery, archive, Browser Direct import, and capability acceptance tests exist.
8. Local validation evidence is written into the testing / release docs for the current host.

If any one of these is missing, the browser stays in `implemented, not yet publicly promised`.

## Adapter Implementation Checklist

### 1. Name And Family

- Add or confirm a stable browser key in `vault_core::chrome`.
- Reuse an existing `browser_family` when the browser is schema-compatible with `chromium`, `firefox`, or `safari`.
- Only add a new family when discovery, parser behavior, and archive ingest truly diverge.

### 2. Discovery Wiring

- Update browser definitions in `src-tauri/crates/vault-core/src/chrome.rs`.
- Add host-specific path candidates in `src-tauri/crates/vault-core/src/chrome/paths.rs`.
- Keep discovery honest about missing or unreadable history files; do not hide degraded profiles just because access is missing.

### 3. Parser Strategy

- Reuse `browser-history-parser` family extractors when the browser uses the same database shape.
- Add a new parser module in `src-tauri/crates/browser-history-parser/src/` when the schema is genuinely different.
- Extractors must stay provided-path only: no installed-browser discovery, no live-file copying, and no Tauri dependencies.
- Every adapter must produce schema observation, capability snapshot, canonical facts, typed evidence, and native-entity preservation rules.

### 4. Staging And Ingest

- Define which history DB and sidecar files must be staged before parsing.
- Keep staging in `vault-core` / `vault-platform`; keep parsing in `browser-history-parser`.
- Staging must hand the read-only parser a clean, self-contained database. Prefer an online SQLite backup (`Connection::backup`) of the live file with a busy timeout; it copies a transactionally consistent, journal-free snapshot even while the browser writes. The raw file-copy fallback can capture a live browser's hot rollback journal or WAL (Chrome's `History` uses a `TRUNCATE` rollback journal), which a read-only open cannot replay — `SQLITE_READONLY_ROLLBACK` (extended code 776) / `_RECOVERY`. So the fallback copies the sidecars and then recovers the copy (open read-write, `PRAGMA journal_mode=DELETE`) **only on the worker-owned staging copy, never the live browser file**. Never require the user to quit the browser to back up.
- Wire the adapter into archive ingest only after schema warnings, source-batch provenance, watermarks, capability tags, and source-kind naming are explicit.
- Browser Direct local database import must use `inspect_browser_history` / `import_browser_history`, not Takeout commands. It must stage a SQLite snapshot, run `PRAGMA quick_check`, create an import batch, preserve source evidence, refresh search projection, and reuse import-batch revert / restore.
- Browser Direct support parity means end-to-end reliability within the source's real capabilities. Safari and Firefox must not fabricate Chrome-only Favicons, downloads, or keyword-search sidecars.
- Microsoft Edge and Microsoft Edge Dev are Chromium-family adapters, but source profile metadata must preserve their product identity. Discovery-sourced Edge imports must not fall back to generic `Google Chrome`; generic Chromium fallback is only allowed when a user manually picks a raw `History` file without profile metadata.
- Firefox support uses `places.sqlite` and `browser-history-parser::firefox::stream_history` for a history-only baseline. It preserves source evidence and import-batch rollback / restore, but richer favicon/download/search evidence stays additive follow-up work.
- ChatGPT Atlas support is a Chromium-family adapter with a narrower source boundary: PathKeep reads `<profile>/History` and Chromium sidecars such as `<profile>/Favicons` from the validated `com.openai.atlas/browser-data/host` macOS profile root. It must not inspect or import Atlas workspace, chat, tab, bookmark, or suggestion data.
- Perplexity Comet support is a Chromium-family adapter with a narrower source boundary: PathKeep reads `<profile>/History` and Chromium sidecars such as `<profile>/Favicons` from the validated `~/Library/Application Support/Comet` macOS profile root. It must not inspect or import Comet AI memory, Perplexity account/workspace data, chats, tabs, bookmarks, or suggestion data.

### 5. UI And Copy

- Add browser icon coverage in `src/lib/browser-icons.tsx` if the UI should render a dedicated glyph.
- Add or adjust retention / caveat copy for onboarding, dashboard, settings, and troubleshooting surfaces.
- All new or changed user-visible strings must ship in `en`, `zh-CN`, and `zh-TW`.

### 6. Required Tests

- Parser tests for happy path and missing-table / damaged-shape behavior
- Discovery tests for default paths, overrides, and unreadable files
- Archive acceptance proving backup / recall on that adapter path
- Browser Direct acceptance proving preview, execute, re-import dedupe, import-batch revert / restore, and source-evidence batch writes
- Capability snapshot / coverage tests for version drift and partial support
- Route or i18n tests for any new public-support promise copy

### 7. Dev Guides

- Follow [../dev/browser-schema-evolution.md](../dev/browser-schema-evolution.md) when schema changes.
- Follow [../dev/browser-adapter-guide.md](../dev/browser-adapter-guide.md) when adding a new adapter.
- Follow [../dev/field-promotion-playbook.md](../dev/field-promotion-playbook.md) when promoting preserved native fields into typed evidence or module capabilities.

### 7. Local Validation Evidence

- Document the exact local validation recipe in `TESTING.md` and `docs/plan/m4-full-polish/release-readiness-runbook.md`.
- Record which browser was validated, what caveats were exercised, and which commands or manual checks were used.
- Only after that evidence exists may README/UI promise the adapter publicly.

## Current Local Validation Recipe

- `Google Chrome`
  - successful local backup path
  - archive recall still works after the backup
- `Microsoft Edge` / `Microsoft Edge Dev`
  - Browser Direct local `History` preview / import / re-import / revert / restore succeeds through the Chromium parser path
  - source profile metadata preserves `Microsoft Edge` / `Microsoft Edge Dev` instead of collapsing the profile into generic Chrome
  - backup discovery and selected-profile handling treat unreadable Edge profiles as skipped/degraded when another selected profile is readable
- `Firefox`
  - profile-directory and direct `places.sqlite` Browser Direct preview / import / re-import / revert / restore succeeds through the Firefox parser path
  - source profile metadata uses `Firefox`, `firefox-history`, and `firefox-places-db`
  - backup discovery and selected-profile handling treat unreadable Firefox profiles as skipped/degraded when another selected profile is readable
  - validation records aggregate counts and time ranges only; private URLs and titles must not be copied into docs or chat
- `ChatGPT Atlas`
  - macOS discovery finds profiles under `~/Library/Application Support/com.openai.atlas/browser-data/host`
  - Browser Direct local `History` preview / import / re-import / revert / restore succeeds through the Chromium parser path
  - source profile metadata preserves `ChatGPT Atlas` instead of collapsing the profile into generic Chrome
  - local validation records schema coverage, aggregate counts, and time ranges only; private URLs and titles must not be copied into docs or chat
- `Perplexity Comet`
  - macOS discovery finds profiles under `~/Library/Application Support/Comet`
  - Browser Direct local `History` preview / import / re-import / revert / restore succeeds through the Chromium parser path
  - source profile metadata preserves `Perplexity Comet` instead of collapsing the profile into generic Chrome
  - local validation records schema coverage, aggregate counts, and time ranges only; private URLs and titles must not be copied into docs or chat
- `Safari`
  - profile remains visible when `History.db` is unreadable
  - needs-access guidance points to macOS Full Disk Access and exposes a direct System Settings action
  - baseline history backup succeeds once access is available
  - Browser Direct local `History.db` preview / import / re-import / revert / restore succeeds once access is available
  - local validation records aggregate counts and time ranges only; private URLs must not be copied into docs or chat

This recipe is intentionally narrower than the total code coverage in the repo. Public promise follows the validated recipe, not the broadest internal implementation surface.
