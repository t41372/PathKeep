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

## Build flags

`.cargo/config.toml` sets `--cfg aes_armv8 --cfg polyval_armv8` on 64-bit ARM. Without them
the `aes` and `polyval` crates use software AES and GHASH on Apple Silicon (part-b measured a
7× slowdown for AES-CTR spills). x86_64 detects AES-NI and CLMUL at runtime. A `RUSTFLAGS`
environment variable replaces the config's flags entirely (`cargo llvm-cov` sets one), so
coverage builds run the software fallback: still correct, only slower.

## Dependencies

New to the workspace, all from RustCrypto (a well-known organization, which AGENTS.md accepts
in place of the star count):

| crate     | version | maintainer                                                                    | used for                |
| --------- | ------- | ----------------------------------------------------------------------------- | ----------------------- |
| `argon2`  | 0.5.3   | RustCrypto ([password-hashes](https://github.com/RustCrypto/password-hashes)) | password → KEK          |
| `aes-gcm` | 0.10.3  | RustCrypto ([AEADs](https://github.com/RustCrypto/AEADs))                     | envelope wrap           |
| `hkdf`    | 0.12.4  | RustCrypto ([KDFs](https://github.com/RustCrypto/KDFs))                       | sub-keys                |
| `zeroize` | 1.8.2   | RustCrypto ([utils](https://github.com/RustCrypto/utils))                     | wiping keys and buffers |

Already in the workspace: `sha2` (RustCrypto), `getrandom` (rust-random), `thiserror`;
`libc` and `tempfile` for tests.

## Tests

| test file                 | what it proves                                                                                                                                                                            |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/envelope.rs`       | envelope format against an independent implementation, wrong passwords, every flipped bit, parameter limits, rewrap, sub-key values and separation, no key bytes in `Debug`               |
| `tests/envelope_crash.rs` | SIGKILL at each step of the atomic replacement, and at 30 random moments in a loop of password changes: the envelope always opens, with the old or the new password, to the same data key |

The crash tests run their own test binary as a child process and kill it. The `crash-points`
feature (test-only; the crate enables it for itself as a dev-dependency) lets a child stop at
a named step so the parent can kill it there.
