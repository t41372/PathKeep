//! Responsibilities: bootstrap, conversion assessment and native rekey orchestration.
//! Not responsible for: storage engines, file installation or frontend rendering.
//! Failure modes: wrong-password mutation, stale session after a swap, config-only mode changes,
//! or hidden conversion progress. Tests: worker bootstrap, password and keychain acceptance.
//!
//! App/bootstrap worker flows.
//!
//! These entrypoints assemble the desktop snapshot that the Tauri layer and
//! CLI-facing worker commands consume. They coordinate config hydration,
//! browser discovery, archive status reads, and bootstrap-time mutations such
//! as archive initialization or archive-mode changes.
//!
//! This module deliberately keeps the public worker surface stable. It may
//! compose several `vault-core` and `vault-platform` calls, but it should not
//! invent new product semantics beyond what the accepted command/read-model
//! docs already describe.

use crate::context::{
    current_app_lock_biometric_state, degrade_ai_status, degrade_intelligence_status,
    hydrate_derived_config_state, load_hydrated_config, load_unlocked_config,
    resolved_app_lock_status,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vault_core::{
    ARCHIVE_RECOVERY_REQUIRED_PREFIX, AppConfig, AppSnapshot, ArchiveMode, ArchiveRecoveryReport,
    ArchiveUpgradeAssessment, ArchiveUpgradeProgress, DISCOVERY_ISSUE_DISCOVERY_ERROR,
    DISCOVERY_ISSUE_FULL_DISK_ACCESS, LaunchRecovery, ReconcileReport, RuntimeDiagnostics,
    ensure_app_lock_unlocked, full_disk_access_applies, hydrate_app_lock_config,
    is_permission_denied, load_config, load_snapshot_reads, recover_archive_on_launch, save_config,
    validate_app_lock_change,
};
use vault_platform::{
    FullDiskAccessProbe, discover_browser_profiles, keyring_status, probe_full_disk_access,
};

/// Rekey request payload used by desktop and worker command surfaces.
///
/// The worker keeps this as a simple transport struct: it describes the target
/// archive mode plus the optional new key material, while the actual
/// preview/execute semantics stay in `vault-core`.
///
/// `Debug` is written by hand so the keys never reach a log line.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RekeyRequest {
    /// The archive mode to end up with after the rewrite finishes.
    pub new_mode: ArchiveMode,
    /// The new database key when the target mode is encrypted.
    pub new_key: Option<String>,
    /// The archive's current password. Required whenever the archive is
    /// encrypted (changing the password or decrypting), so that someone at an
    /// unlocked window cannot take the archive over; ignored for a plaintext
    /// archive, which has none. See [`crate::current_password`].
    #[serde(default)]
    pub current_key: Option<String>,
}

impl std::fmt::Debug for RekeyRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redacted = |key: &Option<String>| key.as_ref().map(|_| "<redacted>");
        formatter
            .debug_struct("RekeyRequest")
            .field("new_mode", &self.new_mode)
            .field("new_key", &redacted(&self.new_key))
            .field("current_key", &redacted(&self.current_key))
            .finish()
    }
}

fn snapshot_runtime_diagnostics(paths: &vault_core::ProjectPaths) -> RuntimeDiagnostics {
    match vault_core::load_runtime_diagnostics(paths) {
        Ok(diagnostics) => diagnostics,
        Err(error) => runtime_diagnostics_fallback(paths, &error),
    }
}

fn runtime_diagnostics_fallback(
    paths: &vault_core::ProjectPaths,
    error: &anyhow::Error,
) -> RuntimeDiagnostics {
    log::warn!(target: "pathkeep::app_snapshot", "runtime diagnostics fallback during shell bootstrap: {error:#}");
    RuntimeDiagnostics {
        log_directory: paths.logs_dir.display().to_string(),
        rust_log_path: paths.rust_log_path.display().to_string(),
        frontend_log_path: paths.frontend_log_path.display().to_string(),
        crash_reports_directory: paths.crash_reports_dir.display().to_string(),
        latest_crash_report: None,
    }
}

/// Browser discovery plus the reason the list is what it is.
///
/// Bundling the two lets [`app_snapshot`] tell a genuine "no browsers installed" apart
/// from a permission wall: the profile list ALONE cannot, because a Full Disk Access
/// denial yields the SAME empty list a fresh machine does. `issue` carries the stable
/// marker the onboarding UI keys on (see [`resolve_browser_discovery_issue_core`]).
struct BrowserDiscovery {
    profiles: Vec<vault_core::BrowserProfile>,
    issue: Option<String>,
}

/// Discovers browser profiles and classifies WHY the result is what it is, combining the
/// discovery outcome with an INDEPENDENT Full Disk Access probe.
///
/// The probe matters even when discovery does not error: on some macOS setups a TCC wall
/// makes discovery silently skip a browser (its path "does not exist") instead of
/// surfacing an error, so an empty list with no error can still be a permission problem.
fn snapshot_browser_profiles() -> BrowserDiscovery {
    let probe = probe_full_disk_access();
    match discover_browser_profiles() {
        Ok(profiles) => {
            let issue = resolve_browser_discovery_issue(Ok(profiles.len()), false, probe);
            BrowserDiscovery { profiles, issue }
        }
        Err(error) => browser_profiles_fallback(&error, probe),
    }
}

/// Records a discovery failure and returns an empty list carrying the CLASSIFIED reason,
/// so an onboarding user sees WHY (a Full Disk Access wall, or a surfaced error) instead
/// of a silent "found 0 browsers". `vault-core` stays log-free, so the warning is emitted
/// here as before — but the signal is no longer swallowed.
fn browser_profiles_fallback(
    error: &anyhow::Error,
    probe: FullDiskAccessProbe,
) -> BrowserDiscovery {
    log::warn!(target: "pathkeep::app_snapshot", "browser discovery fallback during shell bootstrap: {error:#}");
    let issue = resolve_browser_discovery_issue(Err(()), is_permission_denied(error), probe);
    BrowserDiscovery { profiles: Vec::new(), issue }
}

/// Thin host wrapper over [`resolve_browser_discovery_issue_core`] that supplies the real
/// OS facts: whether Full Disk Access applies on this host, and whether the probe
/// concluded `Denied`.
fn resolve_browser_discovery_issue(
    discovery: Result<usize, ()>,
    error_is_permission_denied: bool,
    probe: FullDiskAccessProbe,
) -> Option<String> {
    resolve_browser_discovery_issue_core(
        discovery,
        error_is_permission_denied,
        full_disk_access_applies(),
        probe == FullDiskAccessProbe::Denied,
    )
}

/// Decides the snapshot's `browser_discovery_issue` marker from a discovery outcome plus
/// an INDEPENDENT Full Disk Access probe. Every OS-specific fact is injected so the
/// precedence is fully unit-testable on any host without real TCC.
///
/// Precedence (the exact contract the front end consumes):
/// 1. Discovery ERRORED and the error is a permission denial on a Full-Disk-Access host
///    (macOS), OR the probe independently reports `Denied` ⇒ `Some("macos-full-disk-access")`.
/// 2. Discovery ERRORED for any other reason ⇒ `Some("discovery-error")` (surfaced, never
///    hidden as "no browsers").
/// 3. Discovery SUCCEEDED but returned ZERO profiles AND the probe reports `Denied` ⇒
///    `Some("macos-full-disk-access")` — the headline silent-skip case, where a TCC wall
///    made discovery find nothing without erroring.
/// 4. Otherwise ⇒ `None`. A populated list is neutral, and a genuinely empty list with
///    Full Disk Access granted / inconclusive / not-applicable stays `None`, so a fresh
///    install is never mistaken for a permission wall (no false nag).
fn resolve_browser_discovery_issue_core(
    discovery: Result<usize, ()>,
    error_is_permission_denied: bool,
    fda_applies: bool,
    probe_denied: bool,
) -> Option<String> {
    match discovery {
        Err(()) => {
            if (error_is_permission_denied && fda_applies) || probe_denied {
                Some(DISCOVERY_ISSUE_FULL_DISK_ACCESS.to_string())
            } else {
                Some(DISCOVERY_ISSUE_DISCOVERY_ERROR.to_string())
            }
        }
        Ok(profile_count) => (profile_count == 0 && probe_denied)
            .then(|| DISCOVERY_ISSUE_FULL_DISK_ACCESS.to_string()),
    }
}

/// Builds the canonical desktop snapshot for the current unlocked session.
pub fn app_snapshot(session_database_key: Option<&str>) -> Result<AppSnapshot> {
    app_snapshot_with_vault(None, session_database_key)
}

/// Uses the session owner for canonical reads, including keychain-only unlocks.
pub fn app_snapshot_with_vault(
    vault: Option<&vault_core::Vault>,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    // The session's vault keeps the settings it was opened with. Its reads must describe the saved
    // ones, or a search index built with a newly chosen embeddings service reads as empty.
    let vault = vault.map(|vault| vault.with_config(config.clone()));
    let browser_discovery = snapshot_browser_profiles();
    // One archive open and one intelligence open for everything below; each open of an encrypted
    // archive derives its key again.
    let reads = load_snapshot_reads(&paths, &config, session_database_key, vault.as_ref())?;
    let ai_status = degrade_ai_status(&config, reads.ai_status);
    let intelligence_status = degrade_intelligence_status(reads.intelligence_status);
    let app_lock_status = resolved_app_lock_status(&paths, &config)?;
    let runtime_diagnostics = snapshot_runtime_diagnostics(&paths);

    Ok(AppSnapshot {
        directories: vault_core::AppDirectories {
            app_root: paths.app_root.display().to_string(),
            config_path: paths.config_path.display().to_string(),
            archive_database_path: reads.archive_status.database_path.clone(),
            search_database_path: paths.search_database_path.display().to_string(),
            intelligence_database_path: paths.intelligence_database_path.display().to_string(),
            audit_repo_path: paths.audit_repo_path.display().to_string(),
            manifests_dir: paths.manifests_dir.display().to_string(),
            exports_dir: paths.exports_dir.display().to_string(),
            raw_snapshots_dir: paths.raw_snapshots_dir.display().to_string(),
            staging_dir: paths.staging_dir.display().to_string(),
            quarantine_dir: paths.quarantine_dir.display().to_string(),
            schedule_dir: paths.schedule_dir.display().to_string(),
            semantic_index_dir: paths.semantic_index_dir.display().to_string(),
            intelligence_blobs_dir: paths.intelligence_blobs_dir.display().to_string(),
            logs_dir: paths.logs_dir.display().to_string(),
            rust_log_path: paths.rust_log_path.display().to_string(),
            frontend_log_path: paths.frontend_log_path.display().to_string(),
            crash_reports_dir: paths.crash_reports_dir.display().to_string(),
            stronghold_path: paths.stronghold_path.display().to_string(),
            stronghold_salt_path: paths.stronghold_salt_path.display().to_string(),
        },
        runtime_diagnostics,
        config,
        archive_status: reads.archive_status,
        app_lock_status,
        keyring_status: keyring_status(),
        ai_status,
        intelligence_status,
        browser_profiles: browser_discovery.profiles,
        browser_discovery_issue: browser_discovery.issue,
        recent_runs: reads.recent_runs,
        recent_runs_next_cursor: reads.recent_runs_next_cursor,
        recent_import_batches: reads.recent_import_batches,
        recent_import_batches_next_cursor: reads.recent_import_batches_next_cursor,
    })
}

/// Saves config, reconciles runtime-derived controls, and returns a fresh snapshot.
pub fn save_user_config(
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    save_user_config_with_base(config, None, session_database_key)
}

/// Saves a user configuration while preserving concurrent edits made after the
/// caller's snapshot. Desktop Settings auto-saves independent controls, so a
/// full stale config must never erase a newer unrelated preference.
pub fn save_user_config_with_base(
    config: &AppConfig,
    base_config: Option<&AppConfig>,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    save_user_config_with_vault(None, config, base_config, session_database_key)
}

/// Reconciles database-backed controls through the caller's current owner.
pub fn save_user_config_with_vault(
    vault: Option<&vault_core::Vault>,
    config: &AppConfig,
    base_config: Option<&AppConfig>,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let previous_config = load_hydrated_config(&paths)
        .context("loading the current configuration before saving user changes")?;
    // Settings (including App Lock's own enabled/biometric fields) must not be
    // mutated while the session is locked: otherwise the lock could be disabled
    // out from under itself without the passcode. Enabling the lock from an
    // unlocked session, and all initial setup, still pass this no-op check.
    ensure_app_lock_unlocked(&paths, &previous_config)?;
    let mut next_config = match base_config {
        Some(base_config) => merge_config_change(&previous_config, base_config, config)?,
        None => config.clone(),
    };
    if previous_config.initialized && previous_config.archive_mode != next_config.archive_mode {
        anyhow::bail!(
            "encryption-mode-change-requires-rekey: use rekey_archive to change encryption"
        );
    }
    if let Some(zone) = next_config.insights_time_zone.as_deref()
        && !vault_core::is_insights_time_zone(zone)
    {
        anyhow::bail!("invalid-time-zone: {zone} is not an IANA time zone");
    }
    hydrate_derived_config_state(&mut next_config);
    hydrate_app_lock_config(&paths, &mut next_config)?;
    validate_app_lock_change(
        &paths,
        &previous_config,
        &next_config,
        current_app_lock_biometric_state(),
    )?;
    save_config(&paths, &next_config)?;
    reconcile_ai_queue_controls_or_restore_config(vault, &paths, &previous_config, &next_config)?;
    // A new Insights time zone rebuilds Insights in it now, not at the next backup. A build
    // running meanwhile is followed by one in the new zone when it ends (the intelligence drain).
    if previous_config.insights_time_zone != next_config.insights_time_zone
        && let Some(vault) = vault
    {
        crate::intelligence::rebuild_in_chosen_zone(&vault.with_config(next_config.clone()))
            .context("queueing an Insights build in the new time zone")?;
    }
    app_snapshot_with_vault(vault, session_database_key)
}

/// Applies only the leaves the caller changed relative to its base snapshot to
/// the current durable configuration. This is intentionally structural rather
/// than field-by-field: nested Settings domains evolve frequently, and an
/// omitted new field must not become a future data-loss footgun.
pub(crate) fn merge_config_change(
    current: &AppConfig,
    base: &AppConfig,
    proposed: &AppConfig,
) -> Result<AppConfig> {
    let mut current_value = serde_json::to_value(current).context("serializing current config")?;
    let base_value = serde_json::to_value(base).context("serializing base config")?;
    let proposed_value = serde_json::to_value(proposed).context("serializing proposed config")?;
    apply_config_delta(&mut current_value, &base_value, &proposed_value);
    serde_json::from_value(current_value).context("decoding merged config")
}

/// Recursively applies `proposed - base` onto `current`. Arrays are atomic
/// settings values; objects merge by leaf so a concurrent language change and
/// App Lock change cannot reset each other.
pub(crate) fn apply_config_delta(current: &mut Value, base: &Value, proposed: &Value) {
    match (current, base, proposed) {
        (Value::Object(current), Value::Object(base), Value::Object(proposed)) => {
            for (key, proposed_value) in proposed {
                match base.get(key) {
                    Some(base_value) => match current.get_mut(key) {
                        Some(current_value) => {
                            apply_config_delta(current_value, base_value, proposed_value)
                        }
                        None if proposed_value != base_value => {
                            current.insert(key.clone(), proposed_value.clone());
                        }
                        None => {}
                    },
                    None => {
                        current.insert(key.clone(), proposed_value.clone());
                    }
                }
            }
        }
        (current, base, proposed) if proposed != base => *current = proposed.clone(),
        _ => {}
    }
}

fn reconcile_ai_queue_controls_or_restore_config(
    vault: Option<&vault_core::Vault>,
    paths: &vault_core::ProjectPaths,
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> Result<()> {
    if let Some(vault) = vault {
        vault_core::reconcile_ai_queue_controls(vault, previous_config, next_config).or_else(
            |error| restore_config_after_ai_reconcile_failure(paths, previous_config, error),
        )?;
    }
    Ok(())
}

fn restore_config_after_ai_reconcile_failure(
    paths: &vault_core::ProjectPaths,
    previous_config: &AppConfig,
    error: anyhow::Error,
) -> Result<()> {
    save_config(paths, previous_config).with_context(
        || "restoring the previous config after AI queue control reconciliation failed",
    )?;
    Err(error.context("syncing AI queue controls with the updated Settings"))
}

/// Initializes the archive with the provided config and returns the hydrated snapshot.
/// For callers without a long-lived session: the archive is closed again afterwards.
pub fn initialize_archive_database(
    config: &AppConfig,
    database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let session = crate::VaultSession::default();
    let snapshot =
        initialize_archive_database_with_progress(&session, config, database_key, |_| {});
    session.close_vault().map_err(anyhow::Error::msg)?;
    snapshot
}

/// Conversion assessment for the upgrade screen. Counts run on the worker without
/// creating a native destination or consuming a pending upgrade. An archive in the
/// old format is first brought to a consistent state (see [`recover_unconverted`]):
/// this is the first thing the shell asks at launch.
pub fn assess_archive_upgrade(database_key: Option<&str>) -> Result<ArchiveUpgradeAssessment> {
    let paths = vault_core::project_paths()?;
    let mut config = load_config(&paths).unwrap_or_default();
    if !is_native(&paths) {
        recover_unconverted(&paths, &mut config)?;
    }
    let conversion = vault_convert_sqlcipher::assess(&paths, &config, database_key)?;
    let phases = if conversion.required {
        use vault_core::{ArchiveUpgradePhase as Phase, ArchiveUpgradePhaseAssessment};
        let rows = conversion.tables.iter().map(|table| table.rows).sum::<u64>();
        [
            (Phase::StorageConversion, rows),
            (Phase::ContentVerification, rows * 2),
            (Phase::SearchReprojection, 0),
        ]
        .into_iter()
        .map(|(phase, total)| ArchiveUpgradePhaseAssessment {
            phase,
            phase_label: phase.label_key().into(),
            pending: true,
            streamed: true,
            estimated_total: total,
        })
        .collect()
    } else {
        Vec::new()
    };
    Ok(ArchiveUpgradeAssessment {
        pending: conversion.required,
        current_schema_version: i64::from(paths.app_root.join("archive/history-vault.db").exists()),
        target_schema_version: 1,
        phases,
        conversion: Some(conversion),
    })
}

/// Converts an existing archive before the desktop publishes its native session.
pub fn convert_archive_with_progress(
    session: &crate::VaultSession,
    config: &AppConfig,
    password: Option<&str>,
    mut report: impl FnMut(ArchiveUpgradeProgress),
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = config.clone();
    recover_unconverted(&paths, &mut config)?;
    let config = &config;
    let installed =
        crate::maintenance_jobs::run(&paths.app_root, "converting", |cancel, mut sink| {
            vault_convert_sqlcipher::convert_owned(
                &paths,
                config,
                password,
                cancel,
                |p| {
                    let phase = match p.phase.as_str() {
                        "verifying" => vault_core::ArchiveUpgradePhase::ContentVerification,
                        "publishing" => vault_core::ArchiveUpgradePhase::Finalizing,
                        "indexing" => vault_core::ArchiveUpgradePhase::SearchReprojection,
                        _ => vault_core::ArchiveUpgradePhase::StorageConversion,
                    };
                    let mut event = ArchiveUpgradeProgress::phase(phase, p.processed, p.total);
                    event.database = p.database.clone();
                    event.table = p.table.clone();
                    report(event);
                    sink(p);
                },
                session.owner(),
            )
        })?;
    session.install_archive(installed, password.map(str::to_owned))?;
    report(ArchiveUpgradeProgress::finished());
    app_snapshot_with_vault(session.current_vault().as_ref(), None)
}

/// Saves the setup config, converting an archive still in the old format
/// first (streaming its progress), opens the archive through `session` (which
/// creates a new one natively) and returns the snapshot.
///
/// The observable twin of [`initialize_archive_database`], which delegates here
/// with a no-op callback.
pub fn initialize_archive_database_with_progress<F>(
    session: &crate::VaultSession,
    config: &AppConfig,
    database_key: Option<&str>,
    mut report_progress: F,
) -> Result<AppSnapshot>
where
    F: FnMut(ArchiveUpgradeProgress),
{
    // A wipe cut short must finish before this writes a new config, or onboarding would reopen
    // the half-deleted archive.
    crate::data_wipe::finish_interrupted_data_wipe()?;
    let paths = vault_core::project_paths()?;
    // Only to tell whether Touch ID is being turned on; a first run has no config yet.
    let previous_config = load_hydrated_config(&paths).unwrap_or_default();
    let mut next_config = config.clone();
    if previous_config.initialized && previous_config.archive_mode != next_config.archive_mode {
        anyhow::bail!(
            "encryption-mode-change-requires-rekey: use rekey_archive to change encryption"
        );
    }
    if let Some(zone) = next_config.insights_time_zone.as_deref()
        && !vault_core::is_insights_time_zone(zone)
    {
        anyhow::bail!("invalid-time-zone: {zone} is not an IANA time zone");
    }
    hydrate_derived_config_state(&mut next_config);
    hydrate_app_lock_config(&paths, &mut next_config)?;
    validate_app_lock_change(
        &paths,
        &previous_config,
        &next_config,
        current_app_lock_biometric_state(),
    )?;
    if is_native(&paths) {
        match recover_archive_on_launch(&paths, &next_config)? {
            LaunchRecovery::Healed { to_mode, .. } => next_config.archive_mode = to_mode,
            LaunchRecovery::Unrecoverable(report) => {
                return Err(archive_recovery_required_error(report));
            }
            LaunchRecovery::Healthy => {}
        }
    } else {
        recover_unconverted(&paths, &mut next_config)?;
    }
    if next_config.archive_mode == ArchiveMode::Encrypted
        && database_key.is_none()
        && (paths.archive_database_path.exists()
            || paths.app_root.join("archive/history-vault.db").exists())
    {
        return app_snapshot(None);
    }
    if paths.archive_database_path.exists()
        && !paths.app_root.join("archive/history-vault.db").exists()
    {
        let archive =
            crate::maintenance_jobs::run(&paths.app_root, "converting", |cancel, mut sink| {
                vault_convert_sqlcipher::convert(&paths, &next_config, database_key, cancel, |p| {
                    let phase = match p.phase.as_str() {
                        "verifying" => vault_core::ArchiveUpgradePhase::ContentVerification,
                        "publishing" => vault_core::ArchiveUpgradePhase::Finalizing,
                        "indexing" => vault_core::ArchiveUpgradePhase::SearchReprojection,
                        _ => vault_core::ArchiveUpgradePhase::StorageConversion,
                    };
                    let mut event = ArchiveUpgradeProgress::phase(phase, p.processed, p.total);
                    event.database = p.database.clone();
                    event.table = p.table.clone();
                    report_progress(event);
                    sink(p);
                })
            })?;
        let snapshot = app_snapshot_with_vault(Some(&archive.vault), None)?;
        archive.vault.close()?;
        report_progress(ArchiveUpgradeProgress::finished());
        return Ok(snapshot);
    }
    vault_core::config::ensure_paths(&paths)?;
    next_config.initialized = true;
    save_config(&paths, &next_config)?;
    // Opening the session's store creates the archive in the native format, with
    // its key envelope when encrypted. Nothing here may create the old SQLCipher
    // files: they would be taken for an archive to convert.
    session.update_password(database_key.map(str::to_owned)).map_err(anyhow::Error::msg)?;
    let vault = session.background_vault().map_err(anyhow::Error::msg)?;
    report_progress(ArchiveUpgradeProgress::finished());
    app_snapshot_with_vault(Some(&vault), database_key)
}

/// The native archive exists, or a native install is waiting to be finished.
fn is_native(paths: &vault_core::ProjectPaths) -> bool {
    paths.app_root.join("archive/history-vault.db").exists()
        || vault_core::maintenance::install::pending(paths)
}

/// Brings an archive of the old format to a consistent state before anything reads it: an
/// import, password change or restore the old app left half-done is finished or rolled back,
/// and config follows the file's real at-rest mode, as the old app's launch did. Runs on every
/// entry that can reach an unconverted archive (assess, convert, initialize), because the shell
/// asks for the assessment first. An archive it cannot bring back goes to the recovery screen.
fn recover_unconverted(paths: &vault_core::ProjectPaths, config: &mut AppConfig) -> Result<()> {
    match vault_convert_sqlcipher::recover_on_launch(paths, config)? {
        LaunchRecovery::Healed { to_mode, .. } => config.archive_mode = to_mode,
        LaunchRecovery::Unrecoverable(report) => {
            return Err(archive_recovery_required_error(report));
        }
        LaunchRecovery::Healthy => {}
    }
    Ok(())
}

/// Wraps an unrecoverable launch state in a STRUCTURED, prefix-tagged, JSON-carrying error so
/// Phase D can route it to the recovery screen rather than treat it as opaque. Mirrors the
/// `IMPORT_SOURCE_KEY_REQUIRED_PREFIX` convention: a stable prefix the FE matches, then the
/// serialized [`ArchiveRecoveryReport`].
fn archive_recovery_required_error(report: ArchiveRecoveryReport) -> anyhow::Error {
    let payload = serde_json::to_string(&report).unwrap_or_default();
    anyhow::anyhow!("{ARCHIVE_RECOVERY_REQUIRED_PREFIX}: {payload}")
}

/// Parses a launch-recovery-required error message back into its [`ArchiveRecoveryReport`].
///
/// The inverse of [`archive_recovery_required_error`]. It exists to DOCUMENT + TEST the exact wire
/// contract the FE recovery screen parses: the stable `ARCHIVE_RECOVERY_REQUIRED_PREFIX`, a `": "`
/// separator, then the JSON report (which now carries `recoverySnapshots`). Returns `None` for any
/// message that is not this shape, so a generic error handler can fall through unchanged.
pub fn parse_archive_recovery_required(message: &str) -> Option<ArchiveRecoveryReport> {
    let payload = message.strip_prefix(ARCHIVE_RECOVERY_REQUIRED_PREFIX)?.strip_prefix(": ")?;
    serde_json::from_str::<ArchiveRecoveryReport>(payload).ok()
}

/// Executes a rekey/mode-switch request and returns the post-rewrite snapshot.
///
/// The archive is opened with the request's verified current password, never
/// with a key the caller merely holds (see [`crate::current_password`]).
pub fn rekey_archive_database(
    session: &crate::VaultSession,
    request: &RekeyRequest,
) -> Result<AppSnapshot> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let current = crate::current_password::verified_current_key(&paths, &config, request)?;
    let vault = if let Some(vault) = session.current_vault() {
        vault.with_config(config)
    } else if let Some(password) = current {
        session.open_password(paths, config, password.into()).map_err(anyhow::Error::msg)?
    } else {
        session.vault().map_err(anyhow::Error::msg)?
    };
    let mut audit_warning = None;
    if request.new_mode == vault.config().archive_mode {
        if request.new_mode == ArchiveMode::Encrypted {
            let password =
                request.new_key.as_deref().context("new archive password is required")?;
            crate::maintenance_jobs::run(&vault.paths().app_root, "rewrapping", |cancel, _| {
                cancel.check()?;
                vault_core::maintenance::rewrap_password(&vault, current.unwrap(), password)
            })?;
            session.remember_password(password.into());
        }
    } else {
        let installed = crate::maintenance_jobs::run(
            &vault.paths().app_root,
            "copying",
            |cancel, progress| {
                vault_core::maintenance::switch_mode(
                    &vault,
                    current,
                    request.new_mode.clone(),
                    request.new_key.as_deref(),
                    cancel,
                    progress,
                )
            },
        )?;
        let snapshot_path = installed.previous_files.display().to_string();
        session.install_archive(installed, request.new_key.clone())?;
        if let Err(error) = vault_core::maintenance::record_mode_change(
            &session.vault().map_err(anyhow::Error::msg)?,
            &snapshot_path,
        ) {
            audit_warning = Some(format!(
                "Archive mode changed, but its review artifact could not be saved: {error}"
            ));
        }
    }
    let mut snapshot = app_snapshot_with_vault(session.current_vault().as_ref(), None)?;
    if audit_warning.is_some() {
        snapshot.archive_status.warning = audit_warning;
    }
    Ok(snapshot)
}

/// Repairs stale native encryption configuration after the owner authenticates
/// the database files. SQLCipher conversion has its own explicit upgrade flow.
pub fn reconcile_archive_encryption(vault: &vault_core::Vault) -> Result<ReconcileReport> {
    // Successful Store preflight already authenticated every database. Reconcile
    // configuration to the actual archive header without rewriting its key.
    vault_core::maintenance::reconcile(vault)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_surfaces_a_permission_denial_instead_of_swallowing_it() {
        // The exact shape onboarding used to turn into a silent empty-with-None: an FDA
        // denial (`Operation not permitted`) while reading a browser data directory.
        let denied = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Operation not permitted",
        ))
        .context("reading /Users/me/Library/Application Support/Google/Chrome");

        let fallback = browser_profiles_fallback(&denied, FullDiskAccessProbe::Inconclusive);
        assert!(fallback.profiles.is_empty(), "a denied discovery still yields no profiles");
        assert!(
            fallback.issue.is_some(),
            "the swallow is fixed: a discovery failure must carry a reason, never a silent None"
        );

        // On a Full-Disk-Access host that reason is the actionable FDA marker. The core
        // proves the verdict independent of the test host's own OS.
        assert_eq!(
            resolve_browser_discovery_issue_core(Err(()), true, true, false).as_deref(),
            Some(DISCOVERY_ISSUE_FULL_DISK_ACCESS),
            "a permission denial on a Full Disk Access host is the headline FDA case"
        );
    }
}
