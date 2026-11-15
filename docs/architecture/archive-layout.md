# Archive layout: one table for what the archive is made of

The `vault-layout` crate names every file and folder PathKeep keeps under its data folder and says what each archive operation does with it. Opening, snapshots, restore, the move bundle, turning encryption on or off, changing the password, the at-rest check, storage usage, retention, the health check and "delete all data" all read their membership from this one table.

## Why it exists

Before this crate, each of those operations kept its own list. Adding a database meant finding about fifty places; three of them stopped compiling, the rest kept compiling and left the new database out. The result of a missed list is quiet: a store that survives "delete all data", stays under an old key after a password or mode change, or is missing after a restore. The tests of the time compared each list with itself, so none of them could fail.

The table turns that into a compile error. A new artifact needs a row, and a row has to answer every operation.

## What is in the crate

`vault-layout` depends on nothing but `std` and does no file I/O.

- `identity.rs`: the identities. `Db` (the seven databases the store can open: the five of PathKeep 0.4 and 0.5, and the agent-history evidence and read model), `ArtifactId` (every database, file, folder and family of files under the data folder) and `KeyPurpose` with its key-derivation label. `vault-store::Db` and `vault-crypto::KeyPurpose` are re-exports of these.
- `policies.rs`: one small enum per operation, listing the answers that operation can get, and `LifecyclePolicies`, the struct that holds one answer per operation.
- `catalog.rs`: `spec(ArtifactId) -> ArtifactSpec`, one `match` with one arm per artifact. Each row gives the path or path family, whether the content is original, rebuildable or control data, whose data it is, the database wiring and key purpose if it is a database, its `LifecyclePolicies`, and the layout that introduced it.
- `paths.rs`: the views the operations use, for example `snapshot_databases(layout)`, `portable_files(layout)`, `wipe_targets()`, `installation_files(layout, search)`. Each is a filter over the table. Callers do not keep their own lists.

## Layouts

Not every archive holds every row. `ArchiveLayout` names what an archive holds: `BrowsingV1` is what PathKeep 0.4 and 0.5 write (five stores), `AgentHistoryV2` adds the agent-history evidence, its read model and its search index. Every row says which layout introduced it, and a layout holds every row of the layouts before it (`each_layout_holds_everything_the_one_before_held`).

The layout is not stored on its own. It follows from the archive's format revision in `config.json` (`ArchiveLayout::of_revision`; no stamp is revision 0, `BrowsingV1`), and `vault_core::admission::root_layout` reads it.

The views that list stores or files take a layout, and every caller passes the layout of the archive it is working on: the open archive's (`Vault::layout`, read when it was opened), a snapshot's or a stage's from its own `config.json`, a move bundle's from its format number. Never the newest layout by default: a restore of a snapshot taken before the agent-history stores existed must not require them, and opening an archive must not create stores its layout does not have. The views that only order (`startup_databases`), delete (`wipe_targets`) or measure what exists (`usage_members` for storage) are not tied to a layout in that way.

`ProjectPaths` in `vault-core` is still the typed set of paths the rest of the backend passes around. Its values now come from the table, and `ProjectPaths::artifact_path` resolves any artifact, honouring a caller's overridden paths where the old code did.

## What the compiler enforces

- A new `ArtifactId` without an arm in `spec` does not compile.
- A new `Db` without an artifact does not compile.
- A row without the layout that introduced it does not compile.
- A new operation adds a field to `LifecyclePolicies`, and every row fails to compile until it answers.
- A row cannot leave an answer out: there is no default and no base row.

`vault-layout/tests/compile_fail.rs` builds copies of the crate with each of these mistakes and checks the compiler's message.

The compiler cannot see a file that some code writes without declaring it. That is caught on disk, see below.

## The table records what the app does today, including what is wrong

The table was introduced without changing behaviour, so some rows hold answers that look wrong. They are the app's real behaviour, now visible in one place:

- `derived/intelligence.db` is original data (site categories the user edited, the records that locate saved page text) but the move bundle omits it. See `WORK-VAULT-FOLLOWUPS-2026-11`.
- Saved page text and the copies of browser history files stay readable in an encrypted archive. See `WORK-ENCRYPT-AT-REST-ALL`.
- After a conversion from the old format, the control it replaced stays in a `maintenance.bak-*` folder that neither storage usage nor retention counts. (What a restore or an import replaces is a safety copy under `raw-snapshots/maintenance/`, counted and freed with the others.)
- Each operation treats a database's companion files (`-wal`, `-shm`, `-tshm`) differently.

Changing one of these is a behaviour change with its own decision and test. It is made by changing the row, not by adding a special case at a call site.

## How it is tested

Three things, written by different hands on purpose:

- **What each operation leaves on disk.** `vault-worker/tests/vault_lifecycle/` runs the real owner on a synthetic data folder through first open, backup, snapshot, restore, encryption on and off, password change, move export and import, index rebuild and wipe, in plaintext and encrypted mode. After each step it lists every file and every file containing each marker string, and compares with a recorded literal in `expected/`. No expectation is computed from the table.
- **An independent list of what may exist.** `vault_lifecycle/support/expected_root.rs` classifies every path allowed under the data folder. It was written from a separate reading of the code and is not generated from the table, and the table is not generated from it. A file one of them does not know shows up as a disagreement.
- **The identities that must never change.** `vault-crypto/tests/historical_labels.rs` pins the eight key labels against independently computed keys; `vault_lifecycle/contracts.rs` pins the five database paths, opens each encrypted file with its literal path and key purpose, and reads each database's migration ledger.

When a lifecycle test fails after a table change, the table changed behaviour. If that was intended, the expectation is updated in the same commit and the commit says what changed for the user. If it was not, the row is wrong.

## Adding a store or a folder

1. Add the `ArtifactId` (and the `Db` if the store opens it) and its row. Answer every operation from what should happen to that data, not from what is convenient for the code that writes it.
2. Add its path to `expected_root.rs` by hand, with whether it must be unreadable in an encrypted archive and whether a wipe may leave it.
3. Give it a marker in the lifecycle fixture and extend the recorded expectations.
4. A new key purpose gets a new label. An existing label is never reused or edited: changing its bytes makes existing data unreadable.
