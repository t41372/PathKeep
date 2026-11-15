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

**This build is level 1 and writes revision 1.** Every archive it creates is stamped from the start, an archive of PathKeep 0.4 or 0.5 is brought up to revision 1 the first time this build opens it (below), and every archive a restore, an import or a conversion installs gets a stamp. From then on 0.4 and 0.5 stop at its config. The level went from 0 to 1 in the same change that made the build create everything revision 1 requires, the two agent-history stores: a build that claimed level 1 before that would have admitted archives it could not serve.

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

| Code                            | Meaning                                                                                            |
| ------------------------------- | -------------------------------------------------------------------------------------------------- |
| `archive-reader-too-old`        | A newer PathKeep can read this archive; this one cannot.                                           |
| `archive-writer-too-old`        | A newer PathKeep can change this archive safely; this one cannot.                                  |
| `archive-config-format-too-new` | `archiveMode` has a `formatVersion` above 2.                                                       |
| `archive-control-missing`       | There is an archive here and no `config.json`.                                                     |
| `archive-control-unreadable`    | `config.json` exists and cannot be read.                                                           |
| `archive-control-invalid`       | `config.json` is not something any PathKeep wrote.                                                 |
| `archive-compatibility-invalid` | The stamp's numbers are not positive integers in order.                                            |
| `archive-store-missing`         | The archive is stamped, a store its format requires is gone, and nothing says it is being created. |
| `archive-uninitialized`         | Not a refusal: a worker command was asked to use an archive that was never set up.                 |

The first three mean the archive is intact and needs a newer build. The next five mean its control is damaged or gone, or a store is. These are different messages to the user and are never reported as a wrong password, a corrupt database or an empty archive.

## Bringing a released archive up to revision 1

An archive PathKeep 0.4 or 0.5 wrote has no stamp and five stores. Revision 1 adds two: the agent-history evidence (`archive/agent-history.db`) and its read model (`derived/agent-history-read-model.db`); their search index is a folder made when it is first needed. The first time this build opens such an archive it brings it up (`vault-core/src/maintenance/creation.rs`), in this order:

1. The stores the layout requires that are not there are made in a new folder under `staging/` (`creating-*`), with the archive's keys, closed with everything in their main files, and flushed. Nothing in the archive has changed yet.
2. A record names those stores and that folder. It is written where the install journal goes and under the journal's name (`archive/.pk-install-v2.json`, identity `pathkeep-create`), which 0.5 does not know.
3. `config.json` gets the stamp. From here 0.4 and 0.5 stop at the config.
4. Each store is moved into its place by one rename, only where nothing is.
5. The record and the folder are removed.

A crash at any of these points is finished by the next open, and nothing already in the archive is deleted or written over on the way:

- **Before the stamp, the record is only discarded, with its folder.** Nothing stopped 0.5 from opening the folder since, and 0.5 creates a store it lacks and saves into it; whatever it made stays. The next open makes what is missing then.
- **After the stamp, the stores the folder holds are moved in, only where nothing is**, and the open makes again any the folder lost. A store already in its place stays as it is.
- **A crash while a store is made leaves a half-written file under `staging/` only**, which goes with the record. The archive receives a store only whole.
- **A store's `-wal` or `-shm` in place without its main file stops the open** (`archive-store-missing`). A new main file beside it would be read with another file's log.
- **A stamped archive missing a store that no record names is refused** (`archive-store-missing`) and left as it is. The store layer would otherwise create the store empty, and the archive would open as if its data had never been there.
- **A record that names anything other than a store this build knows, or any folder but one of its own under `staging/`, is refused** (`archive-control-invalid`), and it touches nothing.
- **A record carried into another archive's folder changes nothing there.** That archive's stores are in their places, and nothing is moved over a file.
- **Only "not found" is missing.** A store PathKeep cannot look at (a folder without permission) stops the bring-up before the record. An earlier version took such an error for a missing store, made the store again and stamped the archive, and the Ask conversations and Insights then opened empty.

Once the archive is up, an open writes nothing: no record, the same control bytes.

### What an installed archive is stamped with

A restore, an import or a conversion replaces the archive with another one. The archive it installs keeps the stamp of the one it replaces, or takes the incoming archive's stamp when that is at least as high on all three numbers; an older snapshot or file never lowers it (`admission::compose`). When neither stamp is at least as high as the other on all three numbers, the operation is refused (`archive-compatibility-conflict`) before anything is installed. A folder without a stamp counts as this build's own, revision 1.

The installed archive's layout follows from that stamp.

**A restore brings back browsing data, under the archive's own encryption and password** (the user's decisions, 2026-11-15):

- **The agent conversations stay where they are**, never rolled back, converted or moved: they are only ever added to, and the tools delete their own transcripts (Claude Code after 30 days by default), so a rollback could only lose them and put back what was archived or thrown away since. The conversations in a copy are for a separate repair of damaged agent-history stores (BACKLOG `WORK-AH-REPAIR-FROM-COPY`), never for a restore. An archive that has none (one of 0.5's layout) gets empty stores.
- **The archive keeps its encryption mode, key and password.** Restoring a copy from before encryption was turned on must not turn it off and write everything, conversations included, unencrypted; nor may a restore put back a password changed since. The copy's stores are written under the archive's key: as they are when the copy is under the same key (a password change rewraps the same key, so keys are compared, not key files), rewritten table by table as in a mode change when it is not (a mode change since). An encrypted copy restored into an archive whose encryption was turned off is written unencrypted, as the user chose. The archive's key file stays; the copy's never replaces it.
- **This computer's settings stay**: App Lock and whether the keychain remembers the key, as an import keeps them (`keep_machine_settings`). Otherwise a restore could turn App Lock off, or store the key in a keychain the user stopped using. The copy's other settings come back with it.

The archive is encrypted when it has a key file (one written on its first open counts) or its key is given; not by its control, which can be what is damaged, nor by its stores' headers, which a damaged store garbles. A folder with no archive at all takes the copy's encryption and key file. When no archive is open and it is encrypted, the restore needs the archive's key, from its current password; without it the restore stops before anything changes (`archive-current-password-required`). An encrypted copy is opened with the password it was made with (`archive-password-required`); that password opens only the copy.

A move file from 0.5, which has five stores, is imported with the two agent-history stores empty. An import replaces the whole archive, conversations included; whether it should keep them instead is not decided yet.

**What a restore or an import replaces becomes a safety copy** (the user's decision, 2026-11-15): `raw-snapshots/maintenance/before-restore-*` or `before-import-*`, listed for a restore as "Before a restore" or "Before an import", counted in disk usage and removed by freeing space like the other copies, never deleted on its own. Restoring it undoes the restore or the import. Before, it was a `maintenance.bak-*` folder in the archive's root that no screen showed and only "delete all data" removed, so every restore or import silently doubled the archive's size.

The install journal names the copy (`kept_as`), and only where this folder's installs put it: beside the other copies, under the random part of the journal's own backup folder name. The files are moved there and sealed after the new archive is published, so a crash after the publish still ends with the copy at the next start. The copy drops the search index, which a restore builds again, and a restore's copy has no agent conversations, which the restore left in place; snapshot verification does not require them (a restore never takes a copy's). A replaced folder that held nothing (an import into an empty folder) leaves no copy. One whose control cannot be read, so it cannot be sealed or restored, is kept all the same, counted and freeable, and not offered for a restore. A conversion from the old format still leaves its backup folder: it holds only the control the conversion replaced.

A move file this build writes is format 3: format 2 plus the two agent-history stores. 0.4 and 0.5 read only format 2 and refuse a format-3 file ("unsupported bundle format 3") before asking for its password. This build reads both.

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

- **Its records have names those releases do not know** (`archive/.pk-install-v2.json`, `.pk-wipe-v2.json`) and carry an identity and a version that are checked before anything is replayed. A record this build did not write in that shape deletes and replaces nothing. A 0.5 that starts on the folder halfway through an operation of this build does not find a record to act on, and since this build stamps every archive it opens, 0.5 stops at the config of any archive this build has used.
- **An install never takes the control away.** The control is copied into the backup, not moved, and replaced by one rename on install; a rollback writes it back from that copy. An install that would leave no control is refused before it starts.
- **A journal acts only on the files it moved.** Where a file is says what the install did with it. The backup folder is made, under a new random name, when the install begins, and only the install moves anything into it: an entry the backup holds is an original the install moved out of its place, and one it does not hold was never moved or has been put back. That decides every step of a rollback. Before a rollback takes a file out of its place (the staged file, so that the original can come back, or a file that was not there before), it checks every byte against what the journal recorded of the staged file (`vault-core/src/content_identity.rs`: SHA-256 over the file in 64 MiB chunks hashed in parallel; for a folder, over what its files hold, not its own times or the `.DS_Store` a file browser drops into it). Anything else in a place stops the rollback before anything moves, with the journal kept. The control binds the journal to its folder: the journal records the control's bytes before and after the install, and a rollback starts only where the control is one of the two. Every install moves `config.json` and every archive has its own, so a journal carried into another archive's folder, even with the backup folder it names (two folders merged, or the journal's files restored over another archive), meets that archive's control and changes nothing. A faithful copy of the whole folder, made by any tool, with or without the files' times, recovers like the original.
- **Every step of a rollback is one rename or one atomic write.** A file taken out of its place is renamed into the stage, which goes with the journal at the end, so a rollback cut short never leaves half of a folder, and the next start finishes it.
- **A publish that reports an error is not rolled back in memory.** It may still have put the published record on disk (the flush after its rename failed). The install stops where it is, and the next recovery goes the way the record on disk says. Rolled back in memory, the files would go back while the record says published, and a later recovery would finish the install over them: databases of one generation beside a control and a key file of another.
- **No file numbers, and no length and time.** An earlier draft recognised the files by inode number. Measured on macOS disk images, exFAT and FAT32 give an empty file a new number at every move and every mount, and Linux numbers vfat and exfat files afresh at every mount, so after a crash and a reboot such a drive could never have been rolled back. The next draft used length and modification time, which survive a rename and a remount everywhere but tell nothing apart that is as long and was written within the file system's time resolution, and change in a copy made without times: a rollback then took another file for its own, or refused its own. What a file holds is the same on every file system and after every faithful copy.
- A file that was there before the install and is now neither in place nor in the backup stops the rollback with the journal kept; it is not reported as rolled back. The journal records its stage and backup relative to the folder. A 0.5 journal records no identities: it is rolled back only when the backup folder it names, randomly named when the install began, is in this folder, and its absolute paths are taken relative to where its backup was. Beside a stamped control it is set aside unread (`archive/.pk-import-journal.legacy-stale.json`): 0.5 cannot open a stamped archive, and this build replays a 0.5 journal before it stamps one, so it was carried in.
- **Moves are made durable destination first.** New folders' entries are flushed before anything is moved into them, and a move flushes the destination's directory before the source's, so a power cut can leave an entry in both places, never in neither.
- **"Delete all data" keeps the control until the very end, and deletes nothing until everything is moved aside.** Order: the intent an earlier wipe left, settled; a trash folder in the data folder (`wipe-trash-*`), then the intent; `config.json` replaced by an empty, not-initialized one that keeps the stamp (every background worker stops when it sees that); the workers stopped; every target moved into the trash, each by one rename, including a leftover `.pk-durable-*` temporary file in the folder (a crash during an atomic write can leave a full copy of the old settings there); a scan for anything outside the trash that still means an archive, by the same rule as a folder without a control; the commit, which rewrites the intent to say everything is in the trash; the trash deleted; the keychain; the automatic backup; the intent; `config.json`. Before the commit a failure puts everything back in the same call, and a crash leaves the same to the next start: nothing is deleted. After the commit the next start finishes the wipe. A crash after the intent is removed leaves an empty, not-initialized control in an empty folder. Where there was no control, the empty one is written before the intent, so the intent never lies alone.
- **The intent acts only in the folder it was written in.** It records the control the user confirmed deleting and the emptied control the wipe writes, both byte for byte, and the trash it made. A wipe cut short is settled only while the folder holds one of the two controls; anywhere else (a folder restored from a backup, imported into, or an intent carried in) the intent is discarded and nothing else is touched, its trash included. Undoing puts an entry back only where nothing is now, then the confirmed control over the emptied one; what stands in an entry's place stays, and so does the entry, in the trash, which the next "Delete all data" lists and removes. Finishing deletes the wipe's own trash and, only while the control is the emptied one, whatever has appeared at a target since. A copy of the folder made during a wipe is the same folder to the intent: undone before the commit, which gives the copy its archive back, and finished after it, which deletes the copy's copy of what the user confirmed deleting. An intent that names as its trash anything but a `wipe-trash-*` folder directly in the data folder is refused (`archive-control-invalid`). The earlier drafts told a copy from the original by the inode numbers of the archive's databases, which exFAT, FAT and network shares do not keep; nothing depends on them now, since nothing is deleted on the record alone. Import, restore and conversion settle a pending wipe before they install anything, as initialization does, and refuse while one is pending. A second "Delete all data" settles it too, before its own intent could replace one that still names keys to clear.
- **A 0.5 marker is finished only where there is no control.** It records nothing of the kind. PathKeep 0.5 removed the control right after writing its marker, before deleting anything, so beside a control, stamped or not, its wipe had deleted nothing: the marker is discarded, and the user still sees the archive and can delete it again. Without a control it is finished as that release would have: deleting in place.
- **Nothing writes a control into a folder the wipe is emptying.** The wipe holds the control's gate (below) from reading what to delete to its last removal. A settings save waiting for it then finds no archive and is refused (`archive-reset`) when the settings it was given were read from one; a provider key or an automatic backup waiting for it is refused too, so none is left behind.

`maintenance/install_tests.rs` checks the control after every step of an install and of its recovery; carries a journal cut at each step into another archive's folder together with its backup folder and finds every file there unchanged, for an install that replaced an archive and for one into an empty folder; rolls back a copy of the folder made without the files' times, and an install whose staged file has the original's length and time; recovers a publish that reported an error the way the record on disk says; changes nothing beside no control, and sets aside a 0.5 journal beside a stamp; cuts a rollback at each move and finishes it at the next start; and knows a folder by what its files hold, `.DS_Store` aside. `archive/wipe/tests.rs` cuts the wipe at each durable step, then undoes it before the commit, every file as it was byte for byte, and finishes it after; undoes a wipe that fails in the same call, and one cut short while it was being undone; undoes a wipe in a copy of the folder; leaves another archive's folder as it is, trash included; keeps what stands where an entry goes back; refuses an intent that names anything but its own trash; settles an earlier intent before a second wipe; wipes a folder that had no control without leaving its intent alone; and discards a 0.5 marker beside a stamped or unstamped control. `creation/tests.rs` cuts the bring-up at each step and finishes it, makes again a store whose folder was lost, refuses a store's log without the store, stops at a store it cannot look at, and changes nothing in another archive's folder that a record was carried into. Each rule was also broken on purpose and its test watched to fail.

## Who writes `config.json`

The control holds settings and the archive's own state (whether it is set up, its mode, its stamp). Several things write it, some of them operations that take minutes. They follow one rule: every write is a read-modify-write under the control's gate (`vault-core/src/config/control_gate.rs`), and each writer changes only the fields it owns, on the control as it is when it holds the gate. `save_config` takes the gate as an argument, so a write without it does not compile.

| Writer                                       | Owns                                                                                                                                             |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Settings save                                | The settings it was given, as changes against the settings the window showed. Never whether there is an archive, never the mode                  |
| Menu bar icon, App Lock passcode             | Their own fields                                                                                                                                 |
| Setup                                        | Every setting it was given, and `initialized`                                                                                                    |
| Bring-up to this build's layout              | The stamp                                                                                                                                        |
| At-rest repair (at launch, and the command)  | The mode, from the files' headers                                                                                                                |
| Mode change                                  | The mode, on the control as it is at install time; then whether the key is remembered                                                            |
| Restore                                      | Everything but this machine's settings (App Lock, whether the key is remembered) and the encryption mode, which stay as they are at install time |
| Import                                       | Everything but this machine's settings (App Lock, whether the key is remembered), which it takes from the control as it is at install time       |
| Conversion from the old format               | The settings it was given (setup's, or the upgrade screen's)                                                                                     |
| Install recovery, old-format launch recovery | What the interrupted operation recorded                                                                                                          |
| "Delete all data"                            | Everything, from start to end                                                                                                                    |

The gate is held for the file only: never across database work or a wait on another lock, and taken after the archive's operation gate by those that hold both. That is why a settings save never waits for an export, a snapshot, a retention prune or the copy of a mode change, restore or import, which hold the operation gate for minutes: those take the control's gate only for their own write at the end. An operation that rewrites the archive builds the replacement's control at that moment from the live one, so a setting saved while it ran is kept. A conversion is the exception: it installs the settings it was given, because setup's choices are what it writes, and nothing else changes settings while the upgrade screen is up.

An operation that closes the archive to put another generation in its place (a mode change, a restore, an import, a conversion) holds the session's maintenance lease from before it closes the archive until the replacement is published (`VaultSession::begin_maintenance`). An open in the same process meanwhile waits and gets the replacement, or what is on disk when the operation ends without one; a close waits and closes the replacement. Without the lease, an open between the close and the publish took the files being moved, or replayed the operation's install record while it was being written: the owner lock does not stop it, because the operation and the open are in the same process.

The settings page saves each change against the settings it showed (`baseConfig`), so a save carries only its own changes. A save whose settings came from a set-up archive, into a folder that has none any more, writes nothing (`archive-reset`); a save never sets an archive up (`archive-uninitialized`).

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
- `vault-core/src/maintenance/creation/tests.rs` cuts the bring-up at each of its three durable points and opens again; has a 0.5 store open the folder after a crash before the stamp and save into a store the record names; refuses a stamped archive missing a store, and a record naming a file that is not a store, comparing every file before and after; restores a snapshot and imports a move file that 0.5 would have written (five stores, no stamp, format 2); and keeps a later build's revision-2 stamp byte for byte through an open, a settings save, a mode change and a restore. Each property was also checked by breaking the code and watching its test fail.
- `vault-core/src/maintenance/safety_copy_tests.rs` restores a copy and an import over an archive, finds what each replaced listed under its operation, sealed, without the search index, and restores it to get back what was replaced; cuts a restore right after its publish and finds the copy made at the next start; imports into an empty folder and finds nothing kept; restores the copy of an encrypted archive with the password it had. `install_tests` refuses a journal that names any other place for the copy, moving nothing.
- `vault-core/src/maintenance/restore_tests.rs` restores a copy and checks that browsing comes back while the agent conversations stay the same files, and that the archive keeps its encryption and password: after a password change since the copy only the current password opens it; a copy from before encryption was turned on comes back encrypted under the current key; an encrypted copy restored into an archive whose encryption was turned off comes back unencrypted, without the copy's key file; a copy from two mode changes ago is rewritten under the current key; with no archive open, an encrypted archive is refused with every file unchanged until its key is given; App Lock and the keychain choice stay this computer's. `vault-convert-sqlcipher/tests/snapshot.rs` restores an old single-file snapshot into an unencrypted and an encrypted archive, each keeping its own encryption.
- `tests/e2e/restore.spec.ts` restores, in the app, the copy the move replaced (under another key): afterwards only the current password opens the archive, the key the keychain remembers still opens it on its own, and the restore is listed as "Before a restore".
- `vault-worker/tests/root_admission/operations.rs` refuses both a reader floor and a writer floor above this build's level at every entry point, with their own reasons.
- `vault-worker/tests/vault_lifecycle/` follows a marker in each of the seven stores through first open, backup, mode change, password change, safety snapshot and restore, move export and import into an empty and a populated folder, reopen, and "delete all data", and pins the file list of each.

Not proven yet:

- The foreground dialog. The tests cover the background start; nothing drives the dialog.
- A v0.5.0-written archive. The fixtures are hand-written controls over archives this build created.

## The shape of what is persisted

A format change that nobody noticed is the usual way two versions come to disagree: a field is added to a record, the build that added it writes it, and an older build drops it on its next save or reads a default where a value was. So the shape of every JSON control record is generated from the types that are actually serialized (`vault-core/src/persisted_schema.rs`, with `schemars`) and compared to a snapshot in the repository:

- `src-tauri/crates/vault-core/tests/persisted_schema/revision-N.json` is the snapshot for revision N. Any change to a listed type fails `tests/persisted_schema.rs` until the snapshot is rewritten, so the change shows up as an edit to that file in the same commit and in review.
- `formats.json` beside it has one entry per revision: what changed and what older builds do with it. Its `released` list ties a revision to the hash of its snapshot. Once a revision is listed there its snapshot cannot change at all; a shape change then needs a new revision. The check needs no git history.
- The snapshot is for the revision this build writes. Until a release writes it, it may be edited in place.

Records in the snapshot today: `config.json`, the install journal, the "delete all data" marker, the two App Lock records, a snapshot's file list, a move bundle's manifest, a scheduled run's attempt record.

Not in it yet: JSON stored in database columns, the search index's and vector index's own manifests, the portable-artifacts origin record, the old-format conversion state, the import-roots record, crash reports, and the standalone agent-history marker. Binary formats (the key envelope, the encrypted bundle entries) have their own golden bytes. A snapshot cannot see a change of meaning that keeps the shape, such as a default or a unit; that still takes a person.

## Changing the format later

Revision 1 is the first stamp, so the bring-up starts only from an archive without one. The next format change keeps the same order and extends it:

- Publish the higher stamp first, then write the first byte of the new format. Never lower it again.
- Bring an archive up from a lower stamp as well as from none: record the stores the new layout requires, stamp, create, remove the record (`maintenance/creation.rs`).
- A new move-file format number for a layout with new stores (`FORMATS` in `migration/native.rs`).
- When a release is cut, its revision and the hash of that revision's snapshot go into `formats.json` under `released`.
