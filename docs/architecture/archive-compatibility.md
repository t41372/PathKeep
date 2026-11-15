# Archive compatibility: which PathKeep may open which archive

An archive outlives the build that wrote it. A user updates, goes back to an older build, restores a backup made by another version, or opens the same folder from two installed copies. This document is the rule for what a build does with an archive it did not write.

The short form: before a build creates, repairs, deletes or unlocks anything in a data folder, it reads `config.json` and decides whether the archive is one it may use. If not, it leaves the folder exactly as it found it and says why.

## Why this exists

v0.4.0 and v0.5.0 have no such check. Each acts on an archive before reading its config: it finishes an interrupted "delete all data", replays an install journal, creates folders, and only then finds out what it is looking at. The one thing they refuse is a database schema number they do not know, and their restore and move-import skip even that. v0.6 adds databases those builds do not know about; a v0.5.0 that changed encryption on such an archive would re-encrypt the files it knows and leave the rest under a key that no longer exists.

Those two releases cannot be changed. What v0.6 can do is make sure they stop early (the stamp below is a value they cannot parse, and they read the config before starting any service), and make sure no later build is ever in their position.

## The stamp

An archive has one format revision. It is written in `config.json`, in `archiveMode`:

```json
"archiveMode": {
  "formatVersion": 2,
  "mode": "Encrypted",
  "compatibility": { "revision": 1, "minReader": 1, "minWriter": 1 }
}
```

- `revision` is the archive's format.
- `minReader` is the lowest build level that can read it.
- `minWriter` is the lowest build level that can change it without damaging it.
- All three are positive integers with `minReader <= minWriter <= revision`. Anything else is a damaged control, not a version this build is too old for.

v0.4.0 and v0.5.0 write `"archiveMode": "Plaintext"` or `"Encrypted"`. That bare value is revision 0 and has no floors. Each database keeps its own schema version in its own migration table; the stamp does not repeat them.

Every build has one compiled level, `PRODUCT_ROOT_LEVEL` in `vault-core/src/admission.rs`. Nothing at run time changes it.

**The level today is 0, and this build writes no stamp.** It reads both forms, refuses every stamped archive (any valid stamp has `minReader >= 1`), and still writes the bare value, so v0.5.0 can open what it writes. The level becomes 1 in the same change that makes the build create everything revision 1 requires. A build that claimed level 1 earlier would admit an archive it cannot serve.

## The decision

With level `L` and a valid stamp:

| Condition       | Result                                                         |
| --------------- | -------------------------------------------------------------- |
| `L < minReader` | Refused: `archive-reader-too-old`                              |
| `L < minWriter` | Refused: `archive-writer-too-old`. There is no read-only mode. |
| otherwise       | Admitted, even when `revision > L`                             |

A higher revision with floors this build meets is the newer build's statement that this one is still safe. This build then uses its own layout and writes the `compatibility` object back **byte for byte** on every save, including keys it does not know. It never lowers a floor or replaces the revision with its own.

There are two outcomes, open or refuse. A read-only mode was considered and left out: the floors are in the format, so a later build can add one without a format change.

Reasons, as stable strings in errors and on the worker's stderr:

| Code                            | Meaning                                                                            |
| ------------------------------- | ---------------------------------------------------------------------------------- |
| `archive-reader-too-old`        | A newer PathKeep can read this archive; this one cannot.                           |
| `archive-writer-too-old`        | A newer PathKeep can change this archive safely; this one cannot.                  |
| `archive-config-format-too-new` | `archiveMode` has a `formatVersion` above 2.                                       |
| `archive-control-missing`       | There is an archive here and no `config.json`.                                     |
| `archive-control-unreadable`    | `config.json` exists and cannot be read.                                           |
| `archive-control-invalid`       | `config.json` is not something any PathKeep wrote.                                 |
| `archive-compatibility-invalid` | The stamp's numbers are not positive integers in order.                            |
| `archive-uninitialized`         | Not a refusal: a worker command was asked to use an archive that was never set up. |

The first three mean the archive is intact and needs a newer build. The next four mean its control is damaged or gone. These are different messages to the user and are never reported as a wrong password, a corrupt database or an empty archive.

## A folder without `config.json`

Without a control, only what is in the folder can say whether an archive was ever there. `admission::census` decides, and it reads; it creates nothing.

| What is in the folder                                                                                                                                                                         | It is                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Nothing, or the folder does not exist                                                                                                                                                         | A first run                                                               |
| Only empty folders                                                                                                                                                                            | A first run                                                               |
| Locks, the owner socket, logs, crash reports                                                                                                                                                  | A first run                                                               |
| What the scheduler keeps under `schedule/`, and the records under `audit/`                                                                                                                    | A first run                                                               |
| A staging folder an operation never finished (`staging/maintenance-*`), a leftover `.pk-durable-*` file                                                                                       | A first run; the next open deletes them                                   |
| Downloaded models, integration files, files under `exports/`                                                                                                                                  | A first run                                                               |
| Files at the top of the folder that the layout table has no name for (`.DS_Store`, an export the user saved there)                                                                            | A first run; the files are left alone                                     |
| A valid "delete all data" marker under the name PathKeep 0.5 wrote (`.pk-wipe-in-progress.json`)                                                                                              | An interrupted wipe (below)                                               |
| An install journal under the name PathKeep 0.5 wrote (`archive/.pk-import-journal.json`)                                                                                                      | An interrupted install (below)                                            |
| A "delete all data" intent or an install journal under the names this build writes (`.pk-wipe-v2.json`, `archive/.pk-install-v2.json`)                                                        | `archive-control-missing`: this build removes its control only after them |
| A PathKeep 0.3 import, rekey or restore journal, in a folder with no archive in the current format                                                                                            | An interrupted old-format operation (below)                               |
| Files the file manager writes into any folder a person opens (`.DS_Store`, `._*`, `desktop.ini`, `Thumbs.db`), at any depth                                                                   | A first run                                                               |
| Anything else under `archive/`, `derived/`, `raw-snapshots/`, `quarantine/` or `sidecars/`; a previous archive kept by a restore; an App Lock passcode; a half-finished old-format conversion | `archive-control-missing`                                                 |

Each row of the layout table (`vault-layout`) answers this question for its own file or folder, as it answers for every other operation (`CensusPolicy`): part of an archive, not part of one, or "whatever my folder says". A file the table has no row for takes the answer of the folder it lies in, and the data folder's own answer is "not part of an archive". Adding a row without answering does not compile.

Two properties have to hold, and a test holds each:

- **Whatever a finished "delete all data" leaves reads as a first run.** The test wipes a populated archive with automatic backup installed (removing the schedule writes a record after `audit/` is gone), a `.DS_Store` and an export in the folder, then initializes again.
- **Whatever the app creates on its first start, before anything is set up, reads as a first run.** The test starts the real desktop binary on an empty folder and classifies what it left.

An earlier version of this rule inferred the answer from the wipe policy ("an archive unless a wipe leaves it"). It missed, one after another, the scheduler's records, the staging folder of an import in progress, and the record a schedule removal writes during the wipe itself. Each miss would have locked users out with no way back, which is why the answer is now a column of its own.

Three things here differ from the first design, which said any unknown leftover means "recovery required":

- **Unknown files at the top of the folder do not count.** "Delete all data" does not remove an export the user saved into the data folder (a known gap in the wipe), and Finder writes `.DS_Store` into any folder it shows. Under the stricter rule a user who deleted everything could never start again. Initialization creates only paths the layout table names and removes nothing, so it cannot harm such a file. An unknown file _inside_ a folder the table owns (`archive/`, `derived/`, …) still counts: a wipe empties those folders, so anything there is not wipe residue.
- **Interrupted wipes and installs are recognized, not refused.** See the next section.
- **A staging folder nobody finished does not count.** Otherwise an import into a new data folder would refuse itself halfway, and a crash during one would lock the user out.

## Interrupted wipes and installs

PathKeep 0.5 and earlier take `config.json` away for a moment by design:

- "Delete all data" writes its marker, then removes the control first, then everything else.
- A restore, import or encryption change moves the control into a backup folder before the new one lands. Its journal records how to finish or undo that.
- PathKeep 0.3 did the same with its own import, rekey and restore journals.

A crash of one of those releases in such a window leaves an archive with no control and a record that explains why. Every release recovers from all of them automatically. A rule that refused every archive without a control would turn each of these crashes into a dead end, so the records are recognized, and such a folder behaves as it always has:

- Reading the config gives "not initialized", not an error. The recoveries are built on that: the wipe is finished at start and before initialization, a native install when the session opens, an old-format one when the shell asks for the upgrade assessment. A reader does not act on the record itself; the operation that wrote it may be running in this very process.
- The worker's command line lets such a folder through to the app, because the app it starts is what finishes the job. A scheduled backup would otherwise fail until someone opened the window.
- A wipe marker that does not decode is not acted on. The marker is written whole or not at all, so an undecodable one was not left by a wipe, and nothing else says the user asked for a deletion. The same goes for an install journal that is not JSON.
- When the control is present and needs a newer build, no record is replayed, whatever it says. Before this rule, a "delete all data" marker lying in a newer archive made an older build delete the archive at start; a test starts the real app on that folder and compares every file.
- A journal is still replayed next to a control that is merely damaged: rolling the install back is what puts the old control back.

This build never leaves a folder like that:

- **Its records have names those releases do not know** (`archive/.pk-install-v2.json`, `.pk-wipe-v2.json`) and carry an identity and a version that are checked before anything is replayed. A record this build did not write in that shape deletes and replaces nothing. A 0.5 that starts on the folder halfway through an operation of this build does not find a record to act on; once the stamp is written it stops at the config anyway.
- **An install never takes the control away.** The control is copied into the backup, not moved, and replaced by one rename on install; a rollback writes it back from that copy. An install that would leave no control is refused before it starts.
- **A journal acts only on the folder it was written in.** It records its stage and backup relative to the folder. A rollback runs only when the backup folder it names, randomly named when the install began, is in this folder: a journal carried into another archive's folder (by a copy, a merge, a backup restore) finds none and is refused before anything moves, while one in a folder that was moved whole is recovered where it is. A 0.5 journal's absolute paths are taken relative to where its backup was, by the same rule. A file that was there before the install and is now neither in place nor in the backup stops the rollback with the journal kept; it is not reported as rolled back.
- **Moves are made durable destination first.** New folders' entries are flushed before anything is moved into them, and a move flushes the destination's directory before the source's, so a power cut can leave an entry in both places, never in neither.
- **"Delete all data" keeps the control until the very end.** Order: the intent; `config.json` replaced by an empty, not-initialized one that keeps the stamp (every background worker stops when it sees that); the workers stopped; the files, including a leftover `.pk-durable-*` temporary file in the folder (a crash during an atomic write can leave a full copy of the old settings there); the keychain; the automatic backup; a scan for anything that still means an archive, by the same rule as a folder without a control, which refuses to finish while anything is left; the intent; `config.json`. A crash before the intent is removed is finished at the next start. A crash after it leaves an empty, not-initialized control in an empty folder.
- **The intent is tied to the archive the user confirmed deleting.** It records the SHA-256 of the control at confirmation and the exact emptied control it writes. An interrupted wipe is finished only while the folder holds one of the two; otherwise the folder holds another archive (restored from a backup, imported, carried in) and the record is discarded without touching it. Import, restore and conversion finish a pending wipe before they install anything, as initialization does, and refuse while one is pending. A 0.5 marker, which records nothing of the kind, is honoured as that release would, except beside a stamped control: 0.5 cannot have started a wipe of an archive it cannot read.
- **Settings writes are serialized with the wipe.** The settings save, the menu bar setting and the App Lock passcode take the same operation gate the wipe holds from its intent to its last removal, so a save that read the settings before a wipe cannot write them back after it.

`maintenance/install_tests.rs` checks the control after every step of an install and of its recovery; `archive/wipe/tests.rs` cuts the wipe at each durable step and finishes it.

## Where the decision is made

Three functions in `vault-core/src/admission.rs`, all read-only:

- `inspect(root)` classifies a folder: admitted, first run, interrupted, or refused with a reason.
- `require_control(root)` is `inspect` for readers of the config: the control, nothing (a first run or an interrupted operation), or an error.
- `guard(root)` is the check before a write: a control that is on disk must be one this build may use. A folder without a control passes. Staging folders and snapshot destinations have databases before they have a control, and whether a control-less folder is a first run is `inspect`'s question, asked where the answer changes what happens.

There is no token passed from the check to the operation. Every boundary reads the disk again, which is the stronger guarantee: a token says what the folder was, and a restore or import may have replaced the control since.

| Entry point                                                                          | Check                                                                                                                                                                                  |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop start                                                                        | `inspect`, before the window, the owner lock, folders, logs, recovery, keychain, App Lock. Again after recovery, under the lock.                                                       |
| Worker command line (backup, doctor, MCP server, …)                                  | `inspect`, before the schedule's logs are trimmed or the app is started                                                                                                                |
| Requests the owner serves                                                            | No check of their own: every route that uses the archive reads the config first. The scheduled backup, which records its attempt before reading the config, checks before that record. |
| `load_config`                                                                        | `require_control`. Never a default for a file that is there.                                                                                                                           |
| `save_config`, creating the folder layout                                            | `guard`; a save writes the stamp on disk back unchanged                                                                                                                                |
| Opening the archive (any variant), password open                                     | `guard`, before the owner lock or a key envelope                                                                                                                                       |
| Finishing a wipe                                                                     | `inspect`                                                                                                                                                                              |
| Replaying an install journal, old-format launch recovery                             | Refused only when the control needs a newer build                                                                                                                                      |
| Old-format conversion                                                                | `guard`                                                                                                                                                                                |
| Mode change, password change, snapshot, export, retention, checkpoint replay, backup | `guard` again when the operation starts (`Vault::recheck_root`)                                                                                                                        |
| Restore and move-import                                                              | Target: refused only when its control needs a newer build. Source: its own control is decided before anything is staged in the target.                                                 |
| Keychain reads and writes, provider keys, installing or removing the schedule        | `guard`, before the machine is touched                                                                                                                                                 |
| Standalone agent-history command line                                                | Refuses any folder that has a PathKeep `config.json`                                                                                                                                   |

A restore over a damaged or missing control is allowed, because that is what a restore repairs. A restore over a control that needs a newer build is not: that archive is intact, and replacing it would put an older one on top.

A copy of an archive (a snapshot, a staging folder for a mode change, an export) carries the stamp of the archive it was copied from.

### What a refused start does

- Started in the background or by the scheduler: the process exits with status 1 and the reason on stderr.
- Started by a person: no window is created. A native dialog gives the reason in English, Simplified or Traditional Chinese (picked from the OS languages, since the config is the thing that could not be used), with the folder and the code, and the app quits when it is dismissed.
- A refused start does not register as "the running PathKeep" for the data folder. Otherwise a newer build started while the dialog is still open would hand over to the refused one and show nothing.
- **Gap, to close before v0.6.0 ships:** the dialog is all there is. For an archive that needs a newer build the screen that replaces it should offer the update. For a control that is damaged or missing it must lead to restoring a safety snapshot: the backend still lists snapshots and restores over a damaged control, but with no window nothing can ask it to. Until then that user is safe and stuck. Tracked in `WORK-ARCHIVE-REFUSAL-SCREEN`.
- Guaranteed untouched: every file and folder under the data root, including the lock files and logs; the keychain; the schedule; App Lock state.

One exception is outside the app's control: on macOS a schedule installed by an earlier run redirects the worker's output into `schedule/logs/`, and launchd opens those files before the worker starts.

## What is proven, and how

- `vault-worker/tests/root_admission/operations.rs` drives every entry point in the table against an archive stamped for a newer build, through the real owner and the real worker binary where the entry point is one. Each case hashes the whole data folder, the test keychain and the test scheduler before and after, and counts keychain reads. The list of entry points is written out by hand; `registration.rs` fails when a command is registered without a row.
- `vault-core/tests/root_admission_states.rs` is the table in "A folder without `config.json`" as literal rows crossing control, record and contents. Its expectations are not computed from the layout table.
- `src-tauri/tests/root_admission_startup.rs` starts the real desktop binary: a background start and a cold worker start on a newer archive leave the folder unchanged, an interrupted-wipe marker in a newer archive deletes nothing, and whatever a first start creates still reads as a first run.
- `vault-core/tests/root_stamp_codec.rs` holds the byte-for-byte rule for the stamp across a setting change and a mode change, the version rule against every ordering of level and floors, and that a bare `archiveMode` is written back as v0.5.0 expects.

Not proven yet:

- An archive with a higher revision that this build may write, opened and saved through the real owner. At level 0 no stamp is admitted, so today only the codec holds the byte-for-byte rule. The owner-level test comes with level 1.
- The writer-too-old refusal through the real owner. At level 0 the reader check always comes first; the pure rule covers it.
- The foreground dialog. The tests cover the background start; nothing drives the dialog.
- A v0.5.0-written archive. The fixtures are hand-written controls over archives this build created.

## The shape of what is persisted

A format change that nobody noticed is the usual way two versions come to disagree: a field is added to a record, the build that added it writes it, and an older build drops it on its next save or reads a default where a value was. So the shape of every JSON control record is generated from the types that are actually serialized (`vault-core/src/persisted_schema.rs`, with `schemars`) and compared to a snapshot in the repository:

- `src-tauri/crates/vault-core/tests/persisted_schema/revision-N.json` is the snapshot for revision N. Any change to a listed type fails `tests/persisted_schema.rs` until the snapshot is rewritten, so the change shows up as an edit to that file in the same commit and in review.
- `formats.json` beside it has one entry per revision: what changed and what older builds do with it. Its `released` list ties a revision to the hash of its snapshot. Once a revision is listed there its snapshot cannot change at all; a shape change then needs a new revision. The check needs no git history.
- The snapshot is for the revision the next release writes. That is one ahead of the build's level until the build creates everything the revision requires.

Records in the snapshot today: `config.json`, the install journal, the "delete all data" marker, the two App Lock records, a snapshot's file list, a move bundle's manifest, a scheduled run's attempt record.

Not in it yet: JSON stored in database columns, the search index's and vector index's own manifests, the portable-artifacts origin record, the old-format conversion state, the import-roots record, crash reports, and the standalone agent-history marker. Binary formats (the key envelope, the encrypted bundle entries) have their own golden bytes. A snapshot cannot see a change of meaning that keeps the shape, such as a default or a unit; that still takes a person.

## Changing the format later

Not built yet; recorded so the next format change does not rediscover it:

- Publish the higher stamp first, then write the first byte of the new format. Never lower it again.
- A new required database is recorded in the install journal ("being created, did not exist") before it is created, so a crash in between can be finished.
- When a release is cut, its revision and the hash of that revision's snapshot go into `formats.json` under `released`.
