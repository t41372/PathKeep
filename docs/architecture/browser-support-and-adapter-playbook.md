# Browser Support And Adapter Playbook

> This document is the source of truth for PathKeep's browser-support taxonomy. It separates code coverage from public promise so README/UI only claim what maintainers have recently validated.

## Current Support Taxonomy

### Validated Now

- `Google Chrome`
  - Discovery: shipping
  - Parser / ingest: shipping via the Chromium family pipeline
  - Public promise: allowed in README, onboarding, and release docs
  - Validation evidence required: local backup / recall pass on the current dev host
- `Safari` on macOS
  - Discovery: shipping
  - Parser / ingest: shipping as history-only baseline
  - Public promise: allowed only with the explicit Full Disk Access caveat
  - Validation evidence required: visible-but-unreadable state when access is missing, plus a successful backup once access is granted

### Implemented, Not Yet Publicly Promised

- Chromium-family adapters currently wired into discovery and ingest:
  - `Chromium`
  - `Microsoft Edge`
  - `Microsoft Edge Dev`
  - `Brave`
  - `Vivaldi`
  - `Arc`
  - `Opera`
  - `Opera GX`
- Firefox-family adapters currently wired into discovery and ingest:
  - `Firefox`
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
7. Parser, discovery, archive, and capability acceptance tests exist.
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
- Wire the adapter into archive ingest only after schema warnings, source-batch provenance, watermarks, capability tags, and source-kind naming are explicit.

### 5. UI And Copy

- Add browser icon coverage in `src/lib/browser-icons.tsx` if the UI should render a dedicated glyph.
- Add or adjust retention / caveat copy for onboarding, dashboard, settings, and troubleshooting surfaces.
- All new or changed user-visible strings must ship in `en`, `zh-CN`, and `zh-TW`.

### 6. Required Tests

- Parser tests for happy path and missing-table / damaged-shape behavior
- Discovery tests for default paths, overrides, and unreadable files
- Archive acceptance proving backup / recall on that adapter path
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
- `Safari`
  - profile remains visible when `History.db` is unreadable
  - needs-access guidance points to macOS Full Disk Access
  - baseline history backup succeeds once access is available

This recipe is intentionally narrower than the total code coverage in the repo. Public promise follows the validated recipe, not the broadest internal implementation surface.
