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

## Phase C — Reconcile + crash-recovery on launch (Track 1) — DONE

- **[DONE] Launch-time at-rest reconcile for BOTH DBs + recover interrupted rekey/import on launch**
  (`at_rest::recover_archive_on_launch`, wired in `vault-worker::app::initialize_archive_database` after the
  config save and BEFORE `ensure_archive_initialized`). LAUNCH MUST NEVER FREEZE: the overwhelmingly common
  HEALTHY launch takes NO lock — a cheap unlocked pre-check (`launch_is_provably_healthy`: two marker `stat`s +
  the canonical history-vault's 16-byte header + the small `config.json`, NO gate/flock/DB-open) returns
  `Healthy` immediately, so a healthy GUI launch is never blocked behind an out-of-process scheduled backup
  holding the cross-process write lock. ONLY when a crash marker is present OR config↔file at-rest drift is
  detected does it take the in-process op-gate FIRST, then the cross-process write lock (blocking — correct and
  rare; it does NOT try-and-defer, since the config↔file reconcile has no other actor and deferring could leave
  launch to open a known half-state), and run the authoritative locked recovery that: (1) runs
  `recover_interrupted_import`, (2) runs `maintenance::recover_interrupted_rekey` (keyed on a durable
  `.pk-rekey-journal.json` written just before the rekey swap and cleared after the config commit), then (3)
  reconciles a STALE `config.json` to the canonical history-vault's REAL on-disk at-rest mode — HEADER READS
  ONLY, no key, no DB open (the 14.4M-row constraint). This heals THE incident (encrypted files under a
  `Plaintext` config, no journal → `NOTADB` dead-end) into the graceful locked unlock-prompt instead of bricking
  launch. `recover_interrupted_rekey` is KEY-FREE and fail-closed: swap-didn't-land → roll config back to the
  captured pre-rekey state + drop the orphaned export temp; canonical archive missing → bail and LEAVE the
  marker. Swap-LANDED converges config to `to_mode` in the ENCRYPT direction; the DECRYPT direction
  (`to_mode == Plaintext`) is ASYMMETRIC — it FAILS CLOSED when source-evidence is still Encrypted on disk
  (decryptable only with the now-unprompted old key), leaving the marker rather than silently committing a
  Plaintext config over an Encrypted source-evidence. Every fail-closed bail is surfaced as a structured
  `archive_recovery_required:`-prefixed, JSON-carrying error (`LaunchRecovery::Unrecoverable` /
  `ArchiveRecoveryReport`) the Phase-D recovery screen routes on. Source-evidence convergence otherwise stays
  DEFERRED to the keyed on-unlock `reconcile_archive_encryption` (it needs a key not held at launch) and
  self-heals only in the ENCRYPT direction — the documented division of labor. Every branch is covered by
  kill-at-checkpoint regression tests (`rekey.after_swap_before_config`) plus the worker headline test that
  FAILS on HEAD, and watchdog tests proving the healthy launch never blocks on a held foreign write lock.
- **[LOW carry-in, from the MEDIUM-fix review]** discriminant-vs-write asymmetry: the locked path's
  `Healed`/`Healthy` DISCRIMINANT compares the PASSED in-memory config to the header, while the write decision and
  the unlocked fast path compare the ON-DISK `config.json`. They agree only because the sole production caller
  (`initialize_archive_database`) saves config to disk immediately before calling. Safe today; if a future caller
  ever passes a config diverging from the just-saved on-disk one, the fast path could return `Healthy` while the
  locked path would `Healed`, and an Encrypted+no-key shape would then force-open the stale mode → `NOTADB`. Harden
  by keying the discriminant on the on-disk config (or asserting the save-before-call invariant). Non-blocking.

## Phase D — Snapshot + Recovery GUI (Track 3)

- Verified snapshot before EVERY destructive op (today only rekey, unverified); verified-openable; retention
  never deletes the last-good.
- **Corruption-detected-on-launch → calm in-app "Restore from snapshot"** (dated/sized/verified list, preview,
  one-click, broken state auto-quarantined). Build on existing `run_snapshot_restore`; FE by Sonnet 4.6.

### Phase D BACKEND — DONE (data + command layer)

- **D1 — full-archive one-click restore** (`run_full_archive_snapshot_restore`, maintenance.rs): validates the
  chosen snapshot BY FILE (security-guarded to `.sqlite` under `raw-snapshots/`) + keyed `quick_check` BEFORE
  touching the live archive, QUARANTINES the broken canonical files + stale crash markers (move, never delete),
  durably installs the verified snapshot, rebuilds an empty source-evidence, reconciles config LAST, verifies
  after, and records an `archive_restore` audit run. `preview_snapshot_restore` now returns
  `executeSupported: true` for the safety-snapshot kind. Deliberately does NOT recover-first (D1 supersedes an
  interrupted import/rekey). D1 is now a **crash-recoverable commit unit** guarded by a durable
  `.pk-restore-journal.json` (written before quarantine, cleared after the post-restore verify): a crash in the
  absent-canonical quarantine→install window can no longer boot an empty archive. The launch recovery sequence
  (`recover_archive_on_launch_locked`) now runs `recover_interrupted_restore` (key-free: re-install from the
  still-available snapshot, or roll back the quarantined originals; fail-closed `Unrecoverable` when neither is
  possible — never an empty archive) alongside import/rekey recovery. `quarantine_canonical_archive` also fsyncs
  the quarantine subdir (MEDIUM-1: the cross-dir move is now crash-durable).
- **D2 — `list_recovery_snapshots`** (at_rest.rs): rich keyless metadata (date/size/`verifiedOpenable`/`sourceOp`)
  per snapshot, NEVER a full-DB scan (plaintext: page-1 `PRAGMA schema_version`; encrypted: structural size). A
  new `recoverySnapshots` field rides the startup `ArchiveRecoveryReport`.
- **D3 — verified safety snapshot before whole-archive REWRITES**: the rekey-only helper is generalized to
  `create_verified_safety_snapshot(op)` and now also fires before the at-rest reconcile rewrite and (best-effort)
  before a whole-app import, each landing in `raw-snapshots/<op>/`. Retention keeps the last-good verified
  snapshot (never prunes the freshest restorable backstop).
- **BACKLOG**: retention's last-good guard trusts the keyless structural check (`size >= 512`) for ENCRYPTED
  snapshots — the authoritative keyed `quick_check` runs only at restore (D1). A keyed verify or a
  keep-N-newest policy for encrypted mode is a follow-up.
- **Backup whole-DB-snapshot DEFERRAL (deliberate)**: backup does NOT snapshot the archive. Backup is
  append-only + crash-atomic (Phase B) and NEVER REWRITES the canonical history-vault, so a per-backup
  copy + `quick_check` of the 14.4M-row DB would cost minutes for zero added safety. The backstop is kept fresh
  by the REWRITE ops (rekey/reconcile/import). `sourceOp = "periodic"` is reserved, not produced today.
- **D4 — worker commands + startup surface**: `list_recovery_snapshots`, `run_full_archive_restore`, and
  `parse_archive_recovery_required` (parses the `ARCHIVE_RECOVERY_REQUIRED_PREFIX` wire string the FE reads).

### Phase D FRONTEND — DONE (recovery GUI)

- Guided full-screen recovery screen (`src/components/archive-recovery-screen/`) — leads with the newest verified
  snapshot (one-click restore + a destructive-confirm; the full dated/sized/✓verified list is secondary),
  reassures "your current state is quarantined (moved, recoverable), not deleted", honest
  loading/empty/restoring/error states, never a dead end (failure → "Try another snapshot"). Mirrors the
  `archive-unlock-gate` calm full-screen posture; focus-trap + `aria-live` + `aria-describedby`; reduced-motion.
- Proactive Settings → "Restore from snapshot" section (`src/pages/settings/snapshot-restore-section.tsx`) reuses
  the shared restore module (`src/components/snapshot-restore/`). Restore is an explicit destructive action (not
  auto-save).
- Startup detection (`shell-data.tsx` + `shell-data-helpers.ts`): `archiveNeedsLaunchRecovery` fires only on
  `initialized && !unlocked && !encrypted && warning`, then catches the `ARCHIVE_RECOVERY_REQUIRED_PREFIX` error
  from `initialize_archive_database` and routes to the recovery screen (pre-empts routing in `app/index.tsx`); a
  malformed payload falls back to the generic error path, never crashes.
- Desktop Tauri command wiring (`src-tauri/src/commands/archive.rs`, `worker_bridge/archive.rs`, `lib.rs`,
  `dev_ipc_bridge/dispatch.rs`) mirrors `run_snapshot_restore`. TS types mirror the Rust DTOs (camelCase);
  frozen `backend.ts` fixture not extended. **Preview button removed** (deliberate): `preview_snapshot_restore`
  opens the LIVE archive (un-openable in recovery) + needs a snapshots-table row only rekey snapshots have +
  returns 0 estimates for safety snapshots — the card's date/size/✓verified/sourceOp is the honest summary.
- Gate: typecheck/lint/`check:i18n` 100% (3319 keys ×3)/`coverage:js` 100% (332 files / 3361 tests)/format +
  desktop-crate fmt/clippy all green. Independent FE review **SHIP**.

### Phase D — encrypted-user recovery gap (carry-ins from the FE review)

- **[HIGH follow-up]** a genuinely-corrupt ENCRYPTED archive has NO in-GUI recovery path: `archiveNeedsLaunchRecovery`
  returns `false` for `status.encrypted`, so damaged encrypted files route to the unlock gate, where the correct
  password still fails to unlock — indistinguishable from a wrong password — and Settings→Restore is unreachable
  behind the z-9999 overlay. Non-regressing (no recovery GUI existed pre-Phase-D; the headline PLAINTEXT-drift
  incident IS covered) but it's the one place "never a dead end" can still break for encrypted users. Fix: an
  "Can't unlock? Recover from snapshot" escape hatch on the unlock gate that routes into the recovery flow.
- **[MEDIUM follow-up]** the recovery screen can offer an ENCRYPTED snapshot it can't restore keylessly
  (`verifiedOpenable` is size-only for encrypted → a green "Verified" badge; `run_full_archive_snapshot_restore`
  bails "unlock with the archive key first"). The failure path is honest, but if ALL available snapshots are
  encrypted the non-empty panel offers no forward path (no key entry, no "Reveal logs" in the main panel). Fix:
  annotate/suppress encrypted snapshots when no session key is held, add a key-entry affordance, and always
  surface "Reveal logs" in the main panel.

## Phase E — Test infrastructure sweep + methodology (Track 4)

- Real-encrypted-file round-trips (encrypt→kill→reopen); **config↔on-disk invariant checker** run in the suite;
  tests use `load_config` not hand-built config; concurrency/torture + property tests over interleavings.
- Update `quality-matrix.md`: durability (fsync/atomic), config↔disk consistency, crash-window, and
  "no production I/O cfg-compiled-out" rules. The direct answer to "why didn't we test this".
- **[carry-in, from the Phase-D restore review]** flaky test
  `diagnostics::tests::rust_panic_payloads_keep_owned_strings_and_fallback_text`: `capture_panic_summary` installs
  a PROCESS-GLOBAL panic hook guarded only against its sibling diagnostics test, not the rest of the suite, so
  concurrent panic-injecting tests (migration/fault, fault_inject, the new restore crash tests) race it (~3/8
  full-suite runs; reproduces WITHOUT the restore tests — root cause predates this work in unmodified
  `diagnostics.rs`). Filtered gate runs (`archive::`/`migration`/`worker`) don't hit it. Fix in this sweep:
  isolate the global-hook test (serialize it, or scope the guard to the whole suite).

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
