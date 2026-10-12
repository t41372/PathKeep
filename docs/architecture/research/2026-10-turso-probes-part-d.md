# Turso acceptance probes, part d: product-shaped memory (2026-10-05)

> Measured on a macOS arm64 host (18 logical CPUs) on 2026-10-05, for [ADR-012](../decisions/012-turso-storage-engine.md). Links to `src/`, `logs/` and `scripts/` point to the probe checkout, which is not kept in the repo.

# Turso 0.8.1 — product-shaped WAL memory

**Verdict: no—not comfortably within the proposed 3 GB process budget with Turso 0.8.1 native FTS and the tested public controls.** This conclusion no longer includes an oracle: all 41 reported measurement processes exclude verification structures, and all 38 search responses plus every backup/export/import are checked afterwards in separate processes. [Evidence audit](logs/part-d-audit.json).

Reads, streaming export and the one-time bulk import fit. **Backup is the limiting workload.** With one writer, a separate reader whose FTS cache is already warm, an 8-MiB cache per connection, 10k SQL statements and immediate mimalloc purging, peak/steady RSS is **3,082,338,304 / 3,082,256,384 bytes** for 10k batches. The 50k/100k cases peak at **3,382,607,872 / 3,383,427,072 bytes**. If “3 GB” instead means **3 GiB**, the 10k case uses **95.7%** of it, and the larger batches exceed it. These results do not prove that the application cannot run on an 8-GB machine; they do not provide comfortable headroom or an enforceable 3-GB database-process bound.

| Requested item                                              | Verdict against 3 GB RSS                    | Measured result with 8-MiB cache and immediate purge                                                                                                                                                                                                          | Evidence                                                                                                                                                        |
| ----------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Idle after open/history                                  | **PASS**                                    | **14.5 MiB peak/steady** with schema/index definitions loaded; **599.3 MiB peak/steady** after actually warming FTS                                                                                                                                           | [Idle](logs/part-d-idle-purged.jsonl), [code](src/d_search.rs)                                                                                                  |
| 2. Ranked first/next pages, cold/warm                       | **PASS**                                    | **600.0–605.8 MiB peak**, similar steady RSS; zero archive-sized eligibility array, heap ≤ **6,400 bytes**. All three queries/pages match the separate exhaustive oracle.                                                                                     | [Search](src/d_search.rs), [oracle result](logs/part-d-verify-search-all.jsonl)                                                                                 |
| 3. 1M backup updates, 10k/50k/100k transactions             | **FAIL**                                    | Single-writer, FTS-warm process: **2,939.5 / 3,225.9 / 3,226.7 MiB peak**, with steady RSS essentially the same. All updates/annotations and counts verify; memory exceeds the budget.                                                                        | [10k](logs/part-d-backup-single-purged-10000.jsonl), [50k](logs/part-d-backup-single-purged-50000.jsonl), [100k](logs/part-d-backup-single-purged-100000.jsonl) |
| 4. Full 14.4M streaming export                              | **PASS**                                    | **23.8 MiB peak/steady**, **30.600 s**, real JSONL output including flush/fsync; every row separately verified                                                                                                                                                | [Export](logs/part-d-export-purged.jsonl), [verification](logs/part-d-verify-export-purged.jsonl)                                                               |
| 5. Empty-archive 14.4M import                               | **PASS** with the measured bulk-load policy | **1,182.8 / 1,182.7 MiB peak/steady**, **387.544 s**, including final relational/FTS index builds. Default 64-MiB/100k-batch case: **2,387.5 / 1,921.8 MiB**, **180.822 s**.                                                                                  | [Tuned import](logs/part-d-import-purged.jsonl), [baseline](logs/part-d-import-baseline.jsonl), [verification](logs/part-d-verify-import-purged.jsonl)          |
| 6. Controls sufficient for a comfortable 3-GB process bound | **FAIL**                                    | Page-cache/sorter limits, request-local/inline eligibility, bounded transactions, one writer and supported allocator purging were tested. Native FTS's **64-MiB segment-writer budget is fixed**, and neither it nor `cache_size` caps total/live FTS memory. | [Settings implementation](src/d_runtime.rs), [source](SOURCE-part-d.md#fts-budgets), [all measurements](logs/part-d-summary.json)                               |

The detailed tables below pair every timing/RSS window with `uptime` load averages. Memory is in MiB there; the budget above is 3,000,000,000 bytes.

## Measurement method

All memory measurements use fresh, **single-owner WAL processes**, unencrypted databases, and the existing ten-field native FTS term index. No MVCC or experimental multiprocess WAL is enabled. This report measures the existing fixture's term index; it does not claim a memory budget for an additional full-size trigram index or arbitrary larger document content.

**No exhaustive oracle, expected-visit array, duplicate-detection bitset, full result sort, integrity check, or verification digest runs in a measured process.** The measured [part-d binary](src/part_d.rs) does not import the verification module. [part-d-verify](src/part_d_verify.rs) is a different executable and runs only after the measured process exits. The setup seed is a separate, unmeasured process. Development smoke runs are excluded from the reported windows.

Memory comes from macOS `MACH_TASK_BASIC_INFO`, sampled by a constant-space thread every **1 ms**. Each [window](src/d_metrics.rs) records current RSS before/at completion, the window's sampled peak, the kernel's lifetime process peak, and post-operation RSS after **250 ms with database handles still alive**. This last number is the operational definition of **steady RSS** here; it is not an assertion that an allocator will never release more memory after a longer idle interval. No explicit heap collection, allocator trim, process restart, or dependency patch is used to make a measured window smaller. Where a new kernel lifetime high-water mark appears, it is included in the window peak. Warm-window peaks below an earlier process maximum are sampled, while the process maximum remains an exact conservative budget check.

Every window records `uptime` at start and end. Individual backup requests/checkpoints also record load averages. CPU/latency measurements are from the same macOS arm64 host as parts A/C (**18 logical CPUs**); the host was not restricted to four cores, and no target-hardware latency is extrapolated. `cache_size` and allocator options are memory controls, not an OS-enforced RSS or CPU cap. Budget comparisons use **3,000,000,000 bytes**, not 3 GiB.

The baseline has **14,400,000 visits**, **3,600,000 stored page documents**, and **2,865,791 pages with visits**. The workload has the same synthetic URL/title lengths and field contents as the earlier fixture. An empty-archive import creates only visited pages, rather than fabricating unused page rows: its result is **14.4M visits / 2,865,791 page documents**.

## Settings and limits

| Profile in logs                     |    Page cache per connection | SQL statement rows for backup | Import transaction rows | Eligibility                         | Allocator                                                                          |
| ----------------------------------- | ---------------------------: | ----------------------------: | ----------------------: | ----------------------------------- | ---------------------------------------------------------------------------------- |
| `baseline`                          | 64 MiB (`cache_size=-65536`) |                        10,000 |                 100,000 | 57,600,016-byte request-local array | Stock mimalloc defaults                                                            |
| `bounded` (small-statement control) |   8 MiB (`cache_size=-8192`) |                         1,000 |                  10,000 | Projected from SQL rows; no array   | Stock mimalloc defaults                                                            |
| `cache8` (backup control)           |                        8 MiB |                        10,000 |                       — | Not used by backup                  | Stock mimalloc defaults                                                            |
| `inline64` (search control)         |                       64 MiB |                             — |                       — | Projected from SQL rows             | Stock mimalloc defaults                                                            |
| `purged`                            |                        8 MiB |                        10,000 |                  10,000 | Projected from SQL rows             | `MIMALLOC_PURGE_DELAY=0`, `MIMALLOC_PURGE_DECOMMITS=1`, set before process startup |

All profiles use `synchronous=FULL`, `cache_spill=ON`, `temp_store=FILE`, `wal_autocheckpoint=0`, and explicit PASSIVE checkpoints for writes. The actual PRAGMA readbacks and allocator environment are logged by [d_runtime.rs](src/d_runtime.rs). Backup uses **two SQL connections**, with the long writer on its own thread; search/export/import use one. The `single-purged` variant gives one connection exclusive ownership of all writes and makes the other query-only; its reader FTS cache is prewarmed. Other backup controls also execute interactive mutations on the second connection. A bounded producer/consumer handshake prevents the next backup batch from starting before queued interactive work and checkpointing finish.

The native FTS writer setting is **not tunable through the public release API**. `index_method/fts/mod.rs:133` fixes its per-segment writer arena budget at **64 MiB**; `:1991` passes that constant to Tantivy. The supported SQL WITH keys at `:613` are only `tokenizer`, `weights`, `min_gram`, and `max_gram`, and unknown keys are rejected. The arena budget is not a cap on all FTS memory. [Exact source](SOURCE-part-d.md#fts-budgets).

`cache_size` converts negative KiB to a pager page count (`translate/pragma.rs:1890–1942`). It also controls the external sorter's in-memory threshold (`vdbe/execute.rs:9394–9403,9427–9438`), relevant to building relational indexes during import. `temp_store=FILE` permits external-sort storage on disk. [Cache](SOURCE-part-d.md#cache-size), [sorter](SOURCE-part-d.md#sorter-memory).

Turso itself installs **mimalloc as the global Rust allocator** by default (`turso/src/lib.rs:35–37`). The locked `libmimalloc-sys` build selects v3 unless its v2 feature is enabled. Its default purge delay is **1,000 ms**; the documented zero-delay setting purges unused OS pages immediately. The `purged` runs set the options before executable startup; they do not call a special post-measurement trim. `MIMALLOC_SHOW_STATS=1` additionally retains allocator exit diagnostics for those runs. [Default allocator](SOURCE-part-d.md#mimalloc-default), [version selection](SOURCE-part-d.md#mimalloc-version), [supported purge controls](SOURCE-part-d.md#mimalloc-purge).

## What dominates memory

These are source-backed explanations and measured ablations, **not an exact per-type heap allocation profile**:

- **Idle/search:** FTS residency dominates. Opening/history alone is about 15 MiB; actually touching FTS raises the process to about 600 MiB even for a single rare result. The engine materializes every visible segment before constructing the Tantivy view (`fts/mod.rs:1263–1314`), and its own cost model calls cold segment loading the dominant cost (`:3597–3605`). [Source](SOURCE-part-d.md#fts-residency). The 57.6-MB eligibility array is real but is not the main memory consumer. Projecting eligibility from SQL eliminates that array without limiting recall.
- **FTS updates/backup:** live segment data, assembled searcher views and maintenance/indexing work dominate over the tiny result heap or the nominal 8-MiB page-cache setting. The shared searcher cache retains up to **four views** by count (`fts/mod.rs:494–532`); the views retain their file-byte Arcs (`:1287–1345`). The separate **192-MiB segment-byte cache** explicitly does **not** bound live query memory, and a clean view can be cached when a writer first opens it (`:1350–1376`). [Searcher-cache source](SOURCE-part-d.md#fts-searcher-cache). Shrinking the page cache, shortening SQL statements, immediate purging and reusing one writer all leave the observed backup peak around or above 3 GB. This is not explainable by an exhaustive oracle, which is absent.
- **Writing/large index builds:** the page-cache setting is a **soft limit**, not a total page-buffer bound (`pager.rs:3741–3750`). At WAL commit, `ScanAndIssueReads` scans **all dirty page IDs**, rereads evicted pages without admitting them to the normal cache, and retains `PageSource::Evicted(PageRef)` values until commit completion (`pager.rs:4534–4586,4611–4627`). Prepared WAL frames are also retained in commit state. [Commit buffers](SOURCE-part-d.md#commit-resident-pages), [soft cache limit](SOURCE-part-d.md#soft-page-cache). This explains why an 8-MiB cache is not an 8-MiB write-memory cap. The detailed baseline/bounded import windows locate the large increase in **CREATE INDEX including its implicit commit**, not the subsequent PASSIVE call. The 100k single-writer backup's `commit-memory` records, however, show its high RSS already present while staging; the backup peak must not all be attributed to explicit COMMIT.
- **WAL frame metadata:** the single-process engine uses a dynamic page→frame-vector map (`wal.rs:2930–2940`), separate from page contents and FTS buffers. [Source](SOURCE-part-d.md#wal-frame-cache). Every measured backup checkpoint reports success and all frames backfilled, so these runs do not leave a long reader pinning an ever-growing WAL. The source does not make `cache_size` a cap on this map either.
- **Export:** the dominant controllable memory is page caching plus the 1-MiB output buffer and a bounded SQL page; no archive-sized Rust collection or FTS searcher is required. Changing 64 MiB to 8 MiB reduces measured export RSS from 84.3 MiB to about 24 MiB.
- **First import:** batch-local page metadata is bounded to at most 100k/10k input rows (observed unique-page maxima **98,002 / 9,991**), and dropped between batches. The large peaks occur during relational-index/FTS construction, not from an oracle or a Rust map of the full archive. The low-cache import reaches about 829 MiB during relational index work and about 1.17 GiB with FTS. Source settings for the sorter and commit buffers are linked above.

Mimalloc's unused-page purge setting is a legitimate startup control, but it does not free objects that the engine still references or impose a process ceiling. In the 10k two-writer controls, cache-only tuning measures **3,094,003,712 bytes** peak; immediate purge with the same cache/statement sizes measures **3,169,124,352 bytes**. It did **not** establish a reliable further peak reduction. The final one-writer variant is a sensible WAL ownership pattern and has the smallest tested backup peak, but still misses the proposed budget. No memory setting or row cutoff is presented as a guarantee for larger/richer FTS content.

## Workload and verification boundaries

**Idle.** Open the archive, load its schema/FTS definitions, serve a 100-row history response with URL/title data, and measure. A second idle window actually touches FTS with a rare-term request and serves history again. This distinguishes lazy schema opening from a resident FTS searcher.

**Search.** Each query/page operation gets a new process, then runs once cold and once warm. “Cold” means the first FTS request in that process; the OS file cache is not purged. First and next pages are separately cold: next-page measurements are not warmed by a first-page request. The [collector](src/d_search.rs) streams all native FTS hits and retains only 100/200 hits, then returns the 100 page details. It never adds a recent-window or match-count cap. Dense eligibility is built per request and dropped, so no cross-request cache or verification array survives. Inline eligibility reads `newest`/`visit_count` from the page projection in the same transaction and filters `visit_count>0`, using **zero archive-sized Rust eligibility bytes**. That applies to these all-time queries; arbitrary combined visit filters need their own separately validated query plan.

**Backup.** A [writer thread](src/d_backup.rs) adds 1M visits and updates their 1M FTS documents, in transactions of 10k/50k/100k. An interactive star request arrives after `BEGIN IMMEDIATE` and probes the real busy lock. History is served through the independent connection while the batch runs. The star/note/tag requests are queued until that batch commits, then served before a PASSIVE checkpoint and the next batch. Queue wait and SQL service time are separate measurements. No old reader is pinned across batches. These are WAL queue waits, not concurrent-writer commits.

**Export.** [d_export.rs](src/d_export.rs) writes all 14.4M rows to a real JSONL file, including URL, title, profile, time, domain, star, note and tags. It uses one consistent read transaction, 1,024-row keyset SQL pages, row-at-a-time serialization, and a 1-MiB output buffer; flush and `sync_all` are included. It holds no collection of all output rows.

**First import.** [d_import.rs](src/d_import.rs) streams the visits into an empty archive. A batch-local map holds page URL/title, visit count and newest time, then page upserts are sorted by ID for locality. The map/vector contain at most the transaction's row bound, never the whole archive. Relational indexes and native FTS are built once after ingestion; all builds and checkpoints are included in total import time and peak RSS. Until that final index build finishes, the first import is not presented as a fully searchable archive. This is an explicit bulk-load policy, not an incrementally maintained FTS import benchmark.

The [separate verifier](src/d_verify.rs) regenerates the entire visit stream, checks persisted visit tuples and page aggregates/content, verifies FTS coverage and annotations, and compares measured search results against an exhaustive native-score sort. Export verification streams every JSONL row against the generator in another process. Verifier RSS, setup RSS, and development smoke windows are excluded from every product-memory table.

<!-- GENERATED MEASUREMENTS -->

## Measured results

Memory values below are **MiB**; paired values are **peak / steady RSS**. Steady is the recorded 250-ms post-operation point. Load is **1/5/15 minutes**.

### Idle

| Profile                                     | Archive/schema + history, peak / steady | FTS warmed + history, peak / steady | Warmed FTS load start → end     |
| ------------------------------------------- | --------------------------------------: | ----------------------------------: | ------------------------------- |
| [baseline](logs/part-d-idle-baseline.jsonl) |                             14.6 / 14.6 |                       699.7 / 699.7 | 3.55/3.78/3.67 → 3.55/3.78/3.67 |
| [bounded](logs/part-d-idle-bounded.jsonl)   |                             14.6 / 14.6 |                       599.4 / 599.4 | 3.54/3.77/3.67 → 3.54/3.77/3.67 |
| [purged](logs/part-d-idle-purged.jsonl)     |                             14.5 / 14.5 |                       599.3 / 599.3 | 3.68/3.20/3.27 → 3.68/3.20/3.27 |

### Ranked search

Each row is a separately cold process followed by a warm repetition. The FTS stream covers the complete archive. The rare query has one matching page, so its next page is empty.

**baseline**

| Query / page                                                    | Cold peak / steady | Warm peak / steady |    Cold / warm ms | Load at cold / warm starts      |
| --------------------------------------------------------------- | -----------------: | -----------------: | ----------------: | ------------------------------- |
| [topic / first](logs/part-d-search-baseline-topic-0.jsonl)      |      672.7 / 672.7 |      674.0 / 674.0 | 888.133 / 670.010 | 3.55/3.78/3.67 / 3.55/3.78/3.67 |
| [topic / next](logs/part-d-search-baseline-topic-100.jsonl)     |      672.7 / 672.7 |      674.0 / 674.0 | 891.664 / 688.067 | 3.55/3.78/3.67 / 3.55/3.78/3.67 |
| [topic42 / first](logs/part-d-search-baseline-topic42-0.jsonl)  |      676.3 / 676.3 |      677.7 / 677.7 | 634.700 / 457.522 | 3.59/3.78/3.67 / 3.59/3.78/3.67 |
| [topic42 / next](logs/part-d-search-baseline-topic42-100.jsonl) |      676.3 / 676.3 |      677.7 / 677.7 | 631.269 / 449.963 | 3.59/3.78/3.67 / 3.59/3.78/3.67 |
| [rare / first](logs/part-d-search-baseline-rare-0.jsonl)        |      672.6 / 672.6 |      673.8 / 673.7 | 629.095 / 434.179 | 3.59/3.78/3.67 / 3.54/3.77/3.67 |
| [rare / next](logs/part-d-search-baseline-rare-100.jsonl)       |      672.5 / 672.5 |      673.6 / 673.6 | 610.053 / 426.003 | 3.54/3.77/3.67 / 3.54/3.77/3.67 |

**bounded**

| Query / page                                                   | Cold peak / steady | Warm peak / steady |    Cold / warm ms | Load at cold / warm starts      |
| -------------------------------------------------------------- | -----------------: | -----------------: | ----------------: | ------------------------------- |
| [topic / first](logs/part-d-search-bounded-topic-0.jsonl)      |      600.2 / 600.2 |      600.4 / 600.4 | 813.878 / 639.300 | 3.54/3.77/3.67 / 3.54/3.77/3.67 |
| [topic / next](logs/part-d-search-bounded-topic-100.jsonl)     |      600.2 / 600.2 |      600.4 / 600.4 | 794.198 / 632.758 | 3.90/3.84/3.69 / 3.90/3.84/3.69 |
| [topic42 / first](logs/part-d-search-bounded-topic42-0.jsonl)  |      603.9 / 603.9 |      605.8 / 605.8 |  223.763 / 56.325 | 3.90/3.84/3.69 / 3.90/3.84/3.69 |
| [topic42 / next](logs/part-d-search-bounded-topic42-100.jsonl) |      603.9 / 603.9 |      605.8 / 605.8 |  220.192 / 61.096 | 3.90/3.84/3.69 / 3.90/3.84/3.69 |
| [rare / first](logs/part-d-search-bounded-rare-0.jsonl)        |      600.2 / 600.2 |      600.2 / 600.2 |   171.749 / 1.440 | 3.90/3.84/3.69 / 4.22/3.90/3.72 |
| [rare / next](logs/part-d-search-bounded-rare-100.jsonl)       |      600.1 / 600.1 |      600.1 / 600.1 |   172.432 / 1.115 | 4.22/3.90/3.72 / 4.22/3.90/3.72 |

**purged**

| Query / page                                                  | Cold peak / steady | Warm peak / steady |    Cold / warm ms | Load at cold / warm starts      |
| ------------------------------------------------------------- | -----------------: | -----------------: | ----------------: | ------------------------------- |
| [topic / first](logs/part-d-search-purged-topic-0.jsonl)      |      600.2 / 600.2 |      600.3 / 600.3 | 932.411 / 672.894 | 3.68/3.20/3.27 / 3.68/3.20/3.27 |
| [topic / next](logs/part-d-search-purged-topic-100.jsonl)     |      600.2 / 600.2 |      600.3 / 600.3 | 867.495 / 665.873 | 3.79/3.23/3.28 / 3.79/3.23/3.28 |
| [topic42 / first](logs/part-d-search-purged-topic42-0.jsonl)  |      603.8 / 603.8 |      605.8 / 605.8 |  254.550 / 61.736 | 3.79/3.23/3.28 / 3.79/3.23/3.28 |
| [topic42 / next](logs/part-d-search-purged-topic42-100.jsonl) |      603.8 / 603.8 |      605.8 / 605.8 |  251.968 / 55.734 | 3.79/3.23/3.28 / 3.79/3.23/3.28 |
| [rare / first](logs/part-d-search-purged-rare-0.jsonl)        |      600.1 / 600.1 |      600.2 / 600.2 |   198.894 / 2.334 | 3.96/3.28/3.30 / 3.96/3.28/3.30 |
| [rare / next](logs/part-d-search-purged-rare-100.jsonl)       |      600.0 / 600.0 |      600.0 / 600.0 |   197.381 / 1.361 | 3.96/3.28/3.30 / 3.96/3.28/3.30 |

The same-cache eligibility control, [inline SQL with 64 MiB cache](logs/part-d-search-inline64-topic-0.jsonl), measures **660.1 / 660.1** cold and **660.2 / 660.2** warm. The dense array is exactly **57,600,016 bytes**; the inline variant uses **zero** archive-sized eligibility bytes. The heap capacity is **3,200 bytes** for the first page and **6,400 bytes** for the next page, plus at most 100 returned page-detail rows. RSS differences are not exact allocation subtraction: cache use and allocator reuse overlap.

### Backup: memory, queueing and reads

Every row adds 1M visits and updates 1M FTS page documents. The three interactive requests arrive while the current batch owns the writer; their SQL executes between batches. Queue wait is reported separately from service time.

| Profile / batch                                                          |     Peak / steady | Total s | Queue wait p50 / max ms | During-backup history p50 / max ms | Load at window start → end      |
| ------------------------------------------------------------------------ | ----------------: | ------: | ----------------------: | ---------------------------------: | ------------------------------- |
| [baseline / 10,000](logs/part-d-backup-baseline-10000.jsonl)             | 3,160.2 / 3,160.1 | 169.622 |   1,058.629 / 3,048.895 |                      0.114 / 0.141 | 1.56/2.39/3.03 → 3.07/2.75/3.06 |
| [baseline / 50,000](logs/part-d-backup-baseline-50000.jsonl)             | 3,239.2 / 3,239.1 | 120.787 |   5,157.135 / 7,983.635 |                      0.118 / 0.150 | 2.98/2.74/3.05 → 2.70/2.71/3.00 |
| [baseline / 100,000](logs/part-d-backup-baseline-100000.jsonl)           | 3,229.6 / 3,229.5 | 121.638 | 11,188.650 / 14,361.151 |                      0.128 / 0.158 | 2.80/2.73/3.00 → 3.28/2.84/3.00 |
| [bounded / 10,000](logs/part-d-backup-bounded-10000.jsonl)               | 3,180.0 / 3,027.5 | 477.062 |   3,508.821 / 7,393.140 |                      0.120 / 0.275 | 3.42/2.88/3.01 → 4.57/4.02/3.50 |
| [bounded / 50,000](logs/part-d-backup-bounded-50000.jsonl)               | 3,311.0 / 3,284.2 | 410.339 | 19,128.050 / 25,159.479 |                      0.120 / 0.209 | 4.52/4.02/3.50 → 3.29/3.59/3.51 |
| [bounded / 100,000](logs/part-d-backup-bounded-100000.jsonl)             | 3,297.7 / 3,258.1 | 408.613 | 37,695.008 / 52,931.690 |                      0.118 / 0.170 | 3.10/3.55/3.49 → 3.79/3.44/3.41 |
| [purged / 10,000](logs/part-d-backup-purged-10000.jsonl)                 | 3,022.3 / 2,967.9 | 194.127 |   1,221.869 / 3,274.251 |                      0.119 / 0.210 | 3.96/3.28/3.30 → 3.18/3.51/3.40 |
| [purged / 50,000](logs/part-d-backup-purged-50000.jsonl)                 | 3,080.4 / 2,993.4 | 169.098 |  7,289.190 / 11,936.395 |                      0.125 / 0.148 | 3.17/3.50/3.40 → 2.89/3.29/3.34 |
| [purged / 100,000](logs/part-d-backup-purged-100000.jsonl)               | 3,120.7 / 3,107.5 | 169.201 | 15,140.673 / 23,344.814 |                      0.132 / 0.175 | 2.82/3.27/3.33 → 4.08/3.68/3.49 |
| [single-purged / 10,000](logs/part-d-backup-single-purged-10000.jsonl)   | 2,939.5 / 2,939.5 | 217.281 |   1,383.636 / 2,911.113 |                      0.153 / 0.404 | 4.26/3.72/3.60 → 4.95/4.33/3.88 |
| [single-purged / 50,000](logs/part-d-backup-single-purged-50000.jsonl)   | 3,225.9 / 3,225.8 | 182.593 |  8,201.061 / 10,965.794 |                      0.158 / 0.522 | 5.44/4.44/3.92 → 5.23/4.88/4.21 |
| [single-purged / 100,000](logs/part-d-backup-single-purged-100000.jsonl) | 3,226.7 / 3,226.6 | 174.124 | 15,955.020 / 23,609.037 |                      0.175 / 0.399 | 5.35/4.91/4.23 → 6.95/5.60/4.62 |
| [cache8 / 10,000](logs/part-d-backup-cache8-10000.jsonl)                 | 2,950.7 / 2,950.6 | 186.535 |   1,127.219 / 2,683.670 |                      0.118 / 0.193 | 2.98/3.21/3.32 → 3.22/3.10/3.24 |

`baseline`, `bounded`, `cache8`, and `purged` use two write-capable connections. `single-purged` routes **all writes through one writer connection**, uses a query-only reader, and prewarms that reader’s FTS cache before backup. This last variant represents an already-used application, not an empty FTS cache. [Code](src/d_single_backup.rs).

Service and end-to-end response times for that one-writer variant:

| Batch / operation | SQL service p50 / max ms | Arrival-to-response p50 / max ms | Load at longest response arrival |
| ----------------- | -----------------------: | -------------------------------: | -------------------------------- |
| 10,000 / star     |            0.459 / 0.898 |            1,384.142 / 2,911.745 | 5.34/3.99/3.69                   |
| 10,000 / note     |      330.383 / 1,276.210 |            1,771.634 / 3,118.412 | 5.34/3.99/3.69                   |
| 10,000 / tag      |        328.003 / 489.521 |            2,094.220 / 3,331.933 | 5.34/3.99/3.69                   |
| 50,000 / star     |            2.211 / 3.140 |           8,203.588 / 10,967.757 | 6.19/4.64/4.00                   |
| 50,000 / note     |      349.151 / 1,578.988 |           8,586.368 / 11,176.828 | 6.19/4.64/4.00                   |
| 50,000 / tag      |        329.106 / 697.547 |           8,870.623 / 11,382.576 | 6.19/4.64/4.00                   |
| 100,000 / star    |            3.748 / 5.233 |          15,958.857 / 23,612.828 | 5.35/4.91/4.23                   |
| 100,000 / note    |      425.805 / 1,411.255 |          16,463.733 / 23,823.927 | 5.35/4.91/4.23                   |
| 100,000 / tag     |        376.713 / 604.977 |          16,909.597 / 24,040.390 | 5.35/4.91/4.23                   |

These waits are the current transaction boundary, not a claim that WAL permits concurrent writes. A 100k batch can keep an interactive write queued for the whole batch. History is served independently. The complete logs retain each request, initial busy result (two-writer controls), and PASSIVE checkpoint result.

### Full export and first import

| Workload / profile                                     |     Peak / steady | Total measured s | Load start → end                |
| ------------------------------------------------------ | ----------------: | ---------------: | ------------------------------- |
| [export / baseline](logs/part-d-export-baseline.jsonl) |       84.3 / 84.3 |           31.813 | 7.27/5.69/4.65 → 7.30/5.87/4.76 |
| [export / bounded](logs/part-d-export-bounded.jsonl)   |       23.6 / 23.6 |           25.483 | 6.78/5.81/4.75 → 7.88/6.12/4.89 |
| [export / purged](logs/part-d-export-purged.jsonl)     |       23.8 / 23.8 |           30.600 | 3.99/3.67/3.48 → 4.00/3.71/3.50 |
| [import / baseline](logs/part-d-import-baseline.jsonl) | 2,387.5 / 1,921.8 |          180.822 | 7.53/6.10/4.89 → 5.69/5.77/4.97 |
| [import / bounded](logs/part-d-import-bounded.jsonl)   | 1,195.2 / 1,195.1 |          377.675 | 5.69/5.77/4.97 → 6.55/5.89/5.30 |
| [import / purged](logs/part-d-import-purged.jsonl)     | 1,182.8 / 1,182.7 |          387.544 | 4.07/3.73/3.52 → 3.07/3.71/3.59 |

Export includes writing, flushing and syncing the complete JSONL file; it does not load FTS segments because export does not query FTS. A running process can also retain an earlier search cache, separately measured by the warmed-idle case. Import totals include ingestion, all four relational indexes, native FTS and checkpoints, including the five 250-ms phase observation intervals. The first-import FTS is built once after data ingestion.

| Import phase / profile   |     Peak / steady | Measured s | Load start → end                |
| ------------------------ | ----------------: | ---------: | ------------------------------- |
| page-visits / baseline   | 2,167.1 / 2,167.0 |     12.591 | 5.71/5.81/4.93 → 5.58/5.78/4.93 |
| visit-time / baseline    | 2,387.5 / 2,387.5 |     10.085 | 5.58/5.78/4.93 → 5.81/5.82/4.95 |
| visit-profile / baseline | 2,387.5 / 1,808.6 |     14.670 | 5.81/5.82/4.95 → 5.61/5.78/4.95 |
| domain / baseline        | 1,943.1 / 1,943.0 |      1.750 | 5.61/5.78/4.95 → 5.40/5.73/4.94 |
| fts / baseline           | 1,956.7 / 1,921.8 |     23.600 | 5.40/5.73/4.94 → 5.69/5.77/4.97 |
| page-visits / bounded    |     824.1 / 824.0 |     11.155 | 5.48/5.61/5.17 → 5.26/5.56/5.15 |
| visit-time / bounded     |     826.0 / 825.9 |      8.706 | 5.26/5.56/5.15 → 5.82/5.67/5.20 |
| visit-profile / bounded  |     828.5 / 828.4 |     13.132 | 5.82/5.67/5.20 → 6.12/5.75/5.23 |
| domain / bounded         |     828.5 / 828.4 |      1.906 | 6.12/5.75/5.23 → 6.12/5.75/5.23 |
| fts / bounded            | 1,195.2 / 1,195.1 |     23.241 | 6.12/5.75/5.23 → 6.55/5.89/5.30 |
| page-visits / purged     |     823.9 / 823.9 |     11.597 | 4.07/3.98/3.67 → 3.84/3.93/3.66 |
| visit-time / purged      |     825.4 / 825.4 |      9.090 | 3.84/3.93/3.66 → 3.63/3.88/3.65 |
| visit-profile / purged   |     828.6 / 828.6 |     12.986 | 3.63/3.88/3.65 → 3.38/3.82/3.63 |
| domain / purged          |     828.7 / 828.7 |      1.880 | 3.38/3.82/3.63 → 3.35/3.81/3.62 |
| fts / purged             | 1,182.8 / 1,182.7 |     25.090 | 3.35/3.81/3.62 → 3.07/3.71/3.59 |

## Reproducibility and retained evidence

All runs use stock cached Turso **0.8.1** and the existing lockfile. No external services, commits, upstream drafts, encryption keys or product-code changes were used. Neither measured nor verification entry points import `encryption.rs`, `crash.rs` or `spill.rs`; those three files were not opened or run. [Environment/build identity](logs/part-d-environment.json), [checked source/archive integrity](logs/part-d-source-integrity.json), [source excerpts](SOURCE-part-d.md).

The primary windows and outer import-index windows use the 250-ms post-operation observation described above. Later diagnostic subwindows separating index DDL from PASSIVE checkpoint use `steady_delay_ms=0`; the memory tables use the primary/outer windows, not those immediate subwindow endpoints. The old `fts-storage` diagnostic queries in verification logs read an internal backing-table placeholder and return zero; they are **not** used as a measurement of index bytes or as evidence of FTS coverage. Coverage comes from native FTS result enumeration and the separate oracle.

[A full artifact audit](logs/part-d-audit.json) confirms **41 reported measurement processes**, **38 independently checked search responses**, successful full-data verification for every backup, every export, and every import, and successful/full-backfill PASSIVE results after each backup batch. Development import smoke logs, a corrected startup `query_only=ON` syntax attempt, setup seeding, and all verifier resource usage are excluded from the product-memory results. The working reader setting is `query_only=1`.

A fresh reproduction uses only the allowed binaries:

```sh
cd research/turso-probes
export CARGO_HOME="$PWD/.work/cargo-home"
export CARGO_NET_OFFLINE=true
cargo build --offline --locked --release -j 4 --bin part-d --bin part-d-verify
export PROBE_BINARY=target/release/part-d
PROBE_LOG=part-d-setup python3 scripts/run_probe.py seed part-d-base baseline
for probe_profile in baseline bounded; do
  PROBE_LOG=part-d-idle-$probe_profile python3 scripts/run_probe.py idle part-d-base "$probe_profile"
  for probe_query in topic topic42 rare; do
    for probe_offset in 0 100; do
      PROBE_LOG=part-d-search-$probe_profile-$probe_query-$probe_offset \
        python3 scripts/run_probe.py search part-d-base "$probe_profile" "$probe_query" "$probe_offset"
    done
  done
done
PROBE_LOG=part-d-search-inline64-topic-0 \
  python3 scripts/run_probe.py search part-d-base inline64 topic 0
python3 scripts/run_part_d_writes.py
python3 scripts/run_part_d_cache_control.py
python3 scripts/run_part_d_purged.py
python3 scripts/run_part_d_remaining.py
python3 scripts/audit_part_d.py
python3 scripts/summarize_part_d.py
```

Use fresh directories. The drivers run each measured process to exit, run its verifier afterwards, and reclaim completed disposable fixtures. The remaining driver also runs the FTS-prewarmed, single-writer comparisons before its export/import controls. The supervisor records command, exit, timeout, peak child RSS and load. [Measurement inventory](logs/part-d-summary.json), [audit implementation](scripts/audit_part_d.py), [table generator](scripts/write_part_d_tables.py).

Validation is the standalone offline release build and actual Turso workloads, not mocks. Product checks were not run because this task changes only the standalone research project. Final removal of `research/turso-probes/data/` and `target/` is recorded in [part-d-cleanup.json](logs/part-d-cleanup.json); code, logs and source evidence remain.
