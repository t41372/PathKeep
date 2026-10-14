# Encrypted vault-core search benchmark — 2026-10-05

This is the whole owner path, including encrypted Turso eligibility and hydration,
not an engine-only search. The fixture has 14,400,000 visits and 2,900,000 distinct
pages. Each query returns 100 rows per page. All five counts are checked against
the generated facts. The date range covers 288,000 visits (2%), and the profile
filter selects one of three profiles. The rare term has only one match, so it has
no next page.

The run used the local macOS host. Hardware inventory was denied by the execution
environment; these numbers are measurements of this host, not a claim of a verified
4-core / 8 GB hardware run. Index settings remain two writer threads, 64 MiB arena
and 128 MiB decrypted cache. “Cold” below means a fresh process whose index/decryption caches have not
served another query. Vault open is outside the query timers. The OS filesystem cache was not purged. Warm values are medians
of three subsequent runs. Load averages are retained in the raw artifact.

## Queries

All latency columns are milliseconds. “First results” is the first nonempty step,
including a final step when the query finishes immediately. The start response and
initial indexer event precede it. Ranking and counting use independent walks; count
completion need not coincide with the final first page.

| Query               | Cache | First results | Final first page | Next page | Exact count | Count ready | Steady RSS (MiB) |
| ------------------- | ----- | ------------: | ---------------: | --------: | ----------: | ----------: | ---------------: |
| Common: archive     | Cold  |         93.52 |         8,320.58 |    267.09 |   2,900,000 |    8,319.94 |           174.17 |
| Common: archive     | Warm  |         11.37 |         8,200.98 |    267.64 |   2,900,000 |    8,200.98 |           176.61 |
| Medium: webassembly | Cold  |        130.63 |           176.63 |      4.95 |       9,148 |      176.63 |           165.14 |
| Medium: webassembly | Warm  |         23.87 |           109.20 |      5.03 |       9,148 |      109.20 |           166.09 |
| Rare: raresentinel  | Cold  |         56.23 |            59.68 |         — |           1 |       49.44 |            89.81 |
| Rare: raresentinel  | Warm  |          1.59 |             1.59 |         — |           1 |        1.59 |           134.72 |
| archive + date (2%) | Cold  |        407.37 |           410.96 |    226.13 |      57,600 |      185.48 |           119.95 |
| archive + date (2%) | Warm  |        329.85 |           336.05 |    226.52 |      57,600 |      128.55 |           164.59 |
| archive + profile   | Cold  |        103.89 |         3,807.94 |    264.11 |     966,666 |    3,806.69 |           178.30 |
| archive + profile   | Warm  |         16.15 |         4,186.35 |    286.77 |     966,666 |    4,185.45 |           179.94 |

The worker targets 8 ms steps, sharing the budget between ranking and count walks.
Canonical SQL batches contain at most 1,024 posting candidates, and regex batches
contain 512 visits; both continue until exhaustion. Individual SQL/I/O operations
can exceed the target budget. Rare/tag/date queries do not need a preliminary
whole-archive MAX aggregate. The date case must resolve correlated eligibility and
walk postings before its first eligible hit; progress/counts can arrive earlier.

## Backup contention and indexing

Each trial runs the real Chromium parser and backup path for 20,000 visits touching
10,000 existing pages. Pending index work is drained before each trial. The indexer
is paused only for the two baseline trials. Its catch-up after a baseline is outside
the timed backup. Document throughput counts applied replacements during the backup,
including more than one committed change to a page; it is not unique pages/second.

| Trial | Indexer | Backup seconds | Visits/s | Documents applied during backup | Documents/s | Load averages  |
| ----- | ------- | -------------: | -------: | ------------------------------: | ----------: | -------------- |
| 0     | Paused  |         10.221 |  1,956.8 |                               0 |         0.0 | 4.58 5.45 6.69 |
| 1     | Running |         10.357 |  1,931.0 |                          15,000 |     1,448.3 | 4.87 5.48 6.68 |
| 2     | Running |         10.424 |  1,918.7 |                          15,000 |     1,439.0 | 5.18 5.53 6.69 |
| 3     | Paused  |         10.182 |  1,964.3 |                               0 |         0.0 | 5.47 5.58 6.69 |

Observed mean backup throughput was 1,924.9 visits/s with indexing and
1,960.5 visits/s while paused (1.82% lower). These are
paired local trials with changing background load, not a hardware-independent ratio.

A fifth running backup included a real star and note write. Both commits returned
in 4.39 ms. One indexer cycle completed at 1751.44 ms
(1747.05 ms after commit), and a search restricted to starred pages
verified the note at 1853.88 ms from write start. Load: 18:28 5 users, load averages: 4.77 5.42 6.61.

## Rebuild, memory and cleanup

The full encrypted rebuild took 35.569 seconds, with peak RSS
404.02 MiB and post-rebuild RSS 396.70 MiB.
Load: 18:27 5 users, load averages: 4.78 5.51 6.72. Query RSS appears in the table above. The backup process
peaked at 1119.91 MiB; its final measured steady RSS was
1107.58 MiB. The seed process peaked at 807.61 MiB.

The seeded files occupied 5,807,677,233 bytes (5.808 GB).
The test enforces a 40 GB ceiling during generation and after each workload.
It removed its generated root and verified `cleanup.exists == false`.

## Reproduction and evidence

```sh
CARGO_NET_OFFLINE=true \
CARGO_TARGET_DIR=/tmp/pathkeep-target \
cargo test --release --manifest-path src-tauri/Cargo.toml -p vault-core --lib \
  search::benchmark::encrypted_archive_search -- --ignored --exact --nocapture --test-threads=1
```

The local execution also used a test-only dynamic network guard permitting loopback
fixtures and Unix sockets while denying outside resolution/connections. The benchmark
itself has no network workload. The guard was verified with a numeric-address lookup
and an invalid socket descriptor, without an outside connection.

[Raw per-run JSON Lines](2026-10-vault-core-search-benchmark.jsonl) contains all
query timings, step counts, maximum observed step duration, RSS and `uptime` output.
The ignored benchmark is in `vault-core/src/search/benchmark.rs`; fixture, measurement
and real-backup workloads are separate modules below `search/benchmark/`.
