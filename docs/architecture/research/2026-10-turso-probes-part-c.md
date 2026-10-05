# Turso acceptance probes, part c (2026-10-05)

> Run by Codex (GPT 6 astra) for [ADR-012](../decisions/012-turso-storage-engine.md). Links to `src/`, `logs/` and `scripts/` point to the probe checkout, which is not kept in the repo.

# Turso 0.8.1 — one database-owning process

The one-process design removes the part-A `.tshm` overflow stall. Both WAL and MVCC completed the same 1M-visit / 1M-document transaction at a 14.4M-visit baseline. **The 1M MVCC transaction nevertheless exceeds the 8 GB memory target:** 8,679,243,776 resident bytes while held, 10,112,581,632 bytes at the process peak. Concurrency correctness and resource suitability are separate findings.

All runs are unencrypted and use stock cached Turso 0.8.1, built with Cargo offline. No external services, upstream drafts, commits, or product-code changes. The [part-C entry point](src/part_c.rs) excludes the three reserved modules. The [scenario builder](src/single_owner.rs) omits `experimental_multiprocess_wal`; the reused [seed helper](src/fixture.rs) explicitly disables it. Resource inventories contain no `.tshm` file. Every database connection for a scenario belongs to the same PID; the long backup has its own OS thread and independent connection.

## Verdicts

| Requested item                                               | Verdict                                                              | Evidence and consequence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------ | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Large-transaction stall, one owning process, WAL and MVCC | **PASS** for removing the part-A frame-limit stall                   | Both 1M-visit/1M-document transactions commit and subsequent reads remain correct. WAL reaches **309,333 frames**, exceeding the old 262,144-entry limit. The disabled multiprocess flag selects a different WAL coordination implementation. [WAL](logs/part-c-wal.jsonl), [MVCC](logs/part-c-mvcc.jsonl), [source](SOURCE-part-c.md#single-process-selection). No cutoff for that old failure appears through 1M; this does not remove memory limits.                                                                                                                                                                                                                                                                                   |
| 2. MVCC at scale with the specified single 1M transaction    | **FAIL** for the 8-GB product target; concurrency correctness passes | Disjoint star/note/tag/ordinary edits commit in **1.919 / 283.099 / 313.230 / 0.228 ms** while the backup is held; the overlapping edit loses in **0.156 ms**. Snapshot, exact rank, FTS and integrity checks pass. Big commit **24,588.945 ms**; peak RSS **10,112,581,632 bytes**. A separately tested 100k-batch policy reduces peak to **4,796,645,376 bytes**, but changes transaction/reader lifetimes. [Full transaction](logs/part-c-mvcc.jsonl), [bounded policy](logs/part-c-drained.jsonl), [code](src/bounded_owner.rs).                                                                                                                                                                                                      |
| 3. MVCC growth and checkpointing with active readers         | **PASS WITH WORKAROUND** for the tested bounded policy               | Default TRUNCATE cannot proceed with an active reader. PathKeep-owned policy: enable PASSIVE checkpoints, use **100k batches and short read transactions**, then drain readers and TRUNCATE between batches. Main log/WAL return to zero at each boundary; **1,894/1,894** monitored reads succeed, maximum **71.775 ms**. Admission pauses during **333.864–378.579 ms** quiescent truncations. The separate unbounded `TRUNCATE while active → PASSIVE` sequence has an unresolved live integrity-check timeout and is **not** counted as a pass. [Measured policy](logs/part-c-drained.jsonl), [checkpoint reader](src/checkpoint_watch.rs), [source](SOURCE-part-c.md#checkpoint-lock), [failed sequence](logs/part-c-passive.jsonl). |

## Measurement contract

- Baseline: **14,400,000 visits**, **3,600,000 page documents**, **2,865,791 pages with visits**; the same deterministic fixture, native ten-field FTS index, and bounded collector as part A. Each experiment starts from a closed/checkpointed copy of the baseline.
- Backup: one explicit transaction inserts visit IDs **14,400,001…15,400,000**, updates page documents **1…1,000,000**, and appends `backupmarker` to both indexed title fields. SQL is issued in 10,000-row statements, without commits between statements. A second database is attached and receives a backup record in the same transaction.
- An old reader has an explicit transaction **and a live cursor** from before the backup. Separate connections perform interactive edits, fresh reads, and checkpoints. Connections are obtained with `db.connect()`, never `Connection::clone()`.
- After commit: **15,400,000 visits**, **2,872,010 pages with visits**. All 1M marker IDs are checked, ranks are checked against an exhaustive sort with independently regenerated visit aggregates, and `PRAGMA integrity_check` must return `ok`.
- `synchronous=FULL`, 64-MiB page-cache setting per connection. WAL autocheckpoint is disabled. MVCC sets `mvcc_checkpoint_threshold=-1` so the measurement separates commit from explicit checkpoint; this policy is recorded in the logs. It is a measurement setting, not a recommendation to leave checkpointing disabled indefinitely.
- Host: macOS arm64, 18 logical CPUs, Rust 1.96.0; **not the 4-core/8-GB target**. Filesystem cache is not purged. Reported latency is measured on this host, not extrapolated to the target. Every timing records `uptime` load averages; every memory/file snapshot also records them.
- Memory is **whole-process resident memory**, including caches, reader snapshots, FTS and the oracle. [Own-task Mach measurements](src/probe_metrics.rs) report current/peak RSS; the supervisor independently records peak child RSS. These are not estimates of transaction-private heap allocations.

The research harness's `pass:true` records assert database correctness. They do not by themselves certify the product's RAM or interactive-latency requirements.

## 1. Large-transaction stall

| Operation                                 |                           Single-process WAL | Single-process MVCC, default checkpoint mode |
| ----------------------------------------- | -------------------------------------------: | -------------------------------------------: |
| Stage 1M visits + 1M page updates         |      **102,402.064 ms**; load 4.98/6.07/5.25 |      **350,961.338 ms**; load 3.27/4.90/4.92 |
| Commit big transaction                    |          **467.901 ms**; load 4.09/5.41/5.08 |       **24,588.945 ms**; load 4.21/4.33/4.60 |
| History during uncommitted backup         |            **0.594 ms**; load 4.19/5.45/5.10 |          **592.117 ms**; load 3.66/4.22/4.57 |
| Ranked top 100 during backup              |          **936.687 ms**; load 4.19/5.45/5.10 |        **1,663.795 ms**; load 3.66/4.22/4.57 |
| Old-snapshot history after commit         |            **0.128 ms**; load 4.09/5.41/5.08 |          **609.715 ms**; load 4.00/4.28/4.58 |
| Old-snapshot ranked top 100 after commit  |                    **678.821 ms**; same load |                  **1,344.181 ms**; same load |
| Fresh ranked top 100 after commit         |        **1,020.337 ms**; load 4.09/5.41/5.08 |        **2,312.291 ms**; load 4.00/4.27/4.57 |
| Truncate checkpoint after readers release |          **431.519 ms**; load 3.84/5.34/5.06 |       **54,673.235 ms**; load 3.69/4.20/4.54 |
| Integrity check                           | **37,819.131 ms**, `ok`; load 3.84/5.34/5.06 | **65,094.683 ms**, `ok`; load 3.00/3.91/4.41 |
| Whole scenario                            |                        **155.140 s**, exit 0 |                        **530.911 s**, exit 0 |

Evidence: [WAL log](logs/part-c-wal.jsonl), [MVCC log](logs/part-c-mvcc.jsonl), [scenario code](src/single_owner.rs), [collector and independent exhaustive check](src/search.rs).

The WAL reaches **1,274,451,992 bytes = 309,333 frames** at 4,096-byte pages, explicitly crossing part A's 262,144-entry threshold. Reads still finish; no restart/reopen is needed. The MVCC logical log reaches **452,164,562 bytes**, and its physical WAL is zero before the explicit checkpoint. Neither run hits the part-A overflow/retry failure. There is no 262,144-frame transaction boundary to binary-search in this configuration; **1M visits plus 1M document updates is the largest full transaction tested**, not a universal maximum safe transaction size.

Why: `database.rs:2926–2927` returns no shared authority when multiprocess WAL is disabled. `storage/wal.rs:4810–4818` constructs `InProcessWalCoordination`, whose frame cache is a dynamically allocated `HashMap<u64, Vec<u64>>` (`wal.rs:2937`), not the bounded `.tshm` index. The overflow refusal from part A belongs to `ShmWalCoordination`, which this configuration does not instantiate. [Source excerpts](SOURCE-part-c.md#single-process-selection), [in-process WAL](SOURCE-part-c.md#in-process-wal).

This removes that particular failure, **not all long operations**. MVCC history reads during the large transaction are about 0.6 s, full ranked reads are 1.3–2.3 s, commit is 24.6 s, and the default checkpoint is 54.7 s. No stopped-progress/deadlock or supervisor timeout occurred. The default checkpoint holds a database-wide checkpoint gate; fresh transactions cannot freely start while it holds that gate. The source establishes that limitation; the separate passive experiment below measures reads during checkpoint work.

## 2. MVCC concurrency, snapshots, conflicts and RAM

All following small writes were attempted while the complete **1M-visit/1M-document transaction remained open**. The backup thread was not released to commit until these requests and their visibility checks finished.

| Small write               |                       Target page | Result                                                        |  Total latency | Load 1/5/15    |
| ------------------------- | --------------------------------: | ------------------------------------------------------------- | -------------: | -------------- |
| Star                      |                         3,599,991 | Committed                                                     |   **1.919 ms** | 4.41/4.36/4.62 |
| Note                      |                         3,599,992 | Committed; native FTS immediately finds `notemarker`          | **283.099 ms** | 4.41/4.36/4.62 |
| Tag                       |                         3,599,993 | Committed; native FTS immediately finds `tagmarker`           | **313.230 ms** | 4.41/4.36/4.62 |
| Ordinary `preview` column |                         3,599,994 | Committed                                                     |   **0.228 ms** | 4.41/4.36/4.62 |
| Overlapping star          | **42**, already updated by backup | **`Write-write conflict` at UPDATE; small transaction loses** |   **0.156 ms** | 4.41/4.36/4.62 |

The four disjoint edits completed without waiting for the backup to commit. This does not mean zero execution or fsync latency; note/tag work exceeds 250 ms but successfully commits while the backup is still held. The overlapping edit is rolled back; page 42 retains `star=0`, its backup title change survives, and the big transaction subsequently commits. WAL controls reject all five edits with Busy after the configured timeout; one owning process alone does not make WAL a multiwriter engine.

The row-conflict cause is `mvcc/database/mod.rs:5560–5572,5660–5684`: an update deletes/inserts a row version, and deletion checks visibility and an existing conflicting transaction before modifying that version. The second writer to page 42 loses at the update, rather than waiting for a file-wide writer lock. [Source](SOURCE-part-c.md#write-conflict).

Snapshot checks in [the log](logs/part-c-mvcc.jsonl):

- Before and during the backup, both old/fresh readers count exactly 14.4M committed visits; FTS returns no uncommitted `backupmarker` IDs. History and ranked results equal the baseline.
- After small commits but before backup commit, a fresh reader sees the note and tag; the pinned old reader sees neither. Its history/ranking remain unchanged.
- After big commit, the pinned reader still counts 14.4M and sees no backup markers. A fresh reader counts 15.4M and sees **every marker ID 1…1,000,000 exactly once**.
- The fresh collector's top 100 matches the independent exhaustive sort, including newest-visit ties and counts. The live old cursor remains valid across checkpoint attempts and advances from visit ID 1 to ID 2.
- After releasing the reader and checkpointing, fresh reads and the million-ID FTS check pass again. The integrity check is `ok`.

### Memory measured across the held transaction

Current RSS, in bytes; each row has its own recorded load in [resource records](logs/part-c-mvcc.jsonl). This is process residency, not a computed allocation budget.

| Phase                           | Resident bytes |   GiB | Load 1/5/15    |
| ------------------------------- | -------------: | ----: | -------------- |
| before-backup                   |    878,182,400 | 0.818 | 3.27/4.90/4.92 |
| staged-100000                   |  3,270,754,304 | 3.046 | 2.88/4.65/4.83 |
| staged-200000                   |  4,033,806,336 | 3.757 | 3.19/4.56/4.78 |
| staged-300000                   |  4,660,396,032 | 4.340 | 3.44/4.51/4.76 |
| staged-400000                   |  5,243,060,224 | 4.883 | 4.31/4.60/4.78 |
| staged-500000                   |  5,777,555,456 | 5.381 | 4.13/4.49/4.73 |
| staged-600000                   |  6,451,822,592 | 6.009 | 3.85/4.39/4.68 |
| staged-700000                   |  7,037,239,296 | 6.554 | 3.89/4.33/4.65 |
| staged-800000                   |  7,531,200,512 | 7.014 | 5.29/4.59/4.73 |
| staged-900000                   |  8,061,845,504 | 7.508 | 4.08/4.36/4.64 |
| staged-1000000                  |  8,679,243,776 | 8.083 | 3.66/4.22/4.57 |
| after-small-writes              |  9,128,394,752 | 8.501 | 4.21/4.33/4.60 |
| after-backup-commit             |  9,788,178,432 | 9.116 | 4.00/4.28/4.58 |
| readers-released-TRUNCATE-after | 10,112,516,096 | 9.418 | 3.00/3.91/4.41 |

The MVCC process peaks at **10,112,581,632 bytes (9.418 GiB)**, versus **4,546,428,928 bytes (4.234 GiB)** for the WAL control. Even the process at the end of staging, before subsequent fresh-reader/checkpoint work, uses **8,679,243,776 bytes (8.083 GiB)** for the whole process. Thus the exact 1M-transaction configuration cannot be certified for an 8-GB machine; it already exceeds 8 GiB without counting the OS or frontend.

The mechanism is visible in `mvcc/database/mod.rs`: row versions retain record bytes, transaction write sets retain the row/version chains, and commit constructs a serialized logical-log record (`:352–367,965–982,1010–1025,1083–1102,3396–3425`). Uncommitted work remains in memory; its logical log is not a mechanism for spilling an uncommitted transaction. [Source](SOURCE-part-c.md#mvcc-memory). The bounded-transaction experiment below tests an application-owned alternative; changing the transaction size also changes backup atomic visibility and must not be hidden from a product decision.

## 3. Checkpointing and file growth

Default MVCC checkpoint behavior with `experimental_mvcc_passive_checkpoint` disabled:

| State                                         |    Database bytes | Logical-log bytes | Physical WAL bytes | Observation                                                          |
| --------------------------------------------- | ----------------: | ----------------: | -----------------: | -------------------------------------------------------------------- |
| Before backup                                 |     2,384,220,160 |                 0 |                  0 | Baseline checkpoint finished.                                        |
| 1M visits/documents staged, uncommitted       |     2,384,220,160 |                 0 |                  0 | Changes reside in MVCC memory.                                       |
| Four small edits committed; backup still open |     2,384,220,160 |             6,457 |                  0 | Only small commits are durable in the logical log.                   |
| Big transaction committed                     |     2,384,220,160 |   **452,164,562** |                  0 | Database file still unchanged.                                       |
| Checkpoint attempts with old reader active    |     2,384,220,160 |       452,164,562 |                  0 | TRUNCATE returns Busy; PASSIVE is rejected without its feature flag. |
| Readers released; TRUNCATE finished           | **2,582,872,064** |             **0** |              **0** | Database grows by **198,651,904 bytes**; log/WAL reclaimed.          |

TRUNCATE with backup and reader active returns Busy in **250.096 ms**, load **4.21/4.33/4.60**. PASSIVE without the feature flag returns `PASSIVE checkpoint requires experimental_mvcc_passive_checkpoint` in **0.017 ms**, same load. It is not a successful checkpoint or a fallback to an online checkpoint. The exact after-commit attempts and file inventories are in [the MVCC log](logs/part-c-mvcc.jsonl).

Root cause: default MVCC transactions hold the checkpoint gate in read mode for their lifetime (`mvcc/database/mod.rs:6624–6627,6646`). A default checkpoint must acquire it exclusively (`checkpoint_state_machine.rs:2084–2089`), so an active reader prevents it. After the reader leaves, that exclusive gate also prevents new transactions from entering freely during the long checkpoint. [Reader gate](SOURCE-part-c.md#reader-checkpoint-lock), [checkpoint gate](SOURCE-part-c.md#checkpoint-lock).

### Experimental PASSIVE checkpoint with a long-lived reader

A separate fresh fixture enables the public builder option `experimental_mvcc_passive_checkpoint(true)`; multiprocess WAL remains disabled. [Code](src/single_owner.rs), [full log](logs/part-c-passive.jsonl).

The deliberately tested sequence is important:

1. With the old reader still active **after the 1M commit**, TRUNCATE performs **52,569.763 ms** of work, then returns Busy. Load **3.87/4.17/4.36**. The database is unchanged; the physical WAL grows to **1,163,595,152 bytes** while the logical log remains **452,161,758 bytes**.
2. A subsequent PASSIVE call finishes the remaining work in **887.472 ms**, load **4.22/4.26/4.38**. It returns **`(0,282426,0)`**: not busy, but **zero of 282,426 physical WAL frames backfilled**. The logical log becomes zero while the physical WAL remains 1,163,595,152 bytes. This also crosses the former 262,144-frame limit without that failure.
3. Both independent history requests issued during that PASSIVE call succeed; maximum **698.510 ms**. The watch window loads are **4.22/4.26/4.38 → 5.00/4.42/4.44**. The full old-reader history/rank/count still equal the original snapshot, and its live cursor remains valid.
4. After releasing that reader, TRUNCATE takes **1,703.078 ms**, load **5.00/4.42/4.44**, and leaves **2,582,880,256 database bytes**, zero logical log, and zero physical WAL.

**887 ms is not the cost of a full 1M PASSIVE checkpoint.** The preceding unsuccessful TRUNCATE already did substantial materialization work. For direct PASSIVE timings, use the separate batch experiments below. Enabling the feature alone does not make TRUNCATE an online operation.

This run has a separate failure: the following live-process `PRAGMA integrity_check` never returned before the **1,200.115 s whole-process watchdog** killed the probe. Its last start record is at **18:35**, the supervisor exit at **18:47**; there is no invented completion duration. [Timeout log](logs/part-c-passive.jsonl). Attempted stack sampling and early phase termination were denied by the sandbox; the original supervisor performed the eventual termination. [Phase-watchdog evidence](logs/part-c-passive-watchdog.json).

The persisted data was then verified by a new **sole** owner, after the prior process had exited:

| Fresh-owner check of the same persisted file | Result                                                                  | Integrity timing and load         |
| -------------------------------------------- | ----------------------------------------------------------------------- | --------------------------------- |
| Passive feature disabled                     | All 15.4M visits, all 1M FTS IDs, exhaustive top 100 and integrity pass | **64,840.989 ms**, 5.91/6.40/5.30 |
| Passive feature still enabled                | Same checks pass                                                        | **64,316.642 ms**, 4.43/5.54/5.09 |

[Reopen code](src/reopen_owner.rs), [default reopen log](logs/part-c-passive-reopened.jsonl), [passive reopen log](logs/part-c-passive-reopened-flag.jsonl). The reopen checks are sequential; two processes never open the same fixture concurrently.

This narrows the unexpected integrity delay to the prior live-process state or its interaction with checking, rather than demonstrating persistent corruption or a feature-flag-only failure. **The exact internal cause of that delay is not established.** The relevant integrity path selects the shared MVCC schema and physical roots, runs structural checks, then row/index checks (`translate/integrity_check.rs:80–98,216–226,262–270`; [source](SOURCE-part-c.md#integrity-path)). Source pointers are not claimed to be a stack trace. The complete live sequence above is retained as the reproduction and remains a failed validation case.

PASSIVE's storage behavior is explained by `checkpoint_state_machine.rs:2811–2824,2954–2985,3020–3046`: it can publish materialized data into the WAL, truncate the covered logical log, and finish without backfill when a reader pins the old DB-file view. It preserves versions needed by old snapshots. [Source](SOURCE-part-c.md#passive-publish). Default mode rejects PASSIVE at `translate/pragma.rs:945` unless the builder feature is enabled ([source](SOURCE-part-c.md#passive-pragma-gate)).

### Application-owned bounded batches and reader quiescence

The [bounded-batch code](src/bounded_owner.rs) adds the same total **1M visits and 1M FTS document updates** as ten **100,000-row transactions**, with four disjoint interactive commits and one expected overlapping conflict in every batch. Each batch checks the old committed count while its backup is open, the new count after commit/checkpoint, and all accumulated FTS IDs. The final full-stream collector is again compared with the independent exhaustive sort.

Two policies were measured:

| Policy                                                                                                                                                                 |                    Peak process RSS | Outcome                                                                                                             |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------: | ------------------------------------------------------------------------------------------------------------------- |
| Commit each 100k batch; direct PASSIVE checkpoint with concurrent history requests; TRUNCATE only after all ten batches                                                | **8,762,474,496 bytes (8.161 GiB)** | All data/rank/integrity checks pass, but memory still exceeds an 8-GiB machine before OS/frontend memory.           |
| Commit each 100k batch; direct PASSIVE checkpoint while history requests continue; finish readers, pause new request admission, and TRUNCATE **before the next batch** | **4,796,645,376 bytes (4.467 GiB)** | All data/rank/integrity checks pass; both main logical log and main WAL are zero at every completed batch boundary. |

[PASSIVE-only batch log](logs/part-c-batched.jsonl), [reader-drain batch log](logs/part-c-drained.jsonl), [concurrent checkpoint reader](src/checkpoint_watch.rs). Both policies commit all **40 disjoint edits** and reject all **10 conflicting edits** without aborting the backup batches.

Batches alone are not a memory bound when backfill is deferred. `checkpoint_state_machine.rs:3045–3074` publishes a backfill floor and explicitly retains versions that have not been backfilled for readers that can still need them. [Source](SOURCE-part-c.md#backfill-and-gc). Process RSS also includes allocator/cache retention, so these measurements do not attribute every retained byte to one structure. The second policy tests the physical-reclamation boundary rather than assuming a successful PASSIVE result means the WAL was reclaimed.

Measured latency ranges across the ten batches (milliseconds). Each endpoint's load is retained below; complete per-batch timings remain in the logs.

| Operation                       | PASSIVE-only batches: min–max ms | Reader-drain batches: min–max ms | Reader-drain loads at min → max (1/5/15) |
| ------------------------------- | -------------------------------: | -------------------------------: | ---------------------------------------- |
| Commit 100k                     |                  275.094–301.182 |                  272.831–301.493 | 3.36/4.02/4.42 → 2.25/3.41/4.11          |
| Direct PASSIVE checkpoint       |              3,275.345–9,052.886 |              3,235.613–8,281.042 | 2.92/3.41/4.06 → 2.96/3.86/4.34          |
| Quiescent TRUNCATE              |                                — |                  333.864–378.579 | 3.08/3.48/4.10 → 2.23/3.39/4.09          |
| History while batch open        |                   53.118–628.344 |                    52.043–59.567 | 3.36/4.02/4.42 → 2.79/3.60/4.19          |
| Ranked top 100 while batch open |              1,387.589–2,810.887 |              1,224.005–1,631.005 | 3.36/4.02/4.42 → 2.83/3.40/4.06          |
| History after checkpoint        |                   58.714–651.702 |                      1.199–1.387 | 2.62/3.92/4.39 → 3.17/3.78/4.29          |
| Ranked top 100 after checkpoint |              1,398.898–2,936.397 |              1,039.119–1,479.045 | 3.40/3.93/4.36 → 2.92/3.41/4.06          |
| Star                            |                      1.417–2.661 |                      1.565–3.913 | 2.83/3.40/4.06 → 2.73/3.98/4.42          |
| Note                            |                195.681–5,477.978 |                190.568–1,930.510 | 2.73/3.98/4.42 → 2.62/3.41/4.08          |
| Tag                             |                154.161–1,476.055 |                  151.453–492.011 | 2.67/3.95/4.41 → 2.83/3.40/4.06          |
| Ordinary row                    |                      0.177–0.271 |                      0.175–0.214 | 3.35/3.83/4.31 → 2.62/3.41/4.08          |
| Overlapping conflict            |                      0.076–0.114 |                      0.074–0.110 | 2.83/3.40/4.06 → 2.93/3.80/4.31          |

During direct PASSIVE checkpoints, the history monitor sees:

- PASSIVE-only policy: **434/434 successful requests**, maximum **679.300 ms**; worst watch-window load **3.85/4.24/4.57 → 4.50/4.37/4.61**.
- Reader-drain policy: **1,894/1,894 successful requests**, maximum **71.775 ms**; worst watch-window load **2.57/3.38/4.07 → 3.08/3.48/4.10**.

Each monitor record includes its window's starting/ending uptime and every individual latency/outcome. No Busy replies or unexpected stalls occurred in those monitored requests. This is finite measured evidence, **not a guarantee that every request is instantaneous**: the quiescent TRUNCATE explicitly pauses admission for **333.864–378.579 ms** on this host, ranked searches still take **1.0–1.6 s**, and a note edit reaches **1,930.510 ms**. The owner must keep that work off the frontend thread and expose progress/admission status. New requests were not admitted during the quiescent phase; those pauses must not be hidden from a product design.

PASSIVE-only batch integrity passes in **69,681.904 ms**, load **3.65/4.18/4.54**. Reader-drain batch integrity passes in **66,972.796 ms**, load **2.78/3.36/4.03**. The final collector and FTS checks cover all 15.4M visits and all 1M updates. The reader-drain policy's peak includes the final exhaustive oracle, not just staging. A subsequent fresh sole owner, with the passive feature still enabled, verifies all persisted visits/FTS IDs and ranks again; integrity is `ok` in **62,187.539 ms**, load **3.17/3.16/3.69**. [Fresh-owner evidence](logs/part-c-drained-reopened.jsonl).

The validated application policy is therefore:

```text
one owning process; independent connection per transaction owner
→ BEGIN CONCURRENT, stage at most the tested 100k fixture rows, COMMIT
→ PASSIVE checkpoint while short read requests continue
→ stop admission briefly and finish existing read transactions
→ TRUNCATE checkpoint, check its result and actual file sizes
→ resume requests and start the next batch
```

This policy lives in PathKeep-owned scheduling/transaction code and builder configuration; no Turso fork is used. It depends on **short-lived reader transactions**. A long export pinned across the entire import can prevent the quiescent boundary and retain old versions; this test does not promise bounded memory with such a reader left open indefinitely. It also exposes each committed batch rather than making the whole million-row backup atomic. A whole-backup visibility contract would need a separately validated publication mechanism, not an undocumented assumption.

**100k is a demonstrated bound for this fixture, not a discovered maximum safe transaction size.** The requested 262,144-frame failure did not recur through the full 1M WAL/MVCC transactions, so its conditional cutoff/binary search does not apply. The extra batch experiments address the separately measured RAM and checkpoint-admission problems. They do not certify an arbitrary future data shape or a full Tauri app on target hardware.

## Reproduce and verify

The preserved entry points are [part_c.rs](src/part_c.rs), [single_owner.rs](src/single_owner.rs), [bounded_owner.rs](src/bounded_owner.rs), [checkpoint_watch.rs](src/checkpoint_watch.rs), [memory/file metrics](src/probe_metrics.rs), and [fresh-owner verification](src/reopen_owner.rs). The fixture and collector reuse [fixture.rs](src/fixture.rs) and [search.rs](src/search.rs).

```sh
cd research/turso-probes
export CARGO_HOME="$PWD/.work/cargo-home"
export CARGO_NET_OFFLINE=true
cargo build --offline --locked --release --bin part-c
export PROBE_BINARY=target/release/part-c
PROBE_LOG=replay-c-seed python3 scripts/run_probe.py seed replay-c-base
# Run one copied fixture at a time; the supervisor limits total data-directory bytes.
# Choose: wal, mvcc, mvcc-passive, mvcc-passive-batched, or mvcc-drained-batches.
probe_mode=mvcc-drained-batches
probe_fixture="replay-c-$probe_mode"
mkdir -p "data/$probe_fixture"
cp -c data/replay-c-base/archive.db "data/$probe_fixture/archive.db"
PROBE_LOG="$probe_fixture" PROBE_TIMEOUT=1200 \
  python3 scripts/run_probe.py "$probe_mode" "$probe_fixture" 100000
# Only after its supervisor exits; the full passive sequence has an expected timeout.
PROBE_LOG="$probe_fixture-reopened" PROBE_TIMEOUT=300 PROBE_IDLE_TIMEOUT=180 \
  python3 scripts/run_probe.py reopen-passive "$probe_fixture"
# After reviewing the retained logs, remove that disposable copy before the next case.
rm -rf -- "data/$probe_fixture"
```

Run scenarios sequentially, reclaim completed scenario data before making another large copy if necessary, and use fresh directories. The supervisor retains exit status, timing, load, peak child RSS and disk usage. `PROBE_IDLE_TIMEOUT` is an explicit no-output watchdog; a timeout is not asserted to prove a deadlock. The early `complete` field in WAL/default-MVCC checkpoint logs means the result was not busy; the later logs distinguish `not_busy` from `all_frames_backfilled`.

[verify_part_c.py](scripts/verify_part_c.py) checks single-process ownership, absence of `.tshm`, exact counts/ranks, conflict outcomes, expected timeouts and completed batch checks from retained evidence. [Verification output](logs/part-c-verification.json). [summarize_part_c.py](scripts/summarize_part_c.py) generates the compact [measurement inventory](logs/part-c-summary.json). [Source excerpts](SOURCE-part-c.md) preserve release locations and hashes; no external lookup was performed.

Validation used the standalone offline release build and real-engine probes. The product `bun run check` was not run because this task is confined to research files and changes no product code. Final disposal of `data/` and `target/` is recorded in [part-c-cleanup.json](logs/part-c-cleanup.json); probe code, reports, logs and source evidence remain.
