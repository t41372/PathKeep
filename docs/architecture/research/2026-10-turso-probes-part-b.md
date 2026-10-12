# Turso acceptance probes, part b: encryption (2026-10-05)

> Measured on a macOS arm64 host (18 logical CPUs) on 2026-10-05, for [ADR-012](../decisions/012-turso-storage-engine.md). Links to `src/`, `logs/` and `scripts/` point to the probe checkout, which is not kept in the repo.

# Turso 0.8.1: part B results (encryption at rest)

All runs use stock `turso = 0.8.1`, native encryption with `cipher=aegis256`, multiprocess WAL on (PathKeep's planned configuration) unless stated otherwise, and synthetic data. No external service was contacted. No product code was changed. The probes are in the [`part-b` binary](src/part_b.rs), and every number below comes from a log written in this run under [`logs/b/`](logs/b/). Older logs directly under `logs/` came from an earlier run in a different directory and are not cited.

Host: macOS arm64, 18 logical CPUs, Rust 1.96.0, shared with other work. Every timing record carries `uptime`'s 1/5/15-minute load averages, and the load ranged from 3.2 to 11.5 during the runs. This is not the 4-core/8 GB target, and none of these numbers certify target latency. "Cold" means the first use in a fresh process. The OS file cache was not purged.

Fixture: the existing deterministic generator ([fixture.rs](src/fixture.rs)) at **1,000,000 visits / 250,000 page documents** with the ten-field native FTS index, seeded once encrypted and once in plaintext (162 MB each). [Seed logs](logs/b/seed-enc.jsonl), [plain](logs/b/seed-plain.jsonl).

## Verdict table

| #   | Requirement                         | Verdict                                     | One-line evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| --- | ----------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Key wrapping                        | **PASS**                                    | The Argon2id KEK wraps a random 256-bit data key with AES-256-GCM. Five encrypted databases, with FTS and a keyed ATTACH, open and query correctly before and after a password change. A wrong password fails at the AEAD. A wrong key at Turso fails with "Decryption failed for page=1" and changes no file (15/15 hashes equal). Rewrap leaves all 15 database files byte-identical. Unlock: **297 ms** (Argon2id m=256 MiB, t=3, p=4: 284 ms).                                                                                                                                                                        |
| 2   | Encrypted copy for portable bundles | **PASS WITH WORKAROUND**                    | Keyed `ATTACH` + table copy + index/FTS rebuild of the 1M archive in **4.96 s**. Schema, every row (rowid order), `sqlite_sequence`, the FTS stream (ids and scores) and three ranked pages are identical. `integrity_check` ok. No sentinel in the raw bytes. The header starts with `Turso\0\x03`. The destination key opens it; the source key fails. Workaround: skip the reserved `__turso_internal_*` tables and let `CREATE INDEX … USING fts` rebuild them. At full scale, run the copy behind the R3 spill layer, because its index builds sort. `VACUUM INTO` writes plaintext (confirmed by executable probe). |
| 3   | Temp and spill files                | **PASS WITH WORKAROUND** (native: **FAIL**) | Natively, `temp_store=FILE` writes plaintext sorter, hash and ephemeral spills to `$TMPDIR` (6 of 6 spilling cases leak). `temp_store=MEMORY` writes no files but has no memory bound. The workaround keeps memory bounded with no plaintext on disk: `temp_store=FILE` + a PathKeep-owned IO layer that AES-CTR-encrypts `tursodb_temp_file` + no `TEMP` tables (use a keyed ATTACHed scratch database). Same results (digest-checked); cost with hardware AES is −0.2% to +21%. `CREATE TEMP TABLE` still leaks under the layer (155 MB plaintext WAL), by design of Turso's temp database.                             |
| 4   | Crash safety                        | **PASS**                                    | **168** runs, **162** real SIGKILLs at random I/O points or random times: during a 100k-row encrypted FTS write transaction (48), during an explicit checkpoint's backfill (46), and during many small commits with automatic checkpoints (48), plus 24 extra runs with a raw-byte scan. **0 failures**: the database always opens with the key, `integrity_check` is ok, exactly the committed rows exist (no partial transaction, no lost acknowledged commit), and the FTS id sets equal the table scans.                                                                                                              |
| 5   | Encryption overhead                 | **PASS** (measured)                         | Medians of 5 interleaved fresh-process runs at 1M. Warm history page **0.050 vs 0.050 ms**. Cold history page **0.469 vs 0.251 ms**. Ranked "topic" page cold **88.0 vs 65.3 ms**, warm **42.5 vs 43.4 ms**. Full streaming export **872 vs 851 ms (+2.5%)**. 100k-row import **541 vs 508 ms (+6.5%)**, of which COMMIT is **50.2 vs 26.1 ms**.                                                                                                                                                                                                                                                                          |

`PASS WITH WORKAROUND` means the fix lives entirely in PathKeep's code or build configuration, with no Turso patch.

---

## 1. Key wrapping

Code: [`encryption.rs` `envelope`/`unlock`/`kdf_bench`](src/encryption.rs). Logs: [envelope](logs/b/r1-envelope.jsonl), [unlock](logs/b/r1-unlock.jsonl), [KDF](logs/b/r1-kdf.jsonl).

**How it could fail, and what was checked**

- The wrong password unwraps something. Checked: AES-GCM authentication over the whole header.
- A wrong key that reaches Turso opens garbage, or writes a WAL, `-tshm` or header and damages the files. Checked: per-file SHA-256 and the directory listing before and after 15 wrong-key and no-key open attempts, in both WAL modes.
- A password change touches data files. Checked: hashes of all 15 files (`.db`, `-wal`, `-tshm`) before and after the rewrap.
- After the change, FTS or a keyed ATTACH no longer works. Checked: the same query contract runs before and after and must give equal results.
- KDF parameters are not bound to the envelope, so an attacker could downgrade them. Checked: flip one parameter byte and expect rejection.

**Design probed.** The envelope is 94 bytes: `PKENV`, version, then Argon2id `m_kib`, `t`, `p` (u32 LE), a 16-byte salt and a 12-byte nonce, followed by AES-256-GCM(data key) with a 16-byte tag. The header is the AEAD associated data, so the parameters can change later and cannot be edited silently. The file is replaced atomically: temp file, fsync, rename, directory fsync. The unwrapped key is passed hex-encoded to `turso::Builder::with_encryption(EncryptionOpts { cipher: "aegis256", hexkey })`, which also needs `experimental_encryption(true)` ([turso-0.8.1/src/lib.rs:207–214](.work/cargo-home/registry/src/index.crates.io-1949cf8c6b5b557f/turso-0.8.1/src/lib.rs)). ATTACH uses the URI form `file:<path>?cipher=aegis256&hexkey=<hex>`, which `turso_core/util.rs:1194–1195` parses. Available ciphers are listed in `storage/encryption.rs:413–434`.

**Five databases** (archive, source-evidence, search, intelligence, agent), multiprocess WAL, each with a two-column `USING fts` index. The archive has 100,000 rows; the others have 20,000. Results:

| Check                                                                      | Result                                                                                                                                                                   |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Correct password → all five open; FTS sentinel ids equal a full-table scan | archive 100 ids; others 20 ids each                                                                                                                                      |
| Keyed `ATTACH` of source-evidence + cross-DB join + FTS on the attached DB | 20 joined rows, 20 FTS ids                                                                                                                                               |
| Wrong password                                                             | rejected at unwrap ("wrong password or damaged envelope"); no database touched                                                                                           |
| Wrong key at Turso (×5 DBs × {multiprocess, single-process})               | every open: `Decryption failed for page=1`                                                                                                                               |
| No key at all (×5)                                                         | every open: `file is not a database`                                                                                                                                     |
| Files after the 15 failed opens                                            | **identical** hashes and listing, including `-wal` and `-tshm`                                                                                                           |
| Keyed ATTACH with the wrong key                                            | `Decryption failed for page=1`, not attached; the main connection still answers (100,000 rows)                                                                           |
| Password change                                                            | **579 ms** (two Argon2id runs + atomic replace); all 15 database files **byte-identical**; old password rejected; new password opens all; results equal before and after |
| Tampered wrapped key / tampered `t_cost` byte                              | both rejected                                                                                                                                                            |

Load during the envelope run: 3.85 / 5.76 / 5.13.

**Unlock time** (fresh process, measured three times, load 3.78 / 5.72 / 5.11): total **297.1 / 298.6 / 299.4 ms**, made up of Argon2id **283.5 / 284.9 / 285.7 ms**, unwrap **0.013 ms**, and opening plus one query on all five databases **13.5–13.7 ms**. Peak RSS 302 MiB, mostly the KDF.

**Argon2id parameters.** Chosen: **m = 256 MiB, t = 3, p = 4**. This is RFC 9106's second recommendation (64 MiB, t=3, p=4) with four times the memory. A transient 256 MiB is about 3% of 8 GB. Measured alternatives on this host (load 4.2–4.3), median of 3 runs:

| m           | t     | p     | ms      | peak RSS    |
| ----------- | ----- | ----- | ------- | ----------- |
| 19 MiB      | 2     | 1     | 11      | 26 MiB      |
| 64 MiB      | 3     | 4     | 61      | 71 MiB      |
| **256 MiB** | **3** | **4** | **275** | **263 MiB** |
| 512 MiB     | 3     | 4     | 563     | 519 MiB     |
| 1 GiB       | 2     | 4     | 768     | 1,032 MiB   |
| 2 GiB       | 1     | 4     | 968     | 2,055 MiB   |

`argon2` 0.5.3 has **no `parallel` feature** (its `Cargo.toml` features are `alloc`, `default`, `rand`, `simple`, `std`), so the four lanes run one after another on one core and `p` changes only the output. Unlock time on the target was not measured.

**Crate facts, from local metadata.**

- `argon2` 0.5.3: authors "RustCrypto Developers", repository `github.com/RustCrypto/password-hashes`, MIT OR Apache-2.0.
- `aes-gcm` 0.10.3: RustCrypto.
- Stars are not part of crate metadata, so I can't see them from the crate. A GitHub API response cached by an earlier run ([`logs/argon2-repository.json`](logs/argon2-repository.json), not refetched) lists `RustCrypto/password-hashes` with **912 stars**, last pushed 2026-09-21. That is below the 6k-star threshold, but the crate is maintained by a well-known organization (RustCrypto), which AGENTS.md accepts.

## 2. Encrypted copy for portable bundles

Code: [`encryption.rs` `bundle`](src/encryption.rs). Log: [r2-bundle.jsonl](logs/b/r2-bundle.jsonl). Load 3.73 / 4.19 / 4.41.

**How it could fail.**

- The destination is not actually keyed.
- Plaintext reaches the destination WAL or temp files during the copy.
- FTS backing data is copied as ordinary tables, or is missing.
- Ids or AUTOINCREMENT high-water marks change.
- Results differ subtly: scores, or rows in a different order.
- A failed open with the wrong key damages the bundle.

**Procedure.** Open the encrypted 1M archive. `ATTACH 'file:bundle.db?cipher=aegis256&hexkey=<other key>' AS bundle`. For each user table, replay its stored `CREATE TABLE` into `bundle.` and copy rows with `INSERT … SELECT * … ORDER BY rowid` in 200k-rowid transactions, checkpointing the bundle after each. Copy `sqlite_sequence`. Replay every index, view and trigger, including `CREATE INDEX search … USING fts (…) WITH (…)` as stored. Checkpoint, then `DETACH`.

Timings: pages 276 ms; visits 249 ms; `by_page` 789 ms; `by_time` 566 ms; `by_profile` 860 ms; `by_domain` 142 ms; FTS rebuild 2,082 ms; total **4,965 ms**. Verification took 5,741 ms. Peak destination WAL during the copy was 62.7 MB.

**Turso detail (workaround).** `sqlite_schema` contains `__turso_internal_fts_dir_search` (FTS segment storage) and its `backing_btree` index. Replaying that `CREATE TABLE` fails with _"Parse error: Object name reserved for internal use"_. The copy must skip `__turso_internal_*` and let `CREATE INDEX … USING fts` recreate the storage. Once that is done, the bundle's `sqlite_schema` is identical to the source's.

**Verification.**

- Schema rows are equal.
- Every row of `pages` (250,000) and `visits` (1,000,000), read in rowid order with rowid, has an equal SHA-256 digest.
- `sqlite_sequence` is equal.
- `integrity_check` returns `ok`.
- The full `fts_score` stream for `topic` (ids and scores) is equal.
- The bounded-collector ranked pages for `topic` (203,414 pages / 1,000,000 visits), `topic42` (5,561 / 29,633) and `rare` (1 / 7) are equal.

**Raw bytes.** The needles were `about topic`, `https://site`, `CREATE TABLE`, `by_time`, `SQLite format 3` and `synthetic evidence`. None of them appears in `bundle.db` (162 MB), `-tshm` or `-wal`, and the watcher saw no plaintext in the bundle directory or `$TMPDIR` during the copy. The header starts with `54 75 72 73 6f 00 03` (`Turso`, version 0, cipher id 3), as described in `storage/encryption.rs:44–75`. The first 100 bytes are clear by design and are authenticated as associated data. Key checks:

- source key → `Decryption failed for page=1`;
- no key → `file is not a database`;
- files unchanged by both attempts;
- destination key reopens all 1,000,000 visits.

**Scale caveat.** At 1M, the seven index builds fit in the 64 MiB cache and wrote no temp file. At 14.4M the same `CREATE INDEX` sorts spill: R3 measured a plaintext `tursodb_temp_file` from `CREATE INDEX` at 1M once the cache is smaller than the sort. Run bundle creation with the R3 spill layer enabled.

**`VACUUM INTO` from an encrypted source writes plaintext** (executable probe):

- The output header is `SQLite format 3\0`.
- `about topic`, `https://site`, `CREATE TABLE` and `by_time` are present in clear.
- Turso opens it **without a key** and reads 1,000,000 visits and `https://site1.example/page/1`.
- Plain SQLite (rusqlite) cannot open it at all: _"malformed database schema (\_\_turso_internal_fts_dir_search_key) – near "USING""_. Any SQLite tool that reads a Turso file with FTS hits the same parse error.

Root cause: `turso_core-0.8.1/vdbe/execute.rs:19594–19616` opens the target with a fresh `PlatformIO`, calls `Database::open_file_with_flags(io, dest_path, OpenFlags::Create, output_opts, None, …)` (encryption `None`), then `output_db.connect()`, so no key is ever applied. The in-place `VACUUM` temp database is different: `vdbe/vacuum.rs:256–318` copies the source key. A keyed URI target is not supported either. `VACUUM INTO 'file:…/out.db?cipher=aegis256&hexkey=…'` fails with _"I/O error (statfs shared WAL coordination path): entity not found"_ and writes nothing, in single-process mode too, because the string is used as a literal path. Minimal reproduction: the `vacuum-uri` mode in [part_b.rs](src/part_b.rs) ([log](logs/b/r2-vacuum-uri.jsonl)):

```sql
-- source opened with cipher=aegis256, hexkey=K
CREATE TABLE t(id INTEGER PRIMARY KEY, body TEXT);
INSERT INTO t VALUES (1, 'PATHKEEP_VACUUM_SENTINEL');
VACUUM INTO '/path/out.db';  -- out.db: "SQLite format 3", sentinel readable
VACUUM INTO 'file:/path/out.db?cipher=aegis256&hexkey=K2';  -- error, nothing written
```

## 3. Temp and spill files

Code: [`spill.rs`](src/spill.rs), [`tempcrypt.rs`](src/tempcrypt.rs), [`watch.rs`](src/watch.rs). Logs: `logs/b/r3-{file,memory,crypt}-<case>.jsonl`; first crypt runs with software AES are in [`logs/b/soft-aes/`](logs/b/soft-aes/). Summary: `python3 scripts/summarize_r3.py`.

**How it could fail.**

- The query doesn't spill, so the test proves nothing. Guard: `EXPLAIN` must show the sorter or hash opcodes, and the FILE run must produce a temp file.
- The engine deletes the temp file before it is inspected. Guard: a 2 ms poller hard-links every new file into a quarantine directory, so its final bytes survive the unlink.
- Files appear outside the watched directories. Guard: `lsof` of the process while the spill is live.
- Plaintext is written and then overwritten. This is a known limitation: only the final bytes per file are scanned.
- Encryption is "on" but bypassed for some path. Guard: every case runs under all three configurations.

**Setup.** A copy of the encrypted 1M archive, multiprocess WAL, `cache_size = -8192` (8 MiB) so that 1M rows exceed the cache. `TMPDIR` pointed at `data/tmp`, and both it and the database directory were watched. Needles: `about topic`, `https://site`, `.example/page/` (all in titles and URLs). The positive control is the FILE configuration, where the needles are found in temp files. In no run did the process have a file open outside the project directory.

**Results.** Final run with the hardware-AES build. The three configurations ran back to back for each case, at load 4.6–5.2 / 4.4–4.5 / 4.6. Each run records an ordered and an order-free SHA-256 of every result row, and **all seven cases give identical digests under `file`, `memory` and `crypt`**, so the encrypted spill returns exactly the plaintext spill's answers.

| Case (1M)                                        | Opcodes                   | FILE: temp file / plaintext              | FILE ms / RSS   | MEMORY: files / RSS | crypt: temp file / plaintext | crypt ms / RSS  |
| ------------------------------------------------ | ------------------------- | ---------------------------------------- | --------------- | ------------------- | ---------------------------- | --------------- |
| `ORDER BY title` over visits⋈pages               | HashBuild, SorterOpen     | 69.7 MB / **yes**                        | 1,709 / 171 MiB | none / 225 MiB      | 69.7 MB / no                 | 1,706 / 179 MiB |
| `GROUP BY url`                                   | HashBuild, SorterOpen     | 45.7 MB / **yes**                        | 1,219 / 163     | none / 194          | 45.7 MB / no                 | 1,216 / 165     |
| `DISTINCT title‖time`                            | HashBuild (+HashDistinct) | none                                     | 1,509 / 232     | none / 232          | none                         | 1,573 / 232     |
| Hash join on computed key                        | HashBuild, OpenEphemeral  | 57.9 MB / **yes**                        | 7,152 / 102     | none / 154          | 57.9 MB / no                 | 8,686 / 103     |
| `CREATE TEMP TABLE … AS SELECT`                  | —                         | `tursodb-temp.db-wal` 155.4 MB / **yes** | 949 / 284       | none / 433          | **155.4 MB / yes**           | 987 / 284       |
| `CREATE INDEX pages(url,title,…)`                | SorterOpen                | 26.8 MB / **yes**                        | 375 / 136       | none / 160          | 26.8 MB / no                 | 415 / 146       |
| Export written as a plain `JOIN … ORDER BY v.id` | HashBuild, SorterOpen     | 82.1 MB / **yes**                        | 1,964 / 166     | none / 221          | 82.1 MB / no                 | 2,007 / 178     |

In the crypt `TEMP TABLE` run, the layer opened one `tursodb_temp_file` and wrote 0 bytes to it. The 155 MB leak is entirely the temp database's WAL, which the layer never sees.

The 1M `DISTINCT` stayed under the hash table's 64 MiB _accounted_ budget, so it did not spill, yet RSS rose by about 150 MB. Actual memory is two to three times the accounted figure. The hash-table spill path itself is exercised by the hash-join case.

**Why, from the source** (all paths under `turso_core-0.8.1/`):

- Sorter, hash-table and ephemeral-table spills all use `TempFile` (`io/mod.rs:301–329`). It calls `tempfile::tempdir()` (which honors only `$TMPDIR`) and opens `<tmpdir>/tursodb_temp_file` **through the connection's own IO object**. Callers: `vdbe/sorter.rs:536`, `vdbe/hash_table.rs:831,881`, `vdbe/execute.rs:16193`.
- The ephemeral pager is built as `Pager::new(db_file, None, io, …)` at `vdbe/execute.rs:16204` with no encryption context, and the sorter and hash table write raw records. No encrypted spill path exists.
- `TEMP` tables live in a per-connection temp database (`connection.rs:690–745`). It is opened through a **fresh** `Database::io_for_path` (line 727) with encryption `None`, in a directory from `create_tempdir` (`connection.rs:4559–4568`, honoring `TURSO_TMPDIR`, then `SQLITE_TMPDIR`, then `$TMPDIR`). A custom IO layer never sees it. Turso's own test `page_codec_connection_uses_plain_internal_temp_database` (`database.rs:5114`) shows this is intended.
- `temp_store=MEMORY` swaps every one of these for a `MemoryIO` file (`io/mod.rs:351–358`; temp database `connection.rs:695–711`). It also removes the hash budget entirely: `mem_budget = usize::MAX` (`vdbe/execute.rs:17966–17971` for DISTINCT, the same pattern at line 17860). Nothing bounds memory.
- Bounds in FILE mode: the sorter's in-memory buffer equals `cache_size` (`vdbe/execute.rs:9397–9402`). The hash budget is 64 MiB in release builds and 32 KiB in debug builds (`vdbe/hash_table.rs:26–33`, "TODO: make configurable via PRAGMA").

**Memory cost of `temp_store=MEMORY` at 14.4M-equivalent sizes.** These are scaled measurements: the 1M result × 14.4, since spill volume is linear in rows. They are not separate runs.

| Case                     | Spill at 1M | Spill ×14.4 | MEMORY RSS increase over FILE at 1M | ×14.4     |
| ------------------------ | ----------- | ----------- | ----------------------------------- | --------- |
| ORDER BY                 | 69.7 MB     | 1.00 GB     | +54 MiB                             | +0.78 GiB |
| GROUP BY                 | 45.7 MB     | 0.66 GB     | +31 MiB                             | +0.44 GiB |
| Hash join                | 57.9 MB     | 0.83 GB     | +52 MiB                             | +0.73 GiB |
| TEMP table               | 155.4 MB    | 2.24 GB     | +149 MiB                            | +2.10 GiB |
| CREATE INDEX (url,title) | 26.8 MB     | 0.39 GB     | +24 MiB                             | +0.34 GiB |
| Naive JOIN export        | 82.1 MB     | 1.18 GB     | +55 MiB                             | +0.77 GiB |

So no native configuration has both bounded memory and no plaintext on disk. FILE is bounded but writes plaintext. MEMORY writes nothing but is unbounded: a single user-triggered sort on a 14.4M archive costs about a gigabyte on an 8 GB machine.

**Workaround: a bounded configuration with no plaintext**, all in PathKeep code:

1. Set `PRAGMA temp_store=FILE` (explicitly; the default behaves the same).
2. Open every connection with `Builder::with_io_impl(TempCryptIo)` ([tempcrypt.rs](src/tempcrypt.rs), about 200 lines).
   - It wraps `PlatformIO` and passes every file through untouched, except files whose path ends in `tursodb_temp_file`.
   - Those get AES-256-CTR with a random per-file key and IV held only in RAM; the keystream position is the file offset.
   - Writes encrypt a copy of the buffer. Reads wrap the read `Completion` so the buffer is decrypted in place before the engine's callback runs.
   - It forwards `open_shared_wal_file` and `supports_shared_wal_coordination`, which multiprocess WAL needs.
3. Never use `TEMP` tables or `temp.` for archive-derived data. For explicit scratch work, ATTACH a keyed scratch database. The earlier probe used this pattern, and it is ordinary encrypted Turso storage.

Measured: in every spilling case the crypt temp file has the same size as the FILE run, **contains no needle**, and the query returns identical results. Cost with hardware AES, comparing crypt with FILE run back to back: −0.2% (ORDER BY), −0.2% (GROUP BY), +2.2% (export join), +11% (CREATE INDEX), and +21% for the hash join, which rewrites its 58 MB file to a total of 3.67 GB written. Multiprocess WAL works through a custom IO: every R3 run used it (the watcher records `archive.db-tshm`), and in R4 the hook IO left `crash.db-tshm` files that reopened cleanly.

**Build requirement found on the way.** `aes` 0.8.4 uses ARMv8 AES instructions on aarch64 **only** with `--cfg aes_armv8` (`aes-0.8.4/src/lib.rs:104,134`); otherwise it falls back to software AES. Without the flag, the hash-join case took **50.1 s instead of 6.96 s** ([soft-AES logs](logs/b/soft-aes/)). The probe now sets it in [.cargo/config.toml](.cargo/config.toml). On x86_64, `aes` detects AES-NI at runtime. Turso's own AEGIS-256 page cipher is unaffected; it comes from the `aegis` crate.

**Limits of the workaround.**

- It covers only `tursodb_temp_file`. A future Turso release that renames that file or adds another spill path would silently bypass it, so the R3 probe must run on every Turso upgrade.
- CTR reuses keystream when an offset is overwritten. Ephemeral pages are rewritten, so two captured versions of the same bytes leak their XOR. The key never leaves the process, so a stolen disk holds one version per offset, plus whatever older blocks the SSD kept.
- It provides no integrity protection for scratch data.
- After a kill -9, temp directories are not removed, but their contents are ciphertext under a key that died with the process.
- Real memory can exceed the nominal bounds (see the DISTINCT row).

**Related finding: the export query shape.** `SELECT … FROM visits v JOIN pages p ON p.id=v.page_id ORDER BY v.id` plans as `HASH JOIN visits` + `SCAN pages` + `USE SORTER FOR ORDER BY`. It sorts the entire export, which spills 82 MB of plaintext at 1M (about 1.2 GB at 14.4M) and is slower: **1,153 ms vs 872 ms** for the streaming form, medians of 5 ([naive logs](logs/b/)). Writing `CROSS JOIN` (or `LEFT JOIN`) pins the plan to `SCAN visits` + `SEARCH p USING INTEGER PRIMARY KEY`, with no sorter. PathKeep should write the streaming form and assert in a test that its plan has no sorter. [overhead.rs](src/overhead.rs) does exactly that.

## 4. Crash safety

Code: [`crash.rs`](src/crash.rs). Logs: [r4-crash.jsonl](logs/b/r4-crash.jsonl) (144 runs, seed 20261004), [r4-crash-rawscan.jsonl](logs/b/r4-crash-rawscan.jsonl) (24 runs, seed 7041). Summary: `python3 scripts/summarize_r4.py`. Load 3.4–5.0. The campaign took 143.6 s.

**How it could fail.**

- A partial transaction becomes visible.
- An acknowledged commit is lost.
- A checkpoint interrupted mid-backfill corrupts the database file.
- The FTS index diverges from the table (the FTS segment writes and the table writes land at different times).
- The key no longer opens the database, for example because of a torn page 1 or a broken WAL salt.
- A stale `-tshm` blocks reopen in multiprocess mode.
- After recovery the database reads correctly but can no longer be written or checkpointed.

**Method.**

- Base database: encrypted, 100,000 rows of about 470 bytes with a `USING fts` index, checkpointed. It is copied fresh for each run.
- The child process runs the scenario through a hook `IO` that counts `pwrite`/`pwritev`, `sync` and `truncate` calls on `crash.db` and `crash.db-wal`.
- Calibration counts the events in the armed window. Each run then draws, from a seeded RNG, a hook kind that occurs in the window, a threshold uniform over its count, and whether to kill before or after the I/O call. The child sends itself `SIGKILL` at that event and first writes the exact kill point to a marker file.
- Every 4th run is an external kill instead: the parent sends `SIGKILL` after a uniform random delay inside the calibrated window, measured from a start marker.
- The child writes fsynced acknowledgements immediately after each `COMMIT` returns.
- Every run is reproducible from its record: seed, scenario, hook, threshold, before/after, delay.

**Scenarios**, each run in both WAL modes:

- **write**: one transaction inserting 100,000 FTS-indexed rows with a 4 MiB cache, so dirty pages spill into the WAL before COMMIT. The window covers the whole transaction and COMMIT. Turso 0.8.1 has **no `wal_autocheckpoint` PRAGMA** (no occurrence in `turso_core`; the statement is accepted and has no effect), and COMMIT runs a passive auto-checkpoint (`storage/pager.rs:3403–3417`). So this window also covers backfill to the database file inside COMMIT.
- **checkpoint**: commit 100,000 rows while a reader snapshot is held, so the auto-checkpoint cannot backfill them. End the reader, then arm and run `PRAGMA wal_checkpoint(TRUNCATE)` over a **111 MB WAL** (59 / 36 data-file writes, 3 syncs and 1 truncate in the window, which lasts 85 / 98 ms).
- **durable**: 200 transactions of 500 rows with default automatic checkpoints and external kills.

**Checks after every kill.**

- Reopen with the key in the same WAL mode.
- `integrity_check = ok`.
- Ids are contiguous from 1.
- All 100,000 base rows are present.
- For write: pending rows are 0 or exactly 100,000. They must be 100,000 if the commit was acknowledged, and 0 if COMMIT was never issued.
- For checkpoint: all 100,000 committed rows are present.
- For durable: the row count is a whole number of batches, k ≥ acknowledged and k ≤ acknowledged + 1.
- The `fts_match` id sets for both markers equal the `LIKE` table scans.
- One more row is inserted, is findable through FTS, and `wal_checkpoint(TRUNCATE)` succeeds.
- In the extra 24 runs, the raw `crash.db` and leftover `-wal` were also scanned for `pendingmarker`, `committedmarker` and `payloadpayload` before reopen.

**Results: 0 failures in 168 runs.**

| Scenario   | Mode         | Runs    | SIGKILLed | Kill points                                                                                                        | Outcome                                                                    |
| ---------- | ------------ | ------- | --------- | ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| write      | single       | 24 (+4) | 24 (+4)   | 5 before COMMIT (3 WAL writes, 2 timed); 19 inside COMMIT (13 data-file backfill writes, 2 WAL, 4 timed)           | 7 recovered with 0 pending rows, 17 with 100,000; all pass                 |
| write      | multiprocess | 24 (+4) | 24 (+4)   | 8 before COMMIT; 16 inside COMMIT (9 data-file, 7 WAL)                                                             | 11 with 0, 13 with 100,000; all pass                                       |
| checkpoint | single       | 24 (+4) | 20 (+4)   | 14 data-file backfill writes, 3 WAL sync/truncate, 3 timed; 4 timed kills landed after the checkpoint had finished | all 100,000 committed rows present every time                              |
| checkpoint | multiprocess | 24 (+4) | 22 (+4)   | 17 data-file backfill writes, 1 WAL, 4 timed (2 after finish)                                                      | all present                                                                |
| durable    | single       | 24 (+4) | 24 (+4)   | random times across 200 commits and their auto-checkpoints                                                         | recovered batches from 0 to 192, always the acknowledged count or one more |
| durable    | multiprocess | 24 (+4) | 24 (+4)   | same                                                                                                               | same                                                                       |

At reopen there was usually a non-empty `-wal`, and in multiprocess mode a stale `-tshm`. Recovery handled both every time. All 48 raw scans, 22 of them on non-empty post-kill WALs including uncommitted frames, found **no plaintext**.

Scope: SIGKILL leaves the OS page cache intact, so this proves crash consistency of the engine's write ordering and recovery, not durability across power loss or torn sector writes. Kills before an fsync and after a write count as durable here, because they are.

## 5. Encryption overhead

Code: [`overhead.rs`](src/overhead.rs). Driver: [`scripts/bench_b.sh`](scripts/bench_b.sh), which alternates the variant order each round. Summary: `python3 scripts/summarize_r5.py bench`. Five fresh processes per variant on identical 1M fixtures, with a 64 MiB page cache and multiprocess WAL. Load at start ranged 4.68–5.18 / 4.45–4.52 / 4.52–4.54. The OS file cache was warm, so these numbers show decryption CPU cost, not disk I/O. Correctness guards:

- The history page's first id must be the newest visit.
- Ranked-page digests must be identical across runs and variants (they were: `d1c8652a…`, `9b4a2211…`).
- The export must have n rows, the correct id sum and strictly increasing ids, and its SHA-256 must be identical across variants (`5a09ac01…`, 147,906,354 bytes).
- The import is checked by visit count and 25,000 new FTS documents.

| Operation (1M visits)                                                                        | Encrypted (median) | Plain (median) | Ratio |
| -------------------------------------------------------------------------------------------- | ------------------ | -------------- | ----- |
| Open                                                                                         | 0.625 ms           | 0.615 ms       | 1.02  |
| History page, 101 rows, cold in process                                                      | 0.469 ms           | 0.251 ms       | 1.87  |
| History page, warm (median of 21 per process)                                                | 0.050 ms           | 0.050 ms       | 1.01  |
| History page in the middle of the archive, cold region                                       | 0.354 ms           | 0.204 ms       | 1.74  |
| Ranked FTS first page `topic` (bounded collector, all 250k docs, includes eligibility), cold | 87.96 ms           | 65.35 ms       | 1.35  |
| same, warm                                                                                   | 42.54 ms           | 43.44 ms       | 0.98  |
| Ranked `topic42`, warm                                                                       | 26.61 ms           | 26.27 ms       | 1.01  |
| Ranked `rare`, warm                                                                          | 24.93 ms           | 24.27 ms       | 1.03  |
| Full streaming export to JSONL (1M rows, 148 MB, 256 KiB buffer, fsync)                      | 872.2 ms           | 850.9 ms       | 1.03  |
| Import: 100k visits + 25k FTS documents, one transaction, total                              | 540.9 ms           | 507.7 ms       | 1.07  |
| — staging                                                                                    | 483.7 ms           | 476.7 ms       | 1.01  |
| — COMMIT                                                                                     | 50.2 ms            | 26.1 ms        | 1.92  |
| — checkpoint (TRUNCATE)                                                                      | 1.6 ms             | 1.6 ms         | 0.97  |
| Peak RSS                                                                                     | 154.9 MiB          | 157.0 MiB      | 0.99  |

The pattern: encryption costs something only where pages are read from the file or written to the WAL for the first time. That is the first touch of a region (cold history page and cold ranked page) and the commit, where dirty pages are encrypted. Once a page sits decrypted in the page cache, cost is equal. Whole-archive streaming adds 2.5% and an import adds 6.5%. The history-page plan is `SCAN visits USING COVERING INDEX by_time` + `SEARCH p USING INTEGER PRIMARY KEY`; the export plan is `SCAN visits` + `SEARCH p USING INTEGER PRIMARY KEY`. Neither uses a sorter.

## Turso issues recorded here (not reported upstream)

| Issue                                                                                     | Turso source                                                                                           | Reproduction                                                                                    | PathKeep handling                                              |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `VACUUM INTO` from an encrypted DB writes plaintext; keyed URI target unsupported         | `vdbe/execute.rs:19594–19616` (encryption `None`, `connect()` without key)                             | `vacuum-uri` mode in [part_b.rs](src/part_b.rs); `bundle` in [encryption.rs](src/encryption.rs) | never use it; keyed ATTACH copy                                |
| Spill files (sorter, hash, ephemeral) are plaintext; no setting encrypts them             | `io/mod.rs:301–329`, `vdbe/execute.rs:16193–16212`, `vdbe/sorter.rs:536`, `vdbe/hash_table.rs:831,881` | `spill <root> file <case>` in [spill.rs](src/spill.rs)                                          | `TempCryptIo` via `with_io_impl`                               |
| TEMP database is plaintext and bypasses a custom IO                                       | `connection.rs:690–745` (`io_for_path` at 727, encryption `None`)                                      | `spill <root> crypt temptable`                                                                  | no TEMP tables; keyed ATTACH scratch                           |
| `temp_store=MEMORY` has no memory bound (hash budget becomes `usize::MAX`)                | `io/mod.rs:351–358`, `vdbe/execute.rs:17860, 17966–17971`                                              | `spill <root> memory <case>`                                                                    | do not use MEMORY                                              |
| Hash budget not configurable; actual RSS exceeds the accounted 64 MiB                     | `vdbe/hash_table.rs:26–33`                                                                             | `spill <root> file distinct`                                                                    | accept; avoid unbounded DISTINCT on archive-sized inputs       |
| No `wal_autocheckpoint` PRAGMA; COMMIT runs a passive auto-checkpoint                     | `storage/pager.rs:3403–3417`; no PRAGMA in `pragma.rs`                                                 | write scenario calibration in [crash.rs](src/crash.rs)                                          | hold a reader or call explicit checkpoints when timing matters |
| A plain `JOIN … ORDER BY rowid` export plans as hash join + full sort                     | planner                                                                                                | `plan` mode; `spill <root> file exportjoin`                                                     | write `CROSS JOIN`; test the plan has no sorter                |
| Reserved `__turso_internal_*` FTS tables cannot be recreated by replaying `sqlite_schema` | parse error "Object name reserved for internal use"                                                    | `bundle` mode (before the fix)                                                                  | skip them; `CREATE INDEX … USING fts` rebuilds                 |
| Plain SQLite cannot read a Turso file with FTS (`USING backing_btree` in schema)          | schema SQL in `index_method/fts`                                                                       | `bundle` → rusqlite open of the VACUUM output                                                   | don't hand Turso files to SQLite tools                         |

## Reproducing

```sh
export CARGO_HOME=$PWD/.work/cargo-home        # offline, pinned sources
cargo build --release --offline --bin part-b   # .cargo/config.toml enables aes_armv8
export PROBE_BINARY=target/release/part-b
python3 scripts/run_probe.py seed fx-enc 1000000
python3 scripts/run_probe.py seed fx-plain 1000000 plain
bash scripts/bench_b.sh                                      # R5
python3 scripts/run_probe.py bundle r2 $PWD/data/fx-enc 1000000   # R2
mkdir -p data/r3 && cp data/fx-enc/archive.db* data/r3/ && bash scripts/spill_b.sh   # R3
python3 scripts/run_probe.py envelope r1 && target/release/part-b unlock data/r1     # R1
python3 scripts/run_probe.py crash r4 24 20261004                                    # R4
```

`scripts/run_probe.py` kills a probe if `data/` exceeds 5 GB. The peak in this run was 0.86 GB, during R4. `data/` and `target/` were deleted afterwards.
