# vault-crypto

Encryption at rest for PathKeep, below the storage engines. It follows
[ADR-012](../../../docs/architecture/decisions/012-turso-storage-engine.md) and
[the Turso migration design](../../../docs/architecture/turso-migration-design.md) §3.

Three parts:

- **Key envelope** (`Envelope`, `DataKey`, `SubKey`): the vault's random data key, wrapped
  under the user's password, and one derived key per use.
- **Turso spill files** (`TempCryptIo`): an IO layer that encrypts the files Turso spills
  sorts and hash joins into.
- **Encrypted search index** (`EncryptedDirectory`): a Tantivy `Directory` that keeps the
  index encrypted on disk, with a bounded plaintext cache.

This crate does not open databases, build the search index, run queries or decide when the
vault locks. `vault-store` and the search crate do that, using what is here.

## Key hierarchy

```
password ──Argon2id──▶ KEK ──AES-256-GCM unwrap──▶ data key (random, 256-bit)
                                                      │
                                       HKDF-SHA256, no salt, info = label
                                                      │
        ┌──────────────┬──────────────┬───────────────┼──────────────┬──────────────┬──────────────┐
        ▼              ▼              ▼               ▼              ▼              ▼              ▼
     archive      source evidence  intelligence     agent         scratch     search index     temp files
     database       database        database       database       database   (per-file keys)  (per-file keys)
```

- **Data key.** Created once per vault from the OS random generator. It never changes; a
  password change rewraps it. The OS keychain stores this key for auto-unlock
  (`DataKey::expose_bytes` / `DataKey::from_bytes`), as the design's §3 says.
- **Sub-keys.** `DataKey::derive(KeyPurpose)` gives each use its own key, so no two uses share
  key material. Labels are `pathkeep/v1/<purpose>`: `archive-database`,
  `source-evidence-database`, `intelligence-database`, `agent-database`, `scratch-database`,
  `search-index`, `temp-files`. Changing a label makes existing data unreadable; the tests pin
  each sub-key to a value computed with Python's `hmac`/`hashlib`.
- **One key per database.** The migration design (§1, §3) says all databases open with the
  data key itself. This crate gives each database its own derived key instead, so a weakness
  or leak in one file's encryption does not reach the others. `vault-store` passes
  `derive(…).to_hex()` to Turso.
- **Search index.** Each index file gets its own key: HKDF(search-index sub-key, salt = the
  file's random 32-byte salt from its header, info `pathkeep/v1/search-index-file`). The
  header's key id is the first 16 bytes of HKDF(search-index sub-key, info
  `pathkeep/v1/search-index-key-id`), a fingerprint that tells a file from another vault apart
  without trying to decrypt it.
- **Temp files.** Each spill file gets a key from HKDF(temp-files sub-key, a random 32-byte
  salt that exists only in memory). The salt is never written anywhere, so after a crash a
  leftover spill file cannot be decrypted by anyone, including someone with the password. The
  sub-key is mixed in so the keys still depend on a secret if the OS random generator ever
  produced predictable output.

## Password envelope

`archive/keys.envelope`, 94 bytes, version 1:

| offset | size | field                                    |
| -----: | ---: | ---------------------------------------- |
|      0 |    5 | magic `PKENV`                            |
|      5 |    1 | version `1`                              |
|      6 |    4 | Argon2id memory, KiB (u32 little-endian) |
|     10 |    4 | Argon2id iterations (u32 LE)             |
|     14 |    4 | Argon2id lanes (u32 LE)                  |
|     18 |   16 | Argon2id salt                            |
|     34 |   12 | AES-GCM nonce                            |
|     46 |   32 | AES-256-GCM ciphertext of the data key   |
|     78 |   16 | AES-GCM tag                              |

- Version 1 means Argon2id (v0x13) and AES-256-GCM. Bytes 0..46 are the associated data, so
  the parameters can be raised later but cannot be edited without failing authentication.
- The KDF runs before authentication can say anything, so `parse` rejects parameters above
  2 GiB of memory, 64 iterations or 16 lanes. An edited envelope cannot make unlock hang or
  run the machine out of memory.
- Every wrap draws a fresh salt and nonce.
- `write_envelope_file` replaces the file atomically: write `.keys.envelope.pk-tmp`, fsync it,
  rename it over the envelope, fsync the directory. A crash leaves the old envelope or the
  new one. At most one stale temporary file can be left, and it holds only an envelope.

### Argon2id parameters

`KdfParams::RECOMMENDED` is **m = 256 MiB, t = 3, p = 4** (RFC 9106's second recommendation,
64 MiB / t=3 / p=4, with four times the memory; the parameters ADR-012 accepted). Measured with
`cargo test --release -p vault-crypto --test envelope -- --ignored --nocapture kdf_timing`
on the development machine (Apple Silicon, 18 cores, shared with other work, load averages
4.41 / 9.96 / 11.71), median of 5 unlocks, including wiping the Argon2 memory afterwards:

| m       | t   | p   | unlock |
| ------- | --- | --- | -----: |
| 64 MiB  | 3   | 4   |  60 ms |
| 256 MiB | 3   | 4   | 286 ms |
| 256 MiB | 4   | 4   | 377 ms |
| 512 MiB | 2   | 4   | 392 ms |
| 512 MiB | 3   | 4   | 590 ms |

- `argon2` 0.5.3 has no parallel implementation: the four lanes run one after another on one
  core, and `p` only changes the output. They are 4 so that a parallel implementation can use
  the target machine's four cores later without changing the format.
- Peak memory during unlock is about the memory parameter (part-b measured 263 MiB for
  256 MiB), about 3% of an 8 GB machine, and it is freed and wiped right after.
- The 4-core, 8 GB target was not measured. Its single-core speed decides the unlock time.

## Turso spill files

Turso 0.8.1 writes sort, GROUP BY, hash-join, ephemeral-table and index-build spills to
`$TMPDIR/<random dir>/tursodb_temp_file` in plaintext (part-b). `TempCryptIo` is the IO layer
`vault-store` passes to `turso::Builder::with_io_impl`:

- Files named exactly `tursodb_temp_file` are encrypted with AES-256-CTR, keystream position =
  file offset, under a per-file key that exists only in memory. Reads are decrypted inside the
  read completion, before the engine's callback runs.
- Every other file (databases, WALs, `.tshm`) goes to the platform IO untouched; Turso's own
  page encryption covers them. Every `IO` method forwards to the platform IO, so its choices
  (shared-WAL coordination, completion polling) stay in effect.
- Not covered: Turso's per-connection TEMP database (`CREATE TEMP TABLE`, `temp.`), which Turso
  opens through a fresh IO of its own, in plaintext. PathKeep must not use TEMP tables; scratch
  work goes to the ATTACHed, encrypted `derived/scratch.db` (`KeyPurpose::ScratchDatabase`).
- Not covered: integrity of spill bytes (confidentiality only), and CTR keystream reuse when
  Turso rewrites a spill page in place: two captured versions of one region XOR to the XOR of
  their plaintexts. The key dies with the process.

### Rerun on every Turso upgrade

The layer recognizes spill files **by Turso's internal file name**. If an upgrade renames the
file or adds a spill path, plaintext reaches disk and nothing in this crate notices. On every
Turso version change, run

```sh
cargo test --release -p vault-crypto --test temp_io -- --nocapture
```

and check that each case reports a spill file with plaintext under the platform IO (the
positive control) and none under `TempCryptIo`, with identical rows. The debug run of the same
test is part of `cargo test`, at a smaller scale (Turso's hash table spills above 32 KiB in
debug builds but only above 64 MiB in release builds, so release needs 1M rows).

Result on Turso 0.8.1 (release, 250,000 pages / 1,000,000 visits, encrypted archive, page
cache 256 KiB, load 5.3 / 5.7 / 5.8). The test sets `$TMPDIR` in a child process, records
every byte written through the IO layer underneath `TempCryptIo`, and hard-links every file
that appears in `$TMPDIR` or the database directory so deleted spill files can still be
scanned:

| case                                                                          | spill (peak) | platform IO: plaintext | `TempCryptIo`: plaintext | rows      |
| ----------------------------------------------------------------------------- | -----------: | ---------------------- | ------------------------ | --------- |
| `ORDER BY` over visits ⋈ pages                                                |      86.1 MB | yes                    | no                       | identical |
| `GROUP BY url`                                                                |      58.6 MB | yes                    | no                       | identical |
| hash join on a computed key                                                   |      72.3 MB | yes                    | no                       | identical |
| `CREATE INDEX pages(title, url)`                                              |      21.5 MB | yes                    | no                       | identical |
| `CREATE TABLE scratch.work AS …` + `GROUP BY` on an ATTACHed scratch database |     104.1 MB | yes                    | no                       | identical |

The test's timings are not an overhead measurement: its recorder scans every written byte,
which costs more on ciphertext (no early match) than on plaintext. Part-b measured the layer
alone at −0.2% to +21%.

## Encrypted search index

`EncryptedDirectory::open(dir, &data_key, cache_bytes)` is a `tantivy::Directory`.
`DEFAULT_CACHE_BYTES` is 64 MiB.

### File format, version 1

Every file Tantivy writes, segment files and `meta.json` / `.managed.json` alike:

| part   | bytes        | contents                                                            |
| ------ | ------------ | ------------------------------------------------------------------- |
| header | 53           | magic `PKSI` (4), version `1` (1), key id (16), file salt (32)      |
| blocks | 65,552 each  | AES-256-GCM ciphertext of 64 KiB of plaintext, then its 16-byte tag |
| last   | 16 to 65,552 | ciphertext of the remaining 0 to 64 KiB, then its tag               |

- Every block but the last holds exactly 64 KiB of plaintext, so the plaintext length and every
  block's position follow from the file size, without decrypting.
- File key: HKDF-SHA256(search-index sub-key, salt = file salt, info
  `pathkeep/v1/search-index-file`). The salt is new for every file written, including every
  rewrite of `meta.json`, so no two files share a key.
- Nonce: the block index (u64 little-endian), then four zero bytes. Each key seals each block
  index once.
- Associated data of block `i`: the header, `i` (u64 LE), the block's plaintext length (u32
  LE), a final-block flag (1 byte), and the file's name in the index directory.
- `tests/fixtures/search-index-v1.idx` is a two-block file built from this description by
  `tests/fixtures/make_search_index_vector.py` (Python `hmac` + `cryptography`); a test reads
  it through `EncryptedDirectory`.

### Writing

- `open_write` creates the file (`FileAlreadyExists` if it exists), writes the header, and
  returns a writer that seals each 64 KiB block as soon as more data follows it. Plaintext
  only ever sits in the writer's memory buffer, which is wiped on drop.
- `terminate` seals the last block with the final flag and fsyncs. A file whose writer never
  terminated (crash, error) has no final block and fails to open with `Incomplete`.
- `flush` writes nothing, as with `MmapDirectory`: sealing a short block mid-file would break
  the fixed layout, and Tantivy only reads files it has terminated (a segment's files are named
  in `meta.json` after they are finished).
- `atomic_write` (`meta.json`, `.managed.json`) seals the whole content in memory, writes it
  to `.<name>.pk-tmp`, fsyncs, renames over the target and fsyncs the directory. The
  associated data names the target, not the temporary file. Writing `meta.json` notifies
  `watch` subscribers.

### Reading and the cache

- `get_file_handle` checks the header and decrypts the last block (final flag required), so a
  file cut at a block boundary fails at open whatever Tantivy reads first.
- Reads smaller than 64 KiB copy out of cached 64 KiB blocks. Reads of 64 KiB or more are
  cached as whole ranges and handed to Tantivy without copying: Tantivy asks for the same
  large ranges on every query (fast-field columns, field norms, a term's postings), which
  `MmapDirectory` serves for free. The first benchmark run copied them per query and the
  encrypted warm query took 0.93 ms instead of 0.12 ms; with range entries both take 0.12 ms.
- One LRU byte budget covers both kinds of entry. An entry larger than the budget is not
  cached. Entries are keyed by SHA-256 of the file's salt, length and name, so a file that was
  copied over another name, cut, or extended never reuses blocks verified for the original
  (the tamper test caught exactly that when the key was the salt alone).
- `clear_cache()` drops every entry. Every plaintext buffer, in the cache or handed to
  Tantivy, wipes itself when its last reference goes. Tantivy keeps references in its readers
  (term dictionaries, open columns), so on lock the app drops its `IndexReader`s and `Index`,
  then calls `clear_cache()`, then drops the directory.
- `cache_stats()` reports bytes, entries, hits and misses.

### Locks, notifications, crashes

- `acquire_lock` takes an OS file lock (`File::lock` / `try_lock`), released when the lock is
  dropped or the process dies. Tantivy's default implementation creates a lock file and
  deletes it on drop, which a crash never does, so every later writer would fail with
  `LockBusy`. A test kills a child holding the writer lock and takes it.
- `watch` callbacks fire when `meta.json` is replaced through this directory. PathKeep has one
  process that owns the index; changes made by another process are not noticed (unlike
  `MmapDirectory`, which polls the file).
- After a crash: `meta.json` names the last commit; files written since are unfinished or
  unreferenced, and Tantivy's garbage collection removes them on the next commit.

### Errors

- Failures are `io::Error`s of kind `InvalidData` wrapping `IndexFileError`: `NotEncrypted`,
  `UnsupportedVersion`, `WrongKey` (another vault's index: rebuild it), `Incomplete`
  (unfinished or cut at a block boundary) and `Tampered` (with the file name and block). Its
  `Debug` prints the same sentence as `Display`, because Tantivy formats the `io::Error` with
  `{:?}`.
- `IndexFileError::find_in(&error)` finds it inside Tantivy's errors (`TantivyError::IoError`,
  `OpenReadError::IoError`), which hold the `io::Error` in a field that is not an error source.
- `EncryptedDirectory::integrity_error()` returns the first integrity failure any read hit.
  **Tantivy 0.26.2 drops some read errors:** `SegmentReader::open` ignores any error opening a
  segment's positions file and uses an empty one (`segment_reader.rs:162–168`, meant for
  indexes without positions), so the first query on a text field then fails with "Data
  corruption: Failed to open field …'s positions in the composite file. Has the schema been
  modified?". Reproduction: `tests/search_tamper.rs`, whose output lists every damage that
  makes a `.pos` file fail to open (a cut, an edited header, another file's bytes). The
  search crate should check `integrity_error()` whenever Tantivy reports data corruption.

### Rerun on every Tantivy upgrade

The directory relies on how Tantivy 0.26 uses it: files are written once and terminated
before they are read; `meta.json` is the commit point and is written with `atomic_write`;
readers reload on its notification; the positions-file behaviour above. On every Tantivy
version change, run the default tests and the ignored crash tests:

```sh
cargo test -p vault-crypto
cargo test --release -p vault-crypto --test search_crash -- --ignored --nocapture
```

## Threat model

The attacker this protects against has the disk, or a copy of it (a stolen or lost laptop, a
backup, a synced folder), while the vault is locked or PathKeep is not running, and may change
files on it.

Protected:

- Contents. The databases (Turso's own page encryption, under keys from this crate), the
  search index, Turso's spill files and the data key in the envelope are ciphertext. No
  plaintext is written, even temporarily, by the parts in this crate; the tests scan every
  byte that reaches disk.
- Changes to the search index and the envelope. Any edited, swapped, moved, truncated or
  appended block, any file moved to another name, and any edited header fail with an error;
  the index never answers from changed data.
- The password. Argon2id with 256 MiB makes guessing expensive; the KDF parameters cannot be
  weakened without failing authentication.
- Crashes. A kill at any point leaves the envelope and the index in their previous or next
  state, never neither.

Not protected:

- File names, sizes, counts and times. Segment ids, file extensions and sizes are visible, so
  the rough size of the index and when it changed can be read off the disk. The envelope and
  lock files are visible as such.
- An older, complete version of a file put back in place (for example a previous `meta.json`,
  or an old envelope with an old password). Every version is authentic. The search crate has
  to notice an index that is behind the archive.
- Spill-file integrity, and CTR keystream reuse when Turso rewrites a spill page (see above).
- Turso's TEMP database: PathKeep must not use TEMP tables.
- Someone who can read the process's memory while the vault is unlocked: keys and plaintext
  are there by necessity. After lock, what this crate allocated is wiped, but plaintext
  Tantivy copied into its own structures (decompressed document blocks, decoded postings) is
  freed without wiping. Swap and hibernation files are the operating system's to protect.
- SSD remapping: overwritten or deleted ciphertext may survive in flash. It is ciphertext.
- Access patterns: which blocks are read, and when, is visible to whoever watches the disk.

## Build flags

- `.cargo/config.toml` sets `--cfg aes_armv8 --cfg polyval_armv8` on 64-bit ARM. Without them
  the `aes` and `polyval` crates use software AES and GHASH on Apple Silicon (part-b measured
  a 7× slowdown for AES-CTR spills). x86_64 detects AES-NI and CLMUL at runtime. A `RUSTFLAGS`
  environment variable replaces the config's flags entirely (`cargo llvm-cov` sets one), so
  coverage builds run the software fallback: still correct, only slower.
- `src-tauri/Cargo.toml` builds the RustCrypto primitives (`argon2`, `blake2`, `aes`,
  `aes-gcm`, `ghash`, `polyval`, `ctr`) with `opt-level = 3` in dev and test builds. A debug
  unlock took 6.6 s unoptimized; with the override and the Argon2 memory wiped block by block
  it takes about 0.5 s.

## Dependencies

New to the workspace:

| crate        | version | maintainer                                                                    | used for                                                                    |
| ------------ | ------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `argon2`     | 0.5.3   | RustCrypto ([password-hashes](https://github.com/RustCrypto/password-hashes)) | password → KEK                                                              |
| `aes-gcm`    | 0.10.3  | RustCrypto ([AEADs](https://github.com/RustCrypto/AEADs))                     | envelope, search index blocks                                               |
| `aes`        | 0.8.4   | RustCrypto ([block-ciphers](https://github.com/RustCrypto/block-ciphers))     | spill-file encryption                                                       |
| `ctr`        | 0.9.2   | RustCrypto ([block-modes](https://github.com/RustCrypto/block-modes))         | spill-file encryption                                                       |
| `hkdf`       | 0.12.4  | RustCrypto ([KDFs](https://github.com/RustCrypto/KDFs))                       | sub-keys, per-file keys                                                     |
| `zeroize`    | 1.8.2   | RustCrypto ([utils](https://github.com/RustCrypto/utils))                     | wiping keys and buffers                                                     |
| `tantivy`    | 0.26.2  | Quickwit ([quickwit-oss/tantivy](https://github.com/quickwit-oss/tantivy))    | the `Directory` trait                                                       |
| `ownedbytes` | 0.9.0   | Quickwit (part of the Tantivy repository)                                     | `StableDeref`, which `OwnedBytes::new` needs and Tantivy does not re-export |
| `turso_core` | =0.8.1  | Turso ([tursodatabase/turso](https://github.com/tursodatabase/turso))         | the `IO` trait `TempCryptIo` implements                                     |
| `turso`      | =0.8.1  | Turso                                                                         | tests only (dev-dependency)                                                 |

RustCrypto is a well-known organization, which AGENTS.md accepts in place of the star count.
Tantivy (16k+ stars) and Turso were accepted by ADR-012. `tantivy` is a normal dependency with
default features off; the tests turn the defaults on for `MmapDirectory`, the benchmark's
baseline. `vault-store` depends on the same exact Turso version, which is what makes
`TempCryptIo` an `IO` that `turso::Builder` takes. Already in the workspace: `sha2`
(RustCrypto), `getrandom` (rust-random), `thiserror`; `libc`, `tempfile` and `tokio` for
tests. Adding Turso raised three existing lockfile entries to versions it requires (`cc`
1.2.61 → 1.6.0, `bytemuck` 1.25.0 → 1.25.2, `find-msvc-tools` 0.1.9 → 0.1.14).

## Tests

`cargo test -p vault-crypto` runs everything except the crash campaigns and the benchmark
(3 min 24 s in a debug build on the development machine). Crash tests re-run their own test
binary as a child process and SIGKILL it; the `crash-points` feature (test-only; the crate
enables it for itself as a dev-dependency) lets a child stop at a named step, optionally for
one file (`atomic-replace/renamed@meta.json:2`), so the parent can kill it exactly there.
Random kill times print their seed; `VAULT_CRYPTO_SEED` replays a run.

| test file                 | what it proves                                                                                                                                                                                                                                   |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `tests/envelope.rs`       | envelope format against an independent implementation, wrong passwords, every flipped bit, parameter limits, rewrap, sub-key values and separation, no key bytes in `Debug`                                                                      |
| `tests/envelope_crash.rs` | SIGKILL at each step of the atomic replacement, and at 30 random moments in a loop of password changes: the envelope always opens, with the old or the new password, to the same data key                                                        |
| `tests/temp_io.rs`        | Turso spills for sort, GROUP BY, hash join, CREATE INDEX and an ATTACHed scratch database hold no plaintext through `TempCryptIo`, while the same queries without it do; rows are identical                                                      |
| `tests/search_dir.rs`     | Tantivy's directory contract; file sizes around block boundaries; the independent format vector; unfinished, foreign, renamed, cut and extended files; watch; locks, including one held by a killed process; the cache limit; concurrent readers |
| `tests/search_index.rs`   | 100,000 pages through Tantivy: commit, merge, reopen, exact counts and top-100 against `MmapDirectory`; no plaintext in any file ever written; readers during commits and merges; resident memory under the cache limit                          |
| `tests/search_tamper.rs`  | every kind of damage in `format.rs` on a real index: an integrity error, never different results or a panic                                                                                                                                      |
| `tests/search_crash.rs`   | ignored: SIGKILL during commits (random and at each step of replacing `meta.json` and `.managed.json`) and during merges                                                                                                                         |
| `tests/search_bench.rs`   | ignored: the benchmark below                                                                                                                                                                                                                     |

### Results (2026-10-04, development machine)

- **100,000 pages** (debug build): 4 commits, then one merge into a single segment; reopened
  with a fresh directory. Counts equal the generator's for `the` (46,495 pages), `best`
  (14,748), a medium word (94), a rare word (2) and the sentinel word (101); every top-100
  equals `MmapDirectory`'s (scores, and ids above the 100th score). Stored titles and URLs
  come back exactly.
- **No plaintext**: a watcher hard-linked all 60 files the encrypted build wrote (46 MB at
  their largest, including segments the merge deleted) and none contains the sentinel word,
  the sentinel domain, a schema field name or the commit payload. The same scan of the
  `MmapDirectory` build (uncompressed stored fields) finds all four.
- **Readers during writes**: 20 commits of 2,000 pages with merges running; a reader reloading
  on notifications ran 21,161 searches and one reloading by hand 3,845, each over all 21
  states, and every search saw whole commits only.
- **Tamper**: 83 kinds of damage over the 14 files of a 10,000-page, two-segment index (the
  damaged positions are random; this run's seed is printed). 71 stopped the search with
  `IndexFileError`; 10 with Tantivy's own error while `integrity_error()` named the damaged
  file (all of them damage that makes a `.pos` file fail to open); 2 were in blocks the search
  never read, and its results were unchanged. Reading every file through the directory caught
  all 83. No different results, no panic.
- **Cache bound** (debug, 100,000-page index, 16.6 MiB on disk; each run a fresh process
  reading every stored document twice plus the probe queries): with a 2 MiB limit the cache
  held at most 2.00 MiB and resident memory grew 11.4 MiB; unbounded, the cache grew to
  9.91 MiB and resident memory 18.8 MiB. The rest of the growth is Tantivy's own structures.
- **Crash, commits** (release, `--ignored`): 24 kills at random times over about four
  commits of 5,000 pages, and 18 kills at named steps (temp file written, synced, renamed;
  `meta.json` and `.managed.json`; 1st, 2nd and 7th time). Every time the index opened at the
  acknowledged commit or the one after it (exactly the one after only when killed after the
  `meta.json` rename), with exact counts, no plaintext, a working writer lock, and nothing but
  managed files left after the next commit.
- **Crash, merges** (release): 24 kills at random times up to 1.2 times the length of a merge
  of 8 segments (0.30 s), in each of seven runs: between 18 and 21 reopened before the merge and
  the rest after it, always with exact counts, no plaintext, and a clean next commit.
- **Envelope**: 4 kills at named steps, 30 at random times (14 left a stale temporary file);
  the envelope always opened.

### Benchmark

`cargo test --release -p vault-crypto --test search_bench -- --ignored --nocapture`: 1,000,000
page-shaped documents (titles of 3 to 11 words from a skewed 40,000-word vocabulary, URLs
`https://<domain>/<words>/<id>`), built into each directory with the same settings
(Tantivy's defaults, LZ4 stored fields), two indexing threads, merged into one segment.
Every query row is a fresh process: "cold" is its first query, with an empty block cache and
nothing mapped, but the OS file cache was not purged (that needs root), so it is a cold
process, not a cold disk. "Warm" is the median of 20 more runs; "4 threads" runs the query
20 times on each of four threads at once. A query is a title term, top 100, plus each hit's
page id from its fast field. `score` lets Tantivy skip postings (Block-WAND);
`score+recency` breaks ties by the last-visit fast field, so every match is scored. Both
directories returned the same results for every query. Load averages 12.2 / 10.0 / 10.7
(other work was running); the data took 296 MiB and was deleted afterwards.

| build          | encrypted | `MmapDirectory` |
| -------------- | --------: | --------------: |
| index 1M pages |     2.5 s |           2.5 s |
| index + merge  |     5.6 s |           4.8 s |
| size on disk   | 148.2 MiB |       148.2 MiB |
| peak RSS       |   384 MiB |         521 MiB |

| query                   |    hits | cold, encrypted | cold, mmap | warm, encrypted | warm, mmap | 4 threads, encrypted | 4 threads, mmap | peak RSS, encrypted | peak RSS, mmap |
| ----------------------- | ------: | --------------: | ---------: | --------------: | ---------: | -------------------: | --------------: | ------------------: | -------------: |
| `review`, score         | 117,250 |          2.4 ms |     0.3 ms |         0.12 ms |    0.12 ms |              0.12 ms |         0.12 ms |              11 MiB |         11 MiB |
| `the`, score            | 466,747 |          2.8 ms |     0.4 ms |         0.17 ms |    0.17 ms |              0.17 ms |         0.17 ms |              12 MiB |         11 MiB |
| rare word, score        |       5 |          2.2 ms |     0.1 ms |         0.03 ms |    0.03 ms |              0.03 ms |         0.03 ms |              11 MiB |          9 MiB |
| `review`, score+recency | 117,250 |          3.5 ms |     0.8 ms |         0.60 ms |    0.73 ms |              0.63 ms |         0.68 ms |              12 MiB |         11 MiB |
| `the`, score+recency    | 466,747 |          5.8 ms |     2.6 ms |         2.32 ms |    2.38 ms |              2.42 ms |         2.38 ms |              12 MiB |         12 MiB |

- Opening the index took 0.3 ms either way. The encrypted cold cost is decrypting what the
  first query touches: 2.3 to 3.2 MiB of plaintext ended up in the cache.
- Warm, the two are the same, also with four threads: the cache lock did not show.
- The first run of this benchmark, before large reads were cached as shared ranges, measured
  0.93 ms warm for `review` (copying the fast-field column and field norms on every query).
- `MmapDirectory`'s peak RSS counts only the mapped pages it touched; the encrypted
  directory's includes its cache. Neither includes the OS file cache.

## Limits and open points

- The 4-core, 8 GB target was not measured: unlock time, query latency and memory there are
  unknown. Everything above ran on an 18-core development machine shared with other work.
- The ranges Tantivy reads per query grow with the index: field norms are one byte per
  document per scored field, and fast-field columns a few bytes per document. For the
  2.9 million pages of a 14.4M-visit archive (the research fixture), the product's ten
  weighted fields need about 29 MB of norms. While they fit in the cache they are shared
  without copying; when they do not, each query decrypts them again. The search crate has to
  size the cache from that measurement; 64 MiB is only the default.
- Windows was not tested: `sync_directory` is a no-op there, and a file that a searcher still
  holds cannot be deleted (Tantivy's garbage collection retries later, as with
  `MmapDirectory`).
- `clippy.toml` still says `msrv = "1.85.0"` while `rust-toolchain.toml` pins 1.96 (wasmtime
  needs 1.95). `File::lock` is from Rust 1.89, so `acquire_lock` carries
  `#[clippy::msrv = "1.89"]`. Raising the workspace `msrv` would make that unnecessary.
