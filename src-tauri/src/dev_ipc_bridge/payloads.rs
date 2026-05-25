//! JSON payload DTOs accepted by the dev-only IPC bridge.
//!
//! ## Responsibilities
//!
//! - Keep the localhost bridge request envelopes in one small module.
//! - Preserve the camelCase payload shape consumed by browser automation.
//! - Translate command-specific JSON bodies into typed worker inputs.
//!
//! ## Not responsible for
//!
//! - Registering command names or choosing which worker function to call.
//! - Validating domain invariants beyond serde shape decoding.
//! - Exposing any production HTTP API contract.
//!
//! ## Dependencies
//!
//! - `serde` for request body decoding.
//! - `vault-core` request/input types already owned by the desktop command surface.
//!
//! ## Performance notes
//!
//! Payloads are tiny command envelopes. Large history datasets must stay behind
//! worker-side streaming, pagination, or batching; this module must not grow
//! fields that inline bulk browser history rows into the bridge body.

use serde::Deserialize;
use vault_core::{
    AiProviderSecretInput, AppConfig, AppUpdateInstallRequest, BrowserHistoryImportRequest,
    ExportRequest, HistoryFaviconLookupEntry, HistoryOgImageLookupEntry, HistoryQuery,
    ReplaceTagsRequest, SchedulePlan, SetNotesRequest, TakeoutRequest,
};

/// Carries archive bootstrap input across the browser automation mirror without
/// changing the production `initialize_archive` command signature.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InitializeArchivePayload {
    pub(super) config: AppConfig,
    pub(super) database_key: Option<String>,
}

/// Mirrors commands whose desktop payload is already grouped under a `request`
/// field, letting the bridge reuse the exact same typed request contract.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WrappedRequest<T> {
    pub(super) request: T,
}

/// Accepts the in-memory session key used by dev automation when the native
/// app session should be unlocked without touching keyring storage.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseKeyPayload {
    pub(super) database_key: String,
}

/// Carries the backup execution mode flag while keeping the dev mirror aligned
/// with the existing `run_backup_now` command body.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunBackupPayload {
    #[serde(default)]
    pub(super) due_only: bool,
}

/// Contains a paginated history query; callers must keep limits bounded because
/// the bridge is not a bulk export transport.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QueryHistoryPayload {
    pub(super) query: HistoryQuery,
}

/// Carries favicon lookup keys in the same batched form used by the desktop
/// facade, avoiding per-row bridge calls during Explorer rendering tests.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryFaviconPayload {
    pub(super) entries: Vec<HistoryFaviconLookupEntry>,
}

/// Mirrors the favicon lookup batching for og:image hydration so the
/// dev bridge can exercise the card-mode hydration path under Playwright.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryOgImagePayload {
    pub(super) entries: Vec<HistoryOgImageLookupEntry>,
}

/// Carries a flat list of page URLs for the og:image mark-shown,
/// trigger-refetch, and (future) selective-clear bridge endpoints.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OgImageUrlsPayload {
    pub(super) urls: Vec<String>,
}

/// Identifies an audit or job run for detail lookups without exposing archive
/// internals through the localhost bridge.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunIdPayload {
    pub(super) run_id: i64,
}

/// Carries export options for the existing desktop export command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportPayload {
    pub(super) request: ExportRequest,
}

/// Points a verifier command at an already-built bundle artifact on disk.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundlePathPayload {
    pub(super) bundle_path: String,
}

/// Carries Google Takeout scan or import options through the dev mirror.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TakeoutPayload {
    pub(super) request: TakeoutRequest,
}

/// Carries Browser Direct scan or import options through the dev mirror.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BrowserHistoryPayload {
    pub(super) request: BrowserHistoryImportRequest,
}

/// Selects an import batch for preview, revert, or restore commands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BatchIdPayload {
    pub(super) batch_id: i64,
}

/// Lets dev automation ask for platform-specific schedule behavior while still
/// allowing the worker to default to the host platform.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlatformPayload {
    pub(super) platform: Option<String>,
}

/// Carries the reviewed schedule plan into apply/remove commands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanPayload {
    pub(super) plan: SchedulePlan,
}

/// Carries an AI provider secret update without leaking provider-specific fields
/// into the command dispatcher.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiProviderSecretPayload {
    pub(super) input: AiProviderSecretInput,
}

/// Mirrors commands whose desktop payload is grouped under an `input` field
/// rather than a `request` field.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InputPayload<T> {
    pub(super) input: T,
}

/// Selects a configured AI provider for secret removal or provider-specific
/// management commands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderIdPayload {
    pub(super) provider_id: String,
}

/// Bounds queue execution during dev automation so callers can avoid launching
/// an unbounded batch of AI jobs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MaxJobsPayload {
    pub(super) max_jobs: Option<u32>,
}

/// Selects a queued AI or intelligence job for replay, cancel, retry, or detail
/// inspection.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct JobIdPayload {
    pub(super) job_id: i64,
}

/// Selects a canonical visit for navigation-path lookups.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VisitIdPayload {
    pub(super) visit_id: i64,
}

/// Carries an optional profile scope for commands whose default is "all active
/// profiles" rather than a single browser profile.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProfileIdPayload {
    pub(super) profile_id: Option<String>,
}

/// Selects a browsing session in the deterministic intelligence read model.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionIdPayload {
    pub(super) session_id: String,
}

/// Selects a search trail in the deterministic intelligence read model.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TrailIdPayload {
    pub(super) trail_id: String,
}

/// Selects a search-engine rule for deletion while preserving the rule registry
/// as the only owner of rule semantics.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuleIdPayload {
    pub(super) rule_id: String,
}

/// Carries a local path to the desktop file-manager helper.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PathPayload {
    pub(super) path: String,
}

/// Carries an external URL to the desktop launcher helper.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UrlPayload {
    pub(super) url: String,
}

/// Bridge envelope for the annotation-search command — the desktop signature
/// takes both a non-optional query string and an optional row cap.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AnnotationSearchPayload {
    pub(super) query: String,
    pub(super) limit: Option<usize>,
}

/// Bridge envelope for the annotation-list command — mirrors the desktop
/// signature's optional row cap.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AnnotationLimitPayload {
    pub(super) limit: Option<usize>,
}

/// Bridge envelope for set_url_notes / replace_url_tags — both desktop
/// commands group their typed input under a `request` field.
pub(super) type SetNotesPayload = WrappedRequest<SetNotesRequest>;
pub(super) type ReplaceTagsPayload = WrappedRequest<ReplaceTagsRequest>;

/// Bridge envelope for export_app_data — the desktop command takes the
/// user-chosen target path as a single string field.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportAppDataPayload {
    pub(super) target_path: String,
}

/// Bridge envelope for preview_app_data_import — carries the bundle path
/// the user picked from the OS file picker.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PreviewAppDataImportPayload {
    pub(super) bundle_path: String,
}

/// Bridge envelope for apply_app_data_import — carries the bundle path
/// plus the confirm-overwrite acknowledgement the Settings PME captured.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ApplyAppDataImportPayload {
    pub(super) bundle_path: String,
    pub(super) options: vault_core::ApplyImportOptions,
}

/// Provides an optional human-readable reason for locking the app session.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LockReasonPayload {
    pub(super) reason: Option<String>,
}

/// Carries a single secret value for legacy keyring commands that predate the
/// richer typed secret inputs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ValuePayload {
    pub(super) value: String,
}

/// Carries updater install options when browser automation mirrors the desktop
/// updater command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppUpdateInstallPayload {
    pub(super) request: Option<AppUpdateInstallRequest>,
}

/// Carries the app configuration saved by the bridge without changing the
/// existing desktop `save_config` payload.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WrappedConfigPayload {
    pub(super) config: AppConfig,
}
