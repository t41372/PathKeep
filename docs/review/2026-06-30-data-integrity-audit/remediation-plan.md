# Data-integrity remediation roadmap (Track 1–4)

Source: `findings.md` (Opus cluster, 48 scenarios / 27 confirmed high-critical / 5 CRITICAL). Triggered by the
encrypted-files + `Plaintext`-config + zeroed-salt incident that reached a user through a 100%-green gate.

**North star:** the canonical archive must survive a kill / power-loss / concurrent-backup at _any_ instant.
Backup is the core responsibility; AI/search/progress are secondary and must never jeopardize the archive.
Even when corruption does occur, the user gets a calm in-app restore — never a dead error page.

**No new dependency:** `libc` (direct dep) gives `F_FULLFSYNC` + `flock`; `tempfile` is present; a backend
snapshot-restore (`preview_snapshot_restore` / `run_snapshot_restore`) already exists. Zero `fsync` exists in
the archive write paths today (the disease).

## Execution model

Sequenced, individually shippable, gate-green, independently-reviewed work blocks. **Every hardening fix ships
with a kill-at-checkpoint regression test that FAILS on current code.** Foundation primitives first (everything
depends on them), built by hand (linchpin); application + tests orchestrated with the Opus cluster where parallel.

## Phase A — Foundation primitives (prerequisite for all)

1. **`atomic_durable_write` / `install_file_durably`** (`vault-core/src/durable_io.rs`): write→`F_FULLFSYNC`(temp)
   →rename→fsync(parent dir). For `config.json` and every file install. `save_config` switches to this.
2. **`ArchiveWriteLock`** (cross-process advisory lock, `libc::flock`/`O_EXLOCK` on a lockfile under the archive
   dir): every destructive op (rekey, backup, import, retention-prune, reconcile, restore) acquires it
   EXCLUSIVELY; the **separate scheduled-backup process** acquires it too (defers if held). The write-serialization contract.
3. **Fault-injection seam** (`vault-core/src/fault_inject.rs`): named checkpoints (`rekey.after_swap`, …) the
   production code calls; in the MEASURED build (NOT cfg-compiled-out) a test can arm "fail/panic at checkpoint X".
   This is the test infrastructure that makes crash-window tests possible.

## Phase B — Apply primitives to every destructive write (Track 1)

Reorder + harden, each with a kill-at-checkpoint test reproducing the relevant confirmed finding:

- **rekey** (`maintenance.rs`): verified snapshot → export temp → `F_FULLFSYNC` → **verify opens w/ key +
  quick_check** → acquire lock → checkpoint(TRUNCATE)+assert-no-wal → atomic swap → scrub sidecars
  (`db+wal+shm+journal`) → **write config LAST (atomic+durable)** → only THEN delete backstop. Persisted
  rekey-journal `{phase, old/new key ref}` for resume/rollback. Migrate source-evidence inside the same
  recoverable step (CRIT 2: key-rotation).
- **`apply_import`** (`migration.rs`): write config LAST; crash-recovery on next launch (mirror
  `recover_interrupted_rewrite`); scrub the target's pre-existing `-wal`/`-shm` before installing (CRIT 3).
- **backup** (`backup.rs`): atomic + lock-held writes; no second backup concurrent.

## Phase C — Reconcile + crash-recovery on launch (Track 1)

- **Full at-rest reconcile for BOTH DBs** (extend `at_rest.rs`): detect ANY config↔file mode/key drift
  (try-open, salt check, rekey-journal) → self-heal from the kept backstop/snapshot instead of dead-ending at
  `NOTADB`. Recover interrupted swaps for rekey + import.

## Phase D — Snapshot + Recovery GUI (Track 3)

- Verified snapshot before EVERY destructive op (today only rekey, unverified); verified-openable; retention
  never deletes the last-good.
- **Corruption-detected-on-launch → calm in-app "Restore from snapshot"** (dated/sized/verified list, preview,
  one-click, broken state auto-quarantined). Build on existing `run_snapshot_restore`; FE by Sonnet 4.6.

## Phase E — Test infrastructure sweep + methodology (Track 4)

- Real-encrypted-file round-trips (encrypt→kill→reopen); **config↔on-disk invariant checker** run in the suite;
  tests use `load_config` not hand-built config; concurrency/torture + property tests over interleavings.
- Update `quality-matrix.md`: durability (fsync/atomic), config↔disk consistency, crash-window, and
  "no production I/O cfg-compiled-out" rules. The direct answer to "why didn't we test this".

## Confirmed CRITICALs this must close

1. Scheduled-backup process races GUI rekey, no cross-process lock (Phase A.2 + B).
2. Interrupted key-rotation leaves source-evidence on the old key; reconcile only fixes MODE (Phase B + C).
3. Bundle import replays the previous archive's hot `-wal` into the new DB (Phase B).
4. Rekey swap leaves a concurrent open's `-wal` to replay into the rekeyed DB (Phase B).
5. No lock between rekey and in-flight backup → committed rows land in the renamed-away inode (Phase A.2 + B).

## Phase-A done — foundation primitives (committed) + Phase-B carry-ins from reviews

Phase A landed three review-clean primitives (`durable_io`, `archive::ArchiveWriteLock`, `fault_inject`). The
per-block + integration reviews surfaced items that are NOT foundation defects but MUST be honored when Phase B
wires the primitives into rekey / import / backup:

- **`sweep_stale_temps` MUST run while holding `ArchiveWriteLock`** (MEDIUM): it deletes every `.pk-durable-*`
  with no age/owner guard, so a sweep racing an out-of-process backup's in-flight durable temp would fail that
  backup's `persist()`. Hold the lock first; document the contract on the fn at the call site.
- **Sweep BOTH `root/` and `root/archive/`**: `config.json` durable temps land in `root/`, the DB/lock temps in
  `root/archive/`. Sweeping only the archive dir leaks a SIGKILL'd config temp.
- **Fault-seam "must-fire" guard before the first crash-window test**: add an opt-in guard that PANICS on drop if
  its checkpoint was never hit, so an injection that silently misses (e.g. the destructive section ran on a
  different thread than the arming thread) FAILS loudly instead of false-passing. Every crash-window test must
  assert the _injected_ error propagated via `format!("{err:#}")` / `err.chain()` — never bare `is_err()` /
  recoverability-only. Crash-window tests are in-crate `#[cfg(test)]` unit tests (the arming API is
  `cfg(test) pub(crate)`), not `vault-core/tests/`. Confirm each op's destructive section runs on the arming thread.
- **Call the seam qualified** (`fault_inject::checkpoint("rekey.after_swap")`) — `checkpoint` is overloaded in the
  crate (AiRunControl / intelligence / SQLite). Optionally tighten `fault_inject::checkpoint` to `pub(crate)`.
- **`install_file_durably` requires `built` in the SAME directory as `dest`** (only `dest`'s parent is fsynced) —
  the rekey export temp must be built in `root/archive/`, not a system tempdir, or the unlink half isn't durable.

## Lock-completion block (carry-ins from rekey + backup reviews)

`ArchiveWriteLock` is wired into rekey (`69bb3c28`) and backup (`fcd334eb`); the import block adds it too. A
dedicated block must finish the lock contract:

- **In-process top-level serialization (MEDIUM, backup review):** the lock is process-REENTRANT, so it serializes
  only ACROSS processes. Two ops dispatched in the SAME GUI process (e.g. a manual backup mid-transaction while the
  user toggles encryption/rekey from Settings) each get a reentrant guard sharing one fd → NOT mutually excluded →
  the same rename-out-from-under-open-txn loss (CRIT-5's same-process trigger). Add a process-wide archive-op gate
  that the TOP-LEVEL ops (backup, rekey, import) take exclusively, while NESTED helpers (reconcile/migrate called
  within a top-level op) do NOT take it (so backup→reconcile can't self-deadlock). Likely a `_locked` variant for
  reconcile: the public entry takes the gate, the nested entry assumes the caller holds it.
- **`reconcile_archive_encryption` must hold the lock (MEDIUM, backup review):** the unlock-path
  `reconcile_archive_encryption` (vault-worker app.rs ~230) is a destructive at-rest rewrite that does NOT take the
  lock, against `write_lock.rs`'s own contract — a scheduled backup can race it on source-evidence. Wrap it.
- **Wire the lock into retention-prune + snapshot-restore** too (the module doc lists them as MUST-hold ops). When
  the lock is added to each, also add the `crate::migration::recover_interrupted_import(paths)?` recover-first call
  under that lock (mirroring rekey/backup/unlock) — these are the only destructive pre-open paths still missing it.
  Do NOT add a recover-WITHOUT-lock half-measure in the meantime: recovery rewrites canonical DB files and must
  serialize against a concurrent scheduled backup, so the recover-first wiring lands WITH the lock, not before it.
- **`apply_import` in-process quiescence + crash-recovery wiring (import block carry-in):** `apply_import` holds the
  cross-process `ArchiveWriteLock`, which excludes only the OUT-OF-PROCESS scheduled backup — there is NO in-process
  worker quiescence in the `apply_app_data_import` → `worker_bridge::apply_app_data_import_impl` →
  `vault_worker::apply_import` → `vault_core::apply_import` chain (the old doc claim that "the Tauri command façade
  stopped background workers" was inaccurate; corrected). `recover_interrupted_import` (the crash-recovery twin of
  `apply_import`'s in-band rollback over the durable `.pk-import-journal.json`) is now wired at THREE destructive
  pre-open sites: the unlock-path `reconcile_archive_encryption`, the backup pre-open path
  (`run_backup_with_progress`) — closing the out-of-process scheduled-backup hole where a crashed import's same-mode
  half-state would be backed up and recorded as a SUCCESSFUL backup of corrupt state — AND the rekey pre-open path
  (`rekey_archive`), closing the hole where a rekey on a PLAINTEXT archive (never reached by the encryption-gated
  launch reconcile) would rekey a half-applied import into a permanent config↔source-evidence mode-drift brick.
  `recover_interrupted_import` is itself now fail-closed: after restoring, `ensure_recovered_modes_are_consistent`
  refuses (leaving the marker + surfacing a recoverable error) if the two canonical DBs cannot reach one at-rest mode
  the about-to-be-written config can serve, so even the not-yet-wired paths can never silently commit a drift brick.
  Still TODO for this block: wire recover-first into the retention-prune + snapshot-restore pre-open paths (with the
  lock, per the bullet above; user-initiated, in-process ops that do not silently auto-record success — lower
  residual risk, and the fail-closed guard backstops them until then).
- **LOW polish:** manual backup uses blocking `acquire` with no "waiting for another archive operation" UI state —
  switch to `acquire_interruptible` with a busy/cancel state; correct the findings note that the source-evidence
  after-commit orphan is "permanently orphaned for that batch" (watermark advanced), not "rebuildable".
