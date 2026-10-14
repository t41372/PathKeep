# vault-search benchmark at 2.9M pages (2026-10-05)

> Measured on a macOS arm64 development host (macOS 27.0.1) on 2026-10-05, for the [vault-search crate](../../../src-tauri/crates/vault-search/README.md). Raw evidence files were not kept.

# Encrypted search measurements — 2026-10-05

All indexes contain **2,900,000 page documents**, written directly through
`vault_crypto::EncryptedDirectory`. These are engine measurements on the sandboxed
arm64 development host (macOS 27.0.1, Rust 1.96.0, Tantivy 0.26.2), not measurements
on physical 4-core / 3 GHz / 8 GB hardware. CPU/RAM inventory queries were blocked
by the sandbox. No external service was contacted.

The streaming fixture has sixteen prose title templates, unique URL identifiers,
20,000 domains, raw/normalized title and URL fields, search terms on one fifth of
pages, bounded enrichment/notes/tags on subsets, Chinese grams, three profiles with
additional multi-profile membership, and repeated recency values. All ten fields
and complete prefix aliases are indexed, plus trigrams over the compact text.
Controlled probe terms have exact known frequencies. The predicate selects every
50th page. This is reproducible synthetic page-shaped text, not a sampled user archive.
Canonical SQL, eligibility construction, hydration and IPC are excluded.

## Reproduce

From the repository root (no network access needed; dependencies must be cached):

```sh
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR=/tmp/pathkeep-targetcargo test --manifest-path src-tauri/Cargo.toml --release -p vault-search --test benchmark encrypted_2_9m_pages -- --ignored --nocapture
```

The default run compares 32/128/64 MiB writer arenas and 16/64/128 MiB block caches.
`SEARCH_BENCH_WRITER_MIB=64` repeats the default writer build and all query/cache
measurements. Both runs are retained: [calibration](evidence/benchmark-calibration.jsonl)
and [final](evidence/benchmark-final.jsonl). Query results below are from the final
run after moving cold BM25 statistics into the time-sliced traversal. The writer
path did not change; its 32/128 MiB calibration measurements remain applicable.

Every query/count/progressive case starts in a new child process. “Cold” means a
fresh process and empty directory cache; the OS file cache was **not** purged.
Warm times are medians of nine more runs. Next-page timing follows its first page
in that process because the cursor owns a live snapshot; it is not independently
cold. Rank/count timing includes handle creation and traversal; opening the index
and constructing the session are recorded separately in the raw evidence.

Automatic merges remain enabled. Build timing ends at the durable commit and
reader reload; it is not a forced single-segment merge. Writer thread count is two.
Tantivy's default merger pool is retained. RSS high-water comes from `getrusage`;
steady RSS is the process's resident size from `task_info` after repeated searches.
The Linux runner reads `/proc/self/statm`. No `ps` permission is needed.

## Build and memory

All builds below use a 128 MiB directory cache. RSS includes arenas, ingestion,
merges, encryption cache, readers and runtime allocations; an arena budget is not
an RSS ceiling. Disk size includes every remaining index file after closing the writer.

| Writer MiB | Build seconds | Peak RSS MiB | Disk MiB |
| ---------: | ------------: | -----------: | -------: |
|         32 |        241.73 |        391.5 |   1689.8 |
|         64 |         63.10 |        387.8 |   1540.5 |
|        128 |         49.39 |        575.2 |   1574.7 |

Defaults: **64 MiB writer arena, two indexing threads, 128 MiB decrypted cache,
AfterCommit reader reload**. The 32 MiB arena sits close to Tantivy's per-thread
minimum and made this build much slower without a lower measured peak. The
128 MiB arena improved build throughput but increased peak RSS substantially.
64 MiB keeps that trade-off within the 8 GB target's memory budget while leaving
room for canonical storage, the webview and the OS. Combined app memory pressure
and physical target-machine latency still need measurement during integration.

Opening for reads allocates no writer. AfterCommit uses an explicitly reloaded
manual Tantivy reader; it creates no extra delayed reload worker or result cache.
Caller-retained sessions pin old readers/segments, so the owner must release them.

## Top 100 and exact counts

Times are milliseconds. Each triple is **first page / next page / separate count**.
The count uses its own fresh process, not the warmed ranking process.

| Query                    |   Matches | Fresh-process first / next / count | Warm first / next / count | Steady RSS MiB, rank / count |
| ------------------------ | --------: | ---------------------------------- | ------------------------- | ---------------------------- |
| archive (common)         | 2,900,000 | 345.83 / 43.46 / 291.26            | 39.89 / 40.60 / 13.73     | 329.0 / 309.6                |
| webassembly (medium)     |     9,149 | 320.01 / 5.17 / 298.35             | 4.59 / 4.56 / 1.10        | 331.3 / 323.1                |
| raresentinel (rare)      |         1 | 276.82 / 1.47 / 278.82             | 1.34 / 1.36 / 0.66        | 308.7 / 305.2                |
| archive + domain         |       145 | 327.06 / 11.83 / 280.72            | 10.38 / 10.38 / 0.79      | 335.2 / 313.0                |
| archive + 2% eligibility |    58,000 | 342.21 / 31.89 / 291.99            | 31.84 / 31.88 / 15.79     | 327.2 / 305.1                |

All first/next-page digests and exact counts agree across all three cache sizes.
Rare-term page two is empty. The domain has 145 matches, so page two has 45 rows.
The common term reaches every page; no candidate window is used.

## Cache comparison

Warm first-page latency, milliseconds; RSS is the maximum steady rank/count RSS
observed across the five cases at that cache size.

| Cache MiB | Common | Medium | Rare | Domain | Eligibility | Max steady RSS MiB |
| --------: | -----: | -----: | ---: | -----: | ----------: | -----------------: |
|        16 |  97.29 |  60.32 | 1.33 |  74.89 |       88.62 |              233.7 |
|        64 |  40.84 |   4.67 | 1.36 |  75.47 |       32.17 |              294.7 |
|       128 |  39.89 |   4.59 | 1.34 |  10.38 |       31.84 |              335.2 |

The 128 MiB cache avoids the selective-domain refill seen at 64 MiB and the broad
refills seen at 16 MiB. Norms alone are about 29 MB for ten fields at this scale;
fast fields, term dictionaries, postings and shared decrypted ranges also compete
for memory. The cache limit covers retained cache entries, not buffers still held
by Tantivy readers or the allocator. The measured RSS above includes those costs.

Beyond Tantivy's index readers, each ranking handle retains O(query terms + page
size) state; counts retain one integer and one active segment traversal. There is
no matching-page list, prefix expansion list or ever-growing pagination offset.
The benchmark checks cache occupancy against its configured byte limit. These
bounds are not a claim that arbitrary documents, queries or retained sessions
have a fixed total RSS.

## Progressive work, 8 ms requested budget

Fresh-process ranking now starts without I/O. Global BM25 metadata reads are part
of the stepped traversal and report their own completed/total field counters.
Counts can already advance while ranking gathers global statistics. Steps report
provisional results; the final top 100/count equals its one-shot result.

| Query                    | Work  | Cold handle creation ms | Cold steps | Cold maximum step ms | Cold total ms | Warm maximum step ms | Warm total median ms |
| ------------------------ | ----- | ----------------------: | ---------: | -------------------: | ------------: | -------------------: | -------------------: |
| archive (common)         | rank  |                   0.004 |         43 |                 8.88 |        348.73 |                 8.03 |                41.33 |
| archive (common)         | count |                   0.001 |         35 |                 9.75 |        290.73 |                 8.00 |                14.12 |
| webassembly (medium)     | rank  |                   0.004 |         40 |                 9.48 |        321.62 |                 4.65 |                 4.60 |
| webassembly (medium)     | count |                   0.001 |         34 |                10.35 |        298.98 |                 8.09 |                 1.04 |
| raresentinel (rare)      | rank  |                   0.004 |         35 |                 8.62 |        279.52 |                 1.41 |                 1.34 |
| raresentinel (rare)      | count |                   0.001 |         33 |                10.53 |        279.46 |                 5.43 |                 0.66 |
| archive + domain         | rank  |                   0.005 |         41 |                 8.46 |        328.96 |                 8.03 |                10.44 |
| archive + domain         | count |                   0.001 |         33 |                10.05 |        282.06 |                 8.00 |                 0.79 |
| archive + 2% eligibility | rank  |                   0.003 |         42 |                 9.58 |        340.08 |                 8.05 |                32.19 |
| archive + 2% eligibility | count |                   0.001 |         36 |                 9.69 |        292.76 |                 8.00 |                15.80 |

Budgets are cooperative, not hard deadlines: a field read, scorer setup or caller
predicate cannot be preempted. Checks occur after each metadata field, every 256
candidates and at segment boundaries. Cold whole-search latency is therefore
visible as many bounded steps rather than an opaque statistics-loading pause.
All individual step durations, including setup, are in the JSONL evidence.

## Load and cleanup

Final-run load-average ranges were 4.24–9.12 / 4.15–5.61 / 4.85–5.34 (1/5/15 minutes). Every child records the full `uptime` string. Other workspace work shared the host; small timing differences are not causal claims.

Only one generated index was retained at a time. The largest observed size across the recorded builds was 1689.8 MiB, below 10 GB. The runner checks size during construction, removes each index before the next build, and asserts the entire temporary root is absent at exit. Both recorded runs ended with `cleanup.exists = false`; the paths were independently checked afterwards. No generated index or source corpus remains.
