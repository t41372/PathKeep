//! Trusted external-output and local-host Core Intelligence DTOs.
//!
//! ## Responsibilities
//! - Define embed card, widget snapshot, public snapshot, and deterministic
//!   local-host artifact payloads.
//! - Keep trusted-output bundles separate from in-app read-model DTOs.
//! - Preserve generated-file and installed-host contracts used by Settings.
//!
//! ## Not responsible for
//! - Rendering HTML/JS local-host artifacts.
//! - Verifying installed files on disk.
//! - Deciding which Core Intelligence sections are eligible for public export.
//!
//! ## Dependencies
//! - Shared entity references and date ranges.
//! - Digest/discovery/search DTOs reused in exported snapshots.
//! - `schedule::GeneratedFile` for preview/build file manifests.
//!
//! ## Performance notes
//! - Export payloads should contain curated summaries, not full archive-scale
//!   lists. Builders must keep embed cards and top-domain arrays bounded.

use super::{
    analytics::DiscoveryTrend,
    overview::DigestSummary,
    reads::EngineRanking,
    shared::{DateRange, InsightEntityReference},
};
use crate::models::schedule::GeneratedFile;
use serde::{Deserialize, Serialize};

/// Card payload used by embed/widget/public Core Intelligence outputs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceEmbedCardPayload {
    pub card_id: String,
    pub card_type: String,
    pub title: String,
    pub eyebrow: Option<String>,
    pub body: String,
    pub metric_label: Option<String>,
    pub metric_value: Option<String>,
    pub href: Option<String>,
    pub primary_target: Option<InsightEntityReference>,
    #[serde(default)]
    pub secondary_targets: Vec<InsightEntityReference>,
    pub internal_only: bool,
}

/// Widget-oriented snapshot built from trusted Core Intelligence summaries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceWidgetSnapshot {
    pub generated_at: String,
    pub date_range: DateRange,
    pub digest_summary: DigestSummary,
    pub highlights: Vec<IntelligenceEmbedCardPayload>,
    pub notes: Vec<String>,
}

/// Public snapshot with only share-safe aggregate fields.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligencePublicSnapshot {
    pub generated_at: String,
    pub date_range: DateRange,
    pub digest_summary: DigestSummary,
    pub top_domains: Vec<String>,
    pub search_engines: Vec<EngineRanking>,
    pub discovery_trend: DiscoveryTrend,
    pub notes: Vec<String>,
}

/// Machine-readable bundle saved beside one local Core Intelligence host.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceLocalHostBundle {
    pub bundle_version: String,
    pub host_id: String,
    pub generated_at: String,
    pub locale: String,
    pub date_range: DateRange,
    pub profile_id: Option<String>,
    pub embed_cards: Vec<IntelligenceEmbedCardPayload>,
    pub widget_snapshot: IntelligenceWidgetSnapshot,
    pub public_snapshot: IntelligencePublicSnapshot,
    pub trusted_only_card_ids: Vec<String>,
    pub trusted_only_card_count: usize,
    pub boundary_notes: Vec<String>,
}

/// Existing installed local-host artifact discovered on disk for verify UX.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceInstalledLocalHost {
    pub artifact_root: String,
    pub entry_file_path: String,
    pub bundle: IntelligenceLocalHostBundle,
}

/// Preview payload for one deterministic local-host artifact without writing files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceLocalHostPreview {
    pub artifact_root: String,
    pub entry_file_path: String,
    pub generated_files: Vec<GeneratedFile>,
    pub bundle: IntelligenceLocalHostBundle,
    pub boundary_notes: Vec<String>,
    pub manual_steps: Vec<String>,
    pub warnings: Vec<String>,
    pub installed_host: Option<IntelligenceInstalledLocalHost>,
}

/// Result payload after writing one deterministic local-host artifact bundle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceLocalHostBuildResult {
    pub artifact_root: String,
    pub entry_file_path: String,
    pub generated_files: Vec<GeneratedFile>,
    pub bundle: IntelligenceLocalHostBundle,
    pub boundary_notes: Vec<String>,
    pub manual_steps: Vec<String>,
    pub warnings: Vec<String>,
    pub installed_host: Option<IntelligenceInstalledLocalHost>,
}
