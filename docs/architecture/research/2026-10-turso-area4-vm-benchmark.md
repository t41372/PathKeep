# Area 4 intelligence benchmark: final record of the interrupted microVM run

The benchmark is **incomplete**. The user confirmed that the old Modal account ran out of
credit, the account was switched, and Sandbox `sb-01M4A6NR9CMF0VK2H8C3F4EMQR` and its run
are gone and cannot be restored. **No replacement run was started.** This report preserves
the measurements retrieved before access was lost and the progress delivered by the original
offload session afterward.

This run used commit `ba9cd9f7`. Product source, SQL, schemas, checkpoint policy and cipher
were unchanged. The measurement scripts remain in `scripts/`, uncommitted, for a future run.
Earlier integration and gate results are in [the integration report](2026-10-turso-area4-modal.md).

## Results and missing measurements

| Measurement                                     | Surviving result                                                                                                            |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Visit-derived-facts stage                       | **9,193.814 s**, completed for all **14,400,000 visits**                                                                    |
| Daily rollups                                   | Completed; entry into navigation confirms that this stage returned. Exact stage duration was not retrieved                  |
| Navigation / sessions rebuild                   | Completed; entry into aggregate refind processing confirms that this stage returned. Exact stage duration was not retrieved |
| Aggregates                                      | Started; last delivered `refind-pages` progress reported **2,048 visits scanned**                                           |
| Publish / finalization                          | Not reached in the delivered evidence; no completed publication result                                                      |
| Full rebuild total time                         | Missing: the rebuild did not finish. Last delivered progress had elapsed **10,593.589 s** since rebuild start               |
| Full rebuild peak RSS                           | Missing: the completed `getrusage` high-water measurement was never emitted                                                 |
| Increment after 10,000 visits                   | Not reached                                                                                                                 |
| Overview cold / warm                            | Not reached                                                                                                                 |
| Domain cold / warm                              | Not reached                                                                                                                 |
| Sessions cold / warm                            | Not reached                                                                                                                 |
| Interactive star write during rebuild           | **1.442 ms**, returned while fact derivation continued                                                                      |
| Rebuild-start load averages, 1 / 5 / 15 minutes | **1.00 / 1.00 / 1.00**                                                                                                      |

The last elapsed timestamp is a progress observation, **not** a full-rebuild duration. No
missing value is extrapolated from the completed fact stage. The incremental and read
measurements were scheduled after a completed, published intelligence model; measuring an
empty or partial model would not satisfy the benchmark.

### Exact fact-stage timing

The external source-boundary probes recorded these monotonic timestamps:

| Boundary      |      Timestamp, ns |
| ------------- | -----------------: |
| `facts-begin` |  3,023,896,550,660 |
| `facts-end`   | 12,217,710,977,681 |
| `daily-begin` | 12,217,711,179,786 |

The fact-stage difference is **9,193.814427021 seconds**, reported above to three decimal
places. It includes debugger instrumentation overhead. The rebuild's final returned timing
report was not produced, so this is an observed boundary duration, not a value recovered from
a completed `CoreIntelligenceRebuildReport`.

### How far daily rollups and navigation got

These are delivered progress records. All elapsed times are measured from the start of the
full rebuild, not from the start of the named stage.

| Phase                       | Reported processed items |     Elapsed, s |
| --------------------------- | -----------------------: | -------------: |
| Daily rollups               |                    2,048 |      9,208.931 |
| Daily rollups               |                1,224,704 |      9,268.953 |
| Daily rollups               |                3,649,536 |      9,389.664 |
| Daily rollups               |                6,080,512 |      9,509.691 |
| Daily rollups               |                9,732,096 |      9,689.714 |
| Daily rollups               |               10,917,888 |      9,749.719 |
| Daily rollups               |               14,400,000 |      9,809.724 |
| Navigation (`sessions`)     |                    2,048 |      9,924.985 |
| Navigation (`sessions`)     |                1,339,392 |      9,985.051 |
| Navigation (`sessions`)     |                2,652,160 |     10,045.122 |
| Navigation (`sessions`)     |                3,969,024 |     10,105.180 |
| Navigation (`sessions`)     |                5,285,888 |     10,165.188 |
| Navigation (`sessions`)     |                6,572,032 |     10,225.205 |
| Navigation (`sessions`)     |                7,854,080 |     10,285.242 |
| Navigation (`sessions`)     |                9,121,792 |     10,345.314 |
| Navigation (`sessions`)     |               10,397,696 |     10,405.382 |
| Navigation (`sessions`)     |               11,681,792 |     10,465.416 |
| Navigation (`sessions`)     |               12,967,936 |     10,525.494 |
| Navigation (`sessions`)     |               14,239,744 |     10,585.514 |
| Aggregates (`refind-pages`) |                    2,048 | **10,593.589** |

Some daily checkpoint callbacks also reported the full 14,400,000 count before the daily
walk finished. Those values alone do not establish completion. The subsequent `sessions`
phase does: the pipeline calls navigation only after daily rollups return. Likewise, entering
`refind-pages` establishes that navigation returned. The first callback in a stage occurs
after work has already begun, so differences between these progress timestamps are **not**
substitutes for the missing exact stage-boundary durations.

## Completed fact-stage profile

GDB sampled all-thread stacks at five-second intervals. Waiting stacks were filtered using
their leaf frames.

| Observation                                                   |     Count |
| ------------------------------------------------------------- | --------: |
| Stack samples tagged `visit-derived-facts`                    | **1,832** |
| Non-waiting thread-stack observations                         | **1,872** |
| Observations containing software AES (`softaes::`)            | **1,388** |
| Observations containing checkpoint internals                  |   **575** |
| Checkpoint observations in the commit path (`op_auto_commit`) |   **566** |
| Checkpoint observations in explicit pragmas (`op_checkpoint`) |     **8** |
| Checkpoint observations unclassified at the captured depth    |     **1** |
| Observations containing `vault_store::session::transaction`   |       242 |
| Observations containing `build_rows::persist_facts`           |        67 |

These categories overlap. They are stack observations, **not CPU-time percentages or exact
checkpoint wall times**. The captured depth was 48 frames; absence of an application frame
does not prove that it was absent deeper in the call stack.

The observed commit/checkpoint stack was:

```text
turso_core::vdbe::execute::op_auto_commit
  → Program::commit_txn / commit_txn_wal / step_end_write_txn
  → Pager::commit_tx
  → Pager::checkpoint / checkpoint_inner
  → WalFile::checkpoint_inner
  → write_pages_vectored / database::write_pages
  → EncryptionContext::encrypt_page
  → Aegis256Cipher::encrypt
  → aegis::pure_rust::aes_soft::AesBlock::round
  → softaes::bitsliced::block_encrypt / sub_bytes_shift_rows
```

This establishes encryption and backfill work **inside commit**, in addition to the
application's explicit checkpoints between serial batches. The 566 commit-path observations
are distinguished by the actual `op_auto_commit` frame, not merely by failure to find the
explicit checkpoint wrapper. Another captured commit stack reached
`vault_store::session::transaction` at `session.rs:176`, the `COMMIT` call.

A separate fact-insert stack reached `Statement::execute` at `statements.rs:33` and
`build_rows::persist_facts` at `build_rows.rs:67`, underneath B-tree page-editing work.
Together with the batch observations below, this identifies the fact write/commit path as
a substantial cost. No cipher or checkpoint change was applied.

### Interim batch observations

The following capture was retrieved partway through fact derivation. It is **not a completed
statement-time breakdown**. Durations include preparation, row conversion and owner-queue
waits; scopes can nest. Unmatched debugger returns were retained explicitly rather than
counted as successful measurements.

| Function                    | Matched probe records | Unmatched returns | Observed inclusive time, s | Median, ms | Maximum, ms |
| --------------------------- | --------------------: | ----------------: | -------------------------: | ---------: | ----------: |
| `build_rows::persist_facts` |                 1,149 |               170 |                 337.792395 | 350.693591 |  564.392148 |
| `build_rows::source_page`   |                 1,231 |                 6 |                  15.218547 |  12.336614 |   16.046052 |
| `build_state::save`         |                 1,234 |                 2 |                   0.979735 |   0.574638 |   46.381764 |

These are probe-record counts, not an audited count of all SQL executions. The incomplete
coverage and nested scopes prevent summing or extrapolating these observations into the
full stage duration.

## SQL and EXPLAIN QUERY PLAN evidence retrieved

The following read plans were captured by the native Store on the real scale archive using
the read path's bindings. A complete cross-stage ranking of the slowest statements was not
recovered before the Sandbox was lost.

### Archive source-page read

```sql
SELECT v.id, ?1, v.source_profile_id,
       COALESCE(CAST(v.source_visit_id AS INTEGER), 0),
       u.id, u.url, u.title, v.visit_time_ms, v.from_visit,
       v.transition_type, v.external_referrer_url
FROM visits v INDEXED BY idx_visits_visible_profile_time_id
JOIN urls u ON u.id = v.url_id
WHERE v.source_profile_id = ?2
  AND v.reverted_at IS NULL
  AND v.id <= ?3
ORDER BY v.visit_time_ms, v.id
LIMIT ?4;
```

```text
SEARCH v USING INDEX idx_visits_visible_profile_time_id (source_profile_id=?)
SEARCH u USING INTEGER PRIMARY KEY (rowid=?)
```

### Source search-term hydration

This statement uses batches of up to 400 requested URL IDs. The placeholder list below is
abbreviated; the reported native plan also occurred for the smaller tail batch.

```sql
WITH requested(url_id) AS (VALUES (?2), …, (?401))
SELECT t.url_id, t.normalized_term
FROM requested
CROSS JOIN search_terms t INDEXED BY idx_search_terms_profile_url_term
  ON t.url_id = requested.url_id
WHERE t.source_profile_id = ?1 AND t.reverted_at IS NULL
ORDER BY t.id;
```

```text
SCAN requested
SEARCH t USING INDEX idx_search_terms_profile_url_term (source_profile_id=? AND url_id=?)
USE SORTER FOR ORDER BY
```

The scan is over the bounded requested-ID list. This fixture has no search terms, so these
observations do not characterize a large matching term set.

### Fact and navigation writes

The verified 10,000-visit encrypted smoke fixture, **not the interrupted scale run's final
write-plan probe**, returned no EQP rows for these fixed-`VALUES` statements:

- `INSERT OR REPLACE INTO visit_derived_facts_rows (...) VALUES (?1, …, ?16)`
- `INSERT OR REPLACE INTO visit_assignments (...) VALUES (?1, …, ?7)`
- `INSERT INTO session_domains (...) VALUES (?1, …, ?5) ON CONFLICT (...) DO UPDATE ...`

The fact insert was observed in the scale-run execution stacks. An empty EQP for a
fixed-`VALUES` write does not mean the write is free: index maintenance, encryption, WAL
writes and commit-time checkpoint work are not described by scan/seek plan rows.
No native EQP for `COMMIT` was captured.

## Environment and measurement method

| Property                 | Value                                                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Source commit            | `ba9cd9f7`                                                                                                               |
| Modal Sandbox            | `sb-01M4A6NR9CMF0VK2H8C3F4EMQR`                                                                                          |
| Offload key              | `area4-benchmark-ba9cd9f7`                                                                                               |
| Runtime / reservation    | `vm`; **8 CPUs / 32 GiB** from the current project configuration                                                         |
| CPU reported by the VM   | Intel Xeon Platinum 8375C @ 2.90 GHz                                                                                     |
| OS                       | Linux 7.2.9, x86_64                                                                                                      |
| Reported `MemTotal`      | 32,877,772 KiB                                                                                                           |
| Exposed process affinity | CPUs 0–23; no four-core affinity override was applied in this run                                                        |
| Rust / Turso             | Rust 1.96.0 / Turso 0.8.1                                                                                                |
| Build                    | Cargo release; workspace `lto = "thin"`, `codegen-units = 1`; debug information retained and symbol stripping disabled   |
| Encryption               | Existing vault-store **`aegis256`**, unchanged                                                                           |
| Command                  | `offload run --key area4-benchmark-ba9cd9f7 --as-root --timeout-min 1440 -- 'python3 -u scripts/run-area4-benchmark.py'` |

These are microVM results, not gVisor results. They use the **8-CPU/32-GiB** configuration
provided for this rerun and must not be presented as completed results for the earlier
4-CPU/8-GiB target.

The corpus was fully generated: **14,400,000 visits, 2,880,000 stored pages, one profile and
20,000 domains**. Every 144th visit goes to page 1, leaving 20,000 stored pages without visits.
Ten-second visit spacing produces one continuously open session; session-list timings from
this fixture would not characterize a large population of short sessions.

The unchanged rebuild runs with 2,048-visit batches. The star request targets
`https://site1.test/page/1` after at least 4,096 facts have been processed. Cold reads were
defined as the first read in a fresh process and Vault; OS caches were not to be flushed.
Warm reads would immediately repeat that read. The fixed read window was 2025-03-01 through
2025-04-07, the domain was `site1.test`, and the sessions request used a page size of 50.
Those cold/warm measurements were never reached.

The harness reuses the existing fixture and product API. Cargo supplies the worker's
dependency definitions, copied lockfile versions and workspace release profile. The prior
manual-link LTO failure was corrected without changing product code. A small encrypted
fixture verified the returned report, all required stage boundaries, batch probes and native
query-plan capture before the large run.

The rebuild report exposes fact, daily and combined structural timings. External
source-boundary probes split navigation from aggregates and time publication. The intended
final report JSON was written only after the full rebuild returned, which did not happen.
SQL plan capture was bounded to a small sample of real batches; the planned read measurements
excluded separate plan-probe reads.

GDB sampling and breakpoint overhead are included in the observed fact-stage wall time.
The final sampling-pause total was not recovered, so no overhead-corrected duration is given.

### Surviving resource observations

| Observation                                      | Value                                                        |
| ------------------------------------------------ | ------------------------------------------------------------ |
| Rebuild-start uptime output                      | `04:22:00 up 50 min, 0 user, load average: 1.00, 1.00, 1.00` |
| Workload RSS after about 5.03M facts             | 92,860 KiB                                                   |
| Workload RSS after about 6.52M facts             | 98,024 KiB                                                   |
| Workload RSS after about 9.01M facts             | 98,944 KiB                                                   |
| Workload RSS after 11.52M facts                  | 99,592 KiB                                                   |
| Workload RSS during early daily rollups          | **117,368 KiB**                                              |
| Separate GDB RSS in that daily-stage observation | 1,169,660 KiB                                                |
| Last retrieved fixture footprint, `du -sh`       | **9.6 GiB**, during early daily rollups                      |

These are sampled RSS values and a rounded disk-footprint observation, **not final peaks**.
The harness's final workload peak would have come from
`getrusage(RUSAGE_SELF).ru_maxrss`, excluding GDB. The final peak and final disk size are
unknown. The benchmark retained its 40 GB data guard.

## Why the remaining results are unavailable

After daily rollups, new diagnostic calls failed with:

```text
modal.exception.PermissionDeniedError: caller does not have read access to this app. Contact your workspace administrator to update permissions.
```

The original offload session continued to deliver navigation and aggregate progress. Its last
delivered progress was:

```text
INTELLIGENCE_BENCH phase=refind-pages processed=Some(2048) total=Some(14400000) elapsed_ms=10593589
```

The local execution handle subsequently ended with exit code **143**, with no further offload
output. The user then confirmed the account credit exhaustion, account switch and loss of
the Sandbox. This final interruption is distinct from the earlier heartbeat-thread defect,
which the user had fixed before this run.

The generated archive was under `/tmp/pathkeep-area4-j52wa652`. Raw logs, profiles, plans and
report files were under `/state/area4-observed`. The run never reached its normal cleanup or
final state-save step; those files are no longer accessible and cannot be restored. No
large benchmark archive or raw gate/profile log was copied into this clone. This document
preserves the summaries that had already been retrieved, rather than claiming a recoverable
complete profile or a successful cleanup callback.

Missing results are therefore: full rebuild duration and peak RSS; exact daily, navigation,
aggregate and publish durations; final debugger-pause and SQL-profile totals; the completed
cross-stage SQL ranking and remaining plans; incremental-update timing; and all six cold/warm
read timings. No replacement benchmark was launched after the user's stop instruction.

## Scripts retained for a future run

- `scripts/run-area4-benchmark.py`: Cargo-built measurement harness, fixture, structured
  report capture, bounded native plans, resource logging and normal-completion cleanup.
- `scripts/profile-native.py`: streaming GDB samples, inferior output and sampling-pause data.
- `scripts/profile-stages-gdb.py`: stage boundaries and batch-function observations.
- `scripts/summarize-area4-benchmark.py`: completed-run measurement and plan reduction;
  deliberately rejects missing or unsuccessful completion reports.
- `scripts/profile-checkpoints-gdb.py` and `scripts/summarize-checkpoint-profile.py`:
  the earlier explicit-checkpoint timing probes and reducer.

The 10,000-visit smoke report returned 513 ms for facts, 248 ms for daily rollups,
1,167 ms for structural work and 2,148 ms total before overview warming. Its external
navigation/aggregate boundaries spanned 435.545176 ms and 731.442585 ms; publication spanned
4.296547 ms. All required stage markers and native plans were verified. These are tool
validation results, not product-scale benchmark numbers.

## Earlier observations, kept separate

Earlier 4-CPU/8-GiB attempts did not complete. One was stopped for an account switch after
1,724,416 facts at 563.052 s and recorded a 1.538 ms star write. A later heartbeat-affected
attempt reached 3,584,000 facts at 1,713.746 s and recorded a 2.129 ms star write, with
rebuild-start load averages 1.00 / 1.03 / 1.00. Those values are not substituted into the
current run's table.

An earlier partial native capture observed 380 Intelligence-writer PASSIVE checkpoint calls
taking 3.456968 s and 381 idle TRUNCATE calls taking 0.430621 s. It included startup and only
part of a rebuild. It does not give the checkpoint share of a completed 14.4M rebuild.
The application requests PASSIVE after each batch and TRUNCATE when the writer queue is
empty; the current completed fact-stage samples additionally establish checkpoint work
inside Turso's commit path.

Vault-store's existing cipher selection is `aegis256`. The inspected AEGIS 0.9.20 fingerprint
enabled `default, pure-rust, softaes, std` with empty Rust flags. Its pure-Rust implementation
selects hardware AES by compile-time target features; the observed path used software AES.
Turso 0.8.1 also accepts `aes256gcm`, whose x86_64 AES/POLYVAL dependencies support runtime
AES-NI/PCLMULQDQ detection. AEGIS has compile-time AES-NI/ARM-crypto paths as well. No alternate
cipher or target-feature setting was applied, and no speedup for either option was measured.
