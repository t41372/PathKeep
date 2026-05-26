//! App-level configuration and shell read models.

use super::ArchiveMode;
use super::ArchiveStatus;
use super::{
    AiIndexStatus, BackupRunOverview, BrowserProfile, ImportBatchOverview, IntelligenceStatus,
    KeyringStatusReport,
};
use super::{AiSettings, DeterministicSettings, EnrichmentSettings};
use super::{
    merge_deterministic_module_states, merge_enrichment_plugin_preferences,
    merge_enrichment_plugin_states,
};
use serde::{Deserialize, Serialize};

const DEFAULT_EXPLORER_BACKGROUND_PREFETCH_PAGES: u64 = 5;
const MAX_EXPLORER_BACKGROUND_PREFETCH_PAGES: u64 = 10;

/// Repairs config payloads so newly added runtime defaults are always present.
pub fn normalize_app_config(config: &mut AppConfig) {
    config.enrichment.plugins = merge_enrichment_plugin_states(&config.enrichment.plugins);
    config.ai.enrichment_plugins =
        merge_enrichment_plugin_preferences(&config.ai.enrichment_plugins);
    config.deterministic.modules = merge_deterministic_module_states(&config.deterministic.modules);
    config.explorer_background_prefetch_pages =
        config.explorer_background_prefetch_pages.min(MAX_EXPLORER_BACKGROUND_PREFETCH_PAGES);
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

/// Persisted application configuration owned by Settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub initialized: bool,
    pub archive_mode: ArchiveMode,
    pub preferred_language: LanguagePreference,
    pub due_after_hours: f64,
    pub schedule_check_interval_hours: u64,
    pub checkpoint_days: u64,
    pub capture_favicons: bool,
    pub selected_profile_ids: Vec<String>,
    pub git_enabled: bool,
    pub remember_database_key_in_keyring: bool,
    pub app_autostart: bool,
    pub explorer_background_prefetch_pages: u64,
    pub app_lock: AppLockConfig,
    pub enrichment: EnrichmentSettings,
    pub deterministic: DeterministicSettings,
    pub ai: AiSettings,
    pub og_image: OgImageSettings,
}

/// How aggressively the og:image worker fetches link-preview bytes.
///
/// `Off`        — no fetching, anywhere. The frontend hook short-circuits
///                and the post-backup tick is skipped. Use this when you
///                want zero outbound HTTP for previews.
/// `OnDemand`   — fetches only fire when a card-mode row scrolls into
///                view (the legacy v0.2/v0.3 behaviour). Negative-cache
///                cooldowns are honoured but no proactive sweep runs.
/// `Background` — `OnDemand` PLUS a per-backup pass that enqueues
///                page URLs from the archive that don't yet have an
///                `og_images` row, and the daily negative-cache retry.
///                This is the default: it keeps social cards warm
///                without pinning UI activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OgImageFetchMode {
    Off,
    OnDemand,
    #[default]
    Background,
}

/// User-controllable og:image fetch + cache settings.
///
/// `fetch_enabled` is the legacy master kill switch and defaults to
/// true; `fetch_mode` is the finer-grained policy (default `Background`
/// — keeps the previously implicit post-backup refetch behaviour and
/// adds an explicit new-visit prefetch sweep). When `fetch_enabled` is
/// false, every code path treats it as `Off` regardless of `fetch_mode`.
///
/// `blocked_hosts` is a per-domain blocklist (one host per line in the
/// UI, normalized to lower-case here). `cleanup` chooses how the cache
/// is bounded; the default is `Off` per user direction (cache grows
/// unbounded until the user opts into time / size / LRU eviction).
///
/// Budgets:
/// - `daily_refetch_budget` caps how many already-negative-cached rows
///   the post-backup retry pass touches per day. Default 50.
/// - `new_visit_prefetch_budget` caps how many URLs the post-backup
///   prefetch pass enqueues per tick when running in `Background` mode.
///   Default 100. Zero disables the sweep without leaving `Background`
///   mode entirely (e.g. you still want daily refetch but no prefetch).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct OgImageSettings {
    pub fetch_enabled: bool,
    pub fetch_mode: OgImageFetchMode,
    pub daily_refetch_budget: u32,
    pub new_visit_prefetch_budget: u32,
    pub blocked_hosts: Vec<String>,
    pub cleanup: crate::models::OgImageCleanupMode,
}

impl OgImageSettings {
    /// Resolves the effective policy after honouring the legacy
    /// `fetch_enabled` kill switch. Use this when deciding whether any
    /// fetch should fire — it folds both knobs into one decision so
    /// callers do not have to remember the precedence rule.
    pub fn effective_mode(&self) -> OgImageFetchMode {
        if !self.fetch_enabled {
            return OgImageFetchMode::Off;
        }
        self.fetch_mode
    }
}

impl Default for OgImageSettings {
    fn default() -> Self {
        Self {
            fetch_enabled: true,
            fetch_mode: OgImageFetchMode::default(),
            daily_refetch_budget: 50,
            new_visit_prefetch_budget: 100,
            blocked_hosts: Vec::new(),
            cleanup: crate::models::OgImageCleanupMode::default(),
        }
    }
}

impl Default for AppConfig {
    /// Returns the accepted config defaults for a fresh install.
    fn default() -> Self {
        Self {
            initialized: false,
            archive_mode: ArchiveMode::Plaintext,
            preferred_language: LanguagePreference::System,
            due_after_hours: 72.0,
            schedule_check_interval_hours: 6,
            checkpoint_days: 90,
            capture_favicons: true,
            selected_profile_ids: Vec::new(),
            git_enabled: true,
            remember_database_key_in_keyring: false,
            app_autostart: false,
            explorer_background_prefetch_pages: DEFAULT_EXPLORER_BACKGROUND_PREFETCH_PAGES,
            app_lock: AppLockConfig::default(),
            enrichment: EnrichmentSettings::default(),
            deterministic: DeterministicSettings::default(),
            ai: AiSettings::default(),
            og_image: OgImageSettings::default(),
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
    pub search_database_path: String,
    pub intelligence_database_path: String,
    pub audit_repo_path: String,
    pub manifests_dir: String,
    pub exports_dir: String,
    pub raw_snapshots_dir: String,
    pub staging_dir: String,
    pub quarantine_dir: String,
    pub schedule_dir: String,
    pub semantic_index_dir: String,
    pub intelligence_blobs_dir: String,
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
    pub intelligence_status: IntelligenceStatus,
    #[serde(alias = "chromeProfiles")]
    pub browser_profiles: Vec<BrowserProfile>,
    pub recent_runs: Vec<BackupRunOverview>,
    pub recent_import_batches: Vec<ImportBatchOverview>,
}
