# vault-crypto

Encryption at rest for PathKeep, below the storage engines. It follows
[ADR-012](../../../docs/architecture/decisions/012-turso-storage-engine.md) and
[the Turso migration design](../../../docs/architecture/turso-migration-design.md) §3.

This crate does not open databases, build the search index or decide when the vault locks.
`vault-store` and the search crate do that, using what is here.

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
64 MiB / t=3 / p=4, with four times the memory). Measured with
`cargo test --release -p vault-crypto --test envelope -- --ignored --nocapture kdf_timing`
on the development machine (Apple Silicon, 18 cores, shared with other work, load averages
6.52 / 6.74 / 5.60), median of 5 unlocks, including wiping the Argon2 memory afterwards:

| m       | t   | p   | unlock |
| ------- | --- | --- | -----: |
| 64 MiB  | 3   | 4   |  78 ms |
| 256 MiB | 3   | 4   | 355 ms |
| 256 MiB | 4   | 4   | 450 ms |
| 512 MiB | 2   | 4   | 512 ms |
| 512 MiB | 3   | 4   | 681 ms |

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

| crate        | version | maintainer                                                                    | used for                                |
| ------------ | ------- | ----------------------------------------------------------------------------- | --------------------------------------- |
| `argon2`     | 0.5.3   | RustCrypto ([password-hashes](https://github.com/RustCrypto/password-hashes)) | password → KEK                          |
| `aes-gcm`    | 0.10.3  | RustCrypto ([AEADs](https://github.com/RustCrypto/AEADs))                     | envelope                                |
| `aes`        | 0.8.4   | RustCrypto ([block-ciphers](https://github.com/RustCrypto/block-ciphers))     | spill-file encryption                   |
| `ctr`        | 0.9.2   | RustCrypto ([block-modes](https://github.com/RustCrypto/block-modes))         | spill-file encryption                   |
| `hkdf`       | 0.12.4  | RustCrypto ([KDFs](https://github.com/RustCrypto/KDFs))                       | sub-keys, per-file keys                 |
| `zeroize`    | 1.8.2   | RustCrypto ([utils](https://github.com/RustCrypto/utils))                     | wiping keys and buffers                 |
| `turso_core` | =0.8.1  | Turso ([tursodatabase/turso](https://github.com/tursodatabase/turso))         | the `IO` trait `TempCryptIo` implements |
| `turso`      | =0.8.1  | Turso                                                                         | tests only (dev-dependency)             |

RustCrypto is a well-known organization, which AGENTS.md accepts in place of the star count.
Turso was accepted by ADR-012; `vault-store` depends on the same exact version, which is what
makes `TempCryptIo` an `IO` that `turso::Builder` takes. Already in the workspace: `sha2`
(RustCrypto), `getrandom` (rust-random), `thiserror`; `libc`, `tempfile` and `tokio` for tests.

## Tests

| test file                 | what it proves                                                                                                                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/envelope.rs`       | envelope format against an independent implementation, wrong passwords, every flipped bit, parameter limits, rewrap, sub-key values and separation, no key bytes in `Debug`                 |
| `tests/envelope_crash.rs` | SIGKILL at each step of the atomic replacement, and at 30 random moments in a loop of password changes: the envelope always opens, with the old or the new password, to the same data key   |
| `tests/temp_io.rs`        | Turso spills for sort, GROUP BY, hash join, CREATE INDEX and an ATTACHed scratch database hold no plaintext through `TempCryptIo`, while the same queries without it do; rows are identical |

The crash tests run their own test binary as a child process and kill it. The `crash-points`
feature (test-only; the crate enables it for itself as a dev-dependency) lets a child stop at
a named step so the parent can kill it there.
