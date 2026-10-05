# Turso acceptance probes, part a (2026-10-05)

> Run by Codex (GPT 6 astra) for [ADR-012](../decisions/012-turso-storage-engine.md). Links to `src/`, `logs/` and `scripts/` point to the probe checkout, which is not kept in the repo.

# Turso 0.8.1 — part A results

All new runs used **unencrypted, synthetic databases**, stock Turso `0.8.1`, and native `USING fts` indexes. No external service was contacted, no commit was made, and no product code was changed. The separate [part-A entry point](src/part_a.rs) neither imports nor executes `encryption.rs`, `crash.rs`, or `spill.rs`; those modules were not opened or edited.

The published crate archives match `Cargo.lock`, and the extracted Turso, SDK, Tantivy, and query-grammar sources match the archives byte for byte: [environment and source hashes](logs/part-a-environment.json). The previous patched-interrupt experiment is **not** accepted as a workaround: it changes the dependency, not PathKeep's code.

## Verdict table

`PASS WITH WORKAROUND` means the demonstrated adjustment is entirely in PathKeep-owned Rust/query code. `FAIL` means at least one required behavior is unavailable or fails; a recovery procedure does not erase that failure.

| Item                                                            | Verdict                                           | Evidence, numbers, and cause                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| --------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1. Full-text query language                                     | **PASS WITH WORKAROUND**                          | Bare `topi*` returns 0 of 4 expected documents; `42*` returns 1 of 4. Compile normalized prefixes into field-scoped term ranges: all expected IDs pass in **0.079 / 0.068 ms**. Multi-term AND with a last-term prefix passes with the same builder. Exact phrase, exclusion, ten-field weights, typo trigrams and Chinese trigrams pass. The lenient Tantivy parser consumes bare `*` as text; quoting reaches a phrase-prefix restriction requiring at least two terms. [Code](src/language.rs), [query builder](src/common.rs), [log](logs/part-a-language-complete.jsonl), [source](SOURCE-part-a.md#prefix).                                                                                                                                                                                                                                                                                                                                                        |
| 2. Exact ranked pages at 14.4M visits                           | **PASS WITH WORKAROUND**                          | The full-stream Rust heap matches an exhaustive sort for both pages of **all seven queries**, with exact page/visit counts. Baseline: **3,600,000 page documents; 2,865,791 visited pages; 14,400,000 visits**. `topic` first page: **852.980 ms cold / 261.982 ms warm**; separately cold next page **1,454.108 ms**. SQL composite order returns **[3,2,1]**, expected **[1,3,2]**: projected scores use the FTS cursor, but the sort expression calls a scalar fallback returning zero. [Collector/oracle](src/search.rs), [minimal reproduction](src/regressions.rs), [ordering log](logs/part-a-ordering-complete.jsonl), [verification](logs/part-a-retained-verification.json), [source](SOURCE-part-a.md#ranking-optimizer).                                                                                                                                                                                                                                     |
| 3. Concurrent writes/reads, FTS + ATTACH + MVCC + two processes | **FAIL**                                          | This combination is explicitly rejected: MVCC has no cross-process coordination. Rejection **0.043 ms** at 14.4M; WAL `BEGIN CONCURRENT` also fails. Under supported multiprocess WAL, all overlapping small writes fail after **250.120–250.340 ms**; history reads **0.164–0.175 ms**, correct ranked reads **887.712–957.595 ms**. The 1M-visit/1M-document transaction stages in **120,216.645 ms**, commits in **674.726 ms**, then the existing reader stalls and fails. Reopening verifies **1M FTS IDs and 15.4M total visits**. Small single-process MVCC disjoint commit/conflict: **1.256 / 0.100 ms**. [Code](src/concurrency.rs), [gate](logs/part-a-mvcc-gate.jsonl), [scale run](logs/part-a-wal-scenario.jsonl), [reopen verification](logs/part-a-backup-reopened.jsonl), [MVCC control](logs/part-a-mvcc-small.jsonl), [source](SOURCE-part-a.md#mvcc-multiprocess).                                                                                   |
| 4. Post-index ATTACH / first-search stall                       | **FAIL**                                          | Reproduced with 3.6M documents/two FTS indexes; the **130.064 s** supervisor deadline stops the ATTACH retry loop. The `.tshm` frame index holds **262,144 entries**; the log shows **266,278 frames** and a refused live fallback-cache refresh. A smaller reproduction **without FTS or ATTACH** produces a **1,097,160,152-byte WAL**: `SELECT … LIMIT 1` returns `Busy` after **10,483.325 ms**, despite busy timeout 0. Checkpoint returns a busy result; checkpoints between FTS builds also fail to prevent the stall. Fresh-process reopen + checkpoint restores ATTACH/search (**0.286 / 119.556 ms**); this is recovery, not a general live-operation fix. [Minimal code](src/wal_limit.rs), [minimal log](logs/part-a-wal-limit-minimal.jsonl), [FTS reproduction](logs/part-a-lifecycle-full.jsonl), [checkpoint attempt](logs/part-a-lifecycle-checkpoints.jsonl), [recovery](logs/part-a-lifecycle-reopen.jsonl), [source](SOURCE-part-a.md#wal-capacity). |
| 5. Rust API                                                     | **FAIL** for the required running-query interrupt | Public `turso::Connection` has no interrupt method; the SDK method is behind a private handle. [Stock compile failure](logs/interrupt-stock-compile.log), [reproducer](compile-fail/interrupt.rs), [source](SOURCE-part-a.md#interrupt-private-sdk). `INSERT … RETURNING` and busy handling pass. An owned connection thread with a synchronous request/reply facade is recommended for blocking Tauri commands. Cooperative export cancellation between 1,024-row batches passes (**0.357 ms** after cancellation, **580,608 rows** traversed), but cannot abort an in-flight statement. [API/worker code](src/api.rs), [API log](logs/part-a-api-complete.jsonl), [worker log](logs/part-a-worker.jsonl).                                                                                                                                                                                                                                                              |
| 6. Live browser snapshots and one-binary coexistence            | **PASS WITH WORKAROUND**                          | Turso live SQLite snapshots fail under WAL recycling: SQLite checkpoints successfully while Turso has a read transaction; the next read gets a **short WAL-frame read**. SQLite's control reader prevents truncation and retains its snapshot. Use **rusqlite 0.39.0 with plain bundled SQLite + online backup** to read browser files, then Turso for owned databases. Chrome/Firefox committed-WAL snapshots both pass (**0.592 / 0.591 ms**), excluding the uncommitted row. Both engines build, link and run in one macOS arm64 executable: **22,497,408 bytes stripped (21.455 MiB)**; **29,080,136 bytes unstripped**. [Code](src/browser.rs), [WAL failure/control](logs/part-a-browser-recycle-fixed.jsonl), [browser log](logs/part-a-browser.jsonl), [binary/link evidence](logs/part-a-binary.json), [stripped-binary run](logs/part-a-browser-stripped.jsonl).                                                                                               |

## Measurement conditions

The host is macOS arm64, **18 logical CPUs**, Rust 1.96.0. These runs demonstrate behavior at the target data cardinality; they do **not** certify performance on a 4-core/8-GB target. Memory capacity could not be read through the sandbox. Some independent probes and unrelated machine activity overlapped. Every measured operation records `uptime`'s 1/5/15-minute load averages in its JSONL record; comparisons must account for them.

“Cold” means a **new OS process**, not a purged OS filesystem cache. First-page timing includes eligibility construction and the first FTS use; database open is separately recorded. The isolated next/count runs also start new processes and include eligibility construction. Their warm repetitions reuse the connection, index cache, eligibility, and read-transaction snapshot. No eligibility cache is claimed safe across changes or different snapshots.

The legacy search harness calls its initial three-request sequence `cold-process`; its initial next/count operations are already warmed by the first request. The table below deliberately uses the additional `search-isolated` processes for truly cold next/count requests. First-page results come from `part-a-search-*`; next/count results come from `part-a-isolated-*`.

The fixture follows the existing [deterministic generator](src/fixture.rs): 20,000 domains, three profiles, five years of visits, ten search fields, and one document per page ID. There are **734,209 documents without visits**, excluded from results but present in the scoring corpus. Their inclusion is explicit, not counted as visited pages. Baseline seeding took **10,203.533 ms** for visits, **5,472.977 ms** for documents, and **32,202.588 ms** for FTS ([seed log](logs/part-a-seed.jsonl)); peak child RSS was **2,639,413,248 bytes**. The later concurrency run adds 1M visits only after the baseline search measurements.

## 1. Query language details

All values below are from [the unencrypted language run](logs/part-a-language-complete.jsonl), load **2.59 / 3.71 / 4.85**. Inputs are already normalized tokens/text; this probe does not reimplement the product's Unicode normalization contract.

| Subitem                     | Verdict              | Actual result and milliseconds                                                                                                                                                                                                                   |
| --------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `topi*`                     | PASS WITH WORKAROUND | Native `[]` (**0.375**), expected IDs `[1,2,3,5]` including `topic`, `topics`, `topical`. Range gives all four (**0.079**).                                                                                                                      |
| `42*`                       | PASS WITH WORKAROUND | Native `[1]` (**0.110**), expected `[1,2,3,4]` including `421` and `420`. Range gives all four (**0.068**).                                                                                                                                      |
| `alpha AND topi*`           | PASS WITH WORKAROUND | Native `[]` (**0.088**); `alpha AND (body:[topi TO topj})` passes `[1,2,3,5]`.                                                                                                                                                                   |
| `"alpha beta"`              | PASS                 | `[1,4,5]`, **0.090**; excludes reversed tokens and `alpha unrelated beta`.                                                                                                                                                                       |
| `topic AND -blocked`        | PASS                 | `[1]`, **0.060**.                                                                                                                                                                                                                                |
| Ten-field weights           | PASS                 | Equal-length/equal-frequency documents, weights 1 through 10, expected and actual rank `[10,9,8,7,6,5,4,3,2,1]`, **0.191**.                                                                                                                      |
| Typo 3-grams                | PASS WITH WORKAROUND | PathKeep splits `browseer` into `bro OR row OR ows OR wse OR see OR eer`; native `tokenizer='ngram',min_gram=3,max_gram=3` retrieves `browser` only in this fixture, **0.115**. This is candidate recall, not a guaranteed edit-distance metric. |
| Unsegmented Chinese 3-grams | PASS WITH WORKAROUND | `设计系 AND 计系统` retrieves `设计系统资料` only, **0.059**. Query windows are formed in Rust; strings shorter than three characters require a separate product policy/index, not claimed covered here.                                         |

The prefix cause is precise: Turso calls `parse_query_lenient` at `turso_core/index_method/fts/mod.rs:3114`. Tantivy grammar `term_or_phrase_infallible` passes `")^"` as its bare-word delimiters (`query_grammar.rs:246`); `word_infallible` therefore consumes the trailing `*`. The simple tokenizer discards that punctuation, yielding exact `topi`/`42`. Quoting (`"topi"*`) reaches prefix parsing, but `generate_literals_for_str` rejects a one-token phrase (`query_parser.rs:974`); the probe captures that error. [Exact source excerpts](SOURCE-part-a.md#prefix-grammar).

The cleanest demonstrated PathKeep workaround is [the existing `prefix_query`](src/common.rs): for an already-normalized alphanumeric token, form a half-open lexicographic term range, such as `body:[42 TO 43}`, OR it across the allowed fields, and AND it with previous terms. It introduces no expansion-count ceiling, no extra index, and no dependency patch. Do not pass arbitrary raw query text into this builder; validate the token and use trusted schema field names. Prefix range scoring is Tantivy range-query scoring, not a claim that expanded terms receive exactly the BM25 score of a literal term. Ranking below checks the actual native `fts_score` contract.

## 2. Ranked full-archive results

The request order is score descending, then **newest eligible visit** descending, then page ID descending for deterministic remaining ties. Last-term prefix semantics apply to `topic 42` and `topic 421`; the builder expands across all ten fields, including page numbers in URLs. The rare query is `page AND 1200001`.

The collector walks the **entire** unordered native FTS result stream, filters eligibility, and keeps at most 100/200 hits for page one/two. It never applies a recent-window or lexical-candidate cap. Exact counts walk the same complete stream. The heap is O(k); the probe's dense eligibility array is **57,600,016 bytes** (3,600,001 × 16), in addition to engine caches. The exhaustive oracle independently regenerates all visits and lexical membership, checks every matching ID for omissions/duplicates, materializes all eligible hits, and fully sorts them. It shares native score values and the stated ordering contract, not the bounded selection algorithm. Both pages and both counts agree.

The [retained-evidence verifier](scripts/verify_part_a.py) also compares every cold result against the verified warm result, isolated next/count results against the oracle, and all three concurrent ranked reads against the baseline top 100. [Verification output](logs/part-a-retained-verification.json).

Each timing cell is **milliseconds (load 1/5/15)**. Open timings are separate in the linked logs.

| Query                                         |   Exact pages / visits | Cold first                 | Cold next                  | Cold count               | Warm first               | Warm next                | Warm count               |
| --------------------------------------------- | ---------------------: | -------------------------- | -------------------------- | ------------------------ | ------------------------ | ------------------------ | ------------------------ |
| [topic](logs/part-a-search-topic.jsonl)       | 2,865,791 / 14,400,000 | 852.980 (3.44/4.74/5.90)   | 1,454.108 (9.93/5.91/6.09) | 852.687 (9.93/5.91/6.09) | 261.982 (3.44/4.74/5.90) | 259.333 (9.93/5.91/6.09) | 251.008 (9.93/5.91/6.09) |
| [topic42](logs/part-a-search-topic42.jsonl)   |       73,731 / 388,018 | 626.858 (3.44/4.74/5.90)   | 622.671 (9.93/5.91/6.09)   | 620.489 (9.38/5.86/6.07) | 29.372 (3.48/4.73/5.89)  | 28.272 (9.38/5.86/6.07)  | 27.990 (9.38/5.86/6.07)  |
| [topic421](logs/part-a-search-topic421.jsonl) |         6,853 / 37,018 | 608.283 (3.48/4.73/5.89)   | 610.020 (9.38/5.86/6.07)   | 616.288 (9.38/5.86/6.07) | 17.164 (3.48/4.73/5.89)  | 16.877 (9.38/5.86/6.07)  | 16.792 (9.38/5.86/6.07)  |
| [rare](logs/part-a-search-rare.jsonl)         |                  1 / 7 | 600.799 (3.48/4.73/5.89)   | 594.571 (9.38/5.86/6.07)   | 594.865 (9.38/5.86/6.07) | 2.804 (3.48/4.73/5.89)   | 2.748 (9.38/5.86/6.07)   | 2.745 (9.38/5.86/6.07)   |
| [last7](logs/part-a-search-last7.jsonl)       |        54,577 / 55,232 | 429.911 (3.48/4.73/5.89)   | 435.427 (9.38/5.86/6.07)   | 428.447 (9.38/5.86/6.07) | 243.671 (3.48/4.73/5.89) | 243.033 (9.38/5.86/6.07) | 239.345 (8.79/5.79/6.04) |
| [profile](logs/part-a-search-profile.jsonl)   |    955,064 / 4,798,287 | 1,018.932 (4.08/4.83/5.92) | 980.716 (8.79/5.79/6.04)   | 969.449 (8.79/5.79/6.04) | 260.124 (4.08/4.83/5.92) | 257.612 (8.79/5.79/6.04) | 256.255 (8.79/5.79/6.04) |
| [domain](logs/part-a-search-domain.jsonl)     |              145 / 738 | 421.228 (4.08/4.83/5.92)   | 426.411 (8.79/5.79/6.04)   | 422.159 (8.79/5.79/6.04) | 243.093 (4.08/4.83/5.92) | 242.701 (8.79/5.79/6.04) | 241.149 (8.79/5.79/6.04) |

Next/count evidence: [topic next](logs/part-a-isolated-topic-next.jsonl), [topic count](logs/part-a-isolated-topic-count.jsonl), [topic42 next](logs/part-a-isolated-topic42-next.jsonl), [topic42 count](logs/part-a-isolated-topic42-count.jsonl), [topic421 next](logs/part-a-isolated-topic421-next.jsonl), [topic421 count](logs/part-a-isolated-topic421-count.jsonl), [rare next](logs/part-a-isolated-rare-next.jsonl), [rare count](logs/part-a-isolated-rare-count.jsonl), [last7 next](logs/part-a-isolated-last7-next.jsonl), [last7 count](logs/part-a-isolated-last7-count.jsonl), [profile next](logs/part-a-isolated-profile-next.jsonl), [profile count](logs/part-a-isolated-profile-count.jsonl), [domain next](logs/part-a-isolated-domain-next.jsonl), [domain count](logs/part-a-isolated-domain-count.jsonl).

“Last 7 days” is relative to the deterministic fixture's end, with a half-open upper bound. Profile = 1; domain = 42. Profile/time selection happens on individual visible visits before per-page count/MAX; domain is a page property. All-time aggregates come from the transactionally seeded document projection. Counts are exact, not capped.

The large generator assigns a fixed profile per page, so it alone cannot detect a cross-visit predicate bug. The additional [same-visit adversarial probe](src/search.rs) creates a page with an old visit in profile 1 and a recent visit in profile 2; it must be excluded. The valid page has a newer **nonmatching** visit at time 99, but its matching newest must be **80**, with count **1**. A wrong-domain page and a hidden visit are also excluded. [The log](logs/part-a-same-visit.jsonl) confirms exactly page 2 / newest 80 / one visit.

Peak process RSS for the seven complete search/oracle runs was **683.5–835.9 MiB**, including the deliberately unbounded exhaustive oracle. This is not a measurement of only the heap, nor a guarantee of target-device latency.

Minimal ranking reproduction, also retained in [regressions.rs](src/regressions.rs):

```sql
CREATE TABLE docs(id INTEGER PRIMARY KEY,body TEXT,newest INTEGER);
INSERT INTO docs VALUES
  (1,'needle needle needle',1),
  (2,'needle filler filler',2),
  (3,'needle needle filler',3);
CREATE INDEX terms ON docs USING fts(body);
SELECT id,fts_score(body,'needle') AS score,newest
FROM docs WHERE fts_match(body,'needle')
ORDER BY score DESC,newest DESC,id DESC LIMIT 3;
```

Expected `[1,3,2]`, actual `[3,2,1]`, despite returned scores **0.209835127**, **0.133531436**, **0.183605731** for IDs 1/2/3. Score-only SQL returns `[1,3,2]`. Arithmetic `(score+0)` and positional `ORDER BY 2,3,1` do not fix the composite order. [Results and EXPLAIN bytecode](logs/part-a-ordering-complete.jsonl), load **2.59/3.71/4.85**.

Root cause: `translate/optimizer/mod.rs:1957–1987` rewrites matching **result columns** to FTS synthetic columns, leaving the remaining ORDER BY expression as an ordinary FTS function. `vdbe/execute.rs:12182–12185` implements scalar `fts_score` fallback as **0.0**, so all score sort keys tie and recency becomes primary. The emitted bytecode contains the scalar function for sorting and an index-method column for the displayed score. [Source evidence](SOURCE-part-a.md#ranking-optimizer). A bounded SQL candidate window would not restore correctness; the verified Rust collector avoids both this ordering path and premature candidate truncation.

## 3. Concurrency and transaction ownership

The exact requested combination cannot be enabled. With the real 14.4M archive, FTS present and a second DB attached, `PRAGMA journal_mode=mvcc` returns:

```text
journal_mode=mvcc is not supported with experimental multiprocess WAL:
MVCC does not support multiprocess access
```

`BEGIN CONCURRENT` in WAL mode returns `Concurrent transaction mode is only supported when MVCC is enabled`. Both are measured in [the gate log](logs/part-a-mvcc-gate.jsonl), load **3.44/4.74/5.90**. Source `vdbe/execute.rs:19043–19057` explains that commit serialization, logical-log append offsets, and checkpoint exclusion are process-local. Disabling the multiprocess flag does not make two-process MVCC safe. This is not an encryption limitation.

The following **WAL diagnostic is explicitly not a passing MVCC test**. One writer process keeps a single `BEGIN IMMEDIATE` transaction open while inserting 1M visits and updating their 1M documents. It tests an independent connection in that process, a coordinator process, then an additional process. All small updates target pages outside the backup's page-ID range, so their failures are writer serialization, not row overlap. A second database is attached to the long writer.

| While the long transaction is open           | Star / note / tag / ordinary row update (ms)        | History page (ms) | Correct ranked top 100 (ms) | Load 1/5/15                     |
| -------------------------------------------- | --------------------------------------------------- | ----------------: | --------------------------: | ------------------------------- |
| Independent `db.connect()` in writer process | 250.139 / 250.136 / 250.120 / 250.128; **all Busy** |             0.175 |                     887.712 | 6.44/5.56/5.94                  |
| Additional process                           | 250.319 / 250.246 / 250.243 / 250.230; **all Busy** |             0.164 |                     954.463 | 6.25/5.53/5.93                  |
| Coordinator process                          | 250.340 / 250.228 / 250.228 / 250.240; **all Busy** |             0.165 |                     957.595 | 6.44/5.56/5.94 → 6.25/5.53/5.93 |

The backup stages **1,000,000 visits and 1,000,000 page updates** in **120,216.645 ms** (this interval includes its same-process interference checks). Commit takes **674.726 ms**, load **4.95/5.26/5.76**. The precommit FTS assertion sees no `backupmarker`. The existing coordinator's postcommit FTS read then stalls and returns `database is locked`; total scenario runtime is **255.628 s**, ending at load **4.17/4.68/5.44**. The failing read has a start record but no success-duration record; no exact per-read duration is invented. [Full log](logs/part-a-wal-scenario.jsonl).

A [fresh-process verification](logs/part-a-backup-reopened.jsonl) confirms every FTS ID **1…1,000,000**, every added visit ID **14,400,001…15,400,000**, and total **15,400,000 visits**. That validation takes **608.473 ms**, load **3.47/4.40/5.28**. Reopened star/note/tag/ordinary updates commit in **232.269 / 3,399.983 / 429.808 / 0.216 ms** (load **3.47/4.40/5.28 → 3.44/4.38/5.26**); subsequent FTS reads find the note and tag at the correct page. Consistency after reopen passes; live read continuity and nonwaiting small-write commits fail. No permanent deadlock was established: the observed stall eventually returns Busy.

The separate [single-process MVCC control](logs/part-a-mvcc-small.jsonl), with FTS and ATTACH, shows a disjoint update committing in **1.256 ms** while another transaction is open; the overlapping row update fails with `Write-write conflict` in **0.100 ms**, load **2.85/5.12/6.16**. Rollback of the loser and commit of the winner leave the expected FTS entries; integrity check is `ok`. These are three-document controls, not a substitute for an unavailable two-process scale configuration.

The safe transaction-ownership pattern is:

```rust
let backup = db.connect()?;
let interactive = db.connect()?; // Independent transaction ownership.
let reader = db.connect()?;      // Independent read snapshot.
// Each owner serializes its own operations and drains/drops Rows before COMMIT.
// backup.clone() shares the same underlying connection; it is not another owner.
```

The control explicitly updates through a clone during the backup transaction and proves that rollback undoes both changes. A separate connection cannot see the clone's uncommitted FTS entry. The public [Clone implementation](SOURCE-part-a.md#connection-ownership) shares the inner `Arc`; asynchronous tasks or a mutex around individual statements do not change transaction ownership. Serialize the **whole transaction** per owner. A single-process database broker could change the topology, but that is not a demonstrated pass for the requested two-process database access.

## 4. Stall diagnosis and recovery boundary

The earlier plain run already failed at live-producer ATTACH after two FTS builds ([original log](logs/lifecycle-plain-lifecycle.jsonl)); reopening succeeded ([original reopen](logs/lifecycle-plain-attach-existing.jsonl)). The new [instrumented run](logs/part-a-lifecycle-full.jsonl) reproduces it without encryption. Its trace repeatedly says:

```text
required_max_frame=266278 authority_max_frame=266278 authority_checkpoint_seq=73
refusing live overflow fallback refresh on a read path
because it would require blocking WAL scan I/O
```

The source chain is retained in [SOURCE-part-a.md](SOURCE-part-a.md#wal-capacity):

1. `storage/shared_wal_coordination.rs:63–74,2315–2338`: 4,096 entries × 64 blocks = **262,144 frame-index entries**, then mark overflow.
2. `storage/wal.rs:1901–1927`: reject a live read when the overflow fallback has not been populated for the requested snapshot; return Busy instead of scanning the WAL.
3. `storage/wal.rs:3407–3417`: reinterpret that Busy as retryable.
4. `storage/wal.rs:3450–3485`: up to 100 retries with quadratic sleeps. This is inside the WAL reader, so a zero connection busy timeout does not bound it.
5. `database.rs:1783–1803`: opening the attached pager begins a read transaction and can repeat that inner attempt. The ordinary busy handler can add further attempts.

The small [wal_limit.rs reproduction](src/wal_limit.rs) needs no FTS, no ATTACH, and only one connection: enable multiprocess WAL; disable autocheckpoint; create a table; checkpoint; insert **140,000 × zeroblob(4000)** in one transaction; commit; read one row. WAL bytes **1,097,160,152** include repeated page frames and exceed the shared index capacity. Write/commit **869.777 ms**; first read **10,483.325 ms → Busy**, load **3.65/4.66/5.46**. The following checkpoint returns **`(1,NULL,NULL)`** in **0.049 ms**, load **3.80/4.66/5.45**. An SQL call returning successfully does not mean the checkpoint completed. [Log](logs/part-a-wal-limit-minimal.jsonl).

A full-scale attempt to checkpoint after each index also fails: the first returns `(0,0,0)` in **13.153 ms**, the second `(1,NULL,NULL)` in **0.036 ms**, followed by the same ATTACH stall and **130.062 s** supervisor deadline. [Log](logs/part-a-lifecycle-checkpoints.jsonl). Thus “add a checkpoint after CREATE INDEX” is not offered as a reliable fix.

A new process rebuilds the fallback from the WAL (`storage/wal.rs:5793–5799`). Reopen + checkpoint + ATTACH + search completes in **1.205 s** end to end; ATTACH **0.286 ms**, search **119.556 ms**, load **3.48/4.82/5.58**. [Recovery evidence](logs/part-a-lifecycle-reopen.jsonl). Replacing all database/connection owners at an explicit bulk-build boundary is a possible PathKeep recovery procedure, but is not a proven general solution for live two-process reads. This is why the item remains FAIL.

One index does not inevitably stall: the baseline seed's first search and ATTACH completed in **146.279 / 62.984 ms** while the producer stayed alive, load **3.75/4.86/5.96**. Small two-index controls at 10k and 100k documents also pass ([10k](logs/part-a-lifecycle-min.jsonl), [100k](logs/part-a-lifecycle-100k.jsonl)). The failing condition is WAL-frame overflow and unavailable fallback coverage, not merely forgetting to close a statement or the presence of FTS.

## 5. Rust API and Tauri integration

| API subitem                                      | Verdict                                                | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------ | ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Interrupt an in-flight query from another thread | **FAIL**                                               | The unmodified release does not compile `connection.interrupt()` (`E0599`). `turso_sdk_kit::rsapi::TursoConnection::interrupt` exists at `rsapi.rs:1181`, but public Turso keeps the handle and its accessor private (`connection.rs:57,138`). No safe public forwarding path was found. [Compile failure](logs/interrupt-stock-compile.log), load **6.75/6.66/6.73**, and [source](SOURCE-part-a.md#interrupt-private-sdk). |
| `INSERT … RETURNING`                             | **PASS**                                               | Two returned IDs/bodies are fully drained, then both are visible through another connection: **0.767 ms**. Use `query`/`Rows::next` through `None`; `execute` expects no result rows.                                                                                                                                                                                                                                        |
| Busy handling                                    | **PASS**, with the separate WAL-stall limitation above | Configured 0 / 75 / 250 ms produce Busy in **0.082 / 75.086 / 250.103 ms** under a held writer. After release, retry commits in **0.471 ms**. A stale read-snapshot upgrade returns `BusySnapshot` in **0.025 ms**, not another 250-ms wait; roll back and retry the whole transaction.                                                                                                                                      |
| Dedicated connection-thread facade               | **PASS** for the demonstrated integration pattern      | The worker owns a current-thread Tokio runtime, database, connection, and complete export transaction. The caller uses a separate atomic cancellation flag; a 1,024-row chunk loop stops after **580,608 rows**, **0.357 ms** after the flag is set, and proves the connection is reusable. Total worker test **52.626 ms**. This is cooperative cancellation, not SQL interruption.                                         |

The API run's load is **3.47/4.46/5.32** ([log](logs/part-a-api-complete.jsonl)); the worker run's load is **3.25/4.41/5.65** ([log](logs/part-a-worker.jsonl)). The worker's measured byte traversal is a database-export loop; it does not benchmark filesystem serialization or a full Tauri executable.

**Recommendation: own each connection on a dedicated database worker thread and expose a typed synchronous request/reply facade to commands already running on Tauri's blocking pool.** Run the async Turso API inside that worker's runtime. Keep transaction begin/work/commit in one request, and never let Rows/Statement escape the owner. Use separate owned connections/workers for backup, ordinary interactive operations, and long reads/exports; one global FIFO would put interactive work behind the export. Bound the request queue, return cancellation separately from that queue, and discard cancelled output before publishing an export.

This has one clear transaction owner and avoids nesting runtime `block_on` calls in arbitrary command executors. `async` does not itself move work off a thread: the public future directly steps the engine and drives local I/O ([source](SOURCE-part-a.md#api-step)). In particular, a Tokio timeout cannot promise interruption while the engine is blocked inside that poll. The wrapper also cannot create concurrent WAL writers or fix the overflow stall. Keep cooperative cancellation for bounded page reads; the requested arbitrary running-query interruption remains unavailable through this public release API. No dependency fork is proposed or counted as a pass.

## 6. Browser-file safety and linking

The [browser fixture](src/browser.rs) uses real bundled SQLite to hold live Chrome-like `urls/visits` and Firefox-like `moz_places/moz_historyvisits` databases. It checkpoints the base row, commits row 2 **only in WAL**, then holds uncommitted row 3. Turso's simple live read happens to return rows 1/2, but that is not sufficient evidence of safe live snapshots.

The additional WAL-recycle reproduction opens a Turso read transaction, reads one page, then has SQLite update another page and run `wal_checkpoint(TRUNCATE)`. SQLite reports **`(0,0,0)`**, despite the active Turso read snapshot. Turso's subsequent read fails:

```text
I/O error: short read on WAL frame at offset 8272:
expected 4096 bytes, got 0
```

Failure arrives in **0.292 ms**, load **9.93/5.91/6.09**. The SQLite control performs the same steps with a real SQLite reader: checkpoint reports **`(1,8,4)`** and the reader correctly retains `sqlite-snapshot`. This reproduces a safety gap, not just an absent convenience backup method. Turso's own coordination uses `.tshm` (or its in-process coordination), not SQLite's shared WAL read marks; see [local source](SOURCE-part-a.md#browser-wal) and [failure/control log](logs/part-a-browser-recycle-fixed.jsonl).

The PathKeep-owned boundary should therefore be:

```text
live Chrome/Firefox DB + WAL
    → rusqlite read-only connection + SQLite online backup
    → completed, integrity-checked SQLite snapshot
    → import rows into PathKeep-owned Turso DB
```

Both browser backups return exactly IDs **[1,2]**, exclude uncommitted ID **3**, and pass SQLite integrity check. Chrome **0.592 ms**, Firefox **0.591 ms**, both with **4,152-byte WALs**, load **2.85/5.12/6.16**. Import into an unencrypted Turso database also passes in the same process. Safari is an extra fixture/control, not a required platform claim. [Evidence](logs/part-a-browser.jsonl).

[The manifest](Cargo.toml) links `turso = 0.8.1` and `rusqlite = 0.39.0` with `bundled,backup`. The measured Mach-O arm64 binary contains both Turso Rust symbols and `sqlite3_backup_init` / `sqlite3_open_v2`. `otool -L` lists CoreFoundation, libiconv and libSystem only: no dynamic SQLite/SQLCipher or Homebrew database library. The standalone research executable is **29,080,136 bytes unstripped / 22,497,408 bytes stripped** (27.733 / 21.455 MiB); this is not the full Tauri app's size. [Size, hashes, symbol and link evidence](logs/part-a-binary.json), load **3.09/4.15/5.12**. The measured stripped binary also ran the full browser snapshot/import probe successfully ([log](logs/part-a-browser-stripped.jsonl)).

## Reproduction and retained artifacts

Only the part-A binary is needed. Use the cached dependencies and prohibit Cargo network access:

```sh
cd research/turso-probes
export CARGO_HOME="$PWD/.work/cargo-home"
export CARGO_NET_OFFLINE=true
cargo build --offline --locked --release --bin part-a
export PROBE_BINARY=target/release/part-a
PROBE_LOG=replay-seed python3 scripts/run_probe.py seed replay-scale
for query in topic topic42 topic421 rare last7 profile domain; do
  PROBE_LOG=replay-search-$query python3 scripts/run_probe.py search replay-scale "$query"
  for operation in next count; do
    PROBE_LOG=replay-$query-$operation python3 scripts/run_probe.py search-isolated replay-scale "$query" "$operation"
  done
done
PROBE_LOG=replay-language python3 scripts/run_probe.py language replay-language
PROBE_LOG=replay-ordering python3 scripts/run_probe.py ordering replay-ordering
PROBE_LOG=replay-same-visit python3 scripts/run_probe.py same-visit replay-same-visit
PROBE_LOG=replay-gate python3 scripts/run_probe.py mvcc-gate replay-scale
PROBE_LOG=replay-mvcc python3 scripts/run_probe.py mvcc-small replay-mvcc
PROBE_LOG=replay-worker python3 scripts/run_probe.py worker replay-scale
PROBE_LOG=replay-api python3 scripts/run_probe.py api replay-api
PROBE_LOG=replay-browser python3 scripts/run_probe.py browser replay-browser
PROBE_LOG=replay-recycle python3 scripts/run_probe.py browser-recycle replay-recycle
# Expected scenario failure after successful backup commit:
PROBE_LOG=replay-concurrency python3 scripts/run_probe.py wal-scenario replay-scale
PROBE_LOG=replay-consistency python3 scripts/run_probe.py verify-backup replay-scale
# Expected Busy result, captured by the probe:
PROBE_LOG=replay-limit PROBE_TIMEOUT=40 python3 scripts/run_probe.py wal-limit replay-limit
# Expected supervisor deadline at ATTACH:
TURSO_TRACE_ATTACH='turso_core::storage::wal=debug,turso_core::connection=debug,turso_core::database=debug' \
  PROBE_LOG=replay-lifecycle PROBE_TIMEOUT=130 python3 scripts/run_probe.py lifecycle replay-lifecycle
PROBE_LOG=replay-reopen python3 scripts/run_probe.py attach-existing replay-lifecycle checkpoint
```

Run against fresh replay directories. The supervisor records the command, exit status, external deadline, peak disposable-data size, process RSS, and uptime. Expected SQL defects sometimes produce process exit 0 because the probe records the defective result; interpret the result records, not exit status alone. Checkpoint success requires its busy-status column to be zero.

The original probe code/logs were reused for fixture generation, language, ranking, lifecycle, concurrency and browser snapshots; the missing unencrypted runs, isolated cold operations, WAL-recycle control, API worker, and FTS-independent WAL-limit reproduction were added. The earlier plain lifecycle failure and stock interrupt compile failure were retained as evidence; earlier encrypted timings and the patched interrupt result were not used as passing evidence.

Focused validation: offline release build, formatted part-A Rust modules, real-engine probes, all seven exhaustive ranking checks, and [retained-log verification](scripts/verify_part_a.py). The product `bun run check` was not run: this task changes only the standalone research project and prohibits external services/product-file writes. [Source excerpts](SOURCE-part-a.md) retain root-cause evidence after build artifacts are removed.

Cleanup is recorded in [part-a-cleanup.json](logs/part-a-cleanup.json): `research/turso-probes/data/` and `research/turso-probes/target/` are deleted after the probes. Code, logs, release-source excerpts, and this report remain.
