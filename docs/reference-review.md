# Reference Review

This project's browser/OS support and data-shape decisions were reviewed against:

- `localfirstapp/1History`
- `purarue/browserexport`

These repositories are cloned locally under `reference/` and kept out of version control.

## What The Codebase Already Implements

This section tracks implementation coverage, not the public support promise. The public taxonomy lives in [browser-support-and-adapter-playbook.md](./architecture/browser-support-and-adapter-playbook.md).

- Chromium-family desktop browsers:
  - Google Chrome
  - Chromium
  - Microsoft Edge
  - Microsoft Edge Dev
  - Brave
  - Vivaldi
  - Arc
  - ChatGPT Atlas
  - Perplexity Comet
  - Opera
  - Opera GX
- Firefox-family desktop browsers:
  - Firefox
  - LibreWolf
  - Floorp
  - Waterfox
- Safari on macOS
- Multi-profile discovery on macOS, Linux, and Windows path variants
- Linux path variants including common flatpak and snap layouts
- Incremental ingest with duplicate suppression for repeated imports
- Plaintext exports and structured archive/export formats

## Ideas Borrowed or Validated

- Browser location heuristics:
  The Chromium/Firefox/Safari path coverage in `vault-core/src/chrome.rs` was expanded using the location patterns and support matrix used by those projects.
- Direct database snapshotting instead of live reads:
  The archive keeps staging copies of browser databases before parsing, which aligns with the durability/safety assumptions those tools also lean on.
- Repeated-backup workflow:
  Both reference projects assume history should be backed up periodically because browsers age history out; that validates this app's due-aware scheduler model.
- Merge/deduplicate mindset:
  `browserexport` treats repeated exports as mergeable history streams. That influenced the app's import-batch dedupe and revert semantics.

## Gaps Worth Considering

- Firefox Android / Fenix manual import adapters
- Pale Moon
- qutebrowser
- More explicit custom-path onboarding for unsupported-but-compatible browser databases
- Additional parser adapters for non-Chromium / non-Firefox SQLite schemas beyond Safari

## Current Position

PathKeep's internal implementation surface is broader than its public promise. The codebase currently covers the main desktop browser families directly in the app, but README/UI now only promise the adapters backed by current validation evidence. The remaining gaps are mostly promotion evidence, niche browsers, mobile browser exports, or custom-path onboarding improvements.
