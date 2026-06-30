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

## Lock-completion block (carry-ins from rekey + backup reviews) — DONE

`ArchiveWriteLock` is wired into rekey (`69bb3c28`), backup (`fcd334eb`), and import. The lock-completion block
finished the contract; the remaining carry-in is the LOW UI polish at the bottom.

- **[DONE] In-process top-level serialization (MEDIUM, backup review):** the cross-process lock is process-
  REENTRANT, so it serializes only ACROSS processes. Two ops dispatched in the SAME GUI process (e.g. a manual
  backup mid-transaction while the user toggles encryption/rekey from Settings) each got a reentrant guard sharing
  one fd → NOT mutually excluded → the same rename-out-from-under-open-txn loss (CRIT-5's same-process trigger).
  CLOSED by `ArchiveOpGate` (`archive/write_lock.rs`): a process-global, NON-reentrant, keyed-by-archive in-process
  gate the TOP-LEVEL ops (backup, rekey, import, retention-prune, snapshot-restore, reconcile) take FIRST, then the
  reentrant flock; NESTED helpers (reconcile_source_evidence / migrate / `recover_interrupted_import`) take ONLY the
  flock, never the gate (so `backup → reconcile` can't self-deadlock). Enforced type-wise via public-vs-`_locked`
  variants, NOT a thread-local flag.
- **[DONE] `reconcile_archive_encryption` must hold the lock (MEDIUM, backup review):** the unlock-path
  `reconcile_archive_encryption` now takes the gate + cross-process lock for its whole at-rest rewrite (public entry
  in `at_rest.rs`; nested `reconcile_archive_encryption_locked` assumes the caller holds them). The vault-worker
  app.rs ~230 call site reaches it through that gated public entry, so a scheduled backup can no longer race it on
  source-evidence.
- **[DONE] Wire the lock into retention-prune + snapshot-restore:** both now take the gate + `ArchiveWriteLock` and
  call `crate::migration::recover_interrupted_import(paths)?` recover-first under that lock (public entry delegates
  to `run_retention_prune_locked` / `run_snapshot_restore_locked`). For snapshot-restore, recover-first reverts a
  half-import to a single consistent at-rest mode before the restore replays its checkpoint (which overwrites the
  visible facts anyway), so the two never fight.
- **[DONE] `apply_import` in-process quiescence + crash-recovery wiring:** `apply_import` holds the cross-process
  `ArchiveWriteLock` AND now the in-process `ArchiveOpGate`, so a concurrently-dispatched TOP-LEVEL destructive op in
  THIS process serializes behind it (unrelated non-destructive workers that never rename a canonical DB stay out of
  scope by design). `recover_interrupted_import` (the crash-recovery twin of `apply_import`'s in-band rollback over
  the durable `.pk-import-journal.json`) is now wired at EVERY destructive pre-open site under the lock: the unlock-
  path `reconcile_archive_encryption`, backup (`run_backup_with_progress`), rekey (`rekey_archive`), retention-prune
  (`run_retention_prune`), and snapshot-restore (`run_snapshot_restore`). It remains fail-closed:
  `ensure_recovered_modes_are_consistent` refuses (leaving the marker + surfacing a recoverable error) if the two
  canonical DBs cannot reach one at-rest mode the about-to-be-written config can serve.
- **LOW polish (REMAINING carry-in):** manual backup still uses blocking `acquire` (gate + lock) with no "waiting for
  another archive operation" UI state — switch to `acquire_interruptible` with a busy/cancel state (needs FE work +
  `en`/`zh-CN`/`zh-TW` copy). The findings note (findings.md §594) already states the source-evidence after-commit
  orphan is "permanently orphans that batch's source-evidence" (watermark advanced), NOT rebuildable — confirmed
  correct, no change needed.
