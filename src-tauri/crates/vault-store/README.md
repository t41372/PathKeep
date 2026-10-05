# vault-store

PathKeep's synchronous boundary for its own Turso databases. It opens files,
owns connections, schedules transactions, applies migrations and maintains WALs.
It contains no business schema, IPC, password derivation or cipher implementation.
Browser-owned SQLite files do not belong here.

## API

```rust,no_run
use std::sync::Arc;
use vault_store::{Config, Db, Migration, Priority, Result, Store};

fn open_archive() -> Result<Arc<Store>> {
    let mut config = Config::new("/path/to/archive-root");
    config.key = None; // Or Some(already_derived_32_byte_database_key).
    config.migrations.register(Db::Archive, vec![
        Migration::sql(1, "CREATE TABLE notes(page_id INTEGER PRIMARY KEY, body TEXT)"),
    ]);
    let store = Arc::new(Store::open(config)?);
    store.write(Db::Archive, Priority::Interactive, |tx| {
        tx.execute("INSERT INTO notes VALUES (?1, ?2)", (42, "Remember this"))?;
        Ok(())
    })?;
    let body = store.read(Db::Archive, |conn| {
        conn.query_one("SELECT body FROM notes WHERE page_id=?1", [42], |row| {
            Ok(row.get::<String>(0)?)
        })
    })?;
    assert_eq!(body, "Remember this");
    Ok(store)
}
```

Call `read` and `write` from Tauri's `spawn_blocking` threads. Both wait for a
reply; neither belongs on the frontend or an async executor's polling thread.
Closures and returned values must be `Send + 'static`. A `Session` is borrowed,
not Send or Sync, and cannot escape its closure. It exposes synchronous
`execute`, `query_one`, `query` (for bounded result pages), and `for_each` (row
streaming). It never exposes a Turso connection, statement, or cursor. Nested
synchronous Store requests from an owner thread return `Reentrant`.

`Db` identifies Archive, SourceEvidence, Intelligence, Agent and Scratch. The
caller can replace every `config.paths` entry. Defaults under `config.root` are:

| Database | Path |
| --- | --- |
| Archive | `archive/history-vault.db` |
| SourceEvidence | `archive/source-evidence.db` |
| Intelligence | `derived/intelligence.db` |
| Agent | `derived/agent.db` |
| Scratch | `derived/scratch.db` |

Use the same root for every attempt to open a given set of files. The root is
the ownership identity, not a discovery mechanism for unrelated file sets.
Before accessing database files, open takes a nonblocking exclusive OS lock on
`archive/.pk-owner.lock`. Rust 1.96's `File::try_lock` uses `flock` on Unix and
`LockFileEx` on Windows. A loser gets `OwnedElsewhere`. The lockfile is never
unlinked: deleting it could let two processes lock different inodes.

Open first verifies the schema and migration version of **all existing files**
through read-only handles on a dedicated thread. Only after that succeeds does
it create missing files, reset scratch and start migrations. A wrong/missing key
or a newer schema therefore cannot trigger an earlier database's migration or
scratch cleanup. Scratch's contents are disposable on every successful open.
Migrations across different databases are not one atomic transaction.

## Connections, queues and transactions

Each database has one writer thread and a fixed reader pool (default: two).
Every thread creates, uses and drops its own connection. Sharing the database
handle is safe; cloning a connection would share transaction ownership and is
never used by this crate. Each thread drives Turso with its own current-thread
Tokio runtime. Only jobs and decoded results cross channels.

Every request owns one transaction. Readers have `query_only=1` and use short
read transactions. Keep each read closure bounded; use `walk` for long scans.
The crate cannot forcibly shorten an arbitrary user closure or interrupt a
running Turso statement. Writes use deferred `BEGIN`: the queue already gives
exclusive writer ownership, and `BEGIN IMMEDIATE` would also reserve the shared
scratch attachment and cause unrelated databases to contend.

The writer has two FIFO lanes:

- `Interactive`: stars, notes, tags, settings and other direct interactions.
  The pending capacity defaults to 64 and is configurable.
- `Batch`: exactly one pending job. A backup/import producer submits one batch,
  waits for its reply, then produces/submits the next. Size each batch's input
  before capturing it in the closure. The queue cannot bound memory retained by
  arbitrary numbers of blocked caller threads.

At every dequeue, interactive jobs take precedence over the next batch. A
queued interaction waits for the currently executing transaction and earlier
interactive jobs, not the remaining import. This is ordering, not a fixed
wall-clock latency promise: statement, fsync, checkpoint and OS scheduling time
still count. Each batch is an independent committed transaction; owner crates
must include their watermark, run state and outbox changes in that transaction.

A returned error rolls back the transaction. A closure panic is caught, its
transaction rolls back, and its owner remains usable. `JobPanicked` is returned;
the application's normal panic hook still runs. Commit is acknowledged before
idle reader draining, so a reader cannot hold up a successful write reply.
Shutdown closes queues and joins all threads before releasing the ownership
file. Scratch's owners outlive the attached connections.

## Encryption and scratch

`key: Option<[u8; 32]>` is the already-derived database key. `None` selects
plaintext for all five databases. A key is hex-encoded with standard-library
formatting and passed to native `EncryptionOpts { cipher: "aegis256", hexkey }`
with `experimental_encryption(true)`. Scratch attachments use the same native
key through a bound `file:...?cipher=aegis256&hexkey=...` URI. URI paths are
percent-encoded, including non-ASCII characters and `?`, `#`, and quotes.
Configuration has no Debug implementation that could print the key.

`IoLayer = Arc<dyn turso::core::IO>` is the optional caller-supplied IO boundary,
passed unchanged to `Builder::with_io_impl`. For protected spill files, the
caller must provide its encryption IO from `vault-crypto`. Native database
page encryption alone does **not** encrypt Turso's spill files. This crate does
not claim spill protection when that IO is absent and implements no cipher.

Every connection sets `cache_size=-8192` (8 MiB), including scratch's dedicated
connections. These are main-pager/sorter settings, not total memory limits.
Turso 0.8.1 gives attached pagers their native 2,000-page cache (about 7.8 MiB
at 4 KiB pages), allocated as they are used. Its schema-qualified
`PRAGMA scratch.cache_size` incorrectly changes the main pager, so this crate
does not issue it: `translate/pragma.rs:376` uses the main pager supplied by
`connection.rs:996`, and readback at `translate/pragma.rs:847` is connection-wide.
`page_cache_and_reader_pragmas_are_applied` verifies the actual 8 MiB main setting,
FILE temporary storage and query-only readers. The attached cache is additional;
8 MiB is not a hard total allocation ceiling for a connection.
All connections use WAL, `synchronous=FULL`, and `temp_store=FILE`. MVCC,
experimental multiprocess WAL, native FTS and Turso's default global allocator
feature are not enabled.

Write scratch through `write(Db::Scratch, ...)`, naming its tables without a
`scratch.` qualifier. Read them from other databases as `scratch.table_name`.
This preserves one scratch writer. Only transactions that actually mention
scratch join its reader admission accounting; an unrelated archive batch must
not delay scratch checkpointing or other readers.

## Checkpoints

Every batch request is followed by an explicit PASSIVE checkpoint. Results are
consumed and the busy column is inspected. A busy checkpoint is not reported
as successful reclamation and does not turn an already committed job into a
retryable write failure.

When its write queue is idle, the writer reserves reader admission, lets existing
short reads finish, then runs TRUNCATE with no active readers. New readers can
wait for this maintenance window, not for a running import transaction. This
brief admission pause prevents overlapping short readers from keeping the WAL
alive forever. The last reader's exit wakes an idle writer; queue notifications
are stateful to avoid losing that wakeup. Admission resumes after the checkpoint.
`metrics(Db)` exposes pending counts, checkpoint counts, busy results, frame
counts and the last maintenance error with its cause. An unsuccessful checkpoint
is retried at a later job or reader-completion boundary, without a busy loop.

**Measured correction to the probe report:** stock cached Turso 0.8.1 ignores
`wal_autocheckpoint=0`, but it does not backfill on *every* small commit. Its
`WalFile::should_checkpoint` checks whether unbackfilled frames exceed 1,000
(`storage/wal.rs:4099`, threshold initialization at 4845); the commit state
machine only enters AutoCheckpoint when that condition holds
(`storage/pager.rs:4761`). The test
`autocheckpoint_pragma_is_ignored_and_native_threshold_is_measured` observes:

- the pragma readback returns no rows;
- a 1,000-row small-payload commit leaves the main file at 8,192 bytes;
- the following 2,000-row larger-payload commit grows it to 9,244,672 bytes,
  **before** vault-store's explicit checkpoint.

The explicit policy above does not depend on the native automatic threshold. The 100-batch test
measured a 168,952-byte WAL peak with continuously active short readers, then
zero bytes when idle. WAL size depends on batch payload size and reader lifetime;
this is not a bound for a caller holding a snapshot indefinitely.

## Migrations and cancellation

Register positive, strictly increasing versions for each database. Gaps are
allowed. `Migration::sql` executes one statement; use `Migration::rust` for a
step with several statements or data transformation. Each pending step and its
entry in `__pk_migrations` commit atomically on that database's writer during
open. A failed step leaves earlier committed steps applied and is retried on the
next open. A stored version newer than the supplied list returns `SchemaTooNew`.
Read requests never initialize or migrate schemas.

`CancelToken` is a clonable atomic signal, independent of queue admission.
`check()` returns `Cancelled`. `Store::walk` fetches a caller-defined keyset page
with a nonzero row bound, ends that page's read transaction, checks cancellation,
and calls the consumer on the requesting thread. The consumer returns the next
cursor. Each fetch must put the supplied row limit in its query; an oversized
returned page is rejected. Each page gets a fresh committed snapshot. An export
that needs a fixed range should capture an immutable upper watermark, rather
than pinning one read transaction for the entire archive. Cancellation is checked
before and after each page and consumer call; it cannot stop an executing statement.

## SQL restrictions

All SQL enters through the facade. There is no raw connection accessor. The
SQL policy decodes identifiers and skips comments and string literals before
Turso prepares the statement; the tests execute the facade and compile ownership
violations, rather than grepping source text at runtime.

TEMP/TEMPORARY tables, the `temp` namespace, VACUUM (including VACUUM INTO),
arbitrary ATTACH/DETACH, PRAGMA and transaction control are rejected. Migration
metadata and pragma table functions are private. SQL is one statement per call;
use multiple calls within the closure for multi-statement work. Multi-statement
trigger definitions are outside this SQL surface. Values belong in bound
parameters. Scratch-qualified mutations are rejected on attached connections;
use its dedicated writer. Reader connections additionally enforce query-only
mode in Turso, including for a mutation hidden behind a WITH clause.

`Error` classifies ownership, wrong/missing keys, busy snapshots/locks, corruption,
cancellation, too-new schemas and other engine failures. Engine failures retain
`std::error::Error::source`; cooperative cancellation has no underlying engine
cause. Native decryption errors do not distinguish a wrong key from damaged
authentication data. No business code should inspect engine error strings.

## Dependencies and validation

- `turso = "=0.8.1"`: the vetted engine, exactly pinned; default FTS/allocator
  features disabled. Its native transitive dependencies remain engine-owned.
- Workspace `tokio`: its established current-thread runtime drives Turso futures
  privately. The business API stays synchronous.
- Workspace `thiserror`: preserves typed errors and source chains without custom
  repetitive Error implementations.
- Workspace `tempfile` (tests only): isolated real databases and automatic cleanup.
- OS locking, queueing, reply channels and cancellation use only `std`.

Rust 1.96 matches the repository toolchain. The crate-local Clippy configuration
uses that MSRV; the repository-wide legacy setting is still 1.85. No other
product crate or frontend file was changed.

Run from `src-tauri`, with locally cached dependencies for an offline run:

```sh
CARGO_NET_OFFLINE=true cargo fmt --all --check
CARGO_NET_OFFLINE=true cargo clippy -p vault-store --all-targets -- -D warnings
CARGO_NET_OFFLINE=true cargo test -p vault-store
CARGO_NET_OFFLINE=true cargo build --workspace
CARGO_NET_OFFLINE=true cargo test -p vault-store --release --test benchmark \
  -- --ignored --nocapture --test-threads=1
```

Each module's header lists the ways it can fail and the tests that cover them.
