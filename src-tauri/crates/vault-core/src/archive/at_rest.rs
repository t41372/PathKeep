//! At-rest encryption reconciliation for the canonical encrypted-tier databases.
//!
//! ## Responsibilities
//! - Detect when `source-evidence.sqlite`'s on-disk encryption state has drifted
//!   from the configured `archive_mode`, and converge it in place (encrypt a
//!   plaintext file, or decrypt an encrypted one) atomically and verifiably.
//! - Migrate `source-evidence.sqlite` in lockstep with an archive rekey.
//! - Phase C launch-time auto-heal ([`recover_archive_on_launch`]): on every app
//!   start, recover any interrupted import/rekey, then reconcile a STALE `config.json`
//!   to the canonical history-vault's REAL on-disk at-rest mode (header reads only, no
//!   key, no DB open). This is the case the source-evidence reconcile above CANNOT heal —
//!   when the config itself is the wrong one (the 2026-06-30 incident: both canonical
//!   DBs encrypted on disk while config says `Plaintext` and there is no journal, so
//!   the open hit `SQLITE_NOTADB` and dead-ended). Healing config to the history-vault's
//!   real mode un-bricks the CANONICAL archive and turns that brick into the graceful
//!   locked unlock-prompt. Source-evidence at-rest convergence is left to the keyed
//!   on-unlock reconcile and self-heals only in the ENCRYPT direction (see
//!   [`recover_archive_on_launch`] for the decrypt-direction caveat).
//!
//! ## Not responsible for
//! - Re-keying or migrating the canonical archive (`history-vault.sqlite`) bytes —
//!   owned by the rekey flow in `maintenance.rs`. The launch heal only corrects the
//!   config's recorded at-rest MODE to match the bytes already on disk; it never
//!   rewrites a canonical DB.
//! - The derived sidecars (search / intelligence / agent) — plaintext by design.
//!
//! ## Why this exists
//! The historical rekey migrated only the archive, leaving `source-evidence` in
//! its prior at-rest mode. When the prior mode was plaintext and the new mode
//! encrypted, `open_source_evidence_connection` then applied `PRAGMA key` to a
//! plaintext file; SQLCipher decoded the plaintext header as ciphertext, derived
//! a bogus page size, and aborted with `SQLITE_NOMEM` (out of memory) on *every*
//! backup — foreground and scheduled. Reconciling the at-rest *mode* fixes that
//! (the confirmed bug) and self-heals installs already in the drifted state.
//!
//! Scope: this handles MODE drift (plaintext-vs-encrypted), detected from the
//! file header without a key. It cannot detect or repair KEY drift — a file
//! encrypted with a stale passphrase (possible only on installs that rotated the
//! passphrase under the old archive-only rekey) looks identical to a
//! correctly-keyed one, and recovering it would need the old passphrase we never
//! store. The fixed rekey prevents new key drift by migrating both DBs in
//! lockstep; pre-existing key drift is tracked as a separate follow-up.

use super::{
    ArchiveOpGate, ArchiveWriteLock, apply_cipher_key, current_timezone_name,
    export_archive_database,
};
use crate::{
    config::{ProjectPaths, ensure_paths, load_config, save_config},
    models::{AppConfig, ArchiveMode, RecoverySnapshot},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, io::Read, path::Path, time::Duration as StdDuration};

/// The 16-byte magic every plaintext SQLite database file begins with. An
/// encrypted SQLCipher file starts with a random salt instead, so the presence
/// of this header is a cheap, key-free signal that a file is plaintext.
const SQLITE_FILE_HEADER: &[u8; 16] = b"SQLite format 3\0";

const BUSY_TIMEOUT: StdDuration = StdDuration::from_secs(5);

/// On-disk encryption state of a database file, detected without a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiskEncryptionMode {
    /// File begins with the SQLite plaintext header.
    Plaintext,
    /// File exists but does not begin with the plaintext header (SQLCipher salt).
    Encrypted,
    /// File is missing or shorter than the 16-byte header (nothing to reconcile).
    Absent,
}

/// Reads the first 16 bytes to classify a file's at-rest mode without a key.
pub(crate) fn detect_disk_encryption_mode(path: &Path) -> DiskEncryptionMode {
    let mut header = [0u8; 16];
    match fs::File::open(path).and_then(|mut file| file.read_exact(&mut header)) {
        Ok(()) if &header == SQLITE_FILE_HEADER => DiskEncryptionMode::Plaintext,
        Ok(()) => DiskEncryptionMode::Encrypted,
        Err(_) => DiskEncryptionMode::Absent,
    }
}

// --- Phase C: launch-time at-rest reconcile + crash-recovery -------------------------------------

/// Error-message prefix the worker tags a launch-recovery failure with so the FE can route it
/// to the Phase-D recovery screen instead of treating it as an opaque bootstrap error. Mirrors
/// `IMPORT_SOURCE_KEY_REQUIRED_PREFIX`; the JSON [`ArchiveRecoveryReport`] rides after the colon.
pub const ARCHIVE_RECOVERY_REQUIRED_PREFIX: &str = "archive_recovery_required";

/// Why a launch-time recovery could not safely self-heal — the Phase-D recovery-screen
/// classification. Serializable; `DiskEncryptionMode` is deliberately NOT exposed (the public
/// surface speaks [`ArchiveMode`], mapping an absent/unreadable file to `None`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveRecoveryKind {
    /// An interrupted whole-app import could not reach one consistent at-rest mode.
    InterruptedImportModeDrift,
    /// An interrupted rekey could not be resolved (e.g. the canonical archive is gone).
    InterruptedRekeyUnresolved,
    /// An interrupted full-archive restore could not be completed or rolled back (snapshot +
    /// quarantined originals both unusable).
    InterruptedRestoreUnresolved,
    /// Config↔file at-rest drift that could not be safely reconciled (reserved for Phase D;
    /// the launch heal converges every observable drift, so this is not produced today).
    AtRestDriftUnresolved,
}

/// The structured, serializable feed the worker turns into the Phase-D recovery screen when a
/// launch recovery cannot self-heal. Carries the detected modes + the verified safety snapshots
/// a one-click restore can choose from, plus the underlying error chain for the screen and logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRecoveryReport {
    /// What kind of unrecoverable state was found.
    pub kind: ArchiveRecoveryKind,
    /// The at-rest mode `config.json` currently declares.
    pub config_mode: ArchiveMode,
    /// The canonical history-vault's real on-disk mode (`None` = absent / unreadable).
    pub history_vault_mode: Option<ArchiveMode>,
    /// The source-evidence DB's real on-disk mode (`None` = absent / unreadable).
    pub source_evidence_mode: Option<ArchiveMode>,
    /// Verified rekey safety-snapshot paths a Phase-D one-click restore can offer. Legacy
    /// rekey-only path-string list, kept for the existing wire contract.
    pub available_snapshots: Vec<String>,
    /// Rich, keyless metadata for every verified full-archive safety snapshot (rekey/reconcile/
    /// import) the Phase-D recovery screen can offer. Superset of `available_snapshots`.
    pub recovery_snapshots: Vec<RecoverySnapshot>,
    /// The underlying error chain (`format!("{err:#}")`), for the recovery screen + logs.
    pub detail: String,
}

/// Outcome of the launch-time at-rest reconcile.
///
/// Internally tagged (`outcome`) so the FE can switch on a single discriminant. `Healed` and
/// `Unrecoverable` carry the data Phase D needs; `Healthy` is the overwhelmingly common path.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum LaunchRecovery {
    /// Config already matches the canonical file's real at-rest mode, OR the archive is
    /// uninitialized (nothing on disk yet). The cheap no-op path.
    Healthy,
    /// `config.json` was corrected to the canonical files' real on-disk at-rest mode (the
    /// 2026-06-30 incident: a stale `Plaintext` config over encrypted files).
    Healed {
        /// The stale mode the config declared before the heal.
        from_mode: ArchiveMode,
        /// The real on-disk mode the config was corrected to.
        to_mode: ArchiveMode,
    },
    /// The state cannot be safely reconciled at launch — feeds the Phase-D recovery screen.
    Unrecoverable(ArchiveRecoveryReport),
}

/// Maps a key-free on-disk detection to the public [`ArchiveMode`], collapsing
/// `Absent`/unreadable to `None` so `DiskEncryptionMode` never crosses the public surface.
fn disk_mode_to_archive_mode(mode: DiskEncryptionMode) -> Option<ArchiveMode> {
    match mode {
        DiskEncryptionMode::Plaintext => Some(ArchiveMode::Plaintext),
        DiskEncryptionMode::Encrypted => Some(ArchiveMode::Encrypted),
        DiskEncryptionMode::Absent => None,
    }
}

/// The FULLY-CONVERGED-end-state cross-file at-rest invariant: the mode recorded in `config.json`
/// equals the REAL on-disk at-rest mode of every canonical database (`history-vault` AND
/// `source-evidence`) that exists on disk.
///
/// This is the check that would have caught the 2026-06-30 incident BEFORE it shipped through a
/// 100%-green gate: encrypted history-vault + source-evidence on disk under a `Plaintext` config (a
/// rekey cut AFTER the file swap but BEFORE config was written), which bricked the next open with
/// `SQLITE_NOTADB`. The gate measured code EXECUTION (line/function coverage), never this cross-FILE
/// BEHAVIOR, so nothing tripped — the methodology hole this checker + its post-condition tests close.
/// Asserting BOTH canonical DBs also re-catches the SOURCE-EVIDENCE drift mode of the incident (an
/// encrypted config over a plaintext source-evidence → the `SQLITE_NOMEM` backup failures).
///
/// It reads through the REAL [`load_config`] (never a hand-built `AppConfig`, so the true config-load
/// path is exercised) and each canonical DB's ACTUAL on-disk mode via [`detect_disk_encryption_mode`]
/// — HEADER-ONLY, KEY-FREE, no DB open — so it is cheap enough to assert after every archive-mutation
/// test even on a 14.4M-row archive. An ABSENT canonical database is skipped (uninitialized /
/// rebuildable — nothing to diverge from); every PRESENT one must match. Returns `Err` naming the
/// exact divergence so a post-condition `.expect(...)` prints the offending shape.
///
/// SCOPE — this asserts the FULLY-CONVERGED end state that a SUCCESSFUL mutation (or a completed
/// encrypt-direction / rollback recovery) reaches, NOT the production LAUNCH invariant. The production
/// self-heal [`recover_archive_on_launch`] converges only the canonical history-vault; source-evidence
/// decrypt-direction drift (source-evidence still Encrypted under a healed Plaintext config) is a
/// deliberately-DEFERRED, known-degraded follow-up that [`super::recover_interrupted_rekey`]
/// fail-closes rather than silently completes. So this checker is STRICTER than the launch guarantee
/// and must NOT be promoted into a production launch assertion as-is, nor threaded as a post-condition
/// after a decrypt-direction recovery that legitimately leaves source-evidence still-encrypted — every
/// caller here runs it only after a fully-converged success/rollback, where both DBs must match.
///
/// It is therefore `#[cfg(test)]` — compiled into the MEASURED test/coverage build (its own tests
/// cover both arms) but not the production `lib`, mirroring `fault_inject`'s `#[cfg(test)]` arming
/// helpers; keeping it test-facing also avoids paying a second header read on every launch.
#[cfg(test)]
pub(crate) fn check_config_disk_consistency(paths: &ProjectPaths) -> Result<()> {
    let config = load_config(paths)
        .context("loading config.json for the config\u{2194}disk at-rest invariant check")?;
    let expected = config.archive_mode;
    for (label, path) in [
        ("history-vault", &paths.archive_database_path),
        ("source-evidence", &paths.source_evidence_database_path),
    ] {
        // Absent / unreadable header => not yet created (or rebuildable): nothing to diverge from.
        let Some(on_disk) = disk_mode_to_archive_mode(detect_disk_encryption_mode(path)) else {
            continue;
        };
        if on_disk != expected {
            anyhow::bail!(
                "config\u{2194}disk at-rest invariant violated: config.json declares {expected:?} but \
                 the {label} database on disk is {on_disk:?} ({}). This is the 2026-06-30 incident \
                 shape (encrypted files under a Plaintext config brick the next open with \
                 SQLITE_NOTADB); a committed archive must never diverge from its config.",
                path.display(),
            );
        }
    }
    Ok(())
}

/// Lists the verified rekey safety-snapshot files (`raw-snapshots/rekey/*.sqlite`) a Phase-D
/// restore could offer. Best-effort + cheap: a directory listing, never a DB open. Sorted for
/// a stable, newest-last presentation.
fn available_verified_snapshots(paths: &ProjectPaths) -> Vec<String> {
    let rekey_dir = paths.raw_snapshots_dir.join("rekey");
    let Ok(entries) = fs::read_dir(&rekey_dir) else {
        return Vec::new();
    };
    let mut snapshots: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("sqlite"))
        .map(|path| path.display().to_string())
        .collect();
    snapshots.sort();
    snapshots
}

/// Lists every verified full-archive safety snapshot the Phase-D recovery GUI can offer, with
/// rich KEYLESS metadata (capture time, size, a cheap "does it open?" signal, which whole-archive
/// rewrite op produced it).
///
/// Why it exists separately from [`available_verified_snapshots`]: the legacy helper feeds the
/// existing `available_snapshots: Vec<String>` field (rekey-only, path strings) that older tests
/// pin; this richer surface drives the recovery SCREEN, which needs per-snapshot metadata to let
/// the user choose a backstop confidently.
///
/// PERFORMANCE: best-effort + cheap. A directory scan, then per file a 16-byte header read and —
/// for plaintext files ONLY — a page-1 `PRAGMA schema_version` (NEVER a 14.4M-row b-tree walk).
/// Encrypted snapshots get a structural size-only check because we hold no key here; the
/// authoritative keyed `quick_check` runs at restore time (D1). Sorted NEWEST FIRST so the
/// recovery screen defaults to the freshest backstop. A missing/empty `raw-snapshots/` yields an
/// empty list rather than an error.
pub fn list_recovery_snapshots(paths: &ProjectPaths) -> Vec<RecoverySnapshot> {
    /// Bucket subdirectory names mapped to a known `source_op`; anything else is `"unknown"`.
    const KNOWN_OPS: [&str; 4] = ["rekey", "reconcile", "import", "periodic"];

    let mut snapshots: Vec<RecoverySnapshot> = Vec::new();
    let Ok(buckets) = fs::read_dir(&paths.raw_snapshots_dir) else {
        return snapshots;
    };
    for bucket in buckets.flatten() {
        let bucket_path = bucket.path();
        if !bucket_path.is_dir() {
            continue;
        }
        let source_op = bucket_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| KNOWN_OPS.contains(name))
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string());
        // `Result::into_iter().flatten().flatten()` skips an unreadable bucket dir AND any
        // per-entry read error without an extra branch (both are vanishingly rare here).
        for entry in fs::read_dir(&bucket_path).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("sqlite") {
                continue;
            }
            // `fs::metadata` follows symlinks, so a dangling snapshot symlink errors here and is
            // skipped (rather than surfacing a bogus zero-size entry).
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let size_bytes = metadata.len();
            let created_at = metadata
                .modified()
                .ok()
                .map(|mtime| chrono::DateTime::<chrono::Utc>::from(mtime).to_rfc3339());
            let path_string = path.display().to_string();
            let disk_mode = detect_disk_encryption_mode(&path);
            snapshots.push(RecoverySnapshot {
                id: path_string.clone(),
                verified_openable: snapshot_is_openable_keyless(disk_mode, &path, size_bytes),
                encrypted: matches!(disk_mode, DiskEncryptionMode::Encrypted),
                // Short, stable English fallback. The FE localizes from `source_op` + `created_at`.
                label: format!("Safety snapshot ({source_op})"),
                source_op: source_op.clone(),
                size_bytes,
                created_at,
                path: path_string,
            });
        }
    }
    // Newest first (created_at desc), path desc as a stable tiebreak so equal mtimes still order
    // deterministically. `None` mtimes sort last under the descending compare.
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.path.cmp(&a.path)));
    snapshots
}

/// KEYLESS "does this snapshot open?" probe. Plaintext: a page-1 `PRAGMA schema_version` (header
/// only — it does NOT walk the b-tree, so a 14.4M-row file is as cheap as a tiny one). Encrypted:
/// structural-only (`>= 512` bytes = at least one small page) because we hold no key to decrypt
/// it here; the authoritative keyed `quick_check` runs at restore time. `Absent`/unreadable header
/// is never openable.
fn snapshot_is_openable_keyless(mode: DiskEncryptionMode, path: &Path, size_bytes: u64) -> bool {
    match mode {
        DiskEncryptionMode::Absent => false,
        DiskEncryptionMode::Encrypted => size_bytes >= 512,
        DiskEncryptionMode::Plaintext => {
            // READ-ONLY open: the default RW+CREATE flags would mislabel a valid-but-read-only
            // plaintext snapshot as un-openable, which both hides a good backstop from the recovery
            // GUI AND lets `prune_snapshot_bucket` compute `protected = None` and delete the
            // last-good snapshot. Snapshots are checkpoint-TRUNCATE'd, so a read-only open needs no
            // WAL. A single fallible chain (open -> busy_timeout -> header-only `schema_version`)
            // folds every failure mode into one `is_ok()` — a malformed/truncated/unreadable file
            // surfaces as `false` here without an extra, list-unreachable early-return branch.
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .and_then(|connection| {
                    connection.busy_timeout(BUSY_TIMEOUT)?;
                    connection.query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
                })
                .is_ok()
        }
    }
}

/// Builds the recovery-screen report for an unrecoverable launch state, reading both canonical
/// DBs' real on-disk modes (header-only) and the available safety snapshots.
fn build_recovery_report(
    paths: &ProjectPaths,
    config: &AppConfig,
    kind: ArchiveRecoveryKind,
    error: &anyhow::Error,
) -> ArchiveRecoveryReport {
    ArchiveRecoveryReport {
        kind,
        config_mode: config.archive_mode.clone(),
        history_vault_mode: disk_mode_to_archive_mode(detect_disk_encryption_mode(
            &paths.archive_database_path,
        )),
        source_evidence_mode: disk_mode_to_archive_mode(detect_disk_encryption_mode(
            &paths.source_evidence_database_path,
        )),
        available_snapshots: available_verified_snapshots(paths),
        recovery_snapshots: list_recovery_snapshots(paths),
        detail: format!("{error:#}"),
    }
}

/// Launch-time auto-heal for the canonical archive's config↔file at-rest drift.
///
/// THE 2026-06-30 incident this closes: both canonical DBs SQLCipher-encrypted ON DISK while
/// `config.json` says `archive_mode = "Plaintext"` and there is NO journal — a rekey
/// (Plaintext→Encrypted) interrupted AFTER the file swap but BEFORE config was durably updated.
/// The next open applied no key to an encrypted file → `SQLITE_NOTADB` → a dead-end error page.
/// This converges the STALE config to the canonical history-vault's real at-rest mode so the open
/// instead reaches the graceful locked unlock-prompt (encrypted) or plaintext open — the CANONICAL
/// archive is un-bricked in BOTH rekey directions (config always ends up matching the history-vault
/// header, so no NOTADB).
///
/// SCOPE — source-evidence: only the canonical archive (history-vault) is reconciled at this
/// keyless launch. Source-evidence at-rest drift left by an interrupted rekey self-heals on the
/// next KEYED open ONLY in the ENCRYPT direction (config now Encrypted → the on-unlock
/// [`reconcile_archive_encryption`] re-encrypts a plaintext source-evidence under the unlock key).
/// The DECRYPT direction (config now Plaintext, source-evidence still Encrypted) needs the OLD key,
/// unavailable at a keyless plaintext launch, so it surfaces as recurring backup failures and is
/// tracked as the key/decrypt-direction follow-up — it does NOT silently self-heal.
///
/// LOCK-FREE FAST PATH — launch must NEVER freeze: the overwhelmingly common HEALTHY launch
/// acquires NO lock. A cheap unlocked pre-check ([`launch_is_provably_healthy`]) — two marker
/// `stat`s + the canonical history-vault's 16-byte header + the small `config.json`, with NO
/// gate, NO flock, NO DB open/scan — returns [`LaunchRecovery::Healthy`] immediately when no crash
/// marker is present AND config already matches the file's real at-rest mode. So even while an
/// out-of-process scheduled backup holds the cross-process write lock for the minutes-long
/// duration of a 14.4M-row backup, a healthy GUI launch is never blocked behind it.
///
/// LOCKED RECOVERY — only when the pre-check finds a marker OR config↔file drift does it take the
/// in-process [`ArchiveOpGate`] FIRST, then the cross-process [`ArchiveWriteLock`] (BLOCKING), and
/// run [`recover_archive_on_launch_locked`]. Blocking here is correct and rare: an archive that
/// genuinely needs recovery MUST serialize against concurrent writers, and we deliberately do NOT
/// try-and-defer — the config↔file step-3 reconcile has no other actor, so deferring could leave
/// launch to open a known half-state (the NOTADB brick) that nothing else would heal. The locked
/// body RE-READS every signal under the lock and is the SOLE source of truth, so the unlocked
/// pre-check is purely a fast-path gate (TOCTOU-safe).
///
/// FAIL-CLOSED decrypt direction: an interrupted Encrypted→Plaintext rekey whose history-vault
/// swap landed but left source-evidence still Encrypted is NOT silently completed (that would
/// commit a Plaintext config decryptable only with the now-unprompted old key); it surfaces as
/// [`LaunchRecovery::Unrecoverable`] with the marker LEFT (see [`super::recover_interrupted_rekey`]).
/// The encrypt direction (Plaintext→Encrypted) self-heals as before.
///
/// `_database_key` is currently UNUSED: every detection here is header-only / key-free, so the
/// heal needs no key. It is threaded so a future keyed convergence (migrating source-evidence
/// at launch when the key is already in hand) can use it without a signature change.
/// The cheap, UNLOCKED launch pre-check: `true` when the archive is provably healthy so the full
/// locked recovery can be skipped having taken NO lock — the 99.9% launch.
///
/// Reads ONLY a few `stat`s + the canonical history-vault's 16-byte header + the small
/// `config.json`: NO gate, NO flock, NO DB open/scan. Returns `false` (pay for the lock and run
/// the authoritative locked recovery) the moment ANY crash marker (import / rekey / restore) is
/// present OR the canonical history-vault's real on-disk at-rest mode disagrees with the recorded
/// config. The marker check returns early, so the `Absent => true` shortcut below CANNOT fire while
/// a restore marker exists (a crash mid-restore left the canonical absent on purpose). It is the source
/// of truth for NOTHING — the locked body re-reads every signal and decides — so a `false` here
/// only ever means "let the locked body confirm", never a heal by itself. It reads the ON-DISK
/// config (the worker saved it immediately before calling, so it equals the passed `config`;
/// falling back to the passed config keeps the check correct if `config.json` is briefly
/// unreadable).
fn launch_is_provably_healthy(paths: &ProjectPaths, config: &AppConfig) -> bool {
    if crate::migration::interrupted_import_marker_present(paths)
        || super::interrupted_rekey_marker_present(paths)
        || super::interrupted_restore_marker_present(paths)
    {
        return false;
    }
    let recorded = load_config(paths)
        .map(|on_disk| on_disk.archive_mode)
        .unwrap_or_else(|_| config.archive_mode.clone());
    match detect_disk_encryption_mode(&paths.archive_database_path) {
        // Absent: uninitialized / fresh — nothing on disk to drift from.
        DiskEncryptionMode::Absent => true,
        DiskEncryptionMode::Plaintext => recorded == ArchiveMode::Plaintext,
        DiskEncryptionMode::Encrypted => recorded == ArchiveMode::Encrypted,
    }
}

pub fn recover_archive_on_launch(
    paths: &ProjectPaths,
    config: &AppConfig,
    _database_key: Option<&str>,
) -> Result<LaunchRecovery> {
    // FAST PATH (no lock): a provably-healthy launch must never block, even while an
    // out-of-process scheduled backup holds the cross-process write lock for minutes.
    if launch_is_provably_healthy(paths, config) {
        return Ok(LaunchRecovery::Healthy);
    }
    // A marker is present OR config↔file at-rest drift was detected: this archive genuinely needs
    // recovery, which MUST serialize against concurrent writers. Take the in-process gate FIRST,
    // then the cross-process flock (BLOCKING — correct and rare). The locked body RE-READS every
    // signal under the lock and is the SOLE source of truth (TOCTOU-safe).
    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for launch-time at-rest recovery")?;
    recover_archive_on_launch_locked(paths, config, _database_key)
}

/// Runs the launch-time recovery assuming the caller already holds the top-level
/// [`ArchiveOpGate`] + [`ArchiveWriteLock`].
///
/// The nested helpers it calls (`recover_interrupted_import`, `recover_interrupted_rekey`,
/// `recover_interrupted_restore`) take ONLY the reentrant [`ArchiveWriteLock`] transitively, NEVER
/// the non-reentrant [`ArchiveOpGate`], so they cannot self-deadlock against the gate held above. It
/// is reached ONLY after the unlocked pre-check found a marker (import / rekey / restore) or
/// config↔file drift (or as the TOCTOU re-check under the lock). On a healthy
/// archive this is a cheap no-op: two marker `stat`s, one 16-byte history-vault header read, and
/// one small `config.json` read, with NO `open_archive_connection`, NO migration, NO scan, and
/// NO config write. (The second header read — source-evidence — happens only inside
/// `build_recovery_report` on the Unrecoverable branch.)
fn recover_archive_on_launch_locked(
    paths: &ProjectPaths,
    config: &AppConfig,
    _database_key: Option<&str>,
) -> Result<LaunchRecovery> {
    // A pending full-archive restore SUPERSEDES any interrupted import/rekey: it replaces the whole
    // canonical archive, so steps (1)+(2) are SKIPPED while it is pending. Running them first would
    // let one that fails closed on the restore's absent-canonical window (e.g. rekey recovery hitting
    // the Absent history-vault) surface Unrecoverable and STARVE the auto-completable restore — stuck
    // on every launch. The restore's own quarantine + `clear_superseded_crash_markers` retire those
    // stale markers, so skipping their recovery here loses nothing.
    let restore_pending = super::interrupted_restore_marker_present(paths);
    if !restore_pending {
        // (1) Recover a whole-app import whose commit phase was cut by a crash. `recover_interrupted_import`
        // leaves its marker on Err, so we surface — never clear — an unrecoverable import.
        if let Err(error) = crate::migration::recover_interrupted_import(paths) {
            return Ok(LaunchRecovery::Unrecoverable(build_recovery_report(
                paths,
                config,
                ArchiveRecoveryKind::InterruptedImportModeDrift,
                &error,
            )));
        }

        // (2) Recover an interrupted rekey (the marker the rekey swap writes). Fail-closed when the
        // canonical archive is gone — `recover_interrupted_rekey` leaves the marker on Err.
        if let Err(error) = super::recover_interrupted_rekey(paths) {
            return Ok(LaunchRecovery::Unrecoverable(build_recovery_report(
                paths,
                config,
                ArchiveRecoveryKind::InterruptedRekeyUnresolved,
                &error,
            )));
        }
    }

    // (3) Complete or roll back a full-archive restore whose quarantine→install→commit window was cut
    // by a crash. `recover_interrupted_restore` leaves its marker on Err, so we surface — never clear —
    // an unrecoverable restore (rather than letting step 4 see an Absent canonical and boot an EMPTY
    // archive). It is KEY-FREE: it re-installs the still-available snapshot (a file copy) or rolls back
    // the quarantined originals, then converges config; the keyed verify + source-evidence rebuild are
    // deferred to the next keyed open (mirrors the rekey source-evidence deferral).
    if let Err(error) = super::recover_interrupted_restore(paths) {
        return Ok(LaunchRecovery::Unrecoverable(build_recovery_report(
            paths,
            config,
            ArchiveRecoveryKind::InterruptedRestoreUnresolved,
            &error,
        )));
    }

    // (4) Config↔file at-rest drift, HEADER READS ONLY (never open/scan the DB — the 14.4M-row
    // constraint). The canonical history-vault is the authority for whether the next open needs a
    // key, so reconciling config to its real on-disk mode un-bricks the canonical archive in BOTH
    // rekey directions (config always ends up matching the history-vault header — no NOTADB).
    // Source-evidence at-rest drift is NOT reconciled here (it needs a key not held at a keyless
    // launch): it self-heals on the next KEYED open ONLY in the ENCRYPT direction (config now
    // Encrypted -> the unlock key re-encrypts a plaintext source-evidence via the on-unlock
    // `reconcile_archive_encryption`). The DECRYPT direction (config now Plaintext, source-evidence
    // still Encrypted) needs the OLD key, which is not available at a keyless plaintext launch, so
    // it surfaces as recurring backup failures and is tracked as the key/decrypt-direction
    // follow-up — it does NOT silently self-heal.
    let history = detect_disk_encryption_mode(&paths.archive_database_path);
    let target = match history {
        DiskEncryptionMode::Absent => {
            // Absent: uninitialized / fresh — `ensure_archive_initialized` will create it.
            return Ok(LaunchRecovery::Healthy);
        }
        DiskEncryptionMode::Plaintext => ArchiveMode::Plaintext,
        DiskEncryptionMode::Encrypted => ArchiveMode::Encrypted,
    };
    // Steps 1–3 (interrupted import/rekey/restore recovery) may already have rewritten config.json
    // (e.g. complete_rekey_config / reconcile_restore_config preserve the prior settings +
    // initialized=true). Reconcile against the POST-RECOVERY on-disk config so we never clobber that
    // reconstruction with the stale in-memory `config`, and only write when the canonical archive's
    // real on-disk header STILL disagrees with config.json.
    let on_disk = load_config(paths).unwrap_or_else(|_| config.clone());
    if on_disk.archive_mode != target {
        let mut healed = on_disk.clone();
        healed.archive_mode = target.clone();
        save_config(paths, &healed)?;
    }
    // Signal the caller to reload whenever the FINAL canonical mode differs from what it
    // passed in (whether an earlier step or this step changed it), so it never reopens with a
    // stale-mode config (the NOTADB brick).
    if config.archive_mode == target {
        Ok(LaunchRecovery::Healthy)
    } else {
        Ok(LaunchRecovery::Healed { from_mode: config.archive_mode.clone(), to_mode: target })
    }
}

/// Outcome of a source-evidence at-rest reconciliation, surfaced to the UI so a
/// silent self-heal can still be reported honestly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    /// True when source-evidence had to be rewritten to match the configured mode.
    pub repaired: bool,
    /// The drifted on-disk mode that was found (`None` when nothing was repaired).
    pub from_mode: Option<ArchiveMode>,
    /// The mode source-evidence now matches (the configured archive mode).
    pub to_mode: ArchiveMode,
}

/// Converges `source-evidence.sqlite` to `config.archive_mode`, using the
/// already-open `archive` connection for the repair run-ledger entry.
///
/// Cheap no-op (a single 16-byte header read) when source-evidence is absent or
/// already consistent, so it is safe to call on every backup and every unlock.
/// Records a `rekey`/`repair` run for PME transparency only when it actually
/// rewrites the file. `key` is the archive's current key; it both decrypts an
/// encrypted source and encrypts a plaintext one, so the realistic drift cases
/// use it directly.
pub(crate) fn reconcile_source_evidence_with_archive(
    archive: &Connection,
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ReconcileReport> {
    ensure_paths(paths)?;
    let to_mode = config.archive_mode.clone();
    let path = &paths.source_evidence_database_path;

    // First, heal any rewrite interrupted by a crash/power loss: restore the live
    // file if a swap was cut between renames, and scrub a stale (possibly
    // plaintext) backstop left after a completed swap. MUST run before the
    // detect below, or a mid-swap-missing file would read as `Absent` and the
    // next open would silently create an empty database, orphaning the data.
    recover_interrupted_rewrite(path);

    let plan: Option<(Option<&str>, Option<&str>, ArchiveMode)> =
        match (&to_mode, detect_disk_encryption_mode(path)) {
            (ArchiveMode::Encrypted, DiskEncryptionMode::Plaintext) => {
                key.map(|_| (None, key, ArchiveMode::Plaintext))
            }
            (ArchiveMode::Plaintext, DiskEncryptionMode::Encrypted) => {
                key.map(|_| (key, None, ArchiveMode::Encrypted))
            }
            _ => None,
        };

    let Some((open_key, write_key, from_mode)) = plan else {
        return Ok(ReconcileReport { repaired: false, from_mode: None, to_mode });
    };

    let started_at = now_rfc3339();
    archive.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('rekey', 'repair', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, current_timezone_name()],
    )?;
    let run_id = archive.last_insert_rowid();

    let rewrite = rewrite_database_file(path, open_key, write_key);
    finalize_repair_run(archive, run_id, &from_mode, &to_mode, &rewrite)?;
    rewrite?;

    Ok(ReconcileReport { repaired: true, from_mode: Some(from_mode), to_mode })
}

/// Opens the archive itself (for the run ledger) and reconciles source-evidence.
///
/// Used by the on-unlock repair command so the self-heal happens proactively,
/// before the user's next backup. The backup path instead reuses its
/// already-open archive connection via [`reconcile_source_evidence_with_archive`]
/// to avoid re-opening the (potentially multi-GB) archive twice.
///
/// TOP-LEVEL destructive entry (lock-completion block): the at-rest rewrite this drives
/// is a destructive source-evidence swap, so it takes the in-process [`ArchiveOpGate`]
/// (FIRST — excludes a SECOND same-process top-level op, CRIT-5's trigger) + the cross-
/// process [`ArchiveWriteLock`] (so the SEPARATE scheduled backup can never race it on
/// source-evidence) for its whole duration, then delegates to the nested
/// [`reconcile_archive_encryption_locked`].
pub fn reconcile_archive_encryption(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ReconcileReport> {
    let _op_gate = ArchiveOpGate::acquire(paths);
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for at-rest reconcile")?;
    reconcile_archive_encryption_locked(paths, config, key)
}

/// Recovers any interrupted import, then opens the archive and reconciles source-evidence
/// — assuming the caller ALREADY holds the top-level [`ArchiveOpGate`] + [`ArchiveWriteLock`].
///
/// The nested variant: it takes only the reentrant [`ArchiveWriteLock`] transitively
/// (through `recover_interrupted_import`), NEVER the non-reentrant [`ArchiveOpGate`], so a
/// future in-op caller that already holds the gate cannot self-deadlock.
pub(crate) fn reconcile_archive_encryption_locked(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ReconcileReport> {
    // Heal a whole-app import whose commit phase was cut by a crash BEFORE the
    // archive is opened: an interrupted-import marker means the two canonical
    // renames + config write did not all land, so restore the consistent pre-import
    // state first (mirrors how `recover_interrupted_rewrite` is woven into the
    // reconcile family). Recover-first now runs under the lock at ALL the destructive
    // pre-open sites: the unlock-path reconcile (here), the backup pre-open path
    // (`run_backup_with_progress`), the rekey pre-open path (`rekey_archive`), AND — as
    // of the lock-completion block — the retention-prune (`run_retention_prune`) and
    // snapshot-restore (`run_snapshot_restore`) pre-open paths.
    crate::migration::recover_interrupted_import(paths)?;
    ensure_paths(paths)?;
    let archive = super::open_archive_connection(paths, config, key)?;
    // D3: capture a VERIFIED full-archive safety snapshot BEFORE the whole-archive rewrite, but
    // ONLY when a rewrite is actually pending. A no-op reconcile (and the frequent BACKUP path,
    // which calls `reconcile_source_evidence_with_archive` directly) stays snapshot-free, so we
    // never copy + quick_check the 14.4M-row DB on a hot path for zero added safety.
    if source_evidence_rewrite_pending(paths, config, key) {
        super::create_verified_safety_snapshot(paths, &archive, config, key, "reconcile")?;
    }
    reconcile_source_evidence_with_archive(&archive, paths, config, key)
}

/// `true` when [`reconcile_source_evidence_with_archive`] is about to REWRITE the source-evidence
/// file — the only case worth a verified backstop: config↔source-evidence at-rest drift in either
/// direction AND a key in hand. A cheap header read that replicates the reconcile `plan` match, so
/// the common no-op reconcile pays nothing.
fn source_evidence_rewrite_pending(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> bool {
    if key.is_none() {
        return false;
    }
    matches!(
        (&config.archive_mode, detect_disk_encryption_mode(&paths.source_evidence_database_path)),
        (ArchiveMode::Encrypted, DiskEncryptionMode::Plaintext)
            | (ArchiveMode::Plaintext, DiskEncryptionMode::Encrypted)
    )
}

/// Migrates source-evidence to the post-rekey at-rest mode, in lockstep with the
/// archive.
///
/// Detects the file's ACTUAL on-disk mode (it may already have drifted from
/// `current_config` on installs hit by the archive-only rekey) to pick the open
/// key, and writes with `target_key` (`None` when the new mode is plaintext).
pub(crate) fn migrate_source_evidence_for_rekey(
    paths: &ProjectPaths,
    old_key: Option<&str>,
    target_key: Option<&str>,
) -> Result<()> {
    let path = &paths.source_evidence_database_path;
    recover_interrupted_rewrite(path);
    let open_key = match detect_disk_encryption_mode(path) {
        DiskEncryptionMode::Encrypted => old_key,
        DiskEncryptionMode::Plaintext => None,
        DiskEncryptionMode::Absent => return Ok(()),
    };
    rewrite_database_file(path, open_key, target_key)
}

/// Rewrites one SQLite/SQLCipher database file from `open_key` (`None` =
/// plaintext source) to `write_key` (`None` = plaintext target), atomically.
///
/// The new copy is produced via `sqlcipher_export` into a sibling temp file
/// (`export_archive_database` propagates any write failure), then swapped in
/// atomically. The previous file is kept as a transient `.preencrypt` backstop
/// until the swap succeeds, then removed — we deliberately do NOT persist a
/// lingering plaintext snapshot of evidence the user just asked to encrypt.
pub(crate) fn rewrite_database_file(
    path: &Path,
    open_key: Option<&str>,
    write_key: Option<&str>,
) -> Result<()> {
    // Heal any prior interrupted rewrite (restore a cut swap, scrub stale
    // siblings) before starting a new one — never blindly delete a `.preencrypt`
    // that may be the only surviving copy.
    recover_interrupted_rewrite(path);
    let temp_path = path.with_extension("reencrypt.sqlite");
    let backup_path = path.with_extension("preencrypt.sqlite");

    export_source_to_temp(path, &temp_path, open_key, write_key)?;
    swap_in_place(path, &temp_path, &backup_path)?;
    remove_stale_sidecars(path);
    let _ = fs::remove_file(&backup_path);
    Ok(())
}

/// Recovers from a rewrite interrupted by a crash or power loss, and scrubs any
/// stale swap siblings left behind. Idempotent and cheap (a few `stat`s).
///
/// Two windows are handled:
/// - Crash *between* the two swap renames: the live file is missing but the
///   pre-rewrite copy survives at `.preencrypt`. We rename it back so the data is
///   never orphaned (a later reconcile redoes the rewrite).
/// - Crash *after* a completed swap but before the backstop was deleted: the live
///   file is correct and `.preencrypt` is a now-redundant copy — for a
///   plaintext→encrypted repair that is lingering plaintext, so it MUST be
///   removed. The exported `.reencrypt` is always stale and self-contained.
fn recover_interrupted_rewrite(path: &Path) {
    let temp_path = path.with_extension("reencrypt.sqlite");
    let backup_path = path.with_extension("preencrypt.sqlite");
    if !path.exists() && backup_path.exists() {
        let _ = fs::rename(&backup_path, path);
    }
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(&backup_path);
}

/// Opens `path` in its current at-rest mode, confirms it is readable, and
/// exports a self-contained copy to `temp_path` in the target mode.
fn export_source_to_temp(
    path: &Path,
    temp_path: &Path,
    open_key: Option<&str>,
    write_key: Option<&str>,
) -> Result<()> {
    let source = open_in_mode(path, open_key)
        .with_context(|| format!("opening {} for at-rest rewrite", path.display()))?;
    // Confirm the source is actually readable in the assumed mode before export —
    // a wrong-mode open would otherwise surface as a confusing export failure.
    source
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .with_context(|| format!("reading {} before at-rest rewrite", path.display()))?;
    export_archive_database(&source, temp_path, write_key)
}

/// Atomically swaps `temp_path` into `path`, keeping the previous file at
/// `backup_path` so a failed replace can be rolled back.
fn swap_in_place(path: &Path, temp_path: &Path, backup_path: &Path) -> Result<()> {
    fs::rename(path, backup_path)
        .with_context(|| format!("staging at-rest swap for {}", path.display()))?;
    if let Err(error) = fs::rename(temp_path, path) {
        let _ = fs::rename(backup_path, path);
        let _ = fs::remove_file(temp_path);
        return Err(error)
            .with_context(|| format!("replacing {} after at-rest rewrite", path.display()));
    }
    Ok(())
}

fn open_in_mode(path: &Path, key: Option<&str>) -> Result<Connection> {
    let connection =
        Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    if let Some(key) = key {
        apply_cipher_key(&connection, key)?;
    }
    Ok(connection)
}

/// Removes stale rollback/WAL sidecars left next to the freshly swapped-in file.
/// The exported copy is self-contained, so any sidecar present belongs to the
/// previous file and would be wrongly replayed against the new one.
///
/// Shared with the archive rekey swap (`maintenance.rs`), which has the identical
/// "scrub the swapped-in file's foreign WAL so it cannot replay" need.
pub(crate) fn remove_stale_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(std::path::PathBuf::from(format!("{}{}", path.display(), suffix)));
    }
}

fn finalize_repair_run(
    archive: &Connection,
    run_id: i64,
    from_mode: &ArchiveMode,
    to_mode: &ArchiveMode,
    rewrite: &Result<()>,
) -> Result<()> {
    let finished_at = now_rfc3339();
    let (status, error) = match rewrite {
        Ok(()) => ("success", None),
        Err(error) => ("failed", Some(format!("{error:#}"))),
    };
    let stats = json!({
        "operation": "source-evidence-at-rest-repair",
        "fromMode": from_mode,
        "toMode": to_mode,
    });
    archive.execute(
        "UPDATE runs
         SET finished_at = ?1, status = ?2, stats_json = ?3, error_message = ?4
         WHERE id = ?5",
        params![finished_at, status, serde_json::to_string(&stats)?, error, run_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{open_archive_connection, open_source_evidence_connection, rekey_archive};
    use crate::config::{load_config, project_paths_with_root};
    use tempfile::tempdir;

    const KEY: &str = "at-rest-reconcile-test-key";

    /// True when the durable interrupted-rekey marker is present beside the archive DB. The
    /// marker path is private to `maintenance.rs`, so launch-recovery tests assert on the
    /// stable on-disk filename directly.
    fn rekey_journal_present(paths: &ProjectPaths) -> bool {
        paths
            .archive_database_path
            .parent()
            .expect("archive parent")
            .join(".pk-rekey-journal.json")
            .exists()
    }

    /// True when the durable interrupted-import marker is present beside the archive DB.
    fn import_journal_present(paths: &ProjectPaths) -> bool {
        paths
            .archive_database_path
            .parent()
            .expect("archive parent")
            .join(".pk-import-journal.json")
            .exists()
    }

    fn encrypted_config() -> AppConfig {
        AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        }
    }

    fn plaintext_config() -> AppConfig {
        AppConfig {
            archive_mode: ArchiveMode::Plaintext,
            initialized: true,
            ..AppConfig::default()
        }
    }

    /// Seeds a *plaintext* source-evidence database with one row, exactly the
    /// drifted state an archive-only rekey leaves behind.
    fn seed_plaintext_source_evidence(paths: &ProjectPaths) {
        let connection =
            open_source_evidence_connection(paths, &plaintext_config(), None).expect("seed open");
        connection
            .execute(
                "INSERT INTO source_batches (
                   source_profile_id, run_id, source_kind, browser_version,
                   schema_version_text, schema_version_int, schema_fingerprint,
                   parser_version, capability_snapshot_json, coverage_stats_json,
                   artifact_refs_json, notes_json, created_at)
                 VALUES (1, NULL, 'chromium', NULL, NULL, NULL, 'fp', 'pv', '{}', '{}', NULL, NULL, '2026-06-28T00:00:00Z')",
                [],
            )
            .expect("seed row");
    }

    #[test]
    fn detect_disk_encryption_mode_classifies_each_state() {
        let dir = tempdir().expect("tempdir");
        let plaintext = dir.path().join("plain.sqlite");
        fs::write(&plaintext, b"SQLite format 3\0and then some").expect("write plain");
        assert_eq!(detect_disk_encryption_mode(&plaintext), DiskEncryptionMode::Plaintext);

        let encrypted = dir.path().join("enc.sqlite");
        fs::write(&encrypted, [0u8; 32]).expect("write enc");
        assert_eq!(detect_disk_encryption_mode(&encrypted), DiskEncryptionMode::Encrypted);

        let absent = dir.path().join("missing.sqlite");
        assert_eq!(detect_disk_encryption_mode(&absent), DiskEncryptionMode::Absent);

        let too_short = dir.path().join("short.sqlite");
        fs::write(&too_short, b"SQLite").expect("write short");
        assert_eq!(detect_disk_encryption_mode(&too_short), DiskEncryptionMode::Absent);
    }

    #[test]
    fn reproduces_oom_then_self_heals_plaintext_source_evidence() {
        // The regression: an encrypted config over a plaintext source-evidence
        // makes `open_source_evidence_connection` apply PRAGMA key to a plaintext
        // file, which SQLCipher rejects (NOMEM/NOTADB) — the live "out of memory"
        // backup failure.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );

        let config = encrypted_config();
        // The archive must exist + open with the key for the repair run ledger.
        let archive = open_archive_connection(&paths, &config, Some(KEY)).expect("open archive");

        // Bug present: opening source-evidence as if encrypted errors out.
        assert!(
            open_source_evidence_connection(&paths, &config, Some(KEY)).is_err(),
            "plaintext source-evidence opened as encrypted should fail (the OOM)"
        );

        // Auto-repair converges it.
        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, Some(KEY))
            .expect("reconcile");
        assert!(report.repaired);
        assert_eq!(report.from_mode, Some(ArchiveMode::Plaintext));
        assert_eq!(report.to_mode, ArchiveMode::Encrypted);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );

        // Fix proven: the encrypted open now succeeds AND preserves the row.
        let healed =
            open_source_evidence_connection(&paths, &config, Some(KEY)).expect("encrypted re-open");
        let rows: i64 = healed
            .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
            .expect("count");
        assert_eq!(rows, 1, "repair must preserve the source-evidence rows");

        // No lingering plaintext backstop or temp files.
        assert!(!paths.source_evidence_database_path.with_extension("preencrypt.sqlite").exists());
        assert!(!paths.source_evidence_database_path.with_extension("reencrypt.sqlite").exists());
    }

    #[test]
    fn reconcile_is_a_noop_when_already_consistent() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = plaintext_config();
        seed_plaintext_source_evidence(&paths);
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");

        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, None)
            .expect("reconcile");
        assert!(!report.repaired);
        assert_eq!(report.from_mode, None);

        // Absent source-evidence is also a no-op.
        let empty = tempdir().expect("tempdir");
        let empty_paths = project_paths_with_root(empty.path());
        let empty_archive =
            open_archive_connection(&empty_paths, &config, None).expect("open empty archive");
        let absent_report =
            reconcile_source_evidence_with_archive(&empty_archive, &empty_paths, &config, None)
                .expect("reconcile absent");
        assert!(!absent_report.repaired);
    }

    #[test]
    fn reconcile_without_key_leaves_the_file_untouched() {
        // When encrypted config drifts but no key is present (locked), we cannot
        // encrypt — reconcile must not touch the file (the upstream open will
        // surface the honest "key required" error instead).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        let config = encrypted_config();
        // Build a plaintext archive connection purely to host the runs table.
        let archive =
            open_archive_connection(&paths, &plaintext_config(), None).expect("open archive");

        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, None)
            .expect("reconcile");
        assert!(!report.repaired);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );
    }

    #[test]
    fn reconcile_archive_encryption_opens_and_repairs() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        // Materialise an encrypted archive so the public entry point can open it.
        let config = encrypted_config();
        drop(
            open_archive_connection(&paths, &config, Some(KEY)).expect("create encrypted archive"),
        );

        let report =
            reconcile_archive_encryption(&paths, &config, Some(KEY)).expect("reconcile entry");
        assert!(report.repaired);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );
    }

    #[test]
    fn reconcile_archive_encryption_locked_runs_under_a_held_gate_without_deadlock() {
        // Self-deadlock guard (the #1 risk of the lock-completion block): the NESTED
        // `_locked` variant must take ONLY the reentrant write lock, NEVER the non-
        // reentrant top-level gate. We hold the gate + lock on THIS thread (as a real
        // top-level op would) and run `_locked` on a worker; it must complete. If it
        // mistakenly re-acquired the gate this thread holds, the worker would block
        // forever — so a watchdog turns that regression into a fast failure, not a hang.
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        let config = encrypted_config();
        drop(
            open_archive_connection(&paths, &config, Some(KEY)).expect("create encrypted archive"),
        );

        // Hold the top-level guards on the main thread (gate FIRST, then the flock).
        let _gate = ArchiveOpGate::acquire(&paths);
        let _lock = ArchiveWriteLock::acquire(&paths).expect("top-level write lock");

        let worker_paths = paths.clone();
        let worker_config = config.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let report =
                reconcile_archive_encryption_locked(&worker_paths, &worker_config, Some(KEY));
            done_tx.send(report.map(|r| r.repaired)).expect("worker signals completion");
        });

        let repaired = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the nested _locked variant must not re-take the non-reentrant gate (deadlock)")
            .expect("reconcile_locked under a held gate+lock must still succeed");
        assert!(repaired, "the drifted plaintext source-evidence must be repaired");
        worker.join().expect("worker thread");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );
    }

    #[test]
    fn migrate_source_evidence_for_rekey_encrypts_and_decrypts() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);

        // Plaintext -> encrypted (enabling encryption).
        migrate_source_evidence_for_rekey(&paths, None, Some(KEY)).expect("encrypt");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );
        let encrypted =
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY)).expect("open");
        assert_eq!(
            encrypted
                .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get::<_, i64>(0))
                .expect("count"),
            1
        );
        drop(encrypted);

        // Encrypted -> plaintext (disabling encryption); open key detected from disk.
        migrate_source_evidence_for_rekey(&paths, Some(KEY), None).expect("decrypt");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );

        // Absent file is a no-op.
        let empty = tempdir().expect("tempdir");
        let empty_paths = project_paths_with_root(empty.path());
        migrate_source_evidence_for_rekey(&empty_paths, None, Some(KEY)).expect("absent no-op");
    }

    #[test]
    fn rewrite_database_file_rejects_unreadable_source() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("source-evidence.sqlite");
        // A non-SQLite blob cannot be read as plaintext SQLite.
        fs::write(&path, b"not a database at all, just bytes").expect("write garbage");
        let error = rewrite_database_file(&path, None, Some(KEY)).expect_err("garbage source");
        assert!(format!("{error:#}").contains("at-rest rewrite"));
        // The original file is left in place (no half-swap).
        assert!(path.exists());
    }

    #[test]
    fn reconcile_decrypts_drifted_encrypted_source_evidence() {
        // The inverse drift: config is plaintext but source-evidence is still
        // encrypted. Reconcile must decrypt it using the supplied key.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        seed_plaintext_source_evidence(&paths);
        migrate_source_evidence_for_rekey(&paths, None, Some(KEY)).expect("encrypt");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );

        let config = plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        let report = reconcile_source_evidence_with_archive(&archive, &paths, &config, Some(KEY))
            .expect("reconcile");
        assert!(report.repaired);
        assert_eq!(report.from_mode, Some(ArchiveMode::Encrypted));
        assert_eq!(report.to_mode, ArchiveMode::Plaintext);
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Plaintext
        );
    }

    #[test]
    fn reconcile_records_failed_run_when_rewrite_fails() {
        // A file that *looks* plaintext (right header) but is unreadable forces
        // the rewrite to fail; reconcile must finalize the repair run as failed
        // and surface the error rather than swallowing it.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = encrypted_config();
        let archive =
            open_archive_connection(&paths, &config, Some(KEY)).expect("open encrypted archive");
        let mut header = b"SQLite format 3\0".to_vec();
        header.extend_from_slice(b"corrupt body that is not a real page");
        fs::write(&paths.source_evidence_database_path, header).expect("write corrupt");

        let error = reconcile_source_evidence_with_archive(&archive, &paths, &config, Some(KEY))
            .expect_err("rewrite of a corrupt source must fail");
        assert!(format!("{error:#}").contains("at-rest rewrite"));

        let failed: i64 = archive
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE run_type = 'rekey' AND trigger = 'repair' AND status = 'failed'",
                [],
                |row| row.get(0),
            )
            .expect("count failed repair runs");
        assert_eq!(failed, 1, "a failed repair run must be recorded for transparency");
    }

    #[test]
    fn swap_in_place_rolls_back_when_replace_fails() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("db.sqlite");
        let temp = dir.path().join("db.reencrypt.sqlite"); // intentionally missing
        let backup = dir.path().join("db.preencrypt.sqlite");
        fs::write(&path, b"original").expect("write path");

        let error = swap_in_place(&path, &temp, &backup).expect_err("missing temp fails replace");
        assert!(format!("{error:#}").contains("replacing"));
        // Rollback must restore the original, untouched.
        assert!(path.exists());
        assert_eq!(fs::read(&path).expect("read restored"), b"original");
        assert!(!backup.exists());
    }

    #[test]
    fn recover_interrupted_rewrite_restores_then_scrubs() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("source-evidence.sqlite");
        let temp = path.with_extension("reencrypt.sqlite");
        let backup = path.with_extension("preencrypt.sqlite");

        // Crash BETWEEN the two swap renames: live file missing, backstop survives.
        fs::write(&backup, b"original data").expect("write backup");
        fs::write(&temp, b"half-written export").expect("write temp");
        recover_interrupted_rewrite(&path);
        assert!(path.exists(), "the pre-rewrite copy must be restored");
        assert_eq!(fs::read(&path).expect("read restored"), b"original data");
        assert!(!backup.exists());
        assert!(!temp.exists(), "stale export must be scrubbed");

        // Crash AFTER a completed swap: live file present, plaintext backstop lingers.
        fs::write(&backup, b"lingering plaintext").expect("rewrite backstop");
        recover_interrupted_rewrite(&path);
        assert!(path.exists());
        assert_eq!(fs::read(&path).expect("read"), b"original data");
        assert!(!backup.exists(), "lingering plaintext backstop must be scrubbed");

        // Nothing to do when the file is already clean.
        recover_interrupted_rewrite(&path);
        assert!(path.exists());
    }

    // --- Phase C: recover_archive_on_launch ----------------------------------------------------

    #[test]
    fn launch_recovery_heals_a_stale_plaintext_config_over_both_encrypted_dbs() {
        // THE 2026-06-30 incident: both canonical DBs encrypted on disk while config says
        // Plaintext and there is NO journal -> the open hit NOTADB and dead-ended.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        drop(
            open_archive_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted history-vault"),
        );
        drop(
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted source-evidence"),
        );
        save_config(&paths, &plaintext_config()).expect("write stale plaintext config");
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Encrypted
        );

        let outcome =
            recover_archive_on_launch(&paths, &plaintext_config(), None).expect("launch recovery");
        assert!(
            matches!(
                outcome,
                LaunchRecovery::Healed {
                    from_mode: ArchiveMode::Plaintext,
                    to_mode: ArchiveMode::Encrypted,
                }
            ),
            "got {outcome:?}",
        );

        assert!(matches!(load_config(&paths).expect("load").archive_mode, ArchiveMode::Encrypted));
        assert!(!rekey_journal_present(&paths), "the heal must not fabricate a rekey marker");
        assert!(!import_journal_present(&paths), "the heal must not fabricate an import marker");

        // The heal un-bricks the open: encrypted+no-key is now the graceful "key required"
        // unlock prompt (NOT a NOTADB brick), and the key opens it cleanly.
        let healed = load_config(&paths).expect("load healed config");
        let locked_error = open_archive_connection(&paths, &healed, None)
            .expect_err("an encrypted archive opened with no key must error");
        let rendered = format!("{locked_error:#}");
        assert!(rendered.contains("database key is required"), "got: {rendered}");
        drop(
            open_archive_connection(&paths, &healed, Some(KEY))
                .expect("the healed encrypted archive opens with the key"),
        );
    }

    #[test]
    fn launch_recovery_completes_an_interrupted_rekey_swap() {
        // A rekey crashed AFTER the swap but BEFORE config -> launch recovery completes it.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Plaintext history-vault only (no source-evidence -> SE stays Absent = consistent).
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("plaintext history-vault"),
        );
        save_config(&paths, &plaintext_config()).expect("seed plaintext config");
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Absent
        );

        {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(
                "rekey.after_swap_before_config",
            );
            let error =
                rekey_archive(&paths, &plaintext_config(), None, ArchiveMode::Encrypted, Some(KEY))
                    .expect_err("the injected crash must abort the rekey");
            let rendered = format!("{error:#}");
            assert!(
                rendered.contains("simulated error at checkpoint")
                    && rendered.contains("rekey.after_swap_before_config"),
                "the INJECTED fault must propagate, got: {rendered}",
            );
        }

        // Post-crash on disk: history Encrypted, marker present, config still Plaintext.
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Encrypted
        );
        assert!(rekey_journal_present(&paths), "the crashed rekey must leave its marker");
        assert!(matches!(load_config(&paths).expect("load").archive_mode, ArchiveMode::Plaintext));

        let outcome = recover_archive_on_launch(&paths, &load_config(&paths).expect("load"), None)
            .expect("launch recovery");
        assert!(matches!(outcome, LaunchRecovery::Healed { .. }), "got {outcome:?}");
        assert!(!rekey_journal_present(&paths), "recovery must clear the rekey marker");
        assert!(matches!(load_config(&paths).expect("load").archive_mode, ArchiveMode::Encrypted));
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Encrypted
        );
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Absent
        );
        drop(
            open_archive_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("the recovered encrypted archive opens with the key"),
        );
    }

    #[test]
    fn launch_recovery_consumes_an_interrupted_import_marker() {
        // A real durable interrupted-import journal (fresh install, no prior) is consumed by the
        // launch path's recover-first, which removes the half-installed DB and clears the marker.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("half-installed history-vault"),
        );
        let marker =
            paths.archive_database_path.parent().expect("parent").join(".pk-import-journal.json");
        let journal = serde_json::json!({
            "version": 1,
            "timestamp": "2026-06-30T03-00-00Z",
            "canonical": [{
                "target": paths.archive_database_path.to_string_lossy(),
                "had_previous": false,
            }],
            "subtrees": [],
            "previous_config": serde_json::Value::Null,
        });
        fs::write(&marker, serde_json::to_vec(&journal).expect("serialize journal"))
            .expect("seed import marker");
        assert!(import_journal_present(&paths));

        let outcome =
            recover_archive_on_launch(&paths, &plaintext_config(), None).expect("launch recovery");
        // The fresh-install rollback removed the half-installed history-vault, so the archive is
        // now Absent (uninitialized) and the reconcile reports Healthy.
        assert!(matches!(outcome, LaunchRecovery::Healthy), "got {outcome:?}");
        assert!(!import_journal_present(&paths), "the import marker must be consumed by recovery");
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Absent
        );
    }

    #[test]
    fn launch_recovery_reports_unrecoverable_import_mode_drift_with_no_snapshots() {
        // An interrupted whole-app import that left the two canonical DBs at DIFFERENT at-rest
        // modes (history-vault Plaintext, source-evidence Encrypted) cannot reach one consistent
        // mode, so `recover_interrupted_import` -> `ensure_recovered_modes_are_consistent` bails
        // and the launch reconcile surfaces `InterruptedImportModeDrift` (the import-side
        // Unrecoverable arm) with the marker LEFT and config UNTOUCHED. With NO `raw-snapshots/
        // rekey/` dir, `available_verified_snapshots` also hits its read-dir-Err `Vec::new()` branch.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        // history-vault Plaintext, source-evidence Encrypted: the drifted-apart canonical pair the
        // consistency guard refuses to commit a single config over.
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("plaintext history-vault"),
        );
        drop(
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted source-evidence"),
        );
        save_config(&paths, &plaintext_config()).expect("write plaintext config");
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Plaintext
        );
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );

        // Hand-seed an interrupted-import marker (its `ImportJournal` schema is private to
        // migration.rs, so mirror the existing `launch_recovery_consumes_an_interrupted_import_marker`
        // bytes): BOTH canonical paths `had_previous: true` so `rollback_import` leaves them
        // untouched, and `previous_config` = the Plaintext config so the recovery's required mode is
        // Plaintext -> the still-Encrypted source-evidence trips `ensure_recovered_modes_are_consistent`.
        let marker =
            paths.archive_database_path.parent().expect("parent").join(".pk-import-journal.json");
        let previous_config =
            serde_json::to_string(&plaintext_config()).expect("serialize previous config");
        let journal = serde_json::json!({
            "version": 1,
            "timestamp": "2026-06-30T03-00-00Z",
            "canonical": [
                { "target": paths.archive_database_path.to_string_lossy(), "had_previous": true },
                {
                    "target": paths.source_evidence_database_path.to_string_lossy(),
                    "had_previous": true,
                },
            ],
            "subtrees": [],
            "previous_config": previous_config,
        });
        fs::write(&marker, serde_json::to_vec(&journal).expect("serialize journal"))
            .expect("seed import marker");
        assert!(import_journal_present(&paths));
        // No `raw-snapshots/rekey/` dir -> `available_verified_snapshots` reads-dir-Err branch.
        assert!(!paths.raw_snapshots_dir.join("rekey").exists());

        let config_before = fs::read(&paths.config_path).expect("read config before");

        let outcome = recover_archive_on_launch(&paths, &plaintext_config(), None)
            .expect("recovery returns Ok(Unrecoverable), never Err");
        let report = match outcome {
            LaunchRecovery::Unrecoverable(report) => report,
            other => panic!("expected Unrecoverable, got {other:?}"),
        };
        assert!(
            matches!(report.kind, ArchiveRecoveryKind::InterruptedImportModeDrift),
            "got {:?}",
            report.kind,
        );
        assert!(!report.detail.is_empty(), "the report must carry the underlying error chain");
        assert!(
            report.available_snapshots.is_empty(),
            "with no rekey snapshots dir the restore list is empty",
        );

        assert!(import_journal_present(&paths), "the import marker must be LEFT for retry");
        let config_after = fs::read(&paths.config_path).expect("read config after");
        assert_eq!(config_before, config_after, "config.json must NOT be rewritten on the bail");
    }

    #[test]
    fn launch_recovery_falls_back_to_the_passed_config_when_config_json_is_corrupt() {
        // Step 3's on-disk reconcile cannot read a CORRUPT config.json (load_config errors, unlike
        // a merely-absent one which defaults), so it falls back to the in-memory config the caller
        // passed (`unwrap_or_else(|_| config.clone())`) and still heals the mode to the encrypted
        // archive on disk.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted history-vault"),
        );
        // Seed the config dir, then overwrite with bytes that fail to parse so load_config errors.
        save_config(&paths, &plaintext_config()).expect("seed config dir");
        fs::write(&paths.config_path, b"{ not valid json").expect("corrupt config");

        let outcome =
            recover_archive_on_launch(&paths, &plaintext_config(), None).expect("launch recovery");
        assert!(
            matches!(
                outcome,
                LaunchRecovery::Healed {
                    from_mode: ArchiveMode::Plaintext,
                    to_mode: ArchiveMode::Encrypted,
                }
            ),
            "got {outcome:?}",
        );
        // The corrupt config was overwritten with a healed Encrypted config.
        assert!(matches!(
            load_config(&paths).expect("load healed config").archive_mode,
            ArchiveMode::Encrypted,
        ));
    }

    #[test]
    fn launch_recovery_reports_an_unrecoverable_interrupted_rekey_when_history_vault_is_gone() {
        // Fail-closed: a real rekey crash leaves a marker + safety snapshot, then the canonical
        // history-vault is removed so `resolve_interrupted_rekey` bails -> Unrecoverable, marker
        // LEFT, config NOT rewritten.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("plaintext history-vault"),
        );
        save_config(&paths, &plaintext_config()).expect("seed plaintext config");
        {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(
                "rekey.after_swap_before_config",
            );
            rekey_archive(&paths, &plaintext_config(), None, ArchiveMode::Encrypted, Some(KEY))
                .expect_err("the injected crash must abort the rekey");
        }
        assert!(rekey_journal_present(&paths));
        // Remove the canonical archive so detect == Absent -> the recovery fails closed.
        fs::remove_file(&paths.archive_database_path).expect("remove history-vault");
        remove_stale_sidecars(&paths.archive_database_path);

        let config_before = fs::read(&paths.config_path).expect("read config before");

        let outcome = recover_archive_on_launch(&paths, &load_config(&paths).expect("load"), None)
            .expect("recovery returns Ok(Unrecoverable), never Err");
        let report = match outcome {
            LaunchRecovery::Unrecoverable(report) => report,
            other => panic!("expected Unrecoverable, got {other:?}"),
        };
        assert!(matches!(report.kind, ArchiveRecoveryKind::InterruptedRekeyUnresolved));
        assert!(!report.detail.is_empty(), "the report must carry the underlying error chain");
        assert!(
            !report.available_snapshots.is_empty(),
            "the rekey safety snapshot must be offered for restore",
        );
        assert!(report.history_vault_mode.is_none(), "the removed history-vault reads as None");

        assert!(rekey_journal_present(&paths), "the marker must be LEFT on a fail-closed bail");
        let config_after = fs::read(&paths.config_path).expect("read config after");
        assert_eq!(config_before, config_after, "config.json must NOT be rewritten on fail-closed");
    }

    #[test]
    fn launch_recovery_is_a_noop_on_a_consistent_archive() {
        // A consistent Plaintext archive: Healthy, config byte-for-byte unchanged, no markers.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("plaintext history-vault"),
        );
        save_config(&paths, &plaintext_config()).expect("write plaintext config");
        let config_before = fs::read(&paths.config_path).expect("config before");

        let outcome =
            recover_archive_on_launch(&paths, &plaintext_config(), None).expect("launch recovery");
        assert!(matches!(outcome, LaunchRecovery::Healthy), "got {outcome:?}");
        assert_eq!(
            fs::read(&paths.config_path).expect("config after"),
            config_before,
            "config.json must be untouched on the Healthy path",
        );
        assert!(!rekey_journal_present(&paths));
        assert!(!import_journal_present(&paths));
        assert!(
            !paths.raw_snapshots_dir.join("rekey").exists(),
            "the Healthy no-op must not touch the rekey snapshots dir",
        );

        // The `history == Absent` (uninitialized) branch is also Healthy.
        let empty = tempdir().expect("tempdir");
        let empty_paths = project_paths_with_root(empty.path());
        let uninitialized = recover_archive_on_launch(&empty_paths, &plaintext_config(), None)
            .expect("uninitialized recovery");
        assert!(matches!(uninitialized, LaunchRecovery::Healthy), "got {uninitialized:?}");
    }

    #[test]
    fn recover_interrupted_rekey_runs_under_a_held_gate_without_deadlock() {
        // Self-deadlock guard: the NESTED `recover_interrupted_rekey` must take ONLY the reentrant
        // write lock, NEVER the non-reentrant top-level gate. We hold the gate + lock on THIS
        // thread (as the gated `recover_archive_on_launch` does) and run the nested helper on a
        // worker; it must complete. A watchdog turns a self-deadlock regression into a fast fail.
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("plaintext history-vault"),
        );
        save_config(&paths, &plaintext_config()).expect("seed plaintext config");
        {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(
                "rekey.after_swap_before_config",
            );
            rekey_archive(&paths, &plaintext_config(), None, ArchiveMode::Encrypted, Some(KEY))
                .expect_err("the injected crash must abort the rekey");
        }
        assert!(rekey_journal_present(&paths));

        // Hold the top-level guards on the main thread (gate FIRST, then the flock).
        let _gate = ArchiveOpGate::acquire(&paths);
        let _lock = ArchiveWriteLock::acquire(&paths).expect("top-level write lock");

        let worker_paths = paths.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let recovered = crate::archive::recover_interrupted_rekey(&worker_paths)
                .map_err(|error| format!("{error:#}"));
            done_tx.send(recovered).expect("worker signals completion");
        });

        let recovered = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the nested recover_interrupted_rekey must not re-take the non-reentrant gate")
            .expect("recovery under a held gate+lock must still succeed");
        assert!(recovered, "the interrupted rekey must be recovered");
        worker.join().expect("worker thread");
        assert!(!rekey_journal_present(&paths), "the marker must be cleared after recovery");
    }

    // --- Phase C FIX 1: launch must not freeze on the healthy path -----------------------------

    #[cfg(unix)]
    #[test]
    fn launch_recovery_does_not_block_on_a_foreign_lock_when_healthy() {
        // FIX 1 (launch must never freeze): a HEALTHY launch takes NO lock, so it returns promptly
        // EVEN while another OS process holds the cross-process write lock (a minutes-long
        // scheduled backup). A consistent ENCRYPTED archive also exercises the fast path's
        // Encrypted-matches branch. A watchdog turns a regression that re-introduced the blocking
        // acquire into a fast failure instead of a hung suite.
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted history-vault"),
        );
        save_config(&paths, &encrypted_config()).expect("consistent encrypted config");

        // A SEPARATE OS process holds the write lock for the whole test.
        let _foreign =
            crate::archive::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

        let worker_paths = paths.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let outcome = recover_archive_on_launch(&worker_paths, &encrypted_config(), None)
                .map(|outcome| matches!(outcome, LaunchRecovery::Healthy))
                .map_err(|error| format!("{error:#}"));
            done_tx.send(outcome).expect("worker signals completion");
        });

        let healthy = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a HEALTHY launch must NOT block on a foreign write lock (FIX 1 regression)")
            .expect("launch recovery must not error");
        assert!(healthy, "a consistent archive must be Healthy without taking the lock");
        worker.join().expect("worker thread");
    }

    #[cfg(unix)]
    #[test]
    fn launch_recovery_takes_the_lock_when_drift_is_present() {
        // FIX 1 (other half): when config↔file at-rest drift IS present the launch MUST serialize
        // against concurrent writers — so with a foreign process holding the write lock it BLOCKS
        // until released, then heals. Proves the non-healthy path actually acquires the lock (vs.
        // the healthy fast path, which takes none). While `foreign` lives the worker can NEVER win
        // the lock, so the short non-blocking probe deterministically times out (still parked);
        // releasing the lock lets it heal.
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Encrypted on disk, stale Plaintext config = a real config↔file drift.
        drop(
            open_archive_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted history-vault"),
        );
        save_config(&paths, &plaintext_config()).expect("stale plaintext config");

        let foreign =
            crate::archive::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

        let worker_paths = paths.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let outcome = recover_archive_on_launch(&worker_paths, &plaintext_config(), None)
                .map(|outcome| matches!(outcome, LaunchRecovery::Healed { .. }))
                .map_err(|error| format!("{error:#}"));
            done_tx.send(outcome).expect("worker signals completion");
        });

        // The foreign lock is held, so the drift recovery is parked acquiring it: it CANNOT have
        // completed yet (deterministic — the worker can never win the lock while `foreign` lives).
        assert!(
            matches!(
                done_rx.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "drift recovery must BLOCK on the foreign write lock, proving it takes the lock",
        );

        // Release the foreign lock; the recovery now acquires it and heals.
        drop(foreign);
        let healed = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("recovery must complete once the foreign lock releases")
            .expect("launch recovery must not error");
        assert!(healed, "the drift must heal to Healed once the lock is acquired");
        worker.join().expect("worker thread");
        assert!(matches!(load_config(&paths).expect("load").archive_mode, ArchiveMode::Encrypted));
    }

    #[test]
    fn launch_recovery_is_healthy_after_recovering_a_consistent_interrupted_import() {
        // After FIX 1 the cheap fast path short-circuits a provably-healthy launch, so the locked
        // body's bottom `Healthy` branch is reached only when a marker is present yet recovery
        // lands a state already consistent with the passed config. An interrupted import with
        // `had_previous: true` (rollback leaves the existing DB untouched) + a Plaintext
        // `previous_config` over a consistent Plaintext archive does exactly that: the marker takes
        // the lock, recovery restores the Plaintext config, step 3 finds no drift, and the passed
        // Plaintext config already matches the file -> Healthy, marker consumed.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &plaintext_config(), None)
                .expect("plaintext history-vault"),
        );
        save_config(&paths, &plaintext_config()).expect("plaintext config");

        let marker =
            paths.archive_database_path.parent().expect("parent").join(".pk-import-journal.json");
        let previous_config =
            serde_json::to_string(&plaintext_config()).expect("serialize previous config");
        let journal = serde_json::json!({
            "version": 1,
            "timestamp": "2026-06-30T03-00-00Z",
            "canonical": [
                { "target": paths.archive_database_path.to_string_lossy(), "had_previous": true },
            ],
            "subtrees": [],
            "previous_config": previous_config,
        });
        fs::write(&marker, serde_json::to_vec(&journal).expect("serialize journal"))
            .expect("seed import marker");
        assert!(import_journal_present(&paths));

        let outcome =
            recover_archive_on_launch(&paths, &plaintext_config(), None).expect("launch recovery");
        assert!(matches!(outcome, LaunchRecovery::Healthy), "got {outcome:?}");
        assert!(
            !import_journal_present(&paths),
            "the consumed marker recovers to a consistent archive",
        );
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Plaintext
        );
        assert!(matches!(load_config(&paths).expect("load").archive_mode, ArchiveMode::Plaintext));
    }

    // --- Phase C FIX 2: fail-closed Encrypted->Plaintext at launch -----------------------------

    #[test]
    fn launch_recovery_is_unrecoverable_for_an_interrupted_decrypt_leaving_encrypted_source_evidence()
     {
        // FIX 2 (fail-closed decrypt direction) at the launch entry: an Encrypted->Plaintext rekey
        // crashed AFTER the history-vault swap (now Plaintext) but BEFORE source-evidence was
        // decrypted (still Encrypted) and BEFORE config committed. Silently completing it would
        // write a Plaintext config + CLEAR the marker while source-evidence stays Encrypted
        // (decryptable only with the now-unprompted old key) -> a drift that surfaces only as
        // repeated backup failures. Launch recovery must FAIL CLOSED: Unrecoverable, marker LEFT,
        // config unchanged.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted history-vault"),
        );
        drop(
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("encrypted source-evidence"),
        );
        save_config(&paths, &encrypted_config()).expect("encrypted config");

        {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(
                "rekey.after_swap_before_config",
            );
            rekey_archive(&paths, &encrypted_config(), Some(KEY), ArchiveMode::Plaintext, None)
                .expect_err("the injected crash must abort the decrypt rekey");
        }
        // Post-crash on disk: history Plaintext (swapped), source-evidence STILL Encrypted, config
        // Encrypted (not committed), marker present.
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Plaintext
        );
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted
        );
        assert!(rekey_journal_present(&paths), "the crashed rekey must leave its marker");
        let config_before = fs::read(&paths.config_path).expect("read config before");

        let outcome = recover_archive_on_launch(&paths, &load_config(&paths).expect("load"), None)
            .expect("recovery returns Ok(Unrecoverable), never Err");
        let report = match outcome {
            LaunchRecovery::Unrecoverable(report) => report,
            other => panic!("expected Unrecoverable, got {other:?}"),
        };
        assert!(
            matches!(report.kind, ArchiveRecoveryKind::InterruptedRekeyUnresolved),
            "got {:?}",
            report.kind,
        );
        assert!(
            report.detail.contains("decrypt direction"),
            "the report must explain the decrypt-direction bail: {}",
            report.detail,
        );

        assert!(
            rekey_journal_present(&paths),
            "the marker must be LEFT so the state stays flagged"
        );
        assert_eq!(
            fs::read(&paths.config_path).expect("read config after"),
            config_before,
            "config must NOT be cleared/healed on the fail-closed bail",
        );
    }

    // --- D2: list_recovery_snapshots rich metadata ----------------------------------------------

    /// Seeds a tiny but REAL plaintext SQLite database at `path` (so the keyless openability probe
    /// passes), creating parent dirs as needed.
    fn seed_plaintext_snapshot(path: &Path) {
        fs::create_dir_all(path.parent().expect("snapshot parent")).expect("snapshot dir");
        let connection = Connection::open(path).expect("open snapshot db");
        connection
            .execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES (1);")
            .expect("seed snapshot db");
    }

    /// Pins `path`'s mtime to a fixed instant so newest-first ordering is deterministic.
    fn set_snapshot_mtime(path: &Path, secs: u64) {
        let file = fs::File::options().write(true).open(path).expect("open for mtime");
        file.set_modified(std::time::SystemTime::UNIX_EPOCH + StdDuration::from_secs(secs))
            .expect("set mtime");
    }

    #[test]
    fn list_recovery_snapshots_reports_validity_and_newest_first() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");

        // A valid plaintext snapshot (real db) and a corrupt one (SQLite header + garbage body so it
        // is detected Plaintext but `PRAGMA schema_version` errors).
        let valid = rekey_dir.join("archive-before-rekey-valid.sqlite");
        seed_plaintext_snapshot(&valid);
        let corrupt = rekey_dir.join("archive-before-rekey-corrupt.sqlite");
        fs::write(&corrupt, b"SQLite format 3\0\x01\x02\x03corrupt-body").expect("write corrupt");

        // Pin mtimes so newest-first ordering is deterministic: the valid one is newer.
        set_snapshot_mtime(&corrupt, 1_000_000_000);
        set_snapshot_mtime(&valid, 2_000_000_000);

        let snapshots = list_recovery_snapshots(&paths);
        assert_eq!(snapshots.len(), 2, "both snapshots are listed");
        assert_eq!(snapshots[0].path, valid.display().to_string(), "newest (valid) leads");
        assert_eq!(snapshots[1].path, corrupt.display().to_string());
        assert!(snapshots[0].verified_openable, "a real plaintext db opens");
        assert!(!snapshots[1].verified_openable, "a corrupt plaintext file does not open");
        for snapshot in &snapshots {
            assert_eq!(snapshot.source_op, "rekey");
            assert_eq!(snapshot.id, snapshot.path, "id == path");
            assert!(snapshot.size_bytes > 0, "size is read from metadata");
            assert!(snapshot.created_at.is_some(), "created_at comes from mtime");
            assert!(snapshot.label.contains("rekey"), "label encodes the op");
        }
    }

    #[test]
    fn list_recovery_snapshots_classifies_encrypted_unknown_and_skips_noise() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        // Encrypted-looking (non-SQLite header) >= 512 bytes under import/ -> structurally openable.
        let import_dir = paths.raw_snapshots_dir.join("import");
        fs::create_dir_all(&import_dir).expect("import dir");
        fs::write(import_dir.join("archive-before-import-big.sqlite"), vec![0xAB; 1024])
            .expect("write encrypted snapshot");

        // A tiny (< 512B) encrypted-looking file under rekey/ -> structurally NOT openable.
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        fs::write(rekey_dir.join("archive-before-rekey-tiny.sqlite"), vec![0xCD; 100])
            .expect("write tiny snapshot");

        // A snapshot under an UNKNOWN bucket -> source_op "unknown".
        seed_plaintext_snapshot(&paths.raw_snapshots_dir.join("mystery").join("whatever.sqlite"));

        // Noise that MUST be ignored: a non-.sqlite file in a bucket, and a plain file directly
        // under raw-snapshots/ (not a bucket directory).
        fs::write(import_dir.join("notes.txt"), b"ignore me").expect("write non-sqlite");
        fs::write(paths.raw_snapshots_dir.join("loose.sqlite"), b"loose")
            .expect("write loose file");

        let snapshots = list_recovery_snapshots(&paths);
        assert_eq!(snapshots.len(), 3, "only the three bucketed .sqlite files are listed");
        let find = |needle: &str| {
            snapshots.iter().find(|s| s.path.contains(needle)).expect("snapshot present").clone()
        };
        let big = find("big.sqlite");
        assert_eq!(big.source_op, "import");
        assert!(big.verified_openable, ">= 512B encrypted is structurally openable");
        assert!(big.encrypted, "a 0xAB header is a SQLCipher salt, not the SQLite magic");
        assert!(!find("tiny.sqlite").verified_openable, "< 512B encrypted is not openable");
        assert_eq!(find("whatever.sqlite").source_op, "unknown");
        assert!(
            !find("whatever.sqlite").encrypted,
            "a seeded plaintext snapshot is not flagged encrypted",
        );
    }

    #[test]
    fn list_recovery_snapshots_empty_for_missing_and_empty_dirs() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        assert!(list_recovery_snapshots(&paths).is_empty(), "missing dir -> empty");
        fs::create_dir_all(&paths.raw_snapshots_dir).expect("raw snapshots dir");
        assert!(list_recovery_snapshots(&paths).is_empty(), "empty dir -> empty");
    }

    #[test]
    fn list_recovery_snapshots_marks_a_sub_header_sized_file_unopenable() {
        // A `.sqlite` smaller than the 16-byte header makes `detect_disk_encryption_mode` read fewer
        // than 16 bytes and classify it `Absent`, driving the `Absent` arm of
        // `snapshot_is_openable_keyless` (verified_openable == false) — a truncated/empty snapshot
        // file is never a usable backstop.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        fs::write(rekey_dir.join("tiny.sqlite"), b"abcde").expect("write 5-byte snapshot");

        let snapshots = list_recovery_snapshots(&paths);
        assert_eq!(snapshots.len(), 1, "the sub-header file is still listed");
        assert!(
            !snapshots[0].verified_openable,
            "a <16-byte (Absent) file is never verified_openable",
        );
    }

    #[cfg(unix)]
    #[test]
    fn list_recovery_snapshots_skips_a_dangling_symlink() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        fs::create_dir_all(&rekey_dir).expect("rekey dir");
        // A .sqlite symlink whose target does not exist: it passes the extension filter, then
        // `fs::metadata` (which follows links) errors, so the entry is skipped.
        std::os::unix::fs::symlink(
            rekey_dir.join("missing-target.sqlite"),
            rekey_dir.join("dangling.sqlite"),
        )
        .expect("create dangling symlink");
        assert!(
            list_recovery_snapshots(&paths).is_empty(),
            "a dangling snapshot symlink contributes nothing",
        );
    }

    // --- D3: verified safety snapshot captured before a reconcile rewrite ------------------------

    #[test]
    fn reconcile_captures_a_verified_snapshot_only_when_a_rewrite_is_pending() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Drift that FORCES a rewrite: an encrypted archive + config over a PLAINTEXT source-evidence.
        drop(open_archive_connection(&paths, &encrypted_config(), Some(KEY)).expect("enc archive"));
        seed_plaintext_source_evidence(&paths);

        let report = reconcile_archive_encryption(&paths, &encrypted_config(), Some(KEY))
            .expect("reconcile");
        assert!(report.repaired, "the drift forces a source-evidence rewrite");

        let captured = list_recovery_snapshots(&paths)
            .into_iter()
            .find(|snapshot| snapshot.source_op == "reconcile")
            .expect("a reconcile safety snapshot must be captured before the rewrite");
        assert!(captured.verified_openable, "the captured backstop must be restorable");
    }

    #[test]
    fn reconcile_without_drift_captures_no_snapshot() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Already consistent: encrypted archive + encrypted source-evidence under an encrypted config.
        drop(open_archive_connection(&paths, &encrypted_config(), Some(KEY)).expect("enc archive"));
        drop(
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("enc source-evidence"),
        );

        let report = reconcile_archive_encryption(&paths, &encrypted_config(), Some(KEY))
            .expect("reconcile");
        assert!(!report.repaired, "no drift -> no rewrite");
        assert!(
            list_recovery_snapshots(&paths).iter().all(|s| s.source_op != "reconcile"),
            "a no-op reconcile must NOT capture a snapshot (perf: never copy the full DB on a hot path)",
        );
    }

    #[test]
    fn launch_is_provably_healthy_is_false_when_a_restore_marker_is_present() {
        // The Phase-D restore marker must force the locked recovery path so a crash mid-restore is
        // completed/rolled back — and so the `Absent => true` shortcut cannot boot an empty archive.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
            .expect("archive dir");

        // A plaintext canonical + a matching Plaintext config -> normally provably healthy.
        fs::write(&paths.archive_database_path, b"SQLite format 3\0plaintext body")
            .expect("write canonical");
        save_config(&paths, &plaintext_config()).expect("save config");
        assert!(
            launch_is_provably_healthy(&paths, &plaintext_config()),
            "a matching plaintext config + canonical is provably healthy without a marker",
        );

        // A restore marker beside the archive forces the locked recovery path.
        let marker = paths
            .archive_database_path
            .parent()
            .expect("archive parent")
            .join(".pk-restore-journal.json");
        fs::write(&marker, b"{}").expect("write restore marker");
        assert!(
            !launch_is_provably_healthy(&paths, &plaintext_config()),
            "a present restore marker makes the launch NOT provably healthy",
        );

        // Even with an ABSENT canonical, the marker must keep the Absent -> Healthy shortcut from firing.
        fs::remove_file(&paths.archive_database_path).expect("remove canonical");
        assert!(
            !launch_is_provably_healthy(&paths, &plaintext_config()),
            "an absent canonical + a restore marker must NOT shortcut to healthy",
        );
    }

    // --- E1/E3: config\u{2194}disk at-rest invariant + crash-window torture --------------------------
    //
    // These are the Phase-E answer to "WHY DIDN'T WE CATCH THIS?". `check_config_disk_consistency`
    // is the cross-FILE behavioral assertion the 100%-coverage gate never had: it reads the persisted
    // `config.json` through the REAL `load_config` and compares it to the canonical DBs' ACTUAL on-disk
    // at-rest mode. The incident-reproduction test proves it FAILS on the exact shipped shape; the
    // crash-window test proves it + launch recovery together catch "config lags the installed files";
    // and the exhaustive per-checkpoint torture test proves every rekey crash window recovers to a
    // consistent-or-fail-closed state, never a silent brick. All are deterministic (fixed checkpoint
    // enumeration, no wall-clock / RNG / thread-timing dependence).

    /// Seeds one uniquely-tagged canonical `runs` row so a crash-window test can prove the canonical
    /// facts survived the abort + recovery (the rekey export copies it forward, so it is present in
    /// BOTH the original and the rekeyed file).
    fn seed_canonical_marker(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) {
        let connection = open_archive_connection(paths, config, key).expect("open archive to seed");
        connection
            .execute(
                "INSERT INTO runs (run_type, trigger, started_at, status)
                 VALUES ('backup', 'e2e-marker', '2026-06-30T00:00:00Z', 'success')",
                [],
            )
            .expect("seed canonical marker row");
    }

    /// Counts the seeded marker rows, opening the archive in the given mode/key. Reopening from disk
    /// and counting is how these tests prove the canonical rows are intact + the archive is openable.
    fn canonical_marker_count(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) -> i64 {
        let connection =
            open_archive_connection(paths, config, key).expect("open archive to count");
        connection
            .query_row("SELECT COUNT(*) FROM runs WHERE trigger = 'e2e-marker'", [], |row| {
                row.get(0)
            })
            .expect("count marker rows")
    }

    #[test]
    fn config_disk_consistency_passes_on_consistent_plaintext_encrypted_and_uninitialized() {
        // Plaintext: history-vault + source-evidence both plaintext under a persisted Plaintext config.
        let plain_dir = tempdir().expect("tempdir");
        let plain = project_paths_with_root(plain_dir.path());
        drop(
            open_archive_connection(&plain, &plaintext_config(), None).expect("plaintext archive"),
        );
        seed_plaintext_source_evidence(&plain);
        save_config(&plain, &plaintext_config()).expect("save plaintext config");
        check_config_disk_consistency(&plain).expect("a consistent plaintext archive must pass");

        // Encrypted: both canonical DBs encrypted under a persisted Encrypted config.
        let enc_dir = tempdir().expect("tempdir");
        let enc = project_paths_with_root(enc_dir.path());
        drop(open_archive_connection(&enc, &encrypted_config(), Some(KEY)).expect("enc archive"));
        drop(
            open_source_evidence_connection(&enc, &encrypted_config(), Some(KEY)).expect("enc SE"),
        );
        save_config(&enc, &encrypted_config()).expect("save encrypted config");
        check_config_disk_consistency(&enc).expect("a consistent encrypted archive must pass");

        // Uninitialized: no config file (load_config defaults Plaintext) + both DBs absent -> the
        // Absent arm is skipped and the invariant holds vacuously.
        let empty_dir = tempdir().expect("tempdir");
        let empty = project_paths_with_root(empty_dir.path());
        check_config_disk_consistency(&empty).expect("an uninitialized layout must pass");
    }

    #[test]
    fn config_disk_consistency_fails_on_the_incident_shape_then_launch_recovery_heals_it() {
        // THE headline: both canonical DBs SQLCipher-encrypted on disk while the persisted config says
        // Plaintext and there is NO journal — the exact 2026-06-30 shape that shipped through a
        // 100%-green gate and bricked the next open with NOTADB. The checker MUST fail on it.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(open_archive_connection(&paths, &encrypted_config(), Some(KEY)).expect("enc archive"));
        drop(
            open_source_evidence_connection(&paths, &encrypted_config(), Some(KEY))
                .expect("enc SE"),
        );
        save_config(&paths, &plaintext_config()).expect("write the stale Plaintext config");

        let error = check_config_disk_consistency(&paths)
            .expect_err("encrypted files under a Plaintext config MUST fail the invariant");
        let rendered = format!("{error:#}");
        assert!(
            rendered.contains("invariant violated")
                && rendered.contains("history-vault")
                && rendered.contains("SQLITE_NOTADB"),
            "the failure must name the divergence + the incident, got: {rendered}",
        );

        // Launch recovery converges config to the files' real mode; the invariant then holds.
        let outcome =
            recover_archive_on_launch(&paths, &plaintext_config(), None).expect("launch recovery");
        assert!(matches!(outcome, LaunchRecovery::Healed { .. }), "got {outcome:?}");
        check_config_disk_consistency(&paths)
            .expect("after the launch heal the config\u{2194}disk invariant must hold");
    }

    #[test]
    fn config_disk_consistency_catches_config_lagging_installed_files_in_the_rekey_crash_window() {
        // THE proof that the checker catches the CLASS: reproduce, on disk, the exact intermediate
        // state a rekey crash leaves (the file swapped to Encrypted, config still Plaintext because it
        // is written LAST). This is also precisely what the PRE-HARDENING ordering — which wrote config
        // BEFORE / independently of the file swap and had no launch heal — would ship PERMANENTLY. The
        // checker MUST trip on that "config lags the installed files" divergence, and launch recovery
        // MUST restore the invariant. Together they make this class un-shippable-green.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        drop(
            open_archive_connection(&paths, &plaintext_config(), None).expect("plaintext archive"),
        );
        save_config(&paths, &plaintext_config()).expect("seed plaintext config");
        seed_canonical_marker(&paths, &plaintext_config(), None);

        {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(
                "rekey.after_swap_before_config",
            );
            let error =
                rekey_archive(&paths, &plaintext_config(), None, ArchiveMode::Encrypted, Some(KEY))
                    .expect_err("the injected crash must abort the rekey");
            assert!(
                format!("{error:#}").contains("rekey.after_swap_before_config"),
                "the INJECTED fault must propagate, got: {error:#}",
            );
        }
        // On disk now: history-vault Encrypted, config still Plaintext -> the divergence the checker
        // exists to catch. This assertion FAILS (checker returns Ok) if the file-swap-then-config-last
        // ordering were reverted to committing config independently of the installed files.
        assert_eq!(
            detect_disk_encryption_mode(&paths.archive_database_path),
            DiskEncryptionMode::Encrypted
        );
        let lag = check_config_disk_consistency(&paths)
            .expect_err("config lagging the installed files MUST trip the invariant");
        assert!(format!("{lag:#}").contains("invariant violated"), "got: {lag:#}");

        // Launch recovery heals the lag; the invariant holds and the canonical rows survive.
        recover_archive_on_launch(&paths, &load_config(&paths).expect("load"), None)
            .expect("launch recovery");
        check_config_disk_consistency(&paths).expect("recovery must restore the invariant");
        assert_eq!(
            canonical_marker_count(&paths, &encrypted_config(), Some(KEY)),
            1,
            "the canonical rows must survive the crash + recovery",
        );
    }

    #[test]
    fn rekey_crash_at_every_checkpoint_recovers_to_a_consistent_openable_archive() {
        // E3 torture (deterministic, exhaustive): a crash at EACH named rekey checkpoint, followed by
        // launch recovery, must land a consistent + openable archive whose config matches the files on
        // disk and whose canonical rows survive — never an empty/mixed/bricked state. Enumerating the
        // fixed checkpoint list (no RNG, no timing) keeps it reproducible and fast.
        for checkpoint in [
            "rekey.after_export_before_swap",
            "rekey.after_swap_before_config",
            "rekey.after_config",
        ] {
            let dir = tempdir().expect("tempdir");
            let paths = project_paths_with_root(dir.path());
            drop(
                open_archive_connection(&paths, &plaintext_config(), None)
                    .expect("plaintext archive"),
            );
            save_config(&paths, &plaintext_config()).expect("seed plaintext config");
            seed_canonical_marker(&paths, &plaintext_config(), None);

            {
                let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(checkpoint);
                let error = rekey_archive(
                    &paths,
                    &plaintext_config(),
                    None,
                    ArchiveMode::Encrypted,
                    Some(KEY),
                )
                .expect_err(&format!("the injected crash at {checkpoint} must abort rekey"));
                assert!(
                    format!("{error:#}").contains(checkpoint),
                    "the INJECTED fault must propagate at {checkpoint}, got: {error:#}",
                );
            }

            let outcome =
                recover_archive_on_launch(&paths, &load_config(&paths).expect("load"), None)
                    .unwrap_or_else(|error| {
                        panic!("launch recovery at {checkpoint} errored: {error:#}")
                    });
            // A plaintext->encrypted rekey self-heals in every window (encrypt direction), so no
            // window is fail-closed here; the recovery must reach a healed/healthy consistent state.
            assert!(
                !matches!(outcome, LaunchRecovery::Unrecoverable(_)),
                "the encrypt-direction crash at {checkpoint} must self-heal, got {outcome:?}",
            );
            check_config_disk_consistency(&paths).unwrap_or_else(|error| {
                panic!(
                    "the invariant must hold after recovering a crash at {checkpoint}: {error:#}"
                )
            });

            // Open in the RECOVERED mode and prove the canonical rows are intact + the archive opens.
            let recovered = load_config(&paths).expect("load recovered config");
            let key = match recovered.archive_mode {
                ArchiveMode::Encrypted => Some(KEY),
                ArchiveMode::Plaintext => None,
            };
            assert_eq!(
                canonical_marker_count(&paths, &recovered, key),
                1,
                "the canonical rows must survive a crash at {checkpoint} + recovery",
            );
        }
    }

    #[test]
    fn two_same_process_top_level_reconciles_serialize_and_leave_a_consistent_archive() {
        // E3 concurrency: two TOP-LEVEL destructive ops dispatched in ONE process against the SAME
        // archive must serialize on the in-process `ArchiveOpGate` (the reentrant flock alone cannot
        // exclude them) and leave a single consistent archive — never a half-converted source-evidence
        // or a mode-drifted config. Two concurrent `reconcile_archive_encryption` calls over a seeded
        // drift: the first repairs it, the second is a no-op; both must complete (watchdog guards a
        // serialization regression that would deadlock/hang) and the invariant must hold afterwards.
        use std::sync::mpsc;
        use std::time::Duration;

        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        // Drift that forces a rewrite: encrypted archive + config, PLAINTEXT source-evidence.
        drop(open_archive_connection(&paths, &encrypted_config(), Some(KEY)).expect("enc archive"));
        seed_plaintext_source_evidence(&paths);
        save_config(&paths, &encrypted_config()).expect("save encrypted config");

        let (done_tx, done_rx) = mpsc::channel();
        let mut workers = Vec::new();
        for _ in 0..2 {
            let worker_paths = paths.clone();
            let worker_config = encrypted_config();
            let worker_done = done_tx.clone();
            workers.push(std::thread::spawn(move || {
                let outcome =
                    reconcile_archive_encryption(&worker_paths, &worker_config, Some(KEY))
                        .map(|report| report.repaired)
                        .map_err(|error| format!("{error:#}"));
                worker_done.send(outcome).expect("worker signals completion");
            }));
        }
        drop(done_tx);

        // Both must finish (a serialization/deadlock regression trips the watchdog), collecting each
        // reconcile's `repaired` flag.
        let mut repaired_flags = Vec::new();
        for _ in 0..2 {
            let repaired = done_rx
                .recv_timeout(Duration::from_secs(20))
                .expect("both same-process reconciles must complete (op-gate serialization)")
                .expect("neither concurrent reconcile may error");
            repaired_flags.push(repaired);
        }
        for worker in workers {
            worker.join().expect("worker thread");
        }

        // EXACTLY-ONCE repair: because the gate fully serializes them, the winner repairs the drift
        // (repaired == true) and the loser then re-detects a now-consistent archive (repaired ==
        // false) — never both repairing (a double-rewrite) or both skipping (a lost repair).
        assert_eq!(
            repaired_flags.iter().filter(|&&repaired| repaired).count(),
            1,
            "exactly one serialized reconcile repairs the drift; the other is a no-op, got: {repaired_flags:?}",
        );

        // A consistent end state: source-evidence is encrypted and the config\u{2194}disk invariant holds.
        assert_eq!(
            detect_disk_encryption_mode(&paths.source_evidence_database_path),
            DiskEncryptionMode::Encrypted,
            "the drifted source-evidence must end encrypted, matching the archive + config",
        );
        check_config_disk_consistency(&paths)
            .expect("two serialized reconciles must leave a consistent archive");
    }
}
