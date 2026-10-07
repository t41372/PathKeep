# vault-convert-sqlcipher

Reads existing PathKeep SQLCipher/SQLite files through read-only rusqlite
connections and writes native databases through `Vault` in 4,096-row
`Priority::Batch` pages. Browser parsing, UI, encryption primitives and AI
algorithms belong to their existing crates.

`assess` reports source file sizes including existing WAL/SHM sidecars, table counts, destination paths, retained
legacy paths and whether a conversion can resume. `requiredFreeBytes` reserves
three times the source database bytes for staging and search; it is a planning
reserve, not a guaranteed upper bound for arbitrary index content.

`convert` acquires ownership; `convert_owned` shares an app startup reservation.
The staged Vault lives under `staging/sqlcipher-conversion`. Each committed page
has a durable cursor. If the process dies before saving its cursor, replay
replaces the same source rowids rather than adding duplicate rows. Cancellation
retains this encrypted staging area for resumption.

URL pages and annotation keys use vault-core's current canonicalizer. URL stars
are normalized again, including decoding unreserved percent escapes. Search
documents and their outbox use the same writer as ingest; the owner indexer then
builds the search generation. Per-table typed-value digests and row counts must
agree before installation. Original `.sqlite` files are kept at their original
paths; removing them is a separate user decision.

Canonical enrichment-job and refind collisions merge before they reach the unique
indexes: newest timestamp content, summed counters, unioned sets, stable minimum
source IDs, and all job triggers remapped to their surviving job. Offset-bearing
RFC3339 timestamps compare as instants; equal timestamps use the largest source
ID for content. The converter builds the expected groups in Scratch under the target mode,
then verifies the merged native tables. Rebuilding that scratch proof on resume
prevents counters being added twice. Source high-water marks still retire old IDs.

Old archive-only SQLCipher safety snapshots also use this converter before native
restoration. They restore an empty evidence/agent store because those files were
never part of the old snapshot; the worker reports `sourceEvidenceRebuilt: true`.

The installer uses the existing import-journal location and preserves replaced
native files until the complete generation is published. A prepared journal
rolls back on startup; a committed journal finishes cleanup.

## SQLite library

While this converter ships, **the whole application links rusqlite's
`bundled-sqlcipher-vendored-openssl` build**. Cargo feature unification makes a
second plain bundled SQLite build in the same executable impossible. Browser
databases are opened without a key, which is normal plaintext SQLite behavior.
There is no converter sidecar or separate converter executable. When the
converter is removed, the workspace feature returns to `bundled`.

## Failure coverage and measurement

Tests generate encrypted legacy archives and exercise old `%7E` keys, preserved
original files, native search, canonical collisions with remapped triggers,
cancellation/resume, abrupt process exit and
digest mismatch refusal. Parity tests send the same browser facts through the real
area-1 importer and compare page identities, visit times, searchable text and
profile memberships in both modes. Canonical star collisions keep the latest
star timestamp and available source profile; notes retain their raw URL plus the recomputed canonical key. `tests/benchmark.rs` is ignored by default; it generates
14.4M visits and 2.9M URLs under `/tmp` in the Modal Sandbox. Separate child
processes exclude fixture-generation RSS and measure conversion, the following
index build and an encrypted-to-plaintext mode switch.

All builds and tests run through modal-offload. No generated data or gate logs
belong in this crate.
