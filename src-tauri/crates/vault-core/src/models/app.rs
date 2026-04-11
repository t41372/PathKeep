//! App-level configuration and shell read models.

use super::ArchiveMode;
use super::{
    AiIndexStatus, BackupRunOverview, BrowserProfile, ImportBatchOverview, InsightStatus,
    KeyringStatusReport,
};
use super::{AiSettings, DeterministicSettings, EnrichmentSettings};
use super::{ArchiveStatus, RemoteBackupConfig};
use super::{
    merge_deterministic_module_states, merge_enrichment_plugin_preferences,
    merge_enrichment_plugin_states,
};
use serde::{Deserialize, Serialize};

/// Repairs config payloads so newly added runtime defaults are always present.
pub fn normalize_app_config(config: &mut AppConfig) {
    config.enrichment.plugins = merge_enrichment_plugin_states(&config.enrichment.plugins);
    config.ai.enrichment_plugins =
        merge_enrichment_plugin_preferences(&config.ai.enrichment_plugins);
    config.deterministic.modules = merge_deterministic_module_states(&config.deterministic.modules);
}

/// User language selection persisted in config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum LanguagePreference {
    #[serde(rename = "system")]
    #[default]
    System,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
}

/// Persisted App Lock settings that shape future UI sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppLockConfig {
    pub enabled: bool,
    pub idle_timeout_minutes: u64,
    pub biometric_enabled: bool,
    pub passcode_enabled: bool,
    pub passcode_configured: bool,
    pub recovery_hint: Option<String>,
}

impl Default for AppLockConfig {
    /// Returns the accepted default App Lock policy for new installs.
    fn default() -> Self {
        Self {
            enabled: false,
            idle_timeout_minutes: 5,
            biometric_enabled: false,
            passcode_enabled: true,
            passcode_configured: false,
            recovery_hint: None,
        }
    }
}

/// Host biometric capability state surfaced to the desktop shell.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AppLockBiometricState {
    TouchIdAvailable,
    TouchIdUnavailable,
    #[default]
    Unsupported,
}

/// Runtime App Lock status shown by the shell.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppLockStatus {
    pub enabled: bool,
    pub locked: bool,
    pub idle_timeout_minutes: u64,
    pub biometric_available: bool,
    pub biometric_enabled: bool,
    pub biometric_state: AppLockBiometricState,
    pub passcode_enabled: bool,
    pub passcode_configured: bool,
    pub config_path: String,
    pub lock_reason: Option<String>,
    pub locked_at: Option<String>,
    pub last_unlocked_at: Option<String>,
    pub recovery_hint: Option<String>,
    pub warnings: Vec<String>,
    pub degradation_notes: Vec<String>,
}

/// Unlock request payload sent from the shell to the worker.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnlockAppSessionRequest {
    pub passcode: Option<String>,
    pub use_biometric: bool,
}

/// Request payload for configuring an App Lock passcode.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SetAppLockPasscodeRequest {
    pub passcode: String,
    pub recovery_hint: Option<String>,
}

/// User analytics consent settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnalyticsConfig {
    pub enabled: bool,
    pub consent_granted_at: Option<String>,
}

/// Persisted application configuration owned by Settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub initialized: bool,
    pub archive_mode: ArchiveMode,
    pub preferred_language: LanguagePreference,
    pub due_after_hours: u64,
    pub schedule_check_interval_hours: u64,
    pub checkpoint_days: u64,
    pub capture_favicons: bool,
    pub selected_profile_ids: Vec<String>,
    pub git_enabled: bool,
    pub remember_database_key_in_keyring: bool,
    pub app_autostart: bool,
    pub app_lock: AppLockConfig,
    pub analytics: AnalyticsConfig,
    pub remote_backup: RemoteBackupConfig,
    pub enrichment: EnrichmentSettings,
    pub deterministic: DeterministicSettings,
    pub ai: AiSettings,
}

impl Default for AppConfig {
    /// Returns the accepted config defaults for a fresh install.
    fn default() -> Self {
        Self {
            initialized: false,
            archive_mode: ArchiveMode::Plaintext,
            preferred_language: LanguagePreference::System,
            due_after_hours: 72,
            schedule_check_interval_hours: 6,
            checkpoint_days: 90,
            capture_favicons: true,
            selected_profile_ids: Vec::new(),
            git_enabled: true,
            remember_database_key_in_keyring: false,
            app_autostart: false,
            app_lock: AppLockConfig::default(),
            analytics: AnalyticsConfig::default(),
            remote_backup: RemoteBackupConfig::default(),
            enrichment: EnrichmentSettings::default(),
            deterministic: DeterministicSettings::default(),
            ai: AiSettings::default(),
        }
    }
}

/// Absolute application directories surfaced to the shell for diagnostics and previews.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDirectories {
    pub app_root: String,
    pub config_path: String,
    pub archive_database_path: String,
    pub audit_repo_path: String,
    pub manifests_dir: String,
    pub exports_dir: String,
    pub raw_snapshots_dir: String,
    pub staging_dir: String,
    pub quarantine_dir: String,
    pub schedule_dir: String,
    pub logs_dir: String,
    pub rust_log_path: String,
    pub frontend_log_path: String,
    pub crash_reports_dir: String,
    pub stronghold_path: String,
    pub stronghold_salt_path: String,
}

/// Immutable build metadata for About/support surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppBuildInfo {
    pub product_name: String,
    pub version: String,
    pub git_commit_short: String,
    pub git_commit_full: String,
    pub git_dirty: bool,
}

/// Availability snapshot returned by the updater check flow.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateAvailability {
    pub supported: bool,
    pub checked_at: String,
    pub available: bool,
    pub current_version: Option<String>,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub error: Option<String>,
    pub download_url: Option<String>,
}

/// Pending update metadata that the shell can keep around between steps.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PendingAppUpdate {
    pub current_version: Option<String>,
    pub version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
}

/// Full updater check result returned to the shell.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheckResult {
    pub availability: AppUpdateAvailability,
    pub pending_update: Option<PendingAppUpdate>,
}

/// Optional guard payload for installing a specific expected update version.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInstallRequest {
    pub expected_version: Option<String>,
}

/// Progress snapshot for the updater download/install flow.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInstallState {
    pub phase: String,
    pub version: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub content_length: Option<u64>,
    pub message: Option<String>,
}

/// Summary of one persisted crash or frontend error report.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CrashReportSummary {
    pub source: String,
    pub recorded_at: String,
    pub fatal: bool,
    pub message: String,
    pub location: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostics {
    pub log_directory: String,
    pub rust_log_path: String,
    pub frontend_log_path: String,
    pub crash_reports_directory: String,
    pub latest_crash_report: Option<CrashReportSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FrontendErrorReportRequest {
    pub source: String,
    pub message: String,
    pub stack: Option<String>,
    pub url: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub fatal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub directories: AppDirectories,
    pub runtime_diagnostics: RuntimeDiagnostics,
    pub config: AppConfig,
    pub archive_status: ArchiveStatus,
    pub app_lock_status: AppLockStatus,
    pub keyring_status: KeyringStatusReport,
    pub ai_status: AiIndexStatus,
    pub insight_status: InsightStatus,
    #[serde(alias = "chromeProfiles")]
    pub browser_profiles: Vec<BrowserProfile>,
    pub recent_runs: Vec<BackupRunOverview>,
    pub recent_import_batches: Vec<ImportBatchOverview>,
}
