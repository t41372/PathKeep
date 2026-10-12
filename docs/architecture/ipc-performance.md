# IPC read-path performance

What each command costs at archive sizes that matter, what was slow, what changed, and how to measure it again. Target: 14.4M visits on a 4-core, 8 GB machine, and the UI never waits on a command it did not ask for.

## How the numbers were taken

> These findings were measured on the SQLCipher archive, before the move to Turso. The SQLCipher benchmark (`archive_scale_bench`'s `PATHKEEP_ARCHIVE_BENCH` sections) was removed with the old format in October 2026. Native measurements: `turso_reads::encrypted_archive_reads_at_scale` (timeline, day insights, URL detail, overview, source stats, full export) and `turso_intelligence::encrypted_intelligence_at_scale` in the same test binary, both `#[ignore]`d, encrypted, 14.4M visits, and removing their data afterwards. Doctor and import-batch revert/restore have no native at-scale benchmark yet.

The SQLCipher benchmark built a synthetic archive through the real schema (3 profiles, one URL per four visits, 20,000 domains, five years of visits), let the real open path project it into the search database, then timed each call. Calls that mattered for memory also printed the peak Rust heap and SQLite's own peak (`sqlite3_memory_highwater`).

Machine: Apple Silicon, 18 cores, 64 GB, SSD, release build. That is far faster than the target, so read the before/after ratios, not the absolute times. Each figure is the median of 5 to 20 calls in a warm process (the archive was already opened once).

"Before" is the code at `redesign/v0.4` before this work (`13223bd6`); "after" is with the changes below. The benchmark was run before and after at 1M visits and after at 14.4M. The 14.4M "before" figures for removed queries were timed directly on the same database file: Python's `sqlite3` (SQLite 3.53) for the old browse query, and the benchmark itself for the old search count.

## Findings

### 1. The History list sorted every visit to return one page

`query_history` ordered by `CASE WHEN :sort = 'oldest' THEN ... END`, and the only time indexes on visible visits start with the profile or the URL. SQLite therefore read every visible visit, sorted them in a temporary B-tree, and returned 101 rows. `includeTotal: false` removed the count but not the sort.

Fix: migration 016 adds `idx_visits_visible_time_id (visit_time_ms, id) WHERE reverted_at IS NULL`; the list query is now two statements (newest, oldest) built from one body with a plain `ORDER BY`, and the time and cursor bounds are plain comparisons with sentinel values instead of `:x IS NULL OR ...`, so SQLite can seek. A plan test (`list_plan_tests`) fails if either direction, with or without a cursor, goes back to a temporary sort.

|                                             | 1M before | 1M after | 14.4M before | 14.4M after |
| ------------------------------------------- | --------- | -------- | ------------ | ----------- |
| browse, first page, `includeTotal: false`   | 911 ms    | 1.7 ms   | about 26 s   | 1.9 ms      |
| browse, page 51 by cursor                   | 970 ms    | 1.4 ms   |              | 1.9 ms      |
| browse, first page, exact total             | 1,688 ms  | 779 ms   |              | 16.4 s      |
| keyword "topic 42", first page, no total    | 49 ms     | 51 ms    |              | 1.1 s       |
| keyword "topic 42", first page, exact total | 70 ms     | 86 ms    |              | 2.1 s       |

Cost: the index is 274 MB at 14.4M visits (the per-profile index next to it is 326 MB, the visits table 676 MB). It is built once on upgrade behind the "Upgrading your archive" screen, since a pending schema migration already counts as a pending upgrade: 0.5 s at 1M and 4.0 s at 14.4M on this machine.

The exact-total browse count is still a full count (16 s at 14.4M). The new frontend never asks for it: it only asks for an exact total when there is search text (`useSearchTotal` in `src/features/history/queries.ts`), and runs that beside the list rather than before it. Keyword search itself is not touched by this change; see open risks.

### 2. `app_snapshot` opened the archive five times

`archive_status`, `load_recent_runs` and `load_import_batches` each opened the archive, and `ai_index_status` and `intelligence_status` each opened the intelligence database, which attaches the archive again. For an encrypted archive every one of those derives the SQLCipher key. `load_import_batches` also re-ran `create_schema`, which takes a `BEGIN IMMEDIATE` write lock: during a backup that waited out the 5 s busy timeout and then failed the whole snapshot.

Fix: `vault_core::load_snapshot_reads` opens the archive once and the intelligence database once and serves all five reads from them. The redundant `create_schema` is gone; `open_archive_connection` already bootstraps the schema once per process. Before onboarding the intelligence status is the empty default instead of opening (and creating) the intelligence database.

| `app_snapshot`                                         | before  | after       |
| ------------------------------------------------------ | ------- | ----------- |
| encrypted archive, 100k visits (one open: 43 to 47 ms) | 237 ms  | 94 to 97 ms |
| plaintext archive, 1M visits                           | 14.5 ms | 4.3 ms      |
| plaintext archive, 14.4M visits                        |         | 3.0 ms      |

### 3. Every archive open wrote to the search database

Already fixed in the redesign import (`4bbd8d62`), measured here. Each open verified the search schema by rewriting its meta row (a write transaction) and ran `SELECT COUNT(*) FROM search_documents` to decide whether to seed. The schema check now runs once per process, the seed check is a `LIMIT 1` probe, and the search database is in WAL mode so readers do not wait on the projection writer.

| per archive open                    | 1M visits (250k documents) | 14.4M visits (3.6M documents) |
| ----------------------------------- | -------------------------- | ----------------------------- |
| old search `COUNT(*)`, now removed  | 6.9 to 8.0 ms              | 99 ms                         |
| old meta row upsert, now removed    | under 0.1 ms               | under 0.1 ms                  |
| current existence probe             | under 0.01 ms              | under 0.01 ms                 |
| whole `open_archive_connection` now | 0.9 to 1.0 ms              | 0.7 ms                        |

So before the fix every command against a 14.4M archive paid about 100 ms here before doing its own work.

### 4. Every command re-reads the config

Measured and left alone. `load_config` plus the App Lock hydrate and unlock check takes 0.01 to 0.02 ms (median of 200), at either archive size. A cache would have to be invalidated by every writer, including the scheduled backup in another process, for no visible gain.

The part of config loading that is slow is the system keychain lookup for "is an AI provider key saved", which the redesign import already caches for 60 seconds. `app_snapshot` also calls `keyring_status()`, which asks the keychain whether an archive key is stored on every snapshot. The benchmark uses the file-backed test keyring, so that cost is **not measured**; on macOS a keychain search can take tens of milliseconds.

### 5. `query_history` counted every match on every page

Already addressed in the redesign import: `includeTotal: false` skips the count and reads one extra row to know whether there is a next page. The infinite History list uses it; only the search total asks for the count. A test (`uncounted_history_pages_walk_the_same_rows_as_counted_pages`) checks that uncounted cursor pages return the same rows as a counted query, once each.

### 6. Search over a common word walked every match

> Replaced. Search now runs on the Tantivy index and covers every match, streamed in steps; see `search.md`. This section records why the earlier bound existed.

Search asks `query_history` with `groupByUrl: true`: one row per URL with its visit count (`vault-core/src/archive/history/grouped.rs`). Until this change, ranking, grouping and the exact count all walked the full match set: both FTS tables scored every matching document with `bm25`, every matching `url_id` was aggregated over its visits, and the totals ran it all again. A word in every one of the 3.6M titles ("topic") took 10.5 s for the first page and 26.6 s for the History header's count.

Fix (`vault-core/src/archive/history/grouped/window.rs`): the matching `url_id`s are collected first into `temp.history_ranked_urls`, which the visit list, the page list and the totals then read instead of the search index. Each index is read in `url_id` order and stops after 25,001 URLs that have a visible visit inside the browser, profile, date and URL filters. A word under the cap is ranked whole, exactly as before. Above it, only the 25,000 most recently archived matching URLs (the least recently archived for `oldest` order) are ranked, listed and counted; the response says `windowed: true` with `totalExact: false`, the header shows "25,000+ pages" and one line saying not every match was ranked and that another word narrows it. What is bounded now:

- the scan, at `cap + 1` qualifying URLs per index (one visit-index probe each);
- relevance scores, at one per ranked URL; when the term index alone passes the cap, a URL both indexes match keeps its term score, so the trigram index's posting list is not read just to reorder the window (recorded in the cursor so later pages score the same way);
- grouping, sorting and both totals, at the window: the count stops at the cap instead of counting everything;
- a narrow date range: when it holds at most 250,000 visits, its URLs are collected first (`temp.history_window_scope`) and matches are tested against them instead of probing each match's visits.

The window's edge is written into the cursor (`w…|`), so later pages read the same window even if a backup lands between them. Filters choose the window, never trim it afterwards. Why "most recently archived" and not "most recently visited": an FTS index can hand out matches in `url_id` order without scoring them all, but it has no visit-time order, and testing the newest visits one by one against the index costs 20 to 70 µs per visit (measured with a correlated `MATCH … AND rowid = ?` probe). The trade-off is that a page archived long ago and still visited today can fall outside the window of a word on more than 25,000 pages.

Measured on the 14.4M archive (`PATHKEEP_ARCHIVE_BENCH_ONLY=search`), `limit: 100`, median of 3 to 5 calls. The machine was shared with several builds throughout (load average 15 to 40), so the same unchanged query moved by up to 50% between runs; read the ratios. "pages" is the History list (`groupByUrl`, no total), "count" the header's `limit: 1, includeTotal: true` call, "visits" the visit list (export, AI tools) with and without the total.

| 14.4M visits                             | before   | after  |
| ---------------------------------------- | -------- | ------ |
| rare term, pages, first page             | 51 ms    | 73 ms  |
| rare term, count                         | 104 ms   | 73 ms  |
| "topic 421" (6,853 pages, exact), pages  |          | 346 ms |
| "topic 421", count                       |          | 368 ms |
| "topic 42" (73,731 pages), pages         | 704 ms   | 304 ms |
| "topic 42", count                        | 1,336 ms | 349 ms |
| "topic 42", visits with total            | 2,582 ms | 537 ms |
| "topic" (every title), pages, first page | 10.5 s   | 288 ms |
| "topic", pages, second page by cursor    | 11.2 s   | 295 ms |
| "topic", count                           | 26.6 s   | 318 ms |
| "topic", visits, first page              | 30.6 s   | 338 ms |
| "topic", visits with total               | 60.5 s   | 415 ms |
| "topic", visits newest-first (AI tools)  | 29.4 s   | 337 ms |
| "topic", last 7 days, pages              | 5.6 s    | 727 ms |
| "topic", last 7 days, count              | 16.8 s   | 799 ms |
| "topic", first year, pages               | 7.5 s    | 399 ms |
| "topic", first year, count               | 20.1 s   | 424 ms |

"topic 42" matches about 90,000 URLs once the prefix and trigram matches of "42" are counted, so it is now windowed (25,000 of its 73,731 pages); "topic 421" (about 18,000 URLs) is the medium term that still ranks whole. The rare term costs about 20 ms more than before: the index is now read newest-first and probed per match, a fixed cost for a one-row result. A second run of the old code stalled on the first "topic 421" case (5 repetitions of the visit list with its total) for 25 minutes and was stopped; the cause was not investigated.

Choosing the cap: with "topic", the first page costs about 200 ms fixed (the posting lists, read for the matches and for the relevance weight) plus about 4 µs per ranked URL. In the same run the first page took 290 ms at 25,000 and 390 ms at 50,000 (1.0 s and 0.73 s with the one-week filter). The target machine is several times slower, so 25,000 keeps the worst common word near half a second there; it is still 250 screens of results.

A plan test (`keyword_pages_read_visits_through_the_url_time_index`) fails if the page list, the totals, the visit list or the window scan stop reading visits through `idx_visits_visible_url_time`, scan `visits`, or if the window scan sorts. Tests for each failure mode (a rare term windowed, the best match dropped, counts claiming exactness, cursor pages skipping or repeating across the window edge, filters applied after the window) are in `grouped/tests.rs`.

Semantic search already returned one row per page; its header shows pages only.

### 7. Regex searched only the newest 50,000 visits

> Replaced. Search now runs on the Tantivy index and covers every match, streamed in steps; see `search.md`. This section records why the earlier bound existed.

A regex cannot use an index, so regex search read the newest 50,000 visits inside the filters and matched each URL and title in Rust, and reported counts for that window as if exact. At 14.4M visits that is 0.35% of the archive: a regex for a page last visited a few weeks back silently found nothing.

Fix (`vault-core/src/archive/history/regex_scan.rs`): each request scans newest-first from where the last one stopped, for at most 200 ms (a 4M-row backstop), and returns the matches, a cursor at the last visit it consumed (`x|time|id`), and `regexScan: { scannedToMs, complete }`. Scrolling the list continues the scan; the list also asks for the next chunk by itself until it has a page of rows, then shows "Keep searching" (or "Stop" while it runs) next to "48+ pages so far · searched back to Mar 2025". Counts say "so far" until `complete`; `totalExact` is true only when one response scanned the whole filtered range. A grouped row counts the visits of its own chunk, and the History list adds up rows of the same page. Browser, profile, date and domain filters are conditions of the scan query, so a date filter bounds the scan. Nothing runs between chunks: the cursor is the whole state, and an abandoned search is simply not continued. The scan query carries only the domain filter (regex mode has no `site:`-style operators), matches on borrowed text and builds a row only for a match, and bounds its time range by the cursor so each chunk seeks instead of re-reading what earlier chunks covered (without that bound the walk was quadratic: a whole-archive scan did not finish in 10 minutes).

Measured on the same 14.4M archive (`PATHKEEP_ARCHIVE_BENCH_ONLY=regex`, under `/usr/bin/time -l`), grouped, `limit: 100`:

| 14.4M visits                                               | after                             |
| ---------------------------------------------------------- | --------------------------------- |
| rare regex (one URL, 7 visits), first chunk                | 204 ms, about 155,000 visits      |
| rare regex, whole archive                                  | 18.9 s, 93 chunks                 |
| common regex (`topic 4\d\d$`, one title in 5), first chunk | 2 ms (the page fills at once)     |
| common regex, whole archive by cursor                      | 59.7 s, 28,811 chunks of 100 rows |
| peak memory of the process for both walks                  | 92 MB footprint, 362 MB RSS       |

Before, the same rare regex answered in one call, after reading 50,000 visits, with nothing found. The per-request overhead (opening the archive, preparing the query) is about 2 ms, which is why a common regex walked to the end costs more than a rare one: the list never does that, it stops at a page of rows until the user scrolls. Memory per request is at most `limit + 1` matched visits, or `limit` pages plus one map entry each.

Tests for each failure mode (a match older than the first chunk, rows repeated or skipped at chunk edges, a partial count shown as final, a scan left running, filters applied after the regex, the regex dialect) and a plan test (the scan walks `idx_visits_visible_time_id` without sorting) are in `regex_scan/tests.rs`. E2E runs with `PATHKEEP_DEBUG_REGEX_CHUNK_ROWS=2000` (debug builds only), so the fixture's 21,000 visits take about eleven chunks and `search-limits.spec.ts` proves the oldest visits of a page are counted.

Operator-only searches (`site:docs.rs` with no words) start from every row of `urls`, so they cost like the "every page" row above. The History tag chips made one of them common, `tag:name`, so a search whose operators include `tag:` and whose words are empty starts from the tagged URLs instead (`url_tags`, then `idx_urls_url`, then the visit index per URL): its cost follows the number of tagged pages, not the archive. A plan test (`tag_pages_start_from_the_tagged_urls_not_every_url`) fails if `urls` or `visits` is scanned. Not measured at 14.4M; with tens of tagged pages it is a handful of index lookups. Other operator-only searches (`site:` alone, `-word` alone) still scan; they are typed rarely and were left as they are.

### 7. The doctor copied every visible visit id into memory

`doctor_report` and `repair_health` found derived rows that point at a hidden or missing visit with `visit_id NOT IN (SELECT id FROM archive.visits WHERE reverted_at IS NULL)`. SQLite runs that subquery once and stores every visible id in a temporary index (`LIST SUBQUERY` in the plan). It did this for `visit_derived_facts`, `search_trail_members` and `ai_embeddings`, and the AI indexer did it again to find stale embeddings.

Fix: each check is now `NOT EXISTS (SELECT 1 FROM archive.visits WHERE visits.id = t.visit_id AND visits.reverted_at IS NULL)`, which looks each row's visit up by primary key (`SEARCH visits USING INTEGER PRIMARY KEY`). The same rewrite went into the AI stale-embedding delete and lookup (`ai.rs`, `ai/indexing/candidates.rs`) and the og:image orphan-blob sweep (`og_image_blobs` against the indexed `og_images.image_blob_hash`). A plan test (`stale_row_checks_look_visits_up_by_primary_key`) fails if the doctor checks go back to a materialized list.

Measured at 14.4M visits with one derived row per visit plus 1,000 stale ones, and a trail member for every eighth visit. Both query shapes ran in the same process against the same database (median of 3; "SQLite memory" is SQLite's own peak above the start of the call, from `sqlite3_memory_highwater`):

| stale-row count                           | time before | time after | SQLite memory before | after |
| ----------------------------------------- | ----------- | ---------- | -------------------- | ----- |
| `visit_derived_facts` (14.4M rows)        | 2.25 s      | 2.45 s     | 321 MiB              | 0     |
| `search_trail_members` (1.8M rows)        | 1.08 s      | 0.47 s     | 166 MiB              | 0     |
| whole `doctor` report, after (all checks) |             | 3.4 s      |                      |       |

The time for a table with one row per visit stays about the same: both shapes still read every derived row and touch every visit once. What changes is memory: the old shape held a temporary index whose size grows with the archive (321 MiB at 14.4M, on an 8 GB target), the new one holds nothing. On tables much smaller than the visit table the new shape is also faster, because it no longer reads all 14.4M visits to build the list.

Left alone: the "broken visibility" check (`visits.reverted_at IS NOT NULL` joined to `runs`) scans the whole visits table, 0.4 s at 14.4M. No index covers hidden visits; a partial index `WHERE reverted_at IS NOT NULL` would make it near-free, but needs an archive migration. `reverted_by_run_id NOT IN (SELECT id FROM runs)` in repair stays as it is: `runs` has one row per backup.

### 8. Insights "Frequent searches" counted all-time totals

The card summed `memberCount` of the query families that overlapped the range. A family's member count covers its whole history, so a query could show 100 next to a "Searches" figure of 74 for the same 30 days. Search events carry no time of their own, so the only range-scoped way to count them was to join each one to `archive.visits`, as `get_search_queries` and `get_top_search_concepts` still do.

Fix: intelligence migration 9 stores each search event's `visit_time_ms` and adds `idx_search_events_kind_time (query_kind, visit_time_ms)`; the rebuild writes the time with every new event. The new read `get_frequent_searches` counts keyword search events inside the range, grouped by normalized query across engines, and the card uses it. The plan seeks the range in that index, then groups the matches in a temporary B-tree, so the work grows with the searches in the range, not with the archive. A plan test (`the_range_read_walks_the_kind_and_time_index`) holds that.

At 14.4M visits with 1.8M search events (median of 5):

| `get_frequent_searches`, top 10                | time   | SQLite memory |
| ---------------------------------------------- | ------ | ------------- |
| 7 days                                         | 2.5 ms | 1.2 MiB       |
| 30 days                                        | 9.6 ms | 4.9 MiB       |
| 90 days                                        | 30 ms  | 15 MiB        |
| 365 days                                       | 145 ms | 59 MiB        |
| same count joined to `archive.visits`, 30 days | 440 ms |               |
| same count joined to `archive.visits`, 1 year  | 559 ms |               |

The joined shape scans every search event whatever the range. The site page's "Searched on this site" runs the same count for one search engine through `idx_search_events_engine_time (query_kind, search_engine, visit_time_ms, generation)`, so it reads that engine's searches in the range only. A covering index that also holds `normalized_query` and `raw_query` cut the one-year read to 95 ms in a trial; not worth a second copy of every query.

One-time cost: the migration fills in the time for existing events with one `UPDATE` against the archive, then builds the index. At 14.4M visits that took 2.2 s with the archive in the page cache and 9.4 s on the first run after the archive was generated (cold cache). Intelligence migrations run lazily, on the first intelligence read after an upgrade (not behind the "Upgrading your archive" screen), off the UI thread; that read and any that start beside it wait for it. Because several reads start at once, migrations now run under a process-wide lock. Without it every concurrent read would try to apply the pending migration, and the late ones could fail on SQLite's 5 s busy timeout or on the duplicate version row.

### 9. `export_history` held the whole archive in memory

Settings exports the whole archive. `export_history` walked History in 1,000-row pages, but collected every row into one `Vec`, rendered all of them into one `String` (JSON Lines built a second `Vec<String>` first) and only then wrote the file. Peak heap grew by about 1.2 GB per million visits for JSON Lines and 0.9 GB for HTML, so a 14.4M archive needed roughly 13 to 17 GB on an 8 GB machine.

Fix: each page is written to the file as it arrives and then dropped. The file is still written atomically: `durable_io::atomic_durable_write_with` streams into a buffered same-directory temp, then does the usual `F_FULLFSYNC` → rename → directory fsync; if the walk fails (archive locked, disk full) the temp is deleted and no partial export appears. The bytes are the same as before (a test pins every format). Temps left by an export that was killed are swept from the exports folder once they are an hour old; a fresh one may belong to an export still running.

Peak Rust heap above the start of the call ("heap", from the benchmark's counting allocator), whole archive, one run each on a shared machine:

| `export_history`, 1M visits | time before | time after | heap before | heap after | file    |
| --------------------------- | ----------- | ---------- | ----------- | ---------- | ------- |
| JSON Lines                  | 16 to 27 s  | 17 to 28 s | 1,201 MiB   | 0.6 MiB    | 325 MiB |
| HTML                        | 15 to 16 s  | 18 s       | 891 MiB     | 0.6 MiB    | 176 MiB |

Time did not change in any way that rises above the noise on this machine (other jobs were running; the same build varied by 10 s between runs). It is the page walk: about 5 ms per 1,000-row page at 14.4M with a warm cache, and much more when the archive is not in the OS cache, because each row looks up its URL in a table far larger than SQLite's page cache.

At 14.4M visits the whole-archive JSON Lines export finished: 14.4M rows, a 4.7 GB file, peak Rust heap +0.6 MiB, SQLite +78 MiB (its page caches), resident size about 92 MiB throughout. It took 66 minutes on a freshly generated archive that was not in the OS cache, with other jobs on the machine; warm, a page costs about 5 ms, which puts the walk alone near 75 s. The old code was stopped after 21 minutes with about a million rows collected and 1.2 GB resident and growing. In an earlier attempt the disk filled up after 2.3 GB: the export failed with "No space left on device", its temp was deleted and no partial file was left.

The follow-up export path (2026-10-04) opens the archive once for the whole request. The old cursor already sought `(visit_time_ms, id)` through the visible-time index; OFFSET was always zero. Reopening every 1,000 rows discarded SQLite's page cache and mapping and repeated archive/search initialization and advanced-filter table setup. Encrypted archives also repeated SQLCipher key derivation. Ordinary time-sorted exports now stream one statement, using the shared filters, ordering and row hydration; search exports retain the shared keyset readers on that same connection, including existing grouped/regex/fuzzy behavior. The unchanged format renderer writes through a fixed 256 KiB buffer. SQLite keeps its fixed cache and export-only temporary B-trees spill to disk instead of consuming archive-sized memory.

Re-measured on this checkout with the same export-only release benchmark. The baseline is the page-streaming implementation at `5c7d108c` (its benchmark binary was preserved before editing); the follow-up uses one connection. Plaintext, whole-archive JSON Lines, one run each:

| Visits | Time before            | Time after | Peak Rust heap before → after | Peak SQLite before → after | Artifact  |
| ------ | ---------------------- | ---------- | ----------------------------- | -------------------------- | --------- |
| 1M     | 20.17 s                | 3.22 s     | +0.6 → +0.3 MiB               | +0.4 → +0.4 MiB            | 325 MiB   |
| 14.4M  | 3,090.59 s (51.51 min) | 40.17 s    | +0.7 → +0.3 MiB               | +77.8 → +77.8 MiB          | 4,744 MiB |

The 14.4M export wrote all 14,400,000 rows, about 77 times faster in this run, with the same artifact size. Memory figures are per-call allocation peaks above the starting point, from the benchmark's Rust allocator and SQLite counters; they do not include mapped-file residency or OS file cache. The sandbox denied `/usr/bin/time -l`'s system-counter query, so no new process RSS figure is reported. A previous follow-up run at 1M was 3.04 s. Timings exclude fixture seeding/compilation and include rendering, buffered writes, durable publication and output removal. The 14.4M baseline followed fixture generation (499.8 s to seed, 80.0 s to project), with builds/tests also running during part of the export; the follow-up reused that archive after those jobs finished. OS cache and competing work were not controlled, so these are observed end-to-end results, separate from the historical 66-minute run above.

Reproduce from the checkout with `export CARGO_TARGET_DIR=$PWD/.cargo-target`, then `PATHKEEP_ARCHIVE_BENCH=1 PATHKEEP_ARCHIVE_BENCH_VISITS=14400000 PATHKEEP_ARCHIVE_BENCH_DIR=$PWD/.export-bench PATHKEEP_ARCHIVE_BENCH_SECTIONS=export PATHKEEP_ARCHIVE_BENCH_EXPORT_FORMATS=jsonl cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --test archive_scale_bench --release archive_scale_bench -- --nocapture` (use `1000000` for 1M). Archives were kept below the disk budget and deleted after both measurements.

**Encrypted, 14.4M visits** (2026-10-04, after the change; `PATHKEEP_ARCHIVE_BENCH_SECTIONS=encrypted-export`, which seeds a separate SQLCipher archive with the same rows): JSON Lines took 113.8 s for the same 14,400,000 rows and 4,744 MiB, peak Rust heap +0.3 MiB, SQLite +77.8 MiB. A 1,000-row page cost 52–61 ms. The load average was 19 during the run (another agent was building and running E2E), so the gap to the plaintext 40 s is partly load, not only decryption; the two runs were not taken side by side. No before figure exists for encrypted archives: the old code re-derived the key every 1,000 rows, so it was slower than its plaintext 51 minutes. On the 4-core target machine expect several minutes for a full encrypted export, which is why Settings shows progress and a Stop button.

Optional `exportId` enables polling through `get_export_progress` and cancellation through `cancel_export`, on Tauri and the dev bridge. Totals come from recent successful runs' cached stats, never a pre-export count: they estimate the archive, also when the query is filtered. Progress counts rendered rows/bytes, with updates every 128 rows or 64 KiB. Cancellation checks every row and every 64 KiB write chunk; SQLite's progress handler checks every 1,000 VM operations, including queries that have not yielded a row. The cancellation/publication barrier shares the job lock: accepted cancellation removes the temp and returns `export-cancelled`; after state becomes `finishing`, cancellation returns false. Flush, durable sync and atomic publication then lead to `done` or `failed`. There are at most 64 tracked jobs; finished entries expire after five minutes or are evicted at capacity. Different ids own separate counters/stop flags and unique artifact names.

Regression coverage compares exported bytes with the previous independent-page walk for every format, both sort directions, filters, keyword/regex/grouped recall and empty results. It also exercises cancellation while writing and inside a large SQL batch, large-field chunk checks, failed setup/query/publication state, concurrent ids, registry expiry/capacity, and allocator-measured heap bounds at two archive sizes. The reproducible export-only benchmark uses `PATHKEEP_ARCHIVE_BENCH_SECTIONS=export PATHKEEP_ARCHIVE_BENCH_EXPORT_FORMATS=jsonl`; generated archives and output files are removed after measurement.

### 10. Undoing an import rebuilt the whole search index

Release build on Apple Silicon; five alternating revert/restore calls in a warm process on the same plaintext archive: 1,000,000 visits, 250,000 URL rows, a 10,000-visit batch touching 9,724 URLs. Before uses the full rebuild; after seeks through at most 1,000 batch visits per statement and refreshes their URLs, keeping every chunk and both FTS mirrors in one search transaction after the canonical commit. The query plan uses `idx_visits_import_batch_id (import_batch_id=? AND rowid>?)` and URL primary-key lookups. Existing enrichment, notes and tags survive; visibility still comes from canonical visits. Existing archive-total counts and batch review remain in these end-to-end timings.

| Operation              | Before median / max    | After median / max |
| ---------------------- | ---------------------- | ------------------ |
| `revert_import_batch`  | 2,731.70 / 2,858.21 ms | 740.43 / 843.83 ms |
| `restore_import_batch` | 2,722.40 / 2,745.52 ms | 732.25 / 860.27 ms |

Reproduce with `export CARGO_TARGET_DIR=$PWD/.cargo-target`, then `PATHKEEP_IMPORT_BATCH_BENCH=1 PATHKEEP_ARCHIVE_BENCH_VISITS=1000000 PATHKEEP_ARCHIVE_BENCH_DIR=$PWD/.batch-revert-bench cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker --test archive_scale_bench --release import_batch_visibility_scale_bench -- --nocapture`. The synthetic archive was created inside the checkout and deleted after both measurements. These numbers do not measure the 14.4M target or encrypted archives.

### 11. Insights read whole generations for every date range

Every range read went through the generation views (`domain_daily_rollups`, `sessions`, `search_trails`, …). Turso plans those as a scan of the view and a search of the rows table on `generation` alone, so a 30-day card read every row the last rebuild wrote. Compare sets were worked out from every trail member in range on every read, habits read every visit, and the rhythm map counted visits by hour.

Fix: range reads query the `_rows` tables `INDEXED BY` an index that starts with the range column (`idx_domain_daily_range`, `idx_daily_summary_range`, `idx_sessions_range`, `idx_refind_pages_range`, `idx_search_trails_landing`, `idx_domain_daily_site`) and check the generation per row. Compare sets are found during the build and stored (`compare_sets_rows`). Habits take a site's last visit from the daily rollups (`last_visit_ms`), and the rhythm map adds up the hour counts each daily summary row now keeps (`hours`). A plan test (`vault-core/tests/intelligence_read_plans.rs`) fails if a range read scans a large table or searches one by generation alone.

`turso_intelligence::encrypted_intelligence_at_scale`, Modal VM (4 vCPU, 8 GiB), release, encrypted, 14.4M visits, one profile, 2026-10-08. Each figure is the second of two calls in the same process; the first was within 15%.

| 14.4M visits                       | 30 days before | 30 days after | 1 year after |
| ---------------------------------- | -------------- | ------------- | ------------ |
| primary overview (the first batch) | 581 s          | 3.3 s         | 37.8 s       |
| secondary overview                 | 350 s          | 4.3 s         | 55.0 s       |
| `get_digest_summary`               | 122 s          | 0.32 s        | 3.1 s        |
| `get_top_sites`                    | 55 s           | 1.27 s        | 16.3 s       |
| `get_browsing_rhythm`              | 31 s           | 0.9 ms        | 1.3 ms       |
| `get_breadth_index`                | 58 s           | 1.15 s        | 15.1 s       |
| `get_habit_patterns`               | 119 s          | 1.38 s        | 18.4 s       |
| `get_multi_browser_diff`           | 147 s          | 2.38 s        | 31.7 s       |
| `get_path_flows`                   | 71 s           | 0.76 s        | 9.4 s        |
| `get_compare_sets`                 | 68 s           | 0.5 ms        | 2.6 ms       |
| `get_observed_interactions`        | 61 ms          | 0.2 ms        | 0.1 ms       |
| every other card read              | under 19 ms    | under 11 ms   | under 12 ms  |

All time opens from the overview snapshot the rebuild writes: 1.5 s for each batch. A card read the snapshot does not hold is still a full scan: changing the top-sites order or asking for its next page takes 74 s for all time.

Full rebuild, same run: 5,721 s before, 5,955 s after; peak RSS 1.96 → 2.14 GB.

| stage               | before  | after   |                                                                                        |
| ------------------- | ------- | ------- | -------------------------------------------------------------------------------------- |
| visit-derived facts | 1,879 s | 1,872 s |                                                                                        |
| daily rollups       | 241 s   | 1,293 s | the new site and range indexes; the site index is written in site order, not day order |
| sessions and trails | 376 s   | 402 s   |                                                                                        |
| aggregates          | 1,259 s | 1,541 s | compare sets are now found here                                                        |
| overview snapshots  | 1,964 s | 844 s   | the all-time snapshot, built with the faster reads                                     |

The benchmark's data is a worst case for these reads: its visits walk 20,000 sites in turn, 8,640 a day, so nearly every visit starts a new site-day row (about 8,600 a day, 14.4M in all). Real browsing comes back to the same sites, so it has far fewer site-day rows per visit; every read above that grows with the range (top sites, breadth, habits, browsers) reads those rows, and so does the slower daily-rollup stage. Top sites, breadth, habits and browsers also read the same rows once each. Both are open below.

**After the follow-ups (2026-10-08):** common paths for a range read stored session paths, both batches are read for every range, the cards that rank or count sites share one read of a range's site-day rows, and the archive change probe behind the all-time snapshot no longer reads every visit. On the real-data copy (386,891 visits, 24 profiles, release build on Apple Silicon), second call:

| real data                             | before         | after      |
| ------------------------------------- | -------------- | ---------- |
| full Insights rebuild                 | 145 s          | 40 s       |
| all time, first / second batch        | 287 / 317 ms   | 4 / 3 ms   |
| one year, first / second batch        | 348 ms / 2.0 s | 67 / 43 ms |
| one year, common paths                | 1.45 s         | 22 ms      |
| top sites, any order or page          | 28–61 ms       | 1 ms       |
| all time, common paths (all profiles) | 918 ms         | 151 ms     |

Every other card read is under 50 ms. The real data has about 19 visits per site-day row (19,457 rows for 371,347 visits that were not searches); the benchmark has one.

The same follow-ups on the 14.4M benchmark (same VM; its one-site-day-per-visit shape is the worst case for the shared read):

| 14.4M visits, after the follow-ups                   | first call   | again         |
| ---------------------------------------------------- | ------------ | ------------- |
| all time, first batch                                | 0.11 s       | 0.11 s        |
| one year, first batch                                | 39.1 s       | 1.6 s         |
| one year, second batch                               | 1.4 s        | 0.9 s         |
| one year, common paths                               | 0.84 s       | 0.82 s        |
| 30 days, first / second batch                        | 3.3 / 0.19 s | 0.41 / 0.14 s |
| all time, KPI row read on its own (not the snapshot) | 90 s         | 0.95 s        |

Full rebuild 5,795 s (was 5,955): sessions and trails 476 s (was 402, the path segments), aggregates 1,592 s, the overview snapshot warm 207 s (was 844). The first one-year batch is still the cost of one walk over the year's site-day rows, now done once and kept: 3.15M rows on the benchmark, each brought into Rust, which costs more per row than the SQL groupings it replaced. On the real data the same batch takes 74 ms. Not measured in this run: the export and the append, the client lost the sandbox when the machine slept.

**An append updated in place (2026-10-09, ④):** the benchmark in its realistic shape (`PATHKEEP_BENCH_SHAPE=habits`), same VM, appending 10,000 visits after a full build:

| stage                          | time  |                                                                             |
| ------------------------------ | ----- | --------------------------------------------------------------------------- |
| facts, rollups, sessions       | 7 s   |                                                                             |
| refind pages                   | 270 s | 5,378 pages, each read again from all its visits; the most visited dominate |
| compare sets                   | 6 s   | the trails walked from the one left open                                    |
| paths, families, the one write | < 1 s |                                                                             |
| all-time snapshot warm         | 85 s  | the whole all-time overview again                                           |
| **total**                      | 376 s | was 13.7 min (aggregates rebuilt from every visit)                          |

The full build in the same run took 2,385 s; the process peaked at 474 MB in both. Next: merge a page's new visits into its stored refind row instead of reading its history again, and warm the snapshot from what changed.

**Every Insights read, 2026-10-09** (`35e35471`, same benchmark and shape, Modal VM on an AMD EPYC 9554, each read in a new process, first call / second call). The `habits` shape had no searches in this run, so the search cards read empty tables; see below.

| 14.4M visits, ms                    | 30 days     | 1 year      | all time      |
| ----------------------------------- | ----------- | ----------- | ------------- |
| primary overview (the first batch)  | 189 / 95    | 1,402 / 200 | 12 / 12       |
| secondary overview                  | 140 / 114   | 967 / 844   | 12 / 5        |
| `get_digest_summary` on its own     | 69 / 69     | 173 / 172   | 2,892 / 138   |
| `get_interrupted_habits` on its own | 2,748 / 0.8 | 2,777 / 0.8 | 0.3 / 0.4     |
| `get_path_flows` on its own         | 91 / 85     | 795 / 785   | 1,012 / 1,007 |
| `get_discovery_trend` (per day)     | 11 / 11     | 11 / 11     | 18 / 18       |
| `get_multi_browser_diff`            | 5 / 5       | 8 / 9       | 11 / 14       |
| every other card read               | under 2     | under 3     | under 5       |

All time is answered from the snapshot. Opening a range costs its two batches; the cards read on their own only when they change their own options. Three reads stand out: common paths for a long range (0.8–1.0 s every time, the stored session paths of every day in it), the first interrupted-habits read in a process (2.7 s, then under 1 ms), and the all-time digest when it is read without the snapshot (2.9 s the first time).

An export between two reads (`export-between-reads`): the secondary overview took 110 ms before, 1,992 ms just after the export (the export pushed the Insights pages out of the cache) and 116 ms again after that; the export itself took 489 s for 5.0 GB and the process stayed at 722 MB resident afterwards (peak 957 MB).

**The three questions before merge (2026-10-09).** Asked of the in-place append: how long an append takes, how long new categories take, and whether the full build got slower.

- Full build: not slower. Run back to back on one VM (Intel Xeon 8375C, CPU probe 1.65–1.66 s throughout), `d182481e` took 2,777 s and `35e35471` 2,823 s. The earlier 2,385 s against 2,850 s came from two different hosts (an EPYC 9554 runs the same build in about 2,360 s).
- Append of 10,000 visits: 126 s on the EPYC, of which 94 s were spent writing path flows: before each of the 4,812 flows the append asked whether its row existed, and Turso planned that through `idx_path_flows_profile_rank` on profile and step count, a read of every flow of that length (19 ms a flow). It is one upsert now (`RETURNING` the count), and `append_plans.rs` counts step count and the other kind columns as columns that never find one row.
- New categories (an override that moves `site1.test`): 62 s on the EPYC, 72 s on the Xeon. 61 s of it sorts all 2.88M pages again, even for one site's override; the rest warms the all-time snapshot.

**Searches at scale.** None of these runs had a single search: every seeded page was `site{n}.test/page/{id}`, so search events, trails, query families and Chinese segmentation had never run at 14.4M. The `habits` shape now makes every 25th visit a Google search (4%, the share on the real-data copy) from `test_support::search_history::synthetic_searches`: repeats, rewordings a minute later, new searches of one to six words drawn by Zipf's law from 50,000 words, and 5% in Chinese; the two visits after a search are opened from it. 580,001 searches, 307,694 different ones.

Query families alone, 576,000 of those searches in one profile (`intelligence/tests/family_scale.rs`, release, in memory):

| rule                                     | build   | families | largest family | append's re-index |
| ---------------------------------------- | ------- | -------- | -------------- | ----------------- |
| v9: like any wording of a family         | 2,780 s | 15,066   | 164,788        | 1.2 s             |
| v10, first search, containment counts    | 4.2 s   | 131,736  | 6,683          | 0.7 s             |
| v10, first search, half and at least two | 6.3 s   | 185,131  | 3,589          | 1.0 s             |

v9's cost grew with every batch (13 s for the first 48,000 searches, 469 s for the last); v10's stays flat. The largest family under the shipped rule is begun by the generator's two most common words, which it draws independently (`deterministic-intelligence.md` §5.4).

The whole benchmark with searches (structural v10 with containment, the rule before the last row; same EPYC; `PATHKEEP_BENCH_SHAPE=habits`):

| 14.4M visits, 580,001 searches | time    | without searches |                                                                                                             |
| ------------------------------ | ------- | ---------------- | ----------------------------------------------------------------------------------------------------------- |
| full build                     | 2,915 s | 2,356 s          | sessions and trails 698 s (359), refind and compare sets 657 s (582), families 33 s; peak 611 MB            |
| append of 10,000 visits        | 65 s    | 126 s            | facts to sessions 6.5 s, refind 25.6 s (5,181 pages), families 1.3 s, write 3.0 s, all-time snapshot 28.4 s |
| new categories                 | 90 s    | 62 s             | 3.19M pages sorted again 66.8 s, snapshot 21 s                                                              |

The reads that searches made slow (ms, first / second call):

| 14.4M visits with searches | 30 days       | 1 year        | all time      |
| -------------------------- | ------------- | ------------- | ------------- |
| primary overview           | 2,836 / 2,729 | 4,336 / 3,152 | 12 / 12       |
| secondary overview         | 557 / 539     | 1,980 / 1,858 | 12 / 7        |
| `get_top_search_concepts`  | 2,534 / 2,530 | 2,823 / 2,811 | 3,989 / 3,978 |
| `get_search_trails`        | 995 / 462     | 455 / 454     | 447 / 447     |
| `get_friction_signals`     | 388 / 398     | 396 / 396     | 430 / 427     |
| `get_query_families`       | 89 / 90       | 89 / 89       | 92 / 94       |
| `get_path_flows`           | 115 / 101     | 911 / 894     | 1,851 / 1,827 |
| `get_search_effectiveness` | 21 / 18       | 275 / 270     | 1,412 / 1,379 |
| `get_frequent_searches`    | 21 / 13       | 225 / 182     | 1,159 / 987   |
| `get_stable_sources`       | 12 / 10       | 170 / 160     | 804 / 798     |

What you search for, search trails, friction and query families cost about the same whatever the range, so they read more than the range.

**After keeping the search reads in their range (2026-10-09).** What you search for walks the range's searches by time and joins each one's terms by key, instead of reading the `search_events` and `search_event_terms` views (every row of the build). The query family and trail totals count the rows through their time index; trails are counted and listed by when they began. Friction reads the trails that may be friction from a partial index by time and sorts them; the hardest topics read the families searched in the range and sort them by size. Walked in rank order, both read past a short range until they found a page of their own. `intelligence_read_plans.rs` now runs every Insights read for 30 days and all time, across profiles and in one, and fails when a statement scans a large table or searches one only by generation, profile and kind columns; pages by refind score are the one exception (`store-schema.md`).

Measured with the last 3.6M visits of the same benchmark (`PATHKEEP_BENCH_FIRST_VISIT=10800001`: the same days and density at the end of the data, so 30 days and a year read the rows they would among 14.4M), on the Xeon VM, about 30% slower than the EPYC above (ms, first / second call):

| at the end of 14.4M visits with searches | 30 days before | 30 days after | 1 year before | 1 year after  |
| ---------------------------------------- | -------------- | ------------- | ------------- | ------------- |
| primary overview                         | 2,836 / 2,729  | 287 / 160     | 4,336 / 3,152 | 1,677 / 865   |
| secondary overview                       | 557 / 539      | 225 / 222     | 1,980 / 1,858 | 2,023 / 1,770 |
| `get_top_search_concepts`                | 2,534 / 2,530  | 36 / 35       | 2,823 / 2,811 | 550 / 510     |
| `get_search_trails`                      | 995 / 462      | 3 / 4         | 455 / 454     | 31 / 30       |
| `get_friction_signals`                   | 388 / 398      | 0.3 / 0.3     | 396 / 396     | 0.3 / 0.1     |
| `get_query_families`                     | 89 / 90        | 16 / 16       | 89 / 89       | 152 / 97      |
| `get_search_effectiveness`               | 21 / 18        | 43 / 45       | 275 / 270     | 563 / 526     |

The "before" columns are the EPYC run above, so the gains are if anything understated. 30 days is back to where it was without searches. What a year still costs grows with the rows in it: common paths 1.1 s (it was 0.9 s before searches), What you search for and the hardest topics about 0.5 s, the stable sources and frequent searches about 0.2 s, and the first interrupted-habits read in a process 0.84 s; STATUS item 14. The export between reads with searches: 534 ms before, 4.9 s just after, 532 ms again; 5.3 GB in 509 s.

### 12. A large import wrote each chunk twice, and opened a temporary file per row

A Takeout import writes in chunks of 10,000 records. Each chunk was two archive transactions: its URLs, then its visits (`archive/ingest/consumer.rs`). Each rebuilt the search document of every page it touched, so a page's document was written twice per chunk and queued for the index twice. The first rebuild had an empty title, because a URL's title only counts once it has a visible visit. Each visit looked up the URL row its chunk had just written. A URL whose page already existed rewrote the page row to read its id (`ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id`).

Sampling the archive writer (`scripts/profile-native.py`) found the larger cost. Every `INSERT … RETURNING` makes Turso buffer the returned rows in an ephemeral table, and with `temp_store=FILE` each ephemeral table is a new temporary directory and an encrypted temporary file (`op_open_ephemeral`; `vault_crypto::temp_io`). The URL upsert and the visit insert both returned their ids, so every record created two temporary files.

Fix:

- **One transaction per chunk.** A transaction holds at most one batch of each kind: a Takeout chunk's URLs and visits, then each page's document rebuilt once, after all of it is written, with one outbox entry. This is the write model `turso-migration-design.md` §5 already describes.
- **Ids are passed along.** The chunk's URL ids go to its visits, which no longer look them up.
- **Pages are looked up first.** A page is read before it is inserted.
- **Plain statements instead of `RETURNING`.** Inserts read the new row's id from `Session::insert` (the connection's last rowid). The URL upsert is a plain `INSERT` for a new row, an `UPDATE` by id for a newer observation, and nothing at all when an older observation would not raise the counts.

`archive_scale_bench::turso_same_visit` with `PATHKEEP_BENCH_SAME_VISIT_ONLY=no-overlap`, Modal (release, encrypted), 14.4M visits of one browser, 1,000,000 Takeout records that overlap no source. Old and new ran at the same time on separate sandboxes:

| Write path                | 1M records  | Per record | 14.4M records |
| ------------------------- | ----------- | ---------- | ------------- |
| Before                    | 948 / 933 s | 0.94 ms    | about 3.7 h   |
| One transaction per chunk | 699 s       | 0.70 ms    | about 2.8 h   |
| …and no `RETURNING`       | 432 s       | 0.43 ms    | about 1.7 h   |

Sampled at 200,000 records, the writer was busy in every sample before and after. Before: URL upsert 32%, visit insert 20%, commit 16%, search documents 19%. With one transaction per chunk: URL upsert 27%, visit insert 26%, commit 16%, search documents 17%; `op_open_ephemeral` alone was a fifth of both inserts. The sampled share understated it: removing it saved 38% of the time. After both, the 200,000 records took 72 s under the sampler (127 s before the second change, 192 s before the first); commit is a fifth of the writer's samples, and nearly all the rest is the engine executing the inserts, mostly index B-tree inserts.

A user's own write during an import (a star, a note) waits for the transaction in progress. A chunk's one transaction now takes about 4.3 s, where each of the two took about 4.7 s, so that wait is no longer than it was.

`archive::ingest::import_equivalence` pins the result. It runs a 21,000-record export with Chrome history and My Activity (three chunks) over a browser backup, and prints per-table digests of the archive. The digests were identical before and after each change, as were an interrupted import run again and an undo followed by a restore.

Not done:

- **Rebuilding search documents once at the end.** During a long import its pages would not be searchable, which search must never allow.
- **Dropping secondary indexes during a large import.** History, search and Insights need them while the app is in use, and rebuilding them over 14.4M visits is itself a long job.

Not tried, since the target was met:

- **Multi-row `INSERT`s.** Without `RETURNING`, a multi-row insert cannot tell which of its rows were new.

Left as it is:

- **The checkpoint after each batch.** It did not appear in the samples: Turso checkpoints large commits itself.

An interrupted import run again re-reads the records it already wrote (`WORK-TAKEOUT-RESUME`; it carries on in the same batch). Those records are duplicates, but each still rebuilt its page's search document and queued it for the index, because every URL of a chunk marked its page. Now only a URL row that is new or takes a newer observation marks its page. Importing the same 1,000,000 records a second time (`PATHKEEP_BENCH_SAME_VISIT_ONLY=again`, after a first import of 508–536 s on the same machines) went from 219 s to 62 s: 62 µs a record, against about 500 µs for a new one.

What remains is the engine's own work per row: the B-tree inserts of `urls` (5 indexes) and `visits` (11, two of them keyed by hashes, so random), and the commit that writes the pages they touched.

### 13. "Pages reopened" counted pages that were not opened in the range

The KPI counted every refind page whose first and last visit spanned the range (`refind_pages_rows`, by `last_seen_ms`), opened in the range or not: a day with no visits said 58. It now counts the refind pages opened on a day of the range, the pages the day page lists (`deterministic-intelligence.md` §5.7).

Which days a page was opened on was worked out by the build and then thrown away. A table with one row per page and day was tried first. At 14.4M visits in the `habits` shape, the digest took 3.3 s for a custom year and 10.8 s for all time. Nearly every visit there is its own page-day, so a year is about 3M rows to read, twice (the range and the one before it). That design was dropped. The table now kept is inverted: one row per profile and day, holding that day's pages as a blob of sorted 64-bit keys (`refind_day_pages_rows`, `intelligence::refind_days`). A range reads one row per day and unions the keys.

`turso_intelligence::encrypted_intelligence_at_scale`, Modal, 14.4M visits; old and new in the `cycle` shape ran at the same time:

| 14.4M visits                          | old (`cycle`) | new (`cycle`) | one row per page-day (`habits`) | new (`habits`) |
| ------------------------------------- | ------------- | ------------- | ------------------------------- | -------------- |
| full build                            | 5,612 s       | 5,704 s       | 3,788 s                         | 2,662 s        |
| append of 10,000 visits               | 182 s         | 182 s         |                                 |                |
| digest, 30 days                       | 162 ms        | 43 ms         | 223 ms                          | 24 ms          |
| digest, a custom year                 | 1,107 ms      | 418 ms        | 3,320 ms                        | 141 ms         |
| first batch, a custom year            | 34.0 s        | 32.5 s        | 5.6 s                           | 2.0 s          |
| digest, all time (read directly)      | 77.2 s        | 76.0 s        | 10.8 s                          | 3.0 s          |
| first batch, all time (from snapshot) | 88 ms         | 86 ms         | 14 ms                           | 12 ms          |

The new count is faster than the span it replaced, because it reads a row per day instead of a row per refind page. The all-time digest read directly is dominated by reading every site-day row (§11); Insights opens all time from its snapshot. The full build collects day → keys in memory until the refind stage ends (8 bytes per page and day); peak memory did not change measurably in the build (478 MB against 476 MB in the `cycle` shape). The data grew from 22.19 to 22.44 GB (the day rows and the page keys). The append's peak was 708 MB against 606 MB; it adds one new day of about 10,000 keys, which does not account for the difference, so it is noted as measured.

### 14. Ephemeral tables opened a temporary file on every run

Turso 0.8.1 gives every ephemeral table a new temporary file: a directory and a file in `$TMPDIR`, encrypted by `vault_crypto::temp_io`, with its first page written, then removed (`op_open_ephemeral`). PathKeep sets `temp_store=FILE` so large sorts do not grow memory, and Turso has no setting that keeps small ephemeral tables in memory while sorts spill. Ephemeral tables back `RETURNING`, a materialized `IN (SELECT …)`, any `IN (…)` list, and compound selects. Sorts and hash joins create a file only when they spill. One file costs about 150 µs on macOS (directory 71 µs, file 66 µs, measured on the user's machine) and 130 µs on Modal; it is also a disk write.

`vault_store` now records, when plan capture is on, each statement run whose program opens an ephemeral table (`Store::take_ephemeral_tables`). `vault-core/tests/ephemeral_tables.rs` drives a Takeout import, an Insights build and an append, the overviews for 30 days, a year and all time, a day page, History, and an undo and restore. It counted:

| Step                        | Temporary files before                                  | After |
| --------------------------- | ------------------------------------------------------- | ----- |
| undo and restore, 400 URLs  | 806, of which 800 were one `UPDATE … RETURNING` per URL | 6     |
| append                      | 432, of which 165 were one path-flow upsert per flow    | 267   |
| build                       | 267 (the overview reads that warm the snapshots)        | 267   |
| one range's two batches     | 44                                                      | 44    |
| all time, from the snapshot | 3                                                       | 3     |
| a day page                  | 16                                                      | 16    |
| a History search            | 2                                                       | 2     |
| an import (3,000 records)   | 3                                                       | 3     |

The loops that ran one per row are rewritten without `RETURNING`. Undo and restore read the visits that change, then change them. The append inserts a new flow with `INSERT OR IGNORE` (`Session::insert`) and updates a stored one by its key, naming the key's index so the planner cannot pick the rank index (§11). The import had done the same in §12. An undo of 1M URLs would otherwise have created a million temporary files. The test fails when a statement opens one more than 50 times in a step; its fixture makes a per-row statement run hundreds of times.

The reads are left as they are. Most Insights reads filter `generation IN (SELECT … FROM intelligence_generations)`, in 52 places and 16 views. That is 44 files, about 7 ms on macOS, each time a range is read live, and 3 when Insights opens on all time. Rewriting every read to bind its generations would not repay the change.

### 15. Importing the same records again wrote their evidence again

Source evidence (`archive/source-evidence.db`) had no key of its own: each row hung off the batch of the run that wrote it. An import that read records the archive already held (the same file imported again, an import carried on after a stop, a URL a backup scans again because its counts changed) wrote their evidence again. Readers merged by visit, so no number was wrong, but every re-import of a file grew the evidence by as much as the first import had written.

Fix: each evidence table has a `content_key` (source profile and the row's content, hashed) with a unique index, and the writer skips what is already there (`store-schema.md` §Source Evidence). The per-batch indexes, which nothing read, are gone, so the unique index does not add an index per table.

`takeout::tests::browser_history::reimport_evidence_bench` (debug build, this cloud machine, 2026-10-12): a Chrome History of 200,000 visits over 20,000 URLs, imported twice. "Before" ran the same code with every row given a key of its own, which writes every row as the old code did.

|              | First import                  | Same file again               |
| ------------ | ----------------------------- | ----------------------------- |
| Before       | 620,000 rows, 173.6 MB, 216 s | +620,000 rows, 347.4 MB, 71 s |
| Content keys | 620,000 rows, 173.6 MB, 223 s | +0 rows, 173.6 MB, 49 s       |

About 3.1 evidence rows per visit, so at 14.4M visits a re-import wrote about 45M rows and some 12 GB again. The first import costs the same within noise: the unique index replaces the batch index. Not measured on the 14.4M bench (`WORK-SCALE-BENCH-RERUN`).

## Other reads

| call                                          | 1M visits     | 14.4M visits  |
| --------------------------------------------- | ------------- | ------------- |
| `load_source_stats`, first call after a write | 21 to 28 ms   | 294 ms        |
| `load_source_stats`, cached                   | under 0.01 ms | under 0.01 ms |
| `get_url_detail`, the most-visited URL        | 1.1 ms        | 1.0 ms        |
| `preview_wipe_all_data`                       | 1.0 to 1.6 ms | 1.1 ms        |

Generating the 14.4M archive took 208 s for the rows and 47 s to project 3.6M search documents.

**Recent ranges as snapshots (2026-10-10, STATUS Insights ⑤).** Same benchmark in the `cycle` shape, where every visit goes to a different site on its day (8,640 site-day rows a day). Grouping the shared site-day read by site in SQL instead of in Rust took 18.7 s for a year against 19.9 s, so the read stays as it is. 1 year, 90, 30 and 7 days ending today, over every profile, are now snapshots like all time (`recent:<days>` in `store-schema.md`), warmed after each build and when the app opens on a new day: a preset year's first batch takes 5.7 ms and its second 3.4 ms. A custom year is read live. Its first batch takes 39.9 s: the KPI row reads the year's site-day rows and the year before it, about 20 s each. Read again, it takes 1.6 s. The same read takes 1.74 s in the `habits` shape and 67 ms on the real-data copy. A site-month rollup for archives this dense was weighed and not added (user, 2026-10-10): [research/2026-10-site-month-rollup.md](research/2026-10-site-month-rollup.md).

## Open risks

- ~~Keyword search over a word on more than 25,000 pages ranks only the most recently archived 25,000 (§6).~~ Gone with the Turso search, which ranks every matching page ([search.md](search.md)).
- `load_source_stats` recounts each profile after every backup (294 ms at 14.4M). It is cached until the archive file changes and runs off the UI thread.
- `keyring_status()` on every `app_snapshot` is unmeasured (see 4).
- ~~A whole-archive export at 14.4M is a long job, with no progress or cancel.~~ One connection for the whole walk: 40 s for 14.4M visits as plain JSON Lines, with progress and Stop in Settings → Storage (§9).
- Insights over long ranges (§11): all time and the recent ranges ending today open from snapshots. A custom range is read live, and in the benchmark's densest shape a custom year's first batch takes 39.9 s (1.74 s in the `habits` shape, 67 ms on real data). A site-month rollup would cut it to about 3 s. The user decided on 2026-10-10 not to add one, since no real archive comes near that density; if one ever shows a custom year over 3.3 s, that rollup is the fix ([research/2026-10-site-month-rollup.md](research/2026-10-site-month-rollup.md)).
- Numbers come from a machine several times faster than the target. The fixes change the shape of the work (one page instead of a sort over every visit, two key derivations instead of five), which holds on slower hardware, but absolute times there will be higher.
