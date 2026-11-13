# PathKeep v0.5.0

Private test candidate containing the merged fix for reads stalling during semantic-index updates. Maintainer testing and release approval are pending.

PathKeep v0.5.0 improves browser backups, adds search sorting and a search builder, and keeps the existing semantic index available while a replacement is built. AI remains optional.

## Before upgrading

This version automatically upgrades the archive and semantic-index metadata. **v0.4.0 cannot open the upgraded data or import a portable archive exported by this version.** If you want to keep the option of returning to v0.4.0, quit PathKeep and save a complete copy of the data folder before upgrading. Keep your archive password. This is a downgrade compatibility limit, not a known failure of the upgrade.

The first backup after upgrading reads the selected browser histories again. Larger archives take longer, and Insights may rebuild in the background. History already deleted by a browser cannot be recovered from that browser.

## Changes

- Semantic-index updates count remaining work in short, cancellable reads. A long read no longer holds up every other read or later write, and the idle search indexer stops writing empty updates.
- Backups identify native visits by their address and browser timestamp, so a browser reusing its row numbers no longer silently skips those new visits. Navigation links survive the upgrade. Firefox Sync copies retain their own records while counting once.
- Runs distinguish complete, partially completed and failed backups, with results for each profile. The menu bar and scheduler retain the latest backup warning until a later backup resolves it.
- Link previews continue in the background after the backup returns.
- History can sort search results and build word or pattern searches without writing the syntax by hand. Quoted operators remain literal text, and later pages of a pattern search continue the same search.
- Semantic-index rebuilds publish complete generations. Searches keep using the existing complete index until its replacement is ready, and History describes the index that answered the search. Turning semantic search off stops embedding requests.
- Quitting while an import, export or other protected work is running asks for a decision. Notes and tags keep edits made while an earlier save is still on its way.
- Insights reads newly published results again and counts the same search once across browsers and engines. Ask explains when a model finishes without writing an answer.

## Known limitations

- Incremental semantic-index updates still scan the archive even when only a few visits changed. Large archives can keep background indexing busy.
- At 14.4M visits, some first-time Insights reads and searches with no exact match can take a minute or more. Large site-category changes are also slow.
- Navigation updates when a late opener is found, but previously built Insights paths wait for a full rebuild.
- There is no separate interface for clearing semantic-index files. A large index can use substantial disk space.
- Stop a running backup before deleting all data; the delete operation does not wait for it.
- A backup of an open browser can pause while obtaining a readable copy and can report that it used a copy.

## Distribution

The package set covers Apple Silicon and Intel Macs, Windows x64, and Linux x64. Windows installers remain unsigned preview builds; Linux packages remain preview builds. Safari needs Full Disk Access on macOS. Firefox support remains history-only; Atlas and Comet support remains limited to their validated macOS history profiles.

These preview packages are unsigned. Installation is manual; in-app automatic updates are unavailable. macOS may block the downloaded app, and Full Disk Access may need to be granted again after updating. Windows may show Unknown Publisher or a SmartScreen prompt.

Verify downloaded files against `SHA256SUMS.txt` and `RELEASE-MANIFEST.json`. These preview packages do not include updater signatures or `latest.json`. The JavaScript lockfile audit reported zero advisories. The Rust audit passed with eight documented exceptions.
