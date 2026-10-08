# Turso migration review ledger

Review findings for the move to Turso (`WORK-TURSO-STORAGE`), collected while the areas were built. They are fixed in one pass after every area is done, not area by area; the user asked for this on 2026-10-05.

Raw reports (Codex results, review texts, briefs) are kept outside the repo in `.git/pathkeep-wip/reports/` on the development machine.

The findings here are unverified. In the final pass, verify each one as BDFL:

- is it reachable?
- is it the root cause?
- does it fit the threat model?
- is it worth the complexity?

Then do one fix pass.

## Area 1 (write path)

### Corrections I already verified (the round-3 brief over-built these)

These are in `brief-area1-fixes3b.txt` (raw reports):

- **M2:** write the envelope before `open_owned`. This replaces the pending, promote and recover scheme, if round 3 built it.
- **Drop "a typed password during the startup open is dropped".** It can't happen; remove any retry logic added for it.
- **Drop the migration of old keychain entries.** There are no released users.
- **Owner endpoint:** don't add a bind-failure dialog. Binding at startup fails during setup, as HEAD did.
- **Statement benchmark:** stays an ignored measurement, with no per-row budget assertion.

### Open questions for the product

- [x] **Background mode never exits.** Done (no objection from the user): `src-tauri/src/idle_exit.rs`, proven by `owner_process::a_background_owner_exits_once_its_work_is_done_and_it_has_been_idle`. A `--background` launch (scheduled backup, CLI, MCP) stays resident in the tray, even when the tray preference is off.
  - Proposal: when the window was never opened and the tray preference is off, exit after all work is done (backup, import, indexing, requests) and a short idle period.
  - I told the user and am waiting for an objection.

### The canonical-URL rule changed silently (round 3)

- [x] Checked 2026-10-07: data-model.md states the rule; stars, annotations, enrichment, pages and the AI working set all call `visit_taxonomy::normalize_visit_url`; the converter recomputes stored keys (`encrypted_conversion_recomputes_keys_preserves_original_and_builds_search` covers `%7E`).

- Canonical normalisation now decodes percent-escapes of unreserved characters in the path, so `%7E` and `~` become the same page.
- RFC 3986 treats these as equivalent, so the rule itself is probably right. What to check:
  - Star and annotation keys stored under the old rule may no longer match their page.
  - History grouping changes.
  - Every place that computes the canonical key must use the same function.
- The SQLCipher converter (area 5) must recompute every stored canonical key (stars, annotations, enrichment) with the current rule. Put this into the area 5 brief.
- Write the rule down in data-model.md as a decision, not as a side effect.

### Reviewer round-3 minors not put into any brief

- [x] Firefox resume (M1c) is in the round-3 brief; check that it's done. (Done: Firefox and Safari carry `url_scan` like Chromium.)

## My frontend to-do (not review findings)

- [x] (Done: `search-coverage.tsx` in History and the command palette; E2E in `search-limits.spec.ts`.) **Search while the index is building** (the user agreed on 2026-10-05: "記得 UI 上要讓用戶知道這一點"). After an upgrade or an index reset, search covers only what is indexed so far. While that lasts:
  - every search result view (History search, the command palette, AI search evidence) shows clearly that the index is still being built, with its progress;
  - it must not look like the full answer. No "no results" without that notice, and counts are marked as partial;
  - the notice goes away once the index catches up.
  - Applies in en / zh-CN / zh-TW. E2E: hold the indexer mid-build and assert the notice and the partial count.
- [x] Streamed search presentation (provisional results, progress, counts arriving).
- [x] Run states, partial-import states, revert and restore triggers: interrupted runs say they stopped and that the next backup finishes; imports show importing / stopped partway / undo or restore stopped partway, with "Finish undo" / "Finish restore".
  - Not covered by E2E: reaching those states needs the app to quit mid-run, and the E2E harness can't restart the app. Rust tests cover the backend side (`process_exit_mid_revert_resumes_without_repeating_committed_pages`, interrupted runs at open). Add an app-restart harness to E2E, then a test that quits during an import and an undo.

## Area 2 (read paths), Codex's own report

- [x] (Already folded; the rule is now in `store_schema/mod.rs`.) **Schema migrations again.** Area 2 added "forward v2/v3 migrations", although round 3 had folded the unreleased schema into v1. In the final pass, fold them into v1 again. Then say plainly in AGENTS.md or the store-schema doc that unreleased schemas stay at v1.
- **legacy\_\* modules** (legacy_source_evidence, legacy_status, legacy_totals) hold the old SQLCipher helpers. Delete them in area 6.
- **Changed command shapes (frontend is mine):**
  - Timeline `page > 1` requires a cursor.
  - `load_recent_runs` and `load_import_batches` return `{items, nextCursor}`; unfinished batches come first.
  - `preview_import_batch` takes a cursor and limit and returns `previewNextCursor`.
  - The snapshot carries run and import cursors.
  - The dashboard has `recentRunsNextCursor`.
  - The repair report has new counts.
- **Benchmark** (14.4M encrypted): first timeline page 2.0 ms cold, day insights 26 ms cold, URL detail with 100k visits 1.9 ms, full export 34 s cold at 39 MiB peak.
- Not run on real 4-core / 8 GB hardware.
- Turso has no `foreign_key_check` and no way to interrupt a running statement. Codex replaced them with explicit relationship checks and cooperative cancellation.

### Codex review of area 2 (unverified; full text in review-area2-codex.md)

- [x] (Fixed 2026-10-07 for the AI key commands; content-fetch already read through the session's archive. Test: `saving_an_ai_key_returns_the_unlocked_snapshot_of_the_open_archive`.) [P2] `vault-worker/src/app.rs:180-181`: commands that return a snapshot, such as saving or clearing an AI provider key or changing content-fetch settings, go through a wrapper that builds a locked, empty snapshot. The frontend (`use-ai-providers.ts`) then overwrites the valid snapshot cache with it. Codex's proposed fix: pass the session's Vault through.
- [x] (Already done: `health-row.tsx` and the three catalogs.) [P2] `doctor.rs:49-53`: the new Doctor check codes (`canonical-references`, `cached-totals`, `search-outbox` and the four integrity codes) aren't in `HealthRow`'s `knownChecks` or the locale catalogs, so zh-CN and zh-TW show English. This is frontend and mine: add the labels in all three locales.

## Area 3 (search), Codex's own report (full text in the raw reports)

- **Performance to investigate (root cause, not symptoms):**
  - The final first page for a common term takes 8.3 s, and a profile filter 3.8 s.
  - The vault-search crate alone did a common word's first page in 40 ms warm, so the cost is in the vault-core layer (eligibility, visibility validation, hydration, or "pinned visit boundaries") rather than in Tantivy.
  - The first provisional results arrive in 11–93 ms, so the user does see results quickly, but 8 s to final is far off the crate's numbers. Profile it before deciding anything.
- **Indexer delay:** a star or note becomes searchable 1.75 s after commit. Is the delay in the poll interval?
- **Full rebuild:** 35.6 s at 404 MiB. Good: an upgraded archive's index is ready quickly.
- **Commands (frontend is mine):**
  - `start_search({request, onEvent}) -> {sessionId, indexer}`;
  - `next_search_page({sessionId, cursor})`;
  - `cancel_search`;
  - `search_indexer_state`.
  - Events: `indexer`, `step`, `error`, `cancelled`. A step carries page, items, provisional, progress, exactness, cursor, indexer state, snapshotSeq and `coverageComplete`.
  - Dev IPC adds SSE at `/searches/{id}/events`.
  - Contract: docs/architecture/search.md.
- [x] (None left: no `windowed` or truncation fields in search responses.) "Retaining historical wire fields without emitting truncation": check for dead fields (`windowed` and the like) and remove them.
- [x] (Read: a factual validation record, kept.) Check the new docs for process narration: `research/2026-10-search-integration-validation.md`.
- `bun run check` failed in Codex's sandbox at owner_process with kLSNoExecutableErr; that's a sandbox issue, so run it natively.

## Area 4 (intelligence, AI queue, agent, enrichment), Codex's own report (raw reports)

- **Full intelligence rebuild at 14.4M is not finished after 1,877 s.** It was stopped at the 15 GB data budget during refind generation; peak RSS was 151 MiB.
  - Find the root cause: is refind quadratic, is scratch I/O the bottleneck, or is it batch-size churn?
  - Measure on Modal, where disk is not a constraint.
- Incremental update and cold/warm read numbers were never measured.
- The changed request/response shapes are in `docs/architecture/research/2026-10-turso-area4.md`: list/detail cursors, job-history endpoints, enrichment provenance and agent output fragments. The frontend is mine.
- 17 SQLite-engine tests deleted and 23 renamed. Check that the deletions are justified.

### Area 4 profiling leads (Codex, incomplete benchmark; full text in the raw reports)

- Serial batch writes run PASSIVE and TRUNCATE checkpoints between batches. Check whether vault-store checkpoints more often than its design needs during long rebuilds and backups.
- [x] Turso encrypts with a software AEGIS implementation even though the CPU has AES hardware. Fixed 2026-10-07: pages are encrypted with AES-256-GCM, which detects AES-NI at run time (data-model.md, "Page encryption"). On x86_64 AEGIS only used hardware AES when compiled for it, which would crash CPUs without AES-NI. Measured (Modal VM, release): cold scan 0.53 s → 0.15 s, large commits 1.23 s → 0.80 s, 4,000 small commits 7.30 s → 6.15 s; small commits are dominated by commit cost, not the cipher. Check which cipher vault-store configures and whether the hardware path is available. This affects every encrypted read and write.
- The intelligence rebuild benchmark at 14.4M never completed (network loss, then gVisor). Re-run it on vm.

## Area 6 work list (mine)

- [x] **Open-file limit:** the app peaked at 156 open files during the full E2E run; a macOS app launched from Finder gets a soft limit of 256.
  - Done: the app raises its soft limit to 10,240 (never past the hard limit) before the archive opens (`vault_platform::raise_open_file_limit`).
  - Measured (`vault-core/tests/open_file_budget.rs`, Linux): one open encrypted archive holds 175 files, flat over 64 rounds of import + index commit + lexical and regex search. About 45 are epoll/eventfd handles of the ~15 connection threads; most of the rest are index segment files, each open once per reader generation (`EncryptedDirectory` keeps a file per handle). Open searches share the published generation; closing them releases it. The test fails if the count grows with imports or searches.
- [x] **`search-limits.spec.ts`** rewritten for the streamed search with exact counts.
- **Insights E2E** (5 specs) failed before area 4 landed. Re-run after it.

## Found on Linux (Modal)

- [x] (Passes on Linux since area 5's watermark fix; the folder walk is now in name order on every system.) `takeout::tests::import_takeout_persists_source_evidence_for_all_recognized_payloads` fails on Linux and passes on macOS: `last_source_batch_id` stays NULL for `takeout::browser-history`.
  - Likely root cause: the Takeout import processes recognised files in directory order. That is sorted on APFS and hash order on Linux, and a later file's write resets the browser-history watermark.
  - Fix the ordering dependence: per-file watermarks, or a deterministic order where no file overwrites another's.

## Found while running E2E for feat/turso-storage (fixed, not deferred)

- Regex search never finished in E2E, for two reasons:
  - A session restarted on every archive write, so a steady stream of writes (backup, link previews, page text) kept it from finishing. Fixed: sessions no longer restart. Hydration rechecks visibility and, once the archive moved past the session's snapshot, each page's current text, so revoked visits and text never reach a result. Counts stay those of the snapshot; the History screen starts a new search when the archive changes.
  - Each 512-visit chunk scanned the time index from the newest visit. Turso does not seek on `(t<? OR (t=? AND id<?))`; `EXPLAIN` shows `SCAN visits USING INDEX`. Fixed with `t<=? AND (t<? OR id<?)`, which plans as `SEARCH ... (visit_time_ms<=?)` (22k visits: deep chunk 6.4 ms → 0.12 ms). Plan test in `search/regex.rs`.
- **Regex speed beyond the literal shortcut (user, 2026-10-06: record, don't do now).** A regex with a literal of 3+ ASCII letters/digits reads only the candidate pages. Anything else (`\d+`, `^https?://`, every Chinese pattern) walks every visit: about 30–40 ms per 512 visits inside the debug E2E app, so minutes at 14.4M. Ideas, not started:
  - Walk URLs instead of visits: a regex only looks at the URL and its title, so match each URL once and count visits (in scope) only for matching URLs. Several times less work for every regex, Chinese included; results would no longer arrive newest first while scanning.
  - Chinese literals could use the index's CJK grams the way ASCII literals use trigrams.
  - Unexplained: the same 512-visit chunk costs 2–6 ms in a vault-core test (22k visits, encrypted, with or without a concurrent writer) but 30–40 ms inside the E2E app. Not read queueing: inside the app a chunk waits about 60 µs for a reader, then runs about 30 ms. Something in the app's data or store configuration (page cache size, row shape, three profiles) makes the same query about 7× slower. Find out why before tuning anything else.
- E2E `history-tools.spec.ts` "a regex Rust cannot run…" passes alone (the fragment-less `tok+io` walk of 22k visits finishes in 1.7 s) but is borderline in the full suite (one worker; failed twice, passed once): the walk's 44 chunks at about 30 ms each plus steps land near the 15 s wait. Same root cause as the item above.
- **Insights rebuild, measured (Codex, 14.4M visits, 4 vCPU / 8 GiB, release):** the visit-derived-facts stage alone took 9,194 s. Profiles show it dominated by software AES (the `aegis256` cipher, built without hardware AES in this configuration) and by checkpoint work inside Turso's commit path (566 of 575 sampled checkpoint stacks), i.e. many small commits each paying a checkpoint. Fix-pass targets: a hardware-accelerated cipher (`aes256gcm` has runtime detection; AEGIS needs compile-time flags), and far fewer, larger commits during rebuilds. Daily rollups and later stages were still running when the old Modal account's credit ran out.
- [x] (Fixed 2026-10-07: the daily stage committed each day's rollups in five small transactions, and every batch commit also runs a PASSIVE and, with the queue momentarily empty, a TRUNCATE checkpoint. That came to 10,721 commits for the 22k-visit fixture. Days now go in together, up to 4,096 rows per commit, with the cursor. Debug build on Modal (`vault-core/tests/intelligence_rebuild_bench.rs`): 27.4 s → 11.6 s, 74 commits, daily stage 6 s → 0.9 s per profile. The all-time overview warm-up, 3.4 s, is now the largest part; see the ranked-aggregates item.) E2E `background-work.spec.ts` fails: a full Insights rebuild of the 22k-visit fixture (three profiles, debug build) shows "9,083 of 9,083 · Running" after 60 s. It passed on SQLite (`redesign/v0.4`). Same cause as the rebuild-performance item: Codex's area 4 benchmark derived about 2,000 visits/s in release (3.58M visits in 1,714 s, unfinished).
- [x] (Fixed 2026-10-07. Since the area 6 refactor the wrong-password refusal is the worker's sentence, which the bridge sends as `code: archive-password-wrong` beside the message; the E2E helper `backend()` threw only the message. It now puts the code in front.) E2E `security.spec.ts` "changing the archive password needs the current one" fails with `storage-migration-incomplete` from `preview_rekey_archive`: rekey is area 5 (Codex, in progress).
- **Same keyset pattern elsewhere (fixed for every indexed list; aggregates still open).** About 20 cursor predicates used `a<? OR (a=? AND b<?)`.
  - Two more Turso findings: a cursor clause behind `?N IS NULL OR` never seeks either, and reads through the generation views (`sessions`, `search_trails`, …) read the whole generation by primary key and sort it on every page, whatever indexes exist.
  - Fixed: Insights sessions, search trails, query families, hardest topics, stable sources, friction signals, reopened investigations, refind pages, one profile's all-time path flows, observed-interaction visits and the job history. Each reads its `_rows` table through an index in list order, with the bounded cursor present only after page one. New v1 indexes are listed in `docs/architecture/store-schema.md`. Plan tests: `vault-core/tests/intelligence_keyset_plans.rs` and `intelligence_runtime/tests_queue.rs`.
  - Not a rebuild problem: the two rebuild walks with this shape (`intelligence_daily_rollups`, `intelligence_structural_stage`) and the enrichment claim page in `claims.rs` are test-only reference code. Production rebuilds page through `build_rows::source_page`, which already seeks. The test-only code was moved to the bounded form anyway.
  - Small by construction, left as is: notes/tags (`annotations.rs`) and stars (`stars/list.rs`) are written one user action at a time.
  - Entity-scoped, left as is: related trails of one refind page or one query family are collected and sorted per page; the cost follows that entity's visits, not the archive.
  - **Open: ranked aggregates recompute their whole input on every page.** No index can seek on `HAVING COUNT(*)<?`, and nothing materializes these rankings for an arbitrary date range: hub pages (the plan walks every visit-derived fact of the published generation, whatever the date range, then probes each visit, URL and trail member), frequent searches and top concepts (every keyword search event or term in range), top sites (every domain-day rollup in range), domain detail pages and landing queries (every URL of the domain), recent queries in `search_queries.rs` (every search event in range, ranked in Rust), and all-profile path flows (every flow of that length across profiles). At 14.4M visits that is up to a few million rows per page for the search- and trail-based lists. Needs a design: rank once per scope and page the stored ranking (an all-time materialization in the rebuild, or a per-scope result like the overview snapshot), not a cap.
- The first backup never finished. Turso compiled every SQL statement on every call (`Connection::execute` compiles each time), so a 10,000-row source-evidence batch spent minutes in the compiler.
  - Fixed: vault-store keeps up to 256 compiled statements per connection, resets them before reuse and clears them on DDL.
  - Also fixed: Turso's crates are built at `opt-level = 3` in dev builds.

## Found while moving checks to Modal (Linux, 16 cores)

- [x] (2026-10-07: the deadline no longer runs while the app answers a query, a product bug on its own, since a busy or large archive timed out sound scripts; Cranelift is optimised in dev builds. Test: `time_spent_answering_queries_does_not_count_against_the_script`. Not proven to be the flake's only cause; watch the next full runs.) `ai::code_mode::tests::the_async_bridge_is_safe_from_inside_a_runtime_worker` fails in the full suite with a `Time` trap but passes alone (32 s each in a debug build).
  - Cause: the wall-clock deadline is hit under parallel load.
  - The test depends on wall time, and compiling the debug wasm/JS engine is slow. Fix the test, or give cranelift/wasmtime an opt-level in the dev profile; don't just raise the deadline.
  - Present on redesign/v0.4 too.

## Benchmarks measured under gVisor

- Until 2026-10-06 ~13:30 local time, offload Sandboxes ran under gVisor, which makes SQLite system calls several times slower. That covers area 3's Linux test timings and any area 4 or area 5 benchmark run before the switch to `runtime = "vm"`.
- Re-measure those benchmarks on vm before quoting them, and check each report for which runtime it ran on.

## Area 5 (maintenance and conversion), Codex's own report plus merge notes

Unverified until the review pass. Full report: `docs/architecture/research/2026-10-turso-maintenance.md`.

- [x] (Re-measured 2026-10-07 after the cipher change, Modal VM 4 vCPU / 8 GiB, release: conversion 1,439 s + index 51 s = 1,490 s, peak RSS 624 MiB. About 25 minutes on the target.) **Conversion time:** converting a 14.4M-visit / 2.9M-page SQLCipher archive took 6,612 s before indexing plus 81 s for the index (4 vCPU / 8 GiB, peak RSS 738 MiB). It is a one-time upgrade with progress and resume, but 1.8 hours on the target machine is long; profile where it goes.
- **Encrypted → plaintext switch:** (2026-10-07, same setup: 459 s, peak RSS 860 MiB. Open: find what holds the memory before deciding whether it is too much.) 603 s at 14.4M, peak RSS 1,049 MiB (over a GiB on an 8 GB target).
- [x] **Saved page text in the export bundle:** the native bundle skipped `sidecars/intelligence-blobs/` (an exclusion carried over from the old export). By the user's rule that user data moves with the archive, it now travels, sealed with the export password; paths are relative to the blobs folder, so nothing needs rewriting. Covered by the bundle round-trip test.
- [x] **Agent chats in the export bundle:** decided by the user on 2026-10-07: chats are no more private than browsing history and must be backed up and moved with it. Export includes `agent.db`, encrypted with the export password; data-model.md updated.
- **Merge decisions (mine):** the Takeout watermark fix keeps area 5's version (loads the committed watermark), which also covers payloads with evidence but no visits. Claim recovery from area 4 runs only on the owner's recovering open. `Session::execute_rows` now compiles through the statement cache.
- **Fixed while merging (areas 4 and 5 were built in parallel):**
  - The converter copied tables by name, so the old `visit_content_enrichments` (fetched page text, per visit) was silently dropped: area 4 replaced it with `page_content_enrichments` (per canonical page). Conversion now maps each old row onto its page (newest fetch per page and source wins), verifies every row was carried, publishes each page's best enrichment (area 4's ranking, now one shared `BEST_ENRICHMENT_ORDER`) into search, recomputes embedding hashes with live indexing's summary rule, recounts `ai_embedding_totals`, and queues one full Insights rebuild (derived tables aren't converted).
  - Refind pages left the collision merge: they are derived data in area 4.
  - Maintenance copies (rekey, mode switch, snapshot, bundle) recreated tables and indexes but not views; area 4's intelligence schema has 16 views, so Insights and Doctor broke after any of them. Views are now recreated and covered by the copy test.
  - A fresh install still ran the old SQLCipher initializer, which created `history-vault.sqlite`; the next step took it for an old archive and converted it (empty) on every first run. The native session open now creates the archive.
  - Converter tests carry the old agent and intelligence schemas as fixtures instead of borrowing production legacy openers.
- **Decisions Codex made (check in review):** canonical-key collisions merge by newest timestamped content, deterministic ties, summed counters and unioned sets; every enrichment trigger is remapped; installation journals are recovered before credentials or conversion checks; the indexer can open deferred (no cycle until maintenance finishes); `experimental_encryption(true)` is now always set on Turso connections, also without a key.

## Area 6 (old SQLCipher code), found while mapping it

- [x] Conversion keeps the old `.sqlite` files, so three runtime paths could reach them on a native archive. Fixed:
  - The wipe preview opened the old `history-vault.sqlite` (running its schema migrations on it) when no archive was open, and reported that file's visit count. It now counts from the open archive only.
  - Asking running jobs to stop before a wipe only updated the old intelligence file and never the native job tables. It now asks the open archive's `ai_jobs` and `intelligence_jobs`.
  - Install recovery rolled back any import journal without a `stage`, an old-format one, even next to a native archive, which would have put an old `config.json` over it. Such a journal is now set aside as `archive/.pk-import-journal.legacy-stale.json`.
- [x] Old-format code split by who still needs it. v0.1–v0.3 were published, so someone can update straight after a crash in the old app; that recovery has to keep working until the converter goes.
  - Moved to `vault-convert-sqlcipher::legacy_recovery`: launch recovery for unconverted archives (import, rekey and restore journals the old app wrote, then config to the file's real mode) and the listing of old `.sqlite` safety snapshots. The worker picks native or old-format launch recovery, merges both snapshot lists, and passes the snapshot to keep into native retention.
  - Deleted from vault-core: the old export/import, rekey, retention, full restore and at-rest reconcile flows (`archive/maintenance.rs`, most of `at_rest.rs` and `migration.rs`, `maintenance_runs.rs`, `migration/fault_tests.rs`), `archive_key_opens` (the rekey password check is native-only; the preview already refused unconverted archives), and their tests. Creating a password envelope next to an unconverted archive now refuses with `archive-conversion-required` instead of testing the password against the old file.
  - Tests: the direct recovery tests moved with the code. Those that reached a crash state by killing a deleted flow now build that state on disk (`legacy_recovery/fixtures.rs`). Quality-matrix rule 2 keeps a checker on both sides: `check_config_disk_consistency` now reads the native databases and runs after native mode switch, restore and install rollback.
- [x] Step C: vault-core no longer opens an old-format file. Deleted: the rusqlite archive, source-evidence and intelligence openers, the old schema migrations and their SQL, `legacy_status.rs`, `storage_migration.rs` (its refusal only guarded the old openers), the old enrichment table schema, and `test_support::legacy_fixtures` with its copy-into-the-vault layer.
  - Tests seed the native vault directly. `test_support::history::History` writes through the backup ingest path (`ArchiveChunkConsumer`), so pages, totals, search documents and the outbox are what a backup leaves; the AI tests use their native fixtures (`ai/tests_native/fixtures.rs`). Several tests had been creating old-format files next to a native vault that never read them; those calls are gone.
  - Old-format fixtures for conversion and recovery tests live in `vault_convert_sqlcipher::test_support` (feature `test-fixtures`), built from the SQL the old releases shipped (001–016). The v0.2.0 upgrade test became a conversion test there.
  - `lmstudio_agent_e2e.rs` had been importing into a Vault it then threw away, so the agent searched an empty archive; it now uses the Vault the tool context reads (still ignored, needs LM Studio).
- **Fix pass, found while doing this:**
  - [x] Bundles exported by v0.1–v0.3 (format 1, SQLCipher inside) are refused with "unsupported bundle format 1". Moving to a new computer from an old version then needs the old app. Decided by the user on 2026-10-07: convert format-1 bundles on import. Done: only v0.3 wrote whole-app bundles (v0.1 and v0.2 had no whole-app export or import; their remote-backup zips have no format version). `vault_convert_sqlcipher::bundle` unpacks one as the old app's data folder in staging, converts it there, rewrites the exporting computer's paths and installs it like a native bundle (`tests/bundle.rs`). The import dialog says the password is the old archive password and that 0.3 files carry no Ask chats.
  - [x] `run_full_archive_restore` routes any `snapshot_path` that is a file to the converter's restore. The old flow refused paths outside `raw-snapshots/` (canonicalized). Check native restore too. Done: a full restore takes only a snapshot `list_recovery_snapshots` lists (compared canonicalized), else `snapshot-not-listed`; the checkpoint replay already required a recorded snapshot row.
  - `ProjectPaths::archive_database_path` / `source_evidence_database_path` mean the old `.sqlite` files, until `Vault::open` rewrites them to the native `.db`. The same field names two files depending on where the paths came from; split into explicit old-format and native fields.
  - [x] Archives from older releases have older schemas (the old app migrated them on open). Checked: an authentic v0.2.0 archive (shipped migrations 001–010) converts without loss (`vault-convert-sqlcipher/tests/old_versions.rs`); the converter copies the columns both sides have. v0.1.0 is not covered yet.
  - Launch recovery for an unconverted archive ran only on the setup path, but the shell asks for the upgrade assessment first and the update screen converts directly. Fixed in this step: assess, convert and initialize all run it first; an archive it can't bring back opens the recovery screen.
  - [x] An old install that changed its password under the old archive-only rekey can have `source-evidence.sqlite` keyed with the previous password (BACKLOG `WORK-ENCRYPT-KEYDRIFT-A`). The converter opens it with the archive password and fails the whole conversion. Decided by the user on 2026-10-07: convert everything else, keep that file untouched, and tell the user which part was not carried over. Done: with the password, the assessment lists `filesLocked`; the update screen checks before starting and asks; conversion skips those files and clears the watermarks that named their batches (`a_file_keyed_with_an_earlier_password_is_named_left_in_place_and_skipped`). v0.3's own export opened source evidence with the current key, so bundles never carry such a file.
  - The old SQLCipher benchmarks (`archive_scale_bench` sections) went with the old format. Doctor and import-batch revert/restore have no native at-scale benchmark yet; the native read, export and Insights ones remain.
  - [x] (Cause found 2026-10-07: the error was "this archive is owned by another process … would block". A dropped `OwnerLock` only closed its file, and the lock belongs to the open file: a child process another thread was spawning held a copy of the descriptor until it started its program, so the next acquire failed. The app spawns children too, so a reopen after a failed open, a wipe or a move could hit it. `OwnerLock` now unlocks on drop; test `dropping_releases_ownership_while_a_copy_of_the_descriptor_lives`.) Flaky: `vault::tests::a_password_open_beside_an_unconverted_archive_creates_nothing` failed once in a full workspace run (its error did not start with `archive-conversion-required`; the message wasn't captured) and passed alone and in the next full vault-core run. Capture the error next time it fails.
  - [x] Flaky: `worker_bridge::recovery_tests::assessment_and_bootstrap_finish_a_published_mode_switch_after_process_exit` failed once in the full desktop suite (the install journal was no longer pending after the child exited at the checkpoint) and passes alone. Likely another test in the same binary resolves the project root from the environment without `lock_env` and recovers this test's root. Find it. No such test found by reading; the scenario now runs alone in a child process (`published_mode_scenario`), so nothing else in the binary can touch its folder.

## Found in the review pass

- [x] **The old-archive upgrade has no app-level test** (side agent, 2026-10-07). Done: `tests/e2e/upgrade.spec.ts`, after wipe, on a folder written by `vault-convert-sqlcipher/examples/v0-3-archive.rs`. Only crate tests cover the conversion, and merging found it had been silently dropping all saved page text. Add an E2E that seeds an old-format archive (v0.3 layout, encrypted, with saved page text, stars, notes, chats and Insights), starts the real app, goes through the update screen and checks that the data is there afterwards.
- [x] **Inside the app, every storage step asked the logger whether to log it** (found 2026-10-07 while chasing the `background-work` E2E). Turso opens `tracing` spans on each B-tree and VDBE step. With no `tracing` dispatcher, `tracing` hands each span to `log`, and the app's logger (Debug for our crates) runs its per-module filter every time; stack samples of the hot store thread were all inside `fern::…::find_module`. Tests have no logger, so they never paid it. The same 21.6k-visit archive rebuilt Insights in 7.1 s from a test and took over a minute in the app. Fixed: `vault_store::route_engine_diagnostics_to_log()`, installed at app start, wants only engine warnings and errors, so the spans are off at their callsites (test: `vault-store/tests/engine_log.rs`). E2E after the fix (debug app on Modal): first-run 61 s → 31 s, backup 26 s → 9.5 s, background-work passes in 26 s.
- [x] **When the archive closed under an open window, every screen showed an empty archive** (found 2026-10-07: the stricter `security.spec` check after a cleared session key). The window decides "locked or not" only when it boots, so reads refused with `archive-password-required` left Home at "Saved visits 0". Fixed: a query refused because the archive is closed (`archive-password-required` or `lock-required`) sends the window back through its gates (keychain, then password or passcode), and Home's tiles show "—" for a failed read instead of 0. `move.spec` waits for the archive to be open before it reads the visit total.
- [x] **A moved archive cleared this computer's keychain** (found 2026-10-07, first full run where `move.spec` reached `wipe.spec`). Export writes "remember in keychain" off, and import took the setting from the file, so every move cleared the keychain and the person had to type the password at each launch. Import now keeps this computer's setting, as it already did for App Lock (`native::keep_machine_settings`, also used by the v0.3 bundle import). `move.spec` checks that the window reopens the moved archive from the keychain alone.
- [ ] **Re-measure the key timings in the app** (side agent, 2026-10-07). Every benchmark so far ran in a test program, which had no logger and so never paid the cost above. The release app paid it too (cheaper per call, never measured). With the fix in, measure in a release build of the app: Insights rebuild, search (common word, regex, profile filter), conversion and backup at 14.4M, and compare with the test-program numbers. Also recheck the regex item above ("7× slower in the app"), which is very likely this.
- [x] (Done 2026-10-08: `tests/fixtures/v0.3-archive/` holds a data folder and a bundle made by v0.3.0's own app, built from the `v0.3.0` tag on Modal and driven through its dev bridge: an encrypted archive with two Chrome profiles over a year (1,808 visits), a hand-imported Chrome file (120), both spellings of one address, a Chinese address, page and domain stars, a note, tags, page text fetched from example.com and iana.org, a chat, Insights, App Lock. `upgrade.spec.ts` now updates that folder and imports that bundle, and checks against what v0.3.0 reported (`expected.json`); the stand-in `examples/v0-3-archive.rs` is gone. The generator and how to rerun it are in the fixture's README.) **The upgrade E2E runs on a stand-in old archive** (side agent, 2026-10-07). `v0-3-archive` creates the tables from the SQL the old releases shipped, but writes the rows itself: a handful of pages, a star, a note, one chat. It can't catch anything a real v0.3 archive holds that the stand-in leaves out.
- [ ] **Convert a v0.3.0-made archive at 14.4M on the target size** (split from the item above). The 14.4M conversion benchmark uses an archive written by the old fixture code, not by v0.3.0. Grow the v0.3.0 generator (or feed v0.3.0 a 14.4M browser history) and time conversion on 4 vCPU / 8 GiB.
- [x] (Rewritten 2026-10-07; Ask answers also show a note when a search ran on an unfinished index, `a_search_on_a_partial_index_adds_one_note_to_the_answer`.) `docs/architecture/ai-security-posture.md` still describes the SQLCipher archive and says chats are excluded from the bundle. Rewrite for the native store and the 2026-10-07 bundle decision.

- **Insights showed partial numbers as final during the first build** (E2E `insights-drill` "Frequent searches", 80 of 97). A build publishes each browser profile as it finishes it; modules with no finished build reported `idle`, and KPI tiles never showed `updating`. Fixed 2026-10-07: while a build is queued or running, a module without a finished build is `stale` (`first-build-running`), and KPI tiles stay on screen marked as updating (`aria-busy`).
- [x] (Done 2026-10-07: `query_family_queries` holds each family's wordings, and `get_search_families` looks a search up by them, across all time, as the page's "in total" says; test `every_family_of_a_search_is_found_however_old_it_is`. The likely cause of the E2E's 80 of 97: a browser's family last used more than 30 days ago fell out of the 30-day list (to confirm when that E2E passes).) **Search detail filters only the first 200 query families on the client** (`useSearch` in `drill-queries.ts`). At 14.4M a search's families need not be among the newest 200, so "Times searched" can undercount. Fix: a member table of normalized queries per family, written by the build, and a lookup by query; to do after the keyset branch merges (same schema file).
- **Top sites counted search result pages as site visits** (E2E `insights-drill` day test, date-dependent). Both Top sites (Insights and the day view) now count them as searches only; the change is in features/intelligence.md.
- Search at 14.4M (Modal VM, 4 vCPU / 8 GiB, release, AES-GCM), warm: rare word 2 ms, medium word 0.3 s, date filter 0.6 s, profile filter first results 20 ms but final count 8.3 s (also cold). The profile-filtered count is the area 3 item above, still open; the common-word case needs a rerun (its log lines were cut).

## Found on the user's own data (2026-10-08)

A release build of `redesign/v0.4` ran on an APFS clone of the user's v0.3 data folder on their Mac (386,891 visits, 212,023 addresses, 10 profiles, April 2025 to October 2026; folder 16 GB, of which Insights 9.9 GB). Separate Keychain item, dev bridge on; the real folder and app were not touched. Nothing left the machine.

- Update from v0.3: about 4 min (envelope 20:19:49, index committed 20:23:41, installed 20:23:44), peak RSS 1.9 GB, archive 3.45 GiB. The search index was built inside it in about 10 s. Insights full rebuild afterwards: 2 min 37 s.
- Dashboard 24 ms, top sites (30 days) 76 ms. Search, first rows 10–23 ms; exact counts: "rust" 40 ms, "google" 0.37 s, "com" (209k visits) 0.63 s, CJK 49 ms, `site:github.com` 0.36 s, regex with a literal 0.2 s, regex without one 1.2 s.
- Export ("move to another computer"): 111 s, 5.6 GB, `unzip -t` clean. The installed v0.3 had failed on the same data with the ZIP64 error ("Large file option has not been set"); this build writes ZIP64.
- [x] **Switching Insights to 1y crashed the whole screen** ("date value is not finite in DateTimeFormat format()"), reported by the user on this data, but any data did it: the E2E fixture has 420 days. The one-year chart asked the backend for weekly buckets, whose keys are ISO weeks (`2025-W41`), and formatted them as dates. Fixed: charts ask for days and group them into Monday-start weeks themselves (`insights/buckets.ts`, shared with the site screen), so every bar key is a date. A card that throws while rendering now shows its error and a retry instead of taking the screen down, and the error is recorded in the log. `insights.spec` now clicks every range on Insights and on a site screen; before, every Insights test used 30 days only.
- [ ] **Insights overviews got 3–4× slower after the export and stayed slow**: primary all-time 0.87 s → 3.5 s, secondary 4.6 s → 15.8 s (even 30 days 12 s), repeatable. A stack sample during the slow read showed `pwrite` and `fsync` from a read-only request. Reproduce on the synthetic data on Modal: read, export, read again.
- [ ] **Two Insights jobs stayed queued forever** after the rebuild (`queued 2, running 0` for over an hour, last activity when the rebuild finished). Find what they are and why the worker never picks them up.
- [ ] **Memory stayed at 3 GB after the export** (2.6 GB peak during it, 0.55 GB before). Too much for the 8 GB target; find what the export keeps.
- [ ] **A profile filter with no search words**: first rows 66 ms, exact count 3.3 s for one profile's 84k visits. Same family as the 8.3 s profile-filter count at 14.4M above.
- [ ] tantivy's INFO lines (segment flushes, merges, deletes) fill `rust.log`; cap that crate at WARN like the engine.
- The dashboard counts 10 profiles while the profile list has 24 entries (14 Atlas logins with no visits). Check whether empty profiles should be listed.

## Area 5+

(appended as blocks finish)
