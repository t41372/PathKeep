//! Canonical archive read/write models.

use super::ProgressLogEvent;
use serde::{Deserialize, Serialize};

/// Storage mode for the canonical archive database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ArchiveMode {
    #[serde(rename = "Plaintext", alias = "plaintext")]
    #[default]
    Plaintext,
    #[serde(rename = "Encrypted", alias = "encrypted")]
    Encrypted,
}

/// High-level archive readiness/unlock status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStatus {
    pub initialized: bool,
    pub encrypted: bool,
    pub unlocked: bool,
    pub database_path: String,
    pub last_successful_backup_at: Option<String>,
    pub warning: Option<String>,
}

/// Browser-managed retention boundary surfaced for honesty in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRetentionBoundary {
    pub kind: String,
    pub local_days: Option<u32>,
}

/// Discovered browser profile read model used before backup ingest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub profile_id: String,
    pub profile_name: String,
    pub browser_family: String,
    pub browser_name: String,
    pub user_name: Option<String>,
    pub profile_path: String,
    pub history_path: Option<String>,
    pub favicons_path: Option<String>,
    pub history_exists: bool,
    pub history_readable: bool,
    pub access_issue: Option<String>,
    pub browser_version: Option<String>,
    pub history_file_name: String,
    pub history_bytes: u64,
    pub favicons_bytes: u64,
    pub supporting_bytes: u64,
    pub retention_boundary: BrowserRetentionBoundary,
}

/// Compact run-ledger summary used in lists and dashboards.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupRunOverview {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub run_type: String,
    pub trigger: String,
    pub profile_scope: Vec<String>,
    pub manifest_hash: Option<String>,
    pub profiles_processed: usize,
    pub new_visits: usize,
    pub new_urls: usize,
    pub new_downloads: usize,
    /// Why a `failed` run failed (the full error chain). `None` for success/running rows. Surfaced so
    /// the Audit ledger never shows a failure without its reason — a silent failed backup is the one
    /// thing a backup tool must never do.
    pub error_message: Option<String>,
}

/// Per-profile backup summary returned by one run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupProfileSummary {
    pub profile_id: String,
    pub new_visits: usize,
    pub new_urls: usize,
    pub new_downloads: usize,
    pub checkpoint_created: bool,
    pub notes: Vec<String>,
}

/// Full backup/report payload returned by backup-like operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupReport {
    pub due_skipped: bool,
    pub reason: Option<String>,
    pub run: Option<BackupRunOverview>,
    pub profiles: Vec<BackupProfileSummary>,
    pub manifest_path: Option<String>,
    pub git_commit: Option<String>,
    pub warnings: Vec<String>,
}

/// Progress event streamed during a running backup.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgressEvent {
    pub phase: String,
    pub label: String,
    pub detail: String,
    pub step: usize,
    pub total_steps: usize,
    pub completed_profiles: usize,
    pub total_profiles: usize,
    pub profile_id: Option<String>,
    pub progress_current: Option<usize>,
    pub progress_total: Option<usize>,
    pub progress_percent: Option<f32>,
    pub log_lines: Vec<String>,
    pub source_label: Option<String>,
    pub processed_records: Option<usize>,
    pub total_records: Option<usize>,
    pub imported_records: Option<usize>,
    pub duplicate_records: Option<usize>,
    pub skipped_records: Option<usize>,
    pub log_events: Vec<ProgressLogEvent>,
}

impl BackupProgressEvent {
    /// Attaches one structured log event using the event's current counters.
    pub fn with_log_event(mut self, level: &str, code: &str) -> Self {
        self.log_events = vec![ProgressLogEvent {
            level: level.to_string(),
            code: code.to_string(),
            message: self.detail.clone(),
            source_label: self.source_label.clone().or_else(|| self.profile_id.clone()),
            diagnostic: None,
            processed_records: self.processed_records,
            total_records: self.total_records,
            imported_records: self.imported_records,
            duplicate_records: self.duplicate_records,
            skipped_records: self.skipped_records,
        }];
        self
    }
}

/// Disk-usage summary for archive-managed artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StorageSummary {
    pub archive_database_bytes: u64,
    pub source_evidence_database_bytes: u64,
    pub search_database_bytes: u64,
    pub intelligence_database_bytes: u64,
    pub manifest_bytes: u64,
    pub snapshot_bytes: u64,
    pub export_bytes: u64,
    pub staging_bytes: u64,
    pub quarantine_bytes: u64,
    pub semantic_sidecar_bytes: u64,
    pub intelligence_blob_bytes: u64,
}

/// Dashboard snapshot used by the archive home surface.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub generated_at: String,
    pub total_profiles: usize,
    pub total_urls: usize,
    pub total_visits: usize,
    pub total_downloads: usize,
    pub last_successful_backup_at: Option<String>,
    /// Earliest visit_time_iso across every visible visit, or `None` when the
    /// archive has zero rows. The dashboard "Span" stat reads this to label
    /// archive coverage (e.g. "1y 2m") instead of the time since last backup,
    /// which was confusing for users with imported historical data.
    pub earliest_visit_at: Option<String>,
    /// Latest visit_time_iso across every visible visit. Pairs with
    /// `earliest_visit_at`; both are populated together or both are `None`.
    pub latest_visit_at: Option<String>,
    pub recent_runs: Vec<BackupRunOverview>,
    pub storage: StorageSummary,
    pub next_action: Option<String>,
}

/// Request payload for replaying a checkpoint snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRestoreRequest {
    pub snapshot_path: String,
}

/// Result of a one-click full-archive restore from a verified safety snapshot.
///
/// Exists so the Phase-D recovery GUI can report, in one transparent payload, exactly what the
/// headline restore did: which snapshot it reinstalled (and in which at-rest mode), where it
/// quarantined the broken canonical files it superseded, whether it had to rebuild an empty
/// source-evidence, and any non-fatal warnings — never a black-box "fixed it".
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FullArchiveRestoreReport {
    /// The `archive_restore` run-ledger row id recorded in the restored archive. `None` when the
    /// archive was already restored + reconciled + verified-healthy but the audit-run bookkeeping
    /// (ledger INSERT/UPDATE or manifest write) failed — that is downgraded to a `warnings` entry
    /// rather than failing an already-healed restore, so a `None` here always carries a matching
    /// warning.
    pub run_id: Option<i64>,
    /// The safety snapshot the canonical archive was restored from.
    pub restored_snapshot_path: String,
    /// The at-rest mode the restored canonical archive really opens in (config reconciled to match).
    pub restored_mode: ArchiveMode,
    /// The dated `quarantine/<ts>/` directory the superseded broken canonical files were moved into.
    pub quarantine_dir: String,
    /// True when a previous source-evidence database was quarantined and replaced with a fresh
    /// empty one consistent with the restored history-vault + config.
    pub source_evidence_rebuilt: bool,
    /// Non-fatal notes for the recovery screen (the FE localizes around them).
    pub warnings: Vec<String>,
}

/// One verified full-archive safety snapshot the Phase-D recovery GUI can offer as a restore source.
///
/// Exists to give the recovery screen rich, keyless metadata (date / size / which op produced it /
/// whether it even opens) WITHOUT a full-DB scan, so the user picks a restore point from honest
/// information. `verified_openable` is a cheap header-only check; the authoritative keyed
/// `quick_check` runs at restore time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    /// Stable identity for the FE list — the snapshot's display path.
    pub id: String,
    /// The snapshot file's path (the value a restore request echoes back).
    pub path: String,
    /// RFC 3339 creation time from the file mtime (`None` when the mtime is unreadable).
    pub created_at: Option<String>,
    /// On-disk size in bytes.
    pub size_bytes: u64,
    /// KEYLESS header-only openability signal (never a b-tree walk). For an encrypted snapshot
    /// this is structural-only (size); the authoritative keyed `quick_check` runs at restore.
    pub verified_openable: bool,
    /// Which whole-archive rewrite produced it: `"rekey"`, `"reconcile"`, `"import"`, `"periodic"`,
    /// or `"unknown"` for an unexpected bucket.
    pub source_op: String,
    /// Short English fallback descriptor; the FE localizes it.
    pub label: String,
}

/// Preview payload for a checkpoint restore before execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRestorePreview {
    pub snapshot_path: String,
    pub snapshot_kind: String,
    pub source_run_id: Option<i64>,
    pub source_profile_id: Option<String>,
    pub source_browser_name: Option<String>,
    pub created_at: Option<String>,
    pub reason: Option<String>,
    pub execute_supported: bool,
    pub estimated_visits: usize,
    pub estimated_urls: usize,
    pub estimated_downloads: usize,
    pub warnings: Vec<String>,
}

/// One retention bucket that can be previewed or pruned explicitly.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetentionBucket {
    pub id: String,
    pub bytes: u64,
    pub item_count: usize,
    pub paths: Vec<String>,
}

/// Preview payload for retention pruning.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPreview {
    pub buckets: Vec<RetentionBucket>,
    pub warnings: Vec<String>,
}

/// Request payload for retention pruning.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPruneRequest {
    pub bucket_ids: Vec<String>,
}

/// Result payload for a retention-prune execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPruneResult {
    pub run_id: Option<i64>,
    pub deleted_bytes: u64,
    pub deleted_files: usize,
    pub buckets: Vec<RetentionBucket>,
    pub warnings: Vec<String>,
}

/// History query contract used by archive recall and export surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub q: Option<String>,
    pub profile_id: Option<String>,
    pub browser_kind: Option<String>,
    pub domain: Option<String>,
    pub start_time_ms: Option<i64>,
    pub end_time_ms: Option<i64>,
    pub sort: Option<String>,
    pub limit: Option<u32>,
    pub page: Option<u32>,
    pub cursor: Option<String>,
    pub regex_mode: Option<bool>,
}

impl Default for HistoryQuery {
    /// Returns the default archive recall settings used by the shell.
    fn default() -> Self {
        Self {
            q: None,
            profile_id: None,
            browser_kind: None,
            domain: None,
            start_time_ms: None,
            end_time_ms: None,
            sort: None,
            limit: Some(150),
            page: None,
            cursor: None,
            regex_mode: Some(false),
        }
    }
}

/// One favicon payload returned alongside a visible history row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFavicon {
    pub data_url: String,
}

/// One batch favicon lookup entry used by Explorer's post-reveal icon hydration path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFaviconLookupEntry {
    pub profile_id: String,
    pub url: String,
    #[serde(default)]
    pub visit_time: i64,
}

/// One resolved favicon payload returned by the lazy Explorer icon lookup command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFaviconLookupResult {
    pub profile_id: String,
    pub url: String,
    pub visit_time: i64,
    pub favicon: Option<HistoryFavicon>,
}

/// One og:image payload returned alongside a card-mode history row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryOgImage {
    pub data_url: String,
}

/// One batch og:image lookup entry used by the card-mode hydration path.
///
/// Unlike favicon lookup the lookup key is page-URL only — there is no
/// visit-time scoping because the og:image describes the page itself, and
/// no profile partitioning because the cache is shared across browsers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryOgImageLookupEntry {
    pub url: String,
}

/// One resolved og:image payload returned by the lazy card-mode lookup command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryOgImageLookupResult {
    pub url: String,
    pub og_image: Option<HistoryOgImage>,
    pub fetch_status: String,
}

/// Counts and bytes reported by the og:image storage stats command.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OgImageStorageStats {
    pub row_count: i64,
    pub blob_count: i64,
    pub total_bytes: i64,
    pub oldest_fetched_at: Option<String>,
}

/// Coverage of og:image fetching across the archive's web pages, surfaced in
/// Settings → Link previews. Raw counts only — the UI derives the percentages so
/// it can present both coverage (of all eligible pages) and the success rate (of
/// pages actually checked) honestly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OgImageCoverageStats {
    /// Distinct HTTPS pages in the archive — the eligible denominator. Only
    /// https is counted: the fetcher never fetches http:// (it short-circuits to
    /// `parse_error`), so http pages can never carry a preview.
    pub eligible_pages: i64,
    /// Distinct HTTPS pages an og:image fetch has been attempted for.
    pub attempted_pages: i64,
    /// Pages with a successfully fetched og:image (`fetch_status = 'ok'`).
    pub pages_with_image: i64,
}

/// Cleanup mode chosen by the user. Default is `Off` — cache grows unbounded
/// and only manual "Clear cache" clears it.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum OgImageCleanupMode {
    #[default]
    Off,
    /// Delete rows older than `max_age_days`.
    TimeTtl { max_age_days: u32 },
    /// Delete oldest-fetched rows until total bytes drop below `max_bytes`.
    SizeCap { max_bytes: u64 },
    /// Delete least-recently-shown rows until total bytes drop below `max_bytes`.
    Lru { max_bytes: u64 },
}

/// Outcome of one cleanup pass — reported back to the UI so the user sees
/// how many rows/blobs were evicted and how many bytes were reclaimed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OgImageCleanupReport {
    pub deleted_rows: i64,
    pub deleted_blobs: i64,
    pub reclaimed_bytes: i64,
}

/// One visible visit row returned by archive recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub favicon: Option<HistoryFavicon>,
    pub visited_at: String,
    pub visit_time: i64,
    pub duration_ms: Option<i64>,
    pub transition: Option<i64>,
    pub source_visit_id: i64,
    pub app_id: Option<String>,
    /// Capped excerpt of the matched URL's enrichment text (W-ENRICH-1, 06 §6).
    ///
    /// Populated ONLY by the lexical-search recall path, and only when the
    /// matched `search_documents` row carries non-empty enrichment text (a
    /// content-fetch summary + GitHub topics/desc). Plain browse, regex, and
    /// fuzzy-fallback rows leave this `None` so the Explorer affordance stays
    /// suppressed outside keyword search. `#[serde(default)]` keeps older
    /// payloads (and preview fixtures) that omit it deserializable.
    #[serde(default)]
    pub enrichment_excerpt: Option<String>,
}

/// Paginated history-query response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQueryResponse {
    pub total: usize,
    pub items: Vec<HistoryEntry>,
    pub page: usize,
    pub page_size: usize,
    pub page_count: usize,
    pub has_previous: bool,
    pub has_next: bool,
    pub next_cursor: Option<String>,
}

impl Default for HistoryQueryResponse {
    fn default() -> Self {
        Self {
            total: 0,
            items: Vec::new(),
            page: 1,
            page_size: 0,
            page_count: 1,
            has_previous: false,
            has_next: false,
            next_cursor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Html,
    Markdown,
    Text,
    Jsonl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub query: HistoryQuery,
    pub format: ExportFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub format: ExportFormat,
    pub path: String,
    pub count: usize,
}
