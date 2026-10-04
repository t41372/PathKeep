# IPC read-path performance

What each command costs at archive sizes that matter, what was slow, what changed, and how to measure it again. Target: 14.4M visits on a 4-core, 8 GB machine, and the UI never waits on a command it did not ask for.

## How the numbers were taken

`src-tauri/crates/vault-worker/tests/archive_scale_bench.rs` builds a synthetic archive through the real schema (3 profiles, one URL per four visits, 20,000 domains, five years of visits), lets the real open path project it into the search database, then times each call. It is skipped unless asked for:

```sh
PATHKEEP_ARCHIVE_BENCH=1 PATHKEEP_ARCHIVE_BENCH_VISITS=14400000 \
  PATHKEEP_ARCHIVE_BENCH_DIR=/tmp/pathkeep-bench \
  cargo test --manifest-path src-tauri/Cargo.toml -p vault-worker \
  --test archive_scale_bench --release -- --nocapture
```

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

### 6. History search lists pages, not visits (`groupByUrl`)

Searching listed every matching visit, so one page visited thousands of times filled the list. Search now asks `query_history` with `groupByUrl: true`: one row per URL with its visit count (`vault-core/src/archive/history/grouped.rs`). Matching `url_id`s are aggregated through `idx_visits_visible_url_time` (count, newest visit), folded into pages with window functions, and only the page window is sorted. A plan test fails if the visit lookups stop using that index or scan `visits`.

Measured on the same 14.4M archive, `limit: 100`, the "before" column with the code before this change and the rest in a second run. Other builds were running on the machine during both runs, so the same unchanged query ("visits" rows) came out 973 ms in the first run and 1,341 ms in the second; compare within a run.

| 14.4M visits                  | visits, first run (before) | visits, second run | pages, second run |
| ----------------------------- | -------------------------- | ------------------ | ----------------- |
| "topic 42", first page        | 973 ms                     | 1,341 ms           | 811 ms            |
| "topic 42", with exact totals | 1,857 ms                   | 2,085 ms           | 1,656 ms          |
| "topic 42", second page       |                            |                    | 832 ms            |
| rare term, first page         | 50 ms                      | 74 ms              | 79 ms             |
| rare term, with exact totals  | 54 ms                      | 72 ms              | 158 ms            |
| "topic" (every page), first   |                            | 25.0 s             | 10.2 s            |
| "topic" (every page), totals  |                            | 68.0 s             | 24.9 s            |

"topic 42" is in one title in 500 (7,200 pages, about 29,000 visits); the rare term is one page with a handful of visits; "topic" is in every one of the 3.6M titles. Grouped totals run the match twice (once for the totals, once for the rows), which is why the rare term's exact-totals call doubles; the UI runs that call beside the list, not before it. A cursor page costs the same as the first page: the match and the aggregation run again, only the cut moves.

Regex search groups the same bounded window the visit list scans (the newest 50,000 visits inside the filters), so its counts cover that window. Semantic search already returned one row per page; its header shows pages only.

## Other reads

| call                                          | 1M visits     | 14.4M visits  |
| --------------------------------------------- | ------------- | ------------- |
| `load_source_stats`, first call after a write | 21 to 28 ms   | 294 ms        |
| `load_source_stats`, cached                   | under 0.01 ms | under 0.01 ms |
| `get_url_detail`, the most-visited URL        | 1.1 ms        | 1.0 ms        |
| `preview_wipe_all_data`                       | 1.0 to 1.6 ms | 1.1 ms        |

Generating the 14.4M archive took 208 s for the rows and 47 s to project 3.6M search documents.

## Open risks

- Keyword search over a common term is about 0.8 s for the first page of grouped results at 14.4M (1.7 s with totals), and a word in every page title takes 10 s (25 s with totals). It runs off the UI thread, but the list waits for it. Both FTS tables match and rank every document before a page is cut, then every matching `url_id` is aggregated. For "topic" the term-table match alone is 60 ms and aggregating all 14.4M visits by `url_id` is about 1 s (sqlite3 CLI on the same file), so most of the 10 s is ranking, the trigram match and folding 3.6M URLs; that split is not measured yet. Cutting the match by score before aggregating is the next thing to look at.
- `load_source_stats` recounts each profile after every backup (294 ms at 14.4M). It is cached until the archive file changes and runs off the UI thread.
- `keyring_status()` on every `app_snapshot` is unmeasured (see 4).
- Numbers come from a machine several times faster than the target. The fixes change the shape of the work (one page instead of a sort over every visit, two key derivations instead of five), which holds on slower hardware, but absolute times there will be higher.
